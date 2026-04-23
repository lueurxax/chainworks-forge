# Proposal 061 Implementation Audit R1

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md` |
| Proposal revision | `p061-r3-generated-state-housekeeping` |
| Audit mode | `auto` via `proposal-implementation-audit` |
| Report | `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure_IMPLEMENTATION_AUDIT_R1.md` |
| Audit date | 2026-04-23 |
| Repo root | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | Current dirty worktree over `7912de0db5e68c5ed52ee5cb340c015bda23fa41` |
| Compare base | Implicit current worktree audit; no PR/base supplied |
| Working tree status | Dirty: relevant P061 source/proposal changes plus unrelated local deletions/untracked files |
| Canonical gate | `./scripts/test-gate.sh proposal-061` |
| Gate execution | Not run in this audit because source-level blockers make a successful readiness verdict impossible |
| Overall Conformance | **Not Implemented** |
| Overall Implementation Readiness | **Not Ready** |
| Audit Confidence | Medium-High |

## Prior Proposal-Review Reuse Summary

| Item | Result |
|---|---|
| Prior proposal-review artifacts found | None beside the proposal; no `.review/` directory or sibling review file was found for this proposal |
| Reviewer Selection Reuse | `Not reused` |
| Reason | No concrete prior reviewer-selection artifact was available; routing was derived from proposal scope and current implementation evidence |

## Selected Reviewers

| Reviewer | Why selected |
|---|---|
| `rust_arch_reviewer` | P061 changes Rust engine/db/module boundaries, SQLite transactions, migrations, scheduler state, and executor ownership |
| `rust_reliability_reviewer` | P061 is primarily about backpressure, retry/recovery, bounded scheduling, host interruption, idempotency, and overload behavior |
| `api_contract_reviewer` | P061 commits GraphQL/MCP readback, notification payloads, schema/read-model parity, and host-interruption API facts |
| `observability_rollout_reviewer` | P061 commits migrations, gate registration, runtime health, DB contention instrumentation, rollout/dogfood proof, and housekeeping safety evidence |
| `chainworks_execution_truth_reviewer` | P061 changes durable Run/Stage/Agent/WorkItem/artifact-claim/recovery truth across scheduler, executor, and readback surfaces |

## Rejected Close Alternatives

| Reviewer | Reason rejected |
|---|---|
| `macos_ui_reviewer` | UI commitments are audited as explicit REQ/UI findings, but current implementation evidence showed no Swift scheduler/backpressure UI wiring; the blocker is direct conformance, not nuanced visual review |
| `apple_ux_reviewer` | Same as above; no implemented Swift UX surface exists to review beyond missing committed behavior |
| `rust_security_reviewer` | No auth/secret/public-network/unsafe expansion appeared central to the P061 implementation slice |
| `rust_performance_reviewer` | P061 includes latency requirements, but the current blockers are conformance/readiness failures and the p95 gate was not run |
| `product_reviewer` | Product metrics exist, but user did not request product review and source-level implementation blockers dominate readiness |

## Proposal State and Contract Summary

Proposal state: `Active / Revised for implementation planning` based on the proposal metadata.

P061 commits a local-capacity and recovery-truth slice for the Rust control plane, not a SQLite-only migration. Its core contract is:

- Keep SQLite as the source of truth while serializing multi-row writes under bounded `BEGIN IMMEDIATE` transactions.
- Enforce default active-agent capacity caps: global `20`, per-run `4`, Claude `8`, Gemini `4`, Codex `3`, Auggie `1`, Junie `1`.
- Leave capacity-blocked `InvokeAgent` work pending/backpressured rather than failed.
- Persist durable scheduler queue summaries, health snapshots, freshness, DB writer contention, command latency, startup-recovery readback, host-interruption facts, and GraphQL/MCP parity readback.
- Use provider-family normalization everywhere capacity is counted so aliases cannot bypass caps.
- Add fair candidate selection using bounded windows and least-recently-served run state.
- Make retry, startup repair, claim/start, operator commands, and artifact import/supersession atomic where proposal-defined invariants cross rows/tables.
- Detect host sleep/wake and network migration, classify affected executions as host interruptions, clean up ACP/provider runtime, supersede source claims, and requeue with jitter under global/provider/run caps without consuming provider quota retry budget.
- Expose backpressure in operator surfaces and notifications instead of making operators poll.
- Add generated-state housekeeping that deletes only rebuildable inactive state and proves the safety allowlist in the P061 gate.

## Platform / Product Scope

| Scope Axis | Audit Classification |
|---|---|
| Apple platform scope | macOS operator UI diagnostics were explicitly in scope, but current Swift evidence does not implement the committed scheduler/backpressure UI surfaces |
| Backend/service scope | Rust daemon/service, SQLite persistence, background executor, work queue, GraphQL/MCP APIs, ACP recovery |
| Data scope | SQLite migrations and repository read/write models |
| Rollout scope | Canonical `proposal-061` test gate and stable reference docs |

## Primary Implementation Flows Audited

1. `InvokeAgent` scheduling: pending work is scanned, capacity-checked by global/provider/run counts, claimed under a writer transaction, and backpressured work remains pending with queue projections refreshed.
2. Scheduler readback: queue summaries, health snapshots, DB writer wait p95, command latency p95, startup recovery, and host interruption facts are exposed through GraphQL and MCP.
3. Host interruption recovery: host event creates an epoch, affected executions are cancelled, runtime cleanup is attempted, source claims are superseded, and retry work is requeued under capacity caps.
4. Operator visibility: scheduler/backpressure state should reach macOS UI surfaces, GraphQL subscription, MCP notification, and freshness indicators.
5. Generated-state housekeeping: daemon loop prunes only terminal-run generated outputs, stale ACP homes, and stale git temp objects while preserving active/blocked work, worktrees, artifacts, source files, and DB files.

## Proposal Fidelity / Divergence Inventory

### Matches

- Provider aliases normalize through a shared `ProviderFamily` resolver and unknown aliases fail loudly in domain/workflow tests.
- Scheduler queue summaries and health snapshots exist in migrations and DB repository code.
- GraphQL exposes scheduler health, queue summaries, startup recovery, command latency, DB writer contention, queue position hints, host interruption epochs, and a `schedulerBackpressureChanged` subscription.
- MCP `reports.get` includes scheduler health, startup recovery, queue summaries, backpressure notification state, and host interruption readback.
- Claim/start paths use bounded candidate windows, capacity checks, `BEGIN IMMEDIATE`, durable service state, and same-transaction queue projection refresh.
- Hot-index migrations and query-plan tests exist for pending `InvokeAgent` scans and active-count joins.
- Host interruption code records epochs, cancels affected executions, requeues work, marks active artifact source claims superseded, and has tests for late-output suppression and quota-budget exemption.
- Generated-state housekeeping code exists and is daemon-loop integrated.

### Divergences

- **Default Codex capacity is wrong:** the proposal says Codex default cap is `3`; implementation and tests set Codex to `10`.
- **The gate blesses the wrong capacity:** P061 gate routes through tests that assert Codex `10`, so the canonical gate cannot catch the proposal/default-cap drift.
- **Host-interruption cleanup failure still requeues:** implementation intentionally continues retry enqueue after runtime cleanup failure, contradicting AC-10b/settlement wording that ACP/provider runtime termination must happen before retry enqueue is complete.
- **Host-interruption durable facts omit proposed cleanup/quota status columns:** migration/read models do not persist `previous_status`, `settlement_status`, `cleanup_status`, or `quota_budget_effect` from the proposal schema contract.
- **macOS operator UI diagnostics are not implemented:** Swift app search found no scheduler/backpressure UI wiring or committed display strings despite explicit UI/UX surface requirements.
- **Generated-state housekeeping gate proof is incomplete:** housekeeping code exists, but `proposal-061` gate does not include housekeeping tests and existing tests cover only a subset of AC-16 safety behavior.

### Ambiguities / Evidence Gaps

- The current stable reference docs now describe Codex cap `10`, but P061 itself still says Codex cap `3`; this audit is proposal-anchored, so implementation is assessed against the proposal.
- The worktree is dirty, including relevant P061 changes and unrelated local deletions. This audit treats the current worktree as the implementation target, but readiness cannot be promoted without a clean same-tree gate after fixes.
- The P061 canonical gate was not run because direct source evidence already establishes conformance blockers.

## Requirement Summary

| REQ | Title | Status | Evidence Types |
|---|---|---|---|
| REQ-001 | Default capacity caps and bounded active agents | **Partially Implemented** | proposal, code, tests-found |
| REQ-002 | Capacity-blocked work remains pending/backpressured | Implemented | proposal, code, tests-found |
| REQ-003 | Provider alias normalization and unknown-provider failure | Implemented | proposal, code, tests-found |
| REQ-004 | Durable scheduler projections, freshness, GraphQL/MCP readback, notifications | Implemented | proposal, migration, code, tests-found |
| REQ-005 | Fair bounded candidate selection | Implemented | proposal, code, tests-found |
| REQ-006 | SQLite write serialization and command latency proof | Partially Implemented | proposal, code, tests-found |
| REQ-007 | Retry/stale repair/claim-start atomicity | Partially Implemented | proposal, code, tests-found |
| REQ-008 | Startup recovery through capacity gates and readback | Implemented | proposal, code, tests-found |
| REQ-009 | Host interruption classification, cleanup, requeue, quota, late-output settlement | **Partially Implemented** | proposal, migration, code, tests-found |
| REQ-010 | DB contention instrumentation | Implemented | proposal, code, tests-found |
| REQ-011 | Hot indexes and query-plan proof | Implemented | proposal, migration, tests-found |
| REQ-012 | Generated-state housekeeping safety and gate proof | **Partially Implemented** | proposal, code, tests-found |
| REQ-013 | macOS operator UI backpressure diagnostics | **Missing** | proposal, code-search |
| REQ-014 | Canonical P061 gate validates the committed contract | **Missing** | proposal, config, tests-found |
| REQ-015 | Dogfood 5/10-run rollout evidence | Not Verifiable | proposal |

## Detailed REQ Audit

### REQ-001: Default capacity caps and bounded active agents

- Proposal source: `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:76`, `:228`, `:710`.
- Status: **Partially Implemented**.
- Evidence references:
  - `control-plane/crates/domain/src/provider.rs:104` sets `ProviderFamily::Codex` cap to `10`.
  - `control-plane/crates/domain/src/provider.rs:208` tests Codex default as `10`.
  - `control-plane/crates/engine/src/capacity.rs:80` tests loaded Codex default as `10`.
- Implementation mapping: global `20`, per-run `4`, Claude `8`, Gemini `4`, Auggie `1`, and Junie `1` are represented, but Codex is not.
- Gap / note: The proposal requires Codex `3`; implementation and tests enforce `10`. This is not a tuning backlog item because P061 explicitly chose daemon-owned conservative caps as the safety model.

### REQ-002: Capacity-blocked work remains pending/backpressured

- Proposal source: `current_baseline.remaining_work`, `architecture.capacity_model.behavior_when_full`, AC-2.
- Status: Implemented.
- Evidence references:
  - `control-plane/crates/engine/src/executor.rs:147-215` prechecks capacity, scans candidates, skips backpressured work, and selects eligible work.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:1446` proves a provider-capped item remains pending while a later eligible provider is claimed.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:1558` proves all-blocked pending work reports no eligible work.
- Gap / note: Implementation evidence is strong by code/tests-found, but tests were not run in this audit.

### REQ-003: Provider alias normalization and unknown-provider failure

- Proposal source: `architecture.capacity_model.provider_normalization` and AC-13.
- Status: Implemented.
- Evidence references:
  - `control-plane/crates/domain/src/provider.rs:37-55` resolves aliases to canonical provider families and fails unknown aliases.
  - `control-plane/crates/workflow/src/compiler.rs:393-399` routes YAML provider normalization through the shared provider-family resolver.
  - `control-plane/crates/workflow/tests/integration.rs:655` and `:683` cover alias resolution and unknown-provider validation.
- Gap / note: No conformance gap found for normalization behavior.

### REQ-004: Durable scheduler projections, freshness, GraphQL/MCP readback, notifications

- Proposal source: `architecture.work_item_state_and_readback`, `architecture.api_contracts`, AC-9, AC-14.
- Status: Implemented.
- Evidence references:
  - `control-plane/crates/db/migrations/021_scheduler_backpressure_foundation.sql:61-98` creates scheduler queue/health projection tables.
  - `control-plane/crates/db/src/repos/scheduler.rs:595-742` refreshes queue summaries, health snapshots, freshness, contention, command-latency, and host-interruption linkage in one write unit.
  - `control-plane/crates/graphql-server/src/schema.rs:235-413` exposes GraphQL scheduler/host-interruption readback.
  - `control-plane/crates/graphql-server/src/schema.rs:1256` exposes `schedulerBackpressureChanged`.
  - `control-plane/crates/mcp-server/src/tools/reports.rs:88-96`, `:198-282` exposes MCP report readback.
  - `control-plane/crates/mcp-server/src/server.rs:54-73` maps domain backpressure events to MCP notification method `scheduler.backpressure.changed`.
- Gap / note: GraphQL/MCP parity is represented, but macOS UI consumption is tracked separately as REQ-013.

### REQ-005: Fair bounded candidate selection

- Proposal source: `architecture.scheduler_fairness`, rollout phase 4.
- Status: Implemented.
- Evidence references:
  - `control-plane/crates/engine/src/executor.rs:93` defines bounded scan limit `32`.
  - `control-plane/crates/engine/src/executor.rs:141-215` scans pending candidates and keeps eligible candidates under capacity.
  - `control-plane/crates/engine/src/executor.rs:584-607` implements least-recently-served ordering with deterministic scheduled/id tie-breaks.
  - `control-plane/crates/engine/src/executor.rs:609-626` persists run service state.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:1650` proves unserved run selection over older recently served work.
