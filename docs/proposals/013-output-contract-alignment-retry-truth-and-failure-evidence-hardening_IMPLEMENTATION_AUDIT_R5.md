# Proposal 013: Output Contract Alignment, Declarative Runtime Coverage, Retry Truth, and Failure Evidence Hardening Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `5c870b4` |
| Working Tree | `dirty (37 modified, 25 untracked)` |
| Audited At | `2026-03-29T16:51:32+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Implemented` |
| Overall Conformance | `Not Implemented` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`R5` is not a no-delta repeat. The current tree is stronger than `R4`: local macOS `build` passed, focused `Proposal013Tests` passed `29/29`, and the live runtime now carries more of the proposal contract on the real path. But Proposal 013 is still not `Implemented`. The blocking gap remains explicit and proposal-owned: Section `10.2` app-level proof is still missing in canonical form. `UITestProposal013EvidenceSurface` exists, but it is still orphaned from the app’s direct-surface boot path, not wired into `scripts/test-gate.sh`, not exercised by `Chainworks ForgeUITests`, and still does not prove the full required story of failure -> same-run retry -> success -> prior-attempt inspectability -> declarative-coverage export/logging.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Section `10.2` app-level proof is still absent as an executable acceptance path | High |
| Architecture | At Risk | proposal-review contract truth still accepts markdown as sufficient without proving the explicit machine + human companion pair | High |
| Product | At Risk | the proposal’s acceptance story still depends on a proof owner that is not actually reachable from the shipped app/test path | High |
| UI | Acceptable | failed-stage evidence and recovery surfaces exist and are materially clearer | Medium |
| UX | Acceptable | retry and clone semantics are now better explained, but the promised end-to-end operator proof is still not exercisable | Medium |
| Readiness | Not Ready | targeted non-UI proof is green, but one explicit proposal gate is still missing and unexecutable on the current tree | High |

## Proposal Contract

### Scope

- align output-contract truth across catalog, runtime validation, persistence, reporting, recovery, and export
- preserve failed-stage evidence even when validation fails after output generation
- make same-run retry lineage durable and operator-visible
- keep stage-attempt artifacts immutable while allowing agent-only retry under a disjoint namespace
- harden Appendix B Tier 1 YAML surfaces into executable runtime truth or fail-closed behavior
- bound oversized proposal outputs with auditable compaction metadata

### Locked Decisions

- `AgentCatalog.contracts` remains the canonical contract authority
- `OutputContractResolverV2` is the runtime reader for contract truth
- `Retry Failed Agent`, `Retry Failed Stage`, and clone-run remain distinct actions
- stage-attempt artifacts remain immutable; agent-only retry uses a disjoint namespace
- recovery/report/export surfaces must point to `ValidationFailureRecord` or the failed-stage evidence packet
- mandatory-tier YAML fields must be enforced or rejected

### Primary User Flows

1. A proposal-review or contract-bound stage produces outputs and validates them against catalog-backed truth.
2. If validation fails after generation, the run blocks with preserved raw outputs, receipts/transcripts, and canonical failure evidence.
3. The operator opens recovery UI, sees the narrowest valid retry action before clone-run, and understands what will be reused or re-executed.
4. A same-run retry succeeds without cloning the run, while earlier failed-attempt evidence remains inspectable.
5. Terminal run/report/export surfaces preserve declarative coverage and compaction truth for what the runtime actually enforced.

### UI Commitments

- `RecoverySheet` and `BlockedRunRecoveryView` expose narrow retry, stage retry, and clone-run actions when valid
- recovery UI links to canonical failed-stage evidence, not only summary prose
- an app-level proof exists for the Section `10.2` acceptance story

### UX Commitments

- the operator can distinguish generation failure from post-generation validation failure
- retry recommendations explain reuse, re-execution, and same-run versus clone-run semantics
- output-contract mismatch defaults to operator-mediated recovery, not silent auto-retry

