# Workflow Execution Engine

## Purpose

The Workflow Execution Engine compiles YAML workflow definitions and agent catalogs
into executable run plans, then drives them through a state machine to completion.
It handles sequential and parallel agent execution, human approval gates, bounded
loops, artifact persistence, transition evaluation, and safe resume after app
interruption. All engine code lives under `Chainworks Forge/Engine/`.

---

## Components

### RunPlan (`RunPlan.swift`)

An immutable, `Sendable` value type representing a compiled execution topology.
A RunPlan carries no run-scoped identity -- identity lives on the `Run` SwiftData
model. The orchestrator receives a `(Run, RunPlan, RunWorkspace)` tuple.

Contents:

- **states** -- `[String: ExecutableState]` resolved state machine.
- **initialStateID** -- entry point for execution.
- **agentBindings** -- `[String: ResolvedAgent]` with backend profiles resolved to
  concrete provider/model/effort/maxTurns/temperature values.
- **variables** -- frozen at compile time from the workflow definition.
- **scoring / failurePolicy** -- optional workflow-level configuration.
- **provenance** -- SHA-256 hashes and JSON snapshots of the source workflow and
  catalog, enabling drift detection on resume.
- **planCompilerVersion** -- monotonic integer (currently `1`); persisted on `Run`
  so the resume path can reject version mismatches.

Supporting types: `ExecutableState`, `ExecutableRunBlock`, `ExecutionPhase`
(`.sequential` / `.parallel`), `ExecutableTransition`, `TransitionCondition`
(`.always`, `.artifactExists`, `.approvalGranted`, `.expression`),
`ResolvedLoopConfig`, `ResolvedAgent`.

### RunPlan Compiler (`RunPlanCompiler.swift`)

A `@MainActor` two-phase compiler (ARCH-021).

**Phase 1 -- `previewCompile`** (no persistence, safe to cancel):

1. Validate via `YAMLValidator.validateAll`.
2. Resolve agent references against the catalog; resolve backend profiles.
3. Parse transition `when` clauses into `TransitionCondition` variants.
4. Resolve loop budgets -- `vars.*` references substituted at compile time.
5. Build `ExecutableState` instances with run blocks, transitions, and loops.
6. Compute provenance hashes via `DefinitionHasher`.
7. Assemble and return an in-memory `RunPlan`.

A `previewCompileCompact` variant normalizes compact workflow definitions through
`CompactNormalizer` before delegating to `previewCompile`.

**Phase 2 -- `createRun`** (irreversible):

1. Generate a run UUID.
2. Provision a workspace directory under Application Support
   (`~/Library/Application Support/Chainworks Forge/runs/{runID}/`).
3. Persist a `Run` record via `RunRepository`.

**Resume path -- `rebuildPlanFromSnapshot`**: Decodes frozen JSON snapshots stored
on a `Run` record and re-runs `previewCompile`. Rejects compiler version mismatches.

### Workflow Orchestrator (`WorkflowOrchestrator.swift`)

A `@MainActor @Observable` per-run state machine driver. Design invariants:

- `StageExecution` and `AgentExecution` records are created lazily when entering a
  state (ARCH-027), not upfront.
- `Run.currentStageID` stays derived from the orchestrator's `currentStateID`.
- Agent execution runs off-MainActor; results are marshalled back.

**State machine loop** (`executeStateMachine`):

1. Check for end state -- if reached, mark run completed.
2. Execute the current state (create `StageExecution`, run the run block).
3. If the state requires approval, pause and publish an `ApprovalRequest` via callback.
4. Handle loops: increment counter, check budget, update runtime variables.
5. Evaluate transitions via `TransitionEvaluator` to determine the next state.
6. Advance `currentStateID` and repeat.

**Run block execution**: Phases execute in declaration order. `.sequential` tasks
run one-by-one; `.parallel` tasks run via `withTaskGroup`. Each task creates an
`AgentExecution`, gathers input artifacts, builds an `ExecutionContext`, calls the
executor, then persists outputs through `ArtifactManager`.

**Approval flow**: When a state has `approval: required`, the orchestrator pauses,
creates an `Approval` record, and publishes an `ApprovalRequest`. On resolution:
granted resumes execution (including any `runAfterApproval` block); rejected cancels
the run.

**Cancellation**: Sets `isCancelled`, updates run status, stops the loop.

### Agent Executor Protocol (`AgentExecutor.swift`)

