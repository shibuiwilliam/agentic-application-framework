//! Capability composition safety check.
//!
//! v0.1 enforced a single invariant: "no plan may exceed N
//! write-class side effects". That caught the crudest form of
//! emergent risk but missed three cases the ontology now exposes:
//!
//! 1. **Double-write** — two capabilities in the same plan write the
//!    *same* entity (say `commerce.Order`) with no explicit
//!    reconciliation step. Under v0.1 this looked like "two write
//!    capabilities", which is below the default `max_writes=3`, so
//!    the plan was accepted. With entities declared, we can tell the
//!    two writes collide on the same noun and reject the plan.
//! 2. **Classification leak** — a plan reads an entity whose
//!    classification is `Pii`/`Regulated` and then feeds it into a
//!    step whose capability advertises a lower `DataClassification`.
//!    Under v0.1 there was no way to connect the reader's output to
//!    the next step's input *by noun*; classification flowed through
//!    capability tags that humans had to set consistently. The
//!    ontology makes this mechanical.
//! 3. **Cross-tenant fan-out** — a plan writes `EntityRef`s carrying
//!    different tenants, which should only be possible behind an
//!    explicit federation agreement. v0.1 never saw tenants on
//!    entities because entities were not first-class.
//!
//! This module keeps the v0.1 `CompositionChecker` unchanged so
//! existing call sites stay source-compatible, and adds an
//! `EntityAwareComposition` wrapper that layers the three new checks
//! on top.

use aaf_contracts::{CapabilityContract, DataClassification, SideEffect};
use std::collections::{BTreeSet, HashMap};

/// Severity classes emitted by the entity-aware composition checker.
///
/// We intentionally keep this enum tiny: the planner only needs to
/// decide "accept the plan" vs "reject it", and the reason string is
/// surfaced to the caller via `PlannerError::UnsafeComposition` so the
/// trace records the concrete violation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CompositionViolation {
    /// Two or more capabilities declare a write to the same entity id
    /// without an explicit reconciliation step. The reported set is
    /// the capability ids and the entity id they collide on.
    DoubleWrite {
        /// Colliding entity id.
        entity_id: String,
        /// Capabilities whose `writes:` include the entity id.
        capabilities: Vec<String>,
    },
    /// A capability reads an entity whose declared classification
    /// exceeds the (next or own) capability's advertised
    /// `data_classification`, i.e. sensitive data flows into a
    /// lower-classification output.
    ClassificationLeak {
        /// Capability that declared the leaky read.
        capability: String,
        /// Entity that flows into the capability.
        entity_id: String,
        /// The capability's own data classification.
        capability_class: DataClassification,
        /// The entity's classification as resolved from the ontology.
        entity_class_hint: ClassificationHint,
    },
    /// Writes in a single plan fan out across two or more tenants.
    CrossTenantFanOut {
        /// Entities whose writes crossed the boundary.
        tenants: BTreeSet<String>,
    },
}

impl CompositionViolation {
    /// Render a short single-line explanation for the trace / log.
    pub fn explain(&self) -> String {
        match self {
            CompositionViolation::DoubleWrite {
                entity_id,
                capabilities,
            } => format!(
                "double-write on entity `{}` by capabilities [{}]",
                entity_id,
                capabilities.join(", ")
            ),
            CompositionViolation::ClassificationLeak {
                capability,
                entity_id,
                capability_class,
                entity_class_hint,
            } => format!(
                "classification leak: capability `{}` (class {:?}) reads entity `{}` classified {:?}",
                capability, capability_class, entity_id, entity_class_hint
            ),
            CompositionViolation::CrossTenantFanOut { tenants } => format!(
                "cross-tenant fan-out across tenants [{}]",
                tenants.iter().cloned().collect::<Vec<_>>().join(", ")
            ),
        }
    }
}

