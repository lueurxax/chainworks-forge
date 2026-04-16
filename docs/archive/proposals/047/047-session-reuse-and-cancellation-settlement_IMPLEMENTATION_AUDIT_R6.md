# Proposal 047: Session Lineage, Context Budget, and Cancellation Settlement Multi-Lens Audit R6

| Field | Value |
|---|---|
| Proposal | docs/proposals/047-session-reuse-and-cancellation-settlement.md |
| Repository Root | . |
| Git SHA | db7d51aa91f71f898c4e621c01523708ca7d3c1b |
| Working Tree | dirty; many uncommitted control-plane, docs, DB, and generated `control-plane/target` changes were present before this report was written |
| Audited At | 2026-04-16T07:58:27+03:00 |
| Platform Scope | macOS control-plane / Rust daemon |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready with Risks |
| Audit Confidence | High |

## Executive Verdict

P047 is implemented on the audited tree. The Rust control plane now has durable session lineage and generations, policy-driven live ACP reuse, persisted execution provenance, generation-scoped budget evaluation from stored runtime signals, two-phase cancellation settlement, and the promised canonical/list reader split. The same-tree canonical proof gate passed with `bash ./scripts/test-gate.sh proposal-047`. The only readiness caveat is delivery reproducibility: the audited tree is heavily dirty and should be committed or otherwise frozen before this result is treated as a release baseline.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | No proposal-owned requirements remain open | High |
| Architecture | Strong | Runtime truth spans several crates, so regression coverage must stay proposal-gate owned | High |
| Product | Strong | The intended operator value is delivered through backend truth and readers | High |
| UI | Not Applicable | P047 explicitly excludes UI for a session inspector | High |
| UX | Acceptable | Reader UX is API/payload clarity, not an interactive flow | High |
| Readiness | Ready with Risks | Dirty worktree reduces handoff reproducibility until committed/frozen | High |

## Proposal Contract

### Scope

P047 covers three backend/control-plane slices:

- Durable session lineage with immutable generations, invocation owner keys, binding fingerprints, and append-only events matching the stable Swift owner chain.
- Generation-scoped context budget evaluation driven by hard guardrails and economic signals, not prompt-size heuristics alone.
- Two-phase cancellation settlement with `cancelling -> cancelled`, structured evidence, cancelled execution/work-item truth, and per-session close outcomes.

### Locked Decisions

- Legacy projection-era `session_lineages` must be renamed to `session_lineages_legacy`; canonical lineage truth starts in new `session_lineages`, `session_generations`, and `session_events` tables.
- No synthetic generation backfill is allowed from legacy rows.
- Binding fingerprint mismatch always rejects reuse, including family-scope reuse.
- `AcpRuntimeManager` owns live ACP session handles; DB `provider_session_id` alone is never enough to reuse.
- Execution records carry the "what happened" session provenance for report/recovery readers.
- Context budget decisions read persisted generation economics and runtime telemetry.
- Cancellation is two phase: phase 1 records execution-first settlement while the run stays `cancelling`; phase 2 records close outcomes and marks the run `cancelled`.
- Single-run readers expose full settlement JSON; list readers expose only summary.

### Primary User Flows

1. Reuse an agent session across loop iterations when lineage, binding fingerprint, owner policy, and live handle all remain valid.
2. Force a fresh generation when fingerprint, owner, reset, budget, missing live handle, or unverifiable history makes reuse unsafe.
3. Resume from a checkpoint-backed generation and preserve checkpoint provenance.
4. Cancel an active run and observe deterministic settlement truth after phase 1 and phase 2.
5. Inspect cancellation truth through GraphQL/MCP single-run and list-reader surfaces.

### UI Commitments

No direct UI implementation is in scope. The proposal explicitly excludes UI for a session inspector.

### UX Commitments

UX commitments are backend/operator-trust commitments:

- reuse must be safe and explainable through persisted provenance;
- cancellation must not report `cancelled` until settlement is finalized;
- list surfaces must stay compact while single-run surfaces expose full evidence.

