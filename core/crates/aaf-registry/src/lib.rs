//! Capability Registry.
//!
//! - CRUD on capability records
//! - Semantic discovery (lexical similarity over name + description + tags)
//! - Health tracking
//! - Per-capability degradation state machine
//! - Version management
//!
//! Enforces **Rule 9** at registration time: a capability declaring a
//! write/delete/send/payment side effect with no compensation handler is
//! rejected.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod a2a;
pub mod circuit_breaker;
pub mod degradation;
pub mod discovery;
pub mod error;
pub mod health;
pub mod prelude;
pub mod registration;
pub mod store;
pub mod version;

pub use circuit_breaker::{
    BreakerConfig, BreakerLevel, BreakerSnapshot, BreakerState, CircuitBreakerRegistry,
};
pub use degradation::{DegradationStateMachine, DegradationTransition};
pub use discovery::{DiscoveryQuery, DiscoveryResult, EntityQueryKind};
pub use error::RegistryError;
pub use health::{HealthMonitor, HealthStatus};
pub use registration::{RegistrationError, RegistrationPipeline, RegistrationResult};
pub use store::Registry;
