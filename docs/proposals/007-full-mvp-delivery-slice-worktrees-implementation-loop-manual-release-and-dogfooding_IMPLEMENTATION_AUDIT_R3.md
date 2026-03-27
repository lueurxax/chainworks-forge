# Proposal 007: Full MVP Delivery Slice — Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md |
| Repository Root | . |
| Git SHA | 63f5270 |
| Working Tree | dirty |
| Audited At | 2026-03-27T07:43:12+0200 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 007 is materially stronger than in R2. The repo-backed runtime now wires `SourceContextBuilder` into live execution inputs, the release gate exposes real open-artifact actions, and fresh focused proof is green: `build` passed, the focused unit slice passed `42/42`, and the focused macOS UI slice passed `4` tests with `1` headless skip. The proposal still does not reach `Implemented`, though, because its sign-off-critical dogfood evidence is still open and one runtime contract remains incomplete: `DeliveryReceiptBuilder` is still not wired into the real delivery path, so `delivery_receipt` is still seeded for direct-surface/demo data rather than produced by a real repo-backed run.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Full dogfood sign-off proof and runtime `delivery_receipt` remain open | High |
| Architecture | At Risk | Final receipt/report boundary still bypasses `DeliveryReceiptBuilder` at runtime | High |
| Product | At Risk | The core repo-backed dogfood job is still not proven end-to-end from inside the app | High |
| UI | Acceptable | Focused release-gate and owner-path UI proof is green on macOS | High |
| UX | At Risk | Blocked-release recovery and evidence export are still not proven as an operator flow | Medium |
| Readiness | Not Ready | Proposal-level happy/non-happy dogfood proof is still missing | High |

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
- `SourceContextBuilder` is now wired into repo-backed execution inputs via `WorkflowOrchestrator.gatherExecutionInputs(...)`.
- Release-stage agents still bypass the generic executor and use deterministic git/build services.
- `ReleaseGateView` quick actions are now real buttons that open the relevant artifact when it exists.
- The app exposes `Export Evidence Pack` from repo-backed completed/failed runs.
- Fresh focused build, unit, and UI evidence all passed on the current tree.

### Divergences

- `DeliveryConfiguration` is still frozen in `IdeaListView.startRun()` after `createRun()`, not inside a shared run-creation boundary as Proposal 007 §6.4 describes.
- `DeliveryReceiptBuilder` remains unwired into the runtime release/finalization path; `delivery_receipt` still appears only in seeded direct-surface data, not from real repo-backed execution.
- No app-launched happy-path or non-happy-path repo-backed run receipts/evidence pack were found under the run storage or Desktop export roots.

### Ambiguities / Evidence Gaps

- `testFullProductCheckpointCanonicalExecution()` still skipped in the headless macOS UI environment, so the proposal’s full checkpoint remains unproven by runtime automation here.
- This audit intentionally used focused slices; a full `xcodebuild test` run for the entire scheme was not executed in this pass.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 9 |
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
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: Workflow topology, approval states, and implementation/review loop structure are covered by the focused `Full MVP Delivery` suites.

### REQ-002 Start Run, delivery preflight, and run creation share one frozen `DeliveryConfiguration`

- Proposal Source: `6.4 Delivery configuration is a first-class boundary`, `6.5 Sample repo profile schema stays subordinate`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.1 Dogfood Start Run preset`, `14. Acceptance criteria / UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1310`
  - `Chainworks Forge/Views/IdeaListView.swift:1323`
  - `Chainworks Forge/Views/IdeaListView.swift:1333`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:120`
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
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: Provisioning, persisted `worktreeRoot`, and base-revision capture are now directly exercised by focused unit coverage.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree

- Proposal Source: `7.5 No shared write worktrees`, `7.7 Path boundary enforcement`, `14. Acceptance criteria / Runtime / worktree`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RepoSafetyGuard.swift`
  - `Chainworks ForgeTests/DeliveryServicesTests.swift`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: Guard APIs and focused tests exist, but this audit still did not prove boundary enforcement at every repo-backed tool/file operation in a real run.

### REQ-005 Approved implementation agents execute against the real provisioned worktree and explicit source context

- Proposal Source: `3. What we build / Layer I`, `8. Implementation slice`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:464`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:1037`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:1060`
  - `Chainworks Forge/Engine/SourceContextBuilder.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: The repo-backed execution path now gathers and injects `source_context`, `source_diff_summary`, and `source_changed_files_manifest` from the provisioned worktree.

### REQ-006 The implementation review/refine loop persists the required artifacts and can iterate until `Implemented`

- Proposal Source: `8.2 Continue until seemingly complete`, `8.3 Implementation reviewed against proposal`, `8.4 Implementation refined`, `14. Acceptance criteria / Workflow`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/workflows/full-mvp-live.yaml`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: The workflow structure and artifact contract are validated, but this audit still did not prove a real repo-backed implementation/review/refine loop against a live worktree-backed code change inside the app.

### REQ-007 Manual release stays explicit and the operator gets a dedicated release-gate surface with sufficient context

