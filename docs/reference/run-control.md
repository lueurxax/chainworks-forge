# Run Control

Stable reference for run-stop semantics, two-phase cancellation settlement, and operator-visible terminal truth.

## Purpose

Run control must be operationally trustworthy.

The operator must be able to:

- stop active work without guessing what happens next,
- distinguish `cancelling` from settled `cancelled`,
- trust that in-flight agent work was actually asked to stop,
- see terminal history without confusing stop with archive.

## Scope

This reference covers:

- stop vs archive semantics,
- two-phase cancellation settlement,
- operator-visible cancellation state,
- persisted cancellation evidence,
- northbound reader split for settlement data,
- terminal-history rules for cancelled runs.

It does not define repo-backed release approval or delivery execution — that boundary lives in [release-gate.md](release-gate.md).

## Core rule

`Stop` and `Archive` are different actions.

- `Stop` is execution control.
- `Archive` is visibility control.

Archive never implies stop, and stop never implies archive.

An active idea must first settle its run into a terminal state before archive becomes eligible.

### External Command Surface (P031-r18)

Per P031-r18, the macOS UI is a **read-only thin client**. Run control actions — `CancelRun`, `StartRun`, `RetryStage` — are **prohibited** within the governed macOS UI. 

Operators must use external workflows (CLI, MCP tools, or automation) to issue these commands. The macOS UI renders the resulting state transitions (`cancelling`, `cancelled`) from GraphQL projections but provides no in-app write affordances.

## Stop semantics

Stopping an active idea means:

- the active run stops advancing its state machine,
- in-flight agent executions receive cooperative cancellation,
- active runtime sessions are closed where available,
- the run remains visibly `cancelling` until settlement is confirmed,
- only then does the run become terminal `cancelled`.

`ExecutionService` and `RunCancellationCoordinator` own this path in the Swift app. In the Rust control-plane, the equivalent is `cancellation::begin_settlement` → async session close → `cancellation::finalize_settlement`.

## Two-phase cancellation settlement

Cancellation settles in two phases with durable evidence so operator and report readers never see half-finished state.

### Phase 1 — synchronous on `CancelRun`

`begin_settlement` runs synchronously when the operator requests stop:

1. `run.cancellation_requested_at` is set,
2. all active agent executions (`Running`/`Pending`/`Ready`) transition to terminal `Cancelled`,
3. a `CancellationSettlementEntry` is built per agent execution with `session_close_succeeded: None`,
4. entries are serialized as JSON into `run.cancellation_settlement_log`,
5. all `Running` work items are marked `Cancelled` (`WorkItemStatus::Cancelled`),
6. all `Running` stages with terminal agent executions are marked `Failed`,
7. `run.status` stays `Cancelling`.

### Async session close

After Phase 1 completes, a background task closes live ACP sessions through `AcpRuntimeManager`:

- per session: SIGTERM to the ACP subprocess, wait up to 10s timeout, SIGKILL if needed,
- returns `SessionCloseOutcome { session_id, attempted, succeeded }` per session.

### Phase 2 — `finalize_settlement`

Once async close completes:

1. read preliminary settlement entries from `cancellation_settlement_log`,
2. update each entry with actual `session_close_succeeded` from close outcomes,
3. re-serialize entries to `run.cancellation_settlement_log`,
4. `run.cancellation_settled_at` is set,
5. `run.status` transitions to `Cancelled`.

Only after Phase 2 does the run appear as `Cancelled` to operator and report readers.

### Settlement evidence

`CancellationSettlementEntry` carries:

- `agent_execution_id`,
- `agent_id`,
- `prior_status`,
- `terminal_status` (`"cancelled"`),
- `session_close_attempted`,
- `session_close_succeeded` (`None` in Phase 1, `Some` in Phase 2),
- `settled_at`.

Persisted run-level fields:

- `cancellation_requested_at`
- `cancellation_settled_at`
- `cancellation_settlement_log` — full JSON array of entries

