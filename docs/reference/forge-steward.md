# Forge Steward -- V1 Reference

Status: **Implemented** (V1 -- Observer mode)

## Purpose

The Steward is a meta-workflow that observes the health of the Chainworks Forge
agent factory. It runs _after_ normal workflow runs complete, comparing recent
execution metrics against a historical baseline to surface degradations in
timing, rework, quality, cost, and stability.

V1 operates as an offline observer. There is no in-app Steward UI. Results are
persisted to SwiftData and written as JSON artifacts to disk.

## V1 Scope

- Deterministic metrics collection from persisted SwiftData run data.
- Arithmetic anomaly detection (observation window vs. baseline window).
- Primary-cohort partitioning so runs with different `(workflowFamily, riskClass)` keys are never compared.
- Optional LLM interpretation via `system_steward` and `steward_auditor` agents from the catalog.
- Trigger mechanisms: manual, post-N-runs, config-change.
- No schedule-based trigger (cron field is parsed but not wired).

## Analysis Pipeline (Steps 1--8)

`StewardAnalysisService.runAnalysis()` executes the full pipeline:

1. **Validate config** -- `YAMLValidator.validateStewardConfig` rejects configs with errors before proceeding.
2. **Query completed runs** -- fetches all `Run` records from SwiftData, filters to `.completed`.
3. **Select primary cohort** -- groups by `(workflowFamily, riskClass)`, selects the largest group. Other groups are excluded.
4. **Split windows** -- `CohortClassifier.splitWindows` divides the cohort into observation and baseline windows by recency, respecting `maximumWindowAgeDays`. If either window has fewer runs than `minimumWindowSize`, the analysis is marked inconclusive.
5. **Classify cohort quality** -- `CohortClassifier.classifyQuality` returns `strong`, `acceptable`, or `weak` based on sample size and tagging completeness.
6. **Collect metrics** -- `MetricsCollector.collectMetrics` computes a `MetricsSnapshot` for each window.
7. **Detect anomalies** -- `AnomalyDetector.detect` compares observation vs. baseline using configured thresholds. Refuses to produce findings when sample size is below minimum.
8. **Build dossiers** -- `RunDossierBuilder` assembles detailed evidence for implicated runs (or up to 5 observation runs when no runs are implicated).
9. **Write artifacts** -- deterministic JSON files are written to the workspace directory under `Application Support/Chainworks Forge/steward/analyses/<analysisID>/`.
10. **LLM agents (optional)** -- if `system_steward` and `steward_auditor` agents exist in the catalog, they are executed to produce `health-report.json` and `audit-report.json`.
11. **Persist** -- a `StewardAnalysis` record is inserted along with `StewardAnalysisRunLink` records (role: `implicated`, `baseline`, or `context`) and `StewardRecommendation` records for each degradation signal.

## Metrics Families

`MetricsSnapshot` covers five families, each backed by a configurable threshold:

| Family      | Key Metrics                                           | Default Threshold       |
|-------------|-------------------------------------------------------|-------------------------|
| `timing`    | `leadTimeMedianSeconds`, stage latencies, approval wait | 30% median increase   |
| `rework`    | `proposalLoopMean`, `implementationLoopMean`, retries  | 50% mean increase      |
| `quality`   | `approvalRejectionRate`, `auditPassRate`               | 2x ratio               |
| `cost`      | `costPerRunMedianCents`, cost by stage                 | 25% median increase    |
| `stability` | `failedRunRate`, `blockedRunRate`, drift events        | 2x ratio               |

## Anomaly Detection

`AnomalyDetector` is fully deterministic (no LLM). It supports three threshold methods:

- `median_percentage` -- fires when `(observed - baseline) / baseline >= trigger`.
- `mean_percentage` -- same formula applied to mean values.
- `ratio` -- fires when `observed / baseline >= trigger`.

Each signal is emitted as a `DegradationSignal` with severity (`high` >= 100% delta, `medium` >= 50%, `low` otherwise) and confidence capped by cohort quality.

## Cohorting

`CohortClassifier` enforces the rule that runs with different primary keys `(workflowFamily, riskClass)` are never compared. Quality classification:

- **strong** -- 10+ runs, no untagged projects, no unknown stacks.
- **acceptable** -- 5--9 runs, or has unknown stacks.
- **weak** -- fewer than 5 runs, or has untagged projects.

Confidence levels map directly: strong = high, acceptable = medium, weak = low.

## Trigger Mechanisms

Triggers are wired in `ExecutionService`:

### Manual
`ExecutionService.runStewardAnalysis()` runs the pipeline on demand.

### Post-run hook
`ExecutionService.notifyRunCompleted()` increments a counter after each completed run. When `completedRunsSinceLastAnalysis >= config.triggers.postRunHook.runInterval` and the trigger is enabled, analysis fires automatically and the counter resets.

