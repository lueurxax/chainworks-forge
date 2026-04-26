# Proposal 031 Implementation Audit R2

Proposal: `docs/proposals/031-thin-graphql-ui-rewrite.md`
Mode: `auto` via `proposal-implementation-audit`
Generated: `2026-04-24T19:00:53Z`
Repository: `/Users/user/Documents/Chainworks Forge`
Implementation target: current worktree, implicit compare base
HEAD: `8a0d0494e8b2c8bc6ceb21f970532bdde83373b1`
Worktree state: dirty; this audit inspected the current tree and wrote only this report.

## Verdict

Overall conformance: `Not Implemented` as a full proposal.
Implementation readiness: `Not Ready`.
Confidence: `High`.

The core Phase 1 technical slice is materially implemented: the governed SwiftUI read path is now centered on GraphQL query/subscription documents, the P031 static gate passes, the app builds, the P031 GraphQL read-boundary Swift suite passes, the inventory now covers the embedded document owner, report metadata rows expose a fixed trailing payload slot, and the operator write-path guide covers all removed controls with validated approval and retry workflows.

The proposal as a whole is still not implemented because the required Phase 0d and Phase 3 evidence is absent or explicitly blocked: rollback drill or waiver, live freshness p50/p95, runtime UX/accessibility sign-off, dogfood runs, operator notes, degraded-state recovery evidence, and final sign-off are not present. Legacy rollback safety is also not proven in the current tree.

## Reviewer Reuse

Prior current-proposal review artifacts: none discovered for `031-thin-graphql-ui-rewrite.md`.

Direct predecessor review reused for routing context:

- `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/proposal-readiness-review.md`

Reuse validity: `Partially reused`.

Selected implementation reviewers:

| Reviewer | Why selected |
|---|---|
| `macos_ui_reviewer` | Current implementation now has concrete SwiftUI layout, report payload indicators, freshness placement, and accessibility/sign-off obligations. This is a delta from the predecessor review, which rejected UI review because layout details were not yet concrete. |
| `apple_arch_reviewer` | P031 moves Swift UI state ownership to GraphQL read models, presenters, read-refresh state, and presentation-only local state. |
| `api_contract_reviewer` | P031 depends on GraphQL query/subscription contracts, authorization/redaction behavior, enums, and no mutation/read-write fallback. |
| `observability_rollout_reviewer` | P031 has manifest, static gate, rollout, rollback, freshness, dogfood, hold, and sign-off requirements. |
| `chainworks_execution_truth_reviewer` | P031 changes visible run/stage/approval/artifact/report truth ownership and must avoid client-owned workflow truth. |

Predecessor reviewer not carried:

- `apple_ux_reviewer`: dropped under the hard cap because the current concrete UI delta is better covered by `macos_ui_reviewer`, while rollout/user-outcome evidence is covered by `observability_rollout_reviewer`.

Prior `IMPLEMENTATION_AUDIT` reports were ignored for reviewer selection, per the audit skill.

## Verification

Commands run:

| Command | Result | Notes |
|---|---|---|
| `./scripts/test-gate.sh proposal-031` | Passed | Ran the P043 composed gate, P031 static inventory/write-path/manifest gate, P031 GraphQL lib tests, and P031 GraphQL authorization tests. |
| `./scripts/test-gate.sh build` | Passed | macOS app build and bundled control-plane daemon cargo dev build succeeded. Rust build emitted existing warnings in `daemon/src/supervisor.rs` for unused imports. |
| `xcodebuild test -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:"Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=` | Passed | Swift Testing suite `P031 thin GraphQL read boundary` executed 48 tests with 0 failures. The XCTest wrapper also reported 0 selected XCTest tests before Swift Testing ran. |

Not run:

- Full regression gate. This audit has an unsuccessful verdict, so focused/canonical P031 evidence is sufficient.
- Runtime UI screenshot/VoiceOver/visual review.
- Live daemon dogfood run.
- Rollback drill.
- Representative freshness p50/p95 measurement.

## Requirement Conformance

