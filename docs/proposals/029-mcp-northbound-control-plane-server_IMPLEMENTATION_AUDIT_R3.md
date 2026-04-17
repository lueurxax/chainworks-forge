# Proposal 029 Implementation Audit R3 — MCP Northbound Control-Plane Server

**Audit date:** 2026-04-16  
**Auditor:** Codex  
**Proposal:** `docs/proposals/029-mcp-northbound-control-plane-server.md`  
**Report path:** `docs/proposals/029-mcp-northbound-control-plane-server_IMPLEMENTATION_AUDIT_R3.md`  
**Git HEAD:** `345956b854358ad12a478867469aee6025d8a7c0`  
**Working tree note:** The proposal file and unrelated docs/reference files were already dirty before this audit. This audit is read-only except for this report.

## Verdict

**Overall Conformance:** Not Implemented

**Overall Readiness:** Not Ready

**Audit Confidence:** High

The current tree contains a substantial partial implementation: MCP HTTP auth, MCP stdio token binding, command-journal caller columns, `CommandHandler::handle -> Commanded`, MCP command-tool journaling for most command tools, GraphQL mutation wrappers with `journalId`, redaction, and dogfood `.mcp.json` auth wiring. The same-tree `proposal-029-mcp` gate is green.

However, Proposal 029 R8 explicitly forbids landing while the stale string-based auth scaffold remains, and that stale scaffold is still the active implementation. Several acceptance-critical contracts are missing or contradicted by tests that currently pass:

- `domain` still depends on `auth`; `PrincipalClass` still lives in `auth`; `CapabilityToolId` and `ResourceTemplateId` do not exist in `domain`.
- Capability/resource policy is string-based, not typed, and the actual steward policy grants `agent` steward read access that the proposal explicitly forbids.
- GraphQL WebSocket subscription auth via `connection_init` is absent.
- `steward.run_analysis` invokes `CommandHandler` but does not return `journal_id`.
- MCP stdio first non-`initialize` returns the wrong JSON-RPC error code.
- The green gate is under-scoped and includes a test that asserts the wrong steward resource policy.

Because in-scope requirements are missing and some tests encode policy opposite to the proposal, this implementation is not proposal-ready even though the current test gate passes.

## Scope And Method

This audit followed the `proposal-implementation-audit` skill in read-only mode. It checked:

- Proposal contract and acceptance criteria in `docs/proposals/029-mcp-northbound-control-plane-server.md`.
- Test-gate registration in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.
- Rust implementation under `control-plane/crates/{auth,domain,mcp-server,graphql-server,engine,db,daemon}`.
- Dogfood MCP client config in `.mcp.json` and `CLAUDE.md`.
- Same-tree regression via `./scripts/test-gate.sh proposal-029-mcp`.

## Regression Evidence

Command run:

```bash
./scripts/test-gate.sh proposal-029-mcp
```

Result:

```text
==> Proposal 029-MCP control-plane gate passed
```

The gate runs `cargo test --workspace` from `control-plane` (`scripts/test-gate.sh:1445-1451`) and completed successfully. This is useful regression evidence for the partial implementation, but it is not sufficient readiness evidence because the source audit found missing requirements and misaligned tests.

## Track 1 — Requirement Conformance

### REQ-001 — Type ownership and dependency direction

**Status:** Missing  
**Proposal evidence:** §4.0 requires `domain ← auth`, says `domain` gains no new workspace-crate dependencies, and makes `domain` the owner of `PrincipalClass`, `CallerSurface`, `CallerContext`, `CapabilityToolId`, and `ResourceTemplateId` (`docs/proposals/029-mcp-northbound-control-plane-server.md:154-241`). §4.5 explicitly says the stale artifacts must be replaced in one slice and the gate cannot be green while they remain (`docs/proposals/029-mcp-northbound-control-plane-server.md:530-546`).

**Implementation evidence:**

