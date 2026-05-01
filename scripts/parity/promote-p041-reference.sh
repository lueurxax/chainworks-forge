#!/usr/bin/env bash
# Promote runtime P041 parity evidence into tracked docs/reference snapshots.
#
# This is the only legal write path for docs/reference/p031-p041-parity-evidence.json.
# Ordinary gate execution (./scripts/test-gate.sh proposal-041) never touches
# tracked reference files. Call this script explicitly after a successful gate run
# to advance the reference snapshot.
#
# Usage:
#   ./scripts/parity/promote-p041-reference.sh [--repo-root <path>]
#
# Exit codes:
#   0 — promoted successfully, or snapshot already matches (no-op)
#   1 — promotion refused (row not ready_same_tree_verified, or missing artifacts)

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
ROOT_DIR="$(cd "$SCRIPT_DIR/../.." && pwd)"

while [[ $# -gt 0 ]]; do
  case "$1" in
    --repo-root)
      ROOT_DIR="$2"
      shift 2
      ;;
    *)
      echo "Unknown argument: $1" >&2
      exit 1
      ;;
  esac
done

RUNTIME_ROW="$ROOT_DIR/control-plane/target/parity/publication/current/p031-phase-0-manifest-row.json"
RUNTIME_DETAIL="$ROOT_DIR/control-plane/target/parity/publication/current/p031-p041-parity-evidence.json"
REFERENCE_DETAIL="$ROOT_DIR/docs/reference/p031-p041-parity-evidence.json"

REQUIRED_ROW_SCHEMA="p031-phase-0-runtime-manifest-row.v1"
REQUIRED_DETAIL_SCHEMA="p031-p041-parity-evidence.v1"
READY_STATUS="ready_same_tree_verified"

# Require python3 for JSON parsing
if ! command -v python3 &>/dev/null; then
  echo "promote-p041-reference: python3 required" >&2
  exit 1
fi

if [[ ! -f "$RUNTIME_ROW" ]]; then
  echo "promote-p041-reference: runtime row not found at $RUNTIME_ROW" >&2
  echo "  Run './scripts/test-gate.sh proposal-041' on a clean tree first." >&2
  exit 1
fi

if [[ ! -f "$RUNTIME_DETAIL" ]]; then
  echo "promote-p041-reference: runtime detail not found at $RUNTIME_DETAIL" >&2
  echo "  Run './scripts/test-gate.sh proposal-041' on a clean tree first." >&2
  exit 1
fi

python3 - "$RUNTIME_ROW" "$RUNTIME_DETAIL" "$REFERENCE_DETAIL" \
         "$REQUIRED_ROW_SCHEMA" "$REQUIRED_DETAIL_SCHEMA" "$READY_STATUS" \
         "$ROOT_DIR" <<'PY'
import json
import subprocess
import sys
from pathlib import Path

runtime_row_path = Path(sys.argv[1])
runtime_detail_path = Path(sys.argv[2])
reference_detail_path = Path(sys.argv[3])
required_row_schema = sys.argv[4]
required_detail_schema = sys.argv[5]
ready_status = sys.argv[6]
repo_root = Path(sys.argv[7])

row = json.loads(runtime_row_path.read_text())
detail = json.loads(runtime_detail_path.read_text())

# Validate schema versions
if row.get("schema_version") != required_row_schema:
    sys.exit(
        f"promote-p041-reference: row schema_version mismatch: "
        f"expected {required_row_schema}, got {row.get('schema_version')}"
    )
if detail.get("schema_version") != required_detail_schema:
    sys.exit(
        f"promote-p041-reference: detail schema_version mismatch: "
        f"expected {required_detail_schema}, got {detail.get('schema_version')}"
    )

# Refuse to promote unless the row is ready_same_tree_verified
if row.get("validation_status") != ready_status:
    sys.exit(
        f"promote-p041-reference: refusing to promote; row.validation_status is "
        f"'{row.get('validation_status')}', not '{ready_status}'"
    )

# Verify cross-artifact compatibility before promoting
if row.get("validation_status") != detail.get("overall_status"):
    sys.exit(
        "promote-p041-reference: row.validation_status != detail.overall_status; "
        "runtime artifacts are inconsistent"
    )

# Live-HEAD comparison (Decision 4, Section 6.6): refuse promotion if HEAD has moved
# since the gate ran. An operator running the promoter after a rebase, amend, or
# checkout could otherwise stamp stale runtime evidence into tracked reference snapshots.
try:
    live_commit = subprocess.check_output(
        ["git", "rev-parse", "HEAD"],
        stderr=subprocess.DEVNULL,
        text=True,
        cwd=str(repo_root),
    ).strip()
    live_tree = subprocess.check_output(
        ["git", "rev-parse", "HEAD^{tree}"],
        stderr=subprocess.DEVNULL,
        text=True,
        cwd=str(repo_root),
    ).strip()
    prov = row.get("provenance", {})
    row_commit = prov.get("commit_sha", "")
    row_tree = prov.get("tree_id", "")
    if live_commit and row_commit and row_commit != live_commit:
        sys.exit(
            f"promote-p041-reference: refusing to promote; row.provenance.commit_sha "
            f"({row_commit[:12]}) does not match live HEAD ({live_commit[:12]}); "
            "HEAD may have moved since the gate ran — rerun the gate on the same tree first"
        )
    if live_tree and row_tree and row_tree != live_tree:
        sys.exit(
            f"promote-p041-reference: refusing to promote; row.provenance.tree_id "
            f"({row_tree[:12]}) does not match live HEAD^{{tree}} ({live_tree[:12]}); "
            "HEAD may have moved since the gate ran — rerun the gate on the same tree first"
        )
except subprocess.CalledProcessError:
    pass  # Not in a git repo; skip live-checkout comparison

# Idempotency: skip copy if the reference already matches
if reference_detail_path.is_file():
    existing = json.loads(reference_detail_path.read_text())
    if existing == detail:
        print(
            f"promote-p041-reference: reference snapshot already matches runtime "
            f"generation {row.get('publication_generation_id')} (no-op)"
        )
        sys.exit(0)

# Write the promoted reference snapshot
reference_detail_path.parent.mkdir(parents=True, exist_ok=True)
reference_detail_path.write_text(json.dumps(detail, indent=2) + "\n")
print(
    f"promote-p041-reference: promoted generation "
    f"{row.get('publication_generation_id')} to {reference_detail_path}"
)
PY
