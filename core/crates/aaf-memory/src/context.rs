//! Context budget manager.
//!
//! `~7,500 tokens` per LLM call broken down into five sections:
//!
//! ```text
//!  System prompt + policy : 2,000
//!  Intent envelope        :   500
//!  Memory (retrieved)     : 2,000
//!  Step context           : 1,000
//!  Tool results           : 2,000
//! ```
//!
//! [`ContextBudget::fit`] truncates section content so that the total
//! does not exceed the budget; tokens are approximated using
//! `len_chars / 4` (a generally-accurate heuristic across LLM tokenisers).

use serde::{Deserialize, Serialize};

/// Default total budget — Rule 10.
pub const DEFAULT_TOTAL_BUDGET: usize = 7_500;

/// Five labelled sections used to portion the budget.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ContextSection {
    /// System prompt + policy preamble.
    System,
    /// The intent envelope.
    Intent,
    /// Retrieved memory (long-term hits, thread snippets).
    Memory,
    /// Current step context.
    Step,
    /// Tool / capability results.
    Tools,
}

/// Per-section caps. Default values come from `PROJECT.md` §3.6.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct ContextBudget {
    /// Total budget — informational; per-section caps are authoritative.
    pub total: usize,
    /// Cap on the system section.
    pub system: usize,
    /// Cap on the intent section.
    pub intent: usize,
    /// Cap on the memory section.
    pub memory: usize,
    /// Cap on the step section.
    pub step: usize,
    /// Cap on the tools section.
    pub tools: usize,
}

impl Default for ContextBudget {
    fn default() -> Self {
        Self {
            total: DEFAULT_TOTAL_BUDGET,
            system: 2_000,
            intent: 500,
            memory: 2_000,
            step: 1_000,
            tools: 2_000,
        }
    }
}

impl ContextBudget {
    /// Approximate token count for a string (chars / 4).
    pub fn approx_tokens(text: &str) -> usize {
        (text.chars().count() + 3) / 4
    }

    /// Truncate `text` so its approximate token count fits within `cap`.
    pub fn truncate(text: &str, cap_tokens: usize) -> String {
        let max_chars = cap_tokens.saturating_mul(4);
        if text.chars().count() <= max_chars {
            text.to_string()
        } else {
            text.chars().take(max_chars).collect()
        }
    }

    /// Apply the per-section cap that matches `section` to `text`.
    pub fn fit(&self, section: ContextSection, text: &str) -> String {
        let cap = match section {
            ContextSection::System => self.system,
            ContextSection::Intent => self.intent,
            ContextSection::Memory => self.memory,
            ContextSection::Step => self.step,
            ContextSection::Tools => self.tools,
        };
        Self::truncate(text, cap)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approx_token_count_is_chars_div_4() {
        assert_eq!(ContextBudget::approx_tokens("abcdefgh"), 2);
        assert_eq!(ContextBudget::approx_tokens(""), 0);
    }

    #[test]
    fn truncate_respects_cap() {
        let s = "x".repeat(8000);
        let out = ContextBudget::truncate(&s, 100); // 400 chars
        assert_eq!(out.len(), 400);
    }

    #[test]
    fn fit_uses_section_cap() {
        let b = ContextBudget::default();
        let intent_text = "y".repeat(10_000);
        let fitted = b.fit(ContextSection::Intent, &intent_text);
        assert_eq!(fitted.len(), b.intent * 4);
    }
}
