# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T17:54:49+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 is still `Not Implemented` on the current `HEAD`, but for a different reason than `R4`. The old upstream blocker is now closed: Proposal 007 is `Implemented` on the same SHA in [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md](007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md), and the same-`HEAD` app-launched happy-path and non-happy-path dogfood exports in `/tmp/p007-r6-sample-*` remain valid sign-off evidence. The proposal nevertheless fails on its own current runtime contract. A fresh local unit slice proves that approval-gate relaunch continuity is regressed: `ResumeManagerTests/executionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage()` fails on current `HEAD`, leaving the run blocked instead of restoring `waiting_approval`. Two older fidelity gaps also remain open: `CompletedRunExportHub` still overclaims `Exported` from receipt presence instead of persisted `evidencePackExportedAt`, and the persisted/exported benchmark cohort still drops the `real_world` vs `controlled_sample` repository distinction. Build health is mixed rather than red now: a fresh `DeliveryServicesTests` slice passes cleanly, but the canonical `./scripts/test-gate.sh build` / `fast` path could not be refreshed in this audit because the approved remote host was already occupied by another test session.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Approval-gate relaunch contract is freshly failing on current `HEAD` | High |
| Architecture | At Risk | Benchmark/export truth still diverges between persisted state and operator-facing status | High |
| Product | At Risk | Sign-off-ready recovery UX is not trustworthy if relaunch at approval gate regresses to `blocked` | High |
| UI | At Risk | Report/export surfaces still lack explicit timeout/retry states promised by the proposal | High |
| UX | At Risk | Interrupted approval flows still are not restored into the intended visible approval context | High |
| Readiness | Not Ready | Canonical remote gate path could not be refreshed and one fresh focused unit slice is red | High |

## Proposal Contract

### Scope

- Freeze the final MVP provider and sign-off boundary after Proposal 007.
- Add a fixed benchmark cohort and persisted benchmark/sign-off records outside the operational `Run` aggregate.
- Compute `GO/HOLD` only from persisted benchmark records.
- Harden blocked recovery, completed-run export, and sign-off summary UX inside the current shell.
- Require one happy-path and one recovered non-happy-path evidence pack before MVP sign-off.

### Locked Decisions

- Proposal 008 is blocked until Proposal 007 is implemented and review-proven on current `HEAD`.
- The canonical MVP provider set is `codex`, `claude_code`, and `gemini`.
- Benchmark/sign-off state lives outside `Run`.
- Attachments remain reference-only and are not agent-ingested.
- Recovery/export/sign-off remain subordinate to `RunsHomeView`, `RecoverySheet`, and `RunReportView`.
- MVP sign-off is an explicit `GO/HOLD` gate.

### Primary User Flows

1. Define a fixed benchmark cohort spanning one controlled repo and one messier real-world repo.
2. Record repeatable manual-baseline and app-driven benchmark pairs for the same ideas.
3. Recover blocked repo-backed runs from the shell without raw-log archaeology.
4. Export a trustworthy completed-run packet and a replayable sign-off packet from the app.
5. Decide `GO/HOLD` from persisted benchmark records plus complete exported evidence.

### UI Commitments

- Shell-owned blocked recovery path under `RunsHomeView` / `RecoverySheet`.
- Completed-run export hub under `RunReportView`.
- Embedded sign-off summary route under `RunReportView`.
- Visible evidence-pack status on completed benchmark runs.
- Screenshot-tested recovery, re-entry, and export states.

### UX Commitments

- No silent resume after relaunch at approval gates.
- Completed-run overview stays calm while export hub carries deeper cost/receipt detail.
- Attachment language stays truthful: `reference_only` or `rejected`.
- Operators should not need raw-log archaeology for blocked benchmark recovery.

### Acceptance Criteria

- Proposal 007 prerequisite is green on current `HEAD`.
- Benchmark cohort and manual-vs-app protocol are fixed and repeatable.
- Every benchmark run captures proposal approval, implementation approval, release decision, and total elapsed time.
- Manual baselines and app-driven runs are persisted as immutable benchmark pairs.
- Final `GO/HOLD` evaluation uses only persisted benchmark records.
- Exported sign-off packet is replayable without external notes.
- Attachment policy, cost policy, approval-gate relaunch behavior, and output/report SLO are fixed.
- Blocked recovery/export/sign-off are shell-owned and screenshot-tested.
- At least one happy-path and one recovered non-happy-path evidence pack exist.
- MVP sign-off cannot pass without complete exported review packets.

