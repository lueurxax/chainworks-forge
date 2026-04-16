# Proposal 048: Failed-Stage Evidence, Delivery Preflight, and MCP Resolution

| Field | Value |
|---|---|
| Date | 2026-04-15 |
| Status | Draft |
| Author | Claude |
| Depends on | [044-post-approval-task-execution-and-release-gate-completion.md](../reference/044-post-approval-task-execution-and-release-gate-completion.md), [../reference/045-deterministic-release-operations.md](../reference/045-deterministic-release-operations.md), [../reference/structured-output-envelope-and-contract-validation.md](../reference/structured-output-envelope-and-contract-validation.md), [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md) |
| Scope | (A) add stage-owned failed-stage evidence packets, including stage-owned `recovery_snapshot`, without creating a second export truth lane; (B) add run-creation-time delivery preflight persistence and blocking semantics; (C) add execution-time MCP resolution and northbound exposure from the canonical Rust owner chain `backend_profile.mcp -> ResolvedAgent -> AgentExecution` |
| Goal | The Rust control plane persists failed-stage evidence, stage-owned recovery truth, and delivery-preflight truth at the same ownership boundaries the current product expects, and it resolves MCP from the same canonical agent binding that the workflow compiler already emits. |

---

## 1. Context and Motivation

### 1a. Current Rust baseline

This proposal is no longer designing against an empty substrate.

The current Rust control plane already has the implemented structured-output and validation-failure slice:

- output contracts compile through `workflow/src/compiler.rs` and `workflow/src/plan.rs`
- validation failures persist durably through `validation_failure_records`
- GraphQL artifact reads already expose typed `validationFailureRecord`
- MCP `reports.get` and `report://{run_id}` already decode and return the same typed validation-failure payload

That means P048 is a **delta proposal**, not a greenfield design. It must layer on top of the existing durable artifact, report, and validation-failure paths instead of describing them as absent or re-proposing them under a new owner chain.

What is still missing at `HEAD` is narrower:

1. stage-owned failed-stage evidence packets that summarize failure/recovery context beyond the validation-failure artifact alone, including the already-promoted stage-owned `recovery_snapshot`
2. persisted delivery-preflight results at run creation time
3. execution-level MCP requested/predicted/actual/denied truth and explicit northbound readers for that truth

### 1b. Failed-stage evidence is not export-pack truth

The stable product separates three evidence lanes:

| Owner | Scope | Trigger | Persistence | Consumers |
|---|---|---|---|---|
| `FailedStageEvidenceBuilder` | Stage-attempt failure truth | Immediately on failed settlement | Stage-owned packet + report lane | Recovery, reports, operator diagnostics |
| `EvidencePackBuilder` | Run export pack | After run completion | Export directory | Human review, export workflows |
| `SignOffEvidencePackBuilder` | Cohort-level evaluation | Sign-off time | Checksummed packet | Benchmark audit trail |

P048 ports only the first lane.

It must not:

- invent a second canonical report namespace
- treat export-pack filenames as storage truth
- or duplicate the existing `report_kind` artifact lane

Instead, failed-stage evidence becomes:

1. durable stage-owned JSON on `stage_executions`
2. a normal report artifact with `report_kind = "failed_stage_evidence"`
3. readable through the existing northbound report surfaces

### 1c. Delivery preflight is run-creation validation, not release readiness

`DeliveryPreflightService` validates mutable delivery configuration before a run is allowed to start.

It is not:

- the broad `PreflightService` workflow validator
- a release-time readiness gate
- or a substitute for `run_after_approval` artifact requirements from [044-post-approval-task-execution-and-release-gate-completion.md](../reference/044-post-approval-task-execution-and-release-gate-completion.md)

P048 therefore adds one frozen run-owned result:

- `run.delivery_preflight_json`

and one blocking behavior:

- failed delivery preflight prevents `StartRun` from creating or starting the run

### 1d. Canonical MCP owner contract

The canonical Rust owner chain for MCP intent is:

```text
AgentEntry.backend_profile
  -> backend_profile.mcp
  -> workflow::compiler::ResolvedAgent.backend_profile_id
  -> workflow::compiler::ResolvedAgent.requested_mcp_server_ids
  -> executor-side MCP resolver
  -> AgentExecution MCP provenance fields
  -> northbound report / GraphQL readers
```

