//! E2E tests for PROJECT_AafService §3.3 (registration pipeline)
//! and §7.3 (degradation chain).
//!
//! These prove the service-integration story works cross-crate:
//!
//! 1. **Registration pipeline** — validate → health check → conflict
//!    detect → initial trust → ready to discover.
//! 2. **Degradation chain** — Full → Partial → Cached → Unavailable
//!    cycles through the state machine correctly.
//! 3. **Circuit breaker + degradation** — breaker trips → capability
//!    degrades → breaker resets → capability recovers.
//! 4. **Scope intersection** — the §8.2 permission model computes
//!    effective scopes correctly across user × intent × autonomy.

use aaf_contracts::{
    AutonomyLevel, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
    CapabilitySla, DataClassification, DegradationLevel, DegradationSpec, EndpointKind, SideEffect,
};
use aaf_policy::effective_scopes;
use aaf_registry::{
    BreakerConfig, BreakerLevel, CircuitBreakerRegistry, DegradationStateMachine,
    RegistrationPipeline, Registry,
};
use aaf_trust::TrustRegistry;
use chrono::Duration;
use std::sync::Arc;

fn test_cap(id: &str) -> CapabilityContract {
    CapabilityContract {
        id: CapabilityId::from(id),
        name: id.into(),
        description: format!("{id} capability"),
        version: "1.0.0".into(),
        provider_agent: "test-agent".into(),
        endpoint: CapabilityEndpoint {
            kind: EndpointKind::Grpc,
            address: "svc:50051".into(),
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
        required_scope: "test:read".into(),
        data_classification: DataClassification::Internal,
        degradation: vec![
            DegradationSpec {
                level: DegradationLevel::Full,
                description: "all warehouses real-time".into(),
                trigger: None,
                fallback: None,
            },
            DegradationSpec {
                level: DegradationLevel::Partial,
                description: "primary only, 15 min delay".into(),
                trigger: Some("primary_db_slow".into()),
                fallback: None,
            },
            DegradationSpec {
                level: DegradationLevel::Cached,
                description: "1 hour old cache".into(),
                trigger: Some("db_unreachable".into()),
                fallback: None,
            },
            DegradationSpec {
                level: DegradationLevel::Unavailable,
                description: "manual verification needed".into(),
                trigger: Some("total_failure".into()),
                fallback: Some("ask human".into()),
            },
        ],
        depends_on: vec![],
        conflicts_with: vec![],
        tags: vec![],
        domains: vec!["test".into()],
        reads: vec![],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.5,
        learned_rules: vec![],
    }
}

// ── §3.3 Registration pipeline ──────────────────────────────────────

#[tokio::test]
async fn registration_pipeline_validates_and_assigns_trust() {
    let reg = Arc::new(Registry::in_memory());
    let trust = Arc::new(TrustRegistry::new());
    let pipeline = RegistrationPipeline::new(reg.clone(), trust);
    let result = pipeline
        .register(test_cap("cap-inventory"), false)
        .await
        .unwrap();
    assert_eq!(result.capability_id, "cap-inventory");
    assert!(!result.replaced);
    assert!(result.health_ok);
    assert!(
        (result.initial_trust - 0.5).abs() < 1e-9,
        "initial trust should be 0.5"
    );

    // Capability is now discoverable.
    let found = reg.get(&CapabilityId::from("cap-inventory")).await.unwrap();
    assert_eq!(found.name, "cap-inventory");
}

#[tokio::test]
async fn registration_rejects_write_without_compensation() {
    let reg = Arc::new(Registry::in_memory());
    let trust = Arc::new(TrustRegistry::new());
    let pipeline = RegistrationPipeline::new(reg, trust);
    let mut cap = test_cap("cap-bad-write");
    cap.side_effect = SideEffect::Write;
    // No compensation → Rule 9 violation.
    let err = pipeline.register(cap, false).await.unwrap_err();
    assert!(
        format!("{err}").contains("compensation")
            || format!("{err}").contains("invalid")
            || format!("{err}").contains("registry"),
        "should mention compensation: {err}"
    );
}

#[tokio::test]
async fn registration_detects_conflict_and_allows_force_overwrite() {
    let reg = Arc::new(Registry::in_memory());
    let trust = Arc::new(TrustRegistry::new());
    let pipeline = RegistrationPipeline::new(reg, trust);
    pipeline
        .register(test_cap("cap-conflict"), false)
        .await
        .unwrap();
    // Second registration without force → conflict.
    let err = pipeline
        .register(test_cap("cap-conflict"), false)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("already registered"));
    // With force → success + replaced=true.
    let result = pipeline
        .register(test_cap("cap-conflict"), true)
        .await
        .unwrap();
    assert!(result.replaced);
}

