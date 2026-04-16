# Proposal 049: Steward Analysis System Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/049-steward-analysis-system.md` |
| Proposal State | Active draft; no supersession marker found in the proposal |
| Platform Scope | Rust control-plane / daemon / GraphQL / MCP. Dedicated Steward dashboard UI is out of scope. |
| Worktree | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| Git SHA | `af3054c73064b05e42cb816a81a3c5fb0c2e29d9` |
| Audit Timestamp | `2026-04-16T11:10:24+03:00` |
| Working Tree | Dirty; P049 implementation files and prior audit reports are uncommitted/untracked |
| Supersedes | `049-steward-analysis-system_IMPLEMENTATION_AUDIT_R2.md` |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Verdict

P049 is now materially implemented, and the focused `proposal-049` gate passes on this tree. The previous R1-level "no Rust Steward implementation" blocker is closed.

The implementation is still not proposal-complete. The remaining gaps are not documentation nits: production Steward analysis never wires a real active-catalog steward agent executor, frozen snapshot hashing is not proven canonical over `HashMap`-backed parsed definitions, deterministic degradation artifacts include a fresh random `analysis_id`, drift ownership is schema-only, several metrics are sourced or grouped incorrectly, failed analyses are not durably persisted as failed records, and the proof gate does not cover those failure modes.

This is a Partial / Not Ready result.

## Verification

- `./scripts/test-gate.sh proposal-049` passed on this working tree.
- `git diff --check` passed.
- Full regression was not run for this audit because the roll-up is not a successful `Implemented` / `Ready` verdict. The skill requires full regression before a successful verdict; this report fails closed before that point.
- No UI runtime validation was performed because P049 explicitly excludes a dedicated Steward dashboard.

## Primary User Flows

1. A caller starts a run through GraphQL or MCP with workflow and agent-catalog YAML paths; run creation freezes cohort metadata and snapshot provenance.
2. The daemon boots with current steward config and agent catalog, hashes effective parsed inputs, and marks config-change analysis pending without immediate execution.
3. A manual, post-run, or config-change trigger enqueues the shared `WorkItemKind::StewardAnalysis` lane.
4. Steward reads completed persisted runs, selects a cohort, splits baseline and observation windows, collects metrics, detects signals, writes canonical artifacts, persists analysis rows, links, and recommendations.
5. Operators read analyses and recommendations through GraphQL, MCP tools, and the `steward-analysis://{analysis_id}` resource.

## Contract Extraction

P049 commits to a deterministic Rust Steward pipeline over persisted DB truth, daemon-owned parsed current inputs, and canonical serialization. Core commitments include:

- Cohort identity is exactly `(workflow_family, risk_class)`, with `project_key` and `stack` used only as quality/diagnostic facets.
- Workflow metadata, idea `project_key`, run snapshot hashes/JSON, drift fields, and retry reasons have durable owners.
- Frozen workflow/catalog snapshot hashes are computed by named run-start owners from parsed frozen definitions, not mutable YAML bytes or later file reloads.
- Legacy pre-P049 runs missing cohort/snapshot truth are excluded.
- The active catalog steward lane supports `system_steward` and `steward_auditor` IO under the proposal-owned `active-catalog-io` root.
- Deterministic artifacts use canonical sorted-key serialization, and rerunning the deterministic slice on unchanged data yields byte-identical deterministic JSON artifacts.
- GraphQL, MCP tools, and an MCP resource expose persisted analyses, recommendations, linked runs, and artifact IDs/paths.
- Operators can distinguish `completed`, `inconclusive`, and `failed` analyses from northbound reads.
- Post-run, config-change, and manual triggers converge on `WorkItemKind::StewardAnalysis`.

## Track 1: REQ Conformance Audit

