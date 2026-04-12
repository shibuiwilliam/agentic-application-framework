# Capability Authoring Guide

> How to define, register, and test capabilities in AAF.

A *capability* is the unit of work the AAF orchestration layer
discovers and invokes. It represents something a service can do
(e.g. "check inventory", "process payment", "generate report"),
described with enough metadata that the planner can compose it into
execution plans without knowing the service's internal implementation.

---

## Capability Contract structure

Every capability is a `CapabilityContract` (defined in `aaf-contracts`):

```rust
CapabilityContract {
    // ── Identity ────────────────────────────────────
    id: CapabilityId,              // e.g. "cap-stock-check"
    name: String,                  // Human-readable
    description: String,           // Used by semantic discovery
    version: String,               // Semver
    provider_agent: String,        // Agent that owns this capability

    // ── Endpoint ────────────────────────────────────
    endpoint: CapabilityEndpoint { kind, address, method },

    // ── Schema ──────────────────────────────────────
    input_schema: Value,           // JSON Schema for input
    output_schema: Value,          // JSON Schema for output

    // ── Classification (critical) ───────────────────
    side_effect: SideEffect,       // None | Read | Write | Delete | Send | Payment
    idempotent: bool,
    reversible: bool,
    deterministic: bool,           // Rule 5: must be true for financial/auth/crypto
    compensation: Option<CompensationSpec>,  // Rule 9: required for write capabilities

    // ── Policy ──────────────────────────────────────
    required_scope: String,        // Scope the requester must carry
    data_classification: DataClassification,  // Public | Internal | Confidential | Restricted
    sla: CapabilitySla,
    cost: CapabilityCost,

    // ── Ontology (E2) ──────────────────────────────
    domains: Vec<String>,
    reads: Vec<EntityRefLite>,     // What entities this cap reads
    writes: Vec<EntityRefLite>,    // What entities this cap writes
    emits: Vec<EventRefLite>,      // What events this cap emits
    entity_scope: Option<EntityScopeLite>,
    tags: Vec<String>,

    // ── Identity (X1) ──────────────────────────────
    required_attestation_level: Option<AttestationLevelRef>,
    reputation: f64,               // Updated by CapabilityScorer (E1)
    learned_rules: Vec<String>,    // Fast-path rules referencing this cap
}
```

---

## Naming conventions

| Field | Convention | Example |
|---|---|---|
| `id` | `cap-<domain>-<verb>` | `cap-stock-check`, `cap-payment-execute` |
| `name` | lowercase phrase | `stock check`, `payment execute` |
| `description` | Sentence starting with a verb | `check inventory stock for a product` |
| `domains` | lowercase, plural | `["ecommerce"]`, `["analytics"]` |
| `required_scope` | `<domain>:<permission>` | `inventory:read`, `payment:execute` |

---

## Checklist for a new capability

### Before writing code

- [ ] **Classify the side effect.** Is it Read, Write, Delete, Send,
  or Payment? If Write/Delete/Send/Payment, you MUST define
  compensation (Rule 9).
- [ ] **Is it deterministic?** Financial calculations, auth decisions,
  inventory reservations → `deterministic: true` (Rule 5).
- [ ] **What entities does it touch?** Declare `reads`, `writes`,
  `emits`. The ontology lint (`make ontology-lint`) will catch
  missing declarations.
- [ ] **What scope is required?** The policy engine checks this at
  every step (Rule 6).
- [ ] **What is the data classification?** This determines what
  scopes the requester needs (boundary enforcement rule).

### Registration

Capabilities are registered in the `Registry`:

```rust
let registry = Arc::new(Registry::in_memory());
registry.register(CapabilityContract { /* ... */ }).await.unwrap();
```

In YAML configuration (`aaf.yaml` or `spec/examples/`):

```yaml
capabilities:
  - id: cap-stock-check
    name: stock check
    description: check inventory stock for a product
    domains: [ecommerce]
    required_scope: inventory:read
    side_effect: read
```

### Compensation (Rule 9)

Every write capability must have a compensation spec:

```yaml
capabilities:
  - id: cap-payment-execute
    side_effect: payment
    compensation:
      capability: cap-payment-refund
```

In code:

```rust
compensation: Some(CompensationSpec {
    capability: CapabilityId::from("cap-payment-refund"),
}),
```

### Entity declarations (E2)

Declare what ontology entities the capability touches:

```rust
reads: vec![EntityRefLite::new("commerce.Order")],
writes: vec![EntityRefLite::new("finance.Payment")],
emits: vec![],
```

The planner uses these to:
- Find capabilities by entity (semantic discovery)
- Check composition safety (no double-write on same entity)
- Enforce data classification boundaries

---

## Testing a new capability

### Unit test in the capability's crate

Test the capability's business logic in isolation.

### Integration test

Register the capability, build a graph with a `DeterministicNode`
(or the actual node), and run it through `GraphExecutor`:

```rust
let registry = Arc::new(Registry::in_memory());
registry.register(my_capability()).await.unwrap();

let node = Arc::new(DeterministicNode::new(
    NodeId::from("cap-my-capability"),
    SideEffect::Read,
    Arc::new(|_, _| Ok(serde_json::json!({"result": "ok"}))),
));

let graph = GraphBuilder::new().add_node(node).build().unwrap();
let exec = GraphExecutor::new(policy, recorder, budget);
let outcome = exec.run(&graph, &intent).await.unwrap();
```

### Schema validation

If you add a YAML example under `spec/examples/`, run:

```bash
make schema-validate
```

### Ontology lint

```bash
make ontology-lint
```

This checks that capabilities declare entity refs for the entities
they touch. Adoption ratio < 90% generates warnings; ≥ 90% triggers
strict-mode errors.

---

## Fast-path rules

If a capability handles a well-known structured request pattern, add
a fast-path rule so it can be invoked without LLM planning (Rule 4):

```rust
FastPathRule {
    pattern: RequestPattern {
        intent_type: "AnalyticalIntent".into(),
        domain: "sales".into(),
    },
    target_capability: CapabilityId::from("cap-sales-monthly"),
    field_mapping: vec![FieldMapping {
        from: "period_ref".into(),
        to: "period".into(),
    }],
    conditions: vec![Condition {
        field: "period_ref".into(),
        equals: json!("last_month"),
    }],
}
```

Target: >60% of requests should hit the fast path.

---

## Degradation levels

Capabilities can declare degradation behavior:

```yaml
degradation:
  - level: full
    description: "Real-time, all warehouses"
  - level: partial
    trigger: "primary_db_slow"
    description: "Primary warehouses only, 15min delay"
  - level: cached
    trigger: "db_unreachable"
    description: "Up to 1hr stale cache"
  - level: unavailable
    fallback: "Request manual check"
```

The `DegradationStateMachine` transitions monotonically:
Full → Partial → Cached → Unavailable (and recovers in reverse).
