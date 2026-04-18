# Known Gotchas

> Things that have bitten at least one previous iteration. Read this
> file *before* you make a non-trivial change. Every entry below is
> load-bearing knowledge the compiler cannot give you.

---

## Compile-time gotchas

### G1. Adding a field to `CapabilityContract` breaks twelve files

`CapabilityContract` is constructed as a full struct literal in
**every** crate that writes a capability fixture for its unit
tests. Adding a new required field (no `#[serde(default)]`, no
builder) forces you to update every construction site or the
workspace stops compiling.

Canonical construction sites that have needed updating each time:

- `core/crates/aaf-contracts/src/capability.rs` (the tests for
  `validate`)
- `core/crates/aaf-registry/src/store.rs` + `discovery.rs`
- `core/crates/aaf-planner/src/planner.rs` + `composition.rs`
- `core/crates/aaf-policy/src/engine.rs` + `rules/boundary.rs` +
  `rules/pii.rs` + `rules/injection.rs` + `guard.rs`
- `core/crates/aaf-runtime/src/executor.rs` (tests)
- `core/crates/aaf-saga/src/*` (tests)
- `core/crates/aaf-sidecar/src/*`
- `core/crates/aaf-wrapper/src/*`
- `core/crates/aaf-federation/src/lib.rs`
- `core/crates/aaf-server/src/main.rs`
- `core/crates/aaf-trust/src/signing.rs` (the test fixture)
- `core/tests/integration/tests/*` — every smoke test that builds a
  cap

**Mitigation:** either add the field with `#[serde(default)]` and
an `Option<_>` shape (preferred), or use a grep-driven checklist:

```bash
grep -rn "CapabilityContract {" core/crates/ core/tests/
```

Iteration 5 fixed twelve stale sites this way. Iteration 8 was
careful to add `required_attestation_level: None` everywhere when
X1 Slice B extended the struct.

### G2. `PolicyContext` field-literal churn

`PolicyContext` is constructed as a struct literal in **five**
files:

- `core/crates/aaf-policy/src/engine.rs` (3 test sites)
- `core/crates/aaf-policy/src/guard.rs` (2 sites)
- `core/crates/aaf-policy/src/rules/pii.rs` (1 test site)
- `core/crates/aaf-policy/src/rules/injection.rs` (1 test site)
- `core/crates/aaf-runtime/src/executor.rs` (3 hook sites)

Adding a new field (`ontology_class_lookup` in iteration 8 is the
most recent example) means patching every site.

**Mitigation:**

```bash
grep -rn "composed_writes:" core/crates/
```

finds every `PolicyContext` construction. Every site now carries
`ontology_class_lookup: None`.

### G3. `IntentEnvelope` field-literal churn

`IntentEnvelope` is constructed as a struct literal mostly in test
fixtures. Adding `entities_in_context: Vec<_>` in iteration 4 was
the last big wave.

**Mitigation:** `grep -rn "IntentEnvelope {" core/ | wc -l` — expect
~25 sites.

### G4. Long-term memory search default impl

`LongTermMemoryStore::search_by_entity` has a **default impl** that
returns `Ok(vec![])`. If you add a new storage backend and forget
to override it, the integration tests will pass (because the
default is silent) but entity-keyed retrieval will silently return
nothing.

**Mitigation:** the `InMemoryLongTermStore` does override it. Any
new backend (pgvector, Qdrant) must override it too — there is a
TODO at the top of the default impl reminding future authors.

---

## Runtime gotchas

### R1. The executor's five hook points

`aaf-runtime::executor::GraphExecutor::run` calls the policy
engine at these exact hook points:

1. `PolicyHook::PrePlan` (once, before any node runs)
2. *(optional)* revocation gate — X1 Slice B
3. For each node in topological order:
   a. `PolicyHook::PreStep`
   b. Run the node
   c. `PolicyHook::PostStep`
   d. `PolicyHook::PreArtifact` (only when the node produced an artifact)
4. Close the trace on success; walk the compensation chain on
   failure

**If you add a new hook point**, update this document, update
`development/architecture-overview.md` → "The hot path, annotated",
and update every test that asserts a hook count.

