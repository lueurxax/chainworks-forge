# Workflow Execution Engine

## Purpose

The Workflow Execution Engine compiles YAML workflow definitions and agent catalogs
into executable run plans, then drives them through a state machine to completion.
It handles sequential and parallel agent execution, human approval gates, bounded
loops, artifact persistence, transition evaluation, and safe resume after app
interruption.

All core engine code lives under `Chainworks Forge/Engine/` (SwiftUI client) or
`control-plane/crates/engine/` (Rust daemon).

**Thin UI Boundary:**
The production macOS UI is a **thin client** focused on read-side truth
and governed human gates. While the Swift engine remains implemented for
parity, the governed UI is prohibited from calling most mutation paths in
`ExecutionService` or `WorkflowOrchestrator` directly. Start, Cancel, and
other operational commands remain in external CLI/MCP workflows. Approval resolution
is the only governed mutation path allowed in the macOS UI via GraphQL.

**Rust Daemon Implementation:**
The Rust control-plane daemon implements the same state machine and transition
semantics while adding robust capacity-aware scheduling, scheduler fairness,
executor backpressure, evidence spooling, and host interruption recovery to handle
concurrent runs on a single host. See [rust-control-plane.md](rust-control-plane.md)
for details on the daemon's scheduler, write serialization, and recovery logic.

Related stable docs:

- [skill-resolution-and-runtime-integration.md](skill-resolution-and-runtime-integration.md)
- [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md)
- [acp-runtime-transport.md](acp-runtime-transport.md)

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
3. Resolve skills and role-specific injected content for each agent.
4. Freeze MCP-profile intent on resolved agents.
5. Parse transition `when` clauses into `TransitionCondition` variants.
6. Resolve loop budgets -- `vars.*` references substituted at compile time.
7. Build `ExecutableState` instances with run blocks, transitions, and loops.
8. Compute provenance hashes via `DefinitionHasher`.
9. Assemble and return an in-memory `RunPlan`.

A `previewCompileCompact` variant normalizes compact workflow definitions through
`CompactNormalizer` before delegating to `previewCompile`.

**Phase 2 -- `createRun`** (irreversible):

1. Generate a run UUID.
2. Provision a workspace directory under Application Support
   (`~/Library/Application Support/Chainworks Forge/runs/{runID}/`).
3. Persist a `Run` record via `RunRepository`.

**Resume path -- `rebuildPlanFromSnapshot`**: Decodes frozen JSON snapshots stored
on a `Run` record and re-runs `previewCompile`. Rejects compiler version mismatches.

### Skill and MCP compilation

`RunPlanCompiler` also owns the compile-time resolution that turns catalog declarations into runtime-authoritative agent bindings:

- `skill_ref` / `skill_role` -> frozen `ResolvedSkill`
- `AgentEntry.backend_profile` / `backend_profile.mcp` -> `ResolvedAgent.backend_profile_id` plus frozen `ResolvedAgent.requested_mcp_server_ids`
- `backend_profile` / `runtime_profile` -> transport-ready provider binding truth

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
run one-by-one; `.parallel` tasks run via `withTaskGroup`; `.dynamic_parallel`
tasks read a typed selector artifact (e.g. `AgentSelectionPlanV1`) and materialize
only the selected agent executions from compiled candidate bindings. Each task
creates an `AgentExecution`, gathers input artifacts, builds an `ExecutionContext`,
calls the executor, then persists outputs through `ArtifactManager`.

### System Tasks and Routing (`proposal_review_router.rs`)

The engine supports first-class `SystemTask` types that execute without provider
invocation. The `proposal_review_router` uses `executor_mode: system.routing` to
perform deterministic reviewer selection over proposal evidence and catalog
metadata.

**Deterministic Routing (P060):**
Replaces fixed proposal-review fan-out with a scoring-based model that selects
2-5 specialists from an expanded catalog.
- **Scoring**: Based on force-includes, stack/surface/risk matches, strong keywords,
  repo signals, and cross-stack dependencies, with an overlap penalty.
