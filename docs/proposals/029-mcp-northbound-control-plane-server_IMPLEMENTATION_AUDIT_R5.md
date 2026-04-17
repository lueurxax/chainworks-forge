# Proposal 029 MCP Northbound Control-Plane Server Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/029-mcp-northbound-control-plane-server.md` |
| Repository Root | `.` |
| Git SHA | `bb3f0ef3ac562267e6cd5b5462aee5d7f01888a2` |
| Working Tree | dirty: active P029 control-plane/auth/domain/graphql/mcp/test-gate changes, proposal edits, unrelated P048/P049 deletions and reference-doc moves, prior R3/R4 audit reports untracked |
| Audited At | `2026-04-16T23:24:08+03:00` |
| Platform Scope | macOS local control-plane / daemon; no UI rewrite in P029 scope |
| Proposal State | Active draft R8 |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The current dirty tree is materially closer than R4: domain-owned typed capability/resource enums now exist, principals carry typed capability sets, MCP and GraphQL command converters exist, the P029 MCP gate passes, dogfood bearer config is present, and command `journal_id` surfacing is broadly implemented. The implementation is still **Partial** against Proposal 029 because several explicit protocol and seam contracts remain incomplete: MCP stdio does not close after first non-initialize, stdio unauthorized messages do not match the proposal, GraphQL HTTP still lacks a visible request-extension-to-`async_graphql::Context` bridge, GraphQL WS does not prove the required 4401 close behavior, daemon empty-env handling is implicit rather than deliberate, and the proof gate still does not cover the full named transport evidence inventory.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Transport/session contracts and GraphQL context bridging are still incomplete | High |
| Architecture | At Risk | Resource URI parsing moved into `auth`, weakening the proposal's transport-boundary ownership model | High |
| Product | At Risk | Valid GraphQL HTTP mutation flow may reject authorized callers because the principal bridge is not proven | High |
| UI | Not Applicable | P029 explicitly excludes UI changes | High |
| UX | At Risk | Stdio clients see non-final pre-initialize errors and non-canonical auth messages | High |
| Readiness | Not Ready | Same-tree P029 gate passes, but critical focused transport tests are absent and tree is dirty | High |

## Proposal Contract

### Scope

P029 is an active Rust control-plane delta, not a greenfield MCP server. It closes auth, principal-scoped capability policy, MCP/GraphQL northbound coexistence, command journaling, resource filtering, dogfood MCP auth migration, and a deterministic `proposal-029-mcp` proof lane.

### Locked Decisions

- `domain` remains transport-neutral and must not depend on `auth`.
- `auth` depends on `domain`; server crates depend on `auth`.
- `PrincipalClass`, `CallerSurface`, `CallerContext`, `CapabilityToolId`, and `ResourceTemplateId` are domain-owned shared types.
- `auth::Principal` carries typed tool/resource capabilities.
- MCP and GraphQL own transport-specific converters into typed capability IDs.
- `command_journal` is extended instead of creating `mcp_audit_log`.
- MCP command tools and GraphQL mutations converge on `CommandHandler` and surface `journal_id`.
- GraphQL mutations remain active in Stage A; GraphQL removal is future work.

### Primary User Flows

1. An MCP HTTP client connects with `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}`, sees only allowed tools/resources, and receives `journal_id` for command tools.
2. An MCP stdio client sends `initialize.params.clientInfo.principal_token`, binds a session principal, and cannot rebind mid-session.
3. A GraphQL HTTP client sends an authorized mutation, is checked against the same capability policy, and receives `journalId`.
4. A GraphQL WebSocket client authenticates in `connection_init.Authorization` before subscription resolvers run.
5. A first-start operator obtains the auto-bootstrapped token and uses the committed `.mcp.json` dogfood configuration.

### UI Commitments

None. P029 explicitly excludes UI rewrite and does not require UI tests.

### UX Commitments

