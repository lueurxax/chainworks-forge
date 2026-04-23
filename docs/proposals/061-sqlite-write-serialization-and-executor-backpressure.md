# Proposal 061: SQLite Write Serialization and Executor Backpressure

| Field | Value |
|---|---|
| Date | 2026-04-19 |
| Status | Implemented |
| Author | Andrey Khasanov |
| Depends on | [045-run-recovery-and-granular-retry-mcp-tools.md](045-run-recovery-and-granular-retry-mcp-tools.md), [051-shared-xcode-mcp-bridge-pool.md](051-shared-xcode-mcp-bridge-pool.md), [056-control-plane-engine-modularization-and-typed-contracts.md](056-control-plane-engine-modularization-and-typed-contracts.md), [060-lead-driven-reviewer-routing-and-expanded-reviewer-catalog.md](060-lead-driven-reviewer-routing-and-expanded-reviewer-catalog.md) |
| Scope | Keep the local Rust/SQLite control plane stable at 5 active proposal runs, bounded at 10 active runs, and capable of up to 20 active agent executions through explicit executor backpressure, provider caps, serialized write paths, and DB contention observability. |
| Goal | Let operators keep 5-10 proposal runs active while review fan-out can reach 20 active agents without ACP handshake storms, SQLite lock failures, stale running executions, or retry commands timing out under load. |

---

## 1. Context and Motivation

The local-first target remains a single Rust daemon with SQLite and a local artifact store. We are not moving to Postgres or a distributed workflow platform for this stage.

Dogfooding multiple proposal runs on 2026-04-19 exposed the practical ceiling of the current baseline:

- fan-out stages can enqueue many `InvokeAgent` work items at once;
- the background executor spawns `InvokeAgent` items concurrently without a capacity gate;
- ACP providers can accumulate stale processes and session handshakes;
- laptop sleep/wake or Wi-Fi migration can make healthy ACP sessions surface as provider idle timeouts;
- SQLite remains healthy as a file, but its single-writer model is stressed by bursty claim/start/settle/projection writes;
- operator commands such as retry can time out or see `database is locked` when the daemon is already saturated;
- skipped or failed stages can leave stale `running` agent execution records if recovery and retry do not close ownership cleanly.

The problem is not raw database size. The current observed database/WAL sizes are normal for local use. The problem is uncontrolled concurrency around a single SQLite writer plus provider runtimes that have much lower practical concurrency than the number of active runs.

This proposal sets the local scale target explicitly:

1. **Stable target:** 5 active runs should work without operator babysitting.
2. **Bounded stretch target:** 10 active runs may exist, but only when scheduler backpressure keeps active agent executions bounded.
3. **Active agent target:** up to 20 active agent executions is the desired review-fan-out ceiling, protected by global, per-run, and per-provider caps.

Active run count and active agent execution count are different. A run may be active while its next work item is queued behind capacity.

---

## 2. Product Questions This Proposal Must Answer

1. Can the daemon keep 5 active proposal runs healthy without SQLite lock failures or stale running executions?
2. Can the daemon allow up to 10 active runs while queueing surplus agent work instead of launching it all immediately?
3. Can operators see when work is queued because of global, provider, per-run, or DB writer capacity?
4. Can retry/approve/cancel commands remain responsive while agent work is saturated?
5. Can recovery requeue work without causing a startup thundering herd?
6. Can provider-specific caps protect fragile runtimes such as Gemini/Xcode-backed paths?
7. Can host sleep/wake or network migration become a bounded retryable interruption instead of a pile of permanent provider failures?
8. Can the proposal gate prove the behavior without requiring a remote deployment or Postgres?

---

## 3. Scope

This proposal includes:

- Executor-level backpressure before claiming or starting `InvokeAgent` work.
- Global active agent execution cap.
- Provider-specific active execution caps.
- Per-run active execution cap.
- Capacity-aware work item claim semantics.
- Read-only capacity precheck before acquiring the SQLite writer lock for `InvokeAgent` claim/start.
- Hot SQLite indexes for pending `InvokeAgent` scans and active execution count queries.
- Serialized DB write pathway for multi-row domain mutations.
- Shorter write transactions with no provider I/O inside transaction boundaries.
- A longer SQLite busy timeout and explicit retry/backoff for short `SQLITE_BUSY` cases.
- Runtime projections that expose queued/backpressured work and the reason.
- Recovery behavior that respects capacity when re-enqueuing work.
- Stale `running` execution repair for stages that are skipped, failed, or superseded.
- Host suspend/network-interruption detection and staggered retry for affected ACP executions.
- Language-neutral provider toolchain homes for daemon-launched agent sessions so build caches are writable without making Rust, Swift, Go, or any other toolchain a special orchestration concept.
- Provider runtime config isolation so daemon-launched agent sessions do not inherit the operator's personal MCP servers, plugins, skills, project trust, or feature flags.
- Automatic generated-state housekeeping for inactive run build outputs, stale ACP runtime homes, and git temp objects so repository growth does not become an indirect SQLite/provider latency failure.
- Contention observability: write wait, write duration, queue depth, active execution counts, and command latency.
- A focused `proposal-061` validation gate.

This proposal does not include:

- Migrating from SQLite to Postgres.
- Increasing SQLite pool size as the primary scale strategy.
- Unlimited local parallelism.
- P051's shared Xcode MCP bridge implementation.
- P060's dynamic agent selection semantics.
- Provider CLI bug fixes outside the control-plane scheduling boundary.
- Operator include/exclude reviewer overrides.

---

## 4. Target Capacity Model

Initial defaults:

| Capacity dimension | Default | Rationale |
|---|---:|---|
| Active runs | 5 stable, 10 bounded | Operators can keep several proposals moving, but backpressure is expected near 10. |
| Global active agent executions | 20 | Matches the desired reviewer-agent ceiling while still bounding local process and DB pressure. |
| Per-run active agent executions | 4 | Allows a single proposal review fan-out to progress, while preventing one run from consuming all slots. |
| Gemini active executions | 4 | Protects the observed fragile provider path and Xcode-adjacent handshakes. |
| Codex active executions | 3 | Keep Codex below the aggregate cap until ACP/MCP subprocess cleanup is proven under repeated `xcode`/`context7` sessions. |
| Claude active executions | 8 | Higher cap for review/audit lanes that are usually read-heavy. |
| Auggie active executions | 1 | Unknown maturity, keep isolated. |
| Junie active executions | 1 | Unknown maturity, keep isolated. |

All caps must be configurable. The defaults are product targets, not hard-coded constants.

When capacity is unavailable, the correct behavior is:

- leave the work item pending or move it to an explicit queued/backpressured state;
- record the backpressure reason;
- keep the run/stage active but not failed;
- publish enough projection data for UI, GraphQL, and MCP readback;
- wake or poll the executor when capacity is released.

Backpressure is a normal state, not an error.

---

## 5. Design

### 5.1 Capacity-aware executor

The background executor must acquire a capacity lease before claiming or starting an `InvokeAgent` item.

Capacity dimensions:

- global active agent execution count;
- provider active execution count;
- run active execution count;
- DB writer health/backlog signal.

The claim path must avoid the current "claim first, discover capacity later" pattern for `InvokeAgent`. A saturated executor should not mark work `running` merely because it found a pending row. Work should remain claimable only when a matching capacity lease is available.

The executor should also avoid taking SQLite's writer lock when a cheap read-only scan can prove that the current pending `InvokeAgent` candidates are all capacity-blocked. This precheck is allowed to be racy because the real claim/start transaction repeats the capacity check under `BEGIN IMMEDIATE` before mutating ownership. If capacity is released immediately after a negative precheck, the next executor wake/poll can pick the work up.

