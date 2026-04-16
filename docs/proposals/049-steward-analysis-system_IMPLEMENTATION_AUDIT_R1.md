# Proposal 049 Steward Analysis System Multi-Lens Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/049-steward-analysis-system.md` |
| Repository Root | `.` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Working Tree | dirty; broad local modifications plus untracked control-plane files are present |
| Audited At | `2026-04-16T10:26:19+03:00` |
| Platform Scope | Universal / Rust control-plane; dedicated Steward UI is out of scope |
| Proposal State | Active Draft; no superseded/deprecated/replaced marker found |
| Overall Conformance | Not Implemented |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 049 is not implemented in the current tree. The run-start ingress prerequisite has moved in the right direction: `StartRunCmd`, GraphQL `startRun`, and MCP `runs.start` now require workflow and agent-catalog YAML paths. The actual Steward system promised by the proposal is still absent from the Rust control-plane: no Steward domain/repo/service modules, no Steward tables, no run-owned cohort/snapshot fields, no daemon-owned runtime inputs, no `WorkItemKind::StewardAnalysis`, no GraphQL/MCP Steward readback, and no `proposal-049` proof gate.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Not Implemented | Rust Steward pipeline and persistence/readback surfaces are missing | High |
| Architecture | Weak | Frozen truth owner chain stops after YAML path ingress | High |
| Product | At Risk | Operators cannot trigger or read Steward analyses through the control-plane | High |
| UI | Acceptable | Dedicated Steward dashboard is explicitly out of scope | High |
| UX | At Risk | No northbound analysis feedback or status readback exists | High |
| Readiness | Not Ready | Required proof gate is absent and target test names run zero tests | High |

## Proposal Contract

### Scope

- Deterministic Rust Steward observer pipeline over completed DB runs.
- Run-owned cohort and snapshot provenance freezing at run creation.
- Daemon-owned current `steward_config.yaml` and `agents.yaml` loading, validation, hashing, and config-change pending flag.
- Active-catalog optional LLM lanes for `system_steward` and `steward_auditor`.
- Durable analysis records, run links, recommendations, artifact IDs/paths, and northbound GraphQL/MCP/resource readback.
- Queue-based manual, post-run, and config-change triggers through `WorkItemKind::StewardAnalysis`.

### Locked Decisions

- Primary cohort identity is exactly `(workflow_family, risk_class)`.
- `project_key` and `stack` are quality/diagnostic facets only.
- Pre-P049 completed runs missing frozen cohort or snapshot fields are `legacy_pre_p049` and excluded from deterministic cohorting.
- Snapshot hashes/JSON are produced at run creation from parsed frozen definitions; Steward analysis must not recompute them from mutable YAML.
- Optional LLM lanes must not block deterministic analysis persistence.
- GraphQL/MCP/resource readback must expose persisted analysis truth; disk artifacts alone do not count.

### Primary User Flows

- Start a YAML-backed run and durably freeze workflow family, project key, risk class, stack, workflow snapshot, and catalog snapshot.
- Let the daemon detect current Steward config/catalog changes and queue analysis after the next completed run.
- Trigger Steward analysis manually through MCP.
- Analyze completed deterministic-eligible runs, persist metrics, signals, dossiers, analysis records, run links, recommendations, and artifacts.
- Read completed, inconclusive, or failed analyses through GraphQL, MCP tools, and `steward-analysis://{analysis_id}`.

### UI Commitments

- No dedicated Steward dashboard UI is in scope.

### UX Commitments

- Operators must have northbound readback for analysis status, recommendations, linked runs, and artifacts.
- Operators must be able to distinguish `completed`, `inconclusive`, and `failed` analyses.
- Manual trigger must enqueue the same queue lane as automatic triggers, avoiding hidden direct-execution semantics.

### Acceptance Criteria

- AC 1-7: cohort owner contract.
- AC 8-10: frozen snapshot provenance.
- AC 11-13: daemon current-input bootstrap semantics.
- AC 14-17: deterministic pipeline.
- AC 18-22: active-catalog steward parity.
- AC 23-26: metric-source correctness.
- AC 27-30: northbound readback.
- AC 30-31: trigger semantics.

### Test / Evidence Requirements

- `proposal-049|p049` focused composite gate.
- Target buckets include workflow metadata freezing, run snapshot production, steward runtime bootstrap, deterministic pipeline, cohort classifier, legacy exclusion, trigger semantics, GraphQL readback, and MCP tools.
- `cargo test --workspace` alone is explicitly insufficient proof.

