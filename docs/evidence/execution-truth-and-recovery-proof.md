# Execution Truth and Recovery Proof

Current implementation and proof status for the execution-truth / settlement / recovery slice in the current Chainworks Forge baseline.

## Status

| Field | Value |
|---|---|
| Slice | Execution Truth and Recovery |
| Source contract | [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) |
| Current implementation status | Implemented |
| Current readiness | Ready |
| Primary evidence owner | current-head non-UI proof lane plus app-launched Proposal 016 harness |
| Last consolidated audit | `R4` on `2026-03-30` |

## What is considered proven

The accepted proof set now supports these claims:

- each agent attempt settles to one canonical terminal outcome, including truthful cancellation and limit exhaustion,
- stage and approval create paths enforce one active lineage owner at a time,
- startup repair collapses stale active records before new work begins,
- aggregate settlement is persisted as first-class subordinate truth instead of being inferred from fan-out artifacts,
- report and recovery surfaces show frozen-vs-runtime binding truth and downgrade weak runtime evidence to `unverifiable`,
- legacy backfill is deterministic when possible and fail-closed when not,
- app-level proof demonstrates repair, exhaustion, policy-stop narrowing, and truthful downgrade behavior on the current head.

## Accepted current-head proof set

The current proof story rests on three pillars:

1. green current-head non-UI proposal slice,
2. green historical replay and legacy-backfill owners inside that same slice,
3. green app-launched `Proposal016ExecutionTruthHarness` proof.

### Current-head non-UI proof lane

Accepted current-head lane:

- `Proposal016Tests`
- `ActiveExecutionUniquenessGuardTests`
- `RuntimeBindingTruthSummaryTests`
- `LegacyExecutionTruthBackfillTests`
- `HistoricalRunReplayTests`
- `RunCancellationCoordinatorTests`
- `ResumeManagerTests`
- `RecoveryCoordinatorTests`
- `OrchestratorTests`
- `Proposal013Tests`

The latest accepted same-head slice passed `116` tests across `10` suites during audit `R4`.

### App-level proof

Accepted app-level proof comes from the `Proposal016ExecutionTruthHarness` autorun path and proves:

- startup repair leaves one canonical active owner,
- limit exhaustion preserves durable output truth,
- provider policy-bound stops suppress default same-run retry,
- legacy rows without canonical truth remain `unverifiable`,
- standard report/recovery surfaces expose frozen-vs-runtime binding truth honestly.

## Consolidation note

The original Proposal 016 draft, review, evidence, research, and implementation-audit files were transitional implementation artifacts.

They have been removed after consolidation into:

- [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md)
- this proof document

The stable reference/evidence pair above is now the only long-lived documentation surface for this slice.

## Current interpretation

Execution truth and recovery should now be treated as implemented baseline behavior, not an active proposal.

The proposal lineage showed a clear progression:

1. early rounds identified ambiguous transport outcome truth, stale lineage drift, and aggregate invisibility,
2. mid rounds closed storage ownership, limit-exhaustion, cancellation, and startup-repair gaps,
3. later rounds shifted from behavior gaps to proof-lane quality,
4. the final consolidated audit marked the slice `Implemented` and `Ready`.

## Remaining caution

The remaining caution is operational, not contractual:

- the `proposal-016` wrapper gate refuses to start while unrelated test/app processes are already running on the host,
- some proof artifacts are current-head snapshots and should be reproved on later heads instead of being inherited by assumption.

That caution does not reopen the execution-truth contract.
It only narrows how far one current-head proof bundle should be generalized without rerun.

## Recommended usage

Use:

- [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) for the stable runtime contract,
- [../reference/runtime-contract.md](../reference/runtime-contract.md) for the adjacent frozen snapshot and artifact boundary,
- [../reference/provider-binding-truth.md](../reference/provider-binding-truth.md) for the narrower binding-provenance contract,
- [../reference/run-control.md](../reference/run-control.md) for the cancellation/control layer.

Do not recreate proposal-local duplicates for this slice unless a future change genuinely needs a new delta proposal.
