//! Capability Contract — the typed declaration of an executable agent /
//! tool / service capability.
//!
//! Rule 9 (Compensation Before Implementation) is enforced here: any
//! capability whose [`SideEffect`] is one of `write`, `delete`, `send`,
//! `payment` MUST carry a [`CompensationSpec`].

use crate::error::ContractError;
use crate::ids::CapabilityId;
use serde::{Deserialize, Serialize};

/// Side-effect classification used by the policy engine to gate execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum SideEffect {
    /// Pure function — no observable effect.
    #[default]
    None,
    /// Reads from a system of record.
    Read,
    /// Writes to a system of record.
    Write,
    /// Deletes from a system of record (irreversible by default).
    Delete,
    /// Sends a message externally (mail, push, webhook).
    Send,
    /// Initiates a financial transaction.
    Payment,
}

impl SideEffect {
    /// Returns true for side effects that must always carry a
    /// compensation handler under Rule 9.
    pub fn requires_compensation(self) -> bool {
        matches!(
            self,
            SideEffect::Write | SideEffect::Delete | SideEffect::Send | SideEffect::Payment
        )
    }
}

/// Data sensitivity classification for boundary enforcement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DataClassification {
    /// Publicly disclosable.
    Public,
    /// Internal-only.
    Internal,
    /// Confidential — restricted distribution.
    Confidential,
    /// Restricted — strict access controls.
    Restricted,
}

/// Transport kind for the capability endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EndpointKind {
    /// gRPC service.
    Grpc,
    /// HTTP/REST.
    Http,
    /// In-process function (modular monolith).
    InProcess,
    /// Model Context Protocol server.
    Mcp,
    /// A2A (Agent-to-Agent) endpoint.
    A2a,
}

/// Endpoint coordinates for invoking a capability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityEndpoint {
    /// Transport kind.
    #[serde(rename = "type")]
    pub kind: EndpointKind,
    /// Address (URL, hostport, in-process function name).
    pub address: String,
    /// Optional method name.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub method: Option<String>,
}

/// SLA targets declared by the capability owner.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Default)]
pub struct CapabilitySla {
    /// 50th percentile latency in ms.
    #[serde(default)]
    pub latency_p50_ms: u64,
    /// 99th percentile latency in ms.
    #[serde(default)]
    pub latency_p99_ms: u64,
    /// Availability target in \[0,1\].
    #[serde(default)]
    pub availability: f64,
}

/// Per-request cost declared by the capability owner.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Default)]
pub struct CapabilityCost {
    /// Cost charged per request in `currency`.
    #[serde(default)]
    pub per_request: f64,
    /// ISO 4217 currency code (default `USD`).
    #[serde(default = "default_currency")]
    pub currency: String,
}

fn default_currency() -> String {
    "USD".to_string()
}

/// One degradation level a capability supports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DegradationSpec {
    /// Logical level name.
    pub level: DegradationLevel,
    /// Human-readable description.
    pub description: String,
    /// Optional condition that triggers transitioning into this level.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trigger: Option<String>,
    /// Optional fallback procedure when the capability is unavailable.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback: Option<String>,
}

/// Discrete degradation levels per the AAF degradation chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Ord, PartialOrd)]
#[serde(rename_all = "snake_case")]
pub enum DegradationLevel {
    /// Full functionality.
    Full,
    /// Partial functionality (subset of data, slightly stale).
    Partial,
    /// Cached responses.
    Cached,
    /// Capability unavailable — fallback only.
    Unavailable,
}

/// Compensation handler reference for write capabilities (Rule 9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompensationSpec {
    /// Logical endpoint of the compensating capability.
    pub endpoint: String,
}

/// Lightweight version triple used on the wire. Intentionally serde-compatible
/// with `aaf_ontology::EntityVersion` so the two types can round-trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityVersionLite {
    /// Major.
    pub major: u32,
    /// Minor.
    pub minor: u32,
    /// Patch.
    pub patch: u32,
}

impl Default for EntityVersionLite {
    fn default() -> Self {
        Self {
            major: 0,
            minor: 1,
            patch: 0,
        }
    }
}

/// Lightweight entity reference — the on-the-wire shape of
/// `aaf_ontology::EntityRef`. Lives in `aaf-contracts` so every
/// contract that needs to point at an entity can do so without
/// depending on the full ontology crate.
///
/// **Rule 21** — `tenant` is carried explicitly.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EntityRefLite {
    /// Dot-qualified entity id (e.g. `"commerce.Order"`).
    pub entity_id: String,
    /// Version at which the ref was resolved.
    #[serde(default)]
    pub version: EntityVersionLite,
    /// Owning tenant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tenant: Option<crate::ids::TenantId>,
    /// Service-local id, if known.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub local_id: Option<String>,
}

impl EntityRefLite {
    /// Convenience constructor.
    pub fn new(entity_id: impl Into<String>) -> Self {
        Self {
            entity_id: entity_id.into(),
            version: EntityVersionLite::default(),
            tenant: None,
            local_id: None,
        }
    }
}

/// Reference to a domain event produced by a capability.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct EventRefLite {
    /// Event id.
    pub id: String,
    /// Event schema version.
    #[serde(default)]
    pub version: EntityVersionLite,
}

