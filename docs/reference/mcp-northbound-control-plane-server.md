# MCP Northbound Control-Plane Server

Stable reference for bearer-auth, caller-scoped capability filtering, per-command audit journaling, and `journal_id` surfacing on the Rust control-plane daemon's northbound surfaces.

This document describes the implemented system. It is not a proposal or future-state design.

Related stable docs:

- [rust-control-plane.md](rust-control-plane.md)
- [run-control.md](run-control.md)
- [failed-stage-evidence-delivery-preflight-and-mcp-resolution.md](failed-stage-evidence-delivery-preflight-and-mcp-resolution.md)
- [steward-analysis-system.md](steward-analysis-system.md)
- [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md)
- [test-gates.md](test-gates.md)
- [ui-action-boundary.md](ui-action-boundary.md)

## Purpose

The Rust control-plane daemon exposes two northbound surfaces on a single port (default `0.0.0.0:4000`):

- **MCP** — JSON-RPC over stdio and Streamable HTTP (`/mcp`). Serves tools and resources to agents, CLIs, and automations.
- **GraphQL** — HTTP POST `/graphql` and subscriptions over `/graphql/ws`. Serves read queries, approval-gate decisions, and event streams to the macOS operator UI.

MCP is the external control plane for operational commands. GraphQL is the UI read/subscription surface plus the approval-only human-gate mutation path. Non-approval operational commands such as run start, cancellation, stage retry, session reset, compaction, recovery, cloning, and experiment control are MCP-only. The governed UI action boundary is summarized in [ui-action-boundary.md](ui-action-boundary.md).

Both surfaces are authenticated with bearer tokens and filter their visible surface area by the caller's principal class. MCP command tools and approval-gate GraphQL mutations converge on a single `engine::command_handler::CommandHandler` for command execution. Every command execution writes an auditable row to `command_journal` tagged with the caller's surface, principal id, principal class, and tool/mutation name.

## Scope

This reference covers:

- the `auth` crate and principal table (bootstrap, fail-closed, file-mode)
- domain-owned typed capability identifiers
- bearer auth on MCP HTTP, MCP stdio (via `initialize`), GraphQL HTTP, and GraphQL WebSocket transports
- per-class capability filtering for MCP `tools/list`, `tools/call`, `resources/list`, `resources/read`
- the GraphQL approval-only mutation boundary and legacy non-UI compatibility boundary
- `CommandHandler` caller-context propagation and `command_journal` audit row shape
- the `command_journal` redaction matrix
- `journal_id` surfacing on MCP command-tool responses and approval mutation payload wrappers
- the typed `DeliveryPreflight` object on blocked MCP `runs.start`
- the dogfood `.mcp.json` / `CLAUDE.md` contract
- the canonical `proposal-029-mcp` verification gate

It does not cover:

- southbound per-agent MCP policy (covered by [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md))
- token rotation, revocation, or delegation (daemon reads the principal table once at startup; changes require a restart)
- YAML-driven or per-subscription capability policy (first-wave uses static class tables)

## Crate graph

Auth lives in its own crate so that both northbound server crates can depend on it without creating a reverse edge:

```text
domain  <-  auth  <-  mcp-server
               <-  graphql-server
               <-  engine
```

- `domain` has no new dependencies; it owns `PrincipalClass`, `CallerSurface`, `CallerContext`, `CapabilityToolId`, and `ResourceTemplateId`.
- `auth` depends only on `domain`. It owns `Principal`, `PrincipalTable`, `AuthError`, `resolve_bearer`, `extract_bearer_token`, `filter_tools`, `filter_resources`, `match_resource_uri`, and the static class -> capability tables.
- `mcp-server` and `graphql-server` depend on `auth` and `engine`. Neither depends on the other.

Implementation: `control-plane/crates/domain/src/commands.rs` (`PrincipalClass`, `CallerSurface`, `CallerContext`), `control-plane/crates/domain/src/capabilities.rs` (`CapabilityToolId`, `ResourceTemplateId`), `control-plane/crates/auth/src/lib.rs` (principal types, token resolution, capability filtering).

## Typed capability and resource identifiers

`CapabilityToolId` and `ResourceTemplateId` are closed enums in `domain`. Adding a new MCP tool or resource template requires adding a variant here, which forces Rust's exhaustive-match checker to flag every policy table, filter, and converter that has not yet been updated.

