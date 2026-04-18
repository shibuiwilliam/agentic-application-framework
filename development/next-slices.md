# Next Slices — Concrete Playbook

> The single most valuable continuity document. If you are a
> Claude Code session picking up where iterations 1–9 left off,
> this file tells you **exactly** what the next three slices
> should land, which crates to touch, which tests to add, and
> which success criteria to verify.
>
> Update this file at the end of every iteration: mark the
> completed slice `✓`, append a row to the iteration log in
> `development/roadmap.md`, and promote the next slice to the
> top.

---

## Status coming in

| Enhancement | A | B | C |
|---|---|---|---|
| E2 Domain Ontology | ✓ | ✓ | ✓ |
| E1 Feedback Spine | ✓ | ✓ | **next** |
| E3 App-Native Surface | ✓ | pending | pending |
| X1 Agent Identity | ✓ | ✓ | ✓ |
| X2 Knowledge Fabric | pending | pending | pending |
| X3 DX Surface | pending | pending | pending |
| F2 LLM Integration | **✓** | pending | pending |
| F1 Developer XP | planned | planned | planned |
| F3 Protocol Bridge | planned | planned | planned |
| P2 Capability Invocation | **✓** | pending | pending |

Per `PROJECT.md` §18 and §20 the next iterations are
**E1 Slice C → E3 Slice B → E3 Slice C**, followed by Wave 4
**F2 → F1 → F3** (each with 3 slices). Wave 4 may be
interleaved with Wave 3 (E4/E5/E6). Every slice is substantial;
scoping each as its own iteration is the right move.

---

## ✓ Slice — E1 Slice B — `aaf-learn` crate and subscribers (LANDED)

E1 Slice B delivered the `aaf-learn` crate with four subscriber
modules: `FastPathMiner`, `CapabilityScorer`, `RouterTuner`,
`EscalationTuner`. The `TraceSubscriber` trait was added to
`aaf-trace::recorder`. The `RoutingPolicy` trait and
`LearnedRoutingPolicy` were added to `aaf-llm::router`.
`FastPathRuleSet::add_learned` was added to `aaf-planner::fast_path`.
Integration test: `e1_slice_b_smoke.rs`. Rules 15–18 preserved.

---

## Slice 1 — E1 Slice C — CLI, semantic regression, governance

### Motivation

E1 Slices A and B delivered the feedback shape (`Outcome` on every
`Observation`) and the learning loop (`aaf-learn` subscribers that
read outcomes and propose adaptations). What's missing is the
**operational surface**: a CLI for managing learned rules, a CI
target for semantic regression, and governance documentation for
the learning pipeline.

### Scope

| Area | What lands |
|---|---|
| **`aaf learn` CLI subcommand** | `aaf-server learn list` (show proposals), `learn approve <id>`, `learn reject <id>`, `learn inspect <id>` (show evidence). Wired into `aaf-server::main.rs`. |
| **`make test-semantic-regression`** | New Makefile target that loads a golden suite from `spec/examples/eval-suite-order-processing.yaml`, replays it, and fails if any case regresses beyond the configured threshold. |
| **Governance docs** | `docs/learning-governance.md`: how learned rules are proposed, approved, and rolled back; the evidence requirements; the anomaly thresholds; the approval workflow. |
| **Anomaly detection** | `FastPathMiner` gains `min_distinct_sessions` enforcement (adversarial traffic concentration). MinerConfig threshold tuning. |

### Files to touch

- **Edit**
  - `core/crates/aaf-server/src/main.rs` (add `learn` subcommand dispatch)
  - `core/crates/aaf-learn/src/fast_path_miner.rs` (anomaly thresholds)
  - `Makefile` (new `test-semantic-regression` target)
- **New**
  - `docs/learning-governance.md`
  - `core/tests/integration/tests/e1_slice_c_smoke.rs`

### Unit tests expected

- `aaf-server::learn::tests` — at least 3 (list, approve, reject)
- `aaf-learn::fast_path_miner::tests` — at least 2 new (anomaly detection)

### Integration test

