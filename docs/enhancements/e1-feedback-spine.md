# E1 — Feedback Spine

> **Status:** Slices A + B complete. Slice C pending.
>
> **Rules enforced:** R15 Feedback is a contract. R16 Learning
> never touches the hot path. R17 Every adaptation is reversible.
> R18 Policy governs learning.
>
> **Authoritative design:** `PROJECT.md` §16.1 +
> `CLAUDE.md` rules 15-18. This page is a reader-facing summary.

---

## The problem

Before E1, `aaf-trace` wrote `Observation`s — and **nothing read
them back**. There was no path from a production trace into
routing, capability scoring, fast-path rule generation, or
semantic regression detection. Consequences:

- **Fast Path rate cannot hit >60% by hand-writing rules.** Rule 4
  mandates > 60% but humans will never author enough rules, or
  keep them fresh.
- **LLM $/intent cannot trend down.** Without observed
  cost/quality data, the router cannot pick cheaper models on
  easy intents.
- **Regressions are silent.** A prompt or model change can
  degrade quality and AAF has no way to notice before the next
  production incident.
- **A framework that calls itself "agentic" but behaves
  statically from day one is 2023-era thinking.**

---

## The solution

Close the loop. Every `Observation` must have a path back into
at least one of: the LLM router, the capability registry, the
planner's fast-path miner, the evaluation harness, or the
approval workflow. The system must measurably improve over time
on two axes: **cost** ($/intent trending down) and **quality**
(intent resolution rate staying ≥ 97%, fewer rollbacks, fewer
escalations).

---

## What landed

### Slice A — contracts + crate skeleton (iteration 4)

| Deliverable | Location |
|---|---|
| `Outcome` contract attached to every `Observation` | `aaf-contracts::observation::Outcome` |
| `OutcomeStatus { Succeeded, Failed, Partial, Escalated, RolledBack }` | `aaf-contracts::observation` |
| `UserFeedback { Rating, EditDistance, FreeText }` | `aaf-contracts::observation` |
| `DownstreamError`, `SemanticScore` | `aaf-contracts::observation` |
| `aaf-eval` crate | `core/crates/aaf-eval/` |
| `Judge` trait + `DeterministicJudge` (Jaccard) | `aaf-eval::judge` |
| `GoldenSuite` YAML loader + runner | `aaf-eval::golden` |
| `Replayer` (cost/latency drift + status change detection) | `aaf-eval::replay` |
| `RegressionReport` + `ReportWriter` | `aaf-eval::regression`, `::report` |
| Runtime attaches a minimal `Outcome` at step-end | `aaf-runtime::executor` uses `Outcome::minimal(...)` |
| Cost attribution helper | `aaf-trace::cost_attribution::{CostAttributor, CostBucket}` |
| OTLP JSON export | `aaf-trace::export::otel_json_for` |
| `eval-suite-order-processing.yaml` example | `spec/examples/` |

### Slice B — `aaf-learn` crate + subscribers (LANDED)

| Deliverable | Location |
|---|---|
| `aaf-learn` crate with 4 subscriber modules | `core/crates/aaf-learn/` |
| `FastPathMiner` — proposes new fast-path rules from agent-assisted traffic | `aaf-learn::fast_path_miner` |
| `CapabilityScorer` — outcome-weighted reputation per capability | `aaf-learn::capability_scorer` |
| `RouterTuner` — per-`(intent_type, risk_tier, entity_class)` model weights | `aaf-learn::router_tuner` |
| `EscalationTuner` — approval threshold adjustments within policy bounds | `aaf-learn::escalation_tuner` |
| `TraceSubscriber` trait + spawn-based fan-out | `aaf-trace::recorder::Recorder` |
| `RoutingPolicy` trait + `LearnedRoutingPolicy` impl | `aaf-llm::router` |
| Learned fast-path rules tagged, policy-pack-disable-able | `aaf-planner::fast_path` |
| Smoke test | `core/tests/integration/tests/e1_slice_b_smoke.rs` |

### Slice C — CLI, regression gate, docs (deferred)

- `aaf learn` CLI subcommand (list proposals, approve, inspect).
- `make test-semantic-regression` target.
- Governance docs for learned adaptation.
- Production anomaly detection on evidence concentration.

---

## The outcome contract

```
Outcome {
    status: OutcomeStatus,         // Succeeded / Failed / Partial / Escalated / RolledBack
    latency_ms: u64,
    tokens_used: u32,
    cost_usd: f64,
    policy_violations: Vec<PolicyViolation>,
    user_feedback: Option<UserFeedback>,
    downstream_error: Option<DownstreamError>,
    semantic_score: Option<SemanticScore>,
}
```

Rule 15 says this is the **single canonical location** for
outcome data. Every writer (runtime, saga, Front Door, app
surface, eval harness) and every reader (aaf-learn, dashboards,
CI regression gate) shares the same shape.

---

## Non-hot-path invariant (Rule 16)

`TraceSubscriber::on_observation` is called from
`tokio::spawn` so the executor **never** awaits a subscriber.
The runtime's hot path is completely decoupled from learning:
adding a subscriber cannot slow down a production intent.

Slice B ships a `subscriber_is_not_on_hot_path` unit test that
asserts `record_observation` returns **before** the subscriber
completes.

---

## Reversibility (Rule 17)

Every adaptation carries a `LearnedRuleRef` with `learned_by`,
`learned_at`, and `evidence` fields. The policy engine can
roll any adaptation back by rule id. Learned fast-path rules
are tagged so a policy pack can disable them wholesale.

---

## Governance (Rule 18)

`aaf-learn` never touches policy rules themselves. It can
*propose* tightening thresholds; adoption requires the same
approval workflow as any other policy change. Learned
fast-path rules also go through the approval workflow before
they become live.

---

## Rules

| Rule | How E1 enforces it |
|---|---|
| **R15** Feedback is a contract | `Observation.outcome_detail: Option<Outcome>` — single location |
| **R16** Learning never touches the hot path | `TraceSubscriber` fan-out is `tokio::spawn`-based |
| **R17** Every adaptation is reversible | Every learned change carries `LearnedRuleRef` metadata |
| **R18** Policy governs learning | Adaptations pass through `ApprovalWorkflow` |

---

## Success criteria (over 4 weeks of production)

- **Fast Path rate** climbs from cold-start baseline to > 60%
  *without manual rule authoring*, driven by the miner.
- **$/intent** decreases by at least 20%, driven by router
  tuning, while **intent resolution rate** stays ≥ 97%.
- **Every router/registry adaptation** is visible in the Trace
  Explorer with evidence and one-click rollback.
- **CI runs `aaf-eval`** against the golden set on every merge;
  regressions block the merge.

---

## Safety rails

- **No online learning writes to production policy rules.** Policy
  remains governed by the policy pack. `aaf-learn` can propose;
  adoption requires approval.
- **Bounded adaptation rate.** Router weights and reputation
  scores are rate-limited per unit time to prevent oscillation
  or adversarial manipulation.
- **Adversarial traffic detection.** The fast-path miner rejects
  patterns whose evidence is concentrated in a suspiciously
  small set of sessions or tenants.

---

## Further reading

- [`../../development/next-slices.md`](../../development/next-slices.md)
  → Slice 1 — E1 Slice C (CLI, semantic regression, governance)
- [`../../development/observability.md`](../../development/observability.md)
- [`../../development/contracts-reference.md`](../../development/contracts-reference.md)
  → `Observation` / `Outcome` sections
- `PROJECT.md` §16.1
- `core/crates/aaf-eval/src/` — the Slice A harness
- `core/crates/aaf-learn/src/` — the Slice B learning subscribers
