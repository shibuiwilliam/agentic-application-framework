# sidecar-gateway

Demonstrates AAF's **Agent Sidecar** — the service-integration
pattern for microservices (PROJECT.md §19.2.1). The sidecar sits
alongside an existing service, intercepts traffic, and adds AAF
capabilities without modifying the service itself (Rule 3).

## Scenario

A CRM service exposes customer lookup and order history APIs. The
sidecar sits in front of it and provides:

```text
  [Client Request]
        |
  [Sidecar Proxy] ── health check ──> [CRM Service]
        |
        +── Fast-path match? ── YES ──> invoke capability locally
        |
        +── No match ──> forward to AAF control plane
        |
        +── AAF unhealthy? ──> direct forward (Rule 13)
```

## What it exercises

- **Proxy routing** (3 paths): FastPath, ForwardToAaf, DirectForward
- **Rule 13 transparent fallback**: when the sidecar detects AAF is
  unhealthy, it bypasses AAF and forwards requests directly to the
  service — system degrades to "no AAF" not "broken"
- **Sidecar-local fast-path**: fast-path rules evaluated locally
  without round-trip to control plane (Rule 4)
- **Anti-Corruption Layer (ACL)**: entity translation between AAF's
  unified semantic model (`commerce.Customer`) and the CRM's internal
  model (`Account`) — prevents vocabulary leakage in both directions
- **Local guards**: input/output guards running at the sidecar layer
  for injection detection and PII scanning
- **Health monitoring**: mutable health state that drives the Rule 13
  fallback
- **Capability publishing**: registering service capabilities into
  the AAF registry on startup
- **Field mapping**: translating intent constraints to API parameters

## Files

- `aaf.yaml` — sidecar config with capabilities, fast-path rules,
  and ACL entity translation mappings

## Run it

```bash
# Run the integration test (exercises all sidecar paths)
cargo test -p aaf-integration-tests --test sidecar_gateway_e2e

# Validate the config
cargo run -p aaf-server -- validate examples/sidecar-gateway/aaf.yaml
```