| ID | Requirement | Proposal Source | Status | Evidence | Gap / Note |
|---|---|---|---|---|---|
| REQ-001 | Workflow metadata owners are added and frozen on runs. | Section 3a lines 149-178; AC 1 | Implemented | `workflow/src/definition.rs:24-36`, `workflow/src/compiler.rs:28-49`, `engine/src/command_handler.rs:177-211`, gate pass | `family`, `risk_class`, and `stack` are parsed/fallbacked and persisted on `Run`. |
| REQ-002 | `Idea.project_key` owner chain and `"untagged"` run fallback. | Section 3a lines 180-210; AC 2, 6 | Implemented | `domain/src/idea.rs:43-45`, `db/src/repos/ideas.rs:16-51`, `mcp-server/src/tools/ideas.rs:36-74`, `engine/src/command_handler.rs:166-175` | Domain, DB, MCP ingress, and run freeze path carry the field. |
| REQ-003 | `startRun` and `runs.start` require workflow/catalog YAML paths. | Section 3a lines 244-248; AC 5 | Implemented | `domain/src/commands.rs:27-30`, `graphql-server/src/schema.rs:283-325`, `mcp-server/src/tools/runs.rs:14-116` | Both northbound start lanes require the paths and pass them into `StartRunCmd`. |
| REQ-004 | Run schema/repo round-trip stores frozen cohort/provenance fields. | Section 3a lines 212-269; AC 8-10 | Implemented | `domain/src/run.rs:123-143`, `db/migrations/012_steward_analysis.sql:3-16`, `db/src/repos/runs.rs:8-74`, `engine/src/command_handler.rs:201-208` | Fields are present and persisted. Canonical hashing quality is covered separately in REQ-005. |
| REQ-005 | `DefinitionHasher`/equivalent computes canonical hashes from parsed frozen definitions. | Section 3a lines 232-263; AC 8-10 | Partially Implemented | `workflow/src/compiler.rs:44-49`, `workflow/src/compiler.rs:217-227`, `workflow/src/definition.rs:13-18`, `workflow/src/catalog.rs:12-20` | Hashes are produced at run start, but the implementation uses `serde_json::to_value` + `serde_json::to_string` over structs containing `HashMap` fields and does not use a named `DefinitionHasher` or explicit sorted canonical writer. Byte-stable canonical hashing is not proven. |
| REQ-006 | Retry reason and drift owners are durable and updated by owning paths. | Section 3b lines 270-295; AC 24-25 | Partially Implemented | `domain/src/stage.rs:110-111`, `db/migrations/012_steward_analysis.sql:16`, `db/src/repos/stages.rs:22-43`, `engine/src/command_handler.rs:377-409`, `domain/src/run.rs:140-143` | `RetryStage` writes `retry_reason`. Drift fields exist and are read, but no recovery/resume classification path writes `runs.drift_detected_at` or `runs.drift_details_json`. |
| REQ-007 | Steward analysis tables, domain types, repos, recommendations, and run links exist. | Section 4 lines 375-504; AC 27-30 | Implemented | `domain/src/steward.rs:6-123`, `db/migrations/012_steward_analysis.sql:18-83`, `db/src/repos/steward.rs:15-254`, gate pass | Durable schema and repo layer are present. Failed-row production is covered in REQ-014. |
| REQ-008 | Metrics source matrix uses durable owners and deterministic stage-family mapping. | Section 5 lines 506-528; AC 23-26 | Partially Implemented | `engine/src/steward/metrics.rs:46-150`, `engine/src/steward/metrics.rs:204-226`, `engine/src/steward/metrics.rs:252-326`, `domain/src/approval.rs:6-23` | Cost joins use `session_generations`, but `approval_rejection_rate` filters `approved` while the domain decision is `granted`; stage-family grouping is a string heuristic over stage IDs/types rather than frozen workflow snapshot truth; drift metrics read fields that no owner currently writes. |
| REQ-009 | Cohort classifier uses `(workflow_family, risk_class)` only, excludes legacy runs, and grades quality with project/stack facets. | Section 2a lines 48-75; Section 6b lines 545-587; AC 4, 7 | Partially Implemented | `engine/src/steward/cohort.rs:11-82`, `db/src/repos/runs.rs:113-124`, `engine/src/steward/service.rs:101-145`, gate pass | Eligibility and quality facets exist. Primary cohort selection picks the first eligible run in recency order, not an explicitly grouped/stable primary cohort selection, and the gate does not exercise multi-cohort selection. |
| REQ-010 | Deterministic pipeline writes canonical artifacts, handles inconclusive windows, and persists context dossiers. | Section 2a lines 41-64; Section 9 lines 779-809; AC 14-17 | Partially Implemented | `engine/src/steward/service.rs:147-212`, `engine/src/steward/json.rs:7-31`, `engine/src/steward/anomaly.rs:9-23`, gate pass | Inconclusive/context paths exist. Deterministic degradation artifacts include `analysis_id`, which is generated as a fresh UUID each run, so unchanged-data reruns are not byte-identical when signals exist. |
| REQ-011 | Optional active-catalog steward lanes support current `system_steward`/`steward_auditor` contract. | Section 2c lines 87-108; Section 3c lines 297-374; AC 18-22 | Partially Implemented | `engine/src/steward/service.rs:214-256`, `engine/tests/integration.rs:951-1021`, `engine/src/executor.rs:1045-1069` | A fake-test executor seam exists and captures output paths if files appear. Production `WorkItemKind::StewardAnalysis` calls `run_steward_analysis(...)`, which passes `None` for the agent executor, so real daemon execution never invokes `system_steward` or `steward_auditor`. |
| REQ-012 | Daemon-owned current inputs load canonical config/catalog sources, validate/fallback, hash effective parsed inputs, and schedule pending config-change only. | Section 7 lines 634-704; AC 11-13, 26, 31 | Partially Implemented | `daemon/src/main.rs:42-55`, `daemon/src/steward_runtime.rs:10-65`, `engine/src/steward/config.rs:87-197`, `examples/steward/steward_config.yaml:1-73`, gate pass | Env/default loading, fallback, effective hash, and pending flag exist. Validation does not enforce threshold method vocabulary or required V1 threshold families, `default_config()` has empty thresholds, and default path helpers guess cwd-relative paths despite the proposal's no-guessing rule. |
| REQ-013 | Manual, post-run, and config-change triggers converge on `WorkItemKind::StewardAnalysis`. | Section 10 lines 813-844; AC 30-31 | Implemented | `db/src/work_item.rs`, `engine/src/command_handler.rs:470-487`, `engine/src/orchestrator.rs:33-64`, `engine/src/orchestrator.rs:452-453`, `engine/src/executor.rs:1045-1069`, gate pass | Queue convergence exists; startup config-change sets pending only and completion consumes it first. |
| REQ-014 | Completed, inconclusive, and failed analyses are distinguishable northbound. | Section 8 lines 742-748; AC 30 | Partially Implemented | `domain/src/steward.rs:6-21`, `graphql-server/src/types/steward.rs:5-62`, `engine/src/steward/service.rs:85-300`, `rg StewardAnalysisStatus::Failed` | The enum and readback fields support `failed`, but the service never inserts a failed analysis row; errors before insert return `Err`, and `error_summary` is always `None` in service-produced rows. |
| REQ-015 | GraphQL exposes analysis and recommendation readback. | Section 8a lines 712-748; AC 27, 29 | Implemented | `graphql-server/src/schema.rs:128-190`, `graphql-server/src/types/steward.rs:5-125`, `graphql-server/src/schema.rs:1533-1638`, gate pass | Named queries return persisted analysis, links, recommendations, and artifact fields. |
| REQ-016 | MCP exposes manual trigger, list/get tools, and `steward-analysis://{analysis_id}` resource. | Section 8b-8c lines 749-775; AC 28-29 | Implemented | `mcp-server/src/tools/steward.rs:13-129`, `mcp-server/src/server.rs:276`, `mcp-server/src/server.rs:460-471`, `mcp-server/src/server.rs:1051-1141`, gate pass | Tools/resource exist and tests pass. Failed-analysis production is covered in REQ-014. |
| REQ-017 | Focused `proposal-049` proof gate proves the guarantee buckets. | Section 13 lines 951-988 | Partially Implemented | `scripts/test-gate.sh:1520-1532`, `./scripts/test-gate.sh proposal-049` passed | The gate exists and passes, but it does not prove byte-identical reruns, production steward-agent execution, multi-cohort primary selection, failed-analysis persistence, drift write ownership, or approval decision correctness. |

