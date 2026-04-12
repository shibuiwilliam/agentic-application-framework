//! Capability tokens — signed bearer grants handed between agents.
//!
//! A `CapabilityToken` replaces Wave 1's naked `min(a, b)` integer
//! for delegation. The token is a signed payload that binds the
//! issuer, subject, exact capability scope, delegation depth,
//! validity window, and a unique `jti`. The runtime checks the
//! token at every handoff and at every step that inspects
//! `required_attestation_level`.

use crate::did::AgentDid;
use crate::error::IdentityError;
use crate::keystore::{Signer, Verifier};
use aaf_contracts::CapabilityId;
use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Claims inside a capability token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenClaims {
    /// Issuer (delegator).
    pub issuer: AgentDid,
    /// Subject (delegatee).
    pub subject: AgentDid,
    /// Exact capabilities this token grants.
    pub scope: Vec<CapabilityId>,
    /// Remaining delegation depth. Decrements on every re-delegation.
    pub depth_remaining: u8,
    /// Earliest valid time.
    pub not_before: DateTime<Utc>,
    /// Expiry — absolute deadline.
    pub expires_at: DateTime<Utc>,
    /// Unique token id, used for revocation + audit.
    pub jti: String,
}

impl TokenClaims {
    fn canonical_hash(&self) -> String {
        let bytes = serde_json::to_vec(self).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }
}

/// A signed capability token.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityToken {
    /// Claims payload.
    pub claims: TokenClaims,
    /// Hex-encoded signature over the canonical hash of `claims`.
    pub signature: String,
}

impl CapabilityToken {
    /// Issue a new token by signing the supplied claims.
    pub fn issue(claims: TokenClaims, signer: &dyn Signer) -> Result<Self, IdentityError> {
        let hash = claims.canonical_hash();
        let signature = signer.sign(&claims.issuer, hash.as_bytes())?;
        Ok(Self { claims, signature })
    }

    /// Convenience constructor for short-lived tokens.
    pub fn quick(
        issuer: AgentDid,
        subject: AgentDid,
        scope: Vec<CapabilityId>,
        depth_remaining: u8,
        ttl: Duration,
        jti: impl Into<String>,
        signer: &dyn Signer,
    ) -> Result<Self, IdentityError> {
        let now = Utc::now();
        Self::issue(
            TokenClaims {
                issuer,
                subject,
                scope,
                depth_remaining,
                not_before: now,
                expires_at: now + ttl,
                jti: jti.into(),
            },
            signer,
        )
    }

    /// Verify the signature, the validity window, and that
    /// `required_capability` falls inside the token's scope.
    pub fn verify(
        &self,
        verifier: &dyn Verifier,
        required_capability: &CapabilityId,
    ) -> Result<(), IdentityError> {
        // 1. Cryptographic signature.
        let hash = self.claims.canonical_hash();
        verifier.verify(&self.claims.issuer, hash.as_bytes(), &self.signature)?;

        // 2. Validity window.
        let now = Utc::now();
        if now < self.claims.not_before || now > self.claims.expires_at {
            return Err(IdentityError::TokenNotInEffect);
        }

        // 3. Scope check.
        if !self.claims.scope.iter().any(|c| c == required_capability) {
            return Err(IdentityError::NotInScope(required_capability.to_string()));
        }

        Ok(())
    }

    /// Decrement the delegation depth. Returns `DepthExhausted` if
    /// the token cannot be re-delegated further.
    pub fn step_down(&self) -> Result<TokenClaims, IdentityError> {
        if self.claims.depth_remaining == 0 {
            return Err(IdentityError::DepthExhausted);
        }
        let mut next = self.claims.clone();
        next.depth_remaining -= 1;
        Ok(next)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::{InMemoryKeystore, Keystore};

    fn setup() -> (InMemoryKeystore, AgentDid, AgentDid) {
        let ks = InMemoryKeystore::new();
        let issuer = ks.generate(b"issuer-seed");
        let subject = ks.generate(b"subject-seed");
        (ks, issuer, subject)
    }

    #[test]
    fn issue_and_verify_round_trip() {
        let (ks, issuer, subject) = setup();
        let token = CapabilityToken::quick(
            issuer,
            subject,
            vec![CapabilityId::from("cap-order-read")],
            3,
            Duration::minutes(5),
            "jti-1",
            &ks,
        )
        .unwrap();
        token
            .verify(&ks, &CapabilityId::from("cap-order-read"))
            .unwrap();
    }

    #[test]
    fn out_of_scope_capability_is_rejected() {
        let (ks, issuer, subject) = setup();
        let token = CapabilityToken::quick(
            issuer,
            subject,
            vec![CapabilityId::from("cap-order-read")],
            3,
            Duration::minutes(5),
            "jti-2",
            &ks,
        )
        .unwrap();
        let err = token
            .verify(&ks, &CapabilityId::from("cap-order-write"))
            .unwrap_err();
        assert!(matches!(err, IdentityError::NotInScope(_)));
    }

    #[test]
    fn expired_token_fails_verify() {
        let (ks, issuer, subject) = setup();
        let now = Utc::now();
        let claims = TokenClaims {
            issuer,
            subject,
            scope: vec![CapabilityId::from("cap-x")],
            depth_remaining: 3,
            not_before: now - Duration::hours(2),
            expires_at: now - Duration::hours(1),
            jti: "jti-expired".into(),
        };
        let token = CapabilityToken::issue(claims, &ks).unwrap();
        let err = token.verify(&ks, &CapabilityId::from("cap-x")).unwrap_err();
        assert_eq!(err, IdentityError::TokenNotInEffect);
    }

    #[test]
    fn tampered_scope_invalidates_signature() {
        let (ks, issuer, subject) = setup();
        let mut token = CapabilityToken::quick(
            issuer,
            subject,
            vec![CapabilityId::from("cap-read")],
            3,
            Duration::minutes(5),
            "jti-x",
            &ks,
        )
        .unwrap();
        // Attacker tries to upgrade scope.
        token.claims.scope.push(CapabilityId::from("cap-admin"));
        let err = token
            .verify(&ks, &CapabilityId::from("cap-read"))
            .unwrap_err();
        assert_eq!(err, IdentityError::InvalidSignature);
    }

    #[test]
    fn step_down_decrements_depth() {
        let (ks, issuer, subject) = setup();
        let token = CapabilityToken::quick(
            issuer,
            subject,
            vec![CapabilityId::from("cap-x")],
            2,
            Duration::minutes(5),
            "jti-d",
            &ks,
        )
        .unwrap();
        let next = token.step_down().unwrap();
        assert_eq!(next.depth_remaining, 1);
    }

    #[test]
    fn step_down_from_zero_errors() {
        let (ks, issuer, subject) = setup();
        let token = CapabilityToken::quick(
            issuer,
            subject,
            vec![CapabilityId::from("cap-x")],
            0,
            Duration::minutes(5),
            "jti-d0",
            &ks,
        )
        .unwrap();
        assert!(matches!(
            token.step_down(),
            Err(IdentityError::DepthExhausted)
        ));
    }
}
