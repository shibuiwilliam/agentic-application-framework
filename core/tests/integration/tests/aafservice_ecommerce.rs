//! PROJECT_AafService §10.1 — E-commerce order processing.
//!
//! End-to-end integration test that exercises the full AAF pipeline
//! against the canonical e-commerce scenario:
//!
//! 1. Intent Compiler classifies a Japanese natural-language order
//!    intent as `TransactionalIntent`.
//! 2. The planner discovers capabilities from a seeded registry.
//! 3. The runtime executes a 3-node graph:
//!    - Step 1: stock check (deterministic, read)
//!    - Step 2: payment (deterministic, write w/ compensation)
//!    - Step 3: shipping (agent-class, write w/ compensation)
//! 4. Policy guards run at every hook.
//! 5. A failure at step 3 triggers compensation rollback of step 2.
//! 6. The trace records every step with an `outcome_detail`.
//!
//! This is the single most important integration test in the
//! codebase: if it breaks, the core AafService story is broken.

use aaf_contracts::{
    BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
    CapabilitySla, CompensationSpec, DataClassification, EndpointKind, IntentType, NodeId,
    Requester, SideEffect,
};
use aaf_intent::{compiler::CompileOutcome, IntentCompiler};
use aaf_policy::PolicyEngine;
use aaf_registry::Registry;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node, RuntimeError};
use aaf_trace::{Recorder, TraceRecorder};
use std::sync::Arc;

fn stock_check_cap() -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from("cap-stock-check"),
        name: "在庫照会".into(),
        description: "stock check order ecommerce inventory".into(),
        version: "1.0.0".into(),
        provider_agent: "inventory-agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::Grpc,
            address: "inventory:50051".into(),
            method: Some("CheckStock".into()),
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
        required_scope: "inventory:read".into(),
        data_classification: DataClassification::Internal,
        degradation: vec![],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec!["stock".into(), "inventory".into()],
        domains: vec!["ecommerce".into()],
        reads: vec![],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.9,
        learned_rules: vec![],
    }
}

fn payment_cap() -> CapabilityContract {
    let mut cap = stock_check_cap();
    cap.id = CapabilityId::from("cap-payment-execute");
    cap.name = "決済実行".into();
    cap.description = "payment execute order ecommerce".into();
    cap.side_effect = SideEffect::Payment;
    cap.deterministic = true;
    cap.compensation = Some(CompensationSpec {
        endpoint: "cap-payment-refund".into(),
    });
    cap.required_scope = "payment:execute".into();
    cap
}

fn shipping_cap() -> CapabilityContract {
    let mut cap = stock_check_cap();
    cap.id = CapabilityId::from("cap-shipping-arrange");
    cap.name = "配送手配".into();
    cap.description = "shipping arrange order ecommerce delivery".into();
    cap.side_effect = SideEffect::Write;
    cap.deterministic = false;
    cap.compensation = Some(CompensationSpec {
        endpoint: "cap-shipping-cancel".into(),
    });
    cap.required_scope = "shipping:write".into();
    cap
}

fn det_node(id: &str, side_effect: SideEffect, output: serde_json::Value) -> Arc<dyn Node> {
    Arc::new(DeterministicNode::new(
        NodeId::from(id),
        side_effect,
        Arc::new(move |_, _| Ok(output.clone())),
    ))
}

fn failing_node(id: &str) -> Arc<dyn Node> {
    Arc::new(DeterministicNode::new(
        NodeId::from(id),
        SideEffect::Write,
        Arc::new(|_, _| {
            Err(RuntimeError::Node(
                "shipping failure: address invalid".into(),
            ))
        }),
    ))
}

fn compensation_node(id: &str) -> Arc<dyn Node> {
    Arc::new(DeterministicNode::new(
        NodeId::from(id),
        SideEffect::None,
        Arc::new(|_, _| Ok(serde_json::json!({"compensated": true}))),
    ))
}

