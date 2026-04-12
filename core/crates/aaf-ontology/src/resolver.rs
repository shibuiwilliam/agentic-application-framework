//! Entity resolution.
//!
//! A resolver turns a `(service_id, local_id)` pair into a global
//! [`crate::entity::EntityRef`]. Concrete strategies include:
//!
//! - `ExactMatchResolver` — lookups in a static in-process table
//! - (future) embedding-based resolver
//! - (future) external resolver service over gRPC

use crate::entity::{EntityId, EntityRef, EntityVersion};
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;

/// Resolution outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolverOutcome {
    /// Found a single canonical [`EntityRef`].
    Resolved(EntityRef),
    /// Found multiple candidates — caller must choose or refine.
    Ambiguous(Vec<EntityRef>),
    /// No match.
    Unresolved,
}

/// Pluggable resolver trait.
#[async_trait]
pub trait EntityResolver: Send + Sync {
    /// Resolve a `(service_id, local_id)` into an `EntityRef`.
    async fn resolve(&self, service_id: &str, local_id: &str) -> ResolverOutcome;
}

/// Deterministic resolver backed by an exact-match table. Primarily
/// used in tests and for small deployments where the mapping is
/// static.
pub struct ExactMatchResolver {
    table: Arc<RwLock<HashMap<(String, String), EntityRef>>>,
}

impl Default for ExactMatchResolver {
    fn default() -> Self {
        Self {
            table: Arc::new(RwLock::new(HashMap::new())),
        }
    }
}

impl ExactMatchResolver {
    /// Construct empty.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a `(service, local_id) → EntityRef` mapping.
    pub fn insert(&self, service_id: impl Into<String>, local_id: impl Into<String>, r: EntityRef) {
        self.table
            .write()
            .insert((service_id.into(), local_id.into()), r);
    }

    /// Convenience: register by raw entity id + version.
    pub fn register(
        &self,
        service_id: &str,
        local_id: &str,
        entity_id: impl Into<EntityId>,
        version: EntityVersion,
    ) {
        self.insert(
            service_id,
            local_id,
            EntityRef::new(entity_id, version).with_local_id(local_id),
        );
    }
}

#[async_trait]
impl EntityResolver for ExactMatchResolver {
    async fn resolve(&self, service_id: &str, local_id: &str) -> ResolverOutcome {
        let key = (service_id.to_string(), local_id.to_string());
        match self.table.read().get(&key).cloned() {
            Some(r) => ResolverOutcome::Resolved(r),
            None => ResolverOutcome::Unresolved,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn exact_match_round_trip() {
        let r = ExactMatchResolver::new();
        r.register(
            "order-service",
            "ord-123",
            "commerce.Order",
            EntityVersion::initial(),
        );
        let outcome = r.resolve("order-service", "ord-123").await;
        match outcome {
            ResolverOutcome::Resolved(e) => {
                assert_eq!(e.entity_id, "commerce.Order");
                assert_eq!(e.local_id.unwrap(), "ord-123");
            }
            _ => panic!(),
        }
    }

    #[tokio::test]
    async fn unresolved_returns_unresolved() {
        let r = ExactMatchResolver::new();
        assert_eq!(r.resolve("x", "y").await, ResolverOutcome::Unresolved);
    }
}
