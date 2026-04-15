# Proposal 049: Steward Analysis System — Deterministic Quality Observatory

| Field | Value |
|---|---|
| Date | 2026-04-14 |
| Status | Draft |
| Author | Claude |
| Depends on | [048-evidence-packs-delivery-preflight-and-mcp-resolution.md](048-evidence-packs-delivery-preflight-and-mcp-resolution.md) |
| Scope | Port the complete Steward V1 analysis pipeline to the Rust daemon: metrics collection (all five families with full signal parity), anomaly detection, cohort classification, evidence dossiers with provenance hashes, both optional LLM lanes (`system_steward` + `steward_auditor`), and config-change detection covering both steward config and agent catalog snapshot hashes. |
| Goal | The Rust daemon autonomously monitors workflow health across completed runs, detects degradations in timing/rework/quality/cost/stability dimensions, and surfaces actionable recommendations — matching the full stable Swift V1 contract including audit lane, config provenance, and evidence completeness. |

---

## 1. Context and Motivation

The Steward is the factory's quality conscience. It answers: "Are runs getting slower? More expensive? Failing more? Requiring more rework loops?" These questions cannot be answered by looking at a single run — they require **cross-run statistical comparison**.

Swift implements this as a complete 11-step deterministic pipeline. The Rust daemon has **zero Steward functionality**. Without it:

- Gradual degradation (e.g. review scores creeping down over 10 runs) goes undetected
- Cost regressions from backend profile changes are invisible
- Timing bloat from additional review iterations has no alerting
- Config changes (catalog/workflow YAML edits) have no before/after comparison

The Steward is **not** an LLM feature. Its core is pure arithmetic: medians, means, ratios, and threshold comparisons. Two optional LLM agents provide interpretation:
- **`system_steward`** — produces `health-report.json` (narrative health summary)
- **`steward_auditor`** — produces `audit-report.json` (challenges the analysis, looks for blind spots)

Both are optional (analysis completes without them), but both must be wired when present in the catalog.

---

## 2. Architecture Overview

### Pipeline (11 steps, matching Swift `StewardAnalysisService.runAnalysis()`)

```
1. Validate steward_config.yaml (full schema: windows, thresholds, triggers, context_strategy_profiles)
2. Query completed runs from DB
3. Select primary cohort by (workflow_family, risk_class)
4. Split into observation window (recent N) and baseline window (next N)
5. Classify cohort quality (strong / acceptable / weak)
6. Collect deterministic metrics from both windows (all 5 families, full signal set)
7. Detect anomalies: compare observation vs baseline against thresholds
8. Build evidence dossiers for implicated runs (with provenance hashes)
9. Write JSON artifacts to disk
10. [Optional] Execute system_steward LLM agent → health-report.json
    [Optional] Execute steward_auditor LLM agent → audit-report.json
11. Persist analysis record + recommendations + run links
```

### Determinism Guarantee

Steps 1–9 are **pure functions** over DB data. No randomness, no network, no LLM. Same input → same output. Step 10 is optional and clearly separated.

---

## 3. Data Model

### 3a. DB Schema (new tables)