### Acceptance Criteria

The proposal defines 27 acceptance criteria across session lineage, legacy migration, execution provenance, context budget, cancellation settlement, and northbound readers. This audit also treats the explicit `proposal-047|p047` gate as `REQ-028`.

### Test / Evidence Requirements

P047 requires a `proposal-047|p047` entry in `scripts/test-gate.sh` running `cargo test --workspace` from `control-plane`. This audit executed that gate on the audited tree.

### Explicit Exclusions

- Session checkpoint serialization format is out of scope.
- Provider-specific budget tuning is out of scope.
- UI for a session inspector is out of scope.

## Proposal Fidelity / Divergence

### Matches

- The legacy migration path exists and renames the old projection-era table before creating canonical lineage tables.
- Session lineage, generation, and event domain/repository owners exist.
- Invocation owner key and binding fingerprint builders implement the proposal tuple and sorted binding components.
- Policy evaluation covers reuse, reset, budget, checkpoint resume, family-scope owner relaxation, owner mismatch, fingerprint mismatch, and unverifiable history.
- ACP runtime reuse is handle-backed through `AcpRuntimeManager`, not just string-backed through persisted `provider_session_id`.
- Execution provenance is persisted on `agent_executions`.
- Budget evaluation reads persisted generation signals and runtime usage snapshots.
- Cancellation settlement persists execution-keyed entries, final close outcomes, `cancellation_settled_at`, and terminal cancellation status.
- GraphQL and MCP readers expose full single-run logs and list summaries as specified.
- The canonical proposal gate exists and passed.

### Divergences

- No proposal-owned implementation divergence found.

### Ambiguities / Evidence Gaps

- The worktree is dirty and includes broad uncommitted changes plus generated artifacts. This does not block proposal conformance because same-tree tests passed, but it is a handoff/reproducibility risk.
- Runtime validation used automated Rust workspace tests, not a live app UI flow, because P047 is control-plane scoped and excludes UI.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 28 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Reuse loop iterations through the same live ACP session and lineage
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC1.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:74`
  - `control-plane/crates/engine/src/executor.rs:388`
  - `control-plane/crates/engine/tests/integration.rs:1249`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Runtime manager validates the live handle before reuse and the end-to-end test proves reuse through a live session generation.

### REQ-002 Binding fingerprint changes force `FreshSessionRequired`
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC2.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:46`
  - `control-plane/crates/engine/src/session/policy.rs:103`
  - `control-plane/crates/engine/src/session/fingerprint.rs:127`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Fingerprint covers prompt, IO, worktree, backend profile, permission profile, MCP inventory, skill fields, output contract, max turns, and temperature.

### REQ-003 Operator reset ends the generation and next invocation gets `FreshAfterReset`
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC3.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/command_handler.rs:408`
  - `control-plane/crates/engine/src/session/policy.rs:307`
  - `control-plane/crates/engine/tests/integration.rs:1285`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Reset records an operator reset event, clears active generation, and closes the live ACP session when present.

### REQ-004 Canonical session lineage, generation, and event tables are populated
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC4.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:3`
  - `control-plane/crates/domain/src/session.rs:42`
  - `control-plane/crates/db/src/repos/sessions.rs:9`
  - `control-plane/crates/db/tests/integration.rs:21`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Repository CRUD inserts lineages, generations, and append-only events.

### REQ-005 Generation owner key and binding fingerprint are immutable after creation
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC5.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/src/repos/sessions.rs:30`
  - `control-plane/crates/db/src/repos/sessions.rs:182`
  - `control-plane/crates/db/src/repos/sessions.rs:204`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Update paths mutate active pointer, status/end reason, provider session id, and usage fields; they do not update `invocation_owner_key` or `binding_fingerprint`.

### REQ-006 Missing active generation row fails closed as `UnverifiableSessionHistory`
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC6.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:67`
  - `control-plane/crates/engine/src/session/policy.rs:70`
  - `control-plane/crates/engine/src/session/policy.rs:606`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Policy creates a fresh generation instead of trusting a dangling active-generation pointer.

