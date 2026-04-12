# Security Model

> How AAF enforces each security-relevant rule from `CLAUDE.md`
> end-to-end, and where the enforcement lives in the code.

Every row in the table below is a **construction-time invariant**
or a **runtime gate**. There are no runtime feature flags that
disable them and there are no "dev-mode" bypasses.

---

## Rule map

| Rule | Risk model | Enforcement point | Crate |
|---|---|---|---|
| R1 Agents translate, services decide | Agents leaking business logic | `DeterministicNode` / `AgentNode` separation | `aaf-runtime` |
| R2 Typed internals | Injection via free-form strings | Every cross-crate message is a `aaf-contracts` type | `aaf-contracts` |
| R5 Deterministic core is sacred | LLMs making financial / auth / crypto decisions | `DeterministicNode::run` refuses `LLMProvider` use | `aaf-runtime` |
| R6 Policy at every step | Unchecked execution paths | `PolicyEngine::evaluate` at 4 hooks | `aaf-runtime`, `aaf-policy` |
| R7 Guard every agent | Agents acting without guards | `InputGuard`, `OutputGuard`, `ActionGuard` | `aaf-policy::guard` |
| R8 Depth + budget limits | Runaway delegation / cost | `IntentEnvelope.depth ≤ 5`, `BudgetTracker` | `aaf-contracts`, `aaf-runtime::budget` |
| R9 Compensation before implementation | Write capabilities without rollback | `CapabilityContract::validate` + `Registry::register` | `aaf-contracts`, `aaf-registry` |
| R10 Context minimisation | Leaking context to LLMs | `ContextBudget ≈ 7,500 tokens` | `aaf-memory::context` |
| R11 Storage behind traits | Direct DB driver misuse | Compile-time: no `aaf-*` crate imports a DB driver | `aaf-storage` |
| R12 Trace everything | Silent failures | `Recorder::record_observation` on every step | `aaf-trace` |
| R13 Sidecar transparent fallback | Control plane failure cascading | `Proxy::forward_direct` bypasses AAF on failure | `aaf-sidecar` |
| R14 Semantics are nouns (E2) | String-typed boundaries | `reads`/`writes`/`emits` + entity-aware checks | `aaf-contracts`, `aaf-planner`, `aaf-policy`, `aaf-federation` |
| R19 Projections default-deny (E3) | State leakage via projections | `StateProjection::allows_field` defaults false | `aaf-surface` |
| R20 Proposals, not mutations (E3) | Agents mutating state directly | `ActionProposal::new_with_mutations` enforces at construction | `aaf-surface` |
| R21 Entities tenant-scoped (E2) | Cross-tenant reads/writes | `EntityRefLite.tenant`, boundary rule, federation router | `aaf-contracts`, `aaf-policy`, `aaf-federation` |
| R22 Identity is cryptographic (X1) | Spoofed agent identity | `AgentDid` is a public-key thumbprint | `aaf-identity` |
| R23 Signed manifest (X1) | Modified agent code / prompts | `AgentManifest::build` signs at build time | `aaf-identity` |
| R24 Provenance as BOM (X1) | Unknown provenance | `AgentSbom` with content hashes | `aaf-identity` |

---

## The four policy hooks

Every execution path through `aaf-runtime::executor::GraphExecutor::run`
passes through the policy engine at these four points:

```
                    ┌─────────────────────────┐
intent received ──▶ │ PolicyHook::PrePlan     │──▶ Deny / Approve / Allow
                    └─────────────────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │ (X1 Slice B) revocation │──▶ RuntimeError::Revoked
                    └─────────────────────────┘
                                │
          for each step in topological order:
                                │
                                ▼
                    ┌─────────────────────────┐
                    │ PolicyHook::PreStep     │
                    └─────────────────────────┘
                                │
                                ▼
                          run the node
                                │
                                ▼
                    ┌─────────────────────────┐
                    │ PolicyHook::PostStep    │
                    └─────────────────────────┘
                                │
                                ▼
                    ┌─────────────────────────┐
                    │ PolicyHook::PreArtifact │ (when applicable)
                    └─────────────────────────┘
                                │
                                ▼
                     record Observation
```

See [policies.md](policies.md) for the full rule set and decision
algorithm.

---

## Identity (X1)

Every agent has a **Decentralized Identifier** (`did:aaf:<thumbprint>`)
derived from the public key in its keystore. An agent's
`AgentManifest` enumerates:

- Model version (`ModelPin`)
- Prompts (hashed)
- Tool bindings (`Vec<ToolBinding>`)
- Ontology slices (`Vec<EntityRef>`)
- Capability allow-list
- Upstream providers

The manifest is **signed at build time** (Rule 23): the only public
constructor is `AgentManifest::build`, which signs in its final
step. Any subsequent tamper is detected by `verify_manifest`.

