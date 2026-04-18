#!/usr/bin/env python3
"""
Validate every YAML example in spec/examples/ against the corresponding
JSON Schema in spec/schemas/.

Called by `make schema-validate`. Can also be run directly:

    python3 scripts/schema_validate.py \
        --schema-dir spec/schemas --examples-dir spec/examples

Exits 0 if every example validates, 1 otherwise. The exit code is what
CI keys off.
"""
from __future__ import annotations

import argparse
import json
import pathlib
import sys

# Map example-file prefix → schema file. Explicit, so any unmapped
# example shows up as SKIP rather than being silently ignored by a
# fuzzy match.
#
# Keep this ordered most-specific to least-specific: the first prefix
# that matches wins.
PREFIX_TO_SCHEMA: dict[str, str] = {
    "sidecar-config":   "sidecar-config.schema.json",
    "wrapper-config":   "wrapper-config.schema.json",
    "cell-config":      "cell-config.schema.json",
    "fast-path":        "fast-path-rules.schema.json",
    "degradation":      "degradation-spec.schema.json",
    "capability-":      "capability-contract.schema.json",
    "policy-pack":      "policy-pack.schema.json",
    "saga-":            "saga-definition.schema.json",
    "intent-":          "intent-envelope.schema.json",
    # Enhancement X1 — Agent Identity
    "manifest-":        "agent-manifest.schema.json",
    "sbom-":            "agent-sbom.schema.json",
    # Enhancement E1 / E2 / E3
    "app-event":        "app-event.schema.json",
    "eval-suite":       "eval-suite.schema.json",
    "ontology-":        "entity.schema.json",
    "proposal-":        "action-proposal.schema.json",
    "state-projection": "state-projection.schema.json",
}


def schema_for(example_name: str) -> str | None:
    for prefix, schema in PREFIX_TO_SCHEMA.items():
        if example_name.startswith(prefix):
            return schema
    return None


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--schema-dir",   required=True, type=pathlib.Path)
    parser.add_argument("--examples-dir", required=True, type=pathlib.Path)
    args = parser.parse_args()

    try:
        import yaml  # noqa: F401
        from jsonschema import Draft202012Validator
        from referencing import Registry, Resource
        from referencing.jsonschema import DRAFT202012
    except ImportError as exc:
        print(f"schema-validate needs python3 with 'jsonschema' and 'pyyaml' ({exc}).")
        print("Install: pip install jsonschema pyyaml")
        return 1

    if not args.schema_dir.is_dir():
        print(f"schema directory not found: {args.schema_dir}")
        return 1
    if not args.examples_dir.is_dir():
        print(f"examples directory not found: {args.examples_dir}")
        return 1

    # Build a local Registry from every schema in spec/schemas/ so cross-schema
    # $ref lookups (e.g. sidecar-config → capability-contract) are resolved
    # offline. Schemas that declare an $id are keyed by it; every schema is
    # additionally keyed by its filename, so refs like "capability-contract.json"
    # also work.
    registry = Registry()
    schema_files = sorted(args.schema_dir.glob("*.schema.json"))
    for schema_path in schema_files:
        try:
            raw = json.loads(schema_path.read_text())
        except json.JSONDecodeError as exc:
            print(f"  FAIL loading {schema_path.name}: {exc}")
            return 1
        resource = Resource(contents=raw, specification=DRAFT202012)
        declared_id = raw.get("$id")
        if declared_id:
            registry = registry.with_resource(uri=declared_id, resource=resource)
        registry = registry.with_resource(uri=schema_path.name, resource=resource)
        registry = registry.with_resource(
            uri=schema_path.name.removesuffix(".schema.json") + ".json",
            resource=resource,
        )

    errors = 0
    checked = 0
    examples = sorted(
        list(args.examples_dir.glob("*.yaml"))
        + list(args.examples_dir.glob("*.yml"))
    )
    if not examples:
        print(f"no YAML examples found under {args.examples_dir}")
        return 0

    for example in examples:
        schema_name = schema_for(example.name)
        if schema_name is None:
            print(f"  SKIP {example.name} (no schema mapping)")
            continue

        schema_path = args.schema_dir / schema_name
        if not schema_path.exists():
            print(f"  FAIL {example.name}: schema {schema_name} not found")
            errors += 1
            continue

        try:
            schema = json.loads(schema_path.read_text())
            data = yaml.safe_load(example.read_text())
        except (json.JSONDecodeError, yaml.YAMLError) as exc:
            print(f"  FAIL {example.name}: parse error ({exc})")
            errors += 1
            continue

        validator = Draft202012Validator(schema, registry=registry)
        issues = sorted(
            validator.iter_errors(data),
            key=lambda e: list(e.absolute_path),
        )

        if issues:
            print(f"  FAIL {example.name} against {schema_name}")
            for issue in issues:
                path = "/".join(str(p) for p in issue.absolute_path) or "<root>"
                print(f"        {path}: {issue.message}")
            errors += 1
        else:
            print(f"  OK   {example.name} against {schema_name}")
        checked += 1

    print(f"\n{checked} example(s) checked, {errors} failure(s)")
    return 1 if errors else 0


if __name__ == "__main__":
    sys.exit(main())
