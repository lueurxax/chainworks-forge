# Chainworks Forge — Quick Reference

## 🎯 Project in one line

**Chainworks Forge** — a macOS operator-oriented application for managing multi-agent engineering workflows with explicit approvals, durable artifacts, and repo-backed delivery.

## 📦 Two codebases work together

| Component | Language | Role |
|-----------|----------|------|
| **Chainworks Forge/** | Swift (SwiftUI) | Canonical operator shell (priority) |
| **control-plane/** | Rust | Parity daemon with capacity-aware scheduling |

## 🏗️ Core objects

```
Idea → Run → States → Stages → Agent Executions → Artifacts
         ↓
    RunPlanSnapshot (frozen)
```

- **Idea** — what needs to be done (user input)
- **Run** — one workflow instance for an idea
- **RunPlanSnapshot** — frozen compiled workflow (immutable)
- **Stage** — execution of one state in a run
- **Agent** — specialized worker (Claude, Gemini, etc.)
- **Artifact** — output file (proposal, review, code, receipt)

## 🚀 Build and test

```bash
# First launch
open "Chainworks Forge.xcodeproj"

# Build
./scripts/test-gate.sh build

# Test (default gate)
./scripts/test-gate.sh fast

# View all gates
./scripts/test-gate.sh list

# UI tests (remote only)
ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"

# Rust daemon
cd control-plane && cargo build
```

## 📁 Main directories

```
Engine/           Compiler, orchestrator, execution, recovery
Models/           SwiftData models (Run, Idea, Artifact, etc.)
Views/            UI surfaces (RunsHome, Progress, Timeline, Approvals)
Providers/        ACP adapters (Claude, Codex, Gemini, etc.)
DSL/              YAML parsing & validation
Support/          Design tokens, policies, configuration
```

## 🔌 Providers

Supported providers in agents.yaml:
- `claude_acp` → Claude Agent (ACP)
- `codex_acp` → Codex (ACP)
- `gemini_acp` → Gemini (ACP)
- `auggie_acp` → Auggie (ACP)
- `junie` → Junie

## 📝 YAML basics

### Workflow

```yaml
workflow:
  id: workflow_id
  uses_agent_catalog: ./agents.yaml
  states:
    state1:
      owner: agent_id
      run:
        sequence: [...]
      transitions:
        - to: state2
          when: exists('artifact_name')
```

### Catalog

```yaml
agents:
  - id: agent_id
    backend_profile: profile_id
    prompt: "Instructions"
    inputs: [artifact_name, ...]
    outputs: [artifact_name, ...]
```

## 🔍 Find the right function

| Task | File |
|------|------|
| Compile workflow → RunPlan | `RunPlanCompiler.swift` |
| Execute agent | `RuntimeAgentExecutor.swift` |
| Save artifact | `ArtifactStorage.swift` |
| Evaluate transition (when:) | `TransitionEvaluator.swift` |
| Process approval | `Approval.swift` |
| Launch provider | `BackendProfileResolverV2.swift` |
| Recover from error | `RecoveryCoordinator.swift` |
| Deliver to repo | `DeliveryConfiguration.swift` |

## 📚 Documentation (reading priority)

1. **docs/README.md** — documentation index
2. **docs/reference/current-system-baseline.md** — what is implemented now
3. **docs/reference/workflow-execution-engine.md** — how execution works
4. **docs/reference/acp-runtime-transport.md** — how providers work
5. **examples/workflows/full-mvp-live.yaml** — complete workflow example

## ⚙️ Architectural decisions (ARCH-XX)

- **ARCH-002** — RunRepository = sole point of Run creation
- **ARCH-021** — Compilation in 2 phases (preview + create)
- **ARCH-027** — StageExecution created lazily on state entry
- **ARCH-031** — TransitionEvaluator for expressions in `when:`

## 🛡️ Safety Rules

❌ **DO NOT DO WITHOUT PERMISSION:**
- `git reset --hard` or other destructive git commands
- Delete `.chainworks/` directory
- Run UI tests locally (remote only)
- Call raw `xcodebuild -testPlan` instead of `test-gate.sh`

✅ **DO:**
- Use `./scripts/test-gate.sh` for all tests
- Read `docs/reference/` for system truth
- Check `docs/proposals/` for design intent
- Use `codebase-retrieval` for code search

## 🎨 Design Tokens & Brand

- Design system: `Chainworks Forge/Support/Design/`
- Design tokens: `Chainworks Forge/Support/DesignTokens.swift`
- Brand assets: `docs/brand/`
- Design kit: `docs/reference/chainworks_forge_design_kit_v1.md`

## 🔗 Related systems

- **Rust Control Plane** — GraphQL server + MCP protocol
- **ACP Transport** — JSON-RPC 2.0 ndjson over stdio
- **Skills** — Shared MCP-based tooling
- **Steward** — System health analysis

## 💡 Key Insights

1. **Main object = Run, not chat** — Artifacts on disk, metadata in DB
2. **Frozen Snapshots** — RunPlanSnapshot immutable after creation
3. **Single Active Run per Idea** — RunRepository enforces this via contract
4. **ACP-First Runtime** — Goose = legacy only
5. **Thin GraphQL UI** — Server owns truth, UI = observer + approvals
6. **Capability-Aware Scheduling** — Rust daemon handles concurrency
