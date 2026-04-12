//! End-to-end test for examples/app-native-surface.
//!
//! Exercises AAF's **app-native surface layer** — how existing
//! applications integrate with AAF through events, proposals, and
//! projections without surrendering authority over their own state.
//!
//! **Event Routing & Adaptation:**
//!
//! 1.  Structured events route to FastPath (no LLM needed).
//! 2.  Ambiguous events route to AgentInterpret.
//! 3.  Batch events decompose into parallel sub-events.
//! 4.  RuleBasedAdapter converts known events to IntentEnvelopes.
//! 5.  Unknown events are dropped (not actionable).
//! 6.  Surface constraints override the default budget.
//!
//! **Proposals & Lifecycle (Rule 20):**
//!
//! 7.  Proposals with mutations require compensation_ref.
//! 8.  Proposals without mutations need no compensation.
//! 9.  Lifecycle: Draft → Proposed → AppReview → Accepted.
//! 10. Lifecycle: Draft → Proposed → AppReview → Rejected.
//! 11. Lifecycle: Draft → Proposed → AppReview → Transformed.
//! 12. Lifecycle: Draft → Proposed → AppReview → Expired.
//! 13. Illegal transitions are rejected.
//!
//! **Projections (Rule 19):**
//!
//! 14. Listed fields are readable.
//! 15. Unlisted fields are denied (default-deny).
//! 16. Cross-tenant access is rejected.
//!
//! **Situation Packager:**
//!
//! 17. Entity refs are forwarded to the intent.
//! 18. Oversized field lists exceed the context budget.
//!
//! **Full Pipeline:**
//!
//! 19. Event → Intent → Graph execution → Trace with outcome.
//!
//! Run this test with:
//!
//!     cargo test -p aaf-integration-tests --test app_native_surface_e2e

use aaf_contracts::{
    CapabilityId, EntityRefLite, IntentType, NodeId, SessionId, SideEffect, TenantId, TraceId,
    UserId,
};
use aaf_policy::PolicyEngine;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_surface::{
    ActionProposal, AppEvent, CompensationRef, EventRouter, EventSource, EventToIntentAdapter,
    ProjectionError, ProposalApprovalState, ProposalLifecycle, RouteOutcome, RuleBasedAdapter,
    ScreenContext, SessionContext, Situation, SituationPackager, StateMutationProposal,
    StateProjection, SurfaceConstraints, SurfaceError, UiHints,
};
use aaf_trace::{Recorder, TraceRecorder};
use chrono::{Duration, Utc};
use std::sync::Arc;

// ── Helpers ─────────────────────────────────────────────────────────

fn situation() -> Situation {
    Situation {
        current_entities: vec![EntityRefLite::new("commerce.Order")],
        current_screen: Some(ScreenContext {
            route: "/orders/42".into(),
            component: "OrderDetail".into(),
            visible_fields: vec!["id".into(), "status".into(), "created_at".into()],
        }),
        session: SessionContext {
            user_id: UserId::new(),
            role: "operator".into(),
            scopes: vec!["orders:read".into(), "orders:write".into()],
            locale: "ja-JP".into(),
            tenant_id: TenantId::from("tenant-jp"),
        },
        constraints: SurfaceConstraints::default(),
    }
}

fn source() -> EventSource {
    EventSource {
        app_id: "ops-dashboard".into(),
        surface: "order-detail".into(),
    }
}

fn event(event_type: &str, payload: serde_json::Value) -> AppEvent {
    let mut ev = AppEvent::new(event_type, source(), situation(), SessionId::new());
    ev.payload = payload;
    ev
}

fn mutation() -> StateMutationProposal {
    StateMutationProposal {
        entity_ref: EntityRefLite::new("commerce.Order"),
        field_path: "status".into(),
        from_value: serde_json::json!("pending"),
        to_value: serde_json::json!("cancelled"),
        preview_renderer_hint: Some("diff".into()),
        reversible: true,
        compensation_ref: CompensationRef {
            capability: CapabilityId::from("cap-order-reopen"),
        },
    }
}

fn order_cancel_compensation() -> Option<CompensationRef> {
    Some(CompensationRef {
        capability: CapabilityId::from("cap-order-reopen"),
    })
}

// ════════════════════════════════════════════════════════════════════
// EVENT ROUTING
// ════════════════════════════════════════════════════════════════════

/// 1. Structured event with a small payload routes to FastPath.
#[test]
fn structured_event_routes_to_fast_path() {
    let router = EventRouter::new();
    let ev = event(
        "order.status.check",
        serde_json::json!({"order_id": "ord-42", "status": "shipped"}),
    );
    assert!(
        matches!(router.route(ev), RouteOutcome::FastPath(_)),
        "small structured payload → FastPath"
    );
}

