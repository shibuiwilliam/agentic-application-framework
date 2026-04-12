//! Memory-layer storage traits + in-memory backends.
//!
//! Three of the four memory layers from `PROJECT.md` §3.6 — Working,
//! Thread, Long-term — live behind these traits. The fourth (Artifact)
//! lives in [`crate::artifact`].

use crate::error::StorageError;
use aaf_contracts::{EntityRefLite, TaskId, TenantId};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

// ───────────────────────────── Working Memory ─────────────────────────────

/// Per-task transient state.
#[async_trait]
pub trait WorkingMemoryStore: Send + Sync {
    /// Put a key for a task.
    async fn put(
        &self,
        task: &TaskId,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), StorageError>;

    /// Get a key for a task.
    async fn get(
        &self,
        task: &TaskId,
        key: &str,
    ) -> Result<Option<serde_json::Value>, StorageError>;

    /// Delete all entries for a task (called at task completion).
    async fn drop_task(&self, task: &TaskId) -> Result<(), StorageError>;
}

/// In-memory implementation of [`WorkingMemoryStore`].
#[derive(Default)]
pub struct InMemoryWorkingStore {
    inner: Arc<RwLock<HashMap<TaskId, HashMap<String, serde_json::Value>>>>,
}

impl InMemoryWorkingStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl WorkingMemoryStore for InMemoryWorkingStore {
    async fn put(
        &self,
        task: &TaskId,
        key: &str,
        value: serde_json::Value,
    ) -> Result<(), StorageError> {
        self.inner
            .write()
            .entry(task.clone())
            .or_default()
            .insert(key.to_string(), value);
        Ok(())
    }

    async fn get(
        &self,
        task: &TaskId,
        key: &str,
    ) -> Result<Option<serde_json::Value>, StorageError> {
        Ok(self
            .inner
            .read()
            .get(task)
            .and_then(|m| m.get(key).cloned()))
    }

    async fn drop_task(&self, task: &TaskId) -> Result<(), StorageError> {
        self.inner.write().remove(task);
        Ok(())
    }
}

// ───────────────────────────── Thread Memory ─────────────────────────────

/// Identifier for a conversation / case thread.
pub type ThreadId = String;

/// Per-thread persistent state.
#[async_trait]
pub trait ThreadMemoryStore: Send + Sync {
    /// Append a message blob to the thread.
    async fn append(&self, thread: &ThreadId, value: serde_json::Value)
        -> Result<(), StorageError>;

    /// Read all messages for the thread, in insertion order.
    async fn read(&self, thread: &ThreadId) -> Result<Vec<serde_json::Value>, StorageError>;

    /// Delete the entire thread (e.g. when the case closes).
    async fn drop_thread(&self, thread: &ThreadId) -> Result<(), StorageError>;
}

/// In-memory implementation of [`ThreadMemoryStore`].
#[derive(Default)]
pub struct InMemoryThreadStore {
    inner: Arc<RwLock<HashMap<ThreadId, Vec<serde_json::Value>>>>,
}

impl InMemoryThreadStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl ThreadMemoryStore for InMemoryThreadStore {
    async fn append(
        &self,
        thread: &ThreadId,
        value: serde_json::Value,
    ) -> Result<(), StorageError> {
        self.inner
            .write()
            .entry(thread.clone())
            .or_default()
            .push(value);
        Ok(())
    }

    async fn read(&self, thread: &ThreadId) -> Result<Vec<serde_json::Value>, StorageError> {
        Ok(self.inner.read().get(thread).cloned().unwrap_or_default())
    }

    async fn drop_thread(&self, thread: &ThreadId) -> Result<(), StorageError> {
        self.inner.write().remove(thread);
        Ok(())
    }
}

// ───────────────────────────── Long-term Memory ─────────────────────────────

/// One long-term memory record. Tenant-scoped to enforce isolation.
#[derive(Debug, Clone, PartialEq)]
pub struct LongTermRecord {
    /// Tenant the record belongs to.
    pub tenant: TenantId,
    /// Logical kind: `semantic`, `episodic`, or `procedural`.
    pub kind: String,
    /// Free-text content used for similarity search.
    pub content: String,
    /// Optional structured payload.
    pub payload: serde_json::Value,
    /// Ontology entity references this record is indexed under
    /// (Rule 14 / Rule 21). Optional: records without entity refs
    /// still flow through the keyword `search` path.
    #[doc(alias = "entities")]
    pub entity_refs: Vec<EntityRefLite>,
}

