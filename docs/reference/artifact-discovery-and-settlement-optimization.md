# Artifact Discovery and Settlement Optimization

Stable reference for the bounded discovery model, discovery filesystem ownership, pre-prompt metadata capture, and engine-owned settlement pipeline.

## Purpose

Fresh ACP startup and per-execution discovery must be bounded and efficient.

The system must be able to:

- remove broad filesystem discovery from the pre-`initialize` path,
- restrict discovery to the run-owned meta-root and explicitly declared expected outputs,
- capture bounded pre-prompt metadata per-execution,
- and settle discovered artifacts through a typed discovery-decision pipeline.

## Scope

This reference covers:

- `domain::discovery` and `DiscoveryFilesystem` ownership,
- bounded discovery rules for meta-root and exact-path outputs,
- pre-prompt metadata capture semantics,
- engine-owned `OutputDiscoveryDecision` and settlement logic,
- `AgentOutputSettlement` truth,
- and legacy broad-discovery opt-in.

It does not define:

- lower-layer ACP transport details in [acp-runtime-transport.md](acp-runtime-transport.md),
- higher-layer orchestrator topology in [workflow-execution-engine.md](workflow-execution-engine.md),
- operator-facing recovery presentation in [operator-experience.md](operator-experience.md),
- or macOS operator UI rendering for these diagnostics. The UI owner is [Proposal 069](../proposals/069-bounded-discovery-diagnostics-operator-ui.md), which is blocked by [Proposal 031](../proposals/031-thin-graphql-ui-rewrite.md).

## Operator UI Deferral

The implemented reference truth is the control-plane discovery and settlement model plus durable readback. The macOS operator UI for discovery diagnostics is intentionally owned separately.

Proposal 069 owns the UI surfaces for missing/stale/rejected outputs, discovery mode, startup timing, cap warnings, source changes, Copy Path, Open Location, accessibility labels, and Dynamic Type behavior. That UI must build on the P031 thin UI boundary and consume GraphQL read projections only. The macOS UI must not use MCP, direct SQLite reads, local artifact scanning, or Swift-local workflow truth for discovery diagnostics.

## Bounded Discovery Model

The runtime replaces implicit broad discovery with a bounded model. Discovery is no longer a pre-handshake global scan.

### DiscoveryFilesystem Ownership

The `DiscoveryFilesystem` logic lives in `control-plane/crates/domain/src/discovery.rs`. It provides the value-types and filesystem primitives used by both the engine and ACP adapters.

Policy construction and high-level discovery coordination remain engine-owned.

### Discovery Boundaries

| Boundary | Rule |
|---|---|
| **Meta-root** | Discovery is restricted to the run-owned meta-root directory. |
| **Exact-path outputs** | Only declared expected output paths are read back from the workspace/worktree. |
| **Broad discovery** | Repository, workspace, and worktree-wide scanning is removed from the default path. |

### Generated-State Exclusion Denylist

Discovery must maintain an explicit denylist for any fallback traversal or diagnostic scan. This prevents performance and correctness issues when encountering massive generated trees.

Denylist includes:

- `.chainworks/worktrees`
- `.chainworks/backups`
- `.forge-codex-acp`
- `.claude/worktrees`
- `.git/objects`
- `**/target`
- `DerivedData`, `.build`, `node_modules`
- `.chainworks/*.db.backup-*`, `.chainworks/*.sqlite`

### Housekeeping Policy

Discovery must avoid relying on cleanup for correctness, but the system maintains a housekeeping policy to manage generated-state growth.

| Category | Policy |
|---|---|
| **Worktrees** | Cleanup may remove generated build outputs (e.g., `target/`) only for inactive, cancelled, archived, or stale run worktrees. Must not delete source files, `.git` metadata, or active run outputs. |
| **ACP Runtime Homes** | Cleanup may remove stale `.forge-codex-acp/<session>` directories when no live provider/MCP process references them. |
| **DB Backups** | Must keep the current DB, WAL/SHM files, the newest pre-cleanup backup, and any operator-pinned backup. Older migration/test backups may be pruned. |

### Pre-Prompt Metadata Capture

Metadata capture is now a per-execution, per-prompt-turn step.

- **Fresh Sessions**: Capture occurs before the first prompt.
- **Reused Sessions**: Capture occurs before each subsequent prompt turn to ensure context remains fresh.
- **Bounds**: Metadata capture follows the same byte and aggregate cap model as declared outputs.

#### Metadata Capture Bounds

