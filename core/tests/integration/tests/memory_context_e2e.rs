//! End-to-end test for examples/memory-context.
//!
//! Exercises AAF's **four-layer memory model** and **context budget**
//! (Rule 10) — the last major subsystem not covered by the other
//! examples.
//!
//! **Working Memory (Layer 1):**
//!
//! 1.  Per-task put/get round-trip.
//! 2.  Overwriting a key replaces the value.
//! 3.  Clearing a task removes all its entries.
//! 4.  Different tasks are isolated from each other.
//!
//! **Thread Memory (Layer 2):**
//!
//! 5.  Append-only conversation log preserves insertion order.
//! 6.  Different threads are isolated from each other.
//!
//! **Long-Term Memory (Layer 3):**
//!
//! 7.  Keyword search matches records containing all query terms.
//! 8.  Entity-keyed retrieval returns records indexed under an entity.
//! 9.  Tenant isolation: cross-tenant queries return empty.
//! 10. Limit parameter caps result count.
//! 11. Multiple entity refs index the same record under each entity.
//!
//! **Artifact Store (Layer 4):**
//!
//! 12. Artifact round-trip preserves provenance.
//! 13. Artifact content and metadata are intact after retrieval.
//!
//! **Context Budget (Rule 10):**
//!
//! 14. Default budget matches PROJECT.md §3.6 (7,500 total).
//! 15. Token approximation: chars / 4.
//! 16. Truncation respects per-section cap.
//! 17. Fit applies the correct section cap.
//! 18. Short text passes through untruncated.
//!
//! **Full Pipeline:**
//!
//! 19. Multi-step workflow: working memory → graph execution →
//!     artifact storage → long-term indexing → entity retrieval.
//!
//! Run this test with:
//!
//!     cargo test -p aaf-integration-tests --test memory_context_e2e

use aaf_contracts::{
    AgentId, Artifact, ArtifactId, ArtifactProvenance, BudgetContract, CapabilityId, EntityRefLite,
    IntentEnvelope, IntentId, IntentType, NodeId, Requester, RiskTier, SideEffect, TaskId,
    TenantId, TraceId,
};
use aaf_memory::{ContextBudget, ContextSection, MemoryFacade, DEFAULT_TOTAL_BUDGET};
use aaf_policy::PolicyEngine;
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{ExecutionOutcome, GraphBuilder, GraphExecutor, Node};
use aaf_storage::memory::LongTermRecord;
use aaf_trace::{Recorder, TraceRecorder};
use chrono::Utc;
use std::sync::Arc;

// ── Helpers ─────────────────────────────────────────────────────────

fn provenance(task: &TaskId) -> ArtifactProvenance {
    ArtifactProvenance {
        intent_id: IntentId::new(),
        task_id: task.clone(),
        trace_id: TraceId::new(),
        producing_agent: AgentId::from("report-agent"),
        capability: CapabilityId::from("cap-report-generate"),
        data_sources: vec!["cap-order-lookup".into()],
        model_used: Some("claude-3-haiku".into()),
    }
}

