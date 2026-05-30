#!/usr/bin/env bash
# P081 Phase 1: boundary coverage guardrail.
#
# Fails if a commit touches in-scope boundary files without also satisfying one of:
#   - The boundary matrix fixture (docs/reference/boundary-first-api-auth-contract.json)
#     AND the boundary matrix doc (docs/reference/boundary-first-api-auth-contract.md) touched
#   - A `matrix_row` citation in a changed source file (e.g. // matrix_row: p081.ui_operator.*)
#   - A `boundary-no-op` label comment in a changed source file
#
# Usage:
#   ./scripts/check-boundary-coverage.sh [--base <ref>]
#
# If no --base is given, compares against origin/main (or main if origin/main is unavailable).
#
# Exit codes:
#   0  All in-scope changes satisfy fixture/doc, matrix_row citation, or no-op label
#   1  In-scope changes missing fixture/doc touch, matrix_row citation, or boundary-no-op label
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
    # Fail closed: missing base ref means we cannot enforce coverage.
    # Set BOUNDARY_COVERAGE_SKIP_MISSING_BASE=1 to bypass in local-only workflows.
    if [[ "${BOUNDARY_COVERAGE_SKIP_MISSING_BASE:-0}" == "1" ]]; then
        echo "check-boundary-coverage: base ref '$BASE_REF' not found; BOUNDARY_COVERAGE_SKIP_MISSING_BASE=1; skipping" >&2
        exit 0
    fi
    echo "check-boundary-coverage: base ref '$BASE_REF' not found; cannot enforce boundary coverage (set BOUNDARY_COVERAGE_SKIP_MISSING_BASE=1 to bypass locally)" >&2
    exit 1
fi

# Files changed relative to base.
CHANGED_FILES=$(git -C "$REPO_ROOT" diff --name-only "$BASE"...HEAD 2>&1)
if [[ $? -ne 0 ]]; then
    echo "check-boundary-coverage: git diff failed; cannot enforce boundary coverage" >&2
    exit 1
fi

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
    "control-plane/crates/db/migrations/068_p081"
    "control-plane/crates/db/migrations/069_p081"
    "control-plane/crates/db/migrations/070_p081"
    "control-plane/crates/db/migrations/071_p081"
    "control-plane/crates/db/migrations/072_p081"
    "control-plane/crates/db/migrations/073_p081"
    "control-plane/crates/db/migrations/074_p081"
    "control-plane/crates/db/migrations/075_p081"
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

# Check for boundary-no-op label or matrix_row citation in changed lines of changed Rust files.
# Uses structured anchored patterns to prevent accidental satisfaction via unrelated comments:
#   - matrix_row citation: "// matrix_row: p081.<row_id>" on a changed line
#   - boundary-no-op label: "// boundary-no-op: <reason>" on a changed line
# Matches only lines actually changed in the diff, not any literal in the file body.
has_no_op_label=false
has_matrix_row_citation=false
while IFS= read -r file; do
    [[ -z "$file" ]] && continue
    if [[ "$file" == *.rs ]]; then
        # Extract only added/modified lines from the diff for this file (lines starting with +).
        changed_lines=$(git -C "$REPO_ROOT" diff "$BASE"...HEAD -- "$file" 2>/dev/null \
            | grep '^+' | grep -v '^+++' || true)
        if echo "$changed_lines" | grep -qE '//[[:space:]]*boundary-no-op:[[:space:]]+\S'; then
            has_no_op_label=true
        fi
        if echo "$changed_lines" | grep -qE '//[[:space:]]*matrix_row:[[:space:]]+p081\.[a-z0-9_]+\.[a-z0-9_]+\.[a-z0-9_]+'; then
            has_matrix_row_citation=true
        fi
    fi
done <<< "$CHANGED_FILES"

if $fixture_touched && $doc_touched; then
    echo "check-boundary-coverage: fixture and doc both touched — PASS" >&2
    exit 0
fi

if $has_matrix_row_citation; then
    echo "check-boundary-coverage: matrix_row citation found in changed source — PASS" >&2
    exit 0
fi

if $has_no_op_label; then
    echo "check-boundary-coverage: boundary-no-op label found in changed source — PASS" >&2
    exit 0
fi

echo "check-boundary-coverage: FAIL" >&2
echo "  In-scope boundary files changed but none of the following were satisfied:" >&2
echo "    - Both $FIXTURE_PATH and $DOC_PATH touched" >&2
echo "    - A '// matrix_row: <row_id>' citation in changed .rs files" >&2
echo "    - A '// boundary-no-op: <reason>' label in changed .rs files" >&2
echo "" >&2
echo "  Options:" >&2
echo "    1. Update the fixture and doc to reflect the boundary contract change." >&2
echo "    2. Add '// matrix_row: <p081.row_id>' to the boundary-specific test." >&2
echo "    3. Add '// boundary-no-op: <reason>' to confirm no boundary contract change." >&2
exit 1
