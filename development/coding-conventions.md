# Coding Conventions

> Not a generic Rust style guide — the patterns that make *this*
> codebase consistent. Everything here is enforced by clippy,
> reviewed in every iteration, or shipped as a concrete example in
> the tree.

---

## Edition, toolchain, clippy

- **Edition 2021**, `rust-version = "1.70"`.
- `PROJECT.md` calls for Edition 2024; the migration is deferred.
- Clippy baseline: `-W clippy::all` must stay at zero warnings.
  Pedantic (`-W clippy::pedantic`) is a long-term goal but not yet
  green.
- Formatter: `cargo fmt --all`. Run `make fmt-check` before
  pushing.

---

## Every crate has the same skeleton

```
aaf-<name>/
├── Cargo.toml           description field, deps from [workspace.dependencies]
└── src/
    ├── lib.rs           #![deny(missing_docs)], #![forbid(unsafe_code)], pub use re-exports
    ├── error.rs         thiserror-based CrateError
    ├── <module>.rs      one file per concept; nested modules via mod.rs
    └── …
```

**Required attributes on every lib.rs:**

```rust
#![deny(missing_docs)]
#![forbid(unsafe_code)]
```

**Required derives on every public type:**

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
```

Drop `PartialEq` when the type contains a `Box<dyn Fn>` or an
`Arc<dyn Trait>`; drop `Serialize`/`Deserialize` when the type is
internal-only (not part of a contract).

---

## Errors

- Library crates use `thiserror`:

  ```rust
  #[derive(Debug, thiserror::Error)]
  pub enum CrateError {
      #[error("storage: {0}")]
      Storage(#[from] aaf_storage::StorageError),
      #[error("invalid input: {0}")]
      Invalid(String),
  }
  ```

- Binary crates (`aaf-server`) may use `Box<dyn std::error::Error>`
  at the `main` boundary. Do not use `anyhow` — it has never been
  a workspace dep.
- **Never** use `unwrap()` in library code. `unwrap()` is
  permitted in test code only.
- Prefer `?` over `match … Err(e) => return Err(e)`.

---

## Async

- `tokio` is the only runtime.
  `tokio = "=1.28.2"` features: `["macros", "rt-multi-thread", "sync", "time"]`.
- Public async functions on traits go through `#[async_trait]`.
- Non-trait-bound async fns should *not* use `#[async_trait]`;
  write `pub async fn foo() -> Result<…>` directly.
- Use `tokio::sync::RwLock` only when you actually need
  cross-thread sync; prefer `parking_lot::RwLock` for sync-only
  state. Every in-memory backend uses `parking_lot::RwLock`
  behind an `Arc<>`.

---

## Doc comments

- Every public item has a `///` doc comment. Clippy's
  `missing_docs` is deny-level on every lib.rs.
- Every doc comment starts with a single-line summary that fits on
  one line of the rustdoc output.
- Cross-reference other items with `[`InlineBackticks`]` — rustdoc
  will link them.
- Use `# Examples`, `# Errors`, `# Panics` headings when they
  apply; they drive the rustdoc layout.
- Short crate-level doc comments in every `lib.rs` reference the
  architecture rules the crate enforces (see `aaf-policy/src/lib.rs`
  for the canonical example).

---

## Test fixtures

- Put fixture constructors at the top of the `tests` module:

  ```rust
  fn sample_intent() -> IntentEnvelope { … }
  fn sample_cap(id: &str) -> CapabilityContract { … }
  ```

- Every crate has exactly one `fn sample_*` style per concept —
  don't reinvent it mid-file.
- When a struct grows a new field (especially `CapabilityContract`,
  `IntentEnvelope`, `Artifact`), **update every fixture
  constructor** across every crate. Iteration 5 fixed twelve
  stale construction sites; do not let the count grow again.
- Prefer explicit field-by-field literals over `..Default::default()`
  so the compiler tells you when a new field is added.

---

## Ids

- Every identifier type is a newtype around `String`:

  ```rust
  #[derive(Debug, Clone, Hash, Eq, PartialEq, Serialize, Deserialize)]
  pub struct IntentId(String);
  impl IntentId {
      pub fn new() -> Self { … }
      pub fn as_str(&self) -> &str { &self.0 }
  }
  impl From<&str> for IntentId { fn from(s: &str) -> Self { Self(s.into()) } }
  impl std::fmt::Display for IntentId { … }
  ```

- Always use `*::new()` in runtime code, `*::from("literal")` in
  tests.

---

## Traits and trait objects

- Storage traits are **object-safe** — `trait Store: Send + Sync`
  with async methods via `#[async_trait]`. Every call site owns an
  `Arc<dyn Store>`.
- Provide `InMemory*` implementations for every trait.
- Provide default trait method impls when widening a trait so
  downstream crates keep compiling
  (`LongTermMemoryStore::search_by_entity` is the canonical
  example).

---

## Construction-time invariants

Whenever a type has an invariant, enforce it in the constructor
and make the fields unreachable from outside the module unless the
invariant holds. Examples:

- `CapabilityContract::validate` called by `Registry::register` —
  Rule 9.
- `IntentEnvelope::validate` called by `IntentCompiler::compile` —
  Rule 8 depth + budget.
- `ActionProposal::new_with_mutations` returns `Err` if
  `mutations` is non-empty and `compensation_ref` is `None` —
  Rule 20.
- `StateProjection::allows_field` defaults to `false` — Rule 19.
- `AgentManifest::build` is the only public constructor; it signs
  at build time — Rule 23.

**Rule:** if the invariant can be expressed at construction time,
do it there. Never defer the check to a `validate_later()` method
that callers might forget.

---

## Serde

- Every contract type that crosses the wire is `#[derive(Serialize, Deserialize)]`.
- For enum variants use `#[serde(rename_all = "snake_case")]`.
- For optional fields use `#[serde(default, skip_serializing_if = "Option::is_none")]`.
- For slices use `#[serde(default, skip_serializing_if = "Vec::is_empty")]`.
- Never `#[serde(deny_unknown_fields)]` on a contract type —
  future fields must deserialize on old consumers.

---

## Logging / tracing

- `tracing` is a workspace dep, but most crates do not use it
  yet — `aaf-trace::Recorder` is the canonical observation path.
- If you need diagnostic logging inside a node, prefer `tracing`
  with a crate-local target name, e.g.
  `tracing::debug!(target: "aaf_runtime::executor", …)`.
- Never `println!` in library code; it ends up in tests.

---

## Naming

- Crate names are lowercase, `aaf-` prefixed, no underscores.
- Module names are lowercase, underscore-separated.
- Types are `UpperCamelCase`.
- Trait names use nouns (`Store`, `Recorder`, `Signer`, `Judge`)
  not adjectives (`Storable`, `Recordable`).
- Error variant names are declarative (`MissingCompensation`,
  `DepthExceeded`) not procedural (`CompensationNotFound`).
- Field names are `snake_case`, descriptive, lossless — no
  `_` prefix, no `my_` / `the_`.

---

## File size

- Files over 500 lines should split into a sub-module.
- `core/crates/aaf-contracts/src/capability.rs` is the largest
  contract file at ~385 lines — still fits on two screens in a
  normal editor.
- `core/crates/aaf-runtime/src/executor.rs` is the largest
  behaviour file — if it grows past 500 lines, split executor hooks
  into their own module.

---

## Commit / edit hygiene

- Each logical change goes in a single commit or a single file
  edit batch. Do not bundle "add feature X + fix bug Y + rename Z"
  into one commit.
- Commit messages follow Conventional Commits: `feat:`, `fix:`,
  `docs:`, `refactor:`, `test:`, `chore:`.
- Branches follow the same conventions.
- Never edit generated code; there is no generated code today, but
  the protobuf / buf generators are on the roadmap.
