# Error Handling

> How errors are structured, propagated, and handled across AAF crates.

---

## Principles

1. **All errors are typed.** Every crate has its own error enum.
   `thiserror` for libs, `anyhow` only in the server binary.
2. **Errors carry context.** Variant fields describe what went wrong
   (step id, budget amount, rule name) so the caller can decide what
   to do without re-parsing strings.
3. **No `unwrap()` in lib code.** The only places `unwrap()` may
   appear are in test code and the server binary's startup path.
4. **Compensation failures always surface.** A failed compensator is
   never swallowed — it becomes `RuntimeError::CompensationFailed`
   or `ExecutionOutcome::RolledBack`.
5. **Policy denials are errors, approvals are outcomes.** A `Deny`
   from the policy engine is `Err(RuntimeError::PolicyViolation)`.
   A `RequireApproval` is `Ok(ExecutionOutcome::PendingApproval)`.

---

## Error taxonomy by crate

### `aaf-contracts` — no error types

Contracts are data. They validate at construction time (e.g.
`IntentEnvelope::validate()`) but validation returns `Result<(), String>`.

### `aaf-storage::StorageError`

```rust
pub enum StorageError {
    NotFound(String),
    Backend(String),
    Conflict(String),
    BoundaryViolation(String),
}
```

Used by: every storage trait (`WorkingMemoryStore`, `ThreadMemoryStore`,
`LongTermMemoryStore`, `ArtifactStore`, `TraceStore`).

### `aaf-runtime::RuntimeError`

```rust
pub enum RuntimeError {
    StepTimeout { step_id, timeout_ms },
    BudgetExceeded(BudgetTrackerError),
    PolicyViolation(Vec<PolicyViolation>),
    Graph(GraphValidationError),
    Node(String),              // catch-all for node execution failures
    CompensationFailed { step_id, reason },
    Revoked { did, reason },   // Wave 2 X1
}
```

The executor converts internal failures into these variants. Callers
match on the variant to decide recovery strategy:

| Variant | Recovery |
|---|---|
| `StepTimeout` | Retry or fail task |
| `BudgetExceeded` | Return partial results |
| `PolicyViolation` | Deny and record |
| `Node` | Trigger compensation chain |
| `Revoked` | Reject before trace opens |

### `aaf-runtime::BudgetTrackerError`

```rust
pub enum BudgetTrackerError {
    Tokens { budget },
    Cost { budget },
    Time { budget },
}
```

Used by: the executor's budget charge step. When triggered, the
executor returns `ExecutionOutcome::Partial` (not an error — partial
results are still useful).

### `aaf-policy::PolicyDecision`

Not an error enum per se, but controls flow:

```rust
pub enum PolicyDecision {
    Allow,
    AllowWithWarnings(Vec<PolicyViolation>),
    RequireApproval(Vec<PolicyViolation>),
    Deny(Vec<PolicyViolation>),
}
```

`Deny` → `Err(RuntimeError::PolicyViolation)`.
`RequireApproval` → `Ok(ExecutionOutcome::PendingApproval)`.

### `aaf-surface::SurfaceError`

```rust
pub enum SurfaceError {
    MissingCompensation { count },
    ProjectionDenied { projection, field },
    ContextBudgetExceeded { used, limit },
    IllegalTransition { from, to },
}
```

Used by: `ActionProposal::build()` (Rule 20 enforcement) and
`ProposalLifecycle` state machine.

### `aaf-identity::IdentityError`

Used by: DID generation, manifest signing/verification, token
verification, revocation.

### `aaf-trust::DelegationError`

```rust
pub enum DelegationError {
    InsufficientTrust { required, effective },
    Token(IdentityError),
}
```

Used by: `effective_trust()` / `require()` / `verify_token()`.

---

## Error propagation patterns

### Across crate boundaries

Crates convert foreign errors at the boundary using `From` impls or
explicit `.map_err()`:

```rust
// aaf-runtime converts StorageError to RuntimeError
self.recorder.close(trace_id, status)
    .await
    .map_err(|e| RuntimeError::Node(e.to_string()))?;
```

### In the executor loop

The executor runs five hooks per step. At each hook:

1. Evaluate → get `PolicyDecision`
2. Match on decision → `Deny` returns `Err`, `RequireApproval`
   returns `Ok(PendingApproval)`, others continue
3. Run node → failure triggers compensation chain
4. Charge budget → exhaustion returns `Ok(Partial)`
5. Record observation → storage error becomes `RuntimeError::Node`

### In tests

Tests use `.unwrap()` freely. For expected errors, use:

```rust
let err = action.unwrap_err();
assert!(matches!(err, RuntimeError::PolicyViolation(_)));
```

---

## Adding a new error variant

1. Add the variant to the crate's error enum with `#[error("...")]`.
2. Include context fields (not just a string message).
3. Add a `From` impl if the error should auto-convert at boundaries.
4. Add a test that triggers the new error path.
5. If the error affects the executor loop, update
   `development/runtime-internals.md`.