This is already reflected in the current Rust compiler:

- `workflow/src/compiler.rs` reads `profile.mcp`
- `workflow/src/plan.rs` persists `ResolvedAgent.backend_profile_id`
- `workflow/src/plan.rs` persists `ResolvedAgent.requested_mcp_server_ids`

`required_tools` is not MCP authority.
Older `mcp_profile` wording is retired for the Rust control-plane slice and must not be reused by this proposal.

### 1e. Current northbound baseline

The operator/read path at `HEAD` is uneven and must be named precisely.

Current GraphQL:

- `QueryRoot.run` / `GqlRun` expose run metadata and projection counts
- `QueryRoot.stages` / `GqlStageExecution` expose stage summary plus `has_validation_failure`
- `QueryRoot.artifacts` / `GqlArtifact.validationFailureRecord` expose the decoded typed validation-failure payload

Current MCP:

- `reports.get` returns report artifacts and injects typed `validation_failure_record`
- `report://{run_id}` returns the same typed artifact payloads in the run report resource

What does **not** exist yet as an explicit northbound contract:

- execution-level MCP requested/predicted/actual/denied truth
- run-level delivery-preflight result exposure
- explicit placement rules for which new fields belong in GraphQL, which belong in report resources/tools, and which remain persistence-only

P048 must close those gaps explicitly.

---

## 2. Design

### 2a. Failed-stage evidence packet

```rust
// engine/src/evidence.rs

pub struct FailedStageEvidencePacket {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub stage_execution_id: String,
    pub stage_id: String,
    pub stage_label: String,
    pub stage_attempt_number: i64,
    pub failed_agent_id: Option<String>,
    pub failed_agent_title: Option<String>,  // nullable in V1 until Rust has a durable execution-time owner
    pub failure_summary: String,
    pub failure_class: String,
    pub supervision_classification: Option<String>,
    pub canonical_outcome: Option<String>,
    pub transport_error_kind: Option<String>,
    pub output_presence: Option<String>,
    pub raw_outputs_exist: bool,
    pub receipt_exists: bool,
    pub transcript_exists: bool,
    pub validation_failure: Option<serde_json::Value>,
    pub output_envelopes: Vec<serde_json::Value>,
    pub timing: StageTiming,
    pub recovery_snapshot: Option<serde_json::Value>,
}

pub struct StageTiming {
    pub stage_started_at: DateTime<Utc>,
    pub stage_completed_at: Option<DateTime<Utc>>,
    pub agent_started_at: Option<DateTime<Utc>>,
    pub agent_completed_at: Option<DateTime<Utc>>,
    pub agent_duration_seconds: Option<f64>,
}
```

Required V1 fields:

- stage identity and timing
- failure summary and classification
- raw/receipt/transcript existence
- typed `validation_failure`
- typed `output_envelopes`
- stage-owned `recovery_snapshot`, copied from the same canonical stage record / recovery owner chain used by the current product

Nullable/deferred V1 fields until explicit Rust owners exist:

- `failed_agent_title`
- `canonical_outcome`
- `transport_error_kind`
- `output_presence`

Build time:

- immediately when a stage attempt settles `Failed`
- `recovery_snapshot` is loaded from the canonical stage-owned recovery field first; the failed-stage packet mirrors that truth and does not become a second recovery authority

Recovery snapshot producer contract:

- `engine/src/recovery.rs` owns the deterministic next-action recovery snapshot for this Rust slice.
- On failed stage settlement, `engine/src/orchestrator.rs` calls the recovery snapshot producer before it calls the failed-stage evidence builder.
- The producer computes the snapshot from persisted run, stage, agent-execution, validation-failure, artifact, and retry/recovery state that already exists at failure-settlement time.
- The producer persists that payload to `stage_executions.recovery_snapshot_json`.
- `engine/src/evidence.rs` may read and embed `recovery_snapshot_json`, but it must not synthesize recovery truth when the stage-owned field is absent.
- If the producer cannot compute a snapshot, it persists a typed snapshot with `status = "unavailable"` and a bounded `reason`; it does not leave the field silently null for newly failed P048-era stages.

Stage-owned ownership for this slice stays aligned with the current execution-truth baseline:

1. `stage_executions.validation_failure_json` is the canonical stage-owned copy of the typed `ValidationFailureRecord`
2. `stage_executions.evidence_packet_json` is the stage-owned failed-stage evidence packet
3. `stage_executions.recovery_snapshot_json` is the stage-owned next-action snapshot

The failed-stage evidence packet may embed `validation_failure` and `recovery_snapshot` for report convenience, but those embedded copies do not replace the canonical stage-owned fields.

Persistence:

1. `stage_executions.validation_failure_json`
2. `stage_executions.evidence_packet_json`
3. `stage_executions.recovery_snapshot_json`
4. a normal artifact with `report_kind = "failed_stage_evidence"`

Canonical artifact path:

```text
{artifact_root}/failure-evidence/{stage_execution_id}/failed-stage-evidence.json
```

Friendly names such as `evidence-{stage_id}-attempt{n}.json` remain export-pack aliases only.

### 2b. Delivery preflight

```rust
// engine/src/preflight.rs

pub struct DeliveryPreflightResult {
    pub checks: Vec<PreflightCheck>,
    pub passed: bool,
    pub timestamp: DateTime<Utc>,
}

pub struct PreflightCheck {
    pub id: String,
    pub label: String,
    pub passed: bool,
    pub detail: Option<String>,
}
```

```rust
// engine/src/command_handler.rs

pub struct StartRunBlockedByDeliveryPreflight {
    pub delivery_preflight: DeliveryPreflightResult,
}

pub enum CommandResult {
    RunStarted { run_id: RunId },
    StartRunBlockedByDeliveryPreflight(StartRunBlockedByDeliveryPreflight),
    // ...
}
```

Validation checks:

1. repo root exists
2. repo root is a git repository
3. base branch exists
4. worktree base is writable
5. release target identifier is non-empty
6. repo identifier is non-empty

Call site:

- `engine/src/command_handler.rs` during `StartRun`
- only when `delivery_configuration_json` is present

Blocking semantics:

- failed preflight aborts `StartRun` before a run is created
- the blocked-start transport contract is `CommandResult::StartRunBlockedByDeliveryPreflight`
- `StartRunBlockedByDeliveryPreflight.delivery_preflight` carries the full typed `DeliveryPreflightResult`, including all failing and passing checks
- passing preflight is persisted on the run as `delivery_preflight_json`

Northbound transport contract for blocked starts:

- GraphQL `startRun` does not use `errors[].extensions` for this domain outcome
- GraphQL `startRun` returns an explicit result union:

```rust
pub union StartRunResult = StartRunSuccess | StartRunBlockedByDeliveryPreflightPayload;

pub struct StartRunSuccess {
    pub run: GqlRun,
}

pub struct StartRunBlockedByDeliveryPreflightPayload {
    pub delivery_preflight: GqlDeliveryPreflight,
}
```

- `StartRunBlockedByDeliveryPreflightPayload.delivery_preflight` is derived directly from `StartRunBlockedByDeliveryPreflight`
- MCP `runs.start` returns the same typed `delivery_preflight` payload instead of a generic string-only failure
- no run resource is created on blocked preflight, so `runs.get` / `run://{run_id}` do not participate in blocked-start truth

Northbound read contract for persisted run truth:

- when a run is created, `delivery_preflight_json` is the canonical persisted run-owned payload
- that persisted payload must round-trip through GraphQL run reads, MCP `runs.get`, and the canonical MCP run resource `run://{run_id}`
- blocked-start transport truth and persisted run truth are complementary surfaces and must not be collapsed into one ambiguous error string

### 2c. MCP resolution is a delta on top of the current compiler

The current compiler already persists:

```rust
pub struct ResolvedAgent {
    pub backend_profile_id: Option<String>,
    pub requested_mcp_server_ids: Vec<String>,
    // ...
}
```

P048 therefore does not need to invent a new request-intent owner.
Its delta is:

1. resolve executable MCP definitions at executor time
2. persist requested/predicted/actual/denied/blocking truth on `AgentExecution`
3. expose that truth northbound through explicit reader surfaces

Proposed resolver contract:

```rust
pub struct McpRuntimeBinding {
    pub runtime_id: Option<String>,
    pub provider: Option<String>,
}

pub struct McpResolutionReport {
    pub profile_id: String,
    pub requested_extensions: Vec<String>,
    pub predicted_effective_extensions: Vec<String>,
    pub predicted_effective_runtime_ids: Vec<String>,
    pub denied_extensions: Vec<String>,
    pub warnings: Vec<String>,
    pub blocking_issues: Vec<String>,
}

pub struct McpActualReport {
    pub actual_extensions: Vec<String>,
    pub actual_runtime_ids: Vec<String>,
    pub denied_extensions: Vec<String>,
    pub blocking_issues: Vec<String>,
    pub startup_latency_ms: Option<i64>,
}
```

Machine-local executable registry source:

- canonical path: `~/.config/mcp/config.yaml`
- explicit override: `CHAINWORKS_CODEX_CONFIG_PATH`
- one-time legacy migration source when canonical file is absent: `~/.config/goose/config.yaml`

Resolution rules:

1. requested intent always comes from `ResolvedAgent.requested_mcp_server_ids`
2. backend profile identity always comes from `ResolvedAgent.backend_profile_id`
3. executable server definitions come from the machine-local MCP registry, not from catalog YAML
4. runtime/provider binding determines `stdio` versus `platform` filtering
5. missing or disabled entries are persisted into `denied_extensions` / `blocking_issues` and fail closed before ACP session startup

Executable ACP payload contract:

```rust
pub struct ResolvedMcpServer {
    pub extension_id: String,
    pub runtime_id: String,
    pub transport: ResolvedMcpServerTransport,
}

pub enum ResolvedMcpServerTransport {
    Stdio {
        command: String,
        args: Vec<String>,
        env: BTreeMap<String, String>,
    },
    Platform {
        provider: String,
    },
}

pub struct AcpMcpServerPayload {
    pub id: String,            // runtime_id; stable key in ACP session/new
    pub extension_id: String,  // provenance only; not operator-facing by default
    pub transport: ResolvedMcpServerTransport,
}
```

Internal handoff rules:

- `engine/src/mcp.rs` resolves requested extension IDs into `ResolvedMcpServer` values.
- `acp::ExecutionRequest` carries `mcp_servers: Vec<AcpMcpServerPayload>` plus the persisted resolver report.
- `acp/src/transport.rs` serializes those payloads into the ACP `session/new` `mcpServers` array before `session/prompt`.
- The `mcpServers[].id` key is the runtime ID because the executable registry owns runtime identity and de-duplication.
- The extension ID is preserved inside the internal payload and persisted provenance, but operator-facing readers expose extension/runtime IDs and blocking issues only.
- Registry command/args/env are carried only in `ExecutionRequest` and the ACP transport payload; they are not exposed by GraphQL, MCP reports, or resource reads.
- Missing, disabled, unsupported, or malformed registry entries produce `blocking_issues` and prevent ACP session startup; no partial `mcpServers` payload is sent for a blocked execution.

Persisted execution truth on `AgentExecution`:

- `requested_mcp_extensions_json`
- `predicted_mcp_extensions_json`
- `predicted_mcp_runtime_ids_json`
- `actual_mcp_extensions_json`
- `actual_mcp_runtime_ids_json`
- `denied_mcp_extensions_json`
- `mcp_blocking_issues_json`
- `mcp_session_startup_latency_ms`

### 2d. Northbound placement contract

This proposal explicitly binds each new field family to a reader owner.

