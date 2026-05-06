# Governed Thin UI External Write-Path Guide

This guide is the stable operator reference for write controls that are not available from the governed macOS thin UI. The `p031` filename and schema identifiers are retained as gate aliases for existing automation.

The governed SwiftUI app reads workflow truth through GraphQL projections and may execute only the approval mutations allowed by [ui-action-boundary.md](ui-action-boundary.md). All other operational writes remain external, unavailable, or owned by later write-path proposals.

## Summary

| Control | External Workflow | Status |
| --- | --- | --- |
| Create Idea | Unavailable from governed UI | Pending follow-up |
| Start Run | Unavailable from governed UI | Pending follow-up |
| Cancel Run | Unavailable from governed UI | Pending follow-up |
| Retry Stage | `chainworks-control-plane: stages.retry` via MCP terminal workflow | Validated external workflow |
| Approve / Reject | `approveApproval` / `rejectApproval` in SwiftUI; `chainworks-control-plane: approvals.resolve` externally | Validated |
| Steward Analysis | Unavailable from governed UI | Pending follow-up |
| Reset Session | Unavailable from governed UI | Pending follow-up |
| Resume Session | Unavailable from governed UI | Pending follow-up |
| Clone Run | Unavailable from governed UI | Pending follow-up |
| Compare Runs | Unavailable from governed UI | Pending follow-up |
| Launch Experiment | Unavailable from governed UI | Pending follow-up |
| Runtime Health Action | Unavailable from governed UI | Pending follow-up |
| Reset Agent | Unavailable from governed UI | Pending follow-up |

## Validated External Workflows

### approvals.resolve

- **UI status:** Approval decisions are available in SwiftUI only through `approveApproval` and `rejectApproval`.
- **External workflow:** `chainworks-control-plane: approvals.resolve`
- **Required identifiers:** `approval_id`, `run_id`, `stage_id`
- **Minimum parameters:** `{"run_id":"<copied run_id>","stage_id":"<copied stage_id>","decision":"granted|rejected","comment":"<optional operator comment>"}`
- **Expected output:** `{"resolved":true,"journal_id":"<uuid>"}`
- **Rule:** Use identifiers copied from the read-only UI. Do not construct non-approval command payloads inside governed SwiftUI.

### stages.retry

- **UI status:** Retry is not available from governed SwiftUI.
- **External workflow:** `chainworks-control-plane: stages.retry`
- **Required identifiers:** `run_id`, `stage_id`
- **Minimum parameters:** `{"run_id":"<copied run_id>","stage_id":"<copied stage_id>","agent_execution_id":"<optional copied agent_execution_id>","consume_quota_budget_now":false}`
- **Expected output:** `{"scheduled":true,"journal_id":"<uuid>"}`
- **Rule:** Use identifiers copied from the read-only UI. Do not construct retry payloads inside governed SwiftUI.

## Pending Follow-Ups

| Removed control | Required identifiers | Follow-up | Current operator note |
| --- | --- | --- | --- |
| `ideas.create` | `idea_title` | `P031-FOLLOWUP-IDEA-WRITE-PATH` | Governed UI remains read-only until a non-P031 write workflow is restored. |
| `runs.start` | `idea_id` | `P031-FOLLOWUP-RUN-START-WRITE-PATH` | The UI may copy diagnostic identifiers but must not construct start-run commands. |
| `runs.cancel` | `run_id` | `P031-FOLLOWUP-RUN-CANCEL-WRITE-PATH` | The UI may show `run_id` for external resolution. |
| `steward.run_analysis` | `run_id` | `P031-FOLLOWUP-STEWARD-WRITE-PATH` | Read-only steward results may still be shown if exposed by GraphQL. |
| `session.reset` | `run_id` | `P031-FOLLOWUP-SESSION-RESET-WRITE-PATH` | Session lifecycle writes stay outside governed SwiftUI. |
| `session.resume` | `run_id` | `P031-FOLLOWUP-SESSION-RESUME-WRITE-PATH` | Session lifecycle writes stay outside governed SwiftUI. |
| `runs.clone` | `run_id` | `P031-FOLLOWUP-RUN-CLONE-WRITE-PATH` | Clone workflows require a separate approved write path. |
| `runs.compare` | `run_id` | `P031-FOLLOWUP-RUN-COMPARE-WRITE-PATH` | Read-only comparison rendering may remain when driven by GraphQL read models. |
| `experiments.launch` | `run_id` | `P031-FOLLOWUP-EXPERIMENT-WRITE-PATH` | Experiment writes require a separate approved workflow. |
| `runtime.health` | `run_id` | `P031-FOLLOWUP-RUNTIME-HEALTH-WRITE-PATH` | Daemon lifecycle readback remains GraphQL/read-only. |
| `agents.reset` | `run_id` | `P031-FOLLOWUP-AGENT-RESET-WRITE-PATH` | Agent reset writes remain outside governed SwiftUI. |

The machine-readable gate source for this guide is [p031-operator-write-path-guide.json](p031-operator-write-path-guide.json).
