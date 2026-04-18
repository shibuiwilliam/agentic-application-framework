//! Model pricing tables for cost calculation (Rule 35).
//!
//! Each provider implementation uses these tables to compute
//! `ChatResponse.cost_usd` from real token counts. The tables are
//! data, not code, and can be overridden via configuration.

use serde::{Deserialize, Serialize};

/// Pricing for one model family.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ModelPricing {
    /// Model name prefix to match (e.g. `"claude-sonnet-4"`).
    pub model_prefix: String,
    /// USD per million input tokens.
    pub input_per_mtok: f64,
    /// USD per million output tokens.
    pub output_per_mtok: f64,
}

/// Calculate cost in USD from token counts and pricing.
pub fn calculate_cost(
    pricing_table: &[ModelPricing],
    model: &str,
    input_tokens: u32,
    output_tokens: u32,
) -> f64 {
    let pricing = pricing_table
        .iter()
        .find(|p| model.starts_with(&p.model_prefix));

    match pricing {
        Some(p) => {
            (f64::from(input_tokens) * p.input_per_mtok
                + f64::from(output_tokens) * p.output_per_mtok)
                / 1_000_000.0
        }
        None => 0.0, // unknown model — zero cost, not crash
    }
}

/// Default Anthropic Claude pricing (as of 2025).
pub fn anthropic_pricing() -> Vec<ModelPricing> {
    vec![
        ModelPricing {
            model_prefix: "claude-opus-4".into(),
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
        },
        ModelPricing {
            model_prefix: "claude-sonnet-4".into(),
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        },
        ModelPricing {
            model_prefix: "claude-haiku-4".into(),
            input_per_mtok: 0.80,
            output_per_mtok: 4.0,
        },
        // Legacy models
        ModelPricing {
            model_prefix: "claude-3-5-sonnet".into(),
            input_per_mtok: 3.0,
            output_per_mtok: 15.0,
        },
        ModelPricing {
            model_prefix: "claude-3-5-haiku".into(),
            input_per_mtok: 0.80,
            output_per_mtok: 4.0,
        },
        ModelPricing {
            model_prefix: "claude-3-opus".into(),
            input_per_mtok: 15.0,
            output_per_mtok: 75.0,
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sonnet_cost_calculation() {
        let table = anthropic_pricing();
        // 1000 input + 500 output tokens with Sonnet pricing:
        // (1000 * 3.0 + 500 * 15.0) / 1_000_000 = (3000 + 7500) / 1_000_000 = 0.0105
        let cost = calculate_cost(&table, "claude-sonnet-4-6-20250514", 1000, 500);
        assert!((cost - 0.0105).abs() < 1e-9, "cost: {cost}");
    }

    #[test]
    fn haiku_cost_calculation() {
        let table = anthropic_pricing();
        let cost = calculate_cost(&table, "claude-haiku-4-5-20251001", 1000, 500);
        // (1000 * 0.80 + 500 * 4.0) / 1_000_000 = (800 + 2000) / 1_000_000 = 0.0028
        assert!((cost - 0.0028).abs() < 1e-9, "cost: {cost}");
    }

    #[test]
    fn unknown_model_returns_zero() {
        let table = anthropic_pricing();
        let cost = calculate_cost(&table, "gpt-4o-unknown", 1000, 500);
        assert!((cost).abs() < 1e-9, "unknown model should return zero");
    }

    #[test]
    fn pricing_table_round_trip() {
        let table = anthropic_pricing();
        let json = serde_json::to_string(&table).unwrap();
        let back: Vec<ModelPricing> = serde_json::from_str(&json).unwrap();
        assert_eq!(table, back);
    }
}
