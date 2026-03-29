# Proposal 013: Output Contract Alignment, Declarative Runtime Coverage, Retry Truth, and Failure Evidence Hardening Multi-Lens Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `5c870b4` |
| Working Tree | `dirty (33 modified, 21 untracked)` |
| Audited At | `2026-03-29T15:12:18+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Implemented` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 013 is materially closer than `R1`. The repo builds, targeted `Proposal013Tests` now pass `26/26`, `OutputContractResolverV2` is wired on the live runtime path, validation-failure state is persisted onto `AgentExecution` and `StageExecution`, stage-level evidence packets are now written, `RecoverySheet` and `BlockedRunRecoveryView` both surface failure evidence, `StructuredOutputSchemaGate` is integrated into preflight, `DeclarativeCoverageReport` is emitted at terminal state, and proposal-output compaction now runs on the live persistence path.

The proposal is still not `Implemented`. Two acceptance-class gaps remain open and they are core, not cosmetic: same-stage `Retry Failed Agent` still has no live artifact namespace / supersession truth, and the canonical motivating-class proof still stops before a successful same-run retry. Export surfaces also remain behind the proposal contract because they do not consume the canonical failed-stage packet or compaction truth.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | same-stage agent-retry storage truth is still not on the live persistence path | High |
| Architecture | At Risk | retry lineage exists in models, but artifact persistence still collapses back to stage-attempt-only paths | High |
| Product | At Risk | the motivating failure class is preserved and diagnosable, but not yet proven recoverable to completion in the same run | High |
| UI | At Risk | failure-evidence UI exists, but current-head proof for those states is code-only rather than runtime-validated | Medium |
| UX | At Risk | contract-mismatch recovery still defaults to retry-first instead of the operator-mediated posture promised by the proposal | High |
| Readiness | Not Ready | Sections `10.2` and `10.3` still lack current-head app-level proof | High |

## Proposal Contract

### Scope

- Align output-contract truth across catalog, runtime validation, persistence, reporting, and recovery.
- Make retry lineage and same-run retry semantics explicit and durable.
- Preserve failed-stage evidence even when validation fails after output generation.
- Extend shell-owned recovery surfaces with narrow retry truth and evidence explanation.
- Harden Appendix B Tier 1 YAML surfaces (`contracts.*`, `backend_profiles.*.structured_output`) into executable truth or fail-closed behavior.
- Bound oversized proposal outputs with explicit compaction metadata.

### Locked Decisions

- `AgentCatalog.contracts` remains the canonical contract authority.
- `OutputContractResolverV2` becomes the runtime reader for contract truth.
- `Retry Failed Agent`, `Retry Failed Stage`, and `Clone Run` are distinct persisted actions.
- Stage-attempt artifacts remain immutable; same-stage agent retry must use a disjoint namespace.
- Recovery/report/export surfaces must point back to the canonical `ValidationFailureRecord` or failed-stage evidence packet.
- Mandatory-tier YAML rows must be enforced or fail closed.

### Primary User Flows

- Proposal-review stage produces structured outputs and validates them against catalog-backed contracts.
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

1. Proposal-review output contracts are aligned across agent catalog, runtime validation, and persisted artifacts.
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
- Live runtime now uses `ArtifactPersistenceOrderingPolicy` from `WorkflowOrchestrator` to persist raw outputs before validation, persist `outputEnvelopesJSON`, persist `validationFailureJSON`, and write `stageExec.evidencePacketJSON`.
- `RecoverySheet` and `BlockedRunRecoveryView` both build and present failure-evidence UI using `FailedStageEvidencePanel`.
- `StructuredOutputSchemaGate` is now wired into `PreflightService`.
- `DeclarativeCoverageReport` is now emitted from `WorkflowOrchestrator` at terminal state.
- Proposal-output compaction is now invoked on the live persistence path and stores `compactionMetadataJSON` on `AgentExecution`.
- Local macOS `build` succeeded and the focused `Proposal013Tests` suite passed `26/26`.

### Divergences