### REQ-007 `ReusedAfterResume` records checkpoint artifact id on the new generation
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC7.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:198`
  - `control-plane/crates/engine/src/session/policy.rs:347`
  - `control-plane/crates/engine/src/session/policy.rs:396`
  - `control-plane/crates/engine/tests/integration.rs:2330`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Checkpoint provenance is stored on the new generation's `rehydrated_from_checkpoint_artifact_id`.

### REQ-008 Recovery branch mismatch under same-owner scope forces a fresh session
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC8.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/fingerprint.rs:12`
  - `control-plane/crates/engine/src/executor.rs:344`
  - `control-plane/crates/engine/src/session/policy.rs:122`
  - `control-plane/crates/engine/src/session/policy.rs:574`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: The owner key includes `owner_execution_lineage_id`; current runtime derives it from `stage_execution_id`, so retry/new stage execution identity changes the owner tuple and fails closed for `same_invocation_owner`.

### REQ-009 Generic invalidation maps to `FreshAfterInvalidation`
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC9.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/policy.rs:340`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Specialized budget, compaction, transport, timeout, reset, and checkpoint reasons map separately; generic invalidated status maps to `FreshAfterInvalidation`.

### REQ-010 DB-active generation without a live runtime handle is not reused
- Proposal Source: Section 4, Acceptance Criteria, Session Lineage AC10.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/acp/src/manager.rs:74`
  - `control-plane/crates/engine/src/executor.rs:393`
  - `control-plane/crates/engine/tests/integration.rs:2103`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Missing live handle invalidates the active generation and re-runs policy, yielding checkpoint-backed resume or fresh fail-closed behavior.

### REQ-011 Legacy projection-era session lineage schema migrates safely
- Proposal Source: Section 4, Acceptance Criteria, Legacy Migration AC11.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/002_projections.sql:1`
  - `control-plane/crates/db/migrations/006_session_lineage.sql:1`
  - `control-plane/crates/db/tests/integration.rs:21`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Migration renames old `session_lineages` to `session_lineages_legacy` and creates canonical tables.

### REQ-012 No synthetic generation backfill occurs from legacy rows
- Proposal Source: Section 4, Acceptance Criteria, Legacy Migration AC12.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/006_session_lineage.sql:1`
  - `control-plane/crates/db/tests/integration.rs:21`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Migration creates empty canonical generation/event tables and keeps legacy rows only in the renamed table.

### REQ-013 Agent executions persist session provenance after policy evaluation
- Proposal Source: Section 4, Acceptance Criteria, Execution-Side Provenance AC13.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/domain/src/agent.rs:50`
  - `control-plane/crates/db/src/repos/agent_executions.rs:8`
  - `control-plane/crates/engine/src/executor.rs:455`
  - `control-plane/crates/db/tests/integration.rs:169`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Persisted fields include lineage id, generation id, checkpoint id, owner key, reuse scope, family id, disposition, and reset reason.

### REQ-014 Report/recovery readers can answer reuse disposition from `agent_executions`
- Proposal Source: Section 4, Acceptance Criteria, Execution-Side Provenance AC14.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/src/repos/agent_executions.rs:39`
  - `control-plane/crates/engine/src/recovery.rs`
  - `control-plane/crates/mcp-server/src/tools/reports.rs`
  - `control-plane/crates/db/tests/integration.rs:169`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: The repository returns provenance directly with agent execution rows, so readers do not need lineage joins for "what happened" disposition truth.

