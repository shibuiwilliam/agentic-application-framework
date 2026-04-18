# AAF — developer Makefile
#
# Run `make help` for a grouped list of every target.
# Conventions:
#   - default goal prints help
#   - `make ci` is what every PR must pass
#   - `make all` is a fast local loop (check + test)
#
# This Makefile intentionally keeps every recipe to one command per line so
# it works on stock macOS make (GNU Make 3.81, which has no .ONESHELL).
# Anything non-trivial (e.g. schema validation) lives in scripts/.

CARGO             ?= cargo
CARGO_FLAGS       ?=
CLIPPY_FLAGS      ?= -D warnings -W clippy::pedantic \
                     -A clippy::module_name_repetitions \
                     -A clippy::must_use_candidate \
                     -A clippy::return_self_not_must_use \
                     -A clippy::missing_errors_doc \
                     -A clippy::missing_panics_doc \
                     -A clippy::doc_markdown \
                     -A clippy::match_same_arms \
                     -A clippy::redundant_closure_for_method_calls \
                     -A clippy::similar_names \
                     -A clippy::cast_lossless \
                     -A clippy::cast_possible_truncation \
                     -A clippy::cast_precision_loss \
                     -A clippy::needless_pass_by_value \
                     -A clippy::default_trait_access \
                     -A clippy::uninlined_format_args \
                     -A clippy::items_after_statements \
                     -A clippy::unnecessary_wraps \
                     -A clippy::too_many_lines \
                     -A clippy::single_match_else \
                     -A clippy::manual_let_else \
                     -A clippy::match_wildcard_for_single_variants \
                     -A clippy::case_sensitive_file_extension_comparisons \
                     -A clippy::float_cmp
SCHEMA_DIR        ?= spec/schemas
EXAMPLES_DIR      ?= spec/examples
PYTHON            ?= python3
SCHEMA_VALIDATOR  ?= scripts/schema_validate.py

.DEFAULT_GOAL := help

.PHONY: help all ci \
        build build-release check \
        test test-doc test-quiet \
        fmt fmt-check \
        clippy clippy-fix lint \
        doc doc-open \
        bench \
        watch watch-test \
        clean clean-all \
        tree outdated audit update deny \
        schema-validate schema-list ontology-lint \
        version bump-major bump-minor bump-patch \
        todo loc \
        hooks hooks-pre-commit hooks-run \
        install-tools

## ── Help ──────────────────────────────────────────────────────────────
help: ## Show this help (grouped by section)
	@awk 'BEGIN {FS = ":.*?## "} \
		/^## ── / {print "\n\033[1;34m" substr($$0, 5) "\033[0m"; next} \
		/^[a-zA-Z0-9_-]+:.*?## / {printf "  \033[36m%-20s\033[0m %s\n", $$1, $$2}' \
		$(MAKEFILE_LIST)

## ── Common loops ──────────────────────────────────────────────────────
all: check test ## Fast local loop: cargo check + cargo test

ci: fmt-check clippy test schema-validate ontology-lint ## Full PR gate: fmt-check + clippy + test + schema-validate + ontology-lint

## ── Build ─────────────────────────────────────────────────────────────
build: ## Debug build of the whole workspace
	$(CARGO) build --workspace $(CARGO_FLAGS)

build-release: ## Release build of the whole workspace
	$(CARGO) build --workspace --release $(CARGO_FLAGS)

check: ## cargo check every crate and every target (tests, examples, benches)
	$(CARGO) check --workspace --all-targets $(CARGO_FLAGS)

## ── Test ──────────────────────────────────────────────────────────────
test: ## Run every unit + integration test in the workspace
	$(CARGO) test --workspace $(CARGO_FLAGS)

test-doc: ## Run doc-tests only
	$(CARGO) test --workspace --doc $(CARGO_FLAGS)

test-quiet: ## Run tests with minimal output (useful in CI logs)
	$(CARGO) test --workspace --quiet $(CARGO_FLAGS)

## ── Format and lint ───────────────────────────────────────────────────
fmt: ## Format every crate with rustfmt
	$(CARGO) fmt --all

fmt-check: ## Verify formatting without modifying files
	$(CARGO) fmt --all -- --check

clippy: ## Strict clippy (pedantic, warnings are errors)
	$(CARGO) clippy --workspace --all-targets -- $(CLIPPY_FLAGS)

clippy-fix: ## Apply clippy's machine-applicable fixes
	$(CARGO) clippy --workspace --all-targets --fix --allow-dirty --allow-staged -- $(CLIPPY_FLAGS)

lint: fmt-check clippy ## fmt-check + clippy

## ── Documentation ─────────────────────────────────────────────────────
doc: ## Build rustdoc for every crate (no deps)
	$(CARGO) doc --workspace --no-deps

doc-open: ## Build rustdoc and open it in the browser
	$(CARGO) doc --workspace --no-deps --open

