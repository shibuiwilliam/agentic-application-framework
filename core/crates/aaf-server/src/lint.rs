//! Ontology lint (E2 Slice C).
//!
//! Scans a directory of capability YAML files and reports any
//! capability that is missing an entity-space declaration
//! (`reads:` / `writes:` / `emits:`). The tool is deliberately the
//! thinnest possible wrapper around `serde_yaml::from_str` +
//! `CapabilityContract`, because the only *semantic* work is
//! classifying each finding as `Ok` / `Warn` / `Error` — and that
//! classification is policy, not parsing.
//!
//! **Severity ladder.**
//!
//! - `Ok` — at least one of `reads`, `writes`, `emits` is populated.
//! - `Warn` — side-effect is `None` or `Read` and no declarations are
//!   present. This is informational: the capability may be
//!   legitimately structure-less (a simple health probe), but human
//!   review is encouraged.
//! - `Error` — side-effect is `Write`, `Delete`, `Send`, or `Payment`
//!   (Rule 9 candidates) and there is no `writes:` declaration. This
//!   is the material failure case: the boundary rule and the
//!   composition checker cannot reason about a write that doesn't
//!   name its entity.
//!
//! **Adoption ratio ramp.** Per `PROJECT.md` §16.2, the
//! intended adoption path is:
//!
//! > "Add a `make ontology-lint` target that warns on capabilities
//! > missing entity declarations; advance to error once adoption is
//! > \>90%."
//!
//! The tool computes an adoption ratio (how many scanned capabilities
//! already carry declarations) and uses it to choose the reporting
//! mode:
//!
//! - adoption < 90% → `warn-only` (every finding is reported at most
//!   as `Warn`, even writers)
//! - adoption ≥ 90% → `strict` (writers without declarations are
//!   reported as `Error`)
//!
//! The threshold is a `const` rather than a config option so there
//! is exactly one value in the entire codebase.

use aaf_contracts::{CapabilityContract, SideEffect};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// Ratio at which the lint flips from warn-only to strict.
pub const ADOPTION_STRICT_THRESHOLD: f32 = 0.90;

/// Severity of a single lint finding.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Severity {
    /// No violation — the capability carries at least one entity
    /// declaration.
    Ok,
    /// Informational — read or pure-function capability without a
    /// declaration. Not a blocker.
    Warn,
    /// Material — write/delete/send/payment capability without a
    /// `writes:` declaration. Blocker once adoption ≥ 90%.
    Error,
}

/// A single finding for one capability file.
#[derive(Debug, Clone, Serialize)]
pub struct LintFinding {
    /// Path of the YAML the finding was produced from.
    pub path: PathBuf,
    /// Capability id as parsed from the YAML (or `"<parse-error>"` if
    /// the file did not deserialise).
    pub capability_id: String,
    /// Severity after ratio ramp has been applied.
    pub severity: Severity,
    /// Short human-readable reason.
    pub reason: String,
}

/// Aggregate report over a whole directory scan.
#[derive(Debug, Clone, Serialize)]
pub struct LintReport {
    /// Total number of capability YAMLs scanned.
    pub scanned: usize,
    /// Number that already carry at least one entity declaration.
    pub with_declarations: usize,
    /// Adoption ratio in `[0, 1]`.
    pub adoption_ratio: f32,
    /// Whether strict mode is in effect.
    pub strict: bool,
    /// All findings, in scan order.
    pub findings: Vec<LintFinding>,
}

impl LintReport {
    /// Returns `true` if any finding is `Severity::Error`.
    pub fn has_errors(&self) -> bool {
        self.findings.iter().any(|f| f.severity == Severity::Error)
    }

    /// Count findings of a given severity.
    pub fn count(&self, s: Severity) -> usize {
        self.findings.iter().filter(|f| f.severity == s).count()
    }
}

/// Classify a single capability. Severity is provisional — the caller
/// applies the adoption-ratio ramp afterwards.
fn classify(cap: &CapabilityContract) -> (Severity, String) {
    let has_decl = !cap.reads.is_empty() || !cap.writes.is_empty() || !cap.emits.is_empty();
    if has_decl {
        return (Severity::Ok, "has entity declarations".into());
    }
    match cap.side_effect {
        SideEffect::Write
        | SideEffect::Delete
        | SideEffect::Send
        | SideEffect::Payment => (
            Severity::Error,
            format!(
                "capability declares side_effect `{}` but `writes:` is empty; the boundary rule and composition checker cannot reason about it",
                side_effect_name(cap.side_effect)
            ),
        ),
        SideEffect::Read | SideEffect::None => (
            Severity::Warn,
            "capability has no entity declarations; add `reads:` to let the planner key memory retrieval off nouns".into(),
        ),
    }
}

