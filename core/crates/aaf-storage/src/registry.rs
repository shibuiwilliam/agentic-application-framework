//! Capability registry storage trait + in-memory backend.

use crate::error::StorageError;
use aaf_contracts::{CapabilityContract, CapabilityId};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Persistence boundary for the capability registry.
#[async_trait]
pub trait RegistryStore: Send + Sync {
    /// Insert or replace a capability.
    async fn upsert(&self, cap: CapabilityContract) -> Result<(), StorageError>;

    /// Fetch by id.
    async fn get(&self, id: &CapabilityId) -> Result<CapabilityContract, StorageError>;

    /// Delete by id.
    async fn delete(&self, id: &CapabilityId) -> Result<(), StorageError>;

    /// List every capability (test/dev only).
    async fn list(&self) -> Result<Vec<CapabilityContract>, StorageError>;
}

/// In-memory implementation of [`RegistryStore`].
#[derive(Default)]
pub struct InMemoryRegistryStore {
    inner: Arc<RwLock<HashMap<CapabilityId, CapabilityContract>>>,
}

impl InMemoryRegistryStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RegistryStore for InMemoryRegistryStore {
    async fn upsert(&self, cap: CapabilityContract) -> Result<(), StorageError> {
        self.inner.write().insert(cap.id.clone(), cap);
        Ok(())
    }

    async fn get(&self, id: &CapabilityId) -> Result<CapabilityContract, StorageError> {
        self.inner
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }

    async fn delete(&self, id: &CapabilityId) -> Result<(), StorageError> {
        self.inner.write().remove(id);
        Ok(())
    }

    async fn list(&self) -> Result<Vec<CapabilityContract>, StorageError> {
        Ok(self.inner.read().values().cloned().collect())
    }
}
