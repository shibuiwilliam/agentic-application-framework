# Ontology Lint

> How to run and interpret `make ontology-lint`. Shipped in
> **E2 Slice C** (iteration 8) to ratchet capability adoption of
> entity declarations.

---

## What it checks

For every `capability-*.yaml` under the scanned directory:

| Finding | Severity | Condition |
|---|---|---|
| `Ok` | informational | `reads` / `writes` / `emits` is non-empty |
| `Warn` | informational | Capability has no entity declarations and its side-effect is `None` or `Read` |
| `Error` | blocking (strict mode only) | Capability's side-effect is `Write` / `Delete` / `Send` / `Payment` and `writes:` is empty |

Parse failures and read failures are always `Error` regardless of
mode.

---

## The adoption ratio ramp

```
adoption_ratio = with_declarations / scanned

if adoption_ratio >= 0.90:
    mode = strict         // errors block
else:
    mode = warn-only      // errors downgrade to warnings
```

The threshold is a **constant**
(`ADOPTION_STRICT_THRESHOLD = 0.90`) — there is one value in the
entire codebase and it is not configurable. The threshold exists
so that a directory's overall health determines how strict the
tool is: before 90% adoption, the team is still learning the
convention and you do not want a single bad capability blocking
CI; after 90%, the convention is expected and missing `writes:`
is a regression.

---

## Running it

```bash
make ontology-lint                                 # default: scans spec/examples
target/debug/aaf-server ontology lint <dir>       # any directory
cargo run -p aaf-server -- ontology lint <dir>    # cargo-invoked
```

Exit code:

- `0` — no errors (either clean or warn-only mode).
- `1` — at least one `Severity::Error` finding in strict mode.

---

## Example output

### Clean (strict mode, 0 findings)

```
scanned: 2, with declarations: 2 (100%), mode: strict
  OK   cap-inventory-check   capability-inventory.yaml  — has entity declarations
  OK   cap-payment-execute   capability-payment.yaml    — has entity declarations

0 errors, 0 warnings
```

### Mixed (warn-only mode)

```
scanned: 3, with declarations: 1 (33%), mode: warn-only
  OK   cap-order-read          capability-order-read.yaml    — has entity declarations
  WARN cap-something            capability-something.yaml     — capability has no entity declarations; add `reads:` to let the planner key memory retrieval off nouns
  WARN cap-broken-payment       capability-broken-payment.yaml — capability declares side_effect `payment` but `writes:` is empty; the boundary rule and composition checker cannot reason about it

0 errors, 2 warnings
```

Notice the second warning. In strict mode this would be an
`Error` — the capability declares `payment` side-effect but
carries no `writes:`. In warn-only mode (adoption 33% < 90%) it
is downgraded.

---

## What to do with warnings

Each warning is a concrete action item:

- **"capability has no entity declarations"** — add `reads:`
  (or `writes:` / `emits:`) to the YAML. The nouns to name are
  the ones the capability actually touches. If it reads customer
  data, add `commerce.Customer`; if it writes orders, add
  `commerce.Order`.
- **"side_effect is X but writes: is empty"** — the writer must
  declare what it writes. The boundary rule and composition
  checker both depend on this.

After fixing: re-run `make ontology-lint` and the finding should
clear.

---

## Extending the lint

The lint classifier lives in `core/crates/aaf-server/src/lint.rs`.
To add a new finding:

1. Extend the `classify(cap)` function to emit a new
   `(Severity, String)` pair.
2. Add a unit test in the same file.
3. Re-run `cargo test -p aaf-server`.

Do *not* add new severity variants — the current four (`Ok`,
`Warn`, `Error`, and implicit parse/read errors) cover every
case shipped in iterations 1–8.

---

## Integration with CI

`make ci` wires `ontology-lint` into the PR gate after
`schema-validate`:

```makefile
ci: fmt-check clippy test schema-validate ontology-lint
```

Any PR that drops adoption below 90% without fixing the new
capability will pass in warn-only mode (the lint does not block)
but CI will surface the warning. Any PR that introduces a
write-class capability without `writes:` will *fail* CI once
adoption is at or above 90%.

---

## Further reading

- [adr/ADR-008-entity-space-boundaries.md](adr/ADR-008-entity-space-boundaries.md)
  — the "why entity space" decision
- [contracts.md](contracts.md) — what `reads` / `writes` / `emits`
  are and how the hot-path crates consume them
- `core/crates/aaf-server/src/lint.rs` — implementation