### Test / Evidence Requirements

- Current-head repo-backed evidence proving Proposal 007 first.
- Proposal-level UI proof for canonical happy-path and non-happy-path checkpoints.
- Fresh happy-path and recovered non-happy-path evidence packs on disk.
- Screenshot-tested recovery, re-entry, and export states.

### Explicit Exclusions

- Forge Steward activation.
- Backend extraction / Temporal migration.
- Provider families beyond `codex`, `claude_code`, and `gemini`.
- Autonomous recovery.
- Automatic attachment ingestion into agent execution context.

## Proposal Fidelity / Divergence

### Matches

- Proposal 007 prerequisite is now closed on the same SHA via [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md](007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md).
- `MVPBoundaryPolicy` still freezes the canonical MVP provider set and reference-only attachment types.
- `BenchmarkCohort`, `BenchmarkExecutionRecord`, `BenchmarkPair`, and `MVPSignOffDecisionSnapshot` remain persisted outside `Run`.
- `RunRepository.createRunFromPlan(...)` still assigns `run.experimentCohortID = idea.experimentCohortID`.
- `ExecutionService` still invokes benchmark recording from the live completion path.
- `MVPSignOffSummaryView` exports via `SignOffEvidencePackBuilder`.
- `MVPSignOffEvaluator` still refuses `GO` when an app-driven benchmark record lacks `evidencePackExportedAt`.
- Same-`HEAD` Proposal 007 dogfood artifacts still prove one exported happy-path pack and one exported non-happy-path pack from inside the app.
- Fresh `DeliveryServicesTests` proof is green on current `HEAD`.

### Divergences

- Fresh runtime proof shows approval-gate relaunch does not restore `waiting_approval` on current `HEAD`; the run falls back to `blocked` instead.
- `CompletedRunExportHub` still marks benchmark evidence as `Exported` from `appDrivenRecord != nil && hasReceipts`, not from the persisted `evidencePackExportedAt` truth used by the evaluator.
- The persisted/exported benchmark cohort drops repository type information, so the `real_world` vs `controlled_sample` distinction is not replayable from the sign-off packet itself.
- Report/export surfaces still do not implement explicit timeout/retry states despite the proposal’s SLO surface contract.

### Ambiguities / Evidence Gaps

- The happy-path and non-happy-path evidence packs used here are same-`HEAD` accepted artifacts from Proposal 007 in `/tmp/p007-r6-sample-*`, not a newly refreshed Proposal 008 benchmark rerun.
- Local UI tests were intentionally not run in this audit. The canonical remote-only gate path could not be refreshed because the approved host was already busy running another session, so screenshot-bearing UI proof remains partially inherited rather than newly replayed.
- No dedicated current-head tests surfaced for `MVPSignOffEvaluator`, `SignOffEvidencePackBuilder`, or `OutputRetrievalSLOProbe`; those contracts remain mostly code-inspected.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 15 |
| Partially Implemented | 4 |
| Missing | 1 |
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
- Gap / Note: The explicit upstream blocker from `R4` is now closed on the same git SHA.

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
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift:7-74`
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
- Gap / Note: The cohort definition still encodes `2` repositories, `6` ideas, and a required `real_world` repository type.

### REQ-005 Manual baselines and app-driven benchmark records are written only as persisted benchmark records with immutable pairs
- Proposal Source: `3. Layer K`, `5.2 Persisted benchmark and sign-off model`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ManualBaselineImport.swift`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift:16-58`
  - `Chainworks Forge/Models/BenchmarkPair.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The benchmark services still operate on benchmark-side records, not launch-governance state on `Run`.