### Acceptance Criteria

1. proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
2. failed stages preserve outputs and execution evidence
3. retry-in-place preserves truthful attempt numbering and lineage
4. same-stage `Retry Failed Agent` has explicit storage truth with no collisions
5. blocked-run recovery surfaces expose the narrowest valid retry action before clone-run
6. canonical regression proves failed review stages can be retried and completed without a new run
7. recovery/report/export surfaces reference canonical failed-stage evidence
8. proposal drafting compaction is explicit and auditable
9. mandatory-tier YAML fields are enforced or rejected
10. Appendix B tiering is persisted and testable
11. Section `10.1` unit/integration proof is complete
12. Section `10.2` app-level proof is complete

### Test / Evidence Requirements

- Section `10.1` focused unit/integration coverage
- Section `10.2` app-launched proof covering failure, preserved evidence, same-run retry success, prior-attempt inspectability, and declarative-coverage export/logging
- Section `10.3` canonical motivating-class regression

### Explicit Exclusions

- no new full clone-only recovery model
- no second independent contract authority
- no silent oversized-output truncation without persisted audit truth

## Proposal Fidelity / Divergence

### Matches

- `OutputContractResolverV2`, `ArtifactPersistenceOrderingPolicy`, failed-stage evidence persistence, recovery evidence UI, and compaction outcome truth are all present on the live path.
- Focused non-UI proof is real and green on the current dirty tree: local macOS `build` passed and focused `Proposal013Tests` passed `29/29`.
- Recovery surfaces now explain same-run versus clone-run, reuse, re-execution, and canonical retry recommendation sources much more clearly than earlier rounds.

### Divergences

- proposal-review companion truth still allows markdown to count as a passing review output under `structured_with_human_companion` without proving an explicit persisted machine + human pair
- the proposal-owned app proof exists only as an orphan direct surface, not as a canonical shipped/tested proof path
- same-stage agent-only retry storage truth is materially improved but still not fully closed by dedicated app-level or focused storage-proof evidence

### Ambiguities / Evidence Gaps

- no `proposal-013` case exists in `scripts/test-gate.sh`, so the proposal’s own acceptance path is still not a canonical gate
- no `P013` UI proof test exists in `Chainworks ForgeUITests`
- `UITestProposal013EvidenceSurface` still stops at retry availability and evidence presence; it does not exercise retry-to-success or declarative-coverage export/logging

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 4 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
- Proposal Source: `4.3`, `4.4`, acceptance criterion `1`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - focused suite passed `29/29`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-test.hrXc4c/p013-r5-tests.xcresult`
- Gap / Note: Catalog-backed resolver truth is now live. The requirement remains partial because `ProposalReviewContractAdapter` still accepts standalone markdown as a passing review output under `structured_with_human_companion`, and `resolveReviewSchema(...)` still does not pass `outputName`, so the schema’s explicit raw/normalized artifact pair is not materialized on the review path.

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
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-test.hrXc4c/p013-r5-tests.xcresult`
- Gap / Note: Raw outputs still persist before validation, validation failure is first-class persisted evidence, and the stage-level evidence packet is preserved.

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
- Gap / Note: Stage lineage and same-run retry-to-success are materially present. The requirement remains partial because focused proof still centers on stage-level retry and collapsed lineage, not a dedicated proof that agent-only retry preserves distinct agent-attempt truth without blurring stage-attempt numbering.

