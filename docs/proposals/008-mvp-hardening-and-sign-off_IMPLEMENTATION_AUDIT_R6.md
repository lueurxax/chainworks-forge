# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T18:32:33+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `At Risk` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 is now `Partial` on current `HEAD`. The key contract regressions that kept `R5` red are materially fixed on this same SHA: the approval-gate relaunch path is green again in fresh runtime proof, `CompletedRunExportHub` now derives `Exported` from persisted `evidencePackExportedAt` truth instead of receipt heuristics, and the persisted/exported benchmark model now preserves the `controlled_sample` vs `real_world` repository distinction. Fresh targeted proof is strong where the old blockers lived: [`/tmp/p008-r6-resume2.xcresult`](/tmp/p008-r6-resume2.xcresult) passed `10/10`, including `executionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage()`, and [`/tmp/p008-r6-delivery.xcresult`](/tmp/p008-r6-delivery.xcresult) passed `14/14`.

The proposal still is not fully implemented because two explicit contracts remain only partially proven. First, `REQ-012` is still incomplete: `OutputRetrievalSLOProbe` exists and `RunReportView` now has loading/error/empty states, but the report/export surfaces still do not provide explicit timeout and retry behavior. Second, `REQ-016` remains only partially evidenced on current `HEAD`: screenshot-bearing UI tests exist, but this audit did not freshly execute the remote UI proof for recovery/re-entry/export states. I also attempted the canonical local build gate via `./scripts/test-gate.sh build`, but the script refused to start because unrelated app/test processes were already active in the environment; that was an environment guardrail failure, not a code failure.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Report/export surfaces still miss explicit timeout/retry UX promised by the proposal | High |
| Architecture | Good | Benchmark/export truth is now materially aligned, but fresh end-to-end proof for the 008 UI checkpoint was not replayed | High |
| Product | At Risk | The sign-off loop is much closer, but screenshot-proof of the final recovery/export UX still lags current `HEAD` | Medium |
| UI | At Risk | `RunReportView` still has no explicit timeout/retry affordances | High |
| UX | At Risk | Failed report retrieval still degrades to a generic load error with no recovery action | High |
| Readiness | At Risk | Fresh targeted unit proof is green, but current-head UI proof was not replayed in this audit | High |

## Proposal Contract

### Scope

- Freeze the final MVP boundary after Proposal 007.
- Persist benchmark and sign-off state outside the operational `Run` aggregate.
- Evaluate `GO/HOLD` only from persisted benchmark records.
- Harden recovery, export, and sign-off UX inside the current shell.
- Require one happy-path and one recovered non-happy-path evidence pack before MVP sign-off.

### Locked Decisions

- Proposal 007 must already be implemented and review-proven on current `HEAD`.
- The canonical MVP provider set is `codex`, `claude_code`, and `gemini`.
- Benchmark/sign-off state lives outside `Run`.
- Attachments remain `reference_only` / `rejected`.
- Recovery, export, and sign-off remain shell-owned subordinate routes.
- MVP sign-off is an explicit `GO/HOLD` gate.

### Primary User Flows

1. Define and persist a fixed benchmark cohort spanning one controlled sample repo and one real-world repo.
2. Record manual-baseline and app-driven benchmark pairs for the same ideas.
3. Restore blocked or approval-paused repo-backed runs without raw-log archaeology.
4. Export a trustworthy completed-run packet and a replayable sign-off packet from the app.
5. Decide `GO/HOLD` only from persisted benchmark records plus complete exported evidence.

### UI Commitments

- Shell-owned blocked recovery surface.
- Completed-run export hub inside `RunReportView`.
- Embedded sign-off summary surface inside the current report context.
- Visible evidence-pack status on completed benchmark runs.
- Screenshot-tested recovery, re-entry, and export states.

### UX Commitments

- No silent continuation after relaunch at approval gates.
- Completed-run overview stays calm while the export hub carries deeper receipt detail.
- Attachment language stays truthful.
- Operators should not need raw-log archaeology for blocked benchmark recovery.

### Acceptance Criteria

- Proposal 007 prerequisite is green on current `HEAD`.
- Benchmark cohort and manual-vs-app protocol are fixed and repeatable.
- Every benchmark run captures proposal approval, implementation approval, release decision, and total elapsed time.
- Manual baselines and app-driven runs persist as immutable benchmark pairs.
- Final `GO/HOLD` evaluation uses only persisted benchmark records.
- Exported sign-off packet is replayable without external notes.
- Attachment policy, cost policy, approval-gate relaunch behavior, and output/report SLO are fixed.
- Blocked recovery/export/sign-off are shell-owned and screenshot-tested.
- At least one happy-path and one recovered non-happy-path evidence pack exist.
- MVP sign-off cannot pass without complete exported review packets.

### Explicit Exclusions

- Forge Steward activation.
- Backend extraction / Temporal migration.
- Provider families beyond `codex`, `claude_code`, and `gemini`.
- Autonomous recovery.
- Automatic attachment ingestion into agent context.

## Proposal Fidelity / Divergence

### Matches

