//! Deterministic mock provider for tests.

use crate::provider::{ChatRequest, ChatResponse, LLMError, LLMProvider};
use async_trait::async_trait;

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
        Ok(ChatResponse {
            content: format!("[mock:{}] {}", self.name, last_user),
            tokens_in,
            tokens_out,
            cost_usd,
            model: req.model,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::provider::ChatMessage;

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
            })
            .await
            .unwrap();
        assert!(resp.content.ends_with("hello"));
        assert!(resp.cost_usd > 0.0);
    }
}
