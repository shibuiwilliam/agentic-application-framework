//! Deterministic mock provider for tests.
//!
//! When the request carries `tools`, the mock returns a tool-use
//! response targeting the first tool. Otherwise it echoes the last
//! user message. Token and cost calculations are deterministic.

use aaf_contracts::tool::{StopReason, ToolUseBlock};
use async_trait::async_trait;

use crate::provider::{ChatRequest, ChatResponse, LLMError, LLMProvider};

/// Mock provider that echoes the last user message back and emits
/// deterministic token / cost numbers.
pub struct MockProvider {
    name: String,
    /// USD per 1k tokens.
    pub cost_per_1k: f64,
}

impl MockProvider {
    /// Construct a mock with the given name and cost.
    pub fn new(name: impl Into<String>, cost_per_1k: f64) -> Self {
        Self {
            name: name.into(),
            cost_per_1k,
        }
    }
}

#[async_trait]
impl LLMProvider for MockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LLMError> {
        let last_user = req
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, crate::provider::Role::User))
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let tokens_in: u32 = req
            .messages
            .iter()
            .map(|m| (m.content.chars().count() / 4 + 1) as u32)
            .sum();
        let tokens_out: u32 = (last_user.chars().count() / 4 + 1) as u32;
        let cost_usd = self.cost_per_1k * ((tokens_in + tokens_out) as f64 / 1000.0);

        // When tools are provided, simulate a tool-use response
        // targeting the first tool (E4 Slice A).
        if let Some(first_tool) = req.tools.first() {
            return Ok(ChatResponse {
                content: String::new(),
                tokens_in,
                tokens_out,
                cost_usd,
                model: req.model,
                stop_reason: StopReason::ToolUse,
                tool_use: Some(ToolUseBlock {
                    id: "mock-tc-1".into(),
                    name: first_tool.name.clone(),
                    input: serde_json::json!({}),
                }),
            });
        }

        Ok(ChatResponse {
            content: format!("[mock:{}] {}", self.name, last_user),
            tokens_in,
            tokens_out,
            cost_usd,
            model: req.model,
            stop_reason: StopReason::EndTurn,
            tool_use: None,
        })
    }
}

/// Mock provider for multi-turn tool-use testing (E4 Slice B).
///
/// Returns `StopReason::ToolUse` for the first `tool_calls_before_end`
/// calls that carry tools, then returns `StopReason::EndTurn`. This
/// lets tests exercise the multi-turn agentic loop with a controlled
/// number of iterations.
pub struct MultiTurnMockProvider {
    name: String,
    /// USD per 1k tokens.
    pub cost_per_1k: f64,
    /// Number of tool-use responses before switching to `EndTurn`.
    pub tool_calls_before_end: u32,
    call_count: std::sync::Arc<parking_lot::Mutex<u32>>,
}

impl MultiTurnMockProvider {
    /// Construct a multi-turn mock.
    ///
    /// `tool_calls_before_end` controls how many tool-use iterations
    /// occur before the mock produces a final `EndTurn` response.
    pub fn new(name: impl Into<String>, cost_per_1k: f64, tool_calls_before_end: u32) -> Self {
        Self {
            name: name.into(),
            cost_per_1k,
            tool_calls_before_end,
            call_count: std::sync::Arc::new(parking_lot::Mutex::new(0)),
        }
    }

    /// How many calls have been made so far.
    pub fn call_count(&self) -> u32 {
        *self.call_count.lock()
    }
}

