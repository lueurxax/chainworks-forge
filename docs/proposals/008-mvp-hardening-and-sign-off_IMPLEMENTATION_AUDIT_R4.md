# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T15:48:16+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 is materially closer to closure than `R3`, but it is still `Not Implemented` on the current tree. The biggest real fix since `R3` is now present in code: `MVPSignOffEvaluator` explicitly refuses `GO` when an app-driven benchmark record lacks `evidencePackExportedAt`, so the old “no exported review packet, still pass” hole is closed. Same-`HEAD` app-launched proof from Proposal 007 also still demonstrates exported happy-path and non-happy-path evidence packs in `/tmp`. The proposal nevertheless remains open because its hard prerequisite is still unmet (`Proposal 007` is only `Partial`), the completed-run evidence-pack status UI still overclaims `Exported` without consulting the persisted export timestamp, and the fresh verification surface is weaker than required: `xcodebuild build` is green, but the focused test slice fails before executing any tests due a duplicate `setUpWithError()` override in `Chainworks_ForgeUITests.swift`, while remote-only UI reruns could not be refreshed because the approved host path was unreachable from this environment.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Proposal 007 prerequisite is still open on current `HEAD` | High |
| Architecture | Acceptable | Completed-run evidence-pack status still diverges from persisted export truth | High |
| Product | At Risk | MVP sign-off still depends on an upstream delivery slice that is not fully green | High |
| UI | Evidence Gap | Remote-only UI proof could not be refreshed and the current test slice fails at build time | High |
| UX | At Risk | Relaunch/recovery/export continuity is only partially re-proven on current `HEAD` | Medium |
| Readiness | Not Ready | Build is green, but the proposal-level verification surface is still red/non-refreshable | High |

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

- `MVPBoundaryPolicy` still freezes the canonical MVP provider set and reference-only attachment types.
- `BenchmarkCohort`, `BenchmarkExecutionRecord`, `BenchmarkPair`, and `MVPSignOffDecisionSnapshot` remain persisted outside `Run`.
- `RunRepository.createRunFromPlan(...)` still assigns `run.experimentCohortID = idea.experimentCohortID`.
- `ExecutionService` still invokes benchmark recording from the live completion path.
- `MVPSignOffSummaryView` still exports through `SignOffEvidencePackBuilder`.
- `RunReportView` still routes report opens through `OutputRetrievalSLOProbe`.
- `MVPSignOffEvaluator` now hard-fails `GO` when `evidencePackExportedAt` is absent for an app-driven benchmark record.
- Same-`HEAD` Proposal 007 dogfood artifacts still prove one exported happy-path pack and one exported non-happy-path pack from inside the app.

### Divergences

- Proposal 007 is still only `Partial` on current `HEAD`, so Proposal 008’s hard prerequisite remains unmet.
- `CompletedRunExportHub` still marks benchmark evidence as `Exported` when a linked app-driven record exists and receipts are present, even if `evidencePackExportedAt` was never persisted.
- Fresh 008-focused UI proof could not be rerun because UI execution is remote-only by policy and the approved remote host path was not reachable from this environment.
- The fresh focused test slice failed before executing any tests because `Chainworks_ForgeUITests.swift` now contains two `setUpWithError()` overrides.

### Ambiguities / Evidence Gaps

- The app-launched happy/non-happy evidence packs used in this audit come from same-`HEAD` Proposal 007 proof in `/tmp`, not from a newly refreshed Proposal 008 run.
- Desktop-based export destinations could not be inspected from this audit environment, so on-disk export verification relied on the preserved `/tmp/p007-r6-*` artifacts and same-`HEAD` audit evidence instead of default Desktop storage.
- Recovery/re-entry/export screenshot coverage now exists in `Chainworks_ForgeUITests.swift`, but current-head execution proof is still incomplete because the focused `xcodebuild test` path fails at build time and remote UI reruns were blocked.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 14 |
| Partially Implemented | 5 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence
- Proposal Source: `1.1 Hard prerequisite from Proposal 007`, `9. Acceptance criteria / Boundary freeze`
- Status: Missing
- Evidence Type: code, runtime
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R7.md`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R6.md`
- Gap / Note: Proposal 007 is still only `Partial` on the current tree, so Proposal 008’s explicit prerequisite remains open even though same-`HEAD` dogfood proof exists.

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
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
- Gap / Note: The persistence split promised by the proposal still exists in the live schema.

### REQ-004 The benchmark cohort contract is fixed to two repositories and six ideas with one real-world repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
- Gap / Note: The cohort definition still encodes `2` repositories, `6` ideas, and a required `real_world` repository type.

