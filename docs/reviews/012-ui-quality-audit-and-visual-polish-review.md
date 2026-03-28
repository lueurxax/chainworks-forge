# Proposal 012: UI Quality Audit and Visual Polish Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/012-ui-quality-audit-and-visual-polish.md` |
| Repository Root | `.` |
| Git SHA | `fa31abc` |
| Reviewed At | `2026-03-28T15:48:47+0200` |
| Review Mode | `proposal-readiness` |
| Product Overlay | `omitted` |
| Overall Status | `Full Review` |
| Readiness | `Green` |
| Confidence | `High` |
| Evidence Completeness | `Complete` |

## 0. Review Mode and Proposal Evidence Summary

- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md`
  - pre-refresh review artifact at `docs/reviews/012-ui-quality-audit-and-visual-polish-review.md`
  - pre-refresh evidence pack at `docs/reviews/012-ui-quality-audit-and-visual-polish-evidence-pack.md`
  - `docs/reference/idea-lifecycle.md`
  - `docs/reference/live-workflow-map.md`
- Reusable baseline used: `none`
- Baseline reused: `no`
- Baseline refreshed: `no`
- Baseline freshness: `Missing`
- Proposal-specific integration context: `none`
- Targeted context refresh performed: `current code-path mapping only`
- External research used: `None`
- Research pack: `n/a`
- Sources reused: `n/a`
- Sources refreshed: `n/a`
- Time-sensitive external guidance: `n/a`
- Code areas inspected:
  - `ContentView.swift`
  - `Views/RunsHomeView.swift`
  - `Views/IdeaListView.swift`
  - `Views/ProviderSettingsView.swift`
  - `Views/PilotReadinessView.swift`
  - `Views/FirstRunSetupWizard.swift`
  - `Views/GooseProviderConnectionAssistantView.swift`
  - `Views/WorkflowMapView.swift`
  - `Views/ReleaseGateView.swift`
  - `Views/ForegroundBannerView.swift`
  - `Views/DeliveryPreflightReportView.swift`
  - `Views/ApprovalGateView.swift`
  - `Views/RecoverySheet.swift`
  - `Views/ArchivedIdeasView.swift`
  - `Views/RunStartOverridesView.swift`
- Current repo contradictions found:
  - none material in this revision
  - the three previous blockers are now closed in proposal text: audited-surface ownership, keyboard/interaction verification, and `StatusCapsule` contract alignment
- Runtime evidence used: `None`
- Provenance of key evidence:
  - proposal text
  - stable reference docs
  - current SwiftUI source and preview declarations
- Remaining assumptions:
  - proposal readiness can be judged from proposal/docs/code evidence without build/run proof
  - missing repo-level baseline does not block this round because current code mapping was sufficient
- Remaining blockers:
  - none

## 1. Executive Summary

- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- Top strengths:
  1. Appendix A now lists the subordinate surfaces and proof owners that were previously outside the authoritative audit boundary.
  2. Verification criteria now explicitly cover keyboard/interaction ownership on the operator flows that needed it.
  3. The shared `StatusCapsule` contract is now internally consistent between the main spec and Appendix B.
- Residual note:
  1. The repo still has no reusable `.review-baselines/current-system-baseline.md`, so this round relied on direct code mapping instead of baseline reuse.

Proposal 012 is now ready for implementation handoff as a proposal artifact. The prior P0/P1 issues are closed in the current draft: Appendix A matches the real touched-surface set, Section 6 now proves the keyboard and below-the-fold interaction work explicitly, and the shared badge primitive is no longer internally contradictory. This round stays proposal-first: no build/run/simulator proof was required or used.

## 2. Proposal Scope and Completeness

- In scope:
  - visual polish across the operator-facing macOS shell and the audited surfaces listed in Appendix A
  - readability, hierarchy, badge/token consistency, empty/loading feedback, and a bounded design-system slice
  - non-happy-path presentation semantics for the touched surfaces
- Out of scope:
  - business-logic changes
  - light mode support
  - localization readiness
  - iOS/iPadOS adaptation
  - motion work beyond the banner fix
  - runtime/Xcode validation in this review round
- Deferred intentionally:
  - product KPI overlay
  - external research
  - engine-level rollback semantics and session-recovery ownership
- Most important baseline refreshes performed:
  - none; `.review-baselines/current-system-baseline.md` is absent in the current repo
- Most important contradictions with current repo:
  - none material after the current proposal revision
- Most important missing or partial states:
  - none material at proposal-readiness level; Section 3, Section 6, and Appendix A now cover the active surfaces and proof obligations well enough for handoff

## 3. External Research Summary

Not used in this round.

## 4. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | Medium | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings

No material UI blocker remains in the current draft. The previously open audited-surface boundary gap is closed: Appendix A now enumerates subordinate surfaces and assigns proof ownership to them, and the `StatusCapsule` contract is consistent between the main `M-01` example and Appendix B.

### 5.2 UX Findings

No material UX blocker remains in the current draft. The verification section now explicitly names the keyboard/interaction-heavy surfaces and the expected proof obligations for confirm/dismiss bindings and below-the-fold action discoverability.

### 5.3 iOS Architecture Findings

No material iOS-architecture blocker was found in this round. Dependency framing, bounded shared-primitive rollout, and state-ownership boundaries remain coherent enough for proposal-readiness.

## 6. Cross-Discipline Conflicts and Decisions

No open cross-discipline conflict remains after this revision. The current proposal now aligns:

- Appendix A with the actual touched-surface set
- Section 6 with the interaction-heavy Phase 2 commitments
- `M-01` with Appendix B for the shared badge primitive

## 7. Prioritized Action Backlog

No blocking proposal changes remain before implementation handoff.

Optional follow-on outside Proposal 012:

- create `.review-baselines/current-system-baseline.md` via `$integration-context-baseline` so future proposal rounds can reuse host-system mapping instead of rebuilding it from source

## 8. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Proposal-to-implementation traceability | whether Appendix A proof owners and Section 6 checks survive implementation unchanged | every touched surface still maps cleanly to a proof owner during implementation review | do not silently drop subordinate surfaces or interaction checks | first implementation evidence review after code lands | hold if implementation narrows proof coverage relative to the proposal |
| State/feedback contract conformance | whether touched surfaces implement the Section 3 semantics or explicitly defer them | validation, degraded/offline, retry, and cancellation semantics stay tied to the named surfaces | do not invent engine/session semantics that Proposal 012 explicitly leaves out of scope | first implementation evidence review after code lands | hold if surface-level semantics drift from Section 3 without explicit deferral |
| Bounded design-system rollout | whether Phase 3 stays inside the adopter slice and preserves behaviour | shared primitives appear only on the named adopter surfaces first | no business-logic or navigation drift during token/capsule extraction | Phase 3 implementation review | hold if primitive rollout expands beyond the adopter slice before verification passes |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: `.review-baselines/current-system-baseline.md` is absent, so this round relied on direct code mapping instead of reusable host-system baseline intake.
- `GAP-02`: Proposal-readiness mode intentionally did not run previews or simulator proofs; this round only verified that the declared preview/test surfaces exist in source and that the proposal’s scope matches them.

### Open Questions

No blocking open question remains for the proposal artifact itself.

## 10. Evidence Gap Review Fallback

Not used in this round. Proposal/doc/code evidence was sufficient for a full proposal-readiness review.
