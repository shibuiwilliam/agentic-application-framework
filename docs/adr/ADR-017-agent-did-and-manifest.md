# ADR-017 — Agent DID + Signed Manifest + SBOM + Delegated Capability Tokens

- **Status:** Accepted (Wave 2 X1, landed across iterations 6 / 9 / 10).
- **Date:** 2026-04-12
- **Supersedes:** none
- **Related:** ADR-015 (Feedback is a contract), ADR-014 (Ontology as first-class).

## Context

Wave 1 gave AAF a trust subsystem — behavioural scores, autonomy
levels, and `min(delegator, delegatee)` delegation propagation — but
did **not** address cryptographic identity. Every `agent_id` was
still a plain string. That is adequate for research and in-process
tests but impossible to deploy in any of the following settings:

- **Regulated industries** (finance, healthcare, defence) where every
  record must carry a cryptographic attestation of its author.
- **Multi-organisation federation** where no shared user directory
  can anchor trust.
- **Incident response** where an operator needs to revoke a compromised
  agent platform-wide within seconds, not hours.
- **Supply chain audit** where reviewers demand a bill of materials
  (model, prompts, tools, eval suites) with content hashes.

Shipping AAF into 2026 production without answering those questions
would block adoption in exactly the customers the framework was
built for.

## Decision

We add four orthogonal primitives, all in a new `aaf-identity` crate:

1. **`AgentDid`** — a W3C-style decentralised identifier of the form
   `did:aaf:<24-hex-thumbprint>`, deterministically derived from a
   keystore-minted verifying key. DIDs are opaque handles; the
   internal hex is never parsed outside the identity crate.

2. **`AgentManifest`** — a **signed** declaration of what the agent
   is: model pins, prompt hashes, tool bindings, ontology slices,
   capability allow-list, eval suite references. `ManifestBuilder::build`
   is the only constructor; it signs at the end and the resulting
   manifest is by construction a signed manifest.

3. **`AgentSbom`** — a hash-based bill of materials with 7 entry
   kinds (`Model / Prompt / Tool / Ontology / EvalSuite / TrainingData
   / Other`). Slice C adds SPDX 2.3 and CycloneDX 1.5 JSON exporters
   so third-party supply-chain tooling can ingest the document.

4. **`CapabilityToken`** — a short-lived signed bearer grant with
   issuer, subject, scope, delegation depth, validity window, and a
   unique `jti`. Replaces Wave 1's numeric `min(a, b)` integer on
   delegation with a cryptographically verifiable claim.

Two surface choices follow from these primitives:

- **`RevocationRegistry`** — an async trait + in-memory backend
  serving a signed revocation list keyed on `(RevocationKind, target)`
  pairs. The runtime consults it at a new `Hook 0: Revocation check`
  that runs before the trace is opened, so a revoked attempt leaves
  zero trace artefacts.

- **Token-chain verification in `aaf-trust`** — `verify_token(token,
  verifier, required)` composes `CapabilityToken::verify` with typed
  `DelegationError::Token` error mapping so the runtime can treat
  behavioural and cryptographic trust uniformly.

## Signature backend

Slice A ships a **deterministic HMAC-SHA256** keystore behind the
`Keystore` / `Signer` / `Verifier` traits. This is a *functional*
identity layer — every contract surface (sign, verify, manifest
tamper detection, token expiry, revocation short-circuit,
attestation level enforcement) is exercised end-to-end — but the
signing algorithm is a shared-secret MAC rather than real asymmetric
crypto.

**Rationale:** the `ed25519-dalek` versions that compile on the
workspace's pinned Rust 1.70 toolchain drag in `curve25519-dalek`
trees that have historically caused the workspace to fail to build.
Shipping a trait-backed MAC that keeps the tree green is strictly
better than shipping real Ed25519 that breaks the build. Because the
contract surface is backend-agnostic, swapping in Ed25519, a real
KMS, or an HSM in a later iteration does **not** touch any call site
outside the `InMemoryKeystore` implementation.

## Consequences

### Positive

- Every capability invocation has a cryptographic audit trail
  chained to a DID (Rule 22).
- Every deployed agent has a signed manifest that can be verified
  from its YAML source offline (Rule 23).
- Every agent ships a SPDX + CycloneDX SBOM ingestable by
  Dependency-Track, Grype, FOSSA, Snyk, … (Rule 24).
- Every artifact carries a DID-bound signature envelope
  (`x1:<did>:<sig>`) distinguishable from the legacy `v0:` envelope,
  so existing tests and legacy call sites continue to work
  unchanged (Rule 28).
- Cross-cell federation now requires **both** the issuing and
  receiving cell to sign a `CoSignedToken`, preventing a rogue
  intermediate cell from fabricating delegation authority.
- Revoking a compromised DID is a single call that propagates
  platform-wide via the runtime's pre-plan hook.

### Negative / trade-offs

- The Slice A signature backend is HMAC-SHA256, not real Ed25519.
  This is an explicit, documented trade-off; see "Signature backend"
  above.
- The in-memory revocation registry forgets revocations on process
  restart. Persistent backends land alongside the real storage
  driver work.
- The CLI uses a deterministic per-seed keystore so `verify` can
  work offline against a YAML manifest. Operators in production
  will use a persistent keystore (HSM / KMS / SPIFFE); the CLI's
  determinism is a demo / test convenience, not a production
  posture.

## Alternatives considered

- **Reuse Wave 1 `AgentId` strings.** Rejected: cannot survive any
  security review in a regulated industry.
- **Adopt full W3C DID Core.** Partially adopted (the `did:aaf:`
  method), but we explicitly do not implement universal DID
  resolution or the full controller / verification-method document
  layout. That is a future integration concern.
- **Defer to the OS keystore (macOS Keychain, Linux kernel keyring,
  Windows DPAPI).** Too platform-specific for a Rust workspace
  targeting containers.
- **Start with Ed25519.** Attempted; the dependency tree breaks the
  workspace build. Deferred to a future iteration where the
  toolchain can be upgraded alongside the crypto crate.

## Future work

- Replace `InMemoryKeystore`'s HMAC-SHA256 with Ed25519 behind the
  existing trait.
- Persistent revocation registry backed by Postgres / Redis.
- Universal DID resolver for `did:web`, `did:key`, `did:ion`.
- `aaf identity` CLI gains `import-manifest <json>`,
  `list-revocations`, `rotate-key`.
- SPDX 2.3 + CycloneDX 1.5 output validated against the official
  JSON Schemas in CI.
