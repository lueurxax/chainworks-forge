# Proposal 048: Steward Recommendation Lifecycle and Experiment Tracking

| Field | Value |
|---|---|
| Date | 2026-04-17 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [../reference/forge-steward.md](../reference/forge-steward.md), [../reference/context-strategy-and-experiment-framework.md](../reference/context-strategy-and-experiment-framework.md) |
| Scope | Add accept/reject/defer lifecycle actions for steward recommendations via MCP and GraphQL, introduce experiment cohort tracking for accepted tuning recommendations, and close the feedback loop with post-recommendation impact analysis. |
| Goal | An operator can act on steward recommendations with full accountability (reject requires rationale, defer batches changes), and the system measures whether accepted recommendations actually improved the target metric. |

---

## 1. Context and Motivation

The Steward V1 pipeline (implemented per [forge-steward.md](../reference/forge-steward.md)) runs anomaly detection across completed runs, produces `StewardRecommendation` records with category, target metric, and confidence level, and persists them to SwiftData. MCP tools exist for `steward.run_analysis`, `steward.list_analyses`, and `steward.get_analysis`. The Swift app has basic `StewardDecision` and `StewardExperiment` models defined as V3 placeholders but unused.

Three gaps prevent the steward from being operationally useful beyond observation:

1. **No action tools.** There is no MCP tool or GraphQL mutation to accept, reject, or defer a recommendation. The operator can read recommendations but cannot act on them through the control-plane. The Swift app defines `RecommendationStatus` values (`proposed`, `approved`, `rejected`, `superseded`, `adoptedAfterExperiment`, `rolledBack`) but nothing writes those transitions.

2. **No accountability on rejection.** The current `StewardRecommendation` model has an optional `decisionComment` field. Nothing enforces that a rejection includes a rationale. Silent dismissal of recommendations makes it impossible to audit whether the operator is engaging with steward output or ignoring it.

3. **No feedback loop.** When a recommendation is accepted, there is no mechanism to track whether subsequent runs actually improved on the target metric. The `StewardExperiment` model exists as a placeholder with `controlConfigHash` and `treatmentConfigHash` fields, but nothing populates it. The operator must manually compare pre-change and post-change runs to judge effectiveness.

The combined effect is a steward that observes and recommends but cannot learn whether its recommendations work.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can the operator accept a steward recommendation through MCP and GraphQL, with the system recording who accepted it and when?
2. Can the operator reject a recommendation only by providing a rationale, preventing silent dismissal?
3. Can the operator defer a recommendation to a specific future date without losing track of it?
4. When a tuning recommendation is accepted, does the system automatically track subsequent runs as an experiment cohort?
5. After enough post-acceptance runs complete, can the operator retrieve a quantitative impact assessment comparing before and after values of the target metric?
6. Can the operator query experiment cohorts to see individual run-level metric values for both baseline and experiment groups?
7. Do GraphQL subscriptions fire when recommendation status changes or impact data becomes available?
8. Does the existing `stewardRecommendations` query support filtering by status and sorting by confidence?

---

## 3. Scope

This proposal includes:

- Four new MCP tools: `steward.accept_recommendation`, `steward.reject_recommendation`, `steward.defer_recommendation`, `steward.recommendation_impact`.
- Three new GraphQL mutations: `acceptRecommendation`, `rejectRecommendation`, `deferRecommendation`.
- Enhancement of the existing `stewardRecommendations` query with status filtering, impact summary, and confidence sorting.
- One new GraphQL query: `stewardExperimentCohorts`.
- One new GraphQL subscription: `stewardRecommendationChanged`.
- Schema additions: `defer_until` and impact fields on `StewardRecommendation`, new `steward_experiment_cohorts` table.
- SQLite migration for new columns and table.

This proposal does **not** include:

- Changes to the Steward V1 analysis pipeline (anomaly detection, metrics collection, cohort classification).
- Automatic application of accepted recommendations (the operator still applies changes manually).
- Schedule-based steward triggers (the `cron` field remains unwired per V1 scope).
- Swift app UI for recommendation lifecycle (this proposal covers the control-plane daemon only).
- Changes to `steward_config.yaml` schema.
- Cross-recommendation experiment design (each recommendation tracks its own cohort independently).

---

## 4. Problem Statement

### 4.1 Recommendations are write-once, read-only

`StewardRecommendation` records are created during step 11 of the analysis pipeline with `status: proposed`. The `decisionComment`, `decidedAt`, `status` fields exist but no tool or mutation writes to them. The operator's only interaction path is reading recommendations through `steward.get_analysis`.

### 4.2 Rejection has no accountability constraint

The `decisionComment` field is `Option<String>`. Nothing prevents a transition to `rejected` with a null comment. In the Swift app, `StewardDecision` has a `rationale: String` field, but the control-plane daemon has no equivalent enforcement. Without mandatory rationale on rejection, the steward's recommendations can be silently dismissed, and post-hoc auditing cannot determine whether the operator evaluated the recommendation seriously.

### 4.3 No defer action exists

The current `RecommendationStatus` enum has no `deferred` variant. An operator who agrees with a recommendation but wants to batch it with other changes has two options: accept immediately (premature) or leave it as `proposed` (loses the intent to act later). There is no way to signal "acknowledged, will act on date X."

### 4.4 Accepted recommendations produce no measurable outcome

When the operator accepts a recommendation (once that action exists), the system does not automatically:

- identify which metric the recommendation targets,
- snapshot the pre-change metric value,
- tag subsequent runs as part of an experiment cohort,
- or compute whether the metric improved.

The `StewardExperiment` model in the Swift app has the right shape (`controlConfigHash`, `treatmentConfigHash`, `minimumSampleSize`) but is unused. The control-plane daemon has no equivalent.

### 4.5 No experiment cohort visibility

Even if the operator manually compares runs before and after a change, there is no structured way to query "which runs belong to the baseline cohort and which belong to the experiment cohort for recommendation X." The `StewardAnalysisRunLink` table links runs to analyses, not to recommendations or experiments.

---

## 5. Core Product Behavior

### 5.1 MCP Tool: `steward.accept_recommendation`

Accept a steward recommendation, optionally flagging subsequent runs for experiment tracking.

**Input JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "recommendation_id": {
      "type": "string",
      "format": "uuid",
      "description": "ID of the recommendation to accept."
    },
    "comment": {
      "type": "string",
      "description": "Optional operator comment explaining the acceptance rationale."
    }
  },
  "required": ["recommendation_id"]
}
```

**Behavior:**

1. Look up the `StewardRecommendation` by ID. Return error if not found or if `status != proposed` and `status != deferred`.
2. Set `status` to `accepted`, `decided_at` to current timestamp, `decision_comment` to the provided comment (or null).
3. If the recommendation has `category` of `agent_tuning` or `workflow_tuning`:
   a. Read `experiment_cohort_size` from steward config (default: 5).
   b. Snapshot the current value of the recommendation's `target_metric` from the most recent analysis baseline as `impact_before_value`.
   c. Create a baseline cohort by selecting the last N completed runs (where N = `experiment_cohort_size`) that share the recommendation's analysis cohort keys, and inserting one `steward_experiment_cohort` row per run with `cohort_type = baseline`.
   d. Mark the recommendation as experiment-tracking-active. Subsequent completed runs matching the cohort keys will be automatically added to the experiment cohort (up to `experiment_cohort_size` runs) with `cohort_type = experiment`.
4. Return the updated recommendation.

**Output JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "id": { "type": "string", "format": "uuid" },
    "status": { "type": "string", "enum": ["accepted"] },
    "decided_at": { "type": "string", "format": "date-time" },
    "decision_comment": { "type": ["string", "null"] },
    "category": { "type": "string" },
    "target_metric": { "type": "string" },
    "confidence_level": { "type": "string" },
    "experiment_tracking": {
      "type": ["object", "null"],
      "properties": {
        "baseline_cohort_size": { "type": "integer" },
        "experiment_cohort_target": { "type": "integer" },
        "experiment_cohort_collected": { "type": "integer" },
        "impact_before_value": { "type": ["number", "null"] }
      }
    }
  }
}
```

