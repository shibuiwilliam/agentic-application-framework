# Iteration Playbook

> The repeatable recipe every iteration in `IMPLEMENTATION_PLAN.md`
> has followed. If you are about to land a new Slice, start here.

---

## The seven-step cycle

This is the cycle the user asked for at the start of iterations 7
and 8, and every iteration before that followed the same shape:

> 1. Understand the project deeply and carefully.
> 2. Plan implementation with priorities.
> 3. Document the plan.
> 4. Implement.
> 5. Review, test, and validate.
> 6. Fix.
> 7. Repeat.

Each step has a concrete meaning in this tree.

### Step 1 — Understand

- Read `PROJECT.md` §§ relevant to the slice.
- Read `CLAUDE.md` for the rules that apply.
- Read `PROJECT.md` §§16-18 and `CLAUDE.md` rules 14-24 if the
  slice is in an enhancement program (E1/E2/E3/X1/X2/X3).
- Read `IMPLEMENTATION_PLAN.md` to see what has already landed and
  what is deferred.
- Skim `development/crate-reference.md` to confirm which crates own
  what you need to change.
- **Run `cargo build && cargo test && cargo clippy && make schema-validate && make ontology-lint`
  before you edit anything.** If the tree is not green, stop and
  stabilise it first — do not build on top of a broken base.

### Step 2 — Plan

- Break the work into the smallest unit that is shippable.
  Prefer A → B → C slice ordering; do not leapfrog.
- Identify every crate that has to change.
- Identify every test that must be added (unit tests first,
  integration smoke test last).
- Identify every invariant that must stay green (see
  `development/testing-strategy.md` → "Invariants tests must
  preserve").
- **Write down** the plan as concrete task items. Every iteration
  used `TaskCreate` to make the plan visible.

### Step 3 — Document the plan

- Add a new "Iteration N — <title>" section to
  `IMPLEMENTATION_PLAN.md`.
- Use the same section structure as iterations 4–8:
  - **Motivation** — why now, what gap does this close
  - **Scope of iteration N** — table of `Area → What lands`
  - **Rules enforcement** — which rules this slice tightens
  - **Public-API back-compatibility** — what stays additive,
    what migrates
  - **Validation approach** — the tests that prove the slice works
  - **Deferred after iteration N** — what explicitly remains
- If the change is architectural, write an ADR under `docs/adr/`
  (copy the structure of `ADR-008-entity-space-boundaries.md`).

### Step 4 — Implement

- Work **one task at a time**. Mark the task `in_progress` when
  you start, `completed` when the tests for that task pass.
- Keep commits (or edits, if you are not using git) small and
  focused. One conceptual change per commit.
- Every new public item needs a `///` doc comment. Every new error
  variant needs a `#[error(...)]` message.
- If you add a `PolicyContext { … }` struct literal you *must*
  update every construction site in the tree — there is an
  explicit test for this and the build fails otherwise.
- If you widen a trait method, provide a default impl so
  downstream crates keep compiling. `LongTermMemoryStore::search_by_entity`
  is the canonical example.

### Step 5 — Review, test, validate

Walk the five gates **in order** every time:

```bash
cargo build --workspace
cargo test --workspace
cargo clippy --workspace --all-targets -- -W clippy::all
python3 scripts/schema_validate.py --schema-dir spec/schemas --examples-dir spec/examples
make ontology-lint
```

- Build breaks? Fix the missing field or the trait impl mismatch.
- Test fails? Walk the failure; do not paper over it.
- Clippy fires? Fix the pattern; do not `#[allow]` unless you have
  written an ADR for why the suggestion is wrong.
- Schema fails? Either (a) add a schema, (b) add a prefix mapping in
  `scripts/schema_validate.py`, or (c) rename the example file.
- Ontology lint fails? Add the missing entity declarations — that
  is the whole point of the tool.

### Step 6 — Fix

- If you found a bug while testing, **write a regression test
  first**, then fix. Iteration 2 found 4 bugs and each has a
  regression test; iteration 3 found 1 bug and has a regression test;
  iteration 5 found 5 structural issues and has regression tests for
  each. The discipline holds.
- If the fix reveals a scope change, update
  `IMPLEMENTATION_PLAN.md`.

### Step 7 — Repeat

- When the last task is completed, write the iteration's **totals**
  table into `IMPLEMENTATION_PLAN.md` (test count, crate count,
  clippy status, schema status, adoption ratios).
- Summarise the iteration in one paragraph at the end of the plan's
  iteration section.
- Identify what the next iteration should pick up, and record it in
  `## Next iterations (not in this PR)`.

---

## Slice ordering

Per `PROJECT.md` §18 and `CLAUDE.md`, enhancements land in a strict order:

1. **E2 → E1 → E3** (Wave 1)
2. **X1 → X2 → X3** (Wave 2)

Within each enhancement, three slices:

| Slice | Scope | Criteria |
|---|---|---|
| **A** | Contracts + crate skeleton + in-memory impl + unit tests | Every new type exists and has a unit test |
| **B** | Integration into the hot-path crates | Every hot-path crate consumes the new contracts; a smoke test proves end-to-end |
| **C** | SDK primitives, examples, CLIs, tooling, polish | A developer can use the new capability from outside the core crates |

**Do not leapfrog.** If you are tempted to land a Slice C SDK for
an enhancement whose Slice B is not done, stop — the SDK will need
a rewrite once the hot-path integration lands.

Current status:

| Enhancement | A | B | C |
|---|---|---|---|
| E2 Domain Ontology | ✓ iter 4 | ✓ iter 7 | ✓ iter 8 |
| E1 Feedback Spine | ✓ iter 4 | ✓ | **next** (iter 11) |
| E3 App-Native Surface | ✓ iter 4 | deferred (iter 12) | deferred |
| X1 Agent Identity | ✓ iter 6 | ✓ iter 9 | ✓ iter 10 |
| X2 Knowledge Fabric | deferred | deferred | deferred |
| X3 DX Surface | deferred | deferred | deferred |

**Next target** is E1 Slice C: the `aaf learn` CLI subcommand,
`make test-semantic-regression` target, and governance docs for
the learning pipeline. See `development/next-slices.md`.

---

## Anti-patterns seen in past iterations

Every one of these has burned at least one iteration. Avoid them.

1. **Depending on `aaf-ontology` from a tier-2 crate.** The policy
   engine and the planner both need *classification information*
   but they must not import `aaf-ontology` directly — the crate
   would pull a full dependency tree. Instead, pass an
   `Option<Lookup>` callback through the crate's public API (see
   `aaf-policy::context::OntologyClassificationLookup` and
   `aaf-planner::composition::ClassificationLookup`).