- Dogfood `.mcp.json` moves from bare HTTP to bearer-token auth using `CHAINWORKS_MCP_TOKEN`.
- First-start bootstrap logs the generated token once and subsequent starts log only the principals-file path.
- Unauthenticated callers fail closed with deterministic MCP/GraphQL errors.

### Acceptance Criteria

The explicit acceptance criteria are P029 AC-1 through AC-15 in `docs/proposals/029-mcp-northbound-control-plane-server.md:625-645`.

### Test / Evidence Requirements

P029 requires a green same-tree `./scripts/test-gate.sh proposal-029-mcp` and a focused proof inventory covering principal bootstrap, MCP HTTP auth, MCP stdio auth/session binding, GraphQL HTTP auth/context, GraphQL WS auth/close behavior, capability policy, steward policy, command journal caller rows, redaction, `journal_id` surfacing, and cross-surface parity (`docs/proposals/029-mcp-northbound-control-plane-server.md:647-747`).

### Explicit Exclusions

P029 excludes UI rewrite, orchestration ownership changes, southbound runtime protocol replacement, high-frequency reads through MCP, token rotation/revocation/delegation, and deferred second-wave tools/resources (`docs/proposals/029-mcp-northbound-control-plane-server.md:123-146`, `749-757`).

## Proposal Fidelity / Divergence

### Matches

- `domain` no longer depends on `auth`, while `auth` depends on `domain`.
- `PrincipalClass` and `CallerContext` are domain-owned; `CallerContext` carries a typed `PrincipalClass`.
- `domain::CapabilityToolId` and `domain::ResourceTemplateId` exist and are exported.
- `auth::Principal` carries typed `BTreeSet<CapabilityToolId>` and `BTreeSet<ResourceTemplateId>`.
- `auth::filter_tools`, `auth::filter_resources`, and `auth::match_resource_uri` consume typed IDs.
- MCP tool registration crosses a `CapabilityToolId` converter.
- GraphQL mutations cross a `MutationName -> CapabilityToolId` converter.
- MCP HTTP bearer auth is wired before request handling.
- MCP tool/resource filtering now uses principal capabilities.
- Agent class no longer receives steward tools or `steward-analysis://` resource access.
- Observer receives read-only steward access and read resources.
- Command journal caller columns and payload redaction are wired through `CommandHandler`.
- MCP command tools and GraphQL mutation payloads surface `journal_id` / `journalId`.
- `.mcp.json` and `CLAUDE.md` document/use `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}`.
- `./scripts/test-gate.sh proposal-029-mcp` passed on this audited tree.

### Divergences

- `CapabilityToolId` and `ResourceTemplateId` are not marked `#[non_exhaustive]` as shown in the proposal's canonical domain additions.
- Resource URI shape knowledge and template matching live in `auth` instead of being owned by `mcp-server` and passed into `auth::match_resource_uri` through a parser/closure boundary.
- MCP stdio first non-`initialize` returns `-32002`, but continues the loop instead of closing the session.
- MCP stdio missing-token and unknown-token messages differ from the proposal's exact strings.
- GraphQL HTTP middleware inserts a principal into axum request extensions, but the mounted `GraphQL::new(schema.clone())` service does not visibly bridge that extension into `async_graphql::Context`.
- GraphQL WS uses `on_connection_init` and returns `async_graphql::Error`, but no direct 4401 close-frame implementation or test evidence was found.
- `CHAINWORKS_AUTH_PRINCIPALS_PATH=""` is not handled by an explicit daemon branch with a tailored fail-closed error.
- The P029 gate runs four typed focused tests plus `cargo test --workspace`, but does not include the full named transport proof inventory from the proposal.

### Ambiguities / Evidence Gaps

- It is possible `async_graphql_axum` has implicit request-data behavior not obvious from the code, but no route-level test proves that an HTTP `Authorization` header reaches mutation `ctx.data::<auth::Principal>()`.
- The WS `connection_error` versus close-code 4401 behavior was not runtime-validated.
- The bootstrap "token logged exactly once" behavior is supported by branch shape, but not asserted by a focused test.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 9 |
| Partially Implemented | 7 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Domain/auth dependency and caller type ownership