### 5.2 MCP Tool: `steward.reject_recommendation`

Reject a steward recommendation. The operator must provide a rationale.

**Input JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "recommendation_id": {
      "type": "string",
      "format": "uuid",
      "description": "ID of the recommendation to reject."
    },
    "comment": {
      "type": "string",
      "description": "Mandatory rationale explaining why the recommendation is rejected."
    }
  },
  "required": ["recommendation_id", "comment"]
}
```

**Behavior:**

1. Look up the `StewardRecommendation` by ID. Return error if not found or if `status != proposed` and `status != deferred`.
2. Validate that `comment` is non-empty after trimming whitespace. Return a validation error if empty.
3. Set `status` to `rejected`, `decided_at` to current timestamp, `decision_comment` to the provided comment.
4. If experiment tracking was active (recommendation was previously deferred after partial acceptance), cancel it and mark any incomplete experiment cohort rows as `cancelled`.
5. Return the updated recommendation.

**Output JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "id": { "type": "string", "format": "uuid" },
    "status": { "type": "string", "enum": ["rejected"] },
    "decided_at": { "type": "string", "format": "date-time" },
    "decision_comment": { "type": "string" },
    "category": { "type": "string" },
    "target_metric": { "type": "string" },
    "confidence_level": { "type": "string" }
  }
}
```

**Difference from Swift app:** The Swift `StewardDecision` model has `rationale: String` (non-optional), but the decision is a separate entity linked through `StewardExperiment`. This proposal makes the comment mandatory at the API boundary (`required` in schema, non-empty validation in handler) rather than through a separate model. This is simpler and achieves the same accountability goal.

### 5.3 MCP Tool: `steward.defer_recommendation`

Defer a recommendation for later review. This action is new and has no equivalent in the Swift app.

**Input JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "recommendation_id": {
      "type": "string",
      "format": "uuid",
      "description": "ID of the recommendation to defer."
    },
    "defer_until": {
      "type": "string",
      "format": "date",
      "description": "ISO 8601 date (YYYY-MM-DD) when the recommendation should resurface for review."
    },
    "comment": {
      "type": "string",
      "description": "Optional explanation for why the recommendation is being deferred."
    }
  },
  "required": ["recommendation_id", "defer_until"]
}
```

**Behavior:**

1. Look up the `StewardRecommendation` by ID. Return error if not found or if `status != proposed`.
2. Validate that `defer_until` is a future date. Return a validation error if it is today or in the past.
3. Set `status` to `deferred`, `defer_until` to the provided date, `decided_at` to current timestamp, `decision_comment` to the provided comment (or null).
4. The recommendation remains visible in queries filtered by `status = deferred`. It does not disappear from the operator's view.
5. Return the updated recommendation.

**Output JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "id": { "type": "string", "format": "uuid" },
    "status": { "type": "string", "enum": ["deferred"] },
    "defer_until": { "type": "string", "format": "date" },
    "decided_at": { "type": "string", "format": "date-time" },
    "decision_comment": { "type": ["string", "null"] },
    "category": { "type": "string" },
    "target_metric": { "type": "string" },
    "confidence_level": { "type": "string" }
  }
}
```

**Re-evaluation:** When the deferred date arrives, the recommendation does not automatically transition back to `proposed`. Instead, the `stewardRecommendations` query (SS5.8) surfaces deferred recommendations whose `defer_until` date has passed, allowing the operator to accept or reject them at that point. Automatic status transitions would create invisible state changes.

### 5.4 MCP Tool: `steward.recommendation_impact`

Compute the quantitative impact of an accepted recommendation by comparing pre-change and post-change metric cohorts. This tool is new and has no equivalent in the Swift app.

