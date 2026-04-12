//! Agent Software Bill of Materials.
//!
//! An `AgentSbom` enumerates every input that shaped the agent:
//! model versions, prompt content hashes, tool versions, ontology
//! entity versions, eval suite refs, and any additional artifacts.
//! Each entry carries a content hash so tampering anywhere in the
//! supply chain is detectable.
//!
//! - **Slice A** shipped the canonical JSON shape used by the
//!   framework internally.
//! - **Slice C** adds `export::{to_spdx_json, to_cyclonedx_json}`
//!   for interop with standards-based supply-chain tooling.

pub mod export;

use crate::did::AgentDid;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Classes of SBOM entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SbomEntryKind {
    /// LLM model version.
    Model,
    /// System / user / tool prompt.
    Prompt,
    /// External tool or capability binding.
    Tool,
    /// Ontology entity version.
    Ontology,
    /// Eval suite reference.
    EvalSuite,
    /// Training-data reference.
    TrainingData,
    /// Anything else, free-form.
    Other,
}

/// One line in an agent's SBOM.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SbomEntry {
    /// Category.
    pub kind: SbomEntryKind,
    /// Human-readable identifier.
    pub name: String,
    /// Version string (semver if applicable).
    pub version: String,
    /// SHA-256 of the referenced content, hex-encoded.
    pub content_hash: String,
    /// Optional upstream URL / registry coordinates.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
}

impl SbomEntry {
    /// Build an entry by hashing the supplied bytes.
    pub fn from_bytes(
        kind: SbomEntryKind,
        name: impl Into<String>,
        version: impl Into<String>,
        bytes: &[u8],
    ) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(bytes);
        Self {
            kind,
            name: name.into(),
            version: version.into(),
            content_hash: hex::encode(hasher.finalize()),
            locator: None,
        }
    }

    /// Attach a locator (URL / registry coordinates).
    pub fn with_locator(mut self, locator: impl Into<String>) -> Self {
        self.locator = Some(locator.into());
        self
    }
}

/// Full agent Software Bill of Materials.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentSbom {
    /// Agent this SBOM describes.
    pub did: AgentDid,
    /// When the SBOM was generated.
    pub generated_at: DateTime<Utc>,
    /// Entries.
    pub entries: Vec<SbomEntry>,
}

impl AgentSbom {
    /// Construct empty.
    pub fn new(did: AgentDid) -> Self {
        Self {
            did,
            generated_at: Utc::now(),
            entries: vec![],
        }
    }

    /// Append.
    pub fn push(&mut self, entry: SbomEntry) {
        self.entries.push(entry);
    }

    /// Count entries of a particular kind.
    pub fn count(&self, kind: SbomEntryKind) -> usize {
        self.entries.iter().filter(|e| e.kind == kind).count()
    }

    /// Serialise to pretty JSON (the canonical wire format until
    /// Slice C adds SPDX / CycloneDX export).
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }

    /// Compute a stable content hash over the entire SBOM. Used by
    /// `Attestation` to bind an attestation record to a specific
    /// SBOM snapshot.
    pub fn content_hash(&self) -> String {
        let bytes = serde_json::to_vec(&self.entries).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_bytes_is_stable() {
        let a = SbomEntry::from_bytes(SbomEntryKind::Prompt, "system", "v1", b"prompt text");
        let b = SbomEntry::from_bytes(SbomEntryKind::Prompt, "system", "v1", b"prompt text");
        assert_eq!(a.content_hash, b.content_hash);
    }

    #[test]
    fn count_filters_by_kind() {
        let mut s = AgentSbom::new(AgentDid::from_raw("did:aaf:test"));
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Prompt,
            "p1",
            "v1",
            b"a",
        ));
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Prompt,
            "p2",
            "v1",
            b"b",
        ));
        s.push(SbomEntry::from_bytes(SbomEntryKind::Tool, "t1", "v1", b"c"));
        assert_eq!(s.count(SbomEntryKind::Prompt), 2);
        assert_eq!(s.count(SbomEntryKind::Tool), 1);
        assert_eq!(s.count(SbomEntryKind::Model), 0);
    }

    #[test]
    fn content_hash_changes_when_entries_change() {
        let mut s = AgentSbom::new(AgentDid::from_raw("did:aaf:test"));
        let h1 = s.content_hash();
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Prompt,
            "p1",
            "v1",
            b"a",
        ));
        let h2 = s.content_hash();
        assert_ne!(h1, h2);
    }

    #[test]
    fn to_json_round_trips_via_serde() {
        let mut s = AgentSbom::new(AgentDid::from_raw("did:aaf:test"));
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Model,
            "claude",
            "sonnet-4",
            b"modelref",
        ));
        let j = s.to_json();
        let back: AgentSbom = serde_json::from_str(&j).unwrap();
        assert_eq!(back.entries.len(), 1);
    }
}
