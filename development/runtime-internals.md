# Runtime Internals

> Deep dive into `aaf-runtime`, the most complex crate in the
> workspace. If you need to change the executor, add a node
> kind, wire a new hook, or debug a compensation failure, this
> is the file to read first.
>
> Source lives in `core/crates/aaf-runtime/src/`.

---

## File map

| File | Lines | Owns |
|---|---|---|
| `lib.rs` | 32 | Re-exports + crate-level doc comment |
| `graph.rs` | 217 | `Graph`, `GraphBuilder`, `Edge`, Kahn's topological sort, validation |
| `executor.rs` | 431 | `GraphExecutor`, `ExecutionOutcome`, the full run loop with five hook points |
| `node/mod.rs` | 75 | `Node` trait, `NodeOutput`, `NodeKind` |
| `node/deterministic.rs` | 66 | `DeterministicNode` — pure function / tool call |
| `node/agent.rs` | 81 | `AgentNode` — LLM-powered with guard wrapping |
| `node/approval.rs` | 206 | `ApprovalNode` — polls an `ApprovalWorkflow` |
| `node/fork.rs` | 74 | `ForkNode` — parallel fork/join |
| `node/event_wait.rs` | 57 | `EventWaitNode` — blocks on an external event |
| `compensation.rs` | 59 | `CompensationChain` — LIFO stack of compensators drained on failure |
| `budget.rs` | 127 | `BudgetTracker` — Rule 8 enforcement |
| `checkpoint.rs` | 51 | Checkpoint writer facade over `aaf-storage::CheckpointStore` |
| `scheduler.rs` | 34 | Sequential / parallel scheduler primitives |
| `timeout.rs` | 12 | Timeout wrapper |
| `error.rs` | 59 | `RuntimeError` with six variants |

Roughly **1,580 lines** of runtime code.

---

## The `Node` trait

Every executable step in a graph is a `Node`. The trait is
defined in `node/mod.rs`:

```rust
#[async_trait]
pub trait Node: Send + Sync {
    fn id(&self) -> &NodeId;
    fn kind(&self) -> NodeKind;
    fn side_effect(&self) -> SideEffect { SideEffect::None }
    async fn run(
        &self,
        intent: &IntentEnvelope,
        prior_outputs: &HashMap<NodeId, NodeOutput>,
    ) -> Result<NodeOutput, RuntimeError>;
}
```

**Required methods:**
- `id()` — stable node id used by the executor and the trace
- `kind()` — discriminator (`Deterministic`, `Agent`,
  `Approval`, `Fork`, `EventWait`)
- `run()` — the actual work; returns a `NodeOutput` on success

**Defaulted:**
- `side_effect()` — defaults to `SideEffect::None`. Override for
  write / delete / send / payment nodes so the policy engine's
  side-effect gate fires.

### `NodeOutput`

```rust
#[derive(Debug, Clone, Default, PartialEq)]
pub struct NodeOutput {
    pub data: serde_json::Value,  // structured payload
    pub tokens: u64,               // consumed by LLMs
    pub cost_usd: f64,             // dollar cost
    pub duration_ms: u64,          // wall-clock
    pub model: Option<String>,     // for trace + router
}
```

Every node must report its token/cost/duration accurately
because `BudgetTracker::charge` uses these values and will
terminate execution if any budget is exceeded.

### `NodeKind`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeKind {
    Deterministic,  // pure function / tool call
    Agent,          // LLM-powered
    Approval,       // human gate
    Fork,           // parallel fork/join
    EventWait,      // blocks on external event
}
```

The five kinds correspond to the five canonical node types in
`PROJECT.md` §3.5.

---

## The five concrete nodes

### `DeterministicNode` (`node/deterministic.rs`)

Pure function / tool call. **Rule 5 forbids LLM use here** —
financial calcs, inventory reservation, authentication
decisions, audit log writes, cryptographic operations, state
machine transitions, and rate limiting all live on
`DeterministicNode`.

Constructor takes an `Arc<dyn Fn>` closure:

```rust
DeterministicNode::new(
    node_id,
    side_effect,
    Arc::new(|intent, prior| Ok(json!({"rows": 47}))),
)
```

The closure is called on every run; it is the deterministic
business logic.

### `AgentNode` (`node/agent.rs`)

LLM-powered reasoning. Wraps an `LLMProvider` handle and runs
through **three guards** (Rule 7):

- `InputGuard` — injection + auth check on the prompt
- `OutputGuard` — PII + policy compliance on the response
- `ActionGuard` — scope + side-effect check on any proposed
  action

Failure to wire all three is a compile-time-enforced mistake:
the `AgentNode::new` constructor requires every guard.

### `ApprovalNode` (`node/approval.rs`, 206 lines — the largest)

Human approval gate. Holds an `Arc<ApprovalWorkflow>` and polls
it when `run()` is called. The iteration-2 integration wired
the node to the real workflow so the runtime pauses on a
`PendingApproval` outcome and resumes when the workflow state
flips to `Approved`.

States:

```
ApprovalNode::run →
    workflow.state(trace_id) match:
        Pending  → Ok(NodeOutput::pending_approval) → executor pauses
        Approved → continue normally
        Rejected → Err(RuntimeError::Node("rejected"))
