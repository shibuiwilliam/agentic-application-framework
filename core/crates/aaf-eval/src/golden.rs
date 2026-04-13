//! Golden suite loader + runner.
//!
//! A golden suite is a YAML document listing `(intent_text,
//! expected_output)` cases plus a pass-threshold. The runner judges
//! each case with a [`crate::judge::Judge`] and emits a pass/fail
//! report.

use crate::judge::{Judge, JudgeVerdict};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// One case in a golden suite.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoldenCase {
    /// Stable id.
    pub id: String,
    /// Intent text.
    pub intent: String,
    /// Expected output string.
    pub expected: String,
    /// Optional override threshold; falls back to `GoldenSuite::threshold`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub min_score: Option<f64>,
}

/// A suite of golden cases.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct GoldenSuite {
    /// Suite name shown in reports.
    pub name: String,
    /// Default pass threshold.
    pub threshold: f64,
    /// Test cases.
    pub cases: Vec<GoldenCase>,
}

/// Per-case result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CaseResult {
    /// Which case this result belongs to.
    pub case_id: String,
    /// Judge verdict.
    pub verdict: JudgeVerdict,
    /// Whether the case passed its threshold.
    pub passed: bool,
}

/// Aggregate run result.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SuiteResult {
    /// Suite name (copied in for convenience).
    pub suite: String,
    /// Total cases.
    pub total: usize,
    /// Passed cases.
    pub passed: usize,
    /// Mean score across all cases.
    pub mean_score: f64,
    /// Per-case results in input order.
    pub cases: Vec<CaseResult>,
}

impl SuiteResult {
    /// Whether every case passed.
    pub fn all_passed(&self) -> bool {
        self.passed == self.total
    }
}

/// Errors raised by the suite loader.
#[derive(Debug, Error)]
pub enum GoldenError {
    /// YAML could not be parsed.
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    /// Suite is empty.
    #[error("suite has no cases")]
    Empty,
}

impl GoldenSuite {
    /// Load from a YAML string.
    pub fn from_yaml(yaml: &str) -> Result<Self, GoldenError> {
        let s: Self = serde_yaml::from_str(yaml)?;
        if s.cases.is_empty() {
            return Err(GoldenError::Empty);
        }
        Ok(s)
    }

    /// Run the suite against a provider function + judge. The provider
    /// is a closure that maps an intent string to a produced output —
    /// tests pass a deterministic mock; real deployments pass a call
    /// into the runtime.
    pub async fn run<F>(&self, provider: F, judge: &dyn Judge) -> SuiteResult
    where
        F: Fn(&str) -> String,
    {
        let mut cases = Vec::with_capacity(self.cases.len());
        let mut total_score = 0.0_f64;
        let mut passed = 0usize;
        for c in &self.cases {
            let actual = provider(&c.intent);
            let verdict = judge.judge(&c.expected, &actual).await;
            let threshold = c.min_score.unwrap_or(self.threshold);
            let ok = verdict.score >= threshold;
            if ok {
                passed += 1;
            }
            total_score += verdict.score;
            cases.push(CaseResult {
                case_id: c.id.clone(),
                verdict,
                passed: ok,
            });
        }
        let mean_score = if cases.is_empty() {
            0.0
        } else {
            total_score / cases.len() as f64
        };
        SuiteResult {
            suite: self.name.clone(),
            total: self.cases.len(),
            passed,
            mean_score,
            cases,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::judge::DeterministicJudge;

    const SAMPLE: &str = r"
name: order-processing
threshold: 0.5
cases:
  - id: happy-order
    intent: place an order for SKU-1
    expected: order placed for SKU-1
  - id: stock-query
    intent: check stock for SKU-1
    expected: stock level for SKU-1
";

    #[tokio::test]
    async fn parses_and_runs_suite() {
        let suite = GoldenSuite::from_yaml(SAMPLE).unwrap();
        let judge = DeterministicJudge::default();
        let result = suite.run(|intent| format!("ran: {intent}"), &judge).await;
        assert_eq!(result.total, 2);
        assert_eq!(result.cases.len(), 2);
    }

    #[tokio::test]
    async fn perfect_match_all_pass() {
        let suite = GoldenSuite::from_yaml(SAMPLE).unwrap();
        let judge = DeterministicJudge::default();
        // Provider returns exactly the expected string for any intent.
        let result = suite.run(|_| "order placed for SKU-1".into(), &judge).await;
        // The first case will perfectly match, the second won't.
        assert!(result.cases[0].passed);
    }

    #[test]
    fn empty_suite_is_rejected() {
        let y = "name: empty\nthreshold: 0.5\ncases: []\n";
        let err = GoldenSuite::from_yaml(y).unwrap_err();
        assert!(matches!(err, GoldenError::Empty));
    }
}
