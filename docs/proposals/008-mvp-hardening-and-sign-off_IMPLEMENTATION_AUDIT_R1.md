# Proposal 008: MVP Hardening and Sign-Off — Validation Loop, Boundary Freeze, Recovery UX, and Launch Gate Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/008-mvp-hardening-and-sign-off.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Working Tree | `dirty` |
| Audited At | `2026-03-27T23:48:45+0200` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 008 is not implemented on the current `HEAD`. The repo now contains substantial 008 scaffolding: the canonical MVP boundary policy exists, benchmark/sign-off models are in the app schema, and shell-owned recovery/export/sign-off surfaces are real SwiftUI views. But the contract that makes Proposal 008 an MVP sign-off slice is still open. The benchmark/sign-off services are not wired into the live run path, `Run.experimentCohortID` is not assigned anywhere, attachment-policy enforcement is not surfaced in the operator UI, and the proposal-level checkpoint UI proof failed `2/2` on current `HEAD`. The build is green, but the targeted unit bundle that was supposed to support this audit ran `0` tests, so it does not close the proposal’s proof gate.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | The sign-off loop is modeled but not wired into real run completion/export paths | High |
| Architecture | At Risk | Benchmark/sign-off services are leaf implementations with no runtime owner path | High |
| Product | Not Ready | The canonical happy-path and non-happy-path checkpoint flows are not achievable from the real UI on current `HEAD` | High |
| UI | At Risk | Export/sign-off surfaces exist, but the proposal-level checkpoint tests fail before the operator can create the benchmark idea | High |
| UX | At Risk | Attachment truth and evidence-pack truth are still heuristic instead of operator-trustworthy | High |
| Readiness | Not Ready | Proposal 007 prerequisite proof and Proposal 008 sign-off evidence are both still open | High |

## Proposal Contract

### Scope

- Freeze the final MVP provider and sign-off boundary after Proposal 007.
- Add a fixed benchmark cohort and persisted benchmark/sign-off records outside the operational `Run` aggregate.
- Compute an explicit `GO/HOLD` launch decision only from persisted benchmark data.
- Harden blocked-run recovery, completed-run export, and sign-off summary UX inside the existing shell.
- Require one happy-path and one recovered non-happy-path evidence pack before MVP sign-off.

### Locked Decisions

- Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence.
- The canonical MVP provider set is `codex`, `claude_code`, `gemini`.
- Benchmark/sign-off state lives outside `Run`.
- Attachments are reference-only local references in MVP.
- Recovery/export/sign-off remain subordinate to `RunsHomeView`, `RecoverySheet`, and `RunReportView`.
- MVP sign-off is an explicit `GO/HOLD` gate, not an inferred confidence decision.

### Primary User Flows

1. Freeze one benchmark cohort with one controlled repo and one messier real-world repo.
2. Record manual baseline and app-driven execution pairs for the same ideas.
3. Recover blocked repo-backed runs from the shell without raw-log archaeology.
4. Export a trustworthy completed-run packet and a replayable MVP sign-off packet.
5. Decide `GO/HOLD` from persisted benchmark records and visible operator evidence.

### UI Commitments

- Shell-owned blocked recovery path under `RunsHomeView` / `RecoverySheet`.
- Completed-run export hub under `RunReportView`.
- Embedded sign-off summary route under `RunReportView`, not a parallel destination.
- Visible evidence-pack status on completed benchmark runs.
- Screenshot-tested recovery, re-entry, and export states.

### UX Commitments

- No silent resume after relaunch at approval gates.
- Completed-run overview stays calm while export hub carries deeper cost/receipt detail.
- Attachment language must stay truthful: `reference_only` or `rejected`, never implied ingestion.
- Operators should not need raw-log archaeology for blocked benchmark recovery.

### Acceptance Criteria

