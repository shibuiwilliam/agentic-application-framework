//! JSON report writer — stable on-disk format for CI to parse.

use crate::golden::SuiteResult;
use crate::regression::RegressionReport;
use serde::{Deserialize, Serialize};

/// Combined report document emitted by `make test-semantic-regression`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ReportDocument {
    /// Suite run result.
    pub suite: SuiteResult,
    /// Optional regression report against a baseline.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regression: Option<RegressionReport>,
}

/// Helper to emit [`ReportDocument`]s as JSON.
pub struct ReportWriter;

impl ReportWriter {
    /// Render a report to a JSON string (stable key order via
    /// `serde_json`).
    pub fn to_json(doc: &ReportDocument) -> String {
        serde_json::to_string_pretty(doc).unwrap_or_else(|e| format!("{{\"error\":\"{e}\"}}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::CaseResult;
    use crate::judge::JudgeVerdict;

    #[test]
    fn renders_minimal_report() {
        let doc = ReportDocument {
            suite: SuiteResult {
                suite: "x".into(),
                total: 1,
                passed: 1,
                mean_score: 1.0,
                cases: vec![CaseResult {
                    case_id: "c1".into(),
                    verdict: JudgeVerdict {
                        score: 1.0,
                        reasoning: String::new(),
                        judge_model: "mock".into(),
                    },
                    passed: true,
                }],
            },
            regression: None,
        };
        let s = ReportWriter::to_json(&doc);
        assert!(s.contains("\"passed\": 1"));
    }
}
