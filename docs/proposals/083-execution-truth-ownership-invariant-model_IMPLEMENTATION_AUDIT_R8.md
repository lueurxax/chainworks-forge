# P083 Implementation Audit R8 - Execution-Truth Ownership and Invariant Model

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/083-execution-truth-ownership-invariant-model.md` |
| Proposal id / revision | `P083` / `P083-r70-refined-r69-score-lift` |
| Audit report | `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R8.md` |
| Audit date | 2026-06-21 |
| Implementation target | Current working tree at `0e6482c82b58` plus dirty/untracked P083 implementation changes |
| Compare base | Current repository state; no alternate base was supplied |
| Prior review reuse | Not reused: `discover_prior_review.py` found no proposal-review artifacts for this proposal path |
| Canonical gate | `./scripts/test-gate.sh proposal-083` passed in this audit run |
| Security-sensitive diff | Triggered; manual security pass completed for auth, public ingress, parser/schema, process boundary, and redaction surfaces |

## Executive Verdict

Overall conformance: **Not Implemented for full proposal acceptance**.

Readiness: **Not Ready**.

The implementation contains substantial P083 work: the eight migration files exist, the P083 gate passes, CallerRequestId validation is present, rollback target mode is now wired across GraphQL/MCP/idempotency/readback checks, durable monotonic-clock correlation is statically verified, and macOS Run menu/toolbar parity has static proof. That is enough to call this a broad partial implementation, but not enough for Ready.

The blockers are contract-level, not just missing evidence. The proposal itself still says implementation may start only after a fresh approve review with `blocker_count=0`, while its active readiness narrative has `implementation_may_start=false` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`, `:43-46`). The runtime GraphQL/MCP denial vocabulary and MCP output schemas diverge from the proposal's executable API contract. Both GraphQL and MCP advertise `defer` as an approval resolution, but both deny it rather than implementing a durable defer transition.

## Proposal State

State classification: **Revise-required / pre-implementation approval not satisfied**.

Evidence:

- Proposal status says implementation may start only after the human approval gate and a fresh aggregate approve review with `blocker_count=0` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`).
- Active readiness says `implementation_may_start=false` and points to `reviews/proposal/summary.json decision=revise_required, blocker_count=1` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`).
- The prior-review helper returned no sidecar review artifacts for reuse. The embedded `review_basis` was useful as proposal context, but it is not a reusable implementation-review artifact.

## Primary Flows Audited

1. Lifecycle mutation ingress through GraphQL and MCP using `CallerRequestId`, principal class checks, and command idempotency.
2. Rollback and enforcement-mode changes carrying non-null target enforcement mode through API, intent hash, audit rows, and rollout readback.
3. Provider-session shutdown/cancellation, process-identity ambiguity, late output handling, and recovery classifications.
4. Migration/readback/rollout evidence including the P083 migration corpus, rollout contract, metrics, and readback lanes.
5. macOS read-only operator surfaces for lifecycle commands, toolbar/menu placement, and manual process identity check UX.

## Fidelity and Divergence Summary

Implemented or strongly evidenced:

- All eight P083 migration files are present and the gate verifies physical filenames plus readback descriptors (`scripts/test-gate.sh:9553-9587`).
- P083 migration integration tests passed: 57 DB tests, 0 failures.
- Focused engine P083 and shutdown tests passed.
- GraphQL and MCP crates compile under the P083 gate.
- MCP rollback/set-enforcement inputs include `target_enforcement_mode` / `target_mode` and `caller_request_id` (`control-plane/crates/mcp-server/src/tools/runs.rs:370-447`).
- GraphQL `CallerRequestId` scalar validation is explicit and UUIDv4/lowercase constrained (`control-plane/crates/graphql-server/src/types/p083.rs:13-65`).
- The P083 gate statically verifies monotonic-clock source usage and baseline correlation (`scripts/test-gate.sh:9603-9636`).
- The P083 gate statically verifies macOS Run menu and toolbar parity (`scripts/test-gate.sh:9808-9835`).

Material divergences:

- Proposal denial vocabulary is not byte-equal to the runtime GraphQL enum or MCP schemas.
- MCP reference output schemas require `schema_version` and `status`, while runtime tool schemas and outputs still use command-specific booleans such as `cancelled`, `resolved`, and `committed`.
- `approvals.resolve` exposes `defer` but fails closed before durable command dispatch.
- Per-command intent-hash fields are not exactly the proposal's `per_command_logical_fields` for `provider_session.shutdown` and `side_effects.force_reconcile`.
- The canonical P083 gate is not a full macOS Swift build/UI execution proof. It uses static macOS string checks for the menu/toolbar surfaces.

## Reviewer Selection and Reuse

Reviewer artifacts reused: **None**. `discover_prior_review.py` returned no artifacts for this proposal path.

Selected reviewers:

| Reviewer | Selected | Reason |
| --- | --- | --- |
| `chainworks_execution_truth_reviewer` | Yes | Mandatory repo-local reviewer for durable run/stage/agent/approval/artifact/recovery/projection/MCP/ACP truth changes. |
| `api_contract_reviewer` | Yes | GraphQL SDL, MCP inventory, JSON Schema, denial vocabulary, and readback parity are central P083 contracts. |
| `rust_reliability_reviewer` | Yes | Command idempotency, recovery, shutdown, late output, monotonic deadlines, and process fate are reliability-critical. |
| `rust_security_reviewer` | Yes | The security-sensitive diff triggered on auth/public ingress/parser/process-boundary/redaction categories. |
| `observability_rollout_reviewer` | Yes | P083 defines rollout holds, metrics, readback lanes, rollback disposition, and gate aliases. |

Not selected because of the five-reviewer cap:

| Reviewer | Disposition | Risk |
| --- | --- | --- |
| `macos_ui_reviewer` | Displaced | UI/UX scope is present; only static menu/toolbar proof was audited. This contributes to Not Ready. |
| `apple_ux_reviewer` | Displaced | Manual identity-check UX was not exercised interactively. |
| `apple_arch_reviewer` | Displaced | SwiftData lifecycle-boundary claims were not runtime-verified in this audit. |
| `rust_performance_reviewer` | Displaced | The helper fingerprint triggered performance, but P083 has no primary latency/throughput acceptance path. |
| `product_reviewer` | Displaced | Operator-facing UX exists, but readiness is already blocked on proposal/API conformance. |

## Security-Sensitive Diff Scan

Result: **Triggered and reviewed**.

Triggered categories included auth, public ingress, parser/schema boundary, filesystem/subprocess/process boundary, denial/redaction privacy, and dependency/crypto-sensitive surfaces. The manual pass covered:

- GraphQL lifecycle ingress and principal mutation policy.
- MCP lifecycle tools, input-schema rejection, output payloads, and denial mapping.
- CallerRequestId validation at GraphQL/MCP/engine boundaries.
- Provider-session shutdown and manual process-absent command paths.
- Process identity ambiguity handling, late output holds, and redaction/readback surfaces.

No independent Critical/Major security vulnerability was confirmed in this audit. The security pass does not clear the API-contract blockers below; those are correctness/readiness blockers even where the implementation fails closed.

## Specialist Coverage Matrix

| Surface | Coverage | Result |
| --- | --- | --- |
| Execution truth ownership | Proposal, migrations, command handler, gate static checks | Partial; durable authority shape is broad, but API/readback contracts still diverge. |
| API contract | GraphQL types/schema, MCP tool specs, reference schemas | Blocking gaps found. |
| Reliability | Command idempotency, shutdown, recovery, monotonic clock, gate tests | Partial; core tests pass, but exact intent-hash field contract drifts. |
| Security | Security-sensitive diff plus manual pass | Pass with residual correctness blockers. |
| Observability/rollout | Rollout contract lint, migration descriptors, gate readback checks | Partial; gate passes, but readiness holds remain. |
| macOS UI/UX | Static menu/toolbar proof and source review | Partial/not runtime verified. |

## Requirement Summary

