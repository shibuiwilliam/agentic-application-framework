//! `composition_safety` rule (P8 — Compositional Safety).

use super::Rule;
use crate::context::PolicyContext;
use aaf_contracts::{PolicySeverity, PolicyViolation, RuleKind, SideEffect};

/// Composition safety rule.
pub struct CompositionSafety {
    /// Maximum number of write-class side effects allowed in a single
    /// trace before composition is deemed unsafe.
    pub max_combined_writes: u32,
}

impl Default for CompositionSafety {
    fn default() -> Self {
        Self {
            max_combined_writes: 3,
        }
    }
}

impl Rule for CompositionSafety {
    fn id(&self) -> &str {
        "composition-safety"
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
        if ctx.composed_writes + 1 > self.max_combined_writes {
            return Some(PolicyViolation {
                rule_id: self.id().into(),
                kind: RuleKind::CompositionSafety,
                severity: PolicySeverity::Warning,
                message: format!(
                    "composition safety: {} write-class side effects already executed (max {})",
                    ctx.composed_writes, self.max_combined_writes
                ),
            });
        }
        None
    }
}
