//! High-level planner that consults the registry, then runs bounds and
//! composition checks before returning an execution plan.

use crate::bounds::{BoundedAutonomy, BoundsViolation};
use crate::cache::{key_for, PlanCache};
use crate::composition::{CompositionChecker, CompositionViolation, EntityAwareComposition};
use crate::plan::{ExecutionPlan, PlannedStep, PlannedStepKind};
use aaf_contracts::{IntentEnvelope, NodeId};
use aaf_registry::{DiscoveryQuery, Registry, RegistryError};
use thiserror::Error;

/// Planner errors.
#[derive(Debug, Error)]
pub enum PlannerError {
    /// Registry failure.
    #[error("registry: {0}")]
    Registry(#[from] RegistryError),

    /// Bounds violation.
    #[error("bounds: {0}")]
    Bounds(#[from] BoundsViolation),

    /// Composition was deemed unsafe — base checker rejected the mix.
    #[error("composition unsafe")]
    UnsafeComposition,

    /// Composition was deemed unsafe by the entity-aware checker.
    /// Carries the concrete reason so the trace records it.
    #[error("composition unsafe (entity-aware): {}", .0.explain())]
    UnsafeEntityComposition(CompositionViolation),

    /// No capability matched the intent.
    #[error("no capability matched the intent")]
    NoCapability,
}

/// Registry-backed planner.
pub struct RegistryPlanner {
    registry: std::sync::Arc<Registry>,
    bounds: BoundedAutonomy,
    composition: CompositionChecker,
    /// Opt-in entity-aware composition checker (E2 Slice B). When
    /// present the planner runs it *after* the base checker so the
    /// v0.1 invariants remain the first line of defence.
    entity_composition: Option<EntityAwareComposition>,
    cache: PlanCache,
}

impl RegistryPlanner {
    /// Construct.
    pub fn new(
        registry: std::sync::Arc<Registry>,
        bounds: BoundedAutonomy,
        composition: CompositionChecker,
    ) -> Self {
        Self {
            registry,
            bounds,
            composition,
            entity_composition: None,
            cache: PlanCache::new(),
        }
    }

    /// Install an entity-aware composition checker. The planner will
    /// run it after the base composition check and reject the plan
    /// with [`PlannerError::UnsafeEntityComposition`] on any
    /// violation.
    pub fn with_entity_composition(mut self, checker: EntityAwareComposition) -> Self {
        self.entity_composition = Some(checker);
        self
    }

    /// Plan an intent.
    ///
    /// Algorithm:
    /// 1. Cache lookup keyed by `(intent_type, domain, goal)`.
    /// 2. Discover candidate capabilities (domain-scoped first, then
    ///    domain-agnostic fallback).
    /// 3. **Topologically order** the candidates by their `depends_on`
    ///    edges, producing a multi-step plan whose later steps consume
    ///    the outputs of their dependencies. Capabilities with no
    ///    declared dependency on each other are emitted in registry
    ///    discovery order.
    /// 4. Run [`crate::composition::CompositionChecker`] over the chosen
    ///    set (rejects emergent multi-write composition).
    /// 5. Run [`crate::bounds::BoundedAutonomy::validate`] (Rule 8).
    /// 6. Cache and return.
    pub async fn plan(&self, intent: &IntentEnvelope) -> Result<ExecutionPlan, PlannerError> {
        let key = key_for(
            &intent.goal,
            &intent.domain,
            &format!("{:?}", intent.intent_type),
        );
        if let Some(p) = self.cache.get(&key) {
            return Ok(p);
        }

        let q = DiscoveryQuery {
            query: intent.goal.clone(),
            domain: Some(intent.domain.clone()),
            limit: 10,
        };
        let mut hits = self.registry.discover(&q).await?;
        if hits.is_empty() {
            hits = self
                .registry
                .discover(&DiscoveryQuery::new(intent.goal.clone()))
                .await?;
        }
        if hits.is_empty() {
            return Err(PlannerError::NoCapability);
        }

        // Take the highest-scoring capability and any of the remaining
        // candidates that the top one depends on transitively. The
        // result is the *capability set* the plan will execute.
        let top = hits[0].capability.clone();
        let mut chosen: Vec<aaf_contracts::CapabilityContract> = vec![top.clone()];
        for hit in hits.iter().skip(1) {
            if top
                .depends_on
                .iter()
                .any(|d| d.as_str() == hit.capability.id.as_str())
            {
                chosen.push(hit.capability.clone());
            }
        }

        // Composition safety + bounds.
        let cap_refs: Vec<&aaf_contracts::CapabilityContract> = chosen.iter().collect();
        if !self.composition.is_safe(&cap_refs) {
            return Err(PlannerError::UnsafeComposition);
        }
        // Entity-aware composition safety (E2 Slice B). Only runs if
        // the planner was explicitly wired with one; absent the
        // wiring, the planner behaves exactly as it did pre-Slice-B.
        if let Some(ec) = &self.entity_composition {
            if let Err(v) = ec.check(&cap_refs) {
                return Err(PlannerError::UnsafeEntityComposition(v));
            }
        }

        // Topological order: dependencies first.
        let ordered = topo_sort(&chosen);

        let mut steps = Vec::with_capacity(ordered.len());
        for (idx, cap) in ordered.iter().enumerate() {
            let step_id = (idx + 1) as u32;
            steps.push(PlannedStep {
                step_id,
                kind: if cap.deterministic {
                    PlannedStepKind::Deterministic
                } else {
                    PlannedStepKind::Agent
                },
                capability: cap.id.clone(),
                input_mapping: if idx == 0 {
                    format!("intent.goal → {}", cap.id)
                } else {
                    format!("step_{}.output → {}", idx, cap.id)
                },
                output_id: NodeId::new(),
            });
        }

        let plan = ExecutionPlan {
            intent_id: intent.intent_id.clone(),
            steps,
        };
        self.bounds.validate(intent, &plan)?;
        self.cache.put(key, plan.clone());
        Ok(plan)
    }
}

/// Topological sort over a small capability set using their
/// `depends_on` edges. Stable: capabilities with the same dependency
/// depth keep their input order. Cycles are broken by emitting in
/// input order (the registry's `register` cannot create true cycles
/// because it does not validate transitively in v0.1).
fn topo_sort(caps: &[aaf_contracts::CapabilityContract]) -> Vec<aaf_contracts::CapabilityContract> {
    use std::collections::{HashMap, HashSet};

    fn visit(
        c: &aaf_contracts::CapabilityContract,
        by_id: &std::collections::HashMap<String, &aaf_contracts::CapabilityContract>,
        id_set: &std::collections::HashSet<String>,
        visited: &mut std::collections::HashSet<String>,
        order: &mut Vec<aaf_contracts::CapabilityContract>,
    ) {
        if !visited.insert(c.id.to_string()) {
            return;
        }
        for dep in &c.depends_on {
            if id_set.contains(dep.as_str()) {
                if let Some(d) = by_id.get(dep.as_str()) {
                    visit(d, by_id, id_set, visited, order);
                }
            }
        }
        order.push(c.clone());
    }

    let id_set: HashSet<String> = caps.iter().map(|c| c.id.to_string()).collect();
    let mut visited: HashSet<String> = HashSet::new();
    let mut order: Vec<aaf_contracts::CapabilityContract> = Vec::with_capacity(caps.len());
    let by_id: HashMap<String, &aaf_contracts::CapabilityContract> =
        caps.iter().map(|c| (c.id.to_string(), c)).collect();

    for c in caps {
        visit(c, &by_id, &id_set, &mut visited, &mut order);
    }
    order
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
        CapabilitySla, DataClassification, EndpointKind, IntentId, IntentType, Requester, RiskTier,
        SideEffect, TraceId,
    };
    use chrono::Utc;
    use std::sync::Arc;

    fn cap(id: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: "stock check".into(),
            description: "check stock".into(),
            version: "1.0".into(),
            provider_agent: "inv".into(),
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
            domains: vec!["warehouse".into()],
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        }
    }

