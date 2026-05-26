# Proposal 081 Implementation Audit R7

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Proposal state | Active, `revised_for_review_blocker_closure` (`Proposal Revision Id: 081-v6`) |
| Audit report | `docs/proposals/081-boundary-first-api-auth-contract-matrix_IMPLEMENTATION_AUDIT_R7.md` |
| Audit timestamp | 2026-05-25 11:10:37 EEST |
| Skill | `proposal-implementation-audit` |
| Implementation worktree | `.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Implementation HEAD | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| Compare base | `3a93e76332512fc07e8b7bec50882ee83d703c2f` (`git merge-base HEAD origin/main`) |
| Canonical verification | `./scripts/test-gate.sh proposal-081` passed |
| Reviewer reuse | Not reused; discovery found no prior proposal-review artifacts |

## Direct Verdict

- **Implementation conformance:** Partial.
- **Implementation readiness:** Not Ready.
- **Primary blocker:** P081 runtime reliability and MCP runtime readback blockers are closed, but rollout/observability metrics still do not fully preserve the proposal-required label dimensions and delivery semantics.
- **Requirement coverage:** 24 Implemented, 1 Partially Implemented, 0 Missing, 0 Not Verifiable, 0 Out of Scope.

## Prior Proposal-Review Reuse

The review helper did not discover prior proposal-review artifacts for reviewer reuse. Per the skill instructions, previous implementation-audit reports were ignored for reviewer selection and used only as historical context after the current code/proposal evidence was inspected.

## Selected Reviewers

| Reviewer | Used for | Rationale |
| --- | --- | --- |
| `rust_arch_reviewer` | BoundaryPolicy ownership, daemon injection, dependency direction | P081 is primarily a Rust control-plane contract with GraphQL/MCP/approval shared policy ownership. |
| `rust_reliability_reviewer` | Audit budget, safe mode, cursor replay, contention, shutdown drain | The prior highest-risk area was runtime reliability and bounded failure behavior. |
| `api_contract_reviewer` | GraphQL/MCP surface contracts, error/readback shape, actionability | P081 is an API/auth contract matrix across northbound surfaces. |
| `observability_rollout_reviewer` | Metrics, rollout readback, shadow coverage, operator evidence | The remaining blocker is in metrics and rollout observability. |
| `macos_ui_reviewer` | Swift approval-only UI, redaction accessibility, native alerts | The proposal explicitly requires macOS-native alert and accessibility parity evidence. |

## Rejected Close Alternatives

| Reviewer | Reason not selected |
| --- | --- |
| `rust_security_reviewer` | Security hardening was inspected through the architecture/API/reliability lenses; no separate unrepresented security finding remained. |
| `apple_arch_reviewer` | Swift changes are support/readback/accessibility behaviors, not broad app architecture changes. |
| `product_reviewer` | Residual risk is technical rollout correctness rather than product scope or UX value. |
| `rust_performance_reviewer` | The proposal defines latency metric contracts, but no p99/throughput performance claim needed a separate performance lens. |

## Proposal Contract Summary

P081 requires canonical human and machine-readable boundary matrix artifacts, server-derived `CallerClass`, shared `BoundaryPolicy` routing for GraphQL, MCP, and approval actionability, approval-only Swift UI mutations, durable idempotency, fail-closed audit behavior, runtime readback, operator alerts, reliability safe-mode behavior, rollout metrics, and macOS accessibility/native alert parity. The proposal is explicitly scoped to preserve the existing GraphQL read/subscription plus approval-only UI boundary while keeping non-approval control on MCP.

Key proposal anchors inspected:

- Goals and non-goals: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:30`.
- Runtime readback and reliability contract: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:653`.
- Metrics contract: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:766`.
- Acceptance criteria: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:995`.

## Platform / Product Scope

- **Apple/macOS:** SwiftUI operator shell, approval-only mutation client, redaction decoding, accessibility parity, native notification/menu bar/dock attention behavior.
- **Rust/backend:** Auth fixture validation, boundary policy, GraphQL/MCP transport guards, audit/idempotency repositories, runtime readback, reliability gates, metrics.
- **Product boundary:** No new UI write behavior beyond approve/reject approval; no GraphQL agent control plane; no broad audit browser.

## Primary Implementation Flows Audited

1. Operator UI reads and subscribes over GraphQL, then sends only `approveApproval` and `rejectApproval` mutations with an idempotency key.
2. Agent/automation callers use MCP initialize, tools/list, and tools/call through the shared policy, capability filtering, idempotency preclaim, and denial audit.
3. Observer callers receive compact readback with redaction and `actionability_false` semantics rather than write access.
4. Runtime safety surfaces expose safe mode, audit health, fixture digests, subscription replay/gap state, shadow coverage refs, and operator alerts through bounded GraphQL/MCP diagnostics.
5. Swift renders redacted nil, restricted view, actionability diagnostics, native alert lifecycle, and retry-stable approval idempotency behavior.

## Fidelity / Divergence Inventory

### Matches

- Boundary contract docs and JSON exist and are wired into documentation indexes.
- The Rust daemon constructs one validated policy at startup and injects it into GraphQL/MCP paths.
- GraphQL, MCP, and approval paths now share policy decisioning, bounded denial audit, and idempotency semantics.
- MCP exposes `boundary.runtime.get` with the snake_case diagnostic shape promised by the contract.
- Audit budget safe mode, recovery, subscription gap detection, tamper-startup safe mode, and shutdown drain are covered by tests and passed the focused gate.
- Swift tests cover redaction typing, `actionability_false`, Full Keyboard Access, Increase Contrast, Reduce Motion, native alert delivery lifecycle, hidden/inactive window alert behavior, and approval retry idempotency.

### Divergences

- The metrics surface records required metric names, but several required label dimensions and semantics are not represented end to end. This affects rollout and operational observability, not the core policy decision path.

### Ambiguities / Evidence Gaps

- No live daemon startup or remote UI smoke was required by the proposal gate. The proposal explicitly defines the P081 gate as focused contract validation that does not require live daemon startup or UI smoke hosts.
- No separate follow-up proposal was found that owns the metric label/semantic gap.

## Residual Scope / Follow-up Ownership

| Residual item | Status | Follow-up owner found? | Blocks readiness? |
| --- | --- | --- | --- |
| Proposal-required metric label dimensions and native delivery metric semantics | Open | No | Yes |
| Runtime reliability safe mode/readback/recovery | Closed in this implementation | N/A | No |
| MCP `boundary.runtime.get` diagnostic lane | Closed in this implementation | N/A | No |
| Swift native alert/accessibility parity | Closed in this implementation | N/A | No |

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 24 |
| Partially Implemented | 1 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed Requirement Audit

| ID | Requirement | Status | Evidence / notes |
| --- | --- | --- | --- |
| REQ-001 | Canonical human-readable and machine-readable boundary matrix artifacts exist and are indexed. | Implemented | `docs/reference/boundary-first-api-auth-contract.md`, `docs/reference/boundary-first-api-auth-contract.json`, `docs/reference/README.md`, `docs/README.md`. |
| REQ-002 | JSON fixture validator rejects unknown/missing/duplicate/invalid matrix rows and covers required rows. | Implemented | P081 gate validated fixture/doc coupling, all required rows, compact observer rows, shadow coverage fixtures, and structured canaries. |
| REQ-003 | Build embeds a validated last-known-good matrix and malformed deployed fixture enters read-only safe mode. | Implemented | `control-plane/crates/auth/src/boundary/embedded_fixture.json`; fallback/safe-mode handling in `control-plane/crates/auth/src/boundary/mod.rs:825`. |
| REQ-004 | Server-derived `CallerClass` and `CallerContext.caller_class` drive dispatch decisions. | Implemented | Boundary auth module and schema v3 principal tests passed; caller class is derived rather than trusted from stored files. |
| REQ-005 | One immutable validated `BoundaryPolicy` is built at daemon startup and shared across GraphQL, MCP, and approval actionability. | Implemented | Explicit constructor/injection checks passed; policy ownership in `control-plane/crates/auth/src/boundary/mod.rs:694` and daemon wiring. |
| REQ-006 | GraphQL queries, subscriptions, and approval-only mutations use deterministic boundary/error contracts. | Implemented | GraphQL bounded readback and denial tests passed; approval mutation safe-mode denial covered by `control-plane/crates/graphql-server/src/schema.rs:8321`. |
| REQ-007 | MCP initialize, tools/list, and tools/call use boundary policy, distinguish known denied from unknown tools, and advertise P081 capability. | Implemented | MCP server tests cover denied state-changing calls, runtime diagnostics, and Codex-compatible tool aliasing; dispatch includes `boundary.runtime.get` at `control-plane/crates/mcp-server/src/server.rs:1528`. |
| REQ-008 | Approval actionability reflects policy decisions and uses `actionability_false` rather than misleading enabled actions. | Implemented | GraphQL/Swift tests cover actionability false and accessible disabled controls. |
| REQ-009 | `approveApproval` and `rejectApproval` require idempotency keys, are terminal-state safe, and do not double-settle on retry. | Implemented | Command handler duplicate/terminal replay paths and Swift retry-key tests passed; Swift client sends `idempotencyKey`. |
| REQ-010 | State-changing MCP commands preclaim idempotency, reject inappropriate keys on read-only calls, and recover committed-unack results. | Implemented | MCP idempotency repository and server tests passed, including committed-unack recovery and conflict behavior. |
| REQ-011 | Allowed mutating calls atomically couple command/domain writes, boundary row, idempotency key, and audit append where required. | Implemented | Approval settlement and audit append occur in the settlement transaction; command journal/idempotency linkage tests passed. |
| REQ-012 | Denied calls produce no command/projection side effects and fail closed if required denial audit cannot be written. | Implemented | GraphQL/MCP denial audit paths write bounded primary rows and fail closed on append failure; denial-side-effect tests passed. |
| REQ-013 | Audit log migrations/repository implement payload hashes, row hashes, checkpoints, bounded readback, retention, and tamper verification. | Implemented | Additive audit migrations and `db::repos::audit_log` append/cleanup/checkpoint behavior inspected; audit repository tests passed. |
| REQ-014 | Principal-table compatibility preserves v1/v2, introduces schema_version 3, rejects unknown versions, hardens paths, and redacts tokens. | Implemented | Principal schema tests passed for v3 writing, unknown-version rejection, private parent dirs, hardlink rejection, and derived caller class. |
| REQ-015 | Security hardening covers strict parsing, constant-time token comparison, expiry, redaction, break-glass non-disclosure, DoS controls, and tamper evidence. | Implemented | Auth boundary module tests passed; no production break-glass endpoint was introduced. |
| REQ-016 | Runtime diagnostic readback exposes policy mode, safe mode, fixture digests, audit health, shadow coverage, and bounded audit diagnostics in GraphQL and MCP. | Implemented | GraphQL runtime readback in `control-plane/crates/graphql-server/src/schema.rs:411`; MCP runtime readback tests assert `boundary.runtime.get` shape and no broad audit browser. |
| REQ-017 | Operator alert contract has GraphQL/MCP readback, payload schema, lifecycle, thresholds/windows, native delivery, and hidden/inactive window tests. | Implemented | GraphQL alert readback and MCP `operator.alerts.list` tests passed; Swift native alert tests passed, including hidden/inactive window behavior. |
| REQ-018 | Reliability runtime covers audit budget warning/safe mode/cleanup/recovery, subscription cursor/gap detection, tamper startup safe mode, SQLite contention, SIGTERM drain, and denial-audit backpressure. | Implemented | Audit budget safe-mode denial tests for GraphQL and MCP passed; subscription replay/gap tests passed; shutdown drain test passed; runtime readback shows audit budget and replay state. |
| REQ-019 | Startup/restart/rollback behavior is bounded and request paths do not read reference files directly. | Implemented | Fixture validation and policy construction occur at startup; request paths use injected in-memory policy. |
| REQ-020 | Swift approval mutations use an `ApprovalActionAttemptStore`-owned idempotency key that is reused until success. | Implemented | `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:3184` and `:3216` passed. |
| REQ-021 | Swift decoding preserves GraphQL redaction extensions and accessibility distinguishes redacted nil, ordinary nil, restricted view, and actionability false. | Implemented | Swift redaction/accessibility tests passed at `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift:8`, `:45`, `:182`, `:305`, `:334`. |
| REQ-022 | Current-system baseline remains explicit: GraphQL read/subscription plus approval-only UI mutation; non-approval control remains MCP. | Implemented | Reference docs updated; no evidence of new UI write behavior beyond approvals. |
| REQ-023 | Guardrails, canaries, shadow coverage, and no-op label workflow are wired into focused gates. | Implemented | `scripts/test-gate.sh proposal-081` includes coverage, fixture, canary, and shadow coverage checks; gate passed. |
| REQ-024 | Evidence fixtures and readback lanes exist for operator readback, release receipt, shadow coverage, and docs. | Implemented | Operator readback and shadow coverage fixtures validated by the gate. |
| REQ-025 | Metrics implement adoption/operational metric names with required labels and meaningful rollout semantics. | Partially Implemented | Metric names exist, but several required labels/semantics are absent or not proven. See OPS-001. |

## Reviewer / Lens Scorecard

| Lens | Result | Notes |
| --- | --- | --- |
| Rust architecture | Pass with minor residual risk | Shared policy ownership/injection is explicit and tested. |
| Rust reliability | Pass | R6 reliability blockers are closed; P081 gate passed audit budget, subscription gap, tamper/safe-mode, and shutdown checks. |
| API contract | Pass | GraphQL/MCP readback and denial semantics match the proposal, including MCP `boundary.runtime.get`. |
| Observability / rollout | Fail | Metrics label and native delivery semantics remain incomplete. |
| macOS UI | Pass | Swift accessibility, redaction, native alert, and idempotency behaviors are covered by targeted tests. |

## Routed Specialist Findings

### OPS-001 - P081 metrics do not preserve all required label dimensions and native delivery semantics

- **Severity:** Major.
- **Confidence:** High.
- **Related requirement:** REQ-025.
- **Proposal source:** `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:766`.
- **Evidence:**
  - The proposal requires `boundary_policy_decision_latency_ms{transport,caller_class,mode}`, `boundary_commit_transaction_latency_ms{transport,action_kind,decision}`, and `operator_alert_clear_latency_ms{alert_id,severity}`.
  - The implemented helpers store these as bare latency keys with no required labels: `control-plane/crates/db/src/metrics.rs:219`, `:227`, `:243`.
  - The proposal requires `audit_log_append_failure_total{event_type,transport,mode}`, but several call sites increment only the bare metric name, losing the required dimensions: `control-plane/crates/graphql-server/src/schema.rs:320`, `:397`; `control-plane/crates/mcp-server/src/server.rs:413`, `:516`, `:645`, `:720`, `:2106`.
  - The proposal requires `operator_alert_native_delivery_total{severity,surface,result}`. The helper supports labels, but GraphQL readback records `surface=graphql_operator_alerts,result=available` when an alert merely has a `nativeDelivery` payload (`control-plane/crates/graphql-server/src/schema.rs:598`), which is readback availability rather than an actual macOS native delivery result.
- **Impact:** Rollout dashboards can show required names while hiding transport/caller/mode/action/alert dimensions needed to detect boundary drift, audit failures, and native alert delivery failure. This weakens phase-gate evidence and can produce false confidence during staged enforcement.
- **Recommended fix:** Add label-aware storage/readback or metric emission for the required histogram dimensions, replace all bare `audit_log_append_failure_total` increments with structured event_type/transport/mode helpers, and move or supplement native delivery metric emission so it represents actual native delivery/silence/error outcomes rather than server-side readback availability. Extend `proposal-081` tests to assert label dimensions and native delivery semantics, not only metric-name presence.

### READY-001 - Implementation is not merge-ready while REQ-025 remains partial and unowned

- **Severity:** Major.
- **Confidence:** High.
- **Related requirement:** REQ-025.
- **Evidence:** The proposal acceptance criteria require rollout metrics and readback evidence to be executable enough for implementation not to invent semantics. No separate follow-up proposal or owner was found for the metric contract gap.
- **Impact:** Merging as implemented would leave the reference docs claiming full P081 observability while the operational contract is only name-complete.
- **Recommended fix:** Close OPS-001 in this implementation or create an explicit follow-up proposal that narrows and owns the rollout metric contract before declaring P081 implemented.

## Readiness Checklist

| Check | Result |
| --- | --- |
| Proposal state understood and not treated as superseded/deprecated | Pass |
| Prior reviewer reuse handled | Pass, not reused |
| Worktree implementation inspected against proposal | Pass |
| Canonical focused gate run | Pass |
| Report written beside proposal with next version number | Pass |
| Requirements fully implemented | Fail, REQ-025 partial |
| No unowned residual scope | Fail, metric label/semantic gap unowned |
| Ready for merge/closeout | Fail |

## Verification Log

- `./scripts/test-gate.sh proposal-081` passed.
  - Boundary fixture/doc coverage passed.
  - Contract fixture, operator readback fixture, shadow coverage fixture, and canary validation passed.
  - Rust auth boundary, caller class, audit repo, metrics-name recordability, GraphQL/MCP runtime readback, safe-mode denial, idempotency, and daemon shutdown drain checks passed.
  - Swift targeted tests passed: 11 tests across P081 redaction/readback/accessibility/native alert behavior and approval action attempt store.
  - Xcode result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//chainworks-test-gates/proposal-081-swift-20260525-110925.xcresult`.
- Source inspection covered the proposal, worktree diff, contract docs/JSON, Rust auth/db/graphql/mcp/daemon changes, Swift client/tests, evidence fixtures, and gate wiring.

## Final Verdict and Required Actions

P081 is substantially implemented and the previous reliability/runtime-readback blockers are closed. The implementation is still **Not Ready** because the observability contract is only partially implemented: required metric labels and actual native delivery semantics are not proven or preserved end to end.

Required actions before closeout:

1. Fix OPS-001 by preserving the proposal-specified metric label dimensions and correcting native delivery metric semantics.
2. Add focused tests/gate assertions that fail when required label dimensions are missing or native delivery is recorded as mere readback availability.
3. Re-run `./scripts/test-gate.sh proposal-081` after the metric fix.
