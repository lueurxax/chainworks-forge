# Query projections and client consumption contract

This document is the canonical implemented GraphQL read contract for the thin macOS client. It replaces Proposal 043 and the former thin UI handoff artifact as the operational source of truth. Future macOS UI proposals must consume this contract instead of depending on historical proposal text.

| Field | Value |
|---|---|
| Implementation status | Implemented |
| Readiness | Ready with Risks |
| Contract schema | `p043-read-contract-v1` |
| Gate | `./scripts/test-gate.sh proposal-043` |
| Alias | `./scripts/test-gate.sh p043` |
| Composed downstream gate | `./scripts/test-gate.sh p031` |
| Scope | Rust control-plane GraphQL read contract for thin macOS UI consumption. Command/control (MCP mutations) is explicitly NOT part of this contract, with the exception of the governed approval mutation path. See [ui-action-boundary.md](ui-action-boundary.md). |
| Current UI boundary | Thin macOS UI (read-side and human-gate mutation consumer over server-owned projections). |
| Stabilization owners | P032 for productization and honest operator dogfood; [macos-operator-navigation.md](macos-operator-navigation.md) for the implemented visual/navigation shell over this read model. |

## Thin UI Boundary

The original P043/P031 gate labels are retained for compatibility. The active content below is the current thin UI boundary:

The macOS thin UI is a **read-side consumer and human-gate resolver** of the GraphQL surface defined here. It renders run / stage / artifact / report / approval / health state and resolves pending approvals via governed GraphQL mutations. The short canonical action boundary lives in [ui-action-boundary.md](ui-action-boundary.md).

**PROHIBITED ACTIONS for governed macOS UI:**
- It does **NOT** issue MCP mutations.
- It does **NOT** use GraphQL mutations EXCEPT for the `approveApproval` and `rejectApproval` human-gate path.
- It does **NOT** use local workflow mutation fallback.
- It does **NOT** probe raw truth from SwiftData or filesystem (except for authorized artifact display).
- **Server parity readiness**: if parity readiness is shown in the UI, it must be consumed from a daemon-owned GraphQL read surface derived from runtime artifacts, never from direct file reads.

