# Proposal 081 Implementation Audit R6

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Proposal id / revision | `081` / `081-v6` |
| Proposal state | Active, status `revised_for_review_blocker_closure` |
| Audit report | `docs/proposals/081-boundary-first-api-auth-contract-matrix_IMPLEMENTATION_AUDIT_R6.md` |
| Audit timestamp | 2026-05-25T10:07:51+0300 |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Branch | `cw/implement-proposal-081-boundar/4dd7c886` |
| Current SHA | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| Compare base | `3a93e76332512fc07e8b7bec50882ee83d703c2f` (`git merge-base HEAD origin/main`) |
| Working tree status before report | Dirty: 24 modified implementation/reference/gate files plus pre-existing untracked R4/R5 audit reports |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer selection reuse | Not reused |
| Audit confidence | High for code/gate evidence; medium for live recovery semantics because no daemon runtime scenario was exercised |

## Implementation Target

The audited target is the supplied worktree at commit `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa`, including uncommitted implementation changes present at audit time. The target now includes additional changes after R5 in the boundary fixture/reference, MCP runtime tool registration, GraphQL readback, db metrics, audit budget tests, approval actionability metrics, and the P081 gate.

This R6 audit treats existing implementation-audit reports as historical context only. It writes exactly one new report: this file.

## Prior Proposal-Review Reuse

The prior-review discovery helper returned no proposal-review artifacts for P081. Existing `IMPLEMENTATION_AUDIT` files were ignored for reviewer selection as required by the skill.

Reuse status: Not reused.

## Selected Reviewer Lenses

| Reviewer | Reason selected |
| --- | --- |
| `rust_arch_reviewer` | P081 changes shared Rust policy injection, MCP tools, command journaling, persistence, and audit/readback boundaries. |
| `rust_reliability_reviewer` | P081 explicitly requires idempotency, contention handling, shutdown drain, audit budget recovery, and subscription gap behavior. |
| `api_contract_reviewer` | P081 defines GraphQL/MCP shapes, casing, tool names, error codes, idempotency, redaction, and readback semantics. |
| `observability_rollout_reviewer` | P081 has rollout phases, canaries, shadow coverage, exact metrics, alerts, and cutover hold conditions. |
| `macos_ui_reviewer` | P081 includes Swift operator approval state, native alert behavior, redaction accessibility, keyboard access, contrast, and reduced motion. |

Rejected close alternatives: `rust_security_reviewer` was not separated because remaining security concerns are audit-boundary and service-contract behavior covered by Rust/API/reliability. `apple_arch_reviewer` was not selected because the Swift delta is support-model/test behavior rather than broader app architecture. `product_reviewer` was not selected because the product/rollout commitments are metric and gate contracts covered by observability. `performance_reviewer` was not selected because latency appears only as rollout telemetry, not a benchmark target.

## Proposal State And Contract Summary

P081 remains active. It requires canonical boundary matrix artifacts, trusted server-derived `CallerClass`, shared `BoundaryPolicy` routing for GraphQL/MCP/approval actionability, approval-only SwiftUI mutation behavior, executable denial/redaction/idempotency/audit/readback/alert/rollout semantics, and macOS-native critical alert plus accessibility parity (`docs/proposals/081-boundary-first-api-auth-contract-matrix.md:30-37`).

Explicit non-goals include no new UI write behavior beyond approve/reject, no GraphQL agent control plane, no production debug break-glass endpoint, no broad audit browser, and no local UI smoke requirement in proposal-readiness mode (`:39-50`).

Platform/product scope:

| Scope | Classification |
| --- | --- |
| Apple | macOS |
| Backend/service | Rust service, API, persistence, reliability, rollout/telemetry |
| Cross-stack | GraphQL/MCP boundary contract plus Swift operator shell |

Primary implementation flows:

1. Principal table load derives `CallerClass`, then `BoundaryPolicy` gates GraphQL, MCP, and approval actionability.
2. GraphQL query/subscription/mutation paths return deterministic allow/deny/redact behavior and bounded runtime/alert readback.
3. MCP initialize/tools/list/tools/call exposes boundary capability metadata, hides/denies tools correctly, and supports `boundary.runtime.get` with snake_case readback.
4. State-changing MCP commands and approval mutations use durable idempotency, command-journal stamping, replay, conflict, and committed-unack recovery.
5. Audit log health, operator alerts, Swift redaction/approval state, native alert metadata, and accessibility-mode presentation reach the operator shell.

