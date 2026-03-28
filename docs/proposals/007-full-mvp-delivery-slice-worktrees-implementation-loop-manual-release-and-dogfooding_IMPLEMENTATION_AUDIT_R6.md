# Proposal 007: Full MVP Delivery Slice — Dedicated Worktrees, Implementation Loop, Manual Release, and Dogfooding Implementation Audit R6

| Field | Value |
|---|---|
| Proposal | docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md |
| Repository Root | . |
| Git SHA | fa31abc |
| Working Tree | dirty |
| Audited At | 2026-03-28T15:23:06+0200 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 007 is now materially stronger than `R5`. Fresh app-launched dogfood proof against clean local sample repositories completed both a `happy_path` run and a `non_happy_path` run from inside the app, exported real evidence packs, and produced the expected delivery receipts/manifests. The proposal still does not reach `Implemented`, because its explicit repository-baseline sign-off gate is red on current `HEAD`: fresh full-scheme `xcodebuild test` failed at `/tmp/p007-r6-full.xcresult` (`95` failed, `206` passed, `5` skipped), and the current self-repo dogfood path still blocks at worktree provisioning with a repository-identity mismatch before implementation can start.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Full repository sign-off gate is still red | High |
| Architecture | Acceptable | Repo identity normalization is brittle for the current self-repo dogfood path | High |
| Product | At Risk | Sample-repo dogfood works, but the current self-repo path still blocks before implementation | High |
| UI | Acceptable | Fresh screenshot-based checkpoint proof was not regenerated because the full UI runner failed to initialize | Medium |
| UX | At Risk | Operators can complete the sample-repo flow, but the current self-profile path still fails with a low-level repo-identity error | Medium |
| Readiness | Not Ready | Proposal-required `xcodebuild build && xcodebuild test` sign-off is not green | High |

## Proposal Contract

### Scope

- Deliver the first full repo-backed 12-state workflow from idea through completed release candidate inside the app.
- Freeze `DeliveryConfiguration` before execution and carry it through Start Run, preflight, run creation, and resume.
- Provision one dedicated writable worktree per run before the first implementation write.
- Keep release side effects explicit and deterministic.
- Export a dogfood evidence pack for both happy-path and non-happy-path repo-backed runs.

### Locked Decisions

- One run equals one dedicated writable worktree.
- No concurrent writable agents may share a writable worktree.
- Release mechanics execute through deterministic services, not free-form agent shelling.
- `full-mvp-live.yaml` remains a dedicated repo-backed preset.
- Approval gates remain explicit workflow states.
- Default release targets stay sandbox/staging, not production.
- Partial release failure returns to blocked/operator recovery instead of hidden rollback.

### Primary User Flows

1. Start a `Full MVP Live` repo-backed run with frozen delivery configuration and passing delivery preflight.
2. Approve the proposal and move into a dedicated worktree-backed implementation slice.
3. Review implementation, loop/refine as needed, then reach an explicit manual release gate.
4. Complete a happy-path or blocked-release dogfood session and export a reviewable evidence pack.

### UI Commitments

- Start Run supports the `Full MVP Live` preset and delivery-specific context.
- Run Progress exposes repo/worktree-aware progress.
- Release Gate View surfaces enough context for informed approval.
- Existing report/recovery/comparison surfaces continue working for repo-backed runs.

### UX Commitments

- Manual approval gates remain visible and explicit.
- Release failure returns to an operator-visible blocked state with preserved receipts.
- A single engineer can complete both happy-path and non-happy-path dogfood sessions without guessing.

### Acceptance Criteria

- Dedicated writable worktree is provisioned and persisted before the first implementation write.
- `full-mvp-live.yaml` compiles into a valid executable 12-state plan.
- Implementation review produces the required review artifacts.
- Manual release blocks on explicit approval.
- Deterministic release services emit durable receipts/manifests.
- Happy-path and non-happy-path repo-backed runs complete inside the app with exported evidence.
- `xcodebuild build && xcodebuild test` is green with no regressions.

### Test / Evidence Requirements

- One full happy-path dogfood run.
- One non-happy-path run.
- Exported evidence packs.
- Screenshot proof for release-gate/final-receipt review.
- Green `xcodebuild build && xcodebuild test`.

### Explicit Exclusions

- Proposal 007 does not replace the operator-shell or provider-platform baselines.
- Proposal 007 does not introduce production release targets.
- Proposal 007 does not require a full repo browser or multi-repo orchestration surface.

