# Proposal 007: Full MVP Delivery Slice — Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R4

| Field | Value |
|---|---|
| Proposal | docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md |
| Repository Root | . |
| Git SHA | 63f5270 |
| Working Tree | dirty |
| Audited At | 2026-03-27T22:56:33+0200 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 007 is stronger than in R3. The runtime now emits `delivery_receipt` from the real delivery path, focused repo-backed proof is green at the build and unit layers (`build` passed, focused unit slice passed `60/60`), and the proposal-scoped macOS UI slice passed `4/5`. The proposal still does not reach `Implemented`, though, because its sign-off-critical dogfood proof remains open: the full product checkpoint UI flow failed on the current tree, no app-launched happy-path or non-happy-path repo-backed evidence pack was found, and `DeliveryConfiguration` still freezes inside the Start Run UI boundary rather than in a shared run-creation boundary.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Proposal-level dogfood sign-off proof is still not closed | High |
| Architecture | At Risk | `DeliveryConfiguration` freeze remains UI-owned instead of shared runtime truth | High |
| Product | At Risk | Repo-backed happy-path/non-happy-path dogfood evidence still not exists from inside the app | High |
| UI | Acceptable | Focused release-gate/start/progress surfaces are green, but the full checkpoint owner path still fails | High |
| UX | At Risk | Blocked-release recovery and evidence-export behavior are still not proven as an operator flow | Medium |
| Readiness | Not Ready | The proposal’s own sign-off checkpoint still fails on current `HEAD` | High |

## Proposal Contract

### Scope

- Deliver the first repo-backed 12-state end-to-end workflow from idea through completed release candidate inside the app.
- Add one dedicated writable worktree per run for implementation and release execution.
- Run real implementation, review, and release stages against a repository-backed target.
- Keep manual release gating explicit and route release side effects through deterministic services.
- Ship a dogfood-ready preset, sample repo profile, and exportable evidence pack.

### Locked Decisions

- One run equals one dedicated writable worktree.
- No concurrent write-capable agents may share a writable worktree.
- Release mechanics run through deterministic services, not free-form agent shelling.
- `full-mvp-live.yaml` is separate from the fast proposal-loop smoke path.
- `docs_report` must exist before audit aggregation in the first implementation review cycle.
- Default release targets are `sandbox` and `staging`, not production.
- Partial release failure returns to blocked/operator recovery rather than hidden rollback.
- Approval gates remain explicit workflow states.

### Primary User Flows

1. Start a repo-backed run with a frozen `DeliveryConfiguration` and delivery-specific preflight.
2. Enter implementation with a dedicated worktree and iterate through implementation/review/refine until `Implemented`.
3. Reach a dedicated manual release gate and approve or reject deterministic release side effects.
4. Export a dogfood evidence pack and recover a blocked release path without guessing.

### UI Commitments

- Dogfood Start Run preset with repo, branch, worktree, and release-target inputs.
- Worktree-aware run progress and release-gate surfaces.
- Existing operator/report/recovery/provider baseline surfaces apply to repo-backed runs.
- Evidence pack export is available from run surfaces.

### UX Commitments

- Explicit approvals for initial proposal, implementation approval, and manual release.
- No hidden release mechanics or implicit repo state.
- Safe repo/worktree isolation and recoverable blocked-release behavior.
- Dogfood sign-off requires happy-path and non-happy-path evidence from inside the app.

### Acceptance Criteria

- Dedicated writable worktree provisioned and persisted before the first implementation write.
- No write-capable action can target outside `worktreeRoot` / `workspaceRoot`; no shared writable worktrees.
- `full-mvp-live.yaml` compiles into a valid 12-state executable plan.
- Implementation review/refine loop persists required artifacts and can iterate until `Implemented`.
- Manual release blocks on explicit human approval.
- Release side effects execute only through deterministic services and produce durable manifests/receipts.
- Start Run, preflight, run creation, and resume share the same frozen `DeliveryConfiguration`.
- Repo-backed runs work cleanly in Run Progress, Release Gate, report/recovery/comparison, and provider baseline.
- Happy-path and non-happy-path dogfood runs can be completed from inside the app with exported evidence.
- `xcodebuild build && xcodebuild test` is green with no regressions in earlier slices.

