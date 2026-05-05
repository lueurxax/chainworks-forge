# Proposal 031 Implementation Audit R8: Thin GraphQL-Only UI Rewrite

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/031-thin-graphql-ui-rewrite.md` |
| Proposal revision | `031-2026-04-24-r19-degraded-state-correction` |
| Proposal state | Active stopped-state proposal, partially superseded by the implemented P072 UI action boundary |
| Audit mode | `auto` / implementation audit |
| Audit timestamp | 2026-05-05 11:37:33 EEST |
| Implementation target | Current worktree |
| Current HEAD | `3364a26554a455cb733095868faa7500a0267773` |
| Compare base | Implicit current branch/worktree |
| Worktree status before report | Clean |
| Report path | `docs/proposals/031-thin-graphql-ui-rewrite_IMPLEMENTATION_AUDIT_R8.md` |
| Overall Conformance | Implemented for the P031 stopped-state cutover |
| Overall Implementation Readiness | Not Ready for release closeout |
| Reviewer Selection Reuse | Partially reused |
| Audit Confidence | High for gate, schema, static guard, and code evidence; Medium for live UI behavior |

## Implementation Target / Compare Base

This audit evaluates the current worktree at HEAD `3364a26554a455cb733095868faa7500a0267773`. Unlike R7, the tree was clean before this report was written, and the P041 same-tree provenance blocker is no longer present.

No implementation files were modified by this audit. This report is the single generated artifact.

## Prior Proposal-Review Reuse

Reviewer selection reuse: **Partially reused**.

The active renamed proposal path has no adjacent `.review/` directory. The direct predecessor review under `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/` still applies to the macOS operator UI, Swift architecture, GraphQL/API contract, rollout/readiness, and execution-truth concerns. Reuse remains partial because the predecessor review covered a broader GraphQL+MCP command framing, while r19 narrows P031 to GraphQL-only reads plus the P072 approval-only GraphQL mutation exception.

Selected reviewers:

| Reviewer | Reason |
| --- | --- |
| `apple_ux_reviewer` | P031 changes operator read flows, degraded/freshness states, diagnostics, and accessibility/readiness evidence. |
| `apple_arch_reviewer` | P031 owns Swift read-model, coordinator, transport, subscription, presenter, and write-boundary separation. |
| `api_contract_reviewer` | GraphQL fields, enums, subscriptions, payload availability, redaction, and authorization are central. |
| `observability_rollout_reviewer` | Manifest, evidence, gates, dogfood, signoff, hold criteria, and release readiness are explicit proposal concerns. |
| `chainworks_execution_truth_reviewer` | P031 depends on server projection truth, P041 parity evidence, and no restored local workflow truth. |

Rejected close alternatives:

| Reviewer | Rejection reason |
| --- | --- |
| `macos_ui_reviewer` | Visual parity and navigation polish are explicitly handed to P036; no fresh UI runtime/screenshot review was obtained. |
| `rust_arch_reviewer` | Rust surfaces are schema/API checks, not new crate/module/worker architecture. |
| `rust_reliability_reviewer` | Retry/resume/cancel lifecycle behavior is outside P031 and belongs to later write-path proposals. |
| `rust_security_reviewer` | Authorization is exercised by P031/P072 tests; no new auth mechanism or public security boundary was added. |
| `product_reviewer` | Product polish and dogfood decision completion are deferred to P032/P036; readiness findings cover the current blocker. |
| Go reviewers | No real Go implementation surface or `go.mod` exists. |

## Prior Metrics Preserved

Leading metric from predecessor review:

- Percentage of P031-owned screens whose visible state is sourced only from named GraphQL read models/projections.

Guardrail metric from predecessor review:

- Zero P031-owned operator mutations bypass MCP/CommandHandler/audit unless explicitly deferred and disabled in the UI.

Updated interpretation under r19:

- P031 now has zero UI MCP usage and zero non-approval GraphQL mutations; only `approveApproval` and `rejectApproval` are allowed through the P072 boundary.

Decision checkpoint from predecessor review:

- Do not start implementation until P031 has a read-model matrix, action/deferral matrix, Swift cutover inventory, and canonical gate bundle.

Current audit result:

- The stopped-state implementation now has the read contracts, inventory, write-path guide, static gate, schema evidence, and canonical `proposal-031` gate. Phase 3 release/dogfood readiness is still not complete.

## Proposal State and Contract Summary

P031 r19 is a stopped-state cutover proposal. It lands the durable GraphQL-only read boundary and thin read surfaces, while handing visual/product polish, dogfood release evidence, degraded drills/waivers, freshness confirmation, and documentation cleanup to P032/P036. It also recognizes that P072 supersedes the original all-mutation ban with one narrow exception: governed SwiftUI may use GraphQL mutations only for `approveApproval` and `rejectApproval`.

Locked decisions:

- Governed macOS UI reads workflow truth from GraphQL projections only.
- Governed UI has no MCP reads/writes, no local workflow mutation fallback, no command payloads, no receipts, and no command correlation.
- Non-approval GraphQL mutations are prohibited from governed UI code.
- Approval rows may use only the P072 `approveApproval` / `rejectApproval` GraphQL exception.
- Removed write controls remain hidden, unavailable, diagnostic-only, or external-follow-up guided.
- Reports list metadata and payload availability are in scope; full report payload rendering remains follow-up unless server-owned GraphQL payload support lands.
- Degraded state is a read-only UI state over control-plane truth, not a restored local orchestrator.

## Platform / Product Scope

Apple scope: **macOS**.

Backend/service scope: **cross-stack API/data/rollout** across SwiftUI read models, Rust GraphQL schema/types/tests, static gates, reference artifacts, and release evidence.

## Primary Implementation Flows

1. Operator opens Runs Home and reads run/stage projection truth from GraphQL with freshness and projection-lag state.
2. Operator inspects run detail, stages, artifacts, report metadata, and daemon lifecycle without local workflow truth or raw artifact probing.
3. Operator views approval rows, sees server-derived actionability/diagnostics, and can settle only actionable approvals through the P072 GraphQL exception.
4. Operator refreshes visible read surfaces through targeted GraphQL reads/subscriptions without invoking MCP, local recovery, or non-approval mutations.
5. Release owner validates the stopped-state contract through `proposal-031`, then separately evaluates dogfood/signoff readiness through `proposal-031-readiness`.

## Proposal Fidelity / Divergence Inventory

### Matches

- `./scripts/test-gate.sh proposal-031` passes on the audited clean tree.
- `python3 scripts/p031-thin-ui-gate.py --repo-root .` passes the P031 inventory/static guard/write-path guide checks.
- `./scripts/test-gate.sh proposal-072` passes, including the P031 gate, 61 targeted macOS P031 boundary tests, and approval-only mutation policy checks.
- `docs/reference/p031-thin-ui-inventory.json` lists governed Swift files, embedded GraphQL operations, allowed approval mutation operations, exclusions, and allowed static guard matches.
- `docs/reference/p031-operator-write-path-guide.json` covers removed UI write controls and records validated external workflows for approvals and stage retry.
- `docs/reference/p031-phase-0-artifact-manifest.json` exists and links the required stopped-state artifacts.
- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift` enforces query/subscription reads and rejects forbidden operation names/fields while allowing the two P072 approval mutations.
- `Chainworks Forge/Views/RunsHomeView.swift` uses the P031 GraphQL read dashboard model and routes approval settlement through `P072ApprovalMutationClient`.
- Rust GraphQL schema/types expose the required freshness, write-path, disabled-reason, payload availability, diagnostic, projection, and authorization behaviors.

