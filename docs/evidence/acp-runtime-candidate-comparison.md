# ACP Runtime Candidate Comparison

Status: **Comparative Evidence Snapshot** (2026-04-04)

## Purpose

This note compares the seven ACP runtime candidates currently researched for
[`Proposal 026`](../proposals/026-acp-first-runtime-transport-and-goose-decoupling.md):

- Claude Agent ACP
- Gemini CLI ACP
- OpenCode ACP
- Junie CLI ACP
- Cline CLI ACP
- Auggie CLI ACP
- Goose ACP

The comparison is intentionally narrow. It is about suitability for Forge’s
transport, observability, recovery, and report-truth needs. It is not a
general-purpose product comparison.

## Executive Summary

Current ranking for `P026`:

1. **Claude Agent ACP**: strongest current transport and observability
   candidate
2. **Gemini CLI ACP**: strongest current native-style ACP runtime candidate
3. **Auggie CLI ACP**: execution-proven with replay, permission callbacks,
   edit settlement, and real MCP execution, but durable mutation truth remains
   weak
4. **Junie CLI ACP**: execution-proven and observability-capable, but weak on
   replay and persisted mutation truth
5. **Cline CLI ACP**: execution-proven with live tool events, but replay truth
   and edit settlement are weaker
6. **OpenCode ACP**: strongest broader runtime product surface, but ACP
   observability is still too opaque
7. **Goose ACP**: useful bridge reference only, not a strong direct cutover
   target

Why Claude currently leads:

- real `session/update` streaming
- persisted `set_mode` and `set_model` truth
- real `usage_update`
- real permission callback proof
- rich edit diff lifecycle
- real MCP attach and tool execution proof

Why Gemini still matters:

- real `session/update` streaming
- real `loadSession` replay
- real tool-call visibility
- real edit permission callback proof
- real ACP `fs/read_text_file` callback proof
- real MCP attach and tool execution proof
- but weaker persisted session-config truth than Claude

Why Auggie now ranks above Junie, Cline, and OpenCode:

- authenticated `session/new` and `session/prompt` are now proven
- `session/request_permission` is real
- `session/load` replay is real when called with its stricter parameter shape
- edit settlement is proven with real file mutation
- client-supplied MCP tool execution is proven
- but durable `set_mode` / `set_model` truth is still weak and no
  `agent_thought_chunk` or usage telemetry has been observed

Why Junie remains strong but now ranks below Auggie:

- real authenticated ACP prompt execution
- real `agent_thought_chunk` and `agent_message_chunk`
- real permission callbacks
- real `tool_call` / `tool_call_update`
- real client-supplied MCP tool execution
- but still weak on persisted mode/model truth and `loadSession` replay

Why Cline remains below Auggie and Junie:

- authenticated `session/new` and `session/prompt` are now proven
- live `tool_call` / `tool_call_update` events are real
- but `loadSession` capability is advertised while `session/load` is missing
- edit settlement is weaker than Junie
- MCP is still doc-conflicted and unproven live

Why OpenCode still matters:

- stronger runtime-native story for skills, agents, and permissions
- better persisted mode/model truth than Gemini
- but ACP still does not expose enough execution truth for Forge live surfaces

Why Goose is no longer the lead candidate:

- no usable `session/update` prompt stream in the probe
- no replay-rich `loadSession`
- untrustworthy persisted model truth
- unhealthy MCP attach behavior

## Comparative Matrix

| Capability | Claude Agent ACP | Gemini CLI ACP | OpenCode ACP | Junie CLI ACP | Cline CLI ACP | Auggie CLI ACP | Goose ACP |
|---|---|---|---|---|---|---|---|
| ACP server startup | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Stdio transport usability | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| Stdio framing detail | NDJSON over stdio | NDJSON over stdio | Not material in probe | NDJSON over stdio | NDJSON over stdio | NDJSON over stdio | Not material in probe |
| `session/new` | Yes | Yes | Yes | Yes | Yes | Yes | Yes |
| `session/load` | Yes | Yes | Yes | Yes | Advertised, but missing at runtime | Yes, with strict params | Yes |
| `session/load` transcript replay | Partial | Yes | No | No | No | Yes | No |
| Prompt `session/update` streaming | Yes | Yes | No | Yes | Partial | Yes | No |
| Thought streaming | Yes | Yes | No | Yes | Not observed | Not observed | No |
| Tool-call event visibility | Yes | Yes | No | Yes | Yes | Yes | No |
| Permission callback proof | Yes | Yes | No | Yes | Not observed | Yes | No |
| ACP file read callback proof | Not yet observed | Yes | Unknown | Not observed | Not proven | Not observed | Unknown |
| ACP file write callback proof | Not yet observed | Not yet explicitly observed | Unknown | Not observed | Not proven | Not observed | Unknown |
| MCP attach via `mcpServers` | Yes | Yes | Partial, healthier than Goose | Yes | Docs conflicted / not proven | Yes | Degraded/hangs on bad inputs |
| Real MCP tool execution proof | Yes | Yes | No | Yes | No | Yes | No |
| Prompt usage telemetry | Yes | Yes | Yes | Not observed | Not observed | Not observed | Weak / absent |
| Usage update events | Yes | Not observed | No | No | Not observed | No | No |
| Persisted model mutation truth | Yes | No | Yes | No | Not provable with current replay gap | No | No |
| Persisted mode mutation truth | Yes | No | Yes | No | Not provable with current replay gap | No | Yes |
| Fresh-session reloadability before prompt | Not proven | No | Not proven | Yes | Not proven | Not proven | Unknown |
| Skills as runtime feature | Strong slash-command/runtime heritage | Not central in current evidence | Yes in runtime, ACP opaque | Yes in runtime/docs | Yes in runtime/docs | Yes in runtime/docs | Not a current strength |
| Custom agents/subagents | Not central in current evidence | Not central in current evidence | Yes in runtime, ACP opaque | Yes in runtime/docs | Hooks/skills emphasized; subagents not proven | Personas/plugins strong; subagents not proven | Not a current strength |
| Fit for Forge live timeline | Strongest current fit | Good | Poor-to-partial | Good-to-partial | Partial | Good-to-partial | Poor |
| Fit for Forge report/recovery truth | Strongest current fit, with replay caveat | Strong, but persisted truth is weaker | Partial | Partial, but replay is weak | Weak-to-partial due to replay gap | Partial-to-good, but persisted truth is weak | Poor |