- **Specialists**: Launches with seven core specialists (macOS, Apple architecture,
  Rust architecture, reliability, security, API contract, and observability/rollout)
  while cataloging others as disabled until a golden-output gate is passed.
- **Determinism**: The same inputs must produce identical selected order, evidence
  IDs, and plan hashes across Swift and Rust implementations.

**Key Artifacts:**
- **`AgentSelectionPlanV1`** -- the authoritative plan for selected reviewers,
  referencing compiler-owned materialization bindings.
- **`RoutingReceipt`** -- the terminal receipt for every routing outcome, including
  rationale, status, and input snapshot hashes.
- **`SystemExecution`** -- the lifecycle record for the system task, owning the
  task status and timestamps.

**Routing Conflicts and Fallbacks:**
- **Under-specified Selection**: If no specialists qualify, falls back to
  `product_owner` plus `architect` with a caution warning.
- **Mandatory Overflow**: If more than 5 mandatory reviewers match, blocks with a
  `Routing conflict` and requires operator intervention (e.g., cloning with
  overrides).

**Approval flow**: When a state has `approval: required`, the orchestrator pauses,
creates an `Approval` record, and publishes an `ApprovalRequest`. On resolution:
granted resumes execution (including any `runAfterApproval` block); rejected typically
cancels the run, but some workflows define explicit loopback transitions for
`approval.rejected == true` (e.g., looping back to proposal refinement).

**Cancellation**: Sets `isCancelled`, updates run status, stops the loop.

### Agent Executor Protocol (`AgentExecutor.swift`)

```swift
protocol AgentExecutor: Sendable {
    func execute(task: AgentTask, agent: ResolvedAgent,
                 context: ExecutionContext) async throws -> AgentResult
}
```

Executors return `[String: Data]` (in-memory) or write to disk via the discovery settlement pipeline. The `ArtifactManager` is the primary disk writer for in-memory results and the metadata authority for all artifacts (ARCH-030).

The executor path consumes:

- frozen skill truth,
- frozen MCP intent,
- and resolved runtime transport selection.

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

#### RuntimeAgentExecutor (`RuntimeAgentExecutor.swift`)

Live executor using the selected ACP runtime transport. Per-execution flow:

1. Validate workspace boundaries.
2. **Toolchain Cache Mapping**: Prepare isolated toolchain roots (Xcode/Go) based on agent policy and session/run scope. Acquire exclusive per-run lease for Xcode work.
3. Capture pre-prompt metadata for the per-execution baseline.
4. Create an isolated session via `RuntimeSessionBridge`.
5. **Prompt Augmentation (P065)**: if an operator retry instruction is active, the executor renders a reserved engine-owned prompt section (`## Operator Retry Instruction`) before the task text.
6. Stream execution events through `ExecutionEventBridge`.
7. Build receipt and transcript artifacts (`ExecutionReceiptBuilder`).
8. Bounded output discovery: read declared output files and meta-root outputs through the discovery settlement pipeline.
9. Validate required outputs -- missing or rejected (over-cap) outputs fail the stage.

On stream failure, the executor salvages any files the agent already wrote to disk
before the transport closed, governed by the discovery settlement policy.

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

### Worktree Mutation Barrier (P064)

To protect worktrees during orchestrated mutations (like main-sync), the engine uses an exclusive mutation barrier.
- **Barrier Acquisition**: Active sync or repair tasks request an exclusive barrier.
- **Consumer Blocking**: The scheduler prevents new read/write work items from being claimed while the barrier is active for a worktree.
- **Read-only Reviewers**: Review agents that read directly from the implementation worktree must declare `read` access and are subject to barrier blocking.

### Transition Evaluator (`TransitionEvaluator.swift`)
...

Stateless evaluator for transition `when` clauses (ARCH-031). Supports only
canonical patterns:

| Pattern | Example |
|---|---|
| Always | `when: 'true'` |
| Artifact exists | `when: exists('proposal_review_summary')` |
| Approval granted | `when: approval.granted == true` |
| Approval rejected | `when: approval.rejected == true` |
| Comparison | `when: review.score >= vars.min_score` |
| Connectives | `expr and expr`, `expr or expr` |

Value resolution supports `vars.*` (runtime variables), `artifact.field` (artifact
metadata), and literals (int, double, bool, quoted string). Comparison operators:
`==`, `>`, `>=`, `!=`. Unrecognized expressions fail closed (return false).

### Transition Authority Resolver (`WorkflowOrchestrator.swift`)

The Transition Authority Resolver (ARCH-032) enforces the compiled workflow graph
as the sole authority for stage progression.

**Authority Rules:**
- The compiled workflow graph is the only authority for legal next state selection.
- Agent-authored `next_stage`, `next_action`, `run_state.json`, and narrative
  transition hints are treated as **advisory evidence only**.
- A legal declarative transition always takes precedence over a conflicting
  advisory hint.
- An advisory `next_stage` absent from the graph never creates a synthetic state.
- Multiple matched declarative transitions without a tie-break result in a
  blocking conflict.
- Unknown catalog artifact references (`exists(unknown_artifact)`) never evaluate
  to true; they are classified as `invalid_expression` (undeclared) or
  `missing_input` (declared but absent).

### Aggregate Artifact Field Authority

To ensure deterministic evaluation, aggregate artifact fields are classified by
authority (ARCH-035). For example, in `proposal_review_summary_v1`:

- **Transition Authoritative**: `pass`, `blocker_count`, `blocking_issues`,
  `required_changes`. These drive graph transitions.
- **Advisory Only**: `next_action`, `next_stage`. These are recorded as
  advisory evidence but cannot select a graph transition alone.
- **Contradiction Bearing**: `decision`. Used to detect internal aggregate
  inconsistency.

### Candidate Transition Evaluation

Every transition evaluation produces a `CandidateTransitionEvaluation` record
detailing why a transition matched or failed (ARCH-033).

Results include:
- `matched`
- `not_matched`
- `missing_input` (declared artifact absent)
- `invalid_expression` (undeclared artifact or invalid field)
- `evaluation_error`

### Transition Input Dependency Classification

Fail-closed behavior applies to all transition inputs (ARCH-036):
- If a referenced artifact is not declared by the workflow/catalog contract,
  it is `invalid_expression`.
- If declared but absent, it is `missing_input`.
- `exists(unknown_artifact)` never returns true in graph-authoritative evaluation.

#### Workflow Conflict and Advisory Rejection

If graph authority cannot determine a single valid next state, the engine persists
a `WorkflowConflictRecord`:
- `no_declarative_transition_matched`
- `multiple_declarative_transitions_matched_without_tie_break`
- `required_artifact_or_field_missing_for_transition`
- `aggregate_transition_truth_conflicted`
- `workflow_conflict_unverifiable`
- `implementation_handoff_unavailable`

If the graph advances legally despite a conflicting agent hint, the hint is
persisted as a `WorkflowAdvisoryRejectionRecord` for historical truth.

#### Lead Conflict Mediation

The engine provides automated conflict resolution through **Lead Conflict Mediation**.
Eligible conflicts are routed to a system lead for same-run resolution before
falling back to manual intervention.

**Mediation Lifecycle:**
1. **Detection**: A blocking conflict is detected and persisted.
2. **Escalation**: If mediation is enabled and a system lead is resolvable via
   `PhaseBLeadResolver`, the engine creates a `LeadConflictMediationRecord`.
3. **Execution**: The system lead is invoked with the conflict context. The
   execution is owned by the mediation record (`owner_kind: lead_conflict_mediation`).
4. **Settlement**: Lead output must satisfy the lead agent's
   `LeadResolutionContract`; malformed or absent output moves the mediation and
   conflict to `terminal_unverifiable`.
5. **Confirmation**: If the resolution requires operator sign-off, a
   `lead_mediation_confirmation` is created in the separate mediation confirmation
   store and appears in the mixed `approvals.list` inbox.
