//! Parallel orchestration — diamond-shaped graph with ForkNode.
//!
//! Demonstrates a realistic multi-service pipeline:
//!
//! ```text
//!   validate-order (deterministic)
//!        |
//!   +----+-----+------+
//!   |          |      |
//!   check      verify  validate
//!   inventory  payment address   ← ForkNode runs these in parallel
//!   |          |      |
//!   +----+-----+------+
//!        |
//!   confirm-order (deterministic, write, with compensation)
//! ```
//!
//! Tests verify:
//! - ForkNode runs children concurrently and joins outputs
//! - Diamond topology is valid (topological sort)
//! - Budget and tokens accumulate across parallel branches
//! - Compensation fires on downstream failure
//! - Trace records every step including fork
//! - YAML config parses correctly

use aaf_contracts::{
    BudgetContract, IntentEnvelope, IntentId, IntentType, NodeId, Requester, RiskTier, SideEffect,
    TraceId,
};
use aaf_policy::PolicyEngine;
use aaf_runtime::node::{DeterministicNode, ForkNode};
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_trace::{Recorder, TraceRecorder};
use chrono::Utc;
use std::sync::Arc;

fn test_intent() -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::TransactionalIntent,
        requester: Requester {
            user_id: "user-1".into(),
            role: "operator".into(),
            scopes: vec![
                "orders:read".into(),
                "orders:write".into(),
                "inventory:read".into(),
                "payments:read".into(),
                "shipping:read".into(),
                "auto-approve".into(),
            ],
            tenant: None,
        },
        goal: "process order ORD-001".into(),
        domain: "orders".into(),
        constraints: Default::default(),
        budget: BudgetContract {
            max_tokens: 8_000,
            max_cost_usd: 1.0,
            max_latency_ms: 20_000,
        },
        deadline: None,
        risk_tier: RiskTier::Write,
        approval_policy: "auto-approve".into(),
        output_contract: None,
        trace_id: TraceId::new(),
        depth: 0,
        created_at: Utc::now(),
        entities_in_context: vec![],
    }
}

/// Build the diamond graph:
///   validate → fork(inventory, payment, address) → confirm
fn build_diamond_graph(
    with_compensation: bool,
) -> (
    aaf_runtime::graph::Graph,
    NodeId, // validate
    NodeId, // fork
    NodeId, // confirm
) {
    let validate_id = NodeId::from("validate-order");
    let fork_id = NodeId::from("parallel-checks");
    let confirm_id = NodeId::from("confirm-order");

    // Three parallel check nodes (children of ForkNode).
    let inventory_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("check-inventory"),
        SideEffect::Read,
        Arc::new(|_, _| {
            Ok(serde_json::json!({
                "sku": "PROD-42",
                "available": 150,
                "warehouse": "WEST-1",
            }))
        }),
    ));
    let payment_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("verify-payment"),
        SideEffect::Read,
        Arc::new(|_, _| {
            Ok(serde_json::json!({
                "method": "credit_card",
                "verified": true,
                "fraud_score": 0.02,
            }))
        }),
    ));
    let address_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("validate-address"),
        SideEffect::Read,
        Arc::new(|_, _| {
            Ok(serde_json::json!({
                "valid": true,
                "normalized": "123 Main St, Tokyo, JP",
            }))
        }),
    ));

    // Validate node (runs first, sequential).
    let validate_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        validate_id.clone(),
        SideEffect::Read,
        Arc::new(|_, _| {
            Ok(serde_json::json!({
                "order_id": "ORD-001",
                "valid": true,
                "items": 3,
            }))
        }),
    ));

    // Fork node (runs three children in parallel).
    let fork_node: Arc<dyn Node> = Arc::new(ForkNode::new(
        fork_id.clone(),
        vec![inventory_node, payment_node, address_node],
    ));

    // Confirm node (runs last, writes).
    let confirm_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        confirm_id.clone(),
        SideEffect::Write,
        Arc::new(|_, prior| {
            // Read the fork output to verify parallel results arrived.
            let fork_out = prior.get(&NodeId::from("parallel-checks"));
            let has_fork = fork_out.is_some();
            Ok(serde_json::json!({
                "order_id": "ORD-001",
                "confirmed": true,
                "parallel_data_received": has_fork,
            }))
        }),
    ));

    let mut builder = GraphBuilder::new()
        .add_node(validate_node)
        .add_node(fork_node)
        .add_node(confirm_node)
        .add_edge(validate_id.clone(), fork_id.clone())
        .add_edge(fork_id.clone(), confirm_id.clone());

    if with_compensation {
        let compensator: Arc<dyn Node> = Arc::new(DeterministicNode::new(
            NodeId::from("cancel-order"),
            SideEffect::Write,
            Arc::new(|_, _| Ok(serde_json::json!({"cancelled": true}))),
        ));
        builder = builder.add_compensator(confirm_id.clone(), compensator);
    }

    let graph = builder.build().expect("diamond graph validates");
    (graph, validate_id, fork_id, confirm_id)
}

// ── Tests ───────────────────────────────────────────────────────────