### Config-change
`ExecutionService.checkForConfigChange()` runs at app launch. It hashes the current `StewardConfig` and `AgentCatalog` using `DefinitionHasher` and compares against the most recent `StewardAnalysis` record. On mismatch, it sets `configChangeAnalysisScheduled = true`; the next `notifyRunCompleted()` call runs analysis immediately (config-change takes priority over the post-run counter).

### Schedule
The `schedule` trigger config is parsed (`cron` field) but not wired to a scheduler in V1.

## Configuration

`steward_config.yaml` is loaded at app bootstrap from either the app bundle or `examples/steward/steward_config.yaml`. Validation runs at load time; on error the `StewardConfig.defaultConfig` is used.

```yaml
schema_version: 1
windows:
  observation_window_size: 20
  baseline_window_size: 20
  minimum_window_size: 5
  maximum_window_age_days: 90
thresholds:
  timing:   { method: median_percentage, trigger: 0.30 }
  rework:   { method: mean_percentage,   trigger: 0.50 }
  quality:  { method: ratio,             trigger: 2.0  }
  cost:     { method: median_percentage, trigger: 0.25 }
  stability:{ method: ratio,             trigger: 2.0  }
triggers:
  post_run_hook: { enabled: false, run_interval: 5 }
  on_config_change: { enabled: true }
  schedule: { enabled: false, cron: "0 8 * * 1" }
```

Validation rules (enforced by `YAMLValidator.validateStewardConfig`):
- `schema_version` must be `1`.
- All window sizes must be positive; `minimum_window_size <= observation_window_size`.
- All five threshold families (`timing`, `rework`, `quality`, `cost`, `stability`) are required.
- Threshold methods must be one of `median_percentage`, `mean_percentage`, `ratio`.
- `post_run_hook.run_interval >= 1` when enabled.

## Persistence Model

Five SwiftData `@Model` classes registered in `Chainworks_ForgeApp`:

| Model                       | Purpose                                                |
|-----------------------------|--------------------------------------------------------|
| `StewardAnalysis`           | One record per analysis run. Tracks window bounds, cohort quality, snapshot hashes, artifact paths, degradation count, and status (`completed`, `inconclusive`, `superseded`). |
| `StewardAnalysisRunLink`    | Join table linking an analysis to its runs with a role (`implicated`, `baseline`, `context`). |
| `StewardRecommendation`     | One per degradation signal. Category, target metric, confidence, and lifecycle status (`proposed` through `rolledBack`). Links to optional `StewardExperiment`. |
| `StewardExperiment`         | V3 placeholder. Schema is defined but unused in V1.    |
| `StewardDecision`           | V3 placeholder. Schema is defined but unused in V1.    |

## Workspace Artifacts

Each analysis writes to `Application Support/Chainworks Forge/steward/analyses/<analysisID>/`:

```
metrics-window.json          # MetricsSnapshot for the observation window
baseline-window.json         # MetricsSnapshot for the baseline window
dossiers/<runID>.json        # RunDossier per implicated run
degradation-alerts.json      # DegradationSignal array (only if signals exist)
health-report.json           # system_steward agent output (optional)
audit-report.json            # steward_auditor agent output (optional)
```

## Source File Index

| File | Role |
|------|------|
| `Chainworks Forge/Engine/Steward/StewardAnalysisService.swift` | Pipeline orchestrator (steps 1--8) |
| `Chainworks Forge/Engine/Steward/MetricsCollector.swift` | Deterministic metrics from SwiftData runs |
| `Chainworks Forge/Engine/Steward/AnomalyDetector.swift` | Threshold-based degradation detection |
| `Chainworks Forge/Engine/Steward/CohortClassifier.swift` | Window splitting and cohort quality |
| `Chainworks Forge/Engine/Steward/RunDossierBuilder.swift` | Evidence dossiers for implicated runs |
| `Chainworks Forge/Engine/Steward/DegradationSignal.swift` | Signal value type |
| `Chainworks Forge/DSL/StewardConfig.swift` | Config structs and defaults |
| `Chainworks Forge/DSL/YAMLParser.swift` | `loadStewardConfig(from:)` |
| `Chainworks Forge/DSL/YAMLValidator.swift` | `validateStewardConfig(_:)` |
| `Chainworks Forge/Models/StewardAnalysis.swift` | SwiftData model + `AnalysisStatus`, `CohortQuality` |
| `Chainworks Forge/Models/StewardAnalysisRunLink.swift` | SwiftData model + `RunRole` |
| `Chainworks Forge/Models/StewardRecommendation.swift` | SwiftData model + enums |
| `Chainworks Forge/Models/StewardExperiment.swift` | V3 placeholder model |
| `Chainworks Forge/Models/StewardDecision.swift` | V3 placeholder model |
| `Chainworks Forge/Engine/ExecutionService.swift` | Trigger wiring (`notifyRunCompleted`, `checkForConfigChange`, `runStewardAnalysis`) |
| `Chainworks Forge/Chainworks_ForgeApp.swift` | `loadStewardConfig()` at bootstrap |
| `examples/steward/steward_config.yaml` | Default configuration file |