#[tokio::test]
async fn registration_with_custom_health_check_blocks_on_failure() {
    let reg = Arc::new(Registry::in_memory());
    let trust = Arc::new(TrustRegistry::new());
    let failing: aaf_registry::registration::HealthCheck =
        Arc::new(|_| Err("connection refused".into()));
    let pipeline = RegistrationPipeline::with_health_check(reg, trust, failing);
    let err = pipeline
        .register(test_cap("cap-unhealthy"), false)
        .await
        .unwrap_err();
    assert!(format!("{err}").contains("health check failed"));
}

// ── §7.3 Degradation chain ──────────────────────────────────────────

#[test]
fn degradation_chain_cycles_through_all_levels() {
    let mut sm = DegradationStateMachine::new();
    assert_eq!(sm.current(), DegradationLevel::Full);

    let t1 = sm.degrade().unwrap();
    assert_eq!(t1.to, DegradationLevel::Partial);

    let t2 = sm.degrade().unwrap();
    assert_eq!(t2.to, DegradationLevel::Cached);

    let t3 = sm.degrade().unwrap();
    assert_eq!(t3.to, DegradationLevel::Unavailable);

    // Can't degrade further.
    assert!(sm.degrade().is_none());

    // Recovery.
    let r1 = sm.recover().unwrap();
    assert_eq!(r1.to, DegradationLevel::Cached);

    let r2 = sm.recover().unwrap();
    assert_eq!(r2.to, DegradationLevel::Partial);

    let r3 = sm.recover().unwrap();
    assert_eq!(r3.to, DegradationLevel::Full);

    assert!(sm.recover().is_none());
}

// ── Circuit breaker + degradation interaction ───────────────────────

#[test]
fn circuit_breaker_trips_and_recovers() {
    let breakers = CircuitBreakerRegistry::new();
    breakers.register(
        "cap-inventory",
        BreakerLevel::Capability,
        BreakerConfig {
            failure_threshold: 3,
            window: Duration::seconds(60),
            cooldown: Duration::seconds(0), // instant cooldown for test
        },
    );

    // 3 failures → trip.
    breakers.record_failure("cap-inventory", None);
    breakers.record_failure("cap-inventory", None);
    let tripped = breakers.record_failure("cap-inventory", Some("timeout".into()));
    assert!(tripped);

    // Cooldown of 0 → immediately transitions to HalfOpen on allows().
    assert!(breakers.allows("cap-inventory")); // HalfOpen probe

    // Success → Closed.
    breakers.record_success("cap-inventory");
    let snap = breakers.snapshot("cap-inventory").unwrap();
    assert_eq!(snap.state, aaf_registry::BreakerState::Closed);
    assert_eq!(snap.failure_count, 0);
}

// ── §8.2 Scope intersection ──────────────────────────────────────────

#[test]
fn scope_intersection_filters_by_intent_and_autonomy() {
    // User has broad scopes.
    let user = vec![
        "order:read".into(),
        "order:write".into(),
        "payment:execute".into(),
        "admin:delete".into(),
    ];
    // Intent only needs order + payment.
    let intent = vec![
        "order:read".into(),
        "order:write".into(),
        "payment:execute".into(),
    ];

    // Level 2 (read-only): only order:read should survive.
    let l2 = effective_scopes(&user, &intent, AutonomyLevel::Level2);
    assert_eq!(l2, vec!["order:read"]);

    // Level 3 (read + write): order:read + order:write.
    let l3 = effective_scopes(&user, &intent, AutonomyLevel::Level3);
    assert!(l3.contains(&"order:read".to_string()));
    assert!(l3.contains(&"order:write".to_string()));
    assert!(!l3.contains(&"payment:execute".to_string()));

    // Level 5 (full): all three intent scopes (admin:delete was
    // filtered out by the intent intersection).
    let l5 = effective_scopes(&user, &intent, AutonomyLevel::Level5);
    assert_eq!(l5.len(), 3);
    assert!(!l5.contains(&"admin:delete".to_string()));
}