| Requirement area | Status | Evidence / gap |
| --- | --- | --- |
| Proposal current-revision readiness gate | Missing | Proposal still says implementation may not start without fresh approve review and `blocker_count=0`. |
| Durable ownership model / SQLite authority | Partially implemented | Migrations, repos, and command handlers are broad; API/readback contract mismatches keep it partial. |
| CallerRequestId and lifecycle idempotency | Partially implemented | UUIDv4 validation and idempotency are wired; exact per-command logical fields drift for two commands. |
| Rollback target reconciliation | Implemented for the audited surfaces | Gate verifies GraphQL/MCP/intent/audit/readback target mode alignment. |
| GraphQL SDL and denial vocabulary | Partially implemented | CallerRequestId and payload types exist; DenialReason set is not the proposal set. |
| MCP lifecycle inventory and schemas | Partially implemented | Required inputs exist; runtime output schemas do not match reference output schemas. |
| Approval resolution enum | Missing for `defer` success path | `defer` is advertised but denied in GraphQL and MCP. |
| Migrations and migration readback | Implemented by gate evidence | All eight files present; descriptors verified. |
| Shutdown/cancellation/late output recovery | Partially implemented | DB and engine tests pass; no live daemon crash/restart drill was run in this audit. |
| Durable monotonic clock | Partially implemented | Static/source checks pass; no live daemon baseline sample was independently observed. |
| Metrics and rollout contract | Partially implemented | Rollout lint/gate pass; full operational readback lanes were not live-verified. |
| macOS UI/SwiftData boundary | Partially implemented / not fully verified | Static Run menu/toolbar proof passes; no Swift build/UI execution proof in this audit. |

## Detailed Findings

### READY-001 - Proposal state still blocks implementation readiness

Severity: Major
Reviewer lens: observability_rollout_reviewer / chainworks_execution_truth_reviewer
Track: Track 1 conformance and readiness

The proposal is not in an implementation-approved state. It says implementation may start only after the human implementation approval gate is granted and a fresh aggregate review returns approve with `blocker_count=0` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`). Its active readiness narrative still says `implementation_may_start=false` and cites the latest review authority as `decision=revise_required, blocker_count=1` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`).

Impact: even a technically complete implementation cannot be marked Ready under this proposal's own process contract until the current-revision review/approval gate is satisfied.

Required action: land or attach the fresh current-revision approve review, confirm `blocker_count=0`, and update the proposal/readiness authority before claiming Ready.

### API-001 - GraphQL/MCP denial vocabulary is not byte-equal to the proposal contract

Severity: Major
Reviewer lens: api_contract_reviewer
Track: Track 1 conformance

The proposal defines the GraphQL `DenialReason` set as:

`request_intent_mismatch`, `malformed_request_id`, `missing_caller_request_id`, `request_id_not_owned`, `principal_class_not_allowed`, `lifecycle_state_invalid`, `lifecycle_not_actionable`, `approval_not_pending`, `artifact_not_active`, `side_effect_not_pending`, `provider_session_not_cancellable`, `rollout_contract_disabled`, `enforcement_mode_blocked`, `identity_ambiguous`, `late_output_overflow_latched`, `schema_invalid`, `additional_properties_rejected`, `unknown_command`, `rollback_target_required`, `rollback_target_invalid`.

The runtime GraphQL enum instead includes implementation-specific values such as `IdempotencyInFlight`, `IdempotencyReplayed`, `OperatorRequired`, `ProviderSessionNotFound`, `RunNotFound`, `StageNotRetryable`, `ApprovalNotActionable`, `SideEffectNotReconcilable`, `EnforcementModeTransitionDenied`, `IdempotencyTerminalFailure`, and `Internal` (`control-plane/crates/graphql-server/src/types/p083.rs:68-95`). The runtime MCP schemas mirror that implementation set for tool outputs such as `runs.cancel` and `p083.rollback_execution` (`control-plane/crates/mcp-server/src/tools/runs.rs:93-107`, `:393-407`).

The proposal requires byte-equal parity between GraphQL and MCP and names exact denial values (`docs/proposals/083-execution-truth-ownership-invariant-model.md:157-164`, `:183`, `:212-235`). Current runtime parity may be internally consistent, but it is not byte-equal to the proposal vocabulary.

Required action: either change GraphQL/MCP runtime denial enums to the proposal vocabulary, or revise P083 and all reference schemas/fixtures to make the implementation vocabulary authoritative. Add a gate assertion that compares the proposal/reference vocabulary to the generated/runtime GraphQL and MCP sets.

### API-002 - MCP runtime output schemas do not match the P083 reference schemas

Severity: Major
Reviewer lens: api_contract_reviewer
Track: Track 1 conformance

