# Proposal 031 Implementation Audit R9: Thin GraphQL-Only UI Rewrite

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/031-thin-graphql-ui-rewrite.md` |
| Proposal revision | `031-2026-04-24-r19-degraded-state-correction` |
| Proposal state | Active stopped-state proposal, partially superseded by the implemented P072 UI action boundary |
| Audit mode | `auto` / implementation audit |
| Audit timestamp | 2026-05-05 19:45:03 EEST |
| Implementation target | Current worktree |
| Current HEAD | `07b0545999f3945f3411a2b586b21b6ea07d82f2` |
| Compare base | Implicit current branch/worktree |
| Initial worktree status | Clean before validation began |
| Final worktree status before report | Dirty: `Chainworks Forge/Support/DaemonLifecycleClient.swift`, `Chainworks Forge/Views/RunsHomeView.swift` |
| Report path | `docs/proposals/031-thin-graphql-ui-rewrite_IMPLEMENTATION_AUDIT_R9.md` |
| Overall Conformance | Partial for the live dirty worktree; implemented for the clean HEAD stop-state checked before new local edits appeared |
| Overall Implementation Readiness | Not Ready |
| Reviewer Selection Reuse | Partially reused |
| Audit Confidence | High for gate results and current dirty-tree blocker; Medium for the uncommitted Swift diff |

## Implementation Target / Compare Base

This audit started from a clean worktree at HEAD `07b0545999f3945f3411a2b586b21b6ea07d82f2`. On that clean tree, the focused P031 static gate, canonical P031 gate, readiness gate path, and adjacent P072 gate were executed.

During or after validation, two uncommitted Swift changes appeared:

- `Chainworks Forge/Views/RunsHomeView.swift`: adds accessibility identifiers to Runs Home, run detail, run rows, daemon lifecycle card, and freshness badge.
- `Chainworks Forge/Support/DaemonLifecycleClient.swift`: clears daemon status to `nil` when snapshot refresh fails.

Those changes are not part of HEAD and were not authored by this audit. Because P031's gate validates P041 same-tree provenance against the live `git status --short` snapshot, the final current worktree is no longer same-tree gate green.

No implementation files were modified by this audit. This report is the single generated artifact.

## Prior Proposal-Review Reuse

Reviewer selection reuse: **Partially reused**.

The active renamed proposal path has no adjacent `.review/` directory. The direct predecessor review under `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/` remains relevant for macOS operator UX, Swift architecture, API contract, rollout/readiness, and execution truth. Reuse remains partial because r19 narrowed the predecessor GraphQL+MCP framing to GraphQL-only reads plus the P072 approval-only GraphQL mutation exception.

Selected reviewers:

| Reviewer | Reason |
| --- | --- |
| `apple_ux_reviewer` | P031 governs operator read flows, accessibility/readiness evidence, diagnostics, and degraded/freshness states. |
| `apple_arch_reviewer` | Swift read model, coordinator, transport, daemon lifecycle, and write-boundary ownership are in scope. |
| `api_contract_reviewer` | GraphQL fields, enums, authorization, subscriptions, and approval mutation boundary are in scope. |
| `observability_rollout_reviewer` | Manifest, gates, evidence, dogfood, signoff, and release hold criteria are explicit proposal concerns. |
| `chainworks_execution_truth_reviewer` | P031 depends on server projection truth, P041 same-tree evidence, and no restored local workflow truth. |

Rejected close alternatives:

| Reviewer | Rejection reason |
| --- | --- |
| `macos_ui_reviewer` | Visual/navigation restoration is handed to P036; no live UI/screenshot review was requested. |
| `rust_arch_reviewer` | Rust work is covered through API/schema contract checks rather than new Rust module architecture. |
| `rust_reliability_reviewer` | Retry/resume/cancel semantics remain out of P031 and belong to later write-path proposals. |
| `rust_security_reviewer` | Authorization behavior is covered by P031/P072 tests; no new auth mechanism was introduced. |
| `product_reviewer` | Product acceptance and dogfood completion are deferred to P032/P036; readiness findings cover the current blocker. |
| Go reviewers | No Go implementation surface or `go.mod` exists. |

## Prior Metrics Preserved

Leading metric:

- Percentage of P031-owned screens whose visible state is sourced only from named GraphQL read models/projections.

Guardrail metric:

- Zero P031-owned operator mutations bypass MCP/CommandHandler/audit unless explicitly deferred and disabled in the UI.

Updated r19 interpretation:

- P031 has zero governed UI MCP usage and zero non-approval GraphQL mutations; only `approveApproval` and `rejectApproval` are allowed through the P072 boundary.

Decision checkpoint:

- Do not start implementation until the proposal has a read-model matrix, action/deferral matrix, Swift cutover inventory, and canonical gate bundle.

Current audit result:

- The clean HEAD satisfies the stopped-state implementation gate. The live dirty worktree requires reconciliation and rerun before the same claim can be made for the current working copy.

## Proposal State and Contract Summary

P031 r19 is a stopped-state cutover proposal. It lands durable GraphQL-only read ownership and thin read surfaces, while handing visual/product polish, dogfood release evidence, degraded drills/waivers, freshness confirmation, and documentation cleanup to P032/P036. P072 supersedes the original all-mutation ban with one narrow exception: governed SwiftUI may use GraphQL mutations only for `approveApproval` and `rejectApproval`.

Locked decisions:

- Governed macOS UI reads workflow truth from GraphQL projections only.
- Governed UI has no MCP reads/writes, no local workflow mutation fallback, no command payloads, no receipts, and no command correlation.
- Non-approval GraphQL mutations are prohibited from governed UI code.
- Approval rows may use only the P072 `approveApproval` / `rejectApproval` GraphQL exception.
- Removed write controls remain hidden, unavailable, diagnostic-only, or external-follow-up guided.
- Degraded state is a read-only UI state over control-plane truth, not a restored local orchestrator.

## Platform / Product Scope

Apple scope: **macOS**.

Backend/service scope: **cross-stack API/data/rollout** across SwiftUI read models, Rust GraphQL schema/types/tests, static gates, reference artifacts, and release evidence.

## Primary Implementation Flows

1. Operator opens Runs Home and reads run/stage projection truth from GraphQL with freshness and projection-lag state.
2. Operator inspects run detail, stages, artifacts, report metadata, and daemon lifecycle without local workflow truth or raw artifact probing.
3. Operator views approval rows, sees server-derived actionability/diagnostics, and can settle only actionable approvals through the P072 GraphQL exception.
4. Operator refreshes visible read surfaces through targeted GraphQL reads/subscriptions without MCP, local recovery, or non-approval mutations.
5. Release owner validates the stopped-state contract through `proposal-031`, then separately validates dogfood/signoff readiness through `proposal-031-readiness`.

## Proposal Fidelity / Divergence Inventory

### Matches

- On the initial clean HEAD, `python3 scripts/p031-thin-ui-gate.py --repo-root .` passed.
- On the initial clean HEAD, `./scripts/test-gate.sh proposal-031` passed.
- On the initial clean HEAD, `./scripts/test-gate.sh proposal-072` passed, including targeted P031 Swift tests and approval-only mutation policy checks.
- The R8 `ARCH-001` P031 actor-isolation warning was not observed in the current P072 run when compiling `P031ThinGraphQLReadBoundary.swift`.
- UX/accessibility signoff improved since R8: manifest now records `signed_human_accessibility_check`, and `proposal-031-readiness` no longer reports the prior UX/accessibility release-closeout qualification.
- The current uncommitted Runs Home diff adds accessibility identifiers, which aligns directionally with UI testability/accessibility evidence needs.

### Divergences

- The final live worktree is dirty with two uncommitted Swift files, so the current worktree no longer passes the P031 same-tree gate.
- `./scripts/test-gate.sh proposal-031-readiness` still fails on dogfood/degraded/freshness evidence and unsigned/incomplete Phase 3 signoff.
- The uncommitted `DaemonLifecycleClient.swift` status-clearing behavior affects degraded/daemon state and has not been validated by a successful same-tree P031 gate.

### Ambiguities / Evidence Gaps

- The source and intended scope of the two uncommitted Swift changes are not known to this audit.
- No live UI smoke, remote UI run, screenshot review, or runtime VoiceOver execution was performed in this audit.
- The clean-tree gate evidence is useful for HEAD, but it is stale for the final dirty worktree.

## Requirement Summary

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | r19 stopped-state proposal and P032/P036/P072 ownership boundary are checked in | Implemented | Proposal, docs |
| REQ-002 | P043/P031 reconciliation removes P031 UI command/MCP obligations | Implemented | Docs, tests-run |
| REQ-003 | Governed UI uses GraphQL-only reads and rejects MCP/local/non-approval mutation paths | Implemented on clean HEAD; Not Verifiable on final dirty worktree | Code, config, tests-run |
| REQ-004 | Approval-only GraphQL exception is limited to P072 approve/reject mutations | Implemented on clean HEAD | Code, tests-run |
| REQ-005 | GraphQL schema exposes required freshness, disabled reason, write path, payload, diagnostic, and projection fields | Implemented | Schema, tests-run |
| REQ-006 | Machine-readable UI inventory is gate-consumed and fail-closed | Implemented on clean HEAD; Not Verifiable on final dirty worktree | Config, tests-run |
| REQ-007 | Operator write-path guide covers all removed controls | Implemented | Docs/reference, tests-run |
| REQ-008 | Swift read clients, stores, coordinators, presenters, refresh, and subscriptions cover stopped-state read surfaces | Implemented on clean HEAD; final dirty Swift diff not same-tree verified | Code, tests-run, diff |
| REQ-009 | Reports expose metadata and payload availability without bulk payload readback | Implemented | Schema, tests-run |
| REQ-010 | Degraded/fail-closed paths preserve control-plane truth and do not restore local writes | Not Verifiable on final dirty worktree | Code, diff, tests-run |
| REQ-011 | Canonical P031 same-tree gate passes on audited implementation | Not Verifiable for final dirty worktree | Tests-run |
| REQ-012 | Phase 3 dogfood/release closeout evidence is complete | Out of Scope for P031 stopped-state; Not Ready for release | Proposal, tests-run |

## Detailed Requirement Audit

### REQ-001 - r19 stopped-state proposal and ownership boundary are checked in

- Proposal source: Status, Boundary status, Decision Summary, Final Recommendation.
- Status: **Implemented**.
- Evidence types: `proposal`, `docs`.
- Evidence references: `docs/proposals/031-thin-graphql-ui-rewrite.md`, `docs/reference/ui-action-boundary.md`.
- Implementation mapping: The proposal stops P031 at GraphQL-only read-boundary stabilization and hands product polish/readiness tails to P032/P036, with approval mutation scope owned by P072.
- Gap / note: None for stopped-state scope.

### REQ-002 - P043/P031 reconciliation removes P031 UI command/MCP obligations

- Proposal source: P043/P031 Reconciliation, Goals, Non-Goals.
- Status: **Implemented**.
- Evidence types: `docs`, `tests-run`.
- Evidence references: `docs/reference/query-projections-and-client-consumption-contract.md`, `scripts/test-gate.sh`.
- Implementation mapping: The P043 prerequisite gate passed and checks that command-completion, receipt, correlation, and MCP control obligations are not assigned to P031 UI.
- Gap / note: None found.

### REQ-003 - Governed UI uses GraphQL-only reads and rejects MCP/local/non-approval mutation paths

- Proposal source: Read Plane, UI Write Prohibition, Static guard requirements.
- Status: **Implemented on clean HEAD; Not Verifiable on final dirty worktree**.
- Evidence types: `code`, `config`, `tests-run`, `diff`.
- Evidence references: `P031ThinGraphQLReadBoundary.swift`, `RunsHomeView.swift`, `docs/reference/p031-thin-ui-inventory.json`, `scripts/p031-thin-ui-gate.py`.
- Implementation mapping: Clean HEAD passed the P031 gate and the P072 boundary gate. Final live status includes uncommitted changes in a governed P031 UI file, so same-tree verification is no longer valid for the current working copy.
- Gap / note: Commit or shelve the Swift diff, then rerun `proposal-031`.

### REQ-004 - Approval-only GraphQL exception is limited to P072 approve/reject mutations

- Proposal source: Boundary status, Approval Diagnostic Contract, Non-Goals.
- Status: **Implemented on clean HEAD**.
- Evidence types: `code`, `tests-run`.
- Evidence references: `./scripts/test-gate.sh proposal-072`.
- Implementation mapping: P072 passed, proving the approval-only mutation boundary and denial of non-approval mutations for UI principals.
- Gap / note: Final dirty worktree did not complete a same-tree P072 rerun after the uncommitted changes appeared; the P031 prerequisite fails first.

### REQ-005 - GraphQL schema exposes required read-state contracts

- Proposal source: Schema Contract.
- Status: **Implemented**.
- Evidence types: `schema`, `tests-run`.
- Evidence references: `control-plane/crates/graphql-server/src/types/p031.rs`, `types/run.rs`, `types/stage.rs`, `types/approval.rs`, `types/artifact.rs`, `./scripts/test-gate.sh proposal-031`.
- Implementation mapping: P031 schema tests passed on clean HEAD, covering freshness, disabled reason, write-path state, payload availability/unavailable reason, diagnostic fields, projection fields, and report metadata behavior.
- Gap / note: No schema gap found.

### REQ-006 - Machine-readable UI inventory is gate-consumed and fail-closed

- Proposal source: UI Ownership Inventory.
- Status: **Implemented on clean HEAD; Not Verifiable on final dirty worktree**.
- Evidence types: `config`, `tests-run`.
- Evidence references: `docs/reference/p031-thin-ui-inventory.json`, `scripts/p031-thin-ui-gate.py`.
- Implementation mapping: Clean HEAD passed the inventory/static guard gate. Final dirty worktree fails P041 same-tree provenance before successful completion.
- Gap / note: The new `RunsHomeView.swift` accessibility identifiers need to be included in a clean same-tree validation run.

### REQ-007 - Operator write-path guide covers all removed controls

- Proposal source: Rollout, Operator write-path guide, Dogfood start acceptance packet.
- Status: **Implemented** for stopped-state guide coverage.
- Evidence types: `docs`, `tests-run`.
- Evidence references: `docs/reference/p031-operator-write-path-guide.json`, `./scripts/test-gate.sh proposal-031`.
- Implementation mapping: The guide covers all removed controls, validates approvals and stage retry as external workflows, and marks remaining controls unavailable with follow-up IDs.
- Gap / note: Pending rows still affect dogfood/release viability, not stopped-state conformance.

### REQ-008 - Swift read clients, stores, coordinators, presenters, refresh, and subscriptions cover stopped-state read surfaces

- Proposal source: Phase 0c, Phase 1, In-scope reads, Read Refresh Contract.
- Status: **Implemented on clean HEAD; final dirty Swift diff not same-tree verified**.
- Evidence types: `code`, `tests-run`, `diff`.
- Evidence references: `P031ThinGraphQLReadBoundary.swift`, `RunsHomeView.swift`, `DaemonLifecycleClient.swift`, `./scripts/test-gate.sh proposal-072`.
- Implementation mapping: Clean HEAD passed targeted P031 Swift tests through P072. The final dirty diff changes Runs Home accessibility identifiers and daemon status error behavior, which should be validated on a clean tree.
- Gap / note: The P031-specific actor warning from R8 appears resolved.

### REQ-009 - Reports expose metadata and payload availability without bulk payload readback

- Proposal source: Report payload indicators, Schema Contract, Non-Goals.
- Status: **Implemented**.
- Evidence types: `schema`, `tests-run`.
- Evidence references: `types/artifact.rs`, `Proposal031ThinGraphQLReadBoundaryTests.swift`, `./scripts/test-gate.sh proposal-031`.
- Implementation mapping: P031 tests prove report metadata/payload availability behavior and separated selected payload readback on clean HEAD.
- Gap / note: Full report payload rendering remains follow-up.

### REQ-010 - Degraded/fail-closed paths preserve control-plane truth and do not restore local writes

- Proposal source: Degraded-state criteria, Fail-closed action, Phase 0d.
- Status: **Not Verifiable on final dirty worktree**.
- Evidence types: `code`, `diff`, `tests-run`.
- Evidence references: `DaemonLifecycleClient.swift`, `Proposal031ThinGraphQLReadBoundaryTests.swift`, `docs/evidence/p031-degraded-state-evidence.md`.
- Implementation mapping: Clean HEAD had passing fail-closed/degraded-related tests. The final uncommitted daemon lifecycle diff changes error behavior by clearing status on snapshot failures; that may be correct but is not same-tree gate verified.
- Gap / note: Revalidate the dirty change after it is committed or intentionally shelved.

### REQ-011 - Canonical P031 same-tree gate passes on audited implementation

- Proposal source: Hold criteria, Acceptance Packets, Test/evidence requirements.
- Status: **Not Verifiable for final dirty worktree**.
- Evidence types: `tests-run`.
- Evidence references: `./scripts/test-gate.sh proposal-031`, `python3 scripts/p031-thin-ui-gate.py --repo-root .`.
- Implementation mapping: Clean HEAD passed `proposal-031`. After the two Swift changes appeared, both the focused P031 gate and canonical P031 gate fail because P041 same-tree provenance expects a clean live status snapshot.
- Gap / note: Final failure reports live status line count 2.

### REQ-012 - Phase 3 dogfood/release closeout evidence is complete

- Proposal source: Decision Summary, Phase 3, Rollout, Metrics, Final Recommendation.
- Status: **Out of Scope for P031 stopped-state; Not Ready for release**.
- Evidence types: `proposal`, `docs`, `tests-run`.
- Evidence references: `./scripts/test-gate.sh proposal-031-readiness`, `docs/reference/p031-phase-0-artifact-manifest.json`, `docs/evidence/p031-dogfood-signoff.md`.
- Implementation mapping: P031 intentionally stops at the read-boundary handoff. The readiness gate remains red until dogfood/freshness/degraded-state/signoff evidence is complete or waived.
- Gap / note: UX/accessibility signoff is now signed, but dogfood and degraded/freshness blockers remain.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial for live dirty worktree; implemented on clean HEAD | Final worktree does not pass same-tree P031 gate | High |
| Apple UX | Improved but not release-ready | Accessibility signoff improved, but operator dogfood acceptance remains incomplete | Medium |
| Apple architecture | Not fully verified on final dirty worktree | Uncommitted daemon lifecycle/Runs Home Swift changes need same-tree validation | Medium |
| API contract | Conformant on clean HEAD | No schema/API gap found | High |
| Observability/rollout | Not release-ready | Phase 0d/Phase 3 evidence remains pending/qualified | High |
| Execution truth | Blocked for live dirty worktree | P041 same-tree provenance mismatch due dirty status | High |
| Release readiness | Not Ready | `proposal-031-readiness` fails | High |

## Routed Specialist Findings

### READY-001 - Final current worktree is not same-tree gate green

- Reviewer: `chainworks_execution_truth_reviewer`, `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-003, REQ-006, REQ-008, REQ-010, REQ-011
- Evidence types: `tests-run`, `diff`
- Evidence references: `python3 scripts/p031-thin-ui-gate.py --repo-root .`, `./scripts/test-gate.sh proposal-031`, `git status --short`
- Why it matters: P031's implementation gate intentionally fails when P041 same-tree provenance no longer matches the live worktree. The clean HEAD passed, but the final live tree has uncommitted Swift changes in `DaemonLifecycleClient.swift` and `RunsHomeView.swift`, so current implementation conformance cannot be claimed from stale clean-tree evidence.
- Recommended action: Commit, stage for review, or shelve the two Swift changes, then rerun `./scripts/test-gate.sh proposal-031` and `./scripts/test-gate.sh proposal-072` on the exact tree intended for handoff.
- Acceptance criteria: `git status --short` is clean, `proposal-031` passes, and any retained Swift changes are represented in the audited tree.

