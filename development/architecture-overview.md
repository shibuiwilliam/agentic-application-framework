# Architecture Overview

> The 5-minute version of what the code actually does. For the full
> design story read `PROJECT.md`; for the rules the code must
> conform to read `CLAUDE.md`.

---

## Mental model

AAF sits **above** existing services (microservices, modular
monoliths, cell architectures) and adds a semantic orchestration
layer. Natural language enters at the edges, gets compiled into a
typed `IntentEnvelope`, is planned against the capability registry,
executes through a graph runtime with policy hooks at every step,
and records everything to a trace.

```
┌──────────────────────────────────────────────────────────────┐
│                        USER / APP                            │
│   (NL, click, cron, webhook — anything that produces an      │
│    AppEvent or a goal string)                                │
└─────────────────────┬────────────────────────────────────────┘
                      │   AppEvent | goal string
                      ▼
┌──────────────────────────────────────────────────────────────┐
│                  aaf-intent (Intent Compiler)                │
│   classify → extract → enrich (with ontology) → refine       │
│                                                              │
│   out: IntentEnvelope                                        │
└─────────────────────┬────────────────────────────────────────┘
                      │
                      ▼
┌──────────────────────────────────────────────────────────────┐
│               aaf-planner (Planner / Router)                 │
│   1. fast-path / agent-assisted / full-agentic / choreography│
│   2. registry discovery (lexical + by-entity)                │
│   3. bounds check (depth / steps / cost / latency)           │
│   4. composition safety (base + entity-aware:                │
│      double-write, classification-leak, cross-tenant)        │
│                                                              │
│   out: ExecutionPlan                                         │
└─────────────────────┬────────────────────────────────────────┘
                      │
                      ▼
┌──────────────────────────────────────────────────────────────┐
│                   aaf-runtime (Graph Runtime)                │
│                                                              │
│   executor loops over steps:                                 │
│     ─ pre-plan hook  → PolicyEngine.evaluate                 │
│     ─ pre-step hook  → PolicyEngine.evaluate                 │
│     ─ run node       → Deterministic / Agent / Approval /    │
│                        Fork / EventWait                      │
│     ─ post-step hook → PolicyEngine.evaluate                 │
│     ─ pre-artifact   → PolicyEngine.evaluate                 │
│     ─ record Observation                                     │
│     ─ budget / depth bookkeeping                             │
│                                                              │
│   on failure: compensation chain walks in reverse            │
└───┬──────────────────────────┬───────────────────────────────┘
    │                          │
    ▼                          ▼
┌───────────────┐         ┌────────────────────────┐
│  aaf-policy   │         │   aaf-registry         │
│ 7 rules, 3    │         │  CRUD + lexical        │
│ guards, 4     │         │  discovery + entity-   │
│ hooks         │         │  aware discovery       │
└───────────────┘         └────────────────────────┘
```

Every decision, every step, every observation flows into
`aaf-trace`. Traces feed `aaf-eval` (E1: feedback spine) and,
eventually, `aaf-learn` (deferred). The ontology (`aaf-ontology`) is
the *noun layer* that every other crate keys off to reason in
entity-space instead of string-space (E2).

---

## The 21 crates in three tiers

### Tier 1 — Pure contract / storage (sync, no orchestration)

| Crate | One-liner |
|---|---|
| `aaf-contracts` | Every typed message: `IntentEnvelope`, `CapabilityContract`, `Task`, `Artifact`, `Handoff`, `Observation`, `Trust`, `Policy`, `Trace`, `EntityRefLite`, `EventRefLite`, `AgentDidRef`, `AttestationLevelRef`, `CapabilityTokenLite`. |
| `aaf-storage` | `*Store` traits + in-memory backends. Rule 11 says no other crate may import a database driver. |
| `aaf-ontology` | `Entity`, `Classification`, `Relation`, `OntologyRegistry`, `EntityResolver`, `Lineage`, JSON-Schema `import`. |
| `aaf-transport` | Transport abstraction trait (real drivers deferred). |

### Tier 2 — Domain modules (async, orchestration-facing)

