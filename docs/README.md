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

## Features

The canonical source of truth for implemented behavior is [`reference/`](reference). High-signal entry points grouped by feature area:

### Foundations

- [reference/domain-model.md](reference/domain-model.md) — SwiftData models and persistence
- [reference/yaml-dsl-parser.md](reference/yaml-dsl-parser.md) — YAML workflow and agent catalog parsing and validation
- [reference/architecture-decisions.md](reference/architecture-decisions.md) — Architectural decision log

### Execution engine

- [reference/workflow-execution-engine.md](reference/workflow-execution-engine.md) — RunPlan compiler, orchestrator, executors, artifact management, resume, declarative workflow authority, and conflict resolution (Proposal 017)
- [reference/artifact-discovery-and-settlement-optimization.md](reference/artifact-discovery-and-settlement-optimization.md) — Bounded discovery, settlement pipeline, pre-prompt metadata (P053)
- [reference/runtime-contract.md](reference/runtime-contract.md) — Frozen run snapshots, state machines, artifact model
- [reference/execution-truth-and-recovery.md](reference/execution-truth-and-recovery.md) — Terminal outcomes, atomic transition settlement, cursor-driven resume, recovery precedence, host interruption, workflow conflict recovery, and startup recovery progress
- [reference/rust-control-plane.md](reference/rust-control-plane.md) — Rust + SQLite daemon: architecture, crate layout, persistence, boundaries, capacity-aware scheduling, write serialization, provider toolchain homes, and generated-state housekeeping

### Agents, skills, and MCP

- [reference/skill-resolution-and-runtime-integration.md](reference/skill-resolution-and-runtime-integration.md) — Skill resolution, specialization, runtime injection
- [reference/per-agent-mcp-policy-and-runtime-validation.md](reference/per-agent-mcp-policy-and-runtime-validation.md) — Per-agent MCP intent, runtime validation, persisted MCP truth
- [reference/failed-stage-evidence-delivery-preflight-and-mcp-resolution.md](reference/failed-stage-evidence-delivery-preflight-and-mcp-resolution.md) — Failed-stage evidence, delivery preflight, MCP resolution

### ACP transport and sessions

- [reference/acp-runtime-transport.md](reference/acp-runtime-transport.md) — ACP transport contract, adapter families (Claude/Gemini/Codex/Auggie/Junie), runtime selection, and capacity management
- [reference/session-lineage-reuse-and-operator-reset.md](reference/session-lineage-reuse-and-operator-reset.md) — Session reuse, invocation-owner keys, binding fingerprints, context budget, checkpoint rehydration, operator reset
- [reference/live-provider-execution-slice.md](reference/live-provider-execution-slice.md) — Live proposal loop runtime contract

### Outputs, contracts, and feedback

- [reference/structured-output-envelope-and-contract-validation.md](reference/structured-output-envelope-and-contract-validation.md) — Named envelopes, contract binding, validation, failure substrate
- [reference/output-contracts-failure-evidence-and-recovery.md](reference/output-contracts-failure-evidence-and-recovery.md) — Catalog-backed output contracts, implementation self-assessment and handoff, failed-stage evidence, narrow recovery
- [reference/proposal-loop-feedback-fidelity-and-rereview.md](reference/proposal-loop-feedback-fidelity-and-rereview.md) — Review-corpus fidelity, backlog carry-forward, targeted rereview
- [reference/context-strategy-and-experiment-framework.md](reference/context-strategy-and-experiment-framework.md) — Strategy-profile freezing, handoff compilation, normalized telemetry

### Run control, delivery, and release

- [reference/run-control.md](reference/run-control.md) — Stop/cancel, two-phase cancellation settlement, terminal-history rules
- [reference/release-gate.md](reference/release-gate.md) — Manual release gate: post-approval execution, native git/publish, delivery receipts
- [reference/full-mvp-delivery.md](reference/full-mvp-delivery.md) — Repo-backed delivery slice: worktrees, implementation loop, manual release, assessment and handoff
- [reference/project-workspace-contract.md](reference/project-workspace-contract.md) — Idea-owned workspace and frozen run project contract
- [reference/provider-binding-truth.md](reference/provider-binding-truth.md) — Provider/model truth and provenance contract
- [reference/mvp-sign-off.md](reference/mvp-sign-off.md) — Benchmark, recovery/export, launch-gate sign-off

### Operator experience

- [reference/operator-experience.md](reference/operator-experience.md) — Operator shell baseline, backpressure visibility, and host interruption labels
- [reference/p031-operator-write-path-guide.md](reference/p031-operator-write-path-guide.md) — External workflow mapping for removed UI write controls (P031)
- [reference/query-projections-and-client-consumption-contract.md](reference/query-projections-and-client-consumption-contract.md) — GraphQL projection read contract for the thin macOS client
- [reference/run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md) — Segmented run shells, focused timeline, artifact hierarchy
- [reference/live-workflow-map.md](live-workflow-map.md) — Workflow topology and agent-activity surface
- [reference/artifact-content-rendering.md](artifact-content-rendering.md) — Unified read-only markdown/JSON rendering
- [reference/provider-platform.md](provider-platform.md) — Multi-provider/settings/diagnostics baseline and capacity caps
- [reference/idea-lifecycle.md](reference/idea-lifecycle.md) — Archive/restore lifecycle for ideas
- [reference/ui-quality-and-polish.md](reference/ui-quality-and-polish.md) — UI readability, accessibility, shared status semantics
- [reference/design-system-and-brand-application.md](reference/design-system-and-brand-application.md) — Forge token lane, brand assets, visual rollout

### System health

- [reference/forge-steward.md](reference/forge-steward.md) — Forge Steward V1 observer: metrics, anomaly detection, cohorting, triggers
- [reference/steward-analysis-system.md](reference/steward-analysis-system.md) — Rust Steward: frozen cohort owners, deterministic analysis, triggers

### Testing

- [reference/test-suite-architecture.md](reference/test-suite-architecture.md) — Swift Testing suite structure
- [reference/test-gates.md](reference/test-gates.md) — Layered fast/UI/focused/full gates
- [reference/agent-ui-test-execution.md](reference/agent-ui-test-execution.md) — Agent-facing UI execution rules

### Risk analysis

- [reference/workspace-isolation-risk.md](reference/workspace-isolation-risk.md) — Workspace-bound execution risk and guardrails

## Active Proposals

Design intent and work-in-progress lives under [`proposals/`](proposals). When a proposal reaches implemented/ready status, its content is folded into `reference/` and the proposal file is retired.

## Examples

Runnable agent catalogs and workflow presets live under [`../examples`](../examples):

- [../examples/README.md](../examples/README.md)
- [../examples/agents/agents.yaml](../examples/agents/agents.yaml)
- [../examples/agents/agents_with_gemini.yaml](../examples/agents/agents_with_gemini.yaml)
- [../examples/workflows/workflow.yaml](../examples/workflows/workflow.yaml)
- [../examples/workflows/proposal-loop-live.yaml](../examples/workflows/proposal-loop-live.yaml)
- [../examples/workflows/full-mvp-live.yaml](../examples/workflows/full-mvp-live.yaml)

## Research and Product

- [research/chainworks_core_idea.md](research/chainworks_core_idea.md) — product vision and positioning
- [research/goose_swiftui_agent_architecture_research.md](research/goose_swiftui_agent_architecture_research.md) — architecture research notes
- [ps/chainworks-forge-mvp.md](ps/chainworks-forge-mvp.md) — MVP problem statement

## Brand

- [brand/](brand) — brand assets and guidelines
