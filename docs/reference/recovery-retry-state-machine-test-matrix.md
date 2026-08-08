# Recovery and Retry State-Machine Test Matrix

## Introduction

This reference is the single authoritative recovery/retry state-machine matrix and proof gate for Chainworks Forge. Recovery fixes across restart-mid-command, stale execution truth, duplicate mediation, late output, ACP startup recovery, and cancellation must use this matrix as the shared contract.

This reference defines:

- the canonical scenario matrix (P082-R01 through P082-R17);
- the reason-code vocabulary (append-only);
- the column schema that each matrix row must satisfy;
- the nested subcontract schemas for readback objects;
- the exact readback lane placement rules per MCP surface;
- the fail-closed side-effect behavior;
- the late-output quarantine semantics;
- the approval restart, cancellation, and startup-requeue-exhausted semantics;
- the long-held observability thresholds;
- the live-principal authorization boundary for recovery operator-only diagnostics;
- the gate ownership.

**Rule**: Recovery behavior changes add or update matrix rows before the behavior change lands. No recovery mutation may ship without a corresponding row in this document.

---

## Scenario ID Convention

Scenario IDs use the form `P082-R01` through `P082-R17`. IDs are append-only and never renumbered. When a new scenario is added it receives the next sequential suffix.

---

## Reason-Code Vocabulary

The following reason codes are the canonical vocabulary for `recovery_reason_code` in `p082_recovery_matrix_readback_v1`. This list is append-only; existing codes are never removed or renamed.

| Reason Code | Meaning |
|---|---|
| `resume_claim_status` | Startup requeue in progress; claim status is being resumed |
| `startup_requeue_once` | Work item requeued exactly once at startup recovery |
| `startup_requeue_exhausted` | Allowed requeue generation consumed; run is held pending operator clearance |
| `invalid_stage_for_retry` | Retry command rejected because target stage or status is not retryable |
| `ignored_late_outputs` | Late agent output arrived after superseding generation; quarantined |
| `duplicate_owner_repaired` | Duplicate session or startup ownership claim was detected and repaired |
| `startup_stalled` | ACP startup row remained running without a provider session after the grace window |
| `stale_repaired` | Stale scheduler ownership detected and repaired through an explicit transition |
| `needs_effect_reconciliation` | Stale repair held because unresolved side-effect ledger rows require reconciliation first |
| `requires_effect_reconciliation` | Retry or release blocked because unresolved side-effect ledger rows exist |
| `valid_identifier_guidance` | Retry command rejected because the operator supplied the wrong identifier kind; guidance provided |
| `approval_pending_operator_action_required` | Daemon restarted while an approval was pending; visibility restored, no auto-resolution |
| `duplicate_mediation_owner_rejected` | Duplicate mediation or session attempt for the same conflict owner was rejected |
| `cancel_active_stage_requested` | Cancellation requested while an active stage, retry, invoke, or provider session was running |
| `cancel_pending_approval_preserved` | Legacy reason code retained for append-only compatibility; superseded by `cancel_pending_approval_expired` for current cancellation settlement |
| `cancel_pending_approval_expired` | Cancellation requested while an approval was pending; pending approval is expired/terminalized so it cannot remain actionable |
| `cancel_side_effect_reconciliation_required` | Cancellation held because unresolved side-effect rows require external settlement |
| `cancel_startup_repair_converged` | Cancellation raced with startup recovery requeue; both converged idempotently |
| `cancelled_provider_late_output_ignored` | Output arrived from a cancelled or terminalized provider session; quarantined without active-projection mutation |
| `repair_crash_resume_idempotent` | Daemon crashed during a repair; replay used the subsystem idempotency key and converged without duplicate mutation |

---

## Matrix Column Definitions

Each matrix row must populate the following columns.

| Column | Description |
|---|---|
| **ID** | Scenario identifier (`P082-R01` etc.) |
| **Scenario** | Short name for the fault scenario |
| **Reason Code** | The `recovery_reason_code` value emitted for this scenario |
| **Setup** | Initial state or precondition required to trigger the scenario |
| **Expected Repair/Reject** | What the engine must do: repair, hold, reject, or quarantine |
| **DB Assertion** | At least one durable-storage assertion (row existence, column value, uniqueness constraint) |
| **Engine Assertion** | At least one engine or command-handler assertion (eligibility check, idempotency, no duplicate scheduling) |
| **Readback Requirement** | Which readback objects or fields must be present and non-null |
| **Durable Owner** | Tables that hold the authoritative durable evidence for this scenario |
| **Projection Path** | The readback path that must contain a row for this scenario (`p082_recovery_matrix_readbacks[scenario_id=P082-RNN]`) |
| **Crash/Replay Proof** | What crash-boundary replay must demonstrate |
| **Obs Threshold** | Observability warning/critical thresholds for long-held states (n/a when instantaneous) |

---

## Canonical Matrix

