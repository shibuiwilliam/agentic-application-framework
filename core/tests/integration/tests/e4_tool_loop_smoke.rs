//! E4 Slice B — Agentic tool-loop smoke test.
//!
//! Exercises the multi-turn agentic loop end-to-end:
//!
//! 1. Register a **non-deterministic** capability (deterministic=false)
//! 2. Compile an intent targeting that capability
//! 3. Planner should produce a `PlannedStepKind::Agent` step
//! 4. Materialise with `AgentNode` + tools + `MultiTurnMockProvider`
//! 5. Execute the graph
//! 6. Verify: tool calls recorded, tokens accumulated, trace written

use aaf_contracts::{
    BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
    CapabilitySla, DataClassification, EndpointKind, NodeId, Requester, SideEffect, ToolDefinition,
};
use aaf_intent::{compiler::CompileOutcome, IntentCompiler};
use aaf_llm::{LLMProvider, MultiTurnMockProvider};
use aaf_planner::{BoundedAutonomy, CompositionChecker, PlannedStepKind, RegistryPlanner};
use aaf_policy::PolicyEngine;
use aaf_registry::Registry;
use aaf_runtime::node::AgentNode;
use aaf_runtime::{
    ExecutionOutcome, GraphBuilder, GraphExecutor, Node, RuntimeError, ToolExecutor,
};
use aaf_trace::{Recorder, TraceRecorder};
use async_trait::async_trait;
use std::sync::Arc;

/// Build a non-deterministic capability (will produce Agent plan steps).
fn agent_cap(id: &str, name: &str, desc: &str) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: name.into(),
        description: desc.into(),
        version: "1.0.0".into(),
        provider_agent: "warehouse-agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::InProcess,
            address: String::new(),
            method: None,
        },
        input_schema: serde_json::json!({"type": "object", "properties": {"sku": {"type": "string"}}}),
        output_schema: serde_json::json!({"type": "object", "properties": {"available": {"type": "integer"}}}),
        side_effect: SideEffect::Read,
        idempotent: true,
        reversible: false,
        deterministic: false, // ← non-deterministic → Agent step
        compensation: None,
        sla: CapabilitySla::default(),
        cost: CapabilityCost::default(),
        required_scope: "stock:read".into(),
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

/// Test tool executor that returns inventory data.
struct InventoryToolExecutor;

#[async_trait]
impl ToolExecutor for InventoryToolExecutor {
    async fn execute(&self, name: &str, _input: serde_json::Value) -> Result<String, RuntimeError> {
        Ok(format!("{{\"tool\":\"{name}\",\"available\":42}}"))
    }
}

#[tokio::test]
async fn e4_multi_turn_tool_loop_end_to_end() {
    // ── Register a non-deterministic capability ─────────────────────
    let registry = Arc::new(Registry::in_memory());
    let cap = agent_cap(
        "cap-stock-check",
        "check stock",
        "check product availability using warehouse lookup",
    );
    registry.register(cap.clone()).await.unwrap();

    // ── Compile intent ──────────────────────────────────────────────
    let compiler = IntentCompiler::default();
    let outcome = compiler
        .compile(
            "show stock report for product SKU-42",
            Requester {
                user_id: "user-1".into(),
                role: "operator".into(),
                scopes: vec!["stock:read".into()],
                tenant: None,
            },
            "warehouse",
            BudgetContract {
                max_tokens: 10_000,
                max_cost_usd: 1.0,
                max_latency_ms: 30_000,
            },
        )
        .expect("compile");
    let intent = match outcome {
        CompileOutcome::Compiled(env) => env,
        CompileOutcome::NeedsRefinement(qs) => panic!("unexpected refinement: {qs:?}"),
    };

    // ── Plan ────────────────────────────────────────────────────────
    let planner = RegistryPlanner::new(
        registry.clone(),
        BoundedAutonomy::default(),
        CompositionChecker::default(),
    );
    let plan = planner.plan(&intent).await.expect("plan");
    assert!(!plan.is_empty(), "plan must have at least one step");

    // Verify the planner produced an Agent step (non-deterministic cap).
    assert_eq!(
        plan.steps[0].kind,
        PlannedStepKind::Agent,
        "non-deterministic capability should produce Agent step"
    );

    // ── Materialise with AgentNode + tools ──────────────────────────
    let provider: Arc<dyn LLMProvider> = Arc::new(MultiTurnMockProvider::new("e4-test", 0.001, 2));
    let executor: Arc<dyn ToolExecutor> = Arc::new(InventoryToolExecutor);

    let mut builder = GraphBuilder::new();
    let mut prev: Option<NodeId> = None;
    for step in &plan.steps {
        let node_id = NodeId::from(step.capability.as_str());
        let tools: Vec<ToolDefinition> = if let Ok(c) = registry.get(&step.capability).await {
            vec![ToolDefinition::from(&c)]
        } else {
            vec![]
        };
        let node: Arc<dyn Node> = Arc::new(
            AgentNode::new(
                node_id.clone(),
                provider.clone(),
                "You are a warehouse agent.",
                "mock-model",
                512,
            )
            .with_tools(tools, executor.clone()),
        );
        builder = builder.add_node(node);
        if let Some(p) = prev.take() {
            builder = builder.add_edge(p, node_id.clone());
        }
        prev = Some(node_id);
    }
    let graph = builder.build().expect("graph");

    // ── Execute ─────────────────────────────────────────────────────
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    let outcome = exec.run(&graph, &intent).await.expect("execute");

    match &outcome {
        ExecutionOutcome::Completed { outputs, steps } => {
            assert_eq!(*steps as usize, plan.steps.len());

            // Verify tool calls were recorded in the output.
            let cap_node = NodeId::from(plan.steps[0].capability.as_str());
            let output = outputs.get(&cap_node).expect("output for agent node");

            let tool_calls = output.data["tool_calls"]
                .as_array()
                .expect("tool_calls array");
            assert_eq!(
                tool_calls.len(),
                2,
                "MultiTurnMockProvider(2) should produce 2 tool calls"
            );
            assert_eq!(
                tool_calls[0]["name"].as_str().unwrap(),
                "check stock",
                "tool name should match capability name"
            );

            // Verify tokens and cost accumulated across turns.
            assert!(output.tokens > 0, "tokens should accumulate");
            assert!(output.cost_usd > 0.0, "cost should accumulate");
        }
        other => panic!("expected Completed, got {other:?}"),
    }

    // ── Verify trace ────────────────────────────────────────────────
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
    assert_eq!(
        trace.steps.len(),
        plan.steps.len(),
        "trace should record each step"
    );
}

