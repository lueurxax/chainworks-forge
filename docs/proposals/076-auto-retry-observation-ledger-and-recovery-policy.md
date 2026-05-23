# Proposal 076: Auto-Retry Observation Ledger and Recovery Policy

> Source: current unfinished-run proposal artifact.

## Metadata

- **Source run:** `260ec20f-549a-487c-9bbe-698166801218`
- **Source artifact:** `.chainworks/runs/260ec20f-549a-487c-9bbe-698166801218/proposals/approved/proposal.md`
- **Source md5:** `0e84834191158dae8048fa5e9badd65d`
- **Proposal Id:** proposal-076
- **Proposal Revision Id:** p076-2026-05-23-r14-contract-refresh
- **Schema Version:** proposal_document_v1
- **Status:** Draft ready for rereview
- **Date:** 2026-05-23
- **Owner:** Control-plane automation / chainworks-orchestrator-ops
- **Source Review Pass Id:** 9a635252-b59e-45a2-92c0-b7a3805973fe

## Problem

The hourly auto-retry automation produces useful human evidence, but it is not precise enough to become a reliable improvement loop. It inspects runs, writes free-form notes, and can retry blocked stages without durable normalized observations, stable blocker signatures, complete schemas, explicit readback envelopes, durable budget settlement semantics, or a concrete idempotent MCP side-effect contract. The result is that substantive output-contract failures, stale execution truth, projection divergence, provider/session failures, retry identifier mistakes, and human approval waits are too easy to conflate.

## Goals

- Persist every completed monitor poll as one validated append-only observation record in the resolved automation ledger.
- Maintain machine-readable deduplicated known-issue state keyed by stable blocker_signature_id values and generate markdown only as a human view.
- Classify human gates, substantive output-contract failures, stale execution truth, projection divergence, provider/session failures, retry identifier shape issues, and unknown evidence gaps before recommending action.
- Keep P076 side-effect free: the monitor records observations, budget state, cooldowns, recommendations, and readback, but does not issue side-effecting retry or recovery MCP calls until a later proposal defines a concrete idempotent command contract.
- Prevent shotgun retry loops by making retry recommendation, cooldown, orphaned planned-attempt settlement, lock behavior, backpressure, and budget-unavailable states explicit and fixture-proven.
- Expose proposal-ready evidence through deterministic validation, rollup tooling, and MCP readback rather than manual markdown scraping.
- Keep human approvals as real quality gates. The monitor must never auto-approve, synthesize, retry, or bypass approval waits.
- Keep the monitor MCP-first and aligned with chainworks-orchestrator-ops while preserving the SwiftUI app as a passive operator shell.

## Non Goals

- Do not issue side-effecting retry, recovery, continue, fork, cancel, archive, or approval commands in P076.
- Do not bind P076 to an implicit or underspecified MCP retry command. Side-effecting retry enablement requires a later reviewed proposal with request, response, idempotency, authorization, error, timeout, duplicate, and settlement fixtures.
- Do not replace canonical recovery commands if they become the official control-plane recovery API.
- Do not mutate run-owned worktrees or artifact contents.
- Do not auto-approve, synthesize, or bypass human approval gates.
- Do not add GraphQL schema, GraphQL subscription, SwiftData entity, SQLite migration, workflow YAML field, agent catalog YAML field, or artifact path-map schema changes in this slice.
- Do not make the JSONL ledger canonical run state. It is operational evidence derived from MCP/control-plane truth.

## Ux Ui Notes

### Budget Status Rendering
- **Available**
  - **Component:** small_state_badge
  - **Tone:** success
- **Budget Unavailable**
  - **Component:** small_state_badge
  - **Tone:** error
- **Cooldown**
  - **Component:** small_state_badge
  - **Tone:** warning
- **Disabled Pending Idempotency Contract**
  - **Component:** small_state_badge
  - **Tone:** info
- **Needs Human Triage**
  - **Component:** small_state_badge
  - **Tone:** warning
- **Needs Systemic Fix**
  - **Component:** small_state_badge
  - **Tone:** warning
### Diagnostic Row Anatomy
- **Accessibility:** VoiceOver labels include severity, short reason, and whether details are expanded. Copy buttons have explicit labels.
- **Collapsed:** Severity icon, short reason, run_id when scoped, and disclosure chevron in one compact row.
- **Copy Affordance:** blocker_signature_id, observation_id, diagnostic code, and path text are selectable and expose context-menu Copy actions on macOS.
- **Expanded:** Shows code, message, path, blocker_signature_id, observation_id, and next_systemic_action when present. Expansion is row-scoped, not global.
- **Icons:** Severity icon leads the row; warning/error icons use existing system semantic colors. Informational rows use a neutral info glyph.
### Field Grouping
- **Diagnostic**
  - auto_retry_budget_unavailable_reason
  - auto_retry_backpressure_skip_count
  - diagnostics
  - auto_retry_readback_version
- **Evidence**
  - auto_retry_observation_path
  - auto_retry_rollup_report_path
  - ledger_path
  - budget_state_path
  - known_issue_catalog_path
  - generated_markdown_catalog_path
  - lock_path
  - rollup_report_path
- **Lifecycle**
  - auto_retry_observation_record_id
  - auto_retry_retry_budget_state
  - auto_retry_last_retry_result
  - auto_retry_known_issue_status
  - oldest_planned_attempt_at
  - planned_attempt_age_seconds
  - unknown_attempt_count
  - required_operator_settlement
- **Primary**
  - auto_retry_policy_status
  - auto_retry_policy_decision
  - auto_retry_blocker_class
  - auto_retry_blocker_signature_id
### Policy Decision Rendering
- **Budget Unavailable**
  - **Component:** compact_error_row
  - **Tone:** error
- **Collect Evidence**
  - **Component:** diagnostic_status_row
  - **Tone:** info
- **Cooldown Exhausted**
  - **Component:** compact_warning_row
  - **Tone:** warning
- **Human Gate**
  - **Component:** approval_work_row
  - **Tone:** attention
- **Needs Human Triage**
  - **Component:** triage_required_row
  - **Tone:** warning
- **Needs Systemic Fix**
  - **Component:** owner_routing_row
  - **Tone:** warning
- **Observe Only**
  - **Component:** neutral_status_row
  - **Tone:** secondary
