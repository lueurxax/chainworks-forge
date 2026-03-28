# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R7

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T18:53:10+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Ready with Risks` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 is now `Partial` on the current dirty tree, but the remaining gap is much narrower than in `R6`. The last explicit implementation hole from `R6` is now closed: `REQ-012` moves to `Implemented` because `RunReportView` now has explicit loading, empty, timeout, and retry states, while `CompletedRunExportHub` adds explicit export-failure retry affordances. Fresh current-tree proof is also stronger than before: [`/tmp/p008-r7-build.xcresult`](/tmp/p008-r7-build.xcresult) is green, and [`/tmp/p008-r7-unit.xcresult`](/tmp/p008-r7-unit.xcresult) passed `24/24`, including the approval-gate relaunch path and delivery/sign-off service slice.

The only proposal-level item still left open is `REQ-016`: the contract requires screenshot-bearing recovery / re-entry / export proof on the current tree, and I could not freshly replay that UI checkpoint from this environment. The repository now correctly codifies remote-only UI execution in both the UI test target and the gate runner, local UI execution is disallowed by policy, and the approved remote host remained inaccessible from this agent session (`SMacBook.local` rejected SSH auth). So the report no longer points at a product bug or missing shell wiring; it points at one unresolved proving gap on the approved UI host path.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Current-tree screenshot proof for `REQ-016` was not replayed on an approved remote host | High |
| Architecture | Good | Persisted benchmark/export truth is aligned, and fresh unit proof stayed green | High |
| Product | Good | Sign-off loop is substantially complete, but final operator proof still depends on remote UI replay | Medium |
| UI | Ready with Risks | Report/export surfaces now encode the promised timeout/retry states, but no fresh screenshots were produced in this pass | Medium |
| UX | Ready with Risks | Recovery/export affordances are materially stronger in code; only screenshot-proof freshness remains open | Medium |
| Readiness | Ready with Risks | Canonical UI proof is remote-only and the approved host was not reachable from this environment | High |

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

- Proposal 007 remains implemented on the same base SHA in [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md](007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R10.md).
- `MVPBoundaryPolicy` still freezes the canonical three-provider MVP boundary and attachment truth.
- Benchmark/sign-off state still lives outside `Run`.
- `RunRepository.createRunFromPlan(...)` still assigns `run.experimentCohortID`.
- `ExecutionService` still records benchmark executions from the live completion path.
- `MVPSignOffEvaluator` still blocks `GO` when an app-driven benchmark record lacks `evidencePackExportedAt`.
- `ResumeManager` still restores approval-gate runs without silently continuing execution.
- `CompletedRunExportHub` still derives `Exported` from persisted `evidencePackExportedAt` truth and now exposes explicit export retry feedback.
- `RunReportView` now encodes explicit loading, empty, timeout, and retry states around report retrieval.
- The UI test target and `scripts/test-gate.sh` now explicitly encode the repository’s remote-only UI proving policy.

### Divergences

- The screenshot-bearing Proposal 008 UI checkpoint was not freshly replayed on the current tree, so `REQ-016` remains only partially proven.

### Ambiguities / Evidence Gaps

- The happy-path and non-happy-path app-launched evidence packs remain accepted inherited proof from Proposal 007 in `/tmp/p007-r6-sample-*`, not a newly refreshed Proposal 008 rerun.
- Local canonical gate attempts (`./scripts/test-gate.sh build` and `./scripts/test-gate.sh fast`) were blocked by the idle-environment guardrail because unrelated test/app processes were already active.
- Fresh remote UI replay was impossible from this environment because `SMacBook.local` rejected SSH authentication, and local UI execution is forbidden by repository policy.

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
  - `/tmp/p008-r7-unit.xcresult`
- Gap / Note: Fresh current-tree unit proof is green, including `ExecutionService resume waiting approval restores pending approval without re-executing stage`.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift:5-16`
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift:114-166`
  - `Chainworks Forge/Views/RunReportView.swift:19-24`
  - `Chainworks Forge/Views/RunReportView.swift:142-181`
  - `Chainworks Forge/Views/RunReportView.swift:217-269`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:77-99`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:468-491`
- Gap / Note: The probe still measures `p50/p95/p99`, `RunReportView` now exposes explicit loading/empty/timeout/retry states, and `CompletedRunExportHub` adds explicit retry on export failure.

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
  - `Chainworks Forge/Views/RunReportView.swift:69-82`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
- Gap / Note: The report view still owns both subordinate Proposal 008 surfaces.

### REQ-015 Evidence-pack status is first-class on completed benchmark runs
- Proposal Source: `7.5 Evidence-pack status is first-class`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:574-610`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:658-669`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The UI still derives `.exported` from persisted `evidencePackExportedAt` truth and stamps that same field during export.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Partially Implemented
- Evidence Type: tests-found, code
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:11-44`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1171-1321`
  - `docs/reference/agent-ui-test-execution.md:61-69`
  - `docs/reference/agent-ui-test-execution.md:114-147`
  - `scripts/test-gate.sh:24-39`
  - `scripts/test-gate.sh:78-96`
- Gap / Note: Screenshot-bearing proposal-scoped UI tests clearly exist, and the repository now encodes the remote-only proving path explicitly. This audit still could not freshly replay that checkpoint because approved-host access was unavailable from this environment and local UI execution is forbidden.

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
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:156-166`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:658-669`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The evaluator still blocks `GO` when `evidencePackExportedAt` is missing, and the export hub stamps that persisted truth directly.

## Track 2: Expert Findings

### READY-001 Fresh approved-host screenshot proof is still missing
- Severity: Major
- Confidence: High
- Related Proposal Items: `REQ-016`
- Evidence Type: tests-found, code
- Evidence References:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:11-44`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1171-1321`
  - `docs/reference/agent-ui-test-execution.md:61-69`
  - `docs/reference/agent-ui-test-execution.md:114-147`
  - `scripts/test-gate.sh:78-96`
- Why It Matters: Proposal 008 explicitly treats recovery/export/sign-off UX as part of the ship contract, not a best-effort polish pass. Until the screenshot-bearing UI checkpoint is replayed on an approved remote host, the audit still stops one step short of a fully closed proposal sign-off.
- Recommended Action: Re-run the proposal-scoped UI checkpoint on an approved remote host, preserve the screenshots / xcresult, and attach them to the next audit pass.

## Evidence Run Log

- `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-r7-build-dd -resultBundlePath /tmp/p008-r7-build.xcresult ...`
  - Result: passed
  - Bundle: [`/tmp/p008-r7-build.xcresult`](/tmp/p008-r7-build.xcresult)
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-r7-unit-dd -resultBundlePath /tmp/p008-r7-unit.xcresult -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/DeliveryServicesTests' ...`
  - Result: passed `24/24`
  - Bundle: [`/tmp/p008-r7-unit.xcresult`](/tmp/p008-r7-unit.xcresult)
- `./scripts/test-gate.sh build`
  - Result: did not start
  - Note: local guardrail refusal because unrelated test/app processes were already active
- `./scripts/test-gate.sh fast`
  - Result: did not start
  - Note: local guardrail refusal because unrelated test/app processes were already active
- `ssh -o BatchMode=yes -o ConnectTimeout=5 SMacBook.local 'hostname'`
  - Result: failed
  - Note: approved remote host rejected SSH authentication, so fresh remote UI replay was not possible from this environment

## Roll-up

- Overall Conformance: `Partial`
- Overall Readiness: `Ready with Risks`
- Audit Confidence: `High`

Proposal 008 is now one proving step away from a clean `Implemented` verdict. On the implementation side, the last concrete gap from `R6` is closed: the report/export surfaces now carry the explicit timeout and retry behavior the proposal promised, and fresh focused unit proof is green on the current tree. What remains is not a missing product slice; it is the required approved-host UI replay for `REQ-016`.
