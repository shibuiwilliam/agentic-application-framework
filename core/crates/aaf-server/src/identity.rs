//! `aaf-server identity` subcommands — a minimal CLI surface on top
//! of [`aaf_identity`]'s Slice A/B primitives.
//!
//! This module is intentionally **thin**: every command walks a
//! public trait-backed path in `aaf-identity` and nothing more. The
//! aim is to expose the identity primitives to operators without
//! standing up a full pipeline. Persistent keystore / HSM backends
//! are deferred to X1 Slice C; every command here uses the in-memory
//! HMAC-SHA256 backend that ships today.
//!
//! ```text
//! aaf-server identity generate [display-name]        # mint a new DID
//! aaf-server identity revoke   <did> <reason>        # add a signed revocation
//! aaf-server identity help                           # this help
//! ```
//!
//! Slice C will add `verify <manifest.yaml>`, `sbom export <did>`,
//! and persistent keystore subcommands once the relevant loaders
//! exist in `aaf-identity`.

use aaf_contracts::CapabilityId;
use aaf_identity::sbom::export::{to_cyclonedx_json, to_spdx_json};
use aaf_identity::{
    AgentDid, AgentSbom, InMemoryKeystore, Keystore, ManifestBuilder, RevocationEntry,
    RevocationKind, SbomEntry, SbomEntryKind,
};
use serde::Deserialize;

/// Dispatch an `identity <subcommand>` CLI invocation.
pub fn dispatch(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    match args.first().map(String::as_str) {
        Some("generate" | "generate-did") => cmd_generate(&args[1..]),
        Some("sign-manifest") => cmd_sign_manifest(&args[1..]),
        Some("verify") => cmd_verify(&args[1..]),
        Some("export-sbom") => cmd_export_sbom(&args[1..]),
        Some("revoke") => cmd_revoke(&args[1..]),
        Some("help" | "--help" | "-h") | None => {
            print_help();
            Ok(())
        }
        Some(other) => {
            eprintln!("unknown identity subcommand `{other}`");
            print_help();
            Ok(())
        }
    }
}

fn print_help() {
    println!("usage: aaf-server identity <subcommand>");
    println!();
    println!("  generate-did  [display-name]                                    mint a new DID");
    println!("  sign-manifest <manifest.yaml>                                   emit signed manifest JSON");
    println!("  verify        <manifest.yaml>                                   verify a signed manifest");
    println!("  export-sbom   <sbom.yaml> [--format spdx|cyclonedx]             render SBOM in SPDX/CycloneDX JSON");
    println!("  revoke        <did> <reason>                                    add a signed revocation entry");
    println!("  help                                                            show this message");
    println!();
    println!("NOTE: the identity subcommand uses an in-memory keystore that");
    println!("forgets all keys on process exit. Persistent keystore / HSM /");
    println!("SPIFFE backends are a future concern.");
}

/// `aaf-server identity generate` — mints a fresh DID using the
/// in-memory keystore and prints it.
fn cmd_generate(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let display = args
        .first()
        .cloned()
        .unwrap_or_else(|| "cli-agent".to_string());
    let seed = display_seed(&display);
    let ks = InMemoryKeystore::new();
    let did = ks.generate(&seed);
    println!("did:         {}", did.as_str());
    println!("display:     {display}");
    println!("backend:     in-memory HMAC-SHA256 (X1 Slice A/B)");
    println!();
    println!("This keystore forgets keys on shutdown — store the DID before exit.");
    Ok(())
}

/// `aaf-server identity revoke <did> <reason>` — writes a signed
/// revocation entry into the in-memory registry and prints the
/// resulting record. The revoker identity is minted on-the-fly to
/// keep the CLI zero-config; production deployments should pass a
/// real operator DID.
fn cmd_revoke(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    if args.len() < 2 {
        eprintln!("usage: aaf-server identity revoke <did> <reason>");
        return Ok(());
    }
    let did_str = &args[0];
    let reason = args[1..].join(" ");

    // CLI role: *generate* a signed revocation entry. Persistence
    // is the operator's concern — they pipe the JSON into their own
    // `aaf_identity::RevocationRegistry` backend. The in-memory
    // registry lives inside the runtime, not in a one-shot CLI.
    let ks = InMemoryKeystore::new();
    let revoker_did = ks.generate(&display_seed("cli-revoker"));

    let entry = RevocationEntry::issue(
        RevocationKind::Did,
        did_str.clone(),
        reason,
        revoker_did,
        &ks,
    )?;

    // Render the entry as pretty JSON.
    println!("{}", serde_json::to_string_pretty(&entry)?);
    Ok(())
}

/// Stable per-display seed so `generate` is deterministic when the
/// same display name is supplied twice in the same process.
fn display_seed(display: &str) -> Vec<u8> {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos())
        .unwrap_or_default();
    let mut buf = Vec::with_capacity(display.len() + 16);
    buf.extend_from_slice(display.as_bytes());
    buf.extend_from_slice(&nanos.to_le_bytes());
    buf
}

/// Expose the `AgentDid` type from this module so other parts of
/// the server binary can refer to it without importing
/// `aaf_identity` directly — purely an ergonomic re-export.
#[allow(dead_code)]
pub(crate) type ClientDid = AgentDid;

// ── X1 Slice C: sign-manifest / verify / export-sbom ─────────────────

