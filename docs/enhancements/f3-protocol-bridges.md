# F3 — Universal Protocol Bridge (MCP + A2A)

> Wave 4 critical infrastructure. See `PROJECT.md` §20.3 and
> `CLAUDE.md` rules 36 and 38.

## Status: Planned

## Problem

AAF's `adapters/` directory is empty. The framework claims to be
the orchestration layer between agents, services, and APIs, but
has no concrete protocol implementations — no MCP client/server,
no A2A participant.

## Solution

### MCP Client (`adapters/mcp/client/`)

Connect AAF to external MCP tool servers. Every external tool
call is governed by AAF's policy engine (Rule 36).

- **Transports:** stdio (local), SSE (remote), streamable HTTP
- **Discovery:** `tools/list` → `CapabilityContract` registration
  (prefixed `mcp:{server_name}:`)
- **Invocation:** policy check → budget check → `tools/call` →
  policy check → observation → budget charge

### MCP Server (`adapters/mcp/server/`)

Expose AAF capabilities as MCP tools. External AI tools (Claude
Code, Cursor, etc.) can use governed enterprise services.

- `handle_list_tools()` — query AAF registry
- `handle_call_tool()` — build `IntentEnvelope`, submit to runtime

### A2A Participant (`adapters/a2a/`)

- Agent Card serving (builds on `aaf-registry::a2a`)
- Task lifecycle: send / get / cancel
- DID-based trust propagation (Rule 22)
- SSE streaming for task updates
- Federation agreement enforcement

### ProtocolBridge Unifier

Unified `CapabilityInvoker` that dispatches to local, MCP, or A2A:

```
Protocol::Local → local_invoker.invoke(...)
Protocol::Mcp   → mcp_client.invoke_tool(...)
Protocol::A2a   → a2a_participant.handle_task_send(...)
```

## Governance (Rule 36)

Every external protocol interaction passes through:

1. **Policy Engine** (Rule 6) — 4 hooks
2. **Trust Boundaries** (Rule 22) — DID-based
3. **Budget Tracker** (Rule 8) — cost charged
4. **Trace Recorder** (Rule 12) — observation recorded

Ungoverned external calls are architecturally impossible.

## Degradation (Rule 38)

- MCP server unreachable → capability removed from registry
- A2A agent unavailable → capability marked degraded
- Never causes a pipeline crash

## Slices

| Slice | Scope |
|---|---|
| A | MCP client: stdio transport, discovery, governed invocation |
| B | MCP server + SSE/streamable HTTP transports |
| C | A2A participant + ProtocolBridge unifier |

## Rules

- **R36** Protocol bridges are governed
- **R38** Bridge failures are graceful
- **R22** Identity is cryptographic (DID-based trust across A2A)

## Dependencies

- F2 (MCP tool calls may need LLM)
- `aaf-registry` (capability discovery)
- `aaf-policy` (governed bridges)
- `aaf-identity` (DID for A2A trust)