- `core/tests/integration/tests/e1_slice_c_smoke.rs`:
  - Propose a learned rule via the miner.
  - List proposals via CLI (assert the rule appears).
  - Approve it.
  - Verify the rule is live in the fast-path rule set.
  - Reject a second rule; verify it does not activate.

### Rules preserved

| Rule | How |
|---|---|
| R17 Every adaptation is reversible | CLI `reject` removes a learned rule; evidence ref preserved. |
| R18 Policy governs learning | Approval workflow unchanged; CLI is a convenience, not a bypass. |

### Success criteria

- `aaf-server learn list|approve|reject|inspect` works.
- `make test-semantic-regression` exits 0 on a clean tree.
- `docs/learning-governance.md` exists and is linked from `docs/README.md`.
- `make ci` stays green.
- 475+ tests passing.

### Deferred after E1 Slice C

- Production-grade anomaly detection with sliding-window statistics.
- `aaf-learn` Prometheus metrics subscriber.

---

## Slice 2 — E3 Slice B — EventGateway + surface-keyed thread memory

### Motivation

E3 Slice A (iteration 4) delivered `AppEvent`, `Situation`,
`ActionProposal`, `StateMutationProposal`, `StateProjection`, and
the `ProposalLifecycle` state machine. Construction-time
enforcement of Rules 19 and 20 already works. What's missing is
**the bridge from an application into the runtime**: today an
`AppEvent` is a struct you can construct, but nothing ingests it
into the hot path, no thread memory is keyed by
`(user_id, tenant_id, surface)`, and the ActionGuard does not
yet treat `StateMutationProposal` as a distinct side-effect
class.

### Scope

| Area | What lands |
|---|---|
| **`EventGateway`** | New module in `aaf-sidecar::gateway` and `aaf-wrapper::gateway`. Accepts `AppEvent`, runs it through an `EventToIntentAdapter` (already shipped in E3 Slice A), yields an `IntentEnvelope`, and forwards to the intent compiler. Enforces: (a) replay safety via the event's idempotency key, (b) per-surface rate limits, (c) per-tenant budget, (d) policy pre-check. |
| **Surface-keyed thread memory** | `aaf-memory::thread` already uses `ThreadId` keys. Slice B adds a helper `thread_id_for_surface(user_id, tenant_id, surface)` that derives a deterministic `ThreadId` so per-surface context survives across events from the same user. |
| **Action guard extension** | `aaf-policy::guard::action::ActionGuard::check_mutation_proposal(intent, proposal)` understands a `StateMutationProposal` as a distinct side-effect class. Enforces Rule 20 construction-time (already done) **plus** a runtime scope check on each field path being mutated. |
| **Proposal outcomes flow into trace** | `Observation.outcome_detail` carries a new optional `proposal_outcome` variant for `Accepted` / `Rejected` / `Transformed` / `Expired`. The saga engine wires accept → saga step, reject → task `Cancelled`, transform → step with edited parameters. |
| **new contracts** | `GatewayConfig` (rate limits, per-surface budgets), `GatewayError`, and a new `ProposalOutcome` variant on `Outcome`. |

### Files to touch

- **New**
  - `core/crates/aaf-sidecar/src/gateway.rs`
  - `core/crates/aaf-wrapper/src/gateway.rs`
- **Edit**
  - `core/crates/aaf-memory/src/facade.rs` (add `thread_id_for_surface`)
  - `core/crates/aaf-policy/src/guard.rs` (action guard extension)
  - `core/crates/aaf-contracts/src/observation.rs` (proposal outcome variant)
  - `core/crates/aaf-runtime/src/executor.rs` (wire proposal outcome into trace)
  - `core/crates/aaf-saga/src/executor.rs` (accept/reject/transform → saga)

### Unit tests expected

- `aaf-sidecar::gateway::tests` — at least 4:
  - `event_becomes_intent_via_adapter`
  - `idempotent_event_deduplicated`
  - `per_surface_rate_limit_enforced`
  - `per_tenant_budget_enforced`
- `aaf-memory::facade::tests::thread_id_for_surface_is_deterministic`
- `aaf-policy::guard::action::tests::mutation_proposal_scope_check`

### Integration test