- Proposal 007 remains implemented on the same SHA in [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md](007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md).
- `MVPBoundaryPolicy` still freezes the canonical three-provider MVP boundary and attachment truth.
- Benchmark/sign-off state still lives outside `Run`.
- `RunRepository.createRunFromPlan(...)` still assigns `run.experimentCohortID`.
- `ExecutionService` still records benchmark executions from the live completion path.
- `MVPSignOffEvaluator` still blocks `GO` when an app-driven benchmark record lacks `evidencePackExportedAt`.
- `ResumeManager` now restores approval-gate runs even when source drift exists.
- `CompletedRunExportHub` now derives `Exported` strictly from persisted `evidencePackExportedAt` for benchmark-linked runs.
- The persisted/exported cohort model now carries repository type truth.

### Divergences

- `RunReportView` still has no explicit timeout state or retry control despite the proposal’s report/export SLO contract.
- Recovery/re-entry/export screenshot proof was not freshly replayed on current `HEAD` in this audit.

### Ambiguities / Evidence Gaps

- The happy-path and non-happy-path app-launched evidence packs remain same-`HEAD` accepted artifacts inherited from Proposal 007 in `/tmp/p007-r6-sample-*`, not a newly refreshed Proposal 008 cohort rerun.
- The canonical `./scripts/test-gate.sh build` gate could not be refreshed in this audit because the script detected unrelated active processes (`debugserver`, external agent session) and refused to start.
- I did not find new dedicated tests for `CompletedRunExportHub` status derivation or sign-off packet repository-type serialization; those contracts are proven here by direct code inspection plus the existing targeted service/runtime slices.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 18 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence
- Proposal Source: `1.1 Hard prerequisite from Proposal 007`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: runtime, inference
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
- Gap / Note: The explicit upstream blocker remains closed on the same SHA.

### REQ-002 The canonical MVP provider set is frozen to `codex`, `claude_code`, and `gemini` across repo policy/docs
- Proposal Source: `4. Frozen MVP boundary`, `9. Acceptance criteria / Boundary freeze`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/provider-platform.md`
  - `docs/ps/chainworks-forge-mvp.md`
- Gap / Note: Repo docs and runtime policy remain aligned to the three-provider MVP set.

### REQ-003 Benchmark/sign-off state lives outside the operational `Run` aggregate and remains linked to runs by ID
- Proposal Source: `5.2 Persisted benchmark and sign-off model`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/BenchmarkCohort.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
  - `Chainworks Forge/Models/BenchmarkPair.swift`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
- Gap / Note: The persistence split promised by the proposal still exists in the live schema.

### REQ-004 The benchmark cohort contract is fixed to two repositories and six ideas with one real-world repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
- Gap / Note: The fixed cohort definition still encodes two repositories, six ideas, and a required `real_world` profile.

### REQ-005 Manual baselines and app-driven benchmark records are written only as persisted benchmark records with immutable pairs
- Proposal Source: `3. Layer K`, `5.2 Persisted benchmark and sign-off model`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ManualBaselineImport.swift`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift`
  - `Chainworks Forge/Models/BenchmarkPair.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: Benchmark services still operate on benchmark-side records, not launch-governance state on `Run`.

### REQ-006 App-driven benchmark runs are actually linked to a cohort and recorded from the live runtime path
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.3 Required measurements`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/RunRepository.swift`
  - `Chainworks Forge/Engine/ExecutionService.swift`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift`
- Gap / Note: The shared run-creation path still assigns cohort identity, and the live completion path still records benchmark executions.

### REQ-007 The evaluator computes `GO/HOLD` only from persisted benchmark records and persists a replayable snapshot checksum
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.6 Sign-off gate`, `5.7 Required sign-off summary payload`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
- Gap / Note: The evaluator still reads persisted benchmark records only and persists checksum-backed decision snapshots.

### REQ-008 The app can export a replayable sign-off packet from the shell-owned report/sign-off flow
- Proposal Source: `5.7 Required sign-off summary payload`, `7.4 Sign-off summary surface`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The sign-off route still exports through the dedicated builder from the shell-owned sign-off surface.

### REQ-009 Attachments are validated as reference-only/rejected and those states are visible before run start
- Proposal Source: `6.1 Attachment policy`, `9. Acceptance criteria / PS closure`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
- Gap / Note: Attachment truth remains deterministic and visible as `reference_only` / `rejected`.

### REQ-010 Completed-run overview shows total cost while the export hub exposes deeper receipt breakdown
- Proposal Source: `6.2 Cost granularity`, `7.3 Completed-run export hub`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The dominant summary and subordinate receipt breakdown remain aligned with the proposal hierarchy.

### REQ-011 Relaunch at an approval gate restores visible `waiting_approval` context with no silent continuation
- Proposal Source: `6.3 Relaunch behavior at approval gate`, `7.1 Shell ownership is explicit`
- Status: Implemented
- Evidence Type: tests-run, code
- Evidence:
  - `Chainworks Forge/Engine/ResumeManager.swift:88-105`
  - `Chainworks ForgeTests/ResumeManagerTests.swift:331-408`
  - `/tmp/p008-r6-resume2.xcresult`