| ID | Scenario | Reason Code | Setup | Expected Repair/Reject | DB Assertion | Engine Assertion | Readback Requirement | Durable Owner | Projection Path | Crash/Replay Proof | Obs Threshold |
|---|---|---|---|---|---|---|---|---|---|---|---|
| P082-R01 | Restart mid command | `startup_requeue_once` | `command_journal` has an accepted command; the associated work item is unsettled after daemon restart | Write `startup_repairs.id=p082-requeue:{cj.id}:{wi.id}:1`; requeue once; replay idempotently; hold as `startup_requeue_exhausted` if a second non-replay requeue is required | `startup_repairs` row present with source fields; `work_items.payload_json.p061_startup_recovery` populated; `max_requeue_generation=1` | No duplicate `stage_execution`, `session_generation`, or `retry_authority`; crash-boundary replay converges to a single owner | `p082_startup_repair_summary_v1` populated; `next_retry_or_backoff_time` follows the startup repair/work-item owner state | `startup_repairs`, `work_items`, `command_journal` | `p082_recovery_matrix_readbacks[scenario_id=P082-R01]` | Crash after each durable write boundary converges | startup repair hold: warn=900s, crit=1800s |
| P082-R02 | Reject non-retryable stage retry | `invalid_stage_for_retry` | `stages.retry` targets a non-retryable stage or a stage in a terminal status | Reject before any mutation; write `p082_rejected_command_error_v1` envelope to `command_journal.error` | No new `stage_execution`, `work_item`, `retry_authority`, or `instruction_binding` row; `command_journal.error` contains `p082_rejected_command_error_v1`; `command_journal.payload_json` unchanged | Eligibility validated before enqueue; redacted typed error envelope written | `reason=invalid_stage_for_retry`; `recovery_decision=no_mutation`; source is `command_journal.error` | `command_journal`, `stages` | `p082_recovery_matrix_readbacks[scenario_id=P082-R02]` | `command_journal.payload_json` is never mutated | n/a (instant rejection) |
| P082-R03 | Late output after supersede | `ignored_late_outputs` | Old agent output arrives after a retry has superseded the source generation claim | Ignore and quarantine old output; terminalize the superseded source work item | `artifact_source_generation_claims.claim_state=superseded` or `closed`; `agent_execution_runtime_facts.ignored_late_output_count` incremented; superseded work item completed or failed; active artifact record unchanged | Active stage projection and artifact links not regressed by late output | `p082_late_output_settlement_v1` with `active_projection_changed=false` | `agent_execution_runtime_facts`, `artifact_source_generation_claims`, `work_items` | `p082_recovery_matrix_readbacks[scenario_id=P082-R03]` | Ignored late output terminalizes the superseded work item | n/a (idempotent) |
| P082-R04 | Duplicate session/startup claim | `duplicate_owner_repaired` | Two startup claims target the same active work item | Keep one durable owner; reject or terminalize the duplicate | Exactly one active `session_lineages.active_generation_id`; duplicate `session_events` row is terminal or rejected | Scheduler not double-counted after repair | `reason=duplicate_owner_repaired`; `next_action=inspect_duplicate_owner` when review is warranted | `session_lineages`, `session_generations`, `session_events`, `work_items` | `p082_recovery_matrix_readbacks[scenario_id=P082-R04]` | Replay is idempotent with a single surviving owner | n/a |
| P082-R05 | Stale ACP startup | `startup_stalled` | A running invoke work item has no `provider_session_id` and no `last_activity_at` after the grace window (3 min standard; 12 min Xcode) | Invalidate the startup generation and requeue once when eligible; when `xcode_required=true` a non-null `recovery_operator_message` naming the Xcode grace window and cutoff is required | `session_generations` row ended with `end_reason=stale_acp_startup_without_provider_session`; one replacement pending work item; `work_items.payload_json.p061_startup_recovery` contains the P082-R05 readback owner | Startup and watchdog paths share eligibility and idempotency checks; scheduler capacity not consumed twice | `p082_startup_repair_summary_v1` present; if `xcode_required=true`, `recovery_operator_message` is non-null naming Xcode grace and cutoff; `source_json_key=work_items.payload_json.p061_startup_recovery` | `work_items`, `session_generations`, `session_events`, `startup_repairs` | `p082_recovery_matrix_readbacks[scenario_id=P082-R05]` | Replay converges; Xcode grace path requires non-null operator message | Xcode startup grace: warn=720s, crit=900s |
| P082-R06 | Stale scheduler ownership | `stale_repaired` | A running work item has no live executor owner | Repair through explicit transition or hold for reconciliation; never issue a blind retry | `work_items` status changes only through a recorded transition; unresolved `side_effects` rows unchanged | Capacity freed only through a recorded transition | `reason=stale_repaired` when repaired with `source_json_key=work_items.payload_json.p061_startup_recovery`; `needs_effect_reconciliation` when held | `work_items`, `startup_repairs`, `side_effects` | `p082_recovery_matrix_readbacks[scenario_id=P082-R06]` | Idempotent replay | startup repair hold: warn=900s, crit=1800s |
| P082-R07 | Release side-effect drift | `requires_effect_reconciliation` | Unresolved `side_effects` row exists for the run or the target stage | Block retry; route to side-effect reconciliation | `side_effects.status` unchanged; no `side_effect_attempts` retry row added; no release work item scheduled; rejected command stores `p082_rejected_command_error_v1` | No duplicate external mutation scheduled | `recovery_side_effect_blocking_status` populated; `recovery_operator_message` non-null | `side_effects`, `side_effect_attempts`, `command_journal` | `p082_recovery_matrix_readbacks[scenario_id=P082-R07]` | Fail-closed: no retry until reconciled | side-effect reconciliation hold: warn=3600s, crit=14400s |
| P082-R08 | Retry identifier mismatch | `valid_identifier_guidance` | Operator supplies the wrong identifier kind (e.g. stage execution UUID where workflow stage ID is required) | Reject with deterministic guidance before any mutation | No retry mutation, work item, retry authority, or instruction binding created; `command_journal.error` contains `p082_rejected_command_error_v1` with nested `p082_retry_identifier_guidance_v1`; `payload_json` unchanged | MCP error and report readback name the expected identifier kind and provide valid examples | `p082_retry_identifier_guidance_v1` with `no_mutation=true` | `command_journal`, `retry_payload_recovery_events` | `p082_recovery_matrix_readbacks[scenario_id=P082-R08]` | `payload_json` is never mutated | n/a |
| P082-R09 | Pending human approval restart | `approval_pending_operator_action_required` | Daemon restarts while a human approval is pending | Restore pending approval visibility without auto-resolution | `approvals.decision=pending`; `decided_at=null`; `approval_inbox` has a pending entry | Orchestrator waits at the approval gate; no synthesized approval or rejection | `recovery_decision=operator_approval_required`; `next_action` points to the existing approval path | `approvals`, `approval_inbox`, `stage_executions` | `p082_recovery_matrix_readbacks[scenario_id=P082-R09]` | Restart restores pending state only; no fabricated decision | pending approval: warn=86400s, crit=259200s |
| P082-R10 | Duplicate mediation attempt | `duplicate_mediation_owner_rejected` | A duplicate mediation or session attempt targets the same conflict owner | Keep one active mediation owner; preserve duplicate evidence | `lead_conflict_mediations` active fingerprint is unique; `lead_mediation_confirmations` has at most one pending entry per mediation | No duplicate lead conflict settlement | Readback names the current mediation owner | `lead_conflict_mediations`, `lead_mediation_confirmations`, `workflow_conflicts` | `p082_recovery_matrix_readbacks[scenario_id=P082-R10]` | Replay is idempotent with a single surviving owner | n/a |
| P082-R11 | Cancel interleaved with active stage | `cancel_active_stage_requested` | Cancellation requested while an active stage, retry, invoke operation, or provider session is running | Settle cancellation; prevent new invoke work; terminalize the active provider session record | `runs.cancellation_requested_at` set; `cancellation_settlement_log` has one non-empty `action_id`-scoped entry per settled item, including a synthetic P082 readback entry when active invoke ownership exists before a running `agent_execution` row; work items settled exactly once; `session_generations` and `session_events` record terminalization; no duplicate `retry_authority` | No duplicate work, owners, side effects, or orphaned provider sessions after replay; provider subprocess cleanup proof cites ACP transport lifecycle evidence | `scenario_status=cancelled` or held with a clear held-vs-cancelled message | `runs`, `work_items`, `retry_stage_execution_authorities`, `session_generations`, `session_events` | `p082_recovery_matrix_readbacks[scenario_id=P082-R11]` | Replay in either ordering converges; provider subprocess cleanup required | n/a |
| P082-R12 | Cancel with pending approval | `cancel_pending_approval_expired` | Cancellation requested while a human approval is pending | Cancel the run and expire pending approval actionability | pending approval rows are terminalized with cancellation evidence; `approval_inbox` has no actionable row for the cancelled run; run projections report `pending_approvals=0` | Approval gate does not resume work after cancellation; approvals.list excludes the cancelled run | `next_action` describes cancellation settlement, not an approval retry | `runs`, `approvals`, `approval_inbox` | `p082_recovery_matrix_readbacks[scenario_id=P082-R12]` | Idempotent convergence; startup recovery repairs legacy cancelled-run pending approvals | n/a |
| P082-R13 | Cancel with unresolved side effects | `cancel_side_effect_reconciliation_required` | Cancellation requested while unresolved `side_effects` rows exist | Cancel future scheduling; hold external-effect settlement for reconciliation | `side_effects.status` unchanged except through explicit reconciliation; no `side_effect_attempts` retry row; `cancellation_settlement_log` records the hold | No duplicate external side effect; cancellation does not mask reconciliation need | `recovery_decision=reconcile_side_effects`; `recovery_operator_message` non-null | `runs`, `side_effects`, `side_effect_attempts` | `p082_recovery_matrix_readbacks[scenario_id=P082-R13]` | Idempotent; no retry during hold | side-effect reconciliation hold: warn=3600s, crit=14400s |
| P082-R14 | Cancel interleaved with startup repair | `cancel_startup_repair_converged` | Cancellation races with a startup recovery requeue | Cancellation wins for future scheduling; already-journaled repair converges idempotently | `startup_repairs` retains exactly one idempotency row; no cancelled-and-pending duplicates for the same source; `cancellation_settlement_log` records the interaction | Replay in either ordering converges without duplicate work, owners, or provider sessions | `p082_startup_repair_summary_v1` names the repair idempotency key and replay state | `runs`, `startup_repairs`, `work_items`, `session_generations` | `p082_recovery_matrix_readbacks[scenario_id=P082-R14]` | Convergence verified under both ordering scenarios | n/a |
| P082-R15 | Daemon crash during repair | `repair_crash_resume_idempotent` | Daemon crashes after the eligibility check but before final readback settlement | Replay repair using the subsystem idempotency key; converge without duplicate mutation; crash-loop replay variant for the same key across multiple restarts | Exactly one durable repair key per affected subsystem; no duplicates across repeated crashes | Recovery service resumes from every crash point; projections remain consistent; provider subprocess cleanup evidence cited when a provider session is involved | `recovery_projection_integrity=valid`; `replayed=true` when a duplicate key is observed | Row-specific per crash boundary (`startup_repairs`, `retry_payload_recovery_events`, `side_effects`, `runs`, `command_journal`) | `p082_recovery_matrix_readbacks[scenario_id=P082-R15]` | Repeated-crash variant: same idempotency key across multiple restarts produces a single owner, single work item, and single readback row | n/a |
| P082-R16 | Startup requeue exhausted | `startup_requeue_exhausted` | Startup recovery observes the same source work item after the allowed requeue generation has been consumed | Hold without enqueuing duplicate work, a new session generation, or any side-effect mutation; operator clearance via existing recovery inspection or cancellation paths | `startup_repairs` retains exactly one idempotency row; no second pending work item for the same `source_work_item_id`; `scenario_status=held`; `recovery_operator_message` is non-null | No duplicate generations or provider sessions; scheduler capacity not consumed | `scenario_status=held`; `reason_code=startup_requeue_exhausted`; `p082_startup_repair_summary_v1` present; operator message names the clearance path | `startup_repairs`, `work_items` | `p082_recovery_matrix_readbacks[scenario_id=P082-R16]` | Idempotent: a second observation of the same key produces the same held state | startup_requeue_exhausted: warn=0s, crit=300s |
| P082-R17 | Cancel then late provider output | `cancelled_provider_late_output_ignored` | A provider session is cancelled or terminalized; output then arrives from that cancelled generation | Classify as late output; quarantine or ignore it; preserve evidence; no active projection mutation | `session_generations` and `session_events` show cancellation; `artifact_source_generation_claims=superseded` or `closed`; source work item is terminal; `agent_execution_runtime_facts.ignored_late_output_count` incremented; active artifacts, `state/run-state.json`, `artifacts/active-index.json`, and reports unchanged | Cancelled provider output cannot update active artifacts, reports, stage projections, retry authorities, or side-effect state | `p082_late_output_settlement_v1` with `cancelled_provider_session=true` and `active_projection_changed=false` | `session_generations`, `session_events`, `artifact_source_generation_claims`, `agent_execution_runtime_facts`, `work_items` | `p082_recovery_matrix_readbacks[scenario_id=P082-R17]` | Late output after cancellation does not mutate the active run-state or active-index projection | n/a |

