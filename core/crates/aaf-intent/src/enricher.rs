//! Context enrichment.
//!
//! v0.1 enriches an in-progress envelope with hints derived from the
//! requester profile (role + scopes) and the long-term memory facade.
//! It is intentionally synchronous and side-effect free; an LLM-backed
//! enricher can implement the same surface in a future iteration.
//!
//! **E2 Slice B** adds a second entry point,
//! [`Enricher::enrich_with_ontology`], that populates
//! `IntentEnvelope.entities_in_context` from an
//! [`OntologyResolver`]. The planner, policy engine, and long-term
//! memory retrieval all key off those entity refs, so every
//! request that flows through the enricher carries an entity
//! context by default.
//!
//! The enricher does not depend on the `aaf-ontology` crate directly
//! (it would introduce a cycle once new downstream crates grow to
//! call this enricher). Callers wire their own resolver, typically
//! backed by `aaf_ontology::OntologyRegistry::list()`.

use aaf_contracts::{EntityRefLite, IntentEnvelope, RiskTier, TenantId};

/// A callback that returns the set of entity refs known to match a
/// given (domain, goal) pair. Called once per `enrich_with_ontology`.
/// The caller typically iterates the ontology registry and picks the
/// entities whose `owner_service` matches the intent's domain.
pub trait OntologyResolver: Send + Sync {
    /// Return the entity refs whose entity-id, owner, or description
    /// matches the intent domain and/or the goal text.
    fn resolve(&self, domain: &str, goal: &str) -> Vec<EntityRefLite>;
}

impl<F> OntologyResolver for F
where
    F: Fn(&str, &str) -> Vec<EntityRefLite> + Send + Sync,
{
    fn resolve(&self, domain: &str, goal: &str) -> Vec<EntityRefLite> {
        (self)(domain, goal)
    }
}

/// Enricher.
pub struct Enricher;

impl Enricher {
    /// Apply role-based defaults to an envelope. Specifically:
    ///
    /// - if the requester role is `analyst` and no `dimension` constraint
    ///   was extracted, default to `region`
    /// - if the risk tier is `Read`, force the approval policy to `none`
    pub fn enrich(envelope: &mut IntentEnvelope) {
        if envelope.requester.role == "analyst" && !envelope.constraints.contains_key("dimension") {
            envelope
                .constraints
                .insert("dimension".into(), serde_json::json!("region"));
        }
        if envelope.risk_tier == RiskTier::Read {
            envelope.approval_policy = "none".into();
        }
    }

    /// Same as [`Enricher::enrich`], plus populates
    /// `envelope.entities_in_context` from `resolver` (E2 Slice B).
    ///
    /// Entities returned by the resolver are annotated with the
    /// requester's tenant when the intent carries one, so downstream
    /// consumers (policy boundary rule, memory lookup, composition
    /// checker) see a fully-qualified `EntityRef` rather than a
    /// tenantless id.
    ///
    /// Calling this on an envelope that already has
    /// `entities_in_context` populated will merge the new entity refs
    /// in and de-duplicate by `(entity_id, tenant)`.
    pub fn enrich_with_ontology<R: OntologyResolver + ?Sized>(
        envelope: &mut IntentEnvelope,
        resolver: &R,
    ) {
        Self::enrich(envelope);
        let discovered = resolver.resolve(&envelope.domain, &envelope.goal);
        let tenant: Option<TenantId> = envelope.requester.tenant.clone();
        for mut ent in discovered {
            if ent.tenant.is_none() {
                ent.tenant.clone_from(&tenant);
            }
            if !envelope.entities_in_context.iter().any(|existing| {
                existing.entity_id == ent.entity_id && existing.tenant == ent.tenant
            }) {
                envelope.entities_in_context.push(ent);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{BudgetContract, IntentId, IntentType, Requester, TraceId};
    use chrono::Utc;

    fn sample_envelope() -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::AnalyticalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "analyst".into(),
                scopes: vec![],
                tenant: Some(aaf_contracts::TenantId::from("tenant-a")),
            },
            goal: "show orders for region".into(),
            domain: "commerce".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 1.0,
                max_latency_ms: 1000,
            },
            deadline: None,
            risk_tier: RiskTier::Read,
            approval_policy: "human".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    #[test]
    fn enrich_with_ontology_populates_entities_and_attaches_tenant() {
        let mut env = sample_envelope();
        let resolver = |domain: &str, _goal: &str| -> Vec<EntityRefLite> {
            if domain == "commerce" {
                vec![
                    EntityRefLite::new("commerce.Order"),
                    EntityRefLite::new("commerce.Customer"),
                ]
            } else {
                vec![]
            }
        };
        Enricher::enrich_with_ontology(&mut env, &resolver);
        assert_eq!(env.entities_in_context.len(), 2);
        // Tenant propagates from requester.
        for e in &env.entities_in_context {
            assert_eq!(e.tenant.as_ref().map(|t| t.as_str()), Some("tenant-a"));
        }
    }

    #[test]
    fn enrich_with_ontology_dedupes_existing_entries() {
        let mut env = sample_envelope();
        env.entities_in_context
            .push(EntityRefLite::new("commerce.Order"));
        env.entities_in_context[0].tenant = Some(aaf_contracts::TenantId::from("tenant-a"));
        let resolver = |_d: &str, _g: &str| -> Vec<EntityRefLite> {
            vec![
                EntityRefLite::new("commerce.Order"),
                EntityRefLite::new("commerce.Customer"),
            ]
        };
        Enricher::enrich_with_ontology(&mut env, &resolver);
        assert_eq!(env.entities_in_context.len(), 2);
    }

    #[test]
    fn enriches_analyst_with_region_default() {
        let mut env = IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::AnalyticalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "analyst".into(),
                scopes: vec![],
                tenant: None,
            },
            goal: "g".into(),
            domain: "sales".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 1.0,
                max_latency_ms: 1000,
            },
            deadline: None,
            risk_tier: RiskTier::Read,
            approval_policy: "human".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        };
        Enricher::enrich(&mut env);
        assert_eq!(
            env.constraints.get("dimension"),
            Some(&serde_json::json!("region"))
        );
        assert_eq!(env.approval_policy, "none");
    }
}
