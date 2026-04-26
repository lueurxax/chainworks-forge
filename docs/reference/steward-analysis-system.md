# Steward Analysis System

This document is the stable implementation reference for the Rust Steward analysis system.

The Steward is the deterministic quality observatory for Chainworks Forge. Product-level motivation and V1 concepts remain in [forge-steward.md](forge-steward.md). This document describes the implemented Rust control-plane contracts: persisted owners, deterministic analysis, optional active-catalog Steward agents, triggers, artifacts, and northbound readback.

The canonical proof gate is:

```bash
./scripts/test-gate.sh proposal-049
```

P049 does not define a dedicated Steward dashboard UI. UI gates are not required to prove this subsystem.

## System Role

The Steward observes completed runs and produces quality analysis from persisted runtime truth.

It answers questions such as:

- which completed runs form a deterministic cohort
- whether the recent observation window degraded against a baseline window
- which runs are implicated or useful as context
- which deterministic recommendations should be stored
- whether optional Steward LLM lanes can produce health reports, tuning proposals, experiment plans, and audit reports

The deterministic analysis record must persist even when optional Steward-agent execution is absent or fails.

## Pipeline

The Rust pipeline is:

```text
1. Load daemon-owned current Steward inputs.
2. Query completed runs from the database.
3. Filter to deterministic-eligible runs.
4. Select the primary cohort by (workflow_family, risk_class).
5. Split observation and baseline windows.
6. Classify cohort quality using project_key and stack as quality facets.
7. Collect deterministic metrics from persisted owners.
8. Detect degradations and improvements.
9. Build implicated-run or context dossiers.
10. Write canonical JSON artifacts.
11. Optionally execute active Steward catalog agents.
12. Persist analysis rows, run links, recommendations, and artifact pointers.
13. Expose analysis through GraphQL and MCP.
```

Steps 1 through 10 are deterministic over database truth and daemon-owned current inputs. Optional agent lanes are non-deterministic and must not block deterministic persistence.

## Cohort Ownership

Steward cohorting reads frozen run-owned fields. It does not infer cohort identity from mutable workflow files during analysis.

### Primary cohort key

The primary cohort key is exactly:

```json
{
  "workflow_family": "proposal_to_release",
  "risk_class": "standard"
}
```

`project_key` and `stack` are quality and diagnostic facets only. They do not narrow the primary cohort.

### Workflow-owned fields

Workflow metadata provides:

- `workflow_family`
- `risk_class`
- `stack`

Owner rules:

- `workflow_family` comes from workflow metadata, with workflow id as legacy fallback
- `risk_class` comes from workflow metadata, with `standard` as legacy fallback
- `stack` comes from workflow metadata, with `unknown` as legacy fallback

If workflow family cannot be resolved, run creation fails rather than creating a run that Steward cannot classify deterministically.

### Idea-owned project key

`Idea.project_key` is the canonical project owner.

Ingress rules:

- `ideas.create` accepts optional `project_key`
- legacy ideas may store `NULL`
- run creation freezes `untagged` onto `Run.project_key` when the idea has no project key

Steward does not invent project identity at analysis time.

### Frozen run fields

Run creation freezes:

- `workflow_family`
- `project_key`
- `risk_class`
- `stack`
- `workflow_snapshot_hash`
- `catalog_snapshot_hash`
- `workflow_snapshot_json`
- `catalog_snapshot_json`

The run row is the durable owner consumed by Steward.

## Frozen Snapshot Provenance

Run-start compilation and hashing are the only valid producers for frozen snapshot truth.

Owner chain:

```text
workflow YAML + agent catalog YAML
  -> workflow compiler parses normalized definitions
  -> DefinitionHasher hashes parsed frozen definitions
  -> StartRun persists snapshot hashes and payloads on Run
  -> db::repos::runs round-trips them durably
  -> Steward reads persisted Run fields only
```

`workflow_yaml_path` and `agent_catalog_yaml_path` are ingress pointers. After run creation they are not canonical snapshot truth.

Steward analysis must not reload mutable YAML files to recompute historical run truth.

## Legacy Run Eligibility

Completed runs that predate this implementation may lack frozen cohort or snapshot fields.

Default rule:

- such rows are classified as `legacy_pre_p049`
- they are excluded from deterministic cohort analysis
- they may appear only as observability metadata for migration/backfill planning

Historical backfill is a maintenance path, not part of normal analysis.

## Cohort Quality

Cohort selection uses `(workflow_family, risk_class)`.

Quality grading then considers:

- run count
- project-key completeness
- stack completeness

V1 quality rules:

- `strong`: at least 10 runs, no untagged projects, no unknown stacks
- `acceptable`: at least 5 runs and not weak
- `weak`: fewer than 5 runs, or any untagged project cohort member

## Metrics Source Matrix

Steward metrics use durable owners only.

| Signal | Canonical owner |
|---|---|
| Lead time | `runs.started_at`, `runs.completed_at` |
| Stage latency | `stage_executions.started_at`, `stage_executions.completed_at`, `stage_executions.stage_id` |
| Approval wait | `approvals.requested_at`, `approvals.decided_at` |
| Proposal / implementation loops | `stage_executions.iteration` plus frozen workflow stage-family mapping |
| Retries per stage | `stage_executions.attempt_number`, `stage_executions.retry_reason` |
| Approval rejection rate | `approvals.decision` |
| Audit pass rate | `stage_executions.status` for audit-class stages |
| Cost per run | `agent_executions.session_generation_id -> session_generations.cumulative_cost_cents` |
| Cost by stage family | same session generation owner, grouped by stage family |
| Failed run rate | `runs.status` |
| Blocked run rate | terminal run status semantics |
| Drift event count | `runs.drift_detected_at`, `runs.drift_details_json` |
| Resumed run count | `agent_executions.session_reuse_disposition`, `agent_executions.session_reset_reason` |

`work_items` are not cost owners.

## Deterministic Analysis Artifacts

Steward writes canonical sorted-key JSON artifacts under:

```text
{artifact_base}/steward/analyses/{analysis_id}/
```

The active catalog IO root is:

```text
{artifact_base}/steward/analyses/{analysis_id}/active-catalog-io
```

`CHAINWORKS_META_ROOT` is set to that active-catalog IO root when optional Steward agents execute.

Canonical active-catalog paths include:

```text
active-catalog-io/steward/metrics-window.json
active-catalog-io/steward/baseline-window.json
active-catalog-io/steward/dossiers/{run_id}.json
active-catalog-io/steward/catalog-snapshot.json
active-catalog-io/steward/workflow-snapshot.json
active-catalog-io/steward/config-change-log.json
active-catalog-io/steward/reports/health-report.json
active-catalog-io/steward/reports/degradation-alert.json
active-catalog-io/steward/reports/audit-report.json
active-catalog-io/steward/proposals/agent-tuning.json
active-catalog-io/steward/proposals/workflow-tuning.json
active-catalog-io/steward/proposals/experiment-plan.json
```

When no runs are implicated, Steward writes bounded context dossiers and persists their run links with role `context`.

## Snapshot Artifact Hashes

The persisted analysis hash model separates workflow aggregate truth from current catalog truth.

- `workflow_snapshot_artifact_hash` is the canonical JSON hash of the materialized `workflow-snapshot.json` artifact.
- `agent_catalog_snapshot_hash` is the daemon-owned current catalog hash materialized into `catalog-snapshot.json`.
- `steward_config_snapshot_hash` is the effective Steward config hash used for analysis.

`workflow-snapshot.json` is singular by active-catalog contract, but its payload may index multiple frozen workflow snapshot hashes across analyzed runs.

It must truthfully represent:

- snapshot count
- primary workflow family
- entries keyed by run-owned `workflow_snapshot_hash`
- run ids associated with each snapshot
- frozen workflow snapshot payloads

No scalar may collapse workflow aggregate truth and current agent catalog truth.

## Optional Steward Agent Lanes

The active catalog defines two Steward agents.

### `system_steward`

Inputs:

- metrics window
- baseline window
- implicated run dossiers
- agent catalog snapshot
- workflow snapshot
- config change log

Outputs:

- SDLC health report
- degradation alert
- agent tuning proposal
- workflow tuning proposal
- experiment plan

### `steward_auditor`

Inputs:

- SDLC health report
- implicated run dossiers
- metrics window
- baseline window

Output:

- stewardship audit report

`steward_auditor` depends on the health report produced by `system_steward`.

Optional agent failures may set analysis error metadata or omit optional artifact pointers, but they do not erase deterministic metrics, dossiers, recommendations, or analysis records.

## Daemon-Owned Current Inputs

The daemon owns current Steward runtime inputs:

- Steward config path and parsed config
- effective Steward config hash
- Steward config load status
- agent catalog path and parsed catalog
- agent catalog hash
- config-change pending flag

Source paths:

- `STEWARD_CONFIG_PATH`, else `examples/steward/steward_config.yaml`
- `AGENT_CATALOG_PATH`, else `examples/agents/agents.yaml`

No subsystem should guess Steward paths from the current working directory.

### Config validation and fallback

Bootstrap validates the parsed Steward config.

If validation succeeds:

- the loaded config is the effective config
- the effective config is hashed

If validation fails:

- daemon runtime falls back to default Steward config semantics
- validation errors are recorded in load status
- trigger scheduling and analysis continuity are not suppressed
- the default effective config is what gets hashed

### Config-change detection

Config-change detection compares parsed-object hashes for:

- effective Steward config
- current agent catalog file

The Steward config hash includes all semantics that affect analysis, including:

- windows
- thresholds
- context strategy profiles
- triggers

Startup config-change detection sets a pending flag. It does not immediately run an analysis while claiming post-run trigger parity.

## Persistence Model

Core tables:

- `steward_analyses`
- `steward_analysis_run_links`
- `steward_recommendations`

`steward_analyses` stores:

- analysis id and timestamps
- window bounds
- run count
- primary cohort keys JSON
- cohort quality
- status: `completed`, `inconclusive`, `failed`, or `superseded`
- degradation and improvement counts
- workflow/catalog/config hashes
- artifact pointers
- trigger reason
- error summary

Run links store each related run and its role:

- `implicated`
- `baseline`
- `context`

Recommendations store first-class rows. They are not inferred from artifact presence.

## Trigger Semantics

All Steward triggers converge on `WorkItemKind::StewardAnalysis`.

Trigger sources:

- manual MCP trigger
- post-run interval trigger
- config-change pending flag consumed on the next completed run

There is no second direct-execution owner lane.

## Northbound Read Contract

### GraphQL

GraphQL exposes:

- `stewardAnalyses(limit, status)`
- `stewardAnalysis(id)`

The analysis payload includes:

- metadata
- status and trigger reason
- cohort quality and primary cohort JSON
- run/degradation/improvement counts
- workflow, catalog, and config hashes
- artifact ids
- error summary
- recommendations
- linked runs

Operators must be able to distinguish completed, inconclusive, failed, and superseded analyses.

### MCP

MCP exposes:

- `steward.run_analysis`
- `steward.list_analyses`
- `steward.get_analysis`
- `steward-analysis://{analysis_id}`

The resource returns the same persisted truth as GraphQL and MCP tool reads:

- analysis metadata
- recommendations
- linked runs
- deterministic artifact pointers
- optional Steward-agent artifact pointers

A disk artifact directory alone does not count as surfaced recommendations.

## Implementation Map

Primary Rust owners:

- `control-plane/crates/workflow/src/definition.rs`
- `control-plane/crates/workflow/src/compiler.rs`
- `control-plane/crates/workflow/src/plan.rs`
- `control-plane/crates/domain/src/commands.rs`
- `control-plane/crates/domain/src/idea.rs`
- `control-plane/crates/domain/src/run.rs`
- `control-plane/crates/domain/src/stage.rs`
- `control-plane/crates/domain/src/steward.rs`
- `control-plane/crates/db/src/repos/ideas.rs`
- `control-plane/crates/db/src/repos/runs.rs`
- `control-plane/crates/db/src/repos/stages.rs`
- `control-plane/crates/db/src/repos/steward.rs`
- `control-plane/crates/db/src/work_item.rs`
- `control-plane/crates/engine/src/command_handler.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/engine/src/steward/`
- `control-plane/crates/daemon/src/config.rs`
- `control-plane/crates/daemon/src/steward_runtime.rs`
- `control-plane/crates/graphql-server/src/types/steward.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/mcp-server/src/tools/steward.rs`
- `control-plane/crates/mcp-server/src/server.rs`

## Verification

Canonical focused gate:

```bash
./scripts/test-gate.sh proposal-049
```

The gate covers:

- workflow metadata and parsed snapshot freezing
- run-start frozen cohort/project/snapshot persistence
- Steward config validation and default fallback
- parsed config/catalog hashing and pending config-change bootstrap
- Steward analysis tables, run links, recommendations, failed-analysis rows, and work items
- cohort classification and primary cohort grouping
- legacy pre-P049 exclusion
- deterministic metrics, dossiers, anomaly signals, and recommendation persistence
- active-catalog Steward IO paths under `CHAINWORKS_META_ROOT`
- production `StewardAnalysis` work-item execution through ACP-backed Steward lanes
- manual, post-run interval, and config-change trigger convergence
- run-owned drift detection persistence from startup recovery
- GraphQL, MCP tool, and `steward-analysis://` readback

Relevant same-tree non-UI regression for broader control-plane readiness is `proposal-027`.

Remote UI `full` is not a P049 requirement because this subsystem does not define a dedicated Steward dashboard UI.

## Out of Scope

- dedicated Steward dashboard UI
- schedule trigger wiring
- V2 recommendation synthesis beyond persisted proposal artifacts
- V3 experiment execution
- live-session introspection outside persisted run/session truth
