//! X1 agent identity smoke test.
//!
//! Exercises the full Wave 2 X1 Slice A flow across crate
//! boundaries:
//!
//! 1. Generate two DIDs (issuer + subject) in a keystore.
//! 2. Build a signed [`aaf_identity::AgentManifest`] for the subject.
//! 3. Assemble a matching [`aaf_identity::AgentSbom`].
//! 4. Issue a short-lived capability token from issuer → subject
//!    over a specific capability.
//! 5. Verify the token, verify the manifest, verify the SBOM hash.
//! 6. Revoke the subject in an [`aaf_identity::InMemoryRevocationRegistry`].
//! 7. Prove the revocation is visible.
//! 8. Tamper with the token's scope and prove the signature no
//!    longer verifies.

use aaf_contracts::CapabilityId;
use aaf_identity::{
    Attestation, AttestationLevel, CapabilityToken, InMemoryKeystore, InMemoryRevocationRegistry,
    Keystore, ManifestBuilder, RevocationEntry, RevocationKind, RevocationRegistry, SbomEntry,
    SbomEntryKind,
};
use chrono::Duration;

#[tokio::test]
async fn end_to_end_identity_flow() {
    // 1. Keystore with two DIDs.
    let ks = InMemoryKeystore::new();
    let issuer_did = ks.generate(b"order-reviewer");
    let subject_did = ks.generate(b"order-agent");
    assert!(issuer_did.is_well_formed());
    assert!(subject_did.is_well_formed());
    assert_ne!(issuer_did, subject_did);

    // 2. Build and verify a manifest.
    let manifest = ManifestBuilder::new(subject_did.clone(), "order-agent", "source-hash-abc")
        .allow(CapabilityId::from("cap-order-read"))
        .allow(CapabilityId::from("cap-order-cancel"))
        .ontology_slice("commerce")
        .eval_ref("order-processing-golden")
        .build(&ks)
        .expect("build");
    manifest.verify(&ks).expect("manifest verify");

    // 3. SBOM with a few entries and a stable content hash.
    let mut sbom = aaf_identity::AgentSbom::new(subject_did.clone());
    sbom.push(SbomEntry::from_bytes(
        SbomEntryKind::Prompt,
        "system",
        "v1",
        b"prompt content",
    ));
    sbom.push(SbomEntry::from_bytes(
        SbomEntryKind::Model,
        "claude-sonnet-4-20250514",
        "sonnet-4",
        b"model-ref",
    ));
    let hash_a = sbom.content_hash();
    let hash_b = sbom.content_hash();
    assert_eq!(hash_a, hash_b);

    // 4. Issue a capability token from the issuer to the subject.
    let mut token = CapabilityToken::quick(
        issuer_did.clone(),
        subject_did.clone(),
        vec![CapabilityId::from("cap-order-cancel")],
        3,
        Duration::minutes(5),
        "jti-smoke-1",
        &ks,
    )
    .expect("issue token");

    // 5. Verify the token is good for the capability it was issued
    //    for, and refused for one it wasn't.
    token
        .verify(&ks, &CapabilityId::from("cap-order-cancel"))
        .expect("token verify");
    let err = token
        .verify(&ks, &CapabilityId::from("cap-order-read"))
        .unwrap_err();
    assert!(matches!(err, aaf_identity::IdentityError::NotInScope(_)));

    // 6. Issue an attestation from the issuer confirming the
    //    manifest has been human-reviewed.
    let attestation = Attestation::issue(
        issuer_did.clone(),
        subject_did.clone(),
        &manifest,
        AttestationLevel::HumanReviewed,
        "reviewed by ops on 2026-04-12",
        None,
        &ks,
    )
    .expect("issue attestation");
    attestation.verify(&ks).expect("attestation verify");
    assert!(attestation.grants(AttestationLevel::AutoVerified));
    assert!(attestation.grants(AttestationLevel::HumanReviewed));
    assert!(!attestation.grants(AttestationLevel::Certified));

    // 7. Revoke the subject's DID in the registry and confirm it.
    let reg = InMemoryRevocationRegistry::new();
    let revocation = RevocationEntry::issue(
        RevocationKind::Did,
        subject_did.to_string(),
        "compromised key",
        issuer_did.clone(),
        &ks,
    )
    .expect("issue revocation");
    revocation.verify(&ks).expect("revocation signature");
    reg.revoke(revocation).await.unwrap();
    assert!(
        reg.is_revoked(&RevocationKind::Did, subject_did.as_str())
            .await,
        "subject DID should be in the revocation registry"
    );

    // 8. Tampering with the token's scope invalidates the signature.
    token
        .claims
        .scope
        .push(CapabilityId::from("cap-order-read"));
    let err = token
        .verify(&ks, &CapabilityId::from("cap-order-cancel"))
        .unwrap_err();
    assert_eq!(err, aaf_identity::IdentityError::InvalidSignature);
}
