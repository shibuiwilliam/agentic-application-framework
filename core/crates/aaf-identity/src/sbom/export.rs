//! SPDX + CycloneDX exporters for [`super::AgentSbom`].
//!
//! These are **interop** shapes — they do not try to cover every
//! field of SPDX 2.3 or CycloneDX 1.5. They produce enough structured
//! JSON that any standards-based consumer (Dependency-Track, Grype,
//! FOSSA, Snyk SBOM Import, etc.) can ingest the bill of materials.
//!
//! The guiding principle is: **every AAF SBOM entry must show up as
//! exactly one package / component in the exported document, with a
//! content hash the consumer can re-verify**.
//!
//! # SPDX (v2.3 subset)
//!
//! Maps an `AgentSbom` to an `SPDXDocument` with:
//! - `spdxVersion = "SPDX-2.3"`
//! - `SPDXID = "SPDXRef-DOCUMENT"`
//! - `name` = the agent DID
//! - `packages[]` — one per SBOM entry, with `checksums[].algorithm
//!   = "SHA256"` and `checksumValue` = the entry's content hash
//!
//! # CycloneDX (v1.5 subset)
//!
//! Maps an `AgentSbom` to a `CycloneDXDocument` with:
//! - `bomFormat = "CycloneDX"`
//! - `specVersion = "1.5"`
//! - `version = 1`
//! - `metadata.component.name` = the agent DID
//! - `components[]` — one per SBOM entry, with `hashes[].alg
//!   = "SHA-256"` and `content` = the entry's content hash

use super::{AgentSbom, SbomEntry, SbomEntryKind};
use serde::{Deserialize, Serialize};

// ── SPDX 2.3 subset ────────────────────────────────────────────────

/// Minimal SPDX 2.3 document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxDocument {
    #[serde(rename = "spdxVersion")]
    /// Version tag.
    pub spdx_version: String,
    #[serde(rename = "dataLicense")]
    /// License under which the SBOM document itself is shared.
    pub data_license: String,
    #[serde(rename = "SPDXID")]
    /// Root id.
    pub spdx_id: String,
    /// Document name.
    pub name: String,
    #[serde(rename = "documentNamespace")]
    /// Namespace URI.
    pub document_namespace: String,
    #[serde(rename = "creationInfo")]
    /// Creation metadata.
    pub creation_info: SpdxCreationInfo,
    /// Packages enumerated in the document.
    pub packages: Vec<SpdxPackage>,
}

/// SPDX creation-info block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxCreationInfo {
    /// Creator identifier.
    pub creators: Vec<String>,
    /// ISO 8601 creation timestamp.
    pub created: String,
}

/// SPDX package entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxPackage {
    #[serde(rename = "SPDXID")]
    /// Stable id within the document.
    pub spdx_id: String,
    /// Display name.
    pub name: String,
    #[serde(rename = "versionInfo")]
    /// Version string.
    pub version_info: String,
    #[serde(rename = "downloadLocation")]
    /// Download URL, or `NOASSERTION`.
    pub download_location: String,
    #[serde(rename = "filesAnalyzed")]
    /// SPDX `filesAnalyzed` flag.
    pub files_analyzed: bool,
    /// Content hashes.
    pub checksums: Vec<SpdxChecksum>,
    #[serde(rename = "licenseConcluded")]
    /// License.
    pub license_concluded: String,
    #[serde(rename = "licenseDeclared")]
    /// Declared license.
    pub license_declared: String,
    #[serde(rename = "copyrightText")]
    /// Copyright text.
    pub copyright_text: String,
}

/// SPDX checksum block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SpdxChecksum {
    /// Algorithm name (e.g. `SHA256`).
    pub algorithm: String,
    #[serde(rename = "checksumValue")]
    /// Hex-encoded digest.
    pub checksum_value: String,
}

// ── CycloneDX 1.5 subset ───────────────────────────────────────────

/// Minimal CycloneDX 1.5 document.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycloneDxDocument {
    #[serde(rename = "bomFormat")]
    /// Literal `"CycloneDX"`.
    pub bom_format: String,
    #[serde(rename = "specVersion")]
    /// Literal `"1.5"`.
    pub spec_version: String,
    /// BOM revision counter.
    pub version: u32,
    /// Optional serial number URN (e.g. `urn:uuid:...`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub serial_number: Option<String>,
    /// Metadata block.
    pub metadata: CycloneDxMetadata,
    /// Components enumerated in the document.
    pub components: Vec<CycloneDxComponent>,
}

