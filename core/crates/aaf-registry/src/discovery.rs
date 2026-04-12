//! Capability discovery.
//!
//! v0.1 uses a deterministic lexical-similarity scorer over
//! `name + description + tags`. A future iteration will swap this for an
//! embedding-based vector search behind the same trait.
//!
//! **E2 Slice B** adds an **entity-aware** discovery surface: consumers
//! can ask the registry "what capabilities *write* `commerce.Order`?"
//! and receive the matching capability set. The inverted index is
//! rebuilt on demand (every call to `list()`), which is O(n) over the
//! registry size. For the in-memory backend used in tests and dev this
//! is measurement-noise cheap; a pgvector / Postgres backend would
//! maintain a materialised JSONB GIN index instead.

use crate::error::RegistryError;
use crate::store::Registry;
use aaf_contracts::{CapabilityContract, EntityRefLite};

/// Discovery query.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiscoveryQuery {
    /// Free-text goal.
    pub query: String,
    /// Optional domain filter.
    pub domain: Option<String>,
    /// Maximum results.
    pub limit: usize,
}

impl DiscoveryQuery {
    /// Convenience constructor.
    pub fn new(query: impl Into<String>) -> Self {
        Self {
            query: query.into(),
            domain: None,
            limit: 5,
        }
    }
}

/// Ranked discovery result.
#[derive(Debug, Clone, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct DiscoveryResult {
    /// Matched capability.
    pub capability: CapabilityContract,
    /// Lexical match score in `[0,1]`.
    pub score: f64,
}

/// Compute a lexical match score in `[0,1]` between `q` and a haystack.
fn lexical_score(q: &str, haystack: &str) -> f64 {
    let q = q.to_lowercase();
    let haystack = haystack.to_lowercase();
    let q_terms: Vec<&str> = q.split_whitespace().collect();
    if q_terms.is_empty() {
        return 0.0;
    }
    let mut hits = 0;
    for term in &q_terms {
        if haystack.contains(term) {
            hits += 1;
        }
    }
    hits as f64 / q_terms.len() as f64
}

/// Which declared entity role to match against (E2 Slice B).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntityQueryKind {
    /// Capabilities that **read** the target entity.
    Reads,
    /// Capabilities that **write** the target entity.
    Writes,
    /// Capabilities that **emit** the target entity as an event payload.
    Emits,
}

impl Registry {
    /// Discover the top-N capabilities matching `query`.
    pub async fn discover(
        &self,
        query: &DiscoveryQuery,
    ) -> Result<Vec<DiscoveryResult>, RegistryError> {
        let mut results = vec![];
        for cap in self.list().await? {
            if let Some(d) = &query.domain {
                if !cap.domains.iter().any(|cd| cd == d) {
                    continue;
                }
            }
            let haystack = format!("{} {} {}", cap.name, cap.description, cap.tags.join(" "));
            let score = lexical_score(&query.query, &haystack);
            if score > 0.0 {
                results.push(DiscoveryResult {
                    capability: cap,
                    score,
                });
            }
        }
        results.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        results.truncate(query.limit);
        Ok(results)
    }

    /// Find capabilities whose declared ontology fields reference
    /// `entity` under the chosen role (`reads` / `writes` / `emits`).
    ///
    /// Entity IDs are compared by their dot-qualified string
    /// (`commerce.Order`); tenant, version, and local_id are ignored
    /// at the discovery boundary because tenants are resolved per-call
    /// by the planner. Results are returned in registry order, which
    /// is deterministic for the in-memory backend and tie-breaks
    /// meaningfully in production-style backends.
    pub async fn discover_by_entity(
        &self,
        entity: &EntityRefLite,
        kind: EntityQueryKind,
    ) -> Result<Vec<CapabilityContract>, RegistryError> {
        let mut hits = vec![];
        for cap in self.list().await? {
            let declared: &[EntityRefLite] = match kind {
                EntityQueryKind::Reads => &cap.reads,
                EntityQueryKind::Writes => &cap.writes,
                // `emits` carries `EventRefLite`, which is structurally
                // similar but not the same type — map it through the
                // event id so we can match on a single string.
                EntityQueryKind::Emits => {
                    if cap.emits.iter().any(|e| e.id == entity.entity_id) {
                        hits.push(cap);
                    }
                    continue;
                }
            };
            if declared.iter().any(|r| r.entity_id == entity.entity_id) {
                hits.push(cap);
            }
        }
        Ok(hits)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla, DataClassification,
        EndpointKind, SideEffect,
    };

