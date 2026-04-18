# Architecture

> The 10-minute overview. For the complete design rationale read
> `PROJECT.md`; for the non-negotiable rules read `CLAUDE.md`.

---

## What AAF is

AAF is a **semantic orchestration layer** that sits on top of
existing service architectures (microservices, modular monoliths,
cell architectures). AI agents are the universal interface between
humans, applications, services, and APIs. AAF does not replace
existing services — it translates intents into typed execution
plans, discovers capabilities, enforces policies, manages trust,
and records every decision.

> **Core equation:** natural language at the edges, typed protocols
> internally, deterministic logic preserved.

---

## The flow of a single intent

```
1. App / user emits an AppEvent or a goal string
2. Intent compiler: classify → extract → enrich → refine → cache
   → IntentEnvelope
3. Planner: fast-path? agent-assisted? full-agentic? choreography?
   → ExecutionPlan
4. Runtime: per step, PreStep policy → node → PostStep policy
   → Observation → trace
5. On failure: compensation chain walks in reverse
6. Outcome attached to the final observation; feedback spine picks
   it up
```

Every step is typed. Natural language exists only (a) at the
front door, (b) inside LLM prompts within agent nodes, and (c) in
the human-review paths for proposals and approvals. Nothing
in-between is a free-form string.

---

## The 22 crates at a glance

Organised in three tiers by dependency layer.

### Tier 1 — Pure contract / storage

- `aaf-contracts` — the typed surface; every cross-crate message.
- `aaf-storage` — storage traits + in-memory backends (Rule 11).
- `aaf-ontology` — the noun layer: Entity, Classification,
  Relation, registry, resolver, lineage (E2).
- `aaf-transport` — transport abstraction (real drivers deferred).

### Tier 2 — Domain modules

- `aaf-trace` — Observation recorder, OTLP export, cost attribution
  (Rule 12).
- `aaf-trust` — trust score, autonomy, min-delegation, DID-bound
  signing.
- `aaf-memory` — four-layer memory + context budget manager.
- `aaf-policy` — engine, 7 rules, 3 guards, approval workflow
  (Rules 6, 7).
- `aaf-registry` — CRUD, discovery (lexical + entity-aware),
  health, degradation state machine, attestation gate.
- `aaf-intent` — NL → IntentEnvelope pipeline.
- `aaf-llm` — LLMProvider trait + mock + value router + per-call
  budget + LearnedRoutingPolicy (E1 Slice B).
- `aaf-identity` — DID, keystore, signed manifest, SBOM,
  capability token, revocation registry (X1 complete).
- `aaf-eval` — feedback spine: Judge, GoldenSuite, Replayer,
  RegressionReport (E1 Slice A).
- `aaf-learn` — online learning: FastPathMiner, CapabilityScorer,
  RouterTuner, EscalationTuner (E1 Slice B).
- `aaf-surface` — app-native surface: AppEvent, Situation,
  ActionProposal, StateMutationProposal, StateProjection (E3
  Slice A).

### Tier 3 — Composition

- `aaf-runtime` — graph executor, 5 node types, checkpoint,
  compensation, budget, scheduler, revocation gate.
- `aaf-saga` — saga engine with intelligent recovery.
- `aaf-planner` — router (4 patterns), fast path, bounded
  autonomy, composition safety (entity-aware).
- `aaf-sidecar` — microservice sidecar + transparent fallback.
- `aaf-wrapper` — modular monolith wrapper.
- `aaf-federation` — cell config + cross-cell router with
  entity-space agreements.
- `aaf-server` — reference binary + CLI (`run`, `validate`,
  `discover`, `compile`, `ontology lint`, `ontology import`,
  `identity generate|verify|export-sbom`).

For details see
[../development/crate-reference.md](../development/crate-reference.md).

---

## The four communication patterns

Every request is classified into one of four patterns:

| Pattern | Latency p99 | When |
|---|---|---|
| **Fast Path** | < 50 ms | Fully structured + unambiguous target |
| **Agent Assisted** | < 500 ms | Single service, minor ambiguity |
| **Full Agentic** | < 15 s | Multi-service coordination needed |
| **Agentic Choreography** | minutes–hours | Long-running, async |

