# Boundary-First API and Auth Contract Matrix

**Status**: Implemented as repository truth for the P081 boundary contract. The matrix doc, executable fixture, validator, embedded last-known-good fallback, `audit_log` storage, `CallerClass` enum, `CallerContext.caller_class`, and principal-table `schema_version 3` reader have landed. The daemon-injected shared `BoundaryPolicy` is wired into the `CommandHandler`, GraphQL query/subscription/mutation paths, and the MCP `initialize`/`tools/list`/`tools/call` paths with mode-aware semantics (`shadow`, `enforce`, `read_only_safe_mode`, `legacy_compat`). `approval_mutation_idempotency` backs the `approveApproval` / `rejectApproval` retry contract, and the Swift operator shell injects `P081ApprovalActionAttemptStore` so approval actions reuse UUIDv7 idempotency keys across retries and app restarts until terminal success. The `mcp_command_idempotency` table plus dispatcher enforcement is wired for state-changing MCP tools and linked to `command_journal` before successful idempotency commits. GraphQL WebSocket pre-auth rejects with P081 close codes (`4401`, `4403`, `4408`), GraphQL denial responses include bounded `extensions.redactions`, and bounded operator diagnostics expose `boundaryRuntime`/`operatorAlerts` through GraphQL and MCP without raw audit rows. Swift readback preserves redaction accessibility metadata and native-alert lifecycle fields.
**Proposal**: P081-v6
**Matrix ID**: `p081-boundary-matrix-v1`
**Machine-readable fixture**: [`boundary-first-api-auth-contract.json`](boundary-first-api-auth-contract.json)

---

## Overview

This document is the authoritative boundary contract for the Chainworks Forge control plane.
It defines which callers may reach which surfaces, what the authoritative record is for each
action, how denied calls behave, and what redaction applies.

The governed macOS UI remains a **GraphQL read/subscription** surface with `approveApproval`
and `rejectApproval` as the only allowed UI mutations. All non-approval operator control
remains **MCP-owned** for operators, agents, and automations.

Request paths never read this file or the JSON fixture directly. Only daemon startup validation
uses these artifacts. Live requests consume only the injected in-memory policy instance.

---

## Caller Classes

| CallerClass | Description |
|---|---|
| `ui_operator` | macOS SwiftUI operator app authenticated with the `default-operator` bearer token via GraphQL. |
| `agent_operator` | Agent runtime (Claude Code, Codex, Gemini) authenticated via MCP with operator-class surface policies. |
| `automation` | Scripted or CI caller authenticated via MCP with constrained automation capabilities. |
| `observer` | Read-only principal with opt-in GraphQL fields or compact MCP diagnostics; actionability forced false. |
| `developer_break_glass` | Engineering access to diagnostic preflight; disabled unless env gate set and durable audit row commits. |

CallerClass is server-derived from `PrincipalClass`, transport, matched token, and resolved surface policy.
It is never written back to `principals.json` as a persisted identity field.

---

## Boundary Matrix