---

## Readback Lane Placement

### MCP `runs.get`

Both the singular and plural fields must be present:

- `p082_recovery_matrix_readback` — singular; the latest non-`not_applicable` readback row
- `p082_recovery_matrix_readbacks` — plural; all readback rows for the run

### MCP `reports.get`

- `p082_recovery_matrix_readbacks` — plural; present at the result level and on each aggregated report entry (`reports[].p082_recovery_matrix_readbacks`)
- The singular `p082_recovery_matrix_readback` field **must be absent** from `reports.get` at every level

### `report://{run_id}` resource

- `p082_recovery_matrix_readbacks` — plural only
- Readback rows must be byte-equivalent (snake_case) with the `reports.get` P082 payloads

### Run report JSON

- `p082_recovery_matrix_readbacks` — plural only

### Release receipts

- `p082_recovery_matrix_readbacks` — plural diagnostic field
- `rollout_contract_readback` — present
- No recovery command affordances in release receipts

### Advisory GraphQL (optional)

- `p082RecoveryMatrixReadbackJson` — camelCase; diagnostic-only; tolerant; not required
- `p082RecoveryMatrixReadbacksJson` — camelCase; diagnostic-only; tolerant; not required
- GraphQL readback is advisory. It must never be a required readback lane without an explicit contract amendment and tolerant test coverage.

