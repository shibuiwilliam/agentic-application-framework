//! Automated service registration flow (PROJECT_AafService §3.3).
//!
//! The 7-step registration pipeline automates what operators
//! previously did by hand: validate the capability contract, check
//! the health endpoint, detect conflicts with existing capabilities,
//! assign an initial trust score, attach base policies, and mark the
//! capability as discoverable.
//!
//! ```text
//! 1. Capability Contract authored            ← caller
//! 2. Agent Sidecar / Wrapper configured      ← caller
//! 3. Register with Capability Registry       ← RegistrationPipeline::register
//! 4. Control Plane validates:
//!    a. Schema validity (Rule 9: compensation check)
//!    b. Health-check reachability (simulated in v0.1)
//!    c. Conflict detection against existing capabilities
//! 5. Trust Manager assigns initial Trust Score (autonomy_level=1)
//! 6. Policy Engine attaches relevant policies
//! 7. Registration complete → discoverable
//! ```
//!
//! In v0.1, steps 4b (health check) and 6 (policy attach) are
//! simulated — real health probes and dynamic policy attachment land
//! alongside transport drivers. The pipeline still validates the
//! contract (Rule 9), detects id conflicts, and assigns initial
//! trust, so the core guarantees hold.

use crate::error::RegistryError;
use crate::store::Registry;
use aaf_contracts::CapabilityContract;
use aaf_trust::TrustRegistry;
use std::sync::Arc;
use thiserror::Error;

/// Errors specific to the registration pipeline.
#[derive(Debug, Error)]
pub enum RegistrationError {
    /// Underlying registry error (covers Rule 9 validation).
    #[error("registry: {0}")]
    Registry(#[from] RegistryError),

    /// Conflict: a capability with the same id already exists and is
    /// healthy. Use `force: true` to overwrite.
    #[error("capability `{0}` already registered (use force to overwrite)")]
    Conflict(String),

    /// Health check failed (simulated in v0.1).
    #[error("health check failed for `{0}`: {1}")]
    HealthCheckFailed(String, String),
}

/// Result of a successful registration.
#[derive(Debug, Clone, PartialEq)]
pub struct RegistrationResult {
    /// The registered capability id.
    pub capability_id: String,
    /// Whether the registration overwrote an existing capability.
    pub replaced: bool,
    /// Initial trust score assigned (always 0.5 at autonomy level 1).
    pub initial_trust: f64,
    /// Whether the health check passed (always true in v0.1 unless
    /// a hook fails).
    pub health_ok: bool,
}

/// Health check hook. In v0.1 the default always returns `Ok(())`.
/// Production deployments replace this with a real gRPC/HTTP probe.
pub type HealthCheck = Arc<dyn Fn(&CapabilityContract) -> Result<(), String> + Send + Sync>;

/// 7-step automated registration pipeline.
pub struct RegistrationPipeline {
    registry: Arc<Registry>,
    trust: Arc<TrustRegistry>,
    health_check: HealthCheck,
}

impl RegistrationPipeline {
    /// Construct with default (always-pass) health check.
    pub fn new(registry: Arc<Registry>, trust: Arc<TrustRegistry>) -> Self {
        Self {
            registry,
            trust,
            health_check: Arc::new(|_| Ok(())),
        }
    }

    /// Construct with a custom health check hook.
    pub fn with_health_check(
        registry: Arc<Registry>,
        trust: Arc<TrustRegistry>,
        hc: HealthCheck,
    ) -> Self {
        Self {
            registry,
            trust,
            health_check: hc,
        }
    }