- **Poll Timeout**
  - **Component:** compact_error_row
  - **Tone:** error
- **Retry Disabled Pending Idempotency Contract**
  - **Component:** contract_blocked_row
  - **Tone:** info
- **Skipped Backpressure**
  - **Component:** compact_diagnostic_row
  - **Tone:** secondary
- **Skipped Lock Held**
  - **Component:** compact_diagnostic_row
  - **Tone:** secondary
### State Presentation
- no_observation_history is hidden from ordinary run detail unless diagnostics are expanded.
- readback_degraded and budget_unavailable render as compact diagnostic rows.
- Human approval waits are presented as approval work, not failures. Any summary line for a human gate states that no retry was attempted.
- Long blocker_signature_id and next_systemic_action values truncate in primary rows with disclosure in the expanded diagnostic row.
### Surface Scope
- No new primary SwiftUI screen is introduced by P076.
- SwiftUI/AppKit views do not set up MCP clients, parse JSONL, resolve automation paths, interpret retry policy, or persist P076 state. Any MCP-sourced app data reaches views through AutoRetryReadbackRepository or an equivalent app-owned read-model adapter.
- AutoRetryReadbackRepository decodes readback off the MainActor into compact Sendable/Codable snapshots. View models assign snapshots on MainActor only after verifying run_id plus observation_id freshness, falling back to generated_at when no observation_id exists. In-flight refreshes cancel on run selection change, view disappearance, scene/window teardown, and window close. Background diagnostic refreshes run at background priority and never block primary run selection rendering.

## Architecture

### Backpressure And Fairness
- **Human Gate Starvation Rule:** Already-reported human gates must not repeatedly consume all consideration capacity. Previously skipped non-human retry-safe or evidence-collection work is prioritized ahead of already-reported human gates, and at least ten consideration slots are reserved for non-human blockers when that many exist.
- **Max Retry Actions Per Poll:** `0`
- **Max Retry Actions Per Signature Per Poll:** `0`
- **Max Runs Considered Per Poll:** `25`
- **P076 Reason:** Side-effecting retry is disabled in this proposal.
- **Selection Order**
  - previously skipped non-human evidence or recovery work
  - new human_gate observations with no retries
  - oldest blocked run by observed_at
  - highest recurrence signatures within observe-only budget state
  - new signatures after representative evidence capture
- **Skipped Work Rule:** Runs skipped because of max_runs_considered_per_poll are durably recorded with skipped_backpressure reason buckets and prioritized in the following poll.
### Known Issue Catalog
- **Atomicity:** Append ledger first, then update catalog JSON by temp-file fsync, atomic rename, and parent-directory fsync. If divergence is detected, reconstruct catalog from ledger plus budget state.
- **Canonical Json Path Alias:** .chainworks/automation/auto-retry-known-issues.json
- **Generated Markdown Path Alias:** .chainworks/automation/auto-retry-known-issues.md
- **Rule:** JSON is the canonical dedupe state. Markdown is generated from JSON only and must never be parsed as authority.
- **Schema Version:** auto-retry-known-issues.v1
### Mcp Readback Contract
- **Degraded Artifact Behavior:** Artifact-read degradation is a successful readback result, not a transport error. It returns schema_version, generated_at, all resolved top-level path fields, diagnostics, and empty observations/latest_by_run when data cannot be trusted.
- **Null History Behavior:** Runs with no observation history return auto_retry_policy_status=no_observation_history and null latest-observation fields.
- **Owning Surface:** chainworks-orchestrator-ops tool automation.auto_retry.latest owns P076 readback.
- **Unsupported Version Behavior:** Unsupported versions are JSON-RPC application errors with code -32076, message unsupported_version, and error.data containing code, supported_versions, unsupported_versions, and requested_versions. They return no partial success payload.
### No Change Compatibility
- No GraphQL schema, resolver, subscription, or persisted projection change is introduced. Future GraphQL exposure must derive from the same versioned MCP readback object or use a separately reviewed projection contract.
- No SwiftData entity, app persistence field, or SQLite migration is introduced. Future persisted readback requires migration, backfill, null-readback compatibility tests, and a new rollout contract.
- No workflow YAML, agent catalog YAML, or artifact path-map schema change is introduced. Implementers must not add ad hoc YAML retry policy knobs for this slice.
### Observation Ledger
- **Lock Scope:** The writer lock is held from initial automation artifact read through ledger append, budget/catalog update, generated markdown update, and final fsync completion. Cancellation is deferred inside this critical section after mutation begins.
- **Partial Read Tolerance:** Readers ignore one missing or partial trailing JSONL line and emit a partial_trailing_record diagnostic.
- **Path Alias:** .chainworks/automation/auto-retry-observations.jsonl
- **Record Identity:** observation_id format is ar_obs_<UTC basic timestamp>_<12 lowercase hex chars>. canonical_record_hash is sha256 over canonical JSON excluding canonical_record_hash.
- **Schema Version:** auto-retry-observation.v1
- **Write Invariant:** A completed poll appends exactly one locally validated newline-terminated JSON object. The append is cancellation-shielded once validation succeeds: write all bytes including the trailing newline, fsync the file descriptor, and only then release the writer lock. If append fails after partial bytes, readers ignore at most one trailing partial line and emit partial_trailing_record.
### Path Resolution
- **Automation Dir Alias:** .chainworks/automation
- **Default Meta Root:** /Users/user/Documents/Chainworks Forge/.chainworks
- **Readback Requirement:** automation.auto_retry.latest must echo resolved absolute ledger_path, budget_state_path, known_issue_catalog_path, generated_markdown_catalog_path, lock_path, and rollup_report_path. These six path fields are top-level required fields in auto_retry_readback.v1.
- **Rule:** Automation paths resolve under CHAINWORKS_META_ROOT when set; otherwise they resolve under<workspace_root>/.chainworks. Relative proposal paths such as .chainworks/automation/auto-retry-observations.jsonl are display aliases only.
- **Workspace Root:** /Users/user/Documents/Chainworks Forge
### Retry Budget Store
- **Authority:** The budget file is durable policy/readback state for per-run/per-signature recommendation and cooldown accounting. JSONL is audit evidence, not the budget source.
- **Durable Write Semantics:** Any claimed durable budget reservation, catalog update, generated markdown update, or override update must write a temp file in the target directory, fsync the temp file, atomic rename over the destination, and fsync the parent directory before reporting success.
- **Fail Closed Rule:** If the budget file is missing, unreadable, malformed, locked by another live writer, fails validation, cannot be durably renamed, or cannot be reconciled with the ledger, the monitor records budget_unavailable or observe_only and must not issue side effects.
- **Orphaned Planned Attempt Escalation:** A planned or unknown attempt older than two poll_deadline_seconds or observed in three consecutive polls without reconciliation moves the run/signature to needs_human_triage. Readback exposes oldest_planned_attempt_at, planned_attempt_age_seconds, unknown_attempt_count, and required_operator_settlement. Settlement must be audited as consumed, reconciled, or reopened before recommendation state can leave needs_human_triage.
- **P076 Side Effect Rule:** Because P076 is observe-only, new side-effecting attempt rows must not be created by this proposal. Existing or fixture planned/unknown rows are read, surfaced, and settled according to the schema so stale policy state cannot become invisible.
- **Path Alias:** .chainworks/automation/auto-retry-budget.json
- **Schema Version:** auto-retry-budget.v1
- **Scope Boundary:** P076 is an observe-only ledger, catalog, budget-readback, schema, and rollout-gate proposal. It can recommend that a retry would be safe under future policy, but it must not dispatch retry or recovery side effects.
### Single Writer And Deadlines
- **Deadline Behavior:** A deadline produces one partial observation with policy_decision=poll_timeout for unfinished items. No retry or recovery side effect is attempted in P076.
- **Evidence Fetch Timeout Seconds:** `30`
- **Future Retry Timeout Settlement:** Fixtures must cover a future retry dispatch timeout after budget reservation: attempt_result=unknown, ambiguity recorded in the observation, and a duplicate retry suppressed until reconciliation.
- **Lock Identity Fields**
  - hostname
  - boot_id_or_session_id
  - pid
  - process_start_time
  - command_identity
  - lock_token
  - created_at
  - expires_after_seconds
