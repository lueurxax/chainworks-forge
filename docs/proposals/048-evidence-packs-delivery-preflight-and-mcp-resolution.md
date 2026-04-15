# Proposal 048: Failed-Stage Evidence, Delivery Preflight, and MCP Resolution

| Field | Value |
|---|---|
| Date | 2026-04-14 |
| Status | Draft |
| Author | Claude |
| Depends on | [044-post-approval-task-execution-and-release-gate-completion.md](044-post-approval-task-execution-and-release-gate-completion.md), [../reference/045-deterministic-release-operations.md](../reference/045-deterministic-release-operations.md), [046-structured-output-envelope-and-contract-validation.md](046-structured-output-envelope-and-contract-validation.md) — P048's `FailedStageEvidencePacket` references `ValidationFailureRecord` and `StructuredOutputEnvelope` types defined and persisted by P046. Without P046, fields `validation_failure` and `output_envelopes` have no source. |
| Scope | (A) Port `FailedStageEvidenceBuilder` to the Rust daemon as stage-owned failure evidence (not a second export truth lane). (B) Port `DeliveryPreflightService` as a run-creation-time frozen config validator (not a release-time readiness gate). (C) Port MCP resolution from `backend_profile.mcp` through `MCPPolicyResolver` semantics to ACP `mcpServers` (not from `required_tools`). |
| Goal | Failed stages produce durable evidence packets for recovery/report readers; delivery config is validated before freezing at run start; agents receive their declared MCP servers from the stable `backend_profile.mcp` owner chain. |

---

## 1. Context and Motivation

### 1a. Evidence — Three Distinct Owners

The stable Swift codebase separates evidence into three distinct owners:

| Owner | Scope | Trigger | Persistence | Consumers |
|---|---|---|---|---|
| **FailedStageEvidenceBuilder** | Stage-level failure truth | Immediately on stage failure | `stageExecution.evidencePacketJSON` (on model) | Recovery UI, report readers, exports |
| **EvidencePackBuilder** | Run-level export pack | After run completion | Disk directory: `evidence-pack-{runID}/` | Human review, CI/CD export |
| **SignOffEvidencePackBuilder** | Cohort-level evaluation | At benchmark sign-off | Checksummed JSON packet | Evaluation audit trail |

The Rust daemon has **none of these**. P048 ports only **FailedStageEvidenceBuilder** — the stage-owned failure/recovery truth that report readers and recovery surfaces depend on. The run-level export (EvidencePackBuilder) and cohort sign-off (SignOffEvidencePackBuilder) remain out of scope — they are shell-owned export paths, not daemon-internal truth.

The daemon already has a `reports.get` MCP tool that returns artifacts with `report_kind` set. P048 does **not** create a second report lane; it ensures failed-stage evidence is persisted as an artifact readable through that existing lane.

### 1b. Delivery Preflight — Run-Creation, Not Release-Time

Swift separates two validation moments:

| Service | When | What | Frozen? |
|---|---|---|---|
| **DeliveryPreflightService** | Run creation (before start) | Validates mutable `DeliveryConfiguration` draft: repo exists, git repo valid, base branch exists, worktree base writable, release target set | Result frozen as `run.deliveryPreflightJSON` |
| **PreflightService** | Pre-run (before start) | Validates full workflow: YAML, catalog, providers, MCP, skills, workspace | Ephemeral report, not persisted |

Release-time readiness is **not** a preflight concern — it's governed by explicit artifact/stage inputs in the workflow YAML (P044's `run_after_approval` tasks consume required outputs) plus deterministic services from P045.

P048 ports `DeliveryPreflightService` as a run-creation validator. It does **not** create a release-time readiness gate.

### 1c. MCP Resolution — `backend_profile.mcp`, Not `required_tools`

The stable MCP owner chain:

```
backend_profile.mcp: [String]       (YAML source of truth)
  → RunPlanCompiler                  (compiles into ResolvedAgent.requestedMCPServerIDs)
  → MCPPolicyResolver.resolve()     (queries runtime registry, produces requested/predicted/denied)
  → RuntimeSessionBridge             (materializes into [RuntimeMCPServerDefinition] for ACP)
  → transport session/new { mcpServers: [...] }
```