| Caller | Transport | Row ID | Allowed Actions | Authoritative Record | Deny Behavior | Required Tests |
|---|---|---|---|---|---|---|
| `ui_operator` | `graphql_query` | `p081.ui_operator.graphql_query.read` | `runs.get`, `approvals.get`, `artifacts.get`, operator-scoped read fields | `projection_read_model` | UNAUTHENTICATED, AMBIGUOUS_CALLER, CAPABILITY_OUT_OF_SCOPE, or field redaction | `query_allow`, `redaction_parity`, `graphql_extensions_casing` |
| `ui_operator` | `graphql_subscription` | `p081.ui_operator.graphql_subscription.subscribe` | `runs.subscribe`, `approvals.subscribe`, `artifacts.subscribe` | `projection_read_model` | connection_init denial or subscribe-start error with no events | `subscription_allow`, `subscription_denied_at_subscribe_start`, `subscription_policy_reload_reconnect` |
| `ui_operator` | `graphql_mutation` | `p081.ui_operator.graphql_mutation.approval_action` | `approveApproval`, `rejectApproval` | `approval_record` | FORBIDDEN with NON_APPROVAL_MUTATION or APPROVAL_NOT_ACTIONABLE | `allowed_approval`, `denied_non_approval`, `no_journal_write_on_deny`, `approval_idempotency` |
| `agent_operator` | `mcp_initialize` | `p081.agent_operator.mcp_initialize.capability` | `initialize` | `none` | auth failure without capability inventory | `mcp_initialize_boundary_policy_capability` |
| `agent_operator` | `mcp_tools_list` | `p081.agent_operator.mcp_tools_list.discovery` | `tools/list`, `resources.list`, `resources.templates.list` | `none` | denied known tools omitted; no command_journal write | `mcp_tools_list_filters_denied_tools` |
| `agent_operator` | `mcp_tools_call` | `p081.agent_operator.mcp_tools_call.command` | `runs.*`, `ideas.*`, `approvals.*`, `stages.*`, `reports.*`, `artifacts.*`, `steward.*`, `storage.*`, `runtime.*`, `boundary.*`, `effects.*`, `resources.*` (allow.wildcard=true) | `command_journal` | JSON-RPC -32004 with CAPABILITY_OUT_OF_SCOPE or MATRIX_NO_ROW | `allowed_mcp_command`, `denied_graphql_mutation`, `denied_out_of_scope_tool`, `mcp_command_idempotency` |
| `automation` | `mcp_tools_list` | `p081.automation.mcp_tools_list.discovery` | `tools/list`, `resources.list`, `resources.templates.list` | `none` | tools outside automation_capabilities omitted | `automation_tools_list_scope` |
| `automation` | `mcp_tools_call` | `p081.automation.mcp_tools_call.command` | `runs.*`, `ideas.*`, `approvals.*`, `stages.*`, `reports.*`, `artifacts.*`, `steward.*`, `storage.*`, `runtime.*`, `boundary.*`, `effects.*`, `resources.*` (allow.wildcard=true) | `command_journal` | JSON-RPC -32004 with CAPABILITY_OUT_OF_SCOPE | `token_scope_matrix`, `denial_side_effect_test`, `mcp_command_idempotency` |
| `observer` | `mcp_tools_call` | `p081.observer.mcp_tools_call.compact_read` | `runtime.health`, `boundary.runtime.get`, `runs.list`, `runs.get`, `approvals.list`, `approvals.get`, `stages.list`, `stages.get`, `artifacts.list`, `artifacts.get`, `reports.list`, `reports.get`, `steward.status`, `resources.list`, `resources.read` (allow.wildcard=false; redaction mode `actionability_false`) | `projection_read_model` | OBSERVER_SCOPE for mutations; actionability forced false | `observer_cannot_mutate`, `observer_compact_read_only` |
| `observer` | `graphql_query` | `p081.observer.graphql_query.read_only_opt_in` | `graphql.read_only` explicit opt-in | `projection_read_model` | FORBIDDEN or redaction with OBSERVER_SCOPE; actionability forced false | `observer_graphql_default_denied`, `opt_in_redaction_parity`, `accessibility_redaction_parity` |
| `developer_break_glass` | `debug_endpoint` | `p081.developer_break_glass.debug_endpoint.disabled` | `debug.preflight` (allow.enabled=false until env gate set) | `audit_log` | BREAK_GLASS_DISABLED; E_AUDIT_UNAVAILABLE if audit row cannot commit | `env_gate_test`, `audit_event_assertion`, `no_projection_write_test`, `audit_unavailable_fail_closed` |

---

## Denial Reason Codes

