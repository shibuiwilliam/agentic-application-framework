//! End-to-end test for examples/cross-cell-federation.
//!
//! Exercises AAF's third service-integration pattern — **Cell
//! Architecture + Federation** — which no other example covers:
//!
//! 1. **Cell routing** — a capability is resolved to the correct
//!    cell based on `local_capabilities`.
//! 2. **Data boundary enforcement** — a payload containing a
//!    prohibited field (`pii_email`) is blocked from crossing cells.
//! 3. **Co-signed capability tokens** — a delegation token signed by
//!    both cells is verified end-to-end.
//! 4. **Federation agreement parsing** — the `federation.yaml` from
//!    the example loads and its structure is validated.
//! 5. **Permitted cross-cell payload** — a payload without
//!    prohibited fields crosses the boundary successfully.
//! 6. **No-agreement rejection** — an attempt to route to a cell
//!    with no federation agreement fails cleanly.
//!
//! Run:
//!     cargo test -p aaf-integration-tests --test cross_cell_federation_e2e

use aaf_contracts::CapabilityId;
use aaf_federation::{
    cosign::{cosign_token, verify_cosigned},
    CellConfig, CellId, FederationAgreement, FederationError, Router,
};
use aaf_identity::{CapabilityToken, InMemoryKeystore, Keystore};
use chrono::Duration;
use std::collections::HashSet;

// ── Helpers ─────────────────────────────────────────────────────────

fn build_router() -> Router {
    let japan = CellConfig {
        id: CellId("cell-japan".into()),
        region: "ap-northeast-1".into(),
        local_capabilities: vec!["cap-jp-orders".into(), "cap-jp-inventory".into()],
    };
    let us = CellConfig {
        id: CellId("cell-us".into()),
        region: "us-east-1".into(),
        local_capabilities: vec!["cap-us-orders".into()],
    };
    let agreement = FederationAgreement {
        parties: vec![CellId("cell-japan".into()), CellId("cell-us".into())],
        shared_capabilities: vec!["cap-jp-orders".into(), "cap-us-orders".into()],
        prohibited_fields: ["pii_email".into(), "pii_phone".into()]
            .into_iter()
            .collect::<HashSet<_>>(),
        entity_rules: vec![], // E2 Slice C entity-space rules — not exercised here
    };
    Router::new(vec![japan, us], vec![agreement])
}

// ── Tests ───────────────────────────────────────────────────────────

#[test]
fn cell_routing_resolves_capability_to_owning_cell() {
    let router = build_router();

    let japan = router.route("cap-jp-orders");
    assert!(japan.is_some());
    assert_eq!(japan.unwrap().id, CellId("cell-japan".into()));

    let us = router.route("cap-us-orders");
    assert!(us.is_some());
    assert_eq!(us.unwrap().id, CellId("cell-us".into()));

    // Unknown capability → None
    assert!(router.route("cap-eu-orders").is_none());
}

#[test]
fn data_boundary_blocks_pii_fields() {
    let router = build_router();
    let from = CellId("cell-japan".into());
    let to = CellId("cell-us".into());

    // Payload with a prohibited field → blocked.
    let payload = serde_json::json!({
        "order_id": "ord-42",
        "total": 1280,
        "pii_email": "tanaka@example.com"
    });
    let err = router.enforce_outbound(&from, &to, &payload).unwrap_err();
    match err {
        FederationError::BoundaryViolation(field) => {
            assert_eq!(field, "pii_email");
        }
        other => panic!("expected BoundaryViolation, got {other:?}"),
    }
}

#[test]
fn clean_payload_crosses_boundary_successfully() {
    let router = build_router();
    let from = CellId("cell-japan".into());
    let to = CellId("cell-us".into());

    // Payload without prohibited fields → allowed.
    let payload = serde_json::json!({
        "order_id": "ord-42",
        "total": 1280,
        "region": "tokyo"
    });
    router
        .enforce_outbound(&from, &to, &payload)
        .expect("clean payload should be allowed");
}

#[test]
fn no_agreement_blocks_cross_cell_communication() {
    let router = build_router();
    let from = CellId("cell-japan".into());
    let to = CellId("cell-mars".into()); // no agreement

    let payload = serde_json::json!({"order_id": "ord-1"});
    let err = router.enforce_outbound(&from, &to, &payload).unwrap_err();
    assert!(matches!(err, FederationError::NoAgreement(_)));
}