fn sample_intent() -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::AnalyticalIntent,
        requester: Requester {
            user_id: "user-analyst".into(),
            role: "analyst".into(),
            scopes: vec!["orders:read".into(), "reports:read".into()],
            tenant: Some(TenantId::from("tenant-jp")),
        },
        goal: "look up order and produce a report".into(),
        domain: "commerce".into(),
        constraints: Default::default(),
        budget: BudgetContract {
            max_tokens: 5_000,
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

// ════════════════════════════════════════════════════════════════════
// WORKING MEMORY (Layer 1)
// ════════════════════════════════════════════════════════════════════

/// 1. Per-task put/get round-trip.
#[tokio::test]
async fn working_memory_put_get_round_trip() {
    let mem = MemoryFacade::in_memory();
    let task = TaskId::new();

    mem.working_put(&task, "step_result", serde_json::json!({"rows": 47}))
        .await
        .unwrap();

    let val = mem.working_get(&task, "step_result").await.unwrap();
    assert_eq!(val, Some(serde_json::json!({"rows": 47})));
}

/// 2. Overwriting a key replaces the previous value.
#[tokio::test]
async fn working_memory_overwrite_replaces_value() {
    let mem = MemoryFacade::in_memory();
    let task = TaskId::new();

    mem.working_put(&task, "counter", serde_json::json!(1))
        .await
        .unwrap();
    mem.working_put(&task, "counter", serde_json::json!(2))
        .await
        .unwrap();

    let val = mem.working_get(&task, "counter").await.unwrap();
    assert_eq!(val, Some(serde_json::json!(2)));
}

/// 3. Clearing a task removes all its entries.
#[tokio::test]
async fn working_memory_clear_removes_all_entries() {
    let mem = MemoryFacade::in_memory();
    let task = TaskId::new();

    mem.working_put(&task, "a", serde_json::json!("x"))
        .await
        .unwrap();
    mem.working_put(&task, "b", serde_json::json!("y"))
        .await
        .unwrap();
    mem.working_clear(&task).await.unwrap();

    assert_eq!(mem.working_get(&task, "a").await.unwrap(), None);
    assert_eq!(mem.working_get(&task, "b").await.unwrap(), None);
}

/// 4. Different tasks are isolated from each other.
#[tokio::test]
async fn working_memory_task_isolation() {
    let mem = MemoryFacade::in_memory();
    let task_a = TaskId::new();
    let task_b = TaskId::new();

    mem.working_put(&task_a, "key", serde_json::json!("alpha"))
        .await
        .unwrap();
    mem.working_put(&task_b, "key", serde_json::json!("beta"))
        .await
        .unwrap();

    assert_eq!(
        mem.working_get(&task_a, "key").await.unwrap(),
        Some(serde_json::json!("alpha"))
    );
    assert_eq!(
        mem.working_get(&task_b, "key").await.unwrap(),
        Some(serde_json::json!("beta"))
    );
}

// ════════════════════════════════════════════════════════════════════
// THREAD MEMORY (Layer 2)
// ════════════════════════════════════════════════════════════════════

/// 5. Append-only conversation log preserves insertion order.
#[tokio::test]
async fn thread_memory_preserves_insertion_order() {
    let mem = MemoryFacade::in_memory();
    let thread = "conv-order-42".to_string();

    mem.thread_append(
        &thread,
        serde_json::json!({"role": "user", "text": "cancel my order"}),
    )
    .await
    .unwrap();
    mem.thread_append(
        &thread,
        serde_json::json!({"role": "agent", "text": "processing cancellation"}),
    )
    .await
    .unwrap();
    mem.thread_append(
        &thread,
        serde_json::json!({"role": "agent", "text": "order cancelled"}),
    )
    .await
    .unwrap();

    let messages = mem.thread.read(&thread).await.unwrap();
    assert_eq!(messages.len(), 3);
    assert_eq!(messages[0]["role"], "user");
    assert_eq!(messages[1]["role"], "agent");
    assert_eq!(messages[2]["text"], "order cancelled");
}

/// 6. Different threads are isolated.
#[tokio::test]
async fn thread_memory_isolation() {
    let mem = MemoryFacade::in_memory();

    mem.thread_append(&"thread-A".into(), serde_json::json!("msg-A"))
        .await
        .unwrap();
    mem.thread_append(&"thread-B".into(), serde_json::json!("msg-B"))
        .await
        .unwrap();

    let a = mem.thread.read(&"thread-A".into()).await.unwrap();
    let b = mem.thread.read(&"thread-B".into()).await.unwrap();
    assert_eq!(a.len(), 1);
    assert_eq!(b.len(), 1);
    assert_eq!(a[0], "msg-A");
    assert_eq!(b[0], "msg-B");
}

// ════════════════════════════════════════════════════════════════════
// LONG-TERM MEMORY (Layer 3)
// ════════════════════════════════════════════════════════════════════

/// 7. Keyword search matches records containing all query terms.
#[tokio::test]
async fn longterm_keyword_search() {
    let mem = MemoryFacade::in_memory();
    let tenant = TenantId::from("tenant-jp");

    mem.longterm_insert(LongTermRecord {
        tenant: tenant.clone(),
        kind: "semantic".into(),
        content: "Tokyo branch outperformed Osaka in Q1 2026".into(),
        payload: serde_json::json!({"region": "APAC"}),
        entity_refs: vec![],
    })
    .await
    .unwrap();

    mem.longterm_insert(LongTermRecord {
        tenant: tenant.clone(),
        kind: "semantic".into(),
        content: "Osaka office renovation completed".into(),
        payload: serde_json::json!({}),
        entity_refs: vec![],
    })
    .await
    .unwrap();

    // Search for "Tokyo" — only the first record matches
    let hits = mem.longterm_search(&tenant, "tokyo", 10).await.unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("Tokyo"));

    // Search for "Osaka" — both records match
    let hits = mem.longterm_search(&tenant, "osaka", 10).await.unwrap();
    assert_eq!(hits.len(), 2);
}

