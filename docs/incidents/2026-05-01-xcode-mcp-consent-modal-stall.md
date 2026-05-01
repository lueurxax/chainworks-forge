# 2026-05-01 Xcode MCP Consent Modal Stall

Status: Open

## Summary

Xcode MCP startup and `tools/list` can block behind the Xcode consent modal. If the operator does not click OK, the MCP helper can appear hung indefinitely. Retrying the same Xcode-backed agent invocation while that condition is unresolved can spawn additional helper attempts and additional modals, making the run slower and harder to recover.

This incident should not be collapsed into the generic message "Claude ACP silent" or "provider did not respond". The root blocking condition for modal-backed stalls is the Xcode MCP consent modal / helper lifecycle, with orchestration misclassifying the wait as an ACP idle timeout.

Important correction from the follow-up P066 investigation: not every `ACP session idle timeout - no message received` is an Xcode consent modal stall. The daemon can distinguish these cases only by correlating the ACP stream timeout with the persisted Xcode runtime observations:

- If the last Xcode broker observation is `backend_request_action_required`, an incomplete `tools/list`, or an Xcode MCP timeout, the run is waiting on Xcode/operator consent or helper recovery.
- If Xcode broker `initialize`, `notifications/initialized`, and `tools/list` all completed, and the lease was activated, then the Xcode MCP path was not the active blocker. The failure is provider/ACP stream silence after `session/prompt`.

## Evidence

- Direct `claude-agent-acp` without Xcode MCP initialized, created a session, and streamed a response normally.
- Claude ACP with a dummy HTTP MCP server initialized, listed tools, and streamed responses normally.
- A P066-sized prompt with a dummy HTTP MCP server streamed normally, including tool/thought updates.
- Direct `mcpbridge tools/list` was inconsistent: one run completed quickly, one later timed out after 60 seconds with no useful stderr, and another completed after a longer delay.
- The operator confirmed the missing variable: each real Xcode MCP `tools/list` may show an Xcode consent modal, and if OK is not clicked, the call can wait indefinitely.
- Multiple stale `mcpbridge` / `xcodebuildmcp` helper processes were observed during the same diagnostic window.
- The latest inspected P066 attempt (`24d79782-a3f6-4856-91c1-a9b7a90dfce4`) was not blocked on Xcode MCP: broker warm-up and provider first-connect `tools/list` completed in 94 ms and 56 ms through backend pid 65942, then ACP produced no prompt-stream messages until the 300 second idle timeout.
- An earlier P066 attempt (`a02d6edc-0ff4-4e22-86a4-a6f00eecdc74`) did show Xcode broker `backend_request_action_required` after 5002 ms and then completed `tools/list` in 6620 ms. That is the modal/action-required class, but it had settled before the later ACP idle failure.

## Operational Rule

When an Xcode-backed invocation has no ACP stream progress while MCP startup or `tools/list` is still in progress, treat it as a potential `waiting_for_xcode_consent_modal` condition, not as a generic provider timeout.

Do not blindly auto-retry Xcode-backed invocations while an unresolved consent-modal condition may exist. At most one consent-producing Xcode MCP attempt should be active for a run unless the previous lease/session has been explicitly settled or reaped.

The operator-facing readback should say that Xcode consent may be required. It should not present the failure as "ACP no message received" without the Xcode context.

If all Xcode broker operations completed before `session/prompt`, classify the failure as `provider_stream_silent_after_prompt` or equivalent. In that case, the system is waiting on the provider ACP process to emit stdout/ACP events, not on the daemon's internal Xcode broker.

## Required System Fixes

1. Detect Xcode MCP startup / `tools/list` waits and classify them separately from provider idle timeouts.
2. Make the Xcode consent wait operator-visible and long enough for human response, with a minimum practical wait of 10 minutes if the system cannot detect the modal directly.
3. Prevent auto-retry storms from spawning fresh Xcode MCP helpers while the previous Xcode lease/session is unresolved.
4. Add owned helper cleanup for stale `mcpbridge` / `xcodebuildmcp` processes by lease/session owner.
5. Surface `waiting_for_xcode_consent_modal` or equivalent in MCP readback, run diagnostics, and UI status.
6. Add boundary-aware failure classification for `ACP session idle timeout`: include the last ACP event, last Xcode broker observation, last MCP method, provider pid, and whether `session/prompt` had already been sent.
7. Avoid giving Xcode MCP to code-writer stages that do not actually need Xcode host execution; Xcode-capable profiles should not automatically make every implementation prompt depend on Xcode MCP.

## Follow-Up Scope

This is a system reliability issue in the Xcode MCP bridge / ACP supervision boundary. A narrow local fix can improve classification and retry suppression, but a durable solution should include helper ownership, consent-aware lease state, startup reconciliation, and operator-visible readback.

## P066 ACP Stream Follow-Up

The P066 attempt `24d79782-a3f6-4856-91c1-a9b7a90dfce4` exposed a second, separate failure mode:

- Claude's local session log `~/.claude/projects/.../29060931-513e-4193-abb1-e71602597bc1.jsonl` shows real agent activity after `session/prompt`.
- The agent read files, reasoned, and attempted to run `./scripts/test-gate.sh proposal-066`.
- While that internal Claude `Bash` tool was running, Chainworks received no ACP stdout messages.
- After the Chainworks 300 second ACP idle watchdog fired, Claude recorded the tool use as rejected/interrupted.

So the P066 timeout was not "daemon waiting on its own MCP" and not "Xcode MCP still blocked". It was a supervision blind spot: Claude Code can make progress inside its own transcript/tool runner while `claude-agent-acp` emits no ACP stream updates to Chainworks.

Required fix for this second class:

1. Classify `ACP session idle timeout` by correlating ACP stream state, Xcode runtime observation, and provider-local activity where available.
2. For Claude, monitor the provider session JSONL file or another supported activity source and treat new assistant/tool/user entries as invocation progress.
3. Do not kill a prompt at 300 seconds when provider-local activity is advancing, especially during long tool execution.
4. If no provider-local activity source is available, surface the uncertainty explicitly as `provider_stream_silent_no_sidecar_activity`, not as a generic retryable provider timeout.