/// Hint about an entity's classification supplied by the caller.
///
/// The planner does not depend on `aaf-ontology` directly (doing so
/// would introduce a cycle once the ontology layer grows), so
/// classification information is passed in through a lookup callback.
/// See [`EntityAwareComposition::with_classification_lookup`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ClassificationHint {
    /// Public / disclosable.
    Public,
    /// Internal-only.
    Internal,
    /// Contains PII — must not flow into lower-class output.
    Pii,
    /// Regulated (e.g. PCI, HIPAA).
    Regulated(String),
}

impl ClassificationHint {
    /// Numeric level used to compare classifications.
    pub fn level(&self) -> u8 {
        match self {
            ClassificationHint::Public => 0,
            ClassificationHint::Internal => 1,
            ClassificationHint::Pii => 2,
            ClassificationHint::Regulated(_) => 3,
        }
    }

    /// Level of a [`DataClassification`] tag on a capability.
    pub fn level_of_capability(c: DataClassification) -> u8 {
        match c {
            DataClassification::Public => 0,
            DataClassification::Internal => 1,
            DataClassification::Confidential => 2,
            DataClassification::Restricted => 3,
        }
    }
}

/// Callback that resolves an entity id → its declared classification.
pub type ClassificationLookup = Box<dyn Fn(&str) -> Option<ClassificationHint> + Send + Sync>;

/// Validates a candidate set of capabilities.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CompositionChecker {
    /// Maximum write-class side effects allowed in a single plan.
    pub max_writes: u32,
}

impl Default for CompositionChecker {
    fn default() -> Self {
        Self { max_writes: 3 }
    }
}

impl CompositionChecker {
    /// Returns `true` if the composition is safe under the configured
    /// limits.
    pub fn is_safe(&self, capabilities: &[&CapabilityContract]) -> bool {
        let writes = capabilities
            .iter()
            .filter(|c| {
                matches!(
                    c.side_effect,
                    SideEffect::Write | SideEffect::Delete | SideEffect::Send | SideEffect::Payment
                )
            })
            .count() as u32;
        writes <= self.max_writes
    }
}

/// Entity-aware composition safety (E2 Slice B).
///
/// Wraps a base [`CompositionChecker`] and layers three entity-aware
/// detectors on top. Constructed with a classification-lookup
/// callback so the planner does not need to depend on the ontology
/// crate directly; the callback is populated by whatever wires the
/// planner (`aaf-server`, integration tests, user code) from an
/// `OntologyRegistry::get` call.
pub struct EntityAwareComposition {
    base: CompositionChecker,
    classification_lookup: Option<ClassificationLookup>,
}

impl std::fmt::Debug for EntityAwareComposition {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("EntityAwareComposition")
            .field("base", &self.base)
            .field(
                "classification_lookup",
                &self.classification_lookup.is_some(),
            )
            .finish()
    }
}

impl EntityAwareComposition {
    /// Construct with just a base checker — classification leak
    /// detection is disabled (returns no hint) unless a lookup is
    /// installed via [`Self::with_classification_lookup`].
    pub fn new(base: CompositionChecker) -> Self {
        Self {
            base,
            classification_lookup: None,
        }
    }

    /// Install a classification resolver. The planner must supply one
    /// in any deployment that carries an ontology.
    pub fn with_classification_lookup(mut self, lookup: ClassificationLookup) -> Self {
        self.classification_lookup = Some(lookup);
        self
    }

