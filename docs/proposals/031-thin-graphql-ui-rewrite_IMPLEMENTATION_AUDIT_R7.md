# Proposal 031 Implementation Audit R7: Thin GraphQL-Only UI Rewrite

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/031-thin-graphql-ui-rewrite.md` |
| Proposal revision | `031-2026-04-24-r19-degraded-state-correction` |
| Proposal state | Active stopped-state proposal, partially superseded by the implemented P072 UI action boundary |
| Audit mode | `auto` / implementation audit |
| Audit timestamp | 2026-05-05 10:19:22 EEST |
| Implementation target | Current worktree |
| Current HEAD | `122761b42dfd0c40ba06c70f76d1a38d0f97a3d5` |
| Compare base | Implicit current branch/worktree |
| Worktree status | Dirty, 15 `git status --short` lines at audit time |
| Report path | `docs/proposals/031-thin-graphql-ui-rewrite_IMPLEMENTATION_AUDIT_R7.md` |
| Overall Conformance | Partial |
| Overall Implementation Readiness | Not Ready |
| Reviewer Selection Reuse | Partially reused |
| Audit Confidence | High for static/code/gate evidence; Medium for UI runtime behavior |

## Implementation Target / Compare Base

This audit evaluates the current worktree at HEAD `122761b42dfd0c40ba06c70f76d1a38d0f97a3d5`. The tree is not clean. That matters for P031 because the canonical P031 gate validates the P041 parity runtime row against the live same-tree status snapshot, and the live status currently has 15 lines while the recorded P041 provenance snapshot has 0.

No implementation files were modified by this audit. This report is the single generated artifact.

## Prior Proposal-Review Reuse

Reviewer selection reuse: **Partially reused**.

The renamed active proposal path does not have an adjacent `.review/` directory, but the direct predecessor review artifacts under `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/` still cover the same macOS operator UI, Swift architecture, GraphQL/API contract, rollout/readiness, and execution-truth concerns. Reuse is partial because r19 narrows the contract to GraphQL-only reads with an approval-only GraphQL mutation exception and defers broader product/visual polish to P032/P036/P072.

Selected reviewers:

| Reviewer | Reason |
| --- | --- |
| `apple_ux_reviewer` | P031 changes operator flows, disabled action clarity, degraded/freshness states, and approval diagnostics. |
| `apple_arch_reviewer` | SwiftUI read model, coordinator, transport, subscription, and mutation-boundary ownership are central. |
| `api_contract_reviewer` | GraphQL schema/read model fields, enums, authorization, subscriptions, and payload contracts are in scope. |
| `observability_rollout_reviewer` | Manifest, gates, evidence docs, dogfood readiness, and release hold criteria are explicit proposal commitments. |
| `chainworks_execution_truth_reviewer` | P031 depends on durable server projection truth and P041 same-tree parity evidence. |

Rejected close alternatives:

| Reviewer | Rejection reason |
| --- | --- |
| `macos_ui_reviewer` | Visual polish and final UI acceptance are explicitly deferred; no fresh runtime screenshot/UI smoke was requested or obtained. |
| `rust_arch_reviewer` | Rust schema surfaces are relevant, but no new worker, retry, storage, or async ownership behavior is introduced beyond API contract checks. |
| `rust_reliability_reviewer` | Retry/resume/cancellation semantics are explicitly out of P031 scope after the P072 boundary. |
| `rust_security_reviewer` | Authorization is covered by P031 GraphQL contract tests; no new auth mechanism or public security boundary was found. |
| `product_reviewer` | Product polish and dogfood decision checkpoints are deferred; readiness blockers are better represented by rollout/readiness findings. |
| Go reviewers | No real Go implementation surface or `go.mod` is present. |

## Proposal State and Contract Summary

P031 r19 governs a stopped-state migration of the macOS operator UI to thin GraphQL-only reads over server projections. It preserves only an approval-specific GraphQL mutation exception through P072 and explicitly excludes MCP reads/writes, local workflow mutation fallback, and non-approval GraphQL mutations. Create/start/cancel/retry/reset/compact/clone/recover/runtime/context actions are not owned by P031.

Locked decisions and acceptance criteria extracted from the proposal:

- macOS UI reads workflow truth through GraphQL projections only.
- UI write controls outside approval settlement are removed, hidden, or converted to diagnostics/external guidance.
- `approveApproval` and `rejectApproval` are the only allowed GraphQL mutations in the P031 governed UI boundary.
- GraphQL read models expose freshness, projection lag, disabled reason, write path state, payload availability, and diagnostic fields.
- The repository contains a machine-readable UI inventory, operator write-path guide, phase 0 artifact manifest, and static guards.
- The rollout fails closed until P027/P041/P042/P043/P031 gates and Phase 0d/Phase 3 evidence are satisfied.
- Visual polish, broader user-facing product completion, and many write-path workflows are handed to P032/P036/P072.

## Platform / Product Scope

Apple scope: **macOS**.

Backend/service scope: **cross-stack API/data/rollout** across SwiftUI client read models, Rust GraphQL schema/types/tests, repository gates, and durable reference/evidence artifacts.

## Primary Implementation Flows

1. Operator opens Runs Home and reads run/stage projection truth through GraphQL with freshness and projection-lag state.
2. Operator inspects a run, stage, artifact, and report metadata without bulk report payload fetches.
3. Operator views approval queue rows, settles only actionable approvals through the P072 GraphQL exception, and sees diagnostics for unavailable rows.
4. Operator sees daemon/freshness/degraded states and can trigger read refresh without invoking local/MCP workflow mutation paths.
5. Release/readiness validation checks the UI inventory, write-path guide, phase manifest, P041 parity row, and P031/P043 gates on the same tree.

## Proposal Fidelity / Divergence Inventory

### Matches

- The active proposal file reflects the r19 GraphQL-only stopped scope and P072 approval exception.
- `P031ThinGraphQLReadBoundary.swift` parses GraphQL operation documents, rejects forbidden operation names, allows query/subscription reads, and allows only the `P072ApproveApproval` and `P072RejectApproval` mutation documents.
- `RunsHomeView.swift` uses the P031 GraphQL read dashboard model and routes approval settlement through the P072 approval mutation client.
- Rust GraphQL types expose the P031 freshness, write-path, disabled reason, payload availability, diagnostic, and projection fields.
- The machine-readable inventory, operator write-path guide, and phase 0 artifact manifest exist under `docs/reference/`.
- Supplemental P031 GraphQL server tests pass independently: 11 library tests and 5 authorization tests.

### Divergences

- The canonical P031 gate and P031 readiness gate fail on the current tree because the P041 same-tree runtime row requires a clean live status snapshot and the live tree is dirty.
- Phase 3 dogfood/release evidence is still pending or qualified, consistent with the proposal's stopped-state handoff but not sufficient for release readiness.
- The phase 0 artifact manifest is present but does not carry per-artifact revision/commit identifiers requested by the proposal governance text.

### Ambiguities / Evidence Gaps

- No fresh runtime UI smoke, screenshot, VoiceOver, keyboard navigation, or accessibility execution was obtained during this audit.
- Prior proposal-review artifacts apply to a predecessor stem and broader GraphQL+MCP framing, so reviewer reuse is not exact.
- The current dirty tree includes P041 closeout/reference changes that are outside P031's feature code but directly affect P031's same-tree gate.

## Requirement Summary

| ID | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | r19 stopped-scope proposal state and ownership boundary are present | Implemented | Proposal, docs |
| REQ-002 | Governed UI uses GraphQL-only reads with no MCP/local write fallback and only approval mutations | Implemented | Code, inventory, tests-found |
| REQ-003 | Static UI inventory and guardrails cover governed files and forbidden operations | Implemented | Config, code, tests-run |
| REQ-004 | GraphQL schema exposes P031 projection, freshness, actionability, diagnostic, and payload contracts | Implemented | Schema, code, tests-run |
| REQ-005 | Swift read store/coordinators/presenters cover runs, stages, approvals, artifacts, reports, daemon, refresh, and subscriptions | Implemented | Code, tests-found |
| REQ-006 | Non-approval write affordances are removed, hidden, or diagnostic-only | Implemented | Code, inventory, guide |
| REQ-007 | Approval-only settlement exception is wired through the P072 GraphQL mutation boundary | Implemented | Code, tests-found |
| REQ-008 | Report metadata and payload-availability behavior avoid bulk payload readback | Implemented | Schema, code, tests-run |
| REQ-009 | Operator write-path guide covers removed controls and replacement status | Implemented | Docs/reference |
| REQ-010 | Phase 0 artifact manifest/source governance is durable and complete | Partially Implemented | Docs/reference |
| REQ-011 | Degraded/freshness/UX/accessibility evidence supports dogfood trust | Partially Implemented | Docs/reference |
| REQ-012 | Phase 3 dogfood, metrics, signoff, and release closeout evidence is complete | Partially Implemented | Docs/reference |
| REQ-013 | Canonical same-tree proposal/readiness gates pass on the audited tree | Not Verifiable | Tests-run |

## Detailed Requirement Audit

### REQ-001 - r19 stopped-scope proposal state and ownership boundary are present

- Proposal source: Status / Boundary status / Goals and non-goals sections.
- Status: **Implemented**.
- Evidence types: `proposal`, `docs`.
- Evidence references: `docs/proposals/031-thin-graphql-ui-rewrite.md`, `docs/reference/ui-action-boundary.md`.
- Implementation mapping: The proposal explicitly narrows P031 to GraphQL thin UI reads plus approval-only mutation stop-state and delegates broader action routing to P072 and polish to P032/P036.
- Gap / note: None for stopped-state scope.

### REQ-002 - Governed UI uses GraphQL-only reads with no MCP/local write fallback and only approval mutations

- Proposal source: Goals, non-goals, static guard requirements, Phase 0b/0c.
- Status: **Implemented**.
- Evidence types: `code`, `config`, `tests-found`.
- Evidence references: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `Chainworks Forge/Views/RunsHomeView.swift`, `docs/reference/p031-thin-ui-inventory.json`, `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`.
- Implementation mapping: P031 request parsing allows query/subscription operations and only the P072 approval mutations. Runs Home bootstraps `P031GraphQLWorkflowReadStore` and does not use local SwiftData `@Query` or direct MCP paths in governed files.
- Gap / note: Runtime UI validation was not obtained.

### REQ-003 - Static UI inventory and guardrails cover governed files and forbidden operations

- Proposal source: Static guard requirements and Phase 0b.
- Status: **Implemented**.
- Evidence types: `config`, `code`, `tests-run`.
- Evidence references: `docs/reference/p031-thin-ui-inventory.json`, `scripts/p031-thin-ui-gate.py`.
- Implementation mapping: The inventory lists governed Swift files, embedded GraphQL operation names, allowed approval mutations, explicit exclusions, and forbidden operation patterns consumed by the gate script.
- Gap / note: The direct gate reached a P041 prerequisite failure and did not produce a successful overall pass on this dirty tree.

### REQ-004 - GraphQL schema exposes P031 projection, freshness, actionability, diagnostic, and payload contracts

- Proposal source: Schema/API commitments and Phase 0c.
- Status: **Implemented**.
- Evidence types: `schema`, `code`, `tests-run`.
- Evidence references: `control-plane/crates/graphql-server/src/types/p031.rs`, `control-plane/crates/graphql-server/src/types/run.rs`, `control-plane/crates/graphql-server/src/types/stage.rs`, `control-plane/crates/graphql-server/src/types/approval.rs`, `control-plane/crates/graphql-server/src/types/artifact.rs`.
- Implementation mapping: Rust GraphQL types expose freshness enums, disabled reasons, write-path state, payload availability, projection presence/update/lag fields, approval diagnostic fields, available actions, and report metadata-only behavior.
- Gap / note: Supplemental P031 GraphQL tests passed, but the canonical P031 wrapper did not reach them because the earlier static gate failed.

### REQ-005 - Swift read store/coordinators/presenters cover governed read surfaces

- Proposal source: In-scope reads, Phase 1, UX/UI notes.
- Status: **Implemented**.
- Evidence types: `code`, `tests-found`.
- Evidence references: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `Chainworks Forge/Views/RunsHomeView.swift`, `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`.
- Implementation mapping: The P031 read store/coordinator covers runs, run detail, stages, stage detail, approval inbox, artifacts, report metadata/payload availability, daemon lifecycle, refresh, and subscriptions.
- Gap / note: No live UI run or screenshot was captured in this audit.

### REQ-006 - Non-approval write affordances are removed, hidden, or diagnostic-only

- Proposal source: Removed/diagnostic writes and non-goals.
- Status: **Implemented**.
- Evidence types: `code`, `config`, `docs`.
- Evidence references: `docs/reference/p031-thin-ui-inventory.json`, `docs/reference/p031-operator-write-path-guide.json`, `Chainworks Forge/Views/RunsHomeView.swift`.
- Implementation mapping: Removed controls such as create/start/cancel/retry/reset/clone/compare/experiment/runtime/session/local recovery are listed in the guide and inventory, while governed UI approval rows expose buttons only when `writePathState == available` and server actions include approve/reject.
- Gap / note: Some guide rows intentionally remain temporarily unavailable or external, which is acceptable for the stopped-state but not final product completeness.

### REQ-007 - Approval-only settlement exception is wired through P072 GraphQL mutation boundary

- Proposal source: Approval-only GraphQL exception and P072 boundary handoff.
- Status: **Implemented**.
- Evidence types: `code`, `tests-found`.
- Evidence references: `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`, `Chainworks Forge/Views/RunsHomeView.swift`, `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`.
- Implementation mapping: The allowed mutation documents are named `P072ApproveApproval` and `P072RejectApproval`; Runs Home wires approval settlement through `P072ApprovalMutationClient`; presenters gate available actions based on server read model state.
- Gap / note: No approval runtime settlement was executed during this audit.

### REQ-008 - Report metadata and payload-availability behavior avoid bulk payload readback

- Proposal source: Report metadata inspection and report payload priority sections.
- Status: **Implemented**.
- Evidence types: `schema`, `code`, `tests-run`.
- Evidence references: `control-plane/crates/graphql-server/src/types/artifact.rs`, `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`.
- Implementation mapping: Report artifacts default to metadata-only/deferred payload state for bulk views; selected artifact payload access is separated and server-owned.
- Gap / note: Full report rendering remains outside P031 stopped-state unless a server-owned query lands.

### REQ-009 - Operator write-path guide covers removed controls and replacement status

- Proposal source: Operator write-path guide requirements.
- Status: **Implemented**.
- Evidence types: `docs`, `config`.
- Evidence references: `docs/reference/p031-operator-write-path-guide.json`.
- Implementation mapping: The guide maps removed controls to external transports, explicit unsupported/pending status, follow-up IDs, and diagnostic guidance. It includes approval and retry rows with external workflow validation.
- Gap / note: This is sufficient for the stopped-state guide contract; it is not equivalent to complete operator write-path availability.

### REQ-010 - Phase 0 artifact manifest/source governance is durable and complete

- Proposal source: Phase 0a/0d manifest and source governance requirements.
- Status: **Partially Implemented**.
- Evidence types: `docs`, `config`.
- Evidence references: `docs/reference/p031-phase-0-artifact-manifest.json`.
- Implementation mapping: The manifest exists, names required artifacts, owners, blocking status, and evidence status.
- Gap / note: Manifest status remains `phase0d_runtime_evidence_attached_phase3_dogfood_signoff_pending`, several entries carry pending/qualified statuses, and per-artifact revision/commit identifiers are not present.

### REQ-011 - Degraded/freshness/UX/accessibility evidence supports dogfood trust

- Proposal source: Phase 0d, UX/UI notes, rollout hold criteria.
- Status: **Partially Implemented**.
- Evidence types: `docs`, `tests-found`.
- Evidence references: `docs/reference/p031-phase-0-artifact-manifest.json`, P031 reference evidence entries listed there.
- Implementation mapping: Evidence artifacts exist for degraded state, freshness copy, screenshot/drill, UX signoff, and accessibility-related limitations.
- Gap / note: Several evidence statuses are explicitly qualified, such as scripted drill or waiver pending, dogfood confirmation pending, and assistive-access limitation.

### REQ-012 - Phase 3 dogfood, metrics, signoff, and release closeout evidence is complete

- Proposal source: Phase 3, rollout metrics, hold criteria.
- Status: **Partially Implemented**.
- Evidence types: `docs`, `tests-found`.
- Evidence references: `docs/reference/p031-phase-0-artifact-manifest.json`, `scripts/test-gate.sh`.
- Implementation mapping: Readiness scaffolding and readiness gate logic exist.
- Gap / note: Dogfood/signoff remains pending or template-backed, and the readiness gate did not pass. This is consistent with the r19 stopped-state handoff but blocks release closeout.

### REQ-013 - Canonical same-tree proposal/readiness gates pass on the audited tree

- Proposal source: Validation, hold criteria, and test gate requirements.
- Status: **Not Verifiable**.
- Evidence types: `tests-run`.
- Evidence references: `scripts/test-gate.sh`, `scripts/p031-thin-ui-gate.py`.
- Implementation mapping: `./scripts/test-gate.sh proposal-031` and `./scripts/test-gate.sh proposal-031-readiness` both run the P043 prerequisite successfully, then fail in the P031 gate on P041 same-tree provenance.
- Gap / note: The failure is concrete: P041 requires a clean live git status snapshot; recorded line count is 0, live line count is 15, and snapshot hashes mismatch. Therefore no successful same-tree canonical gate evidence exists for this audit.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial | Canonical gate/readiness evidence cannot pass on current dirty tree | High |
| Apple UX | Partial | No fresh runtime UI/accessibility validation; Phase 3 signoff remains qualified | Medium |
| Apple architecture | Mostly conformant for stopped-state | Runtime UI behavior not executed | Medium |
| API contract | Conformant for inspected P031 GraphQL schema/tests | Canonical wrapper stops before integrated P031 test step | High |
| Observability/rollout | Not Ready | Manifest/evidence statuses remain pending or qualified | High |
| Execution truth | Not Ready | P041 same-tree parity row no longer matches live status snapshot | High |
| Release readiness | Not Ready | P031 and readiness gates fail | High |

## Routed Specialist Findings

### READY-001 - Canonical P031 gate is blocked by P041 same-tree provenance mismatch

- Reviewer: `chainworks_execution_truth_reviewer`, `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related requirements: REQ-013, REQ-010
- Evidence types: `tests-run`
- Evidence references: `python3 scripts/p031-thin-ui-gate.py --repo-root .`, `./scripts/test-gate.sh proposal-031`, `./scripts/test-gate.sh proposal-031-readiness`
- Why it matters: P031's gate intentionally fails closed when P041 runtime evidence no longer proves the current same-tree state. The current worktree has 15 status lines while P041 recorded a clean status snapshot, so P031 cannot claim validated readiness on this tree even though the P031 feature code appears largely in place.
- Recommended action: Land or otherwise reconcile the current dirty P041 closeout/reference changes, regenerate or update P041 same-tree runtime evidence against the exact clean tree intended for release, then rerun `./scripts/test-gate.sh proposal-031` and `./scripts/test-gate.sh proposal-031-readiness`.
- Acceptance criteria: Both P031 gate aliases pass on the same clean HEAD whose P041 runtime row records matching commit/tree/status provenance.