### REQ-006 App-driven benchmark runs are actually linked to a cohort and recorded from the live runtime path
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.3 Required measurements`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/RunRepository.swift:126-136`
  - `Chainworks Forge/Engine/ExecutionService.swift:160-172`
  - `Chainworks Forge/Engine/ExecutionService.swift:540-553`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift:16-58`
- Gap / Note: The shared run-creation path assigns `experimentCohortID`, and the live completion path still records cohort-linked benchmark executions.

### REQ-007 The evaluator computes `GO/HOLD` only from persisted benchmark records and persists a replayable snapshot checksum
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.6 Sign-off gate`, `5.7 Required sign-off summary payload`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:25-71`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
- Gap / Note: The evaluator still reads persisted benchmark records only and persists checksum-backed decision snapshots.

### REQ-008 The app can export a replayable sign-off packet from the shell-owned report/sign-off flow
- Proposal Source: `5.7 Required sign-off summary payload`, `7.4 Sign-off summary surface`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift:648-670`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:17-116`
  - `Chainworks Forge/Views/RunReportView.swift:76-80`
- Gap / Note: The sign-off route exports through the dedicated builder from the shell-owned sign-off surface.

### REQ-009 Attachments are validated as reference-only/rejected and those states are visible before run start
- Proposal Source: `6.1 Attachment policy`, `9. Acceptance criteria / PS closure`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift:33-56`
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
- Status: Missing
- Evidence Type: tests-run, code
- Evidence:
  - `Chainworks ForgeTests/ResumeManagerTests.swift:331-408`
  - `Chainworks Forge/Engine/ExecutionService.swift:194-243`
  - `/tmp/p008-r5-resume.xcresult`
- Gap / Note: This is no longer just an evidence gap. Fresh current-head unit proof fails because the interrupted run is not restored into `waiting_approval`; `pendingApprovalCount` stays `0`, `run.status` becomes `blocked`, and no resumed orchestrator is attached.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift:5-249`
  - `Chainworks Forge/Views/RunReportView.swift:19-23`
  - `Chainworks Forge/Views/RunReportView.swift:141-245`
  - `rg -n "timeout|retry|Retry" 'Chainworks Forge/Views/RunReportView.swift' 'Chainworks Forge/Views/CompletedRunExportHub.swift' 'Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift'`
- Gap / Note: The probe computes `p50/p95/p99`, and `RunReportView` has loading/error/empty states, but there is still no explicit timeout/retry implementation on the report/export surfaces.

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
  - `Chainworks Forge/Views/RunReportView.swift:68-81`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
- Gap / Note: The report view still owns both subordinate 008 surfaces.

### REQ-015 Evidence-pack status is first-class on completed benchmark runs
- Proposal Source: `7.5 Evidence-pack status is first-class`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:564-597`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:645-656`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift:4-16`
- Gap / Note: The UI still derives `.exported` from `pair.appDrivenRecord != nil && hasReceipts` instead of the persisted `evidencePackExportedAt` truth it writes during export.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Partially Implemented
- Evidence Type: tests-found, runtime
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:968-1010`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1171-1321`
  - `docs/reference/agent-ui-test-execution.md`
  - `./scripts/test-gate.sh build`
  - `./scripts/test-gate.sh fast`
- Gap / Note: Screenshot-bearing UI tests clearly exist, but this audit did not freshly execute them. Local UI execution remained intentionally out of scope, and the canonical approved-host gate path could not be refreshed because the remote host was already busy.

### REQ-017 At least one happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
- Gap / Note: Same-`HEAD` accepted dogfood proof still shows a completed `happy_path` run with an exported evidence pack.

### REQ-018 At least one recovered non-happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
- Gap / Note: Same-`HEAD` accepted dogfood proof still shows a blocked `non_happy_path` run with an exported evidence pack.

### REQ-019 One benchmark repo is a messier real-world target, not only the sample repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift:7-74`
  - `Chainworks Forge/Models/BenchmarkCohort.swift:10-33`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:38-44`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:219-223`
- Gap / Note: The fixed cohort definition still requires one `real_world` repository, but the persisted/exported cohort drops repository type down to plain IDs and labels. The sign-off packet therefore cannot itself replay or prove which cohort member was the messier real-world repo.