- `PrincipalClass` is defined in `auth`, not `domain` (`control-plane/crates/auth/src/lib.rs:12-18`).
- `domain` depends on `auth` (`control-plane/crates/domain/Cargo.toml:7`).
- `CallerContext.principal_class` uses `auth::PrincipalClass` (`control-plane/crates/domain/src/commands.rs:82-88`).
- `domain/src/capabilities.rs` does not exist.
- `domain/src/lib.rs` re-exports `auth::PrincipalClass` instead of owning it (`control-plane/crates/domain/src/lib.rs:14-15`).

**Gap:** The implemented dependency direction is the reverse of the proposal. This removes the compile-time drift guarantee that P029 uses as its main hardening mechanism.

### REQ-002 — Principal table bootstrap and fail-closed loading

**Status:** Partially Implemented  
**Proposal evidence:** P029 requires `CHAINWORKS_AUTH_PRINCIPALS_PATH`, default `~/.chainworks/auth/principals.json`, first-start bootstrap, owner-only `0600` permissions set via `OpenOptions::mode(0o600)` before write, token logged exactly once, and fail-closed behavior for empty/unparseable files (`docs/proposals/029-mcp-northbound-control-plane-server.md:247-256`, `625-644`).

**Implementation evidence:**

- Daemon reads `CHAINWORKS_AUTH_PRINCIPALS_PATH` and defaults to `~/.chainworks/auth/principals.json` (`control-plane/crates/daemon/src/main.rs:109-118`).
- `PrincipalTable::load_or_bootstrap` creates an operator token when the file is missing and rejects zero principals (`control-plane/crates/auth/src/lib.rs:79-95`, `100-123`).
- The implementation writes the file and then calls `set_permissions(0o600)` (`control-plane/crates/auth/src/lib.rs:110-119`).

**Gap:** Permissions are not set with `OpenOptions::mode(0o600)` before writing as required, leaving a creation-window mismatch against the contract. No focused test matching `test_principals_file_created_with_owner_only_permissions` or `test_principals_bootstrap_token_logged_once_on_first_start` was found.

### REQ-003 — MCP HTTP bearer auth

**Status:** Implemented  
**Proposal evidence:** MCP HTTP must parse `Authorization: Bearer`, resolve the token, reject unauthenticated calls with JSON-RPC `-32000`, and pass the principal to `McpServer::handle_request` (`docs/proposals/029-mcp-northbound-control-plane-server.md:258-265`, `629`).

**Implementation evidence:** `http.rs` extracts the bearer header, resolves against the principal table, returns JSON-RPC unauthorized errors, and passes the resolved principal into `handle_request` (`control-plane/crates/mcp-server/src/http.rs:46-80`).

### REQ-004 — MCP stdio initialize auth and session immutability

**Status:** Partially Implemented  
**Proposal evidence:** First non-`initialize` must return `-32002 / "server not initialized"`; missing/unknown `principal_token` must return `-32000`; after initialize, the principal is bound for the session; second initialize is rejected (`docs/proposals/029-mcp-northbound-control-plane-server.md:267-288`, `629-631`).

**Implementation evidence:**

- Initialize reads `params.clientInfo.principal_token` and binds a resolved principal (`control-plane/crates/mcp-server/src/server.rs:80-139`).
- Second initialize is rejected (`control-plane/crates/mcp-server/src/server.rs:82-92`).
- Requests before session auth return an error (`control-plane/crates/mcp-server/src/server.rs:142-155`).

**Gap:** The first non-`initialize` path returns `-32000 / "unauthorized: session not initialized"`, not the required `-32002 / "server not initialized"`. No focused stdio auth tests named in the proposal inventory were found.

### REQ-005 — GraphQL HTTP bearer auth and mutation principal checks

**Status:** Partially Implemented  
**Proposal evidence:** GraphQL `/graphql` must reject missing/unresolvable bearer tokens with HTTP 401 and inject `Principal` into `async_graphql::Context`; mutation resolvers must enforce class capability and produce no journal row on forbidden (`docs/proposals/029-mcp-northbound-control-plane-server.md:290-317`, `632`, `635`).

**Implementation evidence:**

