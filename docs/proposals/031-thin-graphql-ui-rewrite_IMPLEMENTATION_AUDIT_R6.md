# Proposal 031 Implementation Audit R6

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/031-thin-graphql-ui-rewrite.md` |
| Proposal title | Proposal 031: Thin GraphQL-Only UI Rewrite Over Server Projections |
| Proposal revision under audit | `031-2026-04-24-r19-degraded-state-correction` |
| Audit report | `docs/proposals/031-thin-graphql-ui-rewrite_IMPLEMENTATION_AUDIT_R6.md` |
| Audit timestamp | `2026-04-25T09:39:25Z` |
| Repository HEAD | `9c59df8045512fae6e5c26f0ca45cc4ef616f8ee` |
| Proposal checksum | `md5:f2b70768bf8f784db79f86f93d5ea50e` |
| Audit mode | Implementation audit, repository evidence and focused validation |
| Overall conformance | Partial |
| Overall implementation readiness | Ready for GraphQL-only read-boundary stop-state handoff; not ready for Phase 3 / release closeout |
| Audit confidence | High for read-boundary/code-gate conformance; medium-high for runtime UI evidence because no live UI smoke was run in this audit |

## Implementation Target

P031 r19 is no longer a full visual/product rewrite proposal. The governing target is the stopped GraphQL-only read-boundary stabilization state:

- SwiftUI governed workflow surfaces render server-owned GraphQL read models.
- SwiftUI keeps only presentation, server-derived, read-refresh, and freshness state.
- P031 explicitly excludes MCP reads/writes, GraphQL mutations, local mutation fallback, command payloads, receipts, correlation IDs, and write-path implementation.
- P032 owns stabilization, release, dogfood, degraded-state, freshness, and docs closeout after this stop-state.
- P036 owns visual/navigation restoration.
- Future write proposals own create/start/cancel/retry/approve operator commands.

The audit therefore scores implementation against the GraphQL-only stop-state contract, and separately records release/readiness blockers that remain outside the stopped P031 vehicle.

## Prior Review Reuse

Reuse state: Partially reused.

No current adjacent `031-thin-graphql-ui-rewrite.review/` artifacts were found for this renamed r19 proposal. The audit reused the prior review context from `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/` and its evidence pack, then applied a delta for the r19 scope change from GraphQL+MCP to GraphQL-only.

Selected reviewer perspectives retained:

- `apple_ux_reviewer`
- `apple_arch_reviewer`
- `api_contract_reviewer`
- `observability_rollout_reviewer`
- `chainworks_execution_truth_reviewer`

Rejected reviewer perspectives remained unchanged:

- `macos_ui_reviewer`
- `rust_arch_reviewer`
- `rust_reliability_reviewer`
- `rust_security_reviewer`
- `product_reviewer`
- Go reviewers

Delta from prior review: MCP action/readwrite scrutiny is no longer a P031 implementation target. The relevant question is now whether all governed UI behavior is bounded to GraphQL reads and server projections, with release/dogfood gaps handed off rather than silently carried as P031 implementation scope.

## Current System Evidence

Primary implementation artifacts inspected:

- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- `Chainworks Forge/Views/RunsHomeView.swift`
- `Chainworks Forge/Views/DaemonLifecycleSurface.swift`
- `Chainworks Forge/Views/WorkflowInspectorView.swift`
- `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`
- `control-plane/graphql-server/src/schema.rs`
- `control-plane/graphql-server/src/types/p031.rs`
- `control-plane/graphql-server/src/types/run.rs`
- `control-plane/graphql-server/src/types/stage.rs`
- `control-plane/graphql-server/src/types/approval.rs`
- `control-plane/graphql-server/src/types/artifact.rs`
- `docs/reference/p031-thin-ui-inventory.json`
- `docs/reference/p031-operator-write-path-guide.json`
- `docs/reference/p031-schema-decision-record.json`
- `docs/reference/p031-phase-0-artifact-manifest.json`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`
- `docs/evidence/p031-dogfood-signoff.md`
- `docs/evidence/p031-degraded-state-evidence.md`
- `docs/evidence/p031-freshness-baseline.md`
- `docs/evidence/p031-ux-accessibility-signoff.md`
- `docs/evidence/p031-report-payload-priority-decision.md`

## Fidelity Summary

### Implemented As Proposed

- The active P031 proposal is explicit about the GraphQL-only stop-state and exclusion of UI write-path implementation.
- The canonical `proposal-031` gate exists and passed.
- The governed Swift files are represented by a machine-readable inventory and fail-closed static guard.
- Governed UI code routes through a P031 read-boundary support layer and GraphQL read models.
- GraphQL schema/types expose P031 projection fields for runs, stages, approvals, artifacts, report metadata, freshness, redaction/source metadata, and daemon lifecycle state.
- Targeted Swift P031 tests passed.
- GraphQL P031 tests and auth contract tests passed via the canonical gate.
- Approval rows are diagnostic/read-only and route write intent to external workflow guidance rather than implementing approval commands in P031.
- Full report payload rendering is not implemented, matching the P031 stop-state decision to leave full payload handling to a P0 follow-up unless evidence downgrades the decision.

