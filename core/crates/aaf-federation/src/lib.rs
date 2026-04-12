//! Cell / cross-org federation.
//!
//! Implements:
//! - [`CellConfig`] — local cell runtime configuration
//! - [`FederationAgreement`] — bilateral / multilateral data sharing
//!   contract, expressible in **entity space** (E2 Slice C) as well
//!   as the legacy field-name space
//! - [`Boundary`] — see [`Router::enforce_outbound`] and the new
//!   [`Router::enforce_capability`] / [`Router::enforce_outbound_entity`]
//!   helpers
//! - [`Router`] — chooses a destination cell for a target capability
//!
//! **Entity-space boundaries (E2 Slice C).** Before iteration 8 a
//! federation agreement was a pile of `prohibited_fields: HashSet<String>`.
//! That reproduced the classic "semantics are field names" mistake the
//! whole ontology layer was built to avoid. Iteration 8 adds
//! `entity_rules: Vec<EntityAccessRule>`, where each rule declares a
//! target entity id, the set of operations the destination cell may
//! perform on it (`Read` / `Write` / `Emit`), and an optional
//! `max_classification` cap. The router then enforces agreements
//! against the *declared* `CapabilityContract.reads/writes/emits`
//! fields, rather than asking a human to maintain two parallel lists.

#![deny(missing_docs)]
#![forbid(unsafe_code)]

pub mod cosign;

pub use cosign::{cosign_token, verify_cosigned, CoSignError, CoSignedToken};

use aaf_contracts::{CapabilityContract, DataClassification, EntityRefLite};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use thiserror::Error;

/// Cell identifier (e.g. `cell-japan`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct CellId(pub String);

/// Cell runtime configuration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CellConfig {
    /// Cell id.
    pub id: CellId,
    /// Region label.
    pub region: String,
    /// Capabilities provided locally.
    pub local_capabilities: Vec<String>,
}

/// Operation kind for an [`EntityAccessRule`]. Matches the three
/// fields added to `CapabilityContract` by the E2 Slice A ontology
/// work: `reads` / `writes` / `emits`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EntityOp {
    /// Destination cell may read instances of the entity.
    Read,
    /// Destination cell may write / mutate instances of the entity.
    Write,
    /// Destination cell may emit events carrying the entity as payload.
    Emit,
}

/// Maximum classification a federation agreement permits. Mirrors
/// [`DataClassification`] but tenant-scoped to the agreement: cell A
/// may tell cell B "you can read `commerce.Customer` *but only at
/// Internal class or below*", and the router rejects caps that would
/// cross that cap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClassificationCap {
    /// Cell may only touch `Public`-classified instances.
    Public,
    /// Cell may touch up to `Internal`-classified instances.
    Internal,
    /// Cell may touch up to `Confidential`-classified instances.
    Confidential,
    /// Cell may touch up to `Restricted`-classified instances.
    Restricted,
}

impl ClassificationCap {
    fn level(self) -> u8 {
        match self {
            ClassificationCap::Public => 0,
            ClassificationCap::Internal => 1,
            ClassificationCap::Confidential => 2,
            ClassificationCap::Restricted => 3,
        }
    }

    fn for_data(c: DataClassification) -> u8 {
        match c {
            DataClassification::Public => 0,
            DataClassification::Internal => 1,
            DataClassification::Confidential => 2,
            DataClassification::Restricted => 3,
        }
    }
}

/// An entity-space rule attached to a [`FederationAgreement`].
///
/// **Semantics.** A rule is *permissive*: a cell may perform the
/// declared `op` on the declared `entity_id` up to (and including)
/// the `max_classification`. Anything that would exceed the cap is
/// rejected. Anything on an entity that has *no* rule at all is
/// rejected by default (deny-by-default), so agreements must be
/// explicit.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityAccessRule {
    /// Dot-qualified entity id (e.g. `"commerce.Order"`).
    pub entity_id: String,
    /// Operation permitted.
    pub op: EntityOp,
    /// Optional cap on the classification level. `None` means "no cap
    /// beyond the agreement's own scope".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub max_classification: Option<ClassificationCap>,
    /// Optional tenant restriction — when present, the rule applies
    /// only to the named tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<String>,
}

