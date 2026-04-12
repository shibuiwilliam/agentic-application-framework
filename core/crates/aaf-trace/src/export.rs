//! Trace export formats.
//!
//! Rule 12 explicitly requires AAF traces to be **OpenTelemetry-compatible**.
//! Rather than depend on the heavy `opentelemetry` SDK in v0.1, this
//! module produces a JSON shape that maps 1:1 onto the OTLP/JSON span
//! schema. Downstream systems (a real OTel exporter, an OTLP collector,
//! or a custom trace UI) can ingest the output without further
//! translation.
//!
//! Mapping:
//!
//! | OTel field      | AAF source                          |
//! |-----------------|-------------------------------------|
//! | `traceId`       | `ExecutionTrace.trace_id`           |
//! | `spanId`        | `TraceStep.observation.node_id`     |
//! | `name`          | `TraceStep.step_type`               |
//! | `startTimeUnixNano` | `step.observation.recorded_at - duration_ms` |
//! | `endTimeUnixNano`   | `step.observation.recorded_at`              |
//! | `attributes`    | step model / tokens / cost_usd      |
//! | `status.code`   | derived from observation outcome    |

use aaf_contracts::{ExecutionTrace, StepOutcome, TraceStep};
use serde::{Deserialize, Serialize};

/// One OTel span (OTLP/JSON shape, simplified to what consumers need).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtelSpan {
    /// Trace id.
    #[serde(rename = "traceId")]
    pub trace_id: String,
    /// Span id (per step).
    #[serde(rename = "spanId")]
    pub span_id: String,
    /// Step type as the span name.
    pub name: String,
    /// Start time in Unix nanoseconds.
    #[serde(rename = "startTimeUnixNano")]
    pub start_time_unix_nano: i128,
    /// End time in Unix nanoseconds.
    #[serde(rename = "endTimeUnixNano")]
    pub end_time_unix_nano: i128,
    /// Key/value attributes.
    pub attributes: Vec<OtelAttr>,
    /// Span status.
    pub status: OtelStatus,
}

/// One key/value attribute in OTel format.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtelAttr {
    /// Attribute key.
    pub key: String,
    /// Stringified value (OTel allows typed values; we keep it simple).
    pub value: String,
}

/// OTel span status.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OtelStatus {
    /// `OK`, `ERROR`, or `UNSET`.
    pub code: String,
    /// Optional message.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

fn step_to_span(trace_id: &str, step: &TraceStep) -> OtelSpan {
    // chrono 0.4.26 only has the deprecated `timestamp_nanos` (no Option
    // form). Cast through i128 so the math below stays sign-safe.
    #[allow(deprecated)]
    let end_ns = step.observation.recorded_at.timestamp_nanos() as i128;
    let start_ns = end_ns - (step.duration_ms as i128) * 1_000_000;
    let mut attrs = vec![
        OtelAttr {
            key: "aaf.step".into(),
            value: step.step.to_string(),
        },
        OtelAttr {
            key: "aaf.tokens_in".into(),
            value: step.tokens_in.to_string(),
        },
        OtelAttr {
            key: "aaf.tokens_out".into(),
            value: step.tokens_out.to_string(),
        },
        OtelAttr {
            key: "aaf.cost_usd".into(),
            value: format!("{:.6}", step.cost_usd),
        },
        OtelAttr {
            key: "aaf.confidence".into(),
            value: format!("{:.4}", step.observation.confidence),
        },
    ];
    if let Some(model) = &step.model {
        attrs.push(OtelAttr {
            key: "aaf.model".into(),
            value: model.clone(),
        });
    }
    let code = match step.observation.outcome {
        StepOutcome::Success => "OK",
        StepOutcome::Failure => "ERROR",
        StepOutcome::Skipped | StepOutcome::Compensated | StepOutcome::Pending => "UNSET",
    };
    OtelSpan {
        trace_id: trace_id.to_string(),
        span_id: step.observation.node_id.to_string(),
        name: step.step_type.clone(),
        start_time_unix_nano: start_ns,
        end_time_unix_nano: end_ns,
        attributes: attrs,
        status: OtelStatus {
            code: code.into(),
            message: None,
        },
    }
}

/// Render a whole [`ExecutionTrace`] as the OTLP/JSON document the
/// `traces` endpoint of an OTel collector accepts.
pub fn otel_json_for(trace: &ExecutionTrace) -> serde_json::Value {
    let trace_id = trace.trace_id.to_string();
    let spans: Vec<OtelSpan> = trace
        .steps
        .iter()
        .map(|s| step_to_span(&trace_id, s))
        .collect();
    serde_json::json!({
        "resourceSpans": [{
            "resource": {
                "attributes": [
                    {"key": "service.name", "value": "aaf"},
                    {"key": "aaf.intent_id", "value": trace.intent_id.to_string()},
                ]
            },
            "scopeSpans": [{
                "scope": {"name": "aaf-trace"},
                "spans": spans,
            }]
        }]
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        IntentId, NodeId, Observation, StepOutcome, TraceId, TraceStatus, TraceStep,
    };

    fn sample() -> ExecutionTrace {
        let trace_id = TraceId::new();
        let mut t = ExecutionTrace::open(trace_id.clone(), IntentId::new());
        t.record(TraceStep {
            step: 1,
            node_id: NodeId::from("a"),
            step_type: "node_run".into(),
            model: Some("mock-claude".into()),
            tokens_in: 50,
            tokens_out: 80,
            cost_usd: 0.012,
            duration_ms: 200,
            observation: Observation::minimal(
                trace_id,
                NodeId::from("a"),
                1,
                "agent".into(),
                StepOutcome::Success,
            ),
        });
        t.close(TraceStatus::Completed);
        t
    }

    #[test]
    fn span_carries_aaf_attributes() {
        let t = sample();
        let span = step_to_span(t.trace_id.as_str(), &t.steps[0]);
        assert_eq!(span.name, "node_run");
        assert!(span.attributes.iter().any(|a| a.key == "aaf.cost_usd"));
        assert!(span.attributes.iter().any(|a| a.key == "aaf.model"));
        assert_eq!(span.status.code, "OK");
    }

    #[test]
    fn otel_json_envelope_shape() {
        let t = sample();
        let v = otel_json_for(&t);
        let resource = &v["resourceSpans"][0];
        assert!(resource["resource"]["attributes"].is_array());
        let spans = &resource["scopeSpans"][0]["spans"];
        assert_eq!(spans.as_array().unwrap().len(), 1);
    }
}