### Test / Evidence Requirements

- Unit tests for worktree/repo safety, workflow structure, implementation loop, and release ops.
- Safe local integration tests for sample-repo runs, blocked-release recovery, resume, and release rejection.
- Env-gated live smoke tests for sandbox push/upload and a full dogfood run.
- One happy-path run, one non-happy-path run, exported evidence pack, and screenshots for release gate plus final receipts.

### Explicit Exclusions

- Multiple concurrent write-capable agents in one worktree.
- Autonomous release with no human gate.
- Automatic rollback after push/upload.
- Multi-repo orchestration.
- Cloud/background execution.
- Production-by-default release targets.

## Proposal Fidelity / Divergence

### Matches

- `examples/workflows/full-mvp-live.yaml` exists and focused `Full MVP Delivery` suites validate the promised 12-state structure and approval topology.
- The runtime provisions a dedicated worktree and persists repo/base metadata on the run.
- `SourceContextBuilder` is wired into repo-backed execution inputs via `WorkflowOrchestrator.gatherExecutionInputs(...)`.
- Release-stage agents still bypass the generic executor and use deterministic git/build services.
- `ReleaseGateView` quick actions are real buttons that expose the promised receipt/proposal actions.
- `DeliveryReceiptBuilder` is now wired into the terminal delivery path and focused orchestrator tests prove `delivery_receipt` is emitted for both successful and partial-failure delivery outcomes.
- The app exposes `Export Evidence Pack` from repo-backed completed/failed runs.
- Fresh focused build and unit proof are green on the current tree.

### Divergences

- `DeliveryConfiguration` is still frozen in `IdeaListView.startRun()` after `createRun()`, not inside a shared run-creation boundary as Proposal 007 §6.4 describes.
- This audit found no app-launched happy-path or non-happy-path repo-backed run receipts/evidence pack under the default run-storage or Desktop export roots.
- The proposal-scoped macOS UI slice still fails its full checkpoint proof on current `HEAD`.

### Ambiguities / Evidence Gaps

- This pass intentionally used focused build/unit/UI slices; a full-scheme `xcodebuild test` run was not executed.
- No real sample-repo dogfood run was executed manually in this audit session, so readiness depends on runtime evidence already present on disk or exercised through the focused UI proof.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 8 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state plan with explicit approval gates
- Proposal Source: `2. Product question this proposal must answer`, `11.2 Add full-mvp-live.yaml`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r4-unit.xcresult`
- Gap / Note: Workflow topology, approval states, and implementation/review loop structure are covered by the focused `Full MVP Delivery` suites.

### REQ-002 Start Run, delivery preflight, and run creation share one frozen `DeliveryConfiguration`
- Proposal Source: `6.4 Delivery configuration is a first-class boundary`, `6.5 Sample repo profile schema stays subordinate`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.1 Dogfood Start Run preset`, `14. Acceptance criteria / UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1274`
  - `Chainworks Forge/Views/IdeaListView.swift:1285`
  - `Chainworks Forge/Views/IdeaListView.swift:1291`
  - `Chainworks Forge/Views/IdeaListView.swift:1323`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Gap / Note: The config is frozen, mirrored onto `Run`, and resumed from persisted state, but the freeze still lives in the Start Run UI after `createRun()` instead of inside one shared run-creation boundary.

### REQ-003 The orchestrator provisions and persists one dedicated writable worktree before implementation begins
- Proposal Source: `7.1 Core rule`, `7.2 Worktree identity`, `7.3 Persisted metadata`, `7.4 Provisioning rules`, `8.1 Handoff from approved proposal`, `14. Acceptance criteria / Runtime / worktree`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:936`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:952`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
  - `/tmp/p007-r4-unit.xcresult`
