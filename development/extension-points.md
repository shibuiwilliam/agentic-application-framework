# Extension Points

> How to plug new behaviour into AAF without forking the core
> crates. Three main pathways:
>
> 1. Adding a new **policy rule**
> 2. Adding a new **LLM provider**
> 3. Adding a new **storage backend**
>
> Everything here is code-oriented. Each section walks the
> exact trait, the file to edit, the test to add, and the
> gotchas to avoid.

---

## 1. Adding a new policy rule

Policy rules live in `core/crates/aaf-policy/src/rules/`. The
seven defaults (scope, side_effect, budget, pii, injection,
composition, boundary) are loaded via
`PolicyEngine::with_default_rules`. A custom rule is just one
more implementation of the `Rule` trait.

### The trait

```rust
// core/crates/aaf-policy/src/rules/mod.rs
pub trait Rule: Send + Sync {
    fn id(&self) -> &str;
    fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation>;
}
```

- `id()` — stable rule id (kebab-case, e.g. `"tenant-isolation"`)
- `evaluate()` — returns `Some(violation)` or `None`

### Step-by-step

1. **Create the file.** `core/crates/aaf-policy/src/rules/tenant_isolation.rs`
   (or a new module — no hard rules about where).
2. **Implement the trait:**

   ```rust
   use super::Rule;
   use crate::context::PolicyContext;
   use aaf_contracts::{PolicySeverity, PolicyViolation, RuleKind};

   /// Rejects a capability whose writes declare a tenant that
   /// differs from the active tenant.
   pub struct TenantIsolation;

   impl Rule for TenantIsolation {
       fn id(&self) -> &str { "tenant-isolation" }

       fn evaluate(&self, ctx: &PolicyContext<'_>) -> Option<PolicyViolation> {
           let cap = ctx.capability?;
           let active = ctx.tenant?;
           for w in &cap.writes {
               if let Some(t) = &w.tenant {
                   if t != active {
                       return Some(PolicyViolation {
                           rule_id: self.id().into(),
                           kind: RuleKind::BoundaryEnforcement,
                           severity: PolicySeverity::Fatal,
                           message: format!(
                               "cap {} writes entity {} in tenant {}, active is {}",
                               cap.id, w.entity_id, t, active
                           ),
                       });
                   }
               }
           }
           None
       }
   }
   ```

3. **Add `pub mod tenant_isolation;`** to `rules/mod.rs`.

4. **Register the rule at engine construction:**

   ```rust
   // at your server's wiring site
   let mut engine = PolicyEngine::with_default_rules();
   engine.add_rule(Arc::new(TenantIsolation));
   ```

5. **Write tests.** Each new rule must ship ≥ 3 unit tests:
   happy-path (no violation), violation-detected, and
   edge-case (e.g. no tenant → no violation).

   ```rust
   #[cfg(test)]
   mod tests {
       use super::*;
       use crate::context::PolicyContext;
       // ... fixture helpers per development/coding-conventions.md ...

       #[test]
       fn allows_same_tenant_write() { ... }
       #[test]
       fn rejects_cross_tenant_write() { ... }
       #[test]
       fn ignores_writes_without_tenant() { ... }
   }
   ```

### Severity ladder — pick carefully

The `PolicyEngine::evaluate` decision algorithm is:

1. `Error` or `Fatal` → `Deny`
2. `SideEffectGate` violation → `RequireApproval`
3. Any remaining warnings → `AllowWithWarnings`
4. Otherwise → `Allow`

So your severity choice determines whether the rule blocks
execution or just logs a warning. Use:

- **`Fatal`** — invariant violation that must never pass. Runtime
  fails immediately. Example: `tenant-boundary`.
- **`Error`** — violation that should not pass in production but
  is not catastrophic. Runtime still denies. Example: `scope`.
- **`Warning`** — advisory, does not block unless combined with a
  side-effect gate. Example: `pii-output-minor`.
- **`Info`** — logged only.

### Plugin via `PolicyContext`