### R2. Compensation chain ordering

Nodes register their compensator **on successful completion** via
`GraphBuilder::add_compensator`. On any later failure the executor
drains the chain **in reverse** — last-in-first-out. If you forget
to register, the chain is empty and the failing node leaves the
partial write un-rolled-back. Iteration 3 fixed this as a bug;
the regression test is
`aaf-runtime::tests::rule_9_compensation_runs_on_node_failure`.

### R3. Revocation gate runs *before* the trace opens

X1 Slice B wired `GraphExecutor::with_revocation` so that a revoked
DID fails **before** the trace is opened. Do not move the check
later — the invariant is that revoked agents leave no trace
artefacts behind.

### R4. Trust `min(a, b)` applies to *every* delegation

`aaf-trust::delegation::effective_trust(delegator, delegatee)`
returns the minimum numeric level. If you introduce a new
delegation primitive (cross-cell, cross-tenant, etc.), remember
this is the *behavioural* dimension and the cryptographic
dimension (X1 Slice B `verify_token`) is additive — both must
pass before the delegation is honoured.

### R5. Budget decrement must happen on *every* LLM call

`aaf-llm::budget::PerCallBudget` and `aaf-runtime::budget::BudgetTracker`
are two halves of the same contract. The LLM router charges the
per-call budget; the runtime executor charges the per-intent
budget. Both must decrement or Rule 8 breaks. Iteration 2 added a
deterministic cost-based test in `aaf-runtime::budget::tests` to
replace an earlier timing-based one — keep that test green.

---

## Ontology / boundary gotchas

### O1. The planner does not depend on `aaf-ontology`

`aaf-planner::composition::EntityAwareComposition` takes a
**callback** (`ClassificationLookup`) instead of an
`OntologyRegistry`. This is intentional: importing `aaf-ontology`
from `aaf-planner` would introduce a dependency cycle once the
ontology grows. If you need entity-aware logic in the planner,
pass it in as a callback.

Same for `aaf-policy::context::OntologyClassificationLookup`.

### O2. `EntityRefLite.tenant` is optional — think carefully before
skipping it

`EntityRefLite` can be constructed without a tenant. Most hot-path
consumers treat a `None` tenant as "unresolved / global". The
federation router **rejects cross-tenant fan-out only when both
sides carry tenants**; a `None`-tenant ref against a tenant-A rule
is currently allowed by design (the rule does not tighten past its
declared scope). Changing this is a semantics change and must be
an ADR.

### O3. Ontology lint severity flips at exactly 90%

The adoption-ratio ramp in `aaf-server::lint::lint_directory`
flips from warn-only to strict at `adoption_ratio >= 0.90`. The
threshold lives in a `const ADOPTION_STRICT_THRESHOLD: f32 = 0.90;`
— do not pull it from config. The threshold exists so the lint
does not become stricter when a directory adds a single bad
capability and drops below 90%.

### O4. Two of the shipped examples have `reads` + `writes` + `emits`
populated — do not accidentally remove them

`spec/examples/capability-inventory.yaml` and
`spec/examples/capability-payment.yaml` were updated in iteration 8
to carry entity declarations. This puts the adoption ratio at 100%
and flips `make ontology-lint` to strict mode.

If you add a new `capability-*.yaml` under `spec/examples/`:

- Populate `reads:` / `writes:` / `emits:` (or consciously leave
  one empty if the capability is read-only and you want it to
  trigger a `Warn`).
- Run `make ontology-lint` before committing.

---

## Identity (X1) gotchas

### I1. The signer backend is HMAC, not Ed25519

Despite the X1 design calling for Ed25519, Slice A + Slice B both
ship a deterministic HMAC-SHA256 backend because `ed25519-dalek`
does not compile cleanly on Rust 1.70. This is documented in the
crate-level doc comment in `aaf-identity/src/lib.rs` and in
`docs/adr/*` (future Slice C ADR).

**Do not silently swap the backend.** Slice C will introduce a
trait impl for Ed25519 without changing any call site.

### I2. Artifacts carry a `v0:` or `x1:` signature prefix