Capacity release is also a scheduler event. When an `InvokeAgent` item completes or fails, the completion path must durably enqueue or signal the next `AdvanceRun`/finalizer wake-up needed to settle the stage. A run must not rely on a best-effort in-memory wake after the last agent finishes; otherwise a stage can remain `running` with no active agents and no pending work until an operator inserts a manual catch-up item.

For non-agent work items such as `AdvanceRun`, `SettleStage`, `RebuildProjection`, and `StartupRepair`, the executor may keep inline handling, but these paths must go through the serialized write pathway when they mutate domain state.

### 5.2 Work item state and readback

Queued work needs an operator-visible reason. Acceptable reason values:

- `global_capacity`
- `provider_capacity`
- `run_capacity`
- `db_writer_capacity`
- `startup_recovery_backpressure`

GraphQL and MCP should expose:

- active execution counts by provider;
- pending/backpressured counts by provider and reason;
- run-level queued work summary;
- stage-level queued work summary where relevant;
- oldest queued age.

These fields are diagnostic state, not an invitation for clients to mutate scheduler internals.

### 5.3 Serialized DB write pathway

SQLite allows concurrent readers but one writer. The control plane should embrace that model instead of trying to out-parallelize it with more connections.

Introduce a narrow write coordination path for multi-row mutations that must stay ordered:

- `StartRun`
- `ApproveStage`
- `RejectStage`
- `RetryStage`
- `CancelRun`
- `ResetSession`
- work item claim/start/complete/fail
- stage settlement
- artifact import and projection rebuild
- startup repair

The implementation can be a `WriteCoordinator`, repository-level transaction helper, or equivalent engine-owned abstraction. The important contract is:

1. Multi-row invariants run in one transaction.
2. Claim/start paths use `BEGIN IMMEDIATE` or equivalent write-lock acquisition before reading mutable ownership state.
3. Provider I/O, filesystem scans, and ACP process waits never happen inside DB transactions.
4. Projection rebuild that represents the same domain mutation runs in the same write unit when the user-visible truth depends on it.
5. `SQLITE_BUSY` from short writes is classified and retried with bounded backoff before surfacing to the operator.

### 5.4 SQLite configuration

Keep SQLite as the target persistence layer.

Update the pool defaults:

- keep WAL mode;
- keep a small connection pool;
- increase busy timeout from 5 seconds to 30 seconds;
- add query/write timing instrumentation;
- do not use pool expansion as the primary concurrency fix.

The longer busy timeout is not a substitute for write serialization. It is a safety margin for legitimate short contention.

### 5.5 Retry and stale execution cleanup

Retry must not create a new stage attempt while stale agent executions from the superseded attempt remain active.

The retry transaction must:

1. mark the old stage attempt skipped/superseded;
2. close or supersede active agent executions for that attempt;
3. supersede pending/running work items owned by the old attempt;
4. create the new stage attempt;
5. enqueue the new `AdvanceRun` or `InvokeAgent` path;
6. rebuild projections that drive UI and MCP readback.

If the daemon crashes between any of these steps, startup recovery must converge to one visible truth: no terminal/skipped stage owns a `running` agent execution.

### 5.6 Recovery without thundering herd

Startup recovery should repair stale state and enqueue necessary follow-up work, but it must not immediately spawn all recovered `InvokeAgent` items.

Recovered work goes through the same capacity gate as ordinary work. If the daemon restarts with many active runs, the visible state should become "queued because capacity is full", not a storm of provider handshakes.

### 5.7 Host suspend and network migration recovery

Laptop sleep/wake and Wi-Fi migration are local host interruptions, not normal provider failures. The daemon should maintain a lightweight runtime heartbeat with both monotonic and wall-clock timestamps. If the wall-clock gap exceeds the expected heartbeat interval by a configured threshold, or if a platform observer reports sleep/wake or network path migration, the daemon records a `host_interruption_epoch`.

Any ACP execution that was `running` across that epoch should be settled as retryable host interruption unless it already produced valid promotable outputs. The settlement should:

- close the ACP session and signal the provider process group;
- classify runtime facts with a distinct host-interruption reason instead of plain `provider_timeout`;
- leave no active artifact generation claim owned by the interrupted execution;
- enqueue retry work with jitter/backoff through the same global, provider, and per-run caps;
- avoid consuming quota retry budget for failures attributed to host sleep/network migration;
- expose the interruption epoch, affected run count, and retry/backoff decision through GraphQL, MCP, and operator diagnostics.

Automatic retry should be bounded. If too many executions are interrupted at once, the daemon should stage retries in small batches rather than recreating the original handshake storm immediately after wake.

### 5.8 Provider Toolchain Home

Daemon/provider launch must expose a language-neutral writable toolchain root instead of treating Rust as part of the orchestration contract.

The canonical root is:

- `CHAINWORKS_TOOLCHAIN_HOME`, controlled by the daemon/operator config.
- `TOOLCHAIN_HOME`, exported as a generic compatibility alias for provider processes and agent instructions.

Provider adapters map that root into concrete tool-specific environment variables only when the launched tools require them. Examples:

- Rust: `RUSTUP_HOME`, `CARGO_HOME`, and `CARGO_TARGET_DIR`.
- Swift/Xcode: DerivedData, module cache, SDK stat cache, and package cache paths when the invocation supports explicit paths.
- Go: `GOCACHE`, `GOMODCACHE`, and related build cache variables.
- Other languages: provider-specific cache/build directories under the same root, without adding language-specific orchestration semantics.

The daemon must not encode "Rust-heavy" as a scheduler or proposal concept. The scheduler can know that a work item needs writable toolchain/cache space, but language-specific names belong at the provider adapter boundary.

Required behavior:

1. Every daemon-launched provider session gets a writable `TOOLCHAIN_HOME` outside isolated runtime homes that may be read-only under provider sandboxing.
2. Language-specific env vars are derived from `TOOLCHAIN_HOME` and scoped to the provider/session or another explicitly configured isolation level.
3. Shared build outputs that can cause cross-run lock contention, such as Cargo `target`, must not default to one repo-global directory for all active runs.
4. Provider prompts may refer to `TOOLCHAIN_HOME` as the durable writable toolchain/cache root; they should not need to know whether the current task is Rust, Swift, Go, or another stack.
5. Provider adapters should avoid forcing toolchain selectors such as `+stable` when the executable already resolves on `PATH`, unless a workflow explicitly requests that selector.

### 5.9 Provider Runtime Config Isolation

Daemon-launched ACP providers must not inherit the operator's personal Codex/Claude/Gemini config as executable runtime policy. Copying credentials is acceptable; copying tool surfaces is not.

For Codex specifically, the isolated `CODEX_HOME` config copy must be sanitized as a deny-by-default runtime config:

- keep only inert notices or other explicitly allowlisted client preferences;
- strip model, effort, sandbox, personality, and project trust settings because the engine/session config owns those choices;
- strip all inherited `mcp_servers` entries, including disabled entries, so an operator-level `xcode`, Slack, Backstage, or control-plane MCP server cannot appear in an agent session by accident;
- strip all inherited `plugins` entries because plugins can implicitly add MCP servers, skills, tools, or approval prompts outside the workflow contract;
- strip inherited `features`, especially multi-agent toggles, unless the engine explicitly enables the feature for that invocation;
- strip inherited `skills`; workflow skills must be resolved from the Chainworks catalog/run snapshot, not from the operator's personal Codex skill registry.

This does not mean Chainworks should pass every skill as prompt text forever. The target model is:

1. The engine resolves required skills from the workflow/agent catalog.
2. The resolved skill bundle is frozen into run truth, with content hash and source metadata.
3. Provider adapters may materialize those frozen skills into a managed runtime config block or managed files under the isolated runtime home.
4. The provider session receives only that managed skill set, never the user's global skill/plugin config.

