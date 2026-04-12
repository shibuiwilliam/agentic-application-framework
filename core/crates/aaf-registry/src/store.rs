//! Registry CRUD facade over an [`aaf_storage::RegistryStore`].

use crate::error::RegistryError;
use aaf_contracts::learn::LearnedRuleRef;
use aaf_contracts::{AttestationLevelRef, CapabilityContract, CapabilityId};
use aaf_storage::RegistryStore;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use std::collections::HashMap;
use std::sync::Arc;

/// Minimum interval between reputation updates for a single
/// capability. Prevents adversarial oscillation (E1 §2.8).
fn reputation_rate_limit() -> chrono::Duration {
    chrono::Duration::seconds(60)
}

/// High-level capability registry.
pub struct Registry {
    store: Arc<dyn RegistryStore>,
    /// Last-update timestamp per capability id for rate-limiting
    /// reputation writes.
    reputation_rate: Arc<Mutex<HashMap<CapabilityId, DateTime<Utc>>>>,
}

impl std::fmt::Debug for Registry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Registry").finish_non_exhaustive()
    }
}

impl Registry {
    /// Wrap a storage backend.
    pub fn new(store: Arc<dyn RegistryStore>) -> Self {
        Self {
            store,
            reputation_rate: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Build an in-memory registry for tests / dev.
    pub fn in_memory() -> Self {
        Self::new(Arc::new(aaf_storage::InMemoryRegistryStore::new()))
    }

    /// Insert or replace a capability after validating it (Rule 9).
    pub async fn register(&self, cap: CapabilityContract) -> Result<(), RegistryError> {
        cap.validate()?;
        self.store.upsert(cap).await?;
        Ok(())
    }

    /// Fetch by id.
    pub async fn get(&self, id: &CapabilityId) -> Result<CapabilityContract, RegistryError> {
        Ok(self.store.get(id).await?)
    }

    /// Fetch a capability only if the caller's presented attestation
    /// level meets the capability's declared
    /// `required_attestation_level`. Capabilities without a declared
    /// level are served unconditionally (preserves Wave 1 behaviour).
    ///
    /// Rule 23 (Wave 2 X1 Slice B): returns
    /// [`RegistryError::InsufficientAttestation`] when the caller
    /// cannot clear the bar.
    pub async fn get_for_attestation(
        &self,
        id: &CapabilityId,
        presented: AttestationLevelRef,
    ) -> Result<CapabilityContract, RegistryError> {
        let cap = self.store.get(id).await?;
        if let Some(required) = cap.required_attestation_level {
            if presented < required {
                return Err(RegistryError::InsufficientAttestation {
                    required,
                    presented,
                });
            }
        }
        Ok(cap)
    }

    /// Delete by id.
    pub async fn deregister(&self, id: &CapabilityId) -> Result<(), RegistryError> {
        Ok(self.store.delete(id).await?)
    }

    /// List every capability.
    pub async fn list(&self) -> Result<Vec<CapabilityContract>, RegistryError> {
        Ok(self.store.list().await?)
    }

    /// Update the reputation score for a capability, clamping to
    /// `[0.0, 1.0]`. Rate-limited to one update per capability per
    /// 60 seconds to prevent adversarial oscillation (E1 §2.8).
    ///
    /// Returns `Ok(())` if the update was applied, or
    /// `Err(RegistryError::RateLimited)` if the rate limit was hit.
    pub async fn update_reputation(
        &self,
        id: &CapabilityId,
        new_score: f32,
        evidence: LearnedRuleRef,
    ) -> Result<(), RegistryError> {
        // Rate limit check.
        {
            let mut guard = self.reputation_rate.lock();
            let now = Utc::now();
            if let Some(last) = guard.get(id) {
                if now.signed_duration_since(*last) < reputation_rate_limit() {
                    return Err(RegistryError::RateLimited(id.to_string()));
                }
            }
            guard.insert(id.clone(), now);
        }

        let mut cap = self.store.get(id).await?;
        cap.reputation = new_score.clamp(0.0, 1.0);
        cap.learned_rules.push(evidence);
        self.store.upsert(cap).await?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        CapabilityCost, CapabilityEndpoint, CapabilitySla, CompensationSpec, DataClassification,
        EndpointKind, SideEffect,
    };

    fn read_cap() -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from("cap-readonly"),
            name: "readonly".into(),
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

    #[tokio::test]
    async fn rejects_write_capability_without_compensation() {
        let r = Registry::in_memory();
        let mut cap = read_cap();
        cap.side_effect = SideEffect::Write;
        let err = r.register(cap).await.unwrap_err();
        assert!(matches!(err, RegistryError::Invalid(_)));
    }

    #[tokio::test]
    async fn accepts_write_capability_with_compensation() {
        let r = Registry::in_memory();
        let mut cap = read_cap();
        cap.side_effect = SideEffect::Write;
        cap.compensation = Some(CompensationSpec {
            endpoint: "cap-undo".into(),
        });
        r.register(cap.clone()).await.unwrap();
        let got = r.get(&cap.id).await.unwrap();
        assert_eq!(got.id, cap.id);
    }

    // ── Wave 2 X1 Slice B — attestation gate ───────────────────────────

    #[tokio::test]
    async fn attestation_gate_serves_when_presented_meets_required() {
        let r = Registry::in_memory();
        let mut cap = read_cap();
        cap.required_attestation_level = Some(AttestationLevelRef::AutoVerified);
        r.register(cap.clone()).await.unwrap();

        // Caller presents HumanReviewed, which is ≥ AutoVerified.
        let got = r
            .get_for_attestation(&cap.id, AttestationLevelRef::HumanReviewed)
            .await
            .unwrap();
        assert_eq!(got.id, cap.id);
    }

    #[tokio::test]
    async fn attestation_gate_refuses_when_presented_is_too_weak() {
        let r = Registry::in_memory();
        let mut cap = read_cap();
        cap.required_attestation_level = Some(AttestationLevelRef::Certified);
        r.register(cap.clone()).await.unwrap();

        // Caller presents Unattested — far below Certified.
        let err = r
            .get_for_attestation(&cap.id, AttestationLevelRef::Unattested)
            .await
            .unwrap_err();
        match err {
            RegistryError::InsufficientAttestation {
                required,
                presented,
            } => {
                assert_eq!(required, AttestationLevelRef::Certified);
                assert_eq!(presented, AttestationLevelRef::Unattested);
            }
            other => panic!("expected InsufficientAttestation, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn capabilities_without_required_level_are_unconditional() {
        let r = Registry::in_memory();
        let cap = read_cap(); // required_attestation_level: None
        r.register(cap.clone()).await.unwrap();

        // Even the weakest presented level should succeed.
        let got = r
            .get_for_attestation(&cap.id, AttestationLevelRef::Unattested)
            .await
            .unwrap();
        assert_eq!(got.id, cap.id);
    }

    // ── E1 Slice B — reputation ─────────────────────────────────────

    fn evidence_ref() -> LearnedRuleRef {
        use aaf_contracts::learn::LearnedSource;
        LearnedRuleRef {
            learned_rule_id: "lr-test".into(),
            learned_by: LearnedSource::Miner,
            learned_at: Utc::now(),
            evidence_count: 5,
        }
    }

    #[tokio::test]
    async fn reputation_update_persists() {
        let r = Registry::in_memory();
        r.register(read_cap()).await.unwrap();
        r.update_reputation(&CapabilityId::from("cap-readonly"), 0.85, evidence_ref())
            .await
            .unwrap();
        let cap = r.get(&CapabilityId::from("cap-readonly")).await.unwrap();
        assert!((cap.reputation - 0.85).abs() < 1e-6);
        assert_eq!(cap.learned_rules.len(), 1);
    }

    #[tokio::test]
    async fn reputation_clamps_above_one() {
        let r = Registry::in_memory();
        r.register(read_cap()).await.unwrap();
        r.update_reputation(&CapabilityId::from("cap-readonly"), 1.5, evidence_ref())
            .await
            .unwrap();
        let cap = r.get(&CapabilityId::from("cap-readonly")).await.unwrap();
        assert!((cap.reputation - 1.0).abs() < 1e-6);
    }

    #[tokio::test]
    async fn reputation_rate_limited() {
        let r = Registry::in_memory();
        r.register(read_cap()).await.unwrap();
        r.update_reputation(&CapabilityId::from("cap-readonly"), 0.7, evidence_ref())
            .await
            .unwrap();
        let err = r
            .update_reputation(&CapabilityId::from("cap-readonly"), 0.8, evidence_ref())
            .await
            .unwrap_err();
        assert!(matches!(err, RegistryError::RateLimited(_)));
    }
}
