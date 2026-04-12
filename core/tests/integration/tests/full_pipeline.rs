//! Full pipeline: Intent Compiler → Planner → Runtime → Trace.
//!
//! Exercises every public surface of the iteration-1 / iteration-2
//! foundation as one integrated story:
//!
//! 1. The intent compiler produces a validated `IntentEnvelope` from
//!    natural-language input.
//! 2. The planner discovers a capability in the registry and produces
//!    an execution plan.
//! 3. The runtime executes a graph derived from the plan, with the
//!    policy engine gating every step (Rule 6) and the trace recorder
//!    capturing every observation (Rule 12).
//! 4. The recorded trace can be inspected and metrics computed.
//!
//! This test is the canonical "does AAF actually work end-to-end"
//! check; if it ever breaks, something fundamental in the wiring has
//! regressed.

use aaf_contracts::{
    BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
    CapabilitySla, DataClassification, EndpointKind, NodeId, Requester, SideEffect,
};
use aaf_intent::{compiler::CompileOutcome, IntentCompiler};
use aaf_planner::{BoundedAutonomy, CompositionChecker, RegistryPlanner};
use aaf_policy::PolicyEngine;
use aaf_registry::Registry;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_trace::{Recorder, TraceMetrics, TraceRecorder};
use std::sync::Arc;

fn cap(id: &str, name: &str, desc: &str) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: name.into(),
        description: desc.into(),
        version: "1.0.0".into(),
        provider_agent: "sales-agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::Grpc,
            address: "sales:50051".into(),
            method: Some("Query".into()),
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
        reads: vec![],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.5,
        learned_rules: vec![],
    }
}

#[tokio::test]
async fn intent_compiler_to_planner_to_runtime_end_to_end() {
    // ── Wire core components ─────────────────────────────────────────
    let registry = Arc::new(Registry::in_memory());
    registry
        .register(cap(
            "cap-sales-monthly",
            "monthly sales report",
            "produce a monthly sales report grouped by region",
        ))
        .await
        .unwrap();

    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());

    // ── Compile a natural-language intent ────────────────────────────
    let compiler = IntentCompiler::default();
    let outcome = compiler
        .compile(
            "show last month's sales by region",
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
        CompileOutcome::Compiled(env) => env,
        CompileOutcome::NeedsRefinement(qs) => panic!("unexpected refinement: {qs:?}"),
    };
    intent.validate().expect("envelope validates");

    // ── Plan ─────────────────────────────────────────────────────────
    let planner = RegistryPlanner::new(
        registry.clone(),
        BoundedAutonomy::default(),
        CompositionChecker::default(),
    );
    let plan = planner.plan(&intent).await.expect("plan");
    assert!(!plan.is_empty(), "plan must contain at least one step");

    // ── Materialise the plan into a runtime Graph ───────────────────
    let mut builder = GraphBuilder::new();
    let mut prev: Option<NodeId> = None;
    for step in &plan.steps {
        let cap_id = step.capability.clone();
        let node_id = NodeId::from(cap_id.as_str());
        let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
            node_id.clone(),
            SideEffect::Read,
            Arc::new(move |_, _| Ok(serde_json::json!({"ran": "true", "rows": 47}))),
        ));
        builder = builder.add_node(node);
        if let Some(p) = prev.take() {
            builder = builder.add_edge(p, node_id.clone());
        }
        prev = Some(node_id);
    }
    let graph = builder.build().expect("graph validates");

    // ── Execute ──────────────────────────────────────────────────────
    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    let outcome = exec.run(&graph, &intent).await.expect("execute");
    match outcome {
        ExecutionOutcome::Completed { steps, outputs } => {
            assert_eq!(steps as usize, plan.steps.len());
            assert_eq!(outputs.len(), plan.steps.len());
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // ── Inspect the trace ────────────────────────────────────────────
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
    assert_eq!(trace.steps.len(), plan.steps.len());

    let metrics = TraceMetrics::compute(std::iter::once(&trace));
    assert_eq!(metrics.total_traces, 1);
    assert_eq!(metrics.completed, 1);
    assert!((metrics.intent_resolution_rate - 1.0).abs() < 1e-9);
}