## Proposal Fidelity / Divergence

### Matches

- Shared run-creation freeze is still owned by `RunStartSnapshot` and applied in `RunRepository.createRunFromPlan(...)`.
- Fresh sample-repo `happy_path` app-launched proof completed end-to-end with a dedicated worktree, manual release, and exported delivery receipts.
- Fresh sample-repo `non_happy_path` app-launched proof blocked at manual release with preserved partial receipts and exported evidence.
- `DeliveryReceiptBuilder` is now on the real runtime path and its output appears in exported dogfood deliverables.
- `EvidencePackBuilder` exports the delivery-side packet the proposal promised, including delivery configuration, preflight, stage summary, agent detail, and release deliverables.

### Divergences

- The proposal’s repository-baseline sign-off gate is still red: `/tmp/p007-r6-full.xcresult` failed with broad regressions, including guardrail failures and UI-runner initialization failure.
- The current self-repo dogfood path still blocks before worktree provisioning because `repoIdentifier` is frozen as the folder name while the provisioner resolves a different repository identity from Git.
- Fresh local full-suite UI proof did not produce release-gate/final-receipt screenshots because the UI runner failed to initialize.

### Ambiguities / Evidence Gaps

- The repository now documents remote-host policy for UI proof, but this audit did not have a repository-defined remote host endpoint to use for a replacement screenshot run.
- The sample-repo app-launched harness closes the generic dogfood contract, but the current self-profile path remains a distinct product/readiness defect on this tree.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 `full-mvp-live.yaml` compiles into the promised 12-state executable plan
- Proposal Source: `5. Canonical live workflow for Proposal 007` (`...:213-249`), `14. Acceptance criteria / Workflow` (`...:1003-1010`)
- Status: Implemented
- Evidence Type: code, tests-run, runtime
- Evidence References:
  - `examples/workflows/full-mvp-live.yaml`
  - `/tmp/p007-r6-full.xcresult`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/stage-summary.json`
- Gap / Note: The sample-repo happy-path run reached the expected repo-backed stages and the full suite still exercised Proposal-007-specific workflow tests before the broader gate failed.

### REQ-002 Start Run, delivery preflight, run creation, and resume share one frozen `DeliveryConfiguration`
- Proposal Source: `6.4 Delivery configuration is a first-class boundary` (`...:323-356`), `9.6 Delivery preflight extends the provider-platform baseline` (`...:716-733`), `10.1 Dogfood Start Run preset` (`...:741-770`)
- Status: Implemented
- Evidence Type: code, runtime
- Evidence References:
  - `Chainworks Forge/Engine/RunStartSnapshot.swift`
  - `Chainworks Forge/Models/RunRepository.swift`
  - `Chainworks Forge/Views/IdeaListView.swift:1709-1722`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/delivery-configuration.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/delivery-preflight.json`
- Gap / Note: The exported sample-repo evidence pack now proves the frozen configuration survives into the live app-launched run.

### REQ-003 One dedicated writable worktree is provisioned and persisted before the first implementation write
- Proposal Source: `7.1 Core rule` (`...:382-391`), `7.3 Persisted metadata` (`...:409-439`), `14. Acceptance criteria / Runtime / worktree` (`...:997-1002`)
- Status: Implemented
- Evidence Type: runtime, tests-run
- Evidence References:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/run-metadata.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/run-metadata.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/stage-summary.json`
- Gap / Note: Both sample-repo app-launched runs now persist a real `worktreeRoot` before implementation/release activity.

### REQ-004 Repo safety guards enforce path boundaries, repo identity, and no shared writable worktree
- Proposal Source: `7.5 No shared write worktrees` (`...:452-457`), `7.7 Path boundary enforcement` (`...:472-479`), `14. Acceptance criteria / Runtime / worktree` (`...:997-1002`)
- Status: Partially Implemented
- Evidence Type: code, tests-run, runtime
- Evidence References:
  - `Chainworks Forge/Engine/RepoSafetyGuard.swift`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:82-89`
  - `/tmp/p007-r6-full.xcresult`
  - `/tmp/p007-r6-happy/exports/evidence-pack-2CF43EA1/stage-summary.json`
- Gap / Note: Guard code exists and sample-repo runs succeed, but the current self-repo dogfood path still fails on repository-identity matching before worktree provisioning, so the identity contract is not robust across intended dogfood targets.

