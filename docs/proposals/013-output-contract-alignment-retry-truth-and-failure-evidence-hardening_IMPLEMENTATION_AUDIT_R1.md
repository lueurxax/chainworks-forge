# Proposal 013: Output Contract Alignment, Declarative Runtime Coverage, Retry Truth, and Failure Evidence Hardening Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `5c870b4` |
| Working Tree | `dirty (26 modified, 20 untracked)` |
| Audited At | `2026-03-29T14:40:35+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Implemented` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

Proposal 013 has landed a meaningful amount of code on the current tree. The repo builds cleanly, the dedicated `Proposal013Tests` suite passes `25/25`, the new Layer M/N/O/P/Q types exist, and the runtime now preserves more failure context than the motivating incident originally allowed.

The proposal still does not qualify as `Implemented`. Several proposal-level contracts remain unwired or only unit-tested: runtime still splits contract truth between `OutputContractResolverV2` and the legacy `OutputContractResolver`, same-stage `Retry Failed Agent` storage truth is not actually used by artifact persistence, stage-level failed-evidence truth is rebuilt transiently instead of being durably persisted and consumed by reports/exports, `ProposalDraftCompactionPolicy` is not on a live runtime path, and the app-level / canonical motivating-class proofs from Sections `10.2` and `10.3` are absent.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | multiple acceptance criteria are still only modeled or unit-tested, not closed in live runtime paths | High |
| Architecture | At Risk | V1/V2 contract authority split persists in runtime-critical readers | High |
| Product | At Risk | motivating failure class still has no canonical same-run retry regression proof | High |
| UI | At Risk | `BlockedRunRecoveryView` does not actually surface Proposal 013 evidence-panel behavior | High |
| UX | At Risk | report/export surfaces still summarize failures without canonical packet-backed provenance | High |
| Readiness | Not Ready | app-level proof and motivating-class regression proof are both missing | High |

## Proposal Contract

### Scope

- Align output-contract truth across catalog, runtime validation, persistence, reporting, and recovery.
- Make retry lineage and same-run retry semantics explicit and durable.
- Preserve failed-stage evidence even when validation fails after output generation.
- Extend blocked-run recovery owners with narrow retry truth and evidence explanation.
- Audit mandatory declarative YAML surfaces and harden them into runtime truth or fail-closed validation.

### Locked Decisions

- Proposal 013 does not create a second contract authority; catalog-backed truth remains primary.
- `Retry Failed Agent`, `Retry Failed Stage`, and `Clone Run` are distinct persisted actions.
- Failed-stage evidence must survive validation failure and remain inspectable.
- `contracts.*` and `backend_profiles.*.structured_output` are the mandatory Tier 1 declarative surfaces for this slice.
- Proposal output compaction is bounded resilience, not a new content-generation feature.

### Primary Runtime / Operator Flows

- Structured output generation and validation for proposal-review stages.
- Same-run recovery after failed validation with narrow retry before clone-run.
- Report and export inspection of the failed-stage evidence and retry lineage.
- Preflight enforcement of `structured_output` transport support.

## Proposal Fidelity / Divergence Inventory

### Matches

- `OutputContractSchemaV2`, `OutputContractResolverV2`, `ValidationFailureRecord`, `StructuredOutputSchemaGate`, `StageRetryCoordinator`, `FailedStageEvidenceBuilder`, `BlockedStageReportBuilder`, `FailedStageEvidencePanel`, `OutputContractDeclarativeBridge`, and `DeclarativeCoverageReport` all exist.
- The runtime now persists raw outputs before validation failure handling and records agent-level `outputEnvelopesJSON` and `validationFailureJSON`.
- `RecoverySheet` builds and displays a failed-stage evidence packet on the current tree.
- Dedicated Proposal 013 unit tests pass on the current tree.

### Divergences

- Runtime-critical readers still use legacy `OutputContractResolver` in `ArtifactManager`, `WorkflowOrchestrator.validateStructuredOutputs(...)`, `GooseAgentExecutor`, `GooseSessionBridge`, and `SimulatedAgentExecutor`.
- Same-stage agent-retry storage truth is specified in code comments and helper functions, but artifact persistence still writes to the stage-attempt path using hardcoded `attemptNumber: 1`.
- Stage-level `validationFailureJSON` and `evidencePacketJSON` fields exist in the model but are not written anywhere on the current tree.
- `BlockedStageReportBuilder` and `ProposalDraftCompactionPolicy` are still test-only on the current tree.

