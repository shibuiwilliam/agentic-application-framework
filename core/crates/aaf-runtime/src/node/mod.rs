//! Node abstractions for the graph runtime.
//!
//! Five concrete node kinds map to the design in `PROJECT.md` §3.5:
//!
//! - `Deterministic` — pure function / API call (Rule 5)
//! - `Agent` — LLM-powered reasoning (Rule 7 guarded)
//! - `Approval` — human approval gate
//! - `Fork` — parallel fork/join
//! - `EventWait` — wait for an external event

pub mod agent;
pub mod approval;
pub mod deterministic;
pub mod event_wait;
pub mod fork;

pub use agent::{AgentNode, ToolCallRecord, ToolExecutor, DEFAULT_MAX_TOOL_CALLS};
pub use approval::ApprovalNode;
pub use deterministic::DeterministicNode;
pub use event_wait::EventWaitNode;
pub use fork::ForkNode;

use crate::error::RuntimeError;
use aaf_contracts::{IntentEnvelope, NodeId, SideEffect};
use async_trait::async_trait;
use std::collections::HashMap;

/// What a node returns.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeOutput {
    /// Structured output payload.
    pub data: serde_json::Value,
    /// Tokens consumed.
    pub tokens: u64,
    /// Cost in USD.
    pub cost_usd: f64,
    /// Wall-clock duration in ms.
    pub duration_ms: u64,
    /// Optional model used (for trace).
    pub model: Option<String>,
}

/// Discriminator surfaced via [`Node::kind`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    /// Deterministic / pure node.
    Deterministic,
    /// Agent / LLM node.
    Agent,
    /// Human approval gate.
    Approval,
    /// Parallel fork / join.
    Fork,
    /// External event wait.
    EventWait,
}

/// One node in the graph.
#[async_trait]
pub trait Node: Send + Sync {
    /// Stable id.
    fn id(&self) -> &NodeId;
    /// Discriminator.
    fn kind(&self) -> NodeKind;
    /// Side effect classification — used by the policy engine.
    fn side_effect(&self) -> SideEffect {
        SideEffect::None
    }
    /// Run the node and return its output.
    async fn run(
        &self,
        intent: &IntentEnvelope,
        prior_outputs: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError>;
}