### REQ-005 Approved implementation agents execute against the real provisioned worktree with explicit source context
- Proposal Source: `3. What we build / Layer I` (`...:128-139`), `8. Implementation slice` (`...:483-589`)
- Status: Implemented
- Evidence Type: tests-run, runtime
- Evidence References:
  - `/tmp/p007-r6-full.xcresult`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/stage-summary.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/diff-summary.json`
- Gap / Note: The full suite reported `Repo-backed execution injects source context into agent inputs` as passing before the broader gate failed, and the sample-repo happy-path run completed implementation/review on a real provisioned worktree.

### REQ-006 The implementation review/refine loop persists required artifacts and can iterate until `Implemented`
- Proposal Source: `8.2 Continue until seemingly complete` (`...:510-526`), `8.3 Implementation reviewed against proposal` (`...:527-560`), `8.4 Implementation refined` (`...:562-589`)
- Status: Partially Implemented
- Evidence Type: runtime, tests-found
- Evidence References:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/docs-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/audit-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/security-report.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/prepush-review-report.json`
  - `Chainworks ForgeTests/FullMVPDeliveryTests.swift`
- Gap / Note: Required implementation-review artifacts are now exported from a live app-launched run, but this audit did not directly prove a live refine-loop re-entry before the full repository gate failed.

### REQ-007 Manual release remains explicit and a dedicated release-gate surface exists
- Proposal Source: `9.1 Release must remain explicit` (`...:595-612`), `10.3 Release Gate View` (`...:787-809`)
- Status: Implemented
- Evidence Type: code, runtime
- Evidence References:
  - `Chainworks Forge/Views/ReleaseGateView.swift`
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
- Gap / Note: Both sample-repo app-launched runs recorded `approvalCount = 3`, confirming the explicit manual gates remain on the live path.

### REQ-008 Release side effects execute only through deterministic runtime services
- Proposal Source: `9.2 Release step sequence` (`...:613-631`), `9.3 Service contract` (`...:632-678`), `15. Locked decisions / ARCH-069` (`...:1047-1055`)
- Status: Implemented
- Evidence Type: code, tests-run, runtime
- Evidence References:
  - `Chainworks Forge/Engine/GitReleaseService.swift`
  - `Chainworks Forge/Engine/ConnectPublishService.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `/tmp/p007-r6-full.xcresult`
- Gap / Note: The fresh full-suite run explicitly reported `Release side effects execute only through deterministic services` as passing before the broader gate failed.

### REQ-009 Commit/push emits `git_push_receipt` and `release_manifest`
- Proposal Source: `9.2 Release step sequence` (`...:615-631`), `9.3 Service contract / GitReleaseService` (`...:634-657`), `14. Acceptance criteria / Release` (`...:1011-1016`)
- Status: Implemented
- Evidence Type: runtime
- Evidence References:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/git-push-receipt.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/release-manifest.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/git-push-receipt.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/release-manifest.json`
- Gap / Note: Fresh app-launched happy-path and non-happy-path sample-repo evidence now includes both release-side Git deliverables.

### REQ-010 Archive/distribute emits `connect_upload_receipt` and `release_bundle_manifest`
- Proposal Source: `9.2 Release step sequence` (`...:623-631`), `9.3 Service contract / ConnectPublishService` (`...:658-678`), `14. Acceptance criteria / Release` (`...:1011-1016`)
- Status: Implemented
- Evidence Type: runtime
- Evidence References:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/connect-upload-receipt.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974/deliverables/release-bundle-manifest.json`
- Gap / Note: The happy-path sample-repo run now proves the archive/distribute deliverables on the real app-launched path.

### REQ-011 Partial release failure preserves receipts and returns the run to blocked/operator recovery
- Proposal Source: `9.4 Partial failure semantics` (`...:680-701`), `14. Acceptance criteria / Release and Dogfooding` (`...:1011-1030`)
- Status: Implemented
- Evidence Type: runtime
- Evidence References:
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/run-metadata.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/delivery-receipt.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9/deliverables/git-push-receipt.json`
- Gap / Note: The sample-repo non-happy-path run ends blocked with preserved partial delivery receipts and no connect-upload receipt.

