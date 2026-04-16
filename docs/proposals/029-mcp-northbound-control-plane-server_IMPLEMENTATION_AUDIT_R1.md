# Proposal 029: MCP Northbound Control-Plane Server Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/029-mcp-northbound-control-plane-server.md` |
| Repository Root | `.` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working Tree | Dirty: broad control-plane/docs changes present; this audit did not modify implementation or proposal files. |
| Audited At | `2026-04-16T08:51:44+03:00` |
| Platform Scope | macOS control-plane daemon / Rust northbound server surfaces |
| Proposal State | Active |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 029 is not implemented on the audited tree. The current implementation still exposes unauthenticated MCP HTTP, unauthenticated MCP stdio initialize, unauthenticated GraphQL HTTP/WS, static `tools/list` and `resources/list`, unfiltered `tools/call` / `resources/read`, and the old `CommandHandler::handle(cmd) -> CommandResult` journal path. The existing MCP and GraphQL package tests pass, but they prove the pre-P029 baseline only; the named `proposal-029-mcp` gate is not registered.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Core auth, capability, journal, and gate requirements are absent | High |
| Architecture | Weak | No shared auth crate, no caller context, no caller-scoped journal schema | High |
| Product | At Risk | Future P031/automation callers would still receive full unauthenticated authority | High |
| UI | Not Applicable | Proposal explicitly excludes UI rewrite | High |
| UX | At Risk | Dogfood MCP config remains bare HTTP with no token setup path | High |
| Readiness | Not Ready | Proof lane is missing and acceptance tests are absent | High |

## Proposal Contract

### Scope

P029 is a delta on the existing Rust control plane: add principal resolution on MCP HTTP, MCP stdio, GraphQL HTTP, and GraphQL WS; add caller-scoped capability policy for tools and resources; extend the existing `command_journal`; define GraphQL coexistence/cutover; keep the current canonical resource URI surface; and register `./scripts/test-gate.sh proposal-029-mcp`.

### Locked Decisions

- `Principal`, `PrincipalClass`, `AuthError`, `resolve_bearer`, `filter_tools`, `filter_resources`, and URI matching live in a shared `control-plane/crates/auth` crate or equivalent auth module.
- Token material is loaded from `CHAINWORKS_AUTH_PRINCIPALS_PATH`, defaulting to `~/.chainworks/auth/principals.json`, with first-start operator bootstrap and fail-closed startup on malformed/empty files.
- MCP HTTP resolves `Authorization: Bearer <token>` on every request and passes `Principal` into `McpServer::handle_request`.
- MCP stdio authenticates during the first `initialize` request via `params.clientInfo.principal_token`, binds principal for session lifetime, and rejects reinitialize.
- GraphQL HTTP and `/graphql/ws` share auth and inject `Principal` into resolver context.
- Capability denial for MCP tools returns `-32601`, not a forbidden error.
- P029 extends `command_journal`; it does not create `mcp_audit_log`.
- `CommandHandler::handle` accepts `CallerContext` and returns `Commanded { result, journal_id }`.
- Redaction happens inside `engine::command_journal_redact::redact_for_journal` before journal insert.
- GraphQL mutations remain active in Stage A, but all five return dedicated payload wrappers with `journalId`.
- `.mcp.json` and `CLAUDE.md` migrate dogfood MCP HTTP to `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}`.
- Deferred tools/resources in §3.2 remain out of scope.

### Primary User Flows

1. Operator starts the daemon, receives or reuses an operator token, and configures local Claude Code MCP HTTP with `CHAINWORKS_MCP_TOKEN`.
2. MCP client authenticates, sees only class-allowed tools/resources, and invokes allowed command tools.
3. Unauthorized MCP clients cannot discover or invoke unauthorized tools/resources.
4. SwiftUI GraphQL clients continue using mutations during Stage A, but now authenticated, capability-checked, and journaled with `caller_surface = 'graphql'`.
5. Operators and reviewers can inspect `command_journal` rows and correlate commands to `caller_surface`, principal, tool/mutation, result, and redacted payload.
6. Engineers can run `./scripts/test-gate.sh proposal-029-mcp` as the deterministic proof lane.

