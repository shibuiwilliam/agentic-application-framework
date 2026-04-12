# ADR-008 — Entity-Space Boundaries

**Status:** Accepted (iteration 8, E2 Slice C)
**Supersedes:** Pre-Slice-C `FederationAgreement.prohibited_fields` semantics
**Related:** `PROJECT.md` §16.2 (E2 Domain Ontology Layer),
`CLAUDE.md` Rules 14, 21, `IMPLEMENTATION_PLAN.md` iterations 4 / 7 / 8

## Context

Before iteration 7, `aaf-federation::FederationAgreement` expressed
cross-cell boundaries as a `HashSet<String>` of *prohibited field
names*. Before iteration 7, the `aaf-policy::rules::boundary` rule
likewise matched a capability's `DataClassification` enum tag against
a hand-maintained scope set. Both mechanisms shared the same failure
mode: *semantics were encoded as strings that humans had to keep in
sync across every declaration site*.

Concretely, the pre-Slice-C shape forced three undesirable patterns:

1. **Parallel lists.** A PII field had to be named in every
   `prohibited_fields` set *and* tagged `Confidential` on every
   capability that touched it. Drift was silent.
2. **No composition safety.** Two capabilities that each touched
   `commerce.Order` from different services looked indistinguishable
   from two capabilities touching two unrelated order-shaped records.
   The planner's composition checker — introduced in v0.1 — could
   only reason about side-effect counts, not about the nouns that the
   side effects operated on.
3. **Tenant leakage was invisible to the boundary rule.** A cap that
   wrote `Order` in tenant A and a cap that wrote `Order` in tenant B
   could legitimately share the same plan; the boundary rule had no
   way to notice, because nothing in the contract said which tenant
   the write applied to.

The ontology layer (`aaf-ontology`, landed in iteration 4 as E2 Slice
A) gave every capability a structured way to declare what it `reads`,
`writes`, and `emits`. Iterations 7 and 8 *use* those declarations
on the hot path.

## Decision

**All cross-boundary enforcement in AAF operates on declared
`EntityRef`s, not on free-form field or tag strings.**

Concretely:

1. `aaf-federation::FederationAgreement` gains an `entity_rules:
   Vec<EntityAccessRule>` field. Each rule carries
   `(entity_id, op, max_classification?, tenant?)`.
   `Router::enforce_capability(from, to, cap)` walks the capability's
   declared `reads` / `writes` / `emits` and checks each against the
   rules. The legacy `prohibited_fields` path remains as a fallback
   so pre-Slice-C agreements stay valid.
2. `aaf-policy::rules::boundary::BoundaryEnforcement` consults an
   optional `OntologyClassificationLookup`. When present, the rule
   flags a capability that reads an entity whose declared
   classification exceeds the capability's own `data_classification`,
   and flags writes whose declared entity tenant mismatches the
   active tenant.
3. `aaf-planner::composition::EntityAwareComposition` runs a set of
   entity-aware detectors (double-write, classification leak,
   cross-tenant fan-out) on top of the v0.1 `CompositionChecker`.
4. `aaf-registry::discover_by_entity` lets consumers query "who
   writes `commerce.Order`?" directly.
5. `aaf-memory::longterm_search_by_entity` retrieves records indexed
   under a given entity id, so the memory system keys off nouns too.
6. `aaf-intent::Enricher::enrich_with_ontology` populates
   `IntentEnvelope.entities_in_context` from an
   ontology-backed resolver, so every intent carries an entity
   context before it reaches the planner.

The union of those six points is the "entity-space boundary" this
ADR accepts: **there is one source of truth for "which nouns does
this capability touch", and every enforcement layer consults it.**

## Consequences

### Positive

- **One declaration site per capability.** A capability author names
  the nouns once, and the planner, policy engine, memory system,
  registry, and federation layer all pick them up automatically.
- **Composition safety becomes mechanical.** Double-writes,
  classification leaks, and cross-tenant fan-outs are detected by
  reading the declarations, not by static scoping tricks.
- **Federation agreements become readable.** "cell-jp may read
  `commerce.Product` but not `commerce.Customer`" is a sentence
  humans can audit in minutes.
- **Tenants propagate.** Because `EntityRef` carries `Option<TenantId>`,
  cross-tenant fan-out is a first-class error variant
  (`CrossTenantFanOut`, `TenantMismatch`) rather than a silent
  collision.
- **Adoption is observable.** `make ontology-lint` reports the
  fraction of capabilities carrying declarations and upgrades
  warnings to errors at the 90% threshold.

### Negative

- **Migration cost.** Every capability contract must grow
  `reads` / `writes` / `emits` sections. The lint's warn-only mode
  under 90% adoption is the ramp for this; once the codebase passes
  the threshold, missing declarations become hard errors.
- **Ontology-registry freshness.** The enforcement layers depend on
  the ontology being current. Drift (an entity whose real-world
  classification changes but the ontology doesn't update) silently
  weakens the boundary. Mitigation: ontology changes are themselves
  versioned and auditable; a future Slice C+ iteration should wire a
  CI check that flags entities whose last-updated timestamp exceeds
  a configured freshness window.
- **Policy-engine opt-in.** For back-compatibility the
  `OntologyClassificationLookup` is `Option`-wrapped on
  `PolicyContext`. Deployments that forget to wire it silently
  revert to pre-Slice-C semantics. Mitigation: the server's
  default `seed_registry` should wire the lookup in iteration 9.

### Neutral

- The legacy `prohibited_fields` path is **not** removed. It will be
  deprecated-but-supported for the foreseeable future so existing
  configs keep working. New deployments should use `entity_rules`.

## Alternatives considered

1. **OpenAPI-only.** Keep the pre-Slice-C boundary as-is and require
   users to point at OpenAPI `x-*` extensions. Rejected: that leaves
   the planner, memory system, and registry blind; the whole value
   of Slice B was that *every* hot layer now speaks the same noun
   vocabulary, not just the HTTP boundary.
2. **CEL-expression boundaries.** Let agreements carry a CEL
   expression evaluated against the capability contract. Rejected:
   the expression becomes the new string-typed source of truth; the
   failure mode is the same one we are trying to leave behind.
3. **Annotation-driven.** Carry a `#[allowed_across_cells]` attribute
   on each capability and propagate it at registration time.
   Rejected: capabilities are declared in YAML more often than in
   Rust, and attributes pin the authority to the wrong layer.

## References

- `PROJECT.md` §16.2 — E2 Domain Ontology Layer
- `CLAUDE.md` Rule 14 — Semantics Are Nouns, Not Names
- `CLAUDE.md` Rule 21 — Entities Are Tenant-Scoped by Default
- `core/crates/aaf-federation/src/lib.rs` — implementation
- `core/crates/aaf-planner/src/composition.rs` —
  `EntityAwareComposition`
- `core/crates/aaf-policy/src/rules/boundary.rs` — rewritten
  boundary rule
- `core/tests/integration/tests/e2_slice_b_smoke.rs` — full-chain
  Slice B smoke test
- `core/tests/integration/tests/e2_slice_c_smoke.rs` — federation
  entity-rule smoke test
