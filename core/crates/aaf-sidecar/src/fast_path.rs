//! Local fast-path rule evaluator. Wraps [`aaf_planner::FastPathRuleSet`]
//! so the sidecar can call it without depending on the full planner
//! surface.

use aaf_contracts::IntentEnvelope;
use aaf_planner::{FastPathOutcome, FastPathRule, FastPathRuleSet};

/// Local fast-path holder.
#[derive(Debug, Default)]
pub struct LocalFastPath {
    set: FastPathRuleSet,
}

impl LocalFastPath {
    /// New empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a rule.
    pub fn add(&mut self, rule: FastPathRule) {
        self.set.push(rule);
    }

    /// Try to match.
    pub fn evaluate(&self, intent: &IntentEnvelope) -> FastPathOutcome {
        self.set.evaluate(intent)
    }
}