### Divergences

- `./scripts/test-gate.sh proposal-031-readiness` fails because Phase 0d/Phase 3 release evidence remains pending or qualified.
- The phase 0 manifest status is `phase0d_runtime_evidence_attached_phase3_dogfood_signoff_pending`, which is intentionally not closeout-ready.
- The dogfood signoff remains a template with unchecked items and is not signed/complete.
- UX/accessibility evidence still carries an assistive-access limitation/release-closeout qualification.

### Ambiguities / Evidence Gaps

- No fresh live UI smoke, remote UI run, screenshot review, VoiceOver pass, or keyboard/focus runtime verification was performed in this audit.
- Several Swift build warnings indicate actor-isolation issues that are warnings now but future Swift-language risks.
- The predecessor proposal review is relevant but not exact because the proposal stem and scope changed.

## Requirement Summary

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | r19 stopped-state proposal and P032/P036/P072 ownership boundary are checked in | Implemented | Proposal, docs |
| REQ-002 | P043/P031 reconciliation removes P031 UI command/MCP obligations | Implemented | Docs, tests-run |
| REQ-003 | Governed UI uses GraphQL-only reads and rejects MCP/local/non-approval mutation paths | Implemented | Code, config, tests-run |
| REQ-004 | Approval-only GraphQL exception is limited to P072 approve/reject mutations | Implemented | Code, tests-run |
| REQ-005 | GraphQL schema exposes required freshness, disabled reason, write path, payload, diagnostic, and projection fields | Implemented | Schema, code, tests-run |
| REQ-006 | Machine-readable UI inventory is gate-consumed and fail-closed | Implemented | Config, tests-run |
| REQ-007 | Operator write-path guide covers all removed controls | Implemented | Docs/reference, tests-run |
| REQ-008 | Swift read clients, stores, coordinators, presenters, refresh, and subscriptions cover stopped-state read surfaces | Implemented | Code, tests-run |
| REQ-009 | Reports expose metadata and payload availability without bulk payload readback | Implemented | Schema, code, tests-run |
| REQ-010 | Degraded/fail-closed paths preserve control-plane truth and do not restore local writes | Implemented for stopped-state | Code, tests-run, docs |
| REQ-011 | Canonical P031 same-tree gate passes | Implemented | Tests-run |
| REQ-012 | Phase 3 dogfood/release closeout evidence is complete | Out of Scope for P031 stopped-state; Not Ready for release | Proposal, tests-run |

