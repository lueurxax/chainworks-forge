# Proposal 013: Output Contract Alignment, Declarative Runtime Coverage, Retry Truth, and Failure Evidence Hardening Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `5c870b4` |
| Working Tree | `dirty (36 modified, 22 untracked)` |
| Audited At | `2026-03-29T15:32:10+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Implemented` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`R3` is not a no-delta repeat. The current dirty tree closes several real `R2` blockers: live persistence now uses an `agent-retry-{agentAttemptNumber}` namespace, `ArtifactManager` now writes retry-lineage metadata, `EvidencePackBuilder` now exports failed-stage evidence plus compaction / validation payloads, contract-mismatch recovery no longer suggests blind retry first, and the canonical Proposal 013 regression now runs the full failure -> same-run retry -> success loop. Fresh proof is also stronger: local macOS `build` passed, and the focused `Proposal013Tests` suite passed `27/27`.

Proposal 013 is still not `Implemented`. One acceptance-class gap remains outright `Missing`, and several others are still only partial. The repo still has no current-head app-launched proof for Section `10.2`, same-stage `Retry Failed Agent` storage truth is still incomplete at the artifact-lineage level, proposal-review contract truth still relies on inferred `structured_with_human_companion` semantics rather than explicit dual-artifact persistence, and compaction metadata still omits the required outcome truth (`succeeded with compaction` vs `failed despite compaction`).

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Section `10.2` app-level proof is still absent on the current tree | High |
| Architecture | At Risk | agent-retry path is live, but artifact supersession / sibling-reference truth is still incomplete | High |
| Product | At Risk | canonical regression is now green, but the operator-facing same-run retry promise is still not app-level-proven | High |
| UI | At Risk | recovery surfaces exist, but `RecoverySheet` still does not expose the full reuse / re-execution explanation contract | Medium |
| UX | At Risk | contract-mismatch posture improved, but action reasoning is still split across surfaces instead of one consistent persisted recommendation model | Medium |
| Readiness | Not Ready | current-head app-launched proof required by `10.2` still has not been surfaced | High |

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
- `Retry Failed Agent`, `Retry Failed Stage`, and `Clone Run` are distinct persisted actions.
- Stage-attempt artifacts remain immutable; same-stage agent retry must use a disjoint namespace.
- Recovery/report/export surfaces must point back to `ValidationFailureRecord` or the failed-stage evidence packet.
- Mandatory-tier YAML rows must be enforced or fail closed.

### Primary User Flows

- Proposal-review stage produces contract-bound outputs and validates them against catalog-backed truth.
- A validation failure preserves raw outputs and evidence instead of collapsing into opaque blockage.
- The operator inspects failed-stage evidence and chooses the narrowest valid recovery action.
- A same-run retry completes without cloning the run, while prior failed-attempt evidence remains inspectable.
- Terminal runs persist declarative-coverage truth for the active contract / YAML enforcement surface.

### UI Commitments

- `RecoverySheet` and `BlockedRunRecoveryView` expose `Retry Failed Agent`, `Retry Failed Stage`, `Clone Run (Frozen Snapshot)`, and `Clone Run (Current Config)` when valid.
- Recovery surfaces explain reuse vs re-execution and link to canonical failure evidence.
- A shell-owned `FailedStageEvidencePanel` shows raw output presence, validation failure, receipt/transcript availability, and next action context.

### UX Commitments

- The operator can distinguish transport failure from post-generation validation failure.
- Output-contract mismatch and post-generation validation failure default to operator-mediated recovery posture rather than blind auto-retry.
- Retry lineage remains truthful across reports and recovery surfaces.

### Acceptance Criteria

1. Proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts.
2. Failed stages that produced outputs preserve raw outputs, receipts/transcripts or equivalent evidence, and validation failure records.
3. Retry-in-place no longer resets attempt numbering or obscures stage lineage.
4. Same-stage `Retry Failed Agent` has explicit artifact / receipt / transcript storage truth without collisions.
5. Blocked-run recovery surfaces expose the narrowest valid retry action before clone-run.
6. A canonical regression proves failed review stages can be retried and completed without creating a new run.
7. Recovery, reporting, and export surfaces reference canonical failure evidence rather than only derived summaries.
8. Proposal drafting oversized-output failures are bounded by explicit compaction policy and evidence.
9. Mandatory-tier YAML fields are enforced or fail-closed; non-mandatory fields are explicitly tiered.
10. Appendix B tiering is persisted and testable.

### Test / Evidence Requirements

- Section `10.1` targeted unit/integration proof.
- Section `10.2` app-launched proof: output generated, validation fails, evidence survives, narrow retry shown, retry succeeds in same run, prior failed-attempt evidence remains inspectable, declarative-coverage snapshot is exported or logged.
- Section `10.3` canonical motivating-class regression proving the full failure -> retry -> success loop.