MCP command/control — `runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, `ideas.create`, `steward.run_analysis` — lives on the operator-facing MCP surface and is invoked directly by operators (via MCP tools in CLI / automation / scripted clients) or by separate follow-up transport proposals. A client that issues MCP mutations ("command UI") is out of scope for the governed thin UI and for this reference document.

Wherever this document mentions "controls", "actions", "mutations", or "command completion" it refers to the generic client / operator system, not to the governed read-only thin UI. For the current thin UI those rules apply vacuously: no mutation surfaces to enable/disable, no command-completion refresh to perform. Rules for freshness rendering, read-only evidence, subscription consumption, and projection ownership remain in force for all future UI work.

Downstream proposal rule: new macOS UI features must start from this thin UI boundary. They may add server-owned GraphQL read fields, projections, subscriptions, and read-side presentation state. They must not restore Swift-local workflow truth, UI MCP calls, GraphQL mutations, raw artifact scans as truth, or old local-orchestrator fallback paths unless a later approved write-transport proposal explicitly changes this boundary.

## Thin UI Schema Contract

Every visible governed workflow field must have one named GraphQL source or one explicit disabled/deferred state.

### Metadata and Freshness Fields

| Field | GraphQL Type | Semantics |
| --- | --- | --- |
| `freshnessState` | `FreshnessState` | `live`, `refreshing`, `projection_lag`, `stale`, `unavailable`, `unauthorized`. |
| `disabledReasonCode` | `DisabledReasonCode` | `WRITE_PATH_NOT_AVAILABLE`, `MANAGED_OUTSIDE_UI`, `AMBIGUOUS_APPROVAL_IDENTITY`, `STALE_READ`, `PROJECTION_LAG`, `UNAUTHORIZED`, `UNSUPPORTED_ACTION`. |
| `writePathState` | `WritePathState` | `available`, `read_only_diagnostic`, `write_path_not_available`, `external_transport_required`, `hidden`. |
| `diagnosticId` | `String` | Unique identifier for the row/approval/report used for external write workflows. |
| `payloadAvailabilityState` | `PayloadAvailabilityState` | `available`, `metadata_only`, `payload_deferred`, `generating`, `unavailable`. |
| `payloadUnavailableReasonCode` | `PayloadUnavailableReasonCode` | `PAYLOAD_DEFERRED_BY_P031`, `GENERATING`, `NOT_INDEXED`, `NOT_AUTHORIZED`, `NOT_AVAILABLE`, `UNKNOWN`. |
| `serverDebugDetail` | `String` | Internal debug information for diagnostic copy-paste; not for primary UI display. |

### Field Ownership Matrix

| Field | Source surface | Swift Owner |
| --- | --- | --- |
| `freshnessState` | run, stage, approval, artifact, report | `WorkflowFreshnessReducer` |
| `disabledReasonCode` | approvals and deferred action metadata | `DisabledReasonPresenter` |
| `writePathState` | approval rows | `ApprovalDiagnosticPresenter` |
| `diagnosticId` | approvals, reports | `ApprovalDiagnosticPresenter` |
| `payloadAvailabilityState` | report metadata | `PayloadUnavailableReasonPresenter` |
| `payloadUnavailableReasonCode` | report metadata | `PayloadUnavailableReasonPresenter` |
| `serverDebugDetail` | diagnostic extensions | `DiagnosticDetailsPresenter` |

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
| Runs home | `runs(ideaId:)` and `runs` | `db::repos::projections::{list_by_idea_projection,list_active_projection}` | Implemented | Render run list from projection-backed `GqlRun`; do not compute state from local rows or files. |
| Run detail | `run(id:)` with projection enrichment | Canonical run row enriched by `db::repos::projections::find_run_projection` | Implemented | Use projection-backed counters and summaries; show projection lag when projection truth is missing or stale. **P065 expansion**: includes compact retry-instruction provenance. |
| Retry authority recovery readback | `run(id:)` fields `retryAuthorityJson`, `retryAuthorityHistoryJson`, `p091OrphanRepairReadbackJson` | `retry_stage_execution_authorities`, `retry_payload_recovery_events`, and `p091_orphan_repair_passes` | Implemented | Render targeted retry authority, retry payload recovery diagnostics, nullable missing-authority history rows, and recovery counters from server-owned JSON readback only. |
| Stage list / progress | `stages(runId:)` | `db::repos::projections::list_stages_projection` | Implemented | Use projection-owned decision flags; disable dependent actions when `projectionLag` is true. |
| Stage detail | `stage(id:)` plus `agentExecutions(stageExecutionId:)` | Canonical stage row enriched by stage summary projection and agent execution readback | Implemented | Use server-owned stage flags and execution truth; do not compute retry/reset/resume eligibility in Swift. **P065 expansion**: includes `retry_instruction` group and delivery status. |
| Approval inbox | `approvalInbox` | `db::repos::projections::list_pending_inbox_projection` | Implemented | Render pending approvals from projection truth. Resolution is supported via governed GraphQL mutations or operator-side MCP action (`approvals.resolve`). |
| Artifact viewer | `artifacts(runId:)` | artifact index projection / `db::repos::projections::list_artifacts_projection` | Implemented | Browse the server artifact hierarchy only; direct file open/export may happen only after server selection. |
| Side-effect ledger | `unresolvedSideEffects` | `db::repos::side_effects::list_unresolved` | Implemented | Render unresolved side effects from projection truth. Resolution is MCP-only. |
| Scheduler health | `schedulerHealthSummary` | `scheduler_health_snapshots` projection | Implemented | Render system-wide capacity, pressure, and latency health. |
| Startup recovery | `startupRecoverySummary` | `startup_recovery_readbacks` projection | Implemented | Render startup recovery progress, counts, and backpressure state. |
| Command latency | `commandLatencySummary` | `scheduler_health_snapshots` projection | Implemented | Render p95 latency for operator commands (approve, retry, cancel). |
| DB contention | `dbWriterContentionSummary` | `scheduler_health_snapshots` projection | Implemented | Render SQLite write wait p95 and transaction contention. |
| Provider capacity | `activeExecutionCountsByProvider` | `agent_executions` active counts | Implemented | Render active execution counts per canonical provider family. |
| Global queue depth | `oldestQueuedAge` and `queuedBackpressuredCountsByProviderAndReason` | `scheduler_queue_summaries` | Implemented | Render system-wide oldest queued item age and counts by reason. |
| Run/Stage queue | `runQueueSummary(runId:)` and `stageQueueSummary(stageExecutionId:)` | `scheduler_queue_summaries` projection | Implemented | Render queued/backpressured work counts and reasons. |
| Queue position | `queuePositionHint` | `scheduler_queue_summaries` | Implemented | Render non-ETA position hint for queued work. |
| Host interruption | `hostInterruptionEpochs` and `hostInterruptionAffectedExecutions` | `host_interruption_epochs` / `affected_executions` | Implemented | Render host sleep/wake and network migration history and impact. |
| Report viewer | report metadata through `artifacts(runId:)`; dedicated report payload query remains future work | artifact/report projection and future payload owner | Partial | Report metadata can render; payload rendering stays disabled unless a server-owned GraphQL payload path exists. |
| Daemon lifecycle | `daemonStatus` and `daemonStatusChanged` | [local-daemon-lifecycle-supervision-and-packaging.md](local-daemon-lifecycle-supervision-and-packaging.md) | Implemented | Render daemon live/degraded/failed/unavailable state from the lifecycle read model; do not infer lifecycle state from arbitrary request failures. |
| Storage health | `storageHealth` | `db::repos::storage_health::storage_health_with_writer` | Implemented | Render current health state of the storage subsystem, including DbWriter, WAL, projections, evidence spool, and freshness details. |
| Experiment comparison | future comparison read query | future comparison/report owner | Deferred | Keep comparison disabled or placeholder-only. |

## Projection freshness fields

Run and stage read payloads expose projection freshness explicitly. This prevents the client from mistaking missing projection rows for real zero/false truth.

| GraphQL type | Fields | Semantics | Focused proof |
|---|---|---|---|
| `GqlRun` | `projectionPresent`, `projectionUpdatedAt`, `projectionLag`, `workflowConflict` | `projectionPresent=false` and `projectionLag=true` when no `run_summaries` row exists. `projectionLag=true` when the projection row exists but status diverges from the canonical run row. `workflowConflict` exposes current and historical conflict truth, including **lead mediation status, resolution mode, and confirmation subject linkage**. | `proposal_043_missing_projection_rows_are_explicit_lag_state`, `proposal_043_run_query_uses_projection_summary_fields`, `proposal_043_run_subscription_uses_projection_summary_fields`, `proposal_017_workflow_conflict_readback` (retained historical alias) |
| `GqlStageExecution` | `projectionPresent`, `projectionUpdatedAt`, `projectionLag` | `projectionPresent=false` and `projectionLag=true` when no `stage_summaries` row exists. `projectionLag=true` when the projection row exists but status or attempt diverges from the canonical stage row. | `proposal_043_missing_projection_rows_are_explicit_lag_state`, `proposal_043_stage_queries_expose_projection_decision_flags`, `proposal_043_stage_subscription_uses_projection_decision_flags` |

Client behavior:

- Treat `projectionLag=true` as `projection_lag` freshness state.
- Display a projection-updating label for projection-derived fields.
- Disable actions that depend on projection-owned counters or decision flags until projection truth catches up.
- Never convert `projectionPresent=false` into normal zero/false UI truth.

## Storage Health Fields

The `storageHealth` query returns a `GqlStorageHealth` object, providing a comprehensive view of the local storage subsystem's operational status. This includes details on the DbWriter, WAL (Write-Ahead Log), projection subsystem, evidence spooling, kill switches, and freshness of projection data.

| GraphQL type | Fields | Semantics |
|---|---|---|
| `GqlStorageHealth` | `dbState` | Overall health status of the database (e.g., `HEALTHY`, `DEGRADED`, `STALE`). |
| | `writer` | Details about the DbWriter, including alive status, queue depths for different write classes, and latency metrics. |
| | `wal` | Status of the SQLite Write-Ahead Log, including availability, size, and checkpointing information. |
| | `projections` | Health metrics for the projection subsystem, including pending invalidations and projection lag. |
| | `evidenceSpool` | Summary of the evidence spooling mechanism, including file counts and orphan status. |
| | `killSwitches` | State of various storage-related kill switches (e.g., disabled write classes, evidence spool kinds). |
| | `thresholds` | Configured warning and critical thresholds for various storage metrics. |
| | `projectionFreshness` | A list of `GqlProjectionFreshnessV1` objects, each detailing the freshness of a specific projection, including its watermark, poisoning status, and backlog. |
| | `projectionFreshnessBySource(projectionName: String, sourceName: String)` | A filterable complex field returning `GqlProjectionFreshnessV1` objects. Allows clients to query freshness details for specific projections or data sources. |
| | `hotReadGuards` | A list of `HotReadCircuitStateV1` objects exposing per-surface hot-read circuit state (closed, open, half-open) including `wouldOpen` observe-mode counters used by the storage-tiering promotion budget. |
| | `maintenanceOperations` | A list of `MaintenanceOperationStatusV1` objects describing active and recently terminal maintenance operations (e.g., `repair_slot`) with `operationId`, `slotGeneration`, and audit-bound state. |
| | `degraded` | Optional `DegradedStateV1` carrying a compact severity, short reason, and inline-detail payload for the operator UI degraded-state pattern. |
| | `rollout` | JSON readback of the active rollout contract for the storage tiering / read-path liveness surface, including `rollout_contract_status`, decision, enforcement mode, hold conditions, and retained `p087_*` readback fields enumerated by the rollout contract (P084 schema). |
| | `updatedAt` | Timestamp of the last health status update. |
| | `staleAfterMs` | Duration in milliseconds after which the health data is considered stale. |
| | `isStale` | Boolean indicating if the current health data is considered stale. |

Storage health diagnostics are public readback, not raw internal error transport. Unknown persisted error strings, idempotency material, hostnames, principal tokens, and provider-authored diagnostic text must be reduced to explicit public error codes or stable hash references before they leave the daemon. If a diagnostic subquery fails, `storageHealth` remains available with `degraded.reason = storage_health_partial_readback_unavailable` or a projection-local `storage_health_subquery_unavailable` marker instead of silently presenting missing data as absent data.

Hot-read liveness mode defaults may be relaxed only for local development. Production mode (`CHAINWORKS_ENV=production` or `CHAINWORKS_STORAGE_TIERING_PRODUCTION_MODE=1|true`) requires `CHAINWORKS_STORAGE_TIERING_READ_PATH_LIVENESS_MODE=enforce`; any other value is a rollout hold reported as `p087_liveness_mode_not_enforced_in_production`.

The `p087_*` field prefix is a retained schema and rollout-readback alias for this implemented storage-tiering contract. It is stable wire vocabulary, not a dependency on the retired proposal document.

### GqlProjectionFreshnessV1

Provides detailed freshness information for individual projections.

| Field | Semantics |
|---|---|
| `projectionName` | The name of the projection. |
| `sourceName` | The name of the data source for the projection (optional). |
| `watermarkMs` | The highest timestamp (in milliseconds) processed by the projection. |
| `isPoisoned` | Indicates if the projection is in a poisoned state due to errors. |
| `lastError` | Details of the last error encountered by the projection (if any). |
| `updatedAtMs` | Timestamp (in milliseconds) when this freshness record was last updated. |
| `throttledUntilMs` | Optional timestamp (in milliseconds) until which producers for this projection/source are throttled because the invalidation backlog crossed bound thresholds; `null` when no throttle is active. |
| `backlogRows` | Number of rows currently in the projection's processing backlog. |
| `backlogBytes` | Size in bytes of the projection's processing backlog. |



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

These values bind any client consuming this contract. The read-only thin UI must use them for stale-state rendering, retry cadence, and release hold/degraded-state checks. The `Stale/action-safety disable threshold` and `Cutover rollback threshold` rows that reference commands/mutations are NO-OPS for the governed thin UI (no commands to disable, no mutations to roll back) but remain normative for any command-UI consumer. These rows do not restore the old Swift orchestrator or any local workflow-truth write path. Changes to these values are contract changes and must update the gate.

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
| Cutover rollback threshold | 3 consecutive command-completion refresh timeouts or 2 minutes continuous `unavailable` | Hold or roll back the affected command-client surface. For the governed thin UI, this is only a read-only unavailable/degraded-state release hold; it is not a rollback to local Swift orchestration. |

## Refresh and subscription posture

| Trigger | Required behavior |
|---|---|
| View load/navigation | Execute the matrix entrypoint for the selected surface. |
| App foreground/reconnect | Refresh visible run, stage, approval, artifact, report, and health surfaces. |
| MCP command accepted (command-UI only; N/A for governed thin UI) | Keep previous authoritative read model plus pending receipt; refresh GraphQL before displaying new workflow truth. |
| Subscription event | Patch only fields covered by that event contract, or perform a bounded refresh. Do not infer unrelated state. |
| Subscription disconnect | Mark affected surfaces `refreshing_disconnected`; schedule bounded reconnect; transition to `live` if reconnect succeeds inside 10 seconds or `stale` if the grace window expires. |
| Query failure | Keep last known state as `stale` when available; otherwise show `unavailable` or `unauthorized` based on error class. |
| Projection rebuild lag | Mark projection-derived fields `projection_lag` until the projection-backed query returns consistent state. |

Recognized subscription names for this contract:

| Subscription | Payload contract | Thin UI consumption rule |
|---|---|---|
| `runStatusChanged(runId:)` | Projection-enriched `GqlRun` via `find_run_projection`, including `projectionPresent`, `projectionUpdatedAt`, and `projectionLag`. | May patch displayed run summary fields; refresh after command completion still required. |
| `stageStatusChanged(runId:)` | Projection-enriched `GqlStageExecution` via `list_stages_projection`, including `projectionPresent`, `projectionUpdatedAt`, and `projectionLag`. | May patch stage decision flags; controls remain disabled during `projection_lag`. |
| `approvalRequested` | Emits current approval row for the requested approval. | May update approval inbox; command completion refresh still required after decisions. |
| `approvalResolved` | Emits current approval row for the resolved approval. | May remove/update approval row; bounded refresh fallback remains valid if subscription is unavailable. |
| `schedulerBackpressureChanged` | Emits sustained-backpressure events when thresholds are crossed. | May trigger UI health alerts or banner changes. |
| `runtimeStatusChanged` | Broader runtime/adapter health event stream remains future work beyond the implemented daemon lifecycle stream. | Deferred for adapter-health UI until a server-owned runtime-health contract is accepted. |

Missing subscription support is not a reason for the client to infer truth locally. It only changes the refresh strategy to bounded visible-surface polling.

## Freshness behavior evidence and limitations

The control-plane read contract owns server-published facts. The thin macOS UI owns timers, reconnect loops, and read-side freshness rendering. Command-completion refresh and disabled-control rendering apply to any command-issuing client that consumes this contract; the read-only thin UI has no commands to refresh or controls to disable, so those rows apply vacuously to it (they remain normative for any future command UI).

| Freshness behavior | P043 evidence | Consumer cutover rule |
|---|---|---|
| Initial query failure to `unavailable` or `stale` | Contract row: Initial read timeout = 5 seconds. | Thin UI must test visible read-surface initial-failure rendering. A command UI must additionally disable surfaces that depend on the same initial read. |
| Command-completion refresh timeout to `stale` | Contract row: Command-completion refresh timeout = 3 seconds. | Applies to a command-issuing client: must test accepted-command pending receipt plus stale transition before enabling follow-on mutations. Governed thin UI has no commands; vacuous. |
| Foreground/reconnect refresh timeout | Contract row: Foreground/reconnect refresh timeout = 5 seconds. | Thin UI must test foreground/reconnect refresh state settlement for its read surfaces. |
| Projection lag action safety | Contract row: Projection-lag grace window = 2 seconds. | A command-issuing client must disable actions that depend on projection flags until projection-backed queries catch up. Governed thin UI has no actions; vacuous. |
| Subscription disconnect action safety | Contract row: Subscription disconnect grace window = 10 seconds and state `refreshing_disconnected`. | A command-issuing client must disable destructive/state-changing actions during reconnect grace and mark `stale` after expiry. Governed thin UI renders the freshness state read-only; no actions to disable. |
| Bounded polling fallback | Contract rows: interval = 5 seconds; backoff = 5s, 10s, 20s, then 30s max. | Thin UI may poll only visible implemented surfaces and must not poll deferred surfaces. |
| Unauthorized read behavior | Executable proof: `proposal_043_graphql_reads_are_operator_only_v1`. | Thin UI must show read authorization error and never fall back to local storage. |
| Stale/action-safety disable threshold | Contract row: immediate disable for unsafe freshness states. | A command-issuing client must prove disabled controls for `refreshing_disconnected`, `stale`, `projection_lag`, `unavailable`, and `unauthorized`. Governed thin UI renders these as read-side badges/annotations on the affected surfaces and has no controls to disable. |

## GraphQL field proof

| Surface | Proof | Result |
|---|---|---|
| Runs home | Projection-backed list query through `list_by_idea_projection` and `list_active_projection`. | Sufficient for thin UI consumption. |
| Run detail | `run(id:)` returns projection-enriched counters, summaries, `workflowConflict`, `projectionPresent`, `projectionUpdatedAt`, and `projectionLag` from `find_run_projection`. | Sufficient for thin UI consumption. |
| Stage list / progress | `stages(runId:)` reads stage projection rows with decision flags, `projectionPresent`, `projectionUpdatedAt`, and `projectionLag`. | Sufficient for thin UI consumption. |
| Stage detail | `stage(id:)` returns projection-enriched decision flags and projection freshness while preserving canonical evidence/recovery payloads. | Sufficient for thin UI consumption. |
| Missing projection rows | Missing `run_summaries` or `stage_summaries` rows surface as `projectionPresent=false` and `projectionLag=true`, not normal zero/false truth. | Sufficient for projection-lag rendering. |
| Run status subscription | `runStatusChanged(runId:)` emits projection-enriched run summary and freshness fields. | Sufficient for P031 event patching. |
| Stage status subscription | `stageStatusChanged(runId:)` emits projection-enriched stage decision and freshness fields. | Sufficient for P031 event patching. |
| Approval resolved subscription | `approvalResolved` emits current resolved approval rows. | Sufficient for P031 event patching. |
| Approval inbox | `approvalInbox` is projection-backed. | Sufficient for thin UI consumption. |
| Artifact viewer | `artifacts(runId:)` | is projection-backed. | Sufficient for thin UI consumption. |
| Scheduler health | `schedulerHealthSummary` returns global capacity, active counts, oldest queued age, and sustained backpressure state. | Sufficient for thin UI health alerts. |
| Startup recovery | `startupRecoverySummary` returns recovered items, backpressured recovery counts, and affected runs. | Sufficient for thin UI recovery UI. |
| Command latency | `commandLatencySummary` returns p95 latency for approve, retry, and cancel. | Sufficient for thin UI diagnostics. |
| DB contention | `dbWriterContentionSummary` returns write wait p95 and transaction contention. | Sufficient for thin UI diagnostics. |
| Provider capacity | `activeExecutionCountsByProvider` returns active execution counts per canonical family. | Sufficient for thin UI capacity UI. |
| Queue summaries | `runQueueSummary`, `stageQueueSummary`, and `queuedBackpressuredCountsByProviderAndReason` are projection-backed. | Sufficient for thin UI backpressure UI. |
| Queue position | `queuePositionHint` returns non-ETA position hint from projection truth. | Sufficient for thin UI queue UI. |
| Host interruption | `hostInterruptionEpochs` and `hostInterruptionAffectedExecutions` are canonical readbacks. | Sufficient for thin UI recovery UI. |
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

## Artifact Metadata Pointers

Hot read surfaces expose artifact payload location as `artifact_metadata_pointer.v1`, not as a raw filesystem path. The pointer contains only:

- `schemaVersion = artifact_metadata_pointer.v1`;
- `artifactId`;
- `checksumSha256`;
- `sizeBytes`;
- `authorizedPayloadRoute`;
- `payloadPathRedacted = true`;
- `forbiddenFields = ["absolutePath", "filesystemPath", "rawPayload"]`.

The canonical on-disk path can remain in compact persistence and privileged diagnostic paths, but `artifact://...` MCP metadata readback and GraphQL artifact metadata must provide the pointer so list/detail hot reads do not leak host-local paths or raw payload bytes.

