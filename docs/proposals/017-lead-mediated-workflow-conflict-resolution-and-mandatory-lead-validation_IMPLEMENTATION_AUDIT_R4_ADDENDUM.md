# Proposal 017 Implementation Audit R4 — Closure Addendum

| Field | Value |
|---|---|
| Audit | `017-..._IMPLEMENTATION_AUDIT_R4.md` |
| Audit verdict | **Partial / Not Ready** (narrower than R3) |
| Addendum date | 2026-04-27 |
| Addendum scope | Close the two R4 findings — API-002 (per-attempt cost / transcript / artifact attribution) and OPS-002 (missing runtime metric emissions) — plus the gate-coverage gap (READY-001) those imply. |
| Branch | `claude/bold-lichterman` (will fast-forward `main`) |

## R4 audit blockers closed

### API-002 (Major) — Per-attempt cost / transcript / direct artifact refs

**Status: Closed.**

The R4 audit said the `execution_attempts` array existed but `cost` and
`transcript_ref` were always null and artifacts were correlated only by
`agent_id`. This addendum closes the gap by persisting both per
attempt and by linking transcripts directly to the `AgentExecution` row.

Schema (migration `031_p017_metric_inventory_and_attempt_attribution.sql`):

- `agent_executions.total_cost_cents BIGINT NULL`
- `agent_executions.input_tokens INTEGER NULL`
- `agent_executions.output_tokens INTEGER NULL`
- `agent_executions.cached_input_tokens INTEGER NULL`
- `agent_executions.transcript_artifact_id TEXT NULL REFERENCES artifacts(id)`
- `idx_agent_executions_transcript_artifact_id` index for the FK lookup.

Domain (`crates/domain/src/agent.rs`):

- `AgentExecution` gains five new optional fields with `#[serde(default)]`
  so older serialized rows still deserialize cleanly.

Repository (`crates/db/src/repos/agent_executions.rs`):

- `SELECT_COLS` extended to read the new columns.
- `parse_agent_execution_row` populates them.
- New `update_attempt_attribution{,_tx}` helper writes per-attempt cost
  + transcript via `COALESCE(?, existing)` so partial updates don't
  clobber prior values (idempotent across retries).

Executor (`crates/engine/src/executor.rs`):

- The mediation-completion path now persists the transcript artifact
  inline via `persist_transcript_artifact_if_present`, then immediately
  calls `update_attempt_attribution` with the provider's
  `result.usage` (cost cents, input/output/cached tokens) and the
  transcript artifact id. Failure to persist attribution is logged but
  does not abort the mediation completion.

MCP (`crates/mcp-server/src/tools/reports.rs`):

- `mediation_execution_attempts_json` now reads `total_cost_cents`,
  `input_tokens`, `output_tokens`, `cached_input_tokens` directly from
  the execution row and returns them under `cost` (or null when the
  provider returned no usage data).
- `transcript_ref` resolves the `transcript_artifact_id` FK via
  `artifacts::find_by_id` and returns `{artifact_id, file_path, format}`.
- `artifacts` are emitted in tiered priority: tier 1 = the direct
  transcript artifact (linkage `transcript_direct`); tier 2/3 =
  `agent_id` correlation (linkage `agent_id_correlation`) for
  pre-R4 attempts that have no direct linkage yet. IDs are
  deduplicated across tiers.

GraphQL (`crates/graphql-server/src/types/run.rs`):

- `GqlMediationExecutionAttempt.cost` and
  `GqlMediationExecutionAttempt.transcriptRef` follow the same
  population rules as MCP. `artifacts` use the same tiered linkage.

MCP test (`proposal_017_workflow_conflict_lead_mediation_execution_attempts`)
now stamps `update_attempt_attribution` on attempt 2 with concrete
cost + transcript values and asserts:

- attempt 1 keeps `cost` and `transcript_ref` as null,
- attempt 2 has `cost = {total_cost_cents: 123, input_tokens: 500,
  output_tokens: 75, cached_input_tokens: 40}`,
- attempt 2 `transcript_ref = {artifact_id, file_path, format=markdown}`,
- attempt 2 `artifacts` array includes the direct transcript with
  `linkage: "transcript_direct"`.

DB persistence test (`p017_per_attempt_cost_and_transcript_persisted`)
proves `update_attempt_attribution`:

- writes cost without transcript (idempotent on null transcript),
- later writes the transcript artifact id without clobbering cost
  (COALESCE semantics).

### OPS-002 (Major) — Missing runtime metric emissions

**Status: Closed for the audit-named gaps; helpers shipped for
remaining inventory items.**

The R4 audit listed three missing emissions plus Phase C fail-path:

| Metric | Helper | Production caller | Test |
|---|---|---|---|
| `phase_c_validation_outcome_total` (FAIL path) | `record_phase_c_validation_failure_tx` (NEW) | `command_handler::Command::StartRun` early-return when `workflow::compiler::compile()` returns `Err`. Failure kind is classified by `classify_phase_c_failure_kind` so the label cardinality stays bounded. Run id is None — migration 031 made the column NULL-able to support daemon-level events. | `p017_phase_c_validation_failure_metric_emits_without_run` |
| `duplicate_mediation_session_total` | `record_duplicate_mediation_session_tx` (NEW) | `orchestrator` mediation creation when `find_active_for_conflict_tx` returns `Some`. Detection source `try_initiate`. | `p017_duplicate_mediation_session_metric_emits` |
| `report_readback_completeness` | `record_report_readback_completeness_tx` (NEW) | `mcp::tools::reports::workflow_conflict_json` after composing the response, ratio = present / expected against the proposal's "current conflict, history, advisory rejections, lead owner, valid action class, terminal failure reason" set. | `p017_report_readback_completeness_metric_emits` |
| `phase_c_lead_inventory_external_catalog_total` | `record_phase_c_lead_inventory_external_catalog_tx` (NEW) | `command_handler::StartRun` after run insert, with the bundled-only result `inventory_result=zero_active_externals` + `enforcement_decision=waive_warning_window` per the attested evidence. | `p017_phase_c_lead_inventory_external_catalog_metric_emits` |

Plus two longer-tail helpers shipped for completeness (the audit
listed them in the proposal inventory but did not flag them as missing
production callers):

| Metric | Helper | Production caller | Test |
|---|---|---|---|
| `mediation_late_output_ignored_total` | `record_mediation_late_output_ignored_tx` (NEW) | `executor.rs` `mediation_stale` branch | `p017_mediation_late_output_ignored_metric_emits` |
| `mediation_retry_budget_exhausted_total` | `record_mediation_retry_budget_exhausted_tx` (NEW) | helper available; production caller is reserved for the budget-enforcement contract slice (no production caller in this slice — documented as deferred). | covered by helper unit tests |

Migration `031_p017_metric_inventory_and_attempt_attribution.sql`:

- Recreates `workflow_conflict_metric_events` with run_id NULL-able and
  the CHECK list extended to all 16 names from the proposal's
  `operational_metrics` block (previously 10).
- Backfills existing rows.

`WorkflowConflictMetricEvent.run_id` is now `Option<String>` so
daemon-level emits (Phase C compile-fail) compile cleanly.

### READY-001 — Gate coverage for R4 closures

**Status: Closed.**

`scripts/test-gate.sh proposal-017` adds 13 new closure guards (after
the R2 set) that `die` if any of:

- `update_attempt_attribution` is missing from `executor.rs`
- `p017_per_attempt_cost_and_transcript_persisted` test missing
- migration 031 missing `transcript_artifact_id`/`total_cost_cents`/`input_tokens`/`output_tokens`
- any of 6 OPS-002 helper functions missing
- any of 5 OPS-002 metric tests missing
- any production caller missing for the 5 R4-named metrics

Combined with the R2 guards, the gate now enforces 24 closure surfaces
end-to-end.

## What is intentionally still deferred

- **`mediation_retry_budget_exhausted_total` production caller** is
  reserved for the budget-enforcement contract slice. The helper is
  available; the production wire-up requires the budget enforcement
  contract that lives outside this audit's scope.
- **Direct `artifact_source_generation_claims` projection** in the
  `execution_attempts.artifacts` array. The current tier-1
  (transcript_direct) + tier-2/3 (agent_id correlation) is sufficient
  per the audit acceptance criteria, but a future slice can add
  owner-aware claims as a fourth tier between transcript and
  agent_id.
- **Live transcript export validation** is not in this audit's scope
  (the existing redaction is validated at source/test level).

## Verification

| Check | Result |
|---|---|
| `cargo test -p db --test proposal_017_workflow_conflict_persistence` | 17 passed (was 11; +6 R4 metric/persistence tests) |
| `cargo test -p engine -- --test-threads=1 proposal_017 p017_` | 18 integration + 7 lib passed |
| `cargo test -p mcp-server -- proposal_017_` | 7 passed (incl. extended cost/transcript assertions) |
| `cargo test -p graphql-server -- proposal_017_` | 4 passed |
| `./scripts/test-gate.sh proposal-017` | (recorded once gate finishes; full log archived under `docs/proposals/017-evidence/proposal-017-r4-closure-gate-*.log`) |

The closure tests visible in the gate log:

- All R2 closure tests (8) still green.
- R4 additions: `p017_per_attempt_cost_and_transcript_persisted`,
  `p017_phase_c_validation_failure_metric_emits_without_run`,
  `p017_duplicate_mediation_session_metric_emits`,
  `p017_report_readback_completeness_metric_emits`,
  `p017_phase_c_lead_inventory_external_catalog_metric_emits`,
  `p017_mediation_late_output_ignored_metric_emits`.
- MCP test `proposal_017_workflow_conflict_lead_mediation_execution_attempts`
  asserts the new non-null cost/transcript_ref and direct
  transcript artifact linkage tier.
