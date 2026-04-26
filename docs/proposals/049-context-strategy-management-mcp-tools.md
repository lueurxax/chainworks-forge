# Proposal 049: Context Strategy Management MCP Tools

| Field | Value |
|---|---|
| Date | 2026-04-17 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [043-query-projections-and-client-consumption-contract.md](043-query-projections-and-client-consumption-contract.md) |
| Scope | Add context strategy assignment, querying, pressure monitoring, handoff compilation, and simulation to the MCP tool surface, with a file-based strategy profile registry. |
| Goal | Orchestrating agents can manage context budgets dynamically during execution, and operators can simulate strategy impact before starting runs. |

---

## 1. Context and Motivation

The Swift app has Context Strategy support (Proposal 019): strategy profile assignment, handoff packet compilation, limit pressure tracking, promoted artifact management. The control-plane has **none of this** — no strategy profiles, no handoff logic, no budget tracking.

Context strategy matters because agent invocations have finite context windows. A Code Writer receiving the full proposal, all review feedback, implementation plan, and audit report may exceed the context limit. The strategy profile controls:

- How much context each agent receives (max payload bytes).
- Which artifacts are promoted (all, latest per contract, none).
- How context is handed off between agents (full, summary, references only).
- When to warn about budget pressure.

Without MCP tools for strategy management, the orchestrating agent cannot:
1. Choose a strategy profile at run start.
2. Check whether the current context is within budget mid-run.
3. Adjust handoff behavior when approaching limits.
4. Predict budget issues before they cause failures.

---

## 2. Product Questions This Proposal Must Answer

1. Can the operator or orchestrating agent assign a strategy profile at run start or mid-run?
2. Can the agent query current pressure levels in real time?
3. Can the agent inspect what artifacts will be handed to the next stage?
4. Can the operator simulate strategy impact for an entire run before starting?
5. Does the system provide actionable recommendations when pressure is high?

---

## 3. Scope

This proposal includes:

- 6 new MCP tools: `strategy.assign`, `strategy.get`, `strategy.profiles`, `strategy.pressure`, `strategy.handoff`, `strategy.simulate`.
- Strategy profile registry loaded from `strategy-profiles.yaml`.
- Pressure signal recording in `strategy_pressure_signals` table.
- Run model additions for strategy tracking.

This proposal does **not** include:

- Automatic strategy switching (all tools require explicit invocation).
- GraphQL equivalents (strategy is primarily an agent-facing concern via MCP).
- Changes to the ACP transport payload format.
- Token counting at the transport level (estimates based on byte size).

---

## 4. Problem Statement

### 4.1 No strategy profile concept in control-plane

The Swift app has `contextStrategyProfileID` on the Run model. The Rust `Run` struct has no equivalent field. There is no strategy profile registry.

### 4.2 No pressure monitoring

The Swift app tracks `inputPayloadBytes` and `limitPressureSignalsJSON` per `AgentExecution`. The Rust model has no pressure tracking. When an agent approaches its context limit, nothing warns the operator or orchestrator.

### 4.3 No handoff control

The Swift app compiles handoff packets with promoted artifacts filtered by policy. The control-plane passes a flat prompt string to each agent with no artifact-aware handoff logic.

### 4.4 No pre-execution prediction

Neither system can predict before execution whether a given strategy profile will cause pressure issues for specific agents.

---

## 5. Core Product Behavior

### 5.1 MCP Tool: `strategy.profiles`

List available strategy profiles.

```json
{
  "name": "strategy.profiles",
  "description": "List available context strategy profiles",
  "input_schema": {
    "type": "object",
    "properties": {}
  }
}
```

**Response:**

```json
{
  "profiles": [
    {
      "id": "current_mixed_baseline",
      "description": "Default balanced profile",
      "max_input_payload_bytes": 100000,
      "preferred_model_tier": "high",
      "handoff_mode": "full_context",
      "promoted_artifact_policy": "latest_per_contract",
      "budget_guard_threshold_percent": 80
    },
    {
      "id": "lean_fast",
      "description": "Minimal context for speed-sensitive runs",
      "max_input_payload_bytes": 50000,
      "preferred_model_tier": "medium",
      "handoff_mode": "summary_only",
      "promoted_artifact_policy": "none",
      "budget_guard_threshold_percent": 90
    },
    {
      "id": "deep_analysis",
      "description": "Maximum context for thorough review and audit",
      "max_input_payload_bytes": 200000,
      "preferred_model_tier": "high",
      "handoff_mode": "full_context",
      "promoted_artifact_policy": "all",
      "budget_guard_threshold_percent": 70
    }
  ]
}
```

### 5.2 MCP Tool: `strategy.assign`

```json
{
  "name": "strategy.assign",
  "description": "Assign a context strategy profile to a run",
  "input_schema": {
    "type": "object",
    "required": ["run_id", "strategy_profile_id"],
    "properties": {
      "run_id": { "type": "string" },
      "strategy_profile_id": { "type": "string" }
    }
  }
}
```

