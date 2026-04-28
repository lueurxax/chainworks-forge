# Proposal 017 Implementation Audit R3

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md` |
| Audit report | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R3.md` |
| Audit date | 2026-04-27 |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| Audited HEAD | `fd6dea4a94a4e58dd75df28179df74a4172461ce` |
| Compare base | `c750b72140f50925b68e5b6c10b4214648c70f6c` (`merge-base HEAD origin/main`) |
| Implementation target | Current worktree at `main`; P017 control-plane scope only |
| Worktree note | Audit started from a clean tree. After verification, this report was the only intentional write. |
| Canonical gate | `./scripts/test-gate.sh proposal-017` passed on audited HEAD |
| Proposal state | Active for implementation-readiness review |
| Prior reviewer reuse | Not reused; no proposal-review artifacts found. Prior implementation-audit reports were ignored for reviewer selection per skill rule. |
| Overall conformance | Not Implemented |
| Overall readiness | Not Ready |
| Audit confidence | High for source-level blockers, medium-high overall because no live provider-backed daemon run was executed |

## Implementation Target And Compare Base

The audited implementation is the current `main` worktree at `fd6dea4a94a4e58dd75df28179df74a4172461ce`, compared to `origin/main` merge base `c750b72140f50925b68e5b6c10b4214648c70f6c`.

The audited diff changes Swift daemon/read-boundary surfaces, Rust DB/domain/engine/GraphQL/MCP contracts, workflow evidence, and reference/proposal artifacts. P017's post-UI-DB-cutover amendment makes the Rust control plane the conformance target. Missing SwiftData storage, concrete Swift UI mediation screens, and legacy Swift report generation are not treated as blockers for this audit.

## Prior Review Reuse

The proposal-review discovery helper returned no reusable proposal-review artifacts. Existing `_IMPLEMENTATION_AUDIT_R*` reports were not used for reviewer selection.

Selected reviewers:

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | P017 changes Rust domain, DB migration, workflow compiler, engine, and owner model behavior. |
| `rust_reliability_reviewer` | P017 commits retry, cancellation, resume, queue ownership, idempotency, and stale-output handling. |
| `api_contract_reviewer` | GraphQL, MCP, workflow YAML, agent catalog, and report payloads are explicit contract surfaces. |
| `observability_rollout_reviewer` | P017 includes migrations, gate coverage, rollout metrics, dogfood, and external catalog evidence. |
| `chainworks_execution_truth_reviewer` | P017 changes durable Run/Stage/Agent/Approval/artifact/recovery truth. |

Rejected close alternatives:

| Reviewer | Reason |
|---|---|
| `macos_ui_reviewer` | P017 control-plane conformance excludes concrete UI implementation after the UI DB cutover. |
| `apple_arch_reviewer` | Swift client/provider state is adjacent, but not the acceptance surface. |
| `rust_security_reviewer` | Redaction/privacy is handled through API-contract review; auth/unsafe/security boundaries do not dominate this slice. |
| Product reviewer | Product decision metrics are covered by observability rollout; product review was not explicitly requested. |
| iOS reviewer | No iOS target is introduced. |

## Proposal State And Contract Summary

P017 is treated as active for implementation-readiness review. The proposal commits to:

- Phase A graph-authoritative transition truth, blocking workflow conflict persistence, advisory rejection history, cursor/resume behavior, report/API readback, and a canonical P017 gate.
- Phase B lead mediation with exactly-one lead resolution, durable `LeadConflictMediationRecord`, a normal mediation-owned `AgentExecution`, owner-aware retry/artifact/cost/runtime-fact behavior, confirmation settlement, cancellation, resume, dogfood, and sanitized GraphQL/MCP readback.
- Phase C exactly-one `system_role=lead` executable catalog validation, external catalog enforcement inventory, typed warnings or waiver evidence, and validation readback/metrics.
- No synthetic `StageExecution` for mediation-owned work.
- No UI-owned DB truth for the P017 acceptance target.

Platform/product scope:

| Dimension | Scope |
|---|---|
| Apple | macOS app present, but UI implementation is out of P017 conformance scope |
| Backend/service | Rust control-plane service, worker, API, data, rollout, workflow/catalog contracts |
| Product metrics | In scope only where P017 explicitly commits dogfood, operator feedback, rollout metrics, or decision evidence |

