//! AAF trace recorder.
//!
//! Implements **Rule 12 — Trace Everything**: every node execution emits an
//! [`Observation`] which is rolled up into an append-only [`ExecutionTrace`].
//!
//! This crate exposes a [`TraceRecorder`] handle that the runtime, planner,
//! intent compiler, and saga engine all clone via `Arc`. The default
//! implementation persists into a pluggable [`aaf_storage::TraceStore`].

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod cost_attribution;
pub mod export;
pub mod metrics;
pub mod prelude;
pub mod recorder;
pub mod replay;

pub use cost_attribution::{AttributionRule, CostAttribution, CostAttributor, CostBucket};
pub use export::{otel_json_for, OtelAttr, OtelSpan, OtelStatus};
pub use metrics::TraceMetrics;
pub use recorder::{Recorder, TraceRecorder, TraceSubscriber};
pub use replay::{ReplayError, Replayer};