## Thin UI Consumption Contract

Governed macOS UI surfaces ship from this contract:

- Runs home;
- Run detail;
- Stage list / progress;
- Stage detail;
- Approval inbox;
- Artifact viewer.

Report viewing may ship only as a partial surface where missing payload readback is visibly annotated as unavailable. Runtime health and experiment comparison remain hidden or placeholder-only until server-owned read surfaces exist.

The macOS thin UI owns the UI-side evidence for the **read-side** client contract:

- reconnect timers;
- live / refreshing_disconnected / stale / projection_lag / unavailable / unauthorized rendering on each surface (as badges or inline annotations, not disabled controls because the governed thin UI has no write controls);
- no SwiftData / local-service fallback for workflow truth;
- subscription patching rules from the "Refresh and subscription posture" section.

Scope boundary: the governed thin UI remains a read-only consumer for workflow truth and non-approval commands. The UI action boundary adds only the approval-resolution mutation exception; it does not make SwiftUI responsible for MCP command-control behavior, command receipts, or broad UI writes.

The thin UI boundary does NOT own:

- MCP command-completion refresh behavior (the governed thin UI issues no MCP mutations);
- disabled-action rendering for destructive commands (the governed thin UI has no action surfaces);
- "command receipt" displays (no commands);
- any `runs.start` / `runs.cancel` / `approvals.resolve` / `stages.retry` / `ideas.create` UI.

