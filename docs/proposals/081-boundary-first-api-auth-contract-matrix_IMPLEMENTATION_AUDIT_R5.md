# Proposal 081 Implementation Audit R5

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Proposal id / revision | `081` / `081-v6` |
| Proposal state | Active, status `revised_for_review_blocker_closure` |
| Audit report | `docs/proposals/081-boundary-first-api-auth-contract-matrix_IMPLEMENTATION_AUDIT_R5.md` |
| Audit timestamp | 2026-05-25T09:04:58+0300 |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Branch | `cw/implement-proposal-081-boundar/4dd7c886` |
| Current SHA | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| Compare base | `3a93e76332512fc07e8b7bec50882ee83d703c2f` (`git merge-base HEAD origin/main`) |
| Working tree status before report | Dirty: 17 modified implementation/gate files plus pre-existing untracked R4 audit report |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready |
| Reviewer selection reuse | Not reused |
| Audit confidence | High for Rust/API/metric source evidence and canonical gate result; medium for live recovery/UI runtime behavior |

## Implementation Target

The audited target is the current worktree at commit `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa`, including uncommitted implementation changes present in the supplied worktree. This R5 audit treats the existing R4 implementation audit as a pre-existing artifact and writes exactly one new report: this file.

The implementation changed Rust auth, db metrics/audit-log repos, engine command handling, GraphQL/MCP runtime readback, MCP idempotency write units, Swift P081 support/tests, and `scripts/test-gate.sh`.

## Prior Proposal-Review Reuse

The prior-review discovery helper found no proposal-review artifacts for P081. Existing `IMPLEMENTATION_AUDIT` files were not reused for reviewer selection, per the audit skill's selection rule.

Reuse status: Not reused.

## Selected Reviewer Lenses

| Reviewer | Reason selected |
| --- | --- |
| `rust_arch_reviewer` | Shared Rust boundary policy, command journal, MCP write units, audit log, and daemon/API injection are core to P081. |
| `rust_reliability_reviewer` | P081 explicitly requires idempotency, committed-unack recovery, SQLite contention, audit budget recovery, policy reload, and subscription gap behavior. |
| `api_contract_reviewer` | P081 defines GraphQL/MCP shape, casing, tool names, error codes, redaction, readback, and idempotency contracts. |
| `observability_rollout_reviewer` | P081 contains rollout phases, canaries, shadow coverage, required metrics, alerting, readback, and hold conditions. |
| `macos_ui_reviewer` | P081 includes Swift operator approval behavior, redaction accessibility, native alerts, keyboard behavior, and macOS accessibility modes. |

Rejected close alternatives: `rust_security_reviewer` was covered by the Rust/API lenses because remaining security-related evidence is auth-boundary and audit-DoS behavior. `apple_arch_reviewer` was not selected because the Swift changes are support-model and test changes rather than architectural ownership changes. `product_reviewer` was not selected because P081 is a contract/readiness proposal; rollout metrics are handled by the observability reviewer. `performance_reviewer` was not selected because the latency commitments are rollout/telemetry checks, not benchmarked performance claims.

## Proposal State And Contract Summary

P081 is active. Its goals require canonical machine/human boundary matrix artifacts, server-derived `CallerClass`, shared `BoundaryPolicy` routing for GraphQL/MCP/approval actionability, an approval-only Swift operator app, executable denial/redaction/idempotency/audit/readback/alert/rollout semantics, and macOS-native critical alerts plus accessibility parity (`docs/proposals/081-boundary-first-api-auth-contract-matrix.md:30-37`).

Explicit non-goals include no additional UI write behavior beyond approve/reject, no GraphQL agent control plane, no production developer break-glass endpoint, no local UI smoke requirement in proposal-readiness mode, and no broad audit-log browser (`:39-50`).

Platform/product scope:

| Scope | Classification |
| --- | --- |
| Apple | macOS |
| Backend/service | Rust service, API, persistence, runtime reliability, rollout/telemetry |
| Cross-stack | GraphQL/MCP boundary contract plus Swift operator shell |