### REQ-012 The app exports a dogfood evidence pack with the promised delivery artifacts
- Proposal Source: `12.2 Evidence pack builder` (`...:898-925`), `14. Acceptance criteria / Dogfooding` (`...:1026-1030`)
- Status: Implemented
- Evidence Type: runtime
- Evidence References:
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
- Gap / Note: Both fresh sample-repo runs exported full evidence packs with delivery configuration, preflight, stage summary, agent detail, and the expected deliverables.

### REQ-013 Happy-path and non-happy-path repo-backed runs complete from inside the app with exported evidence
- Proposal Source: `12.3 Manual dogfood script` (`...:927-942`), `13.3 Evidence-based review requirement` (`...:983-991`), `14. Acceptance criteria / Dogfooding` (`...:1026-1030`)
- Status: Implemented
- Evidence Type: runtime
- Evidence References:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
- Gap / Note: The requirement is now directly proven for the sample-repo path. The current self-repo path is still broken, but that is a separate product/readiness defect rather than the absence of a working dogfood path.

### REQ-014 `xcodebuild build && xcodebuild test` is green with no regressions in earlier slices
- Proposal Source: `14. Acceptance criteria / General` (`...:1032-1034`)
- Status: Partially Implemented
- Evidence Type: tests-run, runtime
- Evidence References:
  - `/tmp/p007-r6-build.xcresult`
  - `/tmp/p007-r6-full.xcresult`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:879`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:164`
- Gap / Note: Build is green, but full test is not. The fresh full-suite summary reported `95` failed tests, including `RunTests/noDirectRunConstruction()`, `SimulatedAgentExecutorTests/explicitOutputContractOnlyAppliesToMatchingOutput()`, broad trap-crash failures, and a UI-runner initialization error caused by `Authentication cancelled`.

## Architecture Review

**Summary:** Acceptable

### ARCH-001 Self-repo dogfood path still freezes a brittle repository identity
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-013
- Evidence Type: code, runtime
- Evidence References:
  - `Chainworks Forge/Engine/Proposal007DogfoodHarness.swift:193-206`
  - `Chainworks Forge/Views/IdeaListView.swift:1713-1718`
  - `Chainworks Forge/Engine/WorktreeProvisioner.swift:82-89`
  - `/tmp/p007-r6-happy/exports/evidence-pack-2CF43EA1/stage-summary.json`
  - `/tmp/p007-r6-nonhappy/exports/evidence-pack-BA1D1BC0/stage-summary.json`
- Why It Matters: The live app still freezes `repoIdentifier` as the directory basename for the current repo (`Chainworks Forge`), while worktree provisioning validates against a different Git-derived identity (`https://github.com/lueurxax/chainworks-forge.git`). That blocks the current self-repo dogfood path before worktree creation.
- Recommended Action: Canonicalize repository identity across `DeliveryConfiguration`, sample-profile production, and worktree provisioning so basename, slug, and remote-origin forms resolve to one stable value.

## Product Review

**Summary:** At Risk

