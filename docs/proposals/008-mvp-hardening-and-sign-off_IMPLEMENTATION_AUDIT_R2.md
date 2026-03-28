# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-28T07:06:12+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 is materially closer than it was in `R1`, but it is still not implemented on the current `HEAD`. The real fixes are visible in code: app-driven runs are now linked to benchmark cohorts, the live completion path now invokes `BenchmarkRunRecorder`, the sign-off export route now uses `SignOffEvidencePackBuilder`, attachment truth is surfaced as `reference_only` / `rejected`, and report loading now runs through `OutputRetrievalSLOProbe`. The remaining blockers are proposal-critical rather than cosmetic: Proposal 007 is still only `Partial`, the canonical repo-backed checkpoint still fails from the real UI, no fresh happy-path or recovered non-happy-path evidence packs were found in default run storage, and the MVP gate still does not enforce “no complete exported review packet, no pass.”

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Proposal-level sign-off evidence gates are still open | High |
| Architecture | Acceptable | Core 008 services are now wired, but the final launch gate is still weaker than the proposal contract | High |
| Product | At Risk | The canonical repo-backed checkpoint still breaks before the operator reaches Start Run | High |
| UI | At Risk | Export/sign-off surfaces exist, but the product-critical checkpoint still fails in the Ideas owner path | High |
| UX | At Risk | Approval-gate relaunch and screenshot-backed recovery/export proof are still incomplete | Medium |
| Readiness | Not Ready | Proposal 007 prerequisite proof and Proposal 008 evidence-pack proof remain unresolved on current `HEAD` | High |

## Proposal Contract

### Scope

- Freeze the final MVP provider and sign-off boundary after Proposal 007.
- Add one fixed benchmark cohort with persisted benchmark/sign-off records outside the operational `Run` aggregate.
- Compute `GO/HOLD` only from persisted benchmark records.
- Harden blocked recovery, completed-run export, and sign-off summary UX inside the current shell.
- Require one happy-path and one recovered non-happy-path evidence pack before MVP sign-off.

### Locked Decisions

- Proposal 008 is blocked until Proposal 007 is implemented and review-proven on current `HEAD`.
- The canonical MVP provider set is `codex`, `claude_code`, and `gemini`.
- Benchmark/sign-off state lives outside `Run`.
- Attachments stay reference-only and are not agent-ingested.
- Recovery/export/sign-off stay subordinate to `RunsHomeView`, `RecoverySheet`, and `RunReportView`.
- MVP sign-off is an explicit `GO/HOLD` gate, not an inferred confidence signal.

### Primary User Flows

1. Define a fixed benchmark cohort spanning one controlled repo and one messier real-world repo.
2. Record repeatable manual-baseline and app-driven benchmark pairs for the same ideas.
3. Recover blocked repo-backed runs from the shell without reconstructing context from raw logs.
4. Export a trustworthy completed-run packet and a replayable sign-off packet from the app.
5. Decide `GO/HOLD` from persisted benchmark records plus complete exported evidence.

### UI Commitments

- Shell-owned blocked recovery path under `RunsHomeView` / `RecoverySheet`.
- Completed-run export hub under `RunReportView`.
- Embedded sign-off summary route under `RunReportView`, not a parallel top-level destination.
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
- `BenchmarkCohort`, `BenchmarkExecutionRecord`, `BenchmarkPair`, and `MVPSignOffDecisionSnapshot` are real persisted models in the app schema.
- `RunRepository.createRunFromPlan(...)` now assigns `run.experimentCohortID = idea.experimentCohortID`.
- `ExecutionService` now calls `recordBenchmarkExecutionIfNeeded(...)` from the live completion path.
- `MVPSignOffSummaryView` now exports through `SignOffEvidencePackBuilder`.
- `IdeaListView` now renders attachment validation state as `reference_only` / `rejected`.
- `RunReportView` and `CompletedRunExportHub` now integrate `OutputRetrievalSLOProbe` and benchmark-aware evidence-pack status.

### Divergences

- Proposal 007 is still only `Partial` on current `HEAD`, so Proposal 008’s hard prerequisite remains unmet.
- The canonical full-product checkpoint still fails from the real UI before the repo-backed sign-off flow can begin.
- No fresh happy-path or recovered non-happy-path evidence packs were found in default run storage during this audit.
- No dedicated current-head screenshot-backed proof was found for recovery/re-entry/export states as Proposal 008-specific acceptance evidence.
- The final launch gate still does not require proof that complete exported review packets exist before `GO`.

### Ambiguities / Evidence Gaps