Primary implementation flows:

1. Principal table load derives trusted caller class, then shared `BoundaryPolicy` evaluates GraphQL, MCP, and approval actionability.
2. GraphQL query/subscription/mutation paths return deterministic allow/deny/redact behavior and bounded runtime/alert readback.
3. MCP initialize/tools/list/tools/call expose boundary capabilities, hide/deny tools correctly, and enforce idempotency for state-changing calls.
4. Audit log append/checkpoint/readback, audit budget, safe-mode, and operator alerts provide operational visibility and fail-closed behavior.
5. Swift approval/redaction/alert models preserve idempotency keys, typed redaction, disabled actionability, native alert state, and accessibility-mode presentation.

Leading metric: zero ambiguous-caller warnings during phase 3, 100 percent required matrix validation/citation coverage, 100 percent critical alert fires-and-clears tests before phase 4, and 100 percent accessibility redaction parity coverage before phase 5 (`:982-991`).

Guardrail metric: the required P081 counter/histogram set in the proposal metrics section (`:766-781`, `:963-977`).

Decision checkpoint: enforce/cutover remains blocked until shadow coverage, metric, runtime reliability, and macOS alert/accessibility gates meet the proposal's phase and hold conditions.

## Fidelity And Divergence Inventory

### Matches

- The current tree passes `./scripts/test-gate.sh proposal-081`.
- Structured canary validation is wired and checks canary/report row agreement, required rows, redaction proof, unknown fields, duplicate keys, and disagreements.
- GraphQL `boundaryRuntime` now exposes richer audit health fields, including writability, last write timestamp, failure counters, budget/usage, cleanup state, integrity, and shadow coverage reference.
- MCP `runtime.health` now includes the same bounded boundary runtime object, though with a casing/tool-name divergence from the active proposal.
- Audit budget readback now reports warning/read-only-safe-mode states and emits `audit_log_rate_limited_total` at the 95 percent threshold.
- New production metric call sites cover ambiguous caller warnings, no-op labels, audit rate limiting, native alert delivery availability, duplicate approval idempotency, and approval commit transaction latency.
- Direct MCP write units now claim pending idempotency inside durable command transactions for storage/projection/effects paths, and nested JSON canonicalization is tested.
- Swift P081 tests now include concrete keyboard focus, high contrast, and reduced motion policy behavior.

### Divergences

- The active proposal says MCP `initialize.boundary_policy` and `tools/call boundary.runtime.get` return the same runtime fields in `snake_case` (`:656`). The implementation advertises `field_casing = "snake_case"` during initialize but returns `runtime.health.boundaryRuntime` with camelCase fields.
- Several required rollout metrics are still declaration/test-only or have no production emission path in code search, including `operator_alert_clear_latency_ms`, `p081_boundary_policy_enforcement_parity_percent`, `boundary_policy_shadow_disagreement_total`, `boundary_policy_evaluation_error_total`, and `approval_actionability_false_total`.
- Subscription gap coverage is readback-only: `sequenceCursor`, `projectionGeneration`, and `gapDetected=false` are exposed as static diagnostic fields, but reconnect outside the retained cursor window and `gap_detected` behavior are not implemented/proven.
- Audit budget recovery now detects thresholds, but the full proposal behavior for cleanup every 30 seconds, exit below 80 percent, and three successful half-open audit writes is not proven.

### Ambiguities / Evidence Gaps

- `docs/reference/boundary-first-api-auth-contract.md` currently describes MCP `runtime.health` with `boundaryRuntime`, while the active proposal names `boundary.runtime.get` and `snake_case`. Because the proposal remains active, this audit treats the proposal as the controlling contract.
- macOS evidence is strong at support-model/test level. No remote UI smoke was run, and the proposal explicitly says local UI smoke is not required in proposal-readiness mode.
- The proposal gate still contains a reliability proof inventory token check; some reliability behavior is now directly tested, but cursor gap/exit scenarios remain token/readback based.

## Residual Scope / Follow-Up Ownership

