# Rust Control Plane

Stable reference for the Rust + SQLite local control-plane daemon.

This document describes the implemented system at `control-plane/`. It is not a proposal or future-state design.

Related stable docs:

- [operator-experience.md](operator-experience.md)
- [runtime-contract.md](runtime-contract.md)
- [structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md)
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md)
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

It does not cover the SwiftUI operator shell, the thin-client cutover (P031), or the parity harness (P041).

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
| `workflow` | `crates/workflow/src/lib.rs` | YAML workflow definition parsing, agent catalog loading, `RunPlan` compilation. |
| `acp` | `crates/acp/src/lib.rs` | ACP runtime manager, per-provider adapters, JSON-RPC 2.0 stdio transport. |
| `engine` | `crates/engine/src/lib.rs` | Orchestrator, command handler, background executor, work queue, recovery service, event bus. |
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

The daemon exposes two northbound surfaces on a single port (default `0.0.0.0:4000`):

### GraphQL

- `GET /graphql` -- playground UI
- `POST /graphql` -- queries and mutations
- `WS /graphql/ws` -- subscriptions

Queries: `ideas`, `idea`, `runs`, `run`, `stages`, `approvals`, `artifacts`.

**Implementation self-assessment summary extension:**
The `Run` type includes a nullable `implementationSelfAssessmentSummary` field that exposes structured assessment truth (status, verification, code tasks, handoff tasks) without requiring raw artifact parsing.

Mutations: `startRun`, `approveStage`, `rejectStage`, `retryStage`, `cancelRun`.

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
| `runs.*` | `runs.start`, `runs.list`, `runs.get`, `runs.cancel` |
| `approvals.*` | `approvals.list`, `approvals.resolve` |
| `stages.*` | `stages.retry` |
| `reports.*` | `reports.get` |

**Implementation self-assessment detail extension:**
`runs.get` and `runs.list` (detail view) include `implementation_self_assessment_summary` in the response payload.

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

## Workflow engine

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

P053 bounded discovery replaces broad pre-prompt workspace scanning with an engine-owned settlement pipeline. The transport captures deterministic digest-backed pre-prompt metadata only for declared outputs. After the prompt completes, the engine builds `OutputDiscoveryDecision` records from exact expected paths, provider output envelopes, control-plane generated manifests, and a bounded scan of the current run's `chainworks_meta_root` (maximum 500 files, 10 MiB aggregate size unless sampled defaults are tuned).

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
| `ClaudeAgentAdapter` | `claude` | `CLAUDE_ACP_BIN` |
| `CodexAdapter` | `codex` | `CODEX_ACP_BIN` |
| `GeminiCliAdapter` | `gemini` | `GEMINI_ACP_BIN` |
| `AuggieAdapter` | `auggie` | `AUGGIE_ACP_BIN` |
| `JunieAdapter` | `junie` | `JUNIE_ACP_BIN` |

Each adapter reads its binary path from the environment at construction and spawns the subprocess with piped stdio in its own process group when `execute()` is called.

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

### SQLite write serialization

The engine enforces a single-writer model for all domain mutations through a dedicated
write coordination layer:

- **Transaction Scope**: Multi-row invariants run in a single transaction under
  `BEGIN IMMEDIATE` with bounded retry. This covers commands like `RetryStage`,
  `CancelRun`, and `StartupRepair` where multiple table updates must be atomic.
- **Atomic Supersession**: Operations like `RetryStage` atomically supersede old
  stage attempts, active agent executions, work items, and artifact source claims
  to prevent stale running work.
- **Excluded I/O**: Provider I/O, filesystem scans, and network waits never run
  inside a database transaction.
- **Write Coordination**: The `engine` crate owns command ordering and recovery
  semantics, while `db` exposes transaction-scoped repository methods.
- **Command Latency**: The system targets p95 command latency (approve, retry, cancel)
  below 2 seconds even under saturated agent load. The retained `proposal-061` gate alias enforces
  this under a load of 20 active fake agents.
- **Contention Monitoring**: DB write lock wait time and transaction duration are
  instrumented and exposed via GraphQL and MCP. SQLITE_BUSY retries are logged
  and surfaced if exhausted.
- **Host Interruption Recovery**: Detected host sleep/wake and network migration
  epochs classify affected executions, which are then cleaned up and requeued
  with jitter under capacity caps. Retries are exempt from provider quota budgets.

### Provider runtime homes and toolchain caches

Provider runtime homes are isolated from writable toolchain cache roots. The
Codex adapter derives a per-session toolchain root and publishes both
`CHAINWORKS_TOOLCHAIN_HOME` and `TOOLCHAIN_HOME` to the provider process. Rust
tooling uses subpaths under that root for `TMPDIR`, `RUSTUP_HOME`,
`CARGO_HOME`, and `CARGO_TARGET_DIR` so generated build/cache output does not
land inside read-only provider runtime homes or shared repository-global build
directories.

The scheduler stays language-neutral: it allocates bounded execution capacity
and writable roots, while provider adapters map the generic toolchain root to
tool-specific environment variables or command arguments. Swift/Xcode and Go
adapter-specific mappings extend this same contract; they must not add
language-specific scheduler capacity dimensions.

### Schema

The database schema is evolved through migrations located at `control-plane/crates/db/migrations/`. These migrations define the canonical domain tables, support projections for client readback, and metadata for scheduling and recovery.

**Canonical domain tables** (e.g., `001_initial.sql`, `003_workflow_state_machine.sql`, `021_p017_workflow_conflicts.sql`):

