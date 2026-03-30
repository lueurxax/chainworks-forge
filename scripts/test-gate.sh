#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_PATH="$ROOT_DIR/Chainworks Forge.xcodeproj"
SCHEME_NAME="Chainworks Forge"
DESTINATION="platform=macOS"
TMP_BASE="${TMPDIR:-/tmp}/chainworks-test-gates"
TEST_PLANS_DIR="$ROOT_DIR/TestPlans"
UNSIGNED_BUILD_ARGS=(
  CODE_SIGNING_ALLOWED=NO
  CODE_SIGNING_REQUIRED=NO
  CODE_SIGN_IDENTITY=
)

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
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testCompletedRunExportHubSurface"
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

PROPOSAL_006_UNIT_TESTS=(
  "Chainworks ForgeTests/ProviderPlatformTests"
)

PROPOSAL_006_UI_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface"
)

PROPOSAL_012_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testGooseAssistantSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testWorkflowMapSurfaceShowsAfterRunStart"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AdopterSliceAccessibilityProof"
)

PROPOSAL_014_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal014ShellBrandHeaderVisible"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal014ForegroundBannerVisible"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testGooseAssistantSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testWorkflowMapSurfaceShowsAfterRunStart"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AdopterSliceAccessibilityProof"
)

PROPOSAL_016_TESTS=(
  "Chainworks ForgeTests/ActiveExecutionUniquenessGuardTests"
  "Chainworks ForgeTests/Proposal016Tests"
  "Chainworks ForgeTests/RuntimeBindingTruthSummaryTests"
  "Chainworks ForgeTests/LegacyExecutionTruthBackfillTests"
  "Chainworks ForgeTests/HistoricalRunReplayTests"
  "Chainworks ForgeTests/Proposal013Tests"
  "Chainworks ForgeTests/OrchestratorTests"
  "Chainworks ForgeTests/RunCancellationCoordinatorTests"
  "Chainworks ForgeTests/ResumeManagerTests"
  "Chainworks ForgeTests/RecoveryCoordinatorTests"
)

DEFAULT_REMOTE_UI_TEST_HOSTS=("SMacBook.local" "SMacBook")

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

normalize_host() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
}

approved_remote_ui_hosts() {
  if [[ -n "${CHAINWORKS_REMOTE_UI_TEST_HOSTS:-}" ]]; then
    IFS=',' read -r -a hosts <<<"$CHAINWORKS_REMOTE_UI_TEST_HOSTS"
    printf '%s\n' "${hosts[@]}"
  else
    printf '%s\n' "${DEFAULT_REMOTE_UI_TEST_HOSTS[@]}"
  fi
}

observed_host_names() {
  {
    hostname 2>/dev/null || true
    scutil --get LocalHostName 2>/dev/null || true
    scutil --get ComputerName 2>/dev/null || true
  } | while IFS= read -r host; do
    host="$(normalize_host "$host")"
    [[ -n "$host" ]] && printf '%s\n' "$host"
  done | awk '!seen[$0]++'
}

default_codesign_keychain() {
  local test_keychain="$HOME/Library/Keychains/test.keychain-db"
  local login_keychain="$HOME/Library/Keychains/login.keychain-db"
  if [[ -n "${CHAINWORKS_CODESIGN_KEYCHAIN:-}" ]]; then
    printf '%s\n' "$CHAINWORKS_CODESIGN_KEYCHAIN"
  elif [[ -f "$test_keychain" ]]; then
    printf '%s\n' "$test_keychain"
  else
    printf '%s\n' "$login_keychain"
  fi
}

prepare_codesign_keychain() {
  local keychain password
  local login_keychain system_keychain
  local -a search_list
  keychain="$(default_codesign_keychain)"
  password="${CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD:-}"
  login_keychain="$HOME/Library/Keychains/login.keychain-db"
  system_keychain="/Library/Keychains/System.keychain"

  [[ -f "$keychain" ]] || return 0

  search_list=("$keychain")
  if [[ -f "$login_keychain" && "$login_keychain" != "$keychain" ]]; then
    search_list+=("$login_keychain")
  fi
  if [[ -f "$system_keychain" ]]; then
    search_list+=("$system_keychain")
  fi

  security list-keychains -d user -s "${search_list[@]}" >/dev/null
  security default-keychain -d user -s "$keychain" >/dev/null

  if [[ -z "$password" ]]; then
    security show-keychain-info "$keychain" >/dev/null 2>&1 || \
      die "codesign keychain is locked: $keychain. Set CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD for remote UI gates."
    return 0
  fi

  log "Unlocking codesign keychain: $keychain"
  security unlock-keychain -p "$password" "$keychain"
  security set-keychain-settings -lut 21600 "$keychain"
  security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" >/dev/null
}

require_remote_ui_host() {
  local approved observed host
  approved=()
  while IFS= read -r host; do
    approved+=("$host")
  done < <(
    approved_remote_ui_hosts \
      | while IFS= read -r host; do
          printf '%s\n' "$(normalize_host "$host")"
        done \
      | awk '!seen[$0]++'
  )

  observed=()
  while IFS= read -r host; do
    observed+=("$host")
  done < <(observed_host_names)

  for host in "${observed[@]}"; do
    local allowed
    for allowed in "${approved[@]}"; do
      if [[ "$host" == "$allowed" ]]; then
        return 0
      fi
    done
  done

  printf 'error: UI tests are remote-only and may not run on this host.\n' >&2
  printf 'approved remote hosts: %s\n' "$(IFS=', '; printf '%s' "${approved[*]}")" >&2
  printf 'observed host names: %s\n' "$(IFS=', '; printf '%s' "${observed[*]}")" >&2
  exit 3
}

