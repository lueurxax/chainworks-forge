# P086 Evidence: Manual Provider-Session Resurrection Experiment

Date: 2026-05-08

## Context

This experiment was run while investigating repeated P075 code-writer retries that lost useful implementation context. A normal Chainworks retry created a fresh provider turn, and the agent repeatedly spent time rediscovering the proposal, review findings, repository shape, and current worktree state before doing new implementation work.

The operator then used a known provider `session_id` directly through ACP/provider tooling and sent a strict continuation instruction into that prior provider session. This was intentionally outside the Chainworks workflow engine and database path.

## Important distinction

This was not ordinary Chainworks live-handle continuation.

In ordinary live continuation, `AcpRuntimeManager` already owns a live `ActiveAcpSessionHandle` for a `session_generation_id` and can send another prompt into that handle.

In this experiment, Chainworks did not own or drive the continuation. The operator resurrected/continued a provider session by known provider `session_id` using provider-side session continuity. Chainworks had no first-class continuation record, no preflight, no server-owned prompt dispatch, and no durable continuation receipt for the direct prompt.

This means P086 must model two different capabilities:

- `live_handle_continuation`: send a continuation prompt into a live handle already owned by `AcpRuntimeManager`.
- `provider_session_resurrection`: attach/resume a provider session by known provider `session_id` when the adapter explicitly supports that operation.

The second capability must be explicit and fail closed when unsupported. It must not be silently treated as normal retry, checkpoint rehydration, or ordinary `SessionReuseDisposition::Reused`.

## Observed behavior

The manual provider-session resurrection changed agent behavior in the useful direction:

- the agent continued from prior work context instead of starting broad proposal/repository rediscovery;
- the operator instruction was treated as an implementation continuation request, not as a normal retry prompt;
- the agent focused on closing concrete P075 review findings and producing a durable worktree diff;
- later Chainworks-managed validation/retry could inspect the resulting work through normal artifacts and review paths.

The experiment also exposed the system gap: Chainworks could benefit from the provider's continued context, but the workflow truth was only restored later by separate normal orchestration. The direct continuation itself was invisible to scheduler truth, lineage truth, worktree readback, continuation metrics, and operator UI readback.

## Gaps found

- No `agent_work_continuation` record existed for the direct prompt.
- No continuation preflight checked run/stage/agent/worktree compatibility.
- No side-effect ledger check ran before the direct prompt.
- No continuation prompt receipt or ACP transcript was linked to the run.
- No worktree readback was captured immediately after the direct continuation.
- A later normal retry still had to re-enter Chainworks-managed execution truth.
- Current `AcpRuntimeManager::prompt_session` only targets an existing live session generation. It does not attach to a provider session by known provider `session_id`.

## Implementation implications

P086 should not only say "same live ACP session". It must distinguish:

1. live continuation through an existing `AcpRuntimeManager` handle;
2. provider-session resurrection by recorded provider `session_id`;
3. checkpoint rehydration, which creates a fresh ACP session with checkpoint context and is not provider-native session resurrection.

Initial implementation may support only live-handle continuation, but it must still expose provider-session resurrection as an explicit unsupported/fail-closed path when requested. A future adapter-specific implementation can then add provider-native attach/resume without changing the workflow model.

Required durable evidence for provider-session resurrection:

- requested continuation mode;
- source run, stage execution, agent execution, and session generation;
- recorded provider `session_id`;
- adapter/runtime capability proving provider-session resurrection is supported;
- attach/resume receipt;
- new managed continuation handle or generation created after attach;
- ACP transcript spool path;
- worktree readback after the continuation;
- final continuation report and follow-up validation result.