    fn cap(id: &str, name: &str, desc: &str, domain: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: name.into(),
            description: desc.into(),
            version: "1.0".into(),
            provider_agent: "a".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::Grpc,
                address: "x".into(),
                method: None,
            },
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            side_effect: SideEffect::Read,
            idempotent: true,
            reversible: true,
            deterministic: true,
            compensation: None,
            sla: CapabilitySla::default(),
            cost: CapabilityCost::default(),
            required_scope: "x:read".into(),
            data_classification: DataClassification::Internal,
            degradation: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            tags: vec![],
            domains: vec![domain.into()],
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        }
    }

    #[tokio::test]
    async fn discover_ranks_by_lexical_overlap() {
        let r = Registry::in_memory();
        r.register(cap(
            "c-a",
            "stock check",
            "check warehouse stock",
            "warehouse",
        ))
        .await
        .unwrap();
        r.register(cap(
            "c-b",
            "ticket close",
            "close support tickets",
            "support",
        ))
        .await
        .unwrap();

        let res = r
            .discover(&DiscoveryQuery::new("warehouse stock"))
            .await
            .unwrap();
        assert_eq!(res[0].capability.id.as_str(), "c-a");
    }

    #[tokio::test]
    async fn domain_filter_excludes_others() {
        let r = Registry::in_memory();
        r.register(cap("c-a", "stock check", "x", "warehouse"))
            .await
            .unwrap();
        r.register(cap("c-b", "ticket", "stock review", "support"))
            .await
            .unwrap();

        let q = DiscoveryQuery {
            query: "stock".into(),
            domain: Some("warehouse".into()),
            limit: 5,
        };
        let res = r.discover(&q).await.unwrap();
        assert_eq!(res.len(), 1);
        assert_eq!(res[0].capability.id.as_str(), "c-a");
    }

    // ── Entity-aware discovery (E2 Slice B) ────────────────────────────

    fn cap_with_entity_fields(
        id: &str,
        reads: Vec<&str>,
        writes: Vec<&str>,
        emits: Vec<&str>,
    ) -> CapabilityContract {
        let mut c = cap(id, "cap", "cap", "warehouse");
        c.reads = reads.into_iter().map(EntityRefLite::new).collect();
        c.writes = writes.into_iter().map(EntityRefLite::new).collect();
        c.emits = emits
            .into_iter()
            .map(|e| aaf_contracts::EventRefLite {
                id: e.into(),
                version: aaf_contracts::EntityVersionLite::default(),
            })
            .collect();
        // A write capability needs compensation under Rule 9; tests
        // here use read side effect so they remain valid contracts.
        c
    }

    #[tokio::test]
    async fn discover_by_entity_writes_returns_writers_only() {
        let r = Registry::in_memory();
        r.register(cap_with_entity_fields(
            "cap-reader",
            vec!["commerce.Order"],
            vec![],
            vec![],
        ))
        .await
        .unwrap();

        // A writer must carry compensation — bypass by using a read
        // cap that has `writes:` populated in its declaration but no
        // runtime side effect. The discovery index does not consult
        // `side_effect`, only the declared entity fields.
        let mut writer =
            cap_with_entity_fields("cap-writer", vec![], vec!["commerce.Order"], vec![]);
        writer.description = "writer".into();
        r.register(writer).await.unwrap();

        let order = EntityRefLite::new("commerce.Order");
        let writers = r
            .discover_by_entity(&order, EntityQueryKind::Writes)
            .await
            .unwrap();
        assert_eq!(writers.len(), 1);
        assert_eq!(writers[0].id.as_str(), "cap-writer");

        let readers = r
            .discover_by_entity(&order, EntityQueryKind::Reads)
            .await
            .unwrap();
        assert_eq!(readers.len(), 1);
        assert_eq!(readers[0].id.as_str(), "cap-reader");
    }

    #[tokio::test]
    async fn discover_by_entity_emits_matches_event_id() {
        let r = Registry::in_memory();
        r.register(cap_with_entity_fields(
            "cap-emitter",
            vec![],
            vec![],
            vec!["commerce.OrderPlaced"],
        ))
        .await
        .unwrap();

        let event = EntityRefLite::new("commerce.OrderPlaced");
        let hits = r
            .discover_by_entity(&event, EntityQueryKind::Emits)
            .await
            .unwrap();
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].id.as_str(), "cap-emitter");
    }

    #[tokio::test]
    async fn discover_by_entity_returns_empty_for_unknown_entity() {
        let r = Registry::in_memory();
        r.register(cap_with_entity_fields(
            "cap-other",
            vec!["commerce.Customer"],
            vec![],
            vec![],
        ))
        .await
        .unwrap();
        let unknown = EntityRefLite::new("commerce.Shipment");
        let hits = r
            .discover_by_entity(&unknown, EntityQueryKind::Reads)
            .await
            .unwrap();
        assert!(hits.is_empty());
    }
}
