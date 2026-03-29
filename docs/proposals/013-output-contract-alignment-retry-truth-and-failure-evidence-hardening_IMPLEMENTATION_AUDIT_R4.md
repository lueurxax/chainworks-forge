# Proposal 013: Output Contract Alignment, Declarative Runtime Coverage, Retry Truth, and Failure Evidence Hardening Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `5c870b4` |
| Working Tree | `dirty (36 modified, 24 untracked)` |
| Audited At | `2026-03-29T16:16:18+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Implemented` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`R4` is not a no-delta repeat. The current tree closes several real `R3` gaps: `OutputContractSchemaV2` now carries `humanFormat`, `rawArtifactName`, and `normalizedArtifactName`; `ProposalReviewContractAdapter` is live in `ArtifactPersistenceOrderingPolicy`; `ArtifactManager` now resolves `supersedesAgentArtifactID` from the prior artifact and marks `reused_sibling_reference`; `RecoverySheet` now explains reuse vs re-execution and same-run vs clone semantics; `CompactionMetadata` now records explicit outcome truth and `WorkflowOrchestrator` persists it on success/failure; local macOS `build` passed; and focused `Proposal013Tests` passed `27/27`.

Proposal 013 is still not `Implemented`. One acceptance-class gap remains `Missing`: Section `10.2` app-level proof is still absent in a form that is both executable and sufficient. A dedicated `UITestProposal013EvidenceSurface` now exists, but it is not reachable from the app’s `UISurface` enum / switch and it still stops short of proving retry-to-success, prior-attempt inspectability after success, and declarative-coverage export/logging. `REQ-001`, `REQ-003`, `REQ-004`, and `REQ-011` also remain partial because live proposal-review persistence still does not materialize the explicit machine + human companion artifact pair promised by `4.3`, same-stage agent-retry truth is still only partially proved, and the focused suite still does not cover every `10.1` bullet.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Section `10.2` app-level proof is still missing | High |
| Architecture | At Risk | proposal-review dual-artifact truth is still incomplete on the live persistence path | High |
| Product | At Risk | the proposal-owned app proof owner exists but is not actually handoff-ready | High |
| UI | Acceptable | recovery and failed-stage evidence surfaces are materially present | Medium |
| UX | Acceptable | recovery explanations are now clearer, but the proof owner does not yet exercise the promised end-to-end retry story | Medium |
| Readiness | Not Ready | targeted non-UI proof is green, but one explicit acceptance gate still has no executable current-tree proof | High |

## Proposal Contract

### Scope

- Align output-contract truth across catalog, runtime validation, persistence, reporting, recovery, and export.
- Make retry lineage and same-run retry semantics explicit and durable.
- Preserve failed-stage evidence even when validation fails after output generation.
- Extend shell-owned recovery surfaces with narrow retry truth and evidence explanation.
- Harden Appendix B Tier 1 YAML surfaces (`contracts.*`, `backend_profiles.*.structured_output`) into executable truth or fail-closed behavior.
- Bound oversized proposal outputs with explicit compaction metadata.

### Locked Decisions

- `AgentCatalog.contracts` remains the canonical contract authority.
- `OutputContractResolverV2` is the runtime reader for contract truth.
- `Retry Failed Agent`, `Retry Failed Stage`, and `Clone Run` remain distinct persisted actions.
- Stage-attempt artifacts stay immutable; agent-only retry uses a disjoint namespace.
- Recovery/report/export surfaces must point back to `ValidationFailureRecord` or the failed-stage evidence packet.
- Mandatory-tier YAML rows must be enforced or fail closed.

### Primary User Flows

- Proposal-review stage produces contract-bound outputs and validates them against catalog-backed truth.
- A validation failure preserves raw outputs and evidence instead of collapsing into opaque blockage.
- The operator inspects failed-stage evidence and chooses the narrowest valid recovery action.
- A same-run retry completes without cloning the run while prior failed-attempt evidence remains inspectable.
- Terminal runs persist declarative-coverage truth for the active contract / YAML enforcement surface.

### UI Commitments

- `RecoverySheet` and `BlockedRunRecoveryView` expose `Retry Failed Agent`, `Retry Failed Stage`, `Clone Run (Frozen Snapshot)`, and `Clone Run (Current Config)` when valid.
- Recovery surfaces explain reuse vs re-execution and link to canonical failure evidence.
- A shell-owned `FailedStageEvidencePanel` shows raw output presence, validation failure, receipt/transcript availability, and next-action context.

### UX Commitments

