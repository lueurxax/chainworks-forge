# Proposal 002: Workflow Execution Engine — RunPlan Compiler, Orchestrator, and Approval Flow

| Field | Value |
|---|---|
| Date | 2026-03-22 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Proposal 001 (Foundation Domain Model + YAML DSL Parser) |
| Prerequisite gate | Proposal 001 gate: **GO** (all 18 requirements verified, 74/74 tests pass) |

---

## 1. Context

Proposal 001 delivered the **foundation layer**: six SwiftData models, YAML parsers for three formats, ten-category validation, provenance hashing, and a verification scaffold. The domain model captures the full run lifecycle (Idea -> Run -> StageExecution -> AgentExecution -> Artifact + Approval), but nothing drives it yet. The parser loads `agents.yaml`, `workflow.yaml`, and `proposal-to-release.yaml` into typed Swift structs, but those structs are only inspected in the scaffold UI.

**What's missing** is the engine that takes an idea, compiles a workflow definition and agent catalog into an executable plan, drives the state machine through stages, calls agents, stores artifacts, tracks costs, pauses at approval gates, and resumes safely after interruption.

This proposal builds that engine.

### Why now

The PS defines the product as a "workflow constructor/executor" (PS section 2). Without execution, the app is a viewer. The PS success metric ("50% reduction in manual orchestration time per idea") requires the engineer to actually run workflows, not just inspect their definitions. Proposal 002 closes the gap between "we can read and validate a workflow" and "we can run it."

### Design principle from research

The architecture research (`goose_swiftui_agent_architecture_research.md`) establishes one core principle:

> "Your control plane is: your YAML catalog of agents and workflows, your store for jobs, approvals, artifacts, your worktree manager, your deterministic MCP services. Goose is responsible for: LLM runtime, tool usage, session lifecycle."

This means the execution engine lives in the SwiftUI app, not in the provider runtime. The engine drives state transitions, manages artifacts, and enforces approval gates. The provider is a pluggable adapter that the engine calls to execute agents.

Proposal 002 builds the engine with an abstract `AgentExecutor` protocol and a **simulated executor** for deterministic testing. Real provider adapters (Goose REST/SSE, Codex) are Proposal 003 scope.

### Dependencies on Proposal 001

| Proposal 001 artifact | Usage in Proposal 002 |
|---|---|
| `Idea`, `Run`, `StageExecution`, `AgentExecution`, `Approval`, `Artifact` models | Populated and driven by engine |
| `RunRepository` (extended with `createRunFromPlan()` in §3.5) | Phase 2 persistence boundary for the execution flow |
| `YAMLParser.loadAgentCatalog/loadWorkflow/loadCompactWorkflow` | Called by compiler to load source definitions |
| `YAMLValidator.validateAll` | Called before compilation to reject invalid configs |
| `DefinitionHasher.hash/snapshot` | Called by compiler to stamp provenance |
| `WorkflowDefinition`, `AgentCatalog`, `CompactWorkflowDefinition` | Input to RunPlan Compiler |
| `LoadState<T>` | Reused for execution UI state management |

---

## 2. What we build

Two layers, following the same pattern as Proposal 001:

### Layer C: Execution Engine

The backend machinery that compiles, drives, and manages workflow runs.

| Component | Responsibility |
|---|---|
| **RunPlan Compiler** | Combines `WorkflowDefinition` + `AgentCatalog` into an executable `RunPlan`, resolves agent references, creates immutable snapshot |
| **Compact Normalizer** | Converts `CompactWorkflowDefinition` into `WorkflowDefinition` with alias resolution |
| **Workflow Orchestrator** | Drives the state machine: start -> evaluate run blocks -> execute agents -> check transitions -> advance -> repeat |
| **Agent Executor Protocol** | Abstract interface for calling an agent with task/inputs/config and receiving outputs/cost/status |
| **Simulated Agent Executor** | Deterministic mock executor that produces structurally valid outputs without real LLM calls |
| **Artifact Manager** | Creates, stores, retrieves, and checksums artifacts on disk with SwiftData metadata |
| **Transition Evaluator** | Evaluates transition conditions (`when` clauses) against current run state and artifact existence |
| **Cost Tracker** | Records `costCents` per agent execution, aggregates into `Run.totalCostCents` |
| **Resume Manager** | On app launch, detects interrupted runs and resumes them safely per PS section 4.5 |

### Layer D: Execution UI

The views that let the engineer start runs, monitor progress, and make approval decisions.

| Component | Responsibility |
|---|---|
| **Start Run Flow** | From IdeaDetailView: select workflow -> compile -> create run -> begin execution |
| **Run Progress View** | Real-time view of stage chain with current position, agent statuses, and timing |
| **Approval Gate View** | Detail sheet launched from approval inbox: shows gate context, preceding artifacts, approve/reject buttons, optional comment |
| **Stage Detail View** | Drill-down into a stage showing agent executions, artifacts produced, and elapsed time |
| **Artifact Inspector** | Read-only view of an artifact's metadata and content (markdown/JSON) |

---

## 3. RunPlan Compiler

### 3.1 Compilation pipeline

```
                          ┌──────────────────┐
                          │  agents.yaml     │
                          │  (AgentCatalog)  │
                          └────────┬─────────┘
                                   │
┌──────────────────┐    ┌──────────▼──────────┐    ┌──────────────────┐
│ workflow.yaml    │───▶│  RunPlan Compiler    │───▶│  RunPlan         │
│ (WorkflowDef)   │    │                      │    │  (executable)    │
└──────────────────┘    │  1. Validate         │    └────────┬─────────┘
                        │  2. Resolve agents   │             │
┌──────────────────┐    │  3. Bind backends    │    ┌────────▼─────────┐
│ compact.yaml     │───▶│  4. Hash provenance  │    │  Run + stages    │
│ (CompactDef)     │    │  5. Create snapshot  │    │  (SwiftData)     │
│ [optional path]  │    │  6. Create Run       │    └──────────────────┘
└──────────────────┘    └─────────────────────┘
```

### 3.2 RunPlan structure

`RunPlan` is a **pure execution topology** — it describes the compiled workflow, resolved agents, and provenance hashes. It deliberately carries no run-scoped identity (`runID`, `ideaID`). This allows `previewCompile()` to produce a `RunPlan` that can be inspected and cancelled without side effects; identity is established only when `createRun()` persists the Run.

```swift
/// Compiled execution topology from YAML. Immutable. No run-scoped identity.
/// The orchestrator receives a (Run, RunPlan) pair — identity lives on Run.
struct RunPlan: Sendable {
    let workflowID: String
    let workflowTitle: String

    /// Resolved state machine: state ID -> ExecutableState
    let states: [String: ExecutableState]
    let initialStateID: String

    /// Resolved agent catalog bindings
    let agentBindings: [String: ResolvedAgent]

    /// Variables from workflow definition
    let variables: [String: AnyCodableValue]

    /// Scoring configuration
    let scoring: ScoringConfig?

    /// Failure policy
    let failurePolicy: FailurePolicy

    /// Provenance
    let workflowSnapshotHash: String
    let catalogSnapshotHash: String
    let workflowSnapshotJSON: Data
    let catalogSnapshotJSON: Data

    /// Compiler version — monotonic integer incremented when compiler semantics change.
    /// Persisted on Run for resume safety. If run.planCompilerVersion != current compiler version,
    /// resume blocks instead of silently recompiling with different semantics.
    let planCompilerVersion: Int
}

/// Run-scoped workspace context. Created at `createRun()` time,
/// frozen in the Run's persisted state. Passed to the orchestrator and executor.
struct RunWorkspace: Sendable {
    let runID: UUID
    let workspaceRoot: URL         // isolation boundary (from workspace-isolation-risk.md)
    let artifactRoot: URL          // {workspaceRoot}/artifacts/ — already run-scoped, no extra runID nesting
    let worktreeRoot: URL?         // set by Proposal 003 when worktrees are provisioned
}
```

> **Path contract (single rule):** `artifactRoot` is `{workspaceRoot}/artifacts/` — it is **already run-scoped** because `workspaceRoot` contains the `runID`. Paths below `artifactRoot` do NOT repeat `runID`. The full artifact path convention is:
> ```
> {artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}
> ```
> Where `name` is the full artifact filename including extension (e.g., `proposal_current.md`, `proposal_review_po.json`). The `{stageID}.{iteration}` segment encodes the loop iteration directly in the path, preventing collisions when the same stage executes multiple times (e.g., `state_5_proposal_refined.3/proposal_writer/1/proposal_current.md`). First iteration uses `.1`.

struct ExecutableState: Sendable {
    let id: String
    let label: String
    let type: StateType?
    let ownerAgentID: String
    let runBlock: ExecutableRunBlock?
    let runAfterApproval: ExecutableRunBlock?
    let transitions: [ExecutableTransition]
    let approvalRequired: Bool
    let loop: LoopConfig?
}

enum StateType: String, Sendable {
    case start, end, manualGate = "manual_gate"
}

struct ExecutableRunBlock: Sendable {
    let phases: [ExecutionPhase]
}

enum ExecutionPhase: Sendable {
    case sequential([AgentTask])
    case parallel([AgentTask])
}

