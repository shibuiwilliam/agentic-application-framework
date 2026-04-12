//! Bounded autonomy constraints (Rule 8 + PROJECT.md §3.4).

use crate::plan::ExecutionPlan;
use aaf_contracts::IntentEnvelope;
use thiserror::Error;

/// Constraint set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BoundedAutonomy {
    /// Maximum number of steps.
    pub max_steps: u32,
    /// Maximum delegation depth.
    pub max_depth: u32,
    /// Maximum monetary cost.
    pub max_cost_usd: f64,
    /// Maximum end-to-end duration in ms.
    pub max_latency_ms: u64,
}

impl Default for BoundedAutonomy {
    fn default() -> Self {
        Self {
            max_steps: 10,
            max_depth: 5,
            max_cost_usd: 1.0,
            max_latency_ms: 300_000,
        }
    }
}

/// Bound violation.
#[derive(Debug, Error, PartialEq)]
pub enum BoundsViolation {
    /// Plan exceeds the step cap.
    #[error("plan has {actual} steps but max is {max}")]
    TooManySteps {
        /// Actual.
        actual: u32,
        /// Max.
        max: u32,
    },

    /// Intent depth exceeds the protocol cap.
    #[error("delegation depth {actual} exceeds max {max}")]
    DepthExceeded {
        /// Actual.
        actual: u32,
        /// Max.
        max: u32,
    },

    /// Intent budget exceeds the configured cap.
    #[error("intent budget ${actual:.4} exceeds max ${max:.4}")]
    BudgetExceeded {
        /// Actual.
        actual: f64,
        /// Max.
        max: f64,
    },

    /// Intent latency budget exceeds the configured cap.
    #[error("intent latency {actual}ms exceeds max {max}ms")]
    LatencyExceeded {
        /// Actual.
        actual: u64,
        /// Max.
        max: u64,
    },
}

impl BoundedAutonomy {
    /// Validate an intent + plan against the bounds.
    pub fn validate(
        &self,
        intent: &IntentEnvelope,
        plan: &ExecutionPlan,
    ) -> Result<(), BoundsViolation> {
        if plan.steps.len() as u32 > self.max_steps {
            return Err(BoundsViolation::TooManySteps {
                actual: plan.steps.len() as u32,
                max: self.max_steps,
            });
        }
        if intent.depth > self.max_depth {
            return Err(BoundsViolation::DepthExceeded {
                actual: intent.depth,
                max: self.max_depth,
            });
        }
        if intent.budget.max_cost_usd > self.max_cost_usd {
            return Err(BoundsViolation::BudgetExceeded {
                actual: intent.budget.max_cost_usd,
                max: self.max_cost_usd,
            });
        }
        if intent.budget.max_latency_ms > self.max_latency_ms {
            return Err(BoundsViolation::LatencyExceeded {
                actual: intent.budget.max_latency_ms,
                max: self.max_latency_ms,
            });
        }
        Ok(())
    }
}
