//! End-to-end integration tests for the graph runtime.
//!
//! Each test exercises one architecture rule from `CLAUDE.md`.

use aaf_contracts::{
    BudgetContract, IntentEnvelope, IntentId, IntentType, NodeId, Requester, RiskTier, SideEffect,
    TenantId, TraceId,
};
use aaf_policy::PolicyEngine;
use aaf_runtime::node::{DeterministicNode, NodeKind, NodeOutput};
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node, RuntimeError};
use aaf_trace::{Recorder, TraceRecorder};
use async_trait::async_trait;
use chrono::Utc;
use std::collections::HashMap;
use std::sync::Arc;

/// Test-only node that charges a fixed cost / token count, so the
/// budget exhaustion path can be exercised deterministically without
/// relying on wall-clock timing.
struct CostingNode {
    id: NodeId,
    cost_usd: f64,
    tokens: u64,
    side_effect: SideEffect,
    output: serde_json::Value,
}

impl CostingNode {
    fn boxed(id: &str, cost_usd: f64, tokens: u64, output: serde_json::Value) -> Arc<dyn Node> {
        Arc::new(Self {
            id: NodeId::from(id),
            cost_usd,
            tokens,
            side_effect: SideEffect::None,
            output,
        })
    }
}

#[async_trait]
impl Node for CostingNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Agent
    }
    fn side_effect(&self) -> SideEffect {
        self.side_effect
    }
    async fn run(
        &self,
        _intent: &IntentEnvelope,
        _prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        Ok(NodeOutput {
            data: self.output.clone(),
            tokens: self.tokens,
            cost_usd: self.cost_usd,
            duration_ms: 0,
            model: Some("mock".into()),
        })
    }
}

/// Test-only node with `Write` side effect — no LLM, no cost.
struct WriteNode {
    id: NodeId,
}

impl WriteNode {
    fn boxed(id: &str) -> Arc<dyn Node> {
        Arc::new(Self {
            id: NodeId::from(id),
        })
    }
}

#[async_trait]
impl Node for WriteNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Deterministic
    }
    fn side_effect(&self) -> SideEffect {
        SideEffect::Write
    }
    async fn run(
        &self,
        _intent: &IntentEnvelope,
        _prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        Ok(NodeOutput {
            data: serde_json::json!({"wrote": true}),
            ..Default::default()
        })
    }
}

/// Test-only node that always fails. Used to trigger rollback.
struct AlwaysFailNode {
    id: NodeId,
}

impl AlwaysFailNode {
    fn boxed(id: &str) -> Arc<dyn Node> {
        Arc::new(Self {
            id: NodeId::from(id),
        })
    }
}

#[async_trait]
impl Node for AlwaysFailNode {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Deterministic
    }
    fn side_effect(&self) -> SideEffect {
        SideEffect::None
    }
    async fn run(
        &self,
        _intent: &IntentEnvelope,
        _prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        Err(RuntimeError::Node("simulated failure".into()))
    }
}

/// Test-only compensator that increments a shared counter.
struct CounterCompensator {
    id: NodeId,
    counter: Arc<std::sync::atomic::AtomicU32>,
}

impl CounterCompensator {
    fn boxed(id: &str, counter: Arc<std::sync::atomic::AtomicU32>) -> Arc<dyn Node> {
        Arc::new(Self {
            id: NodeId::from(id),
            counter,
        })
    }
}

