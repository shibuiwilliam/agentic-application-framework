# Integrating AAF with Microservices

> **Pattern A** from `PROJECT.md` §2.3 — drop AAF alongside an
> existing set of microservices as a **sidecar container**. The
> target services keep running unmodified (Rule 3). Every
> architectural component in this pattern lives in
> `aaf-sidecar`.

---

## The model

```
┌────────────────┐      ┌────────────────────┐
│   Front Door   │──▶── │     aaf-server     │
│  (chat / app)  │      │  (control plane)   │
└────────────────┘      └─────────┬──────────┘
                                  │ typed calls
          ┌───────────────────────┼───────────────────────┐
          │                       │                       │
          ▼                       ▼                       ▼
┌────────────────┐      ┌────────────────────┐   ┌──────────────┐
│ aaf-sidecar    │      │  aaf-sidecar       │   │ aaf-sidecar  │
│ └ service-A    │      │  └ service-B       │   │ └ service-C  │
└────────────────┘      └────────────────────┘   └──────────────┘
```

Every target service gets a sidecar container alongside it. The
sidecar:

1. Publishes the service's capabilities to the control-plane
   registry (`aaf-sidecar::capability::CapabilityPublisher`).
2. Intercepts inbound traffic, applies input / output / action
   guards (`aaf-sidecar::guard::LocalGuard`).
3. Evaluates local fast-path rules — if a request matches, it
   routes directly to the service without a control-plane
   round-trip (`aaf-sidecar::fast_path::LocalFastPath`).
4. On any AAF layer failure, forwards the request **directly** to
   the service (`aaf-sidecar::proxy::Proxy::forward_direct`).
   This is Rule 13 — the system degrades to "no AAF", not
   "broken".
5. Monitors upstream service health
   (`aaf-sidecar::health::UpstreamHealth`).

---

## Zero code changes to the target service

**Rule 3: Services Stay Untouched.** Every existing microservice
— whatever language, whatever framework — gets an AAF sidecar
without modifying its source. The sidecar is a **separate
process** that speaks the same protocol as the service (gRPC,
HTTP, whatever) and intercepts traffic at the network layer.

If a design requires modifying the target service, the design is
wrong. Go back and re-read `CLAUDE.md` Rule 3.

---

## Sidecar configuration

The sidecar reads a YAML config at startup. Full schema:
`spec/schemas/sidecar-config.schema.json`. Minimal example:
`spec/examples/sidecar-config-order.yaml`.

Structure:

```yaml
service:
  name: order-service
  endpoint: grpc://order-service:50051
  capabilities:
    - id: cap-order-create
      side_effect: write
      compensation:
        endpoint: cap-order-cancel
      reads:
        - entity_id: commerce.Customer
      writes:
        - entity_id: commerce.Order
      emits:
        - id: commerce.OrderPlaced

fast_path_rules:
  - id: fp-order-by-id
    pattern:
      intent_type: analytical
      has_field: [order_id]
    target_capability: cap-order-read

guards:
  input:
    injection_detection: true
    auth_required: true
  output:
    pii_detection: true
  action:
    scope_check: true
    side_effect_gate: true

degradation:
  on_control_plane_unreachable: forward_direct
  on_llm_unavailable: fast_path_only
```

Every field above is validated at startup; malformed configs
refuse to start the sidecar.

---

## The fast-path round-trip budget

Fast-path rules are evaluated **locally** in the sidecar with no
round-trip to the control plane. The overhead target is < 5 ms
p99 for a Fast Path match. See [fast-path.md](fast-path.md) for
the rule authoring guide.

For requests that do *not* match a fast-path rule, the sidecar
forwards to the control plane, which classifies the request,
plans, executes, and streams results back.

---

## Transparent fallback (Rule 13)

On any of these events the sidecar enters transparent-fallback
mode:

- Control plane becomes unreachable (TCP reset, 503, timeout).
- A required guard cannot run (e.g. policy plugin crash).
- The sidecar's own health check fails.

In transparent-fallback mode, every inbound request goes directly
to the target service via
`aaf-sidecar::proxy::Proxy::forward_direct`. The target service
is unaffected. When the control plane comes back, the sidecar
rejoins automatically.

**Invariant:** in transparent-fallback mode, the target service's
behaviour is indistinguishable from "no sidecar attached".

---

## How capabilities get published

At startup (or when the config is hot-reloaded), the sidecar
walks every capability declared in its config and calls
`Registry::register` against the control plane. Each
registration:

- Validates the contract (Rule 9 compensation gate).
- Attaches the sidecar's identity (X1 Slice A DID) if present.
- Indexes the capability under its `reads` / `writes` / `emits`
  fields so the planner's entity-aware discovery finds it.

If a capability fails validation, the sidecar logs the error and
**does not publish it** — the control plane never sees a
broken capability.

---

## The `aaf-server ontology lint` gate

Before you deploy a sidecar config, run:

```bash
aaf-server ontology lint ./sidecar-configs/
```

against the directory of capability YAMLs referenced by your
sidecar. The lint reports any capability missing entity
declarations; in strict mode (adoption ≥ 90%) it refuses writes
without a `writes:` declaration. See [ontology-lint.md](ontology-lint.md).

---

## Observability

Every sidecar inbound / outbound call produces an `Observation`
through `aaf-trace::Recorder`. Observations are exported via
OTLP (`aaf-trace::otel::OtlpExporter`) and carry:

- The trace id.
- The target capability id.
- The input + output digests.
- The cost + token usage.
- The policy decision.
- The outcome (E1 Slice A: attached at step-end).

---

## Deployment

- One sidecar container per target service. Typical topology:
  service and sidecar in the same Kubernetes Pod, sidecar
  listening on `localhost:<original-port>`, service bound to
  `127.0.0.1:<internal-port>`.
- Network policy: only the sidecar reaches the control plane;
  the service never calls AAF directly.
- Resource budget: < 50 MB memory, < 5% CPU on typical steady
  state. Heavy lifting (planning, running agent nodes) happens
  on the control plane.

---

## Further reading

- [architecture.md](architecture.md) — the three integration
  patterns in context
- [fast-path.md](fast-path.md) — authoring fast-path rules
- [security.md](security.md) — guards, rules, identity
- [ontology-lint.md](ontology-lint.md) — the lint gate
- `core/crates/aaf-sidecar/src/` — the implementation
- `spec/examples/sidecar-config-order.yaml` — canonical example
