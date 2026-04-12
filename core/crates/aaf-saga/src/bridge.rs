//! Saga ↔ Registry / Runtime bridge.
//!
//! `SagaExecutor` operates on opaque `StepRunner` / `CompensationRunner`
//! closures so it stays agnostic to how each step actually executes.
//! `RegistryBridge` is the production wiring: it consults the
//! [`aaf_registry::Registry`] for the capability declared by each saga
//! step, hands the call off to a [`CapabilityInvoker`], and translates
//! the invoker's result into a [`StepResult`].
//!
//! The split lets the saga executor stay tested in isolation while
//! servers can plug a real invoker (gRPC client, HTTP client, in-process
//! function call) without touching the saga logic.

use crate::definition::{SagaStep, StepKind};
use crate::executor::{CompensationRunner, StepResult, StepRunner};
use aaf_registry::Registry;
use async_trait::async_trait;
use parking_lot::RwLock;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

/// Errors raised by the bridge during step execution.
#[derive(Debug, Error)]
pub enum BridgeError {
    /// The capability id referenced by the step is unknown.
    #[error("unknown capability: {0}")]
    UnknownCapability(String),
    /// The invoker reported a failure tagged for the recovery rules.
    #[error("invocation tagged failure: {0}")]
    Tagged(String),
}

/// Pluggable invoker that runs a single capability.
///
/// Real deployments back this with a gRPC stub, an HTTP client, or an
/// in-process function pointer. The default
/// [`InMemoryInvoker`] used by tests just records calls and returns
/// whatever was pre-programmed.
#[async_trait]
pub trait CapabilityInvoker: Send + Sync {
    /// Run a capability and return either an opaque success payload or a
    /// failure tag matched against the saga's recovery rules.
    async fn invoke(&self, capability_id: &str) -> Result<serde_json::Value, BridgeError>;
}

/// In-memory invoker used by tests and demos. Stores per-capability
/// outcomes; missing entries default to success with an empty value.
pub struct InMemoryInvoker {
    outcomes: Arc<RwLock<HashMap<String, Result<serde_json::Value, String>>>>,
    log: Arc<RwLock<Vec<String>>>,
}

impl Default for InMemoryInvoker {
    fn default() -> Self {
        Self {
            outcomes: Arc::new(RwLock::new(HashMap::new())),
            log: Arc::new(RwLock::new(vec![])),
        }
    }
}

impl InMemoryInvoker {
    /// Construct.
    pub fn new() -> Self {
        Self::default()
    }

    /// Configure a capability to succeed with the given payload.
    pub fn ok(&self, capability: &str, value: serde_json::Value) {
        self.outcomes
            .write()
            .insert(capability.to_string(), Ok(value));
    }

    /// Configure a capability to fail with the given tag.
    pub fn fail(&self, capability: &str, tag: &str) {
        self.outcomes
            .write()
            .insert(capability.to_string(), Err(tag.to_string()));
    }

    /// Inspect the call log (for tests).
    pub fn calls(&self) -> Vec<String> {
        self.log.read().clone()
    }
}

#[async_trait]
impl CapabilityInvoker for InMemoryInvoker {
    async fn invoke(&self, capability_id: &str) -> Result<serde_json::Value, BridgeError> {
        self.log.write().push(capability_id.to_string());
        let outcome = self
            .outcomes
            .read()
            .get(capability_id)
            .cloned()
            .unwrap_or_else(|| Ok(serde_json::Value::Null));
        match outcome {
            Ok(v) => Ok(v),
            Err(tag) => Err(BridgeError::Tagged(tag)),
        }
    }
}

/// Bridge that produces saga `StepRunner` / `CompensationRunner`
/// closures backed by a [`Registry`] + a [`CapabilityInvoker`].
pub struct RegistryBridge {
    registry: Arc<Registry>,
    invoker: Arc<dyn CapabilityInvoker>,
}

impl RegistryBridge {
    /// Construct a new bridge.
    pub fn new(registry: Arc<Registry>, invoker: Arc<dyn CapabilityInvoker>) -> Self {
        Self { registry, invoker }
    }

    /// Build a `StepRunner` closure that drives `runner.invoke` for each
    /// step. Capability id resolution against the registry happens
    /// up-front when the closure is built so an unknown capability fails
    /// the first time the saga executor invokes the step.
    ///
    /// The closure is `Sync` because it consults `Arc`s only.
    pub fn step_runner(&self) -> StepRunner {
        let registry = self.registry.clone();
        let invoker = self.invoker.clone();
        Arc::new(move |step: &SagaStep| -> StepResult {
            // Validate that the capability is known.
            let cap_id = aaf_contracts::CapabilityId::from(step.capability.as_str());
            let known = futures_block_on(registry.get(&cap_id)).is_ok();
            if !known {
                return StepResult::FailedWithTag("unknown_capability".into());
            }
            match futures_block_on(invoker.invoke(&step.capability)) {
                Ok(_) => StepResult::Ok,
                Err(BridgeError::Tagged(t)) => StepResult::FailedWithTag(t),
                Err(BridgeError::UnknownCapability(_)) => {
                    StepResult::FailedWithTag("unknown_capability".into())
                }
            }
        })
    }

