# E2 — Domain Ontology Layer

> **Status:** Complete (Slice A iter 4, Slice B iter 7, Slice C iter 8).
>
> **Rules enforced:** R14 Semantics are nouns, not names. R21 Entities
> are tenant-scoped by default.
>
> **Authoritative design:** `PROJECT.md` §16.2 + `CLAUDE.md` rules
> 14, 21. This page is a reader-facing summary.

---

## The problem

Before E2, AAF's "semantics" were actually field names. A
`CapabilityContract` carried `input_schema` and `output_schema`
(JSON shapes) but nothing said *which real-world entities* the
capability read or wrote. Three capabilities that all touched
the same `Order` looked indistinguishable from three
capabilities that touched three unrelated order-shaped records.

Consequences:

- **The planner could not detect** that a plan was going to
  write `commerce.Order` from two different services.
- **The policy boundary rule could not express** "this
  capability reads PII" — it relied on hand-maintained tags.
- **Long-term memory retrieval was keyword-only** — there was
  no "give me the last 5 observations where `Order#123` was
  written".
- **Federation agreements were string denylists** — parallel
  lists of prohibited field names that drifted out of sync with
  the actual capability contracts.

The slogan "semantic orchestration layer" could not be earned
without a **noun layer**.

---

## The solution

Give AAF a first-class ontology. Every capability declares the
entities it reads, writes, and emits, using a shared, versioned
vocabulary. The planner, policy engine, memory system, registry,
and federation layer all reason over those entities.

---

## What landed

### Slice A — contracts + crate skeleton (iteration 4)

| Deliverable | Location |
|---|---|
| `aaf-ontology` crate | `core/crates/aaf-ontology/` |
| `Entity`, `EntityRef`, `EntityVersion`, `Classification`, `Relation` types | `aaf-ontology::entity`, `::relation` |
| `OntologyRegistry` trait + in-memory impl | `aaf-ontology::registry` |
| `EntityResolver` trait + `ExactMatchResolver` | `aaf-ontology::resolver` |
| `LineageRecord` / `EntityRefVersioned` | `aaf-ontology::lineage` |
| `VersionCompatibility`, `compare_versions` | `aaf-ontology::version` |
| JSON-Schema ingest (best-effort) | `aaf-ontology::import` |
| Classification lattice | `Public ⊂ Internal ⊂ Pii ⊂ Regulated(_)` |
| Optional `reads` / `writes` / `emits` / `entity_scope` on `CapabilityContract` | `aaf-contracts::capability` |
| Optional `entities_in_context` on `IntentEnvelope` | `aaf-contracts::intent` |
| Optional `derived_from` on `Artifact` | `aaf-contracts::artifact` |
| `ontology-commerce.yaml` example | `spec/examples/` |

### Slice B — integration into the hot-path crates (iteration 7)

| Deliverable | Location |
|---|---|
| Long-term memory entity-keyed retrieval | `aaf-storage::LongTermMemoryStore::search_by_entity` + in-memory inverted index |
| `MemoryFacade::longterm_search_by_entity` | `aaf-memory::facade` |
| Registry entity-aware discovery | `aaf-registry::discover_by_entity(EntityQueryKind)` |
| Entity-aware composition checker | `aaf-planner::composition::EntityAwareComposition` with 3 detectors: `DoubleWrite`, `ClassificationLeak`, `CrossTenantFanOut` |
| `RegistryPlanner::with_entity_composition` wiring + `PlannerError::UnsafeEntityComposition` | `aaf-planner::planner` |
| Intent enricher ontology resolver | `aaf-intent::Enricher::enrich_with_ontology` + `OntologyResolver` trait |
| Policy boundary rule ontology lookup | `aaf-policy::context::OntologyClassificationLookup` + rewritten `BoundaryEnforcement` |

End-to-end smoke test: `core/tests/integration/tests/e2_slice_b_smoke.rs`.

### Slice C — federation, lint, import, ADR (iteration 8)

| Deliverable | Location |
|---|---|
| Federation in entity space | `aaf-federation::{EntityAccessRule, FederationAgreement::with_entity_rules, Router::enforce_capability, enforce_outbound_entity}` |
| Four new `FederationError` variants | `EntityNotPermitted`, `ClassificationCapExceeded`, `TenantMismatch`, (plus legacy `BoundaryViolation`, `NoAgreement`) |
| Ontology lint CLI + module | `aaf-server ontology lint` + `aaf-server::lint` |
| `make ontology-lint` Makefile target + CI gate | `Makefile` |
| Adoption-ratio ramp (`ADOPTION_STRICT_THRESHOLD = 0.90`) | `aaf-server::lint` |
| `ontology import` CLI (OpenAPI → Entity proposals) | `aaf-server::import` |
| Federation cosign (subsequent linter addition) | `aaf-federation::cosign` |
| Example capability YAMLs updated with entity declarations | `spec/examples/capability-inventory.yaml`, `capability-payment.yaml` |
| [ADR-008 — Entity-space boundaries](../adr/ADR-008-entity-space-boundaries.md) | `docs/adr/` |

---

## How each hot-path crate keys off the ontology

```
aaf-intent::Enricher::enrich_with_ontology(env, resolver)
  → populates env.entities_in_context from an OntologyResolver

aaf-registry::discover_by_entity(entity_ref, kind)
  → indexes over cap.{reads, writes, emits}

aaf-planner::composition::EntityAwareComposition::check(caps)
  → runs 3 detectors over the declared entity fields

aaf-policy::rules::boundary::BoundaryEnforcement::evaluate(ctx)
  → consults ctx.ontology_class_lookup for read-classification flow

aaf-memory::longterm_search_by_entity(tenant, entity_ref, limit)
  → O(1) lookup via an (tenant, entity_id) inverted index

aaf-federation::Router::enforce_capability(from, to, cap)
  → walks cap.{reads, writes, emits} against EntityAccessRule set
```

Six hot-path crates, one shared vocabulary.

---

## Rules

| Rule | How E2 enforces it |
|---|---|
| **R14** Semantics are nouns, not names | `CapabilityContract.{reads, writes, emits}` are the source of truth; lint tracks adoption; strict mode fails the build on writers missing `writes:` |
| **R21** Entities are tenant-scoped by default | `EntityRefLite.tenant: Option<TenantId>`; policy boundary rule + federation router both enforce per-tenant scoping |

---

## What's next

**E2 is complete.** No further Slice planned.

Follow-ups that touch the ontology but are owned by other
enhancements:

- **E1 Slice B** will key reputation and router weights by
  `(capability, entity_class)`, not just by capability id —
  the entity dimension makes the feedback signal vastly more
  informative.
- **E3 Slice B** will populate `entities_in_context` from the
  situation packager, so every intent that originates from an
  app event carries its entity context automatically.
- **X2 Knowledge Fabric** (deferred) will give every `GroundedChunk`
  an ontology `LineageRecord` so retrieval is entity-lineage-aware.

---

## Further reading

- [ADR-008 — Entity-space boundaries](../adr/ADR-008-entity-space-boundaries.md)
- [`../../development/crate-reference.md`](../../development/crate-reference.md)
  → `aaf-ontology` section
- [`../integration-cell-architecture.md`](../integration-cell-architecture.md)
  → federation in entity space
- [`../ontology-lint.md`](../ontology-lint.md)
- `PROJECT.md` §16.2 — the design document
- `core/crates/aaf-ontology/src/` — the source
