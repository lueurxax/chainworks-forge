# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R10

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `c387e38` |
| Working Tree | `dirty` |
| Audited At | `2026-03-29T09:05:18+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Needs Verification` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 is `Not Implemented` on the current dirty tree `c387e38`, but the reason is now almost entirely proof-gated rather than code-gated. The implementation-side hardening work that was credited in `R9` still appears intact on this tree: fresh local non-UI verification is green, with `xcodebuild build` passing in [`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`](/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult) and the focused `ResumeManager + DeliveryServices` slice passing `24/24` in [`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult`](/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult). The credited fixes also still inspect correctly in code: persisted benchmark/export truth is still wired, approval-gate relaunch still restores pending approval, `CompletedRunExportHub` still derives `Exported` from `evidencePackExportedAt`, and `RunReportView` still owns explicit loading / empty / timeout / retry states.

What reopened on `c387e38` are two proposal-level evidence contracts. First, `REQ-001` is not closed on this tree because the last accepted Proposal 007 audit is on SHA `fa31abc`, not on `c387e38`, and the local attempt to use `FullMVPDeliveryTests` as a same-head upstream proof was non-proving: the targeted class-level run in [`/tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult`](/tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult) executed `0` tests. Second, `REQ-016` is not closed because there is still no finished current-head approved-host screenshot-bearing replay for recovery / re-entry / export states. I synced the current tree to `SMacBook.local` and initiated a same-head replay against `/tmp/chainworks-p008-r10-c387e38`, but the approved host was already occupied by another live `xcodebuild` session rooted in `/private/tmp/chainworks-ui-gate`, so the new replay never produced [`/tmp/p008-r10-ui-c387e38.xcresult`](/tmp/p008-r10-ui-c387e38.xcresult) during this audit window. That leaves the overall verdict at `Not Implemented`: code-level conformance remains strong, but the proposal explicitly requires current-head proof gates, and two of those gates are still open on this audit target.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | `REQ-001` and `REQ-016` are current-head proof requirements and remain open on `c387e38` | High |
| Architecture | Good | Persisted benchmark/export truth still matches the proposal’s boundary decisions | High |
| Product | Good | Sign-off flows remain present, but the proposal’s proof contract is not yet re-closed on this dirty tree | High |
| UI | Needs Verification | Approved-host screenshot-bearing replay for the current tree did not complete in this audit turn | High |
| UX | Needs Verification | Recovery/re-entry/export ownership still looks right, but current-head replay proof is still missing | High |
| Readiness | Needs Verification | The remaining blockers are explicit sign-off proof gates, not obvious new code-level regressions | High |

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

- `MVPBoundaryPolicy` still freezes the canonical three-provider MVP boundary and attachment truth.
- Benchmark/sign-off state still lives outside `Run`.
- `RunRepository.createRunFromPlan(...)` still assigns `run.experimentCohortID`.
- `ExecutionService` still records benchmark executions from the live completion path.
- `MVPSignOffEvaluator` still blocks `GO` when an app-driven benchmark record lacks `evidencePackExportedAt`.
- `ResumeManager` still restores approval-gate runs without silently continuing execution.
- `CompletedRunExportHub` still derives `Exported` from persisted `evidencePackExportedAt` truth and exposes explicit export retry feedback.
- `RunReportView` still encodes explicit loading, empty, timeout, and retry states around report retrieval.
- Local current-tree build and focused non-UI verification are green in this audit round.

### Divergences

- Proposal 007 is no longer directly review-proven on the current audit SHA `c387e38`; the last accepted upstream audit is on `fa31abc`.
- The current tree still lacks a finished approved-host screenshot-bearing replay for Proposal 008 operator closure states.

### Ambiguities / Evidence Gaps

- A same-head approved-host replay was started against `/tmp/chainworks-p008-r10-c387e38`, but it did not complete during this audit because another `xcodebuild` session was already active on `SMacBook.local`.
- The local attempt to use `FullMVPDeliveryTests` as a same-head upstream proof path was non-proving because the class-targeted run executed `0` tests.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 18 |
| Partially Implemented | 0 |
| Missing | 2 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence
- Proposal Source: `1.1 Hard prerequisite from Proposal 007`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Missing
- Evidence Type: runtime, tests-run
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
  - `/tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult`
