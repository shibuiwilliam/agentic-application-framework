//! Contract-level errors. These are validation errors raised when a
//! caller constructs a contract that violates an invariant declared in the
//! schema.

use thiserror::Error;

/// Errors produced when validating or constructing AAF contract values.
#[derive(Debug, Error)]
pub enum ContractError {
    /// A required field was missing or empty.
    #[error("missing required field: {0}")]
    MissingField(&'static str),

    /// A field value violated a structural invariant.
    #[error("invalid field {field}: {reason}")]
    InvalidField {
        /// The offending field name.
        field: &'static str,
        /// Human-readable reason.
        reason: String,
    },

    /// `IntentEnvelope.depth` exceeded the protocol max of 5.
    #[error("delegation depth {depth} exceeds max {max}")]
    DepthExceeded {
        /// Observed depth.
        depth: u32,
        /// Maximum permitted depth (5).
        max: u32,
    },

    /// A budget value was negative or otherwise unrepresentable.
    #[error("invalid budget: {0}")]
    InvalidBudget(String),

    /// A capability declared a write/delete/send/payment side effect with
    /// no compensation handler.
    #[error("capability {0} declares side effect {1:?} but has no compensation")]
    MissingCompensation(String, crate::capability::SideEffect),
}
