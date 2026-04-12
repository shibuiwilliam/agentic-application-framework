//! Sidecar health flag — drives Rule 13 transparent fallback.

use parking_lot::RwLock;
use std::sync::Arc;

/// Mutable sidecar health.
#[derive(Default, Clone)]
pub struct SidecarHealth {
    inner: Arc<RwLock<bool>>,
}

impl SidecarHealth {
    /// Construct in the healthy state.
    pub fn healthy() -> Self {
        Self {
            inner: Arc::new(RwLock::new(true)),
        }
    }

    /// Mark unhealthy.
    pub fn mark_unhealthy(&self) {
        *self.inner.write() = false;
    }

    /// Mark healthy.
    pub fn mark_healthy(&self) {
        *self.inner.write() = true;
    }

    /// Read.
    pub fn is_healthy(&self) -> bool {
        *self.inner.read()
    }
}
