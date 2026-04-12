# Testing Strategy

> How tests are organised in this workspace, and how to pick the
> right place for a new test. Every iteration in
> `IMPLEMENTATION_PLAN.md` has followed this strategy — please
> continue doing so.

Current total: **463 tests passing across 22 crates, 0 failures**.

---

## Three layers

AAF tests live in exactly three places.

### Layer 1 — Unit tests inside each crate

**Location:** `core/crates/aaf-*/src/**/*.rs`, inside
`#[cfg(test)] mod tests { … }` blocks at the bottom of each file.

**Use for:**

- Invariant checks on a single type (e.g. `IntentEnvelope::validate`).
- Behaviour of a single function or trait impl.
- Failure-mode tests that require nothing but the crate's own types
  and an in-memory store.

**Rules:**

- Test fixtures should be tiny — construct structs inline via a
  `fn sample_*() -> T` helper at the top of the `tests` module.
- No dependencies on other `aaf-*` crates that are not already
  in `[dependencies]` (tests inherit the crate's dep graph).
- Prefer `#[tokio::test]` for any test that touches an async trait.
- If the assertion is boolean, prefer `assert!(expr)` /
  `assert!(!expr)` over `assert_eq!(expr, true|false)` — clippy
  flags the latter at `-W clippy::all`.

**Example:** `core/crates/aaf-contracts/src/capability.rs` has three
unit tests covering `CapabilityContract::validate`'s Rule 9
compensation gate (happy path + reject-missing + accept-with-comp).

### Layer 2 — Cross-crate integration tests

**Location:** `core/tests/integration/tests/*.rs`, inside the
`aaf-integration-tests` workspace crate.

**Use for:**

- Testing that *two or more* crates interoperate correctly.
- End-to-end pipeline tests that thread Intent → Plan → Runtime →
  Trace.
- Smoke tests for a new Slice that touches multiple hot-path crates.

**Conventions:**

- One test file per Slice / per feature, named after the enhancement
  and the slice:
  - `e1_feedback_smoke.rs` — E1 Slice A
  - `e2_ontology_smoke.rs` — E2 Slice A
  - `e2_slice_b_smoke.rs` — E2 Slice B
  - `e2_slice_c_smoke.rs` — E2 Slice C
  - `e3_surface_smoke.rs` — E3 Slice A
  - `x1_identity_smoke.rs` — X1 Slice A
  - `x1_slice_b_integration.rs` — X1 Slice B
  - `x1_slice_c_cli.rs` — X1 Slice C
  - `full_pipeline.rs` — cross-cutting end-to-end
  - `aafservice_ecommerce.rs` — canonical e-commerce story
  - `e1_slice_b_smoke.rs` — E1 Slice B (learning subscribers)
  - `order_saga_e2e.rs` — order-saga example (saga/compensation)
  - `resilient_query_e2e.rs` — resilient-query example (guards/degradation)
  - `feedback_loop_e2e.rs` — feedback-loop example (trust/learning)
  - `memory_context_e2e.rs` — memory-context example (4-layer memory)
  - `app_native_surface_e2e.rs` — app-native-surface example (events/proposals)
  - `cross_cell_federation_e2e.rs` — cross-cell-federation example
- Each file exposes **one or two** `#[tokio::test]` or `#[test]`
  functions whose bodies walk a clear narrative sequence (numbered
  `// ── N. … ──────` section comments are the house style).
- Helper constructors (`fn cap(id)`, `fn sample_intent(t)`) live at
  the top of the file — the dep graph is already broad so this does
  not hurt.

**Adding a dependency:** every crate you need must be in
`core/tests/integration/Cargo.toml`'s `[dev-dependencies]`. As of
iteration 8 that list includes every hot-path crate plus
`aaf-federation`, `aaf-memory`, `aaf-ontology`, `aaf-identity`,
`aaf-eval`, `aaf-surface`.

### Layer 3 — Spec validation

**Location:** `scripts/schema_validate.py`, invoked by
`make schema-validate`.

**Use for:**

- Every YAML in `spec/examples/` validates against its JSON Schema.
- New JSON Schemas go in `spec/schemas/`.
- New examples should either (a) validate under an existing schema
  or (b) add a new schema and a new prefix mapping in
  `schema_validate.py`.

**Current status:** 9/9 examples validate. Two examples
(`manifest-order-agent.yaml`, `sbom-order-agent.yaml`) are
intentionally unmapped — they are consumed by
`aaf-identity::manifest::from_yaml` directly, not by a schema.

---

## The new: `make ontology-lint`

**Location:** `core/crates/aaf-server/src/lint.rs` +
`aaf-server ontology lint <dir>` subcommand +
`make ontology-lint` target (iteration 8 E2 Slice C).

**What it catches:**

- Any `capability-*.yaml` under `spec/examples/` whose `side_effect`
  is `Write` / `Delete` / `Send` / `Payment` but whose `writes:`
  is empty.
- Any capability whose `reads:` / `writes:` / `emits:` are all
  empty, even on read-only side effects (reported as `Warn`).

**Adoption ratio ramp:**

- `< 90%` of scanned capabilities carry declarations → warn-only
  mode (CI does not fail on warnings).
- `≥ 90%` → strict mode (writers missing `writes:` are
  `Severity::Error` and the command exits non-zero).

**As of iteration 8:** `spec/examples/` has 2 capability YAMLs, both
carry entity declarations, so adoption is 100% and the lint runs in
strict mode with 0 errors.

---

## How to pick a layer for a new test

Flowchart (Claude Code should literally walk this):

```
Does the test touch only one crate's public surface?
├─ Yes → Unit test inside that crate's src/*.rs
└─ No  → Does it exercise a new Slice's end-to-end story?
         ├─ Yes → Integration test under core/tests/integration/tests/
         └─ No  → Does it cross two crates but the story is
                  about just one of them?
                  ├─ Yes → Unit test in the "owning" crate,
                  │        importing the other as a dev-dep
                  └─ No  → Integration test (default)
```

**Rule of thumb:** if you have to say "integration test" because the
unit-test doc you wrote needed three sentences of context to
explain the setup, it belongs in the integration tests.

---

## Coverage expectations per slice

| Slice | Unit tests required | Integration test required |
|---|---|---|
| A (contracts + skeleton) | ≥ 10 per new crate | none |
| B (integration into hot path) | ≥ 2 per hot-path crate touched | one new file named `{enhancement}_slice_b_smoke.rs` |
| C (SDK / examples / polish) | ≥ 1 per new tool | one new file named `{enhancement}_slice_c_smoke.rs` |

Every iteration has landed at or above this expectation.

---

## Running the full suite

From the repo root:

```bash
cargo test --workspace                              # every unit + integration test
cargo test --workspace -p aaf-planner               # only aaf-planner's tests
cargo test -p aaf-integration-tests --test e2_slice_b_smoke
cargo test --workspace -- --nocapture               # print test output (useful when debugging)
```

Or via `make`:

```bash
make test          # cargo test --workspace
make test-quiet    # same, minimal output
make test-doc      # cargo test --workspace --doc (always 0 as of iter 8)
make ci            # the full PR gate: fmt-check + clippy + test + schema-validate + ontology-lint
```

---

## Writing tests under clippy's `-W clippy::all`

Iteration 5 cleaned every clippy warning; iteration 8 kept the bar.
A few patterns to remember:

- `assert!(x)` / `assert!(!x)` instead of `assert_eq!(x, true|false)`.
- Don't `borrow-then-deref` (`&path.as_path()` where a plain `&path`
  is already a `&Path`).