## Detailed Requirement Audit

### REQ-001 - r19 stopped-state proposal and ownership boundary are checked in

- Proposal source: Status, Boundary status, Decision Summary, Final Recommendation.
- Status: **Implemented**.
- Evidence types: `proposal`, `docs`.
- Evidence references: `docs/proposals/031-thin-graphql-ui-rewrite.md`, `docs/reference/ui-action-boundary.md`.
- Implementation mapping: The proposal states that P031 stops at GraphQL-only read-boundary stabilization and hands product polish/dogfood/release tails to P032/P036, with approval-only mutation scope owned by P072.
- Gap / note: None for stopped-state conformance.

### REQ-002 - P043/P031 reconciliation removes P031 UI command/MCP obligations

- Proposal source: P043/P031 Reconciliation, Goals, Non-Goals.
- Status: **Implemented**.
- Evidence types: `docs`, `tests-run`.
- Evidence references: `docs/reference/query-projections-and-client-consumption-contract.md`, `scripts/test-gate.sh`.
- Implementation mapping: The P043 gate checks that stale command-completion, receipt, correlation, and MCP control obligations are not assigned to P031 UI.
- Gap / note: P043 prerequisite tests passed inside both `proposal-031` and `proposal-072`.

### REQ-003 - Governed UI uses GraphQL-only reads and rejects MCP/local/non-approval mutation paths

- Proposal source: Read Plane, UI Write Prohibition, Static guard requirements.
- Status: **Implemented**.
- Evidence types: `code`, `config`, `tests-run`.
- Evidence references: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `Chainworks Forge/Views/RunsHomeView.swift`, `docs/reference/p031-thin-ui-inventory.json`, `scripts/p031-thin-ui-gate.py`.
- Implementation mapping: The request validator rejects forbidden operation names and fields, the inventory enumerates governed surfaces and forbidden patterns, and the P031 gate passes on the clean tree.
- Gap / note: No live UI runtime was executed.

### REQ-004 - Approval-only GraphQL exception is limited to P072 approve/reject mutations