```

### `ForkNode` (`node/fork.rs`)

Parallel fork / join. Spawns its child nodes on
`tokio::spawn`, awaits all, and merges their `NodeOutput::data`
into a single JSON object.

Budget is charged **per child**, not once for the fork — so a
fork of five children counts as five charges against the
`BudgetTracker`.

### `EventWaitNode` (`node/event_wait.rs`)

Blocks on an external event — e.g. a webhook callback, a
message from a queue, an operator action. Holds a
`tokio::sync::oneshot::Receiver` and awaits it. Times out
according to the intent's `max_latency_ms` budget.

Not used on the critical path today (the shipped examples all
use `Deterministic` or `Agent`), but the scaffold is ready for
E3 Slice B's event gateway to wire.

---

## The graph

`Graph` is a validated DAG. Construction goes through
`GraphBuilder`:

```rust
let graph = GraphBuilder::new()
    .add_node(node_a.clone())
    .add_node(node_b.clone())
    .add_edge(node_a.id().clone(), node_b.id().clone())
    .add_compensator(node_a.id().clone(), compensator_a)
    .build()?;
```

**Validation** (`Graph::validate` / `validate_with_compensators`)
runs **Kahn's topological sort** over the nodes + edges. It:

1. Refuses an empty graph (`GraphValidationError::Empty`).
2. Refuses edges whose `from` or `to` id is not a known node
   (`GraphValidationError::MissingNode`).
3. Refuses compensators whose key is not a known node id.
4. Refuses cycles (`GraphValidationError::Cycle`).
5. Produces a **stable** topological order (ties broken by
   sorted node id string for determinism).

The returned `Graph` has four public fields:

- `nodes: HashMap<NodeId, Arc<dyn Node>>`
- `edges: Vec<Edge>`
- `order: Vec<NodeId>` — topologically sorted execution order
- `compensators: HashMap<NodeId, Arc<dyn Node>>`

The executor walks `order` in sequence, runs each node, and
pushes its compensator (if any) onto the `CompensationChain`.

---

## The executor — `GraphExecutor::run`

The single most important function in the runtime. Lives in
`executor.rs`. ~300 lines of the 431 in that file. Here is the
precise control flow.

### Fields

```rust
pub struct GraphExecutor {
    pub policy: Arc<PolicyEngine>,
    pub recorder: Arc<dyn TraceRecorder>,
    pub budget: BudgetTracker,
    pub revocation: Option<Arc<dyn RevocationRegistry>>,  // X1 Slice B
}
```

### Construction

```rust
let exec = GraphExecutor::new(policy, recorder, budget);
// X1 Slice B: optional revocation gate
let exec = exec.with_revocation(revocation_registry);
```

### Run loop (simplified pseudocode)

```
run(graph, intent):

  // Hook 0 — X1 Slice B revocation gate
  if self.revocation.is_some() && intent.requester.user_id starts with "did:aaf:":
      if revocation.is_revoked(Did, did).await:
          return Err(RuntimeError::Revoked)   // trace is NOT opened

  // Hook 1 — PrePlan
  decision = policy.evaluate(PrePlan, ctx)
  recorder.open(trace_id, intent_id).await    // trace opens here
  match decision:
      Deny    → close_trace(Failed) → return Err(PolicyViolation)
      RequireApproval → close_trace(Partial) → return PendingApproval{at_step: 0}
      _       → continue

  compensation_chain = CompensationChain::new()
  compensator_targets = vec![]

  for node_id in graph.order:
      step += 1
      node = graph.nodes[node_id]

      // Hook 2 — PreStep
      decision = policy.evaluate(PreStep, ctx)
      match decision:
          Deny    → close_trace(Failed) → return Err(PolicyViolation)
          RequireApproval → close_trace(Partial) → return PendingApproval{at_step}
          _       → continue

      // Execute
      started = Instant::now()
      output = node.run(intent, &outputs).await
      if output is Err(e):
          compensated = rollback(chain, compensator_targets, intent).await
          close_trace(Failed)
          if compensated.len() > 0:
              return Ok(RolledBack{failed_at, reason, compensated})
          else:
              return Err(e)

      // Charge budget — Rule 8
      budget.charge(output.tokens, output.cost_usd, elapsed_ms)
      if charge returns Err:
          close_trace(Partial)
          return Ok(Partial{outputs, steps: step-1, reason})

      // Hook 3 — PostStep
      decision = policy.evaluate(PostStep, ctx_with_output)
      match decision:
          Deny    → close_trace(Failed) → return Err(PolicyViolation)
          RequireApproval → close_trace(Partial) → return PendingApproval{at_step}
          _       → continue

      // Record observation + attach minimal Outcome (E1 Slice A)
      obs = Observation {
          trace_id, node_id, step,
          outcome: StepOutcome::Success,
          outcome_detail: Some(Outcome::minimal(...)),
          ...
      }
      recorder.record_observation(obs, ...).await

      // Register compensator if this step was a successful write
      if graph.compensators[node_id] is Some:
          chain.push(compensator)
          compensator_targets.push(node_id)

      outputs[node_id] = output

  close_trace(Completed)
  return Ok(Completed{outputs, steps})
