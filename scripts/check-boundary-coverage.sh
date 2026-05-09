#!/usr/bin/env bash
# P081 Phase 1: boundary coverage guardrail.
#
# Fails if a commit touches in-scope boundary files without also touching:
#   - The boundary matrix fixture (docs/reference/boundary-first-api-auth-contract.json)
#   - The boundary matrix doc (docs/reference/boundary-first-api-auth-contract.md)
#   - OR includes a boundary-no-op label comment in a changed source file
#
# Usage:
#   ./scripts/check-boundary-coverage.sh [--base <ref>]
#
# If no --base is given, compares against origin/main (or main if origin/main is unavailable).
#
# Exit codes:
#   0  All in-scope changes include fixture/doc touch or no-op label
#   1  In-scope changes missing fixture/doc touch
#   2  Usage/invocation error

set -euo pipefail

BASE_REF="main"
while [[ $# -gt 0 ]]; do
    case "$1" in
        --base)
            BASE_REF="$2"
            shift 2
            ;;
        *)
            echo "Usage: $0 [--base <ref>]" >&2
            exit 2
            ;;
    esac
done

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Resolve base ref (fall back to local main if origin/main is absent).
if git -C "$REPO_ROOT" rev-parse --verify "origin/$BASE_REF" >/dev/null 2>&1; then
    BASE="origin/$BASE_REF"
elif git -C "$REPO_ROOT" rev-parse --verify "$BASE_REF" >/dev/null 2>&1; then
    BASE="$BASE_REF"
else
    echo "check-boundary-coverage: base ref '$BASE_REF' not found; skipping check" >&2
    exit 0
fi

# Files changed relative to base.
CHANGED_FILES=$(git -C "$REPO_ROOT" diff --name-only "$BASE"...HEAD 2>/dev/null || true)

if [[ -z "$CHANGED_FILES" ]]; then
    echo "check-boundary-coverage: no changed files relative to $BASE; skipping" >&2
    exit 0
fi

# In-scope boundary source paths — changes here require fixture/doc touch or no-op label.
IN_SCOPE_PATTERNS=(
    "control-plane/crates/auth/"
    "control-plane/crates/graphql-server/"
    "control-plane/crates/mcp-server/"
    "control-plane/crates/engine/"
    "control-plane/crates/db/src/repos/audit_log"
    "control-plane/crates/db/migrations/049"
    "control-plane/crates/db/migrations/050"
)

# Fixture and doc paths that satisfy the coverage requirement.
FIXTURE_PATH="docs/reference/boundary-first-api-auth-contract.json"
DOC_PATH="docs/reference/boundary-first-api-auth-contract.md"

fixture_touched=false
doc_touched=false
in_scope_touched=false

while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    if [[ "$file" == "$FIXTURE_PATH" ]]; then
        fixture_touched=true
    fi
    if [[ "$file" == "$DOC_PATH" ]]; then
        doc_touched=true
    fi
    for pattern in "${IN_SCOPE_PATTERNS[@]}"; do
        if [[ "$file" == $pattern* ]]; then
            in_scope_touched=true
            break
        fi
    done
done <<< "$CHANGED_FILES"

if ! $in_scope_touched; then
    echo "check-boundary-coverage: no in-scope boundary files changed; check passes" >&2
    exit 0
fi

# Check for boundary-no-op label in changed Rust files.
has_no_op_label=false
while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    if [[ "$file" == *.rs ]]; then
        if git -C "$REPO_ROOT" show "HEAD:$file" 2>/dev/null | grep -q 'boundary-no-op'; then
            has_no_op_label=true
            break
        fi
    fi
done <<< "$CHANGED_FILES"

if $fixture_touched && $doc_touched; then
    echo "check-boundary-coverage: fixture and doc both touched — PASS" >&2
    exit 0
fi

if $has_no_op_label; then
    echo "check-boundary-coverage: boundary-no-op label found in changed source — PASS" >&2
    exit 0
fi

echo "check-boundary-coverage: FAIL" >&2
echo "  In-scope boundary files changed but neither:" >&2
echo "    - $FIXTURE_PATH touched" >&2
echo "    - $DOC_PATH touched" >&2
echo "    - boundary-no-op label in changed .rs files" >&2
echo "" >&2
echo "  Either update the fixture/doc, or add a '// boundary-no-op: <reason>' comment" >&2
echo "  to confirm no boundary contract change was intended." >&2
exit 1
