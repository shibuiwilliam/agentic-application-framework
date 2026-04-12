//! Value-based model router.
//!
//! Mirrors PROJECT.md §6.3: tasks are categorised into `high_value` /
//! `standard` / `low_value` and routed to a model registered for that
//! tier. The router is provider-agnostic.
//!
//! ## E1 Slice B — `RoutingPolicy` trait
//!
//! The value router now accepts a pluggable [`RoutingPolicy`] that
//! overrides the fixed tier selection. `aaf-learn::router_tuner`
//! ships a [`LearnedRoutingPolicy`] that adjusts weights per
//! `(intent_type, risk_tier)` based on observed cost/quality.

use crate::provider::LLMProvider;
use aaf_contracts::IntentEnvelope;
use std::collections::HashMap;
use std::sync::Arc;

/// Routing tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RoutingTier {
    /// Customer-facing or high-value.
    High,
    /// Standard internal task.
    Standard,
    /// Low-value / template-able.
    Low,
}

/// Pluggable routing policy (E1 Slice B, Rule 17: every adaptation
/// is reversible). Implementations choose a `RoutingTier` for a
/// given intent; the `ValueRouter` then resolves the tier to a
/// concrete `LLMProvider`.
pub trait RoutingPolicy: Send + Sync {
    /// Choose a tier for this intent.
    fn choose(&self, intent: &IntentEnvelope) -> RoutingTier;
}

/// Default policy: map `RiskTier` → `RoutingTier` mechanically.
#[derive(Debug, Default)]
pub struct DefaultRoutingPolicy;

impl RoutingPolicy for DefaultRoutingPolicy {
    fn choose(&self, intent: &IntentEnvelope) -> RoutingTier {
        use aaf_contracts::RiskTier;
        match intent.risk_tier {
            RiskTier::Governance => RoutingTier::High,
            RiskTier::Write | RiskTier::Delegation => RoutingTier::Standard,
            RiskTier::Read | RiskTier::Advisory => RoutingTier::Low,
        }
    }
}

/// A learned routing policy driven by per-`(intent_type, risk_tier)`
/// weights. Weights are `RoutingTier` values; the tuner in `aaf-learn`
/// updates them from observed outcomes. Rule 17: carry evidence so
/// every weight change is reversible.
#[derive(Debug, Default)]
pub struct LearnedRoutingPolicy {
    /// `(format!("{:?}", intent_type), format!("{:?}", risk_tier))` →
    /// `RoutingTier`. If no entry matches, falls back to the default.
    pub overrides: HashMap<(String, String), RoutingTier>,
}

impl LearnedRoutingPolicy {
    /// Construct empty (delegates everything to the default policy).
    pub fn new() -> Self {
        Self::default()
    }

    /// Set an override for a specific intent_type + risk_tier combo.
    pub fn set(&mut self, intent_type: &str, risk_tier: &str, tier: RoutingTier) {
        self.overrides
            .insert((intent_type.to_string(), risk_tier.to_string()), tier);
    }
}

impl RoutingPolicy for LearnedRoutingPolicy {
    fn choose(&self, intent: &IntentEnvelope) -> RoutingTier {
        let key = (
            format!("{:?}", intent.intent_type),
            format!("{:?}", intent.risk_tier),
        );
        if let Some(tier) = self.overrides.get(&key) {
            return *tier;
        }
        DefaultRoutingPolicy.choose(intent)
    }
}

/// Value-based router.
pub struct ValueRouter {
    providers: HashMap<RoutingTier, Arc<dyn LLMProvider>>,
    policy: Box<dyn RoutingPolicy>,
}

impl Default for ValueRouter {
    fn default() -> Self {
        Self {
            providers: HashMap::new(),
            policy: Box::new(DefaultRoutingPolicy),
        }
    }
}

impl ValueRouter {
    /// Construct an empty router with the default policy.
    pub fn new() -> Self {
        Self::default()
    }

