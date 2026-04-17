# Proposal 029 MCP Northbound Control-Plane Server Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/029-mcp-northbound-control-plane-server.md` |
| Repository Root | `.` |
| Git SHA | `345956b854358ad12a478867469aee6025d8a7c0` |
| Working Tree | dirty: P029, control-plane auth/domain/graphql/mcp files, docs/reference changes, unrelated P048/P049 deletions, prior R3 audit untracked |
| Audited At | `2026-04-16T22:37:32+03:00` |
| Platform Scope | macOS local control-plane / daemon; no UI rewrite in P029 scope |
| Proposal State | Active draft R8 |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

The current dirty tree is materially closer to Proposal 029 than the prior audited tree: `PrincipalClass` now lives in `domain`, the `domain -> auth` back-edge was removed, steward agent access was fixed, `steward.run_analysis` returns `journal_id`, MCP stdio now returns `-32002` for pre-initialize calls, principal bootstrap uses `OpenOptionsExt::mode(0o600)`, GraphQL subscriptions now have a `connection_init` auth hook, and `./scripts/test-gate.sh proposal-029-mcp` passed on rerun. The implementation is still **Not Implemented** against P029 because the proposal's central typed capability/resource contract is absent: no `CapabilityToolId`, no `ResourceTemplateId`, no `Principal` capability sets, no typed `auth::filter_tools` / `filter_resources` / `match_resource_uri`, and no server-side converters. Several route-level and protocol-close behaviors also remain unproven or partial.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Typed capability/resource model and converters are missing | High |
| Architecture | At Risk | Runtime policy remains string-based, bypassing P029's compile-time drift guard | High |
| Product | At Risk | Auth/capability flows mostly exist, but GraphQL HTTP context and stdio close behavior are not fully contract-aligned | Medium |
| UI | Not Applicable | P029 explicitly excludes UI rewrite | High |
| UX | Acceptable with Risks | Dogfood auth config exists; stdio pre-init failure does not close session as specified | Medium |
| Readiness | Not Ready | Passing gate is broad workspace regression, not the focused P029 proof inventory | High |

## Proposal Contract

### Scope

P029 is an active draft R8 delta over the existing Rust control plane. It is not a greenfield MCP server. In-scope work is principal resolution, caller-scoped capability policy, command journaling, explicit GraphQL coexistence, canonical resource alignment, dogfood MCP auth migration, and a deterministic `proposal-029-mcp` proof lane.

### Locked Decisions

- `domain` remains the transport-neutral root. `auth` depends on `domain`; server crates depend on `auth`; `domain` must not depend on `auth`.
- `PrincipalClass`, `CallerSurface`, `CallerContext`, `CapabilityToolId`, and `ResourceTemplateId` are domain-owned shared types.
- `auth` owns `Principal`, `PrincipalTable`, `Capabilities`, `AuthError`, `resolve_bearer`, `filter_tools`, `filter_resources`, and `match_resource_uri`, all consuming typed IDs from `domain`.
- MCP and GraphQL own transport-specific converters into typed capability IDs.
- `command_journal` is extended instead of creating `mcp_audit_log`.
- MCP command tools and GraphQL mutations converge on `CommandHandler` and surface `journal_id`; direct MCP tools do not.
- GraphQL mutations remain active in Stage A and are deprecated only in Stage B; GraphQL removal is future work.

### Primary User Flows

1. An MCP HTTP client connects with `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}`, sees only allowed tools/resources, and receives `journal_id` for command tools.
2. An MCP stdio client sends `initialize.params.clientInfo.principal_token`, gets a session-bound principal, and cannot rebind mid-session.
3. A GraphQL HTTP client sends an authorized mutation, is checked against the same capability policy, and receives a payload with `journalId`.
4. A GraphQL WebSocket client authenticates in `connection_init` before subscription resolvers run.
5. A first-start operator obtains the auto-bootstrapped token and uses the committed `.mcp.json` dogfood configuration.

### UI Commitments

None. P029 explicitly says there is no UI rewrite.

### UX Commitments

