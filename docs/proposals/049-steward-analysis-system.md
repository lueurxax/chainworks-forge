# Proposal 049: Steward Analysis System — Deterministic Quality Observatory

| Field | Value |
|---|---|
| Date | 2026-04-15 |
| Status | Draft |
| Author | Claude |
| Depends on | [../reference/current-system-baseline.md](../reference/current-system-baseline.md), [../reference/forge-steward.md](../reference/forge-steward.md), [../reference/structured-output-envelope-and-contract-validation.md](../reference/structured-output-envelope-and-contract-validation.md), [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md), [048-evidence-packs-delivery-preflight-and-mcp-resolution.md](048-evidence-packs-delivery-preflight-and-mcp-resolution.md) |
| Scope | Port the deterministic Steward V1 observer pipeline to the Rust daemon, including: cohort selection, metrics collection, anomaly detection, dossiers, persisted analysis records, active-catalog steward LLM lanes, daemon-owned current-input hashing, queue-based triggers, and northbound analysis readback. |
| Goal | The Rust daemon can run Steward analyses deterministically from persisted run truth, execute the active steward catalog contract when present, and expose analyses/recommendations through the same GraphQL/MCP/report spine used by other durable subsystem truth. |

---

## 1. Context and Motivation

The Steward is already a stable product concept in [../reference/forge-steward.md](../reference/forge-steward.md). What is missing is the Rust control-plane implementation.

This proposal is therefore a **delta on top of the current baseline**, not a greenfield design.

Current repo reality that this proposal must respect:

- the current daemon already expects durable subsystem truth to surface northbound through GraphQL, MCP tools/resources, and report-style reads
- the active catalog in `examples/agents/agents.yaml` already defines steward agents and output contracts
- the current workflow/catalog/idea contracts do **not** yet expose deterministic owners for all cohort fields Steward wants to use
- the current Rust persistence owns some metric inputs already (`runs`, `stage_executions`, `approvals`, `session_generations`, `validation_failure_records`) and lacks others (`drift_*`, explicit retry reason, steward analysis tables)

The immediate proposal-readiness blockers are therefore concrete:

1. define authoritative owners for `workflow_family`, `project_key`, `risk_class`, and `stack`
2. reconcile Steward's optional LLM lanes with the **active** steward catalog contract rather than a narrower hypothetical one
3. define one canonical northbound read surface for analyses and recommendations

This revision resolves those three P0 items directly.

---

## 2. Architecture Overview

### 2a. Pipeline

The deterministic Rust Steward pipeline remains the same high-level observer loop as the stable reference:

```text
1. Load and validate daemon-owned current inputs:
   - current steward_config.yaml
   - current agents.yaml
2. Query completed runs from DB
3. Filter to deterministic-eligible completed runs:
   - required: `workflow_family`, `risk_class`, and snapshot provenance fields from run start are present (`workflow_snapshot_hash`, `catalog_snapshot_hash`, `workflow_snapshot_json`, `catalog_snapshot_json`)
   - legacy, pre-P049 runs that miss any of these fields are excluded from cohort analysis
   - excluded runs are visible only as metadata in analysis observability for migration/backfill planning
4. Select primary cohort by the persisted run-owned primary cohort key `(workflow_family, risk_class)`
5. Split observation and baseline windows
6. Classify cohort quality using `project_key` and `stack` as quality facets, not grouping keys
7. Collect deterministic metrics from persisted owners
8. Detect degradations and improvements
9. Build dossiers for implicated runs, or bounded context dossiers when no runs are implicated
10. Write canonical JSON artifacts
11. [Optional] Execute active steward catalog agents:
    - system_steward
    - steward_auditor
12. Persist analysis record, run links, recommendations, and artifact pointers
13. Expose the analysis through GraphQL, MCP tools/resources, and report-style reads
```

Primary cohort selection in this pipeline is not left implicit:

- the primary cohort key is exactly `(workflow_family, risk_class)`
- `project_key` and `stack` are quality and diagnostic facets only
- all downstream windowing, signal generation, and recommendation continuity inherit that split

Legacy-run rule:

- Completed runs predating this proposal are treated as ineligible by default until they carry both snapshot and cohort-owned fields.
- Historical backfill is a migration-only maintenance path; Steward analysis itself does not recompute frozen cohort or snapshot truth from mutable workspace files.

### 2b. Determinism boundary

Steps 1–9 are deterministic and pure over:

- persisted DB truth
- daemon-owned parsed current inputs
- canonical hashing/serialization

The optional steward agent lanes are the only non-deterministic slice. They must never block persistence of the deterministic analysis record.

### 2c. Active-catalog parity rule

This proposal chooses **current-catalog parity**, not a V1-only catalog downgrade.

That means the Rust Steward LLM lane must support the steward outputs that are live today in `examples/agents/agents.yaml`:

- `system_steward`
  - `sdlc_health_report`
  - `degradation_alert`
  - `agent_tuning_proposal`
  - `workflow_tuning_proposal`
  - `experiment_plan`
- `steward_auditor`
  - `stewardship_audit_report`

Steward analysis execution sets `CHAINWORKS_META_ROOT` to the analysis-owned catalog-IO root:

