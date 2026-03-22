# Proposal 003: Forge Steward — SDLC Health, Degradation Detection, and Controlled Adaptation

**Date:** 2026-03-22  
**Status:** Draft  
**Depends on:** Proposal 001 (Foundation Domain Model + YAML DSL Parser), Proposal 002 (Workflow Execution Engine)
**Primary role:** Meta-workflow and system agent for observing the health of the agent factory itself
**Prerequisite gate:** V1 (Observer) is deliberately offline and operates only over persisted run data. Operator-facing Steward screens (dossier browser, health dashboard, approval inbox) are **not** in scope until Proposal 002's execution UI and reporting surfaces are shipped and available in the app. Until that gate is met, Steward produces file-based reports and indexed artifacts only.

---

## 1. Why this exists

Chainworks has the right lower-level **domain model and engine primitives** to execute and record work:
- immutable run snapshots
- stage and agent executions
- artifacts with provenance
- approvals and manual gates
- cost tracking
- an app-scoped execution service

However, the current app shell still exposes only the scaffold baseline (Ideas list, Agent Catalog, Workflow Inspector). The operator-facing execution UI — current stage view, active agent inspection, approval inbox, artifact browser, and completed-run reports — is defined in Proposal 002 but not yet shipped. This means Steward V1 cannot assume that the engineer can directly inspect the execution surfaces it wants to optimize.

**Therefore, V1 is scoped as an offline observer** that reads persisted run data, produces file-based reports and indexed artifacts, and does not include operator-facing Steward UI. Steward screens become eligible only after Proposal 002's execution/reporting UI is live.

What is still missing is the layer that watches the **system across many runs** and answers questions like:
- Why did proposal loops suddenly get longer?
- Why did security findings spike after changing a backend profile?
- Why are approvals being rejected more often this week?
- Did the new workflow ordering actually help, or did it only make reports look cleaner?

The lead agent is responsible for getting **one run** across the line.
Forge Steward is responsible for watching the **whole forge** and noticing when the system starts to drift, slow down, waste money, or produce lower-quality outcomes.

This is not "another worker agent" inside the normal proposal-to-release chain.
It is a **system agent** and a **meta-workflow** that operates on windows of completed runs, blocked runs, approval comments, artifacts, and execution traces.

---

## 2. Core idea

> Forge Steward is the system-level observer and advisor for Chainworks.
> It measures SDLC health across runs, detects degradations, builds evidence dossiers,
> performs retrospectives, and proposes changes to agents or workflows.
>
> It does **not** silently rewrite the system in production.

The practical split is important:
- the normal workflow produces features
- Forge Steward evaluates how well the workflow itself is performing

If the main workflow is the assembly line, Forge Steward is the person who notices that the line now jams every third batch, the screws are stripped more often, and the team has started compensating with awkward manual workarounds.

---

## 3. Position in the architecture

Forge Steward should live **outside** the canonical proposal-to-release workflow.
Do not make it state 13 or agent 14 inside the normal run.
That would mix local delivery concerns with system governance.

Instead, implement it as a separate **meta-workflow** that is triggered by one or more of the following:
- schedule (daily / weekly)
- every N completed runs
- every config change to `agents.yaml`, `workflow.yaml`, or `steward_config.yaml`
- anomaly detection trigger
- explicit manual request by the engineer

### Recommended identity

**Product name:** Forge Steward  
**Internal ID:** `system_steward`

Optional supporting agents:
- `steward_auditor` — independent challenger that tests Steward’s claims
- `agent_retrospective_interviewer` — reconstructs why a specific implicated agent behaved the way it did, using dossiers rather than memory

---

## 4. What Forge Steward should actually do

### 4.1 Measure system health

First, do not ask an LLM to guess metrics.
Compute the metrics deterministically in code.

Candidate metric families:

#### Throughput and timing
- idea → proposal draft lead time
- idea → release lead time
- stage latency by state
- time spent waiting at approval gates
- time lost to blocked runs and resumes

#### Rework and churn
- proposal refinement loop count
- implementation refinement loop count
- retries per stage
- number of runs that re-enter the same stage family

#### Quality and gate health
- proposal review aggregate score trend
- implementation audit pass/fail rate
- security findings by severity
- pre-push review failures
- documentation drift or docs-quality warnings
- approval rejection rate

#### Cost and efficiency
- cost per run
- cost per successful release
- cost by backend / model / effort
- cost per stage family
- cost increase after config changes

#### Human pain signals
- number of manual interventions
- number of approval comments containing dissatisfaction / rewrite requests
- blocked runs that required rescue
- runs cancelled late in the pipeline

#### Stability signals
- failed runs
- blocked runs
- drift events
- resumed runs
- side-effect stages that required manual recovery

### 4.2 Detect degradations

Use simple, transparent heuristics before doing anything fancy.
Examples:
- proposal loop average increased from 1.4 to 3.1 over the last 20 comparable runs
- approval rejection rate doubled after a change to proposal reviewers
- security findings per implementation run increased after swapping the code writer backend
- release latency rose 35% while cost also rose, with no improvement in final acceptance rate

Do this in a deterministic detector first.
An LLM should explain and reason about patterns, not be the first place where the numbers are invented.

### 4.3 Build run dossiers

When a degradation is detected, collect a dossier for the most relevant runs.
A dossier should include:
- run metadata
- workflow snapshot hash
- agent catalog snapshot hash
- stage/agent execution summaries
- consumed inputs and produced artifacts
- approval history and comments
- cost breakdown
- failures / retries / resume events
- transcript and tool traces where available

This matters because the agent cannot reason well about a degradation if you give it only a dashboard headline.
It needs the raw trail of what actually happened.

### 4.4 Form hypotheses

