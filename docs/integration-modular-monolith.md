# Integrating AAF with a Modular Monolith

> **Pattern B** from `PROJECT.md` §2.3 — drop AAF alongside an
> existing modular monolith as an **in-process wrapper** around
> module public APIs. Every architectural component in this
> pattern lives in `aaf-wrapper`.

---

## The model

```
┌────────────────────────────────────────────────────────┐
│                 monolith.jar / monolith binary          │
│                                                         │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐     │
│  │  module A   │  │  module B   │  │  module C   │     │
│  └──────▲──────┘  └──────▲──────┘  └──────▲──────┘     │
│         │                │                │            │
│   ┌─────┴──────┐   ┌─────┴──────┐   ┌─────┴──────┐     │
│   │ aaf-wrap.A │   │ aaf-wrap.B │   │ aaf-wrap.C │     │
│   └─────┬──────┘   └─────┬──────┘   └─────┬──────┘     │
└─────────┼────────────────┼────────────────┼────────────┘
          │                │                │
          └────────────────┴────────────────┘
                           │ in-process calls
                           ▼
                     ┌───────────────┐
                     │  aaf-runtime  │
                     └───────────────┘
```

Each module gets an **in-process wrapper** that exposes its public
methods as `Capability`s. The wrapper:

1. Maps module methods to `CapabilityContract`s
   (`aaf-wrapper::capability::MethodToCapability`).
2. Applies input / output / action guards in-process
   (`aaf-wrapper::guard::InProcessGuard`).
3. Intercepts calls and routes them through the runtime.

---

## Zero code changes to the module's public API

**Rule 3: Services Stay Untouched.** The wrapper lives *outside*
the module boundary. If your module exposes `OrderService.cancel`,
the wrapper calls `OrderService.cancel` — it does not require you
to rename it, add a trait, or inject a new parameter.

---

## Decorator-first ergonomics

In Rust this looks like:

```rust
use aaf_wrapper::{capability, guard, compensation};

#[capability(
    id = "cap-order-cancel",
    side_effect = "write",
    reads = ["commerce.Order"],
    writes = ["commerce.Order"],
    compensation = "cap-order-reopen",
)]
pub fn cancel(order_id: &str) -> Result<(), OrderError> {
    // existing business logic — unchanged
}

#[compensation(for = "cap-order-cancel")]
pub fn reopen(order_id: &str) -> Result<(), OrderError> {
    // ...
}

#[guard(output = "pii")]
pub fn list_customers() -> Vec<Customer> {
    // output guard runs PII detection on the result
}
```

The macros produce the glue that registers the capability and
wraps the call with AAF's guards. No manual `GraphBuilder` wiring
required. (Note: the full `#[capability]` / `#[guard]` /
`#[compensation]` proc-macros are scheduled for X3 Slice A; the
`aaf-wrapper` crate today exposes them through a manual builder
API.)

---

## Benefits vs the sidecar pattern

- **Near-zero latency overhead** — no network hop; guards and the
  policy engine run inline.
- **Simpler deployment** — one binary, one process, one config.
- **Static typing** — the wrapper surface is checked at compile
  time, so mismatches between module methods and capability
  declarations are caught by the compiler.

Trade-offs:

- **Single language** — the wrapper runs inside the module's
  process, so it must speak the same language. Rust monoliths
  use `aaf-wrapper` directly; Python / TypeScript monoliths use
  the (deferred) X3 SDKs.
- **Shared failure domain** — if the wrapper panics, the module
  panics. Mitigation: the wrapper's guards return `Result`, not
  panic; runtime failures surface as `RuntimeError`.

---

## Wrapper configuration

The wrapper reads a YAML at startup. Full schema:
`spec/schemas/wrapper-config.schema.json`.

```yaml
wrapper:
  modules:
    - name: orders
      capabilities:
        - id: cap-order-create
          side_effect: write
          reads: [commerce.Customer]
          writes: [commerce.Order]
          emits:  [commerce.OrderPlaced]
          compensation:
            endpoint: cap-order-cancel
          required_scope: orders:write
          data_classification: internal

guards:
  input: { injection: true }
  output: { pii: true }
  action: { scope: true, side_effect_gate: true }

budget:
  max_tokens: 2000
  max_cost_usd: 0.05
  max_latency_ms: 5000
```

Same validation gates as the sidecar. The ontology lint applies
to the same capability YAMLs — run
`aaf-server ontology lint` against them.

---

## The AAF runtime runs in-process

In a modular-monolith integration, `aaf-runtime::GraphExecutor` is
instantiated inside the monolith's own process. Every runtime
component (policy engine, trace recorder, trust manager, registry)
is constructed at startup and shared via `Arc<_>`.

The monolith's entrypoint wires the components:

```rust
let registry = Arc::new(Registry::in_memory());
// ... register every module's capabilities ...

let policy  = Arc::new(PolicyEngine::with_default_rules());
let trace   = Arc::new(Recorder::in_memory());
let memory  = MemoryFacade::in_memory();
let runtime = GraphExecutor::new(policy.clone(), trace.clone(), default_budget);

// Monolith's HTTP handler turns an inbound request into an
// IntentEnvelope, then runs:
let plan    = planner.plan(&intent).await?;
let graph   = build_graph_from_plan(&plan);
let outcome = runtime.run(&graph, &intent).await?;
```

See `core/crates/aaf-server/src/main.rs` for a working reference.

---

## Observability

The monolith emits traces exactly the same way the sidecar does,
via `aaf-trace::Recorder`. OTLP export through
`aaf-trace::otel::OtlpExporter` is configured at startup.

---

## Further reading

- [architecture.md](architecture.md)
- [fast-path.md](fast-path.md) — fast-path rules run in-process
- [integration-microservices.md](integration-microservices.md) —
  compare-and-contrast with Pattern A
- `core/crates/aaf-wrapper/src/` — the implementation
