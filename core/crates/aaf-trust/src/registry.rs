//! In-memory trust registry — keyed by agent id.

use crate::autonomy::AutonomyPolicy;
use crate::promotion::{PromotionDecision, PromotionRules};
use crate::score::{ScoreEvent, ScoreHistory};
use aaf_contracts::{AgentId, AutonomyLevel, TrustScore};
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

#[derive(Debug, Clone)]
struct TrustRecord {
    score: TrustScore,
    history: ScoreHistory,
}

/// Trust registry.
#[derive(Default)]
pub struct TrustRegistry {
    inner: Arc<RwLock<HashMap<AgentId, TrustRecord>>>,
    policy: AutonomyPolicy,
    rules: PromotionRules,
}

impl TrustRegistry {
    /// Construct an empty registry with default policy and rules.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a new agent at the bootstrap trust score.
    pub fn register(&self, agent: AgentId) {
        self.inner.write().insert(
            agent,
            TrustRecord {
                score: TrustScore::initial(),
                history: ScoreHistory::default(),
            },
        );
    }

    /// Look up an agent's current trust score (or initial if unknown).
    pub fn get(&self, agent: &AgentId) -> TrustScore {
        self.inner
            .read()
            .get(agent)
            .map_or_else(TrustScore::initial, |r| r.score)
    }

    /// Record an event for an agent and recompute its score / autonomy.
    pub fn observe(&self, agent: &AgentId, event: ScoreEvent) -> (TrustScore, PromotionDecision) {
        let mut guard = self.inner.write();
        let record = guard.entry(agent.clone()).or_insert_with(|| TrustRecord {
            score: TrustScore::initial(),
            history: ScoreHistory::default(),
        });
        let delta = record.history.observe(event);
        record.score.value = TrustScore::clamped(record.score.value + delta);
        record.score.autonomy = self.policy.level_for(record.score.value);
        let decision = self.rules.evaluate(record.score.autonomy, &record.history);
        match decision {
            PromotionDecision::DropToFloor => {
                record.score.value = 0.5_f64.min(record.score.value);
                record.score.autonomy = AutonomyLevel::Level1;
            }
            PromotionDecision::Demote => {
                let lowered = record.score.autonomy.as_u8().saturating_sub(1).max(1);
                record.score.autonomy = AutonomyLevel::from_u8(lowered);
            }
            PromotionDecision::Promote => {
                let raised = (record.score.autonomy.as_u8() + 1).min(5);
                record.score.autonomy = AutonomyLevel::from_u8(raised);
            }
            PromotionDecision::Hold => {}
        }
        (record.score, decision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn policy_violation_resets_to_floor() {
        let reg = TrustRegistry::new();
        let agent = AgentId::from("expense");
        reg.register(agent.clone());
        for _ in 0..200 {
            reg.observe(&agent, ScoreEvent::Success);
        }
        let (_, decision) = reg.observe(&agent, ScoreEvent::PolicyViolation);
        assert_eq!(decision, PromotionDecision::DropToFloor);
        let s = reg.get(&agent);
        assert_eq!(s.autonomy, AutonomyLevel::Level1);
    }
}
