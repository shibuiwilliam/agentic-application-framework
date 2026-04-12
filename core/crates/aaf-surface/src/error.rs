//! Surface errors.

use thiserror::Error;

/// Errors raised by the app-native surface.
#[derive(Debug, Error)]
pub enum SurfaceError {
    /// Attempted to construct an [`crate::proposal::ActionProposal`]
    /// whose `mutations[]` is non-empty without a `compensation_ref`.
    /// Violates Rule 20 (Proposals, Not Mutations).
    #[error("proposal has {count} mutation(s) but no compensation_ref (Rule 20)")]
    MissingCompensation {
        /// Number of mutations declared.
        count: usize,
    },

    /// Attempted to expose a field that the projection did not
    /// explicitly list. Violates Rule 19 (Projections Default-Deny).
    #[error("projection `{projection}` denies field `{field}`")]
    ProjectionDenied {
        /// Projection id.
        projection: String,
        /// Requested field.
        field: String,
    },

    /// The situation packager could not fit the required context
    /// within the configured token budget.
    #[error("situation exceeds context budget ({used} > {limit} tokens)")]
    ContextBudgetExceeded {
        /// Estimated tokens used.
        used: usize,
        /// Configured limit.
        limit: usize,
    },

    /// Invalid state transition on the proposal lifecycle.
    #[error("illegal proposal transition: {from:?} → {to:?}")]
    IllegalTransition {
        /// From state.
        from: String,
        /// To state.
        to: String,
    },
}