| Crate | One-liner |
|---|---|
| `aaf-trace` | `Recorder` that takes observations, cost attribution, OTLP export (E1 Slice A). |
| `aaf-trust` | Numeric trust score, 5-level autonomy, `min(a,b)` delegation, DID-bound artifact signing (X1 Slice B). |
| `aaf-memory` | 4-layer memory (working / thread / longterm / artifact) + `longterm_search_by_entity` (E2 Slice B) + context budget manager. |
| `aaf-policy` | `PolicyEngine` + 7 rules (scope, side_effect, budget, pii, injection, composition, boundary) + 3 guards (input, output, action) + approval workflow. Boundary rule consults the ontology (E2 Slice B). |
| `aaf-registry` | CRUD + lexical discovery + `discover_by_entity` (E2 Slice B) + health + degradation state machine + `get_for_attestation` gate (X1 Slice B). |
| `aaf-intent` | `IntentCompiler` pipeline: classifier → extractor → enricher (with ontology resolver, E2 Slice B) → refinement → cache → versioning. |
| `aaf-llm` | `LLMProvider` trait, deterministic mock, value-based router, per-call budget enforcement. |
| `aaf-identity` | X1 Slice A+B: DID, Keystore/Signer/Verifier, `AgentManifest`, `AgentSbom`, `Attestation`, `CapabilityToken`, `RevocationRegistry`. |
| `aaf-eval` | E1 Slice A: `Judge` trait + `DeterministicJudge`, `GoldenSuite`, `Replayer`, `RegressionReport`. |
| `aaf-surface` | E3 Slice A: `AppEvent`, `Situation`, `EventToIntentAdapter`, `ActionProposal`, `StateMutationProposal`, `StateProjection`, `ProposalLifecycle`, `SituationPackager`. Rules 19 and 20 enforced at construction. |

### Tier 3 — Composition (everything below wires into these)

| Crate | One-liner |
|---|---|
| `aaf-runtime` | `GraphBuilder` / `GraphExecutor`, 5 node types, checkpoint, compensation chain, budget tracker, scheduler, revocation gate (X1 Slice B). |
| `aaf-saga` | Agentic saga engine: definition parser, executor, intelligent recovery state machine, bridge to registry/runtime. |
| `aaf-planner` | Pattern router (fast path / agent-assisted / full agentic / choreography), fast-path rules, planner, bounded autonomy, composition checker, plan cache. Entity-aware composition added in E2 Slice B. |
| `aaf-sidecar` | Transparent proxy + capability publisher + local fast path + guards + transparent fallback (Rule 13). |
| `aaf-wrapper` | In-process wrapper for modular monoliths. |
| `aaf-federation` | Cell config, cross-cell router, federation agreements in **entity space** (E2 Slice C: `EntityAccessRule`, classification cap, tenant restriction). |
| `aaf-server` | Main binary. Subcommands: `run`, `validate`, `discover`, `compile`, `ontology lint`, `ontology import`, `help`. Wires everything together. |

---

## Dependency graph

Dependencies are strictly acyclic. A new crate should slot in at the
lowest tier where it can satisfy its needs; avoid adding an
upward-only edge.

```
                    aaf-contracts
                    ├─────────────┐─────────────┬──────────┐
                    ▼             ▼             ▼          ▼
              aaf-ontology  aaf-storage  aaf-identity aaf-transport
                               │            │
                               ├────────────┼─────────────┐──────────┐
                               ▼            ▼             ▼          ▼
                           aaf-trace    aaf-trust    aaf-memory   aaf-llm
                                           │            │
                                           │            │
                                           ▼            ▼
                                      aaf-policy   aaf-eval     aaf-surface
                                           │
                                           ▼
                                      aaf-registry
                                           │
                                           ▼
                                      aaf-intent
                                           │
                                           ▼
                                      aaf-runtime ◄──── aaf-identity
                                           │
                        ┌──────────────────┼──────────────────┐
                        ▼                  ▼                  ▼
                    aaf-saga          aaf-planner         aaf-wrapper
                                           │                  │
                        ┌──────────────────┘                  │
                        ▼                                     │
                    aaf-sidecar                               │
                        │                                     │
                        └──────────────────┬──────────────────┘
                                           ▼
                                      aaf-federation
                                           │
                                           ▼
                                      aaf-server
```

**Rule of thumb:** if your change introduces a new dep from a tier-1
or tier-2 crate onto a tier-3 crate, you have gotten the layering
backwards. Invert the dependency — usually by defining a trait in the
lower tier and implementing it in the higher one.

---

## The hot path, annotated

Here is what runs on every single intent, from `aaf-server`'s
`cmd_run`:

1. **Compile.** `aaf_intent::IntentCompiler::compile(goal, requester, domain, budget)`
   - `Classifier` → `IntentType`
   - `Extractor` → constraints from NL
   - `Enricher::enrich_with_ontology` → `entities_in_context`
     populated from an `OntologyResolver` (E2 Slice B)
   - `Refiner` → clarification questions (or `CompileOutcome::Compiled(env)`)
