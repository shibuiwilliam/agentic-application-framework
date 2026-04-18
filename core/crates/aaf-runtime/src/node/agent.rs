//! Agent node — calls an [`aaf_llm::LLMProvider`] inside a guarded
//! envelope.
//!
//! ## E4 Slice B — Multi-turn agentic tool loop
//!
//! When `tools` is non-empty, the agent sends tool definitions to the
//! LLM and enters a bounded loop (Rule 27). Each iteration:
//!
//! 1. Call LLM with the accumulated message history and available tools
//! 2. If the LLM responds with `StopReason::ToolUse`:
//!    - Execute the tool via [`ToolExecutor`]
//!    - Append tool-use and tool-result messages to the conversation
//!    - Continue the loop
//! 3. If the LLM responds with any other stop reason → return final answer
//!
//! The loop is bounded by `max_tool_calls` (default 10, Rule 27) and
//! terminates with partial results when the bound is reached. Every
//! tool call is recorded in the output for trace integration (Rule 12).

use super::{Node, NodeKind, NodeOutput};
use crate::error::RuntimeError;
use aaf_contracts::tool::{StopReason, ToolDefinition, ToolResultBlock};
use aaf_contracts::{IntentEnvelope, NodeId, SideEffect};
use aaf_llm::{ChatMessage, ChatRequest, LLMProvider};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;

/// Default maximum tool calls per agent node (Rule 27).
pub const DEFAULT_MAX_TOOL_CALLS: u32 = 10;

/// Trait for executing tool calls during agent inference (Rule 26).
///
/// Implementations are responsible for invoking the backing capability
/// and returning the result as a string. Policy checks should be
/// performed by the caller before delegating to this trait.
#[async_trait]
pub trait ToolExecutor: Send + Sync {
    /// Execute a tool call and return the result.
    async fn execute(&self, name: &str, input: serde_json::Value) -> Result<String, RuntimeError>;
}

/// Record of a single tool invocation within the agentic loop.
///
/// Stored in the `NodeOutput.data["tool_calls"]` array for trace
/// and observability (Rule 12).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ToolCallRecord {
    /// Tool name.
    pub name: String,
    /// Input passed to the tool.
    pub input: serde_json::Value,
    /// Output returned by the tool.
    pub output: String,
    /// Execution time in ms.
    pub duration_ms: u64,
}

/// Agent node.
pub struct AgentNode {
    id: NodeId,
    /// LLM provider used for this agent.
    pub provider: Arc<dyn LLMProvider>,
    /// System prompt.
    pub system: String,
    /// Model identifier.
    pub model: String,
    /// Per-call max output tokens.
    pub max_output_tokens: u32,
    /// Tool definitions available to this agent (E4, Rule 25).
    pub tools: Vec<ToolDefinition>,
    /// Executor for tool calls (E4, Rule 26).
    pub tool_executor: Option<Arc<dyn ToolExecutor>>,
    /// Maximum tool calls before forced termination (Rule 27).
    pub max_tool_calls: u32,
}

impl AgentNode {
    /// Construct a basic agent node without tools.
    pub fn new(
        id: NodeId,
        provider: Arc<dyn LLMProvider>,
        system: impl Into<String>,
        model: impl Into<String>,
        max_output_tokens: u32,
    ) -> Self {
        Self {
            id,
            provider,
            system: system.into(),
            model: model.into(),
            max_output_tokens,
            tools: Vec::new(),
            tool_executor: None,
            max_tool_calls: DEFAULT_MAX_TOOL_CALLS,
        }
    }

    /// Attach tool definitions and an executor (builder pattern).
    pub fn with_tools(
        mut self,
        tools: Vec<ToolDefinition>,
        executor: Arc<dyn ToolExecutor>,
    ) -> Self {
        self.tools = tools;
        self.tool_executor = Some(executor);
        self
    }

    /// Set the maximum number of tool calls (Rule 27).
    pub fn with_max_tool_calls(mut self, max: u32) -> Self {
        self.max_tool_calls = max;
        self
    }
}

