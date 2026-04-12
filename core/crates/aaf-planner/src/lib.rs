//! Planner / Router.
//!
//! - **Communication pattern classification** — every request is sorted
//!   into one of four patterns (Fast Path / Agent Assisted / Full
//!   Agentic / Choreography). Rule 4 mandates checking Fast Path first.
//! - **Fast path** — local rule evaluation that avoids the LLM.
//! - **Planner** — turns an intent into an [`ExecutionPlan`].
//! - **Bounded autonomy** — refuses plans that exceed depth / step /
//!   budget caps.
//! - **Composition safety** — validates that the chosen capability mix
//!   does not exceed the configured emergent-risk budget.
//! - **Plan cache** — semantic-hash cached execution plans.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod bounds;
pub mod cache;
pub mod composition;
pub mod fast_path;
pub mod plan;
pub mod planner;
pub mod router;

pub use bounds::{BoundedAutonomy, BoundsViolation};
pub use cache::PlanCache;
pub use composition::{
    ClassificationHint, ClassificationLookup, CompositionChecker, CompositionViolation,
    EntityAwareComposition,
};
pub use fast_path::{FastPathOutcome, FastPathRule, FastPathRuleSet};
pub use plan::{ExecutionPlan, PlannedStep, PlannedStepKind};
pub use planner::{PlannerError, RegistryPlanner};
pub use router::{CommunicationPattern, Router};
