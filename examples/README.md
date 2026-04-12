# AAF Examples

Eight runnable examples that demonstrate progressively deeper AAF
functionality. Start with `hello-agent`, then `order-saga`, then
`resilient-query`, then `feedback-loop`, then `memory-context`,
then `app-native-surface`, then `cross-cell-federation`, then
`signed-agent`.

## Quick start

```bash
# 1. Simplest possible: read-only intent → plan → execute
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml

# 2. Multi-step saga with compensation, shadow mode, and policy
cargo test -p aaf-integration-tests --test order_saga_e2e

# 3. Resilience: fast-path, guards, degradation, budget, approval
cargo test -p aaf-integration-tests --test resilient_query_e2e

# 4. Trust lifecycle + learning feedback loop
cargo test -p aaf-integration-tests --test feedback_loop_e2e

# 5. Four-layer memory model + context budget (Rule 10)
cargo test -p aaf-integration-tests --test memory_context_e2e

# 6. App-native surface: events, proposals, projections
cargo test -p aaf-integration-tests --test app_native_surface_e2e

# 7. Cross-cell federation: routing, data boundaries, co-signed tokens
cargo test -p aaf-integration-tests --test cross_cell_federation_e2e

# 8. Agent identity: sign manifest, export SBOM, verify, revoke
cargo run -p aaf-server -- identity verify examples/signed-agent/manifest.yaml
```

## Examples

### [`hello-agent/`](hello-agent/)

The smallest end-to-end AAF example. Seeds two read-only
capabilities, compiles a natural-language goal into an
`IntentEnvelope`, plans against the registry, executes the graph,
and prints the trace. No write side-effects, no saga, no identity —
just the core pipeline.

**Demonstrates:** intent compilation, capability discovery, graph
execution, trace recording.

**Run:**
```bash
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml
```

---

### [`order-saga/`](order-saga/)

The canonical AAF story: a 3-step e-commerce order flow (stock check
→ payment → shipping) with saga compensation when shipping fails,
shadow mode for phased adoption, and full policy enforcement.

**Demonstrates:** multi-step graph execution, deterministic vs agent
nodes (Rule 5), saga compensation rollback (Rule 9), policy at
every step (Rule 6), shadow mode (PROJECT_AafService §6.2), outcome
tracking (E1 Feedback), saga YAML definition parsing.

**Run:**
```bash
cargo test -p aaf-integration-tests --test order_saga_e2e
```

4 tests exercise: happy path, compensation rollback, shadow mode,
and saga YAML parsing.

---

### [`resilient-query/`](resilient-query/)

Demonstrates AAF's **resilience and policy enforcement** features:
fast-path routing that skips LLM planning for structured queries,
input/output guards that catch prompt injection and PII leaks,
degradation chain cycling through all four levels, budget enforcement
that returns partial results on exhaustion, and approval workflow for
gated write operations.

**Demonstrates:** fast-path routing (Rule 4), input guard — injection
detection (Rule 7), output guard — PII detection (Rule 7), action
guard — side-effect gating (Rule 7), degradation state machine
(5-level chain), budget enforcement (Rule 8), approval workflow,
trace recording (Rule 12).

**Run:**
```bash
cargo test -p aaf-integration-tests --test resilient_query_e2e
```

15 tests exercise: fast-path match/miss, injection guard block/allow,
PII guard flag/allow, degradation chain cycle, budget exhaustion,
approval workflow with/without auto-approve, runtime integration, and
YAML parsing.

---

### [`feedback-loop/`](feedback-loop/)

Demonstrates AAF's **trust lifecycle** and **learning feedback loop**:
how agents earn autonomy through consistent execution, how learning
subscribers mine fast-path rules and score capabilities from trace
observations, and how proposed adaptations require human approval
before going live.

**Demonstrates:** 5-level autonomy (Rule 3), score history and override
tracking, promotion/demotion/DropToFloor, delegation chain trust
propagation (min rule), FastPathMiner with adversarial rejection,
CapabilityScorer, EscalationTuner, RouterTuner, learned rule approval
workflow (Rule 18), Recorder subscriber integration (Rule 16).

**Run:**
```bash
cargo test -p aaf-integration-tests --test feedback_loop_e2e
```