```sql
-- Steward analysis record
CREATE TABLE steward_analyses (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    window_start TEXT NOT NULL,
    window_end TEXT NOT NULL,
    run_count INTEGER NOT NULL,
    cohort_keys_json TEXT,                -- {"workflow_family":"...", "risk_class":"..."}
    cohort_quality TEXT NOT NULL,          -- "strong" | "acceptable" | "weak"
    metrics_snapshot_path TEXT,
    baseline_snapshot_path TEXT,
    degradations_detected INTEGER NOT NULL DEFAULT 0,
    report_artifact_path TEXT,            -- health-report.json (system_steward)
    audit_artifact_path TEXT,             -- audit-report.json (steward_auditor)
    status TEXT NOT NULL DEFAULT 'completed',  -- "completed" | "inconclusive" | "superseded"
    workflow_catalog_snapshot_hash TEXT,   -- canonical DefinitionHasher-style hash of parsed AgentCatalogFile at analysis time
    steward_config_snapshot_hash TEXT      -- canonical DefinitionHasher-style hash of parsed StewardConfig at analysis time
);

-- Join table: analysis ↔ run with role
CREATE TABLE steward_analysis_run_links (
    id TEXT PRIMARY KEY,
    analysis_id TEXT NOT NULL REFERENCES steward_analyses(id),
    run_id TEXT NOT NULL REFERENCES runs(id),
    role TEXT NOT NULL  -- "implicated" | "baseline" | "context"
);

-- Actionable recommendations
CREATE TABLE steward_recommendations (
    id TEXT PRIMARY KEY,
    analysis_id TEXT NOT NULL REFERENCES steward_analyses(id),
    created_at TEXT NOT NULL,
    category TEXT NOT NULL,     -- "agent_tuning" | "workflow_tuning" | "backend_change" | "input_contract_change"
    summary TEXT NOT NULL,
    target_metric TEXT NOT NULL,
    confidence_level TEXT NOT NULL,  -- "high" | "medium" | "low"
    status TEXT NOT NULL DEFAULT 'proposed',
    decision_comment TEXT,
    decided_at TEXT
);
```

### 3b. Rust Domain Structs — Full Signal Parity

```rust
// domain/src/steward.rs

pub struct MetricsSnapshot {
    pub run_count: usize,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,

    // ── Timing family ──
    pub lead_time_median_seconds: Option<f64>,
    pub stage_latency_medians: HashMap<String, f64>,    // per-stageID median durations
    pub approval_wait_median_seconds: Option<f64>,

    // ── Rework family ──
    pub proposal_loop_mean: f64,
    pub implementation_loop_mean: f64,
    pub retries_per_stage_mean: HashMap<String, f64>,   // per-stageID retry averages

    // ── Quality family ──
    pub approval_rejection_rate: f64,
    pub audit_pass_rate: f64,                           // completed audit stages / total

    // ── Cost family ──
    pub cost_per_run_median_cents: Option<i64>,
    pub cost_by_stage_family: HashMap<String, i64>,     // cost aggregated by stageID

    // ── Stability family ──
    pub failed_run_rate: f64,
    pub blocked_run_rate: f64,
    pub drift_event_count: usize,                       // runs with drift detected
    pub resumed_run_count: usize,                       // running runs with retry attempts
}

pub struct DegradationSignal {
    pub analysis_id: String,                            // links back to StewardAnalysis
    pub metric_name: String,                            // e.g. "lead_time_median"
    pub metric_family: String,                          // "timing" | "rework" | "quality" | "cost" | "stability"
    pub observed_value: f64,
    pub baseline_value: f64,
    pub delta_percentage: f64,
    pub threshold_used: f64,
    pub severity: String,                               // "high" | "medium" | "low"
    pub confidence: String,                             // capped by cohort quality
    pub implicated_run_ids: Vec<RunId>,
    pub likely_causes: Vec<String>,                     // empty in V1; populated by V2 recommender
}

pub struct RunDossier {
    pub run_id: RunId,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub workflow_snapshot_hash: String,                  // SHA-256 of workflow YAML at run time
    pub catalog_snapshot_hash: String,                   // SHA-256 of catalog YAML at run time
    pub stage_execution_summaries: Vec<StageExecutionSummary>,
    pub approval_history: Vec<ApprovalSummary>,
    pub cost_breakdown: CostBreakdown,
    pub failure_retry_events: Vec<FailureEvent>,
    pub artifact_manifest: Vec<ArtifactSummary>,
    pub loop_counters: HashMap<String, u64>,            // "proposal_refinement_loop" → count
    pub drift_detected_at: Option<DateTime<Utc>>,
    pub drift_details: Option<String>,
}

pub struct StageExecutionSummary {
    pub stage_id: String,
    pub label: String,
    pub status: String,
    pub duration_seconds: f64,
    pub iteration: i64,
    pub attempt_number: i64,
    pub agent_count: usize,
}

pub struct ApprovalSummary {
    pub stage_id: String,
    pub decision: String,
    pub wait_seconds: Option<f64>,
    pub comment: Option<String>,
}

pub struct CostBreakdown {
    pub total_cost_cents: i64,
    pub cost_by_stage: HashMap<String, i64>,
    pub cost_by_agent: HashMap<String, i64>,
}

pub struct FailureEvent {
    pub stage_id: String,
    pub agent_id: Option<String>,
    pub status: String,
    pub attempt_number: i64,
    pub retry_reason: Option<String>,
}

pub struct ArtifactSummary {
    pub name: String,
    pub format: String,
    pub size_bytes: Option<i64>,
    pub agent_id: String,
    pub stage_id: String,
}
```

