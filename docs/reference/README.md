# Reference

Implementation-oriented reference docs live here.

## Foundation layer (implemented)

- [domain-model.md](domain-model.md) — SwiftData persistence layer: 6 models (`Idea`, `Run`, `StageExecution`, `AgentExecution`, `Approval`, `Artifact`), status enums, `RunRepository`, provenance snapshots, drift detection, cost tracking
- [yaml-dsl-parser.md](yaml-dsl-parser.md) — YAML parsing (`YAMLParser`), validation (`YAMLValidator` with 10 check categories, `CompactWorkflowValidator`), provenance hashing (`DefinitionHasher`), parsed Codable structures, verification scaffold UI
- [architecture-decisions.md](architecture-decisions.md) — Key architecture decisions: CodingKeys strategy, single-active-run invariant, drift detection, snapshot storage, integer cost tracking, compact inspector-only scope, derived currentStageID, single-target enforcement

## Runtime contracts

- [runtime-contract.md](runtime-contract.md) — Frozen run snapshots, state machines, artifact model, storage boundaries, resume/retry rules
- [workspace-isolation-risk.md](workspace-isolation-risk.md) — Goose backend isolation risk, failure modes, required guardrails around workspace-bound execution
