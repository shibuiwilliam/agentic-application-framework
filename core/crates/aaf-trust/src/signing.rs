//! Artifact signing.
//!
//! Two surface levels:
//!
//! - [`sign_artifact`] / [`verify_artifact`] — Wave 1 content-hash
//!   path kept for backward compatibility with existing callers that
//!   do not yet have a DID / keystore.
//! - [`sign_artifact_with`] / [`verify_artifact_with`] — Wave 2 X1
//!   Slice B, production path. Signs the content hash with the
//!   identity keystore and binds the resulting signature to the
//!   producing agent's DID. Rule 28 ("The SDK emits signed artifacts
//!   by default").
//!
//! The keystore is injected via `&dyn aaf_identity::Signer`, so Slice
//! B's HMAC-SHA256 in-memory backend and a future Slice C Ed25519 /
//! KMS backend both work without any call-site change.

use aaf_contracts::Artifact;
use aaf_identity::{AgentDid, IdentityError, Signer, Verifier};
use sha2::{Digest, Sha256};

/// Compute SHA-256 over the artifact content and write the hash and a
/// stubbed signature into the artifact in place. Legacy Wave 1 entry
/// point — new code should prefer [`sign_artifact_with`].
pub fn sign_artifact(artifact: &mut Artifact, signer: &str) {
    let serialised = serde_json::to_vec(&artifact.content).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&serialised);
    let hash = hex::encode(hasher.finalize());
    artifact.content_hash = Some(hash.clone());
    artifact.signature = Some(format!("v0:{signer}:{hash}"));
}

/// Verify the artifact hash matches the embedded `content_hash`.
/// Legacy Wave 1 entry point — new code should prefer
/// [`verify_artifact_with`].
pub fn verify_artifact(artifact: &Artifact) -> bool {
    let Ok(serialised) = serde_json::to_vec(&artifact.content) else {
        return false;
    };
    let mut hasher = Sha256::new();
    hasher.update(&serialised);
    let computed = hex::encode(hasher.finalize());
    artifact.content_hash.as_deref() == Some(computed.as_str())
}

/// Wave 2 X1 Slice B production signer: binds the artifact's
/// signature to the producing agent's DID via the identity
/// keystore's `Signer` trait.
///
/// Signature format on the wire:
///
/// ```text
/// x1:<did>:<detached-signature-hex>
/// ```
///
/// The leading `x1:` discriminator lets verifiers tell a Wave 2
/// signed artifact from a Wave 1 legacy (`v0:`) one. Anything else
/// is treated as unsigned.
pub fn sign_artifact_with(
    artifact: &mut Artifact,
    did: &AgentDid,
    signer: &dyn Signer,
) -> Result<(), IdentityError> {
    let serialised = serde_json::to_vec(&artifact.content)
        .map_err(|e| IdentityError::Serialisation(e.to_string()))?;
    let mut hasher = Sha256::new();
    hasher.update(&serialised);
    let hash = hex::encode(hasher.finalize());
    let detached = signer.sign(did, hash.as_bytes())?;
    artifact.content_hash = Some(hash);
    artifact.signature = Some(format!("x1:{did}:{detached}"));
    Ok(())
}

