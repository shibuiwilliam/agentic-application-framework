//! Action proposals — Rule 20 (Proposals, Not Mutations).

use crate::error::SurfaceError;
use aaf_contracts::{
    ArtifactId, CapabilityId, EntityRefLite, IntentId, ProposalId, TenantId, TraceId,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Typed reference to a compensating capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationRef {
    /// Capability id that undoes the proposed mutation.
    pub capability: CapabilityId,
}

/// How the UI should render a proposal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UiKind {
    /// Diff view: show old vs new.
    Diff,
    /// Modal form the user fills in.
    Form,
    /// Compact card inline with the surface.
    Card,
    /// Non-blocking banner.
    Banner,
}

/// UI rendering hints for the application.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiHints {
    /// Preferred rendering kind.
    pub kind: UiKind,
    /// Priority (0 = lowest, 100 = highest).
    pub priority: u8,
    /// Whether the user can dismiss the proposal without acting.
    pub dismissable: bool,
}

impl Default for UiHints {
    fn default() -> Self {
        Self {
            kind: UiKind::Card,
            priority: 50,
            dismissable: true,
        }
    }
}

/// One proposed state change.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StateMutationProposal {
    /// Target entity.
    pub entity_ref: EntityRefLite,
    /// Dot-delimited field path on the entity.
    pub field_path: String,
    /// Previous value (for diff / preview).
    pub from_value: serde_json::Value,
    /// Proposed value.
    pub to_value: serde_json::Value,
    /// Optional hint for a preview renderer.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preview_renderer_hint: Option<String>,
    /// Whether this mutation is reversible.
    pub reversible: bool,
    /// Reference to the compensating capability — required for every
    /// mutation (Rule 20 + Rule 9).
    pub compensation_ref: CompensationRef,
}

/// Proposal lifecycle state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalState {
    /// Being constructed by an agent.
    Draft,
    /// Ready to hand to the application.
    Proposed,
    /// In front of the user / app.
    AppReview,
    /// User accepted.
    Accepted,
    /// User rejected.
    Rejected,
    /// User edited the proposal.
    Transformed,
    /// TTL expired.
    Expired,
}

/// Action proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ActionProposal {
    /// Stable id.
    pub proposal_id: ProposalId,
    /// Originating intent.
    pub intent_id: IntentId,
    /// Trace id (already linked to `aaf-trace`).
    pub trace_id: TraceId,
    /// Tenant scope.
    pub tenant_id: TenantId,
    /// One-line summary rendered in the UI.
    pub summary: String,
    /// Free-text rationale shown on demand.
    pub rationale: String,
    /// Proposed mutations.
    pub mutations: Vec<StateMutationProposal>,
    /// Artifacts attached to the proposal (reports, previews, ...).
    #[serde(default)]
    pub artifacts: Vec<ArtifactId>,
    /// Rendering hints.
    pub ui_hints: UiHints,
    /// Optional top-level compensation ref — required whenever
    /// `mutations[]` is non-empty. Rule 20 is enforced at
    /// construction time by [`ActionProposal::build`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensation_ref: Option<CompensationRef>,
    /// Expiry timestamp.
    pub expires_at: DateTime<Utc>,
    /// Current approval state.
    pub approval_state: ApprovalState,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

