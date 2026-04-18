//! Capability invocation bridge — end-to-end test (Pillar 2 Slice A).
//!
//! Proves the complete path:
//!   Agent → ToolExecutor → GoverningToolExecutor → InProcessInvoker → handler
//!
//! The agent calls a tool by name, the GoverningToolExecutor looks up
//! the capability in the registry, and the InProcessInvoker executes
//! the registered handler function. The agent receives the handler's
//! response as its tool result.

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

fn agent_cap(id: &str, name: &str, desc: &str) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: name.into(),
        description: desc.into(),
        version: "1.0.0".into(),
        provider_agent: "test-agent".into(),
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
        required_scope: "test:read".into(),
        data_classification: DataClassification::Internal,
        degradation: vec![],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec![],
        domains: vec!["warehouse".into()],
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
            role: "operator".into(),
            scopes: vec!["test:read".into()],
            tenant: None,
        },
        goal: "show stock report for product".into(),
        domain: "warehouse".into(),
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

/// Agent calls a tool → GoverningToolExecutor → InProcessInvoker →
/// handler returns response → agent receives it.
#[tokio::test]
async fn agent_invokes_capability_via_governing_executor() {
    // 1. Set up registry with a capability.
    let registry = Arc::new(Registry::in_memory());
    let cap = agent_cap("cap-stock", "stock lookup", "look up stock levels");
    registry.register(cap.clone()).await.unwrap();

    // 2. Set up InProcessInvoker with a handler.
    let invoker = Arc::new(InProcessInvoker::new());
    invoker.register(
        "stock lookup",
        Arc::new(|input| {
            let sku = input["sku"].as_str().unwrap_or("unknown");
            Ok(serde_json::json!({
                "sku": sku,
                "available": 42,
                "warehouse": "WEST-1",
            }))
        }),
    );

    // 3. Create GoverningToolExecutor.
    let executor: Arc<dyn ToolExecutor> = Arc::new(GoverningToolExecutor::new(
        invoker.clone(),
        registry.clone(),
    ));

    // 4. Create AgentNode with tools.
    let provider: Arc<dyn LLMProvider> = Arc::new(MultiTurnMockProvider::new("test", 0.001, 1));
    let tools = vec![ToolDefinition::from(&cap)];
    let node_id = NodeId::from("cap-stock");
    let node: Arc<dyn Node> = Arc::new(
        AgentNode::new(node_id.clone(), provider, "system", "mock", 512)
            .with_tools(tools, executor),
    );

    // 5. Build graph and execute.
    let graph = GraphBuilder::new().add_node(node).build().unwrap();
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let intent = test_intent();
    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    let outcome = exec.run(&graph, &intent).await.unwrap();

    // 6. Verify the handler was called.
    assert_eq!(invoker.call_count(), 1, "handler should be called once");
    let calls = invoker.calls();
    assert_eq!(calls[0].0, "stock lookup");

    // 7. Verify execution completed.
    match outcome {
        ExecutionOutcome::Completed { outputs, .. } => {
            let output = outputs.get(&node_id).expect("node output");
            let tool_calls = output.data["tool_calls"].as_array().unwrap();
            assert!(!tool_calls.is_empty(), "tool calls should be recorded");

            // The tool output should contain the handler's response.
            let tool_output = tool_calls[0]["output"].as_str().unwrap();
            let parsed: serde_json::Value = serde_json::from_str(tool_output).unwrap();
            assert_eq!(parsed["available"], 42);
            assert_eq!(parsed["warehouse"], "WEST-1");
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // 8. Verify trace recorded.
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
}

/// GoverningToolExecutor returns error when capability not in registry.
#[tokio::test]
async fn governing_executor_errors_on_unknown_capability() {
    let registry = Arc::new(Registry::in_memory());
    let invoker = Arc::new(InProcessInvoker::new());
    let executor = GoverningToolExecutor::new(invoker, registry);

    let err = executor
        .execute("nonexistent tool", serde_json::json!({}))
        .await
        .unwrap_err();
    assert!(
        err.to_string().contains("capability not found"),
        "error: {err}"
    );
}

/// InProcessInvoker with multiple handlers, each called correctly.
#[tokio::test]
async fn multiple_capabilities_invoked_through_executor() {
    let registry = Arc::new(Registry::in_memory());
    let cap_a = agent_cap("cap-a", "service_a", "Service A");
    let cap_b = agent_cap("cap-b", "service_b", "Service B");
    registry.register(cap_a).await.unwrap();
    registry.register(cap_b).await.unwrap();

    let invoker = Arc::new(InProcessInvoker::new());
    invoker.register_fixed("service_a", serde_json::json!({"result": "A"}));
    invoker.register_fixed("service_b", serde_json::json!({"result": "B"}));

    let executor = GoverningToolExecutor::new(invoker.clone(), registry);

    let ra = executor
        .execute("service_a", serde_json::json!({}))
        .await
        .unwrap();
    let rb = executor
        .execute("service_b", serde_json::json!({}))
        .await
        .unwrap();

    let parsed_a: serde_json::Value = serde_json::from_str(&ra).unwrap();
    let parsed_b: serde_json::Value = serde_json::from_str(&rb).unwrap();
    assert_eq!(parsed_a["result"], "A");
    assert_eq!(parsed_b["result"], "B");
    assert_eq!(invoker.call_count(), 2);
}
