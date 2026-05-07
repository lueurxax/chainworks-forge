# Rust Control Plane

Stable reference for the Rust + SQLite local control-plane daemon.

This document describes the implemented system at `control-plane/`. It is not a proposal or future-state design.

Related stable docs:

- [operator-experience.md](operator-experience.md)
- [runtime-contract.md](runtime-contract.md)
- [structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md)
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md)
- [p041-generated-artifact-schemas.md](p041-generated-artifact-schemas.md)
- [test-gates.md](test-gates.md)

## Purpose

The Rust control-plane daemon is a server-side parity replica of the orchestration logic that previously lived exclusively in the SwiftUI client. It owns:

- workflow progression and stage transitions
- approval waits and settlement
- retries and restart reconciliation
- cancellation flow
- projection updates for read models
- ACP runtime adapter coordination
- command journaling and startup repair

The daemon runs alongside the desktop application on the same machine. During the current phase, the SwiftUI client remains the canonical user-facing owner. The daemon provides shadow truth through GraphQL and MCP, validated before any authority transfer.

## Scope

This reference covers:

- the Rust workspace structure and crate responsibilities
- the persistence model (SQLite schema, WAL mode, projections)
- the workflow engine (state machine, transitions, fan-out, loops)
- the ACP transport layer and provider adapters
- the northbound boundary (GraphQL + MCP)
- the work queue and recovery service
- daemon startup and configuration

It does not cover the SwiftUI operator shell or the implemented thin-client read boundary. The server parity harness is a specialized engine extension described in [test-gates.md](test-gates.md) and [p041-generated-artifact-schemas.md](p041-generated-artifact-schemas.md).

## Architecture

The daemon is a single Rust binary built from an 8-crate workspace at `control-plane/`.

```text
                   GraphQL clients           MCP clients
                   (SwiftUI app)        (agents, CLI, automation)
                        |                        |
                        |  /graphql              |  /mcp
                        |  /graphql/ws           |  MODE=mcp (stdio)
                        v                        v
               +--------+--------+------+--------+---------+
               |                 daemon                     |
               |                                            |
               |  graphql-server  |  mcp-server             |
               |       |              |                     |
               |       +------+-------+                     |
               |              |                     |
               |       command-handler              |
               |              |                     |
               |       +------+------+              |
               |       |             |              |
               |  orchestrator   scheduler /        |
               |       |         work-queue         |
               |  domain-engine  background-executor|
               |       |             |              |
               |       +------+------+              |
               |              |                     |

               |           event-bus                        |
               |              |                             |
               +-------+------+------+------+---------------+
                       |             |      |
                    SQLite         ACP    local FS
                  (db crate)    adapters  (artifacts)
```

### Crate responsibilities

| Crate | Path | Role |
|---|---|---|
| `domain` | `crates/domain/src/lib.rs` | Value types, status enums, commands, events. No I/O. |
| `db` | `crates/db/src/lib.rs` | SQLite pool, migrations, repository modules, work item types. |
| `workflow` | `crates/workflow/src/lib.rs` | YAML workflow definition parsing, agent catalog loading, `RunPlan` compilation, and `PhaseBLeadResolver` compatibility mapping. |
| `acp` | `crates/acp/src/lib.rs` | ACP runtime manager, per-provider adapters, JSON-RPC 2.0 stdio transport. |
| `engine` | `crates/engine/src/lib.rs` | Orchestrator, command handler, background executor, work queue, recovery service, event bus, mediation settlement, and run-start rollout-contract preflight. |
| `graphql-server` | `crates/graphql-server/src/lib.rs` | async-graphql schema (queries, mutations, subscriptions) served over axum. |
| `mcp-server` | `crates/mcp-server/src/lib.rs` | MCP JSON-RPC server with tool dispatch, resource reads, stdio and HTTP transports. |
| `daemon` | `crates/daemon/src/main.rs` | Binary entry point. Wires all crates, runs startup recovery, enters mode dispatch. |

### Dependency flow

```text
domain  <--  db  <--  workflow
                  <--  acp
                  <--  engine  <--  graphql-server
                               <--  mcp-server
                               <--  daemon
```

`domain` has no dependencies on other workspace crates. `db` depends on `domain`. `engine` depends on `domain`, `db`, `workflow`, and `acp`. The server crates and `daemon` depend on `engine`.

## Boundary shape

The daemon exposes two northbound surfaces on a single port (default `0.0.0.0:4000`).
Both surfaces are authenticated with bearer tokens (P029) and filter their
visible area by the caller's principal class. See
[mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md)
for the full authentication and capability filtering reference.

### GraphQL

- `GET /graphql` -- playground UI
- `POST /graphql` -- queries and mutations
- `WS /graphql/ws` -- subscriptions

Queries: `ideas`, `idea`, `runs`, `run`, `stages`, `approvals`, `artifacts`.

**Implementation self-assessment summary extension:**
The `Run` type includes a nullable `implementationSelfAssessmentSummary` field that exposes structured assessment truth (status, verification, code tasks, handoff tasks) without requiring raw artifact parsing.

Mutations: `approveApproval`, `rejectApproval`.

GraphQL is the macOS UI read/subscription surface plus the approval-gate
settlement surface. Non-approval operator commands such as starting runs,
retrying stages, cancelling runs, resolving workflow conflicts, and recovery
actions are MCP-only.

Subscriptions: `runStatusChanged`, `stageStatusChanged`, `approvalRequested`, `approvalResolved`, `runtimeStatusChanged`.

Implementation: `control-plane/crates/graphql-server/src/schema.rs`.

### MCP

Two transports serve the same `McpServer` logic:

**Streamable HTTP** (daemon mode): `POST /mcp` with `Mcp-Session-Id` header tracking. Defined in `crates/mcp-server/src/http.rs`.

**stdio** (`MODE=mcp`): ndjson over stdin/stdout. Defined in `crates/mcp-server/src/server.rs`.

