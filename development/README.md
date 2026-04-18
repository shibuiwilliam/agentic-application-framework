# development/ — Technical Documentation for Claude Code

> **This directory is written for Claude Code sessions (or any engineer)
> continuing development on AAF.** It supplements `PROJECT.md` (the
> vision) and `CLAUDE.md` (the architecture rules) with the *mechanical*
> knowledge a fresh session needs to be productive in under 10 minutes:
> what crate owns what, how the build runs, where tests live, how to
> plan a new slice, and which pitfalls have bitten previous iterations.
>
> `PROJECT.md` and `CLAUDE.md` are normative — they describe *what* AAF
> must be. The docs in this directory are operational — they describe
> *how* to build it here, in this repository, today.

---

## Read in this order

If you are a Claude Code session and you have not touched this tree
before, open these files in this order. You do not need to read any
other file in the repository to be productive.

1. **`CLAUDE.md`** (repo root) — the 43 architecture rules (13
   foundation + 11 enhancement + 5 Wave 4 infrastructure + 5 Three
   Pillars + 9 service integration). Non-negotiable. Every code
   change must conform.
2. **`development/architecture-overview.md`** — crate map, dependency
   graph, hot path, where contracts live, where enforcement lives.
3. **`development/crate-reference.md`** — one page per crate: what it
   owns, its key public types, its test count, its Slice status.
4. **`development/build-and-ci.md`** — `cargo build`, `cargo test`,
   `cargo clippy`, `make ontology-lint`, `make schema-validate`. All
   four gates must stay green on every PR.
5. **`development/testing-strategy.md`** — where unit tests live,
   where integration tests live, how to pick between them.
6. **`development/iteration-playbook.md`** — the repeatable recipe for
   planning and landing a new Slice. Every iteration in
   `IMPLEMENTATION_PLAN.md` followed this recipe.
7. **`development/next-slices.md`** — the concrete playbook for the
   next upcoming slices (E1 C, E3 B, E3 C). **Start here when
   you are about to pick up real work.**
8. **`development/known-gotchas.md`** — the things that bit previous
   iterations. **Read this before you pick a task.**
9. **`development/roadmap.md`** — what has landed, what is next, what
   is explicitly deferred.

The other files in this directory are references you load on demand:

- **`development/coding-conventions.md`** — Rust style as practiced in
  this repo (not the generic clippy docs).
- **`development/contracts-reference.md`** — the major contract types,
  their fields, their invariants.
- **`development/runtime-internals.md`** — deep dive into
  `aaf-runtime` (executor loop, 5 node types, compensation chain,
  budget tracker). Load when you need to change the runtime.
- **`development/extension-points.md`** — how to add a policy rule, an
  LLM provider, or a storage backend. Load when you are plugging in
  a new piece.
- **`development/observability.md`** — how tracing, cost attribution,
  and OTLP export work. Load when you are debugging in production or
  adding a metric.
- **`development/changing-contracts.md`** — positive migration guide
  for adding a field to `CapabilityContract`, `IntentEnvelope`,
  `PolicyContext`, etc. Load before you touch `aaf-contracts`.
- **`development/glossary.md`** — AAF terminology in alphabetical
  order. Load when you don't recognise a term.
- **`development/examples-walkthrough.md`** — how to run, read, and
  extend the 13 runnable examples. Includes code patterns and rule
  coverage matrix.
- **`development/error-handling.md`** — error taxonomy by crate,
  propagation patterns, how to add a new error variant.
- **`development/capability-authoring.md`** — how to define, register,
  and test new capabilities. Naming conventions, compensation, entity
  declarations, fast-path rules, degradation levels.
- **`development/learning-pipeline.md`** — how the feedback spine
  works end-to-end: `aaf-eval` (Judge, GoldenSuite, Replayer) and
  `aaf-learn` (FastPathMiner, CapabilityScorer, RouterTuner,
  EscalationTuner). Load when you are working on the learning loop
  or evaluation harness.

Enhancement designs (E1/E2/E3/X1) are merged into `PROJECT.md`
§§16–18 and `CLAUDE.md` (rules 14–24). Service architecture
integration design is merged into `PROJECT.md` §19. The former
standalone files (`PROJECT_AafService.md`, `CLAUDE_AaFService.md`,
`PROJECT_ENHANCE.md`, `CLAUDE_ENHANCE.md`) have been merged and
removed.

---

## What this repository is

AAF is a **Rust workspace of 22 crates** (including `aaf-learn`)
implementing the core control plane of an Intent-first Application
Architecture platform. The architecture is laid out in `PROJECT.md`
§§1–20 and enforced by the 43 rules in `CLAUDE.md` (13 foundation +
11 enhancement + 5 Wave 4 infrastructure + 5 Three Pillars + 9
service integration).
Iterations 1–10+ built the Wave-1 foundation (E1/E2/E3) and Wave-2
identity (X1). Wave 4 (F1/F2/F3) adds developer SDKs, live LLM
providers, and protocol bridges — the critical infrastructure for
framework adoption.

Enhancement rules 14–24, Wave 4 rules 34–38, and the slicing strategy
are now merged into `CLAUDE.md`. Enhancement designs are merged into
`PROJECT.md` §§16–20. The former `PROJECT_ENHANCE.md` and
`CLAUDE_ENHANCE.md` have been merged into these files and removed.

Current state at the time of this writing:

| Metric | Value |
|---|---|
| Crates in the workspace | **22** |
| Rust lines (`core/crates/**/src/*.rs`) | **≈26,000** |
| Tests passing | **554** |
| Test failures | **0** |
| Build warnings | **0** |
| Clippy warnings (`-W clippy::all`) | **0** |
| JSON schemas | **18** |
| Example configs | **11** (all validate against their schema) |
| Ontology lint | **strict mode, 0 errors** |
| ADRs | **2** (ADR-008, ADR-017) |
| CLI subcommands | **13** |
| Runnable examples | **13** |

