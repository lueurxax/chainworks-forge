# Proposal 013: Output Contract Alignment, Aggregate Contract Hardening, Failure Evidence, and Narrow Recovery Multi-Lens Audit R6

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `c1d9124` |
| Working Tree | `dirty (36 modified, 23 added/untracked)` |
| Audited At | `2026-03-30T21:30:29+0300` |
| Platform Scope | `macOS` |
| Proposal State | `Active` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P013` is materially stronger than in `R5`, but it is still not fully implemented as a proposal. The core contract/evidence/runtime slice is now in much better shape: local macOS `build` passed, and the focused `Proposal013Tests` suite passed `38/38`, including strict review-contract alignment, same-run retry-to-success on the motivating class, declarative coverage, and compaction proof. The remaining blocker is proposal-owned and explicit: Section `9.2` still does not have a canonical app-launched proof path. `UITestProposal013EvidenceSurface` exists, but it is not routed through the current app direct-surface boot path, not covered by a `Chainworks ForgeUITests` owner, not exposed by `scripts/test-gate.sh`, and even in isolation it seeds a blocked run instead of proving a real app-launched fan-out -> aggregate -> evidence -> narrow-recovery flow.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Section `9.2` app-level proof remains scaffold-only and non-canonical | High |
| Architecture | Acceptable | the runtime contract/evidence layer is now coherent, but the app-proof surface still lives outside the canonical boot/test lane | High |
| Product | At Risk | the operator-facing incident-closure story is still not executable from a shipped/tested app path | High |
| UI | Acceptable | failed-stage evidence UI and recovery explanation surfaces are present and supported by code/tests | Medium |
| UX | Acceptable | narrow recovery semantics are much clearer, but the promised app-level proof is still not reachable end-to-end | Medium |
| Readiness | Not Ready | one explicit proposal proof owner is still not executable as a canonical acceptance path | High |

## Proposal Contract

### Scope

- proposal-review output contract alignment
- `proposal_review_summary` aggregate contract hardening
- declarative contract-tier hardening for `contracts.*` and `backend_profiles.*.structured_output`
- canonical failure-evidence persistence for contract failures
- narrow recovery actions and truthful reporting/export references
- bounded proposal-output compaction / oversized-output resilience

### Locked Decisions

- `AgentCatalog.contracts` remains the canonical contract authority
- `OutputContractResolverV2` is the runtime reader for contract truth
- proposal-review outputs in this slice are strict JSON machine artifacts
- `proposal_review_summary` is a first-class contract, not an implicit transition artifact
- raw invalid artifacts remain evidence only and may not be treated as aggregate inputs
- recovery/report/export surfaces must point at canonical failure objects
- same-run narrow retry remains distinct from clone-run
- mandatory Appendix B tier-1 fields must be enforced or rejected

### Primary User Flows

1. Proposal-review fan-out emits contract-valid outputs and aggregate contract truth remains explicit.
2. If validation fails after output generation, the run blocks with preserved raw output, receipt/transcript evidence, and canonical failure records.
3. The operator opens recovery UI and sees the narrowest valid next action before clone-run.
4. The motivating failure class can be retried and completed without falling back to a full new run.
5. Declarative coverage and compaction truth remain inspectable in reports/exports.

### UI Commitments

- shell-owned recovery surfaces expose narrow retry, stage retry, and clone-run appropriately
- failed-stage evidence remains inspectable through the evidence panel
- at least one app-launched proof path demonstrates evidence preservation and narrow recovery

### UX Commitments

- operators can distinguish post-generation validation failure from generation failure
- retry guidance explains why narrow retry is valid
- contract mismatch defaults to operator-mediated recovery, not silent auto-retry

### Acceptance Criteria

- Section `10.1` items `1` through `5`
- Section `10.2` items `6` through `8`

### Test / Evidence Requirements

- Section `9.1` Phase A core proof
- Section `9.2` Phase A app-level proof
- Section `9.3` motivating-class regression proof
- Section `9.4` Phase B additional proof

### Explicit Exclusions

- no transport outcome normalization
- no stage settlement / resume idempotency repair
- no new provider-binding truth repair
- no second contract authority
- no clone-only recovery model

## Proposal Fidelity / Divergence

### Matches

- strict contract alignment is now materially landed on the current tree:
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift`
- canonical failure evidence is persisted and consumed through runtime/report/recovery paths:
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/ValidationFailureRecord.swift`
  - `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift`
- same-run retry truth, declarative coverage, and compaction are all covered by fresh focused proof:
  - `Chainworks ForgeTests/Proposal013Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-test2.x53uWC/p013-r6-tests.xcresult`
- local macOS build is green:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-build.JmQx0T/p013-r6-build.xcresult`

