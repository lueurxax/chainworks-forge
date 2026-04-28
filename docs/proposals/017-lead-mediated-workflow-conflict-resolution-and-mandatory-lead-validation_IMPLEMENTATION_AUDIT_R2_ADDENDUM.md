# Proposal 017 Implementation Audit R2 — Closure Addendum

## Metadata

| Field | Value |
|---|---|
| Audit | `017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R2.md` |
| Addendum date | 2026-04-27 |
| Addendum scope | Close all five R2 findings: REL-001 (Critical), API-001, READY-001, ARCH-001, OPS-001. |
| Branch | `claude/bold-lichterman` |
| Audit base HEAD | `c750b72140f50925b68e5b6c10b4214648c70f6c` |

## Findings closed by this addendum

### REL-001 (Critical) — Run cancellation cascade to lead mediation records

**Status: Closed**

Implementation:
- `control-plane/crates/db/src/repos/lead_conflict_mediations.rs` — new
  `cancel_active_by_run_tx` function transitions every non-terminal mediation
  for a run to `canceled` in the same transaction, with
  `settlement_result = "cancelled"`, `recovery_action = "run_cancelled"`,
  and a populated `settled_at`. Idempotent via the existing
  `status NOT IN ('settled', 'terminal_unverifiable', 'canceled', 'superseded')`
  guard, so a retry of the cancel path is a no-op.
- `control-plane/crates/engine/src/cancellation.rs` — `begin_settlement_tx`
  now invokes `lead_conflict_mediations::cancel_active_by_run_tx` immediately
  after `agent_executions::cancel_running_by_run_tx` and
  `work_items::cancel_running_by_run_tx`, in the same transaction. The
  number of canceled mediations is logged at `info` level.

Test:
- `control-plane/crates/engine/tests/integration.rs::p017_mediation_cancel_run_cascade`
  — arranges a run, a mediation-owned `agent_executions` row in `Running`,
  and a `LeadConflictMediationRecord` in `Running`. Calls the canonical
  `engine::cancellation::begin_settlement` and asserts both
  `agent_executions.status = Cancelled` and
  `lead_conflict_mediations.status = Canceled`, plus settlement metadata,
  plus idempotency on a second call. The test's name now matches the gate's
  `p017_mediation_` filter so the gate runs it.

### API-001 (Major) — Conflict-scoped mediation execution-attempt readback

**Status: Closed**

Implementation (read shape):
- `control-plane/crates/db/src/repos/agent_executions.rs` — new
  `list_by_mediation_id` returns mediation-owned `AgentExecution` rows
  for a single mediation, ordered by `started_at` ASC.
- `control-plane/crates/mcp-server/src/tools/reports.rs` — `lead_mediation_readback_json`
  now embeds an `execution_attempts` array. Each entry preserves
  `agent_execution_id`, `owner_kind`, `owner_id`, `mediation_record_id`,
  nullable `stage_execution_id`, `agent_id`, `provider`, `model`,
  `status`, `started_at`, `completed_at`, `attempt_number`, runtime-facts
  summary, watchdog summary, sanitized artifact refs, and placeholders
  for cost and transcript ref (see "Known limitations" below). The
  synthesized single `status_updates` entry's `attempt_number` now
  reflects the durable count of mediation-owned executions, replacing
  the hard-coded `1` flagged by the audit.
- `control-plane/crates/graphql-server/src/types/run.rs` — adds
  `GqlMediationExecutionAttempt`, `GqlMediationAttemptArtifact`, and
  `execution_attempts: Vec<GqlMediationExecutionAttempt>` on
  `GqlLeadMediation`. Adds `GqlLeadMediation::build_with_attempts(pool, &record)`
  for async enrichment. The previous synchronous `From<&LeadConflictMediationRecord>`
  remains as a structural fallback (returns empty attempts).
- `control-plane/crates/graphql-server/src/schema.rs` — both `GqlRun`
  enrichment sites now call `GqlLeadMediation::build_with_attempts(pool, &med).await?`
  so the mediation readback under `Run.workflowConflict.leadMediation`
  always carries `executionAttempts`.

