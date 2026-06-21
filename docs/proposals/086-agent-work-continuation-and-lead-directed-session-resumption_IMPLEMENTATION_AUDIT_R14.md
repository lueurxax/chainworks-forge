# P086 Implementation Audit R14 - Provider Session Resurrection Completion

## Metadata

| Field | Value |
|---|---|
| Proposal | [086-agent-work-continuation-and-lead-directed-session-resumption.md](086-agent-work-continuation-and-lead-directed-session-resumption.md) |
| Proposal title | Proposal 086: Provider Session Resurrection Completion |
| Audit type | Implementation audit |
| Audit round | R14 |
| Audit timestamp | 2026-06-20T20:25:45Z |
| Repository root | `/Users/user/Documents/Chainworks Forge` |
| Target revision | `0e6482c82b588b74a76294a225e68286bfe37fa4` plus local working-tree changes |
| Proposal status | Draft |
| Prior proposal-review reuse | Not reused; helper search returned no applicable prior review artifacts |
| Overall conformance | Not Implemented for closeout/readiness purposes |
| Overall readiness | Not Ready |
| Audit confidence | High for the blocking findings; medium for complete behavioral coverage because the canonical gate fails before daemon and Swift portions run |

## Target And Compare Base

The audit target is the current working tree for P086. The compare base is the proposal contract itself, especially:

- scope and non-goals in lines 10-12;
- mode architecture in lines 57-90;
- adapter capability contract in lines 92-130;
- durable `resurrection_phase` and replay contract in lines 291-367;
- data/readback/metrics contract in lines 369-438;
- safety rules in lines 440-457;
- required tests and acceptance criteria in lines 459-577.

The tree is dirty and contains unrelated work. This audit only evaluates the P086 surfaces and does not treat prior untracked audit reports as implementation evidence.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `chainworks_execution_truth_reviewer` | Mandatory repo-local reviewer for durable run/stage/agent/ACP/MCP/recovery truth changes |
| `rust_arch_reviewer` | ACP adapter contract, engine worker boundaries, DB state model |
| `rust_reliability_reviewer` | crash/replay, idempotency, side-effect ledger, prompt duplication prevention |
| `api_contract_reviewer` | MCP/GraphQL schemas, readback projections, receipt contracts |
| `rust_security_reviewer` | provider session identifiers, raw receipt access, MCP/GraphQL authorization, subprocess/session boundaries |

## Rejected Reviewers

| Reviewer | Reason |
|---|---|
| `observability_rollout_reviewer` | Relevant to metrics and rollout gates, but the hard cap is five reviewers and readiness is already blocked by compile/security/API findings |
| `macos_ui_reviewer` / `apple_ux_reviewer` | Swift scope is passive readback only; the gate fails before Swift readback tests run |
| `rust_performance_reviewer` | Resource limits and process ownership are relevant, but no performance acceptance target is decisive for this round |
| `product_reviewer` | Product contract is represented directly by the proposal acceptance criteria |

## Proposal State And Contract

P086 asks to complete `provider_session_resurrection`: start a new Chainworks-managed ACP subprocess, attach/resume a known provider session id for adapters that support it, verify requested-vs-actual session identity before any prompt, and preserve fail-closed behavior for unsupported or unsafe cases. It explicitly excludes reimplementing shipped live-handle continuation and P093 soak/scale evidence.

The implementation has meaningful pieces in place: a versioned ACP capability, a Claude adapter declaration, durable DB phase columns, an engine resurrection path that writes a v2 attach receipt before prompt send, output-only receipt fields, MCP admission gates, and targeted tests. It is not ready because required code does not compile under the canonical gate, required phase/readback fields are not consistently exposed, and attach receipt readback leaks cross-run existence/projection data for non-operator principals.

## Platform And Product Scope

Primary implementation scope is the Rust control-plane daemon: ACP adapters, engine worker execution, SQLite migrations/repos, MCP tools, GraphQL readback, and daemon tests. Swift/macOS scope is read-only continuation presentation through `P031ThinGraphQLReadBoundary` and a passive Runs UI card. There is no new Swift mutation surface in scope.

