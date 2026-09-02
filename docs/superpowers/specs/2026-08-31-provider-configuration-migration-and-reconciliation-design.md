# Provider Configuration Migration and Reconciliation

Date: 2026-08-31
Status: Deferred roadmap source; not implementation-approved
Source checkpoint: `acf85de1`
Inherited findings: P1-01, P1-05
Reserved focused gate: `provider-configuration-migration`

## Purpose

Own the storage migration, bootstrap, and restart reconciliation needed by a
future durable provider-configuration authority. It is separate from the
implemented [planned-binding contract](../../reference/provider-binding-truth.md),
which adds no authority tables.

## Owned scope

- A registry manifest containing every and only Class A authority row.
- Preservation and explicit exclusion of Class B, C, and D rows.
- Immutable pending intents with append-only terminal successor evidence.
- One terminal outcome vocabulary and deterministic result/replay mapping.
- Clean-install and upgrade manifests for every table, index, trigger, and
  registered operation.
- Bootstrap lock, migration journal, startup ordering, bounded reconciliation,
  crash checkpoints, and failed-serve behavior.
- One canonical migration terminal phase; the inherited
  `final_swap_committed` versus `complete` contradiction must not recur.
- Commit-before-ack and restart convergence for each registered operation.

## Required proof when scheduled

- Class A filtered-manifest parity and zero mutation of all excluded rows.
- Append/update/delete mutation negatives.
- Crash injection before and after every durable phase transition.
- Clean database, previous-version database, malformed schema, duplicate
  registry, and partial-migration fixtures.

## Activation rule

This source must be split again if storage migration and runtime reconciliation
cannot fit one sub-2,000-line proposal. It confers no current migration or
schema authority.