- Proposal source: Boundary status, Approval Diagnostic Contract, Non-Goals.
- Status: **Implemented**.
- Evidence types: `code`, `tests-run`.
- Evidence references: `P031ThinGraphQLReadBoundary.swift`, `RunsHomeView.swift`, `./scripts/test-gate.sh proposal-072`.
- Implementation mapping: Embedded GraphQL documents include `P072ApproveApproval` and `P072RejectApproval`; Runs Home routes approval settlement through `P072ApprovalMutationClient`; P072 tests prove UI operators are limited to the two approval mutations and denied non-approval mutations.
- Gap / note: Runtime approval settlement was not exercised against a live daemon.

### REQ-005 - GraphQL schema exposes required read-state contracts

- Proposal source: Schema Contract.
- Status: **Implemented**.
- Evidence types: `schema`, `code`, `tests-run`.
- Evidence references: `control-plane/crates/graphql-server/src/types/p031.rs`, `types/run.rs`, `types/stage.rs`, `types/approval.rs`, `types/artifact.rs`, `./scripts/test-gate.sh proposal-031`.
- Implementation mapping: Server schema/types expose required enum values and fields for freshness, disabled reason, write-path state, payload availability/unavailable reason, diagnostic IDs/debug detail, projection presence/update/lag, and report metadata behavior. P031 GraphQL schema tests passed.
- Gap / note: None found for stopped-state schema contract.

### REQ-006 - Machine-readable UI inventory is gate-consumed and fail-closed

- Proposal source: UI Ownership Inventory.
- Status: **Implemented**.
- Evidence types: `config`, `tests-run`.
- Evidence references: `docs/reference/p031-thin-ui-inventory.json`, `scripts/p031-thin-ui-gate.py`.
- Implementation mapping: The inventory lists governed Swift files, embedded operation names, explicit exclusions, allowed matches, and forbidden guard groups; the P031 static gate consumes it and passed.
- Gap / note: None found.

### REQ-007 - Operator write-path guide covers all removed controls

- Proposal source: Rollout, Operator write-path guide, Dogfood start acceptance packet.
- Status: **Implemented** for stopped-state guide coverage.
- Evidence types: `docs`, `config`, `tests-run`.
- Evidence references: `docs/reference/p031-operator-write-path-guide.json`, `./scripts/test-gate.sh proposal-031`, `./scripts/test-gate.sh proposal-072`.
- Implementation mapping: The guide covers create idea, start/cancel run, retry stage, approvals, steward, sessions, clone, compare, experiments, runtime health, and agent reset. It validates approvals and stage retry as external workflows and marks other controls temporarily unavailable with follow-up IDs.
- Gap / note: Several rows are pending/temporarily unavailable by design. That is acceptable for the P031 stopped-state but blocks release/dogfood viability unless completed or waived.

### REQ-008 - Swift read clients, stores, coordinators, presenters, refresh, and subscriptions cover stopped-state read surfaces

- Proposal source: Phase 0c, Phase 1, In-scope reads, Read Refresh Contract.
- Status: **Implemented**.
- Evidence types: `code`, `tests-run`.
- Evidence references: `P031ThinGraphQLReadBoundary.swift`, `RunsHomeView.swift`, `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`, `./scripts/test-gate.sh proposal-072`.
- Implementation mapping: P031 read store/coordinators cover Runs Home, run detail, stages, approvals, artifacts, reports, daemon lifecycle, targeted refresh, and subscriptions. The targeted Swift suite passed 61 tests.
- Gap / note: Build emitted actor-isolation warnings in subscription methods; see `ARCH-001`.

### REQ-009 - Reports expose metadata and payload availability without bulk payload readback

- Proposal source: Report payload indicators, Schema Contract, Non-Goals.
- Status: **Implemented**.
- Evidence types: `schema`, `code`, `tests-run`.
- Evidence references: `types/artifact.rs`, `Proposal031ThinGraphQLReadBoundaryTests.swift`, `./scripts/test-gate.sh proposal-031`.
- Implementation mapping: Report metadata is represented with payload availability and unavailable reason; bulk list/detail documents avoid `payloadText`, while selected artifact payload readback is separated.
- Gap / note: Full report payload rendering remains follow-up.

