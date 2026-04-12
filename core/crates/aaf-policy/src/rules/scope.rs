//! `scope_check` rule.

use super::Rule;
use crate::context::PolicyContext;
use aaf_contracts::{PolicySeverity, PolicyViolation, RuleKind};

/// Verifies the requester carries the scope required by the capability.
pub struct ScopeCheck;

impl Rule for ScopeCheck {
    fn id(&self) -> &str {
        "scope-check"
    }

    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation> {
        let cap = ctx.capability?;
        let required = cap.required_scope.trim();
        if required.is_empty() {
            return None;
        }
        if ctx.requester.scopes.iter().any(|s| s == required) {
            return None;
        }
        Some(PolicyViolation {
            rule_id: self.id().into(),
            kind: RuleKind::ScopeCheck,
            severity: PolicySeverity::Error,
            message: format!(
                "requester missing required scope `{}` for capability `{}`",
                required, cap.id
            ),
        })
    }
}
