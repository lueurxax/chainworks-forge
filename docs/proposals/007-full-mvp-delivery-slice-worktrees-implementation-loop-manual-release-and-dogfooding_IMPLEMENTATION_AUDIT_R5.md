# Proposal 007: Full MVP Delivery Slice — Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R5

| Field | Value |
|---|---|
| Proposal | docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md |
| Repository Root | . |
| Git SHA | fa31abc |
| Working Tree | dirty |
| Audited At | 2026-03-28T00:11:23+0200 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 007 remains `Partial`, but the architecture is materially stronger than in `R4`. The shared run-creation boundary now freezes delivery truth through `RunStartSnapshot` and applies it atomically inside `RunRepository.createRunFromPlan(...)`, and the runtime path now persists `delivery_receipt` from `DeliveryReceiptBuilder` instead of leaving it as demo data. The proposal still does not reach `Implemented`, because its sign-off-critical dogfood proof is red on the current tree: the fresh canonical repo-backed checkpoint UI bundle failed `2/2`, no fresh repo-backed delivery artifacts were found under the default run storage, and the full `xcodebuild test` rerun visibly reproduced UI failures before this audit closed.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | App-launched dogfood proof is still not closed | High |
| Architecture | Acceptable | Shared freeze boundary is fixed; remaining risk is proof depth, not design shape | High |
| Product | At Risk | Happy-path and non-happy-path repo-backed runs still do not complete from the real UI | High |
| UI | At Risk | Canonical checkpoint still breaks in the Ideas owner path before delivery proof can finish | High |
| UX | At Risk | Exported evidence remains a promised operator outcome but is not proven from a live repo-backed run | Medium |
| Readiness | Not Ready | Proposal-level sign-off gate is still red on current `HEAD` | High |

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

## Proposal Fidelity / Divergence

### Matches

- `RunStartSnapshot` now defines the shared immutable run-start boundary and includes frozen provider binding, workspace root, delivery configuration, and delivery preflight payloads.
- `RunRepository.createRunFromPlan(...)` now applies the start snapshot during run creation, and `RunPlanCompiler.createRun(...)` passes that snapshot through the shared persistence path.
- Focused tests in `FullMVPDeliveryTests` now assert that frozen delivery configuration and related run-start fields persist through creation and resume.
- The terminal delivery runtime path calls `DeliveryReceiptBuilder.buildReceipt(...)` and persists `delivery_receipt` as a real system artifact.
- `EvidencePackBuilder` still exports the promised delivery-side deliverables, and `RunsHomeView` still exposes `Export Evidence Pack` for completed/failed delivery runs.

### Divergences

- The fresh canonical repo-backed checkpoint UI run still fails both sign-off tests before it can prove a happy-path or non-happy-path evidence export.
- No fresh `delivery_receipt`, `release_manifest`, `git_push_receipt`, `connect_upload_receipt`, or `release_bundle_manifest` files were found in the default run-storage root during this audit.
- The broader `xcodebuild test` gate was visibly red in the fresh rerun before audit close, including non-Proposal-007 UI failures.

### Ambiguities / Evidence Gaps

- The focused unit attempt using narrow `-only-testing` selectors produced a non-proving `0`-test bundle, so requirement evidence for the new shared-boundary slice relies on direct code inspection plus proposal-scoped tests found in source.
- Desktop export enumeration is permission-limited in this environment, so exported evidence-pack discovery had to rely on the canonical UI test contract rather than direct Desktop listing.
- A full-scheme rerun was started and reproduced red UI failures, but it had not fully drained by the time this report was written.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 8 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state plan with explicit approval gates
- Proposal Source: `2. Product question this proposal must answer`, `11.2 Add full-mvp-live.yaml`, `14. Acceptance criteria / Workflow`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `examples/workflows/full-mvp-live.yaml`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Gap / Note: The topology and approval structure remain present and proposal-scoped tests still exist for the full MVP workflow.