#[async_trait]
impl Node for AgentNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Agent
    }
    fn side_effect(&self) -> SideEffect {
        SideEffect::None
    }

    async fn run(
        &self,
        intent: &IntentEnvelope,
        _prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        let started = std::time::Instant::now();

        // Build the initial conversation.
        let mut messages = vec![
            ChatMessage::system(&self.system),
            ChatMessage::user(&intent.goal),
        ];

        let has_tools = !self.tools.is_empty() && self.tool_executor.is_some();
        let mut total_tokens: u64 = 0;
        let mut total_cost: f64 = 0.0;
        let mut tool_calls: Vec<ToolCallRecord> = Vec::new();
        let mut final_content = String::new();
        let mut final_stop_reason = StopReason::EndTurn;
        let mut last_model = String::new();

        // Bounded agentic loop (Rule 27).
        for turn in 0..=self.max_tool_calls {
            let req = ChatRequest {
                model: self.model.clone(),
                temperature: 0.2,
                max_output_tokens: self.max_output_tokens,
                messages: messages.clone(),
                tools: if has_tools {
                    self.tools.clone()
                } else {
                    vec![]
                },
                ..Default::default()
            };

            let resp = self
                .provider
                .chat(req)
                .await
                .map_err(|e| RuntimeError::Node(e.to_string()))?;

            total_tokens += resp.tokens_in as u64 + resp.tokens_out as u64;
            total_cost += resp.cost_usd;
            last_model.clone_from(&resp.model);
            final_stop_reason = resp.stop_reason;

            // If the LLM does NOT want to call a tool, we're done.
            if resp.stop_reason != StopReason::ToolUse {
                final_content = resp.content;
                break;
            }

            // If the LLM wants to call a tool but we're at the limit, stop.
            if turn >= self.max_tool_calls {
                final_content = resp.content;
                final_stop_reason = StopReason::BudgetExhausted;
                break;
            }

            // Execute the tool if we have a tool_use block and an executor.
            if let (Some(tool_use), Some(executor)) = (&resp.tool_use, &self.tool_executor) {
                let tool_started = std::time::Instant::now();
                let tool_result = executor
                    .execute(&tool_use.name, tool_use.input.clone())
                    .await?;
                let tool_duration = tool_started.elapsed().as_millis() as u64;

                // Record the tool call for trace output.
                tool_calls.push(ToolCallRecord {
                    name: tool_use.name.clone(),
                    input: tool_use.input.clone(),
                    output: tool_result.clone(),
                    duration_ms: tool_duration,
                });

                // Append tool-use and tool-result to the conversation
                // so the LLM can see what happened.
                messages.push(ChatMessage::tool_use_msg(tool_use.clone()));
                messages.push(ChatMessage::tool_result_msg(ToolResultBlock {
                    tool_use_id: tool_use.id.clone(),
                    content: tool_result,
                    is_error: false,
                }));
            } else {
                // No tool_use block or no executor — stop the loop.
                final_content = resp.content;
                break;
            }
        }

        Ok(NodeOutput {
            data: serde_json::json!({
                "content": final_content,
                "stop_reason": format!("{:?}", final_stop_reason),
                "tool_calls": tool_calls,
            }),
            tokens: total_tokens,
            cost_usd: total_cost,
            duration_ms: started.elapsed().as_millis() as u64,
            model: Some(last_model),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::tool::ToolDefinition;
    use aaf_contracts::{
        BudgetContract, CapabilityId, IntentId, IntentType, Requester, RiskTier, TraceId,
    };
    use aaf_llm::{MockProvider, MultiTurnMockProvider};
    use chrono::Utc;

    fn test_intent() -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::AnalyticalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec![],
                tenant: None,
            },
            goal: "check stock".into(),
            domain: "warehouse".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 1000,
                max_cost_usd: 1.0,
                max_latency_ms: 5000,
            },
            deadline: None,
            risk_tier: RiskTier::Read,
            approval_policy: "none".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    fn test_tools() -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "check_stock".into(),
            description: "Check availability".into(),
            input_schema: serde_json::json!({"type": "object"}),
            output_schema: serde_json::json!({}),
            side_effect: SideEffect::Read,
            capability_id: CapabilityId::from_raw("cap-stock"),
        }]
    }

    /// Mock tool executor that returns a fixed result.
    struct FixedToolExecutor(String);

    #[async_trait]
    impl ToolExecutor for FixedToolExecutor {
        async fn execute(
            &self,
            _name: &str,
            _input: serde_json::Value,
        ) -> Result<String, RuntimeError> {
            Ok(self.0.clone())
        }
    }

    #[tokio::test]
    async fn agent_without_tools_echoes() {
        let provider: Arc<dyn LLMProvider> = Arc::new(MockProvider::new("test", 0.001));
        let node = AgentNode::new(NodeId::new(), provider, "system prompt", "mock-model", 100);
        let output = node.run(&test_intent(), &HashMap::new()).await.unwrap();
        let content = output.data["content"].as_str().unwrap();
        assert!(content.contains("check stock"));
        assert_eq!(output.data["stop_reason"].as_str().unwrap(), "EndTurn");
        assert!(output.data["tool_calls"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn agent_with_tools_does_single_round_trip() {
        // MultiTurnMockProvider with 1 tool call, then EndTurn.
        let provider: Arc<dyn LLMProvider> = Arc::new(MultiTurnMockProvider::new("test", 0.001, 1));
        let executor: Arc<dyn ToolExecutor> =
            Arc::new(FixedToolExecutor("42 units available".into()));
        let node = AgentNode::new(NodeId::new(), provider, "system prompt", "mock-model", 100)
            .with_tools(test_tools(), executor);

        let output = node.run(&test_intent(), &HashMap::new()).await.unwrap();

        assert!(output.tokens > 0);
        assert!(output.cost_usd > 0.0);
        let calls = output.data["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0]["name"].as_str().unwrap(), "check_stock");
        assert_eq!(calls[0]["output"].as_str().unwrap(), "42 units available");
    }

    #[tokio::test]
    async fn multi_turn_loop_calls_tool_multiple_times() {
        // MultiTurnMockProvider returns ToolUse 3 times, then EndTurn.
        let provider: Arc<dyn LLMProvider> =
            Arc::new(MultiTurnMockProvider::new("multi", 0.001, 3));
        let executor: Arc<dyn ToolExecutor> = Arc::new(FixedToolExecutor("result".into()));

        let node = AgentNode::new(NodeId::new(), provider, "system prompt", "mock-model", 100)
            .with_tools(test_tools(), executor);

        let output = node.run(&test_intent(), &HashMap::new()).await.unwrap();

        let calls = output.data["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 3, "should have made 3 tool calls");
        assert_eq!(
            output.data["stop_reason"].as_str().unwrap(),
            "EndTurn",
            "should end with EndTurn after all tool calls"
        );
        assert!(output.tokens > 0);
        assert!(output.cost_usd > 0.0);
    }

    #[tokio::test]
    async fn loop_respects_max_tool_calls_bound() {
        // MultiTurnMockProvider would return ToolUse 10 times,
        // but we limit to 2.
        let provider: Arc<dyn LLMProvider> =
            Arc::new(MultiTurnMockProvider::new("multi", 0.001, 10));
        let executor: Arc<dyn ToolExecutor> = Arc::new(FixedToolExecutor("result".into()));

        let node = AgentNode::new(NodeId::new(), provider, "system prompt", "mock-model", 100)
            .with_tools(test_tools(), executor)
            .with_max_tool_calls(2);

        let output = node.run(&test_intent(), &HashMap::new()).await.unwrap();

        let calls = output.data["tool_calls"].as_array().unwrap();
        assert_eq!(calls.len(), 2, "should stop at max_tool_calls=2");
        assert_eq!(
            output.data["stop_reason"].as_str().unwrap(),
            "BudgetExhausted",
            "should report BudgetExhausted when bound reached"
        );
    }
}