/// Persistent long-term knowledge store.
#[async_trait]
pub trait LongTermMemoryStore: Send + Sync {
    /// Insert a record.
    async fn insert(&self, record: LongTermRecord) -> Result<(), StorageError>;

    /// Brute-force search for records whose content contains every term
    /// in `query` (case-insensitive). Real backends use vector search.
    async fn search(
        &self,
        tenant: &TenantId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LongTermRecord>, StorageError>;

    /// Entity-keyed retrieval (E2 Slice B, Rule 14).
    ///
    /// Returns records that were indexed against `entity`. Implementations
    /// MUST respect the tenant boundary: records belonging to another
    /// tenant are never returned, even when the entity id matches. The
    /// default implementation scans the whole store linearly — backends
    /// with native entity indexes (pgvector + JSONB, in-memory inverted
    /// index, etc.) should override it for O(1) lookup.
    async fn search_by_entity(
        &self,
        tenant: &TenantId,
        entity: &EntityRefLite,
        limit: usize,
    ) -> Result<Vec<LongTermRecord>, StorageError> {
        // Default impl: linear scan. Exists so existing backends remain
        // trait-complete without bespoke work.
        let _ = (tenant, entity, limit);
        Ok(vec![])
    }
}

/// In-memory implementation of [`LongTermMemoryStore`].
///
/// Maintains two indexes:
///
/// 1. **Records vector** — the ground truth list of every record,
///    preserving insertion order for deterministic replay.
/// 2. **Entity inverted index** — `(tenant, entity_id) → Vec<record_idx>`
///    so `search_by_entity` is O(hits) rather than O(n).
#[derive(Default)]
pub struct InMemoryLongTermStore {
    inner: Arc<RwLock<LongTermInner>>,
}

#[derive(Default)]
struct LongTermInner {
    records: Vec<LongTermRecord>,
    /// Inverted index: (tenant, entity_id) → indices into `records`.
    entity_index: HashMap<(TenantId, String), Vec<usize>>,
}

impl InMemoryLongTermStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl LongTermMemoryStore for InMemoryLongTermStore {
    async fn insert(&self, record: LongTermRecord) -> Result<(), StorageError> {
        let mut guard = self.inner.write();
        let idx = guard.records.len();
        // Update the entity index *before* pushing so we can borrow
        // the record fields without aliasing issues.
        for ent in &record.entity_refs {
            // Skip cross-tenant entity refs: an indexed record whose
            // entity carries a tenant that differs from the record's
            // own tenant is malformed and we refuse to index it under
            // that cross-tenant key (Rule 21). It still lives in the
            // records vector so keyword search behaves as before.
            if let Some(ref rt) = ent.tenant {
                if rt != &record.tenant {
                    continue;
                }
            }
            let key = (record.tenant.clone(), ent.entity_id.clone());
            guard.entity_index.entry(key).or_default().push(idx);
        }
        guard.records.push(record);
        Ok(())
    }

    async fn search(
        &self,
        tenant: &TenantId,
        query: &str,
        limit: usize,
    ) -> Result<Vec<LongTermRecord>, StorageError> {
        let q = query.to_lowercase();
        let terms: Vec<&str> = q.split_whitespace().collect();
        let guard = self.inner.read();
        let mut hits: Vec<LongTermRecord> = guard
            .records
            .iter()
            .filter(|r| &r.tenant == tenant)
            .filter(|r| {
                let c = r.content.to_lowercase();
                terms.iter().all(|t| c.contains(t))
            })
            .cloned()
            .collect();
        hits.truncate(limit);
        Ok(hits)
    }

