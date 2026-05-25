# Proposal 081 Implementation Audit R8

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Proposal state | Active, `revised_for_review_blocker_closure` (`Proposal Revision Id: 081-v6`) |
| Audit report | `docs/proposals/081-boundary-first-api-auth-contract-matrix_IMPLEMENTATION_AUDIT_R8.md` |
| Audit timestamp | 2026-05-25 14:57:40 EEST |
| Skill | `proposal-implementation-audit` |
| Implementation worktree | `.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Implementation HEAD | `68faf5a7e26dd11aac3a4a635e7ecdf3d4fab2aa` |
| Compare base | `3a93e76332512fc07e8b7bec50882ee83d703c2f` (`git merge-base HEAD origin/main`) |
| Canonical verification | `./scripts/test-gate.sh proposal-081` passed on this same worktree |
| Reviewer reuse | Not reused; discovery found no prior proposal-review artifacts |
| Audit confidence | High |

## Direct Verdict

- **Overall conformance:** Implemented.
- **Overall implementation readiness:** Ready.
- **Blocking findings:** None.
- **Requirement coverage:** 25 Implemented, 0 Partially Implemented, 0 Missing, 0 Not Verifiable, 0 Out of Scope.
- **Delta from the previous audit trail:** The prior metric/observability blocker is now closed by labeled metric helpers, structured audit append-failure recording, macOS-native delivery metric events, gate-owned token checks, and passing same-tree `proposal-081` verification.

## Prior Proposal-Review Reuse

The discovery helper found no prior proposal-review artifacts for this proposal. Prior implementation-audit files were ignored for reviewer selection per the skill instructions and used only as historical context after the current proposal and implementation evidence were re-read.

## Selected Reviewers

| Reviewer | Used for | Rationale |
| --- | --- | --- |
| `rust_arch_reviewer` | BoundaryPolicy ownership, daemon injection, dependency direction | P081 is primarily a Rust control-plane contract with shared policy ownership across GraphQL, MCP, and approvals. |
| `rust_reliability_reviewer` | Audit budget, safe mode, subscription replay, contention, shutdown drain | The proposal explicitly requires bounded failure and recovery behavior. |
| `api_contract_reviewer` | GraphQL/MCP surface contracts, error/readback shape, actionability | P081 defines a cross-surface API/auth contract matrix. |
| `observability_rollout_reviewer` | Metrics, rollout readback, shadow coverage, gate ownership | P081 has explicit adoption and operational metric commitments. |
| `macos_ui_reviewer` | Swift approval-only UI, redaction accessibility, native alerts | The proposal requires macOS-native critical alerts and accessibility parity. |

## Rejected Close Alternatives

| Reviewer | Reason not selected |
| --- | --- |
| `rust_security_reviewer` | Auth and hardening evidence was covered through Rust architecture/API/reliability inspection; no separate unresolved security risk remained. |
| `apple_arch_reviewer` | Swift changes are narrow notification/readback/idempotency support, not a broad app architecture change. |
| `product_reviewer` | Product scope is stable; the audit risk was contract and rollout correctness rather than value definition. |
| `rust_performance_reviewer` | The proposal requires latency metrics but does not claim a new benchmark or throughput target requiring a performance lens. |

## Proposal State and Contract Summary

P081 requires canonical human and machine-readable boundary matrix artifacts, server-derived `CallerClass`, shared `BoundaryPolicy` routing for GraphQL, MCP, and approval actionability, approval-only Swift UI mutations, durable idempotency, fail-closed audit behavior, runtime readback, operator alerts, reliability safe-mode behavior, rollout metrics, and macOS accessibility/native alert parity. It explicitly preserves the existing GraphQL read/subscription plus approval-only UI boundary while keeping non-approval control on MCP.

Primary proposal anchors:

- Goals and non-goals: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:30`.
- Runtime readback and reliability contract: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:653`.
- Metrics contract: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:766`.
- Acceptance criteria: `docs/proposals/081-boundary-first-api-auth-contract-matrix.md:995`.

## Platform / Product Scope

- **Apple:** macOS operator shell, approval-only mutation client, redaction decoding, accessibility parity, native notification/menu bar/dock attention behavior.
- **Backend/service:** Rust service/API/data/rollout scope across auth, DB, GraphQL, MCP, engine command handling, migrations, readback, and metrics.
- **Cross-stack:** GraphQL/MCP/Swift contract alignment for approvals, observer reads, alerts, redactions, idempotency, and operational evidence.