/// Federation agreement between cells.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FederationAgreement {
    /// Parties.
    pub parties: Vec<CellId>,
    /// Capabilities shared across the agreement.
    pub shared_capabilities: Vec<String>,
    /// Legacy field-name denylist. Kept for back-compatibility with
    /// pre-Slice-C configs; new deployments should prefer
    /// [`FederationAgreement::entity_rules`].
    #[serde(default)]
    pub prohibited_fields: HashSet<String>,
    /// Entity-space rules (E2 Slice C). An empty vec means "no
    /// entity-space restrictions apply", which falls back to the
    /// field-name denylist.
    #[serde(default)]
    pub entity_rules: Vec<EntityAccessRule>,
}

impl FederationAgreement {
    /// Construct a legacy (field-name only) agreement.
    pub fn with_prohibited_fields(
        parties: Vec<CellId>,
        shared_capabilities: Vec<String>,
        prohibited_fields: HashSet<String>,
    ) -> Self {
        Self {
            parties,
            shared_capabilities,
            prohibited_fields,
            entity_rules: vec![],
        }
    }

    /// Construct an entity-space agreement.
    pub fn with_entity_rules(
        parties: Vec<CellId>,
        shared_capabilities: Vec<String>,
        entity_rules: Vec<EntityAccessRule>,
    ) -> Self {
        Self {
            parties,
            shared_capabilities,
            prohibited_fields: HashSet::new(),
            entity_rules,
        }
    }

    /// Does this agreement carry entity-space rules?
    pub fn has_entity_rules(&self) -> bool {
        !self.entity_rules.is_empty()
    }

    /// Returns the first rule that matches `entity_id` + `op`, or
    /// `None` if no rule covers the access.
    fn find_rule(&self, entity_id: &str, op: EntityOp) -> Option<&EntityAccessRule> {
        self.entity_rules
            .iter()
            .find(|r| r.entity_id == entity_id && r.op == op)
    }
}

/// Errors raised by the federation layer.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum FederationError {
    /// The destination cell isn't a party to any agreement.
    #[error("no agreement covers cell {0:?}")]
    NoAgreement(CellId),

    /// Outbound payload contains a prohibited field (legacy path).
    #[error("data boundary violation: field `{0}` is prohibited")]
    BoundaryViolation(String),

    /// No entity-space rule permits the requested access.
    #[error("entity boundary violation: entity `{entity_id}` op `{op:?}` is not permitted by the agreement")]
    EntityNotPermitted {
        /// Entity that the capability tried to access.
        entity_id: String,
        /// Operation the capability wanted to perform.
        op: EntityOp,
    },

    /// A rule exists but the data classification is above its cap.
    #[error(
        "entity classification cap exceeded: capability `{capability}` declares `{data_class:?}` on entity `{entity_id}` but the agreement cap is `{cap:?}`"
    )]
    ClassificationCapExceeded {
        /// Capability id that triggered the rejection.
        capability: String,
        /// Entity id whose rule is active.
        entity_id: String,
        /// Capability's declared data class.
        data_class: DataClassification,
        /// Agreement's cap for that entity.
        cap: ClassificationCap,
    },

    /// A tenant-restricted rule rejected a capability whose entity
    /// ref carried a different tenant.
    #[error(
        "tenant-restricted rule: entity `{entity_id}` rule is bound to tenant `{rule_tenant}` but capability carried `{cap_tenant}`"
    )]
    TenantMismatch {
        /// Entity id.
        entity_id: String,
        /// Tenant declared by the rule.
        rule_tenant: String,
        /// Tenant declared by the capability's entity ref.
        cap_tenant: String,
    },
}