### Divergences

- Section `9.2` still lacks a canonical app-launched proof lane

### Ambiguities / Evidence Gaps

- `UITestProposal013EvidenceSurface` exists, but it is not reachable through the current direct-surface enum/switch in the app boot path
- `scripts/test-gate.sh` still has no `proposal-013` case; `Proposal013Tests` currently piggybacks only inside `PROPOSAL_016_TESTS`
- no `Chainworks ForgeUITests` owner drives a `P013` direct surface or validates the operator-facing proof story

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Proposal-review output contracts are aligned across catalog, runtime validation, and persisted artifacts
- Proposal Source: Section `4`, Section `10.1.1`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift:45`
  - `Chainworks Forge/Engine/OutputContractSchemaV2.swift:43`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift:205`
  - `Chainworks ForgeTests/Proposal013Tests.swift:530`
  - `Chainworks ForgeTests/Proposal013Tests.swift:550`
  - `Chainworks ForgeTests/Proposal013Tests.swift:567`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-test2.x53uWC/p013-r6-tests.xcresult`
- Gap / Note: The current tree now explicitly rejects markdown-only primary review artifacts and keeps proposal reviews strict structured.

### REQ-002 `proposal_review_summary` is a first-class contract with runtime validation and persisted artifact truth
- Proposal Source: Section `4.3`, Section `10.1.2`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift:29`
  - `Chainworks Forge/Engine/OutputContractDeclarativeBridge.swift:66`
  - `Chainworks Forge/Engine/GooseSessionBridge.swift:263`
  - `Chainworks ForgeTests/Proposal013Tests.swift:552`
  - `Chainworks ForgeTests/Proposal013Tests.swift:1014`
  - `Chainworks ForgeTests/Proposal013Tests.swift:1653`
- Gap / Note: Aggregate summary contract truth is no longer implicit; it is treated as a first-class contract participant in code and focused regression proof.

### REQ-003 Failed review or aggregate stages that produced outputs preserve receipts, transcripts or equivalent evidence, raw outputs, and validation error records
- Proposal Source: Section `6`, Section `10.1.3`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift`
  - `Chainworks Forge/Engine/ValidationFailureRecord.swift:8`
  - `Chainworks Forge/Engine/FailedStageEvidenceBuilder.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift:192`
  - `Chainworks ForgeTests/Proposal013Tests.swift:273`
  - `Chainworks ForgeTests/Proposal013Tests.swift:604`
- Gap / Note: Post-generation validation failure remains a first-class persisted evidence path.

### REQ-004 Blocked-run recovery surfaces expose the narrowest valid recovery action before clone-run
- Proposal Source: Section `7`, Section `10.1.4`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift:604`
  - `Chainworks ForgeTests/Proposal013Tests.swift:964`
- Gap / Note: Focused proof now explicitly asserts narrow same-run recovery remains available before clone-run.

### REQ-005 Reports, exports, and recovery surfaces reference the canonical `ValidationFailureRecord` or failed-stage evidence packet
- Proposal Source: Section `6.3`, Section `10.1.5`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift:344`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:355`
  - `Chainworks Forge/Views/FailedStageEvidencePanel.swift:7`
  - `Chainworks Forge/Views/RecoverySheet.swift:212`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift:107`
  - `Chainworks ForgeTests/Proposal013Tests.swift:964`
  - `Chainworks ForgeTests/Proposal013Tests.swift:1566`
- Gap / Note: Canonical failed-stage evidence is now consistently consumed by report and recovery code paths.

### REQ-006 Mandatory-tier YAML fields from Appendix B are either enforced by runtime code or rejected by validation / preflight
- Proposal Source: Section `4.5`, Section `10.2.6`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/StructuredOutputSchemaGate.swift`
  - `Chainworks Forge/Engine/OutputContractDeclarativeBridge.swift`
  - `Chainworks ForgeTests/Proposal013Tests.swift:436`
  - `Chainworks ForgeTests/Proposal013Tests.swift:468`
  - `Chainworks ForgeTests/Proposal013Tests.swift:486`