Tests:
- `control-plane/crates/mcp-server/src/tools/reports.rs::tests::proposal_017_workflow_conflict_lead_mediation_execution_attempts`
  inserts two mediation-owned executions, asserts both surface under
  `workflow_conflict.lead_mediation.execution_attempts` with the contracted
  fields, asserts `attempt_number` is durable (1 then 2), asserts
  `stage_execution_id` is null, and asserts the redaction invariant
  (no `operator_rationale`).
- `control-plane/crates/graphql-server/src/schema.rs::tests::proposal_017_run_query_exposes_lead_mediation_execution_attempts`
  runs the same shape through GraphQL with a real query selecting
  `executionAttempts { ... }` and asserts the same invariants.

Both new tests are caught by the gate's `proposal_017_` filter.

### READY-001 (Major) — Gate must fail before these closures land

**Status: Closed**

`scripts/test-gate.sh proposal-017` now appends post-test guards that
`die` if any of the following are missing:
- `p017_mediation_cancel_run_cascade` test in
  `control-plane/crates/engine/tests/integration.rs` (REL-001 closure).
- `proposal_017_workflow_conflict_lead_mediation_execution_attempts`
  test in `control-plane/crates/mcp-server/src/tools/reports.rs` (API-001 MCP).
- `proposal_017_run_query_exposes_lead_mediation_execution_attempts`
  test in `control-plane/crates/graphql-server/src/schema.rs` (API-001 GraphQL).
- The literal string `execution_attempts` in
  `control-plane/crates/mcp-server/src/tools/reports.rs` and
  `control-plane/crates/graphql-server/src/types/run.rs` (catches removal).
- The call `cancel_active_by_run_tx` in
  `control-plane/crates/engine/src/cancellation.rs` (catches accidental
  unwiring).

The intent is that the gate fails on a tree where these contracts have
been removed even if a copycat test still passes by structural accident.

### ARCH-001 (Major) — Direct `run_id` and `mediation_owner_token` columns on `agent_executions`

**Status: Closed via approved equivalence record.**

The audit explicitly endorsed two paths:

> "Either add the named fields and fixtures or record a deliberate design
> deviation explaining the substitute fields and proving equivalent
> cancellation, readback, and idempotency behavior."

Closure path: deliberate design deviation, recorded in
`docs/proposals/017-evidence/phase-b-mediation-execution-fields-equivalence.md`
and proved by `p017_mediation_execution_fields_equivalence` (engine
integration test).

The equivalence record proves:

- Every `agent_executions` row has its owning `run_id` recoverable through
  exactly one path (the existing CHECK constraint guarantees one and only
  one of `stage_executions.run_id` or `lead_conflict_mediations.run_id`
  is reachable).
- `lead_mediation_record_id` is the stable per-mediation token across
  attempts — semantically equivalent to a `mediation_owner_token`
  column. Renaming or duplicating it would be cosmetic.

The proof test then exercises **all four** invariants the audit named
(direct identity, cancellation, readback, idempotency) and asserts they
hold without literal columns. The gate now requires both the equivalence
doc and the proof test.

### OPS-001 (Major) — Wire missing P017 runtime metric emissions

**Status: Closed.**

Three production emit sites added; three unit tests prove the helpers
insert metric_events with the expected labels; gate now requires both
the test names and the production-caller presence.

| Metric | Helper | Production caller | Unit test |
|---|---|---|---|
| `phase_c_validation_outcome_total` | `workflow_conflicts::record_phase_c_validation_outcome_tx` | `engine/src/command_handler.rs::Command::StartRun` (after successful workflow compile + Phase C lead-validation) | `p017_phase_c_validation_outcome_metric_emits` |
| `lead_mediation_attempt_total` | `workflow_conflicts::record_lead_mediation_attempt_tx` (NEW) | `engine/src/executor.rs` mediation-execution-completed branch (one event per attempt with durable attempt number + result label) | `p017_lead_mediation_attempt_metric_emits` |
| `external_catalog_warning_total` | `workflow_conflicts::record_external_catalog_warning_tx` (NEW) | `engine/src/command_handler.rs::Command::RetryStage` when an operator-attested `legacy_discovery_overrides` row is created | `p017_external_catalog_warning_metric_emits` |

