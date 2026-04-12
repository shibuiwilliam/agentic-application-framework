//! Storage layer error type.

use thiserror::Error;

/// Errors raised by storage backends.
#[derive(Debug, Error)]
pub enum StorageError {
    /// Requested key/id was not found.
    #[error("not found: {0}")]
    NotFound(String),

    /// Underlying backend failure (network, disk, etc.).
    #[error("backend failure: {0}")]
    Backend(String),

    /// A precondition / unique constraint was violated.
    #[error("conflict: {0}")]
    Conflict(String),

    /// Tenant boundary violation — caller asked for data outside its
    /// tenant scope.
    #[error("tenant boundary violation: {0}")]
    BoundaryViolation(String),
}
