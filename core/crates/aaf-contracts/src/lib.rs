//! AAF contract surface.
//!
//! Strongly-typed Rust representations of every cross-component message in
//! the Agentic Application Framework. These types are deliberately
//! data-only — they own no I/O, no async, and no business logic — so they
//! can be reused by every crate (runtime, planner, sidecar, etc.) without
//! introducing dependency cycles.
//!
//! In a later iteration these shapes will be generated from
//! `spec/proto/aaf/v1/*.proto` via `buf`. The hand-written types in this
//! crate define the *current* contract surface and are kept aligned with
//! the JSON Schemas under `spec/schemas/`.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod artifact;
pub mod capability;
pub mod error;
pub mod handoff;
pub mod identity;
pub mod ids;
pub mod intent;
pub mod learn;
pub mod observation;
pub mod policy;
pub mod task;
pub mod tool;
pub mod trace;
pub mod trust;

pub use artifact::*;
pub use capability::*;
pub use error::*;
pub use handoff::*;
pub use identity::*;
pub use ids::*;
pub use intent::*;
pub use observation::*;
pub use policy::*;
pub use task::*;
pub use tool::*;
pub use trace::*;
pub use trust::*;

/// Convenience prelude bringing the most-used contract types into scope.
pub mod prelude {
    pub use crate::artifact::{Artifact, ArtifactProvenance};
    pub use crate::capability::{
        CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilitySla, DataClassification,
        DegradationLevel, DegradationSpec, EndpointKind, EntityRefLite, EntityScopeLite,
        EntityVersionLite, EventRefLite, SideEffect,
    };
    pub use crate::error::ContractError;
    pub use crate::handoff::Handoff;
    pub use crate::identity::{
        AgentDidRef, AttestationLevelRef, CapabilityTokenLite, TokenClaimsLite,
    };
    pub use crate::ids::{ArtifactId, CapabilityId, IntentId, NodeId, TaskId, TraceId};
    pub use crate::intent::{BudgetContract, IntentEnvelope, IntentType, Requester, RiskTier};
    pub use crate::observation::{
        DownstreamError, FeedbackRating, Observation, Outcome, OutcomeStatus, PolicyViolationRef,
        SemanticScore, UserFeedback,
    };
    pub use crate::policy::{PolicyDecision, PolicySeverity, PolicyViolation, RuleKind};
    pub use crate::task::{Task, TaskState};
    pub use crate::tool::{StopReason, ToolChoice, ToolDefinition, ToolResultBlock, ToolUseBlock};
    pub use crate::trace::{ExecutionTrace, TraceStep};
    pub use crate::trust::{AutonomyLevel, TrustScore};
}