### Registered tool capabilities

| `CapabilityToolId` variant | MCP tool name | Command tool? |
|---|---|---|
| `IdeasCreate` | `ideas.create` | no (direct) |
| `IdeasList` | `ideas.list` | no (direct) |
| `RunsStart` | `runs.start` | yes |
| `RunsList` | `runs.list` | no (direct) |
| `RunsGet` | `runs.get` | no (direct) |
| `RunsCancel` | `runs.cancel` | yes |
| `ApprovalsList` | `approvals.list` | no (direct) |
| `ApprovalsResolve` | `approvals.resolve` | yes |
| `StagesRetry` | `stages.retry` | yes |
| `WorkflowConflictsResolve` | `workflow_conflicts.resolve` | yes |
| `LegacyDiscoveryOverrideCreate` | `legacy_discovery_override_create` | yes |
| `ReportsGet` | `reports.get` | no (direct) |
| `ArtifactsOverrideContract` | `artifacts.override_contract` | yes |
| `StewardRunAnalysis` | `steward.run_analysis` | yes |
| `StewardListAnalyses` | `steward.list_analyses` | no (direct) |
| `StewardGetAnalysis` | `steward.get_analysis` | no (direct) |
| `RunsMainSyncRequest` | `runs.main_sync.request` | yes |
| `RunsMainSyncRetry` | `runs.main_sync.retry` | yes |
| `RunsMainSyncSetOverride` | `runs.main_sync.set_override` | yes |
| `RunsMainSyncRepairState` | `runs.main_sync.repair_state` | yes |
| `RunsMainSyncRecordRecoveryDecision` | `runs.main_sync.record_recovery_decision` | yes |
| `RunsKnowledgeCapsuleIgnore` | `runs.knowledge_capsule.ignore` | yes |
| `ProposalGateSettle` | `runs.settle_proposal_gate` | yes |
| `EffectsList` | `effects.list` | no (direct) |
| `EffectsInspect` | `effects.inspect` | no (direct) |
| `EffectsReconcile` | `effects.reconcile` | no (direct) |
| `EffectsMarkUnrecoverable` | `effects.mark_unrecoverable` | no (direct) |
| `EffectsClearAfterManualVerification` | `effects.clear_after_manual_verification` | no (direct) |

Command tools build a typed `Command` enum value and call `CommandHandler::handle`; they emit a `command_journal` row and return `journal_id`. Direct tools call repo functions directly and do not produce journal rows or `journal_id`.

### MCP tool payloads

#### `approvals.resolve`

The `approvals.resolve` tool supports additive evolution to handle both legacy stage approvals and new lead mediation confirmations.

**Legacy Stage Approval (subject_kind implicitly stage_approval):**
- `run_id`: UUID
- `stage_id`: String
- `decision`: `"granted"` | `"rejected"`
- `comment`: String (optional)

**Lead Mediation Confirmation:**
- `subject_kind`: `"lead_mediation_confirmation"` (required)
- `subject_id`: UUID
- `decision`: `"confirm"` | `"manual_fallback"`
- `conflict_fingerprint`: String (must match current mediation truth)
- `idempotency_key`: UUID
- `comment`: String (optional)

### Registered resource templates

| `ResourceTemplateId` variant | URI shape |
|---|---|
| `RunEntity` | `run://{run_id}` |
| `IdeaEntity` | `idea://{idea_id}` |
| `ArtifactEntity` | `artifact://{artifact_id}` |
| `ReportEntity` | `report://{run_id}` |
| `StewardAnalysisEntity` | `steward-analysis://{analysis_id}` |
| `ChainworksRuns` | `chainworks://runs` |
| `ChainworksIdeas` | `chainworks://ideas` |
| `ChainworksApprovalsInbox` | `chainworks://approvals/inbox` |
| `ChainworksRunStages` | `chainworks://runs/{run_id}/stages` |
| `ChainworksRunArtifacts` | `chainworks://runs/{run_id}/artifacts` |

### Execution-truth report readback

`reports.get` and `report://{run_id}` expose the same execution-truth report family.
For agent executions, the report includes runtime facts persisted by the Rust control
plane:

