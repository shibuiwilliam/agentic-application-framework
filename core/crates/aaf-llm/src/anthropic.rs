//! Anthropic Claude provider (Rule 39: use reqwest, not provider SDK).
//!
//! Maps AAF's [`ChatRequest`] / [`ChatResponse`] to the Anthropic
//! Messages API. The request/response mapping, pricing, and retry
//! logic are pure functions — the actual HTTP transport is pluggable
//! via [`HttpSender`].
//!
//! ## Usage with reqwest (when available)
//!
//! ```ignore
//! let sender = ReqwestSender::new(api_key); // impl HttpSender
//! let provider = AnthropicProvider::new(sender);
//! ```
//!
//! ## Usage in tests
//!
//! ```ignore
//! let sender = FixedSender::new(canned_json); // returns same body every call
//! let provider = AnthropicProvider::new(sender);
//! ```

use crate::pricing::{self, ModelPricing};
use crate::provider::{ChatRequest, ChatResponse, LLMError, LLMProvider, Role};
use aaf_contracts::tool::{StopReason, ToolChoice, ToolUseBlock};
use async_trait::async_trait;

/// Pluggable HTTP transport for the Anthropic API.
///
/// Implementations send a JSON body to the Messages API endpoint and
/// return the response body. This decouples the mapping logic from
/// the HTTP client library (Rule 39).
#[async_trait]
pub trait HttpSender: Send + Sync {
    /// Send a POST request and return `(status_code, body_json)`.
    ///
    /// The caller builds the request body; the sender handles
    /// authentication headers, base URL, and timeouts.
    async fn send(&self, body: &serde_json::Value) -> Result<(u16, serde_json::Value), LLMError>;
}

/// A fixed-response sender for unit tests.
pub struct FixedSender {
    status: u16,
    body: serde_json::Value,
}

impl FixedSender {
    /// Construct a sender that always returns the given status + body.
    pub fn new(status: u16, body: serde_json::Value) -> Self {
        Self { status, body }
    }

    /// Shortcut: 200 OK with the given body.
    pub fn ok(body: serde_json::Value) -> Self {
        Self::new(200, body)
    }
}

#[async_trait]
impl HttpSender for FixedSender {
    async fn send(&self, _body: &serde_json::Value) -> Result<(u16, serde_json::Value), LLMError> {
        Ok((self.status, self.body.clone()))
    }
}

/// Anthropic Claude provider.
///
/// Sends requests to the Anthropic Messages API via a pluggable
/// [`HttpSender`]. Handles retry logic for rate limits and server
/// errors.
pub struct AnthropicProvider {
    sender: Box<dyn HttpSender>,
    default_model: String,
    max_retries: u32,
    retry_base_delay_ms: u64,
    pricing: Vec<ModelPricing>,
}

impl AnthropicProvider {
    /// Construct with a custom HTTP sender.
    pub fn new(sender: Box<dyn HttpSender>) -> Self {
        Self {
            sender,
            default_model: "claude-sonnet-4-6-20250514".into(),
            max_retries: 3,
            retry_base_delay_ms: 1000,
            pricing: pricing::anthropic_pricing(),
        }
    }

    /// Override the default model.
    pub fn with_model(mut self, model: impl Into<String>) -> Self {
        self.default_model = model.into();
        self
    }

    /// Override the maximum retry count.
    pub fn with_max_retries(mut self, n: u32) -> Self {
        self.max_retries = n;
        self
    }

    /// Override the pricing table.
    pub fn with_pricing(mut self, pricing: Vec<ModelPricing>) -> Self {
        self.pricing = pricing;
        self
    }

    // ── Request building ────────────────────────────────────────────

    /// Build the Anthropic Messages API request body from an AAF
    /// `ChatRequest`.
    pub fn build_request_body(&self, req: &ChatRequest) -> serde_json::Value {
        let model = if req.model.is_empty() {
            &self.default_model
        } else {
            &req.model
        };

        // Extract system message.
        let system_text: String = req
            .messages
            .iter()
            .filter(|m| m.role == Role::System)
            .map(|m| m.content.as_str())
            .collect::<Vec<_>>()
            .join("\n");

        // Build messages array (non-system).
        let messages: Vec<serde_json::Value> = req
            .messages
            .iter()
            .filter(|m| m.role != Role::System)
            .map(Self::map_message)
            .collect();

        let mut body = serde_json::json!({
            "model": model,
            "max_tokens": if req.max_output_tokens > 0 { req.max_output_tokens } else { 1024 },
            "messages": messages,
        });

        if !system_text.is_empty() {
            body["system"] = serde_json::json!(system_text);
        }

        if !req.tools.is_empty() {
            let tools: Vec<serde_json::Value> = req
                .tools
                .iter()
                .map(|t| {
                    serde_json::json!({
                        "name": t.name,
                        "description": t.description,
                        "input_schema": t.input_schema,
                    })
                })
                .collect();
            body["tools"] = serde_json::json!(tools);
            body["tool_choice"] = match &req.tool_choice {
                ToolChoice::Auto => serde_json::json!({"type": "auto"}),
                ToolChoice::None => serde_json::json!({"type": "none"}),
                ToolChoice::Specific(name) => {
                    serde_json::json!({"type": "tool", "name": name})
                }
            };
        }

        body
    }