### Explicit Exclusions

- provider-family expansion
- repo-backed delivery changes already owned by Proposal 007
- general UI polish work already owned by Proposal 012
- broad migration of historical runs

## Proposal Fidelity / Divergence

### Matches

- `OutputContractResolverV2`, `StructuredOutputSchemaGate`, `ArtifactPersistenceOrderingPolicy`, `ValidationFailureRecord`, `FailedStageEvidenceBuilder`, `FailedStageEvidencePanel`, `StageRetryCoordinator`, `DeclarativeCoverageReport`, `OutputContractDeclarativeBridge`, and `ProposalDraftCompactionPolicy` all exist on the current tree.
- Live persistence now routes retry artifacts through the disjoint `agent-retry-{agentAttemptNumber}` namespace in `ArtifactStorage.write(...)`.
- `ArtifactManager.persistOutputs(...)` now writes retry-lineage metadata onto `Artifact` records (`agentAttemptNumber`, `artifactLineageKind`, `supersedesAgentArtifactID`).
- `WorkflowOrchestrator` still preserves raw outputs before validation and now persists `outputEnvelopesJSON`, `validationFailureJSON`, and `stageExec.evidencePacketJSON` on the live path.
- `RecoverySheet` and `BlockedRunRecoveryView` both surface failure evidence through `FailedStageEvidencePanel`.
- `RecoveryCoordinator.recoveryContext(...)` now fail-closes to operator inspection posture for `outputContractMismatch` by leaving `suggestedAction` unset.
- `EvidencePackBuilder` now exports failed-stage evidence packets, validation failure records, compaction metadata, and the declarative-coverage report.
- The canonical Proposal 013 regression now proves same-run retry-to-success with preserved prior failure evidence, and the focused suite passed `27/27`.

### Divergences

- Proposal-review contract truth is still only partially aligned. `OutputContractSchemaV2` still derives `humanFormat`, `rawArtifactName`, and `normalizedArtifactName` as `nil`, and runtime still does not persist a distinct machine payload plus human companion pair for review outputs.
- Same-stage `Retry Failed Agent` storage truth is only partially complete. The path split is live, but `supersedesAgentArtifactID` is still populated from `supersedesAgentExecutionID`, and no live `reused_sibling_reference` artifact metadata is written.
- Recovery surfaces still do not fully satisfy the action-explanation contract across both owners. `BlockedRunRecoveryView` has one-line descriptions, but `RecoverySheet` still lists available actions without reuse / re-execution / same-run-vs-clone explanation text.
- Compaction metadata still omits the required outcome truth (`succeeded with compaction` vs `failed despite compaction`).

### Ambiguities / Evidence Gaps

- No current-head app-launched or screenshot-bearing proof was surfaced for Section `10.2`.
- No current-head proof shows proposal-review outputs persisting both machine payload and human companion as separate artifacts under `structured_with_human_companion`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 5 |
| Partially Implemented | 6 |
| Missing | 1 |
| Not Verifiable | 0 |

## Track 1: Objective Proposal-Conformance Audit

### REQ-001 Proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
- Proposal Source: `4.2`, `4.3`, `4.4`, acceptance criterion `1`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `examples/agents/agents.yaml`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Contract resolution is now catalog-driven on the live path, and the focused suite proves markdown review output can pass under `structured_with_human_companion`. But the schema is still inferred instead of fully declared: `humanFormat`, `rawArtifactName`, and `normalizedArtifactName` remain unset in the derived V2 schema, `ProposalReviewContractAdapter` is still test-only, and runtime does not persist the explicit machine + human companion artifact pair required by `4.3`.

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
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-test.s2u9CE/p013-r3-tests.xcresult`
- Gap / Note: The live path now persists raw outputs before validation, preserves failure records, and writes a stage-level evidence packet. This requirement is closed.

### REQ-003 Retry-in-place preserves attempt truth and stage lineage
- Proposal Source: `5.2`, `5.3`, acceptance criterion `3`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Stage and agent retry lineage are explicitly modeled, and the canonical regression now proves a same-run retry can complete successfully. What remains incomplete is truthful artifact-lineage closure for agent-only retry and current-head app-level proof that the operator sees that lineage coherently end to end.

### REQ-004 Same-stage `Retry Failed Agent` has explicit artifact / receipt / transcript storage truth with no collisions
- Proposal Source: `5.4`, acceptance criterion `4`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Models/Artifact.swift`
- Gap / Note: This is materially better than `R2`. The disjoint `agent-retry-{agentAttemptNumber}` namespace is now live and retry-lineage metadata is now written on persisted artifacts. But the lineage is still not fully truthful: `supersedesAgentArtifactID` is populated from an execution ID rather than a prior artifact ID, and the required `reused_sibling_reference` metadata is still not written for reused sibling outputs.