### UI Commitments

No UI rewrite is in scope.

### UX Commitments

- Dogfood MCP workflow must be migrated in the same commit so mandatory auth does not strand Claude Code users.
- Denials should avoid capability probing by returning method/resource-not-found style errors.
- GraphQL schema break is explicitly absorbed by updating the only current local SwiftUI consumer in the same commit.

### Acceptance Criteria

The proposal defines 14 acceptance criteria in §8: MCP HTTP auth, MCP stdio auth/session immutability, GraphQL auth, per-principal tool filtering, tool-call denial, GraphQL forbidden behavior, MCP and GraphQL caller journal rows, payload redaction, `journal_id` surfacing, resource filtering/read guarding, and green `proposal-029-mcp` gate.

### Test / Evidence Requirements

The proposal requires `./scripts/test-gate.sh proposal-029-mcp`, a `PROPOSAL_029_MCP_TESTS` array, a `proposal-029-mcp|p029-mcp` case block, and focused tests covering transport auth, capability policy, journal shape, `journal_id`, resource filtering, and cross-surface parity.

### Explicit Exclusions

No UI rewrite, no business-logic move into MCP, no southbound protocol replacement, no high-frequency read migration, no token rotation/revocation/delegation policy, and no deferred tools/resources from §3.2.

## Proposal Fidelity / Divergence

### Matches

- Current implementation already has the baseline MCP server with HTTP route, stdio loop, static tool registry, JSON-RPC dispatch, resources, and command tools.
- Current implementation already has active GraphQL mutations and MCP command tools converging on `CommandHandler`.
- Current implementation still has no cross-crate dependency from GraphQL to MCP, matching the Stage A architecture constraint.
- Deferred tool expansion is not present in the MCP tool registry.

### Divergences

- No shared auth crate/module exists in the workspace or crate dependencies.
- MCP HTTP does not parse `Authorization` or reject unauthenticated requests.
- MCP stdio accepts `initialize` without `principal_token` and has no session auth state.
- GraphQL HTTP and `/graphql/ws` are mounted without auth middleware.
- `tools/list` and `resources/list` return static vectors.
- `tools/call` and `resources/read` dispatch before capability filtering.
- `command_journal` schema and repo writer lack caller columns.
- `CommandHandler::handle` still takes only `Command` and returns `CommandResult`, with no `CallerContext`, no `Commanded`, and no returned `journal_id`.
- `payload_json` is serialized directly, not redacted through `command_journal_redact`.
- MCP command tools and GraphQL mutations do not expose `journal_id`.
- `.mcp.json` and `CLAUDE.md` still document bare HTTP MCP.
- `proposal-029-mcp` is not registered in `scripts/test-gate.sh`.

### Ambiguities / Evidence Gaps

- Runtime auth behavior was not exercised because the implementation has no auth seam to configure.
- Full regression was not run because the audit verdict is not successful; the skill's full-regression requirement only gates `Implemented`, `Ready`, or `Ready with Risks` outcomes.
- The working tree is dirty with many unrelated files modified, deleted, and untracked. Findings are still high confidence because the audited P029 surfaces show direct absence of the required constructs.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 2 |
| Partially Implemented | 2 |
| Missing | 14 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Shared auth crate and token material loading

