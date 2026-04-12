//! Wrapper around an [`aaf_storage::CheckpointStore`].

use crate::error::RuntimeError;
use aaf_contracts::{CheckpointId, NodeId, TaskId};
use aaf_storage::{Checkpoint, CheckpointStore};
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;

use crate::node::NodeOutput;

/// Helper that serialises the runtime's intermediate state into a
/// [`Checkpoint`] payload and writes it via the configured store.
pub struct CheckpointWriter {
    store: Arc<dyn CheckpointStore>,
}

impl CheckpointWriter {
    /// Construct.
    pub fn new(store: Arc<dyn CheckpointStore>) -> Self {
        Self { store }
    }

    /// Write a checkpoint after step `step` for `task`. The state map
    /// is `node_id → output.data` so the runtime can resume.
    pub async fn write(
        &self,
        task: TaskId,
        step: u32,
        outputs: &HashMap<NodeId, NodeOutput>,
        remaining_budget_usd: f64,
        remaining_time_ms: u64,
    ) -> Result<CheckpointId, RuntimeError> {
        let mut state = serde_json::Map::new();
        for (k, v) in outputs {
            state.insert(k.to_string(), v.data.clone());
        }
        let cp = Checkpoint {
            checkpoint_id: CheckpointId::new(),
            task_id: task,
            step,
            state: serde_json::Value::Object(state),
            remaining_budget_usd,
            remaining_time_ms,
            created_at: Utc::now(),
        };
        let id = cp.checkpoint_id.clone();
        self.store.put(cp).await?;
        Ok(id)
    }
}