## ── Benchmarks ────────────────────────────────────────────────────────
bench: ## Run cargo bench across the workspace
	$(CARGO) bench --workspace

## ── Watch mode (requires cargo-watch) ─────────────────────────────────
watch: ## Re-run `cargo check` on every file change
	@command -v cargo-watch >/dev/null 2>&1 || { \
		echo "cargo-watch is not installed. Run: make install-tools"; exit 1; }
	$(CARGO) watch -x "check --workspace --all-targets"

watch-test: ## Re-run tests on every file change
	@command -v cargo-watch >/dev/null 2>&1 || { \
		echo "cargo-watch is not installed. Run: make install-tools"; exit 1; }
	$(CARGO) watch -x "test --workspace"

## ── Clean ─────────────────────────────────────────────────────────────
clean: ## cargo clean
	$(CARGO) clean

clean-all: clean ## cargo clean + remove local caches and generated docs
	rm -rf target/doc target/criterion

## ── Dependency hygiene ────────────────────────────────────────────────
tree: ## Show the dependency tree for the workspace
	$(CARGO) tree --workspace

outdated: ## Report outdated dependencies (requires cargo-outdated)
	@command -v cargo-outdated >/dev/null 2>&1 || { \
		echo "cargo-outdated is not installed. Run: make install-tools"; exit 1; }
	$(CARGO) outdated --workspace --root-deps-only

audit: ## Audit dependencies for known vulnerabilities (requires cargo-audit)
	@command -v cargo-audit >/dev/null 2>&1 || { \
		echo "cargo-audit is not installed. Run: make install-tools"; exit 1; }
	$(CARGO) audit

deny: ## Run cargo-deny checks (licenses, bans, advisories)
	@command -v cargo-deny >/dev/null 2>&1 || { \
		echo "cargo-deny is not installed. Run: make install-tools"; exit 1; }
	$(CARGO) deny check

update: ## Update Cargo.lock to the latest allowed versions
	$(CARGO) update

## ── Spec / schema ─────────────────────────────────────────────────────
schema-list: ## List every JSON Schema and every example in spec/
	@echo "Schemas:"
	@ls -1 $(SCHEMA_DIR) 2>/dev/null | sed 's/^/  /' || echo "  (none)"
	@echo "Examples:"
	@ls -1 $(EXAMPLES_DIR) 2>/dev/null | sed 's/^/  /' || echo "  (none)"

schema-validate: ## Validate every YAML example in spec/examples/ against its JSON Schema
	$(PYTHON) $(SCHEMA_VALIDATOR) --schema-dir $(SCHEMA_DIR) --examples-dir $(EXAMPLES_DIR)

ontology-lint: build ## Lint capability YAMLs for entity declarations (E2 Slice C)
	target/debug/aaf-server ontology lint $(EXAMPLES_DIR)

## ── Version management ───────────────────────────────────────────────
version: ## Print the current project version
	@cat VERSION

bump-major: ## Bump major version (x.0.0) — breaking changes
	@./scripts/bump-version.sh major

bump-minor: ## Bump minor version (0.x.0) — new features
	@./scripts/bump-version.sh minor

bump-patch: ## Bump patch version (0.0.x) — bug fixes
	@./scripts/bump-version.sh patch

## ── Repo inspection ───────────────────────────────────────────────────
todo: ## Grep the workspace for TODO / FIXME / XXX markers
	@grep -rn --include='*.rs' -E "TODO|FIXME|XXX" core/ || echo "No markers found."

loc: ## Rough line count per crate (Rust sources only)
	@find core/crates -name '*.rs' -not -path '*/target/*' \
		| xargs wc -l | sort -n

## ── Git hooks ────────────────────────────────────────────────────────
hooks: ## Install git hooks (pre-commit: fmt+clippy, pre-push: full CI)
	git config core.hooksPath .githooks
	@echo "Git hooks installed from .githooks/"

hooks-pre-commit: ## Install pre-commit framework hooks (alternative to 'make hooks')
	@command -v pre-commit >/dev/null 2>&1 || { \
		echo "pre-commit is not installed. Run: pip install pre-commit"; exit 1; }
	@git config --unset-all core.hooksPath 2>/dev/null || true
	pre-commit install
	pre-commit install --hook-type pre-push
	@echo "pre-commit hooks installed"

hooks-run: ## Run all pre-commit hooks against all files
	@command -v pre-commit >/dev/null 2>&1 || { \
		echo "pre-commit is not installed. Run: pip install pre-commit"; exit 1; }
	pre-commit run --all-files

## ── Tooling bootstrap ─────────────────────────────────────────────────
install-tools: ## Install the cargo-* helpers used by this Makefile
	$(CARGO) install --locked cargo-watch    || true
	$(CARGO) install --locked cargo-outdated || true
	$(CARGO) install --locked cargo-audit    || true
	$(CARGO) install --locked cargo-deny     || true