### REQ-015 Twenty turns on a reused session triggers `Compact`
- Proposal Source: Section 4, Acceptance Criteria, Context Budget AC15.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:45`
  - `control-plane/crates/engine/src/session/budget.rs:153`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Default `max_turns` is 20 and triggers compaction.

### REQ-016 Estimated input tokens >= 128000 triggers `Compact`
- Proposal Source: Section 4, Acceptance Criteria, Context Budget AC16.
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:28`
  - `control-plane/crates/engine/src/session/budget.rs:54`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Covered by budget evaluator code and workspace gate; no separate exact-name test was required because evaluator unit tests and the workspace suite passed.

### REQ-017 Cumulative cost over 500 cents triggers `Invalidate`
- Proposal Source: Section 4, Acceptance Criteria, Context Budget AC17.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:31`
  - `control-plane/crates/engine/src/session/budget.rs:70`
  - `control-plane/crates/engine/src/session/budget.rs:167`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Runtime test also proves provider cost can be persisted and used by the next policy cycle.

### REQ-018 Reuse more expensive than fresh invalidates
- Proposal Source: Section 4, Acceptance Criteria, Context Budget AC18.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:112`
  - `control-plane/crates/engine/src/session/budget.rs:181`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: `normalized_savings_versus_fresh < -0.05` returns `Invalidate`.

### REQ-019 Transcript growth over 2.0x triggers `Compact`
- Proposal Source: Section 4, Acceptance Criteria, Context Budget AC19.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/session/budget.rs:32`
  - `control-plane/crates/engine/src/session/budget.rs:99`
  - `control-plane/crates/engine/src/session/budget.rs:195`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Evaluator uses the configured max transcript growth ratio.

### REQ-020 Budget signals are read from persisted generation state
- Proposal Source: Section 4, Acceptance Criteria, Context Budget AC20.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/migrations/007_session_budget_signals.sql:1`
  - `control-plane/crates/db/migrations/008_session_runtime_usage.sql:1`
  - `control-plane/crates/db/src/repos/sessions.rs:204`
  - `control-plane/crates/engine/src/session/policy.rs:262`
  - `control-plane/crates/engine/src/executor.rs:561`
  - `control-plane/crates/engine/tests/integration.rs:1686`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: ACP usage snapshots populate persisted input/cached/output/context/cost fields, and policy derives budget signals from the generation row.

### REQ-021 `CancelRun` phase 1 settles active work while run remains `Cancelling`
- Proposal Source: Section 4, Acceptance Criteria, Cancellation Settlement AC21.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/cancellation.rs:33`
  - `control-plane/crates/db/src/repos/agent_executions.rs:190`
  - `control-plane/crates/db/src/repos/work_items.rs:130`
  - `control-plane/crates/engine/tests/integration.rs:242`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Phase 1 persists entries with `session_close_succeeded: None`, cancels running agent executions/work items, marks running stages failed, and marks the run cancelling.

### REQ-022 Phase 2 records close outcomes, settled time, and terminal `Cancelled`
- Proposal Source: Section 4, Acceptance Criteria, Cancellation Settlement AC22.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/cancellation.rs:85`
  - `control-plane/crates/db/src/repos/runs.rs:171`
  - `control-plane/crates/engine/tests/integration.rs:1909`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Phase 2 closes live sessions through `AcpRuntimeManager`, updates entry close outcomes, writes `cancellation_settled_at`, and finalizes run status.

### REQ-023 Run status remains `Cancelling` between phase 1 and phase 2
- Proposal Source: Section 4, Acceptance Criteria, Cancellation Settlement AC23.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/engine/src/cancellation.rs:65`
  - `control-plane/crates/engine/src/cancellation.rs:71`
  - `control-plane/crates/engine/tests/integration.rs:279`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Finalization is spawned asynchronously after phase 1; the phase 1 test proves the intermediate state.

### REQ-024 Single-run readers expose full cancellation log and settled time
- Proposal Source: Section 4, Acceptance Criteria, Cancellation Settlement AC24.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/graphql-server/src/schema.rs:70`
  - `control-plane/crates/graphql-server/src/types/run.rs:28`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:126`
  - `control-plane/crates/graphql-server/src/schema.rs:724`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:265`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: `QueryRoot.run(id)` and MCP `runs.get` read canonical `Run` data and expose full log JSON.

