//! AAF trust subsystem.
//!
//! Implements **Trust min propagation** (security checklist) and the
//! **5-level autonomy ladder** (P5 — Trust is Earned, Not Granted).
//!
//! Components:
//! - [`score`] — score arithmetic + history
//! - [`autonomy`] — promotion / demotion thresholds
//! - [`delegation`] — `min(delegator, delegatee)` propagation
//! - [`promotion`] — promote / demote rules
//! - [`signing`] — content-hash + detached signature for artifacts

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod autonomy;
pub mod delegation;
pub mod prelude;
pub mod promotion;
pub mod registry;
pub mod score;
pub mod signing;

pub use autonomy::AutonomyPolicy;
pub use delegation::{effective_trust, verify_token, DelegationError};
pub use promotion::{PromotionDecision, PromotionRules};
pub use registry::TrustRegistry;
pub use score::{ScoreEvent, ScoreHistory};
pub use signing::{sign_artifact, sign_artifact_with, verify_artifact, verify_artifact_with};
