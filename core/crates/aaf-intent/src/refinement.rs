//! Refinement / clarification.

use aaf_contracts::{IntentEnvelope, IntentType};

/// One question to ask the requester.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClarificationQuestion {
    /// Field that needs clarification.
    pub field: &'static str,
    /// Free-text question presented to the requester.
    pub prompt: String,
}

/// Refiner.
pub struct Refiner;

impl Refiner {
    /// Inspect an envelope and emit clarification questions for required
    /// fields that are missing or empty. Limited to **at most 2** so we
    /// stay aligned with the §3.2 refinement protocol.
    pub fn questions_for(env: &IntentEnvelope) -> Vec<ClarificationQuestion> {
        let mut out: Vec<ClarificationQuestion> = vec![];
        if env.goal.trim().is_empty() {
            out.push(ClarificationQuestion {
                field: "goal",
                prompt: "What goal would you like the agent to achieve?".into(),
            });
        }
        if env.domain.trim().is_empty() {
            out.push(ClarificationQuestion {
                field: "domain",
                prompt: "Which business domain does this fall under (e.g. sales, warehouse)?"
                    .into(),
            });
        }
        match env.intent_type {
            IntentType::TransactionalIntent if !env.constraints.contains_key("target") => {
                out.push(ClarificationQuestion {
                    field: "target",
                    prompt: "What entity should I act on?".into(),
                });
            }
            IntentType::AnalyticalIntent if !env.constraints.contains_key("period_ref") => {
                out.push(ClarificationQuestion {
                    field: "period_ref",
                    prompt: "Which time period should I analyse?".into(),
                });
            }
            _ => {}
        }
        out.truncate(2);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentId, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn env(intent_type: IntentType) -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec![],
                tenant: None,
            },
            goal: "g".into(),
            domain: "sales".into(),
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
    fn analytical_without_period_asks_for_period() {
        let q = Refiner::questions_for(&env(IntentType::AnalyticalIntent));
        assert!(q.iter().any(|c| c.field == "period_ref"));
    }

    #[test]
    fn caps_questions_at_two() {
        let mut e = env(IntentType::TransactionalIntent);
        e.goal = String::new();
        e.domain = String::new();
        let q = Refiner::questions_for(&e);
        assert!(q.len() <= 2);
    }
}
