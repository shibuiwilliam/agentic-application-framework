//! E2 Slice B smoke test.
//!
//! Proves that the ontology (Slice A) now reaches all four hot crates:
//!
//! 1. **`aaf-intent::enricher`** — populates
//!    `IntentEnvelope.entities_in_context` from an ontology-backed
//!    resolver.
//! 2. **`aaf-registry::discovery`** — finds capabilities by declared
//!    entity (reads / writes / emits).
//! 3. **`aaf-planner::composition`** — the entity-aware composition
//!    checker rejects a plan whose two capabilities both write the
//!    same entity (`commerce.Order`).
//! 4. **`aaf-policy::rules::boundary`** — the boundary rule
//!    consults an `OntologyClassificationLookup` and flags a
//!    capability that reads a `Pii`-classified entity while declaring
//!    `Public` output.
//! 5. **`aaf-memory::longterm`** — entity-keyed retrieval returns the
//!    record indexed under `commerce.Order`.
//!
//! This is one test, deliberately. Slice B's whole point is that a
//! single intent now touches every layer with the same entity
//! vocabulary — so a single test is the right granularity to prove
//! that the layers *agree*.

use aaf_contracts::{
    BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
    CapabilitySla, CompensationSpec, DataClassification, EndpointKind, EntityRefLite,
    IntentEnvelope, IntentId, IntentType, PolicyDecision, Requester, RiskTier, SideEffect,
    TenantId, TraceId,
};
use aaf_intent::{Enricher, OntologyResolver};
use aaf_memory::MemoryFacade;
use aaf_ontology::{
    entity::{Classification, Entity},
    InMemoryOntologyRegistry, OntologyRegistry,
};
use aaf_planner::{
    BoundedAutonomy, CompositionChecker, EntityAwareComposition, PlannerError, RegistryPlanner,
};
use aaf_policy::{
    EntityClass, OntologyClassificationLookup, PolicyContext, PolicyEngine, PolicyHook,
};
use aaf_registry::{EntityQueryKind, Registry};
use aaf_storage::memory::LongTermRecord;
use chrono::Utc;
use std::sync::Arc;

fn cap_base(id: &str) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: id.into(),
        description: id.into(),
        version: "1.0".into(),
        provider_agent: "agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::Grpc,
            address: "x".into(),
            method: None,
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
        required_scope: "x:read".into(),
        data_classification: DataClassification::Internal,
        degradation: vec![],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec![],
        domains: vec!["commerce".into()],
        reads: vec![],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.5,
        learned_rules: vec![],
    }
}

