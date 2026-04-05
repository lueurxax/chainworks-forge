# Junie CLI ACP Research

Status: **Live-Probed** (2026-04-04)

## Purpose

This note evaluates whether `Junie CLI ACP` is a realistic execution substrate
for `Chainworks Forge` under
[`Proposal 026`](../proposals/026-acp-first-runtime-transport-and-goose-decoupling.md),
with specific focus on:

- ACP transport fidelity
- authenticated prompt execution
- mode/model mutation truth
- session replay viability
- permission and tool-call observability
- MCP attachment and runtime integration surface
- fit for Forge runtime transport decoupling

The goal is not to judge Junie as a product in general. The goal is to decide
whether its ACP runtime is currently a credible candidate for Forge's
post-Goose transport seam.

## Executive Summary

Junie CLI ACP is now a **real, execution-proven candidate** for `P026`, but it
is still not among the top two options.

What is solid on current evidence:

- `junie` is installable and runnable locally
- ACP mode is official and documented
- stdio transport works and uses newline-delimited JSON
- headless CLI execution is now authenticated on this machine
- `initialize`, `session/new`, and `session/load` all work
- `session/prompt` works
- ACP surfaced real:
  - `agent_thought_chunk`
  - `agent_message_chunk`
  - `tool_call`
  - `tool_call_update`
  - `plan`
- `session/request_permission` is real
- client-supplied MCP servers are attachable and real MCP tool execution is
  proven
- edit tasks are real and can complete through ACP-observed tool execution

What is still not good enough for Forge:

- `session/set_model` success is not durably reflected by `session/load`
- `session/set_mode` success is not durably reflected by `session/load`
- `session/load` still exposes only session envelope truth:
  - modes
  - models
  - config options
  and does **not** replay transcript/history
- ACP `fs/read_text_file` and `fs/write_text_file` were not observed in this
  pass, even when edits succeeded
- `usage_update` or prompt-usage telemetry were not observed in this pass

Bottom line:

- Junie is no longer auth-blocked
- Junie is stronger than an opaque ACP shell and clearly stronger than Goose
- but weak persisted session-config truth and missing replay keep it below
  Claude and Gemini for `P026`

## Method

The evaluation used four evidence sources:

1. Official Junie documentation
2. Local CLI install and help-surface inspection
3. Same-machine local CLI execution probes
4. Live stdio ACP probes against:

```text
/Users/user/.local/bin/junie --acp true
```

Probe environment:

- macOS desktop environment
- local install command:

```text
npm install -g @jetbrains/junie-cli@888.77.0
```

- local CLI version:

```text
Junie version: 888.77
```

- probe workspace: `/Users/user/Documents/Chainworks Forge`
- client: temporary Python JSON-RPC stdio probe

Observed local auth/runtime state during the successful pass:

- ordinary headless `junie` execution no longer failed auth
- the same-machine CLI probe returned:

```text
Authenticated successfully
...
TASK RESULT:
READY
```

That matters because it proves the current machine now has a usable headless
Junie auth path, not just an interactive login.

## Official Documentation Findings

### ACP mode is official and explicitly intended for ACP clients

Official docs:

- [Junie for ACP clients](https://junie.jetbrains.com/docs/junie-cli-acp.html)
- [CLI reference](https://junie.jetbrains.com/docs/parameters.html)

These docs explicitly document:

- `junie --acp true`
- ACP over local subprocess stdio
- Junie serving requests initiated by ACP clients

That is a positive fit for `P026`, because ACP is not a community shim or
accidental surface. It is a documented product mode.

### Authentication remains intentionally split across account, token, and BYOK

Official docs:

- [Quickstart](https://junie.jetbrains.com/docs/junie-cli.html)
- [Environment variables](https://junie.jetbrains.com/docs/environment-variables.html)
- [Bring Your Own Key (BYOK)](https://junie.jetbrains.com/docs/byok.html)

The docs define three broad auth paths:

- JetBrains account / subscription path
- `JUNIE_API_KEY` / `--auth`
- BYOK provider keys such as:
  - `JUNIE_ANTHROPIC_API_KEY`
  - `JUNIE_OPENAI_API_KEY`
  - `JUNIE_GOOGLE_API_KEY`

That still matters for Forge, because deployment portability depends on whether
Junie auth can be made machine-stable outside an interactive desktop.

## Local CLI Findings

Observed locally:

- binary path: `/Users/user/.local/bin/junie`
- `junie --help` exposes:
  - `--acp`
  - `--auth`
  - `--project`
  - `--model`
  - BYOK provider key flags
  - MCP location flags

Important product/runtime strengths surfaced by the CLI/docs:

- custom skills
- custom agents
- custom commands
- MCP discovery
- model selection

Those are strategically interesting for Forge, but the ACP execution and replay
surface matters more than product breadth for `P026`.

## Live ACP Probe Findings

## 1. Handshake is real and usable

Observed `initialize` result:

- `protocolVersion = 1`
- `agentInfo.name = "@jetbrains/junie"`
- `agentInfo.version = "888.219"`
- `agentInfo.title = "Junie"`
- `agentCapabilities.loadSession = true`
- `promptCapabilities.image = true`
- `promptCapabilities.embeddedContext = true`
- `mcpCapabilities.http = true`
- `mcpCapabilities.sse = true`

Observed auth methods:

- agent auth method:
  - `jetbrains-account`
- terminal auth method:
  - `junie-cli`

Observed session capabilities:

- `list`
- `resume`

This is a healthy envelope and clearly above the absolute minimum ACP handshake.

## 2. Session creation exposes strong session metadata

`session/new` returned:

- real `sessionId`
- `currentModeId = auto`
- available modes:
  - `auto`
  - `ask`
  - `code`
- `currentModelId = DEFAULT`
- a large available-model catalog including:
  - Gemini
  - Claude
  - GPT
  - Grok
- `configOptions` including:
  - `mode`
  - `model`
  - `brave_mode`
  - `think_more`

This is better than Goose ACP at the session-envelope level and broadly
competitive with richer ACP candidates on initial control truth.

## 3. Authenticated prompt execution is real

Headless CLI proof:

- `junie --project ... --timeout 20000 'Reply with exactly: READY'`

Observed result:

- `Authenticated successfully`
- final task result `READY`

ACP prompt proof:

- simple prompt with a minimal client:
  - `Answer with one lowercase word: ready`

Observed ACP `session/update` notifications included:

- `available_commands_update`
- `agent_thought_chunk`
- `agent_message_chunk`

The prompt completed successfully with:

- `stopReason = end_turn`

This is the key change from the earlier auth-blocked pass: Junie now has a real
same-machine ACP execution path.

## 4. Permission callbacks and tool execution are real

For prompts where Junie chose an execution path rather than direct answer
generation, ACP surfaced:

- `session/request_permission`
- `tool_call`
- `tool_call_update`
- terminal metadata in `_meta`

Observed permission-request shape:

- prompt turn paused for a permission decision
- the response had to use:

```json
{
  "outcome": {
    "outcome": "selected",
    "optionId": "yes"
  }
}
```

That is important for Forge because it means Junie is not merely "running a
tool behind the curtain"; it actually exposes permission and execution events
through ACP.

## 5. Mode/model mutation truth is still weak

Probe:

- `session/set_model("GEMINI_3_0_FLASH")`
- `session/set_mode("ask")`
- `session/load`
- then another successful prompt
- then `session/load` again

Observed result:

- both mutation calls returned `{}` success
- `session/load` still reported:
  - `currentModelId = DEFAULT`
  - `currentModeId = auto`
- `configOptions` also remained at:
  - `model = DEFAULT`
  - `mode = auto`

This means Junie still has the same critical weakness previously seen on the
unauthenticated path: mutation success exists at the RPC layer, but durable
session truth is not confirmed by `loadSession`.

That is a real `P026` concern because Forge cannot treat `session/load` as a
trustworthy replayable source for settled mode/model state.

## 6. `loadSession` replay is still weak

After successful prompt execution, `session/load` still returned only:

- `modes`
- `models`
- `configOptions`

It did **not** return transcript/history replay or prior assistant content.

So Junie currently proves:

- session-envelope reloadability

but does **not** prove:

- transcript replay
- tool-history replay
- prompt-history reconstruction

This keeps it below Gemini for Forge live-report and recovery surfaces.

## 7. MCP server injection and MCP tool execution are real

A local stdio MCP probe server was attached during `session/new` with one tool:

- `forge_probe_echo(text: str) -> str`

Prompt:

- `Use the forge_probe_echo tool with text exactly hello-junie-acp and then tell me the returned value.`

Observed behavior:

- session started successfully with ACP `mcpServers`
- Junie emitted:
  - `plan`
  - `tool_call`
  - `tool_call_update`
- the local MCP probe log showed a real invocation:
  - `forge_probe_echo("hello-junie-acp")`
- the prompt completed successfully with `stopReason = end_turn`

This is a strong proof that client-provided MCP servers are operationally usable
through Junie ACP, not just accepted in configuration.

## 8. Edit execution is real, but not through ACP file callbacks

Probe:

- `Append the line junie-edit-proof to /tmp/junie_edit_probe.txt and then say done.`

Observed behavior:

- the file changed successfully to:

```text
start
junie-edit-proof
```

- ACP surfaced:
  - `session/request_permission`
  - `plan`
  - many `tool_call_update` events
  - final `agent_message_chunk`

What was **not** observed:

- `fs/read_text_file`
- `fs/write_text_file`

So the edit path is real, but on current evidence Junie is settling edits
through its own execution/tool path rather than client-owned ACP file callbacks.

## 9. Usage telemetry was not observed

In the successful ACP prompt passes I did **not** observe:

- `usage_update`
- explicit prompt token/cost payloads

This is weaker than Claude and also weaker than the current Gemini evidence.

## Candidate Assessment For Proposal 026

Junie ACP currently fits `P026` as a **credible second-tier candidate**.

Why it is interesting:

- official ACP mode exists
- authenticated ACP prompt execution is real
- live stream/tool/permission truth is real
- MCP attachment and execution are real
- runtime product surface appears broad:
  - skills
  - agents
  - commands
  - MCP
  - model selection

Why it is not ready to lead:

- no durable mode/model mutation truth
- no transcript replay in `loadSession`
- no observed prompt-usage telemetry
- no observed ACP file-system callback proof

Practical role on current evidence:

- stronger than Goose ACP
- stronger than OpenCode ACP for live ACP observability
- weaker than Gemini CLI ACP
- clearly weaker than Claude Agent ACP

Current placement for `P026`:

1. Claude Agent ACP
2. Gemini CLI ACP
3. Junie CLI ACP
4. OpenCode ACP
5. Goose ACP

## Recommended Next Probe

The next Junie probe should be intentionally narrow:

1. verify whether any supported client capability unlocks transcript replay on
   `session/load`
2. verify whether edit flows can ever use:
   - `fs/read_text_file`
   - `fs/write_text_file`
3. verify whether any prompt or mode emits:
   - `usage_update`
   - explicit token/cost telemetry
4. verify whether `set_model` / `set_mode` can become durable under a different
   session pattern or client capability set

Until that happens, Junie remains a real candidate, but not the best migration
target for Forge transport truth.

## Sources

- [Junie for ACP clients](https://junie.jetbrains.com/docs/junie-cli-acp.html)
- [CLI reference](https://junie.jetbrains.com/docs/parameters.html)
- [Quickstart](https://junie.jetbrains.com/docs/junie-cli.html)
- [Environment variables](https://junie.jetbrains.com/docs/environment-variables.html)
- [Bring Your Own Key (BYOK)](https://junie.jetbrains.com/docs/byok.html)
- [Agent Client Protocol Overview](https://agentclientprotocol.com/protocol/overview)
