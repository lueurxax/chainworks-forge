# Proposal 081: Boundary-First API and Auth Contract Matrix

> Source: current unfinished-run proposal artifact.

## Metadata

- **Source run:** `4dd7c886-e7b4-4f6d-8afe-f76d63bf268d`
- **Source artifact:** `.chainworks/runs/4dd7c886-e7b4-4f6d-8afe-f76d63bf268d/proposals/approved/proposal.md`
- **Source md5:** `21dea7eafb2772672d21ae1142b530d6`
- **Proposal Id:** 081
- **Proposal Revision Id:** 081-v6
- **Status:** revised_for_review_blocker_closure
- **Run Id:** 4dd7c886-e7b4-4f6d-8afe-f76d63bf268d
- **Source Review Pass Id:** 185f009d-f6cb-4e10-99e1-9f12b25289bd
- **Supersedes Revision Id:** 081-v5

## Summary

At the current HEAD baseline, Chainworks Forge remains the product described in docs/reference/current-system-baseline.md: the governed macOS UI is a GraphQL read/subscription surface with approveApproval and rejectApproval as the only allowed UI mutations, while all non-approval operator control remains MCP-owned. P081 does not claim that HEAD already enforces a unified matrix-backed boundary policy or boundary-aware principal-table format. Instead it defines the post-merge contracts that add shared caller classification, boundary-policy evaluation, durable audit semantics, and runtime readback across GraphQL, MCP, approvals, auth resolution, command journal, and audit surfaces without changing the existing UI action boundary. This run-local revision clarifies the Rust ownership boundary for the shared BoundaryPolicy service, preserves existing schema_version 2 principal-table behavior by introducing schema_version 3 for boundary-aware transport and caller policy, and makes the audit_log contract implementable against the current control-plane migration, transaction, command-handler, and bounded readback architecture.

## Problem

- **Current State:** Authorization behavior is split across GraphQL guards, resolver allow lists, MCP tool filtering, MCP call handlers, approval actionability, token resolution, command-journal stamping, and operator presentation. These surfaces can drift independently.
- **Risk:** Drift can over-permit mutations, leak MCP capability inventory, show misleading approval actionability, drop audit evidence, obscure redactions as ordinary nulls, and make it impossible to reconstruct who was allowed to perform a durable action.
### Why Now
- PR #3 on 2026-04-29 found drift across GraphQL mutation allow-listing, MCP capability filtering, approval actionability, observer redaction, and caller provenance.
- P068 and P072 introduced role-on-transport distinctions without a shared typed caller class.
- The latest review found the direction sound but blocked readiness on executable schema contradictions, audit tamper-evidence omissions, macOS alert delivery, and accessibility evidence.

## Goals

- Create canonical human-readable and machine-readable boundary matrix artifacts at docs/reference/boundary-first-api-auth-contract.md and docs/reference/boundary-first-api-auth-contract.json.
- Define a server-derived CallerClass enum and require CallerContext.caller_class for dispatch decisions.
- Route GraphQL queries, subscriptions, mutations, MCP initialize, MCP tools/list, MCP tools/call, and approval actionability through BoundaryPolicy.
- Keep the SwiftUI operator app an approval-only mutation surface through approveApproval and rejectApproval; agents and automations use MCP for command and control.
- Make denial behavior, redaction, idempotency, audit, runtime readback, alerts, tests, migration, rollout, and rollback executable enough that implementation does not invent semantics.
- Provide macOS-native critical alert delivery and accessibility parity for redacted nil, drop_resource, and actionability_false states.

## Non Goals

- Do not add UI write behavior beyond approveApproval and rejectApproval.
- Do not make GraphQL the agent or automation control plane.
- Do not move operator UI control into MCP.
- Do not introduce a production developer_break_glass endpoint; any real debug data access needs a separate proposal.
- Do not remove compatibility fixtures before phase 6.
- Do not require local UI smoke tests in proposal-readiness mode.
- Do not replace bearer-token storage with hashed-token auth in P081; the principal-table compatibility sections document current bearer_token representation and redaction rules only.
- Do not ship a broad audit-log browser; P081 adds bounded health, alert, and diagnostic readback.
- Do not reinterpret or break the existing P072 schema_version 2 principal-table shape; P081 introduces boundary-aware transport and caller policy only in schema_version 3.
- Do not change the current governed UI boundary from GraphQL reads/subscriptions plus approveApproval and rejectApproval, and do not move non-approval operator control out of MCP.

## Ux Ui Notes

- **Alerts:** Critical alerts must remain visible even when the main window is hidden or inactive through the macOS alert delivery contract.
### Denial Copy
- **Ambiguous Caller:** Access Could Not Be Verified
- **Approval Not Actionable:** Approval Not Actionable
- **Break Glass Disabled:** Debug Access Disabled
- **Capability Out Of Scope:** Action Not Available
- **E Audit Unavailable:** Audit Storage Unavailable
- **Matrix No Row:** Access Rule Missing
- **Non Approval Mutation:** GraphQL Action Blocked
- **Observer Scope:** Read-Only Access
- **Sqlite Contention Retry Exhausted:** Storage Busy
- **Unauthenticated:** Session Expired
- **Diagnostics:** Raw reason_code, row_id, caller_class, request_id, and redaction_id remain available in copied diagnostics, but primary operator copy uses human titles and troubleshooting text. Diagnostics must exclude bearer tokens, token ids by default, principal secrets, raw fixture contents, and route inventory for unauthenticated users.
### Operator Boundary
- The SwiftUI operator app continues reading run, approval, artifact, recovery, boundaryRuntime, and operatorAlerts state through GraphQL.
- GraphQL mutations remain limited to approveApproval and rejectApproval.
- Observer principals never receive actionable approval controls; opt-in observer GraphQL read forces actionability false and redacts sensitive fields.
- **Redaction:** GraphQL extensions.redactions paths render through RedactionState or equivalent. Redacted nulls must not look like ordinary missing data. drop_resource primary-object redaction renders Restricted View or Permission Denied and invalidates stale selected content.

## Architecture

### Approval Idempotency Contract
- **Api Change:** Phase 5 requires client-supplied idempotencyKey. Phase 4 may tolerate optional server-generated compatibility keys but cannot pass phase 5 exit with them.
- **Scope:** approveApproval and rejectApproval GraphQL mutations.
- **Semantics**
  - Same caller, approval_id, action, and idempotencyKey after committed success returns the original result and does not re-emit command_journal, settlement, durable domain writes from that command unit, or primary audit rows.
  - Duplicate attempts write approval_idempotency_duplicate audit_log when audit storage is available; inability to write that duplicate audit row must not cause a second settlement.
  - Already terminal approval with a different idempotencyKey returns APPROVAL_NOT_ACTIONABLE with no settlement side effects.
  - Terminal-state precheck, idempotency lookup, command_journal append, settlement, projection write, and required audit writes happen in one transaction.
- **Shape:** idempotencyKey is UUIDv7 per operator action attempt.
- **Storage:** approval_mutation_idempotency stores idempotency_key, approval_id, action, caller_fingerprint, request_id, request_hash, result_hash, committed_at_ms, and expires_at_ms. Retention is at least 7 days and at least as long as ApprovalActionAttemptStore persistence.
### Atomic Commit Contract
- **Failure Behavior**
  - Commit failure returns a retriable internal error with request_id and no success acknowledgement.
  - Panic or IO error before successful COMMIT rolls back the transaction; clients may retry with the same idempotency key.
  - SIGTERM before successful COMMIT drains or rolls back without ACK. SIGTERM after successful COMMIT but before ACK is treated as committed-unack; post-restart retry returns the original result through the idempotency table.
  - Failure to insert an audit-required row fails the entire action closed.
- **Order**
  - Begin IMMEDIATE transaction.
  - Resolve principal and caller_class from the same loaded principal table generation used by dispatch.
  - Evaluate BoundaryPolicy in memory.
  - For denied calls, write only matrix-declared deny side effects; no command_journal rows, approval settlements, or other business-truth writes are allowed on deny.
  - For allowed command-producing calls, append command_journal with caller_class, row_id, request_id, and idempotency_key where present before other durable domain writes in that command unit.
  - For approveApproval and rejectApproval, check pending approval state and idempotency under the same transaction before settlement.
  - Apply approval settlement, idempotency record, other durable domain writes owned by that command unit, and required audit rows.
  - Acknowledge success only after sqlite3_step COMMIT returns SQLITE_DONE or equivalent successful commit result.
- **Scope:** All state-changing allowed calls use one SQLite transaction for the policy decision record, command_journal append, approval settlement, idempotency writes, other durable domain writes owned by that command unit, and any required audit rows. Post-commit projection rebuild or readback refresh may run after commit and is not promised as part of that same write transaction.
- **Time Budget**
  - **Boundary Policy Timeout Ms:** `25`
  - **Retry Chain Budget Ms:** `310`
  - **Sigterm Drain Deadline Ms:** `10000`
  - **Sqlite Busy Timeout Ms:** `250`
  - **Transaction Deadline Ms:** `1500`
### Audit Log Contract
- **Checkpoint Table**
  - **Columns**
    - checkpoint_id TEXT NOT NULL PRIMARY KEY UUIDv7
    - checkpoint_seq INTEGER NOT NULL UNIQUE
    - covered_start_id TEXT NOT NULL
    - covered_end_id TEXT NOT NULL
    - covered_row_count INTEGER NOT NULL
    - previous_checkpoint_hash TEXT NULL
    - checkpoint_hash TEXT NOT NULL
    - created_at_ms INTEGER NOT NULL
  - **Semantics:** A checkpoint is written every 1000 audit_log rows and at clean shutdown if the open window is non-empty. Startup verifies the latest checkpoint window and reports verified, degraded, or tamper_suspected.
  - **Table:** audit_log_checkpoints
