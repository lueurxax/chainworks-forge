# Failed-Stage Evidence, Delivery Preflight, and MCP Resolution

This document is the stable implementation reference for the Rust control-plane failed-stage evidence, delivery-preflight, and MCP-resolution slice.

It covers three related owner chains:

- failed-stage evidence packets and stage-owned recovery truth
- delivery preflight for repo-backed run start
- execution-time MCP resolution, ACP `mcpServers` payload construction, and northbound execution truth

The canonical proof gate is:

```bash
./scripts/test-gate.sh proposal-048
```

This is a backend and northbound API contract. It does not define screen-level UI behavior.

## Design Principles

1. Persist truth at the owner boundary where it is produced.
2. Expose persisted truth through GraphQL and MCP readers without reconstructing it from logs.
3. Keep transport-only secrets and executable MCP registry payloads out of operator-facing reads.
4. Use focused proposal gates as regression contracts for cross-crate control-plane behavior.

## Failed-Stage Evidence

Failed-stage evidence is stage-attempt truth. It is not run export-pack truth and it is not sign-off evidence.

The owner chain is:

```text
stage failure settlement
  -> recovery snapshot producer
  -> failed-stage evidence builder
  -> stage_executions canonical JSON fields
  -> report artifact with report_kind = failed_stage_evidence
  -> reports.get / report://{run_id}
```

### Canonical stage fields

`stage_executions` owns the durable failure and recovery payloads:

- `validation_failure_json`
- `evidence_packet_json`
- `recovery_snapshot_json`

The failed-stage evidence packet may embed validation failure and recovery data for report convenience, but those embedded values do not replace the stage-owned fields.

### Recovery snapshot ownership

`engine/src/recovery.rs` owns deterministic next-action recovery snapshots for this Rust slice.

On failed stage settlement:

1. the orchestrator computes and persists `stage_executions.recovery_snapshot_json`
2. the failed-stage evidence builder reads that stage-owned recovery snapshot
3. the evidence packet mirrors the recovery snapshot into the report payload
4. if recovery cannot be computed, the producer writes a typed unavailable snapshot rather than leaving newly failed P048-era stages silent
### Evidence packet content

The V1 packet records:

- stage execution identity
- stage id, label, attempt number, and timing
- failed agent identity when available
- failure summary and failure class
- supervision / transport / output outcome fields when available
- raw output, receipt, and transcript presence
- typed validation failure payload when present
- output envelopes
- stage-owned recovery snapshot

Some fields remain nullable when Rust has no durable producer for them yet, including agent display title and some transport classification details.

#### Minimal Discovery Readback Path

Production-exposed Phase 1 implementations must provide a stable readback route for support and operator diagnosis. The minimal readback owner is the control-plane run evidence path:
- `settle_agent_outputs_from_discovery_decisions` writes the discovery projection.
- The existing failed-stage/run-detail evidence payload reads this projection for `FailedStageEvidencePanel` and run report diagnostics.

This path ensures that even before full Phase 2/3 UI enrichment, operators can diagnose discovery decisions and artifact settlement.

### Artifact lane

Failed-stage evidence uses the normal artifact/report lane.

Canonical artifact properties:

- `report_kind = "failed_stage_evidence"`
- JSON format
- stage-execution-derived path, conventionally under `failure-evidence/{stage_execution_id}/failed-stage-evidence.json`

Readers:

- `reports.get`
- `report://{run_id}`

No second report namespace is introduced.

## Delivery Preflight

Delivery preflight validates repo-backed delivery configuration during `StartRun`, before a run is created.

It is not release readiness, workflow validation, or post-approval artifact validation.

### Inputs

The preflight runs only when `delivery_configuration_json` is supplied.

The delivery configuration is expected to identify:

- repository identifier
- repository root
- base branch
- worktree base path
- target branch / release target data where applicable

### Checks

The implemented checks cover:

- repo root exists
- repo root is a git repository
- base branch exists
- worktree base is writable
- release target identifier is non-empty when required
- repo identifier is non-empty

Each check records:

- `id`
- `label`
- `passed`
- `detail`

### Blocking behavior

If preflight fails:

- `StartRun` returns a typed blocked-start result
- no `Run` row is created
- GraphQL and MCP return delivery-preflight details through domain payloads, not generic error strings

If preflight passes:

- the run is created
- the run stores `delivery_preflight_json`
- the persisted run-owned preflight payload is readable northbound

### MCP command contract

`runs.start` returns a result union with two domain outcomes:

- started run payload
- blocked delivery-preflight payload

Blocked delivery preflight is not transported through `errors[].extensions`.

Run reads expose the persisted `delivery_preflight_json` for successfully created runs.

### MCP contract

`runs.start` returns the same typed blocked-start preflight payload when run creation is rejected.

Successful run reads expose persisted delivery-preflight truth through:

- `runs.get`
- `run://{run_id}`

Blocked starts have no run resource because no run exists.

## MCP Resolution

MCP intent is resolved at executor time from the compiled agent binding and the machine-local MCP registry.

The owner chain is:

```text
AgentEntry.backend_profile
  -> backend_profile.mcp
  -> ResolvedAgent.backend_profile_id
  -> ResolvedAgent.requested_mcp_server_ids
  -> engine MCP resolver
  -> acp::ExecutionRequest.mcp_servers
  -> ACP session/new mcpServers
  -> AgentExecution MCP provenance fields
  -> GraphQL / MCP report readers
```

`required_tools` is not MCP authority.

### Registry sources

Executable server definitions come from the machine-local MCP registry:

- override: `CHAINWORKS_CODEX_CONFIG_PATH`
- canonical path: `~/.config/mcp/config.yaml`
- legacy fallback when canonical config is absent: `~/.config/goose/config.yaml`

The registry is read during execution so operator edits can take effect without daemon restart.

### Resolution outputs

The resolver records:

- backend profile id
- requested extension ids
- predicted effective extension ids
- predicted runtime ids
- denied extension ids
- blocking issues
- resolved executable payloads for ACP when resolution succeeds

Missing, disabled, unsupported, or malformed registry entries fail closed before ACP startup.

### ACP payload

Resolved MCP servers are carried internally as executable ACP payloads:

```rust
pub struct AcpMcpServerPayload {
    pub id: String,             // runtime id
    pub extension_id: String,   // provenance id
    pub transport: ResolvedMcpServerTransport,
}
```

Transport variants:

- stdio: command, args, env
- platform: provider

The ACP `session/new` payload uses `mcpServers` built from the same production helper tested by `mcp_servers_session_new_serialization_tests`.

Runtime id is the ACP server key. Extension id is preserved as provenance.

### Secret boundary

Registry command, args, and env are internal runtime data.

They may be present in `ExecutionRequest` and the ACP transport payload, but they are not exposed in GraphQL, MCP reports, or MCP resources.

### Actual MCP truth

When ACP startup succeeds, actual truth is observed from the transport response when the provider supplies accepted MCP server data.

When the provider does not return accepted MCP server data, the transport records an explicit fallback observation rather than silently treating predicted truth as observed truth.

When resolution blocks before `session/new`, the executor persists explicit empty actual truth:

- `actual_mcp_extensions_json = []`
- `actual_mcp_runtime_ids_json = []`
- `actual_mcp_observation_json.source = "mcp_resolution_blocked_before_session_new"`
- trust metadata records that no ACP session was attempted

## AgentExecution Persistence

Execution-level MCP truth is stored on `AgentExecution`:

- `backend_profile_id`
- `requested_mcp_extensions_json`
- `predicted_mcp_extensions_json`
- `predicted_mcp_runtime_ids_json`
- `actual_mcp_extensions_json`
- `actual_mcp_runtime_ids_json`
- `denied_mcp_extensions_json`
- `mcp_blocking_issues_json`
- `actual_mcp_observation_json`
- `mcp_session_startup_latency_ms`

Readers must use these durable fields. They must not reconstruct MCP truth from logs or from the current registry.

## Northbound Read Contract

### GraphQL

GraphQL exposes P048 truth through:

- run reads for persisted `delivery_preflight_json`
- `GqlStageExecution.executions` for execution-owned MCP truth
- `GqlAgentExecution` fields for requested, predicted, actual, denied, blocking, observation, and latency
- artifact reads for typed validation failure and failed-stage evidence artifacts

Stage summaries remain summary-oriented. Full execution MCP payloads belong to execution rows.

### MCP tools and resources

MCP exposes P048 truth through:

- `runs.start` for typed delivery-preflight blocking
- `runs.get` for persisted run-owned delivery-preflight truth
- `run://{run_id}` for the same run-owned preflight truth
- `reports.get` for failed-stage evidence and execution MCP truth
- `report://{run_id}` for the same report/resource truth

`reports.get` and `report://{run_id}` must stay in parity for execution-level MCP truth.

## Legacy Rows

Pre-migration rows may lack P048 fields.

Reader behavior:

- absence is exposed as absence
- old rows are not backfilled opportunistically by readers
- readers do not synthesize delivery-preflight, failed-stage evidence, recovery, or MCP truth from mutable files or logs

## Implementation Map

Primary Rust owners:

- `control-plane/crates/engine/src/evidence.rs`
- `control-plane/crates/engine/src/recovery.rs`
- `control-plane/crates/engine/src/preflight.rs`
- `control-plane/crates/engine/src/mcp.rs`
- `control-plane/crates/engine/src/executor.rs`
- `control-plane/crates/engine/src/command_handler.rs`
- `control-plane/crates/acp/src/transport.rs`
- `control-plane/crates/domain/src/run.rs`
- `control-plane/crates/domain/src/stage.rs`
- `control-plane/crates/domain/src/agent.rs`
- `control-plane/crates/db/src/repos/runs.rs`
- `control-plane/crates/db/src/repos/stages.rs`
- `control-plane/crates/db/src/repos/agent_executions.rs`
- `control-plane/crates/graphql-server/src/schema.rs`
- `control-plane/crates/graphql-server/src/types/run.rs`
- `control-plane/crates/graphql-server/src/types/stage.rs`
- `control-plane/crates/mcp-server/src/tools/runs.rs`
- `control-plane/crates/mcp-server/src/tools/reports.rs`
- `control-plane/crates/mcp-server/src/server.rs`

## Verification

Canonical focused gate:

```bash
./scripts/test-gate.sh proposal-048
```

The gate covers:

- DB/domain round trip for P048 fields
- delivery-preflight passing and blocked-start behavior
- GraphQL delivery-preflight readback
- MCP `runs.get` and `run://{run_id}` delivery-preflight readback
- ACP `session/new.mcpServers` serialization
- engine fail-closed MCP resolution persistence
- failed-stage evidence packet shape
- failed-stage evidence `reports.get` and `report://{run_id}` readback
- typed GraphQL blocked-preflight payload
- GraphQL stage `executions` MCP parity
- MCP reports/resources execution-level MCP truth

The gate is local Rust control-plane verification. It does not require a remote UI host.
