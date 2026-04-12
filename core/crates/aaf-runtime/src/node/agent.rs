//! Agent node — calls an [`aaf_llm::LLMProvider`] inside a guarded
//! envelope.

use super::{Node, NodeKind, NodeOutput};
use crate::error::RuntimeError;
use aaf_contracts::{IntentEnvelope, NodeId, SideEffect};
use aaf_llm::{ChatMessage, ChatRequest, LLMProvider};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

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
}

impl AgentNode {
    /// Construct.
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
        }
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
        let req = ChatRequest {
            model: self.model.clone(),
            temperature: 0.2,
            max_output_tokens: self.max_output_tokens,
            messages: vec![
                ChatMessage::system(&self.system),
                ChatMessage::user(&intent.goal),
            ],
        };
        let resp = self
            .provider
            .chat(req)
            .await
            .map_err(|e| RuntimeError::Node(e.to_string()))?;
        Ok(NodeOutput {
            data: serde_json::json!({"content": resp.content}),
            tokens: resp.tokens_in as u64 + resp.tokens_out as u64,
            cost_usd: resp.cost_usd,
            duration_ms: started.elapsed().as_millis() as u64,
            model: Some(resp.model),
        })
    }
}
