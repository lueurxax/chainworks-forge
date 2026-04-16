# P049 Steward Analysis System Implementation Plan

## Goal

Implement `docs/proposals/049-steward-analysis-system.md` in the Rust control plane until the proposal-owned slice is complete and proved by a canonical `proposal-049` gate.

## Constraints

- Preserve the existing dirty files outside P049 unless a P049 requirement directly touches them.
- Use current Rust daemon/control-plane owners, not Swift-only runtime state.
- Freeze run cohort metadata and parsed definition snapshots at run creation; Steward analysis must not recompute them from mutable YAML.
- Keep Swift V1 parity where the proposal requires it: config-change startup only sets pending work, manual/post-run/config-change converge on `WorkItemKind::StewardAnalysis`, and optional LLM lanes are non-blocking.

## Task Breakdown

1. Add failing tests for workflow metadata/snapshot compilation, run-start frozen metadata persistence, idea `project_key`, stage `retry_reason`, and `StewardAnalysis` work item roundtrips.
2. Implement domain, migration, repo, compiler, GraphQL, and MCP start/read surfaces for frozen run metadata and parsed snapshot provenance.
3. Add deterministic Steward domain/repo/service modules: cohort classifier, dossier builder, canonical JSON writer, analysis persistence, legacy-pre-P049 exclusion, inconclusive windows, and bounded context dossiers.
4. Wire triggers: post-run completion, manual MCP trigger, config-change pending enqueue path, and executor dispatch for `WorkItemKind::StewardAnalysis`.
5. Add GraphQL and MCP readback for analyses and recommendations, including `steward-analysis://{analysis_id}` resource parity.
6. Add daemon bootstrap config validation/fallback/hash tests and minimal runtime owner for config-change pending work.
7. Add `proposal-049|p049` to `scripts/test-gate.sh` and `docs/reference/test-gates.md`, then iterate until the gate passes.

## Verification

Run the focused gate first:

```bash
bash ./scripts/test-gate.sh proposal-049
```

Run narrower package tests while iterating:

```bash
cargo test -p workflow steward_metadata_contract_tests
cargo test -p engine steward_pipeline_tests steward_cohort_classifier_tests steward_legacy_pre_p049_eligibility_tests steward_trigger_tests
cargo test -p graphql-server steward_graphql_readback_tests
cargo test -p mcp-server steward_mcp_tools_tests
```