### READY-002 - Phase 3 dogfood and release evidence remain pending or qualified

- Reviewer: `observability_rollout_reviewer`, `apple_ux_reviewer`
- Severity: Major
- Confidence: High
- Related requirements: REQ-010, REQ-011, REQ-012
- Evidence types: `docs`, `config`
- Evidence references: `docs/reference/p031-phase-0-artifact-manifest.json`
- Why it matters: The proposal allows a stopped-state handoff, but release/readiness requires complete dogfood, freshness, degraded-state, UX, and signoff evidence. The manifest still records pending/qualified status values, so release closeout would overstate the implemented evidence.
- Recommended action: Complete the remaining Phase 3 evidence or keep the work explicitly handed off to P032/P036, then remove release-closeout qualifications only after runtime evidence exists.
- Acceptance criteria: Manifest and referenced evidence artifacts have non-template, non-pending, non-limitation status values, and the readiness gate passes.

### OPS-001 - Phase 0 manifest lacks artifact revision/commit identifiers

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: Medium
- Related requirements: REQ-010
- Evidence types: `config`, `docs`
- Evidence references: `docs/reference/p031-phase-0-artifact-manifest.json`
- Why it matters: P031's governance text asks the manifest to make artifact ownership and revision/source identity durable. The current manifest records paths, owners, and statuses, but not per-artifact revision or commit identifiers, so future audits have weaker provenance for which evidence version was accepted.
- Recommended action: Add revision or commit provenance fields for each manifest artifact when completing P031/P032/P036 closeout.
- Acceptance criteria: The manifest records source identity sufficient to tie each referenced artifact to the accepted implementation tree.

