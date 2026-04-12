//! AAF Graph Runtime.
//!
//! Executes a DAG of [`node`] instances against a single
//! [`aaf_contracts::IntentEnvelope`] while enforcing:
//!
//! - **Rule 5** — deterministic-only nodes never invoke an LLM
//! - **Rule 6** — every step passes through the policy engine at four
//!   hooks (pre-plan, pre-step, post-step, pre-artifact)
//! - **Rule 8** — every step decrements `BudgetTracker` and the runtime
//!   gracefully terminates with partial results when exhausted
//! - **Rule 12** — every step emits an `Observation` via the recorder

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod budget;
pub mod checkpoint;
pub mod compensation;
pub mod error;
pub mod executor;
pub mod graph;
pub mod node;
pub mod prelude;
pub mod scheduler;
pub mod timeout;

pub use budget::{BudgetTracker, BudgetTrackerError};
pub use checkpoint::CheckpointWriter;
pub use compensation::CompensationChain;
pub use error::RuntimeError;
pub use executor::{ExecutionOutcome, GraphExecutor};
pub use graph::{Graph, GraphBuilder, GraphValidationError};
pub use node::{Node, NodeKind, NodeOutput};