- **Columns**
  - id TEXT NOT NULL PRIMARY KEY UUIDv7
  - request_id TEXT NOT NULL
  - timestamp_ms INTEGER NOT NULL
  - event_type TEXT NOT NULL
  - principal_id TEXT NULL
  - principal_class TEXT NULL
  - caller_class TEXT NULL
  - token_id TEXT NULL
  - transport TEXT NOT NULL
  - action_attempted TEXT NOT NULL
  - decision TEXT NOT NULL
  - denial_reason_code TEXT NULL
  - row_id TEXT NULL
  - env_gate_state TEXT NULL
  - source_ip_hash_or_local_process_id TEXT NULL
  - boundary_policy_mode TEXT NOT NULL
  - fixture_version TEXT NOT NULL
  - payload_schema_version INTEGER NOT NULL DEFAULT 1
  - payload TEXT NOT NULL
  - payload_sha256 TEXT NOT NULL
  - diagnostic_truncated INTEGER NOT NULL DEFAULT 0
  - prev_hash TEXT NULL
  - row_hash TEXT NOT NULL
  - checkpoint_id TEXT NULL
  - created_at_ms INTEGER NOT NULL
- **Hash Chain:** row_hash = sha256(canonical audit row fields excluding row_hash plus prev_hash). prev_hash is the previous committed audit_log row_hash within the same database. checkpoint_hash = sha256(previous_checkpoint_hash plus covered row_hash sequence plus checkpoint metadata).
- **Indexes**
  - UNIQUE(id)
  - UNIQUE(row_hash)
  - audit_log_request_id_idx(request_id)
  - audit_log_time_idx(timestamp_ms)
  - audit_log_reason_idx(denial_reason_code,row_id)
  - audit_log_principal_idx(principal_id,caller_class)
  - audit_log_checkpoint_idx(checkpoint_id)