---

## 4. Core Modules

### 4a. MetricsCollector (`engine/src/steward/metrics.rs`)

Pure calculation over DB data:

```rust
pub async fn collect_metrics(
    pool: &SqlitePool,
    run_ids: &[RunId],
) -> Result<MetricsSnapshot>;
```

Queries: runs, stage_executions, approvals, work_items (for cost), agent_executions (for per-agent cost). Computes medians and means across all five families. Empty set → neutral snapshot (zeros, None optionals, empty maps).

**Full signal list:**
- Timing: `lead_time_median` (completedAt - startedAt), `stage_latency_medians` (per-stage duration median), `approval_wait_median` (decidedAt - requestedAt)
- Rework: `proposal_loop_mean`, `implementation_loop_mean` (from stage iteration counts), `retries_per_stage_mean` (attempt_number > 1 counts)
- Quality: `approval_rejection_rate` (rejected / decided), `audit_pass_rate` (completed audit stages / total audit stages)
- Cost: `cost_per_run_median` (from work_item cost_cents), `cost_by_stage_family` (aggregated per stage_id)
- Stability: `failed_run_rate`, `blocked_run_rate`, `drift_event_count`, `resumed_run_count`

### 4b. AnomalyDetector (`engine/src/steward/anomaly.rs`)

```rust
pub fn detect(
    analysis_id: &str,
    observation: &MetricsSnapshot,
    baseline: &MetricsSnapshot,
    thresholds: &HashMap<String, ThresholdEntry>,
    minimum_window_size: usize,
    cohort_quality: CohortQuality,
    observation_run_ids: &[RunId],
) -> Vec<DegradationSignal>;
```

Guard: if `observation.run_count < minimum_window_size` OR `baseline.run_count < max(minimum_window_size, 3)` → log `sample_too_small`, return empty (never silently skip).

**Detection per metric family (matching Swift):**

| Family | Metric | Default Threshold | Method |
|--------|--------|-------------------|--------|
| timing | `lead_time_median_seconds` | 0.30 (30%) | `median_percentage` |
| rework | `proposal_loop_mean` | 0.50 (50%) | `mean_percentage` |
| quality | `approval_rejection_rate` | 2.0 | `ratio` |
| cost | `cost_per_run_median_cents` | 0.25 (25%) | `median_percentage` |
| stability | `failed_run_rate` | 2.0 | `ratio` |

**Severity:** `delta >= 1.0` → high, `>= 0.5` → medium, else low.

**Confidence cap:** strong cohort → high, acceptable → medium, weak → low.

Each signal carries `analysis_id` and `likely_causes: Vec::new()` (populated by V2 recommender).

### 4c. CohortClassifier (`engine/src/steward/cohort.rs`)

```rust
pub fn split_windows(
    runs: &[Run],
    observation_size: usize,
    baseline_size: usize,
    max_age_days: i64,
) -> (Vec<Run>, Vec<Run>);

pub fn classify_quality(runs: &[Run]) -> CohortQuality;
```

Quality classification (matching Swift):
- `strong`: run_count >= 10 AND no untagged projects AND no unknown stacks
- `acceptable`: (run_count 5–9 OR has unknown stacks) AND not weak
- `weak`: run_count < 5 OR has untagged projects

### 4d. RunDossierBuilder (`engine/src/steward/dossier.rs`)

```rust
pub async fn build_dossiers(
    pool: &SqlitePool,
    run_ids: &[RunId],
) -> Result<Vec<RunDossier>>;
```

