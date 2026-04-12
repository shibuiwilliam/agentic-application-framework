//! Saga definition — the parsed YAML form.

use serde::{Deserialize, Serialize};

/// Step type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StepKind {
    /// Deterministic / API call.
    Deterministic,
    /// Agent / LLM-driven.
    Agent,
    /// Human approval gate.
    Approval,
}

/// Compensation requirement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CompensationType {
    /// Must run on rollback or the engine surfaces a failure.
    Mandatory,
    /// Best-effort.
    BestEffort,
    /// No compensation needed.
    None,
}

impl Default for CompensationType {
    fn default() -> Self {
        Self::BestEffort
    }
}

/// One recovery rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoveryRule {
    /// Free-text condition tag (matched verbatim against
    /// `failure_tag`).
    pub condition: String,
    /// Action to take.
    pub action: RecoveryAction,
    /// Steps whose results to keep.
    #[serde(default)]
    pub preserve: Vec<String>,
}

/// Recovery action.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RecoveryAction {
    /// Pause and route to user.
    PauseAndAskUser,
    /// Retry the failed step against an alternate provider.
    RetryWithAlternative,
    /// Roll back only the failed step's effects.
    PartialCompensation,
    /// Roll back the whole saga.
    FullCompensation,
    /// Skip the step and continue.
    Skip,
}

/// Failure policy attached to a step.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OnFailure {
    /// Strategy name.
    pub strategy: String,
    /// Match rules in order — first match wins.
    #[serde(default)]
    pub rules: Vec<RecoveryRule>,
}

/// One step in the saga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaStep {
    /// Step number, 1-based.
    pub step: u32,
    /// Display name.
    pub name: String,
    /// Step type.
    #[serde(rename = "type")]
    pub kind: StepKind,
    /// Capability id.
    pub capability: String,
    /// Optional compensating capability id.
    #[serde(default)]
    pub compensation: Option<String>,
    /// Mandatory / best-effort.
    #[serde(default)]
    pub compensation_type: CompensationType,
    /// Failure policy.
    #[serde(default)]
    pub on_failure: Option<OnFailure>,
}

/// A whole saga.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SagaDefinition {
    /// Display name.
    pub name: String,
    /// Description.
    #[serde(default)]
    pub description: Option<String>,
    /// Steps in execution order.
    pub steps: Vec<SagaStep>,
}

impl SagaDefinition {
    /// Parse from YAML.
    pub fn from_yaml(yaml: &str) -> Result<Self, serde_yaml::Error> {
        serde_yaml::from_str(yaml)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
name: order-processing
steps:
  - step: 1
    name: 在庫予約
    type: deterministic
    capability: cap-stock-reserve
    compensation: cap-stock-release
    compensation_type: mandatory
  - step: 2
    name: 決済実行
    type: deterministic
    capability: cap-payment-execute
    compensation: cap-payment-refund
    compensation_type: mandatory
"#;

    #[test]
    fn parses_yaml() {
        let s = SagaDefinition::from_yaml(SAMPLE).unwrap();
        assert_eq!(s.steps.len(), 2);
        assert_eq!(s.steps[0].name, "在庫予約");
        assert_eq!(s.steps[1].compensation_type, CompensationType::Mandatory);
    }
}
