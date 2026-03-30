# Execution Truth and Recovery

Stable reference for canonical agent outcome truth, stage settlement, resume idempotency, aggregate settlement, and recovery/report read behavior in the current Chainworks Forge baseline.

## Purpose

The runtime must be able to say, once and only once, what actually happened in an attempt after output, timeout, cancellation, limit exhaustion, relaunch, and recovery.

This document is now the authoritative contract for:

- canonical terminal outcomes on `AgentExecution`,
- stage and approval lineage ownership,
- aggregate settlement truth,
- fail-closed legacy backfill,
- frozen-vs-runtime binding truth in report/recovery surfaces,
- and the narrowest valid recovery action after settlement.

For implementation/proof status, use [../evidence/execution-truth-and-recovery-proof.md](../evidence/execution-truth-and-recovery-proof.md).

## Scope

This reference covers:

- agent-level terminal outcome classification,
- stage-level settlement ownership,
- create-path uniqueness and startup repair,
- aggregate-step settlement,
- runtime binding truth summaries,
- report/recovery read precedence,
- and proposal-owned proof lanes for this slice.

It does not redefine:

- the broader workflow topology contract in [workflow-execution-engine.md](workflow-execution-engine.md),
- the general frozen snapshot rules in [runtime-contract.md](runtime-contract.md),
- provider-platform setup in [provider-platform.md](provider-platform.md),
- or release-side delivery behavior in [full-mvp-delivery.md](full-mvp-delivery.md).

## Core Rules

### One canonical terminal outcome per agent attempt

Every `AgentExecution` settles to exactly one canonical terminal outcome:

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
- `legacy_unverifiable`

`Finish: stop` and other neutral transport-finish markers are not success on their own. Success requires both a durable-output story and an explicit success criterion.

### Canonical outcome to coarse status mapping

`AgentStatus` remains the coarse lifecycle field used for broad run progression. `canonicalOutcome` remains the authoritative terminal truth.

| `canonicalOutcome` | Coarse `AgentStatus` |
|---|---|
| `completed` | `completed` |
| `completed_with_transport_error` | `completed` |
| `failed_before_output` | `failed` |
| `failed_after_output_validation` | `failed` |
| `timed_out_before_output` | `failed` |
| `timed_out_after_output` | `failed` |
| `cancelled_before_output` | `cancelled` |
| `cancelled_after_output` | `cancelled` |
| `limit_exhausted_before_output` | `failed` |
| `limit_exhausted_after_output` | `failed` |
| `legacy_unverifiable` | `failed` |

Readers must not reverse this mapping and invent finer truth from coarse `AgentStatus`.

### One authority for persisted truth

Canonical flattened columns on `AgentExecution` are the authority:

- `canonicalOutcome`
- `transportErrorKind`
- `providerStopReason`
- `outputPresence`
- `settledAt`
- `runtimeProvider`
- `runtimeModel`

Supporting evidence stays supporting evidence:

- raw provider/session receipts,
- transcripts,
- diagnostic envelopes,
- legacy receipt JSON.

Readers use this precedence:

1. canonical flattened columns,
2. normalized supporting receipt/envelope evidence,
3. legacy fallback classification,
4. `legacy_unverifiable` if truth still cannot be recovered without guesswork.

Raw receipts or transcripts must never override canonical persisted columns.

## Stage and Approval Settlement

### Stage ownership

`StageExecution` remains the canonical owner of stage terminality.

Required stable fields:

- `lineageID`
- `settlementKind`
- `settledAt`
- `activeOwnerToken`

One logical stage lineage may not have more than one active stage execution at the same time.

### Approval ownership

Approval gates use the same lineage model:

- `Approval.lineageID`
- `Approval.repairedAt`

One logical approval lineage may not have more than one active approval record at the same time.

### Lineage propagation

- same-run retry keeps the same `lineageID`
- startup repair keeps the same `lineageID`
- clone run creates a new `lineageID`
- aggregate settlement inherits the aggregate stage lineage

## Create-Path Prevention and Startup Repair

### Prevention comes first

`ActiveExecutionUniquenessGuard` is the primary boundary. New stage or approval work must pass through it before active records are created.

### Startup repair comes second

`StartupSettlementRepair` exists to repair stale persisted truth after interruption or relaunch. It is not the normal prevention path.

Fail-closed expectations:

- stale `running` without durable evidence becomes explicit operator decision territory,
- stale `waitingApproval` restores the same approval lineage when possible,
- missing lineage is never guessed,
- conflicting legacy signals degrade to `legacy_unverifiable`.

## Aggregate Settlement

Aggregate work is first-class runtime truth.

The owner model is strict:

- aggregate-stage `StageExecution` owns stage terminality,
- `AggregateSettlementRecord` is subordinate evidence tied to the same lineage and stage execution,
- report and recovery readers must not infer aggregate truth from fan-out artifacts alone.

## Runtime Binding Truth

Operator surfaces must distinguish:

1. frozen binding intent,
2. runtime provider/model evidence,
3. trust level of that runtime evidence.

Current reporting and recovery surfaces use explicit frozen-vs-runtime summaries and may downgrade to `unverifiable` when runtime evidence is weak or contradictory.

Related stable reference: [provider-binding-truth.md](provider-binding-truth.md).

## Recovery Policy

Recovery must choose the narrowest valid next action from canonical settlement and recovery records.

Default policy:

- contract-valid same-run retry is allowed only when canonical settlement permits it,
- aggregate retry appears only when aggregate settlement is the blocker,
- limit exhaustion and provider policy-bound stops are non-auto-retryable by default,
- clone remains available when same-run continuation is unsafe or unverifiable,
- blocked historical or legacy runs may require explicit operator decision instead of automatic resume.

Related operator reference: [operator-experience.md](operator-experience.md).

## Proof Owners

The current proving story for this slice has two owners:

1. non-UI current-head proof lane in `Chainworks ForgeTests`
2. app-launched proof harness driven by `Proposal016ExecutionTruthHarness`

Operational entry point: [test-gates.md](test-gates.md) and the `proposal-016` gate.

Implementation/proof status: [../evidence/execution-truth-and-recovery-proof.md](../evidence/execution-truth-and-recovery-proof.md).
