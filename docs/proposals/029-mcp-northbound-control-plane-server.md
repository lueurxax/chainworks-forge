# Proposal 029: MCP Northbound Control-Plane Server

| Field | Value |
|---|---|
| Date | 2026-04-01 (last revised 2026-04-15, R3) |
| Status | Draft (R3 — gate slug disambiguated, current-HEAD surface corrected, audit bound to existing `command_journal`) |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 027 (server-side parity replica); Proposal 043 is the complementary read-path contract, not a blocker for northbound command work |
| Goal | Close the remaining gaps in the already-implemented MCP northbound server so that auth, caller-scoped capability exposure, and GraphQL mutation cutover have concrete owners and a deterministic proof lane. |
| Proof lane | `./scripts/test-gate.sh proposal-029-mcp` (the slug `proposal-029` is already bound to the second-wave ACP runtime profiles gate in [`scripts/test-gate.sh:1425`](../../scripts/test-gate.sh) and referenced as a prerequisite in [`docs/reference/test-gates.md:434`](../reference/test-gates.md); this proposal claims a new, non-colliding slug). |

## 1. Posture: delta on an already-implemented baseline

The Rust control plane at `control-plane/crates/mcp-server/` already runs an MCP server with a static tool registry, HTTP + stdio transports, JSON-RPC dispatch, resource URIs, and a converged command path through `engine::command_handler::CommandHandler`. The canonical surface is documented in [`docs/reference/rust-control-plane.md`](../reference/rust-control-plane.md). P029 is therefore **not** a greenfield server proposal. It is the delta that closes three explicit gaps against current HEAD:

1. The server has no principal-resolution path on either HTTP or stdio transports, and `tools/list` returns one static vector to every caller.
2. GraphQL still owns an active mutation surface for the same five commands MCP serves, with no declared coexistence or cutover rule.
3. The proposal text itself conflated the already-shipped first-wave surface with later extensions (clone, agent/session retry-reset, compare, automation, runtime-health, experiments), leaving "first wave" ambiguous.

Everything below is scoped against that baseline.

### 1.1 Why land this now (framing)

P029 is **anticipatory infrastructure**, not a response to a current incident. The app today runs on one operator's machine; the only northbound callers are (a) the local SwiftUI client speaking GraphQL and (b) development-time agents speaking MCP over stdio (Claude Code via `.mcp.json`). Both currently receive full access because the daemon binds to localhost. There is no active security breach, no observer or agent class principal in production, and no third-party automation consuming the daemon today.

What P029 does close is the prerequisite for future work:

- **P031 (thin-client cutover)** depends on having a single authoritative command surface with auth. Without P029, cutover would either keep GraphQL as primary (contradicting the thin-client direction) or expose an unauthenticated MCP surface.
- **Third-party automation** (cron-driven runs, CI triggers, external agent orchestration) cannot be reasonably enabled without capability-scoped tokens.
- **Audit completeness** is a single-table problem right now only because every command flows through one `CommandHandler`; once internal callers and different client classes exist, `caller_surface` is the only way to keep the audit table comprehensible.

Reading this proposal as "close a live bug" will overstate urgency. Reading it as "remove the architectural blocker for P031" is correct.

## 2. Current HEAD baseline (do not re-specify)

The items in this section are already implemented and must not be treated as P029 deliverables.

### 2.1 Tools — implemented today

| Namespace | Tools | Owner module |
|---|---|---|
| `ideas` | `create`, `list` | `mcp-server/src/tools/ideas.rs` |
| `runs` | `start`, `list`, `get`, `cancel` | `mcp-server/src/tools/runs.rs` |
| `approvals` | `list`, `resolve` | `mcp-server/src/tools/approvals.rs` |
| `stages` | `retry` | `mcp-server/src/tools/stages.rs` |
| `reports` | `get` | `mcp-server/src/tools/reports.rs` |

All of these converge on `engine::command_handler::CommandHandler`. The set is accepted as current HEAD truth. The implementation of any **new** tool listed in this proposal is a future slice, explicitly not part of P029's first wave.

### 2.2 Resources — implemented today

Entity URIs (verified at `mcp-server/src/server.rs`):

- `run://{id}`
- `idea://{id}`
- `artifact://{id}`
- `report://{id}`

Collection URIs (verified at `mcp-server/src/server.rs:178-202`, all registered in the `chainworks://` family):

- `chainworks://runs`
- `chainworks://ideas`
- `chainworks://approvals/inbox`
- `chainworks://runs/{run_id}/stages` — run-scoped, not a flat collection
- `chainworks://runs/{run_id}/artifacts` — run-scoped, not a flat collection

The previous P029 draft listed `workflow://{id}`, `approval://{id}`, a flat `chainworks://stages`, and a flat `chainworks://artifacts`. None of those match current HEAD. The flat-collection URIs are dropped in favor of the run-scoped pairs. `workflow://` and `approval://` are dropped from the specification until a future slice adopts them with a named reader.

### 2.3 Transports — implemented today

- HTTP (`mcp-server/src/http.rs`): Streamable HTTP transport with session correlation via `Mcp-Session-Id`. **No auth header parsing exists.**
- stdio (`mcp-server/src/server.rs`): JSON-RPC over stdin/stdout. **No caller-class seam exists.**

