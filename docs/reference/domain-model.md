# Domain Model

This document describes the SwiftData persistence layer of Chainworks Forge. All six models live in `Chainworks Forge/Models/` and are registered in the shared `ModelContainer` at app launch.

## Entity relationship diagram

```
┌──────────┐     1    ┌──────────┐     *    ┌──────────────┐
│   Idea   │────────▶│   Run    │────────▶│    Stage     │
│          │         │          │         │  Execution   │
└──────────┘         └────┬─────┘         └──────┬───────┘
                          │                      │
                          │ *                    │ *
                     ┌────▼─────┐          ┌─────▼───────┐
                     │ Approval │          │   Agent     │
                     │          │          │  Execution  │
                     └──────────┘          └─────┬───────┘
                                                 │ *
                                           ┌─────▼───────┐
                                           │  Artifact   │
                                           └─────────────┘
```

## Models

### `Idea`

**File:** `Models/Idea.swift`

The user's starting point. An idea is a text description of what the engineer wants to build, optionally accompanied by an attachment file.

| Field | Type | Notes |
|---|---|---|
| `id` | `UUID` | Unique identifier (`.unique` attribute) |
| `title` | `String` | Short description |
| `body` | `String` | Full text of the idea |
| `attachmentPath` | `String?` | Optional path to a file on disk |
| `createdAt` | `Date` | Creation timestamp |
| `status` | `IdeaStatus` | `draft` · `active` · `completed` · `failed` |
| `runs` | `[Run]` | Cascade-deleted children |

`Idea` intentionally does **not** carry a `workflowID`. Workflow identity lives on `Run` because each run must capture the exact workflow and catalog revision it was started with. This makes reruns unambiguous and resume-safe even when YAML changes between runs.

### `Run`

**File:** `Models/Run.swift`

One execution instance of a workflow for one idea. Stores both mutable execution state and an immutable provenance snapshot.

#### Mutable execution fields

| Field | Type | Notes |
|---|---|---|
| `id` | `UUID` | Unique identifier |
| `startedAt` | `Date` | When the run was created |
| `completedAt` | `Date?` | When the run finished |
| `status` | `RunStatus` | Current lifecycle state |
| `loopCounters` | `[String: Int]` | e.g. `proposal_revision_cycles: 2` |
| `totalCostCents` | `Int64?` | Total cost in minor currency units (cents); `$12.34` = `1234` |

#### Immutable provenance snapshot (RunPlanSnapshot)

These fields are `private(set)` and frozen at run creation. They record the exact YAML definitions the run was started with.

| Field | Type | Notes |
|---|---|---|
| `workflowID` | `String` | Workflow identifier from the catalog |
| `workflowTitle` | `String` | Human-readable title at creation time |
| `workflowSnapshotHash` | `String` | SHA-256 of the canonical JSON-serialized `WorkflowDefinition` |
| `catalogSnapshotHash` | `String` | SHA-256 of the canonical JSON-serialized `AgentCatalog` |
| `workflowSourcePath` | `String` | Path to `workflow.yaml` used at creation |
| `catalogSourcePath` | `String` | Path to `agents.yaml` used at creation |
| `workflowSnapshotJSON` | `Data` | Full serialized `WorkflowDefinition` |
| `catalogSnapshotJSON` | `Data` | Full serialized `AgentCatalog` |

#### Drift detection fields

| Field | Type | Notes |
|---|---|---|
| `driftDetectedAt` | `Date?` | When drift was detected (`nil` = no drift) |
| `driftDetails` | `String?` | Human-readable description of what changed |
| `driftDecision` | `DriftDecision?` | Engineer's decision after drift |

#### Derived properties

`currentStageID` is now **cursor-first**. It is a computed property that first resolves continuation from durable transition cursor metadata (and only falls back to `stageExecutions` ordering when cursor metadata is absent). This preserves resume truth when mixed or stale stage rows exist and makes continuation deterministic across resume/restart paths.

#### Relationships

| Relationship | Target | Delete Rule |
|---|---|---|
| `idea` | `Idea` | Inverse of `Idea.runs` |
| `stageExecutions` | `[StageExecution]` | Cascade |
| `approvals` | `[Approval]` | Cascade |

### `StageExecution`

**File:** `Models/StageExecution.swift`

Tracks the execution of a single stage (state) in the workflow.

