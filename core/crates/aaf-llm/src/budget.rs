//! Per-call budget enforcement.
//!
//! Wraps an [`crate::provider::LLMProvider`] and decrements an
//! [`aaf_contracts::BudgetContract`] in place after each call. Calls
//! that would push usage past zero raise [`CallBudgetError::Exhausted`]
//! before invoking the provider.

use crate::provider::{ChatRequest, ChatResponse, LLMError, LLMProvider};
use aaf_contracts::BudgetContract;
use parking_lot::Mutex;
use std::sync::Arc;
use thiserror::Error;

/// Errors raised by [`CallBudget::call`].
#[derive(Debug, Error)]
pub enum CallBudgetError {
    /// The remaining budget cannot cover the request.
    #[error("call budget exhausted")]
    Exhausted,

    /// Provider error.
    #[error("provider: {0}")]
    Provider(#[from] LLMError),
}

/// Wraps a provider with mutable budget tracking.
pub struct CallBudget {
    provider: Arc<dyn LLMProvider>,
    remaining: Arc<Mutex<BudgetContract>>,
}

impl CallBudget {
    /// Construct.
    pub fn new(provider: Arc<dyn LLMProvider>, budget: BudgetContract) -> Self {
        Self {
            provider,
            remaining: Arc::new(Mutex::new(budget)),
        }
    }

    /// Snapshot of remaining budget.
    pub fn remaining(&self) -> BudgetContract {
        *self.remaining.lock()
    }

    /// Run a chat call. Returns `Exhausted` if the call cannot be made.
    pub async fn call(&self, req: ChatRequest) -> Result<ChatResponse, CallBudgetError> {
        // Pre-check on tokens (we cannot pre-know cost without calling).
        {
            let r = self.remaining.lock();
            if r.max_tokens == 0 || r.max_cost_usd <= 0.0 {
                return Err(CallBudgetError::Exhausted);
            }
        }
        let resp = self.provider.chat(req).await?;
        let mut r = self.remaining.lock();
        let used_tokens = resp.tokens_in as u64 + resp.tokens_out as u64;
        r.max_tokens = r.max_tokens.saturating_sub(used_tokens);
        r.max_cost_usd = (r.max_cost_usd - resp.cost_usd).max(0.0);
        Ok(resp)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;
    use crate::provider::{ChatMessage, ChatRequest};

    fn small() -> BudgetContract {
        BudgetContract {
            max_tokens: 100,
            max_cost_usd: 0.10,
            max_latency_ms: 1000,
        }
    }

    #[tokio::test]
    async fn first_call_succeeds_then_decrements() {
        let cb = CallBudget::new(Arc::new(MockProvider::new("m", 0.001)), small());
        cb.call(ChatRequest {
            model: "x".into(),
            temperature: 0.0,
            max_output_tokens: 10,
            messages: vec![ChatMessage::user("hi")],
            ..Default::default()
        })
        .await
        .unwrap();
        assert!(cb.remaining().max_tokens < 100);
    }

    #[tokio::test]
    async fn exhausted_budget_blocks_call() {
        let cb = CallBudget::new(
            Arc::new(MockProvider::new("m", 0.001)),
            BudgetContract {
                max_tokens: 0,
                max_cost_usd: 0.0,
                max_latency_ms: 0,
            },
        );
        let err = cb
            .call(ChatRequest {
                model: "x".into(),
                temperature: 0.0,
                max_output_tokens: 10,
                messages: vec![ChatMessage::user("hi")],
                ..Default::default()
            })
            .await
            .unwrap_err();
        assert!(matches!(err, CallBudgetError::Exhausted));
    }
}
