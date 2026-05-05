# Proposal 045: Run Recovery and Granular Retry MCP Tools

| Field | Value |
|---|---|
| Date | 2026-04-17 (revised R2) |
| Status | Draft (R2 — readiness blockers addressed: schema/runtime ownership, retry lineage, skip policy, MCP auth/namespace, distinct gate, command/query journaling split) |
| Author | Andrey Khasanov |
| Depends on | Server parity harness contracts in [p041-generated-artifact-schemas.md](../reference/p041-generated-artifact-schemas.md) and the retained `proposal-041|p041` gate in [test-gates.md#proposal-041p041](../reference/test-gates.md#proposal-041p041) |
| Scope | Add run resume, agent-level retry, approval gate re-arm, stage skip, evidence retrieval, and deterministic recovery suggestions to the MCP tool surface, including the required Rust schema/runtime/auth/gate work. |
| Goal | The operator can recover from common blocked/interrupted run states through MCP tools with durable audit, safe retry lineage, and deterministic recovery recommendations. |

**Gate naming note:** `proposal-045|p045` is already owned by deterministic release operations in `scripts/test-gate.sh` and `docs/reference/test-gates.md`. This proposal uses the distinct canonical gate alias `proposal-045-recovery|p045-recovery`; it must not replace or repurpose the existing deterministic-release gate.

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
- 4 new mutating command variants: `ResumeRunCmd`, `RetryAgentCmd`, `RearmApprovalCmd`, `SkipStageCmd`.
- 2 read-only direct-query tools: `recovery.evidence`, `recovery.suggest`.
- Run resume cursor schema and orchestrator updates for durable transition resume.
- Agent retry lineage schema and executor/orchestrator updates for same-stage agent retry.
- Workflow skip policy metadata and safe skip transition semantics.
- MCP namespace dispatch, typed `CapabilityToolId` registration, principal-class policy, and discovery/auth tests.
- Dedicated proof gate `proposal-045-recovery|p045-recovery`.
- Guard-rail validations for each tool.
- Recovery suggestion engine with deterministic ranked recommendations.

This proposal does **not** include:

- GraphQL equivalents (MCP-first; GraphQL can follow).
- LLM/AI-ranked recovery advice. `recovery.suggest` is deterministic and testable.
- ACP provider adapter changes.
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
   - If `runs.transition_cursor_json` exists and `runs.transition_settlement_state = "next_state_scheduled_not_started"`: resume by enqueueing a single `AdvanceRun` for the scheduled next state.
   - Else: fall back to latest stage catchup. If the latest non-terminal stage is `Blocked`, enqueue `AdvanceRun` for the run so the orchestrator can re-evaluate it. Do not silently re-run a provider task from a `Running` stage; mark it `Blocked` with drift details first, matching startup recovery's fail-closed behavior.
3. Atomically claim resume ownership:
   - Reject terminal runs.
   - Reject if any pending/running work item exists for the run.
   - Reject or return idempotent response if `resume_claim_status` is `claimed` or `enqueued`.
   - Set `resume_claim_id`, `resume_claim_status = "claimed"`, `resume_claimed_at`, and run status `Running` in the same transaction-equivalent path before enqueueing work.
   - After the `AdvanceRun` work item is durably inserted, set `resume_claim_status = "enqueued"` and `resume_enqueued_work_item_id`.
4. Return: `{ "resumed": true, "from_state": "state_5_implementation", "resume_claim_id": "...", "journal_id": "..." }`.

**Guard:** Reject if run has active work items in the queue (already being processed).

**Required durable cursor contract:**

Add nullable fields to `Run` / `runs`:

- `transition_cursor_json`: JSON payload containing `{ "from_state": "...", "to_state": "...", "reason": "...", "created_at": "..." }`.
- `transition_settlement_state`: enum-like string; P045 uses `next_state_scheduled_not_started`, `advance_run_enqueued`, and `cleared`.
- `resume_claim_id`: UUID string set while an operator-triggered resume is being scheduled.
- `resume_claim_status`: enum-like string: `claimed`, `enqueued`, `completed`, `failed`, or `cleared`.
- `resume_claimed_at`: timestamp for diagnostics and stale-claim repair.
- `resume_enqueued_work_item_id`: the `AdvanceRun` work item created by the active claim.
- `resume_claim_error`: last enqueue/settlement error for failed claims.

The orchestrator owns writing `transition_cursor_json` before scheduling the next state and clearing it once the next state has an active or terminal stage. `runs.resume` reads this cursor; it does not infer cursor truth from logs or command journal rows.

**Resume claim lifecycle:**

- `claimed`: command owns the resume attempt but has not durably enqueued work yet. If enqueue fails, set `failed` with `resume_claim_error`.
- `enqueued`: `resume_enqueued_work_item_id` exists and is pending/running/completed. Repeated `runs.resume` returns an idempotent response with the same `resume_claim_id` while the work item is pending/running.
- `completed`: the enqueued work item completed and the orchestrator either cleared the transition cursor or created/advanced the next stage. After recording completion, clear `resume_claim_id`, `resume_claim_status`, and `resume_enqueued_work_item_id`.
- `failed`: enqueue or resume execution failed before the run advanced. Repeated `runs.resume` may retry only after verifying there is no pending/running work item for the claim, then replacing the failed claim atomically.
- `cleared`: startup repair found an abandoned stale claim and cleared it after verifying no active work item exists.

Startup recovery repairs stale claims before MCP tools are available:

- `claimed` older than the stale threshold with no enqueued work item -> mark `failed` or `cleared` with drift details.
- `enqueued` whose work item is missing -> re-enqueue exactly one `AdvanceRun` under the same claim and update `resume_enqueued_work_item_id`.
- `enqueued` whose work item is completed but cursor remains uncleared -> enqueue one catchup `AdvanceRun` or mark the run `Blocked` with drift details.

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
3. Atomically create a retry claim and queued execution:
   - Same `stage_execution_id` (stays within the same stage attempt).
   - `status = Queued`; do not mark it `Running` until the executor claims the work item and starts provider work.
   - `queued_at = now`; `started_at = null` until executor claim/start.
   - `agent_attempt_number = previous.agent_attempt_number + 1`.
   - `supersedes_agent_execution_id = previous.id`.
   - `reused_sibling_execution_ids` records successful sibling executions that remain valid for this stage attempt.
   - `retry_claim_id` and `retry_work_item_id` are written with the paired work item.
4. Enqueue an `InvokeAgent` work item that names the pre-created `agent_execution_id`; the executor must consume that ID instead of creating an unrelated lineage entry.
5. If the stage was `Failed` or `Blocked`, update it to `Running`.
6. Return: `{ "retried": true, "new_agent_execution_id": "...", "attempt_number": 2, "journal_id": "..." }`.

**Guard:** Reject if `agent_id` doesn't exist in this stage, if the agent is still running, or if a non-terminal retry attempt (`Queued` or `Running`) already supersedes the same prior execution.

**Required retry-lineage contract:**

Add nullable fields to `AgentExecution` / `agent_executions`:

- `AgentStatus::Queued` in the domain model. Queued means a durable work item exists or is being transactionally created; provider work has not started.
- `agent_attempt_number INTEGER NOT NULL DEFAULT 1`.
- `supersedes_agent_execution_id TEXT NULL` referencing the prior failed/cancelled attempt.
- `reused_sibling_execution_ids_json TEXT NULL` storing a JSON array of sibling `AgentExecutionId`s whose successful outputs are reused for stage completion.
- `retry_claim_id TEXT NULL`.
- `retry_work_item_id TEXT NULL`.
- `queued_at TEXT NULL`.

Stage completion must consider only the latest non-superseded attempt for each agent. Superseded failed/cancelled attempts remain queryable as historical evidence but must not keep the stage failed after a later same-agent retry succeeds.

**Retry queue atomicity and repair:**

`agents.retry` must use a single transaction-equivalent repository helper that inserts the queued `AgentExecution`, inserts the paired pending `InvokeAgent` work item, and writes `retry_work_item_id` back onto the execution. There must be no durable state where a retry execution exists without either a paired work item or a failed/cleared retry claim.

Executor behavior:

- On work-item claim/start, update the pre-created execution from `Queued` to `Running` and set `started_at`.
- On completion/failure/cancellation, update the same execution row.
- If executor sees an `InvokeAgent` payload with an unknown `agent_execution_id`, fail the work item and do not create an implicit replacement execution.

Startup recovery behavior:

- `Queued` retry execution with missing pending/running work item -> re-enqueue one paired `InvokeAgent` and update `retry_work_item_id`.
- `Queued` retry execution with stale pending work item -> leave it to work-queue claim unless the work item is failed/cancelled, then mark the retry execution `Failed` with recovery details or re-enqueue under the same `retry_claim_id`.
- Duplicate `agents.retry` calls for the same superseded execution return the existing queued/running retry or reject consistently; they do not create another attempt until the prior retry reaches a terminal status.

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
2. Reject if a pending/requested approval already exists for the same `(run_id, stage_id)`.
3. Reject if the stage has already been retried after the rejected approval.
4. Create a new `Approval` record with `decision = Pending`, `requested_at = now`, and lineage back to the rejected approval.
5. Update `StageExecution` status to `WaitingApproval`.
6. Update `Run` status to `WaitingApproval`.
7. Emit `ApprovalRequested` domain event.
8. Return: `{ "rearmed": true, "new_approval_id": "...", "supersedes_approval_id": "...", "journal_id": "..." }`.

**Guard:** Reject if no rejected approval exists for this stage, or if stage has already been retried.

**Required approval re-arm contract:**

Add durable approval lineage fields:

- `approvals.supersedes_approval_id TEXT NULL`
- `approvals.rearm_sequence INTEGER NOT NULL DEFAULT 0`

`approvals.rearm` increments `rearm_sequence` from the rejected approval and rejects when `rearm_sequence >= 1`. A future workflow policy can raise that limit, but P045's safe default is one re-arm per stage attempt.

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
2. Load workflow state metadata and reject if the state is not explicitly skippable.
3. Reject manual approval, release, delivery, security, audit, and end states by default unless the workflow marks the state `recovery.skippable: true` and `recovery.skip_allows_artifact_gap: true`.
4. Compute downstream artifact dependencies from workflow transition conditions and prompt input artifact maps. Reject if any downstream required artifact is produced only by the skipped state and no explicit synthetic replacement is configured.
5. Build a skip plan in memory:
   - candidate transition and `next_state`
   - dependency analysis
   - warnings
   - skip evidence payload
   - work item(s) that would be scheduled
6. Evaluate transitions with a synthetic condition context `stage_skipped(stage_id) = true`; do not pretend required artifacts exist.
7. If no valid transition exists, do not settle the stage. Record the failed skip attempt only in `command_journal` / non-settlement skip-attempt audit and return an error. The latest `StageExecution` remains `Failed` or `Blocked`.
8. If a valid transition exists, atomically settle the current stage as `Skipped`, persist committed `skip_evidence_json`, and schedule the next state work item(s) in the same transaction-equivalent path.
9. Return: `{ "skipped": true, "next_state": "state_8_release", "warnings": [], "journal_id": "..." }`.

**Guard:** Reject if `comment` is empty (operator must explain why they're skipping). Reject if the stage is `end` type.

**Required workflow skip policy:**

P045 adds optional workflow metadata:

```yaml
recovery:
  skippable: true
  skip_allows_artifact_gap: false
  skip_reason_required: true
```

Default is fail-closed: states are not skippable unless explicitly marked. The runtime must treat skip as an operator override with its own evidence, not as normal completion.

**Skip mutation ordering:**

`Skipped` is terminal stage truth, so it must not be written until the runtime has already proven that a valid transition exists and can be scheduled. Failed skip attempts are audit-only; they do not update `StageExecution.status`, `settlement_kind`, or `completed_at`.

Committed skip evidence lives on the skipped stage only after the atomic settle-and-schedule step. Failed skip-attempt evidence lives in the command journal payload or a separate non-settlement audit field such as `skip_attempt_evidence_json`; it must not make the skipped stage look terminal.

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

`recovery.evidence` is read-only. It does not create a command journal row and does not return `journal_id`.

### 5.6 MCP Tool: `recovery.suggest`

**New — not in Swift app.** Analyzes a failed/blocked run and returns deterministic ranked recovery recommendations.

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
4. If the run was interrupted mid-transition (`transition_cursor_json` exists) → suggest `runs.resume` (high).
5. If stage is non-critical (no downstream artifacts depend on it) → include `stages.skip` (low).
6. If approval was rejected → suggest `approvals.rearm` (medium).
7. Always include `runs.cancel` as last resort.

`recovery.suggest` is read-only and deterministic. It does not use an LLM and does not create a command journal row. The response must include enough evidence references for the operator to understand the recommendation source.

---

## 6. Migration

### 6.1 Schema changes

Add migrations for:

| Table | Fields |
|---|---|
| `runs` | `transition_cursor_json`, `transition_settlement_state`, `resume_claim_id`, `resume_claim_status`, `resume_claimed_at`, `resume_enqueued_work_item_id`, `resume_claim_error` |
| `agent_executions` | `agent_attempt_number`, `supersedes_agent_execution_id`, `reused_sibling_execution_ids_json`, `retry_claim_id`, `retry_work_item_id`, `queued_at` |
| `approvals` | `supersedes_approval_id`, `rearm_sequence` |
| `stage_executions` | `skip_evidence_json`, optional non-settlement `skip_attempt_evidence_json` if failed skip attempts are stored outside command journal |

All new fields are nullable except counters with safe defaults. Existing rows backfill to legacy behavior:

- Runs without `transition_cursor_json` can only use latest-stage catchup.
- Agent executions without attempt fields are treated as attempt `1` and non-superseded.
- Approvals without rearm fields are treated as original approvals with sequence `0`.

Add `AgentStatus::Queued` and update all status parsers/serializers, DB mappings, projections, and tests that enumerate agent statuses. Queued retry executions are not provider-active until executor claim/start moves them to `Running`.

### 6.2 New commands

Add to `domain/src/commands.rs`:
- `ResumeRunCmd { run_id }`
- `RetryAgentCmd { run_id, stage_id, agent_id }`
- `RearmApprovalCmd { run_id, stage_id, comment }`
- `SkipStageCmd { run_id, stage_id, comment }`

Do not add `SuggestRecoveryCmd`. `recovery.suggest` is a read-only query tool.

### 6.3 Command handler

Add cases to `engine/src/command_handler.rs` for each new command. Each records to `command_journal` before execution.

Command result variants:

- `RunResumed { run_id, from_state, resume_claim_id }`
- `AgentRetried { run_id, stage_id, new_agent_execution_id, attempt_number }`
- `ApprovalRearmed { run_id, stage_id, new_approval_id, supersedes_approval_id }`
- `StageSkipped { run_id, stage_id, next_state, warnings }`

### 6.4 MCP tools and namespace dispatch

Add:

- `mcp-server/src/tools/recovery.rs` for `recovery.evidence` and `recovery.suggest`.
- `agents.retry` support, either in a new `tools/agents.rs` module or in a recovery module with explicit `agents.*` dispatch.
- `runs.resume`, `approvals.rearm`, and `stages.skip` support in their existing namespace modules.

Update `mcp-server/src/server.rs` namespace dispatch for `agents.*` and `recovery.*`. Update `mcp-server/src/tools/mod.rs` `all_tool_specs`, `all_capability_tool_ids`, `capability_id_for`, and `mcp_tool_for`.

### 6.5 Capability and principal policy

Add exact typed capability IDs:

| Tool | CapabilityToolId | Principal classes |
|---|---|---|
| `runs.resume` | `RunsResume` | Operator |
| `agents.retry` | `AgentsRetry` | Operator |
| `approvals.rearm` | `ApprovalsRearm` | Operator |
| `stages.skip` | `StagesSkip` | Operator |
| `recovery.evidence` | `RecoveryEvidence` | Operator, Observer |
| `recovery.suggest` | `RecoverySuggest` | Operator, Observer |

Mutating recovery tools are operator-only. Read-only evidence/suggest tools are visible to operators and observers. Agents do not receive these capabilities by default because these tools can reveal operational evidence or mutate execution state.

Update `domain::CapabilityToolId`, `auth::all_tool_capabilities`, `auth::tool_allowed_for_class`, MCP converter mappings, and discovery tests.

### 6.6 Recovery evidence and suggestion engines

Create `engine/src/recovery_suggester.rs`:
- `suggest(pool, run_id) -> Vec<RecoverySuggestion>`
- Pure analysis — reads run state, stage state, agent state, session state, validation records.
- No side effects.

Create `engine/src/recovery_evidence.rs` or reuse `engine::evidence` with a thin assembler:
- Reads canonical failed-stage evidence from `stage_executions.evidence_packet_json`.
- Reads validation records from the validation repository.
- Reads session/provenance fields from agent execution/session owners.
- Does not create command journal rows.

### 6.7 Runtime integration

Required runtime changes:

- Orchestrator writes, settles, and clears durable transition cursor and resume claim fields.
- `agents.retry` inserts the queued execution and paired pending work item atomically; `InvokeAgent` work item payload names a pre-created `agent_execution_id`.
- Executor consumes command-created retry executions instead of always creating a new unrelated `AgentExecution`.
- Stage completion logic ignores superseded failed/cancelled agent attempts and evaluates latest non-superseded attempt per agent.
- Stage skip builds a transition plan before mutating stage settlement; transition evaluator supports explicit skip override context without fabricating artifact truth.
- Startup recovery and `runs.resume` share active-work/idempotency checks to avoid duplicate enqueue.

### 6.8 Workflow schema/compiler ownership

Add workflow-owned recovery metadata so `stages.skip` can read policy from the compiled plan:

| File | Change |
|---|---|
| `workflow/src/definition.rs` | Add `WorkflowState.recovery: Option<RecoveryPolicy>` with `skippable`, `skip_allows_artifact_gap`, and `skip_reason_required` |
| `workflow/src/plan.rs` | Add `CompiledState.recovery: Option<CompiledRecoveryPolicy>` |
| `workflow/src/compiler.rs` | Copy and validate recovery policy from YAML state into `CompiledState` |
| `workflow/tests/integration.rs` | Prove recovery metadata survives YAML parsing and compilation |
| `examples/workflows/*.yaml` / test fixtures | Add explicit `recovery` metadata only to safe sample states used by P045 tests |

Parser/compiler requirements:

- Unknown `recovery` keys fail compilation, rather than being silently ignored.
- Default compiled policy is fail-closed: `skippable = false`, `skip_allows_artifact_gap = false`, `skip_reason_required = true`.
- Round-trip or compile tests assert the engine can read `plan.states[state_id].recovery`.

### 6.9 Gate ownership

Add `proposal-045-recovery|p045-recovery` to `scripts/test-gate.sh` and `docs/reference/test-gates.md`. Preserve the existing deterministic-release `proposal-045|p045` entries unchanged.

### 6.10 Files to modify

| File | Change |
|---|---|
| `domain/src/run.rs` | Add transition cursor and resume claim fields |
| `domain/src/agent.rs` | Add `Queued` status and agent retry lineage fields |
| `domain/src/approval.rs` | Add approval re-arm lineage fields |
| `domain/src/stage.rs` | Add committed skip evidence and optional failed skip-attempt audit field/readback |
| `domain/src/commands.rs` | Add four mutating recovery command variants |
| `domain/src/capabilities.rs` | Add recovery capability IDs |
| `db/migrations/*_run_recovery_tools.sql` | Add run cursor, retry lineage, approval lineage, and skip evidence columns |
| `db/src/repos/{runs,agent_executions,approvals,stages,work_items}.rs` | Persist/read new fields and add claim/lineage/atomic enqueue helpers |
| `workflow/src/{definition,plan,compiler}.rs` | Parse, compile, and expose recovery skip policy metadata |
| `workflow/tests/integration.rs` | Prove recovery metadata parsing/compiled readback |
| `engine/src/command_handler.rs` | Execute mutating recovery commands and return command results |
| `engine/src/orchestrator.rs` | Persist transition cursors, consume skip policy/context, evaluate latest non-superseded agent attempts |
| `engine/src/executor.rs` | Consume command-created retry `agent_execution_id` in `InvokeAgent` payload |
| `engine/src/recovery.rs` | Share active-work/idempotency helpers with on-demand resume |
| `engine/src/recovery_suggester.rs` | Add deterministic recovery suggestion rules |
| `engine/src/recovery_evidence.rs` / `engine/src/evidence.rs` | Assemble recovery evidence from canonical owners |
| `auth/src/lib.rs` | Add class policy for new capability IDs |
| `mcp-server/src/server.rs` | Dispatch `agents.*` and `recovery.*` namespaces |
| `mcp-server/src/tools/{runs,agents,approvals,stages,recovery}.rs` | Register and execute the new tools |
| `mcp-server/src/tools/mod.rs` | Update tool discovery and capability mapping |
| `scripts/test-gate.sh` | Add `proposal-045-recovery|p045-recovery` without changing `proposal-045|p045` |
| `docs/reference/test-gates.md` | Document the new recovery gate |

---

## 7. Verification

Canonical gate:

```bash
./scripts/test-gate.sh proposal-045-recovery
```

The runner also accepts `p045-recovery`. `./scripts/test-gate.sh proposal-045` remains deterministic release operations.

Focused proof inventory:

- `runs.resume` resumes a run interrupted mid-transition and the run proceeds to the next state.
- `runs.resume` rejects terminal runs, active-work runs, and duplicate resume claims.
- `runs.resume` repeated calls return the documented response for each claim state: `claimed`, `enqueued`, `completed`, `failed`, and `cleared`.
- Stale resume claims are repaired by startup recovery without leaving permanent active claims.
- Orchestrator persists and clears `transition_cursor_json` / `transition_settlement_state` and settles `resume_claim_status`.
- Startup recovery and on-demand resume do not enqueue duplicate `AdvanceRun` items for the same run.
- `agents.retry` retries only the failed agent; successful siblings are not re-executed.
- `agents.retry` creates a `Queued` retry execution atomically with the paired pending `InvokeAgent` work item.
- `agents.retry` increments `agent_attempt_number`, sets `supersedes_agent_execution_id`, and records `reused_sibling_execution_ids_json`, `retry_claim_id`, and `retry_work_item_id`.
- `agents.retry` rejects running agents, unknown agents, terminal runs, and repeated retry while a queued/running retry work item is active.
- Startup recovery repairs queued retry executions with missing or stale work-item linkage.
- Executor consumes a pre-created retry `agent_execution_id` from `InvokeAgent` payload.
- Stage completion ignores superseded failed attempts after a later same-agent retry succeeds.
- `approvals.rearm` creates a new pending approval, links to the rejected approval, increments `rearm_sequence`, and emits `ApprovalRequested`.
- `approvals.rearm` rejects duplicate pending approval, already-retried stage, and second re-arm for the same stage attempt.
- `stages.skip` rejects empty comments.
- `stages.skip` rejects states without explicit `recovery.skippable: true`.
- Workflow parser/compiler tests prove `recovery` metadata survives YAML parsing into `CompiledState`.
- `stages.skip` rejects manual approval, release, delivery, security, audit, and end states by default.
- `stages.skip` rejects artifact-dependent transitions unless the workflow explicitly allows the artifact gap.
- `stages.skip` builds and validates a skip plan before mutating stage settlement.
- Failed skip attempts do not mark the latest stage `Skipped`; they remain command-journal/audit evidence only.
- Successful `stages.skip` atomically settles the stage as `Skipped`, records committed skip evidence, and schedules the next state.
- `recovery.evidence` returns a complete evidence packet in one call from canonical stage/evidence/validation/session owners.
- `recovery.suggest` returns `agents.retry` as top deterministic recommendation for a single-agent failure with healthy session.
- `recovery.suggest` returns `runs.resume` as top deterministic recommendation for an interrupted run with a cursor.
- Mutating tools (`runs.resume`, `agents.retry`, `approvals.rearm`, `stages.skip`) record to `command_journal` with caller identity and return `journal_id`.
- Read-only tools (`recovery.evidence`, `recovery.suggest`) do not create command journal rows and do not return `journal_id`.
- MCP discovery exposes all six tools under the correct namespaces.
- Capability converter tests cover all six tools.
- Principal policy tests prove mutating tools are operator-only and read-only recovery tools are operator/observer.
- Unknown `agents.*` and `recovery.*` namespace/tools still fail closed.

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| `stages.skip` breaks downstream states that depend on skipped stage's artifacts | High | Runtime computes downstream artifact dependencies and rejects by default; skip evidence records the dependency analysis. |
| `approvals.rearm` allows infinite re-arming loops | Medium | Add approval lineage and enforce one re-arm per stage attempt in P045. |
| `recovery.suggest` gives bad advice | Medium | Suggestions are ranked with confidence levels. The operator always makes the final decision. Suggestions are deterministic (no LLM involved), based on concrete state inspection. |
| `agents.retry` within a multi-agent stage has ordering implications | Medium | Add durable retry lineage and evaluate stage completion from latest non-superseded attempt per agent. |
| `runs.resume` and startup recovery race on daemon restart | Medium | Shared active-work and resume-claim guards prevent duplicate enqueue. |
| `stages.skip` could be abused to bypass critical stages | High | Mutating skip is operator-only, requires comment, requires explicit workflow skippability, rejects critical stage families by default, and records skip evidence. |
| Schema migration increases implementation blast radius | Medium | New fields are nullable/defaulted and legacy rows retain existing behavior. |
| Capability policy accidentally exposes mutating recovery tools | High | Explicit capability IDs and class-policy tests make mutating tools operator-only. |