- Dogfood `.mcp.json` must move from bare HTTP to bearer-token auth via `CHAINWORKS_MCP_TOKEN`.
- First-start bootstrap must log the token exactly once and subsequent starts must log only the principals-file path.
- Unauthenticated callers must fail closed with deterministic errors.

### Acceptance Criteria

The explicit acceptance criteria are P029 AC-1 through AC-15 in `docs/proposals/029-mcp-northbound-control-plane-server.md:625-645`.

### Test / Evidence Requirements

P029 requires a green same-tree `./scripts/test-gate.sh proposal-029-mcp` and lists focused test families for bootstrap, MCP HTTP, MCP stdio, GraphQL HTTP, GraphQL WS, capability policy, steward capability policy, command journal rows, `journal_id` surfacing, and cross-surface parity (`docs/proposals/029-mcp-northbound-control-plane-server.md:647-747`).

### Explicit Exclusions

P029 excludes UI rewrite, orchestration ownership changes, southbound runtime protocol replacement, high-frequency reads through MCP, token rotation/revocation/delegation, and deferred second-wave tools/resources (`docs/proposals/029-mcp-northbound-control-plane-server.md:123-146`, `749-757`).

## Proposal Fidelity / Divergence

### Matches

- `PrincipalClass` is now defined in `domain::commands`, and `domain` no longer depends on `auth`.
- `auth` now depends on `domain`.
- `CallerContext` stores `PrincipalClass` as an enum, not a string.
- Principal bootstrap now creates the file with Unix `OpenOptionsExt::mode(0o600)`.
- Agent class no longer gets steward tools or `steward-analysis://`.
- Observer resource policy now includes `artifact://` and the run-scoped `chainworks://runs/{run_id}/stages` and `/artifacts` templates.
- `steward.run_analysis` returns `journal_id`.
- MCP stdio pre-initialize calls now return `-32002`.
- GraphQL subscription route has a manual `on_connection_init` bearer-token hook.
- `.mcp.json` and `CLAUDE.md` contain the dogfood bearer-token migration.
- `proposal-029-mcp` is registered and passed on rerun.

### Divergences

- `CapabilityToolId` and `ResourceTemplateId` do not exist in `domain`.
- `Principal` still contains only `{ id, class }`; it does not carry tool/resource capability sets.
- `auth::ToolSpec` and string-based `filter_tools` remain active.
- `auth::filter_resources` and `auth::match_resource_uri` are absent; resource filtering is via string `is_resource_allowed`.
- `mcp-server/src/tools/mod.rs` has no `capability_id_for` or `mcp_tool_for`.
- `graphql-server/src/schema.rs` has no `MutationName` enum or typed capability converter.
- MCP stdio pre-initialize failure returns `-32002`, but the loop continues instead of closing the session.
- GraphQL HTTP middleware inserts a principal into axum request extensions, but no explicit `GraphQL::new(schema).with_data(...)` or extractor bridge was found to put that principal into `async_graphql::Context`.
- GraphQL WS auth returns an `async_graphql::Error`, but the 4401 close-frame requirement is not directly implemented or tested.
- The gate still runs broad `cargo test --workspace` rather than the named focused inventory and does not fail while string-based capability artifacts remain.

### Ambiguities / Evidence Gaps

- Route-level GraphQL HTTP auth may rely on `async_graphql_axum` behavior not visible in this repo. No route-level tests prove that the principal inserted into axum request extensions reaches mutation `ctx.data::<auth::Principal>()`.
- WebSocket `connection_error` and close-code 4401 behavior were not runtime-validated.
- Bootstrap token "exactly once" is supported by code shape but not asserted by a focused test.
- The first gate run was terminated with SIGTERM during `cargo test --workspace`; an immediate rerun on the same dirty tree passed.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 9 |
| Partially Implemented | 5 |
| Missing | 2 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Domain/auth dependency and caller type ownership