`required_tools` is a **different** subsystem — it declares agent tool-use capabilities for artifact shape validation, not MCP server requirements. Agents without `required_tools` can still legitimately require MCP via `backend_profile.mcp`.

The Rust daemon currently passes `"mcpServers": []` unconditionally. P048 ports the `backend_profile.mcp` → resolved MCP servers chain.

---

## 2. Design

### 2a. Failed-Stage Evidence (FailedStageEvidenceBuilder Port)

```rust
// engine/src/evidence.rs

pub struct FailedStageEvidencePacket {
    pub id: String,
    pub timestamp: DateTime<Utc>,
    pub stage_id: String,
    pub stage_label: String,
    pub stage_attempt_number: i64,
    pub failed_agent_id: Option<String>,
    pub failed_agent_title: Option<String>,
    pub failure_summary: String,
    pub failure_class: String,          // "output_contract_mismatch" | "agent_reported_failure" | "transport_failure" | "timeout" | "no_output_produced" | "empty_output"
    pub supervision_classification: Option<String>,  // "idle_hang_before_first_progress" | "idle_hang_after_progress" | "idle_hang_read_loop" | "idle_hang_after_first_edit" | "mutation_side_effect_missing"
    pub canonical_outcome: Option<String>,           // agent canonical outcome from executor (Rust-owned)
    pub transport_error_kind: Option<String>,        // transport-level error classification (Rust-owned)
    pub output_presence: Option<String>,             // "all_present" | "partial" | "none" (Rust-owned)
    pub raw_outputs_exist: bool,
    pub receipt_exists: bool,
    pub transcript_exists: bool,
    pub validation_failure: Option<serde_json::Value>,  // ValidationFailureRecord from P046
    pub output_envelopes: Vec<serde_json::Value>,       // StructuredOutputEnvelope records from P046
    pub timing: StageTiming,
    pub recovery_snapshot: Option<serde_json::Value>,   // RecoveryActionSnapshot (retry reason, recommended action)
}

pub struct StageTiming {
    pub stage_started_at: DateTime<Utc>,
    pub stage_completed_at: Option<DateTime<Utc>>,
    pub agent_started_at: Option<DateTime<Utc>>,
    pub agent_completed_at: Option<DateTime<Utc>>,
    pub agent_duration_seconds: Option<f64>,
}
```

**Rust payload scope decision (explicitly avoiding overclaim):**

`FailedStageEvidencePacket` is a **Rust V1 contract** in this proposal, not a blanket promise of immediate full stable parity.

Durable fields required for V1:
- `transcript_exists` from runtime artifact presence (`acp-stderr.log` path check).
- `raw_outputs_exist` from artifact persistence outcome.
- `receipt_exists` from persisted receipt artifact detection.
- `supervision_classification` if and only if the supervision lane is persisted on `domain::StageExecution`; if absent in this wave, persist `None`.
- `validation_failure` from `P046` `ValidationFailureRecord` ownership.
- `output_envelopes` from `P046` `StructuredOutputEnvelope` ownership.
- `failure_summary`, `failure_class`, `attempt_number`, `timestamps`, and attempt timing from stage/agent context.

Fields that are not yet durable in Rust owners and therefore must remain nullable until an explicit V2 slice:
- `canonical_outcome` (target owner: `domain::AgentExecution` outcome field)
- `transport_error_kind` (target owner: `domain::AgentExecution` transport diagnostics)
- `output_presence` (target owner: execution result + artifact presence policy)
- `recovery_snapshot` (target owner: `domain::StageExecution`)

If any of these owners are implemented, acceptance criteria and readers must be updated in the same change set. If they remain absent, `FailedStageEvidencePacket` readers MUST treat them as optional (`null`).

**When built:** Immediately when a stage settles as `Failed` in the orchestrator (line ~220 in the existing settlement path). Not deferred to run completion.

**Persistence:** Two locations (matching Swift):
1. Serialized JSON stored on the `stage_executions` row (new column `evidence_packet_json TEXT`)
2. Written to artifact canonical path `{artifact_root}/failure-evidence/evidence-{stage_id}-attempt{n}.json` with `report_kind = "failed_stage_evidence"` — readable via existing `reports.get` MCP tool

**DB migration addition:**
```sql
ALTER TABLE stage_executions ADD COLUMN evidence_packet_json TEXT;
```

### 2b. Delivery Preflight (DeliveryPreflightService Port)

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

