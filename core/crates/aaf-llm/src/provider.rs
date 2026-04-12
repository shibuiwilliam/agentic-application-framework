//! Provider trait + request/response shapes.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Chat role.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Role {
    /// System / preamble.
    System,
    /// User input.
    User,
    /// Assistant reply.
    Assistant,
}

/// One chat message.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatMessage {
    /// Role.
    pub role: Role,
    /// Content.
    pub content: String,
}

impl ChatMessage {
    /// Helper for system messages.
    pub fn system(content: impl Into<String>) -> Self {
        Self {
            role: Role::System,
            content: content.into(),
        }
    }
    /// Helper for user messages.
    pub fn user(content: impl Into<String>) -> Self {
        Self {
            role: Role::User,
            content: content.into(),
        }
    }
}

/// Chat completion request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatRequest {
    /// Model identifier.
    pub model: String,
    /// Sampling temperature in `[0,1]`.
    pub temperature: f32,
    /// Hard cap on output tokens for the call.
    pub max_output_tokens: u32,
    /// Conversation messages.
    pub messages: Vec<ChatMessage>,
}

/// Chat completion response.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ChatResponse {
    /// Generated content.
    pub content: String,
    /// Tokens consumed on the input side.
    pub tokens_in: u32,
    /// Tokens generated on the output side.
    pub tokens_out: u32,
    /// USD cost charged by the provider.
    pub cost_usd: f64,
    /// Provider model id.
    pub model: String,
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
}

/// Pluggable LLM provider.
#[async_trait]
pub trait LLMProvider: Send + Sync {
    /// Identifier shown in traces.
    fn name(&self) -> &str;

    /// Run a chat completion.
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LLMError>;
}
