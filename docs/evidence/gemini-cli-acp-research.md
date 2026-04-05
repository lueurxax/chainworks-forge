# Gemini CLI ACP Research

Status: **Live-Probed** (2026-04-04)

## Purpose

This note evaluates whether Gemini CLI ACP is a realistic execution substrate for
`Chainworks Forge` under
[`Proposal 026`](../proposals/026-acp-first-runtime-transport-and-goose-decoupling.md),
with specific focus on:

- ACP transport fidelity
- prompt streaming and replay visibility
- session load/resume behavior
- model and mode mutation truth
- MCP and file-system proxy shape
- fit for Forge runtime transport decoupling

The goal is not to prove that Gemini CLI is “good in general”. The goal is to
decide whether Gemini CLI ACP is a credible ACP-first candidate for the
post-Goose runtime seam and where it still diverges from Forge needs.

## Executive Summary

Gemini CLI ACP is a credible and strong candidate for `P026`.

What is already solid on current evidence:

- ACP mode is official, documented, and first-class in the product
- the local ACP server is real and probeable with `gemini --acp`
- stdio transport is real and uses newline-delimited JSON rather than
  `Content-Length` framing
- `initialize` works and exposes a credible auth/capability surface
- `session/new` works and returns usable model and mode catalogs
- `session/prompt` emits real `session/update` notifications
- `session/load` replays prior conversation chunks through `session/update`
- active-session model switching works for prompt execution truth
- real tool execution is visible through ACP `tool_call` / `tool_call_update`
- edit flows can raise ACP `session/request_permission`
- edit flows can hit ACP `fs/read_text_file`
- MCP attachment and real MCP tool execution are live-proved
- prompt responses include per-prompt quota and model usage in `_meta.quota`
- official docs and runtime code both support MCP server injection and ACP file
  system proxying

What is still not good enough for a clean Forge cutover:

- session config truth is not durably settled through `loadSession`
- `setSessionMode(plan)` succeeded, but `loadSession` still reported
  `currentModeId = default`
- `setSessionModel(gemini-2.5-flash-lite)` affected live prompt execution, but
  `loadSession` still reported `currentModelId = auto-gemini-3`
- session durability appears lazy: `loadSession` on a brand-new session before
  any prompt returned an internal “invalid session identifier” error
- `fs/write_text_file` was not separately observed even when edit flows
  succeeded, so the exact write-path ownership is still not fully proven
- MCP tool execution is visible, but the observed Gemini ACP trace surfaced only
  a completed `tool_call_update`, not a full pending-to-completed MCP lifecycle

Bottom line:

- Gemini CLI ACP is materially stronger than Goose ACP for the transport and
  observability slice relevant to Forge
- Gemini CLI ACP is also materially stronger than the current OpenCode ACP
  evidence on callback-heavy transport truth
- it is strong enough to take seriously as a primary `P026` migration target
- but Forge should still treat persisted mode/model truth and exact write-path
  settlement as live blockers until explicitly proven

## Method

The evaluation used three evidence sources:

1. Official Gemini CLI documentation
2. Same-machine local binary and installed package inspection
3. Live stdio ACP probes against:

```text
/opt/homebrew/bin/gemini --acp
```

Probe environment:

- macOS desktop environment
- local Gemini CLI binary at `/opt/homebrew/bin/gemini`
- local version at probe time: `0.36.0`
- probe workspace: `/Users/user/Documents/Chainworks Forge`
- client: temporary Python JSON-RPC stdio probe

Additional local inspection used:

- `gemini --help`
- `gemini --acp --help`
- installed package bundle under:
  `/opt/homebrew/lib/node_modules/@google/gemini-cli`
- a temporary local MCP probe server built with Python `FastMCP`

## Official Documentation Findings

### ACP mode is official and first-class

Official docs:

- [ACP Mode](https://geminicli.com/docs/cli/acp-mode/)

The docs explicitly position ACP mode as a special operational mode for IDE and
tool integrations and state that it uses JSON-RPC over stdio.

Documented core methods:

- `initialize`
- `authenticate`
- `newSession`
- `loadSession`
- `prompt`
- `cancel`
- `setSessionMode`
- `unstable_setSessionModel`

The same page also documents:

- MCP integration during ACP `initialize`
- file-system proxying through the ACP client
- debugging via `gemini --acp --debug`
- telemetry capture to local JSON logs

This is significantly stronger positioning than “experimental side transport”.

### Gemini CLI is already positioned as an ACP-distributed agent

Official docs:

- [ACP Mode](https://geminicli.com/docs/cli/acp-mode/)
- [IDE Integration](https://geminicli.com/docs/ide-integration/)
- [Telemetry](https://geminicli.com/docs/cli/telemetry/)

Gemini CLI docs explicitly state that:

- Gemini CLI is in the ACP Agent Registry
- JetBrains IDEs can install ACP agents from the registry
- Zed integrates with the ACP registry

Gemini CLI telemetry docs also identify existing ACP surfaces:

- `GeminiCLI-acp-zed`
- `GeminiCLI-acp-xcode`
- `GeminiCLI-acp-intellijidea`

That does not by itself prove feature parity, but it does prove ACP is not a
theoretical branch. Gemini CLI already treats ACP as a real product surface.

### ACP plus MCP is documented as a first-class path

Official docs:

- [ACP Mode](https://geminicli.com/docs/cli/acp-mode/)
- [Set up an MCP server](https://geminicli.com/docs/tools/mcp-server/)

The ACP docs say the client can provide an MCP server during the `initialize`
handshake and Gemini CLI will connect to it and expose its tools to the model.

This matters for Forge because `P025` keeps MCP policy product-owned while
`P026` wants transport-specific realization. Gemini CLI’s official story is
already aligned with that split.

## Local Package Findings

The installed `0.36.0` package confirms the official ACP story is implemented,
not merely documented.

Observed from the local bundle:

- ACP request handling includes:
  - `initialize`
  - `session/new`
  - `session/load`
  - `session/list`
  - `session/fork`
  - `session/resume`
  - `session/close`
  - `session/set_mode`
  - `session/set_model`
  - `session/prompt`
  - `authenticate`
- the runtime sends structured `session/update` notifications for:
  - `user_message_chunk`
  - `agent_message_chunk`
  - `agent_thought_chunk`
  - `tool_call`
  - `tool_call_update`
  - `plan`
  - `available_commands_update`
  - `current_mode_update`
  - `config_option_update`
  - `session_info_update`
  - `usage_update`
- the ACP server includes an `AcpFileSystemService` when the client advertises
  file-system capabilities
- `newSession` merges client-supplied ACP MCP servers into runtime config

Important implication:

Gemini CLI ACP is not just a thin request/response shim. The local runtime code
is explicitly shaped around ACP-native notifications, callbacks, MCP injection,
and file-system proxying.

One transport-specific implementation detail also matters for Forge:

- Gemini ACP stdio uses newline-delimited JSON (`ndjson`) framing, not
  `Content-Length` framing

That is not a product blocker, but it is an adapter detail `P026` should treat
as runtime-specific rather than ACP-generic.

## Live ACP Probe Findings

## 1. Handshake is real and usable

The following minimal stdio probe succeeded:

```json
{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":1}}
```

Observed `initialize` result:

- `protocolVersion = 1`
- `agentInfo.name = "gemini-cli"`
- `agentInfo.version = "0.36.0"`
- auth methods:
  - `oauth-personal`
  - `gemini-api-key`
  - `vertex-ai`
  - `gateway`
- `loadSession = true`
- `promptCapabilities.image = true`
- `promptCapabilities.audio = true`
- `promptCapabilities.embeddedContext = true`
- `mcpCapabilities.http = true`
- `mcpCapabilities.sse = true`

This is already a materially better foundation than the previously probed Goose
ACP surface.

## 2. Session creation works without special adapter glue

`session/new` succeeded directly against `gemini --acp` for the repo working
directory:

```json
{"jsonrpc":"2.0","id":2,"method":"session/new","params":{"cwd":"/Users/user/Documents/Chainworks Forge","mcpServers":[]}}
```

Observed response properties:

- real `sessionId`
- available modes:
  - `default`
  - `autoEdit`
  - `yolo`
  - `plan`
- available models including:
  - `auto-gemini-3`
  - `auto-gemini-2.5`
  - `gemini-3.1-pro-preview`
  - `gemini-3-flash-preview`
  - `gemini-2.5-pro`
  - `gemini-2.5-flash`
  - `gemini-2.5-flash-lite`
- initial `currentModelId = auto-gemini-3`
- initial `currentModeId = default`

This is usable transport truth for session bootstrapping.

## 3. Prompt execution emits real ACP updates

Prompt probe:

```json
{"jsonrpc":"2.0","id":4,"method":"session/prompt","params":{"sessionId":"...","prompt":[{"type":"text","text":"Reply with exactly: ready"}]}}
```

Observed behavior:

- `session/update` with `available_commands_update`
- `session/update` with `agent_message_chunk`
- prompt result with `stopReason = end_turn`
- per-prompt usage surfaced in `_meta.quota.token_count`
- per-prompt model truth surfaced in `_meta.quota.model_usage`

Example observed result:

- prompt output chunk: `ready`
- model usage reported: `gemini-3.1-pro-preview`

Unlike Goose ACP, Gemini CLI ACP did not collapse prompt flow into a silent
transport-only success. It exposed usable live output.

## 4. Thought streaming is real

When model selection was forced to `gemini-2.5-flash-lite`, the prompt flow
emitted multiple `agent_thought_chunk` updates before the final answer, then an
`agent_message_chunk`.

That matters because Forge’s live runtime surfaces benefit from richer event
streams than just final text.

## 5. Session replay through `loadSession` is real

After a successful prompt, `session/load` replayed history using
`session/update` notifications.

Observed replay included:

- `user_message_chunk`
- `agent_thought_chunk`
- `agent_message_chunk`
- `available_commands_update`

This is a meaningful result for Forge:

- load/resume is not a metadata-only operation
- prior conversation can be reconstructed through ACP updates

This is a major improvement over Goose ACP for resume/recovery surfaces.

## 6. Active-session model mutation works for execution truth

Probe:

1. `session/new`
2. `session/set_model(gemini-2.5-flash-lite)`
3. `session/prompt`

Observed result:

- `session/set_model` returned success
- the subsequent prompt result reported:
  - `_meta.quota.model_usage[0].model = "gemini-2.5-flash-lite"`

So active-session model mutation is semantically real for prompt execution.

### But durable model truth is still not proven

After `set_model` and a successful prompt, `session/load` still reported:

- `currentModelId = auto-gemini-3`

This means Gemini CLI ACP currently appears to have two different truths:

- execution-time model truth in prompt usage
- persisted session model truth in session metadata

Forge cannot treat that as fully settled runtime truth yet.

## 7. Mode mutation is not durably reflected in `loadSession`

Probe:

1. `session/new`
2. `session/set_mode(plan)`
3. `session/load`

Observed result:

- `session/set_mode` returned success
- `session/load` still reported `currentModeId = default`

This is a real gap for Forge.

`P026` can work around this during migration, but it cannot assume that
Gemini’s persisted session metadata is the canonical source of session mode
truth after mutation.

## 8. Session durability is lazy

When `session/set_model` was called on a fresh session before any prompt, then
`session/load` was attempted, Gemini CLI ACP returned:

- `Internal error`
- details said the session identifier was invalid and the runtime searched in
  `~/.gemini/tmp/.../chats`

That strongly suggests a new session is not durably resumable until actual
conversation content is written.

For Forge this means:

- in-memory active session IDs are not equivalent to durable reloadable session
  IDs
- `RunStartSnapshot` and `AgentExecution` truth must continue to distinguish
  “session created” from “session durably reloadable”

## 9. Usage telemetry is useful but non-canonical

Gemini CLI’s prompt result surfaced usage under:

- `result._meta.quota.token_count`
- `result._meta.quota.model_usage`

The local package schema also supports ACP `usage_update`, but it did not appear
in these probes.

Implication:

- Gemini CLI already exposes useful quota truth
- but Forge should treat this as adapter-parsed telemetry, not yet as a stable
  canonical ACP usage contract

## 10. Tool-call visibility is real

Gemini CLI ACP is not limited to text chunks.

A safe shell-tool probe:

- prompt: `Run the command pwd and tell me the result.`

Observed ACP behavior:

- `tool_call` in progress for `pwd`
- `tool_call_update` completed for `pwd`
- tool result surfaced back through ACP content
- final assistant message followed the tool result

Important nuance:

- this shell probe did **not** trigger `session/request_permission`
- so permission requests are not universal for all tool calls
- they appear to remain risk-sensitive and tool-specific

## 11. Edit flows prove permission callbacks and ACP file reads

Edit probes were run in two modes.

### `autoEdit` mode

Probe:

1. `session/new`
2. `session/set_mode(autoEdit)`
3. prompt to create `./acp_fs_probe.txt`

Observed ACP behavior:

- `tool_call` for `write_file`
- `tool_call_update` completed with a diff payload
- file was actually created on disk with the requested content

But:

- no `fs/write_text_file` callback was observed in this successful path

So the edit flow is clearly real and observable, but the exact write-path
ownership is still ambiguous.

### `default` mode

Probe:

1. `session/new`
2. prompt to use `write_file` for a new file in the working directory

Observed ACP behavior:

- `fs/read_text_file` was requested for the target path before writing
- the first write attempt failed after the client returned `ENOENT`
- Gemini retried and then raised `session/request_permission`
- permission options included:
  - `allow_always`
  - `allow_once`
  - `reject_once`
- after selecting `allow_once`, the write completed successfully
- a diff-backed `tool_call_update` surfaced the created file contents

This is a strong result for `P026`:

- permission callbacks are real
- ACP client-side file reads are real
- edit diffs are visible through ACP updates

What is still not proven:

- whether successful writes should always go through `fs/write_text_file`
- or whether Gemini sometimes settles writes inside a different internal path

## 12. MCP server injection and MCP tool execution are real

A local stdio MCP probe server was attached during `session/new` using a
temporary Python `FastMCP` server with one tool:

- `forge_probe_echo(text: str) -> str`

Prompt:

- `Use the forge_probe_echo tool with text exactly hello-gemini-acp and then tell me the returned value.`

Observed behavior:

- Gemini ACP session started successfully with `mcpServers`
- the MCP probe server log showed a real tool invocation:
  - `forge_probe_echo("hello-gemini-acp")`
- ACP surfaced a completed `tool_call_update`
- the returned MCP value was visible in the ACP event content:
  - `forge-probe-echo:hello-gemini-acp`

Important nuance:

- the observed MCP trace showed a completed `tool_call_update`
- it did **not** show a prior pending `tool_call`

So MCP execution is real and visible, but the event lifecycle may be more
compressed than Forge’s current live surfaces expect.

## 13. File-system proxy, permission callbacks, and MCP tool-use are now mostly proven

What is now live-proved:

- `session/request_permission`
- ACP `fs/read_text_file`
- edit diffs through `tool_call_update`
- real MCP attachment through `mcpServers`
- real MCP tool execution over ACP

What remains open:

- `fs/write_text_file` specifically
- MCP error classification and preflight-failure shape
- whether longer tool-heavy sessions emit `usage_update`

These are still live research gaps for `P026`.

## Compatibility Assessment

### Works

- `gemini --acp` startup
- `initialize`
- `session/new`
- `session/prompt`
- `session/load` after a real prompt
- live `session/update` text/thought replay
- active-session model mutation for the next prompt
- tool-call visibility through ACP updates
- permission callback flow for edit operations
- `fs/read_text_file` callback flow
- MCP session attachment and real MCP tool execution

### Works but degraded

- prompt usage truth is carried in `_meta.quota`, not a clearly stabilized ACP
  `usage` field
- `available_commands_update` is emitted eagerly and may add noise to Forge
  live timeline surfaces
- `loadSession` metadata does not reflect post-mutation mode/model truth
- successful edit writes did not surface `fs/write_text_file`, so exact write
  ownership is still ambiguous
- observed MCP tool traces surfaced a completed update, but not a full pending
  lifecycle

### Broken or insufficient for current Forge needs

- durable persisted `currentModeId` after `setSessionMode`
- durable persisted `currentModelId` after `setSessionModel`
- fresh-session reloadability before any prompt has been written

### Unknown

- whether `fs/write_text_file` is part of the canonical successful edit path
- MCP error handling and preflight behavior under bad auth/bad transport inputs
- whether `usage_update` is emitted under longer multi-turn/tool-heavy sessions
- whether session fork/resume/close behave correctly for Forge recovery cases

## Impact on Proposal 026

This research materially strengthens `P026`.

What it proves:

- Forge does not need to stay Goose-shaped to get usable live prompt streaming
- Gemini CLI ACP can supply real session creation, prompt flow, and replay
  semantics today
- ACP-level prompt observability is already good enough to justify an ACP-first
  core transport seam
- callback-heavy paths are materially more proven than in prior OpenCode ACP
  evidence:
  - permission callbacks
  - file-read callbacks
  - MCP tool execution

What it does **not** yet prove:

- Gemini CLI ACP is already good enough to become Forge’s sole runtime adapter
- session metadata mutation truth is reliable enough for canonical report truth
- successful file writes always settle through the ACP client file-system proxy
- MCP preflight failure classification is stable enough for run-owned reports

Recommended proposal consequence:

1. Treat Gemini CLI ACP as a top-tier `P026` migration candidate.
2. Do **not** collapse Forge runtime truth into Gemini session metadata.
3. Keep Forge-owned requested/predicted/actual settlement truth above ACP.
4. Require a follow-up live probe specifically for:
   - `fs/write_text_file`
   - bad-MCP preflight and auth failures
   - long-lived multi-turn session replay

## Practical Verdict

For the transport slice that matters most to `P026`, Gemini CLI ACP currently
looks stronger than Goose ACP and, on current evidence, stronger than OpenCode
ACP for observable ACP execution truth.

Its main weakness is not lack of ACP capability. Its main weakness is durable
session-config truth and exact write-path settlement:

- prompt-time truth is better than persisted metadata truth
- live replay is stronger than durable session settlement
- edit success is observable, but `fs/write_text_file` is still not explicitly
  proven as the canonical write path

That is acceptable for `P026` only if Forge keeps ownership of canonical
runtime/report truth and treats Gemini ACP session metadata as adapter evidence,
not final authority.

## Sources

- [Gemini CLI ACP Mode](https://geminicli.com/docs/cli/acp-mode/)
- [Gemini CLI IDE Integration](https://geminicli.com/docs/ide-integration/)
- [Gemini CLI Telemetry](https://geminicli.com/docs/cli/telemetry/)
- [Gemini CLI Latest Stable Release](https://geminicli.com/docs/changelogs/latest/)
- [Agent Client Protocol Introduction](https://agentclientprotocol.com/protocol/overview)
- [ACP Agent Registry](https://agentclientprotocol.com/registry)
