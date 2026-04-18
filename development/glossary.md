# Glossary

> Terminology used throughout the AAF codebase, in
> alphabetical order. Covers AAF-specific concepts, acronyms,
> and imported terms whose local meaning differs from their
> wider meaning.

---

## A

**A2A (Agent-to-Agent)**
A transport pattern for cross-organisation agent communication.
Imported as a JSON shape via `aaf-registry::a2a::AgentCard`.
A2A Agent Cards are always treated as untrusted input — they
are imported with safe defaults (read classification, no
compensation) and require explicit policy approval before
their writes can land.

**Action Guard**
One of three guards per agent node (Rule 7). Applied to a
proposed action before it runs. Enforces scope and
side-effect gates. Lives in
`aaf-policy::guard::ActionGuard`.

**AAF**
*Agentic Application Framework.* This codebase. A semantic
orchestration layer that adds AI agents to existing service
architectures without replacing them.

**ActionProposal**
E3 Slice A contract. An agent's proposal for something the app
should do — contains a summary, rationale, optional
`StateMutationProposal`s, artifacts, and UI hints. Enforces
**Rule 20** at construction: proposals with mutations require a
`compensation_ref`. Lives in `aaf-surface::proposal`.

**Adoption Ratio**
Used by the ontology lint. The fraction of scanned capability
YAMLs that carry at least one entity declaration
(`reads` / `writes` / `emits`). At `≥ 0.90` the lint flips
from warn-only to strict mode.

**Agent Assisted**
Communication pattern ②. Single-service requests with minor
ambiguity, p99 < 500 ms, uses a small model. See
[fast-path.md](../docs/fast-path.md).

**Agent Node**
A node in the graph runtime whose behaviour is LLM-powered.
Wraps an `LLMProvider` handle and runs through three guards
(input / output / action). Lives in
`aaf-runtime::node::AgentNode`.

**Agentic Choreography**
Communication pattern ④. Long-running, async, minutes to hours.
Used for workflows that cannot be completed in a single
synchronous call. The agentic saga engine drives it.

**Agentic Saga**
AAF's saga engine extended with intelligent failure recovery.
Unlike traditional sagas which compensate everything in reverse,
an agentic saga analyses the failure cause and picks an optimal
recovery (partial compensation, retry, pause-and-ask, full
rollback). Lives in `aaf-saga`. See
[../docs/saga.md](../docs/saga.md).

**AgentDid**
A cryptographic Decentralized Identifier of an agent.
`did:aaf:<thumbprint>` format. Constructed only from a public
key (Rule 22). Lives in `aaf-identity::did::AgentDid`.

**AgentManifest**
X1 Slice A contract. Enumerates every input an agent is
permitted to use: model version, prompts, tool bindings,
ontology slices, capability allow-list, upstream providers.
**Signed at build time** (Rule 23). Lives in
`aaf-identity::manifest`.

**AgentSbom**
*Agent Software Bill of Materials.* X1 Slice A contract. Hashes
every input with content hashes. Exportable as SPDX or
CycloneDX JSON. Lives in `aaf-identity::sbom`.

**ApprovalNode**
A node kind that polls an `ApprovalWorkflow` and blocks until a
human flips its state from `Pending` to `Approved` or
`Rejected`. Lives in `aaf-runtime::node::ApprovalNode`.

**ApprovalWorkflow**
The service that tracks pending approvals. Lives in
`aaf-policy::approval`. Typically fronted by a human-facing UI
(the Front Door, once it ships).

**Artifact**
Any produced output that carries provenance and a signature.
Fields: `id`, `kind`, `content`, `producing_agent`, `trace_id`,
`signature`, `derived_from`. Signed via
`aaf-trust::sign_artifact_with` (X1 Slice B, DID-bound) or the
legacy `sign_artifact` (iteration 1).

**Attestation**
X1 Slice A. An assertion about an agent's identity (e.g. "this
agent is at `Assurance::High`"). Carries an `AttestationLevel`
(0-4). A capability may declare
`required_attestation_level`; the registry refuses to serve it
to callers who do not meet the level.

