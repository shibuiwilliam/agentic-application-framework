#!/usr/bin/env bash
# bump-version.sh — Semantic version management for AAF
#
# Usage:
#   ./scripts/bump-version.sh major   # 0.2.0 → 1.0.0
#   ./scripts/bump-version.sh minor   # 0.2.0 → 0.3.0
#   ./scripts/bump-version.sh patch   # 0.2.0 → 0.2.1
#   ./scripts/bump-version.sh set 1.0.0  # set explicit version
#   ./scripts/bump-version.sh current # print current version
#
# The VERSION file at the repo root is the single source of truth.
# This script updates VERSION and then propagates the new version to:
#   - Cargo.toml (workspace.package.version + all internal crate deps)
#   - aaf.yaml (project.version)
#   - examples/*/aaf.yaml (project.version)
#   - spec/examples/*sbom*.yaml (version field in entries)

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
VERSION_FILE="$REPO_ROOT/VERSION"

if [ ! -f "$VERSION_FILE" ]; then
    echo "ERROR: VERSION file not found at $VERSION_FILE" >&2
    exit 1
fi

CURRENT_VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"

# Validate semver format
validate_semver() {
    local ver="$1"
    if ! echo "$ver" | grep -qE '^[0-9]+\.[0-9]+\.[0-9]+$'; then
        echo "ERROR: '$ver' is not a valid semver (expected MAJOR.MINOR.PATCH)" >&2
        exit 1
    fi
}

validate_semver "$CURRENT_VERSION"

# Parse current version
IFS='.' read -r MAJOR MINOR PATCH <<< "$CURRENT_VERSION"

# Determine action
ACTION="${1:-current}"

case "$ACTION" in
    current)
        echo "$CURRENT_VERSION"
        exit 0
        ;;
    major)
        MAJOR=$((MAJOR + 1))
        MINOR=0
        PATCH=0
        ;;
    minor)
        MINOR=$((MINOR + 1))
        PATCH=0
        ;;
    patch)
        PATCH=$((PATCH + 1))
        ;;
    set)
        if [ -z "${2:-}" ]; then
            echo "Usage: $0 set <version>" >&2
            exit 1
        fi
        validate_semver "$2"
        IFS='.' read -r MAJOR MINOR PATCH <<< "$2"
        ;;
    *)
        echo "Usage: $0 {major|minor|patch|set <version>|current}" >&2
        exit 1
        ;;
esac

NEW_VERSION="$MAJOR.$MINOR.$PATCH"

if [ "$NEW_VERSION" = "$CURRENT_VERSION" ]; then
    echo "Version is already $CURRENT_VERSION"
    exit 0
fi

echo "Bumping version: $CURRENT_VERSION → $NEW_VERSION"

# 1. Update VERSION file
echo "$NEW_VERSION" > "$VERSION_FILE"

# 2. Update workspace Cargo.toml — workspace.package.version
sed -i.bak "s/^version = \"$CURRENT_VERSION\"/version = \"$NEW_VERSION\"/" "$REPO_ROOT/Cargo.toml"

# 3. Update workspace Cargo.toml — all internal crate dependency versions
sed -i.bak "s/version = \"$CURRENT_VERSION\" }/version = \"$NEW_VERSION\" }/" "$REPO_ROOT/Cargo.toml"

# 4. Update root aaf.yaml
if [ -f "$REPO_ROOT/aaf.yaml" ]; then
    sed -i.bak "s/version: $CURRENT_VERSION/version: $NEW_VERSION/" "$REPO_ROOT/aaf.yaml"
fi

# 5. Update all example aaf.yaml files
find "$REPO_ROOT/examples" -name 'aaf.yaml' -exec \
    sed -i.bak "s/version: $CURRENT_VERSION/version: $NEW_VERSION/" {} \;

# 6. Update spec/examples SBOM and policy YAML files
find "$REPO_ROOT/spec/examples" -name '*.yaml' -exec \
    sed -i.bak "s/version: \"$CURRENT_VERSION\"/version: \"$NEW_VERSION\"/" {} \;
find "$REPO_ROOT/spec/examples" -name '*.yaml' -exec \
    sed -i.bak "s/^version: $CURRENT_VERSION/version: $NEW_VERSION/" {} \;

# 7. Update example SBOM files
find "$REPO_ROOT/examples" -name 'sbom.yaml' -exec \
    sed -i.bak "s/version: \"$CURRENT_VERSION\"/version: \"$NEW_VERSION\"/" {} \;

# 8. Clean up sed backup files
find "$REPO_ROOT" -name '*.bak' -delete

echo "Updated to $NEW_VERSION"
echo ""
echo "Files modified:"
echo "  VERSION"
echo "  Cargo.toml (workspace version + $(grep -c "version = \"$NEW_VERSION\" }" "$REPO_ROOT/Cargo.toml") internal deps)"
grep -rl "version: $NEW_VERSION" "$REPO_ROOT/aaf.yaml" "$REPO_ROOT/examples" "$REPO_ROOT/spec/examples" 2>/dev/null | sed 's|^|  |'
echo ""
echo "Next steps:"
echo "  1. Run 'cargo build --workspace' to verify"
echo "  2. Run 'cargo test --workspace' to verify"
echo "  3. Run 'make schema-validate' to verify"
echo "  4. Commit: git add -A && git commit -m 'chore: bump version to $NEW_VERSION'"
