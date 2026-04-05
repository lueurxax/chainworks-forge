# Goose ACP Compatibility Probe

Status: **Probed with Critical Gaps** (2026-04-04)

## Summary

A live probe was executed against the locally installed `goose acp` server from
Goose `1.28.0` using the Python ACP SDK over stdio.

The probe confirms that Goose ACP is real and partially functional, but it does
**not** yet match the current `Chainworks Forge` live execution slice closely
enough to replace the existing Goose REST/SSE transport.

The main blocker is not session creation. The main blocker is execution
observability and runtime fidelity:

- `session/new` works
- `session/list` works
- `session/load` works
- `set_session_mode` persists
- but `session/prompt` returns only `stop_reason = end_turn` with **zero**
  `session/update` events
- `set_session_model` appears to be a no-op for persisted session model truth
- `mcpServers` attachment hangs on invalid probe inputs rather than failing
  quickly with a structured error

This means the current Goose ACP surface is useful as a research and adapter
target, but not yet sufficient as a drop-in replacement for
`GooseServerTransport`.

## Environment

- macOS desktop environment
- Goose CLI `1.28.0`
- Command: `goose acp`
- Probe workspace: `/Users/user/Documents/CryptoSavingsTracker`
- Client: Python ACP SDK in a temporary virtualenv

Goose installation info at probe time:

```text
goose Version:
  Version:                  1.28.0

Paths:
Config dir:              /Users/user/.config/goose
Config yaml:             /Users/user/.config/goose/config.yaml
Sessions DB (sqlite):    /Users/user/.local/share/goose/sessions/sessions.db
Logs dir:                /Users/user/.local/state/goose/logs
```

## What Was Tested

### 1. ACP handshake and capability negotiation

`initialize` succeeded.

Observed capabilities:

- `protocol_version = 1`
- `load_session = true`
- `mcp_capabilities.http = true`
- `mcp_capabilities.sse = false`
- `prompt_capabilities.embedded_context = true`
- `prompt_capabilities.image = true`
- `session_capabilities.list = true`

Observed auth methods:

- `goose-provider`
  - description: `Run goose configure to set up your AI provider and API key`

### 2. Session creation and session metadata

`session/new` succeeded.

Observed response properties:

- real `session_id` values were returned
- available models were exposed
- available modes were exposed

Example model metadata returned by Goose ACP:

- `available_models`
  - `gemini-2.5-flash-lite`
  - `gemini-2.5-flash`
  - `gemini-2.5-pro`
- `current_model_id = opus`

This mixed result is already suspicious because the current model truth does not
match the advertised available-model list.

### 3. Prompt execution

`session/prompt` succeeded in the transport sense, but did not expose usable
live execution detail.

Observed response:

- `stop_reason = end_turn`
- `usage = null`
- `user_message_id = null`

Observed live updates:

- `0` `session/update` notifications

This was reproduced for:

- plain text prompt
- prompt after `set_session_model`
- prompt with `--with-builtin developer`
- prompt with linked resource context

### 4. Session listing and loading

`session/list` succeeded and returned real prior session rows, including:

- `cwd`
- `session_id`
- `title`
- `updated_at`

`session/load` succeeded and returned model/mode metadata for an existing
session.

However:

- it did not stream conversation history back through `session/update`
- it returned no richer replay surface needed for the current Forge live views

### 5. Mode mutation

`set_session_mode('chat')` appears to work.

Evidence:

- the initial `session/new` response reported `current_mode_id = auto`
- after `set_session_mode('chat')`
- `load_session` reported `current_mode_id = chat`

### 6. Model mutation

`set_session_model('gemini-2.5-pro')` did **not** produce trustworthy persisted
model truth.

Observed behavior:

- `set_session_model(...)` returned success with an empty body
- subsequent `load_session(...)` still reported `current_model_id = opus`

That is not sufficient for Forge, which needs stable persisted runtime truth.

### 7. MCP attachment through `mcpServers`

Goose ACP advertises `mcp_capabilities.http = true`, but practical attach
behavior looked unsafe.

Two probe cases were attempted during `session/new`:

- invalid HTTP MCP server:
  - `http://127.0.0.1:9/mcp`
- simple stdio MCP server:
  - command `/bin/echo hi`

Observed result:

- both calls hung until client timeout
- neither returned a structured ACP error promptly

This means the current implementation is not yet good enough for Forge
preflight/runtime expectations around MCP validation and failure classification.

## Compatibility Assessment

### Works

- ACP server startup via `goose acp`
- capability negotiation
- `session/new`
- `session/list`
- `session/load`
- `set_session_mode`

### Works but degraded

- `session/prompt`
  - transport-level success only
  - no visible assistant streaming
  - no incremental updates
  - no usage details
- linked resource prompt input
  - accepted without error
  - no evidence that resource context is surfaced back observably

### Broken or insufficient for current Forge needs

- live response streaming for run/timeline UI
- assistant message visibility through ACP events
- trustworthy persisted model mutation
- robust MCP attach / validation behavior
- MCP error classification
- tool-call visibility and permission flow evidence

### Unknown

- whether valid HTTP MCP servers work correctly in long-lived sessions
- whether image and embedded-context inputs are semantically honored beyond
  transport acceptance
- whether tool execution emits ACP notifications in other model/runtime
  configurations

## Impact on Proposal 026

This probe changes Proposal 026 from a purely architectural hypothesis into a
concrete migration constraint.

Proposal 026 may still be correct directionally:

- ACP-first transport
- Goose as temporary adapter
- Forge keeps ownership of run truth and orchestration semantics

But the current Goose ACP implementation is **not** enough to replace the
existing Goose REST/SSE transport without significant product degradation.

The specific gaps that matter to Forge are:

1. no `session/update` stream for prompt execution
2. no trustworthy post-mutation model truth
3. hanging MCP attachment behavior
4. insufficient runtime observability for Live Timeline, recovery, and report
   truth

## Practical Conclusion

The ACP migration should proceed with this explicit assumption:

- `Goose ACP 1.28.0` is a useful research target and adapter reference
- but it is not yet a sufficient execution substrate for the current Forge live
  transport slice

Any ACP-first migration plan must therefore either:

- target another ACP runtime with stronger fidelity, or
- accept a temporary observability regression while building a Forge ACP adapter
  layer that can tolerate the current Goose ACP limitations

## Reproduction Notes

The probe used:

1. `goose acp`
2. Python ACP SDK client over stdio
3. real `initialize`
4. real `session/new`
5. real `session/prompt`
6. real `session/list`
7. real `session/load`
8. real `set_session_mode`
9. real `set_session_model`
10. real `mcpServers` attachment attempts

The Python SDK was installed into a temporary virtualenv created under `/tmp`
only for the probe and is not part of repo state.
