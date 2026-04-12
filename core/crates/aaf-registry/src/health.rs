//! Service health tracking.

use aaf_contracts::CapabilityId;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Coarse health classification.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthStatus {
    /// Capability is healthy.
    Healthy,
    /// Capability is degraded but reachable.
    Degraded,
    /// Capability is unreachable.
    Unhealthy,
}

/// Per-capability health monitor.
#[derive(Default, Clone)]
pub struct HealthMonitor {
    inner: Arc<RwLock<HashMap<CapabilityId, HealthStatus>>>,
}

impl std::fmt::Debug for HealthMonitor {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let count = self.inner.read().len();
        f.debug_struct("HealthMonitor")
            .field("tracked_capabilities", &count)
            .finish()
    }
}

impl HealthMonitor {
    /// Construct an empty monitor.
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the status for a capability.
    pub fn set(&self, id: CapabilityId, status: HealthStatus) {
        self.inner.write().insert(id, status);
    }

    /// Read the status for a capability (default `Healthy`).
    pub fn get(&self, id: &CapabilityId) -> HealthStatus {
        self.inner
            .read()
            .get(id)
            .copied()
            .unwrap_or(HealthStatus::Healthy)
    }
}
