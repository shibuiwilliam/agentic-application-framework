//! Handoff contract — captured every time one agent delegates work to
//! another. Carries the bounded-autonomy context (Rule 8) and the
//! min-trust effective level (Rule trust min propagation).

use crate::ids::{AgentId, ArtifactId, HandoffId, TaskId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Delegation contract between two agents.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Handoff {
    /// Stable id.
    pub handoff_id: HandoffId,
    /// Delegator agent.
    pub from_agent: AgentId,
    /// Delegatee agent.
    pub to_agent: AgentId,
    /// Task being handed off.
    pub task_id: TaskId,
    /// Free-text question / instruction.
    pub question: String,
    /// Constraints the delegatee MUST honour.
    #[serde(default)]
    pub constraints: Vec<String>,
    /// Artifacts the delegatee may consume.
    #[serde(default)]
    pub available_data: Vec<ArtifactId>,
    /// Operations the delegatee is forbidden from performing.
    #[serde(default)]
    pub prohibited: Vec<String>,
    /// Expected artifact type produced by the delegatee.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_artifact: Option<String>,
    /// Optional deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DateTime<Utc>>,
    /// Effective trust level for this handoff: `min(delegator, delegatee)`.
    /// Wave 1 behavioural-trust dimension. Wave 2 X1 adds a
    /// cryptographic dimension on top via `capability_token`.
    pub effective_trust_level: u8,
    /// Created at.
    pub created_at: DateTime<Utc>,

    // ── Enhancement X1: Agent Identity (Wave 2) ────────────────────────
    /// Optional signed capability token proving the delegator
    /// authorised this specific handoff. Enforced by
    /// `aaf-trust::delegation::require` in Slice B.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_token: Option<crate::identity::CapabilityTokenLite>,
}
