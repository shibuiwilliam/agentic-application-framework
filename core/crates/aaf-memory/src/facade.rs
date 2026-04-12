//! Memory facade — single entry point that fans out to the four
//! storage backends.

use aaf_contracts::{Artifact, ArtifactId, EntityRefLite, TaskId, TenantId};
use aaf_storage::{
    ArtifactStore, LongTermMemoryStore, StorageError, ThreadId, ThreadMemoryStore,
    WorkingMemoryStore,
};
use std::sync::Arc;

/// Aggregated handle that hides the four memory backends.
pub struct MemoryFacade {
    /// Working memory backend.
    pub working: Arc<dyn WorkingMemoryStore>,
    /// Thread memory backend.
    pub thread: Arc<dyn ThreadMemoryStore>,
    /// Long-term memory backend.
    pub longterm: Arc<dyn LongTermMemoryStore>,
    /// Artifact store backend.
    pub artifacts: Arc<dyn ArtifactStore>,
}

impl MemoryFacade {
    /// Construct a facade with explicit backends.
    pub fn new(
        working: Arc<dyn WorkingMemoryStore>,
        thread: Arc<dyn ThreadMemoryStore>,
        longterm: Arc<dyn LongTermMemoryStore>,
        artifacts: Arc<dyn ArtifactStore>,
    ) -> Self {
        Self {
            working,
            thread,
            longterm,
            artifacts,
        }
    }

    /// Build an in-memory facade for tests / dev.
    pub fn in_memory() -> Self {
        Self::new(
            Arc::new(aaf_storage::InMemoryWorkingStore::new()),
            Arc::new(aaf_storage::InMemoryThreadStore::new()),
            Arc::new(aaf_storage::InMemoryLongTermStore::new()),
            Arc::new(aaf_storage::InMemoryArtifactStore::new()),
        )
    }

    /// Get a working-memory entry.
    pub async fn working_get(
        &self,
        task: &TaskId,
        key: &str,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        self.working.get(task, key).await
    }

    /// Put a working-memory entry.
    pub async fn working_put(
        &self,
        task: &TaskId,
        key: &str,
        v: serde_json::Value,
    ) -> Result<(), StorageError> {
        self.working.put(task, key, v).await
    }

    /// Clear all working-memory entries for a completed task.
    /// Should be called when a task reaches a terminal state to
    /// prevent stale data accumulation in long-running servers.
    pub async fn working_clear(&self, task: &TaskId) -> Result<(), StorageError> {
        self.working.drop_task(task).await
    }

    /// Append to a thread.
    pub async fn thread_append(
        &self,
        thread: &ThreadId,
        v: serde_json::Value,
    ) -> Result<(), StorageError> {
        self.thread.append(thread, v).await
    }

    /// Search long-term memory.
    pub async fn longterm_search(
        &self,
        tenant: &TenantId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<aaf_storage::memory::LongTermRecord>, StorageError> {
        self.longterm.search(tenant, query, limit).await
    }

    /// Retrieve long-term memory records indexed under a specific
    /// ontology entity (E2 Slice B, Rule 14).
    ///
    /// Tenant-scoped: records belonging to another tenant are never
    /// returned. See [`aaf_storage::LongTermMemoryStore::search_by_entity`]
    /// for the backend contract.
    pub async fn longterm_search_by_entity(
        &self,
        tenant: &TenantId,
        entity: &EntityRefLite,
        limit: usize,
    ) -> Result<Vec<aaf_storage::memory::LongTermRecord>, StorageError> {
        self.longterm.search_by_entity(tenant, entity, limit).await
    }

    /// Insert a long-term memory record (including any declared entity
    /// refs, which will populate the backend's entity index).
    pub async fn longterm_insert(
        &self,
        record: aaf_storage::memory::LongTermRecord,
    ) -> Result<(), StorageError> {
        self.longterm.insert(record).await
    }

    /// Persist an artifact.
    pub async fn artifact_put(&self, artifact: Artifact) -> Result<(), StorageError> {
        self.artifacts.put(artifact).await
    }

    /// Fetch an artifact.
    pub async fn artifact_get(&self, id: &ArtifactId) -> Result<Artifact, StorageError> {
        self.artifacts.get(id).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_storage::memory::LongTermRecord;

    #[tokio::test]
    async fn facade_routes_calls_to_working_store() {
        let m = MemoryFacade::in_memory();
        let task = TaskId::new();
        m.working_put(&task, "k", serde_json::json!("v"))
            .await
            .unwrap();
        let v = m.working_get(&task, "k").await.unwrap();
        assert_eq!(v, Some(serde_json::json!("v")));
    }

    #[tokio::test]
    async fn facade_entity_keyed_retrieval_round_trip() {
        let m = MemoryFacade::in_memory();
        let tenant = TenantId::new();
        let order = EntityRefLite::new("commerce.Order");
        m.longterm_insert(LongTermRecord {
            tenant: tenant.clone(),
            kind: "episodic".into(),
            content: "order timeline".into(),
            payload: serde_json::json!({}),
            entity_refs: vec![order.clone()],
        })
        .await
        .unwrap();

        let hits = m
            .longterm_search_by_entity(&tenant, &order, 5)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_refs[0].entity_id, "commerce.Order");
    }
}