`AgentSbom` sits alongside the manifest and enumerates every
input with content hashes (Rule 24). SPDX/CycloneDX export is
deferred to X1 Slice C.

### Delegation — capability tokens

`CapabilityToken` is a signed bearer grant:

```
TokenClaims {
    caller: AgentDid,       // who issued
    callee: AgentDid,       // who may use
    capabilities: Vec<CapabilityId>,
    depth_remaining: u32,   // stepped down at each hop
    issued_at,
    expires_at,
}
```

When agent A delegates to agent B:

1. A issues a `CapabilityToken` with
   `depth_remaining = intent.depth - 1`.
2. The token is embedded in the resulting `Handoff`.
3. `aaf-trust::verify_token` checks signature + validity window +
   scope + remaining depth before the call runs.
4. Revoked tokens short-circuit immediately (see below).

### Revocation

`RevocationRegistry` serves a list of revoked DIDs, prompt hashes,
and tool versions. Every entry is itself signed (audit trail).

**Invariant:** a revoked DID is rejected at the runtime's
pre-plan hook, **before the trace opens**. This means a revoked
agent's failed attempt leaves no trace artefacts behind. The
invariant is tested in
`core/tests/integration/tests/x1_slice_b_integration.rs`.

---

## Trust (behavioural + cryptographic)

Two orthogonal dimensions:

- **Behavioural** — `TrustScore` in `[0.0, 1.0]` and
  `AutonomyLevel` in `Observer..Custodian`. Updated by past
  outcomes (E1).
- **Cryptographic** — DID, manifest signature, capability token,
  revocation. Updated by attestation events.

Delegation requires both to pass:

```
effective_trust(a, b) = min(a.trust, b.trust)    // behavioural
verify_token(t, &v)                              // cryptographic
```

---

## Data classification lattice (E2)

```
Public ⊂ Internal ⊂ Pii ⊂ Regulated(tag)
```

A capability's `data_classification` must be ≥ every entity it
reads. The boundary rule (with an ontology lookup) enforces this
at the `PreStep` hook. Downgrades are denied.

Federation agreements can tighten this per-entity via
`EntityAccessRule.max_classification`.

---

## Side-effect gating

Every capability declares a `side_effect`:

```
None     → pure function, no gate
Read     → scope check only
Write    → requires compensation (Rule 9); side-effect gate
Delete   → same, plus "irreversible by default"
Send     → external message; side-effect gate
Payment  → financial transaction; side-effect gate + composition limits
```

The `side_effect_gate` rule produces a `RequireApproval` decision
for any write-class action; the `approval_policy = "auto-approve"`
scope on the requester can bypass this where policy permits.

---

## PII and prompt injection

Two guards run on every agent node:

- `OutputGuard` runs the `pii::PiiGuard` rule over the output
  before it leaves the node. Current detectors: email, JP phone,
  credit card shapes.
- `InputGuard` runs the `injection::InjectionGuard` rule over
  any external payload before it enters the node.

Both rules are regex-based in v0.1 and live in
`aaf-policy::rules`. Extending them is additive — add more
regexes or a richer detector behind the `Rule` trait.

---

## Audit trail

Every decision records an `Observation` via the trace recorder
(Rule 12). OTLP export is wired through
`aaf-trace::otel::OtlpExporter` and cannot be disabled in
production.

Every change to the runtime configuration (capability register,
capability deregister, policy pack swap, revocation add) must
itself record an observation so the operator can reconstruct
"who changed what, when". The observation carries the change
author's DID (X1 Slice B).

---

## Security checklist — what every PR must preserve

From `CLAUDE.md`:

- [ ] No free-form strings between components
- [ ] Input Guard before every agent node
- [ ] Output Guard after every agent node
- [ ] Policy Engine at every step
- [ ] Depth decremented on delegation
- [ ] Budget decremented at every LLM call
- [ ] Trust: `min(delegator, delegatee)`
- [ ] PII detection on all outputs
- [ ] Injection detection on all external inputs
- [ ] Artifact signing with full provenance
- [ ] Tenant isolation on all storage ops
- [ ] Compensation defined for all write capabilities
- [ ] No `unwrap()` in Rust libs, no `any` in TS
- [ ] Secrets never in logs/traces
- [ ] Sidecar transparent fallback on AAF failure
- [ ] Cross-cell data boundary enforced

---

## Further reading

- [policies.md](policies.md) — engine internals
- [adr/ADR-008-entity-space-boundaries.md](adr/ADR-008-entity-space-boundaries.md)
  — the rationale for entity-space boundaries
- [../development/contracts-reference.md](../development/contracts-reference.md)
  — construction-time invariants on contract types