```text
{artifact_base}/steward/analyses/{analysis_id}/active-catalog-io
```

All optional steward-agent inputs and outputs are materialized at the active catalog's existing relative paths under that root. The proposal does not invent a second path vocabulary for the LLM lane.

---

## 3. Canonical Owner Contracts

### 3a. Cohort metadata owners

Steward cohorting must read persisted run-owned fields, not infer from mutable files after the fact.

This proposal introduces the missing authoritative owners explicitly.

#### Primary cohort key versus quality facets

Stable Steward V1 parity requires an explicit owner split:

- primary cohort key: `(workflow_family, risk_class)`
- quality facets only: `project_key`, `stack`

That means:

- window selection, baseline selection, signal generation, and recommendation continuity are grouped only by `workflow_family` plus `risk_class`
- `project_key` and `stack` never create a narrower primary cohort on their own
- `project_key` and `stack` still affect `CohortQuality`, dossier interpretation, and recommendation text

Persisted analysis/readback shape must preserve that split:

- `steward_analyses.cohort_keys_json` records the selected primary key tuple
- any project/stack diagnostics are recorded separately as quality facets or dossier fields, not as the primary cohort identity

Concrete `cohort_keys_json` contract for V1:

```json
{
  "workflow_family": "proposal_to_release",
  "risk_class": "standard"
}
```

It must not include `project_key` or `stack`.

#### Workflow-owned fields

Extend `workflow/src/definition.rs` `WorkflowMeta` with:

```rust
pub struct WorkflowMeta {
    pub id: Option<String>,
    pub name: Option<String>,
    pub description: Option<String>,
    pub family: Option<String>,
    pub risk_class: Option<String>,
    pub stack: Option<String>,
    // existing fields...
}
```

Owner rules:

- `workflow_family`
  - canonical source: `workflow.workflow.family`
  - legacy fallback: `workflow.workflow.id`
  - if both are absent, run creation fails
- `risk_class`
  - canonical source: `workflow.workflow.risk_class`
  - temporary fallback for legacy workflows: `"standard"`
- `stack`
  - canonical source: `workflow.workflow.stack`
  - temporary fallback for legacy workflows: `"unknown"`

These values are frozen onto `Run` at run creation time and never recomputed from current workflow YAML during Steward analysis.

#### Idea-owned field

Extend the current idea owner chain end-to-end:

- `domain/src/idea.rs`
- `db/src/repos/ideas.rs`
- the next free DB migration ordinal at implementation time; do not anchor this to a stale migration slot
- `mcp-server/src/tools/ideas.rs` for `ideas.create`

with:

```rust
pub struct Idea {
    // existing fields...
    pub project_key: Option<String>,
}
```

Owner rule:

- `project_key`
  - canonical source: `Idea.project_key`
  - fallback: `"untagged"`

Ingress rule:

- `ideas.create` accepts optional `project_key`
- legacy rows and callers that omit it persist `NULL`
- Steward freezes `"untagged"` onto `Run.project_key` only when the persisted idea field is absent

This keeps the “untagged project” downgrade deterministic without requiring runtime heuristics and avoids inventing project identity at analysis time.

#### Frozen run-owned fields

`domain/src/run.rs` and the `runs` table are widened with:

```rust
pub struct Run {
    // existing fields...
    pub workflow_family: Option<String>,
    pub project_key: Option<String>,
    pub risk_class: Option<String>,
    pub stack: Option<String>,
    pub workflow_snapshot_hash: Option<String>,
    pub catalog_snapshot_hash: Option<String>,
    pub workflow_snapshot_json: Option<String>,
    pub catalog_snapshot_json: Option<String>,
    pub drift_detected_at: Option<DateTime<Utc>>,
    pub drift_details_json: Option<String>,
}
```

Run owner chain:

```text
workflow definition + agent catalog YAML paths
  -> workflow::compiler parses and normalizes frozen definitions
  -> DefinitionHasher computes canonical workflow/catalog hashes from those parsed frozen definitions
  -> workflow::compiler returns the frozen snapshot payloads plus the computed hashes
  -> engine::command_handler::StartRun persists workflow_family / project_key / risk_class / stack
     and also persists workflow_snapshot_hash/json + catalog_snapshot_hash/json on Run
 -> db::repos::runs is the durable round-trip owner
 -> Steward reads only the persisted Run fields

Active ingress contract:

- GraphQL `startRun` and MCP `runs.start` are updated to require both `workflow_yaml_path` and `agent_catalog_yaml_path`.
- `StartRunCmd` is therefore the owning command contract for those frozen snapshot inputs; callsites in other docs/code paths should be updated to match this contract.
```

Producer bridge requirements:

- this proposal does not allow snapshot hashes or snapshot JSON to be recomputed ad hoc from mutable files during analysis
- the only valid producer bridge is run-creation-time compilation plus hashing from the frozen parsed definitions
- `workflow_yaml_path` and `agent_catalog_yaml_path` are ingress pointers only; they are not the canonical snapshot truth once the run has been created

Concrete Rust producer contract:

- `workflow/src/compiler.rs`
  - parses YAML inputs into frozen normalized definitions
  - emits the frozen workflow/catalog snapshot payloads
- `DefinitionHasher`
  - hashes those parsed frozen definitions, not raw YAML bytes and not later reloaded files
- `workflow/src/plan.rs`
  - carries the compiler-produced snapshot payload/hash bridge needed by run creation
- `engine/src/command_handler.rs`
  - persists `workflow_snapshot_hash/json` and `catalog_snapshot_hash/json` during `StartRun`
- `db/src/repos/runs.rs`
  - is the durable round-trip owner

### 3b. Retry and drift owners

Steward needs durable owners for retry/drift signals instead of hand-wavy inference.

#### Drift

Run-owned fields:

- `runs.drift_detected_at`
- `runs.drift_details_json`

Owner:

- recovery / resume classification path updates these fields when a run is marked drifted or requires operator remap

#### Retry reason

Stage-owned field:

- `stage_executions.retry_reason`

Owner:

- `RetryStage` command path writes the reason when it creates the next attempt

This proposal does not treat `FailureEvent.retry_reason` as implementable until this field exists.

### 3c. LLM-lane owners

The optional LLM lanes consume deterministic artifact inputs and produce contract-bound artifacts.

#### `system_steward`

Inputs:

- `metrics_window`
- `baseline_window`
- `implicated_run_dossiers`
- `agent_catalog_snapshot`
- `workflow_snapshot`
- `config_change_log`

Materialization rules for those inputs:

- `agent_catalog_snapshot`
  - analysis-owned artifact derived from the daemon-owned current `AgentCatalogFile`
  - canonical path: `active-catalog-io/steward/catalog-snapshot.json`
  - always singular because Steward has one daemon-owned current catalog input at analysis time
  - persisted analysis hash: `agent_catalog_snapshot_hash`
- `workflow_snapshot`
  - analysis-owned artifact that materializes the distinct frozen workflow snapshots present across the analyzed runs
  - canonical path: `active-catalog-io/steward/workflow-snapshot.json`
  - singular by contract, but its payload is an index:
    - `snapshot_count`
    - `primary_workflow_family`
    - `entries[]` with `{ workflow_snapshot_hash, workflow_family, run_ids, workflow_snapshot_json }`
  - this avoids pretending there is only one frozen workflow when the cohort spans multiple snapshot hashes while still satisfying the active catalog's singular input name
  - persisted analysis hash: `workflow_snapshot_artifact_hash`
- `config_change_log`
  - analysis-owned artifact derived from the daemon-owned current-input hash comparison against the previous completed analysis
  - canonical path: `active-catalog-io/steward/config-change-log.json`
  - payload includes:
    - `reason`
    - `previous_steward_config_hash`
    - `current_steward_config_hash`
    - `previous_agent_catalog_hash`
    - `current_agent_catalog_hash`
    - `changed_inputs[]`
    - `trigger_pending_before_run`

These three inputs are canonical analysis-owned materializations, not ephemeral in-memory handoffs.

Persisted hash rules:

- `workflow_snapshot_artifact_hash` is the canonical JSON hash of the materialized `workflow_snapshot` artifact, not a single run-owned workflow hash
- the `workflow_snapshot` artifact payload may contain multiple run-owned `workflow_snapshot_hash` values in `entries[]`
- `agent_catalog_snapshot_hash` is the daemon-owned current `agent_catalog_hash` for the catalog snapshot materialized into the analysis
- `steward_config_snapshot_hash` is the hash of the effective steward config used for this analysis
- no persisted field may collapse workflow snapshot aggregate truth and current agent catalog truth into one scalar

Outputs:

- `sdlc_health_report`
- `degradation_alert`
- `agent_tuning_proposal`
- `workflow_tuning_proposal`
- `experiment_plan`

#### `steward_auditor`

Inputs:

- `sdlc_health_report`
- `implicated_run_dossiers`
- `metrics_window`
- `baseline_window`

Outputs:

- `stewardship_audit_report`

`steward_auditor` remains dependent on the presence of `sdlc_health_report`, matching the active catalog chain.

---

## 4. Data Model

### 4a. Analysis tables

