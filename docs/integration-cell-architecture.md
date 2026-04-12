# Integrating AAF with a Cell Architecture

> **Pattern C** from `PROJECT.md` §2.3 — federation across
> **cells**, where each cell owns a local AAF runtime and
> bilateral agreements govern cross-cell calls. Every
> architectural component in this pattern lives in
> `aaf-federation`.

---

## The model

```
┌──────────────────────┐                 ┌──────────────────────┐
│      cell-japan      │                 │       cell-us        │
│  ┌────────────────┐  │                 │  ┌────────────────┐  │
│  │  aaf-runtime   │  │                 │  │  aaf-runtime   │  │
│  └────────┬───────┘  │                 │  └────────┬───────┘  │
│  ┌────────┴───────┐  │                 │  ┌────────┴───────┐  │
│  │ aaf-federation │◀─┼────A2A / cosign┼─▶│ aaf-federation │  │
│  └────────────────┘  │                 │  └────────────────┘  │
└──────────────────────┘                 └──────────────────────┘
```

Each cell has its **own** runtime, policy engine, registry,
memory, and trace store. Cells communicate only through
`aaf-federation`, which enforces the agreements that govern what
capabilities and entities may cross the boundary.

---

## Federation agreements — in entity space (E2 Slice C)

Before iteration 8, agreements were a flat list of
"prohibited field names". That worked for the simplest cases but
did not scale — field names were the single source of truth, and
they drifted out of sync with the actual capability contracts.

Iteration 8 replaced this with **entity-space agreements**. An
agreement declares per-entity rules; the router walks each
capability's declared `reads` / `writes` / `emits` and checks
them against the agreement.

```rust
let rules = vec![
    EntityAccessRule {
        entity_id: "commerce.Product".into(),
        op: EntityOp::Read,
        max_classification: None,
        tenant: None,
    },
    EntityAccessRule {
        entity_id: "commerce.Order".into(),
        op: EntityOp::Write,
        max_classification: Some(ClassificationCap::Internal),
        tenant: None,
    },
    EntityAccessRule {
        entity_id: "commerce.Payment".into(),
        op: EntityOp::Write,
        max_classification: None,
        tenant: Some("tenant-a".into()),
    },
];
let agreement = FederationAgreement::with_entity_rules(
    vec![CellId("cell-japan".into()), CellId("cell-us".into())],
    vec!["cap-order-reserve".into()],
    rules,
);
```

And then at runtime:

```rust
let router = Router::new(cells, vec![agreement]);
router.enforce_capability(&cell_japan, &cell_us, &capability)?;
```

Error variants:

- `EntityNotPermitted { entity_id, op }` — no rule covers the
  access.
- `ClassificationCapExceeded { … }` — capability's
  `data_classification` exceeds the rule's `max_classification`.
- `TenantMismatch { … }` — rule is tenant-restricted and the
  capability's entity ref carries a different tenant.
- `BoundaryViolation(field)` — legacy string-denylist path.
- `NoAgreement(cell)` — no agreement covers `(from, to)`.

**See ADR-008** for the architectural rationale
([adr/ADR-008-entity-space-boundaries.md](adr/ADR-008-entity-space-boundaries.md)).

---

## Cross-cell handoffs with co-signed tokens

When cell A needs to invoke a capability hosted by cell B, it
does so through a **co-signed capability token** (X1 Slice C
deliverable; landed as `aaf_federation::cosign`).

The flow:

1. Cell A mints a `CapabilityToken` bound to the target
   capability id, expiry, and remaining depth.
2. Cell B verifies A's signature, then adds its own signature
   (`CoSignedToken`).
3. The runtime in cell B honours the token only if *both*
   signatures verify against the respective DIDs.

Relevant APIs:

- `aaf_federation::cosign::cosign_token`
- `aaf_federation::cosign::verify_cosigned`
- `aaf_federation::cosign::CoSignedToken`
- `aaf_federation::cosign::CoSignError`

Co-signing composes with the entity-space agreement: a cap that
passes both the agreement check **and** the co-signed token
check is allowed; any failure short-circuits.

---

## Data boundary enforcement

Every cross-cell data flow is checked against the agreement. The
router supports two modes:

1. **Capability-level** —
   `Router::enforce_capability(from, to, cap)` walks the
   capability's declared entities.
2. **Payload-level (legacy)** —
   `Router::enforce_outbound(from, to, json_payload)` walks the
   payload's top-level fields against the legacy denylist. Kept
   for back-compat.
3. **Entity-level helper** —
   `Router::enforce_outbound_entity(from, to, entity_ref, op)`
   for consumers that already have an entity ref.

---

## Cell config example

`spec/examples/cell-config-japan.yaml` (TBD in Slice C polish):

```yaml
cell:
  id: cell-japan
  region: ap-northeast-1
  local_capabilities:
    - cap-jp-orders
    - cap-jp-payments

agreements:
  - parties: [cell-japan, cell-us]
    shared_capabilities: [cap-us-orders]
    entity_rules:
      - entity_id: commerce.Product
        op: read
      - entity_id: commerce.Order
        op: write
        max_classification: internal
      - entity_id: commerce.Payment
        op: write
        tenant: tenant-a
```

---

## Policy hooks + federation

Cross-cell calls still go through the full policy pipeline on the
originating cell. The federation router runs *after* the policy
engine, as a final gate before the request leaves the cell:

```
PrePlan → PreStep → run node →
    if node emits a cross-cell call:
        federation.enforce_capability(from, to, cap)
        if cosigned: federation.cosign.verify_cosigned(token, ...)
    PostStep → record observation
```

---

## Observability

Every cross-cell call records an `Observation` on *both* cells
— the initiator records an outbound, the receiver records an
inbound. Trace ids propagate via the federation header, so a
trace can be reconstructed across cells.

---

## Further reading

- [adr/ADR-008-entity-space-boundaries.md](adr/ADR-008-entity-space-boundaries.md)
  — the rationale for entity-space agreements
- [security.md](security.md) — the security model, including the
  classification lattice
- `core/crates/aaf-federation/src/lib.rs` — implementation
- `core/crates/aaf-federation/src/cosign.rs` — co-signed tokens
- `core/tests/integration/tests/e2_slice_c_smoke.rs` — end-to-end
  test of entity-space enforcement