#[test]
fn cosigned_token_requires_both_cell_signatures() {
    let ks = InMemoryKeystore::new();
    let agent_a = ks.generate(b"agent-jp");
    let agent_b = ks.generate(b"agent-us");
    let cell_jp = ks.generate(b"cell-jp-key");
    let cell_us = ks.generate(b"cell-us-key");

    // Issue a capability token from agent_a → agent_b.
    let token = CapabilityToken::quick(
        agent_a,
        agent_b,
        vec![CapabilityId::from("cap-jp-orders")],
        2,
        Duration::minutes(5),
        "jti-federation-1",
        &ks,
    )
    .unwrap();

    // Co-sign by both cells.
    let cosigned = cosign_token(token, cell_jp, &ks, cell_us, &ks).unwrap();

    // Verification passes with both cells' keystores.
    verify_cosigned(
        &cosigned,
        &ks,
        &ks,
        &ks,
        &CapabilityId::from("cap-jp-orders"),
    )
    .expect("co-signed token should verify");

    // Tampering with the issuer cell signature → rejected.
    let mut tampered = cosigned;
    tampered.issuer_cell_sig.push('0');
    let err = verify_cosigned(
        &tampered,
        &ks,
        &ks,
        &ks,
        &CapabilityId::from("cap-jp-orders"),
    )
    .unwrap_err();
    assert!(
        format!("{err}").contains("issuer"),
        "should mention issuer signature"
    );
}

#[test]
fn cosigned_token_rejects_out_of_scope_capability() {
    let ks = InMemoryKeystore::new();
    let agent_a = ks.generate(b"agent-jp");
    let agent_b = ks.generate(b"agent-us");
    let cell_jp = ks.generate(b"cell-jp-key");
    let cell_us = ks.generate(b"cell-us-key");

    let token = CapabilityToken::quick(
        agent_a,
        agent_b,
        vec![CapabilityId::from("cap-jp-orders")],
        2,
        Duration::minutes(5),
        "jti-federation-2",
        &ks,
    )
    .unwrap();

    let cosigned = cosign_token(token, cell_jp, &ks, cell_us, &ks).unwrap();

    // Verify against a DIFFERENT capability → rejected (scope check).
    let err = verify_cosigned(
        &cosigned,
        &ks,
        &ks,
        &ks,
        &CapabilityId::from("cap-us-inventory"),
    )
    .unwrap_err();
    assert!(
        format!("{err}").contains("scope")
            || format!("{err}").contains("NotInScope")
            || format!("{err}").contains("grant capability"),
        "should mention scope/grant: {err}"
    );
}

#[test]
fn federation_yaml_parses_successfully() {
    let candidates = [
        "examples/cross-cell-federation/federation.yaml",
        "../../examples/cross-cell-federation/federation.yaml",
        "../../../examples/cross-cell-federation/federation.yaml",
    ];
    let yaml = candidates
        .iter()
        .find_map(|p| std::fs::read_to_string(p).ok())
        .expect("federation.yaml should exist under examples/cross-cell-federation/");

    #[derive(serde::Deserialize)]
    struct FedConfig {
        cells: Vec<CellEntry>,
        agreements: Vec<AgreementEntry>,
    }
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct CellEntry {
        id: String,
        region: String,
        local_capabilities: Vec<String>,
    }
    #[derive(serde::Deserialize)]
    #[allow(dead_code)]
    struct AgreementEntry {
        parties: Vec<String>,
        shared_capabilities: Vec<String>,
        prohibited_fields: Vec<String>,
    }

    let config: FedConfig = serde_yaml::from_str(&yaml).expect("should parse");
    assert_eq!(config.cells.len(), 2, "two cells: Japan + US");
    assert_eq!(config.cells[0].id, "cell-japan");
    assert_eq!(config.cells[1].id, "cell-us");
    assert_eq!(config.agreements.len(), 1, "one bilateral agreement");
    assert_eq!(config.agreements[0].parties.len(), 2);
    assert_eq!(
        config.agreements[0].prohibited_fields.len(),
        2,
        "pii_email + pii_phone prohibited"
    );
}
