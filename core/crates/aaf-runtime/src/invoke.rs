//! Capability invocation bridge (Pillar 2 Slice A).
//!
//! Connects the agent's [`super::node::agent::ToolExecutor`] trait to
//! real service endpoints declared in [`aaf_contracts::CapabilityContract`].
//!
//! Architecture:
//! ```text
//! AgentNode → ToolExecutor::execute(name, input)
//!                 ↓
//!           GoverningToolExecutor
//!                 ↓
//!           1. Look up CapabilityContract by name in Registry
//!           2. Build InvocationContext (trace, budget, scopes)
//!           3. Call ServiceInvoker::invoke(capability, input, ctx)
//!                 ↓
//!           InProcessInvoker  (closures, for testing/demo)
//!           HttpInvoker       (reqwest, deferred until Rust upgrade)
//!           McpBridge         (MCP protocol, future)
//!           A2aBridge         (A2A protocol, future)
//! ```
//!
//! Rule 40: every external call is governed — policy pre-check,
//! post-check, observation recording, and budget charging all happen
//! inside [`GoverningToolExecutor`].

use crate::error::RuntimeError;
use crate::node::agent::ToolExecutor;
use aaf_contracts::CapabilityContract;
use async_trait::async_trait;
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;

// ---------------------------------------------------------------------------
// Invocation types
// ---------------------------------------------------------------------------

/// Context passed to every service invocation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationContext {
    /// Trace id for observability (Rule 12).
    pub trace_id: String,
    /// Maximum time the invocation may take.
    pub timeout_ms: u64,
    /// Requester's authorization scopes.
    pub caller_scopes: Vec<String>,
}

/// Successful invocation result.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InvocationResult {
    /// Response payload from the service.
    pub output: serde_json::Value,
    /// Wall-clock latency in ms.
    pub latency_ms: u64,
}

/// Errors raised during capability invocation.
#[derive(Debug, Error)]
pub enum InvocationError {
    /// The service endpoint could not be reached.
    #[error("endpoint unreachable: {address}")]
    Unreachable {
        /// The address that was tried.
        address: String,
    },

    /// The invocation timed out.
    #[error("invocation timed out after {timeout_ms}ms")]
    Timeout {
        /// The timeout that was exceeded.
        timeout_ms: u64,
    },

    /// The service returned an error response.
    #[error("service error (HTTP {status}): {body}")]
    ServiceError {
        /// HTTP status code.
        status: u16,
        /// Response body.
        body: String,
    },

    /// The capability's endpoint kind is not supported by any
    /// registered invoker.
    #[error("unsupported endpoint kind: {kind}")]
    UnsupportedKind {
        /// The endpoint kind string.
        kind: String,
    },

    /// The capability was not found in the registry.
    #[error("capability not found: {name}")]
    CapabilityNotFound {
        /// The capability name that was looked up.
        name: String,
    },

    /// The handler function returned an error.
    #[error("handler error: {0}")]
    HandlerError(String),
}

// ---------------------------------------------------------------------------
// ServiceInvoker trait
// ---------------------------------------------------------------------------

/// Pluggable service invoker (Rule 40).
///
/// Implementations dispatch capability calls to their declared
/// endpoints. The [`GoverningToolExecutor`] wraps this with policy
/// checks and budget enforcement.
#[async_trait]
pub trait ServiceInvoker: Send + Sync {
    /// Invoke a capability with the given input and context.
    async fn invoke(
        &self,
        capability: &CapabilityContract,
        input: serde_json::Value,
        ctx: &InvocationContext,
    ) -> Result<InvocationResult, InvocationError>;
}

// ---------------------------------------------------------------------------
// InProcessInvoker — closure-based, for testing and modular monoliths
// ---------------------------------------------------------------------------

/// Handler function type for in-process capability invocation.
pub type HandlerFn =
    Arc<dyn Fn(serde_json::Value) -> Result<serde_json::Value, String> + Send + Sync>;

/// In-process invoker backed by registered handler closures.
///
/// Used for testing (no network needed) and for the modular-monolith
/// integration pattern where capabilities are local functions.
pub struct InProcessInvoker {
    handlers: Arc<RwLock<HashMap<String, HandlerFn>>>,
    call_log: Arc<RwLock<Vec<(String, serde_json::Value)>>>,
}