- Gap / Note: Tier-1 contract/runtime enforcement is explicitly covered by focused tests and no longer depends on hardcoded output-name fallback branches.

### REQ-007 Appendix B tiering is persisted and testable
- Proposal Source: Appendix `A`, Section `10.2.7`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/DeclarativeCoverageReport.swift:8`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:2027`
  - `Chainworks ForgeTests/Proposal013Tests.swift:427`
  - `Chainworks ForgeTests/Proposal013Tests.swift:450`
- Gap / Note: Declarative coverage remains a first-class persisted/tested artifact on the current tree.

### REQ-008 Proposal-drafting oversized-output failures are bounded by explicit compaction policy and evidence
- Proposal Source: Section `8`, Section `10.2.8`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Engine/ProposalDraftCompactionPolicy.swift:13`
  - `Chainworks Forge/Engine/ArtifactPersistenceOrderingPolicy.swift:41`
  - `Chainworks ForgeTests/Proposal013Tests.swift:362`
  - `Chainworks ForgeTests/Proposal013Tests.swift:372`
  - `Chainworks ForgeTests/Proposal013Tests.swift:384`
- Gap / Note: Compaction behavior and metadata are explicitly covered by focused proof.

### REQ-009 Phase A core proof is complete
- Proposal Source: Section `9.1`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift:530`
  - `Chainworks ForgeTests/Proposal013Tests.swift:552`
  - `Chainworks ForgeTests/Proposal013Tests.swift:192`
  - `Chainworks ForgeTests/Proposal013Tests.swift:273`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-test2.x53uWC/p013-r6-tests.xcresult`
- Gap / Note: The focused suite now cleanly covers reviewer contract validation, aggregate contract truth, and failed-stage evidence persistence.

### REQ-010 Phase A app-level proof is complete
- Proposal Source: Section `9.2`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:830`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:849`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:872`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:886`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:983`
  - `Chainworks Forge/ContentView.swift:37`
  - `Chainworks Forge/ContentView.swift:186`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:576`
  - `scripts/test-gate.sh:75`
  - `scripts/test-gate.sh:532`
- Gap / Note: A meaningful scaffold exists (`UITestProposal013EvidenceSurface`), but it is still not routed into the app’s direct-surface enum/switch, not exposed through a canonical `proposal-013` gate, not owned by a UI test, and it seeds a blocked run instead of proving a real app-launched run with fan-out, aggregate handling, preserved evidence, and narrow recovery.

### REQ-011 Phase A motivating-class regression proof is complete
- Proposal Source: Section `9.3`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift:604`
  - `Chainworks ForgeTests/Proposal013Tests.swift:786`
  - `Chainworks ForgeTests/Proposal013Tests.swift:1653`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-test2.x53uWC/p013-r6-tests.xcresult`
- Gap / Note: The current suite now explicitly covers the motivating-class failure, same-run retry, and truthful reporting path.

### REQ-012 Phase B additional proof is complete
- Proposal Source: Section `9.4`
- Status: Implemented
- Evidence Type: tests-run
- Evidence:
  - `Chainworks ForgeTests/Proposal013Tests.swift:41`
  - `Chainworks ForgeTests/Proposal013Tests.swift:436`
  - `Chainworks ForgeTests/Proposal013Tests.swift:427`
  - `Chainworks ForgeTests/Proposal013Tests.swift:362`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-test2.x53uWC/p013-r6-tests.xcresult`
- Gap / Note: Contract-resolution, schema-gate, declarative-coverage, and compaction proof are all now present in the focused suite.