Primary service flows audited:

1. Graph-authoritative ambiguous/no-match transition persists a blocking workflow conflict and cursor.
2. Invalid advisory next-stage hint records a non-blocking advisory rejection.
3. Phase B conflict resolution finds a lead, creates mediation truth, and enqueues a mediation-owned agent invocation.
4. Lead output validation creates confirmation/settlement state and exposes sanitized operator readback.
5. Cancellation, retry, runtime facts, artifacts, cost, GraphQL, MCP, and rollout metrics preserve mediation-owned truth.

## Proposal Fidelity Inventory

Matches:

- The migration rebuilds `agent_executions` with nullable `stage_execution_id` and owner-kind checks for mediation-owned executions.
- `AgentExecution.stage_execution_id` is optional in the domain model.
- `agent_executions::list_by_run` includes stage-owned and mediation-owned executions through stage and mediation joins.
- The Phase B resolver map is populated for attested bundled workflow/catalog pairs.
- `examples/agents/agents.yaml` declares `system_role: lead` and lead resolution contract metadata.
- Workflow compile validation enforces exactly one lead.
- Owner-aware retry budget and artifact source-generation claim repositories exist.
- GraphQL and MCP expose sanitized lead mediation status and have tests that reject `operator_rationale` leakage.
- Phase B dogfood, Phase C external catalog inventory, and Phase A known-issues evidence artifacts exist.
- The canonical P017 gate exists and passed on the audited HEAD.

Divergences:

- `cancel_running_by_run` cancels matching mediation-owned `AgentExecution` rows but does not transition linked `LeadConflictMediationRecord` rows to `canceled` in the same transaction.
- GraphQL/MCP mediation readback does not expose the promised `execution_attempts` / mediation execution attempt shape with owner, nullable stage, runtime facts, transcript refs, watchdog result, artifacts, and cost.
- Mediation status readback synthesizes one `status_updates` item with hard-coded `attempt_number = 1`; no durable status or retry-attempt history is surfaced.
- `agent_executions` still lacks the proposal-named direct `run_id` and `mediation_owner_token` fields and the related indexes; the implementation uses joins plus `owner_execution_lineage_id` instead.
- Several committed P017 metric names are schema/docs/helper-only; runtime emission was not found for Phase C validation outcomes, lead mediation attempts, or external catalog warnings.

Ambiguities / evidence gaps:

- No live provider-backed daemon run was executed.
- Dogfood evidence is an operator-approved fixture record, not replayed run logs from this audit.
- The P017 gate passes while still not covering the missing execution-attempt readback and mediation-record cancellation invariants.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Graph-authoritative transition selection and advisory non-authority | Implemented |
| REQ-002 | Blocking `WorkflowConflictRecord`, fingerprint, run blocking, and cursor truth | Implemented |
| REQ-003 | Non-blocking advisory rejection history and report/API readback | Implemented |
| REQ-004 | Implementation-entry handoff latest-summary/MCP/GraphQL readback | Implemented |
| REQ-005 | Canonical `proposal-017` gate | Implemented |
| REQ-006 | Phase B resolver, mediation record, lead invocation, and confirmation settlement | Implemented |
| REQ-007 | Mediation-owned `AgentExecution` owner model with null `stage_execution_id` | Partially Implemented |
| REQ-008 | Owner-adjacent retry budget and artifact source-generation claims | Implemented |
| REQ-009 | Phase C exactly-one `system_role=lead` executable validation | Implemented |
| REQ-010 | Sanitized GraphQL/MCP mediation readback | Partially Implemented |
| REQ-011 | Mediation execution-attempt readback under workflow conflict | Missing |
| REQ-012 | Cancellation/resume invariants for mediation-owned executions | Partially Implemented |
| REQ-013 | Rollout metrics, dogfood, known-issues, and external catalog evidence | Partially Implemented |

## Detailed REQ Audit

### REQ-001: Graph-Authoritative Transition Selection

Proposal source: Phase A transition authority and workflow conflict acceptance criteria.

Status: Implemented.

