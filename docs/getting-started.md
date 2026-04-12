# Getting Started

> Build the workspace, run the demo, make one concrete change.
> Time budget: 10 minutes.

---

## Prerequisites

- Rust **1.70+** (Edition 2021). `rustup toolchain install stable`
  will get you a compatible one.
- Python **3.10+** with `jsonschema`, `pyyaml`, `referencing`
  (for the schema validator). `pip install jsonschema pyyaml referencing`.
- `make` (GNU Make 3.81+ or any modern version — the Makefile is
  written to work with stock macOS make).
- **~2 GB** of disk for `target/` after a full build.

No other system dependencies. No Docker, no databases, no
message brokers — the default in-memory backends cover every
integration test.

---

## Clone and build

```bash
# from the repo root
cargo build --workspace
```

The first build pulls pinned versions of every workspace dep and
compiles 22 crates. Expect ~60–120 seconds on a modern laptop.

Verify the tree is green:

```bash
cargo test --workspace
```

You should see **463 tests passing, 0 failures**. If any fail,
stop and open an issue — the tree is supposed to be green at
every committed state.

---

## Run the demo

The `aaf-server` binary is the reference wiring of every core
crate. It takes a YAML config and runs a single intent through
the pipeline: compile → plan → execute → trace.

```bash
cargo run -p aaf-server                              # defaults to ./aaf.yaml
cargo run -p aaf-server -- help                      # show all subcommands
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml
```

Expected output (abbreviated):

```
hello-agent v0.1.0 starting
registered 1 capabilities
compiled intent int-<id> (AnalyticalIntent)
plan: 1 step(s)
✓ completed 1 steps
trace status = Completed, steps recorded = 1
done
```

Other subcommands:

```bash
cargo run -p aaf-server -- validate aaf.yaml
cargo run -p aaf-server -- discover monthly sales
cargo run -p aaf-server -- compile "show last month sales"
cargo run -p aaf-server -- ontology lint spec/examples
cargo run -p aaf-server -- ontology import my-openapi.yaml
```

See [deployment.md](deployment.md) for non-demo deployment options.

---

## Run the 8 runnable examples

AAF ships 8 examples that progressively demonstrate deeper framework
features. Each lives under `examples/<name>/` with an `aaf.yaml` and
a `README.md`:

```bash
# 1. Core pipeline (simplest)
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml

# 2. Saga with compensation
cargo test -p aaf-integration-tests --test order_saga_e2e

# 3. Guards, degradation, budget, approval
cargo test -p aaf-integration-tests --test resilient_query_e2e

# 4. Trust lifecycle + learning
cargo test -p aaf-integration-tests --test feedback_loop_e2e

# 5. Four-layer memory + context budget
cargo test -p aaf-integration-tests --test memory_context_e2e

# 6. App-native events, proposals, projections
cargo test -p aaf-integration-tests --test app_native_surface_e2e

# 7. Cross-cell federation
cargo test -p aaf-integration-tests --test cross_cell_federation_e2e

# 8. Cryptographic identity + SBOM
cargo run -p aaf-server -- identity verify examples/signed-agent/manifest.yaml
```

See `examples/README.md` for the full list with feature coverage and
`development/examples-walkthrough.md` for detailed code patterns.

---

## Explore the shipped contract examples

```
spec/examples/
├── app-event-order-opened.yaml        # E3 Slice A sample AppEvent
├── capability-inventory.yaml          # simple Read capability
├── capability-payment.yaml            # Payment with entity declarations
├── eval-suite-order-processing.yaml   # E1 Slice A golden suite
├── manifest-order-agent.yaml          # X1 Slice A signed agent manifest
├── ontology-commerce.yaml             # E2 Slice A commerce ontology
├── policy-pack-base.yaml              # base policy pack
├── proposal-shipping-fix.yaml         # E3 Slice A sample ActionProposal
├── saga-order-processing.yaml         # saga definition
├── sbom-order-agent.yaml              # X1 Slice A SBOM
└── sidecar-config-order.yaml          # sidecar integration config
```

Every YAML here either validates against a JSON Schema in
`spec/schemas/` (run `make schema-validate`) or is consumed
directly by a crate-level loader.

---

## Make one concrete change

Walk this exercise to ground yourself in the development loop.

### 1. Add an `ontology-lint` check to the server

Open `core/crates/aaf-server/src/lint.rs`. Read the
`classify(cap)` function and the three severity branches.

### 2. Run the lint

```bash
make ontology-lint
```

Expected:

```
scanned: 2, with declarations: 2 (100%), mode: strict
  OK   cap-inventory-check   capability-inventory.yaml  — has entity declarations
  OK   cap-payment-execute   capability-payment.yaml    — has entity declarations

0 errors, 0 warnings
```

### 3. Add a deliberately-broken capability

Create `spec/examples/capability-broken.yaml`:

```yaml
id: cap-broken-payment
name: broken payment
description: deliberately missing an entity declaration
version: 1.0.0
provider_agent: test
endpoint:
  type: grpc
  address: x:0
side_effect: payment
idempotent: true
reversible: true
deterministic: true
compensation:
  endpoint: cap-refund
required_scope: x:write
data_classification: internal
```

### 4. Re-run the lint

```bash
make ontology-lint
```

Expected (strict mode, because adoption drops below 100%? actually
stays above 90% so stays strict):

```
scanned: 3, with declarations: 2 (66%), mode: warn-only
  OK   ...
  OK   ...
  WARN cap-broken-payment  capability-broken.yaml  — capability declares side_effect `payment` but `writes:` is empty; ...
```

The adoption ratio dropped to 66%, so the lint is in
**warn-only** mode — the writer-without-writes finding is
reported as `WARN` and the command exits 0. This is the ramp
behaviour described in [ontology-lint.md](ontology-lint.md).

### 5. Clean up

```bash
rm spec/examples/capability-broken.yaml
make ontology-lint
```

Back to 100% adoption, strict mode, 0 findings.

---

## Five gates you will run a lot

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -W clippy::all
make schema-validate
make ontology-lint
```

Or run all five at once:

```bash
make ci
```

Every gate must stay at zero. If any is red, stop and fix it
before adding new work. See
[../development/build-and-ci.md](../development/build-and-ci.md)
for the complete CI reference.

---

## Where to go next

- [architecture.md](architecture.md) — the 10-minute overview of
  how the crates fit together.
- [contracts.md](contracts.md) — the typed surface.
- [integration-microservices.md](integration-microservices.md) —
  if you plan to drop AAF alongside existing services.
- `../development/README.md` — if you plan to extend the core.
