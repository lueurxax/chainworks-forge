# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R9

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `3e36dfb` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T22:00:51+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Ready with Risks` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 remains `Partial` on the current dirty tree, but `R9` materially improves the proof picture compared with `R8`. The implementation-side contract work still looks closed: local non-UI current-tree verification is green, with `xcodebuild build` passing and the focused `ResumeManager + DeliveryServices` slice passing `24/24` in [`/tmp/p008-r9-unit.xcresult`](/tmp/p008-r9-unit.xcresult). The prior fixes credited in `R8` remain intact on this `HEAD`: benchmark/export truth is persisted, approval-gate relaunch is restored without silent continuation, `CompletedRunExportHub` derives `Exported` from `evidencePackExportedAt`, and `RunReportView` still exposes explicit loading / empty / timeout / retry states.

The only proposal-level item still open is `REQ-016`, but the nature of that gap is now more precise and more serious than in `R8`. It is no longer accurate to say “approved-host replay is still pending.” Fresh approved-host UI evidence now exists in [`/tmp/p008-remote-evidence/p008-r9-ui2.xcresult`](/tmp/p008-remote-evidence/p008-r9-ui2.xcresult) and [`/tmp/p008-remote-evidence/p008-r9-ui3.xcresult`](/tmp/p008-remote-evidence/p008-r9-ui3.xcresult), and both bundles are red on current `HEAD`: one run failed `2/2`, and the later rerun improved only to `1/2`, still failing the canonical full product checkpoint from the real UI. That keeps the overall verdict at `Partial`: Proposal 008 is one UI-proof requirement away from `Implemented`, but that requirement is now blocked by a current-head approved-host failure, not by lack of reachability or stale readiness notes.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | `REQ-016` is still open, and the freshest approved-host UI checkpoint is red on current `HEAD` | High |
| Architecture | Good | Persisted benchmark/export truth and approval-gate recovery remain aligned with the proposal | High |
| Product | Good | The sign-off contract is materially implemented, but the canonical operator checkpoint still fails from the real UI | High |
| UI | Needs Fixes | Approved-host UI proof exists now, but it fails on canonical checkpoint execution and non-happy export flow | High |
| UX | Needs Fixes | Recovery/export/sign-off ownership is correct, but the end-to-end operator proof still breaks before clean closure | High |
| Readiness | Ready with Risks | The ship contract is down to one live UI-proof blocker, but that blocker is on the canonical checkpoint path | High |

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

- Proposal 007 remains implemented on the same base SHA in `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`.
- `MVPBoundaryPolicy` still freezes the canonical three-provider MVP boundary and attachment truth.
- Benchmark/sign-off state still lives outside `Run`.
- `RunRepository.createRunFromPlan(...)` still assigns `run.experimentCohortID`.
- `ExecutionService` still records benchmark executions from the live completion path.
- `MVPSignOffEvaluator` still blocks `GO` when an app-driven benchmark record lacks `evidencePackExportedAt`.
- `ResumeManager` still restores approval-gate runs without silently continuing execution.
- `CompletedRunExportHub` still derives `Exported` from persisted `evidencePackExportedAt` truth and exposes explicit export retry feedback.
- `RunReportView` still encodes explicit loading, empty, timeout, and retry states around report retrieval.
- Accepted app-launched happy-path and recovered non-happy-path evidence packs from Proposal 007 remain valid same-`HEAD` proof inputs for Proposal 008.
- Local current-tree non-UI verification is green in this audit round.

### Divergences

- The freshest approved-host UI checkpoint for Proposal 008 is red on current `HEAD`, so screenshot-bearing operator closure proof remains incomplete.

### Ambiguities / Evidence Gaps

- The approved-host UI xcresults used here were inspected from stored artifacts, not replayed directly from this environment in this audit turn.
- Local `./scripts/test-gate.sh build` and `./scripts/test-gate.sh fast` attempts were non-proving because the repository guardrail refused to start while unrelated app/test processes were already active.
- The remaining open requirement is no longer host reachability; it is a live current-head UI checkpoint failure on the approved-host evidence path.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 19 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence
- Proposal Source: `1.1 Hard prerequisite from Proposal 007`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: runtime
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md`
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/result.json`
- Gap / Note: The explicit upstream blocker remains closed for this audit.

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
  - `/tmp/p008-r9-unit.xcresult`
- Gap / Note: Fresh current-tree unit proof is green, including `ExecutionService resume waiting approval restores pending approval without re-executing stage`.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
- Gap / Note: The probe still measures `p50/p95/p99`, `RunReportView` exposes explicit loading/empty/timeout/retry states, and `CompletedRunExportHub` adds explicit retry on export failure.

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
- Status: Partially Implemented
- Evidence Type: tests-found, runtime
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `docs/reference/agent-ui-test-execution.md`
  - `/tmp/p008-remote-evidence/p008-r9-ui2.xcresult`
  - `/tmp/p008-remote-evidence/p008-r9-ui3.xcresult`
- Gap / Note: Fresh approved-host UI evidence now exists, but it is not green on current `HEAD`. `p008-r9-ui2` failed `2/2` with `Start Run` blocked by delivery preflight and the non-happy-path export proof failing; `p008-r9-ui3` improved to `1/2` but still failed `testFullProductCheckpointCanonicalExecution()` because the seeded repo-backed idea could not be opened from the real UI.

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

### READY-001 Fresh approved-host UI checkpoint is red on current `HEAD`
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-016`
- Evidence Type: runtime
- Evidence References:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `docs/reference/agent-ui-test-execution.md`
  - `/tmp/p008-remote-evidence/p008-r9-ui2.xcresult`
  - `/tmp/p008-remote-evidence/p008-r9-ui3.xcresult`
