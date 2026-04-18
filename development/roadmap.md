# Roadmap

> Where we are, where we are going, and what is explicitly deferred.
> For the full vision read `PROJECT.md` §§1–20; this file is the
> *status board*.

Last updated: post Wave 4 Pillar 1-A / Pillar 2-A landing (Anthropic
provider, capability invocation bridge, governed-invocation example).

---

## Status at a glance

| Metric | Value |
|---|---|
| Crates in the workspace | **22** (incl. `aaf-learn`) |
| Unit + integration tests passing | **554** |
| Rust lines (`core/crates/**/src/*.rs`) | **~26,000** |
| Build warnings | **0** |
| Clippy warnings (`-W clippy::all`) | **0** |
| JSON schemas | **18** |
| Example configs | **11** (all validate against their schema) |
| Ontology lint | **strict mode, 0 errors** |
| ADRs | **2** (ADR-008, ADR-017) |
| CLI subcommands | **13** |
| Runnable examples | **13** |

---

## Enhancement status matrix

```
                   Slice A     Slice B     Slice C
E2 Ontology        ✓ iter 4    ✓ iter 7    ✓ iter 8          ← complete
E1 Feedback        ✓ iter 4    ✓           ✗ deferred
E3 App-Native      ✓ iter 4    ✗ deferred  ✗ deferred
X1 Identity        ✓ iter 6    ✓ iter 9    ✓ iter 10         ← complete
X2 Knowledge       ✗ deferred  ✗ deferred  ✗ deferred
X3 DX Surface      ✗ deferred  ✗ deferred  ✗ deferred
F2 LLM Integration ✓ landed    ✗ planned   ✗ planned         ← Wave 4
F1 Developer XP    ✗ planned   ✗ planned   ✗ planned         ← Wave 4
F3 Protocol Bridge ✗ planned   ✗ planned   ✗ planned         ← Wave 4
```

---

## What has landed, by iteration

| # | Slice | Scope |
|---|---|---|
| 1 | Foundation | 17 core crates, in-memory backends, 98 tests |
| 2 | Foundation fix-forward | Multi-step planner, saga/runtime bridge, approval node integration, full-pipeline test, 4 bugs fixed |
| 3 | Foundation polish | Compensation chain wired, OTel export, cost attribution, A2A import, 5 schemas + 2 examples, base policies, server subcommands |
| 4 | Wave 1 Slice A | `aaf-ontology`, `aaf-eval`, `aaf-surface` crates; E2/E1/E3 contract foundations |
| 5 | Audit + quality | 12 contract literal updates, task state extension, clippy cleanup, schema/example fixes (177 tests) |
| 6 | Wave 2 X1 Slice A | `aaf-identity` crate: DID, Keystore, Manifest, SBOM, Attestation, Token, Revocation |
| 7 | E2 Slice B | Ontology wired into planner / policy / memory / intent / registry (236 tests) |
| 8 | E2 Slice C | Federation in entity-space + `ontology lint` + `ontology import` + ADR-008 (268 tests) |
| 9 | X1 Slice B | Runtime revocation gate + trust token verify + registry attestation gate + DID-bound signing |
| 10 | X1 Slice C | SBOM exporters (SPDX + CycloneDX), co-signed tokens, identity CLI, signed-agent example, ADR-017 |
| — | Quality | Compensation chain bug fix, saga preserve tracking, EventWaitNode timeout, PlanCache bounded eviction, PolicyHook-aware rules, latency validation, working memory cleanup (413 tests) |
| — | Examples | 9 runnable examples: hello-agent, order-saga, resilient-query, feedback-loop, memory-context, app-native-surface, cross-cell-federation, signed-agent, eval-golden. 90 new integration tests. Documentation merge (PROJECT_AafService.md → PROJECT.md §19, CLAUDE_AaFService.md removed). New dev docs: examples-walkthrough, error-handling, capability-authoring (475 tests) |
| — | Wave 4 P1-A | `AnthropicProvider` + `ProviderMetrics` + `ModelPricing` in `aaf-llm`. F2 Slice A landed. `HttpSender` / `FixedSender` traits for testability. Pricing table with per-model cost calculation. |
| — | Wave 4 P2-A | `invoke.rs` in `aaf-runtime`: `ServiceInvoker` trait, `GoverningToolExecutor`, `InProcessInvoker`. Capability invocation bridge connecting ToolExecutor to real service endpoints. `governed-invocation` example. 4 new examples (agentic-tool-loop, parallel-orchestration, sidecar-gateway, governed-invocation). (554 tests) |
| — | Doc merge | `PROJECT_ENHANCE.md` and `CLAUDE_ENHANCE.md` merged into `PROJECT.md` §20 and `CLAUDE.md` respectively. Rules 39–43 added. Development and public docs updated. |

