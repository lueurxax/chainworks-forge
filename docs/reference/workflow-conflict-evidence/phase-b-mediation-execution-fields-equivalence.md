# Mediation Execution Owner Schema Equivalence Record

| Field | Value |
|---|---|
| Retained historical alias | `proposal-017` |
| Schema migration of record | `control-plane/crates/db/migrations/029_p017_nullable_mediation_stage_execution.sql` (retained historical alias) |
| CHECK constraint of record | `control-plane/crates/db/migrations/029_p017_nullable_mediation_stage_execution.sql:42-46` (retained historical alias) |
| Equivalence test | `control-plane/crates/engine/tests/integration.rs::p017_mediation_execution_fields_equivalence` (retained historical alias) |

## Recorded Design Decision

The owner-kind schema deliberately keeps run ownership derivable from canonical
parent records instead of duplicating it on every `agent_executions` row. This
record is paired with an executable proof test that exercises cancellation,
readback, and idempotency through the substitute fields and the existing schema.

## Rejected Literal-Column Shape

Two literal columns on `agent_executions`:

1. `run_id` — direct, denormalised pointer to the owning run.
2. `mediation_owner_token` — a stable opaque token for the mediation owner
   across attempts.

## Why we did not add literal columns

### `run_id` is unambiguously derivable from existing fields

Migration `029_p017_nullable_mediation_stage_execution.sql:42-46` (retained historical alias) enforces:

```sql
CHECK (
    (owner_kind = 'stage_execution'
       AND stage_execution_id IS NOT NULL
       AND owner_id = stage_execution_id)
    OR
    (owner_kind = 'lead_conflict_mediation'
       AND stage_execution_id IS NULL
       AND owner_id IS NOT NULL)
)
```

So **every** `agent_executions` row falls into exactly one of two cases:

- **Stage-owned**: `stage_execution_id IS NOT NULL`, and
  `stage_executions.run_id` is the owning run.
- **Mediation-owned**: `lead_mediation_record_id IS NOT NULL`, and
  `lead_conflict_mediations.run_id` is the owning run.

Both paths are 1:1 with the row, indexed, and enforced by `NOT NULL` +
referential constraints in their respective tables. There is no row where
`run_id` is undefined or ambiguous.

The cost of an extra column would be:
- One extra denormalised value per row that **must** be kept in sync with
  the join path under all migration paths and recovery procedures.
- A backfill that is itself a join.
- A new place to drift out of sync if the parent run id ever changed (it
  doesn't today, but adding a column makes that more likely tomorrow).

The benefit would be:
- One fewer JOIN in the cancellation cascade path
  (`agent_executions::cancel_running_by_run_tx`).
- One fewer JOIN in `agent_executions::list_by_run`.

The repos that need the cancellation cascade and per-run readback paths
already perform the JOIN once, with full test coverage, and the JOIN cost
is dominated by SQLite's index lookups. The denormalised column would be
strictly redundant.

### `mediation_owner_token` is satisfied by `lead_mediation_record_id`

The rejected `mediation_owner_token` field is a stable token for the mediation
owner across attempts. The current schema already has such a token:
`lead_mediation_record_id` (TEXT, foreign key to
`lead_conflict_mediations.id`). It is:

- **Stable**: `lead_conflict_mediations.id` is assigned on creation and
  never changes.
- **Per-mediation**: each `LeadConflictMediationRecord` gets a unique id;
  every `agent_executions` row owned by that mediation carries the same
  id.
- **Across attempts**: a retry of a mediation owner keeps the same
  `lead_mediation_record_id` and inserts a new `agent_executions` row;
  the equivalence test asserts this is what `list_by_mediation_id`
  returns.

Renaming the field to `mediation_owner_token` would be a cosmetic change
without behavior delta. We document it here instead.

## What we proved

The integration test `p017_mediation_execution_fields_equivalence` (retained historical alias, in
`control-plane/crates/engine/tests/integration.rs`) constructs:

- A stage-owned `agent_executions` row.
- A mediation-owned `agent_executions` row.
- A second mediation-owned `agent_executions` row attached to the same
  mediation (a retry attempt).

…and asserts:

1. **Direct identity**: each row's owning run id is recoverable through
   the canonical paths (`stage_executions.run_id` for stage-owned;
   `lead_conflict_mediations.run_id` for mediation-owned), with the **same
   value** that an explicit `agent_executions.run_id` column would have.
2. **Cancellation**: `engine::cancellation::begin_settlement` cancels every
   `agent_executions` row for the run regardless of owner kind, AND
   cascades to active `lead_conflict_mediations` rows in the same
   transaction. (REL-001 closure cross-link.)
3. **Readback**: `agent_executions::list_by_run` returns both stage-owned
   and mediation-owned rows for the run; `list_by_mediation_id` returns
   only the two mediation-owned rows, ordered by `started_at` ASC, and
   both share the same `lead_mediation_record_id` (the
   `mediation_owner_token` equivalent).
4. **Idempotency**: re-running the cancellation cascade with the same
   timestamp is a no-op for already-terminal rows; the
   `lead_mediation_record_id` does not change across attempts.

All four assertions pass against the current schema **without** the literal
columns. Running the test under the canonical gate
(`./scripts/test-gate.sh proposal-017`, retained historical alias) is part of the closure proof.

## Why this is not "kicking the can"

A future schema migration may add `run_id` as a denormalised column for
performance reasons unrelated to owner-kind correctness. If/when that happens,
the migration should:

- Backfill via the same JOIN paths documented above.
- Add a CHECK or trigger to keep the denormalised value in sync with the
  parent.
- Update `cancel_running_by_run_tx` and `list_by_run` to use the column
  directly.

That migration is not required for correctness because it is a pure performance
optimisation. The correctness story is proven equivalent today.

## Status

The owner-kind equivalence is covered by this record and the
`p017_mediation_execution_fields_equivalence` proof test (retained historical
alias), both of which are required by the retained historical alias
`proposal-017` gate.