- This audit found no dedicated 008-focused automated tests in `Chainworks ForgeTests` or `Chainworks ForgeUITests`; current proof still leans heavily on code inspection plus one failing canonical UI checkpoint.
- The fresh UI checkpoint wrote `/tmp/p008-r2-ui2.xcresult`, but `xcresulttool` could not read it because the bundle is missing `Info.plist`; failure details were therefore taken from `xcodebuild` stdout and the failing test source.

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
- Gap / Note: Current `HEAD` still does not satisfy Proposal 007’s own sign-off gate. The latest 007 audit remains `Partial`, and this audit found no fresh repo-backed evidence export proving the prerequisite is closed.

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
- Gap / Note: The proposal-faithful persistence split is present in the app schema.

### REQ-004 The benchmark cohort contract is fixed to two repositories and six ideas with one real-world repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
- Gap / Note: The cohort definition still encodes `2` repositories, `6` ideas, and a required `real_world` repository type.

### REQ-005 Manual baselines and app-driven benchmark records are written only as persisted benchmark records with immutable pairs
- Proposal Source: `3. Layer K`, `5.2 Persisted benchmark and sign-off model`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ManualBaselineImport.swift`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift`
  - `Chainworks Forge/Models/BenchmarkPair.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The services still operate on benchmark-side records rather than mutating launch-governance state onto `Run`.

### REQ-006 App-driven benchmark runs are actually linked to a cohort and recorded from the live runtime path
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.3 Required measurements`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/RunRepository.swift:133-136`
  - `Chainworks Forge/Engine/ExecutionService.swift:160-172`
  - `Chainworks Forge/Engine/ExecutionService.swift:542-556`
- Gap / Note: The shared run-creation path now assigns `experimentCohortID`, and the live completion path now invokes `BenchmarkRunRecorder` for cohort-linked runs.

### REQ-007 The evaluator computes `GO/HOLD` only from persisted benchmark records and persists a replayable snapshot checksum
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.6 Sign-off gate`, `5.7 Required sign-off summary payload`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:25-71`
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:76-157`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
- Gap / Note: The evaluator still reads benchmark-side records only and persists checksum-backed snapshots.

### REQ-008 The app can export a replayable sign-off packet from the shell-owned report/sign-off flow
- Proposal Source: `5.7 Required sign-off summary payload`, `7.4 Sign-off summary surface`, `9. Acceptance criteria / Benchmark and sign-off`
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
- Gap / Note: The dominant summary and subordinate receipt breakdown remain aligned with the proposal’s visual hierarchy.

### REQ-011 Relaunch at an approval gate restores visible `waiting_approval` context with no silent continuation
- Proposal Source: `6.3 Relaunch behavior at approval gate`, `7.1 Shell ownership is explicit`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Gap / Note: The shell clearly treats approval/recovery as first-class state, but this audit still did not find dedicated relaunch-specific proof closing the full contract.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`, `9. Acceptance criteria / PS closure`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift:19-23`
  - `Chainworks Forge/Views/RunReportView.swift:141-166`
  - `Chainworks Forge/Views/RunReportView.swift:215-245`
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift`
  - `rg -n "retry|timeout|ContentUnavailableView" 'Chainworks Forge/Views/RunReportView.swift' 'Chainworks Forge/Views/CompletedRunExportHub.swift'`
- Gap / Note: Live retrieval is now measured and loading/error/empty states exist, but this audit still did not find explicit retry UI or timeout-state rendering, nor any current-head p50/p95/p99 proof pack.

### REQ-013 Blocked implementation/release recovery is available from one shell-owned visible surface
- Proposal Source: `7.1 Shell ownership is explicit`, `7.2 Blocked review / release re-entry`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: The recovery route remains subordinate to the current shell, not a duplicate top-level destination.

### REQ-014 Terminal repo-backed runs expose a completed-run export hub and sign-off summary through `RunReportView`
- Proposal Source: `7.3 Completed-run export hub`, `7.4 Sign-off summary surface`, `8. File and component additions`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
- Gap / Note: The report view still owns both subordinate 008 surfaces.

### REQ-015 Evidence-pack status is first-class on completed benchmark runs
- Proposal Source: `7.5 Evidence-pack status is first-class`, `9. Acceptance criteria / Operator closure UX`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:23-47`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift:564-597`
- Gap / Note: Evidence-pack status is visible and now consults benchmark truth first, but it still falls back to heuristics and this audit did not find current-head exported-state proof from a real benchmark run.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Missing
- Evidence Type: tests-found, tests-run
- Evidence:
  - `rg -n "BenchmarkCohort|BenchmarkExecutionRecord|BenchmarkPair|MVPSignOffDecisionSnapshot|BenchmarkRunRecorder|ManualBaselineImport|MVPSignOffEvaluator|SignOffEvidencePackBuilder|OutputRetrievalSLOProbe|MVPBoundaryPolicy|evidence_pack|reference_only|MVPSignOff" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
  - `xcodebuild ... -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
- Gap / Note: This audit found no dedicated 008 UI coverage, and the fresh canonical checkpoint failed before the export/sign-off route could produce screenshot-backed proof.

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
- Evidence Type: runtime, inference
- Evidence:
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R5.md`
- Gap / Note: This audit found no recovered non-happy-path evidence pack in default run storage, and no fresh 008 proof closed that requirement.