- Proposal Source: `9.1 Release must remain explicit`, `10.3 Release Gate View`, `14. Acceptance criteria / Workflow and UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime, screenshot
- Evidence:
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:485`
  - `/tmp/p007-r3-ui.xcresult`
- Gap / Note: The quick-action rows are now real buttons and the focused release-gate UI proof passed. The surface is still only partially closed because one key receipt (`delivery_receipt`) is seeded for direct-surface/demo data and not yet produced by the real runtime.

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
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: Release-stage agents still route through app-controlled deterministic services rather than the generic executor path.

### REQ-009 Commit/push produces a real `release_manifest` and `git_push_receipt` for the repo-backed workflow

- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / GitReleaseService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:624`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:632`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:635`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: The production release path now persists the right artifacts in code, but this audit still found no repo-backed app run that actually emitted those receipts on disk.

### REQ-010 Archive/distribute produces a real `connect_upload_receipt` and `release_bundle_manifest`

- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / ConnectPublishService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:686`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:696`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:699`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: The deterministic build/receipt path is wired, but this audit still did not verify a real repo-backed archive/upload outcome from inside the app.

### REQ-011 Partial release failure preserves receipts and returns the run to an operator-visible blocked recovery path

- Proposal Source: `9.4 Partial failure semantics`, `14. Acceptance criteria / Release and Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:648`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:722`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r3-unit.xcresult`
- Gap / Note: Partial-failure semantics are modeled and tested at the runtime/service level, but this audit still lacks a real blocked-release recovery loop executed through the app.

### REQ-012 Evidence Pack Builder exports the complete dogfood packet promised by the proposal

- Proposal Source: `12.2 Evidence pack builder`, `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Engine/DeliveryReceiptBuilder.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift:560`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:876`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r3-unit.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
  - `find "$HOME/Desktop" -maxdepth 2 -type d -name 'evidence-pack-*'`
- Gap / Note: The exporter and UI action exist, but the runtime still does not produce a real `delivery_receipt`, and this audit found no exported evidence pack from an actual repo-backed happy-path or non-happy-path run.

### REQ-013 Existing operator/report/recovery/provider-baseline surfaces apply cleanly to repo-backed runs

- Proposal Source: `4. Scope / In scope item 6`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.2 Run Progress View enhancements`, `10.4 Worktree / diff affordances`, `14. Acceptance criteria / UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime, screenshot
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `/tmp/p007-r3-ui.xcresult`
- Gap / Note: Focused macOS UI proof is stronger than in R2, but report/recovery/comparison/evidence-export behavior is still not proven on a real repo-backed delivery run.

### REQ-014 Proposal 007 sign-off proof is closed on current `HEAD`

- Proposal Source: `12.3 Manual dogfood script`, `13. Testing strategy`, `14. Acceptance criteria / General and Product checkpoint`
- Status: Not Verifiable
- Evidence Type: tests-run, runtime, inference
- Evidence:
  - `/tmp/p007-r3-build.xcresult`
  - `/tmp/p007-r3-unit.xcresult`
  - `/tmp/p007-r3-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
  - `find "$HOME/Desktop" -maxdepth 2 -type d -name 'evidence-pack-*'`
- Gap / Note: This audit did not prove one happy-path repo-backed run, one non-happy-path run, one exported evidence pack, or the “under 25 minutes fully inside the app” checkpoint. `testFullProductCheckpointCanonicalExecution()` also skipped in this headless macOS environment, and a full-scheme `xcodebuild test` run was not executed in this pass.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Final receipt/report boundary is still not real runtime truth

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `Layer I / DeliveryReceiptBuilder`, `state_12_workflow_complete`, `REQ-007`, `REQ-012`, `REQ-014`
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Engine/DeliveryReceiptBuilder.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:876`
  - `rg -n "delivery_receipt" 'Chainworks Forge' 'Chainworks ForgeTests'`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
- Why It Matters: Proposal 007 promises durable final receipts and explicitly names `DeliveryReceiptBuilder`, but the current tree still does not invoke that builder from the real delivery path. The only visible `delivery_receipt` on this tree is seeded direct-surface/demo data, which means the release-gate and evidence-pack receipt story is still not fully truthful at runtime.
- Recommended Action: Wire `DeliveryReceiptBuilder` into the real delivery finalization path, persist `delivery_receipt` as a runtime artifact for repo-backed runs, and add one focused integration test that proves it appears in a non-seeded run.

## Product Review

**Summary:** At Risk

### PROD-001 The core dogfood job is still not proven end-to-end from inside the app

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `Product question`, `Manual dogfood script`, `REQ-009`, `REQ-010`, `REQ-011`, `REQ-012`, `REQ-014`
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p007-r3-unit.xcresult`
  - `/tmp/p007-r3-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
  - `find "$HOME/Desktop" -maxdepth 2 -type d -name 'evidence-pack-*'`
- Why It Matters: Proposal 007 is not just a structural workflow slice; it is the first believable repo-backed dogfood loop. The current tree now has strong structural and focused UI proof, but it still does not have the sign-off evidence the proposal itself demands: one happy-path repo-backed run, one non-happy-path run, exported evidence, and final receipts.
- Recommended Action: Run and capture one real sample-repo happy path and one blocked-release/non-happy path from inside the app, export both evidence packs, and attach them to sign-off.

## UI Review

**Summary:** Acceptable

No new UI-specific blocker beyond the product/runtime gaps above surfaced in this audit. `ReleaseGateView` actions are now real buttons, and the focused macOS UI slice passed `4` tests with `1` headless skip.

## UX Review

**Summary:** At Risk

### UX-001 Blocked-release recovery is still not proven as an operator flow

- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `9.4 Partial failure semantics`, `12.3 Manual dogfood script`, `REQ-011`, `REQ-012`, `REQ-014`
- Evidence Type: code, tests-run, runtime
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:648`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
  - `/tmp/p007-r3-ui.xcresult`