| Residual item | Owner / follow-up | Blocks conformance/readiness |
| --- | --- | --- |
| Align MCP runtime diagnostic tool name and casing with the active proposal, or update the proposal to match `runtime.health.boundaryRuntime`. | No concrete follow-up proposal found. | Yes |
| Add production emission for every exact P081 metric, especially alert clear latency, parity percent, shadow disagreement, evaluation error, and approval actionability false. | No concrete follow-up proposal found. | Yes |
| Implement/prove subscription replay cursor retention and reconnect-outside-window `gap_detected` behavior. | No concrete follow-up proposal found. | Yes |
| Complete audit budget recovery semantics: bounded cleanup cadence, exit below 80 percent, and three successful half-open writes. | No concrete follow-up proposal found. | Yes |
| `storage.reconcile_evidence_orphans` non-dry-run remains fail-closed until it has a P081 atomic MCP write unit. | No concrete follow-up proposal found; current path is disabled and does not mutate. | Conditional, only if treated as a required successful state-changing MCP path |

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 22 |
| Partially Implemented | 3 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed REQ Audit

| Req | Requirement | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Matrix doc/JSON artifacts and reference linkage. | Implemented | Proposal gate validates boundary fixture/doc presence and required rows. |
| REQ-002 | Executable schema/fixture validation rejects malformed matrix rows. | Implemented | `scripts/validate-p081-canaries.py` plus auth boundary tests reject unknown fields, bad enums, duplicate rows, missing rows, and invalid grammar. |
| REQ-003 | Required GraphQL/MCP/approval/break-glass matrix rows are covered. | Implemented | Gate validates 11 required rows and canary/shadow coverage. |
| REQ-004 | Embedded last-known-good fixture and safe-mode startup behavior. | Implemented | Gate and readback prove policy injection, safe-mode fields, and fixture digest exposure. |
| REQ-005 | Audit log/checkpoint storage, bounded readback, retention, and fail-closed behavior. | Implemented | `AuditLogHealthSnapshot` includes bounded health fields (`control-plane/crates/db/src/repos/audit_log.rs:102-124`) and readback tests assert no raw rows. |
| REQ-006 | Principal table v1/v2/v3 compatibility and strict schema handling. | Implemented | Auth tests in the proposal gate cover bootstrap, unknown version rejection, caller class not stored, and private principal file constraints. |
| REQ-007 | Server-derived `CallerClass` / `CallerContext.caller_class`. | Implemented | Auth caller-class tests pass; GraphQL now records ambiguous caller diagnostics from derived caller class (`control-plane/crates/graphql-server/src/schema.rs:234-246`). |
| REQ-008 | Shared daemon-injected `BoundaryPolicy` across GraphQL, MCP, and approvals. | Implemented | Gate verifies production daemon constructor injection and policy-readback behavior. |
| REQ-009 | GraphQL deterministic error/redaction/readback contract. | Implemented | GraphQL readback and Swift redaction decoder tests pass in `proposal-081` gate. |
| REQ-010 | MCP initialize/list/call allow/deny/unknown behavior. | Implemented | Initialize advertises boundary policy metadata (`control-plane/crates/mcp-server/src/server.rs:449-460`); MCP denied/unknown/list/idempotency tests pass. |
| REQ-011 | Durable state-changing MCP idempotency preclaim, canonical hash, and committed-unack recovery. | Implemented | Nested hash, direct storage/effects write-unit claims, replay/conflict, and committed-unack tests pass. |
| REQ-012 | `approveApproval` / `rejectApproval` idempotency and terminal-state transaction handling. | Implemented | Engine records duplicate approval metric and returns cached/conflict semantics without double settlement (`control-plane/crates/engine/src/command_handler.rs:2052-2078`). |
| REQ-013 | State-changing MCP commands require idempotency; read-only commands reject it. | Implemented | MCP classification and precheck paths are present; gate passes state-changing/read-only idempotency tests. |
| REQ-014 | Denied calls create no side effects except declared audit rows. | Implemented | Boundary validator rejects forbidden deny side effects; GraphQL tests assert no command-journal rows for denied/terminal attempts (`control-plane/crates/graphql-server/src/schema.rs:8080-8115`, `:8261-8307`). |
| REQ-015 | Boundary guardrail prevents route drift without matrix/fixture/citation touch. | Implemented | `scripts/test-gate.sh` runs boundary coverage guardrail in the proposal gate. |
| REQ-016 | Security hardening: strict JSON, token handling, expiry, private principal files, break-glass non-disclosure, audit DoS/tamper evidence. | Implemented | Auth/principal tests, strict matrix tests, audit tamper tests, and audit budget threshold tests pass. |
| REQ-017 | Canary validator contributes rows to shadow coverage schema. | Implemented | Structured validator is invoked by the gate and validates canary/report agreement. |
| REQ-018 | Reliability runtime covers contention, audit outage/budget, subscription cursor/gap, safe-mode readback/exit, SIGTERM drain, committed-unack, and denial-audit backpressure. | Partially Implemented | Audit budget threshold and committed-unack tests pass, but subscription gap behavior and full audit-budget cleanup/exit semantics remain unproven. |
| REQ-019 | Operator alert readback, lifecycle, native delivery, silence/clear, hidden/inactive fires-and-clears tests. | Implemented | GraphQL/MCP alert readback tests pass; Swift hidden-window/native surface tests pass (`Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:204-303`). |
| REQ-020 | `boundaryRuntime` / `audit_log_health` readback fields across GraphQL and MCP. | Partially Implemented | Field coverage is strong, but MCP runtime readback uses camelCase `boundaryRuntime` fields despite the active proposal's `snake_case` MCP requirement (`docs/proposals/...:655-656`; `control-plane/crates/mcp-server/src/tools/runtime.rs:42-86`). |
| REQ-021 | Swift approval attempt store and typed redaction envelope. | Implemented | Swift tests cover idempotency key reuse/scoping and typed redaction extension decoding. |
| REQ-022 | Accessibility parity across redacted nil, ordinary nil, drop_resource, actionability_false, FKA, Increase Contrast, Reduce Motion. | Implemented | Tests cover redaction/drop-resource metadata and concrete keyboard/contrast/motion policy behavior (`Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:334-375`). |
| REQ-023 | Baseline framing preserves GraphQL read/subscription plus approval-only UI boundary. | Implemented | Proposal/reference/gate checks preserve no extra UI write surface. |
| REQ-024 | Request paths use injected policy rather than reading reference files. | Implemented | Gate verifies daemon injection; runtime paths inspect policy/readback state rather than reading docs/fixtures per request. |
| REQ-025 | Exact P081 metrics are emitted/observable for rollout. | Partially Implemented | New emitters close many gaps, but required metrics still lack production emission call sites; see OPS-001. |

