# P049 Implementation Audit R4

Date: 2026-04-16 12:12:39 +0300

Audited tree: `af3054c` plus dirty working tree

Proposal: `docs/proposals/049-steward-analysis-system.md`

Supersedes: `049-steward-analysis-system_IMPLEMENTATION_AUDIT_R3.md`

## Verdict

- Overall Conformance: Implemented
- Overall Readiness: Ready with Risks
- Audit Confidence: High

The R3 blockers have been closed in the current working tree. The remaining readiness caveat is repository hygiene: the checkout is intentionally very dirty because adjacent proposal slices and documentation archival work are also present. Freeze with a commit before treating this as a release baseline.

## R3 Finding Closure

### Production active-catalog Steward agents

Closed.

`WorkItemKind::StewardAnalysis` no longer calls the deterministic service with `agent_executor = None`. `BackgroundExecutor` now owns a production `StewardAgentExecutor` backed by the active agent catalog and `AcpRuntimeManager`, resolves `system_steward` and `steward_auditor`, roots their IO under `active-catalog-io`, and invokes them through the existing ACP execution path.

Focused proof:

- `steward_executor_tests_work_item_runs_active_catalog_agents_through_acp`
- `bash ./scripts/test-gate.sh proposal-049`

### Canonical snapshot hashing

Closed.

Workflow and catalog snapshot serialization now explicitly sorts JSON object keys before hashing. This removes dependence on `HashMap` iteration or YAML field ordering.

Focused proof:

- `steward_metadata_contract_tests_snapshot_hashes_are_canonical_over_yaml_ordering`
- `bash ./scripts/test-gate.sh proposal-049`

### Deterministic artifacts

Closed.

Deterministic degradation alert payloads no longer serialize the fresh UUID-backed `analysis_id`. Re-running the deterministic slice on unchanged input produces byte-identical `degradation-alerts.json` content.

Focused proof:

- `steward_pipeline_tests_detects_degradation_and_persists_recommendation`
- `cargo test -p engine steward -- --nocapture`

### Drift and failed-analysis semantics

Closed.

Startup recovery now persists run-owned drift truth through `runs.drift_detected_at` and `runs.drift_details_json`. The Steward analysis service now records a `failed` `steward_analyses` row with `error_summary` when the deterministic slice fails before a completed row can be persisted.

Focused proof:

- `steward_drift_tests_startup_repair_clears_stuck_running_stage_and_marks_drift`
- `steward_pipeline_tests_persists_failed_analysis_when_deterministic_slice_errors`
- `cargo test -p engine steward -- --nocapture`

### Approval and stage-family metrics

Closed.

`approval_rejection_rate` now uses the domain decisions `granted` and `rejected`. Stage-family metrics now derive stage family from frozen workflow snapshot state truth before falling back to legacy text heuristics.

Focused proof:

- `steward_metrics_tests_use_domain_decisions_and_frozen_stage_families`
- `cargo test -p engine steward -- --nocapture`

### Cohort grouping

Closed.

Primary cohort selection now groups eligible runs by the P049 primary tuple `(workflow_family, risk_class)`, selects the largest cohort deterministically, and leaves `project_key` / `stack` as quality facets only.

Focused proof:

- `steward_cohort_classifier_tests_primary_key_uses_largest_explicit_cohort`
- `cargo test -p engine steward -- --nocapture`

### Steward config threshold ownership

Closed.

Default Steward config now carries non-empty threshold families. Validation rejects missing required threshold families, unsupported threshold methods, and non-positive trigger values. The anomaly detector no longer silently falls back to a hard-coded threshold when no configured metric or family threshold exists.

Focused proof:

- `steward_runtime_bootstrap_tests_invalid_config_falls_back_to_default`
- `bash ./scripts/test-gate.sh proposal-049`

## Verification

Passed:

- `cargo test -p workflow steward_metadata_contract_tests -- --nocapture`
- `cargo test -p engine steward -- --nocapture`
- `bash ./scripts/test-gate.sh proposal-049`
- `cargo test --workspace`
- `git diff --check`
- `rg -n "^(<<<<<<<|=======|>>>>>>>)" . --glob '!control-plane/target/**' --glob '!**/.git/**'`

## Readiness Risk

The implementation is proposal-complete, but this checkout is not a clean isolated P049 branch. It contains concurrent P047, P048, MCP/auth, and documentation/archive changes. That does not invalidate P049 conformance, but it does mean release readiness should be based on a committed/frozen tree rather than the current mutable working directory.
