# P029 Implementation Audit R2 - MCP Northbound Control Plane Server

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/029-mcp-northbound-control-plane-server.md` |
| Audit report | `docs/proposals/029-mcp-northbound-control-plane-server_IMPLEMENTATION_AUDIT_R2.md` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Audit timestamp | `2026-04-16T15:06:28+03:00` |
| Worktree state | Dirty, mixed proposal/control-plane worktree with unrelated P048/P049/P047/P029 artifacts present |
| Canonical gate run | `./scripts/test-gate.sh proposal-029-mcp` |
| Gate result | Passed |
| Verdict | **Partial / Not Ready** |

## Executive Verdict

P029 is not ready to merge as implemented. The control-plane has a substantial P029 scaffold: a new `auth` crate, MCP HTTP bearer checks, MCP stdio session binding, `CallerContext::test_fixture()`, command journal caller columns, mutation payload wrappers with `journalId`, and the canonical `proposal-029-mcp` gate all exist. However, the implementation misses several core acceptance requirements in security-sensitive paths.

The largest blocker is GraphQL auth. `graphql_server::auth_layer::require_auth` exists, but the server never mounts it, the daemon never passes a `PrincipalTable` into GraphQL, and mutation resolvers default a missing principal to an operator. As a result, the proposal's GraphQL bearer-auth and forbidden-mutation contracts are not enforced by the running daemon.

The focused gate passed, but that is not sufficient evidence: it does not cover the live GraphQL HTTP/WS auth path, daemon principal bootstrap path/permissions, or steward capability/resource exposure. This is a gate coverage gap, not a readiness signal.

## Lens Scorecard

| Lens | Status | Notes |
| --- | --- | --- |
| Proposal fidelity | Fail | GraphQL auth, principal bootstrap, steward capability/resource access, and typed capability ownership diverge from the proposal. |
| Architecture | Partial | Core crates exist, but type ownership remains stringly typed and split incorrectly between `auth` and `domain`; GraphQL auth is disconnected. |
| Product behavior | Fail | Unauthenticated GraphQL mutations can execute as operator-class callers; operator cannot access registered steward MCP tools/resources. |
| UX / operator handoff | Partial | `.mcp.json` and `CLAUDE.md` document bearer usage, but daemon bootstrap behavior does not match the documented `~/.chainworks/auth/principals.json` path. |
| Readiness / verification | Fail | `proposal-029-mcp` passed, but it misses acceptance-critical runtime paths. No Ready verdict is defensible. |

## Proposal Contract Summary

P029 requires the northbound daemon surface to add and prove:

1. Principal resolution for MCP HTTP, MCP stdio, GraphQL HTTP, and GraphQL WebSocket.
2. Caller-scoped capability filtering for MCP tools, MCP resources, and GraphQL mutations.
3. Per-command audit journaling with caller surface/principal/tool fields.
4. `Commanded { result, journal_id }` propagation to MCP command tools and GraphQL mutation payload wrappers.
5. GraphQL coexistence/cutover without breaking blocked `startRun` responses.
6. Dogfood MCP auth through `CHAINWORKS_MCP_TOKEN`.
7. Principal bootstrap at `~/.chainworks/auth/principals.json` with fail-closed invalid table handling and 0600 file permissions.
8. A green `./scripts/test-gate.sh proposal-029-mcp` gate that actually covers the above acceptance surface.

## Implementation Evidence

### Implemented or mostly implemented

| Contract area | Evidence | Status |
| --- | --- | --- |
| Auth crate added | `control-plane/Cargo.toml:6`; `control-plane/crates/auth/src/lib.rs:1-374` | Partial |
| MCP HTTP bearer auth | `control-plane/crates/mcp-server/src/http.rs:46-80` | Partial |
| MCP stdio session binding | `control-plane/crates/mcp-server/src/server.rs:50-156` | Partial |
| Stdio reinitialize rejection | `control-plane/crates/mcp-server/src/server.rs:80-93` | Implemented |
| Command caller context | `control-plane/crates/domain/src/commands.rs:64-121` | Partial |
| Public cross-crate test fixture | `control-plane/crates/domain/src/commands.rs:109-120` | Implemented |
| Journal caller columns | `control-plane/crates/db/migrations/011_auth_tracking.sql:1-5`; `control-plane/crates/db/src/repos/command_journal.rs:9-38` | Partial |
| Commanded wrapper | `control-plane/crates/engine/src/command_handler.rs:46-51`; `control-plane/crates/engine/src/command_handler.rs:77-139` | Partial |
| Journal payload redaction hook | `control-plane/crates/engine/src/command_journal_redact.rs:1-39` | Implemented for approval/rejection comments |
| MCP command tools return journal IDs | `control-plane/crates/mcp-server/src/tools/runs.rs:118-143`; `control-plane/crates/mcp-server/src/tools/approvals.rs:88-92`; `control-plane/crates/mcp-server/src/tools/stages.rs:43-50` | Partial |
| GraphQL mutation payload wrappers | `control-plane/crates/graphql-server/src/schema.rs:235-279` | Implemented |
| GraphQL `startRun` preserves blocked lane with journal ID | `control-plane/crates/graphql-server/src/schema.rs:328-345` | Implemented |
| Dogfood `.mcp.json` header | `.mcp.json:1-10` | Implemented |

### Blocking gaps

#### Finding P1-001: GraphQL auth middleware is not mounted; missing principal defaults to operator

The proposal requires GraphQL HTTP bearer auth, a playground exemption only under `CHAINWORKS_PLAYGROUND_AUTH=skip`, WS authentication via `connection_init.Authorization`, and no journal row for forbidden mutations. The implementation has an `auth_layer::require_auth`, but `graphql_server::server::start_with_extra_routes` builds a router with raw `GraphQL::new(schema)` and `GraphQLSubscription::new(schema)` without applying the middleware or attaching a `PrincipalTable`. The daemon also calls `build_schema(pool, cmd_handler, events)` without auth data. Inside the mutation resolvers, missing `auth::Principal` resolves to `anonymous` with `PrincipalClass::Operator`, which permits all command mutations.

Evidence:

- `control-plane/crates/daemon/src/main.rs:108-139`
- `control-plane/crates/graphql-server/src/server.rs:25-35`
- `control-plane/crates/graphql-server/src/auth_layer.rs:11-50`
- `control-plane/crates/graphql-server/src/schema.rs:298-307`
- `control-plane/crates/graphql-server/src/schema.rs:360-369`
- `control-plane/crates/graphql-server/src/schema.rs:462-471`
- `control-plane/crates/graphql-server/src/schema.rs:496-505`

Impact:

- Unauthenticated GraphQL HTTP callers can execute mutations as operator-class callers.
- WS auth fallback described by the proposal is not implemented.
- Forbidden mutation semantics and the "no journal row" guarantee are not proved on the live route.
- Acceptance criteria 4, 7, and 9 are not satisfied.

Required fix:

- Wire `PrincipalTable` into GraphQL server state.
- Mount auth middleware on `/graphql` POST while allowing playground GET only under `CHAINWORKS_PLAYGROUND_AUTH=skip`.
- Configure `/graphql/ws` to authenticate in `connection_init.Authorization` and reject unauthenticated connections.
- Remove the operator fallback from production mutation resolvers; tests should inject an explicit principal or use a test schema helper.

#### Finding P1-002: Principal bootstrap path and permissions do not match the security contract

The proposal requires bootstrap at `~/.chainworks/auth/principals.json`, mode 0600, fail-closed zero/unparseable table handling, and token logging exactly once. The daemon currently uses a relative `principals.json` in the current working directory. The auth crate writes the file with `std::fs::write`, which does not enforce 0600 permissions on Unix. `CLAUDE.md` tells operators the daemon writes `~/.chainworks/auth/principals.json`, but the running code does not.

Evidence:

- `control-plane/crates/daemon/src/main.rs:108-111`
- `control-plane/crates/auth/src/lib.rs:77-124`
- `CLAUDE.md:59`

Impact:

- Operators following docs may not find the real token file.
- A process started from different working directories can create different principal tables.
- File permissions are inherited from umask rather than the proposal's explicit 0600 requirement.
- Acceptance criterion 15 is not satisfied.

Required fix:

- Resolve the default principal table path to `$HOME/.chainworks/auth/principals.json`, with an explicit override if desired.
- Create parent directories and the file with owner-only permissions on Unix.
- Add tests for default path resolution, permissions, invalid JSON fail-closed behavior, and zero-principal fail-closed behavior.

#### Finding P1-003: Registered steward MCP tools and resource are filtered out for every principal

P029's current baseline includes steward tools and the steward-analysis resource surface. The MCP server registers `steward.run_analysis`, `steward.list_analyses`, `steward.get_analysis`, and `steward-analysis://{analysis_id}`, but `auth::allowed_tools_for_class` omits all steward tools and `auth::allowed_resource_templates` omits `steward-analysis://`. Because `tools/list`, `tools/call`, `resources/list`, and `resources/read` all enforce these string allow-lists, even an operator cannot discover or call steward tools or read steward analysis resources through the capability-guarded MCP surface.

