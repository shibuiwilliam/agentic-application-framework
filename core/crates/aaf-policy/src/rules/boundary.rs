//! `boundary_enforcement` rule — tenant + data classification + entity
//! classification flow.
//!
//! This rule fires three distinct checks:
//!
//! 1. **Tenant scope** — the requester's tenant must match the active
//!    tenant. Violations are `Fatal`.
//! 2. **Data classification** — a capability carrying a `Confidential`
//!    or `Restricted` `DataClassification` requires the matching
//!    requester scope. Violations are `Error`.
//! 3. **Entity classification flow** (E2 Slice B) — when a
//!    [`crate::context::OntologyClassificationLookup`] is installed,
//!    the rule consults the ontology for every entity the capability
//!    declared via `reads:`. If any entity's classification exceeds
//!    the capability's `data_classification`, the rule emits a
//!    `Fatal` violation (classification-flow leak: sensitive data
//!    flowing into a lower-class output).
//!
//! The ontology branch is opt-in: deployments that do not wire a
//! classification lookup continue to see only the tag-based checks
//! (1) and (2), preserving v0.1 semantics.

use super::Rule;
use crate::context::{EntityClass, PolicyContext};
use aaf_contracts::{DataClassification, PolicySeverity, PolicyViolation, RuleKind};

fn capability_level(c: DataClassification) -> u8 {
    match c {
        DataClassification::Public => 0,
        DataClassification::Internal => 1,
        DataClassification::Confidential => 2,
        DataClassification::Restricted => 3,
    }
}

/// Tenant + data-classification boundary check.
pub struct BoundaryEnforcement;

