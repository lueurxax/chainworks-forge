# Proposal 017 Implementation Audit R6

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md` |
| Report | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R6.md` |
| Audit timestamp | 2026-04-28T07:09:45Z / 2026-04-28T10:09:45+0300 |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current worktree on `main` |
| Current HEAD | `0fa242f3ed4cfde511806cfb2b3fbf217832e112` |
| Compare base | Implicit current-branch audit; merge base with `origin/main` is `9042077e7aeedaa8b9bb5d3f10c372851e8e5e6b` |
| Working tree | Dirty before report creation: 4 modified files and 2 untracked files outside this report |
| Proposal state | Active / revised for implementation-readiness review |
| Canonical gate | `./scripts/test-gate.sh proposal-017` |
| Overall conformance | Partial |
| Overall implementation readiness | Not Ready for full P017 closeout |
| Audit confidence | High for code/test evidence; Medium for live provider mediation behavior |

## Implementation Target / Compare Base

The audit target is the exact current worktree at HEAD `0fa242f3ed4cfde511806cfb2b3fbf217832e112`, including uncommitted changes present before this report was written.

Tracked HEAD delta from merge base `9042077e7aeedaa8b9bb5d3f10c372851e8e5e6b` contains:

- `scripts/test-gate.sh`: P017 R5 closure guards for direct artifact linkage, transactional completion/attribution, and metric helper/test presence.
- `control-plane/crates/engine/tests/proposal_065_operator_retry_instruction.rs`: small P065 compatibility test adjustment.
- `docs/proposals/017-evidence/proposal-017-r5-closure-gate-20260427T185600Z.log`: prior gate evidence.
- `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R5_ADDENDUM.md`: prior implementation-audit addendum, not reused for reviewer selection.

Dirty files present before report creation:

- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- `Chainworks Forge/Views/RunsHomeView.swift`
- `Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests.swift`
- `control-plane/crates/graphql-server/src/schema.rs`
- `auto-retry-blocked-runs-known-issues.md`
- `known-issues.md`

The dirty GraphQL diff is P031 artifact payload/readback work plus formatting-only changes around P017 lead-mediation enrichment calls. The canonical P017 gate passed on this dirty tree.

## Prior Proposal-Review Reuse Summary

Reviewer-selection reuse: Not reused.

`discover_prior_review.py` returned no proposal-review artifacts for this proposal. Prior `IMPLEMENTATION_AUDIT` reports and addenda were ignored for reviewer selection per the audit workflow. Historical implementation audits were only treated as context where the current source code and tests independently confirmed or contradicted them.

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | P017's canonical implementation target is Rust control-plane persistence, domain contracts, compiler/engine behavior, and owner-aware execution. |
| `rust_reliability_reviewer` | P017 owns retry, resume, cancellation, terminal outcomes, mediation idempotency, and late-output/attempt handling. |
| `api_contract_reviewer` | P017 commits MCP `reports.get`, GraphQL readback, casing translation, and mediation execution-attempt shapes. |
| `observability_rollout_reviewer` | P017 commits operational metrics, rollout gates, dogfood evidence, known-issues gates, and canonical test-gate enforcement. |
| `chainworks_execution_truth_reviewer` | P017 changes durable Run/Stage/Agent/Approval/artifact/recovery truth and transition cursor behavior. |

## Rejected Close Alternatives

| Reviewer | Reason rejected |
|---|---|
| `macos_ui_reviewer` | Proposal lines 10-23 and 804-823 explicitly move P017 UI out of scope after the UI DB cutover. |
| `apple_arch_reviewer` | Legacy Swift state/provider/runtime code is not the canonical P017 implementation target; Swift P017 tests remain gate coverage, not ownership truth. |
| `apple_ux_reviewer` | Future thin-client display is out of scope; GraphQL/MCP readback is audited through API/contract and execution-truth lenses. |
| `product_reviewer` | Product/dogfood decision evidence is relevant but does not require a separate product lens for this implementation audit. |
| `security_reviewer` | No new auth, public ingress, secret handling, unsafe/FFI, or privacy boundary was introduced beyond redaction commitments covered by API/OPS review. |
| `performance_reviewer` | P017 has no implemented latency/throughput benchmark target; timing targets remain aspirational until dogfood recalibration. |