P083 reference output schemas require a common envelope with `schema_version` and `status`, and optional structured `denial`. For example, `runs.cancel.output.schema.json` defines `schema_version`, `status`, and `denial` (`docs/reference/mcp/p083/runs.cancel.output.schema.json:1-40`), and `p083.rollback_execution.output.schema.json` requires `schema_version` and `status` (`docs/reference/mcp/p083/p083.rollback_execution.output.schema.json:41-44`).

Runtime MCP tool specs do not use that envelope. `runs.cancel` declares required `cancelled` and fields such as `denied` / `denial_code` (`control-plane/crates/mcp-server/src/tools/runs.rs:93-107`). `p083.rollback_execution` declares required `committed` and `denial_code` (`control-plane/crates/mcp-server/src/tools/runs.rs:393-407`). `approvals.resolve` declares required `resolved` (`control-plane/crates/mcp-server/src/tools/approvals.rs:115-129`).

Impact: clients generated from `docs/reference/mcp/p083/*.output.schema.json` cannot rely on the runtime tool schemas. The current P083 gate checks that reference schema files exist and that runtime code contains required input terms, but it does not compare runtime output schemas to the reference files (`scripts/test-gate.sh:9644-9719`).

Required action: wire runtime MCP output schemas/results to the reference envelope, or revise the reference schemas and proposal inventory. Add a gate that compares runtime tool inventory output_schema JSON against the reference schema files.

### API-003 - `approvals.resolve` advertises `defer` but denies it instead of implementing it

Severity: Major
Reviewer lens: api_contract_reviewer / rust_reliability_reviewer
Track: Track 1 conformance

The proposal's GraphQL/MCP contract includes `defer` in `ApprovalResolution` and in the MCP enum constraints (`docs/proposals/083-execution-truth-ownership-invariant-model.md:163`, `:205`). The implementation exposes `Defer` in GraphQL (`control-plane/crates/graphql-server/src/schema.rs:5669-5672`) and in MCP (`control-plane/crates/mcp-server/src/tools/approvals.rs:98-101`), but GraphQL returns `APPROVAL_DEFER_NOT_IMPLEMENTED` before command dispatch (`control-plane/crates/graphql-server/src/schema.rs:5904-5933`) and MCP returns an error stating `defer` is declared but not actionable (`control-plane/crates/mcp-server/src/tools/approvals.rs:339-345`).

Fail-closed behavior is good, but this is still a missing successful path for an advertised in-scope enum value. I found no proposal-owned follow-up that removes `defer` from P083 acceptance.

Required action: implement durable defer semantics end to end, or remove `defer` from the P083 public contract and reference schemas until a follow-up owns it.

### REL-001 - Command intent-hash fields drift from `per_command_logical_fields`

Severity: Major
Reviewer lens: rust_reliability_reviewer
Track: Track 1 conformance

