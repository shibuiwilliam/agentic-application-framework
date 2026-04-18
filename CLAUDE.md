# CLAUDE.md — Agentic Application Framework (AAF)

## What This Project Is

AAF is a software execution platform that adds a **semantic orchestration layer** on top of existing service architectures (microservices, modular monoliths, cell architecture). AI agents serve as the universal interface between humans, applications, services, and APIs.

AAF does NOT replace existing services. It sits above them, translating intents into structured execution plans, discovering capabilities, enforcing policies, managing trust, and providing full observability — while the actual business logic stays in the services where it belongs.

**Core equation:** Natural language at the edges, typed protocols internally, deterministic logic preserved.

---

## Repository Structure

```
aaf/
├── CLAUDE.md
├── README.md
├── LICENSE                            # Apache 2.0
├── Makefile
├── docker-compose.yml
├── docker-compose.prod.yml
│
├── spec/                              # Single source of truth for all contracts
│   ├── proto/                         # Protobuf definitions (gRPC internal comms)
│   │   ├── aaf/v1/
│   │   │   ├── intent.proto           # Intent Envelope
│   │   │   ├── capability.proto       # Capability Contract
│   │   │   ├── task.proto             # Task state machine
│   │   │   ├── artifact.proto         # Artifact Contract
│   │   │   ├── handoff.proto          # Delegation Contract
│   │   │   ├── observation.proto      # Observation record
│   │   │   ├── trust.proto            # Trust Score & Autonomy
│   │   │   ├── policy.proto           # Policy rules
│   │   │   ├── trace.proto            # Execution trace
│   │   │   ├── runtime.proto          # Graph Runtime service API
│   │   │   ├── registry.proto         # Capability Registry service API
│   │   │   └── sidecar.proto          # Agent Sidecar service API
│   │   └── buf.yaml
│   ├── schemas/                       # JSON Schema (external/config contracts)
│   │   ├── intent-envelope.schema.json
│   │   ├── capability-contract.schema.json
│   │   ├── sidecar-config.schema.json
│   │   ├── wrapper-config.schema.json
│   │   ├── cell-config.schema.json
│   │   ├── policy-pack.schema.json
│   │   ├── saga-definition.schema.json
│   │   ├── degradation-spec.schema.json
│   │   └── fast-path-rules.schema.json
│   └── examples/                      # Example contract instances
│       ├── capability-inventory.yaml
│       ├── capability-payment.yaml
│       ├── sidecar-config-order.yaml
│       ├── saga-order-processing.yaml
│       └── policy-pack-base.yaml
│
├── core/                              # Core runtime — Rust workspace
│   ├── Cargo.toml                     # Workspace root
│   ├── crates/
│   │   ├── aaf-runtime/              # Graph Runtime engine
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── graph.rs           # DAG definition & validation
│   │   │       ├── executor.rs        # Graph execution engine
│   │   │       ├── node/
│   │   │       │   ├── mod.rs
│   │   │       │   ├── deterministic.rs  # Deterministic node (no LLM)
│   │   │       │   ├── agent.rs          # Agent node (LLM-powered)
│   │   │       │   ├── approval.rs       # Human approval gate
│   │   │       │   ├── fork.rs           # Parallel fork/join
│   │   │       │   └── event_wait.rs     # External event wait
│   │   │       ├── checkpoint.rs      # State persistence & resume
│   │   │       ├── compensation.rs    # Saga compensation chains
│   │   │       ├── scheduler.rs       # Sequential / parallel scheduling
│   │   │       ├── budget.rs          # Token / cost / time budget tracking
│   │   │       └── timeout.rs
│   │   │
│   │   ├── aaf-intent/               # Intent Compiler
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── types.rs           # 5 intent types + extensibility
│   │   │       ├── compiler.rs        # NL → Intent Envelope pipeline
│   │   │       ├── classifier.rs      # Intent type classification
│   │   │       ├── extractor.rs       # Field extraction from NL
│   │   │       ├── enricher.rs        # Context enrichment from memory
│   │   │       ├── refinement.rs      # Missing field → clarification question
│   │   │       ├── cache.rs           # Intent cache (semantic hash)
│   │   │       └── versioning.rs      # Intent type evolution
│   │   │
│   │   ├── aaf-registry/             # Capability Registry
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── store.rs           # Capability CRUD + indexing
│   │   │       ├── discovery.rs       # Semantic search over capabilities
│   │   │       ├── health.rs          # Service health tracking
│   │   │       ├── degradation.rs     # Degradation level state machine
│   │   │       ├── a2a.rs             # A2A Agent Card import/export
│   │   │       └── version.rs         # Capability versioning & migration
│   │   │
│   │   ├── aaf-planner/              # Planner / Router
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── planner.rs         # Intent → Execution Plan
│   │   │       ├── router.rs          # Communication pattern classification
│   │   │       ├── fast_path.rs       # Fast path detection & routing
│   │   │       ├── bounds.rs          # Bounded autonomy constraints
│   │   │       ├── composition.rs     # Capability composition + safety check
│   │   │       └── plan_cache.rs      # Execution plan caching
│   │   │
│   │   ├── aaf-policy/               # Policy / Risk Engine
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── engine.rs          # Policy evaluation pipeline
│   │   │       ├── guard/
│   │   │       │   ├── mod.rs
│   │   │       │   ├── input.rs       # Injection detection, auth check
│   │   │       │   ├── output.rs      # PII leak, policy compliance
│   │   │       │   └── action.rs      # Scope verification, side effect check
│   │   │       ├── rules/
│   │   │       │   ├── mod.rs
│   │   │       │   ├── scope.rs       # Scope-based authorization
│   │   │       │   ├── side_effect.rs # Read/write/delete/send/payment gates
│   │   │       │   ├── budget.rs      # Budget enforcement
│   │   │       │   ├── pii.rs         # PII detection & masking
│   │   │       │   ├── injection.rs   # Prompt injection detection
│   │   │       │   ├── composition.rs # Composition safety (emergent risk)
│   │   │       │   └── boundary.rs    # Tenant & data boundary enforcement
│   │   │       ├── approval.rs        # Human approval workflow
│   │   │       └── plugin.rs          # Policy plugin loading (WASM / native)
│   │   │
│   │   ├── aaf-trust/                # Trust / Identity
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── score.rs           # Trust score calculation & history
│   │   │       ├── autonomy.rs        # 5-level autonomy management
│   │   │       ├── delegation.rs      # Chain trust propagation: min(a, b)
│   │   │       ├── promotion.rs       # Promotion / demotion logic
│   │   │       └── signing.rs         # Artifact & intent signing
│   │   │
│   │   ├── aaf-memory/               # State / Memory
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── working.rs         # Per-task transient state
│   │   │       ├── thread.rs          # Per-session/case state
│   │   │       ├── longterm.rs        # Persistent knowledge (vector search)
│   │   │       ├── artifact.rs        # Artifact store with provenance
│   │   │       └── context.rs         # Context budget manager (~7500 tok)
│   │   │
│   │   ├── aaf-trace/                # Trace / Replay / Eval
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── recorder.rs        # Observation & step recording
│   │   │       ├── replay.rs          # Checkpoint-based replay engine
│   │   │       ├── metrics.rs         # Operational metric calculation
│   │   │       ├── otel.rs            # OpenTelemetry Span integration
│   │   │       └── export.rs          # Trace export formats
│   │   │
│   │   ├── aaf-sidecar/              # Agent Sidecar (for microservices)
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── proxy.rs           # Transparent request proxying
│   │   │       ├── capability.rs      # Auto-publish capabilities from config
│   │   │       ├── fast_path.rs       # Local fast-path rule evaluation
│   │   │       ├── guard.rs           # Local input/output guard
│   │   │       ├── mapping.rs         # Intent field ↔ API field mapping
│   │   │       └── health.rs          # Upstream service health monitoring
│   │   │
│   │   ├── aaf-wrapper/              # Agent Wrapper (for modular monoliths)
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── wrapper.rs         # In-process module wrapping
│   │   │       ├── capability.rs      # Module method → capability mapping
│   │   │       └── guard.rs           # In-process guard
│   │   │
│   │   ├── aaf-federation/           # Cell/Cross-org federation
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── cell.rs            # Cell runtime configuration
│   │   │       ├── router.rs          # Cross-cell routing
│   │   │       ├── agreement.rs       # Federation agreement enforcement
│   │   │       └── boundary.rs        # Data boundary enforcement
│   │   │
│   │   ├── aaf-saga/                 # Agentic Saga engine
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── definition.rs      # Saga definition parsing
│   │   │       ├── executor.rs        # Saga step execution
│   │   │       ├── compensation.rs    # Intelligent compensation logic
│   │   │       ├── recovery.rs        # Failure analysis & recovery strategy
│   │   │       └── state.rs           # Saga state machine
│   │   │
│   │   ├── aaf-transport/            # Transport abstraction
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── grpc.rs
│   │   │       ├── http.rs
│   │   │       ├── nats.rs
│   │   │       ├── ws.rs
│   │   │       └── cloudevents.rs     # CloudEvents encoding/decoding
│   │   │
│   │   ├── aaf-llm/                  # LLM provider abstraction
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── provider.rs        # LLMProvider trait
│   │   │       ├── anthropic.rs       # Claude integration
│   │   │       ├── openai.rs          # OpenAI integration
│   │   │       ├── local.rs           # Ollama / vLLM integration
│   │   │       ├── router.rs          # Value-based model routing
│   │   │       └── budget.rs          # Per-call token budget enforcement
│   │   │
│   │   ├── aaf-storage/              # Storage backend abstraction
│   │   │   └── src/
│   │   │       ├── lib.rs
│   │   │       ├── traits.rs          # Storage trait definitions
│   │   │       ├── postgres.rs
│   │   │       ├── sqlite.rs
│   │   │       ├── redis.rs
│   │   │       ├── s3.rs
│   │   │       ├── clickhouse.rs
│   │   │       └── pgvector.rs
│   │   │
│   │   └── aaf-server/               # Main server binary
│   │       └── src/
│   │           ├── main.rs
│   │           ├── config.rs          # Unified configuration
│   │           ├── api/
│   │           │   ├── mod.rs
│   │           │   ├── rest.rs        # HTTP/REST API (axum)
│   │           │   ├── grpc.rs        # gRPC services (tonic)
│   │           │   └── ws.rs          # WebSocket streaming
│   │           └── wire.rs            # Component wiring & DI
│   │
│   └── tests/
│       ├── integration/
│       │   ├── full_pipeline_test.rs
│       │   ├── saga_test.rs
│       │   ├── fast_path_test.rs
│       │   ├── degradation_test.rs
│       │   └── multi_service_test.rs
│       └── chaos/
│           ├── service_failure.rs
│           ├── llm_failure.rs
│           ├── injection_attack.rs
│           └── budget_exhaustion.rs
│
├── sdk/
│   ├── python/
│   │   ├── pyproject.toml
│   │   ├── src/aaf/
│   │   │   ├── __init__.py
│   │   │   ├── agent.py
│   │   │   ├── sidecar.py
│   │   │   ├── wrapper.py
│   │   │   ├── decorators.py          # @capability, @guard, @compensation
│   │   │   ├── contracts.py           # Pydantic models from proto
│   │   │   ├── intent.py
│   │   │   ├── saga.py
│   │   │   ├── client.py
│   │   │   ├── memory.py
│   │   │   ├── testing.py
│   │   │   └── cli/
│   │   │       ├── __init__.py
│   │   │       └── main.py
│   │   └── tests/
│   ├── typescript/
│   │   ├── package.json
│   │   ├── tsconfig.json
│   │   ├── src/
│   │   │   ├── index.ts
│   │   │   ├── agent.ts
│   │   │   ├── sidecar.ts
│   │   │   ├── wrapper.ts
│   │   │   ├── decorators.ts
│   │   │   ├── contracts.ts
│   │   │   ├── intent.ts
│   │   │   ├── saga.ts
│   │   │   ├── client.ts
│   │   │   ├── memory.ts
│   │   │   └── testing.ts
│   │   └── tests/
│   └── go/
│       ├── go.mod
│       ├── agent.go
│       ├── sidecar.go
│       ├── wrapper.go
│       ├── contracts.go
│       ├── client.go
│       └── testing.go
│
├── adapters/
│   ├── mcp/
│   ├── a2a/
│   ├── rest/
│   ├── grpc/
│   └── event/
│
├── policies/
│   ├── base/
│   │   ├── scope-check.yaml
│   │   ├── side-effect-gate.yaml
│   │   ├── pii-guard.yaml
│   │   ├── injection-guard.yaml
│   │   ├── budget-control.yaml
│   │   ├── composition-safety.yaml
│   │   └── boundary-enforcement.yaml
│   ├── finance/
│   ├── healthcare/
│   └── README.md
│
├── ui/
│   ├── front-door/
│   │   ├── package.json
│   │   └── src/
│   │       ├── App.tsx
│   │       └── components/
│   │           ├── ChatInterface.tsx
│   │           ├── ApprovalGate.tsx
│   │           ├── TraceViewer.tsx
│   │           ├── ArtifactRenderer.tsx
│   │           └── ProgressIndicator.tsx
│   ├── dashboard/
│   └── trace-explorer/
│
├── deploy/
│   ├── helm/
│   │   ├── aaf-control-plane/
│   │   ├── aaf-data-plane/
│   │   ├── aaf-sidecar-injector/
│   │   └── aaf-front-door/
│   ├── docker/
│   │   ├── Dockerfile.server
│   │   ├── Dockerfile.sidecar
│   │   ├── Dockerfile.front-door
│   │   └── Dockerfile.dashboard
│   ├── terraform/
│   │   ├── aws/
│   │   ├── gcp/
│   │   └── azure/
│   └── compose/
│       ├── minimal.yml
│       ├── standard.yml
│       └── full.yml
│
├── examples/
│   ├── microservices/
│   │   ├── docker-compose.yml
│   │   ├── services/
│   │   │   ├── order-service/
│   │   │   ├── inventory-service/
│   │   │   ├── payment-service/
│   │   │   └── shipping-service/
│   │   ├── sidecar-configs/
│   │   └── saga-definitions/
│   ├── modular-monolith/
│   │   ├── src/
│   │   │   ├── modules/
│   │   │   └── aaf_wrappers/
│   │   └── aaf-config.yaml
│   ├── cell-architecture/
│   │   ├── cell-japan/
│   │   ├── cell-us/
│   │   └── federation-config.yaml
│   └── hello-agent/
│
├── docs/
│   ├── architecture.md
│   ├── getting-started.md
│   ├── integration-microservices.md
│   ├── integration-modular-monolith.md
│   ├── integration-cell-architecture.md
│   ├── contracts.md
│   ├── saga.md
│   ├── fast-path.md
│   ├── policies.md
│   ├── security.md
│   ├── deployment.md
│   ├── migration-patterns.md
│   └── adr/
│
└── tests/
    ├── e2e/
    │   ├── microservices/
    │   ├── monolith/
    │   └── federation/
    ├── contract/
    ├── chaos/
    ├── semantic-regression/
    └── benchmarks/
```