## Primary Flows Audited

1. Operator or lead submits `agents.continue_work` with `continuation_mode = provider_session_resurrection` or `output_only_recovery`.
2. MCP admission validates target run/stage/agent/session/provider, frozen catalog opt-in, triggers, side-effect/pending-approval safety, and adapter capability.
3. Engine continuation worker claims the row, attaches a new managed ACP process to the recorded provider session id, verifies actual identity, persists a v2 attach receipt, then sends the mode-reset prompt once.
4. Crash/replay state is represented through `agent_work_continuations.status`, `resurrection_phase`, side-effect ledger rows, supervised worker rows, and receipt updates.
5. MCP, GraphQL, reports, and Swift readback expose mode, phase, attach receipt, metrics, and results without requiring operators to inspect raw JSON.

## Fidelity And Divergence

### Matches

- The ACP boundary defines `ProviderSessionResurrectionCapability` with provider family, adapter id, capability version, attach support, request/result shapes, identity proof support/source, write safety, and failure classes in `control-plane/crates/acp/src/adapters/mod.rs:30-79`.
- Claude declares support through `claude-agent-acp`, `resumeSessionId`, `session/new.result.sessionId`, and `provider_session_resurrection_v1` in `control-plane/crates/acp/src/adapters/claude.rs:57-70`, and injects `resumeSessionId` into the session/new spec at `claude.rs:103-136`.
- DB migrations add `resurrection_phase`, deadline/heartbeat/timeout fields, checks, and idempotency indexes in `control-plane/crates/db/migrations/079_p086_resurrection_state_and_idempotency.sql:7-133` and the tightened invariant in `083_p086_deadline_invariant.sql:81-119`.
- Admission inserts `resurrection_phase = 'admitted'` and a deadline only for `provider_session_resurrection` rows in `agent_work_continuations.rs:1096-1112`.
- The engine attach path writes a pre-prompt v2 raw receipt, persists a redacted artifact, and only then inserts the provider-send ledger row and moves to `prompting` in `control-plane/crates/engine/src/executor.rs:7736-8118`.
- Prompt-turn correlation rejects terminal responses missing marker/fingerprint/stage/agent proof in `executor.rs:8234-8286`.
- The v2 receipt schema requires requested/actual provider session id, identity proof, process evidence, prompt marker, `resurrection_phase`, output-only flags, session-store recovery, deadlines, and typed failures in `docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v2.schema.json:8-174`.

### Divergences

- The canonical gate fails compiling `graphql-server`, so acceptance criterion 14 is missing on this tree.
- `resurrection_phase` exists in the DB and attach receipt but is not part of the primary `ContinuationRecord` domain projection, the repo `SELECT_COLS`, the MCP `redacted_record`, the MCP continuation status schema, or `GqlContinuationRecord`. This violates the required MCP/GraphQL/report readback contract.
- MCP and GraphQL attach-receipt readback enforce wrong-run/no-oracle behavior for operators, but observer and agent projections fetch by `continuation_id` without first proving `run_id` matches the continuation.
- GraphQL display text handles `live_handle_continuation` and `provider_session_resurrection`, but `output_only_recovery` renders as `UNKNOWN(output_only_recovery)`.
- Metrics contain generic continuation and attach counters, but the summary does not yet expose every proposal-required counter as distinct readback, especially resurrection requested, prompt sent after resurrection, no-progress after resurrection, and useful-progress after resurrection.

## Residual Follow-up Ownership

P086 still owns the compile fix, attach-receipt access matrix, typed readback parity, metrics parity, and canonical gate pass. P093 remains limited to soak/scale evidence and should not absorb the provider-session resurrection implementation gaps.

## Specialist Coverage Matrix