Forge Steward should answer:
- what degraded
- since when it degraded
- what likely changed around that time
- which agents or workflow stages are implicated
- what alternative explanations exist
- what action is safest to test next

The output should not be vague philosophy.
It should be something like:

> Proposal loop count rose after moving UI review later in the sequence. Review artifacts show UI concerns are now discovered after architecture stabilizes, causing churn in state 4 → 5. Recommendation: move UI reviewer earlier or tighten its input contract so style/interaction risks surface before architecture review completes.

### 4.5 Talk to agents, but carefully

Your original idea — letting it talk to the agents that did the work — is useful, but only if done as a **retrospective protocol**, not as faith in the agent’s memory.

A good retrospective interview is based on a dossier:
- exact task
- exact inputs
- exact outputs
- review feedback
- final outcome

Without a dossier, the agent will often produce a neat explanation that sounds plausible and is only loosely attached to what really happened.

### 4.6 Produce change proposals

Forge Steward should emit artifacts like:
- `sdlc_health_report_v1`
- `degradation_alert_v1`
- `agent_tuning_proposal_v1`
- `workflow_tuning_proposal_v1`
- `experiment_plan_v1`
- `post_experiment_evaluation_v1`

These are proposals, not direct mutations.
The system should remain under human control.

---

## 5. Recommended implementation shape

### 5.1 Split deterministic and agentic parts

The clean version looks like this:

1. **Metrics collector** — V1
   - deterministic code
   - computes metrics from runs, stages, agent executions, artifacts, approvals

2. **Anomaly detector** — V1
   - deterministic code
   - flags suspicious shifts using baselines and configurable thresholds (see section 6, threshold configuration)

3. **Dossier builder** — V1
   - deterministic code
   - bundles evidence for implicated runs, agents, and stages

4. **Forge Steward** — V1
   - LLM-backed system agent
   - explains patterns, proposes likely causes, recommends changes

5. **Steward Auditor** — V1
   - separate LLM-backed critic
   - challenges Steward’s hypothesis and looks for weaker evidence or alternate explanations

6. **Experiment manager** — V3
   - deterministic control plane
   - runs controlled rollout or champion/challenger tests after human approval
   - not implemented until V3; `StewardExperiment` and `StewardDecision` entities are defined but unused until then

### 5.2 Why this split matters

If Steward both detects anomalies and interprets them with no deterministic layer underneath, you will eventually end up reading elegant nonsense.

The numbers must be real first.
Then the model can help connect them to behavior.

---

## 6. Data model additions recommended before implementing Steward

Proposal 001 and Proposal 002 already give you a strong base, but Steward will want more detail than a normal run viewer needs.

Recommended additions:

### On `AgentExecution`
- `consumedArtifactIDs` or a dedicated `AgentExecutionInput` relation
- `agentConfigHash`
- `skillSnapshotHash`
- `skillSnapshotBundlePath` or equivalent
- `transcriptPath`
- `toolTracePath`
- `retryReason`

### On `Run`
- tags: project, repo, workflow family, risk class, stack (see cohorting contract below)
- rollout / experiment cohort information
- post-release outcome tags when available

### Cohorting contract for V1

Fair cross-run comparison requires deterministic cohorting. Without strict rules, the anomaly detector can compare non-comparable runs and produce confident noise. The following contract is mandatory for V1.

#### Required run metadata fields (mandatory at run start)

| Field | Type | Source | Allowed values | Fallback if missing |
|---|---|---|---|---|
| `workflowFamily` | `String` | Derived from `workflow.yaml` `id` prefix (e.g., `proposal_to_release`) | Must match a registered workflow family in the catalog | Run is tagged `cohort_excluded` and excluded from Steward analysis |
| `projectKey` | `String` | Set by engineer when creating the idea, or inherited from repo | Non-empty string, unique per logical project | `"untagged"` — included in analysis but cohort quality is downgraded to `.weak` |
| `riskClass` | `RiskClass` | Set by engineer or derived from workflow risk annotations | `.standard`, `.elevated`, `.critical` | `.standard` (default) |
| `stack` | `String` | Derived from repo analysis or set by engineer | Free-form but normalized to lowercase, e.g., `"swift"`, `"typescript"` | `"unknown"` — cohort quality downgraded to `.acceptable` |

#### Optional enrichment fields

| Field | Type | Notes |
|---|---|---|
| `repoIdentifier` | `String?` | Org/repo or local path hash |
| `ideaComplexityEstimate` | `ComplexityEstimate?` | `.trivial`, `.small`, `.medium`, `.large` — if provided, used as secondary grouping |
| `experimentCohortID` | `UUID?` | Set only when run is part of a Steward experiment |

#### Grouping rules

1. **Primary grouping key**: `(workflowFamily, riskClass)`. All Steward metric comparisons use this as the minimum partition. Runs with different primary keys are **never** compared directly.
2. **Secondary grouping key** (when sample size permits): `(workflowFamily, riskClass, projectKey)`. Used for project-specific trend detection.
3. **Tertiary refinement** (optional): `stack` and `ideaComplexityEstimate` are used only when cohort size ≥ 10 and only for within-primary-group sub-analysis.

#### Cohort quality classification

| Quality | Condition | Effect on Steward |
|---|---|---|
| `.strong` | All required fields present, no `"untagged"` or `"unknown"` values, sample size ≥ 10 | Full analysis, normal confidence |
| `.acceptable` | All required fields present but `stack = "unknown"` or sample size 5–9 | Analysis proceeds, confidence capped at `.medium` |
| `.weak` | `projectKey = "untagged"` or sample size < 5 | Analysis proceeds with `.low` confidence; recommendations marked as `low_confidence` and **must not** enter the experiment path without manual override |

#### Confidence downgrade rules

