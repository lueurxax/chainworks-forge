# Chainworks Forge

Chainworks Forge is the macOS SwiftUI app project for **Chainworks**: a local control plane for agent-driven engineering work.

The product thesis is simple: the system should move an idea through proposal, review, implementation, audit, and release using explicit workflows, specialized agents, durable artifacts, and hard approval gates. The primary object is not a chat session. It is a **Run**.

## What This Repository Contains

This repository currently combines four layers:

- a native SwiftUI desktop client scaffold
- MVP and product-definition documents
- architecture research for the runtime and orchestration model
- canonical YAML examples for agent catalogs and workflows

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

## Canonical Documents

- [Product vision](docs/research/chainworks_core_idea.md)
- [MVP scope](docs/ps/chainworks-forge-mvp.md)
- [Architecture research](docs/research/goose_swiftui_agent_architecture_research.md)

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
  Chainworks ForgeTests/        Unit tests
  Chainworks ForgeUITests/      UI tests
  docs/                         Product docs, PS, and research
  examples/                     Agent and workflow YAML examples
```

## Current Status

The SwiftUI app is still close to the default app template. The product model, workflow DSL, and orchestration direction are currently ahead of the UI implementation. At this stage, the repository is primarily validating:

- the product shape
- the workflow model
- the agent catalog structure
- the approval and artifact model
- the local macOS control-plane direction

The current runtime direction assumes:

- local-first control plane in SwiftUI
- YAML-defined workflows and agent catalogs
- a Goose-based execution substrate via `goosed` and REST/SSE
- deterministic side-effect boundaries for git/release/publish operations

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