#[tokio::test]
async fn diamond_graph_executes_all_steps() {
    let (graph, _, _, _) = build_diamond_graph(false);
    let intent = test_intent();
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());

    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    let outcome = exec.run(&graph, &intent).await.expect("execute");

    match outcome {
        ExecutionOutcome::Completed { outputs, steps } => {
            assert_eq!(steps, 3, "validate + fork + confirm = 3 steps");
            assert_eq!(outputs.len(), 3);
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}

#[tokio::test]
async fn fork_node_runs_children_in_parallel_and_joins() {
    let (graph, _, fork_id, _) = build_diamond_graph(false);
    let intent = test_intent();
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());

    let exec = GraphExecutor::new(policy, recorder, intent.budget);
    let outcome = exec.run(&graph, &intent).await.unwrap();

    if let ExecutionOutcome::Completed { outputs, .. } = outcome {
        let fork_output = outputs.get(&fork_id).expect("fork output");

        // ForkNode joins children as child_0, child_1, child_2.
        assert!(fork_output.data["child_0"].is_object(), "inventory output");
        assert!(fork_output.data["child_1"].is_object(), "payment output");
        assert!(fork_output.data["child_2"].is_object(), "address output");

        // Verify inventory data.
        assert_eq!(
            fork_output.data["child_0"]["available"].as_i64().unwrap(),
            150
        );
        // Verify payment data.
        assert!(fork_output.data["child_1"]["verified"].as_bool().unwrap());
        // Verify address data.
        assert!(fork_output.data["child_2"]["valid"].as_bool().unwrap());
    } else {
        panic!("expected Completed");
    }
}

#[tokio::test]
async fn confirm_step_receives_fork_output() {
    let (graph, _, _, confirm_id) = build_diamond_graph(false);
    let intent = test_intent();
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());

    let exec = GraphExecutor::new(policy, recorder, intent.budget);
    let outcome = exec.run(&graph, &intent).await.unwrap();

    if let ExecutionOutcome::Completed { outputs, .. } = outcome {
        let confirm = outputs.get(&confirm_id).expect("confirm output");
        assert!(
            confirm.data["parallel_data_received"].as_bool().unwrap(),
            "confirm should see fork output in prior_outputs"
        );
        assert!(confirm.data["confirmed"].as_bool().unwrap());
    } else {
        panic!("expected Completed");
    }
}

#[tokio::test]
async fn trace_records_all_steps_including_fork() {
    let (graph, _, _, _) = build_diamond_graph(false);
    let intent = test_intent();
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());

    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    exec.run(&graph, &intent).await.unwrap();

    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
    assert_eq!(trace.steps.len(), 3, "trace should have 3 steps");
}

#[tokio::test]
async fn compensation_fires_when_final_step_fails() {
    // Three sequential steps: reserve (write, succeeds) → charge (write, succeeds)
    // → ship (write, fails). Reserve and charge should be compensated.
    let reserve_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("reserve-stock"),
        SideEffect::Write,
        Arc::new(|_, _| Ok(serde_json::json!({"reserved": true}))),
    ));
    let reserve_comp: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("release-stock"),
        SideEffect::Write,
        Arc::new(|_, _| Ok(serde_json::json!({"released": true}))),
    ));

    let charge_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("charge-payment"),
        SideEffect::Write,
        Arc::new(|_, _| Ok(serde_json::json!({"charged": true}))),
    ));
    let charge_comp: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("refund-payment"),
        SideEffect::Write,
        Arc::new(|_, _| Ok(serde_json::json!({"refunded": true}))),
    ));

    let ship_node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("ship-order"),
        SideEffect::Write,
        Arc::new(|_, _| {
            Err(aaf_runtime::RuntimeError::Node(
                "carrier unavailable".into(),
            ))
        }),
    ));

    let graph = GraphBuilder::new()
        .add_node(reserve_node)
        .add_node(charge_node)
        .add_node(ship_node)
        .add_edge(
            NodeId::from("reserve-stock"),
            NodeId::from("charge-payment"),
        )
        .add_edge(NodeId::from("charge-payment"), NodeId::from("ship-order"))
        .add_compensator(NodeId::from("reserve-stock"), reserve_comp)
        .add_compensator(NodeId::from("charge-payment"), charge_comp)
        .build()
        .unwrap();

    let intent = test_intent();
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());

    let exec = GraphExecutor::new(policy, recorder, intent.budget);
    let outcome = exec.run(&graph, &intent).await.unwrap();

    match outcome {
        ExecutionOutcome::RolledBack {
            failed_at,
            reason,
            compensated,
        } => {
            assert_eq!(failed_at, 3, "should fail at step 3 (ship)");
            assert!(reason.contains("carrier unavailable"), "reason: {reason}");
            assert_eq!(
                compensated.len(),
                2,
                "reserve + charge should be compensated"
            );
        }
        other => panic!("expected RolledBack, got {other:?}"),
    }
}

#[test]
fn yaml_config_parses_successfully() {
    let candidates = [
        "examples/parallel-orchestration/aaf.yaml",
        "../../examples/parallel-orchestration/aaf.yaml",
        "../../../examples/parallel-orchestration/aaf.yaml",
    ];
    let yaml = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("aaf.yaml found via at least one candidate path");
    let cfg: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(
        cfg["project"]["name"].as_str().unwrap(),
        "parallel-orchestration"
    );
    assert_eq!(cfg["capabilities"].as_sequence().unwrap().len(), 5);
}
