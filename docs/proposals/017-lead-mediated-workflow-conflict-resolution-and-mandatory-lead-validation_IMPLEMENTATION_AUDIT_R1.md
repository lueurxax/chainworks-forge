# Implementation Audit R1: Proposal 017 - Lead-mediated workflow conflict resolution and mandatory lead validation

Date: 2026-04-23
Auditor: Codex
Mode: proposal-implementation-audit
Proposal: `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md`
Audit report: `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R1.md`
Git HEAD inspected: `eb6980ee9cd04b6b2024ad41df3f16f62f4c6b9c`
Validation commands run by this audit: none

## Verdict

Overall conformance: Partially Implemented.

Implementation readiness: Not Ready for proposal closeout.

P017 Phase A is substantially implemented: the current tree has graph-authoritative transition evaluation, blocking `WorkflowConflictRecord` persistence, non-blocking `WorkflowAdvisoryRejectionRecord` persistence, Swift/Rust DTO parity, implementation handoff status readback, and MCP/GraphQL report exposure for the current blocking conflict.

P017 Phase B and Phase C are not implemented: mediation-owned `AgentExecution` ownership, executable `LeadConflictMediationRecord` lifecycle, owner-aware retry/quota/artifact-claim ownership, mandatory system-lead catalog validation, and the rollout metrics/exit-gate evidence are missing or only documented as future/reference truth.

The canonical `proposal-017` gate exists, but the repository reference describes it as a Phase A-only gate and explicitly says it does not prove Phase B lead mediation or Phase C lead-validation coverage.

## Reviewer selection reuse

Prior reviewer-selection artifacts found: none.

Reviewer selection was not reused because no adjacent review artifacts were discovered by the audit helper.

Selected specialist lenses:

- `rust_arch_reviewer`: DB schema, repositories, workflow engine authority, and `AgentExecution` ownership model.
- `rust_reliability_reviewer`: conflict blocking semantics, cursor/resume, retry, cancellation, and mediation recovery.
- `api_contract_reviewer`: GraphQL, MCP, report payload, and enum casing contracts.
- `observability_rollout_reviewer`: canonical gates, rollout metrics, dogfood exits, and support/debuggability.
- `macos_ui_reviewer`: Swift bridge/report readback and operator conflict surfaces.

## Positive implementation evidence

- `scripts/test-gate.sh:1803` registers `proposal-017|p017`.
- `scripts/test-gate.sh:1808-1817` runs focused Swift tests plus Rust `workflow`, `domain`, `db`, `engine`, `mcp-server`, and `graphql-server` proposal-017 tests.
- `docs/reference/test-gates.md:254-268` documents the current `proposal-017` gate scope as Phase A workflow-authority/conflict-truth coverage.
- `control-plane/crates/db/migrations/025_p017_workflow_conflicts.sql:1-105` creates durable tables for `workflow_conflicts`, `workflow_advisory_rejections`, `implementation_handoff_statuses`, and `workflow_transition_cursors`.
- `control-plane/crates/domain/src/workflow_conflict.rs:1-460` defines the Rust domain contract for conflict reasons/statuses, candidate transition evaluation, advisory rejections, implementation handoff status, cursor records, and aggregate field authority.
- `Chainworks Forge/Models/WorkflowConflict.swift:1-360` defines the Swift bridge models for workflow conflicts, advisory rejections, candidate transition evaluation, and implementation handoff status.
- `Chainworks Forge/Engine/TransitionEvaluator.swift:92-178` evaluates candidate transitions and blocks no-match, multi-match, missing-input, invalid-expression, and evaluation-error outcomes instead of trusting advisory hints.
- `control-plane/crates/engine/src/orchestrator.rs:1380-1428` persists an `implementation_handoff_unavailable` workflow conflict when implementation-entry handoff evidence is unavailable.
- `control-plane/crates/engine/src/orchestrator.rs:1960-2033` persists blocking workflow conflicts and aligns run blocking/cursor state.
- `control-plane/crates/engine/src/orchestrator.rs:2044-2120` persists advisory rejection records when a legal graph transition overrides an invalid advisory `next_stage`.
- `control-plane/crates/mcp-server/src/tools/reports.rs:91-92` includes `workflow_conflict` and `implementation_handoff_status` in MCP report payloads.
- `control-plane/crates/mcp-server/src/tools/reports.rs:129-146` reads the current blocking workflow conflict and implementation handoff status for MCP report serialization.
- `control-plane/crates/mcp-server/src/tools/reports.rs:938` contains a focused proposal-017 MCP report test for current workflow conflict readback.
- `control-plane/crates/graphql-server/src/schema.rs:116` and `control-plane/crates/graphql-server/src/schema.rs:499` populate GraphQL implementation handoff status/current conflict fields.
- `control-plane/crates/graphql-server/src/schema.rs:1784` and `control-plane/crates/graphql-server/src/schema.rs:1845` contain proposal-017 GraphQL tests for current workflow conflict exposure.

