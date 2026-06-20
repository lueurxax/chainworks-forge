# Service Architecture Overview

High-level map of how Chainworks Forge works as a local orchestration service:
what orchestrates work, which entities own truth, and where the system interacts
with the outside world.

## System Boundary

The service has two northbound surfaces: GraphQL for the governed macOS UI and
MCP for operational control by agents, CLI, and automation.

```mermaid
flowchart TB
    subgraph Humans["People and automation"]
        Operator["Operator / engineer"]
        Agents["Agents / CLI / automation"]
    end

    subgraph Clients["Client surfaces"]
        UI["macOS SwiftUI app"]
        MCPClient["MCP clients"]
    end

    subgraph ControlPlane["Rust control-plane daemon"]
        GraphQL["GraphQL\nreads, subscriptions,\napprove/reject"]
        MCP["MCP\ncommands, recovery,\nreports, diagnostics"]
        Core["Workflow orchestration core"]
    end

    subgraph Truth["Local durable truth"]
        DB[("SQLite\ncanonical execution truth")]
        Files["Run workspaces\nartifacts, reports, receipts"]
        Worktrees["Run-owned git worktrees"]
    end

    subgraph World["External world"]
        Providers["AI providers\nClaude, Codex, Gemini, etc."]
        LocalTools["Local tools\nXcode, Go, shell"]
        Repo["Project repo"]
        Release["Git remote / PR / release targets"]
    end

    Operator --> UI
    Agents --> MCPClient

    UI --> GraphQL
    MCPClient --> MCP
    GraphQL --> Core
    MCP --> Core

    DB --> GraphQL
    DB --> MCP
    Core <--> DB
    Core --> Files
    Core --> Worktrees

    Core --> Providers
    Core --> LocalTools
    Worktrees --> Repo
    Core --> Release
```

The Rust control-plane daemon is the orchestrator. The governed macOS UI is not
the orchestrator: it reads GraphQL projections and may only settle approval gates
through `approveApproval` / `rejectApproval`. Non-approval operational commands
belong to MCP, with the narrow exception of the P083 operator GraphQL lifecycle
mutations (provider shutdown, process-absent confirmation, rollback execution,
enforcement-mode changes, and run retry) reserved for explicitly authorized
non-UI operator callers.

Inside the daemon, command handling, scheduling, execution, recovery, artifact
settlement, and projections all converge on SQLite as canonical truth.

```mermaid
flowchart TB
    Northbound["GraphQL + MCP"] --> Auth["Auth + caller policy"]

    Auth --> Queries["Projection queries"]
    Auth --> Commands["Command handler"]

    Commands --> Journal["Command journal"]
    Commands --> Engine["Workflow engine / orchestrator"]

    Engine --> Scheduler["Scheduler + work queue"]
    Engine --> Approvals["Approval gates"]
    Engine --> Recovery["Recovery / reconciliation"]
    Engine --> Artifacts["Artifact settlement"]
    Engine --> Projections["Read-model projections"]
    Engine --> SideEffects["Side-effect ledger"]

    Scheduler --> Executor["Background executor"]
    Executor --> Runtime["ACP runtime adapters"]
    Executor --> Toolchains["Toolchain cache mapping"]

    Queries --> DB[("SQLite")]
    Journal --> DB
    Engine --> DB
    Approvals --> DB
    Recovery --> DB
    Artifacts --> DB
    Projections --> DB
    SideEffects --> DB
```

## Execution Entities

```mermaid
flowchart TD
    Idea["Idea"] --> Workflow["Workflow YAML + agent catalog"]
    Workflow --> Plan["Compiled RunPlan"]
    Plan --> Run["Run"]

    Run --> Stage["StageExecution"]
    Stage --> Agent["AgentExecution"]
    Stage --> SystemTask["System task\nrouting / mediation / checks"]

    Agent --> ProviderSession["Provider session via ACP"]
    ProviderSession --> Output["Declared outputs + discovered files"]

    Output --> Artifact["Artifact records"]
    Output --> Report["Reports / receipts / evidence"]

    Stage --> Approval{"Approval required?"}
    Approval -->|"yes"| Wait["Wait for human approval"]
    Wait -->|"approved"| Continue["Continue transition"]
    Wait -->|"rejected"| Loopback["Refine / loopback / fail policy"]
    Approval -->|"no"| Continue

    Artifact --> Transition["Transition evaluation"]
    Report --> Transition
    Continue --> Transition
    Loopback --> Transition

    Transition -->|"next state"| Stage
    Transition -->|"done"| Delivery["Delivery / manual release / sign-off"]
```

## Core Entities

| Entity | Role |
|---|---|
| `Idea` | Operator-owned work request that starts the process. |
| `Workflow YAML` | Declarative workflow topology, stages, transitions, loops, and approval gates. |
| `Agent catalog` | Agent definitions, provider bindings, output contracts, and runtime policy. |
| `RunPlan` | Compiled immutable execution topology for a run. |
| `Run` | Durable execution aggregate for one workflow launch. |
| `StageExecution` | One attempt to execute a workflow state. |
| `AgentExecution` | One agent or system-task execution inside a stage attempt. |
| `Approval` | Human gate that pauses or redirects execution. |
| `Artifact` | Durable output metadata and linkage to run/stage/agent truth. |
| `Report` | Operator-facing receipts, summaries, diagnostics, and evidence. |
| `Command journal` | Durable audit trail for MCP/command-handler mutations. |
| `Side-effect ledger` | Durable guard for externally visible operations such as push, upload, and release. |
| `SQLite` | Canonical local execution truth. |
| `GraphQL projection` | Read model consumed by the macOS UI. |
| `MCP tool` | Command/control and diagnostics surface for agents, CLI, and automation. |

## Boundary Summary

- SwiftUI reads workflow truth through GraphQL projections.
- SwiftUI writes are limited to approval settlement.
- MCP owns create/start/cancel/retry/recovery/report/control operations.
- SQLite is internal daemon storage, not an operator API.
- Provider execution happens through ACP runtime adapters.
- External side effects must be represented in durable service truth before they
  are retried or reconciled.