    /// Map one AAF `ChatMessage` to an Anthropic message object.
    fn map_message(msg: &crate::provider::ChatMessage) -> serde_json::Value {
        match msg.role {
            Role::User => serde_json::json!({
                "role": "user",
                "content": msg.content,
            }),
            Role::Assistant => {
                if let Some(tu) = &msg.tool_use {
                    serde_json::json!({
                        "role": "assistant",
                        "content": [{"type": "tool_use", "id": tu.id, "name": tu.name, "input": tu.input}],
                    })
                } else {
                    serde_json::json!({"role": "assistant", "content": msg.content})
                }
            }
            Role::ToolUse => {
                let tu = msg
                    .tool_use
                    .as_ref()
                    .expect("ToolUse role requires tool_use block");
                serde_json::json!({
                    "role": "assistant",
                    "content": [{"type": "tool_use", "id": tu.id, "name": tu.name, "input": tu.input}],
                })
            }
            Role::ToolResult => {
                let tr = msg
                    .tool_result
                    .as_ref()
                    .expect("ToolResult role requires tool_result block");
                serde_json::json!({
                    "role": "user",
                    "content": [{"type": "tool_result", "tool_use_id": tr.tool_use_id, "content": tr.content}],
                })
            }
            Role::System => serde_json::json!({"role": "user", "content": ""}),
        }
    }

    // ── Response parsing ────────────────────────────────────────────