---

## Technology Stack

### Core Runtime: Rust

The Graph Runtime, Policy Engine, Sidecar proxy, and Trace recorder are the hot path. They must be fast, memory-safe, and deployable as both a standalone server and an embeddable library.

| Crate | Purpose |
|---|---|
| `tokio` | Async runtime |
| `tonic` | gRPC server/client |
| `axum` | HTTP/REST API |
| `serde` / `prost` | Serialization (JSON, Protobuf) |
| `sqlx` | PostgreSQL / SQLite |
| `redis` | Redis client |
| `aws-sdk-s3` | S3-compatible object storage |
| `clickhouse` | ClickHouse client (traces) |
| `pgvector` | Vector similarity search |
| `nats` | NATS messaging |
| `opentelemetry` | Distributed tracing |
| `thiserror` | Error types |
| `cbindgen` | C FFI header generation |
| `wasm-bindgen` | WASM compilation target |

### Contract Definitions: Protobuf + JSON Schema

- **Protobuf 3** (`spec/proto/`) — internal gRPC communication. Code generation into Rust, Python, TypeScript, Go via `buf`.
- **JSON Schema 2020-12** (`spec/schemas/`) — configuration files, Capability definitions, Policy packs, Saga definitions.
- All SDK types are generated from spec, never hand-written.

