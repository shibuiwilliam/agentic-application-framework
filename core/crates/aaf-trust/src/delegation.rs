//! Delegation chain — `min(delegator, delegatee)` propagation plus
//! cryptographic capability-token verification (Wave 2 X1 Slice B).
//!
//! This module defends against two distinct trust-escalation threats:
//!
//! 1. **Behavioural** — a low-autonomy-level agent delegating to a
//!    higher-level agent to amplify its reach. Handled by the
//!    `effective_trust` / `require` pair from Wave 1.
//! 2. **Cryptographic** — a forged or tampered bearer token granting
//!    capabilities the issuer never actually sanctioned. Handled by
//!    `verify_token`, which consults an `aaf_identity::Verifier`,
//!    enforces the token's signature, validity window, and scope, and
//!    emits a typed `DelegationError::Token` on any failure.

use aaf_contracts::{AutonomyLevel, CapabilityId};
use aaf_identity::{CapabilityToken, IdentityError, Verifier};
use thiserror::Error;

/// Errors raised by the delegation logic.
#[derive(Debug, Error)]
pub enum DelegationError {
    /// The required autonomy level exceeded the effective trust.
    #[error("required {required:?} but effective {effective:?}")]
    InsufficientTrust {
        /// Level the operation requires.
        required: AutonomyLevel,
        /// Effective level after `min(delegator, delegatee)`.
        effective: AutonomyLevel,
    },

    /// Underlying capability token failed verification (Wave 2 X1).
    #[error("token rejected: {0}")]
    Token(IdentityError),
}

impl From<IdentityError> for DelegationError {
    fn from(e: IdentityError) -> Self {
        DelegationError::Token(e)
    }
}

/// Compute the effective trust for a delegation: `min(a, b)`.
pub fn effective_trust(delegator: AutonomyLevel, delegatee: AutonomyLevel) -> AutonomyLevel {
    AutonomyLevel::from_u8(delegator.as_u8().min(delegatee.as_u8()))
}

/// Verify that `effective` meets `required`.
pub fn require(required: AutonomyLevel, effective: AutonomyLevel) -> Result<(), DelegationError> {
    if effective.as_u8() >= required.as_u8() {
        Ok(())
    } else {
        Err(DelegationError::InsufficientTrust {
            required,
            effective,
        })
    }
}

/// Cryptographic dimension of delegation: verify a
/// [`CapabilityToken`] against a keystore-backed `Verifier` and a
/// required capability id. Delegates all of signature check, validity
/// window, and scope check to
/// [`aaf_identity::CapabilityToken::verify`], surfacing any failure as
/// [`DelegationError::Token`] for uniform error handling at the
/// trust-system level.
///
/// This is the Wave 2 X1 Slice B hook the runtime calls on every
/// handoff; Wave 1's behavioural `require` still applies on top for
/// defense-in-depth.
pub fn verify_token(
    token: &CapabilityToken,
    verifier: &dyn Verifier,
    required: &CapabilityId,
) -> Result<(), DelegationError> {
    token.verify(verifier, required)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_identity::{InMemoryKeystore, Keystore};
    use chrono::Duration;

    #[test]
    fn min_propagation_takes_lower_level() {
        assert_eq!(
            effective_trust(AutonomyLevel::Level5, AutonomyLevel::Level2),
            AutonomyLevel::Level2
        );
    }

    #[test]
    fn require_blocks_low_effective() {
        let err = require(AutonomyLevel::Level4, AutonomyLevel::Level2).unwrap_err();
        assert!(matches!(err, DelegationError::InsufficientTrust { .. }));
    }

    #[test]
    fn require_passes_when_sufficient() {
        require(AutonomyLevel::Level2, AutonomyLevel::Level3).unwrap();
    }

    // ── Wave 2 X1 Slice B — cryptographic token verification ──────────

    fn token_for(ks: &InMemoryKeystore, scope: Vec<&str>) -> CapabilityToken {
        let issuer = ks.generate(b"issuer-seed");
        let subject = ks.generate(b"subject-seed");
        CapabilityToken::quick(
            issuer,
            subject,
            scope.into_iter().map(CapabilityId::from).collect(),
            3,
            Duration::minutes(5),
            "jti-trust",
            ks,
        )
        .unwrap()
    }

    #[test]
    fn verify_token_accepts_in_scope_capability() {
        let ks = InMemoryKeystore::new();
        let token = token_for(&ks, vec!["cap-order-read"]);
        verify_token(&token, &ks, &CapabilityId::from("cap-order-read")).unwrap();
    }

    #[test]
    fn verify_token_rejects_out_of_scope_capability() {
        let ks = InMemoryKeystore::new();
        let token = token_for(&ks, vec!["cap-order-read"]);
        let err = verify_token(&token, &ks, &CapabilityId::from("cap-order-write")).unwrap_err();
        match err {
            DelegationError::Token(IdentityError::NotInScope(_)) => {}
            other => panic!("expected NotInScope, got {other:?}"),
        }
    }

    #[test]
    fn verify_token_rejects_tampered_signature() {
        let ks = InMemoryKeystore::new();
        let mut token = token_for(&ks, vec!["cap-x"]);
        token.claims.scope.push(CapabilityId::from("cap-admin"));
        let err = verify_token(&token, &ks, &CapabilityId::from("cap-x")).unwrap_err();
        match err {
            DelegationError::Token(IdentityError::InvalidSignature) => {}
            other => panic!("expected InvalidSignature, got {other:?}"),
        }
    }
}