pub async fn validate_delivery_config(
    config: &DeliveryConfiguration,
) -> DeliveryPreflightResult;
```

Stable payload remains `{ id, label, passed, detail }` so UI/evidence readers that expect `check.id` / `check.label` stay compatible.

**Validation checks (matching Swift `DeliveryPreflightService.validate`):**
1. Repository root exists on disk
2. Valid git repo (`.git` directory or `git rev-parse --git-dir`)
3. Base branch exists (`git rev-parse --verify refs/heads/{base_branch}`)
4. Worktree base path is writable (`std::fs::create_dir_all` succeeds)
5. Release target identifier is non-empty
6. Repo identifier is non-empty

**When called:** At run creation time, in `command_handler.rs` `StartRun`, **only** when `delivery_configuration_json` is provided.

**Blocking semantics (matching Swift `deliveryPreflightBlocksStart`):** If preflight `passed == false`, the run **does not start**. The command handler returns an error with the failing checks. This matches the stable repo-backed gate: the Swift UI blocks run creation when `deliveryPreflightBlocksStart` is true (line 2248 in view code). The Rust daemon enforces the same gate:

```rust
// command_handler.rs StartRun — after delivery preflight
if let Some(ref config_json) = c.delivery_configuration_json {
    let config: DeliveryConfiguration = serde_json::from_str(config_json)?;
    let preflight = validate_delivery_config(&config).await;
    if !preflight.passed {
        let failing = preflight.checks.iter()
            .filter(|c| !c.passed)
            .map(|c| c.id.clone())
            .collect::<Vec<_>>();
        return Err(anyhow!("Delivery preflight failed: {}", failing.join(", ")));
    }
    // Persist passing result for evidence/export readers
    run.delivery_preflight_json = Some(serde_json::to_string(&preflight)?);
}
```

**DB:** New column `delivery_preflight_json TEXT` on `runs` table. Only populated when preflight passes (failed preflight blocks run start entirely).

**Not called at release time.** Release readiness is the workflow's concern (P044 `run_after_approval` tasks check their inputs).

### 2c. MCP Resolution (`backend_profile.mcp` Owner Chain)

**Compilation (workflow compiler):**

Add to `ResolvedAgent`:
```rust
pub requested_mcp_server_ids: Vec<String>,
pub requested_mcp_runtime_id: Option<String>,
```

In `build_agent_lookup`, extract from backend profile:
```rust
let requested_mcp = profile.mcp.clone().unwrap_or_default();
// ... into AgentBinding
```

**Resolution (executor, before ACP session) — full requested/predicted/actual/denied truth:**

```rust
// engine/src/mcp.rs

/// Pre-session resolution report (matching Swift MCPPolicyResolutionReport).
/// Captures predicted truth before execution.
pub struct McpResolutionReport {
    pub profile_id: String,
    pub requested_extensions: Vec<String>,           // echo of agent.requested_mcp_server_ids
    pub predicted_effective_extensions: Vec<String>,  // what should be available (pre-session)
    pub predicted_effective_runtime_ids: Vec<String>, // mapped to runtime registry IDs
    pub denied_extensions: Vec<String>,               // blocked / missing / disabled
    pub warnings: Vec<String>,
    pub blocking_issues: Vec<String>,
}

/// Post-session actual truth. Captured after ACP session completes.
pub struct McpActualReport {
    pub actual_extensions: Vec<String>,   // what the ACP session actually settled on
    pub denied_extensions: Vec<String>,   // settled denied/unavailable set after runtime start
    pub startup_latency_ms: Option<i64>,
}

pub struct McpServerDef {
    pub name: String,
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
}