6. **Resolution**: Once confirmed or auto-settled, the mediation outcome resolves
    the conflict, enabling the orchestrator to advance the transition cursor.

**Compatibility Lead Resolver:**
Lead resolution uses a **versioned JSON compatibility map** (`docs/reference/workflow-conflict-evidence/phase-0-phase-b-lead-resolver.json`) as the sole machine-authoritative source for lead selection until static catalog validation fully owns every executable workflow/catalog pair. This map defines exact matches between workflow/catalog pairs and their designated system lead. Fail-closed rules apply if no match or multiple matches exist.

**Validation and Preflight**:
Mandatory static validation and runtime preflight ensure exactly-one lead
resolution and `LeadResolutionContract` coverage. Failure to resolve a valid
lead results in a `terminal_unverifiable` conflict.

**Observability**:
Workflow-conflict rollout metrics are recorded in durable
`workflow_conflict_metric_events` rows. Lead-validation rollout adds
`phase_c_validation_outcome_total` for lead validation outcomes
(`static_fail`, `preflight_fail`, `legacy_catalog_warning`, `pass`), while
conflict resolution records `workflow_conflict_time_to_resolution_seconds`,
`conflict_reason_to_action_outcome_total`, and
`recovery_action_chosen_total`.

#### Status-based implementation handoff transitions


The implementation completeness and handoff contract uses status-based
transitions for the implementation loop. The `code_writer` exits the implementation
loop when the self-assessment status is `complete`, `handoff_required`, or `blocked`.

#### Implementation closeout readiness transitions

Proposal 077 adds a stricter transition guard for the implementation review stage
(`state_9_implementation_reviewed`). A run may enter manual release (`state_11_manual_release`)
only when `implementation_closeout_readiness_v1.decision == 'enter_manual_release'`.

This guard replaces the simpler `implementation_self_assessment_v2.complete == true`
check. It ensures that proposal-specific gates and implementation audits are satisfied
before a run is presented as release-ready.

If the closeout decision is `return_to_code_refine`, the run moves to
`state_10_implementation_refined`, provided the refine budget is not exhausted.
If the budget is exhausted or a non-code decision is required, the run routes to
`await_operator_decision` or `await_non_code_handoff`.

---

## Resume Manager (`ResumeManager.swift`)
Classifies interrupted runs at app launch (ARCH-029). Three outcomes per run:

- **`.resume`** -- plan rebuilt successfully, no drift, no mid-side-effect
  interruption. Safe to restart.
- **`.needsDecision`** -- drift detected (workflow/catalog source hash mismatch) or
  interrupted during a side-effect stage. Requires user intervention.
- **`.cannotResume`** -- compiler version mismatch or snapshot corruption. Marked
  failed.

### Transition Cursor Authority

Transition completion and cursor update are one atomic settlement unit (ARCH-034).
The run-level transition cursor is the authoritative continuation signal,
anchoring the run at the current state when a blocking conflict exists.

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
| ARCH-032 | Workflow Authority: The compiled graph is the sole authority; agent hints are advisory only. |
| ARCH-033 | Conflict Truth: Blocking graph outcomes must persist as typed `WorkflowConflictRecord`. |
| ARCH-034 | Cursor Authority: Transition settlement and cursor update are atomic; cursor anchors on conflicts. |

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
      │  │AgentExecutor│  (Simulated or ACP)     │
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
| `Engine/RuntimeAgentExecutor.swift` | Live executor via ACP runtime transport |
| `Engine/ArtifactManager.swift` | SwiftData metadata bridge for artifacts |
| `Engine/ArtifactStorage.swift` | Nonisolated disk I/O with path guards |
| `Engine/TransitionEvaluator.swift` | Stateless transition condition evaluator |
| `Engine/ResumeManager.swift` | Interrupted run classification for safe resume |
| `Engine/ExecutionService.swift` | App-scoped orchestrator and approval manager |
| `Models/Artifact.swift` | SwiftData Artifact model and format detection |