fn side_effect_name(s: SideEffect) -> &'static str {
    match s {
        SideEffect::None => "none",
        SideEffect::Read => "read",
        SideEffect::Write => "write",
        SideEffect::Delete => "delete",
        SideEffect::Send => "send",
        SideEffect::Payment => "payment",
    }
}

/// Lint a single capability YAML file.
///
/// Returns a `LintFinding` rather than a `Result` because parse
/// failures are themselves lint findings: the tool is supposed to
/// *describe* the state of a directory, not bail out on the first
/// malformed file.
pub fn lint_file(path: &Path) -> LintFinding {
    let raw = match std::fs::read_to_string(path) {
        Ok(s) => s,
        Err(err) => {
            return LintFinding {
                path: path.to_path_buf(),
                capability_id: "<read-error>".into(),
                severity: Severity::Error,
                reason: format!("could not read file: {err}"),
            };
        }
    };
    let cap: CapabilityContract = match serde_yaml::from_str(&raw) {
        Ok(c) => c,
        Err(err) => {
            return LintFinding {
                path: path.to_path_buf(),
                capability_id: "<parse-error>".into(),
                severity: Severity::Error,
                reason: format!("could not parse as CapabilityContract: {err}"),
            };
        }
    };
    let (severity, reason) = classify(&cap);
    LintFinding {
        path: path.to_path_buf(),
        capability_id: cap.id.to_string(),
        severity,
        reason,
    }
}

/// Lint every `capability-*.yaml` / `capability-*.yml` file under
/// `dir` (non-recursive). Applies the adoption-ratio ramp:
/// if < 90% of scanned files carry declarations, every `Error` is
/// downgraded to a `Warn`.
pub fn lint_directory(dir: &Path) -> std::io::Result<LintReport> {
    let mut paths: Vec<PathBuf> = vec![];
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let p = entry.path();
        if !p.is_file() {
            continue;
        }
        let name = p.file_name().and_then(|s| s.to_str()).unwrap_or("");
        if !name.starts_with("capability-") {
            continue;
        }
        if !(name.ends_with(".yaml") || name.ends_with(".yml")) {
            continue;
        }
        paths.push(p);
    }
    paths.sort();

    let mut findings: Vec<LintFinding> = paths.iter().map(|p| lint_file(p)).collect();
    let scanned = findings.len();
    let with_declarations = findings
        .iter()
        .filter(|f| f.severity == Severity::Ok)
        .count();
    let adoption_ratio = if scanned == 0 {
        0.0
    } else {
        with_declarations as f32 / scanned as f32
    };
    let strict = adoption_ratio >= ADOPTION_STRICT_THRESHOLD;
    if !strict {
        for f in &mut findings {
            if f.severity == Severity::Error {
                f.severity = Severity::Warn;
            }
        }
    }
    Ok(LintReport {
        scanned,
        with_declarations,
        adoption_ratio,
        strict,
        findings,
    })
}