Evidence:

- `control-plane/crates/mcp-server/src/server.rs:27-33`
- `control-plane/crates/mcp-server/src/tools/steward.rs:10-47`
- `control-plane/crates/auth/src/lib.rs:178-208`
- `control-plane/crates/mcp-server/src/server.rs:194-208`
- `control-plane/crates/mcp-server/src/server.rs:220-226`
- `control-plane/crates/mcp-server/src/server.rs:275-280`
- `control-plane/crates/auth/src/lib.rs:212-243`
- `control-plane/crates/mcp-server/src/server.rs:314-321`
- `control-plane/crates/mcp-server/src/server.rs:341-344`

Impact:

- The proposal's stated active steward MCP surface is unusable through normal capability checks.
- Operator parity with registered MCP tools is broken.
- Acceptance criteria 5, 12, and 13 are not satisfied for the steward surface.

Required fix:

- Add steward tool IDs to the operator allow-list according to the proposal's capability matrix.
- Add steward read tools/resources to allowed classes as specified by P029.
- Add tests that `tools/list` and `resources/list` expose steward entries for the intended principal class and that unauthorized classes are denied.

#### Finding P2-004: Capability type ownership remains stringly typed and in the wrong crate

The proposal explicitly moves `PrincipalClass`, `CapabilityToolId`, and `ResourceTemplateId` into `domain` as stable shared contract types. Current implementation keeps `PrincipalClass` in `auth`, stores `CallerContext.principal_class` and `caller_tool` as strings, keeps `auth::ToolSpec`, and does not define `CapabilityToolId` or `ResourceTemplateId` in `domain`. This preserves the earlier scaffold's string allow-list model instead of the proposal's typed contract.

