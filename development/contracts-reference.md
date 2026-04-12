# Contracts Reference

> The eight contract types that flow between crates. For exhaustive
> field-level documentation run `cargo doc -p aaf-contracts --open`.
> This file explains *why* each type exists, what its invariants are,
> and where they are enforced.

Every type below lives in `core/crates/aaf-contracts/src/`. Nothing
outside that crate may define or hand-roll these shapes — contracts
are the single source of truth and every other crate imports them.

---

## `IntentEnvelope`

`intent.rs` — the typed representation of a user goal.

```
IntentEnvelope {
    intent_id: IntentId,
    intent_type: IntentType,          // Transactional/Analytical/Planning/Delegation/Governance
    requester: Requester,
    goal: String,                     // NL preserved for the agent nodes
    domain: String,
    constraints: BTreeMap<String, Value>,
    budget: BudgetContract,           // max_tokens / max_cost_usd / max_latency_ms
    deadline: Option<DateTime<Utc>>,
    risk_tier: RiskTier,
    approval_policy: String,
    output_contract: Option<OutputContract>,
    trace_id: TraceId,
    depth: u32,                       // ≤ 5
    created_at: DateTime<Utc>,
    entities_in_context: Vec<EntityRefLite>,  // E2 Slice B
}
```

**Invariants (`IntentEnvelope::validate`):**

- `goal` non-empty
- `domain` non-empty
- `depth ≤ MAX_DEPTH` (= 5) — Rule 8
- `budget.max_cost_usd` is finite and non-negative

**Delegation (`IntentEnvelope::delegate`):**

- Mints a new `intent_id`
- Increments `depth` by 1
- Rejects if the new depth would exceed `MAX_DEPTH`

**Where it flows:**

- Produced by `aaf-intent::IntentCompiler::compile`
- Consumed by `aaf-planner::RegistryPlanner::plan`
- Threaded through `aaf-runtime::executor` into every node invocation
- Logged by `aaf-trace::Recorder` at trace open

---

## `CapabilityContract`

`capability.rs` — the typed declaration of an executable capability.

```
CapabilityContract {
    id: CapabilityId,
    name: String,
    description: String,
    version: String,                       // semver
    provider_agent: String,
    endpoint: CapabilityEndpoint,
    input_schema: serde_json::Value,       // JSON Schema
    output_schema: serde_json::Value,
    side_effect: SideEffect,               // None/Read/Write/Delete/Send/Payment
    idempotent: bool,
    reversible: bool,
    deterministic: bool,
    compensation: Option<CompensationSpec>,
    sla: CapabilitySla,
    cost: CapabilityCost,
    required_scope: String,
    data_classification: DataClassification,
    degradation: Vec<DegradationSpec>,
    depends_on: Vec<CapabilityId>,
    conflicts_with: Vec<CapabilityId>,
    tags: Vec<String>,
    domains: Vec<String>,

    // E2 Slice A ontology fields
    reads: Vec<EntityRefLite>,
    writes: Vec<EntityRefLite>,
    emits: Vec<EventRefLite>,
    entity_scope: Option<EntityScopeLite>,

    // X1 Slice A identity field
    required_attestation_level: Option<AttestationLevelRef>,
}
```

**Invariants (`CapabilityContract::validate`):**

- `name` and `version` non-empty
- **Rule 9 compensation gate:** if `side_effect` is
  `Write`/`Delete`/`Send`/`Payment`, `compensation` must be
  `Some(_)`. `aaf-registry::Registry::register` calls `validate`
  at registration time and refuses to insert non-compliant caps.

**Ontology fields (E2):**

- `reads` / `writes` / `emits` are optional — a capability without
  declarations falls back to pre-Slice-B semantics. The
  `ontology-lint` tool tracks adoption and flips to strict mode at
  90%.
- `entity_scope` carries a free-form predicate string (e.g.
  `"tenant_id = $caller"`) — today it is opaque; a future Slice C
  will parse it into a structured filter.

**Who reads the fields:**

- `aaf-registry::discover_by_entity` indexes by `reads`/`writes`/`emits`
- `aaf-planner::composition::EntityAwareComposition` runs the double-write
  / classification-leak / cross-tenant detectors on them
- `aaf-policy::rules::boundary` consults `reads` and the ontology lookup
- `aaf-federation::Router::enforce_capability` walks all three lists

---

## `Task`

`task.rs` — the state machine a single intent traverses in the
runtime.

```
TaskState {
    Proposed,
    WaitingForContext,
    Ready,
    Running,
    PausedForApproval,
    Failed,
    Analyzing,
    Recovering,
    Completed,
    Cancelled,

    // E3 Slice A proposal lifecycle
    ProposedMutation,
    AppReview,
    Accepted,
    Rejected,
    Transformed,
    Expired,
}
```

**Invariants:** `TaskState::can_transition_to` defines the legal
transitions. Terminal states (`Completed`, `Cancelled`, `Rejected`,
`Expired`) have no outgoing transitions.

---

## `Artifact`

`artifact.rs` — any produced output that carries provenance and a
signature.

```
Artifact {
    id: ArtifactId,
    kind: ArtifactKind,
    content: serde_json::Value,
    producing_agent: String,
    trace_id: TraceId,
    created_at: DateTime<Utc>,
    signature: String,

    // E2 Slice A: ontology lineage
    derived_from: Vec<EntityRefVersioned>,
}
```

