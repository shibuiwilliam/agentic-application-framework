# Learning Pipeline — `aaf-learn` + `aaf-eval`

> How the feedback spine works end-to-end: recording outcomes,
> evaluating quality, proposing adaptations, and approving learned
> rules. This document covers the E1 enhancement (Slices A + B)
> and the operational patterns for running the evaluation harness.

---

## Overview

The learning pipeline closes the loop between production
observations and system adaptation:

```
  ┌─────────────────┐
  │   aaf-runtime    │  Step-end: attach Outcome to Observation
  │   (executor)     │──────────────────────────────────────────┐
  └─────────────────┘                                          │
                                                               ▼
  ┌─────────────────┐     ┌──────────────────────────────────────────┐
  │   aaf-trace      │────▶│   TraceSubscriber::on_observation (spawn) │
  │   (recorder)     │     │                                          │
  └─────────────────┘     │   ┌─────────────────────┐                │
                          │   │  FastPathMiner       │ → proposes     │
                          │   │  CapabilityScorer    │ → reputation   │
                          │   │  RouterTuner         │ → weights      │
                          │   │  EscalationTuner     │ → thresholds   │
                          │   └─────────────────────┘                │
                          └──────────────────────────────────────────┘
                                           │
  ┌─────────────────┐                      │ Adaptations write back:
  │  aaf-planner     │◀─── learned fast-path rules
  │  aaf-llm         │◀─── routing weight adjustments
  │  aaf-registry    │◀─── capability reputation
  │  aaf-policy      │◀─── escalation threshold hints
  └─────────────────┘
```

**Key invariant (Rule 16):** `TraceSubscriber::on_observation` is
called from `tokio::spawn`. The executor **never** awaits a
subscriber. Adding a subscriber cannot slow down a production
intent.

---

## The four subscribers

### `FastPathMiner`

**Location:** `core/crates/aaf-learn/src/fast_path_miner.rs`

Watches agent-assisted observations for recurring patterns. When
the same `(intent_type, domain, constraint-key-set)` tuple recurs
more than `threshold` times across at least `min_distinct_sessions`
sessions, proposes a new `LearnedRule`.

**Configuration:**

```rust
MinerConfig {
    threshold: 10,            // observations before proposal
    min_distinct_sessions: 3, // prevents adversarial concentration
}
```

**Adversarial detection (Rule 18):** A single user replaying the
same intent across one session cannot trigger a proposal — the
`min_distinct_sessions` guard rejects concentrated evidence.

**Output:** `Vec<LearnedRule>` with `approval_state: Proposed`.
Rules must pass the `ApprovalWorkflow` before activation.

### `CapabilityScorer`

**Location:** `core/crates/aaf-learn/src/capability_scorer.rs`

Nudges a per-agent reputation score based on outcome status:

- `Succeeded` → `+success_delta` (default `0.02`)
- `Failed` / `RolledBack` → `-failure_delta` (default `0.05`)
- `Partial` / `Escalated` → no change

Score is clamped to `[0.0, 1.0]`, starts at `0.5` (neutral).

### `RouterTuner`

**Location:** `core/crates/aaf-learn/src/router_tuner.rs`

Accumulates per-`(intent_type, risk_tier)` statistics:
`count`, `total_cost`, `successes`. Exposes `success_rate()`
and `avg_cost()` for each bucket.

The caller reads these stats and installs weight adjustments
into a `LearnedRoutingPolicy` (in `aaf-llm::router`).

### `EscalationTuner`

**Location:** `core/crates/aaf-learn/src/escalation_tuner.rs`

Tracks escalation frequency: `total`, `escalated`, and
`escalated_then_succeeded`. Exposes `escalation_rate()` and
`false_escalation_rate()`.

High `false_escalation_rate` suggests the approval threshold is
too aggressive — the tuner can recommend relaxing it within
policy-pack bounds.

---

## Wiring subscribers into the recorder

```rust
use aaf_learn::{FastPathMiner, CapabilityScorer, RouterTuner, EscalationTuner};
use aaf_learn::fast_path_miner::MinerConfig;
use aaf_trace::Recorder;

let miner = Arc::new(FastPathMiner::new(MinerConfig::default()));
let scorer = Arc::new(CapabilityScorer::new(Default::default()));
let tuner = Arc::new(RouterTuner::new());
let escalation = Arc::new(EscalationTuner::new());

let recorder = Recorder::in_memory()
    .with_subscriber(miner.clone())
    .with_subscriber(scorer.clone())
    .with_subscriber(tuner.clone())
    .with_subscriber(escalation.clone());
```

Every call to `recorder.record_observation(...)` fans out to all
subscribers via `tokio::spawn` after persisting the observation.

---

## The `LearnedRule` contract