Tools are namespaced:

| Namespace | Tools |
|---|---|
| `ideas.*` | `ideas.create`, `ideas.list` |
| `runs.*` | `runs.start`, `runs.list`, `runs.get`, `runs.cancel`, `runs.main_sync.request`, `runs.main_sync.retry`, `runs.main_sync.set_override`, `runs.main_sync.repair_state`, `runs.main_sync.record_recovery_decision`, `runs.knowledge_capsule.ignore`, `runs.settle_proposal_gate` |
| `approvals.*` | `approvals.list`, `approvals.resolve` |
| `stages.*` | `stages.retry` |
| `reports.*` | `reports.get` |

**Implementation self-assessment detail extension:**
`runs.get` and `runs.list` (detail view) include `implementation_self_assessment_summary` in the response payload.

**Operator Retry Instruction readback (P065):**
`runs.get` includes compact retry-instruction provenance. `reports.get` includes full binding and delivery records, including raw text for operator-class principals.

**Toolchain Mapping Readback:**
`reports.get` includes a `toolchain_mapping` summary for each execution, detailing mapping status, effective scope, and relative root suffixes.

Resources follow two URI families:

**Entity URIs** (P027 contract):
- `run://{run_id}` -- full projection for a single run
- `idea://{idea_id}` -- idea metadata
- `artifact://{artifact_id}` -- artifact metadata
- `report://{run_id}` -- execution report with stages and artifacts

**Collection URIs** (`chainworks://` family):
- `chainworks://runs` -- active run projections
- `chainworks://ideas` -- idea list
- `chainworks://approvals/inbox` -- pending approvals
- `chainworks://runs/{run_id}/stages` -- stage summaries
- `chainworks://runs/{run_id}/artifacts` -- artifact index

Implementation: `crates/mcp-server/src/server.rs` (dispatch and resource reads), `crates/mcp-server/src/tools/` (per-namespace handlers).

## Worktree safety and mutation barrier (P064)

The daemon implements a worktree-safety contract that protects implementation worktrees during concurrent access and main-sync operations.

### Worktree access mode
Every work item or system task declares its worktree access:
- `none` — no worktree access.
- `read` — read-only access (e.g. review agents).
- `write` — write-enabled access (e.g. implementation agents).

### Worktree mutation barrier
The `WorktreeMutationBarrier` ensures that sensitive operations like `git merge` or `git archive` happen in isolation.
- Active sync acquires an exclusive barrier.
- While the barrier is active, the scheduler blocks new `read` and `write` work for the same worktree.
- Existing consumers holding a lease must complete or expire before the barrier moves from `pending` to `active`.

### Main sync and knowledge capsules
- **Main Sync**: Orchestrated synchronization of local main into a run worktree with durable attempt history and conflict routing.
- **Knowledge Capsules**: Compact cross-run knowledge emitted from completed runs, matched and injected into future runs to prevent repeat mistakes.

Note: Main sync and knowledge capsule logic is currently in **Phase 0 contract freeze**.

## Workflow engine
...

### Compilation

The workflow compiler at `crates/workflow/src/compiler.rs` transforms a workflow YAML definition + agent catalog YAML into a `RunPlan` (`crates/workflow/src/plan.rs`). The plan contains:

- `initial_state` -- entry point for the state machine
- `states` -- map of state ID to `CompiledState` (owner agent, tasks, transitions, loop config)
- `variables` -- resolved workflow variables (YAML to JSON)
- `artifact_paths` -- name-to-path-template map from the catalog's `artifacts:` section

Each agent reference is resolved against backend profiles in the catalog to produce a `ResolvedAgent` with provider, model, effort, and system prompt.

Provider names are normalized: `claude_acp` becomes `claude`, `codex_acp` becomes `codex`, `gemini_cli_acp` becomes `gemini`.

### State machine

The orchestrator at `crates/engine/src/orchestrator.rs` drives runs through the compiled state machine. Stages are created lazily -- only when the orchestrator enters a state for the first time (or on loop iteration).

State types:

- **Compute state** -- creates a `StageExecution` with status `Running`, enqueues `InvokeAgent` work items for each task. If no explicit tasks, the owner agent runs as a single task.
- **Manual gate** (`is_manual_gate`) -- creates a `StageExecution` with status `WaitingApproval` and an `Approval` record. The run pauses until the operator approves or rejects.
- **End state** (`is_end`) -- marks the run `Completed`.

### Fan-out parallel tasks

When a state defines multiple tasks in a `parallel:` block, the orchestrator enqueues one `InvokeAgent` work item per task. The background executor spawns each `InvokeAgent` as a concurrent tokio task. After all tasks complete (checked by counting completed/failed work items for the stage), the orchestrator settles the stage.

Single-task stages settle immediately after the agent completes.

### Transition evaluation

After a stage completes, the orchestrator evaluates transition conditions in order
via the **Transition Authority Resolver**. The first matching declarative
transition wins. Agent-authored hints are treated as advisory only.

**Authority Rules:**
- The compiled workflow graph is the only authority.
- Agent-authored hints (`next_stage`, `next_action`) are advisory evidence.
- Unknown catalog artifact references (`exists(unknown_artifact)`) never evaluate
  to true; they fail closed as `invalid_expression` or `missing_input`.

**Aggregate Artifact Field Authority:**
To ensure deterministic evaluation, aggregate artifact fields are classified by
authority. For `proposal_review_summary_v1`:
- `pass` and `blocker_count` are **transition authoritative**.
- `next_action` and `next_stage` are **advisory only**.

