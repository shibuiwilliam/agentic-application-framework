# Agentic Application Framework (AAF)

A **semantic orchestration layer** that adds AI agents on top of your existing service architecture -- microservices, modular monoliths, or cell architecture -- without modifying a single line of service code.

> Natural language at the edges, typed protocols internally, deterministic logic preserved.

```
User: "Cancel my last order and refund the payment"
  |
  v
Intent Compiler   ->  IntentEnvelope { goal, constraints, budget }
  |
  v
Planner           ->  3-step plan: lookup order -> cancel -> refund
  |
  v
Graph Runtime     ->  execute each step with policy checks + trace
  |
  v
Services          ->  Order Service, Payment Service (unchanged)
  |
  v
Result + full audit trail
```

---

## Why AAF?

Most AI agent frameworks ask you to rebuild your backend around their abstractions. AAF takes the opposite approach: **your services stay exactly as they are**.

AAF sits above your existing architecture as a control plane. It translates user intents into structured execution plans, discovers which services can fulfill each step, enforces security and budget policies, manages trust, handles failures intelligently, and traces everything -- while business logic stays in your services where it belongs.

- **Services own the logic.** Agents translate intents and route requests -- they never contain business logic.
- **Safety by default.** Every step passes through a policy engine with 7 rule families, 3 guard types, and 4 hook points. PII detection, injection detection, scope checks, and budget enforcement are always on.
- **Graceful degradation.** If the LLM goes down, AAF falls back through 5 levels -- from cached plans to rule-based flows to transparent bypass. Your services never break because AAF is having a bad day.
- **Start simple, grow incrementally.** Begin with fast-path routing (no LLM needed), add agent-assisted flows for ambiguous requests, and scale to multi-service orchestration only where it adds value.

---

## Quick Start