| Surface | Owner | Fields exposed |
|---|---|---|
| GraphQL `startRun` mutation | `graphql-server/src/schema.rs` | explicit `StartRunResult` union: `StartRunSuccess { run: GqlRun! }` or `StartRunBlockedByDeliveryPreflightPayload { deliveryPreflight: GqlDeliveryPreflight! }`; this domain outcome does not ride `errors[].extensions` |
| GraphQL run read | `graphql-server/src/types/run.rs` | `delivery_preflight_json` or a typed `deliveryPreflight` field derived from the persisted run-owned payload |
| MCP `runs.start` | `mcp-server/src/tools/runs.rs` | same typed blocked-start `delivery_preflight` payload as GraphQL when start is rejected before run creation |
| MCP `runs.get` | `mcp-server/src/tools/runs.rs` | persisted run-owned `delivery_preflight_json` or typed `delivery_preflight` field on successful created runs |
| MCP `run://{run_id}` | `mcp-server/src/server.rs` | same persisted run-owned delivery-preflight payload as `runs.get` |
| GraphQL stage summary | `graphql-server/src/types/stage.rs` | keeps summary-only fields such as `has_validation_failure`; does not become a dumping ground for full execution payloads |
| GraphQL stage-to-execution relation | `graphql-server/src/types/stage.rs` | add `executions: [GqlAgentExecution!]!` on `GqlStageExecution`, sourced from persisted `AgentExecution` rows for that stage execution |
| GraphQL execution read | `graphql-server/src/types/agent_execution.rs` plus `graphql-server/src/schema.rs` | `backend_profile_id`, requested/predicted/actual/denied MCP truth, runtime IDs, startup latency, persisted `mcp_blocking_issues_json` through `GqlAgentExecution` |
| GraphQL artifact read | `graphql-server/src/types/artifact.rs` | decoded `validationFailureRecord` and failed-stage-evidence/report artifacts |
| MCP `report://{run_id}` | `mcp-server/src/server.rs` | run summary, existing artifact payloads, and execution-level MCP truth array including blocking issues |
| MCP `reports.get` | `mcp-server/src/tools/reports.rs` | same execution-level MCP truth, including blocking issues, and same typed failure/report artifacts as `report://{run_id}` |
| Internal persistence only | DB rows / repo layer | raw executable registry definitions, transport command/args/env, and intermediate resolver internals that are not operator-facing |

Required northbound invariant:

- blocked delivery-preflight failures use one explicit transport contract across GraphQL `startRun` and MCP `runs.start`
- GraphQL blocked-start truth is carried by the `StartRunResult` union, not by transport-level GraphQL errors
- persisted `delivery_preflight_json` uses one explicit run-owned read contract across GraphQL run reads, MCP `runs.get`, and `run://{run_id}`
- GraphQL and MCP must both read the same durable `AgentExecution` MCP fields
- GraphQL execution-level MCP truth is reached through `GqlStageExecution.executions`, not through an implicit or conditional resolver path
- typed validation-failure payloads remain owned by the existing artifact/report lane
- failed-stage evidence rides the same artifact/report lane rather than a second bespoke reader

### 2e. MCP preflight ownership

P048 includes only execution-time MCP enforcement.

Included:

- executor-side resolver call
- fail-closed session startup when requested MCP cannot be realized
- persistence of denied/blocking MCP truth on `AgentExecution`

Deferred:

- broad `PreflightService`-style run-start MCP warnings
- a separate MCP readiness summary lane on `StartRun`

This keeps MCP authority at one boundary for V1: executor-time resolution against the current machine-local registry.

---

## 3. Files to Create/Modify

