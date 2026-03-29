# Chainworks Forge

<p align="center">
  <img src="docs/brand/render/chainworks-forge-readme-hero.png" alt="Chainworks Forge brand hero" width="920" />
</p>

Chainworks Forge is a macOS SwiftUI control plane for agent-driven engineering workflows.

It is built around one idea: the primary object is not a chat thread. It is a **Run**.
A run takes one idea, compiles a frozen workflow snapshot, routes work through specialized agents, pauses at explicit approval gates, stores durable artifacts, and leaves behind a truthful report of what happened.

## What The App Does

- captures ideas as units of engineering work
- executes YAML-defined workflows instead of hardcoded chat flows
- binds specialized agents to providers, models, permissions, and output contracts
- preserves run state, stage history, approvals, and artifact metadata in SwiftData
- stores generated artifacts on disk instead of hiding execution inside chat history
- supports recovery, comparison, reporting, and approval-driven continuation
- keeps repo-backed delivery and release work behind explicit gates

In practice, Chainworks Forge sits between ad hoc AI chats and heavyweight orchestration systems: local-first, inspectable, and built for one engineer running governed multi-agent workflows from a desktop app.

## Core Concepts

| Concept | Meaning |
|---|---|
| `Idea` | A user-entered piece of work, optionally tied to files or a workspace. |
| `Workflow` | A YAML-defined execution graph with stages, approvals, transitions, and agent references. |
| `Run` | One execution instance of a workflow for one idea. This is the main operational object in the system. |
| `RunPlanSnapshot` | The frozen workflow, catalog, provider binding, and path snapshot compiled at run start. |
| `Agent` | A specialized worker with explicit role, provider binding, tool access, and output contract. |
| `Artifact` | Durable output such as a proposal, review report, diff, transcript, receipt, or run report. |
| `Approval gate` | A workflow-defined pause where the engineer must explicitly continue. |

## Current Product Shape

Today the app exposes these top-level operator surfaces:

- `Runs Home` for active, blocked, running, and completed runs
- `Ideas` for starting and managing work
- `Approvals` for pending human decisions
- `Agent Catalog` for inspecting the resolved agent catalog
- `Workflow Inspector` for YAML workflow inspection and validation
- `Pilot Readiness` for readiness and sign-off support
- `Settings` for provider configuration, diagnostics, and remediation

The current MVP provider set is:

- `Codex`
- `Claude Code`
- `Gemini`

## Implemented Today

The repository is no longer a scaffold. It already contains the core control-plane and runtime slices:

- SwiftUI macOS app shell with operator-facing tabs and recovery/report surfaces
- SwiftData models for ideas, runs, stages, approvals, artifacts, benchmark/sign-off state, and provider state
- YAML parsing, validation, normalization, and frozen provenance snapshotting
- run compilation and execution services:
  - `RunPlanCompiler`
  - `TransitionEvaluator`
  - `ExecutionService`
  - `WorkflowOrchestrator`
  - `ResumeManager`
- artifact persistence and retrieval:
  - `ArtifactStorage`
  - `ArtifactManager`
  - report/export surfaces
- provider platform slices:
  - provider settings
  - pilot readiness
  - Goose-backed diagnostics and remediation
  - frozen provider/model provenance truth
- repo-backed delivery slice:
  - worktree provisioning
  - delivery configuration freezing
  - release gate UI
  - evidence/export paths
- layered test gates for fast runtime validation, remote UI smoke, and full sign-off

## What Is Still Active

This repo is still under active product and hardening work. Current active areas are mostly about:

- MVP hardening and final sign-off flow
- output-contract alignment, retry truth, and failure-evidence hardening
- design-system adoption and UI polish

The best source of truth for that work is the docs index and the active proposals, not this README.

## Repository Layout

```text
Chainworks Forge/
  Chainworks Forge.xcodeproj/   Xcode project
  Chainworks Forge/             SwiftUI app sources
    DSL/                        YAML parsing, validation, normalization
    Engine/                     Compiler, orchestration, execution, recovery, export
    Models/                     SwiftData models and repositories
    Views/                      Operator UI surfaces
    Support/                    Policies, design tokens, app configuration
  Chainworks ForgeTests/        Unit and integration tests
  Chainworks ForgeUITests/      macOS UI tests
  TestPlans/                    Xcode test-plan metadata
  docs/                         reference docs, proposals, research, evidence
  examples/                     agent catalogs and workflow examples
  scripts/                      operational helpers, including test gates
```

## Getting Started

### Requirements

- macOS
- Xcode `26.3` or newer

### Open The Project

```bash
open "Chainworks Forge.xcodeproj"
```

### Build

```bash
./scripts/test-gate.sh build
```

### Default Engineering Gate

```bash
./scripts/test-gate.sh fast
```

The repository uses layered test gates. The canonical proving path is the gate runner, not raw `xcodebuild -testPlan ...` commands.

## Test Gates

List available gates:

```bash
./scripts/test-gate.sh list
```

Most common gates:

- `./scripts/test-gate.sh build` — compile-only sanity check
- `./scripts/test-gate.sh fast` — default inner-loop runtime/unit gate
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"` — remote-only UI smoke gate
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh full"` — remote-only full sign-off gate

Important:

- UI tests are remote-only by repo policy
- the remote UI host path is documented in [`docs/reference/agent-ui-test-execution.md`](docs/reference/agent-ui-test-execution.md)
- gate behavior and intended usage are documented in [`docs/reference/test-gates.md`](docs/reference/test-gates.md)

## Key Docs

Start here:

- [`docs/README.md`](docs/README.md) — documentation index and reading order
- [`docs/ps/chainworks-forge-mvp.md`](docs/ps/chainworks-forge-mvp.md) — MVP scope and requirements
- [`docs/research/chainworks_core_idea.md`](docs/research/chainworks_core_idea.md) — product vision and positioning

Implemented-system references:

- [`docs/reference/workflow-execution-engine.md`](docs/reference/workflow-execution-engine.md)
- [`docs/reference/runtime-contract.md`](docs/reference/runtime-contract.md)
- [`docs/reference/operator-experience.md`](docs/reference/operator-experience.md)
- [`docs/reference/provider-platform.md`](docs/reference/provider-platform.md)
- [`docs/reference/full-mvp-delivery.md`](docs/reference/full-mvp-delivery.md)
- [`docs/reference/test-gates.md`](docs/reference/test-gates.md)

Examples:

- [`examples/agents/agents.yaml`](examples/agents/agents.yaml)
- [`examples/workflows/workflow.yaml`](examples/workflows/workflow.yaml)
- [`examples/workflows/proposal-to-release.yaml`](examples/workflows/proposal-to-release.yaml)

## Brand Assets

Brand sources and rendered assets live under [`docs/brand`](docs/brand). The app icon set used by the macOS target lives under [`Chainworks Forge/Assets.xcassets/AppIcon.appiconset`](Chainworks%20Forge/Assets.xcassets/AppIcon.appiconset).
