# Proposal 017 Implementation Audit R5

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md` |
| Audit report | `docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation_IMPLEMENTATION_AUDIT_R5.md` |
| Audit date | 2026-04-27 |
| Repository | `/Users/user/Documents/Chainworks Forge` |
| Branch | `main` (`claude/bold-lichterman` also points at HEAD) |
| Audited HEAD | `2fde7c95fcfde1ea896aa18b457be83c8d221d3b` |
| Compare base | `c750b72140f50925b68e5b6c10b4214648c70f6c` (`merge-base HEAD origin/main`) |
| Implementation target | Current worktree at `main`; P017 control-plane scope only |
| Worktree note | Audit started from a clean worktree. This report is the only intentional write. |
| Canonical gate | `./scripts/test-gate.sh proposal-017` passed on audited HEAD |
| Proposal state | Active for implementation-readiness review |
| Prior reviewer reuse | Not reused; no proposal-review artifacts found. Prior implementation-audit reports were ignored for reviewer selection. |
| Overall conformance | Partial |
| Overall readiness | Not Ready |
| Audit confidence | High for source-level gaps, medium-high overall because no live provider-backed daemon run was executed |

## Implementation Target And Compare Base

The audited implementation is the current `main` worktree at `2fde7c95fcfde1ea896aa18b457be83c8d221d3b`, five commits ahead of `origin/main` merge base `c750b72140f50925b68e5b6c10b4214648c70f6c`.

The latest commit, `P017 R4 closure: per-attempt cost/transcript/artifacts + 5 new metric emissions`, adds migration `031_p017_metric_inventory_and_attempt_attribution.sql`, per-attempt cost/transcript columns, MCP/GraphQL projection updates, additional metric helpers/callers/tests, and stronger `proposal-017` gate checks.

P017's post-UI-DB-cutover amendment makes the Rust control plane the conformance target. SwiftData storage, concrete Swift UI mediation screens, and legacy Swift report generation remain out of P017 conformance scope.

## Prior Review Reuse

The proposal-review discovery helper returned no reusable proposal-review artifacts. Existing `_IMPLEMENTATION_AUDIT_R*` files were not used for reviewer selection, per the skill rule.

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

- The canonical P017 gate passed on the audited HEAD and now checks the R2/R3/R4 closure surfaces.
- Run cancellation cascades to active lead mediation records in the same transaction as agent/work-item cancellation.
- MCP and GraphQL expose conflict-scoped mediation `execution_attempts` arrays.
- Per-attempt cost and transcript columns exist on `agent_executions`, and MCP readback tests assert populated cost and `transcript_ref` when attribution is present.
- Phase C validation pass and fail paths have metric emission helpers/callers.
- Duplicate mediation session, report readback completeness, Phase C external inventory, and late-output ignored metric helpers/tests exist.
- The owner-field deviation from literal `run_id` and `mediation_owner_token` columns is documented and gate-proven by equivalence tests.
- Phase A conflict/advisory/cursor behavior, Phase B mediation lifecycle, Phase C exactly-one-lead validation, dogfood evidence, and external catalog evidence remain gate-covered.

Divergences:

- Mediation attempt output artifacts are still not linked directly to the `AgentExecution` attempt except for transcript artifacts. MCP/GraphQL use direct transcript linkage plus broad `agent_id` correlation for other artifacts, which can over-include artifacts across retries for the same lead agent.
- The executor writes per-attempt attribution after marking the execution completed and treats attribution write failures as warnings. A crash or write failure in that window can leave a completed mediation attempt without cost/transcript attribution.
- Several proposal-named operational metrics are still schema-only or not found as runtime emissions: `advisory_rejection_total`, `invalid_next_stage_hint_non_blocking_total`, `workflow_conflict_current_total`, and `terminal_unverifiable_total`.

Ambiguities / evidence gaps:

- No live provider-backed daemon run was executed during the audit.
- Dogfood evidence is an operator-approved fixture/log artifact, not replayed live during this audit.
- GraphQL queries include the cost/transcript fields, but the GraphQL P017 test does not assert non-null populated values the way the MCP test does.

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

Evidence: `./scripts/test-gate.sh proposal-017` passed, including workflow lint/tests and engine P017 tests for graph-authoritative selection, ambiguous/no-match blocking, advisory rejection, and legal transition resolution.

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

Gap / note: Behavior is implemented; the similarly named metric emissions are covered separately under REQ-013.

### REQ-004: Implementation Handoff Readback

Proposal source: Implementation-entry handoff status and code-writer start blocking commitments.

Status: Implemented.

Evidence: The gate ran implementation handoff tests covering blocked-before-invoke and proposal-current snapshot behavior. MCP handoff readback remains exposed through `implementation_handoff_status_json`.

Implementation mapping: `control-plane/crates/engine/tests/integration.rs` and `control-plane/crates/mcp-server/src/tools/reports.rs`.

Gap / note: No gap found.

### REQ-005: Canonical Gate

Proposal source: P017 gate and acceptance evidence requirements.

Status: Implemented.

Evidence: `./scripts/test-gate.sh proposal-017` passed on `2fde7c95fcfde1ea896aa18b457be83c8d221d3b`. The gate now checks R2 closure, R4 attribution columns, R4 metric helpers/tests, and production-caller presence at `scripts/test-gate.sh:1942`.

Implementation mapping: `scripts/test-gate.sh`.

Gap / note: The gate is strong, but READY-001 records remaining explicit contract gaps not yet covered by gate assertions.

### REQ-006: Phase B Lead Mediation Lifecycle

Proposal source: Phase B lead resolver, mediation record, lead invocation, confirmation, and settlement commitments.

Status: Implemented.

Evidence: Resolver, mediation lifecycle, and confirmation paths are implemented and gate-covered. Mediation-owned `InvokeAgent` work uses null stage ownership; confirmation resolution routes through the mediation settlement service.

Implementation mapping: `control-plane/crates/engine/src/orchestrator.rs`, `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/engine/src/command_handler.rs`, and `p017_mediation_record_lifecycle`.

Gap / note: No lifecycle creation/settlement gap found.

### REQ-007: Mediation-Owned AgentExecution Owner Model

Proposal source: Phase B migration contract around `owner_kind=lead_conflict_mediation`, nullable `stage_execution_id`, `owner_id`, `run_id`, `mediation_owner_token`, and lead mediation linkage.

Status: Implemented by documented equivalence.

Evidence: Migration `029_p017_nullable_mediation_stage_execution.sql` implements nullable stage ownership and owner keys. `docs/proposals/017-evidence/phase-b-mediation-execution-fields-equivalence.md` records the deliberate deviation from literal `run_id` and `mediation_owner_token` columns. The gate requires and runs `p017_mediation_execution_fields_equivalence`.

Implementation mapping: DB migration, domain agent model, owner-aware repositories, equivalence record, and gate-required proof test.

Gap / note: Not a literal schema match to the proposal text at lines 512-515, but current repository truth includes a documented equivalence and executable proof for the behaviors those columns were meant to support.

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

Evidence: MCP and GraphQL mediation readback include sanitized fields and execution attempts. Tests assert `operator_rationale` is not leaked, and attempt numbers reflect durable attempt count when attempts exist.

Implementation mapping: `control-plane/crates/mcp-server/src/tools/reports.rs`, `control-plane/crates/graphql-server/src/types/run.rs`, and P017 MCP/GraphQL tests.

Gap / note: No redaction/readback gap found for the current status surface.

### REQ-011: Mediation Execution-Attempt Readback

Proposal source: P017 MCP `reports.get` contract at lines 561-565, GraphQL owner-aware execution shape, AC-009, AC-015, and AC-027.

Status: Partially Implemented.

Evidence type: code, tests-run.

Evidence references:

- Migration `031_p017_metric_inventory_and_attempt_attribution.sql:67` adds `total_cost_cents`, token counts, and `transcript_artifact_id` to `agent_executions`.
- `control-plane/crates/db/src/repos/agent_executions.rs:630` persists per-attempt attribution.
- `control-plane/crates/engine/src/executor.rs:4144` persists a transcript artifact and `:4170` updates attempt attribution for mediation-owned completions.
- MCP projects cost and transcript refs at `control-plane/crates/mcp-server/src/tools/reports.rs:405`.
- MCP test `proposal_017_workflow_conflict_lead_mediation_execution_attempts` asserts non-null cost/transcript/artifact linkage for an attributed attempt at `control-plane/crates/mcp-server/src/tools/reports.rs:1799`.
- GraphQL maps the same fields at `control-plane/crates/graphql-server/src/types/run.rs:420`.

Implementation mapping: DB migration, AgentExecution domain fields, executor completion path, MCP/GraphQL projections, and P017 tests.

Gap / note: The prior null cost/transcript placeholder is mostly closed. The remaining gap is output artifact linkage: only transcript artifacts are directly linked to an execution attempt. Other attempt artifacts are still included by broad `agent_id` correlation (`mcp-server/src/tools/reports.rs:369`, `graphql-server/src/types/run.rs:403`), which can over-include artifacts across retries for the same lead agent and does not prove the proposal's direct mediation artifact namespace / AgentExecution attempt linkage for LeadResolutionContract output artifacts. Attribution persistence is also best-effort: executor logs and continues if the attribution update fails.

### REQ-012: Cancellation And Resume Invariants

Proposal source: Phase B cancellation/resume contract, including same-transaction cancellation of active agent executions and linked lead mediation records.

Status: Implemented.

Evidence: `control-plane/crates/engine/src/cancellation.rs` calls `lead_conflict_mediations::cancel_active_by_run_tx` in the same transaction as agent execution/work-item cancellation. The gate runs `p017_mediation_cancel_run_cascade` and `p017_mediation_execution_fields_equivalence`.

Implementation mapping: Run cancellation service, lead mediation repository, and gate-required integration tests.

Gap / note: The prior critical cancellation blocker remains closed.

### REQ-013: Rollout Metrics And Evidence

Proposal source: P017 operational metrics at lines 1018-1030, dogfood, known-issues, external catalog inventory, and rollout evidence requirements.

Status: Partially Implemented.

Evidence type: migration, telemetry, tests-run, evidence artifacts.

Evidence references:

- Migration `031_p017_metric_inventory_and_attempt_attribution.sql:22` extends the metric-name CHECK set.
- Helpers/callers/tests exist for Phase C validation pass/fail, lead mediation attempts, duplicate mediation sessions, report readback completeness, external catalog warning, Phase C lead inventory external catalog, and late output ignored.
- `rg` found `advisory_rejection_total`, `invalid_next_stage_hint_non_blocking_total`, `workflow_conflict_current_total`, and `terminal_unverifiable_total` only in metric CHECK migrations, not in helper/caller/test paths.
- The canonical gate passed the current metric tests, but the gate does not check the four schema-only names above.

Implementation mapping: `control-plane/crates/db/src/repos/workflow_conflicts.rs`, `control-plane/crates/engine/src/command_handler.rs`, `control-plane/crates/engine/src/executor.rs`, `control-plane/crates/engine/src/orchestrator.rs`, `control-plane/crates/mcp-server/src/tools/reports.rs`, and P017 persistence tests.

Gap / note: The R4 metric gap is substantially reduced, but the full proposal metric inventory is still not implemented as runtime emissions.

## Reviewer / Lens Scorecard

| Lens | Result | Top risk | Confidence |
|---|---|---|---|
| Proposal conformance | Partial | Output artifact attempt linkage and several proposal-named metric emissions remain incomplete | High |
| Rust architecture | Pass with note | Literal owner fields are replaced by documented/gate-proven equivalence | Medium-high |
| Rust reliability | Partial | Attempt attribution is written best-effort after execution completion | Medium-high |
| API contract | Partial | Attempt shape has cost/transcript, but direct non-transcript artifact linkage remains incomplete | High |
| Observability/rollout | Partial | Four committed metric names remain schema-only | High |
| Execution truth | Partial | Execution attempts are visible, but artifact attribution can still be ambiguous across retries | High |
| Release readiness | Not Ready | Passing gate does not cover the remaining explicit P017 commitments | High |

## Routed Specialist Findings

### API-003 / `api_contract_reviewer`

Severity: Major. Confidence: High.

Related proposal items: REQ-011, AC-009, AC-015, AC-027.

Evidence: MCP and GraphQL directly link `transcript_artifact_id`, but non-transcript artifacts are still selected by `agent_id` correlation in `mcp-server/src/tools/reports.rs:369` and `graphql-server/src/types/run.rs:403`. The proposal requires LeadResolutionContract artifacts and transcripts to be linked to the AgentExecution attempt by artifact refs.

Why it matters: For retries or multiple mediation attempts by the same lead agent, `agent_id` correlation can show the same artifact on multiple attempts or attach an artifact to the wrong attempt. That weakens the conflict-scoped execution truth P017 is trying to establish.

Recommended action: Project direct owner-aware artifact source-generation claims or a direct `agent_execution_id` / mediation-attempt link for LeadResolutionContract output artifacts, not just transcript artifacts.

Acceptance criteria: MCP and GraphQL tests create two attempts by the same lead agent with distinct output artifacts and prove each attempt shows only its own direct artifact refs.

### REL-002 / `rust_reliability_reviewer`

Severity: Major. Confidence: Medium.

Related proposal items: REQ-011, REQ-012, AC-009.

Evidence: The mediation executor marks the agent execution completed, persists transcript attribution, and then opens a separate mediation transaction. Transcript persistence uses `.ok().flatten()` at `control-plane/crates/engine/src/executor.rs:4144`, and attribution update errors are logged but not propagated at `control-plane/crates/engine/src/executor.rs:4170`.

Why it matters: A completed mediation-owned execution can lose cost/transcript attribution if transcript persistence or attribution update fails, while the execution still proceeds to mediation settlement/readback. P017 says attempts preserve runtime facts, transcripts, cost, and output validation.

Recommended action: Move execution completion, attribution persistence, and mediation status/metric updates into one transaction where possible, or make attribution failure visible in runtime facts/readback and gate it.

Acceptance criteria: A failing attribution write either fails the mediation completion path or creates explicit redacted runtime-fact/readback evidence that attribution is missing for a known reason.

### OPS-003 / `observability_rollout_reviewer`

Severity: Major. Confidence: High.

Related proposal items: REQ-013 and P017 operational metrics.

Evidence: `advisory_rejection_total`, `invalid_next_stage_hint_non_blocking_total`, `workflow_conflict_current_total`, and `terminal_unverifiable_total` appear in migration CHECK lists but have no helper/caller/test hits in current source searches.

Why it matters: The rollout metric inventory can still miss advisory rejection volume, invalid hint volume, current conflict cardinality, and terminal-unverifiable totals even though the proposal names them explicitly.

Recommended action: Wire runtime emission and tests for the remaining metric names, or narrow the proposal/reference metric contract with an explicit accepted deferral.

Acceptance criteria: The P017 gate fails when any proposal-named metric lacks a helper, production caller, and representative test.

### READY-001 / Readiness

Severity: Major. Confidence: High.

Related proposal items: REQ-005, REQ-011, REQ-013.

Evidence: `./scripts/test-gate.sh proposal-017` passed, but it does not assert direct non-transcript output artifact refs per attempt, attribution failure handling, or runtime emissions for all proposal-named metrics.

Why it matters: The gate is now much closer, but a passing gate still overstates full P017 completion.

Recommended action: Extend `proposal-017` gate coverage for direct attempt artifact attribution and the remaining metric inventory.

Acceptance criteria: The gate fails before the above issues are fixed and passes after the full P017 contract is met or explicitly narrowed.

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
| API parity/readback | Partial | Cost/transcript readback improved; direct output artifact attempt linkage remains incomplete. |
| Reliability/cancellation | Pass | Mediation records are canceled with run cancellation and idempotency is tested. |
| Redaction/privacy | Pass with residual risk | GraphQL/MCP tests exclude `operator_rationale`; no live transcript export was validated. |
| Rollout/metrics | Partial | More metrics are wired; four explicit metric names remain schema-only. |
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
- `git diff --stat 1f29206b28798be42fb51b4abb74793434b8cef3..HEAD`
- `./scripts/test-gate.sh proposal-017`
- Focused source searches and reads across `control-plane/crates/db`, `control-plane/crates/engine`, `control-plane/crates/graphql-server`, `control-plane/crates/mcp-server`, `control-plane/crates/workflow`, `docs/reference`, `docs/proposals/017-evidence`, and `scripts/test-gate.sh`.

Canonical gate result:

- Swift `Chainworks ForgeTests/Proposal017Tests`: 16 tests passed.
- DB P017 persistence tests: 17 tests passed, including the new attribution and metric tests.
- MCP P017 tests: 3 tests passed, including execution-attempt cost/transcript assertions.
- GraphQL P017 tests: 4 tests passed, including execution-attempt query coverage.
- Engine P017 filters passed, including cancellation cascade, equivalence, and mediation lifecycle tests.
- Warnings observed: existing Rust dead-code warnings in `acp` and `engine`; they did not fail the gate.
- Final gate line: `==> Proposal 017 gate passed`.

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

Reviewer-selection reuse: Not reused.

The current implementation closes more of P017 than R4 did: per-attempt cost/transcript fields now exist, MCP readback proves populated attribution, and several additional metrics are wired. It is still not fully P017-complete because non-transcript output artifacts are not directly linked per mediation attempt, attribution persistence is best-effort after completion, and four explicit operational metrics remain schema-only.

Recommended next actions:

1. Add direct per-attempt artifact links for LeadResolutionContract/output artifacts and prove retries do not cross-attach artifacts.
2. Make mediation attempt attribution atomic or explicitly durable when attribution cannot be written.
3. Wire production helpers/callers/tests for `advisory_rejection_total`, `invalid_next_stage_hint_non_blocking_total`, `workflow_conflict_current_total`, and `terminal_unverifiable_total`, or explicitly narrow the metric contract.
4. Extend the P017 gate to catch the remaining gaps before closeout.