struct ExecutableTransition: Sendable {
    let to: String
    let condition: TransitionCondition
}

enum TransitionCondition: Sendable {
    case always                               // when: 'true'
    case artifactExists(String)               // when: exists('artifact_name')
    case approvalGranted                      // when: approval.granted == true
    case expression(String)                   // when: <complex expression>
}

struct ResolvedAgent: Sendable {
    let id: String
    let title: String
    let mode: String
    let provider: String
    let model: String
    let effort: String
    let maxTurns: Int
    let temperature: Double
    let permissionProfile: String
    let skillRef: String
    let skillRole: String?
    let prompt: String
    let outputContract: String?
    let requiresHumanApproval: Bool
    let inputs: [String]
    let outputs: [String]
}
```

### 3.3 Two-phase compilation pipeline

The compiler is split into two explicit phases to avoid coupling preview/validation to irreversible persistence. This ensures the Start Run sheet can show a compilation preview without consuming the single-active-run slot, and that cancelling the sheet never creates orphan runs.

**Phase 1: `previewCompile` — validation and plan assembly (no persistence)**

```swift
@MainActor
final class RunPlanCompiler {
    /// Phase 1: Validate and assemble an in-memory RunPlan.
    /// Does NOT create a Run in SwiftData. Safe to call from the Start Run sheet
    /// for preview/validation. Can be cancelled without side effects.
    func previewCompile(
        workflow: WorkflowDefinition,
        catalog: AgentCatalog
    ) throws -> RunPlan

    /// Phase 1 variant: normalize compact to full, then preview-compile.
    func previewCompileCompact(
        compact: CompactWorkflowDefinition,
        catalog: AgentCatalog
    ) throws -> RunPlan

    /// Phase 2: Persist a previewed RunPlan as a Run in SwiftData.
    /// This is the irreversible step — creates the Run record only.
    /// StageExecutions and AgentExecutions are created lazily by the orchestrator (ARCH-027).
    /// Called only after the engineer confirms in the Start Run sheet.
    func createRun(
        for idea: Idea,
        plan: RunPlan,
        workflowSourcePath: String,
        catalogSourcePath: String,
        in context: ModelContext
    ) throws -> Run

    /// Resume path: rebuild an in-memory RunPlan from a persisted Run's snapshots.
    /// Does NOT create a new Run. Used by ResumeManager on app launch.
    func rebuildPlanFromSnapshot(run: Run) throws -> RunPlan
}
```

**Phase 1 sequence (previewCompile):**

1. **Validate** — call `YAMLValidator.validateAll(workflow:catalog:)`. Reject on any `.error`-severity issue.
2. **Resolve agents** — for each `AgentTask` in every state's run block, look up the agent ID in the catalog. Resolve `backend_profile` to actual provider/model/effort/maxTurns. Collect into `agentBindings`.
3. **Parse transitions** — for each state's transitions, parse `when` clause into `TransitionCondition`.
4. **Resolve loop budgets** — evaluate `vars.` references in `loop.max` fields against the workflow's `variables` dictionary at compile time. The resolved integer is stored on `ExecutableState.loop.resolvedMax`. This is the **only** compile-time variable substitution.
5. **Build ExecutableStates** — convert each `WorkflowState` into `ExecutableState` with resolved run blocks and transitions.
6. **Compute provenance** — hash workflow and catalog using `DefinitionHasher`, create JSON snapshots.
7. **Assemble RunPlan** — return the in-memory plan. No SwiftData mutations.

**Phase 2 sequence (createRun):**

8. **Generate run identity** — create a `UUID` for the new run.
9. **Provision workspace** — create `RunWorkspace` with `runID`, `workspaceRoot` as `{appSupport}/Chainworks Forge/runs/{runID}/`, `artifactRoot` as `{workspaceRoot}/artifacts/` (already run-scoped — no extra runID nesting below this point), and `worktreeRoot` as `nil` (deferred to Proposal 003). Create the workspace directories on disk.
10. **Persist Run** — call the **new** `RunRepository.createRunFromPlan(for:plan:workspace:workflowSourcePath:catalogSourcePath:)` which accepts the precompiled plan and provisioned workspace. This method persists a `Run` with all provenance hashes, snapshot JSON, **and the new workspace-path fields** (`workspaceRoot`, `artifactRoot`). This is the only irreversible step.
11. **Return Run** — no StageExecutions are created upfront. They are created **lazily** by the orchestrator at the moment a stage begins execution. This avoids conflict with Proposal 001's non-optional `startedAt` field on `StageExecution` (writing a fake timestamp for pending stages would violate the model contract). Same applies to `AgentExecution` — created when the agent is actually scheduled, not when the run starts.

> **Invariant preserved from Proposal 001:** `Run.currentStageID` remains derived from `stageExecutions` (not stored). The orchestrator's `currentStateID` property is an **in-memory cache for UI responsiveness only** — it is never a second source of truth. The canonical current stage is always `Run.currentStageID` computed from persisted `StageExecution` records.

> **Model changes required (see §3.5):** Phase 2 depends on new persisted fields on `Run` and a new `RunRepository` API. These changes are explicit in §3.5 and in the file structure.

### 3.5 Required model changes

Proposal 002 adds the following persisted fields to `Run.swift`:

```swift
// Added to @Model final class Run — Proposal 002
private(set) var workspaceRoot: String       // absolute path, frozen at run creation
private(set) var artifactRoot: String        // absolute path, frozen at run creation
private(set) var planCompilerVersion: Int     // compiler semantics version, for resume safety
```

And a new `RunRepository` method in `RunRepository.swift`:

```swift
// Added to RunRepository — Proposal 002
/// Create a Run from a precompiled plan and provisioned workspace.
/// Replaces the Proposal 001 convenience method for the execution flow.
func createRunFromPlan(
    for idea: Idea,
    plan: RunPlan,
    workspace: RunWorkspace,
    workflowSourcePath: String,
    catalogSourcePath: String
) throws -> Run
```

This method:
- Creates a `Run` with `workflowID`, `workflowTitle`, snapshot hashes, and snapshot JSON from the `RunPlan`.
- Stores `workspaceRoot` and `artifactRoot` from `RunWorkspace` for resume reconstruction.
- Enforces the single-active-run invariant (same as existing `createRun`).
- The existing Proposal 001 `createRun(for:workflow:catalog:...)` method remains for backward compatibility but is not used by the execution flow.

**Resume path (rebuildPlanFromSnapshot):**

- Decode `WorkflowDefinition` from `run.workflowSnapshotJSON`.
- Decode `AgentCatalog` from `run.catalogSnapshotJSON`.
- Call `previewCompile()` to rebuild the in-memory plan.
- This ensures resume always uses the frozen snapshot, not current YAML files (PS §4.5).

### 3.4 Compilation errors

```swift
enum CompilationError: Error, LocalizedError {
    case validationFailed([ValidationIssue])
    case agentNotFound(agentID: String, stateID: String)
    case backendProfileNotFound(profileID: String, agentID: String)
    case circularTransitions(stateIDs: [String])
    case noInitialState
    case noEndState
    case unreachableStates([String])
    case duplicateStateIDs([String])
}
```

---

## 4. Compact Normalizer

### 4.1 Alias resolution

Compact `proposal-to-release.yaml` uses hyphenated agent IDs (e.g., `proposal-writer`) while the canonical catalog uses underscored IDs (e.g., `proposal_writer`). The normalizer resolves these aliases:

```swift
struct CompactNormalizer {
    /// Convert a CompactWorkflowDefinition into a WorkflowDefinition.
    /// Agent ID aliases (hyphens) are resolved to canonical IDs (underscores).
    /// Validation against the catalog catches any unresolved aliases.
    static func normalize(
        _ compact: CompactWorkflowDefinition,
        catalog: AgentCatalog
    ) throws -> WorkflowDefinition
}
```

### 4.2 Normalization rules

| Compact field | Full workflow equivalent |
|---|---|
| `stage.id` | `state.id` (used as-is; compact IDs like `draft_initial_proposal` are already descriptive) |
| `stage.type: single` | Single-agent `run.sequence` block |
| `stage.type: fanout` | Multi-agent `run.parallel` block with `then` for aggregation |
| `stage.type: approval` | `type: manual_gate`, `approval: required` |
| `stage.agent` | Resolved agent ID (hyphens -> underscores) |
| `stage.agents` | Resolved agent IDs for parallel execution |
| `stage.needs` | `transitions` from the needed states to this state |
| `stage.gate.require` | Added as transition conditions on outgoing edges |

### 4.3 Agent alias resolution

The compact format uses short, human-friendly agent IDs. Resolution uses exactly two deterministic strategies, applied in order. **There is no automatic guessing or abbreviation generation.**

**Strategy 1: Mechanical transform** — replace hyphens with underscores, attempt direct catalog lookup.

```
compact agent ID          catalog lookup
───────────────────────── ──────────────────────────────
proposal-writer         → proposal_writer          ✅ direct match
code-writer             → code_writer              ✅ direct match
docs-guardian           → docs_guardian             ✅ direct match
```

**Strategy 2: Explicit `agent_aliases` map** — for IDs that don't resolve via Strategy 1, the compact workflow MUST include an `agent_aliases` section. This is a required field when the compact format uses non-mechanical aliases.

```yaml
# In proposal-to-release.yaml (REQUIRED for non-trivial aliases)
agent_aliases:
  proposal-po-reviewer: proposal_reviewer_product_owner
  proposal-ux-reviewer: proposal_reviewer_ux
  proposal-ui-reviewer: proposal_reviewer_ui
  proposal-arch-reviewer: proposal_reviewer_architect
  auditor: proposal_implementation_auditor
  security: security_checker
  prepush-reviewer: prepush_code_reviewer
  github-push: commit_and_push_to_github
  connect-publish: build_archive_and_push_connect
  orchestrator: lead_orchestrator
