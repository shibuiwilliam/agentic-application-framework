//! Task contract — the runtime state machine for an executable unit of
//! work derived from an [`crate::intent::IntentEnvelope`].
//!
//! # Enhancement E3 — Application-Native Surface
//!
//! The state machine below also drives *proposals* produced by the
//! `aaf-surface` crate. A proposal is an [`crate::intent::IntentEnvelope`]
//! that the application has asked the agent to **propose** — the agent
//! cannot mutate state directly (Rule 20), so the task first enters
//! [`TaskState::ProposedMutation`] while a proposal is constructed, then
//! [`TaskState::AppReview`] while the application / user decides, and
//! finally transitions into [`TaskState::Accepted`],
//! [`TaskState::Rejected`], [`TaskState::Transformed`], or
//! [`TaskState::Expired`]. Accepted proposals re-enter `Running` and
//! flow through the saga engine normally.

use crate::ids::{CheckpointId, IntentId, TaskId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Task lifecycle states.
///
/// ```text
/// proposed → waiting_for_context → ready → running
///   → paused_for_approval → running
///   → failed → analyzing → recovering → (varies)
///   → completed | cancelled | compensated
///
/// proposed_mutation → app_review
///                       ├─ accepted    → running
///                       ├─ rejected    → cancelled
///                       ├─ transformed → running
///                       └─ expired     → cancelled
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskState {
    /// Task created, not yet validated.
    Proposed,
    /// Awaiting context enrichment.
    WaitingForContext,
    /// Ready to run.
    Ready,
    /// Currently running.
    Running,
    /// Paused waiting for human approval.
    PausedForApproval,
    /// Blocked on an external event.
    Blocked,
    /// Failed — analysing recovery.
    Analyzing,
    /// Recovering via partial compensation / retry / etc.
    Recovering,
    /// Completed successfully.
    Completed,
    /// Failed terminally.
    Failed,
    /// Cancelled by the requester.
    Cancelled,
    /// Compensated (rolled back).
    Compensated,
    // ── Enhancement E3: App-Native Surface ───────────────────────────
    /// A proposal-producing task has constructed a candidate
    /// [`StateMutationProposal`](crate::capability::EntityRefLite) but
    /// has not yet handed it to the application for review.
    ProposedMutation,
    /// The application / user is reviewing the proposal and may
    /// accept, reject, transform, or let it expire.
    AppReview,
    /// The proposal was accepted. The task re-enters `Running`.
    Accepted,
    /// The proposal was rejected — the task is terminal.
    Rejected,
    /// The proposal was edited by the application; the task
    /// re-enters `Running` with the mutated shape.
    Transformed,
    /// The proposal TTL elapsed without an accept/reject decision.
    Expired,
}

impl TaskState {
    /// Returns `true` if the state is terminal.
    pub fn is_terminal(self) -> bool {
        matches!(
            self,
            TaskState::Completed
                | TaskState::Failed
                | TaskState::Cancelled
                | TaskState::Compensated
                | TaskState::Rejected
                | TaskState::Expired
        )
    }

    /// Returns `true` if a state transition from `self` to `next` is legal.
    /// Used by the runtime executor, saga engine, and app-native surface.
    #[allow(clippy::match_same_arms)]
    pub fn can_transition_to(self, next: TaskState) -> bool {
        use TaskState as S;
        match (self, next) {
            (S::Proposed, S::WaitingForContext | S::Ready | S::ProposedMutation) => true,
            (S::WaitingForContext, S::Ready) => true,
            (
                S::Ready | S::Blocked | S::PausedForApproval | S::Accepted | S::Transformed,
                S::Running,
            ) => true,
            (
                S::Running,
                S::PausedForApproval | S::Blocked | S::Failed | S::Completed | S::Cancelled,
            ) => true,
            (S::PausedForApproval, S::Cancelled) => true,
            (S::Failed, S::Analyzing) => true,
            (S::Analyzing, S::Recovering) => true,
            (S::Recovering, S::Running | S::Completed | S::Compensated | S::Failed) => true,
            // Enhancement E3: App-Native Surface
            (S::ProposedMutation, S::AppReview) => true,
            (S::AppReview, S::Accepted | S::Rejected | S::Transformed | S::Expired) => true,
            (S::Rejected | S::Expired, S::Cancelled) => true,
            _ => false,
        }
    }
}

/// Runtime view of a Task.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Task {
    /// Stable id.
    pub task_id: TaskId,
    /// The originating intent.
    pub intent_id: IntentId,
    /// Current state.
    pub state: TaskState,
    /// Optional assigned agent identifier.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub assigned_agent: Option<String>,
    /// Latest checkpoint id (if any).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checkpoint_id: Option<CheckpointId>,
    /// Remaining cost budget in USD.
    pub remaining_budget_usd: f64,
    /// Remaining time budget in milliseconds.
    pub remaining_time_ms: u64,
    /// Created at.
    pub created_at: DateTime<Utc>,
    /// Last updated.
    pub updated_at: DateTime<Utc>,
}

impl Task {
    /// Construct a new task in the `Proposed` state.
    pub fn new(intent_id: IntentId, budget_usd: f64, time_ms: u64) -> Self {
        let now = Utc::now();
        Self {
            task_id: TaskId::new(),
            intent_id,
            state: TaskState::Proposed,
            assigned_agent: None,
            checkpoint_id: None,
            remaining_budget_usd: budget_usd,
            remaining_time_ms: time_ms,
            created_at: now,
            updated_at: now,
        }
    }

    /// Attempt to transition the task. Returns `false` if the transition
    /// is not allowed.
    pub fn transition(&mut self, next: TaskState) -> bool {
        if self.state.can_transition_to(next) {
            self.state = next;
            self.updated_at = Utc::now();
            true
        } else {
            false
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn legal_transitions_succeed() {
        let mut t = Task::new(IntentId::new(), 1.0, 30_000);
        assert!(t.transition(TaskState::Ready));
        assert!(t.transition(TaskState::Running));
        assert!(t.transition(TaskState::Completed));
        assert!(t.state.is_terminal());
    }

    #[test]
    fn illegal_transitions_are_blocked() {
        let mut t = Task::new(IntentId::new(), 1.0, 30_000);
        assert!(!t.transition(TaskState::Completed));
        assert_eq!(t.state, TaskState::Proposed);
    }

    #[test]
    fn proposal_lifecycle_accept() {
        let mut t = Task::new(IntentId::new(), 1.0, 30_000);
        assert!(t.transition(TaskState::ProposedMutation));
        assert!(t.transition(TaskState::AppReview));
        assert!(t.transition(TaskState::Accepted));
        assert!(t.transition(TaskState::Running));
        assert!(t.transition(TaskState::Completed));
    }

    #[test]
    fn proposal_lifecycle_reject_is_terminal() {
        let mut t = Task::new(IntentId::new(), 1.0, 30_000);
        t.transition(TaskState::ProposedMutation);
        t.transition(TaskState::AppReview);
        assert!(t.transition(TaskState::Rejected));
        assert!(t.state.is_terminal() || t.transition(TaskState::Cancelled));
    }

    #[test]
    fn proposal_lifecycle_expired_terminal() {
        let mut t = Task::new(IntentId::new(), 1.0, 30_000);
        t.transition(TaskState::ProposedMutation);
        t.transition(TaskState::AppReview);
        assert!(t.transition(TaskState::Expired));
        assert!(TaskState::Expired.is_terminal());
    }
}