- Proposal Source: §4.1 lines 132-156.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/Cargo.toml:1` lists workspace crates and has no `crates/auth` member.
  - `control-plane/crates/mcp-server/Cargo.toml:6` and `control-plane/crates/graphql-server/Cargo.toml:6` do not depend on an auth crate.
  - `control-plane/crates/daemon/src/main.rs:27` reads only `DATABASE_URL`, `GRAPHQL_ADDR`, and `MODE`.
- Gap / Note: No `Principal`, `PrincipalClass`, principal table, `CHAINWORKS_AUTH_PRINCIPALS_PATH`, bootstrap, or fail-closed startup behavior is implemented.

### REQ-002 MCP HTTP bearer auth

- Proposal Source: §4.1.a lines 158-165; AC-1 line 495.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/http.rs:36` receives `HeaderMap`, but only uses `mcp-session-id` at lines 69-77.
  - `control-plane/crates/mcp-server/src/http.rs:67` calls `mcp.handle_request(request).await` without a principal.
- Gap / Note: Requests without `Authorization` still reach the handler instead of returning JSON-RPC `-32000`.

### REQ-003 MCP stdio initialize auth and session immutability

- Proposal Source: §4.1.b lines 167-188; AC-2 and AC-3 lines 496-497.
- Status: Missing
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:57` deserializes raw `JsonRpcRequest`, then `server.rs:86` handles each request statelessly.
  - `control-plane/crates/mcp-server/src/server.rs:90` returns initialize success without checking `params.clientInfo.principal_token`.
  - `control-plane/crates/mcp-server/src/protocol.rs:3` has only generic `JsonRpcRequest`; no `ClientInfo` or `principal_token` model exists.
  - `control-plane/crates/daemon/tests/mcp_stdio.rs:25` sends initialize with empty params and expects protocol success.
- Gap / Note: The current stdio test proves the old unauthenticated behavior remains accepted.

### REQ-004 GraphQL HTTP auth and mutation capability enforcement

- Proposal Source: §4.1.c lines 190-216; AC-4 and AC-7 lines 498-501.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/server.rs:26` mounts `GraphQL::new(schema.clone())` directly.
  - No `control-plane/crates/graphql-server/src/auth_layer.rs` file exists.
  - `control-plane/crates/graphql-server/src/schema.rs:145`, `schema.rs:195`, `schema.rs:228`, `schema.rs:261`, and `schema.rs:277` begin mutations without reading `Principal` from `Context`.
- Gap / Note: GraphQL HTTP requests and mutations remain unauthenticated and unfiltered.

### REQ-005 GraphQL WebSocket auth

- Proposal Source: §4.1.c lines 218-232.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/server.rs:27` creates `GraphQLSubscription::new(schema)`.
  - `control-plane/crates/graphql-server/src/server.rs:34` mounts `/graphql/ws` directly with no auth layer or connection-init hook.
- Gap / Note: Upgrade-header auth, `connection_init` fallback, 4401 close behavior, and WS tests are absent.

### REQ-006 Caller-scoped `tools/list`

- Proposal Source: §4.2 lines 235-254; AC-5 line 499.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:104` returns every `self.tool_specs` entry.
  - `control-plane/crates/mcp-server/src/server.rs:21` constructs the full static list in `McpServer::new`.
- Gap / Note: Operator, agent, and observer class tables are absent.

### REQ-007 Caller-scoped `tools/call` denial with `-32601`

- Proposal Source: §4.2 lines 237-240; AC-6 line 500.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:120` enters `tools/call`, extracts the name, and dispatches at line 130.
  - `control-plane/crates/mcp-server/src/server.rs:140` maps tool errors to `-32603`, not capability denials to `-32601`.
- Gap / Note: There is no principal-aware allowed-set check before dispatch.

### REQ-008 Canonical resource URI surface alignment

- Proposal Source: §2.2 lines 50-67; §6 lines 448-453.
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:144` exposes `resources/list`.
  - `control-plane/crates/mcp-server/src/server.rs:155`, `server.rs:161`, `server.rs:167`, and `server.rs:173` expose `run://`, `idea://`, `artifact://`, and `report://`.
  - `control-plane/crates/mcp-server/src/server.rs:180`, `server.rs:186`, `server.rs:192`, `server.rs:198`, and `server.rs:204` expose the `chainworks://` collection family.
  - `rg` found no current `workflow://` or `approval://` resource registration in `mcp-server/src/server.rs`.
