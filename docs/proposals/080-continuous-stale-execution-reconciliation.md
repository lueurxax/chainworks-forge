# Proposal 080: Continuous Stale Execution Reconciliation

| Field | Value |
|---|---|
| Date | 2026-04-30 |
| Status | Draft |
| Author | Codex |
| Depends on | P037 ACP supervision, P058 claim/start ownership, P061 scheduler/write coordination, P065 retry instructions, P076 auto-retry observation ledger |
| Related | P051 Xcode MCP bridge pool, [local persistence write-budget contract](../reference/rust-control-plane.md#sqlite-write-serialization-and-gateway-dbwriter), P078 side-effect reconciliation, `docs/reference/rust-control-plane.md`, `docs/reference/session-lineage-reuse-and-operator-reset.md` |
| Scope | Add a continuous reconciliation subsystem for stale running execution truth, provider/session ownership, and helper-process lifecycle. |
| Non-goal | No automatic human approval, no blind retry of release side effects, and no replacement for existing startup repair or provider idle supervision. |

## 1. Problem

Chainworks already has startup repair for crash recovery, and the executor has provider idle supervision for active ACP prompts. That leaves a gap between those systems:

1. a work item is claimed and marked `running`;
2. an agent execution and session generation are created;
3. provider startup, session startup, or helper setup exits or stalls before normal prompt telemetry exists;
4. the durable tables still say `running`;
5. the scheduler refuses to claim more work because capacity is occupied by a non-useful execution.

The operator sees "running" but there is no useful provider process, no prompt progress, and often no actionable failure. Manual retry then creates more provider/Xcode startup attempts and can multiply permission modals.

Startup-only repair is not enough because the daemon may stay alive while the provider child dies, detaches, fails before session/new, or leaves no `runtime_invocation`/prompt-progress truth.

## 2. Current Baseline

Implemented pieces today:

- `RecoveryService::run_startup_repair()` runs at daemon startup.
- P061/P058 repair can requeue abandoned running `InvokeAgent` work at startup.
- P037 supervises idle ACP prompt activity once a prompt is active.
- P051 keeps Xcode MCP broker warm and classifies modal stalls.
- P076 observes repeated blocked/retry signatures.
- A narrow local hotfix repairs stale ACP startup rows where:
  - `InvokeAgent` is `running`;
  - the linked `session_generation` is `active`;
  - `provider_session_id` is still absent;
  - no session activity has been recorded after the startup grace period.

Missing pieces:

- no durable in-memory-to-SQL ownership map for currently executing work items;
- no periodic reconciliation of running work against live task/session/helper ownership;
- no unified stale classification across ACP startup, prompt, helper, scheduler, and release lanes;
- no MCP/operator readback showing why a running item is considered useful or stale;
- no metrics for stale-running detection, repair action, and repeated repair loops.

## 3. Goals

- Continuously reconcile non-terminal runs while the daemon is live, not only at startup.
- Distinguish useful running work from stale running truth.
- Repair stale scheduler capacity without killing long active prompts.
- Make helper processes and provider sessions owned by durable lease/session records.
- Surface clear operator readback: `useful_work_active`, `startup_stalled`, `prompt_stalled`, `helper_orphaned`, `needs_operator`, `needs_effect_reconciliation`.
- Feed P076 with typed stale signatures so auto-retry can avoid noisy blind retries.
- Keep release side-effect stages fail-closed and route them to P078 reconciliation.

## 4. Non-Goals

- Do not auto-approve human gates.
- Do not retry release/publish/git side effects while P078 reports unresolved effects.
- Do not use GraphQL mutations for repair.
- Do not infer useful work from UI projection freshness alone.
- Do not kill arbitrary user processes outside Chainworks-owned lease/session ownership.
- Do not add broad wall-clock prompt limits for active agents that are still producing progress.

## 5. Stale Classes

### 5.1 ACP Startup Stale

A running `InvokeAgent` is stale when all are true:

- linked session generation is `active`;
- no `provider_session_id`;
- no `last_activity_at`;
- startup age exceeds configured grace;
- no in-memory executor ownership confirms a live startup future.

Action: invalidate session generation, cancel/supersede the preclaimed agent execution, requeue the work item with `startup_repair_stale_acp_startup`, refresh scheduler projections, and emit P076 observation.

### 5.2 ACP Prompt Stale

A running prompt is stale only when:

- provider session exists;
- no prompt progress or tool activity has occurred past the P037 idle policy;
- the runtime manager cannot prove a live session handle.

Action: use the existing P037 failure classification and retry path. Do not use hard wall-clock limits while progress continues.

### 5.3 Scheduler Ownership Drift

A work item is stale when:

- `work_items.status = running`;
- there is no live executor task ownership record for that work item;
- the linked agent execution is still `running`;
- the work kind is retryable and has no unresolved side-effect ledger entry.

Action: repair through the same transaction shape as startup repair, not direct DB patching.

### 5.4 Helper Orphan Drift

A helper process is stale when:

- it is Chainworks-owned by recorded lease/session/process-group metadata;
- no active lease/session references it;
- TTL or idle grace has elapsed.

Action: terminate only the owned process group, record helper cleanup evidence, and update health/readback counters.

### 5.5 Release Side-Effect Drift

Release work is stale only after checking P078 side-effect ledger state.

Action: block retry with `requires_effect_reconciliation` until MCP reconciliation settles the effect.

## 6. Architecture

### 6.1 Execution Ownership Registry

Add a daemon-local registry keyed by `work_item_id`:

- claimed-at timestamp;
- executor task id;
- agent execution id;
- session generation id;
- provider;
- stage/run ids;
- last useful activity timestamp;
- helper lease ids;
- shutdown token.

Persist a bounded heartbeat/projection row for readback. The registry is not canonical truth, but it is the live ownership witness used by reconciliation.

### 6.2 Reconciliation Loop

Run a low-frequency reconciliation loop while the executor is idle or on a bounded interval:

1. load non-terminal runs and running work items;
2. join agent execution, session generation, helper leases, and side-effect ledger state;
3. classify each running row;
4. apply only the smallest safe repair action;
5. refresh scheduler health/projections;
6. emit typed P076 observations.

The loop must be write-budget aware and compatible with the implemented spooling contract.

### 6.3 Repair Transactions

All repairs must use engine/domain transitions or existing repository repair functions:

- no raw operator DB patching;
- no deleting rows;
- no losing old execution evidence;
- no retrying release side effects without P078 clearance.

### 6.4 Operator Readback

MCP readback should show, per running item:

- `running_truth`: `useful`, `stale_suspected`, `stale_repaired`, `needs_operator`, `needs_reconciliation`;
- `stale_class`;
- `last_useful_activity_at`;
- `live_owner_present`;
- `session_generation_status`;
- `provider_session_id_present`;
- `helper_lease_count`;
- `repair_action`;
- `next_retry_or_backoff_time`.

GraphQL may expose read-only projection fields for UI diagnostics, but repair remains MCP/engine-owned.

## 7. Configuration

Initial defaults:

```text
ACP startup stale grace: 180 seconds
Reconciliation interval: 30 seconds while daemon is live
Helper idle grace: 10 minutes
Max stale repairs per loop: 10
Repeated repair cooldown per work item: 5 minutes
```

Provider prompt idle behavior remains owned by P037 and must not become a hard wall-clock prompt timeout.

## 8. Metrics

Emit bounded-cardinality counters:

- `stale_execution_detected_total{class,provider,work_kind}`;
- `stale_execution_repaired_total{class,action}`;
- `stale_execution_repair_failed_total{class,reason}`;
- `live_execution_owner_missing_total{provider}`;
- `helper_orphan_reaped_total{helper_kind}`;
- `release_retry_blocked_for_reconciliation_total`.

Expose health snapshot fields:

- running work count;
- live owner count;
- stale suspected count;
- stale repaired last loop;
- helper orphan count;
- oldest stale age.

## 9. Tests and Gates

Add or extend `proposal-080` gate with deterministic fixtures:

- running `InvokeAgent` with active session generation and no provider session becomes requeued after startup grace;
- active prompt with recent progress is not repaired;
- running work with live owner registry is not repaired;
- missing owner registry plus no side-effect ledger is repaired;
- release work with unresolved side-effect ledger is not retried and returns `requires_effect_reconciliation`;
- helper process is reaped only when Chainworks-owned lease metadata proves ownership;
- repeated reconciliation is idempotent and respects cooldown;
- readback/projection shows stale class and repair action.

## 10. Rollout

1. Ship detection-only readback and metrics.
2. Enable repair for ACP startup stale class.
3. Enable scheduler ownership drift repair for non-release work.
4. Enable owned helper process reaping.
5. Wire P076 auto-retry decisions to stale class observations.
6. Keep release reconciliation behind P078 closeout.

## 11. Acceptance Criteria

- A live daemon cannot keep scheduler capacity occupied indefinitely by a running work item with no live owner and no useful provider/session activity.
- Long prompts that continue producing progress are not interrupted by wall-clock duration.
- Xcode MCP modal stalls do not create retry storms or batches of simultaneous modal prompts.
- Startup repair and live reconciliation produce the same durable repair semantics for equivalent stale states.
- Operator readback explains why each running item is useful, stale, repaired, or waiting for a real human gate.
- All repair actions are auditable and do not delete implementation artifacts or run-owned worktree evidence.