Supported expression syntax (`crates/engine/src/orchestrator.rs`, line 487):
| Pattern | Meaning |
|---|---|
| `"true"` / `"false"` | Boolean literals |
| `exists('artifact_name')` | Checks filesystem for artifact at catalog path |
| `approval.granted == true` | Checks if any approval for the run was granted |
| `approval.rejected == true` | Checks if any approval for the run was rejected |
| `lhs == rhs`, `!=`, `<`, `<=`, `>`, `>=` | Comparison (numeric or string) |
| `expr and expr`, `expr or expr` | Logical connectives (parenthesis-aware split) |

**Workflow Conflict Persistence:**
If no transition matches or multiple match without a tie-break, the orchestrator
persists a `WorkflowConflictRecord` to the `workflow_conflicts` table. Non-blocking
rejected hints are recorded in `workflow_advisory_rejections`.

**Implementation self-assessment mapping:**
Transition expressions can inspect `implementation_self_assessment_v2.status` and other fields. These resolve against the domain-owned assessment summary projection rather than raw files.

Value resolution supports: `vars.name` (plan variables), `artifact.field` (read JSON file, extract field with dot-path), boolean/number/string literals.

If no transition matches and the state has transitions defined, the run becomes `Blocked`.

### Loop support

States with a `loop_config` track iterations by counting `StageExecution` records for the state. When iterations reach `max`, the loop-back transition is skipped and the orchestrator falls through to non-loop transitions.

Loop `max` can be a literal integer or a variable reference (`vars.max_proposal_revision_cycles`).

## ACP transport

The ACP layer at `crates/acp/` manages agent code process (ACP) sessions with external AI agents over JSON-RPC 2.0 ndjson stdio.

### Protocol flow

Defined in `crates/acp/src/transport.rs`:

1. **`initialize`** -- establish protocol version and client identity (`chainworks-control-plane 0.1.0`).
2. **`session/new`** -- start an agent session with provider-specific config (model, mode, extras). Returns `sessionId`.
3. **`session/prompt`** -- submit the prompt. Stream `session/update` notifications until the terminal response arrives.
4. **`session/close`** -- clean shutdown request (best-effort). The runtime manager sends this even when `session/prompt` returns a transport error after `session/new`.
5. Drop stdin (EOF) and wait up to 5 seconds for graceful exit, then signal the provider subprocess process group before falling back to direct kill.

### Permission auto-grant

When the subprocess sends `session/request_permission`, the transport auto-grants by selecting `allow_once` (or `approved` as fallback). This matches the autonomous execution model. See `build_permission_grant()` in `crates/acp/src/transport.rs`.

### Artifact discovery

Bounded discovery replaces broad pre-prompt workspace scanning with an engine-owned settlement pipeline. The transport captures deterministic digest-backed pre-prompt metadata only for declared outputs. After the prompt completes, the engine builds `OutputDiscoveryDecision` records from exact expected paths, provider output envelopes, control-plane generated manifests, and a bounded scan of the current run's `chainworks_meta_root` (maximum 500 files, 10 MiB aggregate size unless sampled defaults are tuned).

Legacy recursive broad discovery is post-prompt only, disabled by default, and requires an explicit `discovery.legacy_broad_discovery_policy: workflow_opt_in` in the workflow YAML or an operator `Command::RetryStage` override for frozen runs. Discovery decisions are written to `agent_execution_discovery_diagnostics` and mapped to runtime facts output settlement.

### Per-adapter session config

Each provider adapter specifies an `AcpSessionConfig` with:

| Field | Claude | Codex | Gemini / Auggie / Junie |
|---|---|---|---|
| `model` | `"default"` | `"o4-mini"` | varies |
| `mode` | `"bypassPermissions"` | `"full-access"` | `"bypassPermissions"` |
| `extra` | `_meta.claudeCode.options` | `None` | `None` |

### Registered adapters

The `AcpRuntimeManager` (`crates/acp/src/manager.rs`) pre-registers five adapters:

| Adapter | Provider name | Binary env var |
|---|---|---|
| `ClaudeAgentAdapter` | `claude` | `CHAINWORKS_CLAUDE_ACP_BINARY` |
| `CodexAdapter` | `codex` | `CHAINWORKS_CODEX_ACP_BINARY` |
| `GeminiCliAdapter` | `gemini` | `CHAINWORKS_GEMINI_ACP_BINARY` |
| `AuggieAdapter` | `auggie` | `CHAINWORKS_AUGGIE_ACP_BINARY` |
| `JunieAdapter` | `junie` | `CHAINWORKS_JUNIE_ACP_BINARY` |

Each adapter reads its binary path from the environment at construction and spawns the subprocess with piped stdio in its own process group when `execute()` is called.
`JunieAdapter` passes `--acp true` at launch so the local Junie CLI enters ACP JSON-RPC mode.

### Timeouts

- Handshake: 90 seconds by default; 120 seconds for Gemini
- Idle (no message): 300 seconds (reset on every received line)
- Shutdown wait: 5 seconds

## Persistence model

### SQLite configuration

The `db` crate creates the pool at `crates/db/src/pool.rs`:

- WAL journal mode (concurrent readers + one writer)
- 30-second busy timeout
- Up to 5 connections
- Auto-create database file if missing
- Migrations applied automatically on pool creation

### Capacity-aware scheduling and backpressure

The active local target keeps SQLite as the source of truth and scales by bounding
work rather than by adding external infrastructure:

- 5 active runs should be stable without operator babysitting.
- 10 active runs are allowed only when executor backpressure keeps active agent executions bounded.
- Active run count is not active agent execution count; surplus agent work remains queued.
- Active execution target: 20 total.
- Default provider caps: Gemini 4, Codex 10, Claude 8, Auggie 1, Junie 1.
- Provider aliases are normalized to canonical families: `claude`, `gemini`, `codex`, `auggie`, `junie`.
- Capacity pressure leaves work pending/backpressured; capacity alone must not mark work failed.
- Capacity state is durable, visible to operators, and supports backpressure alerts via GraphQL subscriptions and MCP notifications.

### SQLite write serialization and gateway (DbWriter)

