# ACP Dependency Audit

Date: 2026-09-27. Repository base: `dc4ea294f25ed1ce9de83e63b93430877e9a1be2`.
Scope: installed provider adapters and their bundled SDKs, not a blanket Cargo
or global npm upgrade. The user's "A2C" was interpreted as ACP in this context.

Status: the three outdated adapters were upgraded after provider quiescence.
Post-install dependency resolution and initialize-only probes passed. Full
authenticated provider execution remains a separate acceptance check.

## Version Inventory

Registry `latest` tags and the actual local executable/package locations were
checked. Preview/nightly releases were excluded.

| Component | Before | Installed After |
| --- | --- | --- |
| `@agentclientprotocol/codex-acp` | 1.10.0 | 1.13.1 |
| `@agentclientprotocol/claude-agent-acp` | 0.75.1 | 0.81.2 |
| `@google/gemini-cli` | 0.58.0 | 0.61.0 |
| `@openai/codex` | 0.157.1 | 0.157.1 |
| `@anthropic-ai/claude-code` | 2.1.283 | 2.1.283 |
| `@augmentcode/auggie` | 0.36.0 | 0.36.0 |
| `@jetbrains/junie-cli` | 1468.30.0 | 1468.30.0 |

Junie's selected native installation was separately checked as 1468.30; no
pending-update manifest was applied. Standalone Claude/Codex CLI versions do
not prove which runtime an ACP adapter loads.

Before the upgrade, Codex ACP declared `@openai/codex ^0.153.3` but resolved the global
0.157.1 package. `npm ls` reported `ELSPROBLEMS` for this incompatible 0.x minor
range. Do not fix this by ignoring semver or forcing deduplication.

The isolated candidate installation resolved a consistent tree:

- Codex ACP 1.13.1 -> ACP SDK 1.5.0 and Codex CLI 0.156.1.
- Claude ACP 0.81.2 -> ACP SDK 1.5.0 and Claude Agent SDK 0.3.280.
- Gemini CLI 0.61.0 uses its own ACP entry point (`--acp`).

The Rust `acp` crate uses the existing JSON-RPC transport and has no npm/ACP SDK
dependency in Cargo. No unrelated Rust dependencies were changed.

## Isolated Verification

Installed exact candidates under `/private/tmp/cw-acp-upgrade-20260927`, without
changing global executable links, user credentials, or the running daemon.
Each process used a new empty HOME/CODEX_HOME, an environment allowlist, and a
disposable working directory. The only request was ACP `initialize`, protocol 1.
No authentication, `session/new`, prompts, MCP project access, or live work ran.

All three candidates returned protocol 1 and their expected agent version.
The first Codex probe failed because the harness had not created CODEX_HOME;
after correcting that fixture, the complete three-provider probe passed.
The harness terminated only its own process groups after receiving the response.

This proves process startup and initial protocol compatibility, not authenticated
model selection, permission handling, reconnect, cancellation, or task execution.
Npm left unapproved optional `@github/keytar` and `node-pty` install scripts
unexecuted. Their behavior was not covered or implicitly approved by this probe.

| Local Evidence | SHA-256 |
| --- | --- |
| Candidate `package-lock.json` | `0d67ba066a14bafa5b941e3b151be9a7859e9117a5bac7c28e3af43bf85a57c6` |
| Initial isolated harness bytes | `7107df6feec89aaf42186d62e39218507cfba193863b96fa5906829d990dcf23` |
| Final `/private/tmp/cw-acp-probe.mjs`, also accepts actual global bin directory | `8385db200829c6c601a93d314c246a8f3a29205877fce47a47f9c65cc2cf5fde` |

Per-provider request/response evidence is private under the candidate directory's
`probes/` tree. No credentials were supplied or recorded.

## Cutover

Initially cutover waited for P070 B, whose runtime advanced from Claude to Gemini.
Fresh MCP readback subsequently showed no active runs, zero provider sessions,
and zero unresolved effects. The app exited and its remaining daemon stopped
with SIGTERM before package replacement. No active invocation was cancelled.

The old three package trees and executable links were preserved at
`/private/tmp/p039-delivery-20260927/previous-acp-packages.tgz`, SHA-256
`f7bf8a190daa56e409045bd694ad84cba594fdab7bdf8233e0d18c442c4a5872`.
After reviewing `npm install -g --dry-run`, the three exact candidates were
installed globally. `npm ls` now succeeds: Codex ACP resolves its own compatible
0.156.1; standalone Codex remains 0.157.1. Unrelated top-level CLI versions did
not change. Optional install scripts remain unapproved and unexecuted.

The same empty-HOME probe was rerun against `/opt/homebrew/bin/codex-acp`,
`claude-agent-acp`, and `gemini`. All three returned protocol 1 and the expected
new version. Post-install evidence directories end in `1790526162802`,
`1790526165341`, and `1790526165631`, respectively. No authenticated sessions or
prompts were sent. The updated Chainworks app/daemon was then started successfully.

An authenticated controlled task and resume/cancel checks remain separate
provider acceptance. This audit is not proof that P095 carry-forward succeeded.

## Independent Review And Compatibility Gaps

Antigravity was delegated public-upstream release research and a separate ACP
SDK 1.5.0 client-contract review through A2A. Its initial protocol examples were
not accepted verbatim: follow-up review corrected permission-response nesting,
version negotiation, and the prohibition on replying to JSON-RPC notifications.
The permission result is an object under `result.outcome`, not a string with a
sibling `optionId`. Local installed SDK declarations independently confirm this.
Research task IDs: `01a0e3ae-843a-7674-b1b0-5556a25ee068` and
`01a0e3b0-6c23-7b89-898f-9b2fae03982e`; correction task:
`01a0e3b5-1a57-7a6b-a223-1fb9e09778b1`.

Local source comparison also found a **pre-existing Claude resurrection wire
mismatch**, not an upgrade regression. Both the archived 0.75.1 package and
installed 0.81.2 implement `newSession` by passing
`params._meta?.claudeCode?.options?.resume` to `createSession`. Neither reads the
top-level `resumeSessionId` emitted by the current Chainworks Claude adapter.
`build_session_new_params` forwards that field without translation. The runtime
manager checks the returned provider session identity and rejects a mismatch;
this does not prove successful resurrection. Existing fixture tests that accept
the top-level field are not upstream interoperability evidence.

This finding is not fixed in the dependency-upgrade commit. Acceptance requires
aligning the adapter's request/capability descriptor and strict fixtures with the
upstream field, then proving same-session attach without a prompt dispatched to
an unverified identity. Authenticated resume/cancel acceptance remains open.
The removed Claude agent-picker option is not used by the current adapter;
its raw-SDK diagnostic and debug-file options are still present upstream.

## Primary Sources

- [Codex ACP releases](https://github.com/agentclientprotocol/codex-acp/releases):
  SDK/runtime updates, session notices, compaction, terminal delta handling.
- [Claude ACP changelog](https://github.com/agentclientprotocol/claude-agent-acp/blob/main/CHANGELOG.md):
  SDK updates and fixes for resumed subagents and context clearing.
- [Gemini CLI releases](https://github.com/google-gemini/gemini-cli/releases) and
  [changelog](https://github.com/google-gemini/gemini-cli/blob/main/docs/changelogs/index.md):
  stable 0.61.0 and intervening workspace-trust/security changes.
