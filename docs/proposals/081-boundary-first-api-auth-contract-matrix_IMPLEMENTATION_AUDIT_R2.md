# P081 Implementation Audit R2

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Audit date | 2026-05-24 |
| Implementation target | Current dirty worktree |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Branch | `cw/implement-proposal-081-boundar/4dd7c886` |
| HEAD | `21b9376f5e799b6ae9c3c3fbdcf6256931833811` |
| Compare base | Implicit current worktree; no PR/base ref supplied |
| Proposal state | Active, status `revised_for_review_blocker_closure` |
| Overall Conformance | Not Implemented |
| Overall Implementation Readiness | Not Ready |
| Reviewer Selection Reuse | Not reused |
| Audit Confidence | High for Rust/API contract gaps; Medium for macOS runtime behavior because no live UI/hidden-window validation was run |

## Implementation Target

The audit targets the supplied worktree path. The worktree already contained broad implementation edits across Swift, Rust, docs, scripts, and evidence files. This audit added only this report.

The current implementation has improved materially since R1: the P081 gate now validates operator readback evidence, GraphQL WebSocket close-code tests, MCP command-journal idempotency linkage, principal hard-link/parent-mode hardening, Swift redaction decoding, and Swift operator-alert native-delivery model tests.

That progress does not close every explicit P081 acceptance criterion. Several proposal commitments remain only partially implemented or missing.

## Prior Review Reuse

`discover_prior_review.py` returned no proposal-adjacent review artifacts:

```text
artifacts: []
```

Run-local P081 review material exists under `.chainworks/runs/4dd7c886.../reviews/proposal/`, but it was not discovered by the helper for this proposal path. It was treated as contextual evidence only.

Reuse status: `Not reused`.

Selected implementation-audit perspectives:

- `rust_arch_reviewer`: shared BoundaryPolicy ownership, daemon injection, crate boundaries, transaction seams.
- `rust_reliability_reviewer`: idempotency, retry, audit durability, safe-mode and crash/restart behavior.
- `api_contract_reviewer`: GraphQL/MCP contract shape, close codes, redaction envelope, error compatibility.
- `observability_rollout_reviewer`: operator alerts, metrics, canaries, rollout evidence, health/readback.
- `apple_arch_reviewer`: Swift decoding/state ownership, notification delivery, accessibility proof.

Rejected close alternatives:

- `rust_security_reviewer`: security concerns are covered under proposal REQs and API/READY findings; no separate exploit-path audit was run.
- `macos_ui_reviewer`: this audit used static/test evidence only; no live UI or screenshot inspection was performed.
- `product_reviewer`: P081 is primarily a boundary/auth/rollout contract rather than a product-metric decision audit.

## Proposal Contract Summary

P081 defines a boundary-first auth matrix spanning Rust control-plane APIs and the macOS operator shell. The proposal commits to:

- Durable human and machine-readable boundary matrix artifacts with executable validation.
- Server-derived `CallerClass` and boundary-aware principal-table schema v3 while preserving v1/v2 compatibility.
- One daemon-injected immutable `BoundaryPolicy` shared by GraphQL, MCP, and approval actionability.
- Deterministic GraphQL/MCP denial, redaction, WebSocket, and idempotency contracts.
- Durable audit log/checkpoint storage with bounded readback and fail-closed semantics.
- Operator-facing `boundaryRuntime` and `operatorAlerts` readback across GraphQL and MCP.
- macOS-native alert delivery plus accessibility parity for redaction and actionability states.
- Rollout canaries, metrics, reliability tests, safe-mode behavior, and rollback evidence.

Platform/product scope:

- Apple: macOS.
- Backend/service: Rust daemon, GraphQL API, MCP API, SQLite persistence, daemon startup/rollout.
- Cross-stack: Swift client to GraphQL/MCP/readback contract, operator notification lifecycle.

Primary implementation flows audited:

1. Daemon startup validates boundary fixtures, loads principals, constructs one shared policy, and injects it into GraphQL/MCP.
2. GraphQL read/subscription/mutation calls resolve caller class, enforce policy, surface redactions/errors, and preserve the approval-only UI mutation boundary.
3. MCP initialize/tools/list/tools/call enforce policy, hide denied capability inventory, and handle state-changing command idempotency.
4. Approval mutations and Swift approval actions reuse idempotency keys across retry/restart without duplicate settlement.
5. Operator diagnostics and alerts expose bounded runtime/audit state to GraphQL, MCP, and the macOS notification surface.

## Fidelity Inventory

### Matches

- Matrix docs and JSON fixture exist and are validated by `scripts/check-boundary-coverage.sh`.
- Embedded fixture and `auth::boundary::BoundaryPolicy` tests pass.
- Production daemon construction is gated to explicit BoundaryPolicy constructors.
- Principal schema v3, caller-class derivation, hard-link rejection, and parent `0700` checks are present.
- GraphQL WebSocket pre-auth close-code constants/tests now cover `4401`, `4403`, and `4408`.
- `boundaryRuntime`, GraphQL `operatorAlerts`, MCP `runtime.health`, and MCP `operator.alerts.list` bounded readbacks are present.
- Swift decodes `extensions.redactions`, has `P081RedactionState`, and has basic operator-alert native-delivery model tests.
- P081 gate passes on the audited worktree.

### Divergences

- GraphQL observer field-level redaction is explicitly still pending in `require_operator_read`; the code logs that observer redaction is not enforced.
- MCP idempotency is command-journal-linked, but the idempotency preclaim and result update are standalone pool writes outside the command transaction.
- The macOS notification implementation covers Dock badge count, request attention, notification scheduling, dedupe, and clear count, but not status item/MenuBarExtra behavior or hidden/inactive window fires-and-clears runtime proof.
- The Swift accessibility tests cover redacted nil, ordinary nil, and drop_resource metadata, but not actionability_false controls, Full Keyboard Access, Increase Contrast, Reduce Motion, or keyboard-driven approval commands.
- The shadow coverage report exists, but `boundary-policy-canaries.yaml` and its validator were not found.
- P081 metrics named in the proposal were found only in the proposal text, not implemented instrumentation.
- The focused P081 gate does not cover the full reliability matrix requested by the acceptance criteria.

### Ambiguities / Evidence Gaps

- No full `./scripts/test-gate.sh full` run was executed.
- No remote UI smoke, hidden-window alert test, notification authorization proof, status-item proof, VoiceOver, or Full Keyboard Access runtime validation was run.
- Live production shadow observations remain deferred; current evidence is same-tree canary coverage with zero live observations.
- The proposal is very broad. Some Phase 4/5/6 items may be intended as cutover gates rather than code-merge gates, but they are written as acceptance criteria in the audited proposal.

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 8 |
| Partially Implemented | 9 |
| Missing | 2 |
| Not Verifiable | 1 |
| Out of Scope | 0 |

Because in-scope explicit requirements are still `Missing`, Track 1 rolls up to `Overall Conformance = Not Implemented` under the skill status rules.

## Track 1: REQ Audit