### REQ-010 - Degraded/fail-closed paths preserve control-plane truth and do not restore local writes

- Proposal source: Degraded-state criteria, Fail-closed action, Phase 0d.
- Status: **Implemented for stopped-state**.
- Evidence types: `code`, `tests-run`, `docs`.
- Evidence references: `Proposal031ThinGraphQLReadBoundaryTests.swift`, `docs/evidence/p031-degraded-state-evidence.md`, `docs/reference/p031-phase-0-artifact-manifest.json`.
- Implementation mapping: Swift tests cover fail-closed errors without local fallback, schema mismatch handling without fallback, and explicit daemon restart action. Evidence artifacts exist for degraded state.
- Gap / note: Release-ready degraded-state evidence still carries a scripted-drill/waiver-pending qualification.

### REQ-011 - Canonical P031 same-tree gate passes

- Proposal source: Hold criteria, Acceptance Packets, Test/evidence requirements.
- Status: **Implemented**.
- Evidence types: `tests-run`.
- Evidence references: `./scripts/test-gate.sh proposal-031`.
- Implementation mapping: On the audited clean tree, `proposal-031` passed P043 prerequisite tests, P031 static/inventory/write-path checks, 11 P031 GraphQL schema tests, and 5 P031 authorization tests.
- Gap / note: None.

### REQ-012 - Phase 3 dogfood/release closeout evidence is complete

- Proposal source: Decision Summary, Phase 3, Rollout, Metrics, Final Recommendation.
- Status: **Out of Scope for P031 stopped-state; Not Ready for release**.
- Evidence types: `proposal`, `docs`, `tests-run`.
- Evidence references: `./scripts/test-gate.sh proposal-031-readiness`, `docs/reference/p031-phase-0-artifact-manifest.json`, `docs/evidence/p031-dogfood-signoff.md`.
- Implementation mapping: The proposal explicitly says P031 should stop after the GraphQL-only read boundary and move dogfood/signoff/stabilization to P032/P036. The readiness gate exists and correctly fails until the release evidence is complete.
- Gap / note: This does not block P031 stopped-state conformance, but it blocks any release-ready or closeout-ready claim.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Implemented for stopped-state | Release evidence is outside stopped-state but still incomplete | High |
| Apple UX | Ready for stopped-state handoff, not release signoff | No live UI/accessibility/VoiceOver execution in this audit | Medium |
| Apple architecture | Mostly conformant | Swift actor-isolation warnings in P031 subscription methods | Medium |
| API contract | Conformant | None found in P031/P072 schema and policy tests | High |
| Observability/rollout | Not release-ready | Phase 0d/Phase 3 evidence remains pending/qualified | High |
| Execution truth | Conformant for gate-proven same-tree stopped-state | Dogfood parity/outcome evidence incomplete | High |
| Release readiness | Not Ready | `proposal-031-readiness` fails | High |

## Routed Specialist Findings

### READY-001 - Release readiness gate fails on Phase 0d and Phase 3 evidence

- Reviewer: `observability_rollout_reviewer`, `apple_ux_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-012
- Evidence types: `tests-run`, `docs`
- Evidence references: `./scripts/test-gate.sh proposal-031-readiness`, `docs/reference/p031-phase-0-artifact-manifest.json`, `docs/evidence/p031-dogfood-signoff.md`, `docs/evidence/p031-ux-accessibility-signoff.md`
- Why it matters: P031 stopped-state implementation is gate-green, but release closeout still lacks complete dogfood, degraded-state, freshness, and UX/accessibility signoff evidence. The readiness gate fails closed, which prevents accidental release-readiness claims.
- Recommended action: Keep P031 closed at the stopped-state boundary and complete the remaining evidence under P032/P036, or explicitly attach dated waivers with named owner, scope, and deadline before attempting release closeout.
- Acceptance criteria: `./scripts/test-gate.sh proposal-031-readiness` passes with no pending/template/limitation statuses, signed dogfood evidence, no unchecked dogfood items, and no release-closeout qualifications in evidence docs.

### ARCH-001 - P031 subscription methods emit Swift actor-isolation warnings

- Reviewer: `apple_arch_reviewer`
- Severity: Minor
- Confidence: Medium
- Related requirements: REQ-008
- Evidence types: `tests-run`, `code`
- Evidence references: `./scripts/test-gate.sh proposal-072`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:2225`, `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:2235`
- Why it matters: The targeted macOS test gate passes, but Xcode reports that `subscribe` calls from `nonisolated` P031 subscription methods use main actor-isolated API/conformance in a synchronous nonisolated context and notes this becomes an error in Swift 6 language mode.
- Recommended action: Move the subscription API boundary to an async actor-aware call path or adjust isolation/conformance ownership before adopting stricter Swift language mode.
- Acceptance criteria: `proposal-072-swift` passes without P031 actor-isolation warnings in `subscribeToRunStatus` and `subscribeToDaemonStatus`.

