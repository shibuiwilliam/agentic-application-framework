//! Evaluation context passed to every rule.

use aaf_contracts::{
    BudgetContract, CapabilityContract, IntentEnvelope, Requester, SideEffect, TenantId,
};

/// Classification level reported by an [`OntologyClassificationLookup`]
/// for a given entity id. The values mirror the lattice in
/// `aaf_ontology::Classification` without introducing a direct crate
/// dependency (policy engine stays lightweight).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EntityClass {
    /// Publicly disclosable.
    Public,
    /// Internal-only.
    Internal,
    /// Personally Identifiable Information.
    Pii,
    /// Regulated (PCI / HIPAA / …).
    Regulated(String),
}

impl EntityClass {
    /// Numeric level used to compare classifications.
    pub fn level(&self) -> u8 {
        match self {
            EntityClass::Public => 0,
            EntityClass::Internal => 1,
            EntityClass::Pii => 2,
            EntityClass::Regulated(_) => 3,
        }
    }
}

/// Callback the boundary rule consults to resolve an entity id to its
/// declared classification. Wired from the server / integration
/// harness at construction time.
pub type OntologyClassificationLookup =
    std::sync::Arc<dyn Fn(&str) -> Option<EntityClass> + Send + Sync>;

/// All the inputs a rule may inspect to make a decision.
pub struct PolicyContext<'a> {
    /// The originating intent.
    pub intent: &'a IntentEnvelope,
    /// The capability being invoked, if any.
    pub capability: Option<&'a CapabilityContract>,
    /// The principal that issued the action.
    pub requester: &'a Requester,
    /// Optional payload to inspect for PII / injection.
    pub payload: Option<&'a str>,
    /// Optional output to inspect for PII / disclosure.
    pub output: Option<&'a str>,
    /// Side-effect of the proposed action (when known up front).
    pub side_effect: Option<SideEffect>,
    /// Remaining budget at the moment of evaluation.
    pub remaining_budget: BudgetContract,
    /// Tenant scope (for boundary enforcement).
    pub tenant: Option<&'a TenantId>,
    /// Number of write capabilities already executed within this trace.
    pub composed_writes: u32,
    /// Optional ontology classification lookup (E2 Slice B). When
    /// present, the boundary rule consults it to detect classification
    /// leaks (a capability reading an entity whose class exceeds its
    /// own data classification).
    pub ontology_class_lookup: Option<OntologyClassificationLookup>,
}
