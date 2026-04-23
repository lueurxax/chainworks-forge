# Query projections and client consumption contract

This document is the implemented GraphQL read contract for the thin macOS client. It replaces Proposal 043 and the former P031 handoff artifact.

| Field | Value |
|---|---|
| Implementation status | Implemented |
| Readiness | Ready with Risks |
| Contract schema | `p043-read-contract-v1` |
| Gate | `./scripts/test-gate.sh proposal-043` |
| Alias | `./scripts/test-gate.sh p043` |
| Scope | Rust control-plane GraphQL **read** contract for P031 client consumption. Command/control (MCP mutations) is explicitly NOT part of this contract. |
| Downstream owner | P031 thin macOS UI rewrite (**read-only** consumer, per r8 scope narrowing — see below). |

### Scope boundary (r8 correction)

P031's thin macOS UI is a **read-only consumer** of the GraphQL surface defined here. It renders run / stage / artifact / report / approval / health state and maintains freshness annotations. It does **NOT** issue MCP mutations. MCP command/control — `runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, `ideas.create` — lives on the operator-facing MCP surface and is invoked directly by operators (via MCP tools in Claude Code / Claude Desktop / scripted clients) or by a future UI proposal. A client that issues MCP mutations ("command UI") is out of scope for P031 and for this reference document; such a client would consume both this read contract AND the separate MCP command surface.

Wherever this document mentions "controls", "actions", "mutations", or "command completion" it refers to the generic client / operator system — not to P031's read-only thin UI. For P031 those rules apply vacuously: no mutation surfaces to enable/disable, no command-completion refresh to perform. Rules that explicitly pin behavior to P031 (freshness rendering, read-only evidence, subscription consumption) remain in force for the thin UI.

The remaining risk scoped to P031 is downstream read-side UI behavior: consuming freshness fields and rendering live/degraded/stale/unavailable/unauthorized states correctly. The server-side GraphQL contract and focused gate are implemented.

## Core rules

GraphQL is the macOS client read plane. MCP remains the operator command/control plane.

The macOS UI must not use MCP read helpers, SwiftData, local workflow compiler output, local recovery coordinators, filesystem artifact scans, or raw report files as alternate sources of workflow truth. If a visible field or action decision is not available through GraphQL, the surface must be disabled or deferred until the server publishes that fact.

The client must not infer:

- next stage;
- terminality;
- approval truth;
- retry, recovery, or reset legality;
- artifact or report hierarchy;
- validation failure availability;
- runtime/session state;
- projection freshness.

## Implemented read surfaces

| Surface | GraphQL entrypoint | Server owner / source | Status | Client rule |
|---|---|---|---|---|
| Runs home | `runs(ideaID:)` and `runs` | `db::repos::projections::{list_by_idea_projection,list_active_projection}` | Implemented | Render run list from projection-backed `GqlRun`; do not compute state from local rows or files. |
| Run detail | `run(id:)` with projection enrichment | Canonical run row enriched by `db::repos::projections::find_run_projection` | Implemented | Use projection-backed counters and summaries; show projection lag when projection truth is missing or stale. |
| Stage list / progress | `stages(runID:)` | `db::repos::projections::list_stages_projection` | Implemented | Use projection-owned decision flags; disable dependent actions when `projectionLag` is true. |
| Stage detail | `stage(id:)` plus `agentExecutions(stageExecutionID:)` | Canonical stage row enriched by stage summary projection and agent execution readback | Implemented | Use server-owned stage flags and execution truth; do not compute retry/reset/resume eligibility in Swift. |
| Approval inbox | `approvalInbox` | `db::repos::projections::list_pending_inbox_projection` | Implemented | Render pending approvals from projection truth. Resolution (`approvals.resolve`) is an operator-side MCP action on a separate surface; P031's thin UI renders the inbox read-only. |
| Artifact viewer | `artifacts(runID:)` | artifact index projection / `db::repos::projections::list_artifacts_projection` | Implemented | Browse the server artifact hierarchy only; direct file open/export may happen only after server selection. |
| Scheduler health | `schedulerHealthSummary` | `scheduler_health_snapshots` projection | Implemented | Render system-wide capacity, pressure, and latency health. |
| Startup recovery | `startupRecoverySummary` | `startup_recovery_readbacks` projection | Implemented | Render startup recovery progress, counts, and backpressure state. |
| Command latency | `commandLatencySummary` | `scheduler_health_snapshots` projection | Implemented | Render p95 latency for operator commands (approve, retry, cancel). |
| DB contention | `dbWriterContentionSummary` | `scheduler_health_snapshots` projection | Implemented | Render SQLite write wait p95 and transaction contention. |
| Provider capacity | `activeExecutionCountsByProvider` | `agent_executions` active counts | Implemented | Render active execution counts per canonical provider family. |
| Global queue depth | `oldestQueuedAge` and `queuedBackpressuredCountsByProviderAndReason` | `scheduler_queue_summaries` | Implemented | Render system-wide oldest queued item age and counts by reason. |
| Run/Stage queue | `runQueueSummary(runID:)` and `stageQueueSummary(stageExecutionID:)` | `scheduler_queue_summaries` projection | Implemented | Render queued/backpressured work counts and reasons. |
| Queue position | `queuePositionHint` | `scheduler_queue_summaries` | Implemented | Render non-ETA position hint for queued work. |
| Host interruption | `hostInterruptionEpochs` and `hostInterruptionAffectedExecutions` | `host_interruption_epochs` / `affected_executions` | Implemented | Render host sleep/wake and network migration history and impact. |
| Report viewer | report metadata through `artifacts(runID:)`; dedicated report payload query remains future work | artifact/report projection and future payload owner | Partial | Report metadata can render; payload rendering stays disabled unless a server-owned GraphQL payload path exists. |
| Daemon lifecycle | `daemonStatus` and `daemonStatusChanged` | [local-daemon-lifecycle-supervision-and-packaging.md](local-daemon-lifecycle-supervision-and-packaging.md) | Implemented | Render daemon live/degraded/failed/unavailable state from the lifecycle read model; do not infer lifecycle state from arbitrary request failures. |
| Experiment comparison | future comparison read query | future comparison/report owner | Deferred | Keep comparison disabled or placeholder-only. |