```

If an alias cannot be resolved after both strategies, `CompilationError.agentNotFound` is thrown. There is no fallback.

> **Implementation note:** The canonical compact fixture `proposal-to-release.yaml` must be updated to include the `agent_aliases` section before Proposal 002 implementation begins. The `CompactWorkflowDefinition` Codable struct must be extended with an optional `agentAliases: [String: String]?` field (CodingKey: `agent_aliases`).

### 4.4 Locked decisions

- **ARCH-010**: Compact is a separate format, not a subset of full workflow. Normalization is a one-way transform.
- **ARCH-011**: Compact normalization does NOT preserve scoring, variables, or failure_policy. These are added from defaults or from the catalog's `app` section.
- **ARCH-012**: Agent alias resolution is deterministic and explicit. Strategy 1 is mechanical (hyphens → underscores). Strategy 2 is an explicit `agent_aliases` map declared in the compact YAML. **No automatic abbreviation generation, no guessing.** Unresolvable IDs are compilation errors. The normalizer code contains zero alias literals.

---

## 5. Workflow Orchestrator

### 5.1 App-scoped execution service

Workflow execution must outlive any single view. When the engineer navigates away from the Run Progress view, switches tabs, or the app approval gate sheet is dismissed, execution must continue. The orchestrator is therefore owned by an **app-scoped service**, not by a view.

```swift
/// App-scoped singleton that owns all active workflow executions.
/// Injected into the SwiftUI environment at app startup.
/// Views observe it to display progress; it does not depend on any view being alive.
@MainActor
@Observable
final class ExecutionService {
    private(set) var activeOrchestrators: [UUID: WorkflowOrchestrator] = [:]  // runID -> orchestrator
    private(set) var pendingApprovals: [UUID: ApprovalRequest] = [:]          // runID -> pending gate

    let compiler: RunPlanCompiler
    let artifactManager: ArtifactManager
    let resumeManager: ResumeManager
    let executor: AgentExecutor

    /// Start a new run for an idea using a previewed plan.
    /// Calls createRun (Phase 2), provisions RunWorkspace, starts orchestrator.
    func startRun(idea: Idea, plan: RunPlan, workflowSourcePath: String, catalogSourcePath: String) async throws -> Run

    /// Resume interrupted runs detected on app launch.
    func resumeInterruptedRuns() async

    /// Respond to a pending approval gate.
    func resolveApproval(runID: UUID, stageID: String, decision: ApprovalDecision, comment: String?) async

    /// Cancel a run.
    func cancelRun(_ runID: UUID) async

    /// Orchestrator for a specific run (for progress observation).
    func orchestrator(for runID: UUID) -> WorkflowOrchestrator?
}
```

**Lifecycle:**
- `ExecutionService` is created in `Chainworks_ForgeApp.init` and injected via `.environment()`.
- On app launch, `resumeInterruptedRuns()` is called from the app's `.task {}` modifier.
- Views observe `ExecutionService.activeOrchestrators` and `pendingApprovals` — they never own or retain an orchestrator directly.
- When `pendingApprovals` is non-empty, the app presents an approval inbox on the root ContentView. The engineer selects an approval to review, opening the ApprovalGateView as a sheet. Multiple ideas can be waiting at gates simultaneously (the PS invariant is one active run per **idea**, not one active run per **app**).
- Execution continues even when no execution-related view is on screen.

### 5.2 Orchestrator contract

Each run gets its own `WorkflowOrchestrator` instance, owned by the `ExecutionService`:

```swift
@MainActor
@Observable
final class WorkflowOrchestrator {
    let runID: UUID
    private(set) var runStatus: RunStatus = .pending
    private(set) var currentStateID: String?

    /// Start executing a compiled RunPlan for a persisted Run.
    func start(run: Run, plan: RunPlan, workspace: RunWorkspace, executor: AgentExecutor) async

    /// Resume an interrupted run from its last known state.
    func resume(run: Run, plan: RunPlan, workspace: RunWorkspace, executor: AgentExecutor) async

    /// Respond to an approval gate.
    func resolveApproval(
        run: Run,
        stageID: String,
        decision: ApprovalDecision,
        comment: String?
    ) async

    /// Cancel a running or paused run.
    func cancel(run: Run) async
}

/// Published by the orchestrator when an approval gate is reached.
/// The ExecutionService surfaces this to the UI layer.
struct ApprovalRequest: Identifiable {
    let id: UUID  // approval record ID
    let runID: UUID
    let stageID: String
    let stageLabel: String
    let precedingArtifacts: [String]  // artifact names available for review
}
```

> **Actor isolation note:** The orchestrator is `@MainActor` because it writes to SwiftData models. However, `AgentExecutor.execute()` is `nonisolated` (the protocol is merely `Sendable`, not actor-isolated). To enable true parallel execution, the orchestrator calls executor methods **off the MainActor** via `@Sendable` closures, then hops back to `@MainActor` only for SwiftData state updates. See §5.4 for the run block execution pattern.

### 5.3 State machine execution loop

```
┌─────────────────────────────────────────────────────────────────┐
│                    Orchestrator Main Loop                       │
│                                                                 │
│  1. Load current state from RunPlan                             │
│  2. If approval required → set status waiting_approval → PAUSE  │
│  3. If run block exists:                                        │
│     a. For each phase in run block:                             │
│        - sequential: execute agents one-by-one                  │
│        - parallel: execute agents concurrently                  │
│     b. After all phases complete, mark stage completed          │
│  4. Evaluate transitions:                                       │
│     a. For each transition in order, evaluate condition          │
│     b. First matching transition → advance to target state      │
│     c. No matching transition → mark run as blocked             │
│  5. If target state is end type → mark run completed            │
│  6. If loop detected → increment counter, check budget          │
│  7. Go to step 1 with new current state                         │
│                                                                 │
│  On error: apply failure_policy                                 │
│  On loop budget exhausted: apply failure_policy                 │
│  On cancellation: mark run cancelled                            │
└─────────────────────────────────────────────────────────────────┘
```

### 5.4 Run block execution

A run block contains phases executed in order. Each phase is either sequential or parallel:

```swift
/// Execute a run block within a stage.
/// Agent executor calls run off-MainActor for true parallelism;
/// SwiftData state updates hop back to @MainActor.
private func executeRunBlock(
    _ block: ExecutableRunBlock,
    stage: StageExecution,
    plan: RunPlan,
    workspace: RunWorkspace,
    executor: AgentExecutor
) async throws {
    for phase in block.phases {
        switch phase {
        case .sequential(let tasks):
            for task in tasks {
                let result = try await runAgentOffMainActor(
                    task, plan: plan, workspace: workspace, executor: executor)
                recordAgentResult(result, task: task, stage: stage)
            }
        case .parallel(let tasks):
            // Agent execution runs off-MainActor for true concurrency.
            // Results are collected and then applied to SwiftData on MainActor.
            let results = try await withThrowingTaskGroup(
                of: (AgentTask, AgentResult).self
            ) { group in
                for task in tasks {
                    let capturedExecutor = executor
                    let capturedPlan = plan
                    let capturedWorkspace = workspace
                    group.addTask {
                        // This closure is @Sendable, NOT @MainActor — runs concurrently
                        let result = try await capturedExecutor.execute(
                            agent: capturedPlan.agentBindings[task.agent]!,
                            task: task,
                            inputs: [:], // resolved via ArtifactManager before call
                            context: ExecutionContext(
                                workspace: capturedWorkspace, /* ... */)
                        )
                        return (task, result)
                    }
                }
                var collected: [(AgentTask, AgentResult)] = []
                for try await pair in group { collected.append(pair) }
                return collected
            }
            // Back on @MainActor — safe to write SwiftData
            for (task, result) in results {
                recordAgentResult(result, task: task, stage: stage)
            }
        }
    }
}

/// Calls executor off-MainActor. Returns result without touching SwiftData.
nonisolated private func runAgentOffMainActor(
    _ task: AgentTask,
    plan: RunPlan,
    workspace: RunWorkspace,
    executor: AgentExecutor
) async throws -> AgentResult {
    try await executor.execute(
        agent: plan.agentBindings[task.agent]!,
        task: task,
        inputs: [:],  // resolved via ArtifactManager before call
        context: ExecutionContext(workspace: workspace, /* ... */)
    )
}