### REQ-005 Blocked-run recovery surfaces expose the narrowest valid retry action before clone-run
- Proposal Source: `7.2`, acceptance criterion `5`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: The surfaces now expose retry-first actions before clone-run, and contract mismatch no longer auto-suggests blind retry. The remaining gap is consistency with the full proposal UX contract: `RecoverySheet` still does not present the one-line explanation text describing reuse vs re-execution and same-run vs clone semantics for each action.

### REQ-006 A canonical regression proves failed review stages can be retried and completed without creating a new run
- Proposal Source: `10.3`, acceptance criterion `6`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-test.s2u9CE/p013-r3-tests.xcresult`
- Gap / Note: The canonical regression now executes the full failure -> retry -> success loop in the same `runID` and asserts that prior failed-attempt evidence remains inspectable after success. This closes the core motivating regression.

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
- Gap / Note: Recovery surfaces now read canonical failed-stage evidence, `RunReportBuilder` derives summaries from `stage.evidencePacketJSON`, and `EvidencePackBuilder` now exports the failed-stage packet plus validation failure and compaction payloads. This requirement is closed.

### REQ-008 Proposal drafting oversized-output failures are bounded by explicit compaction policy and evidence
- Proposal Source: `8.2`, acceptance criterion `8`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Compaction now runs on the live persistence path, stores `compactionMetadataJSON`, and is exported in evidence packs. The remaining contract gap is explicit in `8.2`: `CompactionMetadata` still does not record whether the stage succeeded with compaction or failed despite compaction.

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
- Gap / Note: Contract resolution and `structured_output` enforcement are now on the live runtime/preflight path. This requirement is closed.

### REQ-010 Appendix B tiering is persisted and testable
- Proposal Source: `10.1`, acceptance criterion `10`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/DeclarativeCoverageReport.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Appendix B tiering is now testable and emitted on the live orchestrator path. This requirement is closed.

### REQ-011 Section 10.1 unit and integration proof is complete
- Proposal Source: `10.1`
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - focused suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-test.s2u9CE/p013-r3-tests.xcresult`
- Gap / Note: The focused suite now covers contract resolution, structured-output validation, schema gate behavior, failure-record building, evidence-packet building, declarative-coverage reporting, compaction metadata, and the canonical retry-to-success regression. It still does not include explicit tests for same-stage agent-retry artifact precedence / sibling-reference truth or app-owned runtime surfaces.

### REQ-012 Sections 10.2 and 10.3 app-level proof are complete
- Proposal Source: `10.2`, `10.3`
- Status: Missing
- Evidence Type: code-search
- Evidence:
  - no current-head Proposal 013 UI test or app-launched artifact surfaced from `Chainworks ForgeUITests`
  - `Chainworks ForgeUITests`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The canonical integration regression is now green, but the proposal explicitly requires at least one app-launched proof showing validation failure, evidence preservation, narrow retry visibility, same-run retry success, prior failed-attempt inspectability, and declarative-coverage export/logging. That proof still does not exist on the current tree.

## Track 2: Expert Findings

## Architecture Review

**Summary:** At Risk

### ARCH-001 Agent-retry storage truth is live but not yet fully canonical
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `5.4`, `REQ-004`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Models/Artifact.swift`
- Why It Matters: The hardest architectural part of Proposal 013 was preventing same-stage retry from corrupting immutable stage-attempt truth. The current tree now lands the path split, which is real progress, but the artifact-level supersession model is still not fully canonical because artifact records point back to prior executions rather than prior artifacts and omit the `reused_sibling_reference` layer.
- Recommended Action: Resolve `supersedesAgentArtifactID` against the actual superseded artifact records and write explicit `reused_sibling_reference` artifacts or metadata for sibling reuse.

### ARCH-002 Review-contract dual-artifact truth is still inferred rather than persisted
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `4.3`, `REQ-001`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Why It Matters: Proposal 013 did not just want validation to become looser; it wanted dual-format truth to become explicit. Right now `structured_with_human_companion` is largely an inferred validation policy, not a persisted two-artifact contract.
- Recommended Action: Extend live persistence to emit a distinct machine artifact and human companion artifact for proposal-review outputs, with schema fields no longer defaulting to `nil`.

## Product Review

**Summary:** At Risk

### PROD-001 The core motivating regression is now fixed, but the operator promise is still not runtime-proven
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `1`, `9`, `10.2`, `10.3`, `REQ-006`, `REQ-012`
- Evidence Type: tests-run, code
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-test.s2u9CE/p013-r3-tests.xcresult`
- Why It Matters: The repo now proves the underlying runtime defect is recoverable without cloning. That is the most important product gain in `R3`. But the proposal promised the engineer could verify this from app surfaces and persisted proof, not just from a focused test target.
- Recommended Action: Add one proposal-scoped app-launched replay that drives a real blocked run through evidence inspection and same-run retry completion.

