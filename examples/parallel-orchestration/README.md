# parallel-orchestration

Demonstrates AAF's **parallel execution** with Fork nodes and
**diamond-shaped graph topology** — a realistic multi-service
orchestration pattern where independent checks run concurrently
before a final confirmation step.

## Scenario

An e-commerce order fulfillment pipeline:

```text
  validate-order          (sequential, deterministic)
       |
  +----+-----+------+
  |          |      |
  check      verify  validate
  inventory  payment address    (parallel via ForkNode)
  |          |      |
  +----+-----+------+
       |
  confirm-order               (sequential, deterministic, write)
```

1. **validate-order** — checks order structure and business rules
2. **check-inventory / verify-payment / validate-address** — three
   independent checks run in parallel via a `ForkNode`, reducing
   total latency
3. **confirm-order** — finalizes the order after all checks pass
   (write side-effect with compensation)

## What it exercises

- **ForkNode** parallel execution: three child nodes run concurrently
  via `tokio::spawn`, outputs joined into a single map
- **Diamond-shaped DAG**: validate → fork → confirm, validated by
  Kahn's topological sort
- **Mixed node types**: sequential deterministic nodes + parallel fork
- **Budget tracking across branches**: tokens and cost accumulate
  from all parallel children
- **Compensation for write steps**: confirm-order has a compensator
  that runs on downstream failure (Rule 9)
- **Trace recording**: every node (including fork children) produces
  an observation (Rule 12)
- **Policy enforcement**: all four policy hooks fire at each step
  (Rule 6)

## Run it

```bash
# Run the integration test
cargo test -p aaf-integration-tests --test parallel_orchestration_e2e

# Validate the config
cargo run -p aaf-server -- validate examples/parallel-orchestration/aaf.yaml
```

## Key code paths

- `aaf-runtime/src/node/fork.rs` — ForkNode implementation
- `aaf-runtime/src/graph.rs` — DAG validation with Kahn's algorithm
- `aaf-runtime/src/executor.rs` — graph execution with policy hooks
- `aaf-runtime/src/compensation.rs` — compensation chain for rollback