- `failure_kind`
- `failure_kind_raw_debug`
- `failure_kind_version`
- `failure_message_redacted`
- `failure_message_redaction_version`
- `retry_after`
- `operator_action_hint`
- `provider_exit_status`
- `transport_error_code`
- `supervision_classification`
- `output_settlement`
- `valid_required_outputs`
- `late_output_count`
- `ignored_late_output_count`
- `session_reuse_reason`
- `quota_ledger_id`
- timestamps and active session-generation matching fields

**Workflow Conflict:**
The report also includes the `workflow_conflict` object containing
current blocking conflicts, conflict history, and advisory rejection records.

Readback rules:
...
- operator principals may see raw debug failure-kind detail,
- observer and agent principals receive redacted/null raw debug fields,
- `reports.get` and `report://{run_id}` must stay in parity for the same run,
- missing runtime-facts rows use legacy-safe defaults rather than synthetic failure truth.

Implementation: `control-plane/crates/domain/src/capabilities.rs`, `control-plane/crates/mcp-server/src/tools/mod.rs` (`capability_id_for`, `mcp_tool_for`, `all_capability_tool_ids`), `control-plane/crates/mcp-server/src/server.rs` (`resource_template_id_for_uri`).

## Principal table

### File shape

```json
{
  "principals": [
    { "token": "<uuid>", "id": "<human-label>", "class": "operator" }
  ]
}
```

`class` is one of `operator`, `agent`, or `observer`.

### Discovery and bootstrap

- **Env var:** `CHAINWORKS_AUTH_PRINCIPALS_PATH` names an absolute path to the principal table JSON.
- **Default:** `~/.chainworks/auth/principals.json`.
- **Empty env:** if `CHAINWORKS_AUTH_PRINCIPALS_PATH` is set to the empty string, the daemon refuses to start.
- **Missing file:** on first start the daemon generates a random UUID token, writes a single-entry table with `class = operator` and `id = default-operator` to the discovered path, and logs the path + token at `info` level exactly once.
- **File mode:** on Unix, bootstrap opens the file with `OpenOptions::mode(0o600)` before writing, so only the owning user can read it. On non-Unix platforms the mode is not enforced and the corresponding test is `cfg(unix)`-gated.
- **Empty table:** if the file parses successfully but contains zero principals, the daemon refuses to start with `AuthError::TableLoadFailed("principal table contains zero entries")`. There is no silent auth-disabled mode.

### Token resolution

`auth::extract_bearer_token` parses `"Bearer <token>"` from an `Authorization` header value (trimming whitespace; rejecting an empty token with `AuthError::MalformedHeader`). `auth::resolve_bearer` looks the token up in the `PrincipalTable` and returns a fresh `Principal` whose `tool_capabilities` and `resource_capabilities` are populated from the class's default capability set.

### Rotation

Token rotation, revocation, and delegation are out of scope for the current implementation on MCP transports. The principal table consumed by MCP HTTP and stdio is loaded once at daemon startup; to rotate or revoke an MCP token, edit `principals.json` and restart the daemon.

P046 GraphQL session observability subscriptions are the exception: the daemon spawns a periodic `principals.json` reloader (default interval 2 s, override with `CHAINWORKS_PRINCIPALS_RELOAD_SECS`) that updates the dedicated P046 live auth source used by per-emission authorization recheck. A revocation written to the file is observed by `sessionStatusChanged` within roughly one reload interval and the affected stream terminates fail-closed; reload failures mark the source unavailable so subscriptions terminate with `authorization_recheck_failed` rather than continuing under stale grants. MCP token resolution is not affected by this reloader.

Bearer-token equality on every lookup uses a constant-time byte comparison so timing side channels cannot probe the principal table.

Implementation: `control-plane/crates/auth/src/lib.rs` (`PrincipalTable::load_or_bootstrap`, `resolve_bearer`, `extract_bearer_token`, `ct_eq_bytes`), `control-plane/crates/daemon/src/main.rs` (`principals_path_from_env`, P046 principals reload loop).

## Bearer auth on transports

### MCP HTTP (`POST /mcp`)

