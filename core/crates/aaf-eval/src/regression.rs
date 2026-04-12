//! Regression report — compares two `SuiteResult`s and summarises the
//! delta in a CI-friendly shape.

use crate::golden::SuiteResult;
use serde::{Deserialize, Serialize};

/// Per-case delta between baseline and candidate runs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionDelta {
    /// Case id.
    pub case_id: String,
    /// Baseline score.
    pub baseline_score: f64,
    /// Candidate score.
    pub candidate_score: f64,
    /// `candidate - baseline`.
    pub delta: f64,
}

/// Aggregate regression report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegressionReport {
    /// Baseline suite name.
    pub baseline: String,
    /// Candidate suite name.
    pub candidate: String,
    /// Mean score delta.
    pub mean_delta: f64,
    /// Number of cases where the candidate scored strictly lower than
    /// the baseline.
    pub regressions: u32,
    /// Number of cases where the candidate scored strictly higher.
    pub improvements: u32,
    /// Per-case deltas.
    pub per_case: Vec<RegressionDelta>,
}

impl RegressionReport {
    /// Build a report from two suite results.
    ///
    /// Cases are joined by `case_id`. Cases present in only one side
    /// are dropped from `per_case` but still counted in `regressions`
    /// / `improvements` so missing data is visible.
    pub fn build(baseline: &SuiteResult, candidate: &SuiteResult) -> Self {
        use std::collections::HashMap;
        let base_by: HashMap<_, _> = baseline
            .cases
            .iter()
            .map(|c| (c.case_id.clone(), c.verdict.score))
            .collect();
        let cand_by: HashMap<_, _> = candidate
            .cases
            .iter()
            .map(|c| (c.case_id.clone(), c.verdict.score))
            .collect();

        let mut per_case: Vec<RegressionDelta> = vec![];
        let mut regressions: u32 = 0;
        let mut improvements: u32 = 0;
        for (case_id, base_score) in &base_by {
            if let Some(cand_score) = cand_by.get(case_id) {
                let delta = cand_score - base_score;
                if delta < -1e-9 {
                    regressions += 1;
                } else if delta > 1e-9 {
                    improvements += 1;
                }
                per_case.push(RegressionDelta {
                    case_id: case_id.clone(),
                    baseline_score: *base_score,
                    candidate_score: *cand_score,
                    delta,
                });
            } else {
                regressions += 1;
            }
        }
        per_case.sort_by(|a, b| a.case_id.cmp(&b.case_id));
        let mean_delta = candidate.mean_score - baseline.mean_score;
        Self {
            baseline: baseline.suite.clone(),
            candidate: candidate.suite.clone(),
            mean_delta,
            regressions,
            improvements,
            per_case,
        }
    }

    /// Returns true if any case regressed.
    pub fn has_regression(&self) -> bool {
        self.regressions > 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden::CaseResult;
    use crate::judge::JudgeVerdict;

    fn verdict(score: f64) -> JudgeVerdict {
        JudgeVerdict {
            score,
            reasoning: String::new(),
            judge_model: "mock".into(),
        }
    }

    fn suite(name: &str, scores: &[(&str, f64)]) -> SuiteResult {
        let cases: Vec<CaseResult> = scores
            .iter()
            .map(|(id, s)| CaseResult {
                case_id: (*id).to_string(),
                verdict: verdict(*s),
                passed: *s >= 0.5,
            })
            .collect();
        let total = cases.len();
        let passed = cases.iter().filter(|c| c.passed).count();
        let mean = cases.iter().map(|c| c.verdict.score).sum::<f64>() / total as f64;
        SuiteResult {
            suite: name.into(),
            total,
            passed,
            mean_score: mean,
            cases,
        }
    }

    #[test]
    fn detects_regression_and_improvement() {
        let base = suite("base", &[("a", 1.0), ("b", 0.5), ("c", 0.4)]);
        let cand = suite("cand", &[("a", 1.0), ("b", 0.3), ("c", 0.9)]);
        let rep = RegressionReport::build(&base, &cand);
        assert_eq!(rep.regressions, 1);
        assert_eq!(rep.improvements, 1);
        assert!(rep.has_regression());
    }

    #[test]
    fn no_regression_when_all_equal_or_better() {
        let base = suite("base", &[("a", 0.5)]);
        let cand = suite("cand", &[("a", 0.7)]);
        let rep = RegressionReport::build(&base, &cand);
        assert!(!rep.has_regression());
        assert_eq!(rep.improvements, 1);
    }
}
