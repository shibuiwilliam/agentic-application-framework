//! Convenience prelude — `use aaf_identity::prelude::*;`

pub use crate::attestation::{Attestation, AttestationLevel};
pub use crate::delegation::{CapabilityToken, TokenClaims};
pub use crate::did::AgentDid;
pub use crate::error::IdentityError;
pub use crate::keystore::{InMemoryKeystore, KeyMaterial, Keystore, Signer, Verifier};
pub use crate::manifest::{AgentManifest, ManifestBuilder, ModelPin, ToolBinding};
pub use crate::revocation::{
    InMemoryRevocationRegistry, RevocationEntry, RevocationKind, RevocationRegistry,
};
pub use crate::sbom::{AgentSbom, SbomEntry, SbomEntryKind};
