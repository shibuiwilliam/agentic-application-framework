//! Intent Compiler.
//!
//! Pipeline:
//!
//! 1. [`classifier`] — natural language → [`IntentType`]
//! 2. [`extractor`] — pull constraints / entities into the envelope
//! 3. [`enricher`] — fill blanks from memory + user profile
//! 4. [`refinement`] — emit clarification questions for missing fields
//! 5. [`cache`] — semantic-hash cache for repeated intents
//! 6. [`compiler`] — orchestrates all of the above
//!
//! All steps are pure (no LLM calls in v0.1) so the pipeline is
//! deterministic and trivially testable. A real LLM-backed compiler can
//! be slotted in by replacing the [`classifier::Classifier`] /
//! [`extractor::Extractor`] traits.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod cache;
pub mod classifier;
pub mod compiler;
pub mod enricher;
pub mod error;
pub mod extractor;
pub mod prelude;
pub mod refinement;
pub mod versioning;

pub use aaf_contracts::IntentType;
pub use cache::IntentCache;
pub use classifier::{Classifier, RuleClassifier};
pub use compiler::IntentCompiler;
pub use enricher::{Enricher, OntologyResolver};
pub use error::IntentError;
pub use extractor::{Extractor, RuleExtractor};
pub use refinement::{ClarificationQuestion, Refiner};