---

## What is next (iteration 11+)

### Iteration 11 — E1 Slice C (Feedback Spine completion)

**Scope:**

- `aaf learn` CLI subcommand for managing learned rules
  (list proposals, approve, reject, inspect evidence)
- `make test-semantic-regression` Makefile target
- Governance docs for the learning pipeline under `docs/`
- Production-grade anomaly detection on evidence concentration

**Dependencies:** E1 Slice B landed (✓).

### Iteration 12 — E3 Slice B (App-Native Surface integration)

**Scope:**

- `EventGateway` in `aaf-sidecar` + `aaf-wrapper` that ingests
  `AppEvent` and forwards to the intent compiler.
- `aaf-policy::guard::action::ActionGuard` extended to understand
  `StateMutationProposal` as a distinct side-effect class.
- `aaf-memory::thread` keyed by `(user_id, tenant_id, surface)` so
  per-surface context survives across events.
- Proposal outcomes flow into trace — `Observation.outcome_detail`
  populated by proposal accept/reject/transform, which is then the
  highest-value signal into E1 Slice B.
- Integration test under
  `core/tests/integration/tests/e3_slice_b_smoke.rs`.

### Iteration 13 — E3 Slice C + polish

**Scope:**

- Python / TypeScript SDK primitives
- `examples/app-native/` reference application
- WebSocket proposal channel

---

## Wave 4 — F2 / F1 / F3 (Critical Infrastructure)

> See `PROJECT.md` §20 for the full design rationale and
> `CLAUDE.md` rules 34–38 for the architecture constraints.

Wave 4 addresses three prerequisites for framework viability that
are more urgent than Wave 2/3 feature enhancements. Work order:
**F2 → F1 → F3**.

### F2 — Live LLM Integration & Intelligent Model Routing

**Modified crate:** `aaf-llm`.

**Scope:**
- Anthropic Claude provider (Messages API, tools, streaming, rate limits)
- OpenAI provider (Chat Completions API, function calling)
- Local provider (Ollama / vLLM via OpenAI-compatible API)
- `ProviderMetrics` on every response (Rule 35)
- Value-based router with scoring (cost 40% + latency 30% + capability 30%)
- Pricing table with per-provider model catalogs
- Health tracking, automatic fallback on provider failure
- Budget pre-check before LLM calls
- Dependencies: `reqwest` (json+stream), `wiremock` (dev)

**Slices:**
- A: `AnthropicProvider` + `ProviderMetrics` + pricing table — **LANDED**
- B: `OpenAiProvider` + `LocalProvider` + `ValueRouter` + health + fallback
- C: Streaming + budget pre-check + config loading + classification filtering

### F1 — Developer Experience Platform

**New packages:** `sdk/python/`, `sdk/typescript/`, `sdk/go/`,
`scripts/codegen/`.

**Scope:**
- Python SDK: `@capability` / `@guard` / `@compensation` decorators,
  pydantic v2 models from JSON Schema, `AafClient`, `MockRuntime`,
  `aaf` CLI (init / dev / test / run / trace)
- TypeScript SDK: zod schemas, type-safe builders, streaming consumer
- Go SDK: minimal client + sidecar + wrapper
- Code generation: `spec/schemas/` → pydantic / zod / Go structs
- Dependencies (Python): httpx, pydantic v2, click, pytest, ruff, mypy

**Slices:**
- A: Python SDK core (codegen, decorators, client, testing)
- B: TypeScript SDK + CLI commands
- C: Go SDK + sidecar/wrapper builders + end-to-end example

### F3 — Universal Protocol Bridge (MCP + A2A)

**New crates:** `adapters/mcp/` (Rust), `adapters/a2a/` (Rust).

**Scope:**
- MCP client: stdio/SSE/streamable-HTTP transports, tool discovery →
  capability registration, governed invocation (Rule 36)
- MCP server: expose AAF capabilities as MCP tools for AI IDEs
- A2A participant: Agent Card serving, task lifecycle, DID-based trust
- `ProtocolBridge` unifier: local + MCP + A2A capability invocation
- Dependencies: reqwest, tokio-tungstenite, eventsource-stream

**Slices:**
- A: MCP client (stdio transport, discovery, governed invocation)
- B: MCP server + SSE/streamable HTTP transports
- C: A2A participant + ProtocolBridge unifier

