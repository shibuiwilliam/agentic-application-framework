# F1 — Developer Experience Platform

> Wave 4 critical infrastructure. See `PROJECT.md` §20.1 and
> `CLAUDE.md` rules 34 and 37.

## Status: Planned

## Problem

AAF has no SDK in any language. The only way to interact with AAF
is through Rust integration tests or raw HTTP/gRPC calls. CLAUDE.md
defines comprehensive SDK structures but none exist on disk.

## Solution

### Python SDK (`sdk/python/`)

`pip install aaf-sdk`

- `@capability` / `@guard` / `@compensation` decorators
- Pydantic v2 models generated from `spec/schemas/`
- `AafClient` with `submit_intent()` and `submit_intent_stream()`
- `MockRuntime` for in-process testing
- `aaf` CLI: init / dev / test / run / trace

### TypeScript SDK (`sdk/typescript/`)

`npm install @aaf/sdk`

- Zod schemas generated from `spec/schemas/`
- Type-safe capability builder
- Streaming event consumer
- Vitest test utilities

### Go SDK (`sdk/go/`)

`go get github.com/aaf/sdk-go`

- Client + sidecar builder + wrapper builder
- `context.Context`-threaded

### Code Generation (`scripts/codegen/`)

JSON Schema → SDK contract types:

- Python: pydantic v2 models
- TypeScript: zod schemas + TS interfaces
- Go: Go structs

Run via `make codegen`. Contract types are never hand-written in
SDKs (Rule 34).

## Architecture

SDKs are **thin clients** over HTTP/gRPC/WebSocket APIs. They do
NOT embed the runtime.

```
Developer Code (Python/TS/Go)
  └── AAF SDK (decorators, client, testing)
        │  HTTP / gRPC / WebSocket
        ▼
  AAF Runtime (Rust)
```

Policy enforcement happens server-side (cannot be bypassed by SDK).

## Slices

| Slice | Scope |
|---|---|
| A | Python SDK core: codegen, decorators, client, MockRuntime |
| B | TypeScript SDK + `aaf` CLI commands |
| C | Go SDK + sidecar/wrapper builders + end-to-end example |

## Rules

- **R34** SDKs are generated, not hand-written
- **R37** SDK ergonomics over completeness

## Dependencies

- F2 (SDK demos need real LLM providers)
- `aaf-server` API endpoints (HTTP/gRPC/WS)