    /// Run every detector against `capabilities`. Returns `Ok(())` if
    /// the plan is safe, otherwise the first violation detected.
    ///
    /// Detectors run in this fixed order so the reported reason is
    /// deterministic under test. Cross-tenant fan-out is checked
    /// **before** double-write because when two capabilities both
    /// write the same entity id under different tenants, the
    /// more-precise reason is "cross-tenant fan-out" (the writes are
    /// deliberate under their own tenants; the violation is the
    /// composition crossing the boundary), not "double-write".
    ///
    /// 1. base `max_writes` cap (inherited from v0.1)
    /// 2. cross-tenant fan-out across writes
    /// 3. double-write on the same entity id
    /// 4. classification leak per read
    pub fn check(&self, capabilities: &[&CapabilityContract]) -> Result<(), CompositionViolation> {
        // 1. Base cap.
        if !self.base.is_safe(capabilities) {
            // Base failure is rendered as a DoubleWrite with empty
            // entity — the concrete entity-aware explanation is the
            // whole point of this module, but we still surface the
            // base failure to keep the existing semantics visible.
            return Err(CompositionViolation::DoubleWrite {
                entity_id: String::new(),
                capabilities: capabilities.iter().map(|c| c.id.to_string()).collect(),
            });
        }

        // 2. Cross-tenant fan-out.
        if let Some(v) = detect_cross_tenant_fan_out(capabilities) {
            return Err(v);
        }

        // 3. Double-write.
        if let Some(v) = detect_double_write(capabilities) {
            return Err(v);
        }

        // 4. Classification leak.
        if let Some(lookup) = &self.classification_lookup {
            if let Some(v) = detect_classification_leak(capabilities, lookup.as_ref()) {
                return Err(v);
            }
        }

        Ok(())
    }
}

fn detect_double_write(capabilities: &[&CapabilityContract]) -> Option<CompositionViolation> {
    // `BTreeMap` for deterministic reporting order.
    let mut writers: std::collections::BTreeMap<String, Vec<String>> = Default::default();
    for cap in capabilities {
        for w in &cap.writes {
            writers
                .entry(w.entity_id.clone())
                .or_default()
                .push(cap.id.to_string());
        }
    }
    for (entity_id, caps) in writers {
        if caps.len() >= 2 {
            return Some(CompositionViolation::DoubleWrite {
                entity_id,
                capabilities: caps,
            });
        }
    }
    None
}

fn detect_classification_leak(
    capabilities: &[&CapabilityContract],
    lookup: &(dyn Fn(&str) -> Option<ClassificationHint> + Send + Sync),
) -> Option<CompositionViolation> {
    for cap in capabilities {
        let cap_level = ClassificationHint::level_of_capability(cap.data_classification);
        for read in &cap.reads {
            if let Some(hint) = lookup(&read.entity_id) {
                if hint.level() > cap_level {
                    return Some(CompositionViolation::ClassificationLeak {
                        capability: cap.id.to_string(),
                        entity_id: read.entity_id.clone(),
                        capability_class: cap.data_classification,
                        entity_class_hint: hint,
                    });
                }
            }
        }
    }
    None
}

