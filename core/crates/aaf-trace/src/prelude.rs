//! Convenience prelude — `use aaf_trace::prelude::*;` brings the
//! most-used types into scope.

pub use crate::cost_attribution::{AttributionRule, CostAttribution, CostAttributor, CostBucket};
pub use crate::export::{otel_json_for, OtelAttr, OtelSpan, OtelStatus};
pub use crate::metrics::TraceMetrics;
pub use crate::recorder::{Recorder, TraceRecorder, TraceSubscriber};
pub use crate::replay::{ReplayError, Replayer};