```sql
CREATE TABLE steward_analyses (
    id TEXT PRIMARY KEY,
    created_at TEXT NOT NULL,
    window_start TEXT NOT NULL,
    window_end TEXT NOT NULL,
    run_count INTEGER NOT NULL,
    cohort_keys_json TEXT NOT NULL,
    cohort_quality TEXT NOT NULL,
    status TEXT NOT NULL, -- completed | inconclusive | failed | superseded
    degradation_count INTEGER NOT NULL DEFAULT 0,
    improvement_count INTEGER NOT NULL DEFAULT 0,
    workflow_snapshot_artifact_hash TEXT NOT NULL,
    agent_catalog_snapshot_hash TEXT NOT NULL,
    steward_config_snapshot_hash TEXT NOT NULL,
    metrics_snapshot_artifact_id TEXT,
    baseline_snapshot_artifact_id TEXT,
    agent_catalog_snapshot_artifact_id TEXT,
    workflow_snapshot_artifact_id TEXT,
    config_change_log_artifact_id TEXT,
    health_report_artifact_id TEXT,
    degradation_alert_artifact_id TEXT,
    agent_tuning_artifact_id TEXT,
    workflow_tuning_artifact_id TEXT,
    experiment_plan_artifact_id TEXT,
    audit_report_artifact_id TEXT,
    trigger_reason TEXT NOT NULL, -- manual | post_run_hook | config_change
    error_summary TEXT
);

CREATE TABLE steward_analysis_run_links (
    id TEXT PRIMARY KEY,
    analysis_id TEXT NOT NULL REFERENCES steward_analyses(id),
    run_id TEXT NOT NULL REFERENCES runs(id),
    role TEXT NOT NULL -- implicated | baseline | context
);

CREATE TABLE steward_recommendations (
    id TEXT PRIMARY KEY,
    analysis_id TEXT NOT NULL REFERENCES steward_analyses(id),
    created_at TEXT NOT NULL,
    category TEXT NOT NULL,
    summary TEXT NOT NULL,
    target_metric TEXT NOT NULL,
    confidence_level TEXT NOT NULL,
    status TEXT NOT NULL, -- proposed | accepted | rejected | superseded | rolled_back
    source_artifact_name TEXT, -- agent_tuning_proposal | workflow_tuning_proposal | experiment_plan | deterministic_signal
    decision_comment TEXT,
    decided_at TEXT
);
```

### 4b. Core Rust types

```rust
use std::collections::BTreeMap;

pub struct MetricsSnapshot {
    pub run_count: usize,
    pub window_start: DateTime<Utc>,
    pub window_end: DateTime<Utc>,
    pub lead_time_median_seconds: Option<f64>,
    pub stage_latency_medians: BTreeMap<String, f64>,
    pub approval_wait_median_seconds: Option<f64>,
    pub proposal_loop_mean: f64,
    pub implementation_loop_mean: f64,
    pub retries_per_stage_mean: BTreeMap<String, f64>,
    pub approval_rejection_rate: f64,
    pub audit_pass_rate: f64,
    pub cost_per_run_median_cents: Option<i64>,
    pub cost_by_stage_family: BTreeMap<String, i64>,
    pub failed_run_rate: f64,
    pub blocked_run_rate: f64,
    pub drift_event_count: usize,
    pub resumed_run_count: usize,
}

pub struct DegradationSignal {
    pub analysis_id: String,
    pub metric_name: String,
    pub metric_family: String,
    pub observed_value: f64,
    pub baseline_value: f64,
    pub delta_percentage: f64,
    pub threshold_used: f64,
    pub severity: String,
    pub confidence: String,
    pub implicated_run_ids: Vec<RunId>,
    pub likely_causes: Vec<String>,
}

pub struct RunDossier {
    pub run_id: RunId,
    pub started_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub status: String,
    pub workflow_family: Option<String>,
    pub project_key: Option<String>,
    pub risk_class: Option<String>,
    pub stack: Option<String>,
    pub workflow_snapshot_hash: String,
    pub catalog_snapshot_hash: String,
    pub stage_execution_summaries: Vec<StageExecutionSummary>,
    pub approval_history: Vec<ApprovalSummary>,
    pub cost_breakdown: CostBreakdown,
    pub failure_retry_events: Vec<FailureEvent>,
    pub artifact_manifest: Vec<ArtifactSummary>,
    pub loop_counters: BTreeMap<String, u64>,
    pub drift_detected_at: Option<DateTime<Utc>>,
    pub drift_details_json: Option<String>,
}
```

```rust
pub struct StewardAnalysisRecord {
    pub id: String,
    pub cohort_keys_json: String, // serialized PrimaryCohortKey only
    pub cohort_quality: String,
    // ...
}
```

`StewardAnalysisRecord.cohort_keys_json` carries only the primary cohort tuple.
`project_key` and `stack` continue to appear in run dossiers and quality grading, but not in the canonical cohort identity.

---

## 5. Metrics and Source-of-Truth Matrix

The metrics layer must use current or newly-introduced durable owners only.

| Signal | Canonical owner | Notes |
|---|---|---|
| `lead_time_median_seconds` | `runs.started_at`, `runs.completed_at` | Completed runs only |
| `stage_latency_medians` | `stage_executions.started_at`, `stage_executions.completed_at`, `stage_executions.stage_id` | Median per stage ID |
| `approval_wait_median_seconds` | `approvals.requested_at`, `approvals.decided_at` | Current owner already exists |
| `proposal_loop_mean` / `implementation_loop_mean` | `stage_executions.iteration` grouped by proposal/implementation states | Requires deterministic stage-family mapping from workflow snapshot |
| `retries_per_stage_mean` | `stage_executions.attempt_number` grouped by `stage_id` | Current owner already exists |
| `approval_rejection_rate` | `approvals.decision` | Current owner already exists |
| `audit_pass_rate` | `stage_executions.status` for audit-class stages | Stage family comes from frozen workflow snapshot |
| `cost_per_run_median_cents` | sum of `session_generations.cumulative_cost_cents` for the latest generation referenced by each `agent_execution.session_generation_id` in the run | `work_items` are not cost owners |
| `cost_by_stage_family` | same joined cost source, grouped by `stage_executions.stage_id` | `agent_executions` alone are not cost owners |
| `failed_run_rate` | `runs.status` | Current owner already exists |
| `blocked_run_rate` | `runs.status` / blocked terminal state semantics | Must be defined in the service contract |
| `drift_event_count` | `runs.drift_detected_at` | New run-owned field introduced by this proposal |
| `resumed_run_count` | `agent_executions.session_reuse_disposition`, `agent_executions.session_reset_reason` | Current owner already exists |
| `failure_retry_events[].retry_reason` | `stage_executions.retry_reason` | New stage-owned field introduced by this proposal |
| dossier `cost_breakdown.cost_by_agent` | `agent_executions.session_generation_id -> session_generations.cumulative_cost_cents` | Joined through execution lineage |

