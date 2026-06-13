# Proposal 098: P080 Manual Operator Hold and Clear-Hold Semantics

| Field | Value |
|---|---|
| Date | 2026-06-02 |
| Status | Draft |
| Author | Codex |
| Depends on | [080-continuous-stale-execution-reconciliation.md](080-continuous-stale-execution-reconciliation.md), [081-boundary-first-api-auth-contract-matrix.md](081-boundary-first-api-auth-contract-matrix.md), [execution truth and recovery](../reference/execution-truth-and-recovery.md) |
| Related | P037 ACP supervision, P083 execution-truth ownership invariants, durable side-effect reconciliation, `docs/reference/rust-control-plane.md` |
| Scope | Define the governed manual hold, clear-hold, authorization, replay, readback, and audit contract for P080 stale execution reconciliation. |
| Non-goal | No automatic human approval, no SwiftUI mutation surface, no side-effect retry, and no replacement for P080's automatic stale classification or repair phases. |

## 1. Problem

P080 intentionally keeps `requested_action=hold` disabled. That is correct for the first backend rollout: stale execution reconciliation must prove detection, safe repair, and readback before operators can add durable manual holds.

The missing owner is still real. Operators sometimes need to stop automatic reconciliation of a specific stale-running tuple while investigating ambiguous ownership, side-effect state, provider leases, or repeated repair loops. Leaving that behavior implicit creates two bad outcomes:

- operators use ad-hoc DB patches or broad daemon restarts to freeze a case;
- future P080 implementation may add hold behavior without a stable event, authorization, replay, and clear-hold contract.

P098 defines the missing manual-hold lifecycle so P080 can continue to fail closed until this proposal is implemented.

## 2. Goals

- Add an operator-only MCP command to place a durable manual hold on a specific P080 reconciliation target.
- Add an operator-only MCP command to clear that hold when the root cause has been resolved.
- Make hold and clear-hold idempotent, journaled, auditable, and replay-safe.
- Keep P080 automatic repair disabled for held targets while preserving detection/readback.
- Expose hold state through MCP, GraphQL read-only projection, run reports, and release receipts when applicable.
- Preserve side-effect fail-closed behavior: manual hold cannot make release/publish/git/external-effect work retryable.
- Define steward-readable metrics so manual holds do not look like normal run progress or clean throughput.

## 3. Non-Goals

- Do not bypass human approval gates.
- Do not add GraphQL mutations for hold or clear-hold.
- Do not add SwiftUI buttons or app-side command execution.
- Do not let manual hold authorize a repair that P080 or side-effect reconciliation would reject.
- Do not hold an entire workspace or provider family in v1; the hold target is a bounded reconciliation tuple.
- Do not allow indefinite invisible holds. Every active hold must appear in readback with age, reason, and next operator step.

## 4. Hold Target

The hold target is the smallest stable tuple P080 can classify:

```text
run_id
stage_id
work_item_id
agent_execution_id
stale_class
projection_generation
```

The command may omit `agent_execution_id` only when P080 readback proves the target has no linked agent execution yet, such as early scheduler ownership drift. The engine records the resolved tuple before writing the hold.

The command must fail closed if:

- the run is terminal;
- the work item is no longer present;
- the target tuple no longer matches current P080 readback;
- the target is release/publish/git/external-effect work with unresolved side-effect ledger state and the requested action tries to mark it retryable;
- the caller lacks the operator capability required by P081 boundary policy;
- the current P080 rollout phase has manual hold disabled.

## 5. MCP Contract

Add two operator MCP tools:

```text
p080.hold_target.v1
p080.clear_hold.v1
```

`p080.hold_target.v1` requires:

- `schema_version = "p080_hold_request_v1"`;
- `run_id`;
- `stage_id`;
- `work_item_id`;
- optional `agent_execution_id`;
- `stale_class`;
- `expected_projection_generation`;
- `expected_predicate_hash`;
- `operator_reason`;
- `operator_request_dedup_key`;
- `idempotency_key`.

`p080.clear_hold.v1` requires:

- `schema_version = "p080_clear_hold_request_v1"`;
- `hold_id`;
- `expected_hold_generation`;
- `clear_reason`;
- `operator_request_dedup_key`;
- `idempotency_key`.

Both tools must:

- authorize before dedup lookup or durable writes;
- reject unknown fields and unknown enum values;
- enforce parser limits before durable writes;
- write command journal and audit log rows on success;
- return typed errors for disabled phase, stale predicate, missing capability, side-effect unsafe, expired dedup, and idempotency conflict.

## 6. Persistence

Add an append-only hold event table and a compact active-hold projection.