- **Lock Path Alias:** .chainworks/automation/auto-retry.lock
- **Lock Rule:** Every poll acquires an exclusive lock before reading or writing automation artifacts. If the lock is held by a live writer, the poll exits or records skipped_lock_held with no side effects.
- **Mcp Timeout Seconds:** `30`
- **Poll Deadline Seconds:** `300`
- **Retry Call Timeout Seconds:** `45`
- **Stale Lock Rule:** A lock may be considered stale only when older than two poll_deadline_seconds and the holder identity is not live. Liveness checks compare hostname, boot/session identity, PID, process start time, command identity, and token to defend against PID reuse. Recovery uses atomic compare-and-replace on the lock token; token verification failure records observe_only and exits.

## Retry Policy

- Classify before acting.
- Never retry human_gate.
- Never retry unknown until the minimal evidence packet exists: runs.list, runs.get, and the latest relevant failed-stage report.
- Never retry when budget state is unavailable, invalid, unreconciled, locked, or not durably writable.
- P076 never dispatches side-effecting retry or recovery calls. It records recommend_retry only as an observation/recommendation decision.
- A later proposal may enable retry only after naming a concrete MCP tool with run_id, workflow stage_id, idempotency_key, request hash, accepted/rejected/ambiguous outcomes, duplicate no-op semantics, error envelope, authorization, and fixtures.
- A future retry-call timeout after possible dispatch is ambiguous, not skipped. The reserved attempt remains consumed or unknown until reconciliation and suppresses duplicate retry for the same run/signature.
- Treat ambiguous side-effect lanes conservatively. If readback indicates an external write may be ambiguous, observe and route to reconciliation rather than retrying.

## Normative Schemas

### Auto Retry Budget V1
- **Additionalproperties:** `false`
- **Attempt V1**
  - **Additionalproperties:** `false`
  - **Fields**
    - **Attempt Id:** string
    - **Attempt Result:** retry_result
    - **Created At:** rfc3339
    - **External Retry Enabled:** boolean; must be false for P076-created rows
    - **Idempotency Key:** idempotency_key\|null
    - **Mcp Tool:** string\|null
    - **Request Hash:** sha256\|null
    - **Settlement Reason:** string\|null
    - **Status:** retry_lifecycle
    - **Updated At:** rfc3339
  - **Optional Nullable**
    - idempotency_key
    - mcp_tool
    - request_hash
    - settlement_reason
  - **Required**
    - attempt_id
    - status
    - created_at
    - updated_at
    - attempt_result
    - external_retry_enabled
- **Budget Row V1**
  - **Additionalproperties:** `false`
  - **Fields**
    - **Attempts:** attempt_v1[]
    - **Blocker Signature Id:** string
    - **Cooldown Until:** rfc3339\|null
    - **Last Observation Id:** observation_id\|null
    - **Last Policy Decision:** policy_decision
    - **Max Attempts:** non_negative_integer
    - **Oldest Planned Attempt At:** rfc3339\|null
    - **Planned Attempt Age Seconds:** non_negative_integer\|null
    - **Required Operator Settlement:** string\|null
    - **Run Id:** string
    - **Status:** budget_status
    - **Unknown Attempt Count:** non_negative_integer\|null
    - **Updated At:** rfc3339
    - **Window Hours:** positive_integer
  - **Optional Nullable**
    - oldest_planned_attempt_at
    - planned_attempt_age_seconds
    - unknown_attempt_count
    - required_operator_settlement
  - **Required**
    - run_id
    - blocker_signature_id
    - window_hours
    - max_attempts
    - attempts
    - last_policy_decision
    - cooldown_until
    - status
    - last_observation_id
    - updated_at
- **Fields**
  - **Diagnostics:** common_diagnostic_v1[]
  - **Generated At:** rfc3339
  - **Path Resolution:** object {workspace_root:absolute_path, meta_root:absolute_path, budget_state_path:absolute_path}
  - **Rows:** budget_row_v1[]
  - **Schema Version:** const auto-retry-budget.v1
- **Required**
  - schema_version
  - generated_at
  - path_resolution
  - rows
  - diagnostics
- **Type:** json
### Auto Retry Known Issues V1
- **Additionalproperties:** `false`
- **Fields**
  - **Generated At:** rfc3339
  - **Issues:** known_issue_v1[]
  - **Path Resolution:** object {workspace_root:absolute_path, meta_root:absolute_path, known_issue_catalog_path:absolute_path, generated_markdown_catalog_path:absolute_path}
  - **Schema Version:** const auto-retry-known-issues.v1