- The operator can distinguish transport failure from post-generation validation failure.
- Output-contract mismatch and post-generation validation failure default to operator-mediated recovery posture rather than blind auto-retry.
- Retry lineage remains truthful across reports and recovery surfaces.

### Acceptance Criteria

1. proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
2. failed stages that produced outputs preserve raw outputs, receipts/transcripts or equivalent evidence, and validation failure records
3. retry-in-place no longer resets attempt numbering or obscures stage lineage
4. same-stage `Retry Failed Agent` has explicit artifact / receipt / transcript storage truth without collisions
5. blocked-run recovery surfaces expose the narrowest valid retry action before clone-run
6. a canonical regression proves failed review stages can be retried and completed without creating a new run
7. recovery, reporting, and export surfaces reference canonical failure evidence rather than only derived summaries
8. proposal drafting oversized-output failures are bounded by explicit compaction policy and evidence
9. mandatory-tier YAML fields are enforced or rejected by runtime/preflight
10. Appendix B tiering is persisted and testable

### Test / Evidence Requirements

- Section `10.1` targeted unit/integration proof.
- Section `10.2` app-launched proof.
- Section `10.3` canonical motivating-class regression.

### Explicit Exclusions

- provider-family expansion
- repo-backed delivery changes already owned by Proposal 007
- general UI polish already owned by Proposal 012
- broad migration of historical runs

## Proposal Fidelity / Divergence

### Matches

- `OutputContractResolverV2`, `StructuredOutputSchemaGate`, `ArtifactPersistenceOrderingPolicy`, `ValidationFailureRecord`, `FailedStageEvidenceBuilder`, `FailedStageEvidencePanel`, `StageRetryCoordinator`, `DeclarativeCoverageReport`, `OutputContractDeclarativeBridge`, and `ProposalDraftCompactionPolicy` all exist on the current tree.
- `OutputContractSchemaV2` now carries `humanFormat`, `rawArtifactName`, and `normalizedArtifactName`.
- `ProposalReviewContractAdapter` is now live on the validation path through `ArtifactPersistenceOrderingPolicy`.
- `ArtifactStorage` and `ArtifactPersistenceOrderingPolicy` still use the disjoint `agent-retry-{agentAttemptNumber}` namespace, and `ArtifactManager` now writes correct `supersedesAgentArtifactID` plus `reused_sibling_reference` metadata.
- `RecoverySheet` now includes one-line action explanations matching Section `7.2`.
- `CompactionMetadata` now includes explicit outcome truth and `WorkflowOrchestrator` updates it on success/failure.
- Local `build` passed and focused `Proposal013Tests` passed `27/27`.

### Divergences

- Proposal-review outputs still do not persist the explicit machine payload plus human companion pair promised by `4.3 Rule 2`. The live path still accepts markdown review output under `structured_with_human_companion` rather than persisting a dual-artifact pair.
- The proposal-owned `10.2` app-level proof owner is not wired into the app’s `UISurface` enum / switch and no UI test or test-gate path executes it.
- Even if wired, the current `UITestProposal013EvidenceSurface` stops after blocked-run proof and does not prove retry-to-success, post-success inspectability of prior failed-attempt artifacts, or declarative-coverage export/logging.
- The focused `10.1` suite is still missing explicit tests for retry-in-place attempt-number persistence and clone-run versus retry lineage.

### Ambiguities / Evidence Gaps

- No current-head app-launched or screenshot-bearing Proposal 013 proof bundle was surfaced in `Chainworks ForgeUITests`.
- No current-head proof shows live proposal-review persistence writing a separate machine artifact and human companion artifact for one review output.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 4 |
| Missing | 1 |
| Not Verifiable | 0 |

## Track 1: Objective Proposal-Conformance Audit

### REQ-001 Proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
- Proposal Source: `4.2`, `4.3`, `4.4`, acceptance criterion `1`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-test.EGlmTX/p013-r4-tests.xcresult`
- Gap / Note: This is stronger than `R3`: schema fields now exist and the adapter is live. The requirement is still not closed because `ProposalReviewContractAdapter.resolveReviewSchema(...)` does not pass `outputName`, so `rawArtifactName` / `normalizedArtifactName` stay `nil` on the review path, and the live runtime still accepts markdown as sufficient under `structured_with_human_companion` instead of persisting both machine-valid structured output and an explicit human companion artifact.

### REQ-002 Failed stages that produced outputs preserve raw outputs, receipt/transcript evidence, and validation failure records
- Proposal Source: `6.2`, `6.3`, acceptance criterion `2`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ValidationFailureRecord.swift`
  - `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-test.EGlmTX/p013-r4-tests.xcresult`