| ID | Requirement | Proposal source | Status | Evidence |
| --- | --- | --- | --- | --- |
| REQ-001 | Matrix doc and JSON fixture exist, are linked, and cover all required rows. | Goals lines 32-36; Boundary Matrix lines 178-214; AC lines 994-996 | Implemented | `docs/reference/boundary-first-api-auth-contract.json`; embedded fixture; P081 gate fixture validation passed. |
| REQ-002 | Fixture validator rejects schema/enum/row/field/wildcard/side-effect drift and validates embedded fallback. | Lines 204-213, 439-519, 997 | Implemented | `control-plane/crates/auth/src/boundary/mod.rs`; 38 `boundary::` tests passed. |
| REQ-003 | One immutable daemon-injected `BoundaryPolicy` is shared by GraphQL, MCP, and approval/actionability paths; request paths do not read docs/fixtures. | Lines 349-372, 1001, 1016, 1018 | Implemented | Production constructor guard in `scripts/test-gate.sh:6365`; `daemon/src/main.rs` wiring; GraphQL/MCP constructor evidence. |
| REQ-004 | CallerClass and principal-table v3 preserve v1/v2 compatibility and harden principals.json. | Lines 373-387, 668-679, 730-748, 999-1000, 1009, 1021 | Partially Implemented | `auth/src/lib.rs` implements caller class, v3, hard-link and parent mode checks; packaging has canonical containment. Some wider security acceptance items, including break-glass and audit-log DoS controls, were not verified in this gate. |
| REQ-005 | audit_log and audit_log_checkpoints migrations/repos implement hashing, checkpoint, bounded readback, retention, and fail-closed behavior. | Lines 107-177, 998, 1019-1020 | Partially Implemented | Migrations `064`/`065`; `db::repos::audit_log` tests passed. Audit budget recovery/retention cleanup and every deny-only fail-closed seam were not proven. |
| REQ-006 | GraphQL deterministic errors, WebSocket close codes, and redaction envelope are implemented. | Lines 520-557, 1002 | Partially Implemented | WebSocket close-code tests pass; `add_p081_response_redactions` exists. Observer field redaction without response-level errors is still not implemented, with an explicit pending comment in `control-plane/crates/graphql-server/src/schema.rs:415`. |
| REQ-007 | GraphQL observer read path performs field-level redaction and actionability false. | Lines 67-71, 536-540, 691-719, 1014-1015 | Missing | `require_operator_read` allows/logs observer rows but states field redaction is pending. Swift can decode redactions, but the server does not prove opt-in observer field redaction. |
| REQ-008 | MCP initialize/tools/list/tools/call enforce BoundaryPolicy, omit denied tools, and use `-32004` for known-denied tools. | Lines 558-562, 1003 | Implemented | MCP `initialize` boundary capability tests; tool-call policy denial paths in `mcp-server/src/server.rs`; P081 gate passed. |
| REQ-009 | State-changing MCP commands require idempotency, reject read-only idempotency keys, and replay without duplicate command/domain writes. | Lines 563-570, 1006 | Implemented | `p081_ideas_create_records_command_journal_and_idempotency_linkage` and replay test passed. |
| REQ-010 | State-changing allowed calls commit policy decision, command_journal, idempotency, approval settlement/domain writes, and audit rows atomically in one `BEGIN IMMEDIATE` transaction. | Lines 85-100, 172-177, 1004 | Partially Implemented | Approval path uses transactional machinery; MCP idempotency still uses `insert_pending` and `update_result` standalone pool writes outside the command transaction. |
| REQ-011 | Approval mutations require client idempotency keys, prevent duplicate settlement, and Swift owns retry key persistence. | Lines 75-84, 704, 1005, 1014 | Implemented | Approval idempotency migrations/repo; `P081ApprovalActionAttemptStore`; Swift P081 approval tests passed. |
| REQ-012 | Denial-side-effect tests prove denied calls create no command_journal/settlement/projection writes except declared audit rows. | Lines 95, 175-177, 1007 | Partially Implemented | Several focused denial/no-journal tests exist. No full matrix/caller sweep proving zero projection writes and exactly-one audit row across all denied paths was found. |
| REQ-013 | Operator readback exposes bounded `boundaryRuntime`, `auditLogHealth`, and `operatorAlerts` through GraphQL and MCP. | Lines 653-660, 1013 | Implemented | GraphQL/MCP readback tests passed; operator readback fixture validates in the P081 gate. |
| REQ-014 | Operator alert contract implements severity/dedupe/silence/clear lifecycle, numeric thresholds/windows, and macOS-native delivery with hidden/inactive fires-and-clears tests. | Lines 571-634, 1012 | Partially Implemented | GraphQL/MCP safe-mode and tamper alert payloads exist; Swift `NotificationService` handles Dock badge, attention request, notification, dedupe and clear. Status item/MenuBarExtra, hidden-window fires-and-clears, silence expiry, and threshold-window behavior were not proven. |
| REQ-015 | Runtime reliability tests cover SQLite contention, audit outage, subscription gap, safe-mode exit, SIGTERM drain, committed-unack retry, and denial-audit backpressure. | Lines 661-667, 913-916, 1011 | Partially Implemented | Safe-mode readback and committed-unack-style MCP replay helpers exist. The full named reliability test matrix is not in the P081 gate and was not found as P081-specific coverage. |
| REQ-016 | `boundary-policy-canaries.yaml` has a validator and contributes canary rows to the shadow coverage report. | Lines 657-660, 909-910, 1010 | Partially Implemented | `docs/evidence/boundary-policy-shadow-coverage/report.json` exists and covers all 11 rows via canary flags. No `boundary-policy-canaries.yaml` or validator was found. |
| REQ-017 | Swift/macOS accessibility parity covers redacted nil, ordinary nil, drop_resource, actionability_false, Full Keyboard Access, Increase Contrast, and Reduce Motion. | Lines 691-719, 929-935, 1015 | Partially Implemented | Swift tests cover redaction decoding and accessibility metadata for redacted/ordinary/dropResource. Searches found no actionability_false, Full Keyboard Access, Increase Contrast, Reduce Motion, or keyboard-driven approval proof. |
| REQ-018 | P081 metrics/counters/histograms are implemented and usable for rollout. | Lines 765-780, 960-990 | Missing | Searches found `boundary_policy_decisions_total`, `operator_alert_native_delivery_total`, latency histograms, and related metrics only in proposal text. |
| REQ-019 | Rollout readback fixture and negative fixtures exist and are validated. | Lines 750-856 | Implemented | `docs/evidence/rollout-contract/operator-readback/p081-full-surface.fixture.json`; negative fixtures; P081 gate validates the main readback fixture. |
| REQ-020 | Current-system baseline is preserved: GraphQL remains read/subscription plus approval-only UI mutation, non-approval control stays MCP-owned. | Lines 39-50, 720-729, 1017 | Not Verifiable | Static evidence and tests support the direction, but no full UI/API surface inventory or runtime crawl was run for every non-approval GraphQL mutation path. |

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Not Implemented | Explicit missing observer redaction and metrics/canary artifacts | High |
| Rust architecture | Partial pass | Shared policy is wired, but transaction ownership still splits MCP idempotency from command writes | High |
| Rust reliability | Not Ready | Full reliability acceptance matrix is not covered | Medium |
| API contract | Not Ready | Server-side observer redaction path is not implemented despite GraphQL envelope support | High |
| Observability/rollout | Not Ready | Metrics and full canary artifact/validator are missing | High |
| Apple architecture | Partial pass | Swift model support exists, but native alert/accessibility runtime proof is incomplete | Medium |
| Release readiness | Not Ready | P081 gate passes, but proposal-level acceptance still has major gaps | High |