/// Records result into SwiftData. Must be called on @MainActor.
@MainActor
private func recordAgentResult(
    _ result: AgentResult,
    task: AgentTask,
    stage: StageExecution
) { /* update AgentExecution, store Artifacts via ArtifactManager, aggregate cost */ }
```

**Mapping from YAML `run` blocks:**

| YAML field | Execution |
|---|---|
| `run.sequence` | Single sequential phase |
| `run.parallel` | Single parallel phase |
| `run.parallel` + `run.then` | Two phases: parallel first, then sequential |

### 5.5 Transition evaluation

```swift
struct TransitionEvaluator {
    /// Evaluate a transition condition against current run state.
    /// Returns true if the transition should fire.
    static func evaluate(
        _ condition: TransitionCondition,
        run: Run,
        artifacts: [String: Artifact],
        variables: [String: AnyCodableValue]
    ) -> Bool
}
```

**Supported condition types:**

| Condition | Example | Evaluation |
|---|---|---|
| `always` | `when: 'true'` | Always true |
| `artifactExists` | `when: exists('idea_brief')` | Check artifact dictionary |
| `approvalGranted` | `when: approval.granted == true` | Check the stage's latest Approval record has `.granted` decision |
| `expression` | `when: proposal_review_summary.aggregate_score > vars.proposal_score_target` | Parse and evaluate expression against artifact JSON fields and variables |

**Condition parsing rules:**
- `'true'` → `.always`
- `exists('...')` → `.artifactExists`
- `approval.granted == true` → `.approvalGranted` (special-cased; approval state lives on the Approval model, not on an artifact)
- Everything else → `.expression` (parsed and evaluated at runtime)

**Expression evaluator — minimal canonical-only subset:**

The evaluator is intentionally narrow. It supports only the condition patterns that actually appear in `workflow.yaml`, not a general-purpose expression language. The canonical workflow uses exactly these patterns:

| Pattern | Example from workflow.yaml | Support |
|---|---|---|
| `artifact.field > vars.X` | `proposal_review_summary.aggregate_score > vars.proposal_score_target` | Read JSON artifact field, compare numeric against variable |
| `artifact.field >= vars.X` | `proposal_review_summary.min_individual_score >= vars.min_individual_proposal_score` | Same, `>=` |
| `artifact.field == value` | `implementation_review_summary.status == vars.implementation_target_status` | String equality |
| `artifact.field == literal` | `git_push_receipt.status == 'success'` | String literal comparison |
| `and` | Multiple conditions joined with `and` | Logical AND |
| `or` | Two branches joined with `or` | Logical OR |
| `vars.X` | Variable substitution | Resolved at **runtime** from `RunPlan.variables` |

**Not supported in Proposal 002** (deferred to Proposal 003+ if needed):
- Nested field access (`a.b.c`)
- `in [...]` containment
- `not` / negation
- Arithmetic (`+`, `-`, `*`, `/`)
- Function calls beyond `exists()`

> **Variable resolution contract:** `loop.max` is the only field resolved at **compile time** (§3.3 step 4). Transition `when` expressions keep `vars.*` references as runtime AST nodes, evaluated by the `TransitionEvaluator` against `RunPlan.variables` at each transition check. This avoids baking computed values into the plan while still keeping variables immutable (they come from the frozen workflow snapshot).

### 5.6 Loop management

When a state has a `loop` configuration:

1. Orchestrator reads `loop.counter` name from the plan. The `loop.max` value is resolved at **compile time** from `vars.*` references (e.g., `vars.max_proposal_revision_cycles` → `6`). The resolved integer is stored on `ExecutableState.loop`.
2. On entering the state, the orchestrator checks `Run.loopCounters[counter]`:
   - If the key doesn't exist yet (first entry), it initializes to `1`.
   - If the key exists (re-entry via a back-edge transition), it increments by `1`.
3. If counter exceeds the resolved max, triggers `failure_policy.on_loop_budget_exhausted` (`pause_and_require_human`).
4. The current iteration number is also set on `StageExecution.iteration` for traceability.
5. Loop counters persist on the Run in SwiftData so resume doesn't lose iteration state.

### 5.7 Failure handling

```swift
enum FailureAction: Sendable {
    case pauseAndRequireHuman   // failure_policy.on_error: pause_and_require_human
    case retryStage(maxRetries: Int)
    case cancelRun
}
```

When an agent fails or a stage errors:
1. `AgentExecution.status` → `.failed`
2. `StageExecution.status` → `.failed`
3. Apply `failurePolicy`:
   - `pause_and_require_human` → `Run.status` → `.blocked`, surface to UI
   - Preserve all artifacts (`preserve_artifacts: true`)
4. Engineer can retry or cancel from the UI.

---

## 6. Agent Execution Protocol

### 6.1 Protocol definition

```swift
/// Abstract interface for executing an agent task.
/// Proposal 002 provides SimulatedAgentExecutor.
/// Proposal 003 provides GooseAgentExecutor (real provider).
protocol AgentExecutor: Sendable {
    /// Execute an agent task and produce output data.
    /// The executor does NOT write artifacts to disk — it returns raw output data.
    /// ArtifactManager is the sole owner of artifact persistence (see §7.2).
    func execute(
        agent: ResolvedAgent,
        task: AgentTask,
        inputs: [String: Data],       // artifact name -> content (read by ArtifactManager before call)
        context: ExecutionContext
    ) async throws -> AgentResult
}

struct ExecutionContext: Sendable {
    let workspace: RunWorkspace    // run-scoped isolation boundary
    let stageID: String
    let iteration: Int
    let attemptNumber: Int
    let variables: [String: AnyCodableValue]
}

struct AgentResult: Sendable {
    let status: AgentStatus
    let outputs: [String: Data]       // output artifact name -> raw content (NOT file paths)
    let costCents: Int64
    let logSnippet: String?
    let sessionID: String?
    let durationSeconds: Double
}
```

> **Artifact write ownership (single rule):** The executor **produces** output data. The `ArtifactManager` **persists** it. The executor never writes to the filesystem directly. This eliminates fan-out races where parallel agents compete for file paths, and keeps all path convention logic in one place (`ArtifactStorage`). The orchestrator's `recordAgentResult` method feeds each `AgentResult.outputs` entry into `ArtifactManager.store()`.
```

### 6.2 Simulated Agent Executor

For testing the full execution pipeline without real LLM providers:

```swift
/// Deterministic executor that produces structurally valid mock outputs.
/// Configurable success/failure rates, delays, and output templates.
final class SimulatedAgentExecutor: AgentExecutor {
    struct Configuration: Sendable {
        /// Simulated execution delay per agent (seconds).
        var delayRange: ClosedRange<Double> = 0.1...0.5

        /// Cost per agent execution in cents.
        var costCentsPerExecution: Int64 = 100

        /// Agents that should fail (by ID).
        var failingAgents: Set<String> = []

        /// Custom output generators by agent mode.
        var outputGenerators: [String: @Sendable (AgentTask) -> [String: Data]] = [:]
    }

    func execute(...) async throws -> AgentResult
}
```

**Default output generation:**

For each expected output artifact, the simulated executor produces:
- **Markdown artifacts** (`.md`): template text with agent ID, task name, and timestamp
- **JSON artifacts** (`.json`): structurally valid JSON matching the output contract's `required_fields`
- **Review artifacts** (`proposal_review_v1`): JSON with `score: 9.5`, `verdict: "pass"`, `blocker_count: 0`, etc.
- **Assessment artifacts** (`implementation_self_assessment_v1`): JSON with `seemingly_complete: true`, `status: "Implemented"`

This enables the orchestrator to evaluate transition conditions on simulated data and drive the full workflow to completion.

### 6.3 Output contract templates

```swift
/// Generates structurally valid mock output for a given contract.
struct OutputContractTemplates {
    static func generate(
        contractID: String,
        agentID: String,
        task: String
    ) -> Data  // JSON or Markdown
}
```

Templates for all 11 contracts defined in `agents.yaml`:
1. `proposal_review_v1` — agent_id, role, score, verdict, blocking_issues, suggestions
2. `proposal_review_summary_v1` — aggregate_score, min_individual_score, blocker_count, summary
3. `implementation_self_assessment_v1` — status, seemingly_complete, progress_pct, remaining_work
4. `audit_report_v1` — status, requirement_coverage, gaps, evidence
5. `security_report_v1` — status, findings, severity_counts, recommendations
6. `prepush_review_v1` — status, issues, risk_assessment
7. `implementation_review_summary_v1` — aggregate status across auditor/security/prepush/docs
8. `docs_report_v1` — status, changed_files, alignment_score
9. `git_push_receipt_v1` — commit_sha, branch, push_status, tag
10. `connect_upload_receipt_v1` — artifact_id, checksum, upload_status, destination
11. `final_feature_report_v1` — summary, elapsed_time, total_cost, stages_completed

---

## 7. Artifact Manager

### 7.1 Storage layout

`artifactRoot` is already run-scoped (lives inside `workspaceRoot`). No extra `runID` nesting. Loop iterations are encoded in the stage segment.

```
{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}
```

Where `name` is the full artifact filename including extension (e.g., `proposal_current.md`).