**Autonomy Level**
Five discrete levels from the trust system: `Observer`,
`Assistant`, `Collaborator`, `Operator`, `Custodian`.
Delegation uses `min(delegator, delegatee)`. Lives in
`aaf-trust::autonomy`.

---

## B

**Backend (storage)**
A concrete implementation of one of the storage traits
(`CheckpointStore`, `WorkingMemoryStore`, etc.). Every trait has
an `InMemory*` backend; real backends (PostgreSQL, Redis, S3,
ClickHouse, pgvector) are deferred.

**Budget Tracker**
Rule 8 enforcement. Tracks tokens / cost / latency per intent;
on exhaustion returns a graceful partial result. Lives in
`aaf-runtime::budget::BudgetTracker`.

---

## C

**CapabilityContract**
The typed declaration of an executable capability. Fields
include `id`, `side_effect`, `compensation`, `reads`, `writes`,
`emits`, and many others. Validated at registration time by
`Registry::register`.

**CapabilityToken**
X1 Slice A. A signed bearer grant that binds caller, callee,
capabilities, remaining depth, and expiry. Replaces a bare
`min(a, b)` trust comparison with cryptographic verification.
Lives in `aaf-identity::delegation::CapabilityToken`.

**Cell**
A deployment unit in Pattern C (cell architecture). Each cell
has its own AAF runtime, registry, memory, and trace store.
Cross-cell communication goes through
`aaf-federation::Router`.

**Classification (Entity)**
A level on the data-handling lattice:
`Public ⊂ Internal ⊂ Pii ⊂ Regulated(_)`. Attached to every
entity in the ontology. Enforced by the boundary rule through
an `OntologyClassificationLookup` callback.

**ClassificationCap**
A federation agreement can cap a destination cell's access to a
specific entity at a given `ClassificationCap` level
(`Public` / `Internal` / `Confidential` / `Restricted`).
Exceeding the cap is a `FederationError::ClassificationCapExceeded`.

**Compensation**
An undo operation. Every write-class capability must declare
one (Rule 9). The runtime's `CompensationChain` drains
registered compensators in reverse order on node failure.

**Composition Safety**
A planner concern (Rule 4 / Rule 14). The base
`CompositionChecker` caps the number of write-class steps; the
entity-aware `EntityAwareComposition` (E2 Slice B) adds
double-write, classification-leak, and cross-tenant fan-out
detectors.

**Context Budget**
~7,500 tokens per LLM call (Rule 10). Broken down into
system (~2,000) + intent (~500) + memory (~2,000) + step
(~1,000) + tools (~2,000). Managed by
`aaf-memory::context::ContextBudget`.

**CoSignedToken**
Federation contract (added by linter in iteration 8). A
capability token signed by both the issuing cell and the
receiving cell. Prevents single-cell compromise from
authorising cross-cell access. Lives in
`aaf-federation::cosign`.

---

## D

**Data Classification**
A tag on a capability (`Public` / `Internal` / `Confidential` /
`Restricted`). Coarser than the entity classification lattice
but checked by the same rules.

**Degradation Chain**
Five levels of graceful degradation: Full Agentic → Cached →
Rule-Based → Fast Path Only → Bypass. Configured per deployment.
Capability-level degradation uses the same four levels
(`Full` / `Partial` / `Cached` / `Unavailable`).

**DeterministicNode**
A node whose logic is a pure function / tool call. **Rule 5
forbids LLM use here.** Lives in
`aaf-runtime::node::DeterministicNode`.

**DID**
*Decentralized Identifier.* `did:aaf:<thumbprint>` format. An
`AgentDid` is the public-key thumbprint of an agent's keystore
entry. See `aaf-identity::did`.

---

## E

**E1 / E2 / E3**
Wave-1 enhancements (see `PROJECT.md` §16):
- **E1 — Feedback Spine** (Outcome contract, `aaf-eval`,
  `aaf-learn` landed in Slice B)
- **E2 — Domain Ontology Layer** (`aaf-ontology`, entity-aware
  planner/policy/memory/federation)
- **E3 — Application-Native Surface** (`aaf-surface`,
  AppEvent, ActionProposal, StateProjection)