### Principal-Class Gating

All P082 readback lanes are gated to operator principals, but denied surfaces fail closed before loading report payload lanes.

- `runs.get`: non-operator principals (agent, observer) receive `p082_recovery_matrix_readback: null` and `p082_recovery_matrix_readbacks: []`. Adjacent operator-only diagnostics, including completion receipts, workflow conflicts, retry authority history, P091 repair details, rollout readback, and artifact payload lanes, are omitted or redacted consistently with the existing non-operator `runs.get` boundary.
- `reports.get`: non-operator principals are denied before report lanes are loaded; they do not receive report entries, generated run-report artifacts, release-receipt payloads, or P082 readback rows.

### Live Principal Reload Boundary

P082 readback lanes can expose operator-only diagnostics, recovery evidence, conflict details, retry authority history, rollout readback, and artifact payload lanes. Any northbound surface touched by the P082 implementation must authorize against the live/reloadable principal source rather than a startup snapshot. Revoked, disabled, or re-scoped bearer principals must be rejected after reload on MCP HTTP, MCP stdio, failed-serve diagnostics, and existing GraphQL HTTP/WebSocket guards.

This boundary is access-control scope, not GraphQL readback scope. GraphQL P082 readback fields remain optional and diagnostic-only; implementing live GraphQL auth does not make GraphQL a P082 recovery authority or a required P082 readback lane.
- `report://{run_id}`: non-operator principals are denied before the report resource payload is materialized; they do not receive embedded run-report or release-receipt lanes.
- Operator report surfaces keep their lane field-name contract: `reports.get`, `report://{run_id}`, generated run reports, and release receipts expose plural `p082_recovery_matrix_readbacks` only; `reports.get` and report resources do not expose singular `p082_recovery_matrix_readback`.

