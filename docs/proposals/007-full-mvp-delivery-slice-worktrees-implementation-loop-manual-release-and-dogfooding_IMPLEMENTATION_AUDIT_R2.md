# Proposal 007: Full MVP Delivery Slice — Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md |
| Repository Root | . |
| Git SHA | 63f5270 |
| Working Tree | dirty |
| Audited At | 2026-03-27T07:15:19+0200 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 007 is no longer `Not Implemented` on the current tree. The repo now has a real `full-mvp-live.yaml`, frozen `DeliveryConfiguration` start flow, dedicated worktree provisioning, worktree-backed live execution for write-capable agents, deterministic release services wired into the orchestrator, a release-gate surface, and an evidence-pack exporter. The remaining gap is the one the proposal itself treats as sign-off-critical: this audit still could not prove one full repo-backed happy-path run, one non-happy-path run, and one exported dogfood evidence pack from inside the app on the current tree. That keeps overall conformance at `Partial` and readiness at `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Dogfood sign-off proof is still open | High |
| Architecture | Acceptable | `SourceContextBuilder` remains unwired into runtime execution | High |
| Product | At Risk | The core repo-backed flow is not yet proven end-to-end inside the app | High |
| UI | Acceptable | Release gate context improved, but quick actions are still non-executable labels | Medium |
| UX | At Risk | Blocked-release recovery and evidence export are not proven on a real repo-backed run | Medium |
| Readiness | Not Ready | No authoritative happy/non-happy dogfood evidence pack on current `HEAD` | High |

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

- `examples/workflows/full-mvp-live.yaml` exists and the focused `Full MVP Delivery` suites validate the promised 12-state structure and approval topology.
- Start Run now freezes delivery configuration fields and mirrors them onto `Run`.
- The orchestrator provisions a dedicated worktree and write-capable Goose sessions use that worktree as the execution cwd.
- Release-stage agents are no longer going through the generic executor path; the orchestrator calls deterministic git/build services directly and persists release artifacts.
- The app now exposes a dedicated release gate and an `Export Evidence Pack` action for delivery runs.

### Divergences

- `DeliveryConfiguration` is still frozen in `IdeaListView.startRun()` after `createRun()`, not inside a shared run-creation boundary as the proposal describes.
- `SourceContextBuilder` exists but is still not wired into repo-backed writer/reviewer execution.
- `ReleaseGateView` presents decision-context rows, but those “Open ...” affordances are not real actions yet.
- `DeliveryReceiptBuilder` exists but is not part of the runtime release/evidence export path.

### Ambiguities / Evidence Gaps

- No real repo-backed run artifacts were found under `~/Library/Application Support/Chainworks Forge/runs` for `release_manifest`, `git_push_receipt`, `release_bundle_manifest`, `connect_upload_receipt`, or `delivery_receipt`.
- This audit did not produce one app-launched happy-path repo-backed run, one blocked-release/non-happy-path run, or one exported evidence pack from such a run.
- The proposal’s “under 25 minutes fully inside the app” checkpoint remains unproven on current `HEAD`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 3 |
| Partially Implemented | 10 |
| Missing | 0 |
| Not Verifiable | 1 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state plan with explicit approval gates

- Proposal Source: `2. Product question this proposal must answer`, `11.2 Add full-mvp-live.yaml`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/workflows/full-mvp-live.yaml:1-370`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:29-238`
  - `/tmp/p007-r2-fullmvp.xcresult` — `23` passed, `0` failed, `0` skipped
- Gap / Note: This closes the executable workflow topology only, not the full dogfood sign-off.

### REQ-002 Start Run, delivery preflight, and run creation share one frozen `DeliveryConfiguration`

- Proposal Source: `6.4 Delivery configuration is a first-class boundary`, `6.5 Sample repo profile schema stays subordinate`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.1 Dogfood Start Run preset`, `14. Acceptance criteria / UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1253-1334`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:117-129`
  - `Chainworks ForgeTests/DeliveryServicesTests.swift:10-154`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:429-537`
- Gap / Note: The frozen config and mirrored fields are now persisted on `Run`, but the freeze still happens in the Start Run UI after `createRun()`, not inside a single shared run-creation boundary.

### REQ-003 The orchestrator provisions and persists one dedicated writable worktree before implementation begins

