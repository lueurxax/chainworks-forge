# MCP Northbound Control-Plane Server Proof

Current implementation and proof status for MCP + GraphQL bearer auth, caller-scoped capability filtering, per-command audit journaling, and `journal_id` surfacing on the Rust control-plane daemon.

## Status

| Field | Value |
|---|---|
| Slice | MCP Northbound Control-Plane Server |
| Source contract | [../../reference/mcp-northbound-control-plane-server.md](../../reference/mcp-northbound-control-plane-server.md) |
| Current implementation status | Implemented |
| Current readiness | Ready with Risks |
| Primary proof owners | Focused `proposal-029-mcp` gate, full workspace regression |

## Canonical gate command

```bash
./scripts/test-gate.sh proposal-029-mcp
```

The gate enumerates a fixed inventory of 63 focused tests covering principal-table bootstrap, transport auth (MCP HTTP + stdio, GraphQL HTTP + WebSocket), capability filtering (tool list / call, resource list / read, including the Steward trio), command-journal caller metadata, the §8.1 redaction matrix (one test per decision), `journal_id` surfacing on MCP command tools and GraphQL mutation payload wrappers, the typed `DeliveryPreflight` object on blocked `startRun`, cross-surface parity, and dogfood `.mcp.json` / `CLAUDE.md` consistency. For each test the runner grep-checks for a `test <name>` line so that a rename, typo, or deletion fails the gate independently of the test body. A full `cargo test --workspace` regression runs as the final step.

Gate ownership and inventory source of truth: [../../reference/test-gates.md](../../reference/test-gates.md).

## Gate run

| Field | Value |
|---|---|
| Command | `./scripts/test-gate.sh proposal-029-mcp` |
| Git HEAD | `3add7179d820a943745070a74e3f368a06fdf9ca` |
| Result | `==> Proposal 029-MCP control-plane gate passed` |
| Log | [`gate-proposal-029-mcp.2026-04-18.clean.log`](gate-proposal-029-mcp.2026-04-18.clean.log) |

The final three lines of the log are:

```
test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.00s

==> Proposal 029-MCP control-plane gate passed
```

Gate exits cleanly after the pass marker. 63 focused tests plus the full workspace regression (~328 total including unit, integration, and doctest phases) pass with zero failures.

## What is considered proven

The gate supports these claims:

- principal table auto-bootstraps at `~/.chainworks/auth/principals.json` on first start with owner-only (`0o600`) file mode on Unix,
- the bootstrap token is logged at `info` level exactly once (on the start that created the file); subsequent starts log only the path,
- a zero-principal or unparseable principals file fails the daemon closed,
- MCP HTTP rejects missing and unknown bearer tokens with JSON-RPC `-32000 "unauthorized"`,
- MCP stdio rejects first-frame non-`initialize` with `-32002`, rejects `initialize` without or with an unknown `principal_token` with `-32000`, binds the resolved principal for session lifetime, and rejects mid-session reinitialize,
- GraphQL HTTP rejects missing or unknown bearer tokens with HTTP 401 and a GraphQL-shaped error body,
- GraphQL WebSocket rejects missing or unknown `connection_init` tokens and accepts valid ones via `on_connection_init`,
- `tools/list` and `resources/list` are class-filtered (operator / agent / observer), including the Steward trio policy,
- `tools/call` for a denied tool returns `-32601`; `resources/read` for a denied URI returns `-32002`,
- every command-tool `tools/call` and every GraphQL mutation that invokes `CommandHandler` writes one `command_journal` row with `caller_surface`, non-null `caller_principal_id`, matching `caller_principal_class`, and `caller_tool` set to the tool or mutation name,
- the §8.1 redaction matrix is applied per `Command` variant (delivery configuration and approval comments redacted, identity / audit fields preserved),
- MCP command-tool responses include `journal_id` inside `content[0].text` stringified JSON; direct-tool responses omit it,
- GraphQL mutation payload wrappers expose `journalId: ID!` on both `startRun` variants, `approveStage`, `rejectStage`, `retryStage`, and `cancelRun`,
- blocked `startRun` carries a typed `DeliveryPreflight` object (not a JSON string),
- a GraphQL mutation and an MCP command tool that target the same typed `Command` produce identical run outcomes (cross-surface parity),
- the repo-root `.mcp.json` registers `chainworks-control-plane` at `http://127.0.0.1:4000/mcp` with an `Authorization: Bearer ${CHAINWORKS_MCP_TOKEN}` header, and `CLAUDE.md` references the same URL, header contract, and env var.

## Remaining caution

Readiness sits at `Ready with Risks` rather than fully frictionless `Ready` because:

- token rotation, revocation, and delegation are deferred; the principal table is read once at daemon startup and rotation requires a restart,
- the `structuredContent` typed-output channel is not used yet; `journal_id` flows only inside `content[0].text` because the server still advertises `protocolVersion: "2024-11-05"`,
- per-subscription GraphQL WebSocket capability filtering is permissive (all authenticated principals can subscribe to all subscriptions),
- no `CallerSurface::Internal` variant is defined because no internal caller currently routes through `CommandHandler`; executor and recovery continue to drive work directly.

None of these is a behavioral defect in the landed surface; they are scoped-out extensions whose own owning slices will close them.

## Usage guidance

Use:

- [../../reference/mcp-northbound-control-plane-server.md](../../reference/mcp-northbound-control-plane-server.md) for the stable contract,
- this document for implementation and proof status,
- [../../reference/test-gates.md](../../reference/test-gates.md) for the canonical `proposal-029-mcp` verification lane.
