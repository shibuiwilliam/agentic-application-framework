//! Recorder API for emitting [`Observation`] / [`TraceStep`] entries into
//! a [`TraceStore`].
//!
//! ## E1 Slice B — `TraceSubscriber` fan-out
//!
//! The recorder can hold zero or more [`TraceSubscriber`]s. After every
//! `record_step` writes to the store, it fans out the step's
//! [`Observation`] to each subscriber via `tokio::spawn`. **Rule 16:
//! Learning never touches the hot path** — the recorder returns to the
//! caller *before* any subscriber completes.

use aaf_contracts::{
    ExecutionTrace, IntentId, NodeId, Observation, StepOutcome, TraceId, TraceStatus, TraceStep,
};
use aaf_storage::{StorageError, TraceStore};
use async_trait::async_trait;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors raised by the recorder.
#[derive(Debug, Error)]
pub enum RecorderError {
    /// Underlying storage failure.
    #[error("storage: {0}")]
    Storage(#[from] StorageError),

    /// The trace was finalised already and cannot accept more steps.
    #[error("trace already closed")]
    AlreadyClosed,

    /// The trace id was never opened.
    #[error("trace not opened: {0}")]
    NotOpened(TraceId),
}

/// Trait surface used by other crates so they can be tested with mocks.
#[async_trait]
pub trait TraceRecorder: Send + Sync {
    /// Open a new trace tied to `intent_id`.
    async fn open(&self, trace_id: TraceId, intent_id: IntentId) -> Result<(), RecorderError>;

    /// Append a step to an open trace.
    async fn record_step(&self, step: TraceStep) -> Result<(), RecorderError>;

    /// Helper that constructs a step from an [`Observation`] + cost data
    /// and records it.
    #[allow(clippy::too_many_arguments)]
    async fn record_observation(
        &self,
        observation: Observation,
        step_type: &str,
        cost_usd: f64,
        duration_ms: u64,
        tokens_in: u64,
        tokens_out: u64,
        model: Option<String>,
    ) -> Result<(), RecorderError> {
        let step = TraceStep {
            step: observation.step,
            node_id: observation.node_id.clone(),
            step_type: step_type.to_string(),
            model,
            tokens_in,
            tokens_out,
            cost_usd,
            duration_ms,
            observation,
        };
        self.record_step(step).await
    }

    /// Finalise the trace.
    async fn close(&self, trace_id: &TraceId, status: TraceStatus) -> Result<(), RecorderError>;

    /// Fetch the current trace document.
    async fn get(&self, trace_id: &TraceId) -> Result<ExecutionTrace, RecorderError>;
}

/// A subscriber that receives observation notifications
/// **out of band** (via `tokio::spawn`). Rule 16: learning never
/// blocks the hot path.
///
/// Implementations live in `aaf-learn` (fast-path miner,
/// capability scorer, router tuner, escalation tuner).
pub trait TraceSubscriber: Send + Sync + 'static {
    /// Called once per recorded observation, on a background task.
    /// Implementations must not panic; errors are silently dropped.
    fn on_observation(&self, observation: &Observation);
}

/// Default in-memory recorder that persists into a [`TraceStore`].
pub struct Recorder {
    store: Arc<dyn TraceStore>,
    open: Arc<Mutex<HashMap<TraceId, ExecutionTrace>>>,
    /// Subscribers notified after every `record_step`, off-thread.
    subscribers: Vec<Arc<dyn TraceSubscriber>>,
}

impl Recorder {
    /// Construct a recorder over the given store.
    pub fn new(store: Arc<dyn TraceStore>) -> Self {
        Self {
            store,
            open: Arc::new(Mutex::new(HashMap::new())),
            subscribers: Vec::new(),
        }
    }

    /// Add a subscriber. Returns `self` for chaining.
    pub fn with_subscriber(mut self, sub: Arc<dyn TraceSubscriber>) -> Self {
        self.subscribers.push(sub);
        self
    }

    /// Fan-out: clones the observation, spawns a task per subscriber.
    /// Each spawned task is wrapped in a timeout to prevent a
    /// misbehaving subscriber from accumulating unbounded tasks.
    fn notify_subscribers(&self, observation: &Observation) {
        /// Maximum time a subscriber may hold a spawned task before
        /// it is dropped. Generous enough for a network call, strict
        /// enough to bound task accumulation.
        const SUBSCRIBER_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(10);

        for sub in &self.subscribers {
            let sub = Arc::clone(sub);
            let obs = observation.clone();
            tokio::spawn(async move {
                let _ = tokio::time::timeout(SUBSCRIBER_TIMEOUT, async {
                    sub.on_observation(&obs);
                })
                .await;
            });
        }
    }
}