### SDKs

| Language | Runtime | Key Libraries | Primary Use |
|---|---|---|---|
| Python ≥3.11 | `asyncio` | pydantic v2, grpcio, httpx, click, pytest | Agent dev, data/ML |
| TypeScript | Node/Bun | zod, @grpc/grpc-js, vitest | Web agents, UI |
| Go ≥1.22 | goroutines | google.golang.org/grpc, cobra | Infra agents |

### Storage (all pluggable via traits)

| Layer | Dev | Production |
|---|---|---|
| Checkpoint | SQLite | PostgreSQL |
| Working memory | In-memory | Redis |
| Thread memory | SQLite | PostgreSQL |
| Long-term memory | SQLite brute-force | pgvector / Qdrant |
| Artifacts | Local filesystem | S3 / MinIO |
| Traces | SQLite | ClickHouse |
| Registry | In-memory | PostgreSQL + Redis cache |

### Transport

| Path | Protocol |
|---|---|
| Core ↔ Agents | gRPC |
| Agent ↔ Agent (cluster) | gRPC or NATS |
| Agent ↔ Agent (cross-org) | A2A over HTTPS |
| Agent ↔ Tools | MCP (JSON-RPC 2.0) |
| Async events | CloudEvents over NATS/Kafka |
| Front Door ↔ Runtime | WebSocket + REST |
| Sidecar ↔ Service | Same protocol as service (transparent) |

### LLM Integration

Model-agnostic `LLMProvider` trait. Built-in: Anthropic Claude (primary), OpenAI, Ollama/vLLM, Bedrock, Vertex AI. Value-based routing selects model per request.

---

## Build & Dev Commands

```bash
make setup                      # Install all toolchains
make deps                       # Install all dependencies
make build                      # Build everything
make build-core                 # Rust workspace only
make build-sidecar              # Sidecar binary only
make proto                      # buf generate → all languages
make schema-validate            # Validate JSON Schemas
make test                       # All tests
make test-core                  # Rust unit + integration
make test-e2e-microservices     # E2E with microservice example
make test-e2e-monolith          # E2E with monolith example
make test-e2e-federation        # E2E with multi-cell example
make test-contract              # Proto/schema conformance
make test-chaos                 # Chaos engineering
make test-semantic-regression   # Semantic output comparison
make bench                      # Performance benchmarks
make dev                        # docker-compose minimal
make dev-full                   # docker-compose full
make lint                       # All linters
make fmt                        # All formatters
make docker                     # Build Docker images
make helm-package               # Package Helm charts
```

---

## Architecture Rules

Non-negotiable. Every PR must conform.

### Rule 1: Agents Translate, Services Decide
Agents handle intent interpretation, capability routing, negotiation, and exception classification. They NEVER contain business logic. Business logic stays in services.

### Rule 2: Typed Internals
All inter-component communication uses Protobuf or JSON Schema-validated structures. Natural language exists ONLY at the Front Door input and inside LLM prompts within agent nodes.

### Rule 3: Services Stay Untouched
Existing services need zero code changes. Sidecar (microservices) or Wrapper (monoliths) handles all AAF integration. If a design requires modifying the target service, the design is wrong.

### Rule 4: Fast Path First
Before adding agent intelligence to a communication path, check whether Fast Path works. Classify every request: Fast Path → Agent Assisted → Full Agentic → Choreography. Target: >60% Fast Path.

