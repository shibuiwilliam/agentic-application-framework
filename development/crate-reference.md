# Crate Reference

> One page per crate. For every crate: what it owns, its key public
> types, how it is tested, and its Slice status. Keep this file in
> sync with `core/crates/*/Cargo.toml` descriptions and the
> `IMPLEMENTATION_PLAN.md` roadmap.

Test counts in this file refer to **unit tests inside the crate**,
not integration tests in `core/tests/integration/`. Integration tests
are listed separately in `development/testing-strategy.md`.

---

## Tier 1 — Pure contracts / storage

### `aaf-contracts`

**Role:** The typed surface. Every cross-crate message lives here
as a serde-friendly Rust struct. No behaviour, no async, no
dependencies on other `aaf-*` crates.

**Key modules & types:**

- `intent::{IntentEnvelope, IntentType, Requester, BudgetContract, OutputContract, RiskTier, MAX_DEPTH}`
- `capability::{CapabilityContract, SideEffect, DataClassification, EndpointKind, CapabilityEndpoint, CompensationSpec, CapabilitySla, CapabilityCost, DegradationSpec, DegradationLevel, EntityRefLite, EntityVersionLite, EventRefLite, EntityScopeLite}`
- `task::{Task, TaskState}`
- `artifact::{Artifact, ArtifactKind}`
- `handoff::{Handoff, HandoffReason}`
- `observation::{Observation, Outcome, UserFeedback, SemanticScore}`
- `trust::{TrustScore, AutonomyLevel}`
- `policy::{PolicyDecision, PolicySeverity, PolicyViolation, RuleKind}`
- `trace::{Trace, TraceStep, TraceStatus}`
- `identity::{AgentDidRef, AttestationLevelRef, CapabilityTokenLite, TokenClaimsLite}` (X1 Slice A)
- `ids::{IntentId, TraceId, TaskId, CapabilityId, NodeId, ArtifactId, TenantId, EventId, ProposalId, ProjectionId, SessionId, UserId}`

**Invariants enforced at construction:**

- `CapabilityContract::validate` — Rule 9: write/delete/send/payment side-effect must carry `compensation`.
- `IntentEnvelope::validate` — Rule 8: depth ≤ 5, budget non-negative.
- `IntentEnvelope::delegate` — Rule 8: delegation increments depth.

**Status:** Implemented + tested. Must remain backwards-compatible
across every slice — it is the *only* way crates communicate.

---

### `aaf-storage`

**Role:** Storage trait definitions and in-memory backends. **No
other crate may import a database driver directly** (Rule 11).

**Key traits:**

- `CheckpointStore` — graph execution state persistence
- `WorkingMemoryStore` — per-task transient state
- `ThreadMemoryStore` — per-session / per-case state
- `LongTermMemoryStore` — with E2 Slice B additions
  - `insert` / `search` (keyword)
  - `search_by_entity(tenant, entity_ref, limit)` (default impl + O(1) in-memory override)
- `ArtifactStore`
- `TraceStore`
- `RegistryStore`

**Key structs:**

- `LongTermRecord { tenant, kind, content, payload, entity_refs }`
  — the `entity_refs` field is what the E2 Slice B inverted index
  keys off.

**Status:** Every trait has an `InMemory*` implementation for tests
and dev. Real drivers (PostgreSQL, Redis, S3, ClickHouse, pgvector)
are deferred to post-Wave-2.

---

### `aaf-ontology`

**Role:** The noun layer (E2). Defines what an `Entity` is and how
entity classifications flow.

**Key modules & types:**

- `entity::{Entity, EntityField, EntityId, EntityVersion, Classification, FieldType, EntityRef, EntityScopePredicate, EventRef}`
- `relation::{Relation, RelationKind, Cardinality}`
- `registry::{OntologyRegistry (trait), InMemoryOntologyRegistry}`
- `resolver::{EntityResolver, ExactMatchResolver, ResolverOutcome}`
- `lineage::{LineageRecord, EntityRefVersioned}`
- `version::{VersionCompatibility, compare_versions}`
- `import` — best-effort ingest from JSON Schema / OpenAPI shapes
- `error::OntologyError` — includes `ClassificationDowngrade`,
  `DanglingRelation`, `IncompatibleVersion`

**Classification lattice:** `Public ⊂ Internal ⊂ Pii ⊂ Regulated(_)`.
The lattice is enforced by `can_flow_into` and consulted by the
policy boundary rule (through the callback in `PolicyContext`,
because `aaf-policy` does *not* depend on `aaf-ontology`).

