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
- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) — Canonical terminal outcomes, atomic transition settlement, cursor-driven resume, stage-owned recovery evidence, approval restore, runtime binding truth, host interruption, and startup recovery progress readback
- [skill-resolution-and-runtime-integration.md](skill-resolution-and-runtime-integration.md) — Skill resolution, role specialization, runtime injection, frozen skill truth, and operator readback
- [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md) — Per-agent MCP profiles, requested/predicted/actual/denied truth, runtime validation, and MCP telemetry
- [failed-stage-evidence-delivery-preflight-and-mcp-resolution.md](failed-stage-evidence-delivery-preflight-and-mcp-resolution.md) — Rust failed-stage evidence packets, delivery preflight, execution-time MCP resolution, ACP `mcpServers`, and northbound readback
- [acp-runtime-transport.md](acp-runtime-transport.md) — ACP transport contract, runtime selection, adapter families (Claude/Gemini/Codex/Auggie/Junie), persisted runtime truth, and capacity management
- [structured-output-envelope-and-contract-validation.md](structured-output-envelope-and-contract-validation.md) — Named ACP output envelopes, canonical contract binding, validation modes, normalized artifact identity, and durable validation-failure substrate
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md) — Catalog-backed output contracts, canonical artifact-contract transition truth, implementation self-assessment and handoff, generated run-state projection, failed-stage evidence, same-run retry truth, declarative Tier 1 enforcement, and bounded proposal compaction
- [session-lineage-reuse-and-operator-reset.md](session-lineage-reuse-and-operator-reset.md) — Reusable session lineage, invocation owner keys, binding fingerprints, reuse policy taxonomy, live ACP session ownership, context budget evaluation, checkpoint rehydration, and shell-owned per-agent reset
- [context-strategy-and-experiment-framework.md](context-strategy-and-experiment-framework.md) — Frozen strategy profiles, handoff compilation, lazy evidence, normalized strategy telemetry, and shell-owned recommendation output
- [proposal-loop-feedback-fidelity-and-rereview.md](proposal-loop-feedback-fidelity-and-rereview.md) — Review-corpus bundle ownership, score-lift backlog, writer coverage, targeted rereview, and proposal-growth discipline for the live proposal loop

## Control Plane

- [rust-control-plane.md](rust-control-plane.md) — Rust + SQLite local control-plane daemon: architecture, crate layout, workflow engine, ACP transport, persistence model, boundary shape, configuration, capacity-aware scheduling, and write serialization
- [local-daemon-lifecycle-supervision-and-packaging.md](local-daemon-lifecycle-supervision-and-packaging.md) — Local daemon lifecycle, supervision, health/readiness, packaged-mode paths, SQLite startup safety, failed-serve behavior, diagnostics, and packaging proof lanes
- [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md) — Bearer auth, caller-scoped capability filtering, per-command audit journaling, and `journal_id` surfacing on MCP + GraphQL northbound surfaces
- [per-run-workspace-isolation.md](per-run-workspace-isolation.md) — Per-run meta-root derivation, path resolution, ACP env handoff, worktree exemption, transition/normalization isolation, and legacy fallback semantics
- [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md) — GraphQL projection read contract for the thin macOS client: implemented surfaces, projection freshness, freshness budgets, subscriptions, backpressure, and P031 consumption rules

## Live Execution

- [live-provider-execution-slice.md](live-provider-execution-slice.md) — Live proposal-loop slice: runtime boundary, safety contract, approval flow, app surfaces, verification
- [operator-experience.md](operator-experience.md) — Operator shell baseline: Runs Home, reports, recovery, comparison, artifact inspection, notifications, backpressure visibility, and host interruption labels
- [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md) — Segmented run shells, pane routing, focused timeline, canonical artifact hierarchy, and metadata-demotion continuity
- [artifact-content-rendering.md](artifact-content-rendering.md) — Unified rendering contract for read-only markdown and JSON artifacts
- [provider-platform.md](provider-platform.md) — Provider/settings baseline: registry, adapters, settings, preflight, receipts, capacity caps, first-run and pilot surfaces
- [design-system-and-brand-application.md](design-system-and-brand-application.md) — Forge token lane, bounded brand assets, and shell/run/setup/recovery visual adoption
- [ui-quality-and-polish.md](ui-quality-and-polish.md) — UI readability, bounded accessibility, shared status semantics, and owner-surface proof contract
- [run-control.md](run-control.md) — Stop vs archive boundary, two-phase cancellation settlement, operator-visible `cancelling`/`cancelled` truth, northbound reader split
- [release-gate.md](release-gate.md) — Manual release gate: post-approval task execution, N-phase ordering, native deterministic git/publish, canonical release artifacts, and `delivery_receipt` settlement
- [project-workspace-contract.md](project-workspace-contract.md) — `requires_project_access`, idea-owned workspace root, frozen run workspace contract
- [provider-binding-truth.md](provider-binding-truth.md) — Frozen provider/model truth, provenance, and cross-family mismatch handling
- [idea-lifecycle.md](idea-lifecycle.md) — Active vs archived idea contract, archive/restore eligibility, cross-surface truth
- [live-workflow-map.md](live-workflow-map.md) — Run-detail topology, state vocabulary, handoff counters, loop/fallback visibility
- [full-mvp-delivery.md](full-mvp-delivery.md) — Repo-backed `Full MVP Live` slice: frozen delivery config, dedicated worktree, implementation loop, manual release, implementation self-assessment, and handoff routing
- [mvp-sign-off.md](mvp-sign-off.md) — Benchmark, replayable `GO/HOLD`, export hub, approval relaunch, and current-head sign-off contract
- [current-system-baseline.md](current-system-baseline.md) — Current-head subsystem map and reusable baseline for review and planning work

## Test Strategy

- [test-suite-architecture.md](test-suite-architecture.md) — Swift Testing unit-suite structure, conventions, mock lanes, tags, plans, residual gaps
- [test-gates.md](test-gates.md) — Layered local and CI execution gates, gate ownership, crash-aware runner behavior
- [agent-ui-test-execution.md](agent-ui-test-execution.md) — How agents should run preview review, focused XCUITest, and app-launched UI proof flows

## System Health

- [forge-steward.md](forge-steward.md) — Forge Steward V1 (Observer): deterministic metrics, anomaly detection, cohorting, dossier building, trigger mechanisms
- [steward-analysis-system.md](steward-analysis-system.md) — Rust Steward implementation: frozen cohort owners, deterministic analysis, active-catalog IO, triggers, persistence, and northbound readback

## Risk Analysis

- [workspace-isolation-risk.md](workspace-isolation-risk.md) — Workspace-bound execution risk, failure modes, guardrails
