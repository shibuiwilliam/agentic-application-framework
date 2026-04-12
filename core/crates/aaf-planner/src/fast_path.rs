//! Fast-path rules.
//!
//! A fast-path rule maps a structured request directly to a capability
//! with field-by-field input mapping. The runtime evaluates the rule
//! locally — no LLM, no round-trip to the control plane.

use aaf_contracts::{CapabilityId, IntentEnvelope};
use serde::{Deserialize, Serialize};

/// One field mapping from intent constraint to capability input.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldMapping {
    /// Constraint key inside the intent envelope.
    pub from: String,
    /// Field name on the capability input.
    pub to: String,
}

/// One condition that must be true for the rule to match.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Condition {
    /// Constraint key inside the intent envelope.
    pub field: String,
    /// Required value.
    pub equals: serde_json::Value,
}

/// Pattern descriptor — what the rule is matching against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequestPattern {
    /// Required intent type name.
    pub intent_type: String,
    /// Required domain.
    pub domain: String,
}

/// One fast-path rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FastPathRule {
    /// Pattern.
    pub pattern: RequestPattern,
    /// Target capability.
    pub target_capability: CapabilityId,
    /// Field mappings.
    #[serde(default)]
    pub field_mapping: Vec<FieldMapping>,
    /// Conditions.
    #[serde(default)]
    pub conditions: Vec<Condition>,
}

/// Outcome of fast-path evaluation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum FastPathOutcome {
    /// Matched a rule.
    Match {
        /// Capability id to invoke.
        capability_id: CapabilityId,
        /// Mapped request body.
        mapped_request: serde_json::Value,
    },
    /// No rule matched.
    NoMatch,
}

/// A rule tagged as hand-authored or learned (E1 Slice B).
#[derive(Debug, Clone)]
struct TaggedRule {
    rule: FastPathRule,
    /// `None` for hand-authored rules; `Some(id)` for learned rules
    /// so policy packs can disable them wholesale.
    learned_rule_id: Option<String>,
    /// Evidence intent ids that contributed to learning (empty for hand-authored).
    #[allow(dead_code)]
    evidence: Vec<String>,
    /// Whether the rule is active. Learned rules can be disabled by
    /// the policy pack without removing them from the set.
    enabled: bool,
}

/// A set of fast-path rules.
#[derive(Debug, Clone, Default)]
pub struct FastPathRuleSet {
    rules: Vec<TaggedRule>,
}

impl FastPathRuleSet {
    /// New empty set.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append a hand-authored rule.
    pub fn push(&mut self, rule: FastPathRule) {
        self.rules.push(TaggedRule {
            rule,
            learned_rule_id: None,
            evidence: vec![],
            enabled: true,
        });
    }

    /// Append a learned rule with its evidence trail. Learned rules
    /// carry a `learned_rule_id` so they can be disabled or removed
    /// by the policy pack (Rule 17: every adaptation is reversible).
    pub fn add_learned(
        &mut self,
        rule: FastPathRule,
        learned_rule_id: impl Into<String>,
        evidence: Vec<String>,
    ) {
        self.rules.push(TaggedRule {
            rule,
            learned_rule_id: Some(learned_rule_id.into()),
            evidence,
            enabled: true,
        });
    }

    /// Disable every learned rule whose `learned_rule_id` matches `id`.
    pub fn disable_learned(&mut self, id: &str) {
        for r in &mut self.rules {
            if r.learned_rule_id.as_deref() == Some(id) {
                r.enabled = false;
            }
        }
    }

    /// Re-enable a previously-disabled learned rule.
    pub fn enable_learned(&mut self, id: &str) {
        for r in &mut self.rules {
            if r.learned_rule_id.as_deref() == Some(id) {
                r.enabled = true;
            }
        }
    }

    /// List every learned rule's id (including disabled ones).
    pub fn list_learned(&self) -> Vec<&str> {
        self.rules
            .iter()
            .filter_map(|r| r.learned_rule_id.as_deref())
            .collect()
    }