```swift
protocol AgentExecutor: Sendable {
    func execute(task: AgentTask, agent: ResolvedAgent,
                 context: ExecutionContext) async throws -> AgentResult
}
```

Executors return `[String: Data]` (in-memory), never file URLs. The
`ArtifactManager` is the sole disk writer (ARCH-030).

- **`ExecutionContext`** -- workspace, stageID, iteration, attempt number, input
  artifacts, runtime variables, idea body.
- **`AgentResult`** -- output data map, log snippet, cost estimate, success flag,
  session ID, wall-clock duration.
- **`OutputContractResolver`** -- resolves expected outputs and contract IDs from
  task/agent/catalog definitions.

#### SimulatedAgentExecutor (`SimulatedAgentExecutor.swift`)

Deterministic mock. Generates structurally valid outputs via
`OutputContractTemplates`. Supports injectable failures (`failingAgentIDs`) and
configurable delay. Thread-safe task tracking for test assertions.

#### GooseAgentExecutor (`GooseAgentExecutor.swift`)

Live executor using a Goose backend. Accepts `any GooseTransportProtocol` (bespoke
or `GooseServerTransport`). Per-execution flow:

1. Validate workspace boundaries.
2. Create an isolated session via `GooseSessionBridge`.
3. Stream execution events through `ExecutionEventBridge`.
4. Build receipt and transcript artifacts (`ExecutionReceiptBuilder`).
5. Read declared output files from the workspace artifact directory.
6. Validate required outputs -- missing outputs fail the stage.

On stream failure, the executor salvages any files the agent already wrote to disk
before the SSE connection dropped.

### Artifact Manager (`ArtifactManager.swift`)

`@MainActor` bridge between nonisolated disk I/O (`ArtifactStorage`) and SwiftData
metadata (ARCH-023).

- **`persistOutputs`** -- writes each output via `ArtifactStorage.write`, determines
  format from the catalog contract (`ArtifactFormat.detect`), creates `Artifact`
  SwiftData records with checksum, size, and contract metadata.
- **`readArtifact`** -- reads data from disk with path boundary validation.
- **`producedArtifactNames`** -- returns the set of artifact names for a run
  (consumed by `TransitionEvaluator`).
- **`persistSystemArtifact`** -- for artifacts not attached to a specific agent
  execution (e.g., reports).

### Artifact Storage (`ArtifactStorage.swift`)

Nonisolated, `Sendable` disk I/O layer.

- Path layout: `{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}`
- Path traversal guard: rejects any resolved path outside `workspaceRoot`.
- Atomic writes with SHA-256 checksums.

### Transition Evaluator (`TransitionEvaluator.swift`)

Stateless evaluator for transition `when` clauses (ARCH-031). Supports only
canonical patterns:

| Pattern | Example |
|---|---|
| Always | `when: 'true'` |
| Artifact exists | `when: exists('proposal_review_summary')` |
| Approval granted | `when: approval.granted == true` |
| Comparison | `when: review.score >= vars.min_score` |
| Connectives | `expr and expr`, `expr or expr` |

Value resolution supports `vars.*` (runtime variables), `artifact.field` (artifact
metadata), and literals (int, double, bool, quoted string). Comparison operators:
`==`, `>`, `>=`. Unrecognized expressions fail closed (return false).

### Resume Manager (`ResumeManager.swift`)

Classifies interrupted runs at app launch (ARCH-029). Three outcomes per run:

- **`.resume`** -- plan rebuilt successfully, no drift, no mid-side-effect
  interruption. Safe to restart.
- **`.needsDecision`** -- drift detected (workflow/catalog source hash mismatch) or
  interrupted during a side-effect stage. Requires user intervention.
- **`.cannotResume`** -- compiler version mismatch or snapshot corruption. Marked
  failed.

Side-effect detection uses permission profiles (`RELEASE_GIT`, `RELEASE_PUBLISH`),
the `requiresHumanApproval` flag, and stage-name heuristics (commit, push, release,
publish, deploy).

### Execution Service (`ExecutionService.swift`)

App-scoped `@MainActor @Observable` singleton (ARCH-022). Manages the collection
of active orchestrators (ARCH-028 -- not a per-run singleton).

Responsibilities:

- **Start run** -- creates an orchestrator, wires approval and completion callbacks,
  selects the appropriate executor (simulated vs. live).
- **Resume interrupted runs** -- delegates to `ResumeManager`, then starts
  orchestrators for resumable runs; marks others blocked or failed.
