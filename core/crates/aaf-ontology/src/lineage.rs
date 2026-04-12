//! Entity lineage / provenance.
//!
//! Every capability invocation that writes an entity leaves a
//! [`LineageRecord`]. Downstream code (reports, audits, cost
//! attribution, rollback) walks this graph.

use crate::entity::{EntityId, EntityRef, EntityVersion};
use aaf_contracts::{CapabilityId, TraceId};
use chrono::{DateTime, Utc};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

/// A reference to an entity at a specific version. Used in
/// [`aaf_contracts::Artifact::derived_from`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityRefVersioned {
    /// Which entity.
    pub entity_ref: EntityRef,
    /// The entity version this lineage was captured at.
    pub version: EntityVersion,
}

/// One lineage record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LineageRecord {
    /// The entity that was produced.
    pub produced: EntityRefVersioned,
    /// Entities that were read to produce it.
    pub derived_from: Vec<EntityRefVersioned>,
    /// Capability that produced it.
    pub producing_capability: CapabilityId,
    /// Trace id (links back to [`aaf_trace::Recorder`]).
    pub trace_id: TraceId,
    /// When.
    pub created_at: DateTime<Utc>,
}

/// In-memory lineage store. Keyed by `entity_id`; each entity can
/// have many records (one per write).
#[derive(Default)]
pub struct LineageStore {
    inner: Arc<RwLock<Vec<LineageRecord>>>,
}

impl LineageStore {
    /// Construct.
    pub fn new() -> Self {
        Self::default()
    }

    /// Append.
    pub fn record(&self, rec: LineageRecord) {
        self.inner.write().push(rec);
    }

    /// Return every record for a given entity id.
    pub fn for_entity(&self, id: &EntityId) -> Vec<LineageRecord> {
        self.inner
            .read()
            .iter()
            .filter(|r| &r.produced.entity_ref.entity_id == id)
            .cloned()
            .collect()
    }

    /// Total number of records.
    pub fn len(&self) -> usize {
        self.inner.read().len()
    }

    /// Whether empty.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rec(id: &str) -> LineageRecord {
        LineageRecord {
            produced: EntityRefVersioned {
                entity_ref: EntityRef::new(id, EntityVersion::initial()),
                version: EntityVersion::initial(),
            },
            derived_from: vec![],
            producing_capability: CapabilityId::from("cap-x"),
            trace_id: TraceId::new(),
            created_at: Utc::now(),
        }
    }

    #[test]
    fn records_and_filters_by_entity_id() {
        let s = LineageStore::new();
        s.record(rec("commerce.Order"));
        s.record(rec("commerce.Customer"));
        s.record(rec("commerce.Order"));
        assert_eq!(s.for_entity(&"commerce.Order".to_string()).len(), 2);
        assert_eq!(s.for_entity(&"commerce.Customer".to_string()).len(), 1);
    }
}
