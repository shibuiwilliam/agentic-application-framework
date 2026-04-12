# signed-agent

End-to-end walkthrough of Wave 2 X1 Slice C (Agent Identity, Provenance
& Supply Chain). Demonstrates how an operator uses the `aaf identity`
CLI to produce a cryptographically verifiable agent manifest, export
its SBOM in SPDX and CycloneDX formats, verify the manifest, and
issue a signed revocation record — all against the in-memory HMAC-
SHA256 keystore that ships with Slice A (the trait-backed design swaps
in Ed25519 or a real KMS without changing any call site below).

## 1. Generate a DID

```bash
cargo run -p aaf-server -- identity generate-did signed-order-agent
```

Example output:

```
did:         did:aaf:7f2e8a1c3d4b5e6f
display:     signed-order-agent
backend:     in-memory HMAC-SHA256 (X1 Slice A/B)
```

The DID is deterministic per seed, so running the same command twice
produces the same id. That property makes the `verify` subcommand
below work offline without a keystore lookup.

## 2. Sign the manifest

```bash
cargo run -p aaf-server -- identity sign-manifest examples/signed-agent/manifest.yaml
```

Emits a pretty-printed JSON document of the signed
`AgentManifest` — DID, display name, source hash, model pin, tool
bindings, ontology slices, capability allow-list, eval suite
references, and the detached signature. Pipe it into
`jq '.signature'` to see just the signature.

## 3. Verify the manifest

```bash
cargo run -p aaf-server -- identity verify examples/signed-agent/manifest.yaml
```

Expected output:

```
ok  did:aaf:7f2e8a1c3d4b5e6f  signed-order-agent
```

The verifier re-derives the same keystore entry from the YAML's
`seed` field, rebuilds the manifest, and asks the identity layer
whether the signature matches. A tampered YAML — change the
`display_name`, for example — makes `verify` return a non-zero exit.

## 4. Export the SBOM

```bash
# SPDX 2.3 (default)
cargo run -p aaf-server -- identity export-sbom examples/signed-agent/sbom.yaml

# CycloneDX 1.5
cargo run -p aaf-server -- identity export-sbom examples/signed-agent/sbom.yaml --format cyclonedx
```

The SPDX output is a `SPDX-2.3` document with one `SPDXRef-Package`
per entry, each carrying a `SHA256` `checksumValue`. The CycloneDX
output is a `bomFormat: CycloneDX` / `specVersion: 1.5` document with
one `component` per entry, each carrying a `SHA-256` hash. Both
shapes are ingestable by any standards-based SBOM tool.

## 5. Issue a revocation

```bash
cargo run -p aaf-server -- identity revoke did:aaf:7f2e8a1c3d4b5e6f "compromised key"
```

Emits a pretty-printed JSON document of the signed `RevocationEntry`:
kind, target, reason, timestamp, signer DID, and detached signature.
In production this record is pushed into a persistent
`RevocationRegistry` so every subsequent runtime consults it at the
pre-plan hook (Wave 2 X1 Slice B, Rule 22).

## What this demonstrates

- **Rule 22** — Identity is cryptographic: the DID is derived from a
  keystore-minted key, not a nominal string.
- **Rule 23** — Every deployed agent has a signed manifest: `sign-manifest`
  is the only path from YAML to a valid signed document.
- **Rule 24** — Provenance is a bill of materials: the SBOM exports
  as SPDX and CycloneDX so third-party supply-chain tools (Dependency-
  Track, Grype, FOSSA, Snyk SBOM Import, …) can ingest it.
- **Rule 28** — The SDK emits signed artifacts by default: every
  artifact the runtime produces for this agent carries a
  `x1:<did>:<sig>` envelope verified against the same keystore.