Example (run workspace at `~/Library/Application Support/Chainworks Forge/runs/a1b2c3d4/`):
```
artifacts/
  state_2_proposal_drafted.1/
    proposal_writer/
      1/
        proposal_current.md
        proposal_revision_summary.json
  state_4_proposal_reviewed.1/
    proposal_reviewer_product_owner/
      1/
        proposal_review_po.json
    proposal_reviewer_ux/
      1/
        proposal_review_ux.json
  state_5_proposal_refined.1/           ← first loop iteration
    proposal_writer/
      1/
        proposal_current.md
  state_5_proposal_refined.2/           ← second loop iteration
    proposal_writer/
      1/
        proposal_current.md
```

### 7.2 Artifact Manager contract

The artifact manager is split into two layers to keep disk I/O off the main thread:

**Layer 1: `ArtifactStorage` — background disk I/O (not actor-isolated)**

```swift
/// Handles all file system operations. Thread-safe, no actor isolation.
/// Can be called from any context (including parallel agent execution).
final class ArtifactStorage: Sendable {
    let artifactRoot: URL

    /// Write artifact data to disk at the conventional path.
    /// Path: {artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}
    /// Returns the file URL and SHA-256 checksum.
    func write(
        name: String,
        data: Data,
        stageID: String,
        iteration: Int,
        agentID: String,
        attemptNumber: Int
    ) throws -> (fileURL: URL, checksum: String, sizeBytes: Int64)

    /// Read artifact content from disk.
    func read(at filePath: URL) throws -> Data

    /// Build the conventional path for an artifact.
    func artifactPath(
        stageID: String, iteration: Int, agentID: String,
        attemptNumber: Int, name: String
    ) -> URL
}
```

**Layer 2: `ArtifactManager` — MainActor metadata adapter**

```swift
/// Records artifact metadata in SwiftData. Must be called on @MainActor.
/// Delegates all disk I/O to ArtifactStorage.
@MainActor
final class ArtifactManager {
    let storage: ArtifactStorage
    let modelContext: ModelContext

    /// Store an artifact: write to disk (via ArtifactStorage), then record in SwiftData.
    /// `contractID` matches the agent's `output_contract` from the catalog.
    /// Pass empty string for agents without a declared contract.
    func store(
        name: String,
        data: Data,
        format: ArtifactFormat,
        contractID: String,
        run: Run,
        stageID: String,
        agentID: String,
        provider: String,
        model: String?,
        effort: String?,
        attemptNumber: Int
    ) throws -> Artifact

    /// Retrieve an artifact's file URL by name within a run.
    /// Queries SwiftData for the most recent artifact with matching name.
    func resolve(name: String, in run: Run) -> URL?

    /// Resolve multiple input artifacts for an agent task.
    func resolveInputs(
        _ inputNames: [String],
        in run: Run
    ) -> [String: URL]
}
```

> **Actor isolation boundary:** `ArtifactStorage.write/read` perform disk I/O and are `nonisolated` — they can be called from parallel agent execution contexts without blocking the main thread. `ArtifactManager.store` calls `ArtifactStorage.write` (off-MainActor), then creates the SwiftData `Artifact` record (on MainActor). This matches the claim in the risk table that "artifact writes happen in background; SwiftData metadata update on MainActor."

### 7.3 Artifact format detection

The existing `ArtifactFormat` enum from Proposal 001 has four cases: `.json`, `.markdown`, `.diff`, `.report`. Proposal 001's `ArtifactContract` already stores a `format` field — that is the source of truth when a contract is present. Detection follows a strict priority order:

```swift
extension ArtifactFormat {
    /// Detect format. Priority: explicit extension > contract.format > fallback.
    /// `contract` is the resolved ArtifactContract from the agent catalog (if the agent has one).
    static func detect(from name: String, contract: ArtifactContract?) -> ArtifactFormat {
        // 1. File extension takes precedence
        if name.hasSuffix(".json") { return .json }
        if name.hasSuffix(".md") { return .markdown }
        if name.hasSuffix(".diff") || name.hasSuffix(".patch") { return .diff }

        // 2. If an output contract exists, use its declared format
        if let contract {
            return ArtifactFormat(rawValue: contract.format) ?? .json
        }

        // 3. Fallback: treat as report (generic structured output)
        return .report
    }
}
```

### 7.4 Input binding

When an agent's `inputs` list references artifact names:
1. `ArtifactManager.resolveInputs` looks up each name in the run's artifact history.
2. Latest artifact with matching name is selected (handles retries producing new versions).
3. Special inputs `input.idea` and `input.file` are resolved from the Idea model.
4. Missing inputs that are required cause an error; optional inputs are skipped.

---

## 8. Approval Flow

### 8.1 Gate lifecycle

```
Stage enters state with approval: required
    │
    ▼
Orchestrator pauses execution
    │
    ▼
StageExecution.status → .waitingApproval
Run.status → .waitingApproval
    │
    ▼
Create Approval record: .pending → .requested
    │
    ▼
UI adds entry to ExecutionService.pendingApprovals; approval inbox surfaces on root ContentView
    │
    ├─ Engineer approves → Approval.decision = .granted
    │   │
    │   ▼
    │   If run_after_approval exists → execute that block
    │   Otherwise → evaluate transitions → advance
    │
    └─ Engineer rejects → Approval.decision = .rejected
        │
        ▼
        Run.status → .cancelled (or .blocked per failure_policy)
```

### 8.2 Approval context

When presenting an approval gate, the UI shows:
- **Stage label** (e.g., "Human approval: initial proposal matches intent")
- **Preceding stage summary** (what agents ran, what artifacts were produced)
- **Key artifacts** for review (e.g., the proposal draft before approval)
- **Approve** button (with optional comment field)
- **Reject** button (with required rejection reason)

### 8.3 Approval expiration

The `Approval` model has an `expiresAt` field (from Proposal 001). In Proposal 002, approval expiration is **not enforced** — gates wait indefinitely for engineer input. The `expiresAt` field is set to `nil` on creation. Expiration enforcement (auto-reject after timeout) is deferred to Proposal 003+, where it may be needed for automated/headless runs.

### 8.4 `run_after_approval` handling

Some approval gates have a `run_after_approval` block (e.g., `state_11_manual_release`). After the engineer approves:
1. The `run_after_approval` block executes (not the `run` block).
2. This is used for side-effect stages where execution only happens after explicit approval.
3. The pattern ensures `commit_and_push_to_github` and `build_archive_and_push_connect` never run without approval.

---

## 9. Cost Tracking

When an agent execution completes:
1. `AgentResult.costCents` is recorded on `AgentExecution.costCents`.
2. The orchestrator's `recordAgentResult` method recalculates `Run.totalCostCents` as the sum of all `AgentExecution.costCents` across all stages in the run.
3. For the simulated executor, cost is configurable (default: 100 cents per execution).
4. For real providers (Proposal 003), cost is derived from token usage * provider pricing.

Cost tracking is handled inline by the orchestrator's `recordAgentResult` method (§5.4) — no separate `CostTracker` class is needed. The logic is: `run.totalCostCents = run.stageExecutions.flatMap(\.agentExecutions).compactMap(\.costCents).reduce(0, +)`.

---

## 10. Resume Manager

```swift
@MainActor
final class ResumeManager {
    let modelContext: ModelContext

    /// Check for interrupted runs on app launch.
    /// Returns runs that need attention.
    func detectInterruptedRuns() -> [InterruptedRun]

    /// Resume a run safely.
    func resume(
        _ interrupted: InterruptedRun,
        executor: AgentExecutor
    ) async
}

struct InterruptedRun {
    let run: Run
    let plan: RunPlan
    let resumeAction: ResumeAction
}

enum ResumeAction: Sendable {
    case autoResume              // Safe local stage, continue from last completed
    case restoreApprovalGate     // Was waiting for approval, restore gate
    case blockForReview          // Side-effect stage or unknown state
}
```

### 10.1 RunPlan reconstruction on resume

On app launch, the persisted `Run` contains `workflowSnapshotJSON` and `catalogSnapshotJSON` — the full JSON snapshots stamped at compile time. The `ResumeManager` reconstructs a `RunPlan` using the compiler's dedicated resume path:

1. **Check compiler version** — compare `run.planCompilerVersion` against `RunPlanCompiler.currentVersion`. If they differ, the run is **blocked** (not auto-resumed), because the compiler semantics may have changed since the run was created. The engineer must cancel and re-create the run.
2. Call `RunPlanCompiler.rebuildPlanFromSnapshot(run:)`.
3. Internally, this decodes `WorkflowDefinition` from `run.workflowSnapshotJSON` and `AgentCatalog` from `run.catalogSnapshotJSON`.
4. These decoded definitions are fed into `previewCompile()` to rebuild the in-memory `RunPlan`.
5. A `RunWorkspace` is reconstructed from the Run's persisted paths (`run.workspaceRoot`, `run.artifactRoot`).
6. The result is a `(RunPlan, RunWorkspace)` pair ready for `WorkflowOrchestrator.resume()`.
7. This ensures resume always uses the **original compiled plan**, not the current YAML files on disk (PS §4.5: "Resumed runs must continue from the frozen run snapshot"), AND guards against compiler drift via version check.

### 10.2 Resume rules (from PS section 4.5)

