//! Checkpoint replay divergence detection.
//!
//! Given two [`aaf_contracts::ExecutionTrace`]s produced by running the
//! same intent against two different runtime configurations (e.g.
//! baseline and candidate), [`Replayer::diverges`] returns a list of
//! [`Divergence`]s — step-level differences that a human reviewer or
//! CI gate can act on.

use aaf_contracts::{ExecutionTrace, TraceStep};
use serde::{Deserialize, Serialize};

/// One divergence between two runs of the same intent.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Divergence {
    /// Step exists in the baseline but not the candidate (or vice versa).
    StepMissing {
        /// Step number.
        step: u32,
        /// `"baseline"` or `"candidate"`.
        side: String,
    },
    /// Same step id but different step type.
    StepTypeChanged {
        /// Step number.
        step: u32,
        /// Baseline step type.
        baseline: String,
        /// Candidate step type.
        candidate: String,
    },
    /// Same step id but cost drifted beyond `tolerance_usd`.
    CostDrift {
        /// Step number.
        step: u32,
        /// Baseline cost.
        baseline_cost: f64,
        /// Candidate cost.
        candidate_cost: f64,
    },
    /// Same step id but latency drifted beyond `tolerance_ms`.
    LatencyDrift {
        /// Step number.
        step: u32,
        /// Baseline duration.
        baseline_ms: u64,
        /// Candidate duration.
        candidate_ms: u64,
    },
    /// Terminal status changed.
    StatusChanged {
        /// Baseline status.
        baseline: String,
        /// Candidate status.
        candidate: String,
    },
}

/// Divergence detection helper.
pub struct Replayer {
    /// Cost drift tolerance in USD.
    pub cost_tolerance_usd: f64,
    /// Latency drift tolerance in milliseconds.
    pub latency_tolerance_ms: u64,
}

impl Default for Replayer {
    fn default() -> Self {
        Self {
            cost_tolerance_usd: 0.0005,
            latency_tolerance_ms: 50,
        }
    }
}

impl Replayer {
    /// Compare two traces step-by-step and return every divergence.
    pub fn diverges(
        &self,
        baseline: &ExecutionTrace,
        candidate: &ExecutionTrace,
    ) -> Vec<Divergence> {
        let mut out = vec![];
        if baseline.status != candidate.status {
            out.push(Divergence::StatusChanged {
                baseline: format!("{:?}", baseline.status),
                candidate: format!("{:?}", candidate.status),
            });
        }
        let max_steps = baseline.steps.len().max(candidate.steps.len());
        for i in 0..max_steps {
            let b = baseline.steps.get(i);
            let c = candidate.steps.get(i);
            match (b, c) {
                (Some(b), Some(c)) => self.compare_step(b, c, &mut out),
                (Some(b), None) => out.push(Divergence::StepMissing {
                    step: b.step,
                    side: "candidate".into(),
                }),
                (None, Some(c)) => out.push(Divergence::StepMissing {
                    step: c.step,
                    side: "baseline".into(),
                }),
                _ => {}
            }
        }
        out
    }

    fn compare_step(&self, b: &TraceStep, c: &TraceStep, out: &mut Vec<Divergence>) {
        if b.step_type != c.step_type {
            out.push(Divergence::StepTypeChanged {
                step: b.step,
                baseline: b.step_type.clone(),
                candidate: c.step_type.clone(),
            });
        }
        if (b.cost_usd - c.cost_usd).abs() > self.cost_tolerance_usd {
            out.push(Divergence::CostDrift {
                step: b.step,
                baseline_cost: b.cost_usd,
                candidate_cost: c.cost_usd,
            });
        }
        let dur_delta = b.duration_ms.abs_diff(c.duration_ms);
        if dur_delta > self.latency_tolerance_ms {
            out.push(Divergence::LatencyDrift {
                step: b.step,
                baseline_ms: b.duration_ms,
                candidate_ms: c.duration_ms,
            });
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        ExecutionTrace, IntentId, NodeId, Observation, StepOutcome, TraceId, TraceStatus,
    };

    fn t(cost: f64, duration_ms: u64, step_type: &str) -> ExecutionTrace {
        let tid = TraceId::new();
        let mut tr = ExecutionTrace::open(tid.clone(), IntentId::new());
        tr.record(TraceStep {
            step: 1,
            node_id: NodeId::from("a"),
            step_type: step_type.into(),
            model: None,
            tokens_in: 10,
            tokens_out: 20,
            cost_usd: cost,
            duration_ms,
            observation: Observation::minimal(
                tid,
                NodeId::from("a"),
                1,
                "agent".into(),
                StepOutcome::Success,
            ),
        });
        tr.close(TraceStatus::Completed);
        tr
    }

    #[test]
    fn identical_traces_have_no_divergence() {
        let r = Replayer::default();
        let a = t(0.01, 100, "node_run");
        let b = t(0.01, 100, "node_run");
        assert!(r.diverges(&a, &b).is_empty());
    }

    #[test]
    fn cost_drift_is_detected() {
        let r = Replayer::default();
        let a = t(0.01, 100, "node_run");
        let b = t(0.05, 100, "node_run");
        let d = r.diverges(&a, &b);
        assert!(d.iter().any(|x| matches!(x, Divergence::CostDrift { .. })));
    }

    #[test]
    fn latency_drift_is_detected() {
        let r = Replayer::default();
        let a = t(0.01, 100, "node_run");
        let b = t(0.01, 300, "node_run");
        let d = r.diverges(&a, &b);
        assert!(d
            .iter()
            .any(|x| matches!(x, Divergence::LatencyDrift { .. })));
    }

    #[test]
    fn step_type_change_is_detected() {
        let r = Replayer::default();
        let a = t(0.01, 100, "node_run");
        let b = t(0.01, 100, "agent_run");
        let d = r.diverges(&a, &b);
        assert!(d
            .iter()
            .any(|x| matches!(x, Divergence::StepTypeChanged { .. })));
    }
}
