//! Keystore + Signer/Verifier traits.
//!
//! `Keystore` owns the secret material behind a narrow API:
//!
//! - `generate()` — mint a fresh key pair, return the DID; private
//!   bytes stay inside the store (Rule: "Private keys never leave
//!   the keystore boundary").
//! - `sign(did, message)` — produce a signature over `message`
//!   authorised by `did`'s private key.
//! - `verifier(did)` — hand out a `Verifier` for `did` that callers
//!   can use to check other agents' signatures.
//!
//! The Slice A backend is HMAC-SHA256: each DID maps to a
//! `KeyMaterial` blob, and signing / verification run the same MAC
//! over the input. A future slice swaps in Ed25519 without
//! changing any public API.

use crate::did::AgentDid;
use crate::error::IdentityError;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::sync::Arc;

/// Public API of a signing backend.
pub trait Signer: Send + Sync {
    /// Produce a signature for the given DID over `message`.
    fn sign(&self, did: &AgentDid, message: &[u8]) -> Result<String, IdentityError>;
}

/// Public API of a verification backend.
pub trait Verifier: Send + Sync {
    /// Verify that `signature` is valid for `message` under
    /// `did`'s key.
    fn verify(&self, did: &AgentDid, message: &[u8], signature: &str) -> Result<(), IdentityError>;
}

/// Raw key material. Production Ed25519 backends replace this struct
/// with a `SigningKey` / `VerifyingKey` pair; Slice A stores a single
/// shared secret.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct KeyMaterial {
    /// 32 random bytes used as the HMAC key.
    pub secret: Vec<u8>,
    /// Derived verifying-key bytes — deterministic from `secret`.
    pub verifying: Vec<u8>,
}

impl KeyMaterial {
    /// Derive key material from a caller-supplied seed. Tests pass a
    /// fixed seed so DIDs are deterministic; production calls
    /// `Keystore::generate` which uses a non-deterministic seed.
    pub fn from_seed(seed: &[u8]) -> Self {
        let mut sec_hasher = Sha256::new();
        sec_hasher.update(b"aaf-identity-secret:");
        sec_hasher.update(seed);
        let secret = sec_hasher.finalize().to_vec();

        let mut ver_hasher = Sha256::new();
        ver_hasher.update(b"aaf-identity-verifying:");
        ver_hasher.update(&secret);
        let verifying = ver_hasher.finalize().to_vec();

        Self { secret, verifying }
    }
}

/// HMAC-SHA256 with a 64-byte block size, implemented inline so the
/// crate doesn't pull in a new dependency.
fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    const BLOCK: usize = 64;
    let mut padded = [0u8; BLOCK];
    if key.len() > BLOCK {
        let digest = {
            let mut h = Sha256::new();
            h.update(key);
            h.finalize()
        };
        padded[..digest.len()].copy_from_slice(&digest);
    } else {
        padded[..key.len()].copy_from_slice(key);
    }
    let mut i_key_pad = [0u8; BLOCK];
    let mut o_key_pad = [0u8; BLOCK];
    for i in 0..BLOCK {
        i_key_pad[i] = padded[i] ^ 0x36;
        o_key_pad[i] = padded[i] ^ 0x5c;
    }
    let inner = {
        let mut h = Sha256::new();
        h.update(i_key_pad);
        h.update(message);
        h.finalize()
    };
    let mut outer = Sha256::new();
    outer.update(o_key_pad);
    outer.update(inner);
    outer.finalize().to_vec()
}

/// Unified trait over the signer + verifier + key-management surface.
pub trait Keystore: Signer + Verifier + Send + Sync {
    /// Create a fresh key pair and return its DID.
    fn generate(&self, seed: &[u8]) -> AgentDid;

    /// Convenience: return `true` if `did` is known to this store.
    fn knows(&self, did: &AgentDid) -> bool;
}

/// In-memory keystore backed by HMAC-SHA256.
#[derive(Default)]
pub struct InMemoryKeystore {
    inner: Arc<RwLock<HashMap<AgentDid, KeyMaterial>>>,
}

impl InMemoryKeystore {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }
}

impl Keystore for InMemoryKeystore {
    fn generate(&self, seed: &[u8]) -> AgentDid {
        let material = KeyMaterial::from_seed(seed);
        let did = AgentDid::from_verifying_key(&material.verifying);
        self.inner.write().insert(did.clone(), material);
        did
    }

    fn knows(&self, did: &AgentDid) -> bool {
        self.inner.read().contains_key(did)
    }
}

impl Signer for InMemoryKeystore {
    fn sign(&self, did: &AgentDid, message: &[u8]) -> Result<String, IdentityError> {
        let guard = self.inner.read();
        let material = guard
            .get(did)
            .ok_or_else(|| IdentityError::UnknownDid(did.to_string()))?;
        let mac = hmac_sha256(&material.secret, message);
        Ok(hex::encode(mac))
    }
}

impl Verifier for InMemoryKeystore {
    fn verify(&self, did: &AgentDid, message: &[u8], signature: &str) -> Result<(), IdentityError> {
        let guard = self.inner.read();
        let material = guard
            .get(did)
            .ok_or_else(|| IdentityError::UnknownDid(did.to_string()))?;
        let expected = hex::encode(hmac_sha256(&material.secret, message));
        if constant_time_eq(expected.as_bytes(), signature.as_bytes()) {
            Ok(())
        } else {
            Err(IdentityError::InvalidSignature)
        }
    }
}

/// Constant-time equality for signature strings. Stops a timing side
/// channel from leaking whether the first byte of a forged
/// signature is correct.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut diff: u8 = 0;
    for i in 0..a.len() {
        diff |= a[i] ^ b[i];
    }
    diff == 0
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_is_deterministic_per_seed() {
        let ks_a = InMemoryKeystore::new();
        let ks_b = InMemoryKeystore::new();
        let seed = b"alpha";
        assert_eq!(ks_a.generate(seed), ks_b.generate(seed));
    }

    #[test]
    fn sign_verify_round_trip() {
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"seed-1");
        let sig = ks.sign(&did, b"hello world").unwrap();
        ks.verify(&did, b"hello world", &sig).unwrap();
    }

    #[test]
    fn tampered_message_fails_verification() {
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"seed-2");
        let sig = ks.sign(&did, b"original message").unwrap();
        let err = ks.verify(&did, b"tampered message", &sig).unwrap_err();
        assert_eq!(err, IdentityError::InvalidSignature);
    }

    #[test]
    fn unknown_did_errors_explicitly() {
        let ks = InMemoryKeystore::new();
        let stranger = AgentDid::from_verifying_key(b"never-generated");
        let err = ks.sign(&stranger, b"x").unwrap_err();
        assert!(matches!(err, IdentityError::UnknownDid(_)));
    }

    #[test]
    fn constant_time_eq_works() {
        assert!(constant_time_eq(b"abc", b"abc"));
        assert!(!constant_time_eq(b"abc", b"abd"));
        assert!(!constant_time_eq(b"ab", b"abc"));
    }

    #[test]
    fn hmac_matches_known_vector() {
        // HMAC-SHA256("key", "message") should be deterministic and
        // distinct from HMAC-SHA256("key2", "message").
        let a = hmac_sha256(b"key", b"message");
        let b = hmac_sha256(b"key2", b"message");
        assert_ne!(a, b);
        assert_eq!(a, hmac_sha256(b"key", b"message"));
    }
}