- Proposal Source: `7.1 Core rule`, `7.2 Worktree identity`, `7.3 Persisted metadata`, `7.4 Provisioning rules`, `8.1 Handoff from approved proposal`, `14. Acceptance criteria / Runtime / worktree`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:900-919`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:117-129`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
  - `/tmp/p007-r2-fullmvp.xcresult`
- Gap / Note: The proposal’s optional `worktree_manifest` artifact remains optional in practice, but the core provisioning/persistence contract is in place.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree

- Proposal Source: `7.5 No shared write worktrees`, `7.7 Path boundary enforcement`, `14. Acceptance criteria / Runtime / worktree`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RepoSafetyGuard.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:43-66`
  - `Chainworks ForgeTests/DeliveryServicesTests.swift:52-112`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
- Gap / Note: Guard APIs and focused tests exist, but the runtime still does not prove path-boundary enforcement at every repo-backed tool/file operation beyond worktree validation and execution-cwd selection.

### REQ-005 Approved implementation agents execute against the real provisioned worktree and explicit source context

- Proposal Source: `3. What we build / Layer I`, `8. Implementation slice`, `14. Acceptance criteria / Workflow`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:43-66`
  - `Chainworks Forge/Engine/SourceContextBuilder.swift:3-42`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:672-688`
  - `/tmp/p007-r2-fullmvp.xcresult`
- Gap / Note: The worktree-backed execution part is now real. The explicit source-context part is still incomplete because `SourceContextBuilder` remains unwired into the live writer/reviewer runtime.

### REQ-006 The implementation review/refine loop persists the required artifacts and can iterate until `Implemented`

- Proposal Source: `8.2 Continue until seemingly complete`, `8.3 Implementation reviewed against proposal`, `8.4 Implementation refined`, `14. Acceptance criteria / Workflow`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `examples/workflows/full-mvp-live.yaml:202-317`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:82-163`
  - `/tmp/p007-r2-fullmvp.xcresult`
- Gap / Note: The workflow structure and artifact contract are validated, but this audit did not prove the iterative repo-backed loop against a real worktree-backed code change inside the app.

### REQ-007 Manual release stays explicit and the operator gets a dedicated release-gate surface with sufficient context

- Proposal Source: `9.1 Release must remain explicit`, `10.3 Release Gate View`, `14. Acceptance criteria / Workflow and UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run, screenshot
- Evidence:
  - `Chainworks Forge/Views/ReleaseGateView.swift:21-320`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:165-240`
  - `/tmp/p007-r2-ui.xcresult` — `3` passed, `0` failed, `1` skipped
- Gap / Note: The dedicated release gate is present and richer than in R1, but the “Open ...” affordances are still informational rows rather than executable quick actions.

### REQ-008 Release side effects execute only through deterministic runtime services rather than simulated artifact generation

- Proposal Source: `3. What we build / Layer I`, `9.2 Release step sequence`, `9.3 Service contract`, `14. Acceptance criteria / Release`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:479-490`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:583-721`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `/tmp/p007-r2-fullmvp.xcresult`
- Gap / Note: `ReleaseOpsCoordinator` itself is still not the production entry point, but the runtime is now using deterministic services instead of generic executor artifact synthesis.

### REQ-009 Commit/push produces a real `release_manifest` and `git_push_receipt` for the repo-backed workflow

- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / GitReleaseService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:611-647`
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:243-286`
- Gap / Note: The production release path now persists the right artifacts, but this audit still lacks a repo-backed app run that actually produced those receipts on disk.

### REQ-010 Archive/distribute produces a real `release_bundle_manifest` and `connect_upload_receipt`

- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / ConnectPublishService`, `14. Acceptance criteria / Release`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:657-721`
  - `Chainworks Forge/Engine/ConnectPublishService.swift:57-125`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:288-324`
- Gap / Note: The deterministic build/receipt path is wired, but this audit did not verify a real repo-backed archive/upload outcome from inside the app, and sandbox/staging still record a safe local upload receipt rather than an external distribution success.

### REQ-011 Partial release failure preserves receipts and returns the run to an operator-visible blocked recovery path

- Proposal Source: `9.4 Partial failure semantics`, `14. Acceptance criteria / Release and Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:648-655`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:712-719`
  - `examples/workflows/full-mvp-live.yaml:22-24`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:328-377`
- Gap / Note: The runtime failure path now blocks the run and leaves earlier git artifacts persisted, but this audit did not execute a real blocked-release recovery loop from the app.

### REQ-012 Evidence Pack Builder exports the complete dogfood packet promised by the proposal

- Proposal Source: `12.2 Evidence pack builder`, `13.3 Evidence-based review requirement`, `14. Acceptance criteria / Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift:18-186`
  - `Chainworks Forge/Views/RunsHomeView.swift:560-599`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:579-639`