- **Known Issue V1**
  - **Additionalproperties:** `false`
  - **Fields**
    - **Affected Run Ids:** string[]
    - **Blocker Class:** blocker_class
    - **Blocker Signature Id:** string
    - **Current Status:** known_issue_status
    - **First Seen At:** rfc3339
    - **Last Evidence Report Id:** string\|null
    - **Last Observation Id:** observation_id
    - **Last Policy Decision:** policy_decision
    - **Last Retry Result:** retry_result
    - **Last Seen At:** rfc3339
    - **Last Stage Id:** string
    - **Observation Count:** positive_integer
    - **Proposed Owner Lane:** string
  - **Optional Nullable**
    - last_evidence_report_id
  - **Required**
    - blocker_signature_id
    - blocker_class
    - first_seen_at
    - last_seen_at
    - observation_count
    - affected_run_ids
    - last_stage_id
    - last_observation_id
    - last_policy_decision
    - last_retry_result
    - current_status
    - proposed_owner_lane
- **Required**
  - schema_version
  - generated_at
  - path_resolution
  - issues
- **Type:** json
### Auto Retry Observation V1
- **Additionalproperties:** `false`
- **Blocked Run V1**
  - **Additionalproperties:** `false`
  - **Fields**
    - **Blocker Class:** blocker_class
    - **Blocker Signature Id:** string
    - **Drift Details Json:** object\|null
    - **Evidence Report Id:** string\|null
    - **Failure Class:** string
    - **Failure Summary:** string
    - **Idea Or Proposal:** string\|null
    - **Next Systemic Action:** string
    - **Policy Decision:** policy_decision
    - **Retry Action:** retry_action; must be none or recommend_retry in P076
    - **Retry Budget:** budget_ref_v1
    - **Retry Lifecycle:** retry_lifecycle
    - **Retry Result:** retry_result; must be not_attempted or not_allowed for new P076 observations
    - **Run Id:** string
    - **Run State Projection Status:** run_state_projection_status
    - **Safe Retry:** boolean
    - **Stage Execution Id:** string\|null
    - **Stage Id:** string
    - **Status Before:** string
  - **Optional Nullable**
    - idea_or_proposal
    - stage_execution_id
    - drift_details_json
    - evidence_report_id
  - **Required**
    - run_id
    - stage_id
    - status_before
    - run_state_projection_status
    - blocker_class
    - blocker_signature_id
    - failure_class
    - failure_summary
    - safe_retry
    - retry_budget
    - retry_lifecycle
    - retry_action
    - retry_result
    - policy_decision
    - next_systemic_action
- **Fields**
  - **Artifact Paths:** object of resolved absolute paths
  - **Blocked Runs:** array of blocked_run_v1
  - **Canonical Record Hash:** sha256
  - **Daemon Ready:** boolean
  - **Diagnostics:** common_diagnostic_v1[]
  - **Observation Id:** observation_id
  - **Observed At:** rfc3339
  - **Policy Version:** string
  - **Schema Version:** const auto-retry-observation.v1
  - **Source:** object {tool:string, version:string, workspace_root:absolute_path, meta_root:absolute_path}
  - **Summary:** object with non_negative_integer counts active_total_before, blocked_before, running_before, waiting_approval_before, blocked_after, running_after, waiting_approval_after, retried_count, observe_only_count, cooldown_exhausted_count, budget_unavailable_count, skipped_backpressure_count, partial, poll_deadline_seconds, poll_elapsed_ms
  - **Writer Lock:** object {lock_path:absolute_path, acquired:boolean, token:string\|null, skipped_reason:string\|null, hostname:string\|null, boot_id_or_session_id:string\|null, pid:integer\|null, process_start_time:rfc3339\|null, command_identity:string\|null}
- **Optional**
  - diagnostics
  - artifact_paths
- **Required**
  - schema_version
  - observation_id
  - canonical_record_hash
  - observed_at
  - source
  - daemon_ready
  - policy_version
  - writer_lock
  - summary
  - blocked_runs
- **Type:** jsonl_record
### Auto Retry Readback Request V1
- **Additionalproperties:** `false`
- **Fields**
  - **Blocker Signature Id:** string\|null
  - **Client Supported Versions:** string[] default [auto_retry_readback.v1]
  - **Limit:** positive_integer default20 max100
  - **Run Id:** string\|null
  - **Schema Version:** const auto_retry_readback_request.v1
- **Optional Nullable**
  - run_id
  - blocker_signature_id
  - limit
- **Required**
  - schema_version
  - client_supported_versions
- **Type:** json
### Auto Retry Readback V1
- **Additionalproperties:** `false`
- **Fields**
  - **Budget State Path:** absolute_path
  - **Diagnostics:** common_diagnostic_v1[]
  - **Generated At:** rfc3339
  - **Generated Markdown Catalog Path:** absolute_path
  - **Known Issue Catalog Path:** absolute_path
  - **Latest By Run:** run_summary_v1[]
  - **Ledger Path:** absolute_path
  - **Lock Path:** absolute_path
  - **Observations:** observation_summary_v1[]
  - **Rollup Report Path:** absolute_path
  - **Schema Version:** const auto_retry_readback.v1
  - **Version Negotiation:** object {selected_version:string, supported_versions:string[], unsupported_versions:string[]}
- **Required**
  - schema_version
  - generated_at
  - version_negotiation
  - ledger_path
  - budget_state_path
  - known_issue_catalog_path
  - generated_markdown_catalog_path
  - lock_path
  - rollup_report_path
  - diagnostics
  - observations
  - latest_by_run