- Gap / Note: Provisioning, persisted `worktreeRoot`, and base-revision capture are directly exercised by focused unit coverage.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree
- Proposal Source: `7.5 No shared write worktrees`, `7.7 Path boundary enforcement`, `14. Acceptance criteria / Runtime / worktree`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RepoSafetyGuard.swift`
  - `Chainworks ForgeTests/DeliveryServicesTests.swift`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
  - `/tmp/p007-r4-unit.xcresult`
- Gap / Note: Guard APIs and focused tests exist, but this audit still did not prove boundary enforcement at every repo-backed tool/file operation in a real run.

### REQ-005 Approved implementation agents execute against the real provisioned worktree and explicit source context
- Proposal Source: `3. What we build / Layer I`, `8. Implementation slice`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:464`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:1105`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:1117`
  - `Chainworks Forge/Engine/SourceContextBuilder.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `/tmp/p007-r4-unit.xcresult`
- Gap / Note: The repo-backed execution path now gathers and injects `source_context`, `source_diff_summary`, and `source_changed_files_manifest` from the provisioned worktree.

### REQ-006 The implementation review/refine loop persists the required artifacts and can iterate until `Implemented`
- Proposal Source: `8.2 Continue until seemingly complete`, `8.3 Implementation reviewed against proposal`, `8.4 Implementation refined`, `14. Acceptance criteria / Workflow`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r4-unit.xcresult`
- Gap / Note: The workflow structure and artifact contract are validated, but this audit still did not prove a real repo-backed implementation/review/refine loop against a live worktree-backed code change inside the app.

### REQ-007 Manual release stays explicit and the operator gets a dedicated release-gate surface with sufficient context
- Proposal Source: `9.1 Release must remain explicit`, `10.3 Release Gate View`, `14. Acceptance criteria / Workflow and UI`
- Status: Implemented
- Evidence Type: code, tests-run, runtime, screenshot
- Evidence:
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:485`
  - `/tmp/p007-r4-ui.xcresult`
- Gap / Note: The quick-action rows are real buttons, and the focused release-gate UI proof passed on current `HEAD`.

### REQ-008 Release side effects execute only through deterministic runtime services rather than simulated artifact generation
- Proposal Source: `3. What we build / Layer I`, `9.2 Release step sequence`, `9.3 Service contract`, `14. Acceptance criteria / Release`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:488`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:592`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r4-unit.xcresult`
- Gap / Note: Release-stage agents still route through app-controlled deterministic services rather than the generic executor path.

### REQ-009 Commit/push produces a real `release_manifest` and `git_push_receipt` for the repo-backed workflow
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / GitReleaseService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:624`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:632`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:635`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `/tmp/p007-r4-unit.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
- Gap / Note: The production release path now persists the right artifacts in code, but this audit still found no repo-backed app run that actually emitted those receipts on disk.

### REQ-010 Archive/distribute produces a real `connect_upload_receipt` and `release_bundle_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / ConnectPublishService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:686`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:696`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:699`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `/tmp/p007-r4-unit.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
- Gap / Note: The deterministic build/upload path is wired, but this audit still did not verify a real repo-backed archive/upload outcome from inside the app.

### REQ-011 Partial release failure preserves receipts and returns the run to an operator-visible blocked recovery path
- Proposal Source: `9.4 Partial failure semantics`, `14. Acceptance criteria / Release and Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:648`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:722`
  - `Chainworks ForgeTests/OrchestratorTests.swift:1470`
  - `Chainworks ForgeTests/OrchestratorTests.swift:1630`
  - `/tmp/p007-r4-unit.xcresult`
- Gap / Note: Partial-failure semantics are modeled and tested at the runtime/service level, but this audit still lacks a real blocked-release recovery loop executed through the app.

### REQ-012 Evidence Pack Builder exports the complete dogfood packet promised by the proposal
- Proposal Source: `12.2 Evidence pack builder`, `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift:560`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:584`
  - `/tmp/p007-r4-unit.xcresult`
  - `find "$HOME/Desktop" -maxdepth 3 ...`
- Gap / Note: The exporter and UI action exist, but this audit still found no exported evidence pack from an actual repo-backed happy-path or non-happy-path app run.

