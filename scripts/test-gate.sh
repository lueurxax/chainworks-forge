#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_PATH="$ROOT_DIR/Chainworks Forge.xcodeproj"
SCHEME_NAME="Chainworks Forge"
DESTINATION="platform=macOS"
TMP_BASE="${TMPDIR:-/tmp}/chainworks-test-gates"
TEST_PLANS_DIR="$ROOT_DIR/TestPlans"

FAST_TESTS=(
  "Chainworks ForgeTests/ProviderPlatformTests"
  "Chainworks ForgeTests/OrchestratorTests"
  "Chainworks ForgeTests/ResumeManagerTests"
  "Chainworks ForgeTests/ArtifactManagerTests"
  "Chainworks ForgeTests/RunTests"
)

UI_SMOKE_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface"
)

PROPOSAL_006_TESTS=(
  "Chainworks ForgeTests/ProviderPlatformTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface"
)

log() {
  printf '==> %s\n' "$*"
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

latest_crash_log() {
  ls -1t "$HOME/Library/Logs/DiagnosticReports"/Chainworks\ Forge-*.ips 2>/dev/null | head -1 || true
}

check_idle_environment() {
  local pattern='xcodebuild|xctest|XCTest|debugserver|Chainworks Forge.app/Contents/MacOS/Chainworks Forge'
  local matches
  matches="$(
    {
      ps -axo pid=,command= \
        | grep -E "$pattern" \
        | grep -v -E 'grep -E|pgrep -fal|scripts/test-gate.sh'
    } || true
  )"
  if [[ -n "$matches" ]]; then
    printf 'Refusing to start gate while test/app processes are already running:\n%s\n' "$matches" >&2
    exit 2
  fi
}

guard_direct_run_insertion() {
  log "Guard: no direct Run construction outside RunRepository"
  python3 - "$ROOT_DIR/Chainworks Forge" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
pattern = re.compile(r"(?<![A-Za-z0-9_])Run\s*\(")
block_comments = re.compile(r"/\*.*?\*/", re.S)
string_literals = re.compile(r'"(?:\\.|[^"\\])*"')
exempt = {"RunRepository.swift", "Run.swift"}
violations = []

for file in root.rglob("*.swift"):
    if file.name in exempt:
        continue
    content = file.read_text(encoding="utf-8")
    content = block_comments.sub("", content)
    sanitized_lines = []
    for line in content.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            continue
        sanitized_lines.append(string_literals.sub('""', line))
    sanitized = "\n".join(sanitized_lines)
    if (
        pattern.search(sanitized)
        and "RunStatus" not in sanitized
        and "RunRepositoryError" not in sanitized
        and "// RunRepository-exempt" not in sanitized
    ):
        violations.append(str(file.relative_to(root.parent)))

if violations:
    print("Direct Run construction found outside RunRepository:", file=sys.stderr)
    for violation in violations:
        print(violation, file=sys.stderr)
    sys.exit(1)
PY
}

guard_plan_tag_sync() {
  log "Guard: test-plan selectedTests match Swift Testing tags"
  python3 - "$ROOT_DIR" <<'PY'
"""Verify that .xctestplan selectedTests lists stay in sync with Swift Testing tags.

Xcode test plans do not natively support Swift Testing Tag-based filtering
(as of Xcode 26 / Swift 6). The project uses selectedTests as the
bridging mechanism; this guardrail ensures the lists track the actual
@Tag declarations in source so tags remain the single source of truth.
"""
from pathlib import Path
import json
import re
import sys

root = Path(sys.argv[1])
test_dir = root / "Chainworks ForgeTests"
plans_dir = root / "TestPlans"

# ── Scan source for tagged suites ──────────────────────────────────
# Matches:  @Suite("...", .tags(.fast))  or  @Suite("...", .serialized, .tags(.fast, .provider))
suite_re = re.compile(r"@Suite\([^)]*\)")
tag_re = re.compile(r"\.tags\(([^)]+)\)")
struct_re = re.compile(r"struct\s+(\w+)")

tag_to_suites: dict[str, set[str]] = {}

for swift_file in sorted(test_dir.glob("*.swift")):
    content = swift_file.read_text(encoding="utf-8")
    lines = content.splitlines()
    for i, line in enumerate(lines):
        m_suite = suite_re.search(line)
        if not m_suite:
            continue
        m_tags = tag_re.search(m_suite.group())
        if not m_tags:
            continue
        tags = [t.strip().lstrip(".") for t in m_tags.group(1).split(",")]
        # Find the struct name on this line or the next few lines
        struct_name = None
        for j in range(i, min(i + 4, len(lines))):
            m_struct = struct_re.search(lines[j])
            if m_struct:
                struct_name = m_struct.group(1)
                break
        if struct_name:
            for tag in tags:
                tag_to_suites.setdefault(tag, set()).add(struct_name)

# ── Verify each plan ──────────────────────────────────────────────
plan_tag_map = {
    "FastGate.xctestplan": "fast",
    "ProviderGate.xctestplan": "provider",
}

errors = []
for plan_name, expected_tag in plan_tag_map.items():
    plan_path = plans_dir / plan_name
    if not plan_path.exists():
        errors.append(f"{plan_name}: file not found")
        continue

    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    plan_suites: set[str] = set()
    for target in plan.get("testTargets", []):
        for entry in target.get("selectedTests", []):
            # Entries may be "SuiteName" or "SuiteName/method()"
            plan_suites.add(entry.split("/")[0])

    expected_suites = tag_to_suites.get(expected_tag, set())

    missing_from_plan = expected_suites - plan_suites
    extra_in_plan = plan_suites - expected_suites

    if missing_from_plan:
        errors.append(
            f"{plan_name}: tagged .{expected_tag} in source but missing from selectedTests: "
            + ", ".join(sorted(missing_from_plan))
        )
    if extra_in_plan:
        errors.append(
            f"{plan_name}: in selectedTests but NOT tagged .{expected_tag} in source: "
            + ", ".join(sorted(extra_in_plan))
        )

if errors:
    print("Test-plan / tag sync violations:", file=sys.stderr)
    for e in errors:
        print(f"  • {e}", file=sys.stderr)
    sys.exit(1)
PY
}

