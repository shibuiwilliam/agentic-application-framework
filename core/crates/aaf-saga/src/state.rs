//! Saga state machine.

use serde::{Deserialize, Serialize};

/// Discrete states of a saga execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SagaState {
    /// Just created.
    Initiated,
    /// Currently running.
    Running,
    /// Step N failed.
    StepFailed,
    /// Failure analysis underway.
    Analyzing,
    /// Recovery action selected.
    RecoverySelected,
    /// Partial compensation in progress.
    PartialCompensation,
    /// Awaiting user input.
    WaitingForInput,
    /// Resumed after a pause.
    Resumed,
    /// Full compensation in progress.
    FullCompensation,
    /// Saga failed.
    Failed,
    /// Saga completed.
    Completed,
}

impl SagaState {
    /// Whether the state is terminal.
    pub fn is_terminal(self) -> bool {
        matches!(self, SagaState::Completed | SagaState::Failed)
    }
}
