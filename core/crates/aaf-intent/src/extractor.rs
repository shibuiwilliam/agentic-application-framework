//! Constraint / entity extraction.

use std::collections::BTreeMap;

/// Trait surface for extractors. The extractor populates the
/// `constraints` map of an [`aaf_contracts::IntentEnvelope`].
pub trait Extractor: Send + Sync {
    /// Extract constraints from `input`.
    fn extract(&self, input: &str) -> BTreeMap<String, serde_json::Value>;
}

/// Lightweight regex-based extractor: pulls obvious time periods,
/// regions, and quantity constraints from the input. The patterns are
/// deliberately conservative to keep the v0.1 deterministic.
pub struct RuleExtractor;

impl Extractor for RuleExtractor {
    fn extract(&self, input: &str) -> BTreeMap<String, serde_json::Value> {
        use once_cell::sync::Lazy;
        use regex::Regex;

        static PERIOD_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(?i)(last|previous|先)(月|month|week)").expect("re"));
        static REGION_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"(?i)(地域|region|country|地区)").expect("re"));
        static SKU_RE: Lazy<Regex> =
            Lazy::new(|| Regex::new(r"\b(SKU|sku)[-_:\s]?[A-Za-z0-9]+\b").expect("re"));
        static AMOUNT_RE: Lazy<Regex> = Lazy::new(|| {
            Regex::new(r"\b\d{1,3}(?:,\d{3})*(?:\.\d+)?\s*(円|usd|jpy)\b").expect("re")
        });

        let mut out = BTreeMap::new();
        if PERIOD_RE.is_match(input) {
            out.insert("period_ref".into(), serde_json::json!("last_month"));
        }
        if REGION_RE.is_match(input) {
            out.insert("dimension".into(), serde_json::json!("region"));
        }
        if let Some(m) = SKU_RE.find(input) {
            out.insert("sku".into(), serde_json::json!(m.as_str()));
        }
        if let Some(m) = AMOUNT_RE.find(input) {
            out.insert("amount".into(), serde_json::json!(m.as_str()));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_period_and_region() {
        let m = RuleExtractor.extract("先月の売上を地域別に見たい");
        assert_eq!(m.get("period_ref"), Some(&serde_json::json!("last_month")));
        assert_eq!(m.get("dimension"), Some(&serde_json::json!("region")));
    }
}
