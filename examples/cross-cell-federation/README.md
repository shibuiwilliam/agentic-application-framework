# cross-cell-federation

Demonstrates AAF's third service-integration pattern — **Cell
Architecture + Federation** — where independent cells (self-contained
service groups in different regions) communicate through a federation
layer with typed data boundaries and co-signed capability tokens.

This is the only example that exercises `aaf-federation`: cell routing,
data boundary enforcement, co-signed tokens, and federation agreement
parsing.

## What this example covers

| Feature | Where it's exercised |
|---|---|
| **Cell routing** | `Router::route` resolves a capability to its owning cell (Japan or US) |
| **Data boundary enforcement** | Payloads with prohibited PII fields (`pii_email`, `pii_phone`) are blocked from crossing cells |
| **Clean payload crossing** | Payloads without prohibited fields pass the boundary check |
| **No-agreement rejection** | Cross-cell calls to cells without a federation agreement fail cleanly |
| **Co-signed capability tokens** | Tokens carry signatures from both issuing and receiving cells; tampering is detected |
| **Scope check on co-signed tokens** | Tokens are verified against the declared capability scope |
| **Federation YAML parsing** | `federation.yaml` loads and its structure is validated |

## Files

```
examples/cross-cell-federation/
├── README.md         ← this file
├── aaf.yaml          ← capability seeds from both cells
└── federation.yaml   ← cell configs + bilateral agreement + prohibited fields
```

## Run the tests

```bash
cargo test -p aaf-integration-tests --test cross_cell_federation_e2e
```

Expected output:

```text
running 7 tests
test cell_routing_resolves_capability_to_owning_cell ... ok
test data_boundary_blocks_pii_fields ... ok
test clean_payload_crosses_boundary_successfully ... ok
test no_agreement_blocks_cross_cell_communication ... ok
test cosigned_token_requires_both_cell_signatures ... ok
test cosigned_token_rejects_out_of_scope_capability ... ok
test federation_yaml_parses_successfully ... ok

test result: ok. 7 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

You can also run the basic pipeline using the federation capabilities:

```bash
cargo run -p aaf-server -- run examples/cross-cell-federation/aaf.yaml
```

## What each test proves

### `cell_routing_resolves_capability_to_owning_cell`
`cap-jp-orders` routes to `cell-japan`; `cap-us-orders` routes to
`cell-us`; unknown capabilities return `None`.

### `data_boundary_blocks_pii_fields`
A payload `{"order_id": "ord-42", "pii_email": "tanaka@example.com"}`
is rejected with `FederationError::BoundaryViolation("pii_email")`.

### `clean_payload_crosses_boundary_successfully`
A payload `{"order_id": "ord-42", "total": 1280, "region": "tokyo"}`
passes the boundary check (no prohibited fields).

### `no_agreement_blocks_cross_cell_communication`
Routing from `cell-japan` to `cell-mars` (no agreement) fails with
`FederationError::NoAgreement`.

### `cosigned_token_requires_both_cell_signatures`
A token co-signed by both cells verifies successfully. Tampering with
the issuer-cell signature is detected and rejected.

### `cosigned_token_rejects_out_of_scope_capability`
A co-signed token for `cap-jp-orders` is rejected when verified
against `cap-us-inventory` (not in scope).

### `federation_yaml_parses_successfully`
`federation.yaml` loads and contains 2 cells, 1 agreement, and 2
prohibited fields.

## Architecture rules exercised

| Rule | How |
|---|---|
| **Rule 3** (Services stay untouched) | Federation sits above services; no service code changes |
| **Rule 13** (Sidecar transparent fallback) | Each cell's sidecar can forward directly if AAF is down |
| **Rule 22** (Identity is cryptographic) | Co-signed tokens carry DIDs from both cells |
| **Security checklist: Cross-cell data boundary enforced** | `enforce_outbound` checks every payload against prohibited fields |

## See also

- `PROJECT.md` §Cell Architecture
- `PROJECT.md` §19.2 Integration with Cell Architecture
- `PROJECT.md` §19.8 Security and Governance (cross-cell)
- `examples/signed-agent/` — identity + provenance (single-cell)
- `examples/order-saga/` — saga compensation (single-cell)
