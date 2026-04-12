# Observability

> How tracing, cost attribution, and OTel export work in AAF.
> If you are debugging a production incident, adding a new
> metric, or plugging AAF into a monitoring stack, this is the
> file to read.
>
> Source lives in `core/crates/aaf-trace/src/`.

---

## The shape of a trace

Every intent produces exactly **one** `ExecutionTrace`. The
trace is opened at the start of `GraphExecutor::run`, accrues
one `TraceStep` per node run, and is closed when the
execution terminates.

```
ExecutionTrace {
    trace_id,
    intent_id,
    status: TraceStatus,           // Open/Completed/Failed/Partial
    steps: Vec<TraceStep>,
    opened_at, closed_at,
}

TraceStep {
    step: u32,
    node_id,
    step_type: String,             // "node_run", "compensation", ...
    model: Option<String>,
    tokens_in, tokens_out,
    cost_usd, duration_ms,
    observation: Observation,      // the semantic record
}

Observation {
    trace_id, node_id, step,
    agent: String,
    observed: Vec<...>,
    reasoning: String,
    decision: String,
    confidence: f64,
    alternatives: Vec<...>,
    outcome: StepOutcome,          // Success/Failure/Partial/Paused
    recorded_at: DateTime<Utc>,
    outcome_detail: Option<Outcome>,  // E1 Slice A structured outcome
}

Outcome {
    status: OutcomeStatus,         // Succeeded/Failed/Partial/Escalated/RolledBack
    latency_ms: u64,
    tokens_used: u32,
    cost_usd: f64,
    policy_violations: Vec<PolicyViolation>,
    user_feedback: Option<UserFeedback>,
    downstream_error: Option<DownstreamError>,
    semantic_score: Option<SemanticScore>,
}
```

Rule 12 says **every decision records an Observation**. The
executor cannot be configured to skip tracing in production —
it is hard-wired.

---

## The `TraceRecorder` trait

```rust
#[async_trait]
pub trait TraceRecorder: Send + Sync {
    async fn open(&self, trace_id: TraceId, intent_id: IntentId) -> Result<(), RecorderError>;
    async fn record_step(&self, step: TraceStep) -> Result<(), RecorderError>;
    async fn record_observation(&self, ..., model, tokens_in, tokens_out, cost_usd, duration_ms, ...) -> Result<(), RecorderError>;
    async fn close(&self, trace_id: &TraceId, status: TraceStatus) -> Result<(), RecorderError>;
    async fn get(&self, trace_id: &TraceId) -> Result<ExecutionTrace, RecorderError>;
}
```

The default `Recorder` impl holds an `Arc<dyn TraceStore>` for
persistence (in-memory today; ClickHouse in production) plus a
`parking_lot::Mutex<HashMap<TraceId, ExecutionTrace>>` for
in-flight traces.

**Two ways to construct:**

```rust
let recorder = Recorder::in_memory();                   // for tests + dev
let recorder = Recorder::new(Arc::new(my_trace_store)); // with a real backend
```

---

## What happens per step

Inside `GraphExecutor::run`, for each node:

1. `started = Instant::now()`
2. `output = node.run(intent, &outputs).await?`
3. `elapsed_ms = started.elapsed().as_millis()`
4. `budget.charge(output.tokens, output.cost_usd, elapsed_ms)`
5. PolicyHook::PostStep check
6. `observation = Observation { ..., outcome_detail: Some(Outcome::minimal(...)) }`
7. `recorder.record_observation(observation, "node_run", cost_usd, duration_ms, tokens_in, tokens_out, model).await`

Every `record_observation` call ends up in the `ExecutionTrace`
under the same `trace_id`. The trace is then closed with one
of four statuses:

| Status | When |
|---|---|
| `Completed` | Every step succeeded |
| `Failed` | A policy violation or non-recoverable node error |
| `Partial` | Budget exhausted or pending approval |
| *n/a* | A revoked DID was rejected at Hook 0 — the trace is **never** opened |

---

## Cost attribution

`cost_attribution.rs` provides per-department cost rollups.
Operators add `AttributionRule`s (simple substring matches on
intent fields) and the attributor produces a
`CostAttribution` with one `CostBucket` per department.

### API

```rust
let mut attributor = CostAttributor::new("unassigned");
attributor.add_rule(AttributionRule {
    match_domain: Some("sales".into()),
    department: "revenue-ops".into(),
});
attributor.add_rule(AttributionRule {
    match_domain: Some("payments".into()),
    department: "finance".into(),
});

let attribution = attributor.attribute(&trace);
//   attribution.buckets: Vec<CostBucket {
//     department, cost_usd, token_count, step_count
//   }>
```

### When to use it

- Month-end chargeback — attribute AAF cost to owning teams.
- Incident forensics — "this incident cost $X; which department
  was the expensive one".
- Capability-level cost analysis — the buckets roll up per
  `CapabilityContract.domains`, so a capability that serves
  two departments will show up in both buckets proportionally.

### When not to use it

- Per-request billing — use the raw
  `TraceStep.cost_usd` instead. Attribution is a **rollup** over
  a trace, not a per-step metric.

---

## OTLP / OpenTelemetry export

`export.rs` converts an `ExecutionTrace` to OTLP/JSON spans
without depending on the heavy `opentelemetry` SDK.

### Why no SDK

The OTel SDK has a sprawling dep tree that has broken the
workspace at least twice in previous iterations. We ship a
hand-rolled JSON serializer that emits the same wire format
and can be fed to any OTLP collector that speaks JSON.

