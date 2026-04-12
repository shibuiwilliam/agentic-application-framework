//! Agent Wrapper for modular monoliths.
//!
//! Wraps a single in-process module: each `@capability`-equivalent is a
//! function pointer registered with [`InProcessWrapper::register`].
//! Calls go through the local guard before reaching the underlying
//! function.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

use aaf_contracts::{
    BudgetContract, IntentEnvelope, IntentId, IntentType, PolicyDecision, Requester, RiskTier,
    TraceId,
};
use aaf_policy::{InputGuard, PolicyEngine};
use chrono::Utc;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Closure type for an in-process capability.
pub type CapabilityFn =
    Arc<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>;

/// Wrapper errors.
#[derive(Debug, Error)]
pub enum WrapperError {
    /// No capability registered under that name.
    #[error("unknown capability: {0}")]
    Unknown(String),
    /// Policy denied the call.
    #[error("policy denied: {0:?}")]
    PolicyDenied(PolicyDecision),
    /// Underlying capability raised an error.
    #[error("capability failed: {0}")]
    CapabilityFailed(String),
}

/// In-process wrapper.
pub struct InProcessWrapper {
    capabilities: Arc<RwLock<HashMap<String, CapabilityFn>>>,
    policy: Arc<PolicyEngine>,
}

impl InProcessWrapper {
    /// Construct.
    pub fn new(policy: Arc<PolicyEngine>) -> Self {
        Self {
            capabilities: Arc::new(RwLock::new(HashMap::new())),
            policy,
        }
    }

    /// Register a capability function.
    pub fn register(&self, name: impl Into<String>, func: CapabilityFn) {
        self.capabilities.write().insert(name.into(), func);
    }

    /// Invoke a capability with an arbitrary payload.
    pub fn invoke(
        &self,
        name: &str,
        payload: serde_json::Value,
    ) -> Result<serde_json::Value, WrapperError> {
        let func = {
            let guard = self.capabilities.read();
            guard
                .get(name)
                .cloned()
                .ok_or_else(|| WrapperError::Unknown(name.to_string()))?
        };
        // Synthesize a minimal envelope so the policy engine can run.
        let intent = IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::TransactionalIntent,
            requester: Requester {
                user_id: "in-process".into(),
                role: "module".into(),
                scopes: vec!["auto-approve".into()],
                tenant: None,
            },
            goal: name.into(),
            domain: "in-process".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 0,
                max_cost_usd: 0.0,
                max_latency_ms: 1_000,
            },
            deadline: None,
            risk_tier: RiskTier::Read,
            approval_policy: "none".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        };
        let dec = InputGuard::new(&self.policy).check(&intent, &payload.to_string());
        if !dec.is_allowed() {
            return Err(WrapperError::PolicyDenied(dec));
        }
        func(payload).map_err(WrapperError::CapabilityFailed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registered_capability_runs() {
        let w = InProcessWrapper::new(Arc::new(PolicyEngine::with_default_rules()));
        w.register(
            "double",
            Arc::new(|p| Ok(serde_json::json!({"out": p["x"].as_i64().unwrap_or(0) * 2}))),
        );
        let r = w.invoke("double", serde_json::json!({"x": 5})).unwrap();
        assert_eq!(r["out"], 10);
    }

    #[test]
    fn unknown_capability_errors() {
        let w = InProcessWrapper::new(Arc::new(PolicyEngine::with_default_rules()));
        let e = w.invoke("nope", serde_json::json!({})).unwrap_err();
        assert!(matches!(e, WrapperError::Unknown(_)));
    }
}
