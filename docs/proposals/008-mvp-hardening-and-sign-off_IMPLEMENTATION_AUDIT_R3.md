# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T10:44:11+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

This is a no-delta repeat audit on the same proposal hash and same repository `HEAD` as `R2`, so the verdict does not move. Proposal 008 remains not implemented on current `HEAD`. The meaningful fixes already credited in `R2` are still present: cohort assignment is wired at run creation, the live completion path now invokes benchmark recording, the sign-off export route uses `SignOffEvidencePackBuilder`, attachment truth is surfaced as `reference_only` / `rejected`, and report loading goes through `OutputRetrievalSLOProbe`. The proposal still fails its own closure gate because Proposal 007 is not yet fully green, the canonical repo-backed checkpoint still fails from the real UI, no fresh happy-path or recovered non-happy-path evidence packs were found in default run storage, and the MVP launch gate still does not enforce “no complete exported review packet, no pass.”

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Proposal-level sign-off evidence gates are still open | High |
| Architecture | Acceptable | Core 008 services are wired, but exported-packet completeness is still not a hard launch-gate input | High |
| Product | At Risk | The canonical repo-backed checkpoint still breaks before the operator reaches Start Run | High |
| UI | At Risk | Export/sign-off surfaces exist, but the product-critical checkpoint still fails in the Ideas owner path | High |
| UX | At Risk | Approval-gate relaunch and screenshot-backed recovery/export proof are still incomplete | Medium |
| Readiness | Not Ready | Proposal 007 prerequisite proof and Proposal 008 evidence-pack proof remain unresolved on current `HEAD` | High |

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

- `MVPBoundaryPolicy` freezes the canonical MVP provider set and reference-only attachment types.
- `BenchmarkCohort`, `BenchmarkExecutionRecord`, `BenchmarkPair`, and `MVPSignOffDecisionSnapshot` are persisted outside `Run`.
- `RunRepository.createRunFromPlan(...)` now assigns `run.experimentCohortID = idea.experimentCohortID`.
- `ExecutionService` now invokes benchmark recording from the live completion path.
- `MVPSignOffSummaryView` now exports through `SignOffEvidencePackBuilder`.
- `IdeaListView` now renders attachment validation state as `reference_only` / `rejected`.
- `RunReportView` and `CompletedRunExportHub` now integrate `OutputRetrievalSLOProbe` and benchmark-aware evidence-pack status.

### Divergences

- Proposal 007 is still only `Partial` on current `HEAD`, so Proposal 008’s hard prerequisite remains unmet.
- The canonical full-product checkpoint still fails from the real UI before the repo-backed sign-off flow can begin.
- No fresh happy-path or recovered non-happy-path evidence packs were found in default run storage.
- No dedicated current-head screenshot-backed proof was found for recovery/re-entry/export states as Proposal 008 acceptance evidence.
- The final launch gate still does not require proof that complete exported review packets exist before `GO`.

### Ambiguities / Evidence Gaps

- This is a same-`HEAD`, same-proposal-hash repeat audit. Fresh `R2` evidence is intentionally reused after freshness check rather than rerun.
- The fresh UI checkpoint from `R2` wrote `/tmp/p008-r2-ui2.xcresult`, but `xcresulttool` could not read it because the bundle is missing `Info.plist`; failure details came from `xcodebuild` stdout and the test source.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 11 |
| Partially Implemented | 4 |
| Missing | 5 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence
- Proposal Source: `1.1 Hard prerequisite from Proposal 007`, `9. Acceptance criteria / Boundary freeze`
- Status: Missing
- Evidence Type: code, runtime
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R5.md`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1011-1028`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
- Gap / Note: Proposal 007 is still not `Implemented`, and this audit still has no fresh repo-backed export evidence proving the prerequisite is closed.

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
- Gap / Note: The persistence split promised by the proposal exists in the live schema.

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
  - `Chainworks Forge/Models/RunRepository.swift:133-136`
  - `Chainworks Forge/Engine/ExecutionService.swift:160-172`
  - `Chainworks Forge/Engine/ExecutionService.swift:542-556`
- Gap / Note: The shared run-creation path now assigns `experimentCohortID`, and the live completion path invokes `BenchmarkRunRecorder` for cohort-linked runs.