| Last known state | Run status | Action |
|---|---|---|
| Running a safe local stage | `.running` | Auto-resume from last completed stage |
| Waiting at approval gate | `.waitingApproval` | Restore to `waitingApproval`, show gate |
| Running a side-effect stage (push/publish) | `.running` | Block, require human review |
| Failed | `.failed` | Stay failed, show error |
| Completed | `.completed` | No action |

**Side-effect stage detection:** A stage is classified as side-effect if its agents include any with `requires_human_approval: true` or permission profiles `RELEASE_GIT` / `RELEASE_PUBLISH`.

### 10.3 Drift detection (model only)

On resume, before reconstructing the RunPlan, the orchestrator optionally detects if `agents.yaml` or `workflow.yaml` have changed since the run started:
1. Re-hash the current YAML files using `DefinitionHasher`.
2. Compare against `Run.workflowSnapshotHash` and `Run.catalogSnapshotHash`.
3. If different, set `Run.driftDetectedAt` and populate `Run.driftDetails` with a human-readable summary (e.g., "Workflow hash changed: agents.yaml modified since run started").
4. Set `Run.status` to `.blocked`.

**Drift-review UI is out of scope for Proposal 002.** The model fields exist (from Proposal 001), and the detection logic is implemented, but the three-button decision UI (continue with original / restart with current / cancel) is deferred to Proposal 003/004. In Proposal 002, drift causes a block that the engineer resolves by cancelling and creating a new run.

---

## 11. Execution UI

### 11.1 Enhanced IdeaDetailView

Upgrade the existing `IdeaDetailView` to show run lifecycle:

```
┌──────────────────────────────────────────────┐
│ Idea: "Add user authentication"              │
├──────────────────────────────────────────────┤
│ Status: draft                                │
│ Created: 2026-03-22 14:30                    │
│ Body: Implement OAuth2 login flow with...    │
├──────────────────────────────────────────────┤
│ Runs                                         │
│ ┌──────────────────────────────────────────┐ │
│ │ Run #1 — Proposal to Release             │ │
│ │ Status: running ● Stage: Proposal review │ │
│ │ Elapsed: 2m 34s  Cost: $1.20             │ │
│ │ [View Progress]                          │ │
│ └──────────────────────────────────────────┘ │
│                                              │
│ [▶ Start New Run]                            │
└──────────────────────────────────────────────┘
```

### 11.2 Start Run Sheet

When the engineer taps [Start New Run]:

```
┌──────────────────────────────────────────────┐
│ Start Run                                    │
├──────────────────────────────────────────────┤
│ Workflow: [Proposal to Release    ▾]         │
│                                              │
│ ✅ Workflow validated: 12 states, 0 errors   │
│ ✅ Agent catalog: 13 agents resolved         │
│ ✅ Provenance hashed                         │
│                                              │
│         [Cancel]           [Start Run]       │
└──────────────────────────────────────────────┘
```

### 11.3 Run Progress View

```
┌──────────────────────────────────────────────────┐
│ Run: Proposal to Release                         │
│ Status: running   Elapsed: 4m 12s   Cost: $2.40  │
├──────────────────────────────────────────────────┤
│                                                  │
│  ▶ Idea received              ✅ 12s  $0.10     │
│  ▶ Proposal drafted           ✅ 45s  $0.30     │
│  ✋ Initial proposal approval  ✅ approved        │
│  ▶ Proposal reviewed          ⏳ running...      │
│    ├─ PO review               ✅ 9.2             │
│    ├─ UX review               ✅ 9.0             │
│    ├─ UI review               ⏳ running         │
│    └─ Architect review        ○ pending          │
│  ○ Proposal refined           ○ pending          │
│  ✋ Implementation approval    ○ pending          │
│  ○ Implementation started     ○ pending          │
│  ...                                             │
│                                                  │
└──────────────────────────────────────────────────┘
```

### 11.4 Approval Gate View

When an approval gate is reached, it appears in the approval inbox on root ContentView. The engineer selects it to open the ApprovalGateView as a detail sheet:

```
┌──────────────────────────────────────────────┐
│ ✋ Approval Required                          │
├──────────────────────────────────────────────┤
│                                              │
│ Stage: Initial proposal approval             │
│ "Human approval: initial proposal            │
│  matches intent"                             │
│                                              │
│ Preceding work:                              │
│ • Idea normalized by Lead/Orchestrator       │
│ • Proposal drafted by Proposal Writer        │
│                                              │
│ Key artifacts:                               │
│ 📄 proposal_current.md        [View]         │
│ 📄 proposal_revision_summary  [View]         │
│                                              │
│ Comment (optional):                          │
│ ┌──────────────────────────────────────────┐ │
│ │                                          │ │
│ └──────────────────────────────────────────┘ │
│                                              │
│   [✗ Reject]                 [✓ Approve]     │
└──────────────────────────────────────────────┘
```

### 11.5 Artifact Inspector

Simple read-only viewer for artifact content:
- **Markdown**: rendered as formatted text with `Text(markdown:)` or `AttributedString`
- **JSON**: pretty-printed with syntax highlighting, key fields highlighted
- **Text**: monospaced raw display

### 11.6 LoadState reuse

All new views reuse the `LoadState<T>` pattern from Proposal 001 for async operations:
- Compilation → `LoadState<RunPlan>`
- Artifact loading → `LoadState<Data>`
- Run state is directly observable via `@Query` on SwiftData models

---

## 12. Testing

### 12.1 RunPlan Compiler tests

```swift
class RunPlanCompilerTests: XCTestCase {
    // Compile canonical workflow.yaml + agents.yaml into RunPlan
    func testCompileCanonicalWorkflow()

    // All 13 agents resolved with correct backend profiles
    func testAllAgentsResolved()

    // Initial state is state_1_idea_received
    func testInitialState()

    // End state is state_12_workflow_complete
    func testEndState()

    // Provenance hashes match DefinitionHasher output
    func testProvenanceHashes()

    // Missing agent reference causes compilation error
    func testMissingAgentThrows()

    // Invalid workflow (broken transitions) causes compilation error
    func testInvalidWorkflowThrows()

    // Compact normalization produces valid WorkflowDefinition
    func testCompactNormalization()

    // Compact agent aliases resolve to canonical IDs
    func testCompactAliasResolution()

    // Unresolvable compact alias causes error
    func testCompactUnknownAliasThrows()

    // Variables are preserved in RunPlan
    func testVariablesPreserved()

    // Scoring config preserved
    func testScoringPreserved()
}
```

### 12.2 Orchestrator tests

```swift
class OrchestratorTests: XCTestCase {
    // Run a minimal two-state workflow (start -> end)
    func testMinimalWorkflow()

    // Sequential run block executes agents in order
    func testSequentialExecution()

    // Parallel run block executes agents concurrently
    func testParallelExecution()

    // parallel + then executes in correct order
    func testParallelThenExecution()

    // Approval gate pauses execution
    func testApprovalGatePauses()

    // Approving gate continues execution
    func testApprovalGrantedContinues()

    // Rejecting gate cancels run
    func testApprovalRejectedCancels()

    // run_after_approval executes after approval granted
    func testRunAfterApproval()

    // Loop increments counter
    func testLoopCounter()

    // Loop budget exhausted triggers failure policy
    func testLoopBudgetExhausted()

    // Agent failure triggers failure policy
    func testAgentFailurePolicy()

    // Run cancellation stops execution
    func testCancellation()

    // Full canonical workflow runs to completion with simulated executor
    func testFullCanonicalWorkflowEndToEnd()
}
```

### 12.3 Transition evaluator tests

```swift
class TransitionEvaluatorTests: XCTestCase {
    // always condition returns true
    func testAlwaysTrue()

    // artifactExists returns true when artifact present
    func testArtifactExists()

    // artifactExists returns false when artifact missing
    func testArtifactMissing()

    // Numeric comparison with variable substitution
    func testNumericComparison()

    // Complex expression with and/or
    func testComplexExpression()

    // Field access on JSON artifact
    func testArtifactFieldAccess()

    // in operator for string containment
    func testInOperator()
}
```

### 12.4 Artifact manager tests

```swift
class ArtifactManagerTests: XCTestCase {
    // Store and retrieve artifact
    func testStoreAndRetrieve()

    // Artifact path follows convention
    func testArtifactPathConvention()

    // Checksum is computed correctly
    func testChecksum()

    // Resolve inputs finds latest artifact
    func testResolveInputs()

    // Missing required input returns nil
    func testMissingInput()

    // Multiple attempts create separate artifacts
    func testMultipleAttempts()
}
```

### 12.5 Resume manager tests

```swift
class ResumeManagerTests: XCTestCase {
    // Detect interrupted running run
    func testDetectInterruptedRun()

    // Safe stage auto-resumes
    func testSafeStageAutoResume()

    // Approval gate restores to waitingApproval
    func testApprovalGateRestore()

    // Side-effect stage blocks
    func testSideEffectStageBlocks()

    // Drift detected sets run to blocked
    func testDriftDetection()

    // No interrupted runs returns empty
    func testNoInterruptedRuns()
}
```

### 12.6 Simulated executor tests