Leading metric: zero ambiguous-caller warnings, 100 percent matrix validation/citation coverage, zero shadow disagreements before enforce, 100 percent critical alert fires-and-clears tests, and 100 percent accessibility redaction parity coverage (`:982-991`).

Guardrail metric: the exact P081 counter/histogram set (`:766-781`, `:963-977`).

Decision checkpoint: enforce/cutover remains gated on shadow coverage, metrics, reliability, alert, and accessibility evidence.

## Fidelity And Divergence Inventory

### Matches

- `./scripts/test-gate.sh proposal-081` passed on this same worktree.
- R5's MCP casing/tool blocker is addressed: `boundary.runtime.get` is registered, exposed as `boundary_runtime_get` for Codex-compatible tools/list, classified read-only/hot-read, and returns top-level snake_case fields.
- Boundary fixture/reference rows now include `boundary.runtime.get` for observer diagnostics and `boundary.*` for wildcard command callers.
- Required P081 metric names are declared and recordability tests now cover the previously missing names.
- Production call sites now exist for the previously missing metric names, including parity percent, shadow disagreement, evaluation error, approval actionability false, and alert clear latency.
- Audit budget readback now exposes warning/safe-mode/recovery fields and tests warning, 95 percent rate-limit telemetry, cleanup, and half-open-probe readback.
- Swift P081 tests cover concrete keyboard focus, high contrast, reduced motion, alert lifecycle, and approval attempt idempotency.

### Divergences

- Subscription gap behavior is still not implemented as live subscription replay. The implementation exposes `subscriptionReplay` on `boundaryRuntime` and tests a helper, but does not show actual subscription payloads carrying `sequence_cursor` / `projection_generation` or reconnect behavior returning `gap_detected` outside the retention window.
- Audit budget safe mode is readback-derived. `payload_budget_state == read_only_safe_mode` marks `safeModeActive` in diagnostics, but code inspection found this state only in readback/tests, not in `BoundaryPolicy` evaluation or state-changing request denial.
- Some metric emitters are semantically weak: parity percent is recorded as `100` on runtime readback, alert clear latency records `0ms` when no alert is active, and boundary policy evaluation errors are inferred from latency rather than actual evaluation failures.

### Ambiguities / Evidence Gaps

- The proposal requires cleanup progress every 30 seconds; the code records cleanup duration on `delete_old_rows`, but the audit did not find a scheduled 30-second cleanup-progress loop for audit budget recovery.
- The half-open write count is computed from current health state. The tests prove the field changes to `3` after cleanup, but not that three actual half-open audit writes occurred.
- No remote UI smoke was run. This is acceptable for P081 readiness because the proposal explicitly excludes local UI smoke, but it keeps live macOS notification/entitlement confidence below code/test confidence.

## Residual Scope / Follow-Up Ownership

| Residual item | Owner / follow-up | Blocks conformance/readiness |
| --- | --- | --- |
| Live subscription cursor replay and reconnect-outside-window `gap_detected` behavior. | No concrete follow-up proposal found. | Yes |
| Audit-budget safe mode enforcement for audit-required state-changing calls. | No concrete follow-up proposal found. | Yes |
| Audit-budget cleanup cadence and real three-half-open-write exit proof. | No concrete follow-up proposal found. | Yes |
| Semantic rollout metric values for parity percent, alert clear latency, and policy evaluation errors. | No concrete follow-up proposal found. | Yes |
| `storage.reconcile_evidence_orphans` non-dry-run remains fail-closed until it has a P081 atomic MCP write unit. | No concrete follow-up proposal found; current path does not mutate. | Conditional: only blocks if that disabled path is treated as a required successful state-changing MCP command. |

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 23 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed REQ Audit