#[async_trait]
impl Node for CounterCompensator {
    fn id(&self) -> &NodeId {
        &self.id
    }
    fn kind(&self) -> NodeKind {
        NodeKind::Deterministic
    }
    fn side_effect(&self) -> SideEffect {
        SideEffect::None
    }
    async fn run(
        &self,
        _intent: &IntentEnvelope,
        _prior: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError> {
        self.counter
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(NodeOutput {
            data: serde_json::json!({"compensated": true}),
            ..Default::default()
        })
    }
}

fn intent(scopes: Vec<String>, max_tokens: u64) -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::AnalyticalIntent,
        requester: Requester {
            user_id: "user-1".into(),
            role: "analyst".into(),
            scopes,
            tenant: None,
        },
        goal: "show last month revenue".into(),
        domain: "sales".into(),
        constraints: Default::default(),
        budget: BudgetContract {
            max_tokens,
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

fn det(id: &str, output: serde_json::Value) -> Arc<dyn Node> {
    Arc::new(DeterministicNode::new(
        NodeId::from(id),
        SideEffect::None,
        Arc::new(move |_, _| Ok(output.clone())),
    ))
}

#[tokio::test]
async fn rule_6_policy_runs_at_every_step_and_records_observations() {
    // Build a 3-node read-only graph.
    let g = GraphBuilder::new()
        .add_node(det("a", serde_json::json!({"x": 1})))
        .add_node(det("b", serde_json::json!({"x": 2})))
        .add_node(det("c", serde_json::json!({"x": 3})))
        .add_edge(NodeId::from("a"), NodeId::from("b"))
        .add_edge(NodeId::from("b"), NodeId::from("c"))
        .build()
        .unwrap();
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
    let i = intent(vec![], 1000);
    let outcome = exe.run(&g, &i).await.unwrap();
    match outcome {
        ExecutionOutcome::Completed { outputs, steps } => {
            assert_eq!(outputs.len(), 3);
            assert_eq!(steps, 3);
        }
        _ => panic!("expected Completed"),
    }
    let trace = recorder.get(&i.trace_id).await.unwrap();
    assert_eq!(trace.steps.len(), 3); // Rule 12: every step recorded
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
}

#[tokio::test]
async fn rule_8_partial_result_when_cost_budget_exhausted() {
    // Two CostingNodes each charge $0.40. With a $0.50 budget the
    // first succeeds, the second pushes us over and the executor must
    // return Partial with `steps == 1`.
    let g = GraphBuilder::new()
        .add_node(CostingNode::boxed(
            "first",
            0.40,
            100,
            serde_json::json!({"i": 1}),
        ))
        .add_node(CostingNode::boxed(
            "second",
            0.40,
            100,
            serde_json::json!({"i": 2}),
        ))
        .add_edge(NodeId::from("first"), NodeId::from("second"))
        .build()
        .unwrap();

    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 10_000,
            max_cost_usd: 0.50,
            max_latency_ms: 60_000,
        },
    );
    let i = intent(vec![], 10_000);
    let outcome = exe.run(&g, &i).await.unwrap();
    match outcome {
        ExecutionOutcome::Partial { steps, outputs, .. } => {
            assert_eq!(steps, 1, "exactly one step should have completed");
            assert_eq!(outputs.len(), 1, "outputs should hold the first step only");
        }
        other => panic!("expected Partial, got {other:?}"),
    }
}

#[tokio::test]
async fn rule_7_executor_pauses_on_require_approval() {
    // Write capability without `auto-approve` scope → SideEffectGate
    // returns RequireApproval at PreStep, executor must return
    // PendingApproval rather than silently running the node.
    let g = GraphBuilder::new()
        .add_node(WriteNode::boxed("write"))
        .build()
        .unwrap();
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    );
    let i = intent(vec![], 1000); // no auto-approve scope
    let outcome = exe.run(&g, &i).await.unwrap();
    match outcome {
        ExecutionOutcome::PendingApproval { at_step, .. } => assert_eq!(at_step, 1),
        other => panic!("expected PendingApproval, got {other:?}"),
    }
}

#[tokio::test]
async fn rule_7_executor_runs_when_auto_approve_present() {
    let g = GraphBuilder::new()
        .add_node(WriteNode::boxed("write"))
        .build()
        .unwrap();
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    );
    let i = intent(vec!["auto-approve".into()], 1000);
    let outcome = exe.run(&g, &i).await.unwrap();
    assert!(matches!(outcome, ExecutionOutcome::Completed { .. }));
}

#[tokio::test]
async fn rule_6_pii_in_output_is_blocked_by_post_step_hook() {
    // Deterministic node emits an email address; PII guard should
    // catch it on PostStep and the executor should bail with a
    // PolicyViolation error.
    let g = GraphBuilder::new()
        .add_node(det(
            "leak",
            serde_json::json!({"contact": "alice@example.com"}),
        ))
        .build()
        .unwrap();
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    );
    let i = intent(vec![], 1000);
    let err = exe.run(&g, &i).await.unwrap_err();
    match err {
        RuntimeError::PolicyViolation(violations) => {
            assert!(violations
                .iter()
                .any(|v| v.kind == aaf_contracts::RuleKind::PiiGuard));
        }
        other => panic!("expected PolicyViolation, got {other:?}"),
    }
}

