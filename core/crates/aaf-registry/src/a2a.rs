//! A2A (Agent-to-Agent) Agent Card import/export.
//!
//! An [`AgentCard`] is the public projection of a
//! [`CapabilityContract`] that hides scope/cost/internal data and
//! exposes enough metadata for cross-org discovery.
//!
//! - `from_contract` projects an internal contract → public card
//!   (export, used by partners discovering our capabilities).
//! - `into_contract` ingests an external card → internal contract
//!   (import, used to register a partner agent in our registry).
//!
//! Imported contracts default to `Read` data classification and a
//! caller-provided `required_scope` so the policy engine can still gate
//! them; the cost/SLA fields stay at defaults until the partner
//! advertises them out-of-band.

use aaf_contracts::{
    CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla,
    DataClassification, EndpointKind, SideEffect,
};
use serde::{Deserialize, Serialize};

/// External A2A agent card.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AgentCard {
    /// Capability id (logical).
    pub id: String,
    /// Display name.
    pub name: String,
    /// Description.
    pub description: String,
    /// Version.
    pub version: String,
    /// Endpoint kind.
    pub endpoint_kind: EndpointKind,
    /// Endpoint address (typically a URL).
    pub endpoint_address: String,
    /// Side effect classification.
    pub side_effect: SideEffect,
    /// Domain tags.
    pub domains: Vec<String>,
}

impl AgentCard {
    /// Project an internal contract into an external card.
    pub fn from_contract(c: &CapabilityContract) -> Self {
        Self {
            id: c.id.to_string(),
            name: c.name.clone(),
            description: c.description.clone(),
            version: c.version.clone(),
            endpoint_kind: c.endpoint.kind,
            endpoint_address: c.endpoint.address.clone(),
            side_effect: c.side_effect,
            domains: c.domains.clone(),
        }
    }

    /// Ingest a public card into an internal contract for registration
    /// in this cell's registry.
    ///
    /// `required_scope` is provided by the caller because cards do
    /// not advertise authentication scopes by design — the importing
    /// cell decides what scope its own users need to invoke the
    /// partner agent.
    pub fn into_contract(self, required_scope: impl Into<String>) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(self.id.as_str()),
            name: self.name,
            description: self.description,
            version: self.version,
            provider_agent: "a2a-import".into(),
            endpoint: CapabilityEndpoint {
                kind: self.endpoint_kind,
                address: self.endpoint_address,
                method: None,
            },
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            side_effect: self.side_effect,
            idempotent: false,
            reversible: false,
            deterministic: false,
            // Imported writes are always gated through approval —
            // we cannot inspect the partner's actual compensation
            // surface, so the safe default is "no compensation
            // recorded → policy engine forces approval".
            compensation: None,
            sla: CapabilitySla::default(),
            cost: CapabilityCost::default(),
            required_scope: required_scope.into(),
            data_classification: DataClassification::Internal,
            degradation: vec![],
            depends_on: vec![],
            conflicts_with: vec![],
            tags: vec!["a2a".into(), "imported".into()],
            domains: self.domains,
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            // Imported A2A cards start unattested; the importing cell
            // can upgrade them by issuing its own Attestation.
            required_attestation_level: None,
            // E1 Slice B: reputation starts at neutral, no learned rules.
            reputation: 0.5,
            learned_rules: vec![],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_card() -> AgentCard {
        AgentCard {
            id: "cap-partner-quote".into(),
            name: "partner quote".into(),
            description: "request a quote from partner".into(),
            version: "1.0.0".into(),
            endpoint_kind: EndpointKind::A2a,
            endpoint_address: "https://partner.example.com/agent".into(),
            side_effect: SideEffect::Read,
            domains: vec!["procurement".into()],
        }
    }

    #[test]
    fn round_trip_export_then_reimport_preserves_id_and_name() {
        let card = sample_card();
        let imported = card.clone().into_contract("partner:read");
        let reexported = AgentCard::from_contract(&imported);
        assert_eq!(reexported.id, card.id);
        assert_eq!(reexported.name, card.name);
        assert_eq!(reexported.endpoint_kind, EndpointKind::A2a);
    }

    #[test]
    fn imported_read_capability_passes_validation() {
        let cap = sample_card().into_contract("partner:read");
        // Read capabilities have no compensation requirement, so this
        // must validate without error (Rule 9).
        cap.validate().expect("read capability is valid");
    }
}