- Any analysis over a `.weak` cohort must include a `[WEAK COHORT]` prefix in the report summary.
- Recommendations from `.weak` cohorts are emitted as informational observations, not actionable proposals.
- The anomaly detector must refuse to flag a degradation when sample size < 3; it logs a `sample_too_small` event instead.

### Observation window configuration for V1

The metrics collector and anomaly detector require well-defined window parameters. Without explicit defaults, the system cannot produce its first analysis.

#### Window sizing

| Parameter | Default | Rationale |
|---|---|---|
| `observationWindowSize` | Last 20 completed runs within the primary cohort | Enough for simple trend detection; not so many that slow drifts are averaged out |
| `baselineWindowSize` | Previous 20 completed runs before the observation window | Equal-sized comparison for fair before/after analysis |
| `minimumWindowSize` | 5 completed runs | Below this, the anomaly detector refuses to produce findings (logs `sample_too_small`) |
| `maximumWindowAgeDays` | 90 | Runs older than this are excluded from windows to avoid stale comparisons |

#### Baseline selection rules

1. **Default**: the immediately preceding window of equal size to the observation window.
2. **After config change**: the last full window before the config change timestamp becomes the baseline, regardless of window position. This allows before/after comparisons for agent or workflow changes.
3. **Manual override**: the engineer may specify a baseline window by date range or run ID range.
4. **Non-overlapping**: observation and baseline windows must never overlap. If they would overlap due to insufficient completed runs, the analysis is marked `.inconclusive`.

#### Anomaly detector threshold configuration

Thresholds must be configurable per metric family. V1 ships with sensible defaults that can be overridden.

| Metric family | Default threshold | Detection method |
|---|---|---|
| Timing (lead time, stage latency) | > 30% increase in median vs baseline | Median comparison with percentage delta |
| Rework (loop count, retries) | > 50% increase in mean vs baseline | Mean comparison with percentage delta |
| Quality (rejection rate, finding count) | > 2× baseline rate | Ratio comparison |
| Cost (cost per run, cost per stage) | > 25% increase in median vs baseline | Median comparison with percentage delta |
| Stability (failure rate, blocked rate) | > 2× baseline rate | Ratio comparison |

### `steward_config.yaml` — typed configuration surface

`steward_config.yaml` is a **first-class configuration input** for Steward, treated with the same discipline as `agents.yaml` and `workflow.yaml`:

- **Typed model**: a `StewardConfig` Swift struct loaded by `YAMLParser.loadStewardConfig()`, following the same pattern as `YAMLParser.loadAgentCatalog()` and `YAMLParser.loadWorkflow()`.
- **Validation**: `YAMLValidator.validateStewardConfig()` checks threshold entries (valid method + positive trigger value), window parameters (positive sizes, minimumWindowSize ≤ observationWindowSize), and trigger config (valid runInterval ≥ 1 if enabled).
- **Provenance hashing**: `DefinitionHasher.hash(stewardConfig:)` produces a `stewardConfigSnapshotHash` that is recorded on every `StewardAnalysis` (see updated `StewardAnalysis` fields below).
- **Config-change detection**: the trigger mechanism compares `stewardConfigSnapshotHash` alongside workflow and catalog hashes. A change to any of the three triggers a new analysis.

Full schema:

```yaml
schema_version: 1

windows:
  observation_window_size: 20          # completed runs in the primary cohort
  baseline_window_size: 20             # completed runs for comparison
  minimum_window_size: 5               # below this → sample_too_small
  maximum_window_age_days: 90          # exclude runs older than this

thresholds:
  timing:
    method: median_percentage
    trigger: 0.30
  rework:
    method: mean_percentage
    trigger: 0.50
  quality:
    method: ratio
    trigger: 2.0
  cost:
    method: median_percentage
    trigger: 0.25
  stability:
    method: ratio
    trigger: 2.0

triggers:
  post_run_hook:
    enabled: false
    run_interval: 5                    # every Nth completed run (ignored if enabled=false)
  on_config_change:
    enabled: true                      # schedule analysis when workflow/catalog/steward config hash changes
  schedule:
    enabled: false                     # V2 scope — requires background execution infrastructure
    cron: "0 8 * * 1"                  # illustrative, unused in V1
```

When a metric crosses its threshold, the anomaly detector emits a `DegradationSignal` with the metric name, observed value, baseline value, delta, and threshold used. This signal drives dossier construction.

#### `StewardConfig` typed model

```swift
struct StewardConfig: Codable, Hashable {
    let schemaVersion: Int
    let windows: WindowConfig
    let thresholds: [String: ThresholdEntry]
    let triggers: TriggerConfig
}

struct WindowConfig: Codable, Hashable {
    let observationWindowSize: Int
    let baselineWindowSize: Int
    let minimumWindowSize: Int
    let maximumWindowAgeDays: Int
}

struct ThresholdEntry: Codable, Hashable {
    let method: String          // "median_percentage", "mean_percentage", "ratio"
    let trigger: Double         // e.g., 0.30 for 30%, 2.0 for 2×
}

struct TriggerConfig: Codable, Hashable {
    let postRunHook: PostRunHookConfig
    let onConfigChange: OnConfigChangeConfig
    let schedule: ScheduleConfig
}
```

### V1 trigger mechanism

Since V1 has no in-app Steward UI, the meta-workflow must be triggerable without the app's presentation layer. Supported trigger modes for V1:

1. **Manual via `ExecutionService`** — call `executionService.runStewardAnalysis()` programmatically. This is the primary integration point and can be exposed as a menu item ("Run Steward Analysis") without building full Steward screens.
2. **Post-run hook** — `ExecutionService` optionally triggers a Steward analysis after every Nth completed run (configurable via `steward_config.yaml` `triggers.post_run_hook`, default: disabled). When enabled, the analysis runs asynchronously after run completion.
3. **On config change** — when `agents.yaml`, `workflow.yaml`, or `steward_config.yaml` is reloaded and any of the three hashes differs from the last analysis's provenance record, Steward schedules an analysis after the next completed run.