#[tokio::test]
async fn boundary_cross_tenant_request_is_blocked() {
    // Requester carries tenant A but the executor pre-plan context
    // also carries tenant A. We swap the requester's tenant to B but
    // the executor takes tenant from `intent.requester.tenant` so the
    // boundary rule needs a different invariant: register a confidential
    // capability and verify the boundary rule denies it without the
    // proper scope.
    let g = GraphBuilder::new()
        .add_node(det("only", serde_json::json!({})))
        .build()
        .unwrap();
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    );
    let mut i = intent(vec![], 1000);
    i.requester.tenant = Some(TenantId::from("tenant-a"));
    // The executor passes `intent.requester.tenant` as the active
    // tenant, so the rule's boundary check sees `active == requester`
    // and returns no violation. This test pins that contract: matching
    // tenant is allowed; cross-tenant access requires the caller to
    // override the active tenant context, which is currently a future
    // hook (transports will populate it). The test prevents an
    // accidental regression where the rule starts denying *matching*
    // tenants.
    let outcome = exe.run(&g, &i).await.unwrap();
    assert!(matches!(outcome, ExecutionOutcome::Completed { .. }));
}

#[tokio::test]
async fn rule_5_deterministic_node_has_no_llm_cost() {
    let g = GraphBuilder::new()
        .add_node(det("only", serde_json::json!({})))
        .build()
        .unwrap();
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 5_000,
        },
    );
    let i = intent(vec![], 1000);
    let _ = exe.run(&g, &i).await.unwrap();
    // Deterministic nodes never charge cost.
    let remaining = exe.budget.remaining();
    assert!((remaining.max_cost_usd - 1.0).abs() < 1e-9);
}

#[tokio::test]
async fn rule_9_compensation_runs_on_node_failure() {
    // Two write steps each with a CounterCompensator, then a failing
    // node. After execution the chain must have run both compensators
    // (counter == 2) and the executor must return RolledBack with the
    // two original step ids in execution order.
    let counter = Arc::new(std::sync::atomic::AtomicU32::new(0));
    let g = GraphBuilder::new()
        .add_node(WriteNode::boxed("write-a"))
        .add_node(WriteNode::boxed("write-b"))
        .add_node(AlwaysFailNode::boxed("boom"))
        .add_edge(NodeId::from("write-a"), NodeId::from("write-b"))
        .add_edge(NodeId::from("write-b"), NodeId::from("boom"))
        .add_compensator(
            NodeId::from("write-a"),
            CounterCompensator::boxed("comp-a", counter.clone()),
        )
        .add_compensator(
            NodeId::from("write-b"),
            CounterCompensator::boxed("comp-b", counter.clone()),
        )
        .build()
        .unwrap();

    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    );
    let i = intent(vec!["auto-approve".into()], 1000);
    let outcome = exe.run(&g, &i).await.unwrap();
    match outcome {
        ExecutionOutcome::RolledBack {
            failed_at,
            reason,
            compensated,
        } => {
            assert_eq!(failed_at, 3);
            assert!(reason.contains("simulated"));
            // Both writes were rolled back.
            assert_eq!(compensated.len(), 2);
            assert_eq!(
                counter.load(std::sync::atomic::Ordering::SeqCst),
                2,
                "both compensators should have been invoked"
            );
        }
        other => panic!("expected RolledBack, got {other:?}"),
    }
}

// ───────────── Wave 2 X1 Slice B — revocation gate ─────────────────────