```swift
class SimulatedAgentExecutorTests: XCTestCase {
    // Produces output artifacts for all expected outputs
    func testProducesExpectedOutputs()

    // Configured failing agent produces failure
    func testConfiguredFailure()

    // Cost is recorded correctly
    func testCostRecording()

    // Output contract template is structurally valid
    func testOutputContractValidity()
}
```

### 12.7 End-to-end integration test

```swift
class EndToEndTests: XCTestCase {
    // Create idea -> compile -> run full canonical workflow with simulated executor
    // Verify: all 12 states visited, all agents called, artifacts produced,
    // approval gates paused correctly, run completes with status .completed
    func testFullPipelineWithSimulatedExecutor()
}
```

### 12.8 Workspace isolation tests

```swift
class WorkspaceIsolationTests: XCTestCase {
    // Two concurrent runs get distinct workspaceRoot paths
    func testConcurrentRunsHaveDistinctWorkspaces()

    // ArtifactStorage.write rejects paths outside artifactRoot
    func testWriteOutsideArtifactRootThrows()

    // ArtifactStorage.read rejects paths outside workspaceRoot
    func testReadOutsideWorkspaceRootThrows()

    // Parallel fan-out agents produce artifacts in separate stage.iteration/agent dirs
    func testParallelFanoutNoPathCollisions()

    // RunWorkspace directories are created on disk at provisioning time
    func testWorkspaceDirectoriesCreated()
}
```

### 12.9 UI tests

```swift
class ExecutionUITests: XCTestCase {
    // Start run flow shows compilation summary
    func testStartRunSheet()

    // Run progress view shows stages
    func testRunProgressView()

    // Approval gate modal appears and blocks
    func testApprovalGateModal()

    // Approve continues execution
    func testApproveAction()

    // Reject cancels run
    func testRejectAction()
}
```

---

## 13. File structure

```
Chainworks Forge/
  Models/              (from Proposal 001 — two files changed)
    Idea.swift                       (unchanged)
    Run.swift                        ← CHANGED: add workspaceRoot, artifactRoot, planCompilerVersion
    StageExecution.swift             (unchanged)
    AgentExecution.swift             (unchanged)
    Approval.swift                   (unchanged)
    Artifact.swift                   (unchanged)
    RunRepository.swift              ← CHANGED: add createRunFromPlan() method

  DSL/                 (from Proposal 001 — one addition)
    AgentCatalog.swift
    WorkflowDefinition.swift
    CompactWorkflowDefinition.swift
    YAMLParser.swift
    YAMLValidator.swift
    CompactWorkflowValidator.swift
    DefinitionHasher.swift
    CompactNormalizer.swift          ← NEW

  Engine/              ← NEW directory
    RunPlan.swift                    ← RunPlan, ExecutableState, etc.
    RunPlanCompiler.swift            ← Two-phase compilation pipeline
    ExecutionService.swift           ← App-scoped execution owner
    WorkflowOrchestrator.swift       ← Per-run state machine driver
    TransitionEvaluator.swift        ← Condition evaluation
    AgentExecutor.swift              ← Protocol definition
    SimulatedAgentExecutor.swift     ← Mock executor
    OutputContractTemplates.swift    ← Template generators
    ArtifactStorage.swift            ← Background disk I/O (nonisolated)
    ArtifactManager.swift            ← MainActor metadata adapter
    ResumeManager.swift              ← Resume detection and recovery

  Views/               (from Proposal 001 — upgraded + additions)
    IdeaListView.swift               (unchanged)
    IdeaDetailView.swift             ← UPGRADED with run lifecycle
    AgentCatalogView.swift           (unchanged)
    WorkflowInspectorView.swift      (unchanged)
    StartRunSheet.swift              ← NEW
    RunProgressView.swift            ← NEW
    ApprovalGateView.swift           ← NEW
    StageDetailView.swift            ← NEW
    ArtifactInspectorView.swift      ← NEW

  ContentView.swift                  (add navigation to run views + global approval sheet)
  Chainworks_ForgeApp.swift          ← UPGRADED: create ExecutionService, inject into environment, trigger resume on launch

Chainworks ForgeTests/
  Fixtures/                          (unchanged)
  Chainworks_ForgeTests.swift        (unchanged — Proposal 001 tests)
  RunPlanCompilerTests.swift         ← NEW
  OrchestratorTests.swift            ← NEW
  TransitionEvaluatorTests.swift     ← NEW
  ArtifactManagerTests.swift         ← NEW
  ResumeManagerTests.swift           ← NEW
  SimulatedAgentExecutorTests.swift  ← NEW
  EndToEndTests.swift                ← NEW

Chainworks ForgeUITests/
  Chainworks_ForgeUITests.swift      ← UPGRADED with execution UI tests
```

---

## 14. Acceptance criteria

### RunPlan Compiler
- [ ] Canonical `workflow.yaml` + `agents.yaml` compile into a valid `RunPlan` with 12 states and 13 resolved agents
- [ ] All agent references in run blocks resolve to catalog entries with correct backend profiles
- [ ] Provenance hashes on `RunPlan` match `DefinitionHasher` output
- [ ] `CompactNormalizer` converts `proposal-to-release.yaml` into a valid `WorkflowDefinition`
- [ ] Compact agent aliases resolve to canonical catalog IDs
- [ ] Compilation rejects workflows with validation errors, missing agents, or broken transitions

### Orchestrator
- [ ] Orchestrator drives a minimal workflow (start -> end) to completion
- [ ] Sequential run blocks execute agents in declaration order
- [ ] Parallel run blocks execute agents concurrently (verified by timing)
- [ ] `parallel` + `then` blocks execute in correct two-phase order
- [ ] Approval gates pause execution and set `Run.status = .waitingApproval`
- [ ] Granting approval continues execution past the gate
- [ ] Rejecting approval cancels the run
- [ ] `run_after_approval` blocks execute only after approval is granted
- [ ] Loop counters increment correctly and respect budget limits
- [ ] Agent failures trigger the configured failure policy
- [ ] Full canonical workflow completes with simulated executor

### Transition Evaluation
- [ ] `when: 'true'` always fires
- [ ] `when: exists('artifact_name')` checks artifact presence
- [ ] Numeric comparisons with variable substitution evaluate correctly
- [ ] Complex expressions with `and`/`or` evaluate correctly

### Artifact Manager
- [ ] Artifacts stored on disk follow `{artifactRoot}/{stageID}.{iteration}/{agentID}/{attemptNumber}/{name}` convention (`name` includes extension; no extra runID nesting)
- [ ] Artifacts recorded in SwiftData with correct metadata and checksum
- [ ] Input resolution finds the latest artifact by name within a run
- [ ] Multiple attempts create separate artifacts without overwriting
- [ ] Executor returns raw `Data`; only `ArtifactManager` writes to disk (single owner)
- [ ] `ArtifactFormat.detect` uses `ArtifactContract.format` when contract exists (not hardcoded "contract = JSON")

### Workspace Isolation
- [ ] Two concurrent runs get distinct `workspaceRoot` paths
- [ ] `ArtifactStorage` rejects write paths outside `artifactRoot`
- [ ] `ArtifactStorage` rejects read paths outside `workspaceRoot`
- [ ] Parallel fan-out agents produce no path collisions

### Cost & Resume
- [ ] Agent execution costs aggregate into `Run.totalCostCents`
- [ ] `StageExecution` and `AgentExecution` are created **lazily** (at stage entry / agent scheduling, not at run creation)
- [ ] `Run.currentStageID` remains derived from `stageExecutions` (Proposal 001 invariant preserved)
- [ ] Interrupted running runs detected on launch
- [ ] Compiler version mismatch on resume blocks the run (not silently recompiles)
- [ ] Safe stages auto-resume from last completed point
- [ ] Approval gate stages restore to `waitingApproval`
- [ ] Side-effect stages block for human review
- [ ] Drift detection sets `driftDetectedAt` when YAML hashes differ

### Execution UI
- [ ] IdeaDetailView shows runs and [Start New Run] button
- [ ] Start Run sheet shows compilation summary and validation status
- [ ] RunProgressView displays real-time stage chain with agent statuses
- [ ] Approval inbox appears on root ContentView when `pendingApprovals` is non-empty; selecting an entry opens ApprovalGateView as a detail sheet with context and approve/reject buttons
- [ ] StageDetailView shows agent executions and produced artifacts
- [ ] ArtifactInspectorView renders markdown and JSON content

### General
- [ ] App compiles and launches on macOS
- [ ] All Proposal 001 tests still pass (no regressions)
- [ ] All new unit tests pass
- [ ] `xcodebuild build && xcodebuild test` green

### Product checkpoint (PROD-PA-002)
- [ ] **Leading metric:** engineer can create idea -> start run -> observe full canonical workflow execution through all 12 states with simulated executor -> approve at 3 gates -> see run complete with artifacts. Total automated test time < 120 seconds.
- [ ] **Guardrail metric:** `xcodebuild build && xcodebuild test` green; all Proposal 001 + 002 tests pass; no regressions.
- [ ] **Go/no-go for Proposal 003:** advance only after (a) full canonical workflow executes end-to-end with simulated executor, (b) approval gates correctly block and resume, (c) artifacts are stored and retrievable, and (d) resume detects interrupted runs.