- **Run Summary V1**
  - **Additionalproperties:** `false`
  - **Fields**
    - **Auto Retry Backpressure Skip Count:** non_negative_integer
    - **Auto Retry Blocker Class:** blocker_class\|null
    - **Auto Retry Blocker Signature Id:** string\|null
    - **Auto Retry Budget Unavailable Reason:** string\|null
    - **Auto Retry Human Gate Retry Attempt Total:** non_negative_integer
    - **Auto Retry Known Issue Status:** known_issue_status\|null
    - **Auto Retry Last Retry Result:** retry_result\|null
    - **Auto Retry Next Systemic Action:** string\|null
    - **Auto Retry Observation Path:** absolute_path\|null
    - **Auto Retry Observation Record Id:** observation_id\|null
    - **Auto Retry Policy Decision:** policy_decision\|null
    - **Auto Retry Policy Status:** readback_policy_status
    - **Auto Retry Readback Version:** string
    - **Auto Retry Retry Budget State:** budget_status\|null
    - **Auto Retry Rollup Report Path:** absolute_path\|null
    - **Oldest Planned Attempt At:** rfc3339\|null
    - **Planned Attempt Age Seconds:** non_negative_integer\|null
    - **Required Operator Settlement:** string\|null
    - **Run Id:** string
    - **Unknown Attempt Count:** non_negative_integer\|null
  - **Required**
    - run_id
    - auto_retry_policy_status
    - auto_retry_policy_decision
    - auto_retry_observation_record_id
    - auto_retry_observation_path
    - auto_retry_blocker_signature_id
    - auto_retry_blocker_class
    - auto_retry_retry_budget_state
    - auto_retry_last_retry_result
    - auto_retry_known_issue_status
    - auto_retry_next_systemic_action
    - auto_retry_rollup_report_path
    - auto_retry_human_gate_retry_attempt_total
    - auto_retry_budget_unavailable_reason
    - auto_retry_backpressure_skip_count
    - auto_retry_readback_version
    - oldest_planned_attempt_at
    - planned_attempt_age_seconds
    - unknown_attempt_count
    - required_operator_settlement
- **Type:** json
### Budget Ref V1
- **Additionalproperties:** `false`
- **Fields**
  - **Attempt Count:** non_negative_integer
  - **Blocker Signature Id:** string
  - **Budget State Path:** absolute_path
  - **Budget Unavailable Reason:** string\|null
  - **Cooldown Until:** rfc3339\|null
  - **Last Observation Id:** observation_id\|null
  - **Max Attempts:** non_negative_integer
  - **Oldest Planned Attempt At:** rfc3339\|null
  - **Planned Attempt Age Seconds:** non_negative_integer\|null
  - **Remaining Attempts:** non_negative_integer
  - **Required Operator Settlement:** string\|null
  - **Run Id:** string
  - **Status:** budget_status
  - **Unknown Attempt Count:** non_negative_integer\|null
  - **Window Hours:** positive_integer
- **Optional Nullable**
  - last_observation_id
  - oldest_planned_attempt_at
  - planned_attempt_age_seconds
  - unknown_attempt_count
  - required_operator_settlement
  - budget_unavailable_reason
- **Required**
  - run_id
  - blocker_signature_id
  - status
  - window_hours
  - max_attempts
  - attempt_count
  - remaining_attempts
  - cooldown_until
  - budget_state_path
- **Type:** object
### Common Diagnostic V1
- **Additionalproperties:** `false`
- **Fields**
  - **Blocker Signature Id:** string\|null
  - **Code:** string
  - **Message:** string
  - **Observation Id:** observation_id\|null
  - **Path:** absolute_path\|null
  - **Run Id:** string\|null
  - **Severity:** diagnostic_severity
- **Optional Nullable**
  - path
  - run_id
  - blocker_signature_id
  - observation_id
- **Required**
  - code
  - severity
  - message
- **Type:** object
### Enum Domains
- **Blocker Class**
  - human_gate
  - substantive_output_contract
  - stale_execution_truth
  - projection_divergence
  - provider_or_session_failure
  - retry_identifier_shape
  - unknown
- **Budget Status**
  - available
  - cooldown
  - budget_unavailable
  - needs_human_triage
  - needs_systemic_fix
  - disabled_pending_idempotency_contract
- **Diagnostic Severity**
  - info
  - warning
  - error
- **Known Issue Status**
  - observed
  - retrying_within_budget
  - cooldown_exhausted
  - needs_systemic_fix
  - needs_human_triage
  - resolved_or_quiet
  - archived
- **Policy Decision**
  - observe_only
  - collect_evidence
  - human_gate
  - cooldown_exhausted
  - budget_unavailable
  - needs_systemic_fix
  - needs_human_triage
  - retry_disabled_pending_idempotency_contract
  - poll_timeout
  - skipped_lock_held
  - skipped_backpressure
- **Readback Policy Status**
  - no_observation_history
  - observed
  - readback_degraded
  - budget_unavailable
  - cooldown_exhausted
  - needs_human_triage
  - needs_systemic_fix
  - retry_disabled_pending_idempotency_contract
- **Retry Action**
  - none
  - recommend_retry
- **Retry Lifecycle**
  - not_applicable
  - planned
  - issued
  - accepted
  - rejected
  - advanced
  - reblocked
  - failed
  - unknown
  - settled_consumed
  - settled_reopened
- **Retry Result**
  - not_attempted
  - not_allowed
  - unknown
  - accepted
  - rejected
  - advanced
  - reblocked
  - failed
  - timeout_ambiguous
- **Run State Projection Status**
  - unknown
  - consistent
  - blocked
  - running
  - waiting_approval
  - divergent
### Observation Summary V1
- **Additionalproperties:** `false`
- **Fields**
  - **Blocker Class:** blocker_class
  - **Blocker Signature Id:** string
  - **Evidence Report Id:** string\|null
  - **Failure Summary:** string\|null
  - **Known Issue Status:** known_issue_status
  - **Next Systemic Action:** string\|null
  - **Observation Id:** observation_id
  - **Observation Path:** absolute_path
  - **Observed At:** rfc3339
  - **Policy Decision:** policy_decision
  - **Retry Action:** retry_action
  - **Retry Result:** retry_result
  - **Run Id:** string
  - **Stage Execution Id:** string\|null
  - **Stage Id:** string
- **Optional Nullable**
  - stage_execution_id
  - failure_summary
  - next_systemic_action
  - evidence_report_id
- **Required**
  - observation_id
  - observed_at
  - run_id
  - stage_id
  - blocker_signature_id
  - blocker_class
  - policy_decision
  - retry_action
  - retry_result
  - known_issue_status
  - observation_path