**Input JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "recommendation_id": {
      "type": "string",
      "format": "uuid",
      "description": "ID of an accepted recommendation with experiment tracking."
    }
  },
  "required": ["recommendation_id"]
}
```

**Behavior:**

1. Look up the `StewardRecommendation` by ID. Return error if not found.
2. Return error if `status != accepted` or if no experiment tracking is active for this recommendation.
3. Query `steward_experiment_cohorts` for rows matching this recommendation, partitioned by `cohort_type`.
4. If the experiment cohort has fewer than 3 runs, return an `insufficient_data` response with the current sample size and the target size.
5. Compute `before_value` as the median of `metric_value` across baseline cohort rows.
6. Compute `after_value` as the median of `metric_value` across experiment cohort rows.
7. Compute `change_percent` as `((after_value - before_value) / before_value) * 100`.
8. Determine `verdict`:
   - `improved` if the metric moved in the expected direction by at least 5% (for timing/cost/rework metrics, lower is better; for quality metrics, higher is better). The direction is derived from the recommendation's `target_metric` family.
   - `degraded` if the metric moved in the wrong direction by at least 5%.
   - `no_change` if the absolute change is less than 5%.
9. Compute `confidence_level`:
   - `high` if both cohorts have at least 10 runs each.
   - `medium` if both cohorts have at least 5 runs each.
   - `low` otherwise.
10. Persist the computed values on the recommendation: `impact_before_value`, `impact_after_value`, `impact_verdict`.
11. Emit a `stewardRecommendationChanged` subscription event with `reason: impact_available`.
12. Return the impact assessment.

**Output JSON Schema:**

```json
{
  "type": "object",
  "properties": {
    "recommendation_id": { "type": "string", "format": "uuid" },
    "target_metric": { "type": "string" },
    "before_value": { "type": "number" },
    "after_value": { "type": "number" },
    "change_percent": { "type": "number" },
    "verdict": {
      "type": "string",
      "enum": ["improved", "degraded", "no_change", "insufficient_data"]
    },
    "confidence_level": {
      "type": "string",
      "enum": ["high", "medium", "low"]
    },
    "sample_size": {
      "type": "object",
      "properties": {
        "baseline": { "type": "integer" },
        "experiment": { "type": "integer" }
      }
    }
  }
}
```

### 5.5 GraphQL Mutation: `acceptRecommendation`

```graphql
type Mutation {
  acceptRecommendation(id: ID!, comment: String): StewardRecommendation!
}
```

Delegates to the same handler as `steward.accept_recommendation`. Returns the full `StewardRecommendation` object including experiment tracking state.

### 5.6 GraphQL Mutation: `rejectRecommendation`

```graphql
type Mutation {
  rejectRecommendation(id: ID!, comment: String!): StewardRecommendation!
}
```

The `comment` argument is non-nullable (`String!`), enforcing the mandatory-rationale constraint at the schema level. Delegates to the same handler as `steward.reject_recommendation`.

### 5.7 GraphQL Mutation: `deferRecommendation`

```graphql
type Mutation {
  deferRecommendation(id: ID!, deferUntil: DateTime!, comment: String): StewardRecommendation!
}
```

Delegates to the same handler as `steward.defer_recommendation`.

### 5.8 GraphQL Query: `stewardRecommendations` (enhanced)

The existing query returns recommendations for a given analysis. This proposal adds:

```graphql
type Query {
  stewardRecommendations(
    analysisId: ID
    status: RecommendationStatusFilter
    includeOverdue: Boolean
  ): [StewardRecommendation!]!
}

enum RecommendationStatusFilter {
  PROPOSED
  ACCEPTED
  REJECTED
  DEFERRED
}
```

**Enhancements:**

- `status` filter: return only recommendations with the given status. When omitted, return all.
- `includeOverdue`: when `true` and `status = DEFERRED`, also include deferred recommendations whose `defer_until` date has passed. Default `false`.
- Results are sorted by `confidence_level` descending (high first), then by `created_at` descending.
- Each returned `StewardRecommendation` includes an `impactSummary` field (nullable) that is populated when the recommendation has been accepted and impact analysis has been computed:

```graphql
type StewardRecommendation {
  id: ID!
  createdAt: DateTime!
  category: String!
  summary: String!
  targetMetric: String!
  confidenceLevel: String!
  status: String!
  decisionComment: String
  decidedAt: DateTime
  deferUntil: DateTime
  impactSummary: ImpactSummary
  experimentTracking: ExperimentTracking
}