## Findings

### ARCH-001: Mediation-owned `AgentExecution` ownership is not implemented

Severity: P1

Violated requirement: P017 Phase B owner model and provider-backed mediation prerequisites. The proposal requires `AgentExecution` to support `owner_kind`/`owner_id`, nullable `stage_execution_id` for non-stage owners, mediation owner tokens, owner-aware cancel/readback/cost paths, and retry-budget/source-claim ownership migration before Phase B provider-backed mediation starts. Primary acceptance points: `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:487-566`, `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:1207`, and `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:1235`.

Evidence: `control-plane/crates/db/migrations/001_initial.sql:44` still defines `agent_executions.stage_execution_id TEXT NOT NULL REFERENCES stage_executions(id)`. `control-plane/crates/domain/src/agent.rs:40-69` models `AgentExecution` with a required `stage_execution_id` and no `owner_kind`/`owner_id`. `control-plane/crates/db/src/repos/agent_executions.rs:9-16`, `control-plane/crates/db/src/repos/agent_executions.rs:35-50`, and `control-plane/crates/db/src/repos/agent_executions.rs:208-220` select, insert, and query executions through stage ownership only. `control-plane/crates/engine/src/command_handler.rs:1720` still reads retry quota through `agent_retry_budget_ledger::list_quota_for_stage_tx`. `control-plane/crates/db/migrations/016_p058_runtime_facts_and_artifact_claims.sql:51-78` defines source-generation claims with `stage_execution_id TEXT NOT NULL`, and `control-plane/crates/db/src/repos/artifact_contracts.rs:161-230` loads source-generation claims by the stage-scoped claim key shape.

Why blocking, not backlog: Phase B cannot safely launch a lead-owned provider execution while `AgentExecution` is structurally tied to `StageExecution`. Without the owner migration, mediation executions cannot be durably distinguished from stage executions, cannot be excluded from stage-only lists, cannot be included correctly in run-level cost/cancel/readback, and cannot own retry/quota/source-generation state. This blocks the proposal's Phase B provider-backed mediation contract rather than deferring a cosmetic enhancement.

Minimal fix: add an owner migration for `agent_executions` with `owner_kind`, `owner_id`, nullable `stage_execution_id` only for non-stage owners, `mediation_owner_token`, and `lead_mediation_record_id`; backfill existing rows as `stage_execution`; update domain structs/repositories; filter stage-only queries by owner; include all owners in run-level cancel/cost/readback; migrate retry budget and source-generation claim keys to owner-aware ownership or an equivalent contract; add DB/domain/engine fixtures proving both legacy stage-owned and new mediation-owned rows.

### ARCH-002: Lead mediation and mandatory system-lead validation are only modeled as enum/documentation, not executable behavior

Severity: P1

Violated requirement: P017 Phase B and Phase C. The proposal requires valid same-run-resolvable conflicts to escalate to exactly one system lead before clone fallback once Phase B is enabled, and every executable workflow/catalog pair to resolve exactly one valid system lead by Phase C. Primary acceptance points: `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:398-412`, `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:571-590`, `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:1179`, and `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:1195`.

Evidence: `control-plane/crates/db/migrations/025_p017_workflow_conflicts.sql:1-105` creates conflict, advisory rejection, handoff, and cursor tables, but no `lead_mediation` table. A targeted source search for `CREATE TABLE lead_mediation`, `mediation_owner`, and `lead_mediation_record` found no implementation in `control-plane/crates/db`, `control-plane/crates/domain`, `control-plane/crates/engine`, `control-plane/crates/graphql-server`, `control-plane/crates/mcp-server`, or `Chainworks Forge`; hits were limited to enum/status strings, docs, and fixture-inventory labels. A targeted source search for `system_role` found no executable catalog validation path in the same source surfaces.