### Explicit Exclusions

- Dedicated Steward dashboard UI.
- Schedule trigger wiring.
- V2 recommendation synthesis beyond persisted proposal artifacts.
- V3 experiment execution.
- Live-session introspection outside persisted run/session truth.

## Proposal Fidelity / Divergence

### Matches

- GraphQL `startRun` now accepts required `workflow_yaml_path` and `agent_catalog_yaml_path` arguments and passes them into `StartRunCmd`.
- MCP `runs.start` now requires `workflow_yaml_path` and `agent_catalog_yaml_path` in its schema and execution path.
- `StartRunCmd` now carries non-optional YAML paths.
- `examples/agents/agents.yaml` contains active `system_steward` and `steward_auditor` IO vocabulary matching the proposal.
- The stable Swift Steward V1 implementation still exists, but it is not the Rust control-plane implementation requested by P049.

### Divergences

- Rust `WorkflowMeta` does not expose `family`, `risk_class`, or `stack`.
- Rust `Idea` does not expose or persist `project_key`, and `ideas.create` does not accept it.
- Rust `Run` and the `runs` repo/table do not contain the P049 frozen cohort, snapshot JSON/hash, or drift fields.
- Rust `StageExecution` and stage persistence do not contain `retry_reason`.
- No Rust Steward domain, repo, service, metrics, anomaly, cohort, dossier, canonical JSON, or config modules exist.
- The daemon does not load `STEWARD_CONFIG_PATH` / `AGENT_CATALOG_PATH` into `StewardRuntimeInputs`.
- No `steward_analyses`, `steward_analysis_run_links`, or `steward_recommendations` tables exist.
- No `WorkItemKind::StewardAnalysis` queue lane exists.
- No GraphQL Steward analysis queries/types or MCP `steward.*` tools/resource exist.
- `scripts/test-gate.sh proposal-049` is unknown.

### Ambiguities / Evidence Gaps

- The working tree is dirty with substantial unrelated and adjacent control-plane changes, so this audit reports current working-tree truth rather than a clean branch state.
- No runtime daemon validation was performed because the core P049 entry points and persistence model are absent.
- Full regression was not run because the audit verdict is already non-successful and the proposal-specific gate is absent.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 1 |
| Partially Implemented | 0 |
| Missing | 17 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Require YAML paths on active run-start ingress

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:244`, `docs/proposals/049-steward-analysis-system.md:898`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/commands.rs:16`
  - `control-plane/crates/graphql-server/src/schema.rs:210`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:18`
- Gap / Note: This closes only the ingress-path prerequisite. It does not implement snapshot production or persistence.

### REQ-002 Add workflow metadata owners and freeze them onto runs

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:149`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/workflow/src/definition.rs:24`
  - `control-plane/crates/domain/src/run.rs:86`
- Gap / Note: `WorkflowMeta` has `id`, `name`, `description`, `uses_agent_catalog`, `required_providers`, `execution`, and `idea_input`, but no `family`, `risk_class`, or `stack`. `Run` has no `workflow_family`, `risk_class`, or `stack`.

### REQ-003 Add idea-owned `project_key` end to end

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:180`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/idea.rs:37`
  - `control-plane/crates/db/src/repos/ideas.rs:16`
  - `control-plane/crates/mcp-server/src/tools/ideas.rs:27`
- Gap / Note: `Idea` lacks `project_key`; the ideas table/repo does not persist it; `ideas.create` only accepts `title`, `body`, and `workspace_root_path`.

### REQ-004 Add frozen run-owned cohort, snapshot, and drift fields with DB round-trip

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:212`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/run.rs:86`
  - `control-plane/crates/db/src/repos/runs.rs:8`
  - `control-plane/crates/db/migrations/003_workflow_state_machine.sql:4`
  - `control-plane/crates/db/migrations/010_evidence_preflight_and_mcp.sql:5`
- Gap / Note: Rust `Run` and repo columns stop at workflow YAML paths, worktree/delivery fields, cancellation fields, and preflight JSON. No P049 cohort/snapshot/drift fields are present.

### REQ-005 Produce compiler-owned frozen snapshot payloads and hashes

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:232`, `docs/proposals/049-steward-analysis-system.md:256`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/workflow/src/compiler.rs:25`
  - `control-plane/crates/workflow/src/plan.rs:13`
  - `control-plane/crates/engine/src/command_handler.rs:143`
- Gap / Note: `compile()` returns only `initial_state`, `states`, `variables`, and `artifact_paths`. There is no Rust `DefinitionHasher`, no snapshot JSON/hash fields on `RunPlan`, and `StartRun` persists only YAML paths.

### REQ-006 Persist retry reason on stage retries

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:270`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/stage.rs:83`
  - `control-plane/crates/engine/src/command_handler.rs:354`
- Gap / Note: `StageExecution` has validation/evidence/recovery JSON but no `retry_reason`; `RetryStage` creates the next attempt without a reason field.

### REQ-007 Add Steward SQL tables, domain types, and repo layer

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:375`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:1`
  - `control-plane/crates/db/src/repos/mod.rs:1`
  - missing-file check for `control-plane/crates/domain/src/steward.rs` and `control-plane/crates/db/src/repos/steward.rs`
- Gap / Note: No `steward_analyses`, `steward_analysis_run_links`, or `steward_recommendations` tables/repos/domain types exist in the Rust control-plane.

### REQ-008 Implement deterministic metrics collection from durable owners

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:506`, `docs/proposals/049-steward-analysis-system.md:532`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `rg "MetricsSnapshot|collect_metrics" control-plane/crates -g '*.rs'` returned no Rust matches.
  - `control-plane/crates/db/migrations/006_session_lineage.sql:30`
  - `control-plane/crates/db/src/repos/sessions.rs:37`
