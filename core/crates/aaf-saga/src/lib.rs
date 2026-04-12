//! Agentic Saga engine.
//!
//! Extends a traditional Saga with **intelligent recovery**: instead of
//! always rolling back on failure, the engine analyses the cause and
//! picks one of:
//!
//! - `pause_and_ask_user`
//! - `retry_with_alternative`
//! - `partial_compensation`
//! - `full_compensation`
//! - `skip`

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod bridge;
pub mod definition;
pub mod executor;
pub mod recovery;
pub mod state;

pub use bridge::{BridgeError, CapabilityInvoker, RegistryBridge};
pub use definition::{
    CompensationType, OnFailure, RecoveryAction, RecoveryRule, SagaDefinition, SagaStep, StepKind,
};
pub use executor::{SagaExecutor, SagaOutcome, StepResult};
pub use recovery::RecoveryDecision;
pub use state::SagaState;