/// CycloneDX metadata block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycloneDxMetadata {
    /// ISO 8601 timestamp.
    pub timestamp: String,
    /// Tools used to generate the document.
    pub tools: Vec<CycloneDxTool>,
    /// Subject component.
    pub component: CycloneDxComponent,
}

/// CycloneDX tool block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycloneDxTool {
    /// Vendor.
    pub vendor: String,
    /// Tool name.
    pub name: String,
    /// Tool version.
    pub version: String,
}

/// CycloneDX component block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycloneDxComponent {
    #[serde(rename = "type")]
    /// `"application"`, `"library"`, `"data"`, `"machine-learning-model"`, ...
    pub component_type: String,
    #[serde(rename = "bom-ref")]
    /// Stable in-document id.
    pub bom_ref: String,
    /// Display name.
    pub name: String,
    /// Version string.
    pub version: String,
    /// Content hashes.
    pub hashes: Vec<CycloneDxHash>,
}

/// CycloneDX hash block.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CycloneDxHash {
    /// Algorithm name (e.g. `SHA-256`).
    pub alg: String,
    /// Hex-encoded digest.
    pub content: String,
}

// ── Mapping helpers ────────────────────────────────────────────────

fn spdx_type_for(kind: SbomEntryKind) -> &'static str {
    match kind {
        SbomEntryKind::Model => "LIBRARY",
        SbomEntryKind::Prompt => "FILE",
        SbomEntryKind::Tool => "APPLICATION",
        SbomEntryKind::Ontology => "FILE",
        SbomEntryKind::EvalSuite => "FILE",
        SbomEntryKind::TrainingData => "FILE",
        SbomEntryKind::Other => "OTHER",
    }
}

fn cyclonedx_type_for(kind: SbomEntryKind) -> &'static str {
    match kind {
        SbomEntryKind::Model => "machine-learning-model",
        SbomEntryKind::Prompt => "data",
        SbomEntryKind::Tool => "application",
        SbomEntryKind::Ontology => "data",
        SbomEntryKind::EvalSuite => "data",
        SbomEntryKind::TrainingData => "data",
        SbomEntryKind::Other => "library",
    }
}

fn slug(name: &str) -> String {
    name.chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect()
}

fn spdx_package(entry: &SbomEntry, idx: usize) -> SpdxPackage {
    let _ = spdx_type_for(entry.kind);
    SpdxPackage {
        spdx_id: format!("SPDXRef-Package-{idx}-{}", slug(&entry.name)),
        name: entry.name.clone(),
        version_info: entry.version.clone(),
        download_location: entry
            .locator
            .clone()
            .unwrap_or_else(|| "NOASSERTION".into()),
        files_analyzed: false,
        checksums: vec![SpdxChecksum {
            algorithm: "SHA256".into(),
            checksum_value: entry.content_hash.clone(),
        }],
        license_concluded: "NOASSERTION".into(),
        license_declared: "NOASSERTION".into(),
        copyright_text: "NOASSERTION".into(),
    }
}

fn cyclonedx_component(entry: &SbomEntry, idx: usize) -> CycloneDxComponent {
    CycloneDxComponent {
        component_type: cyclonedx_type_for(entry.kind).into(),
        bom_ref: format!("{}-{}", idx, slug(&entry.name)),
        name: entry.name.clone(),
        version: entry.version.clone(),
        hashes: vec![CycloneDxHash {
            alg: "SHA-256".into(),
            content: entry.content_hash.clone(),
        }],
    }
}

/// Convert an `AgentSbom` into an SPDX 2.3 document.
pub fn to_spdx(sbom: &AgentSbom) -> SpdxDocument {
    let packages = sbom
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| spdx_package(e, i))
        .collect();
    SpdxDocument {
        spdx_version: "SPDX-2.3".into(),
        data_license: "CC0-1.0".into(),
        spdx_id: "SPDXRef-DOCUMENT".into(),
        name: format!("aaf-agent-sbom-{}", sbom.did),
        document_namespace: format!("https://aaf.dev/sbom/{}", sbom.did),
        creation_info: SpdxCreationInfo {
            creators: vec!["Tool: aaf-identity".into()],
            created: sbom.generated_at.to_rfc3339(),
        },
        packages,
    }
}