fn detect_cross_tenant_fan_out(
    capabilities: &[&CapabilityContract],
) -> Option<CompositionViolation> {
    let mut tenants: BTreeSet<String> = BTreeSet::new();
    let mut per_entity_tenants: HashMap<String, BTreeSet<String>> = HashMap::new();
    for cap in capabilities {
        for w in &cap.writes {
            if let Some(t) = &w.tenant {
                let t_str = t.to_string();
                tenants.insert(t_str.clone());
                per_entity_tenants
                    .entry(w.entity_id.clone())
                    .or_default()
                    .insert(t_str);
            }
        }
    }
    // Fan-out is "more than one distinct tenant *and* at least one
    // entity id written under two tenants". The second clause avoids
    // flagging a plan that writes tenant-A's Order and tenant-B's
    // unrelated Customer, which can be legal under some agreements.
    if tenants.len() >= 2 && per_entity_tenants.values().any(|ts| ts.len() >= 2) {
        return Some(CompositionViolation::CrossTenantFanOut { tenants });
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla, CompensationSpec,
        DataClassification, EndpointKind, EntityRefLite, TenantId,
    };

    fn base(id: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: "x".into(),
            description: "x".into(),
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

    fn writer(id: &str, entity: &str) -> CapabilityContract {
        let mut c = base(id);
        c.side_effect = SideEffect::Write;
        c.compensation = Some(CompensationSpec {
            endpoint: format!("{id}-undo"),
        });
        c.writes = vec![EntityRefLite::new(entity)];
        c
    }

    #[test]
    fn base_max_writes_still_enforced() {
        let c = CompositionChecker { max_writes: 1 };
        let a = writer("cap-a", "commerce.Order");
        let b = writer("cap-b", "commerce.Shipment");
        assert!(!c.is_safe(&[&a, &b]));
    }

    #[test]
    fn entity_aware_accepts_plan_without_collisions() {
        let ec = EntityAwareComposition::new(CompositionChecker::default());
        let a = writer("cap-a", "commerce.Order");
        let b = writer("cap-b", "commerce.Shipment");
        ec.check(&[&a, &b]).expect("safe plan");
    }

    #[test]
    fn entity_aware_detects_double_write() {
        let ec = EntityAwareComposition::new(CompositionChecker::default());
        let a = writer("cap-a", "commerce.Order");
        let b = writer("cap-b", "commerce.Order");
        let v = ec.check(&[&a, &b]).unwrap_err();
        match v {
            CompositionViolation::DoubleWrite {
                entity_id,
                capabilities,
            } => {
                assert_eq!(entity_id, "commerce.Order");
                assert_eq!(capabilities.len(), 2);
            }
            other => panic!("wrong violation: {other:?}"),
        }
    }

    #[test]
    fn entity_aware_detects_classification_leak() {
        let lookup: ClassificationLookup = Box::new(|id: &str| {
            if id == "commerce.Customer" {
                Some(ClassificationHint::Pii)
            } else {
                None
            }
        });
        let ec = EntityAwareComposition::new(CompositionChecker::default())
            .with_classification_lookup(lookup);

        let mut a = base("cap-leak");
        a.reads = vec![EntityRefLite::new("commerce.Customer")];
        a.data_classification = DataClassification::Public;

        let v = ec.check(&[&a]).unwrap_err();
        assert!(matches!(v, CompositionViolation::ClassificationLeak { .. }));
    }

    #[test]
    fn entity_aware_allows_read_at_or_below_classification() {
        let lookup: ClassificationLookup = Box::new(|id: &str| {
            if id == "commerce.Order" {
                Some(ClassificationHint::Internal)
            } else {
                None
            }
        });
        let ec = EntityAwareComposition::new(CompositionChecker::default())
            .with_classification_lookup(lookup);

        let mut a = base("cap-read");
        a.reads = vec![EntityRefLite::new("commerce.Order")];
        a.data_classification = DataClassification::Internal;

        ec.check(&[&a]).expect("equal class is fine");
    }

    #[test]
    fn entity_aware_detects_cross_tenant_fan_out() {
        // Cross-tenant runs before double-write, so two writers
        // hitting `commerce.Order` under two different tenants get
        // the more precise `CrossTenantFanOut` reason.
        let ec = EntityAwareComposition::new(CompositionChecker::default());
        let tenant_a = TenantId::from("tenant-a");
        let tenant_b = TenantId::from("tenant-b");

        let mut a = writer("cap-a", "commerce.Order");
        a.writes[0].tenant = Some(tenant_a);
        let mut b = writer("cap-b", "commerce.Order");
        b.writes[0].tenant = Some(tenant_b);

        let v = ec.check(&[&a, &b]).unwrap_err();
        assert!(
            matches!(v, CompositionViolation::CrossTenantFanOut { .. }),
            "expected cross-tenant fan-out, got {v:?}"
        );
    }

    #[test]
    fn same_tenant_double_write_is_classified_as_double_write() {
        // Both writers pin the same tenant — cross-tenant must not
        // fire; double-write must.
        let ec = EntityAwareComposition::new(CompositionChecker::default());
        let tenant = TenantId::from("tenant-a");
        let mut a = writer("cap-a", "commerce.Order");
        a.writes[0].tenant = Some(tenant.clone());
        let mut b = writer("cap-b", "commerce.Order");
        b.writes[0].tenant = Some(tenant);

        let v = ec.check(&[&a, &b]).unwrap_err();
        assert!(matches!(v, CompositionViolation::DoubleWrite { .. }));
    }
}