/// 8. Entity-keyed retrieval returns records indexed under an entity.
#[tokio::test]
async fn longterm_entity_keyed_retrieval() {
    let mem = MemoryFacade::in_memory();
    let tenant = TenantId::from("tenant-jp");
    let order_ref = EntityRefLite::new("commerce.Order");
    let customer_ref = EntityRefLite::new("commerce.Customer");

    mem.longterm_insert(LongTermRecord {
        tenant: tenant.clone(),
        kind: "episodic".into(),
        content: "order-42 was refunded after shipping delay".into(),
        payload: serde_json::json!({"order_id": "ord-42"}),
        entity_refs: vec![order_ref.clone()],
    })
    .await
    .unwrap();

    mem.longterm_insert(LongTermRecord {
        tenant: tenant.clone(),
        kind: "episodic".into(),
        content: "customer Tanaka prefers express shipping".into(),
        payload: serde_json::json!({"customer_id": "cust-1"}),
        entity_refs: vec![customer_ref.clone()],
    })
    .await
    .unwrap();

    // Query by Order entity — only the order record
    let hits = mem
        .longterm_search_by_entity(&tenant, &order_ref, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("order-42"));

    // Query by Customer entity — only the customer record
    let hits = mem
        .longterm_search_by_entity(&tenant, &customer_ref, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("Tanaka"));
}

/// 9. Tenant isolation: cross-tenant queries return empty.
#[tokio::test]
async fn longterm_tenant_isolation() {
    let mem = MemoryFacade::in_memory();
    let tenant_jp = TenantId::from("tenant-jp");
    let tenant_us = TenantId::from("tenant-us");
    let order_ref = EntityRefLite::new("commerce.Order");

    mem.longterm_insert(LongTermRecord {
        tenant: tenant_jp.clone(),
        kind: "episodic".into(),
        content: "JP-only order data".into(),
        payload: serde_json::json!({}),
        entity_refs: vec![order_ref.clone()],
    })
    .await
    .unwrap();

    // Keyword search from wrong tenant
    let hits = mem
        .longterm_search(&tenant_us, "JP-only", 10)
        .await
        .unwrap();
    assert!(
        hits.is_empty(),
        "cross-tenant keyword search must return empty"
    );

    // Entity search from wrong tenant
    let hits = mem
        .longterm_search_by_entity(&tenant_us, &order_ref, 10)
        .await
        .unwrap();
    assert!(
        hits.is_empty(),
        "cross-tenant entity search must return empty"
    );
}