### REQ-013 Existing operator/report/recovery/provider-baseline surfaces apply cleanly to repo-backed runs
- Proposal Source: `4. Scope / In scope item 6`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.2 Run Progress View enhancements`, `10.4 Worktree / diff affordances`, `14. Acceptance criteria / UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime, screenshot
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `/tmp/p007-r4-ui.xcresult`
- Gap / Note: Focused macOS UI proof is materially stronger, but report/recovery/comparison/evidence-export behavior is still not proven on a real repo-backed delivery run.

### REQ-014 Proposal 007 sign-off proof is closed on current `HEAD`
- Proposal Source: `12.3 Manual dogfood script`, `13. Testing strategy`, `14. Acceptance criteria / General and Product checkpoint`
- Status: Not Verifiable
- Evidence Type: tests-run, runtime, inference
- Evidence:
  - `/tmp/p007-r4-build.xcresult`
  - `/tmp/p007-r4-unit.xcresult`
  - `/tmp/p007-r4-ui.xcresult`
  - `xcrun xcresulttool get test-results summary --path /tmp/p007-r4-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
  - `find "$HOME/Desktop" -maxdepth 3 ...`
- Gap / Note: This audit still did not prove one happy-path repo-backed run, one non-happy-path run, and one exported evidence pack from inside the app. The focused UI slice also failed `Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution()`.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Delivery configuration truth is still Start Run UI-owned
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `6.4 Delivery configuration is a first-class boundary`, `REQ-002`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1274`
  - `Chainworks Forge/Views/IdeaListView.swift:1323`
- Why It Matters: Proposal 007 treats `DeliveryConfiguration` as shared run-creation truth for Start Run, preflight, provisioning, and resume. The current implementation still builds and freezes that object inside the Start Run UI after `createRun()`, which keeps one proposal-critical boundary in the presentation layer instead of in a shared runtime service/compiler boundary.
- Recommended Action: Move the final validated delivery freeze into one shared run-creation boundary and let the UI submit only a draft/input object.

## Product Review

**Summary:** At Risk

### PROD-001 The core dogfood job is still not proven end-to-end from inside the app
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `Product question`, `Manual dogfood script`, `REQ-009`, `REQ-010`, `REQ-011`, `REQ-012`, `REQ-014`
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p007-r4-unit.xcresult`
  - `/tmp/p007-r4-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
  - `find "$HOME/Desktop" -maxdepth 3 ...`
- Why It Matters: Proposal 007 is not just a structural workflow slice; it is the first believable repo-backed dogfood loop. The current tree now has strong structural and focused runtime proof, but it still does not have the sign-off evidence the proposal itself demands: one happy-path repo-backed run, one non-happy-path run, and exported evidence from inside the app.
- Recommended Action: Run and capture one real sample-repo happy path and one blocked-release/non-happy path from inside the app, export both evidence packs, and attach them to sign-off.

## UI Review

**Summary:** Acceptable

### UI-001 The full product checkpoint owner path is still unstable in focused macOS proof
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `14. Acceptance criteria / General and Product checkpoint`, `REQ-014`
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p007-r4-ui.xcresult`
  - `Chainworks ForgeUITests/AppScreen.swift:66`
- Why It Matters: Focused release-gate, start-sheet, progress, and approval surfaces are green, but the proposal’s umbrella checkpoint still failed in the current UI proof. That means the polished slice exists, yet the top-level operator journey still cannot be called signed off.
- Recommended Action: Stabilize the canonical product-checkpoint owner path, then rerun the same focused UI slice until `5/5` is green.

## UX Review

**Summary:** At Risk

### UX-001 Blocked-release recovery and evidence export are still not proven as a calm operator flow
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `9.4 Partial failure semantics`, `12.3 Manual dogfood script`, `REQ-011`, `REQ-012`, `REQ-014`
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift:560`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
  - `/tmp/p007-r4-ui.xcresult`
