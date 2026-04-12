//! Checkpoint store trait + in-memory implementation.

use crate::error::StorageError;
use aaf_contracts::{CheckpointId, TaskId};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Persistent checkpoint of a task at a particular step boundary.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Checkpoint {
    /// Stable id.
    pub checkpoint_id: CheckpointId,
    /// Owning task.
    pub task_id: TaskId,
    /// Step the checkpoint was taken after.
    pub step: u32,
    /// Opaque blob containing the runtime state at this step.
    pub state: serde_json::Value,
    /// Remaining USD budget at the time of the checkpoint.
    pub remaining_budget_usd: f64,
    /// Remaining time-to-live in milliseconds.
    pub remaining_time_ms: u64,
    /// Created at.
    pub created_at: DateTime<Utc>,
}

/// Trait for any checkpoint storage backend.
#[async_trait]
pub trait CheckpointStore: Send + Sync {
    /// Persist a checkpoint, replacing any prior checkpoint at the same
    /// `checkpoint_id`.
    async fn put(&self, cp: Checkpoint) -> Result<(), StorageError>;

    /// Fetch the checkpoint at `id`.
    async fn get(&self, id: &CheckpointId) -> Result<Checkpoint, StorageError>;

    /// Fetch the latest checkpoint for `task`, if any.
    async fn latest_for(&self, task: &TaskId) -> Result<Option<Checkpoint>, StorageError>;

    /// Delete a checkpoint.
    async fn delete(&self, id: &CheckpointId) -> Result<(), StorageError>;
}

/// In-memory checkpoint store, used for tests and dev.
#[derive(Default)]
pub struct InMemoryCheckpointStore {
    inner: Arc<RwLock<HashMap<CheckpointId, Checkpoint>>>,
}

impl InMemoryCheckpointStore {
    /// Construct an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl CheckpointStore for InMemoryCheckpointStore {
    async fn put(&self, cp: Checkpoint) -> Result<(), StorageError> {
        self.inner.write().insert(cp.checkpoint_id.clone(), cp);
        Ok(())
    }

    async fn get(&self, id: &CheckpointId) -> Result<Checkpoint, StorageError> {
        self.inner
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }

    async fn latest_for(&self, task: &TaskId) -> Result<Option<Checkpoint>, StorageError> {
        let guard = self.inner.read();
        let mut latest: Option<Checkpoint> = None;
        for cp in guard.values() {
            if &cp.task_id == task && latest.as_ref().map_or(true, |l| l.step < cp.step) {
                latest = Some(cp.clone());
            }
        }
        Ok(latest)
    }

    async fn delete(&self, id: &CheckpointId) -> Result<(), StorageError> {
        self.inner.write().remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn round_trip() {
        let store = InMemoryCheckpointStore::new();
        let task = TaskId::new();
        let cp = Checkpoint {
            checkpoint_id: CheckpointId::new(),
            task_id: task.clone(),
            step: 1,
            state: serde_json::json!({"foo":1}),
            remaining_budget_usd: 0.5,
            remaining_time_ms: 5000,
            created_at: Utc::now(),
        };
        store.put(cp.clone()).await.unwrap();
        let got = store.get(&cp.checkpoint_id).await.unwrap();
        assert_eq!(got, cp);
    }

    #[tokio::test]
    async fn latest_returns_highest_step() {
        let store = InMemoryCheckpointStore::new();
        let task = TaskId::new();
        for step in 1..=3 {
            store
                .put(Checkpoint {
                    checkpoint_id: CheckpointId::new(),
                    task_id: task.clone(),
                    step,
                    state: serde_json::json!({}),
                    remaining_budget_usd: 0.5,
                    remaining_time_ms: 5000,
                    created_at: Utc::now(),
                })
                .await
                .unwrap();
        }
        let latest = store.latest_for(&task).await.unwrap().unwrap();
        assert_eq!(latest.step, 3);
    }
}
