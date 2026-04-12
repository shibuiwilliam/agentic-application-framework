//! Identity crate errors.

use thiserror::Error;

/// Errors raised by the identity subsystem.
#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum IdentityError {
    /// Signature verification failed — either the signer is wrong,
    /// the content was tampered with, or the key material is wrong.
    #[error("signature verification failed")]
    InvalidSignature,

    /// A manifest's declared `source_hash` does not match the hash
    /// the runtime computed over the running code. Rule 23.
    #[error(
        "manifest drift: declared source hash {declared} does not match runtime hash {actual}"
    )]
    ManifestDrift {
        /// Declared hash.
        declared: String,
        /// Hash computed over the running code.
        actual: String,
    },

    /// A capability token was presented with `not_before` in the
    /// future or `expires_at` in the past.
    #[error("token expired or not yet valid")]
    TokenNotInEffect,

    /// A capability token's `depth_remaining` dropped below zero
    /// during delegation.
    #[error("token delegation depth exhausted")]
    DepthExhausted,

    /// A token's `scope` does not grant the requested capability.
    #[error("token does not grant capability `{0}`")]
    NotInScope(String),

    /// A DID is listed in the revocation registry. Rule 22.
    #[error("agent {0} is revoked")]
    Revoked(String),

    /// A key could not be found in the keystore.
    #[error("unknown DID {0}")]
    UnknownDid(String),

    /// JSON / YAML serialisation failed.
    #[error("serialisation: {0}")]
    Serialisation(String),
}
