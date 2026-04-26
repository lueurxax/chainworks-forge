# Idea Brief: Implement Proposal 031 — Thin UI Rewrite Over Projections and MCP

**Run ID:** 8dd01a54-0791-43e0-b526-5ed92c95b34f  
**Date opened:** 2026-04-19  
**Source proposal:** `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md`

---

## Summary

First user-visible cutover from client-owned workflow logic to a thin macOS operator UI over GraphQL read models and MCP-backed control commands. After P031, the macOS app is a renderer of server-owned projections and an initiator of audited control commands — not a place where workflow truth is decided.

---

## Scope

### What changes

| Area | Change |
|------|--------|
| RunsHomeView | Replace `@Query SwiftData` truth with GraphQL `runs` projection query |
| RunDetailView | Replace SwiftData fetch with GraphQL `run(id:)` + `runStatusChanged(runID:)` subscription |
| StagesView | Replace local state with GraphQL `stages(runID:)` |
| ArtifactsView | Replace local state with GraphQL `artifacts(runID:)` |
| ReportsView | Replace local state with GraphQL `reports(runID:)` |
| Mutating actions | Route all writes through MCP: `runs.start`, `runs.cancel`, `approvals.resolve`, `stages.retry`, `ideas.create` |
| Swift runtime teardown | Delete client-side references to `WorkflowOrchestrator`, `RunPlanCompiler`, `TransitionEvaluator` from UI layer |

### What is preserved

- Runs Home, Run Detail, Stages panel, Artifacts inspector, Reports reader, Approvals queue — operator ergonomics intact.
- Existing SwiftData/SQLite persistence in the engine layer; only UI read paths change.

### Feature flag

Rollout behind `CHAINWORKS_THIN_UI=1` env var during dogfood. Flag removed after 2 successful end-to-end dogfood runs.

---

## Dependencies (all satisfied at HEAD)

| Dependency | Status |
|------------|--------|
| P027 Rust control-plane extraction | Landed |
| P029 MCP northbound control-plane server | Landed |
| P041 parity gate evidence | Present at `docs/proposals/031-*.evidence/p041-parity.md` (all 7 fixtures green) |
| P042 local daemon lifecycle supervision | Landed |
| P043 query projections and client consumption contract | Landed (proposal retired) |

---

## Acceptance Criteria

1. All listed Swift read sites consume GraphQL projections — no `@Query SwiftData` for run truth.
2. All listed mutating actions routed through MCP tools, not direct Swift orchestrator calls.
3. Teardown map deletions complete; Swift build has no remaining references to deleted types.
4. `proposal-031|p031` gate registered and passing in `scripts/test-gate.sh`.
5. Two green dogfood runs on `full-mvp-live` workflow end-to-end using the thin UI build.

---

## Key References

- Proposal: `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md`
- Readiness review: `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/proposal-readiness-review.md`
- Evidence pack: `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/evidence-pack.md`
- Parity evidence: `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.evidence/p041-parity.md`
- Proof gate: `scripts/test-gate.sh proposal-031`