    /// Try to match an intent against any rule.
    pub fn evaluate(&self, intent: &IntentEnvelope) -> FastPathOutcome {
        for tagged in &self.rules {
            if !tagged.enabled {
                continue;
            }
            let rule = &tagged.rule;
            if format!("{:?}", intent.intent_type) != rule.pattern.intent_type {
                continue;
            }
            if intent.domain != rule.pattern.domain {
                continue;
            }
            let mut conditions_ok = true;
            for c in &rule.conditions {
                if intent.constraints.get(&c.field) != Some(&c.equals) {
                    conditions_ok = false;
                    break;
                }
            }
            if !conditions_ok {
                continue;
            }
            // Build the mapped request.
            let mut mapped = serde_json::Map::new();
            for fm in &rule.field_mapping {
                if let Some(v) = intent.constraints.get(&fm.from) {
                    mapped.insert(fm.to.clone(), v.clone());
                }
            }
            return FastPathOutcome::Match {
                capability_id: rule.target_capability.clone(),
                mapped_request: serde_json::Value::Object(mapped),
            };
        }
        FastPathOutcome::NoMatch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentId, IntentType, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn analytical_with(period: &str) -> IntentEnvelope {
        let mut env = IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::AnalyticalIntent,
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
        };
        env.constraints
            .insert("period_ref".into(), serde_json::json!(period));
        env
    }

    #[test]
    fn matches_when_pattern_and_conditions_align() {
        let mut set = FastPathRuleSet::new();
        set.push(FastPathRule {
            pattern: RequestPattern {
                intent_type: "AnalyticalIntent".into(),
                domain: "sales".into(),
            },
            target_capability: CapabilityId::from("cap-sales-monthly"),
            field_mapping: vec![FieldMapping {
                from: "period_ref".into(),
                to: "period".into(),
            }],
            conditions: vec![Condition {
                field: "period_ref".into(),
                equals: serde_json::json!("last_month"),
            }],
        });
        let outcome = set.evaluate(&analytical_with("last_month"));
        match outcome {
            FastPathOutcome::Match {
                capability_id,
                mapped_request,
            } => {
                assert_eq!(capability_id.as_str(), "cap-sales-monthly");
                assert_eq!(
                    mapped_request.get("period"),
                    Some(&serde_json::json!("last_month"))
                );
            }
            FastPathOutcome::NoMatch => panic!("expected match"),
        }
    }

    #[test]
    fn no_match_when_condition_fails() {
        let mut set = FastPathRuleSet::new();
        set.push(FastPathRule {
            pattern: RequestPattern {
                intent_type: "AnalyticalIntent".into(),
                domain: "sales".into(),
            },
            target_capability: CapabilityId::from("cap-sales-monthly"),
            field_mapping: vec![],
            conditions: vec![Condition {
                field: "period_ref".into(),
                equals: serde_json::json!("last_month"),
            }],
        });
        let outcome = set.evaluate(&analytical_with("last_year"));
        assert_eq!(outcome, FastPathOutcome::NoMatch);
    }

    // ── E1 Slice B �� learned rules ────────────────────────────────────

    fn sales_rule() -> FastPathRule {
        FastPathRule {
            pattern: RequestPattern {
                intent_type: "AnalyticalIntent".into(),
                domain: "sales".into(),
            },
            target_capability: CapabilityId::from("cap-sales-weekly"),
            field_mapping: vec![],
            conditions: vec![],
        }
    }

    #[test]
    fn learned_rule_matches_like_hand_authored() {
        let mut set = FastPathRuleSet::new();
        set.add_learned(sales_rule(), "lr-weekly-sales", vec!["intent-1".into()]);
        let outcome = set.evaluate(&analytical_with("any"));
        assert!(matches!(outcome, FastPathOutcome::Match { .. }));
    }

    #[test]
    fn disabled_learned_rule_does_not_match() {
        let mut set = FastPathRuleSet::new();
        set.add_learned(sales_rule(), "lr-weekly-sales", vec![]);
        set.disable_learned("lr-weekly-sales");
        let outcome = set.evaluate(&analytical_with("any"));
        assert_eq!(outcome, FastPathOutcome::NoMatch);
    }

    #[test]
    fn re_enabled_learned_rule_matches_again() {
        let mut set = FastPathRuleSet::new();
        set.add_learned(sales_rule(), "lr-weekly-sales", vec![]);
        set.disable_learned("lr-weekly-sales");
        set.enable_learned("lr-weekly-sales");
        let outcome = set.evaluate(&analytical_with("any"));
        assert!(matches!(outcome, FastPathOutcome::Match { .. }));
    }

    #[test]
    fn list_learned_returns_all_ids() {
        let mut set = FastPathRuleSet::new();
        set.push(sales_rule()); // hand-authored — not in list
        set.add_learned(sales_rule(), "lr-1", vec![]);
        set.add_learned(sales_rule(), "lr-2", vec![]);
        let ids = set.list_learned();
        assert_eq!(ids, vec!["lr-1", "lr-2"]);
    }
}