- Same-stage agent-retry storage truth is still not implemented: artifact persistence never enters the `agent-retry-{agentAttemptNumber}` namespace and never writes artifact-level lineage metadata.
- The canonical regression still proves failure preservation, not successful same-run recovery completion.
- Export surfaces remain behind the proposal contract: `CompletedRunExportHub` / `EvidencePackBuilder` do not consume the canonical failed-stage packet, validation-failure record, or compaction metadata.
- Recovery default posture for contract mismatch is still retry-first in the UI path, not operator-mediated inspection-first.

### Ambiguities / Evidence Gaps

- No current-head app-launched or screenshot-bearing proof was surfaced for the Proposal 013 recovery/evidence flow.
- No current-head proof shows proposal-review outputs persisting both machine payload and human companion as separate artifacts under `structured_with_human_companion`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 3 |
| Partially Implemented | 6 |
| Missing | 3 |
| Not Verifiable | 0 |

## Track 1: Objective Proposal-Conformance Audit

### REQ-001 Proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
- Proposal Source: `4.2`, `4.3`, `4.4`, acceptance criterion `1`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/OutputContractTemplates.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `examples/agents/agents.yaml`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Runtime contract lookup is now catalog-driven on the live path, but review-contract alignment is still incomplete. `ProposalReviewContractAdapter` remains unused in runtime, and the current implementation accepts markdown-only `proposal_review_*` payloads under a JSON contract without persisting a distinct machine-valid payload plus human companion pair.

### REQ-002 Failed stages that produced outputs preserve raw outputs, receipt/transcript evidence, and validation failure records
- Proposal Source: `6.2`, `6.3`, acceptance criterion `2`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/ExecutionReceiptBuilder.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ValidationFailureRecord.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The live executor path still returns receipt/transcript artifacts inside `AgentResult.outputs`, and the orchestration path now persists raw outputs before validation plus `ValidationFailureRecord` and stage evidence. The remaining gap is not preservation itself, but downstream export/report consumption, covered in `REQ-007`.

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
- Gap / Note: Stage- and agent-attempt lineage are now modeled explicitly and recovery actions use `StageRetryCoordinator`. What is still missing is a current-head proof that same-run retry actually executes through to success while preserving truthful lineage end to end.

### REQ-004 Same-stage `Retry Failed Agent` has explicit artifact / receipt / transcript storage truth with no collisions
- Proposal Source: `5.4`, acceptance criterion `4`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Models/Artifact.swift`
- Gap / Note: `agentRetryNamespace(...)` exists only as a helper. Live persistence still writes `{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}` via `ArtifactStorage.write(...)`. `Artifact.agentAttemptNumber`, `supersedesAgentArtifactID`, and `artifactLineageKind` are declared but never written. Same-stage agent retry would still collide conceptually with immutable stage-attempt storage truth.

### REQ-005 Blocked-run recovery surfaces expose the narrowest valid retry action before clone-run
- Proposal Source: `7.2`, acceptance criterion `5`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: Recovery surfaces now expose retry/stage/clone actions and can show failure evidence. The remaining gap is that the UI suggestion path still comes from `availableActions.first`, not from the operator-mediated `narrowestRecoveryAction(...)` posture promised for contract mismatch.

### REQ-006 A canonical regression proves failed review stages can be retried and completed without creating a new run
- Proposal Source: `10.3`, acceptance criterion `6`
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - focused suite passed `26/26`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-test.H92oUp/p013-r2-tests.xcresult`
- Gap / Note: The named canonical regression only proves failure preservation plus retry availability. It does not execute the retry, complete the stage in the same run, or verify that prior failed-attempt artifacts remain inspectable after success.

### REQ-007 Recovery, reporting, and export surfaces reference the canonical `ValidationFailureRecord` or failed-stage evidence packet
- Proposal Source: `6.3`, `7.3`, acceptance criterion `7`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
- Gap / Note: Recovery surfaces and `RunReportBuilder` now read canonical failure evidence. Export surfaces still do not. `CompletedRunExportHub` and `EvidencePackBuilder` export generic artifacts but do not explicitly consume `ValidationFailureRecord`, `FailedStageEvidencePacket`, or compaction truth.