```rust
// aaf-contracts::learn
pub struct LearnedRule {
    pub id: String,
    pub source: LearnedSource,        // Miner | Scorer | Router | Escalation
    pub evidence: Vec<IntentId>,
    pub scope: String,
    pub approval_state: LearnedApprovalState,  // Proposed | Approved | Rejected
    pub learned_at: String,
}
```

**Rule 17:** Every learned change carries `(learned_by, learned_at,
evidence)` and can be rolled back by id.

**Rule 18:** `approval_state` starts as `Proposed`. The rule
becomes live only after explicit approval through the
`ApprovalWorkflow`.

---

## The evaluation harness (`aaf-eval`)

### Judge trait

```rust
pub trait Judge: Send + Sync {
    fn judge(&self, expected: &str, actual: &str) -> JudgeVerdict;
}
```

`DeterministicJudge` uses Jaccard similarity over whitespace-
tokenized words. Reproducible in CI without external state.

### Golden suite

A YAML file listing `(intent, expected_output)` pairs:

```yaml
name: ecommerce-golden
threshold: 0.5
cases:
  - id: place-order
    intent: place an order for SKU-1
    expected: order placed for SKU-1
  - id: stock-query
    intent: check stock for SKU-1
    expected: stock level for SKU-1 is 42
    min_score: 0.7   # per-case override
```

`GoldenSuite::load_from_yaml(path)` loads and validates.

### Regression detection

`RegressionReport` compares a baseline run against a candidate:

```rust
let report = RegressionReport::compare(&baseline, &candidate, threshold);
for delta in &report.deltas {
    // delta.case_id, delta.baseline_score, delta.candidate_score
}
```

### Replay

`Replayer` replays a trace against a candidate configuration and
surfaces divergence:

```rust
let divergences = replayer.replay(&trace_a, &trace_b);
// divergences: Vec<Divergence { step, expected, actual }>
```

---

## The `eval-golden` example

**Location:** `examples/eval-golden/`

Demonstrates:
1. Loading a golden suite from `golden-suite.yaml`
2. Running each case through the intent compiler
3. Scoring with `DeterministicJudge`
4. Generating a `RegressionReport`

**Run it:**
```bash
cargo test -p aaf-integration-tests --test eval_golden_e2e
```

**Config (`aaf.yaml`):**
```yaml
eval:
  golden_suite: golden-suite.yaml
  judge: deterministic-jaccard
  replay:
    cost_tolerance_usd: 0.001
    latency_tolerance_ms: 100
```

---

## Integration test coverage

| Test file | What it proves |
|---|---|
| `e1_feedback_smoke.rs` | Outcome attached to every observation; Judge scores; GoldenSuite loads |
| `e1_slice_b_smoke.rs` | Miner proposes after threshold; approval workflow; learned rule activates |
| `feedback_loop_e2e.rs` | Trust lifecycle + learning subscribers wired together |
| `eval_golden_e2e.rs` | Golden suite → judge → regression report end-to-end |

---

## What's next (E1 Slice C)

- `aaf learn` CLI subcommand: `list`, `approve`, `reject`, `inspect`
- `make test-semantic-regression` Makefile target
- Governance docs (`docs/learning-governance.md`)
- Production-grade anomaly detection with sliding-window stats

See `development/next-slices.md` → Slice 1 for the concrete
playbook.

---

## Rules enforced

| Rule | Where |
|---|---|
| **R15** Feedback is a contract | `Observation.outcome_detail: Option<Outcome>` |
| **R16** Learning never touches the hot path | `tokio::spawn` in `Recorder::record_observation` |
| **R17** Every adaptation is reversible | `LearnedRule` carries evidence + rollback |
| **R18** Policy governs learning | `ApprovalWorkflow` gates learned rules |

---

## Wave 4 interactions

The learning pipeline becomes significantly more valuable once
Wave 4 F2 (Live LLM Integration) lands:

- **RouterTuner** can adjust real model routing (not just
  `MockProvider` weights) based on observed cost/quality per
  intent type and risk tier.
- **CapabilityScorer** can factor in actual LLM response quality
  (not mock responses) when scoring capability reputation.
- **`aaf-eval` with real providers:** Golden suites can be
  evaluated against live LLMs (gated behind
  `AAF_LIVE_LLM_TEST=1`), enabling genuine semantic regression
  detection.
- **ProviderMetrics (Rule 35):** Every LLM call records real
  token counts and costs, feeding precise data into the
  `RouterTuner` and `BudgetTracker`.

---

## Further reading

- [observability.md](observability.md) — how traces and costs work
- [extension-points.md](extension-points.md) → "New fast-path rule
  source" and "New judge for `aaf-eval`"
- [contracts-reference.md](contracts-reference.md) → `Observation` /
  `Outcome` sections
- `core/crates/aaf-learn/src/lib.rs` — crate-level doc comment
- `core/crates/aaf-eval/src/` — the evaluation harness
- `PROJECT.md` §20 — Wave 4 design for LLM integration