### REQ-020 MVP sign-off cannot pass without complete exported review packets
- Proposal Source: `2. Product question`, `5.6 Sign-off gate`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:157-168`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:645-656`
  - `Chainworks Forge/Views/RunsHomeView.swift:642-652`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift:15`
- Gap / Note: The evaluator still explicitly requires `evidencePackExportedAt` for every app-driven benchmark record before `GO` can pass.

## Architecture Review

**Summary:** At Risk

### ARCH-008-001 Completed-run evidence-pack status still overstates exported truth
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§7.5`, `REQ-015`, `REQ-020`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:572-583`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:645-656`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift:15`
- Why It Matters: The sign-off gate now relies on persisted export truth, but the operator-facing status chip still can say `Exported` without consulting that same persisted truth. That leaves launch-governance logic and operator-facing readiness status out of sync.
- Recommended Action: Derive benchmark-run `.exported` strictly from `evidencePackExportedAt`, and reserve the receipt-only heuristic for non-benchmark runs only.

### ARCH-008-002 Persisted sign-off packets still cannot prove which repo was the real-world cohort member
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§5.1`, `§5.7`, `REQ-019`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift:23-25`
  - `Chainworks Forge/Models/BenchmarkCohort.swift:16-33`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:38-44`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:219-223`
- Why It Matters: Proposal 008 freezes one controlled repo and one messier real-world repo as part of the benchmark contract. Once that type distinction is lost in persisted/exported cohort state, the sign-off packet is no longer fully replayable on that dimension.
- Recommended Action: Persist and export the repository profile type, not only repository IDs and labels.

## Product Review

**Summary:** At Risk

### PROD-008-001 Approval-gate relaunch continuity is currently regressed
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `§6.3`, `REQ-011`
- Evidence Type: tests-run, code
- Evidence:
  - `Chainworks ForgeTests/ResumeManagerTests.swift:331-408`
  - `/tmp/p008-r5-resume.xcresult`
  - `Chainworks Forge/Engine/ExecutionService.swift:194-243`
- Why It Matters: Proposal 008 is supposed to make blocked and approval-gated repo-backed runs sign-off-ready for one operator, not just structurally present in the shell. On current `HEAD`, relaunching an interrupted waiting-approval run does not restore the pending approval path at all.
- Recommended Action: Fix the resume path until the run reliably comes back as `waiting_approval` with a restored pending approval and attached orchestrator, then rerun the focused proof.

## UI Review

**Summary:** At Risk

### UI-008-001 Report/export surfaces still do not implement the promised timeout/retry states
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§6.4`, `REQ-012`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift:141-245`
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift:114-249`
  - `rg -n "timeout|retry|Retry" 'Chainworks Forge/Views/RunReportView.swift' 'Chainworks Forge/Views/CompletedRunExportHub.swift' 'Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift'`
- Why It Matters: The proposal explicitly promised no blank shells plus explicit loading, empty, timeout, and retry states. Today the implementation covers only part of that surface contract.
- Recommended Action: Add explicit timeout and retry UI flows to the report/export surfaces and back them with fresh runtime proof.

## UX Review

**Summary:** At Risk

### UX-008-001 Interrupted approval recovery still is not trustworthy enough for sign-off
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `§6.3`, `§7.2`, `REQ-011`, `REQ-016`
- Evidence Type: tests-run, tests-found
- Evidence:
  - `Chainworks ForgeTests/ResumeManagerTests.swift:331-408`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:968-1010`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1171-1321`
  - `/tmp/p008-r5-resume.xcresult`
- Why It Matters: The proposal’s operator promise is that recovery and re-entry do not require raw-log archaeology or guesswork. A shell with the right views is not sufficient if the interrupted approval path collapses into `blocked` after relaunch.
- Recommended Action: Restore the relaunch behavior first, then rerun the screenshot-bearing recovery/export proof on the approved remote host.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-008-001 The canonical remote gate path could not be refreshed in this audit
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§9`, `REQ-016`
- Evidence Type: runtime
- Evidence:
  - `./scripts/test-gate.sh build`
  - `./scripts/test-gate.sh fast`
- Why It Matters: The repository now documents the canonical proving path for agents as `./scripts/test-gate.sh ...`, with UI proof remote-only. In this audit both gate commands refused to start because the approved remote host was already occupied by another active test session, so the canonical readiness path could not be freshly replayed.
- Recommended Action: Rerun `build` and `fast` on the approved host once it is free, and preserve those fresh gate outputs alongside the next Proposal 008 audit pass.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Proposal 007 prerequisite is green on current `HEAD` | Pass | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md` is `Implemented` on the same SHA |
| Build succeeds on targeted platform(s) | Pass | `/tmp/p008-r5-resume.xcresult` build summary `status = succeeded`; `/tmp/p008-r5-delivery2.xcresult` build summary `status = succeeded` |
| Core benchmark / export flow runtime-validated | Partial | Same-`HEAD` Proposal 007 dogfood proof still shows exported happy/non-happy evidence packs in `/tmp/p007-r6-sample-*`, but no fresh P008-specific benchmark rerun was performed |
| Approval-gate relaunch continuity runtime-validated | Fail | `/tmp/p008-r5-resume.xcresult` fails `ResumeManagerTests/executionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage()` |
| Empty/loading/error states covered | Partial | `RunReportView` has loading/error/empty, but explicit timeout/retry states remain absent |
| Screenshot-bearing recovery/export proof refreshed | Partial | UI tests exist, but remote-only execution was not refreshed in this audit |
| Critical non-UI tests executed | Partial | `DeliveryServicesTests` passed `14/14`; `ResumeManagerTests` failed `1/10`; canonical `test-gate` path was unavailable |
| MVP sign-off export truth is trustworthy | Partial | Evaluator gate is correct, but `CompletedRunExportHub` still overclaims `.exported` |

