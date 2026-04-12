//! Entity types.
//!
//! An [`Entity`] is a first-class noun in the AAF domain ontology.
//! It is declared once, versioned, classified for security purposes
//! (Rule 21: tenant-scoped by default), and referenced by every
//! capability, intent, and artifact that touches it.

use aaf_contracts::TenantId;
use serde::{Deserialize, Serialize};

/// Dot-qualified, bounded-context id. Example: `"commerce.Order"`.
pub type EntityId = String;

/// Semver-style version triple. Kept simple and deterministic; the
/// [`crate::version`] module knows how to compare them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityVersion {
    /// Major.
    pub major: u32,
    /// Minor.
    pub minor: u32,
    /// Patch.
    pub patch: u32,
}

impl EntityVersion {
    /// Helper constructor.
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
    /// Bootstrap version for newly-declared entities.
    pub const fn initial() -> Self {
        Self::new(0, 1, 0)
    }
}

impl std::fmt::Display for EntityVersion {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

/// Data classification drives security decisions across the framework.
///
/// Classifications form a lattice: `Public ⊂ Internal ⊂ Pii ⊂
/// Regulated(_)`. Downgrades are denied by default (see
/// [`crate::error::OntologyError::ClassificationDowngrade`]).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Classification {
    /// Publicly disclosable.
    Public,
    /// Internal — not for external eyes.
    Internal,
    /// Personally Identifiable Information.
    Pii,
    /// Regulated data with a jurisdiction/tag (e.g. `"pci"`, `"hipaa"`).
    Regulated(String),
}

impl Classification {
    /// Numeric level used to compare classifications.
    pub fn level(&self) -> u8 {
        match self {
            Classification::Public => 0,
            Classification::Internal => 1,
            Classification::Pii => 2,
            Classification::Regulated(_) => 3,
        }
    }

    /// Returns `true` if `self` can flow into a destination classified
    /// as `other`. Flow is allowed only from lower-or-equal level to
    /// higher-or-equal level.
    pub fn can_flow_into(&self, other: &Classification) -> bool {
        self.level() <= other.level()
    }
}

/// Primitive-ish field type. JSON Schema would be more expressive; for
/// v0.1 a small closed enum is sufficient to model real-world shapes.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FieldType {
    /// String.
    String,
    /// 64-bit signed integer.
    Integer,
    /// 64-bit float.
    Float,
    /// Boolean.
    Boolean,
    /// ISO 8601 timestamp.
    Timestamp,
    /// Money (ISO 4217 code + minor units).
    Money,
    /// Reference to another entity by id.
    EntityRef(EntityId),
    /// Nested object (opaque at this level).
    Object,
    /// List of a primitive type name — kept simple.
    List(String),
}

/// One field on an entity. Fields inherit the entity's classification
/// unless they explicitly override with a stricter one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityField {
    /// Field name.
    pub name: String,
    /// Field type.
    pub field_type: FieldType,
    /// Whether the field is required.
    #[serde(default)]
    pub required: bool,
    /// Optional override classification (must be ≥ parent).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub classification: Option<Classification>,
    /// Short human-readable description.
    #[serde(default)]
    pub description: String,
}

/// Optional narrowing predicate attached to a capability's entity scope.
/// v0.1 stores it as an opaque structured filter — a future slice will
/// parse it into CEL / an AST.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityScopePredicate {
    /// Free-form predicate expression, e.g. `"tenant_id = $caller"`.
    pub expression: String,
}

/// A reference to a domain event produced by a capability. Events are
/// first-class members of the ontology because they carry payloads
/// composed of entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventRef {
    /// Event id, dot-qualified like entities. Example: `"commerce.OrderPlaced"`.
    pub id: String,
    /// Version of the event schema.
    pub version: EntityVersion,
}