- `auth_layer.rs` parses bearer tokens, resolves principals, stores `Principal` in request extensions, and returns HTTP 401 on failure (`control-plane/crates/graphql-server/src/auth_layer.rs:32-82`).
- Mutation resolvers read `ctx.data::<auth::Principal>()`, call `auth::is_tool_allowed`, and return `forbidden` before invoking `CommandHandler` (`control-plane/crates/graphql-server/src/schema.rs:300-308`, `357-366`, `451-460`, `481-490`).

**Gap:** The proposal requires explicit transfer from axum request extensions into `async_graphql::Context` (`with_data` or equivalent). The mounted service is `GraphQL::new(schema)` without visible principal-transfer code (`control-plane/crates/graphql-server/src/server.rs:29-42`). Existing schema tests inject `.data(test_principal())` directly into requests, bypassing the route/middleware seam. This leaves the HTTP route integration not proven by the current tree.

### REQ-006 — GraphQL WebSocket subscription auth

**Status:** Missing  
**Proposal evidence:** `/graphql/ws` must authenticate via `connection_init` payload `Authorization: Bearer <token>`, inject `Principal` into subscription data, return `connection_error`, and close with 4401 on missing/unknown auth (`docs/proposals/029-mcp-northbound-control-plane-server.md:318-331`).

**Implementation evidence:** `/graphql/ws` is mounted with `GraphQLSubscription::new(schema)` and no `on_connection_init` handler (`control-plane/crates/graphql-server/src/server.rs:39-42`). Subscription resolvers do not read a principal (`control-plane/crates/graphql-server/src/schema.rs:505-570`). Search found no `connection_init` / `on_connection_init` auth implementation or tests.

**Gap:** The WebSocket auth contract is absent.

### REQ-007 — MCP tools/list and tools/call capability filtering

**Status:** Partially Implemented  
**Proposal evidence:** `tools/list` and `tools/call` must filter by typed `CapabilityToolId`; denied calls must return `-32601`; class tables must match §4.2 exactly (`docs/proposals/029-mcp-northbound-control-plane-server.md:334-355`, `633-635`).

**Implementation evidence:**

- `tools/list` filters with `auth::is_tool_allowed` and `tools/call` denies disallowed tools with `-32601` (`control-plane/crates/mcp-server/src/server.rs:194-226`).
- The auth API uses string tool names via `ToolSpec` / `is_tool_allowed`, not typed IDs (`control-plane/crates/auth/src/lib.rs:164-182`).
- Agent class includes `steward.list_analyses` and `steward.get_analysis` (`control-plane/crates/auth/src/lib.rs:201-210`).

**Gap:** Filtering exists, but it is the stale string-based implementation forbidden by §4.5 and the agent class violates §4.2, which says agents have no steward access.

### REQ-008 — MCP resource list/read capability filtering

**Status:** Partially Implemented  
**Proposal evidence:** `resources/list` must be capability-filtered like `tools/list`; observer sees `run://`, `idea://`, `artifact://`, `report://`, `steward-analysis://`, and every `chainworks://` collection; agent must not see or read `steward-analysis://`; `resources/read` must reject denied concrete URIs with `-32002` using template-instance matching (`docs/proposals/029-mcp-northbound-control-plane-server.md:582-588`, `640-641`).

**Implementation evidence:**

- `resources/list` filters all resource templates with `auth::is_resource_allowed` (`control-plane/crates/mcp-server/src/server.rs:244-319`).
- `resources/read` checks `auth::is_resource_allowed` and returns JSON-RPC `-32002` when denied (`control-plane/crates/mcp-server/src/server.rs:329-345`).
- Resource policy is string-based (`control-plane/crates/auth/src/lib.rs:226-272`).
- Agent class includes `steward-analysis://{analysis_id}` (`control-plane/crates/auth/src/lib.rs:240-245`).
- Observer class lacks `artifact://{artifact_id}`, `chainworks://runs/{run_id}/stages`, and `chainworks://runs/{run_id}/artifacts` in the static list (`control-plane/crates/auth/src/lib.rs:247-257`).
- The current auth test asserts `steward-analysis://` is allowed for all classes, including agent (`control-plane/crates/auth/src/lib.rs:413-430`).