- `core/tests/integration/tests/e3_slice_b_smoke.rs`:
  - Build an `EventGateway` wired to a sidecar.
  - Emit an `AppEvent` with a situation that references an
    `Order` entity ref.
  - Assert the resulting `IntentEnvelope` carries
    `entities_in_context = [commerce.Order]`.
  - Run through the planner + executor.
  - Receive an `ActionProposal` with a `StateMutationProposal`.
  - Accept the proposal.
  - Assert a saga step runs, records an observation with
    `outcome_detail.proposal_outcome = Accepted`.
  - Re-run the same `AppEvent` (idempotency key unchanged).
  - Assert the gateway deduplicates and returns the same
    `trace_id` without re-executing.

### Rules preserved

| Rule | How |
|---|---|
| R19 Projections default-deny | Unchanged — still `StateProjection::allows_field`. |
| R20 Proposals, not mutations | Unchanged — enforced at construction in `ActionProposal::new_with_mutations`. Slice B just wires the runtime consequences. |
| R21 Tenant-scoped entities | `thread_id_for_surface` carries `tenant_id` in its derivation so threads are partitioned per tenant. |
| R12 Trace everything | Every proposal lifecycle transition emits an observation. |

### Success criteria

- `EventGateway` tests demonstrate idempotency, rate limit, and
  budget enforcement.
- Proposal accept/reject/transform/expire all produce
  observations whose `outcome_detail.proposal_outcome` is
  populated.
- Integration test exercises the full AppEvent → Intent →
  Plan → Proposal → Accept → Saga → Trace chain.
- `make ci` stays green.

### Deferred to E3 Slice C

- Python / TypeScript SDK primitives
  (`@on_event`, `@project`, `@accept_proposal`).
- `<AgentProposal/>` React component in `ui/front-door/`.
- `examples/app-native/` reference application.
- WebSocket proposal channel.

---

## ✓ Slice — X1 Slice C — CLI, SPDX/CycloneDX export, co-signed tokens (LANDED)

X1 Slice C delivered SBOM exporters (SPDX + CycloneDX), co-signed
capability tokens for federation, identity CLI expansion, the
`signed-agent` example, and ADR-017. Integration test:
`x1_slice_c_cli.rs`. Rules 22–24 preserved.

---

## Slice 3 — E3 Slice C — SDK primitives, reference app, WebSocket

### Motivation

E3 Slices A and B deliver the contracts and the runtime bridge.
What's missing is **the developer-facing surface**: SDK primitives
(`@on_event`, `@project`, `@accept_proposal`), a reference
application under `examples/app-native/`, and a WebSocket
proposal channel for real-time proposal delivery.

### Scope

| Area | What lands |
|---|---|
| **Python SDK primitives** | `@on_event`, `@project`, `@accept_proposal` decorators in the Python SDK. |
| **TypeScript SDK primitives** | Mirror of the Python decorators. |
| **`<AgentProposal/>` React component** | In `ui/front-door/` — renders `ActionProposal`s inline. |
| **Reference application** | `examples/app-native/` — user opens an Order page → `AppEvent` → `Intent` → planner → runtime → `ActionProposal` rendered inline → user accepts → saga executes. |
| **WebSocket proposal channel** | Real-time delivery of proposals from runtime to front door. |

### Files to touch

- **New**
  - `sdk/python/src/aaf/decorators.py` (surface decorators)
  - `sdk/typescript/src/decorators.ts` (surface decorators)
  - `ui/front-door/src/components/AgentProposal.tsx`
  - `examples/app-native/` (full reference app)
- **Edit**
  - `core/crates/aaf-server/src/api/ws.rs` (proposal channel)

### Success criteria

- Reference app runs end-to-end: event → intent → plan → proposal → accept → saga → outcome.
- Python and TypeScript decorators compile and have tests.
- `make ci` stays green.

### Deferred after E3 Slice C

- Go SDK primitives.
- Dashboard integration for proposal analytics.

---

## What to do at the end of each slice

1. Mark the slice `✓` in the status table at the top of this
   file **and** in `development/roadmap.md`.
2. Append a row to the iteration log in
   `development/iteration-playbook.md`.
3. Update the totals table in `IMPLEMENTATION_PLAN.md` (test
   count, crates, clippy, schemas, adoption).