## Proposal State And Contract Summary

P017 is active and revised for implementation-readiness review. The post UI DB cutover amendment makes the canonical implementation target control-plane only: Rust SQLite persistence, domain contracts, workflow compiler/engine behavior, MCP report/debug readback, and GraphQL read projections. Missing SwiftData, Swift UI, or local UI smoke evidence is not a P017 blocker.

Locked implementation decisions audited here:

- Compiled workflow graph is authoritative for legal progression.
- Agent-authored `next_stage`, `next_action`, `run_state.json`, and narrative transition fields are advisory only.
- Blocking invalid, ambiguous, missing-input, aggregate-conflict, and unverifiable transition outcomes persist `WorkflowConflictRecord`.
- Legal graph progression with rejected advisory hints persists `WorkflowAdvisoryRejectionRecord` and does not set current conflict or `blockedReason`.
- Transition cursor/resume truth must stay aligned with selected graph transitions, blocking conflicts, lead settlement, and terminal outcomes.
- Lead mediation uses a distinct mediation record plus normal owner-aware `AgentExecution` rows, not synthetic stage executions.
- MCP uses snake_case JSON semantics; GraphQL exposes typed fields/enums with surface-appropriate casing.
- Sanitized mediation progress is exposed; raw rationale, prompts, hidden reasoning, and raw transcript text remain out of northbound readback.
- `./scripts/test-gate.sh proposal-017` is the canonical validation gate.

Platform/product scope:

- Apple: macOS UI out of scope for P017 conformance.
- Backend/service: Rust control-plane service, worker/runtime execution paths, SQLite persistence, GraphQL API, MCP report API, workflow compiler, rollout/telemetry.
- Product/readiness: operator-visible recovery truth and measurable rollout gates through control-plane readback and evidence records.

Leading metric: zero runs advance to states absent from the compiled graph, with P017 fixture coverage for graph-authoritative outcomes.

Guardrail metric: zero mediation-owned `AgentExecution` rows appear in stage-scoped GraphQL or `find_by_stage`, while owner-aware run/report readback includes them.

Decision checkpoint: Phase B remains runtime-flag gated until dogfood exit evidence covers sample size, completion rate, duplicate-session rate, readback completeness, time-to-resolution, and operator feedback.

## Primary Implementation Flows

1. Transition authority flow: candidate transition evaluation consumes compiled workflow graph, aggregate artifact truth, and advisory evidence; it selects exactly one legal graph transition or persists a typed conflict.
2. Advisory rejection flow: a legal graph transition advances while a bad advisory hint is persisted as non-blocking history and metrics without setting current conflict or `blockedReason`.
3. Conflict readback/recovery flow: durable conflict truth flows through transition cursor, recovery commands, MCP reports, GraphQL run/readback, latest summary, and terminal history.
4. Lead mediation flow: blocking conflicts create/reuse a mediation record, execute the lead as owner-aware `AgentExecution`, validate output, re-enter graph settlement, or terminalize with sanitized failure context.
5. Rollout/telemetry flow: proposal gate, workflow/catalog validation, known-issues evidence, dogfood evidence, and metric events support rollout decisions and operational readback.

## Implementation Fingerprint

Stack tags:

- Rust control-plane, SQLite, sqlx, async Rust.
- GraphQL server with typed run/readback projections.
- MCP report tooling.
- macOS Swift unit-test bridge for legacy/readback parity, but not P017 ownership truth.

Surface tags:

- Persistence migrations and repositories.
- Engine/orchestrator transition and mediation lifecycle.
- Workflow compiler validation.
- GraphQL/MCP readback.
- Metrics and rollout evidence.
- Canonical gate wrapper.

Risk tags:

- Operational-metric completeness.
- Mediation completion atomicity and retry safety.
- Dirty unrelated P031 worktree changes in audited tree.
- No live provider-backed mediation session executed during this audit.

## Proposal Fidelity / Divergence Inventory

### Matches

