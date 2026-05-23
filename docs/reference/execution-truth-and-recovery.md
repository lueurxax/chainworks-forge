# Execution Truth and Recovery

Stable reference for the execution-truth, settlement, and recovery contract.

Configurable escalation policies layer a chain-level contract on top of agent-level execution truth: tier advancement, trigger classification, ledger persistence, and recovery semantics are owned by the Rust control plane. See [escalation-policies.md](escalation-policies.md) for policy schema, pause-reason catalog, and rollout state. The escalation-specific invariants relevant to recovery readers are pinned in *Escalation chain invariants* below.

## Purpose

The runtime must be able to say, once and only once, what actually happened in an agent attempt after output, timeout, cancellation, limit exhaustion, relaunch, and recovery.

This document is the stable contract for:

- canonical terminal outcomes on `AgentExecution`,
- stage-level settlement and recovery evidence on `StageExecution`,
- approval restoration and resume behavior,
- frozen-vs-runtime binding truth in reports,
- and recovery/report readers that must prefer persisted truth over heuristic reconstruction.

## Scope

This reference covers:

- agent-level terminal outcome classification,
- persisted truth columns versus supporting diagnostic envelopes,
- stage-level failure and recovery evidence,
- resume / approval-restore behavior after interruption,
- runtime binding truth as read by reports and operator surfaces,
- and the current proof-owning test suites for this slice.

It does not replace:

- the broader engine topology in [workflow-execution-engine.md](workflow-execution-engine.md),
- frozen run snapshots in [runtime-contract.md](runtime-contract.md),
- provider setup/platform behavior in [provider-platform.md](provider-platform.md),
- or operator-shell interaction rules in [operator-experience.md](operator-experience.md).

## Core Rules

### One canonical terminal outcome per agent attempt

Every settled `AgentExecution` uses exactly one canonical terminal outcome:

- `completed`
- `completed_with_transport_error`
- `failed_before_output`
- `failed_after_output_validation`
- `timed_out_before_output`
- `timed_out_after_output`
- `cancelled_before_output`
- `cancelled_after_output`
- `limit_exhausted_before_output`
- `limit_exhausted_after_output`

These values live in [`Chainworks Forge/Models/ExecutionTruth.swift`](<../../Chainworks Forge/Models/ExecutionTruth.swift>) and are persisted on `AgentExecution.canonicalOutcome`.

### Neutral finish markers are not success on their own

Transport finish markers such as `stop` or `session_closed` describe how streaming ended.
They do not by themselves prove successful completion.

Current classification rules in `RuntimeAgentExecutor` therefore require more than a neutral finish marker:

- durable output plus later transport failure becomes `completed_with_transport_error`,
- timeout before output becomes `timed_out_before_output`,
- timeout after output becomes `timed_out_after_output`,
- provider/app limit exhaustion becomes one of the explicit `limit_exhausted_*` outcomes,
- neutral stop with no durable output remains failure, not silent success.

### Flattened persisted columns outrank envelopes and receipts

The primary persisted execution-truth columns on `AgentExecution` are:

- `canonicalOutcome`
- `supervisionClassification`
- `transportErrorKind`
- `providerStopReason`
- `outputPresence`
- `settledAt`
- `runtimeProvider`
- `runtimeModel`

`outcomeEnvelopeJSON` is supporting diagnostic evidence.
It exists to explain the settled outcome, not to compete with it.

Readers must use this precedence:

1. flattened persisted execution-truth columns,
2. supporting evidence such as `outcomeEnvelopeJSON`, `providerReceiptJSON`, and validation payloads,
3. coarse legacy fields like `AgentStatus` only when canonical columns are absent.

Raw receipts or transcripts must never silently override canonical persisted outcome truth.

### Watchdog-specific truth refines, but does not replace, canonical outcome

`supervisionClassification` is the durable refinement field for watchdog-specific execution truth.

The stable contract is:

- `canonicalOutcome` remains the terminal execution state,
- `supervisionClassification` carries watchdog-specific or integrity-specific refinement such as:
  - `idleHangBeforeFirstProgress`
  - `idleHangAfterProgress`
  - `idleHangReadLoop`
  - `idleHangAfterFirstEdit`
  - `mutationSideEffectMissing`
- `transportErrorKind` and `providerStopReason` remain orthogonal transport/provider evidence,
- `outcomeEnvelopeJSON` and receipts explain the settled truth but do not redefine it.

