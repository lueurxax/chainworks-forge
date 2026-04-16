# Output Contracts, Failure Evidence, and Narrow Recovery Proof

Current implementation and proof status for the stable output-contract, structured-output validation, validation-failure evidence, typed reader, retry-lineage, and narrow recovery slice consolidated from the older Proposal 013 scope and the later Rust control-plane work that had been tracked by Proposal 046.

## Status

| Field | Value |
|---|---|
| Slice | Output Contracts, Failure Evidence, and Narrow Recovery |
| Source contracts | [../reference/structured-output-envelope-and-contract-validation.md](../reference/structured-output-envelope-and-contract-validation.md), [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md) |
| Current implementation status | Implemented |
| Current readiness | Ready |
| Primary proof owners | ACP integration tests, workflow integration tests, engine integration tests, DB integration tests, GraphQL artifact tests, MCP report/resource tests, `cargo test --workspace` in `control-plane/` |
| Last consolidated documentation refresh | `2026-04-15` |

## What is considered proven

The accepted proof story for this slice supports these claims:

- ACP extracts named `CHAINWORKS_OUTPUT` envelopes instead of relying only on filesystem diffs,
- envelope-owned outputs override same-name filesystem discoveries,
- discovered outputs bind through the full resolver chain, including explicit contract, normalized/raw identity, version fallback, and stem fallback,
- declared machine artifacts persist under normalized canonical identity rather than task-alias-only naming,
- undeclared envelope outputs are persisted rather than dropped,
- raw outputs survive long enough for inspection even when validation fails afterward,
- `ValidationFailureRecord` is durable, typed, and attempt-scoped,
- projection-backed `has_validation_failure` remains scoped to exact `stage_execution_id`,
- GraphQL artifacts, MCP `reports.get`, and MCP `report://{run_id}` decode the full validation-failure payload,
- and same-tree workspace regression coverage passed for the Rust control plane.

## Accepted current-head proof owners

The strongest current-head proof owners for this slice are:

- `control-plane/crates/acp/tests/integration.rs`
- `control-plane/crates/workflow/tests/integration.rs`
- `control-plane/crates/engine/tests/integration.rs`
- `control-plane/crates/db/tests/integration.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `control-plane/crates/mcp-server/src/server.rs`
- `cargo test --workspace` from `control-plane/`

High-signal proof examples on the current tree include:

- envelope extraction without filesystem artifacts,
- explicit-contract plus normalized/raw/version/stem contract binding,
- persisted undeclared envelope outputs,
- persisted declared machine artifacts under normalized names,
- missing-output failures mapping to `operator_inspection`,
- retry isolation where a failed first attempt does not smear `has_validation_failure` onto a later retry,
- GraphQL artifact decoding of `ValidationFailureRecord`,
- MCP `reports.get` decoding of `ValidationFailureRecord`,
- and `report://{run_id}` decoding of `ValidationFailureRecord`.

## Canonical verification landmarks

Representative current-head proof points include:

- `cargo test -p acp test_claude_adapter_extracts_chainworks_output_envelopes_without_filesystem_artifacts -- --exact`
- `cargo test -p workflow test_contract_binding_uses_versioned_and_stem_fallbacks -- --exact`
- `cargo test -p engine test_invoke_agent_persists_undeclared_envelope_output_as_generic_artifact -- --exact`
- `cargo test -p engine test_invoke_agent_persists_declared_machine_artifact_under_normalized_name -- --exact`
- `cargo test -p db stage_projection_validation_flag_is_attempt_scoped -- --exact`
- `cargo test --workspace`

These landmarks are examples, not an exclusive list. The stable proof story is the combined current-head behavior across the owners listed above.

## Consolidation note

The old Proposal 046 draft, review, evidence pack, and proposal-local implementation audits were implementation-trail artifacts.

Their durable content now lives in:

- [../reference/structured-output-envelope-and-contract-validation.md](../reference/structured-output-envelope-and-contract-validation.md)
- [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md)
- this proof document

This slice should be treated as implemented reference behavior rather than as an active proposal dependency.

## Remaining caution

No slice-level blocker remains.

Normal implementation risk still exists in future edits:

- do not regress northbound readers back to metadata-only failure summaries,
- do not flatten attempt-scoped validation truth to logical-stage scope,
- and do not bypass normalized artifact identity with alias-only persistence for declared machine outputs.

Those are maintenance watchpoints, not open readiness gaps.

## Recommended usage

Use:

- [../reference/structured-output-envelope-and-contract-validation.md](../reference/structured-output-envelope-and-contract-validation.md) for the canonical structured-output and contract-validation substrate,
- [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md) for durable failure evidence, narrow recovery, and declarative contract governance,
- [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) for canonical outcome and attempt-scoped recovery truth,
- [../reference/rust-control-plane.md](../reference/rust-control-plane.md) for daemon topology and northbound boundary ownership,
- [../reference/test-gates.md](../reference/test-gates.md) for broader verification lanes.
