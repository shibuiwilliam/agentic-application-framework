//! Learning contracts (Enhancement E1 Slice B).
//!
//! Typed shapes shared between `aaf-learn` (the subscriber crate that
//! adapts routing, reputation, and fast-path rules based on observed
//! outcomes) and every consumer of the adaptations.
//!
//! Rules enforced here:
//!
//! | Rule | Where |
//! |---|---|
//! | 15 Feedback is a contract | Every learning signal lives in a typed struct, not a log line. |
//! | 17 Every adaptation is reversible | `LearnedRuleRef` carries `learned_by`, `learned_at`, `evidence` so any adaptation can be rolled back by id. |
//! | 18 Policy governs learning | `LearnedRule.approval_state` must be `Approved` before the rule goes live; nothing auto-promotes. |

use crate::ids::{CapabilityId, IntentId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Source of a learned rule — who or what produced the adaptation.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnedSource {
    /// Produced by the fast-path miner from observed traffic.
    Miner,
    /// Hand-authored by a human operator.
    Human,
    /// Jointly produced by a miner with human approval edits.
    Hybrid,
}

/// Approval state for a learned rule. Must reach `Approved` before
/// the planner / registry / router honour it (Rule 18).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LearnedApprovalState {
    /// Proposed but not yet reviewed.
    Proposed,
    /// Approved by a human — the rule may go live.
    Approved,
    /// Rejected — the rule must not be applied.
    Rejected,
    /// Rolled back after having been live.
    RolledBack,
}

/// One learned rule — a fast-path rule, a reputation nudge, a router
/// weight change, or an escalation threshold adjustment.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LearnedRule {
    /// Unique id (e.g. `lr-fp-sales-monthly-001`).
    pub id: String,
    /// Who or what produced the rule.
    pub source: LearnedSource,
    /// Evidence — the intent ids that contributed to mining this rule.
    pub evidence: Vec<IntentId>,
    /// Current approval state.
    pub approval_state: LearnedApprovalState,
    /// Domain scope (e.g. `"sales"`, `"warehouse"`). Empty means global.
    #[serde(default)]
    pub scope: String,
    /// When the rule was proposed.
    pub proposed_at: DateTime<Utc>,
    /// When the rule was last updated (approved / rejected / rolled back).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub updated_at: Option<DateTime<Utc>>,
}

impl LearnedRule {
    /// Convenience constructor for a newly-proposed rule.
    pub fn propose(
        id: impl Into<String>,
        source: LearnedSource,
        evidence: Vec<IntentId>,
        scope: impl Into<String>,
    ) -> Self {
        Self {
            id: id.into(),
            source,
            evidence,
            approval_state: LearnedApprovalState::Proposed,
            scope: scope.into(),
            proposed_at: Utc::now(),
            updated_at: None,
        }
    }

    /// Promote to `Approved`.
    pub fn approve(&mut self) {
        self.approval_state = LearnedApprovalState::Approved;
        self.updated_at = Some(Utc::now());
    }

    /// Reject.
    pub fn reject(&mut self) {
        self.approval_state = LearnedApprovalState::Rejected;
        self.updated_at = Some(Utc::now());
    }

    /// Roll back.
    pub fn rollback(&mut self) {
        self.approval_state = LearnedApprovalState::RolledBack;
        self.updated_at = Some(Utc::now());
    }

    /// Returns `true` only when the rule is approved and may be used.
    pub fn is_live(&self) -> bool {
        self.approval_state == LearnedApprovalState::Approved
    }
}

/// Lightweight reference carried on adapted objects (reputation,
/// router weight, fast-path rule) so the adaptation is traceable
/// and reversible (Rule 17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LearnedRuleRef {
    /// Id of the `LearnedRule` this adaptation came from.
    pub learned_rule_id: String,
    /// Who produced it.
    pub learned_by: LearnedSource,
    /// When it was applied.
    pub learned_at: DateTime<Utc>,
    /// How many observations contributed to the adaptation.
    pub evidence_count: u32,
}

/// A record of one LLM routing decision (E1 §2.5).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RoutingDecisionRecord {
    /// Call id for tracing.
    pub call_id: String,
    /// Intent type that was routed.
    pub intent_type: String,
    /// Risk tier at the time of the call.
    pub risk_tier: String,
    /// Model that was chosen.
    pub model_chosen: String,
    /// USD cost of the call.
    pub cost_usd: f64,
    /// Quality score assigned by the eval judge (0–1).
    pub quality_score: f64,
    /// Outcome status.
    pub outcome: String,
    /// Timestamp.
    pub recorded_at: DateTime<Utc>,
}

/// A capability reputation update record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReputationUpdate {
    /// Capability that was scored.
    pub capability_id: CapabilityId,
    /// Previous score.
    pub old_score: f32,
    /// New score.
    pub new_score: f32,
    /// Reference to the learned rule that drove this change.
    pub evidence_ref: LearnedRuleRef,
    /// Timestamp.
    pub updated_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn proposed_rule_is_not_live() {
        let r = LearnedRule::propose("lr-1", LearnedSource::Miner, vec![], "sales");
        assert!(!r.is_live());
    }

    #[test]
    fn approved_rule_is_live() {
        let mut r = LearnedRule::propose("lr-1", LearnedSource::Miner, vec![], "sales");
        r.approve();
        assert!(r.is_live());
        assert!(r.updated_at.is_some());
    }

    #[test]
    fn rolled_back_rule_is_not_live() {
        let mut r = LearnedRule::propose("lr-1", LearnedSource::Human, vec![], "");
        r.approve();
        r.rollback();
        assert!(!r.is_live());
    }
}