| Code | HTTP | Description |
|---|---|---|
| `UNAUTHENTICATED` | 401 | Missing or expired bearer token. |
| `AMBIGUOUS_CALLER` | 403 | Principal resolves to multiple or unknown caller classes. |
| `CAPABILITY_OUT_OF_SCOPE` | 403 | Action not in the principal's capability set for this transport. |
| `NON_APPROVAL_MUTATION` | 403 | GraphQL mutation is not `approveApproval` or `rejectApproval`. |
| `APPROVAL_NOT_ACTIONABLE` | 200 | Approval is in a terminal state or not awaiting action. |
| `OBSERVER_SCOPE` | 403 | Caller is observer and the action requires a higher class. |
| `BREAK_GLASS_DISABLED` | 403 | Break-glass endpoint requested but env gate not set. |
| `MATRIX_NO_ROW` | 500 | No matrix row matched the request; fail closed. |
| `E_AUDIT_UNAVAILABLE` | 503 | Required durable audit row could not be committed. |
| `E_FIXTURE_DIGEST_MISMATCH` | 500 | Deployed fixture digest does not match embedded fixture. |
| `SQLITE_CONTENTION_RETRY_EXHAUSTED` | 503 | SQLite busy timeout exceeded under bounded retry. |
| `IDEMPOTENCY_CONFLICT` | 409 | Same key with different canonical request hash, or replayed by a different caller fingerprint. The conflict envelope never echoes the original `approval_id` or journal id. |
| `IDEMPOTENCY_IN_FLIGHT` | — | MCP retry against a pending sentinel younger than 30 s; the caller must wait for the in-flight request to complete before retrying. |

---

## GraphQL Error Contract

Extensions use **camelCase**: `reasonCode`, `rowId`, `callerClass`, `requestId`, `redactionId`, `redactionMode`.

| Case | HTTP | extensions.code | Data |
|---|---|---|---|
| Missing/invalid token | 401 | `UNAUTHORIZED` | null |
| Authenticated resolver deny | 200 | `FORBIDDEN` | denied field null |
| Observer field redaction | 200 | — | redacted field null; extensions.redactions present |
| drop_resource | 200 | `FORBIDDEN` | resource field null |
| WebSocket connection_init invalid token | — | `UNAUTHORIZED` | close 4401 |
| WebSocket connection_init ambiguous caller | — | `FORBIDDEN` | close 4403 |
| WebSocket connection_init missing, malformed, or delayed | — | `INIT_TIMEOUT` | close 4408 |
| Subscription denied at subscribe-start | — | `FORBIDDEN` | complete with no events |
| Internal error from `approveApproval` / `rejectApproval` | 200 | `INTERNAL` | only `extensions.code = "INTERNAL"` and `extensions.requestId`; the full error chain is logged server-side and never returned to the client |

---

## MCP Error Contract

- **Unknown tools**: JSON-RPC `-32601` (method not found).
- **Known but denied tools**: JSON-RPC `-32004` with `data.reason_code`, `caller_class`, `row_id`, `request_id`, `boundary_policy_version`.
- **Internal dispatch errors**: JSON-RPC `-32603` with `message = "INTERNAL"` and `data = { code: "INTERNAL", request_id }`. The underlying error is logged server-side and not echoed to the client.
- `tools/list` omits denied tools; decision rows never append `command_journal`.
- `initialize` exposes `boundary_policy` capability after Phase 4 shadow: `{matrix_id, schema_version, capability_schema_version: 1, mode, denied_known_tool_code: -32004, field_casing: "snake_case"}`.

## Boundary Runtime Readback

Operators can inspect the active boundary runtime without reading raw audit rows:

- GraphQL query `boundaryRuntime` returns `schemaVersion = "boundary_runtime.v1"`, `matrixId`, `policyInjected`, `policyMode`, `safeModeActive`, `fixtureDigest`, and `auditLogHealth`.
- GraphQL query `operatorAlerts` returns bounded `operator_alert_v1` rows derived from boundary runtime health. The safe-mode alert uses dedupe key `p081.boundary.safe_mode_active`, severity `critical`, is not silenceable while active, and embeds only the bounded `boundaryRuntime` envelope. Each alert includes lifecycle fields (`acknowledgedAtMs`, `silencedUntilMs`, `lifecycle.state`, `lifecycle.dedupeKey`, `lifecycle.ackRequired`, `lifecycle.clearCondition`) plus a native-delivery descriptor (`deliveryKey`, `dockBadgeContribution`, `requestUserAttention`, `notificationCategory`, `dedupePolicy`) so the macOS shell can drive Dock/status/notification escalation from readback, not local inference.
- Observer GraphQL reads are denied by default. The only v1 opt-in read action is `graphql.read_only`; on that path server resolvers must null sensitive fields before response serialization and attach camelCase `extensions.redactions` entries with `redactionMode = field_null_redacted`.
- MCP `runtime.health` includes the same object at `boundaryRuntime` for compatibility. MCP `boundary.runtime.get` returns the same P081 object as top-level `snake_case` fields, matching the `initialize.boundary_policy.field_casing = snake_case` contract.
- MCP tool `operator.alerts.list` returns `operator_alerts_readback_v1` with the same bounded alert rows and lifecycle/native-delivery shape. It is allowed in `read_only_safe_mode` as a diagnostic read, while state-changing MCP calls remain denied.
- `auditLogHealth` is bounded to `schemaVersion = "audit_log_health.v1"`, aggregate row count, latest row/checkpoint identifiers, latest checkpoint hash, `integrityState` from checkpoint verification, audit writability, retention minimum, cleanup state, cleanup eligible/protected row counts, payload budget/used bytes, and the shadow coverage report reference. It never exposes raw audit rows.

P081 rollout metrics are retained by exact name for enforcement readiness:

- `p081_boundary_policy_enforcement_parity_percent`
- `boundary_policy_decisions_total`
- `boundary_policy_shadow_disagreement_total`
- `auth_ambiguous_caller_warn_total`
- `boundary_no_op_label_total`
- `boundary_policy_evaluation_error_total`
- `audit_log_append_failure_total{event_type,transport,mode}`
- `audit_log_rate_limited_total`
- `operator_alert_native_delivery_total{severity,surface,result}`; emitted by the macOS notification service for actual delivered/deduped/silenced native alert outcomes, not by server-side readback availability.
- `approval_idempotency_duplicate_total`
- `mcp_command_idempotency_replay_total`
- `mcp_command_idempotency_conflict_total`
- `approval_actionability_false_total`
- `graphql_redaction_extensions_total`
- `boundary_policy_decision_latency_ms{transport,caller_class,mode}`
- `boundary_commit_transaction_latency_ms{transport,action_kind,decision}`
- `audit_budget_cleanup_duration_ms`
- `operator_alert_clear_latency_ms{alert_id,severity}`

---

## Durable Idempotency Contract

Allowed MCP state-changing calls use a single-transaction command write-unit contract:

1. Before dispatch, the MCP server looks up `mcp_command_idempotency` by key. If a row exists with the same canonical request hash, the cached result is replayed. A pending sentinel younger than 30 s returns `IDEMPOTENCY_IN_FLIGHT`; an older pending sentinel triggers committed-unack recovery against `command_journal`. A hash mismatch returns `IDEMPOTENCY_CONFLICT`.
2. The command write unit opens a `BEGIN IMMEDIATE` SQLite transaction and inserts the pending sentinel via `mcp_command_idempotency::insert_pending_tx`. The same transaction stamps `idempotency_key`, request hash, caller class, BoundaryPolicy row id, and request id into `command_journal`, writes any durable domain rows owned by that command unit, and appends required audit rows.
3. After the transaction commits, the MCP server updates the sentinel with the result JSON and committed `command_journal_id`. Rollback before commit removes the sentinel along with the rest of the write unit.
4. Committed-unack recovery checks `command_journal` for the same idempotency key. A committed journal row returns a recovery response and updates the sentinel; no command is re-executed. If no committed journal row exists, the retry fails closed with `SQLITE_CONTENTION_RETRY_EXHAUSTED` or `IDEMPOTENCY_COMMITTED_UNACK` instead of guessing.