/// Optional narrowing predicate on an entity scope — free text in v0.1.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EntityScopeLite {
    /// Predicate expression.
    pub expression: String,
}

/// Capability Contract — typed declaration of an executable capability.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CapabilityContract {
    /// Stable id (e.g. `cap-inventory-check`).
    pub id: CapabilityId,
    /// Human-readable display name.
    pub name: String,
    /// Description used by capability discovery.
    pub description: String,
    /// Semver string.
    pub version: String,
    /// Provider agent identity.
    pub provider_agent: String,

    /// Endpoint coordinates.
    pub endpoint: CapabilityEndpoint,
    /// JSON Schema describing input.
    #[serde(default)]
    pub input_schema: serde_json::Value,
    /// JSON Schema describing output.
    #[serde(default)]
    pub output_schema: serde_json::Value,

    /// Side-effect classification.
    pub side_effect: SideEffect,
    /// Whether identical inputs always produce identical outputs.
    pub idempotent: bool,
    /// Whether the action is reversible (informational).
    pub reversible: bool,
    /// Whether the capability is deterministic (Rule 5 candidates).
    pub deterministic: bool,

    /// Compensation handler — required for `requires_compensation()` side
    /// effects.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub compensation: Option<CompensationSpec>,

    /// SLA targets.
    #[serde(default)]
    pub sla: CapabilitySla,
    /// Cost model.
    #[serde(default)]
    pub cost: CapabilityCost,

    /// Required scope to invoke this capability.
    pub required_scope: String,
    /// Data classification for boundary enforcement.
    pub data_classification: DataClassification,

    /// Declared degradation chain.
    #[serde(default)]
    pub degradation: Vec<DegradationSpec>,
    /// Capability dependencies.
    #[serde(default)]
    pub depends_on: Vec<CapabilityId>,
    /// Capabilities that must not be combined with this one.
    #[serde(default)]
    pub conflicts_with: Vec<CapabilityId>,

    /// Discovery tags.
    #[serde(default)]
    pub tags: Vec<String>,
    /// Domains the capability serves.
    #[serde(default)]
    pub domains: Vec<String>,

    // ── Enhancement E2: Domain Ontology Layer ──────────────────────────
    /// Entities this capability **reads** from the domain ontology.
    /// Optional in Slice A; becomes a lint warning in Slice B and an
    /// error in Slice C (Rule 14).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub reads: Vec<EntityRefLite>,
    /// Entities this capability **writes** to the domain ontology.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub writes: Vec<EntityRefLite>,
    /// Domain events this capability emits.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub emits: Vec<EventRefLite>,
    /// Optional narrowing predicate (e.g. `"tenant_id = $caller"`).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entity_scope: Option<EntityScopeLite>,

    // ── Enhancement X1: Agent Identity (Wave 2) ────────────────────────
    /// Minimum attestation level an agent must present to invoke
    /// this capability. `None` means no attestation is required.
    /// Enforced by `aaf-registry` + `aaf-policy` in Slice B.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_attestation_level: Option<crate::identity::AttestationLevelRef>,

    // ── Enhancement E1 Slice B: Feedback Spine ───────────────────────
    /// Outcome-weighted reputation score in `[0.0, 1.0]`. Default
    /// 0.5 (neutral). Updated by `aaf-learn::capability_scorer` and
    /// consumed by the planner's value routing and by discovery
    /// ranking. Rule 17: every update carries a `LearnedRuleRef`
    /// so it is reversible.
    #[serde(default = "default_reputation")]
    pub reputation: f32,
    /// Learned rules associated with this capability.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub learned_rules: Vec<crate::learn::LearnedRuleRef>,
}

fn default_reputation() -> f32 {
    0.5
}

impl CapabilityContract {
    /// Validate structural invariants (Rule 9 compensation requirement).
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.name.trim().is_empty() {
            return Err(ContractError::MissingField("name"));
        }
        if self.version.trim().is_empty() {
            return Err(ContractError::MissingField("version"));
        }
        if self.side_effect.requires_compensation() && self.compensation.is_none() {
            return Err(ContractError::MissingCompensation(
                self.id.to_string(),
                self.side_effect,
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn read_cap() -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from("cap-stock-check"),
            name: "stock check".into(),
            description: "check stock levels".into(),
            version: "1.0.0".into(),
            provider_agent: "inventory-agent".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::Grpc,
                address: "inventory:50051".into(),
                method: Some("CheckStock".into()),
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
            required_scope: "inventory:read".into(),
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

    #[test]
    fn read_capability_needs_no_compensation() {
        read_cap().validate().expect("valid");
    }

    #[test]
    fn write_capability_without_compensation_is_rejected() {
        let mut cap = read_cap();
        cap.side_effect = SideEffect::Write;
        let err = cap.validate().unwrap_err();
        assert!(matches!(
            err,
            ContractError::MissingCompensation(_, SideEffect::Write)
        ));
    }

    #[test]
    fn write_capability_with_compensation_is_valid() {
        let mut cap = read_cap();
        cap.side_effect = SideEffect::Write;
        cap.compensation = Some(CompensationSpec {
            endpoint: "cap-stock-release".into(),
        });
        cap.validate().expect("valid");
    }
}
