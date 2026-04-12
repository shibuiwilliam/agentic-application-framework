//! Application events — the single entry point for any signal that
//! should become an [`aaf_contracts::IntentEnvelope`].

use aaf_contracts::{EntityRefLite, EventId, SessionId, TenantId, TraceId, UserId};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Where an event originated.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventSource {
    /// Stable application id (e.g. `"ops-web"`).
    pub app_id: String,
    /// Logical surface name (e.g. `"order-detail"`).
    pub surface: String,
}

/// Context describing the screen / component the user was on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScreenContext {
    /// Route or URL path.
    pub route: String,
    /// Component identifier.
    pub component: String,
    /// Field names visible on the screen — used by the situation
    /// packager.
    #[serde(default)]
    pub visible_fields: Vec<String>,
}

/// Session context: user id, role, scopes, locale, tenant.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionContext {
    /// End-user id.
    pub user_id: UserId,
    /// Role.
    pub role: String,
    /// Scopes.
    pub scopes: Vec<String>,
    /// IETF language tag.
    pub locale: String,
    /// Tenant id.
    pub tenant_id: TenantId,
}

/// Optional constraints the surface wants applied to the downstream
/// intent (shorter budget, faster latency).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct SurfaceConstraints {
    /// Override for the intent's `max_latency_ms`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub time_budget_ms: Option<u64>,
    /// Override for the intent's `max_cost_usd`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost_budget_usd: Option<f64>,
}

/// The situation at the moment an event fired.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Situation {
    /// Entities currently in scope on the surface.
    #[serde(default)]
    pub current_entities: Vec<EntityRefLite>,
    /// Screen context.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub current_screen: Option<ScreenContext>,
    /// Session context.
    pub session: SessionContext,
    /// Surface-level constraints.
    #[serde(default)]
    pub constraints: SurfaceConstraints,
}

/// The event itself.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppEvent {
    /// Idempotency key (Rule: replay safety — see E3 §4.9).
    pub event_id: EventId,
    /// Event type, e.g. `"order.page.opened"`.
    pub event_type: String,
    /// Origin.
    pub source: EventSource,
    /// Situation at emit time.
    pub situation: Situation,
    /// Structured payload (entity refs + typed fields).
    pub payload: serde_json::Value,
    /// Session id.
    pub session_id: SessionId,
    /// When.
    pub timestamp: DateTime<Utc>,
    /// Optional linked trace id.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_id: Option<TraceId>,
}

impl AppEvent {
    /// Minimal constructor for tests.
    pub fn new(
        event_type: impl Into<String>,
        source: EventSource,
        situation: Situation,
        session_id: SessionId,
    ) -> Self {
        Self {
            event_id: EventId::new(),
            event_type: event_type.into(),
            source,
            situation,
            payload: serde_json::Value::Null,
            session_id,
            timestamp: Utc::now(),
            trace_id: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_situation() -> Situation {
        Situation {
            current_entities: vec![],
            current_screen: Some(ScreenContext {
                route: "/orders/123".into(),
                component: "OrderDetail".into(),
                visible_fields: vec!["id".into(), "status".into()],
            }),
            session: SessionContext {
                user_id: UserId::new(),
                role: "analyst".into(),
                scopes: vec!["sales:read".into()],
                locale: "en-US".into(),
                tenant_id: TenantId::from("tenant-a"),
            },
            constraints: SurfaceConstraints::default(),
        }
    }

    #[test]
    fn builds_app_event_with_idempotency_key() {
        let ev = AppEvent::new(
            "order.page.opened",
            EventSource {
                app_id: "ops-web".into(),
                surface: "order-detail".into(),
            },
            sample_situation(),
            SessionId::new(),
        );
        assert_eq!(ev.event_type, "order.page.opened");
        assert!(ev.event_id.as_str().starts_with("evt-"));
    }

    #[test]
    fn same_event_id_means_same_event_for_dedup() {
        let sit = sample_situation();
        let sid = SessionId::new();
        let source = EventSource {
            app_id: "ops-web".into(),
            surface: "order-detail".into(),
        };
        let a = AppEvent::new(
            "order.page.opened",
            source.clone(),
            sit.clone(),
            sid.clone(),
        );
        let mut b = AppEvent::new("order.page.opened", source, sit, sid);
        b.event_id = a.event_id.clone();
        assert_eq!(a.event_id, b.event_id);
    }
}
