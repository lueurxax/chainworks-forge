# Proposal 063: MCP Tool Response Shaping and Field Selection

| Field | Value |
|---|---|
| Date | 2026-04-20 |
| Status | Draft (amended by P068: agent callers must use MCP-only continuation paths; GraphQL is UI-only) |
| Author | Andrey Khasanov |
| Depends on | [reference/mcp-northbound-control-plane-server.md](../reference/mcp-northbound-control-plane-server.md), [031-thin-graphql-ui-rewrite.md](031-thin-graphql-ui-rewrite.md), [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md), [068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md](068-agent-mcp-primary-control-plane-and-graphql-ui-boundary.md) |
| Scope | Make every MCP tool response safe to consume by an LLM agent caller (Claude Code, Claude Desktop, other MCP clients) by establishing a tool-response size budget, trimming oversized fields by default, giving callers explicit field-selection / include-by-opt-in semantics, **and retaining per-round review scores so the inspection tools can return a real score trajectory instead of the last snapshot**. |
| Goal | An agent using the MCP surface for routine inspection (`runs.get`, `reports.get`, `steward.get_analysis`, `reviews.score_trajectory`) receives a response that fits inside its tool-result context budget, with explicit MCP `include` escape hatches for the rare cases where a full blob is genuinely needed. Per P068, GraphQL remains the macOS UI read surface and must not be presented as the agent fallback. |

---

## 1. Context and Motivation

P029 made MCP the northbound command surface and GraphQL the read surface. P031 doubled down: clients read via GraphQL projections, mutate via MCP tools. Both locked in the split.

In practice, callers-who-are-themselves-LLM-agents (Claude Code, Claude Desktop, automation pipelines) also want to **inspect** run/report/steward state via MCP — it is the required surface for agent operation. Today that works for compact tools (`ideas.list`, `runs.list`) but falls over on rich-record tools.

Direct evidence captured on 2026-04-20:

```
mcp__chainworks-control-plane__runs_get(run_id=<any active run>)
  → 66,842 characters in a single tool result
```

That single response exceeds typical LLM tool-result caps (25 k tokens) and forces a fallback. Composition by field size:

| Field | Bytes | Typical use |
|---|---:|---|
| `catalog_snapshot_json` | ~52 000 | Deep agent-catalog frozen snapshot; only needed for full replay / P041 parity. |
| `workflow_snapshot_json` | ~12 000 | Frozen workflow YAML snapshot; only needed for full replay. |
| `delivery_configuration_json` | variable | Delivery preflight blob; only needed when debugging preflight. |
| `delivery_preflight_json` | variable | Same. |
| `active_artifact_index` | variable | Full artifact index including advisory artifacts; usually the caller just wants the current exported path. |
| everything else combined | ~1 000–2 000 | Lifecycle status, stage counts, projection lag, timestamps — the fields the caller actually asked for. |

The **intended** caller use case that motivated this audit was "give me current state + stage counts for every active run", which needs ~100 bytes per run. GraphQL can serve that shape for the macOS UI, but agent callers need the equivalent compact shape through MCP:

```graphql
{ runs { id status totalStages completedStages failedStages } }
```

MCP today forces the caller to pull 66 KB per run. That is the bug. P068 tightens the split: GraphQL is the UI read surface, while agents use MCP for both command and compact inspection. MCP therefore has to respect a response-size budget to stay useful to LLM-shaped callers.

### 1.1 A second, related gap: per-round review scores are not durably retained

The same 2026-04-20 inspection asked a follow-up question: "show the score trajectory across review rounds for the proposals currently in review/refine". Two live runs participated:

- `8dd01a54` (P031 Thin UI Rewrite) — 14 review rounds recorded in the `artifacts` table.
- `4c5dacfa` (P060 Reviewer Routing) — 11 review rounds recorded.

The aggregator writes review artifacts to `.chainworks/runs/<run>/reviews/proposal/`:

```
summary.json                   ← aggregate scores for the round
architect.json                 ← per-reviewer report
ux.json / ui.json / product-owner.json / …
score-lift-backlog.json        ← per-round backlog of issues to lift the score
review-corpus-bundle.json
reviewer-scope-plan.json
feedback-coverage.json
```

Every file is overwritten on the next round. The `artifacts` table records one row per round tagged with `artifactGenerationId`, but each row points at the SAME `file_path`, which has been clobbered by the subsequent round.

What survives today:

- The current round's files on disk.
- A `summaries/orchestrator.md` human-readable snapshot with a "Score Trajectory" table. That snapshot is itself regenerated each round, so it captures only the rounds the last aggregator chose to include. Spot-check: the P031 orchestrator preserves r6 / r7 / r9 and drops r8 + r10–r14. The P060 orchestrator captures pass-3 and drops pass-4–pass-11.
- Nothing queryable over MCP or GraphQL. An operator asking for the trajectory has to read stale markdown files by hand.