#[async_trait]
impl LLMProvider for MultiTurnMockProvider {
    fn name(&self) -> &str {
        &self.name
    }

    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LLMError> {
        let mut count = self.call_count.lock();
        *count += 1;
        let current = *count;
        drop(count);

        let last_user = req
            .messages
            .iter()
            .rev()
            .find(|m| matches!(m.role, crate::provider::Role::User))
            .map(|m| m.content.clone())
            .unwrap_or_default();
        let tokens_in: u32 = req
            .messages
            .iter()
            .map(|m| (m.content.chars().count() / 4 + 1) as u32)
            .sum();
        let tokens_out: u32 = (last_user.chars().count() / 4 + 1) as u32;
        let cost_usd = self.cost_per_1k * ((tokens_in + tokens_out) as f64 / 1000.0);

        // Return ToolUse for the first N calls with tools, then EndTurn.
        if !req.tools.is_empty() && current <= self.tool_calls_before_end {
            let tool = &req.tools[(current as usize - 1) % req.tools.len()];
            return Ok(ChatResponse {
                content: String::new(),
                tokens_in,
                tokens_out,
                cost_usd,
                model: req.model,
                stop_reason: StopReason::ToolUse,
                tool_use: Some(ToolUseBlock {
                    id: format!("mock-tc-{current}"),
                    name: tool.name.clone(),
                    input: serde_json::json!({ "turn": current }),
                }),
            });
        }

        Ok(ChatResponse {
            content: format!("[mock:{}] final answer after {current} calls", self.name),
            tokens_in,
            tokens_out,
            cost_usd,
            model: req.model,
            stop_reason: StopReason::EndTurn,
            tool_use: None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ChatMessage;
    use aaf_contracts::tool::ToolDefinition;
    use aaf_contracts::{CapabilityId, SideEffect};

    #[tokio::test]
    async fn echoes_last_user_message() {
        let p = MockProvider::new("test", 0.001);
        let resp = p
            .chat(ChatRequest {
                model: "mock-1".into(),
                temperature: 0.0,
                max_output_tokens: 100,
                messages: vec![
                    ChatMessage::system("you are helpful"),
                    ChatMessage::user("hello"),
                ],
                ..Default::default()
            })
            .await
            .unwrap();
        assert!(resp.content.ends_with("hello"));
        assert!(resp.cost_usd > 0.0);
        assert_eq!(resp.stop_reason, StopReason::EndTurn);
        assert!(resp.tool_use.is_none());
    }

    #[tokio::test]
    async fn returns_tool_use_when_tools_provided() {
        let p = MockProvider::new("test", 0.001);
        let resp = p
            .chat(ChatRequest {
                model: "mock-1".into(),
                temperature: 0.0,
                max_output_tokens: 100,
                messages: vec![ChatMessage::user("check stock for SKU-42")],
                tools: vec![ToolDefinition {
                    name: "check_stock".into(),
                    description: "Check product availability".into(),
                    input_schema: serde_json::json!({"type": "object"}),
                    output_schema: serde_json::json!({}),
                    side_effect: SideEffect::Read,
                    capability_id: CapabilityId::from_raw("cap-stock"),
                }],
                tool_choice: Default::default(),
            })
            .await
            .unwrap();
        assert_eq!(resp.stop_reason, StopReason::ToolUse);
        let tu = resp.tool_use.unwrap();
        assert_eq!(tu.name, "check_stock");
        assert_eq!(tu.id, "mock-tc-1");
    }

    #[tokio::test]
    async fn multi_turn_mock_returns_tool_use_then_end_turn() {
        let tools = vec![ToolDefinition {
            name: "search".into(),
            description: "Search".into(),
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            side_effect: SideEffect::Read,
            capability_id: CapabilityId::from_raw("cap-search"),
        }];
        let p = MultiTurnMockProvider::new("multi", 0.001, 2);

        // Call 1: should return ToolUse
        let r1 = p
            .chat(ChatRequest {
                model: "m".into(),
                messages: vec![ChatMessage::user("hi")],
                tools: tools.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(r1.stop_reason, StopReason::ToolUse);
        assert_eq!(r1.tool_use.as_ref().unwrap().id, "mock-tc-1");

        // Call 2: should return ToolUse
        let r2 = p
            .chat(ChatRequest {
                model: "m".into(),
                messages: vec![ChatMessage::user("hi")],
                tools: tools.clone(),
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(r2.stop_reason, StopReason::ToolUse);
        assert_eq!(r2.tool_use.as_ref().unwrap().id, "mock-tc-2");

        // Call 3: should return EndTurn (exceeded tool_calls_before_end=2)
        let r3 = p
            .chat(ChatRequest {
                model: "m".into(),
                messages: vec![ChatMessage::user("hi")],
                tools,
                ..Default::default()
            })
            .await
            .unwrap();
        assert_eq!(r3.stop_reason, StopReason::EndTurn);
        assert!(r3.content.contains("final answer"));
        assert_eq!(p.call_count(), 3);
    }
}