P083 states `per_command_logical_fields` as `side_effects.force_reconcile: ["side_effect_id"]` and `provider_session.shutdown: ["provider_session_id"]` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1121-1129`). The implementation hashes additional fields:

- `provider_session.shutdown` hashes `command`, `provider_session_id`, and `reason` (`control-plane/crates/engine/src/command_handler.rs:7370-7382`).
- `side_effects.force_reconcile` hashes `command`, `decision_json_digest`, and `effect_id` (`control-plane/crates/engine/src/command_handler.rs:9163-9180`).

The `decision_json_digest` addition appears intentionally security-hardening and may be the better design, but it is not the proposal contract as written. The `reason` field is service-supplied/diagnostic in current surfaces and should not change caller intent under the proposal's rule.

Required action: either align code/tests to the proposal field list, or revise P083 to include the extra logical fields and state why they are authoritative caller intent. Then add per-command intent-hash fixture coverage for both commands.

### READY-002 - macOS UI/SwiftData scope is only statically covered by the P083 gate

Severity: Major
Reviewer lens: macos_ui_reviewer / apple_ux_reviewer / apple_arch_reviewer
Track: Track 2 coverage/readiness

P083 includes macOS read-only lifecycle enforcement, deterministic menu/toolbar placement, manual identity-check UX, and SwiftData projection boundary contracts. The P083 gate verifies static strings in `Chainworks_ForgeApp.swift` and `RunsHomeView.swift` (`scripts/test-gate.sh:9808-9835`), but it does not run a Swift build, Swift unit tests, or UI interaction proof for these surfaces.

Impact: the audit cannot mark the macOS UI/SwiftData portions Ready. Static coverage is useful, but it does not prove focus-state routing, disabled-state reasons, copy/export behavior, accessibility, or SwiftData actor/projection behavior under runtime conditions.

Required action: add or run a Swift/macOS gate for the P083 UI and SwiftData boundary, or move those claims to an explicit follow-up with acceptance evidence.

## Routed Findings

| Finding | Owner lens | Disposition |
| --- | --- | --- |
| READY-001 | observability_rollout_reviewer / chainworks_execution_truth_reviewer | Blocks Ready until proposal approval state is current and clean. |
| API-001 | api_contract_reviewer | Blocks Ready; proposal/runtime denial vocabulary mismatch. |
| API-002 | api_contract_reviewer | Blocks Ready; runtime MCP output schemas do not match reference schemas. |
| API-003 | api_contract_reviewer / rust_reliability_reviewer | Blocks full implementation; advertised `defer` path is not implemented. |
| REL-001 | rust_reliability_reviewer | Blocks exact conformance unless proposal is revised or hash fields are aligned. |
| READY-002 | macos_ui_reviewer / apple_ux_reviewer / apple_arch_reviewer | Blocks UI/SwiftData readiness evidence. |

## Readiness Checklist

| Check | Status |
| --- | --- |
| Proposal current-revision approval is clean | Fail |
| Prior review artifacts reused or explicitly unavailable | Pass; unavailable |
| Implementation preserves proposal non-goals | Pass with one note: `side_effects.force_reconcile` remains MCP/API-owned, no macOS native write path found in audited surfaces |
| Canonical proposal gate passes on audited tree | Pass |
| Security-sensitive diff independently reviewed | Pass |
| API contracts match proposal/reference schemas | Fail |
| Full-implementation tail gate satisfied | Fail |
| Specialist coverage hard gate satisfied | Fail for UI/UX/SwiftData runtime coverage |
| Runtime/live readback proof for rollout lanes | Partial |
| Ready/Ready with Risks eligible | No |

## Verification Log

Commands and checks run:

- Read `proposal-implementation-audit` skill instructions completely.
- Read repo-local reviewer routing and `chainworks_execution_truth_reviewer` instructions.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/083-execution-truth-ownership-invariant-model.md`
  - Result: no artifacts found.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/security_sensitive_diff.py --root /Users/user/Documents/Chainworks\ Forge --json`
  - Result: triggered; manual security review performed.
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/implementation_surface_fingerprint.py --root /Users/user/Documents/Chainworks\ Forge --json`
  - Result: API, architecture, reliability, security, observability/rollout, Apple UI/UX, and performance lenses triggered.
- `./scripts/test-gate.sh proposal-083`
  - Result: passed.
  - Notable passed pieces: evidence corpus verification, DB migration tests, `cargo check -p db`, focused engine P083/shutdown tests, `cargo check -p daemon`, `cargo check -p graphql-server`, `cargo check -p mcp-server`, domain denial-code round trip, GraphQL approval mutation tests, MCP P083 tests, rollout contract lint, migration/readback static checks, monotonic-clock static checks, rollback target static checks, macOS Run menu/toolbar static proof.

Validation still required before Ready:

- Runtime MCP inventory/output-schema comparison against `docs/reference/mcp/p083/*.schema.json`.
- GraphQL schema introspection comparison against proposal/reference denial vocabulary.
- Durable `approvals.resolve defer` success-path proof, or proposal/schema removal of `defer`.
- Swift/macOS build or UI proof for the P083 manual identity and lifecycle menu/toolbar behaviors.
- Live daemon/readback drill for rollout lanes if Ready requires runtime evidence rather than static gate proof.

## Final Actions

Do not close P083 as implemented. Keep it in implementation-fix state until:

1. The proposal/readiness gate is made current and clean.
2. GraphQL and MCP denial vocabulary are reconciled to one authoritative set.
3. MCP runtime output schemas match the P083 reference schemas, or the proposal/reference schemas are revised.
4. `approvals.resolve defer` is either implemented durably or removed from the public P083 contract.
5. Intent-hash field composition is aligned with the proposal or the proposal is revised to match the hardened implementation.
6. UI/SwiftData runtime evidence is added or explicitly deferred to an owned follow-up.

Final verdict: **Not Ready / Not Implemented for full proposal acceptance**.