**Behavior:**

1. Validate that `strategy_profile_id` exists in the registry.
2. Persist `context_strategy_profile_id` on the Run.
3. Snapshot the profile parameters as `context_strategy_snapshot_json` (frozen at assignment time).
4. Return the assigned profile.

**Guard:** Reject if the profile ID does not exist. Allow mid-run reassignment (the new profile applies to subsequent stages only).

### 5.3 MCP Tool: `strategy.get`

```json
{
  "name": "strategy.get",
  "description": "Get the current strategy profile and pressure state for a run",
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
  "strategy_profile_id": "current_mixed_baseline",
  "profile": { "...same as profiles response..." },
  "current_pressure": {
    "overall_level": "yellow",
    "latest_input_payload_bytes": 78000,
    "budget_used_percent": 78,
    "stages_with_pressure": [
      { "stage_id": "state_5_implementation", "agent_id": "writer", "pressure_percent": 82, "level": "yellow" }
    ]
  }
}
```

### 5.4 MCP Tool: `strategy.pressure`

Real-time pressure query for a specific agent execution.

```json
{
  "name": "strategy.pressure",
  "description": "Query context budget pressure for an agent in a run",
  "input_schema": {
    "type": "object",
    "required": ["run_id"],
    "properties": {
      "run_id": { "type": "string" },
      "stage_id": { "type": "string", "description": "Optional: focus on specific stage" },
      "agent_id": { "type": "string", "description": "Optional: focus on specific agent" }
    }
  }
}
```

**Response:**

```json
{
  "signals": [
    {
      "stage_id": "state_5_implementation",
      "agent_id": "writer",
      "input_payload_bytes": 82000,
      "max_payload_bytes": 100000,
      "estimated_tokens": 20500,
      "model_context_window": 128000,
      "pressure_percent": 82,
      "level": "yellow",
      "recommendations": [
        "Switch to 'summary_only' handoff mode for remaining stages",
        "Drop older review artifacts (keep only latest revision)"
      ]
    }
  ]
}
```

**Pressure levels:**
- **green** (<60%): No action needed.
- **yellow** (60–85%): Approaching limit. Recommendations available.
- **red** (>85%): At risk of exceeding context window. Urgent action recommended.

**Improvement over Swift**: The Swift app tracks pressure internally but doesn't expose recommendations. The MCP tool provides actionable suggestions the orchestrating agent can act on.

### 5.5 MCP Tool: `strategy.handoff`

Get the compiled handoff packet for the next agent.

```json
{
  "name": "strategy.handoff",
  "description": "Get the compiled handoff packet from one stage to another",
  "input_schema": {
    "type": "object",
    "required": ["run_id", "from_stage_id", "to_stage_id"],
    "properties": {
      "run_id": { "type": "string" },
      "from_stage_id": { "type": "string" },
      "to_stage_id": { "type": "string" }
    }
  }
}
```

**Response:**

```json
{
  "handoff_mode": "full_context",
  "promoted_artifacts": [
    { "name": "proposal_current", "format": "markdown", "size_bytes": 12400, "stage_origin": "state_2_proposal" },
    { "name": "implementation_plan", "format": "json", "size_bytes": 8200, "stage_origin": "state_4_plan" }
  ],
  "total_handoff_bytes": 20600,
  "within_budget": true,
  "budget_remaining_bytes": 79400,
  "summary": null
}
```

When `handoff_mode` is `summary_only`, the `summary` field contains a text summary of preceding work instead of full artifact content. When `artifact_references`, only file paths are included.

### 5.6 MCP Tool: `strategy.simulate`

Simulate strategy impact for an entire run without executing.

```json
{
  "name": "strategy.simulate",
  "description": "Simulate context strategy for a run, predicting pressure points",
  "input_schema": {
    "type": "object",
    "required": ["strategy_profile_id"],
    "properties": {
      "run_id": { "type": "string", "description": "Existing run (uses actual artifact sizes)" },
      "workflow_yaml_path": { "type": "string", "description": "Or provide paths for a hypothetical run" },
      "agent_catalog_yaml_path": { "type": "string" },
      "strategy_profile_id": { "type": "string" }
    }
  }
}
```

**Response:**

```json
{
  "strategy_profile_id": "lean_fast",
  "stages": [
    {
      "ordinal": 1,
      "state_id": "state_1_idea",
      "agent_id": "lead",
      "estimated_input_bytes": 5000,
      "max_bytes": 50000,
      "pressure_percent": 10,
      "level": "green"
    },
    {
      "ordinal": 5,
      "state_id": "state_5_implementation",
      "agent_id": "writer",
      "estimated_input_bytes": 48000,
      "max_bytes": 50000,
      "pressure_percent": 96,
      "level": "red",
      "warning": "Estimated payload exceeds budget. Consider 'current_mixed_baseline' profile or drop older artifacts."
    }
  ],
  "overall_verdict": "warning",
  "pressure_points": 1,
  "recommendation": "Profile 'lean_fast' will likely cause budget pressure at state_5_implementation. Consider 'current_mixed_baseline' which allows 100KB payloads."
}
```

