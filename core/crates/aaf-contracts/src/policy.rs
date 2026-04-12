//! Policy contract — kinds, severities, and decision shapes used by
//! the [`crate`]'s policy engine and downstream consumers.

use serde::{Deserialize, Serialize};

/// Severity ladder for policy violations.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PolicySeverity {
    /// Informational.
    Info,
    /// Warning — proceed but record.
    Warning,
    /// Error — block the action.
    Error,
    /// Fatal — terminate the task and notify operators.
    Fatal,
}

/// Built-in policy rule families.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuleKind {
    /// `scope_check` — ensure required scopes are present.
    ScopeCheck,
    /// `side_effect_gate` — gate write/delete/send/payment.
    SideEffectGate,
    /// `budget_control` — enforce per-request / per-day budgets.
    BudgetControl,
    /// `pii_guard` — detect PII in outputs.
    PiiGuard,
    /// `injection_guard` — detect prompt injection in inputs.
    InjectionGuard,
    /// `composition_safety` — emergent risk of capability composition.
    CompositionSafety,
    /// `boundary_enforcement` — tenant / data boundary checks.
    BoundaryEnforcement,
}

/// One detected violation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PolicyViolation {
    /// Rule id.
    pub rule_id: String,
    /// Rule family.
    pub kind: RuleKind,
    /// Severity.
    pub severity: PolicySeverity,
    /// Human-readable description.
    pub message: String,
}

/// Outcome of a policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolicyDecision {
    /// All checks passed.
    Allow,
    /// Allowed but with warnings.
    AllowWithWarnings(Vec<PolicyViolation>),
    /// Requires human approval to proceed.
    RequireApproval(Vec<PolicyViolation>),
    /// Blocked.
    Deny(Vec<PolicyViolation>),
}

impl PolicyDecision {
    /// Returns true if execution may proceed (with or without warnings).
    pub fn is_allowed(&self) -> bool {
        matches!(
            self,
            PolicyDecision::Allow | PolicyDecision::AllowWithWarnings(_)
        )
    }
}