### PROD-001 Sample-repo dogfood works, but the current self-repo path still fails before implementation
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-003, REQ-004, REQ-013
- Evidence Type: runtime
- Evidence References:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-happy/result.json`
  - `/tmp/p007-r6-nonhappy/result.json`
- Why It Matters: The product now has a believable sample-repo dogfood loop, but the same app-launched path still blocks immediately when pointed at the current checkout. That makes the operator experience uneven and leaves the “self-dogfood” story brittle.
- Recommended Action: Fix the repo-identity mismatch in the current self-profile path, then rerun the app-launched harness against the repository checkout in addition to the clean sample repo.

## UI Review

**Summary:** Acceptable

No new UI-only contract defect surfaced beyond the readiness evidence gaps below. The repo-backed operator surfaces already exist; this round’s blocking issue was not layout or hierarchy, but the inability to regenerate fresh UI screenshot proof because the full UI runner failed to initialize.

## UX Review

**Summary:** At Risk

### UX-001 The blocked self-repo path currently fails with a low-level provisioning error instead of a dogfood-ready operator journey
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: REQ-004, REQ-013
- Evidence Type: runtime
- Evidence References:
  - `/tmp/p007-r6-happy/exports/evidence-pack-2CF43EA1/stage-summary.json`
  - `/tmp/p007-r6-nonhappy/exports/evidence-pack-BA1D1BC0/stage-summary.json`
- Why It Matters: Operators do get a blocked state, but the current self-repo path fails with an internal repository-identity mismatch message before the flow reaches a believable implementation/release journey. That is recoverable for an engineer, but not the low-drama dogfood experience the proposal aims for.
- Recommended Action: Normalize the repo identity contract and add a clearer operator-facing preflight/recovery message when a self-repo target does not satisfy worktree identity checks.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The repository-wide sign-off gate is still red
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-014
- Evidence Type: tests-run
- Evidence References:
  - `/tmp/p007-r6-build.xcresult`
  - `/tmp/p007-r6-full.xcresult`
- Why It Matters: Proposal 007 explicitly requires `xcodebuild build && xcodebuild test` to be green. The fresh full-suite summary is not close: `95` failed, `206` passed, `5` skipped, `result = Failed`.
- Recommended Action: Restore the repository baseline first. The immediate blockers surfaced this round are `RunTests/noDirectRunConstruction()`, `SimulatedAgentExecutorTests/explicitOutputContractOnlyAppliesToMatchingOutput()`, broad trap-crash failures across SwiftData-backed tests, and the UI-runner initialization failure.

### READY-002 Fresh screenshot-based sign-off proof is still incomplete
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: REQ-013, REQ-014
- Evidence Type: tests-run, runtime
- Evidence References:
  - `/tmp/p007-r6-full.xcresult`
  - `docs/reference/agent-ui-test-execution.md:127-132`
- Why It Matters: The sample-repo app-launched harness closes the delivery-artifact proof, but the proposal’s review evidence also calls for release-gate/final-receipt screenshots. This audit did not regenerate those screenshots because the full UI runner failed to initialize with `Authentication cancelled`.
- Recommended Action: Rerun the focused repo-backed UI checkpoint on a clean macOS host after resolving the LocalAuthentication blocker, or use the repository-approved remote-Mac path once an explicit host endpoint is available for the audit run.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | Fresh build succeeded at `/tmp/p007-r6-build.xcresult`. |
| Core user flow runtime-validated | Partial | Sample-repo happy/non-happy app-launched runs succeeded, but the current self-repo path still blocks at stage 7 on repository identity mismatch. |
| Empty/loading/error states covered | Partial | Blocked release path is proven on the sample repo; current self-repo failure path is not yet a polished operator journey. |
| Accessibility risk acceptable | Not Checked | Fresh UI runner proof failed to initialize before meaningful UI assertions. |
| Localization risk acceptable | Not Checked | No localization-specific validation ran in this audit. |
| Critical tests executed | Partial | Full repository test gate ran and failed; sample-repo dogfood runtime proof succeeded. |
| Privacy/permissions/entitlements reviewed | Partial | UI automation is currently blocked by LocalAuthentication cancellation on the local host. |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n '^#|^##|^###|Acceptance Criteria|Definition of Done|DoD|dogfood|full-mvp-live|Evidence Pack|Proposal 007' docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md`
- `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -derivedDataPath /tmp/p007-r6-build-DD -resultBundlePath /tmp/p007-r6-build.xcresult build`
- `xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -derivedDataPath /tmp/p007-r6-full-DD -resultBundlePath /tmp/p007-r6-full.xcresult test`
- `xcrun xcresulttool get test-results summary --path /tmp/p007-r6-full.xcresult`
- App-launched dogfood harness against the current repo checkout:
  - `happy_path` → `/tmp/p007-r6-happy/result.json`
  - `non_happy_path` → `/tmp/p007-r6-nonhappy/result.json`
- App-launched dogfood harness against fresh local sample repos:
  - `happy_path` → `/tmp/p007-r6-sample-happy/result.json`, `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `non_happy_path` → `/tmp/p007-r6-sample-nonhappy/result.json`, `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`

## Recommended Next Actions

1. Restore the repository baseline until `/tmp/p007-r6-full.xcresult` is green. The immediate blockers are `RunTests/noDirectRunConstruction()`, `SimulatedAgentExecutorTests/explicitOutputContractOnlyAppliesToMatchingOutput()`, and the trap-crash cluster in the full suite.
2. Canonicalize repo identity across `DeliveryConfiguration`, dogfood harness/profile generation, and `WorktreeProvisioner` so the current self-repo path provisions a worktree instead of blocking on `"Chainworks Forge"` vs remote-URL identity mismatch.
3. Regenerate fresh release-gate/final-receipt screenshot proof on a clean UI host after resolving the LocalAuthentication test-runner failure.