- Proposal 007 prerequisite is green on current `HEAD`.
- Benchmark cohort is fixed and repeatable.
- Every benchmark run captures proposal approval, implementation approval, release decision, and total elapsed time.
- Manual baselines and app-driven runs are persisted as immutable benchmark pairs.
- Final `GO/HOLD` evaluation uses only persisted benchmark records.
- Exported sign-off packet is replayable without external notes.
- Attachment policy, cost policy, approval-gate relaunch behavior, and SLO are fixed.
- Blocked recovery/export/sign-off are shell-owned and screenshot-tested.
- At least one happy-path and one recovered non-happy-path evidence pack exist.

### Explicit Exclusions

- Forge Steward activation.
- Backend extraction / Temporal migration.
- Provider families beyond `codex`, `claude_code`, `gemini`.
- Autonomous recovery.
- Attachment ingestion into agent execution context.

## Proposal Fidelity / Divergence

### Matches

- `MVPBoundaryPolicy` freezes the canonical provider set and the supported reference-only attachment extensions.
- `BenchmarkCohort`, `BenchmarkExecutionRecord`, `BenchmarkPair`, and `MVPSignOffDecisionSnapshot` are real persisted models and are registered in `Chainworks_ForgeApp.swift`.
- `BenchmarkCohortDefinition`, `ManualBaselineImport`, `BenchmarkRunRecorder`, `MVPSignOffEvaluator`, `SignOffEvidencePackBuilder`, and `OutputRetrievalSLOProbe` exist as separate 008-specific types.
- `RunsHomeView`, `RecoverySheet`, `BlockedRunRecoveryView`, `RunReportView`, `CompletedRunExportHub`, and `MVPSignOffSummaryView` implement the promised shell-owned surfaces.
- `CompletedRunExportHub` exposes dominant summary, receipts, cost, and evidence-pack status.

### Divergences

- No runtime path currently invokes `ManualBaselineImport`, `BenchmarkRunRecorder`, `MVPSignOffEvaluator`, `SignOffEvidencePackBuilder`, or `OutputRetrievalSLOProbe`.
- No code writes `Run.experimentCohortID`, so completed runs are not actually linked into benchmark/sign-off state.
- Attachment validation exists only in `MVPBoundaryPolicy.validateAttachment(...)`; the operator UI still shows the raw attachment path and no `reference_only` / `rejected` state.
- `CompletedRunExportHub` and `MVPSignOffSummaryView` export ad hoc JSON payloads rather than calling `SignOffEvidencePackBuilder`.
- Evidence-pack status is heuristically inferred from `deliveryConfigurationJSON` plus receipt names, not from persisted benchmark/sign-off truth.
- The proposal-level full product checkpoint UI proof failed on both happy-path and non-happy-path scenarios.

### Ambiguities / Evidence Gaps

- The default Desktop export root could not be audited directly because the host denied Desktop enumeration in this environment.
- The targeted unit bundle was green but ran `0` tests, so it is repo-health evidence, not meaningful proposal-coverage evidence.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 5 |
| Missing | 7 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal 008 is blocked until Proposal 007 has current-head green repo-backed evidence
- Proposal Source: `1.1 Hard prerequisite from Proposal 007`, `9. Acceptance criteria / Boundary freeze`
- Status: Missing
- Evidence Type: tests-run, runtime
- Evidence:
  - `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding_IMPLEMENTATION_AUDIT_R4.md`
  - `/tmp/p008-audit-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
- Gap / Note: Current `HEAD` still does not have green proposal-level repo-backed checkpoint proof. The fresh canonical checkpoint UI run failed `2/2`, and this audit found no fresh repo-backed evidence-pack/delivery receipts in default run storage.

### REQ-002 The canonical MVP provider set is frozen to `codex`, `claude_code`, and `gemini` across repo policy/docs
- Proposal Source: `4. Frozen MVP boundary`, `9. Acceptance criteria / Boundary freeze`, `11. Locked decisions / ARCH-080`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/provider-platform.md`
  - `docs/ps/chainworks-forge-mvp.md`
  - `README.md`