**Status:** E2 Slice A implemented; Slice B/C wire it into planner,
policy, memory, federation.

---

### `aaf-transport`

**Role:** Transport abstraction trait. Real protocol drivers
(gRPC/tonic, HTTP/axum, NATS, WebSocket, CloudEvents) are deferred.

**Status:** Trait-only. Placeholder.

---

## Tier 2 — Domain modules

### `aaf-trace`

**Role:** Observation + trace recording (Rule 12).

**Key types:**

- `Recorder` implementing `TraceRecorder`
- `TraceRecorder` trait with `record_observation`, `close_trace`, `attach_outcome`
- `cost_attribution::{CostAttributor, CostBucket}` — per-department cost rollups
- `otel::OtlpExporter` — OTLP/JSON span export (no heavy OTel SDK dep)
- `replay::Replayer` — checkpoint-based replay

**Status:** Slice A is comprehensive. Slice B / real storage driver
deferred.

---

### `aaf-trust`

**Role:** Trust score, autonomy, delegation chain, artifact signing.

**Key types:**

- `score::TrustScore`
- `autonomy::AutonomyLevel` (5 levels)
- `delegation::{effective_trust (min(a,b)), verify_token}` (X1 Slice B)
- `promotion::Promotion` — reputation ramp
- `signing::{sign_artifact, sign_artifact_with, verify_artifact_with}` — DID-bound signing (X1 Slice B)

**Status:** Wave-1 numeric trust + X1 Slice B cryptographic trust
both landed.

---

### `aaf-memory`

**Role:** 4-layer memory orchestration + context budget.

**Key types:**

- `facade::MemoryFacade` — aggregates `WorkingMemoryStore`,
  `ThreadMemoryStore`, `LongTermMemoryStore`, `ArtifactStore` into
  one handle.
  - `longterm_search(tenant, query, limit)`
  - `longterm_search_by_entity(tenant, entity_ref, limit)` (E2 Slice B)
  - `longterm_insert(record)`
- `context::ContextBudget` — ~7,500 token budget per LLM call
  (system ~2,000 + intent ~500 + memory ~2,000 + step ~1,000 +
  tools ~2,000).

**Status:** Implemented + tested.

---

### `aaf-policy`

**Role:** Policy engine, three guards, seven rules, approval workflow.

**Key types:**

- `engine::{PolicyEngine, PolicyHook}` with 4 hooks: `PrePlan`,
  `PreStep`, `PostStep`, `PreArtifact`
- `context::{PolicyContext, EntityClass, OntologyClassificationLookup}` (E2 Slice B lookup)
- `guard::{InputGuard, OutputGuard, ActionGuard}` (Rule 7)
- `rules::{scope, side_effect, budget, pii, injection, composition, boundary}` (7 rules)
  - `boundary::BoundaryEnforcement` consults the ontology lookup
    when present (E2 Slice B)
- `approval::{ApprovalRequest, ApprovalState, ApprovalWorkflow}`
- `PolicyDecision` aggregation: `Allow`, `AllowWithWarnings(_)`,
  `RequireApproval(_)`, `Deny(_)`

**Status:** Every rule has tests. Boundary rule's ontology path is
opt-in — call sites that don't wire an `OntologyClassificationLookup`
fall back to pre-Slice-B legacy behaviour.

---

### `aaf-registry`

**Role:** Capability registry — CRUD, discovery, health, degradation,
versioning, attestation gating.

**Key types:**

- `store::Registry` (wraps a `RegistryStore`). `register` enforces
  `CapabilityContract::validate` (Rule 9 at registration).
- `discovery::{DiscoveryQuery, DiscoveryResult, EntityQueryKind}`
  - `Registry::discover(&query)` — lexical
  - `Registry::discover_by_entity(entity_ref, kind)` — entity-keyed (E2 Slice B)
- `health::HealthMonitor` / `HealthStatus`
- `degradation::{DegradationStateMachine, DegradationTransition}`
- `a2a` — A2A Agent Card import / export
- `version::CapabilityVersion`
- `Registry::get_for_attestation` (X1 Slice B) — enforces
  `required_attestation_level`

**Status:** Implemented + tested.

---

### `aaf-intent`

**Role:** Intent compiler pipeline.

**Key types:**

