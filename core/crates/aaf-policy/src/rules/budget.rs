//! `budget_control` rule.

use super::Rule;
use crate::context::PolicyContext;
use aaf_contracts::{PolicySeverity, PolicyViolation, RuleKind};

/// Default warn-at percentage (80%).
pub const DEFAULT_WARN_PCT: f64 = 0.80;

/// Budget control rule.
pub struct BudgetControl {
    /// Per-step max cost as fraction of remaining budget.
    pub warn_at_pct: f64,
}

impl Default for BudgetControl {
    fn default() -> Self {
        Self {
            warn_at_pct: DEFAULT_WARN_PCT,
        }
    }
}

impl Rule for BudgetControl {
    fn id(&self) -> &str {
        "budget-control"
    }

    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation> {
        let total = ctx.intent.budget.max_cost_usd;
        let remaining = ctx.remaining_budget.max_cost_usd;
        if total <= 0.0 {
            return None;
        }
        let used = (total - remaining).max(0.0);
        let pct = used / total;
        if remaining <= 0.0 {
            return Some(PolicyViolation {
                rule_id: self.id().into(),
                kind: RuleKind::BudgetControl,
                severity: PolicySeverity::Error,
                message: format!("budget exhausted: used ${used:.4} of ${total:.4}"),
            });
        }
        if pct >= self.warn_at_pct {
            return Some(PolicyViolation {
                rule_id: self.id().into(),
                kind: RuleKind::BudgetControl,
                severity: PolicySeverity::Warning,
                message: format!(
                    "budget warning: {pct:.0}% used (${used:.4} of ${total:.4})",
                    pct = pct * 100.0
                ),
            });
        }
        None
    }
}