## Readiness Checklist

| Check | Result | Evidence |
| --- | --- | --- |
| Build or canonical gate status | Failed | `./scripts/test-gate.sh proposal-031` failed on P041 same-tree provenance after P043 passed |
| Readiness gate status | Failed | `./scripts/test-gate.sh proposal-031-readiness` failed on the same P041 provenance mismatch |
| Core server/API validation | Passed as supplemental evidence | P031 GraphQL library tests: 11 passed; P031 authorization integration tests: 5 passed |
| Core UI runtime/integration validation | Not run | No local UI smoke; repository policy treats UI smoke as remote-only and it was not requested |
| Empty/loading/error/offline/permission states | Partially verified | Code/tests-found evidence exists; no runtime UI evidence in this audit |
| Accessibility/localization/privacy/permissions/entitlements | Partially verified | No new privacy/entitlement surface found; UX/accessibility signoff remains qualified in manifest evidence |
| Critical tests executed | Partial | P043 prerequisite tests passed in gate; P031 GraphQL tests passed supplementally |
| Full regression or canonical full/proposal gate passed on audited tree | No | Canonical P031 gate failed; successful readiness verdict is prohibited |

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `python3 scripts/p031-thin-ui-gate.py --repo-root .` | Failed | P041 clean live status required; recorded status line count 0 vs live 15; row/detail snapshot hashes mismatch |
| `./scripts/test-gate.sh proposal-031` | Failed | P043 control-plane prerequisite passed 7 tests; P031 gate then failed on P041 same-tree provenance |
| `./scripts/test-gate.sh proposal-031-readiness` | Failed | Repeated proposal-031 path and failed on the same P041 same-tree provenance issue |
| `cd control-plane && cargo test -p graphql-server --lib proposal_031_ && cargo test -p graphql-server --test proposal_031_authorization` | Passed | 11 P031 GraphQL library tests passed; 5 P031 authorization tests passed |
| `git status --short \| wc -l` | Observed 15 | Confirms live dirty status count involved in the P041 provenance failure |

## Final Verdict and Recommended Next Actions

Overall conformance is **Partial**. The implementation substantially satisfies the stopped-state P031 GraphQL-only read boundary, schema/API, Swift read-model, approval exception, report metadata, inventory, and guide requirements. It cannot be marked fully implemented because Phase 0d/Phase 3 evidence remains qualified and the canonical same-tree gates do not pass on the current tree.

Overall readiness is **Not Ready**. The highest-risk blocker is the failed P031/P031-readiness gate caused by P041 same-tree provenance mismatch against a dirty live worktree.

Recommended next actions:

1. Reconcile the current dirty P041 closeout/reference changes into the intended release tree, then regenerate or update P041 runtime evidence on that exact clean tree.
2. Rerun `./scripts/test-gate.sh proposal-031` and `./scripts/test-gate.sh proposal-031-readiness`; do not close P031 as ready until both pass.
3. Complete or explicitly hand off the remaining Phase 3 dogfood, freshness, degraded-state, UX/accessibility, and signoff evidence to P032/P036.
4. Add per-artifact revision/commit provenance to the P031 phase 0 manifest during closeout.
