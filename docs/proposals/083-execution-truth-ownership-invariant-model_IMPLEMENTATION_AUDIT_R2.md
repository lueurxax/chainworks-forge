# P083 Implementation Audit R2: Execution-Truth Ownership and Invariant Model

## Audit Metadata

- Proposal: `docs/proposals/083-execution-truth-ownership-invariant-model.md`
- Proposal id: `P083`
- Proposal revision audited: `P083-r70-refined-r69-score-lift`
- Audit timestamp: `2026-06-20 13:21:52 EEST`
- Repository HEAD: `0e6482c82b588b74a76294a225e68286bfe37fa4`
- Report path: `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R2.md`
- Audit mode: read-only implementation audit; this report is the only file created by this pass.
- Worktree state during audit: dirty before this report. The initial snapshot showed modified files in `control-plane/crates/auth/src/lib.rs`, `control-plane/crates/daemon/src/main.rs`, `control-plane/crates/daemon/tests/mcp_stdio.rs`, `control-plane/crates/mcp-server/src/server.rs`, `docs/reference/test-gates.md`, and `scripts/test-gate.sh`; untracked prior audit reports for P079 and P080 were also present. Final verification also showed unrelated modified ACP files. This audit pass intentionally created only this P083 report.

## Final Verdict

- Overall conformance: **Not Implemented**
- Overall readiness: **Not Ready**
- Blocking reason: P083 R70 still declares `implementation_may_start=false` and requires a fresh aggregate approval against this exact revision before implementation may start (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`). The implementation also misses or only partially implements required GraphQL/MCP lifecycle contracts, current-revision evidence, rollout fixtures, migration inventory consistency, and native macOS command/menu proof.
- Gate result: `./scripts/test-gate.sh proposal-083` passed, but it proves only a subset of the R70 contract. It does not prove current-revision reviewer approval, current-revision fixture integrity, the unified `approvalsResolve`/`DenialReason` API contract, runtime MCP unknown-field rejection, or macOS Run menu/toolbar parity.

## Prior Review Reuse

- Prior review discovery: `discover_prior_review.py` returned no current proposal-review artifacts for this proposal.
- Reuse decision: **Not reused**. No prior reviewer selection, scorecard, or finding disposition was treated as authoritative for this audit.
- Stale-material policy applied: older P083 evidence and local artifacts were inspected only as implementation evidence. They were not reused as current R70 reviewer approval.

## Implementation Target

The audit examined the P083 implementation across:

- Rust control plane: SQLite migrations, DB repos, engine command handling, daemon clock/shutdown support, GraphQL schema/types, MCP tools/server dispatch.
- macOS app: SwiftUI app command menus and the manual process identity banner.
- Rollout/evidence: P083 proposal gate, rollout-contract readback fixture, negative fixtures, metrics/readback expectations, and proposal-owned evidence paths.
- Documentation/readback references: implemented-system references where they claimed P083 behavior.

## Proposal Contract Summary

P083 makes durable storage the execution-truth authority for runs, stages, agents, approvals, artifacts, side effects, provider sessions, command idempotency, shutdown receipts, rollout state, and operator readback (`docs/proposals/083-execution-truth-ownership-invariant-model.md:48-66`). It also explicitly requires:

- Non-null `CallerRequestId` on lifecycle mutations and durable idempotency rows with per-command intent hashing (`docs/proposals/083-execution-truth-ownership-invariant-model.md:58-61`).
- End-to-end rollback target parity across GraphQL, MCP, command idempotency, audit rows, and rollout readback (`docs/proposals/083-execution-truth-ownership-invariant-model.md:60-64`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:950`).
- GraphQL lifecycle SDL with `ApprovalResolution { approve reject defer }`, shared `DenialReason`, and `approvalsResolve(...)` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:151-184`).
- MCP JSON Schema Draft 2020-12 schemas with `additionalProperties:false` on every object and typed `schema_invalid` / `additional_properties_rejected` denials before side effects (`docs/proposals/083-execution-truth-ownership-invariant-model.md:195-244`).
- Seven additive SQLite migrations in `migration_plan_v1` and matching release/operator readback (`docs/proposals/083-execution-truth-ownership-invariant-model.md:353-459`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:754-756`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:948`).
- Durable monotonic clock baseline correlation and fixtures (`docs/proposals/083-execution-truth-ownership-invariant-model.md:520-538`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:954`).
- SwiftUI read-only lifecycle truth, a ManualProcessIdentityCheckBanner action hierarchy, and a deterministic macOS Run menu plus toolbar/accessibility parity (`docs/proposals/083-execution-truth-ownership-invariant-model.md:82-90`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:955`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:960`).
- Current-review and current-fixture integrity before Ready (`docs/proposals/083-execution-truth-ownership-invariant-model.md:718-744`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:966-969`).
- Mandatory implementation hardening with no non-blocking/deferred classification unless an approved successor proposal owns the exact reduction (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1298-1335`).