/// 1b. Events with "fast." prefix route to FastPath regardless of payload.
#[test]
fn fast_prefix_routes_to_fast_path() {
    let router = EventRouter::new();
    let ev = event("fast.health-check", serde_json::Value::Null);
    assert!(matches!(router.route(ev), RouteOutcome::FastPath(_)));
}

/// 2. Ambiguous event with null payload routes to AgentInterpret.
#[test]
fn ambiguous_event_routes_to_agent_interpret() {
    let router = EventRouter::new();
    let ev = event("customer.inquiry", serde_json::Value::Null);
    assert!(matches!(router.route(ev), RouteOutcome::AgentInterpret(_)));
}

/// 3. Batch event with array payload decomposes into sub-events.
#[test]
fn batch_event_decomposes_into_sub_events() {
    let router = EventRouter::new();
    let ev = event(
        "batch.orders",
        serde_json::json!([
            {"order_id": "ord-1", "action": "cancel"},
            {"order_id": "ord-2", "action": "ship"}
        ]),
    );
    match router.route(ev) {
        RouteOutcome::Decomposed(subs) => {
            assert_eq!(subs.len(), 2);
            assert_eq!(subs[0].event_type, "batch.orders[0]");
            assert_eq!(subs[1].event_type, "batch.orders[1]");
            // Each sub-event gets its own idempotency key
            assert_ne!(subs[0].event_id, subs[1].event_id);
            // Situation is preserved in each sub-event
            assert_eq!(subs[0].situation.session.tenant_id.as_str(), "tenant-jp");
        }
        other => panic!("expected Decomposed, got {other:?}"),
    }
}

/// 4. RuleBasedAdapter converts known event types to IntentEnvelopes.
#[tokio::test]
async fn adapter_converts_known_event_to_intent() {
    let adapter = RuleBasedAdapter::with_defaults();
    let ev = event("order.cancel.requested", serde_json::Value::Null);
    let intent = adapter.adapt(&ev).await.unwrap();

    assert_eq!(intent.intent_type, IntentType::TransactionalIntent);
    assert_eq!(intent.domain, "order-detail");
    assert_eq!(
        intent.requester.tenant.as_ref().unwrap().as_str(),
        "tenant-jp"
    );
    assert_eq!(intent.entities_in_context.len(), 1);
    assert_eq!(intent.entities_in_context[0].entity_id, "commerce.Order");
}

/// 5. Unknown event types return None (not actionable).
#[tokio::test]
async fn adapter_drops_unknown_event() {
    let adapter = RuleBasedAdapter::with_defaults();
    let ev = event("random.ping", serde_json::Value::Null);
    assert!(adapter.adapt(&ev).await.is_none());
}

/// 6. Surface constraints override the default budget.
#[tokio::test]
async fn surface_constraints_override_budget() {
    let adapter = RuleBasedAdapter::with_defaults();
    let mut ev = event("order.page.opened", serde_json::Value::Null);
    ev.situation.constraints.time_budget_ms = Some(500);
    ev.situation.constraints.cost_budget_usd = Some(0.01);

    let intent = adapter.adapt(&ev).await.unwrap();
    assert_eq!(intent.budget.max_latency_ms, 500);
    assert!((intent.budget.max_cost_usd - 0.01).abs() < 1e-9);
}

// ════════════════════════════════════════════════════════════════════
// PROPOSALS & LIFECYCLE (Rule 20)
// ════════════════════════════════════════════════════════════════════

/// 7. Rule 20: mutations without compensation_ref are rejected.
#[test]
fn rule_20_rejects_mutations_without_compensation() {
    let err = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "Cancel order #42",
        "Customer requested cancellation due to shipping delay",
        vec![mutation()],
        vec![],
        UiHints::default(),
        None, // ← missing compensation
        Utc::now() + Duration::minutes(5),
    )
    .unwrap_err();
    assert!(matches!(
        err,
        SurfaceError::MissingCompensation { count: 1 }
    ));
}

/// 8. Empty mutations need no compensation.
#[test]
fn empty_mutations_need_no_compensation() {
    let p = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "Order status summary",
        "Informational — no state change",
        vec![],
        vec![],
        UiHints::default(),
        None,
        Utc::now() + Duration::minutes(5),
    )
    .unwrap();
    assert_eq!(p.approval_state, ProposalApprovalState::Draft);
    assert!(p.mutations.is_empty());
}