Readers must therefore interpret agent-level execution truth in this order:

1. `canonicalOutcome` for terminal state,
2. `supervisionClassification` for watchdog-specific refinement,
3. `transportErrorKind` and `providerStopReason` for transport/provider context,
4. evidence payloads only as supporting detail.

### Escalation chain invariants

Two escalation invariants directly affect recovery readers:

- **Overlap-free tier**: `escalation.tier_advanced` is emitted only after the previous tier reaches a settled terminal outcome, and no ledger holds more than one active tier. Force-detach windows therefore cannot double-charge provider quota.
- **Single scheduler transaction**: settlement, trigger selection, digest calculation, frozen policy lookup, readiness/capacity validation, and all `escalation_ledger` / `escalation_events` / `escalation_execution_metadata` updates commit in one SQLite transaction. Provider launch occurs only after commit.

Full policy schema, pause-reason catalog, and Phase 2+ recovery contracts (force-detach replay, shutdown drain, SQLITE_BUSY retry budget) live in [escalation-policies.md](escalation-policies.md).

### Rust ACP runtime facts are durable execution truth

The Rust control plane persists provider-independent runtime facts for every ACP-backed
agent execution that reaches the engine-owned execution path. These facts are not log
parsing and are not reconstructed from transcripts during readback.

#### AgentExecution Owner Model

To support lead-mediated conflicts without synthetic stage states, `AgentExecution` 
uses a general owner model (ARCH-037). 
- **owner_kind**: `stage_execution` or `lead_conflict_mediation`.
- **owner_id**: References either `stage_execution_id` or `mediation_record_id`.
- **stage_execution_id**: Becomes nullable; required for `stage_execution` owners 
  and null for `lead_conflict_mediation`.

This allows mediation-owned executions to reuse the same retry, quota, 
artifact, and cost infrastructure as stage-owned executions.

`agent_execution_runtime_facts` is the durable execution-facts row keyed by
`agent_execution_id`. It records:

- `failure_kind` as a stable `AgentFailureKind`,
- `failure_kind_raw_debug` for future or provider-specific raw values,
- `failure_message_redacted` and its redaction version,
- `retry_after` and `operator_action_hint`,
- provider process / transport diagnostics such as exit status, transport code, and
  supervision classification,
- `output_settlement`,
- required-output validity and late-output counters,
- session reuse reason,
- `quota_ledger_id`,
- creation and update timestamps.

`escalation_ledger`, `escalation_execution_metadata`, and `escalation_events` are the durable chain-level companions to `agent_execution_runtime_facts`: tier advancements, chain exhaustion, and pause reasons are recorded there. Schema details live in [escalation-policies.md](escalation-policies.md).

`agent_execution_discovery_diagnostics` is a related durable table that owns detailed discovery pipeline execution decisions (exact paths, provider envelopes, meta-root bounding). `agent_execution_runtime_facts` projects those decisions into scalar `output_settlement` truth but does not store the full payload.
Current `AgentFailureKind` values include:

- `provider_quota`
- `provider_permission_required`
- `provider_permission_rejected`
- `provider_timeout`
- `provider_internal_error`
- `transport_epipe`
- `transport_protocol_error`
- `transport_closed`
- `mcp_startup_timeout`
- `mcp_permission_modal_stall`
- `xcode_host_environment_error`
- `missing_required_outputs`
- `invalid_output_contract`
- `cancelled_by_operator`
- `superseded_by_retry`
- `host_interruption`
- `unknown`

Rules:

- unknown stored failure kinds map to public `unknown` while preserving the raw value
  in operator-only debug readback,
- non-operator GraphQL/MCP readers must receive `null` or omitted raw debug detail,
- `quota_ledger_id` references the durable provider-quota retry ledger,
- runtime facts are the preferred source for recovery action hints and report summaries,
- output-settlement truth stays separate from failure-kind truth.

`AgentOutputSettlement` captures what happened to declared outputs independently of
why the provider finished:

- `none`
- `valid_outputs_from_completed_execution`
- `valid_outputs_from_failed_execution`
- `missing_required_outputs`
- `invalid_required_outputs`
- `ignored_late_outputs`

`ignored_late_outputs` is settlement truth, not an `AgentFailureKind`.

## Stage Truth and Recovery Evidence

