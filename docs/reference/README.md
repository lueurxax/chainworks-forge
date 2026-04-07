# Reference

Implementation-oriented reference docs for Chainworks Forge.

If you need a current-head orientation first, start with [current-system-baseline.md](current-system-baseline.md), then read the relevant subsystem documents below.

## Foundation Layer

- [domain-model.md](domain-model.md) — SwiftData persistence: `Idea`, `Run`, `StageExecution`, `AgentExecution`, `Approval`, `Artifact`, Steward records, provenance snapshots, drift detection, cost tracking
- [yaml-dsl-parser.md](yaml-dsl-parser.md) — YAML parsing, validation (10 check categories), compact workflow normalization, provenance hashing, verification scaffold
- [architecture-decisions.md](architecture-decisions.md) — Key AD decisions: CodingKeys, single-active-run, drift detection, snapshot storage, integer cost, derived currentStageID

## Execution Engine

- [workflow-execution-engine.md](workflow-execution-engine.md) — RunPlan compiler, Workflow Orchestrator, Agent Executor protocol, Artifact Manager, Transition Evaluator, Resume Manager, Execution Service
- [runtime-contract.md](runtime-contract.md) — Frozen run snapshots, state machines, artifact model, storage boundaries, resume/retry rules
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) — Canonical terminal outcomes, stage-owned recovery evidence, approval restore, runtime binding truth, and report/recovery read precedence
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md) — Catalog-backed output contracts, aggregate summary hardening, failed-stage evidence, same-run retry truth, declarative Tier 1 enforcement, and bounded proposal compaction
- [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md) — Reusable session lineage within one run, immutable generation history, budget-driven compaction, checkpoint rehydration, and shell-owned per-agent reset
- [context-strategy-and-experiment-framework.md](context-strategy-and-experiment-framework.md) — Frozen strategy profiles, handoff compilation, lazy evidence, normalized strategy telemetry, and shell-owned recommendation output
- [proposal-loop-feedback-fidelity-and-rereview.md](proposal-loop-feedback-fidelity-and-rereview.md) — Review-corpus bundle ownership, score-lift backlog, writer coverage, targeted rereview, and proposal-growth discipline for the live proposal loop

## Live Execution

- [live-provider-execution-slice.md](live-provider-execution-slice.md) — Live proposal-loop slice: runtime boundary, safety contract, approval flow, app surfaces, verification
- [goose-server-transport.md](goose-server-transport.md) — GooseServerTransport adapter: goosed API contract, SSE event mapping, session lifecycle, executor pipeline, proven real Goose connection
- [operator-experience.md](operator-experience.md) — Stable operator shell baseline: Runs Home, reports, recovery, comparison, artifact inspection, notifications
- [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md) — Segmented run shells, pane routing, focused timeline, canonical artifact hierarchy, and metadata-demotion continuity
- [artifact-content-rendering.md](artifact-content-rendering.md) — Stable unified rendering contract for read-only markdown and JSON artifacts
- [provider-platform.md](provider-platform.md) — Stable provider/settings baseline: registry, adapters, settings, preflight, receipts, first-run and pilot surfaces
- [design-system-and-brand-application.md](design-system-and-brand-application.md) — Stable Forge token lane, bounded brand assets, and shell/run/setup/recovery visual adoption
- [ui-quality-and-polish.md](ui-quality-and-polish.md) — Stable UI readability, bounded accessibility, shared status semantics, and owner-surface proof contract
- [run-control.md](run-control.md) — Stop vs archive boundary, cancellation settlement, operator-visible cancelling/cancelled truth
- [project-workspace-contract.md](project-workspace-contract.md) — `requires_project_access`, idea-owned workspace root, frozen run workspace contract
- [provider-binding-truth.md](provider-binding-truth.md) — Frozen provider/model truth, provenance, and cross-family mismatch handling
- [idea-lifecycle.md](idea-lifecycle.md) — Active vs archived idea contract, archive/restore eligibility, cross-surface truth
- [goose-provider-remediation.md](goose-provider-remediation.md) — Goose-first Codex/Claude remediation path, assistant, handshake probe, evidence panel
- [live-workflow-map.md](live-workflow-map.md) — Run-detail topology, state vocabulary, handoff counters, loop/fallback visibility
- [full-mvp-delivery.md](full-mvp-delivery.md) — Repo-backed `Full MVP Live` slice: frozen delivery config, dedicated worktree, implementation loop, manual release, evidence export
- [mvp-sign-off.md](mvp-sign-off.md) — benchmark, replayable `GO/HOLD`, export hub, approval relaunch, and current-head sign-off contract
- [current-system-baseline.md](current-system-baseline.md) — current-head subsystem map and reusable baseline for review and planning work

## Test Strategy

- [test-suite-architecture.md](test-suite-architecture.md) — Swift Testing unit-suite structure, conventions, mock lanes, tags, plans, residual gaps
- [test-gates.md](test-gates.md) — Layered local and CI execution gates, gate ownership, crash-aware runner behavior
- [agent-ui-test-execution.md](agent-ui-test-execution.md) — how agents should run preview review, focused XCUITest, and app-launched UI proof flows

## System Health

- [forge-steward.md](forge-steward.md) — Forge Steward V1 (Observer): deterministic metrics, anomaly detection, cohorting, dossier building, trigger mechanisms

## Evidence

- [../evidence/execution-truth-and-recovery-proof.md](../evidence/execution-truth-and-recovery-proof.md) — consolidated implementation/proof story for the execution-truth and recovery slice
- [../evidence/output-contracts-failure-evidence-and-recovery-proof.md](../evidence/output-contracts-failure-evidence-and-recovery-proof.md) — consolidated implementation/proof story for output contracts, failure evidence, and narrow recovery
- [../evidence/session-lineage-reuse-and-operator-reset-proof.md](../evidence/session-lineage-reuse-and-operator-reset-proof.md) — consolidated implementation/proof story for session-lineage reuse, checkpointing, and operator reset
- [../evidence/context-strategy-and-experiment-framework-proof.md](../evidence/context-strategy-and-experiment-framework-proof.md) — consolidated implementation/proof story for context strategies, lazy evidence, telemetry, and recommendation output
- [../evidence/proposal-loop-feedback-fidelity-and-rereview-proof.md](../evidence/proposal-loop-feedback-fidelity-and-rereview-proof.md) — consolidated implementation/proof story for proposal-loop refine fidelity, backlog carry-forward, and targeted rereview
- [../evidence/run-surface-information-architecture-and-artifact-hierarchy-proof.md](../evidence/run-surface-information-architecture-and-artifact-hierarchy-proof.md) — consolidated implementation/proof story for segmented run surfaces, focused timeline, and hierarchical artifact browsing
- [../evidence/artifact-content-rendering-proof.md](../evidence/artifact-content-rendering-proof.md) — consolidated implementation/proof story for unified read-only markdown and JSON rendering
- [../evidence/design-system-and-brand-application-proof.md](../evidence/design-system-and-brand-application-proof.md) — consolidated implementation/proof story for the design-system and brand-application slice

## Risk Analysis

- [workspace-isolation-risk.md](workspace-isolation-risk.md) — Workspace-bound execution risk, failure modes, guardrails