- Proposal Source: §4.0 type ownership and dependency graph, §4.3 caller context (`docs/proposals/029-mcp-northbound-control-plane-server.md:154-180`, `385-428`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/Cargo.toml:6-11`
  - `control-plane/crates/auth/Cargo.toml:6-13`
  - `control-plane/crates/domain/src/commands.rs:5-23`
  - `control-plane/crates/domain/src/commands.rs:86-149`
  - `control-plane/crates/domain/src/lib.rs:14-15`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: This requirement is now implemented for `PrincipalClass`, `CallerSurface`, and `CallerContext`. Typed capability IDs are audited separately because they remain absent.

### REQ-002 Typed capability/resource identifiers and Principal capability sets

- Proposal Source: §4.0 canonical domain additions and auth API (`docs/proposals/029-mcp-northbound-control-plane-server.md:170-231`).
- Status: Missing
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/capabilities.rs` is absent.
  - `control-plane/crates/domain/src/lib.rs:1-15` does not register a capabilities module.
  - `control-plane/crates/auth/src/lib.rs:10-14` defines `Principal` with only `id` and `class`.
  - `control-plane/crates/auth/src/lib.rs:160-178` still exposes `ToolSpec`, `filter_tools(...)->Vec<String>`, and string `is_tool_allowed`.
  - `rg` found no `CapabilityToolId` or `ResourceTemplateId` implementation under `control-plane/crates`.
- Gap / Note: This is the central P029 contract. Without these types and sets, the compile-time drift guard described in §4.0 / §11.2 is not present.

### REQ-003 Server-side capability converters

- Proposal Source: §4.0 server-side converters (`docs/proposals/029-mcp-northbound-control-plane-server.md:233-241`).
- Status: Missing
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/mod.rs:1-6` only exports modules; it has no `capability_id_for` or `mcp_tool_for`.
  - `control-plane/crates/graphql-server/src/schema.rs:1-41` has no `MutationName` enum or typed mutation converter.
  - `control-plane/crates/mcp-server/src/server.rs:194-226` checks string tool names directly.
  - `control-plane/crates/mcp-server/src/server.rs:314-344` checks string resource URIs directly.
- Gap / Note: The implementation has runtime filtering, but not the transport-to-domain typed converter layer P029 requires.

### REQ-004 Principal table bootstrap and fail-closed loading

- Proposal Source: §4.1 token material loading and AC-15 (`docs/proposals/029-mcp-northbound-control-plane-server.md:247-256`, `642-643`).
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:108-119`
  - `control-plane/crates/auth/src/lib.rs:65-126`
  - `control-plane/crates/auth/src/lib.rs:98-110`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: The Unix first-create path now uses `OpenOptionsExt::mode(0o600)` before write, and empty/unparseable files fail closed by code inspection. The explicit empty-env-var contract and "token logged exactly once" contract are not covered by focused tests found in the tree.

### REQ-005 MCP HTTP bearer auth

- Proposal Source: §4.1.a and AC-1 (`docs/proposals/029-mcp-northbound-control-plane-server.md:258-265`, `629`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/http.rs:46-80`
  - `control-plane/crates/mcp-server/src/http.rs:98-103`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: No gap found for source-level behavior. Focused HTTP auth tests with the proposal's exact names were not found, so coverage quality is handled under REQ-016 / READY-001.

### REQ-006 MCP stdio initialize auth and session binding

- Proposal Source: §4.1.b and AC-2/AC-3 (`docs/proposals/029-mcp-northbound-control-plane-server.md:267-288`, `629-631`).
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:80-139`
  - `control-plane/crates/mcp-server/src/server.rs:142-155`
  - `control-plane/crates/daemon/tests/mcp_stdio.rs:8`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: The pre-initialize code now returns `-32002 / "server not initialized"`, but it uses `continue` instead of closing the session. Missing-token and unknown-token paths close, but unknown-token text is generic `"unauthorized"` rather than the proposal's `"unauthorized: unknown token"`. Focused stdio auth tests from §9 were not found.

### REQ-007 GraphQL HTTP auth and mutation principal checks

- Proposal Source: §4.1.c GraphQL HTTP and AC-4/AC-7/AC-9 (`docs/proposals/029-mcp-northbound-control-plane-server.md:290-317`, `632`, `635`, `637`).
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/auth_layer.rs:15-66`
  - `control-plane/crates/graphql-server/src/server.rs:91-106`
  - `control-plane/crates/graphql-server/src/schema.rs:300-308`
  - `control-plane/crates/graphql-server/src/schema.rs:357-366`
  - `control-plane/crates/graphql-server/src/schema.rs:451-460`
  - `control-plane/crates/graphql-server/src/schema.rs:481-490`
  - Schema unit tests inject `.data(test_principal())` directly, for example `control-plane/crates/graphql-server/src/schema.rs:962-989`.
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: Middleware resolves a principal and stores it in request extensions, and resolvers enforce capability checks if `ctx.data::<auth::Principal>()` exists. The proposal explicitly calls for a bridge from axum request extensions into `async_graphql::Context`; no `with_data` or extractor bridge was found, and existing tests bypass the route seam by directly injecting `test_principal()`.

### REQ-008 GraphQL WebSocket connection_init auth

- Proposal Source: §4.1.c subscription path and §9 GraphQL WS tests (`docs/proposals/029-mcp-northbound-control-plane-server.md:318-331`, `687-694`).
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/server.rs:22-69`
  - `control-plane/crates/graphql-server/src/schema.rs:509-555`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: A manual `GraphQLWebSocket` handler with `on_connection_init` now exists and injects `Principal` into `async_graphql::Data`. No tests were found for missing/unknown/valid `connection_init`, and there is no direct evidence that failures close the socket with status 4401 as required.

### REQ-009 MCP tools/list and tools/call runtime class policy

- Proposal Source: §4.2 and AC-5/AC-6 (`docs/proposals/029-mcp-northbound-control-plane-server.md:334-355`, `633-635`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/auth/src/lib.rs:180-215`
  - `control-plane/crates/mcp-server/src/server.rs:194-226`
  - `control-plane/crates/auth/src/lib.rs:387-437`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: Runtime string policy now matches the P029 class table, including the steward agent exclusion. The typed-ID architecture remains missing and is covered by REQ-002/REQ-003.

### REQ-010 MCP resources/list and resources/read runtime class policy

- Proposal Source: §6 and AC-12/AC-13 (`docs/proposals/029-mcp-northbound-control-plane-server.md:582-588`, `640-641`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/auth/src/lib.rs:219-289`
  - `control-plane/crates/mcp-server/src/server.rs:245-345`
  - `control-plane/crates/auth/src/lib.rs:409-455`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: Runtime string resource policy now matches the class table, including agent denial for `steward-analysis://` and observer read access to artifact plus run-scoped collections. The typed `filter_resources` / `match_resource_uri` API remains missing under REQ-002.

### REQ-011 Command journal caller metadata

- Proposal Source: §4.3 and AC-8/AC-9 (`docs/proposals/029-mcp-northbound-control-plane-server.md:357-463`, `636-637`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/011_auth_tracking.sql:1-5`
  - `control-plane/crates/db/src/repos/command_journal.rs:9-38`
  - `control-plane/crates/engine/src/command_handler.rs:77-147`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: Source-level contract is implemented. Focused end-to-end row-shape tests for every named P029 command path were not found and are handled under REQ-016.

### REQ-012 Journal payload redaction

- Proposal Source: §4.3 corrected contract and AC-10 (`docs/proposals/029-mcp-northbound-control-plane-server.md:430-444`, `638`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:89-107`
  - `control-plane/crates/engine/src/command_journal_redact.rs:8-38`
  - `control-plane/crates/engine/src/command_journal_redact.rs:47-68`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: No source-level gap found.

### REQ-013 MCP command tools return journal_id

- Proposal Source: §4.4.a and AC-11 (`docs/proposals/029-mcp-northbound-control-plane-server.md:465-494`, `639`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/runs.rs:117-141`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:164-170`
  - `control-plane/crates/mcp-server/src/tools/approvals.rs:85-89`
  - `control-plane/crates/mcp-server/src/tools/stages.rs:43-49`
  - `control-plane/crates/mcp-server/src/tools/steward.rs:57-74`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: The prior steward gap is fixed in code. Focused assertions for `journal_id` in steward response were not found, but source behavior is direct.

### REQ-014 GraphQL mutation payloads expose journalId

- Proposal Source: §4.4.b and AC-11 (`docs/proposals/029-mcp-northbound-control-plane-server.md:496-518`, `639`).
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:238-280`
  - `control-plane/crates/graphql-server/src/schema.rs:325-341`
  - `control-plane/crates/graphql-server/src/schema.rs:378-391`
  - `control-plane/crates/graphql-server/src/schema.rs:426-439`
  - `control-plane/crates/graphql-server/src/schema.rs:471-500`
  - GraphQL schema tests query `journalId`, for example `control-plane/crates/graphql-server/src/schema.rs:962-989` and `1017-1047`.
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: No source-level gap found.

### REQ-015 Dogfood MCP client migration

- Proposal Source: §7.1 (`docs/proposals/029-mcp-northbound-control-plane-server.md:593-619`).
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `.mcp.json:1-11`
  - `CLAUDE.md:57-59`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Gap / Note: No gap found.

### REQ-016 Proof lane registration and focused coverage

- Proposal Source: §9 test gate and AC-14 (`docs/proposals/029-mcp-northbound-control-plane-server.md:647-747`).
- Status: Partially Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `scripts/test-gate.sh:1198-1199`
  - `scripts/test-gate.sh:1445-1451`
  - `docs/reference/test-gates.md:570-587`
  - First run of `./scripts/test-gate.sh proposal-029-mcp` terminated with SIGTERM during `cargo test --workspace`.
  - Immediate rerun of `./scripts/test-gate.sh proposal-029-mcp` passed.
- Gap / Note: The gate is registered and passes, but it is still broad workspace regression only. Searches found no focused tests named for GraphQL WS auth, GraphQL route auth, stdio close semantics, typed capability IDs, `mcp_tool_for`, `MutationName`, or compile-failing stale-artifact prevention. P029 says the gate cannot be green while stale artifacts remain; it is green while `ToolSpec` and string filtering remain.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Typed capability/resource architecture is still absent

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-002, REQ-003; P029 §4.0, §4.5, §11.2
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/domain/src/capabilities.rs` absent
  - `control-plane/crates/auth/src/lib.rs:10-14`
  - `control-plane/crates/auth/src/lib.rs:160-178`
  - `control-plane/crates/mcp-server/src/tools/mod.rs:1-6`
  - `control-plane/crates/graphql-server/src/schema.rs:1-41`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Why It Matters: P029's architecture is not just class policy; it is a compile-time drift guard. String filtering lets future tool/resource additions compile without updating central capability enums and converters.
- Recommended Action: Add `domain/src/capabilities.rs`, export `CapabilityToolId` and `ResourceTemplateId`, extend `Principal` with capability sets, replace string filter APIs, and add MCP/GraphQL converters that must be updated for every new tool/resource.

### ARCH-002 GraphQL HTTP auth bridge is not explicit

- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: REQ-007; P029 §4.1.c
- Evidence Type: code, tests-found
- Evidence:
  - `control-plane/crates/graphql-server/src/auth_layer.rs:39-43`
  - `control-plane/crates/graphql-server/src/server.rs:91-106`
  - `control-plane/crates/graphql-server/src/schema.rs:300-308`
  - `control-plane/crates/graphql-server/src/schema.rs:962-989`
- Why It Matters: Middleware inserts `Principal` into axum request extensions, while resolvers read `async_graphql::Context` data. P029 calls for an explicit bridge. Without route-level proof, the production HTTP mutation path can reject as "unauthorized: no principal in context" even when bearer auth succeeds.
- Recommended Action: Use an explicit request-to-GraphQL-data bridge or route handler that calls `Request::data(principal)`, then add route-level tests for missing token, unknown token, valid mutation, and observer-forbidden mutation.

## Product Review

**Summary:** At Risk

### PROD-001 Primary northbound auth flows are mechanically present but not fully product-proven

- Severity: Major
- Confidence: Medium
- Related Proposal Items / Requirements: REQ-006, REQ-007, REQ-008, REQ-016
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:142-155`
  - `control-plane/crates/graphql-server/src/server.rs:22-69`
  - `control-plane/crates/graphql-server/src/server.rs:91-106`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Why It Matters: The user value of P029 is safe future client adoption. If stdio clients are not closed after pre-init protocol errors, GraphQL HTTP auth is not route-proven, and WS close behavior is untested, clients can see inconsistent failure behavior across the three ingress modes.
- Recommended Action: Add end-to-end route/protocol tests for MCP stdio failure/close, GraphQL HTTP auth propagation, and GraphQL WS missing/unknown/valid `connection_init`.

## UI Review

**Summary:** Not Applicable

P029 explicitly excludes UI rewrite and does not define UI screens, visual states, SwiftUI surfaces, or Apple Human Interface Guideline commitments. No UI finding is emitted and no UI tests are required for this proposal.

## UX Review

**Summary:** Acceptable with Risks

### UX-001 Stdio pre-initialize failure does not close the session as specified

- Severity: Minor
- Confidence: High
- Related Proposal Items / Requirements: REQ-006; P029 §4.1.b
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/server.rs:142-155`
- Why It Matters: P029's failure contract is explicit: first non-`initialize` gets `-32002` and closes stdin. Continuing the loop leaves the client in a recoverable-but-unspecified state and can hide protocol misuse.
- Recommended Action: Change the pre-initialize non-`initialize` branch from `continue` to session close/break and add the focused stdio test.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Passing gate does not enforce P029's focused proof contract

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-016; P029 §9
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `scripts/test-gate.sh:1445-1451`
  - `docs/proposals/029-mcp-northbound-control-plane-server.md:668-738`
  - `control-plane/crates/auth/src/lib.rs:160-178`
  - `./scripts/test-gate.sh proposal-029-mcp` passed on rerun.
- Why It Matters: The current gate is green while the proposal's explicit stale artifacts remain. That means the proof lane cannot be used as a release/readiness signal for P029.
- Recommended Action: Keep `cargo test --workspace` as the broad regression layer, but add focused assertions/compile-time coverage for every §9 family. The gate must fail while `ToolSpec`/string filters and missing typed converters remain.

### READY-002 Audit is against a dirty working tree

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: Delivery readiness
- Evidence Type: code
- Evidence:
  - `git status --short` shows uncommitted changes in `control-plane/Cargo.lock`, auth/domain/graphql/mcp files, P029, docs/reference files, unrelated P048/P049 deletions, and untracked R3/reference docs.
- Why It Matters: The implementation may be correct in the working tree but not reproducible from `HEAD`. Handoff and release validation should happen from a committed tree or a clearly named branch.
- Recommended Action: After fixing the remaining P029 gaps, commit the P029 implementation and rerun `proposal-029-mcp` from a clean or intentionally documented dirty tree.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `./scripts/test-gate.sh proposal-029-mcp` rerun passed; it runs `cargo test --workspace` |
| Core user flow runtime-validated | Partial | Code/gate validated workspace tests; no live HTTP/WS runtime test was executed |
| Empty/loading/error states covered | Partial | Protocol errors exist, but stdio close and WS 4401 are not proven |
| Accessibility risk acceptable | Not Applicable | No UI in P029 scope |
| Localization risk acceptable | Not Applicable | Daemon/API proposal, no user-facing UI strings committed beyond docs/errors |
| Critical tests executed | Partial | Full workspace tests passed on rerun; focused P029 route/protocol tests missing |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `./scripts/test-gate.sh proposal-029-mcp` passed on rerun |
| Privacy/permissions/entitlements reviewed | Partial | Principal file mode fixed in code; token-once and empty-env focused tests missing |

## Verification Log

- `sed -n '1,260p' /Users/user/.agents/skills/proposal-implementation-audit/SKILL.md`
- `git rev-parse HEAD`
- `date -Iseconds`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/029-mcp-northbound-control-plane-server.md`
- `rg -n "superseded|deprecated|replaced by|obsolete|Proposal 029|029-mcp|proposal-029|p029|mcp northbound" docs/proposals docs/reference docs/reviews scripts/test-gate.sh -g '!docs/proposals/*_IMPLEMENTATION_AUDIT_*.md'`
- `nl -ba docs/proposals/029-mcp-northbound-control-plane-server.md | sed -n '1,220p'`
- `nl -ba docs/proposals/029-mcp-northbound-control-plane-server.md | sed -n '220,460p'`
- `nl -ba docs/proposals/029-mcp-northbound-control-plane-server.md | sed -n '460,760p'`
- `rg -n "PrincipalClass|CapabilityToolId|ResourceTemplateId|CallerContext|ToolSpec|filter_tools|filter_resources|match_resource_uri|capability_id_for|resource_template_id_for|MutationName|on_connection_init|connection_init|principal_token|OpenOptions|set_permissions|0o600|journal_id" control-plane/crates/...`
- `nl -ba control-plane/crates/domain/src/commands.rs | sed -n '1,165p'`
- `nl -ba control-plane/crates/auth/src/lib.rs | sed -n '1,285p'`
- `nl -ba control-plane/crates/auth/src/lib.rs | sed -n '285,470p'`
- `nl -ba control-plane/crates/mcp-server/src/server.rs | sed -n '1,380p'`
- `nl -ba control-plane/crates/mcp-server/src/tools/steward.rs | sed -n '1,110p'`
- `nl -ba control-plane/crates/graphql-server/src/server.rs | sed -n '1,130p'`
- `nl -ba control-plane/crates/graphql-server/src/auth_layer.rs | sed -n '1,130p'`
- `nl -ba control-plane/crates/graphql-server/src/schema.rs | sed -n '280,575p'`
- `rg -n "test_graphql_rejects_missing_authorization_header|test_graphql_rejects_unknown_bearer_token|test_graphql_mutation_reads_principal_from_context|test_graphql_observer_class_cannot_invoke_start_run|test_graphql_ws_rejects|test_graphql_ws_accepts|on_connection_init|connection_init|GraphQL::new\(|with_data|auth_layer|require_auth|Authorization" control-plane/crates/graphql-server control-plane/crates/daemon control-plane/crates/mcp-server -g '!target/**'`
- `rg -n "test_mcp_stdio_rejects_first_frame|test_mcp_stdio_rejects_initialize|test_mcp_stdio_binds|test_mcp_stdio_rejects_reinitialize|test_mcp_tools_list_filtered|test_mcp_resources_list|test_mcp_resources_read|test_mcp_steward_run_analysis_response|test_command_journal_row_has_caller_mcp_for_steward|test_mcp_tools_call_denied_returns_method_not_found|test_principals_file_created|test_principals_bootstrap|test_principals_daemon" control-plane -g '!target/**'`
- `./scripts/test-gate.sh proposal-029-mcp` first run: terminated with SIGTERM during `cargo test --workspace`
- `./scripts/test-gate.sh proposal-029-mcp` second run: passed

## Recommended Next Actions

1. Implement the typed capability/resource model: `domain/src/capabilities.rs`, `CapabilityToolId`, `ResourceTemplateId`, `Principal` capability sets, typed `filter_tools`, `filter_resources`, and `match_resource_uri`.
2. Add MCP and GraphQL converters: `mcp-server::tools::capability_id_for`, `mcp_tool_for`, concrete URI to `ResourceTemplateId`, `MutationName`, and GraphQL mutation to `CapabilityToolId`.
3. Make `proposal-029-mcp` fail on stale artifacts by adding focused §9 tests or compile-time checks for typed IDs and converters.
4. Add route-level GraphQL HTTP tests proving bearer auth reaches `ctx.data::<auth::Principal>()`.
5. Add GraphQL WS tests for missing, unknown, and valid `connection_init`; verify `connection_error` and close behavior.
6. Fix MCP stdio first non-`initialize` to close the session after `-32002`.
7. Add focused bootstrap tests for 0600 mode, token logged once, empty principals file, unparseable file, and explicit empty env var.