### `StageExecution` is the stage-level owner

Stage-level truth remains anchored on `StageExecution`.
The current persisted stage fields for this slice are:

- `lineageID`
- `settlementKind`
- `settledAt`
- `activeOwnerToken`
- `validationFailureJSON`
- `evidencePacketJSON`
- `recoverySnapshotJSON`

The important contract is ownership, not file shape:

- stage terminality belongs to the stage record,
- failed-stage evidence belongs to the stage record,
- recovery recommendations belong to the stage record,
- reports and recovery surfaces read the stage record first instead of inferring truth from loose artifact scans.

`recoverySnapshotJSON` is stage-owned next-action truth, not agent-level execution truth.
It may narrow the operator action after a watchdog failure or exhausted retry, but it must not override the settled `AgentExecution` truth described above.

### Durable Side-Effect Ledger and Reconciliation

For irreversible or externally visible operations (e.g., `git_push`, `connect_upload`), success is not just about the agent finishing. The system must ensure that the side effect is durable and reconcilable if a crash occurs mid-execution.

**Durability Rules:**
- **Durable Intent**: The control plane persists a `SideEffect` record with status `prepared` before the external operation begins.
- **Fail-Closed Retry**: If a run or stage has unresolved side effects (status `prepared`, `executing`, `externally_observed`, `needs_reconciliation`, `conflict`, or `unrecoverable`), the engine **blocks retry, cancellation, scheduler advancement, and recovery mutations** for that run/stage with `requires_effect_reconciliation`.
- **At-Most-Once Write**: The engine guarantees at most one external-write attempt per `side_effect` row. If an attempt fails or is ambiguous, the record moves to `needs_reconciliation` rather than auto-retrying.
- **Idempotency**: Every side effect uses a deterministic `idempotency_key` (derived from run/stage/agent/target) to help external systems (like GitHub or App Store Connect) detect duplicate requests.
- **Readback Circuit Breaker**: Repeated ledger readback failures for the same call site open a fail-closed circuit breaker; fallback heuristics may describe risk but cannot permit mutation while the circuit is open.
- **Evidence Integrity**: Release evidence is file-spooled and checked through a manifest. Missing, partial, checksum, or size failures transition affected records to reconciliation-oriented readback instead of silently settling release truth.

**Side-Effect Statuses:**
- `prepared`: Intent recorded, operation not yet started.
- `executing`: Operation started, outcome unknown.
- `externally_observed`: Evidence suggests the side effect happened, but it's not yet settled.
- `needs_reconciliation`: Ambiguous outcome requiring operator or startup repair.
- `settled`: Side effect confirmed successful and linked to canonical state.
- `reconciled`: Operator manually resolved an ambiguous outcome.
- `conflict`: Idempotency conflict detected.
- `unrecoverable`: Side effect failed and cannot be safely retried.

**Reconciliation Paths:**
- **Startup Repair**: The daemon reconciles stale `executing` side effects at launch. If they outlived their lease or deadline, they move to `needs_reconciliation`.
- **Watchdog Repair**: The engine also checks prepared, externally observed, and settled-evidence integrity windows and fails closed when evidence cannot prove the settled state.
- **MCP Operator Tools**: Operators use `effects.list`, `inspect`, and `reconcile` to review unresolved effects and apply dispositions such as `mark_conflict`, `mark_unrecoverable`, or `clear_after_manual_verification`.
- **Read-Only Clients**: GraphQL, run reports, release receipts, and SwiftUI expose side-effect readback and recommended MCP next actions. They do not provide side-effect mutation affordances.

### Workflow Conflict Recovery

When declarative graph authority fails to select a valid next state, the run
blocks with a `WorkflowConflictRecord`.

System routing is expected to produce a bounded reviewer set. If more than five
reviewers match mandatory routing rules, the router deterministically keeps the
five strongest reviewers, records `mandatory_overflow_pruned` in the
`AgentSelectionPlanV1` warnings, and records each pruned reviewer as a rejected
alternative with reason `mandatory_overflow_pruned`. Invalid overrides or
unverifiable routing inputs still fail closed.

**Conflict Classification:**
- `unresolved`: Initial state requiring attention.
- `routing_conflict`: Specifically for deterministic routing failures.
- `lead_mediation_pending`: Escalated to a system lead for same-run resolution.
- `operator_confirmation_required`: Lead produced a resolution that requires 
  manual approval.
