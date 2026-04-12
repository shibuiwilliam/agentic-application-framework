# Deployment

> How to build and deploy the shipping pieces of AAF. Real Helm
> charts / Dockerfiles / Terraform modules are **deferred** to a
> future iteration; this document describes what exists today and
> the operational contract each component must satisfy.

---

## What ships today

| Component | Binary / artefact | Status |
|---|---|---|
| `aaf-server` | `target/release/aaf-server` | Shippable as a single static binary |
| `aaf-sidecar` | linked into `aaf-server` | Library crate; standalone binary deferred |
| `aaf-wrapper` | linked into host monolith | Library crate only |
| Helm charts | `deploy/helm/` | Directory reserved; charts deferred |
| Docker images | `deploy/docker/` | Directory reserved; Dockerfiles deferred |
| Terraform | `deploy/terraform/` | Directory reserved; modules deferred |

Today the canonical way to run AAF in anger is to `cargo build
--release` the `aaf-server` binary, stage the `aaf.yaml` config
alongside it, and launch it as a long-running process on your
platform of choice.

---

## Build the release binary

```bash
cargo build --workspace --release
```

This produces `target/release/aaf-server`. It is a static binary
with no runtime dependency on `cargo` or `rustc`. Size: roughly
18–25 MB depending on target triple and LTO settings. The
release profile is defined in the root `Cargo.toml`:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
opt-level = 3
```

---

## Configuration

`aaf-server` takes its configuration from a YAML file. The default
search path is `./aaf.yaml`; override with a positional arg:

```bash
./aaf-server run /etc/aaf/aaf.yaml
```

Minimum viable config:

```yaml
project:
  name: my-deployment
  version: 0.1.0

budget:
  max_tokens: 5000
  max_cost_usd: 1.0
  max_latency_ms: 30000

capabilities:
  - id: cap-example
    name: example
    description: example capability
    domains: [demo]
    required_scope: demo:read

demo:
  goal: "show last month sales"
  role: analyst
  domain: sales
  scopes: [sales:read]
```

See `examples/hello-agent/aaf.yaml` for a working example.

---

## Runtime components that must be wired

`aaf-server::cmd_run` wires every core component at startup.
Today all backends are in-memory; a production deployment
replaces them with real drivers (deferred). The wiring order:

1. `ServerConfig::from_path(aaf_yaml)` — parse + validate config.
2. `Registry::in_memory()` (→ PostgreSQL-backed in production).
3. `Recorder::in_memory()` (→ ClickHouse in production).
4. `PolicyEngine::with_default_rules()`.
5. `MemoryFacade::in_memory()` (→ pgvector + Redis + S3).
6. `IntentCompiler::default()`.
7. `RegistryPlanner::new(registry, bounds, composition)`.
8. `GraphExecutor::new(policy, recorder, budget)`.

All eight live in `core/crates/aaf-server/src/main.rs::cmd_run`.

---

## Gates every deployment must pass

Before you ship a build to production, run:

```bash
make ci
```

which is:

```
fmt-check  →  clippy  →  test  →  schema-validate  →  ontology-lint
```

The build is only shippable if all five exit zero. CI pipelines
should treat `make ci` as the canonical pre-deploy gate.

---

## Observability in production

- **Traces** — `aaf-trace::otel::OtlpExporter` ships OTLP-JSON
  spans to any collector. Wire the exporter at startup
  (`Recorder::with_otlp_exporter`).
- **Logs** — the `tracing` crate is a workspace dep. Configure a
  subscriber in `main.rs` for the target platform
  (e.g. `tracing_subscriber::fmt()` in dev,
  `tracing_subscriber::json()` in prod).
- **Metrics** — Prometheus exposition is deferred to post-v0.1.
  Today every hot-path counter lives in the trace recorder.

---

## Security posture

Every deployment must honour:

- **Rule 9** — no write capability without a compensation.
  Enforced at registration; a misconfigured YAML is rejected at
  startup.
- **Rule 6** — policy at every step. Enforced by the runtime;
  bypass is a compile-time impossibility.
- **Rule 13** — sidecar transparent fallback. Your deployment
  must tolerate "AAF unavailable" as a graceful-degradation
  mode, not a hard outage.
- **Rule 22** — agent identities are DIDs. For production, wire
  a persistent keystore through the `Keystore` trait (deferred
  to X1 Slice C; in-memory HMAC backend today).
- **Rule 21** — tenant-scoped entities. Wire a real
  `OntologyRegistry` and pass a classification lookup into the
  policy engine.

See [security.md](security.md) for the full security model.

---

## Scaling characteristics

Today every backend is in-memory, so `aaf-server` is a
**single-node, in-process** deployment. Realistic production
sizes require the deferred drivers:

- **Checkpoint store** — PostgreSQL via `sqlx`.
- **Working memory** — Redis.
- **Thread memory** — PostgreSQL.
- **Long-term memory** — pgvector / Qdrant.
- **Artifact store** — S3 / MinIO.
- **Trace store** — ClickHouse.
- **Registry** — PostgreSQL with a Redis read-through cache.

Each sits behind an `aaf-storage` trait so swapping the backend
is a one-crate change per layer.

---

## Deployment topologies

### Single node (today)

Every component in one process. Fine for dev, demos, and small
production deployments (< 100 req/s). Latency is minimised
because every hop is in-process.

### Sidecar per service (Pattern A)

One `aaf-sidecar` per target service, control plane as a
separate `aaf-server` deployment. This is the microservices
pattern — see [integration-microservices.md](integration-microservices.md).

### In-process wrapper (Pattern B)

`aaf-wrapper` linked into the monolith binary, no separate AAF
process. Simplest deployment, lowest latency, but shared failure
domain with the monolith. See
[integration-modular-monolith.md](integration-modular-monolith.md).

### Cell federation (Pattern C)

One full AAF deployment per cell, with
`aaf-federation::Router` enforcing bilateral agreements at cell
boundaries. See
[integration-cell-architecture.md](integration-cell-architecture.md).

---

## Rolling upgrades

Because contract types are `serde`-versioned with
`#[serde(default)]` on new fields, a newer sidecar and an older
control plane (or vice versa) can coexist during a rolling
upgrade window.

**Forbidden during upgrade:**

- Removing a field from a `CapabilityContract`.
- Changing a field type without a shim.
- Deleting a `TaskState` variant.
- Removing a rule without a policy-pack migration.

When in doubt, add first, deprecate later, remove after two
releases.

---

## Further reading

- [architecture.md](architecture.md) — the layered model
- [getting-started.md](getting-started.md) — build the binary
- [../development/build-and-ci.md](../development/build-and-ci.md)
  — the five gates, in full
- `core/crates/aaf-server/src/main.rs` — reference wiring
