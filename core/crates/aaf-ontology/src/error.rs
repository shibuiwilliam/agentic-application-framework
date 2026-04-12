//! Ontology errors.

use thiserror::Error;

/// Errors raised when constructing / mutating the ontology.
#[derive(Debug, Error)]
pub enum OntologyError {
    /// Entity id is missing from the registry.
    #[error("entity not found: {0}")]
    NotFound(String),

    /// Registration attempted with an incompatible version bump.
    #[error("incompatible version: {0}")]
    IncompatibleVersion(String),

    /// A declared relation references an entity that doesn't exist.
    #[error("dangling relation: {from} → {to}")]
    DanglingRelation {
        /// Source entity id.
        from: String,
        /// Destination entity id.
        to: String,
    },

    /// Classification downgrade attempted without explicit override.
    #[error("classification downgrade {from:?} → {to:?} not allowed")]
    ClassificationDowngrade {
        /// Current classification.
        from: crate::entity::Classification,
        /// Requested classification.
        to: crate::entity::Classification,
    },

    /// Cross-tenant entity access denied.
    #[error("cross-tenant access denied: tenant {caller} reached for tenant {owner}")]
    CrossTenant {
        /// Calling tenant.
        caller: String,
        /// Owning tenant.
        owner: String,
    },
}