| File | Change |
|---|---|
| `engine/src/evidence.rs` | **NEW** failed-stage evidence packet builder and serializers |
| `engine/src/orchestrator.rs` | Compute/persist recovery snapshot, then build and persist failed-stage evidence on failed settlement |
| `domain/src/stage.rs` | Add `validation_failure_json: Option<String>`, `evidence_packet_json: Option<String>`, and `recovery_snapshot_json: Option<String>` as the canonical stage-owned failure/recovery fields |
| `db/src/repos/stages.rs` | Persist/read `validation_failure_json`, `evidence_packet_json`, and `recovery_snapshot_json` |
| `engine/src/recovery.rs` | Own the deterministic failed-stage next-action recovery snapshot producer for `stage_executions.recovery_snapshot_json` |
| `engine/src/preflight.rs` | **NEW** delivery-preflight validator |
| `engine/src/command_handler.rs` | Run delivery preflight during `StartRun`, return typed blocked-start preflight payloads, and block failed starts before run creation |
| `domain/src/run.rs` | Add `delivery_preflight_json: Option<String>` |
| `db/src/repos/runs.rs` | Persist/read `delivery_preflight_json` |
| `engine/src/mcp.rs` | **NEW** runtime-registry loader and MCP resolver |
| `domain/src/agent.rs` | Add MCP provenance fields, including `mcp_blocking_issues_json`, to `AgentExecution` |
| `db/src/repos/agent_executions.rs` | Persist/read MCP provenance JSON, blocking issues, and startup latency |
| `workflow/src/compiler.rs` | Reuse current `backend_profile.mcp` compilation path; add any missing runtime-binding fields only if required |
| `workflow/src/plan.rs` | Keep `backend_profile_id` and `requested_mcp_server_ids` canonical; add new runtime-binding fields only if needed |
| `engine/src/executor.rs` | Resolve MCP before transport startup, persist requested/predicted/denied/blocking truth before session start, and persist actual truth after session startup |
| `acp/src/lib.rs` | Carry `Vec<AcpMcpServerPayload>` and resolver metadata in `ExecutionRequest` |
| `acp/src/transport.rs` | Serialize `ExecutionRequest.mcp_servers` into ACP `session/new.mcpServers` |
| `graphql-server/src/types/run.rs` | Expose persisted run-owned delivery-preflight payload on run reads |
| `graphql-server/src/types/stage.rs` | Keep summary fields and add explicit `executions: [GqlAgentExecution!]!` relation on `GqlStageExecution` |
| `graphql-server/src/types/agent_execution.rs` | **NEW** typed execution-level MCP truth for northbound GraphQL |
| `graphql-server/src/schema.rs` | Add the `GqlStageExecution.executions` field to the schema, wire it unconditionally to the execution-owned resolver path, and change `startRun` to return the explicit `StartRunResult` union rather than `Result<GqlRun>` or `errors[].extensions` for blocked delivery preflight |
| `mcp-server/src/tools/runs.rs` | Return typed blocked-start delivery-preflight payloads from `runs.start` and expose persisted run-owned delivery-preflight payloads from `runs.get` |
| `mcp-server/src/server.rs` | Extend `run://{run_id}` with persisted run-owned delivery-preflight payloads and extend `report://{run_id}` with execution-level MCP truth while keeping the existing typed artifact lane |
| `mcp-server/src/tools/reports.rs` | Extend `reports.get` with the same execution-level MCP truth as `report://{run_id}` |
| `db/migrations/00x_evidence_preflight_and_mcp.sql` | Add `stage_executions.validation_failure_json`, `stage_executions.evidence_packet_json`, `stage_executions.recovery_snapshot_json`, `runs.delivery_preflight_json`, and agent-execution MCP provenance columns including `mcp_blocking_issues_json` using the next free migration ordinal at implementation time. On current `HEAD`, `008_session_runtime_usage.sql` and `009_owner_execution_lineage.sql` already exist, so the current concrete slot would be `010_*`. |
| `docs/reference/test-gates.md` | Add the repo-owned `proposal-048|p048` gate entry and its focused proof scope |
| `scripts/test-gate.sh` | Add the canonical `proposal-048|p048` gate command |

Migration rollout notes:

- historical `stage_executions.validation_failure_json`, `stage_executions.evidence_packet_json`, `stage_executions.recovery_snapshot_json`, `runs.delivery_preflight_json`, and `agent_executions` MCP provenance columns including `mcp_blocking_issues_json` read as `None` / empty for pre-migration rows
- readers do not synthesize missing historical truth out of band
- northbound surfaces expose absence explicitly rather than guessing defaults for older rows

---

## 4. Acceptance Criteria

### Failed-stage evidence

1. A failed stage attempt persists `stage_executions.validation_failure_json` as the canonical stage-owned copy of the typed `ValidationFailureRecord`.
2. A failed stage attempt persists `stage_executions.evidence_packet_json`.
3. The same failure persists as a normal artifact with `report_kind = "failed_stage_evidence"`.
4. The canonical artifact path is stage-execution-derived and collision-safe.
5. Export-pack friendly filenames do not become canonical storage truth.
6. `reports.get` and `report://{run_id}` expose failed-stage evidence through the existing report lane.
7. The failed-stage evidence packet carries the same stage-owned `recovery_snapshot` as the canonical stage record; the packet mirrors that truth and does not invent a second recovery owner.
8. `engine/src/recovery.rs` computes and persists `stage_executions.recovery_snapshot_json` before failed-stage evidence packet construction for newly failed P048-era stages.

### Delivery preflight

9. `StartRun` with `delivery_configuration_json` runs delivery preflight before the run is created or started.
10. Failed preflight blocks run creation/start and returns a typed blocked-start transport payload containing the full `DeliveryPreflightResult`.
11. GraphQL `startRun` and MCP `runs.start` expose the same blocked-start delivery-preflight truth rather than collapsing to unrelated generic errors.
12. GraphQL `startRun` uses an explicit result union/payload contract for blocked preflight and does not route this domain outcome through `errors[].extensions`.
13. Passing preflight persists `delivery_preflight_json` on the run.
14. GraphQL run reads, MCP `runs.get`, and `run://{run_id}` expose the same persisted run-owned delivery-preflight payload on successful created runs.
15. No release-time readiness gate is introduced by this proposal.