`handle_mcp_post` parses `Authorization: Bearer <token>` on every request, resolves it through `auth::resolve_bearer`, and returns a JSON-RPC error with `code = -32000` and `message = "unauthorized"` (HTTP 200 body) on any of: missing header, malformed header, unknown token. Valid requests carry the resolved `Principal` into `McpServer::handle_request`. `Mcp-Session-Id` correlation is preserved; the principal is re-resolved per request rather than cached by session.

Implementation: `control-plane/crates/mcp-server/src/http.rs`.

### MCP stdio (JSON-RPC `initialize`)

Authentication piggybacks on the MCP `initialize` method:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize",
 "params":{"clientInfo":{"name":"...","version":"...","principal_token":"<token>"}}}
```

Contract (see `McpServer::run_stdio`):

1. Any first frame whose method is not `initialize` receives `-32002 "server not initialized"` and closes the session.
2. `initialize` without `params.clientInfo.principal_token` receives `-32000 "unauthorized: principal_token required on initialize"` and closes.
3. `initialize` with an unresolvable token receives `-32000 "unauthorized: unknown token"` and closes.
4. On success the `Principal` is bound to the stdio session for its lifetime.
5. A mid-session second `initialize` is rejected with `-32600 "Session already initialized"`; the bound principal is not replaced (session-lifetime immutability).

Implementation: `control-plane/crates/mcp-server/src/server.rs` (`run_stdio`), `control-plane/crates/mcp-server/src/protocol.rs`.

### GraphQL HTTP (`POST /graphql`)

The `/graphql` HTTP route is wrapped in an axum middleware (`auth_layer::require_auth`) that:

- parses `Authorization: Bearer <token>`,
- resolves through `auth::resolve_bearer`,
- on success inserts the `Principal` into the request's `http::Extensions` so `async_graphql::Context::data::<auth::Principal>()` can retrieve it in resolvers,
- on failure responds with HTTP 401 and a GraphQL-shaped error body: `{ "errors": [{ "message": "unauthorized", "extensions": { "code": "UNAUTHORIZED" } }] }`.

The GET playground is exempt from auth only when `CHAINWORKS_PLAYGROUND_AUTH=skip` is set at daemon startup.

Implementation: `control-plane/crates/graphql-server/src/auth_layer.rs`, `control-plane/crates/graphql-server/src/server.rs` (`build_router`).

### GraphQL WebSocket (`/graphql/ws`)

WebSocket subscriptions are mounted outside the HTTP auth middleware because the middleware cannot reject a WebSocket upgrade while still allowing a `connection_init` handshake to fire. Auth is enforced inside the `connection_init` callback after the WebSocket opens:

- the `connection_init` payload must include `{ "Authorization": "Bearer <token>" }`,
- `connection_init_data` extracts the bearer token and calls `auth::resolve_bearer`,
- on success the resolved `Principal` is injected into the subscription's `async_graphql::Data` and subscription resolvers can read it,
- on missing or unresolvable token, the handler returns `async_graphql::Error("unauthorized")` and no subscription resolver fires.

Implementation: `control-plane/crates/graphql-server/src/server.rs` (`graphql_ws_handler`, `connection_init_data`).

## Per-class capability policy

The `auth::filter_tools` and `auth::filter_resources` functions combine two checks for every tool or resource template:

1. `tool_allowed_for_class` (or `resource_allowed_for_class`) — the static class -> capability policy table in `control-plane/crates/auth/src/lib.rs`.
2. Membership in the principal's own `tool_capabilities` / `resource_capabilities` set (populated from the class defaults at `Principal::new`).

Both checks are required, so a future change that wants to narrow a specific principal below the class default can do so by editing the set without also mutating the class table.

### MCP tool policy

| Tool | Operator | Agent | Observer |
|---|:-:|:-:|:-:|
| `ideas.create` | yes | yes | no |
| `ideas.list` | yes | yes | yes |
| `runs.start` | yes | yes | no |
| `runs.list` | yes | yes | yes |
| `runs.get` | yes | yes | yes |
| `runs.cancel` | yes | no | no |
| `approvals.list` | yes | no | yes | (Mixed inbox: stage approvals + lead mediation confirmations) |
| `approvals.resolve` | yes | no | no | (Resolves stage approvals or lead mediation confirmations) |
| `stages.retry` | yes | no | no |
| `workflow_conflicts.resolve` | yes | no | no |
| `legacy_discovery_override_create` | yes | no | no |
| `reports.get` | yes | yes | yes |
| `artifacts.override_contract` | yes | no | no |
| `steward.run_analysis` | yes | no | no |
| `steward.list_analyses` | yes | no | yes |
| `steward.get_analysis` | yes | no | yes |
| `runs.main_sync.*` | yes | no | no | (Includes request, retry, set_override, repair_state, record_recovery_decision) |
| `runs.knowledge_capsule.ignore` | yes | no | no |
| `runs.settle_proposal_gate` | yes | no | no |
| `effects.list` | yes | no | no |
| `effects.inspect` | yes | no | no |
| `effects.reconcile` | yes | no | no |
| `effects.mark_unrecoverable` | yes | no | no |
| `effects.clear_after_manual_verification` | yes | no | no |

Rationale for the Steward trio: `run_analysis` queues compute work and drives the quality-gate pipeline, so only operators can trigger it. `list_analyses` and `get_analysis` are read-only over persisted analysis records and are visible to the operational/audit (observer) class. Agents are scoped to executing their own run and have no legitimate cross-cohort read surface, so they see none of the three.

### MCP resource policy

| Resource template | Operator | Agent | Observer |
|---|:-:|:-:|:-:|
| `run://{run_id}` | yes | yes | yes |
| `idea://{idea_id}` | yes | yes | yes |
| `artifact://{artifact_id}` | yes | yes | yes |
| `report://{run_id}` | yes | yes | yes |
| `steward-analysis://{analysis_id}` | yes | no | yes |
| `chainworks://runs` | yes | yes | yes |
| `chainworks://ideas` | yes | yes | yes |
| `chainworks://approvals/inbox` | yes | no | yes |
| `chainworks://runs/{run_id}/stages` | yes | no | yes |
| `chainworks://runs/{run_id}/artifacts` | yes | no | yes |