- P017 scope is treated as control-plane-only; Swift UI and SwiftData are not blockers.
- Workflow graph authority and fail-closed unknown-artifact classification are implemented and tested.
- Blocking conflicts, non-blocking advisory rejections, current/history behavior, and transition cursor persistence are implemented and tested.
- MCP and GraphQL expose `workflow_conflict` / `workflowConflict` with current, history, advisory rejection, lead mediation, sanitized progress, and execution-attempt readback.
- Owner-aware mediation execution rows use null `stage_execution_id` and do not leak through stage-scoped readback.
- Per-attempt cost/transcript/artifact readback is implemented for MCP and GraphQL, including direct `artifacts.agent_execution_id` linkage.
- Phase C lead validation, external catalog/known-issues evidence, and dogfood exit evidence are covered by gate tests.
- `./scripts/test-gate.sh proposal-017` is registered and passed on the audited tree.

### Divergences

- No explicit P017 behavior was found to diverge in the main transition, persistence, MCP, GraphQL, mediation-owner, or gate flows.
- The operational metrics inventory is not complete as runtime-emittable behavior: some committed metric names are migration-allowed or evidence-record-backed, but lack production metric callers/tests.

### Ambiguities / Evidence Gaps

- The audited tree includes unrelated dirty P031 files. The P017 gate passed with them, but they should be isolated before closeout.
- No live daemon startup or provider-backed mediation dogfood run was executed; proposal readiness mode explicitly does not require daemon startup, cargo-wide tests, benchmarks, load tests, fuzzing, simulator, or UI smoke tests.
- `phase_b_dogfood_mediation_completion_rate` and `phase_b_dogfood_operator_guidance_sufficient_total` are represented in the Phase B dogfood exit record and metric-name migrations, but not as runtime metric events.
- `mediation_retry_budget_exhausted_total` has a DB helper and allowed metric name, but no production caller or focused metric-emission test was found.
- The executor comments claim transcript artifact row insertion is bundled with completion/attribution, but the current code inserts the transcript artifact row before the completion/attribution transaction.

## Requirement Summary

| REQ | Title | Status |
|---|---|---|
| REQ-001 | Control-plane-only P017 implementation target | Implemented |
| REQ-002 | Compiled graph is authoritative for progression | Implemented |
| REQ-003 | Agent-authored transition fields are advisory only | Implemented |
| REQ-004 | Blocking workflow conflicts persist durable current/history truth | Implemented |
| REQ-005 | Non-blocking advisory rejections persist separately | Implemented |
| REQ-006 | Candidate transition evaluation is typed and fail-closed | Implemented |
| REQ-007 | Transition cursor/resume stays aligned with conflict truth | Implemented |
| REQ-008 | MCP, GraphQL, and latest summary expose equivalent workflow_conflict semantics | Implemented |
| REQ-009 | Lead mediation uses owner-aware AgentExecution, not synthetic stages | Implemented |
| REQ-010 | Mediation attempts preserve runtime facts, watchdog, cost, transcript, artifacts, and validation | Implemented with reliability risk |
| REQ-011 | Lead output validation terminalizes malformed/absent/watchdog-expired output | Implemented |
| REQ-012 | Mediation retry, resume, restart, and cancellation are idempotent | Implemented |
| REQ-013 | Stage-scoped readback excludes mediation-owned executions | Implemented |
| REQ-014 | Phase C workflow/catalog lead validation fails typed lead coverage errors | Implemented |
| REQ-015 | Owner-adjacent retry ledger and artifact claims support mediation-owned executions | Implemented |
| REQ-016 | Implementation-entry handoff truth is durable and read back northbound | Implemented |
| REQ-017 | Sanitized mediation progress is exposed without raw rationale/prompts/transcripts | Implemented |
| REQ-018 | Operational metrics are runtime-emittable and tested | Partially Implemented |
| REQ-019 | Bundled simultaneous-transition scan and known-issues evidence gate Phase A | Implemented |
| REQ-020 | Phase B dogfood/default-on gate remains evidence-backed | Implemented as evidence gate; metric-event emission partial under REQ-018 |
| REQ-021 | Canonical P017 gate exists, is listed, and passes | Implemented |

