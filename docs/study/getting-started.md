# Chainworks Forge — Getting Started

## 📋 System Requirements

- **macOS** 26.2 or higher
- **Xcode** 26.3 or higher
- **Rust** 1.70+ (for control-plane; optional)
- **Git** (for version control)
- Providers: Codex, Claude Code, or Gemini (for live execution)

## 🚀 First Steps

### 1. Understand the Architecture (30 minutes)

Read **in this order:**

1. `docs/study/quick-reference.md` (terminology and map)
2. `docs/reference/current-system-baseline.md` (what exists now)
3. `README.md` sections "Core Concepts" & "Current Product Shape"
4. Open `examples/workflows/full-mvp-live.yaml` and `examples/agents/agents.yaml`

### 2. Set Up Your Local Environment (20 minutes)

```bash
cd "Chainworks Forge"

# Open Xcode
open "Chainworks Forge.xcodeproj"

# Build the project (verify everything compiles)
./scripts/test-gate.sh build

# Run fast tests
./scripts/test-gate.sh fast
```

### 3. Run the Application (10 minutes)

```bash
# In Xcode: Product → Run (or Cmd+R)
# On first launch: Settings → Configure Provider
# Choose Codex/Claude Code/Gemini (if you have credentials)
# Create an Idea for testing
```

### 4. Study the Code: Choose Your Direction

#### 🔧 Execution Engine

```
Goal: Understand how workflows are compiled and executed

Files to read:
1. Chainworks Forge/Engine/RunPlanCompiler.swift (lines 1-100)
2. Chainworks Forge/Engine/WorkflowOrchestrator.swift (lines 1-80)
3. Chainworks Forge/Engine/TransitionEvaluator.swift

Then:
- docs/reference/workflow-execution-engine.md (complete)
- docs/reference/runtime-contract.md
- examples/workflows/workflow.yaml
```

#### 🤖 Providers & Runtime

```
Goal: Understand how agents are executed through providers

Files to read:
1. Chainworks Forge/Providers/BackendProfileResolverV2.swift
2. Chainworks Forge/Engine/RuntimeAgentExecutor.swift
3. Chainworks Forge/Providers/ClaudeACPProviderAdapter.swift

Then:
- docs/reference/acp-runtime-transport.md (complete)
- examples/agents/agents.yaml
- control-plane/crates/acp/src/adapters/ (Rust)
```

#### 📦 Artifacts & Storage

```
Goal: Understand how outputs are saved and retrieved

Files to read:
1. Chainworks Forge/Models/Artifact.swift
2. Chainworks Forge/Engine/ArtifactStorage.swift
3. Chainworks Forge/Engine/ArtifactManager.swift

Then:
- docs/reference/artifact-discovery-and-settlement-optimization.md
- docs/reference/output-contracts-failure-evidence-and-recovery.md
```

#### 🎨 User Interface

```
Goal: Understand how the UI connects to the backend

Files to read:
1. Chainworks Forge/Views/RunsHomeView.swift
2. Chainworks Forge/Views/RunProgressScreen.swift
3. Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift

Then:
- docs/reference/query-projections-and-client-consumption-contract.md
- docs/reference/ui-action-boundary.md
- control-plane/crates/graphql-server/ (Rust)
```

#### 🦀 Rust Daemon

```
Goal: Understand how the daemon replicates Swift app functionality

Files to read:
1. control-plane/crates/daemon/src/main.rs
2. control-plane/crates/engine/src/lib.rs
3. control-plane/crates/db/src/lib.rs

Then:
- docs/reference/rust-control-plane.md (complete)
- cargo test -p engine
```

## 🧪 Running Tests

```bash
./scripts/test-gate.sh fast          # Fast tests (start here)
./scripts/test-gate.sh full          # Full suite
./scripts/test-gate.sh proposal-027  # Specific proposal gate
./scripts/test-gate.sh list          # List all gates
./scripts/test-gate.sh guardrails    # Lint only, no build
```

## 📖 Where to Find Information

| Question | Source |
|----------|--------|
| How does workflow execution work? | `docs/reference/workflow-execution-engine.md` |
| Which providers are supported? | `examples/agents/agents.yaml` + `docs/reference/provider-binding-truth.md` |
| How does persistence work? | `docs/reference/domain-model.md` |
| How is the UI structured? | `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md` |
| How does recovery work? | `docs/reference/execution-truth-and-recovery.md` |
| How does delivery work? | `docs/reference/full-mvp-delivery.md` |
| How are tests written? | `docs/reference/test-suite-architecture.md` |

## 🔍 Quick Code Locations

```
Main workflow loop:
  WorkflowOrchestrator.executeStateMachine() → Chainworks Forge/Engine/

YAML compilation:
  RunPlanCompiler.previewCompile() → Chainworks Forge/Engine/

Agent execution:
  RuntimeAgentExecutor.executeAgent() → Chainworks Forge/Engine/

Artifact storage:
  ArtifactStorage.store() → Chainworks Forge/Engine/

Providers:
  ClaudeACPProviderAdapter, CodexACPProviderAdapter, etc. → Chainworks Forge/Providers/

Approvals UI:
  ApprovalInboxScreen.swift → Chainworks Forge/Views/

Rust daemon sync:
  GraphQL → control-plane/crates/graphql-server/
  MCP tools → control-plane/crates/mcp-server/
```

## 💬 Glossary

- **Run** — the primary object (not a chat)
- **Stage** — one execution of a state in a run
- **Agent Execution** — one execution of an agent in a stage
- **Artifact** — a file on disk (proposal, review, code, etc.)
- **Approval Gate** — a `manual_gate` state requiring explicit approval
- **Transition** — movement from one state to another (`when:` condition)
- **RunPlanSnapshot** — frozen compilation at run start (immutable)
- **Drift** — YAML changed since the run was compiled

## 🎓 Recommended Reading Order

1. ✅ `docs/study/quick-reference.md` (15 min)
2. ✅ `docs/study/getting-started.md` — you are here (20 min)
3. ⬜ `examples/workflows/full-mvp-live.yaml` (30 min)
4. ⬜ `examples/agents/agents.yaml` (30 min)
5. ⬜ `docs/reference/current-system-baseline.md` (30 min)
6. ⬜ `docs/reference/workflow-execution-engine.md` (60 min)
7. ⬜ Your chosen specialization (see sections above)
