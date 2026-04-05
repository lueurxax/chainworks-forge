# OpenCode ACP Research

Status: **Live-Probed** (2026-04-04)

## Purpose

This note evaluates whether OpenCode ACP is a realistic execution substrate for
`Chainworks Forge`, with specific focus on:

- model selection and mutation
- MCP behavior
- skills
- agents/subagents
- tools and permission gates
- session/update observability for Live Timeline, recovery, and reports

The goal is not to prove that OpenCode is “good in general”. The goal is to
decide whether it is a credible target for Proposal 026 and where the transport
seams still do not match Forge needs.

## Executive Summary

OpenCode ACP is materially more promising than Goose ACP on current evidence.

What is already solid:

- ACP server is real and locally probeable
- model mutation persists correctly
- mode mutation persists correctly
- embedded context is accepted
- custom agents are real in the runtime
- local skills are real in the runtime
- permission gating is real in the runtime
- MCP is a first-class product surface in both docs and CLI

What is still not good enough for a clean Forge cutover:

- ACP prompt flow still yielded **zero** `session/update` notifications in all
  probes
- ACP permission callback flow did **not** surface permission requests, even
  where the CLI definitely did
- ACP session envelopes expose models and modes, but do not expose enough
  runtime truth for skill/agent/tool activity
- MCP transport behavior is promising, but real tool-use visibility through ACP
  is still unproven

Bottom line:

- OpenCode is a stronger ACP-first candidate than Goose
- but ACP-level observability is still only **partially proven** for the
  current Forge live execution slice

## Method

The evaluation used two evidence sources:

1. Official OpenCode documentation
2. Live local probes against:

```text
/Users/user/.opencode/bin/opencode acp --pure
```

Additional runtime probes used:

- `/Users/user/.opencode/bin/opencode run`
- `/Users/user/.opencode/bin/opencode agent list`
- `/Users/user/.opencode/bin/opencode mcp --help`
- `/Users/user/.opencode/bin/opencode models`

Temporary project-local fixtures were created to test:

- per-project `opencode.json`
- `.opencode/skills/test-skill/SKILL.md`
- `.opencode/agents/reviewer.md`

## Official Documentation Findings

### ACP support is presented as first-class

Official docs:

- [ACP Support](https://opencode.ai/docs/acp/)

The docs explicitly claim that OpenCode via ACP supports the same features as
the terminal experience, including:

- built-in tools
- custom tools and slash commands
- MCP servers from config
- project `AGENTS.md`
- custom formatters and linters
- agents and permissions system

Relevant lines:

- [ACP docs, support section](https://opencode.ai/docs/acp/)

This is much stronger positioning than Goose ACP.

### MCP is a first-class product system

Official docs:

- [MCP servers](https://opencode.ai/docs/mcp-servers/)

Documented capabilities include:

- local MCP servers by command array
- remote MCP servers by URL and headers
- automatic OAuth detection
- pre-registered OAuth credentials
- manual auth, logout, and debug commands
- global tool-level MCP enable/disable
- per-agent MCP enablement using tool globs

Specific documentation points:

- project config supports `mcp` definitions and `enabled`
- remote servers support `headers`, `oauth`, and `timeout`
- OAuth can be automatic or explicitly configured
- MCP servers become tools and can be disabled globally or re-enabled per agent

Relevant lines:

- [MCP enable/config](https://opencode.ai/docs/mcp-servers/)
- [MCP OAuth and auth flow](https://opencode.ai/docs/mcp-servers/)
- [Per-agent MCP enablement](https://opencode.ai/docs/mcp-servers/)

### Skills are first-class in the runtime

Official docs:

- [Agent Skills](https://opencode.ai/docs/skills/)

The docs position skills as a native runtime feature, not a side convention.
They are discoverable from locations including:

- `.opencode/skills/`
- `~/.config/opencode/skills/`

This aligns much better with Forge Proposal 015 than Goose does.

### Agents and subagents are first-class

Official docs:

- [Agents](https://opencode.ai/docs/agents/)
- [Config](https://opencode.ai/docs/config/)

The docs explicitly support:

- primary agents
- subagents
- per-agent tools/permissions
- markdown agent definitions in `.opencode/agents/`
- JSON config agent definitions in `opencode.json`

Relevant lines:

- [Config: `agent` option](https://opencode.ai/docs/config/)
- [Agents: markdown definitions in `.opencode/agents/`](https://opencode.ai/docs/agents/)

### Tools and permissions are explicit and composable

Official docs:

- [Tools](https://opencode.ai/docs/tools/)

The docs show permissions for:

- `read`
- `grep`
- `glob`
- `list`
- `edit` / `write`
- `bash`
- experimental `lsp`

This maps well to Forge’s need for structured tool/permission policy.

## Live ACP Probe Findings

## 1. ACP server availability and capabilities

The ACP server starts successfully with:

```text
/Users/user/.opencode/bin/opencode acp --pure
```

Observed on `initialize`:

- `agent_info.version = 1.3.13`
- `protocol_version = 1`
- `load_session = true`
- `mcp_capabilities.http = true`
- `mcp_capabilities.sse = true`
- `prompt_capabilities.embedded_context = true`
- `prompt_capabilities.image = true`
- `session_capabilities.list = true`
- `session_capabilities.resume = true`
- `session_capabilities.fork = true`

This is already stronger than the previously probed Goose ACP surface.

## 2. Models

### What works

`session/new` returns a rich model catalog, including current OpenAI models such
as:

- `openai/gpt-5.4-mini`
- `openai/gpt-5.4`
- `openai/gpt-5.3-codex`
- `openai/gpt-5.3-codex-spark`
- `openai/gpt-5.2`
- `openai/gpt-5-codex`

Initial session state reported:

- `current_model_id = "opencode/big-pickle"`

`set_session_model("openai/gpt-5.4-mini")` succeeded, and after
`load_session` the session reported:

- `current_model_id = "openai/gpt-5.4-mini"`

This matters because Goose ACP previously failed this exact trust test.

### What remains unclear

Even though model mutation persists, ACP still does not expose rich streamed
assistant/tool state tied to the selected model. So model routing itself looks
usable, but model-specific runtime observability is not yet proven.

## 3. Modes and agents

### ACP session envelope

Observed via ACP `session/new`:

- `current_mode_id = "build"`
- `available_modes = ["build", "plan"]`
- `config_options = null`

This means ACP session setup exposes:

- modes
- models

But **not** a first-class list of:

- custom agents
- subagents
- skills
- active tool config
- permission policy

That is a transport-level visibility limitation for Forge.

### Runtime reality outside ACP

Using the direct runtime CLI in a temp project workspace:

- `opencode agent list` showed built-ins:
  - `build`
  - `compaction`
  - `explore`
  - `general`
  - `plan`
- and it also showed the project-local custom subagent:
  - `reviewer (subagent)`

So custom agents are real in the runtime even if ACP does not surface them in
the session envelope.

### Decision impact

For Forge this means:

- custom agents are a runtime capability we can likely use
- but ACP transport does not yet give us enough structured truth about agent
  availability or agent-level execution events

## 4. Skills

### Runtime proof

A temp project-local skill was created at:

- `.opencode/skills/test-skill/SKILL.md`

Using direct runtime CLI:

```text
/Users/user/.opencode/bin/opencode run "Use the local skill named test-skill..."
```

Observed output:

- `→ Skill "test-skill"`
- final response began with `TEST_SKILL_CONFIRMED`

So project-local skills are operationally real.

### ACP behavior

The same prompt through ACP completed successfully, but:

- `update_count = 0`
- `permission_count = 0`
- prompt returned only final envelope data:
  - `stop_reason = "end_turn"`
  - token usage

That means ACP does not currently provide proof of:

- skill invocation events
- skill identity in stream updates
- skill-level runtime truth

### Decision impact

For Forge this is important:

- OpenCode runtime skills are viable
- ACP transport does not yet expose enough evidence to build trustworthy skill
  telemetry or timeline surfaces

## 5. Tools and permissions

### Runtime CLI proof

In a temp project workspace with:

- `opencode.json` containing `bash: "ask"`

direct runtime CLI probe:

```text
/Users/user/.opencode/bin/opencode run "Run pwd in the shell..."
```

Observed:

- `permission requested: bash (pwd); auto-rejecting`
- then:
  - `bash failed`
  - `The user rejected permission to use this specific tool call.`

This is a strong proof that OpenCode runtime permission gates are real.

### ACP behavior

The same intent through ACP produced:

- `permission_count = 0`
- `update_count = 0`
- `stop_reason = "end_turn"`

So either:

- ACP is not surfacing permission requests through the expected callback path,
  or
- the runtime is handling that path differently under ACP,
  or
- the SDK/client integration does not yet expose those events

Whatever the cause, for Forge the practical conclusion is the same:

- ACP permission observability is currently not proven, even though runtime
  permission enforcement clearly exists

## 6. MCP

### Documentation and CLI maturity

MCP is one of the strongest parts of the OpenCode story.

Observed directly:

- `opencode mcp --help` exposes:
  - `add`
  - `list`
  - `auth`
  - `logout`
  - `debug`

That aligns with docs that describe:

- local and remote MCP servers
- OAuth auth flow
- auth state management
- debug flow
- per-agent enablement via tools

### ACP capabilities

Observed on `initialize`:

- `mcp_capabilities.http = true`
- `mcp_capabilities.sse = true`

This is already stronger than the Goose ACP probe, where SSE MCP capability was
not available.

### Live probe facts

ACP session creation with a deliberately invalid remote MCP input did **not**
hang. The prompt completed with:

- `stop_reason = "end_turn"`
- no transport failure

This is again better than Goose ACP, where invalid MCP attachment previously
caused hangs/timeouts.

### What is still missing

The probe still does **not** prove:

- successful MCP tool discovery through ACP callbacks
- MCP tool invocation events through ACP callbacks
- MCP auth prompts through ACP callbacks
- enough MCP runtime truth for Forge Live Timeline or reports

So MCP looks architecturally strong and operationally promising, but ACP-level
tool-use visibility is still incomplete.

## 7. Streaming and runtime truth

This is the main unresolved problem.

Across all ACP probes, including:

- baseline prompt
- embedded resource prompt
- model mutation prompt
- skill-oriented prompt
- permission-oriented prompt
- invalid MCP prompt

the observed ACP callback counts remained:

- `update_count = 0`
- `permission_count = 0`

while the prompt RPC returned only:

- `stop_reason`
- `usage`
- `user_message_id = null` in earlier baseline probes

And `load_session` returned only high-level session state:

- `models`
- `modes`
- `config_options`

not a transcript or rich execution event stream.

### Why this matters for Forge

Forge currently depends on runtime truth for:

- Live Timeline
- retry classification
- late-output reconciliation
- report fidelity
- execution receipts
- operator diagnostics

Without streamed ACP updates or another source of structured execution events,
we cannot assume parity with the current Forge live slice.

## Compatibility Matrix For Forge

| Area | Current status | Evidence | Decision signal for Forge |
|---|---|---|---|
| ACP server availability | Ready | `initialize`, `session/new`, `session/load`, `session/list` all worked | Good foundation |
| Model catalog and selection | Ready | rich model catalog surfaced; `set_session_model("openai/gpt-5.4-mini")` persisted after `load_session` | Stronger than Goose ACP |
| Mode mutation | Ready | `set_session_mode("plan")` persisted after `load_session` | Usable for transport cutover |
| Embedded context | Ready | `resource_link_block` prompt completed successfully | Compatible with Forge prompt packets |
| MCP declaration | Partial | docs and CLI are strong; ACP declares `http` and `sse`; invalid MCP attach did not hang | Promising, but not yet sufficient |
| MCP runtime visibility | Blocked | no MCP tool discovery, tool-use, or auth events observed via ACP callbacks | Not enough for operator surfaces |
| Skills in runtime | Ready | direct runtime invoked project-local `test-skill` successfully | Runtime capability is real |
| Skills via ACP truth | Partial | skill-oriented prompt succeeded, but ACP emitted no skill events | Works, but opaque |
| Custom agents in runtime | Ready | direct runtime listed `reviewer (subagent)` from `.opencode/agents/` | Runtime capability is real |
| Custom agents via ACP truth | Partial | ACP session envelopes do not expose custom agents/subagents | Works, but opaque |
| Tool permission enforcement | Ready | direct runtime raised `permission requested: bash (pwd)` and rejected on CLI | Runtime policy is real |
| Permission callbacks via ACP | Blocked | ACP probes observed `permission_count = 0` even for permission-worthy tool intent | Missing callback proof |
| Streaming assistant/tool events | Blocked | all probes had `update_count = 0` | Main cutover blocker |
| Transcript / history replay | Blocked | `load_session` returned only modes/models/config, no transcript replay | Insufficient for Live Timeline and reports |
| Live Timeline / report parity | Blocked | no streamed runtime truth for assistant/tool/permission events | Cannot approve parity yet |

## Decision

OpenCode is the strongest ACP-first runtime candidate seen so far.

Compared with Goose ACP:

- model mutation behaves better
- MCP capability signaling is stronger
- runtime support for skills, agents, and permissions is much richer
- invalid MCP attach behavior is healthier

However, a clean Forge cutover still should **not** be approved solely on this
evidence, because the transport-level observability story is still incomplete.

The practical conclusion is:

1. OpenCode should be treated as the leading ACP-first target for Proposal 026.
2. The next research step should focus narrowly on streamed runtime truth:
   - `session/update`
   - permission callbacks
   - tool-call visibility
   - transcript/history retrieval
3. If those seams can be solved, OpenCode becomes a far more credible cutover
   backend than Goose.
