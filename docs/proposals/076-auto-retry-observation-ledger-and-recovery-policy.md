# Proposal 076: Auto-Retry Observation Ledger and Recovery Policy

| Field | Value |
|---|---|
| Date | 2026-04-29 |
| Status | Draft |
| Author | Codex |
| Depends on | P045, P063, P065, P073, `chainworks-orchestrator-ops`, MCP `runs.list`, MCP `runs.get`, MCP `reports.get`, MCP `stages.retry` |
| Scope | Turn the recurring auto-retry job from free-form babysitting into a structured blocker-observation and recovery-policy subsystem. |
| Non-goal | No automatic human approval, no destructive run lifecycle action, and no proposal-specific implementation work inside the monitor job. |

## 1. Problem

The current hourly auto-retry job helps keep runs moving, but its output is optimized for a human reading one execution at a time, not for system improvement.

Current prompt:

```text
Запроси статус run'ов через [$chainworks-orchestrator-ops](/Users/user/.codex/skills/chainworks-orchestrator-ops/SKILL.md)
Если какие-то blocked, то собери информацию о причинах и складывай в markdown файл с известными проблемами.
Сделай retry всех blocked run'ов через [$chainworks-orchestrator-ops](/Users/user/.codex/skills/chainworks-orchestrator-ops/SKILL.md)
```

Observed source files:

- `/Users/user/.codex/automations/auto-retry/known-issues.md`
- `/Users/user/.codex/automations/auto-retry/memory.md`
- `.chainworks/known-blocked-run-problems.md`

These files show recurring signatures:

- `missing_required_outputs` / `no_output_produced` for proposal writer and proposal reviewers.
- `stage_stuck_running` repaired by startup repair and then retried by the monitor.
- top-level `run.status=blocked` while `run_state_projection.status=running`.
- failed retry by stage execution UUID followed by successful retry by workflow `stage_id`.
- repeated retry of the same stage without a durable signature cooldown or escalation path.

The monitor therefore creates useful raw evidence, but each later proposal/audit still has to rediscover and normalize the same facts by hand.

## 2. Goals

- Persist every auto-retry observation as one structured append-only event.
- Maintain a deduplicated known-issue catalog keyed by stable blocker signatures.
- Separate retry-safe stale infrastructure failures from substantive proposal/output-contract failures.
- Prevent shotgun retry loops on the same unresolved signature.
- Produce proposal-ready evidence without manually scraping multiple markdown files.
- Keep human approvals as real quality gates and never auto-approve them.
- Keep the monitor MCP-first and aligned with `chainworks-orchestrator-ops`.

## 3. Non-Goals

- Do not replace P045 recovery commands if they become the canonical MCP recovery surface.
- Do not implement broad automatic continuation/forking of blocked runs.
- Do not mutate run-owned worktrees or cancel/archive runs.
- Do not treat human approvals as babysitting.
- Do not move operational control into GraphQL or the SwiftUI app.

## 4. Current Evidence Summary

| Signature | Evidence | Current behavior | Problem |
|---|---|---|---|
| `missing_required_outputs:proposal_writer:proposal_current` | `.chainworks/known-blocked-run-problems.md`, write-budget investigation run `4e4f203a...` | Repeated `stages.retry` eventually unblocks or reblocks. | No same-session contract repair or provider fallback is guaranteed before durable block. |
| `missing_required_outputs:proposal_reviewer_*:proposal_review_v1` | `/Users/user/.codex/automations/auto-retry/known-issues.md` on 2026-04-28/29 | Stage retry clears top-level blocked state temporarily. | Reviewer output contract failure is treated like generic retryable blocked state. |
| `stage_stuck_running:startup_repair` | multiple entries for run `4e4f203a...` | Startup repair marks blocked for operator retry. | Repair action is correct, but monitor must classify it as infrastructure stale truth and track recurrence. |
| `projection_divergence:run_blocked_projection_running` | multiple entries for runs `4459e17c...`, `4e4f203a...`, `4c5dacfa...` | Retry or manual projection repair clears symptoms. | The issue is often projection/settlement lag, not a proposal code blocker. |
| `retry_identifier_shape:stage_execution_uuid_rejected` | `.chainworks/known-blocked-run-problems.md` | Retry by execution UUID fails; retry by workflow stage id succeeds. | Operator-facing recovery guidance should name valid retry identifiers. |