### MCP ownership and resolution

16. Requested MCP intent is read only from `backend_profile.mcp` via the already-compiled `ResolvedAgent.requested_mcp_server_ids`.
17. `McpResolutionReport.profile_id` is read only from `ResolvedAgent.backend_profile_id`.
18. `required_tools` does not participate in MCP resolution.
19. Missing/disabled/unsupported requested MCP entries fail closed before ACP session startup and persist denied extensions plus `mcp_blocking_issues_json`.
20. Requested, predicted, actual, denied, and blocking MCP truth persists on `AgentExecution`.
21. Registry reads happen at executor time so operator edits are visible without daemon restart.
22. `ExecutionRequest.mcp_servers` carries executable ACP payloads keyed by runtime ID, while raw registry command/args/env remain internal to the engine/ACP transport boundary.

### Northbound readers

23. GraphQL run reads expose delivery-preflight truth from the persisted run-owned payload.
24. GraphQL stage reads remain summary-oriented and add an explicit `executions: [GqlAgentExecution!]!` relation for execution-owned MCP truth.
25. `GqlAgentExecution` exposes execution-level MCP truth, including blocking issues, from persisted `AgentExecution` rows; GraphQL does not rely on a conditional or implied resolver path.
26. GraphQL artifact reads continue to expose the typed `validationFailureRecord`.
27. `runs.get` and `run://{run_id}` expose the same persisted run-owned delivery-preflight truth as GraphQL run reads.
28. `reports.get` and `report://{run_id}` expose the same execution-level MCP truth sourced from persisted `AgentExecution` rows.
29. Raw registry command/args/env details remain internal persistence/runtime data and are not promoted into operator-facing reads.

---

## 5. Test Gate

Repo-owned proof lane changes required by this proposal:

- `docs/reference/test-gates.md` gains a `proposal-048|p048` entry
- `scripts/test-gate.sh` gains the matching `proposal-048|p048` command
- this gate is the canonical proof path for the P048 control-plane slice; later audits should not treat a generic workspace run as the only proof contract

Focused proof scope for `proposal-048|p048`:

1. delivery-preflight blocked-start contract plus successful `delivery_preflight_json` persistence
2. GraphQL / `runs.get` / `run://{run_id}` parity for persisted run-owned delivery-preflight truth
3. failed-stage evidence persistence on `stage_executions` plus `reports.get` / `report://{run_id}` readback
4. ACP `mcpServers` realization plus fail-closed denied/blocking MCP truth persisted on `AgentExecution`
5. GraphQL stage `executions` parity for execution-level MCP truth, not only MCP report-side truth

Canonical wrapper:

```bash
./scripts/test-gate.sh proposal-048
```

Script entry:

```bash
proposal-048|p048)
  log "Proposal 048 control-plane gate: evidence + preflight + MCP"
  (
    cd "$ROOT_DIR/control-plane"
    cargo test -p engine delivery_preflight_run_persistence_tests -- --nocapture &&
    cargo test -p engine delivery_preflight_blocked_start_tests -- --nocapture &&
    cargo test -p graphql-server delivery_preflight_graphql_contract_tests -- --nocapture &&
    cargo test -p graphql-server delivery_preflight_run_readback_contract_tests -- --nocapture &&
    cargo test -p mcp-server runs_delivery_preflight_contract_tests -- --nocapture &&
    cargo test -p mcp-server run_resource_delivery_preflight_contract_tests -- --nocapture &&
    cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture &&
    cargo test -p mcp-server reports_failed_stage_evidence_contract_tests -- --nocapture &&
    cargo test -p engine mcp_resolution_persistence_tests -- --nocapture &&
    cargo test -p graphql-server execution_mcp_truth_contract_tests -- --nocapture &&
    cargo test -p mcp-server reports_mcp_resolution_truth_tests -- --nocapture
  )
  log "Proposal 048 control-plane gate passed"
  ;;
```

---

## 6. Out of Scope

- run-export evidence pack design
- cohort/sign-off evidence pack design
- broad workflow `PreflightService`
- start-time MCP warning UX beyond executor fail-closed behavior
- redesign of the machine-local MCP registry format or ownership