> **Phases 1–2 implemented (Phases 3–8 deferred):** Phase 1 landed the DbWriter type, write classification types (`WriteClass`, `WriteLane`, `WriteOperation`, `WriteResult`), priority lane constants, write-bypass allowlist, and operation registry. Phase 2 wires the real bounded MPSC executor: per-lane channels at the documented capacities, priority drain (`CriticalBarrier`/`OperatorCommand` polled before lower lanes via biased `tokio::select!`), enqueue-to-commit deadline accounting (`WriteTimeout`), busy-error classification (`WriteBusyExhausted`), 1 Hz heartbeat with `is_alive()`, lane starvation watchdog incrementing `lane_starvation_total`, and graceful shutdown that rejects new B/C/D writes and drains Class A within `SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS` followed by a Class B sub-budget flush. The Class B coalescing buffer, evidence file spool module, orphan sweep, telemetry rollup persistence, and `storageHealth` GraphQL/MCP diagnostics are still deferred to Phases 3–6. Behaviors not yet active are called out inline.

The engine enforces a single-writer model for all domain mutations through a dedicated
write coordination layer, the `DbWriter` (Proposal 075):

- **Single Bounded Gateway**: All non-test runtime writes must route through `DbWriter`. This prevents direct pool-level write contention and ensures write discipline.
- **Write Classification**: Every write operation declares a class and priority lane:
    - **Class A (Barrier)**: Synchronous, durable, never dropped. Used for canonical state transitions and side-effect intent.
    - **Class B (Coalesced state)**: May be merged, delayed, or replaced (last-writer-wins). Used for noisy status updates and projection invalidations.
    - **Class C (Evidence metadata)**: File written and fsynced first; metadata pointer enqueued to DB. Used for high-volume logs and traces.
    - **Class D (Telemetry rollup)**: Aggregated in-memory summary; droppable under pressure.
- **Priority Lanes**: Six bounded lanes (CriticalBarrier, OperatorCommand, ProjectionInvalidation, CoalescedProjection, EvidenceMetadata, TelemetryRollup) ensure that critical transitions are not starved by noisy background work.
- **Shutdown Protocol**: Orderly drain of Class A lanes with specific budgets (e.g., `SHUTDOWN_CLASS_A_DRAIN_BUDGET_MS`) before termination.
- **Class A Deadline Override**: `WriteOperation.deadline` defaults to the per-class default (Class A: 2 s) and is hard-capped at `MAX_CLASS_A_DEADLINE` (5 s). Callers requesting a longer Class A deadline must set `WriteOperation.deadline_reason` to a static justification string; unannotated long deadlines are rejected with `WriteRejected{reason="deadline_exceeds_policy"}`. Per proposal 075 §architecture.deadlines_and_results.caller_override.
- **Idempotency Key Discipline**: `WriteOperation.idempotency_key` rejects NUL and ASCII control characters at validation time (`WriteRejected{reason="idempotency_key_invalid"}`). DbWriter logs emit only an 8-byte SHA-256 fingerprint (`idempotency_key_hash`), never the raw key, so producer-derived path or run identifiers cannot leak into structured logs.
- **Fail-Closed Admission Validation**: `WriteOperation::validate()` rejects misclassified writes before admission with explicit `WriteRejected` reasons: `class_lane_incompatible` (lane does not accept the declared class), `class_a_barrier_required` (Class A write missing `barrier=true`), and `replay_policy_class_mismatch` (replay policy not permitted for the declared class — e.g. Class A requires `NaturalKey` or `CallerGuarded`, Class B requires `LastWriterWins`, Class C requires `ChecksumIdempotent`, Class D requires `TelemetryMerge`). This prevents a misclassified telemetry or coalesced write from being admitted into a barrier lane.
- **Transaction Scope**: Multi-row invariants run in a single transaction under
  `BEGIN IMMEDIATE` with bounded retry (P061 primitive).
- **Atomic Supersession**: Operations like `RetryStage` atomically supersede old
  stage attempts, active agent executions, work items, and artifact source claims
  to prevent stale running work.
- **Excluded I/O**: Provider I/O, filesystem scans, and network waits never run
  inside a database transaction.
- **Write Coordination**: The `engine` crate owns command ordering and recovery
  semantics, while `db` exposes transaction-scoped repository methods and the `DbWriter` actor.
- **Command Latency**: The system targets p95 command latency (approve, retry, cancel)
  below 2 seconds even under saturated agent load.
- **Contention Monitoring**: DB write lock wait time and transaction duration are
  instrumented. `storageHealth.writer.alive` surfaces writer-actor heartbeats.

### Evidence spooling

> **Phase 1 implemented (Phase 3 deferred):** The evidence_spool_refs and storage_write_pressure_snapshots schemas and metadata repository are implemented. The evidence file spool module (temp write, checksum, fsync, atomic rename, orphan sweep) is deferred to Phase 3.

High-volume runtime evidence is spooled to the local filesystem instead of being inserted row-by-row into SQLite (Proposal 075):