Assembles per-run evidence with full provenance: stage timelines, approval history, cost breakdown (per-stage + per-agent), failure/retry events, artifact manifest, **loop counters** (from stage iteration counts), **workflow/catalog snapshot hashes** (from run metadata), **drift details** (if drift was detected during the run).

### 4e. StewardAnalysisService (`engine/src/steward/service.rs`)

Orchestrates the full 11-step pipeline:

```rust
pub async fn run_analysis(
    pool: &SqlitePool,
    config: &StewardConfig,
    artifact_base: &str,
    agent_executor: Option<&dyn AgentExecutor>,  // for optional LLM lanes
    catalog: Option<&AgentCatalogFile>,
) -> Result<StewardAnalysisResult>;
```

**Step 10 — both LLM lanes:**

If `system_steward` agent exists in catalog:
1. Build prompt with metrics snapshot + degradation alerts + dossier summaries
2. Execute via `agent_executor`
3. Write output to `health-report.json`
4. Record path as `report_artifact_path` on analysis record

If `steward_auditor` agent exists in catalog:
1. Build prompt with health-report.json + raw metrics + degradation alerts
2. Execute via `agent_executor`
3. Write output to `audit-report.json`
4. Record path as `audit_artifact_path` on analysis record

`steward_auditor` is optional, but it is not independent of `system_steward` in V1. It runs only when `health-report.json` exists, because that report is part of the stable prompt input chain. If `system_steward` is absent or fails before emitting `health-report.json`, `steward_auditor` is skipped and the deterministic analysis record still persists.

---

## 5. Configuration — Full Schema Parity

```yaml
# examples/steward/steward_config.yaml
schema_version: 1
windows:
  observation_window_size: 20
  baseline_window_size: 20
  minimum_window_size: 5
  maximum_window_age_days: 90
thresholds:
  timing:    { method: median_percentage, trigger: 0.30 }
  rework:    { method: mean_percentage,   trigger: 0.50 }
  quality:   { method: ratio,             trigger: 2.0 }
  cost:      { method: median_percentage, trigger: 0.25 }
  stability: { method: ratio,             trigger: 2.0 }
triggers:
  post_run_hook: { enabled: false, run_interval: 5 }
  on_config_change: { enabled: true }
  schedule: { enabled: false, cron: "0 8 * * 1" }  # parsed, not wired in V1
context_strategy_profiles:
  current_mixed_baseline: { ... }    # per-agent handoff policies (V1: parsed + validated, not yet consumed by runtime)
```

**Rust config struct (full schema):**

```rust
pub struct StewardConfig {
    pub schema_version: u32,
    pub windows: WindowConfig,
    pub thresholds: HashMap<String, ThresholdEntry>,
    pub triggers: TriggerConfig,
    pub context_strategy_profiles: HashMap<String, serde_yaml::Value>,  // V1: parsed + validated, consumed later
}

pub struct TriggerConfig {
    pub post_run_hook: PostRunHookConfig,
    pub on_config_change: OnConfigChangeConfig,
    pub schedule: ScheduleConfig,               // parsed, not wired in V1
}

pub struct ScheduleConfig {
    pub enabled: bool,
    pub cron: Option<String>,
}
```

**Validation (matching Swift `YAMLValidator.validateStewardConfig`):**
1. `schema_version == 1`
2. All window sizes positive; `minimum_window_size <= observation_window_size`
3. All five threshold families required: timing, rework, quality, cost, stability
4. All threshold methods one of: `median_percentage`, `mean_percentage`, `ratio`
5. All threshold triggers positive
6. If `post_run_hook.enabled == true`, then `run_interval >= 1`
7. `context_strategy_profiles` must contain at least one profile
8. Missing recommended profile `selective_compression_and_escalation` is a warning, not an error
9. A strategy profile with no agent entries is a warning
10. A strategy profile that defines `escalation_model_tier` without `default_model_tier` is a warning
11. An empty agent key inside a strategy profile is an error
12. An agent profile with neither `handoff_policy` nor `continuity_mode` is a warning
13. Empty artifact references in `handoff_policy.mandatory`, `.summarized`, or `.lazy` are errors

