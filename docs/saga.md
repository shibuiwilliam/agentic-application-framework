# Agentic Saga

> AAF's saga engine extends the traditional Saga pattern with
> **intelligent failure recovery**. This document summarises the
> concept; the code lives in `core/crates/aaf-saga/src/`.

---

## Traditional Saga vs Agentic Saga

**Traditional Saga:**

```
step 1 (write A)   →  ok
step 2 (write B)   →  ok
step 3 (write C)   →  ✗ fails
                     ↓
compensate step 2, compensate step 1   (everything in reverse)
```

Every failure triggers a full compensation chain in reverse. The
saga author must choose between *all* or *nothing*.

**Agentic Saga:**

```
step 1 (write A)   →  ok
step 2 (write B)   →  ok
step 3 (write C)   →  ✗ fails with reason "address invalid"
                     ↓
analyse cause → choose optimal recovery:
   - pause_and_ask_user   (preserve steps 1 & 2)
   - retry_with_alternative
   - partial_compensation (undo only step 3's side effects)
   - full_compensation    (the traditional path)
```

The saga author declares a set of `RecoveryRule`s per failure
mode. The runtime picks the best option and walks only the
necessary compensation.

---

## Saga definition

`spec/examples/saga-order-processing.yaml` is the canonical
example. Minimal structure:

```yaml
saga:
  name: order-processing
  steps:
    - step: 1
      name: "stock reservation"
      type: deterministic
      capability: cap-stock-reserve
      compensation: cap-stock-release
      compensation_type: mandatory

    - step: 2
      name: "payment execution"
      type: deterministic
      capability: cap-payment-execute
      compensation: cap-payment-refund
      compensation_type: mandatory

    - step: 3
      name: "shipping arrangement"
      type: agent
      capability: cap-shipping-arrange
      on_failure:
        strategy: intelligent_recovery
        rules:
          - condition: "address invalid"
            action: pause_and_ask_user
            preserve: [step-1, step-2]
          - condition: "carrier temporarily unavailable"
            action: retry_with_alternative
            preserve: [step-1, step-2]
          - condition: "oversized package"
            action: full_compensation
```

---

## The saga state machine

```
initiated → running → step_N_failed → analyzing → recovery_selected
   ↓                                      ↓                ↓
   ↓                                      ↓    partial_compensation → waiting_for_input → resumed
   ↓                                      ↓                               ↓
   ↓                                      ↓                         full_compensation → saga_failed
   ↓                                      ↓                         retry → (back to running)
   ↓                                      ↓                                       ↓
   saga_completed                                                               saga_failed
```

Legal transitions are defined in
`aaf-saga::state::SagaStateMachine`. Terminal states are
`saga_completed` and `saga_failed`.

---

## Recovery actions

| Action | What it does |
|---|---|
| `retry_with_alternative` | Retry the step against a different capability (discovered via the registry's `discover_by_entity` / fallback lookup) |
| `pause_and_ask_user` | Move to `waiting_for_input`; the surface layer prompts the user |
| `partial_compensation` | Undo only the explicitly-listed steps |
| `full_compensation` | The traditional rollback — undo everything in reverse |
| `escalate` | Mark the saga as needing human review, preserve all state |

---

## Bridge to runtime + registry

`aaf-saga::bridge::StepRunner` produces a `StepRunner` given a
`Registry` handle + a `RegistryClient`. The runner knows how to
invoke each step's declared capability through the runtime and
honour the compensation metadata. This is what lets a saga walk a
real execution plan rather than just a static YAML.

---

## Writing a new saga

1. Declare the saga YAML in `spec/examples/` (or wherever your
   operator config lives). Validate it against the schema:

   ```bash
   make schema-validate
   ```

2. Register every referenced capability in the registry. Run
   `cargo run -p aaf-server -- ontology lint` to make sure
   capabilities carry `reads`/`writes`/`emits` declarations.

3. Load the saga at runtime via `SagaDefinition::from_yaml`.

4. Run it via `SagaExecutor::run`. The executor walks the steps,
   calls `StepRunner` for each, records observations through the
   trace recorder, and on failure hands off to the recovery
   analyser.

5. Inspect outcomes in the trace explorer (once E3 Slice C UI
   ships; until then, walk the `Trace` returned by
   `Recorder::get`).

---

## Invariants

- Every step with a write-class `side_effect` **must** carry a
  compensation. This is enforced by `CapabilityContract::validate`
  at registration time (Rule 9).
- The saga executor records an `Observation` per step through
  `aaf-trace::Recorder` (Rule 12).
- `analyzing` → `recovery_selected` is the only way to leave the
  failed branch; the transition is deterministic given the
  condition string.

---

## Further reading

- [policies.md](policies.md) — the policy hooks the saga respects
- [contracts.md](contracts.md) — `CapabilityContract.compensation`,
  `Task` state machine
- `core/crates/aaf-saga/src/` — the source
- `spec/examples/saga-order-processing.yaml` — canonical example