- **Metadata Pointers**: SQLite stores compact metadata (path, checksum, size, kind, owner) in the `evidence_spool_refs` table.
- **Path Containment**: `relative_path` is enforced both in Rust (`validate_relative_path`) and at SQL level (migration `048_p075_evidence_path_constraints.sql`): no absolute paths, no `..`/`.` traversal segments, no backslash separators, no empty segments, length capped at 2048 bytes; identity fields (run/stage/agent ids) capped at 512 bytes; checksum capped at 256 bytes. The same path validation is applied symmetrically on reads (`find_by_run_and_path`) so a backslash-spelled path cannot be silently coerced to a forward-slash equivalent on lookup.
- **Metadata String Hardening**: All producer-supplied identity and label strings (`id`, `run_id`, `stage_execution_id`, `stage_id`, `agent_execution_id`, `agent_id`, `content_type`) reject NUL and ASCII control characters before insertion. `checksum_algorithm = sha256` requires exactly 64 lowercase hex digits. `producer_operation` must be a registry-style ASCII token (letters, digits, `_`, `.`, `-`). Conflict errors emit only `run_id` length and a 12-character checksum prefix, never the raw value.
- **`summary_json` Compact-Fact Allowlist**: Per proposal §evidence_spool_ref_contract.summary_json, only a closed set of bounded scalar fields is accepted: `line_count`, `chunk_count`, `byte_count` (non-negative integer), `truncated` (boolean), `started_at`/`finished_at`/`first_timestamp`/`last_timestamp` (string ≤ 64 chars), and `producer_label`/`producer` (string ≤ 256 chars). Nested objects, arrays, unknown keys, oversized strings, and negative counts are rejected. Validation errors never echo raw producer-supplied keys or values — only key length and a fixed field-category token — so transcript fragments or log-injection payloads cannot reach diagnostics output.
- **`summary_json` Canonicalization**: The persisted form is the re-serialized output of the parsed JSON object (`canonicalize_summary_json`), not the raw producer string. This neutralizes duplicate-key smuggling — a payload like `{"line_count":1,"line_count":"<raw transcript>"}` would otherwise round-trip its second value through SQLite even though `serde_json::Map` parsing keeps only the last key. Both `insert_tx` and `insert_idempotent` bind the canonical form.
- **Filesystem First**: The evidence file must be written, checksummed, and fsynced to disk **before** the Class C metadata write is enqueued to `DbWriter`.
- **Artifact Integration**: Spooled evidence is integrated into the settlement pipeline and exposed via the `Artifact` domain model.
- **Categories**: Transcripts, tool-traces, stdout/stderr snippets, and raw runtime events are primary candidates for spooling.

### Repository boundary and guardrails

The system enforces the single-writer model through a strict repository boundary. All runtime writes must be approved and registered.

- **Write-Bypass Allowlist**: A checked-in TOML file at `control-plane/crates/db/write-bypass-allowlist.toml` tracks all authorized direct DB writes (migrations, tests, startup repair, and temporary rollout bypasses). Every entry requires an owner, reason, scope, retirement criteria, and an expiration phase.
- **Write Operation Registry**: A checked-in TOML file at `control-plane/crates/db/write-operation-registry.toml` maps every `WriteOperation.operation_name` to its class, replay policy, and idempotency key kind.
- **Gate Enforcement**: The `proposal-075` gate (Phase 7 fail-closed) fails on unlisted runtime write owners, entries missing retirement data, or operations not present in the registry.

### WAL and checkpoint policy

The system explicitly manages the SQLite Write-Ahead Log (WAL) to prevent unbound growth:

- **PASSIVE Checkpoint**: Requested by a low-priority maintenance task when WAL exceeds `WARN_WAL_SIZE_BYTES` (128 MiB) and no Class A write is waiting.
- **CRITICAL Threshold**: `CRITICAL_WAL_SIZE_BYTES` (512 MiB) drives the `storageHealth.wal` warn/critical bands. The approved P075 policy authorises only PASSIVE above 128 MiB and TRUNCATE on shutdown or explicit maintenance — no hard barrier-coordinated upper bound is wired.
- **Shutdown Checkpoint**: A TRUNCATE checkpoint is performed on graceful shutdown after the Class A drain, or via an explicit maintenance command.

### Provider runtime homes and toolchain caches

Provider runtime homes are isolated from writable toolchain cache roots. Each
agent entry in the catalog can define a `toolchain_cache_policy` to control how
build and toolchain caches are mapped and isolated.

**Policy Scope:**
- `run` (default for Xcode): Cache root is tied to the run ID. Preserves 
  incremental build value across sessions in the same run. Serialized behind 
  an exclusive per-run lease for host-executed Xcode tools.
- `session` (default for Go): Cache root is tied to the ACP session 
  generation. Naturally resets after crashes or explicit session reuse failure.

**Adapter-Specific Mappings:**
- **Xcode**: Redirects `DerivedData`, `SourcePackages`, and `TMPDIR` via 
  `-derivedDataPath` and `-clonedSourcePackagesDirPath` arguments plus 
  environment shaping. Same-run Xcode work is serialized to prevent 
  concurrent DerivedData corruption.
- **Go**: Redirects `GOCACHE`, `GOMODCACHE`, `GOPATH`, and `TMPDIR`. 
  Enforces `GOENV=off` to prevent host-global overrides.

The scheduler remains language-neutral: it allocates bounded execution capacity 
and writable roots, while provider adapters map the generic toolchain root to 
tool-specific environment variables or command arguments.

**Diagnostics and Readback:**
Each `AgentExecution` records `actualToolchainMappingDiagnostics` (GraphQL) / 
`actual_toolchain_mapping_diagnostics` (MCP/Report). This document includes 
setup status, effective scope, created directories, and any validation or 
queue-wait latency.

**Housekeeping and Cleanup:**
- **Startup Recovery**: `startupRecoverySummary.toolchainCache` reports on 
  session-scoped root reclamation after daemon restarts.
- **Periodic Housekeeping**: `toolchainCacheHousekeepingSummary` reports on 
  run-scoped root pruning (default 7-day retention) and disk-pressure 
  eviction health.

### Schema

The database schema is evolved through migrations located at `control-plane/crates/db/migrations/`. These migrations define the canonical domain tables, support projections for client readback, and metadata for scheduling and recovery.

**Canonical domain tables** (e.g., `001_initial.sql`, `003_workflow_state_machine.sql`, `025_p017_workflow_conflicts.sql`, `037_p066_toolchain_cache_mapping.sql`, `044_p084_rollout_contract.sql`, `045_p084_rollout_contract_readback.sql`, `046_p075_evidence_spool_refs.sql`, `047_p075_storage_write_pressure_snapshots.sql`, `048_p075_evidence_path_constraints.sql`):

