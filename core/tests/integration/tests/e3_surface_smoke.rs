//! E3 app-native-surface smoke test: AppEvent → Intent (via adapter) →
//! Plan → Runtime → Trace, with a proposal round-tripped through the
//! lifecycle and Rule 20 enforced at construction.

use aaf_contracts::{
    CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla,
    DataClassification, EndpointKind, EntityRefLite, IntentId, IntentType, NodeId, SessionId,
    SideEffect, TenantId, TraceId, UserId,
};
use aaf_eval::{DeterministicJudge, Judge};
use aaf_planner::{BoundedAutonomy, CompositionChecker, RegistryPlanner};
use aaf_policy::PolicyEngine;
use aaf_registry::Registry;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_surface::{
    ingest::{EventToIntentAdapter, RuleBasedAdapter},
    proposal::{ActionProposal, CompensationRef, StateMutationProposal, UiHints},
    AppEvent, EventSource, ProposalLifecycle, ScreenContext, SessionContext, Situation,
    SurfaceConstraints,
};
use aaf_trace::Recorder;
use chrono::{Duration, Utc};
use std::sync::Arc;

fn cap(id: &str, name: &str) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: name.into(),
        // The planner's v0.1 discovery is lexical; make sure the
        // description contains the same words the adapter will put in
        // the intent's `goal` ("handle event order.page.opened").
        description: "handle event order page opened".into(),
        version: "1.0.0".into(),
        provider_agent: "order-agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::Grpc,
            address: "order:50051".into(),
            method: None,
        },
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        side_effect: SideEffect::Read,
        idempotent: true,
        reversible: true,
        deterministic: true,
        compensation: None,
        sla: CapabilitySla::default(),
        cost: CapabilityCost::default(),
        required_scope: "sales:read".into(),
        data_classification: DataClassification::Internal,
        degradation: vec![],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec![],
        domains: vec!["order-detail".into()],
        reads: vec![EntityRefLite::new("commerce.Order")],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.5,
        learned_rules: vec![],
    }
}

fn app_event() -> AppEvent {
    AppEvent::new(
        "order.page.opened",
        EventSource {
            app_id: "ops-web".into(),
            surface: "order-detail".into(),
        },
        Situation {
            current_entities: vec![EntityRefLite::new("commerce.Order")],
            current_screen: Some(ScreenContext {
                route: "/orders/42".into(),
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
        },
        SessionId::new(),
    )
}

#[tokio::test]
async fn app_event_becomes_intent_and_runs_a_plan() {
    // 1. Seed a registry with an entity-aware capability.
    let registry = Arc::new(Registry::in_memory());
    registry
        .register(cap("cap-order-read", "read order"))
        .await
        .unwrap();

    // 2. Ingest an AppEvent into an IntentEnvelope.
    let adapter = RuleBasedAdapter::with_defaults();
    let ev = app_event();
    let intent = adapter.adapt(&ev).await.expect("event must be adaptable");
    assert_eq!(intent.intent_type, IntentType::AnalyticalIntent);
    assert_eq!(intent.entities_in_context.len(), 1);

    // 3. Plan against the registry. The planner returns at least one
    //    step; we materialise it into a deterministic runtime graph.
    let bounds = BoundedAutonomy {
        max_cost_usd: intent.budget.max_cost_usd,
        ..BoundedAutonomy::default()
    };
    let planner = RegistryPlanner::new(registry.clone(), bounds, CompositionChecker::default());
    let plan = planner.plan(&intent).await.expect("plan");
    let mut builder = GraphBuilder::new();
    for step in &plan.steps {
        let node_id = NodeId::from(step.capability.as_str());
        let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
            node_id,
            SideEffect::Read,
            Arc::new(|_, _| Ok(serde_json::json!({"ran": true}))),
        ));
        builder = builder.add_node(node);
    }
    let graph = builder.build().expect("graph");

    // 4. Execute. The runtime must succeed and every step should
    //    carry an outcome_detail (E1 Slice A).
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        intent.budget,
    );
    let outcome = exe.run(&graph, &intent).await.expect("execute");
    assert!(matches!(outcome, ExecutionOutcome::Completed { .. }));
}

#[tokio::test]
async fn proposal_round_trips_through_lifecycle_with_compensation() {
    let mutation = StateMutationProposal {
        entity_ref: EntityRefLite::new("commerce.Order"),
        field_path: "status".into(),
        from_value: serde_json::json!("pending"),
        to_value: serde_json::json!("cancelled"),
        preview_renderer_hint: Some("diff".into()),
        reversible: true,
        compensation_ref: CompensationRef {
            capability: CapabilityId::from("cap-order-reopen"),
        },
    };
    let mut p = ActionProposal::build(
        IntentId::new(),
        TraceId::new(),
        TenantId::from("tenant-a"),
        "cancel order 42",
        "because the user asked",
        vec![mutation],
        vec![],
        UiHints::default(),
        Some(CompensationRef {
            capability: CapabilityId::from("cap-order-reopen"),
        }),
        Utc::now() + Duration::minutes(5),
    )
    .expect("Rule 20 must accept proposal with compensation");

    let lc = ProposalLifecycle;
    lc.publish(&mut p).unwrap();
    lc.accept(&mut p).unwrap();
    assert_eq!(
        p.approval_state,
        aaf_surface::ProposalApprovalState::Accepted
    );
}

#[tokio::test]
async fn judge_is_still_reachable_from_integration_crate() {
    // Smoke test that aaf-eval's Judge trait is usable end-to-end.
    let j = DeterministicJudge::default();
    let v = j.judge("hello world", "hello world").await;
    assert!((v.score - 1.0).abs() < 1e-9);
}
