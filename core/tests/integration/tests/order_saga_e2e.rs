//! End-to-end test for examples/order-saga.
//!
//! Exercises the full AAF e-commerce saga story:
//!
//! 1. **Happy path** — stock check (read) → payment (write) →
//!    shipping (write) → all three complete, trace records every step
//!    with outcome details.
//!
//! 2. **Compensation rollback** — shipping fails at step 3 → the
//!    payment at step 2 is compensated (refunded) → the stock check
//!    at step 1 is preserved because it has no write side-effect.
//!
//! 3. **Shadow mode** — the same graph runs with `with_shadow()`
//!    enabled, so write nodes produce `{"shadow": true, ...}` without
//!    executing, while read nodes still run normally.
//!
//! 4. **Saga definition YAML** — the saga.yaml from
//!    examples/order-saga/ is loaded and parsed successfully.
//!
//! Run this test with:
//!
//!     cargo test -p aaf-integration-tests --test order_saga_e2e

use aaf_contracts::{
    BudgetContract, IntentEnvelope, IntentId, IntentType, NodeId, Requester, RiskTier, SideEffect,
    TraceId,
};
use aaf_policy::PolicyEngine;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node, RuntimeError};
use aaf_saga::SagaDefinition;
use aaf_trace::{Recorder, TraceRecorder};
use chrono::Utc;
use std::sync::Arc;

// ── Helpers ─────────────────────────────────────────────────────────

fn intent() -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::TransactionalIntent,
        requester: Requester {
            user_id: "user-tanaka".into(),
            role: "customer".into(),
            scopes: vec![
                "inventory:read".into(),
                "payment:execute".into(),
                "shipping:write".into(),
                "auto-approve".into(),
            ],
            tenant: None,
        },
        goal: "create order for product SKU-1".into(),
        domain: "ecommerce".into(),
        constraints: Default::default(),
        budget: BudgetContract {
            max_tokens: 5_000,
            max_cost_usd: 1.0,
            max_latency_ms: 30_000,
        },
        deadline: None,
        risk_tier: RiskTier::Write,
        approval_policy: "auto".into(),
        output_contract: None,
        trace_id: TraceId::new(),
        depth: 0,
        created_at: Utc::now(),
        entities_in_context: vec![],
    }
}

fn det(id: &str, se: SideEffect, output: serde_json::Value) -> Arc<dyn Node> {
    Arc::new(DeterministicNode::new(
        NodeId::from(id),
        se,
        Arc::new(move |_, _| Ok(output.clone())),
    ))
}

fn failing(id: &str) -> Arc<dyn Node> {
    Arc::new(DeterministicNode::new(
        NodeId::from(id),
        SideEffect::Write,
        Arc::new(|_, _| {
            Err(RuntimeError::Node(
                "shipping failure: address_invalid".into(),
            ))
        }),
    ))
}

fn compensator(id: &str) -> Arc<dyn Node> {
    Arc::new(DeterministicNode::new(
        NodeId::from(id),
        SideEffect::None,
        Arc::new(|_, _| Ok(serde_json::json!({"compensated": true}))),
    ))
}

fn build_happy_graph() -> aaf_runtime::Graph {
    GraphBuilder::new()
        .add_node(det(
            "stock-check",
            SideEffect::Read,
            serde_json::json!({"available": 42, "sku": "SKU-1"}),
        ))
        .add_node(det(
            "payment",
            SideEffect::Payment,
            serde_json::json!({"payment_id": "pay-001", "status": "captured"}),
        ))
        .add_node(det(
            "shipping",
            SideEffect::Write,
            serde_json::json!({"tracking": "JP-12345", "eta": "2026-04-14"}),
        ))
        .add_edge(NodeId::from("stock-check"), NodeId::from("payment"))
        .add_edge(NodeId::from("payment"), NodeId::from("shipping"))
        .add_compensator(NodeId::from("payment"), compensator("payment-refund"))
        .add_compensator(NodeId::from("shipping"), compensator("shipping-cancel"))
        .build()
        .unwrap()
}

