//! Cross-cell co-signed capability tokens (Wave 2 X1 Slice C).
//!
//! When a capability is delegated across a cell boundary, a single
//! signature from the issuing cell is not enough. The receiving
//! cell also needs to have accepted the token — because by accepting
//! it the receiving cell is binding the delegation to its own policy
//! pack and audit trail.
//!
//! A [`CoSignedToken`] therefore holds the original
//! `aaf_identity::CapabilityToken` plus a second, independent
//! signature produced by the receiving cell's DID over the same
//! canonical hash. [`verify_cosigned`] enforces that **both**
//! signatures are valid against their respective cell DIDs before
//! the runtime will honour the token.
//!
//! This is the cryptographic complement to the data-boundary
//! enforcement already in `Router::enforce_outbound_entity` — one
//! prevents the wrong *data* from crossing a cell, the other
//! prevents the wrong *authority* from crossing it.

use aaf_identity::{AgentDid, CapabilityToken, IdentityError, Signer, Verifier};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

/// Errors raised when building or verifying a co-signed token.
#[derive(Debug, Error)]
pub enum CoSignError {
    /// Either signature failed verification.
    #[error("co-signature invalid: {which}")]
    InvalidSignature {
        /// Which signature: `"issuer"` or `"receiver"`.
        which: &'static str,
    },

    /// Underlying identity error (e.g. unknown DID in the keystore).
    #[error("identity: {0}")]
    Identity(#[from] IdentityError),

    /// The inner `CapabilityToken` was tampered with after co-signing
    /// — its signature no longer verifies.
    #[error("inner token signature invalid")]
    InnerTampered,
}

/// Canonical bytes that both signatures sign over.
///
/// Includes the issuing cell DID, the receiving cell DID, and the
/// inner token's signature (which is itself a hash over the token
/// claims, so any tamper propagates here).
fn canonical_bytes(
    token: &CapabilityToken,
    issuer_cell: &AgentDid,
    receiver_cell: &AgentDid,
) -> Vec<u8> {
    let mut hasher = Sha256::new();
    hasher.update(b"aaf:cosign:v1:");
    hasher.update(issuer_cell.as_str().as_bytes());
    hasher.update(b"::");
    hasher.update(receiver_cell.as_str().as_bytes());
    hasher.update(b"::");
    hasher.update(token.signature.as_bytes());
    hasher.finalize().to_vec()
}

/// A [`CapabilityToken`] plus both cells' signatures over its body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CoSignedToken {
    /// Inner capability token (carries its own issuer→subject signature).
    pub token: CapabilityToken,
    /// DID of the issuing cell (distinct from the token's agent
    /// `issuer` — can be the same cell if a single cell operates
    /// both agents).
    pub issuer_cell: AgentDid,
    /// DID of the receiving cell.
    pub receiver_cell: AgentDid,
    /// Issuer-cell signature over `canonical_bytes`.
    pub issuer_cell_sig: String,
    /// Receiver-cell signature over `canonical_bytes`.
    pub receiver_cell_sig: String,
}

/// Build a co-signed token. `issuer_signer` signs on behalf of
/// `issuer_cell`, `receiver_signer` signs on behalf of
/// `receiver_cell`. Both signers can be backed by the same keystore
/// (tests) or different keystores (real cells).
pub fn cosign_token(
    token: CapabilityToken,
    issuer_cell: AgentDid,
    issuer_signer: &dyn Signer,
    receiver_cell: AgentDid,
    receiver_signer: &dyn Signer,
) -> Result<CoSignedToken, CoSignError> {
    let bytes = canonical_bytes(&token, &issuer_cell, &receiver_cell);
    let issuer_cell_sig = issuer_signer.sign(&issuer_cell, &bytes)?;
    let receiver_cell_sig = receiver_signer.sign(&receiver_cell, &bytes)?;
    Ok(CoSignedToken {
        token,
        issuer_cell,
        receiver_cell,
        issuer_cell_sig,
        receiver_cell_sig,
    })
}

