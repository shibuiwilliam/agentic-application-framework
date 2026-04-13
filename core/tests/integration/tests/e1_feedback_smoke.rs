//! E1 feedback-spine smoke test: Observation carries an Outcome after
//! runtime execution, and `aaf-eval` can ingest a trace and produce a
//! regression report.

use aaf_contracts::{
    BudgetContract, IntentEnvelope, IntentId, IntentType, NodeId, Requester, RiskTier, SideEffect,
    TraceId,
};
use aaf_eval::{DeterministicJudge, GoldenSuite, RegressionReport};
use aaf_policy::PolicyEngine;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{GraphBuilder, GraphExecutor, Node};
use aaf_trace::{Recorder, TraceRecorder};
use chrono::Utc;
use std::sync::Arc;

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
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
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

#[tokio::test]
async fn every_trace_step_carries_a_minimal_outcome() {
    let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("n1"),
        SideEffect::None,
        Arc::new(|_, _| Ok(serde_json::json!({"ok": true}))),
    ));
    let g = GraphBuilder::new().add_node(node).build().unwrap();
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        recorder.clone(),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    );
    let i = intent();
    let _ = exe.run(&g, &i).await.unwrap();

    let trace = recorder.get(&i.trace_id).await.unwrap();
    assert_eq!(trace.steps.len(), 1);
    let step = &trace.steps[0];
    // Rule 15 — the runtime must attach a structured outcome at
    // step-end. Slice A's minimum payload: status + latency + tokens
    // + cost.
    let outcome = step
        .observation
        .outcome_detail
        .as_ref()
        .expect("runtime must attach outcome_detail");
    assert_eq!(outcome.status, aaf_contracts::OutcomeStatus::Succeeded);
}

#[tokio::test]
async fn golden_suite_and_regression_report_interop() {
    let suite = GoldenSuite::from_yaml(
        r"
name: smoke
threshold: 0.5
cases:
  - id: a
    intent: hello
    expected: hello world
",
    )
    .unwrap();
    let judge = DeterministicJudge::default();
    let baseline = suite.run(|_| "hello world".into(), &judge).await;
    let candidate = suite.run(|_| "goodbye".into(), &judge).await;
    let rep = RegressionReport::build(&baseline, &candidate);
    assert!(rep.has_regression());
    assert_eq!(rep.regressions, 1);
}
