//! Policy engine — owns a set of rules and runs them against a
//! [`crate::context::PolicyContext`].

use crate::context::PolicyContext;
use crate::rules::{
    boundary::BoundaryEnforcement, budget::BudgetControl, composition::CompositionSafety,
    injection::InjectionGuard, pii::PiiGuard, scope::ScopeCheck, side_effect::SideEffectGate, Rule,
};
use aaf_contracts::{PolicyDecision, PolicySeverity, PolicyViolation};
use std::sync::Arc;

/// Where the engine is being invoked from. Different hooks naturally
/// provide different fields in the [`PolicyContext`]; the engine still
/// runs every rule but a rule that lacks the data it needs will return
/// `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PolicyHook {
    /// Before planning — only intent + requester are populated.
    PrePlan,
    /// Before executing a step.
    PreStep,
    /// After a step has produced output.
    PostStep,
    /// Before producing an artifact.
    PreArtifact,
}

/// Policy engine.
pub struct PolicyEngine {
    rules: Vec<Arc<dyn Rule>>,
}

impl Default for PolicyEngine {
    fn default() -> Self {
        Self::with_default_rules()
    }
}

impl PolicyEngine {
    /// Construct an empty engine.
    pub fn empty() -> Self {
        Self { rules: vec![] }
    }

    /// Construct an engine pre-loaded with the seven default rule
    /// families.
    pub fn with_default_rules() -> Self {
        let rules: Vec<Arc<dyn Rule>> = vec![
            Arc::new(ScopeCheck),
            Arc::new(SideEffectGate),
            Arc::new(BudgetControl::default()),
            Arc::new(PiiGuard),
            Arc::new(InjectionGuard),
            Arc::new(CompositionSafety::default()),
            Arc::new(BoundaryEnforcement),
        ];
        Self { rules }
    }

    /// Append a custom rule (plugin).
    pub fn add_rule(&mut self, rule: Arc<dyn Rule>) {
        self.rules.push(rule);
    }

    /// Evaluate every rule against `ctx` and return an aggregate
    /// decision.
    ///
    /// Decision algorithm (severity ladder + one explicit special case):
    ///
    /// 1. **No violations** → `Allow`.
    /// 2. **Any `Fatal` or `Error`** → `Deny`.
    /// 3. **Any `SideEffectGate` violation** → `RequireApproval`. The
    ///    side-effect gate is *always* a "needs approval" signal even
    ///    if its declared severity is only `Warning`, because that is
    ///    its semantic role in the rule set.
    /// 4. **Otherwise** → `AllowWithWarnings`.
    pub fn evaluate(&self, hook: PolicyHook, ctx: &PolicyContext<'_>) -> PolicyDecision {
        let mut violations: Vec<PolicyViolation> = vec![];
        for rule in &self.rules {
            // Skip rules that declared specific hooks if the current
            // hook is not in their list.
            if let Some(hooks) = rule.applicable_hooks() {
                if !hooks.contains(&hook) {
                    continue;
                }
            }
            if let Some(v) = rule.evaluate(ctx) {
                violations.push(v);
            }
        }
        if violations.is_empty() {
            return PolicyDecision::Allow;
        }
        let max_severity = violations
            .iter()
            .map(|v| v.severity)
            .max()
            .unwrap_or(PolicySeverity::Info);
        if matches!(max_severity, PolicySeverity::Error | PolicySeverity::Fatal) {
            return PolicyDecision::Deny(violations);
        }
        if violations
            .iter()
            .any(|v| v.kind == aaf_contracts::RuleKind::SideEffectGate)
        {
            return PolicyDecision::RequireApproval(violations);
        }
        PolicyDecision::AllowWithWarnings(violations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
        CapabilitySla, CompensationSpec, DataClassification, EndpointKind, IntentEnvelope,
        IntentType, Requester, RiskTier, SideEffect, TraceId,
    };
    use chrono::Utc;

    fn intent(scopes: Vec<String>) -> IntentEnvelope {
        IntentEnvelope {
            intent_id: "int-1".into(),
            intent_type: IntentType::TransactionalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes,
                tenant: None,
            },
            goal: "do x".into(),
            domain: "d".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 1.0,
                max_latency_ms: 1000,
            },
            deadline: None,
            risk_tier: RiskTier::Write,
            approval_policy: "human".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    fn write_cap() -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from("cap-stock-reserve"),
            name: "reserve".into(),
            description: "reserve stock".into(),
            version: "1.0".into(),
            provider_agent: "inv".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::Grpc,
                address: "x".into(),
                method: None,
            },
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            side_effect: SideEffect::Write,
            idempotent: false,
            reversible: true,
            deterministic: true,
            compensation: Some(CompensationSpec {
                endpoint: "cap-stock-release".into(),
            }),
            sla: CapabilitySla::default(),
            cost: CapabilityCost::default(),
            required_scope: "inventory:write".into(),
            data_classification: DataClassification::Internal,
            degradation: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            tags: vec![],
            domains: vec![],
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        }
    }

    #[test]
    fn missing_scope_denies() {
        let engine = PolicyEngine::with_default_rules();
        let i = intent(vec![]);
        let cap = write_cap();
        let ctx = PolicyContext {
            intent: &i,
            capability: Some(&cap),
            requester: &i.requester,
            payload: None,
            output: None,
            side_effect: Some(SideEffect::Write),
            remaining_budget: i.budget,
            tenant: None,
            composed_writes: 0,
            ontology_class_lookup: None,
        };
        let decision = engine.evaluate(PolicyHook::PreStep, &ctx);
        assert!(matches!(decision, PolicyDecision::Deny(_)));
    }

    #[test]
    fn write_with_scope_requires_approval() {
        let engine = PolicyEngine::with_default_rules();
        let i = intent(vec!["inventory:write".into()]);
        let cap = write_cap();
        let ctx = PolicyContext {
            intent: &i,
            capability: Some(&cap),
            requester: &i.requester,
            payload: None,
            output: None,
            side_effect: Some(SideEffect::Write),
            remaining_budget: i.budget,
            tenant: None,
            composed_writes: 0,
            ontology_class_lookup: None,
        };
        let decision = engine.evaluate(PolicyHook::PreStep, &ctx);
        assert!(matches!(decision, PolicyDecision::RequireApproval(_)));
    }

    #[test]
    fn write_with_auto_approve_passes() {
        let engine = PolicyEngine::with_default_rules();
        let i = intent(vec!["inventory:write".into(), "auto-approve".into()]);
        let cap = write_cap();
        let ctx = PolicyContext {
            intent: &i,
            capability: Some(&cap),
            requester: &i.requester,
            payload: None,
            output: None,
            side_effect: Some(SideEffect::Write),
            remaining_budget: i.budget,
            tenant: None,
            composed_writes: 0,
            ontology_class_lookup: None,
        };
        let decision = engine.evaluate(PolicyHook::PreStep, &ctx);
        assert!(decision.is_allowed());
    }
}
