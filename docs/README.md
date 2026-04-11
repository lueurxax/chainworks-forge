# Docs Index

Repository documentation for Chainworks Forge.

## Start Here

If you are new to the repo, read these in order:

1. [../README.md](../README.md) — product summary, setup, test gates, and current status
2. [research/chainworks_core_idea.md](research/chainworks_core_idea.md) — product vision and positioning
3. [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP scope and requirements
4. [reference/current-system-baseline.md](reference/current-system-baseline.md) — current-head subsystem map
5. [reference/README.md](reference/README.md) — implemented-system reference index
6. [../examples/README.md](../examples/README.md) — runnable YAML examples and presets

## Status

- Owner: single-engineer working repo
- Last updated: 2026-04-08
- State: active — foundation, execution engine, skill runtime integration, MCP policy/runtime validation, ACP-shaped transport, execution-truth baseline, output-contract and failure-evidence slice, session-lineage reuse slice, context-strategy framework slice, proposal-loop feedback-fidelity slice, live transport layer, UI quality slice, design-system adoption slice, unified artifact rendering, full MVP delivery slice, MVP sign-off layer, and Steward V1 implemented

## Implemented Reference Docs

The canonical source of truth for implemented behavior is [`reference/`](reference). Use [`reference/README.md`](reference/README.md) for the full index. High-signal entry points:

- [reference/current-system-baseline.md](reference/current-system-baseline.md) — current-head subsystem map for review and planning work
- [reference/workflow-execution-engine.md](reference/workflow-execution-engine.md) — RunPlan compiler, orchestrator, executors, artifact management, resume
- [reference/skill-resolution-and-runtime-integration.md](reference/skill-resolution-and-runtime-integration.md) — Stable skill resolution, specialization, runtime injection, and frozen skill truth
- [reference/per-agent-mcp-policy-and-runtime-validation.md](reference/per-agent-mcp-policy-and-runtime-validation.md) — Stable per-agent MCP intent, runtime validation, and persisted MCP truth
- [reference/acp-runtime-transport.md](reference/acp-runtime-transport.md) — Stable ACP-only transport contract, adapter families, and persisted runtime truth
- [reference/execution-truth-and-recovery.md](reference/execution-truth-and-recovery.md) — Stable canonical outcome, recovery, and report/read-truth contract
- [reference/output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md) — Stable output-contract authority, failed-stage evidence, narrow recovery, and declarative contract enforcement
- [reference/session-lineage-reuse-and-operator-reset.md](reference/session-lineage-reuse-and-operator-reset.md) — Stable per-run session reuse, generation history, checkpoint refresh, and shell-owned reset contract
- [reference/context-strategy-and-experiment-framework.md](reference/context-strategy-and-experiment-framework.md) — Stable strategy-profile freezing, handoff compilation, lazy evidence, normalized telemetry, and shell-owned recommendation contract
- [reference/proposal-loop-feedback-fidelity-and-rereview.md](reference/proposal-loop-feedback-fidelity-and-rereview.md) — Stable proposal-loop review-corpus fidelity, backlog carry-forward, writer coverage, and targeted-rereview contract
- [reference/operator-experience.md](reference/operator-experience.md) — Stable operator shell baseline and contracts
- [reference/run-surface-information-architecture-and-artifact-hierarchy.md](reference/run-surface-information-architecture-and-artifact-hierarchy.md) — Stable segmented run-shell IA, pane routing, focused timeline, and shared artifact browsing contract
- [reference/provider-platform.md](reference/provider-platform.md) — Stable multi-provider/settings/diagnostics baseline
- [reference/ui-quality-and-polish.md](reference/ui-quality-and-polish.md) — Stable UI readability, accessibility, and bounded design-system hardening baseline
- [reference/design-system-and-brand-application.md](reference/design-system-and-brand-application.md) — Stable Forge token lane, brand assets, and bounded visual rollout
- [reference/run-control.md](reference/run-control.md) — Stable stop/cancel and cancellation-settlement contract
- [reference/project-workspace-contract.md](reference/project-workspace-contract.md) — Stable idea-owned workspace and frozen run project contract
- [reference/artifact-content-rendering.md](reference/artifact-content-rendering.md) — Stable unified read-only markdown/json artifact rendering contract
- [reference/provider-binding-truth.md](reference/provider-binding-truth.md) — Stable provider/model truth and provenance contract
- [reference/idea-lifecycle.md](reference/idea-lifecycle.md) — Stable archive/restore lifecycle for ideas
- [reference/live-workflow-map.md](reference/live-workflow-map.md) — Stable workflow topology and agent-activity surface
- [reference/full-mvp-delivery.md](reference/full-mvp-delivery.md) — Stable repo-backed delivery slice: worktrees, implementation loop, manual release, evidence export
- [reference/mvp-sign-off.md](reference/mvp-sign-off.md) — Stable benchmark, recovery/export, and launch-gate sign-off contract
- [reference/forge-steward.md](reference/forge-steward.md) — V1 observer: metrics, anomaly detection, cohorting, triggers
- [reference/live-provider-execution-slice.md](reference/live-provider-execution-slice.md) — Live proposal loop runtime contract
- [reference/domain-model.md](reference/domain-model.md) — SwiftData models and persistence
- [reference/architecture-decisions.md](reference/architecture-decisions.md) — AD log
- [reference/test-suite-architecture.md](reference/test-suite-architecture.md) — Stable Swift Testing suite structure and migration baseline
- [reference/test-gates.md](reference/test-gates.md) — layered fast/UI/proposal/full gates
- [reference/agent-ui-test-execution.md](reference/agent-ui-test-execution.md) — Stable agent-facing UI execution rules and fallback proof paths

## Active Proposals

Active work that is not yet fully promoted into `reference/` lives under [`proposals/`](proposals):

- [proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md](proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md)
- [proposals/020-dynamic-cycle-addition.md](proposals/020-dynamic-cycle-addition.md)
- [proposals/021-run-transition-notifications-and-attention-routing.md](proposals/021-run-transition-notifications-and-attention-routing.md)
- [proposals/023-loop-improvement-analytics-and-iteration-progression.md](proposals/023-loop-improvement-analytics-and-iteration-progression.md)
- [proposals/031-thin-ui-rewrite-over-projections-and-mcp.md](proposals/031-thin-ui-rewrite-over-projections-and-mcp.md)
- [proposals/032-polish-stabilization-and-productization-backlog.md](proposals/032-polish-stabilization-and-productization-backlog.md)
- [proposals/034-clean-yaml-runtime-transport-normalization.md](proposals/034-clean-yaml-runtime-transport-normalization.md)

## Examples

Runnable agent catalogs and workflow presets live under [`../examples`](../examples):

- [../examples/README.md](../examples/README.md)
- [../examples/agents/agents.yaml](../examples/agents/agents.yaml)
- [../examples/agents/agents_with_gemini.yaml](../examples/agents/agents_with_gemini.yaml)
- [../examples/workflows/workflow.yaml](../examples/workflows/workflow.yaml)
- [../examples/workflows/proposal-loop-live.yaml](../examples/workflows/proposal-loop-live.yaml)
- [../examples/workflows/full-mvp-live.yaml](../examples/workflows/full-mvp-live.yaml)

## Evidence And Proof

- [evidence/execution-truth-and-recovery-proof.md](evidence/execution-truth-and-recovery-proof.md) -- consolidated implementation/proof status for the execution-truth and recovery slice
- [evidence/output-contracts-failure-evidence-and-recovery-proof.md](evidence/output-contracts-failure-evidence-and-recovery-proof.md) -- consolidated implementation/proof status for output contracts, failure evidence, and narrow recovery
- [evidence/session-lineage-reuse-and-operator-reset-proof.md](evidence/session-lineage-reuse-and-operator-reset-proof.md) -- consolidated implementation/proof status for session-lineage reuse, checkpointing, and operator reset
- [evidence/033-remove-goose-from-canonical-transport-and-simplify-runtime-proof.md](evidence/033-remove-goose-from-canonical-transport-and-simplify-runtime-proof.md) -- proof of canonical ACP transport simplification and legacy settings compatibility
- [evidence/035-atomic-transition-settlement-and-durable-resume-cursor-proof.md](evidence/035-atomic-transition-settlement-and-durable-resume-cursor-proof.md) -- proof of cursor-authored transition settlement and deterministic resume
- [evidence/030-acp-second-wave-runtime-profiles-proof.md](evidence/030-acp-second-wave-runtime-profiles-proof.md) -- proof of second-wave provider profile and adapter-aware runtime readiness behavior
- [evidence/context-strategy-and-experiment-framework-proof.md](evidence/context-strategy-and-experiment-framework-proof.md) -- consolidated implementation/proof status for context strategies, lazy evidence, normalized telemetry, and strategy recommendation output
- [evidence/proposal-loop-feedback-fidelity-and-rereview-proof.md](evidence/proposal-loop-feedback-fidelity-and-rereview-proof.md) -- consolidated implementation/proof status for proposal-loop review fidelity, score-lift backlog, writer coverage, and targeted rereview
- [evidence/run-surface-information-architecture-and-artifact-hierarchy-proof.md](evidence/run-surface-information-architecture-and-artifact-hierarchy-proof.md) -- consolidated implementation/proof status for segmented run surfaces, focused timeline, and hierarchical artifact browsing
- [evidence/ui-quality-and-polish-proof.md](evidence/ui-quality-and-polish-proof.md) -- consolidated implementation/proof status for the UI quality and visual polish slice
- [evidence/design-system-and-brand-application-proof.md](evidence/design-system-and-brand-application-proof.md) -- consolidated implementation/proof status for the design-system and brand-application slice
- [evidence/full-mvp-delivery-proof.md](evidence/full-mvp-delivery-proof.md) -- consolidated implementation/proof status for the repo-backed delivery slice
- [evidence/mvp-sign-off-proof.md](evidence/mvp-sign-off-proof.md) -- consolidated implementation/proof status for MVP hardening and sign-off

## Research

- [research/chainworks_core_idea.md](research/chainworks_core_idea.md)
- [research/goose_swiftui_agent_architecture_research.md](research/goose_swiftui_agent_architecture_research.md)

## Archive

- [archive/README.md](archive/README.md) — historical material that is intentionally retained outside the canonical reference stack

## Product

- [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP problem statement

## Historical Reviews

Historical reviews remain only for work that has not yet been fully promoted into `reference/`.
For current implemented behavior, prefer `reference/`.