- **Type:** object
### Scalar Types
- **Absolute Path:** Absolute filesystem path string after CHAINWORKS_META_ROOT or workspace-root resolution.
- **Idempotency Key:** Opaque non-empty string reserved for future side-effect proposals; P076 does not send it to MCP.
- **Non Negative Integer:** Integer greater than or equal to zero.
- **Observation Id:** ar_obs_<UTC basic timestamp>_<12 lowercase hex chars>.
- **Positive Integer:** Integer greater than zero.
- **Rfc3339:** UTC or offset timestamp string parseable as RFC3339.
- **Sha256:** String beginning sha256: followed by lowercase hex digest.
### Unknown Field Behavior
- **Permissive Report Mode:** Ignore unknown additive fields, preserve diagnostics about ignored fields, and continue when required fields and known enum values are valid.
- **Strict Gate Mode:** Reject missing required fields, invalid scalar types, invalid enum values, invalid nullability, invalid closed object shapes, and unknown object fields where additionalProperties is false.

## Rollout Contract V1

- **Applicability:** required
### Commands
- **Allowlist**
  - ./scripts/test-gate.sh proposal-076
  - ./scripts/test-gate.sh p076
- **Commentary:** Gate commands are declarative expectations. The linter does not execute them.
### Decision Vocabulary
- pass
- fail
- waived
- not_applicable
- timeout
- release
- hold
- waive
### Gate Aliases
- proposal-076
- p076
### Hold Conditions
- Observation validator fails against required fixtures
- budget_ref_v1 or observation_summary_v1 schema is missing or incomplete
- automation.auto_retry.latest omits any required top-level resolved path field
- Any P076-created observation records a side-effecting retry or recovery dispatch
- Human approval gate retry attempt detected
- Required normative schema fixture is missing or incomplete
- Unsupported readback version does not return the defined JSON-RPC application error
- Artifact-read degradation is returned as a transport failure instead of successful degraded readback with diagnostics
- Budget-store read, validation, reconciliation, or durable-write failure does not force observe_only or budget_unavailable
- Ledger append does not prove write-all, trailing newline, fsync, lock scope, and cancellation shielding
- Concurrent writer lock violation detected
- Stale-lock recovery can allow two writers to proceed or lacks PID-reuse-resistant liveness checks
- MCP readback exceeds configured deadline without timeout observation
- Poll backpressure cap is missing or exceeded
- Skipped non-human work is not prioritized ahead of already-reported human gates
- Rollup cannot produce grouped issue table from valid JSONL plus budget/catalog state
- Known-issue markdown is treated as canonical instead of generated from JSON
- GraphQL, SwiftData, SQLite, workflow YAML, or agent catalog YAML is changed without a new proposal
### Metrics
- **Adoption Metric:** auto_retry_polls_with_valid_observation_record_percent
- **Operational Metrics**
  - auto_retry_observation_validation_total{status,failure_reason}
  - auto_retry_policy_decision_total{blocker_class,policy_decision}
  - auto_retry_retry_disabled_total{blocker_class,reason}
  - auto_retry_human_gate_retry_attempt_total
  - auto_retry_rollup_generation_total{status}
  - auto_retry_orphaned_retry_total{state}
  - auto_retry_poll_timeout_total{phase}
  - auto_retry_lock_contention_total{result}
  - auto_retry_budget_reconciliation_total{status}
  - auto_retry_budget_unavailable_total{reason}
  - auto_retry_stale_lock_recovery_total{result}
  - auto_retry_backpressure_skip_total{reason}
  - auto_retry_mcp_readback_failure_total{reason}
  - auto_retry_ledger_append_total{status,failure_reason}
### Migrations
- **Justification:** P076 adds local automation artifacts, scripts, generated reports, and computed MCP/readback output only. It introduces no SwiftData, SQLite, GraphQL, workflow YAML, agent catalog YAML, or artifact path-map migration.
- **Not Applicable:** `true`
### Negative Fixtures
- **Backpressure Exceeded:** docs/evidence/rollout-contract/negative/p076-backpressure-exceeded.json
- **Budget Failure Retried:** docs/evidence/rollout-contract/negative/p076-budget-failure-retried.json
- **Human Gate Retried:** docs/evidence/rollout-contract/negative/p076-human-gate-retried.jsonl
- **Human Gate Starvation:** docs/evidence/rollout-contract/negative/p076-human-gate-starvation.json
- **Invalid Enum Strict:** docs/evidence/rollout-contract/negative/p076-invalid-enum-strict.jsonl
- **Ledger Append Missing Newline:** docs/evidence/rollout-contract/negative/p076-ledger-append-missing-newline.jsonl
- **Ledger Append Not Fsynced:** docs/evidence/rollout-contract/negative/p076-ledger-append-not-fsynced.json
- **Markdown Catalog As Authority:** docs/evidence/rollout-contract/negative/p076-markdown-catalog-as-authority.json
- **Missing Budget Ref Schema:** docs/evidence/rollout-contract/negative/p076-missing-budget-ref-schema.json
- **Missing Observation Summary Schema:** docs/evidence/rollout-contract/negative/p076-missing-observation-summary-schema.json
- **Missing Readback Lock Path:** docs/evidence/rollout-contract/negative/p076-missing-readback-lock-path.json
- **Missing Readback Rollup Report Path:** docs/evidence/rollout-contract/negative/p076-missing-readback-rollup-report-path.json
- **Missing Schema Field:** docs/evidence/rollout-contract/negative/p076-missing-schema-field.jsonl
- **Orphaned Planned Attempt Not Escalated:** docs/evidence/rollout-contract/negative/p076-orphaned-planned-attempt-not-escalated.json
- **Pid Reuse Lock Liveness Gap:** docs/evidence/rollout-contract/negative/p076-pid-reuse-lock-liveness-gap.json
- **Poll Timeout Without Observation:** docs/evidence/rollout-contract/negative/p076-poll-timeout-without-observation.json
- **Retry Timeout Duplicate Not Suppressed:** docs/evidence/rollout-contract/negative/p076-retry-timeout-duplicate-not-suppressed.json
- **Side Effect Retry Present:** docs/evidence/rollout-contract/negative/p076-side-effect-retry-present.jsonl
- **Unknown Field Strict:** docs/evidence/rollout-contract/negative/p076-unknown-field-strict.json
- **Unsafe Stale Lock Recovery:** docs/evidence/rollout-contract/negative/p076-unsafe-stale-lock-recovery.json
### Operator Report Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
- auto_retry_policy_status
- auto_retry_policy_decision
- auto_retry_observation_record_id
- auto_retry_observation_path
- auto_retry_blocker_signature_id
- auto_retry_blocker_class
- auto_retry_retry_budget_state
- auto_retry_last_retry_result
- auto_retry_known_issue_status
- auto_retry_next_systemic_action
- auto_retry_rollup_report_path
- auto_retry_human_gate_retry_attempt_total
- auto_retry_budget_unavailable_reason
- auto_retry_backpressure_skip_count
- auto_retry_readback_version
- ledger_path
- budget_state_path
- known_issue_catalog_path
- generated_markdown_catalog_path
- lock_path
- rollup_report_path
### Readback Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
- auto_retry_policy_status
- auto_retry_policy_decision
- auto_retry_observation_record_id
- auto_retry_observation_path
- auto_retry_blocker_signature_id
- auto_retry_blocker_class
- auto_retry_retry_budget_state
- auto_retry_last_retry_result
- auto_retry_known_issue_status
- auto_retry_next_systemic_action
- auto_retry_rollup_report_path
- auto_retry_human_gate_retry_attempt_total
- auto_retry_budget_unavailable_reason
- auto_retry_backpressure_skip_count
- auto_retry_readback_version
- ledger_path
- budget_state_path
- known_issue_catalog_path
- generated_markdown_catalog_path
- lock_path
- rollup_report_path
- **Readback Fixture:** docs/evidence/rollout-contract/operator-readback/p076-full-surface.fixture.json
### Readback Lanes
- run_report
- mcp
- release_receipt
### Rollback Disposition
- **Data Loss Risk:** none
- **Mode:** disable_recommendations_keep_observe_only_readback
- **Steps**
  - Set policy_decision=observe_only for every blocker class and disable recommend_retry output.
  - Continue writing observation records only if validator and lock acquisition pass; otherwise pause ledger writes and keep manual inspection.
  - Keep budget, ledger, and catalog files in place for diagnosis because they are evidence and local policy state, not canonical run state.
  - Re-enable recommendations only after schema, readback, fail-closed budget behavior, ledger append durability, stale-lock safety, backpressure, and rollup fixtures pass.