/// Cross-cell router.
pub struct Router {
    cells: Vec<CellConfig>,
    agreements: Vec<FederationAgreement>,
}

impl Router {
    /// Construct.
    pub fn new(cells: Vec<CellConfig>, agreements: Vec<FederationAgreement>) -> Self {
        Self { cells, agreements }
    }

    /// Find the cell that hosts `capability_id`.
    pub fn route(&self, capability_id: &str) -> Option<&CellConfig> {
        self.cells
            .iter()
            .find(|c| c.local_capabilities.iter().any(|x| x == capability_id))
    }

    /// Find the agreement that covers a `(from, to)` pair, if any.
    fn agreement_for(
        &self,
        from: &CellId,
        to: &CellId,
    ) -> Result<&FederationAgreement, FederationError> {
        self.agreements
            .iter()
            .find(|a| a.parties.contains(from) && a.parties.contains(to))
            .ok_or_else(|| FederationError::NoAgreement(to.clone()))
    }

    /// Verify a payload may be sent from `from` to `to`. Legacy
    /// field-name path: consults `prohibited_fields` and walks the
    /// top-level keys of the JSON object.
    pub fn enforce_outbound(
        &self,
        from: &CellId,
        to: &CellId,
        payload: &serde_json::Value,
    ) -> Result<(), FederationError> {
        let agreement = self.agreement_for(from, to)?;
        if let Some(obj) = payload.as_object() {
            for field in &agreement.prohibited_fields {
                if obj.contains_key(field) {
                    return Err(FederationError::BoundaryViolation(field.clone()));
                }
            }
        }
        Ok(())
    }

    /// Verify that `capability`'s declared entity access is permitted
    /// by the agreement between `from` and `to`. This is the
    /// **entity-space** enforcement path introduced in E2 Slice C.
    ///
    /// Algorithm:
    /// 1. Find an agreement covering `(from, to)`. If none, deny.
    /// 2. If the agreement has *no* entity rules, delegate to the
    ///    legacy `enforce_outbound` path using the capability id as
    ///    a field-style token. This keeps pre-Slice-C configs valid.
    /// 3. Otherwise iterate the capability's declared
    ///    `reads` / `writes` / `emits` lists. For each entry:
    ///    - find the first matching rule;
    ///    - if there is no rule, return `EntityNotPermitted`;
    ///    - if the rule carries a `max_classification` and the
    ///      capability's `data_classification` exceeds it, return
    ///      `ClassificationCapExceeded`;
    ///    - if the rule carries a `tenant` and the capability's
    ///      entity ref carries a different tenant, return
    ///      `TenantMismatch`.
    pub fn enforce_capability(
        &self,
        from: &CellId,
        to: &CellId,
        capability: &CapabilityContract,
    ) -> Result<(), FederationError> {
        let agreement = self.agreement_for(from, to)?;
        if !agreement.has_entity_rules() {
            // Fall back to the legacy string-denylist check — still
            // valid in production and still cheap.
            return self.enforce_outbound(
                from,
                to,
                &serde_json::json!({}), // empty payload, nothing to reject
            );
        }

        let check = |entity_ref: &EntityRefLite, op: EntityOp| -> Result<(), FederationError> {
            let rule = agreement
                .find_rule(&entity_ref.entity_id, op)
                .ok_or_else(|| FederationError::EntityNotPermitted {
                    entity_id: entity_ref.entity_id.clone(),
                    op,
                })?;
            if let Some(cap) = rule.max_classification {
                if ClassificationCap::for_data(capability.data_classification) > cap.level() {
                    return Err(FederationError::ClassificationCapExceeded {
                        capability: capability.id.to_string(),
                        entity_id: entity_ref.entity_id.clone(),
                        data_class: capability.data_classification,
                        cap,
                    });
                }
            }
            if let (Some(rule_tenant), Some(cap_tenant)) = (&rule.tenant, &entity_ref.tenant) {
                if rule_tenant.as_str() != cap_tenant.as_str() {
                    return Err(FederationError::TenantMismatch {
                        entity_id: entity_ref.entity_id.clone(),
                        rule_tenant: rule_tenant.clone(),
                        cap_tenant: cap_tenant.to_string(),
                    });
                }
            }
            Ok(())
        };

        for r in &capability.reads {
            check(r, EntityOp::Read)?;
        }
        for w in &capability.writes {
            check(w, EntityOp::Write)?;
        }
        for e in &capability.emits {
            // `emits` carries EventRefLite — translate to an
            // EntityRefLite-compatible key for the rule lookup.
            let tmp = EntityRefLite::new(e.id.as_str());
            check(&tmp, EntityOp::Emit)?;
        }
        Ok(())
    }