- Gap / Note: The live path still preserves raw outputs before validation, persists validation failure as first-class evidence, and writes a stage-level evidence packet. This remains closed.

### REQ-003 Retry-in-place preserves attempt truth and stage lineage
- Proposal Source: `5.2`, `5.3`, acceptance criterion `3`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/BlockedStageReportBuilder.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The stage/agent lineage model is explicit and the canonical regression still proves same-run retry-to-success. The requirement remains partial because the focused suite still does not explicitly prove retry-in-place attempt-number persistence and clone-run versus retry lineage as separate `10.1` bullets, and the app-level `10.2` proof is still missing.

### REQ-004 Same-stage `Retry Failed Agent` has explicit artifact / receipt / transcript storage truth with no collisions
- Proposal Source: `5.4`, acceptance criterion `4`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
- Gap / Note: This is materially better than `R3`: the disjoint namespace is live, `supersedesAgentArtifactID` is now resolved from the prior artifact instead of an execution ID, and `reused_sibling_reference` is now written. It is still not fully closed because same-stage agent retry is not exercised by a dedicated test proving no-collision receipt/transcript storage and effective-output precedence, and the proposal’s “same frozen logical snapshot” proof is still inferred from execution context rather than explicitly demonstrated by app-level evidence.

### REQ-005 Blocked-run recovery surfaces expose the narrowest valid retry action before clone-run
- Proposal Source: `7.2`, acceptance criterion `5`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: The surfaces now expose retry-first actions before clone-run, and both owners include one-line explanation text covering reuse / re-execution / same-run-vs-clone semantics. This closes the old explanation gap.

### REQ-006 A canonical regression proves failed review stages can be retried and completed without creating a new run
- Proposal Source: `10.3`, acceptance criterion `6`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-test.EGlmTX/p013-r4-tests.xcresult`
- Gap / Note: The canonical regression remains green and still proves failure -> preserved evidence -> same-run retry -> success in one `runID`.

### REQ-007 Recovery, reporting, and export surfaces reference the canonical `ValidationFailureRecord` or failed-stage evidence packet
- Proposal Source: `6.3`, `7.3`, acceptance criterion `7`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Recovery surfaces, run reports, and evidence export continue to read canonical failed-stage evidence rather than only summary prose.

### REQ-008 Proposal drafting oversized-output failures are bounded by explicit compaction policy and evidence
- Proposal Source: `8.2`, acceptance criterion `8`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The old `R3` blocker is closed. `CompactionMetadata` now contains `stageOutcome`, and `WorkflowOrchestrator` updates it on both success and failure.

### REQ-009 Mandatory-tier YAML fields are enforced or rejected by runtime/preflight
- Proposal Source: `4.2.2`, `3 Layer Q`, acceptance criterion `9`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/StructuredOutputSchemaGate.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Contract resolution and `structured_output` enforcement remain live and tested.

### REQ-010 Appendix B tiering is persisted and testable
- Proposal Source: `10.1`, acceptance criterion `10`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/DeclarativeCoverageReport.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Appendix B tiering remains testable and emitted on the live orchestrator path.

### REQ-011 Section 10.1 unit and integration proof is complete
- Proposal Source: `10.1`
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-test.EGlmTX/p013-r4-tests.xcresult`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The suite still covers contract validation, no-hardcoded-fallback resolution, schema gate behavior, declarative coverage, failed-stage evidence, compaction metadata, and the canonical regression. It still does not explicitly cover every `10.1` bullet, especially retry-in-place attempt-number persistence and clone-run versus retry lineage.

### REQ-012 Section 10.2 app-level proof is complete
- Proposal Source: `10.2`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `scripts/test-gate.sh`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Gap / Note: A proposal-owned proof surface now exists (`UITestProposal013EvidenceSurface`), but it is not reachable from the app because `ContentView.UISurface` and the direct-surface switch do not include a Proposal 013 case, and no UI test or test-gate path executes it. The current proof surface is also incomplete relative to `10.2`: it does not prove retry succeeds without cloning, prior failed-attempt artifacts remain inspectable after success, or declarative-coverage export/logging.

## Track 2: Expert Findings

## Architecture Review

**Summary:** At Risk

