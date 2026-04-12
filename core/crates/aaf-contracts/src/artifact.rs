//! Artifact contract — every output produced by an agent / capability is
//! materialised as an Artifact with full provenance.

use crate::ids::{AgentId, ArtifactId, CapabilityId, IntentId, TaskId, TraceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Provenance record for an [`Artifact`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArtifactProvenance {
    /// Originating intent.
    pub intent_id: IntentId,
    /// Originating task.
    pub task_id: TaskId,
    /// Trace id this artifact was produced under.
    pub trace_id: TraceId,
    /// Producing agent.
    pub producing_agent: AgentId,
    /// Capability that produced the artifact.
    pub capability: CapabilityId,
    /// Source data references.
    #[serde(default)]
    pub data_sources: Vec<String>,
    /// LLM model used (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model_used: Option<String>,
}

/// Generic artifact produced by a capability invocation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Artifact {
    /// Stable id.
    pub artifact_id: ArtifactId,
    /// Logical artifact type (e.g. `sales_report`).
    pub artifact_type: String,
    /// Content body — typically structured JSON.
    pub content: serde_json::Value,
    /// Optional rendered representation (e.g. `markdown`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rendered: Option<String>,
    /// Provenance record.
    pub provenance: ArtifactProvenance,
    /// Confidence in [0,1].
    #[serde(default)]
    pub confidence: f64,
    /// Policy tags (e.g. `internal`, `no-pii`).
    #[serde(default)]
    pub policy_tags: Vec<String>,
    /// SHA-256 content hash, populated by signing.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub content_hash: Option<String>,
    /// Detached signature (base64) populated by [`crate::trust`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub signature: Option<String>,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Optional expiry timestamp.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Schema version of this artifact.
    #[serde(default = "one")]
    pub version: u32,

    // ── Enhancement E2: Domain Ontology Layer ──────────────────────────
    /// Entities this artifact was derived from. Each entry is a
    /// versioned entity reference so provenance survives entity schema
    /// evolution.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub derived_from: Vec<crate::capability::EntityRefLite>,
}

fn one() -> u32 {
    1
}

impl Artifact {
    /// Construct a new artifact with sensible defaults.
    pub fn new(
        artifact_type: impl Into<String>,
        content: serde_json::Value,
        provenance: ArtifactProvenance,
    ) -> Self {
        Self {
            artifact_id: ArtifactId::new(),
            artifact_type: artifact_type.into(),
            content,
            rendered: None,
            provenance,
            confidence: 0.0,
            policy_tags: vec![],
            content_hash: None,
            signature: None,
            created_at: Utc::now(),
            expires_at: None,
            version: 1,
            derived_from: vec![],
        }
    }
}