impl ActionProposal {
    /// Construct a proposal, enforcing Rule 20.
    ///
    /// If `mutations` is empty, `compensation_ref` may be `None`.
    /// Otherwise it **must** be `Some(_)` — otherwise the constructor
    /// returns [`SurfaceError::MissingCompensation`].
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        intent_id: IntentId,
        trace_id: TraceId,
        tenant_id: TenantId,
        summary: impl Into<String>,
        rationale: impl Into<String>,
        mutations: Vec<StateMutationProposal>,
        artifacts: Vec<ArtifactId>,
        ui_hints: UiHints,
        compensation_ref: Option<CompensationRef>,
        expires_at: DateTime<Utc>,
    ) -> Result<Self, SurfaceError> {
        if !mutations.is_empty() && compensation_ref.is_none() {
            return Err(SurfaceError::MissingCompensation {
                count: mutations.len(),
            });
        }
        Ok(Self {
            proposal_id: ProposalId::new(),
            intent_id,
            trace_id,
            tenant_id,
            summary: summary.into(),
            rationale: rationale.into(),
            mutations,
            artifacts,
            ui_hints,
            compensation_ref,
            expires_at,
            approval_state: ApprovalState::Draft,
            created_at: Utc::now(),
        })
    }

    /// Advance the approval state, returning an error on illegal
    /// transitions.
    pub fn transition_to(&mut self, next: ApprovalState) -> Result<(), SurfaceError> {
        use ApprovalState::{Accepted, AppReview, Draft, Expired, Proposed, Rejected, Transformed};
        let ok = matches!(
            (self.approval_state, next),
            (Draft, Proposed)
                | (Proposed, AppReview)
                | (AppReview, Accepted | Rejected | Transformed | Expired)
        );
        if !ok {
            return Err(SurfaceError::IllegalTransition {
                from: format!("{:?}", self.approval_state),
                to: format!("{:?}", next),
            });
        }
        self.approval_state = next;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{IntentId, TenantId, TraceId};
    use chrono::Duration;

    fn mutation() -> StateMutationProposal {
        StateMutationProposal {
            entity_ref: EntityRefLite::new("commerce.Order"),
            field_path: "status".into(),
            from_value: serde_json::json!("pending"),
            to_value: serde_json::json!("cancelled"),
            preview_renderer_hint: Some("diff".into()),
            reversible: true,
            compensation_ref: CompensationRef {
                capability: CapabilityId::from("cap-order-reopen"),
            },
        }
    }

    #[test]
    fn rule_20_build_rejects_mutations_without_compensation() {
        let err = ActionProposal::build(
            IntentId::new(),
            TraceId::new(),
            TenantId::from("t-a"),
            "cancel order",
            "because user asked",
            vec![mutation()],
            vec![],
            UiHints::default(),
            None,
            Utc::now() + Duration::minutes(5),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            SurfaceError::MissingCompensation { count: 1 }
        ));
    }

    #[test]
    fn rule_20_build_accepts_mutations_with_compensation() {
        let p = ActionProposal::build(
            IntentId::new(),
            TraceId::new(),
            TenantId::from("t-a"),
            "cancel order",
            "because user asked",
            vec![mutation()],
            vec![],
            UiHints::default(),
            Some(CompensationRef {
                capability: CapabilityId::from("cap-order-reopen"),
            }),
            Utc::now() + Duration::minutes(5),
        )
        .unwrap();
        assert_eq!(p.approval_state, ApprovalState::Draft);
    }

    #[test]
    fn empty_mutations_need_no_compensation() {
        let p = ActionProposal::build(
            IntentId::new(),
            TraceId::new(),
            TenantId::from("t-a"),
            "hint",
            "fyi",
            vec![],
            vec![],
            UiHints::default(),
            None,
            Utc::now() + Duration::minutes(5),
        )
        .unwrap();
        assert_eq!(p.mutations.len(), 0);
    }

    #[test]
    fn legal_transitions_follow_state_machine() {
        let mut p = ActionProposal::build(
            IntentId::new(),
            TraceId::new(),
            TenantId::from("t-a"),
            "x",
            "x",
            vec![],
            vec![],
            UiHints::default(),
            None,
            Utc::now() + Duration::minutes(5),
        )
        .unwrap();
        p.transition_to(ApprovalState::Proposed).unwrap();
        p.transition_to(ApprovalState::AppReview).unwrap();
        p.transition_to(ApprovalState::Accepted).unwrap();
    }

    #[test]
    fn illegal_transition_errors() {
        let mut p = ActionProposal::build(
            IntentId::new(),
            TraceId::new(),
            TenantId::from("t-a"),
            "x",
            "x",
            vec![],
            vec![],
            UiHints::default(),
            None,
            Utc::now() + Duration::minutes(5),
        )
        .unwrap();
        let err = p.transition_to(ApprovalState::Accepted).unwrap_err();
        assert!(matches!(err, SurfaceError::IllegalTransition { .. }));
    }
}