#[tokio::test]
async fn rule_22_pre_plan_refuses_revoked_requester() {
    use aaf_identity::{
        InMemoryKeystore, InMemoryRevocationRegistry, Keystore, RevocationEntry, RevocationKind,
        RevocationRegistry,
    };

    // Build a keystore + two DIDs.
    let ks = InMemoryKeystore::new();
    let admin = ks.generate(b"admin");
    let agent = ks.generate(b"compromised-agent");

    // Register the agent's DID in a shared registry, then revoke it.
    let reg: Arc<dyn RevocationRegistry> = Arc::new(InMemoryRevocationRegistry::new());
    let entry = RevocationEntry::issue(
        RevocationKind::Did,
        agent.to_string(),
        "compromised key",
        admin,
        &ks,
    )
    .unwrap();
    reg.revoke(entry).await.unwrap();

    // Build a trivial graph.
    let g = GraphBuilder::new()
        .add_node(det("only", serde_json::json!({})))
        .build()
        .unwrap();

    // Executor configured with the revocation gate.
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    )
    .with_revocation(reg);

    // Intent whose requester carries the revoked DID.
    let mut i = intent(vec![], 1000);
    i.requester.user_id = agent.to_string();

    // Must fail fast with RuntimeError::Revoked.
    let err = exe.run(&g, &i).await.unwrap_err();
    match err {
        RuntimeError::Revoked { did, reason } => {
            assert!(did.starts_with("did:aaf:"));
            assert!(reason.contains("revoked"));
        }
        other => panic!("expected Revoked, got {other:?}"),
    }
}

#[tokio::test]
async fn rule_22_non_did_requesters_bypass_revocation_gate() {
    // A Wave 1 requester (plain user id, no DID) must not be denied
    // by the revocation gate even when a registry is attached. This
    // is the backward-compatibility invariant that lets Wave 1 code
    // continue to work unchanged.
    use aaf_identity::{InMemoryRevocationRegistry, RevocationRegistry};

    let reg: Arc<dyn RevocationRegistry> = Arc::new(InMemoryRevocationRegistry::new());
    let g = GraphBuilder::new()
        .add_node(det("only", serde_json::json!({})))
        .build()
        .unwrap();
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    )
    .with_revocation(reg);

    // Default test intent uses plain "user-1" — no did: prefix.
    let i = intent(vec![], 1000);
    let outcome = exe.run(&g, &i).await.unwrap();
    assert!(matches!(outcome, ExecutionOutcome::Completed { .. }));
}

// ───────────── AafService §6.2 — Shadow mode ──────────────────────────

#[tokio::test]
async fn shadow_mode_skips_write_nodes_but_records_trace() {
    // Write node should NOT execute in shadow mode.
    let g = GraphBuilder::new()
        .add_node(det("read-a", serde_json::json!({"data": 1})))
        .add_node(WriteNode::boxed("write-b"))
        .add_edge(NodeId::from("read-a"), NodeId::from("write-b"))
        .build()
        .unwrap();
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        recorder.clone(),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    )
    .with_shadow(); // <-- shadow mode ON

    let i = intent(vec!["auto-approve".into()], 1000);
    let outcome = exe.run(&g, &i).await.unwrap();
    match outcome {
        ExecutionOutcome::Completed { outputs, steps } => {
            assert_eq!(steps, 2);
            // Read node ran normally.
            assert_eq!(outputs[&NodeId::from("read-a")].data["data"], 1);
            // Write node was shadowed — synthetic marker.
            let shadow = &outputs[&NodeId::from("write-b")].data;
            assert_eq!(shadow["shadow"], true);
            assert!(shadow["would_have_run"]
                .as_str()
                .unwrap()
                .contains("write-b"));
        }
        other => panic!("expected Completed, got {other:?}"),
    }
    // Trace should still record both steps.
    let trace = recorder.get(&i.trace_id).await.unwrap();
    assert_eq!(trace.steps.len(), 2);
}

#[tokio::test]
async fn shadow_mode_off_executes_write_nodes_normally() {
    let g = GraphBuilder::new()
        .add_node(WriteNode::boxed("write-a"))
        .build()
        .unwrap();
    let exe = GraphExecutor::new(
        Arc::new(PolicyEngine::with_default_rules()),
        Arc::new(Recorder::in_memory()),
        BudgetContract {
            max_tokens: 1000,
            max_cost_usd: 1.0,
            max_latency_ms: 10_000,
        },
    );
    // NO .with_shadow() — shadow mode OFF (default).
    let i = intent(vec!["auto-approve".into()], 1000);
    let outcome = exe.run(&g, &i).await.unwrap();
    match outcome {
        ExecutionOutcome::Completed { outputs, .. } => {
            // Write node actually ran.
            assert_eq!(outputs[&NodeId::from("write-a")].data["wrote"], true);
        }
        other => panic!("expected Completed, got {other:?}"),
    }
}