### 2.4 GraphQL coexistence — current reality

`control-plane/crates/graphql-server/src/schema.rs` exposes five active mutations, each forwarding to the same `CommandHandler`:

| GraphQL mutation | Schema line | CommandHandler routing |
|---|---|---|
| `startRun` | 101 | `Command::StartRun` |
| `approveStage` | 141 | `Command::ApproveStage` |
| `rejectStage` | 173 | `Command::RejectStage` |
| `retryStage` | 205 | `Command::RetryStage` |
| `cancelRun` | 225 | `Command::CancelRun` |

For every command a P029 MCP tool accepts, GraphQL has an equivalent mutation landing on the same domain owner. P029 must resolve authority ownership explicitly (§5) instead of leaving "MCP becomes canonical" to inference.

## 3. P029 scope — what this proposal actually delivers

### 3.1 In scope (first wave, P029)

1. **Principal resolution** on both transports (§4.1).
2. **Caller-scoped capability policy** gating `tools/list` and every `tools/call` (§4.2).
3. **Per-command audit journaling** (§4.3).
4. **Explicit GraphQL coexistence rule** with a named cutover plan (§5).
5. **Resource surface alignment**: drop `workflow://` and `approval://` from the specification, accept the already-implemented `chainworks://` collection family as canonical (§6).

### 3.2 Out of scope (deferred to later, named slices)

The following tools and resources are genuine product expansion, not P029. Each is listed here so the "done line" of P029 is unambiguous:

| Deferred item | Minimum pre-conditions for future adoption |
|---|---|
| `runs.clone_from_snapshot` | Stable snapshot contract; cloned-run lineage policy |
| `agents.retry` | Matched retry semantics in `CommandHandler` |
| `sessions.reset_agent` | Session reset contract stable in `session::policy` |
| `reports.compare` | Canonical diff-rendering owner (shell or daemon) |
| `automations.list` / `automations.run` | Automation domain model (currently unowned) |
| `runtime.health` | Health-signal owner; current health lives in projections |
| `experiments.list` / `experiments.start` | Experiment domain surface; today only `context-strategy-and-experiment-framework` owns this |
| `workflow://{id}` resource | Named workflow reader with a stable URI contract |
| `approval://{id}` resource | Single-approval reader; today `approvals.list` is the canonical entry |

These are not rejected — they are intentionally **not** P029 deliverables. Each must arrive in its own proposal, once its domain owner is stable.

### 3.3 Explicit non-goals (unchanged)

- No UI rewrite.
- No orchestration logic move out of `CommandHandler` and the Rust control plane.
- No replacement of southbound runtime protocols.
- No forcing high-frequency reads through MCP.

## 4. Security and capability: concrete owner chain

The prior draft stated requirements without binding them to owners. This section closes that gap by naming the module that owns each contract.

### 4.1 Principal resolution

The `Principal` type itself lives in a new shared crate: `control-plane/crates/auth/` (or equivalently a single module `control-plane/crates/auth/src/lib.rs`). Both `mcp-server` and `graphql-server` depend on this crate. Token material loading and the principal table are owned here:

```rust
// control-plane/crates/auth/src/lib.rs
pub struct Principal { pub id: String, pub class: PrincipalClass, pub capabilities: Capabilities }
pub enum PrincipalClass { Operator, Agent, Observer }
pub enum AuthError { MissingCredential, UnknownToken, MalformedHeader }

pub fn resolve_bearer(token: &str, table: &PrincipalTable) -> Result<Principal, AuthError>;
pub fn filter_tools(p: &Principal, specs: &[ToolSpec]) -> Vec<ToolSpec>;
pub fn filter_resources(p: &Principal, uris: &[ResourceUri]) -> Vec<ResourceUri>;
```

**Token material:** out of scope for P029 beyond "local file of token → principal records loaded at daemon start." Rotation and revocation are deferred with no silent-fail fallback — if `auth::load_table` fails, the daemon refuses to start.

#### 4.1.a MCP HTTP transport

**Owner:** `mcp-server/src/http.rs`
- Parse `Authorization: Bearer <token>` on every request.
- Resolve token → `Principal` via `auth::resolve_bearer`.
- Reject unauthenticated calls with JSON-RPC error `-32000 / "unauthorized"` (HTTP status 200 with JSON-RPC error body — MCP clients expect JSON-RPC semantics).
- Continue tracking `Mcp-Session-Id` for correlation; principal is re-resolved per request (tokens are authoritative, sessions are not).
- `Principal` is passed to `McpServer::handle_request` as an additional argument so the server can filter tools and build `CallerContext`.

#### 4.1.b MCP stdio transport

**Owner:** `mcp-server/src/server.rs` and `mcp-server/src/protocol.rs`

MCP stdio sessions already start with the JSON-RPC `initialize` method. P029 piggybacks on it rather than inventing a parallel handshake:

- **Wire shape:** the `initialize` method's `params.clientInfo` is extended with a `principal_token` string field (JSON-RPC allows arbitrary `params` content). Example:

  ```json
  {"jsonrpc":"2.0","id":1,"method":"initialize",
   "params":{"protocolVersion":"2025-03-26","clientInfo":{"name":"forge-cli","version":"0.1","principal_token":"<token>"}}}
  ```