## Detailed REQ Audit

### REQ-001 - Control-plane-only P017 implementation target

- Proposal source: lines 10-23, 103-111, 804-823.
- Status: Implemented.
- Evidence: proposal, code, diff, tests-run.
- References: P017 amendment; `scripts/test-gate.sh` lines 1820-2064; dirty Swift files recorded as out-of-scope context.
- Mapping: Rust control-plane, GraphQL, MCP, workflow compiler, and SQLite are audited as canonical truth.
- Gap / note: No P017 conformance downgrade for missing Swift UI or SwiftData.

### REQ-002 - Compiled graph is authoritative for progression

- Proposal source: lines 84-99, AC-001, AC-016, AC-017.
- Status: Implemented.
- Evidence: code, tests-run.
- References: `control-plane/crates/engine/src/orchestrator.rs` tests for unknown artifacts, candidate classification, no-match conflicts, and legal transition settlement; gate output showed 7 engine orchestrator P017 tests passing.
- Mapping: `TransitionAuthorityResolver`/candidate evaluation fails closed for unknown artifacts and does not honor absent advisory states.
- Gap / note: None found.

### REQ-003 - Agent-authored transition fields are advisory only

- Proposal source: lines 84-89, 372-395, AC-002, AC-016.
- Status: Implemented.
- Evidence: code, tests-run.
- References: `workflow_conflicts::insert_advisory_rejection` inserts advisory records and emits advisory metrics; `proposal_017_orchestrator_records_non_blocking_advisory_rejection` passed in gate.
- Mapping: Legal graph transitions advance while invalid advisory hints persist in advisory rejection history.
- Gap / note: None found.

### REQ-004 - Blocking workflow conflicts persist durable current/history truth

- Proposal source: lines 398-412, 460-492, AC-003 through AC-006.
- Status: Implemented.
- Evidence: migration, code, tests-run.
- References: `control-plane/crates/db/src/repos/workflow_conflicts.rs` lines 27-75, 135-207, 860-890; DB persistence test suite passed 20 tests.
- Mapping: Conflicts upsert by fingerprint, supersede prior current conflicts, persist history, and terminalize with `resolved_at`.
- Gap / note: None found.

### REQ-005 - Non-blocking advisory rejections persist separately

- Proposal source: lines 372-395, AC-002.
- Status: Implemented.
- Evidence: code, tests-run, telemetry.
- References: `workflow_conflicts.rs` lines 78-132; `p017_advisory_rejection_metrics_emit` passed.
- Mapping: Advisory rejections are inserted into `workflow_advisory_rejections`, not current conflicts.
- Gap / note: None found.

### REQ-006 - Candidate transition evaluation is typed and fail-closed

- Proposal source: lines 84-99, AC-017, validation lines 1221-1234.
- Status: Implemented.
- Evidence: code, tests-run.
- References: domain proposal-017 tests passed 6 tests; engine candidate classification and unknown artifact tests passed.
- Mapping: Candidate results classify no-match, multi-match, missing input, invalid expression, aggregate conflict, and unverifiable outcomes.
- Gap / note: None found.

### REQ-007 - Transition cursor/resume stays aligned with conflict truth

- Proposal source: lines 58, 99, 317-324, 847-863, AC-021.
- Status: Implemented.
- Evidence: code, tests-run.
- References: Swift Proposal017Tests passed 16 tests; engine integration startup-repair cursor tests passed 10 proposal-017 tests.
- Mapping: Cursor statuses and resume policies align selected transitions, unresolved conflicts, terminal-unverifiable outcomes, and restart repair.
- Gap / note: None found.

### REQ-008 - MCP, GraphQL, and latest summary expose equivalent workflow_conflict semantics

- Proposal source: lines 561-565, 764-800, AC-006, AC-007, AC-027.
- Status: Implemented.
- Evidence: API/schema code, tests-run.
- References: `control-plane/crates/mcp-server/src/tools/reports.rs` lines 259-460; `control-plane/crates/graphql-server/src/types/run.rs` lines 338-501; MCP P017 tests passed 3; GraphQL P017 tests passed 4.
- Mapping: MCP emits snake_case JSON; GraphQL emits typed fields with surface casing; sanitized lead mediation readback excludes private rationale strings.
- Gap / note: Dirty `schema.rs` P031 changes did not break P017 GraphQL tests.

