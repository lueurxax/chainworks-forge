# Proposal 005: Operator Experience Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md` |
| Repository Root | `.` |
| Git SHA | `c62515f` |
| Working Tree | Dirty |
| Audited At | `2026-03-24T08:53:08+0200` |
| Proposal State | Active |
| Overall Status | Not Implemented |

## Verdict

`P005-OPS` is not fully implemented on current HEAD. The operator spine is materially present: `RunsHomeView`, `RunReportBuilder`, `RecoveryCoordinator`, `RunComparisonService`, `RunComparisonView`, `RecoverySheet`, `NotificationService`, and menu bar wiring all exist, and the focused live/recovery baseline built and passed in this audit. But at least one in-scope requirement is still missing: row-level contextual actions in `RunsHomeRow` are rendered as no-op placeholders and can still promise dead-end actions. Several other proposal surfaces are only partially closed, notably `RunReportView` reachability, clone recovery from the surfaced UI, `ArtifactInspectorV2` wiring, approval notifications, and the explicit full-suite sign-off gate.

## Proposal Contract

### Scope

- Add the first operator spine for the current proposal-loop live baseline: `RunsHomeView`, `RunReportBuilder`, `RecoveryCoordinator`, `RunComparisonService`, `ArtifactInspectorV2`, and `NotificationService`.
- Keep operator scope anchored to the Proposal 004 live baseline plus `P005-TRANSPORT`, without pulling Proposal 007 repo-backed/release behavior forward.
- Make runtime trust and provenance explicit across runs home, reports, comparison, and recovery.

### Locked Decisions

- `P005-OPS` is the operator proposal and must not be confused with `P005-TRANSPORT`.
- Reports are immutable history plus mutable latest summary.
- Runtime trust/provenance must be visible rather than inferred.
- Row actions are contextual, never universal promises.
- Repo-backed and release recovery stay out of scope here.

### Acceptance Criteria

- `RunsHomeView` is the primary operator landing surface, grouped into `Waiting Approval`, `Blocked`, `Running`, and `Recently Completed`.
- Rows show stage, elapsed time, cost, attention level, and runtime trust/provenance; no row advertises an action that cannot be executed from that row.
- Stable checkpoints emit immutable `run_report_v{n}.md/json`; latest summary is separate; recovery never overwrites historical report state.
- Recovery supports proposal-loop retry/re-arm/clone actions and exposes only actions allowed for the current run type.
- Comparison works for compatible proposal-loop runs and does not imply repo-backed/release diff support.
- Artifact Inspector V2 renders supported formats, shows provenance and traceability, and supports pinning plus reveal-on-disk actions.
- Notifications cover approval, blocked, failed, and completed states; dock badge and optional menu bar are supported.
- No Proposal 001/002/003/004 runtime regressions; targeted Proposal 004 + transport baseline must compile/pass before this slice starts.
- `xcodebuild build && xcodebuild test` must be green before sign-off.
- One engineer can understand what happened and recover a proposal-loop blocked/failed run without raw files or database edits.

### Test / Evidence Requirements

- Focused Apple-platform build/test proof on the live/recovery baseline.
- Sign-off gate includes green `xcodebuild build && xcodebuild test`.
- Product checkpoint is behavioral, not just static code presence.

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
| Partially Implemented | 7 |
| Missing | 1 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Runs Home is the primary operator landing surface with the required sections
- Proposal Source: `§5.1-§5.2` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:83-99`, `:418-419`)
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/ContentView.swift:6-23`
  - `Chainworks Forge/Views/RunsHomeView.swift:21-84`
  - `xcodebuild ... test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface'` (passed)
- Gap / Note: The app does default to `Runs Home`, and the grouped sections are implemented in the view tree.

### REQ-002 Runs Home rows show operator metadata, attention, and runtime trust
- Proposal Source: `§5.3` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:100-120`, `:420`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:172-233`
  - `Chainworks Forge/Views/RunsHomeView.swift:314-356`
- Gap / Note: The badge labels match the proposal’s `fixture_verified`, `server_unverified`, and `server_verified` trust model.

### REQ-003 Row actions are contextual and executable from the row they appear on
- Proposal Source: `§4`, `§5.4` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:75-79`, `:122-132`, `:421`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:238-261`
  - `Chainworks Forge/Views/RunsHomeView.swift:300-308`
  - `Chainworks Forge/Views/RunsHomeView.swift:438-439`
  - `rg -n "RunReportView\\(" 'Chainworks Forge'` returned no instantiation sites during this audit
