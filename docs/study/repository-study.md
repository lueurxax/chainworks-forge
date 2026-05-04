# Chainworks Forge — Repository Study

## Project Overview

**Chainworks Forge** is a macOS SwiftUI control plane for managing multi-agent engineering workflows. The primary object of the system is a **Run**, not a chat thread.

### Key Concepts

| Concept | Meaning |
|---------|---------|
| **Idea** | User task, bound to files/workspace |
| **Workflow** | YAML-defined execution graph with stages, approvals, transitions |
| **Run** | Single execution instance of a workflow for one idea |
| **RunPlanSnapshot** | Frozen workflow/catalog/provider snapshot compiled at run start |
| **Agent** | Specialized worker with role, provider, tool access, output contract |
| **Artifact** | Durable output (proposal, review, diff, receipt, report) |
| **Approval gate** | Managed workflow pause requiring explicit continuation by the engineer |

## Repository Structure

```
/
├── Chainworks Forge/              # macOS SwiftUI app (primary)
│   ├── Engine/                    # Compiler, orchestration, execution, recovery
│   ├── DSL/                       # YAML parsing, validation, normalization
│   ├── Models/                    # SwiftData models & repositories
│   ├── Views/                     # Operator UI surfaces
│   ├── Providers/                 # ACP adapters (Claude, Codex, Gemini, Auggie, Junie)
│   └── Support/                   # Design tokens, config, policies
├── Chainworks ForgeTests/         # Unit & integration tests
├── Chainworks ForgeUITests/       # macOS UI tests (remote-only by policy)
├── control-plane/                 # Rust daemon with parity implementation
│   └── crates/
│       ├── daemon/                # Daemon binary
│       ├── engine/                # State machine, orchestrator
│       ├── acp/                   # JSON-RPC 2.0 ndjson transport
│       ├── graphql-server/        # Thin UI read boundary
│       ├── mcp-server/            # MCP Streamable HTTP transport
│       ├── db/                    # SQLite repos, WAL mode
│       ├── workflow/              # YAML compiler (runs & catalogs)
│       └── domain/                # IDs, enums, commands, events
├── docs/                          # Reference docs, proposals, evidence
├── examples/                      # agents.yaml, workflows/*.yaml
└── scripts/                       # test-gate.sh (canonical gate runner)
```

## Architecture

### Two Interacting Codebases

1. **SwiftUI app** (canonical owner) — `Chainworks Forge/`
   - Operator-oriented interface
   - SwiftData for persistence (local SQLite)
   - Serves all primary operations

2. **Rust control-plane daemon** (parity replica) — `control-plane/`
   - Mirrors orchestration & persistence
   - Capacity-aware scheduling, fairness, executor backpressure
   - SQLite WAL mode for concurrent reads
   - GraphQL server for read-side truth

### Key Components

**Execution Engine:**
- `RunPlanCompiler` — YAML → compiled RunPlan (2-phase)
- `WorkflowOrchestrator` — state machine driver with resume support
- `TransitionEvaluator` — expression evaluator for `when:` clauses
- `RuntimeAgentExecutor` — single-agent execution with retries

**Artifact Layer:**
- `ArtifactStorage` — filesystem persistence
- `ArtifactManager` — metadata management
- Bounded discovery & settlement optimization

**Runtime Transport:**
- `RuntimeTransportProtocol` — ACP contract
- Adapter families: Claude, Gemini, Codex, Auggie, Junie
- ACP-only runtime (Goose — legacy)

**Persistence:**
- SwiftData models for App
- SQLite + migrations for Rust daemon
- Frozen snapshots in Run records

## Implemented Workflows

1. Idea creation & archive/restore lifecycle
2. Proposal-loop execution with approval gates
3. Lead conflict mediation for same-run resolution
4. Repo-backed full delivery with worktrees & release gates
5. Implementation self-assessment & handoff routing
6. Evidence-pack export & benchmark/sign-off evaluation
7. Provider setup & pilot readiness validation
8. Run progress, artifact inspection, recovery from UI

## Testing

- **Gate runner:** `./scripts/test-gate.sh`
- **Gates:** build, fast (default), guardrails, full, proposal-XXX
- **UI tests:** remote-only (SSH to `test@SMacBook.local`)
- **Test framework:** Swift Testing + XCTest integration