- Gap / Note: The repo’s current contract text and the runtime boundary policy are aligned to the three-provider MVP set.

### REQ-003 Benchmark/sign-off state lives outside the operational `Run` aggregate and remains linked to runs by ID
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `11. Locked decisions / ARCH-084`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/BenchmarkCohort.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
  - `Chainworks Forge/Models/BenchmarkPair.swift`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
- Gap / Note: The separate persistence model exists and is in the app schema.

### REQ-004 The benchmark cohort contract is fixed to two repositories and six ideas with one real-world repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / Benchmark and sign-off`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
- Gap / Note: The cohort-definition type enforces `2` repositories, `6` ideas, `3` ideas per repository, and requires both `controlled_sample` and `real_world` repository types.

### REQ-005 Manual baselines and app-driven benchmark records are written only as persisted benchmark records with immutable pairs
- Proposal Source: `3. Layer K`, `5.2 Persisted benchmark and sign-off model`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ManualBaselineImport.swift`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift`
  - `Chainworks Forge/Models/BenchmarkPair.swift`
  - `Chainworks Forge/Models/BenchmarkExecutionRecord.swift`
- Gap / Note: The services themselves obey the proposal boundary and write benchmark-side records rather than mutating launch-governance state onto `Run`.

### REQ-006 App-driven benchmark runs are actually linked to a cohort and recorded from the live runtime path
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.3 Required measurements`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift`
  - `rg -n "experimentCohortID\\s*=|BenchmarkRunRecorder\\(|ManualBaselineImport\\(" "Chainworks Forge"`
- Gap / Note: `Run.experimentCohortID` exists, but this audit found no assignment site and no runtime usage of `BenchmarkRunRecorder` or `ManualBaselineImport`.

### REQ-007 The evaluator computes `GO/HOLD` only from persisted benchmark records and persists a replayable snapshot checksum
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.6 Sign-off gate`, `5.7 Required sign-off summary payload`, `11. Locked decisions / ARCH-082`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift`
  - `Chainworks Forge/Models/MVPSignOffDecisionSnapshot.swift`
- Gap / Note: The evaluator is proposal-faithful as an isolated service: it reads benchmark-side records only and persists checksum-backed decision snapshots.

### REQ-008 The app can export a replayable sign-off packet from the shell-owned report/sign-off flow
- Proposal Source: `5.2 Persisted benchmark and sign-off model`, `5.7 Required sign-off summary payload`, `7.4 Sign-off summary surface`, `9. Acceptance criteria / Benchmark and sign-off`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
- Gap / Note: The builder exists, but the shipping export surfaces do not call it. They export ad hoc decision JSON from an already-existing snapshot instead of using the dedicated replayable sign-off packet builder.

### REQ-009 Attachments are validated as reference-only/rejected and those states are visible before run start
- Proposal Source: `6.1 Attachment policy`, `9. Acceptance criteria / PS closure`, `11. Locked decisions / ARCH-086`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `rg -n "validateAttachment\\(|reference_only|rejected" "Chainworks Forge"`
- Gap / Note: Validation logic exists only as a utility. The current operator UI still renders a plain paperclip/path and does not surface `reference_only` or `rejected`, nor does it record deterministic rejection before run start.

### REQ-010 Completed-run overview shows total cost while the export hub exposes deeper receipt breakdown
- Proposal Source: `6.2 Cost granularity`, `7.3 Completed-run export hub`, `11. Locked decisions / UX-081`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The dominant summary block shows total cost, and deeper sections expose per-stage/per-agent receipt detail.

### REQ-011 Relaunch at an approval gate restores visible `waiting_approval` context with no silent continuation
- Proposal Source: `6.3 Relaunch behavior at approval gate`, `7.1 Shell ownership is explicit`, `11. Locked decisions / UX-080`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Gap / Note: The current shell clearly treats `waitingApproval` as a first-class state and surfaces approval/recovery routes, but this audit found no dedicated `ApprovalResumeRouter` type and no fresh relaunch-specific runtime proof.