- Gap / Note: Session cost owners exist, but no Rust Steward metrics collector joins them into P049 metrics.

### REQ-009 Implement primary cohort classifier and legacy exclusion

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:66`, `docs/proposals/049-steward-analysis-system.md:545`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `rg "PrimaryCohortKey|legacy_pre_p049" control-plane/crates -g '*.rs'` returned no matches.
  - `control-plane/crates/domain/src/run.rs:86`
- Gap / Note: Without frozen run fields and cohort module, P049's primary tuple grouping, project/stack quality-facet split, and legacy exclusion cannot execute in Rust.

### REQ-010 Implement anomaly detection, dossiers, inconclusive status, and bounded context dossiers

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:56`, `docs/proposals/049-steward-analysis-system.md:588`, `docs/proposals/049-steward-analysis-system.md:608`
- Status: Missing
- Evidence Type: code
- Evidence:
  - missing-file check for `control-plane/crates/engine/src/steward/anomaly.rs`
  - missing-file check for `control-plane/crates/engine/src/steward/dossier.rs`
  - `rg "DegradationSignal|RunDossier" control-plane/crates -g '*.rs'` returned no matches.
- Gap / Note: The Swift app has old V1 classes, but the Rust control-plane proposal modules are absent.

### REQ-011 Add daemon-owned Steward runtime inputs and bootstrap hashing

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:634`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/daemon/src/main.rs:27`
  - missing-file check for `control-plane/crates/daemon/src/config.rs`
  - missing-file check for `control-plane/crates/daemon/src/steward_runtime.rs`
  - `rg "StewardRuntimeInputs|StewardConfigLoadStatus" control-plane/crates -g '*.rs'` returned no matches.
- Gap / Note: The daemon reads `DATABASE_URL`, `GRAPHQL_ADDR`, and `MODE`; it does not load/validate/hash current Steward config or catalog.

### REQ-012 Materialize active-catalog Steward LLM inputs/outputs

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:87`, `docs/proposals/049-steward-analysis-system.md:297`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `examples/agents/agents.yaml:55`
  - `examples/agents/agents.yaml:1426`
  - missing-file check for `control-plane/crates/engine/src/steward/service.rs`
- Gap / Note: The catalog vocabulary exists, but no Rust Steward service materializes `agent_catalog_snapshot`, `workflow_snapshot`, `config_change_log`, or active output artifacts under `CHAINWORKS_META_ROOT`.

### REQ-013 Write canonical deterministic JSON artifacts

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:779`
- Status: Missing
- Evidence Type: code
- Evidence:
  - missing-file check for `control-plane/crates/engine/src/steward/json.rs`
  - `rg "steward/analyses|active-catalog-io" control-plane/crates -g '*.rs'` returned no matches.
- Gap / Note: There is no Rust canonical JSON writer or P049 artifact tree.

