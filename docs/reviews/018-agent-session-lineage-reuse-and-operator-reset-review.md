# Proposal 018: Agent Session Lineage Reuse and Operator Reset Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/018-agent-session-lineage-reuse-and-operator-reset.md` |
| Repository Root | `.` |
| Git SHA | `9b688cb` |
| Reviewed At | `2026-03-30T23:14:39+0300` |
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
  - `docs/proposals/018-agent-session-lineage-reuse-and-operator-reset.md`
  - prior review: `docs/reviews/018-agent-session-lineage-reuse-and-operator-reset-review.md`
  - prior evidence pack: `docs/reviews/018-agent-session-lineage-reuse-and-operator-reset-evidence-pack.md`
  - proposal-local research pack: `docs/proposals/018-agent-session-lineage-reuse-and-operator-reset.review/research-pack.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/live-provider-execution-slice.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/execution-truth-and-recovery.md`
  - `docs/reference/operator-experience.md`
  - `docs/proposals/015-skill-resolution-and-runtime-injection.md`
  - `examples/workflows/full-mvp-live.yaml`
- Reusable baseline used: `.review-baselines/current-system-baseline.md`
- Baseline reused: `yes`
- Baseline refreshed: `no further repo-local refresh needed`
- Baseline freshness: `Fresh enough for P018 seam mapping`
- Proposal-specific integration context: `none`
- External research used: `yes, reused fresh R2 proposal-local research pack`
- Runtime evidence used: `none required for proposal readiness`
- Current repo contradictions found:
  - none proposal-blocking on the updated draft

## 1. Executive Summary

- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `implementation-ready from current proposal/doc/code/baseline/research evidence`
- Top strengths:
  1. reuse remains downstream of execution truth and does not mint a second lineage authority
  2. family reuse now fail-closes when the reusable static prefix or invocation contract drifts
  3. budget policy is now explicitly metric-driven rather than cap-only
  4. checkpoint artifacts now read as deterministic continuation artifacts rather than vague summaries

This is a clean delta-to-green round. The previous `Amber` blockers were real, but the current proposal now absorbs them directly in the text:

- Section `6.1` expands the binding fingerprint to include static task/instruction scaffold, system prompt framing, and tool contract
- Section `6.2` makes `sessionFamilyID` insufficient on its own and forces family reuse to fail closed on static-prefix drift
- Section `6.3` makes caps guardrails rather than primary authority and explicitly drives `ContextBudgetGuard` from measured reuse economics
- Section `6.4` upgrades the checkpoint artifact with explicit next steps, durable learnings, blockers, and owner/binding context for deterministic fresh rehydration

No live proposal-text findings remain in the current reread.

## 2. Proposal Scope and Completeness

- In scope:
  - same-run, same-agent provider-session reuse
  - invocation-owner narrowing for reuse
  - append-only lineage generation and event history
  - operator-triggered session reset
  - budget-driven invalidation and compaction
  - checkpoint artifacts for fresh rehydration
- Out of scope:
  - cross-run memory
  - cross-agent session sharing
  - replacing artifacts/receipts/outcomes as durable truth
  - provider routing redesign
- Deferred intentionally:
  - explicit proof-lane contract
  - rollout / migration sequencing
  - implementation-plan decomposition

## 3. External Research Summary

The fresh `R2` research pack was reused after a freshness check. Its four important conclusions still hold and are now reflected by the draft:

1. stable root plus subordinate generations remains the right ownership model
2. family reuse must fail closed when the static reusable prefix drifts
3. budget control should be driven by measured cache/value signals, not transcript size alone
4. checkpoint-plus-fresh is valid only when the checkpoint preserves explicit continuation state

The current text now matches those conclusions closely enough that no live proposal-text blocker remains.

## 4. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
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
  the proposal needed to keep provider-agnostic modeling without leaving safety and cost decisions too abstract
- Tradeoff:
  overly generic language would have left family reuse, budget economics, and checkpoint continuity under-specified
- Decision:
  the draft now resolves that correctly by keeping the model provider-agnostic while making the acceptance contract explicit enough for implementation

## 7. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P3 | Optional: add a proposal-local proof-owner section before implementation audit if later rounds need a more explicit proving lane | Review process | proposal author | later hygiene | implementation planning | later audit rounds can reuse the proof-owner text directly | none |

## 8. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Family reuse safety | family reuse never outranks the current invocation contract | fresh generation is forced when static-prefix compatibility is no longer trustworthy | do not allow `sessionFamilyID` alone to authorize reuse | future implementation audit | hold if runtime still allows reuse across incompatible task/tool/system contracts |
| Budget economics | reuse stays cheaper or more valuable than checkpoint-plus-fresh | provider telemetry or normalized cost drives keep/reuse/compact decisions | do not regress to cap-only policy | future implementation audit | hold if implementation ignores measured reuse economics |
| Fresh rehydration | a fresh generation can continue from durable truth after compaction/reset | checkpoint contents remain continuation-safe | do not rely on opaque provider memory or vague summaries | future implementation audit | hold if checkpoint persistence drops owner/binding/next-step truth |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: there is still no proposal-local `integration-context.md`; not blocking for this round.

### Open Questions

- none proposal-blocking in the current reread

## 10. Evidence Gap Review Fallback

Not used in this round. Proposal/doc/code/baseline/research evidence was sufficient for a full reread, and no live proposal-text findings remain.