### REQ-025 List readers expose summary only, not full JSON log
- Proposal Source: Section 4, Acceptance Criteria, Cancellation Settlement AC25.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/src/repos/projections.rs:15`
  - `control-plane/crates/db/src/repos/projections.rs:267`
  - `control-plane/crates/graphql-server/src/types/run.rs:53`
  - `control-plane/crates/graphql-server/src/schema.rs:789`
  - `control-plane/crates/mcp-server/src/tools/runs.rs:318`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: List projections carry `cancellation_settlement_summary`; full log is nil on projection-backed `GqlRun`.

### REQ-026 No active agent executions remain after phase 2
- Proposal Source: Section 4, Acceptance Criteria, Cancellation Settlement AC26.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/src/repos/agent_executions.rs:190`
  - `control-plane/crates/engine/tests/integration.rs:1909`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Running executions are terminally cancelled before phase 2 finalizes the run.

### REQ-027 No running work items remain after phase 2
- Proposal Source: Section 4, Acceptance Criteria, Cancellation Settlement AC27.
- Status: Implemented
- Evidence Type: code, tests-found, tests-run
- Evidence:
  - `control-plane/crates/db/src/repos/work_items.rs:130`
  - `control-plane/crates/engine/tests/integration.rs:242`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: Phase 1 cleanup marks running work items `cancelled`; phase 2 sees no running work item residue.

### REQ-028 Canonical `proposal-047|p047` gate exists and passes
- Proposal Source: Section 5, Test Gate.
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `scripts/test-gate.sh:1490`
  - `bash ./scripts/test-gate.sh proposal-047` -> passed
- Gap / Note: The gate runs `cargo test --workspace` from `control-plane` and passed on this audited tree.

## Architecture Review

**Summary:** Strong

No architecture findings block P047. The implementation follows the proposal's owner boundaries: DB lineage/generation/event truth lives in `db`/`domain`, runtime-backed reuse is owned by `AcpRuntimeManager`, policy/budget decisions live under `engine/src/session`, execution provenance is stored directly on `agent_executions`, and readers consume canonical/projection data according to the proposal split.

## Product Review

**Summary:** Strong

No product findings block P047. The user/operator value promised by the proposal is delivered: session reuse is safer and explainable, costly or stale sessions fail closed, cancellation settles deterministically, and readers expose the right level of evidence for single-run versus list contexts.

## UI Review

**Summary:** Not Applicable

P047 has no UI implementation scope. The proposal explicitly excludes UI for a session inspector.

## UX Review

**Summary:** Acceptable

No interactive UX findings block P047. The relevant UX surface is operator trust in backend payloads: cancellation is not prematurely marked terminal, list rows stay concise, and full evidence remains available on single-run reads.

## Delivery / Readiness Review

**Summary:** Ready with Risks

### READY-001 Dirty audited tree should be frozen before handoff
- Severity: Note
- Confidence: High
- Related Proposal Items / Requirements: All requirements; especially `REQ-028`.
- Evidence Type: code, tests-run
- Evidence:
  - `git status --short -- ':!control-plane/target'` showed many modified/untracked files before this report was written.
  - `bash ./scripts/test-gate.sh proposal-047` -> passed.
