//! Checkpoint-based replay engine.
//!
//! Production replay re-executes the runtime against a snapshot. For
//! v0.1 we expose a deterministic projection: given a trace and a step
//! number, return all observations up to and including that step. This
//! is sufficient for what-if and audit-by-step UI.

use aaf_contracts::{ExecutionTrace, TraceStep};
use thiserror::Error;

/// Errors raised by the replay engine.
#[derive(Debug, Error)]
pub enum ReplayError {
    /// Step number was beyond the recorded steps.
    #[error("step {0} not in trace")]
    OutOfRange(u32),
}

/// Helper that operates on a trace document.
pub struct Replayer<'a> {
    trace: &'a ExecutionTrace,
}

impl<'a> Replayer<'a> {
    /// Wrap a trace.
    pub fn new(trace: &'a ExecutionTrace) -> Self {
        Self { trace }
    }

    /// Return all steps up to and including `up_to`.
    pub fn project(&self, up_to: u32) -> Result<Vec<&'a TraceStep>, ReplayError> {
        let max = self.trace.steps.iter().map(|s| s.step).max().unwrap_or(0);
        if up_to > max && up_to != 0 {
            return Err(ReplayError::OutOfRange(up_to));
        }
        Ok(self
            .trace
            .steps
            .iter()
            .filter(|s| s.step <= up_to)
            .collect())
    }
}