Post-commit projection rebuild may run after commit and is not part of the same write-unit guarantee. Acknowledgement of normal success occurs only after the command handler has committed and returned the journal-linked result.

---

## Rollout Phases

| Phase | Gate | What lands |
|---|---|---|
| 1 | `p081-matrix` | Matrix docs and JSON, fixture validator, embedded fixture, audit_log migrations, audit_log repo |
| 2 | `p081-identity` | CallerClass enum, CallerContext.caller_class, principal-table v3 parser |
| 3 | `p081-resolve` | auth::resolve derives caller_class; shadow warnings for ambiguous callers |
| 4 | `p081-surfaces` | BoundaryPolicy injected into GraphQL and MCP in shadow then enforce |
| 5 | `p081-approval` | ApprovalActionAttemptStore, idempotencyKey, typed redaction envelope |
| 6 | `p081-fixtures` | Enforce cutover, retire compat fixtures, CI citation report |

Kill switches (startup-only; daemon restart required):

- `CHAINWORKS_BOUNDARY_POLICY` selects the policy mode. Accepted values: `shadow`, `enforce`, `read_only_safe_mode`, `legacy` / `legacy_compat`. With no override and no deployed fixture path, the daemon defaults to `shadow` until Phase 4 shadow-coverage gates are met (legacy P072 guards remain authoritative; the matrix is advisory only). When a valid deployed fixture is loaded via `CHAINWORKS_BOUNDARY_POLICY_PATH` and no override is set, the default is `enforce`. Set `CHAINWORKS_BOUNDARY_POLICY=enforce` only after Phase 4 exit evidence. In `read_only_safe_mode`, GraphQL mutations and state-changing MCP calls are denied, while read/subscription paths and bounded diagnostic MCP reads such as `runtime.health`, `boundary.runtime.get`, and `operator.alerts.list` remain available.
- `CHAINWORKS_BOUNDARY_POLICY_PATH` optionally points at an absolute, non-symlink, regular-file path to a deployed boundary fixture JSON. The path must canonicalize to a location inside the trusted boundary fixture root (`${CHAINWORKS_META_ROOT:-~/.chainworks}/boundary/`); paths outside that root are rejected (SEC-M001). The daemon enters `read_only_safe_mode` (and falls back to the embedded last-known-good fixture) when the variable is set but the path is relative, a symlink, outside the trusted root, or cannot be canonicalized; when the file exceeds 1 MiB (SEC-M-002) or is not a regular file; or when the file fails to parse or validate. An unrecognized `CHAINWORKS_BOUNDARY_POLICY` value also coerces the daemon into `read_only_safe_mode` so a typo cannot silently fall back to `shadow`.
- Audit checkpoint verification runs once at startup. If `verify_latest_checkpoint` returns `tamper_suspected`, the daemon replaces the constructed `BoundaryPolicy` with `read_only_safe_mode` so state-changing calls remain denied until an operator confirms or repairs the audit trail. `degraded` and `verified` outcomes do not change the policy mode.

---

## Swift macOS Surface

The SwiftUI operator app remains the only GraphQL mutation caller (approve/reject only).
For the macOS-side contract — accessibility parity for redacted-nil / drop_resource /
actionability_false, the `ApprovalActionAttemptStore` idempotency-key ownership rules,
window-state restoration after `4408 POLICY_RELOAD`, and macOS-native critical alert
delivery (Dock badge, `NSStatusItem`/`MenuBarExtra`, `UNUserNotificationCenter`) — see
[swift-macos-boundary-contract.md](swift-macos-boundary-contract.md).

---

## Coverage Guardrail

