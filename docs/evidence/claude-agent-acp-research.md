# Claude Agent ACP Research

Status: **Live-Probed** (2026-04-04)

## Purpose

This note evaluates whether `Claude Agent ACP` is a realistic execution
substrate for `Chainworks Forge` under
[`Proposal 026`](../proposals/026-acp-first-runtime-transport-and-goose-decoupling.md),
with specific focus on:

- ACP transport fidelity
- prompt streaming and replay visibility
- mode/model mutation truth
- tool and permission callbacks
- MCP attachment and tool realization
- fit for Forge runtime transport decoupling

Important framing:

- this is **not** a native Anthropic ACP runtime
- it is an ACP adapter package built by Zed Industries on top of the official
  Claude Agent SDK and Claude Code runtime

The goal is not to prove that Claude Code is “good in general”. The goal is to
decide whether this ACP adapter is a credible candidate for Forge’s post-Goose
runtime seam and where it still diverges from Forge needs.

## Executive Summary

Claude Agent ACP is a **very strong** `P026` candidate and, on current
evidence, the strongest ACP runtime surface probed so far for Forge’s transport
and observability needs.

What is already solid on current evidence:

- `Claude Code` was already installed locally and authenticated
- the ACP adapter is real, installable, and runnable as `claude-agent-acp`
- stdio transport is real and uses newline-delimited JSON (`ndjson`)
- `initialize` works and exposes a rich capability surface
- `session/new` works and returns usable model, mode, and config-option truth
- `session/prompt` emits real `session/update` notifications
- prompt turns expose `usage` and `usage_update`, including cost and context
  usage
- `session/set_model` persists to `loadSession`
- `session/set_mode` persists to `loadSession`
- edit flows emit real `session/request_permission`
- edit flows surface a rich pending-to-completed diff lifecycle
- shell/tool execution emits rich `tool_call` / `tool_call_update`
- client-supplied MCP servers are attachable and MCP tool execution is real

What is still not good enough for a clean Forge cutover:

- this is an adapter, not a native Anthropic ACP runtime
- `loadSession` replay is only partially proven:
  - observed replay included prior user-side session history
  - observed replay did **not** include assistant message chunks in the same
    way Gemini did
- prompt result usage does not expose a per-prompt model-usage breakdown like
  Gemini `_meta.quota.model_usage`
- ACP file-system client callbacks are implemented in code, but they were not
  live-observed in this pass
- auth methods surfaced as empty in the live `initialize` probe because the
  client did not advertise terminal-auth capability

Bottom line:

- Claude Agent ACP is stronger than Goose ACP
- Claude Agent ACP is stronger than the current OpenCode ACP evidence
- Claude Agent ACP is also stronger than Gemini CLI ACP on persisted
  mode/model/session-config truth
- the main caveat is not transport weakness; it is ownership and provenance:
  Forge would be depending on an ACP adapter around Claude Code, not a native
  Anthropic ACP runtime

## Method

The evaluation used four evidence sources:

1. Official Anthropic documentation for Claude Agent SDK and Claude Code
2. ACP adapter package metadata and local package inspection
3. Same-machine local runtime probes against installed binaries
4. Live stdio ACP probes against:

```text
/opt/homebrew/bin/claude-agent-acp
```

Probe environment:

- macOS desktop environment
- local `Claude Code` version: `2.1.92`
- local `claude-agent-acp` version: `0.25.0`
- probe workspace: `/Users/user/Documents/Chainworks Forge`
- client: temporary Python JSON-RPC stdio probe

Observed local auth state:

- `loggedIn = true`
- `authMethod = claude.ai`
- `apiProvider = firstParty`

Installation performed for this research:

```text
npm install -g @agentclientprotocol/claude-agent-acp@0.25.0
```

## Official Documentation Findings

### Claude-side support is official at the SDK/runtime layer

Official docs:

- [Claude Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview)
- [Claude Code slash commands](https://code.claude.com/docs/en/slash-commands)

These sources matter because the ACP package is **not** Anthropic’s own ACP
runtime. Instead, it is built on top of:

- the official Claude Agent SDK
- the Claude Code runtime and slash-command surface

So the official part of the stack is:

- Claude Code / Claude Agent SDK

while the ACP transport itself is adapter-owned.

### The ACP package is explicit about being an adapter

Primary adapter source:

- [@agentclientprotocol/claude-agent-acp on npm](https://www.npmjs.com/package/@agentclientprotocol/claude-agent-acp)
- [adapter repository](https://github.com/agentclientprotocol/claude-agent-acp)

The package describes itself as:

- an ACP-compatible coding agent
- powered by the Claude Agent SDK
- supporting:
  - tool calls with permission requests
  - edit review
  - TODO lists
  - interactive/background terminals
  - custom slash commands
  - client MCP servers

That positioning is strong, but it also defines the ownership seam clearly:

- Anthropic owns the Claude runtime
- Zed/ACP package owns the ACP bridge

## Local Package Findings

Observed from the installed/local package:

- binary entrypoint: `claude-agent-acp`
- transport uses `ndJsonStream`, not `Content-Length` framing
- request handlers include:
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
- adapter code exposes client methods for:
  - `requestPermission`
  - `readTextFile`
  - `writeTextFile`
- adapter explicitly replays session history on `loadSession`
- adapter includes terminal-aware `tool_call` metadata for Bash tools
- adapter merges ACP `mcpServers` into Claude-side runtime settings

Important implication:

this package is not a thin “launch Claude and hope” shim. It is a substantial
ACP adapter with explicit support for:

- permission callbacks
- edit diff projection
- terminal-aware tool events
- session mutation and replay
- client MCP injection

## Live ACP Probe Findings

## 1. Handshake is real and usable

Observed `initialize` result:

- `protocolVersion = 1`
- `agentInfo.name = "@agentclientprotocol/claude-agent-acp"`
- `agentInfo.version = "0.25.0"`
- `loadSession = true`
- `promptCapabilities.image = true`
- `promptCapabilities.embeddedContext = true`
- `mcpCapabilities.http = true`
- `mcpCapabilities.sse = true`
- session capabilities:
  - `fork`
  - `list`
  - `resume`
  - `close`

Observed nuance:

- `authMethods = []` in the live probe

This was not because Claude was unauthenticated. It happened because the client
probe did not advertise terminal-auth capability, and the adapter gates its
interactive auth methods behind those capabilities.

## 2. Session creation exposes strong envelope truth

`session/new` returned:

- real `sessionId`
- model catalog:
  - `default`
  - `sonnet`
  - `haiku`
- mode catalog:
  - `auto`
  - `default`
  - `acceptEdits`
  - `plan`
  - `dontAsk`
  - `bypassPermissions`
- `configOptions` for both:
  - `mode`
  - `model`

This is stronger than Goose ACP and more explicit than the current Gemini ACP
session envelope because the adapter surfaces config options as first-class
session state.

## 3. Prompt execution emits real stream updates

Probe:

- prompt: `Reply with exactly: ready`

Observed behavior:

- `available_commands_update`
- `agent_message_chunk`
- `usage_update`
- prompt result with `stopReason = end_turn`
- prompt result `usage` object:
  - `inputTokens`
  - `outputTokens`
  - `cachedReadTokens`
  - `cachedWriteTokens`
  - `totalTokens`

The prompt succeeded and streamed usable ACP updates.

This is already materially better than Goose ACP and much stronger than the
current OpenCode ACP evidence, where prompt turns stayed opaque.

## 4. `set_model` and `set_mode` both persist correctly

Probe:

1. `session/new`
2. `session/set_model("haiku")`
3. `session/set_mode("plan")`
4. `session/load`

Observed result:

- `set_model` returned success
- `set_mode` returned success
- `loadSession` reported:
  - `currentModelId = "haiku"`
  - `currentModeId = "plan"`
  - matching `configOptions.currentValue`

This is an important differentiator:

- stronger than Gemini CLI ACP on durable session-config truth
- stronger than Goose ACP on model mutation truth

## 5. `loadSession` replay is real, but partial

Probe:

1. `session/new`
2. prompt: `Reply with exactly: replay-check`
3. clear local notification buffer
4. `session/load`

Observed replay:

- `user_message_chunk`
- `available_commands_update`

Observed replay samples included:

- internal Claude-side `/model` command history
- the user prompt text

Important caveat:

- in this live pass, `loadSession` replay did **not** re-emit assistant message
  chunks the way Gemini CLI ACP did

So replay exists, but it is not yet proven to be transcript-complete enough for
Forge to treat it as fully equivalent to Gemini’s richer replay behavior.

## 6. Shell tool execution is rich and observable

Probe:

- prompt: `Run the command pwd and tell me the result.`

Observed behavior:

- `tool_call` with status `pending`
- multiple `tool_call_update` refinements
- structured tool metadata under `_meta.claudeCode`
- final completed tool update with rendered console content
- assistant message followed the tool result

Observed nuance:

- no ACP `session/request_permission` was raised for this safe shell command

This suggests the adapter/runtime only raises permission callbacks for
sufficiently sensitive operations, rather than for every tool invocation.

## 7. Edit flows prove permission callbacks and rich diff lifecycle

Probe:

- prompt to create `acp_edit_probe.txt` with exact content in a temp directory

Observed behavior:

- one ACP `session/request_permission`
- permission options:
  - `allow_always`
  - `allow_once`
  - `reject_once`
- pending `tool_call`
- multiple `tool_call_update` refinements
- diff content surfaced before completion
- final `tool_call_update` with status `completed`
- file was actually created on disk with requested contents

Observed permission payload included:

- `toolCall.kind = "edit"`
- `rawInput.file_path`
- `rawInput.content`
- diff content with `oldText = null` and `newText = "claude-acp-edit-probe"`

This is one of the strongest results in the whole ACP candidate set:

- permission callbacks are real
- edit diffs are real
- lifecycle is rich enough for Forge Live Timeline and operator surfaces

## 8. MCP server injection and MCP tool execution are real

A local stdio MCP probe server was attached during `session/new` with one tool:

- `forge_probe_echo(text: str) -> str`

Prompt:

- `Use the forge_probe_echo tool with text exactly hello-claude-acp and then tell me the returned value.`

Observed behavior:

- session started successfully with ACP `mcpServers`
- adapter first used a `ToolSearch` step to locate the MCP tool
- then invoked `mcp__probe__forge_probe_echo`
- MCP server log confirmed the real invocation:
  - `forge_probe_echo("hello-claude-acp")`
- ACP surfaced both search and MCP tool execution through `tool_call` /
  `tool_call_update`
- final assistant response used the returned MCP value

This is a strong proof that client-provided MCP servers are not just attached in
configuration; they are operationally usable through the ACP adapter.

## 9. File-system client callbacks exist in the adapter but were not live-proved

The local package clearly implements adapter methods for:

- `readTextFile`
- `writeTextFile`

But in this pass I did **not** observe ACP `fs/read_text_file` or
`fs/write_text_file` requests during the successful prompt flows.

That means:

- file-system proxy support exists in the adapter
- but this pass does not prove that Claude-side edit flows currently depend on
  ACP client file-system callbacks in normal operation

For Forge this is not a blocker by itself, but it remains an unproven seam.

## Compatibility Assessment

### Works

- `claude-agent-acp` startup
- `initialize`
- `session/new`
- `session/prompt`
- `session/load`
- real `session/update` streaming
- `usage_update`
- persisted `set_model`
- persisted `set_mode`
- shell tool execution visibility
- edit permission callbacks
- rich edit diff lifecycle
- MCP attach and MCP tool execution

### Works but degraded

- `loadSession` replay is real, but only partially proven
- prompt result usage is rich, but does not include explicit per-model usage
  breakdown
- auth methods depend on client-advertised terminal-auth capabilities, so a
  non-terminal client may see no auth options even though the runtime is usable

### Broken or insufficient for current Forge needs

- nothing transport-critical in the current probe rose to the same severity as
  Goose/OpenCode gaps

### Unknown

- whether ACP file read/write client callbacks are part of the normal successful
  edit path
- terminal proxy lifecycle under `clientCapabilities.terminal = true`
- whether `loadSession` can replay full assistant/tool history for long
  multi-turn sessions
- how the adapter behaves when Claude auth is absent and terminal-auth is
  required from the client

## Impact on Proposal 026

This research materially strengthens `P026`.

What it proves:

- an ACP-first runtime for Forge does not need to inherit Goose-shaped transport
  semantics
- a Claude-based ACP candidate can provide:
  - strong session envelope truth
  - durable mode/model mutation truth
  - live execution event streams
  - permission callback fidelity
  - MCP execution fidelity

What it does **not** yet prove:

- that Forge should prefer an adapter-owned ACP runtime over a native runtime on
  governance grounds
- that `loadSession` replay is complete enough for all recovery/report cases
- that file-system client callbacks are a proven part of the adapter’s normal
  successful edit path

Recommended proposal consequence:

1. Treat Claude Agent ACP as a top-tier `P026` migration candidate.
2. Explicitly record that it is an adapter around official Claude runtime
   surfaces, not a native Anthropic ACP runtime.
3. Keep Forge-owned run/report truth above adapter session metadata and replay.
4. Require follow-up probes for:
   - transcript completeness on `loadSession`
   - terminal proxy behavior
   - bad-auth / bad-MCP failure classification

## Practical Verdict

Claude Agent ACP is the strongest current ACP candidate for Forge’s live
execution slice.

Compared with the other probed candidates:

- stronger than Goose ACP on virtually every live transport dimension
- stronger than current OpenCode ACP evidence on streamed runtime truth
- stronger than Gemini CLI ACP on persisted mode/model/session-config truth

Its main caveat is structural, not behavioral:

- Forge would be relying on an ACP adapter package around Claude Code and the
  Claude Agent SDK, rather than on a native Anthropic ACP runtime

That is still acceptable for `P026` if the proposal is honest about ownership
layers and keeps canonical product truth above adapter-provided transport state.

## Sources

- [Claude Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview)
- [Claude Code slash commands](https://code.claude.com/docs/en/slash-commands)
- [@agentclientprotocol/claude-agent-acp on npm](https://www.npmjs.com/package/@agentclientprotocol/claude-agent-acp)
- [claude-agent-acp repository](https://github.com/agentclientprotocol/claude-agent-acp)
- [Agent Client Protocol Overview](https://agentclientprotocol.com/protocol/overview)
