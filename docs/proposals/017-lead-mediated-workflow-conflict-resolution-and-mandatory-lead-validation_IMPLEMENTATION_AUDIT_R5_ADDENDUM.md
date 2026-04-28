# Proposal 017 Implementation Audit R5 — Closure Addendum

| Field | Value |
|---|---|
| Audit | `017-..._IMPLEMENTATION_AUDIT_R5.md` |
| Audit verdict | **Partial / Not Ready** (narrower than R4) |
| Addendum date | 2026-04-27 |
| Addendum scope | Close all three R5 findings — API-003 (cross-retry artifact isolation), REL-002 (atomic completion+attribution), OPS-003 (4 schema-only metric names) — plus the gate-coverage gap (READY-001). |
| Branch | `claude/bold-lichterman` (will fast-forward `main`) |

## R5 audit blockers closed

### API-003 (Major) — Direct non-transcript artifact linkage per attempt

**Status: Closed.**

R5 audit said `transcript_artifact_id` was direct but other artifacts
relied on `agent_id` correlation, which over-included artifacts across
retries by the same lead agent. Closed by direct
`artifacts.agent_execution_id` FK and a new tier in MCP/GraphQL
`execution_attempts.artifacts`.

Schema (migration `032_p017_per_attempt_artifact_linkage.sql`):
- `artifacts.agent_execution_id TEXT NULL REFERENCES agent_executions(id)`
- `idx_artifacts_agent_execution_id` index for the per-attempt readback.

Domain (`crates/domain/src/artifact.rs`):
- `Artifact.agent_execution_id: Option<String>` with `#[serde(default)]`.

Repository (`crates/db/src/repos/artifacts.rs`):
- Insert path persists `agent_execution_id`.
- Parse populates the field.
- New `list_by_agent_execution(pool, &str) -> Vec<Artifact>` powers the
  per-attempt readback.

Executor (`crates/engine/src/executor.rs`):
- `prepare_declared_output_artifacts` returns `Vec<Artifact>` and the
  caller stamps `agent_execution_id = Some(agent_exec_id.to_string())`
  on every entry before insert.
- Transcript persistence already populated `agent_execution_id` as
  part of R4; same field now lights up the direct-FK tier on every
  produced artifact.

MCP (`crates/mcp-server/src/tools/reports.rs`) and GraphQL
(`crates/graphql-server/src/types/run.rs`):
- Three-tier attempt-artifact attribution:
  - **Tier 1 — `transcript_direct`**: `agent_executions.transcript_artifact_id`.
  - **Tier 2 — `execution_id_direct`** (NEW): `artifacts.agent_execution_id`
    via `list_by_agent_execution`.
  - **Tier 3 — `agent_id_correlation`**: legacy fallback, only used
    when tier 1 + tier 2 produce zero artifacts (pre-R5 attempts).
- Cross-retry isolation is the tier-2 contract: retries by the same
  lead agent produce different `agent_execution_id`s and therefore
  disjoint artifact sets.

### REL-002 (Major) — Atomic completion + attribution

**Status: Closed.**

R5 audit flagged that `update_completed` ran in one transaction and
`update_attempt_attribution` in another, with attribution failures
logged-and-ignored. A crash window between the two could leave a
mediation attempt without cost/transcript attribution.

Closed by collapsing both writes into a single transaction in the
executor's mediation completion path:

```rust
let mut completion_tx =
    db::pool::begin_immediate_with_retry(&self.pool,
        "mediation.complete_with_attribution").await?;
agent_executions::update_completed_tx(&mut completion_tx, agent_exec_id,
    result.status.clone(), completed_at).await?;
agent_executions::update_attempt_attribution_tx(&mut completion_tx,
    agent_exec_id, usage_cost_cents, usage_input_tokens,
    usage_output_tokens, usage_cached_input_tokens,
    mediation_transcript_artifact_id.as_deref()).await?;
completion_tx.commit().await?;
```

Either both writes land together or neither does — a re-driven work
item observes a still-running execution and rebuilds attribution
fresh. Attribution failure now propagates instead of being suppressed.

Transcript artifact creation (filesystem write + `artifacts::insert`)
still runs before the tx because the filesystem write is not
transactional; if the artifact insert succeeded but the tx fails, the
attribution column won't reference it, but the tier-2 direct FK on
`artifacts.agent_execution_id` still attributes the artifact to this
execution attempt — preserving cross-retry isolation.

### OPS-003 (Major) — 4 missing metric emissions

**Status: Closed.**

R5 audit listed 4 schema-only metric names. Each now has a helper +
production caller + test:

| Metric | Helper | Production caller | Test |
|---|---|---|---|
| `advisory_rejection_total` | `record_advisory_rejection_tx` | `workflow_conflicts::insert_advisory_rejection` (every advisory rejection insert) | `p017_advisory_rejection_metrics_emit` |
| `invalid_next_stage_hint_non_blocking_total` | `record_invalid_next_stage_hint_non_blocking_tx` | same caller, conditional on `graph_membership_result == "absent_from_graph"` | same test (asserts both metrics emit together) |
| `workflow_conflict_current_total` | `record_workflow_conflict_current_tx` | `workflow_conflicts::upsert_conflict_by_fingerprint_tx` (per-upsert, bounded labels: reason × status) | `p017_workflow_conflict_current_metric_emits` |
| `terminal_unverifiable_total` | `record_terminal_unverifiable_tx` | `workflow_conflicts::record_terminal_metric_events_tx` when status transitions to TerminalUnverifiable | `p017_terminal_unverifiable_metric_emits` |

`insert_advisory_rejection` was extended to wrap the rejection insert
+ both metric emits in a single transaction so they can never split.
The `workflow_conflict_current_total` emit is keyed by
`(reason, status)` — bounded to 8 × 6 = 48 unique label tuples.

### READY-001 — Gate guards for R5 closures

**Status: Closed.**

`scripts/test-gate.sh proposal-017` adds 16 new closure guards (after
the R4 set) that `die` if any of:

- migration 032 missing `agent_execution_id` column
- `list_by_agent_execution` missing from artifacts repo
- MCP / GraphQL `execution_attempts.artifacts` missing the
  `list_by_agent_execution` call
- `execution_id_direct` linkage label missing from MCP tier-2
- `mediation.complete_with_attribution` tx label missing from executor
- `update_attempt_attribution_tx` (transactional variant) missing
- any of 4 OPS-003 helper functions missing
- any of 3 OPS-003 metric tests missing
- `record_terminal_unverifiable_tx` not called in `record_terminal_metric_events_tx`

Combined with the R2/R4 guards, the gate now enforces ~40 closure
surfaces end-to-end.

## Verification

| Check | Result |
|---|---|
| `cargo test -p db --test proposal_017_workflow_conflict_persistence` | 20 passed (was 17; +3 R5 metric tests) |
| `cargo test -p engine -- proposal_017_ p017_` | 18 integration + 7 lib passed |
| `cargo test -p mcp-server -- proposal_017_` | 7 passed |
| `cargo test -p graphql-server -- proposal_017_` | 4 passed |
| `./scripts/test-gate.sh proposal-017` | (recorded once gate finishes; full log archived under `docs/proposals/017-evidence/proposal-017-r5-closure-gate-*.log`) |

## What is still deferred (recorded)

- **`mediation_retry_budget_exhausted_total` production caller** —
  helper exists, production wire-up still waiting on the budget
  enforcement contract slice (consistent with R4 deferral).
- **Cross-language fixture parity** for the artifact attribution
  tiered-readback model is not in P017 scope.
