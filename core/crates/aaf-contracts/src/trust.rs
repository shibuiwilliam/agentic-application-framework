//! Trust contract — score and autonomy classifications. The actual
//! score-update and delegation logic lives in `aaf-trust`.

use serde::{Deserialize, Serialize};

/// 5-level autonomy classification (Rule 8 / §3.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AutonomyLevel {
    /// All operations require human approval.
    Level1,
    /// Reads autonomous, writes require approval.
    Level2,
    /// Low-risk writes autonomous, high-risk requires approval.
    Level3,
    /// Approval only on detected anomaly.
    Level4,
    /// Fully autonomous (audit-only).
    Level5,
}

impl AutonomyLevel {
    /// Numeric value 1..=5.
    pub fn as_u8(self) -> u8 {
        match self {
            AutonomyLevel::Level1 => 1,
            AutonomyLevel::Level2 => 2,
            AutonomyLevel::Level3 => 3,
            AutonomyLevel::Level4 => 4,
            AutonomyLevel::Level5 => 5,
        }
    }

    /// Construct from a number, clamping to the valid range.
    pub fn from_u8(n: u8) -> Self {
        match n {
            0 | 1 => AutonomyLevel::Level1,
            2 => AutonomyLevel::Level2,
            3 => AutonomyLevel::Level3,
            4 => AutonomyLevel::Level4,
            _ => AutonomyLevel::Level5,
        }
    }
}

/// Trust score with associated autonomy level. Range `[0.0, 1.0]`.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct TrustScore {
    /// Numerical score in `[0.0, 1.0]`.
    pub value: f64,
    /// Autonomy bracket the score corresponds to.
    pub autonomy: AutonomyLevel,
}

impl TrustScore {
    /// Initial trust for a new agent: `0.5` at `Level1` per the
    /// "Trust is earned" principle (P5).
    pub fn initial() -> Self {
        Self {
            value: 0.5,
            autonomy: AutonomyLevel::Level1,
        }
    }

    /// Clamp the score into [0,1].
    pub fn clamped(value: f64) -> f64 {
        if value.is_nan() {
            0.0
        } else {
            value.max(0.0).min(1.0)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autonomy_round_trips() {
        for n in 1u8..=5 {
            assert_eq!(AutonomyLevel::from_u8(n).as_u8(), n);
        }
    }

    #[test]
    fn clamp_handles_extremes() {
        assert!((TrustScore::clamped(-1.0) - 0.0).abs() < f64::EPSILON);
        assert!((TrustScore::clamped(2.0) - 1.0).abs() < f64::EPSILON);
        assert!((TrustScore::clamped(f64::NAN) - 0.0).abs() < f64::EPSILON);
    }
}