---

## What this directory is NOT

- **Not a tutorial.** It assumes you know Rust, async/tokio, and
  distributed systems.
- **Not a duplicate of `PROJECT.md`.** When in doubt about *what* a
  feature should do, read `PROJECT.md`. These docs answer *where it
  lives in the tree* and *how to change it without breaking anything
  else*.
- **Not a snapshot of the vision.** The vision is in `PROJECT.md` and
  `PROJECT.md`. These docs describe the *current
  implementation*, which is a subset of the vision.
- **Not API reference.** Run `cargo doc --workspace --no-deps` for
  that. `development/contracts-reference.md` is a *guide* to the
  important types, not an exhaustive list.

---

## Ground rules for changes

Every change made in this tree has followed the same protocol. If
you are about to make a change, follow it too:

1. **Read the relevant rule in `CLAUDE.md`.** If your change conflicts
   with one of the 24 rules, stop and reconsider.
2. **Pick the smallest unit of scope that is useful.** Slices are
   organised as A → B → C (contracts → integration → SDK/examples).
   Do not leapfrog.
3. **Plan before you code.** Write the plan into
   `IMPLEMENTATION_PLAN.md` under a new iteration section. Every
   previous iteration did this.
4. **Keep tests ratcheting up, not down.** Every Slice has added
   tests; no Slice has reduced them.
5. **Run the gates.** `cargo build --workspace`,
   `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
   `make ontology-lint`, `make schema-validate`. All must be green.
6. **Document the decision.** If the change is architectural, add an
   ADR under `docs/adr/`. If it is tactical, add it to the iteration
   section in `IMPLEMENTATION_PLAN.md`.

---

## Invariants you can rely on

These hold at every committed state of the tree. If you find one
broken, the tree is in a bad state and you should stop and fix it
before adding new code.

- `cargo build --workspace` is clean.
- `cargo test --workspace` is clean.
- `cargo clippy --workspace --all-targets -- -W clippy::all` is clean.
- `python3 scripts/schema_validate.py --schema-dir spec/schemas --examples-dir spec/examples` prints 0 failures.
- `make ontology-lint` prints 0 errors and exits 0.
- Every crate that touches storage does so through `aaf-storage` traits
  (Rule 11).
- Every write capability in the registry carries a `compensation`
  (Rule 9).
- Every execution path through `aaf-runtime::executor` calls
  `PolicyEngine::evaluate` at the four documented hooks (Rule 6).
- `IntentEnvelope.depth` never exceeds 5 (Rule 8).
- No crate ever calls `unwrap()` in lib code (Rule, coding convention).

---

## Where to find the big things

| Looking for… | Open… |
|---|---|
| The contract types (Intent, Capability, Task, …) | `core/crates/aaf-contracts/src/` |
| The policy engine + 7 rules + 3 guards | `core/crates/aaf-policy/src/` |
| The graph runtime executor | `core/crates/aaf-runtime/src/executor.rs` |
| The planner and composition safety | `core/crates/aaf-planner/src/` |
| The capability registry | `core/crates/aaf-registry/src/` |
| The ontology (entities, relations, classification) | `core/crates/aaf-ontology/src/` |
| The feedback spine (Judge, GoldenSuite, Replayer) | `core/crates/aaf-eval/src/` |
| The app-native surface (AppEvent, ActionProposal) | `core/crates/aaf-surface/src/` |
| Agent identity (DID, manifest, SBOM, tokens, revocation) | `core/crates/aaf-identity/src/` |
| The server binary and CLI | `core/crates/aaf-server/src/main.rs` |
| The end-to-end pipeline test | `core/tests/integration/tests/full_pipeline.rs` |
| The Slice B / C smoke tests | `core/tests/integration/tests/e{1,2,3}_*.rs` / `x1_slice_b_*.rs` |
| The JSON schemas | `spec/schemas/` |
| The example contract instances | `spec/examples/` |
| The plan | `IMPLEMENTATION_PLAN.md` |

---

## Glossary

Abbreviations used throughout these docs:

- **E1 / E2 / E3** — Wave-1 enhancements (see `PROJECT.md` §16):
  E1 = Feedback Spine, E2 = Domain Ontology Layer, E3 = App-Native
  Surface.
- **X1 / X2 / X3** — Wave-2 enhancements: X1 = Agent Identity /
  Provenance / Supply Chain, X2 = Semantic Knowledge Fabric, X3 =
  Developer Experience Surface.
- **F1 / F2 / F3** — Wave-4 critical infrastructure: F1 = Developer
  Experience Platform (SDKs + CLI), F2 = Live LLM Integration &
  Intelligent Model Routing, F3 = Universal Protocol Bridge (MCP + A2A).
- **Slice A / B / C** — every enhancement lands in three slices:
  A = contracts + skeleton + unit tests, B = integration into hot
  crates, C = SDK primitives / examples / polish.
- **Hot path** — the crates the runtime touches on every intent:
  `aaf-intent → aaf-planner → aaf-runtime → aaf-policy → aaf-registry
  → aaf-trace`. E2 Slice B wired the ontology into all of these.
- **DID** — Decentralized Identifier, `did:aaf:<thumbprint>`. Landed
  in X1 Slice A (iteration 6).
- **Ontology lint** — `make ontology-lint` / `aaf-server ontology
  lint`. Reports capabilities missing entity declarations. Ratios <90%
  warn-only, ≥90% strict-mode errors.