## Projection freshness fields

Run and stage read payloads expose projection freshness explicitly. This prevents the client from mistaking missing projection rows for real zero/false truth.

| GraphQL type | Fields | Semantics | Focused proof |
|---|---|---|---|
| `GqlRun` | `projectionPresent`, `projectionUpdatedAt`, `projectionLag` | `projectionPresent=false` and `projectionLag=true` when no `run_summaries` row exists. `projectionLag=true` when the projection row exists but status diverges from the canonical run row. | `proposal_043_missing_projection_rows_are_explicit_lag_state`, `proposal_043_run_query_uses_projection_summary_fields`, `proposal_043_run_subscription_uses_projection_summary_fields` |
| `GqlStageExecution` | `projectionPresent`, `projectionUpdatedAt`, `projectionLag` | `projectionPresent=false` and `projectionLag=true` when no `stage_summaries` row exists. `projectionLag=true` when the projection row exists but status or attempt diverges from the canonical stage row. | `proposal_043_missing_projection_rows_are_explicit_lag_state`, `proposal_043_stage_queries_expose_projection_decision_flags`, `proposal_043_stage_subscription_uses_projection_decision_flags` |

Client behavior:

- Treat `projectionLag=true` as `projection_lag` freshness state.
- Display a projection-updating label for projection-derived fields.
- Disable actions that depend on projection-owned counters or decision flags until projection truth catches up.
- Never convert `projectionPresent=false` into normal zero/false UI truth.

## Freshness states

| State | Meaning | Client behavior |
|---|---|---|
| `live` | Initial query succeeded and refresh/subscription path is healthy. | Render normally; action enablement still depends on MCP capability and server action state. |
| `refreshing` | Last known server state is displayed while a refresh is in flight. | Show refresh indicator; do not mutate local workflow truth. |
| `refreshing_disconnected` | Last known server state is displayed during the subscription reconnect grace window. | Show reconnecting indicator; disable destructive/state-changing actions. |
| `stale` | Last refresh failed, reconnect grace expired, or daemon lifecycle reports degraded/offline while cached server state exists. | Mark data stale; disable destructive/state-changing actions unless a surface explicitly declares stale-safe behavior. |
| `projection_lag` | Canonical row or event indicates newer state than projection fields can prove. | Mark projection-derived fields as updating; disable actions that depend on missing projection flags. |
| `unavailable` | Daemon/read endpoint cannot be reached, or auth/session is invalid and no reliable server state exists. | Show unavailable state; disable MCP mutations; offer reconnect/diagnostics only. |
| `unauthorized` | Bearer auth/principal does not allow the read surface. | Show read authorization error; do not fall back to local storage. |

## Freshness budget