### REQ-007 The evaluator computes `GO/HOLD` only from persisted benchmark records and persists a replayable snapshot checksum
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.6 Sign-off gate`, `5.7 Required sign-off summary payload`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:25-71`
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:76-157`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
- Gap / Note: The evaluator still reads persisted benchmark records only and persists checksum-backed decision snapshots.

### REQ-008 The app can export a replayable sign-off packet from the shell-owned report/sign-off flow
- Proposal Source: `5.7 Required sign-off summary payload`, `7.4 Sign-off summary surface`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift:648-670`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:22-116`
  - `Chainworks Forge/Views/RunReportView.swift:25-29`
- Gap / Note: The sign-off route now exports through the dedicated builder from the shell-owned sign-off surface.

### REQ-009 Attachments are validated as reference-only/rejected and those states are visible before run start
- Proposal Source: `6.1 Attachment policy`, `9. Acceptance criteria / PS closure`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
  - `Chainworks Forge/Views/IdeaListView.swift:352-370`
  - `Chainworks Forge/Views/IdeaListView.swift:2178-2194`
- Gap / Note: The UI now surfaces deterministic attachment truth instead of only a raw path.

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
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Gap / Note: The shell shape suggests the right behavior, but this audit still does not have dedicated relaunch-specific proof closing the full contract.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift:19-23`
  - `Chainworks Forge/Views/RunReportView.swift:141-166`
  - `Chainworks Forge/Views/RunReportView.swift:215-245`
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift`
  - `rg -n "retry|timeout|ContentUnavailableView" 'Chainworks Forge/Views/RunReportView.swift' 'Chainworks Forge/Views/CompletedRunExportHub.swift'`
- Gap / Note: Live retrieval is now measured and loading/error/empty states exist, but explicit timeout/retry UI and current-head p50/p95/p99 proof remain open.

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
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:23-47`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:564-597`
- Gap / Note: Evidence-pack status is visible and benchmark-aware, but still falls back to heuristics and still lacks exported-state proof from a real benchmark run.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Missing
- Evidence Type: tests-found, tests-run
- Evidence:
  - `rg -n "BenchmarkCohort|BenchmarkExecutionRecord|BenchmarkPair|MVPSignOffDecisionSnapshot|BenchmarkRunRecorder|ManualBaselineImport|MVPSignOffEvaluator|SignOffEvidencePackBuilder|OutputRetrievalSLOProbe|MVPBoundaryPolicy|evidence_pack|reference_only|MVPSignOff" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
  - `xcodebuild ... -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- Gap / Note: No dedicated 008 UI coverage was found, and the fresh canonical checkpoint still failed before export/sign-off screenshot proof could be produced.

### REQ-017 At least one happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Missing
- Evidence Type: runtime, tests-run
- Evidence:
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
  - `xcodebuild ... -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- Gap / Note: No fresh happy-path evidence pack was found in default run storage, and the canonical checkpoint still fails before the repo-backed flow can finish.

### REQ-018 At least one recovered non-happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Missing
- Evidence Type: runtime
- Evidence:
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R5.md`
- Gap / Note: No recovered non-happy-path evidence pack was found in default run storage, and no fresher 008 runtime evidence closes this requirement.

### REQ-019 One benchmark repo is a messier real-world target, not only the sample repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
- Gap / Note: The `real_world` repo contract exists, but current-head persisted cohort/use evidence was not found.

### REQ-020 MVP sign-off cannot pass without complete exported review packets
- Proposal Source: `2. Product question`, `5.6 Sign-off gate`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:145-154`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:22-116`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift:648-670`
- Gap / Note: The evaluator still gates only on linked-run/artifact-link presence, not on proof that complete exported review packets exist.

## Architecture Review

**Summary:** Acceptable

### ARCH-008-001 The final launch gate is still weaker than the proposal contract
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `§5.6`, `§5.7`, `REQ-020`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:145-154`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:22-116`
- Why It Matters: Proposal 008 is a sign-off slice, not just a data-modeling slice. Without exported-packet completeness in the launch gate, the product can still produce a green decision on weaker proof than the proposal allows.
- Recommended Action: Promote exported-packet completeness into the evaluator’s persisted gate inputs or an equivalent hard precondition consumed by the evaluator.

## Product Review

**Summary:** At Risk

### PROD-008-001 The canonical repo-backed checkpoint still fails before the sign-off flow can start
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `§2 Product question`, `§5.5`, `§9 MVP sign-off evidence`, `REQ-017`, `REQ-018`
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1016-1028`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-r2-ui2-dd -resultBundlePath /tmp/p008-r2-ui2.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- Why It Matters: Proposal 008 must be earned by a real operator path. The latest canonical checkpoint still fails at `ideas.setProjectDirectory(repoRootPath(), for: ideaTitle)`, so the repo-backed sign-off loop is still not product-real on current `HEAD`.
- Recommended Action: Fix the Ideas owner-path project-directory binding first, then rerun both canonical happy-path and non-happy-path checkpoints and preserve their exported packets.

## UI Review

**Summary:** At Risk

### UI-008-001 008-specific screenshot proof is still weaker than the proposal requires
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§7`, `§9 Operator closure UX`, `REQ-016`
- Evidence Type: tests-found, tests-run
- Evidence:
  - `rg -n "BenchmarkCohort|BenchmarkExecutionRecord|BenchmarkPair|MVPSignOffDecisionSnapshot|BenchmarkRunRecorder|ManualBaselineImport|MVPSignOffEvaluator|SignOffEvidencePackBuilder|OutputRetrievalSLOProbe|MVPBoundaryPolicy|evidence_pack|reference_only|MVPSignOff" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
  - `xcodebuild ... testFullProductCheckpointCanonicalExecution`
- Why It Matters: The proposal explicitly asks for screenshot-tested recovery, re-entry, and export states. Current proof still points mostly to real views plus one failing end-to-end checkpoint.
- Recommended Action: Add or rerun explicit UI proof for `BlockedRunRecoveryView`, `CompletedRunExportHub`, and `MVPSignOffSummaryView`.

## UX Review

**Summary:** At Risk

### UX-008-001 Approval-gate relaunch proof remains incomplete
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `§6.3`, `REQ-011`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Why It Matters: Proposal 008 asks for operator-trustworthy relaunch behavior, not merely plausible state ownership.
- Recommended Action: Add relaunch-specific runtime proof showing a waiting-approval run returning to visible shell context without silent continuation.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-008-001 Proposal 007 prerequisite and Proposal 008 evidence-pack proof are still open together
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `§1.1`, `§5.5`, `§9`, `REQ-001`, `REQ-017`, `REQ-018`
- Evidence Type: code, runtime
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R5.md`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
- Why It Matters: Proposal 008 is intentionally blocked until 007 is fully proven. Current `HEAD` still lacks both the upstream prerequisite proof and the downstream evidence packs that 008 itself requires.
- Recommended Action: Do not treat 008 as sign-off-ready until 007 is `Implemented` and both happy-path and recovered non-happy-path packets exist on disk from current-head runs.