No part of Steward cost parity in this proposal may rely on `work_items.cost` or non-existent cost fields on `agent_executions`.

---

## 6. Core Modules

### 6a. MetricsCollector

```rust
pub async fn collect_metrics(
    pool: &SqlitePool,
    run_ids: &[RunId],
) -> Result<MetricsSnapshot>;
```

This module owns only deterministic arithmetic and source joins.

### 6b. CohortClassifier

```rust
pub struct PrimaryCohortKey {
    pub workflow_family: String,
    pub risk_class: String,
}

pub fn select_primary_cohort_key(runs: &[Run]) -> Option<PrimaryCohortKey>;

pub fn split_windows(
    runs: &[Run],
    primary_key: &PrimaryCohortKey,
    observation_size: usize,
    baseline_size: usize,
    max_age_days: i64,
) -> (Vec<Run>, Vec<Run>);

pub fn classify_quality(runs: &[Run]) -> CohortQuality;
```

Grouping rules remain:

- `select_primary_cohort_key` chooses only by `(workflow_family, risk_class)`
- `split_windows` filters runs to that primary key before window slicing
- `project_key` and `stack` do not change cohort membership
- `classify_quality` then grades the selected cohort using:
  - run count
  - `project_key` completeness / `"untagged"` presence
  - `stack` completeness / `"unknown"` presence

No alternate grouping contract is allowed in V1:

- not `(workflow_family, risk_class, project_key, stack)`
- not `(workflow_family, project_key)`
- not “all frozen cohort fields”

Quality rules remain:

- `strong`: at least 10 runs, no untagged projects, no unknown stacks
- `acceptable`: at least 5 runs and not weak
- `weak`: fewer than 5 runs, or any untagged project cohort member

### 6c. AnomalyDetector

```rust
pub fn detect(
    analysis_id: &str,
    observation: &MetricsSnapshot,
    baseline: &MetricsSnapshot,
    thresholds: &BTreeMap<String, ThresholdEntry>,
    minimum_window_size: usize,
    cohort_quality: CohortQuality,
    observation_run_ids: &[RunId],
) -> Vec<DegradationSignal>;
```

Threshold methods:

- `median_percentage`
- `mean_percentage`
- `ratio`

### 6d. RunDossierBuilder

```rust
pub async fn build_dossiers(
    pool: &SqlitePool,
    run_ids: &[RunId],
) -> Result<Vec<RunDossier>>;
```

No dossier field may be reconstructed from mutable current YAML paths if a frozen run-owned snapshot field exists.

### 6e. StewardAnalysisService

```rust
pub async fn run_analysis(
    pool: &SqlitePool,
    runtime_inputs: &StewardRuntimeInputs,
    artifact_base: &str,
    agent_executor: Option<&dyn AgentExecutor>,
) -> Result<StewardAnalysisResult>;
```

`runtime_inputs` is the only current-input source.

---

## 7. Daemon-Owned Current Inputs

### 7a. Bootstrap owner

Introduce:

- `control-plane/crates/daemon/src/config.rs`
- `control-plane/crates/daemon/src/steward_runtime.rs`

```rust
pub struct StewardRuntimeInputs {
    pub steward_config_path: PathBuf,
    pub steward_config: StewardConfig,
    pub steward_config_hash: String,
    pub steward_config_load_status: StewardConfigLoadStatus,
    pub agent_catalog_path: PathBuf,
    pub agent_catalog: AgentCatalogFile,
    pub agent_catalog_hash: String,
    pub config_change_analysis_scheduled: AtomicBool,
}

pub enum StewardConfigLoadStatus {
    LoadedAndValidated,
    LoadedWithDefaultFallback { validation_errors: Vec<String> },
}
```

Canonical source rules:

- `STEWARD_CONFIG_PATH` if set, else `examples/steward/steward_config.yaml`
- `AGENT_CATALOG_PATH` if set, else `examples/agents/agents.yaml`

No other module may guess current steward paths from cwd or per-run YAML.

Bootstrap validation semantics remain aligned with stable Steward V1:

- the daemon loads the current steward config from the canonical source path
- it validates the parsed config through the Rust-side steward-config validation owner
- if validation fails, runtime inputs fall back to `StewardConfig::default_config()`-equivalent semantics rather than failing bootstrap
- the effective config used for hashing, trigger scheduling, and analysis execution is therefore:
  - the validated loaded config when validation succeeds
  - the default config when validation fails