These values bind any client consuming this contract. P031's read-only thin UI must use them for stale-state rendering, retry cadence, and rollback checks. The `Stale/action-safety disable threshold` and `Cutover rollback threshold` rows that reference commands/mutations are NO-OPS for P031 (no commands to disable, no mutations to roll back) but remain normative for any command-UI consumer. Changes to these values are contract changes and must update the gate.

| Budget | Value | Required behavior |
|---|---:|---|
| Initial read timeout | 5 seconds | Show `unavailable` when no prior state exists, otherwise `stale`. |
| Command-completion refresh timeout | 3 seconds | Keep prior authoritative state plus pending receipt, then mark the surface `stale` if refresh does not land. |
| Foreground/reconnect refresh timeout | 5 seconds | Surface `refreshing`, then settle to `live`, `stale`, `unavailable`, or `unauthorized`. |
| Projection-lag grace window | 2 seconds | Show `projection_lag` and disable actions that depend on missing projection flags. |
| Subscription disconnect grace window | 10 seconds | Show `refreshing_disconnected` while reconnecting, then `stale` if the window expires. |
| Bounded polling interval without subscription | 5 seconds | Poll only visible surfaces; do not infer missing truth locally. |
| Bounded polling backoff | 5s, 10s, 20s, then 30s max | Keep stale/unavailable labels visible and fail closed on repeated refresh failure. |
| Stale/action-safety disable threshold | immediate | Disable destructive/state-changing controls for `refreshing_disconnected`, `stale`, `projection_lag`, `unavailable`, and `unauthorized` unless an action is explicitly stale-safe. |
| Cutover rollback threshold | 3 consecutive command-completion refresh timeouts or 2 minutes continuous `unavailable` | Hold or roll back P031 for the affected surface. |

## Refresh and subscription posture

| Trigger | Required behavior |
|---|---|
| View load/navigation | Execute the matrix entrypoint for the selected surface. |
| App foreground/reconnect | Refresh visible run, stage, approval, artifact, report, and health surfaces. |
| MCP command accepted (command-UI only; N/A for P031) | Keep previous authoritative read model plus pending receipt; refresh GraphQL before displaying new workflow truth. |
| Subscription event | Patch only fields covered by that event contract, or perform a bounded refresh. Do not infer unrelated state. |
| Subscription disconnect | Mark affected surfaces `refreshing_disconnected`; schedule bounded reconnect; transition to `live` if reconnect succeeds inside 10 seconds or `stale` if the grace window expires. |
| Query failure | Keep last known state as `stale` when available; otherwise show `unavailable` or `unauthorized` based on error class. |
| Projection rebuild lag | Mark projection-derived fields `projection_lag` until the projection-backed query returns consistent state. |

Recognized subscription names for this contract:

| Subscription | Payload contract | P031 consumption rule |
|---|---|---|
| `runStatusChanged(runID:)` | Projection-enriched `GqlRun` via `find_run_projection`, including `projectionPresent`, `projectionUpdatedAt`, and `projectionLag`. | May patch displayed run summary fields; refresh after command completion still required. |
| `stageStatusChanged(runID:)` | Projection-enriched `GqlStageExecution` via `list_stages_projection`, including `projectionPresent`, `projectionUpdatedAt`, and `projectionLag`. | May patch stage decision flags; controls remain disabled during `projection_lag`. |
| `approvalRequested` | Emits current approval row for the requested approval. | May update approval inbox; command completion refresh still required after decisions. |
| `approvalResolved` | Emits current approval row for the resolved approval. | May remove/update approval row; bounded refresh fallback remains valid if subscription is unavailable. |
| `schedulerBackpressureChanged` | Emits sustained-backpressure events when thresholds are crossed. | May trigger UI health alerts or banner changes. |
| `runtimeStatusChanged` | Broader runtime/adapter health event stream remains future work beyond the implemented daemon lifecycle stream. | Deferred for P031 adapter-health UI until a server-owned runtime-health contract is accepted. |

Missing subscription support is not a reason for the client to infer truth locally. It only changes the refresh strategy to bounded visible-surface polling.

## Freshness behavior evidence and limitations

P043 owns the read contract and server-published facts. P031 owns macOS UI timers, reconnect loops, and read-side freshness rendering. Command-completion refresh and disabled-control rendering apply to any command-issuing client that consumes this contract; P031's read-only thin UI has no commands to refresh or controls to disable, so those rows apply vacuously to P031 (they remain normative for any future command UI).