| Field | Type | Notes |
|---|---|---|
| `id` | `UUID` | Unique identifier |
| `stageID` | `String` | State identifier from workflow YAML |
| `label` | `String` | Human-readable label |
| `startedAt` | `Date` | When execution began |
| `completedAt` | `Date?` | When execution finished |
| `status` | `StageStatus` | Current lifecycle state |
| `iteration` | `Int` | Loop iteration number (1-based) |
| `attemptNumber` | `Int` | Retry attempt number |

#### Relationships

| Relationship | Target | Delete Rule |
|---|---|---|
| `run` | `Run` | Inverse of `Run.stageExecutions` |
| `agentExecutions` | `[AgentExecution]` | Cascade |

### `AgentExecution`

**File:** `Models/AgentExecution.swift`

Tracks a single agent's work within a stage.

| Field | Type | Notes |
|---|---|---|
| `id` | `UUID` | Unique identifier |
| `agentID` | `String` | Agent identifier from the catalog |
| `agentTitle` | `String` | Human-readable agent name |
| `taskName` | `String` | Task from workflow YAML |
| `startedAt` | `Date` | When agent started |
| `completedAt` | `Date?` | When agent finished |
| `status` | `AgentStatus` | Current lifecycle state |
| `provider` | `String` | ACP-backed provider identifier such as `claude_acp`, `codex_acp`, or `gemini_acp` |
| `effort` | `String` | `low` · `medium` · `high` · `critical` |
| `costCents` | `Int64?` | Cost in minor units; `$0.73` = `73` |
| `logSnippet` | `String?` | Last N lines of log for quick preview |
| `runtimeSessionID` | `String?` | Runtime session tracking |

#### Relationships

| Relationship | Target | Delete Rule |
|---|---|---|
| `stageExecution` | `StageExecution` | Inverse of `StageExecution.agentExecutions` |
| `artifacts` | `[Artifact]` | Cascade |

### `Approval`

**File:** `Models/Approval.swift`

Records an approval gate decision within a run.

| Field | Type | Notes |
|---|---|---|
| `id` | `UUID` | Unique identifier |
| `stageID` | `String` | Stage where approval was requested |
| `requestedAt` | `Date` | When approval was requested |
| `decidedAt` | `Date?` | When the engineer decided |
| `decision` | `ApprovalDecision` | Current approval state |
| `comment` | `String?` | Optional engineer comment |
| `expiresAt` | `Date?` | Optional expiration window |

#### Relationships

| Relationship | Target | Delete Rule |
|---|---|---|
| `run` | `Run` | Inverse of `Run.approvals` |

### `Artifact`

**File:** `Models/Artifact.swift`

Metadata for a durable output produced by an agent. Content lives on disk; SwiftData stores only the metadata. Artifacts are immutable per stage attempt.

| Field | Type | Notes |
|---|---|---|
| `id` | `UUID` | Unique identifier |
| `name` | `String` | e.g. `proposal_review_po`, `audit_report` |
| `contractID` | `String` | e.g. `proposal_review_v1` |
| `format` | `ArtifactFormat` | `json` · `markdown` · `diff` · `report` |
| `filePath` | `String` | Path to content on disk |
| `checksumSHA256` | `String?` | Content integrity check |
| `createdAt` | `Date` | Creation timestamp |
| `sizeBytes` | `Int64?` | File size |
| `runID` | `UUID` | Which run produced it |
| `stageID` | `String` | Which stage |
| `agentID` | `String` | Which agent |
| `provider` | `String` | Which provider was used |
| `model` | `String?` | Which model |
| `effort` | `String?` | Which effort level |
| `attemptNumber` | `Int` | Which attempt (immutable per attempt) |

#### Relationships

| Relationship | Target | Delete Rule |
|---|---|---|
| `agentExecution` | `AgentExecution` | Inverse of `AgentExecution.artifacts` |

## Status enums

### `RunStatus` (8 states)

| Value | Meaning |
|---|---|
| `pending` | Created, not yet compiled |
| `ready` | RunPlanSnapshot compiled, waiting to start |
| `running` | Workflow actively executing |
| `waitingApproval` | Paused at approval gate |
| `blocked` | Drift detected or external blocker |
| `completed` | Finished successfully |
| `failed` | Finished with error |
| `cancelled` | Stopped by user or orchestrator |

### `StageStatus` (8 states)

| Value | Meaning |
|---|---|
| `pending` | Not yet reached |
| `ready` | Dependencies met, waiting to execute |
| `running` | Agents executing |
| `waitingApproval` | Approval gate active |
| `blocked` | External blocker or side-effect hold |
| `completed` | Finished successfully |
| `failed` | Finished with error |
| `skipped` | Stage logic determined stage not needed |

