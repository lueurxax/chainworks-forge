# Proposal 090: Retry Authority Payload Target Invariants and Recovery

| Field | Value |
|---|---|
| Date | 2026-05-19 |
| Status | Draft |
| Author | Codex |
| Depends on | P045 run recovery and granular retry MCP tools, P076 auto-retry observation ledger, P080 continuous stale execution reconciliation, P082 recovery/retry state-machine test matrix, P083 execution-truth ownership invariants, P088 code-writer completion receipts |
| Related | P037 ACP supervision, P079 contract-aware output repair/provider fallback, `control-plane/crates/db/src/repos/work_items.rs`, `control-plane/crates/engine/src/orchestrator.rs`, `control-plane/crates/engine/src/recovery.rs` |
| Scope | Make retry work-item payloads unambiguous, prevent stale source-target mismatches after auto-contract retries, and recover valid completed retry attempts that are stranded as running work. |
| Non-goal | No weakening of output contracts, no blind retry policy, no manual SQL repair path, no provider-specific workaround, and no broad rewrite of work queue or retry authority. |

---

## 1. Problem

P087 exposed a retry settlement failure where useful implementation work completed, all required outputs validated, and the run still remained `running`.

The immediate failure was:

```text
advance_run_source_target_mismatch:
source invoke auto-contract-output-retry:5be16ceb-...:204dcc2c-...
stage cf5c7cfa-... does not match authority target 5be16ceb-...
```

The run state after the failure was contradictory:

- stage `state_10_implementation_refined` / execution `5be16ceb-3027-4130-9b5b-aac95d67b727` remained `running`;
- invoke work item `auto-contract-output-retry:5be16ceb-3027-4130-9b5b-aac95d67b727:204dcc2c-bc0f-4b5e-8cc7-c04f5f5f43dd` remained `running`;
- retry authority `p091-retry-authority:5be16ceb-3027-4130-9b5b-aac95d67b727` remained `active`;
- agent execution `caf13a63-a79c-4f17-8923-dbb1ba15fa03` completed successfully with `valid_outputs_from_completed_execution`;
- the P088 receipt had `terminal_response_status=completed`, `completion_mode=acp_final_text_chainworks_output`, `fresh_required_output_count=4`, and `missing_required_output_count=0`.

This is not a Claude runtime issue and not a slow implementation issue.
It is an engine/retry-authority contract issue: the retry invocation succeeded, but settlement used stale provenance fields from an older failed retry as if they described the current retry target.

## 2. Evidence Baseline

The P087 auto-contract retry work item contained multiple conflicting identities:

| Payload field | Value | Meaning |
|---|---|---|
| `stage_execution_id` | `5be16ceb-...` | current retry target stage execution |
| `target_stage_execution_id` | `cf5c7cfa-...` | stale top-level field inherited from a previous retry |
| `retry_authority_id` | `p091-retry-authority:5be16ceb-...` | current retry authority |
| `p058_claimed.agent_execution_id` | `caf13a63-...` | actual completed current agent execution |
| `targeted_retry.source_stage_execution_id` | `cf5c7cfa-...` | provenance: failed source stage that caused fallback |
| `targeted_retry.source_agent_execution_id` | `204dcc2c-...` | provenance: failed source agent that caused fallback |
| top-level `source_agent_execution_id` | `b9073671-...` | older stale source field from cloned payload |

`build_post_invoke_advance_payload_tx` currently accepts targeted retry provenance fields while deriving the stage that must match the active retry authority.
When auto-contract retry clones an older targeted retry payload, stale top-level fields can survive and poison completion.

The current code already tries to fail closed when a targeted invoke does not match its active authority.
That is correct in principle, but the source of truth is wrong for this case.
The current invocation's target must be authority/current-stage driven; source failed executions must remain provenance only.

## 3. Root Cause Model

The bug has three structural causes.

### 3.1 Payload fields do not separate current routing from provenance

The work item payload uses fields such as `target_stage_execution_id`, `source_agent_execution_id`, `stage_execution_id`, `targeted_retry.source_*`, and `p058_claimed.agent_execution_id` without a strict ownership contract.

For a targeted retry, two identities must coexist:

1. the current invocation and target stage that should settle;
2. the older failed invocation that explains why the retry exists.

Today those identities can be mixed during payload cloning and settlement.

### 3.2 Auto-contract retry clones source payloads without clearing stale routing

`schedule_auto_contract_output_retry_for_stage` reuses the source invoke payload and overwrites some fields.
It removes `p058_claimed`, but it does not clear every stale routing/source field before writing the new retry identity.