    /// Install a custom routing policy. Returns `self` for chaining.
    pub fn with_policy(mut self, policy: Box<dyn RoutingPolicy>) -> Self {
        self.policy = policy;
        self
    }

    /// Register a provider for a tier.
    pub fn register(&mut self, tier: RoutingTier, provider: Arc<dyn LLMProvider>) {
        self.providers.insert(tier, provider);
    }

    /// Choose a provider for `tier`, falling back to `Standard` then any.
    pub fn choose(&self, tier: RoutingTier) -> Option<Arc<dyn LLMProvider>> {
        self.providers
            .get(&tier)
            .cloned()
            .or_else(|| self.providers.get(&RoutingTier::Standard).cloned())
            .or_else(|| self.providers.values().next().cloned())
    }

    /// Choose a provider for `intent` using the installed policy.
    pub fn choose_for_intent(&self, intent: &IntentEnvelope) -> Option<Arc<dyn LLMProvider>> {
        let tier = self.policy.choose(intent);
        self.choose(tier)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::mock::MockProvider;
    use aaf_contracts::{BudgetContract, IntentId, IntentType, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn intent_with_risk(risk: RiskTier) -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::AnalyticalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec![],
                tenant: None,
            },
            goal: "g".into(),
            domain: "d".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 1.0,
                max_latency_ms: 1000,
            },
            deadline: None,
            risk_tier: risk,
            approval_policy: "none".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    #[test]
    fn falls_back_to_standard_when_high_unset() {
        let mut r = ValueRouter::new();
        let p: Arc<dyn LLMProvider> = Arc::new(MockProvider::new("std", 0.001));
        r.register(RoutingTier::Standard, p);
        assert!(r.choose(RoutingTier::High).is_some());
    }

    #[test]
    fn default_policy_maps_governance_to_high() {
        let mut r = ValueRouter::new();
        r.register(RoutingTier::High, Arc::new(MockProvider::new("hi", 0.01)));
        r.register(RoutingTier::Low, Arc::new(MockProvider::new("lo", 0.001)));

        let provider = r
            .choose_for_intent(&intent_with_risk(RiskTier::Governance))
            .unwrap();
        assert_eq!(provider.name(), "hi");
    }

    #[test]
    fn default_policy_maps_read_to_low() {
        let mut r = ValueRouter::new();
        r.register(RoutingTier::High, Arc::new(MockProvider::new("hi", 0.01)));
        r.register(RoutingTier::Low, Arc::new(MockProvider::new("lo", 0.001)));

        let provider = r
            .choose_for_intent(&intent_with_risk(RiskTier::Read))
            .unwrap();
        assert_eq!(provider.name(), "lo");
    }

    #[test]
    fn learned_policy_overrides_default() {
        let mut learned = LearnedRoutingPolicy::new();
        learned.set("AnalyticalIntent", "Read", RoutingTier::High);

        let mut r = ValueRouter::new().with_policy(Box::new(learned));
        r.register(RoutingTier::High, Arc::new(MockProvider::new("hi", 0.01)));
        r.register(RoutingTier::Low, Arc::new(MockProvider::new("lo", 0.001)));

        // Read intent would normally go to Low, but the learned policy overrides to High.
        let provider = r
            .choose_for_intent(&intent_with_risk(RiskTier::Read))
            .unwrap();
        assert_eq!(provider.name(), "hi");
    }

    #[test]
    fn learned_policy_falls_back_when_no_override() {
        let learned = LearnedRoutingPolicy::new(); // no overrides
        let mut r = ValueRouter::new().with_policy(Box::new(learned));
        r.register(RoutingTier::Low, Arc::new(MockProvider::new("lo", 0.001)));

        let provider = r
            .choose_for_intent(&intent_with_risk(RiskTier::Read))
            .unwrap();
        assert_eq!(provider.name(), "lo");
    }
}