### API

```rust
let json = aaf_trace::export::otel_json_for(&trace);
// json is a serde_json::Value matching the OTLP ExportTraceServiceRequest
// schema. POST it at an OTLP collector's HTTP endpoint.
```

One `OtelSpan` per `TraceStep`, plus attributes pulled from the
`Observation`:

| OTel attribute | Source |
|---|---|
| `aaf.trace_id` | `trace.trace_id` |
| `aaf.intent_id` | `trace.intent_id` |
| `aaf.step` | `step.step` |
| `aaf.node_id` | `step.node_id` |
| `aaf.step_type` | `step.step_type` |
| `aaf.model` | `step.model` |
| `aaf.tokens_in` / `aaf.tokens_out` | `step.tokens_*` |
| `aaf.cost_usd` | `step.cost_usd` |
| `aaf.duration_ms` | `step.duration_ms` |
| `aaf.outcome` | `observation.outcome` |
| `aaf.confidence` | `observation.confidence` |

The `OtelStatus` is derived from `StepOutcome`:

- `Success` → `OtelStatus::Ok`
- `Failure` → `OtelStatus::Error`
- others → `OtelStatus::Unset`

### Wiring to a collector

Iteration 8's server wiring sends traces to an OTLP HTTP
endpoint at the end of every execution. For development, point
at a local Jaeger or a console collector:

```bash
docker run -d --name jaeger -p 16686:16686 -p 4318:4318 jaegertracing/all-in-one
# then configure aaf.yaml with:
# trace:
#   otlp:
#     endpoint: http://localhost:4318/v1/traces
```

---

## Metrics (light)

`metrics.rs` holds tiny aggregation helpers used by `aaf-eval`
and by the dashboard (deferred). The file is intentionally
small — the canonical metric source is the trace recorder, not
a separate counter registry.

If you want to expose Prometheus metrics, the hook is:

1. Subscribe to every `record_observation` via the (deferred)
   `TraceSubscriber` trait that E1 Slice B will add.
2. Increment counters and histograms in your subscriber.
3. Expose them through a `prometheus` crate-backed `/metrics`
   endpoint.

Slice B of E1 will also wire an `aaf-learn` subscriber that
listens on the same hook, so adding Prometheus is ~50 lines on
top of the subscriber contract.

---

## Debugging a production trace

1. Fetch the trace by id:
   ```
   let t = recorder.get(&trace_id).await?;
   ```
2. Look at `t.status`. If `Failed`, walk `t.steps` back from the
   end — the last step's `observation.outcome` is the failure.
3. Look at each step's `observation.outcome_detail`:
   - `policy_violations` tells you whether a policy rule fired.
   - `downstream_error` tells you whether the compensation chain
     triggered.
4. Cross-reference `trace_id` in the OTel collector for the
   rendered waterfall view (Jaeger / Tempo / etc.).

For **revoked DIDs**, note: the trace is **never opened** (X1
Slice B invariant R3 in `development/known-gotchas.md`). You
will not find the trace by `trace_id`; look for the revocation
entry in the `RevocationRegistry` instead.

---

## The E1 feedback spine hook

`aaf-eval` consumes `Observation.outcome_detail` to produce
`RegressionReport`s. The `Replayer` replays a trace against a
candidate configuration and compares divergence; the
`DeterministicJudge` assigns a Jaccard-based score to each
step's `semantic_score` field.

None of this modifies the hot path — the feedback spine reads
`outcome_detail` out-of-band, aggregates, and writes back
through the registry / router / fast-path-miner extension
points that E1 Slice B will add.

---

## Cost and token accounting gotchas

- **Self-report.** Every node is responsible for populating
  `NodeOutput.tokens` and `NodeOutput.cost_usd` accurately.
  `AgentNode` delegates to the `LLMProvider::chat` response;
  `DeterministicNode` reports zero.
- **Double-counting on `ForkNode`.** A fork of five children
  charges the budget tracker five times, once per child. Do
  not add a sixth charge for the fork itself — the fork's own
  `duration_ms` is counted, but its `tokens` and `cost_usd` are
  zero.
- **`u64::MAX` as a poison value.** `BudgetTracker::charge` uses
  `saturating_add`, so a buggy node reporting `u64::MAX` tokens
  will trip `Tokens` exhaustion on the very next step. If you
  see cost explosions, look for nodes reporting `u64::MAX`.

---

## Production checklist

Before shipping a deployment:

- [ ] `recorder` backend is a real store, not `in_memory()`
- [ ] OTLP endpoint configured and reachable
- [ ] Cost attribution rules populated per department
- [ ] Retention policy on the trace store (ClickHouse TTL or equivalent)
- [ ] Alert on `Trace.status = Failed` rate above a threshold
- [ ] Alert on `outcome_detail.cost_usd` above a per-intent cap
- [ ] Dashboards for: Fast-path rate, Intent resolution rate,
      LLM $/intent (rolling 7-day average), Compensation
      rollback rate, Revocation denial rate (X1 Slice B)

---

## Further reading

- [runtime-internals.md](runtime-internals.md) — the executor
  that drives the recorder
- [contracts-reference.md](contracts-reference.md) →
  `Observation` / `Outcome` sections
- `core/crates/aaf-trace/src/recorder.rs` — the implementation
- `core/crates/aaf-trace/src/cost_attribution.rs` — cost rollups
- `core/crates/aaf-trace/src/export.rs` — OTLP JSON export