| Surface | Reviewer coverage | Status |
|---|---|---|
| ACP adapter capability and Claude attach request | Rust architecture, execution truth | Mostly implemented |
| MCP admission and frozen catalog gates | API contract, execution truth, security | Partially implemented |
| Engine attach/prompt/receipt worker path | Rust reliability, execution truth, security | Partially implemented |
| DB phase/replay state | Rust reliability, API contract | Partially implemented |
| MCP/GraphQL/report readback | API contract, security | Blocked |
| Raw receipt/session identifier security | Security | Blocked |
| Swift read-only presentation | API contract | Not fully verified because gate failed before Swift tests |

## Requirement Summary

| REQ | Proposal area | Status | Notes |
|---|---|---|---|
| REQ-01 | Explicit mode architecture and no silent fallback | Partially Implemented | P086 modes exist, but `normal_fresh_execution`/`normal_live_reuse` are not surfaced as a full selected/rejected mode taxonomy across execution/report surfaces |
| REQ-02 | ACP adapter capability contract | Implemented | Contract and Claude declaration are present |
| REQ-03 | Frozen catalog gate and fail-closed admission | Mostly Implemented | MCP gate paths exist; full proof blocked by failing canonical gate |
| REQ-04 | Claude provider-session resurrection | Partially Implemented | Attach path and tests exist; gate does not reach daemon integration tests |
| REQ-05 | Safety checks for target, side effects, approvals, forbidden lanes | Partially Implemented | Many checks exist; full worktree/model/projection proof is not complete |
| REQ-06 | Pre-prompt receipt, prompt marker, and terminal correlation | Partially Implemented | Engine evidence exists; full same-tree integration proof blocked by gate |
| REQ-07 | Output-only recovery with no-source-change proof | Partially Implemented | Receipt fields and source edit classification exist; full GraphQL/daemon/Swift proof blocked |
| REQ-08 | Durable replay with typed `resurrection_phase` | Partially Implemented | DB phase exists, but typed/readback parity is incomplete |
| REQ-09 | MCP/GraphQL/report readback of phase and receipt fields | Partially Implemented | Attach-receipt endpoints exist; primary continuation readback lacks phase and GraphQL does not compile |
| REQ-10 | Metrics/readback counters | Partially Implemented | Some counters exist; proposal-required resurrection-specific counters are incomplete |
| REQ-11 | Security and privacy of receipt/session data | Partially Implemented | Raw/redacted projections exist, but cross-run no-oracle failure is a blocker |
| REQ-12 | Required tests and canonical gate | Missing | `./scripts/test-gate.sh proposal-086` fails at `graphql-server` compile |

## Detailed Requirements

### REQ-01 - Mode Architecture

`ContinuationMode` contains `live_handle_continuation`, `provider_session_resurrection`, and `output_only_recovery` in `control-plane/crates/domain/src/continuation.rs:3-34`, and MCP validates those modes in `control-plane/crates/mcp-server/src/tools/agents.rs:1974-1985`. The proposal also requires classifying ordinary attempts as `normal_fresh_execution` and `normal_live_reuse`, plus surfacing selected and rejected alternatives in execution/report output. I did not find a complete, externally visible taxonomy for those ordinary modes.

### REQ-02 - Adapter Capability

Implemented. The adapter contract and failure classes are present in `adapters/mod.rs:30-79`, and Claude declares a supported resume capability in `claude.rs:57-70`. Unsupported providers return `None` from `provider_session_resurrection_capability_for_provider` and should continue to fail closed.

### REQ-03 - Frozen Catalog Gate

Mostly implemented. The MCP admission path has catalog and capability gating, side-effect checks, pending-approval checks, forbidden stage checks, and lead-auto constraints. The canonical gate preflight also statically checks for `continuation_capability_rejection`, `forbidden_stage_kind`, unresolved side-effect guards, lead-auto validations, and mode fields in `scripts/test-gate.sh:10091-10106`. Full acceptance remains blocked because the gate fails later.

### REQ-04 - Claude Resurrection Attach

Partially implemented. The engine calls `attach_provider_session_for_resurrection` with a new `p086-resurrection-*` session generation and the recorded provider session id in `executor.rs:7636-7682`. The receipt records requested and actual ids at `executor.rs:7774-7799`. The gate found and ran the ACP unit tests for Claude capability/session-store, but did not reach the daemon integration tests because `graphql-server` fails first.

