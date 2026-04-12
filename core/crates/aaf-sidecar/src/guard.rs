//! Local input/output guard wrapper around [`aaf_policy::PolicyEngine`].

use aaf_contracts::{IntentEnvelope, PolicyDecision};
use aaf_policy::{InputGuard, OutputGuard, PolicyEngine};
use std::sync::Arc;

/// Local guard pair.
pub struct LocalGuard {
    engine: Arc<PolicyEngine>,
}

impl LocalGuard {
    /// Construct.
    pub fn new(engine: Arc<PolicyEngine>) -> Self {
        Self { engine }
    }

    /// Run input guard.
    pub fn check_input(&self, intent: &IntentEnvelope, payload: &str) -> PolicyDecision {
        InputGuard::new(&self.engine).check(intent, payload)
    }

    /// Run output guard.
    pub fn check_output(&self, intent: &IntentEnvelope, output: &str) -> PolicyDecision {
        OutputGuard::new(&self.engine).check(intent, output)
    }
}