- Gap / Note: This is the baseline surface only; filtering is covered separately.

### REQ-009 Capability-filtered resources and concrete `resources/read` guard

- Proposal Source: §6 lines 448-453; AC-12 and AC-13 lines 506-507.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:144` returns the full resource list without principal input.
  - `control-plane/crates/mcp-server/src/server.rs:219` accepts `resources/read` and calls `handle_resource_read` at line 231.
  - `control-plane/crates/mcp-server/src/server.rs:253` calls `read_resource(uri)` directly.
- Gap / Note: No template-instance matcher or resource-not-found denial path exists.

### REQ-010 Extend `command_journal` caller schema

- Proposal Source: §4.3 lines 256-276; AC-8 and AC-9 lines 502-503.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:104` defines `command_journal` without caller columns.
  - `control-plane/crates/db/migrations/010_evidence_preflight_and_mcp.sql:1` does not alter `command_journal`.
  - `control-plane/crates/db/src/repos/command_journal.rs:18` inserts only `id`, `command_type`, `payload_json`, `result_status`, `run_id`, and `created_at`.
- Gap / Note: Existing rows cannot store `caller_surface`, principal id/class, or tool/mutation name.

### REQ-011 `CallerContext` and `CommandHandler::handle(cmd, caller)`

- Proposal Source: §4.3 lines 286-317.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/commands.rs:5` defines `Command` variants but no `CallerContext` or `CallerSurface`.
  - `control-plane/crates/engine/src/command_handler.rs:69` still has `pub async fn handle(&self, cmd: Command) -> Result<CommandResult>`.
  - `rg` shows all production call sites still invoke `.handle(cmd)` without caller context.
- Gap / Note: The caller context blast-radius migration has not started.

### REQ-012 Command payload redaction inside engine

- Proposal Source: §4.3 lines 319-333; AC-10 line 504.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:80` serializes `payload_json` directly with `serde_json::to_string(&cmd)`.
  - `rg -n "command_journal_redact|redact_for_journal"` found no implementation under `control-plane/crates`.
- Gap / Note: Sensitive fields are not redacted by the proposed engine-owned path.

### REQ-013 `Commanded { result, journal_id }` and MCP `journal_id` surfacing

- Proposal Source: §4.3 lines 335-346; §4.4.a lines 358-383; AC-11 line 505.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:31` defines `CommandResult` only.
  - `control-plane/crates/engine/src/command_handler.rs:69` returns `Result<CommandResult>`, not `Result<Commanded>`.
  - `control-plane/crates/mcp-server/src/tools/runs.rs:115`, `approvals.rs:82`, and `stages.rs:43` ignore any journal id because none is returned.
  - `control-plane/crates/mcp-server/src/server.rs:133` wraps whatever tool JSON returns; no top-level `journal_id` is added.
- Gap / Note: MCP command tools cannot satisfy the client-visible audit pointer contract.

### REQ-014 GraphQL mutation payload wrappers with `journalId`

- Proposal Source: §4.4.b lines 385-404; AC-11 line 505.
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:127` defines `GqlStartRunStarted`, `schema.rs:132` defines `GqlStartRunBlocked`, and `schema.rs:137` defines a `GqlStartRunResult` union from other active work.
  - `control-plane/crates/graphql-server/src/schema.rs:195` still returns `GqlApproval` for `approve_stage`.
  - `control-plane/crates/graphql-server/src/schema.rs:228` still returns `GqlApproval` for `reject_stage`.
  - `control-plane/crates/graphql-server/src/schema.rs:261` and `schema.rs:277` still return `bool`.
  - `rg -n "journalId|journal_id"` found no GraphQL payload field.
- Gap / Note: One mutation has a wrapper-like union, but not the P029 dedicated `StartRunPayload { run, journalId }`; the other four are unchanged.

### REQ-015 Stage A GraphQL/MCP coexistence through shared engine, no GraphQL-to-MCP dependency