**Gap:** Enforcement hooks exist, but the actual policy contradicts AC-12/AC-13.

### REQ-009 — Command journal caller metadata

**Status:** Implemented  
**Proposal evidence:** `command_journal` must gain nullable caller columns; MCP and GraphQL command paths must write caller surface, principal id/class, and caller tool (`docs/proposals/029-mcp-northbound-control-plane-server.md:357-376`, `636-637`).

**Implementation evidence:**

- Migration `011_auth_tracking.sql` adds `caller_surface`, `caller_principal_id`, `caller_principal_class`, and `caller_tool` (`control-plane/crates/db/migrations/011_auth_tracking.sql:1-5`).
- `CommandHandler::handle` writes caller metadata before command execution and closes or fails the row afterward (`control-plane/crates/engine/src/command_handler.rs:77-147`).

### REQ-010 — Command payload redaction before journal insert

**Status:** Implemented  
**Proposal evidence:** `command_journal.payload_json` must pass through `engine::command_journal_redact::redact_for_journal` before insert, with at least one sensitive field asserted absent in a focused test (`docs/proposals/029-mcp-northbound-control-plane-server.md:638`).

**Implementation evidence:** `CommandHandler` calls `redact_for_journal` before `command_journal::record` (`control-plane/crates/engine/src/command_handler.rs:89-107`). Redaction removes approval/rejection comments and tests assert sensitive comment text is absent (`control-plane/crates/engine/src/command_journal_redact.rs:8-38`, `47-68`).

### REQ-011 — MCP command tools return `journal_id`

**Status:** Partially Implemented  
**Proposal evidence:** MCP command tools `runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, and `steward.run_analysis` must include `journal_id`; direct read/non-command tools must omit it (`docs/proposals/029-mcp-northbound-control-plane-server.md:639`, `714-728`).

**Implementation evidence:**

- `approvals.resolve` returns `journal_id` (`control-plane/crates/mcp-server/src/tools/approvals.rs:85-89`).
- `stages.retry` returns `journal_id` (`control-plane/crates/mcp-server/src/tools/stages.rs:43-49`).
- `runs.start` and `runs.cancel` return `journal_id` in their command paths (`control-plane/crates/mcp-server/src/tools/runs.rs:90-180`).
- `steward.run_analysis` calls `CommandHandler` but discards `commanded.journal_id` and returns only `analysis_id` / `queued` (`control-plane/crates/mcp-server/src/tools/steward.rs:49-75`).

**Gap:** The steward command tool violates AC-11.

### REQ-012 — GraphQL mutation payloads expose `journalId`

**Status:** Implemented  
**Proposal evidence:** All five mutations must return dedicated payload wrappers with `journalId: ID!` (`docs/proposals/029-mcp-northbound-control-plane-server.md:500-518`, `639`, `729-733`).

**Implementation evidence:** `start_run`, `approve_stage`, `reject_stage`, `retry_stage`, and `cancel_run` all return payloads carrying `journal_id` derived from `Commanded.journal_id` (`control-plane/crates/graphql-server/src/schema.rs:325-341`, `378-391`, `426-439`, `471-475`, `496-500`).

### REQ-013 — Dogfood MCP client migration

**Status:** Implemented  
**Proposal evidence:** `.mcp.json` must include `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}` and `CLAUDE.md` must document the env var and generated principals file (`docs/proposals/029-mcp-northbound-control-plane-server.md:593-619`).

**Implementation evidence:** `.mcp.json` includes the Authorization header (`.mcp.json:1-11`) and `CLAUDE.md` documents `CHAINWORKS_MCP_TOKEN` plus the auto-created principals file (`CLAUDE.md:57-59`).

### REQ-014 — Proposal test gate registration and same-tree execution

**Status:** Partially Implemented  
**Proposal evidence:** P029 requires a green `proposal-029-mcp` gate on the same tree with the focused inventory listed in §9 (`docs/proposals/029-mcp-northbound-control-plane-server.md:647-747`).

**Implementation evidence:** `proposal-029-mcp|p029-mcp` is registered and runs `cargo test --workspace` (`scripts/test-gate.sh:1445-1451`). The command passed on this tree.

**Gap:** The gate is not the focused inventory from §9. Search found no tests with the named WebSocket auth, typed capability, steward agent exclusion, or steward journal-id test names. Worse, one passing test asserts `steward-analysis://` is allowed for all classes (`control-plane/crates/auth/src/lib.rs:413-430`), directly contradicting AC-12/AC-13.