/// Render a [`LintReport`] to stdout in a terse multi-line format
/// suitable for CI logs.
pub fn print_report(r: &LintReport) {
    println!(
        "scanned: {}, with declarations: {} ({:.0}%), mode: {}",
        r.scanned,
        r.with_declarations,
        r.adoption_ratio * 100.0,
        if r.strict { "strict" } else { "warn-only" }
    );
    for f in &r.findings {
        let tag = match f.severity {
            Severity::Ok => "  OK  ",
            Severity::Warn => "  WARN",
            Severity::Error => "  ERR ",
        };
        println!(
            "{tag} {:<40} {}  — {}",
            f.capability_id,
            f.path.file_name().unwrap_or_default().to_string_lossy(),
            f.reason
        );
    }
    let errs = r.count(Severity::Error);
    let warns = r.count(Severity::Warn);
    println!("\n{} errors, {} warnings", errs, warns);
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla, CompensationSpec,
        DataClassification, EndpointKind, EntityRefLite,
    };

    fn base(id: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: "n".into(),
            description: "d".into(),
            version: "1.0".into(),
            provider_agent: "a".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::Grpc,
                address: "x".into(),
                method: None,
            },
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            side_effect: SideEffect::Read,
            idempotent: true,
            reversible: true,
            deterministic: true,
            compensation: None,
            sla: CapabilitySla::default(),
            cost: CapabilityCost::default(),
            required_scope: "x:read".into(),
            data_classification: DataClassification::Internal,
            degradation: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            tags: vec![],
            domains: vec![],
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        }
    }

    #[test]
    fn classify_ok_when_reads_present() {
        let mut c = base("cap-a");
        c.reads = vec![EntityRefLite::new("commerce.Order")];
        assert_eq!(classify(&c).0, Severity::Ok);
    }

    #[test]
    fn classify_warn_on_read_without_decl() {
        let c = base("cap-r");
        assert_eq!(classify(&c).0, Severity::Warn);
    }

    #[test]
    fn classify_error_on_write_without_decl() {
        let mut c = base("cap-w");
        c.side_effect = SideEffect::Write;
        c.compensation = Some(CompensationSpec {
            endpoint: "cap-undo".into(),
        });
        assert_eq!(classify(&c).0, Severity::Error);
    }

    #[test]
    fn classify_error_on_payment_without_decl() {
        let mut c = base("cap-p");
        c.side_effect = SideEffect::Payment;
        c.compensation = Some(CompensationSpec {
            endpoint: "cap-refund".into(),
        });
        assert_eq!(classify(&c).0, Severity::Error);
    }

    #[test]
    fn ratio_ramp_downgrades_errors_below_threshold() {
        // 1 good + 1 writer-without-decl = 50% adoption → warn-only.
        let tmp = tempdir();
        write_yaml(&tmp, "capability-a.yaml", "cap-a", true, SideEffect::Read);
        write_yaml(&tmp, "capability-b.yaml", "cap-b", false, SideEffect::Write);

        let report = lint_directory(&tmp).unwrap();
        assert_eq!(report.scanned, 2);
        assert!(!report.strict);
        assert_eq!(
            report.count(Severity::Error),
            0,
            "warn-only mode must downgrade errors"
        );
        assert!(report.count(Severity::Warn) >= 1);
    }

    #[test]
    fn ratio_ramp_keeps_errors_above_threshold() {
        // 9 good + 1 writer-without-decl = 90% adoption → strict.
        let tmp = tempdir();
        for i in 0..9 {
            write_yaml(
                &tmp,
                &format!("capability-ok-{i}.yaml"),
                &format!("cap-ok-{i}"),
                true,
                SideEffect::Read,
            );
        }
        write_yaml(
            &tmp,
            "capability-bad.yaml",
            "cap-bad",
            false,
            SideEffect::Write,
        );

        let report = lint_directory(&tmp).unwrap();
        assert_eq!(report.scanned, 10);
        assert!(report.strict);
        assert_eq!(report.count(Severity::Error), 1);
        assert!(report.has_errors());
    }

    #[test]
    fn empty_directory_is_not_an_error() {
        let tmp = tempdir();
        let report = lint_directory(&tmp).unwrap();
        assert_eq!(report.scanned, 0);
        assert!(!report.has_errors());
    }

    // ── tiny local tempdir helpers (no tempfile dep) ──────────────────

    fn tempdir() -> PathBuf {
        let base = std::env::temp_dir();
        let p = base.join(format!(
            "aaf-lint-test-{}-{}",
            std::process::id(),
            uuid_like()
        ));
        std::fs::create_dir_all(&p).unwrap();
        p
    }

    fn uuid_like() -> String {
        use std::time::{SystemTime, UNIX_EPOCH};
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        format!("{n:x}")
    }

    fn write_yaml(dir: &Path, fname: &str, id: &str, with_decl: bool, side_effect: SideEffect) {
        let reads = if with_decl {
            "reads:\n  - entity_id: commerce.Order\n"
        } else {
            ""
        };
        let comp = match side_effect {
            SideEffect::Write | SideEffect::Delete | SideEffect::Send | SideEffect::Payment => {
                "compensation:\n  endpoint: cap-undo\n"
            }
            _ => "",
        };
        let yaml = format!(
            "id: {id}
name: {id}
description: {id}
version: 1.0
provider_agent: a
endpoint:
  type: grpc
  address: x
side_effect: {se}
idempotent: true
reversible: true
deterministic: true
required_scope: x:read
data_classification: internal
{comp}{reads}",
            se = side_effect_name(side_effect)
        );
        std::fs::write(dir.join(fname), yaml).unwrap();
    }
}
