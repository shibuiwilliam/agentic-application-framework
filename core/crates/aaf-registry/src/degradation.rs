//! Degradation level state machine for a single capability.

use aaf_contracts::DegradationLevel;

/// One transition.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DegradationTransition {
    /// State the capability moved from.
    pub from: DegradationLevel,
    /// State the capability moved to.
    pub to: DegradationLevel,
}

/// State machine wrapping a single capability's current degradation
/// level. Transitions monotonically descend (Full → Partial → Cached →
/// Unavailable) on failure and ascend on recovery.
#[derive(Debug, Clone, Copy)]
pub struct DegradationStateMachine {
    current: DegradationLevel,
}

impl Default for DegradationStateMachine {
    fn default() -> Self {
        Self {
            current: DegradationLevel::Full,
        }
    }
}

impl DegradationStateMachine {
    /// Construct in `Full`.
    pub fn new() -> Self {
        Self::default()
    }

    /// Current level.
    pub fn current(&self) -> DegradationLevel {
        self.current
    }

    /// Degrade one step. Returns the transition or `None` if already at
    /// `Unavailable`.
    pub fn degrade(&mut self) -> Option<DegradationTransition> {
        let next = match self.current {
            DegradationLevel::Full => DegradationLevel::Partial,
            DegradationLevel::Partial => DegradationLevel::Cached,
            DegradationLevel::Cached => DegradationLevel::Unavailable,
            DegradationLevel::Unavailable => return None,
        };
        let from = self.current;
        self.current = next;
        Some(DegradationTransition { from, to: next })
    }

    /// Recover one step. Returns the transition or `None` if already
    /// `Full`.
    pub fn recover(&mut self) -> Option<DegradationTransition> {
        let next = match self.current {
            DegradationLevel::Full => return None,
            DegradationLevel::Partial => DegradationLevel::Full,
            DegradationLevel::Cached => DegradationLevel::Partial,
            DegradationLevel::Unavailable => DegradationLevel::Cached,
        };
        let from = self.current;
        self.current = next;
        Some(DegradationTransition { from, to: next })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn degrades_step_by_step() {
        let mut sm = DegradationStateMachine::new();
        assert_eq!(sm.degrade().unwrap().to, DegradationLevel::Partial);
        assert_eq!(sm.degrade().unwrap().to, DegradationLevel::Cached);
        assert_eq!(sm.degrade().unwrap().to, DegradationLevel::Unavailable);
        assert!(sm.degrade().is_none());
    }

    #[test]
    fn recovers_step_by_step() {
        let mut sm = DegradationStateMachine::new();
        sm.degrade();
        sm.degrade();
        assert_eq!(sm.recover().unwrap().to, DegradationLevel::Partial);
        assert_eq!(sm.recover().unwrap().to, DegradationLevel::Full);
        assert!(sm.recover().is_none());
    }
}
