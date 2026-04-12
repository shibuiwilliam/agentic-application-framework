//! LLM-as-judge trait + deterministic mock.
//!
//! Real deployments wire a `ClaudeJudge` / `OpenAIJudge` implementation.
//! Tests and Slice A ship [`DeterministicJudge`] which scores solely
//! on surface token overlap so test outcomes are stable.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};

/// Verdict returned by a judge.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct JudgeVerdict {
    /// Score in `[0.0, 1.0]`.
    pub score: f64,
    /// Optional free-text reasoning from the judge.
    pub reasoning: String,
    /// Model id that produced the verdict.
    pub judge_model: String,
}

/// Pluggable judge trait. `expected` is the reference answer from a
/// golden suite or a previous baseline; `actual` is the candidate
/// output being evaluated.
#[async_trait]
pub trait Judge: Send + Sync {
    /// Judge model identifier shown in reports.
    fn name(&self) -> &str;

    /// Return a verdict.
    async fn judge(&self, expected: &str, actual: &str) -> JudgeVerdict;
}

/// Deterministic judge — scores on token-level Jaccard similarity.
///
/// Given the same inputs it always returns the same score, so eval
/// runs are reproducible in CI without any external state.
pub struct DeterministicJudge {
    /// Identifier advertised to reports.
    pub label: String,
}

impl Default for DeterministicJudge {
    fn default() -> Self {
        Self {
            label: "deterministic-jaccard".into(),
        }
    }
}

#[async_trait]
impl Judge for DeterministicJudge {
    fn name(&self) -> &str {
        &self.label
    }

    async fn judge(&self, expected: &str, actual: &str) -> JudgeVerdict {
        let e: std::collections::HashSet<_> = tokens(expected);
        let a: std::collections::HashSet<_> = tokens(actual);
        let score = if e.is_empty() && a.is_empty() {
            1.0
        } else {
            let inter = e.intersection(&a).count() as f64;
            let union = e.union(&a).count() as f64;
            if union == 0.0 {
                0.0
            } else {
                inter / union
            }
        };
        let reasoning = format!(
            "jaccard over {} expected / {} actual tokens",
            e.len(),
            a.len()
        );
        JudgeVerdict {
            score,
            reasoning,
            judge_model: self.label.clone(),
        }
    }
}

fn tokens(s: &str) -> std::collections::HashSet<String> {
    s.split(|c: char| !c.is_alphanumeric())
        .filter(|t| !t.is_empty())
        .map(|t| t.to_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn identical_strings_score_one() {
        let j = DeterministicJudge::default();
        let v = j.judge("hello world", "hello world").await;
        assert!((v.score - 1.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn disjoint_strings_score_zero() {
        let j = DeterministicJudge::default();
        let v = j.judge("alpha", "omega").await;
        assert!((v.score - 0.0).abs() < 1e-9);
    }

    #[tokio::test]
    async fn partial_overlap_is_jaccard() {
        let j = DeterministicJudge::default();
        let v = j.judge("a b c d", "a b e").await;
        // intersection {a,b} = 2, union {a,b,c,d,e} = 5 → 0.4
        assert!((v.score - 0.4).abs() < 1e-9);
    }
}
