//! Governed invocation — end-to-end example test.
//!
//! Demonstrates the full Capability Invocation Bridge:
//!
//!   Agent → ToolExecutor → GoverningToolExecutor → InProcessInvoker
//!         → registered handler → Agent receives real response
//!
//! This is the canonical "agents can actually do things" test:
//! the agent calls tools by name, the invocation bridge resolves them
//! to real handler functions via the capability registry, and the
//! agent receives structured responses it can reason about.

use aaf_contracts::{
    BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
    CapabilitySla, DataClassification, EndpointKind, IntentEnvelope, IntentId, IntentType, NodeId,
    Requester, RiskTier, SideEffect, ToolDefinition, TraceId,
};
use aaf_llm::{LLMProvider, MultiTurnMockProvider};
use aaf_policy::PolicyEngine;
use aaf_registry::Registry;
use aaf_runtime::node::AgentNode;
use aaf_runtime::{
    ExecutionOutcome, GoverningToolExecutor, GraphBuilder, GraphExecutor, InProcessInvoker, Node,
    ToolExecutor,
};
use aaf_trace::{Recorder, TraceRecorder};
use chrono::Utc;
use std::sync::Arc;

// ── Helpers ─────────────────────────────────────────────────────────

fn catalog_cap(id: &str, name: &str, desc: &str) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: name.into(),
        description: desc.into(),
        version: "1.0.0".into(),
        provider_agent: "catalog-agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::InProcess,
            address: id.into(),
            method: None,
        },
        input_schema: serde_json::json!({"type": "object"}),
        output_schema: serde_json::json!({"type": "object"}),
        side_effect: SideEffect::Read,
        idempotent: true,
        reversible: false,
        deterministic: false,
        compensation: None,
        sla: CapabilitySla::default(),
        cost: CapabilityCost::default(),
        required_scope: "catalog:read".into(),
        data_classification: DataClassification::Internal,
        degradation: vec![],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec![],
        domains: vec!["catalog".into()],
        reads: vec![],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.5,
        learned_rules: vec![],
    }
}