## Reviewer Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial | Three in-scope REQs remain partial. | High |
| Rust architecture | Mostly aligned | Disabled storage orphan write path is fail-closed until an atomic write unit exists. | High |
| Rust reliability | Partial | Cursor replay/gap and audit-budget exit semantics are not executable end-to-end. | High |
| API contract | Partial | MCP diagnostic casing/tool-name drift from active proposal. | High |
| Observability/rollout | Partial | Required rollout metrics are not all emitted from production paths. | High |
| macOS UI | Aligned for proposal gate | Evidence is model/test-level, but local UI smoke is explicitly non-required. | Medium |
| Readiness | Not Ready | Major API, reliability, and rollout findings remain. | High |

## Routed Specialist Findings

### API-001: MCP runtime readback contradicts the active proposal casing/tool contract

Reviewer: `api_contract_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-010, REQ-020  
Evidence types: proposal, code, tests-run

The active proposal says MCP `initialize.boundary_policy` and `tools/call boundary.runtime.get` return the same runtime fields in `snake_case` (`docs/proposals/081-boundary-first-api-auth-contract-matrix.md:655-656`). The implementation advertises `field_casing = "snake_case"` during initialize (`control-plane/crates/mcp-server/src/server.rs:449-460`), but the actual MCP diagnostic readback is `runtime.health.boundaryRuntime` with camelCase fields such as `schemaVersion`, `safeModeActive`, `auditLogHealth`, and `shadowCoverageReportRef` (`control-plane/crates/mcp-server/src/tools/runtime.rs:42-86`). The MCP tests assert the camelCase shape (`control-plane/crates/mcp-server/src/server.rs:3790-3847`).

Why it matters: MCP clients cannot rely on the active proposal's published contract if initialize advertises snake_case while the tool response uses camelCase and the proposal-named tool differs from the implemented tool.

Recommended action: either implement the proposal contract with `boundary.runtime.get` and snake_case payloads, or explicitly revise P081 to bless `runtime.health.boundaryRuntime` camelCase before closeout.

Acceptance criteria: an MCP contract test proves initialize metadata, tools/list name/alias, and tools/call response casing all match the active proposal or an updated proposal.

### OPS-001: Required rollout metrics are still partially declaration-only

Reviewer: `observability_rollout_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-025  
Evidence types: proposal, code, tests-found, tests-run

