//! Artifact store trait + in-memory backend.

use crate::error::StorageError;
use aaf_contracts::{Artifact, ArtifactId};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Persistence boundary for produced [`Artifact`]s.
#[async_trait]
pub trait ArtifactStore: Send + Sync {
    /// Persist an artifact.
    async fn put(&self, artifact: Artifact) -> Result<(), StorageError>;

    /// Fetch by id.
    async fn get(&self, id: &ArtifactId) -> Result<Artifact, StorageError>;

    /// List artifacts (test/dev only — production backends paginate).
    async fn list(&self) -> Result<Vec<Artifact>, StorageError>;
}

/// In-memory implementation of [`ArtifactStore`].
#[derive(Default)]
pub struct InMemoryArtifactStore {
    inner: Arc<RwLock<HashMap<ArtifactId, Artifact>>>,
}

impl InMemoryArtifactStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ArtifactStore for InMemoryArtifactStore {
    async fn put(&self, artifact: Artifact) -> Result<(), StorageError> {
        self.inner
            .write()
            .insert(artifact.artifact_id.clone(), artifact);
        Ok(())
    }

    async fn get(&self, id: &ArtifactId) -> Result<Artifact, StorageError> {
        self.inner
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }

    async fn list(&self) -> Result<Vec<Artifact>, StorageError> {
        Ok(self.inner.read().values().cloned().collect())
    }
}