## 5. Proposed Design

### 5.1 Append-only Observation Ledger

Add a canonical local ledger:

```text
.chainworks/automation/auto-retry-observations.jsonl
```

Each automation execution appends one poll event:

```json
{
  "schema_version": "auto-retry-observation.v1",
  "observed_at": "2026-04-29T09:50:04+03:00",
  "daemon_ready": true,
  "source": "automation:auto-retry",
  "summary": {
    "active_total": 3,
    "blocked_before": 2,
    "blocked_after": 0,
    "running_after": 2,
    "waiting_approval_after": 1
  },
  "blocked_runs": [
    {
      "run_id": "4459e17c-c5f0-48ee-9821-97ee937ec3dd",
      "idea_or_proposal": "P041",
      "stage_id": "state_4_proposal_reviewed",
      "status_before": "blocked",
      "run_state_projection_status": "running",
      "blocker_signature_id": "missing_required_outputs:proposal_review_macos:proposal_review_v1",
      "blocker_class": "substantive_output_contract",
      "failure_class": "no_output_produced",
      "failure_summary": "proposal_review_macos: required output was not produced",
      "safe_retry": true,
      "retry_action": {
        "tool": "stages.retry",
        "stage_id": "state_4_proposal_reviewed",
        "journal_id": "4a9791ba-d006-47ba-b517-4480eae4812d"
      }
    }
  ]
}
```

The ledger is append-only. It is not a replacement for canonical DB truth; it is operational evidence for recurring blocker analysis.

### 5.2 Deduplicated Known-Issue Catalog

Add a compact generated/maintained catalog:

```text
.chainworks/automation/auto-retry-known-issues.md
```

The catalog is keyed by `blocker_signature_id`, not by automation run. Each entry records:

- first seen
- last seen
- count
- affected run ids
- blocker class
- last evidence report id
- last retry result
- proposed systemic owner
- linked proposal or follow-up id

The automation updates an existing signature instead of appending another full narrative block. A new long-form markdown note is allowed only for a new signature or a material classification change.

### 5.3 Blocker Classification

The monitor must classify every blocked run before retry:

| Class | Examples | Default action |
|---|---|---|
| `human_gate` | `pending_approvals > 0` | Do not retry; report approval. |
| `substantive_output_contract` | `missing_required_outputs`, `no_output_produced` | Retry only within budget; escalate after recurrence. |
| `stale_execution_truth` | `stage_stuck_running`, dead active invocation, startup repair marker | Safe retry or recovery repair. |
| `projection_divergence` | `run.status` disagrees with run-state projection | Refresh/rebuild projection or retry only if recovery says so. |
| `provider_or_session_failure` | ACP session closed, toolchain/cache home failure | Retry after provider/session reset when safe. |
| `unknown` | insufficient evidence | Do not shotgun; collect `runs.get` + `reports.get` packet first. |

### 5.4 Retry Policy and Cooldown

The monitor must not retry every blocked run blindly.

Rules:

- Retry only small batches when multiple runs share a new signature.
- Use workflow `stage_id` for `stages.retry` unless MCP explicitly accepts another identifier.
- Do not retry `human_gate`.
- Do not retry the same `blocker_signature_id` for the same run more than twice in a rolling 6 hour window unless the last retry produced measurable stage advancement.
- After the cooldown is exhausted, leave the run blocked and mark the signature `needs_systemic_fix`.
- Keep `consume_quota_budget_now=false` by default for infrastructure/stale truth retries; allow `true` only when the failure is clearly agent-output related and the operator policy permits spending budget.

### 5.5 Proposal-Ready Rollup

Add a small helper:

```text
scripts/chainworks/auto_retry_rollup.py
```

Inputs:

- `.chainworks/automation/auto-retry-observations.jsonl`
- `.chainworks/automation/auto-retry-known-issues.md`

Outputs:

- grouped issue table by `blocker_signature_id`
- recurrence counts
- representative evidence ids
- suggested owner proposal or follow-up
- stale signatures that have not recurred recently

