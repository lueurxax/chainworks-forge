# P053 Phase 1 Retrospective

Date: `2026-04-23`
Tree: `main`

## What Changed

- Fresh ACP startup no longer performs broad repository/workspace/worktree discovery before `initialize`.
- Declared output handling is driven by typed expected outputs, bounded pre-prompt metadata, acceptance decisions, and durable discovery diagnostics.
- Legacy broad discovery remains opt-in, bounded, and auditable.
- macOS operator UI work is explicitly deferred to P069 and no longer blocks P053 closeout.

## What Was Learned

- The closeout risk was no longer in the core control-plane behavior; it was in evidence discipline and observability completeness.
- P053 needed the full structured metrics/readback surface implemented in source, not just mentioned in the proposal.
- Same-tree closeout needs cap-validation, gate checks, and review artifacts to agree on one contract; partial agreement is not enough.

## Decision

Decision: `proceed with P053 merge closeout after same-tree proposal-053 gate passes`

Status update: the same-tree command `./scripts/test-gate.sh proposal-053` passed on `2026-04-23`; see `docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/proposal-053-gate-2026-04-23.md`.

Exposure decision: `production_exposed` for P053 Rust control-plane/API/readback behavior, using the approved replacement sample in `cap-validation.json`.

Fallback approval: direct production execution IDs were unavailable in the local closeout environment. The approved substitute is the same-tree P053 gate, manual reference-workspace latency spot-check, stale-vs-absent readback fixtures, GraphQL/MCP readback fixtures, and trait-injected `DiscoveryFilesystem` fake coverage. Product/UI exposure remains deferred to P069.

## Follow-On Boundaries

- Production exposure does not require new pre-signoff sampling for P053 control-plane/API/readback; production telemetry should be reviewed after rollout for cap retuning.
- P069 remains the follow-up for macOS operator UI.
- P053 does not reopen broad discovery by default.

## Branch Scope Note

Current `main` also contains non-P053 catalog/config/P017 audit changes committed after the P053 closeout merge. Those changes are not part of the P053 production-exposure claim and should be validated under their own proposal/release evidence. P053 readiness is scoped to the P053 Rust control-plane/API/readback files, evidence sidecars, and canonical `proposal-053` gate.