**Edge**
A directed connection between two `Node`s in a `Graph`. Edges
together with nodes define the topological order for execution.

**Entity**
A first-class noun in the domain ontology. Has an id (dot-
qualified like `commerce.Order`), a version, a classification,
a set of fields, and relations. Lives in
`aaf-ontology::entity::Entity`.

**EntityAccessRule**
An entry in a `FederationAgreement` declaring which op (read /
write / emit) on which entity is permitted, optionally capped by
classification and optionally restricted to a tenant. Lives in
`aaf-federation::EntityAccessRule`.

**EntityRefLite**
The wire-format shape of an entity reference (`entity_id`,
`version`, `tenant?`, `local_id?`). Lives in `aaf-contracts`
so every crate can reference an entity without depending on
`aaf-ontology` directly.

**EventGateway**
E3 Slice B (deferred). Will accept `AppEvent`s from a host
application and forward them to the intent compiler, with
idempotency, rate limiting, and per-tenant budgets.

---

## F

**Fast Path**
Communication pattern ①. Fully structured, unambiguous target,
p99 < 50 ms, no LLM. Rules are evaluated locally in the
sidecar. Target: > 60% of production traffic. See
[../docs/fast-path.md](../docs/fast-path.md).

**FederationAgreement**
A bilateral or multilateral contract between cells. Declares
the parties, the shared capabilities, and the entity-space
rules. Lives in `aaf-federation::FederationAgreement`.

**ForkNode**
A node kind that runs children in parallel via `tokio::spawn`
and merges their outputs. Lives in
`aaf-runtime::node::ForkNode`.

**Front Door**
The human-facing chat / approval UI. Deferred to post-Wave-2.
Present as a directory only — `ui/front-door/`.

**Full Agentic**
Communication pattern ③. Multi-service coordination, p99 < 15 s,
uses the full plan-and-execute graph.

---

## G

**Graph**
A validated DAG of `Node`s. Produced by `GraphBuilder::build`.
Holds nodes, edges, a topologically sorted execution order, and
a compensator map. Lives in `aaf-runtime::graph::Graph`.

**GraphExecutor**
The central runtime loop that walks a `Graph` and enforces the
four policy hooks, the revocation gate, the budget tracker,
and the compensation chain. Lives in
`aaf-runtime::executor::GraphExecutor`.

---

## H

**Handoff**
Delegation of work from one agent (or human) to another. Fields
include the parent intent id, the delegator, the delegatee, the
depth after handoff, the reason, and (X1 Slice B) an optional
`capability_token`.

**Hook Point**
One of four places the runtime calls the policy engine:
`PrePlan`, `PreStep`, `PostStep`, `PreArtifact`. Plus the X1
Slice B **revocation gate** which runs *before* `PrePlan`.

---

## I

**InMemoryKeystore**
X1 Slice A `Keystore` impl using a deterministic HMAC-SHA256
backend. Ships as the default because Ed25519 has historically
broken the Rust 1.70 build. Ed25519 backend is an X1 Slice C
deliverable.

**Intent**
A user goal compiled into a typed `IntentEnvelope`. The unit of
execution in AAF.

**IntentEnvelope**
The typed representation of an intent. Carries the goal,
requester, domain, constraints, budget, risk tier, trace id,
depth, `entities_in_context` (E2), `output_contract`, and
`approval_policy`.

---

## J

**Judge**
A trait in `aaf-eval` that scores an observation's
output against an expected outcome. Ships with
`DeterministicJudge` (Jaccard-based). Future Slice C will
add an `LLMJudge`.

---

## L

**LearnedRule**
E1 Slice B. A fast-path rule proposed by the `fast_path_miner`
in `aaf-learn`. Carries source, evidence, and approval state;
must pass the approval workflow before going live. Lives in
`aaf-learn::fast_path_miner`.

**LineageRecord**
Ontology contract (E2). Tracks which capability wrote which
entity version from which inputs. Queryable for audit.

**LLMProvider**
A trait with `fn name() -> &str` and `async fn chat(req) ->
Result<ChatResponse>`. The abstraction behind which real
providers (Anthropic, OpenAI, etc.) are plugged in.

---

## M