/// Verify an artifact's Wave 2 signature chain: parses the `x1:<did>:
/// <sig>` envelope, recomputes the content hash, and asks the
/// identity verifier to check the detached signature under the
/// claimed DID. Returns `false` for legacy Wave 1 envelopes, unsigned
/// artifacts, malformed signatures, tampered content, or wrong keys.
pub fn verify_artifact_with(
    artifact: &Artifact,
    verifier: &dyn Verifier,
) -> Result<bool, IdentityError> {
    let Some(signature) = artifact.signature.as_deref() else {
        return Ok(false);
    };
    // Parse `x1:<did>:<sig>`.
    let Some(rest) = signature.strip_prefix("x1:") else {
        return Ok(false); // legacy `v0:` — not a Wave 2 envelope
    };
    // The DID contains a single colon itself (`did:aaf:...`), so we
    // split on the *last* colon to recover the signature suffix.
    let Some(last_colon) = rest.rfind(':') else {
        return Ok(false);
    };
    let (did_str, sig) = rest.split_at(last_colon);
    let sig = &sig[1..]; // strip leading ':'
    let did = AgentDid::from_raw(did_str);

    // Recompute the content hash.
    let serialised = match serde_json::to_vec(&artifact.content) {
        Ok(b) => b,
        Err(e) => return Err(IdentityError::Serialisation(e.to_string())),
    };
    let mut hasher = Sha256::new();
    hasher.update(&serialised);
    let expected = hex::encode(hasher.finalize());
    if artifact.content_hash.as_deref() != Some(expected.as_str()) {
        return Ok(false);
    }
    match verifier.verify(&did, expected.as_bytes(), sig) {
        Ok(()) => Ok(true),
        Err(IdentityError::InvalidSignature) => Ok(false),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        AgentId, ArtifactId, ArtifactProvenance, CapabilityId, IntentId, TaskId, TraceId,
    };
    use chrono::Utc;

    fn art() -> Artifact {
        Artifact {
            artifact_id: ArtifactId::new(),
            artifact_type: "report".into(),
            content: serde_json::json!({"k": "v"}),
            rendered: None,
            provenance: ArtifactProvenance {
                intent_id: IntentId::new(),
                task_id: TaskId::new(),
                trace_id: TraceId::new(),
                producing_agent: AgentId::from("test"),
                capability: CapabilityId::from("cap-test"),
                data_sources: vec![],
                model_used: None,
            },
            confidence: 1.0,
            policy_tags: vec![],
            content_hash: None,
            signature: None,
            created_at: Utc::now(),
            expires_at: None,
            version: 1,
            derived_from: vec![],
        }
    }

    #[test]
    fn signed_artifacts_verify() {
        let mut a = art();
        sign_artifact(&mut a, "agent-1");
        assert!(verify_artifact(&a));
    }

    #[test]
    fn tampering_breaks_verification() {
        let mut a = art();
        sign_artifact(&mut a, "agent-1");
        a.content = serde_json::json!({"k": "v2"});
        assert!(!verify_artifact(&a));
    }

    // ── Wave 2 X1 Slice B — DID-bound signatures ──────────────────────

    #[test]
    fn sign_artifact_with_binds_to_did_and_round_trips() {
        use aaf_identity::{InMemoryKeystore, Keystore};
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"producing-agent");
        let mut a = art();
        sign_artifact_with(&mut a, &did, &ks).unwrap();

        // Signature envelope is x1:<did>:<hex>.
        let sig = a.signature.as_deref().unwrap();
        assert!(sig.starts_with("x1:"));
        assert!(sig.contains(did.as_str()));

        // Full verify round-trip.
        assert!(verify_artifact_with(&a, &ks).unwrap());
    }

    #[test]
    fn tampering_breaks_did_bound_signature() {
        use aaf_identity::{InMemoryKeystore, Keystore};
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"producing-agent");
        let mut a = art();
        sign_artifact_with(&mut a, &did, &ks).unwrap();
        a.content = serde_json::json!({"k": "tampered"});
        assert!(!verify_artifact_with(&a, &ks).unwrap());
    }

    #[test]
    fn wrong_key_fails_verification() {
        use aaf_identity::{InMemoryKeystore, Keystore};
        let producer = InMemoryKeystore::new();
        let attacker = InMemoryKeystore::new();
        let did = producer.generate(b"producing-agent");
        let mut a = art();
        sign_artifact_with(&mut a, &did, &producer).unwrap();
        // Verifier only knows the attacker's keystore — can't see the
        // producer's DID at all.
        assert!(matches!(
            verify_artifact_with(&a, &attacker),
            Err(aaf_identity::IdentityError::UnknownDid(_))
        ));
    }

    #[test]
    fn legacy_v0_signatures_are_not_wave2_verifiable() {
        use aaf_identity::InMemoryKeystore;
        let mut a = art();
        sign_artifact(&mut a, "legacy-agent");
        let ks = InMemoryKeystore::new();
        assert!(!verify_artifact_with(&a, &ks).unwrap());
    }

    #[test]
    fn unsigned_artifact_verifies_false() {
        use aaf_identity::InMemoryKeystore;
        let a = art();
        let ks = InMemoryKeystore::new();
        assert!(!verify_artifact_with(&a, &ks).unwrap());
    }
}
