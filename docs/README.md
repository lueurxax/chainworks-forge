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
8. [reference/execution-truth-and-recovery.md](reference/execution-truth-and-recovery.md) — canonical outcomes, stage-owned recovery evidence, approval restore, and report/recovery read truth
9. [reference/output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md) — catalog-backed output contracts, aggregate summary hardening, failed-stage evidence, same-run retry truth, Tier 1 declarative enforcement, and bounded proposal compaction
10. [reference/live-provider-execution-slice.md](reference/live-provider-execution-slice.md) — live proposal loop
11. [reference/goose-server-transport.md](reference/goose-server-transport.md) — Goose transport
12. [reference/operator-experience.md](reference/operator-experience.md) — operator shell baseline
13. [reference/provider-platform.md](reference/provider-platform.md) — provider/settings baseline
14. [reference/run-control.md](reference/run-control.md) — stop/cancel truth and settlement
15. [reference/project-workspace-contract.md](reference/project-workspace-contract.md) — idea-owned workspace and frozen run contract
16. [reference/provider-binding-truth.md](reference/provider-binding-truth.md) — frozen provider/model provenance truth
17. [reference/idea-lifecycle.md](reference/idea-lifecycle.md) — archive and restore baseline
18. [reference/goose-provider-remediation.md](reference/goose-provider-remediation.md) — Goose-backed provider remediation path
19. [reference/live-workflow-map.md](reference/live-workflow-map.md) — run-detail topology baseline
20. [reference/ui-quality-and-polish.md](reference/ui-quality-and-polish.md) — implemented UI readability, accessibility, and bounded design-system hardening contract
21. [reference/design-system-and-brand-application.md](reference/design-system-and-brand-application.md) — implemented Forge token lane, brand assets, and shell/run/setup/recovery adoption
22. [reference/full-mvp-delivery.md](reference/full-mvp-delivery.md) — repo-backed worktree, implementation loop, manual release, evidence export
23. [reference/mvp-sign-off.md](reference/mvp-sign-off.md) — benchmark, launch gate, export hub, and current-head sign-off rules
24. [reference/current-system-baseline.md](reference/current-system-baseline.md) — current-head subsystem map and reusable review baseline
25. [reference/forge-steward.md](reference/forge-steward.md) — system health observer
26. [reference/workspace-isolation-risk.md](reference/workspace-isolation-risk.md) — isolation risk
27. [reference/test-suite-architecture.md](reference/test-suite-architecture.md) — Swift Testing suite structure, mock lanes, tags, and plans
28. [reference/test-gates.md](reference/test-gates.md) — layered local/CI test gates
29. [reference/agent-ui-test-execution.md](reference/agent-ui-test-execution.md) — how agents should run preview, XCUITest, and app-launched proof paths
30. [../examples/agents/agents.yaml](../examples/agents/agents.yaml) — agent catalog
31. [../examples/workflows/workflow.yaml](../examples/workflows/workflow.yaml) — canonical workflow

## Status

- Owner: single-engineer working repo
- Last updated: 2026-03-31
- State: active — foundation, execution engine, execution-truth baseline, output-contract and failure-evidence slice, live Goose transport, UI quality slice, design-system adoption slice, full MVP delivery slice, MVP sign-off layer, and Steward V1 implemented

## Reference (implemented)

See [reference/README.md](reference/README.md) for the full index. Key docs:

- [reference/workflow-execution-engine.md](reference/workflow-execution-engine.md) — RunPlan compiler, orchestrator, executors, artifact management, resume
- [reference/execution-truth-and-recovery.md](reference/execution-truth-and-recovery.md) — Stable canonical outcome, recovery, and report/read-truth contract
- [reference/output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md) — Stable output-contract authority, failed-stage evidence, narrow recovery, and declarative contract enforcement
- [reference/goose-server-transport.md](reference/goose-server-transport.md) — GooseServerTransport, SSE mapping, session lifecycle, proven real connection
- [reference/operator-experience.md](reference/operator-experience.md) — Stable operator shell baseline and contracts
- [reference/provider-platform.md](reference/provider-platform.md) — Stable multi-provider/settings/diagnostics baseline
- [reference/ui-quality-and-polish.md](reference/ui-quality-and-polish.md) — Stable UI readability, accessibility, and bounded design-system hardening baseline
- [reference/design-system-and-brand-application.md](reference/design-system-and-brand-application.md) — Stable Forge token lane, brand assets, and bounded visual rollout
- [reference/run-control.md](reference/run-control.md) — Stable stop/cancel and cancellation-settlement contract
- [reference/project-workspace-contract.md](reference/project-workspace-contract.md) — Stable idea-owned workspace and frozen run project contract
- [reference/provider-binding-truth.md](reference/provider-binding-truth.md) — Stable provider/model truth and provenance contract
- [reference/idea-lifecycle.md](reference/idea-lifecycle.md) — Stable archive/restore lifecycle for ideas
- [reference/goose-provider-remediation.md](reference/goose-provider-remediation.md) — Stable Goose-backed provider verification/remediation flow
- [reference/live-workflow-map.md](reference/live-workflow-map.md) — Stable workflow topology and agent-activity surface
- [reference/full-mvp-delivery.md](reference/full-mvp-delivery.md) — Stable repo-backed delivery slice: worktrees, implementation loop, manual release, evidence export
- [reference/mvp-sign-off.md](reference/mvp-sign-off.md) — Stable benchmark, recovery/export, and launch-gate sign-off contract
- [reference/current-system-baseline.md](reference/current-system-baseline.md) — Stable current-head subsystem map for review and proposal dependency normalization
- [reference/forge-steward.md](reference/forge-steward.md) — V1 observer: metrics, anomaly detection, cohorting, triggers
- [reference/live-provider-execution-slice.md](reference/live-provider-execution-slice.md) — Live proposal loop runtime contract
- [reference/domain-model.md](reference/domain-model.md) — SwiftData models and persistence
- [reference/architecture-decisions.md](reference/architecture-decisions.md) — AD log
- [reference/test-suite-architecture.md](reference/test-suite-architecture.md) — Stable Swift Testing suite structure and migration baseline
- [reference/test-gates.md](reference/test-gates.md) — layered fast/UI/proposal/full gates
- [reference/agent-ui-test-execution.md](reference/agent-ui-test-execution.md) — Stable agent-facing UI execution rules and fallback proof paths

## Proposals (active, not yet implemented)

- [proposals/018-agent-session-lineage-reuse-and-operator-reset.md](proposals/018-agent-session-lineage-reuse-and-operator-reset.md) — reusable agent session lineage within one run plus explicit operator reset

## Evidence

- [evidence/goose-server-transport-verification.md](evidence/goose-server-transport-verification.md) -- Goose server transport verification record
- [evidence/execution-truth-and-recovery-proof.md](evidence/execution-truth-and-recovery-proof.md) -- consolidated implementation/proof status for the execution-truth and recovery slice
- [evidence/output-contracts-failure-evidence-and-recovery-proof.md](evidence/output-contracts-failure-evidence-and-recovery-proof.md) -- consolidated implementation/proof status for output contracts, failure evidence, and narrow recovery
- [evidence/ui-quality-and-polish-proof.md](evidence/ui-quality-and-polish-proof.md) -- consolidated implementation/proof status for the UI quality and visual polish slice
- [evidence/design-system-and-brand-application-proof.md](evidence/design-system-and-brand-application-proof.md) -- consolidated implementation/proof status for the design-system and brand-application slice
- [evidence/full-mvp-delivery-proof.md](evidence/full-mvp-delivery-proof.md) -- consolidated implementation/proof status for the repo-backed delivery slice
- [evidence/mvp-sign-off-proof.md](evidence/mvp-sign-off-proof.md) -- consolidated implementation/proof status for MVP hardening and sign-off
- [evidence/live_goose_connection_proof.json](evidence/live_goose_connection_proof.json) -- raw evidence JSON

## Research

- [research/chainworks_core_idea.md](research/chainworks_core_idea.md)
- [research/goose_swiftui_agent_architecture_research.md](research/goose_swiftui_agent_architecture_research.md)

## Product

- [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP problem statement