- Proposal Source: §5 lines 416-446.
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:164`, `schema.rs:209`, `schema.rs:242`, `schema.rs:268`, and `schema.rs:284` construct shared `Command` enum values.
  - `control-plane/crates/mcp-server/src/tools/runs.rs:105`, `approvals.rs:68`, and `stages.rs:42` also construct shared `Command` enum values.
  - `control-plane/crates/graphql-server/Cargo.toml:6` has dependencies on `domain`, `db`, and `engine`, not `mcp-server`.
- Gap / Note: The coexistence topology exists, but the shared `CallerContext`/journal divergence detector required by P029 is missing.

### REQ-016 Dogfood `.mcp.json` and `CLAUDE.md` migration

- Proposal Source: §7.1 lines 459-485.
- Status: Missing
- Evidence Type: code
- Evidence:
  - `.mcp.json:3` defines `chainworks-control-plane` with only `type` and `url`.
  - `.mcp.json:5` is bare `http://127.0.0.1:4000/mcp` with no `headers.Authorization`.
  - `CLAUDE.md:57` still documents connecting Claude Code via bare HTTP MCP.
- Gap / Note: The operator token setup path and `CHAINWORKS_MCP_TOKEN` docs are absent.

### REQ-017 `proposal-029-mcp` gate registration and focused P029 tests

- Proposal Source: §9 lines 512-592; AC-14 line 508.
- Status: Missing
- Evidence Type: code, tests-run
- Evidence:
  - `scripts/test-gate.sh:1192` lists `proposal-029` for second-wave ACP runtime profiles, not P029 MCP.
  - `scripts/test-gate.sh:1425` routes `proposal-029|p029` to `PROPOSAL_029_TESTS`.
  - `rg -n "PROPOSAL_029_MCP|proposal-029-mcp|p029-mcp" scripts/test-gate.sh docs/reference/test-gates.md` returned no implementation references.
  - `./scripts/test-gate.sh proposal-029-mcp` exited 1 with `error: Unknown gate: proposal-029-mcp`.
- Gap / Note: The canonical proof lane is absent.

### REQ-018 P029 deferred tool/resource scope remains absent

- Proposal Source: §3.2 lines 98-114; §10 lines 594-602.
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:22` registers only `ideas`, `runs`, `approvals`, `stages`, and `reports`.
  - `control-plane/crates/mcp-server/src/tools/runs.rs:11`, `ideas.rs:11`, `approvals.rs:11`, `stages.rs:10`, and `reports.rs:11` define only the current first-wave tool set.
  - `control-plane/crates/mcp-server/src/server.rs:144` resource list does not include `workflow://{id}` or `approval://{id}`.
- Gap / Note: The implementation respects the no-expansion boundary.

## Architecture Review

**Summary:** Weak

### ARCH-001 Auth boundary is entirely absent

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-001 through REQ-007, REQ-009
- Evidence Type: code
- Evidence:
  - `control-plane/Cargo.toml:1`
  - `control-plane/crates/mcp-server/src/http.rs:36`
  - `control-plane/crates/mcp-server/src/server.rs:90`
  - `control-plane/crates/graphql-server/src/server.rs:26`
- Why It Matters: P029 is primarily a security/capability proposal. Without a shared principal resolver and auth injection on every ingress, all downstream filtering and audit claims are unenforceable.
- Recommended Action: Implement the shared auth crate/module first, wire principal resolution through daemon construction, and make MCP/GraphQL handlers accept `Principal` before adding policy-specific tests.