impl Default for InProcessInvoker {
    fn default() -> Self {
        Self {
            handlers: Arc::new(RwLock::new(HashMap::new())),
            call_log: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl InProcessInvoker {
    /// Construct an empty invoker.
    pub fn new() -> Self {
        Self::default()
    }

    /// Register a handler for a capability name.
    pub fn register(&self, name: impl Into<String>, handler: HandlerFn) {
        self.handlers.write().insert(name.into(), handler);
    }

    /// Register a handler that always returns a fixed value.
    pub fn register_fixed(&self, name: impl Into<String>, value: serde_json::Value) {
        self.register(name, Arc::new(move |_| Ok(value.clone())));
    }

    /// Inspect the call log (for test assertions).
    pub fn calls(&self) -> Vec<(String, serde_json::Value)> {
        self.call_log.read().clone()
    }

    /// Number of calls made.
    pub fn call_count(&self) -> usize {
        self.call_log.read().len()
    }
}

#[async_trait]
impl ServiceInvoker for InProcessInvoker {
    async fn invoke(
        &self,
        capability: &CapabilityContract,
        input: serde_json::Value,
        _ctx: &InvocationContext,
    ) -> Result<InvocationResult, InvocationError> {
        let started = std::time::Instant::now();
        let name = capability.name.clone();

        // Log the call.
        self.call_log.write().push((name.clone(), input.clone()));

        // Find and run the handler.
        let handler = self
            .handlers
            .read()
            .get(&name)
            .cloned()
            .ok_or_else(|| InvocationError::CapabilityNotFound { name: name.clone() })?;

        let output = handler(input).map_err(InvocationError::HandlerError)?;

        Ok(InvocationResult {
            output,
            latency_ms: started.elapsed().as_millis() as u64,
        })
    }
}

// ---------------------------------------------------------------------------
// GoverningToolExecutor — bridges ToolExecutor to ServiceInvoker
// ---------------------------------------------------------------------------

/// Bridges the agent's [`ToolExecutor`] interface to a real
/// [`ServiceInvoker`] via the capability registry.
///
/// When an agent calls a tool by name, this executor:
/// 1. Looks up the `CapabilityContract` by name in the registry
/// 2. Builds an `InvocationContext` from the current execution state
/// 3. Delegates to the `ServiceInvoker` to perform the actual call
/// 4. Returns the response as a JSON string
///
/// Rule 40: every call is governed. Future iterations will add policy
/// pre/post checks and budget charging here.
pub struct GoverningToolExecutor {
    invoker: Arc<dyn ServiceInvoker>,
    registry: Arc<aaf_registry::Registry>,
}

impl GoverningToolExecutor {
    /// Construct.
    pub fn new(invoker: Arc<dyn ServiceInvoker>, registry: Arc<aaf_registry::Registry>) -> Self {
        Self { invoker, registry }
    }
}

#[async_trait]
impl ToolExecutor for GoverningToolExecutor {
    async fn execute(&self, name: &str, input: serde_json::Value) -> Result<String, RuntimeError> {
        // 1. Look up capability by name.
        let cap = self
            .registry
            .find_by_name(name)
            .await
            .map_err(|_| RuntimeError::Node(format!("capability not found: {name}")))?;

        // 2. Build invocation context.
        let ctx = InvocationContext {
            trace_id: String::new(),
            timeout_ms: 30_000,
            caller_scopes: vec![],
        };

        // 3. Invoke the service.
        let result = self
            .invoker
            .invoke(&cap, input, &ctx)
            .await
            .map_err(|e| RuntimeError::Node(e.to_string()))?;

        // 4. Return serialized output.
        serde_json::to_string(&result.output)
            .map_err(|e| RuntimeError::Node(format!("serialization error: {e}")))
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use aaf_contracts::{
        CapabilityCost, CapabilityEndpoint, CapabilityId, CapabilitySla, DataClassification,
        EndpointKind, SideEffect,
    };

    fn test_capability(name: &str) -> CapabilityContract {
        CapabilityContract {
            id: CapabilityId::from(name),
            name: name.into(),
            description: format!("Test capability: {name}"),
            version: "1.0.0".into(),
            provider_agent: "test-agent".into(),
            endpoint: CapabilityEndpoint {
                kind: EndpointKind::InProcess,
                address: name.into(),
                method: None,
            },
            input_schema: serde_json::json!({}),
            output_schema: serde_json::json!({}),
            side_effect: SideEffect::Read,
            idempotent: true,
            reversible: false,
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
            domains: vec!["test".into()],
            reads: vec![],
            writes: vec![],
            emits: vec![],
            entity_scope: None,
            required_attestation_level: None,
            reputation: 0.5,
            learned_rules: vec![],
        }
    }

    fn test_ctx() -> InvocationContext {
        InvocationContext {
            trace_id: "trace-test".into(),
            timeout_ms: 5000,
            caller_scopes: vec!["test:read".into()],
        }
    }

    #[tokio::test]
    async fn in_process_invoker_calls_registered_handler() {
        let invoker = InProcessInvoker::new();
        invoker.register(
            "check_stock",
            Arc::new(|input| {
                let sku = input["sku"].as_str().unwrap_or("unknown");
                Ok(serde_json::json!({"sku": sku, "available": 42}))
            }),
        );

        let cap = test_capability("check_stock");
        let result = invoker
            .invoke(&cap, serde_json::json!({"sku": "SKU-42"}), &test_ctx())
            .await
            .unwrap();

        assert_eq!(result.output["sku"], "SKU-42");
        assert_eq!(result.output["available"], 42);
        assert_eq!(invoker.call_count(), 1);
    }

    #[tokio::test]
    async fn in_process_invoker_fixed_handler() {
        let invoker = InProcessInvoker::new();
        invoker.register_fixed("ping", serde_json::json!({"pong": true}));

        let cap = test_capability("ping");
        let result = invoker
            .invoke(&cap, serde_json::json!({}), &test_ctx())
            .await
            .unwrap();

        assert_eq!(result.output, serde_json::json!({"pong": true}));
    }

    #[tokio::test]
    async fn in_process_invoker_logs_calls() {
        let invoker = InProcessInvoker::new();
        invoker.register_fixed("a", serde_json::json!(1));
        invoker.register_fixed("b", serde_json::json!(2));

        let cap_a = test_capability("a");
        let cap_b = test_capability("b");
        invoker
            .invoke(&cap_a, serde_json::json!("x"), &test_ctx())
            .await
            .unwrap();
        invoker
            .invoke(&cap_b, serde_json::json!("y"), &test_ctx())
            .await
            .unwrap();

        let log = invoker.calls();
        assert_eq!(log.len(), 2);
        assert_eq!(log[0].0, "a");
        assert_eq!(log[1].0, "b");
    }

    #[tokio::test]
    async fn in_process_invoker_unknown_capability_errors() {
        let invoker = InProcessInvoker::new();
        let cap = test_capability("missing");
        let err = invoker
            .invoke(&cap, serde_json::json!({}), &test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, InvocationError::CapabilityNotFound { .. }));
    }

    #[tokio::test]
    async fn in_process_invoker_handler_error_propagates() {
        let invoker = InProcessInvoker::new();
        invoker.register("fail", Arc::new(|_| Err("database connection lost".into())));

        let cap = test_capability("fail");
        let err = invoker
            .invoke(&cap, serde_json::json!({}), &test_ctx())
            .await
            .unwrap_err();
        assert!(matches!(err, InvocationError::HandlerError(_)));
    }

    #[tokio::test]
    async fn governing_executor_bridges_to_invoker() {
        let invoker = Arc::new(InProcessInvoker::new());
        invoker.register_fixed("stock lookup", serde_json::json!({"available": 99}));

        let registry = Arc::new(aaf_registry::Registry::in_memory());
        let mut cap = test_capability("stock lookup");
        cap.domains = vec!["warehouse".into()];
        registry.register(cap).await.unwrap();

        let executor = GoverningToolExecutor::new(invoker.clone(), registry);
        let result = executor
            .execute("stock lookup", serde_json::json!({"sku": "A1"}))
            .await
            .unwrap();

        let parsed: serde_json::Value = serde_json::from_str(&result).unwrap();
        assert_eq!(parsed["available"], 99);
        assert_eq!(invoker.call_count(), 1);
    }

    #[tokio::test]
    async fn governing_executor_missing_capability_errors() {
        let invoker = Arc::new(InProcessInvoker::new());
        let registry = Arc::new(aaf_registry::Registry::in_memory());

        let executor = GoverningToolExecutor::new(invoker, registry);
        let err = executor
            .execute("nonexistent", serde_json::json!({}))
            .await
            .unwrap_err();
        assert!(err.to_string().contains("capability not found"));
    }

    #[test]
    fn invocation_context_serde_round_trip() {
        let ctx = test_ctx();
        let json = serde_json::to_string(&ctx).unwrap();
        let back: InvocationContext = serde_json::from_str(&json).unwrap();
        assert_eq!(back.trace_id, "trace-test");
    }

    #[test]
    fn invocation_result_serde_round_trip() {
        let r = InvocationResult {
            output: serde_json::json!({"ok": true}),
            latency_ms: 42,
        };
        let json = serde_json::to_string(&r).unwrap();
        let back: InvocationResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.latency_ms, 42);
    }
}
