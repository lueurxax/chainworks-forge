# Auggie CLI ACP Research

Status: **Live-Probed** (2026-04-04)

## Purpose

This note evaluates whether `Auggie CLI ACP` is a realistic execution
substrate for `Chainworks Forge` under
[`Proposal 026`](../proposals/026-acp-first-runtime-transport-and-goose-decoupling.md),
with specific focus on:

- ACP transport fidelity
- authentication and headless execution viability
- session lifecycle and replay surface
- permission and tool-call observability
- MCP attachment and runtime integration surface
- fit for Forge runtime transport decoupling

The goal is not to judge Auggie in general. The goal is to decide whether its
ACP runtime is currently a credible candidate for Forge's post-Goose transport
seam.

## Executive Summary

Auggie CLI ACP is now a **real, execution-proven candidate** for `P026`, and
it is materially stronger than it first looked.

What is solid on current evidence:

- `auggie` is installable and runnable locally
- ACP mode is official and documented
- ACP over stdio is official and explicitly described as JSON-RPC over stdin and
  stdout
- headless auth is now proven on this machine via the CLI session token path
- authenticated `session/new` works
- authenticated `session/prompt` works
- the live runtime handshake is real and returns a healthy envelope
- `session/list` works
- `session/load` works when called with:
  - `sessionId`
  - `cwd`
  - `mcpServers`
- `session/load` replays prior transcript chunks
- ACP surfaced real:
  - `session/request_permission`
  - `available_commands_update`
  - `agent_message_chunk`
  - `user_message_chunk`
  - `tool_call`
  - `tool_call_update`
- edit execution is real and settles truthfully into the target file
- client-supplied MCP server attach is real
- real MCP tool execution is proven
- the local package contains a broad ACP method and event vocabulary, including:
  - `session/load`
  - `session/list`
  - `session/set_mode`
  - `session/set_model`
  - `session/request_permission`
  - `fs/read_text_file`
  - `fs/write_text_file`
  - `agent_message_chunk`
  - `agent_thought_chunk`
  - `tool_call_update`
- skills, rules, permissions, MCP config, plugin directories, and session
  resume/share are all first-class in the CLI surface

What is still not good enough for Forge:

- `session/load` is stricter than many peers:
  - it requires `cwd`
  - it requires `mcpServers`
- persisted `set_mode` truth is not durable in `session/load`
- persisted `set_model` truth is still not proven
- no `agent_thought_chunk` or usage telemetry were observed in the successful
  probes
- ACP `fs/read_text_file` and `fs/write_text_file` callbacks were not observed;
  Auggie handled reads/edits through its own surfaced tool events instead
- official docs explicitly say ACP mode is still incomplete relative to full
  interactive mode

Bottom line:

- Auggie is no longer just a docs-level watch candidate
- it is now stronger than Cline, OpenCode, and Goose for Forge’s ACP-first
  transport seam
- but unresolved persisted mutation truth and missing thought/usage signals keep
  it below Claude and Gemini

## Method

The evaluation used four evidence sources:

1. Official Augment documentation
2. Local CLI install and help-surface inspection
3. Same-machine live ACP handshake probes
4. Local package inspection of the installed runtime

Probe environment:

- macOS desktop environment
- local install command:

```text
npm install -g @augmentcode/auggie
```

- local CLI version:

```text
0.22.0 (commit 053888ee)
```

- probe workspace: `/Users/user/Documents/Chainworks Forge`
- client: temporary Python JSON-RPC stdio probe

Observed local auth/runtime state during the successful pass:

- `auggie token print` returned a valid session payload suitable for:
  - `AUGMENT_SESSION_AUTH`
  - `--augment-session-json`
- after that state was present, the same-machine ACP probes no longer failed on
  authentication

## Official Documentation Findings

### ACP mode is official and explicitly documented

Official docs:

- [ACP Mode](https://docs.augmentcode.com/cli/acp/agent)
- [ACP Clients](https://docs.augmentcode.com/cli/acp/clients)

The official docs explicitly state that:

- Auggie is a "fully compatible" ACP agent
- ACP mode uses JSON-RPC over standard input and output
- ACP clients should launch:

```text
auggie --acp
```

- editor integrations can pass additional CLI arguments and environment
  variables to the agent process

That is a real positive signal for `P026`, because ACP is not an accidental
surface. It is a documented operating mode.

### The docs explicitly warn that ACP mode is still incomplete

The official ACP mode page also states:

- ACP is in active development
- not all features available in interactive mode are supported in ACP mode

That caveat matters for Forge because `P026` is not just about basic prompt
execution. Forge needs live transport truth, replay, permissions, MCP, and
report-grade observability.

### Official docs also expose automation-friendly authentication paths

Official docs:

- [CLI Flags and Options](https://docs.augmentcode.com/cli/reference)
- [Python SDK](https://docs.augmentcode.com/cli/sdk-python)

The CLI reference documents:

- `auggie login`
- `auggie token print`
- `--augment-session-json`
- `AUGMENT_SESSION_AUTH`

The SDK docs also show:

- API-key style initialization in SDK integrations
- streaming listeners such as `AgentEventListener`

That matters because it gives Auggie a plausible non-interactive runtime story
for Forge. The remaining question is not whether such a path exists, but
whether the CLI ACP runtime exposes enough truth once it is authenticated.

## Local CLI Findings

Observed locally:

- binary path: `/opt/homebrew/bin/auggie`
- `auggie --help` exposes:
  - `--acp`
  - `--mcp`
  - `--mcp-config`
  - `--permission`
  - `--persona`
  - `--rules`
  - `--plugin-dir`
  - `--augment-session-json`
  - `--github-api-token`
- environment variables surfaced by the live CLI help:
  - `AUGMENT_SESSION_AUTH`
  - `GITHUB_API_TOKEN`
- explicit auth/session commands:
  - `login`
  - `logout`
  - `token`
  - `session`

The session surface is stronger than a minimal chat shell. Local help exposes:

- `session list`
- `session resume`
- `session continue`
- `session share`
- `session delete`

The broader runtime surface is also strategically relevant for Forge:

- skills load from `.augment/skills/` and `.claude/skills/`
- rules and guidelines are first-class
- MCP config and per-tool permissions are first-class
- plugin directories and marketplaces are first-class

The current machine also proved a real automation-oriented auth path:

- `auggie token print` returned a valid session payload
- the CLI itself suggested:
  - `export AUGMENT_SESSION_AUTH=$SESSION`
  - `auggie --augment-session-json '$SESSION'`

That is materially better than runtimes that only expose browser-only auth.

## Live ACP Probe Findings

## 1. Handshake is real and healthy

Observed live `initialize` result from `auggie --acp`:

- `protocolVersion = 1`
- `agentCapabilities.loadSession = true`
- `promptCapabilities.image = true`
- `sessionCapabilities.list = {}`
- `agentInfo.name = "auggie"`
- `agentInfo.title = "Auggie Agent"`
- `agentInfo.version = "0.22.0 (commit 053888ee)"`
- `authMethods = []`

That is a real ACP server, not a stub.

## 2. Authenticated `session/new` is real, but contract-strict

Auggie enforces a stricter runtime contract than several peers:

- `session/new` required explicit:
  - `cwd`
  - `mcpServers`

Once the machine had valid Augment session state, `session/new` returned:

- real `sessionId`
- available modes:
  - `default`
  - `ask`
- large model catalog including:
  - Claude
  - Gemini
  - GPT
- initial `currentModelId = claude-haiku-4-5`
- initial `currentModeId = default`

This is real transport truth, not a fake shell around a hidden chat session.

## 3. Prompt execution, permission callbacks, and replay are all real

Simple prompt:

```json
{"jsonrpc":"2.0","id":3,"method":"session/prompt","params":{"sessionId":"...","prompt":[{"type":"text","text":"Reply with exactly one lowercase word: ready"}]}}
```

Observed behavior:

- `session/update` with `agent_message_chunk`
- `session/request_permission` for workspace indexing
- after rejecting indexing for the current session, prompt execution continued
- `session/update` with `available_commands_update`
- final prompt result with `stopReason = end_turn`

This is already stronger than runtimes that collapse prompt flow into a silent
final answer.

### `session/load` replay is also real

`session/load` only worked when supplied with:

- `sessionId`
- `cwd`
- `mcpServers`

With those fields present, it:

- resumed the saved session
- replayed `user_message_chunk`
- replayed `agent_message_chunk`
- replayed `available_commands_update`
- returned session envelope metadata

That is meaningful for Forge resume and Live Timeline surfaces.

## 4. Edit execution is real and visibly settled

Edit prompt:

- append a line containing `beta` to `note.txt`
- then reply with `done`

Observed behavior:

- streamed `tool_call` for reading `note.txt`
- streamed `tool_call_update` for the completed read
- streamed `tool_call` for editing `note.txt`
- streamed `tool_call_update` with completed edit result
- final agent reply was `done`
- file contents on disk changed from:
  - `alpha`
  to:
  - `alpha`
  - `beta`

This is strong evidence that Auggie’s ACP mode exposes truthful edit
settlement, not just optimistic tool intent.

## 5. MCP attach and MCP tool execution are real

Using a temporary local stdio MCP probe server, Auggie successfully:

- initialized the supplied MCP server
- logged MCP readiness on stderr
- accepted the client-supplied MCP server in `mcpServers`
- invoked tool `forge_probe_echo`
- surfaced:
  - `tool_call`
  - `tool_call_update`
- returned tool output:
  - `echo:hello-auggie-acp`

This is enough to count as real MCP execution proof for `P026`.

## 6. Mode mutation is semantically real, but not durably reflected

Probe:

1. `session/new`
2. `session/set_model(gpt-5-4)`
3. `session/set_mode(ask)`
4. `session/prompt`
5. `session/load`

Observed result:

- `session/set_model` returned success
- `session/set_mode` returned success
- the subsequent prompt clearly executed under ask-mode-style guidance:
  - replayed prompt included injected ask-mode rules
  - no edit tools were used in the informational prompt
- but `session/load` still reported:
  - `currentModeId = default`
  - `currentModelId = claude-haiku-4-5`

That means Auggie currently has the same broad class of truth gap seen in some
other candidates:

- mutation appears semantically real for execution
- durable session metadata does not necessarily settle to the same truth

## 7. Local package inspection shows a broad ACP/runtime vocabulary

The installed runtime package at:

```text
/opt/homebrew/lib/node_modules/@augmentcode/auggie/augment.mjs
```

contains the following method/event strings:

- `session/load`
- `session/new`
- `session/list`
- `session/set_mode`
- `session/set_model`
- `session/request_permission`
- `fs/read_text_file`
- `fs/write_text_file`
- `agent_message_chunk`
- `agent_thought_chunk`
- `tool_call_update`

That does **not** prove these paths all work live.
But it does prove the local runtime package is shaped like a fuller ACP agent,
not just a thin prompt wrapper.

## What Is Not Yet Proven

This pass did **not** honestly prove:

- `agent_thought_chunk`
- ACP `fs/read_text_file`
- ACP `fs/write_text_file`
- prompt usage telemetry
- durable persisted `set_mode` truth
- durable persisted `set_model` truth
- usage telemetry

Those are exactly the parts that matter most for Forge live timeline,
recovery, and report fidelity.

## Candidate Assessment For Proposal 026

Current position:

- **real second-tier candidate**

Why Auggie is strategically interesting:

- official ACP support is real
- the CLI is rich in runtime-facing surfaces that matter to Forge:
  - rules
  - skills
  - permissions
  - MCP
  - session management
  - plugin extension
- the docs explicitly position Auggie as usable from ACP clients

Why it still ranks lower right now:

- no observed `agent_thought_chunk`
- no observed usage telemetry
- durable mode/model metadata truth is still weak
- file access is proven through tool events, but not through client-owned ACP
  fs callbacks
- docs explicitly warn that ACP mode does not yet match full interactive mode

Current `P026` recommendation:

- keep Auggie on the candidate list
- treat it as stronger than Cline, OpenCode, and Goose on current ACP evidence
- keep it below Claude and Gemini
- compare it directly against Junie for the third slot depending on whether
  Forge values replay and edit settlement more than thought-stream richness

## Recommended Next Probe

If Auggie becomes strategically important later, the next honest pass should do
all of the following on the same authenticated machine:

1. hydrate a stable headless auth path via `AUGMENT_SESSION_AUTH` or
   `--augment-session-json`
2. verify whether `agent_thought_chunk` is ever surfaced on longer prompts
3. verify prompt usage and cost telemetry
4. verify whether `fs/read_text_file` / `fs/write_text_file` are ever used in
   client-owned callback mode
5. verify durable `session/set_mode` and `session/set_model` persistence truth
6. verify multi-turn `session/load` replay across longer sessions

Until that pass exists, Auggie remains a strong fallback-tier candidate rather
than a top-tier cutover target.

## Sources

- [ACP Mode](https://docs.augmentcode.com/cli/acp/agent)
- [ACP Clients](https://docs.augmentcode.com/cli/acp/clients)
- [CLI Flags and Options](https://docs.augmentcode.com/cli/reference)
- [Python SDK](https://docs.augmentcode.com/cli/sdk-python)
- [Agent Client Protocol Overview](https://agentclientprotocol.com/protocol/overview)
