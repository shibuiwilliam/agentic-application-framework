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
| E1 Feedback Spine | ✓ | **next** | pending |
| E3 App-Native Surface | ✓ | pending | pending |
| X1 Agent Identity | ✓ | ✓ | pending |
| X2 Knowledge Fabric | pending | pending | pending |
| X3 DX Surface | pending | pending | pending |

Per `PROJECT.md` §18 the next iterations are
**E1 Slice C → E3 Slice B → E3 Slice C**, in that order. Every
one is substantial; scoping each as its own iteration is the
right move.

---

## Slice 1 — E1 Slice B — `aaf-learn` crate and subscribers

### Motivation

E1 Slice A (iteration 4) delivered the feedback *shape*: every
`Observation` now carries an optional `outcome_detail: Option<Outcome>`
with status, latency, tokens, cost, policy violations, user
feedback, downstream error, and semantic score. The runtime
attaches a minimal outcome at step-end. `aaf-eval` has the
`Judge` trait + `DeterministicJudge`, `GoldenSuite`, `Replayer`,
and `RegressionReport`.

What's missing: **nobody reads the outcomes back** and nothing
adapts in response. Slice B closes the loop by adding a new
`aaf-learn` crate with four subscriber modules and a
non-blocking subscription contract on `aaf-trace`.

### Scope

| Area | What lands |
|---|---|
| **new crate `aaf-learn`** | Four modules: `fast_path_miner.rs`, `capability_scorer.rs`, `router_tuner.rs`, `escalation_tuner.rs`. Each implements a new `TraceSubscriber` trait defined in `aaf-trace::recorder`. Each writes back through a well-defined extension point so adaptations are *additive*, not invasive. |
| **`aaf-trace::recorder` extensions** | `TraceSubscriber` trait with one method `on_observation(&self, obs: &Observation)`. `Recorder::with_subscriber(Arc<dyn TraceSubscriber>) -> Self` (builder). `Recorder::record_observation` fans out to every subscriber *after* writing to storage and *before* returning. Subscribers are called from a `tokio::spawn` so the hot path never waits (Rule 16). |
| **`aaf-registry` extensions** | `Capability.reputation: f32` (default 0.5, clamped `[0, 1]`). `Capability.learned_rules: Vec<LearnedRuleRef>` (empty by default). New getter `Registry::update_reputation(cap_id, new_score)` gated by rate-limit (no more than one update per cap per minute — prevents adversarial oscillation). |
| **`aaf-llm::router` extensions** | Existing `ValueRouter` gains a pluggable `RoutingPolicy` trait (`fn choose(&self, intent: &IntentEnvelope) -> ModelId`). New impl `LearnedRoutingPolicy` stores weights per `(intent_type, risk_tier, entity_class)` and is driven by `aaf-learn::router_tuner`. |
| **`aaf-planner::fast_path` extensions** | `FastPathRuleSet::add_learned(rule, evidence)` accepts both hand-authored and learned rules. Learned rules are tagged with a `learned_rule_id` so policy packs can disable them wholesale. |
| **new contracts** | `LearnedRule { id, source, evidence, approval_state, scope }`, `RoutingDecisionRecord { call_id, intent_type, risk_tier, model_chosen, cost_usd, quality_score, outcome }`, `ReputationUpdate { cap_id, old, new, evidence_ref }`. All live in `aaf-contracts::learn`. |

### Files to touch

- **New**
  - `core/crates/aaf-learn/Cargo.toml`
  - `core/crates/aaf-learn/src/lib.rs`
  - `core/crates/aaf-learn/src/fast_path_miner.rs`
  - `core/crates/aaf-learn/src/capability_scorer.rs`
  - `core/crates/aaf-learn/src/router_tuner.rs`
  - `core/crates/aaf-learn/src/escalation_tuner.rs`
  - `core/crates/aaf-learn/src/error.rs`
  - `core/crates/aaf-contracts/src/learn.rs` (then add `pub mod learn` + `pub use` in `lib.rs`)
- **Edit**
  - `Cargo.toml` (workspace members + deps)
  - `core/crates/aaf-trace/src/recorder.rs` (subscriber hook)
  - `core/crates/aaf-registry/src/store.rs` (reputation + learned_rules)
  - `core/crates/aaf-llm/src/router.rs` (`RoutingPolicy` trait + `LearnedRoutingPolicy`)
  - `core/crates/aaf-planner/src/fast_path.rs` (learned-rule tagging)
  - `IMPLEMENTATION_PLAN.md` (new iteration section)
  - `development/roadmap.md` (mark E1 Slice B ✓ when done)

