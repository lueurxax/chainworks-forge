# Proposal 029: MCP Northbound Control-Plane Server

| Field | Value |
|---|---|
| Date | 2026-04-01 (last revised 2026-04-16, R8) |
| Status | Draft (R8 — absorbed the active P049 Steward northbound surface: §2.1 now lists `steward.run_analysis` (command tool), `steward.list_analyses`, and `steward.get_analysis` (direct tools); §2.2 lists `steward-analysis://{analysis_id}`; §4.0 extends `CapabilityToolId` / `ResourceTemplateId` with the four new variants; §4.2 pins class policy (`run_analysis` = operator-only; `list_analyses` + `get_analysis` + `steward-analysis://` = operator + observer; agent excluded); §4.4.a / §8 AC-11 / AC-12 / AC-13 / §9 all extend accordingly. Also adds §4.5 implementation-handoff note so the stale pre-R6 auth scaffold on the current tree (`auth::PrincipalClass`, `auth::ToolSpec`, string `CallerContext.principal_class`, missing `domain/src/capabilities.rs`, missing server-side converters) is replaced in one slice, not patched incrementally. R7 preserved: §3.4 deferred-scope ownership map; SwiftUI / P031 phrasing is MCP-not-GraphQL. R6 preserved: type ownership, Stage A narrowed to MCP command tools, GraphQL WS unknown-token test, bootstrap token handling) |
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

P029 is **anticipatory infrastructure**, not a response to a current incident. The app today runs on one operator's machine; the only northbound callers are (a) development-time agents speaking MCP over stdio / HTTP (Claude Code via `.mcp.json`) and (b) in-repo GraphQL integration tests plus the dev playground. The SwiftUI client is still app-local and does not yet speak either surface to the daemon — thin-client adoption is **P031** (MCP commands) + **P043** (read projections) scope, mapped in §3.4. Current callers receive full access because the daemon binds to localhost. There is no active security breach, no observer or agent class principal in production, and no third-party automation consuming the daemon today.

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
| `steward` | `run_analysis`, `list_analyses`, `get_analysis` | `mcp-server/src/tools/steward.rs` (added by Proposal 049) |

**Command vs. direct tools.** Not all tools converge on `engine::command_handler::CommandHandler`. The tool set divides into two categories:

- **Command tools** (`runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, `steward.run_analysis`): these build a typed `Command` enum value and call `CommandHandler::handle`. They produce `command_journal` rows and return `journal_id`. `steward.run_analysis` constructs `Command::RunStewardAnalysis(RunStewardAnalysisCmd { … })` and returns `CommandResult::StewardAnalysisQueued`; it is a full command tool on par with `runs.start`.
- **Direct tools** (`ideas.create`, `ideas.list`, `runs.list`, `runs.get`, `approvals.list`, `reports.get`, `steward.list_analyses`, `steward.get_analysis`): these call repo functions directly (e.g. `ideas::insert`, `runs::find_by_id`, `steward_repo::list_analyses`, `steward_repo::find_analysis`) without going through `CommandHandler`. They do **not** produce `command_journal` rows and do **not** return `journal_id`. `ideas.create` is a mutating direct tool — there is no `CreateIdea` variant in `domain::commands`. `steward.list_analyses` and `steward.get_analysis` are read-only direct tools. Moving direct tools onto `CommandHandler` (and adding `CreateIdea` + siblings) is deferred to the "MCP command-path consolidation" future proposal — see §3.4.

Both categories are subject to auth and capability filtering (§4). The distinction matters only for audit journaling (§4.3) and `journal_id` surfacing (§4.4). The implementation of any **new** tool listed in §3.2 is deferred to the owner proposal named in that table (see §3.4), explicitly not part of P029's first wave.

### 2.2 Resources — implemented today

Entity URIs (verified at `mcp-server/src/server.rs`):

- `run://{id}`
- `idea://{id}`
- `artifact://{id}`
- `report://{id}`
- `steward-analysis://{analysis_id}` — registered at [`mcp-server/src/server.rs:276`](../../control-plane/crates/mcp-server/src/server.rs) by Proposal 049; read path at [`server.rs:460`](../../control-plane/crates/mcp-server/src/server.rs) loads from the `steward` repo (`find_analysis`, `list_run_links`, `list_recommendations`).

Collection URIs (verified at `mcp-server/src/server.rs:178-202`, all registered in the `chainworks://` family):

- `chainworks://runs`
- `chainworks://ideas`
- `chainworks://approvals/inbox`
- `chainworks://runs/{run_id}/stages` — run-scoped, not a flat collection
- `chainworks://runs/{run_id}/artifacts` — run-scoped, not a flat collection

The previous P029 draft listed `workflow://{id}`, `approval://{id}`, a flat `chainworks://stages`, and a flat `chainworks://artifacts`. None of those match current HEAD. The flat-collection URIs are dropped in favor of the run-scoped pairs. `workflow://` and `approval://` are dropped from the specification; their future adoption is owned by the "named workflow/approval resource readers" future proposal (§3.4).

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

The following tools and resources are genuine product expansion, not P029. Each is listed here so the "done line" of P029 is unambiguous. The **Target owner** column names the proposal that will deliver the item; the full ownership map for every narrowing in this proposal (not just tool/resource expansion) is §3.4.

| Deferred item | Target owner | Minimum pre-conditions for future adoption |
|---|---|---|
| `runs.clone_from_snapshot` | Future proposal — "MCP runs second-wave tools" (not yet drafted) | Stable snapshot contract; cloned-run lineage policy |
| `agents.retry` | Future proposal — "MCP agents/sessions tools" (not yet drafted) | Matched retry semantics in `CommandHandler` |
| `sessions.reset_agent` | Future proposal — "MCP agents/sessions tools" (not yet drafted) | Session reset contract stable in `session::policy` |
| `reports.compare` | Future proposal — "MCP reports second-wave tools" (not yet drafted) | Canonical diff-rendering owner (shell or daemon) |
| `automations.list` / `automations.run` | Future proposal — "MCP automations surface" (not yet drafted) | Automation domain model (currently unowned) |
| `runtime.health` | Future proposal — "MCP runtime-health surface" (not yet drafted) | Health-signal owner; current health lives in projections |
| `experiments.list` / `experiments.start` | Future proposal — "MCP experiments surface" (not yet drafted) | Experiment domain surface; today only `context-strategy-and-experiment-framework` owns this |
| `workflow://{id}` resource | Future proposal — "named workflow/approval resource readers" (not yet drafted) | Named workflow reader with a stable URI contract |
| `approval://{id}` resource | Future proposal — "named workflow/approval resource readers" (not yet drafted) | Single-approval reader; today `approvals.list` is the canonical entry |

These are not rejected — they are intentionally **not** P029 deliverables. Each must arrive in its own proposal, once its domain owner is stable. "Future proposal (not yet drafted)" means P029 does **not** reserve a proposal number; the number is allocated when the proposal is actually drafted.

### 3.3 Explicit non-goals (unchanged)

- No UI rewrite.
- No orchestration logic move out of `CommandHandler` and the Rust control plane.
- No replacement of southbound runtime protocols.
- No forcing high-frequency reads through MCP.

### 3.4 Deferred scope ownership (complete map)

Every narrowing in this proposal — not just the tool/resource expansions listed in §3.2 — has a named owner here. When the body says "future slice" or "future proposal" without a number, this table is the authoritative source for what that means. Entries labeled "Future proposal — '<slug>' (not yet drafted)" do not reserve a proposal number; numbers are allocated when the proposal is actually written.

| Narrowing in P029 | Cited at | Target owner | Trigger for that owner |
|---|---|---|---|
| MCP tool / resource expansions listed in §3.2 | §3.2, §6 | See §3.2 table's Target owner column | Pre-condition in §3.2 holds |
| Move MCP direct tools onto `CommandHandler` (adds `CreateIdea` + siblings to `domain::commands`; makes direct tools emit `command_journal` rows and `journal_id`) | §2.1, §4.4, §5 | Future proposal — "MCP command-path consolidation" (not yet drafted) | Direct-tool audit gap becomes observable (e.g. `ideas.create` is driven by third-party automation and needs journaling) |
| Test `test_command_journal_row_has_caller_internal_for_recovery` (internal `CommandHandler` caller path) | §9 | Subset of "MCP command-path consolidation" (same future proposal as above) | Executor or recovery reroutes through `CommandHandler::handle` with `CallerSurface::Internal` |
| YAML-driven capability policy (replaces the static `PrincipalClass → BTreeSet<CapabilityToolId>` tables in §4.2) | §4.0, §4.2 | Future proposal — "northbound capability policy hardening" (not yet drafted) | First operational need for a per-principal override beyond class-level defaults |
| Per-subscription capability table for GraphQL WS (first-wave default: all authenticated principals can subscribe to all subscriptions) | §4.1.c | Subset of "northbound capability policy hardening" (same future proposal as above) | First subscription stream that must be class-restricted (e.g. observer cannot see command-level events) |
| GraphQL mutation **removal** (Stage B only deprecates; removal is a separate landing) | §5, §10 | Future proposal — "GraphQL mutation retirement" (not yet drafted) | Residual `command_journal` rows with `caller_surface = 'graphql'` stay at zero for an agreed observation window after P031 lands |
| Token rotation, revocation, delegation | §4.1, §10 | Future proposal — "northbound auth lifecycle" (not yet drafted) | First multi-principal deployment, first third-party automation consumer, or first leaked-token incident |
| MCP protocol bump to 2025-06-18 and `structuredContent` on tool responses (incl. deferred test `test_mcp_tools_call_response_includes_journal_id_in_structured_content`) | §9, §10 | Future proposal — "MCP protocol version uplift" (not yet drafted) | A consumer requests `structuredContent` or other 2025-06-18 deltas |
| SwiftUI thin-client adoption of the daemon surface (MCP commands + GraphQL read projections) | §1.1, §4.4.b, §5 | **Proposal 031** (drafted) — commands via MCP; **Proposal 043** (drafted) — read projections contract | P027 parity validation, then P031 / P043 entry conditions. Note: P031 uses MCP for **commands**; SwiftUI never becomes a GraphQL-mutation consumer, so the §4.4.b schema break has no future SwiftUI migration either. |
| Southbound per-agent MCP policy | §10, §12 | **Proposal 025** (pre-existing) | Out of northbound scope entirely; listed here only to make the routing explicit |
| UI rewrite, move of orchestration logic out of `CommandHandler`, replacement of southbound runtime protocols, high-frequency reads through MCP | §3.3, §10 | Respectively: **Proposal 031** (UI rewrite), **Proposal 027** (`CommandHandler` owner — already-implemented baseline), southbound-track proposals (out of northbound scope), **Proposal 043** (read projections) | Covered by each target proposal's own entry criteria |

With this table, no narrowing in P029 is orphaned: every "future slice / future proposal" phrase in the body resolves to either an existing drafted proposal, an explicitly not-yet-drafted successor with a slug and trigger, or an explicit out-of-northbound-scope marker.

## 4. Security and capability: concrete owner chain

The prior draft stated requirements without binding them to owners. This section closes that gap by naming the module that owns each contract.

### 4.0 Type ownership and crate dependency direction

P029 introduces a new `auth` crate. This subsection fixes the owner of every shared type referenced in §§4.1–4.4 and the crate arrow on which each dependency rides. No crate listed below may acquire a reverse edge on landing.

**Dependency graph (after P029):**

```text
domain  ←  auth  ←  mcp-server
               ←  graphql-server
               ←  engine
```

- `domain` gains no new workspace-crate dependencies. It remains the transport-neutral type root.
- `auth` depends only on `domain`. It must not depend on `mcp-server`, `graphql-server`, or `engine`.
- `mcp-server`, `graphql-server`, and `engine` each gain a dependency on `auth` (in addition to their existing dependency on `domain`). None of them depends on another server crate — the clean `{mcp-server, graphql-server} → engine → db → domain` shape documented in `docs/reference/rust-control-plane.md` is preserved; only the `→ auth → domain` edge is new.

**Type owners:**

| Type | Owner crate | Rationale |
|---|---|---|
| `PrincipalClass` enum (`Operator`, `Agent`, `Observer`) | `domain` | Referenced by `CallerContext`, which lives in `domain/src/commands.rs`. Keeping both in `domain` prevents a `domain → auth` back-edge. |
| `CallerSurface` enum (`Mcp`, `Graphql`) | `domain` | Part of `CallerContext`; transport-neutral. |
| `CallerContext` struct | `domain` | Already scheduled for `domain/src/commands.rs` in §4.3. |
| `CapabilityToolId` enum | `domain` | Transport-neutral tool identity shared by `auth::filter_tools`, `mcp-server`, and `graphql-server`. One variant per currently-registered tool: `IdeasCreate`, `IdeasList`, `RunsStart`, `RunsList`, `RunsGet`, `RunsCancel`, `ApprovalsList`, `ApprovalsResolve`, `StagesRetry`, `ReportsGet`, `StewardRunAnalysis`, `StewardListAnalyses`, `StewardGetAnalysis`. |
| `ResourceTemplateId` enum | `domain` | Transport-neutral resource identity for `resources/list` and `resources/read`. One variant per template registered in §2.2: `RunEntity`, `IdeaEntity`, `ArtifactEntity`, `ReportEntity`, `StewardAnalysisEntity`, `ChainworksRuns`, `ChainworksIdeas`, `ChainworksApprovalsInbox`, `ChainworksRunStages`, `ChainworksRunArtifacts`. |
| `Principal`, `PrincipalTable`, `Capabilities`, `AuthError` | `auth` | Auth-layer composite + errors. `Principal` pairs an id + `PrincipalClass` with `BTreeSet<CapabilityToolId>` and `BTreeSet<ResourceTemplateId>`. |
| `resolve_bearer`, `filter_tools`, `filter_resources`, `match_resource_uri` | `auth` | Policy functions. All consume typed IDs from `domain`; none consume server-crate types. |
| `McpTool` struct (unchanged) | `mcp-server` | Transport descriptor stays where it already lives (`mcp-server/src/protocol.rs`); `mcp-server` owns the `McpTool.name` ⇄ `CapabilityToolId` converter. |
| `MutationName` enum (new, small) | `graphql-server` | `graphql-server` owns the GraphQL-mutation-field ⇄ `CapabilityToolId` converter. |

**Canonical `domain` additions:**

```rust
// control-plane/crates/domain/src/commands.rs (additions)
pub enum PrincipalClass { Operator, Agent, Observer }
pub enum CallerSurface  { Mcp, Graphql }
pub struct CallerContext { /* fields as defined in §4.3 */ }

// control-plane/crates/domain/src/capabilities.rs (new module)
#[non_exhaustive]
pub enum CapabilityToolId {
    IdeasCreate, IdeasList,
    RunsStart, RunsList, RunsGet, RunsCancel,
    ApprovalsList, ApprovalsResolve,
    StagesRetry,
    ReportsGet,
    // Steward northbound surface (landed by P049; absorbed into P029 inventory)
    StewardRunAnalysis, StewardListAnalyses, StewardGetAnalysis,
}

#[non_exhaustive]
pub enum ResourceTemplateId {
    RunEntity, IdeaEntity, ArtifactEntity, ReportEntity,
    StewardAnalysisEntity,
    ChainworksRuns, ChainworksIdeas, ChainworksApprovalsInbox,
    ChainworksRunStages, ChainworksRunArtifacts,
}
```

**Canonical `auth` API (replaces the earlier snippet in §4.1 that referenced undefined `ToolSpec` / `ResourceUri`):**

```rust
// control-plane/crates/auth/src/lib.rs
use domain::{PrincipalClass, CapabilityToolId, ResourceTemplateId};

pub struct Principal {
    pub id: String,
    pub class: PrincipalClass,
    pub tool_capabilities: std::collections::BTreeSet<CapabilityToolId>,
    pub resource_capabilities: std::collections::BTreeSet<ResourceTemplateId>,
}
pub enum AuthError { MissingCredential, UnknownToken, MalformedHeader }

pub fn resolve_bearer(token: &str, table: &PrincipalTable) -> Result<Principal, AuthError>;
pub fn filter_tools(p: &Principal, ids: &[CapabilityToolId]) -> Vec<CapabilityToolId>;
pub fn filter_resources(p: &Principal, ids: &[ResourceTemplateId]) -> Vec<ResourceTemplateId>;
pub fn match_resource_uri(p: &Principal, concrete_uri: &str) -> Option<ResourceTemplateId>;
```

**Server-side converters (no auth-crate knowledge of transport types):**

- `mcp-server/src/tools/mod.rs` exposes `fn capability_id_for(tool_name: &str) -> Option<CapabilityToolId>` and the inverse `fn mcp_tool_for(id: CapabilityToolId) -> &'static McpTool`. The converter covers every tool registered in §2.1, explicitly including the steward triple: `steward.run_analysis → StewardRunAnalysis`, `steward.list_analyses → StewardListAnalyses`, `steward.get_analysis → StewardGetAnalysis`. `McpServer::handle_request` runs `auth::filter_tools` over the full `CapabilityToolId` list, then materializes the corresponding `McpTool` vector for `tools/list`. `tools/call` looks up the `CapabilityToolId` for the incoming tool name and checks membership in the principal's set before dispatch.
- `mcp-server` also owns the concrete-URI → `ResourceTemplateId` parser used at the `resources/read` entry point. The parser recognizes every template registered in §2.2, including the `steward-analysis://{analysis_id}` template → `StewardAnalysisEntity`. It is passed into `auth::match_resource_uri` via a closure or a small trait object constructed at server boot, so `auth` never imports URI-shape knowledge from `mcp-server`.
- `graphql-server/src/schema.rs` exposes `fn capability_id_for(mutation: MutationName) -> CapabilityToolId` covering the five mutations (`startRun → RunsStart`, `approveStage → ApprovalsResolve`, `rejectStage → ApprovalsResolve`, `retryStage → StagesRetry`, `cancelRun → RunsCancel`). Resolvers call `auth::filter_tools` with this ID before invoking `CommandHandler`. (`approveStage` and `rejectStage` share `ApprovalsResolve` because they differ only by `ApprovalDecision`; capability policy does not distinguish them.)

**Capability drift mitigation (replaces the looser claim in §11.2):**

`CapabilityToolId` and `ResourceTemplateId` are closed enums (marked `#[non_exhaustive]` only for downstream-crate graceful handling, not for internal evasion of exhaustiveness). `auth::filter_tools` pattern-matches exhaustively over `CapabilityToolId` to consult the class → allowed-capabilities static table. Adding a new tool requires, in order: (a) add a `CapabilityToolId` variant in `domain`, (b) update the exhaustive match in `auth`, (c) update `mcp-server::capability_id_for`. Steps (b) and (c) refuse to compile until they cover the new variant, so no MCP or GraphQL tool can ship without a compile-time reminder to update capability policy.

### 4.1 Principal resolution

The `Principal` type itself lives in a new shared crate: `control-plane/crates/auth/` (or equivalently a single module `control-plane/crates/auth/src/lib.rs`). Both `mcp-server` and `graphql-server` depend on this crate; `auth` itself depends only on `domain`. Token material loading and the principal table are owned here. The authoritative `auth` API surface and type ownership is defined in §4.0; the signatures listed below are the same types from that section (no `ToolSpec` / `ResourceUri` placeholders — `auth` consumes `CapabilityToolId` and `ResourceTemplateId` from `domain`).

**Token material loading:**

The daemon's existing config surface is env-based ([`daemon/src/main.rs:27`](../../control-plane/crates/daemon/src/main.rs)). P029 follows the same convention:

- **Env var:** `CHAINWORKS_AUTH_PRINCIPALS_PATH` → absolute path to a JSON file containing the principal table. Default: `~/.chainworks/auth/principals.json`.
- **File shape:** `{ "principals": [ { "token": "<uuid>", "id": "<human-label>", "class": "operator" | "agent" | "observer" } ] }`.
- **First-start bootstrap:** if the file does not exist, `auth::load_table` creates one with a single `operator`-class entry (random UUID token) and writes it to disk with **owner-only permissions** (Unix mode `0600`, set via `std::fs::OpenOptions::mode(0o600)` before the write; on platforms without Unix file modes, the equivalent ACL is applied or the writer refuses to continue). The token value is emitted at `info` level **exactly once** — during the bootstrap that created the file. Subsequent daemon starts log only the principals-file path, not the token, so rotated log volumes do not retain a reusable bearer credential. If the operator loses the token, they read the principals file directly (`cat ~/.chainworks/auth/principals.json`) or delete the file to re-bootstrap. This is the only auto-generation; subsequent principals must be added manually.
- **Failure contract:** if the file exists but is unparseable or contains zero principals, the daemon refuses to start with a clear error message. If the env var is set to an explicitly empty string, the daemon refuses to start — there is no silent auth-disabled mode. This preserves the fail-closed contract in AC-1 through AC-4.
- **Development convenience:** to run the daemon without auth during local development, the operator uses the auto-bootstrapped principals file (§7.1) and sets `CHAINWORKS_MCP_TOKEN` in their shell. This is one extra env var, not a mode switch. The proposal does not define a "skip auth entirely" bypass because it conflicts with the fail-closed ACs and would require every AC to carry an exemption clause.
- **Rotation and revocation:** out of scope for P029; owned by the "northbound auth lifecycle" future proposal (§3.4). In P029 the principals file is read once at startup; changes require a daemon restart.

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

#### 4.1.c GraphQL

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

- **Subscription path (`/graphql/ws`) — wire contract:**

  WebSocket subscriptions must authenticate. The `/graphql/ws` subscription service is mounted **outside** the auth middleware layer (which only wraps POST `/graphql`). Instead, auth is enforced inside the `on_connection_init` handler after the WebSocket upgrade succeeds. This split is necessary because the axum auth middleware cannot reject a WebSocket upgrade and still allow the `connection_init` handshake to fire — if the middleware rejects the upgrade, the WebSocket never opens and no `connection_init` is possible.

  1. **Credential source:** the `connection_init` message must include `{ "Authorization": "Bearer <token>" }` in its payload. This is the **only** credential source for WebSocket subscriptions — HTTP upgrade headers are not inspected for auth on the `/graphql/ws` route.
  2. **Verification timing:** `on_connection_init` fires after the WebSocket opens and before any subscription resolvers run. `auth_layer.rs::resolve_bearer` is called on the extracted token. On success, the resolved `Principal` is injected into the subscription's `async_graphql::Data`.
  3. **Error / close behavior:** if `connection_init` is absent, has no `Authorization` field, or the token is unresolvable, the handler returns a `connection_error` with `{ "message": "unauthorized" }`. The server then sends a WebSocket close frame with status 4401 (application-defined). No subscription resolver fires.
  4. **Per-subscription capability:** if the resolved principal's class does not permit a subscription's data (e.g. `observer` cannot subscribe to mutation-level events), the resolver returns an `async_graphql` error stream. Declaring the per-subscription capability table is owned by the "northbound capability policy hardening" future proposal (§3.4). First wave in P029: all authenticated principals can subscribe to all subscriptions.

  **Tests (added to §9 inventory):**
  - `test_graphql_ws_rejects_missing_connection_init_auth`
  - `test_graphql_ws_rejects_unknown_connection_init_token`
  - `test_graphql_ws_accepts_valid_connection_init_token`

- **AC-9 implementability:** with this wiring, every GraphQL mutation invocation produces a `command_journal` row whose `caller_surface = 'graphql'` and `caller_principal_id` is set to `principal.id`. AC-9 becomes verifiable end-to-end.

### 4.2 Caller-scoped capability exposure

**Owner:** `auth::filter_tools(&Principal, &[CapabilityToolId]) -> Vec<CapabilityToolId>` (signature from §4.0; lives in the shared `control-plane/crates/auth` crate — **not** in `mcp-server/src/auth.rs`). `McpServer::handle_request` calls it at two entry points, using `mcp-server::tools::capability_id_for` / `mcp_tool_for` to cross the `CapabilityToolId` ⇄ `McpTool` boundary:

1. Before `tools/list` returns its vector (filter the full `CapabilityToolId` list, then render `McpTool` for each allowed ID).
2. Before `tools/call` dispatches — if the dispatched tool's `CapabilityToolId` is not in the principal's allowed set, return `-32601 / "method not found"` (not `"forbidden"`, to avoid capability probing).

**Client classes (first wave):**

The capability maps below only cite tools that exist in current HEAD (`mcp-server/src/tools/mod.rs` registers exactly `approvals`, `ideas`, `reports`, `runs`, `stages`, `steward`). Deferred-tool entries from §3.2 are deliberately absent; adding them here later is gated on first landing the tool itself.

| Class | Default allowed tools |
|---|---|
| `operator` | Every tool in §2.1: `ideas.create`, `ideas.list`, `runs.start`, `runs.list`, `runs.get`, `runs.cancel`, `approvals.list`, `approvals.resolve`, `stages.retry`, `reports.get`, `steward.run_analysis`, `steward.list_analyses`, `steward.get_analysis` |
| `agent` | `ideas.create`, `ideas.list`, `runs.start`, `runs.list`, `runs.get`, `reports.get` (read + limited create; no approvals, no stage retry, no cancel, no steward access — agents are scoped to individual run execution and have no legitimate reason to trigger cross-run cohort analysis or read Steward recommendations) |
| `observer` | `ideas.list`, `runs.list`, `runs.get`, `approvals.list`, `reports.get`, `steward.list_analyses`, `steward.get_analysis` (read-only, including Steward analysis readback — observers are the operational/audit class, and Steward analyses are observational data; explicitly **no** `create`, `resolve`, or `steward.run_analysis`) |

**Steward class rationale:** `steward.run_analysis` is operator-only because it queues compute work and is the daemon-side trigger for the quality-gate pipeline; allowing `agent` or `observer` to trigger it would let lower-privilege principals cause analysis bursts and influence the audit trail. `steward.list_analyses` and `steward.get_analysis` are read-only surfaces over persisted analysis records — fitting the `observer` mandate of "read the operational audit without being able to mutate anything." Agents are kept out of steward entirely to preserve the scoped-to-run minimal-privilege contract (agents talk about their own run, not cross-cohort history).

Artifact reads happen through the `artifact://{id}` resource URI and the `chainworks://runs/{run_id}/artifacts` collection URI, not through a tool. Resource exposure is filtered by the same capability policy (§6). The `steward-analysis://{analysis_id}` resource mirrors the `steward.get_analysis` tool's class policy: **operator + observer** can read it; **agent** cannot.

These maps live in the shared `auth` crate as static `PrincipalClass → BTreeSet<CapabilityToolId>` tables for P029 (next to `filter_tools` / `filter_resources` from §4.0/§4.1). Promotion to YAML-driven policy is owned by the "northbound capability policy hardening" future proposal (§3.4).

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

**Owner of caller context:**

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

   **No `Internal` variant is defined in P029.** Current HEAD has no internal caller of `CommandHandler::handle` — executor (`executor.rs`) drives work via the orchestrator directly, and recovery (`recovery.rs`) writes to stages/repair tables without going through `CommandHandler`. Adding an `Internal` variant and an `Internal`-typed AC would be dead code on landing. Extending `CallerSurface` with an `Internal` variant is owned by the "MCP command-path consolidation" future proposal (§3.4), which is the first landing that reroutes an internal path through `CommandHandler`.

2. **`CommandHandler::handle` signature changes** from `handle(&self, cmd: Command)` to `handle(&self, cmd: Command, caller: CallerContext)`. Call sites updated:
   - `mcp-server/src/tools/*.rs` — construct `CallerContext::mcp(principal, tool_name)`
   - `graphql-server/src/schema.rs::MutationRoot` — construct `CallerContext::graphql(principal, mutation_name)`

   **Blast radius (acknowledge explicitly).** This signature change is workspace-wide: beyond the ~9 production call sites, every integration test that constructs a `CommandHandler` and calls `.handle(cmd)` must pass a `CallerContext`. Current test coverage includes at minimum 15 call sites in `engine/tests/integration.rs` plus `daemon/tests/mcp_stdio.rs`. To keep the test migration mechanical rather than semantic, P029 ships a convenience constructor:

   ```rust
   // domain/src/commands.rs — always public, unconditionally compiled
   impl CallerContext {
       /// Test/fixture stand-in. Tags rows as `caller_surface = 'mcp'` with a
       /// synthetic operator principal so tests that do not exercise auth
       /// still produce well-formed command_journal rows.
       ///
       /// This is a plain `pub fn`, not `cfg(test)` or `pub(crate)`,
       /// because the integration tests that need it live in `engine/tests/`,
       /// `graphql-server/tests/`, and `daemon/tests/` — all separate crates
       /// that cannot see `domain`'s cfg(test) items or pub(crate) symbols.
       pub fn test_fixture() -> Self {
           CallerContext {
               surface: CallerSurface::Mcp,
               principal_id: "test-operator".into(),
               principal_class: PrincipalClass::Operator,
               caller_tool: "test".into(),
           }
       }
   }
   ```

   Every existing test migrates from `handler.handle(cmd)` to `handler.handle(cmd, CallerContext::test_fixture())` — a mechanical search-and-replace, not a semantic change. The `test_fixture` name makes it obvious in production code reviews that this constructor is not for real callers.

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

### 4.4 Client-visible `journal_id` wire contract

`Commanded.journal_id` is an internal audit pointer. This section defines exactly how it reaches the client on each surface, so AC-11 is implementable end-to-end.

#### 4.4.a MCP tools/call response

**Which tools invoke `CommandHandler::handle`:** `runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, and `steward.run_analysis`. These are the "command tools." `ideas.create` and `ideas.list` write/read directly to the ideas repo without going through `CommandHandler` (there is no `CreateIdea` variant in `domain::commands`); `steward.list_analyses` and `steward.get_analysis` likewise read directly from the steward repo. These direct tools do **not** produce `command_journal` rows and do **not** return `journal_id`.

Every command tool returns an MCP `tools/call` result whose `content[0].text` is a JSON object containing the tool's existing result fields plus a top-level `journal_id` string. Example (for `runs.start`):

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

- **Protocol version note:** the server currently advertises `protocolVersion: "2024-11-05"` (see `mcp-server/src/server.rs:93`). That revision does **not** define the `structuredContent` typed-output channel — it was introduced in MCP 2025-06-18. P029 therefore delivers `journal_id` only inside `content[0].text` as stringified JSON. The protocol bump and the mirrored `structuredContent.journal_id` field are owned by the "MCP protocol version uplift" future proposal — see §3.4.
- **Non-command tools** (`ideas.create`, `ideas.list`, `runs.list`, `runs.get`, `approvals.list`, `reports.get`, `steward.list_analyses`, `steward.get_analysis`) do **not** invoke `CommandHandler` and therefore do **not** include `journal_id`. Their response shape is unchanged from current HEAD. Note that `ideas.create` is a mutating tool but bypasses `CommandHandler` — it writes directly to `ideas::insert`; the two steward direct tools are read-only. The "MCP command-path consolidation" future proposal (§3.4) is what adds `CreateIdea` + siblings to `domain::commands`; once that lands, the direct tools gain `journal_id` automatically without a P029-style change.

Owner: each tool module in `mcp-server/src/tools/*.rs` constructs the JSON from the `Commanded` returned by `CommandHandler::handle`.

#### 4.4.b GraphQL mutation response

Each of the five mutations gets a **dedicated payload type** that wraps the existing return value plus `journalId`. P029 does **not** add `journalId` to shared entity types (`Run`, `Approval`) because those types are also returned by read queries (`run(id:)`, `runs`, `approvals`) which have no mutation journal to resolve. Putting a non-null mutation-only field on a query-visible type would either be impossible to satisfy on read paths or force query-resolver special-casing.

| Mutation | Current return ([`schema.rs`](../../control-plane/crates/graphql-server/src/schema.rs)) | P029 payload type |
|---|---|---|
| `startRun` | `GqlStartRunResult` union: `Started { run }` / `Blocked { deliveryPreflightJson }` (line 138) | `StartRunPayload` union: `Started { run: Run!, journalId: ID! }` / `Blocked { deliveryPreflightJson: String!, journalId: ID! }` |
| `approveStage` | `Approval` (line 141) | `ApproveStagePayload { approval: Approval!, journalId: ID! }` |
| `rejectStage` | `Approval` (line 173) | `RejectStagePayload { approval: Approval!, journalId: ID! }` |
| `retryStage` | `Boolean` (line 205) | `RetryStagePayload { retried: Boolean!, journalId: ID! }` |
| `cancelRun` | `Boolean` (line 225) | `CancelRunPayload { cancelled: Boolean!, journalId: ID! }` |

**`startRun` special handling:** current HEAD already returns a union (`GqlStartRunResult` at [`schema.rs:138`](../../control-plane/crates/graphql-server/src/schema.rs)) with `Started` and `Blocked` variants. The `Blocked` variant carries `deliveryPreflightJson` for P048 delivery-preflight failures. Both variants go through `CommandHandler::handle` and produce a `command_journal` row, so both must carry `journalId`. `StartRunPayload` therefore preserves the union shape — each variant gains `journalId: ID!` alongside its existing fields. Clients that currently write `... on GqlStartRunStarted { run { id } }` update to `... on Started { run { id } journalId }`.

All other payload types are new GraphQL object types defined in `graphql-server/src/schema.rs`. They nest the existing return value under a named field (`approval`, `retried`, `cancelled`) so existing query fields on the inner type continue to resolve normally.

**Schema compatibility:** this is a breaking change for all five mutations — the return type changes from a direct entity/scalar to a wrapper. Existing clients that pattern-match on `startRun { id }` must update to `startRun { run { id } journalId }`. This is a one-time migration on P029 landing and is explicitly permitted because:

1. The only current in-repo GraphQL consumers of these five mutations are `graphql-server/tests/*` integration tests and the dev playground. A targeted repo search for Swift code that calls `/graphql`, `startRun`, or `journalId` found no hits; the SwiftUI client is still app-local for these actions and does **not** consume the GraphQL mutation surface today. The SwiftUI client also **never becomes** a GraphQL-mutation consumer — P031 routes commands through MCP (P031 §3.1), and P043 governs only the GraphQL **read** contract. The "who adopts this schema change later?" question is therefore closed: the future SwiftUI thin-client path does not touch GraphQL mutations. See §3.4 for the full ownership map.
2. P029's client-migration work is therefore limited to updating the `graphql-server` integration-test fixtures that currently assert the old response shapes, in the same commit that lands the schema change. No Swift source-file change is required and none is claimed.
3. Stage A (§5) is updated to say: "No behavior change for existing GraphQL clients **except** the mutation response wrapping documented in §4.4.b, which the in-repo integration tests absorb in the same commit." The prior draft's unqualified "no behavior change" claim and its "SwiftUI client updated in the same commit" claim are both retracted.

Owner: `graphql-server/src/schema.rs::MutationRoot` + the GraphQL object resolvers for `Run`, `Approval`, `CancelRunPayload`.

#### 4.4.c Internal callers

Internal callers (recovery, background executor) never surface `journal_id` to any external client. They may log it but have no wire contract. Their behavior is unchanged from current HEAD.

#### 4.4.d When `journal_id` is absent from the response

A tool call or mutation that returns an error (before `CommandHandler::handle` is invoked — e.g. capability denial, argument-validation failure) produces no `journal_id` because no `command_journal` row was written. The response shape in that case is the error variant (§4.2 for MCP, standard `async_graphql` errors for GraphQL) with no `journal_id` field.

This is the correct behavior: a client that receives a result without `journal_id` must treat that result as having no audit trail in `command_journal`.

### 4.5 Implementation handoff against the current tree

The current working tree already contains a partial auth scaffold that predates R6 / R8 and does **not** match the owner graph in §4.0. This subsection is a handoff note, not a new contract: it names the stale artifacts so the implementation owner replaces them outright instead of incrementally patching around them. Every item below is already cited by §4.0 as the canonical target; the list is collected here so a handoff review can check it in one pass.

| Stale artifact (current HEAD) | Correct R8 target (§4.0) | Action |
|---|---|---|
| `PrincipalClass` enum lives in `control-plane/crates/auth/src/lib.rs` (`pub enum PrincipalClass { Operator, Agent, Observer }`) | `domain/src/commands.rs` owns `PrincipalClass` | **Move** the enum to `domain`; `auth` re-exports or imports it. Do not keep a second copy. |
| `auth::Principal` has only `{ id, class }` | `auth::Principal { id, class, tool_capabilities: BTreeSet<CapabilityToolId>, resource_capabilities: BTreeSet<ResourceTemplateId> }` | **Extend** the struct with the two capability sets; populate them from the class → capability table at load time. |
| `auth::ToolSpec` + `auth::filter_tools(specs: &[ToolSpec]) -> Vec<String>` (string-name based) | `auth::filter_tools(p: &Principal, ids: &[CapabilityToolId]) -> Vec<CapabilityToolId>` consuming typed IDs from `domain` | **Replace** `ToolSpec` and its string-returning variant. The typed-ID version is the only shape that carries the compile-time drift guarantee in §4.0 / §11.2. |
| `CallerContext.principal_class: String` in `domain/src/commands.rs:86`; constructor signatures take `principal_class: &str` | `CallerContext.principal_class: PrincipalClass` (enum); constructors take `&PrincipalClass` or own the enum directly | **Retype** the field and all constructor call sites. String comparison on class is incompatible with the capability-drift compile-time guard. |
| No `domain/src/capabilities.rs` file on tree | New module defined in §4.0 with both enums | **Create** the module; register it in `domain/src/lib.rs`. |
| `mcp-server/src/tools/mod.rs` has no `capability_id_for` / `mcp_tool_for` converters | Both converters per §4.0 server-side converter bullet | **Add** both functions; `capability_id_for` must cover every tool in §2.1, including the steward triple. |
| `mcp-server/src/server.rs` has no `concrete URI → ResourceTemplateId` parser | Parser per §4.0 | **Add** the parser covering every template in §2.2, including `steward-analysis://{analysis_id}`. |
| `graphql-server/src/schema.rs` has no `MutationName` enum or `capability_id_for(MutationName)` converter | Both per §4.0 | **Add** both; map the five mutations to `CapabilityToolId` per §4.0. |
| daemon principal loading may not yet follow the §4.1 env/default path contract (`CHAINWORKS_AUTH_PRINCIPALS_PATH`, `~/.chainworks/auth/principals.json`, `0600` mode, one-time token log) | §4.1 contract | **Verify** at handoff that `daemon/src/main.rs` reads the env var, writes with `OpenOptions::mode(0o600)` on first bootstrap, and logs the token exactly once on the bootstrap run. |

**Landing rule.** The stale artifacts above are replaced in **one** slice together with the P029 additions. Partial replacement (e.g. moving `PrincipalClass` to `domain` but leaving `auth::ToolSpec` intact) is explicitly forbidden because it reintroduces the `auth::filter_tools(specs: &[ToolSpec]) -> Vec<String>` string-based escape hatch that the compile-time drift guard in §11.2 relies on not existing. The `proposal-029-mcp` gate (§9) is the proof lane for this landing; it cannot be green while any of the stale artifacts above still exists, because the typed-ID tests compile-fail against the string-based API.

**Baseline honesty.** §2.3 states "no auth header parsing exists" on the MCP HTTP transport. The current tree has drifted beyond that snapshot: there is partial principal-table infrastructure in `auth/src/lib.rs` (`PrincipalTable::load_or_bootstrap`, `PrincipalEntry`, a `test_fixture()` constructor). Treat §2.3's baseline statement as describing the **behavior** (no request-time auth gate is actually wired into `McpServer::handle_request` yet), not the codebase. The partial scaffold exists; it simply is not the R8 contract and must be replaced per the table above, not polished in place.

## 5. GraphQL coexistence and cutover

The prior draft proposed a three-stage plan where Stage B refactored GraphQL mutations to call MCP tool handlers. That would have inverted the current clean dependency graph (both `graphql-server` and `mcp-server` depend only on `engine`; neither depends on the other). P029 drops that middle stage. The revised plan is two stages, and the "dual authority" concern is addressed by shared audit, not by cross-crate coupling.

### Stage A — Coexistence (P029 landing)

- Both GraphQL mutations and **MCP command tools** (`runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`) remain active. **MCP direct tools** (`ideas.create`, `ideas.list`, `runs.list`, `runs.get`, `approvals.list`, `reports.get`) remain direct repo readers/writers and are **not** part of this coexistence contract — they do not invoke `CommandHandler`, do not supply a `CallerContext`, and do not emit `journal_id`. Moving them onto `CommandHandler` is owned by the "MCP command-path consolidation" future proposal (§3.4).
- Both GraphQL mutations and MCP command tools converge on the same `CommandHandler` via the shared `Command` enum from `domain::commands`.
- Both GraphQL mutations and MCP command tools supply a `CallerContext`, so both land rows in `command_journal` with `caller_surface` set. `command_journal` itself is the divergence detector — if the two surfaces ever produce different journal rows for "the same operation," the audit table will show it.
- No cross-crate dependency is introduced beyond the new `auth` edge from §4.0. `graphql-server` and `mcp-server` both continue to depend only on `engine` + the new shared `auth` crate, which depends only on `domain`.
- **GraphQL schema break (one-time, absorbed on landing):** all five mutation return types change from direct entities/scalars to dedicated payload wrappers per §4.4.b. The in-repo consumers that require migration are the `graphql-server/tests/*` integration tests and the dev playground — both are updated in the same commit. The SwiftUI client does not consume these mutations today **and never will**: P031 routes commands through MCP, not GraphQL mutations, so there is no future SwiftUI migration obligation attached to this schema break (see §3.4 SwiftUI row). After this one-time test-fixture migration, no further behavior change occurs in Stage A.

This is the default state when P029 lands.

### Stage B — GraphQL mutations deprecated (was "Stage C" in prior drafts)

- GraphQL schema keeps the mutation fields but marks them `@deprecated(reason: "use MCP tools")`.
- Residual GraphQL traffic is observable via `command_journal` rows where `caller_surface = 'graphql'` — the audit table is the counter; no separate telemetry is needed.
- No removal in P029. Removal is owned by the "GraphQL mutation retirement" future proposal (§3.4); its trigger is zero residual `caller_surface = 'graphql'` rows in `command_journal` for an agreed observation window after P031 lands.

**Canonical mutation authority** after Stage B: both surfaces are still technically authoritative and both funnel through `CommandHandler`. "MCP is canonical" becomes true only when the "GraphQL mutation retirement" proposal (§3.4) lands; until then, the two surfaces are genuine peers. P029 does not overclaim.

### Why no intermediate "GraphQL wraps MCP" stage

The proposal's prior draft proposed such a stage. It was removed because:

1. The two surfaces already converge cleanly on `CommandHandler`. "Dual command-authoring" is just two call sites constructing the same typed `Command` enum — not a divergence in semantics.
2. Making `graphql-server` depend on `mcp-server::tools::*::handle_*` would invert the dependency graph and create a new cross-crate coupling worse than the status quo.
3. `CallerContext` already distinguishes the two surfaces in the audit trail, which is the only place "dual authority" could actually leak into product behavior.

Any future "collapse both into one command-building layer" work would belong in a new proposal (not currently drafted; captured under "GraphQL mutation retirement" in §3.4 as a prerequisite possibility, not a hard dependency) and would likely extract a shared `domain::commands::builders` helper crate, not route GraphQL through MCP internals.

## 6. Resource surface alignment

- Canonical single-entity URIs for P029: `run://{id}`, `idea://{id}`, `artifact://{id}`, `report://{id}` (all already implemented).
- Canonical collection URIs for P029: the `chainworks://` family listed in §2.2 (already implemented).
- Dropped from the specification; adoption is owned by the "named workflow/approval resource readers" future proposal (§3.4): `workflow://{id}`, `approval://{id}`.
- Resource reads must go through the same capability filter as tool calls (`auth::filter_resources`, shared crate from §4.1). `resources/list` returns a per-principal vector.

## 7. Migration strategy

P029 lands in a single step. There is no multi-phase rollout of the *proposal itself* — auth and capability enforcement must land together, because partial deployment would be worse than the current state (auth without filtering would be security theatre; filtering without auth would be arbitrary).

### 7.1 Dogfood migration (closes UX-029-01)

The active development workflow uses `.mcp.json` at repo root with `"type": "http", "url": "http://127.0.0.1:4000/mcp"` — no credentials. `CLAUDE.md` documents this bare-HTTP connection as the canonical Claude Code ↔ daemon path. P029's mandatory auth would break this workflow on landing.

**Migration contract:**

1. **P029 generates a default operator token at first start.** When `auth::load_table` finds no principal table file, it creates one at `~/.chainworks/auth/principals.json` with a single `operator`-class token (random UUID). The daemon logs the path and token value at startup so the operator can copy it into the MCP client config. Subsequent starts reuse the existing file.

2. **`.mcp.json` updated in the same commit.** The repo-committed `.mcp.json` changes to include the `Authorization` header via the MCP HTTP transport's `headers` field (supported by Claude Code's HTTP MCP transport):

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

   The token value is read from the environment variable `CHAINWORKS_MCP_TOKEN`. `CLAUDE.md` is updated to document the new env var and points at the auto-generated principals file.

3. **stdio path:** `CLAUDE.md` already documents HTTP, not stdio, as the dogfood transport. No `.mcp.json` change is needed for stdio. If a developer switches to stdio, they pass the token in `initialize.params.clientInfo.principal_token` per §4.1.b.

After P029 lands, GraphQL mutation cutover follows the two-stage rule in §5. Each stage has its own proposal or gate update.

**Rollback:** if the `proposal-029-mcp` gate is red after merge, revert the auth wiring, the filter wiring, the `CommandHandler` signature change, and the migration that added the four nullable columns. The `command_journal` rows already written with populated `caller_*` columns remain valid on a pre-P029 schema only if the added columns are preserved; simplest recovery is to keep the migration in place but ignore the new columns. Revert `.mcp.json` to the bare-HTTP version and drop the principals file.

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
11. `CommandHandler::handle` returns `Commanded { result, journal_id }`. MCP command tools (`runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, `steward.run_analysis` — but **not** `ideas.create`, `steward.list_analyses`, or `steward.get_analysis`, which all bypass `CommandHandler`) include `journal_id` inside `content[0].text` stringified JSON per §4.4.a. Every GraphQL mutation return type exposes a `journalId: ID!` field via a dedicated payload wrapper per §4.4.b. Non-command tools and error paths that never reach `CommandHandler` produce no `journal_id` (per §4.4.d).
12. `resources/list` is capability-filtered the same way as `tools/list`. For each class the returned template set matches §4.2: `operator` sees all templates including `steward-analysis://{analysis_id}`; `observer` sees `run://`, `idea://`, `artifact://`, `report://`, `steward-analysis://{analysis_id}`, and every `chainworks://` collection; `agent` does **not** see `steward-analysis://{analysis_id}`.
13. `resources/read` enforces per-principal filtering on the concrete URI it receives (including `steward-analysis://{analysis_id}` for operator/observer but **not** agent). Since `resources/list` returns template URIs (e.g. `chainworks://runs/{run_id}/artifacts`) while `resources/read` receives concrete instances (e.g. `chainworks://runs/abc-123/artifacts`), the filter must use a **template-instance matcher**: extract the URI scheme and path pattern from the concrete URI, match it against the principal's allowed resource template set, and reject with `-32002` (resource not found) if no template matches. Owner: `auth::match_resource_uri(principal, concrete_uri) -> bool`, which strips path parameters and compares against the same template list that `auth::filter_resources` uses for `resources/list`. This covers the direct `resources/read` path at [`server.rs:216`](../../control-plane/crates/mcp-server/src/server.rs) which currently has no guard.
14. The `proposal-029-mcp` gate (see §9) is green on the same tree.
15. **Principal table bootstrap** creates `~/.chainworks/auth/principals.json` with Unix file mode `0600` (owner-only) on first start; the bootstrap token is emitted at `info` level **exactly once** — on the start that created the file. Subsequent daemon starts log only the principals-file path, never the token. A zero-principal or unparseable principals file fails the daemon closed with a clear error. (File-mode assertion is skipped on non-Unix platforms; the corresponding test carries a `cfg(unix)` gate.)

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
# Principal table bootstrap
test_principals_file_created_with_owner_only_permissions   # Unix mode 0600 on first-start; skipped on non-Unix
test_principals_bootstrap_token_logged_once_on_first_start # subsequent starts log only the path, never the token
test_principals_daemon_refuses_empty_principals_file       # fail-closed invariant

# Transport auth — MCP HTTP
test_mcp_http_rejects_missing_authorization_header
test_mcp_http_rejects_unknown_bearer_token

# Transport auth — MCP stdio (initialize.params.clientInfo.principal_token)
test_mcp_stdio_rejects_first_frame_other_than_initialize
test_mcp_stdio_rejects_initialize_without_principal_token
test_mcp_stdio_rejects_initialize_with_unknown_principal_token
test_mcp_stdio_binds_principal_for_session_lifetime
test_mcp_stdio_rejects_reinitialize_mid_session       # session-lifetime immutability

# Transport auth — GraphQL
test_graphql_rejects_missing_authorization_header
test_graphql_rejects_unknown_bearer_token
test_graphql_mutation_reads_principal_from_context
test_graphql_observer_class_cannot_invoke_start_run
test_graphql_ws_rejects_missing_connection_init_auth
test_graphql_ws_rejects_unknown_connection_init_token
test_graphql_ws_accepts_valid_connection_init_token

# Capability policy
test_mcp_tools_list_filtered_for_operator
test_mcp_tools_list_filtered_for_agent
test_mcp_tools_list_filtered_for_observer
test_mcp_tools_call_denied_returns_method_not_found
test_mcp_resources_list_is_capability_filtered
test_mcp_resources_read_denied_returns_not_found      # AC-13: resources/read guard

# Capability policy — Steward (absorbed from P049 surface, see §4.2)
test_mcp_tools_list_includes_steward_trio_for_operator     # operator sees run_analysis + list + get
test_mcp_tools_list_includes_steward_readers_for_observer  # observer sees list + get, NOT run_analysis
test_mcp_tools_list_excludes_steward_entirely_for_agent    # agent sees none of the three
test_mcp_tools_call_steward_run_analysis_denied_for_observer_returns_method_not_found
test_mcp_tools_call_steward_run_analysis_denied_for_agent_returns_method_not_found
test_mcp_resources_list_includes_steward_analysis_template_for_operator_and_observer
test_mcp_resources_list_excludes_steward_analysis_template_for_agent
test_mcp_resources_read_steward_analysis_denied_for_agent_returns_not_found

# Audit contract — against command_journal
test_command_journal_row_has_caller_mcp_for_runs_start    # covers one MCP tool...
test_command_journal_row_has_caller_mcp_for_approvals_resolve   # ...and a second to catch tool-site wiring errors
test_command_journal_row_has_caller_mcp_for_steward_run_analysis  # proves steward command tool is journaled like the other four
test_command_journal_row_has_caller_graphql_for_start_run
test_command_journal_row_has_caller_graphql_for_approve_stage
test_command_journal_caller_columns_nullable_for_pre_p029_rows
test_command_journal_payload_redacted_for_sensitive_fields

# journal_id surfacing (§4.4)
test_mcp_tools_call_response_includes_journal_id_in_content_text
test_mcp_read_only_tool_response_omits_journal_id
test_mcp_steward_run_analysis_response_includes_journal_id      # steward is a command tool
test_mcp_steward_list_analyses_response_omits_journal_id         # steward list is a direct tool
test_mcp_steward_get_analysis_response_omits_journal_id          # steward get is a direct tool
test_graphql_start_run_started_variant_includes_journal_id
test_graphql_start_run_blocked_variant_includes_journal_id
test_graphql_approve_stage_returns_payload_with_approval_and_journal_id
test_graphql_retry_stage_returns_payload_with_retried_and_journal_id
test_graphql_cancel_run_returns_payload_with_cancelled_and_journal_id
test_response_omits_journal_id_when_capability_check_fails

# Cross-surface parity (Stage A coexistence)
test_graphql_and_mcp_produce_identical_run_for_start_run
```

**Tests explicitly NOT in this inventory** (with rationale):

- `test_command_journal_row_has_caller_internal_for_recovery` — dropped. No internal caller of `CommandHandler::handle` exists on current HEAD; see §4.3. This test is owned by the "MCP command-path consolidation" future proposal (§3.4), which is what introduces the first `CallerSurface::Internal` path.
- `test_mcp_tools_call_response_includes_journal_id_in_structured_content` — dropped. The server advertises `protocolVersion: "2024-11-05"`; `structuredContent` is only defined from MCP 2025-06-18. This test is owned by the "MCP protocol version uplift" future proposal (§3.4), which is the one that bumps `protocolVersion` and adds the wire-shape change.

Gate runner: run the full Rust workspace tests, same pattern as `proposal-027` and `proposal-044`.

**Registration:** `scripts/test-gate.sh` must add a `PROPOSAL_029_MCP_TESTS` array and a `proposal-029-mcp|p029-mcp)` case block. The existing `proposal-029|p029` case (second-wave ACP runtime) stays unchanged.

## 10. Non-goals (unchanged; owner for each is named in §3.4)

P029 does not:
- rewrite the UI — owner: **Proposal 031** (thin UI rewrite) per §3.4,
- move business logic into MCP — owner: **Proposal 027** owns `CommandHandler` as the current orchestration authority per §3.4,
- replace southbound runtime protocols — explicitly out of northbound scope; southbound track proposals are the owners per §3.4,
- force every high-frequency read through MCP — owner: **Proposal 043** (read projections contract) per §3.4,
- define token rotation, revocation, or delegation policy — owner: "northbound auth lifecycle" future proposal (not yet drafted) per §3.4,
- deliver any tool or resource listed in §3.2 — owner per item: see the Target owner column in §3.2 (and consolidated in §3.4).

## 11. Risks

### 11.1 Auth-as-theatre
Risk: auth lands without capability filtering, or filtering without auth.
Mitigation: §7 forbids partial rollout; both land together in one slice.

### 11.2 Capability drift
Risk: static class tables in the shared `auth` crate become stale as tools are added.
Mitigation: `CapabilityToolId` and `ResourceTemplateId` are closed enums owned by `domain` (see §4.0). `auth::filter_tools` uses an exhaustive `match` over `CapabilityToolId` to consult the class → capabilities static table; `mcp-server::tools::capability_id_for` and `mcp_tool_for` use exhaustive matches to cross the `McpTool` ⇄ `CapabilityToolId` boundary. Adding a new tool therefore fails to compile in both `auth` and `mcp-server` until the capability table and the tool-name converter are updated — no new MCP or GraphQL tool can ship without a compile-time reminder to update capability policy.

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
