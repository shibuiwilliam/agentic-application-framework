//! Event → Intent adapter.
//!
//! The adapter turns an [`crate::event::AppEvent`] into an
//! [`aaf_contracts::IntentEnvelope`]. Slice A ships a simple
//! rule-based adapter that maps `event_type → intent_type` plus a
//! prefix for the `goal` text. Slice B will add an LLM-backed adapter
//! behind the same trait.

use crate::event::AppEvent;
use crate::situation_packager::SituationPackager;
use aaf_contracts::{BudgetContract, IntentEnvelope, IntentId, IntentType, Requester, RiskTier};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;

/// Pluggable adapter trait.
#[async_trait]
pub trait EventToIntentAdapter: Send + Sync {
    /// Turn an event into an intent. Returning `None` means "this
    /// event is not actionable" — the surface drops it.
    async fn adapt(&self, event: &AppEvent) -> Option<IntentEnvelope>;
}

/// A rule-based adapter configured from an `event_type → intent_type`
/// map.
pub struct RuleBasedAdapter {
    rules: HashMap<String, IntentType>,
    /// Default budget applied when the surface doesn't override.
    pub default_budget: BudgetContract,
}

impl RuleBasedAdapter {
    /// Construct with the given rules and default budget.
    pub fn new(rules: HashMap<String, IntentType>, default_budget: BudgetContract) -> Self {
        Self {
            rules,
            default_budget,
        }
    }

    /// Convenience: prepopulate a small default rule set.
    pub fn with_defaults() -> Self {
        let mut rules = HashMap::new();
        rules.insert("order.page.opened".into(), IntentType::AnalyticalIntent);
        rules.insert(
            "order.cancel.requested".into(),
            IntentType::TransactionalIntent,
        );
        rules.insert(
            "order.review.requested".into(),
            IntentType::DelegationIntent,
        );
        Self::new(
            rules,
            BudgetContract {
                max_tokens: 3_000,
                max_cost_usd: 0.50,
                max_latency_ms: 15_000,
            },
        )
    }
}

#[async_trait]
impl EventToIntentAdapter for RuleBasedAdapter {
    async fn adapt(&self, event: &AppEvent) -> Option<IntentEnvelope> {
        let intent_type = self.rules.get(&event.event_type).copied()?;
        let risk_tier = match intent_type {
            IntentType::TransactionalIntent => RiskTier::Write,
            IntentType::AnalyticalIntent => RiskTier::Read,
            IntentType::PlanningIntent => RiskTier::Advisory,
            IntentType::DelegationIntent => RiskTier::Delegation,
            IntentType::GovernanceIntent => RiskTier::Governance,
        };
        let session = &event.situation.session;

        // Apply surface overrides to the budget.
        let mut budget = self.default_budget;
        if let Some(t) = event.situation.constraints.time_budget_ms {
            budget.max_latency_ms = t;
        }
        if let Some(c) = event.situation.constraints.cost_budget_usd {
            budget.max_cost_usd = c;
        }

        let goal = format!("handle event {}", event.event_type);

        // Situation packager copies the current entity refs into the
        // intent envelope's `entities_in_context`.
        let packager = SituationPackager::default();
        let entities = packager.package_entities(&event.situation);

        Some(IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type,
            requester: Requester {
                user_id: session.user_id.to_string(),
                role: session.role.clone(),
                scopes: session.scopes.clone(),
                tenant: Some(session.tenant_id.clone()),
            },
            goal,
            domain: event.source.surface.clone(),
            constraints: Default::default(),
            budget,
            deadline: None,
            risk_tier,
            approval_policy: "human".into(),
            output_contract: None,
            trace_id: event.trace_id.clone().unwrap_or_default(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: entities,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{
        AppEvent, EventSource, ScreenContext, SessionContext, Situation, SurfaceConstraints,
    };
    use aaf_contracts::{EntityRefLite, SessionId, TenantId, UserId};

    fn event(event_type: &str) -> AppEvent {
        AppEvent::new(
            event_type,
            EventSource {
                app_id: "ops".into(),
                surface: "order-detail".into(),
            },
            Situation {
                current_entities: vec![EntityRefLite::new("commerce.Order")],
                current_screen: Some(ScreenContext {
                    route: "/orders/1".into(),
                    component: "OrderDetail".into(),
                    visible_fields: vec!["id".into()],
                }),
                session: SessionContext {
                    user_id: UserId::new(),
                    role: "analyst".into(),
                    scopes: vec!["sales:read".into()],
                    locale: "en-US".into(),
                    tenant_id: TenantId::from("t-a"),
                },
                constraints: SurfaceConstraints::default(),
            },
            SessionId::new(),
        )
    }

    #[tokio::test]
    async fn known_event_produces_intent() {
        let a = RuleBasedAdapter::with_defaults();
        let ev = event("order.page.opened");
        let intent = a.adapt(&ev).await.unwrap();
        assert_eq!(intent.intent_type, IntentType::AnalyticalIntent);
        assert_eq!(intent.requester.tenant.unwrap().as_str(), "t-a");
        assert_eq!(intent.entities_in_context.len(), 1);
    }

    #[tokio::test]
    async fn unknown_event_is_dropped() {
        let a = RuleBasedAdapter::with_defaults();
        let ev = event("random.ping");
        assert!(a.adapt(&ev).await.is_none());
    }

    #[tokio::test]
    async fn surface_overrides_apply_to_budget() {
        let a = RuleBasedAdapter::with_defaults();
        let mut ev = event("order.page.opened");
        ev.situation.constraints.time_budget_ms = Some(500);
        ev.situation.constraints.cost_budget_usd = Some(0.01);
        let intent = a.adapt(&ev).await.unwrap();
        assert_eq!(intent.budget.max_latency_ms, 500);
        assert!((intent.budget.max_cost_usd - 0.01).abs() < 1e-9);
    }
}