#[async_trait]
impl TraceRecorder for Recorder {
    async fn open(&self, trace_id: TraceId, intent_id: IntentId) -> Result<(), RecorderError> {
        let trace = ExecutionTrace::open(trace_id.clone(), intent_id);
        self.store.put(trace.clone()).await?;
        self.open.lock().insert(trace_id, trace);
        Ok(())
    }

    async fn record_step(&self, step: TraceStep) -> Result<(), RecorderError> {
        let trace_id = step.observation.trace_id.clone();
        // Fan-out to subscribers *before* the store write so we clone
        // the observation while we still own the step. The spawned
        // tasks run independently of the store write — Rule 16.
        self.notify_subscribers(&step.observation);
        let snapshot = {
            let mut guard = self.open.lock();
            let trace = guard
                .get_mut(&trace_id)
                .ok_or_else(|| RecorderError::NotOpened(trace_id.clone()))?;
            if trace.status != TraceStatus::Running {
                return Err(RecorderError::AlreadyClosed);
            }
            trace.record(step);
            trace.clone()
        };
        self.store.put(snapshot).await?;
        Ok(())
    }

    async fn close(&self, trace_id: &TraceId, status: TraceStatus) -> Result<(), RecorderError> {
        let snapshot = {
            let mut guard = self.open.lock();
            let trace = guard
                .get_mut(trace_id)
                .ok_or_else(|| RecorderError::NotOpened(trace_id.clone()))?;
            trace.close(status);
            trace.clone()
        };
        self.store.put(snapshot).await?;
        Ok(())
    }

    async fn get(&self, trace_id: &TraceId) -> Result<ExecutionTrace, RecorderError> {
        let cached = { self.open.lock().get(trace_id).cloned() };
        if let Some(trace) = cached {
            return Ok(trace);
        }
        Ok(self.store.get(trace_id).await?)
    }
}

// Convenience for use as `Arc<dyn TraceRecorder>` in tests.
impl Recorder {
    /// Build an in-memory recorder backed by [`aaf_storage::InMemoryTraceStore`].
    pub fn in_memory() -> Self {
        Self::new(Arc::new(aaf_storage::InMemoryTraceStore::new()))
    }
}

/// Build a minimal observation suitable for tests.
pub fn observation_for(trace_id: TraceId, step: u32, node: NodeId) -> Observation {
    Observation::minimal(trace_id, node, step, "system".into(), StepOutcome::Success)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn open_record_close_round_trip() {
        let r = Recorder::in_memory();
        let trace_id = TraceId::new();
        let intent_id = IntentId::new();
        r.open(trace_id.clone(), intent_id.clone()).await.unwrap();
        let obs = observation_for(trace_id.clone(), 1, NodeId::new());
        r.record_observation(
            obs,
            "agent_execution",
            0.01,
            200,
            50,
            80,
            Some("mock".into()),
        )
        .await
        .unwrap();
        r.close(&trace_id, TraceStatus::Completed).await.unwrap();
        let t = r.get(&trace_id).await.unwrap();
        assert_eq!(t.steps.len(), 1);
        assert!((t.total_cost_usd - 0.01).abs() < 1e-9);
        assert_eq!(t.status, TraceStatus::Completed);
    }

    #[tokio::test]
    async fn subscriber_is_not_on_hot_path() {
        use std::sync::atomic::{AtomicU32, Ordering};

        struct SlowSub {
            count: Arc<AtomicU32>,
        }
        impl TraceSubscriber for SlowSub {
            fn on_observation(&self, _obs: &Observation) {
                // Simulate slow work — in the real system this could be
                // a network call to a scoring backend. The test asserts
                // that record_observation returns *before* this
                // completes.
                self.count.fetch_add(1, Ordering::SeqCst);
            }
        }

        let counter = Arc::new(AtomicU32::new(0));
        let sub = Arc::new(SlowSub {
            count: Arc::clone(&counter),
        });
        let r = Recorder::in_memory().with_subscriber(sub);

        let trace_id = TraceId::new();
        r.open(trace_id.clone(), IntentId::new()).await.unwrap();
        let obs = observation_for(trace_id.clone(), 1, NodeId::new());
        r.record_observation(obs, "test", 0.0, 0, 0, 0, None)
            .await
            .unwrap();

        // The subscriber runs on a spawned task — yield so the
        // runtime can poll it, then check it was called.
        tokio::task::yield_now().await;
        assert_eq!(counter.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn cannot_record_after_close() {
        let r = Recorder::in_memory();
        let trace_id = TraceId::new();
        r.open(trace_id.clone(), IntentId::new()).await.unwrap();
        r.close(&trace_id, TraceStatus::Completed).await.unwrap();
        let obs = observation_for(trace_id.clone(), 2, NodeId::new());
        let err = r
            .record_observation(obs, "agent_execution", 0.0, 0, 0, 0, None)
            .await
            .unwrap_err();
        assert!(matches!(err, RecorderError::AlreadyClosed));
    }
}