Evidence: `./scripts/test-gate.sh proposal-017` passed. The gate ran Swift P017 tests plus Rust workflow/domain/DB/engine filters that cover no-match conflicts, ambiguous transition blocking, terminal-unverifiable conflict history, advisory rejection, and legal transition resolution.

Implementation mapping: Engine orchestrator P017 tests and workflow lint/tests validate graph-authoritative selection.

Gap / note: No gap found for this requirement.

### REQ-002: Blocking Conflict Persistence And Cursor Truth

Proposal source: Phase A blocking `WorkflowConflictRecord`, stable fingerprint, run blocking, and cursor truth commitments.

Status: Implemented.

Evidence: Gate-covered DB/domain/engine tests persist current blocking conflicts, supersede resolved conflicts, store terminal-unverifiable state, and resume selected transition cursors.

Implementation mapping: `control-plane/crates/engine/src/orchestrator.rs`, `control-plane/crates/db/src/repos/workflow_conflicts.rs`, and P017 DB/engine tests.

Gap / note: No gap found for this requirement.

### REQ-003: Advisory Rejection History

Proposal source: Phase A non-blocking advisory rejection and report/API visibility commitments.

Status: Implemented.

Evidence: Gate-covered engine and DB tests record non-blocking advisory rejections. MCP and GraphQL P017 tests pass for current workflow conflict readback.

Implementation mapping: Engine advisory rejection handling and report/API projections.

Gap / note: No gap found for this requirement.

### REQ-004: Implementation Handoff Readback

Proposal source: Implementation-entry handoff status and code-writer start blocking commitments.

Status: Implemented.

Evidence: The gate ran tests for start-implementation blocking before invoke and proposal-current snapshot behavior. MCP still exposes `implementation_handoff_status_json`.

Implementation mapping: `control-plane/crates/engine/tests/integration.rs` and `control-plane/crates/mcp-server/src/tools/reports.rs`.

Gap / note: No gap found for this requirement.

### REQ-005: Canonical Gate

Proposal source: P017 gate and acceptance evidence requirements.

Status: Implemented.

Evidence: `./scripts/test-gate.sh proposal-017` passed on `fd6dea4a94a4e58dd75df28179df74a4172461ce`. The gate ran Swift `Proposal017Tests`, workflow lead validation tests, evidence gates, domain/DB conflict tests, MCP reports tests, GraphQL readback tests, engine orchestrator tests, and targeted engine integration tests.

Implementation mapping: `scripts/test-gate.sh`.

Gap / note: The gate is present and passing, but READY-001 records that it does not yet catch two critical Phase B gaps.

### REQ-006: Phase B Lead Mediation Lifecycle

Proposal source: Phase B lead resolver, mediation record, lead invocation, confirmation, and settlement commitments.

Status: Implemented.

Evidence: The resolver map is populated, mediation lifecycle tests pass, and current source creates mediation-owned `InvokeAgent` work with nullable stage ownership. Confirmation resolution is present in the command handler path.

Implementation mapping: `control-plane/crates/engine/src/orchestrator.rs`, `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/engine/src/command_handler.rs`, resolver evidence, and `p017_mediation_record_lifecycle`.

Gap / note: This requirement covers the lifecycle creation/settlement flow. Owner readback and cancellation are separately tracked in REQ-011 and REQ-012.

### REQ-007: Mediation-Owned AgentExecution Owner Model

Proposal source: Phase B migration contract around `owner_kind=lead_conflict_mediation`, nullable `stage_execution_id`, `owner_id`, `run_id`, `mediation_owner_token`, and lead mediation linkage.

Status: Partially Implemented.

Evidence type: code, migration, tests-run.

Evidence references:

- `control-plane/crates/db/migrations/029_p017_nullable_mediation_stage_execution.sql:8` creates a rebuilt `agent_executions` table with nullable `stage_execution_id`.
- `control-plane/crates/db/migrations/029_p017_nullable_mediation_stage_execution.sql:38` adds `owner_kind`.
- `control-plane/crates/db/migrations/029_p017_nullable_mediation_stage_execution.sql:43` adds CHECK constraints distinguishing stage-owned and lead-mediation-owned executions.
- `control-plane/crates/db/migrations/029_p017_nullable_mediation_stage_execution.sql:81` creates owner/stage indexes, but not the proposal-named `run_id` or `mediation_owner_token` indexes.
- `control-plane/crates/domain/src/agent.rs:41` makes `stage_execution_id` optional and carries owner fields.
- `control-plane/crates/db/src/repos/agent_executions.rs:340` includes mediation-owned rows in `list_by_run` by joining `lead_conflict_mediations`.