- validation failure is recorded in `steward_config_load_status` and may be surfaced in diagnostics, but it does not suppress config-change scheduling or analysis continuity

### 7b. Config-change hashing

Config-change detection compares canonical parsed-object hashes for:

- current effective `StewardConfig`
- current `AgentCatalogFile`

The `StewardConfig` hash is over the full parsed configuration that affects Steward semantics, including:

- `windows`
- `thresholds`
- `context_strategy_profiles`
- `triggers`

`context_strategy_profiles` is not excluded from the hash. Its current semantics already participate in deterministic strategy assignment and recommendation truth, so excluding it would make config-change detection materially wrong.

Hash input semantics:

- when steward config validation succeeds, hash the validated loaded config
- when validation fails, hash the fallback default config that the daemon actually uses
- this preserves parity with the stable V1 runtime contract where invalid config does not create a third ambiguous “loaded but unusable” truth lane

The trigger semantics stay aligned with stable Steward V1:

- startup comparison sets a pending flag only
- the next completed run enqueues `WorkItemKind::StewardAnalysis` with `reason = "config_change"`
- startup does not immediately run the analysis while still claiming V1 parity

---

## 8. Northbound Read Surface

Steward is not implementation-ready unless analyses and recommendations can be read northbound.

### 8a. GraphQL

Add:

- `stewardAnalyses(limit, status)` query
- `stewardAnalysis(id)` query

Types:

```rust
pub struct GqlStewardAnalysis {
    pub id: ID,
    pub created_at: String,
    pub status: String,
    pub trigger_reason: String,
    pub cohort_quality: String,
    pub cohort_keys_json: String,
    pub run_count: i64,
    pub degradation_count: i64,
    pub improvement_count: i64,
    pub workflow_snapshot_artifact_hash: String,
    pub agent_catalog_snapshot_hash: String,
    pub steward_config_snapshot_hash: String,
    pub artifact_ids: Vec<ID>,
    pub error_summary: Option<String>,
    pub recommendations: Vec<GqlStewardRecommendation>,
    pub linked_runs: Vec<GqlStewardAnalysisRunLink>,
}
```

Read semantics:

- `completed`, `inconclusive`, and `failed` analyses must be distinguishable
- recommendations are first-class readback, not inferred from artifact presence
- `cohort_keys_json` in northbound reads decodes to the primary cohort tuple only
- project/stack diagnostics, when exposed, are carried as quality context or dossier content rather than folded into primary cohort identity

### 8b. MCP

Add namespaced tools:

- `steward.run_analysis`
- `steward.list_analyses`
- `steward.get_analysis`

Add resource:

- `steward-analysis://{analysis_id}`

The MCP analysis payload must contain:

- analysis metadata
- recommendation rows
- linked run IDs and roles
- artifact IDs / paths for deterministic outputs and optional steward-agent outputs

### 8c. Report/resource lane

Add a dedicated read lane for Steward analyses rather than hiding them inside unrelated run reports:

- GraphQL analysis queries are the canonical thin-client lane
- MCP `steward.get_analysis` and `steward-analysis://{analysis_id}` mirror the same persisted analysis row

This proposal does **not** claim that a disk artifact directory alone counts as “surfaced recommendations”.

---

## 9. Disk Artifacts

```text
{artifact_base}/steward/analyses/{analysis_id}/
  active-catalog-io/
    steward/metrics-window.json
    steward/baseline-window.json
    steward/dossiers/{run_id}.json
    steward/catalog-snapshot.json
    steward/workflow-snapshot.json
    steward/config-change-log.json
    steward/reports/health-report.json
    steward/reports/degradation-alert.json
    steward/reports/audit-report.json
    steward/proposals/agent-tuning.json
    steward/proposals/workflow-tuning.json
    steward/proposals/experiment-plan.json
  degradation-alerts.json
```

Rules:

- deterministic artifacts use canonical sorted-key serialization
- `HashMap` is not sufficient for artifact-visible maps
- canonical JSON writer owns byte-stable ordering and formatting
- `active-catalog-io` is the value of `CHAINWORKS_META_ROOT` for optional steward-agent execution
- active steward catalog input and output names are satisfied by persisted files under the same relative paths declared in `examples/agents/agents.yaml`
- `steward/workflow-snapshot.json` is a singular analysis input artifact whose payload may index multiple frozen run-owned workflow snapshots
- no artifact in this directory may redefine the primary cohort key beyond `(workflow_family, risk_class)`

When no runs are implicated, write up to five observation-window context dossiers and persist them with `role = "context"`.

---

## 10. Trigger Integration

### 10a. Work queue

Add:

```rust
pub enum WorkItemKind {
    // existing variants...
    StewardAnalysis,
}
```

All triggers converge on this queue lane:

- manual MCP trigger
- post-run interval trigger
- config-change pending flag on next completed run

### 10b. Post-run trigger

Post-run interval behavior mirrors stable Steward V1:

- increment completed-run counter
- if config-change pending, enqueue `StewardAnalysis(reason = "config_change")`
- else if post-run hook threshold reached, enqueue `StewardAnalysis(reason = "post_run_hook")`

### 10c. Manual trigger

`steward.run_analysis` enqueues the same work item with `reason = "manual"`.

There is no second direct-execution owner lane.

---

## 11. Files to Create / Modify

| File | Responsibility |
|---|---|
| `control-plane/crates/workflow/src/definition.rs` | Add workflow metadata owners: `family`, `risk_class`, `stack` |
| `control-plane/crates/workflow/src/compiler.rs` | Produce frozen workflow/catalog snapshot payloads and canonical hashes from parsed run-start definitions |
| `control-plane/crates/workflow/src/plan.rs` | Carry any frozen snapshot payload/hash fields needed to bridge compiler output into run creation deterministically |
| `control-plane/crates/domain/src/commands.rs` | Require/own run-start YAML path contract for deterministic snapshot bridging |
| `control-plane/crates/domain/src/idea.rs` | Add `project_key` |
| `control-plane/crates/db/src/repos/ideas.rs` | Persist/read `project_key` |
| `control-plane/crates/mcp-server/src/tools/ideas.rs` | Accept optional `project_key` on `ideas.create` and return it on reads |
| `control-plane/crates/graphql-server/src/schema.rs` | Make `startRun` carry required `workflow_yaml_path` and `agent_catalog_yaml_path` |
| `control-plane/crates/domain/src/run.rs` | Add frozen cohort/provenance/drift fields |
| `control-plane/crates/domain/src/stage.rs` | Add `retry_reason` |
| `control-plane/crates/domain/src/steward.rs` | Steward domain types |
| `control-plane/crates/engine/src/steward/mod.rs` | Module root |
| `control-plane/crates/engine/src/steward/service.rs` | Pipeline orchestrator |
| `control-plane/crates/engine/src/steward/metrics.rs` | Deterministic metrics |
| `control-plane/crates/engine/src/steward/anomaly.rs` | Signal detection |
| `control-plane/crates/engine/src/steward/cohort.rs` | Cohort splitting and quality |
| `control-plane/crates/engine/src/steward/dossier.rs` | Dossiers |
| `control-plane/crates/engine/src/steward/json.rs` | Canonical JSON writer |
| `control-plane/crates/engine/src/steward/config.rs` | Config validation |
| `control-plane/crates/db/migrations/00x_steward_analysis.sql` | Steward tables + run/stage widening + `ideas.project_key`; use the next free migration ordinal at implementation time rather than hard-coding a stale slot |
| `control-plane/crates/db/src/repos/steward.rs` | Steward repo layer |
| `control-plane/crates/db/src/repos/runs.rs` | Persist/read new run fields |
| `control-plane/crates/db/src/repos/stages.rs` | Persist/read `retry_reason` |
| `control-plane/crates/db/src/work_item.rs` | Add `StewardAnalysis` |
| `control-plane/crates/engine/src/command_handler.rs` | Freeze cohort/provenance fields at run creation and persist compiler-produced workflow/catalog snapshot hashes + payloads |
| `control-plane/crates/mcp-server/src/tools/runs.rs` | Make `runs.start` require workflow and catalog YAML input paths and pass them through StartRun |
| `control-plane/crates/engine/src/executor.rs` | Execute `StewardAnalysis` work item |
| `control-plane/crates/daemon/src/config.rs` | Daemon-owned steward paths |
| `control-plane/crates/daemon/src/steward_runtime.rs` | Current steward inputs, steward-config validation/fallback status, effective hashes, and pending flag |
| `control-plane/crates/daemon/src/main.rs` | Bootstrap runtime inputs |
| `control-plane/crates/graphql-server/src/types/steward.rs` | GraphQL analysis/recommendation types |
| `control-plane/crates/graphql-server/src/schema.rs` | Analysis queries |
| `control-plane/crates/mcp-server/src/tools/steward.rs` | Steward MCP tools |
| `control-plane/crates/mcp-server/src/tools/mod.rs` | Tool registration |
| `control-plane/crates/mcp-server/src/server.rs` | `steward-analysis://{analysis_id}` resource wiring |

---

## 12. Acceptance Criteria

### Cohort owner contract

1. `workflow_family`, `risk_class`, and `stack` are sourced from explicit workflow metadata fields and frozen on `Run` at creation time.
2. `project_key` is sourced from `Idea.project_key` and frozen on `Run`, with deterministic `"untagged"` fallback.
3. Steward never recomputes cohort fields from mutable current YAML or workspace state.
4. Completed runs missing any frozen cohort / snapshot field are treated as `legacy_pre_p049` and excluded from deterministic cohorting.
5. `startRun` and `runs.start` require `workflow_yaml_path` and `agent_catalog_yaml_path` and do not emit runs that cannot freeze snapshot truth.
6. The `project_key` owner chain is complete at ingress and persistence time: domain model, DB schema, repo round-trip, and `ideas.create` all carry the same field.
7. The primary cohort key is explicitly `(workflow_family, risk_class)`; `project_key` and `stack` affect quality grading and diagnostics only.

