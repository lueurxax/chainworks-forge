# Chainworks Forge — Study Materials Index

> Introductory learning materials for the Chainworks Forge repository.  
> For authoritative implemented-system truth, use [`docs/reference/`](../reference/README.md).

## 📖 Recommended Reading Order

### Phase 1: Core Understanding (2–3 hours)

1. **[start-here.md](start-here.md)** — 5-minute entry point, key concepts, three learning paths
2. **[quick-reference.md](quick-reference.md)** — Terminology, file locations, YAML basics, safety rules
3. **`examples/workflows/full-mvp-live.yaml`** — A complete real workflow from idea to delivery
4. **`examples/agents/agents.yaml`** — The live agent catalog with backend profiles

### Phase 2: Architecture (2–3 hours)

5. **[repository-study.md](repository-study.md)** — Repository layout, two codebases, component breakdown, YAML contracts
6. **[architecture-diagrams.md](architecture-diagrams.md)** — System diagram, execution state machine, data flows, recovery path

### Phase 3: Hands-On (2–3 hours)

7. **[getting-started.md](getting-started.md)** — Setup + 5 specialization tracks (choose one)
8. **[code-examples.md](code-examples.md)** — 10 practical examples: YAML patterns, Swift code, test patterns
9. **[common-commands.md](common-commands.md)** — CLI reference for building, testing, searching, debugging

## 🎯 Specialization Tracks

Choose based on your development focus:

| Track | Start With | Key Files |
|-------|-----------|-----------|
| **Execution Engine** | getting-started.md §4 | RunPlanCompiler, WorkflowOrchestrator, TransitionEvaluator |
| **Providers & Runtime** | getting-started.md §5 | ACP Adapters, RuntimeAgentExecutor, BackendProfileResolverV2 |
| **Artifacts & Storage** | getting-started.md §6 | ArtifactStorage, ArtifactManager, Output Contracts |
| **User Interface** | getting-started.md §7 | Views, GraphQL boundary, P031ThinGraphQLReadBoundary |
| **Rust Daemon** | getting-started.md §8 | control-plane/crates, engine, db, graphql-server |

## 🔍 Find Information by Topic

| Topic | Document | Section |
|-------|----------|---------|
| What is Chainworks Forge? | start-here.md | What is Chainworks Forge |
| Setup & first build | getting-started.md | First Steps |
| How code is organized | repository-study.md | Repository Structure |
| How workflow execution works | architecture-diagrams.md | Workflow Execution State Machine |
| How providers work | repository-study.md | Deep Dive: Rust Control-Plane |
| How artifacts are stored | architecture-diagrams.md | Data Flow: Artifact Persistence |
| How approvals work | architecture-diagrams.md | Approval Gate Flow |
| How to write a workflow | code-examples.md | Examples 1–4 |
| How to write agents | code-examples.md | Example 8 |
| Where is RunPlanCompiler? | repository-study.md | Engine Directory |
| How to run tests | common-commands.md | Building & Testing |
| Design principles | quick-reference.md | Key Insights |
| Safety rules | quick-reference.md | Safety Rules |

## 📋 Document Descriptions

### [start-here.md](start-here.md)
5-minute entry point with 30-second repo overview, three learning paths, and first commands to run.

### [quick-reference.md](quick-reference.md)
Quick-lookup card: glossary, file locations, YAML basics, providers, architecture decisions, safety rules.

### [getting-started.md](getting-started.md)
Hands-on setup guide with 5 specialization tracks, each with a specific reading list and goal.

### [repository-study.md](repository-study.md)
Comprehensive breakdown: repository layout, Engine/Models/Views directories, Rust crates, YAML contracts, dev workflow.

### [architecture-diagrams.md](architecture-diagrams.md)
ASCII-art diagrams: system overview, execution state machine, agent flow, artifact persistence, approval gates, recovery.

### [code-examples.md](code-examples.md)
10 copy-paste-ready examples: minimal workflows, loops, fan-out, conditional transitions, Swift code patterns, tests.

### [common-commands.md](common-commands.md)
CLI command reference: building, testing, running the daemon, navigating, searching, debugging, git safety.

## 💻 Most Important Source Files to Explore

**Swift App (Priority Order):**
1. `Chainworks Forge/Engine/RunPlanCompiler.swift` — YAML → RunPlan compilation
2. `Chainworks Forge/Engine/WorkflowOrchestrator.swift` — State machine driver
3. `Chainworks Forge/Engine/RuntimeAgentExecutor.swift` — Agent execution
4. `Chainworks Forge/Models/Run.swift` — Main data model
5. `Chainworks Forge/Engine/ArtifactStorage.swift` — Artifact persistence

**Rust Daemon (Priority Order):**
1. `control-plane/crates/daemon/src/main.rs` — Entry point
2. `control-plane/crates/engine/src/lib.rs` — Engine
3. `control-plane/crates/db/src/lib.rs` — Database
4. `control-plane/crates/graphql-server/src/lib.rs` — GraphQL
5. `control-plane/crates/acp/src/lib.rs` — ACP transport

## ✅ Knowledge Verification

After studying, you should be able to:

- [ ] Explain what a Run is (it is not a chat thread)
- [ ] Sketch the state machine loop
- [ ] Explain the roles of the Swift app vs the Rust daemon
- [ ] Read and understand a YAML workflow definition
- [ ] Read and understand a YAML agent catalog
- [ ] Explain how artifacts are persisted (disk vs DB)
- [ ] Explain how transitions are evaluated
- [ ] Explain how approval gates work
- [ ] Explain how providers are resolved
- [ ] Understand the RunPlanSnapshot concept (frozen, immutable)
- [ ] Run `./scripts/test-gate.sh fast` successfully

## 🔗 Official Documentation

These study materials are introductory. For authoritative system truth:

- [`docs/README.md`](../README.md) — Official index
- [`docs/reference/current-system-baseline.md`](../reference/current-system-baseline.md) — What's implemented
- [`docs/reference/workflow-execution-engine.md`](../reference/workflow-execution-engine.md) — Execution details
- [`docs/reference/acp-runtime-transport.md`](../reference/acp-runtime-transport.md) — Provider details
- [`docs/reference/test-gates.md`](../reference/test-gates.md) — Test strategy

**Use study materials for:** Orientation, navigation, quick lookup, learning paths  
**Use official docs for:** Authoritative truth, complete specifications, decision rationale
