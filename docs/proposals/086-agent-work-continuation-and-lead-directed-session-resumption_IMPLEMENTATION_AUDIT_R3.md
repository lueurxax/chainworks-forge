# Proposal 086 Implementation Audit R3

Proposal: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md`
Audit target: `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b`
Branch: `cw/implement-proposal-086-agent-w/976f3d1b`
HEAD: `9b79b0667ed9ea0c67659fe4f47e47a60118feab`
Audit timestamp: `2026-05-23 08:29:13 EEST`
Audit report: `docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption_IMPLEMENTATION_AUDIT_R3.md`

## Verdict

Overall Conformance: Partial
Overall Implementation Readiness: Not Ready
Audit Confidence: Medium-High
Reviewer Selection Reuse: Not reused

The implementation contains the main backend shape for P086: a server-owned `agents.continue_work` MCP command, continuation persistence, idempotent admission, live ACP handle reuse, side-effect and approval guardrails, read-only GraphQL readback, continuation evidence files, and focused proposal gates. The work is not ready to merge as complete P086 behavior because several durable contracts remain partial or unproven: runtime evidence artifacts do not match their schemas, recovery does not actually reap orphan ACP OS processes, rollout fixtures are synthetic or stale, lead-auto is only MCP-admitted rather than automatically integrated into lead flow, metrics are mostly absent, and no end-to-end MCP-to-worker-to-live-ACP proof covers the primary user flow.

## Prior Review Reuse

The proposal-review discovery helper returned no review artifacts:

```json
{
  "artifacts": [],
  "proposal_path": "/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b/docs/proposals/086-agent-work-continuation-and-lead-directed-session-resumption.md",
  "repo_root": "/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-implement-proposal-086-agent-w-976f3d1b"
}
```

Reviewer reuse decision: Not reused. Existing implementation-audit reports in the worktree were not used for reviewer selection or as evidence for this audit.

## Reviewer Routing

Selected reviewers:

- `rust_arch_reviewer`: The proposal is primarily Rust control-plane orchestration, MCP, engine, ACP, DB, and recovery work.
- `rust_reliability_reviewer`: Continuation safety depends on idempotency, duplicate-send avoidance, worker lifecycle, cancellation, stale-worker repair, and orphan recovery.
- `api_contract_reviewer`: MCP, GraphQL, JSON schemas, response contracts, and readback artifacts are core acceptance surfaces.
- `observability_rollout_reviewer`: The proposal requires metrics, rollout gates, evidence bundles, fixtures, and operator-readable proof.

Rejected alternatives:

- `macos_ui_reviewer`: No SwiftUI implementation was changed for P086. The UI obligation is represented as an API/readback readiness gap.
- `security_reviewer`: The security-sensitive boundary is the operator-only MCP command and side-effect gates; that surface is covered under API/reliability findings in this audit.
- `performance_reviewer`: The proposal is not primarily latency or throughput driven. Performance-related risk is secondary to correctness and durability.

## Proposal Contract Summary

P086 asks for server-owned continuation of an agent's existing work session, not retry, checkpoint restore, output repair, or human chat. The core contract includes:

- `agents.continue_work` owned by the Rust control plane and available through operator MCP.
- Live ACP handle continuation first, preserving the same provider session and run/stage/agent identity.
- Provider-session resurrection by known provider session id only for adapters that explicitly support it; unsupported adapters must fail closed with no fresh-session fallback.
- Lead-directed automatic continuation based on a structured `lead_continuation_decision_v1` artifact, with server-side validation.
- No SwiftUI Continue button or mutation path. GraphQL readback is allowed and must be read-only.
- Strict eligibility checks: run/stage/agent/session/provider match, role capability, worktree/runtime compatibility, no unresolved side effects, no release/publish/upload/git-push/distribution contexts, count limits, budget limits, and prompt guardrails.
- Durable continuation rows, request fingerprints, idempotency behavior, side-effect ledgers, evidence files, worktree/test/readback data, canonical request/response snapshots, and no-progress classification.
- Recovery that locates, terminates, and records stale/orphan ACP subprocess handling before stale terminal settlement or resurrection.
- Metrics for continuation count, fresh sessions avoided, usefulness/no-progress, test outcomes, changed files, trigger success, budget impact, and orphan/reuse outcomes.

Phase 5 UX timeline work is explicitly out of P086 scope and was split to P093.

## Requirement Audit

| Requirement | Status | Evidence | Notes |
| --- | --- | --- | --- |
| REQ-001: P086 scope boundaries and Phase 5 split | Implemented | Proposal scope excludes Phase 5; implementation focuses Rust backend, MCP, GraphQL, DB, ACP, evidence. | No release/publish/upload lanes were added to continuation scope. |
| REQ-002: Server-owned `agents.continue_work` MCP command | Implemented | `control-plane/crates/mcp-server/src/tools/agents.rs` defines `agents.continue_work`; `handle_continue_work` enforces operator-only, schema rejection, eligibility, admission, and queueing. | Command exists and proposal gate covers many MCP validations. |
| REQ-003: Live ACP handle continuation | Implemented | `run_continuation_worker` calls ACP with `reuse_existing_session: true`, `session_generation_id`, `provider_session_id`, and `keep_session_alive: true`; ACP manager has `has_live_session` and `prompt_session` provider-session checks. | ACP unit tests prove live-handle reuse and dead-session rejection, but the full MCP-to-worker live flow is not end-to-end proven. |
| REQ-004: Provider-session resurrection | Partially Implemented | `handle_continue_work` rejects `provider_session_resurrection` with `provider_session_resurrection_unsupported`; catalog supports a provider resurrection capability flag but example code writer disables it. | Fail-closed unsupported path exists. No adapter-supported resurrection path or attach/resume runtime implementation is present. |
| REQ-005: Lead-directed automatic continuation | Partially Implemented | MCP accepts `trigger_kind=lead_auto`, validates lead artifact id/SHA/schema/target/safety, and `examples/agents/agents.yaml` enables lead-auto for `code_writer`. | No automatic lead orchestration path was found that emits the decision and invokes continuation. Prompt/canonical request handling is still operator-shaped in places. |
| REQ-006: Eligibility and safety validation | Partially Implemented | MCP validates run/stage/session/provider, role `code_writer`, catalog capability, forbidden stage names, unresolved side effects, pending approvals, mode, and trigger. | Worktree/runtime compatibility, count/budget limits, candidates, and policy accounting are incomplete or weakly enforced. |
| REQ-007: Durable persistence, idempotency, side-effect ledger | Implemented | Migration `065_p086_agent_work_continuations.sql`; DB repo handles admission, replay, conflict, active worker saturation, side-effect ledger, worker registration, and evidence ids. | Status names differ from proposal vocabulary but the lifecycle model is functional. |
| REQ-008: Duplicate prompt-send prevention and reconciliation | Partially Implemented | DB claim rejects rows at `prompt_sent` or later; worker reconciles duplicate prompt-sent continuations instead of resending. | Reconciliation classifies success from changed files only, which is too weak for the proposal's evidence model. |
| REQ-009: Evidence artifacts and operator readback | Partially Implemented | Worker writes canonical request, attach receipt, response snapshot, result/no-progress report, evidence bundle, worktree readback, and report artifacts. GraphQL exposes read-only status/candidates. | Several generated artifact payloads do not match schemas; evidence bundle content is thinner than proposal requires. |
| REQ-010: Prompt mode-reset contract | Partially Implemented | Prompt contains a `P086 Continuation Mode Reset` header and states same-session live ACP continuation, not retry/output repair/checkpoint rehydration. | It does not implement the full proposal prompt templates, lead-specific prompt, closeout requirements, or explicit `CHAINWORKS_OUTPUT` anti-pattern guard. |
| REQ-011: Separation from retry/output repair/checkpoint recovery | Implemented | Continuation uses a distinct work item and `reuse_existing_session`; provider resurrection unsupported path does not fall back to fresh retry. | No fresh-session fallback was found in the continuation worker. |
| REQ-012: Daemon restart, orphan recovery, and cancellation | Partially Implemented | Recovery scans stale supervised workers, writes stale-worker evidence, releases workers, and marks `needs_continuation_reconciliation`; cancellation marks active continuations `cancelling`. | Recovery does not locate or signal old OS child/helper processes by pid and deadline. It only asks the current ACP manager to close a session it may not know after restart. |
| REQ-013: GraphQL read-only surface and no mutation | Implemented | `ContinuationRecord`, `continuation_status`, and `continuation_candidates` are query-only; GraphQL mutations remain approvals-only. | SwiftUI app consumption of this readback was not implemented or proven. |
| REQ-014: SwiftUI no Continue control | Implemented | No Swift app continuation mutation/control code was found. | This satisfies the "must not invoke" side but not the operator readback UX expectation. |
| REQ-015: Agent catalog opt-in and fail-closed defaults | Implemented | `ContinuationCapabilityYaml` is parsed; `code_writer` opts in; provider resurrection is disabled/fail-closed in the example catalog. | Need broader fixture coverage for agents without capability fields. |
| REQ-016: Metrics and rollout evidence | Partially Implemented | `continuation_active_count` appears in runtime health/projections; focused P086 gates exist and pass. | Most proposal metrics are missing; fixtures are synthetic/stale and gates do not validate runtime truth. |
| REQ-017: Required test matrix | Partially Implemented | `proposal-086`, MCP, DB, ACP live-session, readback, negative fixture, and operator report gates pass. | No end-to-end continuation flow test, no GraphQL continuation tests, no SwiftUI readback tests, no supported resurrection test, and no real orphan-process reap test were found. |

## Specialist Findings

### API-001: Runtime continuation artifacts do not validate against the checked-in schemas

Severity: Major
Confidence: High

The implementation writes runtime artifact payloads that diverge from the schemas under `docs/reference/p086/schemas`.

Evidence:

- `docs/reference/p086/schemas/artifacts/continuation_response_snapshot_v1.schema.json` requires `payload.response_artifact_id`, but `control-plane/crates/engine/src/executor.rs` builds `response_payload` without that field.
- `docs/reference/p086/schemas/artifacts/continuation_result_v1.schema.json` requires `tests_or_gates` items with `name` and `status`, but the worker writes raw strings from `p086_extract_test_gate_lines`.
- `docs/reference/p086/schemas/artifacts/continuation_no_progress_report_v1.schema.json` has `additionalProperties: false`, but the worker writes `provider_transcript_artifact_ids`.
- `scripts/test-gate.sh` only checks schema-file presence and coarse `additionalProperties` expectations, so the proposal gate does not catch generated-payload incompatibility.

Impact:

Consumers that validate continuation evidence against the committed schemas can reject real runtime artifacts. This weakens the proposal's durable evidence and operator readback guarantees.

Required fix:

Add tests that generate continuation terminal artifacts and validate them against the checked-in JSON schemas. Then align either the schemas or the emitted payloads, with preference for preserving the proposal contract unless there is a documented contract revision.

### API-002: `agents.continue_work` output is narrower than the proposal contract

Severity: Major
Confidence: Medium-High

The proposal specifies an MCP output containing mode, session/provider identity, request and response hashes, canonical request artifact id, attach receipt id, evidence bundle id, worktree readback id, and report id. The implemented accepted response returns only `outcome`, `continuation_id`, `status`, and `request_fingerprint_sha256`.

Evidence:

- MCP accepted response code returns the minimal shape in `control-plane/crates/mcp-server/src/tools/agents.rs`.
- The MCP response schema at `docs/reference/p086/schemas/mcp/agents.continue_work.response.schema.json` also defines the narrower shape.

Impact:

The implementation may be intentionally async, but the public contract changed without a clear proposal amendment. Operators and clients must make extra status calls and cannot rely on the proposal's immediate output fields.

Required fix:

Either restore the proposal output fields when artifacts become available, add an explicit accepted-vs-terminal response contract, or amend the proposal/reference docs so the async shape is canonical and tested.

### REL-001: Primary live continuation flow lacks an end-to-end proof

Severity: Major
Confidence: High

The code path for live ACP handle reuse exists, and the focused gates pass, but the tests are split across layers. There is no observed test that submits `agents.continue_work`, admits a row, runs the continuation worker, proves ACP received `reuse_existing_session=true` with the expected session/provider ids, and validates terminal artifacts.

Evidence:

- `./scripts/test-gate.sh proposal-086` passed, but the engine portion only ran one P086 prompt test.
- ACP integration tests passed for live-session reuse and dead-session rejection at the runtime-manager layer.
- MCP tests passed for request validation and admission behavior.
- No GraphQL continuation tests matched the `continuation` filter, and no full worker live-flow integration test was found.

Impact:

The highest-value behavior in the proposal is the same-session continuation flow. Without an end-to-end test, regressions can pass the proposal gate while breaking the actual operator workflow.

Required fix:

Add an integration test with a fake ACP runtime manager that starts from a live session, calls `agents.continue_work`, drives the worker, and asserts terminal evidence, same provider session id, no fresh generation, and no duplicate prompt send.

### REL-002: Stale-worker recovery does not actually reap orphan ACP OS processes

Severity: Major
Confidence: High

P086 requires daemon restart recovery to locate known child/helper processes from a durable registry, terminate/reap matching orphan ACP subprocesses, and record old pid/session/provider id/signal/deadline/outcome evidence before stale terminal settlement or resurrection. The implementation records stale-worker evidence but only calls `acp.close_session` on the current in-memory ACP manager.

Evidence:

- `control-plane/crates/engine/src/recovery.rs` calls `repair_p086_stale_continuation_workers`.
- That repair path attempts `acp.close_session(&worker.session_generation_id)`.
- After daemon restart, the current ACP manager may not contain the old process/session handle.
- No code path was found that sends a signal to the old worker/helper pid, waits by deadline, or records signal/deadline outcome from actual OS reaping.

Impact:

The recovery evidence can say a stale continuation was observed, but it does not prove orphan ACP processes were terminated. This leaves the proposal's "fail closed after verified orphan handling" contract partial.

Required fix:

Persist enough child/helper process identity to locate old ACP subprocesses after restart, implement signal/wait/deadline reaping, and test both successful and failed reap outcomes.

### REL-003: Reconciliation can mark success from unrelated dirty worktree state

Severity: Major
Confidence: Medium-High

`reconcile_p086_continuation_from_evidence` classifies a prompt-sent continuation as `succeeded` when `git status --short` returns any changed file. It does not verify ACP transcript/tool evidence, ownership, intended outputs, tests, or whether changes belong to the continuation.

Evidence:

- The reconciliation path reads worktree status and treats non-empty changed files as success.
- The proposal requires worktree diff/ownership readback, ACP transcript/tool evidence, generated artifacts, tests/results where available, and final summary.

Impact:

A continuation can be marked successful because of unrelated dirty files already present in the worktree. This weakens duplicate-send recovery and operator trust.

Required fix:

Tie reconciliation to continuation-scoped evidence: prompt/send receipt, transcript or provider terminal state, artifact ids, changed-file ownership, and ideally a before/after status baseline.

### ARCH-001: Lead-auto validation exists, but automatic lead integration is incomplete

Severity: Major
Confidence: Medium

The MCP command validates `lead_auto` artifacts and catalog capability, but no lead closeout/orchestrator path was found that emits `lead_continuation_decision_v1` and automatically invokes continuation. The prompt and canonical request payload also remain operator-shaped in places.

Evidence:

- MCP validates lead decision artifact id, SHA, schema, target, role, session, instruction hash, safety checks, and budget fields.
- `examples/agents/agents.yaml` enables `lead_auto` for `code_writer`.
- The canonical request artifact currently hardcodes `caller_principal_id` as `operator`.
- The prompt builder uses a single operator-style prompt rather than the proposal's separate lead-directed template.

Impact:

Lead-auto can be admitted through MCP if a caller supplies the right artifact, but the proposal's automatic lead-directed continuation behavior is not fully implemented.

Required fix:

Wire the lead result path to produce the decision artifact and request continuation through the server, carry the actual principal/trigger identity into canonical request evidence, and use the lead-specific prompt template and tests.

### OPS-001: Rollout fixtures are synthetic or stale

Severity: Major
Confidence: High

Several evidence fixtures are static examples rather than captured runtime proof, and at least one fixture contradicts the current implementation.

Evidence:

- `docs/evidence/rollout-contract/operator-readback/p086-continuation-full-surface.fixture.json` uses synthetic ids and placeholder hashes rather than captured runtime artifacts.
- `docs/evidence/rollout-contract/p086/negative/lead-decision-missing-or-changed.json` states that `lead_auto` is blocked by a Phase 3 gate and verification is unreachable, while current MCP code validates lead-auto artifacts and does not have that gate.
- The negative fixture gate checks field shape and status but not consistency with live code behavior.

Impact:

The rollout evidence can pass while documenting behavior that no longer matches the implementation. This undermines closeout readiness and regression confidence.

Required fix:

Replace stale/synthetic fixtures with generated evidence from the current implementation, and add fixture assertions that compare expected rejection codes and status transitions against actual MCP behavior.

### OPS-002: Proposal metrics are mostly missing

Severity: Major
Confidence: High

The proposal requires metrics for continuation count, fresh sessions avoided, time saved, usefulness/no-progress, tests passed, changed files, trigger success, follow-up validation, provider/session budget, and orphan/attach outcomes. The implementation exposes only limited runtime-health counting, such as active continuation count.

Evidence:

- Searches found `continuation_active_count` but not the proposed metric set.
- Reference docs still note metrics as pending.

Impact:

Rollout cannot answer whether continuation is useful, safe, avoiding fresh sessions, or producing no-progress churn. That blocks the proposal's operational readiness criteria.

Required fix:

Add the proposed metrics or explicitly revise the metrics contract. Include operator-vs-lead trigger splits and orphan/attach success/failure counters.

### READY-001: SwiftUI readback is not implemented or proven

Severity: Major
Confidence: Medium-High

The proposal allows GraphQL readback but forbids SwiftUI from invoking continuation. The GraphQL read-only surface exists, and no SwiftUI Continue control was found. However, no SwiftUI operator readback for continuation status/history/evidence/change/test/failure/no-progress was implemented or tested.

Evidence:

- GraphQL exposes `continuation_status` and `continuation_candidates`.
- No Swift app files changed for P086 continuation readback, and searches under `Chainworks Forge/` found no `continuationStatus`, `ContinuationRecord`, or `continueWork` UI usage.

Impact:

The backend readback exists, but the operator shell does not yet surface the continuation history promised by the proposal. If this was intentionally deferred, the proposal/reference docs should say so.

Required fix:

Either implement SwiftUI readback with no mutation path, or explicitly narrow P086 acceptance to GraphQL/API readback only and move app UI readback to a follow-up proposal.

## Verification Log

Commands run from the target worktree:

- `./scripts/test-gate.sh proposal-086` - Passed.
  - Phase 0 preflight checks passed.
  - Domain continuation tests: 4 passed.
  - DB `proposal_086_continuation_lifecycle`: 9 passed.
  - Engine P086 prompt test: 1 passed.
  - MCP `tools::agents`: 33 passed.
- `./scripts/test-gate.sh p086-continuation-readback` - Passed.
- `./scripts/test-gate.sh p086-continuation-negative-fixtures` - Passed.
- `./scripts/test-gate.sh p086-continuation-operator-report` - Passed.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p acp --test integration test_runtime_manager_reuses_live_session_handle` - Passed.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p acp --test integration test_runtime_manager_healthcheck_rejects_exited_live_session` - Passed.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p mcp-server tools::agents` - Passed, 33 tests.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p db --test proposal_086_continuation_lifecycle` - Passed, 9 tests.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p engine p086` - Passed, 1 matched test.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p workflow --test proposal_066_toolchain_cache_policy continuation_capability` - 0 tests matched; not counted as coverage.
- `CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p graphql-server continuation` - 0 tests matched; not counted as coverage.

Full `fast` or `full` app gates were not run during this audit. UI tests were not run.

## Readiness Checklist

- Core MCP command present: Yes.
- Live ACP same-session reuse path present: Yes.
- Provider resurrection unsupported fail-closed path present: Yes.
- Supported provider resurrection path present: No.
- Lead-auto artifact validation present: Yes.
- Lead-auto automatic orchestration present: No.
- Durable DB model and idempotent admission present: Yes.
- Duplicate prompt-send guard present: Yes.
- Robust evidence-based reconciliation present: No.
- Runtime artifact schema compatibility proven: No.
- GraphQL read-only status present: Yes.
- SwiftUI readback present: No.
- Continuation metrics present: Partial.
- Orphan ACP OS-process reap implemented: No.
- Focused proposal gates passing: Yes.
- End-to-end primary live continuation test: No.

## Recommended Closeout Path

1. Fix schema/runtime payload mismatches and validate generated artifacts in tests.
2. Add one end-to-end continuation integration test covering MCP admission, live ACP reuse, worker execution, terminal evidence, and no fresh session generation.
3. Implement real orphan process reap evidence or explicitly revise the proposal to remove OS-process reap from P086.
4. Replace stale/synthetic rollout fixtures with generated current-runtime evidence.
5. Complete or defer lead-auto orchestration and SwiftUI readback explicitly in docs.
6. Add the proposal metrics or narrow the metrics acceptance contract before closeout.