type ImpactSummary {
  beforeValue: Float!
  afterValue: Float!
  changePercent: Float!
  verdict: String!
  confidenceLevel: String!
  sampleSize: CohortSampleSize!
}

type CohortSampleSize {
  baseline: Int!
  experiment: Int!
}

type ExperimentTracking {
  baselineCohortSize: Int!
  experimentCohortTarget: Int!
  experimentCohortCollected: Int!
}
```

### 5.9 GraphQL Query: `stewardExperimentCohorts`

New query for inspecting the individual runs in an experiment.

```graphql
type Query {
  stewardExperimentCohorts(recommendationId: ID!): ExperimentCohortReport
}

type ExperimentCohortReport {
  recommendationId: ID!
  targetMetric: String!
  baseline: [CohortEntry!]!
  experiment: [CohortEntry!]!
}

type CohortEntry {
  runId: ID!
  metricValue: Float!
  runStatus: String!
  recordedAt: DateTime!
}
```

**Behavior:**

- Returns null if the recommendation has no experiment tracking.
- Both `baseline` and `experiment` arrays are sorted by `recorded_at` ascending.
- Each entry includes the run's completion status so the operator can judge data quality.

### 5.10 GraphQL Subscription: `stewardRecommendationChanged`

```graphql
type Subscription {
  stewardRecommendationChanged: RecommendationChangeEvent!
}

type RecommendationChangeEvent {
  recommendation: StewardRecommendation!
  reason: RecommendationChangeReason!
  changedAt: DateTime!
}

enum RecommendationChangeReason {
  ACCEPTED
  REJECTED
  DEFERRED
  IMPACT_AVAILABLE
  EXPERIMENT_COHORT_UPDATED
}
```

**Emission rules:**

- `ACCEPTED` / `REJECTED` / `DEFERRED`: emitted immediately when the corresponding mutation or MCP tool completes.
- `IMPACT_AVAILABLE`: emitted when `steward.recommendation_impact` successfully computes and persists impact data.
- `EXPERIMENT_COHORT_UPDATED`: emitted when a new completed run is added to the experiment cohort (via the post-run hook in `ExecutionService`).

### 5.11 Experiment cohort auto-population

When a recommendation with `category = agent_tuning` or `category = workflow_tuning` is accepted and experiment tracking is active, the existing `ExecutionService.notifyRunCompleted()` hook gains an additional step:

1. After the existing steward trigger check, query all recommendations with `status = accepted` and active experiment tracking.
2. For each such recommendation, check whether the completed run matches the recommendation's cohort keys (workflow family and risk class from the parent analysis).
3. If the experiment cohort has not yet reached `experiment_cohort_size`, insert a `steward_experiment_cohort` row with `cohort_type = experiment`, the run's ID, and the run's value for the recommendation's `target_metric`.
4. Emit a `stewardRecommendationChanged` subscription event with `reason: EXPERIMENT_COHORT_UPDATED`.
5. If the experiment cohort has reached `experiment_cohort_size`, log that the experiment is complete and the operator can run `steward.recommendation_impact`.

This piggybacks on the existing post-run hook rather than introducing a new trigger mechanism.

---

## 6. Migration

### 6.1 Extend `steward_recommendations` table

Add three columns:

```sql
ALTER TABLE steward_recommendations ADD COLUMN defer_until TEXT;           -- ISO 8601 date, nullable
ALTER TABLE steward_recommendations ADD COLUMN impact_before_value REAL;   -- nullable
ALTER TABLE steward_recommendations ADD COLUMN impact_after_value REAL;    -- nullable
ALTER TABLE steward_recommendations ADD COLUMN impact_verdict TEXT;        -- nullable: improved/degraded/no_change
```

### 6.2 Extend `RecommendationStatus` enum

Add two new variants to the Rust enum and the corresponding SQLite check constraint:

```rust
enum RecommendationStatus {
    Proposed,
    Accepted,     // renamed from Approved for clarity
    Rejected,
    Deferred,     // NEW
    Superseded,
    AdoptedAfterExperiment,
    RolledBack,
}
```

**Migration note:** The Swift app uses `approved` rather than `accepted`. The control-plane daemon should use `accepted` as the canonical value and map `approved` to `accepted` when reading historical data from shared storage, if applicable. The two systems do not currently share a SQLite database, so this is a naming convention difference only.

### 6.3 New table: `steward_experiment_cohorts`

```sql
CREATE TABLE steward_experiment_cohorts (
    id              TEXT PRIMARY KEY,   -- UUID
    recommendation_id TEXT NOT NULL REFERENCES steward_recommendations(id),
    cohort_type     TEXT NOT NULL,      -- 'baseline' or 'experiment'
    run_id          TEXT NOT NULL,
    metric_name     TEXT NOT NULL,
    metric_value    REAL NOT NULL,
    recorded_at     TEXT NOT NULL,      -- ISO 8601 datetime
    UNIQUE(recommendation_id, run_id)
);