## Primary Implementation Flows

1. Operator UI reads and subscribes over GraphQL, then sends only `approveApproval` and `rejectApproval` mutations with a retry-stable idempotency key.
2. Agent/automation callers use MCP initialize, tools/list, and tools/call through shared policy, capability filtering, idempotency preclaim, and denial audit.
3. Observer callers receive compact GraphQL/MCP reads with redaction and `actionability_false` semantics instead of write access.
4. Runtime safety surfaces expose safe mode, audit health, fixture digests, subscription replay/gap state, shadow coverage refs, and operator alerts through bounded GraphQL/MCP diagnostics.
5. Swift renders redacted nil, restricted view, actionability diagnostics, native alert lifecycle, and native delivery metric outcomes.

## Proposal Fidelity / Divergence Inventory

### Matches

- Boundary contract docs and JSON exist, are indexed, and cover required rows.
- The daemon constructs one validated policy at startup and injects it into GraphQL/MCP/approval paths.
- GraphQL, MCP, and approval paths share policy decisioning, bounded denial audit, and idempotency semantics.
- MCP exposes `boundary.runtime.get` with the snake_case diagnostic shape promised by the contract.
- Audit budget safe mode, recovery, subscription gap detection, tamper-startup safe mode, and shutdown drain are covered by tests and passed the focused gate.
- Rollout metric names and proposal-required label dimensions are gate-owned; native delivery outcomes are recorded from the macOS notification service rather than server readback availability.
- Swift tests cover redaction typing, `actionability_false`, Full Keyboard Access, Increase Contrast, Reduce Motion, native alert delivery lifecycle, hidden/inactive window alert behavior, approval retry idempotency, and native delivery metric outcomes.

### Divergences

No proposal-contract divergences remain in the audited implementation.

### Ambiguities / Evidence Gaps

- The proposal gate intentionally does not require live daemon startup, production credentials, remote UI smoke, simulator runs, or destructive operator actions. Runtime evidence is therefore code/test/gate based, not a live production-like daemon session.
- Swift warnings and Rust dead-code/unused warnings appear during the gate, but they are pre-existing/non-P081 blocking noise and do not fail the canonical gate.

## Residual Scope / Follow-up Ownership

| Residual item | Status | Follow-up owner found? | Blocks conformance/readiness? |
| --- | --- | --- | --- |
| Boundary matrix/docs/fixture and validator | Complete | N/A | No |
| Shared policy injection and caller class | Complete | N/A | No |
| GraphQL/MCP/approval idempotency and audit coupling | Complete | N/A | No |
| Runtime reliability safe mode/readback/recovery | Complete | N/A | No |
| Rollout metrics and native delivery semantics | Complete | N/A | No |
| Swift redaction/accessibility/native alerts | Complete | N/A | No |

No unfinished or deferred in-scope proposal behavior remains unowned.

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 25 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

## Detailed Requirement Audit

