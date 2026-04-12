//! Execution Trace contract — append-only record of every step the
//! runtime executes.

use crate::ids::{IntentId, NodeId, TraceId};
use crate::observation::Observation;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Status of a completed trace.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TraceStatus {
    /// Currently running.
    Running,
    /// Completed successfully.
    Completed,
    /// Failed terminally.
    Failed,
    /// Cancelled.
    Cancelled,
    /// Partially completed (graceful degradation under budget exhaustion).
    Partial,
}

/// One step in an [`ExecutionTrace`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TraceStep {
    /// Step number, monotonically increasing.
    pub step: u32,
    /// Node id this step ran.
    pub node_id: NodeId,
    /// Logical step type (`intent_compilation`, `agent_execution`, ...).
    pub step_type: String,
    /// Optional model identifier (LLM steps only).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// Tokens consumed (input, output).
    #[serde(default)]
    pub tokens_in: u64,
    #[serde(default)]
    /// Tokens produced.
    pub tokens_out: u64,
    /// Cost in USD.
    #[serde(default)]
    pub cost_usd: f64,
    /// Wall-clock duration in milliseconds.
    #[serde(default)]
    pub duration_ms: u64,
    /// Observation captured during this step.
    pub observation: Observation,
}

/// Trace document — created when a task starts and appended-to as each
/// node executes.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionTrace {
    /// Stable id.
    pub trace_id: TraceId,
    /// Originating intent.
    pub intent_id: IntentId,
    /// Started at.
    pub started_at: DateTime<Utc>,
    /// Completed at (None while running).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub completed_at: Option<DateTime<Utc>>,
    /// Total cost spent.
    #[serde(default)]
    pub total_cost_usd: f64,
    /// Status.
    pub status: TraceStatus,
    /// Recorded steps.
    #[serde(default)]
    pub steps: Vec<TraceStep>,
}

impl ExecutionTrace {
    /// Construct an empty trace.
    pub fn open(trace_id: TraceId, intent_id: IntentId) -> Self {
        Self {
            trace_id,
            intent_id,
            started_at: Utc::now(),
            completed_at: None,
            total_cost_usd: 0.0,
            status: TraceStatus::Running,
            steps: vec![],
        }
    }

    /// Append a step and update the rolling cost.
    pub fn record(&mut self, step: TraceStep) {
        self.total_cost_usd += step.cost_usd;
        self.steps.push(step);
    }

    /// Finalise the trace.
    pub fn close(&mut self, status: TraceStatus) {
        self.status = status;
        self.completed_at = Some(Utc::now());
    }
}