#[tokio::test]
async fn e4_max_tool_calls_bound_terminates_loop() {
    // Agent with max_tool_calls=1 but provider would give 5 tool calls.
    let provider: Arc<dyn LLMProvider> = Arc::new(MultiTurnMockProvider::new("bounded", 0.001, 5));
    let executor: Arc<dyn ToolExecutor> = Arc::new(InventoryToolExecutor);
    let tools = vec![ToolDefinition {
        name: "search".into(),
        description: "Search inventory".into(),
        input_schema: serde_json::json!({}),
        output_schema: serde_json::json!({}),
        side_effect: SideEffect::Read,
        capability_id: CapabilityId::from_raw("cap-search"),
    }];

    let node_id = NodeId::new();
    let node: Arc<dyn Node> = Arc::new(
        AgentNode::new(node_id.clone(), provider, "system", "mock", 100)
            .with_tools(tools, executor)
            .with_max_tool_calls(1),
    );

    let mut builder = GraphBuilder::new();
    builder = builder.add_node(node);
    let graph = builder.build().unwrap();

    let policy = Arc::new(PolicyEngine::with_default_rules());
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let budget = BudgetContract {
        max_tokens: 10_000,
        max_cost_usd: 1.0,
        max_latency_ms: 30_000,
    };
    let intent = aaf_contracts::IntentEnvelope {
        intent_id: aaf_contracts::IntentId::new(),
        intent_type: aaf_contracts::IntentType::AnalyticalIntent,
        requester: Requester {
            user_id: "u".into(),
            role: "r".into(),
            scopes: vec![],
            tenant: None,
        },
        goal: "search".into(),
        domain: "test".into(),
        constraints: Default::default(),
        budget,
        deadline: None,
        risk_tier: aaf_contracts::RiskTier::Read,
        approval_policy: "none".into(),
        output_contract: None,
        trace_id: aaf_contracts::TraceId::new(),
        depth: 0,
        created_at: chrono::Utc::now(),
        entities_in_context: vec![],
    };

    let exec = GraphExecutor::new(policy, recorder, intent.budget);
    let outcome = exec.run(&graph, &intent).await.expect("execute");

    match outcome {
        ExecutionOutcome::Completed { outputs, .. } => {
            let output = outputs.get(&node_id).expect("output");
            let tool_calls = output.data["tool_calls"].as_array().unwrap();
            assert_eq!(
                tool_calls.len(),
                1,
                "max_tool_calls=1 should limit to 1 tool call"
            );
            assert_eq!(
                output.data["stop_reason"].as_str().unwrap(),
                "BudgetExhausted",
                "should report BudgetExhausted when bound reached"
            );
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}
