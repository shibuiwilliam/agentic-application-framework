//! Agent manifest — the signed declaration of what an agent is
//! allowed to do and what it is made of.

use crate::did::AgentDid;
use crate::error::IdentityError;
use crate::keystore::{Signer, Verifier};
use aaf_contracts::CapabilityId;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Pinned reference to a specific LLM model version.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelPin {
    /// Provider (e.g. `anthropic`, `openai`).
    pub provider: String,
    /// Model id.
    pub model: String,
    /// Content hash of the system prompt used with this model.
    pub system_prompt_hash: String,
}

/// Pinned reference to an external tool the agent may call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolBinding {
    /// Logical tool id.
    pub tool_id: String,
    /// Tool version string.
    pub version: String,
    /// Content hash of the tool's declared interface.
    pub interface_hash: String,
}

/// Signed agent manifest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentManifest {
    /// Agent DID this manifest authorises.
    pub did: AgentDid,
    /// Human-readable display name — never the source of trust.
    pub display_name: String,
    /// Content hash of the agent's source code / config bundle.
    pub source_hash: String,
    /// Model pins the agent is permitted to use.
    pub model_pins: Vec<ModelPin>,
    /// Prompt content hashes.
    pub prompt_hashes: Vec<String>,
    /// Tool bindings.
    pub tool_bindings: Vec<ToolBinding>,
    /// Ontology slice ids the agent is permitted to touch.
    pub ontology_slices: Vec<String>,
    /// Capability allow-list.
    pub capability_allow_list: Vec<CapabilityId>,
    /// References to eval suites the agent must pass.
    pub eval_suite_refs: Vec<String>,
    /// When the manifest was issued.
    pub issued_at: DateTime<Utc>,
    /// Optional expiry.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expires_at: Option<DateTime<Utc>>,
    /// Detached signature over the manifest body — hex-encoded.
    pub signature: String,
}

impl AgentManifest {
    /// Compute a stable hash over every field **except** the
    /// signature itself. Used both when signing and when verifying.
    fn canonical_hash(&self) -> String {
        // Clone and zero out the signature so it never feeds into the
        // hash that it is signing.
        let body = ManifestBody {
            did: self.did.clone(),
            display_name: self.display_name.clone(),
            source_hash: self.source_hash.clone(),
            model_pins: self.model_pins.clone(),
            prompt_hashes: self.prompt_hashes.clone(),
            tool_bindings: self.tool_bindings.clone(),
            ontology_slices: self.ontology_slices.clone(),
            capability_allow_list: self.capability_allow_list.clone(),
            eval_suite_refs: self.eval_suite_refs.clone(),
            issued_at: self.issued_at,
            expires_at: self.expires_at,
        };
        let bytes = serde_json::to_vec(&body).unwrap_or_default();
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hex::encode(hasher.finalize())
    }

    /// Verify the manifest's signature against a supplied verifier.
    pub fn verify(&self, verifier: &dyn Verifier) -> Result<(), IdentityError> {
        if !self.did.is_well_formed() {
            return Err(IdentityError::InvalidSignature);
        }
        let body_hash = self.canonical_hash();
        verifier.verify(&self.did, body_hash.as_bytes(), &self.signature)
    }

    /// Verify that the running code's hash matches the manifest's
    /// declared `source_hash`. Called once at startup by the server
    /// binary. Returns `Err(ManifestDrift)` if the hashes differ.
    pub fn verify_source(&self, actual_source_hash: &str) -> Result<(), IdentityError> {
        if self.source_hash == actual_source_hash {
            Ok(())
        } else {
            Err(IdentityError::ManifestDrift {
                declared: self.source_hash.clone(),
                actual: actual_source_hash.to_string(),
            })
        }
    }
}

// A version of [`AgentManifest`] without the signature — used as the
// canonical payload that gets signed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct ManifestBody {
    did: AgentDid,
    display_name: String,
    source_hash: String,
    model_pins: Vec<ModelPin>,
    prompt_hashes: Vec<String>,
    tool_bindings: Vec<ToolBinding>,
    ontology_slices: Vec<String>,
    capability_allow_list: Vec<CapabilityId>,
    eval_suite_refs: Vec<String>,
    issued_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
}

/// Build a manifest then sign it. This is the **only** way to
/// produce a valid `AgentManifest` in application code.
pub struct ManifestBuilder {
    did: AgentDid,
    display_name: String,
    source_hash: String,
    model_pins: Vec<ModelPin>,
    prompt_hashes: Vec<String>,
    tool_bindings: Vec<ToolBinding>,
    ontology_slices: Vec<String>,
    capability_allow_list: Vec<CapabilityId>,
    eval_suite_refs: Vec<String>,
    expires_at: Option<DateTime<Utc>>,
}

impl ManifestBuilder {
    /// Start a new builder rooted on an agent DID.
    pub fn new(
        did: AgentDid,
        display_name: impl Into<String>,
        source_hash: impl Into<String>,
    ) -> Self {
        Self {
            did,
            display_name: display_name.into(),
            source_hash: source_hash.into(),
            model_pins: vec![],
            prompt_hashes: vec![],
            tool_bindings: vec![],
            ontology_slices: vec![],
            capability_allow_list: vec![],
            eval_suite_refs: vec![],
            expires_at: None,
        }
    }

