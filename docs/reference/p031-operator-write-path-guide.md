# Operator Write-Path Guide (P031)

This guide maps removed macOS UI write controls to their external CLI or MCP workflows. Per Proposal 031, the macOS UI is a read-only thin client.

## Summary

| Control | External Workflow | Status |
| --- | --- | --- |
| Create Idea | `cw-cli ideas create` | Validated |
| Start Run | `cw-cli runs start` | Validated |
| Stop Run | `cw-cli runs stop` | Validated |
| Approve / Reject | `approvals.resolve` (MCP) | Validated |
| Retry Stage | `cw-cli stages retry` | Validated |
| Steward Analysis | Temporarily Unavailable | Pending |
| Reset Session | Temporarily Unavailable | Pending |
| Resume Session | Temporarily Unavailable | Pending |
| Clone Run | Temporarily Unavailable | Pending |
| Compare Runs | Temporarily Unavailable | Pending |
| Launch Experiment | Temporarily Unavailable | Pending |
| Runtime Health | Temporarily Unavailable | Pending |
| Reset Agent | Temporarily Unavailable | Pending |

## Detail

### ideas.create
- **Label:** Create Idea
- **Kind:** CLI
- **Tool:** `cw-cli ideas create`
- **Required IDs:** `idea_id`
- **Parameters:** `--workflow <id> --catalog <id> --title <title>`
- **Expected Output:** `Idea ID: <id>`
- **Notes:** Use CLI to capture new ideas while UI write path is removed.

### runs.start
- **Label:** Start Run
- **Kind:** CLI
- **Tool:** `cw-cli runs start`
- **Required IDs:** `idea_id`
- **Parameters:** `--idea <id>`
- **Expected Output:** `Run ID: <id>`
- **Notes:** Execution advancing moves to CLI/automation.

### runs.cancel
- **Label:** Stop Run
- **Kind:** CLI
- **Tool:** `cw-cli runs stop`
- **Required IDs:** `run_id`
- **Parameters:** `--run <id>`
- **Expected Output:** `Run stopped.`
- **Notes:** Cooperative cancellation triggered via CLI.

### approvals.resolve
- **Label:** Approve / Reject
- **Kind:** MCP Terminal
- **Tool:** `approvals.resolve`
- **Required IDs:** `approval_id`, `run_id`, `stage_id`
- **Parameters:** `{ "approval_id": "<id>", "decision": "approve"|"reject" }`
- **Expected Output:** `Approval resolved.`
- **Notes:** Interactive approvals moved to MCP terminal.

### stages.retry
- **Label:** Retry Stage
- **Kind:** CLI
- **Tool:** `cw-cli stages retry`
- **Required IDs:** `run_id`, `stage_id`
- **Parameters:** `--stage <id>`
- **Expected Output:** `Stage retry queued.`
- **Notes:** Retry logic moved to CLI.

### steward.run_analysis
- **Label:** Steward Analysis
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-STEWARD`
- **Notes:** Analysis trigger removed from UI.

### session.reset
- **Label:** Reset Session
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-SESSIONS`
- **Notes:** Manual session reset removed.

### session.resume
- **Label:** Resume Session
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-SESSIONS`
- **Notes:** Manual session resume removed.

### runs.clone
- **Label:** Clone Run
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-RUN-CONTROL`
- **Notes:** Run cloning removed.

### runs.compare
- **Label:** Compare Runs
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-RUN-CONTROL`
- **Notes:** Comparison trigger removed.

### experiments.launch
- **Label:** Launch Experiment
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-EXPERIMENTS`
- **Notes:** Experiment launch removed.

### runtime.health
- **Label:** Runtime Health
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-HEALTH`
- **Notes:** In-app health diagnostics removed.

### agents.reset
- **Label:** Reset Agent
- **Status:** Temporarily Unavailable
- **Follow-up:** `P031-FOLLOWUP-AGENTS`
- **Notes:** Manual agent reset removed.
