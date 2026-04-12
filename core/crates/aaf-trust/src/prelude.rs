//! Convenience prelude — `use aaf_trust::prelude::*;` brings the
//! most-used types into scope.

pub use crate::autonomy::AutonomyPolicy;
pub use crate::delegation::{effective_trust, verify_token, DelegationError};
pub use crate::promotion::{PromotionDecision, PromotionRules};
pub use crate::registry::TrustRegistry;
pub use crate::score::{ScoreEvent, ScoreHistory};
pub use crate::signing::{
    sign_artifact, sign_artifact_with, verify_artifact, verify_artifact_with,
};
