//! Escalation tuner (E1 Slice B).
//!
//! Tracks approval-overridden observations and accumulates stats.
//! When the override rate exceeds a threshold, the tuner can
//! recommend adjusting approval thresholds within policy-pack bounds.

use aaf_contracts::{Observation, OutcomeStatus};
use aaf_trace::TraceSubscriber;
use parking_lot::Mutex;
use std::sync::Arc;

/// Accumulated escalation statistics.
#[derive(Debug, Clone, Default)]
pub struct EscalationStats {
    /// Total observations processed.
    pub total: u64,
    /// Observations that were escalated (status == Escalated).
    pub escalated: u64,
    /// Escalated observations whose follow-up was successful.
    pub escalated_then_succeeded: u64,
}

impl EscalationStats {
    /// Escalation rate in [0, 1].
    pub fn escalation_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.escalated as f64 / self.total as f64
        }
    }

    /// Of the escalated observations, what fraction succeeded on
    /// retry? High values suggest the escalation was unnecessary.
    pub fn false_escalation_rate(&self) -> f64 {
        if self.escalated == 0 {
            0.0
        } else {
            self.escalated_then_succeeded as f64 / self.escalated as f64
        }
    }
}

/// Escalation tuner subscriber.
pub struct EscalationTuner {
    stats: Arc<Mutex<EscalationStats>>,
}

impl EscalationTuner {
    /// Construct.
    pub fn new() -> Self {
        Self {
            stats: Arc::new(Mutex::new(EscalationStats::default())),
        }
    }

    /// Snapshot of accumulated stats.
    pub fn stats(&self) -> EscalationStats {
        self.stats.lock().clone()
    }
}

impl Default for EscalationTuner {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceSubscriber for EscalationTuner {
    fn on_observation(&self, obs: &Observation) {
        let Some(outcome) = &obs.outcome_detail else {
            return;
        };
        let mut stats = self.stats.lock();
        stats.total += 1;
        if outcome.status == OutcomeStatus::Escalated {
            stats.escalated += 1;
        }
        // Heuristic: an escalated observation whose *next* outcome
        // in the same trace is Succeeded suggests the escalation was
        // a false alarm. In the subscriber context we see each
        // observation individually; tracking same-trace follow-ups
        // requires richer context that Slice C will add. For now,
        // count escalated observations only.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{NodeId, Outcome, StepOutcome, TraceId};

    fn obs(status: OutcomeStatus) -> Observation {
        let mut o = Observation::minimal(
            TraceId::from("t"),
            NodeId::from("n"),
            1,
            "a".into(),
            StepOutcome::Success,
        );
        o.outcome_detail = Some(Outcome::minimal(status, 100, 50, 0.01));
        o
    }

    #[test]
    fn counts_escalations() {
        let tuner = EscalationTuner::new();
        tuner.on_observation(&obs(OutcomeStatus::Succeeded));
        tuner.on_observation(&obs(OutcomeStatus::Escalated));
        tuner.on_observation(&obs(OutcomeStatus::Succeeded));
        let s = tuner.stats();
        assert_eq!(s.total, 3);
        assert_eq!(s.escalated, 1);
        assert!((s.escalation_rate() - 1.0 / 3.0).abs() < 1e-9);
    }

    #[test]
    fn empty_tuner_has_zero_rate() {
        let tuner = EscalationTuner::new();
        let s = tuner.stats();
        assert!((s.escalation_rate() - 0.0).abs() < 1e-9);
        assert!((s.false_escalation_rate() - 0.0).abs() < 1e-9);
    }
}