```

### Five hook points

| Hook | Called | Context populated |
|---|---|---|
| **Revocation (0)** | Before trace opens | requester DID only |
| **`PrePlan` (1)** | Once, immediately after trace opens | intent + requester |
| **`PreStep` (2)** | Before each node runs | + capability (when known) + side_effect + composed_writes |
| **`PostStep` (3)** | After each node's output is known | + output |
| **`PreArtifact` (4)** | Before an artifact is written (future) | + output (the artifact content) |

`PreArtifact` has no call site yet — the scaffold is in the
policy engine and in `CLAUDE.md` but the runtime does not call
it. A future iteration will wire it when `AgentNode` starts
producing `Artifact`s directly.

### Policy decision handling

Every hook returns a `PolicyDecision`. The executor maps each
variant as follows:

| Decision | PrePlan | PreStep | PostStep |
|---|---|---|---|
| `Allow` | continue | continue | continue |
| `AllowWithWarnings` | continue (log) | continue (log) | continue (log) |
| `RequireApproval` | close Partial, return `PendingApproval{at_step: 0}` | close Partial, return `PendingApproval{at_step}` | close Partial, return `PendingApproval{at_step, outputs including this step}` |
| `Deny` | close Failed, return `Err(PolicyViolation)` | close Failed, return `Err(PolicyViolation)` | close Failed, return `Err(PolicyViolation)` |

`PostStep` is the only hook that preserves the *current* step's
output in the returned `PendingApproval` — so a reviewer can
see what would have been emitted.

### Execution outcomes

```rust
pub enum ExecutionOutcome {
    Completed { outputs, steps },
    Partial { outputs, steps, reason: BudgetTrackerError },
    PendingApproval { outputs, at_step, decision: PolicyDecision },
    RolledBack { failed_at, reason, compensated: Vec<NodeId> },
}
```

Plus the error cases which map to `Err(RuntimeError::*)`:

- `RuntimeError::Revoked { did, reason }` — X1 Slice B
- `RuntimeError::PolicyViolation(violations)` — any hook denial
- `RuntimeError::Node(msg)` — a node's `run()` returned `Err`
  and there were no compensators to walk
- `RuntimeError::Budget(e)` — only if the budget tracker fails
  in a way that cannot be caught by `charge`

---

## The compensation chain

Defined in `compensation.rs`:

```rust
pub struct CompensationChain {
    // LIFO stack of compensator nodes.
    compensators: Vec<Arc<dyn Node>>,
}

