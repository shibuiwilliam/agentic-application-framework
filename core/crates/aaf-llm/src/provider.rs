//! Provider trait + request/response shapes.
//!
//! ## E4 Slice A — Tool support
//!
//! `ChatRequest` now carries optional `tools` and `tool_choice` fields.
//! `ChatResponse` reports a `stop_reason` and optional `tool_use` block.
//! `ChatMessage` supports `Role::ToolUse` and `Role::ToolResult` for
//! multi-turn tool conversations. All new fields use `#[serde(default)]`
//! for backward compatibility.

use aaf_contracts::tool::{StopReason, ToolChoice, ToolDefinition, ToolResultBlock, ToolUseBlock};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Chat role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    /// System / preamble.
    System,
    /// User input.
    User,
    /// Assistant reply.
    Assistant,
    /// Assistant requesting a tool call.
    ToolUse,
    /// Result of a tool invocation.
    ToolResult,
}

/// One chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role.
    pub role: Role,
    /// Text content.
    pub content: String,
    /// Tool-use block (present when `role == ToolUse`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<ToolUseBlock>,
    /// Tool-result block (present when `role == ToolResult`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_result: Option<ToolResultBlock>,
}

impl ChatMessage {
    /// Helper for system messages.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
            tool_use: None,
            tool_result: None,
        }
    }
    /// Helper for user messages.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
            tool_use: None,
            tool_result: None,
        }
    }
    /// Helper for assistant messages.
    pub fn assistant(content: impl Into<String>) -> Self {
        Self {
            role: Role::Assistant,
            content: content.into(),
            tool_use: None,
            tool_result: None,
        }
    }
    /// Helper for a tool-use message (assistant requesting a tool call).
    pub fn tool_use_msg(block: ToolUseBlock) -> Self {
        Self {
            role: Role::ToolUse,
            content: String::new(),
            tool_use: Some(block),
            tool_result: None,
        }
    }
    /// Helper for a tool-result message (result sent back to the LLM).
    pub fn tool_result_msg(block: ToolResultBlock) -> Self {
        Self {
            role: Role::ToolResult,
            content: block.content.clone(),
            tool_use: None,
            tool_result: Some(block),
        }
    }
}

/// Chat completion request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChatRequest {
    /// Model identifier.
    #[serde(default)]
    pub model: String,
    /// Sampling temperature in `[0,1]`.
    #[serde(default)]
    pub temperature: f32,
    /// Hard cap on output tokens for the call.
    #[serde(default)]
    pub max_output_tokens: u32,
    /// Conversation messages.
    #[serde(default)]
    pub messages: Vec<ChatMessage>,
    /// Tool definitions available to the LLM (E4, Rule 25).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub tools: Vec<ToolDefinition>,
    /// How the LLM should choose tools.
    #[serde(default)]
    pub tool_choice: ToolChoice,
}

/// Chat completion response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct ChatResponse {
    /// Generated content.
    #[serde(default)]
    pub content: String,
    /// Tokens consumed on the input side.
    #[serde(default)]
    pub tokens_in: u32,
    /// Tokens generated on the output side.
    #[serde(default)]
    pub tokens_out: u32,
    /// USD cost charged by the provider.
    #[serde(default)]
    pub cost_usd: f64,
    /// Provider model id.
    #[serde(default)]
    pub model: String,
    /// Why the turn ended (Rule 28).
    #[serde(default)]
    pub stop_reason: StopReason,
    /// Tool invocation requested by the LLM (present when
    /// `stop_reason == ToolUse`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tool_use: Option<ToolUseBlock>,
}

/// Structured metrics recorded for every LLM call (Rule 35).
///
/// Used by the trace system for cost attribution and by the value
/// router for health tracking.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProviderMetrics {
    /// Model identifier.
    pub model: String,
    /// Input tokens consumed.
    pub input_tokens: u32,
    /// Output tokens generated.
    pub output_tokens: u32,
    /// USD cost.
    pub cost_usd: f64,
    /// Wall-clock latency in ms.
    pub latency_ms: u64,
    /// Provider name (e.g. "anthropic", "openai", "mock").
    pub provider_name: String,
    /// Why the call ended.
    pub stop_reason: StopReason,
}

impl ProviderMetrics {
    /// Build metrics from a response and timing info.
    pub fn from_response(resp: &ChatResponse, latency_ms: u64, provider_name: &str) -> Self {
        Self {
            model: resp.model.clone(),
            input_tokens: resp.tokens_in,
            output_tokens: resp.tokens_out,
            cost_usd: resp.cost_usd,
            latency_ms,
            provider_name: provider_name.to_string(),
            stop_reason: resp.stop_reason,
        }
    }
}