**Manifest** → see `AgentManifest`.

**Memory Facade**
`aaf-memory::MemoryFacade` — the single entry point that
aggregates working, thread, long-term, and artifact memory
backends. Entity-keyed retrieval added in E2 Slice B.

**Minimal Outcome**
`Outcome::minimal(status, latency, tokens, cost)` — the
constructor used by the runtime at step-end when no richer
outcome data is available. Lives in
`aaf-contracts::Outcome::minimal`.

---

## N

**Node**
An executable unit in the graph runtime. Five kinds:
`Deterministic`, `Agent`, `Approval`, `Fork`, `EventWait`. Each
implements the `Node` trait.

**NodeOutput**
What a `Node::run` returns: structured data, tokens, cost,
duration, and optional model id.

---

## O

**Observation**
The canonical record of a single runtime step. Populated by
the executor after every node run. Carries the reasoning,
decision, confidence, alternatives, step outcome, and the
structured `outcome_detail` (E1 Slice A).

**OntologyRegistry**
The CRUD facade over declared entities. Enforces classification
downgrade prevention and breaking-version gates. Lives in
`aaf-ontology::registry::OntologyRegistry`.

**OTLP**
*OpenTelemetry Protocol.* The export format for traces. AAF
ships a hand-rolled JSON exporter
(`aaf-trace::export::otel_json_for`) to avoid the heavy OTel
SDK dep tree.

**Outcome**
E1 Slice A. Rich outcome record attached to an `Observation` at
step-end. Fields: status, latency, tokens, cost, policy
violations, user feedback, downstream error, semantic score.
The **single canonical location** for outcome data (Rule 15).

---

## P

**PlannerError**
Errors returned by `RegistryPlanner::plan`: `NoCapability`,
`UnsafeComposition`, `UnsafeEntityComposition(v)` (E2 Slice B),
`Bounds(v)`, `Registry(e)`.

**PolicyDecision**
`Allow`, `AllowWithWarnings`, `RequireApproval`, `Deny`. Returned
by every rule aggregation. See
[../docs/policies.md](../docs/policies.md).

**PolicyEngine**
Owns a set of `Rule`s and evaluates them at the four hook
points. `with_default_rules` loads the seven defaults. Custom
rules are added via `add_rule`.

**PolicyContext**
The struct a rule inspects. Fields: intent, capability,
requester, payload, output, side_effect, remaining_budget,
tenant, composed_writes, `ontology_class_lookup` (E2 Slice B).

**PrePlan / PreStep / PostStep / PreArtifact**
The four canonical policy hook points (Rule 6). Plus the X1
Slice B revocation gate which runs *before* `PrePlan`.

---

## R

**Recorder**
The default `TraceRecorder` implementation. Persists into an
`Arc<dyn TraceStore>` and holds in-flight traces in a
`parking_lot::Mutex`. Lives in
`aaf-trace::recorder::Recorder`.

**Registry**
The capability registry. CRUD plus lexical discovery plus
entity-aware discovery (E2 Slice B) plus attestation gate (X1
Slice B). Lives in `aaf-registry::store::Registry`.

**Revocation Registry**
X1 Slice A. Serves a signed list of revoked DIDs / prompt
hashes / tool versions. The runtime consults it before the
trace opens (X1 Slice B).

**RiskTier**
One of `Read`, `Write`, `Advisory`, `Delegation`, `Governance`.
Drives policy gating and LLM routing.

**Rule (policy)**
A trait with `id()` and `evaluate()`. Seven defaults ship
(scope, side_effect, budget, pii, injection, composition,
boundary). Custom rules added via `PolicyEngine::add_rule`.

**Rule (architectural)**
One of the numbered rules in `CLAUDE.md`. Rules 1–13 are the
foundation; 14–21 cover E1/E2/E3; 22–24 cover X1.

---

## S

**Saga** → see **Agentic Saga**.

**SBOM** → see `AgentSbom`.

**Side Effect**
A classification on `CapabilityContract`: `None`, `Read`,
`Write`, `Delete`, `Send`, `Payment`. Write-class side effects
(the last four) must carry a `compensation` (Rule 9).

