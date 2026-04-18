//! AAF LLM abstraction.
//!
//! Provides a model-agnostic [`provider::LLMProvider`] trait, a
//! deterministic [`mock::MockProvider`] for tests, a value-based
//! [`router::ValueRouter`], and a per-call [`budget::CallBudget`]
//! enforcer that decrements the intent budget on every call (Rule 8).
//!
//! ## E4 Slice A — Tool support
//!
//! The provider layer now supports tool definitions in `ChatRequest`,
//! tool-use / tool-result message roles, and `StopReason` reporting
//! in `ChatResponse`. See [`provider`] module for details.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod anthropic;
pub mod budget;
pub mod mock;
pub mod pricing;
pub mod provider;
pub mod router;

pub use anthropic::{AnthropicProvider, FixedSender, HttpSender};
pub use budget::{CallBudget, CallBudgetError};
pub use mock::{MockProvider, MultiTurnMockProvider};
pub use pricing::{anthropic_pricing, calculate_cost, ModelPricing};
pub use provider::{
    ChatEvent, ChatMessage, ChatRequest, ChatResponse, LLMError, LLMProvider, ProviderMetrics, Role,
};
pub use router::{
    DefaultRoutingPolicy, LearnedRoutingPolicy, RoutingPolicy, RoutingTier, ValueRouter,
};