    /// Verify a single entity access is permitted. Used by consumers
    /// that hold an [`EntityRefLite`] directly rather than a full
    /// capability contract.
    pub fn enforce_outbound_entity(
        &self,
        from: &CellId,
        to: &CellId,
        entity: &EntityRefLite,
        op: EntityOp,
    ) -> Result<(), FederationError> {
        let agreement = self.agreement_for(from, to)?;
        if !agreement.has_entity_rules() {
            return Ok(());
        }
        agreement
            .find_rule(&entity.entity_id, op)
            .map(|_| ())
            .ok_or(FederationError::EntityNotPermitted {
                entity_id: entity.entity_id.clone(),
                op,
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla, CompensationSpec,
        DataClassification, EndpointKind, EntityRefLite, SideEffect, TenantId,
    };

    fn legacy_router() -> Router {
        let jp = CellConfig {
            id: CellId("cell-japan".into()),
            region: "ap-northeast-1".into(),
            local_capabilities: vec!["cap-jp-orders".into()],
        };
        let us = CellConfig {
            id: CellId("cell-us".into()),
            region: "us-east-1".into(),
            local_capabilities: vec!["cap-us-orders".into()],
        };
        let ag = FederationAgreement::with_prohibited_fields(
            vec![CellId("cell-japan".into()), CellId("cell-us".into())],
            vec!["cap-jp-orders".into(), "cap-us-orders".into()],
            ["pii_email".into(), "pii_phone".into()]
                .into_iter()
                .collect(),
        );
        Router::new(vec![jp, us], vec![ag])
    }

    #[test]
    fn routes_capability_to_owning_cell() {
        let r = legacy_router();
        assert_eq!(
            r.route("cap-jp-orders").unwrap().id,
            CellId("cell-japan".into())
        );
    }

    #[test]
    fn boundary_blocks_pii() {
        let r = legacy_router();
        let err = r
            .enforce_outbound(
                &CellId("cell-japan".into()),
                &CellId("cell-us".into()),
                &serde_json::json!({"order": 1, "pii_email": "x@x.com"}),
            )
            .unwrap_err();
        assert!(matches!(err, FederationError::BoundaryViolation(_)));
    }

    #[test]
    fn no_agreement_blocks_send() {
        let r = legacy_router();
        let err = r
            .enforce_outbound(
                &CellId("cell-japan".into()),
                &CellId("cell-mars".into()),
                &serde_json::json!({}),
            )
            .unwrap_err();
        assert!(matches!(err, FederationError::NoAgreement(_)));
    }

    // ── Entity-space tests (E2 Slice C) ───────────────────────────────

    fn capability(id: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: id.into(),
            description: id.into(),
            version: "1.0".into(),
            provider_agent: "agent".into(),
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
            domains: vec![],
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        }
    }

    fn entity_router(rules: Vec<EntityAccessRule>) -> Router {
        let jp = CellConfig {
            id: CellId("cell-japan".into()),
            region: "ap-northeast-1".into(),
            local_capabilities: vec![],
        };
        let us = CellConfig {
            id: CellId("cell-us".into()),
            region: "us-east-1".into(),
            local_capabilities: vec![],
        };
        let ag = FederationAgreement::with_entity_rules(
            vec![CellId("cell-japan".into()), CellId("cell-us".into())],
            vec![],
            rules,
        );
        Router::new(vec![jp, us], vec![ag])
    }

    #[test]
    fn entity_rule_allows_declared_read() {
        let rules = vec![EntityAccessRule {
            entity_id: "commerce.Product".into(),
            op: EntityOp::Read,
            max_classification: Some(ClassificationCap::Internal),
            tenant: None,
        }];
        let r = entity_router(rules);
        let mut cap = capability("cap-product-read");
        cap.reads = vec![EntityRefLite::new("commerce.Product")];
        cap.data_classification = DataClassification::Internal;
        r.enforce_capability(
            &CellId("cell-japan".into()),
            &CellId("cell-us".into()),
            &cap,
        )
        .expect("allowed read");
    }

    #[test]
    fn entity_rule_rejects_undeclared_write() {
        let rules = vec![EntityAccessRule {
            entity_id: "commerce.Product".into(),
            op: EntityOp::Read,
            max_classification: None,
            tenant: None,
        }];
        let r = entity_router(rules);
        let mut cap = capability("cap-customer-write");
        cap.side_effect = SideEffect::Write;
        cap.compensation = Some(CompensationSpec {
            endpoint: "cap-undo".into(),
        });
        cap.writes = vec![EntityRefLite::new("commerce.Customer")];
        let err = r
            .enforce_capability(
                &CellId("cell-japan".into()),
                &CellId("cell-us".into()),
                &cap,
            )
            .unwrap_err();
        assert!(matches!(err, FederationError::EntityNotPermitted { .. }));
    }

    #[test]
    fn entity_rule_enforces_classification_cap() {
        let rules = vec![EntityAccessRule {
            entity_id: "commerce.Customer".into(),
            op: EntityOp::Read,
            max_classification: Some(ClassificationCap::Internal),
            tenant: None,
        }];
        let r = entity_router(rules);
        let mut cap = capability("cap-customer-pii-read");
        cap.reads = vec![EntityRefLite::new("commerce.Customer")];
        cap.data_classification = DataClassification::Restricted;
        let err = r
            .enforce_capability(
                &CellId("cell-japan".into()),
                &CellId("cell-us".into()),
                &cap,
            )
            .unwrap_err();
        assert!(matches!(
            err,
            FederationError::ClassificationCapExceeded { .. }
        ));
    }

    #[test]
    fn entity_rule_enforces_tenant_restriction() {
        let rules = vec![EntityAccessRule {
            entity_id: "commerce.Order".into(),
            op: EntityOp::Read,
            max_classification: None,
            tenant: Some("tenant-a".into()),
        }];
        let r = entity_router(rules);
        let mut cap = capability("cap-order-read");
        let mut order_ref = EntityRefLite::new("commerce.Order");
        order_ref.tenant = Some(TenantId::from("tenant-b"));
        cap.reads = vec![order_ref];
        let err = r
            .enforce_capability(
                &CellId("cell-japan".into()),
                &CellId("cell-us".into()),
                &cap,
            )
            .unwrap_err();
        assert!(matches!(err, FederationError::TenantMismatch { .. }));
    }

    #[test]
    fn enforce_outbound_entity_short_circuits_on_known_entity() {
        let rules = vec![EntityAccessRule {
            entity_id: "commerce.Product".into(),
            op: EntityOp::Read,
            max_classification: None,
            tenant: None,
        }];
        let r = entity_router(rules);
        let product = EntityRefLite::new("commerce.Product");
        r.enforce_outbound_entity(
            &CellId("cell-japan".into()),
            &CellId("cell-us".into()),
            &product,
            EntityOp::Read,
        )
        .expect("allowed");
    }
}