## Track 2 — Architecture, Product, UX, And Readiness Findings

### ARCH-001 — Critical — Dependency graph and typed capability model are not implemented

P029's core architecture is a typed, transport-neutral capability model rooted in `domain`, with `auth` depending on `domain` and server crates depending on `auth`. The current tree keeps `PrincipalClass` in `auth`, makes `domain` depend on `auth`, has no `domain/src/capabilities.rs`, and filters strings. This is not just an implementation detail: it removes the compile-time drift guard that P029 uses to prevent new tools/resources from bypassing capability policy.

**Evidence:** Proposal lines `154-241`, `530-546`; implementation lines `control-plane/crates/auth/src/lib.rs:12-18`, `control-plane/crates/domain/Cargo.toml:7`, `control-plane/crates/domain/src/commands.rs:82-88`, `control-plane/crates/auth/src/lib.rs:164-182`.

**Required fix:** Move `PrincipalClass` into `domain`, create typed `CapabilityToolId` and `ResourceTemplateId`, make `auth` depend on `domain` and remove the `domain -> auth` edge, replace string `ToolSpec` / string resource policy with typed static tables and server-owned converters.

### SEC-001 — Critical — Steward capability policy grants agent access forbidden by proposal

The proposal says agents have no steward access and cannot read `steward-analysis://` resources. Current auth policy grants `agent` both `steward.list_analyses` and `steward.get_analysis`, and grants `steward-analysis://{analysis_id}` resource access to agent. The test suite currently codifies this wrong resource policy.

**Evidence:** Proposal lines `345-354`, `640-641`; implementation lines `control-plane/crates/auth/src/lib.rs:201-210`, `240-245`, `413-430`.

**Required fix:** Align class tables exactly with §4.2 and add negative tests for agent steward tools and `steward-analysis://` resource reads.

### SEC-002 — High — Observer resource policy is incomplete

AC-12 says observers see `artifact://`, `steward-analysis://`, and every `chainworks://` collection. Current observer templates omit `artifact://{artifact_id}`, `chainworks://runs/{run_id}/stages`, and `chainworks://runs/{run_id}/artifacts`.

**Evidence:** Proposal line `640`; implementation lines `control-plane/crates/auth/src/lib.rs:247-257`.

**Required fix:** Populate the observer resource template set from the typed `ResourceTemplateId` table and assert it exactly.

### SEC-003 — High — GraphQL WebSocket auth is absent

P029 explicitly adds WebSocket subscription auth in `connection_init` and requires missing/unknown credentials to fail before subscription resolvers run. The server currently mounts `GraphQLSubscription::new(schema)` without an init hook, and subscription resolvers do not require a principal.

**Evidence:** Proposal lines `318-331`, `687-694`; implementation lines `control-plane/crates/graphql-server/src/server.rs:39-42`, `control-plane/crates/graphql-server/src/schema.rs:505-570`.

**Required fix:** Add `on_connection_init` auth, inject principal data into subscription context, return `connection_error`, close with 4401, and add the three focused WS tests from §9.

### API-001 — High — `steward.run_analysis` loses `journal_id`

`steward.run_analysis` is explicitly listed as an MCP command tool that must return `journal_id`. The current handler invokes `CommandHandler` but omits `commanded.journal_id` from the response. This breaks client-side audit traceability for the steward compute trigger.

