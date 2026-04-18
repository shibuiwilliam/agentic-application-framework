# Build & CI

> The five gates that must stay green on every change, and the exact
> commands that run them.

---

## The five gates

| Gate | Command | Passes when |
|---|---|---|
| 1. Build | `cargo build --workspace` | Exit 0, zero warnings |
| 2. Test | `cargo test --workspace` | Exit 0, zero failures (currently 554 passing) |
| 3. Clippy (lax) | `cargo clippy --workspace --all-targets -- -W clippy::all` | Zero warnings |
| 4. Schemas | `python3 scripts/schema_validate.py --schema-dir spec/schemas --examples-dir spec/examples` | `0 failures` (currently 9/9 validating, 2 skipped) |
| 5. Ontology lint | `make ontology-lint` (or `target/debug/aaf-server ontology lint spec/examples`) | `0 errors` |

**Every PR must pass all five.** Gates 1–3 are implicit in
`make ci`; gates 4 and 5 are wired into `make ci` as explicit steps.

### Running them all at once

```bash
make ci
```

The `ci` Makefile target runs `fmt-check → clippy → test →
schema-validate → ontology-lint` in sequence. Fastest path to a
green check mark.

### A faster inner loop

```bash
make all        # cargo check --workspace + cargo test --workspace
```

`make all` is the lightweight local loop: no clippy, no formatting,
no schemas. Use it while you are iterating on a change; run
`make ci` before you push.

---

## Why `-W clippy::all` and not `clippy::pedantic`

The Makefile's `clippy` target runs with
`-D warnings -W clippy::pedantic -A clippy::module_name_repetitions`.
That target **is not green** and has never been green in the lifetime
of this tree — pedantic catches dozens of pre-existing minor issues
across crates that would need a dedicated cleanup iteration.

For iteration-level gates, the canonical command is
`cargo clippy --workspace --all-targets -- -W clippy::all`. Every
PR should keep *that* at zero. If you want to be extra rigorous,
run the Makefile's pedantic target and clean up any warnings in
files you are already touching — that is the gradient descent that
will eventually make `make clippy` green.

---

## Makefile targets (full list)

Run `make help` for the grouped, colour-highlighted view.
The targets you will actually type:

| Target | Description |
|---|---|
| `make help` | Print the full grouped list of targets |
| `make all` | `check + test` — the fast inner loop |
| `make ci` | `fmt-check + clippy + test + schema-validate + ontology-lint` — the PR gate |
| `make build` | Debug build of the whole workspace |
| `make build-release` | Release build with LTO, codegen-units=1 |
| `make check` | `cargo check --workspace --all-targets` |
| `make test` | `cargo test --workspace` |
| `make test-doc` | Doc-tests only |
| `make test-quiet` | Test with minimal output (useful in CI logs) |
| `make fmt` | Format every crate with rustfmt |
| `make fmt-check` | Verify formatting without writing |
| `make clippy` | `-D warnings -W clippy::pedantic` (strict — known to have pre-existing issues) |
| `make clippy-fix` | Apply machine-applicable clippy fixes |
| `make lint` | `fmt-check + clippy` |
| `make doc` / `make doc-open` | Build rustdoc |
| `make bench` | Run cargo bench across the workspace |
| `make watch` / `make watch-test` | cargo-watch loops (requires `cargo-watch`) |
| `make clean` / `make clean-all` | Clean target/ and caches |
| `make tree` | Print the workspace dep tree |
| `make outdated` | Report outdated deps (requires `cargo-outdated`) |
| `make audit` | Run `cargo-audit` (requires install) |
| `make deny` | Run `cargo-deny` (requires install) |
| `make schema-list` | List schemas and examples |
| `make schema-validate` | Validate `spec/examples/` against `spec/schemas/` |
| `make ontology-lint` | Run the ontology lint on `spec/examples/` |
| `make todo` | Grep the workspace for TODO/FIXME/XXX markers |
| `make loc` | Line count per crate |
| `make install-tools` | Install the optional cargo-* helpers |

---

## Build profile notes

From `Cargo.toml` workspace root:

```toml
[profile.release]
lto = "thin"
codegen-units = 1
opt-level = 3
```

The workspace targets **Edition 2021** and **rust-version 1.70**.
`PROJECT.md` calls for Edition 2024; that migration is deferred
until the deployment toolchain catches up. Do not silently bump
the edition in a slice — it is an explicit tree-wide migration and
should be its own iteration with its own entry in
`IMPLEMENTATION_PLAN.md`.

---

## Pinned dependency versions

Every third-party dep is **pinned** to a Rust-1.70-compatible
release in the `[workspace.dependencies]` section. If you need a
newer version of a crate, check whether the pin is what's blocking
you; loosening a pin is fine but must be justified (and re-build
the whole workspace to confirm nothing regresses).

Canonical pinned versions (as of iteration 8):

```
serde          = 1.0.164
serde_json     = 1.0.96
serde_yaml     = 0.9.21
thiserror      = 1.0.40
async-trait    = 0.1.74
tokio          = 1.28.2 (macros, rt-multi-thread, sync, time)
uuid           = 1.3.4
chrono         = 0.4.26
parking_lot    = 0.12.1
once_cell      = 1.17.1
regex          = 1.8.4
sha2           = 0.10.6
hex            = 0.4.3
rand           = 0.8.5
```

`aaf-identity` deliberately avoids `ed25519-dalek` / `curve25519`
family deps because they have historically broken the tree on
Rust 1.70. The HMAC-SHA256-backed signer is the shipping primitive;
Ed25519 is an X1 Slice C deliverable.

---

## Running `aaf-server` locally

```bash
cargo run -p aaf-server                                      # defaults to `run ./aaf.yaml`
cargo run -p aaf-server -- run examples/hello-agent/aaf.yaml # explicit path
cargo run -p aaf-server -- validate aaf.yaml                 # parse + report
cargo run -p aaf-server -- discover monthly sales            # registry lexical search
cargo run -p aaf-server -- compile "show last month sales"   # NL → envelope JSON
cargo run -p aaf-server -- ontology lint spec/examples       # E2 Slice C
cargo run -p aaf-server -- ontology import my-openapi.yaml   # E2 Slice C (stdout)
cargo run -p aaf-server -- help                              # show all subcommands
```

All subcommands use in-memory backends by default. Real storage
drivers are deferred.

---

## CI-facing invariants

Any future automation (GitHub Actions, GitLab CI, Jenkins) should
treat `make ci` as the canonical pre-merge gate. A minimal CI file:

```yaml
jobs:
  gate:
    runs-on: ubuntu-latest
    steps:
      - checkout
      - rust-setup:
          toolchain: 1.70
      - run: python3 -m pip install jsonschema pyyaml referencing
      - run: make ci
```

Nothing else is required; `make ci` internally runs every gate
listed in the table at the top of this file.

---

## Exit codes

Every subcommand and every gate communicates success via a standard
Unix exit code. Relevant non-zero exits:

| Tool | Non-zero exit means |
|---|---|
| `cargo build` | Compilation error |
| `cargo test` | At least one test failed |
| `cargo clippy -- -W clippy::all` | At least one warning |
| `scripts/schema_validate.py` | At least one example does not validate |
| `aaf-server ontology lint` | At least one `Severity::Error` finding (strict mode) |
| `aaf-server ontology import` | Input is not a parseable OpenAPI document |

CI should abort on any non-zero exit.