### `AgentStatus` (7 states)

| Value | Meaning |
|---|---|
| `pending` | Not yet scheduled |
| `ready` | Dependencies met, waiting for provider |
| `running` | Actively executing via the selected ACP runtime |
| `completed` | Finished successfully |
| `failed` | Finished with error |
| `cancelled` | Stopped by user or orchestrator |
| `skipped` | Stage logic determined agent not needed |

### `ApprovalDecision` (5 states)

| Value | Meaning |
|---|---|
| `pending` | Approval not yet requested |
| `requested` | Workflow asked for approval, waiting for engineer |
| `granted` | Engineer approved |
| `rejected` | Engineer rejected |
| `expired` | Approval window passed without decision |

### `DriftDecision` (3 values)

| Value | Meaning |
|---|---|
| `continueWithOriginal` | Resume using the snapshotted workflow/catalog |
| `restartWithCurrent` | Abandon this run, start new run with current YAML |
| `cancelled` | Stop the run entirely |

### `IdeaStatus` (4 values)

`draft` · `active` · `completed` · `failed`

### `ArtifactFormat` (4 values)

`json` · `markdown` · `diff` · `report`

## RunRepository

**File:** `Models/RunRepository.swift`

`RunRepository` is the **single approved entry point** for creating `Run` instances. It enforces the single-active-run-per-idea invariant.

### API

```swift
@MainActor
struct RunRepository {
    init(context: ModelContext)

    /// Atomically checks for active runs, then creates and inserts a new run.
    /// Throws RunRepositoryError.activeRunExists if an active run already exists.
    func createRun(
        for idea: Idea,
        workflow: WorkflowDefinition,
        catalog: AgentCatalog,
        workflowSourcePath: String,
        catalogSourcePath: String
    ) throws -> Run

    /// Returns the current active run for the idea, or nil.
    func activeRun(for idea: Idea) -> Run?
}
```

### Single-active-run invariant

A run is considered **active** if its status is one of: `pending`, `ready`, `running`, `waitingApproval`, `blocked`. Only one active run per idea is allowed.

### Enforcement

The app is a single Xcode target, so Swift `internal` access control does not confine `Run.init` to `RunRepository`. Enforcement is automated:

1. **`@MainActor` serialization** — all `RunRepository` methods run on the main serial executor. Check + insert happen in one synchronous block, eliminating TOCTOU races.
2. **Automated codebase scan test** (`testNoDirectRunConstruction`) — recursive walk of all `.swift` files in the app source tree; flags any `Run(` outside exempted files.
3. **CI pre-commit grep guard** — blocks direct `Run` insertion at commit time.

## Provenance and drift detection

### How provenance works

When a run is created through `RunRepository`:

1. The orchestrator serializes `WorkflowDefinition` and `AgentCatalog` into canonical JSON using `DefinitionHasher.canonicalEncoder` (`.sortedKeys` + `.withoutEscapingSlashes` + `.iso8601`).
2. Full snapshots are stored in `workflowSnapshotJSON` / `catalogSnapshotJSON`.
3. SHA-256 hashes are computed from each snapshot and stored in `workflowSnapshotHash` / `catalogSnapshotHash`.

### How drift detection works

On resume after an app restart:

1. The orchestrator loads the current YAML files and computes their hashes.
2. Compares with stored hashes on the run.
3. **Hashes match** — resume normally.
4. **Hashes differ** — set `status = .blocked`, populate `driftDetails`.
5. Engineer sees a drift-review UI and chooses a `DriftDecision`.
6. `continueWithOriginal` — orchestrator deserializes workflow/catalog from the stored snapshot JSON.
7. `restartWithCurrent` — a new run is created with the current YAML files.

### Why full snapshots, not just hashes

Hashes detect drift but cannot reconstruct the original definitions. Snapshots are stored as `Data` (JSON blob) so that `continueWithOriginal` can deserialize the exact workflow and catalog the run was started with, even if the YAML files on disk have changed.

## Cost tracking

All costs are stored as `Int64` minor currency units (cents) to avoid floating-point precision drift in aggregation. Rounding to display currency happens only at the presentation layer.

- `AgentExecution.costCents` — cost of a single agent invocation
- `Run.totalCostCents` — aggregated total for the run

## ModelContainer schema

Registered in `Chainworks_ForgeApp.swift`:

```swift
let schema = Schema([
    Idea.self,
    Run.self,
    StageExecution.self,
    AgentExecution.self,
    Approval.self,
    Artifact.self,
])
```