Minimum event fields:

- `event_id`;
- `hold_id`;
- `event_kind`: `hold_created`, `hold_cleared`, `hold_clear_rejected`, `hold_superseded_by_terminal_state`;
- target tuple fields;
- `stale_class`;
- `predicate_hash`;
- `rollout_generation`;
- `principal_id`;
- `principal_class`;
- `operator_reason_redacted`;
- `clear_reason_redacted`;
- `command_journal_id`;
- `dedup_key_hash`;
- `created_at`.

Minimum active projection fields:

- `hold_id`;
- target tuple fields;
- `hold_generation`;
- `hold_status`: `active`, `cleared`, `superseded`;
- `hold_reason_code`;
- `hold_age_seconds`;
- `next_operator_step`;
- `projection_generation`;
- `updated_at`.

Rows are never deleted as part of normal operation. Projection rebuild derives active holds from event truth.

## 7. Reconciliation Behavior

P080 detection continues while a hold is active. P080 repair does not.

For a held target:

- `p080_readback_v1.running_truth` becomes `needs_operator`;
- `hold_reason` becomes `manual_operator_hold`;
- `repair_action` becomes `held_by_operator`;
- `next_retry_or_backoff_time` is null unless a separate cooldown applies;
- `operator_message` points to the hold id and next safe action.

If the target reaches a terminal run/stage/work-item state, the daemon records `hold_superseded_by_terminal_state` and stops surfacing the hold as active. This does not require an operator clear.

## 8. Clear-Hold Semantics

Clearing a hold removes only the manual hold barrier. It does not force repair.

After clear:

1. P080 reclassifies the current target from live truth.
2. If the target is now useful, no repair runs.
3. If the target is still stale and the current rollout phase enables safe repair for that class, P080 may repair through its normal path.
4. If side-effect safety is unresolved, the target remains `needs_effect_reconciliation`.
5. If the tuple changed, clear-hold succeeds for the old hold, and the new tuple is classified independently.

## 9. Readback and Reporting

MCP diagnostics, GraphQL read-only projection, run reports, and release receipts must expose:

- active hold count;
- hold id;
- hold target tuple;
- hold age;
- operator-safe reason;
- principal class;
- clear eligibility;
- next operator step;
- whether the hold is blocking repair, release, or only diagnostic noise.

Readback must not expose bearer tokens, raw provider messages, raw prompt text, or unredacted operator notes.

## 10. Steward Expectations

Manual holds are operational interventions. Steward analysis must:

- count hold frequency by stale class and run;
- track time-to-clear;
- warn when holds remain active beyond configured age;
- exclude held duration from clean throughput baselines unless explicitly requested;
- flag repeated holds on the same class as a signal for a system fix or P080 rollout rollback.

## 11. Metrics

Emit bounded-cardinality metrics:

- `p080_manual_hold_created_total{stale_class,principal_class}`;
- `p080_manual_hold_cleared_total{stale_class}`;
- `p080_manual_hold_active_count{stale_class}`;
- `p080_manual_hold_age_seconds{stale_class}`;
- `p080_manual_hold_rejected_total{reason}`;
- `p080_repair_skipped_due_to_manual_hold_total{stale_class}`.

## 12. Tests and Gates

Add `proposal-098` / `p098` gate coverage:

- unauthorized hold rejects before dedup and durable writes;
- disabled rollout phase rejects with `action_disabled_in_phase`;
- stale predicate rejects without changing active hold projection;
- duplicate idempotency replays the same response;
- idempotency conflict rejects;
- active hold blocks P080 repair but not detection/readback;
- clear-hold re-enables normal P080 classification without forcing repair;
- terminal target supersedes active hold;
- side-effect target remains fail-closed after clear;
- projection rebuild restores active holds from events;
- GraphQL remains read-only and SwiftUI has no hold mutation path.

## 13. Rollout

1. Ship schema, event table, projection, readback, and disabled MCP responses.
2. Enable hold/clear-hold for detection-only P080 classes.
3. Enable hold/clear-hold for repair-enabled non-side-effect classes.
4. Keep side-effect and release lanes fail-closed permanently unless the side-effect ledger declares retry-safe through its own contract.

## 14. Acceptance Criteria

- Operators can place and clear a durable hold without raw DB mutation.
- Held targets remain visible and diagnosable.
- Held targets are not automatically repaired while the hold is active.
- Clearing a hold does not bypass P080 safety or side-effect reconciliation.
- Every successful hold and clear-hold has command journal, audit, metric, and readback evidence.
- Steward can distinguish manual intervention time from ordinary run progress.

