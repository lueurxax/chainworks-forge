# Proposal 005: Operator Experience Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md` |
| Repository Root | `.` |
| Git SHA | `e63d440` |
| Working Tree | Dirty |
| Audited At | `2026-03-24T20:30:12+02:00` |
| Proposal State | Active |
| Overall Status | Not Implemented |

## Verdict

`P005-OPS` is materially closer to complete than the R1 audit: the operator spine is now genuinely wired through the app shell, with executable row actions in `RunsHomeView`, surfaced `RunReportView`, surfaced `ArtifactInspectorView`, live `RecoverySheet` clone flows, approval notifications, dock-badge refresh, and menu-bar presence. But the proposal still does not close as implemented on current HEAD. The strongest blocker is the explicit sign-off gate: a fresh full-scheme `xcodebuild test` run failed in `Chainworks_ForgeUITests.testProductCheckpointExecutionFlowReachable`, so the proposal’s own green `build && test` contract is not met. Beyond that, several contract details remain only partially implemented: reports still omit retry/resume path details, comparison does not render provider/model/effort bindings, artifact traceability is not true downstream-consumer traceability, and the proposal’s foreground-banner notification surface is still absent.

## Proposal Contract

### Scope

- Add the first operator spine for the current proposal-loop live baseline: `RunsHomeView`, `RunReportBuilder`, `RecoveryCoordinator`, `RunComparisonService`, `ArtifactInspectorV2`, and `NotificationService`.
- Keep operator scope anchored to the current live proposal-loop baseline plus `P005-TRANSPORT`, without pulling Proposal 007 repo-backed implementation or release behavior forward.
- Make runtime trust/provenance explicit across runs home, reports, comparison, recovery, and artifact inspection.

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
- Proposal-loop recovery supports retry/re-arm/clone actions and exposes only actions allowed for the current run type.
- Comparison works for compatible proposal-loop runs and does not imply repo-backed/release diff support.
- Artifact Inspector V2 renders supported formats, shows provenance and traceability, and supports pinning plus reveal-on-disk actions.
- Notifications cover approval, blocked, failed, and completed states; dock badge and optional menu bar are supported.
- No Proposal 001/002/003/004 runtime or UI tests regress.
- The targeted live/recovery baseline from Proposal 004 and `P005-TRANSPORT` compiles and passes before `P005-OPS` implementation starts.
- `xcodebuild build && xcodebuild test` is green before sign-off.
- One engineer can understand what happened and recover a proposal-loop blocked/failed run without raw files or database edits.

### Test / Evidence Requirements

- Evidence-first proof against the current Apple-platform app, not just code presence.
- Strongest practical proof is a fresh `xcodebuild` run with external DerivedData and current-head UI/runtime validation.
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
| Implemented | 5 |
| Partially Implemented | 5 |
| Missing | 2 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 Runs Home is the primary operator landing surface with the required grouped sections
- Proposal Source: `§5.1-§5.2`, `§12 Runs Home` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:83-99`, `:418-419`)
- Status: Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/ContentView.swift:4-24`
  - `Chainworks Forge/ContentView.swift:42-49`
  - `Chainworks Forge/Views/RunsHomeView.swift:22-121`
  - Fresh full-scheme `xcodebuild ... test` run reached and passed `Chainworks_ForgeUITests.testApprovalInboxReachable`
- Gap / Note: `RunsHomeView` is the default operator landing tab, and the sidebar groups runs into `Waiting Approval`, `Blocked`, `Running`, and `Recently Completed`.

### REQ-002 Runs Home rows show operator metadata, trust, and only executable contextual actions
- Proposal Source: `§4`, `§5.3-§5.4`, `§12 Runs Home` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:73-79`, `:100-132`, `:420-421`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:29-38`
  - `Chainworks Forge/Views/RunsHomeView.swift:95-103`
  - `Chainworks Forge/Views/RunsHomeView.swift:260-326`
  - `Chainworks Forge/Views/RunsHomeView.swift:384-409`
- Gap / Note: This closes the main R1 blocker. The row actions are now real closures, gated by status and report/comparison availability instead of dead-end placeholders.

### REQ-003 Reports use immutable-history plus latest-summary semantics, and the operator can read that distinction in the UI
- Proposal Source: `§6.2-§6.3`, `§12 Reports` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:140-168`, `:425-427`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:20-113`
  - `Chainworks Forge/Views/RunReportView.swift:21-127`
  - `Chainworks Forge/Views/RunsHomeView.swift:158-169`
