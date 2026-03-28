# Proposal 013: Output Contract Alignment, Retry Truth, and Failure Evidence Hardening Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `.` |
| Git SHA | `3e36dfb` |
| Reviewed At | `2026-03-28T23:42:30+0200` |
| Review Mode | `proposal-readiness` |
| Product Overlay | `omitted` |
| Overall Status | `Full Review` |
| Readiness | `Green` |
| Confidence | `High` |
| Evidence Completeness | `Complete` |

## 0. Review Mode and Proposal Evidence Summary

- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- No-delta repeat round: `no`
- Proposal / docs reviewed:
  - `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/domain-model.md`
  - prior review: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-review.md`
  - prior evidence pack: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md`
  - proposal-local research pack: `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md`
- Reusable baseline used: `none`
- Baseline reused: `no`
- Baseline refreshed: `no`
- Baseline freshness: `Missing`
- Proposal-specific integration context: `none`
- Targeted context refresh performed: `artifact identity, retry lineage, current recovery ownership, and contract/persistence seams only`
- External research used: `Reused prior proposal-local research pack; no fresh browsing in this round`
- Code areas inspected:
  - `Chainworks Forge/Engine/AgentExecutor.swift`
  - `Chainworks Forge/Engine/ArtifactManager.swift`
  - `Chainworks Forge/Engine/ArtifactStorage.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
  - `examples/agents/agents.yaml`
- Current repo contradictions found:
  - none that remain proposal-blocking after the latest text revision
- Runtime evidence used: `Optional repo-local motivating-run storage only`
- Remaining assumptions:
  - proposal readiness can be judged without new builds or app launches
  - the current artifact identity/path contract in code and docs remains authoritative for validating the new proposal semantics
- Remaining blockers:
  - none

## 1. Executive Summary

- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Implementation-ready from local proposal/doc/code evidence`
- Top strengths:
  1. Contract authority is now explicit and derived from the existing catalog-backed runtime seam.
  2. Failure-evidence ordering is now anchored to the current `AgentExecutor -> ArtifactManager -> WorkflowOrchestrator` boundary.
  3. Recovery UX is now clearly scoped as an extension of existing shell-owned surfaces.
  4. Same-stage `Retry Failed Agent` now has an explicit artifact identity and storage contract via Section `5.4`.
  5. The proposal now explicitly carries the research-backed clarifications around frozen snapshot reuse, canonical failure-evidence reference truth, and non-auto-retryable contract mismatch default posture.

This was a clean delta round, not a pure repeat pass. The new proposal hash adopted all three targeted clarifications from the earlier research pack: same-run `Retry Failed Agent` now explicitly reuses the same frozen logical snapshot and requires persisted snapshot linkage, `ValidationFailureRecord` or the failed-stage packet is now the canonical reference target for recovery/report/export surfaces, and output-contract mismatch plus post-generation validation failure are now non-auto-retryable by default unless an explicit recovery action or policy override says otherwise. Those additions strengthen the already-green storage, recovery, and persistence seams without reopening any contradiction against current repo reality.

## 2. Proposal Scope and Completeness

- In scope:
  - proposal-review output contract alignment
  - failed-stage evidence preservation
  - retry and clone lineage truth
  - blocked-run recovery evidence/trust extensions
  - proposal-output compaction metadata
- Out of scope:
  - workflow-topology changes
  - approval-model redesign
  - provider-family expansion
  - repo-backed delivery changes already owned by Proposal 007
  - broad historical artifact migration
- Deferred intentionally:
  - feature-flag rollout
  - analytics/product KPI overlay
  - broader cancellation semantics
- Most important baseline refreshes performed:
  - none; `.review-baselines/current-system-baseline.md` is absent
- Most important repo alignment checks:
  - Section `4.2` now aligns with current `AgentCatalog.contracts` + `OutputContractResolver`
  - Section `5.4` now aligns with current stage-attempt immutability and path layout by adding explicit agent-retry storage truth
  - Section `6.2` now aligns with the current persistence/validation seam
  - Section `7` now aligns with `RecoverySheet` and `BlockedRunRecoveryView` as current shell owners

## 3. External Research Summary

Prior proposal-local research was reused in this round via `013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md`. No fresh external browsing was needed because the only question here was whether the proposal text had actually absorbed the previously recommended deltas.

## 4. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | Medium | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings

No live UI findings in this reread.

### 5.2 UX Findings

No live UX findings in this reread.

### 5.3 iOS Architecture Findings

No live architecture findings in this reread.

## 6. Cross-Discipline Conflicts and Decisions

- Conflict:
  none remaining that block implementation readiness
  Tradeoff:
  n/a
  Decision:
  current draft is coherent enough to hand off
  Owner:
  proposal author

## 7. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P2 | Optional: create reusable baseline / proposal-local integration context for future rerounds | Review process | repo maintainer | future review hygiene | `.review-baselines/current-system-baseline.md`, `013...review/` | later rounds can reuse host-system mapping without redoing narrow context refresh | none |

## 8. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Contract alignment | runtime and artifact truth use one contract source | catalog contract fields, typed resolver, coherent report/recovery wording | do not reintroduce a second contract authority | implementation audit / future review | hold if runtime starts reading parallel contract truth |
| Agent retry truth | same-stage retry preserves prior evidence while surfacing latest successful retry | disjoint retry namespace, artifact lineage metadata, clear report rendering | do not overwrite prior artifacts or receipts | implementation audit / future review | hold if same-stage retry collapses back to ambiguous storage |
| Failure evidence retention | validation failure still preserves raw output, receipts, transcripts, and failure records | provisional persistence before validation, shared failed-stage packet | do not let validation failure erase evidence | implementation audit / future review | hold if evidence disappears at validation boundary |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: `.review-baselines/current-system-baseline.md` is still absent, so this round continued to rely on targeted code/doc mapping instead of reusable host-baseline intake.
- `GAP-02`: Proposal 013 still has no proposal-local integration-context artifact; future rerounds would benefit from one if the draft keeps changing rapidly.

### Open Questions

- none proposal-blocking in the current reread

## 10. Evidence Gap Review Fallback

Not used in this round. Proposal/doc/code evidence was sufficient for a full proposal-readiness review, and no live proposal-blocking findings remain.