## Candidate Notes

## 0. Claude Agent ACP

Strengths:

- strongest current ACP observability story overall
- persisted mode/model truth survives `loadSession`
- rich edit lifecycle with real permission callback
- `usage_update` carries context usage and cost
- MCP attach and tool execution are real

Weaknesses:

- adapter-based, not a native Anthropic ACP runtime
- `loadSession` replay is only partially proven
- ACP file-system client callbacks are implemented but not yet live-proven

Practical role:

- current leading candidate for `P026`

Primary evidence:

- [claude-agent-acp-research.md](./claude-agent-acp-research.md)

## 1. Gemini CLI ACP

Strengths:

- strongest current non-Claude ACP observability story
- real transcript replay on `loadSession`
- real tool-call visibility
- real permission callback proof
- real `fs/read_text_file` callback proof
- real MCP tool execution proof

Weaknesses:

- persisted `currentModeId` after mutation is not trustworthy
- persisted `currentModelId` after mutation is not trustworthy
- fresh sessions are not durably reloadable before first prompt write
- `fs/write_text_file` has still not been explicitly observed even when edits
  succeed

Practical role:

- strongest current non-Claude candidate for `P026`

Primary evidence:

- [gemini-cli-acp-research.md](./gemini-cli-acp-research.md)

## 2. Junie CLI ACP

Strengths:

- official ACP mode is documented and real
- authenticated ACP prompt execution is proven
- real `agent_thought_chunk` and `agent_message_chunk`
- real permission callback proof
- real `tool_call` / `tool_call_update`
- real client-provided MCP tool execution
- broad runtime surface for skills, agents, commands, and MCP

Weaknesses:

- `set_model` mutation did not durably settle into `loadSession`
- `set_mode` mutation did not durably settle into `loadSession`
- `loadSession` replay still exposes no transcript/history
- ACP file callbacks were not observed in successful edit flows
- prompt usage telemetry was not observed

Practical role:

- credible second-tier candidate for `P026`
- stronger than OpenCode for live ACP observability
- weaker than Gemini because replay and settled session truth are worse

Primary evidence:

- [junie-cli-acp-research.md](./junie-cli-acp-research.md)

## 3. Auggie CLI ACP

Strengths:

- official ACP mode is real and documented
- authenticated `session/new` and `session/prompt` are proven
- `session/request_permission` is real
- `session/list` and `session/load` are real
- `session/load` replays transcript chunks
- edit settlement is proven with real file mutation
- MCP attach and real MCP tool execution are proven
- runtime/docs strongly emphasize:
  - skills
  - rules
  - permissions
  - MCP
  - plugins
  - session management

Weaknesses:

- `session/load` has a stricter parameter contract than many peers
- durable `set_mode` truth is weak
- durable `set_model` truth is weak
- `agent_thought_chunk` was not observed
- usage telemetry was not observed
- ACP fs callbacks were not observed

Practical role:

- strong second-tier candidate
- now stronger than Cline, OpenCode, and Goose on present ACP evidence
- plausible competitor for Junie, with better replay and edit settlement but
  weaker thought/usage signals

Primary evidence:

- [auggie-cli-acp-research.md](./auggie-cli-acp-research.md)

## 4. Cline CLI ACP

Strengths:

- one of the strongest official ACP narratives
- official ACP docs promise the full Cline agent with Skills, Hooks, and MCP
- SDK docs explicitly model ACP lifecycle, events, and permission handling
- live unauthenticated handshake is real and exposes named auth methods

Weaknesses:

- `loadSession` capability is advertised, but `session/load` was missing at
  runtime
