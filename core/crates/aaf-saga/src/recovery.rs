//! Failure analysis & recovery selection.

use crate::definition::{OnFailure, RecoveryAction, RecoveryRule};

/// Result of analysing a failure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RecoveryDecision {
    /// Apply this rule.
    Apply(RecoveryRule),
    /// No matching rule — fall back to full compensation.
    Fallback(RecoveryAction),
}

/// Decide a recovery for a tagged failure given the policy attached to
/// the step.
pub fn decide(failure_tag: &str, on_failure: Option<&OnFailure>) -> RecoveryDecision {
    if let Some(of) = on_failure {
        for rule in &of.rules {
            if rule.condition == failure_tag {
                return RecoveryDecision::Apply(rule.clone());
            }
        }
    }
    RecoveryDecision::Fallback(RecoveryAction::FullCompensation)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::OnFailure;

    fn of() -> OnFailure {
        OnFailure {
            strategy: "intelligent_recovery".into(),
            rules: vec![
                RecoveryRule {
                    condition: "address_invalid".into(),
                    action: RecoveryAction::PauseAndAskUser,
                    preserve: vec!["1".into(), "2".into()],
                },
                RecoveryRule {
                    condition: "carrier_temporary_outage".into(),
                    action: RecoveryAction::RetryWithAlternative,
                    preserve: vec![],
                },
            ],
        }
    }

    #[test]
    fn matches_first_rule() {
        let d = decide("address_invalid", Some(&of()));
        match d {
            RecoveryDecision::Apply(r) => assert_eq!(r.action, RecoveryAction::PauseAndAskUser),
            _ => panic!(),
        }
    }

    #[test]
    fn falls_back_to_full_compensation() {
        let d = decide("unknown_tag", Some(&of()));
        assert!(matches!(
            d,
            RecoveryDecision::Fallback(RecoveryAction::FullCompensation)
        ));
    }
}