### REQ-012 Active output/report retrieval has a measured SLO with p50/p95/p99 and report/export surfaces define loading/empty/timeout/retry states
- Proposal Source: `6.4 Active output/report SLO`, `3. Layer L / OutputRetrievalSLOProbe`, `9. Acceptance criteria / PS closure`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/OutputRetrievalSLOProbe.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `rg -n "OutputRetrievalSLOProbe|measureAsync\\(|measure\\(" "Chainworks Forge"`
- Gap / Note: The probe exists and computes `p50/p95/p99`, but this audit found no integration into live report/export opens and no full loading/timeout/retry contract in the report/export surfaces.

### REQ-013 Blocked implementation/review/release recovery is available from one shell-owned visible surface
- Proposal Source: `7.1 Shell ownership is explicit`, `7.2 Blocked review / release re-entry`, `9. Acceptance criteria / Operator closure UX`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: The shell route is explicit and subordinate to the existing shell rather than a parallel top-level destination.

### REQ-014 Terminal repo-backed runs expose a completed-run export hub and sign-off summary through `RunReportView`
- Proposal Source: `7.3 Completed-run export hub`, `7.4 Sign-off summary surface`, `8. File and component additions`, `9. Acceptance criteria / Operator closure UX`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
- Gap / Note: The report view owns both subordinate surfaces exactly as the proposal requires.

### REQ-015 Evidence-pack status is first-class on completed benchmark runs
- Proposal Source: `7.5 Evidence-pack status is first-class`, `9. Acceptance criteria / Operator closure UX`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
- Gap / Note: A visible evidence-pack status exists, but it is computed heuristically (`missing` / `inProgress` / `ready` / `exported`) from run status and receipt names rather than persisted benchmark/sign-off truth, and it does not use the proposal’s literal status vocabulary.

### REQ-016 Recovery, re-entry, and export states are screenshot-tested on current `HEAD`
- Proposal Source: `9. Acceptance criteria / Operator closure UX`
- Status: Missing
- Evidence Type: tests-run, screenshot
- Evidence:
  - `/tmp/p008-audit-ui.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Gap / Note: The only fresh proposal-level UI proof in this audit failed before the flow could create the idea. No current-head screenshot-backed recovery/export sign-off proof was produced here.

### REQ-017 At least one happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Missing
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p008-audit-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
- Gap / Note: The happy-path checkpoint test failed, and this audit found no fresh exported happy-path evidence pack in default run storage.

### REQ-018 At least one recovered non-happy-path evidence pack exists and is exportable from the app
- Proposal Source: `5.5 Required evidence`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Missing
- Evidence Type: tests-run, runtime
- Evidence:
  - `/tmp/p008-audit-ui.xcresult`
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
- Gap / Note: The non-happy-path checkpoint test also failed before run creation, and no recovered benchmark evidence pack was found in default run storage.

### REQ-019 One benchmark repo is a messier real-world target, not only the sample repo
- Proposal Source: `5.1 Benchmark cohort`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Support/BenchmarkCohortDefinition.swift`
- Gap / Note: The contract for a `real_world` repository exists in the cohort definition, but this audit found no actual persisted cohort/run evidence proving that the real-world target has been instantiated and used.