check_idle_environment() {
  local mode="${1:-strict}"
  local pattern='xcodebuild|xctest|XCTest|debugserver'
  if [[ "$mode" == "strict" ]]; then
    pattern+='|Chainworks Forge.app/Contents/MacOS/Chainworks Forge'
  fi
  local matches
  matches="$(
    {
      ps -axo pid=,command= \
        | grep -E "$pattern" \
        | grep -v -E 'grep -E|egrep |pgrep -fal|ps -axo pid=,command=|ps aux \| egrep|scripts/test-gate.sh'
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
    has_exemption_marker = "// RunRepository-exempt" in content
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
        and not has_exemption_marker
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
    "${UNSIGNED_BUILD_ARGS[@]}" \
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
    -project "$PROJECT_PATH"
    -scheme "$SCHEME_NAME"
    -destination "$DESTINATION"
    -derivedDataPath "$derived_data"
  )

  local includes_ui=0

  local test_id
  for test_id in "$@"; do
    cmd+=("-only-testing:$test_id")
    if [[ "$test_id" == Chainworks\ ForgeUITests/* ]]; then
      includes_ui=1
    fi
  done

  if [[ $includes_ui -eq 0 ]]; then
    cmd+=(test)
    cmd+=(-resultBundlePath "$result_bundle")
    cmd+=("${UNSIGNED_BUILD_ARGS[@]}")
    cmd+=("-skip-testing:Chainworks ForgeUITests")
    log "Test gate: $gate_name"
    "${cmd[@]}"
    log "Result bundle: $result_bundle"
    return
  fi

  cmd+=(test)
  cmd+=(-resultBundlePath "$result_bundle")
  log "UI test gate: $gate_name"
  "${cmd[@]}"
  log "Result bundle: $result_bundle"
}

run_proposal_016_app_proof() {
  local stamp derived_data app_bundle app_binary result_json app_log
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/proposal-016-app-${stamp}-DerivedData"
  result_json="$TMP_BASE/proposal-016-app-${stamp}-result.json"
  app_log="$TMP_BASE/proposal-016-app-${stamp}.log"
  mkdir -p "$TMP_BASE"

  log "Build gate: proposal-016 app proof"
  xcodebuild \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -derivedDataPath "$derived_data" \
    "${UNSIGNED_BUILD_ARGS[@]}" \
    build >/dev/null

  app_bundle="$derived_data/Build/Products/Debug/Chainworks Forge.app"
  app_binary="$app_bundle/Contents/MacOS/Chainworks Forge"
  [[ -x "$app_binary" ]] || die "proposal-016 app proof binary missing: $app_binary"

  log "App-launched proof: proposal-016"
  (
    cd "$ROOT_DIR"
    CHAINWORKS_P016_PROOF_AUTORUN=1 \
    CHAINWORKS_IN_MEMORY_STORE=1 \
    CHAINWORKS_P016_RESULT_PATH="$result_json" \
    "$app_binary"
  ) >"$app_log" 2>&1 &
  local app_pid=$!

  local waited=0
  while [[ $waited -lt 90 ]]; do
    if [[ -f "$result_json" ]]; then
      break
    fi
    if ! kill -0 "$app_pid" 2>/dev/null; then
      break
    fi
    sleep 1
    waited=$((waited + 1))
  done

  wait "$app_pid" || true

  [[ -f "$result_json" ]] || {
    cat "$app_log" >&2 || true
    die "proposal-016 app proof did not produce result json"
  }

  python3 - "$result_json" <<'PY'
import json, sys
path = sys.argv[1]
with open(path, 'r', encoding='utf-8') as f:
    data = json.load(f)
if not data.get("passed"):
    raise SystemExit(f"proposal-016 app proof failed: {data.get('proofStatus', 'unknown')}")
print(f"==> Proposal 016 proof result: {data.get('proofStatus', 'unknown')}")
if data.get("reportPath"):
    print(f"==> Proposal 016 report artifact: {data['reportPath']}")
PY
  log "Proof result: $result_json"
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
  proposal-016    Proposal 016 execution-truth / recovery / app-proof gate
  proposal-014    Proposal 014 design-system and brand adoption gate
  proposal-016    Proposal 016 execution-truth / recovery / app-proof gate
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
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "build"
    ;;
  fast)
    check_idle_environment allow_app
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
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "ui-smoke" "${UI_SMOKE_TESTS[@]}"
    ;;
  proposal-006|p006)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    log "proposal-006 focuses provider/setup UI proof; provider-platform unit truth remains in fast"
    run_targeted_tests "proposal-006" "${PROPOSAL_006_UI_TESTS[@]}"
    ;;
  proposal-012|p012)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "proposal-012" "${PROPOSAL_012_TESTS[@]}"
    ;;
  proposal-014|p014)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "proposal-014" "${PROPOSAL_014_TESTS[@]}"
    ;;
  proposal-016|p016)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_targeted_tests "proposal-016" "${PROPOSAL_016_TESTS[@]}"
    run_proposal_016_app_proof
    ;;
  full)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
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