## Track 2: Routed Specialist Findings

### API-001: Server-side observer field redaction is still pending

Reviewer: `api_contract_reviewer`
Severity: Major
Confidence: High
Related REQs: REQ-006, REQ-007, REQ-017
Evidence types: proposal, code, tests-found, tests-run
Evidence references: proposal lines 67-71, 536-540, 1014-1015; `control-plane/crates/graphql-server/src/schema.rs:403-428`; `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:6-81`

Why it matters: P081 requires observer GraphQL reads to distinguish ordinary nil from redacted nil, include `extensions.redactions`, and apply `drop_resource` behavior. The Swift decoder and metadata tests exist, but the GraphQL guard still states that observer field restrictions are pending. The current server evidence does not prove redacted nullable fields with no response-level error.

Recommended action: Implement actual observer field restrictions in the relevant GraphQL resolvers/read models, emit `extensions.redactions` with `field_null_redacted` where required, and add a server test for observer opt-in field redaction with no response-level GraphQL error.

Acceptance criteria: An observer query returns allowed non-sensitive fields, redacts the sensitive nullable field to null, includes `extensions.redactions` with camelCase metadata, and does not log or expose raw protected data.

### REL-001: MCP idempotency remains outside the command transaction

Reviewer: `rust_reliability_reviewer`
Severity: Major
Confidence: High
Related REQs: REQ-009, REQ-010
Evidence types: proposal, code, tests-found, tests-run
Evidence references: proposal lines 85-100 and 1004; `control-plane/crates/mcp-server/src/server.rs:710-780`; `control-plane/crates/db/src/repos/mcp_command_idempotency.rs:72-135`; tests at `mcp-server/src/server.rs:4063-4170`