Implementation mapping: DB migration, domain agent model, executor invocation ownership, and DB integration tests.

Gap / note: The core null-stage owner model exists, but the schema does not implement the explicit direct `run_id` and `mediation_owner_token` fields named by P017. The implementation may be close through joins and `owner_execution_lineage_id`, but the proposal contract is not literally satisfied and equivalent behavior is not fully proven because cancellation/readback remain incomplete.

### REQ-008: Owner-Adjacent Retry And Artifact Claims

Proposal source: Phase B owner-adjacent retry budget and artifact source-generation claim commitments.

Status: Implemented.

Evidence: Migration `029_p017_nullable_mediation_stage_execution.sql` rebuilds retry budget ledger and artifact source-generation claims with owner keys. Repository APIs use `OwnerKind` and owner IDs. The P017 gate includes DB coverage for owner-adjacent persistence.

Implementation mapping: `control-plane/crates/db/src/repos/agent_retry_budget_ledger.rs`, `control-plane/crates/db/src/repos/artifact_contracts.rs`, and DB tests.

Gap / note: No gap found for the owner-keyed repository slice.

### REQ-009: Phase C Exactly-One Lead Validation

Proposal source: Phase C mandatory executable catalog validation requiring exactly one `system_role=lead` with resolution contract.

Status: Implemented.

Evidence: `AgentEntry` includes `system_role`; workflow validation rejects missing or duplicate leads and rejects leads without a resolution contract. The compiler invokes the validation, and the P017 gate ran the matching workflow tests.

Implementation mapping: `control-plane/crates/workflow/src/catalog.rs`, `control-plane/crates/workflow/src/compiler.rs`, and workflow integration tests.

Gap / note: No gap found for static executable catalog validation.

### REQ-010: Sanitized GraphQL/MCP Mediation Readback

Proposal source: Phase B and AC-027 sanitized live GraphQL/MCP mediation status updates with timestamp, attempt number, and redaction.

Status: Partially Implemented.

Evidence type: code, tests-run.

Evidence references:

- `control-plane/crates/mcp-server/src/tools/reports.rs:132` enriches current workflow conflicts with `lead_mediation`.
- `control-plane/crates/mcp-server/src/tools/reports.rs:155` returns sanitized mediation fields.
- `control-plane/crates/graphql-server/src/types/run.rs:198` exposes `GqlLeadMediation`.
- `control-plane/crates/graphql-server/src/types/run.rs:217` exposes `GqlLeadMediationStatusUpdate`.
- MCP and GraphQL tests passed and assert that `operator_rationale` is not leaked.

Implementation mapping: MCP reports payload and GraphQL run conflict projection.

Gap / note: The current readback has sanitized status fields, but it synthesizes a single status update with hard-coded `attempt_number = 1` at `mcp-server/src/tools/reports.rs:181` and `graphql-server/src/types/run.rs:237`. It does not expose durable attempt/status history or the richer mediation readback semantics that P017 ties to execution attempts.

### REQ-011: Mediation Execution-Attempt Readback

Proposal source: P017 MCP `reports.get` contract for `workflow_conflict.current.lead_mediation.execution_attempts`, owner-aware GraphQL readback, AC-009, AC-015, and AC-027.

Status: Missing.

Evidence type: code, search, tests-run.

Evidence references:

- `rg` found no implementation field/type named `execution_attempts` or `GqlLeadMediationExecution` in GraphQL/MCP code.
- `control-plane/crates/mcp-server/src/tools/reports.rs:171` returns mediation status JSON but no execution attempts.
- `control-plane/crates/graphql-server/src/types/run.rs:200` defines `GqlLeadMediation` without execution attempts.
- Current MCP and GraphQL P017 tests pass without querying execution attempts.

Implementation mapping: No equivalent current mapping found under the conflict mediation readback surfaces.