These belong to a future command-UI proposal or to operators invoking MCP directly.

## Gate and verification

The current proof lane is:

```bash
./scripts/test-gate.sh proposal-043
./scripts/test-gate.sh p043
./scripts/test-gate.sh proposal-031
./scripts/test-gate.sh p031
```

The P043 gate runs focused `graphql-server` tests whose names start with `proposal_043_` and `proposal_031_`, then validates this reference document from the repository root. The P031 gate composes the thin UI inventory, Swift read-boundary tests, GraphQL read model checks, and static guards for governed UI code.

The test slice covers:

- projection-backed run queries;
- projection-backed stage queries;
- missing projection rows surfacing as `projectionPresent=false` and `projectionLag=true`;
- projection-enriched run and stage subscription payloads;
- `approvalResolved` subscription availability;
- operator-only V1 reads and subscription authorization;
- sensitive field redaction (diagnosticId, serverDebugDetail) protected by operator-only policies.

The gate fails closed when this reference document omits required surfaces, statuses, freshness budget rows, projection freshness fields, freshness behavior limitations, subscription posture, operator-only V1 policy, projection parity, known gaps, or cutover decision rules.

## Known holds

- Report payload rendering needs a server-owned GraphQL payload path before it can be a full thin-client surface.
- Adapter/runtime health beyond the daemon lifecycle read model remains deferred unless a future server-owned read surface publishes it.
- Experiment comparison has no current GraphQL read owner and stays deferred.
- Broader operator dogfood/productization is owned by P032, and the implemented visual/navigation shell is owned by [macos-operator-navigation.md](macos-operator-navigation.md). Those tails do not change the thin UI boundary for new feature work.
