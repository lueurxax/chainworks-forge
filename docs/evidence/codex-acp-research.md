# Codex ACP Research

Status: **Live-Probed** (2026-04-06)

## Purpose

This note evaluates whether `Codex ACP` is a realistic ACP runtime candidate
for `Chainworks Forge`, using the same evidence bar applied to the other
runtime-adapter probes:

- ACP transport fidelity
- session creation and replay truth
- prompt streaming and usage signals
- mode/model mutation truth
- tool and permission visibility
- MCP attachment behavior
- fit for Forge runtime/report/recovery needs

Important framing:

- this is **not** a new native OpenAI runtime protocol
- it is an ACP adapter package built by Zed Industries around `Codex CLI`

The goal is not to decide whether Codex is a good coding agent in general.
The goal is to decide whether `codex-acp` is a credible ACP candidate for
Forge’s runtime seam and where it still diverges from Forge needs.

## Executive Summary

`codex-acp` is a **strong, real, execution-proven ACP runtime candidate**, but
it is not yet a top-tier one.

What is already solid on current evidence:

- `codex-acp` is real, installable, and runnable locally
- stdio transport is real and uses newline-delimited JSON
- `initialize` works and exposes a strong capability surface
- `session/new` is real and returns rich mode/model/config truth
- `session/list` is real and returns real persisted Codex sessions
- `session/prompt` emits real `session/update` streaming
- `session/prompt` emits real `agent_message_chunk`
- `session/prompt` emits real `usage_update`
- `session/set_model` is real and returns live `config_option_update`
- `session/set_mode` is real and returns live `config_option_update`
- `session/load` is real for persisted sessions
- persisted-session `load` can replay prior user and assistant transcript chunks
- auth coupling to the local `Codex CLI` login is real and works

What is still not good enough for a clean top-tier placement:

- `session/load` for a fresh session fails before prompt persistence with
  `Resource not found`
- mode/model mutation truth is proven live, but not fully proven as durably
  persisted across a mutated-session reload
- tool-call event visibility was **not** proven in this pass
- permission callback proof was **not** observed in this pass
- MCP attachment shape is stricter than the generic probe shape, and real MCP
  tool execution was not completed in this pass
- runtime startup inherits noisy local MCP startup warnings for unrelated local
  servers (`xcode`, `codex_apps`)

Bottom line:

- `codex-acp` is much stronger than Goose ACP for Forge-style timeline and
  replay needs
- it is competitive with the second tier
- but it does **not** currently displace Claude Agent ACP or Gemini CLI ACP
  because tool/permission/MCP proof is still incomplete

## Method

This evaluation used four evidence sources:

1. Official/public package and product documentation
2. Local package inspection
3. Same-machine local runtime probes against installed binaries
4. Live stdio ACP probes against:

```text
/opt/homebrew/bin/codex-acp
```

Probe environment:

- macOS desktop environment
- local `codex-cli` version: `0.98.0`
- local `codex-acp` version: `0.11.1`
- probe workspace: `/Users/user/Documents/Chainworks Forge`
- client: temporary Python JSON-RPC stdio probe

Observed local auth state:

- `codex login status` returned `Logged in using ChatGPT`

Installation performed for this research:

```text
npm install -g @zed-industries/codex-acp
```

## Official Documentation Findings

Primary public sources:

- [@zed-industries/codex-acp on npm](https://www.npmjs.com/package/@zed-industries/codex-acp)
- [zed-industries/codex-acp](https://github.com/zed-industries/codex-acp)
- [Zed External Agents](https://zed.dev/docs/ai/external-agents)
- [Agent Client Protocol Overview](https://agentclientprotocol.com/protocol/overview)
- [Agent Client Protocol Schema](https://agentclientprotocol.com/protocol/schema)

The package positions itself as:

- an ACP-compatible coding agent
- powered by Codex
- usable from ACP-compatible clients such as Zed

The public README explicitly claims support for:

- tool calls with permission requests
- context `@`-mentions
- images
- following
- edit review
- TODO lists
- slash commands
- client MCP servers
- auth via ChatGPT, `CODEX_API_KEY`, or `OPENAI_API_KEY`

That is a strong product claim set. The live research below is about how much of
that claim set is actually visible through ACP in practice.

## Local Package Findings

Observed from the installed/local package:

- package name: `@zed-industries/codex-acp`
- version: `0.11.1`
- binary entrypoint: `codex-acp`
- wrapper path: `/opt/homebrew/bin/codex-acp`
- repository: `https://github.com/zed-industries/codex-acp`

The local README confirms:

- the adapter is built by Zed Industries
- it is an ACP adapter around Codex CLI
- it supports:
  - tool calls
  - permission requests
  - edit review
  - slash commands
  - client MCP servers
  - ChatGPT / API-key auth methods

Important implication:

`codex-acp` is not a trivial launcher. It is an explicit ACP adapter over the
Codex runtime surface.

## Live ACP Probe Findings

## 1. Handshake is real and usable

Observed `initialize` result:

- `protocolVersion = 1`
- `agentInfo.name = "codex-acp"`
- `agentInfo.title = "Codex"`
- `agentInfo.version = "0.11.1"`
- `loadSession = true`
- `promptCapabilities.image = true`
- `promptCapabilities.embeddedContext = true`
- `mcpCapabilities.http = true`
- `mcpCapabilities.sse = false`
- `sessionCapabilities.list = {}`
- `sessionCapabilities.close = {}`
- `auth.logout = {}`

Observed auth methods:

- ChatGPT login
- `CODEX_API_KEY`
- `OPENAI_API_KEY`

This is already stronger than several peers because auth methods are surfaced
explicitly in the live handshake.

## 2. Local CLI auth coupling is real

Local runtime check:

```text
codex login status
```

Observed result:

- `Logged in using ChatGPT`

The adapter then worked without separately injecting API keys into the ACP
probe process.

This means `codex-acp` really is coupled to the local Codex CLI auth state in a
usable way.

## 3. Session creation exposes a strong runtime envelope

`session/new` succeeded and returned:

- real `sessionId`
- modes:
  - `read-only`
  - `auto`
  - `full-access`
- models:
  - multiple GPT-5.4 / GPT-5.4-Mini / Codex variants
- config options:
  - `mode`
  - `model`
  - `reasoning_effort`

This is a strong result for Forge because it exposes a first-class runtime
envelope rather than hiding mode/model selection in opaque defaults.

Observed nuance:

- new-session startup also emitted `available_commands_update`
- that command catalog included live slash-command/runtime commands such as:
  - `review`
  - `review-branch`
  - `review-commit`
  - `init`
  - `compact`
  - `undo`
  - `logout`

## 4. Session listing is real and useful

`session/list` succeeded and returned a real list of Codex sessions with:

- `sessionId`
- `cwd`
- `title`
- `updatedAt`

This is stronger than several peers because it proves:

- session discovery is real
- persisted session identity is visible
- replay candidates can be operator-discoverable

## 5. Prompt execution emits real ACP stream updates

Probe prompt:

- `Reply with exactly PONG and nothing else.`

Observed behavior:

- `agent_message_chunk` streamed in two chunks:
  - `P`
  - `ONG`
- `usage_update` surfaced:
  - `used`
  - `size`
- prompt result returned:
  - `stopReason = end_turn`

This is a strong live result:

- prompt streaming is real
- chunked assistant output is real
- usage telemetry is real

## 6. Mode and model mutation are real at live-session level

Live probes:

- `session/set_model` to `gpt-5.4-mini/medium`
- `session/set_mode` to `auto`

Observed behavior:

- both methods returned success
- both emitted `config_option_update`
- the update reflected:
  - `model.currentValue = gpt-5.4-mini`
  - `reasoning_effort.currentValue = medium`
  - `mode.currentValue = auto`

This is a stronger mutation story than Gemini, Junie, and Auggie at the
live-session layer.

What is **not yet fully proven**:

- whether those mutations durably survive reload after persistence in every case

## 7. `session/load` is real, but the reload contract is split

### 7.1 Fresh-session reloadability is weak

When `session/load` was called against a just-created session before prompt
persistence, the adapter returned:

- `Resource not found`

That means fresh-session reloadability is **not** robust on current evidence.

### 7.2 Persisted-session replay is strong

When `session/load` was called against:

- a real session surfaced by `session/list`
- and a newly-created session after a completed prompt and process restart

observed behavior included:

- replayed `user_message_chunk`
- replayed `agent_message_chunk`
- loaded mode/model/config envelope
- replayed `available_commands_update`

This is one of the stronger replay results in the ACP field.

It means `codex-acp` does not merely reload metadata; it can replay meaningful
conversation truth for persisted sessions.

## 8. Tool, edit, and permission proof are still incomplete

Two short probes were attempted in temporary workspaces:

- `Run pwd and then reply with exactly TOOL-DONE.`
- `Append the line patched-by-codex-acp to note.txt, then tell me DONE.`

Observed behavior:

- the runtime streamed planning/progress text
- the runtime clearly inherited the same local Codex discipline and startup
  behavior seen elsewhere
- but these short probes did **not** reach completed `tool_call` /
  `tool_call_update`
- the temp file remained unchanged in the edit probe

So current evidence does **not** yet prove:

- tool-call event visibility
- permission callbacks
- edit settlement

That is a meaningful research gap, not a soft assumption.

## 9. MCP attachment is schema-strict, but live tool execution remains unproven

Two MCP attachment attempts were made.

What was observed:

- `mcpServers` is validated strictly
- a generic probe shape like `{name, command, args}` failed with:
  - `Invalid params`
  - `data did not match any variant of untagged enum McpServer`

This means the adapter is not silently ignoring MCP attachment config.
It has a real typed MCP contract.

What is still not proven:

- accepted `mcpServers` shape through to operational MCP tool execution
- real MCP tool invocation visibility

So the honest current verdict is:

- MCP support is **present and schema-real**
- real MCP execution is **not yet live-proven in this pass**

## 10. Runtime startup inherits noisy local MCP state

Multiple probes surfaced warnings such as:

- `Received event for unknown submission ID`
- local MCP startup for `codex_apps`
- local MCP startup for `xcode`
- `xcode` timing out on `tools/list`

Important nuance:

- these warnings did **not** prevent session creation or prompt execution
- but they do add operator/runtime noise

For Forge this matters because:

- a runtime can be functionally good
- while still producing low-signal startup noise that would pollute recovery or
  live timeline surfaces if not normalized

## Fit For Forge

`codex-acp` is a credible Forge runtime candidate because it already proves:

- real stdio ACP transport
- strong handshake/auth truth
- strong session envelope truth
- real prompt streaming
- real usage telemetry
- real persisted-session replay

That combination makes it materially stronger than Goose ACP for the slices that
matter most to Forge’s live timeline and run inspection surfaces.

Its current ceiling is limited by missing proof in the more operational lanes:

- tool lifecycle
- permission callbacks
- edit settlement
- MCP tool execution

So today it looks like:

- stronger than Goose
- stronger than OpenCode on ACP-visible replay/streaming truth
- competitive with the current second tier
- still weaker than Claude and Gemini on end-to-end operator-grade proof

## Recommendation

Treat `codex-acp` as a **strong second-tier candidate** for Forge’s ACP runtime
research.

Practical recommendation:

1. Keep `Claude Agent ACP` and `Gemini CLI ACP` ahead of it for first-wave
   implementation priority.
2. Add `codex-acp` to the comparison set as a serious contender.
3. If Forge later wants a more Codex-native ACP lane, do one deeper pass on:
   - completed tool-call lifecycle
   - permission callbacks
   - accepted MCP server shape and real MCP tool execution
   - durable mutation truth after persisted reload

## Sources

- [@zed-industries/codex-acp on npm](https://www.npmjs.com/package/@zed-industries/codex-acp)
- [zed-industries/codex-acp](https://github.com/zed-industries/codex-acp)
- [Zed External Agents](https://zed.dev/docs/ai/external-agents)
- [Agent Client Protocol Overview](https://agentclientprotocol.com/protocol/overview)
- [Agent Client Protocol Schema](https://agentclientprotocol.com/protocol/schema)
