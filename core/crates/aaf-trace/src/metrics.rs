//! Operational metric aggregation over a set of traces.

use aaf_contracts::{ExecutionTrace, TraceStatus};

/// Aggregate operational metrics across a set of traces.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct TraceMetrics {
    /// Number of traces analysed.
    pub total_traces: u64,
    /// How many of them completed successfully.
    pub completed: u64,
    /// How many failed terminally.
    pub failed: u64,
    /// How many returned a partial result.
    pub partial: u64,
    /// How many were cancelled.
    pub cancelled: u64,
    /// Total cost across all traces.
    pub total_cost_usd: f64,
    /// Mean cost per trace.
    pub mean_cost_usd: f64,
    /// Mean number of steps per trace.
    pub mean_steps: f64,
    /// Resolution rate — completed / total.
    pub intent_resolution_rate: f64,
}

impl TraceMetrics {
    /// Compute aggregate metrics for the given traces.
    pub fn compute<'a, I: IntoIterator<Item = &'a ExecutionTrace>>(traces: I) -> Self {
        let mut m = Self::default();
        let mut total_steps: u64 = 0;
        for t in traces {
            m.total_traces += 1;
            m.total_cost_usd += t.total_cost_usd;
            total_steps += t.steps.len() as u64;
            match t.status {
                TraceStatus::Completed => m.completed += 1,
                TraceStatus::Failed => m.failed += 1,
                TraceStatus::Partial => m.partial += 1,
                TraceStatus::Cancelled => m.cancelled += 1,
                TraceStatus::Running => {}
            }
        }
        if m.total_traces > 0 {
            m.mean_cost_usd = m.total_cost_usd / m.total_traces as f64;
            m.mean_steps = total_steps as f64 / m.total_traces as f64;
            m.intent_resolution_rate = m.completed as f64 / m.total_traces as f64;
        }
        m
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{IntentId, NodeId, Observation, StepOutcome, TraceId, TraceStep};

    fn finished(cost: f64, status: TraceStatus, steps: u32) -> ExecutionTrace {
        let trace_id = TraceId::new();
        let mut t = ExecutionTrace::open(trace_id.clone(), IntentId::new());
        for s in 1..=steps {
            t.record(TraceStep {
                step: s,
                node_id: NodeId::new(),
                step_type: "x".into(),
                model: None,
                tokens_in: 0,
                tokens_out: 0,
                cost_usd: cost / steps as f64,
                duration_ms: 1,
                observation: Observation::minimal(
                    trace_id.clone(),
                    NodeId::new(),
                    s,
                    "system".into(),
                    StepOutcome::Success,
                ),
            });
        }
        t.close(status);
        t
    }

    #[test]
    fn aggregate_three_traces() {
        let traces = vec![
            finished(0.1, TraceStatus::Completed, 2),
            finished(0.2, TraceStatus::Failed, 1),
            finished(0.3, TraceStatus::Completed, 3),
        ];
        let m = TraceMetrics::compute(&traces);
        assert_eq!(m.total_traces, 3);
        assert_eq!(m.completed, 2);
        assert_eq!(m.failed, 1);
        assert!((m.total_cost_usd - 0.6).abs() < 1e-9);
        assert!((m.intent_resolution_rate - 2.0 / 3.0).abs() < 1e-9);
    }
}