/// Streaming chat event (E5 readiness).
///
/// When streaming is implemented, the LLM provider will emit a stream
/// of these events instead of a single `ChatResponse`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ChatEvent {
    /// Incremental text output.
    TextDelta {
        /// The text fragment.
        text: String,
    },
    /// The LLM wants to call a tool.
    ToolUse {
        /// Tool call identifier.
        id: String,
        /// Tool name.
        name: String,
        /// Tool input arguments.
        input: serde_json::Value,
    },
    /// The turn is complete.
    ContentComplete {
        /// Full accumulated content.
        content: String,
        /// Why the turn ended.
        stop_reason: StopReason,
    },
}

/// Provider errors.
#[derive(Debug, Error)]
pub enum LLMError {
    /// Provider returned a transport / API error.
    #[error("provider error: {0}")]
    Provider(String),

    /// The request exceeded the per-call budget.
    #[error("call budget exceeded")]
    BudgetExceeded,

    /// Provider returned HTTP 429. The `u64` is the suggested
    /// retry-after delay in seconds (0 if the header was absent).
    #[error("rate limited (retry after {0}s)")]
    RateLimited(u64),

    /// Provider returned HTTP 400 — the request was malformed.
    #[error("invalid request: {0}")]
    InvalidRequest(String),

    /// Provider returned HTTP 401 or 403 — bad or missing API key.
    #[error("auth error: {0}")]
    AuthError(String),

    /// The HTTP request timed out.
    #[error("request timed out")]
    Timeout,
}

/// Pluggable LLM provider.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Identifier shown in traces.
    fn name(&self) -> &str;

    /// Run a chat completion.
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LLMError>;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn role_serde_round_trip() {
        // Existing roles
        let json = serde_json::to_string(&Role::System).unwrap();
        assert_eq!(json, r#""system""#);

        // New roles
        let json = serde_json::to_string(&Role::ToolUse).unwrap();
        assert_eq!(json, r#""tool_use""#);

        let back: Role = serde_json::from_str(r#""tool_result""#).unwrap();
        assert_eq!(back, Role::ToolResult);
    }

    #[test]
    fn chat_request_backward_compat() {
        // A request without tool fields deserializes fine
        let json = r#"{"model":"m","temperature":0.5,"max_output_tokens":100,"messages":[]}"#;
        let req: ChatRequest = serde_json::from_str(json).unwrap();
        assert!(req.tools.is_empty());
        assert_eq!(req.tool_choice, ToolChoice::Auto);
    }

    #[test]
    fn chat_response_backward_compat() {
        // A response without stop_reason/tool_use deserializes fine
        let json = r#"{"content":"hi","tokens_in":10,"tokens_out":5,"cost_usd":0.01,"model":"m"}"#;
        let resp: ChatResponse = serde_json::from_str(json).unwrap();
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert!(resp.tool_use.is_none());
    }

    #[test]
    fn chat_message_helpers() {
        let sys = ChatMessage::system("hello");
        assert_eq!(sys.role, Role::System);
        assert!(sys.tool_use.is_none());

        let tu = ChatMessage::tool_use_msg(ToolUseBlock {
            id: "tc-1".into(),
            name: "search".into(),
            input: serde_json::json!({}),
        });
        assert_eq!(tu.role, Role::ToolUse);
        assert!(tu.tool_use.is_some());
    }

    #[test]
    fn chat_event_serde() {
        let evt = ChatEvent::TextDelta {
            text: "hello".into(),
        };
        let json = serde_json::to_string(&evt).unwrap();
        let back: ChatEvent = serde_json::from_str(&json).unwrap();
        assert_eq!(evt, back);

        let evt2 = ChatEvent::ContentComplete {
            content: "done".into(),
            stop_reason: StopReason::EndTurn,
        };
        let json2 = serde_json::to_string(&evt2).unwrap();
        let back2: ChatEvent = serde_json::from_str(&json2).unwrap();
        assert_eq!(evt2, back2);
    }

    #[test]
    fn provider_metrics_from_response() {
        let resp = ChatResponse {
            content: "hi".into(),
            tokens_in: 10,
            tokens_out: 5,
            cost_usd: 0.001,
            model: "test-model".into(),
            stop_reason: StopReason::EndTurn,
            tool_use: None,
        };
        let metrics = ProviderMetrics::from_response(&resp, 150, "mock");
        assert_eq!(metrics.input_tokens, 10);
        assert_eq!(metrics.output_tokens, 5);
        assert_eq!(metrics.latency_ms, 150);
        assert_eq!(metrics.provider_name, "mock");
    }
}