    /// Execute the full 7-step registration.
    ///
    /// - `force`: if `true`, overwrites an existing capability with
    ///   the same id. If `false`, returns `RegistrationError::Conflict`
    ///   when the id is already taken.
    pub async fn register(
        &self,
        cap: CapabilityContract,
        force: bool,
    ) -> Result<RegistrationResult, RegistrationError> {
        let cap_id = cap.id.to_string();

        // ── Step 4a: Schema / contract validation (Rule 9) ────────
        cap.validate().map_err(RegistryError::Invalid)?;

        // ── Step 4b: Health check ─────────────────────────────────
        (self.health_check)(&cap)
            .map_err(|reason| RegistrationError::HealthCheckFailed(cap_id.clone(), reason))?;

        // ── Step 4c: Conflict detection ───────────────────────────
        let existing = self.registry.get(&cap.id).await;
        let replaced = match existing {
            Ok(_) if !force => {
                return Err(RegistrationError::Conflict(cap_id));
            }
            Ok(_) => true,   // force-overwrite
            Err(_) => false, // new registration
        };

        // ── Step 3+5: Register in registry + assign initial trust ─
        self.registry.register(cap).await?;
        let agent_id = aaf_contracts::AgentId::from(cap_id.as_str());
        self.trust.register(agent_id);
        let initial_trust = self
            .trust
            .get(&aaf_contracts::AgentId::from(cap_id.as_str()));

        // ── Step 6: Policy attachment (simulated in v0.1) ─────────
        // In production this calls PolicyEngine::attach_default_pack.
        // v0.1 uses with_default_rules() at the engine level, so
        // every capability is already covered.

        // ── Step 7: Complete ──────────────────────────────────────
        Ok(RegistrationResult {
            capability_id: cap_id,
            replaced,
            initial_trust: initial_trust.value,
            health_ok: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla, CompensationSpec,
        DataClassification, EndpointKind, SideEffect,
    };

    fn read_cap(id: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: "test".into(),
            description: "test cap".into(),
            version: "1.0.0".into(),
            provider_agent: "agent".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::Grpc,
                address: "svc:50051".into(),
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
            required_scope: "test:read".into(),
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

    fn write_cap_without_comp(id: &str) -> CapabilityContract {
        let mut cap = read_cap(id);
        cap.side_effect = SideEffect::Write;
        cap
    }

    fn write_cap_with_comp(id: &str) -> CapabilityContract {
        let mut cap = read_cap(id);
        cap.side_effect = SideEffect::Write;
        cap.compensation = Some(CompensationSpec {
            endpoint: "cap-undo".into(),
        });
        cap
    }

    #[tokio::test]
    async fn successful_registration_returns_initial_trust() {
        let reg = Arc::new(Registry::in_memory());
        let trust = Arc::new(TrustRegistry::new());
        let pipeline = RegistrationPipeline::new(reg, trust);
        let result = pipeline
            .register(read_cap("cap-test"), false)
            .await
            .unwrap();
        assert_eq!(result.capability_id, "cap-test");
        assert!(!result.replaced);
        assert!(result.health_ok);
        assert!((result.initial_trust - 0.5).abs() < 1e-9);
    }

    #[tokio::test]
    async fn duplicate_registration_without_force_errors() {
        let reg = Arc::new(Registry::in_memory());
        let trust = Arc::new(TrustRegistry::new());
        let pipeline = RegistrationPipeline::new(reg, trust);
        pipeline.register(read_cap("cap-dup"), false).await.unwrap();
        let err = pipeline
            .register(read_cap("cap-dup"), false)
            .await
            .unwrap_err();
        assert!(matches!(err, RegistrationError::Conflict(_)));
    }

    #[tokio::test]
    async fn duplicate_registration_with_force_overwrites() {
        let reg = Arc::new(Registry::in_memory());
        let trust = Arc::new(TrustRegistry::new());
        let pipeline = RegistrationPipeline::new(reg, trust);
        pipeline.register(read_cap("cap-dup"), false).await.unwrap();
        let result = pipeline.register(read_cap("cap-dup"), true).await.unwrap();
        assert!(result.replaced);
    }

    #[tokio::test]
    async fn rule_9_rejects_write_without_compensation() {
        let reg = Arc::new(Registry::in_memory());
        let trust = Arc::new(TrustRegistry::new());
        let pipeline = RegistrationPipeline::new(reg, trust);
        let err = pipeline
            .register(write_cap_without_comp("cap-bad"), false)
            .await
            .unwrap_err();
        assert!(matches!(err, RegistrationError::Registry(_)));
    }

    #[tokio::test]
    async fn write_with_compensation_passes() {
        let reg = Arc::new(Registry::in_memory());
        let trust = Arc::new(TrustRegistry::new());
        let pipeline = RegistrationPipeline::new(reg, trust);
        let result = pipeline
            .register(write_cap_with_comp("cap-ok"), false)
            .await
            .unwrap();
        assert_eq!(result.capability_id, "cap-ok");
    }

    #[tokio::test]
    async fn custom_health_check_failure_blocks_registration() {
        let reg = Arc::new(Registry::in_memory());
        let trust = Arc::new(TrustRegistry::new());
        let failing_hc: HealthCheck = Arc::new(|_| Err("connection refused".into()));
        let pipeline = RegistrationPipeline::with_health_check(reg, trust, failing_hc);
        let err = pipeline
            .register(read_cap("cap-unhealthy"), false)
            .await
            .unwrap_err();
        match err {
            RegistrationError::HealthCheckFailed(id, reason) => {
                assert_eq!(id, "cap-unhealthy");
                assert!(reason.contains("connection refused"));
            }
            other => panic!("expected HealthCheckFailed, got {other:?}"),
        }
    }
}