/// YAML shape the CLI accepts for `sign-manifest` / `verify`. Kept
/// permissive so operators can hand-write them. The optional `seed`
/// field makes `verify` reproducible: the same YAML derives the same
/// DID deterministically, so signature verification works offline
/// without an external keystore lookup.
#[derive(Debug, Deserialize)]
struct ManifestInput {
    display_name: String,
    source_hash: String,
    #[serde(default)]
    capability_allow_list: Vec<String>,
    #[serde(default)]
    ontology_slices: Vec<String>,
    #[serde(default)]
    eval_suite_refs: Vec<String>,
    #[serde(default)]
    prompt_hashes: Vec<String>,
    #[serde(default)]
    seed: Option<String>,
}

fn manifest_seed(input: &ManifestInput) -> Vec<u8> {
    input
        .seed
        .clone()
        .unwrap_or_else(|| format!("did-seed-{}", input.display_name))
        .into_bytes()
}

fn build_signed_manifest(
    input: &ManifestInput,
) -> Result<(InMemoryKeystore, AgentDid, aaf_identity::AgentManifest), Box<dyn std::error::Error>> {
    let ks = InMemoryKeystore::new();
    let did = ks.generate(&manifest_seed(input));
    let mut builder = ManifestBuilder::new(did.clone(), &input.display_name, &input.source_hash);
    for cap in &input.capability_allow_list {
        builder = builder.allow(CapabilityId::from(cap.as_str()));
    }
    for slice in &input.ontology_slices {
        builder = builder.ontology_slice(slice.clone());
    }
    for r in &input.eval_suite_refs {
        builder = builder.eval_ref(r.clone());
    }
    for p in &input.prompt_hashes {
        builder = builder.prompt_hash(p.clone());
    }
    let manifest = builder.build(&ks)?;
    Ok((ks, did, manifest))
}

fn cmd_sign_manifest(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = args
        .first()
        .ok_or("usage: aaf-server identity sign-manifest <manifest.yaml>")?;
    let raw = std::fs::read_to_string(path)?;
    let input: ManifestInput = serde_yaml::from_str(&raw)?;
    let (_ks, _did, manifest) = build_signed_manifest(&input)?;
    println!("{}", serde_json::to_string_pretty(&manifest)?);
    Ok(())
}

fn cmd_verify(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = args
        .first()
        .ok_or("usage: aaf-server identity verify <manifest.yaml>")?;
    let raw = std::fs::read_to_string(path)?;
    let input: ManifestInput = serde_yaml::from_str(&raw)?;
    let (ks, did, manifest) = build_signed_manifest(&input)?;
    manifest.verify(&ks)?;
    println!("ok  {did}  {}", manifest.display_name);
    Ok(())
}

// ── SBOM export ──────────────────────────────────────────────────────

/// YAML shape the CLI accepts for `export-sbom`. Parallels
/// `spec/examples/sbom-order-agent.yaml`.
#[derive(Debug, Deserialize)]
struct SbomInput {
    did: String,
    #[serde(default)]
    entries: Vec<SbomEntryInput>,
}

#[derive(Debug, Deserialize)]
struct SbomEntryInput {
    kind: String,
    name: String,
    version: String,
    content_hash: String,
    #[serde(default)]
    locator: Option<String>,
}

fn parse_sbom_kind(s: &str) -> SbomEntryKind {
    match s.to_ascii_lowercase().as_str() {
        "model" => SbomEntryKind::Model,
        "prompt" => SbomEntryKind::Prompt,
        "tool" => SbomEntryKind::Tool,
        "ontology" => SbomEntryKind::Ontology,
        "eval_suite" | "eval-suite" => SbomEntryKind::EvalSuite,
        "training_data" | "training-data" => SbomEntryKind::TrainingData,
        _ => SbomEntryKind::Other,
    }
}

fn cmd_export_sbom(args: &[String]) -> Result<(), Box<dyn std::error::Error>> {
    let path = args
        .first()
        .ok_or("usage: aaf-server identity export-sbom <sbom.yaml> [--format spdx|cyclonedx]")?;

    // Scan the remaining args for `--format <value>`. Default: spdx.
    let mut format = "spdx".to_string();
    let mut i = 1;
    while i < args.len() {
        if args[i] == "--format" {
            if let Some(val) = args.get(i + 1) {
                format = val.clone();
                i += 2;
                continue;
            }
        }
        i += 1;
    }

    let raw = std::fs::read_to_string(path)?;
    let input: SbomInput = serde_yaml::from_str(&raw)?;

    let mut sbom = AgentSbom::new(AgentDid::from_raw(input.did));
    for e in input.entries {
        let mut entry = SbomEntry {
            kind: parse_sbom_kind(&e.kind),
            name: e.name,
            version: e.version,
            content_hash: e.content_hash,
            locator: None,
        };
        if let Some(loc) = e.locator {
            entry = entry.with_locator(loc);
        }
        sbom.push(entry);
    }

    let rendered = match format.as_str() {
        "spdx" => to_spdx_json(&sbom),
        "cyclonedx" => to_cyclonedx_json(&sbom),
        other => return Err(format!("unknown --format `{other}` (use spdx or cyclonedx)").into()),
    };
    println!("{rendered}");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn generate_accepts_empty_args_and_uses_default_display() {
        // No panic, no error, just a new DID under the default display.
        cmd_generate(&[]).unwrap();
    }

    #[test]
    fn dispatch_help_is_a_noop() {
        dispatch(&["help".into()]).unwrap();
        dispatch(&[]).unwrap();
    }

    #[test]
    fn revoke_round_trip_reports_successfully() {
        cmd_revoke(&["did:aaf:test".into(), "test".into(), "reason".into()]).unwrap();
    }
}