### REQ-009 - Lead mediation uses owner-aware AgentExecution, not synthetic stages

- Proposal source: lines 414-429, 493-565, AC-008 through AC-010, AC-015.
- Status: Implemented.
- Evidence: migration, code, tests-run.
- References: `p017_mediation_owned_agent_execution_does_not_require_stage_execution`, `p017_mediation_execution_fields_equivalence`, `proposal_017_run_query_exposes_lead_mediation_execution_attempts`, and MCP execution-attempt tests passed.
- Mapping: Mediation-owned executions carry `owner_kind=lead_conflict_mediation`, `owner_id`, `lead_mediation_record_id`, null `stage_execution_id`, and owner-aware readback.
- Gap / note: None found.

### REQ-010 - Mediation attempts preserve runtime facts, watchdog, cost, transcript, artifacts, and validation

- Proposal source: lines 420-428, 561-565, AC-009, validation lines 1228-1236.
- Status: Implemented with reliability risk.
- Evidence: code, migration, tests-run.
- References: migration `032_p017_per_attempt_artifact_linkage.sql` lines 1-21; `artifacts.rs` lines 148-168; MCP readback lines 335-457; GraphQL readback lines 380-498; `p017_per_attempt_cost_and_transcript_persisted`, MCP and GraphQL execution-attempt tests passed.
- Mapping: Attempts include runtime facts, watchdog summary, cost, transcript ref, and artifacts with direct `agent_execution_id` linkage.
- Gap / note: REL-001 records a partial-failure atomicity risk around transcript artifact row insertion.

### REQ-011 - Lead output validation terminalizes malformed/absent/watchdog-expired output

- Proposal source: lines 430-458, AC-011, AC-027.
- Status: Implemented.
- Evidence: code, tests-run.
- References: engine mediation tests and terminal-unverifiable persistence tests passed; GraphQL/MCP sanitized mediation readback tests passed.
- Mapping: Invalid or terminal mediation states expose sanitized terminal failure context and do not export private rationale.
- Gap / note: No live provider watchdog run was executed.

### REQ-012 - Mediation retry, resume, restart, and cancellation are idempotent

- Proposal source: lines 421-428, AC-010, validation lines 1228-1230.
- Status: Implemented.
- Evidence: code, tests-run.
- References: `p017_mediation_cancel_run_cascade`, `p017_mediation_record_lifecycle`, `p017_mediation_execution_fields_equivalence`, startup repair tests passed.
- Mapping: Cancellation cascades from run to mediation-owned execution and mediation record; active mediation lookup avoids duplicate live sessions.
- Gap / note: None found.

### REQ-013 - Stage-scoped readback excludes mediation-owned executions

- Proposal source: lines 523-529, 561-570, primary success metric line 1016.
- Status: Implemented.
- Evidence: code, tests-run.
- References: owner-aware DB integration focused run passed 2 tests; GraphQL/MCP execution-attempt tests passed.
- Mapping: Stage-scoped calls filter stage-owned executions while owner-aware readback includes mediation executions.
- Gap / note: None found.

### REQ-014 - Phase C workflow/catalog lead validation fails typed lead coverage errors

- Proposal source: lines 91-92, AC-012, AC-013, validation line 1231.
- Status: Implemented.
- Evidence: workflow compiler tests-run.
- References: workflow integration tests passed missing lead, duplicate lead, valid lead, and missing resolution-contract cases.
- Mapping: Executable workflow/catalog validation requires exactly one valid system lead with coverage.
- Gap / note: None found.

### REQ-015 - Owner-adjacent retry ledger and artifact claims support mediation-owned executions