## Proposal Fidelity Inventory

### Matches

- Run creation now compiles workflow/catalog YAML and freezes workflow family, risk class, stack, project key, snapshot JSON, and snapshot hashes onto `Run`.
- Legacy completed runs missing P049 snapshot/cohort fields are excluded from analysis metrics and counted for observability.
- SQL/domain/repo support exists for steward analyses, run links, recommendations, runtime state, retry reasons, and widened run/idea fields.
- Deterministic analysis writes the expected `active-catalog-io/steward/...` artifact tree shape.
- Manual, post-run, and config-change triggers converge on the shared queue lane.
- GraphQL, MCP tools, and the MCP resource expose persisted steward analysis readback.
- The focused `proposal-049` gate passes.

### Divergences

- Production execution does not run active-catalog steward agents.
- Frozen snapshot hashing lacks explicit canonical sorted serialization over `HashMap`-backed parsed definitions.
- Deterministic signal artifacts include fresh UUID-derived `analysis_id`, breaking byte-identical rerun semantics.
- Drift fields are not written by recovery/resume classification.
- Approval rejection metrics use `approved` instead of the domain's `granted` decision string.
- Stage-family metrics use substring heuristics instead of frozen workflow snapshot mapping.
- Failed analysis rows are not persisted; `failed` exists as a read type but is not produced by service execution.
- Steward config validation/default semantics are weaker than the stable V1-aligned threshold contract.