Gap / note: Operators and agents cannot inspect the mediation-owned `AgentExecution` through the workflow conflict surface that P017 designates as authoritative. Runtime facts, transcript refs, watchdog outcome, artifacts, provider/model, timing, and cost are not grouped with current conflict mediation readback.

### REQ-012: Cancellation And Resume Invariants

Proposal source: Phase B cancellation/resume contract, including the requirement that `cancel_running_by_run` cancel active agent executions and transition linked `LeadConflictMediationRecord` rows to `canceled` in the same repository transaction.

Status: Partially Implemented.

Evidence type: code, tests-run.

Evidence references:

- `control-plane/crates/engine/src/cancellation.rs:96` calls `agent_executions::cancel_running_by_run_tx` and `work_items::cancel_running_by_run_tx`.
- `control-plane/crates/db/src/repos/agent_executions.rs:426` updates matching `agent_executions` to canceled, including mediation-owned rows by lead mediation join.
- No same-transaction caller was found that updates `lead_conflict_mediations.status` to `canceled`.
- Startup repair and stale-output tests passed under the gate.

Implementation mapping: Run cancellation service and agent execution repository.

Gap / note: Agent executions are canceled, but linked mediation records can remain non-terminal. This splits durable mediation truth from agent execution truth during run cancellation.

### REQ-013: Rollout Metrics And Evidence

Proposal source: P017 metrics, dogfood, known-issues, external catalog inventory, and rollout evidence requirements.

Status: Partially Implemented.

Evidence type: migration, telemetry, tests-run, evidence artifacts.

Evidence references:

- `control-plane/crates/db/migrations/030_p017_workflow_conflict_metric_events.sql` registers P017 metric names including `phase_c_validation_outcome_total`, `external_catalog_warning_total`, and `lead_mediation_attempt_total`.
- `control-plane/crates/db/src/repos/workflow_conflicts.rs:346` defines `record_phase_c_validation_outcome_tx`.
- `rg` found no production caller for `record_phase_c_validation_outcome_tx`.
- `rg` found `lead_mediation_attempt_total` and `external_catalog_warning_total` in migration/docs/evidence strings, but no runtime emission path.
- The P017 gate passed evidence tests for dogfood, external catalog inventory, and known-issues artifacts.

Implementation mapping: Metric schema, workflow conflict metrics helpers, evidence artifacts, and evidence gate tests.

Gap / note: Evidence artifacts exist and some metrics are emitted, but several proposal-named metrics are not wired to representative runtime paths.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Not Implemented | Missing execution-attempt readback | High |
| Rust architecture | Partial | Owner model omits proposal-named fields and remains join-dependent | High |
| Rust reliability | Not Ready | Run cancellation can leave mediation records non-terminal | High |
| API contract | Not Ready | GraphQL/MCP lack promised mediation execution-attempt shape | High |
| Observability/rollout | Partial | Several committed metrics lack runtime emission | Medium |
| Execution truth | Not Ready | Agent execution truth and mediation truth can diverge on cancellation/readback | High |
| Release readiness | Not Ready | The canonical gate passes but does not fail on the remaining blockers | High |

## Routed Specialist Findings

### REL-001 / `rust_reliability_reviewer`

Severity: Critical. Confidence: High.

Related proposal items: REQ-012, Phase B cancellation contract, AC-015.

Evidence: `control-plane/crates/engine/src/cancellation.rs:96` cancels agent executions and work items. `control-plane/crates/db/src/repos/agent_executions.rs:426` updates only `agent_executions`. No same-transaction update of `lead_conflict_mediations` to `canceled` was found.

Why it matters: Canceling a run can leave a linked lead mediation in `queued`, `running`, or `operator_confirmation_required` while the associated agent execution is canceled. That breaks the proposal's durable mediation truth and makes resume, stale-output handling, and operator readback ambiguous.

Recommended action: Add a repository/service path that transitions active lead mediations for the canceled run to `canceled` in the same transaction as agent execution cancellation.

Acceptance criteria: Canceling a run with an active mediation-owned execution updates both `agent_executions.status = canceled` and `lead_conflict_mediations.status = canceled` atomically; late provider output remains ignored and does not mutate resolved/canceled mediation state.

### API-001 / `api_contract_reviewer`

Severity: Major. Confidence: High.