### REQ-008 Proposal drafting oversized-output failures are bounded by explicit compaction policy and evidence
- Proposal Source: `8.2`, acceptance criterion `8`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Compaction now runs on the live persistence path and stores `compactionMetadataJSON`. The contract is still incomplete because the persisted metadata does not capture outcome truth such as "succeeded with compaction" vs "failed despite compaction", and no current report/export surface consumes that evidence.

### REQ-009 Mandatory-tier YAML fields are enforced or rejected by runtime/preflight
- Proposal Source: `4.2.2`, `3 Layer Q`, acceptance criterion `9`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/OutputContractTemplates.swift`
  - `Chainworks Forge/Engine/SimulatedAgentExecutor.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/StructuredOutputSchemaGate.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: On the current app tree, runtime consumers now read contract truth through `OutputContractResolverV2`, and `backend_profiles.*.structured_output` now flows through a fail-closed preflight gate.

### REQ-010 Appendix B tiering is persisted and testable
- Proposal Source: `10.1`, acceptance criterion `10`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/DeclarativeCoverageReport.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: `DeclarativeCoverageReport` is both testable in isolation and emitted from the live workflow orchestrator at terminal state. The remaining open issue is not persistence, but the absence of current-head app-level proof that exports or logs it in the motivating-class flow.

### REQ-011 Section 10.1 unit and integration proof is complete
- Proposal Source: `10.1`
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - focused suite passed `26/26`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-test.H92oUp/p013-r2-tests.xcresult`
- Gap / Note: The focused suite covers schema derivation, V2 resolution, schema gate behavior, failure-record building, evidence-packet building, declarative-coverage report content, compaction metadata, and a bounded motivating-class failure path. It does not close full same-run retry completion, export consumption, or app-launched proof.

### REQ-012 Sections 10.2 and 10.3 app-level proof are complete
- Proposal Source: `10.2`, `10.3`
- Status: Missing
- Evidence Type: code-search
- Evidence:
  - no current-head `Chainworks ForgeUITests` or proposal-scoped app-launched artifact surfaced for Proposal 013
  - `Chainworks ForgeUITests`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The repo still lacks a current-head app-launched proof showing validation failure, evidence-panel visibility, same-run retry success, post-success inspectability of prior failed-attempt artifacts, and declarative-coverage export/logging on the motivating class.

## Track 2: Expert Findings

## Architecture Review

**Summary:** At Risk

### ARCH-001 Agent-retry storage truth is still dead code
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `5.4`, `REQ-004`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Models/Artifact.swift`
- Why It Matters: The proposal's strongest invariants are about preserving immutable stage-attempt artifacts while allowing truthful same-stage retry lineage. The current tree still models that namespace and metadata without using them, so the most important architectural safeguard remains unlanded.
- Recommended Action: Route same-stage retry persistence through an explicit `agent-retry-{agentAttemptNumber}` namespace and write artifact-level lineage metadata (`agentAttemptNumber`, `supersedesAgentArtifactID`, `artifactLineageKind`) on the live path.

## Product Review

**Summary:** At Risk

### PROD-001 The motivating failure class is diagnosable, but not yet proven recoverable to completion
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `1`, `9`, `10.2`, `10.3`, `REQ-006`, `REQ-012`
- Evidence Type: tests-run, code
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-test.H92oUp/p013-r2-tests.xcresult`
- Why It Matters: The current implementation can now explain and preserve the failure. It still does not prove that the operator can stay in the same run, perform the narrow retry, and finish cleanly with truthful history. That is the product promise that justified the proposal.
- Recommended Action: Add one canonical current-head regression that executes the full failure -> evidence -> retry-agent or retry-stage -> success path and asserts inspectability of prior failed-attempt artifacts after success.

## UI Review

**Summary:** At Risk

### UI-001 Proposal 013 surfaces exist, but current-head UI proof is still code-only
- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: `7.2`, `7.3`, `REQ-005`, `REQ-012`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift`
- Why It Matters: The shell-owned evidence UI now exists, which is real progress. But the audit still cannot prove the critical visual states on the current tree: failure explanation, evidence availability, reuse/re-execute explanation, and the post-failure recovery action sequence.
- Recommended Action: Add a proposal-scoped UI replay or direct-surface suite for `RecoverySheet`, `BlockedRunRecoveryView`, and `FailedStageEvidencePanel`.