### Ambiguities / Evidence Gaps

- P049 says primary cohort selection is "not left implicit" and must choose by `(workflow_family, risk_class)`, but does not spell out the exact tie-breaker/group selection algorithm. The implementation's "first eligible run by completed_at order" behavior is therefore a high-risk partial rather than a clean missing item.
- Artifact pointer fields are named `*_artifact_id`, but the implementation stores filesystem path strings. P049 allows "artifact IDs / paths" for MCP payloads, so this audit does not fail that item.
- Schedule trigger wiring is explicitly out of scope.

## Track 2: Expert Findings

### ARCH-001 - Production steward-agent lane is not wired

- Severity: Critical
- Confidence: High
- Related Requirements: REQ-011
- Evidence Types: code, tests-found
- Evidence References: `engine/src/executor.rs:1045-1069`, `engine/src/steward/service.rs:80-90`, `engine/src/steward/service.rs:214-256`, `engine/tests/integration.rs:951-1021`
- Why It Matters: P049 explicitly chooses active-catalog parity. The current code only proves a fake executor seam in tests; the real background executor always calls `run_steward_analysis(...)`, which delegates with `agent_executor = None`. Operators will never receive real `sdlc_health_report`, `agent_tuning_proposal`, `workflow_tuning_proposal`, `experiment_plan`, or `stewardship_audit_report` from daemon execution.
- Recommended Action: Add a production `StewardAgentExecutor` implementation backed by the existing ACP/catalog execution path, pass it from `BackgroundExecutor`, and extend the proposal gate with a non-fake wiring assertion that proves active catalog agents are invoked when configured.

### ARCH-002 - Frozen snapshot hashes are not proven canonical

- Severity: Major
- Confidence: High
- Related Requirements: REQ-005
- Evidence Types: code
- Evidence References: `workflow/src/compiler.rs:44-49`, `workflow/src/compiler.rs:217-227`, `workflow/src/definition.rs:13-18`, `workflow/src/catalog.rs:12-20`
- Why It Matters: P049 uses snapshot hashes as durable provenance. The implementation hashes `serde_json::to_string` output over parsed structs that contain `HashMap` fields. Without a named `DefinitionHasher` or explicit sorted canonical serialization, identical semantic workflow/catalog inputs can produce unstable byte ordering and therefore false provenance drift.
- Recommended Action: Introduce a workflow-owned canonical hasher that recursively sorts object keys before serialization, covers workflow and catalog parsed definitions, and add a cross-process/order-insensitive test that fails on raw `HashMap` serialization.

### ARCH-003 - Deterministic degradation artifacts contain a random analysis id