fn sample_intent(tenant: &TenantId) -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::TransactionalIntent,
        requester: Requester {
            user_id: "alice".into(),
            role: "ops".into(),
            scopes: vec![
                "commerce:read".into(),
                "inventory:write".into(),
                "auto-approve".into(),
            ],
            tenant: Some(tenant.clone()),
        },
        goal: "reserve stock for order 42".into(),
        domain: "commerce".into(),
        constraints: Default::default(),
        budget: BudgetContract {
            max_tokens: 500,
            max_cost_usd: 0.5,
            max_latency_ms: 2000,
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

#[tokio::test]
async fn e2_slice_b_ontology_reaches_every_hot_crate() {
    // ── 0. Register a small commerce ontology. ─────────────────────────
    let ontology = Arc::new(InMemoryOntologyRegistry::new());
    let order = Entity::new(
        "commerce.Order",
        "a customer order",
        Classification::Internal,
    );
    let customer = Entity::new("commerce.Customer", "a customer (PII)", Classification::Pii);
    ontology.upsert(order, false).await.unwrap();
    ontology.upsert(customer, false).await.unwrap();

    // ── 1. Register entity-declaring capabilities. ─────────────────────
    let registry = Arc::new(Registry::in_memory());

    // Reader — reads Order, Internal classification (safe).
    let mut read_order = cap_base("cap-order-read");
    read_order.name = "order read lookup".into();
    read_order.description = "read order details".into();
    read_order.reads = vec![EntityRefLite::new("commerce.Order")];
    registry.register(read_order.clone()).await.unwrap();

    // Reserve writer — writes Order.
    let mut reserve = cap_base("cap-order-reserve");
    reserve.name = "order reserve".into();
    reserve.description = "reserve order finalize".into();
    reserve.side_effect = SideEffect::Write;
    reserve.compensation = Some(CompensationSpec {
        endpoint: "cap-order-release".into(),
    });
    reserve.writes = vec![EntityRefLite::new("commerce.Order")];
    registry.register(reserve.clone()).await.unwrap();

    // Confirm writer — also writes Order (double-write). Made to win
    // the lexical score against `sample_intent().goal` so the
    // dependency-pull loop in the planner picks up `reserve`.
    let mut confirm = cap_base("cap-order-confirm");
    confirm.name = "order confirm reserve stock finalize".into();
    confirm.description = "confirm order reserve stock finalize".into();
    confirm.side_effect = SideEffect::Write;
    confirm.compensation = Some(CompensationSpec {
        endpoint: "cap-order-reopen".into(),
    });
    confirm.writes = vec![EntityRefLite::new("commerce.Order")];
    confirm.depends_on = vec![CapabilityId::from("cap-order-reserve")];
    registry.register(confirm.clone()).await.unwrap();

    // ── 2. Registry discovery by entity. ───────────────────────────────
    let order_ref = EntityRefLite::new("commerce.Order");
    let writers = registry
        .discover_by_entity(&order_ref, EntityQueryKind::Writes)
        .await
        .unwrap();
    assert_eq!(
        writers.len(),
        2,
        "registry should surface both writers of commerce.Order"
    );
    let readers = registry
        .discover_by_entity(&order_ref, EntityQueryKind::Reads)
        .await
        .unwrap();
    assert_eq!(readers.len(), 1);
    assert_eq!(readers[0].id.as_str(), "cap-order-read");

    // ── 3. Intent compiler enricher populates entities_in_context. ─────
    let tenant = TenantId::from("tenant-a");
    let mut env = sample_intent(&tenant);

    // Resolver: pull every ontology entity whose id shares the intent
    // domain prefix.
    let onto_for_resolver = ontology.clone();
    let resolver = move |domain: &str, _goal: &str| -> Vec<aaf_contracts::EntityRefLite> {
        // Async list → sync collect via blocking block_in_place is
        // overkill for a test; instead we pre-list in a sync shim
        // by spawning a tiny runtime. But we already have an async
        // ontology registry, so easiest: pre-list before making
        // the resolver.
        let _ = domain;
        vec![]
    };
    // Do the pre-list here (cheap, deterministic) so the resolver
    // closure is sync.
    let all_entities = onto_for_resolver.list().await.unwrap();
    let domain = env.domain.clone();
    let by_prefix: Vec<EntityRefLite> = all_entities
        .into_iter()
        .filter(|e| e.id.starts_with(&format!("{domain}.")))
        .map(|e| EntityRefLite::new(e.id))
        .collect();
    let domain_for_closure = domain.clone();
    let by_prefix_clone = by_prefix.clone();
    let real_resolver: Box<dyn OntologyResolver> =
        Box::new(move |d: &str, _g: &str| -> Vec<EntityRefLite> {
            if d == domain_for_closure {
                by_prefix_clone.clone()
            } else {
                vec![]
            }
        });
    let _ = resolver; // silence unused
    Enricher::enrich_with_ontology(&mut env, real_resolver.as_ref());
    assert_eq!(env.entities_in_context.len(), 2);
    assert!(env
        .entities_in_context
        .iter()
        .all(|e| e.tenant.as_ref().map(|t| t.as_str()) == Some("tenant-a")));

    // ── 4. Entity-aware composition rejects the double-write plan. ─────
    let planner = RegistryPlanner::new(
        registry.clone(),
        BoundedAutonomy::default(),
        CompositionChecker::default(),
    )
    .with_entity_composition(EntityAwareComposition::new(CompositionChecker::default()));

    let mut plan_env = env.clone();
    plan_env.goal = "confirm order reserve stock finalize".into();
    let err = planner.plan(&plan_env).await.unwrap_err();
    assert!(
        matches!(err, PlannerError::UnsafeEntityComposition(_)),
        "planner must reject double-write on commerce.Order, got {err:?}"
    );

    // ── 5. Boundary rule flags a classification leak. ──────────────────
    //
    // Construct a capability that *reads* `commerce.Customer` (Pii) but
    // advertises `Public` output — the ontology lookup must surface
    // the flow violation as a Fatal boundary violation.
    let mut leaky = cap_base("cap-customer-profile");
    leaky.reads = vec![EntityRefLite::new("commerce.Customer")];
    leaky.data_classification = DataClassification::Public;

    let onto_for_lookup = ontology.clone();
    // Pre-fetch the classification map so the sync closure stays sync.
    let all_entities2 = onto_for_lookup.list().await.unwrap();
    let class_map: std::collections::HashMap<String, Classification> = all_entities2
        .into_iter()
        .map(|e| (e.id, e.classification))
        .collect();
    let lookup: OntologyClassificationLookup = Arc::new(move |id: &str| {
        class_map.get(id).map(|c| match c {
            Classification::Public => EntityClass::Public,
            Classification::Internal => EntityClass::Internal,
            Classification::Pii => EntityClass::Pii,
            Classification::Regulated(tag) => EntityClass::Regulated(tag.clone()),
        })
    });

    let engine = PolicyEngine::with_default_rules();
    let mut probe_env = env.clone();
    probe_env.requester.scopes.push("confidential:read".into());
    let ctx = PolicyContext {
        intent: &probe_env,
        capability: Some(&leaky),
        requester: &probe_env.requester,
        payload: None,
        output: None,
        side_effect: Some(SideEffect::Read),
        remaining_budget: probe_env.budget,
        tenant: Some(&tenant),
        composed_writes: 0,
        ontology_class_lookup: Some(lookup),
    };
    let decision = engine.evaluate(PolicyHook::PreStep, &ctx);
    assert!(
        matches!(decision, PolicyDecision::Deny(_)),
        "boundary rule must deny classification leak, got {decision:?}"
    );

    // ── 6. Long-term memory entity-keyed retrieval. ────────────────────
    let memory = MemoryFacade::in_memory();
    let mut indexed_order = EntityRefLite::new("commerce.Order");
    indexed_order.tenant = Some(tenant.clone());
    memory
        .longterm_insert(LongTermRecord {
            tenant: tenant.clone(),
            kind: "episodic".into(),
            content: "order-42 was reserved at 10:00".into(),
            payload: serde_json::json!({"order_id": "ord-42"}),
            entity_refs: vec![indexed_order],
        })
        .await
        .unwrap();

    let hits = memory
        .longterm_search_by_entity(&tenant, &order_ref, 10)
        .await
        .unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].entity_refs[0].entity_id, "commerce.Order");

    // Sanity check: cross-tenant lookup returns empty.
    let other_tenant = TenantId::from("tenant-b");
    let empty = memory
        .longterm_search_by_entity(&other_tenant, &order_ref, 10)
        .await
        .unwrap();
    assert!(empty.is_empty());
}
