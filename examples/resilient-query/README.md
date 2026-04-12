# resilient-query

Demonstrates AAF's **resilience and policy enforcement** features: fast-path
routing, input/output guards, degradation chain, budget enforcement, and
approval workflow. This is the "defense-in-depth" example — every request
passes through multiple layers of protection before producing results.

## What this example covers

| Feature | Where it's exercised |
|---|---|
| **Fast-path routing (Rule 4)** | Structured `AnalyticalIntent` with `period_ref = last_month` routes directly to `cap-sales-monthly` without LLM planning (<50ms p99) |
| **Fast-path miss** | Unmatched domain falls through to `NoMatch`, requiring full agentic planning |
| **Injection guard (Rule 7)** | `InputGuard` detects prompt-injection patterns and denies execution before the agent sees the payload |
| **PII guard (Rule 7)** | `OutputGuard` detects email, Japanese phone, and credit card patterns in outputs |
| **Degradation chain** | `DegradationStateMachine` cycles through all 4 levels: Full -> Partial -> Cached -> Unavailable -> recovery |
| **Budget enforcement (Rule 8)** | Token budget exhaustion returns `ExecutionOutcome::Partial` with completed steps preserved |
| **Approval workflow** | Write capability without `auto-approve` scope triggers `RequireApproval`; `ApprovalWorkflow` tracks the pending/approved lifecycle |
| **Runtime integration** | Clean analytics graph executes with full trace recording and outcome details |

## Files

```
examples/resilient-query/
├── README.md              <- this file
├── aaf.yaml               <- capability seeds + budget config
└── fast-path-rules.yaml   <- fast-path rule definitions for direct routing
```

## Run the tests

```bash
cargo test -p aaf-integration-tests --test resilient_query_e2e
```

Expected output:

```text
running 15 tests
test degradation_chain_cycles_through_all_levels ... ok
test degradation_partial_then_recover ... ok
test fast_path_matches_structured_sales_query ... ok
test fast_path_misses_when_domain_unsatisfied ... ok
test fast_path_yaml_loads_successfully ... ok
test injection_guard_blocks_prompt_injection ... ok
test injection_guard_allows_clean_payload ... ok
test pii_guard_flags_email_in_output ... ok
test pii_guard_flags_japanese_phone_in_output ... ok
test pii_guard_allows_clean_output ... ok
test action_guard_requires_approval_for_write_without_auto_approve ... ok
test action_guard_allows_write_with_auto_approve ... ok
test budget_exhaustion_returns_partial_with_completed_steps ... ok
test clean_analytics_query_completes_with_trace ... ok
test runtime_blocks_injection_at_pre_plan ... ok

test result: ok. 15 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

You can also run the basic intent -> plan -> execute pipeline:

```bash
cargo run -p aaf-server -- run examples/resilient-query/aaf.yaml
```

## What each test proves

### 1. `fast_path_matches_structured_sales_query`

A structured `AnalyticalIntent` with `period_ref = "last_month"` in the
`analytics` domain matches the first fast-path rule and routes directly to
`cap-sales-monthly`. The field mapping converts `period_ref` to `period` in
the mapped request. No LLM call is needed.

### 2. `fast_path_misses_when_domain_unsatisfied`

A query in the `finance` domain does not match any rule (all rules require
`analytics`), so it falls through to `NoMatch`. This request would proceed
to the full agentic planning pipeline.

### 3-4. `injection_guard_blocks_prompt_injection` / `injection_guard_allows_clean_payload`

Four classic injection patterns ("ignore all instructions", "disregard system
prompt", "you are now", "pretend to be") are tested against the `InputGuard`.
All are detected and denied. A normal analytics query passes cleanly.

### 5-7. `pii_guard_flags_email_in_output` / `pii_guard_flags_japanese_phone_in_output` / `pii_guard_allows_clean_output`

The `OutputGuard` catches PII in agent outputs:
- Email addresses (`tanaka@example.com`)
- Japanese mobile numbers (`090-1234-5678`)

Aggregated outputs without PII (revenue totals, region names) pass through.

### 8-9. `degradation_chain_cycles_through_all_levels` / `degradation_partial_then_recover`

The `DegradationStateMachine` transitions monotonically:
- **Degrade:** Full -> Partial -> Cached -> Unavailable (further degrade returns `None`)
- **Recover:** Unavailable -> Cached -> Partial -> Full (further recover returns `None`)

This models real-world scenarios: LLM latency spike -> cached plans -> rule-based
fallback -> full bypass -> gradual recovery.

### 10. `budget_exhaustion_returns_partial_with_completed_steps`

A two-node graph where the second node consumes 2,000 tokens against a budget
of 100. The first (cheap) node completes and its output is preserved. The
executor returns `ExecutionOutcome::Partial` — graceful degradation rather
than total failure.

### 11-12. `action_guard_requires_approval_for_write_without_auto_approve` / `action_guard_allows_write_with_auto_approve`

The `ActionGuard` gates write operations through the side-effect gate:
- Without `auto-approve` scope: returns `RequireApproval` with a violation
  from the `side-effect-gate` rule. The `ApprovalWorkflow` then tracks the
  pending -> approved lifecycle.
- With `auto-approve` scope: the same operation is allowed immediately.

### 13. `clean_analytics_query_completes_with_trace`

A single-step analytics graph executes with full policy enforcement (4 hooks)
and trace recording. The trace contains the step with `outcome_detail`
confirming E1 Feedback Spine integration.

### 14. `runtime_blocks_injection_at_pre_plan`

An injection pattern embedded in the intent's `goal` field is caught at the
`PrePlan` hook before graph execution begins. The graph never runs.

### 15. `fast_path_yaml_loads_successfully`

Loads `examples/resilient-query/fast-path-rules.yaml` and verifies:
- 2 rules parse correctly
- Rule 1 targets `cap-sales-monthly`
- Rule 2 targets `cap-customer-lookup`

## Architecture rules exercised

| Rule | How |
|---|---|
| **Rule 4** (Fast Path First) | Structured queries route directly via `FastPathRuleSet` |
| **Rule 6** (Policy at every step) | 4 hooks (PrePlan, PreStep, PostStep, PreArtifact) run per execution |
| **Rule 7** (Guard every agent) | InputGuard (injection), OutputGuard (PII), ActionGuard (side-effect) |
| **Rule 8** (Depth and budget limits) | Token budget exhaustion returns partial results |
| **Rule 12** (Trace everything) | Every step records an Observation with outcome_detail |

## See also

- `PROJECT.md` -- degradation chain design (5 levels)
- `PROJECT.md` -- communication pattern classification (Fast Path first)
- `CLAUDE.md` -- performance targets for each pattern
- `examples/hello-agent/` -- simplest AAF pipeline (read-only)
- `examples/order-saga/` -- multi-step saga with compensation
- `examples/signed-agent/` -- identity + provenance CLI walkthrough