### Rule 5: Deterministic Core is Sacred
These are ALWAYS deterministic nodes, never agent nodes: financial calculations, inventory reservation, authentication decisions, audit logs, cryptographic operations, state machine transitions, rate limiting.

### Rule 6: Policy at Every Step
Every execution path passes through Policy Engine. No shortcuts. Checks at: (a) before planning, (b) before each step, (c) after each step output, (d) before artifact creation.

### Rule 7: Guard Every Agent
Three mandatory guards per agent: Input (injection, auth), Output (PII, policy), Action (scope, side-effect).

### Rule 8: Depth and Budget Limits
Every request carries `depth` (max 5), `token_budget`, `cost_budget_usd`, `time_budget_ms`. Exceeding triggers graceful termination with partial results.

### Rule 9: Compensation Before Implementation
Define rollback before implementing any write Capability. No compensation = human approval at ALL trust levels.

### Rule 10: Context Minimization
~7,500 tokens per LLM call: system (~2,000) + intent (~500) + memory (~2,000) + step context (~1,000) + tool results (~2,000).

### Rule 11: Storage Behind Traits
No crate directly imports a database driver. All through trait interfaces.

### Rule 12: Trace Everything
Every decision records an Observation. Integrates with OpenTelemetry. Cannot be disabled in production.

### Rule 13: Sidecar Transparent Fallback
When AAF is unavailable, Sidecar forwards requests directly to the service. System degrades to "no AAF" not "broken."

---

## Communication Pattern Classification

Every request is classified into exactly one of four patterns:

```
Request → Fully structured + unambiguous target?
  YES → ① FAST PATH (no LLM, <50ms p99)
  NO  → Single service with minor ambiguity?
    YES → ② AGENT ASSISTED (small model, <500ms p99)
    NO  → Multi-service coordination needed?
      YES → ③ FULL AGENTIC (plan + graph, <15s p99)
      NO  → ④ AGENTIC CHOREOGRAPHY (async, minutes-hours)
```

### Fast Path Implementation
```rust
// aaf-planner/src/fast_path.rs
pub struct FastPathRule {
    pub pattern: RequestPattern,
    pub target_capability: String,
    pub field_mapping: Vec<FieldMapping>,
    pub conditions: Vec<Condition>,
}

pub enum FastPathResult {
    Match { capability_id: String, mapped_request: ServiceRequest },
    NoMatch,
}
```

Fast Path rules are evaluated locally in the Sidecar — no round-trip to control plane.

---

## Agentic Saga

Extends traditional Saga with intelligent failure recovery.

**Traditional:** fail → compensate everything in reverse
**Agentic:** fail → analyze cause → choose optimal recovery (partial compensation, retry, pause-and-ask, or full rollback)

```yaml
saga:
  name: order-processing
  steps:
    - step: 1
      name: "在庫予約"
      type: deterministic
      capability: cap-stock-reserve
      compensation: cap-stock-release
      compensation_type: mandatory

    - step: 2
      name: "決済実行"
      type: deterministic
      capability: cap-payment-execute
      compensation: cap-payment-refund
      compensation_type: mandatory

    - step: 3
      name: "配送手配"
      type: agent
      capability: cap-shipping-arrange
      on_failure:
        strategy: intelligent_recovery
        rules:
          - condition: "住所不備"
            action: pause_and_ask_user
            preserve: [step-1, step-2]
          - condition: "配送業者一時障害"
            action: retry_with_alternative
            preserve: [step-1, step-2]
          - condition: "商品サイズ超過"
            action: full_compensation
```

Saga state machine:
```
initiated → running → step_N_failed → analyzing → recovery_selected
  → partial_compensation → waiting_for_input → resumed
  → full_compensation → saga_failed
  → retry → (back to running)
  → saga_completed
```

---

## Degradation Chain

5 levels, from full intelligence to transparent bypass:

```
Level 0: FULL AGENTIC — LLM orchestration, dynamic planning
  ↓ LLM latency spike
Level 1: CACHED — Cached intent/plan mappings, small model adjustments
  ↓ LLM unavailable
Level 2: RULE-BASED — Pre-defined flows, rule-based branching
  ↓ Runtime overloaded
Level 3: FAST PATH ONLY — Structured requests only, direct routing
  ↓ AAF layer failure
Level 4: BYPASS — Sidecar transparent proxy, no AAF processing
```

Each Capability also declares its own degradation:
```yaml
degradation:
  - level: full
    description: "リアルタイム全倉庫"
  - level: partial
    trigger: "primary_db_slow"
    description: "主要倉庫のみ、15分遅延"
  - level: cached
    trigger: "db_unreachable"
    description: "最大1時間前のキャッシュ"
  - level: unavailable
    fallback: "手動確認を依頼"
```

---

## Service Integration Patterns

### A: Microservices — Agent Sidecar
Separate container alongside service. Intercepts traffic, publishes capabilities, applies guards, handles fast-path. Zero code changes to service.

### B: Modular Monolith — Agent Wrapper
In-process wrapper around module public API. Near-zero latency overhead. Uses `@capability` / `@guard` / `@compensation` decorators.

### C: Cell Architecture — Cell Runtime + Federation
Each cell has local AAF runtime. Cross-cell communication via Federation layer with A2A protocol. Data boundary enforcement per federation agreement.

---

## Contract Types

All originate in `spec/`. Never hand-write in SDKs.

### Intent Envelope
```
intent_id, type (5 kinds), requester {user_id, role, scopes[]},
goal, domain, constraints, budget {tokens, cost, latency},
deadline, risk_tier, approval_policy, output_contract,
trace_id, depth (max 5)
```

### Capability Contract
```
id, name, description, version, provider_agent,
endpoint {type, address, method},
input_schema, output_schema,
side_effect (none|read|write|delete|send|payment),
idempotent, reversible, deterministic,
compensation {endpoint}?,
sla {latency, availability}, cost {per_request},
required_scope, data_classification,
degradation[], depends_on[], conflicts_with[],
fast_path_rules[]
```

### Task States
```
proposed → waiting_for_context → ready → running
  → paused_for_approval → running
  → failed → analyzing → recovering → (varies)
  → completed | cancelled
```

---

## Implementation Order

Follow strictly. Each step must have passing tests.

