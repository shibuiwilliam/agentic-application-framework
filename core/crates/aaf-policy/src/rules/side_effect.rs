//! `side_effect_gate` rule — gates write/delete/send/payment behind
//! approval unless the requester explicitly carries an `auto-approve`
//! scope.

use super::Rule;
use crate::context::PolicyContext;
use aaf_contracts::{PolicySeverity, PolicyViolation, RuleKind, SideEffect};

/// Side-effect gate.
pub struct SideEffectGate;

impl Rule for SideEffectGate {
    fn id(&self) -> &str {
        "side-effect-gate"
    }

    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation> {
        let se = ctx
            .side_effect
            .or_else(|| ctx.capability.map(|c| c.side_effect))?;
        if !matches!(
            se,
            SideEffect::Write | SideEffect::Delete | SideEffect::Send | SideEffect::Payment
        ) {
            return None;
        }
        if ctx.requester.scopes.iter().any(|s| s == "auto-approve") {
            return None;
        }
        Some(PolicyViolation {
            rule_id: self.id().into(),
            kind: RuleKind::SideEffectGate,
            severity: PolicySeverity::Warning,
            message: format!("side effect {se:?} requires human approval"),
        })
    }
}