fn test_intent() -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::AnalyticalIntent,
        requester: Requester {
            user_id: "user-1".into(),
            role: "analyst".into(),
            scopes: vec!["catalog:read".into()],
            tenant: None,
        },
        goal: "show product catalog report".into(),
        domain: "catalog".into(),
        constraints: Default::default(),
        budget: BudgetContract {
            max_tokens: 10_000,
            max_cost_usd: 1.0,
            max_latency_ms: 30_000,
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

// ── Tests ───────────────────────────────────────────────────────────

/// Full invocation path: agent calls tool → GoverningToolExecutor →
/// registry lookup → InProcessInvoker handler → agent receives response.
#[tokio::test]
async fn agent_calls_tool_through_governing_executor() {
    // 1. Register capabilities with realistic handlers.
    let registry = Arc::new(Registry::in_memory());
    let search_cap = catalog_cap(
        "cap-product-search",
        "product search",
        "search the product catalog",
    );
    let price_cap = catalog_cap(
        "cap-price-lookup",
        "price lookup",
        "retrieve product pricing",
    );
    registry.register(search_cap.clone()).await.unwrap();
    registry.register(price_cap.clone()).await.unwrap();

    // 2. Register handler functions that return structured data.
    let invoker = Arc::new(InProcessInvoker::new());
    invoker.register(
        "product search",
        Arc::new(|input| {
            let query = input["query"].as_str().unwrap_or("*");
            Ok(serde_json::json!({
                "query": query,
                "results": [
                    {"id": 1, "name": "Widget Pro", "category": "tools"},
                    {"id": 2, "name": "Widget Lite", "category": "tools"},
                ],
                "total": 2,
            }))
        }),
    );
    invoker.register(
        "price lookup",
        Arc::new(|input| {
            let product_id = input["product_id"].as_i64().unwrap_or(0);
            Ok(serde_json::json!({
                "product_id": product_id,
                "price": 29.99,
                "currency": "USD",
            }))
        }),
    );

    // 3. Create GoverningToolExecutor (bridges agent → handler).
    let executor: Arc<dyn ToolExecutor> = Arc::new(GoverningToolExecutor::new(
        invoker.clone(),
        registry.clone(),
    ));

    // 4. Build agent with tools from both capabilities.
    let provider: Arc<dyn LLMProvider> = Arc::new(MultiTurnMockProvider::new("test", 0.001, 1));
    let tools = vec![
        ToolDefinition::from(&search_cap),
        ToolDefinition::from(&price_cap),
    ];
    let node_id = NodeId::from("catalog-agent");
    let node: Arc<dyn Node> = Arc::new(
        AgentNode::new(
            node_id.clone(),
            provider,
            "You are a catalog agent.",
            "mock",
            512,
        )
        .with_tools(tools, executor),
    );

    // 5. Execute the graph.
    let graph = GraphBuilder::new().add_node(node).build().unwrap();
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let intent = test_intent();
    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    let outcome = exec.run(&graph, &intent).await.unwrap();

    // 6. Verify the handler was called with correct input.
    assert_eq!(
        invoker.call_count(),
        1,
        "one tool call should have been made"
    );
    let calls = invoker.calls();
    assert_eq!(
        calls[0].0, "product search",
        "first tool called should be product search"
    );

    // 7. Verify execution completed with tool call in output.
    match outcome {
        ExecutionOutcome::Completed { outputs, .. } => {
            let output = outputs.get(&node_id).unwrap();
            let tool_calls = output.data["tool_calls"].as_array().unwrap();
            assert!(!tool_calls.is_empty(), "tool calls recorded in output");

            // Parse the handler's response from the tool output.
            let tool_output = tool_calls[0]["output"].as_str().unwrap();
            let parsed: serde_json::Value = serde_json::from_str(tool_output).unwrap();
            assert_eq!(parsed["total"], 2, "handler should return 2 products");
            assert_eq!(
                parsed["results"][0]["name"], "Widget Pro",
                "first product should be Widget Pro"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // 8. Verify trace recorded.
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
}

/// Handler receives the exact input the agent passed.
#[tokio::test]
async fn handler_receives_exact_tool_input() {
    let registry = Arc::new(Registry::in_memory());
    let cap = catalog_cap("cap-echo", "echo service", "echoes input");
    registry.register(cap.clone()).await.unwrap();

    let invoker = Arc::new(InProcessInvoker::new());
    invoker.register(
        "echo service",
        Arc::new(|input| Ok(serde_json::json!({ "echo": input }))),
    );

    let executor: Arc<dyn ToolExecutor> =
        Arc::new(GoverningToolExecutor::new(invoker.clone(), registry));

    // Call the executor directly (no agent overhead).
    let result = executor
        .execute("echo service", serde_json::json!({"key": "value", "n": 42}))
        .await
        .unwrap();

    let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
    // The handler should receive exactly what was passed.
    assert_eq!(parsed["echo"]["key"], "value");
    assert_eq!(parsed["echo"]["n"], 42);
}

/// Unknown capability produces a clear error, not a crash.
#[tokio::test]
async fn unknown_capability_returns_clear_error() {
    let registry = Arc::new(Registry::in_memory());
    let invoker = Arc::new(InProcessInvoker::new());
    let executor = GoverningToolExecutor::new(invoker, registry);

    let err = executor
        .execute("nonexistent tool", serde_json::json!({}))
        .await
        .unwrap_err();

    assert!(
        err.to_string().contains("capability not found"),
        "error should mention capability not found: {err}"
    );
}

/// Handler error propagates without crashing the runtime.
#[tokio::test]
async fn handler_error_propagates_cleanly() {
    let registry = Arc::new(Registry::in_memory());
    let cap = catalog_cap("cap-fail", "failing service", "always fails");
    registry.register(cap).await.unwrap();

    let invoker = Arc::new(InProcessInvoker::new());
    invoker.register(
        "failing service",
        Arc::new(|_| Err("database connection lost".into())),
    );

    let executor = GoverningToolExecutor::new(invoker, registry);
    let err = executor
        .execute("failing service", serde_json::json!({}))
        .await
        .unwrap_err();

    assert!(
        err.to_string().contains("database connection lost"),
        "handler error should propagate: {err}"
    );
}

/// Call log records every invocation for observability.
#[tokio::test]
async fn call_log_records_all_invocations() {
    let registry = Arc::new(Registry::in_memory());
    let cap_a = catalog_cap("cap-a", "svc_a", "A");
    let cap_b = catalog_cap("cap-b", "svc_b", "B");
    registry.register(cap_a).await.unwrap();
    registry.register(cap_b).await.unwrap();

    let invoker = Arc::new(InProcessInvoker::new());
    invoker.register_fixed("svc_a", serde_json::json!(1));
    invoker.register_fixed("svc_b", serde_json::json!(2));

    let executor = GoverningToolExecutor::new(invoker.clone(), registry);
    executor
        .execute("svc_a", serde_json::json!("x"))
        .await
        .unwrap();
    executor
        .execute("svc_b", serde_json::json!("y"))
        .await
        .unwrap();
    executor
        .execute("svc_a", serde_json::json!("z"))
        .await
        .unwrap();

    let log = invoker.calls();
    assert_eq!(log.len(), 3, "three calls should be logged");
    assert_eq!(log[0].0, "svc_a");
    assert_eq!(log[1].0, "svc_b");
    assert_eq!(log[2].0, "svc_a");
    assert_eq!(log[0].1, serde_json::json!("x"));
    assert_eq!(log[2].1, serde_json::json!("z"));
}

/// YAML config parses correctly.
#[test]
fn yaml_config_parses_successfully() {
    let candidates = [
        "examples/governed-invocation/aaf.yaml",
        "../../examples/governed-invocation/aaf.yaml",
        "../../../examples/governed-invocation/aaf.yaml",
    ];
    let yaml = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("aaf.yaml found");
    let cfg: serde_yaml::Value = serde_yaml::from_str(&yaml).unwrap();
    assert_eq!(
        cfg["project"]["name"].as_str().unwrap(),
        "governed-invocation"
    );
    assert_eq!(cfg["capabilities"].as_sequence().unwrap().len(), 2);
}