`scripts/check-boundary-coverage.sh` runs as part of the `guardrails` test-gate and the
P081 proposal gate. It fails in-scope changes that touch the boundary surface without
satisfying one of:

- Both `docs/reference/boundary-first-api-auth-contract.json` and
  `docs/reference/boundary-first-api-auth-contract.md` touched in the same change.
- A `matrix_row` citation comment in a changed Rust file (e.g.
  `// matrix_row: p081.ui_operator.graphql_mutation.approval_action`).
- A `boundary-no-op` label comment in a changed Rust file.

In-scope paths include `control-plane/crates/auth/`, `control-plane/crates/graphql-server/`,
`control-plane/crates/mcp-server/`, `control-plane/crates/engine/`,
`control-plane/crates/db/src/repos/audit_log*`, and the P081 audit/caller-class
migrations (`068_p081_audit_log.sql`, `069_p081_audit_log_checkpoints.sql`,
`070_p081_caller_class.sql`, `071_p081_approval_idempotency.sql`,
`072_p081_fix_payload_length_check.sql`,
`073_p081_approval_idempotency_request_hash.sql`,
`074_p081_mcp_command_idempotency.sql`,
`075_p081_command_journal_idempotency.sql`).

`scripts/validate-p081-canaries.py` parses
`docs/evidence/boundary-policy-shadow-coverage/boundary-policy-canaries.yaml` as
the fixed `boundary_policy_canaries_v1` subset and fails closed on duplicate
keys, unknown fields, malformed entries, matrix rows missing from the live
fixture, or drift against
`docs/evidence/boundary-policy-shadow-coverage/report.json`. It runs from the
`proposal-081` gate so canary rows contribute to the same
`boundary_policy_shadow_coverage_report_v1` schema as live observations.

---

## Migrations

P081 storage lands as eight additive migrations under
`control-plane/crates/db/migrations/`:

| File | Purpose |
|---|---|
| `068_p081_audit_log.sql` | `audit_log` table with hash-chained row_hash/prev_hash columns and indexes |
| `069_p081_audit_log_checkpoints.sql` | `audit_log_checkpoints` window-verification table |
| `070_p081_caller_class.sql` | Adds `caller_class` to `command_journal` and supporting indexes |
| `071_p081_approval_idempotency.sql` | `approval_mutation_idempotency` table backing Phase 5 `approveApproval` / `rejectApproval` retry contract |
| `072_p081_fix_payload_length_check.sql` | Rebuilds `audit_log` so the `payload` CHECK measures BLOB byte length (matches the Rust 16 KiB guard for multibyte payloads) |
| `073_p081_approval_idempotency_request_hash.sql` | Adds nullable `request_hash` column to `approval_mutation_idempotency` so replays (same key, same hash) are distinguishable from `IDEMPOTENCY_CONFLICT` (same key, different hash) without re-settling |
| `074_p081_mcp_command_idempotency.sql` | `mcp_command_idempotency` table backing the state-changing MCP tool retry contract (one record per `idempotency_key` with 7-day retention) |
| `075_p081_command_journal_idempotency.sql` | Adds nullable `mcp_idempotency_key` and `boundary_row_id` columns (plus partial index on `mcp_idempotency_key`) to `command_journal` so MCP state-changing writes bind the durable command row to the boundary-derived idempotency key and matched matrix row |

Repository code lives at `control-plane/crates/db/src/repos/audit_log.rs`,
`control-plane/crates/db/src/repos/approval_mutation_idempotency.rs`, and
`control-plane/crates/db/src/repos/mcp_command_idempotency.rs`. Boundary
policy implementation and the embedded last-known-good fixture live under
`control-plane/crates/auth/src/boundary/`. MCP idempotency enforcement
(require key on state-changing tools, reject key on read-only tools, replay
cached result on duplicate hash, `IDEMPOTENCY_CONFLICT` on hash mismatch) lives
in `control-plane/crates/mcp-server/src/server.rs`.