---

## Nested Schema Contracts

### `p082_recovery_matrix_readback_v1`

| Field | Type | Notes |
|---|---|---|
| `schema_version` | string | Always `"p082_recovery_matrix_readback_v1"` |
| `scenario_id` | string | One of `P082-R01`–`P082-R17` |
| `scenario_status` | string | `repaired`, `rejected`, `held`, `pending`, `cancelled`, `not_applicable` |
| `recovery_decision` | string | `retry`, `wait`, `reconcile_side_effects`, `operator_approval_required`, `inspect_duplicate_owner`, `cancel`, `no_mutation` |
| `recovery_reason_code` | string | Never null; one value from the reason-code vocabulary |
| `recovery_next_action` | string | Non-null; empty string only when `scenario_status` is `not_applicable` |
| `recovery_hold_conditions` | array of strings | Empty when not held |
| `recovery_side_effect_blocking_status` | string or null | Non-null when side-effect rows are blocking |
| `recovery_retry_identifier_guidance` | `p082_retry_identifier_guidance_v1` or null | Non-null for `valid_identifier_guidance` rows |
| `recovery_late_output_settlement` | `p082_late_output_settlement_v1` or null | Non-null for `ignored_late_outputs` and `cancelled_provider_late_output_ignored` rows |
| `recovery_startup_repair_summary` | `p082_startup_repair_summary_v1` or null | Non-null for startup repair rows |
| `recovery_operator_message` | string or null | Human-readable message; non-null when the operator must act |
| `recovery_projection_integrity` | string | `valid`, `stale`, `tamper_detected`, `unavailable` |
| `source_table` | string | The DB table, or accessor-derived owner set, that is the authoritative source |
| `source_repository` | string | The Rust repository or owner set that owns this row |
| `source_identifier` | string | Row ID or composite key; cancellation readbacks use the settlement `action_id`, or `action_id:agent_execution:{agent_execution_id}` when one cancellation action settles parallel agent executions |
| `source_json_key` | string or null | Approved owner-path key for JSON/text owners; null only for typed-column-derived rows |
| `updated_at` | string (ISO-8601) | Last modification timestamp |
| `diagnostic_redaction` | string | `none`, `partial`, `full` |

### Approved `source_json_key` Owners

`source_json_key` is validated against an explicit owner-path allowlist before a
row can leave the readback accessor. Row-specific JSON/text owners must match
the scenario contract below; a null `source_json_key` is valid only for
typed-column-derived rows or the legacy plain-text `command_journal.error`
fallback for P082-R02 with `recovery_projection_integrity=unavailable`.