Schedule-based triggers (daily/weekly) are V2 scope and require background execution infrastructure not present in V1.

### On artifacts / reporting
- artifact lineage edges
- normalized artifact classes (proposal, review, audit, release, report)
- confidence / source summary for derived reports

Without these, Steward will be trying to reconstruct cause from smoke stains on the wall.

### Steward-domain persistence model

Steward outputs must be first-class persisted entities, not loose artifacts. The following domain model is required before implementation begins so that the full chain — analysis → recommendation → approval → rollout → outcome — is auditable and queryable.

#### `StewardAnalysis`
Represents one complete observation-window analysis.
- `id: UUID`
- `createdAt: Date`
- `windowStart: Date`
- `windowEnd: Date`
- `runCount: Int` — number of runs in the window
- `cohortKeys: [String: String]` — the cohorting dimensions used (see cohorting contract in section 6)
- `cohortQuality: CohortQuality` — `.strong`, `.acceptable`, `.weak`
- `metricsSnapshotPath: String` — path to the deterministic metrics JSON
- `baselineSnapshotPath: String` — path to the comparison baseline metrics JSON
- `degradationsDetected: Int`
- `reportArtifactPath: String` — path to the generated `sdlc_health_report` artifact
- `auditArtifactPath: String?` — path to the Steward Auditor challenge report, if produced
- `status: AnalysisStatus` — `.completed`, `.inconclusive`, `.superseded`
- `workflowCatalogSnapshotHash: String` — hash of workflow + catalog config at analysis time
- `stewardConfigSnapshotHash: String` — hash of `steward_config.yaml` at analysis time (ensures reproducibility across threshold/window changes)
- Relationship: `recommendations: [StewardRecommendation]`
- Relationship: `implicated runs` via `analysisRunLinks: [StewardAnalysisRunLink]` (analysis ↔ run join)

#### `StewardAnalysisRunLink`
Join entity linking an analysis to the runs it examined (many-to-many).
- `id: UUID`
- `analysisID: UUID`
- `runID: UUID`
- `role: RunRole` — `.implicated` (flagged by anomaly detector), `.baseline` (used for comparison), `.context` (included for completeness)
- Relationship: `analysis: StewardAnalysis`
- Relationship: `run: Run`

#### `StewardRecommendation`
Represents one concrete change proposal emitted by Steward.
- `id: UUID`
- `createdAt: Date`
- `category: RecommendationCategory` — `.agentTuning`, `.workflowTuning`, `.backendChange`, `.inputContractChange`, `.other`
- `summary: String` — one-sentence human-readable description
- `targetMetric: String` — which metric should improve if hypothesis is correct
- `proposedPatchPath: String?` — path to the proposed YAML diff, if applicable
- `confidenceLevel: ConfidenceLevel` — `.high`, `.medium`, `.low`
- `status: RecommendationStatus` — `.proposed`, `.approved`, `.rejected`, `.superseded`, `.adoptedAfterExperiment`, `.rolledBack`
- `decisionComment: String?` — human rationale for approval/rejection
- `decidedAt: Date?`
- Relationship: `analysis: StewardAnalysis`
- Relationship: `experiment: StewardExperiment?`

#### `StewardExperiment`
Represents one controlled rollout or champion/challenger test.
- `id: UUID`
- `createdAt: Date`
- `startedAt: Date?`
- `completedAt: Date?`
- `experimentType: ExperimentType` — `.championChallenger`, `.limitedRollout`, `.abTest`
- `controlConfigHash: String` — config hash for the control (baseline) arm
- `treatmentConfigHash: String` — config hash for the treatment arm
- `minimumSampleSize: Int`
- `actualSampleSize: Int`
- `rollbackCondition: String` — explicit human-readable rollback trigger
- `status: ExperimentStatus` — `.planned`, `.running`, `.completed`, `.rolledBack`, `.cancelled`
- `evaluationArtifactPath: String?` — path to `post_experiment_evaluation` artifact
- Relationship: `recommendation: StewardRecommendation`
- Relationship: `decision: StewardDecision?`

#### `StewardDecision`
Represents the final adoption or rollback decision after an experiment completes.
- `id: UUID`
- `decidedAt: Date`
- `outcome: DecisionOutcome` — `.adopted`, `.rolledBack`, `.iterateWithNewExperiment`, `.deferred`
- `rationale: String`
- `adoptedConfigHash: String?` — the config hash that was adopted, if applicable
- `rollbackConfigHash: String?` — the config hash reverted to, if rolled back
- Relationship: `experiment: StewardExperiment`

#### Persistence strategy for V1

In V1 (offline observer), only `StewardAnalysis`, `StewardAnalysisRunLink`, and `StewardRecommendation` are active. `StewardExperiment` and `StewardDecision` are defined but unused until V3. All entities are persisted in SwiftData alongside the existing run/stage/artifact models. Artifact content (reports, patches, evaluation docs) lives on disk; SwiftData stores metadata, paths, and relationship links.

#### SwiftData migration strategy

This proposal adds:
- 5 new entities (`StewardAnalysis`, `StewardAnalysisRunLink`, `StewardRecommendation`, `StewardExperiment`, `StewardDecision`) — **additive, no migration conflict** with existing schema
- New optional fields on `Run` (`workflowFamily`, `projectKey`, `riskClass`, `stack`, `experimentCohortID`) — **lightweight migration** (new optional properties with defaults)
- New optional fields on `AgentExecution` (`agentConfigHash`, `skillSnapshotHash`, `transcriptPath`, `toolTracePath`, `retryReason`) — **lightweight migration** (new optional properties)

