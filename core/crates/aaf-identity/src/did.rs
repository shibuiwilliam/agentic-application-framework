//! Agent DID — a W3C-style decentralised identifier derived from a
//! public-key thumbprint.
//!
//! The string form is `did:aaf:<hex-sha256-of-verifying-key>`. DIDs
//! are opaque handles — callers compare them for equality but never
//! parse the inner hex outside this module.

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fmt;

/// Cryptographic agent identifier.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentDid(String);

impl AgentDid {
    /// Construct a DID from a verifying-key byte slice.
    pub fn from_verifying_key(key_bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(key_bytes);
        let digest = hasher.finalize();
        // 24 hex chars = 96 bits of entropy — plenty for an identifier
        // while staying short enough to be readable in logs.
        let thumbprint = &hex::encode(digest)[..24];
        Self(format!("did:aaf:{thumbprint}"))
    }

    /// Wrap a pre-computed DID string. Slice B swaps the keystore
    /// to a real Ed25519 backend; Slice A tests use this to build
    /// known-good DIDs.
    pub fn from_raw(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the DID as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns `true` if the string looks like an AAF DID. Used by
    /// the manifest / token verifiers to refuse bare names.
    pub fn is_well_formed(&self) -> bool {
        self.0.starts_with("did:aaf:") && self.0.len() > "did:aaf:".len() + 8
    }
}

impl fmt::Display for AgentDid {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for AgentDid {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AgentDid {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn did_is_deterministic_per_key() {
        let key = b"verifying-key-bytes";
        let a = AgentDid::from_verifying_key(key);
        let b = AgentDid::from_verifying_key(key);
        assert_eq!(a, b);
    }

    #[test]
    fn different_keys_produce_different_dids() {
        let a = AgentDid::from_verifying_key(b"key-one");
        let b = AgentDid::from_verifying_key(b"key-two");
        assert_ne!(a, b);
    }

    #[test]
    fn well_formed_dids_pass_guard() {
        let d = AgentDid::from_verifying_key(b"some-key");
        assert!(d.is_well_formed());
        assert!(d.as_str().starts_with("did:aaf:"));
    }

    #[test]
    fn bare_strings_are_not_well_formed() {
        let d = AgentDid::from_raw("agent-1");
        assert!(!d.is_well_formed());
    }
}
