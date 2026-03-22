# Chainworks Forge

Chainworks Forge is a macOS SwiftUI project for exploring a local control plane for multi-agent engineering workflows.

The repository currently combines three layers:

- a native SwiftUI app scaffold for the desktop client
- architecture research for a Goose-based execution model
- YAML examples for agent and workflow definitions

## Current Scope

This repository is an early foundation, not a finished product. The app is still close to the default SwiftUI + SwiftData template, while the documentation and examples define the intended direction:

- SwiftUI desktop UI for jobs, approvals, logs, and artifacts
- Goose as the execution runtime via `goosed` and REST/SSE
- a custom YAML DSL for agents and workflows
- deterministic integrations for release, build, and publishing steps

## Project Structure

```text
Chainworks Forge/
  Chainworks Forge.xcodeproj/   Xcode project
  Chainworks Forge/             SwiftUI application sources
  Chainworks ForgeTests/        Unit tests
  Chainworks ForgeUITests/      UI tests
  docs/                         Long-form documentation
  examples/                     YAML examples for agents and workflows
```

## Documentation

- `docs/research/goose_swiftui_agent_architecture_research.md` contains the main architecture research document.
- `examples/agents/` contains standalone agent YAML examples.
- `examples/workflows/` contains standalone workflow YAML examples.

## Development

Requirements:

- macOS
- Xcode 26.3 or newer

Open in Xcode:

```bash
open "Chainworks Forge.xcodeproj"
```

Build from the command line:

```bash
xcodebuild -project "Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS"
```

## Status

The repository is currently focused on shaping the architecture, repository structure, and orchestration model before implementation expands beyond the initial SwiftUI template.
