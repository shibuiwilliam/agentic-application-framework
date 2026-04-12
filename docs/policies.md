# Policies

> How the policy engine gates every execution path in AAF.
> **Rules 6 and 7** (Policy at every step; Guard every agent) are
> enforced through this subsystem.

---

## The policy engine

`aaf-policy::PolicyEngine` owns a set of `Rule` implementations
and evaluates them against a `PolicyContext`. The engine is
called at **four hook points** inside the runtime executor
(Rule 6):

| Hook | When | Fields populated in context |
|---|---|---|
| `PrePlan` | Once, before any node runs | `intent`, `requester` |
| `PreStep` | Before each node executes | `intent`, `capability`, `requester`, `side_effect`, `remaining_budget`, `tenant`, `composed_writes` |
| `PostStep` | After each node produces output | + `output` |
| `PreArtifact` | Before an artifact is written | + `output` (the artifact content) |

Every step through the executor hits at least `PreStep` and
`PostStep`. Agent nodes hit all four.

---

## The decision algorithm

`PolicyEngine::evaluate(hook, ctx) -> PolicyDecision`:

1. Run every rule; collect `Vec<PolicyViolation>`.
2. If empty → `Allow`.
3. If any violation has severity `Error` or `Fatal` → `Deny(violations)`.
4. Else if any violation has `kind == SideEffectGate` → `RequireApproval(violations)`.
5. Else → `AllowWithWarnings(violations)`.

The side-effect gate is **always** a "needs approval" signal,
regardless of its declared severity — that is its semantic role.

---

## The seven rules

`core/crates/aaf-policy/src/rules/` contains one module per rule.

| # | Rule | What it catches |
|---|---|---|
| 1 | `scope::ScopeCheck` | Requester missing a required OAuth-style scope |
| 2 | `side_effect::SideEffectGate` | Read/write/delete/send/payment gating, approval trigger |
| 3 | `budget::BudgetControl` | Budget exhaustion (cost / tokens / latency) |
| 4 | `pii::PiiGuard` | Emails, JP phone numbers, credit card shapes in output |
| 5 | `injection::InjectionGuard` | Classic prompt injection markers in payload |
| 6 | `composition::CompositionSafety` | More than N write-class side effects in one plan |
| 7 | `boundary::BoundaryEnforcement` | Tenant mismatch, classification violations, **entity classification flow** (E2 Slice B) |

### Entity classification flow (E2 Slice B)

Since iteration 7, `BoundaryEnforcement` consults an optional
`OntologyClassificationLookup` callback on `PolicyContext`. When
present, the rule:

- For every entity in `capability.reads`, resolves its declared
  classification via the callback.
- If the entity classification **exceeds** the capability's
  `data_classification`, emits a `Fatal` boundary violation
  ("classification flow violation").
- For every entity in `capability.writes`, checks the declared
  tenant against the active tenant; mismatches are fatal.

The pre-Slice-B tag-based checks remain as a fallback when no
ontology lookup is wired. Deployments that do not opt in still
see correct v0.1 behaviour.

---

## The three guards

`aaf-policy::guard` wraps the engine in three narrower surfaces
(Rule 7). Every agent node gets all three.

| Guard | Input | Output |
|---|---|---|
| `InputGuard::check(intent, payload)` | Raw NL / structured payload about to enter a node | Injection / auth check |
| `OutputGuard::check(intent, output)` | Text/JSON output about to leave a node | PII / disclosure / policy compliance |
| `ActionGuard::check(intent, capability, composed_writes)` | A proposed action before it runs | Scope / side-effect gate |

Each guard returns the same `PolicyDecision` so callers can match
uniformly.

---

## Custom rules (plugins)

`PolicyEngine::add_rule(Arc<dyn Rule>)` lets you append a custom
rule. The rule only has to implement:

```rust
pub trait Rule: Send + Sync {
    fn id(&self) -> &str;
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation>;
}
```

Custom rules are composed with the default seven — the decision
algorithm is the same.

Future work: WASM-hosted rule plugins via the deferred `plugin.rs`
module.

---

## Approval workflow

`aaf-policy::approval::ApprovalWorkflow` coordinates
human-in-the-loop gating for `RequireApproval` decisions. It
maintains an in-memory registry of `ApprovalRequest`s keyed by
trace id, and the runtime's `ApprovalNode` polls it for state
changes.

Typical flow:

1. Executor hits `PreStep`; policy returns `RequireApproval(v)`.
2. Executor writes an `ApprovalRequest { trace_id, step, violations }`
   to the workflow.
3. Execution pauses (task state → `PausedForApproval`).
4. A human (or an external approver) flips the state to
   `Approved` / `Rejected`.
5. Executor resumes; `Approved` → run the step; `Rejected` →
   task state → `Cancelled`.

---

## Policy packs

`spec/examples/policy-pack-base.yaml` is the baseline policy
pack. It declares which rules are active and at what severity.
A policy pack is the only supported way to tune policy behaviour
without recompiling.

Domain-specific packs can live under `policies/finance/`,
`policies/healthcare/`, etc. (directories prepared, configs TBD).

---

## Example: writing a tenant-restricted composition rule

```rust
use aaf_policy::{Rule, PolicyContext};
use aaf_contracts::{PolicyViolation, PolicySeverity, RuleKind};

pub struct TenantIsolation;

impl Rule for TenantIsolation {
    fn id(&self) -> &str { "tenant-isolation" }

    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation> {
        let cap = ctx.capability?;
        let tenant = ctx.tenant?;
        for w in &cap.writes {
            if let Some(t) = &w.tenant {
                if t != tenant {
                    return Some(PolicyViolation {
                        rule_id: self.id().into(),
                        kind: RuleKind::BoundaryEnforcement,
                        severity: PolicySeverity::Fatal,
                        message: format!(
                            "cap {} writes entity {} in tenant {}, active is {}",
                            cap.id, w.entity_id, t, tenant
                        ),
                    });
                }
            }
        }
        None
    }
}
```

Register it:

```rust
let mut engine = PolicyEngine::with_default_rules();
engine.add_rule(std::sync::Arc::new(TenantIsolation));
```

---

## Further reading

- [security.md](security.md) — the full security model, rule-to-code map
- [../development/known-gotchas.md](../development/known-gotchas.md) —
  things that bit previous iterations around policy context
- `core/crates/aaf-policy/src/` — the source