| Owner path | Required or approved use |
|---|---|
| `command_journal.error.p082_recovery_matrix_readback` | Required for typed rejected-command rows, including P082-R02, P082-R07, and P082-R08 |
| `startup_repairs.notes.p082_recovery_matrix_readback` | Required for P082-R01, P082-R15, and P082-R16 |
| `work_items.payload_json.p061_startup_recovery` | Required for P082-R05 stale ACP startup and pre-session startup repair readback rows; approved for explicit repaired P082-R06 stale scheduler ownership rows |
| `stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback` | Required for P082-R03 and P082-R17; approved for P082-R09 pending-approval restart readback |
| `runs.cancellation_settlement_log.p082_recovery_matrix_readback` | Required for P082-R11 through P082-R14 cancellation settlement readbacks |
| `retry_payload_recovery_events.diagnostic_json.p082_recovery_matrix_readback` | Approved retry-payload recovery diagnostic owner |
| `session_events.details_json.p082_recovery_matrix_readback` | Approved for duplicate-session evidence, including P082-R04; stale-startup session events are supporting evidence, while P082-R05 readback rows use `work_items.payload_json.p061_startup_recovery` |
| `lead_conflict_mediations.validation_errors_json.p082_recovery_matrix_readback` | Approved for P082-R10 duplicate mediation owner evidence |
| `workflow_conflicts.record_json.p082_recovery_matrix_readback` | Approved workflow-conflict diagnostic owner |

### `p082_rejected_command_error_v1`

Stored in `command_journal.error` (text column). **Never stored in `command_journal.payload_json`**.

The P082 readback accessor selects typed rejected-command envelopes from `command_journal` rows whose `result_status` is `failed` or `rejected`. Both statuses are operator-safe rejection outcomes for this readback lane; `payload_json` remains the original inserted command input in either case.

Backward-compatible parsing rule: validate JSON and schema before use; fall back safely for legacy plain-text errors; never expose the raw envelope JSON to operators.

| Field | Type | Notes |
|---|---|---|
| `schema_version` | string | Always `"p082_rejected_command_error_v1"` |
| `reason_code` | string | Never null; one value from the reason-code vocabulary |
| `command_type` | string | The command type that was rejected |
| `redaction` | string | `none`, `partial`, `full` |
| `operator_safe_summary` | string | Human-readable summary safe to display |
| `p082_recovery_matrix_readback` | `p082_recovery_matrix_readback_v1` | Inline readback for the rejection context; must be valid and non-null |

### `p082_retry_identifier_guidance_v1`

| Field | Type | Notes |
|---|---|---|
| `schema_version` | string | Always `"p082_retry_identifier_guidance_v1"` |
| `command` | string | The command that was rejected |
| `provided_identifier` | string | The identifier value the operator supplied |
| `provided_identifier_kind` | string | Kind of the supplied identifier; one of `workflow_stage_id`, `stage_execution_uuid`, `retry_authority_id`, `work_item_id`, or `unknown` |
| `expected_identifier_kind` | string | Kind that was required; one of `workflow_stage_id`, `stage_execution_uuid`, `retry_authority_id`, or `work_item_id` |
| `valid_identifier_examples` | array of strings | One or more valid examples for the expected kind |
| `no_mutation` | boolean | Always `true`; confirms no state was mutated |

### `p082_late_output_settlement_v1`

| Field | Type | Notes |
|---|---|---|
| `schema_version` | string | Always `"p082_late_output_settlement_v1"` |
| `source_agent_execution_id` | string | The agent execution that produced the late output |
| `source_work_item_id` | string | The work item associated with the superseded execution |
| `source_session_generation_id` | string | The session generation that was superseded or cancelled |
| `active_session_generation_id` | string | The current active session generation ID |
| `claim_state` | string | `superseded`, `closed`, `ignored` |
| `output_settlement` | string | `quarantined`, `ignored` |
| `ignored_late_output_count` | integer | Total count of ignored late outputs for this execution |
| `source_work_item_terminal_status` | string | Terminal status of the superseded work item |
| `active_projection_changed` | boolean | Always `false`; confirms active projection was not mutated |
| `cancelled_provider_session` | boolean | `true` when the source generation was cancelled |

### `p082_startup_repair_summary_v1`

| Field | Type | Notes |
|---|---|---|
| `schema_version` | string | Always `"p082_startup_repair_summary_v1"` |
| `startup_repair_id` | string | The `startup_repairs.id` idempotency key |
| `source_work_item_id` | string | The work item being repaired |
| `source_command_journal_id` | string | Non-empty `command_journal.id` of the originating command (required) |
| `requeue_generation` | integer | Generation count for this requeue (1 for the first) |
| `max_requeue_generation` | integer | Maximum allowed generation (1) |
| `replayed` | boolean | `true` when this repair was replayed from an existing idempotency key |
| `stale_after_ms` | integer | Grace window in milliseconds before a session is considered stale |
| `stale_cutoff` | string (ISO-8601) | Absolute cutoff timestamp |
| `xcode_required` | boolean | `true` when the Xcode startup grace path applies |
| `next_retry_or_backoff_time` | string (ISO-8601) or null | When the next retry or backoff check will occur |
| `backpressure_scope` | string | Scope for capacity backpressure |

---

## Command Journal Typed Envelope Contract