### Ambiguities / Evidence Gaps

- The dedicated suite proves unit behavior but not the app-launched same-run retry path required by `10.2` and `10.3`.
- No current-tree evidence shows a declarative-coverage snapshot being exported or logged from a real run.
- `BlockedRunRecoveryView` declares Proposal 013 evidence state, but no current code path populates or presents it.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 1 |
| Partially Implemented | 5 |
| Missing | 6 |
| Not Verifiable | 0 |

## Track 1: Objective Proposal-Conformance Audit

### REQ-001 Proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
- Proposal Source: `4.2`, `4.3`, `4.4`, acceptance criterion `1`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `examples/agents/agents.yaml`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The typed schema and review-contract adapter exist, but runtime truth is still split. `ArtifactManager.persistOutputs(...)` and `WorkflowOrchestrator.validateStructuredOutputs(...)` still read contract truth through legacy `OutputContractResolver`, and `ProposalReviewContractAdapter` is not used outside tests/code definitions.

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
- Gap / Note: Raw outputs plus receipt/transcript artifacts are preserved as outputs, and validation failures are persisted as first-class records. The broader stage-level packet/report/export wiring remains incomplete and is captured separately in `REQ-007`.

### REQ-003 Retry-in-place preserves attempt truth and stage lineage
- Proposal Source: `5.2`, `5.3`, acceptance criterion `3`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/BlockedStageReportBuilder.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The retry coordinator models stage and agent lineage correctly, but the live artifact/report paths do not yet consume that truth consistently. Current runtime persistence still hardcodes stage attempt `1` in multiple orchestration paths.

### REQ-004 Same-stage `Retry Failed Agent` has explicit artifact / receipt / transcript storage truth with no collisions
- Proposal Source: `5.4`, acceptance criterion `4`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
- Gap / Note: The required lineage fields and `agentRetryNamespace(...)` helper exist, but they are not used by the persistence path. `ArtifactManager.persistOutputs(...)` still writes only `{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}`, and `WorkflowOrchestrator` passes `attemptNumber: 1` in current execution paths. `Artifact.agentAttemptNumber`, `supersedesAgentArtifactID`, and `artifactLineageKind` are declared but not written.

### REQ-005 Blocked-run recovery surfaces expose the narrowest valid retry action before clone-run
- Proposal Source: `7.2`, acceptance criterion `5`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: `RecoverySheet` is materially extended and uses Proposal 013 recovery policy. `BlockedRunRecoveryView` still stops short of the promised evidence-aware extension: it declares `evidencePacket` / `showEvidencePanel` state but never populates or presents it.

### REQ-006 A canonical regression proves failed review stages can be retried and completed without cloning the run
- Proposal Source: `10.3`, acceptance criterion `6`
- Status: Missing
- Evidence Type: tests-run, code-search
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - no current-tree hits for the motivating run ID or an equivalent same-run retry regression in `Chainworks ForgeTests` / `Chainworks ForgeUITests`
- Gap / Note: The current suite is unit-only. No canonical regression currently proves: draft succeeds, review fails on contract mismatch, failure evidence survives, narrow retry is offered, same-run retry completes, and prior failed-attempt evidence remains inspectable.

### REQ-007 Recovery, reporting, and export surfaces reference the canonical `ValidationFailureRecord` or failed-stage evidence packet
- Proposal Source: `6.3`, `7.3`, acceptance criterion `7`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
- Gap / Note: RecoverySheet can build a transient evidence packet from agent JSON fields, but stage-level `validationFailureJSON` / `evidencePacketJSON` are not persisted, `BlockedStageReportBuilder` is unused outside tests, `RunReportBuilder` does not read the canonical failure record or evidence packet, and export surfaces remain Proposal 008-oriented.

### REQ-008 Proposal drafting oversized-output failures are bounded by explicit compaction policy and evidence
- Proposal Source: `8.2`, acceptance criterion `8`
- Status: Missing
- Evidence Type: code-search, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The compaction policy and metadata types exist and are unit-tested, but `ProposalDraftCompactionPolicy.apply(...)` is not used anywhere in live runtime code, and `compactionMetadataJSON` is never written on the current tree.

