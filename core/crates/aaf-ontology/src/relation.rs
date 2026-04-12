//! Entity relations.

use crate::entity::EntityId;
use serde::{Deserialize, Serialize};

/// Kind of relation between two entities.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    /// 1:1 containment (`Order` has one `ShippingAddress`).
    HasOne,
    /// 1:N containment (`Order` has many `LineItem`).
    HasMany,
    /// Non-containing reference (`Order` references `Customer`).
    References,
    /// Derived relationship — `Artifact` derived from an entity.
    DerivedFrom,
}

/// Cardinality hint used by the planner / memory retrieval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Cardinality {
    /// Exactly one related entity.
    ExactlyOne,
    /// Zero or one related entity.
    ZeroOrOne,
    /// Zero or many related entities.
    ZeroOrMany,
    /// One or many related entities.
    OneOrMany,
}

/// A directional relation between two entities.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Relation {
    /// Relation kind.
    pub kind: RelationKind,
    /// Source entity id.
    pub from: EntityId,
    /// Destination entity id.
    pub to: EntityId,
    /// Cardinality hint.
    pub cardinality: Cardinality,
    /// Optional description.
    #[serde(default)]
    pub description: String,
}

impl Relation {
    /// Convenience constructor.
    pub fn new(
        kind: RelationKind,
        from: impl Into<EntityId>,
        to: impl Into<EntityId>,
        cardinality: Cardinality,
    ) -> Self {
        Self {
            kind,
            from: from.into(),
            to: to.into(),
            cardinality,
            description: String::new(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_simple_relation() {
        let r = Relation::new(
            RelationKind::HasMany,
            "commerce.Order",
            "commerce.LineItem",
            Cardinality::OneOrMany,
        );
        assert_eq!(r.from, "commerce.Order");
        assert_eq!(r.to, "commerce.LineItem");
    }
}