### REQ-05 - Safety Checks

Partially implemented. MCP blocks many unsafe targets and supports output-only over resurrection when allowed by catalog (`tools/agents.rs:1363-1433`, `1805-2485`). The proposal also requires target worktree root and provider/model family matching. Some of that truth is gathered in DB context, but complete end-to-end verification is not available from the failed gate.

### REQ-06 - Receipt Before Prompt And Correlation

Partially implemented. The engine writes raw and redacted v2 attach receipts before the provider-send ledger and prompt send (`executor.rs:7922-8118`). It then rejects completed results that lack correlation terms (`executor.rs:8234-8286`). This is strong code evidence, but readiness depends on the failing daemon tests listed in `scripts/test-gate.sh:10108-10119`.

### REQ-07 - Output-only Recovery

Partially implemented. The engine allows output-only recovery to attach through provider-session resurrection when no live session exists (`executor.rs:7488-7508`) and records output-only/source-edit fields in the receipt (`executor.rs:7859-7870`, `8437-8457`). It also classifies source edits for output-only rows (`executor.rs:6283-6293`, `6960`). Full proof is blocked by the failed gate, and GraphQL display/readback is incomplete for `output_only_recovery`.

### REQ-08 - Durable Replay State

Partially implemented. The DB stores phases and deadlines, and the claim path refuses to rewind prompt-sent rows (`agent_work_continuations.rs:1157-1249`). However, `update_resurrection_phase` only updates rows where `mode = 'provider_session_resurrection'` (`agent_work_continuations.rs:1290-1319`), while output-only recovery can use the resurrection attach path. More importantly, the typed phase is not exposed in the primary continuation readback.

### REQ-09 - Readback/API Contract

Partially implemented and blocking. `ContinuationRecord` lacks `resurrection_phase` (`domain/src/continuation.rs:148-179`), `SELECT_COLS` omits it (`agent_work_continuations.rs:102-119`), MCP `redacted_record` omits it (`tools/agents.rs:302-328`), the MCP status schema omits it (`agents.continuation_status.response.schema.json:41-79`), and `GqlContinuationRecord` omits it (`graphql-server/src/types/continuation.rs:57-143`). That conflicts with proposal lines 310-313, 423-424, and tests 18-21.

### REQ-10 - Metrics

Partially implemented. `P086ContinuationMetricsSummary` includes generic totals, fresh-session avoided, orphan reap totals, and attach success/failure (`agent_work_continuations.rs:60-100`). It does not clearly expose all proposal-required counters as distinct summary fields: resurrection requested, prompt sent after resurrection, no-progress after resurrection, useful-progress after resurrection, and fresh retry avoided.

### REQ-11 - Security/Privacy

Partially implemented and blocking. Operator raw access checks the requested run before returning a receipt in MCP and GraphQL. Observer and agent projections do not perform the same run match before returning redacted/minimal data, creating a cross-run existence/projection leak for session-related continuation ids.

### REQ-12 - Tests And Gate

Missing. The canonical gate runs static preflight, Rust unit tests, GraphQL tests, daemon integration tests, and Swift readback tests (`scripts/test-gate.sh:10254-10268`). In this audit run, the gate failed at the GraphQL test compile step before daemon and Swift proof could run.

## Reviewer Scorecard

| Reviewer | Score | Rationale |
|---|---:|---|
| `chainworks_execution_truth_reviewer` | 2/5 | Durable rows, receipts, and worker phases exist, but same-tree gate and readback truth are incomplete |
| `rust_arch_reviewer` | 3/5 | Adapter and worker structure are credible; ordinary mode taxonomy and phase API boundaries remain inconsistent |
| `rust_reliability_reviewer` | 2/5 | Idempotency and replay mechanisms exist, but daemon crash/replay tests did not run and phase readback is incomplete |
| `api_contract_reviewer` | 2/5 | Schemas/endpoints exist, but GraphQL does not compile and primary readback omits `resurrection_phase` |
| `rust_security_reviewer` | 1/5 | Raw receipt redaction exists, but non-operator attach-receipt access leaks cross-run existence/projection data |

