# Proposal 045: Run Recovery and Granular Retry MCP Tools

| Field | Value |
|---|---|
| Date | 2026-04-17 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [041-server-parity-harness-golden-runs-and-behavioral-diff.md](041-server-parity-harness-golden-runs-and-behavioral-diff.md) |
| Scope | Add run resume, agent-level retry, approval gate re-arm, stage skip, evidence retrieval, and AI-ranked recovery suggestions to the MCP tool surface. |
| Goal | The operator can recover from any failure state through MCP tools with the same granularity the Swift app provides, plus a new `recovery.suggest` tool that recommends the best recovery path automatically. |

---

## 1. Context and Motivation

The control-plane has `stages.retry` (stage-level retry), `runs.cancel`, and `resetSession` (GraphQL only). Missing: resume interrupted runs, agent-level retry, approval gate re-arm after rejection, and failed stage evidence retrieval. The Swift app has all of these via `RecoveryCoordinator`.

The gap is most painful when a run is blocked:

1. A single agent failed in a multi-agent stage. The operator must retry the entire stage, re-running successful agents unnecessarily.
2. A run was interrupted mid-transition (process restart). There is no `runs.resume` tool — the operator must wait for the next automatic startup recovery or manually re-start.
3. An approval was rejected prematurely. The gate cannot be re-opened without restarting the stage.
4. A non-critical stage (e.g., docs generation) blocks the pipeline. The operator cannot skip it.
5. The operator must call multiple APIs to understand why a run is blocked. There is no single evidence endpoint.

---

## 2. Product Questions This Proposal Must Answer

1. Can the operator resume an interrupted run without waiting for startup recovery?
2. Can the operator retry a single failed agent without re-running successful siblings?
3. Can the operator re-arm a rejected approval gate?
4. Can the operator skip a non-critical failed stage?
5. Can the operator get a complete evidence packet for a failed stage in one call?
6. Can the system recommend the best recovery action automatically?

---

## 3. Scope

This proposal includes:

- 6 new MCP tools: `runs.resume`, `agents.retry`, `approvals.rearm`, `stages.skip`, `recovery.evidence`, `recovery.suggest`.
- 5 new command variants: `ResumeRunCmd`, `RetryAgentCmd`, `RearmApprovalCmd`, `SkipStageCmd`, `SuggestRecoveryCmd`.
- Guard-rail validations for each tool.
- Recovery suggestion engine with ranked recommendations.

This proposal does **not** include:

- GraphQL equivalents (MCP-first; GraphQL can follow).
- Changes to workflow execution logic or transition evaluation.
- Changes to the ACP runtime or provider adapters.
- Automatic recovery (all tools require explicit operator invocation).

---

## 4. Problem Statement

### 4.1 No on-demand resume

The control-plane has startup recovery (`engine/recovery.rs`) that runs once on daemon start. If a run is interrupted while the daemon is running (e.g., provider timeout), there is no way to trigger resume without restarting the daemon.

### 4.2 Stage retry is too coarse

`stages.retry` creates a new `StageExecution` and re-runs all agents in the stage. In a multi-agent stage (e.g., 4 proposal reviewers), if one fails, the other three are re-run unnecessarily. The Swift app has `retryAgent()` which retries only the failed agent.

### 4.3 Rejected approvals are terminal

When `approvals.resolve` is called with `decision: "rejected"`, the stage is marked as failed. To re-attempt, the operator must use `stages.retry`, which restarts the entire stage from scratch. The Swift app has `resumeFromApprovalGate()` which re-opens the gate.

### 4.4 No stage skip

Some stages are non-critical. If docs generation fails due to a transient issue, the operator may prefer to skip it and proceed to release. Currently, the only options are retry (which may fail again) or cancel (which abandons the entire run).

### 4.5 Evidence requires multiple API calls

To understand why a run is blocked, the operator must: `runs.get` → `stages` query → `agent_executions` query → `reports.get`. The Swift app's `BlockedRunRecoveryView` assembles this in one place.

---

## 5. Core Product Behavior

### 5.1 MCP Tool: `runs.resume`

```json
{
  "name": "runs.resume",
  "description": "Resume an interrupted run from its last known checkpoint",
  "input_schema": {
    "type": "object",
    "required": ["run_id"],
    "properties": {
      "run_id": { "type": "string" }
    }
  }
}
```

**Behavior:**

1. Load run. Guard: status must be `Blocked` or `Running` with no active work items.
2. Determine resume point:
   - If `transition_cursor` exists and `settlement_state = next_state_scheduled_not_started`: resume from the scheduled next state.
   - Else: find the latest non-terminal `StageExecution` and re-enqueue its work items.
3. Update run status to `Running`.
4. Return: `{ "resumed": true, "from_state": "state_5_implementation", "journal_id": "..." }`.

**Guard:** Reject if run has active work items in the queue (already being processed).

### 5.2 MCP Tool: `agents.retry`