- **Schema Version:** rollout_contract_v1

## Rollout Plan

1. **Item**
   - **Exit Criteria**
     - ./scripts/test-gate.sh proposal-076 passes
     - ./scripts/test-gate.sh p076 passes
     - Fixtures cover complete schemas, enum closure, unknown-field behavior, top-level readback path echo, readback success, no observation history, degraded artifact success, unsupported_version error, orphaned planned attempts, timeout ambiguity, ledger append durability, fsync/rename semantics, backpressure, and no side-effecting retry.
   - **Phase:** Contract and fixture proof
   - **Steps**
     - Land JSON Schema or equivalent strict fixtures for every normative P076 object, including budget_ref_v1 and observation_summary_v1.
     - Add validator modes for strict gate and permissive report behavior.
     - Add rollout-contract readback fixture and negative fixtures.
2. **Item**
   - **Exit Criteria**
     - Every completed poll writes exactly one valid event or exits before lock acquisition without side effects.
     - Catalog JSON dedupes repeated signatures.
     - Budget-store failures produce budget_unavailable and no retry.
     - Human gates remain approval work with zero retry attempts.
   - **Phase:** Observe-only ledger and catalog
   - **Steps**
     - Update automation to acquire the single-writer lock, collect MCP evidence, append valid newline-terminated observations with write-all and fsync semantics, update budget/catalog state, and regenerate markdown from JSON only.
     - Keep max_retry_actions_per_poll at0 and record retry_disabled_pending_idempotency_contract when a retry would otherwise be recommended.
3. **Item**
   - **Exit Criteria**
     - Operators can read latest observation_id, blocker signature, policy decision, budget state, known issue state, next systemic action, and all resolved path fields without parsing legacy markdown.
     - Historical runs without observations read back as no_observation_history.
     - Artifact-read degradation is a successful response with diagnostics and empty arrays.
     - Unsupported versions return the defined JSON-RPC application error.
   - **Phase:** Operator readback
   - **Steps**
     - Expose automation.auto_retry.latest through chainworks-orchestrator-ops.
     - Derive run report and release receipt fields from the same readback object or generated JSON report.
     - Use AutoRetryReadbackRepository or equivalent app-owned adapter for any SwiftUI display.
4. **Item**
   - **Exit Criteria**
     - P076 remains releasable without any side-effecting retry surface.
     - The later proposal cannot claim P076 approval as authorization to retry.
   - **Phase:** Retry enablement handoff
   - **Steps**
     - Keep side-effecting retry out of P076 implementation.
     - Open a later proposal if bounded retry should be enabled, with a concrete MCP command contract and fixture-proven idempotency/settlement behavior.

## Metrics

- **Cardinality Policy:** Raw blocker_signature_id values are emitted in ledger, catalog, rollup, and logs, not as unbounded metric labels.
### Operational Metrics
- auto_retry_observation_validation_total{status,failure_reason}
- auto_retry_policy_decision_total{blocker_class,policy_decision}
- auto_retry_retry_disabled_total{blocker_class,reason}
- auto_retry_human_gate_retry_attempt_total
- auto_retry_rollup_generation_total{status}
- auto_retry_orphaned_retry_total{state}
- auto_retry_poll_timeout_total{phase}
- auto_retry_lock_contention_total{result}
- auto_retry_budget_reconciliation_total{status}
- auto_retry_budget_unavailable_total{reason}
- auto_retry_stale_lock_recovery_total{result}
- auto_retry_backpressure_skip_total{reason}
- auto_retry_mcp_readback_failure_total{reason}
- auto_retry_ledger_append_total{status,failure_reason}
- **Primary Adoption Metric:** auto_retry_polls_with_valid_observation_record_percent
### Success Thresholds
- 100 percent of enabled completed polls produce a valid observation record or a documented no-side-effect lock skip.
- 0 side-effecting retry or recovery calls issued by P076.
- 0 human-gate retry attempts.
- 0 retries for unknown blockers.
- 0 retries while budget state is unavailable or unreconciled.
- Repeated signatures dedupe in catalog JSON with count increments, not duplicate narrative sections.
- Rollup can generate a proposal-ready table from fixtures and at least one real observation sample.