### GraphQL mutation boundary

GraphQL is not a general-purpose operator command bus. The macOS operator UI may use GraphQL mutations only to resolve human approval gates:

| Mutation | `CapabilityToolId` | Operator | Agent | Observer |
|---|---|:-:|:-:|:-:|
| `approveApproval` | `ApprovalsResolve` | yes | no | no |
| `rejectApproval` | `ApprovalsResolve` | yes | no | no |

`approveApproval` and `rejectApproval` share the same capability because they differ only by `ApprovalDecision`; the surface policy allowlist distinguishes them from all non-approval command mutations. A mutation invoked by a principal whose class or surface policy is not permitted returns `async_graphql::Error("forbidden")` and writes no `command_journal` row.

The following operations are MCP-only for agents, CLIs, automations, and operator diagnostics:

| Operation | MCP tool | SwiftUI via GraphQL |
|---|---|---|
| Create idea | `ideas.create` | no |
| Start run | `runs.start` | no |
| Cancel run | `runs.cancel` | no |
| Retry stage | `stages.retry` | no |
| Resolve workflow conflict | `workflow_conflicts.resolve` | no |
| Override legacy discovery policy | `legacy_discovery_override_create` | no |
| Override artifact contract | `artifacts.override_contract` | no |
| Run Steward analysis | `steward.run_analysis` | no |

Current Rust schema compatibility note: older non-approval GraphQL mutation resolver names may remain in the compiled schema while downstream tests are retired, but production/default UI principals fail closed for those fields. They are not current operator API. SwiftUI must not call them, new UI work must not add call sites for them, and MCP remains the only supported write path for non-approval operations.

### Enforcement points

- `McpServer::handle_request` filters `tools/list` through `visible_tool_specs` (which calls `auth::filter_tools`), and gates every `tools/call` by `principal.tool_capabilities.contains(&tool_id)`. A denied tool returns `-32601 "Method not found: <name>"` (not `"forbidden"`) so the error does not leak capability existence.
- `resources/list` is filtered by `auth::filter_resources` over `auth::all_resource_templates()`.
- `resources/read` parses the concrete URI into a `ResourceTemplateId` via `resource_template_id_for_uri`, then calls `auth::match_resource_uri`. A denied read returns `-32002 "Resource not found"`.
- GraphQL approval mutation resolvers read `Principal` from `async_graphql::Context`, call the same capability policy, and return `Error::new("forbidden")` on denial.