This is the same class of problem as the oversized `runs.get` response: the data the operator cares about (score per round) is reachable only via heroics, while the data they don't need (per-reviewer report bodies) is what gets overwritten. Fixing only the shaping side would leave operators with the same "read a markdown snapshot" workaround for score trajectories.

This proposal does not propose that MCP mirrors GraphQL's schema or pagination model. It proposes a small, consistent set of rules so MCP tools that return a record-like JSON (a) ship a compact default envelope, (b) explicitly opt in to heavy fields, (c) document their default budget, **and (d) retain per-round review scores in a durable place a compact MCP tool can read**.

---

## 2. Product Questions This Proposal Must Answer

1. Can an LLM agent call `runs.get` and `reports.get` on an arbitrary production run and receive a response that fits inside a 25 k-token tool-result context without fallback?
2. Is there a predictable, typed way to request the heavy fields when they genuinely are needed (replay, debugging, audit)?
3. Do we have an enforced default size budget per MCP tool response, and a test that fails the gate when a single compact response exceeds it?
4. Does the response shape degrade gracefully when a field is trimmed — does the caller see `truncated_fields: [...]` with a clear MCP `include` continuation path or artifact/resource URI that carries the full blob?
5. Does this proposal preserve the P068 split? MCP stays the agent command + compact-inspection surface; GraphQL stays the macOS UI read surface.
6. Is the change backward-compatible for existing MCP clients, or does it need a versioned tool rename?
7. Can an operator (or an agent) ask "show me the score trajectory across review rounds for run X" and get a structured, per-round answer — including reviewer scores, aggregate score, blocker count, and decision — without relying on a stale `orchestrator.md` snapshot or reading overwritten JSON files?
8. Does the retention scheme survive proposal revisions to the review contract (e.g. new reviewer classes, new rubric weighting) without requiring retroactive backfill?

---

## 3. Scope

### In scope

- A default response-size budget for MCP **read-shaped** tools (`runs.get`, `reports.get`, `steward.get_analysis`, and any future equivalents). Target: ≤ 8 KB default envelope.
- A per-tool list of fields that are **omitted by default** (the "heavy fields" table below) and are retrievable only via explicit `include` opt-in.
- A typed `include: [field_name, …]` argument on every affected tool. Unknown field names produce a typed error rather than silent pass-through.
- A `truncated_fields` metadata block that lists the fields that were available on the server but omitted from this response, each annotated with an MCP `include` continuation path or resource/artifact URI the agent caller can use for the full value. A UI-only GraphQL hint may be documented separately, but must not be the agent continuation path.
- A per-round **review-score retention scheme** (§ 4.7): on-disk archive under `.chainworks/runs/<run>/reviews/proposal/history/round-NNN/`, plus a new indexed SQLite table `review_score_trajectories` that captures the compact score fields per round without the heavy report bodies.
- A new MCP tool **`reviews.score_trajectory`** (§ 4.7) and a UI-only GraphQL mirror `run(id).reviewScoreTrajectory(...)`, both serving the compact trajectory inside the same 8 KB budget.
- A compact `latest_review_round` field added to `runs.get` default response so one inspection call answers "where is this run in its review loop".
- A migration script that backfills `review_score_trajectories` with whatever rounds are reconstructible from the current `summary.json` + `orchestrator.md` snapshots; missing rounds stay absent and are documented as such.
- A focused gate test `proposal-063-mcp-response-shape` that asserts: (a) compact default responses fit the budget for every covered tool against a fixture run, (b) `include` opt-in restores the named fields, (c) unknown `include` names return a typed error envelope, (d) the review aggregator writes per-round archives + trajectory rows, (e) `reviews.score_trajectory` returns consistent data for a multi-round fixture run.
- Reference-documentation update in `docs/reference/mcp-northbound-control-plane-server.md` describing the shaping contract and the trajectory tool.

### Out of scope (future proposals)

- MCP pagination for list-shaped tools (`runs.list`, `ideas.list`). Their current response is already compact; if it ever gets heavy, a separate proposal adds `limit` / `after` cursors.
- Streaming / chunked MCP responses. Streamable HTTP transport already supports it, but no current tool needs it.
- GraphQL schema changes other than the `reviewScoreTrajectory` field — GraphQL already has field selection and is fine.
- MCP tool-level authorization changes — P029's class-policy stays.
- Trimming **most** command-tool responses (`runs.cancel`, `approvals.resolve`, `stages.retry`, `ideas.create`, etc.) — they are already compact (journal_id + minor fields). The one command tool that IS in scope for shaping is `runs.start`: on success it returns the full newly-created run record, so it behaves like `runs.get` for response-size purposes and gets the same heavy-fields pipeline (see § 4.2.1).
- Retention for non-review per-round artifacts (proposal text revisions, steward analysis generations). Those already live under versioned paths or are intentionally content-addressable; if they regress, a separate proposal handles them.
- Retroactive backfill of rounds where no snapshot survives. The migration script captures what is reconstructible; older rounds remain permanently gone.