### Unit tests expected

- `aaf-learn::fast_path_miner::tests` — at least 5:
  - `mines_recurring_pattern_once_over_threshold`
  - `rejects_adversarial_pattern_concentrated_in_few_sessions`
  - `produces_proposed_rule_with_evidence`
  - `honours_rate_limit`
  - `policy_gate_rejects_unsafe_rule`
- `aaf-learn::capability_scorer::tests` — at least 3:
  - `successful_outcome_nudges_score_up`
  - `failed_outcome_nudges_score_down`
  - `bounded_in_0_1`
- `aaf-learn::router_tuner::tests` — at least 3:
  - `lower_cost_model_chosen_when_quality_equal`
  - `higher_quality_model_chosen_on_higher_risk_tier`
  - `weights_respect_rate_limit`
- `aaf-learn::escalation_tuner::tests` — at least 2
- `aaf-trace::recorder::tests::subscriber_is_not_on_hot_path` — fan-out test that asserts `record_observation` returns **before** the subscriber completes.

### Integration test

- `core/tests/integration/tests/e1_slice_b_smoke.rs`:
  - Set up a recorder with a fast-path miner subscriber.
  - Run 20 agent-assisted intents with the same pattern.
  - Assert the miner produced exactly one `LearnedRule` proposal.
  - Assert the proposal is `ApprovalState::Proposed` (not yet live).
  - Flip the proposal to `Approved` via the approval workflow.
  - Run one more intent matching the pattern.
  - Assert it hits the learned rule (verify via the trace).

### Rules preserved

| Rule | How |
|---|---|
| R15 Feedback is a contract | Already satisfied by `Observation.outcome_detail`; Slice B adds readers, not writers of outcomes. |
| R16 Learning never touches the hot path | `TraceSubscriber::on_observation` is spawned on `tokio::spawn`; the executor never awaits. |
| R17 Every adaptation is reversible | Every reputation update, router weight change, and learned rule carries a `LearnedRuleRef` with `learned_by`, `learned_at`, `evidence`; the policy engine can roll each back by rule id. |
| R18 Policy governs learning | Learned rule promotion goes through `ApprovalWorkflow`; never auto-promoted. |

### Success criteria

- `aaf-learn` compiles with `cargo build --workspace`.
- `cargo test --workspace` grows by ≥ 15 tests (unit + integration).
- `make ci` stays green.
- The integration test demonstrates: observation → subscriber fans out → miner proposes rule → approval → rule goes live → subsequent intent hits it.
- Every new public item has a `///` doc comment.

### Deferred to E1 Slice C

- `aaf learn` CLI subcommand (list proposals, approve, reject, inspect evidence).
- `make test-semantic-regression` Makefile target.
- Governance docs under `docs/`.
- Production-grade anomaly detection on evidence concentration.

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

## Slice 3 — X1 Slice C — CLI, Ed25519, federation co-sign, SPDX export

### Motivation

X1 Slice A (iteration 6) delivered `aaf-identity` with DID,
keystore, manifest, SBOM, attestation, capability token, and
revocation registry — all with a deterministic HMAC-SHA256
backend. X1 Slice B (iteration 9) wired runtime, trust, and
registry gates. What's missing is **the CLI-facing polish**
that moves the identity story from "contract foundation with
hot-path gates" to "operationally deployable".

### Scope