- **Approval resolution** -- routes approval decisions to the correct orchestrator.
- **Cancellation** -- cancels the orchestrator and cleans up state.
- **Executor selection** -- for live workflows (`proposal_loop_live`), selects a
  `GooseAgentExecutor` with the configured transport (bespoke `GooseTransport` or
  `GooseServerTransport`) and optional provider/model override.
- **Post-run hooks** -- triggers Steward analysis and emits run reports on completion.

---

## Architectural Invariants

| ID | Invariant |
|---|---|
| ARCH-021 | Two-phase compilation: `previewCompile` is side-effect free; `createRun` is irreversible. |
| ARCH-022 | `ExecutionService` is app-scoped `@Observable`, injected via SwiftUI environment. |
| ARCH-023 | `ArtifactStorage` handles disk I/O (nonisolated); `ArtifactManager` handles SwiftData metadata (`@MainActor`). |
| ARCH-024 | `RunPlan` carries no run-scoped identity. Identity lives on `Run`. |
| ARCH-025 | `RunWorkspace` defines the isolation boundary. All artifact paths must resolve within it. |
| ARCH-026 | Artifact root is `{workspaceRoot}/artifacts/` -- already run-scoped, no extra nesting. |
| ARCH-027 | `StageExecution` and `AgentExecution` records are created lazily on state entry, not upfront. |
| ARCH-028 | `ExecutionService` maintains a collection of orchestrators, not a singleton. |
| ARCH-029 | Resume safety: compiler version check, drift detection, side-effect stage detection. |
| ARCH-030 | Executors return `[String: Data]`. `ArtifactManager` is the sole disk writer. |
| ARCH-031 | Transition conditions use only the canonical pattern set; unrecognized expressions fail closed. |

---

## Data Flow

```
  WorkflowDefinition + AgentCatalog
               |
               v
      ┌─────────────────┐
      │ RunPlanCompiler  │  previewCompile (Phase 1: validate, resolve, assemble)
      │                  │  createRun      (Phase 2: persist Run + workspace)
      └────────┬────────┘
               │ (Run, RunPlan, RunWorkspace)
               v
      ┌─────────────────┐
      │ExecutionService  │  selects executor, creates orchestrator
      └────────┬────────┘
               │
               v
      ┌─────────────────────────────────────────┐
      │       WorkflowOrchestrator              │
      │                                         │
      │  State Machine Loop                     │
      │  ┌───────────┐   ┌───────────────────┐  │
      │  │ Execute   │──>│TransitionEvaluator │  │
      │  │ State     │   │ (evaluate when)    │  │
      │  └─────┬─────┘   └───────────────────┘  │
      │        │                                 │
      │        v                                 │
      │  ┌─────────────┐                         │
      │  │AgentExecutor│  (Simulated or Goose)   │
      │  └─────┬───────┘                         │
      │        │ AgentResult ([String: Data])     │
      │        v                                 │
      │  ┌──────────────┐  ┌────────────────┐    │
      │  │ArtifactManager│─>│ArtifactStorage │    │
      │  │ (SwiftData)  │  │ (disk I/O)     │    │
      │  └──────────────┘  └────────────────┘    │
      └─────────────────────────────────────────┘
               │
               v
         Run.status = .completed / .failed / .cancelled
```

---

## Source File Index

| File | Role |
|---|---|
| `Engine/RunPlan.swift` | Immutable compiled plan, supporting value types |
| `Engine/RunPlanCompiler.swift` | Two-phase compiler (preview + persist) |
| `Engine/WorkflowOrchestrator.swift` | Per-run state machine driver |
| `Engine/AgentExecutor.swift` | Executor protocol, ExecutionContext, AgentResult |
| `Engine/SimulatedAgentExecutor.swift` | Deterministic mock executor |
| `Engine/GooseAgentExecutor.swift` | Live executor via Goose backend |
| `Engine/ArtifactManager.swift` | SwiftData metadata bridge for artifacts |
| `Engine/ArtifactStorage.swift` | Nonisolated disk I/O with path guards |
| `Engine/TransitionEvaluator.swift` | Stateless transition condition evaluator |
| `Engine/ResumeManager.swift` | Interrupted run classification for safe resume |
| `Engine/ExecutionService.swift` | App-scoped orchestrator and approval manager |
| `Models/Artifact.swift` | SwiftData Artifact model and format detection |