**Prerequisites:** Rust 1.70+ ([rustup](https://rustup.rs/))

```bash
# Build
cargo build --workspace

# Test (463 passing, 0 failures)
cargo test --workspace

# Run the demo
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml

# Try the CLI
cargo run -p aaf-server -- compile "show last month's sales"
cargo run -p aaf-server -- discover "monthly sales"
cargo run -p aaf-server -- validate examples/hello-agent/aaf.yaml
```

---

## How It Works

1. **Intent Compilation** -- Natural language (or app events) become typed `IntentEnvelope`s with goals, constraints, and budgets.
2. **Capability Discovery** -- The registry finds matching services using semantic search against a shared entity vocabulary.
3. **Planning** -- A DAG of execution steps is built, mixing deterministic nodes (no LLM) and agent nodes (LLM-powered). Composition safety checks prevent dangerous combinations.
4. **Policy Enforcement** -- Every step is checked at 4 hook points against 7 rule families: scope, side-effects, budget, PII, injection, composition safety, and data boundaries. Hook-aware rules fire only where relevant (e.g., injection scanning at PrePlan/PreStep, PII scanning at PostStep/PreArtifact).
5. **Graph Execution** -- The runtime walks the DAG through sidecars or wrappers, tracking budgets and recording traces. Shadow mode lets you observe decisions without executing writes.
6. **Intelligent Recovery** -- If a step fails, the Agentic Saga engine analyzes the cause and picks the best strategy: partial compensation, retry with alternative, pause-and-ask, or full rollback. The outcome tracks both compensated and preserved steps.
7. **Learning** -- Trace observations feed back through non-blocking subscribers: the fast-path miner promotes recurring patterns, the capability scorer adjusts reputation, and the router tuner optimizes model selection.

---

## Examples

AAF ships with eight progressive examples:

| Example | What It Shows | How to Run |
|---|---|---|
| [hello-agent](examples/hello-agent/) | Simplest pipeline: intent -> plan -> execute (read-only) | `cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml` |
| [order-saga](examples/order-saga/) | Multi-step saga with compensation, shadow mode, policy | `cargo test -p aaf-integration-tests --test order_saga_e2e` |
| [resilient-query](examples/resilient-query/) | Fast-path routing, guards, degradation, budget, approval | `cargo test -p aaf-integration-tests --test resilient_query_e2e` |
| [feedback-loop](examples/feedback-loop/) | Trust lifecycle, 5-level autonomy, online learning | `cargo test -p aaf-integration-tests --test feedback_loop_e2e` |
| [memory-context](examples/memory-context/) | 4-layer memory model, context budget (Rule 10), tenant isolation | `cargo test -p aaf-integration-tests --test memory_context_e2e` |
| [app-native-surface](examples/app-native-surface/) | Event routing, proposals-not-mutations, default-deny projections | `cargo test -p aaf-integration-tests --test app_native_surface_e2e` |
| [cross-cell-federation](examples/cross-cell-federation/) | Cross-cell routing, data boundaries, co-signed tokens | `cargo test -p aaf-integration-tests --test cross_cell_federation_e2e` |
| [signed-agent](examples/signed-agent/) | Cryptographic identity: DID, manifest signing, SBOM, revocation | `cargo run -p aaf-server -- identity verify examples/signed-agent/manifest.yaml` |

See [examples/README.md](examples/README.md) for full walkthroughs.

---

## CLI Reference

```bash
# Core pipeline
aaf-server                                    # run with ./aaf.yaml
aaf-server run examples/hello-agent/aaf.yaml  # run with explicit config
aaf-server validate aaf.yaml                  # validate config without running
aaf-server discover "monthly sales"           # semantic capability discovery
aaf-server compile "show last month sales"    # NL -> IntentEnvelope JSON

# Ontology
aaf-server ontology lint spec/examples/       # lint capabilities for entity declarations
aaf-server ontology import openapi.json       # import OpenAPI -> ontology YAML

# Identity
aaf-server identity generate-did my-agent     # mint a new DID
aaf-server identity sign-manifest m.yaml      # sign a manifest
aaf-server identity verify m.yaml             # verify a signed manifest
aaf-server identity export-sbom s.yaml        # SPDX JSON (default)
aaf-server identity export-sbom s.yaml --format cyclonedx  # CycloneDX JSON
aaf-server identity revoke <did> "reason"     # issue a signed revocation
```

---

## Architecture

```
+------------------------------------------------------------------+
|                       AAF Control Plane                           |
|                                                                   |
|  Intent Compiler  ->  Planner / Router  ->  Graph Runtime         |
|       |                     |                     |               |
|  Capability Registry    Policy Engine        Trust System         |
|  Memory (4 layers)      Trace Recorder       Budget Tracker      |
|  Ontology Registry      Eval Harness         Identity (DID)      |
|  Learning Pipeline      Surface Adapter      Federation          |
|                                                                   |
+------------------------------------------------------------------+
|                      Service Integration                          |
|                                                                   |
|  [Agent Sidecar]    [Agent Wrapper]     [Cell Federation]         |
|  (microservices)    (monoliths)         (cell architecture)       |
|                                                                   |
|  Services stay completely untouched                               |
+------------------------------------------------------------------+
```

### The 22 Crates

**Contracts and Domain**

| Crate | Purpose |
|---|---|
| `aaf-contracts` | Typed contracts: Intent, Capability, Task, Artifact, Handoff, Observation, Trust, Policy, Trace, Identity, Learn |
| `aaf-ontology` | Domain entities, classification lattice (Public < Internal < PII < Regulated), relations, lineage, versioning |

**Core Pipeline**

| Crate | Purpose |
|---|---|
| `aaf-intent` | Intent compiler: NL classifier, field extractor, context enricher, clarification refinement, semantic-hash cache |
| `aaf-registry` | Capability CRUD, semantic discovery, health tracking, degradation FSM, circuit breaker, 7-step registration pipeline |
| `aaf-planner` | Pattern classifier (4 patterns), fast-path routing with bounded cache, multi-step planner, composition safety |
| `aaf-runtime` | Graph executor: DAG with 5 node types, checkpoint, compensation chain, budget tracking, shadow mode, timeout enforcement |
| `aaf-saga` | Agentic Saga: intelligent recovery with preserved-step tracking (partial compensation, retry, pause-and-ask, full rollback, skip) |

**Safety and Trust**

| Crate | Purpose |
|---|---|
| `aaf-policy` | Policy engine: 7 hook-aware rule families, 3 guard types (input/output/action), approval workflow |
| `aaf-trust` | Trust scores, 5-level autonomy, delegation chain with `min(delegator, delegatee)`, promotion/demotion |
| `aaf-identity` | Cryptographic identity: DID (`did:aaf:<hex>`), signed manifests, SBOM (SPDX 2.3 + CycloneDX 1.5), capability tokens, revocation |

**Intelligence and Learning**

| Crate | Purpose |
|---|---|
| `aaf-llm` | LLM provider trait, mock provider, value-based model router, per-call budget enforcement |
| `aaf-memory` | 4-layer memory: working (transient), thread (session), long-term (entity-keyed semantic), artifact (provenance) + context budget (~7,500 tokens) |
| `aaf-learn` | Online learning via non-blocking trace subscribers: fast-path miner, capability scorer, router tuner, escalation tuner |
| `aaf-eval` | Evaluation harness: Judge trait, golden suites, replay divergence detection, regression reports |

**Observability**

| Crate | Purpose |
|---|---|
| `aaf-trace` | Trace recorder with timeout-bounded subscriber fan-out, OpenTelemetry JSON export, cost attribution, replay |

**Service Integration**

| Crate | Purpose |
|---|---|
| `aaf-sidecar` | Agent Sidecar for microservices: proxy, fast-path, guards, ACL, capability publisher, transparent fallback |
| `aaf-wrapper` | Agent Wrapper for modular monoliths: in-process wrapping with policy enforcement |
| `aaf-federation` | Cell federation: cross-cell routing, co-signed tokens, federation agreements, entity-space data boundaries |
| `aaf-surface` | App-native surface: AppEvent->Intent adapter, ActionProposal (Rule 20), StateProjection (Rule 19), EventRouter |

**Infrastructure**

| Crate | Purpose |
|---|---|
| `aaf-storage` | Storage traits + in-memory backends (no crate imports a DB driver -- Rule 11) |
| `aaf-transport` | Transport trait + loopback driver (gRPC/HTTP/NATS deferred) |
| `aaf-server` | CLI binary wiring all components together |

---

## Service Integration Patterns

AAF integrates with three architecture styles -- always without modifying existing services:

### Microservices: Agent Sidecar

A separate container alongside each service. It intercepts traffic, publishes capabilities to the registry, applies input/output guards, handles fast-path routing, and falls back to transparent proxying if AAF is unavailable. **Zero code changes to the service.**

### Modular Monolith: Agent Wrapper

An in-process wrapper around each module's public API. Near-zero latency overhead. Works with `@capability` / `@guard` / `@compensation` decorators.

### Cell Architecture: Cell Runtime + Federation

Each cell runs its own AAF runtime. Cross-cell communication goes through the Federation layer with co-signed capability tokens and entity-space data-boundary enforcement per federation agreement.

---

## Agentic Saga

AAF extends the traditional Saga pattern with **intelligent failure recovery**:

```
Traditional saga:  fail -> compensate everything in reverse
AAF agentic saga:  fail -> analyze cause -> choose optimal recovery
```

| Strategy | When Used | Example |
|---|---|---|
| **Partial compensation** | Some steps are still valid | Keep the stock reservation, refund only the payment |
| **Retry with alternative** | Transient or provider-specific failure | Switch to a different shipping carrier |
| **Pause and ask** | Missing or ambiguous information | Ask the user to confirm their address |
| **Full rollback** | Fundamental incompatibility | Product too large for any carrier |
| **Skip** | Non-critical step | Skip optional gift wrapping |

See the [order-saga](examples/order-saga/) example for a working demonstration.

---

## Communication Pattern Classification

Every request is classified into one of four patterns. AAF always uses the lightest processing possible:

| Pattern | When | Latency Target | LLM Needed? |
|---|---|---|---|
| **Fast Path** | Fully structured + unambiguous target | < 50ms p99 | No |
| **Agent Assisted** | Single service, minor ambiguity | < 500ms p99 | Small model |
| **Full Agentic** | Multi-service coordination | < 15s p99 | Yes |
| **Agentic Choreography** | Async, event-driven workflows | minutes-hours | Yes |

The goal is **> 60% fast path**. The online learning system (`aaf-learn`) automatically mines recurring patterns and promotes them to fast-path rules over time.

---

## Architecture Rules

AAF enforces 24 non-negotiable rules (13 foundation + 11 enhancement). Highlights:

| # | Rule | What It Means |
|---|---|---|
| 1 | Agents translate, services decide | Agents route and interpret -- they never contain business logic |
| 3 | Services stay untouched | Integration via Sidecar or Wrapper only |
| 5 | Deterministic core is sacred | Financial calculations, auth, audit, crypto -- always deterministic, never LLM |
| 6 | Policy at every step | 4 hook points: pre-plan, pre-step, post-step, pre-artifact |
| 7 | Guard every agent | Input (injection, auth), Output (PII, policy), Action (scope, side-effects) |
| 8 | Depth + budget limits | Every request carries depth (max 5), token/cost/time budgets |
| 9 | Compensation before implementation | Write capabilities must define rollback |
| 10 | Context minimization | ~7,500 tokens per LLM call across 5 budget sections |
| 13 | Sidecar transparent fallback | When AAF is down, sidecars forward directly to services |
| 14 | Semantics are nouns | Capabilities declare which entities they read/write/emit |
| 16 | Learning never touches the hot path | Subscribers observe traces out-of-band |
| 19 | Projections default-deny | State projections expose only explicitly listed fields |
| 20 | Proposals, not mutations | App surface produces proposals that apps accept/reject/transform |
| 22 | Identity is cryptographic | Every agent has a DID; revoked agents are rejected before the trace opens |

See [CLAUDE.md](CLAUDE.md) for the full set of 24 rules.

---

## Development

```bash
make all          # cargo check + cargo test (fast local loop)
make ci           # fmt-check + clippy + test + schema-validate + ontology-lint
make lint         # fmt-check + clippy
make test         # run all tests
make doc-open     # build and open rustdoc in browser
make watch        # re-run cargo check on file changes (needs cargo-watch)
make loc          # line count per crate

make schema-validate  # validate YAML examples against JSON Schemas
make ontology-lint    # lint capabilities for entity declarations

make tree         # show dependency tree
make audit        # audit for known vulnerabilities
make help         # see all targets
```

---

## Project Structure

```
Cargo.toml                     Workspace root (22 crates + integration tests)
CLAUDE.md                      24 architecture rules + coding conventions
PROJECT.md                     Full design document (architecture + enhancements)
IMPLEMENTATION_PLAN.md         Iteration-by-iteration build log

core/crates/aaf-*/             22 Rust crates
core/tests/integration/        Cross-crate integration tests

spec/schemas/                  18 JSON Schemas
spec/examples/                 11 example YAML configs
examples/                      8 runnable examples with READMEs
policies/base/                 7 base policy YAML files

development/                   16 technical docs for contributors
docs/                          Architecture, contracts, policies, security,
                               deployment, ADRs, enhancement designs
```

---

## Current Status

| Metric | Value |
|---|---|
| Workspace crates | 22 |
| Lines of Rust | ~23,500 |
| Tests passing | 463 |
| Test failures | 0 |
| Clippy warnings | 0 |
| JSON Schemas | 18 |
| Spec examples | 11 |
| Runnable examples | 8 |
| Base policies | 7 |
| CLI subcommands | 13 |
| Architecture rules | 24 |
| ADRs | 2 |

### What's implemented

- Complete control plane: intent compilation, capability discovery, planning, graph execution, policy enforcement, trust management, trace recording
- Domain ontology with entity classification lattice and lineage
- Cryptographic agent identity: DID, signed manifests, SBOM (SPDX + CycloneDX), capability tokens, revocation
- Agentic Saga with intelligent recovery and preserved-step tracking
- Online learning: fast-path miner, capability scorer, router tuner, escalation tuner
- App-native surface: AppEvent->Intent adapter, ActionProposal, default-deny StateProjection, EventRouter
- 4-layer memory model with context budget enforcement (~7,500 tokens)
- Evaluation harness: golden suites, replay divergence detection, regression reports
- 5-level circuit breaker, 5-level degradation chain, shadow mode
- 7-step service registration pipeline
- Hook-aware policy rules, timeout-bounded event waits, bounded plan cache

### What's deferred (trait-backed extension points)

- Real transport drivers (gRPC via `tonic`, HTTP via `axum`, NATS, WebSocket)
- Real storage backends (PostgreSQL, Redis, S3, ClickHouse, pgvector)
- Real LLM providers (Anthropic Claude, OpenAI, Bedrock, Vertex AI)
- Python / TypeScript / Go SDKs
- Front Door / Dashboard / Trace Explorer UIs
- Docker images, Helm charts, Terraform modules

All deferred items plug in through trait interfaces -- adding a real gRPC driver or PostgreSQL backend changes zero domain logic.

---

## Documentation

| Document | What It Covers |
|---|---|
| [PROJECT.md](PROJECT.md) | Full design: architecture, contracts, security, economics, enhancements, roadmap |
| [CLAUDE.md](CLAUDE.md) | 24 architecture rules, coding conventions, slicing strategy |
| [IMPLEMENTATION_PLAN.md](IMPLEMENTATION_PLAN.md) | Iteration-by-iteration build log |
| [development/](development/) | 16 technical docs: crate reference, runtime internals, extension points, gotchas, glossary |
| [docs/](docs/) | Architecture, contracts, policies, security, deployment, getting started, ADRs, enhancements |
| [examples/](examples/) | 8 progressive walkthroughs from hello-world to federation + identity |

---

## License

Apache-2.0
