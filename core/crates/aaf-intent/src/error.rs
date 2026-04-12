//! Intent compiler errors.

use aaf_contracts::ContractError;
use thiserror::Error;

/// Errors raised by [`crate::compiler::IntentCompiler`].
#[derive(Debug, Error)]
pub enum IntentError {
    /// The classifier could not assign a type.
    #[error("classification failed for input")]
    ClassificationFailed,

    /// One or more required fields are missing — caller must refine.
    #[error("incomplete intent: missing {0:?}")]
    Incomplete(Vec<&'static str>),

    /// Underlying contract validation failure.
    #[error("contract: {0}")]
    Contract(#[from] ContractError),
}