make_stamp() {
  date +"%Y%m%d-%H%M%S"
}

run_build() {
  local gate_name="$1"
  local stamp derived_data
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  mkdir -p "$TMP_BASE"
  log "Build gate: $gate_name"
  xcodebuild \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -derivedDataPath "$derived_data" \
    build
}

run_test_plan() {
  local gate_name="$1"
  local plan_name="$2"

  local stamp derived_data result_bundle
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/${gate_name}-${stamp}.xcresult"
  mkdir -p "$TMP_BASE"

  log "Test gate (test plan): $gate_name — plan=$plan_name"
  xcodebuild test \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -testPlan "$plan_name" \
    -derivedDataPath "$derived_data" \
    -resultBundlePath "$result_bundle"
  log "Result bundle: $result_bundle"
}

run_targeted_tests() {
  local gate_name="$1"
  shift

  local stamp derived_data result_bundle
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/${gate_name}-${stamp}.xcresult"
  mkdir -p "$TMP_BASE"

  local cmd=(
    xcodebuild
    test
    -project "$PROJECT_PATH"
    -scheme "$SCHEME_NAME"
    -destination "$DESTINATION"
    -derivedDataPath "$derived_data"
    -resultBundlePath "$result_bundle"
  )

  local test_id
  for test_id in "$@"; do
    cmd+=("-only-testing:$test_id")
  done

  log "Test gate: $gate_name"
  "${cmd[@]}"
  log "Result bundle: $result_bundle"
}

run_full_suite() {
  local stamp derived_data result_bundle
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/full-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/full-${stamp}.xcresult"
  mkdir -p "$TMP_BASE"

  log "Full gate: xcodebuild test"
  xcodebuild \
    test \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -derivedDataPath "$derived_data" \
    -resultBundlePath "$result_bundle"
  log "Result bundle: $result_bundle"
}

print_usage() {
  cat <<'EOF'
Usage: ./scripts/test-gate.sh <gate>

Available gates:
  list            Show available gates
  guardrails      Run cheap source-tree guardrails only
  build           Build the app only
  fast            Guardrails + build + high-ROI unit/runtime tests
  ui-smoke        Focused operator-shell UI smoke tests
  proposal-006    Proposal 006 settings/provider/readiness gate
  full            Full xcodebuild test sign-off gate
EOF
}

BEFORE_CRASH_LOG="$(latest_crash_log)"
trap '
  status=$?
  after_crash_log="$(latest_crash_log)"
  if [[ $status -ne 0 ]]; then
    if [[ -n "$after_crash_log" && "$after_crash_log" != "$BEFORE_CRASH_LOG" ]]; then
      printf "Latest new crash log: %s\n" "$after_crash_log" >&2
    fi
  fi
' EXIT

GATE="${1:-list}"

case "$GATE" in
  list|-h|--help)
    print_usage
    ;;
  guardrails)
    check_idle_environment
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    guard_plan_tag_sync
    ;;
  build)
    check_idle_environment
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "build"
    ;;
  fast)
    check_idle_environment
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "fast"
    if [[ "${USE_TEST_PLANS:-}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/FastGate.xctestplan" ]]; then
      run_test_plan "fast" "FastGate"
    else
      run_targeted_tests "fast" "${FAST_TESTS[@]}"
    fi
    ;;
  ui-smoke)
    check_idle_environment
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "ui-smoke" "${UI_SMOKE_TESTS[@]}"
    ;;
  proposal-006|p006)
    check_idle_environment
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    if [[ "${USE_TEST_PLANS:-}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/ProviderGate.xctestplan" ]]; then
      run_test_plan "proposal-006" "ProviderGate"
    else
      run_targeted_tests "proposal-006" "${PROPOSAL_006_TESTS[@]}"
    fi
    ;;
  full)
    check_idle_environment
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "full"
    if [[ "${USE_TEST_PLANS:-}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/FullGate.xctestplan" ]]; then
      run_test_plan "full" "FullGate"
    else
      run_full_suite
    fi
    ;;
  *)
    print_usage >&2
    die "Unknown gate: $GATE"
    ;;
esac
