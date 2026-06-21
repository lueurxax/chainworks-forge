# Proposal 083 Implementation Audit R7

Proposal: `docs/proposals/083-execution-truth-ownership-invariant-model.md`
Proposal revision: `P083-r70-refined-r69-score-lift`
Audit date: 2026-06-20
Repo HEAD: `0e6482c82b588b74a76294a225e68286bfe37fa4`
Report path: `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R7.md`

## Verdict

Overall conformance: **Not Implemented**

Overall implementation readiness: **Not Ready**

The implementation has meaningful P083 work in place: eight P083 migrations exist, the focused `proposal-083` gate passes, rollback target handling is wired through the Rust command path, and the SwiftUI identity-hold banner exists. The implementation still does not satisfy the live API contract that the proposal makes authoritative. Runtime MCP tool schemas still require `request_id` and legacy fields for most lifecycle tools while the P083 contract and reference schemas require `caller_request_id`. The GraphQL lifecycle surface is also incomplete: required mutations such as `runsCancel`, `stagesRetry`, and `sideEffectsForceReconcile` are absent, and the mark-process-absent mutation exposes `requestId` instead of `callerRequestId`.

The proposal itself also still gates implementation start/readiness on a fresh aggregate approval for this exact revision. I found no current `P083-r70-refined-r69-score-lift` aggregate review artifact with `decision=approve`, `blocker_count=0`, and the required corpus-only-current-revision attestation.

## Scope And Routing

Reviewer-selection reuse: **Not reused**. The proposal embeds prior review basis, but `discover_prior_review.py` found no reusable proposal-review artifacts. Prior `IMPLEMENTATION_AUDIT` files were ignored for reviewer selection per the audit skill.

Selected implementation reviewers:

- `api_contract_reviewer` - GraphQL SDL, MCP tool inventory, JSON schema parity.
- `rust_arch_reviewer` - Rust command/repo/migration authority boundaries.
- `rust_reliability_reviewer` - idempotency, shutdown, restart, late-output recovery.
- `rust_security_reviewer` - security-sensitive diff hard gate for auth/public ingress/tool parsing.
- `macos_ui_reviewer` - manual identity check banner and macOS command placement.

Close alternative displaced by hard cap: `observability_rollout_reviewer`. Rollout and gate artifacts were inspected directly, but a separate rollout specialist pass was not available under the five-reviewer cap. A future Ready verdict should either include that pass or explicitly reroute reviewer coverage.

Security-sensitive diff: triggered (`auth`, `public_ingress`, `parser_boundary`, `filesystem_subprocess_boundary`, `secrets_redaction_privacy`, `dos_resource_limits`, `unsafe_crypto_dependency`). Manual security pass was performed over GraphQL/MCP principal gates and tool parsing. No standalone critical security bug was found beyond the API contract failures below.

## Verification Run

Executed:

```bash
./scripts/test-gate.sh proposal-083
```

Result: **passed**.

Key observed proof:

- Declared evidence corpus verified: 112 paths.
- `db --test proposal_083_migrations`: 57 passed.
- Engine P083/shutdown slices passed.
- `cargo check` for `db`, `daemon`, `graphql-server`, and `mcp-server` passed.
- Domain denial-code round-trip test passed.
- GraphQL approval mutation slice passed.
- MCP P083 rollback/set-enforcement tests passed.
- `scripts/lint-rollout-contract docs/evidence/083/rollout-contract-v1.json` passed.
- Static gate checks reported all eight P083 migrations, rollback-disposition validation, atomic idempotency, monotonic clock source, R70 rollback/set-enforcement contract, and macOS menu/toolbar parity.

Important limitation: the green gate does not prove the full proposal contract. It does not check all required GraphQL lifecycle mutations or the runtime MCP schema fields for the non-rollback lifecycle tools.

## Blocking Findings

### API-001 - Critical - Runtime MCP lifecycle tools do not match the P083 caller_request_id contract