### ARCH-002 Audit truth remains pre-P029 and cannot identify callers

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-010 through REQ-013, REQ-015
- Evidence Type: code
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:104`
  - `control-plane/crates/db/src/repos/command_journal.rs:18`
  - `control-plane/crates/engine/src/command_handler.rs:69`
  - `control-plane/crates/engine/src/command_handler.rs:80`
- Why It Matters: The proposal's GraphQL/MCP coexistence safety depends on caller-attributed journal rows. The current journal only proves that a command happened, not who invoked it or through which surface.
- Recommended Action: Add the nullable caller-column migration, introduce `CallerContext`, return `Commanded`, and update both MCP and GraphQL call sites in one mechanical migration.

### ARCH-003 Resource read remains a direct data exfiltration path

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-009
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:219`
  - `control-plane/crates/mcp-server/src/server.rs:253`
  - `control-plane/crates/mcp-server/src/server.rs:269`
- Why It Matters: Filtering `resources/list` is not sufficient if a caller can directly request a known `artifact://` or `chainworks://runs/{id}/artifacts` URI. P029 explicitly calls out the direct read guard.
- Recommended Action: Implement `auth::match_resource_uri(principal, concrete_uri) -> bool` and enforce it before `read_resource`.

## Product Review

**Summary:** At Risk

