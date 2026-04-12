//! Saga executor.
//!
//! v0.1 ships a high-level executor that takes a [`SagaDefinition`]
//! plus a closure for running each step. Real deployments wire the
//! closure to the capability registry + graph runtime.

use crate::definition::{CompensationType, RecoveryAction, SagaDefinition, SagaStep};
use crate::recovery::{decide, RecoveryDecision};
use crate::state::SagaState;
use std::sync::Arc;
use thiserror::Error;

/// Outcome of a saga run.
#[derive(Debug, Clone, PartialEq)]
pub enum SagaOutcome {
    /// All steps completed.
    Completed {
        /// Steps that completed.
        completed: Vec<u32>,
    },
    /// Failed at a specific step.
    Failed {
        /// Step that failed.
        failed_at: u32,
        /// Recovery action taken.
        recovery: RecoveryAction,
        /// Steps whose effects were rolled back.
        compensated: Vec<u32>,
        /// Steps whose effects were intentionally preserved
        /// (partial compensation only).
        preserved: Vec<u32>,
    },
    /// Pending user input.
    PendingUser {
        /// Step that paused.
        step: u32,
    },
}

/// Errors raised by the executor.
#[derive(Debug, Error)]
pub enum SagaError {
    /// Mandatory compensation failed.
    #[error("mandatory compensation failed at step {0}")]
    MandatoryCompensationFailed(u32),

    /// Step closure returned an error other than a tagged failure.
    #[error("step {0} closure failed: {1}")]
    StepClosureFailed(u32, String),
}

/// Outcome of one step closure.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StepResult {
    /// Step succeeded.
    Ok,
    /// Step failed with the given tag (matched against the recovery
    /// rules).
    FailedWithTag(String),
}

/// Closure type the executor calls for each step.
pub type StepRunner = Arc<dyn Fn(&SagaStep) -> StepResult + Send + Sync>;
/// Closure type the executor calls for each compensation.
pub type CompensationRunner = Arc<dyn Fn(&SagaStep) -> Result<(), String> + Send + Sync>;

/// Saga executor.
pub struct SagaExecutor {
    runner: StepRunner,
    compensator: CompensationRunner,
    /// Current state (publicly observable).
    pub state: SagaState,
}

impl SagaExecutor {
    /// Construct.
    pub fn new(runner: StepRunner, compensator: CompensationRunner) -> Self {
        Self {
            runner,
            compensator,
            state: SagaState::Initiated,
        }
    }