- Gap / Note: The row context-menu actions `Open`, `Open Gate`, `Recover`, `Compare`, and `View Report` are no-op closures today. `hasCompatibleComparisonTargets` also treats any sibling run as comparable, and the detail-panel `View Report` navigation path has no live destination wiring to `RunReportView`. That violates the proposal’s “no dead-end actions” rule.

### REQ-004 Stable checkpoints emit immutable reports plus a separate latest summary
- Proposal Source: `§6.2-§6.3` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:142-168`, `:425-427`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:20-113`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:117-124`
  - `Chainworks Forge/Engine/ExecutionService.swift:150-155`
- Gap / Note: Immutable history versions are append-only and latest summary files are written separately. The latest summary JSON file is written to disk but is not tracked by its own `Artifact` row, which is a modeling limitation but not enough to erase the underlying separation behavior.

### REQ-005 Report payload includes runtime trust/provenance and drift notes
- Proposal Source: `§6.4-§6.5` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:170-248`, `:428`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:193-220`
  - `Chainworks Forge/Models/Run.swift:35-41`
  - `Chainworks Forge/Models/Artifact.swift:18-26`
- Gap / Note: The generated payload includes workflow/catalog hashes, runtime trust, drift note, approvals, stage timeline, and key artifacts.

### REQ-006 The operator can read immutable history versus latest summary in the UI
- Proposal Source: `§6.2` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:153-158`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift:8-117`
  - `rg -n "RunReportView\\(" 'Chainworks Forge'` returned no live usage during this audit
- Gap / Note: `RunReportView` has the correct segmented UI and trust labeling, but the current app shell does not route into it. The surfaced `View Report` affordance therefore does not yet close the proposal contract.

### REQ-007 Proposal-loop recovery supports retry, re-arm, and clone flows
- Proposal Source: `§7.1-§7.2` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:252-272`, `:432`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:23-57`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:61-220`
  - `Chainworks Forge/Views/RecoverySheet.swift:119-132`
  - `xcodebuild ... test -only-testing:'Chainworks ForgeTests/ResumeManagerTests/testExecutionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testApprovalGrantedResumesExecution'` (passed)
- Gap / Note: Core recovery methods exist, but surfaced clone actions still stop at explanatory error text in `RecoverySheet`, and there is no current operator entry point that exposes `Resume from Approval Gate` as a recovery action for waiting-approval rows.

### REQ-008 Recovery UI exposes only safe in-scope actions and no repo/release recovery
- Proposal Source: `§7.3-§7.4` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:274-295`, `:433-434`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:23-57`
  - `Chainworks Forge/Views/RecoverySheet.swift:5-10`
  - `Chainworks Forge/Views/RecoverySheet.swift:65-80`
- Gap / Note: The surfaced recovery sheet limits actions to proposal-loop safe actions and does not advertise repo-write or release recovery.

### REQ-009 Comparison works for compatible proposal-loop runs and shows the required deltas without repo-backed claims
- Proposal Source: `§8.1-§8.3` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:298-332`, `:438-440`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunComparisonService.swift:17-113`
  - `Chainworks Forge/Views/RunComparisonView.swift:8-163`
  - `Chainworks Forge/Views/RunsHomeView.swift:498-518`
- Gap / Note: The deterministic comparison service and UI are present, but the picker and row gating still admit any other run for the same idea rather than filtering to truly compatible targets. The resulting “Incompatible Runs” fallback is still a dead-end that the proposal explicitly tried to avoid.

### REQ-010 Artifact Inspector V2 renders the required formats and shows provenance/trust
- Proposal Source: `§9.1-§9.2` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:334-352`, `:444-445`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:14-107`
  - `Chainworks Forge/Views/IdeaListView.swift:1224-1283`
  - `xcodebuild ... test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testArtifactInspectorOpensProposalAndReceiptArtifacts' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testArtifactInspectorViewSurface'` (`TEST SUCCEEDED`, but both UI tests were skipped in this environment)
- Gap / Note: `ArtifactInspectorView` contains the intended V2 presentation, but the app still routes to `WorkflowArtifactInspectorView`, which only renders content and a small header. Provenance chips and trust display are not currently wired into the surfaced inspector.

### REQ-011 Artifact Inspector V2 supports traceability, pinning, and open-on-disk actions
- Proposal Source: `§9.3-§9.5` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:354-365`, `:446-447`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:113-218`
  - `Chainworks Forge/Views/IdeaListView.swift:1224-1313`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:430`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:575`