### REQ-019 One benchmark repo is a messier real-world target, not only the sample repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
- Gap / Note: The contract for a `real_world` repository is encoded, but this audit did not find persisted current-head cohort/use evidence proving that the benchmark has actually been instantiated and run against such a target.

### REQ-020 MVP sign-off cannot pass without complete exported review packets
- Proposal Source: `2. Product question`, `5.6 Sign-off gate`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift:145-154`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift:22-116`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift:648-670`
- Gap / Note: The evaluator still gates only on linked-run/artifact-link presence, not on proof that complete exported review packets exist. The export route exists, but the “no complete packet, no pass” rule is not enforced.

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
- Why It Matters: Proposal 008 is not just a data-modeling slice. Its hardest promise is that MVP sign-off cannot pass without complete exported review packets. Current code still allows a `GO/HOLD` decision without that stronger export-completeness boundary.
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
- Why It Matters: Proposal 008 is supposed to prove MVP readiness through one real operator path. The latest canonical checkpoint no longer fails at idea creation, but it still fails at `ideas.setProjectDirectory(repoRootPath(), for: ideaTitle)`, so the repo-backed sign-off loop is still not product-real on current `HEAD`.
- Recommended Action: Fix the Ideas owner-path project-directory binding first, then rerun both canonical happy-path and non-happy-path checkpoints and preserve their exported packets as proposal evidence.

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
- Why It Matters: The proposal explicitly demands screenshot-tested recovery, re-entry, and export states. Right now the audit can point to real views and one failing end-to-end checkpoint, but not to dedicated current-head screenshot proof for the 008 surfaces themselves.
- Recommended Action: Add or rerun explicit UI proof for `BlockedRunRecoveryView`, `CompletedRunExportHub`, and `MVPSignOffSummaryView`, then keep the produced attachments with the proposal evidence.

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
- Why It Matters: The code shape suggests the app preserves visible approval/recovery context, but Proposal 008 asks for operator-trustworthy behavior after relaunch, not just plausible structure.
- Recommended Action: Add a relaunch-specific test or runtime proof showing a waiting-approval run returning to visible shell context without silent continuation.

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
- Why It Matters: Proposal 008 is intentionally blocked until 007 is fully proven. Current `HEAD` still lacks both the upstream prerequisite proof and the downstream sign-off evidence packs that 008 itself requires.
- Recommended Action: Do not treat 008 as sign-off-ready until 007 is `Implemented` and at least one happy-path plus one recovered non-happy-path packet exist on disk from current-head runs.

### READY-008-002 The audit proof is stronger than R1 but still thin in automated coverage
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `§9`, `REQ-016`
- Evidence Type: tests-found, tests-run, code
- Evidence:
  - `rg -n "BenchmarkCohort|BenchmarkExecutionRecord|BenchmarkPair|MVPSignOffDecisionSnapshot|BenchmarkRunRecorder|ManualBaselineImport|MVPSignOffEvaluator|SignOffEvidencePackBuilder|OutputRetrievalSLOProbe|MVPBoundaryPolicy|evidence_pack|reference_only|MVPSignOff" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
  - `/tmp/p008-r2-build.xcresult`
  - `xcodebuild ... testFullProductCheckpointCanonicalExecution`
- Why It Matters: The repo now has substantially better code fidelity than in `R1`, but there is still no deep 008-specific automated proof layer to catch sign-off regressions early.
- Recommended Action: Add focused tests for benchmark recording, evaluator gate completeness, sign-off packet export, and evidence-pack status transitions.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Proposal 007 prerequisite is green on current `HEAD` | Fail | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R5.md` is still `Partial` |
| Build succeeds on targeted platform(s) | Pass | `/tmp/p008-r2-build.xcresult`, `status = succeeded`, `warningCount = 57` |
| Core user flow runtime-validated | Partial | Fresh canonical checkpoint got farther than `R1` but still failed at `Chainworks_ForgeUITests.swift:1021` |
| Empty/loading/error states covered | Partial | `RunReportView` has loading/error/empty states, but explicit timeout/retry states are still not closed |
| Accessibility risk acceptable | Not Checked | Not a focus of this audit |
| Localization risk acceptable | Not Checked | Not a focus of this audit |
| Critical tests executed | Partial | One fresh canonical checkpoint executed and failed; no dedicated 008 test slice was found |
| Privacy/permissions/entitlements reviewed | Partial | Export/runtime artifact proof is still incomplete; Desktop export evidence was not revalidated here |

## Verification Log

- Resolved report path with:
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/008-mvp-hardening-and-sign-off.md`
- Verified proposal state with:
  - `rg -n "superseded|deprecated|replaced by|obsolete" 'docs/proposals/008-mvp-hardening-and-sign-off.md' 'docs/proposals' 'docs/reviews' 'docs/reference'`
