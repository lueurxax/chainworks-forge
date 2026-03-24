# Reference

Implementation-oriented reference docs for Chainworks Forge.

## Foundation Layer

- [domain-model.md](domain-model.md) — SwiftData persistence: `Idea`, `Run`, `StageExecution`, `AgentExecution`, `Approval`, `Artifact`, Steward records, provenance snapshots, drift detection, cost tracking
- [yaml-dsl-parser.md](yaml-dsl-parser.md) — YAML parsing, validation (10 check categories), compact workflow normalization, provenance hashing, verification scaffold
- [architecture-decisions.md](architecture-decisions.md) — Key AD decisions: CodingKeys, single-active-run, drift detection, snapshot storage, integer cost, derived currentStageID

## Execution Engine

- [workflow-execution-engine.md](workflow-execution-engine.md) — RunPlan compiler, Workflow Orchestrator, Agent Executor protocol, Artifact Manager, Transition Evaluator, Resume Manager, Execution Service
- [runtime-contract.md](runtime-contract.md) — Frozen run snapshots, state machines, artifact model, storage boundaries, resume/retry rules

## Live Execution

- [live-provider-execution-slice.md](live-provider-execution-slice.md) — Live proposal-loop slice: runtime boundary, safety contract, approval flow, app surfaces, verification
- [goose-server-transport.md](goose-server-transport.md) — GooseServerTransport adapter: goosed API contract, SSE event mapping, session lifecycle, executor pipeline, proven real Goose connection

## System Health

- [forge-steward.md](forge-steward.md) — Forge Steward V1 (Observer): deterministic metrics, anomaly detection, cohorting, dossier building, trigger mechanisms

## Risk Analysis

- [workspace-isolation-risk.md](workspace-isolation-risk.md) — Workspace-bound execution risk, failure modes, guardrails