Why it matters: The implementation now links MCP idempotency records to `command_journal` and prevents duplicate replay writes. That is useful, but it is not the proposal's atomic commit contract. The preclaim and result update happen as separate pool writes before and after dispatch, while the command write happens inside the engine command path.

Recommended action: Move the MCP idempotency write/update into the same write unit as the command_journal/domain write, or revise the proposal to define the preclaim/sentinel model as the accepted contract and add crash tests for every window.

Acceptance criteria: A state-changing MCP command has one durable transaction boundary for the policy decision, command_journal row, idempotency result, domain write, and required audit rows, or a documented and tested alternative contract that explicitly replaces the proposal's one-transaction requirement.

### OPS-001: Metrics and canary contract are not fully implemented

Reviewer: `observability_rollout_reviewer`
Severity: Major
Confidence: High
Related REQs: REQ-016, REQ-018, REQ-019
Evidence types: proposal, code, config, tests-run
Evidence references: proposal lines 657-660, 765-780, 960-990, 1010; `docs/evidence/boundary-policy-shadow-coverage/report.json`; `scripts/test-gate.sh:6241-6295`

Why it matters: P081's rollout model depends on canary/shadow evidence and named operational metrics. The checked-in shadow report exists and the gate validates it, but it is same-tree canary evidence with zero live observations, no `boundary-policy-canaries.yaml`, and no implementation of the named counters/histograms.

Recommended action: Add `boundary-policy-canaries.yaml` plus validator, wire the named counters/histograms or explicitly revise the proposal to remove them, and add a gate check that fails when these metrics are only documentation.

Acceptance criteria: The repository contains a validated canary source artifact, the shadow coverage report is generated from it or live observations, and at least the decision/error/alert/idempotency metrics named in P081 are emitted by runtime code or proven through a telemetry test hook.

### UI-001: macOS alert and accessibility proof is narrower than the proposal

Reviewer: `apple_arch_reviewer`
Severity: Major
Confidence: Medium
Related REQs: REQ-014, REQ-017
Evidence types: proposal, code, tests-found, tests-run
Evidence references: proposal lines 571-601, 691-719, 1012, 1015; `Chainworks Forge/Engine/NotificationService.swift:115-219`; `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:83-178`

Why it matters: The current Swift code covers decoding, Dock badge contribution, attention request, notification scheduling, dedupe, and clear semantics. The proposal also requires hidden/inactive fires-and-clears tests, status item/MenuBarExtra behavior, silence expiry behavior, Full Keyboard Access/actionability_false controls, Increase Contrast, and Reduce Motion.

Recommended action: Add macOS tests or remote UI evidence for hidden/inactive alert delivery, status item/MenuBarExtra state, silence expiry, actionability_false keyboard behavior, Full Keyboard Access, Increase Contrast, and Reduce Motion.

Acceptance criteria: The proposal-required test names or equivalent evidence are present and passing, including a hidden/inactive main window path and non-color/non-motion accessibility alternatives.

### REL-002: The P081 gate is narrower than the proposal's reliability acceptance criteria

Reviewer: `rust_reliability_reviewer`
Severity: Major
Confidence: Medium
Related REQs: REQ-012, REQ-015
Evidence types: proposal, tests-found, tests-run
Evidence references: proposal lines 661-667, 913-916, 1011; `scripts/test-gate.sh:6230-6391`; `docs/reference/test-gates.md:2173-2218`

Why it matters: The focused gate is valuable, but it does not run SQLite contention, audit outage, subscription gap replay, safe-mode exit, SIGTERM drain, denial-audit backpressure, or full matrix denial-side-effect sweep tests. Passing the gate therefore cannot be treated as full P081 readiness.

Recommended action: Extend `proposal-081` or add a separate closeout gate for the named reliability cases, and record which cases are intentionally deferred if they belong to enforce cutover rather than implementation merge.

Acceptance criteria: Each reliability case from AC line 1011 has a named passing test or an explicit proposal/deferred-scope decision.

### READY-001: Do not close out P081 as fully implemented from the current gate pass

