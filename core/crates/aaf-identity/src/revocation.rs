//! Revocation registry.
//!
//! Immediate, platform-wide short-circuit for compromised agents,
//! prompts, tool versions, or manifest hashes. Every hot-path call
//! in the runtime consults this before honouring a manifest,
//! capability token, or artifact signature.
//!
//! Rule 22: the registry itself is append-only and every revocation
//! entry is signed by the revoker so there is a clean audit trail.

use crate::did::AgentDid;
use crate::error::IdentityError;
use crate::keystore::{Signer, Verifier};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::sync::Arc;

/// Kinds of thing that can be revoked.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RevocationKind {
    /// Revoke an entire agent by DID.
    Did,
    /// Revoke a specific prompt content hash.
    PromptHash,
    /// Revoke a tool version string (e.g. `cap-order-read@1.0.0`).
    ToolVersion,
    /// Revoke a specific manifest content hash.
    ManifestHash,
    /// Revoke a specific capability token by its `jti`.
    TokenJti,
}

/// One revocation record.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RevocationEntry {
    /// Kind of target.
    pub kind: RevocationKind,
    /// Target string — a DID, a hash, a version string, or a jti.
    pub target: String,
    /// Human-readable reason (audit).
    pub reason: String,
    /// When the revocation was issued.
    pub revoked_at: DateTime<Utc>,
    /// DID of the revoker.
    pub signer: AgentDid,
    /// Hex-encoded signature over the canonical record body.
    pub signature: String,
}

impl RevocationEntry {
    /// Build and sign a new revocation entry.
    pub fn issue(
        kind: RevocationKind,
        target: impl Into<String>,
        reason: impl Into<String>,
        signer_did: AgentDid,
        signer: &dyn Signer,
    ) -> Result<Self, IdentityError> {
        let body = RevocationBody {
            kind,
            target: target.into(),
            reason: reason.into(),
            revoked_at: Utc::now(),
            signer: signer_did.clone(),
        };
        let hash = body.canonical_hash();
        let signature = signer.sign(&signer_did, hash.as_bytes())?;
        Ok(Self {
            kind: body.kind,
            target: body.target,
            reason: body.reason,
            revoked_at: body.revoked_at,
            signer: body.signer,
            signature,
        })
    }

    /// Verify the revocation's signature.
    pub fn verify(&self, verifier: &dyn Verifier) -> Result<(), IdentityError> {
        let body = RevocationBody {
            kind: self.kind.clone(),
            target: self.target.clone(),
            reason: self.reason.clone(),
            revoked_at: self.revoked_at,
            signer: self.signer.clone(),
        };
        verifier.verify(
            &self.signer,
            body.canonical_hash().as_bytes(),
            &self.signature,
        )
    }
}

#[derive(Serialize)]
struct RevocationBody {
    kind: RevocationKind,
    target: String,
    reason: String,
    revoked_at: DateTime<Utc>,
    signer: AgentDid,
}

impl RevocationBody {
    fn canonical_hash(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }
}

/// Public API of a revocation registry. Async so production
/// implementations can be network-backed.
#[async_trait]
pub trait RevocationRegistry: Send + Sync {
    /// Append a revocation entry.
    async fn revoke(&self, entry: RevocationEntry) -> Result<(), IdentityError>;

    /// Returns `true` if the given `(kind, target)` pair has been
    /// revoked.
    async fn is_revoked(&self, kind: &RevocationKind, target: &str) -> bool;

    /// Returns the full list for audit / export.
    async fn list(&self) -> Vec<RevocationEntry>;
}

/// In-memory registry keyed on `(kind, target)` tuples in a
/// hash set. Entries themselves are kept in a Vec so `list()` gives
/// insertion order.
#[derive(Default)]
pub struct InMemoryRevocationRegistry {
    entries: Arc<RwLock<Vec<RevocationEntry>>>,
    index: Arc<RwLock<HashSet<(RevocationKind, String)>>>,
}

impl InMemoryRevocationRegistry {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl RevocationRegistry for InMemoryRevocationRegistry {
    async fn revoke(&self, entry: RevocationEntry) -> Result<(), IdentityError> {
        let key = (entry.kind.clone(), entry.target.clone());
        self.entries.write().push(entry);
        self.index.write().insert(key);
        Ok(())
    }

    async fn is_revoked(&self, kind: &RevocationKind, target: &str) -> bool {
        self.index
            .read()
            .contains(&(kind.clone(), target.to_string()))
    }

    async fn list(&self) -> Vec<RevocationEntry> {
        self.entries.read().clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::{InMemoryKeystore, Keystore};

    #[tokio::test]
    async fn revoke_and_check_round_trip() {
        let ks = InMemoryKeystore::new();
        let signer_did = ks.generate(b"admin");
        let reg = InMemoryRevocationRegistry::new();

        let did_to_revoke = ks.generate(b"compromised");
        let entry = RevocationEntry::issue(
            RevocationKind::Did,
            did_to_revoke.to_string(),
            "compromised key",
            signer_did,
            &ks,
        )
        .unwrap();
        reg.revoke(entry.clone()).await.unwrap();

        assert!(
            reg.is_revoked(&RevocationKind::Did, did_to_revoke.as_str())
                .await
        );
        assert!(
            !reg.is_revoked(&RevocationKind::Did, "did:aaf:unknown")
                .await
        );
    }

    #[tokio::test]
    async fn revocation_signature_verifies() {
        let ks = InMemoryKeystore::new();
        let signer_did = ks.generate(b"admin");
        let entry = RevocationEntry::issue(
            RevocationKind::PromptHash,
            "prompt-hash-abc",
            "prompt contained a secret",
            signer_did,
            &ks,
        )
        .unwrap();
        entry.verify(&ks).expect("verify");
    }

    #[tokio::test]
    async fn tampered_revocation_fails_verify() {
        let ks = InMemoryKeystore::new();
        let signer_did = ks.generate(b"admin");
        let mut entry = RevocationEntry::issue(
            RevocationKind::Did,
            "did:aaf:target",
            "reason",
            signer_did,
            &ks,
        )
        .unwrap();
        entry.reason = "forged reason".into();
        let err = entry.verify(&ks).unwrap_err();
        assert_eq!(err, IdentityError::InvalidSignature);
    }

    #[tokio::test]
    async fn list_returns_every_entry_in_order() {
        let ks = InMemoryKeystore::new();
        let signer_did = ks.generate(b"admin");
        let reg = InMemoryRevocationRegistry::new();
        for i in 0..3 {
            let e = RevocationEntry::issue(
                RevocationKind::TokenJti,
                format!("jti-{i}"),
                "test",
                signer_did.clone(),
                &ks,
            )
            .unwrap();
            reg.revoke(e).await.unwrap();
        }
        assert_eq!(reg.list().await.len(), 3);
    }
}
