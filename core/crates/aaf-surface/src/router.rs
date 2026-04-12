//! Event Router with semantic classification (PROJECT_AafService §4.4).
//!
//! Classifies incoming [`crate::event::AppEvent`]s into one of three
//! routing categories before handing them to the
//! [`crate::ingest::EventToIntentAdapter`]:
//!
//! ```text
//! Event → fully structured? ──YES──→ ① FAST_PATH (bypass LLM)
//!                            │
//!                            NO
//!                            ↓
//!         single-topic? ──YES──→ ② AGENT_INTERPRET (small model)
//!                        │
//!                        NO
//!                        ↓
//!         ③ COMPOSITE (decompose → parallel delivery)
//! ```
//!
//! The classification is **rule-based in Slice A** — the same
//! `EventToIntentAdapter` trait is wired for the agent-interpret path;
//! the composite path splits the event into sub-events and runs them
//! through the same router recursively.
//!
//! A future LLM-backed classifier can replace the rule-based
//! classifier behind the same `EventClassifier` trait without changing
//! any call site.

use crate::event::AppEvent;
use serde::{Deserialize, Serialize};

/// Routing category for an event.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EventCategory {
    /// Fully structured + unambiguous target → bypass LLM.
    FastPath,
    /// Single topic with some ambiguity → small model normalises.
    AgentInterpret,
    /// Multiple topics embedded in one event → decompose.
    Composite,
}

/// Pluggable classifier trait. Slice A ships a deterministic
/// rule-based implementation; future slices can swap in an
/// LLM-backed classifier.
pub trait EventClassifier: Send + Sync {
    /// Classify an event.
    fn classify(&self, event: &AppEvent) -> EventCategory;

    /// If the event is [`EventCategory::Composite`], split it into
    /// sub-events. Returns `None` for non-composite events.
    fn decompose(&self, event: &AppEvent) -> Option<Vec<AppEvent>>;
}

/// Rule-based classifier.
///
/// Heuristics:
/// - **Fast path:** event type starts with `"fast."` OR the payload
///   is a flat JSON object with ≤ 5 keys (signals a structured API
///   call).
/// - **Composite:** the payload is an array of ≥ 2 objects (signals
///   multiple embedded sub-requests).
/// - **Agent interpret:** everything else.
pub struct RuleBasedClassifier;

impl EventClassifier for RuleBasedClassifier {
    fn classify(&self, event: &AppEvent) -> EventCategory {
        // Explicit fast-path prefix.
        if event.event_type.starts_with("fast.") {
            return EventCategory::FastPath;
        }
        // Structured payload with few keys → fast path.
        if let Some(obj) = event.payload.as_object() {
            if obj.len() <= 5 && !obj.is_empty() {
                return EventCategory::FastPath;
            }
        }
        // Array payload with ≥ 2 items → composite.
        if let Some(arr) = event.payload.as_array() {
            if arr.len() >= 2 {
                return EventCategory::Composite;
            }
        }
        EventCategory::AgentInterpret
    }

    fn decompose(&self, event: &AppEvent) -> Option<Vec<AppEvent>> {
        let arr = event.payload.as_array()?;
        if arr.len() < 2 {
            return None;
        }
        let subs: Vec<AppEvent> = arr
            .iter()
            .enumerate()
            .map(|(i, item)| {
                let mut sub = event.clone();
                sub.event_id = aaf_contracts::EventId::new();
                sub.event_type = format!("{}[{}]", event.event_type, i);
                sub.payload = item.clone();
                sub
            })
            .collect();
        Some(subs)
    }
}

/// Result of routing an event.
#[derive(Debug, Clone)]
pub enum RouteOutcome {
    /// Single fast-path event — forward directly, no LLM.
    FastPath(AppEvent),
    /// Single event that needs agent interpretation.
    AgentInterpret(AppEvent),
    /// Multiple sub-events decomposed from a composite event.
    Decomposed(Vec<AppEvent>),
}

/// Event router: classify → (optionally decompose) → produce
/// `RouteOutcome`.
pub struct EventRouter<C: EventClassifier = RuleBasedClassifier> {
    classifier: C,
}

