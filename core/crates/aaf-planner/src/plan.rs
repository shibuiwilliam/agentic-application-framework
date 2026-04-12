//! Execution plan shape.

use aaf_contracts::{CapabilityId, IntentId, NodeId};
use serde::{Deserialize, Serialize};

/// Logical step kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlannedStepKind {
    /// Pure / API call.
    Deterministic,
    /// LLM-driven agent.
    Agent,
    /// Human approval gate.
    Approval,
    /// Parallel fork.
    Fork,
}

/// One step in an [`ExecutionPlan`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlannedStep {
    /// Position (1-based).
    pub step_id: u32,
    /// Step kind.
    pub kind: PlannedStepKind,
    /// Capability id to invoke.
    pub capability: CapabilityId,
    /// Logical input mapping description.
    pub input_mapping: String,
    /// Logical output id.
    pub output_id: NodeId,
}

/// A planner output.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionPlan {
    /// Originating intent.
    pub intent_id: IntentId,
    /// Steps in execution order.
    pub steps: Vec<PlannedStep>,
}

impl ExecutionPlan {
    /// Number of steps.
    pub fn len(&self) -> usize {
        self.steps.len()
    }

    /// Whether the plan is empty.
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }
}
