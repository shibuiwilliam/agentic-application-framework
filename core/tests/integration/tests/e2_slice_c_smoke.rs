//! E2 Slice C smoke test — federation agreements in entity-space.
//!
//! Slice C's architectural centrepiece is
//! [`aaf_federation::Router::enforce_capability`]: given a capability
//! whose `reads:` / `writes:` / `emits:` are declared against the
//! ontology, the router can enforce a cross-cell boundary by
//! *reading the same noun-layer declarations* the planner, policy
//! engine, memory system, and composition checker already use.
//! No parallel "prohibited_fields" list to maintain.
//!
//! This test proves:
//!
//! 1. An entity-space agreement allows a capability whose declared
//!    reads match the agreement's rules.
//! 2. The same router rejects a capability that touches an entity
//!    not named in the agreement.
//! 3. The classification cap rejects a capability whose declared
//!    `data_classification` exceeds the per-entity ceiling.
//! 4. A tenant-restricted rule rejects a capability whose entity
//!    ref carries a different tenant.
//!
//! These four cases together cover the four error variants on
//! [`aaf_federation::FederationError`] that did not exist before
//! iteration 8.

use aaf_contracts::{
    CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla,
    CompensationSpec, DataClassification, EndpointKind, EntityRefLite, SideEffect, TenantId,
};
use aaf_federation::{
    CellConfig, CellId, ClassificationCap, EntityAccessRule, EntityOp, FederationAgreement,
    FederationError, Router,
};

fn cap(id: &str) -> CapabilityContract {
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
        domains: vec![],
        reads: vec![],
        writes: vec![],
        emits: vec![],
        entity_scope: None,
        required_attestation_level: None,
        reputation: 0.5,
        learned_rules: vec![],
    }
}

fn router(rules: Vec<EntityAccessRule>) -> Router {
    let jp = CellConfig {
        id: CellId("cell-japan".into()),
        region: "ap-northeast-1".into(),
        local_capabilities: vec![],
    };
    let us = CellConfig {
        id: CellId("cell-us".into()),
        region: "us-east-1".into(),
        local_capabilities: vec![],
    };
    let agreement = FederationAgreement::with_entity_rules(
        vec![CellId("cell-japan".into()), CellId("cell-us".into())],
        vec![],
        rules,
    );
    Router::new(vec![jp, us], vec![agreement])
}

#[test]
fn entity_space_federation_allow_reject_cap_tenant() {
    // Jurisdiction: cell-japan routes capabilities at cell-us.
    let from = CellId("cell-japan".into());
    let to = CellId("cell-us".into());

    // Rules:
    //   commerce.Product  — read  — any class
    //   commerce.Order    — write — up to Internal only
    //   commerce.Payment  — write — tenant-a only
    let rules = vec![
        EntityAccessRule {
            entity_id: "commerce.Product".into(),
            op: EntityOp::Read,
            max_classification: None,
            tenant: None,
        },
        EntityAccessRule {
            entity_id: "commerce.Order".into(),
            op: EntityOp::Write,
            max_classification: Some(ClassificationCap::Internal),
            tenant: None,
        },
        EntityAccessRule {
            entity_id: "commerce.Payment".into(),
            op: EntityOp::Write,
            max_classification: None,
            tenant: Some("tenant-a".into()),
        },
    ];
    let r = router(rules);

    // 1. ALLOW — reads commerce.Product.
    {
        let mut c = cap("cap-product-read");
        c.reads = vec![EntityRefLite::new("commerce.Product")];
        r.enforce_capability(&from, &to, &c).expect("should allow");
    }

    // 2. REJECT — writes commerce.Customer, which has no rule at all.
    {
        let mut c = cap("cap-customer-write");
        c.side_effect = SideEffect::Write;
        c.compensation = Some(CompensationSpec {
            endpoint: "cap-customer-undo".into(),
        });
        c.writes = vec![EntityRefLite::new("commerce.Customer")];
        let err = r.enforce_capability(&from, &to, &c).unwrap_err();
        assert!(matches!(err, FederationError::EntityNotPermitted { .. }));
    }

    // 3. REJECT — writes commerce.Order at Restricted class, above cap.
    {
        let mut c = cap("cap-order-restricted");
        c.side_effect = SideEffect::Write;
        c.compensation = Some(CompensationSpec {
            endpoint: "cap-order-undo".into(),
        });
        c.data_classification = DataClassification::Restricted;
        c.writes = vec![EntityRefLite::new("commerce.Order")];
        let err = r.enforce_capability(&from, &to, &c).unwrap_err();
        assert!(
            matches!(err, FederationError::ClassificationCapExceeded { .. }),
            "expected classification cap rejection, got {err:?}"
        );
    }

    // 4. REJECT — writes commerce.Payment for tenant-b, rule is
    //    bound to tenant-a.
    {
        let mut c = cap("cap-payment-tenant-b");
        c.side_effect = SideEffect::Payment;
        c.compensation = Some(CompensationSpec {
            endpoint: "cap-payment-refund".into(),
        });
        let mut payment_ref = EntityRefLite::new("commerce.Payment");
        payment_ref.tenant = Some(TenantId::from("tenant-b"));
        c.writes = vec![payment_ref];
        let err = r.enforce_capability(&from, &to, &c).unwrap_err();
        assert!(
            matches!(err, FederationError::TenantMismatch { .. }),
            "expected tenant mismatch, got {err:?}"
        );
    }

    // 5. ALLOW — writes commerce.Payment for tenant-a (matches rule).
    {
        let mut c = cap("cap-payment-tenant-a");
        c.side_effect = SideEffect::Payment;
        c.compensation = Some(CompensationSpec {
            endpoint: "cap-payment-refund".into(),
        });
        let mut payment_ref = EntityRefLite::new("commerce.Payment");
        payment_ref.tenant = Some(TenantId::from("tenant-a"));
        c.writes = vec![payment_ref];
        r.enforce_capability(&from, &to, &c)
            .expect("tenant-a should be allowed");
    }
}

#[test]
fn empty_entity_rules_fall_back_to_legacy_path() {
    // No entity rules at all — the router should fall back to the
    // legacy field-name enforcement, which with an empty payload is
    // a no-op. This keeps pre-Slice-C configs valid.
    let from = CellId("cell-japan".into());
    let to = CellId("cell-us".into());
    let agreement = FederationAgreement::with_prohibited_fields(
        vec![from.clone(), to.clone()],
        vec![],
        std::collections::HashSet::new(),
    );
    let r = Router::new(
        vec![
            CellConfig {
                id: from.clone(),
                region: "ap".into(),
                local_capabilities: vec![],
            },
            CellConfig {
                id: to.clone(),
                region: "us".into(),
                local_capabilities: vec![],
            },
        ],
        vec![agreement],
    );
    let mut c = cap("cap-anything");
    c.reads = vec![EntityRefLite::new("commerce.Whatever")];
    r.enforce_capability(&from, &to, &c)
        .expect("legacy fallback should allow");
}