Evidence:

- `control-plane/crates/auth/src/lib.rs:12-18`
- `control-plane/crates/auth/src/lib.rs:156-176`
- `control-plane/crates/domain/src/commands.rs:82-88`
- `control-plane/crates/domain/src/lib.rs:1-12`

Impact:

- Capability policy can drift from registered tools/resources without compiler help.
- Cross-crate GraphQL/MCP policy mappings remain string-based.
- The proposal's type ownership contract is not implemented.

Required fix:

- Move `PrincipalClass` to `domain` or re-export it from domain as the canonical type.
- Add typed `CapabilityToolId` and `ResourceTemplateId` enums in `domain`.
- Convert MCP tool specs and GraphQL mutation policy to typed IDs before applying auth filters.

#### Finding P2-005: Journal writes are best-effort while the API returns durable journal IDs

`CommandHandler::handle` generates a `journal_id`, attempts to record it, and then ignores insert/complete/fail errors with `let _ = ...`. The API then returns `Commanded { journal_id }` even if the journal row was never created or closed. P029 defines journal ID as an audit pointer for command tools and GraphQL mutation payloads, so this must be durable or fail closed.

Evidence:

- `control-plane/crates/engine/src/command_handler.rs:77-113`
- `control-plane/crates/engine/src/command_handler.rs:117-138`
- `control-plane/crates/db/src/repos/command_journal.rs:9-38`

Impact:

- Clients can receive a `journal_id` that does not resolve to a durable journal row.
- Audit completeness depends on silent DB success, not on command semantics.
- Acceptance criteria 8, 9, 10, and 11 are only partially satisfied.

Required fix:

- Make the initial journal insert mandatory before command execution.
- Fail the command if the audit row cannot be created.
- Treat completion/failure update errors as explicit errors or durable repair work, not silent drops.
- Add a failure-injection test proving no command executes without a journal row.

#### Finding P2-006: The canonical P029 gate passes without testing several acceptance-critical paths

`./scripts/test-gate.sh proposal-029-mcp` passed, but source audit shows it does not catch missing GraphQL route auth, missing WS `connection_init` auth, wrong principal bootstrap path/permissions, or steward capability/resource filtering. The script's P029 MCP array targets crate-level `auth::tests`, `mcp_server::tests`, and `graphql_server::tests`, but the observed GraphQL tests are schema tests that do not exercise the axum route/middleware path.

Evidence:

- `scripts/test-gate.sh:190-194`
- `docs/reference/test-gates.md:570-586`
- Gate output: `==> Proposal 029-MCP control-plane gate passed`
- `control-plane/crates/graphql-server/src/server.rs:25-35`
- `control-plane/crates/graphql-server/src/schema.rs:298-307`