- Captured repo metadata:
  - `git rev-parse --short HEAD` -> `fa31abc`
  - `git status --short` -> `dirty`
  - `date +%Y-%m-%dT%H:%M:%S%z` -> `2026-03-28T07:06:12+0200`
- Inspected proposal contract and prior audit baseline:
  - `nl -ba docs/proposals/008-mvp-hardening-and-sign-off.md | sed -n '1,680p'`
  - `sed -n '1,520p' docs/proposals/008-mvp-hardening-and-sign-off_IMPLEMENTATION_AUDIT_R1.md`
- Inspected current implementation wiring with focused file reads:
  - `nl -ba 'Chainworks Forge/Engine/ExecutionService.swift' | sed -n '156,180p'`
  - `nl -ba 'Chainworks Forge/Engine/ExecutionService.swift' | sed -n '532,566p'`
  - `nl -ba 'Chainworks Forge/Models/RunRepository.swift' | sed -n '120,140p'`
  - `nl -ba 'Chainworks Forge/Views/MVPSignOffSummaryView.swift' | sed -n '640,676p'`
  - `nl -ba 'Chainworks Forge/Views/IdeaListView.swift' | sed -n '344,372p'`
  - `nl -ba 'Chainworks Forge/Views/CompletedRunExportHub.swift' | sed -n '18,48p'`
  - `nl -ba 'Chainworks Forge/Views/CompletedRunExportHub.swift' | sed -n '560,608p'`
  - `nl -ba 'Chainworks Forge/Views/RunReportView.swift' | sed -n '1,246p'`
  - `nl -ba 'Chainworks Forge/Engine/MVPSignOffEvaluator.swift' | sed -n '1,260p'`
  - `nl -ba 'Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift' | sed -n '1,260p'`
- Checked test coverage presence:
  - `rg -n "BenchmarkCohort|BenchmarkExecutionRecord|BenchmarkPair|MVPSignOffDecisionSnapshot|BenchmarkRunRecorder|ManualBaselineImport|MVPSignOffEvaluator|SignOffEvidencePackBuilder|OutputRetrievalSLOProbe|MVPBoundaryPolicy|evidence_pack|reference_only|MVPSignOff" 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
  - Result: no dedicated 008-focused tests found
- Fresh build:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-r2-build-dd -resultBundlePath /tmp/p008-r2-build.xcresult build`
  - Summary via `xcrun xcresulttool get build-results summary --path '/tmp/p008-r2-build.xcresult'` -> `status = succeeded`, `warningCount = 57`, `errorCount = 0`
- Fresh canonical checkpoint:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-r2-ui2-dd -resultBundlePath /tmp/p008-r2-ui2.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution'`
  - Result from `xcodebuild` stdout: `TEST FAILED`
  - Failing assertion source: `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift:1021`
  - Note: `xcrun xcresulttool get test-results summary --path '/tmp/p008-r2-ui2.xcresult'` failed because the bundle is missing `Info.plist`
- Runtime artifact check:
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 \( -name 'delivery_receipt*' -o -name 'release_manifest*' -o -name 'git_push_receipt*' -o -name 'connect_upload_receipt*' -o -name 'release_bundle_manifest*' -o -name 'run_report*' -o -name 'signoff*' -o -name 'evidence_pack*' \) | sort | tail -80`
  - Result: no matching files found during this audit

## Recommended Next Actions

1. Close the upstream gate first: Proposal 007 must become `Implemented` with fresh current-head repo-backed evidence before Proposal 008 can honestly pass.
2. Fix the Ideas owner-path project-directory binding used by `testFullProductCheckpointCanonicalExecution()` so the canonical repo-backed sign-off flow can actually start.
3. Produce and preserve one fresh happy-path and one fresh recovered non-happy-path evidence pack in default run storage from current-head runs.
4. Strengthen the launch gate so `GO` cannot be computed without complete exported review packets, not merely linked run IDs and artifact links.
5. Add explicit 008-focused tests for benchmark recording, sign-off export, evidence-pack lifecycle, and approval-gate relaunch behavior.
