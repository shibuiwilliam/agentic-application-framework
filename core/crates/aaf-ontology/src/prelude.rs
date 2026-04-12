//! Convenience prelude — `use aaf_ontology::prelude::*;`

pub use crate::entity::{
    Classification, Entity, EntityField, EntityId, EntityRef, EntityScopePredicate, EntityVersion,
    EventRef, FieldType,
};
pub use crate::error::OntologyError;
pub use crate::lineage::{EntityRefVersioned, LineageRecord};
pub use crate::registry::{InMemoryOntologyRegistry, OntologyRegistry};
pub use crate::relation::{Cardinality, Relation, RelationKind};
pub use crate::resolver::{EntityResolver, ExactMatchResolver, ResolverOutcome};
pub use crate::version::{compare_versions, VersionCompatibility};