/// 10. Limit parameter caps the result count.
#[tokio::test]
async fn longterm_search_respects_limit() {
    let mem = MemoryFacade::in_memory();
    let tenant = TenantId::from("tenant-jp");
    let order_ref = EntityRefLite::new("commerce.Order");

    for i in 0..10 {
        mem.longterm_insert(LongTermRecord {
            tenant: tenant.clone(),
            kind: "episodic".into(),
            content: format!("order event {i}"),
            payload: serde_json::json!({}),
            entity_refs: vec![order_ref.clone()],
        })
        .await
        .unwrap();
    }

    let hits = mem
        .longterm_search_by_entity(&tenant, &order_ref, 3)
        .await
        .unwrap();
    assert_eq!(hits.len(), 3, "limit should cap at 3");
}

/// 11. Multiple entity refs index the same record under each entity.
#[tokio::test]
async fn longterm_multi_entity_indexing() {
    let mem = MemoryFacade::in_memory();
    let tenant = TenantId::from("tenant-jp");
    let order_ref = EntityRefLite::new("commerce.Order");
    let payment_ref = EntityRefLite::new("finance.Payment");

    mem.longterm_insert(LongTermRecord {
        tenant: tenant.clone(),
        kind: "episodic".into(),
        content: "order-42 payment captured".into(),
        payload: serde_json::json!({}),
        entity_refs: vec![order_ref.clone(), payment_ref.clone()],
    })
    .await
    .unwrap();

    // Findable via either entity
    let by_order = mem
        .longterm_search_by_entity(&tenant, &order_ref, 10)
        .await
        .unwrap();
    assert_eq!(by_order.len(), 1);

    let by_payment = mem
        .longterm_search_by_entity(&tenant, &payment_ref, 10)
        .await
        .unwrap();
    assert_eq!(by_payment.len(), 1);
}

// ════════════════════════════════════════════════════════════════════
// ARTIFACT STORE (Layer 4)
// ════════════════════════════════════════════════════════════════════

/// 12. Artifact round-trip preserves provenance.
#[tokio::test]
async fn artifact_round_trip_preserves_provenance() {
    let mem = MemoryFacade::in_memory();
    let task = TaskId::new();
    let prov = provenance(&task);
    let artifact = Artifact::new(
        "order_summary",
        serde_json::json!({"status": "shipped", "total": 9800}),
        prov.clone(),
    );
    let id = artifact.artifact_id.clone();

    mem.artifact_put(artifact).await.unwrap();
    let retrieved = mem.artifact_get(&id).await.unwrap();

    assert_eq!(retrieved.provenance.task_id, task);
    assert_eq!(
        retrieved.provenance.producing_agent.as_str(),
        "report-agent"
    );
    assert_eq!(
        retrieved.provenance.capability.as_str(),
        "cap-report-generate"
    );
    assert_eq!(
        retrieved.provenance.model_used.as_deref(),
        Some("claude-3-haiku")
    );
}

/// 13. Artifact content and metadata are intact after retrieval.
#[tokio::test]
async fn artifact_content_intact() {
    let mem = MemoryFacade::in_memory();
    let task = TaskId::new();
    let mut artifact = Artifact::new(
        "sales_report",
        serde_json::json!({"revenue": 42000, "region": "APAC"}),
        provenance(&task),
    );
    artifact.confidence = 0.95;
    artifact.policy_tags = vec!["internal".into(), "no-pii".into()];
    artifact.rendered = Some("# Sales Report\n\nRevenue: 42,000".into());
    let id = artifact.artifact_id.clone();

    mem.artifact_put(artifact).await.unwrap();
    let r = mem.artifact_get(&id).await.unwrap();

    assert_eq!(r.artifact_type, "sales_report");
    assert_eq!(r.content["revenue"], 42000);
    assert_eq!(r.content["region"], "APAC");
    assert!((r.confidence - 0.95).abs() < 1e-9);
    assert_eq!(r.policy_tags, vec!["internal", "no-pii"]);
    assert!(r.rendered.unwrap().contains("Sales Report"));
}

// ════════════════════════════════════════════════════════════════════
// CONTEXT BUDGET (Rule 10)
// ════════════════════════════════════════════════════════════════════