### Recommended Wave 4 / Wave 3 interleaving

```
F2-A → E4-A → F1-A → E4-B → F2-B → E5-A → F3-A → F1-B → ...
```

---

## Wave 2 — X2 / X3 (iteration 13+)

Wave 2 adds three more enhancements on top of Wave 1. Once E1 / E3
are complete, pick up:

### X2 Semantic Knowledge Fabric

**New crate:** `aaf-knowledge`.

**Scope:** `KnowledgeSource`, `GroundedChunk` (with
`classification`, `freshness_token`, `embedding_version`), a
`RetrievalStrategy` trait with BM25 / vector / hybrid / rerank /
HyDE impls, a `VectorStore` trait behind `aaf-storage` with an
in-memory impl, freshness policy with entity-write invalidation,
`grounds_from: Vec<SourceId>` on `CapabilityContract`,
`grounding_refs: Vec<ChunkId>` on `Observation`, a
`GroundingArtifact` type, and policy enforcement that refuses
classification downgrades on retrieval.

**Dependencies:** E2 (for entity-keyed lineage), X1 (for signed
grounding artifacts).

### X3 Developer Experience Surface

**Scope:** `aaf-sdk-rs` (proc macros: `#[intent]`, `#[capability]`,
`#[on_event]`, `#[project]`, `#[accept_proposal]`), `aaf-hotloop`
binary (file-watch → rebuild → trace diff), `aaf-sim` (replay
production traces + load test + chaos), `aaf-snapshot` (snapshot
test macro). Python / TypeScript mirror SDKs.

**Dependencies:** X1 (signed artifacts from decorator output), X2
(knowledge fabric visible to decorator tests).

---

## Explicitly deferred

These are known gaps and are **deliberately** not on any
iteration's scope:

- **Real storage drivers** — PostgreSQL via `sqlx`, Redis,
  S3-compatible, ClickHouse, pgvector/Qdrant. Deferred to after
  Wave 2 Slice C.
- **Real LLM providers** — Anthropic, OpenAI, Bedrock, Vertex,
  Ollama/vLLM. **Promoted to Wave 4 (F2).** The `LLMProvider` trait
  is stable; F2 adds concrete providers.
- **Protobuf codegen** — `spec/proto/` + `buf` to produce Rust /
  Python / TypeScript / Go bindings. Deferred — the hand-written
  `aaf-contracts` shapes are the source of truth today.
- **gRPC / REST / WebSocket surfaces** on `aaf-server`. Deferred
  until the Wave-1 enhancements are complete.
- **Front Door UI** — React chat + approval gate + trace viewer.
- **Dashboard UI** — metrics / health map / trace explorer.
- **Python / TypeScript / Go SDKs** — **Promoted to Wave 4 (F1).**
  SDKs are thin clients over HTTP/gRPC/WebSocket APIs.
- **MCP + A2A protocol bridges** — **Promoted to Wave 4 (F3).**
  Governed bridges connecting AAF to the broader AI ecosystem.
- **Helm / Terraform / Docker** packaging.
- **Edition 2024 migration** — waiting for deployment toolchain.

---

## Exit criteria for "v1.0"

The framework is considered *v1.0-ready* when:

1. Every enhancement (E1, E2, E3, X1, X2, X3) has landed Slice C.
2. At least one real LLM provider (Anthropic) is wired behind the
   `LLMProvider` trait — **Wave 4 F2**.
3. At least one real storage backend (PostgreSQL via `sqlx`) is
   wired behind the storage traits.
4. Python and TypeScript SDKs are functional — **Wave 4 F1**.
5. MCP client bridge operational with governed tool invocation —
   **Wave 4 F3**.
6. A reference application under `examples/app-native/` runs
   end-to-end: user opens an Order page → `AppEvent` → `Intent` →
   planner → runtime → `ActionProposal` rendered inline → user
   accepts → saga executes → outcome flows back through E1.
7. CI runs `aaf-eval` regression gates on every merge.
8. `make ontology-lint` runs in strict mode across every example
   and across every capability registered in every reference app.

The framework is roughly 65% of the way to v1.0. E2 and X1 are
complete; E1 has Slices A/B landed (with `aaf-learn` crate and
subscribers operational); E3 has Slice A landed. Wave 4 F2 Slice A
(Anthropic provider) and Pillar 2 Slice A (capability invocation
bridge) have landed with the `governed-invocation` example. The
remaining work is E1 Slice C, E3 Slices B/C, Wave 2 (X2/X3),
Wave 4 F2 Slices B/C, F1, and F3.
