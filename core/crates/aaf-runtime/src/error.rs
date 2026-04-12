//! Runtime error type.

use crate::budget::BudgetTrackerError;
use crate::graph::GraphValidationError;
use aaf_contracts::PolicyViolation;
use aaf_storage::StorageError;
use thiserror::Error;

/// Errors raised by [`crate::executor::GraphExecutor`].
#[derive(Debug, Error)]
pub enum RuntimeError {
    /// A step exceeded its timeout.
    #[error("step {step_id} timed out after {timeout_ms}ms")]
    StepTimeout {
        /// Failing step id.
        step_id: String,
        /// Timeout that was applied.
        timeout_ms: u64,
    },

    /// A budget was exhausted.
    #[error("budget exceeded: {0}")]
    BudgetExceeded(#[from] BudgetTrackerError),

    /// A policy rule denied execution.
    #[error("policy violation: {} rule(s)", .0.len())]
    PolicyViolation(Vec<PolicyViolation>),

    /// Graph structure was invalid.
    #[error("graph validation: {0}")]
    Graph(#[from] GraphValidationError),

    /// Storage backend error.
    #[error("storage: {0}")]
    Storage(#[from] StorageError),

    /// Compensation step failed mid-rollback.
    #[error("compensation failed at step {step_id}: {reason}")]
    CompensationFailed {
        /// Step that failed.
        step_id: String,
        /// Reason.
        reason: String,
    },

    /// Generic execution failure inside a node.
    #[error("node failure: {0}")]
    Node(String),

    /// The intent's requester was rejected by the revocation
    /// registry at the pre-plan hook (Wave 2 X1 Slice B, Rule 22).
    #[error("agent {did} is revoked: {reason}")]
    Revoked {
        /// Revoked DID.
        did: String,
        /// Reason text lifted from the revocation entry.
        reason: String,
    },
}
