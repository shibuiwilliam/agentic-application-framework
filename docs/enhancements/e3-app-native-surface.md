# E3 — Application-Native Surface

> **Status:** Slice A complete (iter 4). Slice B pending.
> Slice C pending.
>
> **Rules enforced:** R19 Projections default-deny. R20 Proposals,
> not mutations.
>
> **Authoritative design:** `PROJECT.md` §16.3 + `CLAUDE.md` rules
> 19-20. This page is a reader-facing summary.

---

## The problem

Before E3, AAF was still *a backend*. `PROJECT.md` §2.4 promised
a Front Door — a human entry point — but even once built, it is a
**single chat surface**. It does not describe how an application
*natively* emits events that become intents, how the agent
returns typed action proposals the app renders inline, or how
application state flows into and out of the agent as a
first-class channel.

Without those primitives, AAF integrates into an application
the way a chatbot does — alongside it, not inside it. The
`PROJECT.md` promise ("AI agents serve as the universal
interface between humans, applications, services, and APIs")
currently covers only **services and APIs**. E3 closes the gap
by making AAF first-class to the app layer itself.

---

## The solution — five design principles

- **S1. Any signal can become an Intent.** Not just text. A
  click, a page view, a cron, a webhook, a state change, a file
  drop — all are first-class Intent sources, each with a
  situation package.
- **S2. The app owns the UI, the agent owns the proposal.**
  Agents return *what should happen and why*, not fully-rendered
  screens. The app renders proposals using its own design system.
- **S3. State flows both ways, but authority does not.** The
  agent reads projections and *proposes* mutations; the app
  always has the final word. Every proposed mutation is
  reversible by construction through the saga engine.
- **S4. Integration should feel like a feature-flag SDK.** An
  app developer adds AAF by decorating functions, emitting
  events, and mounting a proposal component — not by standing
  up a parallel orchestrator.
- **S5. Native does not mean invasive.** Services (Rule 3) still
  stay untouched. The app-native surface sits at the *application
  composition* layer where the app glues its services, state,
  and UI together — not inside the services themselves.

---

## What landed

### Slice A — contracts + crate skeleton (iteration 4)

| Deliverable | Location |
|---|---|
| `aaf-surface` crate | `core/crates/aaf-surface/` |
| `AppEvent`, `EventType`, `Situation`, `ScreenContext`, `SessionContext` | `aaf-surface::event` |
| `SurfaceConstraints` | `aaf-surface::event` |
| `EventToIntentAdapter` trait + `RuleBasedAdapter` | `aaf-surface::ingest` |
| `ActionProposal` with **Rule 20 construction-time enforcement** | `aaf-surface::proposal::ActionProposal::new_with_mutations` |
| `StateMutationProposal` | `aaf-surface::proposal` |
| `StateProjection` with **Rule 19 default-deny enforcement** | `aaf-surface::projection::StateProjection::allows_field` |
| `ProposalLifecycle` state machine | `aaf-surface::proposal` |
| `SituationPackager` (~7,500 token budget) | `aaf-surface::situation_packager` |
| Task state extensions for proposal lifecycle | `aaf-contracts::task::TaskState::{ProposedMutation, AppReview, Accepted, Rejected, Transformed, Expired}` |
| `app-event-order-opened.yaml` example | `spec/examples/` |
| `proposal-shipping-fix.yaml` example | `spec/examples/` |
| Smoke test | `core/tests/integration/tests/e3_surface_smoke.rs` |

### Slice B — EventGateway + surface-keyed thread memory (deferred)

Per the plan in [`../../development/next-slices.md`](../../development/next-slices.md):

| Deliverable | Location |
|---|---|
| `EventGateway` in sidecar + wrapper | `aaf-sidecar::gateway`, `aaf-wrapper::gateway` |
| Idempotent event dedup (via event id) | `aaf-sidecar::gateway` |
| Per-surface rate limits + per-tenant budgets | `aaf-sidecar::gateway` |
| Surface-keyed thread memory | `aaf-memory::facade::thread_id_for_surface(user, tenant, surface)` |
| Action guard extension for `StateMutationProposal` | `aaf-policy::guard::action::ActionGuard::check_mutation_proposal` |
| Proposal outcomes flow into trace | `Observation.outcome_detail.proposal_outcome` variant |
| Saga wiring for accept/reject/transform | `aaf-saga::executor` |
| Smoke test | `core/tests/integration/tests/e3_slice_b_smoke.rs` |

### Slice C — SDK primitives + reference app (deferred)

- Python / TypeScript SDK decorators:
  `@on_event("order.page.opened")`, `@project("Order")`,
  `@accept_proposal`
- React `<AgentProposal proposalId={…} />` component wired to
  the Wave 1 `ActionProposal` contract
- `<AgentSurface for="Order" />` opt-in ambient surface
- `examples/app-native/` reference application
- WebSocket intent-channel client in every SDK

---

## The contract surface

```
AppEvent {
    event_id,                       // idempotency key
    event_type,
    source { app_id, surface },
    situation: Situation,
    payload: StructuredPayload,     // typed via EntityRefLite
    user_id, session_id, timestamp,
    trace_id,                        // already linked to AAF trace
}

Situation {
    current_entities: [EntityRefLite],
    current_screen { route, component, visible_fields[] },
    session { user_id, role, scopes[], locale, tenant_id },
    constraints { time_budget_ms?, cost_budget_usd? },
}

ActionProposal {
    proposal_id, intent_id, trace_id,
    summary, rationale,
    mutations: [StateMutationProposal],
    artifacts: [ArtifactRef],
    ui_hints { kind: diff|form|card|banner, priority, dismissable },
    compensation_ref: CapabilityRef,   // REQUIRED for any mutation
    expires_at,
    approval_state,
}

StateMutationProposal {
    entity_ref, field_path, from_value, to_value,
    preview_renderer_hint,
    reversible: bool,
    compensation_ref: CapabilityRef,
}

StateProjection {
    projection_id, root_entity: EntityRef,
    selected_fields[],                 // subject to classification rules
    freshness_ms,
    policy_scope: [PolicyRuleRef],
}
```

---

## Construction-time enforcement

### Rule 20 — Proposals, not mutations

`ActionProposal::new_with_mutations(mutations, compensation_ref)`
returns `Err` if `mutations` is non-empty without a
`compensation_ref`. There is no way to construct an
`ActionProposal` with unreferenced mutations. The compiler
enforces this at every call site.

### Rule 19 — Projections default-deny

`StateProjection::allows_field(field_name)` defaults to
`false`. A projection only exposes fields that are
**explicitly** listed in `selected_fields`. Omissions are not
"allowed by default" — they are "denied by default".

---

## Task state machine extensions

```
... → proposed_mutation → app_review
           ↓                  ↓
           ↓           accepted → running → completed
           ↓           rejected → cancelled
           ↓           transformed → running (with edited mutation)
           ↓
         expired → cancelled
```

Every `app_review → accepted` transition writes a `Handoff`
and wires the compensation of the underlying capability into
the saga so rollbacks are a single call.

---

## Rules

| Rule | How E3 enforces it |
|---|---|
| **R19** Projections default-deny | `StateProjection::allows_field` returns `false` unless the field is in `selected_fields` |
| **R20** Proposals, not mutations | `ActionProposal::new_with_mutations` construction-time check |
| **R3** Services stay untouched | E3 sits at the *application composition* layer, not inside services |

---

## Safety rails

- **No implicit mutation.** The surface crate refuses to produce an
  `ActionProposal` whose `mutations[]` lacks a declared
  `compensation_ref`.
- **Projection classification enforcement.** A `StateProjection`
  cannot expose fields whose entity classification exceeds the
  requesting agent's trust level and scopes.
- **Rate limits per surface.** A single surface cannot flood the
  runtime with events (enforced by the Slice B `EventGateway`).
- **Replay safety.** Every `AppEvent` is idempotent via
  `event_id` so retries cannot double-trigger.

---

## Success criteria (once Slice B + C ship)

- An existing application can integrate AAF with three
  primitives: `@on_event`, `@project`, and `<AgentProposal/>`.
  No other wiring.
- The reference application under `examples/app-native/`
  demonstrates end-to-end: user opens an Order page → AAF
  receives an event with the Order situation → agent proposes
  a mutation → the app renders the proposal inline → user
  accepts → saga executes → trace is visible in the Trace
  Explorer → outcome flows back into E1.
- Every proposal is reversible by construction (Rule 9).

---

## Further reading

- [`../../development/next-slices.md`](../../development/next-slices.md)
  → Slice 2 — E3 Slice B
- [`../../development/crate-reference.md`](../../development/crate-reference.md)
  → `aaf-surface` section
- `PROJECT.md` §16.3 — the authoritative design
- `core/crates/aaf-surface/src/` — the Slice A implementation
- `spec/examples/app-event-order-opened.yaml` — canonical AppEvent
- `spec/examples/proposal-shipping-fix.yaml` — canonical ActionProposal
