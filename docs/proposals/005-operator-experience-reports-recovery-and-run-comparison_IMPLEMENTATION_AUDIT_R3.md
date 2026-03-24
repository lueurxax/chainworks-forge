# Proposal 005: Operator Experience Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md` |
| Repository Root | `.` |
| Git SHA | `e63d440` |
| Working Tree | Dirty |
| Audited At | `2026-03-24T21:57:36+0200` |
| Proposal State | Active |
| Overall Status | Not Implemented |

## Verdict

`P005-OPS` is materially stronger than it was in R2. The operator spine is now genuinely present in the app shell: `RunsHomeView` is the landing surface, `RunReportView` is surfaced, `RecoverySheet` is wired, `RunComparisonView` renders structural deltas plus bindings, `ArtifactInspectorView` is live, and the proposal's notification/presence layer now includes dock badge, menu bar, and foreground banner wiring. But the proposal still does not close as fully implemented on the current repository state. Two feature-level contract gaps remain partial: report/comparison "model" provenance is still sourced from `resolvedBackendProfileID` instead of the resolved model field, and artifact traceability still uses `consumedInputArtifactNamesJSON` rather than the proposal's `inputBindingsJSON` semantics. More importantly, the proposal's explicit sign-off gate is not closed on the current tree: a fresh full-scheme `xcodebuild test` rerun failed after unrelated dirty-worktree changes in `Chainworks Forge/Support/SettingsTransferService.swift` during this audit window, so the repo is not currently in a sign-off-ready state.

## Proposal Contract

### Scope

- Add the first operator spine for the current proposal-loop live baseline: `RunsHomeView`, `RunReportBuilder`, `RecoveryCoordinator`, `RunComparisonService`, `ArtifactInspectorV2`, and `NotificationService`.
- Keep operator scope anchored to the current Proposal 004 baseline and `P005-TRANSPORT`, without pulling Proposal 007 repo-backed implementation or release behavior forward.
- Make runtime trust/provenance explicit across runs home, reports, comparison, recovery, and artifact inspection.

### Locked Decisions

- `P005-OPS` is the operator proposal and must not be confused with `P005-TRANSPORT`.
- Reports use immutable history plus mutable latest summary semantics.
- Runtime trust/provenance must be visible rather than inferred.
- Row actions are contextual, never universal promises.
- Repo-backed and release recovery remain out of scope.

### Acceptance Criteria

- `RunsHomeView` is the primary operator landing surface, grouped into `Waiting Approval`, `Blocked`, `Running`, and `Recently Completed`.
- Rows show stage, elapsed time, cost, attention level, and runtime trust/provenance; no row advertises an action that cannot be executed from that row.
- Stable checkpoints emit immutable `run_report_v{n}.md/json`; latest summary is separate; recovery never overwrites historical report state.
- Proposal-loop recovery supports retry/re-arm/clone actions and exposes only actions allowed for the current run type.
- Comparison works for compatible proposal-loop runs and does not imply repo-backed/release diff support.
- Artifact Inspector V2 renders supported formats, shows provenance and traceability, and supports pinning plus reveal-on-disk actions.
- Notifications cover approval, blocked, failed, and completed states; dock badge and optional menu bar are supported.
- No Proposal 001/002/003/004 runtime or UI tests regress.
- The targeted live/recovery baseline from Proposal 004 and `P005-TRANSPORT` compiles and passes before `P005-OPS` implementation starts.
- `xcodebuild build && xcodebuild test` is green before sign-off.
- One engineer can understand what happened and recover a proposal-loop blocked/failed run without raw files or database edits.

### Test / Evidence Requirements

- Evidence-first proof against the current macOS app, not just code presence.
- Strongest practical proof is a fresh `xcodebuild` run with external DerivedData plus current-head UI/runtime validation.
- The explicit sign-off gate is a green full-scheme `xcodebuild build && xcodebuild test`.

### Explicit Exclusions

- Writable worktrees.
- Repo-backed implementation runs.
- Git commit / push / release recovery.
- Publish / distribution recovery.
- Release receipts and release comparison.
- Semantic LLM-written reports.
- Shared team/cloud inbox behavior.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 4 |
| Missing | 2 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Runs Home is the primary operator landing surface with the required grouped sections
- Proposal Source: `Section 5.1-5.2`, `Section 12 Runs Home` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:81-99`, `:414-417`)
- Status: Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - Fresh full-scheme `xcodebuild ... test` run reached and passed `Chainworks_ForgeUITests.testApprovalInboxReachable`
- Gap / Note: `RunsHomeView` is the default landing tab and groups runs into `Waiting Approval`, `Blocked`, `Running`, and `Recently Completed`.

### REQ-002 Runs Home rows show operator metadata, trust, and only executable contextual actions
- Proposal Source: `Section 4`, `Section 5.3-5.4`, `Section 12 Runs Home` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:73-79`, `:100-132`, `:418-421`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
- Gap / Note: Rows now expose real status-gated actions only: `Open`, `Open gate`, `Recover`, `Compare`, and `View report` are shown only when the action can actually execute.

