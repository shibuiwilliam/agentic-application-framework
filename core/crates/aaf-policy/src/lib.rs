//! AAF policy / risk engine.
//!
//! Implements:
//! - **Rule 6** (Policy at every step) — [`engine::PolicyEngine::evaluate`]
//!   is called at four hook points by the runtime.
//! - **Rule 7** (Guard every agent) — [`guard::InputGuard`],
//!   [`guard::OutputGuard`], [`guard::ActionGuard`].
//! - The seven rule families: scope_check, side_effect_gate,
//!   budget_control, pii_guard, injection_guard, composition_safety,
//!   boundary_enforcement.
//! - [`approval`] — human approval workflow.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod approval;
pub mod context;
pub mod engine;
pub mod guard;
pub mod prelude;
pub mod rules;
pub mod scope;

pub use approval::{ApprovalRequest, ApprovalState, ApprovalWorkflow};
pub use context::{EntityClass, OntologyClassificationLookup, PolicyContext};
pub use engine::{PolicyEngine, PolicyHook};
pub use guard::{ActionGuard, InputGuard, OutputGuard};
pub use scope::{compute_effective_scopes, effective_scopes, scope_matches};