## Architecture Review

**Summary:** Acceptable

No material architecture finding remains inside the core runtime slice. Resolver, adapter, failure-evidence persistence, and recovery/report consumers now form a coherent bounded contract/evidence layer.

## Product Review

**Summary:** At Risk

### PROD-001 Incident-closure proof still does not run from a canonical app path
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-010`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:830`
  - `Chainworks Forge/ContentView.swift:37`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:576`
  - `scripts/test-gate.sh:532`
- Why It Matters: The proposal explicitly promises an app-launched proof story. Right now the operator-facing proof still exists only as scaffolding, so the strongest acceptance path remains a focused unit suite rather than a shipped/tested app route.
- Recommended Action: Add a `proposal013Proof` direct-surface route, wire it through the app’s direct-surface boot path, add a `proposal-013` gate in `scripts/test-gate.sh`, and cover it with a dedicated UI proof owner.

## UI Review

**Summary:** Acceptable

No material UI finding blocks the bounded contract/evidence rollout. `FailedStageEvidencePanel`, `RecoverySheet`, and `BlockedRunRecoveryView` remain aligned with the proposal’s shell-owned UI boundary.

## UX Review

**Summary:** Acceptable

No material UX finding blocks the non-UI contract/evidence slice. The remaining risk is not that recovery UX is unclear, but that the proposal’s promised app-level proof is still not exercisable from a canonical app lane.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Section 9.2 app-level proof remains scaffold-only
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-010`
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift:830`
  - `Chainworks Forge/ContentView.swift:37`
  - `Chainworks Forge/ContentView.swift:186`
  - `Chainworks Forge/Chainworks_ForgeApp.swift:576`
  - `scripts/test-gate.sh:75`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-test2.x53uWC/p013-r6-tests.xcresult`
- Why It Matters: The current tree can prove the incident-closing runtime slice in focused tests, but it still cannot demonstrate the promised app-launched operator flow from the shipped/tested app path. That keeps handoff and sign-off risk materially open.
- Recommended Action: Promote the existing scaffold into a real proof lane: route `UITestProposal013EvidenceSurface` through the app direct-surface path, add a dedicated `proposal-013` gate, add a UI test owner, and extend the surface so it proves real app-launched run behavior rather than manually seeding a blocked run.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | local macOS build passed: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p013-r6-build.JmQx0T/p013-r6-build.xcresult` |
| Core user flow runtime-validated | Partial | motivating-class regression is green in focused tests, but Section `9.2` app-launched proof is still incomplete |
| Empty/loading/error states covered | Pass | failed-stage evidence and narrow recovery paths are explicitly exercised in focused tests |
| Accessibility risk acceptable | Not Checked | outside the explicit `P013` contract |
| Localization risk acceptable | Not Checked | outside the explicit `P013` contract |
| Critical tests executed | Pass | focused `Proposal013Tests` passed `38/38` |
| Privacy/permissions/entitlements reviewed | Not Checked | outside the explicit `P013` contract |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- `rg -n 'proposal-013|UITestProposal013EvidenceSurface|Proposal013Tests|structured_with_human_companion|agent-retry-' ...`
- `sed -n '388,432p' docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
- `xcodebuild build -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath .../p013-r6-build.xcresult`
- initial concurrent `xcodebuild test ... -only-testing:'Chainworks ForgeTests/Proposal013Tests'` produced a non-proving build-db lock failure
- sequential rerun with isolated `DerivedData`:
  - `xcodebuild test -project '/Users/user/Documents/Chainworks Forge/Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath ... -resultBundlePath .../p013-r6-tests.xcresult -only-testing:'Chainworks ForgeTests/Proposal013Tests'`

## Recommended Next Actions

1. Add a real `proposal013Proof` direct-surface route and canonical `proposal-013` gate so Section `9.2` is executable from the shipped app/test path.
2. Extend the current proof surface from seeded blocked-run scaffolding to a true app-launched fan-out -> aggregate -> evidence -> narrow-recovery flow.
3. Once the canonical lane exists, rerun `P013` audit; the non-UI/runtime side is now strong enough that this should be the last major blocker.
