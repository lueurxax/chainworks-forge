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
  "Chainworks ForgeTests/AgentSessionTests"
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

PROPOSAL_012_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRuntimeAssistantSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testWorkflowMapSurfaceShowsAfterRunStart"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AdopterSliceAccessibilityProof"
)

PROPOSAL_013_TESTS=(
  "Chainworks ForgeTests/Proposal013Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal013AppProofSurface"
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
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRuntimeAssistantSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testWorkflowMapSurfaceShowsAfterRunStart"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AdopterSliceAccessibilityProof"
)

PROPOSAL_015_TESTS=(
  "Chainworks ForgeTests/Proposal015Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal015SkillVisibilityProofSurface"
)

PROPOSAL_015_NON_UI_TESTS=(
  "Chainworks ForgeTests/Proposal015Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
)

PROPOSAL_018_TESTS=(
  "Chainworks ForgeTests/AgentSessionTests"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests"
)

PROPOSAL_019_TESTS=(
  "Chainworks ForgeTests/Proposal019Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests"
  "Chainworks ForgeTests/OrchestratorTests"
)

PROPOSAL_022_TESTS=(
  "Chainworks ForgeTests/Proposal022Tests"
  "Chainworks ForgeTests/Proposal022ScaffoldingTests"
)

PROPOSAL_024_TESTS=(
  "Chainworks ForgeTests/Proposal024RunSurfaceTests"
  "Chainworks ForgeTests/RunArtifactHierarchyBuilderTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testCompletedRunExportHubSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal024FocusedTimelineInspectorSurface"
)

PROPOSAL_025_TESTS=(
  "Chainworks ForgeTests/Proposal025Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/Chainworks_ForgeTests"
)

PROPOSAL_026_TESTS=(
  "Chainworks ForgeTests/Proposal026Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests"
  "Chainworks ForgeTests/ProviderPlatformTests"
)

PROPOSAL_027_TESTS=(
  "Chainworks ForgeTests/Proposal027Tests"
)

PROPOSAL_029_TESTS=(
  "Chainworks ForgeTests/Proposal029Tests"
  "Chainworks ForgeTests/Proposal026Tests"
  "Chainworks ForgeTests/ProviderPlatformTests"
)

PROPOSAL_032_TESTS=(
  "Chainworks ForgeTests/Proposal032Tests"
  "Chainworks ForgeTests/ResumeManagerTests"
  "Chainworks ForgeTests/RecoveryCoordinatorTests"
  "Chainworks ForgeTests/WorkflowMapProjectionTests"
)

PROPOSAL_033_TESTS=(
  "Chainworks ForgeTests/Proposal033Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/LiveACPConnectionProofTests"
  "Chainworks ForgeTests/MVPGoldenRunTests"
  "Chainworks ForgeTests/ProviderPlatformTests"
)

PROPOSAL_037_TESTS=(
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorFailClosesACPProposalReviewReadLoopStallsBeforeWatchdogAndEmitsDurableFailureEvidence()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/acpProposalReviewerReadLoopStallFailsEarlyWithDurableFailureEvidence()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorSurfacesWatchdogFirstProgressHangsWithoutPerformingRetryLineageItself()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorFailsClosedWhenMutatingToolSuccessProducesNoFilesystemSideEffect()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterRunawayGuardrailTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterOversizedRawToolPayloadGuardrailTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterRuntimeHomeGrowthGuardrailTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterSessionHistoryTokenBudgetTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorPreservesCodexACPSessionReuseScopeInsteadOfForcingNone()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesSilentCodexEOFBeforeFinalResultWithAFreshSession()"
  "Chainworks ForgeTests/OrchestratorTests/sequentialWatchdogFailuresCreateDurableSameStageRetryLineageBeforeSucceeding()"
  "Chainworks ForgeTests/OrchestratorTests/downstreamStageMaterializationIsDurablyVisibleBeforeFirstAgentResult()"
  "Chainworks ForgeTests/OrchestratorTests/sequentialAgentExecutionIsDurablyVisibleBeforeFirstAgentResult()"
  "Chainworks ForgeTests/OrchestratorTests/parallelAgentExecutionsAreDurablyVisibleBeforeFirstAgentResult()"
  "Chainworks ForgeTests/OrchestratorTests/orchestratorCreatesTheCursorScheduledStageIterationInsteadOfReusingAStaleRunningStage()"
  "Chainworks ForgeTests/OrchestratorTests/implementationPartialArtifactSetRecoversFailedCodeWriterIntoContinuePath()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileImmediatelyAfterAllFanoutReviewersSettle()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceReconcilesExpiredPostFanoutSettlement()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileFreshStartedDownstreamStageBeforeFirstAgentWork()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceReconcilesTrulyStaleStartedDownstreamStageAfterExtendedGrace()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileNewlyStartedStageFromPreviousSessionClose()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileWhileParallelStageAgentsAreStillRunning()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceReconcilesTrulyStaleRunningAgentStageAfterExtendedGrace()"
  "Chainworks ForgeTests/RecoveryCoordinatorTests"
  "Chainworks ForgeTests/Proposal013Tests"
  "Chainworks ForgeTests/Proposal019Tests"
  "Chainworks ForgeTests/LiveProposalWorkflowTests"
  "Chainworks ForgeTests/WorkflowMapProjectionTests"
  "Chainworks ForgeTests/RunTimelineInspectorViewTests"
)