2. **Plan.** `aaf_planner::RegistryPlanner::plan(&intent)`
   - Cache lookup on `(intent_type, domain, goal)`
   - `Registry::discover` → lexical-ranked capability list
   - `Registry::discover_by_entity` → entity-keyed alternative
   - Topological sort over `depends_on` edges
   - `CompositionChecker` (base) + `EntityAwareComposition` (Slice B)
   - `BoundedAutonomy::validate` → steps / depth / cost / latency caps
   - cache + return
3. **Build graph.** `aaf_runtime::GraphBuilder` materialises each
   planned step as a `DeterministicNode` or `AgentNode` and wires
   compensation chains.
4. **Execute.** `aaf_runtime::GraphExecutor::run(&graph, &intent)`
   - **PrePlan** hook → `PolicyEngine::evaluate`
   - optional **revocation gate** (X1 Slice B): if an
     `Arc<dyn RevocationRegistry>` is attached and the requester
     presents a revoked DID, the executor fails with
     `RuntimeError::Revoked` *before* the trace opens
   - for each node in topological order:
     - **PreStep** hook → `PolicyEngine::evaluate`
     - run the node (Deterministic / Agent / Approval / Fork /
       EventWait), enforcing budget
     - **PostStep** hook → `PolicyEngine::evaluate`
     - **PreArtifact** hook (when the node produced an artifact)
     - record an `Observation` through `aaf_trace::TraceRecorder`
     - on failure: walk the compensation chain in reverse
5. **Trace close.** `Recorder::close_trace(trace_id, status)`;
   `Outcome` is attached to the final observation and optionally
   forwarded to `aaf-eval`.

Every step from (1) to (5) is typed. Natural language exists only
inside the classifier / extractor / agent-node LLM prompts —
nowhere else.

---

## Where each architecture rule lives

| Rule | Enforcement point |
|---|---|
| R1 Agents translate, services decide | `aaf-runtime::node::agent::AgentNode` vs `DeterministicNode` |
| R2 Typed internals | Every cross-crate message is in `aaf-contracts` |
| R3 Services stay untouched | `aaf-sidecar` + `aaf-wrapper` are the only crates that touch target services |
| R4 Fast path first | `aaf-planner::router::Router::classify` + `aaf-planner::fast_path::FastPathRuleSet` |
| R5 Deterministic core is sacred | `aaf-runtime::node::DeterministicNode` refuses LLM use |
| R6 Policy at every step | `aaf-runtime::executor` calls `PolicyEngine::evaluate` at 4 hooks |
| R7 Guard every agent | `aaf-policy::guard::{InputGuard, OutputGuard, ActionGuard}` wrapping every `AgentNode` |
| R8 Depth + budget limits | `aaf-runtime::budget::BudgetTracker` + `IntentEnvelope::delegate()` |
| R9 Compensation before implementation | `aaf-contracts::CapabilityContract::validate` + `aaf-registry::store::Registry::register` |
| R10 Context minimisation | `aaf-memory::context::ContextBudget` (~7,500 tokens) |
| R11 Storage behind traits | No crate imports a DB driver; everything behind `aaf-storage` traits |
| R12 Trace everything | `aaf-trace::Recorder`; every node run emits an Observation |
| R13 Sidecar transparent fallback | `aaf-sidecar::proxy::Proxy::forward_direct` |
| R14 Semantics are nouns, not names (E2) | `CapabilityContract.reads/writes/emits`, `BoundaryEnforcement`, `EntityAwareComposition` |
| R15 Feedback is a contract (E1) | `Observation.outcome_detail: Option<Outcome>` |
| R19 Projections default-deny (E3) | `StateProjection::allows_field` |
| R20 Proposals, not mutations (E3) | `ActionProposal::new_with_mutations` enforced at construction |
| R21 Entities are tenant-scoped (E2) | `EntityRefLite.tenant: Option<TenantId>`; boundary rule; federation router |
| R22 Identity is cryptographic (X1) | `aaf-identity::AgentDid` (public-key thumbprint only) |
| R23 Signed manifest (X1) | `AgentManifest::build` signs at construction time |
| R24 Provenance as BOM (X1) | `AgentSbom` with content hashes |

---

## The three integration patterns

Per `PROJECT.md` §2.3, AAF drops into existing architectures in
three shapes. The relevant crates:

| Pattern | Crate | Status |
|---|---|---|
| A. Microservices | `aaf-sidecar` | foundational; transparent fallback implemented |
| B. Modular monolith | `aaf-wrapper` | foundational; in-process wrapping implemented |
| C. Cell architecture | `aaf-federation` | entity-space rules implemented (Slice C); cross-cell tokens deferred to X1 Slice C |
