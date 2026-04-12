//! Convenience prelude — `use aaf_registry::prelude::*;`

pub use crate::circuit_breaker::{
    BreakerConfig, BreakerLevel, BreakerSnapshot, BreakerState, CircuitBreakerRegistry,
};
pub use crate::degradation::{DegradationStateMachine, DegradationTransition};
pub use crate::discovery::{DiscoveryQuery, DiscoveryResult};
pub use crate::error::RegistryError;
pub use crate::health::{HealthMonitor, HealthStatus};
pub use crate::registration::{RegistrationError, RegistrationPipeline, RegistrationResult};
pub use crate::store::Registry;