/// 9. Happy-path lifecycle: Draft → Proposed → AppReview → Accepted.
#[test]
fn lifecycle_accept() {
    let lc = ProposalLifecycle;
    let mut p = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "Cancel order #42",
        "Customer requested",
        vec![mutation()],
        vec![],
        UiHints::default(),
        order_cancel_compensation(),
        Utc::now() + Duration::minutes(5),
    )
    .unwrap();

    assert_eq!(p.approval_state, ProposalApprovalState::Draft);
    lc.publish(&mut p).unwrap();
    assert_eq!(p.approval_state, ProposalApprovalState::AppReview);
    lc.accept(&mut p).unwrap();
    assert_eq!(p.approval_state, ProposalApprovalState::Accepted);
}

/// 10. Rejection lifecycle: Draft → Proposed → AppReview → Rejected.
#[test]
fn lifecycle_reject() {
    let lc = ProposalLifecycle;
    let mut p = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "Cancel order #42",
        "Agent recommends cancellation",
        vec![mutation()],
        vec![],
        UiHints::default(),
        order_cancel_compensation(),
        Utc::now() + Duration::minutes(5),
    )
    .unwrap();

    lc.publish(&mut p).unwrap();
    lc.reject(&mut p).unwrap();
    assert_eq!(p.approval_state, ProposalApprovalState::Rejected);
}

/// 11. Transform lifecycle: user edits the proposal.
#[test]
fn lifecycle_transform() {
    let lc = ProposalLifecycle;
    let mut p = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "Cancel order #42",
        "Agent recommends",
        vec![mutation()],
        vec![],
        UiHints::default(),
        order_cancel_compensation(),
        Utc::now() + Duration::minutes(5),
    )
    .unwrap();

    lc.publish(&mut p).unwrap();
    lc.transform(&mut p).unwrap();
    assert_eq!(p.approval_state, ProposalApprovalState::Transformed);
}

/// 12. Expire lifecycle: proposal times out.
#[test]
fn lifecycle_expire() {
    let lc = ProposalLifecycle;
    let mut p = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "Suggestion",
        "Timed out",
        vec![],
        vec![],
        UiHints::default(),
        None,
        Utc::now() + Duration::minutes(5),
    )
    .unwrap();

    lc.publish(&mut p).unwrap();
    lc.expire(&mut p).unwrap();
    assert_eq!(p.approval_state, ProposalApprovalState::Expired);
}

/// 13. Illegal transition: accept before publish is rejected.
#[test]
fn illegal_transition_rejected() {
    let lc = ProposalLifecycle;
    let mut p = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "x",
        "x",
        vec![],
        vec![],
        UiHints::default(),
        None,
        Utc::now() + Duration::minutes(5),
    )
    .unwrap();

    let err = lc.accept(&mut p).unwrap_err();
    assert!(matches!(err, SurfaceError::IllegalTransition { .. }));
}

/// 13b. Cannot accept twice.
#[test]
fn cannot_accept_twice() {
    let lc = ProposalLifecycle;
    let mut p = ActionProposal::build(
        aaf_contracts::IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-jp"),
        "x",
        "x",
        vec![],
        vec![],
        UiHints::default(),
        None,
        Utc::now() + Duration::minutes(5),
    )
    .unwrap();

    lc.publish(&mut p).unwrap();
    lc.accept(&mut p).unwrap();
    let err = lc.accept(&mut p).unwrap_err();
    assert!(matches!(err, SurfaceError::IllegalTransition { .. }));
}

// ════════════════════════════════════════════════════════════════════
// PROJECTIONS (Rule 19)
// ════════════════════════════════════════════════════════════════════

/// 14. Listed fields are readable through the projection.
#[test]
fn projection_allows_listed_fields() {
    let proj = StateProjection::new(
        EntityRefLite::new("commerce.Order"),
        TenantId::from("tenant-jp"),
        vec!["id".into(), "status".into(), "created_at".into()],
        60_000,
    );

    let payload = serde_json::json!({
        "id": "ord-42",
        "status": "pending",
        "total": 9800,
        "created_at": "2026-04-01T10:00:00Z"
    });

    assert_eq!(proj.read_field("id", &payload).unwrap(), "ord-42");
    assert_eq!(proj.read_field("status", &payload).unwrap(), "pending");
    assert!(proj.allows_field("created_at"));
}

/// 15. Unlisted fields are denied (Rule 19 default-deny).
#[test]
fn projection_denies_unlisted_fields() {
    let proj = StateProjection::new(
        EntityRefLite::new("commerce.Order"),
        TenantId::from("tenant-jp"),
        vec!["id".into(), "status".into()],
        60_000,
    );

    let payload = serde_json::json!({"id": "ord-42", "total": 9800});

    let err = proj.read_field("total", &payload).unwrap_err();
    assert_eq!(err, ProjectionError::FieldNotSelected("total".into()));
    assert!(!proj.allows_field("total"));
    assert!(!proj.allows_field("customer_email"));
}

