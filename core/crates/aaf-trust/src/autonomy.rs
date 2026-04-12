//! Autonomy-bracket policy: score → AutonomyLevel.

use aaf_contracts::AutonomyLevel;

/// Default thresholds derived from PROJECT.md §3.8 promotion criteria.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AutonomyPolicy {
    /// Score required for L2.
    pub l2: f64,
    /// Score required for L3.
    pub l3: f64,
    /// Score required for L4.
    pub l4: f64,
    /// Score required for L5.
    pub l5: f64,
}

impl Default for AutonomyPolicy {
    fn default() -> Self {
        Self {
            l2: 0.55,
            l3: 0.7,
            l4: 0.85,
            l5: 0.95,
        }
    }
}

impl AutonomyPolicy {
    /// Resolve a numeric score to an [`AutonomyLevel`].
    pub fn level_for(self, score: f64) -> AutonomyLevel {
        if score >= self.l5 {
            AutonomyLevel::Level5
        } else if score >= self.l4 {
            AutonomyLevel::Level4
        } else if score >= self.l3 {
            AutonomyLevel::Level3
        } else if score >= self.l2 {
            AutonomyLevel::Level2
        } else {
            AutonomyLevel::Level1
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boundary_values_resolve_correctly() {
        let p = AutonomyPolicy::default();
        assert_eq!(p.level_for(0.0), AutonomyLevel::Level1);
        assert_eq!(p.level_for(0.55), AutonomyLevel::Level2);
        assert_eq!(p.level_for(0.7), AutonomyLevel::Level3);
        assert_eq!(p.level_for(0.85), AutonomyLevel::Level4);
        assert_eq!(p.level_for(0.95), AutonomyLevel::Level5);
        assert_eq!(p.level_for(1.0), AutonomyLevel::Level5);
    }
}