/// 14. Default budget matches PROJECT.md §3.6.
#[test]
fn context_budget_default_matches_spec() {
    assert_eq!(DEFAULT_TOTAL_BUDGET, 7_500);

    let budget = ContextBudget::default();
    assert_eq!(budget.total, 7_500);
    assert_eq!(budget.system, 2_000);
    assert_eq!(budget.intent, 500);
    assert_eq!(budget.memory, 2_000);
    assert_eq!(budget.step, 1_000);
    assert_eq!(budget.tools, 2_000);

    // Section caps sum to 7,500
    let sum = budget.system + budget.intent + budget.memory + budget.step + budget.tools;
    assert_eq!(sum, budget.total);
}

/// 15. Token approximation: chars / 4.
#[test]
fn context_budget_token_approximation() {
    assert_eq!(ContextBudget::approx_tokens(""), 0);
    assert_eq!(ContextBudget::approx_tokens("abcd"), 1);
    assert_eq!(ContextBudget::approx_tokens("abcdefgh"), 2);
    // 100 chars ≈ 25 tokens
    let s = "x".repeat(100);
    assert_eq!(ContextBudget::approx_tokens(&s), 25);
}

/// 16. Truncation respects per-section cap.
#[test]
fn context_budget_truncation() {
    // 10,000 chars with cap of 100 tokens (= 400 chars)
    let long_text = "a".repeat(10_000);
    let truncated = ContextBudget::truncate(&long_text, 100);
    assert_eq!(truncated.len(), 400, "100 tokens × 4 chars = 400 chars");
}

/// 17. Fit applies the correct section cap for each section.
#[test]
fn context_budget_fit_per_section() {
    let budget = ContextBudget::default();
    let long_text = "b".repeat(40_000);

    // System cap = 2,000 tokens = 8,000 chars
    let system = budget.fit(ContextSection::System, &long_text);
    assert_eq!(system.len(), 8_000);

    // Intent cap = 500 tokens = 2,000 chars
    let intent = budget.fit(ContextSection::Intent, &long_text);
    assert_eq!(intent.len(), 2_000);

    // Memory cap = 2,000 tokens = 8,000 chars
    let memory = budget.fit(ContextSection::Memory, &long_text);
    assert_eq!(memory.len(), 8_000);

    // Step cap = 1,000 tokens = 4,000 chars
    let step = budget.fit(ContextSection::Step, &long_text);
    assert_eq!(step.len(), 4_000);

    // Tools cap = 2,000 tokens = 8,000 chars
    let tools = budget.fit(ContextSection::Tools, &long_text);
    assert_eq!(tools.len(), 8_000);
}

/// 18. Short text passes through untruncated.
#[test]
fn context_budget_short_text_passthrough() {
    let budget = ContextBudget::default();
    let short = "Hello, this is a short intent.";
    let fitted = budget.fit(ContextSection::Intent, short);
    assert_eq!(fitted, short, "short text should not be truncated");
}

// ════════════════════════════════════════════════════════════════════
// FULL PIPELINE
// ════════════════════════════════════════════════════════════════════

