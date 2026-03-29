# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R11

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `c387e38` |
| Working Tree | `dirty` |
| Audited At | `2026-03-29T09:18:10+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 remains `Not Implemented` on the current dirty tree `c387e38`. This was a no-delta repeat pass relative to `R10`: the implementation-side hardening work still looks intact on this tree, and the fresh same-head local proof gathered in `R10` remains valid because neither `HEAD` nor the proposal changed. `xcodebuild build` is still green in [`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`](/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult), and the focused `ResumeManager + DeliveryServices` slice is still green `24/24` in [`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult`](/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult). Code inspection still supports the previously credited fixes: persisted benchmark/export truth remains wired, approval-gate relaunch remains truthful, `CompletedRunExportHub` still derives `Exported` from `evidencePackExportedAt`, and `RunReportView` still owns explicit loading / empty / timeout / retry states.

The point of this repeat pass was to resolve the lingering approved-host replay attempt from `R10`. That attempt had not finished. On `SMacBook.local`, the queued same-head replay shell rooted in `/tmp/chainworks-p008-r10-c387e38` was still waiting and had still not produced [`/tmp/p008-r10-ui-c387e38.xcresult`](/tmp/p008-r10-ui-c387e38.xcresult), so I explicitly interrupted that queued attempt. After interruption, the host was still occupied by a different live `xcodebuild` session rooted in `/private/tmp/chainworks-ui-gate`, which confirms the `R10` replay never reached execution on the synced current-head workspace. That means the audit outcome does not improve: `REQ-001` is still open because the last accepted Proposal 007 audit is on SHA `fa31abc`, not `c387e38`, and the local attempt to use `FullMVPDeliveryTests` as same-head upstream proof remains non-proving (`0` tests executed). `REQ-016` is still open because there is still no finished current-head approved-host screenshot-bearing replay for the recovery / re-entry / export states.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | `REQ-001` and `REQ-016` are still open current-head proof requirements | High |
| Architecture | Good | No new architecture regression surfaced in this repeat pass | High |
| Product | Good | Sign-off behavior is mostly present, but proof-gated acceptance remains open | High |
| UI | Not Ready | Current-head approved-host screenshot-bearing replay still does not exist as a finished bundle | High |
| UX | Not Ready | Recovery/re-entry/export UX remains unclosed on the proposal’s required proof path | High |
| Readiness | Not Ready | The remaining blockers are explicit sign-off gates, not speculative concerns | High |

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
- Same-head local build and focused non-UI verification remain green and unchanged from `R10`.

### Divergences

- Proposal 007 is still not directly review-proven on the current audit SHA `c387e38`; the last accepted upstream audit remains on `fa31abc`.
- Proposal 008 still lacks a finished current-head approved-host screenshot-bearing replay for operator closure UX.

### Ambiguities / Evidence Gaps

- The interrupted queued replay attempt never produced `/tmp/p008-r10-ui-c387e38.xcresult`, so there is still no current-head approved-host bundle to inspect.
- A separate live `xcodebuild` rooted in `/private/tmp/chainworks-ui-gate` was still occupying `SMacBook.local` after the queued current-head replay was cancelled.

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
- Gap / Note: The last accepted Proposal 007 audit is on SHA `fa31abc`, not `c387e38`. The class-targeted `FullMVPDeliveryTests` run remains non-proving because it executed `0` tests.

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
- Gap / Note: Fresh same-head unit proof remains green, including approval-gate relaunch restoration.

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
- Gap / Note: The queued same-head replay rooted in `/tmp/chainworks-p008-r10-c387e38` had still not started and still had not produced `/tmp/p008-r10-ui-c387e38.xcresult`, so I interrupted it. There is still no finished current-head approved-host screenshot-bearing bundle for this requirement.

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

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Current-head proof gates are still open, and the queued approved-host replay was interrupted without producing evidence
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-001`, `REQ-016`
- Evidence Type: runtime, tests-run
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult`
  - `/tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult`
  - `docs/reference/agent-ui-test-execution.md`
- Why It Matters: Proposal 008 is blocked by explicit proof contracts, not by generic polish concerns. On `c387e38`, Proposal 007 is not re-proven on the current tree, and Proposal 008 still has no finished approved-host screenshot-bearing replay. The cancelled queued replay resolves ambiguity about “maybe it is still running,” but it does not close the requirement.
- Recommended Action: Re-run the approved-host Proposal 008 UI checkpoint only after the competing `/private/tmp/chainworks-ui-gate` workload clears, and collect a finished current-head bundle. Separately, re-establish current-head Proposal 007 proof with a proving test path rather than the non-proving `0`-test class run.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | [`/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`](/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult) |
| Core user flow runtime-validated | Partial | Same-head non-UI recovery/export logic is validated, but current-head approved-host operator closure replay is still missing |
| Empty/loading/error states covered | Pass | `RunReportView` and `CompletedRunExportHub` still encode explicit states |
| Accessibility risk acceptable | Not Checked | No new runtime accessibility pass in this repeat round |
| Localization risk acceptable | Not Checked | Proposal does not make localization a primary contract and no new localization audit was run |
| Critical tests executed | Partial | `ResumeManager + DeliveryServices` green `24/24`; upstream `FullMVPDeliveryTests` run non-proving `0` tests |
| Privacy/permissions/entitlements reviewed | Not Checked | No new entitlements/privacy review in this repeat round |

## Verification Log

- Reused same-head local evidence from `R10` because `HEAD` remained `c387e38` and proposal MD5 remained `8e64c6bbde7891dfc04916b01f84fca4`
- `ssh test@SMacBook.local "pgrep -fal 'p008-r10-ui-c387e38|chainworks-p008-r10-c387e38|xcodebuild|xctest|Chainworks Forge.app/Contents/MacOS/Chainworks Forge'"`
- `ssh test@SMacBook.local "pkill -f 'p008-r10-ui-c387e38|chainworks-p008-r10-c387e38'"`
- `xcodebuild build ... -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-build.Y2tOft.xcresult`
- `xcodebuild test ... -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests' -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p008-r10-unit.Hsq7Ds.xcresult`
- `xcodebuild test ... -only-testing:'Chainworks ForgeTests/FullMVPDeliveryTests' -resultBundlePath /tmp/p008-r10-fullmvp.zcSK7J/fullmvp.xcresult`

## Recommended Next Actions

1. Wait for the competing `/private/tmp/chainworks-ui-gate` workload on `SMacBook.local` to clear, then rerun the approved-host Proposal 008 checkpoint and attach the finished screenshot-bearing current-head xcresult.
2. Re-prove Proposal 007 on `c387e38` or on the next commit that supersedes it using a test path that actually executes the repo-backed runtime cases.
3. Only after those two proof gates are green, rerun the Proposal 008 implementation audit for a realistic path to `Implemented`.