All new fields on existing models are optional with `nil` defaults, so SwiftData lightweight migration applies. No versioned `SchemaMigrationPlan` is required unless a future version makes any of these fields non-optional. The implementation must register the new entity types in the `ModelContainer` schema array.

---

## 7. Meta-workflow for Forge Steward

Recommended workflow, annotated by maturity level:

#### V1 — Observer (offline)

1. Collect metrics for the observation window
2. Compare to baseline / prior window
3. Detect degradations or suspicious shifts
4. Build dossiers for implicated runs
5. Ask Forge Steward for analysis and hypotheses
6. Ask Steward Auditor to challenge those hypotheses
7. Produce recommendations as file-based report artifacts
8. Persist `StewardAnalysis` and `StewardRecommendation` records

V1 stops here. Recommendations are informational; no automatic action is taken.

#### V2 — Advisor (adds steps 9–10)

9. Require human approval on selected recommendations
10. Generate concrete YAML patch proposal for `agents.yaml` and/or `workflow.yaml`

#### V3 — Experimenter (adds steps 11–14)

11. Run limited rollout / experiment after human approval
12. Compare outcomes against baseline
13. Produce `StewardDecision` (adopt, iterate, or roll back)
14. Apply or revert config changes

---

## 8. Maturity model

### V1 — Observer (offline)
Read-only. **No operator-facing Steward UI.**
Operates exclusively over persisted run data (SwiftData + artifact store).
Produces file-based reports and indexed artifacts.
No automatic config changes, no in-app Steward screens.
Steward UI is gated on Proposal 002's execution/reporting surfaces being shipped.

### V2 — Advisor
Produces concrete YAML patch proposals or PRs for agent/workflow changes.
Still requires human approval before adoption.

### V3 — Experimenter
Can run controlled experiments after approval:
- champion/challenger agent profile
- alternate workflow ordering
- different backend or effort profile for one role

### What not to do early
Do **not** let Steward directly rewrite prompts or workflow order in live production without approval.
That is how you wake up to a cleaner dashboard and a worse system.

---

## 9. Main risks with this approach

### 9.1 Goodhart’s law
If you optimize the easiest metric, the system will learn to look better instead of getting better.

Examples:
- fewer loops, but worse proposal quality
- lower cost, but more manual rescue work
- faster release, but more regressions afterward

### 9.2 Attribution error
A degradation is rarely caused by one agent alone.
It may be caused by:
- backend change
- effort change
- workflow order change
- task mix shift
- project complexity shift
- bad upstream inputs

Steward must distinguish correlation from likely cause.

### 9.3 Post-hoc rationalization
If you ask an implicated agent why it failed, it may give a polished answer that sounds good and explains little.
That is why dossier-based retrospective mode and an independent auditor are valuable.

### 9.4 Configuration thrash
If Steward is allowed to tweak too often, the baseline never stabilizes.
You end up comparing noise against noise.

Recommendation:
- freeze period after config changes
- minimum sample size before judgment
- explicit rollback criteria

### 9.5 Missing external outcome signals
If Steward sees only internal SDLC signals, it may optimize for internal smoothness rather than actual product success.
Longer term, feed in:
- bugs found after release
- incidents
- issue reopen rate
- support pain
- manual rework after distribution

### 9.6 Cost and privacy
Retrospective analysis can become expensive quickly if it rehydrates too much context, especially transcripts and tool traces.
Budget caps and evidence windows matter.

---

## 10. Concrete recommendations

1. Start with **V1 Observer** only.
2. Build deterministic metrics first.
3. Add dossier generation before retrospective interviews.
4. Keep Steward read-only until you trust the evidence quality.
5. Use an independent auditor before presenting recommendations.
6. Ship controlled experiments before any self-modifying behavior.
7. Track external outcomes as soon as practical.

If you do this in the wrong order, the agent will start making confident optimization proposals on top of weak evidence.
It feels smart for two days, then you spend a week pulling weeds.

---

## 11. Suggested agent definitions

> **⚠ Not yet loadable — companion catalog changes required first.**
> The definitions below show the recommended shape of Steward agents. They are structurally valid against the current `AgentDefinition` contract, but reference backend profiles, skill refs, artifact paths, and output contracts that do not yet exist in `agents.yaml`.
> Section 11.4 provides a **complete, repo-verified checklist** of every addition needed to make these entries loadable and pass `YAMLValidator` validation.
> All provider choices stay within the MVP-supported scope (Codex, Claude Code) unless the PS is updated.

### Output contract binding strategy

The current `AgentDefinition` schema supports one optional `output_contract` per agent. Steward agents produce multiple output artifacts, but only **one primary structured output** per agent is LLM-generated and requires runtime contract validation. The other outputs are either deterministic side products (generated by code before the agent runs) or derivative artifacts (produced by post-processing the primary output).

| Agent | Primary output (contract-bound) | Other outputs (not contract-bound) | Why |
|---|---|---|---|
| `system_steward` | `sdlc_health_report` → `sdlc_health_report_v1` | `degradation_alert`, `agent_tuning_proposal`, `workflow_tuning_proposal`, `experiment_plan` | `degradation_alert` is produced by the deterministic anomaly detector, not by the LLM. Tuning proposals and experiment plan are derived sections extracted from the health report by post-processing code. |
| `steward_auditor` | `stewardship_audit_report` → `stewardship_audit_report_v1` | — | Single output, no ambiguity. |
| `agent_retrospective_interviewer` | `agent_retrospective_report` → `agent_retrospective_report_v1` | — | Single output, no ambiguity. |

