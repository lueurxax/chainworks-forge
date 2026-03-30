# Proposal 013: Output Contract Alignment, Aggregate Contract Hardening, Failure Evidence, and Narrow Recovery Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md` |
| Repository Root | `.` |
| Git SHA | `5014b29` |
| Reviewed At | `2026-03-30T21:39:29+0300` |
| Review Mode | `proposal-readiness` |
| Product Overlay | `omitted` |
| Overall Status | `Full Review` |
| Readiness | `Amber` |
| Confidence | `High` |
| Evidence Completeness | `Complete` |

## 0. Review Mode and Proposal Evidence Summary

- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- No-delta repeat round: `no`
- Proposal / docs reviewed:
  - `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md`
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
  - `docs/reference/full-mvp-delivery.md`
  - `docs/reference/mvp-sign-off.md`
  - prior review: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-review.md`
  - prior evidence pack: `docs/reviews/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening-evidence-pack.md`
  - proposal-local research pack: `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md`
  - latest implementation audit: `docs/proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening_IMPLEMENTATION_AUDIT_R6.md`
- Reusable baseline used: `.review-baselines/current-system-baseline.md`
- Baseline reused: `yes`
- Baseline refreshed: `partially, via targeted proof-lane and app-surface seam refresh`
- Baseline freshness: `Fresh for repo-level context, Partial for P013-specific verification ownership`
- Proposal-specific integration context: `none`
- Targeted context refresh performed:
  - Section `9.2` app-level proof wording
  - direct-surface ownership in `ContentView` / `Chainworks_ForgeApp`
  - current `UITestProposal013EvidenceSurface`
  - canonical gate ownership in `scripts/test-gate.sh`
  - current `Chainworks ForgeUITests` proof owners
- External research used: `reused prior proposal-local research pack; no fresh browsing in this round`
- Code areas inspected:
  - `Chainworks Forge/ContentView.swift`
  - `Chainworks Forge/Chainworks_ForgeApp.swift`
  - `Chainworks Forge/Views/UITestDirectSurfaces.swift`
  - `Chainworks Forge/Engine/OutputContractResolverV2.swift`
  - `Chainworks Forge/Engine/OutputContractDeclarativeBridge.swift`
  - `Chainworks Forge/Engine/ProposalReviewContractAdapter.swift`
  - `Chainworks ForgeUITests/Chainworks_ForgeUITests.swift`
  - `scripts/test-gate.sh`
- Current repo contradictions found:
  - Section `9.2` app-level proof remains under-specified against the repo's canonical UI proof lane, and the latest implementation audit shows that ambiguity already producing a scaffold-only proof surface outside the accepted path.
- Runtime evidence used: `none required for proposal readiness; latest repo-local implementation audit consumed as supporting evidence`
- Remaining assumptions:
  - proposal readiness can still be judged from proposal/doc/code/baseline evidence without a new runtime replay
  - the existing direct-surface + UI-test + `test-gate` lane is now the canonical proof owner for proposal-scoped macOS UI evidence in this repo
- Remaining blockers:
  - Section `9.2` does not yet anchor Proposal 013 app-level proof to that canonical lane

## 1. Executive Summary

- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `bounded and mostly implementation-ready, but one proof-ownership gap remains`
- Top strengths:
  1. Contract authority still stays singular and derived from `AgentCatalog.contracts`.
  2. Same-stage `Retry Failed Agent` remains grounded in current immutable attempt and artifact truth.
  3. Failure-evidence ordering still matches the real executor -> persistence -> validation seam.
  4. Recovery UX still extends the current shell-owned `RecoverySheet` and `BlockedRunRecoveryView`.
  5. Appendix `A` / Tier `1` / Phase `B` boundaries remain bounded and truthful against current repo seams.

The new live issue is not in the contract design itself. It is in verification ownership. Section `9.2` requires "at least one app-launched run" but never says that this proof must extend the repo's canonical proof lane. Current repo reality now has a stable owner model for UI proof: direct surfaces enumerated in `ContentView.UISurface`, bootstrapped in `Chainworks_ForgeApp`, exercised by `Chainworks ForgeUITests`, and invoked through `scripts/test-gate.sh`. The latest implementation audit already shows what the current draft permits instead: `UITestProposal013EvidenceSurface` exists, but sits outside that lane and therefore does not close acceptance. The proposal should make that ownership explicit before handoff.

## 2. Proposal Scope and Completeness

- In scope:
  - output-contract authority and review-output contract alignment
  - failed-stage evidence preservation and canonical validation-failure reference truth
  - retry lineage truth for same-run retry versus clone-run
  - blocked-run recovery explanation and narrow-action trust
  - proposal-draft output compaction metadata
  - Tier `1` declarative-runtime hardening for `contracts.*` and `structured_output`
  - canonical app-level proof for the motivating failure class
- Out of scope:
  - build/run attempts as a default review gate
  - provider-family expansion
  - repo-backed delivery changes already owned by `docs/reference/full-mvp-delivery.md`
  - general UI polish already owned by Proposal 012
  - transport outcome normalization and stage settlement, owned by `docs/reference/execution-truth-and-recovery.md`
- Deferred intentionally:
  - feature-flag rollout
  - analytics / product KPI overlay
  - broad historical artifact migration
  - Tier `3` declarative-runtime gaps such as `skill_ref`, `required_tools`, broader transport policy enforcement, and wider workflow coverage
- Most important baseline inputs reused:
  - `.review-baselines/current-system-baseline.md`
  - `docs/reference/runtime-contract.md`
  - `docs/reference/workflow-execution-engine.md`
  - `docs/reference/operator-experience.md`
- Most important repo alignment checks:
  - `Section 4.2` still matches the current catalog-backed `OutputContractResolverV2` seam
  - `Section 5.2` still keeps mandatory declarative work bounded to Tier `1`
  - `Section 6.2` still matches the current executor / persist / validate boundary
  - `Section 6.3` still extends the current shell-owned recovery surfaces
  - `Section 9.2` is now the only live readiness concern because its proof-owner boundary is not explicit enough for the current repo

## 3. External Research Summary

Prior proposal-local research was reused in this round via `013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.review/research-pack.md`. No fresh external browsing was needed because the new issue is repo-local and evidence-lane-specific, not a modern-platform uncertainty.

## 4. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | High | Complete | 0 | 0 | 1 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings

- `UI-01`
  - Severity: `Medium`
  - Confidence: `High`
  - Evidence IDs: `DOC-01`, `DOC-11`, `MAP-05`, `MAP-06`, `MAP-07`, `MAP-08`, `REAL-04`, `REAL-05`, `REAL-06`, `REAL-07`
  - Finding: Section `9.2` requires an app-level proof, but it does not anchor that proof to the repo's canonical UI-proof lane. Current repo reality already has a stable ownership model for proposal/UI proof through `ContentView.UISurface`, `Chainworks_ForgeApp` forced-surface boot, `Chainworks ForgeUITests`, and `scripts/test-gate.sh`. The latest implementation audit shows that the current wording leaves enough ambiguity for a scaffold-only `UITestProposal013EvidenceSurface` to exist outside that lane and still look superficially compliant.
  - Why it matters: a core acceptance path can drift into ad hoc harnesses instead of one accepted proof owner. That weakens incident-closure sign-off and makes future implementation audits harder to trust.
  - Fix: amend Section `9.2` (and, if needed, `10.1` / `10.2`) to say that Proposal 013 app-level proof must extend the repo's canonical UI-proof lane rather than a standalone scaffold. Name the expected owner boundary explicitly: direct-surface boot path, UI-test owner, and `test-gate` invocation, or one other single named canonical lane.
  - Acceptance criteria: Proposal 013 no longer allows multiple competing proof lanes for app-level closure; the required proof owner is explicit and singular.

### 5.2 UX Findings

No live UX findings in this reread.

### 5.3 iOS Architecture Findings

No live architecture findings in this reread.

## 6. Cross-Discipline Conflicts and Decisions

- Conflict:
  the proposal wants to require one meaningful app-level proof, but the current repo already has a hardened proof-lane contract rather than free-form UI harnesses
- Tradeoff:
  keeping Section `9.2` high-level sounds flexible, but in this repo that flexibility already produced ambiguous proof ownership
- Decision:
  keep the current contract/evidence design, but tighten proof-lane ownership before calling the draft fully handoff-ready
- Owner:
  proposal author

## 7. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Anchor Section `9.2` app-level proof to the repo's canonical UI proof lane | UI | proposal author | next edit pass | existing direct-surface boot path, `Chainworks ForgeUITests`, `scripts/test-gate.sh` | the proposal names one explicit app-proof owner path and no longer permits scaffold-only alternatives | `UI-01` |
| P3 | Optional: add `013...review/integration-context.md` if future rounds keep focusing on proof ownership and direct-surface boundaries | Review process | repo maintainer | future review hygiene | current baseline + current evidence pack | later rounds can reuse the proof-lane mapping directly | none |

## 8. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Contract alignment | review outputs, runtime validation, artifacts, and reports all agree on one contract truth | `OutputContractResolverV2` remains the sole runtime reader and contract tests stay green | do not create a second contract authority | future implementation audit | hold if runtime still reads parallel or ambiguous contract truth |
| Same-stage agent retry | prior failed evidence stays inspectable while the latest successful retry becomes effective output | agent-attempt lineage fields, retry namespace, and failure evidence stay coherent | do not overwrite stage-attempt-primary artifacts | future implementation audit | hold if same-stage retry collapses back to ambiguous or colliding storage |
| App-level proof ownership | Proposal 013 incident closure is demonstrable from one canonical app lane | direct-surface owner exists, UI-test owner exists, `test-gate` owner exists, acceptance wording names them | do not allow standalone scaffold-only proof to count as proposal closure | next proposal review and next implementation audit | hold if Section `9.2` still allows multiple proof authorities |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps

- none proposal-blocking; proposal/doc/code/baseline evidence was sufficient for a full proposal-readiness call

### Open Questions

- none beyond the explicit Section `9.2` ownership fix

## 10. Evidence Gap Review Fallback

Not used in this round. Proposal/doc/code/baseline evidence was sufficient for a full proposal-readiness review, but one live proposal-text finding remains.