### REQ-009 Mandatory-tier YAML fields are enforced or fail-closed, and non-mandatory fields are explicitly tiered
- Proposal Source: `4.2.2`, `3 Layer Q`, acceptance criterion `9`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/StructuredOutputSchemaGate.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/OutputContractDeclarativeBridge.swift`
  - `Chainworks Forge/Engine/DeclarativeCoverageReport.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: `backend_profiles.*.structured_output` is enforced at preflight, but contract resolution is not yet fully catalog-driven in live runtime consumers because V1 readers remain on critical paths. Tier classification exists in code, but the live app does not yet surface or persist it as runtime evidence.

### REQ-010 Appendix B tiering is persisted and testable, and mandatory-tier rows have corresponding enforcement evidence
- Proposal Source: `10.1`, `10.2`, acceptance criterion `10`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/DeclarativeCoverageReport.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Appendix B tiering is testable in isolation, but `DeclarativeCoverageReport` is not currently emitted or persisted by live run/export flows, so the proposal’s persisted-evidence claim is not yet satisfied.

### REQ-011 Unit and integration proof from Section 10.1 is complete
- Proposal Source: `10.1`
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - local targeted suite passed `25/25`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-test.oP133P/p013-tests.xcresult`
- Gap / Note: The current suite proves many unit behaviors, including schema derivation, resolver behavior, schema gate behavior, failure-record building, evidence-packet building, compaction, and coverage-report roundtrips. It does not materially close integration-level proof for same-run retry lineage, canonical packet persistence, or report/export consumption.

### REQ-012 App-level proof and motivating-class regression proof from Sections 10.2 and 10.3 are complete
- Proposal Source: `10.2`, `10.3`
- Status: Missing
- Evidence Type: code-search
- Evidence:
  - no current app-launched proof artifact or proposal-owned UI/integration suite surfaced for Proposal 013 on the current tree
  - no canonical motivating-class regression surfaced in `Chainworks ForgeTests` / `Chainworks ForgeUITests`
- Gap / Note: The proposal explicitly requires an app-launched failure-then-same-run-retry proof plus a canonical motivating-class regression. Neither exists on the current tree.

## Track 2: Expert Findings

## Architecture Review

**Summary:** At Risk

### ARCH-001 V1 and V2 contract readers still coexist on runtime-critical paths
- Severity: Major
- Confidence: High
- Evidence:
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/GooseAgentExecutor.swift`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift`
  - `Chainworks Forge/Engine/SimulatedAgentExecutor.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
- Why It Matters: Proposal 013 explicitly says `OutputContractResolverV2` becomes the runtime reader for orchestrator, artifact manager, reports, and recovery. The current tree still routes critical execution and persistence logic through the legacy resolver, which means the proposal has not actually collapsed contract truth into one authoritative runtime path.

### ARCH-002 Agent-retry storage truth is designed but not executed
- Severity: Major
- Confidence: High
- Evidence:
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Models/Artifact.swift`
- Why It Matters: The proposal’s core promise is not merely that agent-retry lineage fields exist. It promises disjoint storage truth and immutable supersession semantics. That is not yet true in the live persistence path.

## Product Review

**Summary:** At Risk

### PROD-001 The motivating failure class still lacks canonical same-run proof
- Severity: Major
- Confidence: High
- Evidence:
  - `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md:595`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Why It Matters: Proposal 013 is grounded in one concrete blocked-run class. Without the required motivating-class regression and app-level proof, the repo still cannot demonstrate that the product problem which justified the proposal is closed end to end.

## UI Review

**Summary:** At Risk

### UI-001 `BlockedRunRecoveryView` overclaims Proposal 013 completion
- Severity: Major
- Confidence: High
- Evidence:
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
- Why It Matters: `RecoverySheet` is materially extended with failed-stage evidence, but `BlockedRunRecoveryView` still does not populate or present the declared evidence panel state. The proposal promises both shell-owned owners, not one.

## UX Review

**Summary:** At Risk

### UX-001 Report and export surfaces still do not explain validation failure through canonical packet-backed truth
- Severity: Major
- Confidence: High
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/CompletedRunExportHub.swift`
  - `Chainworks Forge/Engine/ValidationFailureRecord.swift`
