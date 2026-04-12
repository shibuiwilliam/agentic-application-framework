# order-saga

Demonstrates AAF's **Agentic Saga** — the most important story in
the framework: a multi-step order flow where failure at a late step
triggers intelligent compensation of earlier steps, all under policy
enforcement and full trace recording.

## What this example covers

| Feature | Where it's exercised |
|---|---|
| **Multi-step graph execution** | 3-node DAG: stock check → payment → shipping |
| **Deterministic vs agent nodes** | Stock check + payment are deterministic (Rule 5); shipping is agent-class |
| **Saga compensation rollback** | Shipping failure at step 3 triggers payment refund at step 2; stock check is preserved (read-only, no compensation needed) |
| **Policy enforcement at every step** | Side-effect gate, scope check, PII guard, injection guard all run at 4 hooks per step (Rule 6) |
| **Shadow mode** | The same graph re-runs with `with_shadow()` — write nodes produce `{"shadow": true}` without executing, while read nodes still run (for phased adoption, PROJECT.md §19.6) |
| **Outcome tracking** | Every step's trace entry carries a structured `outcome_detail` with status, latency, tokens, and cost (E1 Feedback Spine) |
| **Saga YAML definition** | `saga.yaml` is loaded and parsed, demonstrating the intelligent recovery rules from PROJECT.md §Agentic Saga |

## Files

```
examples/order-saga/
├── README.md       ← this file
├── aaf.yaml        ← capability seeds + budget config
└── saga.yaml       ← saga definition with 3 steps + intelligent recovery rules
```

## Run the tests

```bash
cargo test -p aaf-integration-tests --test order_saga_e2e
```

Expected output:

```text
running 4 tests
test happy_path_completes_three_steps_with_trace ... ok
test shipping_failure_compensates_payment_only ... ok
test shadow_mode_records_but_does_not_execute_writes ... ok
test saga_yaml_parses_successfully ... ok

test result: ok. 4 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

You can also run the basic intent → plan → execute pipeline using the
`aaf-server` binary:

```bash
cargo run -p aaf-server -- run examples/order-saga/aaf.yaml
```

## What each test proves

### 1. `happy_path_completes_three_steps_with_trace`

Runs the full 3-step graph. Verifies:
- All 3 steps complete (`ExecutionOutcome::Completed`)
- Stock check returns `available: 42`
- Payment returns `status: captured`
- Shipping returns `tracking: JP-12345`
- Trace has 3 recorded steps, each with `outcome_detail`

### 2. `shipping_failure_compensates_payment_only`

Step 3 (shipping) fails with `"address_invalid"`. Verifies:
- The outcome is `ExecutionOutcome::RolledBack`
- `failed_at` is step 3
- The `reason` mentions `"address_invalid"`
- The `compensated` list contains exactly `["payment"]` — the stock
  check was read-only so it has no compensator and is correctly
  preserved

This demonstrates AAF's intelligent recovery from PROJECT.md §Agentic
Saga: instead of rolling back everything, only the payment is
compensated because the stock reservation is still valid.

### 3. `shadow_mode_records_but_does_not_execute_writes`

Runs the same happy-path graph with `with_shadow()` enabled:
- Read node (`stock-check`) **executes normally** → `available: 42`
- Write nodes (`payment`, `shipping`) produce `{"shadow": true, "would_have_run": "..."}` **without executing**
- The trace still records all 3 steps (full observability even in shadow)

This is the phased-adoption story from PROJECT.md §19.6:
deploy AAF in shadow mode first, compare its decisions against the
existing system, and cut over once the agreement rate exceeds 95%.

### 4. `saga_yaml_parses_successfully`

Loads `examples/order-saga/saga.yaml` and verifies:
- 3 steps parse correctly
- Step 3 has `on_failure` with `intelligent_recovery` strategy
- 3 recovery rules: `address_invalid → pause_and_ask_user`,
  `carrier_temporary_outage → retry_with_alternative`,
  `oversize_package → full_compensation`

## Architecture rules exercised

| Rule | How |
|---|---|
| **Rule 5** (Deterministic Core) | Stock check + payment are deterministic nodes; shipping is agent-class |
| **Rule 6** (Policy at every step) | 4 hooks × 3 steps = 12 policy evaluations per run |
| **Rule 8** (Budget limits) | Budget tracker charges cost at every step |
| **Rule 9** (Compensation before implementation) | Payment has `cap-payment-refund` compensator; shipping has `cap-shipping-cancel` |
| **Rule 12** (Trace everything) | Every step emits an `Observation` with `outcome_detail` |
| **Rule 13** (Sidecar transparent fallback) | Shadow mode is the agentic equivalent of degradation level 4 |

## See also

- `PROJECT.md` §"Agentic Saga" — design rationale for intelligent
  recovery
- `PROJECT.md` §19.4 — fusion of the Saga pattern with
  AAF
- `PROJECT.md` §19.6 — shadow mode for phased adoption
- `PROJECT.md` §19.10 — the e-commerce use case this
  example implements
- `examples/hello-agent/` — simplest possible AAF example (read-only)
- `examples/signed-agent/` — identity + provenance CLI walkthrough