### REQ-005 Manual baselines and app-driven benchmark records are written only as persisted benchmark records with immutable pairs
- Proposal Source: `3. Layer K`, `5.2 Persisted benchmark and sign-off model`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ManualBaselineImport.swift`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift`
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
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift:16-58`
- Gap / Note: The shared run-creation path assigns `experimentCohortID`, and the live completion path still records cohort-linked benchmark executions.

### REQ-007 The evaluator computes `GO/HOLD` only from persisted benchmark records and persists a replayable snapshot checksum
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.6 Sign-off gate`, `5.7 Required sign-off summary payload`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:10-71`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
- Gap / Note: The evaluator still reads persisted benchmark records only and persists checksum-backed decision snapshots.

### REQ-008 The app can export a replayable sign-off packet from the shell-owned report/sign-off flow
- Proposal Source: `5.7 Required sign-off summary payload`, `7.4 Sign-off summary surface`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift:648-670`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The sign-off route exports through the dedicated builder from the shell-owned sign-off surface.

### REQ-009 Attachments are validated as reference-only/rejected and those states are visible before run start
- Proposal Source: `6.1 Attachment policy`, `9. Acceptance criteria / PS closure`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift:33-53`
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
- Status: Partially Implemented
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:973-1013`
- Gap / Note: The shell shape and screenshot-oriented tests point in the right direction, but this audit still does not have fresh relaunch-specific runtime proof on current `HEAD`.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift:19-23`
  - `Chainworks Forge/Views/RunReportView.swift:129-151`
  - `Chainworks Forge/Views/RunReportView.swift:181-220`
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift`
- Gap / Note: Live retrieval is measured and loading/error/empty states exist, but explicit timeout/retry UI and current-head p50/p95/p99 proof remain open.

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
- Gap / Note: The report view still owns both subordinate 008 surfaces.

### REQ-015 Evidence-pack status is first-class on completed benchmark runs
- Proposal Source: `7.5 Evidence-pack status is first-class`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:564-597`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:633-656`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift:4-16`
- Gap / Note: Evidence-pack status is visible and benchmark-aware, but the UI still marks `.exported` from `appDrivenRecord != nil && hasReceipts` rather than from the persisted `evidencePackExportedAt` truth it writes during export.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Partially Implemented
- Evidence Type: tests-found, tests-run
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:973-1013`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1200-1324`
  - `/tmp/p008-r4-unit.xcresult`
  - `docs/reference/agent-ui-test-execution.md`
- Gap / Note: Screenshot-oriented tests now clearly exist for approval/re-entry and export evidence flows, but this round could not execute them because the focused `xcodebuild test` path failed at build time and the approved remote UI host path was unreachable.

### REQ-017 At least one happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime, tests-run
- Evidence:
  - `/tmp/p007-r6-sample-happy/result.json`
  - `/tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R6.md`
- Gap / Note: Same-`HEAD` app-launched proof still shows a completed `happy_path` run with `approvalCount = 3`, `terminalStatus = completed`, and an exported evidence pack containing the expected delivery artifacts.

### REQ-018 At least one recovered non-happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: runtime, tests-run
- Evidence:
  - `/tmp/p007-r6-sample-nonhappy/result.json`
  - `/tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R6.md`
- Gap / Note: Same-`HEAD` app-launched proof still shows a blocked `non_happy_path` run with `approvalCount = 3`, `terminalStatus = blocked`, and an exported evidence pack preserving the expected partial delivery artifacts.

### REQ-019 One benchmark repo is a messier real-world target, not only the sample repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
- Gap / Note: The `real_world` repository contract exists in code, but this audit still did not inspect a persisted current-head benchmark cohort proving it is actively populated and used.

### REQ-020 MVP sign-off cannot pass without complete exported review packets
- Proposal Source: `2. Product question`, `5.6 Sign-off gate`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:79-86`
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:157-168`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:633-656`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift:4-16`
- Gap / Note: The evaluator now explicitly requires `evidencePackExportedAt` for every app-driven benchmark record before `GO` can pass, which closes the old launch-gate hole from `R3`.

## Architecture Review

**Summary:** Acceptable

### ARCH-008-001 Completed-run evidence-pack status still overstates exported truth
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§7.5`, `REQ-015`, `REQ-020`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:564-597`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:633-656`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift:15`
- Why It Matters: The sign-off gate now relies on persisted export truth, but the operator-facing status chip can still say `Exported` without reading that same persisted truth. That is a silent divergence between launch-governance logic and the UI contract that operators use to understand whether a run is sign-off-ready.
- Recommended Action: Make `CompletedRunExportHub` derive `.exported` only from `evidencePackExportedAt` when a benchmark-linked app-driven record exists, and reserve the receipt-only heuristic for non-benchmark runs.

## Product Review

**Summary:** At Risk

### PROD-008-001 Proposal 008 is still blocked by its own upstream prerequisite
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `§1.1`, `§5.5`, `§9`, `REQ-001`
- Evidence Type: code, runtime
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R7.md`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R6.md`
- Why It Matters: Proposal 008 is explicitly a hardening/sign-off layer on top of Proposal 007. Even with improved gate logic and preserved evidence packs, the product cannot honestly claim MVP sign-off readiness while the repo-backed delivery slice it depends on is still only `Partial`.
- Recommended Action: Close Proposal 007 first on the same tree, then rerun the Proposal 008 checkpoint as a downstream sign-off pass rather than treating it as an independent feature slice.

## UI Review

**Summary:** Evidence Gap

### UI-008-001 Fresh UI proof is blocked by host policy and by a current test-target compile error
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§7`, `§9 Operator closure UX`, `REQ-016`
- Evidence Type: tests-run, code, runtime
- Evidence:
  - `docs/reference/agent-ui-test-execution.md`
  - `scripts/test-gate.sh`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:13-16`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:387-389`
  - `/tmp/p008-r4-unit.xcresult`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && xcodebuild -version'`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook 'hostname && xcodebuild -version'`