- **Precedence:** the `principal_token` on `initialize` is authoritative. No second mechanism (parent-process identity, environment variable, one-shot `$/chainworks/authenticate`) is defined in P029. Parent-process identity is explicitly **deferred** — it was ambiguous in prior drafts and is dropped.

- **Failure contract:**
  1. If the first JSON-RPC message is not `initialize`, the server responds `-32002 / "server not initialized"` (matches MCP spec for pre-initialize method calls) and closes stdin.
  2. If `initialize.params.clientInfo.principal_token` is absent, the server responds `-32000 / "unauthorized: principal_token required on initialize"` and closes stdin.
  3. If the token does not resolve, the server responds `-32000 / "unauthorized: unknown token"` and closes stdin.
  4. After a successful initialize, the resolved `Principal` is bound to the stdio session for its lifetime. Subsequent `tools/call`, `tools/list`, `resources/*` methods use this principal. No mid-session re-auth is supported.

- **Protocol update:** `mcp-server/src/protocol.rs` grows a `ClientInfo` struct with an optional `principal_token` field. `JsonRpcRequest.params` for `initialize` deserializes into it.

#### 4.1.c GraphQL (new — closes R3 Finding #1)

**Owner:** `graphql-server/src/server.rs` (mount) + `graphql-server/src/schema.rs` (mutation resolvers).

GraphQL today is mounted with bare `GraphQL::new(schema.clone())` at [`server.rs:30`](../../control-plane/crates/graphql-server/src/server.rs); it has no auth seam and `MutationRoot` has no principal source. P029 adds both:

- **Axum middleware layer:** a new `graphql-server/src/auth_layer.rs` wraps the `/graphql` POST route (and the GET playground). The layer:
  - Parses `Authorization: Bearer <token>` from the HTTP request.
  - Resolves via `auth::resolve_bearer`.
  - On success, stores the `Principal` in the request's `http::Extensions`.
  - On failure, short-circuits with HTTP 401 and a GraphQL-shaped error body (same shape `async_graphql` produces so clients parse it uniformly).
  - The playground endpoint (GET) is exempt from auth only when the environment variable `CHAINWORKS_PLAYGROUND_AUTH=skip` is set at daemon startup. The daemon's existing config surface is env-based ([`daemon/src/main.rs:27`](../../control-plane/crates/daemon/src/main.rs)); P029 does not introduce a CLI flag or a new config file. Default behavior (variable absent): playground requires auth like any other request.

- **Principal injection into `async_graphql::Context`:** the `GraphQL::new(schema).with_data(move |req| ...)` builder (or an `AsyncGraphQLExtractor`) reads `Principal` from the axum request extensions and calls `req.data(principal)`. From there, every resolver reads `ctx.data::<Principal>()?` synchronously.

- **`MutationRoot` contract:** every one of the five mutations (`start_run` at [`schema.rs:101`](../../control-plane/crates/graphql-server/src/schema.rs), `approve_stage:141`, `reject_stage:173`, `retry_stage:205`, `cancel_run:225`) gains a mandatory first step:

  ```rust
  let principal = ctx.data::<Principal>()?;
  if !principal.can_invoke(CallerTool::Graphql(MutationName::StartRun)) {
      return Err("forbidden".into());
  }
  let caller = CallerContext::graphql(principal, MutationName::StartRun);
  command_handler.handle(Command::StartRun(...), caller).await?
  ```

  The `MutationName` → capability mapping mirrors the MCP capability table in §4.2: GraphQL mutations are filtered by the same class policy so `observer`-class tokens cannot invoke any mutation, `agent`-class can invoke only `startRun`, `operator`-class can invoke all five.

- **Subscription path:** WebSocket subscriptions (`/graphql/ws`) also gain the auth layer. If a subscription requires a specific class, the resolver enforces it the same way mutations do. Out of scope for P029: declaring per-subscription capability tables.

- **AC-5 implementability:** with this wiring, every GraphQL mutation invocation produces a `command_journal` row whose `caller_surface = 'graphql'` and `caller_principal_id` is set to `principal.id`. AC-5 becomes verifiable end-to-end.

### 4.2 Caller-scoped capability exposure

**Owner:** `auth::filter_tools(principal, &tool_specs) -> Vec<ToolSpec>` (signature from §4.1, lives in the shared `control-plane/crates/auth` crate — **not** in `mcp-server/src/auth.rs`). `McpServer::handle_request` calls it at two entry points:

1. Before `tools/list` returns its vector.
2. Before `tools/call` dispatches — if the tool is not in the principal's allowed set, return `-32601 / "method not found"` (not `"forbidden"`, to avoid capability probing).

**Client classes (first wave):**

The capability maps below only cite tools that exist in current HEAD (`mcp-server/src/tools/mod.rs` registers exactly `approvals`, `ideas`, `reports`, `runs`, `stages`). Deferred-tool entries from §3.2 are deliberately absent; adding them here later is gated on first landing the tool itself.