**Not in Swift**: Strategy simulation is entirely new. The Swift app has no pre-execution prediction.

**Estimation logic:**
- For existing runs (`run_id` provided): use actual artifact sizes from the database.
- For hypothetical runs (`yaml paths` provided): use average artifact sizes from completed runs with the same workflow, or defaults (proposal: 15KB, review: 5KB, plan: 8KB, implementation: 40KB).

---

## 6. Migration

### 6.1 Run model additions

Add to `domain/src/run.rs`:

```rust
pub context_strategy_profile_id: Option<String>,
pub context_strategy_snapshot_json: Option<String>,
```

### 6.2 New table

```sql
CREATE TABLE IF NOT EXISTS strategy_pressure_signals (
    id TEXT PRIMARY KEY,
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    agent_id TEXT NOT NULL,
    input_payload_bytes INTEGER NOT NULL,
    estimated_tokens INTEGER,
    model_context_window INTEGER,
    pressure_percent REAL NOT NULL,
    level TEXT NOT NULL,
    recorded_at TEXT NOT NULL,
    UNIQUE(run_id, stage_id, agent_id)
);
CREATE INDEX idx_pressure_signals_run_id ON strategy_pressure_signals(run_id);
```

### 6.3 Strategy profile registry

Create `engine/src/strategy_registry.rs`:
- Loads profiles from `strategy-profiles.yaml` at startup.
- `StrategyRegistry::get(id) -> Option<StrategyProfile>`
- `StrategyRegistry::list() -> Vec<StrategyProfile>`

### 6.4 Configuration file

`strategy-profiles.yaml` alongside workflow and catalog:

```yaml
schema_version: 1
profiles:
  current_mixed_baseline:
    description: Default balanced profile
    max_input_payload_bytes: 100000
    preferred_model_tier: high
    handoff_mode: full_context
    promoted_artifact_policy: latest_per_contract
    budget_guard_threshold_percent: 80

  lean_fast:
    description: Minimal context for speed
    max_input_payload_bytes: 50000
    preferred_model_tier: medium
    handoff_mode: summary_only
    promoted_artifact_policy: none
    budget_guard_threshold_percent: 90

  deep_analysis:
    description: Maximum context for thorough work
    max_input_payload_bytes: 200000
    preferred_model_tier: high
    handoff_mode: full_context
    promoted_artifact_policy: all
    budget_guard_threshold_percent: 70
```

### 6.5 MCP tools

Add `strategy.rs` to `mcp-server/src/tools/` with all 6 tools.

### 6.6 Pressure recording

Modify `engine/src/executor.rs`: after each agent invocation, record a `strategy_pressure_signal` with the actual input payload size.

---

## 7. Verification

- `strategy.profiles` returns all profiles from `strategy-profiles.yaml`.
- `strategy.assign` persists the profile ID on the run and snapshots parameters.
- `strategy.assign` rejects unknown profile IDs.
- `strategy.get` returns current profile and latest pressure data.
- `strategy.pressure` returns correct pressure percentages and levels.
- `strategy.pressure` returns actionable recommendations when level is yellow or red.
- `strategy.handoff` filters artifacts by the profile's `promoted_artifact_policy`.
- `strategy.handoff` with `summary_only` mode returns a summary instead of full artifacts.
- `strategy.simulate` predicts red pressure at stages where actual runs historically exceeded budget.
- `strategy.simulate` works with both existing run IDs and hypothetical YAML paths.
- Pressure levels correctly classified: green < 60%, yellow 60-85%, red > 85%.

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Token estimation from byte size is inaccurate | Medium | Use 4 bytes per token as conservative default. Clearly label as estimates. Actual token counting would require tokenizer integration (future). |
| Strategy profile changes mid-run affect in-flight agents | Low | Profile change applies only to subsequent stages. In-flight agents use the snapshot from when their stage started. |
| `strategy.simulate` for hypothetical runs has no artifact size data | Medium | Fall back to default sizes. Log a warning in the response: "Using default artifact sizes; estimates improve with historical data." |
| Profile registry file missing at startup | Low | Fail-open: if `strategy-profiles.yaml` is absent, create a default in-memory `current_mixed_baseline` profile. Log a warning. |
| Pressure recording adds write overhead per agent invocation | Low | One INSERT per agent invocation is negligible. UNIQUE constraint prevents duplicates on retry. |
| Handoff packet compilation for `full_context` mode may be large | Medium | Response includes `total_handoff_bytes` and `within_budget` flag. Client can switch to `summary_only` if too large. |