“Full validation” in this proposal means parity with the current Swift validator’s error-and-warning split, including nested `context_strategy_profiles` checks, not only the seven top-level schema checks above.

---

## 6. Trigger Integration

### 6a. Post-Run Hook

In the orchestrator, after `runs::mark_completed`:

```rust
self.steward_completed_counter += 1;
if config.triggers.post_run_hook.enabled
    && self.steward_completed_counter >= config.triggers.post_run_hook.run_interval
{
    self.steward_completed_counter = 0;
    // Enqueue StewardAnalysis work item
}
```

### 6b. Config-Change Trigger — Dual Canonical Hash

At daemon startup, compute **two separate canonical hashes** (matching Swift provenance model):
1. `steward_config_hash` = hash of the parsed `StewardConfig` value through the `DefinitionHasher` owner model (`canonicalEncoder` / stable canonical serialization), not raw YAML bytes
2. `catalog_hash` = hash of the parsed `AgentCatalogFile` value through the same canonical hash owner model, not raw YAML bytes

Fetch most recent `StewardAnalysis` record. Compare both:
- If `steward_config_hash != analysis.steward_config_snapshot_hash` → schedule analysis
- If `catalog_hash != analysis.workflow_catalog_snapshot_hash` → schedule analysis

This ensures strategy profile changes, threshold edits, **and** catalog changes (new agents, changed backend profiles, modified MCP) all trigger follow-up analysis, while comment-only, whitespace-only, or formatting-only YAML churn does not. The two hashes are stored separately on `steward_analyses` so the trigger source is identifiable.

### 6c. Manual Trigger

Expose via the existing MCP `tools/list` / `tools/call` surface as a namespaced tool, for example:

```json
{
  "name": "steward.run_analysis",
  "arguments": {
    "reason": "operator_manual_trigger"
  }
}
```

The tool is owned by the current MCP server boundary, not an ad hoc JSON-RPC method. The implementation lives alongside the existing namespaced tool handlers and dispatch:
- `control-plane/crates/mcp-server/src/tools/steward.rs` — tool definition + handler
- `control-plane/crates/mcp-server/src/tools/mod.rs` — module registration
- `control-plane/crates/mcp-server/src/server.rs` — `tools/list` / `tools/call` dispatch wiring

---

## 7. Disk Artifacts

```
{artifact_base}/steward/analyses/{analysis_id}/
  metrics-window.json
  baseline-window.json
  dossiers/{run_id}.json        (one per implicated run, with provenance hashes)
  degradation-alerts.json
  health-report.json             (optional, from system_steward LLM agent)
  audit-report.json              (optional, from steward_auditor LLM agent)
```

All JSON: `serde_json` with sorted keys, ISO-8601 dates.

---

## 8. Files to Create

| File | Responsibility |
|---|---|
| `engine/src/steward/mod.rs` | Module root |
| `engine/src/steward/service.rs` | Pipeline orchestrator (11 steps, both LLM lanes) |
| `engine/src/steward/metrics.rs` | MetricsCollector — all 5 families, full signal set |
| `engine/src/steward/anomaly.rs` | AnomalyDetector — threshold comparison with analysis_id linkage |
| `engine/src/steward/cohort.rs` | CohortClassifier — window splitting, quality classification |
| `engine/src/steward/dossier.rs` | RunDossierBuilder — evidence with provenance hashes, loop counters, drift |
| `engine/src/steward/config.rs` | StewardConfig deserialization + full validation parity, including nested strategy-profile warnings/errors |
| `domain/src/steward.rs` | Domain types (MetricsSnapshot, DegradationSignal, RunDossier, etc.) |
| `db/migrations/005_steward.sql` | Schema for analyses, run_links, recommendations |
| `db/src/repos/steward.rs` | CRUD for Steward tables |
| `control-plane/crates/mcp-server/src/tools/steward.rs` | **NEW** — `steward.run_analysis` MCP tool handler |
| `control-plane/crates/mcp-server/src/tools/mod.rs` | Register Steward MCP tool module |
| `control-plane/crates/mcp-server/src/server.rs` | Expose `steward.run_analysis` through current `tools/list` / `tools/call` dispatch |