- Proposal source: lines 530-558, AC-022, validation line 1236.
- Status: Implemented.
- Evidence: code, tests-run.
- References: focused command `cargo test -p db p017_mediation_owned -- --test-threads=1 --nocapture` passed `p017_mediation_owned_retry_budget_and_artifact_claims_are_owner_keyed` and `p017_mediation_owned_agent_execution_does_not_require_stage_execution`.
- Mapping: Retry budget ledger and artifact source-generation claims are keyed by owner kind/id for lead mediation.
- Gap / note: Runtime metric emission for retry-budget exhaustion is covered under REQ-018, not this storage ownership requirement.

### REQ-016 - Implementation-entry handoff truth is durable and read back northbound

- Proposal source: lines 56, 62, 97, 1210-1211, validation lines 1242-1243.
- Status: Implemented.
- Evidence: code, tests-run.
- References: Swift Proposal017Tests handoff cases passed; engine integration tests `test_proposal_017_code_writer_start_missing_handoff_blocks_before_invoke` and `test_proposal_017_code_writer_start_snapshots_proposal_current_before_invoke` passed.
- Mapping: Handoff status and `code_writer_started=false` readback survive pre-code blocking and proposal snapshotting.
- Gap / note: None found.

### REQ-017 - Sanitized mediation progress is exposed without raw rationale/prompts/transcripts

- Proposal source: lines 769-774, AC-027.
- Status: Implemented.
- Evidence: code, tests-run.
- References: MCP and GraphQL sanitized mediation readback tests passed; readback code references artifacts by path and omits `operator_rationale`.
- Mapping: Northbound readback includes sanitized progress/status and attempt metadata; private rationale and raw prompt/transcript text are not inlined.
- Gap / note: Transcript artifact references exist; raw transcript content remains artifact/debug tier.

### REQ-018 - Operational metrics are runtime-emittable and tested

- Proposal source: lines 1018-1034, AC-019, validation line 1241.
- Status: Partially Implemented.
- Evidence: migration, code, tests-run, tests-found.
- References: `030_p017_workflow_conflict_metric_events.sql` lines 1-23; `031_p017_metric_inventory_and_attempt_attribution.sql` lines 18-47; `workflow_conflicts.rs` metric helpers/callers; `scripts/test-gate.sh` lines 1927-2064; DB metric tests passed.
- Implemented mapping: `advisory_rejection_total`, `invalid_next_stage_hint_non_blocking_total`, `workflow_conflict_current_total`, `terminal_unverifiable_total`, `lead_mediation_attempt_total`, `duplicate_mediation_session_total`, `report_readback_completeness`, `external_catalog_warning_total`, `recovery_action_chosen_total`, `workflow_conflict_time_to_resolution_seconds`, `conflict_reason_to_action_outcome_total`, `phase_c_lead_inventory_external_catalog_total`, and `mediation_late_output_ignored_total` have storage allowance plus helper/caller and/or tests.
- Gap / note: `mediation_retry_budget_exhausted_total` has an allowed metric name and helper but no production caller or focused metric-emission test. `phase_b_dogfood_mediation_completion_rate` and `phase_b_dogfood_operator_guidance_sufficient_total` are present in metric-name migrations and dogfood evidence records, but no runtime metric-event helper/caller was found.

### REQ-019 - Bundled simultaneous-transition scan and known-issues evidence gate Phase A

- Proposal source: lines 54-55, AC-018, AC-025, validation lines 1237-1238.
- Status: Implemented.
- Evidence: tests-run, evidence files.
- References: `proposal_017_bundled_workflows_have_no_static_simultaneous_transition_matches` passed; evidence-gate tests passed.
- Mapping: Static scan and known-issues record checks are part of canonical gate.
- Gap / note: Current untracked `known-issues.md` files are not P017 known-issues migration records.

### REQ-020 - Phase B dogfood/default-on gate remains evidence-backed

- Proposal source: lines 923-953, AC-023, validation line 1240.
- Status: Implemented as evidence gate; metric-event emission partial under REQ-018.
- Evidence: evidence record, tests-run.
- References: `docs/proposals/017-evidence/phase-b-dogfood-exit-record.json`; `p017_phase_b_dogfood_exit_record_has_operator_approved_flag_gated_evidence` passed.
- Mapping: Default-on decision remains blocked by evidence record checks.
- Gap / note: Dogfood metrics are not emitted as `workflow_conflict_metric_events` in current code.

