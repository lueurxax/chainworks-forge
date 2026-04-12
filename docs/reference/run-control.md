# Run Control

Stable reference for run-stop semantics, cancellation settlement, and operator-visible terminal truth that were previously tracked by Proposal 011.

## Purpose

Run control must be operationally trustworthy.

The operator must be able to:

- stop active work without guessing what happens next,
- distinguish `cancelling` from settled `cancelled`,
- trust that in-flight agent work was actually asked to stop,
- and see terminal history without confusing stop with archive.

This document defines the implemented run-control contract.

Related stable docs:

- [idea-lifecycle.md](idea-lifecycle.md)
- [operator-experience.md](operator-experience.md)
- [runtime-contract.md](runtime-contract.md)

## Scope

This reference covers:

- stop vs archive semantics,
- cancellation settlement,
- operator-visible cancellation state,
- persisted cancellation evidence,
- terminal-history rules for cancelled runs.

It does not define repo-backed release approval or delivery execution. That boundary remains in [full-mvp-delivery.md](full-mvp-delivery.md).

## Core rule

`Stop` and `Archive` are different actions.

- `Stop` is execution control.
- `Archive` is visibility control.

Archive never implies stop, and stop never implies archive.

An active idea must first settle its run into a terminal state before archive becomes eligible.

## Stop semantics

Stopping an active idea means:

- the active run stops advancing its state machine,
- in-flight agent executions receive cooperative cancellation,
- active runtime sessions are closed where available,
- the run remains visibly `cancelling` until settlement is confirmed,
- only then does the run become terminal `cancelled`.

`ExecutionService` and `RunCancellationCoordinator` own this path.

## Cancellation settlement

Cancellation is settled only when all of the following are true:

1. the orchestrator has stopped advancing workflow state,
2. every agent execution that was running at request time is now terminal,
3. every open runtime session has a recorded close outcome,
4. the run has both request and settlement timestamps plus structured settlement evidence.

Persisted run-level fields:

- `cancellationRequestedAt`
- `cancellationSettledAt`
- `cancellationSettlementLog`

Settlement log entries record:

- agent execution id,
- prior status,
- terminal status,
- whether session close was attempted,
- whether session close succeeded,
- settlement timestamp.

## Operator-visible truth

Run surfaces must distinguish:

- `running`
- `cancelling`
- `cancelled`
- `failed`
- `blocked`

A run with `cancellationRequestedAt != nil` and `cancellationSettledAt == nil` is not allowed to present as ordinary terminal `cancelled`.

The operator path lives in `Ideas`:

1. open idea,
2. choose `Stop Run`,
3. confirm stop,
4. observe `cancelling`,
5. later observe settled `cancelled` history.

The confirmation surface must explicitly say that:

- artifacts remain intact,
- reports remain intact,
- receipts remain intact,
- history is preserved.

## Terminal history

Cancelled runs remain first-class history.

Rules:

- cancelled runs stay visible in run-centric surfaces,
- archive eligibility remains separate,
- reports and artifacts are not rewritten after cancellation,
- recovery/archive actions must not erase cancellation truth.

## Contracts consumed by other docs

This run-control baseline is assumed by:

- [idea-lifecycle.md](idea-lifecycle.md) for archive eligibility,
- [operator-experience.md](operator-experience.md) for run lists, reports, and recovery surfaces,
- [project-workspace-contract.md](project-workspace-contract.md) for fail-closed project-backed execution,
- [provider-binding-truth.md](provider-binding-truth.md) for truthful historical run explanation.