`aaf-trust::signing::sign_artifact_with` produces an
`x1:<did>:<sig>` envelope; the legacy `sign_artifact` produces a
`v0:<checksum>` envelope. `verify_artifact_with` reports `true` on
the former and `false` on the latter — there is no "v0 verifies
successfully" fallback. Tests that check `verify_artifact_with`
must use `sign_artifact_with`.

### I3. Revoked DIDs must short-circuit *before* the trace opens

See R3 above. This is both a hot-path gotcha and an identity
gotcha.

---

## Clippy / style gotchas

### C1. `assert_eq!(x, false)` is a clippy warning

Iteration 8 fixed two of these in `aaf-trust::signing::tests`. Use
`assert!(!x)` everywhere.

### C2. `format!("literal string")` is a clippy warning

If the argument has no substitutions, just pass the literal to
`push_str` / `write!` / etc. Iteration 8 fixed one of these in
`aaf-server::import::render_yaml`.

### C3. `std::fs::read_to_string(&path)` when `path` is already a
`PathBuf` is a clippy warning

Use `path.as_path()` (it's a free method that returns `&Path`).
Iteration 8 fixed one of these in `aaf-server::main::cmd_ontology_import`.

### C4. Pedantic clippy is not green

`make clippy` runs pedantic lints and fails on the current tree.
This has been true since iteration 1. Do not try to make pedantic
green in one go — it is a multi-iteration cleanup and needs its
own iteration entry in `IMPLEMENTATION_PLAN.md`.

The canonical command for iteration-level checks is
`cargo clippy --workspace --all-targets -- -W clippy::all`, which
is zero-warnings today.

---

## Schema / examples gotchas

### S1. `schema_validate.py` uses prefix mapping

The YAML-to-schema mapping in `scripts/schema_validate.py` keys
off filename prefix. A new example file must match one of the
prefixes in `PREFIX_TO_SCHEMA` or `schema_validate.py` will print
`SKIP`. Iteration 3 + 4 had to add new prefixes for the ontology /
proposal / app-event / eval-suite files.

### S2. Two examples intentionally lack a schema

`manifest-order-agent.yaml` and `sbom-order-agent.yaml` are fed
directly to `aaf-identity::manifest::from_yaml` /
`aaf-identity::sbom::from_yaml`; there is no `agent-manifest` or
`agent-sbom` JSON Schema in the prefix map (although the schemas
*exist* under `spec/schemas/`). These print `SKIP` in the
validator output — that is expected.

A future Slice C will add the mappings; until then, `SKIP` is
correct.

---

## Test-isolation gotchas

### T1. `InMemoryOntologyRegistry::list()` is async

`aaf-ontology::registry::OntologyRegistry::list` is
`#[async_trait]` async. If you call it inside a sync closure (e.g.
from `Enricher::enrich_with_ontology`), you must **pre-list** the
entities outside the closure:

```rust
let all_entities = ontology.list().await?;
let resolver = move |domain: &str, _: &str| -> Vec<EntityRefLite> {
    all_entities.iter().filter(…).cloned().collect()
};
```

See `core/tests/integration/tests/e2_slice_b_smoke.rs` for the
canonical pattern.

### T2. Integration tests using `tempdir()`

`aaf-server::lint::tests` has a tiny local `fn tempdir()` helper
that uses `std::env::temp_dir()` + process id + nanosecond
timestamp. Do **not** add `tempfile` as a workspace dep; the
vendored helper works and avoids dragging in a transitive dep.

---

## The "should I add a crate?" gotcha

Every crate in this workspace enforces at least one rule or owns
at least one distinct concept. Before adding a new crate, ask:

1. Can this live as a module inside an existing crate? (Usually
   yes.)
2. Does it have a distinct persistence boundary? (If yes, it needs
   its own storage trait in `aaf-storage` — the crate itself may
   not need to exist.)
3. Does it introduce a new *rule* or a new *contract type*? (If
   yes, a new crate may be justified.)

As of the current state, the 22 crates (including `aaf-learn` from
E1 Slice B) are the minimum decomposition for the vision. Adding a
23rd crate should be an ADR-level decision.
