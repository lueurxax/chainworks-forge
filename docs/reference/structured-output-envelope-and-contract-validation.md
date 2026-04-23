# Structured Output Envelopes and Contract Validation

Stable reference for the Rust control-plane slice that parses named ACP output envelopes, binds discovered outputs to compiled contract metadata, validates structured payloads, and persists durable validation-failure evidence.

## Purpose

The daemon must treat named structured outputs as first-class runtime artifacts rather than as incidental filesystem side effects.

For this slice, the system must be able to answer:

- which named outputs the agent emitted,
- which compiled contract metadata was authoritative for each output,
- which canonical path and artifact identity the runtime persisted,
- whether validation passed, failed, or was intentionally bypassed,
- and which durable failure record report and recovery readers must consume when validation fails.

## Scope

This reference covers:

- `<<<CHAINWORKS_OUTPUT:name>>>...<<<END_CHAINWORKS_OUTPUT>>>` envelope extraction from ACP responses,
- canonical rebinding of discovered outputs to compiled plan metadata,
- contract resolution and validation-mode enforcement,
- companion-artifact handling for machine-plus-human outputs,
- persistence ordering for raw outputs, validation, and failure evidence,
- undeclared envelope output persistence,
- and the implemented owner chain across ACP, workflow compilation, executor persistence, and northbound readers.

It does not replace:

- broader failed-stage evidence and narrow recovery policy in [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md),
- stage settlement and canonical outcome precedence in [execution-truth-and-recovery.md](execution-truth-and-recovery.md),
- daemon topology in [rust-control-plane.md](rust-control-plane.md),
- or frozen run/artifact storage boundaries in [runtime-contract.md](runtime-contract.md).

## Core Rules

### Envelope outputs are canonical named discoveries

ACP responses may contain named structured output blocks:

```text
<<<CHAINWORKS_OUTPUT:proposal_review_summary>>>
{"pass": true, "average_score": 9.55}
<<<END_CHAINWORKS_OUTPUT>>>
```

The transport layer extracts these blocks into named discovered artifacts.

#### Output Caps and Rejection (P053)

Declared-output byte caps and aggregate caps now apply to both provider envelopes and `CHAINWORKS_OUTPUT` payloads. Over-cap outputs are rejected with explicit reasons (e.g., `oversized_rejection`), preventing parser-buffer exhaustion and unbounded storage growth.

#### Discovery Settlement Pipeline (P053)

P053 replaces implicit artifact inference with an engine-owned discovery settlement pipeline. Outputs are no longer guessed from basename heuristics alone. Instead, the pipeline uses:

- typed `ExpectedOutputSpec`,
- bounded pre-prompt metadata,
- and explicit `OutputDiscoveryDecision` records.

If the same canonical output name appears in both:

- an extracted envelope, and
- a bounded filesystem discovery,

the envelope-owned discovery wins.

### Discovery binds to compiled metadata before persistence

Persistence is not allowed to invent contract metadata from filenames alone when the compiled task already declared the output.

Before persistence, declared outputs are rebound to the compiled plan through:

- `CompiledTask.outputs`,
- `CompiledTask.output_schemas`,
- and `RunPlan.artifact_paths`.

That binding determines:

- canonical artifact name,
- canonical target path,
- contract identifier,
- validation mode,
- machine format,
- human companion format,
- raw artifact alias,
- and normalized artifact identity.

### Contract authority remains singular

`AgentCatalog.contracts` is the only post-generation contract authority for this slice.

The runtime does not keep:

- a second validation registry,
- a transport-specific contract table,
- or a persistence-time heuristic that outranks compiled catalog truth.

### Contract resolution follows the ordered resolver chain

When a discovered output needs contract binding, the runtime resolves it in this order:

1. explicit `output_contract`,
2. exact normalized artifact name,
3. exact raw artifact name,
4. version-suffix fallback (`_vN`, `-vN`, `_VN`, `-VN`),
5. stem fallback.

If the chain still misses, the output is treated as undeclared rather than falsely claiming a contract.

### Validation modes are explicit

The implemented validation modes are:

- `human_only`
- `strict_structured`
- `structured_with_human_companion`

Rules:

- `human_only` bypasses machine validation.
- `strict_structured` requires the machine payload to satisfy the declared machine format and required fields.
- `structured_with_human_companion` requires both:
  - a valid machine payload under the normalized artifact identity, and
  - a human companion artifact under the raw companion name.

If the machine payload is missing, invalid, or structurally incomplete, the output fails validation even if prose or markdown exists beside it.

### Canonical artifact identity follows normalized machine names

For declared structured outputs, the persisted machine artifact identity follows the resolved `normalized_artifact_name`, not the task alias that happened to request it.