4. Re-run the five gates (`cargo build / test / clippy /
   schema-validate / ontology-lint`) and record the totals.
5. Promote the next slice in this file to the top, and add a
   new section for the slice *after* that so there is always
   *three* slices queued.

---

## Out-of-order caveats

If you find yourself tempted to implement a later slice because
it is "more interesting" or "easier", stop and re-read
`development/iteration-playbook.md` → "Slice ordering". Every
enhancement lands A → B → C in order; within the two waves,
E2 → E1 → E3 and X1 → X2 → X3. Leapfrogging forces a rewrite
when the prerequisite Slice B lands, and that rewrite is
harder than doing the slices in order would have been.

The only exception is **fix-forward** iterations — if the tree
is broken, fix the tree before adding any new slice. Iteration
5 was exactly this (fix-forward + quality pass) and is the
template to follow.

---

## Wave 4 Slices (after E1 C / E3 B / E3 C)

> See `PROJECT.md` §20, `CLAUDE.md` rules 34–38, and
> `development/roadmap.md` for the full design.

### F2 Slice A — Anthropic Provider + ProviderMetrics

**Scope:**
- `ProviderMetrics` struct on `ChatResponse` (Rule 35)
- `AnthropicProvider` with `from_env()` + `chat()` — Messages API
  format mapping, rate limit handling, real token counting
- `ModelProfile` + `default_model_catalog()` pricing table
- Update `MockProvider` to return `ProviderMetrics`
- Guarded live test behind `AAF_LIVE_LLM_TEST=1` env var

**Files to touch:**
- Edit: `core/crates/aaf-llm/src/provider.rs`, `Cargo.toml`
- New: `core/crates/aaf-llm/src/anthropic.rs`, `pricing.rs`,
  `core/tests/integration/tests/f2_llm_provider_smoke.rs`

### F2 Slice B — Router + Multi-Provider + Fallback

**Scope:**
- `OpenAiProvider` + `LocalProvider`
- `ValueRouter` with scoring algorithm + `RoutingConstraints`
- Health tracking (latency moving average, failure count)
- Auto-fallback on provider failure
- Wire router into `aaf-runtime` executor

### F2 Slice C — Streaming + Budget Pre-Check

**Scope:**
- `chat_stream()` with SSE parsing (Anthropic + OpenAI)
- Budget pre-check: estimate cost, reject if over budget
- Provider configuration from `aaf-server` config file
- Classification-aware provider filtering

### F1 Slice A — Python SDK Core

**Scope:**
- `scripts/codegen/generate.py` — JSON Schema → pydantic v2
- `sdk/python/` — `@capability`, `@guard`, `@compensation`
  decorators, `AafClient`, `MockRuntime`
- Generated pydantic models for all 18+ JSON Schemas
- `Makefile` — `codegen` target

### F1 Slice B — TypeScript SDK + CLI

**Scope:**
- `scripts/codegen/typescript_generator.py` — JSON Schema → zod
- `sdk/typescript/` — builders, client, streaming, testing
- `aaf` CLI commands: init, dev, test, run, trace

### F1 Slice C — Go SDK + Advanced Builders

**Scope:**
- `sdk/go/` — client, sidecar builder, wrapper builder
- End-to-end example: Python agent → sidecar → runtime → trace
- SDK getting-started guides in `docs/`

### F3 Slice A — MCP Client

**Scope:**
- `adapters/mcp/` Rust crate — stdio transport, `McpClient`,
  tool discovery → capability registration, governed invocation
- `spec/schemas/mcp-server-config.schema.json`

### F3 Slice B — MCP Server + Remote Transports

**Scope:**
- SSE + streamable HTTP transports
- `McpServer` exposing AAF capabilities as MCP tools
- Wire MCP server into `aaf-server` (config-gated)

### F3 Slice C — A2A Participant + ProtocolBridge

**Scope:**
- `adapters/a2a/` Rust crate — Agent Card, task lifecycle,
  DID-based trust propagation, SSE streaming
- `ProtocolBridge` unifier (local + MCP + A2A)
- Wire A2A into `aaf-server` (config-gated)