- `resolved`: Successfully settled back into graph authority.
- `superseded`: A newer conflict fingerprint arrived before resolution.
- `terminal_unverifiable`: Irrecoverable conflict requiring manual resolution 
  (e.g., clone or manual edit).

**Advisory Rejection Truth:**
If the graph advances legally despite agent hints that would have caused a 
conflict, the runtime persists a `WorkflowAdvisoryRejectionRecord`. These are 
not blocking and appear in run reports and history as non-critical evidence of 
graph authority.

### Transition Cursor Authority

Transition completion and cursor update are one atomic settlement unit. 
The run-level transition cursor is the authoritative continuation signal:

- `currentStageID` resolution is cursor-first.
- If a blocking `WorkflowConflictRecord` is current, the cursor remains anchored 
  at the current state with `resume_policy=await_conflict_resolution`.
- Transition settlement cannot be inferred from partial stage snapshots alone.

### Implementation Handoff Status

Runs entering implementation use `ImplementationHandoffStatus` to
track engine-owned handoff truth (ARCH-038):
- **Engine-Owned Handoff**: The engine owns the deterministic `approved_proposal`
  snapshot and handoff artifacts.
- **Durable Readback**: `code_writer_start_status` remains `not_queued` until an
  execution is actually claimed. `implementation_handoff_status` distinguishes
  between `ready`, `blocked_before_code`, and `running`.
- **Failure Handling**: Implementation-entry planning timeouts block with
  `implementation_handoff_unavailable` without losing the deterministic approved
  proposal. Retry resumes from the handoff/planning boundary.

### Recovery uses the narrowest valid next action

`StageRetryCoordinator` persists and rebuilds `RecoveryActionSnapshot` values that describe the narrowest valid next step:

- retry failed agent,
- retry failed stage,
- operator inspection first,
- clone run from frozen snapshot,
- clone run from current config.

`RunReportBuilder` and `RecoveryCoordinator` consume these snapshots directly when present and synthesize them from stage evidence only as a fallback.

### Provider quota recovery uses a durable ledger

Provider quota failures are classified as `provider_quota`, may persist `retry_after`,
and are linked to `agent_retry_budget_ledger` through
`agent_execution_runtime_facts.quota_ledger_id`.

The retry ledger is idempotent per execution and records whether a retry waited for
the provider reset window or explicitly consumed normal retry budget early. Stage
retry handling must consult this ledger before resetting execution state so operators
cannot accidentally hide quota exhaustion as an ordinary retry.

Recovery snapshots should prefer runtime facts when selecting the next action:

- `provider_quota` -> wait for `retry_after` or explicitly consume budget,
- `provider_permission_required` -> provider authorization,
- `mcp_permission_modal_stall` -> Xcode/MCP authorization,
- `missing_required_outputs` -> inspect outputs then retry,
- `invalid_required_outputs` -> inspect contract then retry,
- `valid_outputs_from_failed_execution` -> accept degraded outputs or retry,
- `host_interruption` -> automated jittered retry under capacity caps.

### Host Interruption

`host_interruption` is a neutral or cautionary failure kind, not a critical provider
failure. It is detected by comparing monotonic and wall-clock timestamps or via macOS
system hooks.

Rules:
- Only executions running across the detected epoch are eligible for host interruption
  classification.
- Host-interrupted executions terminate ACP sessions and provider process groups
  before retry.
- Retries are exempt from provider quota retry budget but still count against
  active execution capacity.
- Late or partial outputs from superseded host-interrupted attempts are skipped
  unless existing settlement rules allow promotion.

### Startup Recovery

Startup recovery repair is the first phase of daemon execution. It reconciles
stale running work from a previous process crash or hard shutdown.

Rules:
- **Capacity-Aware Requeue**: Recovered work is requeued through the same capacity
  gates as ordinary work. It does not bypass global, provider, or per-run caps.
- **Durable Readback**: Startup recovery progress is persisted in the
  `startup_recovery_readbacks` table and exposed via GraphQL/MCP so operators
  can see the recovery backlog during initialization.
- **Stale Repair**: A terminal, skipped, or superseded stage must not own a
  running agent execution after startup repair completes.
- **Idempotency**: Repeated startup repair cycles converge to the same truth
  without duplicating work items.

## Resume and Approval Restore

### Atomic transition settlement and cursor authority