```json
{
  "name": "agents.retry",
  "description": "Retry a single failed agent within a stage, preserving successful sibling results",
  "input_schema": {
    "type": "object",
    "required": ["run_id", "stage_id", "agent_id"],
    "properties": {
      "run_id": { "type": "string" },
      "stage_id": { "type": "string" },
      "agent_id": { "type": "string" }
    }
  }
}
```

**Behavior:**

1. Find the latest `StageExecution` for `(run_id, stage_id)`.
2. Find the latest `AgentExecution` for `agent_id` within that stage. Guard: must be `Failed` or `Cancelled`.
3. Create a new `AgentExecution`:
   - Same `stage_execution_id` (stays within the same stage attempt).
   - `agent_attempt_number = previous.agent_attempt_number + 1`.
   - `supersedes_agent_execution_id = previous.id`.
   - Preserve `reused_sibling_execution_ids` from successful siblings (P013 §5.4).
4. Enqueue work item for the new agent execution.
5. If the stage was `Failed` or `Blocked`, update it to `Running`.
6. Return: `{ "retried": true, "new_agent_execution_id": "...", "attempt_number": 2, "journal_id": "..." }`.

**Guard:** Reject if agent_id doesn't exist in this stage or if the agent is still running.

### 5.3 MCP Tool: `approvals.rearm`

```json
{
  "name": "approvals.rearm",
  "description": "Re-open a rejected approval gate for a second decision",
  "input_schema": {
    "type": "object",
    "required": ["run_id", "stage_id"],
    "properties": {
      "run_id": { "type": "string" },
      "stage_id": { "type": "string" },
      "comment": { "type": "string", "description": "Reason for re-arming" }
    }
  }
}
```

**Behavior:**

1. Find the latest `Approval` for `(run_id, stage_id)`. Guard: decision must be `Rejected`.
2. Create a new `Approval` record with `decision = Pending`, `requested_at = now`.
3. Update `StageExecution` status to `WaitingApproval`.
4. Update `Run` status to `WaitingApproval`.
5. Emit `ApprovalRequested` domain event.
6. Return: `{ "rearmed": true, "new_approval_id": "...", "journal_id": "..." }`.

**Guard:** Reject if no rejected approval exists for this stage, or if stage has already been retried.

### 5.4 MCP Tool: `stages.skip`

```json
{
  "name": "stages.skip",
  "description": "Skip a failed or blocked stage and force-advance to the next state",
  "input_schema": {
    "type": "object",
    "required": ["run_id", "stage_id"],
    "properties": {
      "run_id": { "type": "string" },
      "stage_id": { "type": "string" },
      "comment": { "type": "string", "description": "Required: reason for skipping" }
    }
  }
}
```

**Behavior:**

1. Find the latest `StageExecution` for `(run_id, stage_id)`. Guard: must be `Failed` or `Blocked`.
2. Update `StageExecution`: `settlement_kind = Skipped`, `completed_at = now`.
3. Evaluate transitions as if the stage completed normally.
4. If a valid transition exists: schedule the next state. If not: fail with "no valid transition from skipped state".
5. Record the skip decision in `command_journal` with the operator's comment.
6. Return: `{ "skipped": true, "next_state": "state_8_release", "journal_id": "..." }`.

