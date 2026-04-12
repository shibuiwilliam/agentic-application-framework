//! Proposal lifecycle helper.
//!
//! Wraps an [`crate::proposal::ActionProposal`] with the thin
//! state-transition API the runtime and server binary use when
//! shepherding proposals through app-review. Slice B will connect
//! this to `aaf-trace` so every transition produces an Observation.

use crate::error::SurfaceError;
use crate::proposal::{ActionProposal, ApprovalState};

/// Public facade around proposal lifecycle transitions. Stateless —
/// holds no data of its own; callers pass the proposal in.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProposalLifecycle;

impl ProposalLifecycle {
    /// Move a proposal from `Draft` → `Proposed` → `AppReview`.
    pub fn publish(&self, p: &mut ActionProposal) -> Result<(), SurfaceError> {
        p.transition_to(ApprovalState::Proposed)?;
        p.transition_to(ApprovalState::AppReview)?;
        Ok(())
    }

    /// Accept a proposal that is in `AppReview`.
    pub fn accept(&self, p: &mut ActionProposal) -> Result<(), SurfaceError> {
        p.transition_to(ApprovalState::Accepted)
    }

    /// Reject a proposal that is in `AppReview`.
    pub fn reject(&self, p: &mut ActionProposal) -> Result<(), SurfaceError> {
        p.transition_to(ApprovalState::Rejected)
    }

    /// Mark a proposal as transformed.
    pub fn transform(&self, p: &mut ActionProposal) -> Result<(), SurfaceError> {
        p.transition_to(ApprovalState::Transformed)
    }

    /// Mark a proposal as expired.
    pub fn expire(&self, p: &mut ActionProposal) -> Result<(), SurfaceError> {
        p.transition_to(ApprovalState::Expired)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::proposal::UiHints;
    use aaf_contracts::{IntentId, TenantId, TraceId};
    use chrono::{Duration, Utc};

    fn proposal() -> ActionProposal {
        ActionProposal::build(
            IntentId::new(),
            TraceId::new(),
            TenantId::from("t-a"),
            "",
            "",
            vec![],
            vec![],
            UiHints::default(),
            None,
            Utc::now() + Duration::minutes(5),
        )
        .unwrap()
    }

    #[test]
    fn publish_then_accept_round_trip() {
        let lc = ProposalLifecycle;
        let mut p = proposal();
        lc.publish(&mut p).unwrap();
        assert_eq!(p.approval_state, ApprovalState::AppReview);
        lc.accept(&mut p).unwrap();
        assert_eq!(p.approval_state, ApprovalState::Accepted);
    }

    #[test]
    fn reject_from_review() {
        let lc = ProposalLifecycle;
        let mut p = proposal();
        lc.publish(&mut p).unwrap();
        lc.reject(&mut p).unwrap();
        assert_eq!(p.approval_state, ApprovalState::Rejected);
    }

    #[test]
    fn accept_before_publish_is_illegal() {
        let lc = ProposalLifecycle;
        let mut p = proposal();
        let err = lc.accept(&mut p).unwrap_err();
        assert!(matches!(err, SurfaceError::IllegalTransition { .. }));
    }
}
