# Fast Path

> How AAF classifies traffic into four patterns and when it skips
> the LLM entirely. Rule 4 mandates: **check fast path first**, with
> a target of > 60% of production traffic on the fast path.

---

## The four communication patterns

Every incoming request is classified into exactly one of four
patterns. The classifier is in `aaf-planner::router::Router`.

```
Request →  Fully structured + unambiguous target?
           │
           YES →  ① FAST PATH
                  <50ms p99, no LLM, direct routing
           │
           NO  →  Single service with minor ambiguity?
                  │
                  YES →  ② AGENT ASSISTED
                         <500ms p99, small model
                  │
                  NO  →  Multi-service coordination needed?
                         │
                         YES →  ③ FULL AGENTIC
                                <15s p99, plan + graph
                         │
                         NO  →  ④ AGENTIC CHOREOGRAPHY
                                minutes–hours, async
```

| Pattern | p50 | p99 | AAF overhead |
|---|---|---|---|
| Fast Path | 5ms | 20ms | <5ms |
| Agent Assisted | 100ms | 500ms | 50–200ms |
| Full Agentic | 2s | 15s | 1–10s |
| Choreography | — | — | — |

See `PROJECT.md` §4 for the authoritative latency table.

---

## Fast path rules

Fast-path rules are declarative: each rule matches a *request
shape* and maps it to a specific capability, with a field mapping
that translates the intent payload into the capability's input.

```rust
// core/crates/aaf-planner/src/fast_path.rs
pub struct FastPathRule {
    pub pattern: RequestPattern,
    pub target_capability: String,
    pub field_mapping: Vec<FieldMapping>,
    pub conditions: Vec<Condition>,
}

pub enum FastPathOutcome {
    Match {
        capability_id: String,
        mapped_request: serde_json::Value,
    },
    NoMatch,
}
```

Rules evaluate locally — **no round trip to the control plane**.
`aaf-sidecar::fast_path::LocalFastPath` owns the local rule set
and runs it against every inbound request before it reaches the
intent compiler.

---

## Where rules come from

- **Hand-authored** — operators write rules in
  `fast-path-rules.yaml` (schema under
  `spec/schemas/fast-path-rules.schema.json`).
- **Learned** (deferred to E1 Slice B) — the `fast_path_miner` in
  the forthcoming `aaf-learn` crate will observe recurring
  agent-assisted patterns and propose new rules, gated by the
  approval workflow before they become live.

Learned rules are tagged so the policy pack can disable them
wholesale if something goes wrong.

---

## Example rule

```yaml
rules:
  - id: fp-inventory-by-sku
    pattern:
      intent_type: transactional
      domain: warehouse
      has_field: [sku]
    target_capability: cap-inventory-check
    field_mapping:
      - from: sku
        to: skus[0]
    conditions:
      - field: sku
        op: matches
        value: '^[A-Z0-9-]{6,20}$'
```

When a request carries `intent_type = transactional`, `domain =
warehouse`, and a `sku` field matching the regex, the sidecar
routes directly to `cap-inventory-check` with no LLM involvement
and no planner round-trip.

---

## The degradation chain

Fast path is also the **floor** of the degradation chain. When the
control plane or the LLM provider is overloaded, the system steps
down through five levels:

```
Level 0: FULL AGENTIC       — LLM orchestration, dynamic planning
            ↓ LLM latency spike
Level 1: CACHED              — cached plans + small model adjustments
            ↓ LLM unavailable
Level 2: RULE-BASED          — predefined flows, rule-based branching
            ↓ runtime overloaded
Level 3: FAST PATH ONLY      — structured requests only, direct routing
            ↓ AAF layer failure
Level 4: BYPASS              — sidecar transparent proxy, no AAF processing
```

Levels 3 and 4 are what Rule 13 ("Sidecar transparent fallback")
is about: when AAF is unavailable, the sidecar still forwards
requests to the target service. The system degrades to "no AAF",
not "broken".

Each capability declares its own degradation levels in
`CapabilityContract.degradation`, which drives what happens when
*that capability* is sick, not when AAF is.

---

## Success criteria

- **Fast Path rate > 60%** in production traffic (Rule 4).
- **Intent Cache hit rate > 40%** on repeated goals.
- **Intent Resolution > 97%** across all patterns.
- **Sidecar overhead < 5ms p99** on the Fast Path.
- **LLM cost < $0.01 per intent** average.

The intent cache and fast-path miner are designed so these
targets can be met *without* manual rule authoring once E1
Slice B ships.

---

## Further reading

- [architecture.md](architecture.md) — the four patterns in the
  full flow
- [integration-microservices.md](integration-microservices.md) —
  the sidecar that runs fast-path rules locally
- `core/crates/aaf-planner/src/fast_path.rs` — the implementation
- `core/crates/aaf-planner/src/router.rs` — the pattern classifier