## UX Review

**Summary:** At Risk

### UX-001 Contract-mismatch recovery still defaults to retry-first instead of operator inspection
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `5.4`, `7.2`, `REQ-005`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Why It Matters: The proposal explicitly says output-contract mismatch is non-auto-retryable by default and should become operator-mediated after the evidence panel explains the failure. `StageRetryCoordinator.narrowestRecoveryAction(...)` models that posture, but `RecoveryCoordinator.recoveryContext(...)` still suggests `availableActions.first`, so the operator sees retry-first instead of inspection-first.
- Recommended Action: Drive suggested recovery UI from `narrowestRecoveryAction(...)` / persisted `RecoveryActionSnapshot` instead of from raw action ordering.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Current-head proof stops at focused unit coverage
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `10.2`, `10.3`, `REQ-011`, `REQ-012`
- Evidence Type: tests-run, code
- Evidence:
  - local build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-build.rxIy3d/p013-r2-build.xcresult`
  - focused proposal suite passed `26/26`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-test.H92oUp/p013-r2-tests.xcresult`
  - no current-head Proposal 013 app-level UI/runtime artifact surfaced
- Why It Matters: The repo is buildable and the focused suite is healthy, but the proposal itself requires proof that only runtime/app-level validation can close.
- Recommended Action: Add and execute one proposal-scoped app-level proof on the current tree that covers validation failure, evidence visibility, same-run retry completion, and post-success inspectability.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-build.rxIy3d/p013-r2-build.xcresult` |
| Core user flow runtime-validated | Partial | failure preservation is integration-tested, but same-run retry completion is not |
| Empty/loading/error states covered | Partial | code exists for recovery/evidence states; no current-head UI/runtime replay surfaced |
| Accessibility risk acceptable | Not Checked | no Proposal 013-specific UI/accessibility proof surfaced |
| Localization risk acceptable | Not Checked | no localized proof surfaced |
| Critical tests executed | Pass | focused `Proposal013Tests` passed `26/26` |
| Privacy/permissions/entitlements reviewed | Partial | no new permission risk surfaced in focused audit, but not a proposal-specific review axis here |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `rg -n "OutputContractResolverV2|OutputContractResolver\\b|OutputContractDeclarativeBridge|ProposalReviewContractAdapter|ProposalDraftCompactionPolicy|FailedStageEvidenceBuilder|ValidationFailureRecord|BlockedStageReportBuilder|ArtifactPersistenceOrderingPolicy|agentRetryNamespace|validationFailureJSON|evidencePacketJSON|compactionMetadataJSON|artifactLineageKind|agentAttemptNumber|supersedesAgentArtifactID|FailedStageEvidencePanel|DeclarativeCoverageReport|StageRetryCoordinator" Chainworks Forge Chainworks ForgeTests Chainworks ForgeUITests`
- `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-build.rxIy3d -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-build.rxIy3d/p013-r2-build.xcresult`
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-test.H92oUp -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r2-test.H92oUp/p013-r2-tests.xcresult -only-testing:'Chainworks ForgeTests/Proposal013Tests'`

## Recommended Next Actions

1. Implement the live `agent-retry-{agentAttemptNumber}` persistence namespace and write artifact-level lineage metadata.
2. Add one canonical current-head regression that executes failure -> evidence -> same-run retry -> success and asserts prior failed-attempt inspectability.
3. Route suggested recovery UI through persisted `RecoveryActionSnapshot` / `narrowestRecoveryAction(...)` so contract mismatch becomes operator-mediated by default.
4. Extend export surfaces to consume the canonical failed-stage evidence packet and compaction truth instead of exporting only generic artifact copies.