| Table | Purpose |
|---|---|
| `ideas` | Idea backlog items with status, workspace path |
| `runs` | Run lifecycle: status, workflow binding, current state, timestamps, cancellation |
| `stage_executions` | Per-stage execution records with iteration and attempt tracking |
| `agent_executions` | Per-agent invocation records (status, **actual_toolchain_mapping_diagnostics_json**, etc.) |
| `workflow_conflicts` | Blocking graph-authority conflicts (run_id, fingerprint, status, reason, current_mediation_id) |
| `workflow_advisory_rejections` | Non-blocking historical records of rejected agent hints |
| `lead_conflict_mediations` | Durable mediation lifecycle (id, run_id, conflict_id, status, lead_agent_id, settlement_result) |
| `lead_mediation_confirmations` | Separate store for mediation confirmations (id, mediation_id, status, deadline_at, suggested_action) |
| `workflow_conflict_metric_events` | Durable rollout metric events for workflow conflict recovery, mediation, Phase C validation, and dogfood decisions |
| `main_sync_attempts` | P064: Lifecycle of worktree sync attempts (status, preservation commit, merge commit, results) |
| `main_sync_conflict_files` | P064: Files that conflicted during a sync attempt |
| `run_knowledge_capsules` | P064: Compact cross-run knowledge capsules emitted from terminal runs |
| `run_knowledge_capsule_match_keys` | P064: Search keys for capsule relevance matching (proposal id, artifact path, etc.) |
| `run_knowledge_capsule_attachments` | P064: Links between matching capsules and an active run |
| `retry_operator_instruction_bindings` | P065: Durable parent bindings for operator-guided retries (ARCH-065) |
| `retry_operator_instruction_deliveries` | P065: Per-work-item delivery records for retry instructions (ARCH-065) |
| `evidence_spool_refs` | P075: Compact metadata pointers to high-volume evidence files |
| `storage_write_pressure_snapshots` | P075: Durable snapshots of writer lane depth and lock wait latency |
| `approvals` | Approval requests with decision, timestamps, expiry |
| `artifacts` | Artifact metadata (file path, format, checksum, provider, report kind) |
| `work_items` | Internal work queue (kind, payload, status, attempts, errors) |
| `command_journal` | Audit trail for mutating commands (type, payload, result, errors, caller metadata) |

**AgentExecution Owner Migration:**
To support lead-mediated conflicts without synthetic stage states, `agent_executions`
migrated to a general owner model:
- `owner_kind`: `stage_execution` or `lead_conflict_mediation`.
- `owner_id`: References either `stage_execution_id` or `mediation_record_id`.
- `stage_execution_id` remains as a nullable compatibility field.
- This allows mediation-owned executions to reuse the same retry, quota,
  artifact, and cost infrastructure as stage-owned executions.

**Projections and read-model tables** (e.g., `002_projections.sql`, `038_p066_cleanup_readbacks.sql`):

| Table | Purpose |
|---|---|
| `run_summaries` | Materialized run projections (stage counts, approval counts) |
| `stage_summaries` | Materialized stage projections (status, artifacts, approvals) |
| `scheduler_queue_summaries` | Durable aggregate readback for queued/backpressured work |
| `scheduler_health_snapshots` | Durable health readback for counts, pressure, latency |
| `toolchain_cache_housekeeping_readbacks` | Low-churn projection for periodic toolchain cleanup health |
| `approval_inbox` | Pending approval projection for operator surfaces |
| `artifact_index` | Artifact discovery projection (format, pinned, report kind) |
| `artifact_contract_summaries` | Structured verification truth for implementation assessment |

**Scheduling, recovery, and host-interruption tables** (e.g., `021_scheduler_backpressure_foundation.sql` to `023_scheduler_backpressure_hysteresis_counters.sql`):

| Table | Purpose |
|---|---|
| `scheduler_service_state` | Durable least-recently-served state for fairness |
| `host_interruption_epochs` | Detected host sleep/wake and network migration epochs |
| `host_interruption_affected_executions` | Executions affected by a host interruption epoch |
| `startup_recovery_readbacks` | Durable readback for startup recovery progress and backpressure |
| `recovery_recommendations` | Operator-facing recovery suggestions per run/stage |
| `session_lineages` | Session reuse/reset metadata per agent |
| `agent_execution_runtime_facts` | Durable provider-independent execution truth |
| `artifact_source_claims` | CAS-backed ownership for artifact generation |

### Projection rebuild

Projections are rebuilt after every mutation that changes run state. The rebuild is triggered by the background executor (`crates/engine/src/executor.rs`) and command handler (`crates/engine/src/command_handler.rs`) via `projections::rebuild_all_for_run()`.

This keeps projections eventually consistent with canonical tables within a single work item cycle.

## Metrics and Observability

The control plane records rollout metrics as durable `workflow_conflict_metric_events`
rows so operator feedback and dogfood gates are auditable from repository-backed
state, not only process logs.

### Rollout Metrics

| Metric | Type | Labels | Description |
|---|---|---|---|
| `workflow_conflict_time_to_resolution_seconds` | Histogram event | `conflict_reason`, `resolution_mode` | Time from conflict detection to resolution or terminal settlement. |
| `conflict_reason_to_action_outcome_total` | Counter event | `conflict_reason`, `action_class`, `terminal_status` | Counts outcomes (resolved, terminal, superseded) per conflict reason. |
| `recovery_action_chosen_total` | Counter event | `conflict_reason`, `action_class`, `source_surface`, `result` | Counts chosen recovery actions (retry, clone, manual_fallback). |
| `phase_c_validation_outcome_total` | Counter | `outcome` | Phase C validation results: `static_fail`, `preflight_fail`, `legacy_catalog_warning`, `pass`. |

## Work queue

The work queue (`crates/engine/src/work_queue.rs`) wraps the `work_items` SQLite table with claim-next / complete / fail semantics.

