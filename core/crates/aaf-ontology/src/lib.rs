//! AAF Domain Ontology Layer (Enhancement E2).
//!
//! The ontology is AAF's shared vocabulary of real-world **entities**
//! and **relations**. Every [`aaf_contracts::CapabilityContract`] that
//! touches the ontology declares which entities it `reads:`,
//! `writes:`, and `emits:`; the planner, policy engine, memory system
//! and federation layer all reason over these nouns instead of over
//! field names.
//!
//! Rules from `CLAUDE.md` enforced by this crate:
//!
//! | Rule | Where |
//! |---|---|
//! | 14 Semantics are nouns, not names | `Entity` + `CapabilityContract.reads/writes/emits` |
//! | 21 Entities are tenant-scoped by default | `EntityRef.tenant` |
//!
//! See also:
//! - [`entity`] — `Entity`, `EntityField`, `Classification`
//! - [`relation`] — `Relation`, `RelationKind`, `Cardinality`
//! - [`registry`] — `OntologyRegistry` trait + in-memory impl
//! - [`resolver`] — service-local id → global `EntityRef`
//! - [`lineage`] — entity provenance tracking
//! - [`version`] — semver-style entity version compatibility
//! - [`import`] — best-effort import from external shapes

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod entity;
pub mod error;
pub mod import;
pub mod lineage;
pub mod prelude;
pub mod registry;
pub mod relation;
pub mod resolver;
pub mod version;

pub use entity::{
    Classification, Entity, EntityField, EntityId, EntityRef, EntityScopePredicate, EntityVersion,
    EventRef, FieldType,
};
pub use error::OntologyError;
pub use lineage::{EntityRefVersioned, LineageRecord};
pub use registry::{InMemoryOntologyRegistry, OntologyRegistry};
pub use relation::{Cardinality, Relation, RelationKind};
pub use resolver::{EntityResolver, ExactMatchResolver, ResolverOutcome};
pub use version::{compare_versions, VersionCompatibility};