- Why It Matters: Proposal 007 promises recoverable dogfooding, not just durable receipts. The runtime and tests now model delivery failure semantics and receipt generation, but this audit still did not observe an operator recover a blocked release and export evidence from that real path.
- Recommended Action: Add one operator-facing runtime proof that drives a blocked release, verifies preserved receipts, re-enters recovery/release-gate context, and exports evidence intentionally.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Focused proof is stronger, but proposal-level sign-off is still open
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`, `REQ-014`
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p007-r4-build.xcresult`
  - `/tmp/p007-r4-unit.xcresult`
  - `/tmp/p007-r4-ui.xcresult`
  - `xcrun xcresulttool get test-results summary --path /tmp/p007-r4-ui.xcresult`
- Why It Matters: Build health and focused repo-backed unit coverage are now strong, and the old `delivery_receipt` runtime gap is closed. The remaining blocker is the proposal’s own sign-off rule: until the real happy-path/non-happy-path dogfood evidence exists and the full product checkpoint passes, Proposal 007 is not ready to call done.
- Recommended Action: Close sign-off with one app-launched happy-path run, one blocked-release/non-happy-path run, exported evidence packs, and a green rerun of the focused product-checkpoint UI slice.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `/tmp/p007-r4-build.xcresult` |
| Core user flow runtime-validated | Partial | Focused release-gate/start/progress/approval UI proof is green, but the full product checkpoint failed |
| Empty/loading/error states covered | Partial | Approval/start/progress/release-gate surfaces exercised; blocked-release recovery/export path still unproven |
| Accessibility risk acceptable | Partial | Focused macOS UI tests are mostly green, but the full checkpoint still fails in automation |
| Localization risk acceptable | Not Checked | No localization-specific evidence in this audit |
| Critical tests executed | Pass | `/tmp/p007-r4-unit.xcresult` passed `60/60`; `/tmp/p007-r4-ui.xcresult` passed `4/5` |
| Privacy/permissions/entitlements reviewed | Not Checked | Out of scope for this focused implementation audit |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `stat -f 'proposal_mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `md5 -q docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `rg -n "DeliveryReceiptBuilder|delivery_receipt|Export Evidence Pack|EvidencePackBuilder|SourceContextBuilder|full-mvp-live|ReleaseGateView|release-gate-open" 'Chainworks Forge' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `sed -n '1080,1135p' Chainworks\ Forge/Engine/WorkflowOrchestrator.swift`
- `sed -n '1600,1695p' Chainworks\ Forge/Engine/WorkflowOrchestrator.swift`
- `sed -n '548,615p' Chainworks\ Forge/Views/RunsHomeView.swift`
- `sed -n '1440,1495p' Chainworks\ ForgeTests/OrchestratorTests.swift`
- `sed -n '1598,1648p' Chainworks\ ForgeTests/OrchestratorTests.swift`
- `sed -n '820,920p' Chainworks\ ForgeUITests/Chainworks_ForgeUITests.swift`
- `sed -n '1260,1355p' Chainworks\ Forge/Views/IdeaListView.swift`
- `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
- `find "$HOME/Desktop" -maxdepth 3 ...`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r4-build-dd -resultBundlePath /tmp/p007-r4-build.xcresult build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r4-unit-dd -resultBundlePath /tmp/p007-r4-unit.xcresult test -only-testing:'Chainworks ForgeTests/FullMVPWorkflowTests' -only-testing:'Chainworks ForgeTests/FullMVPReleaseOpsTests' -only-testing:'Chainworks ForgeTests/FullMVPIntegrationTests' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests' -only-testing:'Chainworks ForgeTests/WorktreeProvisionerTests' -only-testing:'Chainworks ForgeTests/OrchestratorTests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r4-ui-dd -resultBundlePath /tmp/p007-r4-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- `xcrun xcresulttool get test-results summary --path /tmp/p007-r4-unit.xcresult`
- `xcrun xcresulttool get test-results summary --path /tmp/p007-r4-ui.xcresult`

## Recommended Next Actions

1. Move `DeliveryConfiguration` freeze into a shared run-creation boundary instead of keeping it Start Run UI-owned.
2. Run one real happy-path sample-repo flow and one blocked-release/non-happy-path flow from inside the app, then export both evidence packs.
3. Fix `Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution()` so the focused Proposal 007 UI slice is `5/5` green.
4. Re-run the same focused build/unit/UI pack after collecting real app-launched dogfood evidence.
