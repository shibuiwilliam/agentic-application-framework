# Examples Walkthrough

> How to run, read, and extend the 13 runnable examples.

Each example lives under `examples/<name>/` and has an `aaf.yaml`
configuration plus a `README.md`. Every example (except `hello-agent`
and `signed-agent`, which use the server binary) has a corresponding
end-to-end test under `core/tests/integration/tests/<name>_e2e.rs`.

---

## Progression order

The examples build on each other conceptually. Read them in this
order:

| # | Example | What it teaches | Tests |
|---|---|---|---|
| 1 | `hello-agent` | Core pipeline: intent → plan → execute → trace | (run via `aaf-server`) |
| 2 | `order-saga` | Multi-step saga, compensation, shadow mode | 4 |
| 3 | `resilient-query` | Fast-path, guards (PII/injection), degradation, budget, approval | 15 |
| 4 | `feedback-loop` | Trust lifecycle (5-level autonomy, promotion/demotion), learning subscribers | 21 |
| 5 | `memory-context` | Four-layer memory model, context budget (Rule 10) | 20 |
| 6 | `app-native-surface` | Events → proposals → lifecycle, projections (Rules 19/20) | 23 |
| 7 | `cross-cell-federation` | Cell routing, data boundaries, co-signed tokens | 7 |
| 8 | `signed-agent` | Cryptographic identity, manifests, SBOM, revocation | (run via `aaf-server`) |
| 9 | `eval-golden` | Golden suite loading, judge scoring, regression reports (E1) | (via integration test) |
| 10 | `agentic-tool-loop` | Multi-turn tool calling, bounded agentic loop (E4) | (via `e4_tool_loop_smoke`) |
| 11 | `parallel-orchestration` | ForkNode, diamond DAG, parallel execution, compensation | (via `parallel_orchestration_e2e`) |
| 12 | `sidecar-gateway` | Sidecar proxy, Rule 13 transparent fallback, ACL, guards | (via `sidecar_gateway_e2e`) |
| 13 | `governed-invocation` | Capability invocation bridge, GoverningToolExecutor, InProcessInvoker | (via `governed_invocation_e2e`) |

---

## Running them

### All integration-test-based examples at once

```bash
cargo test -p aaf-integration-tests
```

### Individual examples

```bash
# Pipeline examples (server binary)
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml
cargo run -p aaf-server -- identity verify examples/signed-agent/manifest.yaml

# Test-based examples (one at a time)
cargo test -p aaf-integration-tests --test order_saga_e2e
cargo test -p aaf-integration-tests --test resilient_query_e2e
cargo test -p aaf-integration-tests --test feedback_loop_e2e
cargo test -p aaf-integration-tests --test memory_context_e2e
cargo test -p aaf-integration-tests --test app_native_surface_e2e
cargo test -p aaf-integration-tests --test cross_cell_federation_e2e
cargo test -p aaf-integration-tests --test eval_golden_e2e
```

---

## Architecture rules exercised by each example

| Rule | hello | saga | resilient | feedback | memory | surface | federation | signed | eval |
|---|---|---|---|---|---|---|---|---|---|
| R1 Agents translate | | | | | | | | | |
| R4 Fast path first | | | x | | | | | | |
| R5 Deterministic sacred | | x | | | | | | | |
| R6 Policy at every step | | x | x | | | | | | |
| R7 Guard every agent | | | x | | | | | | |
| R8 Budget limits | | | x | | | | | | |
| R9 Compensation first | | x | | | | | | | |
| R10 Context minimization | | | | | x | | | | |
| R11 Storage behind traits | | | | | x | | | | |
| R12 Trace everything | x | x | x | | | | | | x |
| R14 Semantics are nouns | | | | | x | | | | |
| R15 Feedback is a contract | | | | x | | | | | x |
| R16 Learning off hot path | | | | x | | | | | |
| R18 Policy governs learning | | | | x | | | | | |
| R19 Projections default-deny | | | | | | x | | | |
| R20 Proposals not mutations | | | | | | x | | | |
| R21 Entities tenant-scoped | | | | | x | | x | | |
| R22 Identity cryptographic | | | | | | | x | x | |
| R23 Signed manifest | | | | | | | | x | |
| R24 Provenance as BOM | | | | | | | | x | |

---

## How to add a new example

1. Create `examples/<name>/` with `aaf.yaml` + `README.md`.
2. Create `core/tests/integration/tests/<name>_e2e.rs`.
3. Add any new crate dependencies to
   `core/tests/integration/Cargo.toml`.
4. Update `examples/README.md` with a summary + run instructions.
5. Run `cargo test -p aaf-integration-tests` to verify.

### Anatomy of a good example test

```rust
// 1. Helper functions for constructing test data
fn intent() -> IntentEnvelope { /* ... */ }
fn det(id: &str, se: SideEffect, output: Value) -> Arc<dyn Node> { /* ... */ }

// 2. Test names describe what they prove
#[tokio::test]
async fn happy_path_completes_three_steps_with_trace() { /* ... */ }

// 3. YAML config parse test validates the example's aaf.yaml
#[test]
fn aaf_yaml_loads_successfully() { /* ... */ }
```

---

## Common patterns across examples

### Creating a test intent

```rust
IntentEnvelope {
    intent_id: IntentId::new(),
    intent_type: IntentType::AnalyticalIntent,
    requester: Requester {
        user_id: "user-tanaka".into(),
        role: "analyst".into(),
        scopes: vec!["sales:read".into()],
        tenant: None,
    },
    goal: "show me the data".into(),
    domain: "sales".into(),
    constraints: Default::default(),
    budget: BudgetContract { max_tokens: 5_000, max_cost_usd: 1.0, max_latency_ms: 30_000 },
    // ... remaining fields with defaults
}
```

### Creating a deterministic node

```rust
Arc::new(DeterministicNode::new(
    NodeId::from("step-name"),
    SideEffect::Read,
    Arc::new(move |_, _| Ok(serde_json::json!({"result": "ok"}))),
))
```

### Building and executing a graph

```rust
let recorder: Arc<dyn TraceRecorder> = Arc::new(Recorder::in_memory());
let exe = GraphExecutor::new(
    Arc::new(PolicyEngine::with_default_rules()),
    recorder.clone(),
    intent.budget,
);
let graph = GraphBuilder::new().add_node(node).build().unwrap();
let outcome = exe.run(&graph, &intent).await.unwrap();
```
