//! Promotion / demotion logic per PROJECT.md §3.8.

use crate::score::ScoreHistory;
use aaf_contracts::AutonomyLevel;

/// Outcome of evaluating an agent for autonomy adjustment.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PromotionDecision {
    /// Promote to the next level.
    Promote,
    /// Hold the current level.
    Hold,
    /// Demote one level.
    Demote,
    /// Critical event — drop straight to L1.
    DropToFloor,
}

/// Configuration knobs for the promotion engine.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PromotionRules {
    /// Override-rate ceiling at the current level.
    pub override_rate_ceiling: f64,
    /// Override-rate ceiling for promotion.
    pub override_rate_floor_for_promotion: f64,
    /// Minimum executions before promotion is even considered.
    pub min_executions_for_promotion: u64,
}

impl Default for PromotionRules {
    fn default() -> Self {
        Self {
            override_rate_ceiling: 0.05,
            override_rate_floor_for_promotion: 0.01,
            min_executions_for_promotion: 1000,
        }
    }
}

impl PromotionRules {
    /// Decide an action given the current level and history.
    pub fn evaluate(self, current: AutonomyLevel, history: &ScoreHistory) -> PromotionDecision {
        if history.policy_violations > 0 {
            return PromotionDecision::DropToFloor;
        }
        let or = history.override_rate();
        if or > self.override_rate_ceiling {
            return PromotionDecision::Demote;
        }
        if current == AutonomyLevel::Level5 {
            return PromotionDecision::Hold;
        }
        if history.total >= self.min_executions_for_promotion
            && or < self.override_rate_floor_for_promotion
        {
            PromotionDecision::Promote
        } else {
            PromotionDecision::Hold
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::score::ScoreEvent;

    #[test]
    fn policy_violation_drops_to_floor() {
        let rules = PromotionRules::default();
        let mut h = ScoreHistory::default();
        for _ in 0..100 {
            h.observe(ScoreEvent::Success);
        }
        h.observe(ScoreEvent::PolicyViolation);
        assert_eq!(
            rules.evaluate(AutonomyLevel::Level3, &h),
            PromotionDecision::DropToFloor
        );
    }

    #[test]
    fn high_override_rate_demotes() {
        let rules = PromotionRules::default();
        let mut h = ScoreHistory::default();
        for _ in 0..90 {
            h.observe(ScoreEvent::Success);
        }
        for _ in 0..10 {
            h.observe(ScoreEvent::HumanOverride);
        }
        assert_eq!(
            rules.evaluate(AutonomyLevel::Level3, &h),
            PromotionDecision::Demote
        );
    }

    #[test]
    fn promotes_when_threshold_met() {
        let rules = PromotionRules::default();
        let mut h = ScoreHistory::default();
        for _ in 0..1000 {
            h.observe(ScoreEvent::Success);
        }
        assert_eq!(
            rules.evaluate(AutonomyLevel::Level3, &h),
            PromotionDecision::Promote
        );
    }
}