### REQ-020 MVP sign-off cannot pass without complete exported review packets
- Proposal Source: `2. Product question`, `5.6 Sign-off gate`, `9. Acceptance criteria / MVP sign-off evidence`
- Status: Missing
- Evidence Type: code, runtime
- Evidence:
  - `Chainworks Forge/Views/MVPSignOffSummaryView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `rg -n "MVPSignOffEvaluator\\(|SignOffEvidencePackBuilder\\(" "Chainworks Forge"`
- Gap / Note: The app does not currently own a complete runtime sign-off gate. The evaluator and packet builder are not wired into the live export/decision path, so the product cannot yet enforce a true “no packet, no pass” rule.

## Expert Findings

### ARCH-008-001 Runtime ownership for benchmark/sign-off services is still missing
- Severity: Critical
- Confidence: High
- Related Proposal Items: Layer K, §5.2, §5.6, §5.7
- Related REQ IDs: `REQ-006`, `REQ-008`, `REQ-020`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Engine/ManualBaselineImport.swift`
  - `Chainworks Forge/Engine/BenchmarkRunRecorder.swift`
  - `Chainworks Forge/Engine/MVPSignOffEvaluator.swift`
  - `Chainworks Forge/Engine/SignOffEvidencePackBuilder.swift`
  - `rg -n "BenchmarkRunRecorder\\(|MVPSignOffEvaluator\\(|SignOffEvidencePackBuilder\\(" "Chainworks Forge"`
- Why It Matters: Proposal 008 is not a “types exist” proposal; it is a sign-off loop proposal. Without a runtime owner path, the benchmark/sign-off layer remains a disconnected subsystem.
- Recommended Action: Wire cohort assignment, recorder invocation, evaluator invocation, and packet export into the shared run-completion / export boundary, not just leaf views.

### PROD-008-001 The canonical sign-off checkpoint is not currently achievable from the real UI
- Severity: Critical
- Confidence: High
- Related Proposal Items: §2 Product question, §5.5 Required evidence, §9 MVP sign-off evidence
- Related REQ IDs: `REQ-001`, `REQ-016`, `REQ-017`, `REQ-018`
- Evidence Type: tests-run
- Evidence References:
  - `/tmp/p008-audit-ui.xcresult`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Why It Matters: MVP sign-off must be earned by a real operator path. Current `HEAD` still fails both proposal-level checkpoint tests at idea creation, so the promised evidence loop is not yet product-real.
- Recommended Action: Fix the `Ideas` owner path used by the full checkpoint tests first, then rerun happy-path and non-happy-path exports and keep the resulting artifacts as proposal evidence.

### UX-008-001 Attachment truth is still weaker than the proposal contract
- Severity: Major
- Confidence: High
- Related Proposal Items: §6.1 Attachment policy
- Related REQ IDs: `REQ-009`
- Evidence Type: code
- Evidence References:
  - `Chainworks Forge/Support/MVPBoundaryPolicy.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
- Why It Matters: Proposal 008 explicitly tries to stop operator misunderstanding about what attachments do. The current UI still presents only a raw path and never surfaces the promised validation state.
- Recommended Action: Validate attachments before run start and render an explicit, operator-visible `reference_only` / `rejected` state where the user selects or reviews the attachment.

### READY-008-001 Current proof quality is weaker than it first appears
- Severity: Major
- Confidence: High
- Related Proposal Items: §9 Acceptance criteria / Operator closure UX and MVP sign-off evidence
- Related REQ IDs: `REQ-016`, `REQ-017`, `REQ-018`
- Evidence Type: tests-run
- Evidence References:
  - `/tmp/p008-audit-unit.xcresult`
  - `/tmp/p008-audit-ui.xcresult`
- Why It Matters: The targeted unit bundle is green but ran `0` tests, while the proposal-level UI proof is red. That combination can create a false sense of sign-off readiness.
- Recommended Action: Treat the current unit run only as harness/build health, not as proposal coverage. Add or rerun meaningful 008-focused unit/integration coverage together with the UI checkpoint flows.

## Readiness Checklist

| Check | Status | Notes |
|---|---|---|
| Proposal 007 prerequisite is green on current `HEAD` | No | Current proposal-level checkpoint proof still fails |
| Build is green | Yes | `/tmp/p008-audit-build.xcresult`, `status = succeeded`, `56` warnings |
| Proposal-level unit/integration proof is meaningful | No | `/tmp/p008-audit-unit.xcresult` reports `totalTestCount = 0` |
| Proposal-level UI checkpoint proof is green | No | `/tmp/p008-audit-ui.xcresult` reports `2` failures, `0` passes |
| Shell-owned recovery/export/sign-off surfaces exist | Yes | Implemented in SwiftUI code |
| Benchmark/sign-off runtime wiring is complete | No | No invocation sites for recorder/evaluator/builder/probe |
| Happy-path evidence pack exists | No | Not found in default run storage during this audit |
| Recovered non-happy-path evidence pack exists | No | Not found in default run storage during this audit |

## Verification Log

### Metadata and scope

- Resolved report path with:
  - `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/008-mvp-hardening-and-sign-off.md`
- Verified proposal state by searching local docs for supersession/deprecation markers.
- Determined platform scope as `macOS` from proposal text, `xcodebuild` destination, and current app/test surfaces.

### Repository inspection

- Captured current SHA: `git rev-parse --short HEAD` -> `fa31abc`
- Captured working tree status: `git status --short`
- Searched implementation coverage with targeted `rg` across:
  - benchmark/sign-off types and invocations
  - attachment validation states
  - evidence-pack status routing
  - approval-gate / shell-owned recovery routes

### Build / test evidence

- Build:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-audit-build-dd -resultBundlePath /tmp/p008-audit-build.xcresult build`
  - Result: `BUILD SUCCEEDED`
  - Summary: `status = succeeded`, `errorCount = 0`, `warningCount = 56`