## Platform And Product Scope

- In scope: macOS SwiftUI operator shell, Rust control-plane daemon, SQLite, GraphQL, MCP Streamable HTTP/stdio behavior, durable rollout/readback artifacts, and operator evidence fixtures.
- Out of scope per proposal: new authentication/RBAC/token rotation, workflow YAML/catalog semantics, destructive migrations, native macOS write paths for `side_effects.force_reconcile`, or deriving rollback target implicitly (`docs/proposals/083-execution-truth-ownership-invariant-model.md:68-75`).

## Primary Flows Audited

1. Operator lifecycle commands enter through GraphQL/MCP with caller request ids, operator authorization, command journal records, and durable idempotency rows.
2. `p083.rollback_execution` and `p083.set_enforcement_mode` flow through public API schemas, command idempotency, SQLite audit rows, rollback disposition, and operator readback.
3. Shutdown and provider cancellation flows record durable intent before external effects, classify identity ambiguity, and render manual recovery state.
4. Rollout contract gates migrations, metrics, readback lanes, hold conditions, and negative fixtures before release.
5. macOS SwiftUI reads backend truth and renders disabled/actionable recovery state without becoming the lifecycle authority.

## Specialist Coverage

Selected reviewers:

- `chainworks_execution_truth_reviewer`: repo-local execution truth, persistence, API boundary, command journal, and recovery invariants. This reviewer is mandatory for Chainworks durable execution semantics (`.codex/reviewers/chainworks-execution-truth.yaml:5-11`).
- `rust_reliability_reviewer`: retries, idempotency, deadlines, cancellation, shutdown, worker/recovery behavior (`/Users/user/.codex/skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml:120-135`).
- `api_contract_reviewer`: GraphQL/MCP schemas, request/response parity, migration and compatibility drift (`/Users/user/.codex/skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml:235-250`).
- `observability_rollout_reviewer`: migrations, rollout, rollback, metrics, health/readback evidence (`/Users/user/.codex/skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml:252-267`).
- `rust_security_reviewer`: auth, parsing, public ingress, secret/redaction and deserialization boundaries. This was mandatory because the security-sensitive diff helper triggered (`/Users/user/.codex/skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml:154-168`).

Rejected or unperformed close alternatives:

- `macos_ui_reviewer`: required by the fingerprint for Apple UI/UX, and appropriate for menu, toolbar, keyboard, accessibility, and runtime UI checks (`/Users/user/.codex/skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml:52-67`). Not selected under the five-reviewer cap; missing pass blocks Ready.
- `apple_ux_reviewer`: appropriate for the manual identity recovery journey and feedback states (`/Users/user/.codex/skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml:69-84`). Not selected under the cap; UI/UX readiness is not claimable.
- `apple_arch_reviewer`: displaced by the repo-local execution truth reviewer for cross-stack ownership boundaries (`/Users/user/.codex/skills/proposal-implementation-audit/assets/implementation-reviewer-registry.yaml:86-101`).
- `rust_performance_reviewer`: required by fingerprint output, but not selected under the five-reviewer cap. No benchmark/performance proof was audited, so successful readiness is blocked.

