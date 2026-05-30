# P081 Implementation Audit R1

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/081-boundary-first-api-auth-contract-matrix.md` |
| Audit date | 2026-05-24 |
| Worktree | `.chainworks/worktrees/cw-implement-proposal-081-boundar-4dd7c886` |
| Branch | `cw/implement-proposal-081-boundar/4dd7c886` |
| HEAD | `21b9376f5e799b6ae9c3c3fbdcf6256931833811` |
| Worktree state | Dirty implementation worktree; audit added this report only |
| Overall conformance | Partial |
| Overall readiness | Not Ready |
| Reviewer Selection Reuse | Not reused |

## Scope

Audited the checked-in P081 proposal plus the reference documents it points at as current contract truth: `docs/reference/boundary-first-api-auth-contract.md`, `docs/reference/boundary-first-api-auth-contract.json`, and `docs/reference/swift-macos-boundary-contract.md`.

The proposal itself is marked `Partially Implemented`; the reference docs explicitly defer full observer redaction, accessibility parity, native alert delivery, and Phase 4/5 rollout work. The audit therefore treats the landed boundary/auth substrate as real progress, but does not treat the proposal as complete.

## Prior Review And Reviewer Routing

The proposal-review discovery helper found no adjacent prior review artifact for this proposal path, so reviewer selection was not mechanically reused. A run-local P081 review corpus was present under `.chainworks/runs/4dd7c886.../reviews/proposal/` and was used only as contextual evidence for touched surfaces.

Selected implementation-audit perspectives:

- `rust_arch_reviewer`: Rust daemon boundary policy wiring, persistence, and command paths.
- `rust_reliability_reviewer`: idempotency, recovery, audit durability, and rollout failure modes.
- `api_contract_reviewer`: GraphQL/MCP contract conformance and client-visible errors.
- `observability_rollout_reviewer`: runtime readback, operator alerting, and rollout/canary evidence.
- `apple_arch_reviewer`: Swift/macOS approval action retry, redaction, accessibility, and alert surfaces.

Rejected close alternatives:

- `rust_security_reviewer`: security-sensitive gaps are covered as REQ/API/READY findings; no separate exploit-path review was required for this audit pass.
- `ux_reviewer`: no remote UI/runtime session was run; macOS contract gaps are covered through `apple_arch_reviewer`.

## Track 1: Proposal Conformance

| Requirement | Status | Evidence |
| --- | --- | --- |
| REQ-001 Boundary contract matrix is a durable, diffable artifact with fixed row IDs. | Implemented | `docs/reference/boundary-first-api-auth-contract.json`; embedded copy at `control-plane/crates/auth/src/boundary/embedded_fixture.json`; validator checks in `control-plane/crates/auth/src/boundary/mod.rs`. |
| REQ-002 Boundary policy loads once and exposes runtime readback. | Implemented | GraphQL schema injection in `control-plane/crates/graphql-server/src/schema.rs:76`; MCP runtime readback in `control-plane/crates/mcp-server/src/tools/runtime.rs:20`; `boundaryRuntime` tests in GraphQL and MCP. |
| REQ-003 CallerClass resolution is represented in principal schema v3. | Implemented | `CallerClass` and derivation in `control-plane/crates/auth/src/lib.rs:14`; v3 bootstrap/unknown-version/callerClass tests in the P081 gate. |
| REQ-004 Principal file hardening is complete. | Partially Implemented | Absolute-path, symlink, `0600`, and schema-version checks exist in `control-plane/crates/auth/src/lib.rs`, but hard-link count and parent-directory `0700` checks were not found. |
| REQ-005 GraphQL read and mutation paths apply the boundary matrix. | Partially Implemented | Non-approval/mutation gates and approval idempotency are wired in `control-plane/crates/graphql-server/src/schema.rs`, but observer field-level redaction is explicitly pending in `schema.rs:338`. |
| REQ-006 GraphQL error contract matches the reference. | Partially Implemented | HTTP/operation denial handling exists, but GraphQL WebSocket auth still closes as `1002` in `control-plane/crates/graphql-server/src/server.rs:503` rather than the documented `4401`/`4403`, and `extensions.redactions` is not implemented. |
| REQ-007 MCP initialize/tools/list/tools/call paths enforce boundary policy. | Partially Implemented | Capability and tool-call denial paths return typed denial evidence in `control-plane/crates/mcp-server/src/server.rs:563` and `:636`; idempotency is not committed atomically with the command transaction. |
| REQ-008 State-changing MCP commands use idempotency and recovery semantics. | Partially Implemented | Preclaim/commit/recovery exists in `control-plane/crates/mcp-server/src/server.rs:693` and `:1596`, but `control-plane/crates/db/src/repos/mcp_command_idempotency.rs` writes pending/result state through standalone pool operations outside the command transaction. |
| REQ-009 Approval action mutation idempotency and Swift retry persistence exist. | Implemented | Server-side approval idempotency repo exists; Swift `P081ApprovalActionAttemptStore` is in `Chainworks Forge/Views/RunsHomeView.swift:319`; Swift tests pass. |
| REQ-010 Denials write exactly one audit row and avoid side effects. | Partially Implemented | Denial audit helper exists in `control-plane/crates/mcp-server/src/server.rs:1886`; coverage is focused and does not yet prove every caller class and every boundary row under outage/backpressure conditions. |
| REQ-011 Coverage guardrail prevents untested matrix drift. | Implemented | `scripts/check-boundary-coverage.sh` validates fixture/docs/test coverage; `./scripts/test-gate.sh proposal-081` runs it first. |
| REQ-012 Operator-facing boundary diagnostics are available. | Partially Implemented | `boundaryRuntime` and `auditLogHealth` readbacks exist; the operator alert projection/API/native delivery contract is not implemented. |
| REQ-013 macOS client redaction and accessibility parity are complete. | Missing | Reference doc states typed redaction/accessibility/native alert work is deferred; code search found no `RedactionState`, `extensions.redactions`, or accessibility parity tests for this contract. |
| REQ-014 Boundary rollout canaries/shadow evidence exist. | Missing | No `boundary-policy-canaries` or equivalent shadow report artifact was found. |
| REQ-015 WebSocket policy reload and close-code behavior matches the reference. | Missing | No implemented `4408`/`POLICY_RELOAD` behavior was found; current WebSocket auth tests assert `1002`. |

## Track 2: Specialist Findings

### API-001: GraphQL WebSocket close-code and redaction envelope contract is not implemented

Severity: Major

The reference contract requires WebSocket close codes `4401`/`4403` and typed redaction metadata through `extensions.redactions`. The server currently returns a generic unauthorized connection-init error and tests `GRAPHQL_WS_UNAUTHORIZED_CLOSE_CODE == 1002` in `control-plane/crates/graphql-server/src/server.rs:503`. Code search also found no runtime implementation of `extensions.redactions`.

This leaves subscribed clients unable to distinguish authentication, authorization, redaction, and policy reload cases using the documented contract.

### REL-001: MCP idempotency is not atomic with command commit

Severity: Major

P081's reference contract requires the idempotency record, command/audit record, and durable response semantics to share an atomic commit boundary. MCP preclaim and result update are implemented in `control-plane/crates/db/src/repos/mcp_command_idempotency.rs`, but they use standalone pool operations rather than the command transaction. `control-plane/crates/mcp-server/src/server.rs:693` wires the precheck before dispatch and `:744` updates afterward.

The race window is reduced, but crash consistency still does not match the stated atomic contract for state-changing MCP tools.

### OPS-001: Operator alert lifecycle is still absent

Severity: Major

`boundaryRuntime` and bounded `auditLogHealth` are present, but no implemented `operatorAlerts` projection, MCP tool, native macOS delivery path, dedupe, acknowledgement, or failure lifecycle was found. The implementation self-assessment also lists the operator alert contract as blocking.

This blocks the promised operator-facing detectability for auth outage, audit writer failure, policy safe mode, and similar boundary failures.

### APPLE-001: Swift/macOS redaction, accessibility parity, and native alert delivery remain deferred

Severity: Major

The Swift approval retry key store is implemented and tested, but `docs/reference/swift-macos-boundary-contract.md` still marks typed redaction, accessibility parity, and native alert delivery as deferred. Code search found no implemented `RedactionState`, `drop_resource` UI handling, `extensions.redactions` decoding, or P081 accessibility parity tests.

The macOS client therefore cannot yet satisfy the full boundary-first operator contract.

### REL-002: Reliability proof is narrower than the acceptance criteria

Severity: Major

The P081 gate proves the landed subset, but it does not yet cover several acceptance-level reliability scenarios: SQLite contention, audit-log outage, subscription gap replay, invalid fixture safe-mode exit, SIGTERM drain, committed-unack retry under crash, denial audit backpressure, and full caller-class x boundary-row denial coverage.

Until those failure-mode tests exist, the branch does not meet the proposal's reliability bar even though the focused P081 gate passes.

### READY-001: The implementation should not be marked complete

Severity: Critical

The proposal and reference docs already describe P081 as partially implemented. The current `CHAINWORKS_OUTPUT`/`chainworks_output.json` also report `implementation_complete=false` with blocking remaining work. Audit evidence agrees with that self-assessment.

## Verification Run

Executed from the P081 worktree:

```bash
./scripts/test-gate.sh proposal-081
```

Result: passed. Latest result bundle:

```text
/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-081-swift-20260524-090953.xcresult
```

Covered slices included fixture/documentation guardrail, auth boundary tests, caller-class tests, audit-log repository tests, migrations `064` through `071`, principal schema v3 tests, GraphQL/MCP `boundaryRuntime` readback, and Swift approval action attempt store tests.

Caveat: after printing `Proposal 081 boundary-first API/auth gate passed`, the script emitted `syntax error near unexpected token ')'` while still exiting with status 0. This audit treats the P081 gate as passed but records the post-pass script warning as a tooling caveat.

Not run: full repository gate, remote UI smoke, macOS accessibility/runtime UI verification, or crash/restart reliability harnesses.

## Verdict

P081 is a credible partial implementation: the boundary fixture, validation, caller class schema, runtime readbacks, GraphQL/MCP enforcement substrate, audit-log storage, and Swift approval retry persistence are present and covered by a passing focused gate.

It is not ready for closeout or merge as a fully implemented proposal. The remaining blockers are WebSocket close-code/redaction contracts, atomic MCP idempotency with command commit, operator alert lifecycle, Swift/macOS redaction/accessibility/native alerts, and broader failure-mode reliability proof.