Impact:

- The gate gives a false-positive readiness signal for a security-sensitive proposal.
- A green P029 gate cannot currently be used as release evidence.

Required fix:

- Add route-level GraphQL HTTP auth tests for missing, malformed, unknown, observer, agent, and operator credentials.
- Add WS `connection_init.Authorization` tests.
- Add daemon/bootstrap tests for path and permissions.
- Add MCP capability tests covering steward tool/resource exposure and denial.

## Requirement Audit

| ID | Requirement | Status | Evidence / Gap |
| --- | --- | --- | --- |
| REQ-001 | MCP HTTP missing/unresolvable bearer rejected with `-32000` | Partial | Implemented in `mcp-server/src/http.rs:46-80`; tests are not route-comprehensive. |
| REQ-002 | MCP stdio requires initialize with `principal_token` | Partial | Implemented in `mcp-server/src/server.rs:80-156`; initialization path has a daemon stdio smoke test. |
| REQ-003 | MCP stdio rejects reinitialize mid-session | Implemented | `mcp-server/src/server.rs:80-93`. |
| REQ-004 | GraphQL HTTP bearer auth with env-only playground exemption | Fail | Middleware exists but is not mounted; route is unauthenticated. |
| REQ-005 | `tools/list` filtered per principal | Partial | Basic filtering exists, but steward tools are omitted from policy. |
| REQ-006 | `tools/call` outside capability returns `-32601` | Partial | Guard exists, but policy data is incomplete and stringly typed. |
| REQ-007 | GraphQL forbidden mutation returns GraphQL error and no journal row | Fail | Missing principal falls back to operator; no live route auth. |
| REQ-008 | MCP command tools journal caller rows | Partial | Caller values are passed, but journal writes are ignored on failure. |
| REQ-009 | GraphQL mutations journal caller rows | Fail | Mutation calls can be unauthenticated operator calls; GraphQL route auth not wired. |
| REQ-010 | Payload redaction via shared helper | Partial | Helper exists and redacts approval/rejection comments only. |
| REQ-011 | `Commanded { result, journal_id }`; command surfaces expose journal IDs | Partial | Implemented on main command tools/mutations, but IDs can be non-durable if journal insert fails; `steward.run_analysis` omits `journal_id` in its MCP response. |
| REQ-012 | `resources/list` capability-filtered | Partial | Filter exists, but steward-analysis resource is filtered out for all classes. |
| REQ-013 | `resources/read` concrete URI guard including steward-analysis | Fail | Steward read implementation exists, but auth templates omit the steward URI. |
| REQ-014 | `proposal-029-mcp` gate green | Pass with caveat | Gate passed but is under-scoped relative to acceptance criteria. |
| REQ-015 | Principal bootstrap writes `~/.chainworks/auth/principals.json` with 0600 and fail-closed invalid table | Fail | Daemon uses cwd-relative `principals.json`; auth writer does not enforce 0600. |

## Verification Log

| Command | Result | Notes |
| --- | --- | --- |
| `./scripts/test-gate.sh proposal-029-mcp` | Passed | Completed Rust control-plane crate/doc tests and printed `Proposal 029-MCP control-plane gate passed`. This did not cover the blocking live GraphQL/daemon/steward gaps above. |

No full Swift/macOS UI gate was run. P029 is a Rust control-plane northbound/auth proposal, and the implementation is already blocked by source-level security contract gaps; a broader UI gate would not address the failing acceptance criteria.

## Readiness Checklist

| Check | Status |
| --- | --- |
| Proposal requirements mapped to implementation | Complete |
| Security-sensitive auth paths audited | Complete |
| Canonical focused gate run | Complete, passed |
| Gate coverage evaluated against acceptance criteria | Complete, insufficient |
| Implementation ready for merge | No |
| Needs follow-up implementation | Yes |

## Recommended Next Actions

1. Wire GraphQL auth end-to-end before any other P029 readiness work.
2. Fix daemon principal bootstrap path and 0600 creation semantics.
3. Complete typed capability ownership in `domain` and update allow-lists for steward tools/resources.
4. Make command journal insert mandatory before command execution.
5. Expand `proposal-029-mcp` to include route-level GraphQL HTTP/WS auth, daemon bootstrap, and steward capability/resource tests.
6. Re-run `./scripts/test-gate.sh proposal-029-mcp` after fixes; only then consider a broader control-plane regression gate.
