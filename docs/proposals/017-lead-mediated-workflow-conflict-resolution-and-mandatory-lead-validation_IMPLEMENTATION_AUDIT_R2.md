# Proposal 017 Implementation Audit R2

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md` |
| Audit report | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R2.md` |
| Audit date | 2026-04-27 |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` |
| Audited HEAD | `c750b72140f50925b68e5b6c10b4214648c70f6c` |
| Compare base | `c750b72140f50925b68e5b6c10b4214648c70f6c` (`merge-base HEAD origin/main`) |
| Implementation target | Current worktree at `main`; P017 control-plane scope only |
| Worktree note | Audit started from a clean `main...origin/main` tree. During/after validation unrelated modified files appeared at `control-plane/crates/db/src/repos/projections.rs` and `control-plane/crates/db/tests/integration.rs`; this audit did not create or rely on those changes. |
| Canonical gate | `./scripts/test-gate.sh proposal-017` passed |
| Proposal state | Active for implementation-readiness review |
| Prior reviewer reuse | Not reused; no prior proposal-review artifacts found |
| Overall conformance | Not Implemented |
| Overall readiness | Not Ready |
| Audit confidence | High for source-level blockers, medium-high overall because no live provider-backed daemon run was executed |

## Implementation Target And Scope

P017's post-UI-DB-cutover amendment makes the Rust control plane the acceptance target. This audit inspected SQLite persistence, domain contracts, workflow compiler behavior, engine/orchestrator/executor behavior, MCP report/debug readback, GraphQL read projections, proposal evidence artifacts, and the canonical gate.

Out of scope for conformance: SwiftData storage, Swift UI/runtime transition authority, legacy Swift report generation, and concrete macOS UI mediation surfaces. These remain future thin-client or deletion/quarantine work, not P017 blockers.

Platform/product scope:

| Dimension | Scope |
|---|---|
| Apple | macOS app present, but UI implementation is out of P017 conformance scope |
| Backend/service | Rust control-plane service, worker, API, data, rollout, and cross-stack workflow/catalog contracts |
| Product metrics | In scope only where P017 explicitly commits dogfood, operator-feedback, or rollout metric evidence |

## Prior Review Reuse

The discovery helper found no proposal-review artifacts for P017. Existing implementation-audit reports were not used for reviewer selection, per the skill rule.

Selected reviewers:

| Reviewer | Reason |
|---|---|
| `rust_arch_reviewer` | Rust domain, DB migrations, workflow compiler, engine, and owner model are central. |
| `rust_reliability_reviewer` | P017 covers retry, cancellation, resume, queue ownership, idempotency, and stale output handling. |
| `api_contract_reviewer` | GraphQL, MCP, workflow YAML, agent catalog, and report payload contracts are in scope. |
| `observability_rollout_reviewer` | P017 includes migration, gate, rollout, metrics, dogfood, and external catalog evidence. |
| `chainworks_execution_truth_reviewer` | P017 changes durable Run/Stage/Agent/Approval/artifact/recovery truth. |

Rejected close alternatives:

| Reviewer | Reason |
|---|---|
| `macos_ui_reviewer` | P017 explicitly excludes UI implementation after the UI DB cutover. |
| `apple_arch_reviewer` | Swift client state and providers are not the acceptance surface. |
| `rust_security_reviewer` | Redaction is covered by API-contract review; no auth/unsafe/security boundary change dominated this slice. |
| Product reviewer | Metrics and decision records are covered by observability rollout; product review was not explicitly requested. |
| iOS reviewer | No iOS target is introduced. |

## Contract Summary

The proposal commits to:

- Phase A graph-authoritative transition truth, blocking conflict persistence, advisory rejection history, cursor/resume behavior, report/API readback, and canonical test gate.
- Phase B lead mediation using exactly-one lead resolution, durable `LeadConflictMediationRecord`, normal mediation-owned `AgentExecution`, owner-aware retry/artifact/cost/runtime-fact behavior, confirmation settlement, cancellation, resume, dogfood, and sanitized GraphQL/MCP readback.
- Phase C mandatory exactly-one `system_role=lead` executable catalog validation, external catalog enforcement inventory, typed warnings or waiver evidence, and validation readback/metrics.
- No synthetic `StageExecution` for mediation-owned work, and no UI-owned DB truth.

Primary service flows audited:

1. Graph-authoritative ambiguous/no-match transition persists a blocking workflow conflict and cursor.
2. Invalid advisory next-stage hint records a non-blocking advisory rejection.
3. Phase B flag-enabled conflict resolves a lead, creates mediation truth, and enqueues a mediation-owned agent invocation.
4. Lead output validation creates confirmation/settlement state and exposes sanitized operator readback.
5. Cancellation, retry, runtime facts, artifacts, cost, GraphQL, MCP, and rollout metrics preserve mediation-owned truth.

## Proposal Fidelity Inventory

Matches:

- The current HEAD includes nullable mediation-owned `AgentExecution` support and DB checks.
- The Phase B lead resolver map is populated with attested bundled workflow/catalog entries.
- `examples/agents/agents.yaml` declares `system_role: lead` and `lead_resolution_contract`.
- Workflow compile validates exactly one system lead.
- Owner-aware retry budget and artifact source-generation claim repositories exist.
- GraphQL and MCP now expose sanitized lead mediation status and avoid `operator_rationale` in readback tests.
- Phase B dogfood, Phase C external catalog inventory, and Phase A known-issues evidence artifacts exist.
- The canonical P017 gate exists, checks the non-empty resolver map, and passed on the audited HEAD.

Divergences:

- `cancel_running_by_run` cancels mediation-owned `AgentExecution` rows but does not transition linked `LeadConflictMediationRecord` rows to `canceled` in the same transaction.
- GraphQL/MCP mediation readback does not expose the promised `execution_attempts` / mediation execution attempt shape with owner, nullable stage, runtime fact, transcript, watchdog, and cost details.
- Mediation status readback synthesizes a single `status_updates` item with `attempt_number = 1`; there is no durable attempt/status history or retry-attempt count.
- `agent_executions` still lacks the explicit direct `run_id` and `mediation_owner_token` fields named in the Phase B migration contract, relying instead on stage/mediation joins and owner lineage.
- Several P017 metric names exist in schema/docs, but only a subset is wired to runtime emission; `phase_c_validation_outcome_total` and lead-mediation attempt metrics have storage/helper evidence but no production caller found.

Ambiguities / evidence gaps:

- No live provider-backed daemon run was executed during the audit.
- The dogfood evidence is an operator-approved fixture record, not replayed run logs.
- The gate's `p017_mediation_` filter still runs only `p017_mediation_record_lifecycle`; other mediation confirmation/resolver/settlement tests are covered only if they match another gate filter.

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

Status: Implemented.

Evidence: `./scripts/test-gate.sh proposal-017` passed. The engine proposal tests include graph conflict creation, no-match handling, terminal-unverifiable conflict history, and legal transition resolution. The proposal gate ran `cargo test -p workflow proposal_017_` and `cargo test -p engine proposal_017_`.

### REQ-002: Blocking Conflict Persistence And Cursor Truth

Status: Implemented.

Evidence: The orchestrator persists workflow conflicts and blocks the run at `control-plane/crates/engine/src/orchestrator.rs:2020`, then records conflict cursor status and resume policy at `control-plane/crates/engine/src/orchestrator.rs:2077`. The DB/domain P017 conflict tests passed under the gate.

### REQ-003: Advisory Rejection History

Status: Implemented.

Evidence: P017 engine tests cover non-blocking advisory rejection recording, and report/API conflict readback tests pass under MCP and GraphQL gate filters.

### REQ-004: Implementation Handoff Readback

Status: Implemented.

Evidence: The gate ran implementation handoff tests in `control-plane/crates/engine/tests/integration.rs`, including blocked-before-invoke and proposal snapshot cases. MCP handoff readback remains exposed through `implementation_handoff_status_json`.

### REQ-005: Canonical Gate

Status: Implemented.

Evidence: `scripts/test-gate.sh:1807` registers `proposal-017|p017`. It verifies required Phase 0 artifacts, rejects an empty lead resolver map at `scripts/test-gate.sh:1826`, runs Swift P017 tests, and runs targeted Rust workflow, evidence, domain, DB, MCP, GraphQL, and engine filters. The command passed on audited HEAD `c750b72140f50925b68e5b6c10b4214648c70f6c`.

### REQ-006: Phase B Lead Mediation Lifecycle

Status: Implemented.

Evidence: The resolver map has attested entries for bundled workflows. The orchestrator loads it and creates mediation records at `control-plane/crates/engine/src/orchestrator.rs:2125` and `control-plane/crates/engine/src/orchestrator.rs:2175`, then enqueues mediation-owned `InvokeAgent` work with `stage_execution_id: null` at `control-plane/crates/engine/src/orchestrator.rs:2341`. The executor validates lead output and creates mediation confirmations at `control-plane/crates/engine/src/executor.rs:4093`. Confirmation resolution routes through the settlement service in `control-plane/crates/engine/src/command_handler.rs:1785`.

### REQ-007: Mediation-Owned AgentExecution Owner Model

Status: Partially Implemented.

Evidence: `AgentExecution.stage_execution_id` is now optional at `control-plane/crates/domain/src/agent.rs:43`; migration `029_p017_nullable_mediation_stage_execution.sql:7` rebuilds `agent_executions` with nullable `stage_execution_id` and owner CHECK constraints; the executor leaves mediation-owned stage execution IDs as `None` at `control-plane/crates/engine/src/executor.rs:2830`. DB tests prove mediation-owned inserts at `control-plane/crates/db/tests/integration.rs:199`.

Gap: The migration contract also named direct `run_id` and `mediation_owner_token` fields on `agent_executions`. Current schema lacks those and relies on joins through stage or mediation tables plus `owner_execution_lineage_id`. More importantly, cancellation and owner-aware readback still have gaps captured in REQ-011 and REQ-012.

### REQ-008: Owner-Adjacent Retry And Artifact Claims

Status: Implemented.

Evidence: Migration `029_p017_nullable_mediation_stage_execution.sql:89` rebuilds `agent_retry_budget_ledger` with owner keys, and `029_p017_nullable_mediation_stage_execution.sql:131` rebuilds artifact source-generation claims with owner keys. Repository APIs use `OwnerKind` and owner IDs at `control-plane/crates/db/src/repos/agent_retry_budget_ledger.rs:66` and `control-plane/crates/db/src/repos/artifact_contracts.rs:262`. DB tests cover mediation-owned quota and claims at `control-plane/crates/db/tests/integration.rs:255`.

### REQ-009: Phase C Exactly-One Lead Validation

Status: Implemented.

Evidence: `AgentEntry` has `system_role` at `control-plane/crates/workflow/src/catalog.rs:115`, `validate_catalog_has_exactly_one_system_lead` enforces exactly one lead plus profile/permission/contract coverage at `control-plane/crates/workflow/src/catalog.rs:144`, and the workflow compiler invokes it at `control-plane/crates/workflow/src/compiler.rs:30`. Workflow tests cover missing, duplicate, and valid lead declarations.

### REQ-010: Sanitized GraphQL/MCP Mediation Readback

Status: Partially Implemented.

Evidence: MCP `workflow_conflict_json` enriches current conflicts with `lead_mediation` at `control-plane/crates/mcp-server/src/tools/reports.rs:132`; GraphQL exposes `GqlLeadMediation` at `control-plane/crates/graphql-server/src/types/run.rs:198`; both surfaces have tests that exclude `operator_rationale`.

Gap: The readback has only a synthesized current status update and does not expose validation outcome as a typed field, `requires_operator_confirmation`, or short lead-recommendation summaries from the LeadResolutionContract. Some of this can be inferred from status and confirmation ID, but P017 asks for explicit safe mediation readback semantics.

### REQ-011: Mediation Execution-Attempt Readback

Status: Missing.

Evidence: P017 requires `workflow_conflict.current.lead_mediation.execution_attempts` in MCP and an owner-aware GraphQL mediation execution shape with owner kind, owner ID, nullable stage execution ID, mediation record ID, status, provider/model, timing, watchdog outcome, runtime facts, transcript refs, artifacts, and cost. Searching current GraphQL/MCP code found no `execution_attempts`, `GqlLeadMediationExecution`, or equivalent field. MCP `lead_mediation_readback_json` returns status fields only at `control-plane/crates/mcp-server/src/tools/reports.rs:155`; GraphQL `GqlLeadMediation` has status fields only at `control-plane/crates/graphql-server/src/types/run.rs:198`.

### REQ-012: Cancellation And Resume Invariants

Status: Partially Implemented.

Evidence: The executor treats terminal/canceled/superseded mediation output as stale and ignores late output at `control-plane/crates/engine/src/executor.rs:3672`. Startup repair and cursor tests pass under the gate.

Gap: P017 explicitly says `cancel_running_by_run` must cancel active agent executions by run ID and, for `owner_kind=lead_conflict_mediation`, transition the linked `LeadConflictMediationRecord` to `canceled` in the same repository transaction. The current cancellation path calls `agent_executions::cancel_running_by_run_tx` and `work_items::cancel_running_by_run_tx` at `control-plane/crates/engine/src/cancellation.rs:96`. The repository update at `control-plane/crates/db/src/repos/agent_executions.rs:426` updates only `agent_executions`; no caller updates `lead_conflict_mediations` to `canceled`.

### REQ-013: Rollout Metrics And Evidence

Status: Partially Implemented.

Evidence: Metric storage exists in `control-plane/crates/db/migrations/030_p017_workflow_conflict_metric_events.sql:1`. Recovery choice and terminal conflict metrics are emitted through `control-plane/crates/db/src/repos/workflow_conflicts.rs:317` and `control-plane/crates/db/src/repos/workflow_conflicts.rs:611`. Dogfood, external catalog, and known-issues evidence artifacts exist and are checked by `control-plane/crates/workflow/tests/proposal_017_evidence_gate.rs:25`.

Gap: Some committed P017 metric names are schema/documentation only or helper-only. `phase_c_validation_outcome_total` has a helper at `control-plane/crates/db/src/repos/workflow_conflicts.rs:346`, but no production caller was found. Lead-mediation attempt and external-catalog warning metrics are present in migration/doc/evidence strings but no runtime emission path was found.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Not Implemented | Missing execution-attempt readback and incomplete cancellation semantics | High |
| Rust architecture | Partial | Owner model is mostly migrated but not all named fields/readback contracts exist | High |
| Rust reliability | Not Ready | Run cancellation can leave active mediation records non-terminal | High |
| API contract | Not Ready | GraphQL/MCP lack promised mediation execution-attempt shape | High |
| Observability/rollout | Partial | Metrics/evidence improved, but some runtime metric emission paths are absent | Medium |
| Execution truth | Not Ready | Agent execution cancellation and mediation truth can diverge | High |
| Release readiness | Not Ready | Canonical gate passes but does not catch the remaining blockers | High |

## Routed Specialist Findings

### REL-001 / `rust_reliability_reviewer`

Severity: Critical. Confidence: High.

Related requirements: REQ-012, P017 migration contract `cancel_running_by_run`, AC-015.

Evidence: `control-plane/crates/engine/src/cancellation.rs:96` calls agent/work-item cancellation only. `control-plane/crates/db/src/repos/agent_executions.rs:426` cancels matching agent execution rows but does not update `lead_conflict_mediations`. Search found no cancellation caller for mediation records.

Why it matters: A run cancellation can leave a linked lead mediation record in `queued`, `running`, or `operator_confirmation_required` while the agent execution is canceled. That breaks the proposal's durable mediation truth and makes stale-output, resume, and operator readback ambiguous.

Recommended action: Add a same-transaction repository/service path that transitions active lead mediations for the canceled run to `canceled`, with settlement timestamp, and gate it with an integration test.

Acceptance criteria: Canceling a run with an active mediation-owned execution updates both `agent_executions.status = canceled` and the linked `lead_conflict_mediations.status = canceled` in one transaction; late provider output is ignored without changing conflict/run state.

### API-001 / `api_contract_reviewer`

Severity: Major. Confidence: High.

Related requirements: REQ-011, AC-009, AC-015, AC-027.

Evidence: P017 requires mediation execution attempts under `workflow_conflict.current.lead_mediation.execution_attempts` and an owner-aware GraphQL mediation execution shape. No such field or type exists in GraphQL/MCP code. MCP `lead_mediation_readback_json` returns mediation status fields only. GraphQL `GqlLeadMediation` returns status fields only.

Why it matters: Operators and agents cannot inspect the mediation-owned `AgentExecution` attempt through the workflow conflict surface that P017 designates as authoritative. Runtime facts, transcript refs, watchdog outcome, and cost are not grouped with the conflict mediation.

Recommended action: Add MCP and GraphQL mediation execution-attempt readback, backed by owner-aware `agent_executions::list_by_run` plus runtime facts/artifacts/cost joins, and add parity fixtures.

Acceptance criteria: A mediation-owned `AgentExecution` with null stage execution ID appears under `workflow_conflict.current.lead_mediation.execution_attempts` in MCP and the equivalent GraphQL shape, with owner identity, provider/model, timing, runtime facts summary, transcript/artifact refs, watchdog result, and cost. Stage-scoped fields still exclude it.

### ARCH-001 / `rust_arch_reviewer`

Severity: Major. Confidence: Medium.

Related requirements: REQ-007, P017 Phase B migration contract.

Evidence: Migration `029_p017_nullable_mediation_stage_execution.sql` rebuilds owner-aware tables but `agent_executions` still lacks the explicitly named `run_id` and `mediation_owner_token` fields. The executor uses `owner_execution_lineage_id` as the mediation lineage anchor instead.

Why it matters: The implementation may be functionally close, but it diverges from the explicit schema contract that was intended to make owner-aware run cancellation, readback, and idempotency direct rather than join-dependent.

Recommended action: Either add the named fields and fixtures or record a deliberate design deviation explaining the substitute fields and proving equivalent cancellation, readback, and idempotency behavior.

Acceptance criteria: Owner-aware run/cancellation/readback paths are proven without ambiguity, and the schema contract is either implemented literally or amended with an approved equivalence record.

### OPS-001 / `observability_rollout_reviewer`

Severity: Major. Confidence: Medium.

Related requirements: REQ-013, AC-019, P017 metrics section.

Evidence: Runtime emission exists for recovery action and terminal conflict timing/outcome metrics. No production caller was found for `record_phase_c_validation_outcome_tx`; lead-mediation attempt and external-catalog warning metrics appear in schema/docs/evidence only.

Why it matters: The rollout story can pass fixture gates while some operator feedback and Phase C enforcement metrics remain invisible in real runs.

Recommended action: Wire emission for Phase C validation outcomes, lead mediation attempts by result, and external catalog warning decisions, then add gate assertions.

Acceptance criteria: The P017 gate proves representative runtime paths insert all committed P017 metric event names with bounded labels.

### TRUTH-001 / `chainworks_execution_truth_reviewer`

Severity: Critical. Confidence: High.

Related requirements: REQ-011, REQ-012.

Evidence: Mediation-owned execution state is durable, but linked mediation record cancellation and conflict-scoped execution-attempt readback are incomplete.

Why it matters: P017's core model is that workflow conflict truth, mediation truth, and agent execution truth stay synchronized. The current implementation can split those truths across cancellation and readback surfaces.

Recommended action: Treat cancellation synchronization and conflict-scoped execution-attempt readback as a single closeout slice, because both depend on the same owner-aware execution truth.

Acceptance criteria: For a mediation-owned attempt, current conflict readback, run-level execution truth, cancellation, late-output handling, and settlement all report one consistent terminal story.

### READY-001 / Readiness

Severity: Major. Confidence: High.

Related requirements: REQ-005, REQ-011, REQ-012.

Evidence: The canonical P017 gate passed, but it does not fail on missing mediation execution-attempt readback or missing mediation-record cancellation. The `p017_mediation_` engine filter ran only `p017_mediation_record_lifecycle`.

Why it matters: A passing gate currently overstates implementation readiness for the remaining Phase B owner-aware behavior.

Recommended action: Expand `proposal-017` gate coverage to include cancellation-to-mediation status, MCP/GraphQL execution-attempt readback, attempt number behavior beyond hard-coded `1`, and Phase C/mediation metric emission.

Acceptance criteria: The gate fails before the above blockers are fixed and passes afterward on the same tree.

## Readiness Checklist

| Area | Result | Notes |
|---|---|---|
| Canonical gate | Pass | `./scripts/test-gate.sh proposal-017` passed on audited HEAD. |
| Swift P017 tests | Pass | Gate ran `Chainworks ForgeTests/Proposal017Tests`; UI tests remain remote-only/out of scope. |
| Rust targeted tests | Pass | Workflow, evidence, domain, DB, MCP, GraphQL, and engine P017 filters passed. |
| Core Phase A conflict flow | Pass | Conflict/advisory/cursor behavior covered by tests. |
| Phase B resolver and mediation creation | Pass | Resolver populated; code path and tests exist. |
| Mediation-owned execution identity | Partial | Null stage owner model exists; direct `run_id`/`mediation_owner_token` contract not literal. |
| Owner-adjacent retry/artifact claims | Pass | Owner-keyed repos and tests exist. |
| Phase C system lead validation | Pass | Catalog schema/compiler/tests enforce exactly one lead. |
| API parity/readback | Fail | Execution attempts missing from GraphQL/MCP mediation readback. |
| Reliability/cancellation | Fail | Mediation records are not canceled with run cancellation. |
| Redaction/privacy | Pass with residual risk | GraphQL/MCP tests exclude `operator_rationale`; no live transcript export was validated. |
| Rollout/metrics | Partial | Evidence artifacts and some metrics exist; some runtime emissions missing. |
| Full regression | Not run | Negative readiness is established despite passing canonical P017 gate. |

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

- Passed.
- Notable test counts from visible gate output: GraphQL P017 tests 3 passed; engine P017 lib tests 7 passed; engine P017 integration tests 10 passed; `p017_mediation_` filter 1 passed.
- Warnings were emitted by Swift and Rust builds, but no gate failure occurred.

Validation not performed:

- No live daemon/provider-backed mediation run.
- No local UI smoke tests, per repository policy.
- No full repository regression suite, because this audit already found P017-specific readiness blockers.

## Final Verdict

P017 is substantially advanced but not fully implemented. The current tree fixes many prior control-plane gaps and passes the canonical gate, but full proposal conformance is still blocked by missing conflict-scoped mediation execution-attempt readback and incomplete cancellation synchronization between mediation-owned `AgentExecution` rows and `LeadConflictMediationRecord` rows.

Recommended next actions:

1. Make run cancellation transition active linked lead mediation records to `canceled` in the same transaction as agent/work-item cancellation.
2. Add MCP and GraphQL `workflow_conflict.current.lead_mediation.execution_attempts` readback with owner identity, nullable stage ID, runtime facts, watchdog result, transcript/artifact refs, timing, provider/model, and cost.
3. Replace hard-coded mediation readback `attempt_number = 1` with durable attempt counting or documented single-attempt semantics.
4. Either implement the literal `run_id` and `mediation_owner_token` `agent_executions` fields or add an approved equivalence record for the current join/lineage approach.
5. Wire remaining P017 runtime metrics and expand the canonical gate so these gaps fail before sign-off.