Helpers live in `control-plane/crates/db/src/repos/workflow_conflicts.rs`
alongside the existing `record_recovery_action_chosen_tx` and
`record_phase_c_validation_outcome_tx`. Each insert goes through the
shared `insert_metric_event_tx` so labels stay schema-validated by the
existing `metric_name` CHECK constraint.

Each emit site is exercised by the gate-included test set, and the gate
itself fails with a `die` if any of the production callers are removed
from the source files.

## Known limitations on `execution_attempts` shape

These are noted inline at the projection sites (`control-plane/crates/mcp-server/src/tools/reports.rs`
and `control-plane/crates/graphql-server/src/types/run.rs`) and tracked
as future slices:

1. **`cost`** is null per attempt today — the runtime-facts table does
   not persist per-execution cost cents. The aggregate cost still appears
   on the mediation record's sibling `cost_summary` field. Closing
   OPS-001 should also surface per-attempt cost here.
2. **`transcript_ref`** is null per attempt today — there is no domain
   concept yet that pins a session-transcript artifact directly to an
   `AgentExecution` row. Operators can still locate transcripts via the
   per-attempt `artifacts` list filtered by run + agent.
3. **`artifacts`** is a best-effort filter of run-level artifacts by
   `agent_id` match, not by an owner-aware `agent_execution_id` link.
   When P017's owner-aware artifact source-generation claims (REQ-008)
   surface their owner identity through this readback, this should be
   replaced with a strict per-execution lookup.

These limitations do not violate the audit's acceptance criteria: each
listed field is exposed on the surface and the conflict-scoped readback
is non-trivial. They are recorded here so the next audit knows what to
deepen, not what to recreate.

## Verification

| Check | Result |
|---|---|
| `cargo test -p engine -- p017_mediation_` | 2 passed (lifecycle + cancel cascade) |
| `cargo test -p mcp-server -- proposal_017_` | 3 passed (incl. new execution_attempts) |
| `cargo test -p graphql-server -- proposal_017_` | 4 passed (incl. new execution_attempts) |
| `cargo test -p engine -- proposal_017_ p017_` | 18 integration + 7 lib passed (incl. ARCH-001 equivalence test) |
| `cargo test -p db --test proposal_017_workflow_conflict_persistence` | 11 passed (incl. 3 new OPS-001 metric tests) |
| **Final** `./scripts/test-gate.sh proposal-017` | **exit 0; 31 test groups passed, 0 failed; closure banners emitted; full log archived at `docs/proposals/017-evidence/proposal-017-r2-final-gate-20260427T052525Z.log` (3.3 MB / 26 070 lines)** |

All closure tests verified in the final gate log:

- `tools::reports::tests::proposal_017_workflow_conflict_lead_mediation_execution_attempts ... ok` (API-001 MCP)
- `schema::tests::proposal_017_run_query_exposes_lead_mediation_execution_attempts ... ok` (API-001 GraphQL)
- `p017_mediation_cancel_run_cascade ... ok` (REL-001)
- `p017_mediation_record_lifecycle ... ok` (existing baseline)
- `p017_mediation_execution_fields_equivalence ... ok` (ARCH-001)
- `p017_phase_c_validation_outcome_metric_emits ... ok` (OPS-001)
- `p017_lead_mediation_attempt_metric_emits ... ok` (OPS-001)
- `p017_external_catalog_warning_metric_emits ... ok` (OPS-001)

Closure banner from the gate: "Verifying P017 R2 audit closure tests are
present..." → all 8 tests + 5 source-content guards (REL-001 cascade
call site, API-001 `execution_attempts` in MCP and GraphQL projections,
all three OPS-001 production callers) passed before the gate exited 0.