The `p082_rejected_command_error_v1` envelope is stored in `command_journal.error` (the text column). It is **not** stored in `command_journal.payload_json`. The `payload_json` column is write-once at command insertion and must never be mutated by recovery or rejection logic.

Parsing rules:

1. Attempt JSON parse of `command_journal.error`.
2. If JSON is valid, validate the `schema_version` field against known schemas.
3. If schema is recognized and all required fields are present, use typed readback.
4. If schema is recognized but validation fails, including a null or malformed nested readback, emit a sanitized fallback row with `recovery_projection_integrity=tamper_detected`.
5. If JSON parse fails or schema is unrecognized, treat the error as a legacy plain-text error and fall back safely with `recovery_projection_integrity=unavailable`.
6. Never expose the raw envelope JSON directly to operators; always use `operator_safe_summary` for valid envelopes and sanitized fallback text for invalid or legacy records.

Operator-facing P082 readback rows are allowlist-projected before they leave the DB accessor. Unknown keys are stripped, nested subcontracts are recursively allowlisted, and string fields are sanitized before display. Absolute Unix paths, `file://` values, run meta-root paths, provider transcripts, raw stderr, auth material, raw diagnostics, and unredacted command payloads must not pass through these lanes. URL separators such as `https://...` are not treated as filesystem paths, but token-like URL query parameters and key/value forms such as `token=...`, `api_key=...`, `password=...`, `secret=...`, and `authorization=...` are redacted and mark `diagnostic_redaction=partial`.

---

## Fail-Closed Side-Effect Behavior

Retry and release-side-effect operations are fail-closed. A retry or release command **must be rejected** while any of the following `side_effects.status` values exist for the run or target stage:

- `prepared`
- `executing`
- `externally_observed`
- `needs_reconciliation`
- `conflict`
- `unrecoverable`

The rejection writes a `p082_rejected_command_error_v1` envelope with `reason_code=requires_effect_reconciliation`. No side-effect ledger rows are modified during rejection. The operator must resolve the side-effect state through the dedicated reconciliation path before retry is permitted.

---

## Late-Output Quarantine Semantics

When a late output arrives from a superseded or cancelled execution:

1. The `artifact_source_generation_claims` row for the source generation must be set to `superseded` or `closed`.
2. The `agent_execution_runtime_facts.ignored_late_output_count` counter must be incremented.
3. The superseded source `work_items` row must be set to a terminal status (`completed` or `failed`).
4. The `active_projection_changed` field of `p082_late_output_settlement_v1` must be `false`.
5. Active artifact records, stage projections, reports, retry authorities, and side-effect state must not be modified.

---

## Approval Restart Semantics

When the daemon restarts while a human approval is pending:

1. Restore the `approvals` row to its existing `decision=pending`, `decided_at=null` state.
2. Restore the `approval_inbox` entry so operators can see the pending approval.
3. Do **not** synthesize an approval or rejection decision.
4. Do **not** advance the orchestrator past the approval gate.
5. Emit `reason=approval_pending_operator_action_required` in the readback.

---

## Cancellation Semantics

Cancellation must be convergent. After cancellation settles:

- `runs.cancel` requires an `idempotency_key`. Replaying the same key resolves to the existing cancellation command settlement. A later request with a different key after terminal cancellation is an idempotent no-op over the already-settled terminal state, not a second active cancellation mutation.
- No duplicate work items, owners, side effects, or provider subprocess sessions may exist.
- `cancellation_settlement_log` must contain exactly one action entry per settled item.
- Each settlement entry must carry a non-empty `action_id`. For cancellation readbacks (P082-R11 through P082-R14), `source_identifier` must either equal that `action_id` or, when a single action settles parallel agent executions, use the unique composite form `action_id:agent_execution:{agent_execution_id}`.
- Provider subprocess cleanup must be evidenced through `session_generations` terminalization and `session_events` records.
- Active stage projections, artifact links, and reports must reflect the cancelled state.
- Pending approval decisions are preserved as `pending`; they are not synthesized.
- Unresolved side-effect rows are held for reconciliation; cancellation does not clear them.

---

## Startup Requeue Exhausted Held State

One requeue generation is allowed per source work item. The behavior is:

1. **First requeue**: write `startup_repairs.id=p082-requeue:{cj.id}:{wi.id}:1` and enqueue a replacement work item with `requeue_generation=1`.
2. **Second observation** of the same source (non-replay): hold without enqueuing. Set `scenario_status=held` and emit `reason_code=startup_requeue_exhausted` with a non-null `recovery_operator_message` naming the clearance path.
3. Clearance is through the existing recovery inspection path or cancellation; there is no auto-clearance.
4. Replay of an existing idempotency key (crash resume) is not counted as a second requeue.

Observability threshold for the held state: warn at 0 s (immediate), critical at 300 s.

---

## Cancel-Then-Late-Output

When a cancelled provider session emits a late output after cancellation is settled:

1. Classify the output as a late output from a cancelled provider.
2. Quarantine the output; do not update active artifacts, `state/run-state.json`, `artifacts/active-index.json`, or reports.
3. Set `p082_late_output_settlement_v1.cancelled_provider_session=true`.
4. Confirm `active_projection_changed=false`.
5. The source `work_items` row must already be in a terminal state from cancellation settlement.

---

## Provider Subprocess Cleanup Proof

Provider subprocess cleanup evidence must be tied to durable `session_generations` terminalization records and `session_events` evidence. Cancellation readback `source_identifier` remains the settlement `action_id` (or the parallel-settlement `action_id:agent_execution:{agent_execution_id}` composite); the cleanup proof is carried by the durable rows and tests. For scenarios that require subprocess cleanup (P082-R11, P082-R15), proof must cite:

- The `session_generations` row showing the terminal `end_reason`.
- The `session_events` row confirming the ACP transport lifecycle close event.

---

## Swift/macOS Boundary

The app-local `RecoveryCoordinator.swift` is **not** the recovery-matrix authority. The matrix is implemented in the Rust control-plane engine and exposed through the MCP northbound surface. The Swift app reads recovery readbacks as projections; it does not own recovery decisions.

Any later UI integration requires a separate proposal. Prerequisites include:

- Reason-code display names and severity levels
- `ForgeStatusColor` mappings for each reason code
- Accessibility labels and text-scaling rules
- Xcode grace surface handling
- Empty, null, and unavailable state rendering
- `recovery_next_action` display limits (truncation, line limits)
- Redaction behavior per reason code
- Singular vs. plural field usage rules per UI context
- `MainActor` routing for recovery state updates

---

## Long-Held Observability Thresholds

| State | Warning (s) | Critical (s) | Operator Message |
|---|---|---|---|
| pending approval | 86400 | 259200 | "Approval has been pending for more than the expected review window." |
| side-effect reconciliation hold | 3600 | 14400 | "Side-effect reconciliation is blocking retry or cancellation settlement." |
| startup repair hold | 900 | 1800 | "Startup recovery remains held after the expected repair window." |
| Xcode startup grace | 720 | 900 | "Xcode startup grace exceeded the 12 minute window; inspect Xcode broker/session startup." |
| `startup_requeue_exhausted` | 0 | 300 | "Startup requeue exhausted and requires operator clearance through existing recovery inspection paths." |

The code owner for the P082 startup grace values is `control-plane/crates/domain/src/recovery_matrix.rs`: `STANDARD_STARTUP_GRACE_SECONDS`, `XCODE_STARTUP_GRACE_SECONDS`, `XCODE_STARTUP_GRACE_WARN_SECONDS`, and `XCODE_STARTUP_GRACE_CRITICAL_SECONDS`.

---

## Dependency Audit Pinned Risk

The retained `proposal-082` gate records dependency-audit availability as a gate-owned concern. When `cargo audit` or `cargo deny` is available in the local Rust toolchain, operators run it before sign-off. If those tools are unavailable in the execution environment, the accepted pinned-risk item is the workflow parser dependency chain `serde_yaml` -> `unsafe-libyaml`: it remains present because workflow and agent catalog files are YAML contracts, and replacement requires a separate parser migration slice with fixture parity. This pinned-risk acceptance stays visible in this document until advisory monitoring or a replacement parser lands.

---

## Gate Ownership

Gate aliases: `proposal-082`, `p082`

Owner: `scripts/test-gate.sh`

Active tests for gate passage:

- DB repository tests for each of the 17 matrix rows
- Engine P082 tests for each of the 17 matrix rows
- MCP readback test for each recovery reason code in the vocabulary
- Regression fixture for ACP startup stale repair
- Regression fixture for retry identifier guidance
- Crash/replay idempotency test for P082-R15
- Review regressions for Operator-only `runs.start`, Operator-only root-backed `ideas.create`, symlink-safe `artifact_root`/meta-root creation, `runs.get.escalation_readback`, and InvokeAgent completion ownership validation
- Focused auth/revocation regressions for live-principal source revalidation, MCP HTTP live revocation, and daemon failed-serve revocation
- Pinned-risk documentation for `serde_yaml`/`unsafe-libyaml` when dependency-audit tooling is unavailable
- `p082_recovery_matrix_gate_result_total{scenario_id,status}` is gate-harness telemetry and must be emitted by the retained DB harness test after scenario assertion groups. Runtime readback accessors emit readback and state-age telemetry, not gate-result counts.
- The P082 Python static checklist runs inside `scripts/test-gate.sh` before the focused Rust suites. It validates the reference matrix, positive fixture, all 16 negative fixtures, source-wiring expectations, metric ownership, and required proof-test names. Provider cleanup proof remains required by the matrix semantics and is covered by focused Rust tests.

Recovery proposals and implementation PRs add or update matrix rows before the behavior change lands. PRs that change recovery behavior without a corresponding matrix row update fail the retained `proposal-082` gate.
