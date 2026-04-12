//! Wave 2 X1 Slice B — cross-crate hot-path integration.
//!
//! Exercises the full cryptographic-identity story across
//! `aaf-identity`, `aaf-trust`, `aaf-registry`, and `aaf-runtime` in
//! a single scenario:
//!
//! 1. Seed a keystore with a producer DID.
//! 2. Sign an artifact with `sign_artifact_with`; verify it.
//! 3. Issue a capability token and verify it through
//!    `aaf_trust::verify_token`.
//! 4. Register a capability with `required_attestation_level =
//!    HumanReviewed` and prove that `Registry::get_for_attestation`
//!    refuses `Unattested` callers but serves `HumanReviewed` or
//!    above.
//! 5. Revoke the producer DID; wire a `GraphExecutor` with the
//!    revocation registry; prove that running an intent whose
//!    requester carries the revoked DID fails fast with
//!    `RuntimeError::Revoked` and leaves no open trace.
//! 6. Prove that tampering with the artifact after signing
//!    invalidates the Wave 2 `verify_artifact_with` check.

use aaf_contracts::{
    AttestationLevelRef, BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint,
    CapabilityId, CapabilitySla, DataClassification, EndpointKind, EntityRefLite, IntentEnvelope,
    IntentId, IntentType, NodeId, Requester, RiskTier, SideEffect, TraceId,
};
use aaf_identity::{
    CapabilityToken, InMemoryKeystore, InMemoryRevocationRegistry, Keystore, RevocationEntry,
    RevocationKind, RevocationRegistry,
};
use aaf_policy::PolicyEngine;
use aaf_registry::{Registry, RegistryError};
use aaf_runtime::node::DeterministicNode;
use aaf_runtime::{GraphBuilder, GraphExecutor, Node, RuntimeError};
use aaf_trace::{Recorder, TraceRecorder};
use aaf_trust::{sign_artifact_with, verify_artifact_with, verify_token};
use chrono::{Duration, Utc};
use std::sync::Arc;

fn gated_capability(id: &str, level: AttestationLevelRef) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: id.into(),
        description: id.into(),
        version: "1.0.0".into(),
        provider_agent: "gated".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::InProcess,
            address: id.into(),
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
        required_scope: "gated:read".into(),
        data_classification: DataClassification::Internal,
        degradation: vec![],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec![],
        domains: vec![],
        reads: vec![EntityRefLite::new("commerce.Order")],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: Some(level),
        reputation: 0.5,
        learned_rules: vec![],
    }
}

fn intent_for(requester_did: &str) -> IntentEnvelope {
    IntentEnvelope {
        intent_id: IntentId::new(),
        intent_type: IntentType::AnalyticalIntent,
        requester: Requester {
            user_id: requester_did.to_string(),
            role: "analyst".into(),
            scopes: vec!["gated:read".into()],
            tenant: None,
        },
        goal: "gated read".into(),
        domain: "commerce".into(),
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
async fn x1_slice_b_end_to_end_hot_path() {
    // 1. Keystore with producer + reviewer DIDs.
    let ks = InMemoryKeystore::new();
    let producer = ks.generate(b"producer-agent");
    let reviewer = ks.generate(b"reviewer-agent");

    // 2. Sign a synthetic artifact with the producer's DID.
    let mut artifact = aaf_contracts::Artifact::new(
        "sales-summary",
        serde_json::json!({"region": "tokyo", "total": 1234.0}),
        aaf_contracts::ArtifactProvenance {
            intent_id: IntentId::new(),
            task_id: aaf_contracts::TaskId::new(),
            trace_id: TraceId::new(),
            producing_agent: aaf_contracts::AgentId::from(producer.as_str()),
            capability: CapabilityId::from("cap-sales-read"),
            data_sources: vec![],
            model_used: None,
        },
    );
    sign_artifact_with(&mut artifact, &producer, &ks).unwrap();
    assert!(verify_artifact_with(&artifact, &ks).unwrap());

    // 3. Issue + verify a capability token (trust + identity crates).
    let token = CapabilityToken::quick(
        producer.clone(),
        reviewer.clone(),
        vec![CapabilityId::from("cap-sales-read")],
        2,
        Duration::minutes(5),
        "jti-slice-b",
        &ks,
    )
    .unwrap();
    verify_token(&token, &ks, &CapabilityId::from("cap-sales-read")).unwrap();

    // 4. Registry attestation gate: register a HumanReviewed-gated
    //    capability and verify the three-way access matrix.
    let reg = Registry::in_memory();
    let cap = gated_capability("cap-gated-read", AttestationLevelRef::HumanReviewed);
    reg.register(cap.clone()).await.unwrap();

    // Unattested caller → denied.
    let err = reg
        .get_for_attestation(&cap.id, AttestationLevelRef::Unattested)
        .await
        .unwrap_err();
    match err {
        RegistryError::InsufficientAttestation { required, .. } => {
            assert_eq!(required, AttestationLevelRef::HumanReviewed);
        }
        other => panic!("expected InsufficientAttestation, got {other:?}"),
    }

    // HumanReviewed → served.
    let got = reg
        .get_for_attestation(&cap.id, AttestationLevelRef::HumanReviewed)
        .await
        .unwrap();
    assert_eq!(got.id, cap.id);

    // Certified is strictly stronger → also served.
    let got_hi = reg
        .get_for_attestation(&cap.id, AttestationLevelRef::Certified)
        .await
        .unwrap();
    assert_eq!(got_hi.id, cap.id);

    // 5. Revoke the producer and prove the runtime refuses to execute
    //    an intent on their behalf.
    let revocation_reg: Arc<dyn RevocationRegistry> = Arc::new(InMemoryRevocationRegistry::new());
    let entry = RevocationEntry::issue(
        RevocationKind::Did,
        producer.to_string(),
        "compromised",
        reviewer,
        &ks,
    )
    .unwrap();
    revocation_reg.revoke(entry).await.unwrap();

    let graph = GraphBuilder::new()
        .add_node(Arc::new(DeterministicNode::new(
            NodeId::from("noop"),
            SideEffect::None,
            Arc::new(|_, _| Ok(serde_json::json!({}))),
        )) as Arc<dyn Node>)
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
    .with_revocation(revocation_reg);

    let intent = intent_for(producer.as_str());
    let err = exe.run(&graph, &intent).await.unwrap_err();
    match err {
        RuntimeError::Revoked { did, .. } => assert_eq!(did, producer.to_string()),
        other => panic!("expected Revoked, got {other:?}"),
    }

    // Revoked attempt must not have left an open trace.
    let trace = recorder.get(&intent.trace_id).await;
    assert!(trace.is_err(), "revoked intent must leave no partial trace");

    // 6. Tampering with the artifact after signing invalidates
    //    verify_artifact_with.
    let mut tampered = artifact.clone();
    tampered.content = serde_json::json!({"region": "osaka", "total": 9999.0});
    assert!(
        !verify_artifact_with(&tampered, &ks).unwrap(),
        "tampered artifact must fail verification"
    );
}