| Req | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Matrix doc/JSON artifacts and reference linkage. | Implemented | Gate validates boundary fixture/doc presence and required rows. |
| REQ-002 | Executable schema/fixture validation rejects malformed matrix rows. | Implemented | Auth boundary tests plus structured canary validation reject unknown fields, bad enums, duplicate rows, missing rows, wildcard misuse, and invalid grammar. |
| REQ-003 | Required GraphQL/MCP/approval/break-glass matrix rows are covered. | Implemented | Gate validates 11 required rows; fixture/reference now include `boundary.runtime.get`. |
| REQ-004 | Embedded last-known-good fixture and safe-mode startup behavior. | Implemented | Embedded fixture validates; safe-mode policy tests and readback tests pass. |
| REQ-005 | Audit log/checkpoint storage, bounded readback, retention, and fail-closed behavior. | Implemented | `AuditLogHealthSnapshot` includes bounded fields (`control-plane/crates/db/src/repos/audit_log.rs:102-124`), and audit repo tests pass. |
| REQ-006 | Principal table v1/v2/v3 compatibility and strict schema handling. | Implemented | Gate runs bootstrap, unknown-version, caller-class-not-stored, and private principal file tests. |
| REQ-007 | Server-derived `CallerClass` / `CallerContext.caller_class`. | Implemented | CallerClass tests pass; diagnostics use derived caller class. |
| REQ-008 | Shared daemon-injected `BoundaryPolicy` across GraphQL, MCP, and approvals. | Implemented | Gate verifies daemon explicit constructors; GraphQL/MCP/approval paths call policy. |
| REQ-009 | GraphQL deterministic error/redaction/readback contract. | Implemented | GraphQL readback/redaction tests and Swift typed redaction tests pass. |
| REQ-010 | MCP initialize/list/call allow/deny/unknown behavior with capability signal. | Implemented | Initialize advertises `field_casing = snake_case` (`control-plane/crates/mcp-server/src/server.rs:450-461`); known/unknown/read-only/idempotency tests pass. |
| REQ-011 | Durable state-changing MCP idempotency preclaim, canonical hash, and committed-unack recovery. | Implemented | Direct write-unit, canonical hash, replay/conflict, and committed-unack tests pass. |
| REQ-012 | `approveApproval` / `rejectApproval` idempotency and terminal-state transaction handling. | Implemented | Approval duplicate and terminal-conflict behavior is covered by GraphQL/engine tests. |
| REQ-013 | State-changing MCP commands require idempotency; read-only commands reject it. | Implemented | MCP classification includes `boundary.runtime.get` as read-only (`control-plane/crates/mcp-server/src/server.rs:1594-1604`), and state-changing idempotency tests pass. |
| REQ-014 | Denied calls create no side effects except declared audit rows. | Implemented | Boundary validator rejects forbidden deny side effects; GraphQL denial tests assert no command-journal rows. |
| REQ-015 | Boundary guardrail prevents route drift without matrix/fixture/citation touch. | Implemented | `scripts/test-gate.sh proposal-081` runs the boundary coverage guardrail. |
| REQ-016 | Security hardening: strict JSON, token handling, expiry, private principal files, break-glass non-disclosure, audit DoS/tamper evidence. | Implemented | Auth/principal, strict matrix, audit tamper, and audit budget threshold tests pass. |
| REQ-017 | Canary validator contributes rows to shadow coverage schema. | Implemented | Structured validator runs in the gate and validates canary/report agreement. |
| REQ-018 | Reliability runtime covers contention, audit outage/budget, subscription cursor/gap, safe-mode readback/exit, SIGTERM drain, committed-unack, and denial-audit backpressure. | Partially Implemented | Many tests pass, including audit budget readback/recovery and committed-unack. Live subscription replay and audit-budget enforcement/real half-open exit remain incomplete. |
| REQ-019 | Operator alert readback, lifecycle, native delivery, silence/clear, hidden/inactive fires-and-clears tests. | Implemented | GraphQL/MCP alert readback and Swift alert lifecycle tests pass. |
| REQ-020 | `boundaryRuntime` / `audit_log_health` readback across GraphQL and MCP, including MCP snake_case. | Implemented | `boundary.runtime.get` returns snake_case readback (`control-plane/crates/mcp-server/src/tools/runtime.rs:108-126`, `:240-249`) and tests assert it (`control-plane/crates/mcp-server/src/server.rs:3913-3931`). |
| REQ-021 | Swift approval attempt store and typed redaction envelope. | Implemented | Swift gate passes idempotency key reuse/scoping and typed redaction extension tests. |
| REQ-022 | Accessibility parity across redacted nil, ordinary nil, drop_resource, actionability_false, FKA, Increase Contrast, Reduce Motion. | Implemented | Swift gate passes concrete keyboard/contrast/motion policy tests. |
| REQ-023 | Baseline framing preserves GraphQL read/subscription plus approval-only UI boundary. | Implemented | Reference/proposal/gate evidence preserves no additional UI write surface. |
| REQ-024 | Request paths use injected policy rather than reading reference files. | Implemented | Gate verifies daemon injection; inspected request paths use injected policy/readback services. |
| REQ-025 | Exact P081 metrics are emitted/observable with rollout-meaningful semantics. | Partially Implemented | All required names now have helpers/call sites, but some emitters use synthetic values that do not measure the promised rollout signal. |

