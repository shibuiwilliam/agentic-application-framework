//! `pii_guard` rule.

use super::Rule;
use crate::context::PolicyContext;
use crate::engine::PolicyHook;
use aaf_contracts::{PolicySeverity, PolicyViolation, RuleKind};
use once_cell::sync::Lazy;
use regex::Regex;

static EMAIL_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"(?i)\b[A-Z0-9._%+-]+@[A-Z0-9.-]+\.[A-Z]{2,}\b").expect("email regex")
});
static JP_PHONE_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b0[7-9]0[-\s]?\d{4}[-\s]?\d{4}\b").expect("jp phone regex"));
static CREDIT_CARD_RE: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"\b(?:\d[ -]*?){13,19}\b").expect("credit card regex"));

/// PII guard. Inspects `ctx.output` (or `ctx.payload` as fallback) for
/// patterns matching email / Japanese mobile / credit-card numbers.
pub struct PiiGuard;

impl Rule for PiiGuard {
    fn id(&self) -> &str {
        "pii-guard"
    }

    fn applicable_hooks(&self) -> Option<&[PolicyHook]> {
        // PII scanning targets *outputs* — after a step produces data
        // and before an artifact is committed.
        static HOOKS: [PolicyHook; 2] = [PolicyHook::PostStep, PolicyHook::PreArtifact];
        Some(&HOOKS)
    }

    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation> {
        let text = ctx.output.or(ctx.payload)?;
        let mut hits = vec![];
        if EMAIL_RE.is_match(text) {
            hits.push("email");
        }
        if JP_PHONE_RE.is_match(text) {
            hits.push("jp_phone");
        }
        if CREDIT_CARD_RE.is_match(text) {
            hits.push("credit_card");
        }
        if hits.is_empty() {
            return None;
        }
        Some(PolicyViolation {
            rule_id: self.id().into(),
            kind: RuleKind::PiiGuard,
            severity: PolicySeverity::Error,
            message: format!("PII detected: {}", hits.join(", ")),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentEnvelope, IntentType, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn ctx_with_output<'a>(out: &'a str, intent: &'a IntentEnvelope) -> PolicyContext<'a> {
        PolicyContext {
            intent,
            capability: None,
            requester: &intent.requester,
            payload: None,
            output: Some(out),
            side_effect: None,
            remaining_budget: intent.budget,
            tenant: None,
            composed_writes: 0,
            ontology_class_lookup: None,
        }
    }

    fn intent() -> IntentEnvelope {
        IntentEnvelope {
            intent_id: "int-1".into(),
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
            risk_tier: RiskTier::Read,
            approval_policy: "none".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    #[test]
    fn detects_email_in_output() {
        let i = intent();
        let ctx = ctx_with_output("contact me at foo@example.com", &i);
        let v = PiiGuard.evaluate(&ctx).unwrap();
        assert!(v.message.contains("email"));
    }

    #[test]
    fn detects_jp_phone_number() {
        let i = intent();
        let ctx = ctx_with_output("電話 090-1234-5678", &i);
        assert!(PiiGuard.evaluate(&ctx).is_some());
    }

    #[test]
    fn clean_output_passes() {
        let i = intent();
        let ctx = ctx_with_output("nothing sensitive", &i);
        assert!(PiiGuard.evaluate(&ctx).is_none());
    }
}
