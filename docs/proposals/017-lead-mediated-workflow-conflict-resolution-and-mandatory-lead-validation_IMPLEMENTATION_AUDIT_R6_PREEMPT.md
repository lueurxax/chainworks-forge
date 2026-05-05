# Proposal 017 Implementation Audit R6 — Preemptive Closure Addendum

| Field | Value |
|---|---|
| Source audit | `017-..._IMPLEMENTATION_AUDIT_R5.md` (verdict: Partial / Not Ready) |
| Closure mode | **Preemptive** — gaps identified in the R5 verification log are closed before the R6 audit runs |
| Addendum date | 2026-04-28 |
| Branch | `claude/bold-lichterman` (synchronized with `main`) |
| Scope | Three anticipated R6 findings: GraphQL evidence parity for per-attempt cost/transcript (named explicitly in R5 verification log), cross-retry artifact isolation acceptance tests (named in R5 API-003 acceptance criteria), and a direct transactional-boundary proof for REL-002 (R5 atomicity claim was source-only) |

## Anticipated R6 findings closed

### GQL-PARITY-001 — GraphQL P017 test must assert non-null cost/transcript

**Status: Closed (preempt).**

The R5 verification log named this asymmetry explicitly:

> "GraphQL queries include the cost/transcript fields, but the GraphQL
> P017 test does not assert non-null populated values the way the MCP
> test does."

`proposal_017_run_query_exposes_lead_mediation_execution_attempts` in
`crates/graphql-server/src/schema.rs` now mirrors the MCP test
attempt-by-attempt:

- Inserts a `session_transcript` artifact via `artifacts::insert`,
  stamped with `agent_execution_id = exec_two_id` (tier-2 direct FK).
- Calls `agent_executions::update_attempt_attribution(exec_two_id,
  Some(123), Some(500), Some(75), Some(40), Some(&transcript_id))`.
- Asserts attempt 1's `cost` and `transcriptRef` are null.
- Asserts attempt 2's `cost.total_cost_cents == 123`,
  `cost.input_tokens == 500`, `cost.output_tokens == 75`,
  `cost.cached_input_tokens == 40`.
- Asserts attempt 2's `transcriptRef.artifact_id == <inserted id>` and
  `transcriptRef.format == "markdown"`.
- Asserts attempt 2's `artifacts[]` contains an entry with
  `linkage == "transcript_direct"`.

This is a one-for-one mirror of the MCP test
`proposal_017_workflow_conflict_lead_mediation_execution_attempts`
that already proved non-null populated values for the MCP surface in
R4/R5. Both northbound surfaces now have parity evidence.

### API-003-ACC-TEST — Cross-attempt artifact isolation acceptance test

**Status: Closed (preempt).**

R5 closed API-003 by adding the tier-2 direct FK
(`artifacts.agent_execution_id`) and the `execution_id_direct`
linkage label, but the closure test only proved attempt 2 surfaced
its own transcript. The R5 audit acceptance criteria for API-003
explicitly named:

> "MCP and GraphQL tests create two attempts by the same lead agent
> with distinct output artifacts and prove each attempt shows only
> its own direct artifact refs."

Two new dedicated cross-attempt isolation tests close that gap:

| Surface | Test |
|---|---|
| MCP | `p017_cross_attempt_artifact_isolation_via_mcp_readback` (in `crates/mcp-server/src/tools/reports.rs`) |
| GraphQL | `p017_cross_attempt_artifact_isolation_via_graphql_readback` (in `crates/graphql-server/src/schema.rs`) |

Each test does the following:

1. Seeds two `agent_executions` rows with the **same** `agent_id`
   (`"lead-agent-shared"`) — the pre-R5 over-inclusion failure mode.
2. Inserts artifact A linked to attempt 1's `agent_execution_id` and
   artifact B linked to attempt 2's `agent_execution_id`.
3. Reads back the readback (MCP `reports.get` / GraphQL `run.workflowConflict.leadMediation.executionAttempts`).
4. Asserts attempt 1's artifacts contain artifact A with linkage
   `execution_id_direct` and **do not** contain artifact B.
5. Asserts attempt 2's artifacts contain artifact B with linkage
   `execution_id_direct` and **do not** contain artifact A.
6. Asserts no entry in either attempt uses the legacy
   `agent_id_correlation` tier-3 fallback (proving tier-2 dominates
   when direct linkage exists).

### REL-002-ATOMIC-PROOF — Direct transactional-boundary proof test

**Status: Closed (preempt).**

The R5 closure shipped the executor's
`mediation.complete_with_attribution` transaction wrapping
`update_completed_tx` + `update_attempt_attribution_tx`, but R5
verification could only inspect the **source** for the atomic
boundary. No test directly exercised the boundary. R6 audit could
plausibly note this as evidence-only-by-source.