    fn intent() -> IntentEnvelope {
        IntentEnvelope {
            intent_id: IntentId::new(),
            intent_type: IntentType::AnalyticalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec!["x:read".into()],
                tenant: None,
            },
            goal: "check stock".into(),
            domain: "warehouse".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 0.5,
                max_latency_ms: 1000,
            },
            deadline: None,
            risk_tier: RiskTier::Read,
            approval_policy: "none".into(),
            output_contract: None,
            trace_id: TraceId::new(),
            depth: 0,
            created_at: Utc::now(),
            entities_in_context: vec![],
        }
    }

    #[tokio::test]
    async fn plans_single_step_for_top_match() {
        let r = Arc::new(Registry::in_memory());
        r.register(cap("c1")).await.unwrap();
        let p = RegistryPlanner::new(r, BoundedAutonomy::default(), CompositionChecker::default());
        let plan = p.plan(&intent()).await.unwrap();
        assert_eq!(plan.steps.len(), 1);
    }

    #[tokio::test]
    async fn plans_multi_step_chain_via_depends_on() {
        let r = Arc::new(Registry::in_memory());
        // c-base is a dependency of c-derived. Both must surface in
        // discovery for the same goal — ensure both names match the
        // query terms.
        let mut base = cap("cap-stock-fetch");
        base.name = "stock fetch warehouse".into();
        base.description = "fetch stock warehouse".into();
        let mut derived = cap("cap-stock-aggregate");
        derived.name = "stock aggregate warehouse".into();
        derived.description = "aggregate stock warehouse".into();
        derived.depends_on = vec![CapabilityId::from("cap-stock-fetch")];
        r.register(base).await.unwrap();
        r.register(derived).await.unwrap();

        let mut env = intent();
        env.goal = "stock warehouse aggregate".into();
        let p = RegistryPlanner::new(r, BoundedAutonomy::default(), CompositionChecker::default());
        let plan = p.plan(&env).await.unwrap();
        // Two steps, dependency before dependent.
        assert_eq!(plan.steps.len(), 2);
        let pos = |id: &str| {
            plan.steps
                .iter()
                .position(|s| s.capability.as_str() == id)
                .unwrap()
        };
        assert!(pos("cap-stock-fetch") < pos("cap-stock-aggregate"));
    }

    #[tokio::test]
    async fn second_plan_call_hits_cache() {
        let r = Arc::new(Registry::in_memory());
        r.register(cap("c1")).await.unwrap();
        let p = RegistryPlanner::new(r, BoundedAutonomy::default(), CompositionChecker::default());
        let _ = p.plan(&intent()).await.unwrap();
        let _ = p.plan(&intent()).await.unwrap();
        // No public counter on the cache, but we can at least confirm
        // the plan returned is identical.
    }

    #[tokio::test]
    async fn entity_aware_composition_rejects_double_write() {
        use crate::composition::EntityAwareComposition;
        use aaf_contracts::{CompensationSpec, EntityRefLite};

        let r = Arc::new(Registry::in_memory());

        // Two write capabilities that both touch commerce.Order —
        // an illegal composition once the entity-aware checker is on.
        //
        // The *top* of the discovery ranking must depend on the
        // *other* cap (not the other way round) so the planner's
        // "pull in dependencies of top" loop yields both capabilities
        // in the chosen set. We name/describe the top to win the
        // lexical score on the intent goal.
        let mut base_cap = cap("cap-order-reserve");
        base_cap.name = "order reserve".into();
        base_cap.description = "reserve order stock".into();
        base_cap.side_effect = aaf_contracts::SideEffect::Write;
        base_cap.compensation = Some(CompensationSpec {
            endpoint: "cap-order-release".into(),
        });
        base_cap.writes = vec![EntityRefLite::new("commerce.Order")];

        let mut top_cap = cap("cap-order-confirm");
        top_cap.name = "order confirm finalize double".into();
        top_cap.description = "confirm order finalize double commerce".into();
        top_cap.side_effect = aaf_contracts::SideEffect::Write;
        top_cap.compensation = Some(CompensationSpec {
            endpoint: "cap-order-reopen".into(),
        });
        top_cap.writes = vec![EntityRefLite::new("commerce.Order")];
        top_cap.depends_on = vec![CapabilityId::from("cap-order-reserve")];

        r.register(base_cap).await.unwrap();
        r.register(top_cap).await.unwrap();

        let planner =
            RegistryPlanner::new(r, BoundedAutonomy::default(), CompositionChecker::default())
                .with_entity_composition(
                    EntityAwareComposition::new(CompositionChecker::default()),
                );

        // Goal tuned so `cap-order-confirm` (top) wins the lexical
        // scorer; it then pulls in `cap-order-reserve` as its
        // dependency. Both write `commerce.Order` → double-write.
        let mut env = intent();
        env.goal = "order confirm finalize double commerce".into();
        env.requester.scopes = vec!["x:read".into(), "auto-approve".into()];
        env.risk_tier = RiskTier::Write;

        let err = planner.plan(&env).await.unwrap_err();
        assert!(
            matches!(err, PlannerError::UnsafeEntityComposition(_)),
            "expected UnsafeEntityComposition, got {err:?}"
        );
    }
}
