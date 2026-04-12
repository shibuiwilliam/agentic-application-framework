# Changing Contract Types — Migration Guide

> How to add a field to `CapabilityContract`, `IntentEnvelope`,
> `PolicyContext`, `Observation`, or any other cross-crate
> contract without breaking the build. This document is the
> positive counterpart to
> [known-gotchas.md → G1 / G2 / G3](known-gotchas.md).

Contract types are the single most rippling change in this
codebase. A new field on `CapabilityContract` touches roughly
twelve files. A new field on `PolicyContext` touches five. A
new field on `IntentEnvelope` touches ~25 test fixtures. Iteration
5 was a dedicated iteration to clean up the fallout from
iteration 4 getting this wrong.

**If you follow the checklist below, your contract change
should land cleanly and leave `cargo test --workspace` green on
the first try.**

---

## Decision tree: should I add a field at all?

```
Can the information be derived from existing fields?
├─ Yes → do not add the field. Compute it where needed.
└─ No  → Is it needed by more than one crate?
         ├─ Yes → add it to aaf-contracts (cross-crate shape).
         └─ No  → add it to the owning crate's internal type.
```

**Default to *not* adding a new field.** Every past iteration
that added a field regretted one of two things: (a) the field
was only needed by one crate, so it should have lived in that
crate's internal type, or (b) the field was redundant with an
existing field and caused drift.

---

## The three-step protocol

Every contract change follows exactly this shape.

### Step 1 — Make the field optional or defaulted

Add the field with `#[serde(default)]` and (when it is a
struct / vec / option) `#[serde(skip_serializing_if = "...")]`.
This makes the field **optional on the wire** so old consumers
can still deserialize new messages (and vice versa during
rolling upgrades).

Example — adding `risk_tier_hint: Option<RiskTier>` to
`IntentEnvelope`:

```rust
// core/crates/aaf-contracts/src/intent.rs
pub struct IntentEnvelope {
    // ... existing fields ...

    /// Optional risk-tier hint supplied by the app-native surface
    /// layer. Iteration N.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub risk_tier_hint: Option<RiskTier>,
}
```

If the field is a `Vec<_>`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub cohort_ids: Vec<String>,
```

If the field is a non-optional primitive, give it a default
function:

```rust
#[serde(default = "default_priority")]
pub priority: u8,

fn default_priority() -> u8 { 5 }
```

### Step 2 — Update every constructor in the tree

This is the step that has bitten every past iteration. Every
crate that constructs the type as a struct literal must be
updated — *the compiler will tell you, but you need to know
where to look*.

Run the grep first:

```bash
# For CapabilityContract
grep -rn "CapabilityContract {" core/crates/ core/tests/

# For IntentEnvelope
grep -rn "IntentEnvelope {" core/crates/ core/tests/

# For PolicyContext — search for a required field that never changes
grep -rn "composed_writes:" core/crates/ core/tests/

# For Artifact
grep -rn "Artifact {" core/crates/ core/tests/