## Readiness Checklist

| Check | Result | Evidence |
| --- | --- | --- |
| Build or canonical gate status | Passed | `./scripts/test-gate.sh proposal-031` passed |
| Focused static inventory/write-path gate | Passed | `python3 scripts/p031-thin-ui-gate.py --repo-root .` passed |
| Adjacent approval mutation boundary | Passed | `./scripts/test-gate.sh proposal-072` passed |
| Core server/API validation | Passed | P043 7 tests, P031 11 schema tests, P031 5 authorization tests, and P072 approval policy tests passed through gates |
| Core Swift boundary validation | Passed | 61 targeted `Proposal031ThinGraphQLReadBoundaryTests` passed through `proposal-072` |
| Core UI runtime/integration validation | Not run | No remote UI smoke or live app run was requested or performed |
| Empty/loading/error/offline/permission states | Partially verified | Swift tests and docs cover fail-closed/error/degraded paths; no live UI exercise |
| Accessibility/localization/privacy/permissions/entitlements | Partially verified | Evidence exists but readiness gate still flags UX/accessibility signoff limitation |
| Full regression or canonical full/proposal gate passed on audited tree | Proposal gate passed | `proposal-031` passed; `proposal-031-readiness` failed |
| Release closeout readiness | Failed | Phase 0d/Phase 3 evidence pending/qualified |

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `git status --short` | Clean before report | No dirty tracked or untracked files before writing R8 |
| `python3 scripts/p031-thin-ui-gate.py --repo-root .` | Passed | Thin UI inventory, static guards, and write-path guide passed |
| `./scripts/test-gate.sh proposal-031` | Passed | P043 prerequisite 7 tests passed; P031 static gate passed; P031 GraphQL schema 11 tests passed; P031 authorization 5 tests passed |
| `./scripts/test-gate.sh proposal-031-readiness` | Failed | Manifest status pending, degraded/freshness/UX/dogfood evidence not closeout-ready, dogfood not signed/complete, unchecked dogfood items, UX signoff contains release-closeout qualification |
| `./scripts/test-gate.sh proposal-072` | Passed | Includes P031 gate, 61 targeted macOS P031 boundary tests, domain/auth approval mutation policy checks, and GraphQL approval/non-approval mutation checks |

## Final Verdict and Recommended Next Actions

Overall conformance is **Implemented for the P031 stopped-state cutover**. The clean-tree `proposal-031` gate passes, the governed Swift boundary is GraphQL-only except for the P072 approval mutations, schema/API contracts are present, static guards and inventory are gate-consumed, and the operator write-path guide exists.

Overall implementation readiness is **Not Ready for release closeout**. The blocker is not the P031 implementation boundary anymore; it is missing or qualified Phase 0d/Phase 3 evidence. The readiness gate correctly fails until dogfood/signoff, degraded-state, freshness, and UX/accessibility evidence are completed or formally waived.

Recommended next actions:

1. Treat P031 as complete for the stopped-state implementation boundary.
2. Do not claim release readiness until `./scripts/test-gate.sh proposal-031-readiness` passes.
3. Move remaining dogfood, UX/accessibility, degraded-state drill/waiver, freshness confirmation, and documentation cleanup through P032/P036 as the proposal directs.
4. Address the P031 Swift actor-isolation warnings before stricter Swift language mode adoption.