- Why It Matters: The operator can inspect failure evidence in recovery, but report/export flows still present Proposal 008-era summary views without the canonical failed-stage packet. That leaves one of Proposal 013’s key trust goals unresolved.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Local proof is green, but proposal-level proof is still below sign-off
- Severity: Major
- Confidence: High
- Evidence:
  - local build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-build.Cd4LOT/p013-build.xcresult`
  - local targeted Proposal 013 suite passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-test.oP133P/p013-tests.xcresult`
- Why It Matters: The repo is healthy enough for continued implementation, but the proposal’s own closure bar is higher than a green unit slice. Runtime integration, app-level proof, and canonical regression proof are still open.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Local macOS build succeeds | Pass | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-build.Cd4LOT/p013-build.xcresult` |
| Dedicated Proposal 013 unit suite is green | Pass | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-test.oP133P/p013-tests.xcresult`, `25` Swift Testing tests passed |
| Runtime uses `OutputContractResolverV2` as the single contract reader | Fail | live runtime still uses legacy `OutputContractResolver` on critical paths |
| Same-stage agent retry uses disjoint storage truth | Fail | helper exists, live persistence path does not use it |
| Canonical failure packet is persisted and consumed by recovery, reports, and export | Fail | recovery can rebuild a transient packet, reports/exports do not consume a persisted canonical packet |
| Proposal-draft compaction is wired on a live runtime path | Fail | policy exists only in tests |
| App-level failure-then-same-run-retry proof exists | Fail | no current-tree proof surfaced |
| Motivating-class regression proof exists | Fail | no canonical regression surfaced |
| Declarative coverage snapshot is exported or logged by a real run | Fail | no live emission path surfaced |

## Verification Log

1. `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-build.Cd4LOT -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-build.Cd4LOT/p013-build.xcresult`
   - Result: `BUILD SUCCEEDED`
   - Bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-build.Cd4LOT/p013-build.xcresult`

2. `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-test.oP133P -resultBundlePath /var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-test.oP133P/p013-tests.xcresult -only-testing:'Chainworks ForgeTests/Proposal013Tests'`
   - Result: `TEST SUCCEEDED`
   - Note: XCTest summary printed `0` legacy tests, then Swift Testing ran `25` Proposal 013 tests and all passed.
   - Bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-audit-test.oP133P/p013-tests.xcresult`

3. Code inspection and search highlights:
   - `StageExecution.evidencePacketJSON` and `StageExecution.validationFailureJSON` are declared but not written on the current tree.
   - `BlockedStageReportBuilder.buildReport(...)` is only referenced in `Chainworks ForgeTests/Proposal013Tests.swift`.
   - `ProposalDraftCompactionPolicy.apply(...)` is only referenced in `Chainworks ForgeTests/Proposal013Tests.swift`.
   - Runtime-critical readers still reference legacy `OutputContractResolver` in `ArtifactManager`, `WorkflowOrchestrator`, `GooseAgentExecutor`, `GooseSessionBridge`, and `SimulatedAgentExecutor`.

## Recommended Next Actions

1. Replace legacy `OutputContractResolver` reads on execution/persistence/reporting paths with `OutputContractResolverV2`, then rerun the dedicated Proposal 013 suite.
2. Wire `ArtifactManager` / `WorkflowOrchestrator` to honor `stage.attemptNumber`, `agentAttemptNumber`, `artifactLineageKind`, and `agentRetryNamespace(...)` for same-stage retry.
3. Persist stage-level `ValidationFailureRecord` / `FailedStageEvidencePacket` truth into `StageExecution.validationFailureJSON` / `evidencePacketJSON`, and make `RunReportBuilder` plus export/report surfaces consume that canonical object.
4. Put `ProposalDraftCompactionPolicy` on the real proposal-drafting runtime path and persist `compactionMetadataJSON` when invoked.
5. Add one app-launched same-run retry proof plus one canonical motivating-class regression that exercises: raw outputs preserved, validation fails, narrow retry offered, same-run retry succeeds, prior failed-attempt evidence remains inspectable, and declarative-coverage evidence is emitted.