The metric list is now much closer: `control-plane/crates/db/src/metrics.rs:95-114` declares the required names, and new production call sites cover ambiguous callers, no-op labels, audit rate limiting, native alert delivery, approval duplicate idempotency, and approval commit transaction latency. However, repository search still found no production call sites for `operator_alert_clear_latency_ms`, `p081_boundary_policy_enforcement_parity_percent`, `boundary_policy_shadow_disagreement_total`, `boundary_policy_evaluation_error_total`, and `approval_actionability_false_total`; these names appear only in declarations/tests or not as live emitters.

Why it matters: P081 rollout and hold conditions rely on exact signals for enforce/cutover decisions. A declared metric that never emits cannot be used as a rollout guardrail.

Recommended action: wire live emitters for every exact required metric, or revise the active proposal to remove/reclassify metrics that are intentionally evidence-only.

Acceptance criteria: code search and tests show each required P081 counter/histogram is emitted from at least one production path or intentionally backed by a concrete fixture/report contract named in the proposal.

### REL-001: Reliability proof still does not exercise subscription gap or full audit-budget recovery behavior

Reviewer: `rust_reliability_reviewer`  
Severity: Major  
Confidence: High  
Related requirements: REQ-018  
Evidence types: proposal, code, tests-run

P081 requires subscription cursor/gap detection and audit budget recovery semantics (`docs/proposals/081-boundary-first-api-auth-contract-matrix.md:661-667`, `:1012`). The current implementation exposes `subscriptionReplay` readback with `sequenceCursor = "live-tail"`, `projectionGeneration = 0`, and `gapDetected = false` in GraphQL/MCP (`control-plane/crates/graphql-server/src/schema.rs:434-440`; `control-plane/crates/mcp-server/src/tools/runtime.rs:56-63`), and tests only assert those static readback fields (`control-plane/crates/graphql-server/src/schema.rs:7817-7847`). Audit budget thresholds now report warning/read-only-safe-mode and emit rate-limit telemetry (`control-plane/crates/db/src/repos/audit_log.rs:825-864`, `:960-995`), but cleanup cadence, exit below 80 percent, and three half-open successful writes are not proven.

Why it matters: Operators need real recovery behavior, not just diagnostic fields, when reconnecting outside the replay window or recovering from audit pressure.

Recommended action: add integration/scenario tests for retained cursor replay, outside-window gap detection/full refetch signaling, cleanup progress cadence, and safe-mode exit after three half-open writes.

Acceptance criteria: the proposal gate exercises at least one reconnect-inside-window success, one reconnect-outside-window `gap_detected` response, and one audit-budget recovery sequence from warning to safe-mode to exit.

### READY-001: P081 should not be closed out as fully implemented yet

Reviewer: readiness  
Severity: Major  
Confidence: High  
Related requirements: REQ-018, REQ-020, REQ-025  
Evidence types: tests-run, code, proposal