### Frozen snapshot provenance

8. `workflow_snapshot_hash/json` and `catalog_snapshot_hash/json` are produced by named Rust owners at run creation time, not left implicit.
9. The producer bridge is explicit: `workflow::compiler` builds the frozen snapshot payloads, `DefinitionHasher` computes canonical hashes from the parsed frozen definitions, `engine::command_handler::StartRun` persists them onto `Run`, and `db::repos::runs` round-trips them durably.
10. Steward never rebuilds frozen snapshot provenance from mutable YAML paths during analysis, and persisted analysis identity uses separate `workflow_snapshot_artifact_hash` and `agent_catalog_snapshot_hash` fields rather than a collapsed workflow/catalog scalar.

### Current-input bootstrap semantics

11. The daemon validates the current steward config at bootstrap through a named Rust validation owner and does not leave invalid-config behavior implicit.
12. Invalid steward config falls back to `StewardConfig::default_config()`-equivalent runtime semantics rather than breaking trigger scheduling or analysis continuity.
13. Config-change hashing uses the effective validated-or-default steward config that the daemon actually runs with.

### Deterministic pipeline

14. The deterministic analysis record persists even when optional steward-agent lanes are absent or fail.
15. Running the deterministic slice twice on unchanged data yields byte-identical deterministic JSON artifacts.
16. Fewer than `minimum_window_size` runs in either window yields `status = "inconclusive"` with no false-positive signals.
17. No-signal analyses still persist bounded context dossiers.

### Active-catalog steward parity

18. `system_steward` consumes the active input set and may emit all five active outputs.
19. `agent_catalog_snapshot`, `workflow_snapshot`, and `config_change_log` are persisted as analysis-owned artifacts with canonical paths and artifact IDs under the active catalog relative paths rooted at `CHAINWORKS_META_ROOT`.
20. `workflow_snapshot` remains a singular contract input while truthfully representing multiple frozen workflow snapshot hashes when a cohort spans more than one snapshot.
21. `steward_auditor` consumes `sdlc_health_report` and emits `stewardship_audit_report`.
22. Proposal text and `examples/agents/agents.yaml` describe the same steward IO contract, including directory shape for steward inputs, reports, proposals, and audit output.

### Metric-source correctness

23. Steward cost metrics are sourced from session/runtime persistence, not `work_items`.
24. Drift metrics are sourced from persisted run-owned drift fields.
25. Retry reasons are sourced from persisted stage-owned retry fields.
26. Config-change truth hashes the full parsed effective `StewardConfig`, including `context_strategy_profiles`, plus the current `AgentCatalogFile`.

### Northbound readback

27. GraphQL exposes steward analyses and recommendations through named steward queries.
28. MCP exposes manual trigger plus steward analysis list/get tools.
29. `steward-analysis://{analysis_id}` returns the same persisted truth as GraphQL/MCP tool reads.
30. Operators can distinguish `completed`, `inconclusive`, and `failed` analyses from northbound reads.
29. Northbound cohort identity remains the primary tuple only; project/stack information stays in quality/dossier context.

### Trigger semantics

30. Post-run, config-change, and manual triggers all converge on `WorkItemKind::StewardAnalysis`.
31. Config-change startup behavior sets a pending flag only and does not immediately run analysis while claiming V1 parity.

---

## 13. Proof Gate

`cargo test --workspace` is not sufficient proof for this subsystem.

`proposal-049` must instead be a focused composite gate that proves:

1. workflow/idea cohort metadata freezing
2. primary cohort key grouping parity: `(workflow_family, risk_class)` only
3. legacy pre-P049 completed-run exclusion for rows missing frozen cohort/snapshot truth
4. frozen workflow/catalog snapshot production at run creation
5. steward bootstrap validation and default-config fallback semantics
6. deterministic Steward JSON serialization
7. config-change parsed-object hashing and pending-flag semantics
8. active-catalog steward IO parity, including materialized analysis-owned input artifacts
9. northbound GraphQL/MCP analysis readback

Target proof inventory:

```bash
proposal-049|p049)
  log "Proposal 049 control-plane gate: Steward analysis system"
  (
    cd "$ROOT_DIR/control-plane"
    cargo test -p workflow steward_metadata_contract_tests
    cargo test -p workflow run_start_snapshot_contract_tests
    cargo test -p daemon steward_runtime_bootstrap_tests
    cargo test -p engine steward_pipeline_tests
    cargo test -p engine steward_cohort_classifier_tests
    cargo test -p engine steward_legacy_pre_p049_eligibility_tests
    cargo test -p engine steward_trigger_tests
    cargo test -p graphql-server steward_graphql_readback_tests
    cargo test -p mcp-server steward_mcp_tools_tests
  )
  log "Proposal 049 control-plane gate passed"
  ;;
```

The exact test names may evolve, but the proof gate must stay focused on these guarantee buckets rather than collapsing back to a blanket workspace pass.

---

## 14. Out of Scope

- dedicated Steward dashboard UI
- schedule trigger wiring
- V2 recommendation synthesis beyond persisted proposal artifacts
- V3 experiment execution
- live-session introspection outside persisted run/session truth
