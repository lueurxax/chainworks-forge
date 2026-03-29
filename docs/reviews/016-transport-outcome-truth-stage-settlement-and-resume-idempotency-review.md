# Proposal 016: Transport Outcome Truth, Stage Settlement, and Resume Idempotency Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md` |
| Repository Root | `.` |
| Git SHA | `5c870b4` |
| Reviewed At | `2026-03-29T18:02:18+0300` |
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
  - `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/provider-binding-truth.md`
  - `docs/reference/run-control.md`
  - prior review: `docs/reviews/016-transport-outcome-truth-stage-settlement-and-resume-idempotency-review.md`
  - prior evidence pack: `docs/reviews/016-transport-outcome-truth-stage-settlement-and-resume-idempotency-evidence-pack.md`
  - proposal-local research pack: `docs/proposals/016-transport-outcome-truth-stage-settlement-and-resume-idempotency.review/research-pack.md`
- Reusable baseline used: `.review-baselines/current-system-baseline.md`
- Baseline reused: `yes`
- Baseline refreshed: `partially, via targeted transport / settlement / repair seam refresh`
- Baseline freshness: `Fresh for repo-level topology, Partial for P016-specific runtime seams`
- Proposal-specific integration context: `none`
- Targeted context refresh performed:
  - limit-exhaustion and neutral-stop delta over the existing transport-outcome slice
  - cancellation taxonomy and `RunCancellationCoordinator` alignment
  - outcome storage ownership and read order
  - aggregate settlement ownership versus stage-owned truth
  - guard placement and startup-repair ordering
- External research used: `reused existing proposal-local research pack; no fresh web refresh needed`
- Code areas inspected:
  - `Chainworks Forge/Models/Run.swift`
  - `Chainworks Forge/Models/StageExecution.swift`
  - `Chainworks Forge/Models/AgentExecution.swift`
  - `Chainworks Forge/Models/Approval.swift`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift`
  - `Chainworks Forge/Engine/ResumeManager.swift`
  - `Chainworks Forge/Engine/RecoveryCoordinator.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift`
  - `Chainworks Forge/Engine/StageRetryCoordinator.swift`
  - `Chainworks Forge/Engine/ExecutionReceiptBuilder.swift`
  - `Chainworks Forge/Providers/ProviderExecutionReceipt.swift`
  - `Chainworks Forge/Providers/UsageReceiptNormalizer.swift`
  - `Chainworks Forge/Views/RecoverySheet.swift`
  - `Chainworks Forge/Views/BlockedRunRecoveryView.swift`
- Current repo contradictions found:
  - none proposal-blocking in the updated draft
- Runtime evidence used: `none required`
- Remaining assumptions:
  - readiness can be judged from proposal/doc/code/baseline evidence without a new runtime replay
  - `Approval.lineageID` in the storage table and `Approval.lineageKey` in the preferred-contract wording describe the same semantic slot, with naming left intentionally flexible
  - `ExecutionReceiptV2` is the proposal’s named normalized receipt/evidence layer over the existing receipt artifacts and provider receipt, not a second competing truth source
- Remaining blockers:
  - none

## 1. Executive Summary

- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Implementation-ready from local proposal/doc/code/baseline evidence`
- Top strengths:
  1. truthful cancellation remains explicitly part of the canonical outcome taxonomy and verification plan
  2. the outcome storage model still states one authority: flattened persisted columns are canonical and `outcomeEnvelopeJSON` is diagnostic only
  3. aggregate settlement still remains subordinate to the aggregate state’s canonical `StageExecution`
  4. create-path guard placement remains explicit, so startup repair stays secondary rather than the normal prevention path
  5. the fresh edits now also incorporate the research-backed non-auto-retryable policy-stop and neutral-finish clarifications without reopening the earlier blockers

The draft stays `Green` after the fresh edits. The proposal now cleanly carries forward the earlier closed seams and also folds in the recent research-backed deltas: neutral finish markers are explicitly non-success on their own, raw diagnostic evidence is prevented from outranking canonical outcome columns, and provider policy/limit-bound terminal stops default to non-auto-retryable unless a narrower override is persisted. None of those edits reopened the earlier aggregate-authority, outcome-owner, or cancellation-taxonomy blockers.

## 2. Proposal Scope and Completeness

- In scope:
  - canonical agent transport-outcome truth
  - atomic stage settlement and aggregate truth
  - startup repair and resume idempotency
  - frozen-versus-runtime provider binding truth migration
  - report/recovery alignment to canonical settlement and recovery records
  - provider/app limit-exhaustion truth and neutral-stop handling
- Out of scope:
  - broad workflow redesign
  - provider-family expansion
  - design-system/UI polish
  - release-readiness or feature-readiness sign-off
- Deferred intentionally:
  - product/metrics overlay
  - runtime replay proof
  - proposal-local integration-context artifact