A new persistence test
`p017_mediation_complete_with_attribution_is_atomic`
(in `crates/db/tests/proposal_017_workflow_conflict_persistence.rs`)
closes that evidence gap by exercising the boundary directly:

1. Seeds a Running mediation-owned `agent_executions` row.
2. Opens a transaction; calls **both** `update_completed_tx` and
   `update_attempt_attribution_tx` with sentinel cost values.
3. **Rolls back** by dropping `tx` without committing — sqlx auto-rolls.
4. Re-fetches and asserts every column reverted: `status =
   Running`, `completed_at = None`, `total_cost_cents = None`,
   `input_tokens = None`, `transcript_artifact_id = None`. This
   proves no partial state escaped — neither write half-leaked.
5. Re-runs the same two writes inside a **committed** tx; asserts
   every column landed. This proves the happy path of the same
   atomic boundary — both writes visible together.

Two branches together prove all-or-nothing at the row level.

### GQL-LINKAGE-001 — GraphQL artifact projection exposes `linkage`

**Status: Closed (preempt).**

R5 introduced the three-tier MCP linkage label (`transcript_direct` /
`execution_id_direct` / `agent_id_correlation`) but the GraphQL
`GqlMediationAttemptArtifact` struct did not carry an equivalent
field — operators querying GraphQL had no way to tell which tier an
artifact came from. Closed by adding:

```rust
pub struct GqlMediationAttemptArtifact {
    pub id: ID,
    pub name: String,
    pub format: String,
    pub file_path: String,
    pub report_kind: Option<String>,
    pub is_pinned: bool,
    pub linkage: String,   // NEW: tier-1/2/3 label, mirrors MCP
}
```

`build_mediation_execution_attempts` populates each push with the
matching tier label at every of the three tier-emit sites.

## Verification

| Check | Result |
|---|---|
| `cargo build -p graphql-server -p mcp-server` | clean (warnings only, no errors) |
| `cargo test -p mcp-server -- proposal_017_ p017_` | 4 passed (was 3 — `p017_cross_attempt_artifact_isolation_via_mcp_readback` added) |
| `cargo test -p graphql-server -- proposal_017_ p017_` | 5 passed (was 4 — `p017_cross_attempt_artifact_isolation_via_graphql_readback` added) |
| `cargo test -p db --test proposal_017_workflow_conflict_persistence` | 21 passed (was 20 — `p017_mediation_complete_with_attribution_is_atomic` added) |
| `./scripts/test-gate.sh proposal-017` | exit 0; final closure-set log archived as `docs/proposals/017-evidence/proposal-017-r6-preempt-followup-gate-20260428T063242Z.log` (3.4 MB); structural-guard verification log at `docs/proposals/017-evidence/proposal-017-r6-preempt-strict-guard-gate-20260428T063705Z.log`; all three R6-preempt tests appear in the lib unittests output as `... ok` |

## Gate guards

`scripts/test-gate.sh proposal-017` adds 4 new R6-preempt presence
guards appended to the existing R2/R4/R5 set, plus 2 new cargo
invocations that actually execute the new tests:

Presence checks:

- GraphQL `GqlMediationAttemptArtifact` populates each of the three
  tier labels (`transcript_direct`, `execution_id_direct`,
  `agent_id_correlation`).
- GraphQL P017 attempts test contains the non-null cost assertion
  string (locks in the GQL-PARITY-001 closure).
- GraphQL cross-attempt isolation test name present in `schema.rs`.
- MCP cross-attempt isolation test name present in `reports.rs`.
- REL-002 atomicity proof test name present in `proposal_017_workflow_conflict_persistence.rs`.
- REL-002 structural awk-proximity guard: within 40 lines of the
  `mediation.complete_with_attribution` tx label in `executor.rs`,
  the file must contain `update_completed_tx`,
  `update_attempt_attribution_tx`, and `completion_tx.commit()`.
  Defends against a future refactor that moves one of the writes
  outside the tx — the original R5 guard checked the strings
  independently anywhere in the file and would have missed this.

Cargo invocations (the original P017 gate filtered on `proposal_017_`
prefix only, which would not have matched the new `p017_…` test
names):

```bash
cargo test -p mcp-server p017_     -- --test-threads=1 --nocapture
cargo test -p graphql-server p017_ -- --test-threads=1 --nocapture
```

Combined with the R2/R4/R5 guards, the gate now enforces ~44 closure
surfaces end-to-end.

## What remains deferred (recorded)

Carried over from the R5 addendum, unchanged by this preempt:

- **`mediation_retry_budget_exhausted_total` production caller** —
  helper exists, production wire-up still waiting on the budget
  enforcement contract slice (consistent with R4/R5 deferral).
- **Cross-language fixture parity** for the artifact attribution
  tiered-readback model is not in P017 scope.
- **Live provider-backed daemon dogfood replay** — out of scope for
  audit-evidence purposes; recorded as a continuous-validation
  follow-up.
