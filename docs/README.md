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
10. [reference/operator-experience.md](reference/operator-experience.md) — operator shell baseline
11. [reference/provider-platform.md](reference/provider-platform.md) — provider/settings baseline
12. [reference/run-control.md](reference/run-control.md) — stop/cancel truth and settlement
13. [reference/project-workspace-contract.md](reference/project-workspace-contract.md) — idea-owned workspace and frozen run contract
14. [reference/provider-binding-truth.md](reference/provider-binding-truth.md) — frozen provider/model provenance truth
15. [reference/idea-lifecycle.md](reference/idea-lifecycle.md) — archive and restore baseline
16. [reference/goose-provider-remediation.md](reference/goose-provider-remediation.md) — Goose-backed provider remediation path
17. [reference/live-workflow-map.md](reference/live-workflow-map.md) — run-detail topology baseline
18. [reference/full-mvp-delivery.md](reference/full-mvp-delivery.md) — repo-backed worktree, implementation loop, manual release, evidence export
19. [reference/forge-steward.md](reference/forge-steward.md) — system health observer
20. [reference/workspace-isolation-risk.md](reference/workspace-isolation-risk.md) — isolation risk
21. [reference/test-suite-architecture.md](reference/test-suite-architecture.md) — Swift Testing suite structure, mock lanes, tags, and plans
22. [reference/test-gates.md](reference/test-gates.md) — layered local/CI test gates
23. [reference/agent-ui-test-execution.md](reference/agent-ui-test-execution.md) — how agents should run preview, XCUITest, and app-launched proof paths
24. [../examples/agents/agents.yaml](../examples/agents/agents.yaml) — agent catalog
25. [../examples/workflows/workflow.yaml](../examples/workflows/workflow.yaml) — canonical workflow

## Status

- Owner: single-engineer working repo
- Last updated: 2026-03-28
- State: active — foundation, execution engine, live Goose transport, full MVP delivery slice, and Steward V1 implemented

## Reference (implemented)

See [reference/README.md](reference/README.md) for the full index. Key docs:

- [reference/workflow-execution-engine.md](reference/workflow-execution-engine.md) — RunPlan compiler, orchestrator, executors, artifact management, resume
- [reference/goose-server-transport.md](reference/goose-server-transport.md) — GooseServerTransport, SSE mapping, session lifecycle, proven real connection
- [reference/operator-experience.md](reference/operator-experience.md) — Stable operator shell baseline and contracts
- [reference/provider-platform.md](reference/provider-platform.md) — Stable multi-provider/settings/diagnostics baseline
- [reference/run-control.md](reference/run-control.md) — Stable stop/cancel and cancellation-settlement contract
- [reference/project-workspace-contract.md](reference/project-workspace-contract.md) — Stable idea-owned workspace and frozen run project contract
- [reference/provider-binding-truth.md](reference/provider-binding-truth.md) — Stable provider/model truth and provenance contract
- [reference/idea-lifecycle.md](reference/idea-lifecycle.md) — Stable archive/restore lifecycle for ideas
- [reference/goose-provider-remediation.md](reference/goose-provider-remediation.md) — Stable Goose-backed provider verification/remediation flow
- [reference/live-workflow-map.md](reference/live-workflow-map.md) — Stable workflow topology and agent-activity surface
- [reference/full-mvp-delivery.md](reference/full-mvp-delivery.md) — Stable repo-backed delivery slice: worktrees, implementation loop, manual release, evidence export
- [reference/forge-steward.md](reference/forge-steward.md) — V1 observer: metrics, anomaly detection, cohorting, triggers
- [reference/live-provider-execution-slice.md](reference/live-provider-execution-slice.md) — Live proposal loop runtime contract
- [reference/domain-model.md](reference/domain-model.md) — SwiftData models and persistence
- [reference/architecture-decisions.md](reference/architecture-decisions.md) — AD log
- [reference/test-suite-architecture.md](reference/test-suite-architecture.md) — Stable Swift Testing suite structure and migration baseline
- [reference/test-gates.md](reference/test-gates.md) — layered fast/UI/proposal/full gates
- [reference/agent-ui-test-execution.md](reference/agent-ui-test-execution.md) — Stable agent-facing UI execution rules and fallback proof paths

## Proposals (active, not yet implemented)

- [proposals/008-mvp-hardening-and-sign-off.md](proposals/008-mvp-hardening-and-sign-off.md) — MVP validation, boundary freeze, recovery UX, launch gate
- [proposals/014-design-system-adoption-and-brand-application.md](proposals/014-design-system-adoption-and-brand-application.md) — adopt Design Kit v1 across the app shell and operator surfaces
- [proposals/012-ui-quality-audit-and-visual-polish.md](proposals/012-ui-quality-audit-and-visual-polish.md) — operator-surface visual consistency, density, and polish
- [proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md](proposals/013-output-contract-alignment-retry-truth-and-failure-evidence-hardening.md) — stage-output contract alignment, retry truth, failed-stage evidence, and recovery hardening

## Evidence

- [evidence/goose-server-transport-verification.md](evidence/goose-server-transport-verification.md) -- Goose server transport verification record
- [evidence/full-mvp-delivery-proof.md](evidence/full-mvp-delivery-proof.md) -- consolidated implementation/proof status for the repo-backed delivery slice
- [evidence/live_goose_connection_proof.json](evidence/live_goose_connection_proof.json) -- raw evidence JSON

## Research

- [research/chainworks_core_idea.md](research/chainworks_core_idea.md)
- [research/goose_swiftui_agent_architecture_research.md](research/goose_swiftui_agent_architecture_research.md)

## Product

- [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP problem statement