PROPOSAL_044_TESTS=(
  "test_approve_manual_gate_with_post_approval_tasks_sets_running"
  "test_approve_simple_manual_gate_settles_completed"
  "test_compile_n_phase_ordering"
  "test_post_approval_tasks_enqueued_after_approval"
  "test_end_state_with_tasks_does_not_short_circuit"
  "test_n_phase_sequence_ordering"
  "test_post_approval_retry_requires_fresh_approval"
  "test_simple_manual_gate_no_regression"
  "test_state_11_to_state_12_happy_path"
)

DEFAULT_REMOTE_UI_TEST_HOSTS=("SMacBook.local" "SMacBook")
LAST_BUILD_DERIVED_DATA_PATH=""

log() {
  printf '==> %s\n' "$*"
}

should_use_unsigned_ui_tests() {
  local configured="${CHAINWORKS_USE_UNSIGNED_UI_TESTS:-}"
  if [[ -n "$configured" ]]; then
    [[ "$configured" == "1" ]]
    return
  fi

  if [[ -n "${SSH_CONNECTION:-}" ]]; then
    return 1
  fi

  return 0
}

append_xcodebuild_signing_args() {
  local gate_name="${1:-}"
  local includes_ui="${2:-0}"

  if [[ "$includes_ui" == "1" ]] && ! should_use_unsigned_ui_tests; then
    return 0
  fi

  if [[ "$gate_name" == "full" ]] && ! should_use_unsigned_ui_tests; then
    return 0
  fi

  printf '%s\0' "${UNSIGNED_BUILD_ARGS[@]}"
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

gate_requires_remote_ui_host() {
  case "${1:-}" in
    ui-smoke|proposal-006|p006|proposal-012|p012|proposal-013|p013|proposal-014|p014|proposal-015|p015|proposal-022|p022|proposal-024|p024|full)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

should_wrap_gate_in_terminal_gui_session() {
  local gate_name="${1:-}"
  gate_requires_remote_ui_host "$gate_name" || return 1
  [[ -n "${SSH_CONNECTION:-}" ]] || return 1
  [[ "${CHAINWORKS_GUI_SESSION_WRAPPED:-0}" != "1" ]] || return 1
  command -v open >/dev/null 2>&1 || return 1
  return 0
}

emit_forwarded_chainworks_env() {
  local key
  local -a allowed_chainworks_env=(
    CHAINWORKS_REMOTE_UI_TEST_HOSTS
    CHAINWORKS_USE_UNSIGNED_UI_TESTS
    CHAINWORKS_GUI_GATE_TIMEOUT_SECONDS
    CHAINWORKS_CODESIGN_KEYCHAIN
    CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD
    CHAINWORKS_P013_UI_SUCCESS_GRACE_SECONDS
    CHAINWORKS_P013_UI_HARD_TIMEOUT_SECONDS
    CHAINWORKS_P015_UI_SUCCESS_GRACE_SECONDS
    CHAINWORKS_P015_UI_HARD_TIMEOUT_SECONDS
    CHAINWORKS_P022_UI_SUCCESS_GRACE_SECONDS
    CHAINWORKS_P022_UI_HARD_TIMEOUT_SECONDS
  )

  for key in "${allowed_chainworks_env[@]}"; do
    if [[ -n ${!key+x} ]]; then
      printf 'export %s=%q\n' "$key" "${!key}"
    fi
  done

  if [[ -n ${USE_TEST_PLANS+x} ]]; then
    printf 'export %s=%q\n' "USE_TEST_PLANS" "$USE_TEST_PLANS"
  fi
}

run_gate_in_terminal_gui_session() {
  local gate_name="$1"
  local stamp command_path log_path rc_path resolved_unsigned_ui_tests
  stamp="$(make_stamp)"
  command_path="$TMP_BASE/${gate_name}-${stamp}-gui.command"
  log_path="$TMP_BASE/${gate_name}-${stamp}-gui.log"
  rc_path="$TMP_BASE/${gate_name}-${stamp}-gui.rc"
  mkdir -p "$TMP_BASE"

  if should_use_unsigned_ui_tests; then
    resolved_unsigned_ui_tests=1
  else
    resolved_unsigned_ui_tests=0
  fi

  {
    printf '#!/bin/zsh\n'
    printf 'cd %q || exit 97\n' "$ROOT_DIR"
    printf 'export CHAINWORKS_GUI_SESSION_WRAPPED=1\n'
    printf 'export CHAINWORKS_USE_UNSIGNED_UI_TESTS=%q\n' "$resolved_unsigned_ui_tests"
    printf 'trap "" HUP\n'
    emit_forwarded_chainworks_env
    printf 'nohup ./scripts/test-gate.sh %q > %q 2>&1\n' "$gate_name" "$log_path"
    printf 'printf %%s \"$?\" > %q\n' "$rc_path"
  } >"$command_path"
  chmod +x "$command_path"

  log "Re-executing gate '$gate_name' in Terminal GUI session"
  open -a Terminal "$command_path" >/dev/null 2>&1

  local offset=0
  local start_epoch timeout_seconds now size rc_value
  start_epoch="$(date +%s)"
  timeout_seconds="${CHAINWORKS_GUI_GATE_TIMEOUT_SECONDS:-7200}"

  while true; do
    if [[ -f "$log_path" ]]; then
      size="$(wc -c <"$log_path" | tr -d '[:space:]')"
      if [[ -n "$size" ]] && (( size > offset )); then
        tail -c "+$((offset + 1))" "$log_path"
        offset="$size"
      fi
    fi

    if [[ -f "$rc_path" ]]; then
      rc_value="$(cat "$rc_path")"
      return "${rc_value:-1}"
    fi

    now="$(date +%s)"
    if (( now - start_epoch >= timeout_seconds )); then
      printf 'error: terminal GUI session timed out after %ss for gate %s\n' "$timeout_seconds" "$gate_name" >&2
      return 124
    fi

    sleep 2
  done
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
  if should_use_unsigned_ui_tests; then
    return 0
  fi

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
  local matches
  matches="$(
    {
      ps -axo pid=,comm=,args= \
        | awk -v mode="$mode" '
            {
              pid = $1
              comm = $2
              $1 = ""
              $2 = ""
              sub(/^[[:space:]]+/, "", $0)
              args = $0

              if (comm ~ /^(xcodebuild|xctest|XCTest|debugserver)$/) {
                print pid " " args
                next
              }

              if (mode == "strict" && args ~ /Chainworks Forge\.app\/Contents\/MacOS\/Chainworks Forge/) {
                print pid " " args
              }
            }
          '
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

guard_portability_paths() {
  log "Guard: portability-sensitive sources avoid hardcoded user paths"
  python3 - "$ROOT_DIR/Chainworks Forge" "$ROOT_DIR/Chainworks ForgeTests" <<'PY'
from pathlib import Path
import sys

app_root = Path(sys.argv[1])
test_root = Path(sys.argv[2])
violations = []

sensitive_files = [
    app_root / "Support/PreviewSupport.swift",
    app_root / "Views/DeliveryPreflightReportView.swift",
    app_root / "Views/ReleaseGateView.swift",
    app_root / "Views/IdeaListView.swift",
    test_root / "Chainworks_ForgeTests.swift",
    test_root / "RuntimeSessionBridgeTests.swift",
]

for f in sensitive_files:
    if not f.exists():
        continue
    content = f.read_text(encoding="utf-8")
    if "/Users/user/" in content:
        violations.append(f"{f.name}: contains hardcoded /Users/user/ path")

cwd_sensitive_files = [
    app_root / "Chainworks_ForgeApp.swift",
    app_root / "Views/UITestDirectSurfaces.swift",
    app_root / "Engine/SampleRunLauncher.swift",
]
forbidden_fragments = [
    "repoRoot: FileManager.default.currentDirectoryPath",
    "run.repoRoot = FileManager.default.currentDirectoryPath",
    "workspaceRootPath: FileManager.default.currentDirectoryPath",
]

for f in cwd_sensitive_files:
    if not f.exists():
        continue
    content = f.read_text(encoding="utf-8")
    for frag in forbidden_fragments:
        if frag in content:
            violations.append(f"{f.name}: derives repo truth from cwd via: {frag}")

if violations:
    print("Portability violations:", file=sys.stderr)
    for v in violations:
        print(f"  {v}", file=sys.stderr)
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
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  LAST_BUILD_DERIVED_DATA_PATH="$derived_data"
  mkdir -p "$TMP_BASE"
  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "$gate_name" "0")
  log "Build gate: $gate_name"
  xcodebuild \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -derivedDataPath "$derived_data" \
    ${signing_args[@]+"${signing_args[@]}"} \
    build
}

run_proposal022_app_proof() {
  local derived_data="$1"
  local stamp app_binary result_path log_path timeout_seconds pid app_status
  stamp="$(make_stamp)"
  app_binary="$derived_data/Build/Products/Debug/Chainworks Forge.app/Contents/MacOS/Chainworks Forge"
  result_path="$TMP_BASE/proposal-022-app-proof-${stamp}.json"
  log_path="$TMP_BASE/proposal-022-app-proof-${stamp}.log"
  timeout_seconds="${CHAINWORKS_P022_APP_PROOF_TIMEOUT_SECONDS:-90}"

  [[ -x "$app_binary" ]] || die "Proposal 022 app proof binary not found: $app_binary"

  log "App proof gate: proposal-022"
  rm -f "$result_path" "$log_path"

  env \
    CHAINWORKS_IN_MEMORY_STORE=1 \
    CHAINWORKS_FIXTURE_MODE=proposal022_feedback_cycle \
    CHAINWORKS_P022_APP_PROOF_AUTORUN=1 \
    CHAINWORKS_P022_APP_PROOF_RESULT_PATH="$result_path" \
    "$app_binary" >"$log_path" 2>&1 &
  pid=$!

  local deadline=$((SECONDS + timeout_seconds))
  while kill -0 "$pid" 2>/dev/null; do
    if (( SECONDS >= deadline )); then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
      printf 'Proposal 022 app proof timed out after %s seconds.\n' "$timeout_seconds" >&2
      if [[ -f "$log_path" ]]; then
        printf '--- app proof log ---\n' >&2
        cat "$log_path" >&2
      fi
      exit 1
    fi
    sleep 1
  done

  wait "$pid"
  app_status=$?
  if [[ $app_status -ne 0 ]]; then
    printf 'Proposal 022 app proof process exited with status %s.\n' "$app_status" >&2
    if [[ -f "$log_path" ]]; then
      printf '--- app proof log ---\n' >&2
      cat "$log_path" >&2
    fi
    exit 1
  fi

  [[ -f "$result_path" ]] || {
    printf 'Proposal 022 app proof did not produce result JSON at %s.\n' "$result_path" >&2
    if [[ -f "$log_path" ]]; then
      printf '--- app proof log ---\n' >&2
      cat "$log_path" >&2
    fi
    exit 1
  }

  python3 - "$result_path" <<'PY'
import json
import sys
from pathlib import Path

result_path = Path(sys.argv[1])
payload = json.loads(result_path.read_text(encoding="utf-8"))
result = payload.get("result") or {}
summary = payload.get("summary") or {}

checks = [
    (result.get("refineCorpusInputCount") == 5, "refine corpus count must be 5"),
    (result.get("reviewCorpusBundleExists") is True, "review corpus bundle must exist"),
    (result.get("reviewCorpusBundleConsumed") is True, "review corpus bundle must be consumed"),
    (result.get("scoreLiftBacklogExists") is True, "score lift backlog must exist"),
    (result.get("scoreLiftBacklogMergeProvenanceExists") is True, "merge provenance must exist"),
    (result.get("proposalFeedbackCoverageExists") is True, "proposal feedback coverage must exist"),
    (bool(result.get("unresolvedBacklogItemIDs")), "unresolved backlog items must remain visible"),
    (bool((result.get("targetedRerunRationale") or "").strip()), "targeted rerun rationale must be present"),
    ("PASS" in (result.get("proofStatus") or ""), "proof status must be PASS"),
    (summary.get("reviewCorpusBundlePresent") is True, "summary must surface review corpus bundle"),
    ((summary.get("mergeProvenanceItemCount") or 0) > 0, "summary must surface merge provenance"),
]

failed = [message for ok, message in checks if not ok]
if failed:
    print("Proposal 022 app proof validation failed:", file=sys.stderr)
    for message in failed:
        print(f"  - {message}", file=sys.stderr)
    sys.exit(1)

print(f"Proposal 022 app proof result: {result_path}")
PY
}

run_proposal015_app_proof() {
  local derived_data="$1"
  local stamp app_binary result_path log_path timeout_seconds pid app_status
  stamp="$(make_stamp)"
  app_binary="$derived_data/Build/Products/Debug/Chainworks Forge.app/Contents/MacOS/Chainworks Forge"
  result_path="$TMP_BASE/proposal-015-app-proof-${stamp}.json"
  log_path="$TMP_BASE/proposal-015-app-proof-${stamp}.log"
  timeout_seconds="${CHAINWORKS_P015_APP_PROOF_TIMEOUT_SECONDS:-90}"

  [[ -x "$app_binary" ]] || die "Proposal 015 app proof binary not found: $app_binary"

  log "App proof gate: proposal-015"
  rm -f "$result_path" "$log_path"

  env \
    CHAINWORKS_IN_MEMORY_STORE=1 \
    CHAINWORKS_P015_APP_PROOF_AUTORUN=1 \
    CHAINWORKS_P015_APP_PROOF_RESULT_PATH="$result_path" \
    "$app_binary" >"$log_path" 2>&1 &
  pid=$!

  local deadline=$((SECONDS + timeout_seconds))
  while kill -0 "$pid" 2>/dev/null; do
    if (( SECONDS >= deadline )); then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
      printf 'Proposal 015 app proof timed out after %s seconds.\n' "$timeout_seconds" >&2
      if [[ -f "$log_path" ]]; then
        printf '%s\n' '--- app proof log ---' >&2
        cat "$log_path" >&2
      fi
      exit 1
    fi
    sleep 1
  done

  wait "$pid"
  app_status=$?
  if [[ $app_status -ne 0 ]]; then
    printf 'Proposal 015 app proof process exited with status %s.\n' "$app_status" >&2
    if [[ -f "$log_path" ]]; then
      printf '%s\n' '--- app proof log ---' >&2
      cat "$log_path" >&2
    fi
    exit 1
  fi

  [[ -f "$result_path" ]] || {
    printf 'Proposal 015 app proof did not produce result JSON at %s.\n' "$result_path" >&2
    if [[ -f "$log_path" ]]; then
      printf '%s\n' '--- app proof log ---' >&2
      cat "$log_path" >&2
    fi
    exit 1
  }

  python3 - "$result_path" <<'PY'
import json
import sys
from pathlib import Path

result_path = Path(sys.argv[1])
payload = json.loads(result_path.read_text(encoding="utf-8"))
result = payload.get("result") or {}

checks = [
    (result.get("proofAgentID") == "proposal_reviewer_product_owner", "proof agent id must be proposal_reviewer_product_owner"),
    (result.get("reportSkillRef") == "proposal_review_triad", "report skill ref must be proposal_review_triad"),
    (result.get("reportSkillRole") == "product_owner", "report skill role must be product_owner"),
    (result.get("comparisonSkillRole") == "architect", "comparison skill role must be architect"),
    (result.get("primaryArtifactName") == "proposal_current", "primary artifact must be proposal_current"),
    (result.get("primaryArtifactExists") is True, "primary artifact must exist on disk"),
    (result.get("summaryMentionsSkillTruth") is True, "summary must mention skill truth"),
    (result.get("injectedSkillHashPresent") is True, "injected skill hash must be present"),
    ("PASS" in (result.get("proofStatus") or ""), "proof status must be PASS"),
]

failed = [message for ok, message in checks if not ok]
if failed:
    print("Proposal 015 app proof validation failed:", file=sys.stderr)
    for message in failed:
        print(f"  - {message}", file=sys.stderr)
    sys.exit(1)

print(f"Proposal 015 app proof result: {result_path}")
PY
}

run_test_plan() {
  local gate_name="$1"
  local plan_name="$2"

  local stamp derived_data result_bundle
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/${gate_name}-${stamp}.xcresult"
  mkdir -p "$TMP_BASE"
  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "$gate_name" "1")

  log "Test gate (test plan): $gate_name — plan=$plan_name"
  xcodebuild test \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -testPlan "$plan_name" \
    -parallel-testing-enabled NO \
    -maximum-parallel-testing-workers 1 \
    -derivedDataPath "$derived_data" \
    -resultBundlePath "$result_bundle" \
    ${signing_args[@]+"${signing_args[@]}"}
  log "Result bundle: $result_bundle"
}

run_targeted_tests() {
  local gate_name="$1"
  shift

  local stamp derived_data result_bundle log_path automation_log_path previous_automation_log_path
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/${gate_name}-${stamp}.xcresult"
  log_path="$TMP_BASE/${gate_name}-${stamp}.log"
  mkdir -p "$TMP_BASE"

  local cmd=(
    xcodebuild
    test
    -project "$PROJECT_PATH"
    -scheme "$SCHEME_NAME"
    -destination "$DESTINATION"
    -parallel-testing-enabled NO
    -maximum-parallel-testing-workers 1
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

  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "$gate_name" "$includes_ui")

  if [[ $includes_ui -eq 0 ]]; then
    cmd+=("-resultBundlePath" "$result_bundle")
    cmd+=(${signing_args[@]+"${signing_args[@]}"})
    cmd+=("-skip-testing:Chainworks ForgeUITests")
  else
    automation_log_path="$TMP_BASE/${gate_name}-${stamp}-automation.log"
    previous_automation_log_path="${CHAINWORKS_UI_AUTOMATION_LOG_PATH:-}"
    export CHAINWORKS_UI_AUTOMATION_LOG_PATH="$automation_log_path"
    cmd+=(${signing_args[@]+"${signing_args[@]}"})
    if [[ "$gate_name" != "proposal-013-ui" ]]; then
      cmd+=("-resultBundlePath" "$result_bundle")
    fi
  fi

  log "Test gate: $gate_name"
  if [[ "$gate_name" == "proposal-013-ui" || "$gate_name" == "proposal-015-ui" || "$gate_name" == "proposal-022-ui" ]]; then
    # This lane currently hangs on the approved host after it has already
    # printed a successful XCTest summary. Run it through a narrow watchdog
    # that only accepts success after the canonical pass markers.
    python3 - "$gate_name" "$log_path" "${cmd[@]}" <<'PY'
import os
import select
import signal
import subprocess
import sys
import time
from pathlib import Path

gate_name = sys.argv[1]
log_path = sys.argv[2]
cmd = sys.argv[3:]
automation_log_path = Path(os.environ.get("CHAINWORKS_UI_AUTOMATION_LOG_PATH", "/tmp/chainworks-ui-automation.log"))

def dump_automation_log():
    if not automation_log_path.exists():
        return
    try:
        lines = automation_log_path.read_text(encoding="utf-8", errors="replace").splitlines()
    except Exception as exc:
        print(f"warning: failed to read UI automation log {automation_log_path}: {exc}", file=sys.stderr)
        return

    tail = lines[-80:]
    if not tail:
        return
    print(f"--- UI automation log tail: {automation_log_path} ---", file=sys.stderr)
    for line in tail:
        print(line, file=sys.stderr)

if gate_name == "proposal-013-ui":
    marker_test_passed = "Test Case '-[Chainworks_ForgeUITests.Chainworks_ForgeUITests testProposal013AppProofSurface]' passed"
    suite_markers = ("Executed 1 test, with 0 failures", "** TEST SUCCEEDED **")
    success_label = "Proposal 013 UI watchdog"
    grace_seconds = float(os.environ.get("CHAINWORKS_P013_UI_SUCCESS_GRACE_SECONDS", "15"))
    hard_timeout_seconds = float(os.environ.get("CHAINWORKS_P013_UI_HARD_TIMEOUT_SECONDS", "1800"))
elif gate_name == "proposal-022-ui":
    marker_test_passed = "Test Case '-[Chainworks_ForgeUITests.Chainworks_ForgeUITests testProposal022AppProofSurface]' passed"
    suite_markers = ("Executed 1 test, with 0 failures", "** TEST SUCCEEDED **")
    success_label = "Proposal 022 gate watchdog"
    grace_seconds = float(os.environ.get("CHAINWORKS_P022_UI_SUCCESS_GRACE_SECONDS", "10"))
    hard_timeout_seconds = float(os.environ.get("CHAINWORKS_P022_UI_HARD_TIMEOUT_SECONDS", "1800"))
elif gate_name == "proposal-015-ui":
    marker_test_passed = "Test Case '-[Chainworks_ForgeUITests.Chainworks_ForgeUITests testProposal015SkillVisibilityProofSurface]' passed"
    suite_markers = ("Executed 1 test, with 0 failures", "** TEST SUCCEEDED **")
    success_label = "Proposal 015 UI watchdog"
    grace_seconds = float(os.environ.get("CHAINWORKS_P015_UI_SUCCESS_GRACE_SECONDS", "15"))
    hard_timeout_seconds = float(os.environ.get("CHAINWORKS_P015_UI_HARD_TIMEOUT_SECONDS", "1800"))
else:
    raise SystemExit(f"unsupported watchdog gate: {gate_name}")

test_passed = False
suite_passed = False
success_at = None
start = time.time()
known_failure_markers = (
    "before establishing connection",
    "Early unexpected exit",
    "signal kill",
)

with open(log_path, "w", encoding="utf-8") as log:
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        start_new_session=True,
    )
    try:
        while True:
            ready, _, _ = select.select([proc.stdout], [], [], 0.5)
            line = proc.stdout.readline() if ready else ""
            if line:
                sys.stdout.write(line)
                sys.stdout.flush()
                log.write(line)
                log.flush()
                if marker_test_passed in line:
                    test_passed = True
                if any(marker in line for marker in suite_markers):
                    suite_passed = True
                if any(marker in line for marker in known_failure_markers):
                    print(f"error: {success_label} saw known launch failure marker", file=sys.stderr)
                    dump_automation_log()
                    try:
                        os.killpg(proc.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    time.sleep(2)
                    if proc.poll() is None:
                        try:
                            os.killpg(proc.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    raise SystemExit(65)
                if test_passed and suite_passed and success_at is None:
                    success_at = time.time()
                continue

            if proc.poll() is not None:
                if proc.returncode != 0:
                    dump_automation_log()
                raise SystemExit(proc.returncode)

            now = time.time()
            if success_at is not None and now - success_at >= grace_seconds:
                try:
                    os.killpg(proc.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                time.sleep(2)
                if proc.poll() is None:
                    try:
                        os.killpg(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                print(f"==> {success_label}: xcodebuild hung after successful proof; terminating stale process and accepting gate")
                raise SystemExit(0)

            if now - start >= hard_timeout_seconds:
                try:
                    os.killpg(proc.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                time.sleep(2)
                if proc.poll() is None:
                    try:
                        os.killpg(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                dump_automation_log()
                print(f"error: {success_label} hit hard timeout before success markers", file=sys.stderr)
                raise SystemExit(124)

    finally:
        try:
            proc.stdout.close()
        except Exception:
            pass
PY
    log "Watchdog log: $log_path"
    local derived_result
    derived_result="$(find "$derived_data/Logs/Test" -name '*.xcresult' -print 2>/dev/null | sort | tail -1 || true)"
    if [[ -n "$derived_result" ]]; then
      log "Result bundle: $derived_result"
    fi
  else
    "${cmd[@]}"
    log "Result bundle: $result_bundle"
  fi

  if [[ $includes_ui -eq 1 ]]; then
    if [[ -n "$previous_automation_log_path" ]]; then
      export CHAINWORKS_UI_AUTOMATION_LOG_PATH="$previous_automation_log_path"
    else
      unset CHAINWORKS_UI_AUTOMATION_LOG_PATH
    fi
  fi
}

run_split_targeted_gate() {
  local gate_name="$1"
  shift

  local non_ui_tests=()
  local ui_tests=()
  local test_id
  for test_id in "$@"; do
    if [[ "$test_id" == Chainworks\ ForgeUITests/* ]]; then
      ui_tests+=("$test_id")
    else
      non_ui_tests+=("$test_id")
    fi
  done

  if [[ ${#non_ui_tests[@]} -gt 0 ]]; then
    run_targeted_tests "${gate_name}-non-ui" "${non_ui_tests[@]}"
  fi

  if [[ ${#ui_tests[@]} -gt 0 ]]; then
    run_targeted_tests "${gate_name}-ui" "${ui_tests[@]}"
  fi
}

run_full_suite() {
  local stamp derived_data result_bundle
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/full-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/full-${stamp}.xcresult"
  mkdir -p "$TMP_BASE"
  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "full" "1")

  log "Full gate: xcodebuild test"
  xcodebuild \
    test \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -parallel-testing-enabled NO \
    -maximum-parallel-testing-workers 1 \
    -derivedDataPath "$derived_data" \
    -resultBundlePath "$result_bundle" \
    ${signing_args[@]+"${signing_args[@]}"}
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
  proposal-013    Proposal 013 contract/evidence/recovery gate
  proposal-014    Proposal 014 design-system and brand adoption gate
  proposal-015    Proposal 015 skill resolution and runtime injection gate
  proposal-018    Proposal 018 session lineage reuse and operator reset gate
  proposal-019    Proposal 019 context-strategy framework gate
  proposal-022    Proposal 022 feedback fidelity score lift and rereview proof gate
  proposal-024    Proposal 024 run-surface information architecture gate
  proposal-025    Proposal 025 per-agent MCP policy and runtime validation gate
  proposal-026    Proposal 026 ACP-first runtime transport and Goose decoupling gate
  proposal-027    Proposal 027 Rust+SQLite local control-plane extraction gate
  proposal-027r   Proposal 027 unified read-only JSON/markdown rendering gate (legacy renderer)
  proposal-029    Proposal 029 second-wave ACP runtime profiles gate
  proposal-032    Proposal 032 atomic transition settlement and durable resume cursor gate
  proposal-033    Proposal 033 ACP-only runtime architecture gate
  proposal-037    Proposal 037 ACP execution supervision and idle watchdog gate
  proposal-044    Proposal 044 post-approval task execution and release gate completion gate
  proposal-045    Proposal 045 deterministic release operations gate
  proposal-047    Proposal 047 control-plane workspace verification gate
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

if should_wrap_gate_in_terminal_gui_session "$GATE"; then
  run_gate_in_terminal_gui_session "$GATE"
  exit $?
fi

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
    if [[ "${USE_TEST_PLANS:-}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/ProviderGate.xctestplan" ]]; then
      run_test_plan "proposal-006" "ProviderGate"
    else
      run_targeted_tests "proposal-006" "${PROPOSAL_006_TESTS[@]}"
    fi
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
  proposal-013|p013)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_split_targeted_gate "proposal-013" "${PROPOSAL_013_TESTS[@]}"
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
  proposal-015|p015)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-015"
    run_targeted_tests "proposal-015-non-ui" "${PROPOSAL_015_NON_UI_TESTS[@]}"
    run_proposal015_app_proof "$LAST_BUILD_DERIVED_DATA_PATH"
    ;;
  proposal-018|p018)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-018"
    run_targeted_tests "proposal-018" "${PROPOSAL_018_TESTS[@]}"
    ;;
  proposal-019|p019)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-019"
    run_targeted_tests "proposal-019" "${PROPOSAL_019_TESTS[@]}"
    ;;
  proposal-022|p022)
    check_idle_environment strict
    require_remote_ui_host
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-022"
    run_split_targeted_gate "proposal-022" "${PROPOSAL_022_TESTS[@]}"
    run_proposal022_app_proof "$LAST_BUILD_DERIVED_DATA_PATH"
    ;;
  proposal-024|p024)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-024"
    run_split_targeted_gate "proposal-024" "${PROPOSAL_024_TESTS[@]}"
    ;;
  proposal-025|p025)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    guard_portability_paths
    run_build "proposal-025"
    run_targeted_tests "proposal-025" "${PROPOSAL_025_TESTS[@]}"
    ;;
  proposal-026|p026)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-026"
    run_targeted_tests "proposal-026" "${PROPOSAL_026_TESTS[@]}"
    ;;
  proposal-027|p027)
    log "Proposal 027 control-plane gate: Rust+SQLite daemon test suite"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test --workspace 2>&1
    )
    log "Proposal 027 control-plane gate passed"
    ;;
  proposal-027r|p027r)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-027r"
    run_targeted_tests "proposal-027r" "${PROPOSAL_027_TESTS[@]}"
    ;;
  proposal-029|p029)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-029"
    run_targeted_tests "proposal-029" "${PROPOSAL_029_TESTS[@]}"
    ;;
  proposal-032|p032)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-032"
    run_targeted_tests "proposal-032" "${PROPOSAL_032_TESTS[@]}"
    ;;
  proposal-033|p033)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    log "Prerequisite: proposal-029 gate (second-wave ACP)"
    run_targeted_tests "proposal-029-prereq" "${PROPOSAL_029_TESTS[@]}"
    run_build "proposal-033"
    run_targeted_tests "proposal-033" "${PROPOSAL_033_TESTS[@]}"
    ;;
  proposal-037|p037)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-037"
    run_split_targeted_gate "proposal-037" "${PROPOSAL_037_TESTS[@]}"
    ;;
  proposal-044|p044)
    log "Proposal 044 control-plane gate: post-approval + N-phase + end-state"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test --workspace 2>&1
    )
    log "Proposal 044 control-plane gate passed"
    ;;
  proposal-045|p045)
    log "Proposal 045 control-plane gate: deterministic release operations"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p engine --test integration test_start_run_persists_delivery_configuration_json -- --exact --nocapture &&
      cargo test -p engine --test release -- --nocapture &&
      cargo test -p graphql-server -- --nocapture &&
      cargo test -p mcp-server -- --nocapture
    )
    log "Proposal 045 control-plane gate passed"
    ;;
  proposal-047|p047)
    log "Proposal 047 control-plane gate: Rust workspace test suite"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test --workspace 2>&1
    )
    log "Proposal 047 control-plane gate passed"
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
    if [[ "${USE_TEST_PLANS:-1}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/FullGate.xctestplan" ]]; then
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
