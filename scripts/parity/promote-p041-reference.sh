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
import hashlib
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


def require_non_empty_string(value, field_name):
    if not isinstance(value, str) or not value.strip():
        sys.exit(
            f"promote-p041-reference: {field_name} is required for ready_same_tree_verified"
        )
    return value


def require_git_output(args, label):
    try:
        return subprocess.check_output(
            args,
            stderr=subprocess.DEVNULL,
            text=True,
            cwd=str(repo_root),
        )
    except (subprocess.CalledProcessError, FileNotFoundError):
        sys.exit(
            f"promote-p041-reference: refusing to promote; live git {label} "
            "could not be proven"
        )


def validate_ready_provenance(label, provenance, live_commit, live_tree, live_line_count, live_sha256):
    if not isinstance(provenance, dict):
        sys.exit(f"promote-p041-reference: {label}.provenance must be an object")
    commit_sha = require_non_empty_string(
        provenance.get("commit_sha"), f"{label}.provenance.commit_sha"
    )
    tree_id = require_non_empty_string(
        provenance.get("tree_id"), f"{label}.provenance.tree_id"
    )
    status_sha256 = require_non_empty_string(
        provenance.get("status_snapshot_sha256"),
        f"{label}.provenance.status_snapshot_sha256",
    )
    line_count = provenance.get("status_snapshot_line_count")
    if provenance.get("tree_clean") is not True:
        sys.exit(
            f"promote-p041-reference: {label}.provenance.tree_clean must be true"
        )
    if line_count != 0:
        sys.exit(
            f"promote-p041-reference: {label}.provenance.status_snapshot_line_count "
            f"must be 0, got {line_count}"
        )
    if commit_sha != live_commit:
        sys.exit(
            f"promote-p041-reference: refusing to promote; {label}.provenance.commit_sha "
            f"({commit_sha[:12]}) does not match live HEAD ({live_commit[:12]})"
        )
    if tree_id != live_tree:
        sys.exit(
            f"promote-p041-reference: refusing to promote; {label}.provenance.tree_id "
            f"({tree_id[:12]}) does not match live HEAD^{{tree}} ({live_tree[:12]})"
        )
    if line_count != live_line_count:
        sys.exit(
            f"promote-p041-reference: refusing to promote; "
            f"{label}.provenance.status_snapshot_line_count ({line_count}) "
            f"does not match live git status line count ({live_line_count})"
        )
    if status_sha256 != live_sha256:
        sys.exit(
            f"promote-p041-reference: refusing to promote; "
            f"{label}.provenance.status_snapshot_sha256 does not match live git status"
        )

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
if row.get("publication_state") != detail.get("publication_state"):
    sys.exit(
        "promote-p041-reference: row.publication_state != detail.publication_state; "
        "runtime artifacts are inconsistent"
    )
if row.get("publication_generation_id") != detail.get("publication_generation_id"):
    sys.exit(
        "promote-p041-reference: row.publication_generation_id != "
        "detail.publication_generation_id; runtime artifacts are inconsistent"
    )

# Live-checkout comparison (Decision 4, Section 6.6): promotion is allowed only
# when row, detail, and live checkout provenance all agree.
live_commit = require_git_output(["git", "rev-parse", "HEAD"], "HEAD").strip()
live_tree = require_git_output(["git", "rev-parse", "HEAD^{tree}"], "HEAD^{tree}").strip()
live_status = require_git_output(
    ["git", "status", "--porcelain=v1", "--untracked-files=all"],
    "status snapshot",
)
live_line_count = sum(1 for line in live_status.splitlines() if line)
live_sha256 = hashlib.sha256(live_status.encode()).hexdigest()
if live_line_count != 0:
    sys.exit(
        "promote-p041-reference: refusing to promote; live git status is not clean"
    )

row_prov = row.get("provenance", {})
detail_prov = detail.get("provenance", {})
for field in (
    "commit_sha",
    "tree_id",
    "tree_clean",
    "status_snapshot_sha256",
    "status_snapshot_line_count",
):
    if row_prov.get(field) != detail_prov.get(field):
        sys.exit(
            f"promote-p041-reference: row.provenance.{field} != "
            f"detail.provenance.{field}; runtime artifacts are inconsistent"
        )
validate_ready_provenance("row", row_prov, live_commit, live_tree, live_line_count, live_sha256)
validate_ready_provenance(
    "detail", detail_prov, live_commit, live_tree, live_line_count, live_sha256
)

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