Related proposal items: REQ-011, AC-009, AC-015, AC-027.

Evidence: MCP `lead_mediation_readback_json` returns status fields only at `control-plane/crates/mcp-server/src/tools/reports.rs:171`. GraphQL `GqlLeadMediation` has no execution-attempt field at `control-plane/crates/graphql-server/src/types/run.rs:200`. Searches found no `execution_attempts` or equivalent mediation execution projection.

Why it matters: The proposal's authoritative conflict readback is missing the normal mediation-owned `AgentExecution` attempt details that operators and agents need to inspect provider/model, timing, runtime facts, transcript refs, watchdog result, artifacts, and cost.

Recommended action: Add MCP and GraphQL mediation execution-attempt readback backed by owner-aware `agent_executions::list_by_run` plus runtime fact, artifact, transcript, watchdog, and cost joins.

Acceptance criteria: A mediation-owned `AgentExecution` with null `stage_execution_id` appears under `workflow_conflict.current.lead_mediation.execution_attempts` in MCP and the equivalent GraphQL field, with owner identity and the proposal-required runtime detail. Stage-scoped execution fields still exclude mediation-owned attempts.

### ARCH-001 / `rust_arch_reviewer`

Severity: Major. Confidence: Medium.

Related proposal items: REQ-007, Phase B migration contract.

Evidence: Migration `029_p017_nullable_mediation_stage_execution.sql` adds owner fields and nullable stage ownership, but `agent_executions` has no direct `run_id` or `mediation_owner_token` columns. Run listing uses joins at `control-plane/crates/db/src/repos/agent_executions.rs:340`; cancellation also relies on joins at `control-plane/crates/db/src/repos/agent_executions.rs:426`.

Why it matters: P017 named these fields to make owner-aware run cancellation, readback, and idempotent resume direct and unambiguous. The current substitute may be acceptable only if deliberately approved and proven equivalent, which it is not while cancellation/readback gaps remain.

Recommended action: Either implement the named fields and indexes or record an explicit proposal amendment/equivalence decision with tests proving cancellation, readback, and idempotency behavior.

Acceptance criteria: Owner-aware run/cancellation/readback paths are proven without ambiguity, and the schema contract is either implemented literally or amended with approved equivalence.

### OPS-001 / `observability_rollout_reviewer`

Severity: Major. Confidence: Medium.

Related proposal items: REQ-013, P017 metrics section.

Evidence: Runtime emission exists for some workflow conflict metrics, but no production caller was found for `record_phase_c_validation_outcome_tx`. `lead_mediation_attempt_total` and `external_catalog_warning_total` appear in migration/docs/evidence strings without a runtime emission path.

Why it matters: Rollout evidence can pass fixture gates while Phase C validation outcomes, mediation attempt outcomes, and external catalog warnings remain invisible in real runs.

Recommended action: Wire representative runtime emission for Phase C validation outcomes, lead mediation attempts by result, and external catalog warning decisions, then add gate assertions.

Acceptance criteria: The P017 gate proves representative runtime paths insert all committed P017 metric names with bounded labels.

### TRUTH-001 / `chainworks_execution_truth_reviewer`

Severity: Critical. Confidence: High.

Related proposal items: REQ-011, REQ-012.

Evidence: Mediation-owned executions are durable and run-visible, but conflict-scoped execution-attempt readback is absent and mediation record cancellation is not synchronized with agent execution cancellation.

Why it matters: P017's central model is that workflow conflict truth, mediation truth, and agent execution truth stay synchronized. The current implementation can split those truths across readback and cancellation surfaces.

Recommended action: Treat cancellation synchronization and conflict-scoped execution-attempt readback as one closeout slice because both depend on the same owner-aware execution truth.

Acceptance criteria: For a mediation-owned attempt, current conflict readback, run-level execution truth, cancellation, late-output handling, and settlement all report one consistent terminal story.

### READY-001 / Readiness

Severity: Major. Confidence: High.

Related proposal items: REQ-005, REQ-011, REQ-012.

Evidence: `./scripts/test-gate.sh proposal-017` passed, but the audited code still lacks mediation execution-attempt readback and mediation-record cancellation. The passing MCP/GraphQL tests query sanitized status, not execution attempts.

