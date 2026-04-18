# Enhancements

> One-page design summaries for each enhancement program.
>
> The complete design rationale and enhancement rules are now
> consolidated into `PROJECT.md` (sections 16-20) and `CLAUDE.md`
> (rules 14-24 + 34-38). The former `PROJECT_ENHANCE.md` and
> `CLAUDE_ENHANCE.md` have been merged and removed.

---

## Four waves

AAF's post-v0.1 work is organised into two waves.

### Wave 1 — Intent-first foundation

The contracts foundation plus the three architectural
elements without which AAF would just be "a workflow engine
with LLM branch points".

| # | Enhancement | One-liner |
|---|---|---|
| **E2** | [Domain Ontology Layer](e2-domain-ontology.md) | A shared vocabulary of real-world nouns that every crate reasons over instead of field names |
| **E1** | [Feedback Spine](e1-feedback-spine.md) | A typed outcome on every observation, plus out-of-band readers that adapt routing, reputation, and fast-path rules |
| **E3** | [Application-Native Surface](e3-app-native-surface.md) | AppEvent / Situation / ActionProposal contracts that let any app embed AAF natively |

Wave 1 lands **strictly in the order E2 -> E1 -> E3**. Each
enhancement goes through three slices (A -> B -> C).

### Wave 2 — Category-defining platform

Three more enhancements built on Wave 1's foundation.

| # | Enhancement | One-liner |
|---|---|---|
| **X1** | [Agent Identity, Provenance & Supply Chain](x1-agent-identity.md) | DIDs, signed manifests, SBOMs, capability tokens, revocation |
| **X2** | Semantic Knowledge Fabric (pending) | First-class retrieval with chunking, embedding, reranking, lineage, and policy gating |
| **X3** | Developer Experience Surface (pending) | Decorator-first SDK, hotloop dev server, simulation harness, snapshot tests |

Wave 2 lands **strictly in the order X1 -> X2 -> X3**.

### Wave 4 — Critical infrastructure

Three prerequisites for framework viability — SDKs, live LLM
providers, and protocol bridges.

| # | Enhancement | One-liner |
|---|---|---|
| **F2** | [Live LLM Integration](f2-llm-integration.md) | Real providers (Anthropic, OpenAI, local), value-based routing, ProviderMetrics |
| **F1** | [Developer Experience Platform](f1-developer-experience.md) | Python/TypeScript/Go SDKs, CLI, code generation from JSON Schema |
| **F3** | [Universal Protocol Bridge](f3-protocol-bridges.md) | MCP client/server, A2A participant, governed external calls |

Wave 4 lands **strictly in the order F2 -> F1 -> F3**. May be
interleaved with Wave 3 (E4/E5/E6).

---

## Status at a glance

```
                   Slice A     Slice B     Slice C
E2 Ontology        done        done        done       <- complete
E1 Feedback        done        done        pending
E3 App-Native      done        pending     pending
X1 Identity        done        done        done       <- complete
X2 Knowledge       pending     pending     pending
X3 DX Surface      pending     pending     pending
F2 LLM Integration done        pending     pending    <- Wave 4 (Slice A landed)
P2 Cap. Invocation done        pending     pending    <- Wave 4 (Slice A landed)
F1 Developer XP    planned     planned     planned    <- Wave 4
F3 Protocol Bridge planned     planned     planned    <- Wave 4
```

See `../../development/roadmap.md` for the authoritative status
board.

---

## How to read an enhancement page

Each page in this directory answers four questions:

1. **What problem does it solve?** One paragraph.
2. **What landed?** A table of deliverables per slice, with the
   file / crate / contract that owns each.
3. **What rules does it introduce or enforce?** The rules from
   `CLAUDE.md` (rules 14-24) the enhancement is designed to
   satisfy.
4. **What's next?** The concrete work the next slice will pick
   up.

---

## For developers planning to implement a slice

If you are about to build one of the deferred slices, read in
order:

1. The enhancement page in this directory (user-facing summary).
2. `PROJECT.md` sections 16-18 at the repo root (enhancement
   designs).
3. `CLAUDE.md` rules 14-24 + 34-43 (enhancement architecture rules,
   Wave 4 infrastructure rules, Three Pillars rules, and slicing
   strategy).
4. `../../development/next-slices.md` (the concrete playbook for
   the next slices).
5. `../../development/iteration-playbook.md` (the 7-step cycle).

For detailed slice-level implementation specifics, the standalone
enhancement content is now in `PROJECT.md` §§16-18 and `CLAUDE.md`
rules 14-24.

---

## Further reading

- [../README.md](../README.md) — the docs index
- [../../PROJECT.md](../../PROJECT.md) — the vision (including
  enhancement designs in sections 16-18)
- [../../CLAUDE.md](../../CLAUDE.md) — architectural rules 1-24 + 34-43