    /// Build a `CompensationRunner` closure that calls `invoker.invoke`
    /// for the step's `compensation` capability id (if any).
    pub fn compensation_runner(&self) -> CompensationRunner {
        let invoker = self.invoker.clone();
        Arc::new(move |step: &SagaStep| -> Result<(), String> {
            let Some(comp) = step.compensation.as_deref() else {
                return Ok(());
            };
            match futures_block_on(invoker.invoke(comp)) {
                Ok(_) => Ok(()),
                Err(e) => Err(e.to_string()),
            }
        })
    }
}

/// Bridge between the synchronous `StepRunner` closure surface and the
/// async `CapabilityInvoker` trait. Uses a current-thread Tokio
/// runtime built once per call so there is no global state. This is
/// fine because saga execution is sequential and rare.
fn futures_block_on<F: std::future::Future>(fut: F) -> F::Output {
    use tokio::runtime::Handle;
    if let Ok(h) = Handle::try_current() {
        // We are inside an async context — use `block_in_place` so we do
        // not stall the executor.
        tokio::task::block_in_place(|| h.block_on(fut))
    } else {
        let rt = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("build current-thread runtime");
        rt.block_on(fut)
    }
}

// Re-export `SagaStep` reference for downstream consumers; needed only
// to keep the `step_runner` / `compensation_runner` types stable.
#[allow(unused_imports)]
use _step_kind_check::*;
mod _step_kind_check {
    use super::StepKind;
    // Compile-time anchor: ensure the executor enum we expect is in
    // scope so a future rename of StepKind breaks here loudly.
    #[allow(dead_code)]
    fn _anchor(k: StepKind) -> StepKind {
        k
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::definition::{CompensationType, SagaDefinition, SagaStep, StepKind};
    use crate::executor::{SagaExecutor, SagaOutcome};
    use aaf_contracts::{
        CapabilityContract, CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla,
        CompensationSpec, DataClassification, EndpointKind, SideEffect,
    };

    fn write_cap(id: &str, comp_id: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(id),
            name: id.into(),
            description: id.into(),
            version: "1.0".into(),
            provider_agent: "agent".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::InProcess,
                address: id.into(),
                method: None,
            },
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            side_effect: SideEffect::Write,
            idempotent: false,
            reversible: true,
            deterministic: true,
            compensation: Some(CompensationSpec {
                endpoint: comp_id.into(),
            }),
            sla: CapabilitySla::default(),
            cost: CapabilityCost::default(),
            required_scope: "x:write".into(),
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

    fn read_cap(id: &str) -> CapabilityContract {
        let mut c = write_cap(id, "noop");
        c.side_effect = SideEffect::Read;
        c.compensation = None;
        c
    }

    fn step(n: u32, cap: &str, comp: Option<&str>) -> SagaStep {
        SagaStep {
            step: n,
            name: format!("step-{n}"),
            kind: StepKind::Deterministic,
            capability: cap.into(),
            compensation: comp.map(|s| s.to_string()),
            compensation_type: CompensationType::Mandatory,
            on_failure: None,
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn end_to_end_saga_via_bridge_succeeds() {
        let registry = Arc::new(Registry::in_memory());
        registry
            .register(write_cap("cap-a", "cap-a-comp"))
            .await
            .unwrap();
        registry.register(read_cap("cap-a-comp")).await.unwrap();
        registry
            .register(write_cap("cap-b", "cap-b-comp"))
            .await
            .unwrap();
        registry.register(read_cap("cap-b-comp")).await.unwrap();

        let invoker = Arc::new(InMemoryInvoker::new());
        invoker.ok("cap-a", serde_json::json!({"ok": true}));
        invoker.ok("cap-b", serde_json::json!({"ok": true}));

        let bridge = RegistryBridge::new(registry, invoker.clone());
        let mut exec = SagaExecutor::new(bridge.step_runner(), bridge.compensation_runner());
        let outcome = exec
            .run(&SagaDefinition {
                name: "x".into(),
                description: None,
                steps: vec![
                    step(1, "cap-a", Some("cap-a-comp")),
                    step(2, "cap-b", Some("cap-b-comp")),
                ],
            })
            .unwrap();
        assert!(matches!(outcome, SagaOutcome::Completed { .. }));
        assert_eq!(invoker.calls(), vec!["cap-a", "cap-b"]);
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn unknown_capability_surfaces_as_failure_tag() {
        let registry = Arc::new(Registry::in_memory());
        // cap-a registered, cap-b is missing.
        registry
            .register(write_cap("cap-a", "cap-a-comp"))
            .await
            .unwrap();
        registry.register(read_cap("cap-a-comp")).await.unwrap();

        let invoker = Arc::new(InMemoryInvoker::new());
        invoker.ok("cap-a", serde_json::Value::Null);
        let bridge = RegistryBridge::new(registry, invoker);
        let mut exec = SagaExecutor::new(bridge.step_runner(), bridge.compensation_runner());

        let outcome = exec
            .run(&SagaDefinition {
                name: "x".into(),
                description: None,
                steps: vec![
                    step(1, "cap-a", Some("cap-a-comp")),
                    step(2, "cap-b", Some("cap-b-comp")),
                ],
            })
            .unwrap();
        match outcome {
            SagaOutcome::Failed { failed_at, .. } => assert_eq!(failed_at, 2),
            other => panic!("expected Failed, got {other:?}"),
        }
    }
}
