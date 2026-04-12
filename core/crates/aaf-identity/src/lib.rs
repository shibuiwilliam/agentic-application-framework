//! AAF Agent Identity, Provenance & Supply Chain (Enhancement X1 Slice A).
//!
//! This crate gives every agent a cryptographically verifiable
//! identity. An agent is no longer a display string: it is a
//! [`did::AgentDid`] (a public-key thumbprint), governed by a signed
//! [`manifest::AgentManifest`], accompanied by a hash-based
//! [`sbom::AgentSbom`] bill of materials, and represented on the wire
//! by short-lived [`delegation::CapabilityToken`]s that the runtime
//! verifies at every hop.
//!
//! Rules enforced here (see `CLAUDE.md` rules 22-24):
//!
//! | Rule | Where |
//! |---|---|
//! | 22 Identity is cryptographic, not nominal | [`did::AgentDid`] is derived from a verifying key; never constructed from a bare string by callers. |
//! | 23 Every deployed agent has a signed manifest | [`manifest::AgentManifest::build`] is the only constructor — it signs at the end and the signature is verified on load. |
//! | 24 Provenance is a bill of materials | [`sbom::AgentSbom`] enumerates every input (model, prompts, tools, ontology refs, eval suites) with content hashes. |
//!
//! # Signature backend
//!
//! Slice A ships a deterministic HMAC-SHA256 backend behind
//! [`keystore::Signer`] / [`keystore::Verifier`]. This is a
//! *functional* identity layer that exercises the entire contract
//! surface (sign, verify, manifest tamper detection, token expiry,
//! revocation short-circuit). Slice B swaps in Ed25519 by
//! re-implementing the two traits for a new keystore; every call site
//! in this crate (and any downstream consumer) stays unchanged.
//!
//! Why HMAC in Slice A: the `ed25519-dalek` versions that compile on
//! the workspace's pinned Rust 1.70 toolchain drag in
//! `curve25519-dalek` tree that has historically been the top cause
//! of the workspace failing to build. Shipping a fake signer that
//! lets the tree stay green is strictly better than shipping real
//! Ed25519 that breaks the build; the contract surface is identical
//! either way.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod attestation;
pub mod delegation;
pub mod did;
pub mod error;
pub mod keystore;
pub mod manifest;
pub mod prelude;
pub mod revocation;
pub mod sbom;

pub use attestation::{Attestation, AttestationLevel};
pub use delegation::{CapabilityToken, TokenClaims};
pub use did::AgentDid;
pub use error::IdentityError;
pub use keystore::{InMemoryKeystore, KeyMaterial, Keystore, Signer, Verifier};
pub use manifest::{AgentManifest, ManifestBuilder, ModelPin, ToolBinding};
pub use revocation::{
    InMemoryRevocationRegistry, RevocationEntry, RevocationKind, RevocationRegistry,
};
pub use sbom::{AgentSbom, SbomEntry, SbomEntryKind};