- Gap / Note: The standalone V2 view has traceability, pin/unpin, and open actions, but the surfaced inspector does not. The current “consumed-by” implementation in `ArtifactInspectorView` is also not true downstream-consumer traceability; it just replays the producing agent’s stored input names.

### REQ-012 Notifications cover approval, blocked, failed, and completed states, with badge/menu-bar presence
- Proposal Source: `§10` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:368-383`, `:451-453`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/NotificationService.swift:8-99`
  - `Chainworks Forge/Engine/ExecutionService.swift:139-142`
  - `Chainworks Forge/Engine/ExecutionService.swift:428-447`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:35-39`
  - `Chainworks Forge/Views/MenuBarStatusView.swift:8-43`
- Gap / Note: Blocked/failed/completed notifications and menu bar support are wired. Approval-required notifications are not: `onApprovalRequest` only stores the request, and dock badge refresh currently happens on completion notification paths rather than when approval is first requested.

### REQ-013 The targeted live/recovery baseline still compiles and passes
- Proposal Source: `§12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:455-458`)
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-ops-audit.0zTFKq build` (`BUILD SUCCEEDED`)
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-ops-audit-tests.tQ9V4e test -only-testing:'Chainworks ForgeTests/ResumeManagerTests/testExecutionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testApprovalGrantedResumesExecution' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testArtifactInspectorOpensProposalAndReceiptArtifacts' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testArtifactInspectorViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance'` (`TEST SUCCEEDED`)
  - `xcresult: /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-ops-audit-tests.tQ9V4e/Logs/Test/Test-Chainworks Forge-2026.03.24_08-48-41-+0200.xcresult`
- Gap / Note: The focused baseline is green, but two artifact-inspector UI tests were skipped because tabs were not discoverable in this environment.

### REQ-014 Full `xcodebuild build && xcodebuild test` is green before sign-off
- Proposal Source: `§12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:459`)
- Status: Not Verifiable
- Evidence Type: tests-run
- Evidence:
  - `xcodebuild ... build` was run and passed in this audit
  - No full `xcodebuild ... test` run was executed in this audit; only the focused baseline above was run
- Gap / Note: This audit does not contain a full-suite green proof, so the explicit sign-off gate remains open.

### REQ-015 One engineer can understand run state and recover safely without raw files or DB edits
- Proposal Source: `§12 Product checkpoint` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:461-468`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:21-123`
  - `Chainworks Forge/Views/RecoverySheet.swift:18-141`
  - `xcodebuild ... test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance'` (passed)
- Gap / Note: The app now gives a much stronger operator picture than the earlier scaffold, but there is no timed usability proof in this audit, `RunReportView` is not yet surfaced, and surfaced clone recovery still stops at “use from Run context” errors.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "struct RunsHomeView|struct RunReportView|struct RunComparisonView|struct RecoverySheet|struct ArtifactInspectorView|class NotificationService|MenuBarExtra|latestImmutableReportArtifactID|latestSummaryArtifactID|func shouldEmitReport|cloneRunFrozenSnapshot|cloneRunCurrentConfig|func retryAgent|func retryStage|resumeFromApprovalGate|isProposalLoopReadOnly|extractBindings|consumedInputArtifactNamesJSON|reportKind|supersedesArtifactID" 'Chainworks Forge'`
- `rg -n "test.*RunReport|test.*Comparison|test.*Recovery|test.*ArtifactInspector|test.*Resume|test.*Notification|test.*MenuBar|test.*RunsHome|test.*ApprovalInbox|test.*RunProgress" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `rg -n "RunReportView\\(" 'Chainworks Forge'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-ops-audit.0zTFKq build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/proposal-ops-audit-tests.tQ9V4e test -only-testing:'Chainworks ForgeTests/ResumeManagerTests/testExecutionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testApprovalGrantedResumesExecution' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testArtifactInspectorOpensProposalAndReceiptArtifacts' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testArtifactInspectorViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance'`

## Recommended Next Actions

- Wire real handlers for row-level `Open`, `Open Gate`, `Recover`, `Compare`, and `View Report`, and gate `Compare` on true compatibility rather than “any sibling run.”
- Route the app shell into `RunReportView` and the full `ArtifactInspectorView`, or fold their missing capabilities into the currently surfaced views.
- Finish clone execution from `RecoverySheet` and wire approval-required notifications/dock badge refresh at approval-request time, not only at completion time.
- Run and record one clean full-suite `xcodebuild test` pass before claiming the proposal signed off.