- edit settlement proof is weak
- permission callbacks were not observed in the successful probes
- `mcpServers` support is doc-conflicted:
  - ACP docs imply full MCP integration
  - SDK docs say `mcpServers` not supported yet

Practical role:

- real second-tier candidate
- not yet strong enough to outrank Junie because replay and settlement truth
  are weaker

Primary evidence:

- [cline-cli-acp-research.md](./cline-cli-acp-research.md)

## 5. OpenCode ACP

Strengths:

- strongest current runtime feature surface:
  - skills
  - agents/subagents
  - permissions
  - MCP product maturity
- persisted mode/model mutation truth looked good
- invalid MCP attach did not hang

Weaknesses:

- ACP probes still showed zero streamed `session/update` events
- permission callbacks were not surfaced through ACP
- transcript/history replay was not surfaced through ACP
- runtime richness exists, but ACP truth is still too opaque

Practical role:

- strong strategic candidate if ACP observability improves
- not yet the best immediate fit for Forge live runtime surfaces

Primary evidence:

- [opencode-acp-research.md](./opencode-acp-research.md)

## 6. Goose ACP

Strengths:

- real ACP surface exists
- `session/new`, `session/load`, and `session/list` work
- `set_session_mode` looked persistent in the probe

Weaknesses:

- prompt turns emitted zero `session/update` events
- `loadSession` did not replay transcript
- model mutation truth was not trustworthy
- invalid MCP attachment attempts hung instead of failing cleanly

Practical role:

- adapter/bridge reference
- not the best direct transport target for `P026`

Primary evidence:

- [goose-acp-compatibility-probe.md](./goose-acp-compatibility-probe.md)

## Decision Signal For Proposal 026

Current decision signal:

- `P026` should stay ACP-first
- Goose should remain a temporary adapter, not the canonical runtime shape
- Claude Agent ACP is currently the best direct migration target for the live
  execution slice
- Gemini CLI ACP remains the strongest fallback/alternative candidate
- Auggie is now a real fallback-tier candidate with better replay and
  settlement truth than several peers, but durable mutation truth still lags
- Junie remains strong if Forge values thought-stream richness and already-proven
  MCP/permission flows over replay fidelity
- Cline is strategically interesting because the official ACP contract is
  unusually explicit, but replay truth still lags behind its advertised
  capability surface
- OpenCode remains strategically interesting, especially if ACP observability
  catches up to its runtime capabilities

## Remaining Cross-Candidate Questions

The main unresolved questions are now narrower and more concrete:

1. Can Claude prove transcript-complete `loadSession` replay across longer
   multi-turn sessions?
2. Can Gemini prove `fs/write_text_file` as part of a stable client-owned write
   path?
3. Can Claude, Gemini, and OpenCode all provide stable preflight/error
   classification for bad MCP auth/transport scenarios?
4. Can Auggie surface `agent_thought_chunk`, usage telemetry, and stable durable
   mutation truth strongly enough to challenge Gemini for the second slot?
5. Can Junie improve `loadSession` replay and durable `set_model` / `set_mode`
   truth enough to compete with Auggie and Gemini?
6. Can Cline reconcile its advertised `loadSession` capability with a working
   replay method and prove a trustworthy edit/MCP settlement path?
7. Can OpenCode surface enough streamed ACP truth to become competitive for
   Forge Live Timeline and report fidelity?
8. Can Goose ACP improve enough to justify anything beyond a short bridge?

## Sources

- [claude-agent-acp-research.md](./claude-agent-acp-research.md)
- [gemini-cli-acp-research.md](./gemini-cli-acp-research.md)
- [opencode-acp-research.md](./opencode-acp-research.md)
- [junie-cli-acp-research.md](./junie-cli-acp-research.md)
- [cline-cli-acp-research.md](./cline-cli-acp-research.md)
- [auggie-cli-acp-research.md](./auggie-cli-acp-research.md)
- [goose-acp-compatibility-probe.md](./goose-acp-compatibility-probe.md)
- [Claude Agent SDK overview](https://platform.claude.com/docs/en/agent-sdk/overview)
- [@agentclientprotocol/claude-agent-acp](https://www.npmjs.com/package/@agentclientprotocol/claude-agent-acp)
- [Gemini CLI ACP Mode](https://geminicli.com/docs/cli/acp-mode/)
- [Junie for ACP clients](https://junie.jetbrains.com/docs/junie-cli-acp.html)
- [ACP: Editor Integrations - Cline](https://docs.cline.bot/cline-cli/acp-editor-integrations)
- [OpenCode ACP Support](https://opencode.ai/docs/acp/)
- [ACP Mode - Augment](https://docs.augmentcode.com/cli/acp/agent)
- [ACP Clients - Augment](https://docs.augmentcode.com/cli/acp/clients)
- [CLI Flags and Options - Augment](https://docs.augmentcode.com/cli/reference)
- [Agent Client Protocol Overview](https://agentclientprotocol.com/protocol/overview)
