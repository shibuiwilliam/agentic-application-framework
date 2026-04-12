# app-native-surface

Demonstrates AAF's **app-native surface layer** -- how existing
applications integrate with AAF through events, proposals, and
projections without surrendering authority over their own state.

This is the "bridge" example: it shows the boundary between an
application (an ops dashboard for order management) and the AAF agent
layer. Events flow in, proposals flow out, the app retains authority.

## What this example covers

### Event Routing & Adaptation

| Feature | Where it's exercised |
|---|---|
| **Event routing** | `EventRouter` classifies events into FastPath (structured payload), AgentInterpret (ambiguous), or Composite (batch) |
| **Batch decomposition** | Array payloads with 2+ items decompose into parallel sub-events, each with its own idempotency key |
| **Event-to-intent adaptation** | `RuleBasedAdapter` maps event types to intent types (e.g. `order.cancel.requested` -> `TransactionalIntent`) |
| **Unknown event handling** | Unmapped event types return `None` (dropped, not errored) |
| **Budget overrides** | Surface constraints (`time_budget_ms`, `cost_budget_usd`) override the default intent budget |

### Proposals & Lifecycle (Rule 20)

| Feature | Where it's exercised |
|---|---|
| **Rule 20 enforcement** | `ActionProposal::build()` rejects mutations without a `compensation_ref` at construction time |
| **Compensation-free proposals** | Informational proposals (no mutations) need no compensation |
| **Accept lifecycle** | Draft -> Proposed -> AppReview -> Accepted |
| **Reject lifecycle** | Draft -> Proposed -> AppReview -> Rejected |
| **Transform lifecycle** | Draft -> Proposed -> AppReview -> Transformed (user edits) |
| **Expire lifecycle** | Draft -> Proposed -> AppReview -> Expired (TTL timeout) |
| **Illegal transitions** | Accept-before-publish and double-accept are rejected |

### Projections (Rule 19)

| Feature | Where it's exercised |
|---|---|
| **Default-deny fields** | `StateProjection` only exposes explicitly listed fields |
| **Field reading** | `read_field()` returns the value for allowed fields, error for denied |
| **Cross-tenant isolation** | `check_tenant()` rejects access from a different tenant |

### Situation Packager

| Feature | Where it's exercised |
|---|---|
| **Entity forwarding** | Current entities from the situation are forwarded to the intent's `entities_in_context` |
| **Budget enforcement** | Oversized field lists exceed the context token budget |

## Files

```
examples/app-native-surface/
├── README.md       <- this file
└── aaf.yaml        <- capabilities, event rules, projections
```

## Run the tests

```bash
cargo test -p aaf-integration-tests --test app_native_surface_e2e
```

Expected output:

```text
running 23 tests
test structured_event_routes_to_fast_path ... ok
test fast_prefix_routes_to_fast_path ... ok
test ambiguous_event_routes_to_agent_interpret ... ok
test batch_event_decomposes_into_sub_events ... ok
test adapter_converts_known_event_to_intent ... ok
test adapter_drops_unknown_event ... ok
test surface_constraints_override_budget ... ok
test rule_20_rejects_mutations_without_compensation ... ok
test empty_mutations_need_no_compensation ... ok
test lifecycle_accept ... ok
test lifecycle_reject ... ok
test lifecycle_transform ... ok
test lifecycle_expire ... ok
test illegal_transition_rejected ... ok
test cannot_accept_twice ... ok
test projection_allows_listed_fields ... ok
test projection_denies_unlisted_fields ... ok
test projection_rejects_cross_tenant ... ok
test packager_forwards_entity_refs ... ok
test packager_packages_screen_fields ... ok
test packager_rejects_oversized_fields ... ok
test event_to_intent_to_execution_pipeline ... ok
test aaf_yaml_loads_successfully ... ok

test result: ok. 23 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Architecture rules exercised

| Rule | How |
|---|---|
| **Rule 19** (Projections Default-Deny) | `StateProjection` only exposes explicitly listed fields; all others denied |
| **Rule 20** (Proposals, Not Mutations) | `ActionProposal::build()` enforces compensation for every mutation at construction time |
| **Rule 10** (Context Minimization) | `SituationPackager` budget-caps the screen field list to stay within the ~7,500 token window |

## The story

1. A user opens an order page on the ops dashboard.
2. The dashboard emits an `AppEvent` with type `order.page.opened`.
3. The `EventRouter` classifies it (FastPath for structured, AgentInterpret for ambiguous).
4. The `RuleBasedAdapter` converts it to an `AnalyticalIntent`.
5. AAF executes the intent, retrieves order data, and builds an `ActionProposal`.
6. The proposal carries mutations (status: pending -> cancelled) and a compensation ref (cap-order-reopen).
7. The `ProposalLifecycle` publishes the proposal to the app for review.
8. The user accepts, rejects, transforms, or lets it expire.
9. A `StateProjection` controls which order fields the agent can see (Rule 19 default-deny).

## See also

- `CLAUDE.md` -- Rule 19 (Projections Default-Deny) and Rule 20 (Proposals, Not Mutations)
- `examples/hello-agent/` -- simplest AAF pipeline (read-only)
- `examples/order-saga/` -- multi-step saga with compensation
- `examples/resilient-query/` -- guards, degradation, budget
- `examples/feedback-loop/` -- trust lifecycle + learning
- `examples/signed-agent/` -- identity + provenance CLI walkthrough