### READY-002 - Release readiness still fails on dogfood, degraded-state, and freshness evidence

- Reviewer: `observability_rollout_reviewer`, `apple_ux_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-012
- Evidence types: `tests-run`, `docs`
- Evidence references: `./scripts/test-gate.sh proposal-031-readiness`, `docs/reference/p031-phase-0-artifact-manifest.json`, `docs/evidence/p031-dogfood-signoff.md`
- Why it matters: UX/accessibility signoff improved since R8, but Phase 3 closeout remains incomplete. The readiness gate still fails on manifest pending status, degraded-state evidence pending scripted drill/waiver, freshness dogfood confirmation pending, dogfood signoff template status, unsigned/incomplete signoff, and unchecked dogfood items.
- Recommended action: Complete or explicitly waive the remaining P032/P036-owned release evidence before claiming release readiness.
- Acceptance criteria: `./scripts/test-gate.sh proposal-031-readiness` passes with complete dogfood, degraded-state/freshness evidence, and signed closeout.

## Readiness Checklist

| Check | Result | Evidence |
| --- | --- | --- |
| Initial focused static inventory/write-path gate | Passed | `python3 scripts/p031-thin-ui-gate.py --repo-root .` passed before Swift dirty diff appeared |
| Initial canonical P031 gate | Passed | `./scripts/test-gate.sh proposal-031` passed on clean HEAD |
| Final current-worktree P031 gate | Failed | Dirty tree fails P041 same-tree provenance with live status line count 2 |
| Adjacent P072 approval boundary | Passed before final dirty-tree failure | `./scripts/test-gate.sh proposal-072` passed; P031-specific actor warning from R8 not observed |
| Core server/API validation | Passed on clean HEAD | P043 7 tests, P031 11 schema tests, P031 5 authorization tests |
| Core Swift boundary validation | Passed on clean HEAD | Targeted P031 Swift boundary tests passed through P072 |
| Core UI runtime/integration validation | Not run | No remote UI smoke or live app run requested/performed |
| Accessibility/UX evidence | Improved, not release-complete | UX/accessibility signoff is signed; dogfood signoff still incomplete |
| Release closeout readiness | Failed | `proposal-031-readiness` failed |

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `git status --short` | Clean initially | Worktree was clean before validation began |
| `python3 scripts/p031-thin-ui-gate.py --repo-root .` | Passed initially | Thin UI inventory, static guards, and write-path guide passed on clean tree |
| `./scripts/test-gate.sh proposal-031` | Passed initially | P043 7 tests, P031 static gate, P031 11 schema tests, and P031 5 authorization tests passed |
| `./scripts/test-gate.sh proposal-031-readiness` | Failed | P031 gate passed first; readiness failed on pending manifest/degraded/freshness/dogfood evidence and incomplete dogfood signoff |
| `./scripts/test-gate.sh proposal-072` | Passed | Includes P031 gate, targeted P031 Swift tests, and P072 domain/auth/GraphQL approval mutation policy checks |
| `git diff -- Chainworks Forge/Support/DaemonLifecycleClient.swift Chainworks Forge/Views/RunsHomeView.swift` | Observed dirty diff | Accessibility identifiers added; daemon status cleared on snapshot errors |
| `python3 scripts/p031-thin-ui-gate.py --repo-root .` | Failed after dirty diff appeared | P041 live status snapshot mismatch, line count 1 at that moment |
| `./scripts/test-gate.sh proposal-031` | Failed after dirty diff appeared | P043 prerequisite passed; P031 gate failed on P041 live status snapshot mismatch, line count 2 |

## Final Verdict and Recommended Next Actions

Overall conformance is **Partial for the live dirty worktree**. The clean HEAD `07b0545999f3945f3411a2b586b21b6ea07d82f2` satisfied the P031 stopped-state gate, but the final current worktree contains uncommitted Swift changes and fails the same-tree P031 gate. Do not claim current-worktree conformance until those changes are reconciled and the gate is rerun.

Overall implementation readiness is **Not Ready**. Even ignoring the dirty-worktree blocker, release closeout still fails on dogfood, degraded-state, freshness, and incomplete Phase 3 signoff evidence.

Recommended next actions:

1. Decide whether to keep the uncommitted `RunsHomeView.swift` and `DaemonLifecycleClient.swift` changes.
2. Commit or shelve those changes, then rerun `./scripts/test-gate.sh proposal-031` and `./scripts/test-gate.sh proposal-072` on a clean tree.
3. Complete or explicitly waive the remaining dogfood, degraded-state, and freshness evidence before rerunning `./scripts/test-gate.sh proposal-031-readiness`.