- Why It Matters: Proposal 008 explicitly asks for screenshot-tested recovery/re-entry/export states. On this tree the repository policy forbids local UI execution, the approved remote host path was not reachable from this environment, and the focused `xcodebuild test` path fails during build because the UI test target now defines `setUpWithError()` twice.
- Recommended Action: Fix the duplicate override in `Chainworks_ForgeUITests.swift`, restore a working remote UI host path, and rerun the screenshot-bearing UI checkpoint on an approved host.

## UX Review

**Summary:** At Risk

### UX-008-001 Approval-gate relaunch and recovery continuity are still only partially re-proven
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `§6.3`, `§7.2`, `REQ-011`, `REQ-016`
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:973-1013`
- Why It Matters: The product now clearly has shell-owned recovery/re-entry surfaces, but the operator-trust promise in Proposal 008 is about continuity under relaunch and blocked recovery, not just about view existence. That continuity still is not freshly runtime-proven on the current tree.
- Recommended Action: Add or rerun a dedicated relaunch-at-approval-gate proof and one blocked recovery/export proof on an approved remote host, preserving screenshots and exported artifacts together.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-008-001 The fresh verification surface is red before any proposal-focused tests execute
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `§9`, `REQ-016`
- Evidence Type: tests-run, code
- Evidence:
  - `/tmp/p008-r4-build.xcresult`
  - `/tmp/p008-r4-unit.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:13-16`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:387-389`
- Why It Matters: The repo still builds on macOS, but the first focused verification command for the proposal does not reach test execution. That means the current tree is not in a state where Proposal 008 can be freshly signed off with its required UI/runtime proof.
- Recommended Action: Remove the duplicate `setUpWithError()` override and get the focused `xcodebuild test` slice back to an executing state before any further sign-off claims.

### READY-008-002 Remote-only UI proof remains operationally unavailable from this environment
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§9`, `REQ-016`, `REQ-017`, `REQ-018`
- Evidence Type: runtime
- Evidence:
  - `docs/reference/agent-ui-test-execution.md`
  - `scripts/test-gate.sh`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && xcodebuild -version'`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook 'hostname && xcodebuild -version'`
- Why It Matters: The repository’s host policy is explicit: UI proof should run only on approved remote hosts. Until that path is reachable from the audit environment, proposal-level UI reruns cannot be refreshed honestly.
- Recommended Action: Restore SSH access to an approved remote UI host or provide a currently reachable approved host alias before requesting another full sign-off audit.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Proposal 007 prerequisite is green on current `HEAD` | Fail | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R7.md` is still `Partial` |
| Build succeeds on targeted platform(s) | Pass | `/tmp/p008-r4-build.xcresult` -> `status = succeeded`, `warningCount = 57` |
| Core user flow runtime-validated | Partial | Same-`HEAD` Proposal 007 dogfood proof still shows exported happy/non-happy evidence packs in `/tmp/p007-r6-sample-*`, but no fresh Proposal 008 UI rerun was possible |
| Empty/loading/error states covered | Partial | `RunReportView` has loading/error/empty states, but explicit timeout/retry proof remains open |
| Accessibility risk acceptable | Not Checked | Not a focus of this audit |
| Localization risk acceptable | Not Checked | Not a focus of this audit |
| Critical tests executed | Partial | Focused `xcodebuild test` bundle failed before test execution due duplicate `setUpWithError()`; remote UI rerun unavailable |
| Privacy/permissions/entitlements reviewed | Partial | Export/runtime artifact proof exists in `/tmp`, but default Desktop export inspection was not available from this environment |

