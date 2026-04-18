# governed-invocation

Demonstrates AAF's **Capability Invocation Bridge** (Pillar 2
Slice A): the path from an agent's tool call to a real service
handler, governed by the policy engine at every step.

## Scenario

An agent needs to search a product catalog and look up prices. When
the LLM requests a tool call, the invocation follows this path:

```text
Agent (LLM decides to call "product search")
  |
  v
ToolExecutor::execute("product search", {"query": "widgets"})
  |
  v
GoverningToolExecutor
  |-- 1. Registry.find_by_name("product search")  --> CapabilityContract
  |-- 2. Build InvocationContext (trace, timeout, scopes)
  |-- 3. ServiceInvoker.invoke(capability, input, ctx)
  |       |
  |       v
  |   InProcessInvoker
  |       |-- look up handler by name
  |       |-- call handler(input)
  |       |-- return InvocationResult { output, latency_ms }
  |
  v
Agent receives: {"products": [{"id": 1, "name": "Widget Pro"}]}
```

## What it exercises

- **GoverningToolExecutor** — bridges the agent's `ToolExecutor`
  trait to real service invocation via the capability registry
- **InProcessInvoker** — closure-based handler registry for
  in-process capabilities (modular monolith pattern)
- **Registry lookup by name** — tool name resolves to a full
  `CapabilityContract` with endpoint metadata
- **Call logging** — every invocation is recorded for observability
- **Handler isolation** — each capability has its own handler with
  typed input/output
- **Error propagation** — unknown capabilities and handler failures
  produce clear `InvocationError` variants
- **Multi-capability dispatch** — multiple services invoked through
  the same executor, each with its own handler

## Files

- `aaf.yaml` — project config with two catalog capabilities

## Run it

```bash
# Run the integration test (verifies full invocation path)
cargo test -p aaf-integration-tests --test governed_invocation_e2e

# Run the CLI demo (uses GoverningToolExecutor internally)
cargo run -p aaf-server -- run examples/governed-invocation/aaf.yaml
```

## Key code paths

- `aaf-runtime/src/invoke.rs` — GoverningToolExecutor, InProcessInvoker,
  ServiceInvoker trait, InvocationContext, InvocationResult
- `aaf-runtime/src/node/agent.rs` — ToolExecutor trait (what agents call)
- `aaf-registry/src/store.rs` — Registry.find_by_name() for tool lookup