## Security-sensitive Diff Summary

The security-sensitive scan is triggered by MCP/GraphQL ingress, provider session identifiers, raw receipt storage, subprocess/session attach behavior, redaction, and auth boundaries. The most important security finding is SEC-001 below. Until it is fixed and regression-tested, P086 is not safe to mark Ready or close out.

## Routed Findings

### READY-001 - Canonical P086 gate fails compiling GraphQL

- Severity: Critical
- Owner: GraphQL/API owner
- Evidence: `./scripts/test-gate.sh proposal-086` failed during `cargo test -p graphql-server --test proposal_086_continuation_readback`.
- Error summary:
  - `crates/graphql-server/src/schema.rs:5931:46`: cannot find value `pool` in this scope
  - `crates/graphql-server/src/schema.rs:5955:53`: cannot find value `pool` in this scope
  - `crates/graphql-server/src/schema.rs:5968:57`: cannot find value `pool` in this scope
  - `crates/graphql-server/src/schema.rs:6292:64`: `async_graphql::ID` does not implement `std::fmt::Display`
- Impact: Acceptance criterion 14 is missing, and daemon/Swift portions of the gate did not run.
- Required fix: repair GraphQL compile errors, then rerun `./scripts/test-gate.sh proposal-086` on the same tree.

### SEC-001 - Non-operator attach-receipt readback leaks cross-run existence/projection data

- Severity: Major
- Owner: MCP/GraphQL security owner
- Evidence:
  - MCP agent branch reads context by `continuation_id` and returns `attach_receipt_artifact_present` plus `resurrection_phase` without checking `requested_run_id` against actual run at `control-plane/crates/mcp-server/src/tools/agents.rs:520-557`.
  - MCP observer branch fetches raw receipt by `continuation_id` and returns a reviewer projection without a run match at `tools/agents.rs:658-697`.
  - GraphQL observer branch fetches raw receipt by `continuation_id` and returns redacted data without a run match at `control-plane/crates/graphql-server/src/schema.rs:3359-3390`.
  - GraphQL agent branch selects `resurrection_phase` by `continuation_id` without a run match at `schema.rs:3392-3410`.
  - Operator branches do perform run matching before raw access (`tools/agents.rs:560-593`, `schema.rs:3295-3323`), which makes the non-operator gap clear.
- Impact: A lower-privilege principal with a continuation id can learn whether a receipt exists and, for observers, receive redacted receipt metadata even when supplying the wrong run id. This violates the no-existence-oracle intent for session-related receipt access.
- Required fix: apply the same run-scope/no-oracle check to every principal class before returning any projection, and add negative tests for wrong-run agent/observer MCP and GraphQL requests.

### API-001 - `resurrection_phase` is not exposed through primary continuation readback

- Severity: Major
- Owner: API contract owner
- Evidence:
  - Domain `ContinuationRecord` lacks `resurrection_phase` at `control-plane/crates/domain/src/continuation.rs:148-179`.
  - DB repo `SELECT_COLS` omits `resurrection_phase` at `control-plane/crates/db/src/repos/agent_work_continuations.rs:102-119`.
  - MCP `redacted_record` omits `resurrection_phase` at `control-plane/crates/mcp-server/src/tools/agents.rs:302-328`.
  - MCP continuation status schema omits `resurrection_phase` at `docs/reference/p086/schemas/mcp/agents.continuation_status.response.schema.json:41-79`.
  - GraphQL `GqlContinuationRecord` omits `resurrection_phase` at `control-plane/crates/graphql-server/src/types/continuation.rs:57-143`.
- Impact: Operators cannot distinguish `attached_unprompted` from `prompting`/`settling`/terminal phases through primary MCP/GraphQL/report continuation readback, which is explicitly required by proposal lines 310-313, 423-424, and tests 18-21.
- Required fix: promote `resurrection_phase` and relevant attach-result fields into the primary continuation projections and schemas, not only the raw/special attach receipt endpoint.