# For Observation
grep -rn "Observation {" core/crates/ core/tests/
```

Expect to touch:

| Contract type | Typical file count |
|---|---|
| `CapabilityContract` | ~12 (every crate that writes a test fixture) |
| `PolicyContext` | 5 (engine / guard / rules × 2 / executor) |
| `IntentEnvelope` | ~25 (every test that builds an intent) |
| `Artifact` | ~6 |
| `Observation` | ~4 |

Patch every site. Do **not** skip the linter's suggestion to
add `..Default::default()` — that trick prevents the compiler
from catching the *next* iteration's additions. Explicit
field-by-field literals are the house style.

### Step 3 — Update the JSON Schema + examples

If the contract has a JSON Schema in `spec/schemas/` (most do),
add the field there too:

```json
{
  "properties": {
    "risk_tier_hint": {
      "type": "string",
      "enum": ["read", "write", "advisory", "delegation", "governance"],
      "description": "Optional risk-tier hint from the app-native surface (iteration N)."
    }
  }
}
```

Then decide whether any shipped example in `spec/examples/`
should carry the new field. Usually the answer is "no" for
optional fields, but if your example is meant to showcase the
new feature, update it and re-run `make schema-validate`.

---

## Extra steps per contract type

### `CapabilityContract`

1. Updating the struct in `aaf-contracts/src/capability.rs`.
2. Twelve constructor sites. The canonical list from iteration
   8's notes in [known-gotchas.md](known-gotchas.md) → G1:
   - `aaf-contracts/src/capability.rs` (tests for `validate`)
   - `aaf-registry/src/store.rs` + `discovery.rs`
   - `aaf-planner/src/planner.rs` + `composition.rs`
   - `aaf-policy/src/engine.rs` + `rules/boundary.rs` +
     `rules/pii.rs` + `rules/injection.rs` + `guard.rs`
   - `aaf-runtime/src/executor.rs` (tests)
   - `aaf-saga/src/*` (tests)
   - `aaf-sidecar/src/*`
   - `aaf-wrapper/src/*`
   - `aaf-federation/src/lib.rs`
   - `aaf-server/src/main.rs`
   - `aaf-trust/src/signing.rs` (fixture)
   - `core/tests/integration/tests/*` — every smoke test
3. Run `make ontology-lint`. If you added a field that affects
   entity declarations, update the shipped examples in
   `spec/examples/capability-*.yaml`.
4. Update `development/contracts-reference.md` →
   `CapabilityContract` section.

### `IntentEnvelope`

1. Updating the struct in `aaf-contracts/src/intent.rs`.
2. If the field interacts with Rule 8 (depth / budget) or
   validation, update `IntentEnvelope::validate`.
3. ~25 test fixture sites. Grep for `IntentEnvelope {`.
4. If the field flows into the planner's cache key, update
   `aaf-planner::cache::key_for` so caching still behaves
   correctly.
5. Update `development/contracts-reference.md` →
   `IntentEnvelope` section.

### `PolicyContext`

1. Updating the struct in `aaf-policy/src/context.rs`.
2. Five construction sites found via
   `grep -rn "composed_writes:" core/`.
3. If the field is a **callback** (like
   `ontology_class_lookup`), decide whether it must be
   populated or can default to `None`. Most callbacks default
   to `None` for back-compat; write this into the struct's
   doc comment so future authors know.
4. Update `development/crate-reference.md` →
   `aaf-policy` section.

### `Artifact`

1. Updating the struct in `aaf-contracts/src/artifact.rs`.
2. ~6 constructor sites.
3. If the field affects the signature shape, you also need to
   update `aaf-trust::signing::sign_artifact_with`.

### `Observation` / `Outcome`

1. Updating the struct in `aaf-contracts/src/observation.rs`.
2. If you added a field to `Outcome`, update the `minimal`
   constructor so the runtime's default minimal outcome
   populates a sensible value.
3. If the field flows into OTel export, update
   `aaf-trace::export::otel_json_for` to emit the new
   attribute.
4. Update `development/contracts-reference.md` →
   `Observation` section.

---

## Renaming a field (don't)

Do not rename a field in place. The correct procedure is:

1. Add the new field with `#[serde(default)]` (step 1 above).
2. Add a `From` / transition helper that reads the old field
   and populates the new one.
3. Update consumers to read the new field.
4. After one full release, remove the old field (step 1 again,
   in reverse).

Renaming in place breaks rolling upgrades because a newer
writer will emit the new name while an older reader still
looks for the old name.

---

## Removing a field (even less)

Same rule. Mark deprecated first, migrate consumers, remove
after one full release.

The exceptions are **private** fields (never part of the
serialised shape) and **fields that never shipped** (added and
removed in the same iteration). Those can be removed directly.

---

## Testing the change

After the contract change compiles:

1. `cargo build --workspace` — must be clean.
2. `cargo test --workspace` — must stay at-or-above the
   previous test count.
3. `cargo clippy --workspace --all-targets -- -W clippy::all`
   — zero warnings.
4. `make schema-validate` — any updated schema must validate
   against its example.
5. `make ontology-lint` — no regression in adoption ratio.

Every past iteration that touched a contract followed this
checklist. Iterations that skipped any of these (iteration 4
skipped step 2, iteration 7 skipped step 4) got caught by the
next iteration's audit.

---

## A worked example — iteration 7 added `entities_in_context`

Iteration 7 extended `IntentEnvelope` with
`entities_in_context: Vec<EntityRefLite>`. The steps the
iteration actually followed:

1. **Struct update** — added the field in
   `aaf-contracts/src/intent.rs` with
   `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
2. **Constructor updates** — ran
   `grep -rn "IntentEnvelope {" core/` and patched every site.
   Test fixtures got `entities_in_context: vec![]`.
3. **Schema update** — added `entities_in_context` to
   `spec/schemas/intent-envelope.schema.json` as an optional
   array of `EntityRefLite` objects.
4. **Example update** — none. The shipped
   `spec/examples/*` files are not IntentEnvelope examples.
5. **Downstream consumer** — `aaf-intent::Enricher::enrich_with_ontology`
   populates the new field from an `OntologyResolver`.
6. **Tests** —
   `aaf-intent::enricher::tests::enrich_with_ontology_populates_entities_and_attaches_tenant`
   plus the E2 Slice B integration smoke test.
7. **Docs** — updated `development/contracts-reference.md` →
   `IntentEnvelope` section.
8. **Validation** — five gates green.

Total files touched: ~30. Time spent on step 2 alone: ~40%
of the iteration. **This is the cost of a contract change.**
Budget for it.

---

## When *not* to touch contracts

- **If the change is localised.** Add a helper / context struct
  in the owning crate instead.
- **If the change is experimental.** Gate it behind a feature
  flag in the owning crate first.
- **If the change is driven by a single consumer.** Have the
  consumer compute the value instead.
- **If the change is for a single slice.** Slice A of a new
  enhancement *can* add contract fields because the whole
  point of Slice A is to establish the contract surface. Slice
  B and C should mostly extend existing fields, not add new
  ones.

---

## Further reading

- [contracts-reference.md](contracts-reference.md) — the
  current contract surface
- [known-gotchas.md](known-gotchas.md) — G1, G2, G3 describe
  the pain this guide is designed to prevent
- [coding-conventions.md](coding-conventions.md) →
  "Test fixtures" — why explicit field literals are the
  house style
- `core/crates/aaf-contracts/src/` — the contracts themselves