That allows a newly created retry work item to carry both:

- current `stage_execution_id = new_stage.id`;
- stale `target_stage_execution_id` or top-level `source_agent_execution_id` from an older retry.

### 3.3 Recovery treats live work item state as stronger than terminal agent truth

P091-style recovery can identify active retry authority and stale running work, but it can exclude the case as a live work item even when a completed agent execution with valid required outputs already exists for the target stage.

Once the agent has terminal valid output for the current target, the work item is no longer meaningfully live.
The system needs a repair transition for:

```text
completed agent execution
+ valid required outputs
+ running invoke work item
+ active retry authority for the same target stage
```

## 4. Decision

Introduce a strict retry payload identity invariant:

> Top-level routing fields describe the current invocation and current retry target only. `targeted_retry.source_*` describes provenance only and must never determine the current target stage during completion.

This proposal fixes both the forward path and recovery path:

1. sanitize retry payloads before enqueueing auto-contract fallback work;
2. make post-invoke advance payload construction authority/current-target driven;
3. add startup and continuous recovery for valid completed retry attempts stranded behind running work items;
4. expose typed diagnostics for future incidents.

## 5. Proposed Design

### 5.1 Retry payload identity contract

For every `invoke_agent` work item, define field authority as follows:

| Field | Required meaning |
|---|---|
| `run_id` | current run |
| `stage_id` | workflow state id for the current invocation |
| `stage_execution_id` | current target stage execution id |
| `target_stage_execution_id` | current target stage execution id when present; must equal `stage_execution_id` for targeted retry |
| `retry_authority_id` | active authority for the current target, when the invoke is authority-bound |
| `p058_claimed.agent_execution_id` | actual current agent execution claimed by this invoke work item |
| `targeted_retry.source_stage_execution_id` | provenance only: failed stage that caused this retry |
| `targeted_retry.source_agent_execution_id` | provenance only: failed agent that caused this retry |
| `targeted_retry.source_work_item_id` | provenance only: failed or superseded source work item |

Disallowed for newly enqueued targeted retry payloads:

- stale top-level `source_stage_execution_id`;
- stale top-level `source_agent_execution_id`;
- stale top-level `source_work_item_id`;
- stale `target_stage_execution_id` that does not equal the new target stage;
- any preexisting `p058_claimed`.

### 5.2 Auto-contract retry payload sanitization

Before `schedule_auto_contract_output_retry_for_stage` mutates a cloned source payload, it must clear all routing and settlement fields owned by the previous invocation.

Required clear list:

```text
p058_claimed
target_stage_execution_id
source_stage_execution_id
source_agent_execution_id
source_work_item_id
retry_authority_id
```

Then it must set the current routing fields explicitly:

```text
stage_execution_id = new_stage.id
target_stage_execution_id = new_stage.id
retry_authority_id = new_authority.id
targeted_retry.retry_authority_id = new_authority.id
```

The old failed attempt remains only under:

```text
targeted_retry.source_stage_execution_id
targeted_retry.source_agent_execution_id
targeted_retry.source_work_item_id
```

### 5.3 Post-invoke completion semantics

`build_post_invoke_advance_payload_tx` must use the following precedence:

1. if `retry_authority_id` is present, load the active authority by id;
2. authority target is the current target stage;
3. `stage_execution_id` and `target_stage_execution_id` must either match the authority target or be absent/backfilled;
4. `p058_claimed.agent_execution_id` is the current completed agent execution;
5. `targeted_retry.source_*` fields are copied into diagnostics/provenance only and never used to select the current target.

Mismatch handling:

- If an authority id is present and active, stale inherited `target_stage_execution_id` should produce a typed repairable diagnostic, not strand the invoke when the actual completed agent belongs to the authority target.
- If the actual completed agent belongs to a different stage than the authority target, fail closed with a typed mismatch.
- If no current completed agent can be established, keep existing fail-closed behavior.

### 5.4 Completion must not poison valid output settlement

When output settlement has already recorded:

```text
agent_execution.status = completed
runtime_facts.output_settlement = valid_outputs_from_completed_execution
runtime_facts.valid_required_outputs = true
```

for an agent execution on the authority target stage, the work item completion path must not leave the work item `running` solely because stale provenance fields disagree.

The correct behavior is:

1. complete the invoke work item;
2. enqueue a targeted `advance_run` with the authority target;
3. let normal stage advancement terminalize or continue the run;
4. terminalize the retry authority when the target stage reaches terminal state.

### 5.5 Recovery for stranded valid retry attempts