- Most important baseline refreshes performed:
  - verified current stop-path settlement and agent-level `.cancelled` truth
  - verified current receipt and provider-receipt seams that ground the new limit-exhaustion slice
  - verified current stage-owned report/recovery evidence path
  - verified current startup classification and recovery ownership seams
- Most important repo alignment checks:
  - `4.2` still aligns with stable run-control cancellation truth and now also covers limit exhaustion explicitly
  - `4.3` still defines one explicit owner model for outcome storage
  - `5.4` still keeps `AggregateSettlementRecord` subordinate to aggregate-stage `StageExecution`
  - `7.3` still makes create-path guard placement primary and startup repair secondary
  - `6.2` remains aligned with current frozen/provider truth seams
  - `7.4` and `7.5` now make non-auto-retryable limit/policy stop behavior explicit in the canonical recovery path

## 3. External Research Summary

Existing proposal-local research was reused in this round; no new web research was needed. The reused pack matters because the fresh proposal delta directly incorporated the earlier bounded recommendations around neutral-stop semantics, canonical-owner precedence, and non-auto-retryable policy-bound stops.

## 4. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | Medium | Complete | 0 | 0 | 0 | 0 |
| UX | Green | Medium | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings

No live UI findings in this reread.

### 5.2 UX Findings

No live UX findings in this reread.

### 5.3 iOS Architecture Findings

No live architecture findings in this reread.

The earlier blockers remain closed:

- cancellation taxonomy still includes `cancelled_before_output` / `cancelled_after_output` and explicit Proposal 011 bridge tests
- outcome persistence still states that flattened outcome columns are canonical and `outcomeEnvelopeJSON` is diagnostic-only
- aggregate settlement still states that aggregate-stage `StageExecution` remains canonical for stage terminality and `AggregateSettlementRecord` is subordinate detail

The fresh limit-exhaustion delta is also grounded cleanly:

- neutral stop markers like `Finish: stop` are now explicitly non-success on their own
- provider/app exhaustion now gets dedicated terminal outcomes and verification rows
- the new `providerStopReason` column stays inside the same single-owner outcome model
- provider policy/safety/blocklist terminal stops now default to non-auto-retryable unless the canonical recovery snapshot records a narrower override

## 6. Cross-Discipline Conflicts and Decisions

- Conflict:
  the proposal wants more explicit runtime truth without reopening already-stable P011/P013 seams
- Tradeoff:
  adding limit-exhaustion and stop-reason truth only helps if it stays inside the same outcome-owner model rather than creating new parallel receipt truth
- Decision:
  the updated draft still keeps those ownership boundaries coherent enough to hand off
- Owner:
  proposal author

## 7. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P2 | Optional: normalize the approval-lineage example name (`lineageID` vs `lineageKey`) if the repo wants one term throughout the draft | iOS Architecture | proposal author | next editorial pass | `4.3`, `7.2` | editorial consistency only; no semantic change | none |
| P2 | Optional: name explicitly whether `ExecutionReceiptV2` is the structured payload inside `outcomeEnvelopeJSON` or a separate implementation helper type | iOS Architecture | proposal author | next editorial pass | Layer `R`, `4.3` | editorial clarity only; no current blocker | none |
| P3 | Optional: add `016...review/integration-context.md` if the proposal keeps evolving across rerounds | Review process | repo maintainer | future review hygiene | current evidence pack | later rounds can reuse the narrow runtime-seam mapping directly | none |

## 8. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Agent terminal outcomes | every agent attempt settles to one explicit canonical outcome including cancellation and limit exhaustion | outcome-classification, cancellation-bridge, and limit-exhaustion tests | do not bypass canonical outcome columns for stop-path or exhaustion truth | later implementation audit | hold if cancellation or exhaustion still survive only as coarse status or heuristic receipt reads |
| Outcome schema ownership | reader precedence and migration labeling stay singular | canonical flattened columns plus diagnostic envelope-only support | do not recreate dual authority between columns and envelope JSON | later implementation audit | hold if a reader can still derive conflicting truth from two owners |
| Aggregate settlement truth | report/recovery surfaces traverse aggregate truth through the canonical aggregate `StageExecution` | aggregate tests assert stage-owned terminality plus subordinate aggregate detail | do not split aggregate terminality into a parallel owner | later implementation audit | hold if aggregate failure can still resolve from competing authorities |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: no proposal-local `integration-context.md` exists yet; non-blocking in this round because the targeted runtime-seam refresh was sufficient.

### Open Questions

- none proposal-blocking in the updated draft

## 10. Evidence Gap Review Fallback

Not used in this round. Proposal/doc/code/baseline evidence remained sufficient for a full proposal-readiness review, existing proposal-local research was successfully reused, and no live proposal-blocking findings remain.
