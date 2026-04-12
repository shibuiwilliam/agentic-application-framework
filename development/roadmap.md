# Roadmap

> Where we are, where we are going, and what is explicitly deferred.
> For the full vision read `PROJECT.md` §§1–18; this file is the
> *status board*.

Last updated: post-iteration 10 + examples expansion.

---

## Status at a glance

| Metric | Value |
|---|---|
| Crates in the workspace | **22** |
| Unit + integration tests passing | **463** |
| Rust lines (`core/crates/**/src/*.rs`) | **~29,000** |
| Build warnings | **0** |
| Clippy warnings (`-W clippy::all`) | **0** |
| JSON schemas | **18** |
| Example configs | **11** (9 validate against their schema) |
| Ontology lint | **strict mode, 0 errors** |
| ADRs | **2** (ADR-008, ADR-017) |
| CLI subcommands | **12** |
| Runnable examples | **8** |

---

## Enhancement status matrix

```
                   Slice A     Slice B     Slice C
E2 Ontology        ✓ iter 4    ✓ iter 7    ✓ iter 8          ← complete
E1 Feedback        ✓ iter 4    ✓ iter 5    ✗ deferred
E3 App-Native      ✓ iter 4    ✗ deferred  ✗ deferred
X1 Identity        ✓ iter 6    ✓ iter 9    ✓ iter 10         ← complete
X2 Knowledge       ✗ deferred  ✗ deferred  ✗ deferred
X3 DX Surface      ✗ deferred  ✗ deferred  ✗ deferred
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
| — | Examples | 8 runnable examples: hello-agent, order-saga, resilient-query, feedback-loop, memory-context, app-native-surface, cross-cell-federation, signed-agent. 90 new integration tests. Documentation merge (PROJECT_AafService.md → PROJECT.md §19, CLAUDE_AaFService.md removed). New dev docs: examples-walkthrough, error-handling, capability-authoring (463 tests) |

---

## What is next (iteration 11+)

### Iteration 11 — E1 Slice C (Feedback Spine completion)

**Scope:**

- `aaf learn` CLI subcommand for managing learned rules
- `make test-semantic-regression` target
- Governance docs for learning pipeline

**Dependencies:** E1 Slice B landed.

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
  Ollama/vLLM. The `LLMProvider` trait is stable; adding a real
  provider is one-file scope.
- **Protobuf codegen** — `spec/proto/` + `buf` to produce Rust /
  Python / TypeScript / Go bindings. Deferred — the hand-written
  `aaf-contracts` shapes are the source of truth today.
- **gRPC / REST / WebSocket surfaces** on `aaf-server`. Deferred
  until the Wave-1 enhancements are complete.
- **Front Door UI** — React chat + approval gate + trace viewer.
- **Dashboard UI** — metrics / health map / trace explorer.
- **Python / TypeScript / Go SDKs** — part of X3.
- **Helm / Terraform / Docker** packaging.
- **Edition 2024 migration** — waiting for deployment toolchain.

---

## Exit criteria for "v1.0"

The framework is considered *v1.0-ready* when:

1. Every enhancement (E1, E2, E3, X1, X2, X3) has landed Slice C.
2. At least one real LLM provider (Anthropic) is wired behind the
   `LLMProvider` trait.
3. At least one real storage backend (PostgreSQL via `sqlx`) is
   wired behind the storage traits.
4. A reference application under `examples/app-native/` runs
   end-to-end: user opens an Order page → `AppEvent` → `Intent` →
   planner → runtime → `ActionProposal` rendered inline → user
   accepts → saga executes → outcome flows back through E1.
5. CI runs `aaf-eval` regression gates on every merge.
6. `make ontology-lint` runs in strict mode across every example
   and across every capability registered in every reference app.

Iterations 1–10 have taken the framework roughly 55% of the way to
v1.0 by rough reckoning. E2 and X1 are complete; E1 has Slice A/B
landed; E3 has Slice A landed. The remaining work is E1 Slice C,
E3 Slices B/C, and the full Wave 2 (X2/X3).