| ID | Requirement | Status | Evidence / notes |
| --- | --- | --- | --- |
| REQ-001 | Canonical human-readable and machine-readable boundary matrix artifacts exist and are indexed. | Implemented | `docs/reference/boundary-first-api-auth-contract.md`, `docs/reference/boundary-first-api-auth-contract.json`, `docs/reference/README.md`, `docs/README.md`. |
| REQ-002 | JSON fixture validator rejects unknown/missing/duplicate/invalid matrix rows and covers required rows. | Implemented | P081 gate validated fixture/doc coupling, all required rows, compact observer rows, shadow coverage fixtures, and structured canaries. |
| REQ-003 | Build embeds a validated last-known-good matrix and malformed deployed fixture enters read-only safe mode. | Implemented | Embedded fixture and safe-mode fallback in `control-plane/crates/auth/src/boundary/`; auth boundary tests passed. |
| REQ-004 | Server-derived `CallerClass` and `CallerContext.caller_class` drive dispatch decisions. | Implemented | Caller-class tests passed; principal schema v3 derives caller class rather than trusting stored fields. |
| REQ-005 | One immutable validated `BoundaryPolicy` is built at daemon startup and shared across GraphQL, MCP, and approval actionability. | Implemented | Explicit constructor/injection gate passed; policy ownership in `control-plane/crates/auth/src/boundary/mod.rs`. |
| REQ-006 | GraphQL queries, subscriptions, and approval-only mutations use deterministic boundary/error contracts. | Implemented | GraphQL bounded readback, redaction, safe-mode denial, and approval mutation tests passed. |
| REQ-007 | MCP initialize, tools/list, and tools/call use boundary policy, distinguish known denied from unknown tools, and advertise P081 capability. | Implemented | MCP tests cover denied state-changing calls, runtime diagnostics, `boundary.runtime.get`, and Codex-compatible aliasing. |
| REQ-008 | Approval actionability reflects policy decisions and uses `actionability_false` rather than misleading enabled actions. | Implemented | GraphQL/Swift tests cover actionability false and accessible disabled controls. |
| REQ-009 | `approveApproval` and `rejectApproval` require idempotency keys, are terminal-state safe, and do not double-settle on retry. | Implemented | Command handler duplicate/terminal replay paths and Swift retry-key tests passed. |
| REQ-010 | State-changing MCP commands preclaim idempotency, reject inappropriate keys on read-only calls, and recover committed-unack results. | Implemented | MCP idempotency repository and server tests passed, including committed-unack recovery and conflict behavior. |
| REQ-011 | Allowed mutating calls atomically couple command/domain writes, boundary row, idempotency key, and audit append where required. | Implemented | Approval settlement and audit append occur in the settlement transaction; command journal/idempotency linkage tests passed. |
| REQ-012 | Denied calls produce no command/projection side effects and fail closed if required denial audit cannot be written. | Implemented | GraphQL/MCP denial audit paths write bounded primary rows and fail closed on append failure; denial-side-effect tests passed. |
| REQ-013 | Audit log migrations/repository implement payload hashes, row hashes, checkpoints, bounded readback, retention, and tamper verification. | Implemented | Additive audit migrations and `db::repos::audit_log` append/cleanup/checkpoint behavior inspected; audit repository tests passed. |
| REQ-014 | Principal-table compatibility preserves v1/v2, introduces schema_version 3, rejects unknown versions, hardens paths, and redacts tokens. | Implemented | Principal schema and hardening tests passed for v3 writing, unknown-version rejection, private parent dirs, hardlink rejection, and derived caller class. |
| REQ-015 | Security hardening covers strict parsing, constant-time token comparison, expiry, redaction, break-glass non-disclosure, DoS controls, and tamper evidence. | Implemented | Auth boundary module and principal tests passed; no production break-glass endpoint was introduced. |
| REQ-016 | Runtime diagnostic readback exposes policy mode, safe mode, fixture digests, audit health, shadow coverage, and bounded audit diagnostics in GraphQL and MCP. | Implemented | GraphQL runtime readback and MCP runtime readback tests passed; `boundary.runtime.get` exposes bounded snake_case diagnostics. |
| REQ-017 | Operator alert contract has GraphQL/MCP readback, payload schema, lifecycle, thresholds/windows, native delivery, and hidden/inactive window tests. | Implemented | GraphQL alert readback, MCP `operator.alerts.list`, and Swift native alert tests passed, including hidden/inactive window behavior. |
| REQ-018 | Reliability runtime covers audit budget warning/safe mode/cleanup/recovery, subscription cursor/gap detection, tamper startup safe mode, SQLite contention, SIGTERM drain, and denial-audit backpressure. | Implemented | Audit budget safe-mode denial tests for GraphQL/MCP passed; subscription replay/gap tests passed; shutdown drain test passed. |
| REQ-019 | Startup/restart/rollback behavior is bounded and request paths do not read reference files directly. | Implemented | Fixture validation and policy construction occur at startup; request paths use injected in-memory policy. |
| REQ-020 | Swift approval mutations use an `ApprovalActionAttemptStore`-owned idempotency key that is reused until success. | Implemented | `Proposal081ApprovalActionAttemptStoreTests` passed in the Swift gate. |
| REQ-021 | Swift decoding preserves GraphQL redaction extensions and accessibility distinguishes redacted nil, ordinary nil, restricted view, and actionability false. | Implemented | Swift redaction/accessibility tests passed. |
| REQ-022 | Current-system baseline remains explicit: GraphQL read/subscription plus approval-only UI mutation; non-approval control remains MCP. | Implemented | Reference docs updated; no evidence of new UI write behavior beyond approvals. |
| REQ-023 | Guardrails, canaries, shadow coverage, and no-op label workflow are wired into focused gates. | Implemented | `scripts/test-gate.sh proposal-081` includes coverage, fixture, canary, and shadow coverage checks; gate passed. |
| REQ-024 | Evidence fixtures and readback lanes exist for operator readback, release receipt, shadow coverage, and docs. | Implemented | Operator readback and shadow coverage fixtures validated by the gate. |
| REQ-025 | Metrics implement adoption/operational metric names with required labels and meaningful rollout semantics. | Implemented | Labeled histogram helpers and structured audit failure helper in `control-plane/crates/db/src/metrics.rs:160`, `:207`, `:251`, `:268`, `:293`; macOS native delivery metric event in `Chainworks Forge/Engine/NotificationService.swift:5`, `:142`, `:248`; gate-owned semantic scan in `scripts/test-gate.sh:6365`; metric tests passed. |

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
| --- | --- | --- | --- |
| Conformance | Pass | None remaining | High |
| Rust architecture | Pass | Policy must remain daemon-injected and immutable on future changes | High |
| Rust reliability | Pass | Future edits must preserve safe-mode/audit-budget fail-closed behavior | High |
| API contract | Pass | Future MCP/GraphQL additions need matrix rows before exposure | High |
| Observability / rollout | Pass | Metric label semantics are now gate-owned; keep the source scan in the gate until richer metric assertions exist | Medium-high |
| macOS UI | Pass | Native metric events are app-local; future telemetry exporters should consume the same event shape rather than re-infer delivery | Medium-high |
| Readiness | Ready | No blocking findings; canonical same-tree gate passed | High |

