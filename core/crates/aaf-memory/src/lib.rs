//! AAF memory subsystem.
//!
//! Implements the four-layer memory model from `PROJECT.md` §3.6:
//!
//! 1. **Working** — per-task transient state
//! 2. **Thread** — per-conversation continuation
//! 3. **Long-term** — semantic / episodic / procedural knowledge
//! 4. **Artifact** — produced outputs with provenance (lives in `aaf-storage`)
//!
//! On top of those four storage backends sits a [`ContextBudget`] manager
//! that enforces **Rule 10 — Context Minimization** (~7,500 tokens per LLM
//! call).

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod context;
pub mod facade;
pub mod prelude;

pub use context::{ContextBudget, ContextSection, DEFAULT_TOTAL_BUDGET};
pub use facade::MemoryFacade;