    /// Parse an Anthropic Messages API response body into an AAF
    /// `ChatResponse`.
    pub fn parse_response(
        &self,
        body: &serde_json::Value,
        model: &str,
    ) -> Result<ChatResponse, LLMError> {
        let tokens_in = body["usage"]["input_tokens"].as_u64().unwrap_or(0) as u32;
        let tokens_out = body["usage"]["output_tokens"].as_u64().unwrap_or(0) as u32;

        let stop_reason = match body["stop_reason"].as_str().unwrap_or("end_turn") {
            "tool_use" => StopReason::ToolUse,
            "max_tokens" => StopReason::MaxTokens,
            _ => StopReason::EndTurn,
        };

        let mut content = String::new();
        let mut tool_use: Option<ToolUseBlock> = None;

        if let Some(blocks) = body["content"].as_array() {
            for block in blocks {
                match block["type"].as_str() {
                    Some("text") => {
                        if let Some(text) = block["text"].as_str() {
                            content.push_str(text);
                        }
                    }
                    Some("tool_use") => {
                        tool_use = Some(ToolUseBlock {
                            id: block["id"].as_str().unwrap_or("").to_string(),
                            name: block["name"].as_str().unwrap_or("").to_string(),
                            input: block["input"].clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        let cost_usd = pricing::calculate_cost(&self.pricing, model, tokens_in, tokens_out);

        Ok(ChatResponse {
            content,
            tokens_in,
            tokens_out,
            cost_usd,
            model: model.to_string(),
            stop_reason,
            tool_use,
        })
    }

    // ── Send with retry ─────────────────────────────────────────────

    /// Send the request with retry logic for transient errors.
    async fn send_with_retry(
        &self,
        body: &serde_json::Value,
    ) -> Result<serde_json::Value, LLMError> {
        for attempt in 0..=self.max_retries {
            let (status, resp_body) = self.sender.send(body).await?;

            match status {
                200 => return Ok(resp_body),
                400 => return Err(LLMError::InvalidRequest(resp_body.to_string())),
                401 | 403 => return Err(LLMError::AuthError(resp_body.to_string())),
                429 | 500 | 502 | 503 | 529 => {
                    if attempt >= self.max_retries {
                        if status == 429 {
                            return Err(LLMError::RateLimited(0));
                        }
                        return Err(LLMError::Provider(format!("HTTP {status}: {}", resp_body)));
                    }
                    let base = self.retry_base_delay_ms * 2u64.pow(attempt);
                    // Simple jitter from system time nanos — avoids rand dependency.
                    let nanos = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.subsec_nanos() as u64)
                        .unwrap_or(0);
                    let jitter = nanos % self.retry_base_delay_ms;
                    tokio::time::sleep(std::time::Duration::from_millis(base + jitter)).await;
                }
                _ => return Err(LLMError::Provider(format!("HTTP {status}: {}", resp_body))),
            }
        }

        Err(LLMError::Provider("max retries exceeded".into()))
    }
}

#[async_trait]
impl LLMProvider for AnthropicProvider {
    fn name(&self) -> &str {
        "anthropic"
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LLMError> {
        let model = if req.model.is_empty() {
            self.default_model.clone()
        } else {
            req.model.clone()
        };
        let body = self.build_request_body(&req);
        let resp_body = self.send_with_retry(&body).await?;
        self.parse_response(&resp_body, &model)
    }
}

// ── Tests ───────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ChatMessage;
    use aaf_contracts::tool::{ToolDefinition, ToolResultBlock};

    fn provider_with(body: serde_json::Value) -> AnthropicProvider {
        AnthropicProvider::new(Box::new(FixedSender::ok(body)))
    }

    fn text_response(text: &str, input_tokens: u64, output_tokens: u64) -> serde_json::Value {
        serde_json::json!({
            "content": [{"type": "text", "text": text}],
            "usage": {"input_tokens": input_tokens, "output_tokens": output_tokens},
            "stop_reason": "end_turn",
        })
    }

    // ── Request building ────────────────────────────────────────────

    #[test]
    fn build_request_simple() {
        let p = provider_with(serde_json::json!({}));
        let req = ChatRequest {
            model: "claude-sonnet-4-6-20250514".into(),
            max_output_tokens: 100,
            messages: vec![
                ChatMessage::system("You are helpful."),
                ChatMessage::user("Hello"),
            ],
            ..Default::default()
        };
        let body = p.build_request_body(&req);

        assert_eq!(body["model"], "claude-sonnet-4-6-20250514");
        assert_eq!(body["max_tokens"], 100);
        assert_eq!(body["system"], "You are helpful.");
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 1);
        assert_eq!(msgs[0]["role"], "user");
        assert_eq!(msgs[0]["content"], "Hello");
    }

    #[test]
    fn build_request_with_tools() {
        let p = provider_with(serde_json::json!({}));
        let req = ChatRequest {
            messages: vec![ChatMessage::user("check stock")],
            tools: vec![ToolDefinition {
                name: "check_stock".into(),
                description: "Check availability".into(),
                input_schema: serde_json::json!({"type": "object"}),
                ..Default::default()
            }],
            tool_choice: ToolChoice::Auto,
            ..Default::default()
        };
        let body = p.build_request_body(&req);

        let tools = body["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 1);
        assert_eq!(tools[0]["name"], "check_stock");
        assert_eq!(body["tool_choice"]["type"], "auto");
    }

    #[test]
    fn build_request_tool_choice_specific() {
        let p = provider_with(serde_json::json!({}));
        let req = ChatRequest {
            messages: vec![ChatMessage::user("x")],
            tools: vec![ToolDefinition {
                name: "my_tool".into(),
                description: "d".into(),
                input_schema: serde_json::json!({}),
                ..Default::default()
            }],
            tool_choice: ToolChoice::Specific("my_tool".into()),
            ..Default::default()
        };
        let body = p.build_request_body(&req);
        assert_eq!(body["tool_choice"]["type"], "tool");
        assert_eq!(body["tool_choice"]["name"], "my_tool");
    }

    #[test]
    fn build_request_with_tool_use_and_result() {
        let p = provider_with(serde_json::json!({}));
        let req = ChatRequest {
            messages: vec![
                ChatMessage::user("check stock"),
                ChatMessage::tool_use_msg(ToolUseBlock {
                    id: "tc-1".into(),
                    name: "check_stock".into(),
                    input: serde_json::json!({"sku": "SKU-42"}),
                }),
                ChatMessage::tool_result_msg(ToolResultBlock {
                    tool_use_id: "tc-1".into(),
                    content: "42 available".into(),
                    is_error: false,
                }),
            ],
            ..Default::default()
        };
        let body = p.build_request_body(&req);
        let msgs = body["messages"].as_array().unwrap();
        assert_eq!(msgs.len(), 3);

        // Tool use → assistant with content block
        assert_eq!(msgs[1]["role"], "assistant");
        assert_eq!(msgs[1]["content"][0]["type"], "tool_use");
        assert_eq!(msgs[1]["content"][0]["name"], "check_stock");

        // Tool result → user with content block
        assert_eq!(msgs[2]["role"], "user");
        assert_eq!(msgs[2]["content"][0]["type"], "tool_result");
        assert_eq!(msgs[2]["content"][0]["tool_use_id"], "tc-1");
    }

    // ── Response parsing ────────────────────────────────────────────

    #[test]
    fn parse_response_text() {
        let p = provider_with(serde_json::json!({}));
        let body = text_response("Hello, world!", 10, 5);
        let resp = p
            .parse_response(&body, "claude-sonnet-4-6-20250514")
            .unwrap();
        assert_eq!(resp.content, "Hello, world!");
        assert_eq!(resp.tokens_in, 10);
        assert_eq!(resp.tokens_out, 5);
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert!(resp.tool_use.is_none());
        assert!(resp.cost_usd > 0.0);
    }

    #[test]
    fn parse_response_tool_use() {
        let p = provider_with(serde_json::json!({}));
        let body = serde_json::json!({
            "content": [
                {"type": "text", "text": "Let me check."},
                {"type": "tool_use", "id": "toolu_01", "name": "check_stock", "input": {"sku": "SKU-42"}},
            ],
            "usage": {"input_tokens": 20, "output_tokens": 15},
            "stop_reason": "tool_use",
        });
        let resp = p
            .parse_response(&body, "claude-sonnet-4-6-20250514")
            .unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        let tu = resp.tool_use.unwrap();
        assert_eq!(tu.id, "toolu_01");
        assert_eq!(tu.name, "check_stock");
    }

    #[test]
    fn parse_response_max_tokens() {
        let p = provider_with(serde_json::json!({}));
        let body = serde_json::json!({
            "content": [{"type": "text", "text": "truncated..."}],
            "usage": {"input_tokens": 10, "output_tokens": 100},
            "stop_reason": "max_tokens",
        });
        let resp = p
            .parse_response(&body, "claude-sonnet-4-6-20250514")
            .unwrap();
        assert_eq!(resp.stop_reason, StopReason::MaxTokens);
    }

    #[test]
    fn parse_response_cost_uses_pricing() {
        let p = provider_with(serde_json::json!({}));
        let body = text_response("hi", 1000, 500);
        let resp = p
            .parse_response(&body, "claude-sonnet-4-6-20250514")
            .unwrap();
        // Sonnet: (1000*3.0 + 500*15.0) / 1_000_000 = 0.0105
        assert!((resp.cost_usd - 0.0105).abs() < 1e-9);
    }

    // ── End-to-end with FixedSender ─────────────────────────────────

    #[tokio::test]
    async fn chat_with_fixed_sender() {
        let resp_body = text_response("Hi there!", 15, 8);
        let p = provider_with(resp_body);
        let req = ChatRequest {
            model: "claude-sonnet-4-6-20250514".into(),
            max_output_tokens: 100,
            messages: vec![ChatMessage::user("Hello")],
            ..Default::default()
        };
        let resp = p.chat(req).await.unwrap();
        assert_eq!(resp.content, "Hi there!");
        assert_eq!(resp.tokens_in, 15);
        assert_eq!(resp.tokens_out, 8);
        assert_eq!(resp.model, "claude-sonnet-4-6-20250514");
    }

    #[tokio::test]
    async fn chat_handles_auth_error() {
        let p = AnthropicProvider::new(Box::new(FixedSender::new(
            401,
            serde_json::json!({"error": "invalid_api_key"}),
        )))
        .with_max_retries(0);

        let req = ChatRequest {
            messages: vec![ChatMessage::user("hi")],
            ..Default::default()
        };
        let err = p.chat(req).await.unwrap_err();
        assert!(matches!(err, LLMError::AuthError(_)));
    }

    #[tokio::test]
    async fn chat_handles_invalid_request() {
        let p = AnthropicProvider::new(Box::new(FixedSender::new(
            400,
            serde_json::json!({"error": "bad request"}),
        )))
        .with_max_retries(0);

        let req = ChatRequest {
            messages: vec![ChatMessage::user("hi")],
            ..Default::default()
        };
        let err = p.chat(req).await.unwrap_err();
        assert!(matches!(err, LLMError::InvalidRequest(_)));
    }

    #[tokio::test]
    async fn chat_with_tool_use_response() {
        let resp_body = serde_json::json!({
            "content": [
                {"type": "tool_use", "id": "toolu_01", "name": "search", "input": {"q": "rust"}},
            ],
            "usage": {"input_tokens": 30, "output_tokens": 20},
            "stop_reason": "tool_use",
        });
        let p = provider_with(resp_body);
        let req = ChatRequest {
            messages: vec![ChatMessage::user("search for rust")],
            tools: vec![ToolDefinition {
                name: "search".into(),
                description: "Search".into(),
                input_schema: serde_json::json!({}),
                ..Default::default()
            }],
            ..Default::default()
        };
        let resp = p.chat(req).await.unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        assert_eq!(resp.tool_use.unwrap().name, "search");
    }
}
