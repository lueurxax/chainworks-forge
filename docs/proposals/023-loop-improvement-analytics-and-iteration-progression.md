# Proposal 023: Loop Improvement Analytics and Iteration Progression

| Field | Value |
|---|---|
| Date | 2026-04-02 |
| Status | Draft |
| Author | Codex |
| Depends on | [../reference/operator-experience.md](../reference/operator-experience.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [../reference/proposal-loop-feedback-fidelity-and-rereview.md](../reference/proposal-loop-feedback-fidelity-and-rereview.md), [../reference/domain-model.md](../reference/domain-model.md) |
| Scope | Add a canonical run-owned loop-improvement analytics contract, a normalized iteration progression series for any looped workflow, and a dedicated read-only operator page for improvement history and evidence-linked trend inspection. |
| Goal | Let operators see whether a looped workflow is actually improving across iterations, without tying analytics to specific agents, artifact filenames, or workflow YAML shapes. |

---

## 1. Context and Motivation

The system already persists rich loop artifacts for some workflows, especially proposal-loop review and refine cycles. Those artifacts often contain strong evidence that the output improved between iterations:

- reviewer or evaluator scores
- addressed vs. unresolved backlog items
- coverage summaries
- retry and rereview history
- revision summaries that explain what changed

Today this improvement truth is hard to inspect as a first-class operator surface. It is scattered across artifacts and stage history, which creates three problems:

1. Operators can feel that a run is making progress, but the shell does not show a canonical progression view.
2. Workflows with strong loop evidence remain difficult to compare across iterations without opening raw artifacts manually.
3. Any ad-hoc UI that parses artifact names directly would create a second truth system and inevitably become workflow-specific.

The system needs a canonical, run-owned, workflow-agnostic way to represent iteration progression for any looped workflow.

This proposal does **not** create a second report system, and it does **not** hardcode proposal-loop assumptions into a supposedly general feature. It introduces a normalized iteration analytics contract that workflows may publish, and a shell surface that reads only that canonical contract.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can the shell show whether a looped run is improving across iterations?
2. Can this work for loops that have numeric score signals and loops that do not?
3. Can the analytics remain run-owned and evidence-linked rather than derived from UI heuristics?
4. Can relaunch, retry, resume, and repeated stage iterations preserve truthful progression lineage?
5. Can operators inspect loop progression from a dedicated run surface without cluttering the primary run detail screen?

---

## 3. Scope

This proposal includes:

- a canonical normalized loop-improvement contract persisted on `Run`
- iteration snapshots for any looped workflow
- support for both score-bearing and non-score loops
- a dedicated run-owned operator page for progression analytics
- evidence links from iteration metrics back into existing artifact and report surfaces
- immutable report integration that reads the same canonical progression series

This proposal does **not** include:

- new operator decision actions authored from the analytics screen
- workflow-specific parsing rules based on artifact filenames
- speculative analytics for workflows that did not publish normalized loop metrics
- a second inbox, dashboard, or top-level shell tab
- cross-run benchmarking or cohort ranking beyond the current run

---

## 4. Core Product Behavior

### 4.1 Dedicated run-owned analytics page

Loop improvement analytics should live **inside the run**, but not on the primary run overview.

The operator opens a dedicated page from run detail via a separate action, for example:

- `Improvement`
- `Iteration Progress`
- `Loop Analytics`

The page must be a navigated run sub-surface, not a modal. It needs enough space for:

- a progression headline
- a timeline of iterations
- metric deltas
- breakdown cards
- links to evidence

### 4.2 Read-only by design

This page is analytics-only.

It may provide deep links to:

- existing artifacts
- existing reports
- existing blocked/recovery surfaces
- existing stage detail surfaces

It must **not** introduce new operator actions such as:

- approve from analytics
- retry from analytics
- accept recommendation from analytics
- mutate backlog state from analytics

### 4.3 Preferred metric model

If a loop provides numeric quality signals, the page should present score progression as the primary headline metric.

If a loop does not provide numeric scores, the page must still work through normalized progression metrics such as:

- addressed items
- unresolved items
- deferred items
- coverage ratio
- retry count
- pass/fail trend
- completeness / confidence flags

The system must never fabricate a score for loops that do not provide one.

---

## 5. Canonical Authority and Data Ownership

### 5.1 Run-owned progression truth

The canonical owner for loop-improvement analytics must be the `Run`.

Analytics may be derived from persisted stage/agent evidence, but once normalized they must be stored as run-owned immutable progression snapshots.

The shell must read from this normalized run-owned contract, not from arbitrary live artifact parsing.

### 5.2 No artifact-name authority

Artifact names like `proposal_review_summary`, `score_lift_backlog`, or `proposal_feedback_coverage` are implementation details of particular workflows.

They must not become the general analytics authority.

Workflow-specific publishers may read such artifacts internally when constructing normalized progression snapshots, but the shell and reports must never depend on those raw names.

### 5.3 No UI-derived truth

Analytics must not be inferred from:

- visible stage labels
- current screen state
- report regeneration heuristics
- non-canonical log snippets

If the normalized progression contract is missing or incomplete, the UI must show that explicitly rather than improvising.

---

## 6. New Domain Model

### 6.1 `LoopImprovementSeries`

Persisted on `Run` as the canonical analytics surface for loop progression.

```swift
struct LoopImprovementSeries: Codable, Sendable {
    let seriesID: String
    let loopFamily: String
    let summary: LoopImprovementSummary
    let iterations: [LoopImprovementSnapshot]
}
```

### 6.2 `LoopImprovementSummary`

Top-level run-owned summary for the dedicated analytics page.

```swift
struct LoopImprovementSummary: Codable, Sendable {
    let baselineIteration: Int?
    let latestIteration: Int?
    let primaryMetricKind: PrimaryImprovementMetricKind
    let baselineValue: Double?
    let latestValue: Double?
    let totalDelta: Double?
    let targetValue: Double?
    let trend: ImprovementTrend
    let completeness: LoopImprovementCompleteness
}
```

`PrimaryImprovementMetricKind` examples:

- `score`
- `coverageRatio`
- `addressedItems`
- `unresolvedItems`
- `customNamedMetric`

### 6.3 `LoopImprovementSnapshot`

Represents one normalized iteration of a loop.

```swift
struct LoopImprovementSnapshot: Codable, Sendable, Hashable {
    let snapshotID: String
    let iteration: Int
    let semanticPhase: LoopSemanticPhase
    let sourceStageExecutionID: UUID?
    let sourceStageID: String?
    let createdAt: Date
    let primaryMetric: ImprovementMetricValue?
    let deltaFromPrevious: Double?
    let deltaFromBaseline: Double?
    let targetValue: Double?
    let secondaryMetrics: [ImprovementMetricValue]
    let evidenceLinks: [ImprovementEvidenceLink]
    let trust: LoopImprovementTrust
}
```

### 6.4 `ImprovementMetricValue`

Normalized metric value with direction semantics.

```swift
struct ImprovementMetricValue: Codable, Sendable, Hashable {
    let key: String
    let label: String
    let numericValue: Double?
    let integerValue: Int?
    let displayValue: String
    let preferredDirection: MetricDirection
}
```

`MetricDirection` examples:

- `higherIsBetter`
- `lowerIsBetter`
- `informational`

### 6.5 `ImprovementEvidenceLink`

Link back to already-canonical run evidence.

```swift
struct ImprovementEvidenceLink: Codable, Sendable, Hashable {
    let artifactID: UUID?
    let stageExecutionID: UUID?
    let label: String
    let kind: ImprovementEvidenceKind
}
```

Examples:

- summary artifact
- coverage artifact
- backlog artifact
- reviewer output artifact
- immutable report anchor

### 6.6 `LoopImprovementTrust`

Explicit trust/completeness state for each iteration snapshot:

- `complete`
- `partial`
- `unverifiable`

The page must surface this directly. It must never silently present partial analytics as complete truth.

---

## 7. Publisher Contract

### 7.1 General rule

The analytics layer is workflow-agnostic, but workflows must publish normalized improvement snapshots intentionally.

The shell should not reverse-engineer them from arbitrary artifacts.

### 7.2 Publisher responsibility

A workflow that wants progression analytics must provide enough canonical information to construct `LoopImprovementSnapshot` values.

This may happen through:

- explicit stage-owned normalized artifacts
- run-owned normalized envelopes
- a dedicated publisher step that consolidates loop evidence into the run-owned series

### 7.3 Score-bearing loops

If a loop publishes numeric quality scores, it must also publish:

- metric label
- metric direction
- optional target
- iteration identity

Example categories:

- proposal review score
- evaluation confidence
- implementation quality score

### 7.4 Non-score loops

A loop without numeric scores may still publish useful progression through normalized metrics such as:

- addressed vs unresolved items
- regression count
- coverage ratio
- blocker count
- retry count
- issue severity counts

The contract must support these without pretending they are score equivalents.

### 7.5 Fail-closed rule

If a workflow cannot publish a truthful normalized snapshot for an iteration, it must publish either:

- a partial snapshot with explicit trust `partial`
- or no snapshot at all

The system must prefer explicit incompleteness over fabricated progression certainty.

---

## 8. Lineage, Retry, and Resume Semantics

### 8.1 Iteration identity

Each analytics snapshot must attach to canonical iteration identity, not just a display label.

It must survive:

- retry agent
- retry stage
- resume after interruption
- relaunch restore

### 8.2 No duplicate snapshots from stale retries

If a retry reuses an existing iteration lineage, the analytics series must not duplicate an already-persisted snapshot unless a genuinely new iteration truth exists.

### 8.3 Frozen historical truth

Older snapshots must remain frozen even if later parsing logic or UI changes evolve.

Historical runs must continue to show the progression truth they actually published.

### 8.4 Freshness and replacement

If a stage iteration is retried and superseded before analytics are finalized, the system must define one canonical winner for that iteration snapshot rather than leaving parallel mutable candidates.

This authority must align with the existing run/stage recovery spine.

---

## 9. Operator Surface

### 9.1 Entry point

Run detail gets a dedicated action:

- `Improvement`

This opens a separate page inside the current run surface hierarchy.

### 9.2 Page structure

The page should include:

1. **Headline progression summary**
   - primary metric current value
   - total delta from baseline
   - target status if available
   - completeness / trust badge

2. **Iteration timeline**
   - ordered progression cards
   - iteration number
   - semantic phase
   - timestamp
   - delta

3. **Metric breakdown**
   - primary metric
   - secondary metrics
   - score or non-score breakdown depending on series type

4. **Evidence links**
   - deep links to artifacts and existing run surfaces

5. **Integrity state**
   - clear indication when analytics are partial, incomplete, or unavailable

### 9.3 Page behavior

This page must:

- feel native to the current run shell
- stay read-only
- avoid giant text dumps
- prefer compact summary cards and expandable drill-down rows

### 9.4 Empty and partial states

The page must support:

- `No loop analytics available for this run`
- `Loop analytics available, but partial`
- `Loop analytics complete`

These states must be visually distinct.

---

## 10. Immutable Report and Evidence Integration

The immutable run report may summarize progression analytics, but it must read from the same canonical `LoopImprovementSeries`.

It must not re-derive its own separate loop metrics.

At minimum, reports should be able to surface:

- number of iterations
- primary metric progression
- total delta
- latest trust/completeness state

This keeps run detail, analytics page, and immutable report aligned to one authority.

---

## 11. Architecture Boundaries

### 11.1 Extend existing shell owners

This proposal must extend current shell-owned run surfaces.

It must not introduce:

- a new top-level analytics tab
- a separate loop analytics workspace
- a second report product

### 11.2 Agnostic by contract, not by wishful naming

Workflow agnosticism here means:

- no references to specific YAML IDs in the shell layer
- no proposal-loop-only naming in the domain model
- no hardcoded artifact-name parsing in the operator page

It does **not** mean that every workflow magically gets analytics without publishing normalized loop progression truth.

### 11.3 Prefer explicit contracts over heuristics

If a workflow wants this surface, it must publish normalized loop progression inputs.

The shell must remain simple and deterministic by consuming those published contracts.

---

## 12. Rollout Plan

1. **Phase 1: Canonical data model**
   - Add run-owned normalized progression series and iteration snapshot types
   - Define trust/completeness semantics
   - Define evidence-link contract

2. **Phase 2: Publisher integration**
   - Wire at least one score-bearing loop into the normalized contract
   - Wire at least one non-score loop into the normalized contract
   - Verify retry/resume lineage correctness

3. **Phase 3: Operator page**
   - Add dedicated run-level navigation action
   - Implement progression page with summary, timeline, metrics, and evidence links
   - Add empty/partial states

4. **Phase 4: Report integration**
   - Extend immutable report surfaces to summarize the same progression series
   - Verify no second analytics derivation exists

5. **Phase 5: Proof and hardening**
   - Add regression tests for normalized progression persistence
   - Add UI proof for entry point and page rendering
   - Add same-head gate coverage for at least one score and one non-score loop

---

## 13. Acceptance Criteria

This proposal is complete only when:

1. A run can persist a canonical `LoopImprovementSeries`.
2. At least one score-bearing looped workflow publishes numeric progression into that series.
3. At least one non-score looped workflow publishes meaningful progression without numeric score.
4. The run UI exposes a dedicated analytics page through a separate run action.
5. The analytics page is read-only and provides deep links into existing evidence surfaces.
6. Retry, resume, and relaunch preserve truthful iteration lineage without duplicate analytics snapshots.
7. Immutable report output reads from the same canonical progression series.
8. The shell never parses arbitrary artifact names directly to derive progression truth.
9. Partial or missing progression data is surfaced honestly rather than inferred heuristically.

---

## 14. Alternatives Considered

### Quick artifact parsing in run detail

Rejected.

It would be fast initially, but it would immediately couple the shell to workflow-specific artifact naming and produce fragile, misleading analytics.

### Modal instead of dedicated page

Rejected.

A modal is too constrained for a timeline, metric cards, breakdowns, and evidence links. This surface is part of run understanding, not a transient dialog.

### Score-only analytics

Rejected.

That would make the feature proposal-loop-shaped even if it claimed to be general. Many looped workflows improve meaningfully without a numeric score.

### Top-level analytics dashboard

Rejected.

This would create a second operator surface disconnected from run-owned truth.

---

## 15. Open Design Constraints

The implementation must answer these explicitly:

1. Which component owns normalized snapshot publication: stage executors, orchestrator consolidation, or a dedicated progression publisher?
2. What is the canonical replacement rule when a retry supersedes an earlier attempt in the same logical iteration?
3. How should custom named metrics be rendered so the page remains generic but still understandable?
4. How should partial analytics be summarized in immutable reports without overstating certainty?

These are implementation questions, not reasons to weaken the ownership model above.
