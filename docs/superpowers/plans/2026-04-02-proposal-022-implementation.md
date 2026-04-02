## Proposal 022 Implementation Plan

### Goal
Implement full-fidelity proposal-loop handoff, explicit score-lift backlog and writer coverage artifacts, targeted re-review inputs, and shell-owned reporting/comparison visibility for Proposal 022.

### Scope
1. Retire stale `proposal_review_all` seams and make raw quartet + `proposal_review_summary` the canonical refine handoff.
2. Add canonical proposal-loop artifacts/contracts:
   - `score_lift_backlog`
   - `proposal_feedback_coverage`
   - `proposal_fact_digest`
   - `reviewer_scope_plan`
3. Wire live workflow/catalog so:
   - review aggregation emits backlog/scope/digest,
   - refine consumes raw quartet + summary + backlog,
   - reviewers can consume prior coverage/scope for targeted reruns.
4. Extend runtime validation/templates and handoff packet instructions for the new structured artifacts.
5. Extend report/comparison surfaces using existing shell-owned owners, without a parallel proposal-loop console.
6. Add focused proof for:
   - stale seam retirement,
   - full refine handoff fidelity,
   - backlog and coverage persistence,
   - targeted rerun rationale,
   - growth/backlog visibility in reports.

### Execution Order
1. Red tests for config + bridge seams.
2. Red tests for report/comparison visibility.
3. Minimal runtime/catalog/workflow changes to make tests pass.
4. Focused verification on Proposal 022 slice.