### READY-008-002 This repeat audit confirms no delta, not new evidence
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§9`, `REQ-016`
- Evidence Type: code, inference
- Evidence:
  - `git rev-parse --short HEAD` -> `fa31abc`
  - `md5 -q docs/proposals/008-mvp-hardening-and-sign-off.md` -> `8e64c6bbde7891dfc04916b01f84fca4`
  - `docs/proposals/008-mvp-hardening-and-sign-off_IMPLEMENTATION_AUDIT_R2.md`
- Why It Matters: This pass verifies that the verdict remains stable because nothing proposal-relevant changed, not because fresh sign-off proof landed.
- Recommended Action: Spend the next cycle on runtime proof and exported artifacts, not on repeating unchanged audits.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Proposal 007 prerequisite is green on current `HEAD` | Fail | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R5.md` is still `Partial` |
| Build succeeds on targeted platform(s) | Pass | Reused fresh `R2` proof: `/tmp/p008-r2-build.xcresult`, `status = succeeded`, `warningCount = 57` |
| Core user flow runtime-validated | Partial | Reused fresh `R2` proof: canonical checkpoint got farther than `R1` but still failed at `Chainworks_ForgeUITests.swift:1021` |
| Empty/loading/error states covered | Partial | `RunReportView` has loading/error/empty states, but explicit timeout/retry states are still open |
| Accessibility risk acceptable | Not Checked | Not a focus of this audit |
| Localization risk acceptable | Not Checked | Not a focus of this audit |
| Critical tests executed | Partial | Reused fresh `R2` checkpoint evidence; no deeper 008-specific automated slice was found |
| Privacy/permissions/entitlements reviewed | Partial | Export/runtime artifact proof is still incomplete |

## Verification Log

- Resolved new report path with:
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/008-mvp-hardening-and-sign-off.md`
- Confirmed same proposal hash and same repository `HEAD` as `R2`:
  - `git rev-parse --short HEAD` -> `fa31abc`
  - `date +%Y-%m-%dT%H:%M:%S%z` -> `2026-03-28T10:44:11+0200`
  - `md5 -q '/Users/user/Documents/Chainworks Forge/docs/proposals/008-mvp-hardening-and-sign-off.md'` -> `8e64c6bbde7891dfc04916b01f84fca4`
- Confirmed proposal state remains `Active` by searching local docs for supersession/deprecation markers.
- Reused fresh `R2` code/runtime evidence after freshness check:
  - build proof: `/tmp/p008-r2-build.xcresult`
  - canonical checkpoint command/output: `testFullProductCheckpointCanonicalExecution`
  - run-storage artifact search: empty
- Reused focused code inspections from `R2` for the relevant 008 slice:
  - `RunRepository.swift`
  - `ExecutionService.swift`
  - `MVPSignOffSummaryView.swift`
  - `IdeaListView.swift`
  - `CompletedRunExportHub.swift`
  - `RunReportView.swift`
  - `MVPSignOffEvaluator.swift`
  - `SignOffEvidencePackBuilder.swift`

## Recommended Next Actions

1. Close the upstream gate first: Proposal 007 must become `Implemented` with fresh current-head repo-backed evidence before Proposal 008 can honestly pass.
2. Fix the Ideas owner-path project-directory binding used by `testFullProductCheckpointCanonicalExecution()` so the canonical repo-backed sign-off flow can actually start.
3. Produce and preserve one fresh happy-path and one fresh recovered non-happy-path evidence pack in default run storage from current-head runs.
4. Strengthen the launch gate so `GO` cannot be computed without complete exported review packets.
5. Add explicit 008-focused tests for benchmark recording, sign-off export, evidence-pack lifecycle, and approval-gate relaunch behavior.