1. **Spec & Codegen** — Proto + JSON Schema + buf generate
2. **Graph Runtime** — DAG execution, nodes, checkpoint, compensation, budget
3. **Saga Engine** — Definition, execution, intelligent recovery
4. **Policy Engine** — Rules, guards, approval workflow, plugins
5. **Intent Compiler** — Types, classification, extraction, refinement, cache
6. **Capability Registry** — CRUD, discovery, health, degradation, versioning
7. **Planner & Router** — Pattern classification, fast path, planning, composition safety
8. **Memory System** — 4 layers, context budget
9. **Trust System** — Scores, autonomy, delegation chain, signing
10. **Trace System** — Recording, OpenTelemetry, replay, metrics
11. **Sidecar** — Proxy, capability publish, fast path, guards, transparent fallback
12. **Wrapper** — In-process wrapping, capability mapping
13. **Federation** — Cell config, cross-cell routing, agreement enforcement
14. **Transport & Server** — gRPC + REST + WS, component wiring
15. **Python SDK** — Agent, decorators, sidecar/wrapper builders, saga builder, CLI
16. **TypeScript SDK** — Mirror Python
17. **Go SDK** — Core capabilities
18. **Front Door UI** — Chat, approval gate, progress, artifacts
19. **Dashboard & Trace Explorer** — Metrics, health map, trace search/replay
20. **Docker & Helm** — Images, compose profiles, charts, sidecar injector
21. **Examples** — Microservices, monolith, cell architecture

---

## Coding Conventions

### Rust (core/)
- Edition 2024. `clippy::pedantic`. Zero warnings.
- `thiserror` for libs, `anyhow` in bins only.
- All public types: `Debug, Clone, Serialize, Deserialize`.
- Async via `tokio`. No `unwrap()` in lib code.
- `///` doc comments on all public items.
- `prelude` module per crate.

### Python (sdk/python/)
- ≥3.11. `mypy --strict`. pydantic v2.
- `ruff format` + `ruff check`. `pytest` + `pytest-asyncio`. `uv`.

### TypeScript (sdk/typescript/, ui/)
- Strict mode. No `any`. `zod` validation.
- `prettier` + `eslint`. `vitest`. `pnpm`.

### Go (sdk/go/)
- ≥1.22. `golangci-lint`. `testify`.
- `context.Context` first param. `fmt.Errorf("...: %w", err)`.

### General
- Conventional Commits. Branches: `feat/`, `fix/`, `docs/`.
- Contract types generated from `spec/`, never hand-written.
- Never edit generated code.

---

## Error Handling

```rust
#[derive(Debug, thiserror::Error)]
pub enum RuntimeError {
    #[error("step {step_id} timed out after {timeout_ms}ms")]
    StepTimeout { step_id: String, timeout_ms: u64 },
    #[error("budget exceeded: {resource} used {used}, limit {limit}")]
    BudgetExceeded { resource: String, used: f64, limit: f64 },
    #[error("policy violation: {rule} — {details}")]
    PolicyViolation { rule: String, details: String },
    #[error("capability not found for: {query}")]
    CapabilityNotFound { query: String },
    #[error("saga compensation failed at step {step_id}: {reason}")]
    CompensationFailed { step_id: u32, reason: String },
    #[error(transparent)]
    Storage(#[from] StorageError),
    #[error(transparent)]
    Transport(#[from] TransportError),
}
```

All errors typed, carry context, matchable. Compensation failures always surfaced.

---

## Security Checklist (every PR)

- [ ] No free-form strings between components
- [ ] Input Guard before every agent node
- [ ] Output Guard after every agent node
- [ ] Policy Engine at every step
- [ ] Depth decremented on delegation
- [ ] Budget decremented at every LLM call
- [ ] Trust: `min(delegator, delegatee)`
- [ ] PII detection on all outputs
- [ ] Injection detection on all external inputs
- [ ] Artifact signing with full provenance
- [ ] Tenant isolation on all storage ops
- [ ] Compensation defined for all write capabilities
- [ ] No `unwrap()` in Rust libs, no `any` in TS
- [ ] Secrets never in logs/traces
- [ ] Sidecar transparent fallback on AAF failure
- [ ] Cross-cell data boundary enforced

---

## Enhancement Architecture Rules (extending Rules 1–13)

### Rule 14: Semantics Are Nouns, Not Names
A capability's semantics are defined by the **entities** it reads, writes, and emits — not by field names or schema shapes. Every capability must declare `reads:` / `writes:` / `emits:` against the ontology. Missing declarations are a warning below 90% adoption and an error above 90%.

### Rule 15: Feedback Is a Contract
Outcomes are a typed field on `Observation`. They are written by the runtime at step-end, by the saga engine on saga completion, by the Front Door/app surface on user feedback, and by `aaf-eval` on offline scoring. No learning code reads from anywhere else.

### Rule 16: Learning Never Touches the Hot Path
`aaf-learn` subscribes to trace events out of band. It never blocks `aaf-runtime::executor`. Adaptation writes go through `aaf-registry`, `aaf-llm::router`, and `aaf-planner::fast_path` via their public APIs.

### Rule 17: Every Adaptation Is Reversible and Observable
Every learned change carries `(learned_by, learned_at, evidence)`, emits its own Observation, and can be rolled back. No silent mutations to registry/router state.

### Rule 18: Policy Governs Learning
`aaf-learn` cannot mutate policy rules. It may *propose* tightening within the bounds of the active policy pack; adoption requires the same approval workflow as any policy change.

### Rule 19: Projections Default-Deny
A `StateProjection` exposes **only** the fields it explicitly lists, and only if the requesting agent's trust level and scopes satisfy the classification of the underlying entity.

### Rule 20: Proposals, Not Mutations
The app-native surface cannot mutate state. It can only produce `ActionProposal`s that the application accepts, rejects, or transforms. Every `ActionProposal` with non-empty `mutations[]` must reference a `compensation_ref` — enforced at construction time.

### Rule 21: Entities Are Tenant-Scoped by Default
An `EntityRef` carries an implicit tenant dimension. Cross-tenant entity access is denied unless the policy pack declares a federation agreement that permits it.

### Rule 22: Identity Is Cryptographic
Every agent has a DID (`did:aaf:<24-hex>`). The runtime's pre-plan hook short-circuits revoked DIDs before the trace opens.

### Rule 23: Signed Manifest
An agent manifest can only be constructed via `ManifestBuilder::build()`, which signs it. Verification checks well-formedness and signature.

### Rule 24: Provenance as BOM
Every agent declares a software bill of materials (7-kind classification with content hashes). Exportable as SPDX 2.3 or CycloneDX 1.5.

---

## Enhancement Slicing Strategy

Each enhancement is split into three ordered slices (A -> B -> C). Every slice must:
1. Preserve all existing tests (`cargo test --workspace` stays green)
2. Add its own tests (every new public type has at least one unit test)
3. Keep `cargo clippy --workspace` clean
4. Leave the code base in a shippable state