- Gap / Note: The exporter and UI action exist, but this audit did not produce one exported evidence pack from an actual repo-backed happy-path or non-happy-path run.

### REQ-013 Existing operator/report/recovery/provider-baseline surfaces apply cleanly to repo-backed runs

- Proposal Source: `4. Scope / In scope item 6`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.2 Run Progress View enhancements`, `10.4 Worktree / diff affordances`, `14. Acceptance criteria / UI`
- Status: Partially Implemented
- Evidence Type: code, tests-run, screenshot
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1291-1334`
  - `Chainworks Forge/Views/ReleaseGateView.swift:21-320`
  - `Chainworks Forge/Views/RunsHomeView.swift:560-599`
  - `/tmp/p007-r2-ui.xcresult`
- Gap / Note: Focused macOS UI proof is now much stronger, but the repo-backed report/recovery/comparison/evidence-export surfaces were not all exercised on a real delivery run in this audit.

### REQ-014 Proposal 007 sign-off proof is closed on current HEAD

- Proposal Source: `12.3 Manual dogfood script`, `13. Testing strategy`, `14. Acceptance criteria / General and Product checkpoint`
- Status: Not Verifiable
- Evidence Type: tests-run, inference
- Evidence:
  - `/tmp/p007-r2-build.xcresult` — build succeeded
  - `/tmp/p007-r2-fullmvp.xcresult` — focused Full MVP suites passed
  - `/tmp/p007-r2-ui.xcresult` — focused UI slice passed with `1` skip
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f \( -name '*release*manifest*' -o -name '*git*push*receipt*' -o -name '*connect*upload*receipt*' -o -name '*delivery*receipt*' \)` — no matching files found
- Gap / Note: This audit did not prove one happy-path repo-backed run, one non-happy-path run, one exported evidence pack, or the “under 25 minutes fully inside the app” checkpoint. That is the remaining sign-off blocker.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 `SourceContextBuilder` remains unwired into repo-backed execution

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `Layer I`, `REQ-005`, `REQ-006`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/SourceContextBuilder.swift:3-42`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:36-70`
  - `rg -n "SourceContextBuilder|sourceContext" 'Chainworks Forge' 'Chainworks ForgeTests'`
- Why It Matters: Proposal 007 explicitly introduced `SourceContextBuilder` to make repo-backed writing/review context explicit instead of hidden cwd-driven. The worktree cwd issue is now fixed, but the richer diff/source contract is still not part of the runtime path.
- Recommended Action: Feed built source context into the implementation/review quartet and persist or attach it explicitly so repo-backed reviews are grounded in the promised diff context.

## Product Review

**Summary:** At Risk

### PROD-001 The core dogfood job is still not proven end-to-end from inside the app

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `Product question`, `REQ-009`, `REQ-010`, `REQ-011`, `REQ-012`, `REQ-014`
- Evidence Type: tests-run, inference
- Evidence:
  - `/tmp/p007-r2-fullmvp.xcresult`
  - `/tmp/p007-r2-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" ...`
- Why It Matters: Proposal 007 is specifically the first repo-backed dogfood slice. Structural tests and focused UI proof are necessary, but they are not the same as demonstrating one real repo-backed delivery session with receipts and exported evidence.
- Recommended Action: Run and capture one happy-path sample-repo session and one blocked-release/non-happy-path session, then export the evidence packs and attach them to sign-off.

## UI Review

**Summary:** Acceptable

