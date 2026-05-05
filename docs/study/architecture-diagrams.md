# Chainworks Forge — Architecture Diagrams

## System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                      macOS Operator                             │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  ┌──────────────────────────┐     ┌─────────────────────────┐  │
│  │    SwiftUI Views         │     │  GraphQL Read-Only      │  │
│  │  (Thin Client)           │────▶│  Observer               │  │
│  │  - RunsHome              │     │  - Queries              │  │
│  │  - Progress              │     │  - Subscriptions        │  │
│  │  - Timeline              │     └─────────────────────────┘  │
│  │  - Approvals             │               │                   │
│  └──────────────────────────┘               │                   │
│           │                                  │                   │
│           │ (only approval mutations)        │                   │
│           ▼                                  ▼                   │
│  ┌──────────────────────────────────────────────────────────────┐
│  │  Chainworks Forge App (Swift)                               │
│  │  - SwiftData Models (Run, Idea, Artifact, etc.)             │
│  │  - Engine (Compiler, Orchestrator, Execution)               │
│  │  - Providers (ACP adapters)                                 │
│  └──────────────────────────────────────────────────────────────┘
│                            │
│                            │ (via GraphQL)
│                            ▼
├─────────────────────────────────────────────────────────────────┤
│                      control-plane Daemon (Rust)                │
│                                                                  │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐          │
│  │ GraphQL      │  │ MCP Server   │  │ Engine       │          │
│  │ Server       │  │ (HTTP)       │  │ (Orchestrate)│          │
│  │              │  │              │  │              │          │
│  └──────────────┘  └──────────────┘  └──────────────┘          │
│           │               │                    │                 │
│           └───────────────┴────────────────────┘                │
│                        │                                        │
│                        ▼                                        │
│              ┌──────────────────┐                              │
│              │ SQLite Database  │                              │
│              │ (WAL mode)       │                              │
│              └──────────────────┘                              │
└─────────────────────────────────────────────────────────────────┘
```

## Workflow Execution State Machine

```
┌──────────────────────────────────────────────────────────────┐
│ YAML Workflow Definition                                     │
│ (agents.yaml + workflow.yaml)                                │
└────────────────────────────────────────────────────────────┬─┘
                                                              │
        ┌─────────────────────────────────────────────────────┘
        │
        ▼
    ┌─────────────────────────┐
    │ RunPlanCompiler         │
    │ (Phase 1: Validate)     │
    │ (Phase 2: Create Run)   │
    └──────────────┬──────────┘
                  │
                  ▼
    ┌─────────────────────────────────┐
    │ RunPlanSnapshot                 │
    │ - frozen workflow topology      │
    │ - resolved agent bindings       │
    │ - provider/model configuration  │
    │ - immutable (on Run)            │
    └──────────────┬──────────────────┘
                  │
                  ▼
    ┌──────────────────────────────────┐
    │ WorkflowOrchestrator             │
    │ State Machine Loop:              │
    │ 1. Fetch current state           │
    │ 2. Execute run block             │
    │ 3. Check approval gate           │
    │ 4. Evaluate transitions (when:)  │
    │ 5. Advance to next state         │
    │ 6. Loop or finish                │
    └──────────────┬───────────────────┘
                   │
        ┌──────────┴──────────┐
        │                     │
        ▼                     ▼
    State: Normal          State: Manual Gate
    │                      │
    ├─ Sequential     Pause for operator approval
    ├─ Parallel            │
    └─ Dynamic Parallel    ▼
       (for each agent)    State: Next
```

## Agent Execution Flow

```
┌────────────────────────────────────┐
│ RuntimeAgentExecutor               │
│ 1. Resolve provider binding        │
│ 2. Create session                  │
│ 3. Prepare prompt (with artifacts) │
│ 4. Submit to ACP transport         │
│ 5. Stream events                   │
│ 6. Handle permission requests      │
│ 7. Persist outputs                 │
│ 8. Close session                   │
└────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────────────┐
│ ACP Transport (JSON-RPC 2.0 ndjson)        │
│                                             │
│  Provider Adapters:                        │
│  - ClaudeACPProviderAdapter                │
│  - CodexACPProviderAdapter                 │
│  - GeminiACPProviderAdapter                │
│  - AuggieProviderAdapter                   │
│  - JunieProviderAdapter                    │
└────────────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────┐
│ Provider Runtime (on subprocess)    │
│                                     │
│ - Claude Code                      │
│ - Codex                            │
│ - Gemini CLI                       │
│ - Auggie                           │
│ - Junie                            │
└────────────────────────────────────┘
        │
        ▼
┌────────────────────────────────────┐
│ Artifact Storage                    │
│ - Save outputs to filesystem       │
│ - Update metadata in database      │
│ - Persist delivery receipts        │
└────────────────────────────────────┘
```

## Data Flow: Artifact Persistence

```
Agent Execution
    │
    ├─ Generate output (text, JSON, code)
    │
    ▼
Output Contract Validation
    │
    ├─ Check required fields
    ├─ Validate types
    └─ Enforce schema
    │
    ▼