### REQ-021 - Canonical P017 gate exists, is listed, and passes

- Proposal source: line 9, lines 93-95, AC-014, validation lines 1218-1244.
- Status: Implemented.
- Evidence: tests-run.
- References: `./scripts/test-gate.sh proposal-017` passed on 2026-04-28.
- Mapping: Gate executes Swift Proposal017Tests, workflow/domain/db/MCP/GraphQL/engine P017 tests, and source-level closure guards.
- Gap / note: This is the canonical proposal gate, not a full repository regression.

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Operational metrics are not fully runtime-emittable/tested. | High |
| Rust architecture | Pass | None blocking; owner-aware execution and equivalence proof are in place. | High |
| Rust reliability | Pass with risk | Transcript artifact DB row can commit before completion/attribution transaction. | Medium |
| API contract | Pass | Dirty P031 GraphQL diff is unrelated; P017 GraphQL/MCP tests pass. | High |
| Observability / rollout | Partial | `mediation_retry_budget_exhausted_total` and dogfood metric-event paths are incomplete. | High |
| Execution truth | Pass | No current mismatch found in conflict/cursor/owner truth. | High |
| Readiness | Not Ready | Explicit metric gaps prevent full P017 closeout despite green gate. | High |

## Routed Specialist Findings

### OPS-001 - Some committed operational metrics are not runtime-emittable

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items: REQ-018; proposal lines 1018-1034.
- Evidence types: proposal, migration, code, tests-found, tests-run.
- Evidence references: `control-plane/crates/db/migrations/031_p017_metric_inventory_and_attempt_attribution.sql` lines 18-47; `control-plane/crates/db/src/repos/workflow_conflicts.rs` lines 784-812; `rg` found no production caller for `record_mediation_retry_budget_exhausted_tx` and no runtime helper/caller for `phase_b_dogfood_mediation_completion_rate` or `phase_b_dogfood_operator_guidance_sufficient_total`.
- Why it matters: P017 explicitly lists these as operational metrics. Allowing a metric name in the DB or recording dogfood evidence JSON is not equivalent to a runtime emission path that operators can aggregate during rollout.
- Recommended action: Add production emission paths plus focused tests for the missing metric events, or amend the current reference/proposal truth to state which names are evidence-record-only or intentionally deferred behind future provider-backed mediation enforcement.
- Acceptance criteria: `rg` shows callers outside helper definitions; focused tests assert inserted `workflow_conflict_metric_events` rows and labels; `scripts/test-gate.sh proposal-017` guards the caller/test presence for the remaining metrics.