21 tests exercise: score tracking, autonomy mapping, promotion/hold/
demotion/drop, delegation chain, miner proposal/rejection, approval
lifecycle, scorer increase/decrease/mixed, escalation tracking, router
stats, recorder integration, full lifecycle, and YAML parsing.

---

### [`memory-context/`](memory-context/)

Demonstrates AAF's **four-layer memory model** and **context budget**
(Rule 10): working memory for per-task transient state, thread memory
for conversation continuity, long-term memory with entity-keyed
retrieval and tenant isolation, artifact store with full provenance
chains, and the context budget that enforces ~7,500 tokens per LLM
call across five sections.

**Demonstrates:** working memory CRUD + task isolation, thread memory
append-only log, long-term keyword search + entity-keyed retrieval
(Rule 14), tenant isolation (Rule 21), artifact provenance chain,
context budget per-section truncation (Rule 10), multi-step pipeline
integrating all four layers.

**Run:**
```bash
cargo test -p aaf-integration-tests --test memory_context_e2e
```

20 tests exercise: working memory (put/get/overwrite/clear/isolation),
thread memory (append/order/isolation), long-term (keyword/entity/
tenant/limit/multi-entity), artifacts (provenance/content), context
budget (default/tokens/truncate/fit/passthrough), full pipeline, YAML.

---

### [`app-native-surface/`](app-native-surface/)

Demonstrates AAF's **app-native surface layer**: how existing
applications integrate with AAF through events, proposals, and
projections without surrendering authority over their own state.
Events flow in from the app, agents propose changes (never mutate
directly), and the app retains authority to accept, reject, transform,
or let proposals expire.

**Demonstrates:** event routing (FastPath/AgentInterpret/Composite),
event-to-intent adaptation, batch decomposition, Rule 20 (Proposals
Not Mutations) enforcement at construction, proposal lifecycle state
machine (7 states, 6 transitions), Rule 19 (Projections Default-Deny)
field access, cross-tenant isolation, situation packaging with budget,
full event-to-trace pipeline.

**Run:**
```bash
cargo test -p aaf-integration-tests --test app_native_surface_e2e
```

23 tests exercise: event routing (3 paths), adaptation (known/unknown/
budget override), Rule 20 (mutations + compensation), lifecycle (accept/
reject/transform/expire + illegal transitions), projections (allow/deny/
cross-tenant), situation packager, full pipeline, YAML parsing.

---

### [`cross-cell-federation/`](cross-cell-federation/)

Demonstrates AAF's third service-integration pattern — **Cell
Architecture + Federation**: cell routing, data-boundary enforcement,
co-signed capability tokens, and federation agreement parsing.

**Demonstrates:** cell-to-capability routing, PII data-boundary
enforcement, clean-payload crossing, no-agreement rejection,
co-signed token verification + tamper detection, scope check on
federated tokens, federation YAML parsing.

**Run:**
```bash
cargo test -p aaf-integration-tests --test cross_cell_federation_e2e
```

7 tests exercise: cell routing, PII boundary block, clean crossing,
no-agreement block, co-signed token round-trip + tamper, out-of-scope
rejection, YAML parsing.

---

### [`signed-agent/`](signed-agent/)

End-to-end walkthrough of Wave 2 X1 (Agent Identity, Provenance &
Supply Chain). Uses the `aaf identity` CLI to generate a DID, sign a
manifest, export an SBOM in SPDX and CycloneDX formats, verify the
manifest, and issue a signed revocation entry.

**Demonstrates:** cryptographic identity (Rule 22), signed manifests
(Rule 23), SBOM provenance (Rule 24), DID-bound artifact signing
(Rule 28).

**Run:**
```bash
cargo run -p aaf-server -- identity verify examples/signed-agent/manifest.yaml
cargo run -p aaf-server -- identity export-sbom examples/signed-agent/sbom.yaml
cargo run -p aaf-server -- identity export-sbom examples/signed-agent/sbom.yaml --format cyclonedx
```

---

## Adding a new example

1. Create `examples/<name>/` with `aaf.yaml` + `README.md`.
2. If the example needs a test, add it under
   `core/tests/integration/tests/<name>_e2e.rs`.
3. Update this `README.md` with a summary + run instructions.
4. Ensure `cargo test -p aaf-integration-tests` stays green.