### REQ-014 Add `WorkItemKind::StewardAnalysis` and converge all triggers on it

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:813`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/db/src/work_item.rs:6`
  - `control-plane/crates/engine/src/executor.rs`
  - `control-plane/crates/daemon/src/main.rs:66`
- Gap / Note: Work item variants are `InvokeAgent`, `SettleStage`, `AdvanceRun`, `RebuildProjection`, `StartupRepair`, and `TriggerNextStage`; no Steward queue lane or trigger convergence exists.

### REQ-015 Expose GraphQL Steward analysis readback

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:708`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:38`
  - `control-plane/crates/graphql-server/src/types`
  - `rg "stewardAnalyses|stewardAnalysis|GqlSteward" control-plane/crates/graphql-server/src -g '*.rs'` returned no matches.
- Gap / Note: GraphQL exposes ideas, runs, approvals, artifacts, stages, and agent executions, but not Steward analyses or recommendations.

### REQ-016 Expose MCP Steward tools and `steward-analysis://` resource

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:749`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/mod.rs:1`
  - `control-plane/crates/mcp-server/src/server.rs:237`
  - `rg "steward.run_analysis|steward.list_analyses|steward.get_analysis|steward-analysis://" control-plane/crates/mcp-server/src -g '*.rs'` returned no matches.
- Gap / Note: MCP registers ideas, runs, approvals, stages, and reports only.

### REQ-017 Implement metric-source correctness for cost, drift, and retry evidence

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:506`
- Status: Missing
- Evidence Type: code
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:30`
  - `control-plane/crates/domain/src/stage.rs:83`
  - `control-plane/crates/domain/src/run.rs:86`
- Gap / Note: Some source owners exist for session cost/reuse, but P049's drift and retry owners are absent and no Steward metrics layer consumes the durable owners.

### REQ-018 Add focused `proposal-049` proof gate and tests

- Proposal Source: `docs/proposals/049-steward-analysis-system.md:951`
- Status: Missing
- Evidence Type: tests-run
- Evidence:
  - `scripts/test-gate.sh:1170`
  - `scripts/test-gate.sh:1499`
  - `./scripts/test-gate.sh proposal-049` -> `error: Unknown gate: proposal-049`
  - `cargo test -p workflow steward_metadata_contract_tests -- --nocapture` -> 0 tests
  - `cargo test -p daemon steward_runtime_bootstrap_tests -- --nocapture` -> 0 tests
  - `cargo test -p engine steward_pipeline_tests -- --nocapture` -> 0 tests
  - `cargo test -p graphql-server steward_graphql_readback_tests -- --nocapture` -> 0 tests
  - `cargo test -p mcp-server steward_mcp_tools_tests -- --nocapture` -> 0 tests
- Gap / Note: The proposal-specific gate and test inventory are not present.

## Architecture Review

**Summary:** Weak

### ARCH-001 Rust Steward subsystem is absent

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-007 through REQ-016
- Evidence Type: code
- Evidence:
  - missing-file check for `control-plane/crates/engine/src/steward/*`
  - `control-plane/crates/engine/src/lib.rs:1`
  - `control-plane/crates/domain/src/lib.rs:1`
  - `control-plane/crates/db/src/repos/mod.rs:1`
- Why It Matters: P049 is a Rust control-plane proposal. The existing Swift Steward V1 implementation proves product precedent, not conformance to the Rust daemon/control-plane contract.
- Recommended Action: Implement the Rust Steward module set and wire it through domain, DB, engine, daemon, GraphQL, MCP, and queue owners before re-auditing readiness.

### ARCH-002 Frozen owner chain stops at path ingress

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-001 through REQ-006
- Evidence Type: code
- Evidence:
  - `control-plane/crates/domain/src/commands.rs:16`
  - `control-plane/crates/workflow/src/compiler.rs:25`
  - `control-plane/crates/workflow/src/plan.rs:13`
  - `control-plane/crates/domain/src/run.rs:86`
  - `control-plane/crates/engine/src/command_handler.rs:143`
- Why It Matters: Requiring YAML paths is necessary but insufficient. P049's deterministic claims depend on freezing parsed metadata and snapshot JSON/hash truth onto `Run`.
- Recommended Action: Extend workflow metadata, add a Rust `DefinitionHasher`, carry snapshot payload/hash fields through `RunPlan`, and persist all frozen fields in `StartRun`.