- **Migration:** Phase 1 lands the audit tables as the next additive numbered SQL migration file or files under control-plane/crates/db/migrations/ after the current HEAD tail (which presently ends at 045_p084_rollout_contract_readback.sql). The implementation must follow the repository's existing numbered SQL migration convention rather than introducing ad hoc startup DDL. The migration adds audit_log and audit_log_checkpoints before any developer_break_glass row can be enforced. Before phase 4 it can be rolled back by dropping both tables; after phase 4 rollback keeps the tables and disables new writes only through the documented policy mode.
- **Payload Contract:** payload is canonical JSON capped at 16 KiB. If raw diagnostics exceed the cap, payload stores canonical {diagnostic_truncated:true,payload_sha256,original_size_bytes,allowed_keys} and diagnostic_truncated=1. payload_sha256 is the SHA-256 of the untruncated canonical payload when available, otherwise of the stored truncated payload envelope.
- **Readback:** audit_log_readback_v1 is a bounded diagnostic payload shape, not an unrestricted query surface. P081 does not add a broad GraphQL audit browser or raw table browser. audit_log is an adjunct durable append-evidence table, not the business source of truth for approvals, stages, work items, or projections; operator readback remains limited to health, integrity, and targeted diagnostic surfaces aligned with existing daemonStatus, /health, GraphQL operator diagnostic lanes, and MCP/runtime readback conventions. Those bounded payloads may expose nullable callerClass, compatibility callerPrincipalClass, payloadSha256, diagnosticTruncated, rowHash, checkpointId, integrityState, retention state, and last-error summaries when already scoped to a specific diagnostic or health surface.
- **Retention:** Minimum local retention is 90 days unless an existing stronger policy applies. Cleanup runs outside request-handling transactions, deletes only complete checkpoint windows older than retention, records cleanup progress and lag metrics, and surfaces degraded or stalled cleanup through bounded audit health readback rather than ad hoc table inspection.
- **Schema Version:** `1`
- **Table:** audit_log
- **Unavailable Storage:** Ordinary denial telemetry may still go to structured logs if audit_log is unreachable, but any action requiring durable audit fails closed. developer_break_glass exposes no data or boundary decision unless its audit row commits.
- **Write Semantics:** db::repos::audit_log owns append_tx for writes inside an existing caller transaction and a bounded standalone append path for deny-only durable audit writes once the relevant seam has bounded DB access. It never logs bearer_token values. Duplicate audit ids, hash-chain write failures, or inability to commit the required audit transaction fail the request closed at seams where durable audit is part of the implemented contract.
- **Repository Contract**
  - **Module:** db::repos::audit_log
  - **Pattern:** Match existing db repo patterns such as command_journal by exposing transactional helpers for writes that are already inside an engine-owned write unit and bounded pool-based helpers for standalone durable audit writes.
  - **Required Functions**
    - append_tx(&mut Transaction<'_, Sqlite>, entry, context) for allowed mutating paths that are already inside a BEGIN IMMEDIATE transaction
    - append(pool, entry, context) or append_with_retry(pool, entry, context) for deny-only non-command paths that must open their own bounded audit transaction
    - append_checkpoint_tx(...) for checkpoint rows written alongside audited windows when required
    - bounded health/readback helpers for integrity and retention state rather than unrestricted row browsing
  - **Write Scope:** The repo never logs bearer_token values and owns row-hash, prev-hash, checkpoint linkage, and truncation envelope construction so each caller does not reimplement audit serialization rules.
- **Transaction Coupling**
  - **Allowed Mutating Paths:** Allowed state-changing calls that require durable audit write their audit row in the same BEGIN IMMEDIATE SQLite transaction as command_journal, idempotency, approval settlement, and other durable side effects owned by that command write unit. This matches the current control-plane write-unit pattern built on db::pool::begin_immediate_with_retry(...) and existing transactional repos such as command_journal::record_tx / complete_entry_tx / fail_entry_tx. Projection rebuild or readback refresh may happen after commit and is not part of the same transaction guarantee.
  - **Approval Path Anchor:** GraphQL approveApproval and rejectApproval continue to enter through graphql-server/src/schema.rs and call the engine command path, where approval settlement already occurs inside one write transaction. P081 adds audit append_tx to that same transaction rather than introducing a second write unit.
  - **Deny Only Paths:** Deny-only non-command paths that still require exactly one durable audit row open a bounded SQLite write transaction solely for the audit row using db::pool::begin_immediate_with_retry(...) once that transport or pre-dispatch seam has explicit bounded DB pool plumbing. They commit that row before returning the denial and produce no command_journal, approval-settlement, or other business-truth side effects.
  - **Fail Closed Rule:** If a required durable audit write cannot commit in either path, the request fails closed and returns the appropriate denied or unavailable contract instead of acknowledging success or exposing break-glass data. For transport-level denial seams, that fail-closed behavior becomes enforceable only after the implementation adds the required bounded DB access to the seam; until then those rows remain target-contract rollout gates rather than overclaims about current signatures.
  - **Exactly One Row Rule:** A deny path that requires durable audit must commit exactly one primary audit_log row for that request. Retries may emit a separately named duplicate-attempt event only when the contract explicitly calls for it; they must not create multiple primary deny rows for one logical attempt.
### Boundary Matrix
- **Artifacts**
  - docs/reference/boundary-first-api-auth-contract.md
  - docs/reference/boundary-first-api-auth-contract.json
- **Fixture Schema**
  - **Name:** boundary_matrix_fixture_v1
  - **Required Row Fields**
    - row_id
    - caller_class
    - transports
    - actions
    - allow
    - deny
    - redaction
    - authoritative_record
    - read_model_delta
    - required_tests
    - rollout_mode
    - deprecated_after_phase
  - **Required Top Level Fields**
    - schema_version
    - matrix_id
    - generated_from
    - enum_casing
    - rows
  - **Schema Version:** `1`
  - **Validation**
    - Reject unknown fields at every nested level.
    - Reject duplicate row_id values.
    - Reject unknown enum values.
    - Reject invalid schema_version.
    - Reject missing required rows.
    - Reject wildcard action ids unless allow.wildcard is true.
    - Reject rows whose deny.side_effects contradict the side-effect contract.
    - Reject required_rows whose row_id or transports do not validate against executable_boundary_contract.
    - CI validates both checked-in fixture and build-time embedded last-known-good fixture.
- **Required Rows**
  1. **Item**
     - **Allowed Actions**
       - runs.get
       - approvals.get
       - artifacts.get
       - operator-scoped read fields
     - **Authoritative Record:** projection_read_model
     - **Caller Class:** ui_operator
     - **Deny Behavior:** UNAUTHENTICATED, AMBIGUOUS_CALLER, CAPABILITY_OUT_OF_SCOPE, or field redaction before resolver data exposure
     - **Required Tests**
       - query_allow
       - redaction_parity
       - graphql_extensions_casing
     - **Row Id:** p081.ui_operator.graphql_query.read
     - **Transport:** graphql_query
  2. **Item**
     - **Allowed Actions**
       - runs.subscribe
       - approvals.subscribe
       - artifacts.subscribe
     - **Authoritative Record:** projection_read_model
     - **Caller Class:** ui_operator
     - **Deny Behavior:** connection_init denial or subscribe-start operation error with no events
     - **Required Tests**
       - subscription_allow
       - subscription_denied_at_subscribe_start
       - subscription_policy_reload_reconnect
     - **Row Id:** p081.ui_operator.graphql_subscription.subscribe
     - **Transport:** graphql_subscription
  3. **Item**
     - **Allowed Actions**
       - approveApproval
       - rejectApproval
     - **Authoritative Record:** approval_record
     - **Caller Class:** ui_operator
     - **Deny Behavior:** FORBIDDEN with NON_APPROVAL_MUTATION or APPROVAL_NOT_ACTIONABLE before side effects
     - **Required Tests**
       - allowed_approval
       - denied_non_approval
       - no_journal_write_on_deny
       - approval_idempotency
     - **Row Id:** p081.ui_operator.graphql_mutation.approval_action
     - **Transport:** graphql_mutation
  4. **Item**
     - **Allowed Actions**
       - initialize
     - **Authoritative Record:** none
     - **Caller Class:** agent_operator
     - **Deny Behavior:** auth failure without capability inventory
     - **Required Tests**
       - mcp_initialize_boundary_policy_capability
     - **Row Id:** p081.agent_operator.mcp_initialize.capability
     - **Transport:** mcp_initialize
  5. **Item**
     - **Allowed Actions**
       - tools/list
     - **Authoritative Record:** none
     - **Caller Class:** agent_operator
     - **Deny Behavior:** denied known tools omitted; no command_journal write
     - **Required Tests**
       - mcp_tools_list_filters_denied_tools
     - **Row Id:** p081.agent_operator.mcp_tools_list.discovery
     - **Transport:** mcp_tools_list
  6. **Item**
     - **Allowed Actions**
       - operator-class MCP commands scoped by surface_policies.mcp
     - **Authoritative Record:** command_journal
     - **Caller Class:** agent_operator
     - **Deny Behavior:** JSON-RPC -32004 with CAPABILITY_OUT_OF_SCOPE or MATRIX_NO_ROW
     - **Required Tests**
       - allowed_mcp_command
       - denied_graphql_mutation
       - denied_out_of_scope_tool
       - mcp_command_idempotency
     - **Row Id:** p081.agent_operator.mcp_tools_call.command
     - **Transport:** mcp_tools_call
  7. **Item**
     - **Allowed Actions**
       - tools/list
     - **Authoritative Record:** none
     - **Caller Class:** automation
     - **Deny Behavior:** tools outside automation_capabilities omitted
     - **Required Tests**
       - automation_tools_list_scope
     - **Row Id:** p081.automation.mcp_tools_list.discovery
     - **Transport:** mcp_tools_list
  8. **Item**
     - **Allowed Actions**
       - intersection of surface_policies.mcp and automation_capabilities
     - **Authoritative Record:** command_journal
     - **Caller Class:** automation
     - **Deny Behavior:** JSON-RPC -32004 with CAPABILITY_OUT_OF_SCOPE
     - **Required Tests**
       - token_scope_matrix
       - denial_side_effect_test
       - mcp_command_idempotency
     - **Row Id:** p081.automation.mcp_tools_call.command
     - **Transport:** mcp_tools_call
  9. **Item**
     - **Allowed Actions**
       - read-only compact diagnostics
     - **Authoritative Record:** projection_read_model
     - **Caller Class:** observer
     - **Deny Behavior:** OBSERVER_SCOPE for mutations; actionability forced false
     - **Required Tests**
       - observer_cannot_mutate
       - observer_compact_read_only
     - **Row Id:** p081.observer.mcp_tools_call.compact_read
     - **Transport:** mcp_tools_call
  10. **Item**
     - **Allowed Actions**
       - explicitly enabled read-only fields only
     - **Authoritative Record:** projection_read_model
     - **Caller Class:** observer
     - **Deny Behavior:** FORBIDDEN or redaction with OBSERVER_SCOPE; actionability forced false
     - **Required Tests**
       - observer_graphql_default_denied
       - opt_in_redaction_parity
       - accessibility_redaction_parity
     - **Row Id:** p081.observer.graphql_query.read_only_opt_in
     - **Transport:** graphql_query
  11. **Item**
     - **Allowed Actions**
       - diagnostic preflight readback only if separately enabled later
     - **Authoritative Record:** audit_log
     - **Caller Class:** developer_break_glass
     - **Deny Behavior:** BREAK_GLASS_DISABLED when env gate absent; E_AUDIT_UNAVAILABLE if durable audit row cannot be written
     - **Required Tests**
       - env_gate_test
       - audit_event_assertion
       - no_projection_write_test
       - audit_unavailable_fail_closed
     - **Row Id:** p081.developer_break_glass.debug_endpoint.disabled
     - **Transport:** debug_endpoint
### Boundary Policy
- **Rust Home:** BoundaryPolicy and MatrixBoundaryPolicy live in an auth::boundary module/layer inside the existing control-plane auth crate, not inside graphql-server, mcp-server, or engine-specific crates. That auth::boundary layer owns boundary fixture loading and validation, policy types, the decision API, and the immutable validated matrix payload.
- **Service Boundary:** The auth::boundary service exposes a synchronous in-memory decision interface consumed by GraphQL, MCP, and approval actionability. graphql-server and mcp-server depend on the auth crate's shared boundary layer; approval actionability and command-handling code receive the same service through daemon wiring rather than ad hoc fixture access.
- **Daemon Injection Model:** Daemon startup validates the boundary matrix fixture and principal-table inputs, constructs one immutable MatrixBoundaryPolicy instance plus supporting resolvers, wraps it in Arc<dyn BoundaryPolicy> or equivalent shared service ownership, and injects that same instance into GraphQL server construction, MCP server construction, and approval actionability/command paths. The process lifetime instance is immutable; policy mode or fixture changes require restart.
- **Call Sites**
  - GraphQL query guards
  - GraphQL subscription guards
  - GraphQL mutation resolvers before side effects
  - MCP initialize
  - MCP tools/list
  - MCP tools/call
  - Approval ActionabilityProjection::for_caller
- **Decision Fields**
  - allowed
  - row_id
  - denial_reason_code
  - redaction
  - authoritative_record
  - read_model_delta
  - audit_required
  - side_effects
- **Implementation:** MatrixBoundaryPolicy loads a validated in-memory matrix at daemon startup and is then shared unchanged by every request path. Per-request evaluation is pure in-memory and performs no disk, network, or reference-doc I/O.
- **Request Path Io:** Request paths must not read the fixture, docs/reference/current-system-baseline.md, or other reference files directly. Those artifacts guide startup validation and review, while live requests consume only the injected immutable policy instance and already-loaded auth inputs.
- **Latency Budget:** p95 under 1 ms and p99 under 5 ms per decision after startup. If evaluation exceeds 25 ms or panics, fail closed with MATRIX_NO_ROW, emit boundary_policy_evaluation_error_total, and perform no mutating side effects.
### Caller Identity
- **Principal Class Values**
  - operator
  - agent
  - observer
- **Caller Class Values**
  - ui_operator
  - agent_operator
  - automation
  - observer
  - developer_break_glass
- **Persisted Identity Truth:** PrincipalClass remains the persisted identity truth in principal-table storage and audit provenance. P081 does not replace PrincipalClass with CallerClass in stored auth fixtures or historical records.
- **Rule:** CallerClass is a request-scoped server-derived classification computed from authenticated principal-table truth, the matched token entry, transport, and resolved surface policy. Caller-supplied provenance never sets caller_class.
- **Caller Context:** CallerContext carries principal_id, principal_class, caller_class, transport, token_id, request_id, and labels. Dispatch fails closed with AMBIGUOUS_CALLER outside phase 3 warn mode. caller_class is derived after principal resolution and is not written back into principals.json as the canonical identity field.
- **Compatibility Note:** Persisted compatibility fields such as callerPrincipalClass remain readback-compatible while callerClass becomes the clearer runtime classification for new contracts. The first bounded readback surfaces that must carry both fields are audit_log_readback_v1 diagnostic payloads and compatibility GraphQL/MCP operator diagnostic lanes that expose audit-derived denial or settlement evidence.
### Executable Boundary Contract
- **Action Grammar:** actions are exact action ids like approveApproval, rejectApproval, runs.get, tools/list, initialize, or namespace.tool. Wildcards are forbidden unless allow.wildcard=true and action is namespace.*; global * is invalid.
- **Enum Registry**
  - **Authoritative Record**
    - projection_read_model
    - approval_record
    - command_journal
    - audit_log
    - none
  - **Caller Class**
    - ui_operator
    - agent_operator
    - automation
    - observer
    - developer_break_glass
  - **Decision**
    - allow
    - deny
    - redact
    - drop_resource
  - **Denial Reason Code**
    - UNAUTHENTICATED
    - AMBIGUOUS_CALLER
    - CAPABILITY_OUT_OF_SCOPE
    - NON_APPROVAL_MUTATION
    - APPROVAL_NOT_ACTIONABLE
    - OBSERVER_SCOPE
    - BREAK_GLASS_DISABLED
    - MATRIX_NO_ROW
    - E_AUDIT_UNAVAILABLE
    - E_FIXTURE_DIGEST_MISMATCH
    - SQLITE_CONTENTION_RETRY_EXHAUSTED
    - IDEMPOTENCY_CONFLICT
  - **Redaction Mode**
    - none
    - field_null_redacted
    - drop_resource
    - actionability_false
  - **Rollout Mode**
    - shadow
    - enforce
    - read_only_safe_mode
    - legacy_compat
  - **Transport**
    - graphql_query
    - graphql_subscription
    - graphql_mutation
    - mcp_initialize
    - mcp_tools_list
    - mcp_tools_call
    - debug_endpoint
- **Fixture Name:** boundary_matrix_fixture_v1
- **Invalid Examples**
  1. **combined_transport_required_row**
     - **Error:** E_REQUIRED_ROW_TRANSPORT_MISMATCH
     - **Name:** combined_transport_required_row
     - **Row Id:** p081.ui_operator.graphql_query.read
     - **Transports**
       - graphql_query
       - graphql_subscription
  2. **old_combined_transport_value**
     - **Error:** E_UNKNOWN_ENUM
     - **Name:** old_combined_transport_value
     - **Row Id:** p081.ui_operator.graphql_query.read
     - **Transports**
       - graphql_query_or_subscription
  3. **old_compact_transport_value**
     - **Error:** E_INVALID_ROW_ID
     - **Name:** old_compact_transport_value
     - **Row Id:** p081.observer.compact_read.read_only
     - **Transports**
       - mcp_compact_reads_default_graphql_read_only_opt_in
  4. **wildcard_without_flag**
     - **Actions**
       - runs.*
     - **Allow**
       - **Enabled:** `true`
       - **Wildcard:** `false`
     - **Error:** E_WILDCARD_NOT_ALLOWED
     - **Name:** wildcard_without_flag
     - **Row Id:** p081.agent_operator.mcp_tools_call.command
  5. **unknown_nested_field**
     - **Error:** E_UNKNOWN_FIELD
     - **Name:** unknown_nested_field
     - **Redaction**
       - **Extra:** bad
       - **Mode:** field_null_redacted
       - **Paths**
         - $.secret
     - **Row Id:** p081.observer.graphql_query.read_only_opt_in
- **Nullability**
  - **Actions:** required non-empty action array
  - **Allow:** required object {enabled:boolean,wildcard:boolean=false,conditions:array}
  - **Authoritative Record:** required enum
  - **Caller Class:** required enum
  - **Deny:** required object {reason_code enum,side_effects array,client_visibility enum}
  - **Deprecated After Phase:** nullable integer 1..6
  - **Read Model Delta:** required object
  - **Redaction:** required object {mode enum,paths array,extensions_required boolean}
  - **Required Tests:** required non-empty string array
  - **Rollout Mode:** required enum
  - **Row Id:** required non-empty string
  - **Transports:** required non-empty enum array
- **Row Id Grammar:** row_id must match ^p081\.[a-z0-9_]+\.(graphql_query\|graphql_subscription\|graphql_mutation\|mcp_initialize\|mcp_tools_list\|mcp_tools_call\|debug_endpoint)\.[a-z0-9_]+$ and be unique within the fixture. The second segment must equal caller_class; the third segment must equal the only transport for required rows. Rows that intentionally apply to more than one transport are forbidden in required_rows and must be split.
- **Schema Artifacts**
  - JSON Schema or equivalent validators for boundary_matrix_fixture_v1, boundary_policy_canaries_v1, boundary_policy_shadow_coverage_report_v1, operator_alert_v1, boundary_runtime_v1, audit_log_readback_v1, the existing schema_version 2 principal-table format, and the new schema_version 3 principal-table format are generated or checked in before phase 4 enforce.
- **Valid Example Row Ids**
  - p081.ui_operator.graphql_query.read
  - p081.ui_operator.graphql_subscription.subscribe
  - p081.ui_operator.graphql_mutation.approval_action
  - p081.agent_operator.mcp_initialize.capability
  - p081.agent_operator.mcp_tools_list.discovery
  - p081.agent_operator.mcp_tools_call.command
  - p081.automation.mcp_tools_list.discovery
  - p081.automation.mcp_tools_call.command
  - p081.observer.mcp_tools_call.compact_read
  - p081.observer.graphql_query.read_only_opt_in
  - p081.developer_break_glass.debug_endpoint.disabled
- **Validator Error Codes**
  - E_SCHEMA_VERSION
  - E_UNKNOWN_FIELD
  - E_MISSING_FIELD
  - E_DUPLICATE_ROW_ID
  - E_UNKNOWN_ENUM
  - E_INVALID_ROW_ID
  - E_INVALID_ACTION_GRAMMAR
  - E_WILDCARD_NOT_ALLOWED
  - E_REQUIRED_ROW_MISSING
  - E_REQUIRED_ROW_TRANSPORT_MISMATCH
  - E_DENY_SIDE_EFFECT_CONFLICT
  - E_NULLABILITY
  - E_FIXTURE_DIGEST_MISMATCH
### Graphql Contract
- **Approval Actionability:** availableActions is the intersection of durable approval state and BoundaryPolicy approval-action decisions. disabledReasonCode uses APPROVAL_NOT_ACTIONABLE, OBSERVER_SCOPE, NON_APPROVAL_MUTATION, or CAPABILITY_OUT_OF_SCOPE.
- **Cases**
  1. **Item**
     - **Case:** missing_or_invalid_token_before_execution
     - **Data:** `null`
     - **Extensions Code:** UNAUTHORIZED
     - **Http Status:** `401`
     - **Reason Code:** UNAUTHENTICATED
  2. **Item**
     - **Case:** authenticated_resolver_deny
     - **Data:** denied field or mutation payload is null
     - **Extensions Code:** FORBIDDEN
     - **Http Status:** `200`
     - **Reason Code:** BoundaryPolicy reason code at denied field path
  3. **Item**
     - **Case:** observer_field_redaction
     - **Data:** redacted nullable field is null
     - **Errors:** no response-level GraphQL error
     - **Extensions:** extensions.redactions includes path, reasonCode, rowId, redactionMode, callerClass, redactionId
     - **Http Status:** `200`
  4. **Item**
     - **Case:** drop_resource
     - **Data:** resource field is null
     - **Extensions Code:** FORBIDDEN
     - **Http Status:** `200`
     - **Reason Code:** BoundaryPolicy reason code
     - **Redaction Mode:** drop_resource
  5. **Item**
     - **Behavior:** connection_error with UNAUTHORIZED/UNAUTHENTICATED then close 4401
     - **Case:** websocket_connection_init_missing_or_invalid_token
  6. **Item**
     - **Behavior:** connection_error with FORBIDDEN/AMBIGUOUS_CALLER then close 4403
     - **Case:** websocket_connection_init_ambiguous_caller
  7. **Item**
     - **Behavior:** operation error for subscribe id with FORBIDDEN and exact reason_code, then complete and no events
     - **Case:** subscription_denied_at_subscribe_start
- **Field Casing:** GraphQL extensions use camelCase: reasonCode, rowId, callerClass, requestId, redactionId, redactionMode. MCP and internal readback use snake_case. Golden fixtures prove both.
### Mcp Contract
- **Compatibility:** Both MCP transports switch atomically at daemon restart because BoundaryPolicy and kill-switch mode are startup-loaded.
- **Initialize:** After phase 4 shadow starts, initialize exposes boundary_policy {matrix_id,schema_version,capability_schema_version:1,mode,denied_known_tool_code:-32004,field_casing:snake_case}.
- **Tools Call:** Unknown tools keep JSON-RPC -32601. Known but denied tools return JSON-RPC -32004 with message tool denied and data.reason_code, caller_class, row_id, request_id, and boundary_policy_version.
- **Tools List:** Denied tools are omitted. tools/list decision rows never append command_journal rows.
### Mcp Idempotency Contract
- **Request Hash Fields:** canonical_request_hash includes tool_name, normalized arguments, caller_class, principal_id, token_id, row_id, and semantic command target. It excludes transport request_id, timestamps, trace ids, and retry metadata.
- **Scope:** Every state-changing MCP command; read-only tools explicitly reject idempotencyKey to avoid implying writes.
- **Semantics**
  - Same key and same canonical request after committed success returns the original result without duplicate command_journal or projection writes.
  - Same key with different canonical request returns IDEMPOTENCY_CONFLICT and no side effects.
  - State-changing tools missing idempotencyKey are denied before command_journal writes after the compatibility phase.
- **Storage:** mcp_command_idempotency stores idempotency_key, tool_name, caller_fingerprint, canonical_request_hash, row_id, command_journal_id, result_hash, committed_at_ms, and expires_at_ms for at least 7 days.
### Observability Operator Contract
- **Alert Delivery Macos**
  - **Authorization Timing:** The app requests UNUserNotificationCenter authorization during operator setup before phase 4 enforce or when enabling local operator alerts, not at the moment a critical alert fires. The app records authorization state in operator alert settings and surfaces degraded native delivery if notifications are denied.
  - **Dock And Status Item:** Critical and error alerts update Dock badge and status item from the same operatorAlerts projection. Clearing the alert removes the badge contribution. If the app normally hides the status item, a temporary status item is shown while a critical alert is active.
  - **Required Tests**
    - operator_alert_fires_and_clears_hidden_window
    - operator_alert_silence_suppresses_native_escalation_until_expiry
    - operator_alert_dock_status_item_clear_on_recovery
  - **Severity To Surface**
    - **Critical**
      - operatorAlerts inbox
      - non-dismissible persistent in-app banner until condition clears or is silenced with expiry
      - Dock badge count
      - MenuBarExtra or NSStatusItem critical state even when main window is closed
      - NSApp.requestUserAttention(.criticalRequest) when inactive
      - UNUserNotification with critical interruption only when entitlement and user authorization exist; otherwise timeSensitive notification plus requestUserAttention
    - **Error**
      - operatorAlerts inbox
      - persistent in-app banner
      - Dock badge count
      - MenuBarExtra or NSStatusItem warning state
      - NSApp.requestUserAttention(.informationalRequest) when app is inactive
      - UNUserNotification with timeSensitive interruption when authorized
    - **Info**
      - operatorAlerts inbox
      - nonmodal in-app banner while app is foreground
    - **Warn**
      - operatorAlerts inbox
      - persistent in-app banner
      - toolbar or window chrome badge on every open operator window
  - **Silence Semantics:** Silencing uses alert dedupe_key and expiry. Silence suppresses new UN notifications and requestUserAttention for that dedupe_key until silence_until_ms, but the inbox entry, safe-mode banner, and diagnostics remain visible. Critical safe-mode alerts cannot be permanently dismissed while the condition remains active.
- **Operator Alert Contract**
  - **Alert Rules**
    1. **Item**
       - **Alert Id:** boundary.matrix_no_row.enforce
       - **Clear:** 0 events for 10 minutes plus fixture validation pass
       - **Severity:** critical
       - **Threshold:** >=1 MATRIX_NO_ROW in enforce over 1 minute
    2. **Item**
       - **Alert Id:** boundary.denial_spike
       - **Clear:** below threshold for 15 minutes
       - **Severity:** warn
       - **Threshold:** >25 CAPABILITY_OUT_OF_SCOPE or OBSERVER_SCOPE for same row_id\|caller_class over 5 minutes
    3. **Item**
       - **Alert Id:** boundary.safe_mode.active
       - **Clear:** safe_mode_active=false for 2 probes
       - **Severity:** critical
       - **Threshold:** safe_mode_active for >60 seconds
    4. **Item**
       - **Alert Id:** audit_log.unwritable
       - **Clear:** 3 successful half-open writes and budget <80%
       - **Severity:** critical
       - **Threshold:** consecutive audit write failures >=3 or audit budget >=95%
    5. **Item**
       - **Alert Id:** audit_log.integrity_degraded
       - **Clear:** checkpoint verification returns verified after repair
       - **Severity:** error
       - **Threshold:** integrity_state=degraded or tamper_suspected at startup
    6. **Item**
       - **Alert Id:** shadow.disagreement
       - **Clear:** triaged with owner and 0 new disagreements for 24h
       - **Severity:** error
       - **Threshold:** >=1 shadow disagreement
  - **Canonical Surface:** GraphQL operatorAlerts projection plus MCP diagnostics operator.alerts.list; Swift renders through app state as in-app banner/inbox and macOS-native escalation. Daemon stderr is secondary structured log only.
  - **Payload Schema**
    - **Alert Id:** string
    - **Caller Class:** string\|null
    - **Cleared At Ms:** integer\|null
    - **Dedupe Key:** string
    - **Evidence Ref:** string\|null
    - **Fixture Version:** string\|null
    - **Last Seen Ms:** integer
    - **Lifecycle:** open\|active\|silenced\|cleared
    - **Mode:** shadow\|enforce\|read_only_safe_mode\|legacy_compat
    - **Opened At Ms:** integer
    - **Reason Code:** string
    - **Row Id:** string\|null
    - **Runbook Id:** string
    - **Schema Version:** `1`
    - **Severity:** info\|warn\|error\|critical
    - **Silence Until Ms:** integer\|null
    - **Summary:** string
- **Runtime Readback**
  - **Audit Log Health:** audit_log_health_v1 {writable,last_write_ok_at_ms,consecutive_failures,cumulative_failures,budget_bytes,used_bytes,integrity_state,last_checkpoint_hash}
  - **Graphql:** Query.boundaryRuntime returns boundary_runtime_v1 {mode, fixtureVersion, fixtureDigest, embeddedFixtureDigest, boundaryPolicyGenerationId, safeModeActive, safeModeEnteredAtMs, safeModeReason, lastReloadAttemptAtMs, auditLogHealth, alertSummary, shadowCoverageReportRef}.
  - **Mcp:** initialize.boundary_policy and tools/call boundary.runtime.get return the same fields in snake_case.
- **Shadow Coverage Report**
  - **Gate Predicate:** Every shippable cell has observation_count>=10 and shadow_disagreement_count==0, or canary_covered=true with required_test_id. Report max staleness is 24h for cutover PR.
  - **Path:** docs/evidence/boundary-policy-shadow-coverage/report.json
  - **Schema:** boundary_policy_shadow_coverage_report_v1
### Reliability Runtime Contract
- **Audit Budget Recovery:** At 80 percent audit budget emit warning; at 95 percent enter read_only_safe_mode for audit-required state-changing calls, run bounded cleanup outside request transactions, emit cleanup progress every 30 seconds, exit only when budget is below 80 percent and three half-open audit writes succeed.
- **Backfill Progress:** command_journal caller_class backfill uses monotonic batch_id, at most 500 rows per batch, target lock window under 100 ms, and emits progress. No progress for 10 minutes opens boundary.backfill.stalled.
- **Policy Reload:** Policy mode changes close WebSocket subscriptions with 4408 POLICY_RELOAD only during daemon restart or explicit reload. Clients reconnect and re-handshake against boundaryRuntime.
- **Sqlite Contention:** Use busy_timeout 250 ms plus bounded retry/backoff inside transaction_deadline_ms 1500. Exhaustion returns SQLITE_CONTENTION_RETRY_EXHAUSTED with no success acknowledgement.
- **Subscription Cursor:** Subscriptions include sequence_cursor and projection_generation. Server retains cursor replay for at least 15 minutes or 10000 events per stream, whichever is larger. Reconnect outside the window returns gap_detected requiring full refetch.
- **Tamper Suspected Startup:** If audit checkpoint verification reports tamper_suspected, startup enters read_only_safe_mode, opens audit_log.integrity_degraded alert with runbook_id boundary.audit.integrity, and denies audit-required state-changing calls until operator repair or explicit emergency override.
### Security Hardening Contract
- **Bearer Token Handling**
  - Authorization header grammar is exactly Bearer <token> with one SP, token length 32..4096 bytes, visible ASCII except CTL, no comma-list or folded headers.
  - Compare fixed-length SHA-256 digests in constant time; never use string equality, prefix matching, trim, lowercasing, or timing-visible early return.
  - created_at and expires_at are RFC3339 UTC. Expired tokens fail as UNAUTHENTICATED before BoundaryPolicy with maximum 60 seconds skew.
  - For v1 compatibility only, derive token_id = base32(sha256("p081-v1-token-id" \|\| principal_id \|\| bearer_token))[0..26]; derived token_id is diagnostic only.
- **Developer Break Glass:** Disabled unless CHAINWORKS_BREAK_GLASS_1=enabled and principal has developer_break_glass caller_class. Disabled response exposes DEBUG_SURFACE_UNAVAILABLE with no row_id, caller_class, or route inventory. Internally exactly one developer_break_glass_disabled audit row is required; if unavailable return E_AUDIT_UNAVAILABLE and expose no debug data.
- **Principals Json**
  - Configured path only; no cwd-relative fallback in enforce mode.
  - File must be 0600 regular file; parent directory 0700 where platform permits it.
  - Reject symlinks, hard-link count greater than 1, non-regular files, parent traversal, and canonical path outside configured config root.
  - Strict UTF-8 JSON, maximum 256 KiB, unknown fields rejected at every nested level, duplicate principal_id or token_id rejected, duplicate bearer_token rejected after constant-time digest setup.
### Startup Safety
- **Fixture Loading**
  - Build embeds a last-known-good boundary fixture generated from docs/reference/boundary-first-api-auth-contract.json and validated by CI.
  - Startup validates deployed fixture first. If valid, it is used.
  - If deployed fixture is invalid in shadow or enforce mode, daemon loads embedded fixture and enters read_only_safe_mode unless CHAINWORKS_BOUNDARY_POLICY=legacy.
  - If both deployed and embedded fixtures are invalid, listeners do not bind and startup emits structured fixture diagnostics.
- **Read Only Safe Mode**
  - Serve GraphQL reads and subscriptions that can be evaluated by the embedded fixture.
  - Deny all GraphQL mutations, MCP tool calls, and approval actionability mutations.
  - Expose operator-visible local alert with invalid fixture digest and embedded fixture digest.
  - Exit only after deployed fixture validates, audit_log_health is writable, three half-open recovery probes pass, and operator runtime readback reports safeModeActive=false for two probes. Target MTTR for expected fixture rollback is under 5 minutes.
### Swift Macos Boundary Contract
- **Accessibility Contract**
  - **Actionability False:** Approve and Reject controls remain discoverable in Full Keyboard Access when visible, expose disabled state, accessibilityValue from disabledReasonCode, and never advertise an actionable default button trait.
  - **Contrast And Motion:** Increase Contrast uses non-color indicators such as lock icon, text label, and border treatment. Reduce Motion disables alert pulse or shake animations and uses static badges. No state is communicated by color alone.
  - **Drop Resource:** Restricted View exposes accessibilityLabel 'Restricted view', accessibilityValue from denial_copy, accessibilityHint with the human troubleshooting text, and remains reachable by VoiceOver and Full Keyboard Access without exposing stale child controls.
  - **Ordinary Nil:** Ordinary nil exposes the field display name and accessibilityValue 'No value' without restricted hints or locked traits.
  - **Reason Codes:** Known reason codes map through denial_copy. Unknown future codes render 'Action Not Available' while copied diagnostics preserve raw reason_code.
  - **Redacted Nil:** The rendered control or value exposes accessibilityLabel equal to the field display name, accessibilityValue 'Restricted value', accessibilityHint 'Permissions hide this value. Copy diagnostics for the access rule.', and a locked or disabled trait appropriate to the control without pretending the value is empty.
  - **Required Tests**
    - accessibility_redaction_parity
    - keyboard_full_access_actionability_false
    - increase_contrast_redaction_state
    - reduce_motion_alert_state
- **Approval Action Attempt Store:** ApprovalActionAttemptStore owns one UUIDv7 idempotencyKey per approval_id/action attempt, reuses it across retries and duplicate-tap suppression, persists pending attempts across app restart or network loss, and clears only on terminal outcomes or reload-confirmed conflict states. It is an injected app-state or service dependency, not button-local state.
- **Macos Commands**
  - **Default Shortcuts**
    - Approve Approval: Command-Return when focused approval is actionable
    - Reject Approval: Command-Shift-Return when focused approval is actionable
    - Copy Boundary Diagnostics: Command-Option-C
  - **Menu Titles**
    - Approve Approval
    - Reject Approval
    - Copy Boundary Diagnostics
  - **Required Tests**
    - keyboard_driven_approval_uses_actionability_projection
    - copy_boundary_diagnostics_excludes_secrets
  - **Validation Source:** Menus and command palette validate against ActionabilityProjection.availableActions and the front-most key window selection. If multiple windows are open, the front-most focused approval row owns the target; otherwise commands are disabled.
- **Redaction Envelope:** Typed GraphQL envelope decoding preserves extensions.redactions before SwiftUI rendering. Redacted nullable nil becomes RedactionState.redacted with redactionId, reasonCode, rowId, path, and callerClass. Ordinary nil remains ordinary nil. drop_resource invalidates stale selected-detail content and renders Restricted View or Permission Denied.
- **Window State:** boundaryRuntime.safeModeActive drives a window-level toolbar badge or persistent banner in every Scene. State restoration after 4408 POLICY_RELOAD preserves selected run id, selected approval id, scroll anchor where available, and open diagnostics tab through SceneStorage or NSUserActivity.
### Current Baseline Alignment
- **Baseline Truth**
  - docs/reference/current-system-baseline.md is the baseline source of truth for HEAD, not a future-state proposal.
  - The governed macOS UI remains GraphQL read/query and subscription only, with approveApproval and rejectApproval as the only allowed UI mutations.
  - Non-approval operator control remains MCP-owned for operators, agents, and automations.
- **Delta From Head**
  - HEAD does not yet claim a single matrix-backed BoundaryPolicy implementation shared across GraphQL, MCP, and approval actionability.
  - HEAD already has auth reality from P072, including an existing schema_version 2 principal-table meaning in control-plane/crates/auth/src/lib.rs.
  - P081 is additive to the current baseline and defines target post-merge contracts without reopening unrelated baseline subsystems or future P075 landings.
- **Proposal Posture:** This proposal is written as post-merge truth for the new boundary/auth contract while remaining anchored to the current baseline for everything outside the P081 delta.
### Principal Table Versions
- **Baseline Contract:** At the current baseline, control-plane/crates/auth/src/lib.rs already assigns schema_version 2 to the P072 principal-table shape. P081 must preserve that meaning and must not reuse schema_version 2 for a different boundary-aware contract.
- **Version Rules**
  - schema_version 1 remains the legacy compatibility shape through phase 5 with documented defaulting.
  - schema_version 2 remains the existing P072-compatible format through phase 5 and keeps its current meaning for surface policies and token records.
  - schema_version 3 is the first boundary-aware format that can encode caller and transport policy required by P081, and all boundary-aware writers emit schema_version 3 only.
  - Unknown versions fail closed before mutating surfaces are served.
  - Phase 6 may retire v1 compatibility, but schema_version 2 remains a supported historical format until an explicit later migration proposal says otherwise.
- **V2 Compatibility:** Existing schema_version 2 files remain readable without reinterpretation. P081 readers preserve the current auth behavior, redaction expectations, and defaults for those files rather than silently remapping them to boundary-matrix semantics, and readback compatibility keeps callerPrincipalClass alongside nullable callerClass until downstream consumers are explicitly migrated.
- **V3 Scope:** schema_version 3 adds the boundary-aware transport and caller-policy shape needed for CallerClass derivation and shared BoundaryPolicy decisions while keeping PrincipalClass as persisted identity truth.
- **Request Handling Rule:** Request paths operate on the validated in-memory principal-table generation loaded at startup; they do not reopen principals.json or reference docs during request handling.
- **Shape Examples**
  - **Schema Version 2**
    - **Meaning:** Existing P072-compatible surface-policy format retained as-is.
    - **Schema Version:** `2`
  - **Schema Version 3**
    - **Meaning:** New boundary-aware caller and transport policy format introduced by P081.
    - **Schema Version:** `3`
- **Writer Rule:** Boundary-aware writers emit schema_version 3 only. Existing schema_version 2 files are never silently reinterpreted, rewritten in place, or upgraded on read; any conversion from v2 to v3 happens only through an explicit migration or operator-directed upgrade step that preserves rollbackability.

## Rollout Contract V1

- **Schema Version:** rollout_contract_v1
- **Applicability:** required
### Gate Aliases
- proposal-081
- p081
### Commands
- **Allowlist**
  - ./scripts/test-gate.sh proposal-081
  - ./scripts/test-gate.sh p081
- **Commentary:** P081 gate is declarative contract validation plus focused auth, GraphQL, MCP, approvals, accessibility, and readback proof. It must not require live daemon startup, production credentials, UI smoke hosts, simulator runs, or destructive operator actions.
### Migrations
- **Description:** Additive P081 migration set covering boundary-aware principal-table schema_version 3 support, nullable command_journal.caller_class, audit_log and audit_log_checkpoints durability, approval idempotency state, and bounded operator readback surfaces.
- **Not Applicable:** `false`
### Metrics
- **Adoption Metric:** p081_boundary_policy_enforcement_parity_percent
- **Operational Metrics**
  - boundary_policy_decisions_total{transport,row_id,caller_class,action_kind,decision,denial_reason_code,shadow_or_enforce}
  - boundary_policy_shadow_disagreement_total{transport,row_id,caller_class,action_kind,legacy_decision,matrix_decision,denial_reason_code}
  - auth_ambiguous_caller_warn_total{principal_class,surface_policy_hash,transport}
  - boundary_no_op_label_total{repo,month}
  - boundary_policy_evaluation_error_total{transport,mode}
  - audit_log_append_failure_total{event_type,transport,mode}
  - audit_log_rate_limited_total{transport,reason_code}
  - operator_alert_native_delivery_total{severity,surface,result}
  - approval_idempotency_duplicate_total{action,caller_class}
  - boundary_policy_decision_latency_ms{transport,caller_class,mode}
  - boundary_commit_transaction_latency_ms{transport,action_kind,decision}
  - audit_budget_cleanup_duration_ms
  - operator_alert_clear_latency_ms{alert_id,severity}
### Readback Lanes
- run_report
- mcp
- release_receipt
- graphql
### Readback Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
- **Readback Fixture:** docs/evidence/rollout-contract/operator-readback/p081-full-surface.fixture.json
### Operator Report Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
### Hold Conditions
- Any required boundary matrix row lacks executable fixture coverage or row_id grammar validation.
- Any GraphQL non-approval mutation remains reachable for ui_operator or any approval mutation bypasses ActionabilityProjection::for_caller.
- Any MCP caller can discover or invoke a command outside its allowed BoundaryPolicy capability set.
- Any denied call writes command_journal, approval settlement, audit rows that violate fail-closed rules, or projection deltas inconsistent with the durable decision.
- Shadow disagreement, ambiguous caller warnings, or audit integrity/readback checks remain non-zero at the planned enforce cutover.
- macOS fires-and-clears alert delivery, accessibility_redaction_parity, or keyboard-driven approval evidence is missing at the phase that enables enforce mode.
### Rollback Disposition
- **Mode:** set_CHAINWORKS_BOUNDARY_POLICY_to_shadow_or_legacy_preserve_readback
- **Data Loss Risk:** none
- **Steps**
  - Set CHAINWORKS_BOUNDARY_POLICY=shadow to keep matrix telemetry while restoring legacy-authoritative allow and deny behavior.
  - If shadow is insufficient for operator recovery, set CHAINWORKS_BOUNDARY_POLICY=legacy as the audited emergency bypass.
  - Restart daemon through the standard operator control path and allow subscriptions to reconnect or receive 4408 POLICY_RELOAD.
  - Keep audit readback, operator alerts, diagnostics copy, and rollout decision surfaces visible during rollback.
  - Re-enable enforce only after proposal-081 gate, readback fixture parity, shadow disagreement counters, and accessibility or alert evidence return green.
### Decision Vocabulary
- pass
- fail
- waived
- not_applicable
- timeout
- cancelled
- missing_contract
- tamper_detected
- stale
- release
- hold
- waive
### Negative Fixtures
- **P081 Missing Boundary Readback:** docs/evidence/rollout-contract/negative/p081-missing-boundary-readback.json
- **P081 Unsafe Path And Command:** docs/evidence/rollout-contract/negative/p081-unsafe-path-and-command.json

## Rollout

- **Kill Switches:** CHAINWORKS_BOUNDARY_POLICY and CHAINWORKS_AUTH_AMBIGUOUS_MODE are startup-only and require daemon restart. The validated boundary matrix and principal-table generation are immutable for the process lifetime, so policy or fixture changes take effect only after restart. Target local restart is under 30 seconds excluding operator approval time.
### Phases
1. **Item**
   - **Exit Evidence**
     - deployed and embedded fixtures validate
     - all required row ids exist and match executable grammar
     - valid fixture examples cover every required row id
     - README links artifacts
     - malformed deployed fixture enters read_only_safe_mode
     - audit_log migration and rollback rehearsal pass
     - next numbered db migration files for audit_log and audit_log_checkpoints are defined against the current migration tail
     - bounded audit repo contract and fail-closed deny-write path are specified against existing db patterns
   - **Lands**
     - matrix docs and JSON
     - fixture validator
     - embedded last-known-good fixture
     - next additive numbered SQL migrations for audit_log and audit_log_checkpoints
     - db::repos::audit_log contract
     - matrix_row test marker
   - **Phase:** `1`
   - **Sub Gate:** p081-matrix
2. **Item**
   - **Exit Evidence**
     - all production and test principal fixtures derive one CallerClass or explicit deny reason
     - v1 compatibility tests pass
     - existing P072-compatible schema_version 2 fixtures pass unchanged
     - schema_version 3 fixtures validate and fail closed safely on malformed boundary-aware policy
     - principals.json hardening tests pass
     - schema_version 2 fixtures are not silently rewritten during v3 adoption rehearsal
   - **Lands**
     - CallerClass enum
     - CallerContext.caller_class
     - principal-table reader for v1, existing schema_version 2, and new schema_version 3
     - schema_version 3 bootstrap writer
     - nullable command_journal.caller_class
   - **Phase:** `2`
   - **Sub Gate:** p081-identity
3. **Item**
   - **Exit Evidence**
     - zero ambiguous warnings over 7 days with at least 250 auth resolutions per production principal_class
     - or 7 days of canary traffic with zero warnings for low-volume classes
     - or explicit audited operator override
   - **Lands**
     - auth::resolve derives caller_class
     - ambiguous callers warn while legacy guards remain authoritative
   - **Phase:** `3`
   - **Sub Gate:** p081-resolve
4. **Item**
   - **Exit Evidence**
     - each shippable matrix cell observed at least 10 times in shadow with zero disagreement or canary coverage
     - non-zero disagreements triaged within 2 business days
     - cutover PR includes counter report and rollback rehearsal
     - operator_alert_contract and alert_delivery_macos fires-and-clears tests pass with main window hidden or inactive
     - audit budget recovery, SIGTERM commit boundary, subscription gap, and tamper_suspected startup tests pass
     - allowed mutation audit rows commit in the same transaction as command_journal, approval settlement, idempotency, and other durable command-owned writes, while any projection or readback refresh follow-up may occur after commit
     - deny-only durable audit paths prove exactly-one-row commit or fail-closed behavior
     - bounded audit health and integrity readback matches existing daemonStatus or /health style operator diagnostics
   - **Lands**
     - GraphQL, MCP, observer reconciliation, startup safe-mode, boundaryRuntime, operatorAlerts, and native macOS alert delivery call one shared injected BoundaryPolicy service in shadow then enforce
     - MCP -32004 compatibility signal
     - deterministic GraphQL error contract
   - **Phase:** `4`
   - **Sub Gate:** p081-surfaces
5. **Item**
   - **Exit Evidence**
     - approval/principal fixture parity passes
     - idempotent retry returns original result without duplicate settlement
     - terminal approval with new idempotencyKey returns APPROVAL_NOT_ACTIONABLE
     - denial-side-effect sweep passes
     - accessibility_redaction_parity and keyboard-driven approval tests pass
   - **Lands**
     - ActionabilityProjection::for_caller
     - approveApproval/rejectApproval idempotencyKey
     - approval_mutation_idempotency table
     - ApprovalActionAttemptStore persistence
     - typed redaction envelope and accessibility contract
   - **Phase:** `5`
   - **Sub Gate:** p081-approval
6. **Item**
   - **Exit Evidence**
     - compatibility fixture inventory empty or explicitly waived
     - backfill report captured
     - CI citation report attached
     - GraphQL, MCP, report, alert, runtime, and audit readback fixtures prove compatibility
   - **Lands**
     - retire compatibility fixtures
     - enable scripts/check-boundary-coverage.sh in test-gate guardrails
     - callerClass plus compatibility callerPrincipalClass readback docs
     - resumable command_journal backfill
   - **Phase:** `6`
   - **Sub Gate:** p081-fixtures
### Restart Playbook
- Set CHAINWORKS_BOUNDARY_POLICY=shadow to roll back enforce decisions while preserving telemetry, or legacy for emergency bypass before phase 6.
- Restart daemon through the standard operator control path.
- In-flight calls may fail with retriable transport errors during shutdown; no partial success is acknowledged before successful SQLite COMMIT.
- Post-restart retry with same idempotency key returns original result when COMMIT succeeded before ACK.
- Subscriptions receive clean disconnect or 4408 POLICY_RELOAD and re-handshake against the new policy mode.
- **Rollback Rehearsal:** Phase 4 staging rehearsal sets CHAINWORKS_BOUNDARY_POLICY=shadow, restarts daemon, verifies GraphQL/MCP/actionability return to legacy-authoritative behavior, confirms subscriptions reconnect, confirms native alerts clear, and confirms matrix decisions still log.
- **Umbrella Gate:** proposal-081\|p081

## Metrics And Alerts

### Counters
- boundary_policy_decisions_total{transport,row_id,caller_class,action_kind,decision,denial_reason_code,shadow_or_enforce}
- boundary_policy_shadow_disagreement_total{transport,row_id,caller_class,action_kind,legacy_decision,matrix_decision,denial_reason_code}
- auth_ambiguous_caller_warn_total{principal_class,surface_policy_hash,transport}
- boundary_no_op_label_total{repo,month}
- boundary_policy_evaluation_error_total{transport,mode}
- audit_log_append_failure_total{event_type,transport,mode}
- audit_log_rate_limited_total{transport,reason_code}
- operator_alert_native_delivery_total{severity,surface,result}
- approval_idempotency_duplicate_total{action,caller_class}
### Histograms
- boundary_policy_decision_latency_ms{transport,caller_class,mode}
- boundary_commit_transaction_latency_ms{transport,action_kind,decision}
- audit_budget_cleanup_duration_ms
- operator_alert_clear_latency_ms{alert_id,severity}
### Lagging Metrics
- zero new boundary findings on auth, GraphQL, MCP, approvals, command_handler, and macOS alert/redaction PRs over 90 days after phase 6
- median time to classify a new caller no more than two PRs and no more than one week
- monthly boundary-no-op label review attached to guardrail health note
### Leading Metrics
- 100 percent fixture principal classification or explicit deny reason
- 100 percent required matrix rows validate against row_id grammar and transport enum
- 95 percent matrix-row citation rate by end of phase 4
- 100 percent citation coverage for boundary-specific tests by phase 6
- zero shadow disagreements before enforce
- at least 10 shadow observations per shippable matrix cell or canary coverage before enforce
- zero ambiguous-caller warnings over phase 3 evidence window
- 100 percent macOS critical alert fires-and-clears tests passing before phase 4 enforce
- 100 percent accessibility_redaction_parity fixture coverage before phase 5 approval UI enforcement

## Acceptance Criteria

- Matrix doc and JSON fixture exist and are linked from docs/reference/README.md.
- JSON fixture validator rejects missing required fields, duplicate row ids, unknown enum values, unknown fields at all nested levels, invalid schema_version, wildcard misuse, required-row transport mismatch, and missing required rows.
- Every boundary_matrix.required_rows row validates against executable_boundary_contract row_id grammar and transport enum, and valid examples cover every required row id.
- Build embeds a validated last-known-good matrix fixture; malformed deployed fixture enters read_only_safe_mode rather than unrecoverable daemon-down.
- audit_log and audit_log_checkpoints land through the next additive numbered SQL migration file or files under control-plane/crates/db/migrations/, with indexes, repository append semantics, payload_sha256, diagnostic_truncated, prev_hash, row_hash, checkpoint verification, bounded readback versioning, retention, and fail-closed unavailable-storage behavior implemented.
- Principal-table loading accepts existing v1 and existing P072-compatible schema_version 2 fixtures through phase 5, introduces schema_version 3 for boundary-aware caller and transport policy, applies exact defaults, rejects unknown versions, and redacts bearer tokens from logs and diagnostics.
- CallerClass enum and CallerContext.caller_class exist; every production and test principal resolves to one CallerClass or one explicit deny reason.
- GraphQL query, subscription, mutation, MCP initialize, MCP tools/list, MCP tools/call, and approval actionability all call the same daemon-injected BoundaryPolicy instance.
- GraphQL errors use the deterministic HTTP/WebSocket extensions contract with explicit camelCase fields.
- MCP known-but-denied tools use -32004 while unknown tools remain -32601, with initialize capability signal for P081 servers.
- State-changing allowed calls use the P081 durable idempotency preclaim contract: a pending sentinel is written before dispatch, the command write unit stamps the same idempotency key and BoundaryPolicy row into command_journal/domain writes, and post-dispatch commit links the sentinel to the journal/result. If the process crashes after the command commits but before the result is acknowledged, committed-unack recovery resolves the pending sentinel from command_journal and refuses silent re-execution.
- approveApproval and rejectApproval require idempotencyKey, check terminal state under settlement transaction, and do not double-settle on retry.
- State-changing MCP commands have a required idempotency contract or are explicitly classified read-only and reject idempotencyKey.
- Denial-side-effect tests prove denied calls create zero command_journal rows, zero approval settlements, and zero projection writes except matrix-declared audit rows.
- scripts/check-boundary-coverage.sh is wired into test-gate guardrails and fails in-scope changes without matrix/fixture touch, matrix_row citation, or boundary-no-op label.
- Security hardening tests prove principals.json mode/canonical-path checks, strict JSON parsing, constant-time bearer-token comparison, expiry enforcement, token_id derivation redaction, error-envelope non-disclosure, disabled break-glass non-disclosure, audit-log DoS controls, and audit/fixture tamper evidence.
- boundary-policy-canaries.yaml has a validator and contributes canary rows to the same shadow coverage report schema as live observations.
- SQLite contention, audit outage, subscription cursor/gap detection, safe-mode readback/exit, SIGTERM drain, committed-unack idempotency recovery, and denial-audit backpressure behavior are covered by reliability tests.
- Operator alert contract has GraphQL/MCP readback, payload schema, severity/dedupe/silence/clear lifecycle, numeric thresholds/windows, macOS-native alert delivery, and alert-fires-and-clears tests with the main window hidden or inactive.
- boundaryRuntime and audit_log_health readback expose policy mode, safe mode, fixture digests, audit writability, integrity state, retention/cleanup state, and shadow coverage report refs in bounded GraphQL and MCP diagnostic lanes without introducing a broad audit table browser.
- Swift approval mutations use ApprovalActionAttemptStore-owned idempotencyKey generation/reuse, and typed GraphQL decoding preserves extensions.redactions so redacted nil differs from ordinary nil.
- Accessibility parity tests cover redacted nil, ordinary nil, drop_resource Restricted View, actionability_false controls, Full Keyboard Access, Increase Contrast, and Reduce Motion.
- BoundaryPolicy trait ownership, dependency direction, and daemon injection are explicit: one immutable validated policy service instance is constructed at startup and shared across GraphQL, MCP, and approval actionability paths.
- Current-system baseline framing is explicit: P081 changes authorization and audit contracts without changing the existing GraphQL read/subscription plus approval-only UI boundary or moving non-approval control off MCP.
- Request paths never read the matrix fixture, current-system baseline doc, or other reference files directly; only startup validation and restart rebuild the shared in-memory policy inputs.
- db::repos::audit_log defines transactional append_tx for allowed mutating paths and a bounded standalone append path for deny-only durable audit writes, following existing repo patterns such as command_journal.
- Deny-only paths that require durable audit commit exactly one primary audit_log row in a bounded SQLite write transaction before returning denial once that seam has the necessary bounded DB access; if that commit fails, the request fails closed.
- Boundary-aware principal-table writers emit schema_version 3 only, and existing schema_version 2 files are never silently reinterpreted or rewritten in place outside an explicit migration or upgrade step.
- Compatibility readback for audit-derived diagnostics preserves callerPrincipalClass and nullable callerClass on the first bounded GraphQL/MCP operator diagnostic surfaces that expose audit evidence.

## Risks

1. **Item**
   - **Mitigation:** Every required row is single-transport, row_id grammar is enforced, and valid examples cover every required row id.
   - **Risk:** Required matrix rows drift away from the executable schema.
2. **Item**
   - **Mitigation:** Rate limits, payload caps, audit budget thresholds, coalesced rate-limit rows, checkpoint-window cleanup outside request transactions, and read_only_safe_mode at critical budget.
   - **Risk:** Audit-log denial flood fills local disk.
3. **Item**
   - **Mitigation:** payload_sha256, diagnostic_truncated, row_hash, prev_hash, checkpoint table, startup verification, and integrity alerts are part of the migration contract.
   - **Risk:** Audit tampering or truncation weakens evidence.
4. **Item**
   - **Mitigation:** macOS-native delivery maps severity to Dock badge, status item, requestUserAttention, and notifications, with hidden-window fires-and-clears tests.
   - **Risk:** Critical operator alerts are missed when the app is hidden or inactive.
5. **Item**
   - **Mitigation:** Typed redaction envelope, RedactionState, accessibility labels/values/hints, Full Keyboard Access tests, and contrast/motion adaptations are required.
   - **Risk:** Swift treats redacted nil as ordinary nil or inaccessible content.
6. **Item**
   - **Mitigation:** Unknown tools keep -32601; known-but-denied moves to -32004 only after shadow and with initialize capability signal.
   - **Risk:** MCP clients depend on old error behavior.
7. **Item**
   - **Mitigation:** Approval and MCP idempotency tables, canonical request_hash, committed-unack retry semantics, and transaction boundary tests.
   - **Risk:** Approval or MCP retries duplicate state-changing side effects.
8. **Item**
   - **Mitigation:** Embedded last-known-good fixture plus read_only_safe_mode keeps reads alive and denies mutations.
   - **Risk:** Malformed matrix fixture causes daemon outage.
9. **Item**
   - **Mitigation:** developer_break_glass exposes no data or decision unless the required audit row commits.
   - **Risk:** Break-glass attempts lose audit evidence when storage is unavailable.
10. **Item**
   - **Mitigation:** Monthly label tally is required and tracked.
   - **Risk:** boundary-no-op label erodes CI guardrail.
11. **Item**
   - **Risk:** Implementers accidentally reinterpret existing schema_version 2 files as the new boundary-aware format and break deployed auth fixtures.
   - **Mitigation:** The proposal now makes schema_version 2 preservation explicit, introduces schema_version 3 for new semantics, and requires compatibility evidence in rollout and acceptance criteria.
12. **Item**
   - **Risk:** BoundaryPolicy ownership drifts into GraphQL, MCP, or actionability-specific code paths and recreates the split-brain boundary this proposal is trying to remove.
   - **Mitigation:** The shared auth-boundary Rust home, daemon injection model, and single immutable instance requirement are explicit and reviewable.
13. **Item**
   - **Risk:** audit_log lands as an abstract contract with no migration or repo integration point, leaving implementation to invent write ordering and failure behavior.
   - **Mitigation:** The proposal now ties audit tables to the next numbered SQL migration files under control-plane/crates/db/migrations/ and to a concrete db::repos::audit_log contract with transactional and bounded standalone append paths.
14. **Item**
   - **Risk:** deny-only durable audit writes accidentally bypass the existing SQLite write-unit discipline or return denial before the audit row is durably committed.
   - **Mitigation:** The proposal now requires a bounded BEGIN IMMEDIATE audit-only transaction for those paths and fail-closed behavior if the commit cannot complete.

## Reviewer Feedback Resolution

1. **API-BLOCK-006**
   - **Decision:** accepted
   - **Id:** API-BLOCK-006
   - **Resolution:** Split combined required rows into single-transport rows, renamed observer and developer_break_glass row ids to match grammar, added required-row transport mismatch validation, and required valid examples for every required row id.
2. **API-BLOCK-007**
   - **Decision:** accepted
   - **Id:** API-BLOCK-007
   - **Resolution:** Expanded audit_log_contract with payload, payload_sha256, diagnostic_truncated, prev_hash, row_hash, checkpoint_id, audit_log_checkpoints table, hash inputs, indexes, readback fields, and migration semantics.
3. **MACOS-BLOCK-001**
   - **Decision:** accepted
   - **Id:** MACOS-BLOCK-001
   - **Resolution:** Added alert_delivery_macos contract covering severity-to-native-surface mapping, UN authorization timing, Dock badge, MenuBarExtra or NSStatusItem behavior, NSApp.requestUserAttention behavior, silence semantics, and hidden-window fires-and-clears tests.
4. **MACOS-BLOCK-002**
   - **Decision:** accepted
   - **Id:** MACOS-BLOCK-002
   - **Resolution:** Added accessibility_contract covering labels, values, hints, traits, VoiceOver and Full Keyboard Access behavior, Increase Contrast, Reduce Motion, reason-code copy mapping, and accessibility_redaction_parity tests.
5. **API-NB-003**
   - **Decision:** accepted
   - **Id:** API-NB-003
   - **Resolution:** Aligned denial_reason_code registry with executable fixture enum and included E_AUDIT_UNAVAILABLE, E_FIXTURE_DIGEST_MISMATCH, SQLITE_CONTENTION_RETRY_EXHAUSTED, and IDEMPOTENCY_CONFLICT.
6. **API-NB-004**
   - **Decision:** accepted
   - **Id:** API-NB-004
   - **Resolution:** Versioned MCP initialize boundary_policy capability with capability_schema_version:1 and pinned snake_case casing.
7. **REL-V5-NB-3**
   - **Decision:** accepted_as_phase_4_gate
   - **Id:** REL-V5-NB-3
   - **Resolution:** Added audit budget cleanup trigger, progress metrics, safe-mode exit predicate, and recovery clear condition.
8. **REL-V5-NB-4**
   - **Decision:** accepted_as_phase_4_gate
   - **Id:** REL-V5-NB-4
   - **Resolution:** Defined successful COMMIT as the acknowledgment boundary and added committed-unack retry behavior.
9. **MACOS-NB-001**
   - **Decision:** accepted
   - **Id:** MACOS-NB-001
   - **Resolution:** Added menu titles, shortcuts, validation source, front-most-window ownership, and keyboard-driven approval test.
10. **APPLE-NB-002**
   - **Decision:** accepted
   - **Id:** APPLE-NB-002
   - **Resolution:** Required golden fixtures for nested redaction paths, ordinary nil versus redacted nil, resolver FORBIDDEN path handling, WebSocket subscribe denial, and drop_resource selected-detail invalidation.

## Open Questions

1. **OQ1**
   - **Default:** Keep automation as constrained Agent for P081; revisit only if another automation-shaped caller appears.
   - **Id:** OQ1
   - **Question:** Does automation eventually deserve its own persisted PrincipalClass rather than Agent plus surface_policies.mcp=automation?
2. **OQ2**
   - **Default:** If none is documented, phase 1 documents minimum 90-day local retention.
   - **Id:** OQ2
   - **Question:** What is the existing audit retention policy for audit_log?
3. **OQ3**
   - **Default:** No deprecation in P081; retain compatibility until a later consumer-migration proposal.
   - **Id:** OQ3
   - **Question:** Should callerPrincipalClass be deprecated after callerClass adoption?
4. **OQ4**
   - **Default:** Do not require entitlement for P081 readiness; use timeSensitive notification plus NSApp.requestUserAttention(.criticalRequest) when critical notification entitlement is unavailable.
   - **Id:** OQ4
   - **Question:** Does the app have entitlement and operator policy approval for UN critical alerts?