- Proposal Source: §4.0 type ownership and dependency graph; §4.3 caller context (`docs/proposals/029-mcp-northbound-control-plane-server.md:154-180`, `385-428`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/Cargo.toml`
  - `control-plane/crates/auth/Cargo.toml`
  - `control-plane/crates/domain/src/commands.rs`
  - `control-plane/crates/domain/src/lib.rs:1-17`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: No dependency inversion gap found in the inspected tree.

### REQ-002 Typed capability/resource identifiers and Principal capability sets

- Proposal Source: §4.0 canonical domain additions and auth API (`docs/proposals/029-mcp-northbound-control-plane-server.md:170-231`).
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/capabilities.rs:3-32`
  - `control-plane/crates/domain/src/lib.rs:1-17`
  - `control-plane/crates/auth/src/lib.rs:11-31`
  - `control-plane/crates/auth/src/lib.rs:179-194`
  - `control-plane/crates/auth/src/lib.rs:204-263`
  - `control-plane/crates/auth/src/lib.rs:371-412`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: The typed model and principal sets exist, but the enums are not marked `#[non_exhaustive]` as the proposal specified.

### REQ-003 Server-side capability/resource converters

- Proposal Source: §4.0 server-side converters (`docs/proposals/029-mcp-northbound-control-plane-server.md:233-241`).
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/mod.rs:11-64`
  - `control-plane/crates/mcp-server/src/tools/mod.rs:70-85`
  - `control-plane/crates/graphql-server/src/schema.rs:203-225`
  - `control-plane/crates/graphql-server/src/schema.rs:724-745`
  - `control-plane/crates/auth/src/lib.rs:288-310`
  - `control-plane/crates/auth/src/lib.rs:332-363`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: MCP and GraphQL tool/mutation converters are present. Resource URI parsing/template matching is still implemented inside `auth`, not owned by `mcp-server` as the proposal's boundary model requires.

### REQ-004 Principal table bootstrap and fail-closed loading

- Proposal Source: §4.1 token material loading and AC-15 (`docs/proposals/029-mcp-northbound-control-plane-server.md:247-256`, `642-643`).
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:108-119`
  - `control-plane/crates/auth/src/lib.rs:83-148`
  - `control-plane/crates/auth/src/lib.rs:116-131`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: Missing/unparseable/zero-principal files fail closed and bootstrap uses Unix `0600` mode. The daemon does not explicitly reject an empty `CHAINWORKS_AUTH_PRINCIPALS_PATH` before converting it into `PathBuf`, and no focused bootstrap test for empty env or exact once-only logging was found.

### REQ-005 MCP HTTP bearer auth

- Proposal Source: §4.1.a and AC-1 (`docs/proposals/029-mcp-northbound-control-plane-server.md:258-265`, `629`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/http.rs:46-80`
  - `control-plane/crates/mcp-server/src/http.rs:82-119`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: Source-level MCP HTTP fail-closed behavior is implemented. Coverage precision is tracked under REQ-016.

### REQ-006 MCP stdio initialize auth and session binding

- Proposal Source: §4.1.b and AC-2/AC-3 (`docs/proposals/029-mcp-northbound-control-plane-server.md:267-288`, `629-631`).
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:73-132`
  - `control-plane/crates/mcp-server/src/server.rs:135-148`
  - `control-plane/crates/daemon/tests/mcp_stdio.rs`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: Session binding and second-initialize rejection exist. First non-`initialize` returns `-32002`, but continues instead of closing stdin/session. Missing-token text is `"unauthorized: missing principal_token in clientInfo"` rather than `"unauthorized: principal_token required on initialize"`, and unknown-token text is generic `"unauthorized"` rather than `"unauthorized: unknown token"`.

### REQ-007 GraphQL HTTP auth and mutation principal checks

- Proposal Source: §4.1.c GraphQL HTTP and AC-4/AC-7/AC-9 (`docs/proposals/029-mcp-northbound-control-plane-server.md:290-317`, `632`, `635`, `637`).
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/auth_layer.rs:15-66`
  - `control-plane/crates/graphql-server/src/server.rs:91-106`
  - `control-plane/crates/graphql-server/src/schema.rs:223-225`
  - `control-plane/crates/graphql-server/src/schema.rs:324-333`
  - `control-plane/crates/graphql-server/src/schema.rs:382-391`
  - `control-plane/crates/graphql-server/src/schema.rs:476-485`
  - Existing schema tests inject `.data(test_principal())` directly, for example `control-plane/crates/graphql-server/src/schema.rs:1005-1033`.
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: Middleware rejects missing/invalid HTTP bearer auth and mutation resolvers enforce capability checks if a principal is in GraphQL context. The explicit bridge from axum request extensions into `async_graphql::Context` is not visible in `server.rs`, where `/graphql` still mounts `GraphQL::new(schema.clone())`.

### REQ-008 GraphQL WebSocket connection_init auth

- Proposal Source: §4.1.c subscription path and §9 GraphQL WS tests (`docs/proposals/029-mcp-northbound-control-plane-server.md:318-331`, `687-694`).
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/server.rs:22-69`
  - `control-plane/crates/graphql-server/src/schema.rs:539-542`
  - `control-plane/crates/graphql-server/src/schema.rs:577-579`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: `connection_init.Authorization` is parsed and successful auth injects `Principal` into subscription data. No code or test directly proves the proposal's required `{ "message": "unauthorized" }` connection error plus WebSocket close code 4401.

### REQ-009 Per-principal MCP tool policy

- Proposal Source: §4.2 class policy and AC-5/AC-12 (`docs/proposals/029-mcp-northbound-control-plane-server.md:334-355`, `633`, `640`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/auth/src/lib.rs:204-245`
  - `control-plane/crates/mcp-server/src/server.rs:187-226`
  - `control-plane/crates/mcp-server/src/server.rs:295-300`
  - `control-plane/crates/auth/src/lib.rs:383-412`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: No class-policy gap found for the current first-wave plus steward tools.

### REQ-010 Per-principal MCP resource policy and concrete URI checks

- Proposal Source: §4.2 resource policy and AC-6/AC-13 (`docs/proposals/029-mcp-northbound-control-plane-server.md:334-355`, `634`, `641`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/auth/src/lib.rs:249-310`
  - `control-plane/crates/auth/src/lib.rs:332-363`
  - `control-plane/crates/mcp-server/src/server.rs:244-278`
  - `control-plane/crates/mcp-server/src/server.rs:520-583`
  - `control-plane/crates/auth/src/lib.rs:383-448`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: Runtime behavior is implemented. The ownership-boundary issue is counted under REQ-003 / ARCH-001.

### REQ-011 Command journal caller metadata and fail-closed insert

- Proposal Source: §4.3 command journaling and AC-8 (`docs/proposals/029-mcp-northbound-control-plane-server.md:357-463`, `636`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/011_auth_tracking.sql:1-5`
  - `control-plane/crates/db/src/repos/command_journal.rs:9-37`
  - `control-plane/crates/engine/src/command_handler.rs:77-147`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: `CommandHandler::handle` inserts caller metadata before execution and fails closed on insert failure.

### REQ-012 Command journal payload redaction

- Proposal Source: §4.3 payload redaction and §9 command journal tests (`docs/proposals/029-mcp-northbound-control-plane-server.md:430-463`, `706-710`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_journal_redact.rs:8-39`
  - `control-plane/crates/engine/src/command_journal_redact.rs:47-68`
  - `control-plane/crates/engine/src/command_handler.rs:89-90`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: Approval/rejection comments are redacted before journal insert.

### REQ-013 MCP command tools return `journal_id`

- Proposal Source: §4.4 MCP command tool response and AC-10 (`docs/proposals/029-mcp-northbound-control-plane-server.md:465-494`, `638`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/runs.rs:117-141`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:164-170`
  - `control-plane/crates/mcp-server/src/tools/approvals.rs:85-89`
  - `control-plane/crates/mcp-server/src/tools/stages.rs:43-49`
  - `control-plane/crates/mcp-server/src/tools/steward.rs:57-74`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: No source-level gap found.

### REQ-014 GraphQL command mutations return `journalId`

- Proposal Source: §4.5 GraphQL mutation payloads and AC-11 (`docs/proposals/029-mcp-northbound-control-plane-server.md:496-518`, `639`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:262-304`
  - `control-plane/crates/graphql-server/src/schema.rs:350-367`
  - `control-plane/crates/graphql-server/src/schema.rs:403-417`
  - `control-plane/crates/graphql-server/src/schema.rs:451-465`
  - `control-plane/crates/graphql-server/src/schema.rs:496-525`
  - `control-plane/crates/graphql-server/src/schema.rs:1021-1022`
  - `control-plane/crates/graphql-server/src/schema.rs:1076-1083`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: Schema-level GraphQL payloads include `journalId`; route-level auth bridging is counted separately under REQ-007.

### REQ-015 Dogfood MCP auth migration

- Proposal Source: §7.1 dogfood config and AC-15 (`docs/proposals/029-mcp-northbound-control-plane-server.md:593-619`, `642-643`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `.mcp.json:1-11`
  - `CLAUDE.md:47-59`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: `.mcp.json` includes `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}`, and `CLAUDE.md` documents the generated principals file and env var.

### REQ-016 Canonical `proposal-029-mcp` proof lane

- Proposal Source: §9 test inventory, §10 acceptance criteria, §11.3 test gate update (`docs/proposals/029-mcp-northbound-control-plane-server.md:647-747`).
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `scripts/test-gate.sh:1445-1455`
  - `docs/reference/test-gates.md:570-590`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on this audited tree.
- Gap / Note: The gate exists and is green, but it does not run the named focused transport tests listed in the proposal. Searches found no explicit `test_mcp_http_rejects_*`, `test_mcp_stdio_rejects_*`, `test_graphql_rejects_*`, `test_graphql_ws_*`, `test_command_journal_row_*`, or cross-surface parity tests matching the proposal inventory.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Resource URI parsing is owned by the wrong boundary

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-003, REQ-010; §4.0 server-side converters
- Evidence Type: code
- Evidence:
  - `control-plane/crates/auth/src/lib.rs:288-310`
  - `control-plane/crates/auth/src/lib.rs:332-363`
  - `control-plane/crates/mcp-server/src/server.rs:244-278`
- Why It Matters: P029 separates transport-specific parsing from transport-neutral authorization. Keeping URI-shape matching inside `auth` couples the auth crate to MCP URI syntax and weakens the intended compile-time drift boundary.
- Recommended Action: Move concrete URI to `ResourceTemplateId` parsing/template ownership to `mcp-server`, and have `auth` evaluate allowed typed IDs through a parser/closure or equivalent seam.

### ARCH-002 GraphQL HTTP principal bridge remains a route-seam risk

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-007
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/graphql-server/src/auth_layer.rs:40-43`
  - `control-plane/crates/graphql-server/src/server.rs:91-106`
  - `control-plane/crates/graphql-server/src/schema.rs:324-330`
  - `control-plane/crates/graphql-server/src/schema.rs:1005-1033`
- Why It Matters: The middleware stores `Principal` in axum request extensions, but resolvers read from `async_graphql::Context`. Existing tests bypass the HTTP route by injecting `.data(test_principal())`, so a valid authorized HTTP mutation can still fail at the route seam.
- Recommended Action: Add the explicit request-to-GraphQL-data bridge described in P029 and prove it with route-level tests for valid operator, observer-forbidden mutation, missing header, and unknown token.

### ARCH-003 Typed capability enums are not future-proofed as specified

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-002
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/capabilities.rs:3-32`
- Why It Matters: The proposal showed these shared enums as `#[non_exhaustive]`, reducing downstream assumptions as the MCP surface expands. The current enums are typed, but not hardened to that contract.
- Recommended Action: Add `#[non_exhaustive]` or explicitly update the proposal if exhaustive matching inside this workspace is now the intended contract.

## Product Review

**Summary:** At Risk

### PROD-001 The GraphQL HTTP happy path is not proven end-to-end

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-007, REQ-014
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/graphql-server/src/server.rs:91-106`
  - `control-plane/crates/graphql-server/src/schema.rs:324-333`
  - `control-plane/crates/graphql-server/src/schema.rs:350-367`
  - `control-plane/crates/graphql-server/src/schema.rs:1005-1033`
- Why It Matters: One primary P029 job is an authorized GraphQL client invoking command mutations and receiving `journalId`. Schema-level tests prove the resolver when the principal is manually injected, but not the product path over HTTP.
- Recommended Action: Add route-level GraphQL integration coverage that sends `Authorization: Bearer <token>` and verifies both mutation success plus `journalId` and forbidden observer behavior without writing journal rows.

## UI Review

**Summary:** Not Applicable

### UI-001 P029 has no UI surface

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: Explicit exclusions
- Evidence Type: code
- Evidence:
  - `docs/proposals/029-mcp-northbound-control-plane-server.md:123-128`
  - `docs/proposals/029-mcp-northbound-control-plane-server.md:749-757`
- Why It Matters: Requiring UI implementation or UI tests from P029 would be scope creep.
- Recommended Action: Keep UI testing out of this proposal's readiness gate unless a later proposal adds an operator-facing UI requirement.

## UX Review

**Summary:** At Risk

### UX-001 MCP stdio pre-initialize recovery remains ambiguous for clients

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-006
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:108-129`
  - `control-plane/crates/mcp-server/src/server.rs:135-148`
- Why It Matters: P029's stdio failure contract is intentionally deterministic. Continuing after a first non-`initialize` request leaves clients in a recoverable-but-invalid session state instead of forcing a clean reconnect, and non-canonical messages complicate client-side diagnostics.
- Recommended Action: On first non-`initialize`, emit `-32002 "server not initialized"` and terminate the stdio session. Align missing/unknown token messages with the proposal exactly and add stdio process tests.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Green gate does not yet prove the full P029 acceptance inventory

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-016
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `scripts/test-gate.sh:1445-1455`
  - `docs/reference/test-gates.md:570-590`
  - `./scripts/test-gate.sh proposal-029-mcp` passed.
- Why It Matters: The current gate is useful and green, but broad `cargo test --workspace` plus four converter tests can pass while route-level GraphQL auth, WS close code, exact stdio close/messages, and journal-row negative cases remain unproven.
- Recommended Action: Extend `proposal-029-mcp` with the named focused transport/auth/journal tests from §9, then rerun it on the same tree.

### READY-002 Audit target is a dirty working tree with unrelated proposal churn

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Release / handoff readiness
- Evidence Type: code
- Evidence:
  - `git status --short` shows active P029 source changes plus unrelated P048/P049 deletions/reference moves and untracked audit files.
- Why It Matters: Even if P029 implementation gaps were closed, unrelated deletions and dirty proposal docs make the handoff unsafe and hard to reproduce.
- Recommended Action: Isolate P029 implementation, restore or separately land unrelated P048/P049 moves, and rerun the P029 gate on the final intended diff.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `./scripts/test-gate.sh proposal-029-mcp` ran `cargo test --workspace` and passed. |
| Core user flow runtime-validated | Partial | MCP/GraphQL behavior proven mostly by code/tests, not live route-level runtime validation. |
| Empty/loading/error states covered | Partial | MCP HTTP errors exist; stdio exact close/messages and GraphQL WS close code remain partial. |
| Accessibility risk acceptable | Not Applicable | No UI scope. |
| Localization risk acceptable | Pass | Developer/operator protocol messages only; no user-facing UI strings in scope. |
| Critical tests executed | Partial | P029 gate passed, but focused transport tests from §9 are missing. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass for Rust control-plane slice | `./scripts/test-gate.sh proposal-029-mcp` passed on `bb3f0ef3ac562267e6cd5b5462aee5d7f01888a2`; not a full Swift app gate. |
| Privacy/permissions/entitlements reviewed | Partial | Token file uses `0600`; no UI/entitlements changes. Empty env and once-only log assertions are not focused-tested. |

## Verification Log

- `sed -n '1,260p' /Users/user/.agents/skills/proposal-implementation-audit/SKILL.md`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/029-mcp-northbound-control-plane-server.md`
- `git rev-parse HEAD`
- `git status --short`
- `date -Iseconds`
- `rg -n "superseded|deprecated|replaced|obsolete|replaced by" docs/proposals docs/reference docs/reviews`
- `nl -ba docs/proposals/029-mcp-northbound-control-plane-server.md`
- `nl -ba control-plane/crates/domain/src/capabilities.rs`
- `nl -ba control-plane/crates/auth/src/lib.rs`
- `nl -ba control-plane/crates/mcp-server/src/server.rs`
- `nl -ba control-plane/crates/mcp-server/src/http.rs`
- `nl -ba control-plane/crates/mcp-server/src/tools/mod.rs`
- `nl -ba control-plane/crates/mcp-server/src/tools/runs.rs`
- `nl -ba control-plane/crates/mcp-server/src/tools/approvals.rs`
- `nl -ba control-plane/crates/mcp-server/src/tools/stages.rs`
- `nl -ba control-plane/crates/mcp-server/src/tools/steward.rs`
- `nl -ba control-plane/crates/graphql-server/src/auth_layer.rs`
- `nl -ba control-plane/crates/graphql-server/src/server.rs`
- `nl -ba control-plane/crates/graphql-server/src/schema.rs`
- `nl -ba control-plane/crates/daemon/src/main.rs`
- `nl -ba scripts/test-gate.sh`
- `nl -ba docs/reference/test-gates.md`
- `nl -ba .mcp.json`
- `nl -ba CLAUDE.md`
- `rg -n "CapabilityToolId|ResourceTemplateId|match_resource_uri|filter_resources|GraphQL::new|on_connection_init|principal_token|required|unknown token|server not initialized|PROPOSAL_029_TESTS|proposal-029-mcp" control-plane scripts docs/reference/test-gates.md`
- `rg -n "test_principals_file_created|test_principals_bootstrap|test_principals_daemon|test_mcp_http_rejects|test_mcp_stdio_rejects|test_graphql_rejects|test_graphql_ws|test_mcp_tools_list_filtered|test_mcp_resources|test_command_journal_row|test_mcp_steward_run_analysis_response|test_graphql_start_run_started|test_graphql_and_mcp" control-plane scripts docs/reference/test-gates.md`
- `./scripts/test-gate.sh proposal-029-mcp`

## Recommended Next Actions

1. Fix MCP stdio to close after first non-`initialize` and align missing/unknown token error strings with the proposal.
2. Implement and route-test the GraphQL HTTP extension-to-GraphQL-context principal bridge.
3. Prove GraphQL WS unauthorized behavior with connection error and close code 4401, or update the proposal if async-graphql's close semantics differ.
4. Move resource URI parser/template ownership out of `auth` or revise P029's ownership model explicitly.
5. Add the named focused P029 transport/auth/journal tests to `proposal-029-mcp`.
6. Isolate the P029 diff from unrelated P048/P049 proposal churn before final implementation sign-off.
