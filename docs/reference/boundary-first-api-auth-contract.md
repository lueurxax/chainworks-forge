# Boundary-First API and Auth Contract Matrix

**Status**: Phase 1 (Shadow) — fixture and validator landed; enforcement deferred to Phase 4.  
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
| `agent_operator` | `mcp_tools_list` | `p081.agent_operator.mcp_tools_list.discovery` | `tools/list` | `none` | denied known tools omitted; no command_journal write | `mcp_tools_list_filters_denied_tools` |
| `agent_operator` | `mcp_tools_call` | `p081.agent_operator.mcp_tools_call.command` | operator-class MCP commands | `command_journal` | JSON-RPC -32004 with CAPABILITY_OUT_OF_SCOPE or MATRIX_NO_ROW | `allowed_mcp_command`, `denied_graphql_mutation`, `denied_out_of_scope_tool`, `mcp_command_idempotency` |
| `automation` | `mcp_tools_list` | `p081.automation.mcp_tools_list.discovery` | `tools/list` | `none` | tools outside automation_capabilities omitted | `automation_tools_list_scope` |
| `automation` | `mcp_tools_call` | `p081.automation.mcp_tools_call.command` | intersection of surface_policies.mcp and automation_capabilities | `command_journal` | JSON-RPC -32004 with CAPABILITY_OUT_OF_SCOPE | `token_scope_matrix`, `denial_side_effect_test`, `mcp_command_idempotency` |
| `observer` | `mcp_tools_call` | `p081.observer.mcp_tools_call.compact_read` | read-only compact diagnostics | `projection_read_model` | OBSERVER_SCOPE for mutations; actionability forced false | `observer_cannot_mutate`, `observer_compact_read_only` |
| `observer` | `graphql_query` | `p081.observer.graphql_query.read_only_opt_in` | explicitly enabled read-only fields only | `projection_read_model` | FORBIDDEN or redaction with OBSERVER_SCOPE; actionability forced false | `observer_graphql_default_denied`, `opt_in_redaction_parity`, `accessibility_redaction_parity` |
| `developer_break_glass` | `debug_endpoint` | `p081.developer_break_glass.debug_endpoint.disabled` | diagnostic preflight (disabled) | `audit_log` | BREAK_GLASS_DISABLED; E_AUDIT_UNAVAILABLE if audit row cannot commit | `env_gate_test`, `audit_event_assertion`, `no_projection_write_test`, `audit_unavailable_fail_closed` |

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
| `IDEMPOTENCY_CONFLICT` | 409 | Same key with different canonical request hash. |

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
| Subscription denied at subscribe-start | — | `FORBIDDEN` | complete with no events |

---

## MCP Error Contract

- **Unknown tools**: JSON-RPC `-32601` (method not found).
- **Known but denied tools**: JSON-RPC `-32004` with `data.reason_code`, `caller_class`, `row_id`, `request_id`, `boundary_policy_version`.
- `tools/list` omits denied tools; decision rows never append `command_journal`.
- `initialize` exposes `boundary_policy` capability after Phase 4 shadow: `{matrix_id, schema_version, capability_schema_version: 1, mode, denied_known_tool_code: -32004, field_casing: "snake_case"}`.

---

## Atomic Commit Contract

For allowed state-changing calls, one `BEGIN IMMEDIATE` SQLite transaction commits:

1. BoundaryPolicy decision record
2. `command_journal` append (with `caller_class`, `row_id`, `request_id`, `idempotency_key`)
3. Approval settlement (for approval mutations)
4. Idempotency record
5. Other durable domain writes owned by that command write unit
6. Required `audit_log` rows

Post-commit projection rebuild may run after commit and is not part of the same transaction guarantee.
Acknowledgement of success occurs only after `sqlite3_step COMMIT` returns `SQLITE_DONE`.

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

Kill switches: `CHAINWORKS_BOUNDARY_POLICY=shadow|enforce|legacy` (startup-only; daemon restart required).