If your rule needs a lookup (e.g. "is this entity classified
PII?"), do **not** import the ontology crate — that would
create a dependency cycle. Instead, pass a callback through
`PolicyContext`. The boundary rule already does this with
`ontology_class_lookup: Option<OntologyClassificationLookup>`;
extend the same struct for your new rule's needs.

If you extend `PolicyContext` you **must** update every
construction site:

```bash
grep -rn "composed_writes:" core/crates/ core/tests/
```

See [known-gotchas.md](known-gotchas.md) → G2 for the history of
the last time someone added a field to `PolicyContext`.

---

## 2. Adding a new LLM provider

LLM providers live in `core/crates/aaf-llm/src/`. The trait is
tiny: two methods.

### The trait

```rust
// core/crates/aaf-llm/src/provider.rs
#[async_trait]
pub trait LLMProvider: Send + Sync {
    fn name(&self) -> &str;
    async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LLMError>;
}
```

### Step-by-step

1. **Create the file.** `core/crates/aaf-llm/src/anthropic.rs` (or
   `openai.rs`, `bedrock.rs`, `ollama.rs`, etc.).
2. **Implement the trait:**

   ```rust
   use crate::provider::{ChatRequest, ChatResponse, LLMError, LLMProvider};
   use async_trait::async_trait;

   pub struct AnthropicProvider {
       api_key: String,
       model: String,
       http: reqwest::Client,  // or whatever HTTP client the workspace pins
   }

   impl AnthropicProvider {
       pub fn new(api_key: String, model: String) -> Self {
           Self { api_key, model, http: reqwest::Client::new() }
       }
   }

   #[async_trait]
   impl LLMProvider for AnthropicProvider {
       fn name(&self) -> &str { "anthropic" }

       async fn chat(&self, req: ChatRequest) -> Result<ChatResponse, LLMError> {
           // 1. Translate ChatRequest to the Anthropic Messages API
           //    JSON payload.
           // 2. POST to https://api.anthropic.com/v1/messages
           // 3. Map the response to ChatResponse { content, tokens, cost_usd, ... }
           // 4. Map any HTTP/parse error to LLMError.
           todo!()
       }
   }
   ```

3. **Add the dependency.** The workspace does not currently pin
   `reqwest` or any HTTP client — you must add it to
   `core/crates/aaf-llm/Cargo.toml` under `[dependencies]` with
   a Rust-1.70-compatible version. Prefer `ureq` for a
   lightweight sync HTTP client wrapped in `tokio::task::spawn_blocking`;
   `reqwest` pulls a large tree and has historically been the
   cause of build breakage.

4. **Wire it into the router.**

   ```rust
   // at your server's wiring site
   let mut router = ValueRouter::new();
   router.register(RoutingTier::High,
       Arc::new(AnthropicProvider::new(api_key, "claude-opus-4-6".into())));
   router.register(RoutingTier::Low,
       Arc::new(MockProvider::default()));  // keep mock for tests
   ```

5. **Write tests.** Providers are awkward to test because they
   hit real APIs. Do **not** commit tests that make network
   calls. Instead:

   - Unit-test your `ChatRequest → wire-format` translation as a
     pure function with a recorded expected JSON.
   - Unit-test your wire-format → `ChatResponse` mapping similarly.
   - Integration-test via a local mock HTTP server (`wiremock`
     or a hand-rolled `tokio::net::TcpListener`) — optional.

### Budget integration

Every provider implementation is responsible for reporting
`tokens` and `cost_usd` in its `ChatResponse`. The runtime's
`BudgetTracker` will use those values to enforce Rule 8 —
under-reporting or forgetting to report them lets an agent
exceed its budget silently.

### Router rotation

The value router picks a tier based on the intent's risk tier
(Read → `Low`, Write → `Medium`, Governance → `High`). You can
override via `ValueRouter::register` at any tier. When E1
Slice B lands, `LearnedRoutingPolicy` will plug into the router
and pick tiers based on observed cost/quality per
`(intent_type, risk_tier, entity_class)`.

---

## 3. Adding a new storage backend

Storage traits live in `core/crates/aaf-storage/src/`. **No
other crate may import a database driver directly** (Rule 11).
If you want to add PostgreSQL, Redis, S3, ClickHouse, or
pgvector support, the changes live *inside* `aaf-storage` behind
an additional trait impl.

### The eight storage traits

```rust
// core/crates/aaf-storage/src/
CheckpointStore        // graph execution checkpoints
WorkingMemoryStore     // per-task transient state
ThreadMemoryStore      // per-session state
LongTermMemoryStore    // persistent knowledge (keyword + entity indexes)
ArtifactStore          // artifact bytes + metadata
TraceStore             // observations + outcomes
RegistryStore          // capability CRUD + listing
// plus aaf-identity::RevocationRegistry (not in aaf-storage because
// it is agent-identity-aware)
```

Each trait has an `InMemory*` implementation in the same file.
Your new backend adds a sibling — for example
`aaf-storage/src/postgres/checkpoint.rs` implementing
`CheckpointStore`.

### Step-by-step (using PostgreSQL via sqlx as the example)

1. **Create a sub-module.** `core/crates/aaf-storage/src/postgres/mod.rs`
   + one file per trait impl. This keeps driver code in one
   place.

2. **Add the driver dep.** Edit
   `core/crates/aaf-storage/Cargo.toml`:

   ```toml
   [dependencies]
   sqlx = { version = "0.7.4", features = ["postgres", "runtime-tokio-rustls", "chrono", "uuid"] }
   ```

   **Check Rust 1.70 compatibility.** The workspace is pinned to
   1.70; not every sqlx release compiles on it. Iteration 5
   documented that newer sqlx depends on edition 2021 features
   that 1.70 supports, but newer `rustls` versions may not.
   Verify with `cargo build -p aaf-storage` on a clean checkout
   before adding the dep to the lockfile.

3. **Implement the trait.** Example for `CheckpointStore`:

   ```rust
   use crate::checkpoint::{Checkpoint, CheckpointStore};
   use crate::error::StorageError;
   use async_trait::async_trait;
   use sqlx::PgPool;

   pub struct PgCheckpointStore { pool: PgPool }

   impl PgCheckpointStore {
       pub async fn connect(url: &str) -> Result<Self, StorageError> {
           let pool = PgPool::connect(url).await.map_err(|e| {
               StorageError::Backend(format!("postgres connect: {e}"))
           })?;
           Ok(Self { pool })
       }
   }

   #[async_trait]
   impl CheckpointStore for PgCheckpointStore {
       async fn put(&self, cp: Checkpoint) -> Result<(), StorageError> {
           sqlx::query(/* INSERT ... */)
               .bind(...)
               .execute(&self.pool)
               .await
               .map_err(|e| StorageError::Backend(e.to_string()))?;
           Ok(())
       }

       // other methods ...
   }
   ```

4. **Expose it via a module re-export.** Edit
   `core/crates/aaf-storage/src/lib.rs`:

   ```rust
   #[cfg(feature = "postgres")]
   pub mod postgres;
   ```

   Feature-gate so the default build stays fast and driver-free.
   Add `postgres = ["sqlx"]` to `[features]`.

5. **Write tests.** Use the standard pattern:

   - Unit test: mock the pool / use `sqlx::test` macro.
   - Integration test: `docker-compose up postgres` + a
     `#[ignore]`d test that runs when `DATABASE_URL` is set.
     Iteration 1's test strategy marks these as "out of band" so
     they do not block `cargo test`.

6. **Wire it at the server.** In `aaf-server::cmd_run`, swap
   the in-memory store:

   ```rust
   #[cfg(feature = "postgres")]
   let checkpoints: Arc<dyn CheckpointStore> = Arc::new(
       PgCheckpointStore::connect(&cfg.database_url).await?
   );
   #[cfg(not(feature = "postgres"))]
   let checkpoints: Arc<dyn CheckpointStore> = Arc::new(
       InMemoryCheckpointStore::new()
   );
   ```

### Invariants

- **Every store impl is `Send + Sync`.** The trait requires
  this.
- **Every method is `async`.** The traits use `#[async_trait]`.
- **Errors map to `StorageError`.** Do not leak the underlying
  driver error type into the caller.
- **Do not panic.** Surface every failure as `Err`. The executor
  maps storage errors through `RuntimeError::Node`; a panic
  poisons the trace.
- **Tenant isolation.** Every method that takes a `TenantId`
  must enforce it at the SQL / Redis / etc. layer — a
  bug-bitten tenant leak is the worst security failure in this
  codebase.

### Rollout

Backend additions are feature-gated. The default build and the
default integration tests always use in-memory. New backends
are opt-in (`cargo build --features postgres`). Ship the
in-memory path first, prove the real backend in a separate
feature, then swap via config.

---

## Other extension points (short version)

### New capability source

Wire a capability publisher into `aaf-sidecar::capability::CapabilityPublisher`
or `aaf-wrapper::capability::MethodToCapability`. No trait
extension needed — both crates accept a `CapabilityContract`
and call `Registry::register`.

### New transport

Implement the `aaf-transport::Transport` trait. Real drivers
(gRPC via tonic, REST via axum, NATS, WebSocket, CloudEvents)
are all deferred to post-Wave-2; the trait skeleton exists in
`aaf-transport::lib`.

### New ontology resolver strategy

Implement `aaf-ontology::resolver::EntityResolver`. The
default `ExactMatchResolver` handles the simple case; a future
iteration will add `EmbeddingResolver` (vector-based) and
`ExternalResolver` (calls out to a resolver service).

### New judge for `aaf-eval`

Implement the `Judge` trait. The shipped
`DeterministicJudge` uses Jaccard similarity; a Slice C
follow-up could add `LLMJudge` backed by an `LLMProvider`.

### New fast-path rule source

The `FastPathRuleSet` accepts both hand-authored rules (from a
YAML config) and learned rules (from `aaf-learn::fast_path_miner`,
landing in E1 Slice B). If you want to add rules from an
external source, write a loader that produces
`Vec<FastPathRule>` and calls `FastPathRuleSet::add_rule` or
(once E1 B lands) `FastPathRuleSet::add_learned`.

---

## What *not* to extend via this path

- **Contract types.** Do not add a new variant to
  `IntentType` or `TaskState` as an "extension"; those are
  core shapes that require a contract version bump. See
  [changing-contracts.md](changing-contracts.md).
- **Architecture rules.** The thirteen (plus the E1/E2/E3 and
  X1 rules) are in `CLAUDE.md`. Adding a new rule is an
  architectural decision, not an extension — write an ADR first.
- **The policy hook points.** `PrePlan`, `PreStep`, `PostStep`,
  `PreArtifact` are the four canonical hook points in the
  runtime. Adding a fifth is a runtime change that ripples
  through every policy-context construction site — see
  [runtime-internals.md](runtime-internals.md) → "Five hook
  points".

---

## Further reading

- [coding-conventions.md](coding-conventions.md) — the shape
  your new impl has to fit
- [testing-strategy.md](testing-strategy.md) — where and how
  to test your extension
- [known-gotchas.md](known-gotchas.md) — the pitfalls that have
  already bitten
- [runtime-internals.md](runtime-internals.md) — the runtime
  surface your extension plugs into