## Routed Specialist Findings

No blocking or non-blocking specialist findings remain for this audit.

Closed during the current implementation state:

- `OPS`: Metric label and native-delivery semantic gap is closed by labeled metric helpers, structured append-failure helper, macOS notification-service metric events, and gate-owned scans/tests.
- `READY`: No unowned residual proposal scope remains after the metric closure and same-tree gate pass.

## Readiness Checklist

| Check | Result |
| --- | --- |
| Proposal state understood and not treated as superseded/deprecated | Pass |
| Prior reviewer reuse handled | Pass, not reused |
| Worktree implementation inspected against proposal | Pass |
| Canonical focused gate run on audited tree/HEAD | Pass |
| Core service flows validated by targeted tests | Pass |
| macOS accessibility/native alert/idempotency tests executed | Pass |
| Empty/loading/error/offline/permission states relevant to P081 | Pass by scope; proposal concerns redaction/actionability/safe-mode states and they are tested |
| Accessibility risk | Pass; Full Keyboard Access, Increase Contrast, Reduce Motion, redacted nil, restricted view, and disabled actionability are covered |
| Localization/privacy/permissions/entitlements risk | Pass by scope; no new localization/entitlement promise, auth/privacy redaction covered |
| Report written beside proposal with next version number | Pass |
| Unowned residual scope | None |
| Ready for closeout/merge handoff | Pass |

## Verification Log

- `./scripts/test-gate.sh proposal-081` passed on this worktree.
  - Boundary fixture/doc coverage passed.
  - Contract fixture, operator readback fixture, shadow coverage fixture, and canary validation passed.
  - Reliability proof inventory passed.
  - Rust auth boundary tests passed: 39 tests.
  - Rust caller-class tests passed: 10 tests.
  - DB audit repository and P081 metric tests passed.
  - Rollout metric label/native-delivery semantic source scan passed.
  - GraphQL runtime/readback/operator-alert/subscription/idempotency tests passed.
  - MCP runtime/readback/safe-mode/idempotency/operator-alert tests passed.
  - Daemon shutdown drain reliability test passed.
  - Swift targeted gate passed: 11 tests across P081 redaction/readback/accessibility/native alert behavior and approval action attempt store.
  - Xcode result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T//chainworks-test-gates/proposal-081-swift-20260525-145617.xcresult`.
- Source inspection covered the proposal, worktree diff, contract docs/JSON, Rust auth/db/graphql/mcp/daemon changes, Swift notification/client/tests, evidence fixtures, and gate wiring.

## Final Verdict and Recommended Next Actions

P081 is implemented against the audited proposal contract and is ready for closeout/merge handoff. The previously blocking metrics issue is closed. No additional follow-up proposal is required for the audited scope.

Recommended next action: run the proposal closeout workflow to retire implemented proposal truth into reference documentation and preserve this audit trail.
