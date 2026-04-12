//! Score history and event accounting.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One scoring event recorded against an agent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum ScoreEvent {
    /// Successful execution.
    Success,
    /// Human override applied (lowers trust).
    HumanOverride,
    /// Policy violation detected (significantly lowers trust).
    PolicyViolation,
    /// Accuracy regression detected.
    AccuracyRegression,
}

impl ScoreEvent {
    /// Multiplicative weight applied to the rolling score.
    pub fn delta(self) -> f64 {
        match self {
            ScoreEvent::Success => 0.005,
            ScoreEvent::HumanOverride => -0.02,
            ScoreEvent::PolicyViolation => -0.30,
            ScoreEvent::AccuracyRegression => -0.05,
        }
    }
}

/// Time-stamped score history.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ScoreHistory {
    /// Total executions observed.
    pub total: u64,
    /// Number of successes.
    pub success: u64,
    /// Number of human overrides.
    pub human_override: u64,
    /// Number of policy violations.
    pub policy_violations: u64,
    /// Last update timestamp.
    pub last_updated: Option<DateTime<Utc>>,
}

impl ScoreHistory {
    /// Apply an event and return the score delta produced.
    pub fn observe(&mut self, event: ScoreEvent) -> f64 {
        self.total += 1;
        match event {
            ScoreEvent::Success => self.success += 1,
            ScoreEvent::HumanOverride => self.human_override += 1,
            ScoreEvent::PolicyViolation => self.policy_violations += 1,
            ScoreEvent::AccuracyRegression => {}
        }
        self.last_updated = Some(Utc::now());
        event.delta()
    }

    /// Override rate within [0,1] (0 if no executions).
    pub fn override_rate(&self) -> f64 {
        if self.total == 0 {
            0.0
        } else {
            self.human_override as f64 / self.total as f64
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_rate_tracks_total() {
        let mut h = ScoreHistory::default();
        for _ in 0..98 {
            h.observe(ScoreEvent::Success);
        }
        for _ in 0..2 {
            h.observe(ScoreEvent::HumanOverride);
        }
        assert!((h.override_rate() - 0.02).abs() < 1e-9);
    }
}
