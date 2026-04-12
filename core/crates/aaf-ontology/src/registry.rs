//! Ontology registry.

use crate::entity::{Entity, EntityId};
use crate::error::OntologyError;
use crate::version::{compare_versions, VersionCompatibility};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Persistence boundary for the ontology.
#[async_trait]
pub trait OntologyRegistry: Send + Sync {
    /// Insert or replace an entity. Enforces classification-downgrade
    /// and breaking-version rules unless explicitly told to accept
    /// breaking bumps.
    async fn upsert(&self, entity: Entity, allow_breaking: bool) -> Result<(), OntologyError>;

    /// Fetch an entity by id.
    async fn get(&self, id: &EntityId) -> Result<Entity, OntologyError>;

    /// List every known entity (tests + tools).
    async fn list(&self) -> Result<Vec<Entity>, OntologyError>;

    /// Remove an entity entirely. Used by migration tools.
    async fn remove(&self, id: &EntityId) -> Result<(), OntologyError>;
}

/// In-memory backend.
#[derive(Default)]
pub struct InMemoryOntologyRegistry {
    inner: Arc<RwLock<HashMap<EntityId, Entity>>>,
}

impl InMemoryOntologyRegistry {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl OntologyRegistry for InMemoryOntologyRegistry {
    async fn upsert(&self, entity: Entity, allow_breaking: bool) -> Result<(), OntologyError> {
        // Validate relations point at known (or self) entities.
        {
            let guard = self.inner.read();
            for rel in &entity.relations {
                let known = rel.to == entity.id || guard.contains_key(&rel.to);
                if !known {
                    return Err(OntologyError::DanglingRelation {
                        from: rel.from.clone(),
                        to: rel.to.clone(),
                    });
                }
            }
        }

        // Enforce classification-downgrade and version compatibility.
        {
            let guard = self.inner.read();
            if let Some(existing) = guard.get(&entity.id) {
                if !existing
                    .classification
                    .can_flow_into(&entity.classification)
                {
                    return Err(OntologyError::ClassificationDowngrade {
                        from: existing.classification.clone(),
                        to: entity.classification.clone(),
                    });
                }
                let compat = compare_versions(existing.version, entity.version);
                if matches!(compat, VersionCompatibility::Breaking) && !allow_breaking {
                    return Err(OntologyError::IncompatibleVersion(format!(
                        "{} → {} is breaking (set allow_breaking=true to override)",
                        existing.version, entity.version
                    )));
                }
            }
        }

        self.inner.write().insert(entity.id.clone(), entity);
        Ok(())
    }

    async fn get(&self, id: &EntityId) -> Result<Entity, OntologyError> {
        self.inner
            .read()
            .get(id)
            .cloned()
            .ok_or_else(|| OntologyError::NotFound(id.clone()))
    }

    async fn list(&self) -> Result<Vec<Entity>, OntologyError> {
        Ok(self.inner.read().values().cloned().collect())
    }

    async fn remove(&self, id: &EntityId) -> Result<(), OntologyError> {
        self.inner.write().remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::entity::{Classification, EntityField, EntityVersion, FieldType};
    use crate::relation::{Cardinality, Relation, RelationKind};

    fn order() -> Entity {
        let mut e = Entity::new("commerce.Order", "order", Classification::Internal);
        e.fields.push(EntityField {
            name: "id".into(),
            field_type: FieldType::String,
            required: true,
            classification: None,
            description: String::new(),
        });
        e
    }

    #[tokio::test]
    async fn upsert_and_get_round_trip() {
        let r = InMemoryOntologyRegistry::new();
        r.upsert(order(), false).await.unwrap();
        let back = r.get(&"commerce.Order".to_string()).await.unwrap();
        assert_eq!(back.id, "commerce.Order");
    }

    #[tokio::test]
    async fn dangling_relation_is_rejected() {
        let r = InMemoryOntologyRegistry::new();
        let mut o = order();
        o.relations.push(Relation::new(
            RelationKind::References,
            "commerce.Order",
            "commerce.Customer",
            Cardinality::ExactlyOne,
        ));
        let err = r.upsert(o, false).await.unwrap_err();
        assert!(matches!(err, OntologyError::DanglingRelation { .. }));
    }

    #[tokio::test]
    async fn self_reference_is_allowed() {
        let r = InMemoryOntologyRegistry::new();
        let mut o = order();
        o.relations.push(Relation::new(
            RelationKind::References,
            "commerce.Order",
            "commerce.Order",
            Cardinality::ZeroOrOne,
        ));
        r.upsert(o, false).await.unwrap();
    }

    #[tokio::test]
    async fn classification_downgrade_rejected() {
        let r = InMemoryOntologyRegistry::new();
        let mut strict = order();
        strict.classification = Classification::Pii;
        r.upsert(strict, false).await.unwrap();

        let mut relaxed = order();
        relaxed.classification = Classification::Internal;
        let err = r.upsert(relaxed, false).await.unwrap_err();
        assert!(matches!(err, OntologyError::ClassificationDowngrade { .. }));
    }

    #[tokio::test]
    async fn breaking_version_requires_override() {
        let r = InMemoryOntologyRegistry::new();
        let mut v1 = order();
        v1.version = EntityVersion::new(1, 0, 0);
        r.upsert(v1, false).await.unwrap();

        let mut v2 = order();
        v2.version = EntityVersion::new(2, 0, 0);
        let err = r.upsert(v2.clone(), false).await.unwrap_err();
        assert!(matches!(err, OntologyError::IncompatibleVersion(_)));

        // With override it succeeds.
        r.upsert(v2, true).await.unwrap();
    }
}