CREATE INDEX idx_experiment_cohorts_recommendation
    ON steward_experiment_cohorts(recommendation_id, cohort_type);
```

The `UNIQUE(recommendation_id, run_id)` constraint prevents the same run from appearing in both baseline and experiment cohorts for the same recommendation. A run may appear in cohorts for different recommendations.

### 6.4 Add experiment tracking metadata to recommendations

Add a column to track whether experiment auto-population is active and what the target cohort size is:

```sql
ALTER TABLE steward_recommendations ADD COLUMN experiment_cohort_target INTEGER;  -- nullable; null = no experiment tracking
```

When `experiment_cohort_target` is non-null, the post-run hook checks for cohort population. When the experiment cohort reaches the target size, the value remains set but no further rows are added.

---

## 7. Verification

### 7.1 Accept lifecycle

- Call `steward.accept_recommendation` with a valid proposed recommendation ID. Verify status changes to `accepted`, `decided_at` is set, and the response includes the updated recommendation.
- Call `steward.accept_recommendation` on an already-accepted recommendation. Verify it returns an error indicating invalid status transition.
- Call `steward.accept_recommendation` on a deferred recommendation. Verify it succeeds (deferred -> accepted is a valid transition).

### 7.2 Reject lifecycle with mandatory comment

- Call `steward.reject_recommendation` with a valid recommendation ID and a non-empty comment. Verify status changes to `rejected` and the comment is persisted.
- Call `steward.reject_recommendation` with an empty string comment. Verify it returns a validation error.
- Call `steward.reject_recommendation` with a whitespace-only comment. Verify it returns a validation error.
- Call `steward.reject_recommendation` without a comment field. Verify it returns a schema validation error (missing required field).

### 7.3 Defer lifecycle

- Call `steward.defer_recommendation` with a valid recommendation ID and a future date. Verify status changes to `deferred` and `defer_until` is set.
- Call `steward.defer_recommendation` with today's date. Verify it returns a validation error.
- Query `stewardRecommendations(status: DEFERRED)`. Verify deferred recommendations appear.
- Query `stewardRecommendations(status: DEFERRED, includeOverdue: true)` when a deferred recommendation's `defer_until` has passed. Verify it appears in the results.

### 7.4 Experiment cohort tracking

- Accept a recommendation with `category = agent_tuning`. Verify that baseline cohort rows are created from recent completed runs.
- Complete a new run that matches the recommendation's cohort keys. Verify a new `steward_experiment_cohort` row is inserted with `cohort_type = experiment`.
- Complete runs until the experiment cohort reaches the target size. Verify no further rows are added after the target is met.
- Accept a recommendation with `category = backend_change`. Verify no experiment tracking is activated.

### 7.5 Impact analysis

- Call `steward.recommendation_impact` on an accepted recommendation with a full experiment cohort. Verify `before_value`, `after_value`, `change_percent`, `verdict`, `confidence_level`, and `sample_size` are returned.
- Call `steward.recommendation_impact` on an accepted recommendation with fewer than 3 experiment cohort runs. Verify it returns `verdict: insufficient_data`.
- Call `steward.recommendation_impact` on a rejected recommendation. Verify it returns an error.
- Verify that impact values are persisted on the recommendation after computation.

### 7.6 GraphQL mutations mirror MCP tools

- Call `acceptRecommendation(id, comment)`. Verify same behavior as `steward.accept_recommendation`.
- Call `rejectRecommendation(id, comment)` with a non-empty comment. Verify same behavior.
- Attempt `rejectRecommendation(id, comment: "")` via GraphQL. Verify validation error at the resolver level (GraphQL schema allows non-null empty strings; the resolver must enforce non-empty).
- Call `deferRecommendation(id, deferUntil, comment)`. Verify same behavior.

### 7.7 GraphQL subscription

- Subscribe to `stewardRecommendationChanged`. Accept a recommendation. Verify the subscription emits an event with `reason: ACCEPTED`.
- With an active subscription, complete a run that populates an experiment cohort. Verify the subscription emits `reason: EXPERIMENT_COHORT_UPDATED`.
- Run impact analysis. Verify the subscription emits `reason: IMPACT_AVAILABLE`.

### 7.8 Query sorting and filtering

- Create recommendations with `high`, `medium`, and `low` confidence. Query `stewardRecommendations` without filters. Verify results are sorted `high` first, then `medium`, then `low`.
- Query `stewardRecommendations(status: PROPOSED)`. Verify only proposed recommendations appear.
- Query `stewardRecommendations(analysisId: X)`. Verify only recommendations for that analysis appear.

### 7.9 State transition guards

- Verify that only valid transitions are allowed:
  - `proposed` -> `accepted`, `rejected`, `deferred`
  - `deferred` -> `accepted`, `rejected`
  - `accepted` -> (no further transitions via these tools; `superseded` and `rolledBack` are future V3 actions)
  - `rejected` -> (no further transitions)
- Verify that invalid transitions return a clear error message naming the current and attempted status.

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Experiment cohort auto-population adds latency to `notifyRunCompleted` | Low | The check is a single indexed query per active experiment. The number of active experiments at any time is expected to be small (single digits). |
| Median-based impact analysis is sensitive to outlier runs | Medium | Median is already more robust than mean. The `confidence_level` field signals sample size, and the operator can inspect individual cohort entries via `stewardExperimentCohorts` to identify outliers. |
| Deferred recommendations accumulate without automatic cleanup | Low | Deferred recommendations remain visible in queries and can be filtered with `includeOverdue`. No automatic re-proposal avoids invisible state changes. The operator controls the lifecycle. |
| Reject-requires-comment may slow down operators who want to dismiss quickly | Low | This is intentional. The steward exists to surface real signals. If the operator must explain why a signal is wrong, it creates a feedback record that future steward improvements can learn from. A one-sentence comment is sufficient. |
| Experiment cohort keys may not match any subsequent runs | Medium | If the operator changes workflow family or risk class along with the tuning change, no experiment runs will match. The `steward.recommendation_impact` tool returns `insufficient_data` clearly. The operator can inspect the cohort query to understand why. |
| `RecommendationStatus` naming diverges from Swift app (`accepted` vs `approved`) | Low | The two systems do not share a database. If shared storage is introduced later, a migration mapping `approved` to `accepted` is straightforward. |
| Impact verdict threshold (5%) may be too coarse for some metrics | Low | The 5% threshold is a starting default. A future proposal can make it configurable per metric family in `steward_config.yaml`. |