2. **Depending on `aaf-server` from a crate that is not `aaf-server`.**
   `aaf-server` is the binary — everything else must not depend on
   it. The `lint` and `import` modules live inside `aaf-server` for
   that reason; if they were needed elsewhere, they would move to a
   dedicated library crate.
3. **Constructing `PolicyContext` without `ontology_class_lookup`.**
   Every field on `PolicyContext` is required; the compiler enforces
   this. Iteration 8 had to patch five construction sites when the
   field was added. A grep for `composed_writes:` finds every site.
4. **Adding a field to `CapabilityContract` without updating test
   fixtures.** Every crate that constructs a `CapabilityContract`
   literal in its tests has to add the new field. The compiler
   will catch this but it is tedious. Iteration 4 missed twelve
   construction sites and iteration 5 fixed them. Today the
   canonical list is in `development/known-gotchas.md`.
5. **Changing an enum variant order silently.** Clippy pedantic
   flags some of these; code that reads the numeric discriminant
   breaks silently. If you must change a variant order, bump the
   enum's doc comment to note it.
6. **Making `aaf-federation` or `aaf-policy` depend on
   `aaf-registry`.** The dep already exists in the other direction.
   Invert the ask: if `aaf-policy` needs registry lookups, accept a
   closure in the `PolicyEngine` constructor.

---

## Iteration log (quick reference)

| Iter | What landed | Tests after |
|---|---|---|
| 1 | 17 core crates | 98 |
| 2 | Multi-step planner, saga/runtime bridge, approval node integration, full-pipeline test, 4 bugs fixed | 111 |
| 3 | Compensation chain wired, OTel export, cost attribution, A2A import, 5 schemas, 2 examples, base policy files, server subcommands, hello-agent example, 1 bug fixed | 117 |
| 4 | E2/E1/E3 Slice A (`aaf-ontology`, `aaf-eval`, `aaf-surface`) | 117* |
| 5 | Audit + fix-forward: 12 contract literal updates, task state extensions, clippy cleanup, schema fixes | 177 |
| 6 | X1 Slice A (`aaf-identity` crate + contracts + 4 schemas + 2 examples) | 177 |
| 7 | E2 Slice B: ontology wired into planner/policy/memory/intent/registry | 236 |
| 8 | E2 Slice C: federation in entity-space + `ontology lint` + `ontology import` + ADR-008 | 268 |
| 9 | X1 Slice B: runtime revocation gate + trust token verify + registry attestation gate + DID-bound signing | (counted inside the 268 above) |

\* iteration 4 headline was aspirational — test count was not
re-verified; iteration 5 stabilised it at 177.

---

## Anatomy of a healthy iteration PR

A good PR in this tree always has:

- One "Iteration N — …" section in `IMPLEMENTATION_PLAN.md` with
  motivation, scope, rules enforced, back-compat story, validation
  approach, and deferred items.
- One "Iteration N totals" table with the 5-gate status.
- At least one new integration test under
  `core/tests/integration/tests/`.
- Zero new clippy warnings (`-W clippy::all`).
- Zero regressions in unit test counts.
- 9/9 examples still validate.
- A one-line mention in the `Next iterations` list at the end of
  the plan of what iteration N+1 should pick up.

Every iteration from 4 onward has met all seven points.