| Freshness behavior | P043 evidence | Consumer cutover rule |
|---|---|---|
| Initial query failure to `unavailable` or `stale` | Contract row: Initial read timeout = 5 seconds. | P031 must test visible read-surface initial-failure rendering. A command UI must additionally disable surfaces that depend on the same initial read. |
| Command-completion refresh timeout to `stale` | Contract row: Command-completion refresh timeout = 3 seconds. | Applies to a command-issuing client: must test accepted-command pending receipt plus stale transition before enabling follow-on mutations. P031 has no commands; vacuous. |
| Foreground/reconnect refresh timeout | Contract row: Foreground/reconnect refresh timeout = 5 seconds. | P031 must test foreground/reconnect refresh state settlement for its read surfaces. |
| Projection lag action safety | Contract row: Projection-lag grace window = 2 seconds. | A command-issuing client must disable actions that depend on projection flags until projection-backed queries catch up. P031 has no actions; vacuous. |
| Subscription disconnect action safety | Contract row: Subscription disconnect grace window = 10 seconds and state `refreshing_disconnected`. | A command-issuing client must disable destructive/state-changing actions during reconnect grace and mark `stale` after expiry. P031 renders the freshness state read-only; no actions to disable. |
| Bounded polling fallback | Contract rows: interval = 5 seconds; backoff = 5s, 10s, 20s, then 30s max. | P031 may poll only visible implemented surfaces and must not poll deferred surfaces. |
| Unauthorized read behavior | Executable proof: `proposal_043_graphql_reads_are_operator_only_v1`. | P031 must show read authorization error and never fall back to local storage. |
| Stale/action-safety disable threshold | Contract row: immediate disable for unsafe freshness states. | A command-issuing client must prove disabled controls for `refreshing_disconnected`, `stale`, `projection_lag`, `unavailable`, and `unauthorized`. P031 renders these as read-side badges/annotations on the affected surfaces — no controls to disable. |

## GraphQL field proof

| Surface | Proof | Result |
|---|---|---|
| Runs home | Projection-backed list query through `list_by_idea_projection` and `list_active_projection`. | Sufficient for P031 ship. |
| Run detail | `run(id:)` returns projection-enriched counters, summaries, `projectionPresent`, `projectionUpdatedAt`, and `projectionLag` from `find_run_projection`. | Sufficient for P031 ship. |
| Stage list / progress | `stages(runID:)` reads stage projection rows with decision flags, `projectionPresent`, `projectionUpdatedAt`, and `projectionLag`. | Sufficient for P031 ship. |
| Stage detail | `stage(id:)` returns projection-enriched decision flags and projection freshness while preserving canonical evidence/recovery payloads. | Sufficient for P031 ship. |
| Missing projection rows | Missing `run_summaries` or `stage_summaries` rows surface as `projectionPresent=false` and `projectionLag=true`, not normal zero/false truth. | Sufficient for P031 projection-lag rendering. |
| Run status subscription | `runStatusChanged(runID:)` emits projection-enriched run summary and freshness fields. | Sufficient for P031 event patching. |
| Stage status subscription | `stageStatusChanged(runID:)` emits projection-enriched stage decision and freshness fields. | Sufficient for P031 event patching. |
| Approval resolved subscription | `approvalResolved` emits current resolved approval rows. | Sufficient for P031 event patching. |
| Approval inbox | `approvalInbox` is projection-backed. | Sufficient for P031 ship. |
| Artifact viewer | `artifacts(runID:)` | is projection-backed. | Sufficient for P031 ship. |
| Scheduler health | `schedulerHealthSummary` returns global capacity, active counts, oldest queued age, and sustained backpressure state. | Sufficient for P031 health alerts. |
| Startup recovery | `startupRecoverySummary` returns recovered items, backpressured recovery counts, and affected runs. | Sufficient for P031 recovery UI. |
| Command latency | `commandLatencySummary` returns p95 latency for approve, retry, and cancel. | Sufficient for P031 diagnostics. |
| DB contention | `dbWriterContentionSummary` returns write wait p95 and transaction contention. | Sufficient for P031 diagnostics. |
| Provider capacity | `activeExecutionCountsByProvider` returns active execution counts per canonical family. | Sufficient for P031 capacity UI. |
| Queue summaries | `runQueueSummary`, `stageQueueSummary`, and `queuedBackpressuredCountsByProviderAndReason` are projection-backed. | Sufficient for P031 backpressure UI. |
| Queue position | `queuePositionHint` returns non-ETA position hint from projection truth. | Sufficient for P031 queue UI. |
| Host interruption | `hostInterruptionEpochs` and `hostInterruptionAffectedExecutions` are canonical readbacks. | Sufficient for P031 recovery UI. |
| Report viewer | Metadata-backed only; dedicated report payload path is not complete. | Partial proof only. |
| Runtime health | No P043-owned GraphQL health proof for thin-client use. | Deferred. |
| Experiment comparison | No current GraphQL query proof. | Deferred. |

