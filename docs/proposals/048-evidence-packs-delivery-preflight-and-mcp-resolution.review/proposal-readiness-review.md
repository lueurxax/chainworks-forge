# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.md`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.review/evidence-pack.md`
  - `docs/proposals/048-evidence-packs-delivery-preflight-and-mcp-resolution.review/proposal-readiness-review.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/test-gates.md`
- Current code inspected:
  - `control-plane/crates/graphql-server/src/schema.rs`
  - `control-plane/crates/graphql-server/src/types/run.rs`
  - `control-plane/crates/graphql-server/src/types/stage.rs`
  - `control-plane/crates/mcp-server/src/server.rs`
  - `control-plane/crates/mcp-server/src/tools/runs.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `control-plane/crates/workflow/src/compiler.rs`
  - `control-plane/crates/workflow/src/plan.rs`
  - `control-plane/crates/db/migrations`
  - `scripts/test-gate.sh`
- Reusable baseline used:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/rust-control-plane.md`
  - `docs/reference/test-gates.md`
- Baseline freshness: `Partially refreshed`
- External research used: `None`
- Runtime evidence used: `None`

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Closed since the stale prior local review:
  - P048 now correctly binds persisted run-owned delivery-preflight truth to `run://{run_id}`.
  - P048 now defines the GraphQL blocked-start contract as an explicit `StartRunResult` union instead of leaving it implicit.
  - The earlier blockers about missing stage-owned `validation_failure_json` parity and missing blocked-start delivery-preflight transport truth remain closed.
- Remaining live gaps:
  1. The canonical `proposal-048|p048` proof lane is still stale. Its command list reuses an old `delivery_configuration_json` persistence test and does not actually prove several acceptance criteria the proposal now claims as canonical proof.
  2. The migration guidance is also stale. The file plan hard-codes `008_*` as the current concrete migration slot even though `008_session_runtime_usage.sql` and `009_owner_execution_lineage.sql` already exist at `HEAD`.

## 2. Findings

### 2.1 Architecture Findings
- Finding ID: `ARCH-048-01`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-05`, `MAP-04`, `DATA-04`, `TEST-01`, `REAL-03`
  Why it matters:
  P048 says `proposal-048|p048` is the canonical proof path for this slice, and its focused scope explicitly includes delivery-preflight blocking and persistence, failed-stage evidence persistence plus report-lane readback, and GraphQL/MCP parity for northbound execution truth. But the proposed command list still starts with `test_start_run_persists_delivery_configuration_json`, which proves the older P045 field rather than P048's new `delivery_preflight_json` persistence contract. The same snippet also omits any explicit proof for failed-stage evidence report-lane readback, GraphQL execution-level MCP truth, and `run://{run_id}` parity. If implemented as written, later audits could pass the named gate without ever proving several P048 acceptance criteria.
  Recommended fix:
  Rewrite the `proposal-048|p048` gate so each command directly maps to the slice it claims to prove. At minimum, replace the old delivery-configuration persistence test with one that proves successful `delivery_preflight_json` persistence and add explicit coverage for failed-stage evidence readback, GraphQL execution-level MCP truth, and `run://{run_id}` readback parity.
  Acceptance criteria:
  - The gate no longer references `test_start_run_persists_delivery_configuration_json`.
  - The gate proves successful `delivery_preflight_json` persistence plus GraphQL / `runs.get` / `run://{run_id}` readback parity.
  - The gate proves failed-stage evidence persistence and report-lane readback.
  - The gate proves GraphQL execution-level MCP truth, not only MCP report-side truth.
  Confidence: `High`

- Finding ID: `ARCH-048-02`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `MAP-03`, `REAL-04`
  Why it matters:
  The file plan now says the migration should use the next free ordinal at implementation time, but then immediately anchors that guidance to an outdated concrete slot: `008_*`. Current `HEAD` already contains `008_session_runtime_usage.sql` and `009_owner_execution_lineage.sql`, so the next free slot is `010_*`. This is not a design contradiction, but it is a live repo-reality mismatch inside the handoff instructions.
  Recommended fix:
  Either remove the concrete ordinal note entirely and keep only the “next free migration ordinal at implementation time” rule, or update the example to match current `HEAD` (`010_*`).
  Acceptance criteria:
  - The migration guidance does not mention `008_*` as the current concrete slot.
  - The file plan either stays ordinal-agnostic or uses the current next free ordinal at `HEAD`.
  Confidence: `High`

### 2.2 UI / UX Findings
- None blocking.
  This round's remaining issues are implementation-handoff and proof-contract gaps, not UI or user-flow specification problems.

## 3. Cross-Discipline Decisions
- Closed from the stale prior review:
  - wrong MCP single-run resource family,
  - missing exact GraphQL blocked-start schema shape,
  - earlier stage-owned validation and blocked-start transport omissions.
- Still needs explicit tightening:
  - the proof lane must prove the new P048 acceptance surface rather than inherited P045 coverage,
  - the migration note must stop hard-coding an outdated slot.

## 4. Prioritized Action Backlog
| Priority | Item | Owner | Horizon | Success Metric | Source Findings |
|---|---|---|---|---|---|
| P0 | Rewrite `proposal-048|p048` so the proof commands actually cover P048 acceptance criteria | Proposal author | Before implementation | The canonical gate proves delivery-preflight persistence/readback, failed-stage evidence readback, and GraphQL/MCP parity on the promised surfaces | `ARCH-048-01` |
| P1 | Remove or refresh the stale migration-slot note | Proposal author | Before implementation | The migration guidance matches current `HEAD` or stays generic | `ARCH-048-02` |

## 5. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint |
|---|---|---|---|---|
| Proposal proof lane | Whether the named gate proves the proposal's own acceptance criteria | one test per claimed surface or a clearly traceable grouped test | no inherited P045-only assertions inside the P048 lane | proposal update review + implementation audit |
| Migration handoff accuracy | Whether implementation instructions match current repo reality | no stale hard-coded ordinal notes | no guidance that points to already-consumed migration slots | proposal update review |

## 6. Evidence Gaps and Open Questions

### Evidence Gaps
- None blocking in this round.
  Local proposal/docs/code/baseline evidence were sufficient for a defensible proposal-readiness call.

### Open Questions
- Should the proposal keep a concrete migration-slot example at all, or just state the invariant “use the next free migration ordinal at implementation time”?