## UI Review

**Summary:** At Risk

### UI-001 Recovery explanation is still asymmetric across the two shell owners
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `7.2`, `7.3`, `REQ-005`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Why It Matters: Proposal 013 explicitly made explanation part of the contract, not optional copy. `BlockedRunRecoveryView` now has action descriptions, but `RecoverySheet` still renders the action list without the same reuse / re-execution / same-run-vs-clone explanation. That weakens trust in the first recovery surface the operator sees.
- Recommended Action: Render the `RecoveryActionDetail` explanation model, or equivalent persisted explanation text, in `RecoverySheet` as well.

## UX Review

**Summary:** At Risk

### UX-001 Operator-mediated posture exists, but recommendation truth is still split
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `5.4`, `7.2`, `REQ-005`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Why It Matters: The worst UX bug from `R2` is closed: contract mismatch no longer suggests retry-first. But the recommendation model is still split between `StageRetryCoordinator.narrowestRecoveryAction(...)` and `RecoveryCoordinator.recoveryContext(...)` rather than one persisted truth path, which makes recommendation provenance harder to trust.
- Recommended Action: Drive both surfaces from one persisted `RecoveryActionSnapshot` / `RecoveryActionDetail` truth path instead of parallel lightweight summaries.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Current-head proof still stops before the required app-launched replay
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `10.2`, `REQ-012`
- Evidence Type: tests-run, code-search
- Evidence:
  - local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-build.93AK7g/p013-r3-build.xcresult`
  - focused proposal suite passed `27/27`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-test.s2u9CE/p013-r3-tests.xcresult`
  - no current-head Proposal 013 app-level runtime artifact surfaced
- Why It Matters: The code and focused proof are now strong enough to justify a `Partial` implementation story, but not a full closure story. The proposal itself requires an app-launched run as final evidence.
- Recommended Action: Add and execute one Proposal 013 app-level proof that shows validation failure, evidence visibility, same-run retry success, inspectable prior artifacts, and declarative-coverage export/logging.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-build.93AK7g/p013-r3-build.xcresult` |
| Core user flow runtime-validated | Partial | canonical same-run retry regression is green, but no app-launched proof surfaced |
| Empty/loading/error states covered | Partial | recovery/evidence UI exists in code, but no Proposal 013-specific runtime replay surfaced |
| Accessibility risk acceptable | Not Checked | no Proposal 013-specific UI/accessibility proof surfaced |
| Localization risk acceptable | Not Checked | no localized proof surfaced |
| Critical tests executed | Pass | focused `Proposal013Tests` passed `27/27` |
| Privacy/permissions/entitlements reviewed | Partial | no new permission risk surfaced in focused audit, but not a proposal-specific review axis here |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "agentAttemptNumber|artifactLineageKind|supersedesAgentArtifactID|reused_sibling_reference|agent-retry-" Chainworks Forge Chainworks ForgeTests`
- `rg -n "validationFailureJSON|evidencePacketJSON|compactionMetadataJSON|declarative_coverage_report|FailedStageEvidencePanel|ValidationFailureRecord" Chainworks Forge Chainworks ForgeTests`
- `rg -n "retryStage\\(|retryFailedAgent|suggestedAction = nil|outputContractMismatch" Chainworks Forge Chainworks ForgeTests`
- `rg -n "Proposal013|FailedStageEvidencePanel|RecoverySheet|BlockedRunRecoveryView|declarative coverage|declarativeCoverage|validation fails|retry succeeds" Chainworks ForgeUITests Chainworks ForgeTests`
- `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-build.93AK7g -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-build.93AK7g/p013-r3-build.xcresult`
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-test.s2u9CE -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r3-test.s2u9CE/p013-r3-tests.xcresult -only-testing:'Chainworks ForgeTests/Proposal013Tests'`

## Recommended Next Actions

1. Add one current-head app-launched Proposal 013 proof and make it the authoritative closure artifact for `REQ-012`.
2. Finish artifact-lineage truth for same-stage `Retry Failed Agent`: resolve superseded artifact IDs correctly and emit `reused_sibling_reference` metadata.
3. Persist explicit dual-artifact truth for proposal-review outputs under `structured_with_human_companion`.
4. Extend compaction metadata with the required outcome truth and surface it in report/export consumers.