impl CompensationChain {
    pub fn push(&mut self, compensator: Arc<dyn Node>) { ... }
    pub async fn drain(&mut self, intent: &IntentEnvelope)
        -> Result<Vec<NodeId>, RuntimeError> { ... }
}
```

Semantics:

1. **Push on success.** After a step succeeds, if the graph has
   a compensator registered for it
   (`graph.compensators[node_id]`), push the compensator onto
   the chain.
2. **Drain on failure.** When a later step's `run()` returns
   `Err`, `drain` pops compensators in **reverse order** (LIFO,
   matching the way they were pushed) and runs each.
3. **Report which ones succeeded.** `drain` returns a
   `Vec<NodeId>` of the *target* step ids (not the compensator
   ids) whose rollback was successful. The executor
   surfaces this list as `RolledBack { compensated }`.

**Iteration-3 bug fix:** before iter 3, the compensation chain
was declared but never wired. A node failure left successful
write steps un-rolled-back. The fix wired the chain into
`GraphExecutor::run` and added a regression test
(`rule_9_compensation_runs_on_node_failure`). Do not regress
this — the test is in
`core/crates/aaf-runtime/tests/executor_integration.rs`.

---

## The budget tracker

Defined in `budget.rs`. Behind a `parking_lot::Mutex` because
the executor charges after every step and `ForkNode` charges
concurrently.

```rust
BudgetTracker::new(BudgetContract { max_tokens, max_cost_usd, max_latency_ms })
BudgetTracker::charge(tokens, cost_usd, elapsed_ms) -> Result<(), BudgetTrackerError>
BudgetTracker::remaining() -> BudgetContract
```

`charge` uses `saturating_add` on u64 fields to avoid wrap-around,
and reports exhaustion via three discriminated error variants:

- `BudgetTrackerError::Tokens { budget }`
- `BudgetTrackerError::Cost { budget }`
- `BudgetTrackerError::Time { budget }`

The executor turns exhaustion into a **graceful partial** result
(`ExecutionOutcome::Partial`) rather than a hard error — Rule 8
says "Exceeding triggers graceful termination with partial
results".

**Iteration-2 gotcha:** the original partial-result test was
timing-based (`tokio::time::timeout` with 5 ms), which flaked on
fast machines. Iteration 2 replaced it with a deterministic
cost-based variant via a `CostingNode` test helper. Do not
revert to timing-based tests for budget enforcement.

---

## The checkpoint writer

Defined in `checkpoint.rs`. Thin facade over
`aaf-storage::CheckpointStore`. Writes per-step state so an
interrupted execution (e.g. server restart in the middle of a
long saga) can be resumed.

**Current status:** the writer exists and is tested but is **not
yet called from `GraphExecutor::run`** — the executor is
stateless across a single call. Wiring the checkpoint writer
into `run` so executions become resumable is scheduled for a
future iteration. The `Resume` test in
`aaf-runtime::tests::executor_integration` is skipped pending
this work.

---

## Adding a new node type

If you need a new `NodeKind` (rarer than you think — the five
that ship cover every case in the `PROJECT.md` vision), the
steps are:

1. Add a variant to `NodeKind` in `node/mod.rs`.
2. Create `node/<name>.rs` with a struct and an `impl Node`.
3. Re-export from `node/mod.rs` and `lib.rs`.
4. Add tests: at least one happy-path, one failure-path, one
   policy-hook interaction.
5. Update `development/runtime-internals.md` (this file) with a
   new entry in the "Five concrete nodes" section.
6. Update `PROJECT.md` §3.5 if the new kind represents a new
   architectural concept; otherwise just leave it as an
   implementation detail.

Do **not** add a new hook point without updating:
- `aaf-policy::engine::PolicyHook` enum
- Every construction of `PolicyContext` in the repo
  (`grep -rn "PolicyContext {" core/` finds them)
- This document's "Five hook points" table
- The corresponding row in `docs/security.md` → "The four policy
  hooks"

---

## Debugging tips

- **A step is silently skipped.** Check the `graph.order` — a
  cycle or missing edge would have been caught by
  `validate`, but a capability whose dependencies were filtered
  out of the plan by `aaf-planner::topo_sort` will show up as
  `order.len() < nodes.len()` after planning.
- **Compensation ran but reported empty.** The compensator was
  not registered via `GraphBuilder::add_compensator`. The
  planner's `build_graph_from_plan` helper (in `aaf-server`) is
  responsible for registering compensators — check its loop.
- **Budget exhausted on the first step.** A node's `NodeOutput`
  is reporting spurious costs (e.g. `cost_usd = f64::NAN` or
  `u64::MAX` tokens). `BudgetTracker::charge` uses
  `saturating_add` so it will never wrap, but a single big
  reported value will trip `Cost` or `Tokens` immediately.
- **`PendingApproval` is returned but execution is also
  completed.** The `PostStep` hook returned `RequireApproval`
  *after* the step ran. This is intentional — the reviewer sees
  what would have been emitted — but it means the step's side
  effect already landed. For irreversible side effects, gate at
  `PreStep` instead.

---

## Where to go next

- [contracts-reference.md](contracts-reference.md) — the typed
  messages this file references
- [architecture-overview.md](architecture-overview.md) — the
  runtime in context
- [testing-strategy.md](testing-strategy.md) — where runtime
  tests live
- [observability.md](observability.md) — how the recorder hooks
  into the runtime
- `core/crates/aaf-runtime/src/executor.rs` — the code this
  document describes