This means no schema extension is needed. The existing single `output_contract` field is sufficient because each agent has exactly one LLM-generated primary output. The other artifacts follow their contract shapes by construction (deterministic code) and are validated by unit tests rather than by the runtime contract validator.

### 11.1 Forge Steward

```yaml
id: system_steward
title: Forge Steward
mode: meta_analysis
backend_profile: claude_steward_high  # illustrative — profile does not exist yet; needs to be added to agents.yaml backend_profiles
permission_profile: RO_VERIFY
skill_ref: steward_core  # illustrative — skill does not exist yet
skill_role: sdlc_health
output_contract: sdlc_health_report_v1  # primary structured output — validated at runtime
inputs:
  - metrics_window
  - baseline_window
  - implicated_run_dossiers
  - agent_catalog_snapshot
  - workflow_snapshot
  - config_change_log
outputs:
  - sdlc_health_report          # primary — LLM-generated, contract-validated
  - degradation_alert            # deterministic — produced by anomaly detector before agent runs
  - agent_tuning_proposal        # derivative — extracted from health report by post-processing
  - workflow_tuning_proposal     # derivative — extracted from health report by post-processing
  - experiment_plan              # derivative — extracted from health report by post-processing
requires_human_approval: false  # The agent itself runs without approval (it is read-only and produces reports).
                                # Its *recommendations* require human approval before action, enforced in the V2+ meta-workflow, not in the agent definition.
prompt: |
  See system prompt in section 12.1.
```

### 11.2 Steward Auditor

```yaml
id: steward_auditor
title: Steward Auditor
mode: meta_audit
backend_profile: claude_auditor_medium  # illustrative — profile does not exist yet; uses Claude Code to stay within MVP provider scope
permission_profile: RO_VERIFY
skill_ref: steward_core  # illustrative — skill does not exist yet
skill_role: critical_auditor
output_contract: stewardship_audit_report_v1  # primary structured output — validated at runtime
inputs:
  - sdlc_health_report        # produced by system_steward — this IS the Steward analysis that the Auditor challenges
  - implicated_run_dossiers
  - metrics_window
  - baseline_window
outputs:
  - stewardship_audit_report
requires_human_approval: false
prompt: |
  See system prompt in section 12.2.
```

### 11.3 Agent Retrospective Interviewer

```yaml
id: agent_retrospective_interviewer
title: Agent Retrospective Interviewer
mode: retrospective
backend_profile: claude_retrospective_medium  # illustrative — profile does not exist yet; uses Claude Code to stay within MVP provider scope
permission_profile: RO_VERIFY
skill_ref: steward_core  # illustrative — skill does not exist yet
skill_role: agent_retro
output_contract: agent_retrospective_report_v1  # primary structured output — validated at runtime
inputs:
  - implicated_agent_dossier
  - source_artifacts
  - review_feedback
  - final_outcome
outputs:
  - agent_retrospective_report
requires_human_approval: false
prompt: |
  See system prompt in section 12.3.
```

### 11.4 Catalog implementation checklist

Before the section 11 definitions become loadable, the following companion changes are required in `agents.yaml`. This checklist is verified against the current repo contract (`AgentCatalog.swift`, `YAMLValidator.swift`, `agents.yaml` as of 2026-03-22).

#### Backend profiles to add

| Profile key | Current state | Required action |
|---|---|---|
| `claude_steward_high` | Does not exist | Add to `backend_profiles` — provider: `claude_code`, model: `default`, effort: `high`, temperature: `0.1`, max_turns: `16`, structured_output: `required` |
| `claude_auditor_medium` | Does not exist | Add to `backend_profiles` — provider: `claude_code`, model: `default`, effort: `medium`, temperature: `0.0`, max_turns: `14`, structured_output: `required` |
| `claude_retrospective_medium` | Does not exist | Add to `backend_profiles` — provider: `claude_code`, model: `default`, effort: `medium`, temperature: `0.1`, max_turns: `12`, structured_output: `required` |

#### Skill refs to add

| Skill key | Current state | Required action |
|---|---|---|
| `steward_core` | Does not exist | Add to `skills` — either `inline` (with description) or `external_skill` (with path) depending on packaging decision |

#### Permission profile

| Profile key | Current state | Required action |
|---|---|---|
| `RO_VERIFY` | **Already exists** in `agents.yaml` | No action needed. The existing `RO_VERIFY` profile provides read-only filesystem access with verification capabilities, which is appropriate for Steward agents |

#### Agent modes

`AgentDefinition.mode` is a **free-form `String`** in the current schema (not an enum). The values `meta_analysis`, `meta_audit`, and `retrospective` will work without any schema changes. No action needed.

#### Artifact path entries (`artifacts:` map)

`YAMLValidator.validateArtifactRefs()` checks that every agent input and output exists in the top-level `artifacts:` map. The following entries must be added for the Steward agents' inputs and outputs to pass validation:

| Artifact key | Suggested path | Used by |
|---|---|---|
| `metrics_window` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/metrics-window.json` | `system_steward` input |
| `baseline_window` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/baseline-window.json` | `system_steward` input, `steward_auditor` input |
| `implicated_run_dossiers` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/dossiers/` | `system_steward` input, `steward_auditor` input |
| `agent_catalog_snapshot` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/catalog-snapshot.json` | `system_steward` input |
| `workflow_snapshot` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/workflow-snapshot.json` | `system_steward` input |
| `config_change_log` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/config-change-log.json` | `system_steward` input |
| `sdlc_health_report` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/reports/health-report.json` | `system_steward` output, `steward_auditor` input |
| `degradation_alert` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/reports/degradation-alert.json` | `system_steward` output |
| `agent_tuning_proposal` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/proposals/agent-tuning.json` | `system_steward` output |
| `workflow_tuning_proposal` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/proposals/workflow-tuning.json` | `system_steward` output |
| `experiment_plan` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/proposals/experiment-plan.json` | `system_steward` output |
| `stewardship_audit_report` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/reports/audit-report.json` | `steward_auditor` output |
| `implicated_agent_dossier` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/dossiers/agent/` | `agent_retrospective_interviewer` input |
| `source_artifacts` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/dossiers/source-artifacts/` | `agent_retrospective_interviewer` input |
| `review_feedback` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/dossiers/review-feedback.json` | `agent_retrospective_interviewer` input |
| `final_outcome` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/dossiers/final-outcome.json` | `agent_retrospective_interviewer` input |
| `agent_retrospective_report` | `${CHAINWORKS_META_ROOT:-.chainworks}/steward/reports/retrospective-report.json` | `agent_retrospective_interviewer` output |