| Class | Default allowed tools |
|---|---|
| `operator` | Every tool in §2.1: `ideas.create`, `ideas.list`, `runs.start`, `runs.list`, `runs.get`, `runs.cancel`, `approvals.list`, `approvals.resolve`, `stages.retry`, `reports.get` |
| `agent` | `ideas.create`, `ideas.list`, `runs.start`, `runs.list`, `runs.get`, `reports.get` (read + limited create; no approvals, no stage retry, no cancel) |
| `observer` | `ideas.list`, `runs.list`, `runs.get`, `approvals.list`, `reports.get` (read-only — explicitly no `create` or `resolve`) |

Artifact reads happen through the `artifact://{id}` resource URI and the `chainworks://runs/{run_id}/artifacts` collection URI, not through a tool. Resource exposure is filtered by the same capability policy (§6).

These maps live in the shared `auth` crate as static tables for P029 (next to `filter_tools` / `filter_resources` from §4.1). A future slice may promote them to YAML-driven policy; that promotion is out of scope.

### 4.3 Audit journaling — extend `command_journal`, do not fork

Current HEAD already owns the canonical per-command audit trail:

- Table: `command_journal` (schema in [`db/migrations/001_initial.sql:104`](../../control-plane/crates/db/migrations/001_initial.sql)).
- Columns today: `id`, `command_type`, `payload_json`, `result_status`, `run_id`, `created_at`, `completed_at`, `error`.
- Writer: `engine::command_handler::CommandHandler` at [`command_handler.rs:63`](../../control-plane/crates/engine/src/command_handler.rs) (mints `journal_id`) and line 82 (`command_journal::record`), completes at line 99 (`complete_entry`) or fails at line 102 (`fail_entry`).

**Decision:** P029 extends `command_journal` rather than creating a parallel `mcp_audit_log`. A parallel table would split the audit trail between northbound surfaces that already converge on the same writer, would require cross-table joins to answer "who called this command," and would create a second source of truth for data the engine already owns.

**Migration:** a new SQL migration file (next available index) adds four nullable columns to `command_journal`:

```sql
ALTER TABLE command_journal ADD COLUMN caller_surface        TEXT NULL;  -- 'mcp' | 'graphql' (null for pre-P029 and future 'internal' rows)
ALTER TABLE command_journal ADD COLUMN caller_principal_id   TEXT NULL;  -- principal id from shared auth crate (§4.1)
ALTER TABLE command_journal ADD COLUMN caller_principal_class TEXT NULL; -- 'operator' | 'agent' | 'observer'
ALTER TABLE command_journal ADD COLUMN caller_tool           TEXT NULL;  -- 'runs.start' for MCP, 'startRun' for GraphQL
```

All four columns are nullable so pre-P029 rows survive unchanged. Rows written by callers that do not yet supply a `CallerContext` (none exist on current HEAD — P029 updates every call site) would also appear with null caller columns; this is the graceful-degradation path if a future internal caller lands without updating its `CommandHandler::handle` invocation.