### REQ-003 Reports use immutable-history plus latest-summary semantics, and the operator can read that distinction in the UI
- Proposal Source: `Section 6.2-6.3`, `Section 12 Reports` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:140-168`, `:423-426`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Models/Run.swift`
- Gap / Note: Immutable report artifacts are versioned and append-only, latest summary is separate, and the surfaced UI labels the trust difference between immutable history and mutable latest summary.

### REQ-004 Report payload includes the deterministic operator narrative defined in the proposal
- Proposal Source: `Section 6.4-6.5` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:170-248`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Engine/ExecutionReceiptBuilder.swift`
- Gap / Note: Retry path, resume path, retries performed, recovery actions taken, runtime trust, and drift notes are now present. The remaining contract gap is model provenance quality: `RunReportBuilder` still emits the "model" field from `AgentExecution.resolvedBackendProfileID` instead of the resolved model field (`resolvedModel`) or a declared fallback hierarchy, so the report's provider/model/effort record is not fully faithful to the runtime contract.

### REQ-005 Recovery supports in-scope proposal-loop actions and exposes only safe actions for the current run type
- Proposal Source: `Section 7.1-7.4`, `Section 12 Recovery` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:250-295`, `:428-431`)
- Status: Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - Fresh full-scheme `xcodebuild ... test` run passed `Chainworks_ForgeUITests.testWaitingApprovalRunIsRestoredOnLaunch`
- Gap / Note: Retry, re-arm, frozen clone, and current-config clone flows are wired through the current app shell and remain constrained to read-only proposal-loop run types.

### REQ-006 Comparison works for compatible proposal-loop runs and shows the proposal's required comparison dimensions
- Proposal Source: `Section 8.1-8.3`, `Section 12 Comparison` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:296-332`, `:435-438`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunComparisonService.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
- Gap / Note: True compatibility gating and the rendered comparison UI now exist, including snapshot, trust, timing, cost, stage, approval, pinned-artifact, and bindings sections. The remaining gap is the same provenance quality issue as REQ-004: the rendered "model" binding is populated from `resolvedBackendProfileID` rather than `resolvedModel`, so provider/model/effort comparison is only partially faithful.

### REQ-007 Artifact Inspector V2 renders the required formats, shows provenance, supports pin/open actions, and provides traceability
- Proposal Source: `Section 9`, `Section 12 Artifact Inspector V2` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:334-366`, `:441-444`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - Fresh full-scheme `xcodebuild ... test` run passed `Chainworks_ForgeUITests.testArtifactInspectorOpensProposalAndReceiptArtifacts`
  - Fresh full-scheme `xcodebuild ... test` run skipped `Chainworks_ForgeUITests.testArtifactInspectorViewSurface` in headless toolbar conditions
- Gap / Note: The surfaced inspector now supports format-aware rendering, provenance chips, pin/unpin, and open-on-disk actions. The remaining traceability gap is semantic: downstream consumers are derived from `consumedInputArtifactNamesJSON`, while the proposal defines traceability via consuming attempts and `inputBindingsJSON`.

### REQ-008 Notifications and presence cover approval, blocked, failed, completed, dock badge, menu bar, and foreground presence
- Proposal Source: `Section 10`, `Section 12 Notifications` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:368-383`, `:448-450`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/NotificationService.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Views/MenuBarStatusView.swift`
  - `Chainworks Forge/Views/ForegroundBannerView.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/ContentView.swift`
- Gap / Note: Approval, blocked, failed, and completed notification paths are wired; dock badge refresh is wired on attention changes; menu bar presence exists; and the active-app foreground banner promised by the proposal is now implemented.

### REQ-009 No surfaced operator flow implies repo-write, implementation, release, or publish capability before Proposal 007
- Proposal Source: `Section 4`, `Section 7.2-7.3`, `Section 8.3`, `Section 9`, `Section 13`, `Section 14 OPS-056`, `Section 15` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:73-79`, `:262-284`, `:321-366`, `:454-483`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
- Gap / Note: Current operator surfaces stay inside the read-only proposal-loop boundary and do not claim repo-backed, git, publish, or release capabilities.

