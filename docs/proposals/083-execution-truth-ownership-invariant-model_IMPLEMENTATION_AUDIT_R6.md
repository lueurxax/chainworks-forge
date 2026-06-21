# P083 Implementation Audit R6: Execution-Truth Ownership and Invariant Model

## Metadata

| Field | Value |
| --- | --- |
| Proposal | P083 - Execution-Truth Ownership and Invariant Model |
| Proposal file | `docs/proposals/083-execution-truth-ownership-invariant-model.md` |
| Proposal revision | `P083-r70-refined-r69-score-lift` |
| Audit report | `docs/proposals/083-execution-truth-ownership-invariant-model_IMPLEMENTATION_AUDIT_R6.md` |
| Audit date | 2026-06-20 |
| Implementation target | Current dirty worktree at `0e6482c82b588b74a76294a225e68286bfe37fa4` |
| Compare base | Current P083 R70 proposal contract in this worktree |
| Prior implementation-audit versions observed | R1 tracked, R2-R5 untracked; not reused as proposal-review inputs |
| Prior proposal-review reuse | None. `discover_prior_review.py docs/proposals/083-execution-truth-ownership-invariant-model.md` returned `artifacts: []` |
| Overall conformance | Not Implemented |
| Overall readiness | Not Ready |
| Audit confidence | High for API/UI/gate blockers; Medium for non-blocking backend coverage because the gate stops before later checks |

## Verdict

P083 is not implementation-complete and is not ready for closeout. The worktree contains substantial P083 implementation: SQLite migrations, command-idempotency policy code, rollback target handling, MCP schemas for the declared eight tools, provider shutdown and identity-hold primitives, Swift UI surfaces, and a proposal gate. However, the implementation fails the canonical P083 gate and independently diverges from mandatory public contracts.

The blocking issues are:

1. GraphQL does not implement the required SDL shape: lifecycle mutations still expose `caller_request_id: String`, no `CallerRequestId` scalar is present, and the promised `DenialPayload` union/failure branch is absent.
2. The macOS identity-hold UI calls a `p083IdentityHoldSessions` GraphQL query that is not implemented by the GraphQL server.
3. `Retry Identity Check` in the UI only re-fetches readback; no backend read-only process identity probe was found.
4. MCP inventory drift exists: the runtime publishes `provider_session.mark_process_absent`, but the proposal's strict MCP inventory and reference schema directory list only the eight R70 tools.
5. The proposal itself is still `Revise-required` and requires a fresh current-revision approval before Ready; no such review artifact was discovered.
6. The canonical P083 gate fails during engine test compilation after the DB migration suite passes.

## Proposal State And Contract

The proposal JSON declares `proposal_revision_id = P083-r70-refined-r69-score-lift` and a status of `Revise-required`; line 7 says implementation may start only after human implementation approval and a fresh aggregate review against this revision returns approve with `blocker_count=0`.

The active contracts audited here include:

- GraphQL SDL lifecycle mutations, `CallerRequestId`, closed enums, and shared denial payloads (`docs/proposals/083-execution-truth-ownership-invariant-model.md:151`).
- MCP tool inventory and strict Draft 2020-12 schemas for the eight declared tools (`docs/proposals/083-execution-truth-ownership-invariant-model.md:195`).
- Manual process identity check UX and backend effects (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1166`).
- Current review refresh gate before Ready (`docs/proposals/083-execution-truth-ownership-invariant-model.md:598`).
- Mandatory hardening requirements, with no non-blocking override unless a successor proposal explicitly owns the reduction (`docs/proposals/083-execution-truth-ownership-invariant-model.md:1312`).

## Platform And Product Scope

| Area | In scope for this audit |
| --- | --- |
| Rust control plane | SQLite migrations, engine command handling, shutdown and cancellation state, MCP server, GraphQL server, rollout gate |
| macOS app | SwiftData/readback boundary, run command menu, toolbar/focused values, manual identity-check banner |
| API contracts | GraphQL SDL shape, MCP tool inventory and schemas, denial vocabulary parity, rollback target propagation |
| Operations/readiness | P083 gate, current-review refresh requirement, rollout evidence corpus |
| Security-sensitive surfaces | Operator mutation authorization, public ingress schemas, process identity redaction, provider process signal lifecycle |

## Reviewer Selection

Selected reviewers, capped at five:

| Reviewer | Why selected |
| --- | --- |
| `api_contract_reviewer` | P083 is contract-heavy and the strongest blockers are GraphQL/MCP parity issues. |
| `rust_arch_reviewer` | Execution-truth ownership spans durable IDs, command handler state, and database truth. |
| `rust_reliability_reviewer` | The proposal centers on idempotency, recovery, shutdown, late output, and crash consistency. |
| `rust_security_reviewer` | The diff touches auth-gated lifecycle mutations, process signaling, redaction, and public ingress schemas. |
| `macos_ui_reviewer` | The proposal includes mandatory native macOS command placement and identity-check UX. |

Rejected alternatives:

| Reviewer | Reason not selected |
| --- | --- |
| `observability_rollout_reviewer` | Displaced by cap; rollout/readiness was still audited as part of reliability and closeout checks. |
| `rust_performance_reviewer` | Displaced by cap; resource-bound issues were treated under reliability/security, and final verdict is already Not Ready. |
| `apple_arch_reviewer` | Displaced by cap; macOS scope is readback/UI rather than deep app architecture in this pass. |

## Flow Audit

| Flow | Expected behavior | Observed implementation | Result |
| --- | --- | --- | --- |
| 1. Lifecycle command idempotency | Every lifecycle mutation/tool carries a non-null caller request ID into durable idempotency rows with canonical intent hashes. | Engine command paths validate request IDs and hash logical fields, but GraphQL SDL exposes strings and lacks the required scalar/union contract. | Partial |
| 2. Rollback execution | GraphQL, MCP, idempotency, audit rows, and readback carry the same non-null `targetEnforcementMode`. | Runtime handler passes target mode into `P083RollbackExecutionCmd`; GraphQL has an enum argument. Public SDL denial/scalar shape still fails the contract. | Partial |
| 3. Provider shutdown and identity-ambiguous hold | Unknown/ambiguous provider process identity holds shutdown and surfaces operator recovery. | Backend command/result types and DB repos exist; the UI readback query is absent from GraphQL, so the operator path is not live end-to-end. | Partial |
| 4. Manual process identity check | Banner appears in three surfaces; retry runs a read-only probe; mark-absent confirms through backend and resumes settlement. | Banner surfaces exist. Retry only calls `refreshCurrentRun()`. Mark-absent is exposed in backend but not in the proposal MCP inventory/reference schema set, and the UI copies guidance rather than executing a backend command. | Partial, blocked |
| 5. Rollout/readiness | Current-revision review, evidence corpus, lint, DB/engine/API/MCP tests, macOS static proof, and gate all pass. | Evidence corpus and DB migration tests pass. The gate fails compiling engine tests before later API/MCP/macOS checks. No current-revision approval artifact was found. | Fails |

## Fidelity And Divergence

Implemented or substantially present:

- Eight P083 migration files are present and the migration integration test suite passed 57 tests.
- Engine/domain code contains P083 request-ID validation, denial-code typing, idempotency TTL policy, failed-terminal retry policy, rollback target handling, provider shutdown intent handling, and mark-process-absent handling.
- MCP reference schemas exist for the eight R70 inventory tools.
- Swift UI includes manual identity-check banner components, command menu/toolbar guidance, copy diagnostics, and run/stage/recovery placements.
- Identity diagnostic copy uses the process-start identity hash, not the raw process identity string.

Material divergence:

- The proposal requires `scalar CallerRequestId`, non-null `callerRequestId: CallerRequestId!`, and `DenialPayload` union branches in GraphQL. The implementation uses Rust parameters such as `caller_request_id: String` and concrete `SimpleObject` payload structs.
- The P083 gate's static GraphQL check currently searches for `caller_request_id: String`, which is weaker than and contradictory to the proposal SDL contract.
- Swift UI declares and calls `p083IdentityHoldSessions`, but the only GraphQL-server occurrence is a comment/type declaration saying it is returned by that query; no resolver was found.
- The primary retry UI action does not initiate a backend identity probe.
- Runtime exposes `provider_session.mark_process_absent`, but the proposal inventory and `docs/reference/mcp/p083` schema directory do not include that tool.
- The hardening section remains only partially evidenced, especially artifact-lineage backfill posture and schema-version evolution policy.

## Coverage Matrix

| Surface | Status | Evidence |
| --- | --- | --- |
| Proposal/current revision | Blocked | Proposal line 7 remains Revise-required; `current_review_refresh_gate_v1` requires fresh approval at lines 598-607. |
| SQLite migrations | Implemented for tested migration layer | `proposal-083` gate passed 57 DB migration tests before later failure. |
| Command idempotency backend | Partial | Domain policies exist at `control-plane/crates/domain/src/commands.rs:717`; gate fails before full engine test suite can prove them. |
| Rollback target runtime path | Partial | `p083_rollback_execution` accepts `target_enforcement_mode` and command passes it through at `control-plane/crates/graphql-server/src/schema.rs:5886`. |
| GraphQL SDL | Missing mandatory shape | Proposal requires scalar/union at lines 155-181; implementation uses `String` args at `schema.rs:5724`, `5820`, `5886`, `5933`, `5980`. |
| MCP inventory | Partial | Eight reference schema pairs exist; runtime also registers `provider_session.mark_process_absent` at `mcp-server/src/tools/runs.rs:541`. |
| Manual identity UI | Partial, not live end-to-end | UI calls `p083IdentityHoldSessions` at `P083IdentityHoldSessionsModel.swift:73`; no GraphQL resolver found. |
| Native macOS commands | Partial | Static code is present, but canonical gate fails before macOS static proof runs. |
| Metrics/observability | Partial | DB migration metric tests pass; full rollout/lint path did not complete because gate failed. |
| Security | Partial | Operator authorization/redaction are present in inspected paths; public API contract drift remains security-sensitive. |

## Requirement Summary

| Status | Count | Meaning |
| --- | ---: | --- |
| Implemented | 2 | Migration-layer evidence or static implementation is enough for this audit pass. |
| Partially Implemented | 11 | Some code exists, but cross-surface contract, gate, or end-to-end proof is incomplete. |
| Missing | 4 | Mandatory behavior or public contract shape was not found. |
| Blocked / Not Verified | 2 | Canonical verification is blocked by the failing gate or missing current-review approval. |

## Detailed Requirement Audit

| ID | Requirement | Status | Evidence and notes |
| --- | --- | --- | --- |
| REQ-001 | Current revision may become Ready only after fresh approval and current-revision corpus attestation. | Missing | Proposal status line 7 and `current_review_refresh_gate_v1` lines 598-607 require this. Prior-review discovery returned no artifacts. |
| REQ-002 | Execution-truth identifiers have durable authoritative records. | Partially Implemented | New tables and provider session state exist, and migration tests passed; full engine/API proof is blocked by compile failure. |
| REQ-003 | Caller-supplied IDs are classified as authority, selector, diagnostic, service-owned, or forbidden. | Partially Implemented | Domain command comments/types reflect caller request ID policy; no complete ownership-matrix executable proof was reached. |
| REQ-004 | Lifecycle commands use durable command-idempotency rows with canonical intent hashes. | Partially Implemented | Backend policy exists, but public GraphQL contract still exposes plain strings and gate fails before engine test completion. |
| REQ-005 | Rollback execution carries normalized non-null target enforcement mode through every layer. | Partially Implemented | Runtime target argument and command field exist at `schema.rs:5886-5908`; GraphQL SDL failure and gate failure prevent full conformance. |
| REQ-006 | GraphQL SDL declares `CallerRequestId!` and lifecycle payload unions with shared `DenialPayload`. | Missing | Proposal lines 155-181 require this. GraphQL code has `caller_request_id: String`; no `DenialPayload`/`DenialReason` GraphQL types were found. |
| REQ-007 | GraphQL/MCP denial vocabulary and enum sets are byte-equal. | Partially Implemented | Domain denial enum exists at `commands.rs:717`; implementation contains additional operational denial codes not in the proposal vocabulary. |
| REQ-008 | MCP tool inventory contains the strict R70 tool set with schemas and `additionalProperties:false`. | Partially Implemented | Eight reference schema pairs exist; runtime publishes an extra `provider_session.mark_process_absent` tool at `runs.rs:541`. |
| REQ-009 | Shutdown command and side-effect contracts are crash-consistent and identity-aware. | Partially Implemented | Shutdown/hold command results exist; full engine tests did not compile. |
| REQ-010 | Provider cancellation intent supports manual identity ambiguity holds and recovery. | Partially Implemented | Backend state and UI components exist; live readback query and retry probe are missing. |
| REQ-011 | Rollout readback API parity is available across GraphQL/MCP/report surfaces. | Partially Implemented | Some readback types exist; GraphQL identity-hold query is missing and gate does not complete. |
| REQ-012 | Reliability deadline overflow and late-output latching are bounded and atomic. | Partially Implemented | Migration tests cover late-output overflow tables; full concurrent writer proof was not reached. |
| REQ-013 | Durable monotonic clock baseline correlation exists for recovery math. | Implemented at migration layer | Migration suite passed monotonic baseline tests; daemon checks were not reached because the engine suite failed first. |
| REQ-014 | SwiftData lifecycle boundary keeps macOS read-only for P083 lifecycle enforcement. | Partially Implemented | UI copies lifecycle/MCP guidance rather than mutating directly; live readback for identity holds is missing. |
| REQ-015 | Metric labels and operational metric domains are bounded. | Implemented at migration layer | Migration suite passed P083 metric-domain tests; rollout lint did not run due gate failure. |
| REQ-016 | Manual process identity check UI has action hierarchy, loading, success, error, and backend effects. | Partially Implemented | Banner exists, but retry only re-fetches and the GraphQL readback query is absent. |
| REQ-017 | Native macOS command validation and deterministic menu/toolbar placement are proved. | Blocked / Not Verified | Code is present, but `proposal-083` failed before the static macOS proof block in `scripts/test-gate.sh:9688-9700`. |
| REQ-018 | Implementation hardening requirements are all closed. | Partially Implemented | `HARDEN-007` and `HARDEN-011` appear implemented; `HARDEN-003` and `HARDEN-004` are not fully evidenced. |
| REQ-019 | Canonical P083 acceptance gate passes. | Missing | `./scripts/test-gate.sh proposal-083` exits 101 compiling engine tests. |

## Scorecard

| Dimension | Score | Rationale |
| --- | ---: | --- |
| API contract fidelity | 1 / 5 | Mandatory GraphQL scalar/union contract is absent; MCP inventory drift exists. |
| Backend reliability | 3 / 5 | Strong migration and command-policy work is visible, but the engine test suite does not compile. |
| Data/migration coverage | 4 / 5 | Migration suite passes 57 tests; artifact-lineage backfill posture remains incomplete. |
| macOS UX conformance | 2 / 5 | UI shell exists, but live query and retry probe are missing. |
| Security posture | 2 / 5 | Auth/redaction patterns are present; public lifecycle contract drift remains security-sensitive. |
| Rollout readiness | 1 / 5 | Proposal gate fails and current-review refresh is absent. |
| Overall | 2.2 / 5 | Not Implemented / Not Ready. |

## Security-Sensitive Scan

The implementation surface triggered security-sensitive review categories: auth, public ingress, parser/schema boundary, filesystem/subprocess boundary, secrets redaction/privacy, DoS/resource limits, and dependency/crypto-adjacent checks.

Findings:

- No raw process-start identity exposure was found in the inspected macOS diagnostic copy path; the UI copies `process_start_identity_hash`.
- GraphQL mutations call authorization helpers before lifecycle commands, but the public SDL still fails the required typed scalar/denial contract.
- MCP schemas use `additionalProperties:false` for the inspected tool schemas, but runtime inventory publishes an additional lifecycle tool not in the proposal inventory/reference schema set.
- No separate credential leak or direct auth bypass was proven in this audit pass. The security-sensitive blockers are routed as API contract/public-ingress findings because client-visible denial and lifecycle schemas are part of the security boundary.

## Routed Findings

| ID | Severity | Owner | Finding | Evidence | Required fix |
| --- | --- | --- | --- | --- | --- |
| API-001 | Blocker | GraphQL/API contract | GraphQL lifecycle SDL does not implement `CallerRequestId` scalar or shared `DenialPayload` union. | Proposal requires scalar/union at lines 155-181. Code uses `caller_request_id: String` in lifecycle mutations at `schema.rs:5724`, `5820`, `5886`, `5933`, `5980`; `types/p083.rs:738-803` uses concrete `SimpleObject` payloads. | Add SDL-visible `CallerRequestId`, success/denial unions, `DenialReason` parity, and executable fixtures that fail on plain `String` fallback. |
| UI-001 | Blocker | macOS UI + GraphQL | Identity-hold UI calls a GraphQL query that does not exist. | `P083IdentityHoldSessionsModel.swift:73-85` calls `p083IdentityHoldSessions`; repo search found only the comment/type at `types/p083.rs:725-728` and no resolver. | Implement the GraphQL query/resolver from authoritative provider-session state, add Swift/live contract tests, and make errors visible instead of clearing silently. |
| UI-002 | Major | macOS UI + backend recovery | `Retry Identity Check` does not run a backend read-only process identity probe. | Proposal effect at lines 1181-1186; UI method only calls `onRetryIdentityCheck()` and sets local text at `ManualProcessIdentityCheckBanner.swift:215-229`; `RunsHomeView.swift:605-607` maps it to `refreshCurrentRun()`. | Add a read-only backend probe command/query and wire retry loading/success/error to typed readback. |
| API-002 | Major | MCP/API contract | MCP runtime inventory is not strictly equal to the proposal inventory. | Proposal lists eight tools at lines 201-209. Runtime registers `provider_session.mark_process_absent` at `mcp-server/src/tools/runs.rs:541`; `docs/reference/mcp/p083` has no schema pair for it. | Either add the tool to an approved proposal revision and reference schemas, or remove/hide it from P083 inventory until covered. |
| GATE-001 | Blocker | Rust/backend reliability | Canonical P083 gate fails before engine/API/MCP/macOS checks complete. | `./scripts/test-gate.sh proposal-083` exits 101; engine tests fail compiling `ExecutionResult` initializers missing `provider_session_store_recovery`. | Fix test fixtures or `ExecutionResult` construction, then rerun the full P083 gate. |
| READY-001 | Blocker | Proposal/readiness owner | Current-revision approval gate is unsatisfied. | Proposal line 7 and `current_review_refresh_gate_v1` lines 598-607 require fresh approval; prior-review discovery returned no artifacts. | Run fresh proposal review against `P083-r70-refined-r69-score-lift` after implementation fixes and attach current-revision-only attestation. |
| HARDEN-001 | Major | Data/reliability | Mandatory hardening remains partially evidenced. | `P083-HARDEN-003` requires backfill posture or executable no-row evidence at lines 1319-1322; migration `087` creates a new bounded `artifact_lineage` table but does not show pre-existing row evidence. `P083-HARDEN-004` requires schema-version policy at lines 1323-1327. | Add executable backfill/no-row proof and schema-version evolution policy with tests/fixtures. |

## Readiness Checklist

| Check | Result |
| --- | --- |
| Proposal state allows Ready | No |
| Prior/current proposal review artifacts reused | No artifacts found |
| Required reviewers selected and routed | Yes |
| Security-sensitive scan considered | Yes |
| Canonical P083 gate passes | No |
| DB migration tests pass | Yes |
| Engine tests pass | No |
| GraphQL SDL contract matches proposal | No |
| MCP inventory matches proposal | No |
| macOS identity-check UI works end-to-end | No |
| UI remote smoke run | Not run; gate failure and UI tests are remote-only by repo policy |
| Residual scope has owner | Yes, findings routed above |

## Verification Log

| Command / check | Result |
| --- | --- |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/083-execution-truth-ownership-invariant-model.md` | Returned no prior proposal-review artifacts. |
| `./scripts/test-gate.sh proposal-083` with local cargo wrapper bypass env from the active session | Failed with exit 101. Evidence corpus verified 112 paths; DB migration suite passed 57 tests; engine tests failed compiling `ExecutionResult` initializers missing `provider_session_store_recovery`. |
| `rg -n "CallerRequestId|DenialPayload|DenialReason" control-plane/crates/graphql-server/src control-plane/crates/domain/src` | Found comments/domain denial code, but no GraphQL SDL scalar or denial union implementation. |
| `rg -n "p083IdentityHoldSessions|identity_hold_sessions|IdentityHoldSessions" control-plane/crates/graphql-server/src control-plane/crates/engine/src control-plane/crates/db/src` | Found only the GraphQL type comment; no query resolver. |
| `rg -n "retry_identity|identity check|identity_check|process identity probe|read-only process identity" control-plane/crates "Chainworks Forge"` | Found UI/comments and backend identity-hold handling; no operator-triggered read-only retry probe. |
| `find docs/reference/mcp/p083 -maxdepth 1 -type f -print | sort` | Found schema pairs for the eight proposal tools; no `provider_session.mark_process_absent` schema pair. |
| `security_sensitive_diff.py --root ... --json` | Triggered security-sensitive review categories; no separate raw-secret leak found, API/public-ingress findings routed above. |
| `implementation_surface_fingerprint.py --root ... --json` | Required lenses included API contract, architecture, reliability, security, macOS UI, rollout, and performance; selected five reviewers due cap. |

## Residual Scope And Follow-Up Ownership

1. GraphQL/API owner: implement the R70 SDL literally, including `CallerRequestId`, `DenialReason`, `DenialPayload`, lifecycle payload unions, and parity fixtures that inspect generated SDL instead of Rust source strings.
2. GraphQL/backend owner: add `p083IdentityHoldSessions(runId:)` resolver from authoritative provider-session state and surface typed errors.
3. Recovery/backend owner: add the read-only identity probe used by retry and keep it signal-free.
4. MCP/API owner: reconcile `provider_session.mark_process_absent` with the strict inventory, either through a revised proposal/reference schema set or by removing it from the visible runtime inventory.
5. Reliability owner: repair engine test compilation and rerun the full P083 gate.
6. Data/reliability owner: close hardening gaps for artifact-lineage backfill posture and schema-version evolution policy.
7. Proposal owner: obtain fresh current-revision approval after implementation changes.

## Final Verdict

P083 remains Not Implemented and Not Ready. The implementation has meaningful backend and UI scaffolding, but mandatory API contracts, live operator readback, retry semantics, hardening proof, current-review approval, and the canonical gate are not closed.
