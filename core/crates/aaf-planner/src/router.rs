//! Communication pattern classifier (Rule 4).

use aaf_contracts::{IntentEnvelope, IntentType};

/// Four communication patterns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommunicationPattern {
    /// Fully structured + unambiguous target.
    FastPath,
    /// Single service with minor ambiguity.
    AgentAssisted,
    /// Multi-service coordination.
    FullAgentic,
    /// Async / event-driven.
    AgenticChoreography,
}

/// Router.
pub struct Router;

impl Router {
    /// Classify a request given the intent envelope and a heuristic
    /// signal indicating whether a fast-path rule matched.
    pub fn classify(
        intent: &IntentEnvelope,
        fast_path_matched: bool,
        capability_count: usize,
    ) -> CommunicationPattern {
        if fast_path_matched {
            return CommunicationPattern::FastPath;
        }
        match intent.intent_type {
            IntentType::DelegationIntent => CommunicationPattern::AgenticChoreography,
            IntentType::AnalyticalIntent if capability_count <= 1 => {
                CommunicationPattern::AgentAssisted
            }
            IntentType::TransactionalIntent if capability_count <= 1 => {
                CommunicationPattern::AgentAssisted
            }
            _ => CommunicationPattern::FullAgentic,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentId, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn intent(t: IntentType) -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: t,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec![],
                tenant: None,
            },
            goal: "g".into(),
            domain: "d".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 1.0,
                max_latency_ms: 1000,
            },
            deadline: None,
            risk_tier: RiskTier::Read,
            approval_policy: "none".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    #[test]
    fn fast_path_match_short_circuits() {
        assert_eq!(
            Router::classify(&intent(IntentType::TransactionalIntent), true, 5),
            CommunicationPattern::FastPath
        );
    }

    #[test]
    fn analytical_with_one_cap_is_agent_assisted() {
        assert_eq!(
            Router::classify(&intent(IntentType::AnalyticalIntent), false, 1),
            CommunicationPattern::AgentAssisted
        );
    }

    #[test]
    fn delegation_is_choreography() {
        assert_eq!(
            Router::classify(&intent(IntentType::DelegationIntent), false, 1),
            CommunicationPattern::AgenticChoreography
        );
    }
}