### REQ-002 Start Run, delivery preflight, and run creation share one frozen `DeliveryConfiguration`
- Proposal Source: `6.4 Delivery configuration is a first-class boundary`, `9.6 Delivery preflight extends the provider-platform baseline`, `10.1 Dogfood Start Run preset`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift:3-38`
  - `Chainworks Forge/Models/RunRepository.swift:98-139`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:75-99`
  - `Chainworks Forge/Views/IdeaListView.swift:1341-1454`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift:463-557`
- Gap / Note: This was the major `R4` architecture blocker and now looks closed. The shared persistence boundary owns the frozen delivery snapshot instead of relying on post-creation UI mutation.

### REQ-003 The orchestrator provisions and persists one dedicated writable worktree before implementation begins
- Proposal Source: `7.1 Core rule`, `7.3 Persisted metadata`, `7.4 Provisioning rules`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
- Gap / Note: No regression signal reopened this slice in the current audit.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree
- Proposal Source: `7.5 No shared write worktrees`, `7.7 Path boundary enforcement`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Engine/RepoSafetyGuard.swift`
  - `Chainworks ForgeTests/DeliveryServicesTests.swift`
  - `Chainworks ForgeTests/WorktreeProvisionerTests.swift`
- Gap / Note: Guard code and tests exist, but this audit still did not prove end-to-end boundary enforcement across every live repo-backed file operation.

### REQ-005 Approved implementation agents execute against the real provisioned worktree and explicit source context
- Proposal Source: `3. What we build / Layer I`, `8. Implementation slice`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Engine/SourceContextBuilder.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift:1144-1205`
- Gap / Note: The repo-backed source-context injection test is present and still aligns with the promised runtime contract.

### REQ-006 The implementation review/refine loop persists required artifacts and can iterate until `Implemented`
- Proposal Source: `8.2 Continue until seemingly complete`, `8.3 Implementation reviewed against proposal`, `8.4 Implementation refined`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `examples/workflows/full-mvp-live.yaml`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Gap / Note: Workflow structure is present, but this audit still did not prove a complete repo-backed implementation/review/refine cycle from the real app UI.

### REQ-007 Manual release stays explicit and the operator gets a dedicated release-gate surface with sufficient context
- Proposal Source: `9.1 Release must remain explicit`, `10.3 Release Gate View`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift:1785-1809`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
- Gap / Note: The dedicated release-gate surface remains wired into the delivery approval path.

### REQ-008 Release side effects execute only through deterministic runtime services
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract`
- Status: Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Gap / Note: No audit evidence suggested the runtime has regressed back to free-form agent-driven release mechanics.

### REQ-009 Commit/push produces a real `release_manifest` and `git_push_receipt`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / GitReleaseService`
- Status: Partially Implemented
- Evidence Type: code, runtime, tests-run
- Evidence References:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift:83-114`
  - `/tmp/p007-r5-build.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" ...`
- Gap / Note: The runtime/export code now expects the right artifacts, but this audit found no fresh repo-backed run on disk that actually emitted those receipts.

### REQ-010 Archive/distribute produces a real `connect_upload_receipt` and `release_bundle_manifest`
- Proposal Source: `9.2 Release step sequence`, `9.3 Service contract / ConnectPublishService`
- Status: Partially Implemented
- Evidence Type: code, runtime
- Evidence References:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift:97-101`
  - `Chainworks Forge/Views/RunsHomeView.swift:587-594`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" ...`
- Gap / Note: Export wiring exists, but no fresh app-launched delivery run proved the archive/upload deliverables.

### REQ-011 Partial release failure preserves receipts and returns the run to blocked/operator recovery
- Proposal Source: `9.4 Partial failure semantics`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:1625-1695`
  - `Chainworks Forge/Engine/DeliveryReceiptBuilder.swift`
  - `Chainworks ForgeTests/OrchestratorTests.swift`
- Gap / Note: The runtime path now persists `delivery_receipt` for partial outcomes too, but the canonical non-happy-path app proof still fails before blocked-release evidence can be exported.

### REQ-012 The app exports a dogfood evidence pack with the promised delivery artifacts
- Proposal Source: `12.2 Evidence pack contents`, `14. Acceptance criteria / Dogfooding`
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence References:
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift:18-187`
  - `Chainworks Forge/Views/RunsHomeView.swift:587-594`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:109-205`
- Gap / Note: The export contract is implemented in code and in canonical UI assertions, but the fresh end-to-end export proof is still red.

### REQ-013 Happy-path and non-happy-path repo-backed runs can be completed from inside the app with exported evidence
- Proposal Source: `12.4 Dogfood proof`, `14. Acceptance criteria / Dogfooding`
- Status: Partially Implemented
- Evidence Type: tests-run, runtime
- Evidence References:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:987-1115`
  - `Chainworks ForgeUITests/IdeasScreen.swift:45-73`
  - `/tmp/p007-r5-ui.xcresult`
