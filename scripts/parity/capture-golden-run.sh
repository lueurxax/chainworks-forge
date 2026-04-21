#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
FIXTURE_ROOT="$ROOT_DIR/control-plane/crates/engine/tests/fixtures/parity/golden-runs"

usage() {
  cat >&2 <<'USAGE'
usage:
  ./scripts/parity/capture-golden-run.sh <fixture_id> --validate
  ./scripts/parity/capture-golden-run.sh <fixture_id> --record --author <name> --reason <reason> [--previous-fixture-dir <path>]

P041 V1 capture is a checked-in fixture lifecycle. The script validates the
fixture inventory and, when --record is provided, writes capture-record.md so
regeneration has a durable reason/author trail. When --previous-fixture-dir is
provided, the script also writes regeneration-diff-report.json comparing old
expected truth to the current fixture expectations.
USAGE
}

fixture_id="${1:-}"
if [[ -z "$fixture_id" ]]; then
  usage
  exit 2
fi
shift

mode="validate"
author=""
reason=""
previous_fixture_dir=""
while [[ $# -gt 0 ]]; do
  case "$1" in
    --validate)
      mode="validate"
      shift
      ;;
    --record)
      mode="record"
      shift
      ;;
    --author)
      author="${2:-}"
      shift 2
      ;;
    --reason)
      reason="${2:-}"
      shift 2
      ;;
    --previous-fixture-dir)
      previous_fixture_dir="${2:-}"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      echo "P041 capture: unknown argument '$1'" >&2
      usage
      exit 2
      ;;
  esac
done

fixture_dir="$FIXTURE_ROOT/$fixture_id"
fixture_json="$fixture_dir/fixture.json"
if [[ ! -f "$fixture_json" ]]; then
  echo "P041 capture: unknown fixture '$fixture_id'" >&2
  exit 1
fi

python3 - "$fixture_json" "$fixture_dir" <<'PY'
import json
import sys
from pathlib import Path

fixture_json = Path(sys.argv[1])
fixture_dir = Path(sys.argv[2])
fixture = json.loads(fixture_json.read_text())
required_top = [
    "schema_version",
    "fixture_id",
    "fixture_revision",
    "captured_from",
    "frozen_inputs",
    "expected_client_truth",
    "normalization_rules",
    "regeneration",
]
for key in required_top:
    if key not in fixture:
        raise SystemExit(f"P041 capture: fixture missing {key}")
if fixture["schema_version"] != "golden-run-fixture.v1":
    raise SystemExit("P041 capture: unsupported fixture schema")
paths = []
for group in ("frozen_inputs", "expected_client_truth"):
    for rel in fixture[group].values():
        paths.append(rel)
for rel in paths:
    path = fixture_dir / rel
    if not path.is_file():
        raise SystemExit(f"P041 capture: missing fixture file {rel}")
    json.loads(path.read_text())
print(f"P041 capture: validated {fixture['fixture_id']} revision {fixture['fixture_revision']}")
PY

if [[ "$mode" == "record" ]]; then
  if [[ -z "$author" || -z "$reason" ]]; then
    echo "P041 capture: --record requires --author and --reason" >&2
    exit 2
  fi
  if [[ -n "$previous_fixture_dir" && ! -d "$previous_fixture_dir" ]]; then
    echo "P041 capture: --previous-fixture-dir does not exist: $previous_fixture_dir" >&2
    exit 2
  fi
  record_path="$fixture_dir/capture-record.md"
  cat > "$record_path" <<EOF
# P041 Capture Record

Fixture: $fixture_id
Captured on: $(date -u +%Y-%m-%dT%H:%M:%SZ)
Author: $author
Reason: $reason

## Source command

\`\`\`bash
./scripts/parity/capture-golden-run.sh $fixture_id --record --author "$author" --reason "$reason"
\`\`\`

## Required follow-up

- Run \`./scripts/test-gate.sh proposal-041\`.
- Commit fixture changes, capture record, regeneration diff report, behavioral diff report, and P031 handoff update together.
- Do not regenerate fixtures without a review/audit artifact explaining semantic drift.
EOF
  python3 - "$fixture_dir" "$previous_fixture_dir" "$author" "$reason" <<'PY'
import json
import sys
from pathlib import Path

fixture_dir = Path(sys.argv[1])
previous_fixture_dir = Path(sys.argv[2]) if sys.argv[2] else None
author = sys.argv[3]
reason = sys.argv[4]
fixture = json.loads((fixture_dir / "fixture.json").read_text())

def expected_truth(root):
    doc = json.loads((root / "fixture.json").read_text())
    truth = {}
    for surface, rel in doc["expected_client_truth"].items():
        truth[surface] = json.loads((root / rel).read_text())
    return truth

new_truth = expected_truth(fixture_dir)
old_truth = expected_truth(previous_fixture_dir) if previous_fixture_dir else None
comparisons = []
divergences = []
for surface, new_value in new_truth.items():
    old_value = old_truth.get(surface) if old_truth else None
    status = "initial_capture" if old_truth is None else ("matched" if old_value == new_value else "changed")
    comparisons.append({
        "surface": surface,
        "status": status,
        "old": old_value,
        "new": new_value,
    })
    if status == "changed":
        divergences.append({
            "path": f"$.expected_client_truth.{surface}",
            "severity": "info",
            "owner_surface": surface,
            "expected": old_value,
            "actual": new_value,
            "investigation_hint": "Review the capture reason and Swift source owner before accepting fixture regeneration.",
        })

report = {
    "schema_version": "behavioral-diff-report.v1",
    "report_id": f"{fixture['fixture_id']}-regeneration",
    "mode": "fixture_regeneration",
    "proof_mode": "fixture_lifecycle",
    "run_fixture_id": fixture["fixture_id"],
    "fixture_revision": fixture["fixture_revision"],
    "client_snapshot_ref": str(fixture_dir / "fixture.json"),
    "server_replay_ref": None,
    "regeneration": {
        "author": author,
        "reason": reason,
        "previous_fixture_dir": str(previous_fixture_dir) if previous_fixture_dir else None,
        "initial_capture_without_prior": previous_fixture_dir is None,
    },
    "comparison_surface": list(new_truth.keys()),
    "surface_comparisons": comparisons,
    "divergences": divergences,
    "summary": {
        "blocking_count": 0,
        "warning_count": 0,
        "info_count": len(divergences),
        "operator_message": "Fixture regeneration diff recorded.",
    },
    "verdict": "ready",
}
(fixture_dir / "regeneration-diff-report.json").write_text(json.dumps(report, indent=2) + "\n")
PY
  echo "P041 capture: wrote $record_path"
  echo "P041 capture: wrote $fixture_dir/regeneration-diff-report.json"
fi