### ARCH-003 Persistence and northbound read model is missing

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-007, REQ-015, REQ-016
- Evidence Type: code
- Evidence:
  - `control-plane/crates/db/migrations/001_initial.sql:1`
  - `control-plane/crates/graphql-server/src/schema.rs:38`
  - `control-plane/crates/mcp-server/src/tools/mod.rs:1`
- Why It Matters: A Steward analysis that cannot be durably stored and read back cannot satisfy the proposal's operator and automation contract.
- Recommended Action: Add Steward migrations/repos/types first, then expose the same persisted rows through GraphQL, MCP tools, and `steward-analysis://{analysis_id}`.

## Product Review

**Summary:** At Risk

### PROD-001 Primary operator job is not achievable

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-014, REQ-015, REQ-016
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/mcp-server/src/tools/mod.rs:1`
  - `control-plane/crates/db/src/work_item.rs:6`
  - `./scripts/test-gate.sh proposal-049` -> unknown gate
- Why It Matters: Operators cannot manually trigger Steward analysis and cannot list/get analysis results from the control-plane.
- Recommended Action: Ship `steward.run_analysis`, `steward.list_analyses`, `steward.get_analysis`, queue execution, and readback before treating P049 as useful.

### PROD-002 Historical-run migration semantics are specified but not executable

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-004, REQ-009
- Evidence Type: code
- Evidence:
  - `docs/proposals/049-steward-analysis-system.md:72`
  - `control-plane/crates/domain/src/run.rs:86`
  - `control-plane/crates/db/src/repos/runs.rs:8`
- Why It Matters: The proposal now correctly excludes legacy runs missing frozen truth, but the implementation has no fields or classifier to detect that state.
- Recommended Action: Add explicit `legacy_pre_p049` eligibility handling in the Rust cohort classifier after adding the frozen run fields.

## UI Review

**Summary:** Acceptable

### UI-001 No dedicated Steward UI is in scope

- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: explicit exclusions
- Evidence Type: code
- Evidence:
  - `docs/proposals/049-steward-analysis-system.md:994`
- Why It Matters: There is no proposal basis for failing a missing Steward dashboard.
- Recommended Action: Keep P049 validation focused on control-plane APIs/resources. Add UI review only when a dashboard proposal exists.

## UX Review

**Summary:** At Risk

### UX-001 No analysis feedback loop exists for operators

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: REQ-015, REQ-016
- Evidence Type: code
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:38`
  - `control-plane/crates/mcp-server/src/server.rs:237`
- Why It Matters: P049 requires operators to distinguish `completed`, `inconclusive`, and `failed` analyses. With no readback surfaces, the operator cannot know whether analysis ran, failed, or produced recommendations.
- Recommended Action: Implement persisted status readback and mirror it across GraphQL, MCP tools, and the MCP resource lane.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Required proof gate is absent

- Severity: Critical
- Confidence: High
- Related Proposal Items / Requirements: REQ-018
- Evidence Type: tests-run
- Evidence:
  - `./scripts/test-gate.sh proposal-049` -> `error: Unknown gate: proposal-049`
  - targeted P049 cargo filters all ran zero tests
- Why It Matters: P049 explicitly rejects a generic workspace pass as sufficient. The required gate is not available.
- Recommended Action: Add `proposal-049|p049` to `scripts/test-gate.sh` and `docs/reference/test-gates.md`, then add the named focused tests or equivalent guarantee buckets.

### READY-002 Working tree is too broad for a clean sign-off

- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: overall readiness
- Evidence Type: code
- Evidence:
  - `git status --short`
  - `git diff --stat` -> 86 changed tracked files plus untracked control-plane files
- Why It Matters: The tree includes substantial unrelated or adjacent proposal work. Even after implementation, P049 sign-off should happen against a branch where the P049 diff and dependencies are explicit.
- Recommended Action: Land or isolate prerequisite work, then run the focused gate and full regression only if the proposal-conformance verdict is otherwise green.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Partial | Targeted Rust crate filters compiled, but no full workspace or app build was run. |
| Core user flow runtime-validated | Fail | No Steward trigger/readback/pipeline entry points exist. |
| Empty/loading/error states covered | Not Checked | Dedicated UI out of scope; control-plane status readback missing. |
| Accessibility risk acceptable | Not Applicable | No UI surface in this proposal. |
| Localization risk acceptable | Not Applicable | No UI strings in scope. |
| Critical tests executed | Fail | Required P049 test names ran zero tests. |
| Full regression suite / canonical full gate passed on same tree/HEAD | Not Checked | Not required for a non-successful verdict; `proposal-049` gate is absent. |
| Privacy/permissions/entitlements reviewed | Partial | MCP auth exists generally, but no Steward tools/resources exist to review. |

