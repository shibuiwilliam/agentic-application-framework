//! Budget tracker — Rule 8.

use aaf_contracts::BudgetContract;
use parking_lot::Mutex;
use std::sync::Arc;
use thiserror::Error;

/// Budget exhaustion errors.
#[derive(Debug, Error, Clone)]
pub enum BudgetTrackerError {
    /// Token budget hit zero.
    #[error("token budget exhausted (budget {budget})")]
    Tokens {
        /// Original token budget.
        budget: u64,
    },
    /// Cost budget hit zero.
    #[error("cost budget exhausted (budget ${budget:.4})")]
    Cost {
        /// Original cost budget in USD.
        budget: f64,
    },
    /// Time budget hit zero.
    #[error("time budget exhausted (budget {budget}ms)")]
    Time {
        /// Original time budget in ms.
        budget: u64,
    },
}

#[derive(Debug, Clone, Copy)]
struct BudgetState {
    initial: BudgetContract,
    used_tokens: u64,
    used_cost: f64,
    used_ms: u64,
}

/// Tracks remaining budget for a single intent.
#[derive(Clone)]
pub struct BudgetTracker {
    inner: Arc<Mutex<BudgetState>>,
}

impl BudgetTracker {
    /// Construct from an `IntentEnvelope::budget`.
    pub fn new(budget: BudgetContract) -> Self {
        Self {
            inner: Arc::new(Mutex::new(BudgetState {
                initial: budget,
                used_tokens: 0,
                used_cost: 0.0,
                used_ms: 0,
            })),
        }
    }

    /// Charge consumption against the tracker. Returns the kind of
    /// budget that was exhausted, if any.
    pub fn charge(
        &self,
        tokens: u64,
        cost_usd: f64,
        elapsed_ms: u64,
    ) -> Result<(), BudgetTrackerError> {
        let mut s = self.inner.lock();
        s.used_tokens = s.used_tokens.saturating_add(tokens);
        s.used_cost += cost_usd;
        s.used_ms = s.used_ms.saturating_add(elapsed_ms);
        if s.used_tokens > s.initial.max_tokens {
            return Err(BudgetTrackerError::Tokens {
                budget: s.initial.max_tokens,
            });
        }
        if s.used_cost > s.initial.max_cost_usd {
            return Err(BudgetTrackerError::Cost {
                budget: s.initial.max_cost_usd,
            });
        }
        if s.used_ms > s.initial.max_latency_ms {
            return Err(BudgetTrackerError::Time {
                budget: s.initial.max_latency_ms,
            });
        }
        Ok(())
    }

    /// Snapshot of the remaining budget.
    pub fn remaining(&self) -> BudgetContract {
        let s = *self.inner.lock();
        BudgetContract {
            max_tokens: s.initial.max_tokens.saturating_sub(s.used_tokens),
            max_cost_usd: (s.initial.max_cost_usd - s.used_cost).max(0.0),
            max_latency_ms: s.initial.max_latency_ms.saturating_sub(s.used_ms),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn b() -> BudgetContract {
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 0.10,
            max_latency_ms: 5000,
        }
    }

    #[test]
    fn charge_decrements_remaining() {
        let t = BudgetTracker::new(b());
        t.charge(100, 0.01, 200).unwrap();
        let r = t.remaining();
        assert_eq!(r.max_tokens, 900);
        assert!((r.max_cost_usd - 0.09).abs() < 1e-9);
        assert_eq!(r.max_latency_ms, 4800);
    }

    #[test]
    fn cost_overrun_is_caught() {
        let t = BudgetTracker::new(b());
        let err = t.charge(0, 0.5, 0).unwrap_err();
        assert!(matches!(err, BudgetTrackerError::Cost { .. }));
    }
}