- **Maximum expected-output specs**: 200 per agent execution.
- **Aggregate digest-read budget**: 64 MiB.
- **Capture timeout**: 5 seconds per agent execution (continues to prompt on timeout).

## Expected Output Specs

`ExpectedOutputSpec` is the engine-to-runtime discovery contract.

- **Required fields**: `output_name`, `output_role` (machine/companion/control_plane), `target_path`, `display_label`, `required`, `reuse_policy` (must_produce/allow_unchanged_existing), `max_bytes`, `aggregate_acceptance_cap_bytes`, `authorized_roots`, `source_generation_owner` (agent/control_plane).
- **Optional fields**: `companion_of`, `contract_id`.
- **Authorized roots**: Restricted based on output class (worktree for repo outputs, meta-root for run artifacts, etc.). Wrong-root or wrong-run paths are rejected.

## Settlement Pipeline

The engine-owned settlement pipeline converts discovery decisions into active artifact truth.

### Output Caps and Rejection

Declared-output byte caps and aggregate caps apply to provider envelopes, `CHAINWORKS_OUTPUT` payloads, and exact-path outputs. Over-cap outputs are rejected with `oversized_rejection`.

### Output Discovery Decisions

`OutputDiscoveryDecision` records are built by the engine discovery pipeline and stored on runtime facts. They describe what was found, whether it was expected, and how it was handled.

### Agent Output Settlement

`AgentOutputSettlement` describes the outcome of the discovery and settlement process for an agent execution.

| Settlement Value | Meaning |
|---|---|
| `none` | No outputs were expected or discovered. |
| `valid_outputs_from_completed_execution` | All required outputs were found and are contract-valid. |
| `valid_outputs_from_failed_execution` | Outputs were found and valid, but the execution itself failed. |
| `missing_required_outputs` | One or more required outputs were not found. |
| `invalid_required_outputs` | Required outputs were found but failed contract validation. |
| `ignored_late_outputs` | Outputs arrived after the source claim was closed or superseded. |

### Discovery Diagnostics

The discovery process produces a `DiscoveryDiagnosticsV1` payload stored in the `agent_execution_discovery_diagnostics` table. This allows operators to inspect:

- **Decisions**: The full list of `OutputDiscoveryDecision` records for the execution.
- **Pre-prompt Metadata**: The state of expected outputs before the prompt was submitted.
- **Bounding Statistics**: Files visited and total bytes scanned in the meta-root.
- **Truncation Flags**: Whether discovery was truncated by file count, file size, or aggregate byte caps.
- **Legacy Policy Usage**: Whether legacy broad discovery was active for the run.


## Phase 1 Minimal Readback Path

For runs that are production-exposed in Phase 1, the system provides a minimal durable discovery-decision projection. This is the stable route for support and operator diagnosis before the full Phase 2 diagnostics land.

- **Storage**: Written by `settle_agent_outputs_from_discovery_decisions` into the run evidence path.
- **Consumption**: Exposed for server-owned diagnostics, MCP/agent readback, run report diagnostics, and future P069 UI rendering through GraphQL projections.
- **Fields**: `output_name`, `target_path`, `status`, `reason`, `provenance`, `size_bytes`, `decision_at`.


## Legacy Broad Discovery Policy

Workflows can opt-in to legacy broad discovery behavior for compatibility during the transition.

```yaml
discovery:
  legacy_broad_discovery_policy: workflow_opt_in
```

- `disabled` (default): Only bounded discovery is performed.
- `workflow_opt_in`: Permits broader scanning for the specific workflow.

### Legacy Discovery Overrides

Operators can override the discovery policy for a specific retry attempt through the `legacy_discovery_overrides` table. This is used when a workflow-level policy is `disabled` but a specific execution requires broad discovery for recovery.

- **Storage**: `legacy_discovery_overrides` table.
- **Status**: `pending`, `consumed`, `rejected`, `expired`.
- **Consumption**: The engine checks for a `pending` override matching the run, stage, and attempt before starting the discovery pipeline.

## Current Implementation Owners

- `control-plane/crates/domain/src/discovery.rs`
- `control-plane/crates/acp/src/session.rs`
- `control-plane/crates/engine/src/contracts.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/db/src/repos/agent_execution_discovery_diagnostics.rs`
- `Chainworks Forge/DSL/WorkflowDefinition.swift`

## Verification Baseline

- `control-plane/crates/db/tests/proposal_053_discovery_diagnostics.rs`
- `control-plane/crates/acp/tests/integration.rs`
- `Chainworks ForgeTests/ArtifactValidationTests.swift`
