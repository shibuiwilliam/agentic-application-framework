//! Trace store trait + in-memory backend.

use crate::error::StorageError;
use aaf_contracts::{ExecutionTrace, TraceId};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Persistence boundary for execution traces.
#[async_trait]
pub trait TraceStore: Send + Sync {
    /// Insert or replace a trace.
    async fn put(&self, trace: ExecutionTrace) -> Result<(), StorageError>;

    /// Fetch by id.
    async fn get(&self, id: &TraceId) -> Result<ExecutionTrace, StorageError>;

    /// List traces (test/dev only).
    async fn list(&self) -> Result<Vec<ExecutionTrace>, StorageError>;
}

/// In-memory implementation of [`TraceStore`].
#[derive(Default)]
pub struct InMemoryTraceStore {
    inner: Arc<RwLock<HashMap<TraceId, ExecutionTrace>>>,
}

impl InMemoryTraceStore {
    /// Create an empty store.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl TraceStore for InMemoryTraceStore {
    async fn put(&self, trace: ExecutionTrace) -> Result<(), StorageError> {
        self.inner.write().insert(trace.trace_id.clone(), trace);
        Ok(())
    }

    async fn get(&self, id: &TraceId) -> Result<ExecutionTrace, StorageError> {
        self.inner
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| StorageError::NotFound(id.to_string()))
    }

    async fn list(&self) -> Result<Vec<ExecutionTrace>, StorageError> {
        Ok(self.inner.read().values().cloned().collect())
    }
}