### REQ-004 Same-stage `Retry Failed Agent` has explicit artifact / receipt / transcript storage truth with no collisions
- Proposal Source: `5.4`, acceptance criterion `4`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Models/Artifact.swift`
- Gap / Note: The disjoint `agent-retry-{agentAttemptNumber}` namespace is live, `supersedesAgentArtifactID` is artifact-based, and `reused_sibling_reference` is written. The requirement remains partial because there is still no dedicated proof showing agent-only retry receipts/transcripts/effective-output precedence end-to-end without collision.

### REQ-005 Blocked-run recovery surfaces expose the narrowest valid retry action before clone-run
- Proposal Source: `7.2`, acceptance criterion `5`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Gap / Note: Narrow retry remains first-class and explanation text is materially clearer than earlier rounds.

### REQ-006 A canonical regression proves failed review stages can be retried and completed without creating a new run
- Proposal Source: `10.3`, acceptance criterion `6`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-test.hrXc4c/p013-r5-tests.xcresult`
- Gap / Note: The focused suite still contains the canonical motivating-class regression and it passed on the current tree.

### REQ-007 Recovery, reporting, and export surfaces reference the canonical `ValidationFailureRecord` or failed-stage evidence packet
- Proposal Source: `6.3`, `7.3`, acceptance criterion `7`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/EvidencePackBuilder.swift`
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Recovery, reporting, and export continue to read canonical failed-stage evidence instead of relying only on summary prose.

### REQ-008 Proposal drafting oversized-output failures are bounded by explicit compaction policy and evidence
- Proposal Source: `8.2`, acceptance criterion `8`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Compaction metadata now carries explicit outcome truth, and the focused suite covers compaction triggering and metadata round-trip.

### REQ-009 Mandatory-tier YAML fields are enforced or rejected by runtime/preflight
- Proposal Source: `4.5`, `10.1`, acceptance criterion `9`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/StructuredOutputSchemaGate.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Mandatory-tier enforcement remains live and focused proof is green.

### REQ-010 Appendix B tiering is persisted and testable
- Proposal Source: `10.1`, acceptance criterion `10`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/DeclarativeCoverageReport.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: Declarative coverage still persists tier classifications and focused proof remains green.

### REQ-011 Section 10.1 unit and integration proof is complete
- Proposal Source: `10.1`, acceptance criterion `11`
- Status: Partially Implemented
- Evidence Type: tests-run
- Evidence:
  - focused suite passed `29/29`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-test.hrXc4c/p013-r5-tests.xcresult`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Gap / Note: The suite is stronger than `R4` and now covers more regression/report truth. It remains partial because it still does not explicitly prove every `10.1` bullet, especially agent-only retry attempt persistence and clone-run versus retry lineage as separate acceptance items.

### REQ-012 Section 10.2 app-level proof is complete
- Proposal Source: `10.2`, acceptance criterion `12`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `scripts/test-gate.sh`
- Gap / Note: The current tree still lacks a canonical executable app-level proof. `UITestProposal013EvidenceSurface` exists, but there is no `P013` case in the app’s direct-surface routing, no `proposal-013` gate in `test-gate.sh`, no UI test that drives it, and the direct surface itself still does not prove retry-to-success, post-success prior-attempt inspectability, or declarative-coverage export/logging.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Proposal-review companion truth still over-accepts markdown
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `4.3`, `4.4`, `REQ-001`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
- Why It Matters: Proposal 013 promised a truthful contract model for proposal reviews. The current runtime still treats markdown-only review output as a passing result instead of proving a persisted machine artifact plus human companion. That leaves the runtime more permissive than the proposal’s durable artifact contract.
- Recommended Action: Either persist the explicit machine + human companion pair on the live review path or narrow the proposal/runtime contract so one persisted artifact is the truthful canonical output.

## Product Review

**Summary:** At Risk