- Severity: Major
- Confidence: High
- Related Requirements: REQ-010, REQ-017
- Evidence Types: code
- Evidence References: `engine/src/steward/service.rs:91-99`, `engine/src/steward/anomaly.rs:9-23`, `engine/src/steward/anomaly.rs:25-33`, `engine/src/steward/service.rs:186-199`
- Why It Matters: AC 15 requires unchanged-data reruns to produce byte-identical deterministic JSON artifacts. `analysis_id` is a new UUID per run and is embedded in each degradation signal written to deterministic alert artifacts. Any signal-producing rerun will differ even if DB truth and current inputs are unchanged.
- Recommended Action: Remove run-instance identifiers from deterministic signal payloads or move them to non-deterministic metadata outside the deterministic artifact set. Add a gate test that runs the deterministic slice twice on identical seeded data and byte-compares the deterministic artifact contents.

### ARCH-004 - Drift ownership is schema-only

- Severity: Major
- Confidence: High
- Related Requirements: REQ-006, REQ-008
- Evidence Types: code
- Evidence References: `domain/src/run.rs:140-143`, `db/src/repos/runs.rs:8-74`, `engine/src/steward/metrics.rs:143-146`, `rg drift_detected_at` over engine/db/domain sources
- Why It Matters: P049 says recovery/resume classification writes drift fields when a run is drifted or requires operator remap. The current implementation creates and reads the fields but never updates them. `drift_event_count` will therefore remain false-zero for real drift.
- Recommended Action: Add the recovery/resume writer path, persist a canonical drift details payload, and add tests that induce/remap drift and verify both run fields and Steward metrics.

### ARCH-005 - Metric-source implementation has incorrect approval and stage-family semantics

- Severity: Major
- Confidence: High
- Related Requirements: REQ-008
- Evidence Types: code
- Evidence References: `engine/src/steward/metrics.rs:204-226`, `domain/src/approval.rs:6-23`, `engine/src/steward/metrics.rs:311-326`, Proposal lines 515-518
- Why It Matters: `approval_rejection_rate` ignores granted approvals because it filters for `approved`, a value not produced by `ApprovalDecision`. Stage-family metrics are grouped by substring heuristics instead of frozen workflow snapshot state mapping. This can create false rejection rates and misclassified proposal/implementation/audit metrics.
- Recommended Action: Use `granted`/`rejected` for approval rates and derive stage family from frozen workflow snapshot semantics or an explicit compiled stage-family field. Add tests with granted+rejected approvals and non-obvious stage IDs.

### ARCH-006 - Primary cohort selection remains implicit in multi-cohort datasets

- Severity: Major
- Confidence: Medium
- Related Requirements: REQ-009
- Evidence Types: code, tests-found
- Evidence References: `db/src/repos/runs.rs:113-124`, `engine/src/steward/cohort.rs:37-47`, `engine/src/steward/service.rs:101-145`, `engine/src/steward/cohort.rs:127-145`
- Why It Matters: The proposal says primary cohort selection is not implicit and all downstream windowing/signal/recommendation continuity inherits that split. The current selector uses the first eligible run from recency-ordered DB results, so a small latest cohort can displace a larger stable cohort. Existing tests cover quality rules, not multi-cohort selection.
- Recommended Action: Define and implement an explicit grouping/tie-breaker rule, preferably largest eligible cohort then deterministic tie-break by recency/key, and add a multi-cohort test.

### ARCH-007 - Failed analysis status is not service-produced

- Severity: Major
- Confidence: High
- Related Requirements: REQ-014, REQ-015, REQ-016
- Evidence Types: code
- Evidence References: `domain/src/steward.rs:6-21`, `engine/src/steward/service.rs:85-300`, `rg StewardAnalysisStatus::Failed`
- Why It Matters: Northbound types can represent `failed`, but runtime failures before insert return an error and produce no `steward_analyses` row. Operators cannot distinguish failed analyses from missing analyses, which violates the P049 readback contract.
- Recommended Action: Wrap analysis execution in a failure-recording boundary that inserts a failed analysis row with `error_summary` and any available artifact pointers before returning/settling the work item.

### ARCH-008 - Steward config validation/defaults are weaker than the V1-aligned contract