**Work order:** E2 -> E1 -> E3 (Wave 1), then X1 -> X2 -> X3 (Wave 2), then F2 -> F1 -> F3 (Wave 4). Within an enhancement, slices land in order A -> B -> C. Never leapfrog.

**Additional crates introduced by enhancements:**

| Crate | Enhancement | Purpose |
|---|---|---|
| `aaf-ontology` | E2 | Entity definitions, classification lattice, relations, lineage, versioning |
| `aaf-eval` | E1 | Judge trait, golden suites, replay/divergence, regression reports |
| `aaf-learn` | E1 | Trace subscribers: fast-path miner, capability scorer, router tuner, escalation tuner |
| `aaf-surface` | E3 | AppEvent, Situation, ActionProposal, StateProjection, EventRouter |
| `aaf-identity` | X1 | DID, signed manifests, SBOM, capability tokens, revocation |
| `aaf-mcp` | F3 | MCP client + server bridge (adapters/mcp/) |
| `aaf-a2a` | F3 | A2A participant bridge (adapters/a2a/) |

---

## Performance Targets

| Pattern | p50 | p99 | AAF Overhead |
|---|---|---|---|
| Fast Path | 5ms | 20ms | <5ms |
| Agent Assisted | 100ms | 500ms | 50-200ms |
| Full Agentic | 2s | 15s | 1-10s |

| Metric | Target |
|---|---|
| Fast Path rate | >60% |
| Intent Cache hit | >40% |
| Intent Resolution | >97% |
| Sidecar overhead | <5ms p99 |
| LLM cost/intent | <$0.01 avg |

---

## Enhancement Architecture Rules (Wave 4, extending Rules 14–24)

### Rule 34: SDKs Are Generated, Not Hand-Written
All contract types in SDKs are generated from `spec/schemas/` JSON Schemas. The generation pipeline runs as part of `make codegen`. Hand-written SDK types that duplicate schema definitions are a build error. SDK-specific ergonomic wrappers (decorators, builders) are hand-written but delegate to generated types internally.

### Rule 35: Providers Are Observable
Every LLM provider call records: `model_name`, `input_tokens`, `output_tokens`, `cost_usd`, `latency_ms`, `stop_reason`, and `rate_limit_remaining`. This data is wired into the trace system (Rule 12) and budget tracker (Rule 8). Provider calls without observation recording are a bug.

### Rule 36: Protocol Bridges Are Governed
Every external protocol interaction (MCP tool call, A2A task delegation, REST API call) passes through the policy engine (Rule 6), respects trust boundaries (Rule 22), charges the budget (Rule 8), and records an observation (Rule 12). Ungoverned external calls are architecturally impossible.

### Rule 37: SDK Ergonomics Over Completeness
SDKs should make the common case trivial (define a capability in 5 lines, submit an intent in 3 lines). A Python decorator that "just works" is worth more than 100% contract coverage with verbose builders. Ergonomics are a first-class design constraint, not a nice-to-have.

### Rule 38: Bridge Failures Are Graceful
When an MCP server is unreachable, an A2A agent is unavailable, or an external API times out, the bridge degrades gracefully following the Degradation Chain (Levels 0-4). Unavailability removes the capability from the active registry; it never causes a pipeline crash.

### Rule 39: Providers Use reqwest, Not SDKs
LLM providers use `reqwest` directly against provider HTTP APIs. Do not import `anthropic-sdk`, `openai`, or any provider-specific Rust crate. This keeps the dependency tree minimal and gives full control over retry logic, timeout handling, and observability. Every HTTP call records `ProviderMetrics`.

### Rule 40: Every External Call Is Governed
No HTTP request, gRPC call, MCP tool invocation, or A2A task delegation may bypass the policy engine. The `CapabilityDispatcher` enforces: policy pre-check (Rule 6 PreStep) → invoke → policy post-check (Rule 6 PostStep) → record observation (Rule 12) → charge budget (Rule 8). Ungoverned external calls are a build-breaking bug.

### Rule 41: Fallback to Mock Is Always Available
Every real provider (Anthropic, OpenAI, HTTP invoker, MCP bridge) must degrade gracefully when its target is unreachable. The degradation chain (Levels 0–4) applies to providers, not just the overall system. Tests must verify both the happy path and the fallback path.

### Rule 42: SDKs Are Thin Clients
SDKs (`sdk/python/`, `sdk/typescript/`) are HTTP clients that talk to `aaf-server`'s REST API. They do not embed Rust code, do not run the policy engine locally, and do not include any business logic. Contract types are generated from `spec/schemas/` (Rule 34). Ergonomic wrappers (decorators, builders) are hand-written but delegate to generated types.

### Rule 43: The Hello World Must Work in 5 Minutes
The `examples/real-world-hello/` example must be runnable by a developer with no Rust knowledge. `docker compose up` starts everything. `curl` or the Python SDK demonstrates the flow. If it takes longer than 5 minutes from clone to "wow", the onboarding is broken.

---

## Wave 4 Implementation Guide (F2 → F1 → F3)

### F2 — Live LLM Integration & Intelligent Model Routing

#### Provider Trait Extension (`aaf-llm`)

Add `ProviderMetrics` to every response for observability (Rule 35):

```rust
/// Metrics recorded for every LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProviderMetrics {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub latency_ms: u64,
    pub rate_limit_remaining: Option<u32>,
    pub provider: String,
}
```

#### Anthropic Claude Provider (`aaf-llm/src/anthropic.rs`)

```rust
pub struct AnthropicProvider {
    client: reqwest::Client,
    api_key: String,              // from env ANTHROPIC_API_KEY
    base_url: String,             // default: https://api.anthropic.com
    default_model: String,        // default: claude-sonnet-4-6-20250514
    max_retries: u32,             // default: 3
}
```

Request mapping: AAF `ChatRequest` messages → Anthropic Messages API format, including tools/tool_choice. Response mapping extracts real token counts, calculates cost from pricing table, records latency. Rate limits handled with exponential backoff.

#### OpenAI Provider (`aaf-llm/src/openai.rs`)

Maps `ChatRequest` → OpenAI Chat Completions API. Tools mapped to functions format.

#### Local Provider (`aaf-llm/src/local.rs`)

OpenAI-compatible API targeting local endpoints (Ollama, vLLM). Default `http://localhost:11434`.

#### Value-Based Router (`aaf-llm/src/router.rs`)

```rust
pub struct ValueRouter {
    providers: Vec<Arc<dyn LLMProvider>>,
    model_catalog: Vec<ModelProfile>,
    health: Arc<ProviderHealth>,
}
```

Routing algorithm: filter by capability (tools/streaming) → filter by health → filter by budget → filter by latency → filter by classification → score by cost (40%) + latency (30%) + capability (30%) → select highest.