### API-002 - `output_only_recovery` displays as unknown in GraphQL continuation records

- Severity: Medium
- Owner: GraphQL/API owner
- Evidence: `display_mode` handles only `live_handle_continuation` and `provider_session_resurrection`; all other modes become `UNKNOWN(<raw>)` at `control-plane/crates/graphql-server/src/types/continuation.rs:37-42`.
- Impact: A proposal-supported mode appears as unknown in GraphQL-backed readback.
- Required fix: add an explicit `output_only_recovery` display case and include it in readback tests.

### OBS-001 - Proposal-required resurrection metrics are incomplete as summary fields

- Severity: Medium
- Owner: Observability/API owner
- Evidence: `P086ContinuationMetricsSummary` has attach success/failure and several generic continuation counters at `control-plane/crates/db/src/repos/agent_work_continuations.rs:60-100`, but not all fields required by proposal lines 426-435.
- Impact: Operators cannot reliably distinguish requested, prompt-sent, no-progress, and useful-progress counts specifically after resurrection from generic continuation outcomes.
- Required fix: add distinct summary counters or documented metric labels, expose them through MCP/GraphQL/Swift readback, and cover them in the P086 gate.

## Readiness Checklist

| Check | Status |
|---|---|
| Proposal requirements mapped to code | Complete |
| Mandatory repo-local reviewer used | Complete |
| Security-sensitive diff reviewed | Complete |
| Same-tree canonical gate passes | Failed |
| GraphQL compiles | Failed |
| Daemon resurrection integration tests run | Not reached |
| Swift readback tests run | Not reached |
| MCP/GraphQL no-oracle behavior verified for all principals | Failed |
| `resurrection_phase` readback parity verified | Failed |
| Ready for closeout | No |

## Verification Log

| Command / inspection | Result |
|---|---|
| `proposal_reviewer_reuse.py` for P086 | No reusable prior proposal-review artifacts found |
| `implementation_surface_fingerprint.py --json` for P086 | P086 touches Rust ACP/engine/db/MCP/GraphQL, Swift readback, schemas, fixtures, scripts; required lenses included API contract, architecture, reliability, security, observability, performance, Apple UI/UX |
| `security_sensitive_diff.py --json` for P086 | Triggered on auth, public ingress, filesystem/subprocess, parser, secrets/redaction/privacy, DoS/resource-limit surfaces |
| `./scripts/test-gate.sh proposal-086` | Failed with exit 101 at `graphql-server` compile; earlier domain, ACP Claude resurrection/session-store, DB lifecycle, engine lib, and MCP agents tests passed |
| Focused source inspections with `rg` and line-numbered reads | Confirmed adapter capability, Claude resume request, engine pre-prompt receipt/prompt marker, DB phase columns, missing primary phase readback, and attach-receipt run-scope leak |

## Final Verdict

P086 is a substantial partial implementation, not a closeout-ready implementation. The adapter and engine path are no longer merely unsupported; they include real Claude resume wiring, attach receipt persistence, prompt-correlation checks, DB phase state, and tests. However, the proposal cannot be marked Implemented because the canonical P086 gate fails, non-operator receipt readback leaks cross-run existence/projection data, and required `resurrection_phase`/receipt readback is not exposed through the primary MCP/GraphQL/report surfaces.

Required actions before a Ready/Implemented verdict:

1. Fix the GraphQL compile failures and rerun `./scripts/test-gate.sh proposal-086`.
2. Enforce run-scope/no-oracle behavior for MCP and GraphQL attach-receipt projections for operator, observer/read-only, and agent principals.
3. Add `resurrection_phase` and required attach/result fields to domain, DB repo selection, MCP status schema/output, GraphQL `ContinuationRecord`, report output, and Swift readback as applicable.
4. Add explicit `output_only_recovery` display/readback support.
5. Expose proposal-required resurrection metrics as distinct readback counters or documented labels and gate them.
6. Re-run the full canonical gate on the same tree and archive the passing output as P086 evidence.
