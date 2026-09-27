# ACP Dependency Audit

Date: 2026-09-27. Repository base: `dc4ea294f25ed1ce9de83e63b93430877e9a1be2`.
Scope: installed provider adapters and their bundled SDKs, not a blanket Cargo
or global npm upgrade. The user's "A2C" was interpreted as ACP in this context.

## Version Inventory

Registry `latest` tags and the actual local executable/package locations were
checked. Preview/nightly releases were excluded.

| Component | Installed | Stable Candidate |
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

Installed Codex ACP declared `@openai/codex ^0.153.3` but resolved the global
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
| `/private/tmp/cw-acp-probe.mjs` | `7107df6feec89aaf42186d62e39218507cfba193863b96fa5906829d990dcf23` |

Per-provider request/response evidence is private under the candidate directory's
`probes/` tree. No credentials were supplied or recorded.

## Cutover Boundary

At this evidence cutoff, global packages are **not upgraded**. P070 B is still
executing through the installed providers (runtime readback advanced from Claude
to Gemini). Do not replace their module
tree while it runs. Preserve the old package trees and links, wait for provider
quiescence, install only the three exact stable candidates, and verify actual
resolution plus the ACP handshake again. Keep standalone Codex 0.157.1 separate
from the adapter's compatible 0.156.1 dependency.

An authenticated controlled task and resume/cancel checks remain separate
provider acceptance. This audit is not proof that P095 carry-forward succeeded.

## Primary Sources

- [Codex ACP releases](https://github.com/agentclientprotocol/codex-acp/releases):
  SDK/runtime updates, session notices, compaction, terminal delta handling.
- [Claude ACP changelog](https://github.com/agentclientprotocol/claude-agent-acp/blob/main/CHANGELOG.md):
  SDK updates and fixes for resumed subagents and context clearing.
- [Gemini CLI releases](https://github.com/google-gemini/gemini-cli/releases) and
  [changelog](https://github.com/google-gemini/gemini-cli/blob/main/docs/changelogs/index.md):
  stable 0.61.0 and intervening workspace-trust/security changes.