**Evidence:** Proposal lines `639`, `714-728`; implementation lines `control-plane/crates/mcp-server/src/tools/steward.rs:49-75`.

**Required fix:** Return `journal_id` for `steward.run_analysis` and add `test_mcp_steward_run_analysis_response_includes_journal_id`.

### API-002 — Medium — MCP stdio pre-initialize error code is wrong

The proposal requires `-32002 / "server not initialized"` when the first stdio frame is not `initialize`. Current code returns `-32000 / "unauthorized: session not initialized"`. This is a protocol contract mismatch and could break MCP clients that distinguish initialization state from auth failure.

**Evidence:** Proposal lines `282-286`, `629-631`; implementation lines `control-plane/crates/mcp-server/src/server.rs:142-155`.

**Required fix:** Return `-32002` for first non-`initialize`, close stdin/session as required, and add the focused stdio test.

### SEC-004 — Medium — Principal bootstrap file mode is applied after write

P029 requires creating the principals file with owner-only permissions before write. Current code writes JSON first and calls `set_permissions(0o600)` afterward. The final file mode may be correct, but the creation semantics are not the proposal contract.

**Evidence:** Proposal lines `247-255`, `642-643`; implementation lines `control-plane/crates/auth/src/lib.rs:100-123`.

**Required fix:** Use Unix `OpenOptionsExt::mode(0o600)` on create, write through that handle, and keep the `cfg(unix)` assertion.

### READY-001 — Critical — Green gate is under-scoped and masks contract failures

The registered `proposal-029-mcp` gate runs the whole Rust workspace and passes, but it does not implement the focused §9 inventory. Missing tests include the GraphQL WS auth tests, typed capability drift tests, steward agent exclusion tests, and steward run-analysis journal-id test. The current passing suite even asserts the opposite of the proposal for `steward-analysis://` agent access.

**Evidence:** Proposal lines `647-747`; gate registration lines `scripts/test-gate.sh:1445-1451`; wrong passing test lines `control-plane/crates/auth/src/lib.rs:413-430`; same-tree gate output `==> Proposal 029-MCP control-plane gate passed`.

**Required fix:** Replace the broad-only gate with a named focused suite covering every §9 test family, or keep the workspace run as a regression layer after the focused assertions. The gate must fail while any §4.5 stale artifact remains.

### UX-001 — Not Applicable — No UI rewrite is in P029 scope

P029 explicitly says there is no UI rewrite (`docs/proposals/029-mcp-northbound-control-plane-server.md:123-128`, `749-757`). This audit therefore does not require SwiftUI or UI automation evidence for conformance. The only UX-adjacent requirement is dogfood MCP config migration, which is implemented.

## Conformance Summary

| Area | Status |
|---|---|
| Type ownership / dependency graph / typed capabilities | Missing |
| Principal table bootstrap | Partially Implemented |
| MCP HTTP auth | Implemented |
| MCP stdio auth | Partially Implemented |
| GraphQL HTTP auth / mutation checks | Partially Implemented |
| GraphQL WS auth | Missing |
| MCP tool capability filtering | Partially Implemented |
| MCP resource capability filtering | Partially Implemented |
| Command journal caller metadata | Implemented |
| Journal payload redaction | Implemented |
| MCP command `journal_id` responses | Partially Implemented |
| GraphQL mutation `journalId` responses | Implemented |
| Dogfood MCP client migration | Implemented |
| Proposal gate | Partially Implemented |

## Minimum Closure Checklist

1. Replace stale string auth with the §4.0 typed domain/auth model.
2. Fix class policies exactly: no steward access for `agent`; observer gets all required read resources; steward analysis resource is operator + observer only.
3. Implement GraphQL WS `connection_init` auth and tests.
4. Return `journal_id` from `steward.run_analysis`.
5. Fix stdio first non-`initialize` error to `-32002`.
6. Create principals file with `0600` permissions before writing.
7. Add the missing focused §9 tests and make `proposal-029-mcp` fail while any §4.5 stale artifact remains.

Until these are complete, Proposal 029 should stay **Not Ready**.