    /// Add a model pin.
    pub fn model(mut self, pin: ModelPin) -> Self {
        self.model_pins.push(pin);
        self
    }

    /// Add a prompt hash.
    pub fn prompt_hash(mut self, h: impl Into<String>) -> Self {
        self.prompt_hashes.push(h.into());
        self
    }

    /// Add a tool binding.
    pub fn tool(mut self, binding: ToolBinding) -> Self {
        self.tool_bindings.push(binding);
        self
    }

    /// Add an ontology slice reference.
    pub fn ontology_slice(mut self, slice: impl Into<String>) -> Self {
        self.ontology_slices.push(slice.into());
        self
    }

    /// Allow a capability id.
    pub fn allow(mut self, cap: CapabilityId) -> Self {
        self.capability_allow_list.push(cap);
        self
    }

    /// Add an eval suite reference.
    pub fn eval_ref(mut self, r: impl Into<String>) -> Self {
        self.eval_suite_refs.push(r.into());
        self
    }

    /// Set an expiry.
    pub fn expires_at(mut self, at: DateTime<Utc>) -> Self {
        self.expires_at = Some(at);
        self
    }

    /// Consume the builder, sign with `signer`, and return the
    /// finished manifest. Rule 23: this is the only way to produce
    /// a well-formed, signed `AgentManifest`.
    pub fn build(self, signer: &dyn Signer) -> Result<AgentManifest, IdentityError> {
        let body = ManifestBody {
            did: self.did.clone(),
            display_name: self.display_name,
            source_hash: self.source_hash,
            model_pins: self.model_pins,
            prompt_hashes: self.prompt_hashes,
            tool_bindings: self.tool_bindings,
            ontology_slices: self.ontology_slices,
            capability_allow_list: self.capability_allow_list,
            eval_suite_refs: self.eval_suite_refs,
            issued_at: Utc::now(),
            expires_at: self.expires_at,
        };
        let bytes =
            serde_json::to_vec(&body).map_err(|e| IdentityError::Serialisation(e.to_string()))?;
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        let hash = hex::encode(hasher.finalize());
        let signature = signer.sign(&self.did, hash.as_bytes())?;
        Ok(AgentManifest {
            did: body.did,
            display_name: body.display_name,
            source_hash: body.source_hash,
            model_pins: body.model_pins,
            prompt_hashes: body.prompt_hashes,
            tool_bindings: body.tool_bindings,
            ontology_slices: body.ontology_slices,
            capability_allow_list: body.capability_allow_list,
            eval_suite_refs: body.eval_suite_refs,
            issued_at: body.issued_at,
            expires_at: body.expires_at,
            signature,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::keystore::{InMemoryKeystore, Keystore};

    fn build_sample(ks: &InMemoryKeystore, did: AgentDid) -> AgentManifest {
        ManifestBuilder::new(did, "order-agent", "src-hash-abc")
            .model(ModelPin {
                provider: "anthropic".into(),
                model: "claude-sonnet-4-20250514".into(),
                system_prompt_hash: "prompt-hash-1".into(),
            })
            .prompt_hash("prompt-hash-1")
            .tool(ToolBinding {
                tool_id: "cap-order-read".into(),
                version: "1.0.0".into(),
                interface_hash: "iface-hash".into(),
            })
            .ontology_slice("commerce")
            .allow(CapabilityId::from("cap-order-read"))
            .eval_ref("order-processing-golden")
            .build(ks)
            .expect("build")
    }

    #[test]
    fn build_then_verify_round_trip() {
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"order-agent-seed");
        let manifest = build_sample(&ks, did);
        manifest.verify(&ks).expect("verify");
    }

    #[test]
    fn tampering_with_display_name_invalidates_signature() {
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"order-agent-seed");
        let mut manifest = build_sample(&ks, did);
        manifest.display_name = "impersonator".into();
        let err = manifest.verify(&ks).unwrap_err();
        assert_eq!(err, IdentityError::InvalidSignature);
    }

    #[test]
    fn source_drift_is_caught() {
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"order-agent-seed");
        let manifest = build_sample(&ks, did);
        let err = manifest.verify_source("different-hash").unwrap_err();
        assert!(matches!(err, IdentityError::ManifestDrift { .. }));
    }

    #[test]
    fn source_matching_passes() {
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"order-agent-seed");
        let manifest = build_sample(&ks, did);
        manifest.verify_source("src-hash-abc").expect("match");
    }

    #[test]
    fn bare_nominal_did_is_rejected_at_verify_time() {
        let ks = InMemoryKeystore::new();
        let did = ks.generate(b"order-agent-seed");
        let mut manifest = build_sample(&ks, did);
        manifest.did = AgentDid::from_raw("order-agent");
        let err = manifest.verify(&ks).unwrap_err();
        assert_eq!(err, IdentityError::InvalidSignature);
    }
}