- Severity: Major
- Confidence: High
- Related Requirements: REQ-012
- Evidence Types: code
- Evidence References: `engine/src/steward/config.rs:87-130`, `engine/src/steward/anomaly.rs:176-180`, `examples/steward/steward_config.yaml:9-23`, Proposal lines 602-607 and 668-676
- Why It Matters: Invalid config fallback works, but the default config has no thresholds and validation does not reject unsupported methods or missing threshold families. The anomaly detector ignores threshold method and falls back to a hard-coded `0.2`, which diverges from the stable V1 threshold semantics P049 references.
- Recommended Action: Move the example V1 threshold family defaults into `default_config()`, validate method names and required threshold families, and make detector behavior method-aware.

### PROD-001 - Real users will see deterministic records without the promised steward synthesis

- Severity: Major
- Confidence: High
- Related Requirements: REQ-011
- Evidence Types: code
- Evidence References: `engine/src/executor.rs:1045-1069`, `engine/src/steward/service.rs:214-256`
- Why It Matters: The core product value of Steward is not just metrics storage; it is actionable health/tuning/audit synthesis. Since production does not run the steward agents, the most visible product outputs are absent except in synthetic tests.
- Recommended Action: Treat active steward agent execution as release-blocking for P049 unless the product explicitly downgrades P049 to deterministic-only.

### PROD-002 - Incorrect metrics can create misleading recommendations

- Severity: Major
- Confidence: High
- Related Requirements: REQ-008
- Evidence Types: code
- Evidence References: `engine/src/steward/metrics.rs:204-226`, `engine/src/steward/metrics.rs:311-326`, `engine/src/steward/service.rs:331-355`
- Why It Matters: Recommendations are generated from degradation signals. If approval rates and stage families are wrong, Steward can recommend action for a nonexistent process problem or miss a real bottleneck.
- Recommended Action: Fix metric source semantics before relying on recommendations for operational decisions.

### UI

No proposal-scoped UI findings. P049 explicitly excludes a dedicated Steward dashboard in Section 14 lines 992-995, and the audited implementation is control-plane/daemon/GraphQL/MCP work.

### UX-001 - Failure and drift readback do not support operator recovery UX

- Severity: Major
- Confidence: High
- Related Requirements: REQ-006, REQ-014
- Evidence Types: code
- Evidence References: `engine/src/steward/service.rs:85-300`, `engine/src/steward/metrics.rs:143-146`, `rg drift_detected_at`
- Why It Matters: Operators reading GraphQL/MCP cannot tell whether an analysis failed, never existed, or ran without drift because failure rows and drift writers are missing. This weakens the operational UX even though the read APIs exist.
- Recommended Action: Persist failed analysis rows and drift details, then expose those states in list/get/resource tests with concrete operator-facing payload examples.

### READY-001 - Passing focused gate is insufficient proof of P049 readiness

- Severity: Major
- Confidence: High
- Related Requirements: REQ-017
- Evidence Types: tests-run, tests-found, code
- Evidence References: `scripts/test-gate.sh:1520-1532`, `./scripts/test-gate.sh proposal-049` passed, findings ARCH-001 through ARCH-008
- Why It Matters: The focused gate passes but misses several proposal acceptance guarantees: real active-catalog agent execution, byte-identical reruns, multi-cohort primary selection, failed-analysis persistence, drift writer ownership, approval decision correctness, and canonical definition hashing.
- Recommended Action: Expand `proposal-049` gate before declaring readiness. Add narrow tests for each gap above, then run the full canonical regression gate before any Green/Ready verdict.

## Readiness Roll-Up

P049 should not be marked ready on this tree. The implementation has the right broad skeleton and passes its current focused gate, but several core guarantees are incomplete or unproven.

Minimum closeout before Green:

- Wire real production active-catalog steward agent execution.
- Replace workflow/catalog snapshot hashing with explicit sorted canonical hashing and a named owner.
- Remove random analysis IDs from deterministic artifact contents or exclude them from deterministic artifacts by contract.
- Add drift writer ownership in recovery/resume classification.
- Fix approval decision and stage-family metric semantics.
- Make failed analysis persistence real.
- Tighten steward config validation/defaults to V1 threshold semantics.
- Expand `proposal-049` proof gate to catch these cases, then run full regression on the same tree.