**Signature semantics:**

- **Legacy (`v0:`)** — hand-rolled checksum produced by
  `aaf-trust::signing::sign_artifact`. Still supported for
  back-compat; `verify_artifact_with` reports `false` on these.
- **X1 Slice B (`x1:<did>:<sig>`)** — DID-bound signature produced
  by `sign_artifact_with(&mut artifact, &did, &keystore)`, verified
  by `verify_artifact_with(&artifact, &keystore)`. Tamper-detection
  included.

---

## `Handoff`

`handoff.rs` — delegation from one agent / human to another.

Fields include `parent_intent_id`, `delegator_agent_id`,
`delegatee_agent_id`, `depth_after`, `reason`, and the X1 Slice B
addition `capability_token: Option<CapabilityTokenLite>`.

**Rule 8:** every handoff must leave `depth_after ≤ 5`.
**Trust min propagation:** `aaf-trust::delegation::effective_trust`
returns `min(delegator_trust, delegatee_trust)`.

---

## `Observation`

`observation.rs` — what gets recorded on every runtime step.

```
Observation {
    observation_id,
    trace_id, step_id, node_id,
    started_at, ended_at,
    status: ObservationStatus,     // Started/Completed/Failed/Paused
    input_digest: String,
    output_digest: Option<String>,
    cost_usd: f64,
    tokens_used: u64,
    // E1 Slice A feedback spine
    outcome_detail: Option<Outcome>,
}

Outcome {
    status: OutcomeStatus,         // Succeeded/Failed/Partial/Escalated/RolledBack
    latency_ms: u64,
    tokens_used: u64,
    cost_usd: f64,
    policy_violations: Vec<PolicyViolation>,
    user_feedback: Option<UserFeedback>,
    downstream_error: Option<DownstreamError>,
    semantic_score: Option<SemanticScore>,
}
```

The `outcome_detail` field is the **single canonical location** for
outcome data (Rule 15). Everything that wants to learn from a past
run reads this field.

---

## `TrustScore` / `AutonomyLevel`

`trust.rs` — behavioural trust dimension.

```
AutonomyLevel {
    Observer = 0,
    Assistant = 1,
    Collaborator = 2,
    Operator = 3,
    Custodian = 4,
}
```

**`min(a, b)` propagation:** `aaf-trust::delegation::effective_trust`
takes a delegator and a delegatee and returns the minimum — no
delegation can increase trust.

`TrustScore` is a clamped `[0.0, 1.0]` float, with `clamped(f)`
pinning NaN / out-of-range to 0.0. X1 Slice B adds cryptographic
trust *underneath* this — `verify_token` checks signature + validity
window + scope + remaining depth before the numeric check runs.

---

## `PolicyDecision` / `PolicyViolation` / `RuleKind`

`policy.rs` — what rules return.

```
PolicyDecision {
    Allow,
    AllowWithWarnings(Vec<PolicyViolation>),
    RequireApproval(Vec<PolicyViolation>),
    Deny(Vec<PolicyViolation>),
}

PolicyViolation {
    rule_id: String,
    kind: RuleKind,           // one of 7
    severity: PolicySeverity, // Info/Warning/Error/Fatal
    message: String,
}
```

**Decision algorithm (`PolicyEngine::evaluate`):**

1. Any violation of severity `Error` or `Fatal` → `Deny`.
2. Any `SideEffectGate` violation at `Warning`+ → `RequireApproval`.
3. Any remaining warnings → `AllowWithWarnings`.
4. Otherwise → `Allow`.

---

## Identity contracts (X1)

`identity.rs` — wire-format shapes for the identity layer.

```
AgentDidRef(String)                // "did:aaf:<thumbprint>"
AttestationLevelRef { level: u8, rationale: String }
TokenClaimsLite { caller, callee, capabilities, depth_remaining, issued_at, expires_at }
CapabilityTokenLite { claims: TokenClaimsLite, signature: String }
```

These are the wire shapes. The full types with sign/verify behaviour
live in `aaf-identity::{manifest, sbom, attestation, delegation}`.

---

## Ontology contracts (E2)

`capability.rs` carries the ontology field *wire shapes* so every
crate can reference an entity without depending on `aaf-ontology`.

```
EntityRefLite {
    entity_id: String,               // "commerce.Order"
    version: EntityVersionLite,
    tenant: Option<TenantId>,        // Rule 21
    local_id: Option<String>,
}

EventRefLite {
    id: String,                      // "commerce.OrderPlaced"
    version: EntityVersionLite,
}

EntityScopeLite { expression: String }
```

The full types with classification + resolver + lineage live in
`aaf-ontology`. The split is deliberate: `aaf-contracts` has no
`aaf-*` dependencies, so every downstream crate can use the Lite
shapes without ordering headaches.

---

## IDs

`ids.rs` — every id type used in the workspace. All are
newtype-wrapped `String`s so mismatches are caught at compile time.

```
IntentId, TraceId, TaskId, CapabilityId, NodeId, ArtifactId,
TenantId, EventId, ProposalId, ProjectionId, SessionId, UserId
```

Every id implements `Debug`, `Clone`, `PartialEq`, `Eq`, `Hash`,
`Serialize`, `Deserialize`, and `new()` / `from(s)` constructors.
The runtime generates ids via `*::new()` which produces a
hex-encoded random suffix.