    /// Execute a saga definition.
    pub fn run(&mut self, def: &SagaDefinition) -> Result<SagaOutcome, SagaError> {
        self.state = SagaState::Running;
        let mut completed: Vec<u32> = vec![];
        for step in &def.steps {
            match (self.runner)(step) {
                StepResult::Ok => completed.push(step.step),
                StepResult::FailedWithTag(tag) => {
                    self.state = SagaState::StepFailed;
                    self.state = SagaState::Analyzing;
                    let decision = decide(&tag, step.on_failure.as_ref());
                    self.state = SagaState::RecoverySelected;
                    let action = match &decision {
                        RecoveryDecision::Apply(rule) => rule.action,
                        RecoveryDecision::Fallback(a) => *a,
                    };
                    let preserve_set: std::collections::HashSet<u32> = match &decision {
                        RecoveryDecision::Apply(rule) => rule
                            .preserve
                            .iter()
                            .filter_map(|s| s.parse::<u32>().ok())
                            .collect(),
                        RecoveryDecision::Fallback(_) => Default::default(),
                    };
                    let mut compensated = vec![];
                    let mut preserved = vec![];
                    match action {
                        RecoveryAction::FullCompensation | RecoveryAction::PartialCompensation => {
                            self.state = if matches!(action, RecoveryAction::FullCompensation) {
                                SagaState::FullCompensation
                            } else {
                                SagaState::PartialCompensation
                            };
                            for done in completed.iter().rev() {
                                if matches!(action, RecoveryAction::PartialCompensation)
                                    && preserve_set.contains(done)
                                {
                                    preserved.push(*done);
                                    continue;
                                }
                                let done_step =
                                    def.steps.iter().find(|s| s.step == *done).expect("known");
                                if done_step.compensation.is_some()
                                    && (self.compensator)(done_step).is_err()
                                {
                                    if matches!(
                                        done_step.compensation_type,
                                        CompensationType::Mandatory
                                    ) {
                                        return Err(SagaError::MandatoryCompensationFailed(*done));
                                    }
                                } else {
                                    compensated.push(*done);
                                }
                            }
                            self.state = SagaState::Failed;
                            return Ok(SagaOutcome::Failed {
                                failed_at: step.step,
                                recovery: action,
                                compensated,
                                preserved,
                            });
                        }
                        RecoveryAction::PauseAndAskUser => {
                            self.state = SagaState::WaitingForInput;
                            return Ok(SagaOutcome::PendingUser { step: step.step });
                        }
                        RecoveryAction::RetryWithAlternative => {
                            // v0.1: just rerun once.
                            if let StepResult::Ok = (self.runner)(step) {
                                completed.push(step.step);
                                continue;
                            }
                            self.state = SagaState::Failed;
                            return Ok(SagaOutcome::Failed {
                                failed_at: step.step,
                                recovery: action,
                                compensated: vec![],
                                preserved: vec![],
                            });
                        }
                        RecoveryAction::Skip => {
                            continue;
                        }
                    }
                }
            }
        }
        self.state = SagaState::Completed;
        Ok(SagaOutcome::Completed { completed })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::{OnFailure, RecoveryRule, StepKind};

    fn step(n: u32, name: &str, on_failure: Option<OnFailure>) -> SagaStep {
        SagaStep {
            step: n,
            name: name.into(),
            kind: StepKind::Deterministic,
            capability: format!("cap-{n}"),
            compensation: Some(format!("comp-{n}")),
            compensation_type: CompensationType::Mandatory,
            on_failure,
        }
    }

    #[test]
    fn happy_path_completes_all_steps() {
        let def = SagaDefinition {
            name: "x".into(),
            description: None,
            steps: vec![step(1, "a", None), step(2, "b", None)],
        };
        let runner: StepRunner = Arc::new(|_| StepResult::Ok);
        let comp: CompensationRunner = Arc::new(|_| Ok(()));
        let mut e = SagaExecutor::new(runner, comp);
        let outcome = e.run(&def).unwrap();
        assert_eq!(
            outcome,
            SagaOutcome::Completed {
                completed: vec![1, 2]
            }
        );
        assert_eq!(e.state, SagaState::Completed);
    }

    #[test]
    fn pause_and_ask_user_short_circuits() {
        let of = OnFailure {
            strategy: "intelligent".into(),
            rules: vec![RecoveryRule {
                condition: "address_invalid".into(),
                action: RecoveryAction::PauseAndAskUser,
                preserve: vec!["1".into()],
            }],
        };
        let def = SagaDefinition {
            name: "x".into(),
            description: None,
            steps: vec![step(1, "a", None), step(2, "ship", Some(of))],
        };
        let runner: StepRunner = Arc::new(|s| {
            if s.step == 2 {
                StepResult::FailedWithTag("address_invalid".into())
            } else {
                StepResult::Ok
            }
        });
        let comp: CompensationRunner = Arc::new(|_| Ok(()));
        let mut e = SagaExecutor::new(runner, comp);
        let outcome = e.run(&def).unwrap();
        assert_eq!(outcome, SagaOutcome::PendingUser { step: 2 });
        assert_eq!(e.state, SagaState::WaitingForInput);
    }

    #[test]
    fn full_compensation_runs_in_reverse_order() {
        let def = SagaDefinition {
            name: "x".into(),
            description: None,
            steps: vec![step(1, "a", None), step(2, "b", None), step(3, "c", None)],
        };
        let runner: StepRunner = Arc::new(|s| {
            if s.step == 3 {
                StepResult::FailedWithTag("oops".into())
            } else {
                StepResult::Ok
            }
        });
        let comp: CompensationRunner = Arc::new(|_| Ok(()));
        let mut e = SagaExecutor::new(runner, comp);
        let outcome = e.run(&def).unwrap();
        match outcome {
            SagaOutcome::Failed {
                failed_at,
                recovery,
                compensated,
                preserved,
            } => {
                assert_eq!(failed_at, 3);
                assert_eq!(recovery, RecoveryAction::FullCompensation);
                assert_eq!(compensated, vec![2, 1]);
                assert!(preserved.is_empty());
            }
            _ => panic!("expected failed"),
        }
    }
}