- Gap / Note: Immutable report versions are append-only, latest summary is separate, and the surfaced view now labels “Mutable latest summary” versus “Immutable history.”

### REQ-004 Report payload includes the full deterministic operator narrative defined in the proposal
- Proposal Source: `§6.4-§6.5` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:170-248`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift:129-220`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:260-299`
  - `Chainworks Forge/Models/Run.swift:33-39`
  - `Chainworks Forge/Models/AgentExecution.swift:13-30`
- Gap / Note: Runtime trust, drift note, approvals, stage timeline, and agent/provider/model/effort metadata are present. But the proposal’s retry/recovery narrative is still incomplete: `retryPath` and `resumePath` are hard-coded `nil`, and the report does not summarize “retries performed” or “recovery actions taken” even though those fields are part of the contract.

### REQ-005 Recovery supports in-scope proposal-loop actions and exposes only safe actions for the current run type
- Proposal Source: `§7.1-§7.4`, `§12 Recovery` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:252-295`, `:432-434`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:23-57`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:61-220`
  - `Chainworks Forge/Views/RecoverySheet.swift:18-92`
  - `Chainworks Forge/Views/RecoverySheet.swift:101-178`
- Gap / Note: The recovery sheet now wires retry, re-arm, frozen clone, and current-config clone flows into the current app shell, and the action list remains constrained to read-only proposal-loop run types.

### REQ-006 Comparison works for compatible proposal-loop runs and shows the proposal’s required comparison dimensions
- Proposal Source: `§8.1-§8.3`, `§12 Comparison` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:298-332`, `:438-440`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunComparisonService.swift:17-92`
  - `Chainworks Forge/Engine/RunComparisonService.swift:114-186`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift:193-221`
  - `Chainworks Forge/Views/RunsHomeView.swift:544-592`
- Gap / Note: True compatibility gating is now in place and the comparison UI shows snapshot, trust, timing, cost, stage, approval, and pinned-artifact deltas. But the proposal also requires provider/model/effort bindings as a comparison dimension, and those bindings are computed in `RunComparisonService` without ever being rendered in `RunComparisonView`.

### REQ-007 Artifact Inspector V2 renders the required formats, shows provenance, supports pin/open actions, and provides traceability
- Proposal Source: `§9`, `§12 Artifact Inspector V2` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:334-366`, `:444-447`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:22-242`
  - `Chainworks Forge/Views/IdeaListView.swift` (run progress and stage detail sheets route to `ArtifactInspectorView`)
  - Fresh full-scheme `xcodebuild ... test` run passed `Chainworks_ForgeUITests.testArtifactInspectorOpensProposalAndReceiptArtifacts`
  - Fresh full-scheme `xcodebuild ... test` run skipped `Chainworks_ForgeUITests.testArtifactInspectorViewSurface` in headless toolbar conditions
- Gap / Note: The surfaced inspector now uses the V2 view and supports renderers, provenance chips, pin/unpin, and open-on-disk actions. The remaining contract gap is traceability: the “Consumed input” section shows the producing agent’s own consumed inputs from `consumedInputArtifactNamesJSON`, not true downstream consumer attempts for the selected artifact.

### REQ-008 Notifications and presence cover approval, blocked, failed, completed, dock badge, menu bar, and active-app presence
- Proposal Source: `§10`, `§12 Notifications` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:368-383`, `:451-453`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/NotificationService.swift:8-103`
  - `Chainworks Forge/Engine/ExecutionService.swift:139-146`
  - `Chainworks Forge/Engine/ExecutionService.swift:200-205`
  - `Chainworks Forge/Engine/ExecutionService.swift:438-460`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:35-39`
  - `Chainworks Forge/Views/MenuBarStatusView.swift:18-63`
- Gap / Note: Approval, blocked, failed, and completed local notifications are wired, dock badge refresh is wired on attention changes, and the optional menu bar extra exists. The remaining gap is the proposal’s “foreground banners while the app is active” surface; no active-app banner mechanism was found in current HEAD.

