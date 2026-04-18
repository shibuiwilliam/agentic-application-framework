//! Errors raised by the registry.

use aaf_contracts::{AttestationLevelRef, ContractError};
use aaf_storage::StorageError;
use thiserror::Error;

/// Errors raised by [`crate::Registry`].
#[derive(Debug, Error)]
pub enum RegistryError {
    /// Underlying storage failure.
    #[error("storage: {0}")]
    Storage(#[from] StorageError),

    /// Capability validation failed (covers Rule 9).
    #[error("invalid capability: {0}")]
    Invalid(#[from] ContractError),

    /// No capability found matching the discovery query.
    #[error("no capability matched the query: {0}")]
    NoMatch(String),

    /// The caller's attestation level was insufficient to serve the
    /// requested capability (Wave 2 X1 Slice B, Rule 23).
    #[error(
        "insufficient attestation: capability requires {required:?} but caller presented {presented:?}"
    )]
    InsufficientAttestation {
        /// Level declared on the capability.
        required: AttestationLevelRef,
        /// Level the caller actually presented.
        presented: AttestationLevelRef,
    },

    /// Reputation update was rate-limited (E1 Slice B).
    #[error("reputation update rate-limited for capability {0}")]
    RateLimited(String),

    /// No capability found with the given name or id.
    #[error("capability not found: {0}")]
    NotFound(String),
}