/// Convert an `AgentSbom` into an SPDX 2.3 JSON string.
pub fn to_spdx_json(sbom: &AgentSbom) -> String {
    serde_json::to_string_pretty(&to_spdx(sbom))
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

/// Convert an `AgentSbom` into a CycloneDX 1.5 document.
pub fn to_cyclonedx(sbom: &AgentSbom) -> CycloneDxDocument {
    let components: Vec<CycloneDxComponent> = sbom
        .entries
        .iter()
        .enumerate()
        .map(|(i, e)| cyclonedx_component(e, i))
        .collect();
    CycloneDxDocument {
        bom_format: "CycloneDX".into(),
        spec_version: "1.5".into(),
        version: 1,
        serial_number: None,
        metadata: CycloneDxMetadata {
            timestamp: sbom.generated_at.to_rfc3339(),
            tools: vec![CycloneDxTool {
                vendor: "aaf.dev".into(),
                name: "aaf-identity".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            }],
            component: CycloneDxComponent {
                component_type: "application".into(),
                bom_ref: sbom.did.to_string(),
                name: sbom.did.to_string(),
                version: "1.0.0".into(),
                hashes: vec![],
            },
        },
        components,
    }
}

/// Convert an `AgentSbom` into a CycloneDX 1.5 JSON string.
pub fn to_cyclonedx_json(sbom: &AgentSbom) -> String {
    serde_json::to_string_pretty(&to_cyclonedx(sbom))
        .unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
}

#[cfg(test)]
mod tests {
    use super::super::{AgentSbom, SbomEntry, SbomEntryKind};
    use super::*;
    use crate::did::AgentDid;

    fn sbom() -> AgentSbom {
        let mut s = AgentSbom::new(AgentDid::from_raw("did:aaf:test12345678"));
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Model,
            "claude-sonnet-4-20250514",
            "sonnet-4",
            b"model-bytes",
        ));
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Prompt,
            "system",
            "v1",
            b"prompt-bytes",
        ));
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Tool,
            "cap-order-read",
            "1.0.0",
            b"tool-bytes",
        ));
        s.push(SbomEntry::from_bytes(
            SbomEntryKind::Ontology,
            "commerce.Order",
            "0.1.0",
            b"ontology-bytes",
        ));
        s
    }

    #[test]
    fn spdx_document_contains_one_package_per_entry() {
        let s = sbom();
        let doc = to_spdx(&s);
        assert_eq!(doc.spdx_version, "SPDX-2.3");
        assert_eq!(doc.packages.len(), s.entries.len());
        // Every hash lands in the SPDX checksum block.
        for (entry, package) in s.entries.iter().zip(doc.packages.iter()) {
            assert_eq!(package.name, entry.name);
            assert_eq!(package.version_info, entry.version);
            assert_eq!(package.checksums[0].algorithm, "SHA256");
            assert_eq!(package.checksums[0].checksum_value, entry.content_hash);
        }
    }

    #[test]
    fn spdx_json_round_trips_through_serde() {
        let s = sbom();
        let json = to_spdx_json(&s);
        let parsed: SpdxDocument = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.packages.len(), s.entries.len());
    }

    #[test]
    fn cyclonedx_document_contains_one_component_per_entry() {
        let s = sbom();
        let doc = to_cyclonedx(&s);
        assert_eq!(doc.bom_format, "CycloneDX");
        assert_eq!(doc.spec_version, "1.5");
        assert_eq!(doc.components.len(), s.entries.len());
        for (entry, comp) in s.entries.iter().zip(doc.components.iter()) {
            assert_eq!(comp.name, entry.name);
            assert_eq!(comp.hashes[0].alg, "SHA-256");
            assert_eq!(comp.hashes[0].content, entry.content_hash);
        }
    }

    #[test]
    fn cyclonedx_json_round_trips_through_serde() {
        let s = sbom();
        let json = to_cyclonedx_json(&s);
        let parsed: CycloneDxDocument = serde_json::from_str(&json).expect("parse");
        assert_eq!(parsed.components.len(), s.entries.len());
        assert_eq!(parsed.metadata.tools[0].name, "aaf-identity");
    }

    #[test]
    fn slug_replaces_non_alnum_with_dashes() {
        assert_eq!(slug("hello_world"), "hello-world");
        assert_eq!(slug("cap-order.read"), "cap-order-read");
    }

    #[test]
    fn component_type_mapping_is_stable() {
        assert_eq!(
            cyclonedx_type_for(SbomEntryKind::Model),
            "machine-learning-model"
        );
        assert_eq!(cyclonedx_type_for(SbomEntryKind::Tool), "application");
        assert_eq!(spdx_type_for(SbomEntryKind::Model), "LIBRARY");
    }
}