- Why It Matters: The proposal’s non-happy-path promise is about calm operator recovery after a real blocked release. The runtime and tests now model the failure semantics, but this audit still did not observe a blocked release being recovered or intentionally cancelled through the real operator surfaces.
- Recommended Action: Add one operator-facing runtime proof that drives a blocked release, verifies preserved receipts, re-enters recovery/release-gate context, and completes recovery or cancellation intentionally.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Focused proof is green, but dogfood sign-off evidence is still incomplete

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`, `REQ-014`
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p007-r3-build.xcresult`
  - `/tmp/p007-r3-unit.xcresult`
  - `/tmp/p007-r3-ui.xcresult`
  - `testFullProductCheckpointCanonicalExecution()` skip in `/tmp/p007-r3-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
- Why It Matters: Build health and focused repo-backed slices are now strong. The remaining blocker is not stale evidence or a flaky compile; it is the proposal’s own sign-off rule. Until the real happy-path and non-happy-path dogfood evidence exists, Proposal 007 is not ready to call done.
- Recommended Action: Close sign-off with one app-launched happy-path run, one blocked-release/non-happy-path run, exported evidence packs, final receipt proof, and then rerun the same focused build/unit/UI slice.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `/tmp/p007-r3-build.xcresult` |
| Core user flow runtime-validated | Partial | Focused release-gate/start/progress/approval UI proof passed, but full product checkpoint skipped and no real repo-backed dogfood run was observed |
| Empty/loading/error states covered | Partial | Approval/start/progress/release-gate surfaces exercised; blocked-release runtime path still unproven |
| Accessibility risk acceptable | Partial | macOS UI tests passed targeted surfaces, but full repo-backed checkpoint remains skipped in headless env |
| Localization risk acceptable | Not Checked | No localization-specific evidence in this audit |
| Critical tests executed | Pass | `/tmp/p007-r3-unit.xcresult` passed `42/42`; `/tmp/p007-r3-ui.xcresult` passed `4` with `1` skip |
| Privacy/permissions/entitlements reviewed | Not Checked | Out of scope for this focused implementation audit |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `md5 -q docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `sed -n '1,260p' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `sed -n '1,280p' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R2.md`
- `rg -n "SourceContextBuilder|sourceContextBuilder|buildSourceContext|delivery_receipt|EvidencePackBuilder|ReleaseGateView|worktreeRootPath|deliveryConfigurationJSON|full-mvp-live" 'Chainworks Forge' 'Chainworks ForgeTests' 'Chainworks ForgeUITests' examples/workflows`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md docs/proposals docs/reviews docs/reference`
- `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f ...`
- `find "$HOME/Desktop" -maxdepth 2 -type d -name 'evidence-pack-*'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r3-build-dd -resultBundlePath /tmp/p007-r3-build.xcresult build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r3-unit-dd -resultBundlePath /tmp/p007-r3-unit.xcresult test -only-testing:'Chainworks ForgeTests/FullMVPWorkflowTests' -only-testing:'Chainworks ForgeTests/FullMVPReleaseOpsTests' -only-testing:'Chainworks ForgeTests/FullMVPIntegrationTests' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests' -only-testing:'Chainworks ForgeTests/WorktreeProvisionerTests'`
- `xcrun xcresulttool get test-results summary --path /tmp/p007-r3-unit.xcresult`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r3-ui-dd -resultBundlePath /tmp/p007-r3-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- `xcrun xcresulttool get test-results summary --path /tmp/p007-r3-ui.xcresult`

## Recommended Next Actions

1. Wire `DeliveryReceiptBuilder` into the real runtime and persist `delivery_receipt` for repo-backed runs.
2. Drive one real happy-path sample-repo run and one blocked-release/non-happy-path run from inside the app, then export both evidence packs.
3. Re-run the full product checkpoint in an environment where the macOS tab owner path is discoverable and capture the release-gate/final-receipt screenshots the proposal requires.
4. Decide whether `DeliveryConfiguration` freeze should move into a shared run-creation boundary instead of remaining Start Run UI-owned.
