# Chainworks Forge

Chainworks Forge is the macOS SwiftUI app project for **Chainworks**: a local control plane for agent-driven engineering work.

The product thesis is simple: the system should move an idea through proposal, review, implementation, audit, and release using explicit workflows, specialized agents, durable artifacts, and hard approval gates. The primary object is not a chat session. It is a **Run**.

## Architecture Sketch

```text
SwiftUI macOS app
  -> Ideas / Agent Catalog / Workflow Inspector
  -> YAML parser + validator + compact normalizer
  -> RunPlan compiler + transition evaluator
  -> Execution service + workflow orchestrator + resume manager
  -> SwiftData models + RunRepository
  -> Artifact manager + disk-backed storage
  -> Steward deterministic analysis services
  -> Approval + artifact-backed run history
```

## What This Repository Contains

This repository currently combines:

- a native SwiftUI macOS client with implemented foundation and core runtime slices
- MVP, proposal, review, and reference documents
- architecture research for orchestration and workspace isolation
- canonical YAML examples for agent catalogs, workflows, and steward config
- unit/UI tests plus CI configuration

## Product Model

Chainworks is designed around:

- **Ideas** entered as text in the app, optionally with a referenced file
- **Runs** as the main execution object
- **Workflows** defined in YAML
- **Specialized agents** with explicit roles, permissions, and provider bindings
- **Artifacts** instead of free-form chat history
- **Approval gates** before sensitive transitions

For the current MVP slice, the system is intended to support:

- one active run per idea
- SwiftData as the durable local store
- Codex and Claude Code as required providers
- automatic run resume on app launch
- three human checkpoints:
  - after the first proposal
  - before implementation
  - before push / distribution

## Implemented Today

- SwiftUI desktop shell with `Ideas`, `Agent Catalog`, and `Workflow Inspector` tabs
- SwiftData domain models for ideas, runs, stages, agents, approvals, artifacts, and Steward records
- YAML DSL parsing, validation, compact workflow normalization, and provenance hashing
- run compilation/runtime core:
  - `RunPlanCompiler`
  - `TransitionEvaluator`
  - `ExecutionService`
  - `WorkflowOrchestrator`
  - `ResumeManager`
- artifact handling:
  - `ArtifactStorage`
  - `ArtifactManager`
  - output-contract-aware artifact formatting
- deterministic Steward services:
  - metrics collection
  - cohort classification
  - anomaly detection
  - run dossier building
- unit tests, UI tests, and CI workflow

## Planned Next

- real provider-backed execution adapters beyond the simulated executor
- end-to-end `Start Run` / run progress / stage detail / artifact inspection UI
- approval inbox and approval decision surfaces in the app
- richer run and artifact browsing for implemented Proposal 002 flows
- Steward report and recommendation UI for Proposal 003 flows

## Not In MVP

- Gemini and other post-MVP providers
- parallel write-capable agents in one worktree
- distributed workers
- cloud sync
- shared multi-user orchestration

## Canonical Documents

- [Product vision](docs/research/chainworks_core_idea.md)
- [MVP scope](docs/ps/chainworks-forge-mvp.md)
- [Architecture research](docs/research/goose_swiftui_agent_architecture_research.md)
- [Foundation/runtime reference](docs/reference/README.md)
- [Proposal 002: Workflow execution engine](docs/proposals/002-workflow-execution-engine.md)
- [Proposal 003: Forge Steward](docs/proposals/003-forge-steward-sdlc-health-and-adaptation.md)

## Canonical YAML Examples

- [Full agent catalog](examples/agents/agents.yaml)
- [Full workflow example](examples/workflows/workflow.yaml)
- [Compact agent example](examples/agents/proposal-po-reviewer.yaml)
- [Compact workflow example](examples/workflows/proposal-to-release.yaml)

## Repository Layout

```text
Chainworks Forge/
  Chainworks Forge.xcodeproj/   Xcode project
  Chainworks Forge/             SwiftUI application sources
    DSL/                        YAML parsing, validation, normalization
    Engine/                     Compiler, orchestration, execution, Steward services
    Models/                     SwiftData persistence models
    Views/                      Current desktop UI surfaces
  Chainworks ForgeTests/        Unit tests
  Chainworks ForgeUITests/      UI tests
  docs/                         Product docs, proposals, reviews, reference
  examples/                     Agent, workflow, and steward YAML examples
  .github/                      CI workflow
```

## Current Status

The repository is no longer just a template scaffold. The domain model, YAML DSL, and a large part of the run/execution core are now implemented. The main gap is still UI depth: the desktop app is inspection-first and does not yet expose the full Proposal 002 or Proposal 003 lifecycle as end-to-end runtime flows.

At this stage, the repository is primarily validating:

- the product shape
- the workflow model
- the agent catalog structure
- immutable run compilation and orchestration contracts
- the approval and artifact model
- deterministic Steward analysis boundaries
- the local macOS control-plane direction

The current runtime direction assumes:

- local-first control plane in SwiftUI
- YAML-defined workflows and agent catalogs
- SwiftData metadata plus disk-backed artifacts
- provider adapters and workspace-isolation boundaries
- deterministic side-effect boundaries for git/release/publish operations
- workflow topology resolved through agent catalog references and backend profiles

## Development

Requirements:

- macOS
- Xcode 26.3 or newer

Open in Xcode:

```bash
open "Chainworks Forge.xcodeproj"
```

Build:

```bash
xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" build
```

Run tests:

```bash
xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" test
```
