//! Wave 2 X1 Slice C — CLI / tooling integration test.
//!
//! Rather than re-invoke the server binary via `std::process::Command`
//! (which would add an untested shell-escape dependency), this test
//! exercises the same code paths the CLI uses by calling the
//! underlying `aaf-identity` + `aaf-federation` primitives through
//! their public APIs. A failing test here is a failing CLI.

use aaf_contracts::CapabilityId;
use aaf_federation::cosign::{cosign_token, verify_cosigned};
use aaf_identity::sbom::export::{to_cyclonedx_json, to_spdx_json};
use aaf_identity::{
    AgentSbom, CapabilityToken, InMemoryKeystore, Keystore, ManifestBuilder, RevocationEntry,
    RevocationKind, SbomEntry, SbomEntryKind,
};
use chrono::Duration;

#[test]
fn generate_did_is_deterministic_per_seed() {
    let ks_a = InMemoryKeystore::new();
    let ks_b = InMemoryKeystore::new();
    let did_a = ks_a.generate(b"signed-order-agent");
    let did_b = ks_b.generate(b"signed-order-agent");
    assert_eq!(did_a, did_b);
    assert!(did_a.is_well_formed());
}

#[test]
fn sign_then_verify_manifest_round_trip() {
    let ks = InMemoryKeystore::new();
    let did = ks.generate(b"did-seed-signed-order-agent");
    let manifest = ManifestBuilder::new(did, "signed-order-agent", "source-hash-abc")
        .allow(CapabilityId::from("cap-order-read"))
        .allow(CapabilityId::from("cap-order-cancel"))
        .ontology_slice("commerce")
        .eval_ref("order-processing-golden")
        .prompt_hash("abc123")
        .build(&ks)
        .unwrap();
    // The CLI's `verify` path is exactly this call.
    manifest.verify(&ks).unwrap();
}

#[test]
fn sbom_exports_as_spdx_and_cyclonedx_json() {
    let mut sbom = AgentSbom::new(aaf_identity::AgentDid::from_raw(
        "did:aaf:signed-order-agent",
    ));
    sbom.push(SbomEntry::from_bytes(
        SbomEntryKind::Model,
        "claude-sonnet-4",
        "sonnet-4",
        b"model-bytes",
    ));
    sbom.push(SbomEntry::from_bytes(
        SbomEntryKind::Prompt,
        "system",
        "v1",
        b"prompt-bytes",
    ));
    sbom.push(SbomEntry::from_bytes(
        SbomEntryKind::Tool,
        "cap-order-read",
        "1.0.0",
        b"tool-bytes",
    ));

    let spdx = to_spdx_json(&sbom);
    assert!(spdx.contains("\"spdxVersion\": \"SPDX-2.3\""));
    assert!(spdx.contains("\"checksumValue\""));

    let cdx = to_cyclonedx_json(&sbom);
    assert!(cdx.contains("\"bomFormat\": \"CycloneDX\""));
    assert!(cdx.contains("\"specVersion\": \"1.5\""));
    assert!(cdx.contains("\"alg\": \"SHA-256\""));
}

#[test]
fn revoke_produces_signed_entry_that_verifies() {
    let ks = InMemoryKeystore::new();
    let revoker = ks.generate(b"cli-revoker");
    let entry = RevocationEntry::issue(
        RevocationKind::Did,
        "did:aaf:compromised",
        "compromised key",
        revoker,
        &ks,
    )
    .unwrap();
    entry.verify(&ks).unwrap();
    assert_eq!(entry.target, "did:aaf:compromised");
    assert_eq!(entry.reason, "compromised key");
}

#[test]
fn federation_cosigned_token_survives_full_cli_round_trip() {
    // This simulates the "two cells federate via co-signed tokens"
    // story the Slice C CLI would print on a multi-cell deployment.
    let ks = InMemoryKeystore::new();
    let agent_issuer = ks.generate(b"agent-issuer");
    let agent_subject = ks.generate(b"agent-subject");
    let cell_tokyo = ks.generate(b"cell-tokyo");
    let cell_us = ks.generate(b"cell-us");

    let token = CapabilityToken::quick(
        agent_issuer,
        agent_subject,
        vec![CapabilityId::from("cap-cross-cell")],
        2,
        Duration::minutes(5),
        "jti-cli-cosign",
        &ks,
    )
    .unwrap();

    let cosigned = cosign_token(token, cell_tokyo, &ks, cell_us, &ks).unwrap();
    verify_cosigned(
        &cosigned,
        &ks,
        &ks,
        &ks,
        &CapabilityId::from("cap-cross-cell"),
    )
    .expect("co-signed token should verify");

    // Serde round trip — the CLI will eventually print these JSON-
    // encoded, so make sure they survive the trip.
    let json = serde_json::to_string_pretty(&cosigned).unwrap();
    let parsed: aaf_federation::CoSignedToken = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.issuer_cell, cosigned.issuer_cell);
    assert_eq!(parsed.receiver_cell, cosigned.receiver_cell);
}