- Gap / Note: Fresh current-head runtime proof is now green. `ResumeManager` preserves approval-gate restoration through drift, and the old failing test now passes without re-executing the paused stage.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift`
  - `Chainworks Forge/Views/RunReportView.swift:19-23`
  - `Chainworks Forge/Views/RunReportView.swift:127-164`
  - `Chainworks Forge/Views/RunReportView.swift:201-232`
- Gap / Note: The probe computes `p50/p95/p99`, and `RunReportView` now has explicit loading/error/empty states. The promised timeout/retry behavior still is not present on the report/export surfaces.

### REQ-013 Blocked implementation/release recovery is available from one shell-owned visible surface
- Proposal Source: `7.1 Shell ownership is explicit`, `7.2 Blocked review / release re-entry`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: The recovery route remains subordinate to the current shell.

### REQ-014 Terminal repo-backed runs expose a completed-run export hub and sign-off summary through `RunReportView`
- Proposal Source: `7.3 Completed-run export hub`, `7.4 Sign-off summary surface`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift:64-74`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
- Gap / Note: The report view still owns both subordinate 008 surfaces.

### REQ-015 Evidence-pack status is first-class on completed benchmark runs
- Proposal Source: `7.5 Evidence-pack status is first-class`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:564-589`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:649-659`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The UI now derives `.exported` from persisted `evidencePackExportedAt` truth and stamps that same field during export, eliminating the old receipt-heuristic mismatch.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Partially Implemented
- Evidence Type: tests-found
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:968-1010`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1171-1321`
  - `docs/reference/agent-ui-test-execution.md`
- Gap / Note: Screenshot-bearing UI tests clearly exist for the proposal surfaces, but this audit did not freshly execute the current-head UI checkpoint.

### REQ-017 At least one happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
- Gap / Note: Same-`HEAD` accepted dogfood proof still shows a completed happy-path run with an exported evidence pack.

### REQ-018 At least one recovered non-happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
- Gap / Note: Same-`HEAD` accepted dogfood proof still shows a blocked non-happy-path run with an exported evidence pack.

### REQ-019 One benchmark repo is a messier real-world target, not only the sample repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/BenchmarkCohort.swift:62-70`
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:43`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:224-247`
- Gap / Note: The persisted cohort and exported sign-off packet now preserve repository profile type, so the `real_world` vs `controlled_sample` distinction is replayable from the stored/exported model.

### REQ-020 MVP sign-off cannot pass without complete exported review packets
- Proposal Source: `2. Product question`, `5.6 Sign-off gate`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:156-166`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:649-659`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The evaluator still blocks `GO` when `evidencePackExportedAt` is missing, and the export hub now stamps that persisted truth directly.

## Track 2: Expert Findings

### UI-001 Missing timeout/retry affordances in report surfaces
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-012`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Views/RunReportView.swift:127-164`
  - `Chainworks Forge/Views/RunReportView.swift:201-232`
- Why It Matters: Proposal 008 frames report/export retrieval as an operator-facing SLO surface, not just internal instrumentation. A generic load error without timeout and retry keeps the operator one failure away from raw-log archaeology again.
- Recommended Action: Add an explicit timeout state and a retry action on report/export retrieval surfaces, and exercise that state in proposal-scoped UI proof.

### READY-001 Current-head screenshot proof was not freshly replayed
- Severity: Major
- Confidence: Medium
- Related Proposal Items: `REQ-016`
- Evidence Type: tests-found, inference
- Evidence References:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:968-1010`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1171-1321`
  - `docs/reference/agent-ui-test-execution.md`
  - `./scripts/test-gate.sh build`
- Why It Matters: For Proposal 008, UI ownership and calm recovery/export UX are part of the contract. Without fresh screenshot-bearing proof on current `HEAD`, the audit still depends on code inspection for the last operator-facing acceptance slice.
- Recommended Action: Refresh the proposal-scoped UI checkpoint on the approved host and attach the resulting screenshots/xcresult to the next audit pass.

## Evidence Run Log

- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests' -resultBundlePath /tmp/p008-r6-delivery.xcresult ...`
  - Result: passed `14/14`
  - Bundle: [`/tmp/p008-r6-delivery.xcresult`](/tmp/p008-r6-delivery.xcresult)
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-r6-resume-dd -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -resultBundlePath /tmp/p008-r6-resume2.xcresult ...`
  - Result: passed `10/10`
  - Bundle: [`/tmp/p008-r6-resume2.xcresult`](/tmp/p008-r6-resume2.xcresult)
- `./scripts/test-gate.sh build`
  - Result: did not start
  - Note: environment guardrail refusal because unrelated active processes were already running (`debugserver`, external agent session)

## Roll-up

- Overall Conformance: `Partial`
- Overall Readiness: `At Risk`
- Audit Confidence: `High`

Proposal 008 has crossed the threshold from `Not Implemented` to `Partial`. The old launch-governance truth bugs are fixed on current `HEAD`, and the old approval-gate relaunch regression is now proven green by fresh runtime evidence. The remaining work is narrower and more clearly bounded than in `R5`: complete the explicit timeout/retry UX promised by `REQ-012`, then replay the proposal-scoped screenshot proof for `REQ-016`.