Why blocking, not backlog: The proposal's core Phase B/Phase C behavior is not just reporting a blocked conflict. It requires the orchestrator to create and track a mediation attempt, select exactly one system lead, run mediation before clone fallback, and reject executable catalogs that cannot identify the lead. Without those behaviors, P017 remains a Phase A conflict-recording implementation and cannot satisfy the lead-mediated workflow or mandatory-lead validation contract.

Minimal fix: implement a durable `LeadConflictMediationRecord` table/repository/domain model and state machine; wire same-run-resolvable conflicts to create mediation records before clone fallback; resolve exactly one system lead from the workflow/catalog pair; persist lead ownership on conflicts and mediation executions; expose mediation state in GraphQL/MCP/Swift/report surfaces; add Phase B tests for escalation, cancel, retry, resume, and mediation resolution; add Phase C compiler/catalog validation that fails executable workflows with zero or multiple system leads.

### READY-001: The registered `proposal-017` gate proves Phase A only, not full P017 readiness

Severity: P1

Violated requirement: P017 requires a canonical gate that protects the proposal's workflow-conflict, lead-mediation, lead-validation, rollout, and handoff acceptance criteria. The proposal points at `./scripts/test-gate.sh proposal-017` as canonical evidence at `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:9` and includes Phase B/C acceptance criteria at `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:1179-1251`.

Evidence: `scripts/test-gate.sh:1803-1818` registers and runs the `proposal-017` gate, but the command set is focused on Swift bridge/report readback and control-plane conflict truth. `docs/reference/test-gates.md:254-268` explicitly describes the gate as "Phase A only" and states that it "does not prove Phase B lead mediation or Phase C lead-validation coverage." This audit did not run validation commands.

Why blocking, not backlog: A passing `proposal-017` gate can currently demonstrate Phase A but cannot close implementation readiness for the full proposal. Treating that gate as proposal-wide proof would allow Phase B/C requirements to ship untested and would contradict the repository's own test-gate reference.

Minimal fix: keep the current Phase A gate label honest, then either expand `proposal-017` after implementing Phase B/C or add subordinate gates such as `proposal-017-phase-b` and `proposal-017-phase-c` and make the canonical closeout gate invoke all required lanes. The final gate must assert mediation-owned executions, lead mediation lifecycle, mandatory lead validation, retry/source-claim ownership, per-surface mediation readback, and rollout metrics.

### OPS-001: P017 rollout metrics and exit-gate telemetry are absent from source

Severity: P2

Violated requirement: P017 requires recovery and conflict-resolution metrics, including `workflow_conflict_time_to_resolution_seconds`, `conflict_reason_to_action_outcome_total`, and `recovery_action_chosen_total`, with Phase B dogfood/rollout exit-gate use. Primary proposal points: `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:1040-1079` and `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md:1223`.

Evidence: A targeted source search for `workflow_conflict_time_to_resolution`, `conflict_reason_to_action`, and `recovery_action_chosen_total` across `control-plane`, `Chainworks Forge`, and `scripts` returned no source implementation hits.

Why blocking, not backlog: The metrics are part of P017's rollout and dogfood acceptance contract. Without durable emission/readback of these counters/timers, operators cannot prove the proposal's conflict-resolution path is improving recovery behavior or meet the stated exit gates for Phase B adoption.

Minimal fix: add telemetry emission at conflict creation, recovery action selection, mediation start/end, conflict resolution, and terminal-unverifiable settlement; expose/query these metrics through the existing observability surface used by gates; add tests that create conflicts, choose recovery actions, resolve or terminally settle them, and assert the named metrics increment with reason/action/outcome labels.

### DOC-001: Current reference docs describe Phase B ownership tables that the source tree does not implement

Severity: P2

Violated requirement: Repository policy treats `docs/reference/` as current implemented-system truth. P017 closeout cannot leave reference docs claiming implemented Phase B storage that is absent from the source tree.

Evidence: `docs/reference/rust-control-plane.md:371-374` documents `agent_executions` with `owner_kind`/`owner_id` and a `lead_mediation` table. `docs/reference/execution-truth-and-recovery.md:124-125` documents `owner_kind`/`owner_id`. The source evidence in ARCH-001 and ARCH-002 shows `agent_executions` remains stage-owned and no `lead_mediation` table exists.

Why blocking, not backlog: This creates a false current-system contract for downstream proposals and implementers. If P017 is not fully implemented yet, reference docs must not assert the missing Phase B schema as live behavior.

Minimal fix: either implement the Phase B storage/ownership model, or narrow the reference docs to explicitly mark `owner_kind`/`owner_id` and `lead_mediation` as planned/not-yet-implemented until the migration lands.