### PROD-001 The proposal-owned acceptance path is still not a shipped path
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `10.2`, `REQ-012`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/ContentView.swift`
  - `scripts/test-gate.sh`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
- Why It Matters: The proposal’s own acceptance story depends on an app-launched proof, but the current repo still has no canonical way to launch or gate that proof. That means the remaining blocker is not “more confidence would be nice”; it is “the promised product proof path is still not actually part of the product/test surface.”
- Recommended Action: Wire a dedicated `P013` direct surface into app routing, add a canonical `test-gate` entry, and add one UI proof owner that exercises the exact `10.2` story.

## UI Review

**Summary:** Acceptable

### UI-001 Recovery evidence surfaces are materially present and understandable
- Severity: Note
- Confidence: Medium
- Related Proposal Items / Requirements: `7.2`, `7.3`, `REQ-005`, `REQ-007`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift`
- Why It Matters: This is one of the strongest areas of the current implementation. The operator can now see meaningful failure evidence and clearer recovery semantics instead of only opaque blockage.
- Recommended Action: Keep these owners as the canonical shell surfaces and avoid adding parallel proof-only UI that bypasses them.

## UX Review

**Summary:** Acceptable

### UX-001 Recovery explanation improved, but the full promised proof story still stops too early
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `7`, `9`, `10.2`, `REQ-012`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
- Why It Matters: The operator-facing explanation is much better, but the acceptance story still fails to show the full confidence loop of retry success plus preserved inspectability after success. That keeps the proposal from its stated “trustworthy recovery” endpoint.
- Recommended Action: Make the proof owner exercise the entire operator journey, not only pre-retry inspection.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Current-head non-UI proof is green, but the explicit app-level acceptance gate is still absent
- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: `10.1`, `10.2`, `REQ-011`, `REQ-012`
- Evidence Type: tests-run, code
- Evidence:
  - local build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-build.qWa17X/p013-r5-build.xcresult`
  - focused suite passed `29/29`: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-test.hrXc4c/p013-r5-tests.xcresult`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `scripts/test-gate.sh`
- Why It Matters: The current tree is healthy enough to prove non-UI behavior, but the proposal explicitly requires app-level proof. Until that path exists and passes on the current tree, readiness is still blocked by contract, not by taste.
- Recommended Action: Add a first-class app/UI gate for Section `10.2` and make it the canonical proving path for proposal closure.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-build.qWa17X/p013-r5-build.xcresult` |
| Core user flow runtime-validated | Partial | canonical non-UI regression is green, but the explicit app-level proof in `10.2` is still missing |
| Empty/loading/error states covered | Partial | recovery/failure evidence states exist, but proposal-owned app proof is not wired |
| Accessibility risk acceptable | Partial | no dedicated accessibility proof in this audit |
| Localization risk acceptable | Not Checked | not in scope of this focused proposal audit |
| Critical tests executed | Pass | focused `Proposal013Tests` passed `29/29` |
| Privacy/permissions/entitlements reviewed | Not Checked | not a primary contract surface for `P013` |

## Verification Log

- `python3 '/Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py' '/Users/user/Documents/Chainworks Forge/docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md'`
- `git -C '/Users/user/Documents/Chainworks Forge' rev-parse --short HEAD`
- `git -C '/Users/user/Documents/Chainworks Forge' status --short`
- `rg -n "REQ-012|10\\.2|UITestProposal013EvidenceSurface|proposal-013|ui-smoke|Retry Failed Agent|reused_sibling_reference|stageOutcome|ProposalReviewContractAdapter" ...`
- `xcodebuild build -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath ... -resultBundlePath '/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-build.qWa17X/p013-r5-build.xcresult'`
- `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath ... -resultBundlePath '/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r5-test.hrXc4c/p013-r5-tests.xcresult' -only-testing:'Chainworks ForgeTests/Proposal013Tests'`

## Recommended Next Actions

1. Wire a canonical `P013` app-proof path into `ContentView` / app boot, then add a dedicated `proposal-013` gate and UI owner.
2. Make the `10.2` proof exercise the full required story: failure, preserved evidence, same-run retry success, post-success prior-attempt inspectability, and declarative-coverage export/logging.
3. Tighten proposal-review companion truth so runtime persistence matches the proposal’s explicit machine + human artifact contract.
4. Add dedicated proof for same-stage agent-only retry receipt/transcript/effective-output precedence to close `REQ-004` and finish `10.1`.