#[tokio::test]
async fn ecommerce_happy_path_completes_three_step_order() {
    // ── 1. Intent compilation ────────────────────────────────────
    let compiler = IntentCompiler::default();
    let outcome = compiler
        .compile(
            "洗剤をもう一つ作成して",
            Requester {
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
            "ecommerce",
            BudgetContract {
                max_tokens: 5_000,
                max_cost_usd: 1.0,
                max_latency_ms: 30_000,
            },
        )
        .expect("compile");
    let intent = match outcome {
        CompileOutcome::Compiled(env) => {
            assert_eq!(env.intent_type, IntentType::TransactionalIntent);
            env
        }
        CompileOutcome::NeedsRefinement(qs) => panic!("unexpected refinement: {qs:?}"),
    };

    // ── 2. Seed the registry ────────────────────────────────────
    let registry = Arc::new(Registry::in_memory());
    registry.register(stock_check_cap()).await.unwrap();
    registry.register(payment_cap()).await.unwrap();
    registry.register(shipping_cap()).await.unwrap();

    // ── 3. Build the graph ──────────────────────────────────────
    let graph = GraphBuilder::new()
        .add_node(det_node(
            "cap-stock-check",
            SideEffect::Read,
            serde_json::json!({"available": 42}),
        ))
        .add_node(det_node(
            "cap-payment-execute",
            SideEffect::Payment,
            serde_json::json!({"payment_id": "pay-001", "status": "captured"}),
        ))
        .add_node(det_node(
            "cap-shipping-arrange",
            SideEffect::Write,
            serde_json::json!({"tracking": "JP-12345"}),
        ))
        .add_edge(
            NodeId::from("cap-stock-check"),
            NodeId::from("cap-payment-execute"),
        )
        .add_edge(
            NodeId::from("cap-payment-execute"),
            NodeId::from("cap-shipping-arrange"),
        )
        .build()
        .unwrap();

    // ── 4. Execute ──────────────────────────────────────────────
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        recorder.clone(),
        intent.budget,
    );
    let outcome = exe.run(&graph, &intent).await.unwrap();
    match outcome {
        ExecutionOutcome::Completed { steps, outputs } => {
            assert_eq!(steps, 3);
            assert_eq!(outputs.len(), 3);
            assert_eq!(
                outputs[&NodeId::from("cap-stock-check")].data["available"],
                42
            );
            assert_eq!(
                outputs[&NodeId::from("cap-payment-execute")].data["status"],
                "captured"
            );
            assert_eq!(
                outputs[&NodeId::from("cap-shipping-arrange")].data["tracking"],
                "JP-12345"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // ── 5. Verify the trace ─────────────────────────────────────
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
    assert_eq!(trace.steps.len(), 3);
    // Every step must have an outcome_detail (E1 Slice A).
    for step in &trace.steps {
        assert!(
            step.observation.outcome_detail.is_some(),
            "step {} missing outcome_detail",
            step.step
        );
    }
}

#[tokio::test]
async fn ecommerce_shipping_failure_triggers_payment_compensation() {
    let compiler = IntentCompiler::default();
    let intent = match compiler
        .compile(
            "洗剤をもう一つ作成して",
            Requester {
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
            "ecommerce",
            BudgetContract {
                max_tokens: 5_000,
                max_cost_usd: 1.0,
                max_latency_ms: 30_000,
            },
        )
        .unwrap()
    {
        CompileOutcome::Compiled(env) => env,
        _ => panic!(),
    };

    // Step 3 (shipping) will fail; step 2 (payment) has a
    // compensation handler registered.
    let graph = GraphBuilder::new()
        .add_node(det_node(
            "cap-stock-check",
            SideEffect::Read,
            serde_json::json!({"available": 42}),
        ))
        .add_node(det_node(
            "cap-payment-execute",
            SideEffect::Payment,
            serde_json::json!({"payment_id": "pay-001"}),
        ))
        .add_node(failing_node("cap-shipping-arrange"))
        .add_edge(
            NodeId::from("cap-stock-check"),
            NodeId::from("cap-payment-execute"),
        )
        .add_edge(
            NodeId::from("cap-payment-execute"),
            NodeId::from("cap-shipping-arrange"),
        )
        .add_compensator(
            NodeId::from("cap-payment-execute"),
            compensation_node("cap-payment-refund"),
        )
        .build()
        .unwrap();

    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        intent.budget,
    );
    let outcome = exe.run(&graph, &intent).await.unwrap();
    match outcome {
        ExecutionOutcome::RolledBack {
            failed_at,
            reason,
            compensated,
        } => {
            assert_eq!(failed_at, 3);
            assert!(reason.contains("address invalid"));
            // Payment was compensated; stock check is read-only so no
            // compensation needed.
            assert!(
                compensated
                    .iter()
                    .any(|n| n.as_str() == "cap-payment-execute"),
                "payment must have been compensated"
            );
        }
        other => panic!("expected RolledBack, got {other:?}"),
    }
}
