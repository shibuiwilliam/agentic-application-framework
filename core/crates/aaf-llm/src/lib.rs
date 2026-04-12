//! AAF LLM abstraction.
//!
//! Provides a model-agnostic [`provider::LLMProvider`] trait, a
//! deterministic [`mock::MockProvider`] for tests, a value-based
//! [`router::ValueRouter`], and a per-call [`budget::CallBudget`]
//! enforcer that decrements the intent budget on every call (Rule 8).

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod budget;
pub mod mock;
pub mod provider;
pub mod router;

pub use budget::{CallBudget, CallBudgetError};
pub use mock::MockProvider;
pub use provider::{ChatMessage, ChatRequest, ChatResponse, LLMError, LLMProvider, Role};
pub use router::{
    DefaultRoutingPolicy, LearnedRoutingPolicy, RoutingPolicy, RoutingTier, ValueRouter,
};
