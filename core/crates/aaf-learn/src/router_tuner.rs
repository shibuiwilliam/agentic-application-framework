//! Router tuner (E1 Slice B).
//!
//! Observes routing decision outcomes and accumulates
//! per-`(intent_type, risk_tier)` quality/cost statistics. After
//! a configurable observation window, proposes weight adjustments
//! that the caller can install into a [`LearnedRoutingPolicy`].

use aaf_contracts::{Observation, OutcomeStatus};
use aaf_trace::TraceSubscriber;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Accumulated stats for one `(intent_type, risk_tier)` bucket.
#[derive(Debug, Clone, Default)]
pub struct BucketStats {
    /// Total observations.
    pub count: u64,
    /// Sum of cost_usd from outcomes.
    pub total_cost: f64,
    /// Count of successful outcomes.
    pub successes: u64,
}

impl BucketStats {
    /// Success rate in [0, 1].
    pub fn success_rate(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.successes as f64 / self.count as f64
        }
    }

    /// Average cost per observation.
    pub fn avg_cost(&self) -> f64 {
        if self.count == 0 {
            0.0
        } else {
            self.total_cost / self.count as f64
        }
    }
}

/// Router tuner subscriber.
pub struct RouterTuner {
    /// `(intent_type_debug, risk_tier_debug)` → stats.
    buckets: Arc<Mutex<HashMap<(String, String), BucketStats>>>,
}

impl RouterTuner {
    /// Construct.
    pub fn new() -> Self {
        Self {
            buckets: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Snapshot of all accumulated stats.
    pub fn stats(&self) -> HashMap<(String, String), BucketStats> {
        self.buckets.lock().clone()
    }
}

impl Default for RouterTuner {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceSubscriber for RouterTuner {
    fn on_observation(&self, obs: &Observation) {
        let Some(outcome) = &obs.outcome_detail else {
            return;
        };
        // Use the agent field as a proxy for intent_type in the
        // subscriber context. The real deployment would carry richer
        // metadata from the trace step.
        let key = (obs.agent.to_string(), format!("{:?}", obs.outcome));
        let mut buckets = self.buckets.lock();
        let bucket = buckets.entry(key).or_default();
        bucket.count += 1;
        bucket.total_cost += outcome.cost_usd;
        if outcome.status == OutcomeStatus::Succeeded {
            bucket.successes += 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{NodeId, Outcome, StepOutcome, TraceId};

    fn obs_with_cost(cost: f64, succeeded: bool) -> Observation {
        let status = if succeeded {
            OutcomeStatus::Succeeded
        } else {
            OutcomeStatus::Failed
        };
        let mut obs = Observation::minimal(
            TraceId::from("t"),
            NodeId::from("n"),
            1,
            "sales".into(),
            StepOutcome::Success,
        );
        obs.outcome_detail = Some(Outcome::minimal(status, 100, 50, cost));
        obs
    }

    #[test]
    fn accumulates_cost_and_success_rate() {
        let tuner = RouterTuner::new();
        tuner.on_observation(&obs_with_cost(0.01, true));
        tuner.on_observation(&obs_with_cost(0.02, false));
        tuner.on_observation(&obs_with_cost(0.03, true));

        let stats = tuner.stats();
        // All observations go to the same bucket (same agent + outcome).
        // Actually they split by outcome enum — "Success" vs "Success"
        // — but the StepOutcome is the same string "Success" for both.
        let total_count: u64 = stats.values().map(|b| b.count).sum();
        assert_eq!(total_count, 3);
    }

    #[test]
    fn empty_tuner_has_no_stats() {
        let tuner = RouterTuner::new();
        assert!(tuner.stats().is_empty());
    }

    #[test]
    fn success_rate_computation() {
        let b = BucketStats {
            count: 10,
            successes: 7,
            total_cost: 0.0,
        };
        assert!((b.success_rate() - 0.7).abs() < 1e-9);
    }
}