Target: **> 60% of traffic on the Fast Path.** Rule 4 says: before
adding agent intelligence to a path, check whether Fast Path
works. Fast-path rules are evaluated *locally in the sidecar* with
no round trip to the control plane. See [fast-path.md](fast-path.md).

---

## Rules that are enforced in code

Every rule in `CLAUDE.md` has a concrete enforcement point. The
common ones:

| Rule | Enforcement point |
|---|---|
| 5 Deterministic core is sacred | `aaf-runtime::node::DeterministicNode` refuses LLM use |
| 6 Policy at every step | `aaf-runtime::executor` calls `PolicyEngine::evaluate` at 4 hooks |
| 7 Guard every agent | `aaf-policy::guard::{InputGuard, OutputGuard, ActionGuard}` |
| 8 Depth + budget limits | `aaf-runtime::budget::BudgetTracker` + `IntentEnvelope::delegate` |
| 9 Compensation before implementation | `CapabilityContract::validate` + `Registry::register` |
| 11 Storage behind traits | All persistence through `aaf-storage` traits |
| 12 Trace everything | `aaf-trace::Recorder`; runtime emits Observations per step |
| 13 Sidecar transparent fallback | `aaf-sidecar::proxy::forward_direct` |
| 14 Semantics are nouns | `CapabilityContract.{reads,writes,emits}` + `BoundaryEnforcement` + `EntityAwareComposition` |
| 20 Proposals, not mutations | `ActionProposal::new_with_mutations` enforces at construction |
| 22 Identity is cryptographic | `aaf-identity::AgentDid` |

See [security.md](security.md) for the full rule-to-code map.

---

## Entity-space boundaries (E2)

Since iteration 8, AAF expresses **cross-boundary enforcement in
entity space**, not in field-name space:

- **Capabilities declare** what entities they read, write, and emit.
- **The planner's composition checker** runs three detectors:
  double-write, classification-leak, cross-tenant fan-out.
- **The policy boundary rule** consults an ontology classification
  lookup.
- **The memory long-term store** has an entity inverted index.
- **The registry** answers "who writes commerce.Order?"
- **Federation agreements** are written as `EntityAccessRule`s
  (entity + op + classification cap + optional tenant).

See [ADR-008 — Entity-space boundaries](adr/ADR-008-entity-space-boundaries.md)
for the full rationale.

---

## The five gates

Every change must keep these five green:

```bash
cargo build --workspace                                       # gate 1
cargo test --workspace                                        # gate 2
cargo clippy --workspace --all-targets -- -W clippy::all     # gate 3
make schema-validate                                          # gate 4
make ontology-lint                                            # gate 5
```

Or all at once: `make ci`.

Current status: **554 tests passing, 0 failures,
0 build warnings, 0 clippy warnings, 9/9 examples validating,
100% ontology adoption in strict mode, 0 lint errors.**

---

## Wave 4 — Critical infrastructure (planned)

Three prerequisites for framework viability are designed and
ready for implementation (see `PROJECT.md` §20):

| Enhancement | What it adds |
|---|---|
| **F2** LLM Integration | Real providers (Anthropic, OpenAI, local), value-based routing, ProviderMetrics |
| **F1** Developer Experience | Python/TypeScript/Go SDKs, CLI, code generation |
| **F3** Protocol Bridges | MCP client/server, A2A participant, governed external calls |

New rules: R34 (SDKs generated), R35 (providers observable),
R36 (bridges governed), R37 (SDK ergonomics), R38 (bridge
failures graceful).

---

## Where to go next

- [contracts.md](contracts.md) — the typed surface
- [policies.md](policies.md) — the policy engine in detail
- [security.md](security.md) — the security model
- [integration-microservices.md](integration-microservices.md) —
  microservice sidecar integration
- [integration-modular-monolith.md](integration-modular-monolith.md)
  — modular monolith wrapper integration
- [integration-cell-architecture.md](integration-cell-architecture.md)
  — cell architecture federation
- [enhancements/f2-llm-integration.md](enhancements/f2-llm-integration.md) —
  live LLM integration design
- [enhancements/f1-developer-experience.md](enhancements/f1-developer-experience.md) —
  SDK and CLI design
- [enhancements/f3-protocol-bridges.md](enhancements/f3-protocol-bridges.md) —
  MCP + A2A bridge design