## Reviewer Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial | REQ-018 and REQ-025 remain partial. | High |
| Rust architecture | Mostly aligned | Audit health drives diagnostics but not policy enforcement for over-budget state-changing calls. | High |
| Rust reliability | Partial | Recovery/gap behavior is helper/readback level, not live replay/enforcement. | High |
| API contract | Aligned | R5 MCP snake_case/tool blocker is closed. | High |
| Observability/rollout | Partial | Some metric values are not rollout-valid measurements. | High |
| macOS UI | Aligned for proposal gate | Model/test coverage is adequate for the proposal's no-local-UI-smoke stance. | Medium |
| Readiness | Not Ready | Major reliability and rollout findings remain. | High |

## Routed Specialist Findings

### REL-001: Reliability recovery is still diagnostic/helper-level, not live behavior

Reviewer: `rust_reliability_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-018  
Evidence types: proposal, code, tests-run

P081 requires live subscription cursor/gap detection and audit budget recovery semantics (`docs/proposals/081-boundary-first-api-auth-contract-matrix.md:661-667`, `:1012`). The implementation now exposes `subscriptionReplay` on `boundaryRuntime` and tests the helper for inside/outside-window gap calculation (`control-plane/crates/graphql-server/src/schema.rs:463-485`, `:7888-7940`). However, this is diagnostic readback/helper behavior, not evidence that actual subscriptions carry `sequence_cursor` and `projection_generation` or that reconnecting outside the retained window returns `gap_detected` to the subscription client.

Audit budget recovery is similarly partial. `health_snapshot` computes `payload_budget_state` and a `half_open_probe_success_count` (`control-plane/crates/db/src/repos/audit_log.rs:826-868`), and tests cleanup/readback (`:973-1051`). But `payload_budget_state` appears only in readback/tests (`rg payload_budget_state`), and `safeModeActive` is derived in diagnostics (`control-plane/crates/graphql-server/src/schema.rs:415-435`, `control-plane/crates/mcp-server/src/tools/runtime.rs:50-80`) rather than feeding request-time `BoundaryPolicy` denial for audit-required state-changing calls.

Why it matters: Operators could see `safeModeActive` in diagnostics while state-changing request paths continue to rely on the injected boundary policy mode, and subscription clients still lack proven replay/gap behavior on real reconnects.

Recommended action: integrate audit-budget state into state-changing GraphQL/MCP request authorization, and add live subscription reconnect tests for retained cursor replay and outside-window `gap_detected` full-refetch signaling.

Acceptance criteria: the P081 gate proves a 95 percent audit budget denies an audit-required state-changing call, cleanup plus three actual half-open writes exits safe mode, and subscription reconnect tests exercise both inside-window replay and outside-window gap detection.

### OPS-001: Several rollout metrics emit synthetic values instead of proposal-valid measurements

Reviewer: `observability_rollout_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-025  
Evidence types: proposal, code, tests-found, tests-run

The missing-name problem is fixed: all required P081 metric names are declared and recordable (`control-plane/crates/db/src/metrics.rs:95-114`, `:520-584`), and production call sites now exist. The remaining problem is semantic. `p081_boundary_policy_enforcement_parity_percent` is recorded as `100` on every boundary runtime readback (`control-plane/crates/graphql-server/src/schema.rs:419`; `control-plane/crates/mcp-server/src/tools/runtime.rs:54`) rather than calculated from shadow/enforce parity. `operator_alert_clear_latency_ms` records `0ms` when no active safe-mode alert is present (`control-plane/crates/graphql-server/src/schema.rs:527`; `control-plane/crates/mcp-server/src/tools/runtime.rs:188-189`), not actual clear latency for an alert id/severity. `boundary_policy_evaluation_error_total` is triggered by evaluation latency over 25 ms (`control-plane/crates/graphql-server/src/schema.rs:667-675`), which is not the same as a policy evaluation error.

Why it matters: P081's rollout and hold conditions rely on these metrics as decision signals. Synthetic or misdefined values can make cutover look healthy without proving the underlying rollout condition.