- Gap / Note: The last accepted Proposal 007 audit is on SHA `fa31abc`, not the current audit SHA `c387e38`. The local attempt to re-establish same-head upstream proof via `FullMVPDeliveryTests` was non-proving because the targeted run executed `0` tests.

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
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks ForgeTests/ResumeManagerTests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult`
- Gap / Note: Fresh current-tree unit proof is green, including approval-gate relaunch restoration.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
- Gap / Note: The probe still measures `p50/p95/p99`, `RunReportView` exposes explicit loading/empty/timeout/retry states, and `CompletedRunExportHub` keeps explicit retry on export failure.

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
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
- Gap / Note: The report view still owns both subordinate Proposal 008 surfaces.

### REQ-015 Evidence-pack status is first-class on completed benchmark runs
- Proposal Source: `7.5 Evidence-pack status is first-class`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
- Gap / Note: The UI still derives `.exported` from persisted `evidencePackExportedAt` truth and stamps that same field during export.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Missing
- Evidence Type: tests-found, runtime
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `docs/reference/agent-ui-test-execution.md`
- Gap / Note: Current-head approved-host replay was initiated against `/tmp/chainworks-p008-r10-c387e38`, but it did not complete during this audit because `SMacBook.local` was already occupied by another live `xcodebuild` rooted in `/private/tmp/chainworks-ui-gate`. No finished current-head screenshot-bearing bundle exists yet for this requirement.

### REQ-017 At least one happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
- Gap / Note: Accepted app-launched dogfood proof still shows a completed happy-path run with an exported evidence pack.

### REQ-018 At least one recovered non-happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
- Gap / Note: Accepted app-launched dogfood proof still shows a blocked non-happy-path run with an exported evidence pack.

### REQ-019 One benchmark repo is a messier real-world target, not only the sample repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/BenchmarkCohort.swift`
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift`
- Gap / Note: The persisted cohort and exported sign-off packet still preserve repository profile type, so the `real_world` vs `controlled_sample` distinction remains replayable.

### REQ-020 MVP sign-off cannot pass without complete exported review packets
- Proposal Source: `2. Product question`, `5.6 Sign-off gate`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The evaluator still blocks `GO` when `evidencePackExportedAt` is missing, and the export hub stamps that persisted truth directly.

## Track 2: Expert Findings

### READY-001 Current-head proof gates reopened on `c387e38`
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-001`, `REQ-016`
- Evidence Type: runtime, tests-run
- Evidence References:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult`
  - `/tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult`
  - `docs/reference/agent-ui-test-execution.md`
- Why It Matters: Proposal 008 is unusually proof-driven. On the current dirty tree, the remaining blockers are not broad architecture doubts; they are explicit sign-off requirements that depend on current-head repo-backed and screenshot-bearing evidence. Because the latest accepted Proposal 007 audit is on an older SHA and the current-head approved-host replay did not finish, the audit target cannot honestly claim full sign-off even though the code-level hardening work still appears intact.
- Recommended Action: Re-close the proof gates on `c387e38` or the next commit that supersedes it: first, generate fresh current-head Proposal 007 repo-backed proof rather than relying on `fa31abc`; second, rerun the Proposal 008 approved-host UI checkpoint on `SMacBook.local` after the existing host workload clears and attach the resulting screenshot-bearing xcresult.

## Evidence Run Log

- `xcodebuild build -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath <temp> -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`
  - Result: passed
  - Bundle: [`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`](/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult)
- `xcodebuild test -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath <temp> -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests'`
  - Result: passed `24/24`
  - Bundle: [`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult`](/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult)
- `xcodebuild test -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath <temp> -resultBundlePath /tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult -only-testing:'Chainworks ForgeTests/FullMVPDeliveryTests'`
  - Result: non-proving
  - Bundle: [`/tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult`](/tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult)
  - Note: The class-targeted run executed `0` tests, so it cannot be used as current-head Proposal 007 proof.
- Approved-host same-head UI replay initiation against synced workspace `/tmp/chainworks-p008-r10-c387e38`
  - Result: not completed during audit window
  - Intended bundle: [`/tmp/p008-r10-ui-c387e38.xcresult`](/tmp/p008-r10-ui-c387e38.xcresult)
  - Note: `SMacBook.local` was reachable and accepted SSH, but another active `xcodebuild` session in `/private/tmp/chainworks-ui-gate` kept the approved host occupied throughout this audit window, so the new replay never produced a finished current-head bundle.

## Roll-up

- Overall Conformance: `Not Implemented`
- Overall Readiness: `Needs Verification`
- Audit Confidence: `High`

Proposal 008 remains very close to closure, but on `c387e38` it is not closed yet. The blocking delta is narrow and explicit: current-head proof gates, not broad code regressions. Until Proposal 007 is re-proven on the current tree and the approved-host screenshot-bearing Proposal 008 replay actually finishes, the proposal’s own sign-off contract is still open.
