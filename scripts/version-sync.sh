#!/usr/bin/env bash
# version-sync.sh -- Propagate the version from VERSION file to all sources.
#
# The VERSION file is the single source of truth.
# This script updates: Cargo.toml, src/main.rs, README.md, docs/LANGUAGE.md
#
# Usage:
#   ./scripts/version-sync.sh          # sync current VERSION to all files
#   ./scripts/version-sync.sh 0.5.0    # set new version and sync
#   ./scripts/version-sync.sh --check  # verify all files match (for CI)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
VERSION_FILE="$ROOT/VERSION"

# ── Read or set version ──────────────────────────────────────
if [ "${1:-}" = "--check" ]; then
    CHECK_MODE=true
    NEW_VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
elif [ -n "${1:-}" ]; then
    CHECK_MODE=false
    NEW_VERSION="$1"
    echo "$NEW_VERSION" > "$VERSION_FILE"
else
    CHECK_MODE=false
    NEW_VERSION="$(tr -d '[:space:]' < "$VERSION_FILE")"
fi

echo "Version: $NEW_VERSION"

# ── Files to sync ────────────────────────────────────────────
ERRORS=0

sync_file() {
    local file="$1"
    local pattern="$2"
    local replacement="$3"

    if [ ! -f "$file" ]; then
        echo "  SKIP  $file (not found)"
        return
    fi

    if $CHECK_MODE; then
        if grep -q "$replacement" "$file"; then
            echo "  OK    $file"
        else
            echo "  FAIL  $file (version mismatch)"
            ERRORS=$((ERRORS + 1))
        fi
    else
        if [[ "$OSTYPE" == "darwin"* ]]; then
            sed -i '' "s|$pattern|$replacement|g" "$file"
        else
            sed -i "s|$pattern|$replacement|g" "$file"
        fi
        echo "  SYNC  $file"
    fi
}

# Cargo.toml: version = "x.y.z"
sync_file "$ROOT/Cargo.toml" \
    'version = "[0-9]*\.[0-9]*\.[0-9]*"' \
    "version = \"$NEW_VERSION\""

# src/main.rs: v0.X.Y in the REPL banner
sync_file "$ROOT/src/main.rs" \
    'v[0-9]*\.[0-9]*\.[0-9]*' \
    "v$NEW_VERSION"

# README.md: Sabot vX.Y.Z
sync_file "$ROOT/README.md" \
    'Sabot v[0-9]*\.[0-9]*\.[0-9]*' \
    "Sabot v$NEW_VERSION"

# docs/LANGUAGE.md: vX.Y.Z in title
sync_file "$ROOT/docs/LANGUAGE.md" \
    'v[0-9]*\.[0-9]*\.[0-9]*' \
    "v$NEW_VERSION"

# ── Update Cargo.lock if not in check mode ───────────────────
if ! $CHECK_MODE && command -v cargo &>/dev/null; then
    echo ""
    echo "Updating Cargo.lock..."
    (cd "$ROOT" && cargo update --workspace 2>/dev/null || true)
fi

# ── Summary ──────────────────────────────────────────────────
echo ""
if $CHECK_MODE; then
    if [ $ERRORS -gt 0 ]; then
        echo "FAILED: $ERRORS file(s) have mismatched versions."
        echo "Run: ./scripts/version-sync.sh"
        exit 1
    else
        echo "All files match VERSION ($NEW_VERSION)."
    fi
else
    echo "All files synced to v$NEW_VERSION."
fi