Proposal source: `mcp_tool_inventory_contract_v1` requires `caller_request_id` for `runs.cancel`, `runs.retry`, `stages.retry`, `approvals.resolve`, `side_effects.force_reconcile`, `provider_session.shutdown`, `provider_session.mark_process_absent`, `p083.rollback_execution`, and `p083.set_enforcement_mode` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:195-210`).

Implementation evidence:

- `control-plane/crates/mcp-server/src/tools/runs.rs:82` requires `["run_id", "request_id"]` for `runs.cancel`.
- `control-plane/crates/mcp-server/src/tools/runs.rs:338` requires `["provider_session_id", "reason", "request_id"]` for `provider_session.shutdown`.
- `control-plane/crates/mcp-server/src/tools/runs.rs:464` requires `["effect_id", "request_id", "decision_json"]` for `side_effects.force_reconcile`.
- `control-plane/crates/mcp-server/src/tools/runs.rs:511` requires `["run_id", "request_id"]` for `runs.retry`.
- `control-plane/crates/mcp-server/src/tools/runs.rs:551` requires `["provider_session_id", "cancellation_epoch", "request_id"]` for `provider_session.mark_process_absent`.
- `control-plane/crates/mcp-server/src/tools/approvals.rs:80` requires `["decision", "request_id"]` for `approvals.resolve`, and the handler reads `params["request_id"]` at `control-plane/crates/mcp-server/src/tools/approvals.rs:235`.

Why this blocks readiness:

The live MCP registry is the callable API. The checked-in `docs/reference/mcp/p083/*.schema.json` files use `caller_request_id`, but most runtime tool specs and handlers still use `request_id` and older field names. A caller following the P083 contract or reference schemas will fail against the actual MCP tools, and a caller following the runtime registry will violate the proposal's normalized caller-request-id contract.

Required fix:

Make runtime MCP tool specs and handlers byte-align with `mcp_tool_inventory_contract_v1` for every covered tool, not only rollback/set-enforcement. Add gate assertions that compare the live tool registry to the reference schema files for all nine tools.

### API-002 - Critical - GraphQL lifecycle SDL is incomplete and uses an inconsistent request-id argument

Proposal source: `graphql_sdl_contract_v1` says every P083 lifecycle mutation declares `callerRequestId: CallerRequestId!`, and the lifecycle mutation signature list includes `runsCancel`, `runsRetry`, `stagesRetry`, `approvalsResolve`, `sideEffectsForceReconcile`, `providerSessionShutdown`, `p083RollbackExecution`, and `p083SetEnforcementMode` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:151-181`).

Implementation evidence:

- The implemented mutation routing covers `approveApproval`, `rejectApproval`, `approvalsResolve`, `providerSessionShutdown`, `p083RollbackExecution`, `p083SetEnforcementMode`, `runsRetry`, and `p083MarkProviderSessionProcessAbsent` (`control-plane/crates/graphql-server/src/schema.rs:5235-5247`).
- I found no GraphQL mutation implementations for `runsCancel`, `stagesRetry`, or `sideEffectsForceReconcile`.
- `p083_mark_provider_session_process_absent` takes `request_id: CallerRequestId` (`control-plane/crates/graphql-server/src/schema.rs:6205-6211`), which async-graphql exposes as `requestId`, not `callerRequestId`.
- The gate's GraphQL SDL test checks only `approvalsResolve`, `providerSessionShutdown`, `p083RollbackExecution`, `p083SetEnforcementMode`, and `runsRetry` (`control-plane/crates/graphql-server/src/schema.rs:8142-8148`).

Why this blocks readiness:

The proposal designates GraphQL SDL as the single GraphQL surface authority and requires parity with MCP. The implementation leaves required lifecycle mutations absent and introduces a differently named request-id argument on an added lifecycle mutation.

Required fix:

Implement or explicitly remove/renegotiate the missing GraphQL lifecycle mutations. Every in-scope lifecycle mutation must use `callerRequestId: CallerRequestId!`, and SDL tests/gate checks must assert the exact mutation signatures rather than substring presence.

### UI-001 - Major - Manual Process Absent guidance copies an invalid MCP command

Proposal source: the manual identity-check contract requires the `mark_process_absent` action to require operator confirmation and `CallerRequestId`, and to clear only after backend readback confirms resolution (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1192-1222`). The MCP inventory names the tool `provider_session.mark_process_absent` and requires `provider_session_id`, `cancellation_epoch`, and `caller_request_id` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:208`).

Implementation evidence:

- The banner action is present, but `RunsHomeView` copies `Tool: p083.markProviderSessionProcessAbsent` (`Chainworks Forge/Views/RunsHomeView.swift:252-258`).
- The copied guidance omits both `cancellation_epoch` and `caller_request_id` (`Chainworks Forge/Views/RunsHomeView.swift:252-258`).
- The actual MCP registry names the tool `provider_session.mark_process_absent` (`control-plane/crates/mcp-server/src/tools/runs.rs:541`).

Why this blocks readiness:

The UI surface gives operators an invalid command name and incomplete arguments for the manual recovery action. That makes the proposed manual recovery path unreliable even though the banner itself is visible.

Required fix:

Copy guidance for the actual P083 tool name and all required fields, or route through a backend-approved command UI if the action boundary changes in a later approved proposal. The guidance must use the same request-id field name as the resolved MCP contract.

### READY-001 - Critical - The current proposal revision is not implementation-ready by its own gate

Proposal source:

- The proposal status is still `Revise-required` and says implementation may start only after human approval plus a fresh aggregate review for this revision with `decision=approve` and `blocker_count=0` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:4-7`).
- `active_readiness_narrative.implementation_may_start` is `false` and names the same precondition (`docs/proposals/083-execution-truth-ownership-invariant-model.md:40-44`).
- `current_review_refresh_gate_v1` requires current-revision aggregate approval and corpus-only-current-revision attestation before Ready (`docs/proposals/083-execution-truth-ownership-invariant-model.md:600-607`).

Evidence:

Repository search found current-revision fixtures but no fresh aggregate review artifact for `P083-r70-refined-r69-score-lift` with `decision=approve`, `blocker_count=0`, and the required attestation. The only authoritative review pointer in the proposal is the prior R69 revise-required pass.

Why this blocks readiness:

Even if implementation code were corrected, this proposal cannot be marked implementation-complete or Ready until the proposal's own refresh gate is satisfied.

Required fix:

Produce the fresh aggregate review artifact for `P083-r70-refined-r69-score-lift`, or revise the proposal status and readiness gate through the approved proposal process.

### READY-002 - Major - The P083 gate passes despite missing contract coverage

Proposal source: acceptance criteria require the `proposal-083`/`p083` proof gate to cover the P083 contract suite and fail when hardening requirements lack evidence (`docs/proposals/083-execution-truth-ownership-invariant-model.md:960-985`).

Implementation evidence:

- The gate checks the rollback MCP contract for `target_enforcement_mode` and `caller_request_id` (`scripts/test-gate.sh:9638-9644`), but it does not compare all nine runtime MCP tool schemas to the proposal/reference schema inventory.
- The gate checks `providerSessionShutdown` and `runsRetry` arguments, then tests only for broad `caller_request_id: CallerRequestId` substring presence (`scripts/test-gate.sh:9650-9674`).
- The gate's static macOS checks verify command names and focused-value wiring, but not the copied MCP command payload for manual process absence (`scripts/test-gate.sh:9711-9735`).

Why this blocks readiness:

A green canonical gate is required for Ready, but this gate is currently green while the live GraphQL/MCP/API contract is incomplete. That makes the gate insufficient as release evidence.

Required fix:

Add failing checks for the exact SDL mutation set, exact MCP runtime schema inventory, reference-schema parity, and the mark-process-absent command guidance.

## Requirement Conformance Matrix

| Requirement | Status | Evidence | Notes |
| --- | --- | --- | --- |
| REQ-001 Current review refresh gate | Missing | proposal, repo search | Proposal is still revise-required and no fresh R70 approve/no-blockers aggregate review artifact was found. |
| REQ-002 SQLite execution-truth authority and migrations | Implemented | migration, tests-run | Eight P083 migration files exist and `proposal_083_migrations` passed 57 tests. |
| REQ-003 Command idempotency for lifecycle commands | Partially Implemented | code, tests-run | DB/repo/rollback pieces exist, but MCP runtime inputs for most lifecycle commands still use `request_id`, contradicting the caller-request-id contract. |
| REQ-004 Rollback target end-to-end | Implemented | code, tests-run, gate | Rollback target is wired through GraphQL/MCP command path and p083 rollback audit checks passed. |
| REQ-005 GraphQL SDL lifecycle contract | Missing | code, proposal | Missing required mutations and inconsistent `requestId` on mark-process-absent. |
| REQ-006 MCP tool inventory and schema parity | Missing | code, schema, proposal | Reference schema files are not live-registry parity proof; runtime registry still exposes older field names for most tools. |
| REQ-007 Durable monotonic clock and shutdown evidence | Partially Implemented | migration, code, tests-run | Baseline table, startup insert, and baseline_sample_id plumbing exist; runtime skew/rollback behavior was not live-validated in this audit. |
| REQ-008 Post-cancel late-output overflow contract | Partially Implemented | migration, tests-run | Table and DB tests exist; active projection mutation prevention was not live-validated. |
| REQ-009 Manual identity check UI | Partially Implemented | code, proposal | Banner, picker, copy, and retry states exist; mark-process-absent guidance is invalid/incomplete. |
| REQ-010 Native command placement | Partially Implemented | code, gate | Menus/toolbar/focused values are present, but command guidance does not yet carry valid required command payloads. |
| REQ-011 Rollout contract and proof gate | Partially Implemented | tests-run, gate, proposal | Lint/gate pass, but gate misses live API contract failures. |
| REQ-012 Implementation hardening requirements | Partially Implemented | code, tests-found | P083-HARDEN-007 and P083-HARDEN-011 have code hooks; P083-HARDEN-003 backfill/no-existing-row evidence was not proven beyond an empty fresh migration test. |

## Non-Blocking Observations

- The P083 DB migration suite is strong for fresh-schema behavior and should be retained.
- The rollback target R70 blocker appears materially addressed for `p083.rollback_execution` and `p083.set_enforcement_mode`.
- The SwiftUI identity-hold banner is directionally aligned with the proposal's read-only UI boundary and does not directly mutate lifecycle truth.
- The green gate is still useful regression evidence, but it is not sufficient readiness evidence until API parity checks are expanded.

## Final Readiness Decision

Do not close out P083 yet. The next implementation pass should first align the runtime GraphQL and MCP contracts with the proposal, then update the `proposal-083` gate so it fails on the current mismatches. After that, rerun `./scripts/test-gate.sh proposal-083` and satisfy the proposal's fresh aggregate review gate for `P083-r70-refined-r69-score-lift`.