#### Output contracts to add (runtime — bound via `output_contract`)

Only contracts bound to an agent's `output_contract` field need to be added to the catalog `contracts` map. Deterministic artifacts are validated by unit tests, not by the runtime contract validator.

| Contract key | Current state | Required action | Bound by |
|---|---|---|---|
| `sdlc_health_report_v1` | Does not exist | Add to `contracts` — see V1 contract sketches below | `system_steward.output_contract` |
| `stewardship_audit_report_v1` | Does not exist | Add to `contracts` | `steward_auditor.output_contract` |
| `agent_retrospective_report_v1` | Does not exist | Add to `contracts` | `agent_retrospective_interviewer.output_contract` |

#### Deterministic artifact schemas (test-only — NOT added to catalog `contracts`)

The following artifact is produced by deterministic code (the anomaly detector), not by an LLM agent. Its schema is defined as a Swift `Codable` struct and validated by unit tests. It is **not** added to the catalog `contracts` map because the runtime `output_contract` path does not reach it.

| Schema | Produced by | Validation mechanism |
|---|---|---|
| `degradation_alert_v1` | Anomaly detector (deterministic code) | `DegradationSignal` Swift struct + unit tests |

#### `steward_config.yaml`

| Item | Current state | Required action |
|---|---|---|
| `steward_config.yaml` file | Does not exist | Create alongside `agents.yaml` and `workflow.yaml` with defaults from section 6 |
| `YAMLParser.loadStewardConfig()` | Does not exist | Implement typed parser |
| `YAMLValidator.validateStewardConfig()` | Does not exist | Implement validation rules |
| `DefinitionHasher.hash(stewardConfig:)` | Does not exist | Implement provenance hashing |

All profiles use providers within MVP scope (Codex, Claude Code). If Gemini or other providers are desired for the auditor role, the PS must be updated first per section 4.6 of the MVP PS.

#### V1 artifact contract sketches

##### Runtime catalog contracts (added to `agents.yaml` `contracts`)

These follow the same `format + required_fields` pattern as existing contracts in `agents.yaml`:

```yaml
sdlc_health_report_v1:
  format: json
  required_fields:
    - analysis_id
    - window_start
    - window_end
    - cohort_keys
    - cohort_quality
    - run_count
    - metrics_summary        # key metric values for the observation window
    - baseline_summary       # key metric values for the baseline window
    - degradations           # array of detected degradation signals
    - improvements           # array of detected improvement signals
    - executive_summary      # one-paragraph human-readable summary
    - confidence

stewardship_audit_report_v1:
  format: json
  required_fields:
    - analysis_id
    - claims_reviewed
    - claims_supported
    - claims_undersupported
    - alternate_explanations
    - recommendation_risk_review
    - safer_next_step
    - confidence

agent_retrospective_report_v1:
  format: json
  required_fields:
    - analysis_id
    - agent_id
    - run_id
    - situation_reconstruction
    - expected_vs_actual
    - likely_failure_modes
    - evidence_refs
    - suggested_changes
    - confidence
```

##### Deterministic artifact schema (test-only — NOT added to catalog)

`degradation_alert_v1` is produced by the anomaly detector in deterministic code, not by an LLM agent. Its shape is defined as a Swift `Codable` struct (`DegradationSignal`) and validated by unit tests:

```swift
struct DegradationSignal: Codable, Hashable {
    let analysisID: UUID
    let metricName: String
    let metricFamily: String       // "timing", "rework", "quality", "cost", "stability"
    let observedValue: Double
    let baselineValue: Double
    let deltaPercentage: Double
    let thresholdUsed: Double
    let implicatedRunIDs: [UUID]
    let severity: String           // "high", "medium", "low"
    let likelyCauses: [String]
    let confidence: String         // "high", "medium", "low"
}
```

---

## 12. System prompts

These prompts are deliberately operational and constrained.
They are not written to sound mystical.
They are written to keep the agent close to evidence.

### 12.1 System prompt — Forge Steward

