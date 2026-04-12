# X1 — Agent Identity, Provenance & Supply Chain

> **Status:** Complete (Slices A/B/C landed, iterations 6/9/10).
> **Related spec:** `PROJECT.md` §17.1, `CLAUDE.md` rules 22-24,
> ADR-017.

## Why

AAF before Wave 2 treated an "agent" as a string. That is adequate
for a research framework but impossible to deploy in any regulated
industry, any multi-org federation, or anywhere a security reviewer
asks "prove that the agent that wrote this record is the one you
claim it is, and prove it is running the code you think it is
running". X1 answers those questions.

## What landed

### Slice A — Contracts + keystore (iteration 6)

**New crate `aaf-identity`** (33 unit tests):

| Module | Contents |
|---|---|
| `did.rs` | `AgentDid` — W3C DID-style `did:aaf:<24-hex>` thumbprint derived from a verifying key. `is_well_formed()` guard. |
| `keystore.rs` | `Keystore` / `Signer` / `Verifier` traits + `InMemoryKeystore` backed by deterministic HMAC-SHA256. Constant-time signature comparison. Trait-backed so an Ed25519 / KMS / HSM backend swaps in without touching any call site. |
| `manifest.rs` | `AgentManifest` + `ManifestBuilder`. **`build()` is the only constructor** — it signs at the end, so any manifest in memory is by construction a signed manifest. Tamper-detection + source-drift check. |
| `sbom.rs` | `AgentSbom` with 7-kind classification (`Model / Prompt / Tool / Ontology / EvalSuite / TrainingData / Other`). Stable content hashes. |
| `attestation.rs` | `AttestationLevel` lattice (`Unattested < AutoVerified < HumanReviewed < Certified`) with monotonic `grants()`. Signed `Attestation` records. |
| `delegation.rs` | `CapabilityToken` + `TokenClaims` (issuer, subject, scope, depth, validity window, jti). `verify` enforces signature + window + scope. `step_down` decrements delegation depth. |
| `revocation.rs` | `RevocationEntry` with 5 kinds (DID, prompt hash, tool version, manifest hash, token jti) signed by the revoker. `RevocationRegistry` async trait + in-memory backend. |

**Contract extensions in `aaf-contracts`:**

- `AgentDidRef` / `AttestationLevelRef` / `TokenClaimsLite` /
  `CapabilityTokenLite` wire shapes under `aaf_contracts::identity`.
- `CapabilityContract.required_attestation_level:
  Option<AttestationLevelRef>` (optional).
- `Handoff.capability_token: Option<CapabilityTokenLite>` (optional).

**Schemas + examples:** `agent-manifest`, `agent-sbom`,
`capability-token`, `revocation-list` JSON Schemas under
`spec/schemas/`; `manifest-order-agent.yaml`,
`sbom-order-agent.yaml` examples under `spec/examples/`.

### Slice B — Hot-path integration (iteration 9)

**Runtime revocation gate.** `GraphExecutor::with_revocation(Arc<dyn
RevocationRegistry>)` attaches an optional registry consulted at a
brand-new `Hook 0: Revocation check` that runs **before the trace
is opened**. Revoked DIDs fail with `RuntimeError::Revoked` and leave
zero trace artefacts. Non-DID requesters (Wave 1 call sites) bypass
the gate entirely for backward compatibility — asserted by test.

**Trust token-chain verification.** New
`aaf_trust::delegation::verify_token(token, verifier, required)`
helper composes `CapabilityToken::verify` with typed
`DelegationError::Token(IdentityError)` mapping so the runtime can
treat behavioural and cryptographic trust uniformly.

**Registry attestation gate.** New
`Registry::get_for_attestation(id, presented)` method enforces
`CapabilityContract.required_attestation_level`, returning
`RegistryError::InsufficientAttestation { required, presented }`
when the caller's presented level is too weak. Capabilities without
a declared level are served unconditionally (Wave 1 compat).

**DID-bound artifact signing.** New
`aaf_trust::sign_artifact_with(&mut Artifact, &AgentDid, &dyn Signer)`
produces `x1:<did>:<sig>` envelopes;
`verify_artifact_with(&Artifact, &dyn Verifier)` parses and verifies
them. Legacy `sign_artifact` / `verify_artifact` are kept for
backward compatibility and cleanly distinguishable via the `v0:` vs
`x1:` envelope prefix.

### Slice C — Tooling, federation, examples (iteration 10)

**SPDX + CycloneDX SBOM exporter.**
`aaf_identity::sbom::export::{to_spdx_json, to_cyclonedx_json}`
produce standards-compatible JSON documents. Every AAF `SbomEntry`
maps to one `SPDXRef-Package` (with `SHA256` checksum) or one
CycloneDX `component` (with `SHA-256` hash). 6 new tests cover the
round-trip for both formats and the kind → type mappings.

**Cross-cell co-signed tokens.** New `aaf_federation::cosign` module
with `CoSignedToken` wrapping an `aaf_identity::CapabilityToken` plus
both cells' signatures over a canonical hash. `verify_cosigned`
enforces **all three** signatures: issuer cell, receiver cell, inner
token. 6 new tests cover round-trip, tamper detection (issuer sig,
receiver sig, inner token), out-of-scope, and expired-token cases.

**`aaf identity` CLI subcommand** in `aaf-server`:

```text
aaf-server identity generate-did  [seed]
aaf-server identity sign-manifest <manifest.yaml>
aaf-server identity verify        <manifest.yaml>
aaf-server identity export-sbom   <sbom.yaml> [--format spdx|cyclonedx]
aaf-server identity revoke        <did> <reason>
```

Every op consumes / emits JSON or plain text on stdout so operators
can pipe, inspect, and diff. The CLI never writes to the filesystem.

**`examples/signed-agent/` reference.** `manifest.yaml` + `sbom.yaml`
+ `README.md` walking through all 5 CLI operations.

**Integration test.**
`core/tests/integration/tests/x1_slice_c_cli.rs` pins the CLI code
paths against the `aaf-identity` + `aaf-federation` public APIs
(generate, sign, verify, SBOM export, revoke, co-sign).

## Rules enforced

| Rule | Enforcement |
|---|---|
| **22** Identity is cryptographic, not nominal | `AgentDid::from_verifying_key`, `is_well_formed` guard, manifest refuses bare nominal DIDs. |
| **23** Every deployed agent has a signed manifest | `ManifestBuilder::build` is the only constructor; tampering invalidates the signature. |
| **24** Provenance is a bill of materials | `AgentSbom` with content hashes; SPDX + CycloneDX exporters for third-party audit tools. |
| **28** The SDK emits signed artifacts by default | `sign_artifact_with` is the production-path signer; verified via `verify_artifact_with`. |

## What's out of scope

- **Ed25519 / KMS / HSM backends.** Slice A/B/C ship the contract
  surface with a deterministic HMAC-SHA256 backend. Replacing the
  backend does not touch any call site because everything goes
  through the `Keystore` / `Signer` / `Verifier` traits.
- **Persistent revocation backends.** The in-memory registry is the
  trait-backed reference implementation; a Postgres / Redis backend
  is a follow-up to land alongside the real storage driver work.
- **Full W3C DID document resolution.** AAF's DIDs are opaque
  thumbprints; DID method registration and universal resolver
  support are future concerns.

## See also

- `PROJECT.md` §17.1 — design rationale
- `CLAUDE.md` rules 22-24 — architecture rules
- `docs/adr/ADR-017-agent-did-and-manifest.md` — decision record
- `examples/signed-agent/README.md` — end-to-end walkthrough