MCP access follows the same rule. Agent tasks receive MCP servers only through `ExecutionRequest.mcp_servers` after engine resolution, capability checks, and provider-specific transport shaping. Personal `~/.codex/config.toml` MCP entries are not a fallback path.

This isolation is the higher-leverage fix for the observed Xcode MCP approval storms: a Rust/control-plane task should not even see an operator-level Xcode MCP entry unless the Chainworks workflow asked for it. Task-type MCP profiles and healthchecks remain useful after this isolation boundary exists, but they are not sufficient while inherited config can still reintroduce tools behind the scheduler's back.

### 5.10 Generated-State Housekeeping

The daemon should periodically prune rebuildable generated state that otherwise makes local dogfood runs slower and harder to diagnose. This is part of the executor stability envelope, not a correctness dependency for P053 discovery.

The housekeeping loop is enabled by default and configurable through daemon environment:

- `CHAINWORKS_GENERATED_STATE_HOUSEKEEPING=0|false|off|no` disables it.
- `CHAINWORKS_GENERATED_STATE_HOUSEKEEPING_INTERVAL_SECS` controls cadence.
- `CHAINWORKS_GENERATED_STATE_HOUSEKEEPING_MIN_AGE_SECS` controls the minimum age before deletion.

Allowed deletions:

- `control-plane/target` and top-level `target` directories inside managed `.chainworks/worktrees/<run>` directories, but only when the owning run is terminal: `completed`, `failed`, or `cancelled`.
- stale `.forge-codex-acp/<runtime>` homes under known workspace roots, but only when the directory is older than the configured age and no live process command line references it.
- stale `.git/objects/tmp_obj_*` files under known workspace roots.

Explicitly forbidden deletions:

- worktree directories themselves;
- source files;
- `.git` worktree metadata except temporary `tmp_obj_*` garbage files;
- `.chainworks/runs` artifacts;
- current SQLite DB, WAL/SHM, or DB backups;
- generated build outputs for `running`, `blocked`, `pending`, or `cancelling` runs.

The loop emits structured logs with removed directory/file counts and reclaimed bytes. Failures are warnings and must not stop executor scheduling.

### 5.11 Retry And Projection Repair

Targeted retry is an optimization, not the durability boundary. If the source ACP generation for a targeted retry is gone, the engine must fall back to an explicit stage retry that creates a fresh invocation. The fallback must preserve durable worktree/artifact truth, supersede old active claims through the same retry transaction, and record a machine-readable retry reason such as `operator_retry_stale_targeted_retry`.

Executor-driven state mutations must also keep canonical tables and read projections in parity. Any work item that can change `runs.status`, stage status, agent execution status, or active artifact truth must rebuild the affected run projections before the work item is marked complete. At minimum this includes:

- `AdvanceRun`;
- `TriggerNextStage`;
- `SettleStage`;
- `InvokeAgent` completion/error paths;
- artifact import/contract settlement paths;
- startup repair paths that rewrite stale running state.

This closes the observed class where `runs.status = blocked` while `run_state_projection.status = running`, causing UI/MCP/GraphQL readback to disagree about whether a run is still doing work.

### 5.12 Observability

Add structured logs or metrics for:

- DB write wait time;
- DB write transaction duration;
- `SQLITE_BUSY` retry count;
- work queue depth by kind/provider/reason;
- active execution count by provider;
- command handler latency by command type;
- stale execution repair count;
- generated-state housekeeping reclaimed bytes and skipped active directories;
- provider `TOOLCHAIN_HOME` root, isolation scope, and cache-dir creation failures;
- provider runtime config sanitizer decisions, including stripped MCP/plugin/skill tables by category, without logging secrets;
- host interruption epoch count and affected execution count;
- provider session start latency and failure count.

The runtime health surface should make it obvious whether the bottleneck is:

- provider capacity;
- DB writer contention;
- a stuck provider process;
- a single run monopolizing capacity;
- startup recovery backlog.

---

## 6. Acceptance Criteria