```text
You are Forge Steward, the system-level SDLC health analyst for Chainworks.

Your job is to observe the performance of the agent workflow across many runs,
identify meaningful degradations or improvements, separate symptoms from likely causes,
and produce concrete, testable change proposals.

You are NOT responsible for delivering a single feature.
You are responsible for evaluating how well the delivery system itself is functioning.

You receive:
- deterministic metrics for a time window
- baseline or comparison window metrics
- run dossiers with execution history, artifacts, approvals, failures, retries, and cost
- workflow and agent catalog snapshots
- records of recent config changes when available

You must follow this method:

1. Establish what changed.
   - Name the metric or behavior that moved.
   - State the size and direction of the change.
   - Say when the shift appears to begin.

2. Check whether the comparison is fair.
   - Note sample size.
   - Note whether the compared runs look comparable by project type, repo, workflow family, or risk class.
   - If comparability is weak, lower confidence explicitly.

3. Separate symptom from suspected cause.
   - Symptoms are directly observed degradations.
   - Causes are hypotheses.
   - Never present a cause as certain when it is only inferred.

4. Look for competing explanations.
   - backend changes
   - effort changes
   - workflow ordering changes
   - input quality changes
   - task mix changes
   - higher project complexity
   - approval behavior changes

5. Use dossiers, not intuition.
   - Refer to concrete run behaviors.
   - Prefer repeated patterns across multiple runs over a single dramatic example.

6. Produce operational recommendations.
   For each recommendation include:
   - what to change
   - why this change is likely to help
   - which metric should improve if the hypothesis is right
   - possible downside or tradeoff
   - a small experiment or rollout plan
   - rollback condition

7. Stay within your role.
   - Do not silently modify agents, prompts, or workflows.
   - Do not recommend broad rewrites without evidence.
   - Do not optimize a single metric at the expense of overall system quality.

Output structure:
- Executive Summary
- Observed Changes
- Evidence
- Likely Causes
- Competing Explanations
- Recommendations
- Experiment Plan
- Confidence and Open Questions

Tone:
- precise
- practical
- evidence-first
- no theatrics
- no false certainty

If evidence is weak, say so plainly.
If there is no meaningful degradation, say that too.
```

### 12.2 System prompt — Steward Auditor

```text
You are Steward Auditor, an independent critic of the Forge Steward analysis.

Your role is not to generate the first explanation.
Your role is to challenge the current explanation and test whether the evidence really supports it.

You receive:
- the Forge Steward analysis
- the underlying metrics window and baseline
- implicated run dossiers

You must do the following:

1. Identify the strongest claims in the Steward analysis.
2. For each claim, test whether the evidence is sufficient.
3. Look for alternate explanations the Steward may have ignored.
4. Point out sample-size problems, selection bias, or weak comparisons.
5. Distinguish clearly between:
   - supported
   - plausible but unproven
   - weak or speculative
6. Critique recommendations that are expensive to test, hard to reverse, or poorly scoped.
7. Suggest safer or narrower experiments when appropriate.

Output structure:
- Audit Summary
- Claims That Hold Up
- Claims That Are Undersupported
- Alternate Explanations
- Recommendation Risk Review
- Safer Next Step
- Confidence

Constraints:
- Do not rewrite the Steward report from scratch.
- Do not optimize for agreement.
- Be adversarial toward weak reasoning, not toward people.
- Keep criticism concrete and evidence-linked.
```

### 12.3 System prompt — Agent Retrospective Interviewer

```text
You are Agent Retrospective Interviewer.

Your task is to reconstruct why a specific agent execution produced a weak or degraded outcome.
You are not allowed to rely on supposed memory.
You must reason only from the dossier you are given.

You receive:
- the exact task assigned to the agent
- the exact inputs available at execution time
- the outputs the agent produced
- review feedback on those outputs
- downstream outcomes and final status
- optional transcript and tool traces

Your method:

1. Reconstruct the execution situation.
   - What was the agent asked to do?
   - What information did it have?
   - What constraints existed?

2. Compare expected vs actual output.
   - What should a strong output have contained?
   - What is missing, late, shallow, or misaligned?

3. Identify likely failure modes.
   Examples:
   - under-scoping
   - overconfidence
   - missed constraint
   - weak use of inputs
   - late surfacing of concerns
   - format compliance without substance
   - poor escalation when uncertainty was high

4. Stay evidence-bound.
   - If a failure mode is inferred rather than directly visible, mark it as a hypothesis.
   - Do not invent hidden internal reasons.

5. Recommend targeted changes.
   Prefer narrow changes such as:
   - input contract tightening
   - output contract tightening
   - reordering in the workflow
   - backend or effort adjustment
   - better escalation trigger
   - stronger review pairing

Output structure:
- Situation Reconstruction
- Expected vs Actual
- Likely Failure Modes
- Evidence
- Suggested Changes
- Confidence

Constraints:
- No imaginary memory
- No vague psychologizing
- No blaming language
- No broad rewrite proposals unless the dossier truly supports them
```

---

## 13. Suggested first milestone

The safest first milestone is:

### Milestone A — Steward Observer

#### Persistence prerequisites
- `StewardAnalysis`, `StewardAnalysisRunLink`, and `StewardRecommendation` SwiftData models
- SwiftData schema migration plan for existing databases (lightweight migration — all new fields are optional with nil defaults)
- Cohorting metadata fields on `Run` (`workflowFamily`, `projectKey`, `riskClass`, `stack`)
- Steward data model additions on `AgentExecution` (at minimum: `agentConfigHash`, `retryReason`)
- `steward_config.yaml` typed loader (`YAMLParser.loadStewardConfig()`), validator (`YAMLValidator.validateStewardConfig()`), and provenance hasher (`DefinitionHasher.hash(stewardConfig:)`)

#### Runtime components
- deterministic metrics collector
- anomaly detector with configurable thresholds
- run dossier builder
- Forge Steward analysis report (persisted as `StewardAnalysis` + artifact on disk)
- Steward Auditor challenge report (persisted as linked artifact)
- V1 trigger mechanism (manual CLI or on-demand from `ExecutionService`)

#### Scope boundaries
- no automatic patches
- no automatic experiments
- no in-app Steward UI

This is enough to tell whether the idea has real value before you let it start steering the factory.

---

## 14. Final recommendation

Yes, build this.
But build it as a **system governance layer**, not as an ordinary production agent.

If you keep it evidence-first, read-only at first, and disciplined about experiments, it can become one of the highest-leverage parts of Chainworks.
If you make it self-modifying too early, it will mostly generate confident turbulence.

The right first version is boring in the best possible way:
- it measures honestly
- it notices regressions early
- it makes careful proposals
- it earns the right to influence the system later