`./scripts/test-gate.sh proposal-081` passed on the audited tree, including the new audit-budget, subscription readback, metric recordability, and Swift accessibility policy tests. The gate result is necessary but not sufficient because the active proposal still has in-scope requirements that are only partially implemented.

Recommended action: keep P081 active until API-001, OPS-001, and REL-001 are resolved or the proposal is explicitly revised to narrow those commitments.

Acceptance criteria: all REQs are `Implemented`, major findings are resolved or formally descoped by proposal update, and the same-tree canonical gate passes again.

## Readiness Checklist

| Check | Result |
| --- | --- |
| Canonical P081 gate | Passed: `./scripts/test-gate.sh proposal-081` |
| Full repository regression | Not run; current verdict is Not Ready, and the canonical proposal gate was the relevant gate for this audit |
| Core GraphQL/MCP boundary tests | Passed in proposal gate |
| MCP idempotency/direct write-unit tests | Passed in proposal gate and focused prior evidence remains present |
| Audit log/budget tests | Passed, including `p081_audit_budget_warning_and_safe_mode_emit_runtime_readback_and_metrics` |
| Swift approval/redaction/alert/accessibility tests | Passed 11 Swift tests in the P081 gate |
| Accessibility risk | Low to medium; model-level coverage is good, no local UI smoke required by proposal |
| Privacy/security risk | Medium; no open security blocker found, but readiness depends on reliability/metrics completion |
| Localization risk | Not materially changed by P081 implementation |
| Permissions/entitlements risk | Medium for native notifications; proposal-level proof is model/readback based, not entitlement runtime smoke |
| Empty/loading/error/offline UI states | Not central to P081 beyond alert/disabled actionability states, which are covered by Swift model tests |
| Operational rollout | Partial due metric and recovery gaps |

## Verification Log

| Command / inspection | Result |
| --- | --- |
| `git rev-parse HEAD` | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| `git merge-base HEAD origin/main` | `3a93e76332512fc07e8b7bec50882ee83d703c2f` |
| `git status --short --branch` | Dirty worktree with 17 modified implementation/gate files and pre-existing untracked R4 audit report before R5. |
| `python3 .../report_path.py .../081-boundary-first-api-auth-contract-matrix.md` | Selected `..._IMPLEMENTATION_AUDIT_R5.md`. |
| `python3 .../discover_prior_review.py .../081-boundary-first-api-auth-contract-matrix.md` | No prior proposal-review artifacts discovered. |
| `git diff --stat` | 17 files changed, 900 insertions, 79 deletions before R5. |
| `./scripts/test-gate.sh proposal-081` | Passed. Final output: `Proposal 081 boundary-first API/auth gate passed`. |
| Swift result bundle | `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-081-swift-20260525-090218.xcresult` |
| Metrics source search | Found new production emitters for several metrics; found no production emitters for the remaining metrics named in OPS-001. |
| MCP readback source search | Found initialize advertises snake_case but `runtime.health.boundaryRuntime` returns camelCase. |
| Reliability source/test search | Found static subscription readback and audit threshold tests; did not find reconnect outside replay window gap behavior or audit safe-mode exit sequence. |

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

P081 has advanced substantially since R4. The current tree passes the canonical proposal gate, closes the previous accessibility evidence gap at the model/test level, adds production emitters for several previously missing metrics, improves audit budget readback, and strengthens MCP idempotency write units.

It is still not ready for closeout because active proposal commitments remain partial: MCP readback casing/tool naming conflicts with the proposal, several rollout metrics are not live production emissions, and reliability behavior for subscription gap replay plus full audit-budget recovery is not implemented/proven end to end.

Recommended next actions:

1. Resolve the MCP runtime contract mismatch, either by implementing `boundary.runtime.get` with snake_case or revising P081 to match `runtime.health.boundaryRuntime`.
2. Add production emission paths for every exact P081 metric or formally remove/reclassify the unused ones.
3. Add executable subscription gap/replay and audit-budget exit scenario tests to the proposal gate.
4. Re-run `./scripts/test-gate.sh proposal-081` after those changes before proposal closeout.