| Table | Purpose |
|---|---|
| `ideas` | Idea backlog items with status, workspace path |
| `runs` | Run lifecycle: status, workflow binding, current state, timestamps, cancellation |
| `stage_executions` | Per-stage execution records with iteration and attempt tracking |
| `agent_executions` | Per-agent invocation records (provider, model, status, **owner_kind**, **owner_id**) |
| `workflow_conflicts` | Blocking graph-authority conflicts by fingerprint and status |
| `workflow_advisory_rejections` | Non-blocking historical records of rejected agent hints |
| `lead_mediation` | State for lead-owned conflict resolution attempts |
| `approvals` | Approval requests with decision, timestamps, expiry |
| `artifacts` | Artifact metadata (file path, format, checksum, provider, report kind) |
| `work_items` | Internal work queue (kind, payload, status, attempts, errors) |
| `command_journal` | Audit trail for mutating commands (type, payload, result, errors, caller metadata) |

**AgentExecution Owner Migration (Phase B):**
To support lead-mediated conflicts without synthetic stage states, `agent_executions` 
migrated to a general owner model:
- `owner_kind`: `stage_execution` or `lead_conflict_mediation`.
- `owner_id`: References either `stage_execution_id` or `mediation_record_id`.
- `stage_execution_id` remains as a nullable compatibility field.
- This allows mediation-owned executions to reuse the same retry, quota, 
  artifact, and cost infrastructure as stage-owned executions.

**Projections and read-model tables** (e.g., `002_projections.sql`, `021_scheduler_backpressure_foundation.sql`):

| Table | Purpose |
|---|---|
| `run_summaries` | Materialized run projections (stage counts, approval counts) |
| `stage_summaries` | Materialized stage projections (status, artifacts, approvals) |
| `scheduler_queue_summaries` | Durable aggregate readback for queued/backpressured work |
| `scheduler_health_snapshots` | Durable health readback for counts, pressure, latency |
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

The command handler at `crates/engine/src/command_handler.rs` processes six command types. Every command is recorded in the `command_journal` table before execution and marked completed or failed afterward.

| Command | Effect |
|---|---|
| `StartRun` | Validates YAML (if provided), inserts run, activates idea, enqueues `AdvanceRun`. |
| `ApproveStage` | Resolves approval as Granted, settles manual gates or activates compute stages, enqueues `AdvanceRun`. |
| `RejectStage` | Resolves approval as Rejected, marks stage Blocked. |
| `RetryStage` | Marks old stage Skipped, creates new `StageExecution` with incremented attempt, enqueues `AdvanceRun`. |
| `CancelRun` | Sets run to Cancelling, rebuilds projections. |
| `ResetSession` | Resets stage to Pending, enqueues StartupRepair. |

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

- `CLAUDE_ACP_BIN` -- path to Claude Agent ACP binary
- `CODEX_ACP_BIN` -- path to Codex ACP binary
- `GEMINI_ACP_BIN` -- path to Gemini CLI ACP binary
- `AUGGIE_ACP_BIN` -- path to Auggie ACP binary
- `JUNIE_ACP_BIN` -- path to Junie ACP binary

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

## Key design decisions

These decisions are fixed for the baseline and are not under reconsideration. Active proposals may add bounded targets without changing these baseline choices:

1. **Local-first topology.** One daemon, one SQLite database, one local file store. No external orchestration platform, no distributed deployment.

2. **Single-process monolith.** GraphQL and MCP run on the same port in the same process. No service mesh, no sidecar.

3. **Application-owned orchestration.** The workflow engine is product-owned, not delegated to Temporal or any other external workflow platform. This trades platform power for direct control over semantics and simpler local deployment.

4. **SQLite as source of truth.** Both canonical domain state and materialized projections live in the same SQLite database. No separate event store.

5. **Artifact contents on local filesystem.** SQLite stores metadata (paths, checksums, provenance). File contents remain on disk.

6. **Lazy stage creation.** Stages are created only when the orchestrator enters a state, not upfront when the run starts.

7. **Client remains canonical during parity.** The SwiftUI app owns user-visible behavior until the thin-client cutover (P031). The daemon provides verifiable shadow truth.

8. **WAL mode for concurrent access.** Enables concurrent readers with one writer, with a 30-second busy timeout. The daemon keeps SQLite as the source of truth and uses explicit write serialization plus executor backpressure instead of relying on more writer concurrency.

9. **Bounded local concurrency target.** The local daemon target is 5 active runs stable, 10 active runs only with bounded scheduling, and up to 20 active agent executions. Excess work should queue visibly instead of starting every fan-out task immediately.

## Local daemon lifecycle and packaging

The control-plane daemon is a product-owned local macOS component, not just a developer process. Its typed lifecycle state, health/readiness surfaces, packaged supervision, PID lock, crash budget, SQLite startup safety, failed-serve behavior, diagnostics, and packaging proof lanes are documented in [local-daemon-lifecycle-supervision-and-packaging.md](local-daemon-lifecycle-supervision-and-packaging.md).

The retained gate aliases are operational:

- `./scripts/test-gate.sh proposal-042` proves implementation readiness for the local daemon lifecycle slice.
- `./scripts/test-gate.sh proposal-042-packaging` proves signed/notarized packaged-app readiness on a release host.

## Non-goals

The following items are explicitly out of scope for this baseline:

- **Thin-client cutover** -- authority transfer from client to daemon (P031).
- **Parity harness** -- golden-run comparison and behavioral diff tooling (P041).
- **Thin-client UI cutover** -- P043 finalized the GraphQL projection read contract, but user-visible macOS cutover remains owned by P031.
- **Northbound MCP command plane** -- full external command surface (P029).
- **Multi-host or distributed deployment** -- no remote workflow platformization.
- **Proposal-loop telemetry projections** -- the `proposal_loop_metrics` table from the original design is deferred to a future telemetry slice.
- **Extended work item kinds** -- aggregate summary computation, score-lift backlog rebuild, and coverage validation are deferred to application-specific automation slices.
