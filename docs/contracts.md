# Contracts

> The eight typed messages that flow between every crate. For
> field-level documentation run
> `cargo doc -p aaf-contracts --open`. For detailed invariant
> rules and enforcement points read
> [`../development/contracts-reference.md`](../development/contracts-reference.md).

All contract types live in `core/crates/aaf-contracts/src/`.
Nothing outside that crate may hand-roll these shapes. Every
other crate imports them.

---

## IntentEnvelope

The typed representation of a user goal. Produced by the intent
compiler, consumed by every downstream layer.

Key fields: `intent_id`, `intent_type` (one of five: Transactional,
Analytical, Planning, Delegation, Governance), `requester`, `goal`
(free-text NL preserved for agent nodes), `domain`, `constraints`,
`budget` (tokens / cost / latency), `deadline`, `risk_tier`,
`approval_policy`, `output_contract`, `trace_id`, `depth` (≤ 5),
`entities_in_context` (E2 Slice B ontology refs).

Key invariants (`IntentEnvelope::validate`):

- `goal` and `domain` non-empty
- `depth ≤ MAX_DEPTH (= 5)` — Rule 8
- `budget.max_cost_usd` finite and non-negative

Delegation (`IntentEnvelope::delegate`) mints a new `intent_id`
and increments `depth`. Rejects if `depth > MAX_DEPTH`.

---

## CapabilityContract

The typed declaration of an executable capability. Registered
with `aaf-registry::Registry::register`.

Key fields: `id`, `name`, `description`, `version`, `provider_agent`,
`endpoint`, `input_schema`, `output_schema`, `side_effect`
(None/Read/Write/Delete/Send/Payment), `idempotent`, `reversible`,
`deterministic`, `compensation`, `sla`, `cost`, `required_scope`,
`data_classification`, `degradation`, `depends_on`,
`conflicts_with`, `tags`, `domains`.

**E2 Slice A ontology fields:** `reads`, `writes`, `emits`,
`entity_scope`.

**X1 Slice A identity field:** `required_attestation_level`.

Key invariant (`CapabilityContract::validate`):

- **Rule 9 compensation gate** — if `side_effect` is
  `Write`/`Delete`/`Send`/`Payment`, `compensation` must be
  `Some(_)`. `Registry::register` rejects violations.

The ontology fields are optional today; `make ontology-lint`
tracks adoption and flips to strict mode at 90%.

---

## Task

The state machine a single intent traverses in the runtime.

States:

```
Proposed → WaitingForContext → Ready → Running
  ↓                                    ↓
  ↓                          PausedForApproval → Running
  ↓                                    ↓
  ↓                             Failed → Analyzing → Recovering
  ↓                                    ↓            ↓
  Completed                         (various recovery paths)
  Cancelled
```

**E3 Slice A extensions** (proposal lifecycle):

```
ProposedMutation → AppReview
                     ↓
                   Accepted → Running → Completed
                   Rejected → Cancelled
                   Transformed → Running
                   Expired → Cancelled
```

`TaskState::can_transition_to` defines the legal transitions.

---

## Artifact

Any produced output that carries provenance and a signature.

Key fields: `id`, `kind`, `content`, `producing_agent`,
`trace_id`, `created_at`, `signature`, `derived_from` (E2 Slice A
lineage).

Signature semantics:

- **Legacy `v0:`** — hand-rolled checksum, produced by the
  pre-X1 `sign_artifact` helper. Still supported but
  `verify_artifact_with` reports `false`.
- **`x1:<did>:<sig>`** — DID-bound signature from X1 Slice B's
  `sign_artifact_with`. Verified by `verify_artifact_with`,
  tamper-detected.

---

## Handoff

Delegation from one agent / human to another. Carries the
delegator and delegatee ids, the reason, the resulting depth,
and (X1 Slice B) an optional `capability_token`.

- **Rule 8:** `depth_after ≤ 5`.
- **Trust min propagation:** `effective_trust(delegator, delegatee)`
  returns `min(delegator, delegatee)`.
- **X1 Slice B:** when a `capability_token` is present,
  `aaf-trust::verify_token` checks signature + expiry + scope +
  remaining depth before honouring the handoff.

---

## Observation

What gets recorded on every runtime step. The **single canonical
location** for outcome data (Rule 15).

Key fields: `observation_id`, `trace_id`, `step_id`, `node_id`,
`started_at`, `ended_at`, `status`, `input_digest`, `output_digest`,
`cost_usd`, `tokens_used`, `outcome_detail`.

The `outcome_detail` block (E1 Slice A) carries:

- `status` (Succeeded / Failed / Partial / Escalated / RolledBack)
- latency / tokens / cost (precise)
- `policy_violations[]` observed after the fact
- `user_feedback?` from the app-native surface or front door
- `downstream_error?` if the compensation chain triggered later
- `semantic_score?` from an `aaf-eval::Judge`

---

## TrustScore / AutonomyLevel

Behavioural trust. A score in `[0.0, 1.0]` and a five-level
autonomy enum:

```
Observer (0) → Assistant (1) → Collaborator (2) → Operator (3) → Custodian (4)
```

X1 Slice B adds cryptographic trust *underneath* behavioural
trust. Delegation requires both to pass.

---

## PolicyDecision / PolicyViolation

What rules return.

```
PolicyDecision {
    Allow,
    AllowWithWarnings(violations),
    RequireApproval(violations),    // side-effect gate
    Deny(violations),                // any Error or Fatal
}
```

Severity ladder in `PolicyEngine::evaluate`:

1. Any `Error`/`Fatal` → `Deny`
2. Any `SideEffectGate` warning → `RequireApproval`
3. Any remaining warnings → `AllowWithWarnings`
4. Otherwise → `Allow`

See [policies.md](policies.md) for the rules.

---

## Identity wire shapes (X1)

Wire-format shapes used throughout the tree without depending on
`aaf-identity`:

- `AgentDidRef(String)` — `"did:aaf:<thumbprint>"`
- `AttestationLevelRef { level: u8, rationale: String }`
- `TokenClaimsLite` — caller, callee, capabilities, depth
  remaining, issued_at, expires_at
- `CapabilityTokenLite` — claims + signature

The full implementations with sign/verify behaviour live in
`aaf-identity`.

---

## Ontology wire shapes (E2)

Wire-format shapes used throughout the tree without depending on
`aaf-ontology`:

- `EntityRefLite { entity_id, version, tenant?, local_id? }`
- `EventRefLite { id, version }`
- `EntityScopeLite { expression }`

The full types with classification + lineage + resolver live in
`aaf-ontology`.

---

## IDs

Every identifier is a newtype around a `String`:

- `IntentId`, `TraceId`, `TaskId`, `CapabilityId`, `NodeId`,
  `ArtifactId`, `TenantId`, `EventId`, `ProposalId`,
  `ProjectionId`, `SessionId`, `UserId`

All implement `new()` (generates a hex-encoded random suffix)
and `from(&str)` (parses a literal in tests).

---

## Further reading

- [`../development/contracts-reference.md`](../development/contracts-reference.md) —
  field-by-field details and invariant enforcement points.
- [`../spec/schemas/`](../spec/schemas/) — JSON Schema 2020-12
  definitions (the external source of truth for config files).
- [`../spec/examples/`](../spec/examples/) — example contract
  instances (validated by `make schema-validate`).