Implementation: `control-plane/crates/auth/src/lib.rs` (`filter_tools`, `filter_resources`, `match_resource_uri`, `tool_allowed_for_class`, `resource_allowed_for_class`), `control-plane/crates/mcp-server/src/server.rs` (`handle_request`, `visible_tool_specs`), `control-plane/crates/graphql-server/src/schema.rs` (approval mutation resolvers and explicitly quarantined legacy compatibility resolvers).

## Caller context and command journal audit

### `CallerContext`

Every command invocation carries a `CallerContext` from `domain::commands`:

```rust
pub struct CallerContext {
    pub surface: CallerSurface,      // Mcp | Graphql
    pub principal_id: String,
    pub principal_class: PrincipalClass,
    pub caller_tool: String,         // MCP tool name or approval GraphQL mutation name
}
```

Constructors:

- `CallerContext::mcp(principal_id, &principal_class, tool_name)` — built at every MCP command-tool entry point.
- `CallerContext::graphql(principal_id, &principal_class, mutation_name)` — built inside GraphQL approval mutation resolvers after the capability check. Legacy non-approval GraphQL mutation rows may still appear only through explicit compatibility fixtures while those resolvers remain compiled.
- `CallerContext::test_fixture()` — plain `pub fn` (not `cfg(test)`) so that integration tests in `engine/tests/`, `graphql-server/tests/`, `mcp-server/src/` and `daemon/tests/` can construct a well-formed context without touching auth. Rows tagged with this constructor show `caller_surface = "mcp"`, `principal_id = "test-operator"`, `principal_class = "operator"`, `caller_tool = "test"`.

No `Internal` variant is defined. Extending `CallerSurface` to cover internal callers (executor, recovery) happens only when a future slice routes those callers through `CommandHandler`.

### `CommandHandler::handle` signature

```rust
pub async fn handle(&self, cmd: Command, caller: CallerContext) -> Result<Commanded>;

pub struct Commanded {
    pub result: CommandResult,
    pub journal_id: String,
}
```

The handler:

1. Mints a fresh UUID `journal_id`.
2. Serializes the typed `Command` to JSON.
3. Passes the raw JSON to `engine::command_journal_redact::redact_for_journal` (see the matrix below).
4. Inserts one row into `command_journal` with the redacted payload, the caller's surface / principal id / principal class / tool name, and (when the command carries one) the associated `run_id`. The insert is mandatory; a failed insert fails the command.
5. Executes the command.
6. On success, calls `command_journal::complete_entry`. On failure, calls `command_journal::fail_entry` with the error message. Completion and failure writes are best-effort and log their own errors.
7. On success returns `Commanded { result, journal_id }`; on failure returns the underlying error.

### `command_journal` schema

Migration `011_auth_tracking.sql` adds four nullable TEXT columns to the existing `command_journal` table (schema in `control-plane/crates/db/migrations/001_initial.sql`):

| Column | Value |
|---|---|
| `caller_surface` | `"mcp"` or `"graphql"` (serde `snake_case`) |
| `caller_principal_id` | principal id from the auth table (e.g. `default-operator`, `observer`, `test-operator`) |
| `caller_principal_class` | `"operator"`, `"agent"`, or `"observer"` |
| `caller_tool` | MCP tool name (e.g. `runs.start`) or approval GraphQL mutation name (e.g. `approveApproval`) |

Columns are nullable so pre-P029 rows remain readable. Post-P029 rows written through `CommandHandler::handle` always populate all four.

Implementation: `control-plane/crates/db/migrations/011_auth_tracking.sql`, `control-plane/crates/engine/src/command_handler.rs` (`handle`, `Commanded`), `control-plane/crates/domain/src/commands.rs` (`CallerContext`, `CallerSurface`, `PrincipalClass`).

## Redaction matrix

`engine::command_journal_redact::redact_for_journal` applies a per-variant matrix to the serialized `Command` before `command_journal::record` writes the row. The function matches exhaustively on the `Command` enum; adding a new variant forces a compile error here until the author records an explicit decision.

Each decision is one of:

- **`preserve`** — keep the field value verbatim.
- **`redact`** — replace the value with the string `"[REDACTED]"` while preserving key presence and type shape. Null values stay null so audit readers can distinguish "genuinely absent" from "hidden".
- **`omit`** — drop the field from the serialized JSON entirely.

