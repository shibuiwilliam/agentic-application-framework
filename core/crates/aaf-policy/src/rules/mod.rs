//! Rule definitions.

pub mod boundary;
pub mod budget;
pub mod composition;
pub mod injection;
pub mod pii;
pub mod scope;
pub mod side_effect;

use crate::context::PolicyContext;
use crate::engine::PolicyHook;
use aaf_contracts::PolicyViolation;

/// Trait implemented by every rule. Each rule returns at most one
/// violation per evaluation; the engine aggregates results.
pub trait Rule: Send + Sync {
    /// Stable rule identifier (e.g. `pii-output`).
    fn id(&self) -> &str;

    /// Which hooks this rule should fire at. The default returns
    /// `None`, meaning the rule fires at **all** hooks. Override
    /// to restrict a rule to specific hooks — e.g. PII output
    /// scanning only at `PostStep`.
    fn applicable_hooks(&self) -> Option<&[PolicyHook]> {
        None
    }

    /// Run the rule against `ctx` and return the violation, if any.
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation>;
}