Why it matters: The gate currently overstates readiness for the remaining Phase B owner-aware behavior.

Recommended action: Expand the P017 gate to fail before these blockers are fixed.

Acceptance criteria: The gate covers cancellation-to-mediation status, MCP/GraphQL execution-attempt readback, attempt number/history behavior beyond hard-coded `1`, and representative metric emission.

## Readiness Checklist

| Area | Result | Notes |
|---|---|---|
| Canonical gate | Pass | `./scripts/test-gate.sh proposal-017` passed on audited HEAD. |
| Swift P017 tests | Pass | Gate ran `Chainworks ForgeTests/Proposal017Tests` with 16 passing tests; UI tests remain remote-only/out of scope. |
| Rust targeted tests | Pass | Workflow, evidence, domain, DB, MCP, GraphQL, and engine P017 filters passed. |
| Core Phase A conflict flow | Pass | Conflict/advisory/cursor behavior covered by tests. |
| Phase B resolver and mediation creation | Pass | Resolver populated; mediation lifecycle path and test exist. |
| Mediation-owned execution identity | Partial | Null-stage owner model exists; direct `run_id`/`mediation_owner_token` contract not literal. |
| Owner-adjacent retry/artifact claims | Pass | Owner-keyed repositories and tests exist. |
| Phase C system lead validation | Pass | Catalog schema/compiler/tests enforce exactly one lead. |
| API parity/readback | Fail | Execution attempts missing from GraphQL/MCP mediation readback. |
| Reliability/cancellation | Fail | Mediation records are not canceled with run cancellation. |
| Redaction/privacy | Pass with residual risk | GraphQL/MCP tests exclude `operator_rationale`; no live transcript export was validated. |
| Rollout/metrics | Partial | Evidence artifacts and some metrics exist; several runtime emission paths are missing. |
| Full regression | Not run | Negative readiness is established despite the passing canonical P017 gate. |

UI/UX checklist items: Empty/loading/offline/permission states, accessibility, localization, and macOS UI affordances are out of P017 conformance scope after the UI DB cutover amendment.

Privacy/permissions/entitlements: No new entitlements or auth boundary were audited. Redaction was validated at source/test level for GraphQL/MCP mediation status only.

## Verification Log

Commands run:

- `git rev-parse HEAD`
- `git merge-base HEAD origin/main`
- `git status -sb`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py /Users/user/Documents/Chainworks Forge/docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md`
- `./scripts/test-gate.sh proposal-017`
- Focused source searches and reads across `control-plane/crates/domain`, `control-plane/crates/db`, `control-plane/crates/engine`, `control-plane/crates/workflow`, `control-plane/crates/graphql-server`, `control-plane/crates/mcp-server`, `examples/agents`, `examples/workflows`, `docs/reference`, `docs/proposals/017-evidence`, and `scripts/test-gate.sh`.

Canonical gate result:

- Swift `Chainworks ForgeTests/Proposal017Tests`: 16 tests passed.
- Rust workflow/domain/DB/MCP/GraphQL/engine/evidence P017 filters: passed.
- Warnings observed: existing Rust dead-code warnings in `acp` and `engine`; they did not fail the gate.
- Final gate line: `==> Proposal 017 gate passed`.

## Final Verdict

Overall conformance: Not Implemented.

Overall implementation readiness: Not Ready.

Reviewer-selection reuse: Not reused.

The implementation has meaningful Phase A, Phase B, and Phase C progress and the canonical gate passes, but it still fails the proposal contract on conflict-scoped mediation execution-attempt readback and remains unreliable on run cancellation because mediation records are not terminally synchronized with canceled mediation-owned executions.

Recommended next actions:

1. Add same-transaction cancellation of linked `LeadConflictMediationRecord` rows when canceling a run.
2. Add MCP and GraphQL `lead_mediation.execution_attempts` readback backed by owner-aware agent execution truth.
3. Replace synthesized hard-coded attempt readback with durable attempt/status history or a proven equivalent.
4. Decide whether to implement the proposal-named `run_id` and `mediation_owner_token` fields or amend the proposal with an approved equivalence proof.
5. Wire missing runtime metrics and expand `proposal-017` gate coverage so these blockers fail before closeout.