### Work item kinds

| Kind | Behavior |
|---|---|
| `InvokeAgent` | Spawns ACP session via the runtime manager. Concurrent (tokio::spawn). |
| `AdvanceRun` | Re-evaluates run state through the orchestrator. Inline. |
| `TriggerNextStage` | Alias for AdvanceRun. Inline. |
| `SettleStage` | Alias for AdvanceRun. Inline. |
| `RebuildProjection` | Rebuilds all projections for a run. Inline. |
| `StartupRepair` | Runs the recovery service. Inline. |

The background executor (`crates/engine/src/executor.rs`) polls the queue in a loop with 100ms sleep when idle. `InvokeAgent` items are spawned as concurrent tasks; all other kinds run inline on the executor loop.

## Capacity-aware Scheduling

The executor uses a capacity-aware claim/start gate for `InvokeAgent`: it checks global,
provider, and per-run active execution caps before mutating ownership, and leaves
capacity-blocked work pending.

- **Default Caps**: Global 20, per-run 4, Claude 8, Gemini 4, Codex 10, Auggie 1, Junie 1.
- **Backpressure Visibility**: Blocked work remains `pending` and is exposed via
  `scheduler_queue_summaries` and `scheduler_health_snapshots` projections.
- **Wake-up**: `InvokeAgent` completion inserts an idempotent post-completion
  `AdvanceRun` wake-up inside `work_items.complete`, so fan-in observes the
  completed work item before settling the stage.

### Scheduler Fairness

The scheduler ensures that no single run or provider family starves others:

- **Bounded Candidate Window**: Reads a window of pending work (default 20) ordered
  by `scheduled_at` and `rowid`.
- **Least-Recently-Served**: Selects the oldest eligible item from the run that was
  least recently served, using the `scheduler_service_state` table to persist
  fairness state across restarts.
- **Deterministic Tie-breaking**: Uses `scheduled_at` then `rowid`.
- **Hot Indexes**: Scans are backed by hot indexes on `work_items` and
  `agent_executions` to ensure O(1) or O(log N) lookup at scale.

## Command handler

The command handler at `crates/engine/src/command_handler.rs` processes eleven command types. Every command is recorded in the `command_journal` table before execution and marked completed or failed afterward.

| Command | Effect |
|---|---|
| `StartRun` | Validates YAML (if provided), inserts run, activates idea, enqueues `AdvanceRun`. |
| `ApproveStage` | Resolves approval as Granted, settles manual gates or activates compute stages, enqueues `AdvanceRun`. |
| `RejectStage` | Resolves approval as Rejected, marks stage Blocked. |
| `RetryStage` | Marks old stage Skipped, creates new `StageExecution` with incremented attempt, enqueues `AdvanceRun`. Supports optional `operator_instruction` (P065). |
| `ResolveWorkflowConflictTransition` | Resolves a blocking workflow conflict by selecting a legal graph transition manually. |
| `OverrideLegacyDiscoveryPolicy` | Overrides the artifact discovery policy for a specific stage execution. |
| `CancelRun` | Sets run to Cancelling, rebuilds projections. |
| `ResetSession` | Resets stage to Pending, enqueues StartupRepair. |
| `RunStewardAnalysis` | Triggers a Steward system-health analysis. |
| `OverrideArtifactContract` | Applies a manual operator override to an artifact contract's status. |
| `ResolveLeadMediationConfirmation` | Resolves a lead mediation confirmation via the engine-owned settlement boundary. |
| `MainSyncRequest` | P064: Queue or dedupe a main-sync request (Phase 0 contract only). |
| `MainSyncRetry` | P064: Retry a failed sync attempt (Phase 0 contract only). |
| `MainSyncSetRunOverride` | P064: Set per-run main-sync mode override (Phase 0 contract only). |
| `MainSyncRepairState` | P064: Reconcile sync state after failure (Phase 0 contract only). |
| `MainSyncRecordRecoveryDecision` | P064: Record operator recovery decision (Phase 0 contract only). |
| `KnowledgeCapsuleIgnore` | P064: Mark a capsule as ignored for the current run (Phase 0 contract only). |

## Recovery service

The recovery service at `crates/engine/src/recovery.rs` runs at daemon startup (before the mode dispatch). It:

1. Loads all active (non-terminal) runs.
2. Finds stages stuck in `Running` status (orphaned from a crash).
3. Marks stuck stages as `Blocked`.
4. Records repair actions in `startup_repairs` table.
5. Creates recovery recommendations in `recovery_recommendations` table.
6. Re-enqueues `AdvanceRun` for affected runs.

The service returns a `RecoverySummary` with counts of inspected runs, repaired runs, and requeued work items.

## Running the daemon

### Environment variables

| Variable | Default | Purpose |
|---|---|---|
| `DATABASE_URL` | `sqlite://./chainworks-control-plane.db` | SQLite connection string |
| `GRAPHQL_ADDR` | `0.0.0.0:4000` | Bind address for the HTTP server |
| `RUST_LOG` | `info` | Tracing filter (standard `tracing-subscriber` env filter) |
| `MODE` | `daemon` | `daemon` for HTTP server, `mcp` for stdio MCP server |
| `CHAINWORKS_INVOKE_AGENT_GLOBAL_CAP` | `20` | Global active agent execution cap |
| `CHAINWORKS_INVOKE_AGENT_PER_RUN_CAP` | `4` | Per-run active agent execution cap |
| `CHAINWORKS_INVOKE_AGENT_PROVIDER_CAP_{PROVIDER}` | varies | Provider-specific active execution caps (e.g. `_CLAUDE`, `_GEMINI`, `_CODEX`) |

Provider binary paths (required when executing agents):