- Why It Matters: Same-tree tests prove the implementation, but a dirty tree is not a stable handoff artifact. Without a commit or frozen artifact reference, later audits may not be able to reproduce this exact state.
- Recommended Action: Commit or otherwise freeze the audited control-plane changes and generated docs/reports that are intended to be part of the delivery baseline.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `bash ./scripts/test-gate.sh proposal-047` compiled and tested the Rust control-plane workspace |
| Core user flow runtime-validated | Pass | Live reuse, missing-live-handle fallback, checkpoint resume, cost-budget invalidation, cancellation phase 1/finalize, and readers are covered by workspace tests |
| Empty/loading/error states covered | Not Applicable | No UI states in P047 scope |
| Accessibility risk acceptable | Not Applicable | No UI in scope |
| Localization risk acceptable | Pass | Payload strings are internal/operator diagnostics; no localized UI in scope |
| Critical tests executed | Pass | Proposal gate passed with ACP, DB, domain, engine, GraphQL, MCP, workflow, and doc tests |
| Full regression suite / canonical full gate passed on same tree/HEAD | Pass | `bash ./scripts/test-gate.sh proposal-047` passed on commit `db7d51aa91f71f898c4e621c01523708ca7d3c1b` with the current dirty working tree |
| Privacy/permissions/entitlements reviewed | Not Applicable | P047 is backend control-plane/session lifecycle logic and does not alter app entitlements |

## Verification Log

- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/047-session-reuse-and-cancellation-settlement.md` -> `docs/proposals/047-session-reuse-and-cancellation-settlement_IMPLEMENTATION_AUDIT_R6.md`
- `git rev-parse --show-toplevel`
- `git rev-parse HEAD`
- `git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete|replaces|replacement" docs/proposals/047-session-reuse-and-cancellation-settlement.md docs/proposals docs/reference`
- `nl -ba docs/proposals/047-session-reuse-and-cancellation-settlement.md`
- `nl -ba control-plane/crates/db/migrations/006_session_lineage.sql`
- `nl -ba control-plane/crates/db/migrations/007_session_budget_signals.sql`
- `nl -ba control-plane/crates/db/migrations/008_session_runtime_usage.sql`
- `nl -ba control-plane/crates/db/migrations/009_owner_execution_lineage.sql`
- `nl -ba control-plane/crates/domain/src/session.rs`
- `nl -ba control-plane/crates/db/src/repos/sessions.rs`
- `nl -ba control-plane/crates/engine/src/session/policy.rs`
- `nl -ba control-plane/crates/engine/src/session/budget.rs`
- `nl -ba control-plane/crates/engine/src/session/fingerprint.rs`
- `nl -ba control-plane/crates/acp/src/manager.rs`
- `nl -ba control-plane/crates/acp/src/session.rs`
- `nl -ba control-plane/crates/acp/src/transport.rs`
- `nl -ba control-plane/crates/engine/src/executor.rs`
- `nl -ba control-plane/crates/engine/src/cancellation.rs`
- `nl -ba control-plane/crates/db/src/repos/agent_executions.rs`
- `nl -ba control-plane/crates/db/src/repos/work_items.rs`
- `nl -ba control-plane/crates/db/src/repos/runs.rs`
- `nl -ba control-plane/crates/db/src/repos/projections.rs`
- `nl -ba control-plane/crates/graphql-server/src/schema.rs`
- `nl -ba control-plane/crates/graphql-server/src/types/run.rs`
- `nl -ba control-plane/crates/mcp-server/src/tools/runs.rs`
- `rg -n "test_runtime_manager_reuses_live_session_handle|test_claude_adapter_surfaces_usage_snapshot_from_stream_updates|test_invoke_agent_reuses_live_session_generation_end_to_end|test_invoke_agent_rehydrates_from_checkpointed_generation_and_persists_checkpoint_artifact|test_invoke_agent_missing_live_handle_falls_back_to_fresh_generation|test_invoke_agent_persists_runtime_cost_and_next_policy_invalidates_on_cost_budget|test_cancel_run_finalize_closes_live_session_via_runtime_manager" control-plane/crates`
- `bash ./scripts/test-gate.sh proposal-047` -> passed

## Recommended Next Actions

1. Commit or otherwise freeze the audited working tree so R6 can be reproduced later.
2. Treat P047 proposal-owned implementation as closed unless future changes modify session lineage, ACP runtime reuse, cancellation settlement, or reader contracts.
3. Keep `proposal-047` as the required regression gate for any future changes touching session policy, budget signals, cancellation settlement, or related northbound readers.