## Verification Log

- `sed -n '1,760p' /Users/user/.agents/skills/proposal-implementation-audit/SKILL.md`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/'Chainworks Forge'/docs/proposals/049-steward-analysis-system.md`
- `git rev-parse HEAD`
- `git status --short`
- `git diff --stat`
- `nl -ba docs/proposals/049-steward-analysis-system.md | sed -n '41,178p'`
- `nl -ba docs/proposals/049-steward-analysis-system.md | sed -n '180,371p'`
- `nl -ba docs/proposals/049-steward-analysis-system.md | sed -n '375,631p'`
- `nl -ba docs/proposals/049-steward-analysis-system.md | sed -n '634,998p'`
- `sed -n '1,260p' docs/proposals/049-steward-analysis-system.review/proposal-readiness-review.md`
- `sed -n '1,260p' docs/proposals/049-steward-analysis-system.review/evidence-pack.md`
- `find control-plane/crates examples scripts docs -maxdepth 5 \( -iname '*steward*' -o -iname '*definition*hasher*' \) -print | sort`
- `rg -n "Steward|steward|workflow_family|project_key|risk_class|workflow_snapshot_hash|catalog_snapshot_hash|steward_analyses|steward_recommendations|steward-analysis|StewardAnalysis|WorkItemKind::StewardAnalysis|retry_reason|drift_detected_at|DefinitionHasher|context_strategy_profiles|proposal-049|p049" control-plane scripts examples docs/reference docs/reviews -g '*.rs' -g '*.sql' -g '*.md' -g '*.yaml' -g '*.toml'`
- `nl -ba control-plane/crates/domain/src/commands.rs | sed -n '1,180p'`
- `nl -ba control-plane/crates/domain/src/run.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/workflow/src/definition.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/workflow/src/plan.rs | sed -n '1,220p'`
- `nl -ba control-plane/crates/workflow/src/compiler.rs | sed -n '1,340p'`
- `nl -ba control-plane/crates/db/src/repos/runs.rs | sed -n '1,320p'`
- `nl -ba control-plane/crates/engine/src/command_handler.rs | sed -n '1,520p'`
- `nl -ba control-plane/crates/domain/src/stage.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/db/src/work_item.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/domain/src/idea.rs | sed -n '1,220p'`
- `nl -ba control-plane/crates/db/src/repos/ideas.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/mcp-server/src/tools/ideas.rs | sed -n '1,220p'`
- `nl -ba control-plane/crates/graphql-server/src/schema.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/mcp-server/src/tools/runs.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/mcp-server/src/server.rs | sed -n '1,260p'`
- `nl -ba control-plane/crates/daemon/src/main.rs | sed -n '1,260p'`
- `for f in control-plane/crates/domain/src/steward.rs ... control-plane/crates/mcp-server/src/tools/steward.rs; do test -e "$f"; done`
- `./scripts/test-gate.sh proposal-049` -> failed, unknown gate
- `cargo test -p workflow steward_metadata_contract_tests -- --nocapture` -> 0 tests
- `cargo test -p daemon steward_runtime_bootstrap_tests -- --nocapture` -> 0 tests
- `cargo test -p engine steward_pipeline_tests -- --nocapture` -> 0 tests
- `cargo test -p graphql-server steward_graphql_readback_tests -- --nocapture` -> 0 tests
- `cargo test -p mcp-server steward_mcp_tools_tests -- --nocapture` -> 0 tests

## Recommended Next Actions

1. Implement frozen truth prerequisites first: workflow metadata owners, `project_key`, run cohort/snapshot/drift fields, `retry_reason`, DB migrations, and run-start snapshot hashing/persistence.
2. Add the Rust Steward core: domain/repo/service, metrics, cohort classifier, anomaly detector, dossiers, canonical JSON writer, and config validation.
3. Wire daemon runtime inputs and trigger semantics: config/catalog loading, effective hashes, pending config-change flag, `WorkItemKind::StewardAnalysis`, executor dispatch, manual/post-run/config-change enqueue paths.
4. Add durable readback: Steward tables/repos, GraphQL queries/types, MCP tools, and `steward-analysis://{analysis_id}`.
5. Add `proposal-049|p049` proof gate and focused tests before requesting another implementation audit.
