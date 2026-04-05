# Cline CLI ACP Research

Status: **Live-Probed** (2026-04-04)

## Purpose

This note evaluates whether `Cline CLI ACP` is a realistic execution substrate
for `Chainworks Forge` under
[`Proposal 026`](../proposals/026-acp-first-runtime-transport-and-goose-decoupling.md),
with specific focus on:

- ACP transport fidelity
- authenticated prompt execution
- event and permission observability
- session replay and mutation truth
- MCP attachment and runtime integration surface
- fit for Forge runtime transport decoupling

The goal is not to judge Cline in general. The goal is to decide whether its
ACP runtime is currently a credible candidate for Forge's post-Goose transport
seam.

## Executive Summary

Cline CLI ACP is now a **real, execution-proven** `P026` candidate, but it is
still not among the top three options.

What is now solid on current evidence:

- `cline` is installable and runnable locally
- ACP mode is official and documented
- `initialize` works against the live CLI
- the live ACP handshake exposes explicit auth methods
- authenticated `session/new` works
- authenticated `session/prompt` works
- ACP surfaced real:
  - `agent_message_chunk`
  - `tool_call`
  - `tool_call_update`
  - `plan`
- `session/set_mode` and `session/set_model` both return success responses

What is still not good enough for Forge:

- `initialize` advertises `loadSession = true`, but live `session/load`
  returned `Method not found`
- authenticated prompts did **not** surface `session/request_permission` in the
  observed paths
- edit-path proof is weak:
  - an edit prompt completed with `stopReason = end_turn`
  - tool events were emitted
  - but the target file remained unchanged
- `fs/read_text_file` and `fs/write_text_file` were not observed
- `mcpServers` is still unproven live and the docs remain internally
  contradictory about MCP support
- no `agent_thought_chunk` or usage telemetry were observed in the successful
  probes

Bottom line:

- Cline is no longer merely docs-strong
- it has a real authenticated ACP runtime
- but broken replay truth and weak edit/MCP proof keep it below Junie

## Method

The evaluation used four evidence sources:

1. Official Cline documentation
2. Local CLI install and help-surface inspection
3. Same-machine local ACP handshake and execution probes
4. Authentication behavior checks against the live CLI

Probe environment:

- macOS desktop environment
- local install command:

```text
npm install -g cline
```

- local CLI version:

```text
2.13.0
```

- probe workspace: `/Users/user/Documents/Chainworks Forge`
- client: temporary JSON-RPC stdio probe

## Official Documentation Findings

### ACP editor integration is official and explicitly positioned as full Cline

Official docs:

- [ACP: Editor Integrations](https://docs.cline.bot/cline-cli/acp-editor-integrations)

The ACP page states that Cline CLI supports ACP and that this gives access to
"the full Cline agent" including:

- Skills
- Hooks
- MCP integrations

That is a strong product signal for Forge because it frames ACP as a real agent
runtime, not a thin chat bridge.

### The SDK docs give an unusually explicit ACP lifecycle

Official docs:

- [Cline SDK overview](https://docs.cline.bot/cline-sdk/overview)

The SDK page explicitly documents:

```text
initialize() → authenticate() → newSession() → prompt() ⇄ events → shutdown()
```

It also explicitly names:

- `setPermissionHandler`
- `emitterForSession(sessionId)`
- `agent_message_chunk`
- `agent_thought_chunk`
- `tool_call`
- `error`

This is unusually good documentation and maps closely to Forge runtime needs.

### There is still a real documentation contradiction around MCP

The ACP integration page implies full MCP integration.

But the SDK docs say:

- `mcpServers` field not supported yet, but exposed to maintain ACP conformance

This contradiction matters for `P026`, because Forge cares about real MCP
runtime truth, not just conformance-shaped request fields.

## Local CLI Findings

Observed locally:

- binary path: `/opt/homebrew/bin/cline`
- `cline --help` exposes:
  - `--acp`
  - `auth`
  - `mcp`
  - hooks directory support
  - plan/act/yolo modes
  - model selection
  - auto-condense

Important runtime signals from the local help surface:

- ACP is a top-level mode
- hooks are first-class
- MCP is first-class
- mode/model/config control are first-class

## Live ACP Probe Findings

## 1. Handshake is real and usable

Observed live `initialize` result from `cline --acp`:

- `protocolVersion = 1`
- `loadSession = true`
- `promptCapabilities.image = true`
- `promptCapabilities.embeddedContext = true`
- `mcpCapabilities.http = true`
- `mcpCapabilities.sse = false`
- `agentInfo.name = cline`
- `agentInfo.version = 2.13.0`

Observed live `authMethods`:

- `cline-oauth`
- `openai-codex-oauth`

This is a strong ACP envelope.

## 2. Authentication and session creation are real

Earlier in the same-machine pass, `session/new` failed with:

```text
Authentication required
```

After the auth flow settled, a fresh live probe succeeded with:

- real `sessionId`
- modes:
  - `plan`
  - `act`
- `currentModeId = act`
- `currentModelId = openai-codex/gpt-5.3-codex`

Important nuance:

- the earlier browser callback problem was real
- but current-state probing confirms that authenticated session creation is now
  usable on this machine

## 3. Prompt execution is real

Simple prompt:

- `Reply with exactly one lowercase word: ready`

Observed result:

- `stopReason = end_turn`
- ACP emitted:
  - `agent_message_chunk`

More tool-oriented prompts emitted:

- `tool_call`
- `tool_call_update`
- `plan`

This is enough to classify Cline as execution-proven rather than docs-only.

## 4. Replay truth is currently broken or overstated

This is the most important negative finding.

Observed behavior:

- `initialize` advertises `loadSession = true`
- but live `session/load` returned:

```text
Method not found
```

That is a direct contradiction between the capability envelope and the actual
runtime method surface.

For Forge, this is a serious issue because `P026` depends on truthful runtime
capabilities and replay semantics.

## 5. Mutation truth is weak

Probe:

- `session/set_mode("plan")`
- `session/set_model("openai-codex/gpt-5.3-codex")`

Observed result:

- both calls returned `{}` success

What could not be proven:

- whether these mutations durably settle into a replayable session truth

Because `session/load` is not actually callable, Cline currently lacks a
trustworthy way to confirm settled post-mutation session state through ACP.

## 6. Edit/tool path is only partially proven

Edit probe:

- `Append the line cline-edit-proof to /tmp/cline_edit_probe.txt and then reply done.`

Observed behavior:

- prompt completed with `stopReason = end_turn`
- ACP emitted many `tool_call_update` events plus `plan` and final
  `agent_message_chunk`

But the target file remained:

```text
start
```

That means current evidence does **not** prove a successful ACP-observed edit
path, even though tool lifecycle events are clearly real.

Also not observed:

- `session/request_permission`
- `fs/read_text_file`
- `fs/write_text_file`

So Cline shows live tool-event observability, but the current edit path is not
yet strong proof of trustworthy file mutation settlement.

## 7. MCP is still unresolved

The live pass did **not** prove:

- `mcpServers` attachment
- MCP tool execution

Given the doc contradiction around `mcpServers`, this remains an open runtime
question, not just a missing extra check.

## Candidate Assessment For Proposal 026

Cline ACP currently fits `P026` as a **credible but second-tier candidate**.

Why it is interesting:

- strong official ACP narrative
- explicit SDK lifecycle
- authenticated live session creation and prompt execution are real
- live tool-event stream is real

Why it is not ready to lead:

- replay truth is broken or overstated (`loadSession` capability vs method)
- edit settlement proof is weak
- MCP support is still unproven and doc-conflicted
- mutation truth cannot be verified through replay

Practical role on current evidence:

- stronger than OpenCode for live ACP execution/observability
- stronger than Goose
- weaker than Junie because Junie proved:
  - permission callbacks
  - MCP execution
  - stronger practical runtime settlement

Current placement for `P026`:

1. Claude Agent ACP
2. Gemini CLI ACP
3. Junie CLI ACP
4. Cline CLI ACP
5. OpenCode ACP
6. Goose ACP

## Recommended Next Probe

The next Cline pass should be narrow:

1. identify whether replay uses a different ACP method name than `session/load`
   or whether `loadSession` is incorrectly advertised
2. prove at least one successful file mutation path with observable settlement
3. verify whether any prompt path triggers:
   - `session/request_permission`
   - `fs/read_text_file`
   - `fs/write_text_file`
4. verify whether `mcpServers` is truly unsupported or only unsupported in part
   of the stack

Until that happens, Cline remains strategically interesting, but not strong
enough to outrank Junie.

## Sources

- [ACP: Editor Integrations](https://docs.cline.bot/cline-cli/acp-editor-integrations)
- [Cline SDK overview](https://docs.cline.bot/cline-sdk/overview)
- [Installing Cline](https://docs.cline.bot/getting-started/installing-cline)
- [Agent Client Protocol Overview](https://agentclientprotocol.com/protocol/overview)
