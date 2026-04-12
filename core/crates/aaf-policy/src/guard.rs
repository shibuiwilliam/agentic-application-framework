//! Three guards every agent must pass through (Rule 7).
//!
//! Each guard is a thin façade over the [`crate::engine::PolicyEngine`]
//! that pre-fills the [`crate::context::PolicyContext`] with the
//! appropriate inputs.

use crate::context::PolicyContext;
use crate::engine::{PolicyEngine, PolicyHook};
use aaf_contracts::{IntentEnvelope, PolicyDecision};

/// Input guard — runs **before** an agent processes a payload.
pub struct InputGuard<'e> {
    engine: &'e PolicyEngine,
}

impl<'e> InputGuard<'e> {
    /// Wrap an engine.
    pub fn new(engine: &'e PolicyEngine) -> Self {
        Self { engine }
    }

    /// Inspect a payload before the agent sees it.
    pub fn check(&self, intent: &IntentEnvelope, payload: &str) -> PolicyDecision {
        let ctx = PolicyContext {
            intent,
            capability: None,
            requester: &intent.requester,
            payload: Some(payload),
            output: None,
            side_effect: None,
            remaining_budget: intent.budget,
            tenant: intent.requester.tenant.as_ref(),
            composed_writes: 0,
            ontology_class_lookup: None,
        };
        self.engine.evaluate(PolicyHook::PreStep, &ctx)
    }
}

/// Output guard — runs **after** an agent emits an output.
pub struct OutputGuard<'e> {
    engine: &'e PolicyEngine,
}

impl<'e> OutputGuard<'e> {
    /// Wrap an engine.
    pub fn new(engine: &'e PolicyEngine) -> Self {
        Self { engine }
    }

    /// Inspect an output before it leaves the agent.
    pub fn check(&self, intent: &IntentEnvelope, output: &str) -> PolicyDecision {
        let ctx = PolicyContext {
            intent,
            capability: None,
            requester: &intent.requester,
            payload: None,
            output: Some(output),
            side_effect: None,
            remaining_budget: intent.budget,
            tenant: intent.requester.tenant.as_ref(),
            composed_writes: 0,
            ontology_class_lookup: None,
        };
        self.engine.evaluate(PolicyHook::PostStep, &ctx)
    }
}

/// Action guard — runs **before** the agent triggers a side effect.
pub struct ActionGuard<'e> {
    engine: &'e PolicyEngine,
}

impl<'e> ActionGuard<'e> {
    /// Wrap an engine.
    pub fn new(engine: &'e PolicyEngine) -> Self {
        Self { engine }
    }

    /// Inspect a proposed action.
    pub fn check<'a>(
        &self,
        intent: &'a IntentEnvelope,
        capability: &'a aaf_contracts::CapabilityContract,
        composed_writes: u32,
    ) -> PolicyDecision {
        let ctx = PolicyContext {
            intent,
            capability: Some(capability),
            requester: &intent.requester,
            payload: None,
            output: None,
            side_effect: Some(capability.side_effect),
            remaining_budget: intent.budget,
            tenant: intent.requester.tenant.as_ref(),
            composed_writes,
            ontology_class_lookup: None,
        };
        self.engine.evaluate(PolicyHook::PreStep, &ctx)
    }
}