pub fn resolve_mcp_servers(
    requested_ids: &[String],
    runtime_id: Option<&str>,
    registry: &McpRuntimeRegistry,  // machine-local, NOT catalog
) -> (McpResolutionReport, Vec<McpServerDef>);
```

**Resolution logic (matching `MCPPolicyResolver.resolve` + `RuntimeSessionBridge.resolveACPMCPServers`):**

`backend_profile.mcp` provides the **requested intent** (server IDs). The **executable server definitions** (command, args, env) come from the **machine-local runtime extension registry** — not from catalog `runtime_profiles` or repo-local MCP config. This matches the Swift architecture:
- `MCPPolicyResolver` (line 254-306) queries `runtimeRegistry.configsByRuntimeID[serverID]` — a machine-local registry populated from the user's MCP server configuration
- `RuntimeSessionBridge.resolveACPMCPServers` (line 162-208) materializes `RuntimeMCPServerDefinition` from registry entries: `definition.cmd`, `definition.args`, `definition.envs`
- The registry is **not** the YAML catalog — it's a separate machine-local configuration surface

Resolver input must include runtime namespace/provider binding so that `server_type` filtering follows selected runtime rules (`platform` servers only for the codex runtime binding).

**For the Rust daemon V1**, the registry must preserve the current ACP registry contract instead of inventing a second machine-local authority. The machine-local executable registry is:

- canonical path: `~/.config/mcp/config.yaml`
- explicit override: `CHAINWORKS_CODEX_CONFIG_PATH`
- one-time legacy migration source when canonical file is absent: `~/.config/goose/config.yaml`

Registry loads are evaluated at execution boundaries, not only daemon startup.
- load during delivery preflight path, before session startup
- load again when creating the ACP session (`executor` boundary), then pass to `resolve_mcp_servers`
- this ensures runtime edits are visible without restart and mirrors the stable owner chain timing.

The daemon may wrap that registry in a Rust-native parsed type, but it must read the same canonical registry contract the Swift app already uses. The resolution function takes an explicit provider/runtime binding and machine-local registry snapshot:

```rust
pub fn load_mcp_runtime_registry() -> anyhow::Result<McpRuntimeRegistry>;

pub struct McpRuntimeBinding {
    pub runtime_id: Option<String>,
    pub provider: Option<String>,
}

pub fn resolve_mcp_servers(
    requested_ids: &[String],
    binding: &McpRuntimeBinding,
    registry: &McpRuntimeRegistry,  // machine-local, NOT catalog
) -> (McpResolutionReport, Vec<McpServerDef>);
```

1. For each `requested_id` in `agent.requested_mcp_server_ids`:
   - Look up in `registry.servers[requested_id]`
   - Resolve runtime binding:
     - if `binding.runtime_id` is set, treat registry entries under that runtime namespace as source of truth;
     - otherwise, default to provider-resolved namespace for the selected binding.
   - Check: server exists, is enabled, and type is `"stdio"` unless `binding.provider == Some("codex".to_string())` and the runtime policy allows `"platform"`.
   - If available → materialize `McpServerDef { name, command, args, env }`, add to `predicted_effective_extensions`
   - If missing or disabled → add to `denied_extensions` and `blocking_issues`
2. Return `(report, server_defs)` — report for persistence, defs for transport

**McpRuntimeRegistry** (new type):
```rust
pub struct McpRuntimeRegistry {
    pub servers: HashMap<String, McpServerConfig>,
}
pub struct McpServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub enabled: bool,
    pub server_type: String,  // "stdio" | "platform"
}
```

Loaded via `load_mcp_runtime_registry()` at preflight/executor boundaries from the existing ACP registry contract (`~/.config/mcp/config.yaml`, optional `CHAINWORKS_CODEX_CONFIG_PATH` override, one-time migration from `~/.config/goose/config.yaml`). The catalog's `runtime_profiles` section is **not** the source of executable server definitions.

**Persistence (matching Swift `AgentExecution` / report-comparison MCP fields):**

After resolution, persist on the agent execution record (or work_item payload):
- `requested_mcp_extensions_json` — serialized `requested_extensions`
- `predicted_mcp_extensions_json` — serialized `predicted_effective_extensions`
- `predicted_mcp_runtime_ids_json` — serialized `predicted_effective_runtime_ids`
- `denied_mcp_extensions_json` — serialized `denied_extensions`

After ACP session completes, update with actual:
- `actual_mcp_extensions_json` — serialized `actual_extensions`
- `actual_mcp_runtime_ids_json` — serialized settled runtime IDs when available
- `denied_mcp_extensions_json` — updated to settled denied set if runtime startup narrowed availability further
- `mcp_session_startup_latency_ms` — from transport metrics

Northbound/report path:
- per-agent execution truth persists all four layers: requested → predicted → actual → denied
- run report builders and comparison readers consume those persisted agent-execution layers instead of reconstructing MCP truth from raw `mcpServers`
- MCP `report://{run_id}` resources in `mcp-server/src/server.rs`, `reports.get` in `mcp-server/src/tools/reports.rs`, and GraphQL stage/execution readers in `graphql-server/src/types/run.rs` + `graphql-server/src/types/stage.rs` surface the same four-layer truth for operator inspection

