# F2 — Live LLM Integration & Intelligent Model Routing

> Wave 4 critical infrastructure. See `PROJECT.md` §20.2 and
> `CLAUDE.md` rule 35.

## Status: Slice A Landed, Slices B/C Pending

## What has landed (Slice A)

- `AnthropicProvider` in `aaf-llm/src/anthropic.rs` — real Anthropic
  Messages API integration with `HttpSender` (production) and
  `FixedSender` (testing) traits for testability.
- `ProviderMetrics` on every response (Rule 35).
- `ModelPricing` in `aaf-llm/src/pricing.rs` — per-model cost
  calculation with `anthropic_pricing()` default catalog.
- `calculate_cost()` helper for budget-aware routing.
- Tool-use support: `ChatRequest.tools`, `ToolChoice`, `StopReason`,
  `ToolUseBlock`, `ToolResultBlock` (E4 Slice A).
- `MultiTurnMockProvider` for bounded agentic loop testing.

## What remains (Slices B/C)

- OpenAI provider (`aaf-llm/src/openai.rs`)
- Local provider for Ollama/vLLM (`aaf-llm/src/local.rs`)
- Health tracking and automatic fallback in `ValueRouter`
- Streaming (`chat_stream`) support
- Budget pre-check before LLM calls
- Configuration-driven provider setup

## Original Problem

`aaf-llm` defined an `LLMProvider` trait and a `MockProvider`.
Slice A added the first concrete implementation (Anthropic Claude).
OpenAI and local model providers remain pending.

## Solution

### Providers

| Provider | API | File |
|---|---|---|
| Anthropic Claude | Messages API | `aaf-llm/src/anthropic.rs` |
| OpenAI | Chat Completions API | `aaf-llm/src/openai.rs` |
| Local (Ollama/vLLM) | OpenAI-compatible | `aaf-llm/src/local.rs` |

### ProviderMetrics (Rule 35)

Every LLM call records:

```rust
pub struct ProviderMetrics {
    pub model: String,
    pub input_tokens: u64,
    pub output_tokens: u64,
    pub cost_usd: f64,
    pub latency_ms: u64,
    pub rate_limit_remaining: Option<u32>,
    pub provider: String,
}
```

### Value-Based Router

Selects the optimal provider + model for each request:

1. Filter by capability (tools/streaming support)
2. Filter by health (recent failures excluded)
3. Filter by budget (estimated cost vs remaining)
4. Filter by latency (median vs deadline)
5. Filter by classification (data processing agreements)
6. Score: cost (40%) + latency (30%) + capability (30%)

### Pricing Table

`ModelProfile` struct with per-model pricing. Default catalog
includes Claude Sonnet/Haiku, GPT-4o/mini. Overridable via config.

## Slices

| Slice | Scope |
|---|---|
| A | Anthropic provider + ProviderMetrics + pricing table — **LANDED** |
| B | OpenAI + Local providers + ValueRouter + health + fallback |
| C | Streaming + budget pre-check + config + classification filtering |

## Rules

- **R35** Providers are observable
- **R8** Budget enforcement at the wire
- **R12** Every routing decision traced

## Dependencies

- None (standalone). F1 and F3 depend on F2.

## Key decisions

- **reqwest over provider SDKs:** Unified error handling, minimal
  dependency tree, fine-grained retry/timeout control.
- **Pricing as data, not code:** `ModelProfile` structs, overridable
  via config file.