**Guard:** Reject if `comment` is empty (operator must explain why they're skipping). Reject if the stage is `end` type.

### 5.5 MCP Tool: `recovery.evidence`

```json
{
  "name": "recovery.evidence",
  "description": "Get a complete evidence packet for a failed or blocked run",
  "input_schema": {
    "type": "object",
    "required": ["run_id"],
    "properties": {
      "run_id": { "type": "string" },
      "stage_id": { "type": "string", "description": "Optional: focus on a specific stage" }
    }
  }
}
```

**Response:**

```json
{
  "run_id": "...",
  "run_status": "blocked",
  "blocked_stage": {
    "stage_id": "state_5_implementation",
    "label": "Implementation",
    "status": "failed",
    "attempt_number": 1,
    "failure_reason": "Agent 'writer' failed: output contract mismatch"
  },
  "failed_agents": [
    {
      "agent_id": "writer",
      "agent_execution_id": "...",
      "status": "failed",
      "log_snippet": "Missing required field: summary...",
      "provider": "codex",
      "model": "codex-1",
      "session_reuse_disposition": "reused",
      "session_reset_reason": null
    }
  ],
  "validation_failure": {
    "failure_class": "output_contract_mismatch",
    "failure_summary": "report: Missing required fields: summary",
    "missing_fields": ["summary"],
    "recovery_recommendation": {
      "action": "retry_failed_agent",
      "explanation": "Retry the agent with the same inputs."
    }
  },
  "preceding_artifacts": ["proposal_current", "implementation_plan"],
  "session_info": {
    "lineage_id": "...",
    "generation": 3,
    "binding_fingerprint_valid": true,
    "budget_remaining_percent": 62.5
  },
  "available_actions": ["agents.retry", "stages.retry", "stages.skip", "runs.cancel"]
}
```

### 5.6 MCP Tool: `recovery.suggest`

**New — not in Swift app.** Analyzes a failed/blocked run and returns ranked recovery recommendations.

```json
{
  "name": "recovery.suggest",
  "description": "Analyze a failed or blocked run and suggest ranked recovery actions",
  "input_schema": {
    "type": "object",
    "required": ["run_id"],
    "properties": {
      "run_id": { "type": "string" }
    }
  }
}
```

**Response:**

```json
{
  "run_id": "...",
  "recommendations": [
    {
      "rank": 1,
      "action": "agents.retry",
      "params": { "run_id": "...", "stage_id": "state_5_implementation", "agent_id": "writer" },
      "confidence": "high",
      "reason": "Single agent failed with output contract mismatch. Session is healthy (62% budget remaining). Previous attempt produced partial output. Retry is likely to succeed with fresh context."
    },
    {
      "rank": 2,
      "action": "stages.retry",
      "params": { "run_id": "...", "stage_id": "state_5_implementation" },
      "confidence": "medium",
      "reason": "Full stage retry if agent retry fails again. Resets all agent context for a clean slate."
    },
    {
      "rank": 3,
      "action": "stages.skip",
      "params": { "run_id": "...", "stage_id": "state_5_implementation" },
      "confidence": "low",
      "reason": "Skip only if implementation is not required for downstream states. Note: state_6_audit depends on implementation artifacts."
    }
  ]
}
```

**Suggestion logic:**

1. If a single agent failed and session is healthy → suggest `agents.retry` (high confidence).
2. If validation failure indicates contract mismatch → suggest `agents.retry` with note about schema.
3. If session budget is exhausted → suggest `stages.retry` (medium) with session reset.
4. If the run was interrupted mid-transition (cursor exists) → suggest `runs.resume` (high).
5. If stage is non-critical (no downstream artifacts depend on it) → include `stages.skip` (low).
6. If approval was rejected → suggest `approvals.rearm` (medium).
7. Always include `runs.cancel` as last resort.

---

## 6. Migration

### 6.1 New commands

Add to `domain/src/commands.rs`:
- `ResumeRunCmd { run_id }`
- `RetryAgentCmd { run_id, stage_id, agent_id }`
- `RearmApprovalCmd { run_id, stage_id, comment }`
- `SkipStageCmd { run_id, stage_id, comment }`

### 6.2 Command handler

Add cases to `engine/src/command_handler.rs` for each new command. Each records to `command_journal` before execution.

### 6.3 MCP tools

Add `recovery.rs` to `mcp-server/src/tools/` with all 6 tools.

### 6.4 Recovery suggestion engine

Create `engine/src/recovery_suggester.rs`:
- `suggest(pool, run_id) -> Vec<RecoverySuggestion>`
- Pure analysis — reads run state, stage state, agent state, session state, validation records.
- No side effects.

### 6.5 No schema changes

All new tools operate on existing tables. `StageSettlementKind::Skipped` already exists in the domain model.

---

## 7. Verification

- `runs.resume` resumes a run interrupted mid-transition and the run proceeds to the next state.
- `agents.retry` retries only the failed agent; successful siblings are not re-executed.
- `agents.retry` increments `agent_attempt_number` and sets `supersedes_agent_execution_id`.
- `approvals.rearm` creates a new pending approval and emits `ApprovalRequested` event.
- `stages.skip` marks stage as `Skipped`, evaluates transitions, and advances the run.
- `stages.skip` rejects empty comments.
- `recovery.evidence` returns a complete evidence packet in one call.
- `recovery.suggest` returns `agents.retry` as top recommendation for a single-agent failure with healthy session.
- `recovery.suggest` returns `runs.resume` as top recommendation for an interrupted run with a cursor.
- All tools record to `command_journal` with caller identity.
- All tools reject operations on runs in terminal states (Completed, Failed, Cancelled).

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `stages.skip` breaks downstream states that depend on skipped stage's artifacts | Medium | `recovery.suggest` warns about downstream dependencies. `stages.skip` response includes a `warnings` field if downstream states reference the skipped stage's artifacts. |
| `approvals.rearm` allows infinite re-arming loops | Low | Log re-arm count per stage in command journal. Add optional `max_rearms` field to workflow state definition (future). |
| `recovery.suggest` gives bad advice | Medium | Suggestions are ranked with confidence levels. The operator always makes the final decision. Suggestions are deterministic (no LLM involved), based on concrete state inspection. |
| `agents.retry` within a multi-agent stage has ordering implications | Low | New agent execution runs in isolation. Stage completion is re-evaluated when all agents have a terminal status. |
| `runs.resume` and startup recovery race on daemon restart | Low | `runs.resume` checks for active work items first. Startup recovery runs before MCP tools are available. |
| `stages.skip` could be abused to bypass critical stages | Medium | `comment` is required and recorded in audit journal. Can add `skippable: false` flag to workflow states in a future proposal. |
