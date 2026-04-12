//! Capability scorer (E1 Slice B).
//!
//! Watches outcomes and nudges the reputation score of the
//! capability that produced each observation. Successful outcomes
//! push the score towards 1.0; failed outcomes push it towards 0.0.
//! The score is clamped to `[0.0, 1.0]` and the delta per
//! observation is configurable.

use aaf_contracts::{Observation, OutcomeStatus};
use aaf_trace::TraceSubscriber;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Configuration for the scorer.
#[derive(Debug, Clone)]
pub struct ScorerConfig {
    /// How much a single success nudges the score upward.
    pub success_delta: f32,
    /// How much a single failure nudges the score downward.
    pub failure_delta: f32,
}

impl Default for ScorerConfig {
    fn default() -> Self {
        Self {
            success_delta: 0.02,
            failure_delta: 0.05,
        }
    }
}

/// Accumulated reputation scores per node (agent) id.
pub struct CapabilityScorer {
    config: ScorerConfig,
    /// `agent_id → accumulated score`.
    scores: Arc<Mutex<HashMap<String, f32>>>,
}

impl CapabilityScorer {
    /// Construct with the given config.
    pub fn new(config: ScorerConfig) -> Self {
        Self {
            config,
            scores: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Current score for a given agent id. Returns 0.5 (neutral) if
    /// never observed.
    pub fn score_for(&self, agent: &str) -> f32 {
        *self.scores.lock().get(agent).unwrap_or(&0.5)
    }

    /// All current scores.
    pub fn all_scores(&self) -> HashMap<String, f32> {
        self.scores.lock().clone()
    }
}

impl TraceSubscriber for CapabilityScorer {
    fn on_observation(&self, obs: &Observation) {
        let status = match &obs.outcome_detail {
            Some(o) => o.status,
            None => return,
        };
        let delta = match status {
            OutcomeStatus::Succeeded => self.config.success_delta,
            OutcomeStatus::Failed | OutcomeStatus::RolledBack => -self.config.failure_delta,
            _ => 0.0,
        };
        if delta.abs() < f32::EPSILON {
            return;
        }
        let mut scores = self.scores.lock();
        let score = scores.entry(obs.agent.to_string()).or_insert(0.5);
        *score = (*score + delta).clamp(0.0, 1.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{NodeId, Outcome, StepOutcome, TraceId};

    fn obs_with_status(status: OutcomeStatus) -> Observation {
        let mut obs = Observation::minimal(
            TraceId::from("t1"),
            NodeId::from("n"),
            1,
            "agent-x".into(),
            StepOutcome::Success,
        );
        obs.outcome_detail = Some(Outcome::minimal(status, 100, 50, 0.01));
        obs
    }

    #[test]
    fn successful_outcome_nudges_score_up() {
        let scorer = CapabilityScorer::new(ScorerConfig::default());
        scorer.on_observation(&obs_with_status(OutcomeStatus::Succeeded));
        assert!(scorer.score_for("agent-x") > 0.5);
    }

    #[test]
    fn failed_outcome_nudges_score_down() {
        let scorer = CapabilityScorer::new(ScorerConfig::default());
        scorer.on_observation(&obs_with_status(OutcomeStatus::Failed));
        assert!(scorer.score_for("agent-x") < 0.5);
    }

    #[test]
    fn score_bounded_in_0_1() {
        let scorer = CapabilityScorer::new(ScorerConfig {
            success_delta: 0.0,
            failure_delta: 1.0, // massive nudge
        });
        // 5 failures should not push below 0.
        for _ in 0..5 {
            scorer.on_observation(&obs_with_status(OutcomeStatus::Failed));
        }
        assert!((scorer.score_for("agent-x") - 0.0).abs() < 1e-6);
    }

    #[test]
    fn partial_outcome_does_not_change_score() {
        let scorer = CapabilityScorer::new(ScorerConfig::default());
        scorer.on_observation(&obs_with_status(OutcomeStatus::Partial));
        assert!((scorer.score_for("agent-x") - 0.5).abs() < 1e-6);
    }
}
