# Proposal-Loop Feedback Fidelity and Rereview

Stable reference for the proposal-loop feedback-fidelity, score-lift backlog, writer-coverage, and targeted-rereview slice.

## Purpose

The proposal loop must converge on higher review scores by carrying the full score-limiting review corpus into refinement, preserving issue lineage, and making the next review pass narrower and more explainable.

This document is the stable contract for:

- persisted `ReviewCorpusBundle` truth for each aggregate review pass,
- normalized `score_lift_backlog` with explicit merge provenance,
- structured writer coverage through `proposal_feedback_coverage`,
- persisted factual grounding and targeted-rereview planning,
- proposal-growth discipline tied to actual score lift,
- and shell-owned reporting, comparison, and artifact-inspection surfaces for proposal-loop progress.

## Scope

This reference covers:

- the proposal-review quartet plus aggregate summary as one canonical refine-handoff unit,
- backlog normalization and carry-forward semantics for score-limiting issues,
- merge-provenance visibility for collapsed or combined issues,
- writer coverage records and unresolved/deferred/disputed truth,
- targeted rerun scope planning for later review passes,
- proposal-growth and score-delta summary surfaced in existing shell-owned views,
- and the canonical remote `proposal-022` proof lane.

It does not introduce:

- a new transport or session-reuse substrate,
- a parallel operator console,
- a summary-only refine authority,
- or a local UI-proof requirement that conflicts with remote-only policy.

## Core Rules

### Refine handoff truth is bundle-owned

`proposal_review_summary` is not a standalone refine authority.

The canonical refine-handoff owner is `ReviewCorpusBundle`, which persists:

- `review_pass_id`,
- `review_iteration_id`,
- `source_proposal_artifact`,
- the raw quartet review artifact names,
- and the aggregate summary artifact name.

Writers may still consume the raw quartet and summary directly, but the persisted bundle is the canonical read surface that proves the full review corpus was present.

### Score-limiting issues must survive aggregation explicitly

The aggregate review step must persist `score_lift_backlog` as normalized structured items rather than leaving carry-forward truth in reviewer prose only.

Each backlog item persists:

- reviewer ownership,
- severity and blocker state,
- score-impact class,
- evidence refs,
- status,
- last touched iteration,
- and optional merge provenance.

If multiple raw issues are collapsed into one carry-forward item, that collapse must remain inspectable through `merge_provenance`.

### Writer coverage is a persisted contract, not an inferred summary

Each refine pass must emit `proposal_feedback_coverage`.

That coverage record is the canonical persisted answer for:

- which backlog items were addressed,
- which remain unresolved,
- which were deferred,
- which were disputed,
- which sections changed,
- and which factual claims were added or corrected.

Reports and recovery surfaces must read this persisted coverage truth before inventing UI heuristics.

### Targeted rereview is bounded and explainable

Later review passes may consume prior-loop truth through:

- `proposal_feedback_coverage`,
- `reviewer_scope_plan`,
- and `score_lift_backlog`.

The resulting targeted-rerun rationale must stay visible on shell-owned report and comparison surfaces so the operator can tell whether a reviewer needs:

- a full rerun,
- a delta rerun,
- or verification-only follow-up.

### Proposal growth must be tied to score movement

Proposal expansion is not success by itself.

The proposal loop persists growth and closure signals such as:

- proposal byte size,
- previous proposal byte size,
- proposal growth ratio,
- score delta since last review,
- backlog items closed count,
- reopened item count,
- growth-guard recommendation,
- and bounded next action.

If score lift stalls while proposal size grows, the surface must make that visible rather than rewarding expansion by default.

## Persistence and Artifact Owners

The canonical artifact names for this slice are:

- `review_corpus_bundle`
- `score_lift_backlog`
- `proposal_feedback_coverage`
- `proposal_fact_digest`
- `reviewer_scope_plan`
- `proposal_review_summary`
- raw quartet review artifacts:
  - `proposal_review_po`
  - `proposal_review_ux`
  - `proposal_review_ui`
  - `proposal_review_architect`

The aggregate-review stage owns:

- `review_corpus_bundle`
- `score_lift_backlog`
- `proposal_fact_digest`
- `reviewer_scope_plan`
- `proposal_review_summary`

The writer stage owns:

- `proposal_feedback_coverage`

Later reviewer stages may consume:

- `proposal_feedback_coverage`
- `reviewer_scope_plan`
- `score_lift_backlog`

## Read and Report Order

Proposal-loop visibility should read in this order:

1. canonical persisted proposal-loop artifacts,
2. parsed `ProposalLoopFeedbackSummary` derived from those artifacts,
3. shell-owned report and comparison surfaces,
4. UI fallback presentation only when canonical artifacts are absent.

If `review_corpus_bundle` or `score_lift_backlog` are absent, the loop should fail closed or degrade explicitly rather than silently treating summary-only truth as complete.

## Operator Surfaces

This slice extends the existing shell-owned operator spine:

- `RunReportView`
- `RunComparisonView`
- `ArtifactInspectorView`
- proposal-loop summaries surfaced from Ideas / run detail context

Those surfaces expose:

- review-corpus presence,
- raw-review artifact count,
- backlog totals and unresolved counts,
- merge-provenance count,
- targeted-rerun summary,
- coverage summary,
- growth ratio,
- score delta,
- and bounded next action.

No separate proposal-loop dashboard is required for the contract to hold.

## Verification Owners

The strongest current proof owners for this slice are:

- `Proposal022Tests`
- `Proposal022ScaffoldingTests`
- `scripts/test-gate.sh proposal-022`

The canonical proof lane is remote-only because the app-level proof must run on the approved remote host rather than through local UI automation.

Use [test-gates.md](test-gates.md) and [agent-ui-test-execution.md](agent-ui-test-execution.md) for the current execution policy of that gate.

## Adjacent References

Use:

- [live-provider-execution-slice.md](live-provider-execution-slice.md) for the broader live proposal-loop runtime,
- [context-strategy-and-experiment-framework.md](context-strategy-and-experiment-framework.md) for strategy-owned handoff shaping and experiment policy,
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md) for structured artifact and narrow-recovery rules,
- [operator-experience.md](operator-experience.md) for shell ownership,
- [test-gates.md](test-gates.md) for the canonical proof lane.
