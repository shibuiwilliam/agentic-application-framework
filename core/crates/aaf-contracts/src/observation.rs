//! Observation contract — every step records its inputs, reasoning, and
//! decision so that the entire execution can be replayed and audited
//! (Rule 12: Trace Everything).
//!
//! # Enhancement E1 — Feedback Spine
//!
//! [`Observation::outcome_detail`] carries the structured [`Outcome`]
//! block defined in `PROJECT.md` §16.1 (E1 Feedback Spine). It is populated:
//! - at step-end by the runtime (minimal outcome: status/latency/tokens/cost);
//! - later by the saga engine on saga completion;
//! - later still by the app-native surface on user feedback;
//! - later still by `aaf-eval` on offline scoring.
//!
//! Rule 15: "Feedback is a contract" — outcomes flow only through this field.

use crate::ids::{AgentId, NodeId, TraceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// One thing the agent observed during a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservedSource {
    /// Source identifier (e.g. `salesforce-crm`).
    pub source: String,
    /// Short description of what was observed.
    pub data_summary: String,
    /// Capability used to retrieve the data.
    pub retrieval_method: String,
}

/// Result classification of a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepOutcome {
    /// Successful completion.
    Success,
    /// Failed terminally.
    Failure,
    /// Skipped.
    Skipped,
    /// Compensated.
    Compensated,
    /// Awaiting external input.
    Pending,
}

/// Extended outcome status (E1: more states than [`StepOutcome`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeStatus {
    /// Completed successfully.
    Succeeded,
    /// Failed terminally.
    Failed,
    /// Returned partial results.
    Partial,
    /// Escalated to a human.
    Escalated,
    /// Rolled back via compensation.
    RolledBack,
}

/// User feedback on a produced artifact or proposal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UserFeedback {
    /// Rating label.
    pub rating: FeedbackRating,
    /// Edit distance between the produced artifact and the accepted
    /// version, if applicable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub edit_distance_to_accepted: Option<u32>,
    /// Free-text reason.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub free_text: Option<String>,
}

/// Three-way rating.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FeedbackRating {
    /// Positive.
    Positive,
    /// Neutral.
    Neutral,
    /// Negative.
    Negative,
}

/// Downstream error recorded when a later step or compensation fails
/// and points back at this observation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DownstreamError {
    /// Step id where the downstream failure occurred.
    pub at_step_id: String,
    /// Error code or tag.
    pub error_code: String,
}

/// Semantic score from an LLM-as-judge or golden-set match.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SemanticScore {
    /// Judge model identifier.
    pub judge_model: String,
    /// Score in `[0, 1]`.
    pub score: f64,
    /// Optional reference to the reasoning trace.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning_ref: Option<String>,
}

/// Lightweight reference to a policy violation (Rule 15 — feedback is
/// a contract, not log lines).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyViolationRef {
    /// Rule id.
    pub rule_id: String,
    /// Severity.
    pub severity: String,
}

/// Structured outcome attached to an Observation. Populated lazily by
/// multiple writers (see the module doc comment).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Outcome {
    /// Outcome status.
    pub status: OutcomeStatus,
    /// Latency in milliseconds.
    pub latency_ms: u64,
    /// Tokens consumed.
    pub tokens_used: u32,
    /// USD cost.
    pub cost_usd: f64,
    /// Policy violations observed after the fact.
    #[serde(default)]
    pub policy_violations: Vec<PolicyViolationRef>,
    /// User feedback, if any.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub user_feedback: Option<UserFeedback>,
    /// Downstream error that referenced this step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downstream_error: Option<DownstreamError>,
    /// Semantic score from LLM-as-judge or golden match.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub semantic_score: Option<SemanticScore>,
}

impl Outcome {
    /// Minimal outcome carrying only what the runtime can attach at
    /// step-end without any external input.
    pub fn minimal(
        status: OutcomeStatus,
        latency_ms: u64,
        tokens_used: u32,
        cost_usd: f64,
    ) -> Self {
        Self {
            status,
            latency_ms,
            tokens_used,
            cost_usd,
            policy_violations: vec![],
            user_feedback: None,
            downstream_error: None,
            semantic_score: None,
        }
    }
}

/// One Observation captured during execution.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Observation {
    /// Trace this observation belongs to.
    pub trace_id: TraceId,
    /// Originating node id.
    pub node_id: NodeId,
    /// Step number within the trace (monotonic).
    pub step: u32,
    /// Producing agent (or `system` for non-agent nodes).
    pub agent: AgentId,
    /// What the agent observed.
    #[serde(default)]
    pub observed: Vec<ObservedSource>,
    /// Free-text reasoning string.
    pub reasoning: String,
    /// Final decision string.
    pub decision: String,
    /// Confidence in [0,1].
    pub confidence: f64,
    /// Alternative interpretations the agent considered.
    #[serde(default)]
    pub alternatives: Vec<String>,
    /// Outcome classification (coarse enum).
    pub outcome: StepOutcome,
    /// Recorded at.
    pub recorded_at: DateTime<Utc>,
    // ── Enhancement E1: Feedback Spine ──────────────────────────────
    /// Structured outcome block. Attached lazily — starts `None` and
    /// is filled in by the runtime / saga / surface / eval pipeline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub outcome_detail: Option<Outcome>,
}

impl Observation {
    /// Build a minimal observation. Used heavily in tests.
    pub fn minimal(
        trace_id: TraceId,
        node_id: NodeId,
        step: u32,
        agent: AgentId,
        outcome: StepOutcome,
    ) -> Self {
        Self {
            trace_id,
            node_id,
            step,
            agent,
            observed: vec![],
            reasoning: String::new(),
            decision: String::new(),
            confidence: 0.0,
            alternatives: vec![],
            outcome,
            recorded_at: Utc::now(),
            outcome_detail: None,
        }
    }

    /// Attach a structured outcome after the fact.
    pub fn attach_outcome(&mut self, outcome: Outcome) {
        self.outcome_detail = Some(outcome);
    }
}