/// Canonical reference to an entity instance. Carries a tenant
/// dimension implicitly (Rule 21). Nothing in the rest of the
/// framework should build `EntityRef`s without thinking about the
/// tenant.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRef {
    /// Which entity this ref points to.
    pub entity_id: EntityId,
    /// The version of the entity schema this ref was resolved against.
    pub version: EntityVersion,
    /// Owning tenant. `None` means "global / not yet resolved".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<TenantId>,
    /// Service-local identifier, e.g. the primary key in a
    /// microservice's database.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_id: Option<String>,
}

impl EntityRef {
    /// Convenience constructor.
    pub fn new(entity_id: impl Into<EntityId>, version: EntityVersion) -> Self {
        Self {
            entity_id: entity_id.into(),
            version,
            tenant: None,
            local_id: None,
        }
    }

    /// Attach a tenant.
    pub fn with_tenant(mut self, tenant: TenantId) -> Self {
        self.tenant = Some(tenant);
        self
    }

    /// Attach a service-local identifier.
    pub fn with_local_id(mut self, local: impl Into<String>) -> Self {
        self.local_id = Some(local.into());
        self
    }
}

/// The full ontology entry for an entity.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Entity {
    /// Stable id.
    pub id: EntityId,
    /// Entity schema version.
    pub version: EntityVersion,
    /// Human-readable description.
    pub description: String,
    /// Top-level classification, inherited by every field unless
    /// overridden.
    pub classification: Classification,
    /// Field list.
    pub fields: Vec<EntityField>,
    /// Declared relations to other entities.
    pub relations: Vec<crate::relation::Relation>,
    /// Optional owner service id — helps federation agreements.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner_service: Option<String>,
}

impl Entity {
    /// Construct a minimal entity. Tests use this constantly.
    pub fn new(
        id: impl Into<EntityId>,
        description: impl Into<String>,
        classification: Classification,
    ) -> Self {
        Self {
            id: id.into(),
            version: EntityVersion::initial(),
            description: description.into(),
            classification,
            fields: vec![],
            relations: vec![],
            owner_service: None,
        }
    }

    /// Returns the effective classification of a named field (falling
    /// back to the entity's top-level classification if the field
    /// doesn't override it).
    pub fn classification_for(&self, field: &str) -> Classification {
        self.fields
            .iter()
            .find(|f| f.name == field)
            .and_then(|f| f.classification.clone())
            .unwrap_or_else(|| self.classification.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classification_lattice_allows_flow_up() {
        assert!(Classification::Public.can_flow_into(&Classification::Internal));
        assert!(Classification::Internal.can_flow_into(&Classification::Pii));
        assert!(!Classification::Pii.can_flow_into(&Classification::Public));
    }

    #[test]
    fn entity_field_classification_defaults_to_parent() {
        let mut e = Entity::new("commerce.Order", "an order", Classification::Internal);
        e.fields.push(EntityField {
            name: "id".into(),
            field_type: FieldType::String,
            required: true,
            classification: None,
            description: String::new(),
        });
        assert_eq!(e.classification_for("id"), Classification::Internal);
    }

    #[test]
    fn entity_field_classification_can_strengthen() {
        let mut e = Entity::new("commerce.Customer", "a customer", Classification::Internal);
        e.fields.push(EntityField {
            name: "email".into(),
            field_type: FieldType::String,
            required: false,
            classification: Some(Classification::Pii),
            description: String::new(),
        });
        assert_eq!(e.classification_for("email"), Classification::Pii);
    }

    #[test]
    fn entity_ref_builder_attaches_tenant() {
        let r = EntityRef::new("commerce.Order", EntityVersion::initial())
            .with_tenant(TenantId::from("tenant-a"))
            .with_local_id("ord-123");
        assert_eq!(r.tenant.unwrap().as_str(), "tenant-a");
        assert_eq!(r.local_id.unwrap(), "ord-123");
    }

    #[test]
    fn entity_version_display_renders_semver() {
        assert_eq!(EntityVersion::new(1, 2, 3).to_string(), "1.2.3");
    }
}