### Implemented With Qualifications

- Phase 0 artifacts exist and are consumed by gates, but the artifact manifest entries do not include per-entry revision or commit identifiers even though the proposal requires path plus revision-or-commit where available.
- Degraded-state, freshness, UX/accessibility, and dogfood signoff evidence exists, but the evidence is not closeout-ready.
- The schema decision record is present and gate-accepted, but its embedded proposal revision predates the r19 wording. This is acceptable for current gate behavior but weak as long-term repository truth.

### Not In P031 Scope After r19 Stop-State

- Product/visual polish and navigation restoration are delegated to P036.
- Release/dogfood/stabilization closeout is delegated to P032.
- Create/start/cancel/retry/approve/write-path implementation is delegated to future write-path proposals.
- Local orchestrator fallback, MCP write/read behavior, GraphQL mutations, receipts, command payloads, and correlation plumbing are explicitly excluded.

## Requirement Conformance Matrix

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | r19 proposal checked in and supersedes stale broader P031 scope | Implemented | `031-thin-graphql-ui-rewrite.md` declares GraphQL-only stop-state and handoffs to P032/P036/future writes. |
| REQ-002 | P043/P031 reference reconciliation and P031 gate registration | Implemented | `docs/reference/test-gates.md`; `scripts/test-gate.sh proposal-031`. |
| REQ-003 | Governed UI uses GraphQL read plane only, with no MCP, mutations, local truth fallback, or command plumbing | Implemented | P031 inventory and static guard passed; governed source search only found forbidden tokens in rejection/parser guard contexts. |
| REQ-004 | GraphQL schema exposes required projection/read model fields, enums, source metadata, freshness, diagnostics, and redaction/defer information | Implemented | `control-plane/graphql-server/src/schema.rs`; `types/p031.rs`; `types/run.rs`; `types/stage.rs`; `types/approval.rs`; `types/artifact.rs`; P031 GraphQL tests passed. |
| REQ-005 | Machine-readable governed UI inventory exists and is used by fail-closed gate | Implemented | `docs/reference/p031-thin-ui-inventory.json`; `scripts/p031-thin-ui-gate.py` via `proposal-031`. |
| REQ-006 | Swift read-boundary client/presenter/test-double behavior exists with subscriptions/polling/targeted refresh semantics | Implemented | `P031ThinGraphQLReadBoundary.swift`; 53 focused Swift tests passed. |
| REQ-007 | Governed surfaces render server projections for runs, stages, approvals, artifacts, reports, and daemon lifecycle | Implemented for stop-state | `RunsHomeView.swift`, `DaemonLifecycleSurface.swift`, `WorkflowInspectorView.swift`, inventory governed file list. Visual richness is intentionally P036 scope. |
| REQ-008 | Approval behavior remains diagnostic/read-only and does not reintroduce approval write controls | Implemented | Approval view/read models; operator write-path guide; static guard. |
| REQ-009 | Report metadata/payload availability indicators exist while full payload rendering remains outside P031 | Implemented | Report metadata UI/model evidence; `docs/evidence/p031-report-payload-priority-decision.md`. |
| REQ-010 | Operator write-path guide documents removed write controls and validation status | Implemented for dogfood-start template | `docs/reference/p031-operator-write-path-guide.json`; rows include removed controls and external workflow guidance. Some rows intentionally remain `pending` or `temporarily_unavailable`. |
| REQ-011 | Phase 0 artifact manifest contains required artifacts and metadata | Partially implemented | `docs/reference/p031-phase-0-artifact-manifest.json` exists and is gate-consumed, but entries lack revision/commit identifiers required by proposal wording. |
| REQ-012 | Phase 0d degraded-state, freshness, UX/accessibility, and report payload evidence exists | Partially implemented | Evidence files exist; degraded drill/waiver, dogfood confirmation, and VoiceOver validation remain qualified or pending. |
| REQ-013 | Phase 3 dogfood/signoff/release closeout is complete | Out of P031 stop-state scope / not ready | r19 delegates this tail to P032; `proposal-031-readiness` fails on these closeout requirements. |
| REQ-014 | Visual/navigation/product polish complete | Out of P031 stop-state scope | r19 delegates this to P036. |
| REQ-015 | Canonical implementation gate passes | Implemented | `./scripts/test-gate.sh proposal-031` passed. |

Summary counts:

- Implemented: 10
- Implemented for stop-state: 1
- Partially implemented: 2
- Out of P031 stop-state scope: 2
- Missing: 0
- Not verifiable: 0

## Specialist Findings

### READY-001: P031 closeout/readiness remains blocked by incomplete evidence

Severity: Major

Perspective: `observability_rollout_reviewer`, `apple_ux_reviewer`

The GraphQL-only implementation gate passes, but the closeout/readiness gate does not. `./scripts/test-gate.sh proposal-031-readiness` failed because the manifest status remains `phase0d_runtime_evidence_attached_phase3_dogfood_signoff_pending`, degraded-state evidence is not release-closeout-ready, freshness evidence still needs dogfood confirmation, UX/accessibility signoff carries an Assistive Access / VoiceOver limitation, and dogfood signoff remains unsigned with unchecked checklist items. This is not a read-boundary conformance blocker after r19, but it blocks any claim that P031 itself is ready for Phase 3 or release closeout.

Recommendation: Keep P031 closed at the read-boundary stop-state and track the remaining dogfood/stabilization/release work under P032, with P032 owning the readiness gate or replacing it with a P032-specific closeout gate.

### OPS-001: Phase 0 artifact manifest is missing revision-or-commit identifiers

Severity: Minor

Perspective: `observability_rollout_reviewer`, `chainworks_execution_truth_reviewer`

The proposal requires each Phase 0 artifact manifest row to include `path`, revision or commit when available, owner, validation status, and blocking phase. The checked-in manifest contains paths, owner roles, validation statuses, and blocking phase/status metadata, but it does not carry a per-entry revision or commit identifier. The current gate accepts the manifest, so this is a documentation/control weakness rather than a code defect.

Recommendation: Add a `revision` or `commit` field to each manifest entry, or deliberately amend the proposal/gate contract if the repository wants manifest entries to be path/status-only.

### API-001: Schema/read-boundary contract is conformant for the stopped P031 scope

Severity: Informational

Perspective: `api_contract_reviewer`, `apple_arch_reviewer`

The inspected GraphQL schema/types and Swift read-boundary tests align with P031's stopped GraphQL-only contract. The canonical gate ran P043 dependencies, static P031 forbidden-path checks, GraphQL `proposal_031_` tests, and GraphQL auth tests successfully.

Recommendation: Do not expand P031 to absorb write-path or visual work. Preserve this contract boundary and land follow-on work in P032/P036/write-path proposals.

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `./scripts/test-gate.sh proposal-031` | Passed | Ran P043 gate, P031 static gate, GraphQL P031 tests, and GraphQL auth tests. |
| `./scripts/test-gate.sh proposal-031-readiness` | Failed | Expected closeout failure: dogfood/signoff, degraded-state, freshness, UX/accessibility, and manifest status remain qualified/pending. |
| `xcodebuild test -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:"Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests" -skip-testing:"Chainworks ForgeUITests"` | Passed | 53 focused P031 Swift tests passed. UI tests were not run, consistent with repository policy. |

## Scorecard

| Area | Score | Rationale |
| --- | --- | --- |
| Proposal scope alignment | Strong | r19 stop-state is reflected in code, gates, and docs. |
| Read-boundary architecture | Strong | Governed UI is routed through GraphQL projections and P031 presenters. |
| API/schema contract | Strong | GraphQL tests and schema evidence pass for P031 projection needs. |
| Static guard coverage | Strong | Inventory and forbidden-path gate pass. |
| Operator write-path handling | Adequate | Removed controls are documented and externalized; implementation remains intentionally absent. |
| Evidence/readiness | Weak for closeout | Required closeout artifacts remain qualified or incomplete. |
| Artifact manifest rigor | Adequate with gap | Manifest exists, but lacks revision/commit IDs required by proposal wording. |

## Final Verdict

P031 is implemented for its r19 GraphQL-only read-boundary stop-state. The governed SwiftUI surfaces, GraphQL schema/read models, static guard, inventory, operator write-path guide, and focused test coverage match the narrowed proposal intent. The canonical implementation gate passed, and the focused Swift test suite passed.

Overall conformance remains Partial because strict proposal evidence/readiness requirements are not complete: the Phase 0 manifest is missing revision-or-commit identifiers, and the release/readiness evidence still carries dogfood, degraded-state, freshness, and UX/accessibility qualifications. These blockers should not reopen P031 implementation scope unless the project intentionally reverses the r19 stop decision. They should be owned by P032/P036 or by explicit follow-up write/readiness proposals.

Implementation readiness:

- Ready to treat P031 as a completed GraphQL-only read-boundary stabilization handoff.
- Not ready to claim P031 Phase 3 dogfood/release closeout.
- Do not schedule additional P031 implementation work for visual polish or write-path behavior; route those through P036 and future write-path proposals.
