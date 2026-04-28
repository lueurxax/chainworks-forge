# Proposal 017 Implementation Audit R4

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md` |
| Audit report | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R4.md` |
| Audit date | 2026-04-27 |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` (`claude/bold-lichterman` also points at HEAD) |
| Audited HEAD | `1f29206b28798be42fb51b4abb74793434b8cef3` |
| Compare base | `c750b72140f50925b68e5b6c10b4214648c70f6c` (`merge-base HEAD origin/main`) |
| Implementation target | Current worktree at `main`; P017 control-plane scope only |
| Worktree note | Audit started with one pre-existing untracked file: `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R3.md`. This audit did not modify it. |
| Canonical gate | `./scripts/test-gate.sh proposal-017` passed on audited HEAD |
| Proposal state | Active for implementation-readiness review |
| Prior reviewer reuse | Not reused; no proposal-review artifacts found. Prior implementation-audit reports were ignored for reviewer selection. |
| Overall conformance | Partial |
| Overall readiness | Not Ready |
| Audit confidence | High for source-level blockers, medium-high overall because no live provider-backed daemon run was executed |

## Implementation Target And Compare Base

The audited implementation is the current `main` worktree at `1f29206b28798be42fb51b4abb74793434b8cef3`, four commits ahead of `origin/main` merge base `c750b72140f50925b68e5b6c10b4214648c70f6c`.

The diff now includes explicit P017 follow-up work for the prior audit blockers: mediation cancellation cascade, GraphQL/MCP mediation execution-attempt readback, an owner-field equivalence record and proof test, additional metric helpers/callers/tests, and stronger `proposal-017` gate checks.

P017's post-UI-DB-cutover amendment makes the Rust control plane the conformance target. Missing SwiftData storage, concrete Swift UI mediation screens, and legacy Swift report generation are not treated as blockers for this audit.

## Prior Review Reuse

The proposal-review discovery helper returned no reusable proposal-review artifacts. Existing `_IMPLEMENTATION_AUDIT_R*` reports were not used for reviewer selection, per the skill rule.

Selected reviewers:

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | P017 changes Rust domain, DB migrations, workflow compiler, engine, and owner model behavior. |
| `rust_reliability_reviewer` | P017 commits retry, cancellation, resume, idempotency, and stale-output handling. |
| `api_contract_reviewer` | GraphQL, MCP, workflow YAML, agent catalog, and report payloads are explicit contract surfaces. |
| `observability_rollout_reviewer` | P017 includes migrations, gate coverage, rollout metrics, dogfood, and external catalog evidence. |
| `chainworks_execution_truth_reviewer` | P017 changes durable Run/Stage/Agent/Approval/artifact/recovery truth. |

Rejected close alternatives:

| Reviewer | Reason |
|---|---|
| `macos_ui_reviewer` | P017 control-plane conformance excludes concrete UI implementation after the UI DB cutover. |
| `apple_arch_reviewer` | Swift client/provider state is adjacent, but not the acceptance surface. |
| `rust_security_reviewer` | Redaction/privacy is covered through API-contract review; auth/unsafe/security boundaries do not dominate this slice. |
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

- The canonical P017 gate passed on the audited HEAD and now checks for the R2/R3 closure tests.
- Run cancellation now calls `lead_conflict_mediations::cancel_active_by_run_tx` in the same transaction as agent execution and work item cancellation.
- MCP `workflow_conflict.lead_mediation.execution_attempts` is present and tested.
- GraphQL `workflowConflict.leadMediation.executionAttempts` is present and tested.
- Status update `attempt_number` now reflects the durable count of mediation-owned execution rows instead of being hard-coded to `1` when attempts exist.
- The owner-field deviation from literal `run_id` and `mediation_owner_token` columns is documented in `docs/proposals/017-evidence/phase-b-mediation-execution-fields-equivalence.md` and gate-proven by `p017_mediation_execution_fields_equivalence`.
- P017 metric helpers now have representative DB tests and some production callers.
- Phase A conflict/advisory/cursor behavior, Phase B mediation lifecycle, Phase C exactly-one-lead validation, dogfood evidence, and external catalog evidence continue to pass the canonical gate.

Divergences:

- Execution-attempt readback exposes `cost` and `transcript_ref` fields, but both are currently null per attempt. MCP comments state per-execution cost and transcript linkage are not yet persisted.
- Attempt artifacts are correlated best-effort by run artifact `agent_id`, not by the proposal's mediation artifact namespace or direct AgentExecution attempt linkage.
- P017's operational metric list is still broader than the implemented runtime emission. `duplicate_mediation_session_total`, `report_readback_completeness`, and `phase_c_lead_inventory_external_catalog_total` were not found as emitted metrics; Phase C validation runtime emission records the pass path but not fail-closed compile outcomes.
- The implementation uses documented equivalence for direct `agent_executions.run_id` and `mediation_owner_token` rather than adding those literal columns.

Ambiguities / evidence gaps:

- No live provider-backed daemon run was executed during the audit.
- Dogfood evidence is an operator-approved fixture/log artifact, not replayed live during this audit.
- The gate now checks the major prior blockers, but it does not fail on null per-attempt cost/transcript refs or on the full P017 operational metric list.

## Requirement Summary

| ID | Requirement | Status |
|---|---|---|
| REQ-001 | Graph-authoritative transition selection and advisory non-authority | Implemented |
| REQ-002 | Blocking `WorkflowConflictRecord`, fingerprint, run blocking, and cursor truth | Implemented |
| REQ-003 | Non-blocking advisory rejection history and report/API readback | Implemented |
| REQ-004 | Implementation-entry handoff latest-summary/MCP/GraphQL readback | Implemented |
| REQ-005 | Canonical `proposal-017` gate | Implemented |
| REQ-006 | Phase B resolver, mediation record, lead invocation, and confirmation settlement | Implemented |
| REQ-007 | Mediation-owned `AgentExecution` owner model with null `stage_execution_id` | Implemented by documented equivalence |
| REQ-008 | Owner-adjacent retry budget and artifact source-generation claims | Implemented |
| REQ-009 | Phase C exactly-one `system_role=lead` executable validation | Implemented |
| REQ-010 | Sanitized GraphQL/MCP mediation readback | Implemented |
| REQ-011 | Mediation execution-attempt readback under workflow conflict | Partially Implemented |
| REQ-012 | Cancellation/resume invariants for mediation-owned executions | Implemented |
| REQ-013 | Rollout metrics, dogfood, known-issues, and external catalog evidence | Partially Implemented |

## Detailed REQ Audit

### REQ-001: Graph-Authoritative Transition Selection

Proposal source: Phase A transition authority and workflow conflict acceptance criteria.

Status: Implemented.

Evidence: The canonical gate passed workflow and engine P017 tests covering graph-authoritative selection, no-match conflicts, ambiguous transition blocking, non-blocking advisory rejection, and legal transition resolution.

Implementation mapping: `control-plane/crates/engine/src/orchestrator.rs`, workflow lint/tests, and P017 engine tests.

Gap / note: No gap found.

### REQ-002: Blocking Conflict Persistence And Cursor Truth

Proposal source: Phase A blocking `WorkflowConflictRecord`, fingerprint, run blocking, and cursor truth commitments.

Status: Implemented.

Evidence: Gate-covered DB/domain/engine tests persist current blocking conflicts, supersede resolved conflicts, store terminal-unverifiable state, and resume selected transition cursors.

Implementation mapping: `control-plane/crates/db/src/repos/workflow_conflicts.rs`, `control-plane/crates/engine/src/orchestrator.rs`, and P017 persistence/engine tests.

Gap / note: No gap found.

### REQ-003: Advisory Rejection History

Proposal source: Phase A non-blocking advisory rejection and report/API visibility commitments.

Status: Implemented.

Evidence: Gate-covered engine and DB tests record non-blocking advisory rejections. MCP and GraphQL conflict readback tests passed.

Implementation mapping: Engine advisory rejection handling and report/API projections.

Gap / note: No gap found.

### REQ-004: Implementation Handoff Readback

Proposal source: Implementation-entry handoff status and code-writer start blocking commitments.

Status: Implemented.

Evidence: The gate ran implementation handoff tests covering blocked-before-invoke and proposal-current snapshot behavior. MCP handoff readback remains exposed through `implementation_handoff_status_json`.

Implementation mapping: `control-plane/crates/engine/tests/integration.rs` and `control-plane/crates/mcp-server/src/tools/reports.rs`.

Gap / note: No gap found.

### REQ-005: Canonical Gate

Proposal source: P017 gate and acceptance evidence requirements.

Status: Implemented.

Evidence: `./scripts/test-gate.sh proposal-017` passed on `1f29206b28798be42fb51b4abb74793434b8cef3`. The gate now also verifies presence of the cancellation cascade, execution-attempt readback tests, equivalence proof, and metric emit tests at `scripts/test-gate.sh:1878`.

Implementation mapping: `scripts/test-gate.sh`.

Gap / note: The gate is substantially stronger, but READY-001 records residual proposal gaps not yet covered by gate assertions.

### REQ-006: Phase B Lead Mediation Lifecycle

Proposal source: Phase B lead resolver, mediation record, lead invocation, confirmation, and settlement commitments.

Status: Implemented.

Evidence: Resolver, mediation lifecycle, and confirmation paths are implemented and gate-covered. Mediation-owned `InvokeAgent` work uses null stage ownership; confirmation resolution routes through the mediation settlement service.

Implementation mapping: `control-plane/crates/engine/src/orchestrator.rs`, `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/engine/src/command_handler.rs`, and `p017_mediation_record_lifecycle`.

Gap / note: No lifecycle creation/settlement gap found.

### REQ-007: Mediation-Owned AgentExecution Owner Model

Proposal source: Phase B migration contract around `owner_kind=lead_conflict_mediation`, nullable `stage_execution_id`, `owner_id`, `run_id`, `mediation_owner_token`, and lead mediation linkage.

Status: Implemented by documented equivalence.

Evidence type: code, migration, tests-run, evidence artifact.

Evidence references:

- `control-plane/crates/db/migrations/029_p017_nullable_mediation_stage_execution.sql` rebuilds `agent_executions` with nullable `stage_execution_id`, `owner_kind`, `owner_id`, `lead_mediation_record_id`, and owner CHECK constraints.
- `control-plane/crates/db/src/repos/agent_executions.rs:395` adds `list_by_mediation_id`.
- `control-plane/crates/db/src/repos/agent_executions.rs:422` keeps owner-aware run cancellation.
- `docs/proposals/017-evidence/phase-b-mediation-execution-fields-equivalence.md:19` records the deliberate design deviation from literal `run_id` and `mediation_owner_token` columns.
- `control-plane/crates/engine/tests/integration.rs:9805` proves equivalence for run identity, cancellation, readback, and idempotency.

Implementation mapping: DB migration, domain agent model, owner-aware repositories, equivalence record, and gate-required proof test.

Gap / note: This is not a literal schema match to the proposal text at lines 512-515. It is accepted here as implemented because the current repository truth includes a deliberate equivalence record plus executable proof for the behaviors those columns were meant to support.

### REQ-008: Owner-Adjacent Retry And Artifact Claims

Proposal source: Phase B owner-adjacent retry budget and artifact source-generation claim commitments.

Status: Implemented.

Evidence: Migration `029_p017_nullable_mediation_stage_execution.sql` rebuilds retry budget ledger and artifact source-generation claims with owner keys. Repository APIs use `OwnerKind` and owner IDs. The P017 gate includes DB coverage for owner-adjacent persistence.

Implementation mapping: `control-plane/crates/db/src/repos/agent_retry_budget_ledger.rs`, `control-plane/crates/db/src/repos/artifact_contracts.rs`, and DB tests.

Gap / note: No gap found for owner-keyed repository behavior.

### REQ-009: Phase C Exactly-One Lead Validation

Proposal source: Phase C mandatory executable catalog validation requiring exactly one `system_role=lead` with resolution contract.

Status: Implemented.

Evidence: `AgentEntry` includes `system_role`; workflow validation rejects missing or duplicate leads and rejects leads without a resolution contract. The compiler invokes validation, and the P017 gate ran the matching workflow tests.

Implementation mapping: `control-plane/crates/workflow/src/catalog.rs`, `control-plane/crates/workflow/src/compiler.rs`, and workflow integration tests.

Gap / note: No static validation gap found.

### REQ-010: Sanitized GraphQL/MCP Mediation Readback

Proposal source: Phase B and AC-027 sanitized live GraphQL/MCP mediation status updates with timestamp, attempt number, and redaction.

Status: Implemented.

Evidence type: code, tests-run.

Evidence references:

- `control-plane/crates/mcp-server/src/tools/reports.rs:155` returns sanitized mediation fields.
- `control-plane/crates/mcp-server/src/tools/reports.rs:181` sets the status update attempt number from execution attempt count.
- `control-plane/crates/graphql-server/src/types/run.rs:200` exposes `GqlLeadMediation`.
- `control-plane/crates/graphql-server/src/types/run.rs:315` builds mediation readback with attempts.
- MCP and GraphQL tests assert `operator_rationale` is not leaked.

Implementation mapping: MCP reports payload and GraphQL run conflict projection.

Gap / note: The readback remains a current-status projection, not a durable multi-event status-history table. The explicit redaction and attempt-number requirements are satisfied for the current readback surface.

### REQ-011: Mediation Execution-Attempt Readback

Proposal source: P017 MCP `reports.get` contract at lines 561-565, GraphQL owner-aware execution shape, AC-009, AC-015, and AC-027.

Status: Partially Implemented.

Evidence type: code, tests-run.

Evidence references:

- `control-plane/crates/mcp-server/src/tools/reports.rs:177` builds `execution_attempts`.
- `control-plane/crates/mcp-server/src/tools/reports.rs:305` projects owner identity, nullable stage execution ID, provider/model, timing, runtime facts, watchdog, cost, transcript ref, and artifacts fields.
- `control-plane/crates/graphql-server/src/types/run.rs:219` exposes `execution_attempts`.
- `control-plane/crates/graphql-server/src/types/run.rs:234` defines `GqlMediationExecutionAttempt`.
- `control-plane/crates/mcp-server/src/tools/reports.rs:1509` and `control-plane/crates/graphql-server/src/schema.rs:2453` test MCP/GraphQL attempt arrays.

Implementation mapping: MCP/GraphQL conflict mediation readback and `agent_executions::list_by_mediation_id`.

Gap / note: The core readback array is implemented, closing the prior missing-field blocker. It is still incomplete against the full proposal because per-attempt `cost` and `transcript_ref` are always null (`mcp-server/src/tools/reports.rs:322`, `graphql-server/src/types/run.rs:408`), and artifacts are best-effort correlated by `agent_id` rather than linked through the mediation artifact namespace or direct AgentExecution attempt refs. The proposal explicitly requires transcript refs, cost, artifact refs, and run-level cost totals for mediation-owned executions.

### REQ-012: Cancellation And Resume Invariants

Proposal source: Phase B cancellation/resume contract, including same-transaction cancellation of active agent executions and linked lead mediation records.

Status: Implemented.

Evidence type: code, tests-run.

Evidence references:

- `control-plane/crates/engine/src/cancellation.rs:99` cancels matching agent executions and work items.
- `control-plane/crates/engine/src/cancellation.rs:109` calls `lead_conflict_mediations::cancel_active_by_run_tx` in the same transaction.
- `control-plane/crates/db/src/repos/lead_conflict_mediations.rs:276` transitions non-terminal mediations for the run to `canceled`, setting settlement result/action and `settled_at`.
- `control-plane/crates/engine/tests/integration.rs:9652` proves the cancellation cascade and idempotency.

Implementation mapping: Run cancellation service, lead mediation repository, and gate-required integration test.

Gap / note: The prior critical cancellation blocker is closed.

### REQ-013: Rollout Metrics And Evidence

Proposal source: P017 metrics, dogfood, known-issues, external catalog inventory, and rollout evidence requirements.

Status: Partially Implemented.

Evidence type: migration, telemetry, tests-run, evidence artifacts.

Evidence references:

- `control-plane/crates/db/src/repos/workflow_conflicts.rs:346` emits `phase_c_validation_outcome_total`.
- `control-plane/crates/db/src/repos/workflow_conflicts.rs:386` emits `lead_mediation_attempt_total`.
- `control-plane/crates/db/src/repos/workflow_conflicts.rs:427` emits `external_catalog_warning_total`.
- `control-plane/crates/engine/src/command_handler.rs:596` records the Phase C compile pass path.
- `control-plane/crates/engine/src/executor.rs:4310` records lead mediation attempt completion.
- `control-plane/crates/engine/src/command_handler.rs:1131` records external catalog warning decisions for legacy discovery overrides.
- `control-plane/crates/db/tests/proposal_017_workflow_conflict_persistence.rs:471`, `:502`, and `:545` cover the three metric helpers.
- Evidence artifacts for dogfood and external catalog inventory are gate-checked.

Implementation mapping: Metric schema, workflow conflict metrics helpers, production callers, and evidence gate tests.

Gap / note: The previous helper-only issue is partially closed. The full proposal metric list still exceeds implementation evidence: `duplicate_mediation_session_total`, `report_readback_completeness`, and `phase_c_lead_inventory_external_catalog_total` were not found as runtime emissions, and the Phase C production caller records the pass path only.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Execution-attempt readback has null cost/transcript refs; rollout metrics are incomplete | High |
| Rust architecture | Pass with note | Literal owner fields are replaced by documented/gate-proven equivalence | Medium-high |
| Rust reliability | Pass | Cancellation cascade is now implemented and gate-covered | High |
| API contract | Partial | Attempt shape exists, but some required attempt fields are placeholders | High |
| Observability/rollout | Partial | Only part of the committed metric inventory has runtime emission | Medium |
| Execution truth | Partial | Cancellation truth is fixed; transcript/cost/artifact attempt truth is not fully linked | High |
| Release readiness | Not Ready | Passing gate does not cover all remaining explicit P017 commitments | High |

## Routed Specialist Findings

### API-002 / `api_contract_reviewer`

Severity: Major. Confidence: High.

Related proposal items: REQ-011, AC-009, AC-015, AC-027.

Evidence: MCP and GraphQL now expose execution-attempt arrays, but per-attempt `cost` is null and `transcript_ref` is null. MCP comments at `control-plane/crates/mcp-server/src/tools/reports.rs:222` state per-execution cost and transcript linkage are not yet persisted. GraphQL mirrors this at `control-plane/crates/graphql-server/src/types/run.rs:249` and `:252`.

Why it matters: P017 does not only require the array to exist; it requires mediation-owned attempts to preserve transcript refs, cost attribution, output artifacts, and runtime facts. Operators can see attempts now, but not the full committed attempt-level audit trail.

Recommended action: Persist or link transcript refs and per-attempt cost/cost-attribution data to the mediation-owned `AgentExecution`, and project direct owner-aware artifact refs instead of best-effort agent-id correlation.

Acceptance criteria: MCP and GraphQL tests assert non-null transcript refs, non-null or explicitly aggregated cost attribution, direct artifact refs for the mediation namespace, and unchanged redaction.

### OPS-002 / `observability_rollout_reviewer`

Severity: Major. Confidence: Medium.

Related proposal items: REQ-013 and P017 operational metrics.

Evidence: The implementation now emits three P017 metric names, but searches found no runtime emission for `duplicate_mediation_session_total`, `report_readback_completeness`, or `phase_c_lead_inventory_external_catalog_total`. The `phase_c_validation_outcome_total` production caller records successful compile/start only.

Why it matters: The rollout dashboard can still miss duplicate mediation sessions, report readback completeness, external inventory enforcement outcomes, and fail-closed Phase C validation outcomes.

Recommended action: Either wire runtime emission for the remaining committed metrics or narrow the proposal/reference metric contract with an explicit accepted deferral.

Acceptance criteria: The P017 gate proves representative runtime paths insert every committed metric name, including failure outcomes where applicable, or the proposal/reference docs state the deferral and the gate checks that narrowed contract.

### READY-001 / Readiness

Severity: Major. Confidence: High.

Related proposal items: REQ-005, REQ-011, REQ-013.

Evidence: `./scripts/test-gate.sh proposal-017` passed, including the new closure tests. The gate does not assert populated per-attempt cost/transcript refs or full operational metric inventory coverage.

Why it matters: A passing gate is no longer misleading for the prior critical blockers, but it still does not prove the complete P017 acceptance contract.

Recommended action: Expand the gate to fail on null attempt cost/transcript refs and on missing committed metric emissions, or explicitly document those as accepted post-P017 deferrals.

Acceptance criteria: A gate failure is reproducible before the remaining fixes and passes after the full P017 contract is met or explicitly narrowed.

## Readiness Checklist

| Area | Result | Notes |
|---|---|---|
| Canonical gate | Pass | `./scripts/test-gate.sh proposal-017` passed on audited HEAD. |
| Swift P017 tests | Pass | Gate ran `Chainworks ForgeTests/Proposal017Tests`; UI tests remain remote-only/out of scope. |
| Rust targeted tests | Pass | Workflow, evidence, domain, DB, MCP, GraphQL, and engine P017 filters passed. |
| Core Phase A conflict flow | Pass | Conflict/advisory/cursor behavior covered by tests. |
| Phase B resolver and mediation creation | Pass | Resolver populated; mediation lifecycle path and test exist. |
| Mediation-owned execution identity | Pass with note | Null-stage owner model exists; literal `run_id`/`mediation_owner_token` replaced by documented equivalence and proof test. |
| Owner-adjacent retry/artifact claims | Pass | Owner-keyed repositories and tests exist. |
| Phase C system lead validation | Pass | Catalog schema/compiler/tests enforce exactly one lead. |
| API parity/readback | Partial | Execution-attempt arrays exist; cost/transcript/artifact linkage remains incomplete. |
| Reliability/cancellation | Pass | Mediation records are canceled with run cancellation and idempotency is tested. |
| Redaction/privacy | Pass with residual risk | GraphQL/MCP tests exclude `operator_rationale`; no live transcript export was validated. |
| Rollout/metrics | Partial | Some runtime emissions exist; not all committed P017 metric names are wired. |
| Full regression | Not run | Negative readiness is established despite the passing canonical P017 gate. |

UI/UX checklist items: Empty/loading/offline/permission states, accessibility, localization, and macOS UI affordances are out of P017 conformance scope after the UI DB cutover amendment.

Privacy/permissions/entitlements: No new entitlement or auth-boundary behavior was audited. Redaction was validated at source/test level for GraphQL/MCP mediation readback only.

## Verification Log

Commands run:

- `git rev-parse HEAD`
- `git merge-base HEAD origin/main`
- `git status -sb`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md`
- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py /Users/user/Documents/Chainworks Forge/docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md`
- `git log --oneline --decorate --max-count=8`
- `git diff --stat c750b72140f50925b68e5b6c10b4214648c70f6c...HEAD`
- `./scripts/test-gate.sh proposal-017`
- Focused source searches and reads across `control-plane/crates/db`, `control-plane/crates/engine`, `control-plane/crates/graphql-server`, `control-plane/crates/mcp-server`, `control-plane/crates/workflow`, `docs/reference`, `docs/proposals/017-evidence`, and `scripts/test-gate.sh`.

Canonical gate result:

- Swift `Chainworks ForgeTests/Proposal017Tests`: 16 tests passed.
- DB P017 persistence tests: 11 tests passed, including the three new metric tests.
- MCP P017 tests: 3 tests passed, including `proposal_017_workflow_conflict_lead_mediation_execution_attempts`.
- GraphQL P017 tests: 4 tests passed, including `proposal_017_run_query_exposes_lead_mediation_execution_attempts`.
- Engine P017 filters passed, including `p017_mediation_cancel_run_cascade`, `p017_mediation_execution_fields_equivalence`, and `p017_mediation_record_lifecycle`.
- Warnings observed: existing Rust dead-code warnings in `acp` and `engine`; they did not fail the gate.
- Final gate line: `==> Proposal 017 gate passed`.

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

Reviewer-selection reuse: Not reused.

The current implementation closes the prior critical cancellation blocker and the prior missing execution-attempt readback blocker at the structural/API level. It is still not fully P017-complete because execution attempts do not yet carry populated per-attempt cost/transcript refs or direct mediation artifact refs, and the operational metric inventory is still narrower than the proposal.

Recommended next actions:

1. Persist/link transcript refs, per-attempt cost attribution, and direct mediation artifact refs for mediation-owned `AgentExecution` attempts.
2. Expand MCP/GraphQL tests to assert those fields are populated and redacted correctly.
3. Wire or explicitly defer the remaining committed metrics: duplicate mediation sessions, readback completeness, Phase C lead inventory external catalog totals, and Phase C failure outcomes.
4. Expand `proposal-017` gate coverage so these remaining explicit contract gaps fail before closeout.
