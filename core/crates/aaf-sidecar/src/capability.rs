//! Capability publisher: registers a service's capabilities into the
//! AAF registry on startup.

use aaf_contracts::CapabilityContract;
use aaf_registry::{Registry, RegistryError};
use std::sync::Arc;

/// Capability publisher.
pub struct CapabilityPublisher {
    registry: Arc<Registry>,
}

impl CapabilityPublisher {
    /// Construct.
    pub fn new(registry: Arc<Registry>) -> Self {
        Self { registry }
    }

    /// Publish a capability list. Validation (Rule 9) happens inside
    /// the registry.
    pub async fn publish(
        &self,
        caps: impl IntoIterator<Item = CapabilityContract>,
    ) -> Result<(), RegistryError> {
        for c in caps {
            self.registry.register(c).await?;
        }
        Ok(())
    }
}
