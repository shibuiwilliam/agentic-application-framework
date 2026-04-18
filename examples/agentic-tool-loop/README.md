# agentic-tool-loop

Demonstrates AAF's **agentic tool loop** (Enhancement E4 Slice B):
an agent that discovers capabilities as typed tools, calls them
iteratively during LLM inference, and produces a final answer
grounded in tool results. This is what makes AAF agents real agents
rather than one-shot LLM wrappers.

## What it exercises

- Non-deterministic capability registered in the registry
  (`deterministic: false` produces a `PlannedStepKind::Agent` step)
- `CapabilityContract` to `ToolDefinition` conversion (Rule 25)
- `AgentNode` with `ToolExecutor` and bounded tool loop (Rule 27)
- Multi-turn message accumulation: the agent calls tools, observes
  results, and continues reasoning until it produces a final answer
- `max_tool_calls` enforcement: the loop terminates gracefully when
  the bound is reached (returns `BudgetExhausted`)
- Tool call records in `NodeOutput.data["tool_calls"]` for trace
  integration (Rule 12)

## Files

- `aaf.yaml` — project config with a non-deterministic warehouse
  capability and a demo goal

## Run it

```bash
# Run the E4 integration tests
cargo test -p aaf-integration-tests --test e4_tool_loop_smoke

# You can also run the full pipeline via the server CLI
# (the server will create an AgentNode for non-deterministic steps)
cargo run -p aaf-server -- run examples/agentic-tool-loop/aaf.yaml
```

## How the tool loop works

```text
Intent: "show stock report for SKU-42"
  |
  v
Planner: discovers cap-stock-lookup (deterministic=false)
  |       -> produces PlannedStepKind::Agent
  v
AgentNode.run():
  |
  +-> Turn 1: LLM(system + goal + tools[stock_lookup])
  |           -> StopReason::ToolUse { name: "stock lookup" }
  |
  +-> Execute tool: stock_lookup({}) -> "42 units available"
  |
  +-> Turn 2: LLM(system + goal + tool_use + tool_result + tools[stock_lookup])
  |           -> StopReason::ToolUse { name: "stock lookup" }
  |
  +-> Execute tool: stock_lookup({}) -> "42 units available"
  |
  +-> Turn 3: LLM(system + goal + ... + tools[stock_lookup])
  |           -> StopReason::EndTurn
  |
  v
NodeOutput {
  content: "final answer based on tool results",
  tool_calls: [{ name: "stock lookup", ... }, { ... }],
  tokens: <accumulated across all turns>,
  cost_usd: <accumulated across all turns>,
}
```

The loop is bounded by `max_tool_calls` (default 10, Rule 27). If the
LLM keeps requesting tools past the limit, the node terminates with
`StopReason::BudgetExhausted` and returns partial results.