fn build_failing_graph() -> aaf_runtime::Graph {
    GraphBuilder::new()
        .add_node(det(
            "stock-check",
            SideEffect::Read,
            serde_json::json!({"available": 42}),
        ))
        .add_node(det(
            "payment",
            SideEffect::Payment,
            serde_json::json!({"payment_id": "pay-001"}),
        ))
        .add_node(failing("shipping"))
        .add_edge(NodeId::from("stock-check"), NodeId::from("payment"))
        .add_edge(NodeId::from("payment"), NodeId::from("shipping"))
        .add_compensator(NodeId::from("payment"), compensator("payment-refund"))
        .build()
        .unwrap()
}

// ── Tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn happy_path_completes_three_steps_with_trace() {
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        recorder.clone(),
        intent().budget,
    );
    let i = intent();
    let outcome = exe.run(&build_happy_graph(), &i).await.unwrap();

    match outcome {
        ExecutionOutcome::Completed { steps, outputs } => {
            assert_eq!(steps, 3, "three steps: stock → payment → shipping");
            assert_eq!(outputs[&NodeId::from("stock-check")].data["available"], 42);
            assert_eq!(outputs[&NodeId::from("payment")].data["status"], "captured");
            assert_eq!(
                outputs[&NodeId::from("shipping")].data["tracking"],
                "JP-12345"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // Verify trace has all 3 steps with outcome_detail (Rule 12 + E1).
    let trace = recorder.get(&i.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
    assert_eq!(trace.steps.len(), 3);
    for step in &trace.steps {
        assert!(
            step.observation.outcome_detail.is_some(),
            "step {} missing outcome_detail",
            step.step
        );
    }
}

#[tokio::test]
async fn shipping_failure_compensates_payment_only() {
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        intent().budget,
    );
    let outcome = exe.run(&build_failing_graph(), &intent()).await.unwrap();

    match outcome {
        ExecutionOutcome::RolledBack {
            failed_at,
            reason,
            compensated,
        } => {
            assert_eq!(failed_at, 3, "failure at step 3 (shipping)");
            assert!(
                reason.contains("address_invalid"),
                "reason should mention the failure cause"
            );
            // Payment was compensated; stock-check is read-only → no compensation.
            assert_eq!(compensated.len(), 1, "only payment should be compensated");
            assert_eq!(
                compensated[0].as_str(),
                "payment",
                "the compensated node should be 'payment'"
            );
        }
        other => panic!("expected RolledBack, got {other:?}"),
    }
}

#[tokio::test]
async fn shadow_mode_records_but_does_not_execute_writes() {
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        recorder.clone(),
        intent().budget,
    )
    .with_shadow();

    let i = intent();
    let outcome = exe.run(&build_happy_graph(), &i).await.unwrap();

    match outcome {
        ExecutionOutcome::Completed { steps, outputs } => {
            assert_eq!(steps, 3);
            // Read node ran normally.
            assert_eq!(
                outputs[&NodeId::from("stock-check")].data["available"],
                42,
                "read nodes execute in shadow mode"
            );
            // Write nodes were shadowed.
            assert_eq!(
                outputs[&NodeId::from("payment")].data["shadow"],
                true,
                "payment should be shadowed"
            );
            assert_eq!(
                outputs[&NodeId::from("shipping")].data["shadow"],
                true,
                "shipping should be shadowed"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // Trace still records all 3 steps (observability is full even in shadow).
    let trace = recorder.get(&i.trace_id).await.unwrap();
    assert_eq!(trace.steps.len(), 3);
}

#[test]
fn saga_yaml_parses_successfully() {
    // cargo test runs from the workspace root (where Cargo.toml is),
    // so the path is relative to the repo root.
    let candidates = [
        "examples/order-saga/saga.yaml",
        "../../examples/order-saga/saga.yaml",
        "../../../examples/order-saga/saga.yaml",
    ];
    let yaml = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("saga.yaml should exist under examples/order-saga/");
    let saga = SagaDefinition::from_yaml(&yaml).expect("saga.yaml should parse");
    assert_eq!(saga.name, "order-processing");
    assert_eq!(saga.steps.len(), 3);
    assert_eq!(saga.steps[0].name, "Stock Check");
    assert_eq!(saga.steps[1].name, "Payment Execute");
    assert_eq!(saga.steps[2].name, "Shipping Arrange");
    assert!(
        saga.steps[2].on_failure.is_some(),
        "step 3 should have intelligent recovery rules"
    );
    let rules = &saga.steps[2].on_failure.as_ref().unwrap().rules;
    assert_eq!(rules.len(), 3, "three recovery rules for shipping");
    assert_eq!(rules[0].condition, "address_invalid");
}