- Gap / note: The proposal also wanted many-run windows and restart proof. Tests-found prove least-recently-served state; full gate was not run.

### REQ-006: SQLite write serialization and command latency proof

- Proposal source: `architecture.sqlite_write_serialization`, AC-5, AC-12.
- Status: Partially Implemented.
- Evidence references:
  - `control-plane/crates/engine/src/executor.rs:141-316` claim/start uses `BEGIN IMMEDIATE` and avoids provider I/O inside the transaction.
  - `control-plane/crates/db/src/repos/scheduler.rs:375-427`, `:690-742`, `:843-898` records writer wait and command latency observations into health snapshots.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:346` defines the approve/retry/cancel p95 test under 20 fake agents.
- Gap / note: Code/tests-found exist, but this audit did not run the p95 command-latency test or the canonical gate. Therefore the latency acceptance is not verified in this audit.

### REQ-007: Retry/stale repair/claim-start atomicity

- Proposal source: `architecture.sqlite_write_serialization.operation_contracts`, `retry_supersession_and_stale_repair`, AC-6, AC-8.
- Status: Partially Implemented.
- Evidence references:
  - `control-plane/crates/engine/src/executor.rs:141-316` claim/start and projection refresh happen in the same transaction.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:889` covers retry-stage projection cleanup after supersession.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:1023` and related retry/startup repair tests cover supersession and stale-running cleanup paths.
- Gap / note: Tests were not run. No additional source-level contradiction was found in the inspected claim/start slice.

### REQ-008: Startup recovery through capacity gates and readback

- Proposal source: `architecture.startup_recovery`, AC-7.
- Status: Implemented.
- Evidence references:
  - `control-plane/crates/db/migrations/022_startup_recovery_readback.sql:1-13` creates startup recovery readback.
  - `control-plane/crates/db/src/repos/startup_repairs.rs` records recovery readbacks from scheduler summaries.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:799` proves startup repair readback counts requeued InvokeAgent backpressure.
  - `control-plane/crates/graphql-server/src/schema.rs:246-254` and `control-plane/crates/mcp-server/src/tools/reports.rs:180-196` expose startup recovery readback.