### UI-001 Release Gate quick actions are not yet real actions

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: `10.3 Release Gate View`, `REQ-007`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/ReleaseGateView.swift:264-305`
- Why It Matters: The proposal promises operator-facing quick actions for proposal, diff, docs delta, review summary, and receipts. The current surface conveys context, but it still stops short of actionable navigation or open-file affordances.
- Recommended Action: Convert the quick-action rows into actual buttons or links that open the relevant artifacts/views directly from the release gate.

## UX Review

**Summary:** At Risk

### UX-001 Blocked-release recovery is described, but not yet proven as an operator flow

- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `9.4 Partial failure semantics`, `REQ-011`, `REQ-013`, `REQ-014`
- Evidence Type: code, tests-run, inference
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:648-655`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:712-719`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:328-377`
- Why It Matters: The proposal’s non-happy-path contract is about operator confidence after a real partial release failure. The model and failure policy now point the right direction, but this audit still lacks a proved blocked-release recovery path in the UI.
- Recommended Action: Add one focused runtime test or dogfood script that drives a blocked release, verifies preserved git receipts, re-enters the operator surface, and completes recovery or cancellation intentionally.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Dogfood sign-off evidence is still incomplete

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `13. Testing strategy`, `14. Acceptance criteria / Dogfooding`, `REQ-014`
- Evidence Type: tests-run, inference
- Evidence:
  - `/tmp/p007-r2-build.xcresult`
  - `/tmp/p007-r2-fullmvp.xcresult`
  - `/tmp/p007-r2-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" ...`
- Why It Matters: Proposal 007 explicitly locks sign-off to happy-path plus non-happy-path dogfood proof and exported evidence. That proof is still the difference between “feature slice exists” and “proposal is implemented”.
- Recommended Action: Close sign-off with one app-launched happy-path repo-backed run, one blocked-release/non-happy-path run, one exported evidence pack for each, and screenshot/receipt coverage for release gate plus final receipts.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `/tmp/p007-r2-build.xcresult` |
| Core user flow runtime-validated | Partial | Focused `Full MVP` suites passed; no full repo-backed in-app dogfood proof |
| Empty/loading/error states covered | Partial | Some approval/start/progress surfaces exercised; blocked-release runtime path not proven |
| Accessibility risk acceptable | Partial | macOS UI tests passed targeted surfaces, but sign-off dogfood flow still unproven |
| Localization risk acceptable | Not Checked | No localization-specific evidence in this audit |
| Critical tests executed | Partial | Focused `Full MVP` and UI slices executed; proposal-level dogfood sign-off still open |
| Privacy/permissions/entitlements reviewed | Not Checked | Out of scope for this focused implementation audit |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `date +%Y-%m-%dT%H:%M:%S%z`
- `sed -n '1,260p' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R1.md`
- `sed -n '480,590p' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `rg -n "under 25 minutes|Evidence Pack|xcodebuild build && xcodebuild test|happy-path|non-happy-path|worktree|ReleaseOpsCoordinator|ConnectPublishService|manual release|DeliveryConfiguration" docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `rg -n "ReleaseOpsCoordinator|executeRelease\\(" 'Chainworks Forge' 'Chainworks ForgeTests'`
- `rg -n "SourceContextBuilder|sourceContext" 'Chainworks Forge' 'Chainworks ForgeTests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r2-build-dd -resultBundlePath /tmp/p007-r2-build.xcresult build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r2-fullmvp-dd -resultBundlePath /tmp/p007-r2-fullmvp.xcresult test -only-testing:'Chainworks ForgeTests/FullMVPWorkflowTests' -only-testing:'Chainworks ForgeTests/FullMVPReleaseOpsTests' -only-testing:'Chainworks ForgeTests/FullMVPIntegrationTests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007-r2-ui-dd -resultBundlePath /tmp/p007-r2-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface'`
- `xcrun xcresulttool get test-results summary --path /tmp/p007-r2-fullmvp.xcresult`
- `xcrun xcresulttool get test-results summary --path /tmp/p007-r2-ui.xcresult`
- `find "$HOME/Library/Application Support/Chainworks Forge/runs" -type f \( -name '*release*manifest*' -o -name '*git*push*receipt*' -o -name '*connect*upload*receipt*' -o -name '*delivery*receipt*' \) | sort | tail -50`

## Recommended Next Actions

1. Drive one real repo-backed happy-path run from inside the app and export its evidence pack.
2. Drive one blocked-release/non-happy-path run and prove preserved git receipts plus operator-visible recovery.
3. Wire `SourceContextBuilder` into the repo-backed writer/reviewer runtime.
4. Turn release-gate quick-action rows into executable affordances.
5. Decide whether `DeliveryConfiguration` freeze should move into a shared run-creation boundary instead of staying Start Run UI-owned.