impl Rule for BoundaryEnforcement {
    fn id(&self) -> &str {
        "tenant-boundary"
    }

    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation> {
        // 1. Tenant scope: requester tenant must match the active tenant.
        if let (Some(active), Some(req)) = (ctx.tenant, ctx.requester.tenant.as_ref()) {
            if active != req {
                return Some(PolicyViolation {
                    rule_id: self.id().into(),
                    kind: RuleKind::BoundaryEnforcement,
                    severity: PolicySeverity::Fatal,
                    message: format!(
                        "tenant boundary violation: requester tenant {req} != active tenant {active}"
                    ),
                });
            }
        }

        // 2. Data classification: confidential capabilities require an
        //    explicit `confidential:read` scope.
        if let Some(cap) = ctx.capability {
            if matches!(
                cap.data_classification,
                DataClassification::Confidential | DataClassification::Restricted
            ) {
                let needs = match cap.data_classification {
                    DataClassification::Confidential => "confidential:read",
                    DataClassification::Restricted => "restricted:read",
                    _ => unreachable!(),
                };
                if !ctx.requester.scopes.iter().any(|s| s == needs) {
                    return Some(PolicyViolation {
                        rule_id: self.id().into(),
                        kind: RuleKind::BoundaryEnforcement,
                        severity: PolicySeverity::Error,
                        message: format!(
                            "data classification {:?} requires scope `{}`",
                            cap.data_classification, needs
                        ),
                    });
                }
            }
        }

        // 3. Entity classification flow (E2 Slice B).
        if let (Some(cap), Some(lookup)) = (ctx.capability, ctx.ontology_class_lookup.as_ref()) {
            let cap_level = capability_level(cap.data_classification);
            for read in &cap.reads {
                if let Some(entity_class) = lookup(&read.entity_id) {
                    if entity_class.level() > cap_level {
                        return Some(PolicyViolation {
                            rule_id: self.id().into(),
                            kind: RuleKind::BoundaryEnforcement,
                            severity: PolicySeverity::Fatal,
                            message: format!(
                                "classification flow violation: capability `{}` (class {:?}) reads entity `{}` classified {:?}",
                                cap.id, cap.data_classification, read.entity_id, entity_class
                            ),
                        });
                    }
                }
            }
            // Also check cross-tenant on declared entity_scope writes.
            for write in &cap.writes {
                if let (Some(w_tenant), Some(active)) = (&write.tenant, ctx.tenant) {
                    if w_tenant != active {
                        return Some(PolicyViolation {
                            rule_id: self.id().into(),
                            kind: RuleKind::BoundaryEnforcement,
                            severity: PolicySeverity::Fatal,
                            message: format!(
                                "tenant boundary violation: capability `{}` writes entity `{}` in tenant `{}` while active tenant is `{}`",
                                cap.id, write.entity_id, w_tenant, active
                            ),
                        });
                    }
                }
            }
        }

        // Explicitly unused — silences `unused_imports` on EntityClass
        // in deployments that do not opt into the ontology lookup.
        let _ = std::marker::PhantomData::<EntityClass>;

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::context::{EntityClass, OntologyClassificationLookup};
    use aaf_contracts::{
        BudgetContract, CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId,
        CapabilitySla, DataClassification, EndpointKind, EntityRefLite, IntentEnvelope, IntentType,
        Requester, RiskTier, SideEffect, TenantId, TraceId,
    };
    use chrono::Utc;
    use std::sync::Arc;

    fn intent() -> IntentEnvelope {
        IntentEnvelope {
            intent_id: "i1".into(),
            intent_type: IntentType::AnalyticalIntent,
            requester: Requester {
                user_id: "u".into(),
                role: "r".into(),
                scopes: vec![],
                tenant: Some(TenantId::from("tenant-a")),
            },
            goal: "g".into(),
            domain: "commerce".into(),
            constraints: Default::default(),
            budget: BudgetContract {
                max_tokens: 100,
                max_cost_usd: 1.0,
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

    fn cap_reading(id: &str, entity: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: "n".into(),
            description: "d".into(),
            version: "1.0".into(),
            provider_agent: "p".into(),
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
            data_classification: DataClassification::Public,
            degradation: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            tags: vec![],
            domains: vec![],
            reads: vec![EntityRefLite::new(entity)],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        }
    }

    #[test]
    fn ontology_lookup_flags_classification_leak() {
        let env = intent();
        let cap = cap_reading("cap-leak", "commerce.Customer");
        let tenant = TenantId::from("tenant-a");
        let lookup: OntologyClassificationLookup = Arc::new(|id: &str| {
            if id == "commerce.Customer" {
                Some(EntityClass::Pii)
            } else {
                None
            }
        });

        let ctx = PolicyContext {
            intent: &env,
            capability: Some(&cap),
            requester: &env.requester,
            payload: None,
            output: None,
            side_effect: Some(SideEffect::Read),
            remaining_budget: env.budget,
            tenant: Some(&tenant),
            composed_writes: 0,
            ontology_class_lookup: Some(lookup),
        };

        let v = BoundaryEnforcement.evaluate(&ctx).unwrap();
        assert_eq!(v.severity, PolicySeverity::Fatal);
        assert!(v.message.contains("classification flow violation"));
    }

    #[test]
    fn ontology_lookup_allows_equal_or_lower_class() {
        let env = intent();
        let mut cap = cap_reading("cap-ok", "commerce.Order");
        cap.data_classification = DataClassification::Internal;
        let tenant = TenantId::from("tenant-a");
        let lookup: OntologyClassificationLookup = Arc::new(|id: &str| {
            if id == "commerce.Order" {
                Some(EntityClass::Internal)
            } else {
                None
            }
        });

        let ctx = PolicyContext {
            intent: &env,
            capability: Some(&cap),
            requester: &env.requester,
            payload: None,
            output: None,
            side_effect: Some(SideEffect::Read),
            remaining_budget: env.budget,
            tenant: Some(&tenant),
            composed_writes: 0,
            ontology_class_lookup: Some(lookup),
        };
        assert!(BoundaryEnforcement.evaluate(&ctx).is_none());
    }

    #[test]
    fn absent_lookup_falls_back_to_legacy_checks() {
        // Without a lookup, the rule must not fire on entity reads.
        let env = intent();
        let cap = cap_reading("cap-ok", "commerce.Customer");
        let tenant = TenantId::from("tenant-a");

        let ctx = PolicyContext {
            intent: &env,
            capability: Some(&cap),
            requester: &env.requester,
            payload: None,
            output: None,
            side_effect: Some(SideEffect::Read),
            remaining_budget: env.budget,
            tenant: Some(&tenant),
            composed_writes: 0,
            ontology_class_lookup: None,
        };
        assert!(BoundaryEnforcement.evaluate(&ctx).is_none());
    }
}