/// 16. Cross-tenant access is rejected.
#[test]
fn projection_rejects_cross_tenant() {
    let proj = StateProjection::new(
        EntityRefLite::new("commerce.Order"),
        TenantId::from("tenant-jp"),
        vec!["id".into()],
        60_000,
    );

    proj.check_tenant(&TenantId::from("tenant-jp")).unwrap();
    let err = proj.check_tenant(&TenantId::from("tenant-us")).unwrap_err();
    assert_eq!(err, ProjectionError::WrongTenant);
}

// ════════════════════════════════════════════════════════════════════
// SITUATION PACKAGER
// ════════════════════════════════════════════════════════════════════

/// 17. Entity refs are forwarded to the intent via the packager.
#[test]
fn packager_forwards_entity_refs() {
    let packager = SituationPackager::default();
    let sit = situation();
    let entities = packager.package_entities(&sit);
    assert_eq!(entities.len(), 1);
    assert_eq!(entities[0].entity_id, "commerce.Order");
}

/// 17b. Screen fields are packaged within the token budget.
#[test]
fn packager_packages_screen_fields() {
    let packager = SituationPackager::default();
    let sit = situation();
    let fields = packager.package_screen_fields(&sit).unwrap();
    assert_eq!(fields, "id,status,created_at");
}

/// 18. Oversized field list exceeds the context budget.
#[test]
fn packager_rejects_oversized_fields() {
    let packager = SituationPackager { budget_tokens: 1 };
    let mut sit = situation();
    sit.current_screen = Some(ScreenContext {
        route: "/orders/42".into(),
        component: "OrderDetail".into(),
        visible_fields: (0..50).map(|i| format!("field_{i}")).collect(),
    });
    let err = packager.package_screen_fields(&sit).unwrap_err();
    assert!(matches!(err, SurfaceError::ContextBudgetExceeded { .. }));
}

// ════════════════════════════════════════════════════════════════════
// FULL PIPELINE: Event → Intent → Execution → Trace
// ════════════════════════════════════════════════════════════════════

/// 19. End-to-end: an order.page.opened event is adapted to an
///     AnalyticalIntent, executed through a single-step graph, and
///     recorded in the trace.
#[tokio::test]
async fn event_to_intent_to_execution_pipeline() {
    // Adapt the event to an intent
    let adapter = RuleBasedAdapter::with_defaults();
    let ev = event("order.page.opened", serde_json::Value::Null);
    let intent = adapter.adapt(&ev).await.unwrap();

    assert_eq!(intent.intent_type, IntentType::AnalyticalIntent);
    assert_eq!(intent.entities_in_context.len(), 1);

    // Execute through a graph
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);

    let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("cap-order-read"),
        SideEffect::Read,
        Arc::new(|_, _| {
            Ok(serde_json::json!({
                "id": "ord-42",
                "status": "pending",
                "total": 9800,
                "created_at": "2026-04-01T10:00:00Z"
            }))
        }),
    ));
    let graph = GraphBuilder::new().add_node(node).build().unwrap();

    let outcome = exec.run(&graph, &intent).await.unwrap();
    match outcome {
        ExecutionOutcome::Completed { steps, outputs } => {
            assert_eq!(steps, 1);
            assert_eq!(
                outputs[&NodeId::from("cap-order-read")].data["status"],
                "pending"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // Verify the trace recorded the step
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
    assert_eq!(trace.steps.len(), 1);
    assert!(trace.steps[0].observation.outcome_detail.is_some());
}

/// 20. YAML config loads and parses successfully.
#[test]
fn aaf_yaml_loads_successfully() {
    let candidates = [
        "examples/app-native-surface/aaf.yaml",
        "../../examples/app-native-surface/aaf.yaml",
        "../../../examples/app-native-surface/aaf.yaml",
    ];
    let yaml = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("aaf.yaml should exist under examples/app-native-surface/");

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&yaml).expect("aaf.yaml should be valid YAML");

    let caps = doc
        .get("capabilities")
        .expect("should have 'capabilities' key")
        .as_sequence()
        .expect("capabilities should be a sequence");
    assert_eq!(caps.len(), 3, "three capabilities: read, cancel, reopen");

    let rules = doc
        .get("event_rules")
        .expect("should have 'event_rules' key");
    assert!(rules.get("order.cancel.requested").is_some());

    let projs = doc
        .get("projections")
        .expect("should have 'projections' key")
        .as_sequence()
        .expect("projections should be a sequence");
    assert_eq!(projs.len(), 1);
    assert_eq!(
        projs[0].get("entity").and_then(|v| v.as_str()).unwrap(),
        "commerce.Order"
    );
}