## Requirement conformance matrix

| Requirement | Status | Evidence |
|---|---:|---|
| Graph-authoritative transitions; advisory hints are non-authoritative | Implemented | Swift evaluator at `Chainworks Forge/Engine/TransitionEvaluator.swift:92-178`; Rust advisory rejection at `control-plane/crates/engine/src/orchestrator.rs:2044-2120`. |
| Persist blocking invalid/no-match/multi/missing/aggregate/unverifiable conflicts as `WorkflowConflictRecord` | Implemented for Phase A | DB schema at `control-plane/crates/db/migrations/025_p017_workflow_conflicts.sql:1-52`; orchestrator blocking write at `control-plane/crates/engine/src/orchestrator.rs:1960-2033`. |
| Persist non-blocking rejected advisory hints as `WorkflowAdvisoryRejectionRecord` | Implemented | DB schema at `control-plane/crates/db/migrations/025_p017_workflow_conflicts.sql:53-68`; orchestrator write at `control-plane/crates/engine/src/orchestrator.rs:2044-2120`. |
| Shared Swift/Rust conflict DTOs and enum casing | Implemented | Rust domain at `control-plane/crates/domain/src/workflow_conflict.rs:1-460`; Swift models at `Chainworks Forge/Models/WorkflowConflict.swift:1-360`; domain enum round-trip tests in `control-plane/crates/domain/tests/proposal_017_workflow_conflict.rs:1-120`. |
| MCP/GraphQL/report current conflict and implementation handoff readback | Implemented for Phase A | MCP report fields at `control-plane/crates/mcp-server/src/tools/reports.rs:91-146`; GraphQL field population/tests at `control-plane/crates/graphql-server/src/schema.rs:116`, `control-plane/crates/graphql-server/src/schema.rs:499`, `control-plane/crates/graphql-server/src/schema.rs:1784`, and `control-plane/crates/graphql-server/src/schema.rs:1845`. |
| Transition cursor/resume alignment with workflow conflict | Implemented for Phase A | Cursor table at `control-plane/crates/db/migrations/025_p017_workflow_conflicts.sql:88-105`; orchestrator cursor write at `control-plane/crates/engine/src/orchestrator.rs:1960-2033`. |
| Deterministic implementation-start handoff status and blocked conflict on unavailable handoff | Implemented | Handoff status table at `control-plane/crates/db/migrations/025_p017_workflow_conflicts.sql:75-83`; unavailable handoff conflict at `control-plane/crates/engine/src/orchestrator.rs:1380-1428`. |
| Phase B `AgentExecution` owner model for mediation-owned executions | Missing | See ARCH-001. |
| Phase B same-run lead mediation before clone fallback | Missing | See ARCH-002. |
| Phase B retry budget and artifact source-claim ownership for mediation-owned executions | Missing | See ARCH-001. |
| Phase C mandatory exactly-one system lead validation for executable workflow/catalog pairs | Missing | See ARCH-002. |
| Phase B/C canonical gate coverage | Missing | See READY-001. |
| Rollout metrics and dogfood exit-gate telemetry | Missing | See OPS-001. |
| Reference docs reflect current implemented truth | Partially Implemented / Drift | See DOC-001. |

## Validation inventory

Validation discovered but not run by this audit:

- `./scripts/test-gate.sh proposal-017`
- Swift test lane referenced by `scripts/test-gate.sh:1808`
- `cargo test -p workflow proposal_017_`
- `cargo test -p domain --test proposal_017_workflow_conflict`
- `cargo test -p db --test proposal_017_workflow_conflict_persistence`
- `cargo test -p engine proposal_017_`
- `cargo test -p mcp-server proposal_017_`
- `cargo test -p graphql-server proposal_017_`

This audit did not execute validation because the request did not explicitly ask to run tests.

## Recommended disposition

Do not close P017 as fully implemented.

Recommended next implementation path:

1. Treat the current state as Phase A implemented and protected by the existing Phase A gate.
2. Implement the Phase B ownership migration before any provider-backed lead mediation.
3. Implement durable lead mediation records and same-run escalation before clone fallback.
4. Implement Phase C exactly-one system-lead validation and external catalog enforcement.
5. Add rollout metrics and dogfood/readiness evidence.
6. Expand the canonical P017 gate to cover Phase B/C, or explicitly split the proposal into Phase A complete plus separate active Phase B/C proposals.