#### Dependencies

Add to `aaf-llm/Cargo.toml`: `reqwest` (with json+stream features), `wiremock` (dev, for testing).

Use `reqwest` directly instead of provider SDKs for unified error handling and minimal dependency tree.

#### F2 Slices

**Slice A — Status: LANDED:** `AnthropicProvider` (`aaf-llm/src/anthropic.rs`) + `ProviderMetrics` + `ModelPricing` (`aaf-llm/src/pricing.rs`) + MockProvider update + rate limit handling with exponential backoff. Implemented with `reqwest` (Rule 39), wiremock tests.
**Slice B:** `OpenAiProvider` + `LocalProvider` + `ValueRouter` with scoring + health tracking + auto-fallback
**Slice C:** Streaming (`chat_stream`) + budget pre-check + provider configuration from config file + classification-aware filtering

### F1 — Developer Experience Platform

#### Python SDK (`sdk/python/`)

Key components:
- `@capability` decorator with `CapabilityMeta` (name, side_effect, sla, etc.)
- `@guard.input()` / `@guard.output()` decorators
- `@compensation(for_capability=...)` decorator
- `AafClient` with `submit_intent()` and `submit_intent_stream()` via `httpx`
- `MockRuntime` for in-process testing without starting the Rust server
- CLI: `aaf init`, `aaf dev`, `aaf test`, `aaf run`, `aaf trace`

Dependencies: `httpx>=0.27`, `pydantic>=2.0`, `click>=8.0`. Dev: `pytest`, `ruff`, `mypy --strict`.

#### TypeScript SDK (`sdk/typescript/`)

Type-safe capability builder with `zod` schema validation. `AafClient` class. EventSource consumer for streaming. Vitest test utilities.

#### Go SDK (`sdk/go/`)

Minimal surface: `AafClient`, capability registration, sidecar builder, `context.Context`-threaded.

#### Contract Code Generation (`scripts/codegen/`)

`generate.py` reads `spec/schemas/*.schema.json` and outputs:
- Python: pydantic v2 models
- TypeScript: zod schemas + TS interfaces
- Go: Go structs

Run via `make codegen`.

#### F1 Slices

**Slice A:** Python SDK core — codegen, `@capability`, `@guard`, `@compensation`, `AafClient`, `MockRuntime`, tests
**Slice B:** TypeScript SDK + CLI — codegen, builders, client, streaming, `aaf init/dev/test/run/trace`
**Slice C:** Go SDK + advanced builders — client, sidecar, wrapper, saga builders, end-to-end example

### F3 — Universal Protocol Bridge (MCP + A2A)

#### MCP Client (`adapters/mcp/`)

```rust
pub struct McpClient {
    transport: Box<dyn McpTransport>,
    registry: Arc<dyn CapabilityRegistry>,
    policy: Arc<PolicyEngine>,
    budget: Arc<BudgetTracker>,
    trace: Arc<TraceRecorder>,
}
```

`discover_and_register()`: calls `tools/list`, maps each MCP tool → `CapabilityContract` (prefixed `mcp:{server}:`), registers in AAF registry.

`invoke_tool()`: policy check (PreStep) → budget check → `tools/call` → policy check (PostStep) → record observation → charge budget → return result.

Three transports: `stdio` (local), `sse` (remote), `streamable_http`.

#### MCP Server (`adapters/mcp/server/`)

Exposes AAF capabilities as MCP tools. `handle_list_tools()` queries registry. `handle_call_tool()` builds `IntentEnvelope` and submits to runtime (full governance applies).

#### A2A Participant (`adapters/a2a/`)

Agent Card serving, task lifecycle (send/get/cancel), DID-based trust propagation (Rule 22), SSE streaming for updates, federation agreement enforcement.

#### ProtocolBridge Unifier

```rust
impl CapabilityInvoker for ProtocolBridge {
    async fn invoke(&self, capability_id, input, intent) -> Result<Value, InvokeError> {
        match self.resolve_protocol(capability_id) {
            Protocol::Local => self.local_invoker.invoke(...),
            Protocol::Mcp(server) => self.mcp_client.invoke_tool(...),
            Protocol::A2a(agent) => self.a2a_participant.handle_task_send(...),
        }
    }
}
```

#### F3 Slices

**Slice A:** MCP client — stdio transport, `McpClient`, tool mapping, governed invocation, config schema
**Slice B:** MCP server + SSE/streamable HTTP transports, wire into `aaf-server`
**Slice C:** A2A participant + `ProtocolBridge` unifier, wire into `aaf-server`

## Three Pillars — Detailed Slice Guide

The Three Pillars (Brain, Hands, Gateway) map onto F2, F1, F3 but provide finer-grained implementation detail per slice.

| Pillar | Codename | What It Adds | Primary Crates |
|--------|----------|-------------|----------------|
| 1 | **Brain** | Real LLM providers, intelligent intent/planning/recovery | `aaf-llm`, `aaf-intent`, `aaf-planner`, `aaf-saga` |
| 2 | **Hands** | Real service invocation via HTTP, MCP, A2A | `aaf-runtime`, `aaf-transport`, `aaf-server` |
| 3 | **Gateway** | HTTP API, config-driven setup, Python/TS SDKs | `aaf-server`, `sdk/python/`, `sdk/typescript/` |

**Implementation order:** P1-A and P2-A run in parallel → P1-B, P2-B, and P3-A in parallel → P1-C, P2-C, P3-B → P3-C.

### P1 (Brain) Slices

**P1 Slice A — Status: LANDED.** `AnthropicProvider` in `aaf-llm/src/anthropic.rs`, `ModelPricing` in `aaf-llm/src/pricing.rs`. Request/response mapping for Anthropic Messages API. Retry with exponential backoff on 429/529/5xx. Pricing-based cost calculation. Tests via `wiremock`. `reqwest` 0.12 added.

**P1 Slice B:** OpenAI + Local providers + Enhanced Router.
- `aaf-llm/src/openai.rs` — OpenAI Chat Completions provider (same pattern as Anthropic, maps tools to functions format)
- `aaf-llm/src/local.rs` — OpenAI-compatible local provider (Ollama/vLLM, default `http://localhost:11434/v1`, zero-cost pricing, no 429 retry)
- `aaf-llm/src/health.rs` — `ProviderHealth` with sliding window (100 calls), `success_rate()`, `p50_latency()`, `p99_latency()`, `is_healthy()` (success_rate > 0.5)
- `aaf-llm/src/router.rs` — filter unhealthy providers before scoring, retry with next-best on failure, record fallback in trace