- `CHAINWORKS_CLAUDE_ACP_BINARY` -- path to Claude Agent ACP binary
- `CHAINWORKS_CODEX_ACP_BINARY` -- path to Codex ACP binary
- `CHAINWORKS_GEMINI_ACP_BINARY` -- path to Gemini CLI ACP binary
- `CHAINWORKS_AUGGIE_ACP_BINARY` -- path to Auggie ACP binary
- `CHAINWORKS_JUNIE_ACP_BINARY` -- path to Junie ACP binary

### Startup sequence

1. Initialize tracing (stderr writer, env filter).
2. Read config from environment.
3. Create SQLite pool (runs migrations).
4. Create event bus (broadcast channel, capacity 1024).
5. Create work queue.
6. Create command handler.
7. Create ACP runtime manager (registers all adapters).
8. Create orchestrator.
9. Create and start background executor.
10. Run startup recovery.
11. Mode dispatch: daemon mode starts HTTP server; MCP mode starts stdio loop.

### Typical commands

```bash
# Build and run in daemon mode
cd control-plane
cargo run

# Run with explicit database path
DATABASE_URL="sqlite:///tmp/test.db" cargo run

# Run in MCP stdio mode (for Claude Code .mcp.json integration)
MODE=mcp cargo run --quiet

# Run with debug logging
RUST_LOG=debug cargo run
```

### Claude Code integration

Add to `.mcp.json`:

```json
{
  "mcpServers": {
    "chainworks-control-plane": {
      "command": "cargo",
      "args": ["run", "--quiet", "--manifest-path", "/path/to/control-plane/Cargo.toml"],
      "env": {
        "MODE": "mcp",
        "DATABASE_URL": "sqlite:///path/to/chainworks-control-plane.db"
      }
    }
  }
}
```

## Test gate

The proposal-027 gate verifies the full Rust workspace test suite:

```bash
./scripts/test-gate.sh proposal-027
```

This runs `cargo test --workspace` inside `control-plane/`. The gate covers:

- SQLite repository layer (ideas, runs, stages, approvals, artifacts)
- Projection rebuild and parity verification
- Command handling (start, approve, reject, retry, cancel)
- Startup repair and recovery
- Workflow compilation and plan structure
- ACP transport protocol
- MCP stdio server protocol

Integration tests are located in:

- `crates/db/tests/integration.rs`
- `crates/engine/tests/integration.rs`
- `crates/workflow/tests/integration.rs`
- `crates/acp/tests/integration.rs`
- `crates/daemon/tests/mcp_stdio.rs`

Additional focused gates:

- `./scripts/test-gate.sh proposal-058` for ACP failure classification and runtime facts.
- `./scripts/test-gate.sh proposal-061` for SQLite write serialization, executor backpressure, host-interruption recovery, scheduler-health readback, and generated-state housekeeping safety. The `proposal-061|p061` names are retained historical gate aliases for this implemented contract.
- `./scripts/test-gate.sh proposal-084` (retained historical alias `p084`) for the rollout-contract template, linter, fixtures, run-start preflight, parity-lane operator readback, and Swift read-only presentation slice. See [executable-rollout-gate-template.md](executable-rollout-gate-template.md).

## Key design decisions

These decisions are fixed for the baseline and are not under reconsideration. Active proposals may add bounded targets without changing these baseline choices:

1. **Local-first topology.** One daemon, one SQLite database, one local file store. No external orchestration platform, no distributed deployment.

2. **Single-process monolith.** GraphQL and MCP run on the same port in the same process. No service mesh, no sidecar.

3. **Application-owned orchestration.** The workflow engine is product-owned, not delegated to Temporal or any other external workflow platform. This trades platform power for direct control over semantics and simpler local deployment.

4. **SQLite as source of truth.** Both canonical domain state and materialized projections live in the same SQLite database. No separate event store.

5. **Artifact contents on local filesystem.** SQLite stores metadata (paths, checksums, provenance). File contents remain on disk.

6. **Lazy stage creation.** Stages are created only when the orchestrator enters a state, not upfront when the run starts.

7. **Client remained canonical during parity.** The SwiftUI app owned user-visible behavior during parity. The implemented thin-client boundary now consumes daemon-owned GraphQL projections for governed workflow truth.

8. **WAL mode for concurrent access.** Enables concurrent readers with one writer, with a 30-second busy timeout. The daemon keeps SQLite as the source of truth and uses explicit write serialization plus executor backpressure instead of relying on more writer concurrency.

9. **Bounded local concurrency target.** The local daemon target is 5 active runs stable, 10 active runs only with bounded scheduling, and up to 20 active agent executions. Excess work should queue visibly instead of starting every fan-out task immediately.

## Local daemon lifecycle and packaging

The control-plane daemon is a product-owned local macOS component, not just a developer process. Its typed lifecycle state, health/readiness surfaces, packaged supervision, PID lock, crash budget, SQLite startup safety, failed-serve behavior, diagnostics, and packaging proof lanes are documented in [local-daemon-lifecycle-supervision-and-packaging.md](local-daemon-lifecycle-supervision-and-packaging.md).

The retained gate aliases are operational:

- `./scripts/test-gate.sh proposal-042` proves implementation readiness for the local daemon lifecycle slice.
- `./scripts/test-gate.sh proposal-042-packaging` proves signed/notarized packaged-app readiness on a release host.

## Non-goals

The following items are explicitly out of scope for this baseline:

- **Broad command UI writes** -- governed SwiftUI remains read-only for non-approval commands; command transports require separate boundary work.
- **Product-polish UI restoration** -- visual/navigation restoration over the GraphQL read model is owned outside the Rust control-plane baseline.
- **Northbound MCP command plane** -- full external command surface (P029).
- **Multi-host or distributed deployment** -- no remote workflow platformization.
- **Proposal-loop telemetry projections** -- the `proposal_loop_metrics` table from the original design is deferred to a future telemetry slice.
- **Extended work item kinds** -- aggregate summary computation, score-lift backlog rebuild, and coverage validation are deferred to application-specific automation slices.