## Acceptance Criteria

- scripts/test-gate.sh proposal-076 and scripts/test-gate.sh p076 exist and pass.
- Validator fixtures cover required fields, optional fields, nullable fields, scalar types, nested object shapes, budget_ref_v1, observation_summary_v1, closed enum domains, diagnostics shapes, version_negotiation shape, unsupported_version error shape, degraded-success readback, strict unknown-field rejection, and permissive unknown-field diagnostics.
- automation.auto_retry.latest schema and narrative agree on top-level ledger_path, budget_state_path, known_issue_catalog_path, generated_markdown_catalog_path, lock_path, and rollup_report_path.
- Valid fixture records roll up into grouped issues by blocker_signature_id.
- P076-created observations never record side-effecting retry dispatch; retry_action is none or recommend_retry and retry_result is not_attempted or not_allowed.
- Any budget-store read, validation, reconciliation, lock, or durable-write failure forces observe_only or budget_unavailable before any side-effect lane could be considered.
- Ledger append fixtures prove write-all, trailing newline, fsync, lock scope, cancellation shielding, and partial trailing record diagnostics.
- The single-writer lock prevents overlapping decisions and stale-lock recovery uses hostname, boot/session identity, process start time, command identity, PID, and tokenized compare-and-replace semantics.
- MCP calls and whole-poll deadlines are enforced and represented in observations.
- automation.auto_retry.latest has fixture-proven success, no_observation_history, degraded-success, unsupported_version, and null-readback behavior.
- Poll backpressure caps considered runs, records skipped work, prioritizes skipped non-human work next poll, and prevents already-reported human gates from starving non-human recovery evidence.
- The known-issue JSON catalog dedupes repeated signatures and markdown is generated from JSON only.
- Retry guidance uses workflow stage_id; stage_execution_id remains evidence unless a later MCP contract explicitly supports it.
- Rollback switches the automation to observe-only without data migration. Since P076 is already observe-only, rollback primarily disables recommendation generation while retaining readback where validators pass.
- Reference documentation explains the implemented ledger, budget store, catalog, readback contract, observe-only policy, UI rendering map, diagnostic row shape, backpressure, no-change compatibility, and proof gate.

## Risks And Mitigations

1. **Item**
   - **Mitigation:** Make the trade-off explicit: P076 buys durable evidence and safety; a later idempotent MCP command proposal unlocks bounded retry without ambiguity.
   - **Risk:** Observe-only scope disappoints operators expecting automatic recovery.
2. **Item**
   - **Mitigation:** Keep generated artifacts local by default, retain enough history for budget auditing, add rollup/report modes, and snapshot selected evidence only.
   - **Risk:** JSONL evidence grows without bound.
3. **Item**
   - **Mitigation:** Require classification, evidence capture, durable budget state, cooldown, and needs_systemic_fix or needs_human_triage escalation.
   - **Risk:** The monitor hides real blockers by recommending retry too aggressively.
4. **Item**
   - **Mitigation:** Fail closed to observe_only or budget_unavailable on any budget read, validation, reconciliation, lock, or durable-write failure.
   - **Risk:** Budget-state corruption causes unsafe behavior.
5. **Item**
   - **Mitigation:** Require exclusive lock, identity-rich liveness checks, tokenized stale-lock recovery, and cancellation shielding for mutation critical sections.
   - **Risk:** Concurrent polls corrupt ledger or budget invariants.
6. **Item**
   - **Mitigation:** Set per-call and per-poll deadlines; write partial timeout observations and skip side effects.
   - **Risk:** MCP transport stalls and poll overlap accumulates.
7. **Item**
   - **Mitigation:** Prioritize previously skipped non-human work and reserve non-human consideration capacity when available.
   - **Risk:** Large human-gate inventories starve non-human recovery evidence.
8. **Item**
   - **Mitigation:** Make JSON catalog canonical and markdown generated-only.
   - **Risk:** Catalog becomes another hand-maintained markdown sink.
9. **Item**
   - **Mitigation:** Expose observation_id, signature, policy decision, budget state, retry result, next systemic action, retry-disabled reason, and copyable diagnostics in MCP/readback surfaces.
   - **Risk:** Operators lose trust if automation acts invisibly.

## Reviewer Feedback Resolution

### Addressed Backlog Items
- API-CONTRACT-076-R12-001
- API-CONTRACT-076-R12-002
- UI-076-001
- UI-076-002
- UI-076-003
- REL-076-R12-001
- REL-076-R12-002
- APPLE-076-NB-001
- MACOS-076-NB-001
### Resolution Summary
- Defined budget_ref_v1 and observation_summary_v1 with required fields, nullable fields, scalar constraints, and additionalProperties=false.
- Reconciled path echo requirements by making lock_path and rollup_report_path top-level required auto_retry_readback.v1 fields alongside the other resolved paths.
- Added enum-to-component and enum-to-tone maps for policy_decision and budget_status.
- Defined compact diagnostic row anatomy, row-scoped disclosure, severity icon placement, expansion contents, accessibility labels, and copy behavior.
- Grouped auto_retry and rollout_contract readback fields into primary, lifecycle, diagnostic, and evidence clusters.
- Specified ledger append write-all, newline termination, fsync, lock scope, cancellation shielding, and partial trailing record diagnostics.
- Strengthened stale-lock liveness checks with hostname, boot/session identity, process start time, command identity, PID, and lock token to reduce PID-reuse risk.
- Added scene/window teardown cancellation and background-priority diagnostic refresh rules for app-side readback.
- Required selectable/copyable diagnostic identifiers and context-menu copy actions for blocker signatures and diagnostic details on macOS.
- **Source Review Pass Id:** 9a635252-b59e-45a2-92c0-b7a3805973fe

## Open Questions

- Which later proposal should define the concrete idempotent MCP retry or recovery command, if bounded retry is still desired after P076 evidence is available?
- Should legacy /Users/user/.codex/automations/auto-retry/known-issues.md be archived after the first successful structured rollup, or retained as historical context until several clean polls pass?
- Should auto-retry-overrides.json be a short-term local operator file only, or should a future recovery command own override authorization?
- What long-term retention and compaction policy should apply after the initial seven-day or one-hundred-times-window floor is proven?