---

## 4. Design

### 4.1 Response-size budget

Every MCP tool MUST aim for a default response of **≤ 8 KB** (8 192 bytes). Tools whose natural response exceeds that MUST trim by default (heavy-fields policy below) and MUST document the budget in their `tools/list` schema description.

The 8 KB target is empirically chosen: fits in ~2 k tokens, which is well under every known LLM tool-result cap, and leaves room for multi-run batching by the caller.

### 4.2 Heavy fields

For each tool that returns a record containing a heavy field, the tool's implementation omits that field from the default response and lists it under `truncated_fields`. Fields are added to the heavy list when they exceed **1 KB typical size** or are semantically "audit / replay / frozen-snapshot" blobs.

Initial heavy-field assignments:

| Tool | Heavy fields (omitted by default) | Where the full value lives |
|---|---|---|
| `runs.get` | `catalog_snapshot_json`, `workflow_snapshot_json`, `delivery_configuration_json`, `delivery_preflight_json`, `catalog_snapshot_hash`, `operator_overrides`, `cancellation_settlement_log`, `active_artifact_index` (except `exported_path`, `owner`, `run_id`, `schema_version`) | MCP `include=[...]`; artifact/resource URIs under `chainworks_meta_root`; UI-only GraphQL can read the same fields for app screens |
| `runs.start` | Same heavy-field set as `runs.get` plus `operator_overrides_json`, `active_artifact_index`, `drift_details_json`. The compact default returns only the fields a caller needs to confirm the run was created: `id`, `idea_id`, `status`, `current_state`, `started_at`, `target_branch`, `base_revision`, `workflow_id`, `workflow_title`, plus `blocked` + `reason` + `delivery_preflight` (small preflight summary) when the call was blocked by preflight. | MCP `include=[...]`; the delivery-preflight JSON is served compact by default and retrievable in full via `include=["delivery_preflight_json"]` when debugging a preflight block. |
| `reports.get` | `agent_executions` (full execution records), `run_state_projection` blobs, any embedded artifact JSON > 1 KB | MCP `include=[...]`; artifact files/resources referenced in the compact summary; UI-only GraphQL can mirror for app screens |
| `steward.get_analysis` | `input_snapshot_json`, `analysis_report_json` full blob | MCP `include=[...]`; UI-only GraphQL can mirror for app screens |

Fields NOT on the list stay in the default response unconditionally.

### 4.2.1 `runs.start` is a command tool AND a record-returning tool

Command tools normally keep their response compact (`journal_id` + minor fields; see § 3 "Trimming command-tool responses" under out-of-scope). `runs.start` is the exception: on success it returns the full run record, which puts it in the same 66 KB+ trap as `runs.get`. Evidence captured 2026-04-20: a successful `runs.start` for a newly-created idea returned **68 792 bytes** — larger than `runs.get` because it also carries the freshly-frozen `catalog_snapshot_json` + `workflow_snapshot_json`.

The fix therefore applies the same shaping pipeline. Two concrete shapes:

**Success (run created):**

```json
{
  "id": "b57f18ef-...",
  "idea_id": "e9ff4d23-...",
  "status": "pending",
  "current_state": "state_1_idea_received",
  "started_at": "2026-04-20T19:42:19Z",
  "target_branch": "feature/p061-...",
  "base_revision": "da659a2d...",
  "workflow_id": "full-mvp-live",
  "workflow_title": "Full MVP Live",
  "journal_id": "b85dfcf7-...",
  "truncated_fields": {
    "catalog_snapshot_json": { "size_bytes": 54000, "mcp_hint": "runs.get(run_id, include=[\"catalog_snapshot_json\"])", "ui_graphql_hint": "UI-only full run field" },
    "workflow_snapshot_json": { "size_bytes": 13000, "mcp_hint": "runs.get(run_id, include=[\"workflow_snapshot_json\"])", "ui_graphql_hint": "UI-only full run field" },
    "delivery_configuration_json": { "size_bytes": 280, "mcp_hint": "runs.get(run_id, include=[\"delivery_configuration_json\"])", "ui_graphql_hint": "UI-only full run field" },
    "delivery_preflight_json": { "size_bytes": 450, "mcp_hint": "runs.get(run_id, include=[\"delivery_preflight_json\"])", "ui_graphql_hint": "UI-only full run field" }
  }
}
```

**Blocked by preflight:**