- `compiler::{IntentCompiler, CompileOutcome}`
- `classifier::{Classifier, RuleClassifier}` — NL → `IntentType`
- `extractor::{Extractor, RuleExtractor}` — NL → constraints
- `enricher::{Enricher, OntologyResolver}` —
  - `Enricher::enrich` (role-based defaults)
  - `Enricher::enrich_with_ontology(env, resolver)` (E2 Slice B —
    populates `entities_in_context`)
- `refinement::{Refiner, ClarificationQuestion}`
- `cache::IntentCache` — semantic-hash cache
- `versioning::IntentVersionRegistry`

**Status:** Implemented + tested.

---

### `aaf-llm`

**Role:** LLM provider abstraction + routing + per-call budget.

**Key types:**

- `provider::LLMProvider` trait
- `mock::MockProvider` — deterministic for tests
- `router::ValueRouter` — picks a model per request based on value
- `budget::PerCallBudget`

**Status:** Wave-1 foundation. Real providers (Anthropic, OpenAI,
Bedrock, Vertex, Ollama/vLLM) deferred.

---

### `aaf-identity`

**Role:** Agent identity, provenance, supply chain (X1 Slice A+B).

**Key types:**

- `did::AgentDid` — public-key thumbprint (Rule 22)
- `keystore::{Keystore, Signer, Verifier, InMemoryKeystore, KeyMaterial}`
- `manifest::{AgentManifest, ManifestBuilder, ModelPin, ToolBinding}` — sign at build time (Rule 23)
- `sbom::{AgentSbom, SbomEntry, SbomEntryKind}` — content-hash BOM (Rule 24)
- `attestation::{Attestation, AttestationLevel, Attester}`
- `delegation::{CapabilityToken, TokenClaims}` — issue / verify / expiry
- `revocation::{RevocationRegistry, InMemoryRevocationRegistry, RevocationEntry, RevocationKind}`
- `error::IdentityError`

**Signature backend:** Slice A ships a deterministic HMAC-SHA256
backend. Slice B swapped the runtime integration but the backend
itself remains HMAC; Ed25519 is a Slice C deliverable.

**Status:** X1 Slice A (crate foundations) + Slice B (runtime / trust
/ registry wiring) both landed. Slice C deferred.

---

### `aaf-eval`

**Role:** Feedback spine — evaluation harness (E1 Slice A).

**Key types:**

- `judge::{Judge, DeterministicJudge}` (Jaccard-based)
- `golden::{GoldenSuite, GoldenCase}`
- `replay::{Replayer, ReplayDivergence}`
- `regression::{RegressionReport, ReportWriter}`

**Status:** E1 Slice A landed. `aaf-learn` (Slice B subscribers)
deferred.

---

### `aaf-surface`

**Role:** App-Native Surface (E3 Slice A).

**Key types:**

- `event::{AppEvent, EventType, Situation, ScreenContext, SessionContext, SurfaceConstraints}`
- `ingest::{EventToIntentAdapter, RuleBasedAdapter}`
- `proposal::{ActionProposal, StateMutationProposal, ProposalLifecycle, UiHintKind}`
  - `ActionProposal::new_with_mutations` enforces Rule 20 at construction
- `projection::{StateProjection, allows_field}` — Rule 19 default-deny
- `lifecycle::ApprovalState` tied into the task state machine
- `situation_packager::SituationPackager` — ~7,500 token budget

**Status:** E3 Slice A landed. `EventGateway` and surface-keyed
thread memory deferred.

---

## Tier 3 — Composition

### `aaf-runtime`

**Role:** Graph runtime executor.

**Key types:**

- `graph::{GraphBuilder, Graph}`
- `executor::{GraphExecutor, ExecutionOutcome, RuntimeError}`
  - `RuntimeError::Revoked` — X1 Slice B revocation gate
  - `GraphExecutor::with_revocation(Arc<dyn RevocationRegistry>)`
- `node::{Node (trait), DeterministicNode, AgentNode, ApprovalNode, ForkNode, EventWaitNode}`
- `checkpoint::CheckpointWriter`
- `compensation::CompensationChain` — runs in reverse on node failure
- `budget::BudgetTracker` (Rule 8)
- `scheduler::Scheduler` — sequential + parallel
- `timeout::Timeout`

**Policy hook call order:** `PrePlan` → *(optional: revocation gate)*
→ per step: `PreStep` → run node → `PostStep` → optional `PreArtifact`
→ record observation.

**Status:** Implemented + tested. Compensation chain is wired into
`GraphExecutor::run` so write steps register their compensator and
failures drain the chain in reverse (iteration 3 bug fix).

---