### ARCH-001 Proposal-review companion truth is still only half-landed
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-001`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
- Why It Matters: The runtime no longer looks split between V1 and V2 contract authorities, but proposal-review persistence still does not actually realize the dual-artifact contract the proposal commits to. That keeps the most visible contract adopter still slightly ambiguous at the persistence seam.
- Recommended Action: Make the review path persist the explicit machine artifact plus human companion artifact pair and route schema resolution through output-name-aware artifact names on the live path.

## Product Review

**Summary:** At Risk

### PROD-001 The proposal-owned app proof owner is dead-end proof, not ship proof
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-012`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/ContentView.swift`
  - `scripts/test-gate.sh`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Why It Matters: Proposal 013 explicitly requires one app-launched proof of the user-facing failure -> recovery -> success loop. The repo now contains a proof surface, but it is not wired into the app and it still does not cover the whole user promise. That means the product-level proof owner exists mostly as dormant scaffolding.
- Recommended Action: Add a real `UISurface` case plus a UI test or gate entry for Proposal 013, then extend the proof to cover retry-to-success, post-success inspectability, and declarative-coverage export/logging.

## UI Review

**Summary:** Acceptable

No new UI-specific blocker surfaced beyond the proof-ownership issue already captured as `PROD-001` and `READY-001`.

## UX Review

**Summary:** Acceptable

The old recovery-explanation gap is materially improved: `RecoverySheet` now describes what will be reused, what will be re-executed, and whether the action stays in the same run or creates a new run.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Current-head non-UI proof is green, but one explicit acceptance gate is still absent
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `REQ-011`, `REQ-012`
- Evidence Type: tests-run, code
- Evidence:
  - local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-build.LgBsjy/p013-r4-build.xcresult`
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-test.EGlmTX/p013-r4-tests.xcresult`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/ContentView.swift`
  - `scripts/test-gate.sh`
- Why It Matters: The repo is close enough that the remaining blocker is now cleanly isolated. But it is still a blocker: Proposal 013 explicitly requires `10.2` app-level proof, and the current tree does not provide an executable owner for it.
- Recommended Action: wire the Proposal 013 direct surface into the app and a canonical gate, then run it as current-head proof before claiming implementation closure.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-build.LgBsjy/p013-r4-build.xcresult` |
| Focused proposal-scoped unit/integration suite executed | Pass | `Proposal013Tests` passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-test.EGlmTX/p013-r4-tests.xcresult` |
| Canonical motivating-class regression is green | Pass | `Canonical regression: strict contract mismatch blocks run, evidence survives, narrow retry available` passed in the focused suite |
| App-level proof owner is executable on the current tree | Fail | Proposal 013 proof surface is not wired into `UISurface` / direct-surface switching |
| App-level proof covers full `10.2` story | Fail | current proof surface stops before retry-success, post-success inspectability, and declarative-coverage export/logging |
| Recovery explanation contract is surfaced | Pass | `RecoverySheet` and `BlockedRunRecoveryView` now expose explanation text |
| Compaction outcome truth is persisted | Pass | `CompactionMetadata.stageOutcome` exists and is updated by `WorkflowOrchestrator` |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `md5 -q /Users/user/Documents/Chainworks Forge/docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- `rg -n "ProposalReviewContractAdapter|humanFormat|rawArtifactName|normalizedArtifactName|structured_with_human_companion|..." ...`
- `rg -n "reused_sibling_reference|supersedesAgentArtifactID|artifactLineageKind|agent-retry-|..." ...`
- `rg -n "p013-proof|Proposal 013 App-Level Proof|UITestProposal013EvidenceSurface|..." ...`
- `xcodebuild build -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath ... -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-build.LgBsjy/p013-r4-build.xcresult`
- `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath ... -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r4-test.EGlmTX/p013-r4-tests.xcresult -only-testing:'Chainworks ForgeTests/Proposal013Tests'`

## Recommended Next Actions

1. Wire `UITestProposal013EvidenceSurface` into `ContentView.UISurface` and the app/root direct-surface switch, then add a canonical UI/gate owner for it.
2. Extend the Proposal 013 proof owner to actually perform retry-to-success, verify prior failed-attempt inspectability after success, and export/log the declarative-coverage snapshot.
3. Finish the live proposal-review dual-artifact contract by persisting the explicit machine artifact plus human companion artifact pair instead of accepting markdown alone as sufficient runtime truth.

## Final Judgment

`Not Implemented`. `R4` closes several real `R3` code-path gaps, but Proposal 013 still fails one explicit acceptance gate (`10.2` app-level proof), and a few remaining partial seams mean the current tree is not yet honest to mark as fully implemented.