**Sidecar**
A per-service proxy container in Pattern A. Lives in
`aaf-sidecar`. Publishes capabilities, evaluates local
fast-path rules, applies guards, and implements Rule 13's
transparent fallback.

**Slice A / B / C**
The three-slice discipline every enhancement follows:
- **A** — contracts + crate skeleton + in-memory impl + unit tests
- **B** — integration into the hot-path crates
- **C** — SDK primitives, examples, CLI, polish

---

## T

**Task**
A state machine that a single intent traverses.
`Proposed → Ready → Running → Completed` for the happy path,
plus recovery and proposal-lifecycle states. Lives in
`aaf-contracts::task::TaskState`.

**Tenant**
A multi-tenant deployment boundary. Carried on
`Requester.tenant` and `EntityRefLite.tenant`. Enforced by the
policy boundary rule and the federation router.

**Topological Sort**
Kahn's algorithm, as implemented in `Graph::validate`. Produces
a deterministic execution order, with ties broken by sorted
node id.

**Trace**
The typed record of a single execution. Contains steps, each
with an observation. Exported via OTLP.

**Trust Score**
A clamped `[0.0, 1.0]` float representing behavioural trust.
Distinct from cryptographic trust (X1 Slice B).

---

## U

**Untyped Internals — forbidden (Rule 2)**
Every cross-crate message must be a typed `aaf-contracts`
shape. Natural language exists only at the front door and
inside LLM prompts.

---

## V

**ValueRouter**
`aaf-llm::router::ValueRouter` — picks an `LLMProvider` based
on risk tier. `LearnedRoutingPolicy` (E1 Slice B) plugs in and
adjusts routing weights driven by observed outcomes.

---

## W

**Wave 1 / Wave 2**
- **Wave 1** — E1 / E2 / E3, the Slice A contract foundations
  plus Slice B/C integration work.
- **Wave 2** — X1 / X2 / X3, the agent identity, knowledge
  fabric, and developer experience enhancements.

**Wrapper**
The in-process analog of the sidecar. Lives in `aaf-wrapper`.
Used in Pattern B (modular monoliths).

---

## X

**X1 / X2 / X3**
Wave-2 enhancements:
- **X1 — Agent Identity, Provenance & Supply Chain** (DID,
  keystore, manifest, SBOM, tokens, revocation)
- **X2 — Semantic Knowledge Fabric** (deferred)
- **X3 — Developer Experience Surface** (deferred)

---

## F

**F1 / F2 / F3**
Wave-4 critical infrastructure enhancements:
- **F1 — Developer Experience Platform** (Python/TypeScript/Go
  SDKs, CLI, code generation from JSON Schema)
- **F2 — Live LLM Integration & Intelligent Model Routing**
  (Anthropic/OpenAI/local providers, ValueRouter, ProviderMetrics,
  pricing, budget pre-check)
- **F3 — Universal Protocol Bridge** (MCP client/server, A2A
  participant, ProtocolBridge unifier, governed external calls)

**Fast Path Miner**
An E1 Slice B subscriber in `aaf-learn` that proposes new
fast-path rules from recurring agent-assisted patterns. Requires
approval (Rule 18). See also: F2 (live providers make this
operationally meaningful).

---

## Acronyms — quick reference

| Acronym | Expansion |
|---|---|
| AAF | Agentic Application Framework |
| A2A | Agent-to-Agent |
| ADR | Architecture Decision Record |
| CEL | Common Expression Language (referenced in `entity_scope`) |
| DID | Decentralized Identifier |
| E1/E2/E3 | Wave-1 enhancements (feedback, ontology, surface) |
| HMAC | Hash-based Message Authentication Code |
| LLM | Large Language Model |
| MCP | Model Context Protocol |
| NL | Natural Language |
| OTLP | OpenTelemetry Protocol |
| PII | Personally Identifiable Information |
| SBOM | Software Bill of Materials |
| SDK | Software Development Kit |
| X1/X2/X3 | Wave-2 enhancements (identity, knowledge, DX) |
| F1/F2/F3 | Wave-4 infrastructure (SDKs, LLM providers, protocol bridges) |
| SSE | Server-Sent Events |