This helper makes future proposals consume one normalized source instead of scraping automation memory, arbitrary markdown, and run-local reports.

## 6. Updated Automation Prompt

The recurring prompt should become:

```text
Use [$chainworks-orchestrator-ops](/Users/user/.codex/skills/chainworks-orchestrator-ops/SKILL.md).

Inspect active runs through MCP/control-plane readback. For every blocked run, collect runs.list, runs.get, and the latest relevant failed_stage_evidence/report packet before taking action.

Append exactly one structured JSON object for this poll to:
/Users/user/Documents/Chainworks Forge/.chainworks/automation/auto-retry-observations.jsonl

Use schema_version=auto-retry-observation.v1. Include observed_at, daemon_ready, active status counts before/after, and for each blocked run: run_id, proposal/idea label when known, stage_id, status_before, run_state_projection_status, drift_details_json when present, blocker_class, blocker_signature_id, failure_class, failure_summary, evidence_report_id, safe_retry, retry_action, retry_result, and next_systemic_action.

Maintain a deduplicated catalog at:
/Users/user/Documents/Chainworks Forge/.chainworks/automation/auto-retry-known-issues.md

Update an existing catalog entry by blocker_signature_id instead of appending another full narrative block. Add a new entry only for a new signature or a material classification change.

Retry only safe blocked runs through MCP stages.retry. Do not retry human approvals. Do not retry the same run/signature more than twice in a rolling 6 hour window unless the last retry produced stage advancement. Prefer workflow stage_id over stage_execution_id for stages.retry. If several runs share a new systemic signature, retry one or a small batch first, then observe.

Update /Users/user/.codex/automations/auto-retry/memory.md with only the latest compact poll summary and paths to the ledger/catalog.
```

## 7. Implementation Plan

1. Create `.chainworks/automation/` and ensure `.gitignore` excludes generated ledger/catalog files unless the operator intentionally snapshots them.
2. Add `scripts/chainworks/auto_retry_rollup.py`.
3. Teach the automation prompt to write structured ledger events and a deduplicated catalog.
4. Add a lightweight validator for `auto-retry-observation.v1` records.
5. Add a proposal gate or script check that validates sample records and rollup output.
6. Route repeated signatures to existing proposals when applicable:
   - output contract validation, same-session repair, and provider fallback: P079 lane
   - recovery/retry MCP ergonomics: P045/P065 lane
   - projection/settlement divergence: P073 / UI action boundary follow-up lane
   - native release diagnostics: write-budget lane

## 8. Acceptance Criteria

- The auto-retry automation no longer appends repeated free-form issue narratives for the same signature.
- Every auto-retry execution produces exactly one parseable `auto-retry-observation.v1` JSONL record.
- Known issues are deduplicated by `blocker_signature_id`.
- Repeated `missing_required_outputs` and `stage_stuck_running` events appear as recurring signatures with counts, not as unrelated incidents.
- The monitor never retries human approvals.
- The monitor respects retry cooldown for a repeated run/signature pair.
- Rollup output can produce a proposal-ready issue table without reading `/Users/user/.codex/automations/auto-retry/known-issues.md`.
- `stages.retry` guidance consistently uses workflow `stage_id` unless a future MCP contract explicitly supports execution ids.

## 9. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Generated `.chainworks` operational logs grow without bound | Keep JSONL local/generated, add retention guidance, and roll up by signature. |
| The monitor hides real implementation blockers by retrying too aggressively | Require classification, cooldown, and `needs_systemic_fix` escalation. |
| The catalog becomes another hand-maintained markdown sink | Treat JSONL as the primary source and catalog as deduplicated generated/maintained summary. |
| This overlaps existing recovery proposals | Make P076 the observation/policy layer and route implementation fixes to P017/P045/P063/P065/P073 and the implemented write-budget contract where they already own the runtime surface. |

## 10. Open Questions

- Should `.chainworks/automation/auto-retry-known-issues.md` be generated only from JSONL, or may the automation edit it directly?
- Should old `/Users/user/.codex/automations/auto-retry/known-issues.md` be archived after the first successful structured rollup?
- Should retry cooldown state live only in the JSONL ledger, or should the control plane expose a first-class retry budget/readback surface?