Cancellation is considered settled only when all of the following are true:

1. the orchestrator has stopped advancing workflow state,
2. every agent execution that was running at request time is now terminal,
3. every open runtime session has a recorded close outcome,
4. the run has both request and settlement timestamps plus structured settlement evidence.

## Northbound reader split

Settlement truth is exposed at two different granularities depending on the reader's intent.

**Single-run readers** (GraphQL `run(id)`, MCP `runs.get`) read the canonical `Run` and expose:

- full `cancellation_settlement_log` JSON,
- `cancellation_settled_at`.

**List readers** (GraphQL `runs`, MCP `runs.list`) read `RunProjectionRow` and expose only:

- `cancellation_settlement_summary` — a human-readable one-line string derived during projection rebuild (e.g., `"3/3 agents settled, 2 sessions closed"`).

The full JSON log is not projected into list rows. List consumers get a summary; drilldown requires a single-run fetch.

## Operator-visible truth

Run surfaces must distinguish:

- `running`
- `cancelling`
- `cancelled`
- `failed`
- `blocked`

A run with `cancellation_requested_at != nil` and `cancellation_settled_at == nil` is not allowed to present as ordinary terminal `cancelled`.

The operator path for stopping a run (P031 external workflow):

1. open idea in the macOS UI,
2. copy `run_id` or `diagnosticId` from the diagnostic banner,
3. execute `CancelRun` via CLI or MCP tool,
4. observe `cancelling` state in the macOS UI (via GraphQL projection),
5. later observe settled `cancelled` history.

The confirmation surface must explicitly say that:

- artifacts remain intact,
- reports remain intact,
- receipts remain intact,
- history is preserved.

## Terminal history

Cancelled runs remain first-class history.

Rules:

- cancelled runs stay visible in run-centric surfaces,
- archive eligibility remains separate,
- reports and artifacts are not rewritten after cancellation,
- recovery/archive actions must not erase cancellation truth.

## Implementation Surface

| File | Role |
|---|---|
| `control-plane/crates/engine/src/cancellation.rs` | `begin_settlement()`, `close_runtime_sessions()`, `finalize_settlement()` |
| `control-plane/crates/engine/src/command_handler.rs` | `CancelRun` phase 1 entry |
| `control-plane/crates/db/src/repos/work_items.rs` | `WorkItemStatus::Cancelled` variant and cancellation cleanup |
| `control-plane/crates/db/src/repos/runs.rs` | Settlement log and settled-at persistence |
| `control-plane/crates/db/src/repos/projections.rs` | `cancellation_settlement_summary` projection |
| `control-plane/crates/domain/src/run.rs` | `cancellation_settlement_log` on `Run` |
| `control-plane/crates/graphql-server/src/schema.rs` | Single-run vs list reader routing |
| `control-plane/crates/graphql-server/src/types/run.rs` | `cancellation_settlement_log` and `cancellation_settlement_summary` on `GqlRun` |
| `control-plane/crates/mcp-server/src/tools/runs.rs` | `runs.get` full log, `runs.list` summary only |

**Ownership:**
- **Read-plane truth:** GraphQL projections via `GqlRun`.
- **Governed UI (Read-only):** P031-owned SwiftUI views.
- **Write-path:** External MCP/CLI (Governed UI has no write ownership).

## Related Docs

- [idea-lifecycle.md](idea-lifecycle.md) — archive eligibility
- [operator-experience.md](operator-experience.md) — run lists, reports, recovery surfaces
- [project-workspace-contract.md](project-workspace-contract.md) — fail-closed project-backed execution
- [provider-binding-truth.md](provider-binding-truth.md) — truthful historical run explanation
- [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md) — session ownership and live handle lifecycle
- [runtime-contract.md](runtime-contract.md) — run snapshots and artifact boundaries
- [release-gate.md](release-gate.md) — manual release approval and delivery execution