---

## 15. What's NOT in scope

| Exclusion | Reason | Target |
|---|---|---|
| Goose REST/SSE adapter | Separate provider layer | Proposal 003 |
| Real LLM provider calls (Codex, Claude Code) | Depends on Goose adapter | Proposal 003 |
| Multi-provider routing | Depends on provider adapters | Proposal 003 |
| Worktree creation and management | Runtime concern, depends on provider | Proposal 003/004 |
| Drift-review UI (three-button decision modal) | Model exists, UI deferred | Proposal 003/004 |
| Completed run report generation | Depends on real artifacts | Proposal 003 |
| `.gooseignore` and permission enforcement | Provider-layer concern | Proposal 003 |
| Temporal / Rust backend migration | Future architecture | Phase 3 |
| Expression language full specification | Minimal evaluator sufficient for canonical workflow | Proposal 003+ |

---

## 16. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| ARCH-012 | Compact alias resolution is deterministic and explicit — no guessing | Strategy 1: hyphens → underscores. Strategy 2: explicit `agent_aliases` map in compact YAML. No automatic abbreviation generation. |
| ARCH-013 | RunPlan is immutable after compilation | Matches RunPlanSnapshot invariant from Proposal 001; resume always uses compiled plan |
| ARCH-014 | Orchestrator is `@MainActor` for SwiftData; agent execution is `nonisolated` for parallelism | SwiftData requires MainActor. Parallel agents run off-MainActor via `@Sendable` closures; results are marshalled back to MainActor for state updates. This avoids serializing parallel fan-out phases. |
| ARCH-021 | Compiler is two-phase: `previewCompile` (no persistence) + `createRun` (irreversible) | Prevents orphan runs on cancel; allows Start Run sheet to preview/validate without side effects; resume uses `rebuildPlanFromSnapshot` which never creates a new Run. |
| ARCH-022 | `ExecutionService` is app-scoped, owns all orchestrators, injected via SwiftUI environment | Execution must outlive any view. Approval gates surface globally via `pendingApprovals` inbox on root ContentView. Resume triggers at app startup. Views observe, they don't own execution state. |
| ARCH-023 | Artifact I/O split: `ArtifactStorage` (nonisolated disk I/O) + `ArtifactManager` (@MainActor metadata) | Disk writes must not block the main thread. File operations run in any context; only SwiftData metadata commits require MainActor. |
| ARCH-024 | `RunPlan` carries no run-scoped identity (`runID`, `ideaID`) | Preview compilation must be cancellable without side effects. Identity lives on the persisted `Run` model. Orchestrator works with `(Run, RunPlan, RunWorkspace)` tuples. |
| ARCH-025 | `RunWorkspace` with explicit `workspaceRoot` is frozen at run creation | Per `workspace-isolation-risk.md`: "No agent operates in an implicit environment." Every execution context carries explicit `workspaceRoot`. Worktree provisioning is Proposal 003; Proposal 002 sets `workspaceRoot` to a run-scoped subdirectory of the app's data container. |
| ARCH-026 | `artifactRoot` is already run-scoped; no extra runID nesting below it. Path includes `{stageID}.{iteration}` to handle loops. | Prevents double-runID in paths. Iteration in path prevents collision when same stage runs multiple times in a loop. |
| ARCH-027 | `StageExecution` and `AgentExecution` created lazily, at stage entry / agent scheduling | Proposal 001's non-optional `startedAt` fields make pre-creation of pending records invalid. Lazy creation keeps model contract intact. `Run.currentStageID` remains derived (Proposal 001 invariant). |
| ARCH-028 | `pendingApprovals` is a collection (`[UUID: ApprovalRequest]`), not a singleton | PS allows one active run per idea, not one per app. Multiple ideas can hit gates simultaneously. UI shows approval inbox, not a single blocking modal. |
| ARCH-029 | `planCompilerVersion` persisted on Run; resume blocks on version mismatch | Protects against compiler semantic drift. If normalizer/validator/expression-parsing changes between run creation and resume, the old run is blocked rather than silently recompiled with different semantics. |
| ARCH-030 | Executor returns `Data`, `ArtifactManager` is sole disk writer | Eliminates fan-out file races. Executor produces output data in memory; `ArtifactStorage` owns all file path logic. No executor ever writes to the filesystem directly. |
| ARCH-031 | Expression evaluator supports only the canonical workflow's condition patterns | No general-purpose expression language. Only: `artifact.field {==,>,>=} value/vars.X`, `and`, `or`, `exists()`, `approval.granted`. Deferred: `in`, `not`, nested fields, arithmetic. |
| ARCH-015 | AgentExecutor is a protocol, not a concrete type | Enables simulated (testing) and real (Goose) executors without engine changes |
| ARCH-016 | Transition evaluator supports minimal expression subset | Full expression language is over-engineering for the canonical workflow; `exists()`, numeric comparisons, and `and`/`or` cover all 12 states |
| ARCH-017 | Artifact Manager stores files on local disk, metadata in SwiftData | Matches PS section 3.1 requirement: "persist only metadata, indexes, statuses, and artifact references in SwiftData" |
| ARCH-018 | Simulated executor produces deterministic outputs | Enables reproducible end-to-end tests without network or LLM dependencies |
| ARCH-019 | Side-effect stages identified by `requires_human_approval` or permission profile | Prevents silent auto-resume of push/publish stages per PS section 4.5 |
| ARCH-020 | Cost is Int64 cents, aggregated by summation | Matches Proposal 001 model; real provider cost mapping is Proposal 003 scope |

---

## 17. Execution plan

| Day | Deliverable |
|---|---|
| Day 1 | RunPlan types + RunPlanCompiler + CompactNormalizer + compiler tests |
| Day 2 | TransitionEvaluator + expression parser + evaluator tests |
| Day 3 | AgentExecutor protocol + SimulatedAgentExecutor + OutputContractTemplates + executor tests |
| Day 4 | ArtifactManager + disk storage + SwiftData metadata + manager tests |
| Day 5 | WorkflowOrchestrator + state machine loop + orchestrator tests |
| Day 6 | ResumeManager + drift detection + resume tests |
| Day 7 | IdeaDetailView upgrade + StartRunSheet + RunProgressView + ApprovalGateView |
| Day 8 | StageDetailView + ArtifactInspectorView + execution UI tests |
| Day 9 | End-to-end integration test (full canonical workflow) + product checkpoint |
| Day 10 | Polish, edge cases, documentation, final test pass |

---

## 18. What this proposal enables

```
                    ┌─────────────────────────┐
                    │   Proposal 001          │
                    │   Domain Model          │
                    │   + YAML Parser         │
                    └────────┬────────────────┘
                             │
                    ┌────────▼────────────────┐
                    │   Proposal 002          │
                    │   Execution Engine      │
                    │   + Approval Flow       │
                    └────────┬────────────────┘
                             │
              ┌──────────────┼──────────────────┐
              │              │                  │
    ┌─────────▼──────┐ ┌────▼──────────┐ ┌─────▼──────────┐
    │ Proposal 003   │ │ Proposal 004  │ │ Proposal 005   │
    │ Goose + Real   │ │ Worktree +    │ │ Reports +      │
    │ Providers      │ │ Drift Review  │ │ Observability   │
    └────────────────┘ └───────────────┘ └────────────────┘
```

After Proposal 002, the app can:
- Compile YAML workflows into executable plans
- Drive the full 12-state canonical workflow end-to-end
- Pause at approval gates and wait for engineer decisions
- Store artifacts on disk with SwiftData tracking
- Track costs across agents and stages
- Detect and handle interrupted runs on resume
- Show real-time execution progress in the UI

What's left for the app to be **production-useful** is real provider integration (Proposal 003), which plugs into the `AgentExecutor` protocol established here.

---

## 19. Risk assessment

| Risk | Impact | Mitigation |
|---|---|---|
| Expression evaluator complexity grows | Delays, bugs | Minimal subset covers canonical workflow; defer full language |
| SwiftData concurrent access during parallel execution | Data corruption | All SwiftData access through `@MainActor`; agent execution is off-MainActor via `@Sendable`, results marshalled back |
| Artifact disk I/O blocks UI | Perceived lag | `ArtifactStorage` is `nonisolated` — disk writes run off-MainActor. Only SwiftData metadata commits use `@MainActor` via `ArtifactManager` (ARCH-023). |
| Approval gate not visible (app backgrounded) | Missed approvals | `ExecutionService.pendingApprovals` inbox surfaces globally on root ContentView. Future: local notification. Gate persists in SwiftData, restored on foreground/relaunch. |
| Execution stops when view navigates away | Lost run progress | `ExecutionService` is app-scoped (ARCH-022) — execution continues regardless of which view is active. Views observe, they don't own orchestrators. |
| Start Run creates orphan run on cancel | Wasted single-active-run slot | Two-phase compiler (ARCH-021): `previewCompile` has no persistence; `createRun` only after engineer confirms. |
| Simulated executor doesn't surface real-world edge cases | False confidence | Simulated executor is for testing the engine, not the agents. Real provider testing is Proposal 003 |