| Command variant | Field | Decision | Rationale |
|---|---|---|---|
| `StartRun` | `idea_id` | preserve | Audit: which idea triggered the run. |
| `StartRun` | `workflow_id`, `workflow_title` | preserve | Audit: which workflow fired. |
| `StartRun` | `workspace_root`, `artifact_root` | preserve | Audit: where artifacts land. |
| `StartRun` | `workflow_yaml_path`, `agent_catalog_yaml_path` | preserve | Audit: config provenance (paths, not contents). |
| `StartRun` | `delivery_configuration_json` | **redact** | Contains repo identifier, repo root, and target branch; must not sit in operator-readable audit logs. Presence is preserved via `"[REDACTED]"`. |
| `ApproveStage` | `run_id`, `stage_id` | preserve | Audit: which approval was granted. |
| `ApproveStage` | `comment` | **redact** | Free-form operator text may carry credentials, incident detail, or personal notes. |
| `RejectStage` | `run_id`, `stage_id` | preserve | Audit: which approval was rejected. |
| `RejectStage` | `comment` | **redact** | Same rationale as `ApproveStage.comment`. |
| `RetryStage` | all fields | preserve | No sensitive payload. |
| `ResolveWorkflowConflictTransition` | all fields | preserve | Operator selections are audit material. |
| `OverrideLegacyDiscoveryPolicy` | all fields | preserve | Typed override fields are audit material. |
| `CancelRun` | all fields | preserve | No sensitive payload. |
| `ResetSession` | all fields | preserve | No sensitive payload. |
| `RunStewardAnalysis` | `reason`, `artifact_base` | preserve | Operator label + optional output path override. |
| `OverrideArtifactContract` | all fields | preserve | Typed override fields are audit material. |
| `ResolveLeadMediationConfirmation` | `comment` | **redact** | Redact operator-submitted text. |

Variants with no sensitive fields (`RetryStage`, `CancelRun`, `ResetSession`, `RunStewardAnalysis`) correctly preserve every field — the contract does not require every variant to hide something.

Implementation: `control-plane/crates/engine/src/command_journal_redact.rs` (both the function and the focused-test module that pins every decision).

## `journal_id` wire contract

### MCP command tools