## Documentation

- **docs/README.md** — index & reading order
- **docs/reference/** — implemented-system truth (authoritative)
- **docs/proposals/** — design intent & work-in-progress
- **docs/evidence/** — proof artifacts for acceptance gates

## Deep Dive: Swift App Architecture

### Engine Directory (`Chainworks Forge/Engine/`)

**Core Orchestration:**
- `RunPlanCompiler.swift` — previewCompile (validation) + createRun (persistence)
- `WorkflowOrchestrator.swift` — @MainActor state machine driver
- `TransitionEvaluator.swift` — `when:` expression evaluator
- `RuntimeAgentExecutor.swift` — single-agent execution with watchdog/retries
- `AgentSessionManager.swift` — per-run session lineage & reuse

**Artifact Layer:**
- `ArtifactStorage.swift` — filesystem backed storage
- `ArtifactManager.swift` — metadata & persistence ordering
- Bounded discovery & settlement optimization

**Providers:**
- `ACPAdapters/` — Claude, Codex, Gemini, Auggie, Junie adapters
- `RuntimeTransportFactory.swift` — adapter selection

**Key Services:**
- `ExecutionService.swift` — top-level coordinator
- `RecoveryCoordinator.swift` — startup repair & drift handling
- `ResumeManager.swift` — resume after interruption
- `DeliveryConfiguration.swift` — repo-backed delivery setup
- `GitReleaseService.swift` — native git/publish operations

### Models Directory (`Chainworks Forge/Models/`)

**Core Domain:**
- `Run.swift` — main execution unit (RunPlanSnapshot frozen at creation)
- `Idea.swift` — user-entered work items
- `Artifact.swift` — persistent outputs
- `StageExecution.swift` & `AgentExecution.swift` — execution records
- `Approval.swift` — approval requests & decisions
- `RunRepository.swift` — exclusive entry point for Run creation (ARCH-002)

**Advanced Models:**
- `WorkflowConflict.swift` — lead mediation for conflicts
- `AgentSessionLineage.swift` — session reuse tracking
- `BenchmarkExecutionRecord.swift` — sign-off state
- `MVPSignOffDecisionSnapshot.swift` — sign-off decisions

### Views Directory (`Chainworks Forge/Views/`)

**Main Surfaces:**
- `RunsHomeView.swift` — active/blocked/running/completed runs
- `RunProgressScreen.swift` — real-time execution monitoring
- `RunTimelineInspectorView.swift` — focused timeline with artifact hierarchy
- `ApprovalInboxScreen.swift` — pending human decisions (GraphQL)
- `AgentCatalogView.swift` — resolved catalog inspection
- `WorkflowInspectorView.swift` — YAML validation & inspection

## Deep Dive: Rust Control-Plane

### Crates Organization

- **daemon/** — Single binary, GraphQL (:4000/graphql) & MCP (:4000/mcp)
- **engine/** — State machine, orchestrator, work-queue
- **acp/** — JSON-RPC 2.0 ndjson transport + adapters
- **graphql-server/** — async-graphql + axum
- **mcp-server/** — MCP Streamable HTTP + inbox
- **db/** — SQLite repos, WAL mode, migrations
- **workflow/** — YAML parser & RunPlan compiler
- **domain/** — Types, IDs, enums, commands, events
- **auth/** — Bearer token auth, principals

### Key Rust Components

- **Orchestrator** — Drives state machine
- **BackgroundExecutor** — Work-queue with capacity awareness
- **CommandHandler** — Durable event log
- **MediationSettlementService** — Lead conflict resolution
- **RecoveryService** — Startup repair

### Database

- 3 migrations, WAL mode for concurrent reads
- `command_journal` — durable event log
- `agent_executions` — owner-aware execution records
- projections — settled attempt view
- `work_queue` — scheduled tasks

## YAML Contracts Reference

### Workflow States

```yaml
states:
  state_id:
    label: "Human-readable label"
    type: [start | normal | manual_gate | end]
    owner: agent_id
    run:
      sequence: [{agent, task, inputs, outputs}]
      parallel: [{agent, task, inputs, outputs}]
      dynamic_parallel:
        selector_artifact: artifact_name
        output_contract: contract_type
    approval: [required | optional]
    loop:
      counter: loop_var
      max: limit_expr
    transitions:
      - to: next_state_id
        when: |
          exists('artifact_name') |
          artifact.field == value |
          vars.variable > number |
          condition AND condition |
          condition OR condition
```

### Agent Catalog Structure

```yaml
backend_profiles:
  profile_id:
    provider: [claude|codex|gemini|auggie|junie]
    model: model_string
    effort: [experimental|standard|production]
    temperature: 0.0-1.0
    max_turns: number

agents:
  - id: agent_id
    backend_profile: profile_id
    prompt: |
      Agent instructions with {{input_name}} placeholders
    inputs: [artifact_name, var.name, ...]
    outputs: [artifact_name, ...]
    permission_profile: [allow_once|require_confirmation]

artifacts:
  artifact_name: ${CHAINWORKS_META_ROOT:-.chainworks}/path/to/file.ext

paths:
  repo_root: ${CHAINWORKS_REPO_ROOT:-.}
  meta_root: ${CHAINWORKS_META_ROOT:-.chainworks}
  worktrees_root: ${CHAINWORKS_WORKTREES_ROOT:-.chainworks/worktrees}
```

## Development Workflow

### Local Setup

```bash
git clone <repo> && cd "Chainworks Forge"
open "Chainworks Forge.xcodeproj"
./scripts/test-gate.sh build
./scripts/test-gate.sh fast
./scripts/test-gate.sh list
```

### Rust Daemon Development

```bash
cd control-plane
cargo build
cargo test -p engine some_test
DATABASE_URL="sqlite:///.db" GRAPHQL_ADDR="127.0.0.1:4000" RUST_LOG=info \
  ./target/debug/control-plane
```

### Testing Strategy

- **guardrails** — Source lints, no build
- **fast** — Default inner-loop (guardrails + build + unit tests)
- **full** — Complete sign-off via xcodebuild
- **UI smoke** — Remote-only UI smoke test
- **proposal-XXX** — Focused proposal gate

### Important Safety Rules

- Never run destructive git commands without explicit permission
- Do not delete `.chainworks/` without authorization
- Run lifecycle is managed by the orchestrator, not manual cleanup
- UI tests are remote-only by policy
- Always use `./scripts/test-gate.sh` instead of raw xcodebuild

## Architecture Decision Log

- **ARCH-002** — Single active run per idea via RunRepository
- **ARCH-021** — Two-phase compilation (preview + create)
- **ARCH-025/026** — Workspace-bound execution
- **ARCH-027** — Lazy stage creation
- **ARCH-031** — Transition expression evaluation
- **ARCH-067-075** — Delivery configuration contract

See `docs/reference/architecture-decisions.md` for the full log.

## Quick Navigation

| Need | Location |
|------|----------|
| Compile workflow | `RunPlanCompiler.swift` |
| Execute agents | `RuntimeAgentExecutor.swift` |
| Persist artifacts | `ArtifactStorage.swift` |
| Evaluate transitions | `TransitionEvaluator.swift` |
| Handle approval | `Approval.swift` + GraphQL |
| Setup provider | `ProviderSettings.swift` |
| Export evidence | `EvidencePackBuilder.swift` |
| Recover from error | `RecoveryCoordinator.swift` |

✅ Lead-mediated workflow conflict resolution
✅ Thin GraphQL-only UI rewrite (P031)
✅ Frozen run snapshots & deterministic execution
✅ Declarative workflow authority
✅ Operator shell with recovery & comparison
✅ ACP-backed execution
✅ Repo-backed delivery, release gating
✅ Provider toolchain cache mapping
✅ Proposal-loop feedback-fidelity layer
✅ Design-system adoption (Forge tokens)

## Key References

- **Current baseline:** `docs/reference/current-system-baseline.md`
- **Execution engine:** `docs/reference/workflow-execution-engine.md`
- **ACP transport:** `docs/reference/acp-runtime-transport.md`
- **MVP sign-off:** `docs/reference/mvp-sign-off.md`
- **Full delivery:** `docs/reference/full-mvp-delivery.md`
- **Test strategy:** `docs/reference/test-gates.md`