- Focused unit slice:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-audit-unit-dd -resultBundlePath /tmp/p008-audit-unit.xcresult test -only-testing:'Chainworks ForgeTests/FullMVPDeliveryTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests/testExecutionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage'`
  - Result: `TEST SUCCEEDED`
  - Summary: `/tmp/p008-audit-unit.xcresult` reports `totalTestCount = 0`, `passedTests = 0`, `failedTests = 0`
  - Interpretation: useful as harness/build-health evidence only; not meaningful proposal coverage
- Proposal-level UI checkpoint slice:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p008-audit-ui-dd -resultBundlePath /tmp/p008-audit-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalExecution' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testFullProductCheckpointCanonicalNonHappyPathExportsEvidence'`
  - Result: `TEST FAILED`
  - Summary: `totalTestCount = 2`, `failedTests = 2`, `passedTests = 0`
  - Failures:
    - `XCTAssertTrue failed - Canonical full product checkpoint must be able to create an idea from the real UI`
    - `XCTAssertTrue failed - Canonical non-happy-path checkpoint must be able to create an idea from the real UI`

### Runtime artifact checks

- Searched default run storage for `delivery-receipt`, `connect-upload-receipt`, `evidence-pack*`, and `signoff-decision.json` artifacts:
  - `find "$HOME/Library/Application Support/Chainworks Forge/runs" -maxdepth 3 ...`
  - Result: no matching files found during this audit
- Attempted Desktop export-root enumeration for exported evidence/sign-off packets:
  - `find "$HOME/Desktop" -maxdepth 2 ...`
  - Result: blocked by host privacy permissions in this environment

## Recommended Next Actions

1. Wire `BenchmarkRunRecorder`, `MVPSignOffEvaluator`, and `SignOffEvidencePackBuilder` into the shared repo-backed run completion/export path and persist `Run.experimentCohortID` at run creation.
2. Enforce `MVPBoundaryPolicy.validateAttachment(...)` before run start and render explicit `reference_only` / `rejected` UI states instead of a raw path-only attachment label.
3. Integrate `OutputRetrievalSLOProbe` into report/export openings and add explicit loading, timeout, and retry states to `RunReportView` / `CompletedRunExportHub`.
4. Fix the `Ideas` owner path used by `testFullProductCheckpointCanonicalExecution()` and `testFullProductCheckpointCanonicalNonHappyPathExportsEvidence()`, then rerun both and preserve the resulting evidence packs.
5. Re-run Proposal 007 and Proposal 008 sign-off evidence on current `HEAD` only after the real happy-path and recovered non-happy-path packets exist on disk.