Reviewer: readiness
Severity: Critical
Confidence: High
Related REQs: REQ-007, REQ-010, REQ-014, REQ-015, REQ-016, REQ-017, REQ-018
Evidence types: proposal, code, tests-run
Evidence references: This audit; P081 gate output; proposal acceptance lines 992-1022

Why it matters: The implementation self-assessment now says complete and the focused gate passes, but the audited proposal still contains explicit in-scope requirements that are missing or only partially implemented. Closing out now would convert incomplete behavior into reference truth.

Recommended action: Keep P081 open until the missing/partial requirements are either implemented and proven, or the proposal is explicitly amended to move those items into a later proposal/cutover acceptance gate.

Acceptance criteria: A follow-up audit has no `Missing` REQs and no unresolved Major/Critical routed findings.

## Readiness Checklist

| Area | Status | Notes |
| --- | --- | --- |
| Canonical/focused gate | Passed | `./scripts/test-gate.sh proposal-081` passed on this worktree. |
| Full regression | Not run | No `./scripts/test-gate.sh full` evidence. |
| Core Rust/API flows | Partial pass | Matrix, policy injection, WebSocket close codes, readback, and MCP idempotency replay pass; observer redaction and atomic MCP transaction remain gaps. |
| Runtime/live service validation | Not run | No live daemon startup or subscription/reconnect runtime proof. |
| macOS UI runtime | Not run | No remote UI smoke, hidden-window alert proof, status item proof, or screenshot evidence. |
| Accessibility | Partial | Redaction metadata tests pass; Full Keyboard Access, actionability_false, Increase Contrast, and Reduce Motion proof missing. |
| Privacy/security | Partial | Principal hardening improved; broader security hardening acceptance was not fully verified. |
| Observability/metrics | Not Ready | Operator readback exists; named metrics not implemented. |
| Rollout/canary | Partial | Shadow coverage report exists; no `boundary-policy-canaries.yaml` validator and no live observations. |
| Reliability | Not Ready | Full named reliability matrix not covered. |

## Verification Log

Executed from the audited worktree:

```bash
./scripts/test-gate.sh proposal-081
```

Result: passed.

Latest Swift result bundle:

```text
/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-081-swift-20260524-174514.xcresult
```

Observed covered slices:

- Boundary fixture/doc coverage guardrail.
- Operator readback and shadow coverage fixture validation.
- `cargo test -p auth boundary::`.
- `cargo test -p auth caller_class`.
- `cargo test -p db repos::audit_log::`.
- Principal schema v3, unknown-version, caller-class, hard-link, and parent-mode tests.
- GraphQL `boundaryRuntime`, `operatorAlerts`, WebSocket close-code, and policy reload constant tests.
- MCP `runtime.health`, `operator.alerts.list`, and `ideas.create` idempotency/linkage tests.
- Swift `Proposal081ApprovalActionAttemptStoreTests`.
- Swift `Proposal081GraphQLRedactionTests`.

Not run:

- Full repo gate.
- Remote UI smoke.
- Live daemon/reconnect validation.
- macOS hidden/inactive notification authorization path.
- VoiceOver or Full Keyboard Access runtime validation.
- SQLite contention, audit outage, subscription gap, SIGTERM drain, safe-mode exit, and denial-audit backpressure harnesses.

## Recommended Next Actions

1. Implement server-side observer GraphQL field redaction and add server tests for `extensions.redactions` without response-level errors.
2. Fix or explicitly redesign the MCP idempotency atomic commit contract.
3. Add `boundary-policy-canaries.yaml`, its validator, and real metric instrumentation for the named P081 counters/histograms.
4. Expand macOS evidence for hidden/inactive alert delivery, status item/MenuBarExtra state, silence expiry, Full Keyboard Access, actionability_false, Increase Contrast, and Reduce Motion.
5. Add a P081 closeout reliability gate for the named SQLite/audit/subscription/SIGTERM/backpressure cases, or amend the proposal to defer them clearly.

## Final Verdict

The implementation is substantially closer than R1 and the focused P081 gate passes. However, the current branch still does not satisfy the full proposal text. Track 1 has explicit missing requirements, and Track 2 has unresolved major readiness findings.

Final verdict: `Overall Conformance = Not Implemented`; `Overall Implementation Readiness = Not Ready`.