impl EventRouter<RuleBasedClassifier> {
    /// Construct with the default rule-based classifier.
    pub fn new() -> Self {
        Self {
            classifier: RuleBasedClassifier,
        }
    }
}

impl Default for EventRouter<RuleBasedClassifier> {
    fn default() -> Self {
        Self::new()
    }
}

impl<C: EventClassifier> EventRouter<C> {
    /// Construct with a custom classifier.
    pub fn with_classifier(classifier: C) -> Self {
        Self { classifier }
    }

    /// Route an event.
    pub fn route(&self, event: AppEvent) -> RouteOutcome {
        match self.classifier.classify(&event) {
            EventCategory::FastPath => RouteOutcome::FastPath(event),
            EventCategory::AgentInterpret => RouteOutcome::AgentInterpret(event),
            EventCategory::Composite => {
                match self.classifier.decompose(&event) {
                    Some(subs) => RouteOutcome::Decomposed(subs),
                    None => RouteOutcome::AgentInterpret(event), // fallback
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::event::{AppEvent, EventSource, SessionContext, Situation, SurfaceConstraints};
    use aaf_contracts::{SessionId, TenantId, UserId};

    fn event(event_type: &str, payload: serde_json::Value) -> AppEvent {
        AppEvent {
            event_id: aaf_contracts::EventId::new(),
            event_type: event_type.into(),
            source: EventSource {
                app_id: "test".into(),
                surface: "test".into(),
            },
            situation: Situation {
                current_entities: vec![],
                current_screen: None,
                session: SessionContext {
                    user_id: UserId::new(),
                    role: "tester".into(),
                    scopes: vec![],
                    locale: "en".into(),
                    tenant_id: TenantId::from("t"),
                },
                constraints: SurfaceConstraints::default(),
            },
            payload,
            session_id: SessionId::new(),
            timestamp: chrono::Utc::now(),
            trace_id: None,
        }
    }

    #[test]
    fn fast_prefix_routes_to_fast_path() {
        let r = EventRouter::new();
        let e = event("fast.health-check", serde_json::json!(null));
        assert!(matches!(r.route(e), RouteOutcome::FastPath(_)));
    }

    #[test]
    fn small_structured_payload_routes_to_fast_path() {
        let r = EventRouter::new();
        let e = event(
            "order.status",
            serde_json::json!({"order_id": "ord-42", "status": "shipped"}),
        );
        assert!(matches!(r.route(e), RouteOutcome::FastPath(_)));
    }

    #[test]
    fn ambiguous_event_routes_to_agent_interpret() {
        let r = EventRouter::new();
        let e = event("customer.inquiry", serde_json::json!(null));
        assert!(matches!(r.route(e), RouteOutcome::AgentInterpret(_)));
    }

    #[test]
    fn array_payload_routes_to_composite_and_decomposes() {
        let r = EventRouter::new();
        let e = event(
            "batch.orders",
            serde_json::json!([
                {"order_id": "ord-1", "action": "cancel"},
                {"order_id": "ord-2", "action": "ship"},
                {"order_id": "ord-3", "action": "refund"},
            ]),
        );
        match r.route(e) {
            RouteOutcome::Decomposed(subs) => {
                assert_eq!(subs.len(), 3);
                assert_eq!(subs[0].event_type, "batch.orders[0]");
                assert_eq!(subs[1].event_type, "batch.orders[1]");
                assert_eq!(subs[2].event_type, "batch.orders[2]");
                // Each sub-event has its own unique event_id.
                assert_ne!(subs[0].event_id, subs[1].event_id);
            }
            other => panic!("expected Decomposed, got {other:?}"),
        }
    }

    #[test]
    fn single_item_array_is_not_composite() {
        let r = EventRouter::new();
        let e = event("single", serde_json::json!([{"x": 1}]));
        assert!(matches!(r.route(e), RouteOutcome::AgentInterpret(_)));
    }

    #[test]
    fn empty_payload_routes_to_agent_interpret() {
        let r = EventRouter::new();
        let e = event("something", serde_json::json!({}));
        // Empty object has 0 keys → not fast path (needs > 0).
        assert!(matches!(r.route(e), RouteOutcome::AgentInterpret(_)));
    }
}