Transition completion and cursor update are one settlement unit in the execution-persistence flow. Recovery never infers continuation from partial stage snapshots alone.

The run-level transition cursor is the authoritative continuation signal:

- `currentStageID` resolution is cursor-first — if cursor metadata is present, projection and UI-facing stage state follow it; `stageExecutions` order is a compatibility fallback only.
- Interrupted transition paths keep intermediate marker state so the system can surface exact interruption and continue deterministically.
- Workflow-map projection and report/recovery builders derive continuation from cursor data before stage aggregate views.

Invariants:

1. no UI-facing stage claims resumable state without matching cursor continuity,
2. repeated projection/recovery cycles converge to the same cursor-derived stage,
3. resumed runs never lose transition intent when partial transition state is present.

Implementation owners: `Run` cursor metadata and derived-stage helpers, `WorkflowMapProjectionService`, `RunReportBuilder`, `RecoveryCoordinator`.

### Resume is fail-closed

`ResumeManager` does not blindly restart work.
It classifies interrupted runs as:

- resumable,
- needing operator decision,
- or not resumable.

That classification already considers:

- compiler-version mismatches,
- frozen snapshot rebuild failure,
- workflow or catalog drift,
- side-effect-stage interruption,
- and frozen workspace-path validity.

### Approval restore preserves operator context

Approval-bound runs are allowed to restore visible pending approval context after relaunch.
The contract is:

- approval gates restore the same operator decision point (read-only/diagnostic in P031) when the persisted state still supports it,
- drift can be surfaced as context without silently discarding the approval state,
- recovery or report readers must not invent a new approval truth that was not persisted.

`Approval.lineageID` and `Approval.repairedAt` exist as persisted approval-truth fields for this slice; consumers should treat them as the canonical lineage metadata when present.

## Runtime Binding Truth

Execution truth is not only about success or failure.
Reports also need to say what provider/model actually ran.

The current read path combines:

- run-level frozen intent and trust metadata on `Run`,
- frozen provenance in `bindingProvenanceJSON`,
- and runtime provider/model evidence persisted per `AgentExecution`.

Rules:

1. frozen binding intent remains historical context, not reconstructed guesswork,
2. runtime provider/model evidence should be shown when present,
3. weak or contradictory runtime evidence should downgrade trust instead of manufacturing certainty.

The narrower binding contract is documented in [provider-binding-truth.md](provider-binding-truth.md).

## Recovery and Report Read Order

Current report/recovery readers should prefer:

1. `AgentExecution` execution-truth columns,
2. Rust `agent_execution_runtime_facts` when present,
3. `StageExecution` failure and recovery payloads,
4. run-level trust / provenance metadata,
5. coarse legacy statuses only as compatibility fallback.

This keeps report timelines, failed-step summaries, retry hints, and resume guidance tied to persisted truth rather than heuristic rescans of historical artifacts.

## Verification and Proof Owners

This slice is currently proved primarily through current-head non-UI test suites rather than a dedicated standalone wrapper gate.

High-signal proof owners include:

- `RuntimeAgentExecutorTests` for transport-outcome classification and limit exhaustion,
- `OrchestratorTests` for persistence of canonical outcome, provider/model truth, and validation-after-output settlement,
- `ResumeManagerTests` for interrupted-run classification and approval restore behavior,
- `RecoveryCoordinatorTests` for narrow recovery action ownership,
- failed-stage evidence and report/recovery fallback suites,
- retained escalation proof gate for Rust ACP runtime facts, claim/start ownership,
  source-generation artifact ownership, GraphQL/MCP readback parity, and provider-quota
  retry ledger behavior.

## Adjacent References

Use:

- [runtime-contract.md](runtime-contract.md) for frozen snapshots and artifact boundaries,
- [workflow-execution-engine.md](workflow-execution-engine.md) for orchestrator topology,
- [run-control.md](run-control.md) for cancellation settlement and operator-visible cancel truth,
- [provider-binding-truth.md](provider-binding-truth.md) for historical binding provenance,
- [operator-experience.md](operator-experience.md) for shell/report/recovery presentation contracts,
- [recovery-retry-state-machine-test-matrix.md](recovery-retry-state-machine-test-matrix.md) for the canonical P082 scenario matrix, reason-code vocabulary, readback schemas, and proof gate that recovery behavior changes must extend.
