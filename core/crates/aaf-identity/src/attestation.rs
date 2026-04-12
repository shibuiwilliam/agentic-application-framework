//! Attestation — a short statement by a signer that a particular
//! manifest has been vetted to a specific level.
//!
//! Attestation is *orthogonal* to the manifest signature itself.
//! The manifest signature proves "this manifest was produced by the
//! agent's own key". The attestation proves "*some trusted party*
//! reviewed this manifest and graded it as [`AttestationLevel::X`]".
//! The runtime can require a minimum attestation level before
//! serving a capability (`CapabilityContract.required_attestation_level`).

use crate::did::AgentDid;
use crate::error::IdentityError;
use crate::keystore::{Signer, Verifier};
use crate::manifest::AgentManifest;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Attestation levels, ordered from weakest to strongest.
///
/// `PartialOrd` / `Ord` derivations let the runtime compare a
/// required level against an actual level with `>=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationLevel {
    /// No attestation — only the manifest's own signature.
    Unattested,
    /// Passed automated eval suites.
    AutoVerified,
    /// Reviewed by a human operator.
    HumanReviewed,
    /// Formally certified by a recognised authority.
    Certified,
}

/// A signed attestation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attestation {
    /// Who issued the attestation.
    pub issuer: AgentDid,
    /// Whose manifest is attested.
    pub subject: AgentDid,
    /// Content hash of the manifest at the time of attestation.
    pub manifest_hash: String,
    /// Level granted.
    pub level: AttestationLevel,
    /// Human-readable rationale shown in reports.
    pub rationale: String,
    /// Issued at.
    pub issued_at: DateTime<Utc>,
    /// Optional expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Detached signature from `issuer`.
    pub signature: String,
}

impl Attestation {
    /// Issue a new attestation by signing `(subject, manifest_hash,
    /// level, rationale, issued_at, expires_at)` with `issuer`'s key.
    pub fn issue(
        issuer: AgentDid,
        subject: AgentDid,
        manifest: &AgentManifest,
        level: AttestationLevel,
        rationale: impl Into<String>,
        expires_at: Option<DateTime<Utc>>,
        signer: &dyn Signer,
    ) -> Result<Self, IdentityError> {
        let manifest_hash = manifest_content_hash(manifest);
        let now = Utc::now();
        let body = AttestationBody {
            issuer: issuer.clone(),
            subject,
            manifest_hash,
            level,
            rationale: rationale.into(),
            issued_at: now,
            expires_at,
        };
        let hash = body.canonical_hash();
        let signature = signer.sign(&issuer, hash.as_bytes())?;
        Ok(Self {
            issuer: body.issuer,
            subject: body.subject,
            manifest_hash: body.manifest_hash,
            level: body.level,
            rationale: body.rationale,
            issued_at: body.issued_at,
            expires_at: body.expires_at,
            signature,
        })
    }

    /// Verify the attestation's signature against `verifier`. Does
    /// *not* check expiry — callers handle that.
    pub fn verify(&self, verifier: &dyn Verifier) -> Result<(), IdentityError> {
        let body = AttestationBody {
            issuer: self.issuer.clone(),
            subject: self.subject.clone(),
            manifest_hash: self.manifest_hash.clone(),
            level: self.level,
            rationale: self.rationale.clone(),
            issued_at: self.issued_at,
            expires_at: self.expires_at,
        };
        let hash = body.canonical_hash();
        verifier.verify(&self.issuer, hash.as_bytes(), &self.signature)
    }

    /// Returns `true` if the attestation is still in effect.
    pub fn is_in_effect(&self, now: DateTime<Utc>) -> bool {
        match self.expires_at {
            Some(e) => now <= e,
            None => true,
        }
    }

    /// Returns `true` if this attestation grants at least `required`.
    pub fn grants(&self, required: AttestationLevel) -> bool {
        self.level >= required
    }
}

fn manifest_content_hash(manifest: &AgentManifest) -> String {
    use sha2::{Digest, Sha256};
    let bytes = serde_json::to_vec(manifest).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    hex::encode(hasher.finalize())
}

#[derive(Serialize)]
struct AttestationBody {
    issuer: AgentDid,
    subject: AgentDid,
    manifest_hash: String,
    level: AttestationLevel,
    rationale: String,
    issued_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

impl AttestationBody {
    fn canonical_hash(&self) -> String {
        use sha2::{Digest, Sha256};
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::{InMemoryKeystore, Keystore};
    use crate::manifest::ManifestBuilder;

    fn manifest(ks: &InMemoryKeystore, seed: &[u8]) -> (AgentDid, AgentManifest) {
        let did = ks.generate(seed);
        let m = ManifestBuilder::new(did.clone(), "a", "src-hash")
            .build(ks)
            .unwrap();
        (did, m)
    }

    #[test]
    fn levels_order_strongest_last() {
        assert!(AttestationLevel::Unattested < AttestationLevel::AutoVerified);
        assert!(AttestationLevel::AutoVerified < AttestationLevel::HumanReviewed);
        assert!(AttestationLevel::HumanReviewed < AttestationLevel::Certified);
    }

    #[test]
    fn issue_then_verify_round_trip() {
        let ks = InMemoryKeystore::new();
        let issuer = ks.generate(b"reviewer");
        let (subject, mani) = manifest(&ks, b"subject");
        let att = Attestation::issue(
            issuer,
            subject,
            &mani,
            AttestationLevel::HumanReviewed,
            "reviewed by ops",
            None,
            &ks,
        )
        .unwrap();
        att.verify(&ks).expect("verify");
    }

    #[test]
    fn tampered_rationale_invalidates_attestation() {
        let ks = InMemoryKeystore::new();
        let issuer = ks.generate(b"reviewer");
        let (subject, mani) = manifest(&ks, b"subject");
        let mut att = Attestation::issue(
            issuer,
            subject,
            &mani,
            AttestationLevel::AutoVerified,
            "ok",
            None,
            &ks,
        )
        .unwrap();
        att.rationale = "forged".into();
        let err = att.verify(&ks).unwrap_err();
        assert_eq!(err, IdentityError::InvalidSignature);
    }

    #[test]
    fn grants_is_monotonic() {
        let ks = InMemoryKeystore::new();
        let issuer = ks.generate(b"r");
        let (subject, mani) = manifest(&ks, b"s");
        let att = Attestation::issue(
            issuer,
            subject,
            &mani,
            AttestationLevel::HumanReviewed,
            "",
            None,
            &ks,
        )
        .unwrap();
        assert!(att.grants(AttestationLevel::Unattested));
        assert!(att.grants(AttestationLevel::AutoVerified));
        assert!(att.grants(AttestationLevel::HumanReviewed));
        assert!(!att.grants(AttestationLevel::Certified));
    }
}
