//! Full enhanced pipeline: Intent Compiler (with entity enrichment) →
//! Planner (with composition safety) → Runtime (with policy + scope
//! intersection + trace outcome) → verify everything works together.
//!
//! This is the capstone integration test that proves all the
//! enhancement slices (E1, E2, E3, X1) integrate correctly with the
//! core pipeline from PROJECT.md.

use aaf_contracts::{
    AutonomyLevel, BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint,
    CapabilityId, CapabilitySla, DataClassification, EndpointKind, EntityRefLite, IntentType,
    NodeId, Requester, SideEffect,
};
use aaf_intent::{compiler::CompileOutcome, IntentCompiler};
use aaf_planner::{BoundedAutonomy, CompositionChecker, RegistryPlanner};
use aaf_policy::{effective_scopes, PolicyEngine};
use aaf_registry::Registry;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_trace::{Recorder, TraceMetrics, TraceRecorder};
use std::sync::Arc;

fn cap(id: &str, name: &str, desc: &str, reads: Vec<&str>) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: name.into(),
        description: desc.into(),
        version: "1.0.0".into(),
        provider_agent: "test-agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::Grpc,
            address: "svc:50051".into(),
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
        domains: vec!["sales".into()],
        reads: reads.iter().map(|e| EntityRefLite::new(*e)).collect(),
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.9,
        learned_rules: vec![],
    }
}

#[tokio::test]
async fn capstone_enhanced_pipeline() {
    // ── 1. Scope intersection (§8.2) ────────────────────────────────
    let user_scopes = vec![
        "sales:read".into(),
        "sales:write".into(),
        "admin:delete".into(),
    ];
    let intent_scopes = vec!["sales:read".into()];
    let effective = effective_scopes(&user_scopes, &intent_scopes, AutonomyLevel::Level3);
    assert_eq!(
        effective,
        vec!["sales:read"],
        "L3 should keep only reads from the intent intersection"
    );

    // ── 2. Registry with entity-aware capabilities ──────────────────
    let registry = Arc::new(Registry::in_memory());
    registry
        .register(cap(
            "cap-sales-monthly",
            "monthly sales report",
            "produce a monthly sales report grouped by region",
            vec!["commerce.Order", "commerce.Customer"],
        ))
        .await
        .unwrap();

    // ── 3. Intent compilation ───────────────────────────────────────
    let compiler = IntentCompiler::default();
    let outcome = compiler
        .compile(
            "show last month sales by region",
            Requester {
                user_id: "tanaka".into(),
                role: "analyst".into(),
                scopes: vec!["sales:read".into()],
                tenant: None,
            },
            "sales",
            BudgetContract {
                max_tokens: 5_000,
                max_cost_usd: 1.0,
                max_latency_ms: 30_000,
            },
        )
        .expect("compile");
    let intent = match outcome {
        CompileOutcome::Compiled(env) => {
            assert_eq!(env.intent_type, IntentType::AnalyticalIntent);
            env
        }
        CompileOutcome::NeedsRefinement(qs) => panic!("unexpected refinement: {qs:?}"),
    };
    intent.validate().expect("envelope validates");

    // ── 4. Plan against the registry ────────────────────────────────
    let planner = RegistryPlanner::new(
        registry.clone(),
        BoundedAutonomy {
            max_cost_usd: intent.budget.max_cost_usd,
            ..BoundedAutonomy::default()
        },
        CompositionChecker::default(),
    );
    let plan = planner.plan(&intent).await.expect("plan");
    assert!(!plan.is_empty());

    // ── 5. Execute the graph ────────────────────────────────────────
    let mut builder = GraphBuilder::new();
    for step in &plan.steps {
        let node_id = NodeId::from(step.capability.as_str());
        let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
            node_id,
            SideEffect::Read,
            Arc::new(|_, _| {
                Ok(serde_json::json!({"rows": 47, "region": "tokyo", "total": 1234.0}))
            }),
        ));
        builder = builder.add_node(node);
    }
    let graph = builder.build().expect("graph");

    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let exec = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        recorder.clone(),
        intent.budget,
    );
    let outcome = exec.run(&graph, &intent).await.expect("execute");
    match &outcome {
        ExecutionOutcome::Completed { steps, outputs } => {
            assert_eq!(*steps as usize, plan.steps.len());
            assert_eq!(outputs.len(), plan.steps.len());
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // ── 6. Verify trace quality (E1 Slice A + Rule 12) ──────────────
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
    assert_eq!(trace.steps.len(), plan.steps.len());
    for step in &trace.steps {
        assert!(
            step.observation.outcome_detail.is_some(),
            "step {} missing outcome_detail (E1 Rule 15)",
            step.step
        );
    }

    // ── 7. Compute trace metrics ────────────────────────────────────
    let metrics = TraceMetrics::compute(std::iter::once(&trace));
    assert_eq!(metrics.total_traces, 1);
    assert_eq!(metrics.completed, 1);
    assert!((metrics.intent_resolution_rate - 1.0).abs() < 1e-9);

    // ── 8. Verify entity-aware capability had reads declared ────────
    let registered = registry
        .get(&CapabilityId::from("cap-sales-monthly"))
        .await
        .unwrap();
    assert_eq!(
        registered.reads.len(),
        2,
        "capability should declare 2 entity reads"
    );
    assert_eq!(registered.reads[0].entity_id, "commerce.Order");
    assert_eq!(registered.reads[1].entity_id, "commerce.Customer");
}