1. With 5 active proposal runs, the daemon completes ordinary approve/retry/start/read flows without `database is locked` surfacing to GraphQL/MCP clients.
2. With 10 active runs and the default global cap, surplus `InvokeAgent` work remains pending/backpressured instead of failing or launching immediately.
3. A provider-cap test proves that no more than 4 Gemini executions can be active at once with the default config.
4. A per-run-cap test proves that one fan-out stage cannot consume all global execution slots.
5. `RetryStage` under active load returns a completed or domain-rejected command result without timing out on DB contention.
6. Retrying a running stage leaves no stale `running` `agent_executions` for the skipped/superseded attempt.
7. Startup recovery with many requeued items respects the same caps and exposes queued/backpressure readback.
8. Work item claim/start crash recovery cannot duplicate an agent execution for the same ownership boundary.
9. GraphQL and MCP readback both expose queued/backpressured summaries.
10. A simulated host sleep/wake or network-migration event marks affected running ACP executions retryable as host interruptions, cleans provider process groups, and requeues retries with jitter under the same capacity caps.
11. Host-interruption retry does not consume provider quota budget and does not promote late/partial outputs unless existing output-settlement rules allow them.
12. DB contention instrumentation is visible in runtime health logs or projections.
13. Provider launch exports writable `CHAINWORKS_TOOLCHAIN_HOME`/`TOOLCHAIN_HOME`, maps language-specific cache/build env vars from that root at the adapter boundary, and does not require Rust-specific assumptions for Swift, Go, or future stacks.
14. Codex runtime config sanitization strips inherited MCP servers, plugins, features, project trust, and personal skills; any task skills or MCP servers visible to the provider come only from frozen workflow/run truth and `ExecutionRequest` resolution.
15. Generated-state housekeeping removes only inactive run target directories and stale unreferenced ACP runtime homes, never active run outputs, worktrees, source files, run artifacts, or database files.
16. Targeted retry against a dead source ACP generation automatically becomes an explicit stage retry with a fresh invocation instead of failing or reusing stale session identity.
17. Executor-driven `AdvanceRun`, `TriggerNextStage`, and `SettleStage` mutations keep `runs.status` and `run_state_projection.status` in parity before the work item completes.

---

## 7. Implementation Outline

1. Add scheduler capacity config to domain/engine configuration.
2. Add capacity lease tracking in the executor, keyed by global/provider/run.
3. Make `InvokeAgent` claim capacity-aware.
4. Add backpressure reason storage or derive it from scheduler state for projections.
5. Introduce a serialized DB write helper for multi-row mutations.
6. Move command-handler and executor mutations onto the write helper.
7. Increase SQLite busy timeout and add bounded retry for short busy failures.
8. Tighten retry transaction semantics around old stage, agent executions, work items, new stage, and projection rebuild.
9. Update recovery to enqueue repaired work without bypassing capacity.
10. Add host suspend/network-interruption detection, runtime-fact classification, and staggered retry.
11. Add daemon/provider launch env construction for `CHAINWORKS_TOOLCHAIN_HOME`/`TOOLCHAIN_HOME` plus adapter-local mappings for Rust, Swift/Xcode, Go, and future toolchains.
12. Harden Codex runtime config copying into an allowlisted/sanitized config, and add a follow-up managed skill materialization path from frozen workflow/run truth.
13. Add generated-state housekeeping loop with conservative deletion allowlist, age threshold, and live-process guard for ACP homes.
14. Add stale targeted-retry fallback to explicit stage retry and regression coverage for dead ACP generations.
15. Rebuild run projections after executor-driven status mutations, with regression coverage for blocked/running parity.
16. Add GraphQL/MCP fields for queue depth, active execution counts, interruption epochs, and backpressure reasons.
17. Add the `proposal-061` test gate.

---

## 8. Test Plan

Add `./scripts/test-gate.sh proposal-061`.

The gate should include:

- Rust unit tests for capacity lease accounting.
- Repository tests for capacity-aware claim behavior.
- Command-handler tests for retry under queued/running work.
- Recovery tests for stale running agent execution cleanup.
- Integration tests that start 5 simulated runs and assert no DB lock escapes.
- Fake-clock/fake-platform tests for sleep/wake or network migration while ACP sessions are running.
- Integration tests that start 10 simulated runs and assert bounded active execution count.
- Integration tests for stale targeted retry falling back to explicit stage retry when the source ACP generation is dead.
- Integration tests proving executor-driven `AdvanceRun`/`TriggerNextStage`/`SettleStage` paths refresh `run_state_projection.status`.
- Integration tests that simulate 20 active reviewer agents and prove operator commands remain responsive.
- Provider environment tests that prove `TOOLCHAIN_HOME` is writable and language-specific cache variables are derived from it without placing generated outputs under read-only runtime homes.
- Codex runtime config sanitizer tests proving inherited MCP servers, plugins, features, project trust, and personal skills are stripped, while engine-managed skill/MCP inputs still reach the provider through explicit request/session paths.
- Housekeeping tests that prove active/blocked run targets are preserved while terminal-run generated targets and unreferenced stale ACP homes are pruned.
- GraphQL/MCP parity tests for backpressure readback.

Provider processes should be faked in the gate. The proposal gate should not require real Gemini, Codex, Claude, Xcode, or external network calls.

---

## 9. Risks and Tradeoffs

**Risk: Lower raw parallelism.**  
Backpressure intentionally reduces immediate fan-out. This is the right tradeoff for local reliability because queued work is better than failed or duplicate work.

**Risk: Hidden starvation.**  
Provider and per-run caps can starve a run if scheduling is purely FIFO. The executor needs fair selection across runs and providers.

**Risk: Over-centralized write path.**  
A write coordinator can become a broad abstraction if it absorbs too much business logic. Keep it focused on transaction boundaries, ordering, and instrumentation; domain semantics stay in command/orchestrator code.

**Risk: Misleading active run count.**  
Operators may read "10 active runs" as "10 agents are working now." UI copy and projections need to distinguish active runs from active executions.

---

## 10. Open Questions

1. Should queued/backpressured work use a separate `work_items.status` value or remain `pending` with a derived reason?
2. Should provider caps live in `agents.yaml`, daemon config, or both?
3. Should the UI expose a manual "pause scheduling for this run" action in a later proposal?
4. Should DB writer health become a hard capacity dimension immediately, or start as observability and only gate when busy retries exceed a threshold?

---

## 11. Non-Goals Reaffirmed

P061 does not weaken the local-first target. SQLite remains the persistence target for this phase. The goal is to make the current architecture honest and durable at 5-10 active runs and up to 20 active reviewer agents, not to introduce a remote database or a distributed scheduler before the product needs it.

---

## 12. Remaining Proposal-Level Work

The quick implementation path can add conservative caps, add indexes, avoid unnecessary writer locks, shorten/log hot write transactions, make `InvokeAgent` completion enqueue an idempotent post-completion `AdvanceRun` wake-up, and harden ACP subprocess cleanup after prompt transport errors. The following work remains proposal-level and should not be hidden as "already solved":

1. Replace tight polling with an event-driven scheduler wakeup path so idle saturated loops do not repeatedly scan the same blocked queue.
2. Add durable queued/backpressured readback for GraphQL, MCP, and the operator UI, including provider/run/global reason summaries.
3. Add a real capacity lease or scheduler snapshot model if simple count-based caps show fairness or race issues under 20 active agents.
4. Add a `proposal-061` gate that proves 5 active runs, 10 queued/active runs, and 20 active reviewer agents with responsive retry/approve/cancel commands.
5. Add command-latency and write-wait dashboards or runtime health readback so the operator can distinguish DB writer contention from provider capacity.
6. Add host suspend/network-interruption recovery with bounded automatic retry and explicit readback.
7. Revisit provider caps from evidence after P051/P060 settle; especially Gemini/Xcode-backed paths should stay conservative until the bridge pool is proven.
