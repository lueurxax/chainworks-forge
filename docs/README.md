# Docs Index

Repository documentation is organized by purpose:

- `ps/` holds product/problem statements and MVP framing.
- `proposals/` holds implementation proposals and delivery slices.
- `reviews/` holds review outputs, evidence packs, and attached artifacts.
- `research/` holds product vision notes, long-form architecture notes, investigations, and comparative analysis.
- `reference/` holds implementation-oriented runtime contracts, state models, architecture decisions, and schema-level notes.

Reading order:

1. [research/chainworks_core_idea.md](research/chainworks_core_idea.md)
2. [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md)
3. [research/goose_swiftui_agent_architecture_research.md](research/goose_swiftui_agent_architecture_research.md)
4. [reference/domain-model.md](reference/domain-model.md)
5. [reference/yaml-dsl-parser.md](reference/yaml-dsl-parser.md)
6. [reference/architecture-decisions.md](reference/architecture-decisions.md)
7. [reference/runtime-contract.md](reference/runtime-contract.md)
8. [reference/workspace-isolation-risk.md](reference/workspace-isolation-risk.md)
9. [proposals/002-workflow-execution-engine.md](proposals/002-workflow-execution-engine.md)
10. [proposals/003-forge-steward-sdlc-health-and-adaptation.md](proposals/003-forge-steward-sdlc-health-and-adaptation.md)
11. [../examples/agents/agents.yaml](../examples/agents/agents.yaml)
12. [../examples/workflows/workflow.yaml](../examples/workflows/workflow.yaml)

Status:

- Owner: single-engineer working repo
- Last updated: 2026-03-22
- State: active, foundation + core runtime engine implemented, UI still inspection-first

## Reference (implemented)

- [reference/domain-model.md](reference/domain-model.md) — SwiftData persistence layer, run/stage/agent state, approvals, artifacts, Steward records, provenance, drift detection
- [reference/yaml-dsl-parser.md](reference/yaml-dsl-parser.md) — YAML parsing, validation, compact workflow normalization, provenance hashing, verification scaffold UI
- [reference/architecture-decisions.md](reference/architecture-decisions.md) — Key architecture decisions across foundation and runtime slices
- [reference/runtime-contract.md](reference/runtime-contract.md) — Frozen run snapshots, state machines, artifact model, execution and resume contracts
- [reference/workspace-isolation-risk.md](reference/workspace-isolation-risk.md) — Worktree isolation risk analysis
- [reference/README.md](reference/README.md) — Reference section overview

## Product

- [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP problem statement

## Proposals

- [proposals/002-workflow-execution-engine.md](proposals/002-workflow-execution-engine.md) — workflow execution engine slice
- [proposals/003-forge-steward-sdlc-health-and-adaptation.md](proposals/003-forge-steward-sdlc-health-and-adaptation.md) — Steward analysis and adaptation slice

## Reviews

- [reviews/001-proposal-002-gate.md](reviews/001-proposal-002-gate.md) — Go/no-go gate for Proposal 002
- [reviews/002-workflow-execution-engine-review.md](reviews/002-workflow-execution-engine-review.md)
- [reviews/002-workflow-execution-engine-evidence-pack.md](reviews/002-workflow-execution-engine-evidence-pack.md)
- [reviews/003-forge-steward-sdlc-health-and-adaptation-review.md](reviews/003-forge-steward-sdlc-health-and-adaptation-review.md)
- [reviews/003-forge-steward-sdlc-health-and-adaptation-evidence-pack.md](reviews/003-forge-steward-sdlc-health-and-adaptation-evidence-pack.md)

## Research

- [research/chainworks_core_idea.md](research/chainworks_core_idea.md)
- [research/goose_swiftui_agent_architecture_research.md](research/goose_swiftui_agent_architecture_research.md)