### REQ-010 No Proposal 001 / 002 / 003 / 004 runtime or UI tests regress
- Proposal Source: `Section 12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:452-454`)
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p005ops-audit-r3b-dd.NPL89K test`
  - `Chainworks Forge/Support/SettingsTransferService.swift`
  - `Testing failed: Generic parameter 'ElementOfResult' could not be inferred`
- Gap / Note: A fresh full-scheme rerun on the latest dirty tree failed before test execution completed because `Chainworks Forge/Support/SettingsTransferService.swift` changed during the audit window (`mtime 2026-03-24 21:53:18 +0200`) and no longer compiled. The earlier pre-drift run demonstrated the inherited Proposal 001/002/003/004 UI baseline with `0` UI failures, but the current repository state does not close the "no regressions" gate.

### REQ-011 The targeted live/recovery baseline from Proposal 004 and `P005-TRANSPORT` compiled and passed before `P005-OPS` implementation started
- Proposal Source: `Section 12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:455`)
- Status: Not Verifiable
- Evidence Type: tests-run, runtime
- Evidence:
  - Fresh full-scheme pre-drift run reached and passed `Chainworks_ForgeUITests.testLiveProposalLoopFixtureFlowReachesApprovalAndCompletion`
  - Fresh full-scheme pre-drift run reached and passed `Chainworks_ForgeUITests.testWaitingApprovalRunIsRestoredOnLaunch`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
- Gap / Note: Current HEAD still shows a working live/recovery baseline, but the proposal's wording is historical ("before implementation starts"). That sequencing requirement cannot be proven retroactively from the repository alone.

### REQ-012 Full `xcodebuild build && xcodebuild test` is green before sign-off
- Proposal Source: `Section 12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:456`)
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p005ops-audit-r3-dd.Kx9m2U -resultBundlePath /tmp/codex-p005ops-audit-r3.xcresult test`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p005ops-audit-r3b-dd.NPL89K test`
  - `/tmp/codex-p005ops-audit-r3b-dd.NPL89K/Logs/Test/Test-Chainworks Forge-2026.03.24_21-55-57-+0200.xcresult`
- Gap / Note: The first fresh run reached `Executed 13 tests, with 3 tests skipped and 0 failures` for the UI bundle but `xcodebuild` did not terminate cleanly while finalizing the explicit result bundle. A second fresh rerun without `-resultBundlePath` exited with `EXIT_CODE=65` after the unrelated `SettingsTransferService.swift` compile failure. On the current repo snapshot, the explicit sign-off gate is not green.

### REQ-013 One engineer can understand run state quickly and recover a blocked/failed run without raw files or database edits
- Proposal Source: `Section 12 Product checkpoint` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:458-463`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - Fresh full-scheme pre-drift run passed `Chainworks_ForgeUITests.testProductCheckpointExecutionFlowReachable`
  - Fresh full-scheme pre-drift run passed `Chainworks_ForgeUITests.testProductCheckpointScaffoldFlowUnder60Seconds`
  - Fresh full-scheme pre-drift run skipped `Chainworks_ForgeUITests.testFullProductCheckpointCanonicalExecution` in headless toolbar conditions
- Gap / Note: The operator spine is now strong enough that a human can plausibly inspect and recover many proposal-loop runs from the UI, and the main execution checkpoint test now passes. The remaining gap is proof quality: the end-to-end canonical checkpoint remains skipped in headless macOS, so the full "under 30 seconds" product story is still not conclusively closed in this audit.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" 'docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md' 'docs/reviews'`
- `sed -n '1,560p' 'docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md'`
- `rg -n "RunReportView|RunsHomeView|RecoverySheet|ArtifactInspectorView|MenuBarStatusView|ForegroundBannerView|RunComparisonView|testProductCheckpointExecutionFlowReachable|testApprovalInboxReachable|testRunProgressViewSurface|testStageDetailViewSurface|testArtifactInspectorViewSurface|ResumeManagerTests|retryPath|resumePath|consumedInputArtifactNamesJSON|inputBindingsJSON" 'Chainworks Forge' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p005ops-audit-r3-dd.Kx9m2U -resultBundlePath /tmp/codex-p005ops-audit-r3.xcresult test`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-p005ops-audit-r3b-dd.NPL89K test`
- `stat -f 'mtime=%Sm' -t '%Y-%m-%d %H:%M:%S %z' 'Chainworks Forge/Support/SettingsTransferService.swift'`

## Recommended Next Actions

- Correct report/comparison model provenance to use `AgentExecution.resolvedModel` or an explicit fallback contract instead of `resolvedBackendProfileID`.
- Replace artifact downstream-consumer derivation with the proposal's intended traceability source rather than `consumedInputArtifactNamesJSON`.
- Stabilize the current dirty tree and rerun a clean full-scheme `xcodebuild build && xcodebuild test` after the unrelated settings-transfer compile break is fixed.
- If the product checkpoint must be treated as fully closed, add a reliable non-skipped proof for the canonical operator checkpoint on macOS CI/headless execution.