- Gap / note: Tests were not run in this audit.

### REQ-009: Host interruption classification, cleanup, requeue, quota, late-output settlement

- Proposal source: `architecture.host_interruption`, AC-10a, AC-10b, AC-10c, AC-11.
- Status: **Partially Implemented**.
- Evidence references:
  - `control-plane/crates/engine/src/host_interruption.rs:295-437` records epochs, cancels affected executions, requeues work, refreshes scheduler state, and publishes notifications.
  - `control-plane/crates/engine/src/host_interruption.rs:444-506` performs runtime cleanup before the DB transaction but returns only counts; failures do not block retry enqueue.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:2243-2276` explicitly expects cleanup failure not to block settlement/retry requeue.
  - `control-plane/crates/db/migrations/021_scheduler_backpressure_foundation.sql:110-128` persists host-interruption epoch and affected-execution tables without `cleanup_status` or `quota_budget_effect`.
  - `control-plane/crates/engine/tests/proposal_061_backpressure.rs:2288` covers quota-budget exemption behavior.
- Gap / note: Detection/classification/requeue/late-output/quota behavior is substantially implemented, but AC-10b cleanup semantics and durable evidence are incomplete.

### REQ-010: DB contention instrumentation

- Proposal source: metrics, AC-12.
- Status: Implemented.
- Evidence references:
  - `control-plane/crates/db/migrations/021_scheduler_backpressure_foundation.sql:100-108` creates writer wait observations.
  - `control-plane/crates/db/src/repos/scheduler.rs:375-413` records and computes writer wait p95.
  - `control-plane/crates/graphql-server/src/schema.rs:277-293` exposes DB writer contention.
  - `control-plane/crates/mcp-server/src/tools/reports.rs:198-216` includes DB writer wait in MCP report truth.
- Gap / note: Code/readback exist; runtime contention was not generated in this audit.

### REQ-011: Hot indexes and query-plan proof

- Proposal source: `architecture.hot_indexes`, AC-15.
- Status: Implemented.
- Evidence references:
  - `control-plane/crates/db/migrations/019_scheduler_hot_indexes.sql:1-12` and `021_scheduler_backpressure_foundation.sql:29-43` add scheduler hot indexes.
  - `control-plane/crates/db/tests/integration.rs:508-636` seeds 1000 pending work items and 500 running executions and asserts query plans use intended indexes.
- Gap / note: Tests were not run in this audit.

### REQ-012: Generated-state housekeeping safety and gate proof

- Proposal source: `architecture.generated_state_housekeeping`, AC-16.
- Status: **Partially Implemented**.
- Evidence references:
  - `control-plane/crates/engine/src/housekeeping.rs:29-36` implements env configuration.
  - `control-plane/crates/engine/src/housekeeping.rs:100-166` prunes only terminal-run worktree targets and stale runtime/git temp state.
  - `control-plane/crates/engine/src/housekeeping.rs:347-383` tests terminal status filtering and live ACP-home protection.
  - `control-plane/crates/engine/src/executor.rs:1469-1484` integrates the housekeeping loop into the background executor.
  - `scripts/test-gate.sh:430-447`, `:2403-2415` does not include any housekeeping test in `proposal-061`.
- Gap / note: The core implementation exists, but AC-16 explicitly requires proof for active/blocked preservation and forbidden deletion classes; current gate inventory does not exercise it.

### REQ-013: macOS operator UI backpressure diagnostics

- Proposal source: `ux_ui_notes.surfaces` and `ux_ui_notes.copy_guidelines`.
- Status: **Missing**.
- Evidence references:
  - Code search for `Scheduler Health`, `schedulerBackpressure`, `oldestQueued`, `Waiting for provider slot`, `Run at agent limit`, `Database writer busy`, `Recovering from system sleep`, `moon.zzz`, and `wifi.exclamationmark` in `Chainworks Forge/` and `Chainworks ForgeTests/` returned no matches.
  - `Chainworks Forge/Views/PilotReadinessView.swift:1-220` contains sections for hero/config/providers/diagnostics/operator status/actions/support, but no Scheduler Health section or P061 queue/pressure fields.
- Gap / note: Stable docs describe this UI, but current Swift implementation does not expose the committed scheduler/backpressure UI surfaces.

### REQ-014: Canonical P061 gate validates the committed contract

- Proposal source: canonical gate metadata and `test_plan.gate`.
- Status: **Missing**.
- Evidence references:
  - `scripts/test-gate.sh:430-447` lists P061 focused tests.
  - `scripts/test-gate.sh:2403-2415` runs domain/workflow/db/engine/graphql/mcp focused tests.
  - `control-plane/crates/domain/src/provider.rs:208` and `control-plane/crates/engine/src/capacity.rs:80` assert Codex default `10`, not proposal-required `3`.
  - `scripts/test-gate.sh:430-447` lacks housekeeping safety tests required by AC-16.
- Gap / note: The gate is present, but it does not currently prove the proposal contract as written.

### REQ-015: Dogfood 5/10-run rollout evidence

- Proposal source: rollout phases 7 and 8; metrics.
- Status: Not Verifiable.
- Evidence references: No 4-hour 5-run or 2-hour 10-run dogfood log/evidence was found or run during this audit.
- Gap / note: This is rollout evidence, not necessarily a code implementation blocker for the base gate, but it blocks claiming dogfood/readiness completion.

## Reviewer / Lens Scorecard

| Lens | Score | Top Risk | Confidence |
|---|---|---|---|
| Proposal conformance | Not Implemented | Proposal capacity contract and host-interruption cleanup semantics are violated | High |
| Rust architecture | Partial | Implementation is broad and mostly aligned, but stable docs/implementation have drifted from proposal caps | Medium-High |
| Rust reliability | Not Ready | Cleanup failure still requeues host-interrupted work despite AC-10b cleanup-before-complete wording | High |
| API contract | Partial | Host-interruption durable facts omit cleanup/quota status fields proposed for readback/auditability | Medium-High |
| Observability/rollout | Not Ready | Canonical gate can pass while blessing wrong caps and missing AC-16 housekeeping proof | High |
| Chainworks execution truth | Partial | Scheduler/execution truth exists, but host-interruption retry evidence is not truthful enough about cleanup/quota outcomes | High |
| macOS operator UI | Missing | Committed operator backpressure surfaces are only documented, not implemented in Swift | High |
| Readiness | Not Ready | Source-level blockers; no same-tree gate run; dirty worktree | High |

## Routed Specialist Findings

### REL-001: Default Codex provider cap violates the proposal capacity contract

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-001; P061 goals/default caps; `architecture.capacity_model.defaults`; AC-3 capacity model.
- Evidence types: proposal, code, tests-found.
- Evidence references:
  - Proposal requires Codex `3`: `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:76` and `:228`.
  - Implementation sets Codex `10`: `control-plane/crates/domain/src/provider.rs:104`.
  - Tests assert Codex `10`: `control-plane/crates/domain/src/provider.rs:208`, `control-plane/crates/engine/src/capacity.rs:80`.
- Why it matters: P061’s safety model depends on conservative daemon-owned provider caps. Raising Codex from `3` to `10` allows materially more concurrent Codex sessions than the proposal reviewed, and the current tests would pass the wrong value.
- Recommended action: Change the default Codex cap to `3`, update capacity loader/domain tests to expect `3`, and update any stable reference docs that currently say Codex `10` unless the proposal is explicitly revised first.
- Acceptance criteria: `InvokeAgentCapacityConfig::default().provider_cap(ProviderFamily::Codex) == 3`; capacity tests assert `3`; `proposal-061` gate includes and passes the corrected default-cap proof.

### REL-002: Host-interruption retries are enqueued even when runtime cleanup fails

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-009; AC-10b.
- Evidence types: proposal, code, tests-found.
- Evidence references:
  - Proposal requires cleanup before retry completion: `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:561`, `:718`.
  - Runtime cleanup failure is counted but does not block transaction/requeue: `control-plane/crates/engine/src/host_interruption.rs:300`, `:344-350`, `:444-506`.
  - Test explicitly blesses cleanup failure requeue: `control-plane/crates/engine/tests/proposal_061_backpressure.rs:2243-2276`.
- Why it matters: A retry can start while the old ACP session/provider process group may still be alive. That reintroduces the very provider handshake/process-storm and late-output race P061 is trying to control.
- Recommended action: Make retry enqueue conditional on cleanup success, or persist a distinct `cleanup_failed_retry_deferred` state and leave work pending/backpressured until cleanup succeeds or an explicit operator policy override applies.
- Acceptance criteria: A cleanup-failure fixture records the epoch and affected execution, does not enqueue retry work as complete, exposes cleanup failure in readback, and only requeues after successful cleanup or explicit operator override.

### API-001: Host-interruption readback omits cleanup and quota evidence required by the proposal schema contract

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related proposal items / REQs: REQ-009; AC-10b; AC-11.
- Evidence types: proposal, migration, code.
- Evidence references:
  - Proposal schema names `previous_status`, `cleanup_status`, and `quota_budget_effect`: `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:549-553`.
  - Current migration only stores `action` and `retry_enqueued_at` for affected executions: `control-plane/crates/db/migrations/021_scheduler_backpressure_foundation.sql:121-128`.
  - Current read model only exposes `action`/`retry_enqueued_at`: `control-plane/crates/db/src/repos/scheduler.rs:103-110`, `:510-519`, `:1513-1521`.
- Why it matters: Operators and tests cannot distinguish cleanup-success retry, cleanup-failed retry, deferred retry, or quota-exempt retry from durable facts. That makes AC-10b/AC-11 auditability dependent on transient summary counters or tests rather than persisted truth.
- Recommended action: Add migration/read-model/API fields for previous execution status, settlement status, cleanup status, and quota budget effect; populate them in `HostInterruptionService`; expose them through GraphQL and MCP.
- Acceptance criteria: GraphQL/MCP host-interruption readback shows cleanup and quota-budget outcomes for each affected execution; tests assert cleanup-failed, cleanup-succeeded, quota-exempt, and late-output cases.

### UI-001: P061 operator backpressure surfaces are not implemented in the macOS app

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-013.
- Evidence types: proposal, code-search.
- Evidence references:
  - Proposal requires RunsHome/Run detail/Stage detail/Scheduler Health/banner/host-interruption/freshness surfaces in `ux_ui_notes.surfaces`.
  - Search in `Chainworks Forge/` and `Chainworks ForgeTests/` found no Swift usage of P061 scheduler/backpressure strings or GraphQL fields.
  - `Chainworks Forge/Views/PilotReadinessView.swift:1-220` has no Scheduler Health section.
- Why it matters: P061 positions backpressure as visible normal scheduling state. Without Swift operator surfaces, a user still cannot see queued/backpressured state from the app even if Rust/GraphQL/MCP state exists.
- Recommended action: Add a macOS scheduler health/readback model and UI sections for the committed surfaces, or explicitly narrow P061 to Rust/API-only and move macOS UI diagnostics to a follow-up proposal.
- Acceptance criteria: Swift UI exposes queued count, active count, oldest queued age, top reason, freshness, DB writer pressure, command latency, sustained-backpressure banner, and friendly host-interruption labels; tests or snapshots cover the visible states.

### READY-001: The canonical P061 gate does not prove the proposal contract as written

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related proposal items / REQs: REQ-014; REQ-012.
- Evidence types: proposal, config, tests-found.
- Evidence references:
  - P061 canonical gate: `docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md:734`.
  - Gate command inventory: `scripts/test-gate.sh:430-447`, `:2403-2415`.
  - Wrong default-cap assertions: `control-plane/crates/domain/src/provider.rs:208`, `control-plane/crates/engine/src/capacity.rs:80`.
  - AC-16 housekeeping proof is not represented in the P061 gate list: `scripts/test-gate.sh:430-447`.
- Why it matters: Even a green `proposal-061` run would not prove P061 readiness while it asserts Codex `10` and omits housekeeping safety coverage. This blocks handoff because the release/check gate is supposed to be the canonical proof of the proposal.
- Recommended action: Fix the cap assertions, add focused housekeeping safety tests to `PROPOSAL_061_TESTS`, and rerun `./scripts/test-gate.sh proposal-061` on the fixed same tree.
- Acceptance criteria: Gate fails before the fixes, passes after the fixes, and its log proves corrected caps plus AC-16 housekeeping safety.

## Readiness Checklist

| Item | Status | Evidence / Note |
|---|---|---|
| Canonical gate status | Not run / Not accepted | Source-level blockers found; no successful readiness verdict claimed |
| Same-tree full regression or proposal gate | Missing | Required before any `Ready` or `Ready with Risks` verdict |
| Core scheduler flow validation | Tests found | `proposal_061_backpressure.rs` covers capacity, p95, fairness, retry, backpressure, host interruption |
| GraphQL/MCP parity validation | Tests found | GraphQL schema and MCP reports include `proposal_061` tests |
| DB migration/query-plan proof | Tests found | DB integration test seeds 1000 pending + 500 running rows |
| Host-interruption cleanup proof | Failing by source semantics | Cleanup failure requeue is currently accepted by test/code |
| UI/UX empty/loading/error/freshness states | Missing | No Swift scheduler/backpressure UI implementation found |
| Accessibility/localization/privacy/permissions | Not Verifiable | UI missing; no dedicated evidence reviewed |
| Housekeeping safety proof | Partial | Implementation exists; gate lacks required AC-16 focused proof |
| Dogfood 5/10-run evidence | Not Verifiable | No dogfood log reviewed or run |

## Verification Log

| Command / Inspection | Result |
|---|---|
| `git rev-parse --show-toplevel && git rev-parse HEAD && git status --short` | Repo root `/Users/user/Documents/Chainworks Forge`; HEAD `7912de0d...`; dirty worktree with relevant P061 changes and unrelated local changes |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py ...061...md` | Selected this report path: `..._IMPLEMENTATION_AUDIT_R1.md` |
| `find docs/proposals ...061...review/audit...` | No prior P061 review/audit artifacts found |
| Proposal reads (`sed -n`) | Extracted P061 goals, defaults, acceptance criteria, host-interruption schema, housekeeping, and gate contract |
| Router/registry reads | Selected Rust architecture, Rust reliability, API contract, observability rollout, and Chainworks execution truth lenses |
| Implementation searches (`rg`) | Located scheduler, host-interruption, GraphQL, MCP, gate, tests, and housekeeping evidence |
| Swift UI search for P061 scheduler/backpressure strings and fields | No matches in `Chainworks Forge/` or `Chainworks ForgeTests/` |
| `./scripts/test-gate.sh proposal-061` | Not run; direct conformance blockers already make successful verdict impossible |

## Final Verdict

Overall conformance is **Not Implemented** because the implementation violates explicit P061 requirements for default capacity caps, host-interruption cleanup semantics/evidence, macOS operator UI visibility, and canonical gate proof.

Overall implementation readiness is **Not Ready**. The implementation has substantial Rust scheduler/API work, but P061 cannot be closed or handed off until the capacity contract is corrected, host-interruption cleanup/readback semantics are made executable, UI scope is either implemented or explicitly split out, housekeeping proof is added to the gate, and a same-tree `./scripts/test-gate.sh proposal-061` run passes after those fixes.

## Recommended Next Actions

1. Fix Codex default cap from `10` to proposal-required `3` or deliberately revise P061 and reroute the proposal review before accepting `10`.
2. Change host-interruption cleanup failure handling so retry is not considered complete until ACP/provider cleanup succeeds, or add a durable deferred/override model.
3. Add cleanup/quota outcome fields to host-interruption migrations/read models and expose them through GraphQL/MCP.
4. Implement the macOS scheduler/backpressure UI surfaces or split them into a follow-up and narrow P061 acceptance.
5. Add AC-16 housekeeping tests to `proposal-061` gate and run the gate on the fixed same tree.
