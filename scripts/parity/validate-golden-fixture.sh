#!/usr/bin/env bash
set -euo pipefail
ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
fixture_id="${1:?usage: ./scripts/parity/validate-golden-fixture.sh <fixture_id>}"
(
  cd "$ROOT_DIR/control-plane"
  cargo test -p engine --test proposal_041_parity "proposal_041_fixture_inventory_and_schema_contract" -- --exact --nocapture
)
case "$fixture_id" in
  proposal-loop-basic|implementation-refine-review|approval-pause-resume|retry-recovery-flow|cancelled-or-blocked-run|terminal-report-evidence|projection-readback-surface)
    ;;
  *)
    echo "P041 validate: unknown fixture '$fixture_id'" >&2
    exit 1
    ;;
esac