```json
{
  "blocked": true,
  "journal_id": "e1946446-...",
  "reason": "delivery_preflight_failed",
  "delivery_preflight": {
    "passed": false,
    "timestamp": "2026-04-20T19:41:55Z",
    "checks": [
      { "id": "delivery_configuration_present", "label": "Delivery configuration is present", "detail": "release workflows require frozen delivery_configuration_json", "passed": false }
    ]
  }
}
```

The blocked envelope intentionally INCLUDES the preflight summary (small — ≤ 1 KB per check, ≤ 8 KB in total) so the caller can fix and retry without a follow-up `include=`. Only the full `delivery_preflight_json` (raw server-side blob, can include snapshot metadata) stays behind `include`.

### 4.3 `include` opt-in

Every affected tool gains an optional `include: [string]` argument. Calling `runs.get(run_id, include=["catalog_snapshot_json"])` returns the default envelope **plus** the listed fields.

Validation rules:

- Each name in `include` MUST appear in that tool's heavy-fields table. Unknown names produce a JSON-RPC `-32602` (invalid params) with `data.allowed_fields: [...]`.
- Passing `include=["*"]` is a shorthand for "all heavy fields" and is documented per-tool.
- `include` cannot be used to request fields outside the heavy list (those are always present).

### 4.4 `truncated_fields` metadata

Compact default responses include a top-level `truncated_fields` object:

```json
{
  "id": "4b3a582a-…",
  "status": "running",
  "current_state": "state_7_implementation_started",
  "total_stages": 28,
  "completed_stages": 19,
  "failed_stages": 0,
  "pending_approvals": 0,
  "…": "… other compact fields …",
  "truncated_fields": {
    "catalog_snapshot_json": {
      "size_bytes": 52451,
      "mcp_hint": "runs.get(run_id, include=[\"catalog_snapshot_json\"])",
      "resource_hint": "run://4b3a582a-…"
    },
    "workflow_snapshot_json": {
      "size_bytes": 12416,
      "mcp_hint": "runs.get(run_id, include=[\"workflow_snapshot_json\"])",
      "resource_hint": "run://4b3a582a-…"
    }
  }
}
```

`size_bytes` is the bytes the caller would have received if the field had been included. `mcp_hint` is the binding continuation path for agent callers. `resource_hint` points to an MCP resource when the full value is better retrieved as a resource. UI-only GraphQL hints may exist in implementation docs, but they must not replace the MCP continuation path.

### 4.5 Tool-schema documentation

Each affected tool's description in `tools/list` gains:

- A short "Default response ≤ 8 KB" line.
- The heavy-fields table for that tool.
- A reminder that MCP `include` or MCP resources are the canonical full-field continuation path for agent callers. GraphQL examples, if present, must be labeled UI-only.

### 4.6 Error shape on validation failure

If `include` contains an unknown or non-heavy name, the tool returns a JSON-RPC error envelope (status 200 with `error.code = -32602`) containing:

```json
{
  "error": {
    "code": -32602,
    "message": "unknown include field",
    "data": {
      "unknown_fields": ["catalog_snapshot_jsno"],
      "allowed_fields": ["catalog_snapshot_json", "workflow_snapshot_json", "…"]
    }
  }
}
```

No partial success; the whole call fails closed.

### 4.7 Per-round review scores: durable retention and a dedicated MCP tool

#### 4.7.1 On-disk history archive

The review aggregator currently writes to
`.chainworks/runs/<run>/reviews/proposal/` and overwrites each round. The new contract adds a **round-scoped archive sub-directory** alongside the live files:

```
.chainworks/runs/<run>/reviews/proposal/
  summary.json                       ← current round (unchanged: live pointer)
  architect.json                     ← current round (unchanged)
  ux.json / ui.json / product-owner.json / …
  score-lift-backlog.json
  history/
    round-001/
      summary.json                   ← copy of round 1's summary
      architect.json
      ux.json
      ...
      score-lift-backlog.json
      review-corpus-bundle.json
    round-002/
      ...
    round-NNN/
      ...
```

On every aggregator-run completion, the writer:

1. Writes the live `summary.json` + siblings as today (no behavior change for existing consumers).
2. Additionally copies the same bytes into `history/round-NNN/` where `NNN = round_ordinal` zero-padded.
3. Emits one row to the new `review_score_trajectories` SQLite table (see § 4.7.2) with the compact score fields AND a pointer to `history/round-NNN/`.

Zero-padded directory names keep `ls` + lexical sort aligned with round order. The archive subtree is append-only from the aggregator's point of view.

Retention: unlimited for now. A follow-up proposal may add a rolling window (e.g. "keep last 20 rounds + every 10th earlier round") once we observe real disk usage. Every round is ≤ 100 KB today; 50 rounds ≈ 5 MB per run — negligible.

#### 4.7.2 Indexed trajectory table

New SQLite table owned by a new migration under `control-plane/crates/db/migrations/`:

```sql
CREATE TABLE review_score_trajectories (
    run_id TEXT NOT NULL REFERENCES runs(id),
    stage_id TEXT NOT NULL,
    review_pass_id TEXT NOT NULL,
    round_ordinal INTEGER NOT NULL,
    completed_at TEXT NOT NULL,
    average_score REAL,
    aggregate_score REAL,
    min_individual_score REAL,
    blocker_count INTEGER NOT NULL DEFAULT 0,
    decision TEXT NOT NULL,                -- revise | approve | approve_with_conditions | ...
    reviewer_scores_json TEXT NOT NULL,    -- {"ux":9.5,"ui":9.0,"architect":8.4,"product_owner":8.0}
    summary_archive_path TEXT NOT NULL,    -- relative path to history/round-NNN/
    PRIMARY KEY (run_id, stage_id, round_ordinal)
);
CREATE INDEX idx_review_trajectories_run ON review_score_trajectories (run_id, completed_at DESC);
```

Numeric score columns are `REAL NOT NULL`able so a failed round (no aggregate computable) is representable. `reviewer_scores_json` is a compact JSON object keyed by reviewer-class slug.

#### 4.7.3 New MCP tool `reviews.score_trajectory`

- **Input:**
  - `run_id` (required)
  - `stage_id` (optional; defaults to the latest stage that has trajectory rows for this run)
  - `limit` (optional; default 20, max 50) — return the N most recent rounds newest-first
  - `include: [string]` (optional) — same opt-in shape as § 4.3, for heavy per-round fields
- **Output shape (compact default):**

```json
{
  "run_id": "8dd01a54-...",
  "stage_id": "state_4_proposal_reviewed",
  "review_pass_id": "rp-031-001",
  "rounds": [
    {
      "round_ordinal": 14,
      "completed_at": "2026-04-20T11:23:10Z",
      "average_score": 8.125,
      "aggregate_score": 32.5,
      "min_individual_score": 8.0,
      "blocker_count": 1,
      "decision": "revise",
      "reviewer_scores": {"ux": 9.5, "ui": 9.0, "architect": 8.4, "product_owner": 8.0},
      "summary_archive_path": ".chainworks/runs/8dd01a54-.../reviews/proposal/history/round-014/"
    },
    { "round_ordinal": 13, ... },
    ...
  ],
  "truncated_fields": {
    "per_round_reviewer_reports": {
      "size_bytes": 348291,
      "mcp_hint": "reviews.score_trajectory(run_id, include=[\"per_round_reviewer_reports\"])",
      "ui_graphql_hint": "UI-only reviewScoreTrajectory.fullReviewerReports"
    },
    "per_round_score_lift_backlog": {
      "size_bytes": 58210,
      "mcp_hint": "reviews.score_trajectory(run_id, include=[\"per_round_score_lift_backlog\"])",
      "ui_graphql_hint": "UI-only reviewScoreTrajectory.scoreLiftBacklog"
    }
  }
}
```

Default fields per round: `round_ordinal`, `completed_at`, `average_score`, `aggregate_score`, `min_individual_score`, `blocker_count`, `decision`, `reviewer_scores`, `summary_archive_path`.

Heavy fields (available only via `include`): `per_round_reviewer_reports`, `per_round_score_lift_backlog`, `per_round_feedback_coverage`, `per_round_review_corpus_bundle`. Each of these is the concatenation of all archived rounds' corresponding files; `include` returns them as an object keyed by `round_ordinal`.

#### 4.7.4 GraphQL mirror

Same shape exposed under `GqlRun.reviewScoreTrajectory(stageId: String, limit: Int)`. Returns a typed `GqlReviewScoreTrajectory` with a `rounds: [GqlReviewRound!]!` collection. This gives the Swift UI (P031 thin client) a first-class "score lift" widget without a second MCP round-trip.

#### 4.7.5 Compact `latest_review_round` on `runs.get`

To answer the common agent question ("where is this run in its review loop?") in one call, `runs.get`'s compact default envelope gains a new nested field:

```json
{
  "id": "8dd01a54-...",
  "status": "running",
  "current_state": "state_5_proposal_refined",
  "...": "... existing compact fields ...",
  "latest_review_round": {
    "round_ordinal": 14,
    "decision": "revise",
    "average_score": 8.125,
    "blocker_count": 1,
    "completed_at": "2026-04-20T11:23:10Z"
  }
}
```

`null` when the run's stage has no review_score_trajectories row yet. Size impact: ≤ 200 bytes — comfortable inside the § 4.1 budget.

#### 4.7.6 Migration: reconstruct what is reconstructible

A one-shot migration script runs on first daemon startup after this proposal lands:

1. Scan every `.chainworks/runs/<run>/reviews/proposal/summary.json` that exists. Insert one trajectory row at `round_ordinal = <count of artifact rows with name='proposal_review_summary' for this run>` (the current/live round).
2. For each run that has a `summaries/orchestrator.md`, parse the `## Score Trajectory` table and backfill one row per listed round. Absent rounds stay absent.
3. Copy the current live files into `history/round-NNN/` so the archive scheme is populated for the live round going forward.

Rounds that are neither live nor mentioned in any orchestrator snapshot are permanently gone and will not appear in the trajectory. The tool schema documents this: consumers querying historical runs may see gaps with an explanatory `note` field (`"missing_rounds": [<ordinals>]`).

#### 4.7.7 Aggregator contract change

The review aggregator (currently an agent that writes `summary.json` etc.) MUST, as the last step of each run, call a new internal `engine::reviews::record_round` API that:

1. Reads the freshly-written live files.
2. Copies them into `history/round-NNN/`.
3. Inserts a `review_score_trajectories` row in the same transaction as the `artifacts` table rows for that round.

Failure mode: if the copy OR the insert fails, the aggregator's stage exits with a typed error and the run's state transition is blocked until the round is either persisted or explicitly skipped by an operator. No silent "round was aggregated but not archived" state.

---

## 5. Migration and Backward Compatibility

Existing callers who request `runs.get` without `include` TODAY receive the full blob. After this proposal:

- They receive the **compact envelope** plus `truncated_fields`. Every compact field they were using is still present.
- If an agent caller specifically relies on one of the heavy fields, it adds the field to MCP `include` or follows the MCP resource hint. It does not migrate to GraphQL.

Detection plan for existing usage:

1. Before land: grep `command_journal` for tool-call rows over the last 30 days. Enumerate which callers request `runs.get` and look at whether downstream code touches the heavy fields. Document migration steps for each real consumer.
2. During rollout: emit a WARN log entry when `runs.get` is called without `include` AND the caller's principal has accessed a heavy field in recent history (reasonable proxy: check if the same call id has subsequently called `resources/read` on an artifact derived from those blobs).
3. Post-rollout: no deprecation window needed. The fields are still reachable via `include`; the default just changed.

This is a behavior change, not a rename. No new tool names. The affected tools keep their existing names (`runs.get`, `reports.get`, `steward.get_analysis`).

---

## 6. Implementation Inventory

### 6.1 Response shaping (§ 4.1 – § 4.6)

1. **Rust**: `crates/mcp-server/src/tools/runs.rs` — add `include: Option<Vec<String>>` arg to **both** `runs.get` AND `runs.start` (same heavy-field pipeline), and wire through the response builder that already exists for `runs.get` to omit heavy fields by default. The `runs.start` success-shape + blocked-shape live in § 4.2.1.
2. **Rust**: `crates/mcp-server/src/tools/reports.rs` — same pattern.
3. **Rust**: `crates/mcp-server/src/tools/steward.rs` — same pattern.
4. **Rust**: new `crates/mcp-server/src/shaping.rs` helper with `TruncatedFieldHint` struct + a `shape_response(default, include)` function reusable across tools.
5. **Rust**: update `tools/list` schema descriptions to document the budget and heavy-field tables.

### 6.2 Review-score retention and trajectory tool (§ 4.7)

6. **DB migration**: new `control-plane/crates/db/migrations/NNN_review_score_trajectories.sql` creating the table and index described in § 4.7.2. Migration version picked at land-time to avoid clashing with concurrent proposals.
7. **Rust**: new module `crates/db/src/repos/review_score_trajectories.rs` with `insert`, `list_for_run`, `list_for_stage` repo functions.
8. **Rust**: new `crates/engine/src/reviews.rs` with a `record_round(pool, run_id, stage_id, live_dir, round_ordinal, summary_json, sibling_files) -> Result<()>` function that performs the copy-to-`history/round-NNN/` step and inserts the trajectory row in a single SQLx transaction.
9. **Rust**: new MCP tool `reviews.score_trajectory` wired into `crates/mcp-server/src/tools/reviews.rs`. Uses the shared `shape_response` helper from § 6.1 item 4 so the 8 KB budget and `include` contract are reused.
10. **Rust**: `crates/graphql-server/src/schema.rs` — add `GqlRun.reviewScoreTrajectory` field + `GqlReviewScoreTrajectory` + `GqlReviewRound` types.
11. **Rust**: `runs.get` compact-response builder gains a `latest_review_round: Option<…>` field populated by a single `SELECT … ORDER BY round_ordinal DESC LIMIT 1` query.
12. **Rust**: `crates/engine/src/reviews.rs::migrate_existing_runs(pool)` called once on daemon startup (idempotent — checks for presence of trajectory rows before touching runs). Parses any `summaries/orchestrator.md` on disk for the "Score Trajectory" table and backfills reconstructible rounds.
13. **Aggregator prompt/contract**: the review-aggregator agent's output contract (wherever the prompt lives under `examples/agents/` or the catalog) gains a clarifying note that `record_round` is the persistence source of truth. No behavior change required for the agent author; the daemon owns the copy + row insert.