/// Verify both cell signatures on a co-signed token, and verify the
/// inner capability token under the caller-supplied
/// `inner_verifier`. Returns `Ok(())` only if all three checks pass.
pub fn verify_cosigned(
    cosigned: &CoSignedToken,
    issuer_cell_verifier: &dyn Verifier,
    receiver_cell_verifier: &dyn Verifier,
    inner_verifier: &dyn Verifier,
    required: &aaf_contracts::CapabilityId,
) -> Result<(), CoSignError> {
    let bytes = canonical_bytes(
        &cosigned.token,
        &cosigned.issuer_cell,
        &cosigned.receiver_cell,
    );

    // Issuer-cell signature.
    issuer_cell_verifier
        .verify(&cosigned.issuer_cell, &bytes, &cosigned.issuer_cell_sig)
        .map_err(|_| CoSignError::InvalidSignature { which: "issuer" })?;

    // Receiver-cell signature.
    receiver_cell_verifier
        .verify(&cosigned.receiver_cell, &bytes, &cosigned.receiver_cell_sig)
        .map_err(|_| CoSignError::InvalidSignature { which: "receiver" })?;

    // Inner capability-token signature + validity window + scope.
    cosigned
        .token
        .verify(inner_verifier, required)
        .map_err(|e| match e {
            IdentityError::InvalidSignature => CoSignError::InnerTampered,
            other => CoSignError::Identity(other),
        })?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::CapabilityId;
    use aaf_identity::{InMemoryKeystore, Keystore};
    use chrono::Duration;

    fn setup() -> (
        InMemoryKeystore,
        AgentDid,
        AgentDid,
        AgentDid,
        AgentDid,
        CapabilityToken,
    ) {
        let ks = InMemoryKeystore::new();
        let agent_issuer = ks.generate(b"agent-issuer");
        let agent_subject = ks.generate(b"agent-subject");
        let issuer_cell = ks.generate(b"issuer-cell");
        let receiver_cell = ks.generate(b"receiver-cell");
        let token = CapabilityToken::quick(
            agent_issuer.clone(),
            agent_subject.clone(),
            vec![CapabilityId::from("cap-cross-cell")],
            2,
            Duration::minutes(5),
            "jti-cosign",
            &ks,
        )
        .unwrap();
        (
            ks,
            agent_issuer,
            agent_subject,
            issuer_cell,
            receiver_cell,
            token,
        )
    }

    #[test]
    fn cosign_then_verify_round_trip() {
        let (ks, _ai, _as_, ic, rc, token) = setup();
        let cosigned = cosign_token(token, ic, &ks, rc, &ks).unwrap();
        verify_cosigned(
            &cosigned,
            &ks,
            &ks,
            &ks,
            &CapabilityId::from("cap-cross-cell"),
        )
        .expect("cosigned token should verify");
    }

    #[test]
    fn tampering_with_issuer_cell_sig_is_detected() {
        let (ks, _ai, _as_, ic, rc, token) = setup();
        let mut cosigned = cosign_token(token, ic, &ks, rc, &ks).unwrap();
        cosigned.issuer_cell_sig.push_str("00");
        let err = verify_cosigned(
            &cosigned,
            &ks,
            &ks,
            &ks,
            &CapabilityId::from("cap-cross-cell"),
        )
        .unwrap_err();
        match err {
            CoSignError::InvalidSignature { which } => assert_eq!(which, "issuer"),
            other => panic!("expected issuer invalid, got {other:?}"),
        }
    }

    #[test]
    fn tampering_with_receiver_cell_sig_is_detected() {
        let (ks, _ai, _as_, ic, rc, token) = setup();
        let mut cosigned = cosign_token(token, ic, &ks, rc, &ks).unwrap();
        cosigned.receiver_cell_sig.replace_range(0..2, "ff");
        let err = verify_cosigned(
            &cosigned,
            &ks,
            &ks,
            &ks,
            &CapabilityId::from("cap-cross-cell"),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            CoSignError::InvalidSignature { which: "receiver" }
        ));
    }

    #[test]
    fn tampering_with_inner_token_fails_inner_verify() {
        let (ks, _ai, _as_, ic, rc, token) = setup();
        let mut cosigned = cosign_token(token, ic, &ks, rc, &ks).unwrap();
        cosigned
            .token
            .claims
            .scope
            .push(CapabilityId::from("cap-admin"));
        let err = verify_cosigned(
            &cosigned,
            &ks,
            &ks,
            &ks,
            &CapabilityId::from("cap-cross-cell"),
        )
        .unwrap_err();
        // Tampering the scope changes the inner token's canonical
        // hash, so its signature no longer verifies — that surfaces
        // first in our check order **at either the outer or inner
        // layer** because the outer canonical bytes include the
        // inner signature, so the outer signatures are stable but
        // the inner one fails. Either error type is acceptable as
        // long as tampering is caught.
        assert!(matches!(
            err,
            CoSignError::InnerTampered | CoSignError::InvalidSignature { .. }
        ));
    }

    #[test]
    fn out_of_scope_capability_is_rejected_by_inner_verify() {
        let (ks, _ai, _as_, ic, rc, token) = setup();
        let cosigned = cosign_token(token, ic, &ks, rc, &ks).unwrap();
        let err = verify_cosigned(
            &cosigned,
            &ks,
            &ks,
            &ks,
            &CapabilityId::from("cap-not-in-scope"),
        )
        .unwrap_err();
        match err {
            CoSignError::Identity(IdentityError::NotInScope(_)) => {}
            other => panic!("expected NotInScope, got {other:?}"),
        }
    }

    #[test]
    fn expired_token_is_rejected_by_inner_verify() {
        use chrono::Utc;
        let ks = InMemoryKeystore::new();
        let agent_issuer = ks.generate(b"ai");
        let agent_subject = ks.generate(b"as");
        let issuer_cell = ks.generate(b"ic");
        let receiver_cell = ks.generate(b"rc");
        let now = Utc::now();
        let claims = aaf_identity::TokenClaims {
            issuer: agent_issuer,
            subject: agent_subject,
            scope: vec![CapabilityId::from("cap-cross-cell")],
            depth_remaining: 2,
            not_before: now - Duration::hours(2),
            expires_at: now - Duration::hours(1),
            jti: "jti-expired".into(),
        };
        let token = CapabilityToken::issue(claims, &ks).unwrap();
        let cosigned = cosign_token(token, issuer_cell, &ks, receiver_cell, &ks).unwrap();
        let err = verify_cosigned(
            &cosigned,
            &ks,
            &ks,
            &ks,
            &CapabilityId::from("cap-cross-cell"),
        )
        .unwrap_err();
        assert!(matches!(
            err,
            CoSignError::Identity(IdentityError::TokenNotInEffect)
        ));
    }
}