This gives report/comparison readers the full chain: requested → predicted → actual → denied.

**Transport integration:**

In `acp/src/transport.rs`, replace `"mcpServers": []` with resolved server defs:

```rust
let mcp_servers: Vec<serde_json::Value> = req.mcp_servers.iter()
    .map(|s| serde_json::json!({
        "name": s.name,
        "command": s.command,
        "args": s.args,
        "env": s.env,
    }))
    .collect();
```

**ExecutionRequest extension:**
```rust
pub mcp_servers: Vec<McpServerDef>,
pub mcp_resolution_json: Option<String>,  // serialized McpResolutionReport for persistence
pub mcp_runtime_binding: Option<McpRuntimeBinding>, // resolver context used for runtime-typed filtering
```

This keeps the resolver context visible to transport-level diagnostics and prevents
runtime/type assumptions from being re-derived from `required_tools` or other non-runtime surfaces.

---

## 3. Files to Create/Modify

| File | Change |
|---|---|
| **Failed-stage evidence** | |
| `engine/src/evidence.rs` | **NEW** — `FailedStageEvidencePacket`, `build_evidence_packet()` |
| `engine/src/orchestrator.rs` | Build and persist evidence packet on stage failure settlement |
| `domain/src/stage.rs` | Add `evidence_packet_json: Option<String>` to `StageExecution` domain struct |
| `db/migrations/006_evidence_and_preflight.sql` | Add `evidence_packet_json` to `stage_executions`, `delivery_preflight_json` to `runs`, MCP columns to `agent_executions` |
| `db/src/repos/stages.rs` | Persist/read `evidence_packet_json` on `StageExecution` rows |
| `mcp-server/src/server.rs` | Keep `report://{run_id}` / report resources reading the same durable failed-stage evidence artifact without creating a second report lane |
| **Delivery preflight** | |
| `engine/src/preflight.rs` | **NEW** — `validate_delivery_config()` |
| `engine/src/command_handler.rs` | Call preflight at run creation when delivery config present |
| `domain/src/run.rs` | Add `delivery_preflight_json: Option<String>` |
| `db/src/repos/runs.rs` | Persist/read `delivery_preflight_json` |
| **MCP resolution** | |
| `engine/src/mcp.rs` | **NEW** — `resolve_mcp_servers()`, `McpResolutionReport`, `McpActualReport`, `McpRuntimeRegistry` |
| `domain/src/agent.rs` | Add MCP provenance fields to `AgentExecution` domain struct: `requested_mcp_extensions_json`, `predicted_mcp_extensions_json`, `predicted_mcp_runtime_ids_json`, `actual_mcp_extensions_json`, `actual_mcp_runtime_ids_json`, `denied_mcp_extensions_json`, `mcp_session_startup_latency_ms` |
| `db/src/repos/agent_executions.rs` | Persist/read MCP provenance fields, including predicted/actual runtime-ID JSON, on agent execution rows |
| `workflow/src/plan.rs` | Add `requested_mcp_server_ids: Vec<String>` and runtime/provider binding to `ResolvedAgent` |
| `workflow/src/compiler.rs` | Extract `mcp` and resolved runtime binding from backend profile into `ResolvedAgent` |
| `acp/src/lib.rs` | Add `mcp_servers: Vec<McpServerDef>` to `ExecutionRequest` |
| `acp/src/transport.rs` | Pass resolved MCP servers to `session/new` instead of `[]` |
| `engine/src/executor.rs` | Resolve MCP servers before building ExecutionRequest |
| `engine/src/preflight.rs` | Reload machine MCP registry during preflight-style validation checks |
| `graphql-server/src/types/run.rs` | Thread per-execution MCP truth through run reads so northbound GraphQL stays bound to stage-owned agent execution rows rather than a pre-session-only summary |
| `graphql-server/src/types/stage.rs` | Expose per-execution MCP requested/predicted/actual/denied truth on stage/agent execution reads, or introduce the concrete execution-level GraphQL type there if needed |
| `mcp-server/src/server.rs` | Include per-execution MCP requested/predicted/actual/denied truth in `report://{run_id}` resource responses |
| `mcp-server/src/tools/reports.rs` | Include the same per-execution MCP requested/predicted/actual/denied truth in `reports.get` responses so the tool and resource paths do not diverge |
| `engine/src/lib.rs` | Register new modules |