### 6.3 Tests

14. **Tests**: new `crates/mcp-server/tests/proposal_063_shaping.rs` integration test:
    - Seeds a run with populated catalog/workflow snapshots.
    - Calls `runs.get` without `include`, asserts response bytes ≤ 8 192.
    - Calls `runs.get` with `include=["catalog_snapshot_json"]`, asserts `catalog_snapshot_json` is present and compact envelope still present.
    - Calls `runs.get` with `include=["catalog_snapshot_jsno"]`, asserts `-32602` typed error with `data.unknown_fields` and `data.allowed_fields`.
    - Same three cases for `reports.get` and `steward.get_analysis`.
    - **`runs.start` cases:** (a) a successful start returns the compact-success envelope ≤ 8 192 bytes with `id`, `idea_id`, `status`, `current_state`, `journal_id`, and `truncated_fields` populated for `catalog_snapshot_json` + `workflow_snapshot_json`; (b) a preflight-blocked start returns the compact blocked envelope with `blocked=true`, `reason`, and the full `delivery_preflight` checks summary inline; (c) `runs.start(..., include=["catalog_snapshot_json"])` on a success returns the snapshot inline with byte equality to what was frozen into the DB.
15. **Tests**: new `crates/mcp-server/tests/proposal_063_trajectory.rs`:
    - Seeds a fixture run with 5 review rounds via `engine::reviews::record_round`.
    - Calls `reviews.score_trajectory` with default args, asserts response bytes ≤ 8 192 and the 5 rounds are present newest-first with populated compact fields.
    - Calls `reviews.score_trajectory` with `include=["per_round_reviewer_reports"]`, asserts each round carries its full per-reviewer reports and total response is still structurally valid.
    - Calls with unknown `include`, asserts `-32602` typed error.
16. **Tests**: new `crates/engine/tests/proposal_063_review_round_persistence.rs`:
    - Calls `record_round` with a staged `live_dir`, asserts `history/round-001/` exists with a full copy AND a `review_score_trajectories` row with the expected compact fields.
    - Simulates a mid-write crash (close pool before commit), asserts the archive-copy-and-row-insert transaction is atomic: either both present or both absent.
17. **Tests**: new `crates/engine/tests/proposal_063_migration_backfill.rs`:
    - Seeds a `.chainworks/runs/<run>/summaries/orchestrator.md` with a mocked Score Trajectory table listing 3 rounds.
    - Runs `migrate_existing_runs`, asserts 3 trajectory rows created with the parsed fields, no history/ directories created for those rounds (content was already lost), `missing_rounds` exposed in subsequent `reviews.score_trajectory` calls.

### 6.4 Gate + docs

18. **Gate**: register `proposal-063-mcp-response-shape` in `scripts/test-gate.sh`. Runs the four integration tests above plus a workspace regression. Dedicated `CARGO_TARGET_DIR=target/p063-gate`.
19. **Reference doc**: add a "Response shaping" section and a "Review-score retention" section to `docs/reference/mcp-northbound-control-plane-server.md` describing the budget, heavy-field tables, the `include` / `truncated_fields` contract, the `reviews.score_trajectory` tool, and the `history/round-NNN/` on-disk layout.

---

## 7. Acceptance Criteria

### Shaping

1. **Budget** — on a populated fixture run whose full `runs.get` response exceeds 50 KB, the default `runs.get` response serializes to ≤ 8 192 bytes. Same for `runs.start` (success AND blocked shapes), `reports.get`, `steward.get_analysis`, and `reviews.score_trajectory`.
2. **Field completeness** — the compact default envelope preserves every field currently used by the `full-mvp-live` Swift client's `RunDetailView` GraphQL query. Regression test that the set of non-truncated fields is stable.
3. **Include round-trip** — requesting a heavy field via `include` returns a response that contains that heavy field with byte-for-byte equality to the value that was stored in the DB (for shaped `*.get` tools) or the on-disk archive (for `reviews.score_trajectory`).
4. **Error shape** — unknown `include` names produce a typed JSON-RPC `-32602` with populated `data.allowed_fields`. No silent pass-through, no partial success.
5. **Truncated-fields hints** — every `truncated_fields` entry carries `size_bytes` and an MCP continuation path (`mcp_hint` or `resource_hint`). UI-only GraphQL hints are optional and must not be used as the agent continuation path.

### Review-score retention