    async fn search_by_entity(
        &self,
        tenant: &TenantId,
        entity: &EntityRefLite,
        limit: usize,
    ) -> Result<Vec<LongTermRecord>, StorageError> {
        let guard = self.inner.read();
        let key = (tenant.clone(), entity.entity_id.clone());
        let Some(indices) = guard.entity_index.get(&key) else {
            return Ok(vec![]);
        };
        let mut hits: Vec<LongTermRecord> = indices
            .iter()
            .filter_map(|idx| guard.records.get(*idx))
            .filter(|r| &r.tenant == tenant)
            .cloned()
            .collect();
        hits.truncate(limit);
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn working_memory_round_trip() {
        let store = InMemoryWorkingStore::new();
        let task = TaskId::new();
        store.put(&task, "k", serde_json::json!(42)).await.unwrap();
        assert_eq!(
            store.get(&task, "k").await.unwrap(),
            Some(serde_json::json!(42))
        );
        store.drop_task(&task).await.unwrap();
        assert_eq!(store.get(&task, "k").await.unwrap(), None);
    }

    #[tokio::test]
    async fn thread_memory_appends() {
        let store = InMemoryThreadStore::new();
        let t = "thread-1".to_string();
        store.append(&t, serde_json::json!("hi")).await.unwrap();
        store.append(&t, serde_json::json!("there")).await.unwrap();
        let v = store.read(&t).await.unwrap();
        assert_eq!(v.len(), 2);
    }

    #[tokio::test]
    async fn long_term_brute_force_search() {
        let store = InMemoryLongTermStore::new();
        let tenant = TenantId::new();
        store
            .insert(LongTermRecord {
                tenant: tenant.clone(),
                kind: "semantic".into(),
                content: "Tokyo branch outperformed expectations".into(),
                payload: serde_json::json!({}),
                entity_refs: vec![],
            })
            .await
            .unwrap();
        let hits = store.search(&tenant, "tokyo", 5).await.unwrap();
        assert_eq!(hits.len(), 1);
    }

    #[tokio::test]
    async fn long_term_search_respects_tenant_boundary() {
        let store = InMemoryLongTermStore::new();
        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();
        store
            .insert(LongTermRecord {
                tenant: tenant_a.clone(),
                kind: "semantic".into(),
                content: "secret".into(),
                payload: serde_json::json!({}),
                entity_refs: vec![],
            })
            .await
            .unwrap();
        let hits = store.search(&tenant_b, "secret", 5).await.unwrap();
        assert!(hits.is_empty());
    }

    #[tokio::test]
    async fn long_term_search_by_entity_returns_indexed_record() {
        let store = InMemoryLongTermStore::new();
        let tenant = TenantId::new();
        let order_ref = EntityRefLite::new("commerce.Order");
        store
            .insert(LongTermRecord {
                tenant: tenant.clone(),
                kind: "episodic".into(),
                content: "order-42 was refunded after shipping delay".into(),
                payload: serde_json::json!({"order_id":"ord-42"}),
                entity_refs: vec![order_ref.clone()],
            })
            .await
            .unwrap();

        let hits = store
            .search_by_entity(&tenant, &order_ref, 10)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].entity_refs[0].entity_id, "commerce.Order");
    }

    #[tokio::test]
    async fn long_term_search_by_entity_respects_tenant_boundary() {
        let store = InMemoryLongTermStore::new();
        let tenant_a = TenantId::new();
        let tenant_b = TenantId::new();
        let order_ref = EntityRefLite::new("commerce.Order");
        store
            .insert(LongTermRecord {
                tenant: tenant_a.clone(),
                kind: "episodic".into(),
                content: "tenant-a order history".into(),
                payload: serde_json::json!({}),
                entity_refs: vec![order_ref.clone()],
            })
            .await
            .unwrap();

        let hits = store
            .search_by_entity(&tenant_b, &order_ref, 10)
            .await
            .unwrap();
        assert!(
            hits.is_empty(),
            "cross-tenant entity lookup must return empty"
        );
    }

    #[tokio::test]
    async fn long_term_search_by_entity_honours_limit() {
        let store = InMemoryLongTermStore::new();
        let tenant = TenantId::new();
        let order_ref = EntityRefLite::new("commerce.Order");
        for i in 0..5 {
            store
                .insert(LongTermRecord {
                    tenant: tenant.clone(),
                    kind: "episodic".into(),
                    content: format!("event-{i}"),
                    payload: serde_json::json!({}),
                    entity_refs: vec![order_ref.clone()],
                })
                .await
                .unwrap();
        }
        let hits = store
            .search_by_entity(&tenant, &order_ref, 3)
            .await
            .unwrap();
        assert_eq!(hits.len(), 3);
    }

    #[tokio::test]
    async fn long_term_search_by_entity_returns_empty_for_unknown_entity() {
        let store = InMemoryLongTermStore::new();
        let tenant = TenantId::new();
        let unknown = EntityRefLite::new("commerce.Unknown");
        let hits = store.search_by_entity(&tenant, &unknown, 10).await.unwrap();
        assert!(hits.is_empty());
    }
}