Artifact Naming & Path Resolution
    │
    ├─ Map logical name (e.g., "proposal_current")
    ├─ Resolve env vars (${CHAINWORKS_META_ROOT:-.chainworks})
    └─ Determine filesystem path
    │
    ▼
ArtifactStorage.store()
    │
    ├─ Create directory if needed
    ├─ Write to disk
    └─ Return file URL
    │
    ▼
ArtifactManager (Metadata)
    │
    ├─ Create Artifact record (SwiftData)
    ├─ Link to Run & StageExecution
    └─ Track lineage & version
    │
    ▼
TransitionEvaluator
    │
    ├─ Check: exists('artifact_name')?
    └─ Allow next transition
```

## Approval Gate Flow

```
┌─────────────────────────────────────┐
│ WorkflowOrchestrator                │
│ Encounters: type: manual_gate       │
│ approval: required                  │
└──────────────┬──────────────────────┘
               │
               ▼
        Pause Execution
        Create ApprovalRequest
               │
        ┌──────┴──────┐
        │             │
        ▼             ▼
    SwiftUI         GraphQL
    (local)         (external)
       │               │
       │               ▼
       │        MCP Tool: approveApproval()
       │               │
       └───────┬───────┘
               │
               ▼
        ApprovalRequest.decide()
        (granted | rejected)
               │
        ┌──────┴──────┐
        │             │
        ▼             ▼
    Continue      Reject & Revert
    to next       to previous state
    state
```

## Rust Control-Plane Crate Dependencies

```
daemon
  ├─ graphql-server (queries, mutations, subscriptions)
  ├─ mcp-server (MCP Streamable HTTP + inbox)
  └─ engine (state machine, orchestrator)
     ├─ workflow (YAML parser, RunPlan)
     ├─ acp (JSON-RPC transport + adapters)
     ├─ db (SQLite repos, migrations)
     └─ domain (types, enums, commands)

acp
  └─ domain

db
  └─ domain

auth (principals, bearer token)
  └─ domain
```

## Resume & Recovery Path

```
App Restart
    │
    ▼
RecoveryCoordinator.discoverInterruptedRuns()
    │
    ├─ Scan for runs not in terminal state
    ├─ Check for drift (YAML changed?)
    └─ Identify last completed stage
    │
    ▼
Drift Detection?
    │
    ├─ YES: Present DriftDecision UI
    │       (continue as-is | replan | abort)
    │
    ├─ NO: Continue
    │
    ▼
ResumeManager.resume()
    │
    ├─ Rebuild RunPlan from snapshot
    ├─ Validate compiler version
    └─ Restore current state
    │
    ▼
WorkflowOrchestrator.executeStateMachine()
    │
    └─ Resume from last state (no re-execution of completed stages)
```

## Delivery Pipeline (Repo-Backed)

```
Run Completed
    │
    ▼
DeliveryConfiguration
    │
    ├─ repo_identifier
    ├─ base_branch
    ├─ target_branch
    └─ worktree_root
    │
    ▼
DeliveryPreflightService
    │
    ├─ Validate git setup
    ├─ Check permissions
    └─ Provision worktree
    │
    ▼
Manual Release Gate
    │
    ├─ Operator reviews
    └─ Clicks "Release"
    │
    ▼
GitReleaseService
    │
    ├─ Commit changes
    ├─ Push to target_branch
    └─ Generate git_push_receipt
    │
    ▼
Evidence Export
    │
    ├─ Collect all artifacts
    ├─ Package run report
    └─ Generate delivery_receipt
```

## Key Data Structures

```
Run (SwiftData Model)
├─ id: UUID
├─ startedAt: Date
├─ completedAt: Date?
├─ status: RunStatus
├─ RunPlanSnapshot (frozen):
│  ├─ workflowID, workflowTitle
│  ├─ workflowSnapshotHash
│  ├─ catalogSnapshotHash
│  └─ workflowSnapshotJSON (immutable)
├─ providerBindingSnapshotJSON
├─ resolvedSkillsJSON
└─ resolvedMCPPoliciesJSON

Stage (per Run)
├─ id: UUID
├─ stateID: String (from workflow)
├─ executedAt: Date
├─ completedAt: Date?
└─ agentExecutions: [AgentExecution]

AgentExecution
├─ id: UUID
├─ agentID: String
├─ status: ExecutionStatus
├─ inputs: [String: ArtifactReference]
├─ outputs: [String: Artifact]
└─ sessionID: UUID
```

## Transition Condition Language

```
when:
  # Artifact must exist
  - exists('artifact_name')
  
  # Compare fields
  - artifact.field == "value"
  - artifact.score >= 8.5
  
  # Reference variables
  - vars.my_var > 10
  - vars.counter < vars.max_counter
  
  # Combine with logic
  - exists('a') AND vars.x > 5
  - var.status == 'done' OR exists('fallback')
  
  # Complex expressions
  - (artifact.score >= 8) AND (exists('review') OR vars.skip_review)
```
