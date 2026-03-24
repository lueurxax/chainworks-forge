# Docs Index

Repository documentation for Chainworks Forge.

## Organization

- `reference/` — implemented system documentation: architecture, runtime contracts, engine, transport, steward
- `proposals/` — active proposals for features not yet implemented
- `evidence/` — integration proof artifacts (live Goose connection, etc.)
- `research/` — product vision, architecture investigations, comparative analysis
- `ps/` — product/problem statements and MVP framing

## Reading Order

1. [research/chainworks_core_idea.md](research/chainworks_core_idea.md) — product vision
2. [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP problem statement
3. [reference/domain-model.md](reference/domain-model.md) — persistence layer
4. [reference/yaml-dsl-parser.md](reference/yaml-dsl-parser.md) — YAML DSL
5. [reference/architecture-decisions.md](reference/architecture-decisions.md) — AD log
6. [reference/workflow-execution-engine.md](reference/workflow-execution-engine.md) — execution engine
7. [reference/runtime-contract.md](reference/runtime-contract.md) — runtime contracts
8. [reference/live-provider-execution-slice.md](reference/live-provider-execution-slice.md) — live proposal loop
9. [reference/goose-server-transport.md](reference/goose-server-transport.md) — Goose transport
10. [reference/forge-steward.md](reference/forge-steward.md) — system health observer
11. [reference/workspace-isolation-risk.md](reference/workspace-isolation-risk.md) — isolation risk
12. [../examples/agents/agents.yaml](../examples/agents/agents.yaml) — agent catalog
13. [../examples/workflows/workflow.yaml](../examples/workflows/workflow.yaml) — canonical workflow

## Status

- Owner: single-engineer working repo
- Last updated: 2026-03-24
- State: active — foundation, execution engine, live Goose transport, and Steward V1 implemented

## Reference (implemented)

See [reference/README.md](reference/README.md) for the full index. Key docs:

- [reference/workflow-execution-engine.md](reference/workflow-execution-engine.md) — RunPlan compiler, orchestrator, executors, artifact management, resume
- [reference/goose-server-transport.md](reference/goose-server-transport.md) — GooseServerTransport, SSE mapping, session lifecycle, proven real connection
- [reference/forge-steward.md](reference/forge-steward.md) — V1 observer: metrics, anomaly detection, cohorting, triggers
- [reference/live-provider-execution-slice.md](reference/live-provider-execution-slice.md) — Live proposal loop runtime contract
- [reference/domain-model.md](reference/domain-model.md) — SwiftData models and persistence
- [reference/architecture-decisions.md](reference/architecture-decisions.md) — AD log

## Proposals (active, not yet implemented)

- [proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md](proposals/006-provider-expansion-settings-diagnostics-and-mvp-pilot-readiness.md) — multi-provider routing, settings, diagnostics
- [proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md](proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md) — worktrees, implementation loop, release, dogfooding
- [proposals/008-mvp-hardening-and-sign-off.md](proposals/008-mvp-hardening-and-sign-off.md) — MVP validation, boundary freeze, recovery UX, launch gate

## Evidence

- [evidence/goose-server-transport-verification.md](evidence/goose-server-transport-verification.md) -- Goose server transport verification record
- [evidence/live_goose_connection_proof.json](evidence/live_goose_connection_proof.json) -- raw evidence JSON

## Research

- [research/chainworks_core_idea.md](research/chainworks_core_idea.md)
- [research/goose_swiftui_agent_architecture_research.md](research/goose_swiftui_agent_architecture_research.md)

## Product

- [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP problem statement