Add recovery logic for the P087 shape.

Candidate query:

- `work_items.kind = invoke_agent`;
- `work_items.status = running`;
- work item has active retry authority by `retry_authority_id`, active authority source invoke id, or target stage;
- target stage is `running`;
- there is a completed agent execution on the target stage;
- runtime facts show `valid_outputs_from_completed_execution` and `valid_required_outputs = true`.

Repair:

- sanitize/backfill the work item payload current routing fields;
- mark the invoke work item completed through the normal repository completion path;
- enqueue or repair `advance-after-invoke:<invoke_work_item_id>`;
- leave durable evidence in `last_error`/payload diagnostics only if repair had to correct stale fields;
- do not directly settle the stage unless the normal advance path is unavailable.

Recovery must run in startup repair and in the continuous stale reconciliation loop.

### 5.6 Readback and diagnostics

Add typed reason codes that appear in reports/readback:

- `retry_payload_stale_target_stage_repaired`
- `retry_payload_source_provenance_ignored_for_target`
- `valid_retry_invoke_completion_recovered`
- `retry_authority_target_agent_stage_mismatch`
- `retry_authority_missing_for_targeted_invoke`

Reports should expose:

- current target stage id;
- authority id and authority state;
- current completed agent execution id;
- provenance source agent/stage ids;
- whether any stale routing field was repaired.

### 5.7 Backward compatibility

Older work items can still contain stale top-level fields.
The new completion and recovery paths must tolerate them only when current authority and current completed agent truth agree.

The compatibility rule is:

> Stale payload fields may be repaired when they conflict with an active authority and a valid completed current agent. They may not authorize completion when current execution truth is absent or contradictory.

## 6. Implementation Plan

1. Add helper functions for retry payload identity:
   - sanitize cloned targeted retry payloads;
   - extract current target identity;
   - extract provenance identity;
   - verify current completed agent belongs to target stage.
2. Update auto-contract retry scheduling to clear stale routing fields before enqueue.
3. Update post-invoke advance payload construction to use authority/current target precedence.
4. Add typed diagnostics for stale field repair and hard mismatches.
5. Add recovery for stranded valid targeted retry invokes.
6. Add report/readback fields for repaired stale routing and current/provenance identity split.
7. Add regression fixtures from the P087 payload shape.
8. Add a proposal gate alias:

```text
proposal-090|p090
```

## 7. Tests

Required tests:

1. Auto-contract retry sanitization:
   - source payload contains stale `target_stage_execution_id` and top-level `source_agent_execution_id`;
   - enqueued retry payload has current `stage_execution_id`, current `target_stage_execution_id`, current `retry_authority_id`, and old source ids only under `targeted_retry.source_*`.
2. Post-invoke completion:
   - active authority targets new stage;
   - payload contains stale inherited top-level target;
   - `p058_claimed.agent_execution_id` belongs to current target and has valid outputs;
   - work item completes and targeted advance payload uses authority target.
3. Hard mismatch:
   - `p058_claimed.agent_execution_id` belongs to a different stage than the active authority target;
   - completion fails closed with `retry_authority_target_agent_stage_mismatch`.
4. Startup recovery:
   - running invoke work item, active retry authority, completed valid agent on target stage;
   - recovery completes invoke and enqueues/repairs post-invoke advance.
5. Continuous recovery:
   - same candidate shape is repaired without daemon restart.
6. Report readback:
   - diagnostics distinguish current target identity from provenance source identity.

## 8. Acceptance Criteria

P090 is complete when:

1. newly created auto-contract retry work items cannot carry stale top-level routing fields from a cloned source payload;
2. `targeted_retry.source_*` fields never determine the current target stage during completion;
3. a valid completed agent execution on the active authority target cannot be stranded behind a running invoke work item because of stale provenance fields;
4. startup and continuous recovery repair the P087 shape without manual SQL and without blind retry;
5. hard mismatches still fail closed when current agent execution truth contradicts the active authority target;
6. reports expose enough identity split detail for operators to see current target, actual completed agent, and source provenance separately;
7. the `proposal-090` gate proves the forward path, hard mismatch, and recovery path.

## 9. Operator Impact

After P090:

- operators should not need to retry a run when the current retry agent already completed with valid outputs;
- a stale ACP subprocess after terminal output becomes cleanup/recovery work, not evidence that implementation is still running;
- blocked-run triage can classify this family as `valid_retry_invoke_completion_recovered` or a typed hard mismatch instead of generic stale running state;
- auto-contract retries remain fail-closed for true identity contradictions while recovering stale cloned-payload artifacts.