## Verification Log

- Resolved new report path with:
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/008-mvp-hardening-and-sign-off.md`
- Refreshed repo metadata:
  - `git rev-parse --short HEAD && git rev-parse HEAD`
  - `date +%Y-%m-%dT%H:%M:%S%z`
  - `md5 -q 'docs/proposals/008-mvp-hardening-and-sign-off.md'`
  - `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' 'docs/proposals/008-mvp-hardening-and-sign-off.md'`
  - `git status --short`
- Confirmed proposal state remains `Active`:
  - `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/008-mvp-hardening-and-sign-off.md docs/proposals docs/reference -g '*.md'`
- Inspected current 008 implementation surfaces:
  - `sed -n '1,240p' 'Chainworks Forge/Engine/MVPSignOffEvaluator.swift'`
  - `sed -n '1,240p' 'Chainworks Forge/Views/CompletedRunExportHub.swift'`
  - `sed -n '1,240p' 'Chainworks Forge/Views/MVPSignOffSummaryView.swift'`
  - `sed -n '1,260p' 'Chainworks Forge/Views/RunReportView.swift'`
  - `sed -n '1,260p' 'Chainworks Forge/Views/RecoverySheet.swift'`
  - `sed -n '120,180p' 'Chainworks Forge/Models/RunRepository.swift'`
  - `sed -n '150,220p' 'Chainworks Forge/Engine/ExecutionService.swift'`
- Captured exact line references with:
  - `nl -ba 'Chainworks Forge/Engine/MVPSignOffEvaluator.swift' | sed -n '70,170p'`
  - `nl -ba 'Chainworks Forge/Views/CompletedRunExportHub.swift' | sed -n '560,670p'`
  - `nl -ba 'Chainworks ForgeUITests/Chainworks_ForgeUITests.swift' | sed -n '1,30p;380,395p'`
  - `nl -ba 'Chainworks Forge/Views/MVPSignOffSummaryView.swift' | sed -n '640,690p'`
- Verified build and focused test status:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p008-r4-build-dd' -resultBundlePath '/tmp/p008-r4-build.xcresult' build`
  - `xcrun xcresulttool get build-results summary --path '/tmp/p008-r4-build.xcresult'`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath '/tmp/p008-r4-unit-dd' -resultBundlePath '/tmp/p008-r4-unit.xcresult' test -only-testing:'Chainworks ForgeTests/DeliveryServicesTests' -only-testing:'Chainworks ForgeTests/FullMVPWorkflowTests' -only-testing:'Chainworks ForgeTests/FullMVPReleaseOpsTests' -only-testing:'Chainworks ForgeTests/FullMVPIntegrationTests'`
  - `xcrun xcresulttool get build-results summary --path '/tmp/p008-r4-unit.xcresult'`
  - `xcrun xcresulttool get test-results summary --path '/tmp/p008-r4-unit.xcresult'`
- Verified same-`HEAD` happy/non-happy exported evidence packs remain present:
  - `ls -ld /tmp/p007-r6-sample-happy/result.json /tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974 /tmp/p007-r6-sample-nonhappy/result.json /tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9`
  - `cat /tmp/p007-r6-sample-happy/result.json`
  - `cat /tmp/p007-r6-sample-nonhappy/result.json`
  - `find /tmp/p007-r6-sample-happy/exports/evidence-pack-FFA5F974 -maxdepth 2 -type f | sort | sed -n '1,20p'`
  - `find /tmp/p007-r6-sample-nonhappy/exports/evidence-pack-5B2987E9 -maxdepth 2 -type f | sort | sed -n '1,20p'`
- Checked remote-only UI host availability:
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook.local 'hostname && xcodebuild -version'`
  - `ssh -o BatchMode=yes -o ConnectTimeout=8 SMacBook 'hostname && xcodebuild -version'`

## Recommended Next Actions

1. Close the upstream gate first: Proposal 007 must become `Implemented` on the same tree before Proposal 008 can honestly pass.
2. Fix the duplicate `setUpWithError()` override in `Chainworks_ForgeUITests.swift` so the focused verification slice executes again.
3. Make `CompletedRunExportHub` derive `.exported` from persisted `evidencePackExportedAt` for benchmark-linked runs.
4. Restore SSH access to an approved remote UI host and rerun the screenshot-bearing 008 checkpoint there.
5. After the remote rerun, preserve one fresh sign-off-ready evidence pack path and one refreshed screenshot bundle in the audit evidence chain rather than relying only on `/tmp/p007-r6-*`.