The five MCP command tools (`runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, `steward.run_analysis`) include `journal_id` inside their `tools/call` response. The value lives at `result.content[0].text` as stringified JSON, alongside the tool's existing result fields:

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "result": {
    "content": [
      { "type": "text",
        "text": "{\"run_id\":\"...\",\"journal_id\":\"...\"}" }
    ]
  }
}
```

The server currently advertises `protocolVersion: "2024-11-05"`, which predates MCP's `structuredContent` typed-output channel; `journal_id` therefore flows only inside `content[0].text`.

### MCP direct tools

The eight direct tools (`ideas.create`, `ideas.list`, `runs.list`, `runs.get`, `approvals.list`, `reports.get`, `steward.list_analyses`, `steward.get_analysis`) call repo functions directly and bypass `CommandHandler`. They write no `command_journal` row and their response JSON does not include `journal_id`.

### GraphQL approval mutation payload wrappers

Approval mutations return dedicated payload objects so that `journalId: ID!` lives on the mutation result and does not pollute shared entity types (`Approval`) that read queries also return.

| Mutation | Return type |
|---|---|
| `approveApproval` | `ApproveApprovalPayload { approval: Approval!, journalId: ID! }` |
| `rejectApproval` | `RejectApprovalPayload { approval: Approval!, journalId: ID! }` |

Clients select the field via normal GraphQL syntax:

```graphql
mutation {
  approveApproval(approvalId: "...") {
    approval { id decision decidedAt }
    journalId
  }
}
```

### Blocked `runs.start` preflight

MCP `runs.start` returns a typed blocked delivery-preflight payload when repo-backed run creation is rejected before a run row exists. The payload includes `passed` and individual `checks`; callers must not parse generic error strings to determine preflight status.

Compatibility note: the compiled schema may still contain older `startRun`-family fields for historical tests, but production/default UI principals fail closed for non-approval GraphQL command mutations. The typed `DeliveryPreflight` wrapper is not part of the SwiftUI target contract; use MCP `runs.start` for run creation.

### Absent `journal_id`

A command tool call that errors out before `CommandHandler::handle` is reached (capability denial, argument-validation failure) returns only the error variant; no `journal_id` is included, because no `command_journal` row was written. Clients that observe a response without `journal_id` should treat the result as having no audit trail in `command_journal`.

Implementation: `control-plane/crates/mcp-server/src/tools/runs.rs` and sibling tool modules (journal-id insertion inside `content[0].text`), `control-plane/crates/graphql-server/src/schema.rs` (approval payload wrapper types and explicitly quarantined legacy compatibility resolvers).

## Dogfood configuration

The repo-root `.mcp.json` registers this daemon as an MCP server for Claude Code:

```json
{
  "mcpServers": {
    "chainworks-control-plane": {
      "type": "http",
      "url": "http://127.0.0.1:4000/mcp",
      "headers": {
        "Authorization": "Bearer ${CHAINWORKS_MCP_TOKEN}"
      }
    }
  }
}
```

The header references `CHAINWORKS_MCP_TOKEN`, which operators set in their shell. After first-start bootstrap the operator reads the token from `~/.chainworks/auth/principals.json` (or copies it from the one-time bootstrap log line) and exports it.

`CLAUDE.md` documents the same URL, the `Authorization: Bearer` contract, and the env var name. The two files are kept in sync by focused tests that live alongside the daemon and fail the `proposal-029-mcp` gate on drift.

Implementation: `.mcp.json` at repo root, `CLAUDE.md` operator guidance, `control-plane/crates/daemon/tests/dogfood_config.rs`.

## Verification

The canonical verification command is:

```bash
./scripts/test-gate.sh proposal-029-mcp
```

The gate enumerates a fixed inventory of focused tests covering:

- principal-table bootstrap (0o600 file mode, one-time token log, empty-file fail-closed),
- transport auth for MCP HTTP, MCP stdio, GraphQL HTTP, GraphQL WebSocket,
- capability filtering for tool lists, tool calls, resource lists, and resource reads (including the Steward trio and the `steward-analysis://` resource),
- command-journal caller-metadata rows (per surface, per class),
- the §8.1 redaction matrix (one test per decision),
- `journal_id` surfacing on MCP command tools and approval GraphQL mutation payload wrappers,
- the typed `DeliveryPreflight` object on blocked MCP `runs.start`,
- the UI action boundary: GraphQL is read/subscription plus approval-only mutation; MCP owns non-approval operational commands,
- dogfood `.mcp.json` and `CLAUDE.md` consistency.

For each test in the inventory the gate runs `cargo test -p <crate> <name> -- --nocapture` and post-checks the output for a matching `test <name>` line, so a rename, typo, or deletion fails the gate independently of the test body. After every focused test passes, the gate runs `cargo test --workspace` as a final regression step.

Gate ownership and the inventory source of truth are documented in [test-gates.md](test-gates.md). The proof log for the current implementation lives at [../evidence/029-mcp-northbound-control-plane-server/README.md](../evidence/029-mcp-northbound-control-plane-server/README.md).

## Non-goals

The following items are explicitly out of scope for this reference:

- **Southbound per-agent MCP policy.** Covered by [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md). Northbound and southbound are different layers and do not share code.
- **Token rotation, revocation, delegation.** The table is read once at startup; editing requires a restart. A future auth-lifecycle slice owns rotation.
- **YAML-driven capability policy.** First-wave uses static class tables in `auth::tool_allowed_for_class` and `auth::resource_allowed_for_class`.
- **Per-subscription capability filtering.** All authenticated principals can subscribe to all subscriptions; narrowing is a future slice.
- **Internal `CallerSurface` variant.** Executor and recovery do not route through `CommandHandler` today. The internal-caller audit lane is reserved for a future command-path consolidation slice.
- **MCP `structuredContent` typed-output channel.** The server advertises `protocolVersion: "2024-11-05"`; a future protocol-uplift slice adds the typed `structuredContent.journal_id` mirror.
- **Immediate physical deletion of legacy non-approval GraphQL mutation resolvers.** The resolvers remain compatibility surface only, not a product or SwiftUI action-routing contract. Production-style principals fail closed unless they carry explicit approval-only GraphQL surface policy.