The raw or companion artifact keeps its raw identity when the contract declares one.

This prevents the runtime from drifting into two competing machine names for the same contract.

### Undeclared envelope outputs still persist

Named envelope outputs that are not declared by the compiled task are not dropped.

They persist as undeclared artifacts under a dedicated generic lane so operators and downstream tooling can still inspect what the agent returned.

Undeclared persistence does not fabricate a declared contract or validation success.

### Persistence order is fixed

The executor follows this order:

1. persist raw outputs at canonical paths,
2. validate declared outputs against compiled schemas,
3. persist `ValidationFailureRecord` evidence for failed validations,
4. settle the work item and stage.

Raw outputs always survive long enough for inspection, even when validation fails afterward.

## Owner Chain

### ACP transport

The ACP layer owns envelope extraction from session response text and returns named discovered artifacts to the executor.

Relevant owners:

- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/acp/src/manager.rs`

### Workflow parser and compiler

The workflow layer owns contract-schema ingestion and compiled propagation of:

- `validation_mode`,
- `machine_format`,
- `human_format`,
- `raw_artifact_name`,
- `normalized_artifact_name`,
- and explicit `output_contract`.

Relevant owners:

- `control-plane/crates/workflow/src/catalog.rs`
- `control-plane/crates/workflow/src/compiler.rs`
- `control-plane/crates/workflow/src/plan.rs`

### Executor

The executor owns:

- rebinding discoveries to canonical plan metadata,
- materializing declared outputs to canonical paths,
- persisting undeclared envelope artifacts,
- running validation,
- building `ValidationFailureRecord`,
- and persisting validation-failure artifacts before settlement.

Relevant owners:

- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/engine/src/contracts.rs`
- `control-plane/crates/engine/src/orchestrator.rs`

### Durable failure storage and northbound readers

Typed failure payloads are durably stored in `validation_failure_records.record_json`.

Current northbound readers decode that persisted typed record rather than reconstructing it from artifact metadata:

- GraphQL artifact reads,
- MCP `reports.get`,
- MCP `report://{run_id}`,
- and stage projections via `has_validation_failure` for lightweight stage status.

Relevant owners:

- `control-plane/crates/db/src/repos/validation.rs`
- `control-plane/crates/db/src/repos/projections.rs`
- `control-plane/crates/graphql-server/src/types/artifact.rs`
- `control-plane/crates/graphql-server/src/types/stage.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `control-plane/crates/mcp-server/src/server.rs`

## Validation Failure Evidence

When validation fails, the canonical durable evidence path includes:

- the raw output artifact,
- the validation-failure artifact,
- the typed `ValidationFailureRecord`,
- and the stage-owned projection flag `has_validation_failure`.

The typed failure record is the authoritative source for:

- failure summary,
- missing fields,
- contract metadata,
- recovery recommendation,
- session reuse disposition,
- and session reset reason.

Reader surfaces must prefer this record over metadata-only reconstruction.

## Retry and Attempt Scope

Validation failure is attempt-scoped.

The durable record and the stage projection join by exact `stage_execution_id`, so a failed earlier attempt does not smear `has_validation_failure` onto a later retry in the same logical stage lineage.

Attempt scope belongs to the stage/execution truth layer and must remain aligned with [execution-truth-and-recovery.md](execution-truth-and-recovery.md).

## Verification Landmarks

High-signal current-head proof owners for this slice include:

- ACP envelope extraction tests in `control-plane/crates/acp/tests/integration.rs`,
- workflow contract-binding tests in `control-plane/crates/workflow/tests/integration.rs`,
- executor persistence and validation tests in `control-plane/crates/engine/tests/integration.rs`,
- projection retry-isolation tests in `control-plane/crates/db/tests/integration.rs`,
- GraphQL artifact payload tests in `control-plane/crates/graphql-server/src/schema.rs`,
- MCP `reports.get` payload tests in `control-plane/crates/mcp-server/src/tools/reports.rs`,
- MCP `report://{run_id}` payload tests in `control-plane/crates/mcp-server/src/server.rs`,
- and same-tree regression coverage from `cargo test --workspace` in `control-plane/`.

Use the proof document for the consolidated status and exact proof story.

## Adjacent References

Use:

- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md) for failed-stage evidence, narrow recovery, and declarative contract governance,
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) for canonical outcomes, stage-owned recovery evidence, and attempt-scoped settlement,
- [rust-control-plane.md](rust-control-plane.md) for daemon topology and northbound boundary shape,
- [runtime-contract.md](runtime-contract.md) for frozen run and artifact storage rules,
- [test-gates.md](test-gates.md) for the current verification lanes.