6. **Durable archive** — every round that completes via `engine::reviews::record_round` produces `.chainworks/runs/<run>/reviews/proposal/history/round-NNN/` containing byte-identical copies of `summary.json`, the per-reviewer files, `score-lift-backlog.json`, `review-corpus-bundle.json`, and `feedback-coverage.json`.
7. **Trajectory table** — every completed round inserts exactly one row into `review_score_trajectories` with non-null `decision`, `reviewer_scores_json`, and `summary_archive_path`. The row is inserted in the same transaction as the archive copy — crash mid-flight leaves neither artifact nor row.
8. **Trajectory readback** — `reviews.score_trajectory(run_id)` returns every round for that run newest-first, with compact fields matching the DB values, within the 8 KB default budget for runs with up to ~50 rounds.
9. **`runs.get` integration** — the compact `runs.get` default envelope includes `latest_review_round` populated from the most recent trajectory row (or `null` when no rounds exist yet). Size impact ≤ 200 bytes per response.
10. **Migration** — on first startup after this proposal lands, `migrate_existing_runs` runs exactly once per run, backfills trajectory rows from `orchestrator.md` snapshots where available, records rounds that are permanently gone as `missing_rounds: [ordinals]` in subsequent `reviews.score_trajectory` output, and is idempotent across daemon restarts.

### Compatibility

11. **No caller breakage** — `scripts/test-gate.sh proposal-042`, `proposal-031`, and `proposal-041` all continue to pass. Specifically, Swift tests that call `runs.get` continue green without modification.
12. **Aggregator contract** — the review aggregator still writes its live `summary.json` + siblings as before; the archive + trajectory-row step is daemon-owned and transparent to the agent.

### Docs + gate

13. **Docs aligned** — `docs/reference/mcp-northbound-control-plane-server.md` contains the new "Response shaping" section with the heavy-field tables AND a "Review-score retention" section describing `history/round-NNN/`, the trajectory table, and the `reviews.score_trajectory` tool. `tools/list` descriptions include the default-budget line for every affected tool.
14. **Gate registered** — `./scripts/test-gate.sh proposal-063-mcp-response-shape` is registered, executable on the gate list, and green on the implementation tree.

---

## 8. Canonical Proof Gate

`scripts/test-gate.sh proposal-063-mcp-response-shape` runs:

- `cargo test -p mcp-server --test proposal_063_shaping` — response-shaping focused suite (§ 6.3 item 14).
- `cargo test -p mcp-server --test proposal_063_trajectory` — trajectory tool focused suite (§ 6.3 item 15).
- `cargo test -p engine --test proposal_063_review_round_persistence` — archive + transaction atomicity suite (§ 6.3 item 16).
- `cargo test -p engine --test proposal_063_migration_backfill` — migration idempotency + `missing_rounds` semantics (§ 6.3 item 17).
- `cargo test --workspace` — workspace regression.
- Returns exit 0 only on full green.

Dedicated `CARGO_TARGET_DIR=target/p063-gate` per the pattern P042 introduced, so parallel agent activity on the shared target cannot starve the gate.

---

## 9. Out-of-Scope Deferred Follow-Ups

- **MCP list pagination.** If `runs.list` or `ideas.list` ever grow past the budget, a separate proposal adds `limit` / `after` cursors. Not needed today; the current list responses are compact.
- **Streaming tool responses.** Streamable HTTP transport could chunk a heavy blob when a caller explicitly asks for `include=["catalog_snapshot_json"]`. Not needed today; 52 KB fits in a single HTTP response.
- **Authorization-level trimming.** No principal class currently needs different field visibility beyond what P029's existing capability filtering already provides. If that changes, the shaping layer has the right seam to add it.

---

## 10. Success Signal

Two LLM-agent workflows that triggered this proposal complete end-to-end entirely over MCP:

**Workflow A — quick active-run inspection:**

```
"Give me the current state of every active run"
→ call runs.list → for each, call runs.get
→ agent summarizes state in a single response, no fallback to GraphQL, no tool-result overflow
```

The compact default envelope fits the agent's context, and the heavy-field escape hatch is documented and tested for the rare cases where full blobs are genuinely needed. The new `latest_review_round` field in the envelope means the agent also learns the current review verdict for every run in the same call.

**Workflow B — score trajectory across review rounds:**

```
"Show me the score progression across review rounds for every proposal
 currently in the review/refine loop"
→ call runs.list filtered by current_state matching proposal_reviewed/proposal_refined
→ for each, call reviews.score_trajectory(run_id)
→ agent renders a per-run table (round, avg, min, blockers, decision, per-reviewer scores)
  without reading any on-disk markdown and without losing rounds that prior aggregators
  had already dropped from the orchestrator snapshot
```

Today this workflow requires reading overwritten `summary.json` files and a stale `orchestrator.md` snapshot that preserves only an arbitrary subset of rounds. With P063 it becomes a structured query against the `review_score_trajectories` table served compactly over MCP.