## Verification Log

- Resolved new report path with:
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/008-mvp-hardening-and-sign-off.md`
- Refreshed repo metadata:
  - `git rev-parse --short HEAD && git rev-parse HEAD`
  - `git status --short`
  - `date +%Y-%m-%dT%H:%M:%S%z`
  - `md5 -q 'docs/proposals/008-mvp-hardening-and-sign-off.md'`
  - `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' 'docs/proposals/008-mvp-hardening-and-sign-off.md'`
- Confirmed proposal state remains `Active`:
  - `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/008-mvp-hardening-and-sign-off.md docs/proposals docs/reference -g '*.md'`
- Inspected current implementation surfaces:
  - `nl -ba 'Chainworks Forge/Engine/MVPSignOffEvaluator.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Engine/BenchmarkRunRecorder.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Engine/ExecutionService.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift' | sed -n '1,280p'`
  - `nl -ba 'Chainworks Forge/Models/BenchmarkCohort.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Models/BenchmarkExecutionRecord.swift' | sed -n '1,220p'`
  - `nl -ba 'Chainworks Forge/Support/BenchmarkCohortDefinition.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Views/CompletedRunExportHub.swift' | sed -n '560,690p'`
  - `nl -ba 'Chainworks Forge/Views/RunReportView.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Views/MVPSignOffSummaryView.swift' | sed -n '640,690p'`
  - `nl -ba 'Chainworks Forge/Views/RunsHomeView.swift' | sed -n '632,666p'`
- Checked targeted tests:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath '/tmp/p008-r5-resume.xcresult' test -only-testing:'Chainworks ForgeTests/ResumeManagerTests'`
  - `xcrun xcresulttool get test-results summary --path '/tmp/p008-r5-resume.xcresult'`
  - `xcrun xcresulttool get build-results summary --path '/tmp/p008-r5-resume.xcresult'`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p008-r5-delivery-dd' -resultBundlePath '/tmp/p008-r5-delivery2.xcresult' test -only-testing:'Chainworks ForgeTests/DeliveryServicesTests'`
  - `xcrun xcresulttool get test-results summary --path '/tmp/p008-r5-delivery2.xcresult'`
  - `xcrun xcresulttool get build-results summary --path '/tmp/p008-r5-delivery2.xcresult'`
- Verified same-`HEAD` upstream sign-off artifacts:
  - `sed -n '1,220p' '/tmp/p007-r6-sample-happy/result.json'`
  - `sed -n '1,220p' '/tmp/p007-r6-sample-nonhappy/result.json'`
  - `find /tmp/p007-r6-sample-happy -maxdepth 3 -type f | sort | sed -n '1,200p'`
  - `find /tmp/p007-r6-sample-nonhappy -maxdepth 3 -type f | sort | sed -n '1,200p'`
- Checked canonical gate-path availability:
  - `./scripts/test-gate.sh build`
  - `./scripts/test-gate.sh fast`

## Recommended Next Actions

1. Fix the approval-gate relaunch regression first. Proposal 008 cannot honestly pass while `ResumeManagerTests/executionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage()` is red.
2. Make `CompletedRunExportHub` derive `.exported` strictly from persisted `evidencePackExportedAt` for benchmark-linked runs.
3. Persist and export repository profile type so the benchmark packet can replay which repo was the required `real_world` member.
4. Add explicit timeout and retry states to report/export surfaces, then back them with runtime proof.
5. Refresh the canonical remote gate path (`./scripts/test-gate.sh build` and `./scripts/test-gate.sh fast`) once the approved host is free, and pair that rerun with a fresh remote-only screenshot-bearing 008 checkpoint.