**Owner of caller context (corrected — closes R3 Finding #3):**

The prior draft said two things that do not match current HEAD and must be replaced:

1. *"the redacted `payload_json` is what flows into `CommandHandler::execute`"* — wrong. Tools and GraphQL resolvers build *typed `Command` enum values* (not pre-serialized JSON); it is `CommandHandler::handle` itself that serializes the command to `payload_json` at [`command_handler.rs:72`](../../control-plane/crates/engine/src/command_handler.rs): `let payload_json = serde_json::to_string(&cmd).unwrap_or_default();`. Redaction therefore cannot sit upstream of `CommandHandler`.
2. *"the MCP tool handler captures `journal_id` from `CommandResult`"* — wrong. `CommandResult` today is a variant enum ([`command_handler.rs:28`](../../control-plane/crates/engine/src/command_handler.rs)) with no `journal_id` field; `journal_id` is minted at line 63 and never returned.

**Corrected contract:**

1. **`CallerContext` lives in `domain/src/commands.rs`** (new struct), serializable and safe to pass through FFI boundaries:

   ```rust
   pub struct CallerContext {
       pub surface: CallerSurface,             // Mcp | Graphql
       pub principal_id: String,               // always non-empty
       pub principal_class: PrincipalClass,
       pub caller_tool: String,                // 'runs.start' for MCP, 'startRun' for GraphQL
   }
   ```

   **No `Internal` variant is defined in P029.** Current HEAD has no internal caller of `CommandHandler::handle` — executor (`executor.rs`) drives work via the orchestrator directly, and recovery (`recovery.rs`) writes to stages/repair tables without going through `CommandHandler`. Adding an `Internal` variant and an `Internal`-typed AC would be dead code on landing. When a future slice reroutes an internal path through `CommandHandler`, that slice extends `CallerSurface` with an `Internal` variant and adds the corresponding AC.

2. **`CommandHandler::handle` signature changes** from `handle(&self, cmd: Command)` to `handle(&self, cmd: Command, caller: CallerContext)`. Call sites updated:
   - `mcp-server/src/tools/*.rs` — construct `CallerContext::mcp(principal, tool_name)`
   - `graphql-server/src/schema.rs::MutationRoot` — construct `CallerContext::graphql(principal, mutation_name)`

   **Blast radius (acknowledge explicitly).** This signature change is workspace-wide: beyond the ~9 production call sites, every integration test that constructs a `CommandHandler` and calls `.handle(cmd)` must pass a `CallerContext`. Current test coverage includes at minimum 15 call sites in `engine/tests/integration.rs` plus `daemon/tests/mcp_stdio.rs`. To keep the test migration mechanical rather than semantic, P029 ships a convenience constructor:

   ```rust
   // domain/src/commands.rs
   impl CallerContext {
       /// Test-only stand-in. Tags rows as `caller_surface = 'mcp'` with a
       /// synthetic operator principal so tests that do not exercise auth
       /// still produce well-formed command_journal rows. Not exposed in
       /// production code paths (feature-gated on `cfg(test)` or a
       /// `pub(crate)` `test_support` module, TBD by implementer).
       pub fn test() -> Self { ... }
   }
   ```

   Every existing test migrates from `handler.handle(cmd)` to `handler.handle(cmd, CallerContext::test())` — a mechanical search-and-replace, not a semantic change.

3. **Redaction lives in the engine, not the MCP server.** A new module `engine/src/command_journal_redact.rs` owns per-variant redaction keyed by the `Command` enum discriminant:

   ```rust
   // engine/src/command_journal_redact.rs
   pub fn redact_for_journal(cmd: &Command, payload_json: &str) -> String;
   ```

   `CommandHandler::handle` calls it at line 72 instead of the bare `serde_json::to_string`:

   ```rust
   let raw = serde_json::to_string(&cmd).unwrap_or_default();
   let payload_json = command_journal_redact::redact_for_journal(&cmd, &raw);
   ```

   This places redaction inside the engine (which owns the Command schema) rather than inside `mcp-server` (which only sees the typed command, not its serialization). GraphQL benefits automatically because both surfaces funnel through the same handler.

4. **`CommandResult` gains `journal_id` via a wrapper, not a variant change.** To avoid disturbing every match-arm of `CommandResult`, `CommandHandler::handle` returns `Result<Commanded>` where:

   ```rust
   pub struct Commanded {
       pub result: CommandResult,
       pub journal_id: String,
   }
   ```

   Tool handlers and GraphQL resolvers get a stable audit pointer (`commanded.journal_id`) without every `CommandResult` variant growing a redundant field. Internal callers that ignore the journal id keep reading `commanded.result`.

5. **`CommandHandler::handle` writes all four new columns** in the existing `command_journal::record` call — the `record` function signature grows to accept `caller_surface`, `caller_principal_id`, `caller_principal_class`, `caller_tool` as optional parameters. `record`'s existing SQL gets the four new bind parameters.

**What this replaces in the previous draft:**

- The `mcp_audit_log` table is gone. AC-7/AC-8 (see §8) assert rows in `command_journal` with `caller_surface = 'mcp' | 'graphql'` and non-null `caller_principal_id` / `caller_tool`.
- The phrase *"redacted `payload_json` is what flows into `CommandHandler::execute`"* is replaced by *"`CommandHandler::handle` redacts the serialized command before writing the journal row, keyed by the Command variant."*
- The phrase *"MCP tool handler captures this `journal_id` from `CommandResult`"* is replaced by *"`CommandHandler::handle` returns `Commanded { result, journal_id }`; tool and mutation code surface `journal_id` via the wire contract defined in §4.4."*

### 4.4 Client-visible `journal_id` wire contract (closes R4 Finding #1)

`Commanded.journal_id` is an internal audit pointer. This section defines exactly how it reaches the client on each surface, so AC-11 is implementable end-to-end.

#### 4.4.a MCP tools/call response

Every tool that invokes `CommandHandler::handle` (`ideas.create`, `runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`) returns an MCP `tools/call` result whose `content[0].text` is a JSON object containing the tool's existing result fields plus a top-level `journal_id` string. Example (for `runs.start`):

```json
{
  "jsonrpc": "2.0",
  "id": 42,
  "result": {
    "content": [
      {
        "type": "text",
        "text": "{\"run_id\":\"c3a7-...-4e9b\",\"journal_id\":\"93f2-...-1d7c\"}"
      }
    ],
    "isError": false
  }
}
```

- **Protocol version note:** the server currently advertises `protocolVersion: "2024-11-05"` (see `mcp-server/src/server.rs:93`). That revision does **not** define the `structuredContent` typed-output channel — it was introduced in MCP 2025-06-18. P029 therefore delivers `journal_id` only inside `content[0].text` as stringified JSON. When the server bumps `protocolVersion` in a later slice, a subsequent proposal may add a mirrored `structuredContent.journal_id` field; that bump is explicitly **not** part of P029.
- **Read-only tools** (`ideas.list`, `runs.list`, `runs.get`, `approvals.list`, `reports.get`) do **not** invoke `CommandHandler` and therefore do **not** include `journal_id`. Their response shape is unchanged from current HEAD.

Owner: each tool module in `mcp-server/src/tools/*.rs` constructs the JSON from the `Commanded` returned by `CommandHandler::handle`.

#### 4.4.b GraphQL mutation response

Each of the five mutations gets a **dedicated payload type** that wraps the existing return value plus `journalId`. P029 does **not** add `journalId` to shared entity types (`Run`, `Approval`) because those types are also returned by read queries (`run(id:)`, `runs`, `approvals`) which have no mutation journal to resolve. Putting a non-null mutation-only field on a query-visible type would either be impossible to satisfy on read paths or force query-resolver special-casing.

| Mutation | Current return | P029 payload type |
|---|---|---|
| `startRun` | `Run` (line 101) | `StartRunPayload { run: Run!, journalId: ID! }` |
| `approveStage` | `Approval` (line 141) | `ApproveStagePayload { approval: Approval!, journalId: ID! }` |
| `rejectStage` | `Approval` (line 173) | `RejectStagePayload { approval: Approval!, journalId: ID! }` |
| `retryStage` | `Boolean` (line 205) | `RetryStagePayload { retried: Boolean!, journalId: ID! }` |
| `cancelRun` | `Boolean` (line 225) | `CancelRunPayload { cancelled: Boolean!, journalId: ID! }` |

All five payload types are new GraphQL object types defined in `graphql-server/src/schema.rs`. They nest the existing return value under a named field (`run`, `approval`, `retried`, `cancelled`) so existing query fields on the inner type continue to resolve normally.

**Schema compatibility:** this is a breaking change for all five mutations — the return type changes from a direct entity/scalar to a wrapper. Existing clients that pattern-match on `startRun { id }` must update to `startRun { run { id } journalId }`. This is a one-time migration on P029 landing and is explicitly permitted because:

1. The only current GraphQL consumer is the local SwiftUI client on the same machine, which is updated in the same commit.
2. Stage A (§5) is modified to say: "No behavior change for existing GraphQL clients **except** the mutation response wrapping documented in §4.4.b, which the SwiftUI client absorbs in the same commit." The prior draft's unqualified "no behavior change" claim is retracted.

Owner: `graphql-server/src/schema.rs::MutationRoot` + the GraphQL object resolvers for `Run`, `Approval`, `CancelRunPayload`.

#### 4.4.c Internal callers

Internal callers (recovery, background executor) never surface `journal_id` to any external client. They may log it but have no wire contract. Their behavior is unchanged from current HEAD.

#### 4.4.d When `journal_id` is absent from the response

A tool call or mutation that returns an error (before `CommandHandler::handle` is invoked — e.g. capability denial, argument-validation failure) produces no `journal_id` because no `command_journal` row was written. The response shape in that case is the error variant (§4.2 for MCP, standard `async_graphql` errors for GraphQL) with no `journal_id` field.

This is the correct behavior: a client that receives a result without `journal_id` must treat that result as having no audit trail in `command_journal`.

## 5. GraphQL coexistence and cutover

The prior draft proposed a three-stage plan where Stage B refactored GraphQL mutations to call MCP tool handlers. That would have inverted the current clean dependency graph (both `graphql-server` and `mcp-server` depend only on `engine`; neither depends on the other). P029 drops that middle stage. The revised plan is two stages, and the "dual authority" concern is addressed by shared audit, not by cross-crate coupling.

### Stage A — Coexistence (P029 landing)

- Both GraphQL mutations and MCP tools remain active.
- Both converge on the same `CommandHandler` via the shared `Command` enum from `domain::commands`.
- Both supply a `CallerContext`, so both land rows in `command_journal` with `caller_surface` set. `command_journal` itself is the divergence detector — if the two surfaces ever produce different journal rows for "the same operation," the audit table will show it.
- No cross-crate dependency is introduced. `graphql-server` and `mcp-server` both continue to depend only on `engine` + the new shared `auth` crate.
- **GraphQL schema break (one-time, absorbed on landing):** all five mutation return types change from direct entities/scalars to dedicated payload wrappers per §4.4.b. The only current GraphQL consumer — the local SwiftUI client — is updated in the same commit. After that one-time migration, no further behavior change occurs in Stage A.

This is the default state when P029 lands.

### Stage B — GraphQL mutations deprecated (was "Stage C" in prior drafts)

- GraphQL schema keeps the mutation fields but marks them `@deprecated(reason: "use MCP tools")`.
- Residual GraphQL traffic is observable via `command_journal` rows where `caller_surface = 'graphql'` — the audit table is the counter; no separate telemetry is needed.
- No removal in P029. A future proposal removes the GraphQL mutation fields once residual traffic is zero.

**Canonical mutation authority** after Stage B: both surfaces are still technically authoritative and both funnel through `CommandHandler`. "MCP is canonical" becomes true only when the future removal proposal lands; until then, the two surfaces are genuine peers. P029 does not overclaim.

### Why no intermediate "GraphQL wraps MCP" stage

The proposal's prior draft proposed such a stage. It was removed because:

1. The two surfaces already converge cleanly on `CommandHandler`. "Dual command-authoring" is just two call sites constructing the same typed `Command` enum — not a divergence in semantics.
2. Making `graphql-server` depend on `mcp-server::tools::*::handle_*` would invert the dependency graph and create a new cross-crate coupling worse than the status quo.
3. `CallerContext` already distinguishes the two surfaces in the audit trail, which is the only place "dual authority" could actually leak into product behavior.

Any future "collapse both into one command-building layer" work would belong in a new proposal and would likely extract a shared `domain::commands::builders` helper crate, not route GraphQL through MCP internals.

## 6. Resource surface alignment

- Canonical single-entity URIs for P029: `run://{id}`, `idea://{id}`, `artifact://{id}`, `report://{id}` (all already implemented).
- Canonical collection URIs for P029: the `chainworks://` family listed in §2.2 (already implemented).
- Dropped from the specification until a future slice adopts them: `workflow://{id}`, `approval://{id}`.
- Resource reads must go through the same capability filter as tool calls (`auth::filter_resources`, shared crate from §4.1). `resources/list` returns a per-principal vector.

## 7. Migration strategy

P029 lands in a single step. There is no multi-phase rollout of the *proposal itself* — auth and capability enforcement must land together, because partial deployment would be worse than the current state (auth without filtering would be security theatre; filtering without auth would be arbitrary).

After P029 lands, GraphQL mutation cutover follows the three-stage rule in §5. Each stage has its own proposal or gate update.

**Rollback:** if the `proposal-029-mcp` gate is red after merge, revert the auth wiring, the filter wiring, the `CommandHandler` signature change, and the migration that added the four nullable columns. The `command_journal` rows already written with populated `caller_*` columns remain valid on a pre-P029 schema only if the added columns are preserved; simplest recovery is to keep the migration in place but ignore the new columns.

## 8. Acceptance criteria

P029 is complete when all of the following hold on the same tree:

1. **MCP HTTP** rejects requests without a resolvable `Authorization: Bearer` header with JSON-RPC error `-32000`.
2. **MCP stdio** rejects first frames that are not `initialize` with `-32002`, and rejects `initialize` without a resolvable `principal_token` with `-32000`. Post-initialize, the resolved principal is bound to the session.
3. **MCP stdio** rejects a second `initialize` received mid-session and does not rebind the principal (session-lifetime immutability invariant).
4. **GraphQL** rejects requests without a resolvable `Authorization: Bearer` header with HTTP 401 and a GraphQL-shaped error body. The `/graphql` playground GET is exempt only when `CHAINWORKS_PLAYGROUND_AUTH=skip` is set (env-based, matching the daemon's existing config surface).
5. `tools/list` returns a per-principal vector whose contents match the class table in §4.2 exactly.
6. `tools/call` for a tool outside the principal's allowed set returns `-32601`.
7. A GraphQL mutation invoked by a principal whose class is not allowed for that mutation returns a GraphQL error of kind `forbidden` and produces no `command_journal` row.
8. Every `tools/call` that invokes `CommandHandler` writes one row to `command_journal` with `caller_surface = 'mcp'`, a non-null `caller_principal_id`, a matching `caller_principal_class`, and `caller_tool` set to the MCP tool name (e.g. `runs.start`).
9. Every GraphQL mutation that invokes `CommandHandler` writes one row to `command_journal` with `caller_surface = 'graphql'`, a non-null `caller_principal_id`, and `caller_tool` set to the mutation name (e.g. `startRun`).
10. `command_journal.payload_json` goes through `engine::command_journal_redact::redact_for_journal` before insert; at least one sensitive field per Command variant is asserted absent in a focused test.
11. `CommandHandler::handle` returns `Commanded { result, journal_id }`. MCP `tools/call` responses for mutating tools include `journal_id` inside `content[0].text` stringified JSON per §4.4.a (no `structuredContent` until the server bumps `protocolVersion`), and every GraphQL mutation return type exposes a `journalId: ID!` field per §4.4.b. Error paths that never reach `CommandHandler` produce no `journal_id` (per §4.4.d).
12. `resources/list` is capability-filtered the same way as `tools/list`.
13. The `proposal-029-mcp` gate (see §9) is green on the same tree.

Deferred items from §3.2 are **not** acceptance criteria. Adding them without a new proposal is a scope violation.

## 9. Test gate

### `proposal-029-mcp`

The slug `proposal-029` is already claimed by the second-wave ACP runtime profiles gate in [`scripts/test-gate.sh:1425`](../../scripts/test-gate.sh). This proposal uses `proposal-029-mcp` to avoid the collision. Naming precedent: [`proposal-027r`](../reference/test-gates.md) for the P027 renderer slice.

Scope:

- auth rejection on HTTP and stdio transports
- per-principal `tools/list` filtering for all three client classes
- per-principal `tools/call` enforcement (allowed and denied paths)
- audit row shape for MCP tools and GraphQL mutations
- no regression on the already-implemented tool/resource set
- Stage A coexistence: MCP tool + matching GraphQL mutation produce the same run outcome

Command:

```bash
./scripts/test-gate.sh proposal-029-mcp
```

Proposed focused test inventory (new). Test names refer to rows in `command_journal`, not a separate `mcp_audit_log`:

```
# Transport auth — MCP HTTP
test_mcp_http_rejects_missing_authorization_header
test_mcp_http_rejects_unknown_bearer_token

# Transport auth — MCP stdio (initialize.params.clientInfo.principal_token)
test_mcp_stdio_rejects_first_frame_other_than_initialize
test_mcp_stdio_rejects_initialize_without_principal_token
test_mcp_stdio_rejects_initialize_with_unknown_principal_token
test_mcp_stdio_binds_principal_for_session_lifetime
test_mcp_stdio_rejects_reinitialize_mid_session       # P029 R5 — session-lifetime immutability

# Transport auth — GraphQL
test_graphql_rejects_missing_authorization_header
test_graphql_rejects_unknown_bearer_token
test_graphql_mutation_reads_principal_from_context
test_graphql_observer_class_cannot_invoke_start_run

# Capability policy
test_mcp_tools_list_filtered_for_operator
test_mcp_tools_list_filtered_for_agent
test_mcp_tools_list_filtered_for_observer
test_mcp_tools_call_denied_returns_method_not_found
test_mcp_resources_list_is_capability_filtered

# Audit contract — against command_journal
test_command_journal_row_has_caller_mcp_for_runs_start    # covers one MCP tool...
test_command_journal_row_has_caller_mcp_for_approvals_resolve   # ...and a second to catch tool-site wiring errors
test_command_journal_row_has_caller_graphql_for_start_run
test_command_journal_row_has_caller_graphql_for_approve_stage
test_command_journal_caller_columns_nullable_for_pre_p029_rows
test_command_journal_payload_redacted_for_sensitive_fields

# journal_id surfacing (§4.4)
test_mcp_tools_call_response_includes_journal_id_in_content_text
test_mcp_read_only_tool_response_omits_journal_id
test_graphql_start_run_returns_start_run_payload_with_run_and_journal_id
test_graphql_approve_stage_returns_payload_with_approval_and_journal_id
test_graphql_retry_stage_returns_payload_with_retried_and_journal_id
test_graphql_cancel_run_returns_payload_with_cancelled_and_journal_id
test_response_omits_journal_id_when_capability_check_fails

# Cross-surface parity (Stage A coexistence)
test_graphql_and_mcp_produce_identical_run_for_start_run
```

**Tests explicitly NOT in this inventory** (with rationale):

- `test_command_journal_row_has_caller_internal_for_recovery` — dropped. No internal caller of `CommandHandler::handle` exists on current HEAD; see §4.3. Adding it is gated on a future slice that reroutes executor or recovery through `CommandHandler`.
- `test_mcp_tools_call_response_includes_journal_id_in_structured_content` — dropped. The server advertises `protocolVersion: "2024-11-05"`; `structuredContent` is only defined from MCP 2025-06-18. A later slice that bumps the protocol version adds this test along with the wire-shape change.

Gate runner: run the full Rust workspace tests, same pattern as `proposal-027` and `proposal-044`.

**Registration:** `scripts/test-gate.sh` must add a `PROPOSAL_029_MCP_TESTS` array and a `proposal-029-mcp|p029-mcp)` case block. The existing `proposal-029|p029` case (second-wave ACP runtime) stays unchanged.

## 10. Non-goals (unchanged)

P029 does not:
- rewrite the UI,
- move business logic into MCP,
- replace southbound runtime protocols,
- force every high-frequency read through MCP,
- define token rotation, revocation, or delegation policy,
- deliver any tool or resource listed in §3.2.

## 11. Risks

### 11.1 Auth-as-theatre
Risk: auth lands without capability filtering, or filtering without auth.
Mitigation: §7 forbids partial rollout; both land together in one slice.

### 11.2 Capability drift
Risk: static class tables in the shared `auth` crate become stale as tools are added.
Mitigation: adding a new tool to `tool_specs` without touching the capability table is a compile-time error (enforced via an exhaustive match in `auth::filter_tools`).

### 11.3 Dual authority during Stage A
Risk: GraphQL and MCP could diverge in behavior because they construct `Command` values at separate call sites.
Mitigation: both call sites build values of the same typed `Command` enum from `domain::commands`, so the shape of the command is compile-time identical. Semantic divergence — same `Command` variant, different observable run outcome — is detectable post-hoc by comparing `command_journal` rows with `caller_surface = 'mcp'` versus `caller_surface = 'graphql'` for the same logical operation. The cross-surface parity test `test_graphql_and_mcp_produce_identical_run_for_start_run` (§9) is the first-wave canary for this class of regression. Stage B (deprecation) does not eliminate dual authority — that only happens in a future removal proposal once residual GraphQL traffic is zero.

### 11.4 Audit privacy
Risk: serialized `Command` JSON contains sensitive data (tokens, secrets, operator comments).
Mitigation: `engine::command_journal_redact::redact_for_journal(&cmd, &raw) -> String` (owner defined in §4.3) applies per-variant redaction inside `CommandHandler::handle` before the `command_journal::record` insert. This lives in the engine because the engine owns the `Command` enum variants; `mcp-server` and `graphql-server` never see un-redacted payloads because they never serialize — they pass typed `Command` values. Redaction rules are audited by the test gate: `test_command_journal_payload_redacted_for_sensitive_fields` asserts at least one sensitive field per Command variant is absent from `command_journal.payload_json`.

## 12. Relationship to per-agent MCP policy

P029 does not replace per-agent runtime MCP policy from Proposal 025:

- Proposal 025 controls what **southbound** agent sessions may invoke during execution.
- Proposal 029 controls what **northbound** clients may command through the control plane.

These are different layers and must remain separate. Sharing code between them is an anti-pattern.

## 13. Final recommendation

P029 should land as a narrow, deterministic delta against current HEAD: add auth, capability filtering, and audit journaling; declare GraphQL coexistence explicitly; drop unowned resource URIs from the specification. Every deferred item listed in §3.2 stays deferred until its own proposal. That discipline is the point: "MCP is canonical" becomes a statement the proof lane can assert, not a direction of travel.