### REQ-009 No surfaced operator flow implies repo-write, implementation, release, or publish capability before Proposal 007
- Proposal Source: `§4`, `§7.2-§7.3`, `§8.3`, `§9`, `§13`, `§14 OPS-056`, `§15` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:73-79`, `:264-284`, `:323-366`, `:470-507`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:23-57`
  - `Chainworks Forge/Views/RecoverySheet.swift:5-10`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift:13`
- Gap / Note: The current operator surfaces stay inside the read-only proposal-loop boundary. No surfaced recovery or comparison affordance claims repo-backed, git, publish, or release capabilities.

### REQ-010 No Proposal 001 / 002 / 003 / 004 runtime or UI tests regress
- Proposal Source: `§12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:455-458`)
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - Fresh full-scheme command: `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-p005ops-audit-r2 -resultBundlePath /tmp/codex-p005ops-audit-r2.xcresult test`
  - Result bundle: `/tmp/codex-p005ops-audit-r2.xcresult`
  - Failure surfaced in `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:136`
- Gap / Note: The fresh scheme run failed in `Chainworks_ForgeUITests.testProductCheckpointExecutionFlowReachable`, which is a pre-existing Proposal 002/004 checkpoint test. That means the “no regressions” gate is not currently satisfied.

### REQ-011 The targeted live/recovery baseline from Proposal 004 and `P005-TRANSPORT` compiled and passed before `P005-OPS` implementation started
- Proposal Source: `§12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:458`)
- Status: Not Verifiable
- Evidence Type: tests-run, inference
- Evidence:
  - Current-head full-scheme run passed relevant live/recovery slices such as `Chainworks_ForgeUITests.testApprovalInboxReachable`, `Chainworks_ForgeUITests.testRunProgressViewSurface`, `Chainworks_ForgeUITests.testLiveProposalLoopFixtureFlowReachesApprovalAndCompletion`, and `Chainworks_ForgeUITests.testWaitingApprovalRunIsRestoredOnLaunch`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Gap / Note: Current HEAD still demonstrates a working live/recovery baseline, but the proposal’s wording is historical (“before implementation starts”). That sequencing gate cannot be proven retroactively from the current repository alone.

### REQ-012 Full `xcodebuild build && xcodebuild test` is green before sign-off
- Proposal Source: `§12 Sequential implementation gates` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:459`)
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - Fresh full-scheme command: `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-p005ops-audit-r2 -resultBundlePath /tmp/codex-p005ops-audit-r2.xcresult test`
  - Result bundle: `/tmp/codex-p005ops-audit-r2.xcresult`
  - Suite summary from the run: `Executed 13 tests, with 4 tests skipped and 1 failure`
- Gap / Note: The fresh sign-off run is not green, so the proposal’s explicit sign-off condition remains open.

### REQ-013 One engineer can understand run state quickly and recover a blocked/failed run without raw files or database edits
- Proposal Source: `§12 Product checkpoint` (`docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md:461-468`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:22-221`
  - `Chainworks Forge/Views/RunReportView.swift:21-131`
  - `Chainworks Forge/Views/RecoverySheet.swift:18-178`
  - Fresh full-scheme `xcodebuild ... test` run passed `Chainworks_ForgeUITests.testApprovalInboxReachable`, `Chainworks_ForgeUITests.testRunProgressViewSurface`, and `Chainworks_ForgeUITests.testWaitingApprovalRunIsRestoredOnLaunch`
  - Fresh full-scheme `xcodebuild ... test` run failed `Chainworks_ForgeUITests.testProductCheckpointExecutionFlowReachable`
- Gap / Note: The operator spine is now strong enough that a human can plausibly inspect and recover many proposal-loop runs from the UI, but the current head does not close the proposal’s own product-checkpoint proof. The live checkpoint test still fails on the `Approvals` tab handoff after run start, and there is no current timed proof for the “under 30 seconds” understanding claim.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" 'docs/proposals/005-operator-experience-reports-recovery-and-run-comparison.md' 'docs/reviews'`
- `rg -n "RunsHomeView|RunReportBuilder|RecoveryCoordinator|RunComparisonService|ArtifactInspectorView|MenuBarStatusView|NotificationService|RunReportView|RunComparisonView|RecoverySheet" 'Chainworks Forge'`
- `rg -n "test.*(Approval|RunProgress|ArtifactInspector|Report|Comparison|Recovery|MenuBar|RunsHome)" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-p005ops-audit-r2 -resultBundlePath /tmp/codex-p005ops-audit-r2.xcresult test`

## Recommended Next Actions

- Fix the fresh full-suite regression in `Chainworks_ForgeUITests.testProductCheckpointExecutionFlowReachable`, specifically the failed `Approvals` tab reachability after live run start.
- Finish the report payload contract by recording retry/recovery action details instead of leaving `retryPath` and `resumePath` as `nil`.
- Render provider/model/effort bindings in `RunComparisonView`, since the comparison service already computes them.
- Replace the current artifact “Consumed input” display with true downstream-consumer traceability for the selected artifact.
- Add the active-app foreground banner surface promised by `§10`, or narrow the proposal text if banners are no longer intended.