**P1 Slice C:** LLM-Powered Intelligence + Streaming.
- `aaf-intent/src/llm_classifier.rs` — LLM-based intent classification (structured JSON output, confidence > 0.7, falls back to RuleClassifier)
- `aaf-planner/src/llm_planner.rs` — LLM-based plan generation (system prompt includes available capabilities, validates against composition safety)
- `aaf-saga/src/llm_recovery.rs` — LLM-based failure analysis (returns `RecoveryDecision`, falls back to rule-based)
- `aaf-llm/src/provider.rs` — add `chat_stream()` (optional, default returns error)
- `aaf-llm/src/anthropic.rs` — implement streaming
- Key principle: LLM enhances, never replaces. Every LLM-powered component has a deterministic fallback.

### P2 (Hands) Slices

**P2 Slice A — Status: LANDED.** `GoverningToolExecutor`, `InProcessInvoker`, `ServiceInvoker` in `aaf-runtime/src/invoke.rs`. `CapabilityInvoker` trait with `InvocationContext` and `InvocationResult`. HTTP invoker in `aaf-transport`. `CapabilityDispatcher` dispatches by endpoint kind. `governed-invocation` example demonstrates the full chain.

**P2 Slice B:** MCP Client Bridge + MCP Server.
- `aaf-transport/src/mcp_client.rs` — `McpClient` with `McpTransportKind::Stdio` / `McpTransportKind::Sse`
- Discovery maps MCP tools → AAF capabilities (prefixed `mcp:{server_name}:{tool_name}`, conservative `SideEffect::Write` default)
- `aaf-server/src/mcp_server.rs` — Expose AAF capabilities as MCP tools, `handle_call_tool` builds `IntentEnvelope` and submits to runtime (fully governed)
- Add MCP to `CapabilityDispatcher`

**P2 Slice C:** A2A Bridge + InProcess Invoker.
- `aaf-transport/src/a2a_client.rs` — A2A protocol client (Agent Card fetch, task POST/poll, DID verification per Rule 22, federation boundary enforcement)
- `aaf-runtime/src/invoke_inprocess.rs` — In-process capability invoker for wrapper pattern (near-zero overhead, `EndpointKind::InProcess`)
- Add A2A and InProcess to `CapabilityDispatcher`

### P3 (Gateway) Slices

**P3 Slice A:** HTTP API Server + Config-Driven Setup.
- `aaf-server/src/api/rest.rs` — axum handlers: `POST /v1/intents`, `GET /v1/intents/{id}`, `POST /v1/capabilities`, `GET /v1/capabilities`, `GET /v1/capabilities/discover?q=`, `GET /v1/traces/{id}`, `GET /v1/health`
- `aaf-server/src/config.rs` — `AafConfig` (server, llm, capabilities, policies, trust) parsed from YAML
- Config-driven wiring: `serve` subcommand loads config, instantiates providers and capabilities

**P3 Slice B:** Python SDK + TypeScript SDK + Code Generation.
- `sdk/python/` — `AafClient` (httpx), `@capability`/`@guard`/`@compensation` decorators, generated pydantic v2 contracts, `MockRuntime`
- `sdk/typescript/` — `AafClient`, zod-validated contracts, EventSource streaming
- `scripts/codegen/generate.py` — reads `spec/schemas/*.schema.json`, outputs pydantic models + zod schemas
- Run via `make codegen`, verify with `--check` flag

**P3 Slice C:** End-to-End Hello World + Documentation.
- `examples/real-world-hello/` — docker-compose (AAF server + FastAPI demo service), `aaf.yaml` config, capability contracts, `test.sh` smoke test
- Must satisfy Rule 43: clone → "wow" in 5 minutes

---

### Wave 4 Validation Gates (per slice)

Every slice must pass before merge:

1. `cargo build --workspace` — zero warnings
2. `cargo test --workspace` — zero failures, test count ratchets up
3. `make clippy` — zero warnings
4. `make schema-validate` — all examples valid
5. New Rust public types have `Debug, Clone, Serialize, Deserialize`
6. New Rust public items have `///` doc comments
7. No `unwrap()` in Rust library code
8. New `#[serde(default)]` on all wire-format fields
9. Python: `ruff check` + `mypy --strict` clean
10. TypeScript: `tsc --noEmit` + `eslint` clean
11. Go: `golangci-lint run` clean

### Key Design Decisions

**HTTP-first for SDK clients:** No code generation required, works everywhere (browsers, serverless, edge), easier to debug. gRPC used for internal runtime communication.

**reqwest for LLM providers:** Avoids heavy dependency trees from provider SDKs, gives fine-grained control over retry/timeout/streaming, unified error handling.

**MCP as Rust crate (not separate service):** MCP tool calls must go through the policy engine (in-process), sub-millisecond overhead for local MCP servers, configuration-gated.

**SDKs as thin clients:** Runtime is Rust; SDKs are Python/TS/Go — no FFI complexity. Single deployment of runtime serves all languages. Policy enforcement happens server-side.

---

## Dependency Management

New external dependencies introduced by the three pillars:

| Crate/Package | Used By | Purpose |
|---|---|---|
| `reqwest` 0.12 | aaf-llm, aaf-transport | HTTP client for LLM APIs and service invocation |
| `wiremock` 0.6 | dev-dependencies | HTTP mock server for testing |
| `axum` 0.7 | aaf-server | HTTP API server |
| `tower` 0.4 | aaf-server | Middleware (timeout, cors, tracing) |
| `tower-http` 0.5 | aaf-server | HTTP-specific tower middleware |
| `tokio-stream` 0.1 | aaf-llm | Streaming response handling |
| `httpx` >=0.27 | sdk/python | Python HTTP client |
| `pydantic` >=2.0 | sdk/python | Python data validation |
| `click` >=8.0 | sdk/python | Python CLI |
| `zod` >=3.22 | sdk/typescript | TypeScript schema validation |

All Rust dependencies are pinned to exact versions (existing convention).

---

## What NOT to Do

- Do NOT add `anthropic` or `openai` Rust crates as dependencies (Rule 39: use reqwest)
- Do NOT embed Rust code in Python/TypeScript SDKs (Rule 42: thin HTTP clients)
- Do NOT add real database drivers yet (Rule 11: in-memory backends are sufficient)
- Do NOT build a dashboard UI (CLI + REST API is sufficient)
- Do NOT modify existing contract types unless absolutely necessary
- Do NOT break any existing tests — all 554 must continue passing
- Do NOT add optional features or feature flags for providers — they are always compiled
- Do NOT hardcode API keys anywhere — always from environment variables

---

## Codebase Stats

- Tests: 554
- Examples: 13
- Estimated size: ~26K lines
- Version: 0.2.0