- Why It Matters: Proposal 008 treats recovery / re-entry / export UX as a ship contract, not optional polish. The current blocker is no longer “remote host unavailable” or “UI replay missing.” The approved-host replay exists and currently fails on the canonical checkpoint path, which means the remaining open item is a live product defect or unstable proof flow, not just an evidence bookkeeping gap.
- Recommended Action: Fix the real-UI canonical checkpoint failures first, then rerun the Proposal 008 checkpoint on `SMacBook.local` and attach the new screenshot-bearing xcresults to the next audit pass.

## Evidence Run Log

- `xcodebuild build -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath <temp>`
  - Result: passed
  - Note: fresh current-tree local non-UI build succeeded in this audit round
- `xcodebuild test -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath <temp> -resultBundlePath /tmp/p008-r9-unit.xcresult -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests'`
  - Result: passed `24/24`
  - Bundle: [`/tmp/p008-r9-unit.xcresult`](/tmp/p008-r9-unit.xcresult)
- `./scripts/test-gate.sh build`
  - Result: non-proving
  - Note: refused to start because unrelated app/test processes were already running
- `./scripts/test-gate.sh fast`
  - Result: non-proving
  - Note: refused to start because unrelated app/test processes were already running
- Approved-host UI checkpoint bundle inspection: `p008-r9-ui2`
  - Result: failed `0/2`
  - Bundle: [`/tmp/p008-remote-evidence/p008-r9-ui2.xcresult`](/tmp/p008-remote-evidence/p008-r9-ui2.xcresult)
  - Note: `testFullProductCheckpointCanonicalExecution()` failed because `Start Run` never became enabled after compile/preflight; `testFullProductCheckpointCanonicalNonHappyPathExportsEvidence()` also failed
- Approved-host UI checkpoint bundle inspection: `p008-r9-ui3`
  - Result: failed `1/2`
  - Bundle: [`/tmp/p008-remote-evidence/p008-r9-ui3.xcresult`](/tmp/p008-remote-evidence/p008-r9-ui3.xcresult)
  - Note: `testStartRunSheetUI()` passed, but `testFullProductCheckpointCanonicalExecution()` still failed because the seeded repo-backed idea could not be opened from the real UI

## Roll-up

- Overall Conformance: `Partial`
- Overall Readiness: `Ready with Risks`
- Audit Confidence: `High`

Proposal 008 remains one requirement away from a clean `Implemented` verdict, and that remaining requirement is now sharply defined. The implementation-side hardening work still looks complete. The only blocker left is `REQ-016`, and `R9` now records it as an actual approved-host current-head UI checkpoint failure rather than a stale “remote path unavailable” note.
