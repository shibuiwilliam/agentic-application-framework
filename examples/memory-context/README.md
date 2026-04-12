# memory-context

Demonstrates AAF's **four-layer memory model** and **context budget**
(Rule 10) -- how agents store, retrieve, and budget information across
task execution, conversation continuity, long-term knowledge, and
artifact provenance.

## What this example covers

### Four-Layer Memory Model (PROJECT.md 3.6)

| Layer | Purpose | Where it's exercised |
|---|---|---|
| **Working** (Layer 1) | Per-task transient state | Put/get round-trip, overwrite, clear, task isolation |
| **Thread** (Layer 2) | Per-conversation continuation | Append-only log, insertion order, thread isolation |
| **Long-term** (Layer 3) | Semantic/episodic/procedural knowledge | Keyword search, entity-keyed retrieval (Rule 14), tenant isolation (Rule 21), limit, multi-entity indexing |
| **Artifact** (Layer 4) | Produced outputs with provenance | Full provenance chain (intent, task, trace, agent, capability, model), content fidelity, metadata |

### Context Budget (Rule 10)

| Feature | Where it's exercised |
|---|---|
| **Default budget** | 7,500 total: System (2,000) + Intent (500) + Memory (2,000) + Step (1,000) + Tools (2,000) |
| **Token approximation** | chars / 4 heuristic across LLM tokenizers |
| **Per-section truncation** | Long text capped at each section's token limit |
| **Short text passthrough** | Text within budget passes through unmodified |

### Full Pipeline Integration

A multi-step workflow demonstrates all layers working together:
1. Store intermediate state in working memory
2. Execute a graph (intent -> plan -> trace)
3. Produce an artifact with full provenance chain
4. Index the event in long-term memory under an entity
5. Retrieve by entity reference
6. Follow the artifact_id from long-term to artifact store
7. Verify tenant isolation blocks cross-tenant access
8. Clean up working memory at task completion

## Files

```
examples/memory-context/
├── README.md       <- this file
└── aaf.yaml        <- context budget config + 4-layer memory setup
```

## Run the tests

```bash
cargo test -p aaf-integration-tests --test memory_context_e2e
```

Expected output:

```text
running 20 tests
test working_memory_put_get_round_trip ... ok
test working_memory_overwrite_replaces_value ... ok
test working_memory_clear_removes_all_entries ... ok
test working_memory_task_isolation ... ok
test thread_memory_preserves_insertion_order ... ok
test thread_memory_isolation ... ok
test longterm_keyword_search ... ok
test longterm_entity_keyed_retrieval ... ok
test longterm_tenant_isolation ... ok
test longterm_search_respects_limit ... ok
test longterm_multi_entity_indexing ... ok
test artifact_round_trip_preserves_provenance ... ok
test artifact_content_intact ... ok
test context_budget_default_matches_spec ... ok
test context_budget_token_approximation ... ok
test context_budget_truncation ... ok
test context_budget_fit_per_section ... ok
test context_budget_short_text_passthrough ... ok
test full_pipeline_working_to_artifact_to_longterm ... ok
test aaf_yaml_loads_successfully ... ok

test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

## Architecture rules exercised

| Rule | How |
|---|---|
| **Rule 10** (Context Minimization) | ContextBudget enforces ~7,500 tokens per LLM call across 5 sections |
| **Rule 11** (Storage Behind Traits) | MemoryFacade wraps `Arc<dyn Trait>` for all four backends; in-memory for tests, pluggable for production |
| **Rule 14** (Entity-Keyed Memory) | Long-term records are indexed under ontology entities for O(1) retrieval |
| **Rule 21** (Tenant Isolation) | All search operations are tenant-scoped; cross-tenant queries return empty |

## See also

- `CLAUDE.md` -- Rule 10 (Context Minimization), Rule 11 (Storage Behind Traits)
- `PROJECT.md` -- four-layer memory model (Working, Thread, Long-term, Artifact)
- `examples/hello-agent/` -- simplest AAF pipeline (read-only)
- `examples/order-saga/` -- multi-step saga with compensation
- `examples/resilient-query/` -- guards, degradation, budget
- `examples/feedback-loop/` -- trust lifecycle + learning
- `examples/app-native-surface/` -- events, proposals, projections
- `examples/signed-agent/` -- identity + provenance CLI walkthrough
