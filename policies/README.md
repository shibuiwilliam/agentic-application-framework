# Policy Packs

This directory holds the policy packs that ship with AAF.

## `base/`

The seven base policies enforced by `aaf-policy::PolicyEngine::with_default_rules()`:

| File | Rule kind | Severity | Hook(s) |
|---|---|---|---|
| `scope-check.yaml` | `scope_check` | error | PreStep |
| `side-effect-gate.yaml` | `side_effect_gate` | warning (→ approval) | PreStep |
| `budget-control.yaml` | `budget_control` | warning / error | every hook |
| `pii-guard.yaml` | `pii_guard` | error | PostStep |
| `injection-guard.yaml` | `injection_guard` | error | PreStep |
| `composition-safety.yaml` | `composition_safety` | warning | PreStep |
| `boundary-enforcement.yaml` | `boundary_enforcement` | fatal | every hook |

The YAML files in this directory are documentation and a future
loader target. The runtime currently constructs the same rules in
code via `PolicyEngine::with_default_rules()`. A `PolicyEngine::from_pack`
loader will be added in a future iteration to consume these files
verbatim — the schema is already defined at
`spec/schemas/policy-pack.schema.json`.

## Industry packs (planned)

- `finance/` — finance-specific overlays (PCI, SOX)
- `healthcare/` — healthcare-specific overlays (HIPAA)