### REL-001 - Transcript artifact row is outside the completion/attribution transaction

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: Medium
- Related proposal items: REQ-010.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/executor.rs` lines 4125-4188; `persist_transcript_artifact_if_present` inserts the artifact row through `artifacts::insert` at lines 5614-5684 before `mediation.complete_with_attribution` starts and commits completion/attribution updates.
- Why it matters: If completion or attribution fails after the transcript artifact row commits, the database can contain a direct attempt-linked transcript artifact without the corresponding completed/attributed attempt row. A retry can then create duplicate transcript artifacts for the same attempt path. The code comment says the artifact row insert is bundled into the transaction, but the implementation does not do that.
- Recommended action: Move transcript artifact row insertion into the same DB transaction as `update_completed_tx` and `update_attempt_attribution_tx`, or add an explicit repair/dedupe path and failure-injection test proving re-drive safety.
- Acceptance criteria: Failure-injection test proves a simulated attribution failure leaves no orphan transcript artifact row, or proves deterministic repair/dedupe on retry.

### READY-001 - Audit tree is dirty with unrelated P031 work

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Minor
- Confidence: High
- Related proposal items: readiness / closeout hygiene.
- Evidence types: diff, tests-run.
- Evidence references: `git status -sb` before report creation showed dirty Swift P031 files, dirty `control-plane/crates/graphql-server/src/schema.rs`, and two untracked known-issues markdown files.
- Why it matters: The P017 gate passed on this dirty tree, but closeout should not accidentally include unrelated P031 UI/API changes or untracked operator notes as part of P017.
- Recommended action: Before P017 closeout/staging, isolate or intentionally include the unrelated P031 work and confirm the generated R6 report is the only audit-written file.
- Acceptance criteria: `git status --short` clean except intended P017 closeout/report changes, or the unrelated P031 work is staged/committed separately with its own rationale.

## Readiness Checklist

| Check | Result | Evidence |
|---|---|---|
| Canonical gate status | Pass | `./scripts/test-gate.sh proposal-017` exited 0 and printed `Proposal 017 gate passed`. |
| Same-tree gate evidence | Pass | Gate ran on HEAD `0fa242f3...` with dirty P031 worktree state recorded above. |
| Core transition flow integration validation | Pass | Engine/domain/workflow P017 tests passed in gate. |
| MCP readback validation | Pass | MCP P017 tests passed: current conflict, sanitized mediation, execution attempts. |
| GraphQL readback validation | Pass | GraphQL P017 tests passed: current conflict, runs summary, sanitized mediation, execution attempts. |
| SQLite persistence validation | Pass | DB P017 workflow conflict persistence test suite passed 20 tests. |
| Owner-adjacent DB validation | Pass | Additional focused `cargo test -p db p017_mediation_owned` passed 2 tests. |
| Swift legacy/readback bridge tests | Pass | Proposal017 Swift suite passed 16 tests; UI implementation remains out of scope. |
| Empty/loading/error/offline/permission UI states | Out of Scope | P017 explicitly excludes UI implementation after UI DB cutover. |
| Accessibility/localization/entitlements | Out of Scope | No UI or entitlements implementation surface is in P017 scope. |
| Privacy/redaction | Pass with code/test evidence | MCP/GraphQL sanitized mediation tests assert private rationale strings are absent. |
| Operational metrics | Partial | Most metric paths are implemented; missing runtime emission paths remain in OPS-001. |
| Full regression suite | Not Run | Canonical P017 proposal gate passed; full repo regression was not run. |
| Live provider/daemon runtime | Not Run | Proposal readiness mode does not require daemon startup or live provider dogfood runs. |

## Verification Log

| Command | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md` | Returned R6 report path. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py ...017...md` | Returned no prior proposal-review artifacts. |
| `git status -sb && git rev-parse HEAD && git merge-base HEAD origin/main` | Recorded dirty tree, HEAD `0fa242f3...`, base `9042077...`. |
| `rg` and focused `nl -ba` reads over proposal, migrations, Rust repos, MCP, GraphQL, engine, and tests | Confirmed proposal contract and code/test mappings. |
| `./scripts/test-gate.sh proposal-017` | Pass. Swift Proposal017Tests passed 16 tests; workflow/domain/db/MCP/GraphQL/engine P017 targeted tests passed; gate printed `Proposal 017 gate passed`. |
| `cd control-plane && CARGO_TARGET_DIR=target/proposal-017-gate CARGO_BUILD_JOBS=1 cargo test -p db p017_mediation_owned -- --test-threads=1 --nocapture` | Pass. 2 owner-adjacent DB tests passed. |
| `rg -n "record_mediation_retry_budget_exhausted|mediation_retry_budget_exhausted_total" ...` | Found helper and gate guard only; no production caller. |
| `rg -n "phase_b_dogfood_mediation_completion_rate|phase_b_dogfood_operator_guidance_sufficient|..." ...` | Found migrations/evidence record/test gate evidence only; no runtime helper/caller. |

## Final Verdict And Recommended Next Actions

Overall conformance: Partial.

Overall implementation readiness: Not Ready for full P017 closeout, despite a passing same-tree canonical P017 gate.

P017's main control-plane behavior is implemented and well covered: graph authority, conflict/advisory persistence, transition cursor/resume, MCP/GraphQL readback, owner-aware mediation executions, stage isolation, lead validation, and canonical gate enforcement all pass focused validation.

The remaining blockers are operational/readiness issues:

1. Complete or explicitly defer the missing runtime metric emission paths in OPS-001.
2. Fix or test the transcript artifact row atomicity/re-drive behavior in REL-001.
3. Isolate the unrelated dirty P031 worktree files before final P017 closeout.