| ID | Proposal commitment | Status | Evidence and judgment |
|---|---|---|---|
| `REQ-001` | Governed macOS workflow UI reads are GraphQL-only over server-owned projections, reconciled with the P043 read contract. | `Implemented` | `./scripts/test-gate.sh proposal-031` passed, including the composed P043 gate. P031 documents and read models are centered in `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`. |
| `REQ-002` | GraphQL fields, enums, report metadata availability, authorization/redaction, and read contracts have tests/gates. | `Implemented` | P031 GraphQL lib and authorization tests passed through the canonical gate; Swift enum and diagnostic presenter tests passed in the 48-test P031 suite. |
| `REQ-003` | Governed SwiftUI must not call MCP, GraphQL mutations, local mutation fallback, recovery services, raw truth files, or command plumbing. | `Implemented` | `scripts/p031-thin-ui-gate.py` passed against `docs/reference/p031-thin-ui-inventory.json`; Swift tests reject mutation documents and mixed mutation/query documents before transport. |
| `REQ-004` | Machine-readable P031 inventory is consumed by a fail-closed static guard and includes new P031 support/read-boundary code. | `Implemented` | `docs/reference/p031-thin-ui-inventory.json` now includes four governed Swift paths, including `P031ThinGraphQLReadBoundary.swift`, plus an `embedded_graphql_documents` entry for `P031GraphQLDocuments`. The static gate validates embedded documents, allowed guard matches, and the Phase 0 manifest. |
| `REQ-005` | Thin read surfaces render Runs Home, run detail, stage detail, approval inbox, artifact/report metadata, daemon lifecycle, and freshness from GraphQL read models. | `Implemented` | `RunsHomeView.swift` and `P031ThinGraphQLReadBoundary.swift` implement the read store/coordinators/presenters. The P031 Swift suite covers run home, run detail, stage detail, approvals, artifacts, reports, daemon lifecycle, subscriptions, refresh, and fail-closed behavior. Runtime UX sign-off remains separate under `REQ-010`. |
| `REQ-006` | Approval handling is diagnostic/read-only: no in-app approve/reject path, with copyable identifiers and external workflow guidance. | `Implemented` | The operator guide validates `approvals.resolve` as an external MCP workflow. Swift tests cover approval diagnostics, copy items, CLI availability only when guide rows validate it, and no UI execution for incomplete/unavailable rows. |
| `REQ-007` | Reports stay metadata-only unless the separate payload query lands; payload indicators must be clear and layout-stable. | `Implemented` | `RunsHomeView.swift` reserves a fixed trailing payload status slot via `payloadIndicatorSlotWidth`, and Swift tests cover metadata-only report rows and blocked payload opening. `docs/evidence/p031-report-payload-priority-decision.md` records the proposal default to keep report payload follow-up at P0. |
| `REQ-008` | Before dogfood, every removed write control is represented in an operator write-path guide row, and at least one approval diagnostic plus one non-approval removed-control workflow are validated. | `Implemented` | `docs/reference/p031-operator-write-path-guide.json` maps 13 removed controls. `approvals.resolve` and `stages.retry` are validated; the remaining rows are explicitly `temporarily_unavailable` with follow-up IDs. This satisfies the proposal's minimum validation gate, but leaves operator viability gaps for dogfood sign-off. |
| `REQ-009` | Phase 0 artifact manifest exists, links required artifacts, and is consumed by the P031 gate. | `Implemented` | `docs/reference/p031-phase-0-artifact-manifest.json` exists with required entries plus freshness and UX sign-off entries. `scripts/p031-thin-ui-gate.py` validates manifest schema, entries, paths, and does not allow a `ready` manifest entry to point at blocking evidence. |
| `REQ-010` | Phase 0d exit evidence is attached: operator guide, UX/accessibility sign-off, freshness p50/p95, rollback drill or waiver, and report payload priority decision. | `Partially Implemented` | Operator guide and report payload priority are ready. `docs/evidence/p031-rollback-drill.md`, `docs/evidence/p031-freshness-baseline.md`, and `docs/evidence/p031-ux-accessibility-signoff.md` are all `Status: BLOCKED`; no rollback drill/waiver, no p50/p95 measurement, and no runtime/VoiceOver/visual sign-off are present. |
| `REQ-011` | Legacy rollback path remains available behind legacy mode until critical write-path readiness or a dated waiver permits removal. | `Missing` | `docs/reference/p031-thin-ui-inventory.json` has an empty `legacy_only_files` list, application code search did not find a `CHAINWORKS_THIN_UI_MODE=legacy` runtime path, and rollback evidence says no drill or waiver exists. The worktree also deletes multiple legacy workflow UI files, so rollback preservation/removal safety is not proven. |
| `REQ-012` | Phase 3 dogfood includes two `full-mvp-live` runs, operator outcome notes, degraded-state recovery, approval diagnostic comprehension, targeted refresh/report indicators/accessibility/projection/freshness/rollback evidence, and sign-off. | `Missing` | `docs/evidence/p031-dogfood-signoff.md` is a template only. It explicitly says it is not dogfood completion evidence and remains unsigned. |
| `REQ-013` | Implementation approval remains blocked until aggregate re-review evaluates the corrected GraphQL-only scope. | `Not Verifiable` | The proposal itself states implementation approval is rejected/stale until review. This audit can provide evidence for re-review, but does not itself prove that aggregate approval has happened. |

## Specialist Findings

### `READY-001` - Phase 0d exit evidence is explicitly blocked

Severity: Critical
Reviewers: `observability_rollout_reviewer`, `macos_ui_reviewer`
Evidence: `docs/reference/p031-phase-0-artifact-manifest.json`, `docs/evidence/p031-rollback-drill.md`, `docs/evidence/p031-freshness-baseline.md`, `docs/evidence/p031-ux-accessibility-signoff.md`