Recommended action: replace synthetic emissions with real sources: compute parity from shadow coverage/decision comparisons, emit alert clear latency when a specific alert transitions to cleared, and increment evaluation error only on actual evaluation errors.

Acceptance criteria: each P081 metric has a production emission path whose value and labels match the proposal's intended signal, with tests that fail if the metric can be emitted from unrelated readback/no-op paths.

### READY-001: P081 should remain active until reliability and metric semantics are closed

Reviewer: readiness  
Severity: Major  
Confidence: High  
Related requirements: REQ-018, REQ-025  
Evidence types: tests-run, code, proposal

The canonical P081 gate passes on the audited tree, and the R5 API blocker is closed. The remaining partial requirements are still in-scope proposal commitments and have no concrete follow-up proposal owner.

Recommended action: keep P081 in active implementation state until REL-001 and OPS-001 are fixed or the proposal is explicitly narrowed.

Acceptance criteria: all REQ items are `Implemented`, no major findings remain, and `./scripts/test-gate.sh proposal-081` passes again on the same tree.

## Readiness Checklist

| Check | Result |
| --- | --- |
| Canonical P081 gate | Passed: `./scripts/test-gate.sh proposal-081` |
| Full repository regression | Not run; current verdict is Not Ready, and the canonical proposal gate was the relevant audit gate |
| Core GraphQL/MCP boundary tests | Passed in proposal gate |
| MCP `boundary.runtime.get` snake_case contract | Passed in MCP runtime test |
| MCP idempotency/direct write-unit tests | Passed in proposal gate |
| Audit log/budget tests | Passed, including warning/safe-mode and cleanup/half-open readback tests |
| Swift approval/redaction/alert/accessibility tests | Passed 11 Swift tests |
| Accessibility risk | Low to medium; model/test coverage is good, no local UI smoke required by proposal |
| Privacy/security risk | Medium; no open auth/privacy blocker found, but audit-budget enforcement remains a reliability/security-adjacent gap |
| Localization risk | Not materially changed by P081 |
| Permissions/entitlements risk | Medium for live macOS notifications; proposal proof is model/readback based |
| Empty/loading/error/offline UI states | Covered only insofar as disabled actionability and alerts are in P081 scope |
| Operational rollout | Partial due semantic metric issues |

## Verification Log

| Command / inspection | Result |
| --- | --- |
| `git rev-parse HEAD` | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| `git merge-base HEAD origin/main` | `3a93e76332512fc07e8b7bec50882ee83d703c2f` |
| `git status --short --branch` | Dirty worktree with 24 modified files and pre-existing untracked R4/R5 reports before R6. |
| `python3 .../report_path.py .../081-boundary-first-api-auth-contract-matrix.md` | Selected `..._IMPLEMENTATION_AUDIT_R6.md`. |
| `python3 .../discover_prior_review.py .../081-boundary-first-api-auth-contract-matrix.md` | No prior proposal-review artifacts discovered. |
| `git diff --stat` | 24 files changed, 1288 insertions, 101 deletions before R6. |
| `./scripts/test-gate.sh proposal-081` | Passed. Final output: `Proposal 081 boundary-first API/auth gate passed`. |
| Swift result bundle | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-081-swift-20260525-100556.xcresult` |
| MCP contract inspection | Verified `boundary.runtime.get`, `boundary_runtime_get`, read-only classification, and snake_case test assertions. |
| Metrics inspection | Verified all names have helpers/call sites; identified synthetic parity/alert-clear/evaluation-error semantics. |
| Reliability inspection | Verified helper/readback tests; did not find live subscription replay or request-time audit-budget policy enforcement. |

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

The implementation is materially closer than R5. The canonical gate passes, the MCP snake_case runtime contract is implemented, metric coverage is broader, and Swift/accessibility proof remains green. The remaining blockers are narrower but still in-scope: runtime reliability is not yet live enough for subscription replay/audit-budget enforcement, and several rollout metrics do not yet measure the proposal-defined signal.

Recommended next actions:

1. Wire audit-budget state into request-time denial for audit-required state-changing calls and prove safe-mode exit with real half-open writes.
2. Implement live subscription replay/gap behavior, not just runtime readback/helper coverage.
3. Replace synthetic metric emissions with rollout-valid measurements and tests.
4. Re-run `./scripts/test-gate.sh proposal-081` before closeout.
