//! On-the-wire identity shapes (Wave 2 Enhancement X1).
//!
//! These are the *lite* serde shapes that the contracts crate owns so
//! every other crate can reference agent identities, attestation
//! levels, and capability tokens without depending on `aaf-identity`.
//!
//! The types in this module intentionally mirror the shapes in
//! `aaf-identity` field-for-field:
//!
//! | Wire shape (this crate) | Rich shape (aaf-identity) |
//! |---|---|
//! | `AgentDidRef` | `aaf_identity::did::AgentDid` |
//! | `AttestationLevelRef` | `aaf_identity::attestation::AttestationLevel` |
//! | `TokenClaimsLite` | `aaf_identity::delegation::TokenClaims` |
//! | `CapabilityTokenLite` | `aaf_identity::delegation::CapabilityToken` |
//!
//! `aaf-identity` provides `From` impls in both directions so the
//! runtime can consume the rich types while the registry / handoff
//! surface speak wire shapes.

use crate::ids::CapabilityId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

/// Wire-format DID. See `aaf_identity::did::AgentDid` for the rich
/// type with constructors and format guards.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AgentDidRef(pub String);

impl AgentDidRef {
    /// Wrap a raw DID string. Callers that want format-checked DIDs
    /// should build them in `aaf-identity` and convert with `.into()`.
    pub fn new(s: impl Into<String>) -> Self {
        Self(s.into())
    }

    /// Borrow the inner string.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for AgentDidRef {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl From<String> for AgentDidRef {
    fn from(s: String) -> Self {
        Self(s)
    }
}

impl From<&str> for AgentDidRef {
    fn from(s: &str) -> Self {
        Self(s.to_string())
    }
}

/// Wire-format attestation level. Mirrors
/// `aaf_identity::attestation::AttestationLevel`.
///
/// Ordered from weakest to strongest so the runtime can compare
/// required vs presented with `>=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttestationLevelRef {
    /// No third-party attestation.
    Unattested,
    /// Passed automated eval suites.
    AutoVerified,
    /// Reviewed by a human operator.
    HumanReviewed,
    /// Formally certified by a recognised authority.
    Certified,
}

/// Wire-format claims payload inside a capability token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenClaimsLite {
    /// Issuing agent.
    pub issuer: AgentDidRef,
    /// Receiving agent.
    pub subject: AgentDidRef,
    /// Capability ids the token authorises.
    pub scope: Vec<CapabilityId>,
    /// Delegation depth remaining.
    pub depth_remaining: u8,
    /// Earliest valid time.
    pub not_before: DateTime<Utc>,
    /// Hard deadline.
    pub expires_at: DateTime<Utc>,
    /// Unique token id (used for revocation + audit).
    pub jti: String,
}

/// Wire-format capability token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityTokenLite {
    /// Claims payload.
    pub claims: TokenClaimsLite,
    /// Detached signature (hex).
    pub signature: String,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attestation_level_ordering_is_monotonic() {
        assert!(AttestationLevelRef::Unattested < AttestationLevelRef::AutoVerified);
        assert!(AttestationLevelRef::AutoVerified < AttestationLevelRef::HumanReviewed);
        assert!(AttestationLevelRef::HumanReviewed < AttestationLevelRef::Certified);
    }

    #[test]
    fn did_ref_round_trips_via_serde() {
        let d = AgentDidRef::from("did:aaf:abc123");
        let j = serde_json::to_string(&d).unwrap();
        let back: AgentDidRef = serde_json::from_str(&j).unwrap();
        assert_eq!(back, d);
    }

    #[test]
    fn capability_token_lite_round_trips_via_serde() {
        let t = CapabilityTokenLite {
            claims: TokenClaimsLite {
                issuer: AgentDidRef::from("did:aaf:issuer"),
                subject: AgentDidRef::from("did:aaf:subject"),
                scope: vec![CapabilityId::from("cap-x")],
                depth_remaining: 2,
                not_before: Utc::now(),
                expires_at: Utc::now() + chrono::Duration::minutes(5),
                jti: "jti-1".into(),
            },
            signature: "deadbeef".into(),
        };
        let j = serde_json::to_string(&t).unwrap();
        let back: CapabilityTokenLite = serde_json::from_str(&j).unwrap();
        assert_eq!(back, t);
    }
}