## Projection parity

| Surface | Parity statement |
|---|---|
| Runs home | Projection parity is maintained through the run summary projection owner. |
| Run detail | Projection parity is maintained through canonical row plus run projection enrichment and explicit projection freshness fields. |
| Stage list / progress | Projection parity is maintained through stage summary projection rows and explicit projection freshness fields. |
| Stage detail | Projection parity is maintained through canonical row plus stage projection enrichment and explicit projection freshness fields. |
| Approval inbox | Projection parity is maintained through the inbox projection owner. |
| Artifact viewer | Projection parity is maintained through the artifact index projection owner. |
| Scheduler health | Projection parity is maintained through the health snapshot projection owner. |
| Startup recovery | Projection parity is maintained through the startup recovery readback projection owner. |
| Queue summaries | Projection parity is maintained through the queue summary projection owner. |
| Host interruption | Projection parity is not applicable; these are canonical readbacks of detected epochs. |
| Report viewer | Projection parity is partial and limited to metadata until report payload readback lands. |
| Runtime health | Projection parity is deferred to the lifecycle owner. |
| Experiment comparison | Projection parity is deferred to the future comparison owner. |

## Read principal policy

P043 V1 is operator-only for the production macOS client read path. Current GraphQL query resolvers are protected by bearer-authenticated route access and focused operator-only tests; observer and agent read expansion is deferred until query authorization and field redaction tests exist.

| Surface | MacOS client principal | Observer visibility | Agent visibility |
|---|---|---|---|
| Runs home / run detail | Operator | Deferred | Deferred |
| Stage list/detail | Operator | Deferred | Deferred |
| Approval inbox | Operator | Deferred | No |
| Artifact viewer | Operator | Deferred | Deferred |
| Report viewer | Operator | Deferred | Deferred |
| Runtime health | Operator | Deferred | Deferred |

Future non-operator read expansion must be explicit in auth/capability policy and covered by server-side query authorization/redaction tests.

## P031 consumption contract

P031 may ship these surfaces from this contract:

- Runs home;
- Run detail;
- Stage list / progress;
- Stage detail;
- Approval inbox;
- Artifact viewer.

P031 may ship report viewing only as a partial surface where missing payload readback is visibly annotated as unavailable. Runtime health and experiment comparison remain hidden or placeholder-only.

P031 owns the UI-side evidence for the **read-side** client contract:

- reconnect timers;
- live / refreshing_disconnected / stale / projection_lag / unavailable / unauthorized rendering on each surface (as badges or inline annotations, not disabled controls — P031 has no controls);
- no SwiftData / local-service fallback for workflow truth;
- subscription patching rules from the "Refresh and subscription posture" section.

P031 does NOT own:

- MCP command-completion refresh behavior (P031 issues no MCP mutations);
- disabled-action rendering for destructive commands (P031 has no action surfaces);
- "command receipt" displays (no commands);
- any `runs.start` / `runs.cancel` / `approvals.resolve` / `stages.retry` / `ideas.create` UI.

These belong to a future command-UI proposal or to operators invoking MCP directly.

## Gate and verification

The current proof lane is:

```bash
./scripts/test-gate.sh proposal-043
./scripts/test-gate.sh p043
```

The gate runs the focused `graphql-server` tests whose names start with `proposal_043_`, then validates this reference document from the repository root.

The test slice covers:

- projection-backed run queries;
- projection-backed stage queries;
- missing projection rows surfacing as `projectionPresent=false` and `projectionLag=true`;
- projection-enriched run and stage subscription payloads;
- `approvalResolved` subscription availability;
- operator-only V1 reads.

The gate fails closed when this reference document omits required surfaces, statuses, freshness budget rows, projection freshness fields, freshness behavior limitations, subscription posture, operator-only V1 policy, projection parity, known gaps, or cutover decision rules.

## Known holds

- Report payload rendering needs a server-owned GraphQL payload path before it can be a full thin-client surface.
- Adapter/runtime health beyond the daemon lifecycle read model remains deferred unless a future server-owned read surface publishes it.
- Experiment comparison has no current GraphQL read owner and stays deferred.
- P031 must still prove macOS UI consumption behavior before user-visible thin-client cutover.