| Area | What lands |
|---|---|
| **Ed25519 backend swap** | New impl of `Keystore` / `Signer` / `Verifier` using `ed25519-dalek` (or a compatible MSRV-safe crate; the workspace's pinned Rust 1.70 is the constraint). Every call site in `aaf-identity`, `aaf-trust`, and `aaf-runtime` stays unchanged — the swap is trait-only. |
| **Persistent keystore** | `PersistentKeystore` backed by a filesystem directory, one `did.json` per agent. Production deployments point it at their secret manager (HSM / KMS / SPIFFE) through a new `KeyStoreBackend` trait. |
| **`aaf-server identity` CLI expansion** | Already landed in iteration 8 as a stub with `generate`, `revoke`, `sign-manifest`, `verify`, `export-sbom`. Slice C adds: `rotate <did>` (key rotation), `list` (enumerate known DIDs), `issue-token <did> <cap> <expires-at>` (mint a `CapabilityToken` on the CLI), `verify-token <token-json>`, `chain <did1> <did2>` (produce a cosigned delegation). |
| **Federation co-signed tokens** | Already landed in `aaf-federation::cosign`. Slice C wires them into `Router::enforce_capability` so a cross-cell call is rejected unless both cells have signed. |
| **SPDX / CycloneDX export** | Already landed (iteration 8 linter-added). Slice C adds round-trip tests and a schema validation (`spec/schemas/cyclonedx.schema.json`). |
| **ADR** | `docs/adr/ADR-018-ed25519-swap-strategy.md` documenting why the swap is trait-only and how the migration works in rolling upgrade scenarios. |

### Files to touch

- **New**
  - `core/crates/aaf-identity/src/keystore/ed25519.rs` (or a
    sub-module `ed25519_backend.rs`)
  - `core/crates/aaf-identity/src/keystore/persistent.rs`
  - `docs/adr/ADR-018-ed25519-swap-strategy.md`
- **Edit**
  - `core/crates/aaf-identity/src/keystore.rs` (extend trait shape)
  - `core/crates/aaf-identity/Cargo.toml` (add `ed25519-dalek` dep,
    feature-gated)
  - `core/crates/aaf-server/src/identity.rs` (new subcommands)
  - `core/crates/aaf-federation/src/lib.rs` (cosign wiring into
    `Router::enforce_capability`)

### Dependency caveat

The workspace is pinned to **Rust 1.70** (see
`development/build-and-ci.md`). `ed25519-dalek` post-1.0
requires newer `curve25519` versions that do *not* always
compile on 1.70. Two options:

1. **Pin `ed25519-dalek = "1.0"`** (last known 1.70-compatible
   release) and accept the API constraints.
2. **Feature-gate `ed25519`** behind a `--features ed25519` flag
   so the default build stays on HMAC. This is the
   lowest-risk path — the HMAC backend stays the default, and
   production deployments opt into Ed25519 by compiling with the
   feature.

The Slice C iteration entry in `IMPLEMENTATION_PLAN.md` **must**
document which option was chosen and why.

### Unit tests expected

- `aaf-identity::keystore::ed25519::tests` — at least 5:
  - `sign_verify_round_trip`
  - `tampered_message_fails_verify`
  - `wrong_key_fails_verify`
  - `key_rotation_preserves_did`
  - `cross_backend_interop` (sign with HMAC, verify with
    Ed25519 — should fail cleanly, not panic)
- `aaf-identity::keystore::persistent::tests` — at least 3:
  - `round_trip_through_directory`
  - `refuses_world_readable_private_key_file`
  - `list_enumerates_every_known_did`
- `aaf-server::identity::tests` — add coverage for the new
  subcommands.

### Integration test

- `core/tests/integration/tests/x1_slice_c_smoke.rs`:
  - Generate a DID, sign a manifest, export its SBOM as CycloneDX.
  - Validate the CycloneDX JSON against the shipped schema.
  - Issue a capability token on the CLI, verify it on the CLI.
  - Rotate the keystore, verify that tokens signed by the old
    key now fail verification.
  - Wire two `aaf-federation::Router`s; have cell A mint and sign
    a token, cell B cosign it, have `Router::enforce_capability`
    accept a cap that carries the cosigned token.

### Rules preserved

| Rule | How |
|---|---|
| R22 Identity is cryptographic | Ed25519 is the *real* cryptographic backend; HMAC remains as a fallback for tests and the feature-gated default. |
| R23 Signed manifest | `AgentManifest::build` continues to sign at build time, now through Ed25519 when enabled. |
| R24 Provenance as BOM | SPDX / CycloneDX are the external formats. |

### Success criteria

- `ed25519` feature compiles cleanly on Rust 1.70 (or the
  feature is documented as "requires Rust 1.78+"; either is
  acceptable if the ADR says so).
- `aaf-server identity` CLI has all planned subcommands.
- Integration test demonstrates: generate → sign → export SBOM
  (validated against schema) → issue token → cosign cross-cell →
  verify in cell B → rotate → previous-token verification
  fails.
- `make ci` stays green.

### Deferred after X1 Slice C

- Real HSM / KMS / SPIFFE backends (one per target platform).
- ACME-style DID discovery.
- `aaf-server identity watch` for revocation streaming.

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