Specialist coverage hard-gate result: **Fail for Ready**. Required helper lenses were `api-contract`, `apple-ui-ux`, `architecture`, `observability-rollout`, `performance`, `reliability`, and `security`; `apple-ui-ux` and `performance` were not covered by selected specialists.

## Security-Sensitive Diff Summary

The security-sensitive diff helper triggered with categories:

- `auth`
- `dos_resource_limits`
- `filesystem_subprocess_boundary`
- `parser_boundary`
- `public_ingress`
- `secrets_redaction_privacy`
- `unsafe_crypto_dependency`

Flagged dirty files were:

- `control-plane/crates/auth/src/lib.rs`
- `control-plane/crates/daemon/src/main.rs`
- `control-plane/crates/daemon/tests/mcp_stdio.rs`
- `control-plane/crates/mcp-server/src/server.rs`
- `docs/proposals/079-contract-aware-output-repair-and-provider-fallback_IMPLEMENTATION_AUDIT_R2.md`
- `docs/proposals/080-continuous-stale-execution-reconciliation_IMPLEMENTATION_AUDIT_R2.md`
- `docs/reference/test-gates.md`
- `scripts/test-gate.sh`

Manual security pass result: no exploitable secret exposure or privilege bypass was proven in this audit, but MCP runtime schema validation has a Major unresolved security/API finding because the public ingress advertises strict schemas while handlers can ignore unknown fields or return generic errors before typed denial.

## Fidelity And Divergence

Implemented or meaningfully present:

- Strict lowercase UUIDv4 caller request id validation exists in the engine and GraphQL path (`control-plane/crates/engine/src/command_handler.rs:173-205`, `control-plane/crates/graphql-server/src/schema.rs:5791-5815`).
- P083 rollback and set-enforcement GraphQL inputs include `target_enforcement_mode` / `target_mode` and `caller_request_id` (`control-plane/crates/graphql-server/src/schema.rs:5791-5883`).
- MCP tool specs for `p083.rollback_execution` and `p083.set_enforcement_mode` declare `additionalProperties:false` and required target/request fields (`control-plane/crates/mcp-server/src/tools/runs.rs:373-452`).
- P083 migrations exist in the tree, including command idempotency, rollback audit target mode, monotonic clock samples, cancellation/process fate, and an extra signal dispatching state migration.
- The focused proposal gate passes and checks DB migrations, engine P083/shutdown tests, daemon/GraphQL/MCP compile checks, rollout-contract lint, rollback disposition validation, monotonic baseline correlation, and R70 rollback/set-enforcement string checks (`scripts/test-gate.sh:9368-9528`).
- A `ManualProcessIdentityCheckBanner` component exists with copy diagnostic, retry identity check, mark process absent, and open evidence actions (`Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift:6-16`, `Chainworks Forge/Views/ManualProcessIdentityCheckBanner.swift:81-126`).

Material divergences:

- The proposal itself remains `Revise-required` and says implementation may start only after human approval plus a fresh aggregate review against R70 (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`).
- Current R70 evidence is missing or stale: the operator readback fixture is an R64 placeholder and many P083 evidence fixtures identify R68; required R70 rollback, clock, UI, macOS, and negative rollout fixture paths are absent (`docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json:1-68`).
- GraphQL still exposes `approveApproval` and `rejectApproval` with `request_id`, not the proposed `approvalsResolve(approvalId, resolution, callerRequestId)` union payload with `ApprovalResolution { approve reject defer }` (`control-plane/crates/graphql-server/src/schema.rs:5541-5674`).
- MCP `approvals.resolve` uses `decision` and `request_id`, accepts `granted/rejected/confirm/manual_fallback`, and omits the R70 `resolution`/`caller_request_id` contract (`control-plane/crates/mcp-server/src/tools/approvals.rs:72-129`, `control-plane/crates/mcp-server/src/tools/approvals.rs:223-259`).
- The canonical `P083LifecycleDenialCode` contains idempotency and internal codes that are not in the proposal shared denial vocabulary, while missing proposal names such as `rollback_target_required`, `rollback_target_invalid`, `schema_invalid`, and `additional_properties_rejected` (`control-plane/crates/domain/src/commands.rs:720-790`).
- The Swift app command menu currently defines `CommandMenu("Navigation")` only; no P083 Run menu lifecycle actions were found (`Chainworks Forge/Chainworks_ForgeApp.swift:48-84`).
- Implementation/gate expects eight physical P083 migrations, while the R70 proposal says seven additive migrations and the acceptance criteria require all seven (`scripts/test-gate.sh:9390-9406`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:754-756`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:948`).

## Requirement Status Summary

| Area | Status | Evidence |
| --- | --- | --- |
| Fresh human implementation approval and R70 aggregate review | Missing | Proposal says `implementation_may_start=false` and requires fresh approval/review (`docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`). |
| Durable execution-truth authority and ownership matrix | Partially Implemented | Persistence and command-handler work exists, but full current-revision proof is absent. |
| CallerRequestId validation | Implemented | Engine and GraphQL validate lowercase UUIDv4 (`control-plane/crates/engine/src/command_handler.rs:173-205`, `control-plane/crates/graphql-server/src/schema.rs:5808-5815`). |
| Command idempotency rows, leases, replay, TTLs | Partially Implemented | DB and handler paths exist and gate passes; all lifecycle-command R70 fixtures are not current. |
| Rollback target parity across GraphQL/MCP/idempotency/audit/readback | Partially Implemented | Main code and gate cover the R70 field, but required same-request/same-intent/mismatch fixtures are missing. |
| GraphQL lifecycle SDL, shared DenialReason, ApprovalResolution | Missing | Current GraphQL approval surface remains `approveApproval`/`rejectApproval`; no `approvalsResolve` or `defer` case. |
| MCP schemas, enum constraints, shared denial vocabulary, additionalProperties runtime denial | Partially Implemented | Specs declare some schemas; runtime enforcement and denial vocabulary parity are incomplete. |
| SQLite migration plan and readback | Partially Implemented | Eight physical migrations exist; proposal requires seven and current readback evidence is stale. |
| RollbackDispositionJSON validation and parity | Partially Implemented | Gate checks validation wiring; operator readback fixture is stale placeholder. |
| Durable monotonic clock baseline correlation | Partially Implemented | DB/gate support exists; named R70 clock baseline fixture is missing. |
| Shutdown/provider cancellation and identity-ambiguous recovery | Partially Implemented | Tables and banner exist; current fixtures are stale or missing. |
| SwiftData lifecycle boundary and pre-P083 store transition | Partially Implemented | Evidence files exist mostly at R68; current copied-store proof not verified. |
| ManualProcessIdentityCheckBanner action hierarchy | Partially Implemented | Component exists; R70 action-hierarchy and feedback-state fixtures are missing. |
| Native macOS Run menu, toolbar parity, accessibility parity | Missing | App commands expose Navigation only; required Run menu fixture paths are missing. |
| Metrics bounded domains and adoption/readback metrics | Partially Implemented | Gate covers required metric names; current rollout metric evidence/readback remains stale. |
| Rollout contract operator readback and negative fixtures | Missing | P083 full-surface readback is R64 placeholder; required R70 negative fixtures are absent. |
| Current security and observability rollout review artifacts | Missing | This audit performed a security pass, but the proposal-required current artifacts against exact R70 are not present in evidence. |
| Mandatory implementation hardening requirements | Partially Implemented | Some code exists; complete proof for all mandatory hardening items is not present. |
| `proposal-083` / `p083` gate | Implemented | Focused gate passed. |

Summary counts: Implemented 2, Partially Implemented 11, Missing 6, Not Verifiable 0, Out of Scope 0.

## Findings

### READY-001 - Critical - Proposal state still blocks implementation readiness

P083 R70 explicitly says implementation may start only after human implementation approval and a fresh aggregate review against this revision returns approve with blocker_count=0. The proposal still records `status: Revise-required` and `implementation_may_start: false` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:7`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:43-46`). No current review artifacts were discovered for this proposal. Under the proposal's own current-review gate, this implementation cannot be marked implemented, closeout-ready, or release-ready.

Required fix: obtain the human implementation approval and fresh R70 aggregate review/corpus attestation, or move implementation under an explicitly approved successor proposal.

### OPS-001 - Critical - Required current-revision evidence and rollout fixtures are stale or missing

The proposal requires every P083 fixture to assert `proposal_id=P083` and the active proposal revision, and treats missing P083 readback or negative fixtures as release holds (`docs/proposals/083-execution-truth-ownership-invariant-model.md:718-744`). The actual operator readback fixture is a placeholder for R64 with `rollout_contract_status: fail`, placeholder hold reasons, stale projection integrity, and "not release evidence" text (`docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json:1-68`). The evidence scan found no `P083-r70-refined-r69-score-lift` evidence under P083 paths, while many existing files are R68. Required paths for rollback same-request/same-intent/mismatch, daemon baseline clock, Run menu, toolbar parity, banner action hierarchy/feedback, and three negative rollout fixtures are absent.

Required fix: regenerate the P083 evidence corpus for R70, replace placeholder readback with concrete run/MCP/release/GraphQL evidence, and add the missing negative/readback/UI fixtures named by the proposal.

### API-001 - Critical - GraphQL/MCP approval lifecycle contract does not match R70

R70 requires a unified GraphQL `approvalsResolve(approvalId, resolution: ApprovalResolution!, callerRequestId: CallerRequestId!)`, `ApprovalResolution { approve reject defer }`, and a shared `DenialReason` union byte-equal to MCP (`docs/proposals/083-execution-truth-ownership-invariant-model.md:151-184`). The implementation still exposes `approve_approval` and `reject_approval` mutations with `request_id`, not `approvalsResolve` with `callerRequestId` and `resolution` (`control-plane/crates/graphql-server/src/schema.rs:5541-5674`). The domain approval enum only supports `Approved` and `Rejected`, parsing `approved/granted` and `rejected`, with no `defer` case (`control-plane/crates/domain/src/commands.rs:397-425`). MCP `approvals.resolve` uses `decision` plus `request_id`, not `resolution` plus `caller_request_id`, and its enum contains `granted`, `rejected`, `confirm`, and `manual_fallback` (`control-plane/crates/mcp-server/src/tools/approvals.rs:72-129`, `control-plane/crates/mcp-server/src/tools/approvals.rs:223-259`).

Required fix: implement the R70 approval lifecycle surface across GraphQL, MCP, domain commands, command idempotency, tests, and denial vocabulary parity fixtures.

### SEC-001 - Major - MCP runtime does not enforce the advertised strict schema contract for P083 tools

The proposal requires `additionalProperties:false` at every object level and unknown fields denied with `additional_properties_rejected` before side effects (`docs/proposals/083-execution-truth-ownership-invariant-model.md:195-244`). Tool specs declare `additionalProperties:false` for P083 rollback/set-enforcement (`control-plane/crates/mcp-server/src/tools/runs.rs:379-452`), but `tools/call` passes raw `arguments` directly into dispatch without generic JSON Schema validation (`control-plane/crates/mcp-server/src/server.rs:888-1010`). The P083 handlers extract only known fields and ignore unknown properties; invalid target values are returned through `anyhow::bail!` instead of typed `rollback_target_invalid` / `schema_invalid` / `additional_properties_rejected` denial payloads (`control-plane/crates/mcp-server/src/tools/runs.rs:1315-1415`). This is a public ingress/parser-boundary mismatch and was covered by the mandatory security pass.

Required fix: validate P083 MCP inputs against the published schemas before dispatch, reject unknown properties with the proposal vocabulary, and add runtime fixtures proving no side effect occurs after schema denial.

### API-002 - Major - Shared denial vocabulary is not byte-equal across proposal, GraphQL, and MCP

The proposal's shared denial vocabulary includes `missing_caller_request_id`, `principal_class_not_allowed`, `lifecycle_state_invalid`, `schema_invalid`, `additional_properties_rejected`, `rollback_target_required`, and `rollback_target_invalid` (`docs/proposals/083-execution-truth-ownership-invariant-model.md:211-234`). The implementation's canonical `P083LifecycleDenialCode` instead includes idempotency-specific and internal codes such as `idempotency_in_flight`, `idempotency_replayed`, `operator_required`, `p083_operator_required`, and `internal`, and omits several proposal names (`control-plane/crates/domain/src/commands.rs:720-790`). MCP output schemas expose that implementation vocabulary for P083 rollback/set-enforcement (`control-plane/crates/mcp-server/src/tools/runs.rs:397-410`, `control-plane/crates/mcp-server/src/tools/runs.rs:437-449`).

Required fix: decide whether the proposal vocabulary or implementation vocabulary is authoritative, then align GraphQL, MCP schemas, domain enum, metrics labels, and fixtures. If the larger implementation vocabulary is intentional, P083 R70 must be revised and approved before closeout.

### OPS-002 - Major - Migration inventory drift creates a contract/readback mismatch

P083 R70 states that the rollout contract owns seven additive SQLite migrations and the acceptance criteria require `migration_plan_v1` to enumerate all seven (`docs/proposals/083-execution-truth-ownership-invariant-model.md:754-756`, `docs/proposals/083-execution-truth-ownership-invariant-model.md:948`). The implementation tree and gate now require eight physical P083 migrations, including `094_p083_008_signal_dispatching_state.sql` (`scripts/test-gate.sh:9390-9406`). The extra migration appears motivated by `SEC-P083-HIGH-001` and adds a `dispatching` signal state before OS signal dispatch (`control-plane/crates/db/migrations/094_p083_008_signal_dispatching_state.sql:1-53`), but it is not represented in the R70 proposal's seven-migration contract.

Required fix: either update and re-approve the proposal/readback contract to own eight migrations, or move the extra migration under a separately approved successor proposal and adjust P083 gate/readback expectations accordingly.

### UI-001 - Major - Native macOS Run menu and toolbar parity contract is missing

P083 requires deterministic `Run` menu placement for Cancel Run, Retry Run, Retry Stage, Resolve Approval, Shutdown Provider Session, and Retry Identity Check, plus toolbar/accessibility parity via focused values (`docs/proposals/083-execution-truth-ownership-invariant-model.md:960`). The app currently registers only a `CommandMenu("Navigation")` with tab navigation actions (`Chainworks Forge/Chainworks_ForgeApp.swift:48-84`). The Swift search found the manual banner but no `Run` command menu, lifecycle menu actions, or focused-value toolbar parity. The named R70 macOS fixture paths for Run menu and toolbar parity are also missing.

Required fix: implement the native Run command menu and toolbar parity contract, then add current R70 runtime/UI evidence or route the UI contract into an approved successor proposal.

### READY-002 - Major - Mandatory specialist lenses are missing for Ready

The implementation surface helper required `api-contract`, `apple-ui-ux`, `architecture`, `observability-rollout`, `performance`, `reliability`, and `security`. This audit selected five reviewers under the skill cap and covered architecture, API, rollout, reliability, and security. It did not perform Apple UI/UX or performance specialist passes. Because P083 has explicit macOS menu/banner/accessibility behavior and broad runtime gate/readback behavior, missing mandatory lens coverage blocks a Ready verdict even aside from the implementation defects.

Required fix: run the missing Apple UI/UX and performance implementation reviews after the API/evidence blockers are addressed, or document an approved scope reduction.

## Reviewer Scorecard

| Reviewer | Result | Notes |
| --- | --- | --- |
| `chainworks_execution_truth_reviewer` | Fail | Durable ownership pieces exist, but the implementation cannot be authoritative while the proposal is not implementation-approved and evidence is stale. |
| `rust_reliability_reviewer` | Conditional fail | Idempotency, shutdown, and clock code paths are present and the focused gate passes; current R70 failure-injection fixtures are incomplete. |
| `api_contract_reviewer` | Fail | Approval lifecycle SDL/MCP contract and denial vocabulary parity are not implemented. |
| `observability_rollout_reviewer` | Fail | Rollout/readback evidence is stale placeholder material and migration inventory drift is unresolved. |
| `rust_security_reviewer` | Fail | No critical exploit proven, but strict MCP runtime validation is missing on public ingress/parser boundary. |
| `macos_ui_reviewer` | Not performed | Mandatory lens omitted under cap; blocks Ready. |
| `rust_performance_reviewer` | Not performed | Mandatory lens omitted under cap; blocks Ready. |

## Verification Performed

- `./scripts/test-gate.sh proposal-083`: **PASS**
- `security_sensitive_diff.py --json`: **triggered**, security pass required and performed.
- `implementation_surface_fingerprint.py --json`: required seven lenses; two unperformed lenses block Ready.
- `discover_prior_review.py docs/proposals/083-execution-truth-ownership-invariant-model.md`: returned no current review artifacts.
- Static evidence checks: proposal line audit, GraphQL/MCP/domain source inspection, Swift command/menu search, P083 fixture/revision scan, migration inventory check.

Verification not performed:

- Full Swift/Xcode gate.
- Remote UI smoke tests.
- Runtime MCP schema-negation test against a live daemon.
- Performance benchmarks.
- Fresh external specialist review artifacts.

## Readiness Checklist

- [x] Focused `proposal-083` gate exists and passed.
- [x] R70 rollback target is present in the main GraphQL/MCP rollback/set-enforcement implementation path.
- [x] Security-sensitive diff was detected and reviewed.
- [ ] Proposal is implementation-approved for R70.
- [ ] Fresh aggregate R70 review with corpus-only-current-revision attestation exists.
- [ ] GraphQL lifecycle SDL implements the proposal's unified approval resolution and DenialReason contract.
- [ ] MCP runtime enforces strict JSON schemas and typed denials before side effects.
- [ ] P083 evidence fixtures assert `P083-r70-refined-r69-score-lift`.
- [ ] Operator readback fixture is concrete release evidence rather than placeholder/stale R64 evidence.
- [ ] Migration contract reconciles seven-vs-eight P083 migration ownership.
- [ ] Native macOS Run menu, toolbar parity, and accessibility parity are implemented and proven.
- [ ] Apple UI/UX and performance specialist passes are complete.

## Residual Scope Before Closeout

1. Resolve the proposal status gate: approve R70 or create an approved successor proposal for any scope reduction.
2. Align the GraphQL and MCP lifecycle API contract, especially `approvalsResolve`, `ApprovalResolution`, `DenialReason`, and `callerRequestId`/`caller_request_id` naming.
3. Add runtime MCP schema enforcement and typed denial fixtures for unknown properties and invalid enum values.
4. Replace stale R64/R68 evidence with current R70 fixtures, including rollback idempotency, clock baseline, UI action hierarchy, macOS Run menu, toolbar parity, and rollout negative cases.
5. Reconcile P083 migration ownership around the eighth signal-dispatching migration.
6. Implement or explicitly descope the native macOS Run menu and toolbar/focused-value contract.
7. Run the missing Apple UI/UX and performance specialist reviews, then rerun the implementation audit.
