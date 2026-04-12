//! Transparent proxy with Rule 13 fallback.

use crate::fast_path::LocalFastPath;
use crate::health::SidecarHealth;
use aaf_contracts::IntentEnvelope;
use aaf_planner::FastPathOutcome;

/// Decision the proxy makes for a request.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ProxyDecision {
    /// Fast-path matched — invoke this capability locally.
    FastPath {
        /// Capability id.
        capability: String,
        /// Mapped request.
        mapped_request: serde_json::Value,
    },
    /// Forward to the AAF control plane for full processing.
    ForwardToAaf,
    /// Bypass AAF entirely (Rule 13).
    DirectForward,
}

/// Sidecar proxy.
pub struct Proxy {
    fast_path: LocalFastPath,
    health: SidecarHealth,
}

impl Proxy {
    /// Construct.
    pub fn new(fast_path: LocalFastPath, health: SidecarHealth) -> Self {
        Self { fast_path, health }
    }

    /// Handle a request envelope. Order:
    ///
    /// 1. If `health.is_healthy()` is false → `DirectForward` (Rule 13)
    /// 2. Try fast-path → `FastPath` if it matches (Rule 4)
    /// 3. Otherwise → `ForwardToAaf`
    pub fn handle(&self, intent: &IntentEnvelope) -> ProxyDecision {
        if !self.health.is_healthy() {
            return ProxyDecision::DirectForward;
        }
        match self.fast_path.evaluate(intent) {
            FastPathOutcome::Match {
                capability_id,
                mapped_request,
            } => ProxyDecision::FastPath {
                capability: capability_id.to_string(),
                mapped_request,
            },
            FastPathOutcome::NoMatch => ProxyDecision::ForwardToAaf,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentId, IntentType, Requester, RiskTier, TraceId};
    use chrono::Utc;

    fn intent() -> IntentEnvelope {
        IntentEnvelope {
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
        }
    }

    #[test]
    fn rule_13_unhealthy_sidecar_forwards_directly() {
        let health = SidecarHealth::healthy();
        let p = Proxy::new(LocalFastPath::default(), health.clone());
        health.mark_unhealthy();
        assert_eq!(p.handle(&intent()), ProxyDecision::DirectForward);
    }

    #[test]
    fn no_fast_path_match_forwards_to_aaf() {
        let health = SidecarHealth::healthy();
        let p = Proxy::new(LocalFastPath::default(), health);
        assert_eq!(p.handle(&intent()), ProxyDecision::ForwardToAaf);
    }
}