/// 19. Multi-step workflow: store intermediate state in working memory,
///     execute a graph, store the result as an artifact, index it in
///     long-term memory, and retrieve by entity.
#[tokio::test]
async fn full_pipeline_working_to_artifact_to_longterm() {
    let mem = MemoryFacade::in_memory();
    let tenant = TenantId::from("tenant-jp");
    let task = TaskId::new();
    let order_ref = EntityRefLite::new("commerce.Order");

    // ── Step 1: Store intermediate state in working memory ────────
    mem.working_put(
        &task,
        "order_data",
        serde_json::json!({"id": "ord-42", "status": "pending", "total": 9800}),
    )
    .await
    .unwrap();

    // ── Step 2: Execute a graph ───────────────────────────────────
    let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
    let policy = Arc::new(PolicyEngine::with_default_rules());
    let intent = sample_intent();

    let node: Arc<dyn Node> = Arc::new(DeterministicNode::new(
        NodeId::from("cap-order-lookup"),
        SideEffect::Read,
        Arc::new(|_, _| {
            Ok(serde_json::json!({
                "id": "ord-42",
                "status": "pending",
                "total": 9800,
                "items": 3
            }))
        }),
    ));
    let graph = GraphBuilder::new().add_node(node).build().unwrap();

    let exec = GraphExecutor::new(policy, recorder.clone(), intent.budget);
    let outcome = exec.run(&graph, &intent).await.unwrap();
    assert!(matches!(outcome, ExecutionOutcome::Completed { .. }));

    // ── Step 3: Produce and store an artifact with provenance ─────
    let prov = ArtifactProvenance {
        intent_id: intent.intent_id.clone(),
        task_id: task.clone(),
        trace_id: intent.trace_id.clone(),
        producing_agent: AgentId::from("report-agent"),
        capability: CapabilityId::from("cap-report-generate"),
        data_sources: vec!["cap-order-lookup".into()],
        model_used: Some("claude-3-haiku".into()),
    };
    let mut artifact = Artifact::new(
        "order_report",
        serde_json::json!({"summary": "Order ord-42: 3 items, total 9800"}),
        prov,
    );
    artifact.derived_from = vec![order_ref.clone()];
    artifact.confidence = 0.92;
    let artifact_id = artifact.artifact_id.clone();
    mem.artifact_put(artifact).await.unwrap();

    // ── Step 4: Index the event in long-term memory ───────────────
    mem.longterm_insert(LongTermRecord {
        tenant: tenant.clone(),
        kind: "episodic".into(),
        content: "Generated order report for ord-42".into(),
        payload: serde_json::json!({"artifact_id": artifact_id.as_str()}),
        entity_refs: vec![order_ref.clone()],
    })
    .await
    .unwrap();

    // ── Step 5: Retrieve by entity ────────────────────────────────
    let hits = mem
        .longterm_search_by_entity(&tenant, &order_ref, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert!(hits[0].content.contains("ord-42"));

    // Follow the artifact_id from the long-term record
    let art_id_str = hits[0].payload["artifact_id"].as_str().unwrap();
    let art = mem
        .artifact_get(&ArtifactId::from(art_id_str))
        .await
        .unwrap();
    assert_eq!(art.artifact_type, "order_report");
    assert_eq!(art.content["summary"], "Order ord-42: 3 items, total 9800");

    // ── Step 6: Verify tenant isolation ───────────────────────────
    let other_tenant = TenantId::from("tenant-us");
    let empty = mem
        .longterm_search_by_entity(&other_tenant, &order_ref, 10)
        .await
        .unwrap();
    assert!(empty.is_empty(), "other tenant must not see JP data");

    // ── Step 7: Clean up working memory ───────────────────────────
    mem.working_clear(&task).await.unwrap();
    assert_eq!(mem.working_get(&task, "order_data").await.unwrap(), None);

    // ── Step 8: Verify trace was recorded ─────────────────────────
    let trace = recorder.get(&intent.trace_id).await.unwrap();
    assert_eq!(trace.status, aaf_contracts::TraceStatus::Completed);
}

/// 20. YAML config loads and parses successfully.
#[test]
fn aaf_yaml_loads_successfully() {
    let candidates = [
        "examples/memory-context/aaf.yaml",
        "../../examples/memory-context/aaf.yaml",
        "../../../examples/memory-context/aaf.yaml",
    ];
    let yaml = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("aaf.yaml should exist under examples/memory-context/");

    let doc: serde_yaml::Value =
        serde_yaml::from_str(&yaml).expect("aaf.yaml should be valid YAML");

    let budget = doc
        .get("context_budget")
        .expect("should have 'context_budget' key");
    assert_eq!(budget.get("total").and_then(|v| v.as_u64()).unwrap(), 7500);

    let memory = doc.get("memory").expect("should have 'memory' key");
    assert!(memory.get("working").is_some());
    assert!(memory.get("thread").is_some());
    assert!(memory.get("longterm").is_some());
    assert!(memory.get("artifacts").is_some());
}