---

## 4. Acceptance Criteria

### Failed-Stage Evidence
1. When a stage settles as Failed, `evidence_packet_json` is populated on `stage_executions` with a `FailedStageEvidencePacket` including mandatory fields and optional placeholders for parity-deferred fields (`canonical_outcome`, `transport_error_kind`, `output_presence`, `recovery_snapshot`) as `null` until explicit owner persistence is implemented.
2. Evidence artifact written to `failure-evidence/evidence-{stage_id}-attempt{n}.json` with `report_kind = "failed_stage_evidence"`.
3. `reports.get` MCP tool returns the failure evidence artifact for the run.
4. Evidence is built **at failure time**, not deferred to run completion.
5. No second report/export truth lane is introduced — evidence flows through existing `reports.get`.

### Delivery Preflight
6. When `StartRun` includes `delivery_configuration_json`, preflight validation runs. If passed → `delivery_preflight_json` persisted on Run, run proceeds.
7. **Preflight failure blocks run start** (e.g. repo root missing) → `StartRun` returns error, run is NOT created. Matches stable `deliveryPreflightBlocksStart` gate.
8. No preflight runs at release time — release readiness is the workflow's concern.
9. Persisted `DeliveryPreflightResult` and checks use `{ id, label, passed, detail }` for compatibility with current readers.

### MCP Resolution
10. `code_writer` with `backend_profile.mcp: ["filesystem"]` → ACP `session/new` receives `mcpServers` with the filesystem server definition.
11. Agents without `mcp` in their backend profile → `mcpServers: []` (unchanged).
12. Requested server missing, disabled, or unsupported in the machine-local ACP registry → `denied_extensions` plus `blocking_issues` are persisted in the resolution report, preflight marks the MCP check failed, and runtime session creation fails closed before ACP session startup.
13. MCP resolution reads requested intent from `backend_profile.mcp`, **not** from `required_tools`, and resolves executable definitions from the existing machine-local ACP registry contract (`~/.config/mcp/config.yaml`, optional `CHAINWORKS_CODEX_CONFIG_PATH`, one-time migration from `~/.config/goose/config.yaml`).
14. `runtime_id`/provider binding is explicit in the resolver contract and enforces runtime-scoped typing (`platform` vs `stdio`) before session startup.
15. MCP registry snapshot is refreshed at preflight and executor-start boundaries so operator edits are observed without daemon restart.
16. `requested_mcp_extensions_json`, `predicted_mcp_extensions_json`, `predicted_mcp_runtime_ids_json`, `actual_mcp_extensions_json`, `actual_mcp_runtime_ids_json`, and `denied_mcp_extensions_json` are persisted on agent execution records. `mcp_session_startup_latency_ms` is updated post-session. Report/comparison readers, GraphQL stage/execution reads, `report://{run_id}`, and `reports.get` expose the full requested→predicted→actual→denied chain rather than collapsing to a pre-session effective set.

---

## 5. Test Gate

```bash
proposal-048|p048)
  log "Proposal 048 control-plane gate: evidence + preflight + MCP"
  (
    cd "$ROOT_DIR/control-plane"
    cargo test --workspace 2>&1
  )
  log "Proposal 048 control-plane gate passed"
  ;;
```

---

## 6. Out of Scope

- **EvidencePackBuilder (run-level export)**: Shell-owned export path, not daemon internal truth. Separate proposal.
- **SignOffEvidencePackBuilder (cohort sign-off)**: Benchmark evaluation lane, not per-run. Separate proposal.
- **PreflightService (full workflow validation)**: Validates YAML, providers, skills. Broader than delivery config. Separate proposal.
- **Release-time readiness checks**: Governed by workflow artifact inputs (P044) and deterministic services (P045), not preflight.
- **MCP runtime registry redesign/management**: P048 resolves executable server definitions against the existing machine-local ACP registry contract. Redesigning that registry shape, ownership, or mutation workflows is a separate concern.