- Don't `format!("literal\n")` when a plain `"literal\n"` works.
- Don't `.clone()` a value you are about to move.

If clippy fires on a new test you wrote, fix the test — do *not*
pepper `#[allow(clippy::foo)]` attributes. Every previous iteration
has kept the tree warning-free under `-W clippy::all`.

---

## Invariants tests must preserve

Every PR that touches any of these must also add a test that still
passes after the PR:

- **Rule 9 (compensation).** `CapabilityContract::validate` rejects a
  write cap without a compensation — there is a test in
  `aaf-contracts::capability::tests` and a test in
  `aaf-registry::store::tests`.
- **Rule 6 (policy at every step).** `aaf-runtime::tests` asserts
  that the executor calls the policy engine at every hook.
- **Rule 8 (depth/budget).** `aaf-runtime::budget::tests` exercises
  exhaustion (and a deterministic cost-based test — iteration 2
  replaced a timing-fragile version).
- **Rule 14 (semantics are nouns).** The E2 Slice B smoke test
  (`e2_slice_b_smoke.rs`) proves the ontology reaches every hot
  crate. Changes to planner / policy / memory / intent / registry
  must not regress this.
- **Rule 20 (proposals, not mutations).** `aaf-surface` constructor
  tests for `ActionProposal::new_with_mutations` — every path that
  builds a proposal must preserve the "mutations require
  compensation_ref" invariant.
- **Rule 22 (cryptographic identity).** `aaf-identity::manifest::tests`
  + `aaf-identity::delegation::tests` must remain green; the X1
  Slice B integration test exercises them end-to-end.
