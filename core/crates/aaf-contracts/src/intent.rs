//! Intent Envelope — the structured representation of a user goal that
//! flows through every layer of AAF.

use crate::error::ContractError;
use crate::ids::{IntentId, TenantId, TraceId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

/// Maximum delegation depth permitted by Rule 8.
pub const MAX_DEPTH: u32 = 5;

/// The five built-in intent classes. Additional types may be registered at
/// runtime via `aaf-intent::versioning`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum IntentType {
    /// Mutates state on a target service.
    TransactionalIntent,
    /// Reads / aggregates / analyses data.
    AnalyticalIntent,
    /// Produces a multi-step plan or proposal.
    PlanningIntent,
    /// Hands off to another agent or human.
    DelegationIntent,
    /// Modifies AAF policy / permissions / configuration.
    GovernanceIntent,
}

/// Risk category derived from intent type and target side effects.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RiskTier {
    /// Read-only.
    Read,
    /// Mutates state.
    Write,
    /// Advisory output (no execution).
    Advisory,
    /// Delegates work to another principal.
    Delegation,
    /// Modifies governance settings.
    Governance,
}

/// Identity / authority of the principal that submitted the intent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requester {
    /// Identity of the user / system that issued the intent.
    pub user_id: String,
    /// Role of the requester (e.g. `sales_manager`).
    pub role: String,
    /// OAuth-style scopes the requester carries.
    pub scopes: Vec<String>,
    /// Optional tenant scope for multi-tenant deployments.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<TenantId>,
}

/// Bounded autonomy budget for a single intent (Rule 8).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct BudgetContract {
    /// Maximum LLM tokens (input + output) across all steps.
    pub max_tokens: u64,
    /// Maximum monetary cost in USD.
    pub max_cost_usd: f64,
    /// Maximum end-to-end latency budget in milliseconds.
    pub max_latency_ms: u64,
}

impl BudgetContract {
    /// Validate that all budget components are non-negative.
    pub fn validate(&self) -> Result<(), ContractError> {
        if !self.max_cost_usd.is_finite() || self.max_cost_usd < 0.0 {
            return Err(ContractError::InvalidBudget(format!(
                "max_cost_usd={}",
                self.max_cost_usd
            )));
        }
        Ok(())
    }
}

/// The output contract a downstream node should produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OutputContract {
    /// Required output format identifier (e.g. `structured_report`).
    pub format: String,
    /// Optional schema reference URI.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema_ref: Option<String>,
}

/// The Intent Envelope is the canonical typed representation of a goal as
/// it flows through Front Door → Intent Compiler → Planner → Runtime.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct IntentEnvelope {
    /// Stable id assigned by the intent compiler.
    pub intent_id: IntentId,
    /// Classification of the intent.
    #[serde(rename = "type")]
    pub intent_type: IntentType,
    /// Identity of the principal that issued the intent.
    pub requester: Requester,
    /// Free-text goal as understood by the intent compiler.
    pub goal: String,
    /// Domain (e.g. `sales`, `warehouse`) used for capability discovery.
    pub domain: String,
    /// Free-form constraints extracted from the natural-language input.
    #[serde(default)]
    pub constraints: BTreeMap<String, serde_json::Value>,
    /// Bounded-autonomy budget.
    pub budget: BudgetContract,
    /// Optional deadline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deadline: Option<DateTime<Utc>>,
    /// Risk tier — drives policy gating.
    pub risk_tier: RiskTier,
    /// Approval policy identifier (e.g. `none`, `human_review`).
    #[serde(default)]
    pub approval_policy: String,
    /// Optional contract describing the expected output shape.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_contract: Option<OutputContract>,
    /// Trace id this intent is associated with.
    pub trace_id: TraceId,
    /// Delegation depth — checked against [`MAX_DEPTH`].
    pub depth: u32,
    /// Wall-clock timestamp the envelope was sealed at.
    #[serde(default = "Utc::now")]
    pub created_at: DateTime<Utc>,

    // ── Enhancement E2: Domain Ontology Layer ──────────────────────────
    /// Entities known to be in scope for this intent. Populated by the
    /// intent compiler's enricher (Slice B) and by the app-native
    /// surface's situation packager (E3 Slice A).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub entities_in_context: Vec<crate::capability::EntityRefLite>,
}

impl IntentEnvelope {
    /// Validate the envelope's structural invariants. Called once by the
    /// intent compiler before handing the envelope downstream.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.goal.trim().is_empty() {
            return Err(ContractError::MissingField("goal"));
        }
        if self.domain.trim().is_empty() {
            return Err(ContractError::MissingField("domain"));
        }
        if self.depth > MAX_DEPTH {
            return Err(ContractError::DepthExceeded {
                depth: self.depth,
                max: MAX_DEPTH,
            });
        }
        self.budget.validate()?;
        Ok(())
    }

    /// Produce a child envelope with `depth + 1`. Returns
    /// [`ContractError::DepthExceeded`] if the new depth would exceed
    /// [`MAX_DEPTH`]. Required for delegation chains (Rule 8).
    pub fn delegate(&self) -> Result<Self, ContractError> {
        let mut child = self.clone();
        child.intent_id = IntentId::new();
        child.depth += 1;
        if child.depth > MAX_DEPTH {
            return Err(ContractError::DepthExceeded {
                depth: child.depth,
                max: MAX_DEPTH,
            });
        }
        Ok(child)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::AnalyticalIntent,
            requester: Requester {
                user_id: "user-1".into(),
                role: "analyst".into(),
                scopes: vec!["sales:read".into()],
                tenant: None,
            },
            goal: "show last month's revenue".into(),
            domain: "sales".into(),
            constraints: std::collections::BTreeMap::new(),
            budget: BudgetContract {
                max_tokens: 5000,
                max_cost_usd: 1.0,
                max_latency_ms: 30_000,
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
    fn validates_happy_path() {
        sample().validate().expect("valid envelope");
    }

    #[test]
    fn rejects_empty_goal() {
        let mut env = sample();
        env.goal = " ".into();
        assert!(matches!(
            env.validate(),
            Err(ContractError::MissingField("goal"))
        ));
    }

    #[test]
    fn rejects_negative_budget() {
        let mut env = sample();
        env.budget.max_cost_usd = -1.0;
        assert!(matches!(
            env.validate(),
            Err(ContractError::InvalidBudget(_))
        ));
    }

    #[test]
    fn delegation_increments_depth() {
        let env = sample();
        let child = env.delegate().expect("delegated");
        assert_eq!(child.depth, env.depth + 1);
        assert_ne!(child.intent_id, env.intent_id);
    }

    #[test]
    fn delegation_refuses_past_max_depth() {
        let mut env = sample();
        env.depth = MAX_DEPTH;
        assert!(matches!(
            env.delegate(),
            Err(ContractError::DepthExceeded { .. })
        ));
    }
}