---

## 9. V2/V3 Roadmap (from Swift)

- **V2 (Recommender):** Translate degradation signals into concrete config patch recommendations. `DegradationSignal.likely_causes` populated here.
- **V3 (Experimenter):** A/B testing framework. Uses `StewardExperiment` and `StewardDecision` models (already defined in Swift, unused in V1).

This proposal covers V1 only. V2/V3 are separate proposals once V1 proves stable.

---

## 10. Acceptance Criteria

### Deterministic Pipeline
1. After 10+ completed runs, `steward.run_analysis` produces `metrics-window.json` and `baseline-window.json` with all five metric families populated.
2. `metrics-window.json` contains: `retries_per_stage_mean`, `audit_pass_rate`, `cost_by_stage_family`, `drift_event_count`, `resumed_run_count` — not just timing/rework.
3. If observation window shows 40% timing regression: `degradation-alerts.json` contains a signal with `severity: "medium"`, `metric_family: "timing"`, `analysis_id` linking to the analysis record.
4. `steward_analyses` table has a record with correct `cohort_quality`, `degradations_detected`, `workflow_catalog_snapshot_hash`, and `steward_config_snapshot_hash`.
5. `steward_recommendations` has one entry per signal with `status: "proposed"`.
6. Fewer than 5 completed runs → analysis status = `inconclusive`, zero signals (not false positives).
7. Running analysis twice on same data → identical JSON artifacts (determinism).

### Evidence Dossiers
8. `dossiers/{run_id}.json` contains `workflow_snapshot_hash`, `catalog_snapshot_hash`, `loop_counters`, `drift_detected_at`, `cost_breakdown` (per-stage + per-agent), and `failure_retry_events`.

### Optional LLM Lanes
9. If `system_steward` agent exists in catalog → `health-report.json` produced, `report_artifact_path` set on analysis record.
10. If `steward_auditor` agent exists in catalog and `health-report.json` exists → `audit-report.json` produced, `audit_artifact_path` set on analysis record.
11. If `system_steward` is absent or fails before emitting `health-report.json` → `steward_auditor` is skipped, `audit_artifact_path` remains None, and the deterministic analysis record still persists.
12. If either executed LLM lane fails → analysis record still persisted with deterministic fields; artifact path remains None for the failed lane.

### Config-Change Detection
13. Changing `agents.yaml` (but not `steward_config.yaml`) → `workflow_catalog_snapshot_hash` differs from last analysis → config-change trigger fires.
14. Changing `steward_config.yaml` (but not `agents.yaml`) → `steward_config_snapshot_hash` differs → config-change trigger fires.
15. Config validation rejects missing threshold families, invalid methods, zero window sizes, empty agent keys, and empty handoff artifact references.
16. Config validation also emits non-fatal warnings for missing recommended profiles, empty strategy profiles, escalation tier without default tier, and agent profiles with neither `handoff_policy` nor `continuity_mode`.
17. Comment-only, whitespace-only, or key-order-only YAML churn in `steward_config.yaml` or `agents.yaml` does not fire config-change analysis, because hashes are computed from canonical parsed objects rather than raw file bytes.

### Manual MCP Trigger
18. MCP `tools/list` exposes `steward.run_analysis`, and `tools/call` on that tool enqueues or executes a manual Steward analysis without introducing any raw JSON-RPC `steward/run_analysis` method.

---

## 11. Test Gate

```bash
proposal-049|p049)
  log "Proposal 049 control-plane gate: Steward analysis system"
  (
    cd "$ROOT_DIR/control-plane"
    cargo test --workspace 2>&1
  )
  log "Proposal 049 control-plane gate passed"
  ;;
```

---

## 12. Out of Scope

- **V2 Recommender / V3 Experimenter**: Separate proposals.
- **Session checkpoint serialization**: Steward reads completed run data, not live session state.
- **UI for Steward dashboard**: Thin-client concern.
- **Runtime consumption of `context_strategy_profiles`**: V1 parses and validates them; runtime consumption is a separate feature.
