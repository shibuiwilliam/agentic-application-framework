# Architecture Decision Records

> A chronological index of the significant architectural decisions
> made in this codebase. ADRs are written when the decision
> affects more than one crate or establishes an invariant the
> runtime depends on.

---

## What is an ADR?

An **Architecture Decision Record** documents one significant
technical choice: what was decided, why, what alternatives were
considered, and what the positive / negative consequences are.
ADRs live here forever — they are the record of how the
architecture got to its current shape.

ADRs are **not** design docs for work-in-progress features; those
live in `../enhancements/` and in `PROJECT.md` §§16-18 at the
repo root.

---

## The index

| ID | Title | Status | Iteration | Primary area |
|---|---|---|---|---|
| [008](ADR-008-entity-space-boundaries.md) | Entity-space boundaries | Accepted | Iter 8 (E2 Slice C) | `aaf-federation`, `aaf-planner`, `aaf-policy` |
| [017](ADR-017-agent-did-and-manifest.md) | Agent DID + Signed Manifest + SBOM + Delegated Capability Tokens | Accepted | Iter 6 / 9 / 10 (X1 Slice A+B) | `aaf-identity`, `aaf-trust`, `aaf-runtime`, `aaf-registry` |

---

## ADR numbering — where are 1–7 and 9–16?

ADR numbers are reserved for **every significant decision**,
including decisions that were made implicitly in earlier
iterations (iterations 1–5 made many architectural choices
but did not write them up as ADRs at the time).

The numbering gap is intentional. When a future iteration
retroactively formalises one of those decisions (e.g. "Rule 11:
storage behind traits"), it will claim a lower number and the
index will be filled in.

**Reserved slots (not yet written):**

| ID | Likely title |
|---|---|
| 001 | Rust workspace + Edition 2021 + Rust 1.70 pin |
| 002 | Contracts live in one crate, no deps from `aaf-contracts` |
| 003 | Storage behind traits (Rule 11) |
| 004 | Policy at every step — four hook points (Rule 6) |
| 005 | Compensation before implementation (Rule 9) |
| 006 | Bounded autonomy — depth ≤ 5, budget per intent (Rule 8) |
| 007 | Trace everything (Rule 12) |
| 009 | Agentic saga with intelligent recovery |
| 010 | Fast path first — 4 communication patterns (Rule 4) |
| 011 | Sidecar transparent fallback (Rule 13) |
| 012 | Guards on every agent — input/output/action (Rule 7) |
| 013 | Plan cache + intent cache semantic hash |
| 014 | Ontology as first-class (Rule 14) — E2 Slice A |
| 015 | Feedback is a contract (Rule 15) — E1 Slice A |
| 016 | Proposals, not mutations (Rule 20) — E3 Slice A |

These are the architectural decisions iterations 1–5 made. If a
future Claude Code session wants to pick one up and write it
formally, the numbering slot is reserved.

---

## Writing a new ADR

Use the structure of `ADR-008-entity-space-boundaries.md` as
the template. Required sections:

1. **Title** — `ADR-NNN — <short title>`
2. **Status** — Proposed / Accepted / Deprecated / Superseded
3. **Date** — ISO date
4. **Supersedes / Related** — other ADRs this affects
5. **Context** — what problem are we solving, what did we find
6. **Decision** — the concrete choice, bullet-pointed
7. **Consequences** — positive / negative / neutral
8. **Alternatives considered** — at least two, with why we rejected them
9. **References** — links to source, tests, other docs

### When to write an ADR

Write an ADR when your change does any of:

- Touches more than one crate and establishes a new invariant
- Adds a new architectural rule to `CLAUDE.md`
- Changes a contract type's meaning (not just shape)
- Reverses a previous decision
- Introduces a new deployment topology
- Establishes a new integration pattern

### When *not* to write an ADR

- Bug fixes (write them in the iteration log instead)
- Single-crate refactors
- Test additions
- Documentation updates
- Dependency bumps (unless they force a contract change)

---

## ADR status lifecycle

```
Proposed → Accepted → (over time) → Deprecated → Superseded
                                          ↓
                                     archived
```

- **Proposed** — written, under review, not yet merged
- **Accepted** — merged and in effect
- **Deprecated** — the decision still holds for existing code but
  new code should use a different approach
- **Superseded** — replaced by a later ADR. The superseding ADR
  must be named in the `Supersedes:` front-matter.

---

## Rule-to-ADR map

Each architectural rule in `CLAUDE.md` should eventually have an
ADR formalising it. Current mapping (as of iteration 9+):

| Rule | ADR |
|---|---|
| R1 Agents translate, services decide | reserved (001) |
| R2 Typed internals | reserved (002) |
| R3 Services stay untouched | reserved (011) |
| R4 Fast path first | reserved (010) |
| R5 Deterministic core is sacred | reserved (004) |
| R6 Policy at every step | reserved (004) |
| R7 Guard every agent | reserved (012) |
| R8 Depth and budget limits | reserved (006) |
| R9 Compensation before implementation | reserved (005) |
| R10 Context minimisation | (informal) |
| R11 Storage behind traits | reserved (003) |
| R12 Trace everything | reserved (007) |
| R13 Sidecar transparent fallback | reserved (011) |
| R14 Semantics are nouns | **ADR-008** (boundaries) + reserved (014 ontology) |
| R15 Feedback is a contract | reserved (015) |
| R16 Learning never touches the hot path | pending (E1 Slice B) |
| R17 Every adaptation is reversible | pending (E1 Slice B) |
| R18 Policy governs learning | pending (E1 Slice B) |
| R19 Projections default-deny | reserved (016) |
| R20 Proposals, not mutations | reserved (016) |
| R21 Entities are tenant-scoped | **ADR-008** |
| R22 Identity is cryptographic | **ADR-017** |
| R23 Signed manifest | **ADR-017** |
| R24 Provenance as BOM | **ADR-017** |

---

## Further reading

- [../enhancements/](../enhancements/) — per-enhancement design notes
- [../../PROJECT.md](../../PROJECT.md) — the vision document
- [../../CLAUDE.md](../../CLAUDE.md) — the architectural rules
- [../../development/iteration-playbook.md](../../development/iteration-playbook.md)
  — how iterations are structured