The manifest correctly exposes Phase 0d evidence as pending, but the required evidence is not present. Rollback is `BLOCKED`, freshness is `BLOCKED`, and UX/accessibility sign-off is `BLOCKED`. This blocks Phase 0d exit, dogfood start, and implementation readiness even though the P031 static/code gates pass.

Required action: run or waive the rollback drill, measure representative GraphQL freshness p50/p95 and targeted refresh behavior, and attach runtime UI/accessibility sign-off evidence.

### `OPS-001` - Legacy rollback safety is not proven after broad legacy surface removal

Severity: Critical
Reviewers: `observability_rollout_reviewer`, `chainworks_execution_truth_reviewer`, `macos_ui_reviewer`
Evidence: `docs/proposals/031-thin-graphql-ui-rewrite.md`, `docs/reference/p031-thin-ui-inventory.json`, `docs/evidence/p031-rollback-drill.md`, current worktree status

P031 requires legacy rollback to remain available behind `CHAINWORKS_THIN_UI_MODE=legacy` until critical write-path readiness or a dated waiver permits removal. In the current tree, `legacy_only_files` is empty, app code search found no runtime legacy-mode switch, rollback evidence is blocked, and many legacy workflow UI files are deleted. The audit cannot prove that visible legacy Runs Home/Run Detail can be restored within 60 seconds, or that rollback was not effectively removed before the proposal's safety condition.

Required action: either restore and inventory a legacy-mode rollback path and pass the rollback drill, or attach the dated release-owner waiver required by the proposal before treating the migration as ready.

### `READY-002` - Phase 3 dogfood and release handoff evidence are absent

Severity: Critical
Reviewers: `observability_rollout_reviewer`, `chainworks_execution_truth_reviewer`
Evidence: `docs/evidence/p031-dogfood-signoff.md`

The dogfood artifact is a ready template, not completion evidence. It contains no two `full-mvp-live` runs, no operator workflow-completion notes, no degraded-state recovery evidence, no approval diagnostic comprehension evidence, no projection correctness/freshness results, and no sign-off. This keeps the proposal out of closeout even with passing build and P031 gates.

Required action: complete the Phase 3 checklist with run-specific evidence after Phase 0d blockers are cleared.

### `READY-003` - The green P031 gate is not a dogfood/readiness gate

Severity: Major
Reviewers: `observability_rollout_reviewer`
Evidence: `scripts/test-gate.sh`, `scripts/p031-thin-ui-gate.py`, `docs/reference/p031-phase-0-artifact-manifest.json`

`./scripts/test-gate.sh proposal-031` is a strong contract/static/read-boundary gate, and it now correctly passes while the manifest still says Phase 0d and Phase 3 evidence are pending. That behavior is acceptable for a Phase 1 implementation gate, but it is unsafe to interpret the green gate as dogfood or release readiness.

Required action: keep closeout criteria tied to the manifest evidence statuses, or add an explicit P031 dogfood/readiness gate that fails while Phase 0d or Phase 3 evidence remains pending.

## Non-Blocking Observations

- The P031 inventory/gate gap from the earlier implementation state is materially improved: embedded GraphQL documents are now represented, and the static gate validates that `P031GraphQLDocuments` is inventoried when present in governed Swift.
- The report metadata layout issue is materially improved: the view reserves the expected trailing payload indicator slot and truncates row text.
- The operator write-path guide is honest about remaining unavailable write paths. That is acceptable before dogfood only because the proposal's minimum validated workflows are present; it is not proof that operators can complete every workflow without workaround.
- The current audit was performed against a broad dirty worktree with other proposal/reference changes present. That does not invalidate the focused P031 evidence, but a final merge/closeout review should use a clean PR or commit range.

## Readiness Checklist

| Gate or evidence | Status |
|---|---|
| P031 canonical gate | Passed |
| App/control-plane build | Passed |
| P031 Swift GraphQL read-boundary tests | Passed, 48 tests |
| Full regression gate | Not run |
| Runtime UI/VoiceOver sign-off | Blocked/missing |
| Freshness p50/p95 measurement | Blocked/missing |
| Rollback drill or waiver | Blocked/missing |
| Two-run dogfood evidence | Missing |
| Phase 3 sign-off | Missing |
| Legacy rollback removal readiness or waiver | Missing/not proven |

## Recommended Next Actions

1. Clear Phase 0d blockers: rollback drill or waiver, freshness baseline, and UX/accessibility sign-off.
2. Prove or restore legacy rollback mode before relying on any legacy surface deletion.
3. Run Phase 3 dogfood after Phase 0d is green and attach run-specific evidence to `docs/evidence/p031-dogfood-signoff.md`.
4. Add an explicit dogfood/readiness gate or make closeout automation fail when the P031 manifest has pending Phase 0d/Phase 3 evidence.
5. Re-run this implementation audit after the evidence is attached and the worktree is narrowed to the P031 implementation diff.