- Gap / Note: The canonical app-launched checkpoint is present and authoritative, but it is still failing on the current tree: happy-path timed out while synthesizing input, and non-happy-path could not create the idea from the real UI.

### REQ-014 `xcodebuild build && xcodebuild test` is green with no regressions in earlier slices
- Proposal Source: `14. Acceptance criteria / Testing and sign-off`
- Status: Partially Implemented
- Evidence Type: tests-run, runtime
- Evidence References:
  - `/tmp/p007-r5-build.xcresult`
  - `/tmp/p007-r5-ui.xcresult`
  - fresh `xcodebuild test` rerun output in this audit session
- Gap / Note: Build is green, but test is not. The focused canonical checkpoint bundle failed `2/2`, and the fresh full-scheme rerun reproduced additional UI failures before audit close.

## Expert Findings

### UI-001 Canonical repo-backed checkpoint still breaks in the Ideas owner path
- Severity: Major
- Confidence: High
- Related Proposal Items: REQ-013, REQ-014
- Evidence Type: tests-run
- Evidence References:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:987-1115`
  - `Chainworks ForgeUITests/IdeasScreen.swift:45-73`
  - `/tmp/p007-r5-ui.xcresult`
- Why It Matters: Proposal 007’s sign-off is not abstract. It explicitly requires one happy-path and one non-happy-path run from inside the app. Both fresh checkpoint tests still fail before the operator can prove the end-to-end repo-backed flow.
- Recommended Action: Fix the real `Ideas -> New Idea -> Start Run` owner path until both canonical checkpoint tests pass and export their evidence packs.

### READY-001 Proposal-level dogfood sign-off remains red
- Severity: Major
- Confidence: High
- Related Proposal Items: REQ-013, REQ-014
- Evidence Type: tests-run, runtime
- Evidence References:
  - `/tmp/p007-r5-build.xcresult`
  - `/tmp/p007-r5-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" ...`
- Why It Matters: Even with the shared-boundary and receipt-builder fixes, the proposal cannot be called implemented until a repo-backed delivery run actually produces its promised artifacts and evidence pack from the app.
- Recommended Action: Re-run the canonical happy-path and non-happy-path flows after the Ideas owner-path fix and verify fresh delivery artifacts exist either in run storage or in the exported evidence pack.

### READY-002 Broad regression gate is still not clean
- Severity: Major
- Confidence: Medium
- Related Proposal Items: REQ-014
- Evidence Type: tests-run
- Evidence References:
  - fresh `xcodebuild test` rerun output in this audit session
- Why It Matters: Proposal 007 explicitly requires no regressions in earlier slices. The broad rerun already reproduced additional UI failures outside the canonical checkpoint, so the repo is not yet at a clean sign-off point even if the P007-specific architecture is healthier.
- Recommended Action: Finish the broad regression cleanup after the checkpoint flow is stable, then re-run `xcodebuild test` to a fully green result.

## Roll-Up

- Shared delivery freeze no longer lives only in the Start Run UI. That was the biggest architecture divergence in `R4`, and it now looks fixed.
- Runtime delivery receipts are now generated from the real delivery path, so the artifact contract is much closer to the proposal.
- The remaining blocker is not missing scaffolding. It is live operator proof: current repo-backed checkpoint UI tests still fail, and the proposal’s sign-off gate is therefore still open.

## Suggested Next Audit Trigger

Run `R6` only after all three are true:

1. `/tmp/p007-r5-ui.xcresult` style canonical checkpoint rerun passes `2/2`.
2. A fresh repo-backed run produces delivery artifacts and an exported evidence pack.
3. A fresh full `xcodebuild test` rerun reaches green.