### `aaf-saga`

**Role:** Agentic saga — extended with intelligent recovery.

**Key types:**

- `definition::{SagaDefinition, SagaStep, RecoveryRule}`
- `executor::SagaExecutor`
- `compensation::CompensationStrategy`
- `recovery::{RecoveryAnalyzer, RecoveryAction}`
- `state::SagaStateMachine` (`initiated → running → analyzing → recovery_selected → …`)
- `bridge::StepRunner` — produces a step runner from `Registry` + `RegistryClient`

**Status:** Implemented + tested. The bridge connects the saga to
real capability invocations.

---

### `aaf-planner`

**Role:** Planner / router.

**Key types:**

- `planner::{RegistryPlanner, PlannerError}`
  - `PlannerError::UnsafeEntityComposition(CompositionViolation)` (E2 Slice B)
  - `RegistryPlanner::with_entity_composition(checker)`
- `router::{Router, CommunicationPattern}` — 4 patterns
- `fast_path::{FastPathRule, FastPathRuleSet, FastPathOutcome}`
- `bounds::{BoundedAutonomy, BoundsViolation}`
- `composition::{CompositionChecker, EntityAwareComposition, CompositionViolation, ClassificationHint, ClassificationLookup}`
  - 3 new detectors (E2 Slice B): `DoubleWrite`, `ClassificationLeak`, `CrossTenantFanOut`
- `cache::PlanCache`
- `plan::{ExecutionPlan, PlannedStep, PlannedStepKind}`

**Status:** Implemented + tested, entity-aware composition landed
in E2 Slice B.

---

### `aaf-sidecar`

**Role:** Agent sidecar for microservices (Rule 13).

**Key types:**

- `proxy::Proxy` with `forward_direct` for transparent fallback
- `capability::CapabilityPublisher` — auto-publish from sidecar config
- `fast_path::LocalFastPath` — evaluates rules locally, no control-plane round trip
- `guard::LocalGuard` — input + output + action guards
- `mapping::FieldMapping` — intent field ↔ API field
- `health::UpstreamHealth`

**Status:** Foundational — every component exists; wire-format
drivers deferred.

---

### `aaf-wrapper`

**Role:** In-process wrapper for modular monoliths.

**Key types:**

- `wrapper::ModuleWrapper`
- `capability::MethodToCapability`
- `guard::InProcessGuard`

**Status:** Foundational.

---

### `aaf-federation`

**Role:** Cell / cross-org federation (E2 Slice C).

**Key types:**

- `CellId`, `CellConfig`
- `FederationAgreement`:
  - `with_prohibited_fields(...)` — legacy string denylist
  - `with_entity_rules(...)` — entity-space (Slice C)
- `EntityAccessRule { entity_id, op, max_classification?, tenant? }`
- `EntityOp { Read, Write, Emit }`
- `ClassificationCap { Public, Internal, Confidential, Restricted }`
- `FederationError` — 5 variants: `NoAgreement`, `BoundaryViolation`,
  `EntityNotPermitted`, `ClassificationCapExceeded`, `TenantMismatch`
- `Router`
  - `route(capability_id)`
  - `enforce_outbound(from, to, payload)` — legacy
  - `enforce_capability(from, to, cap)` — entity-space (Slice C)
  - `enforce_outbound_entity(from, to, entity_ref, op)` — single-entity helper

**Status:** E2 Slice C landed. Cross-cell co-signed tokens deferred
to X1 Slice C.

---

### `aaf-server`

**Role:** Reference binary. Single source of CLI truth.

**Subcommands:**

- `aaf-server run [path]` — full pipeline: compile → plan → execute
- `aaf-server validate <path>` — YAML validation only
- `aaf-server discover <query>` — ad-hoc registry discovery
- `aaf-server compile <text>` — NL → envelope JSON
- `aaf-server ontology lint <dir>` — E2 Slice C: lint capability YAMLs
  for entity declarations
- `aaf-server ontology import <openapi>` — E2 Slice C: import OpenAPI
  into proposed ontology YAML
- `aaf-server help` — subcommand list

**Modules:**

- `main.rs` — dispatch + wiring
- `config::{ServerConfig, CapabilitySeed, ProjectConfig, DemoConfig}` — YAML-driven
- `lint` — E2 Slice C lint module (LintFinding / Severity / ratio ramp)
- `import` — E2 Slice C OpenAPI → ontology importer

**Status:** Implemented + tested. gRPC / REST / WebSocket drivers
deferred.
