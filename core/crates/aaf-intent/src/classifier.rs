//! Intent type classifier.
//!
//! v0.1 ships a deterministic, rule-based classifier so the rest of the
//! pipeline is testable end-to-end. The trait shape lets a future
//! iteration drop in an LLM classifier without changing call sites.

use aaf_contracts::IntentType;

/// Pluggable classifier surface.
pub trait Classifier: Send + Sync {
    /// Best-effort classification.
    fn classify(&self, input: &str) -> Option<IntentType>;
}

/// Rule-based classifier driven by keyword cues. The keywords here are
/// intentionally Japanese + English so the system handles the bilingual
/// examples in `PROJECT.md`.
pub struct RuleClassifier;

impl Classifier for RuleClassifier {
    fn classify(&self, input: &str) -> Option<IntentType> {
        let lower = input.to_lowercase();
        let signals = [
            (
                IntentType::TransactionalIntent,
                &[
                    "cancel",
                    "create",
                    "update",
                    "delete",
                    "send",
                    "pay",
                    "キャンセル",
                    "更新",
                    "作成",
                    "削除",
                    "送信",
                    "支払",
                ][..],
            ),
            (
                IntentType::AnalyticalIntent,
                &[
                    "show",
                    "report",
                    "analy",
                    "trend",
                    "compare",
                    "見たい",
                    "教えて",
                    "分析",
                    "比較",
                    "売上",
                    "推移",
                ][..],
            ),
            (
                IntentType::PlanningIntent,
                &[
                    "plan",
                    "draft",
                    "design",
                    "計画",
                    "企画",
                    "立てたい",
                    "ロードマップ",
                ][..],
            ),
            (
                IntentType::DelegationIntent,
                &["ask", "delegate", "hand off", "依頼", "委譲", "お願い"][..],
            ),
            (
                IntentType::GovernanceIntent,
                &[
                    "policy",
                    "permission",
                    "approval limit",
                    "issue key",
                    "ポリシー",
                    "承認",
                    "権限",
                    "発行",
                ][..],
            ),
        ];
        for (kind, keys) in signals {
            for k in keys {
                if lower.contains(k) {
                    return Some(kind);
                }
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classifies_japanese_examples_from_spec() {
        let c = RuleClassifier;
        assert_eq!(
            c.classify("この注文をキャンセルして"),
            Some(IntentType::TransactionalIntent)
        );
        assert_eq!(
            c.classify("先月の売上を地域別に見たい"),
            Some(IntentType::AnalyticalIntent)
        );
        assert_eq!(
            c.classify("来期の採用計画を立てたい"),
            Some(IntentType::PlanningIntent)
        );
        assert_eq!(
            c.classify("法務チームに契約書レビューを依頼して"),
            Some(IntentType::DelegationIntent)
        );
        assert_eq!(
            c.classify("経費承認の上限を変更したい"),
            Some(IntentType::GovernanceIntent)
        );
    }

    #[test]
    fn unrecognised_input_returns_none() {
        assert_eq!(RuleClassifier.classify("zzzz"), None);
    }
}