### PROD-001 Future northbound automation still gets full authority

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-001 through REQ-007
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:104`
  - `control-plane/crates/mcp-server/src/server.rs:120`
  - `control-plane/crates/graphql-server/src/schema.rs:145`
- Why It Matters: P029 is the prerequisite for P031 and third-party automation. In the current implementation, any local caller that reaches the MCP or GraphQL endpoint can see and invoke the full surface.
- Recommended Action: Treat P029 as blocked until unauthorized/mis-scoped callers are denied across both surfaces with proposal-specified errors.

## UI Review

**Summary:** Not Applicable

No UI findings. P029 explicitly excludes UI rewrite and the audited implementation surface is the Rust northbound server plus dogfood configuration.

## UX Review

**Summary:** At Risk

### UX-001 Dogfood migration is absent

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-016
- Evidence Type: code
- Evidence:
  - `.mcp.json:3`
  - `.mcp.json:5`
  - `CLAUDE.md:57`
- Why It Matters: The proposal says auth and filtering land together. If auth is implemented without updating dogfood config/docs in the same change, local Claude Code MCP users lose their canonical daemon path.
- Recommended Action: Update `.mcp.json` with `headers.Authorization = "Bearer ${CHAINWORKS_MCP_TOKEN}"` and document the generated principal file plus env var in `CLAUDE.md`.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 The canonical proof lane is missing

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-017
- Evidence Type: tests-run, code
- Evidence:
  - `./scripts/test-gate.sh proposal-029-mcp` exited 1 with `error: Unknown gate: proposal-029-mcp`.
  - `scripts/test-gate.sh:1170` usage output does not list `proposal-029-mcp`.
  - `scripts/test-gate.sh:1425` keeps `proposal-029|p029` assigned to the ACP second-wave gate.
- Why It Matters: AC-14 cannot pass and there is no deterministic acceptance lane for P029.
- Recommended Action: Add `PROPOSAL_029_MCP_TESTS`, register `proposal-029-mcp|p029-mcp`, and wire the focused auth/capability/journal/parity tests from §9.

### READY-002 Existing package tests pass but do not prove P029

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-017
- Evidence Type: tests-run
- Evidence:
  - `cargo test -p mcp-server --lib` passed 7 tests.
  - `cargo test -p graphql-server --lib` passed 5 tests.
  - `rg -n "principal_token|PROPOSAL_029_MCP|graphql_rejects|tools_list_filtered|journal_id" control-plane/crates scripts docs/reference/test-gates.md -g '!target/**'` only found the pre-existing `journal_id` local variable in `CommandHandler`.
- Why It Matters: Green baseline tests can create false confidence because they exercise the old unauthenticated shape.
- Recommended Action: Add P029 tests before claiming implementation progress; the current passing tests should be retained as regression coverage only.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | `cargo test -p mcp-server --lib` and `cargo test -p graphql-server --lib` compiled and passed package tests. Full workspace regression was not run because verdict is not successful. |
| Core user flow runtime-validated | Not Checked | Runtime auth flow cannot be validated because auth is not implemented. |
| Empty/loading/error states covered | Not Applicable | No UI flow in P029; transport error contracts are missing. |
| Accessibility risk acceptable | Not Applicable | No UI changes. |
| Localization risk acceptable | Not Applicable | Server/config proposal. |
| Critical tests executed | Fail | Named gate `proposal-029-mcp` is unregistered and exits 1. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not required for a Not Implemented / Not Ready verdict. |
| Privacy/permissions/entitlements reviewed | Fail | Command payload redaction and caller-scoped auth are missing. |

## Verification Log

- `pwd && git rev-parse --show-toplevel && git rev-parse HEAD && git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/029-mcp-northbound-control-plane-server.md`
- `nl -ba docs/proposals/029-mcp-northbound-control-plane-server.md | sed -n '1,760p'`
- `rg -n "superseded|deprecated|replaced by|obsolete|Proposal 029|029-mcp|mcp northbound|northbound control" docs/proposals docs/reference docs/archive/proposals README.md CLAUDE.md scripts/test-gate.sh .mcp.json`
- `rg -n "auth|Principal|CallerContext|caller_surface|caller_principal|caller_tool|Commanded|journal_id|principal_token|Authorization|tools/list|tools/call|resources/list|resources/read|protocolVersion|Mcp-Session-Id" control-plane/crates/mcp-server control-plane/crates/graphql-server control-plane/crates/engine control-plane/crates/domain control-plane/crates/db scripts/test-gate.sh .mcp.json CLAUDE.md`
- `rg -n "mcp_http_rejects|mcp_stdio|principal_token|resources_read_denied|command_journal_row_has_caller|payload_redacted|proposal-029-mcp|PROPOSAL_029_MCP|graphql_rejects|observer_class|tools_list_filtered|journal_id|Authorization" control-plane/crates scripts docs/reference/test-gates.md -g '!target/**'`
- `nl -ba control-plane/crates/mcp-server/src/server.rs | sed -n '1,520p'`
- `nl -ba control-plane/crates/mcp-server/src/http.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/mcp-server/src/protocol.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/graphql-server/src/server.rs | sed -n '1,220p'`
- `nl -ba control-plane/crates/graphql-server/src/schema.rs | sed -n '1,760p'`
- `nl -ba control-plane/crates/engine/src/command_handler.rs | sed -n '1,220p'`
- `nl -ba control-plane/crates/domain/src/commands.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/db/src/repos/command_journal.rs | sed -n '1,240p'`
- `find control-plane/crates/db/migrations -maxdepth 1 -type f -print | sort`
- `nl -ba scripts/test-gate.sh | sed -n '1160,1465p'`
- `nl -ba .mcp.json | sed -n '1,120p'`
- `nl -ba CLAUDE.md | sed -n '1,140p'`
- `./scripts/test-gate.sh proposal-029-mcp` -> failed with unknown gate.
- `cargo test -p mcp-server --lib` -> passed 7 tests.
- `cargo test -p graphql-server --lib` -> passed 5 tests.

## Recommended Next Actions

1. Implement shared auth crate/module and daemon principal-table loading first; do not start with surface-specific ad hoc auth.
2. Thread `Principal` through MCP HTTP, MCP stdio session state, GraphQL HTTP, and GraphQL WS, then add denial tests before implementing broader policy.
3. Add static capability tables and enforce them for `tools/list`, `tools/call`, `resources/list`, and concrete `resources/read`.
4. Migrate `command_journal`: nullable caller columns, `CallerContext`, `Commanded`, returned `journal_id`, and engine-owned redaction.
5. Update GraphQL mutation payloads and the local SwiftUI consumer in the same implementation slice.
6. Update `.mcp.json` and `CLAUDE.md` for `CHAINWORKS_MCP_TOKEN`.
7. Register and run `proposal-029-mcp`; only after that passes should full Rust workspace or canonical full regression be used for a successful readiness verdict.
