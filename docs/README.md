# docs/ — User-facing technical documentation

Short, link-heavy documents for engineers integrating AAF into
their application or services. These supplement — but do not
replace — `PROJECT.md` (the full vision) and `CLAUDE.md` (the
architecture rules).

## Contents

### Start here

- **[getting-started.md](getting-started.md)** — build the
  workspace, run the demo, and make one concrete change.
- **[architecture.md](architecture.md)** — the 10-minute overview
  of how AAF fits together.

### Core concepts

- **[contracts.md](contracts.md)** — the typed surface: Intent,
  Capability, Task, Artifact, Handoff, Observation, Trust, Policy,
  Trace.
- **[policies.md](policies.md)** — the policy engine, seven rules,
  three guards, four hooks.
- **[security.md](security.md)** — how AAF enforces Rules 6 / 7 /
  9 / 14 / 21 / 22 end-to-end.
- **[saga.md](saga.md)** — the Agentic Saga engine and its
  intelligent recovery state machine.
- **[fast-path.md](fast-path.md)** — how the planner classifies
  traffic into four patterns and when it skips the LLM.

### Integration

- **[integration-microservices.md](integration-microservices.md)** —
  integrating AAF as a sidecar alongside existing microservices.
- **[integration-modular-monolith.md](integration-modular-monolith.md)**
  — integrating AAF as an in-process wrapper inside a modular
  monolith.
- **[integration-cell-architecture.md](integration-cell-architecture.md)**
  — federation across cells with entity-space boundaries.

### Operations

- **[deployment.md](deployment.md)** — how to build, package, and
  deploy `aaf-server` plus the sidecar / wrapper variants.
- **[ontology-lint.md](ontology-lint.md)** — how to run and
  interpret `make ontology-lint` (E2 Slice C).

### Enhancement design notes

- **[enhancements/README.md](enhancements/README.md)** — index of
  the enhancement waves
- **[enhancements/e2-domain-ontology.md](enhancements/e2-domain-ontology.md)**
  — E2 Domain Ontology Layer (Slices A + B + C complete)
- **[enhancements/e1-feedback-spine.md](enhancements/e1-feedback-spine.md)**
  — E1 Feedback Spine (Slices A + B complete; Slice C next)
- **[enhancements/e3-app-native-surface.md](enhancements/e3-app-native-surface.md)**
  — E3 Application-Native Surface (Slice A complete)
- **[enhancements/x1-agent-identity.md](enhancements/x1-agent-identity.md)**
  — X1 Agent Identity, Provenance & Supply Chain (complete)
- **[enhancements/f2-llm-integration.md](enhancements/f2-llm-integration.md)**
  — F2 Live LLM Integration & Intelligent Model Routing (Slice A landed)
- **[enhancements/f1-developer-experience.md](enhancements/f1-developer-experience.md)**
  — F1 Developer Experience Platform — SDKs + CLI (planned)
- **[enhancements/f3-protocol-bridges.md](enhancements/f3-protocol-bridges.md)**
  — F3 Universal Protocol Bridge — MCP + A2A (planned)

### Decisions

- **[adr/README.md](adr/README.md)** — the ADR index with status,
  rule-to-ADR map, and the reserved-slot list
- **[adr/ADR-008-entity-space-boundaries.md](adr/ADR-008-entity-space-boundaries.md)**
  — why federation, policy boundary, and composition safety all
  operate on entities rather than field names
- **[adr/ADR-017-agent-did-and-manifest.md](adr/ADR-017-agent-did-and-manifest.md)**
  — why every agent has a DID, a signed manifest, an SBOM, and
  delegated capability tokens

### Where to go next

If you are **extending the framework**, the deep technical
documentation lives in the `development/` directory at the repo
root. Start with `development/README.md`. Those documents are
written for engineers (or Claude Code sessions) continuing
development of the core control plane — they describe the crate
layout, the build gates, the iteration playbook, and the
known-gotchas list.

If you are **reading the original design**, start with the repo's
`PROJECT.md`. That document is the canonical vision statement and
includes service architecture integration (§19, merged from the
former `PROJECT_AafService.md`).

### Standalone documents merged and removed

The following files have been merged into `PROJECT.md` and `CLAUDE.md`
and are no longer separate files:

- `PROJECT_AafService.md` → `PROJECT.md` §19 (Service Architecture Integration)
- `CLAUDE_AaFService.md` → already a subset of `CLAUDE.md` (removed)
- `PROJECT_ENHANCE.md` → `PROJECT.md` §§16–18 (Wave 1/2 enhancements) and §20 (Wave 4 critical infrastructure)
- `CLAUDE_ENHANCE.md` → `CLAUDE.md` rules 14–24 (Wave 1/2), rules 34–38 (Wave 4), and rules 39–43 (Three Pillars)
