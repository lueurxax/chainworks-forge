# Proposal 058 Implementation Audit R3: Configurable Agent Escalation Chains

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Proposal revision | `p058-r14-2026-05-07` |
| Audit timestamp | `2026-05-28T05:42:57Z` |
| Audit mode | proposal implementation audit |
| Report path | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R3.md` |
| Auditor | Codex |

## Implementation Target / Compare Base

| Field | Value |
| --- | --- |
| Target worktree | `.chainworks/worktrees/cw-configurable-agent-escalation-6764a0c2` |
| Target branch | `cw/configurable-agent-escalation/6764a0c2` |
| Target HEAD | `ce9e7e825cb3777e89c5cb08b619dd0aa863d033` |
| Compare base | `3a93e76332512fc07e8b7bec50882ee83d703c2f` (`git merge-base origin/main HEAD`) |
| Working tree | Dirty; many modified Rust, Swift, docs, migrations, and untracked `docs/proposals/058-configurable-agent-escalation-chains.md` plus `Chainworks Forge/Views/EscalationReadSurfaceViews.swift` |
| Proposal source | The user-supplied proposal in the main workspace is the audited contract |

Target proposal drift: the target worktree contains its own untracked copy of `docs/proposals/058-configurable-agent-escalation-chains.md` that differs from the audited proposal. It removes `shutdown drain` from the summary line and changes Phase 3 scope from `graceful shutdown drain, classifier contract required` to `graceful classifier contract required`. This audit treats the explicitly supplied main-workspace proposal as authoritative and records the target-copy drift as a readiness risk.

## Prior Proposal-Review Reuse

| Item | Result |
| --- | --- |
| Discovery command | `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py /Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md` |
| Prior proposal-review artifacts | None found |
| Reviewer-selection reuse | Not reused |
| Notes | Prior `IMPLEMENTATION_AUDIT` reports were not used for reviewer selection per skill rules. |

## Selected Reviewers

| Reviewer | Why selected |
| --- | --- |
| `rust_arch_reviewer` | Rust control-plane scheduler, workflow compiler, persistence, recovery, and daemon boundaries are central to P058. |
| `rust_reliability_reviewer` | P058 promises idempotency, no-overlap tier advancement, retries, shutdown/replay, late-frame handling, capacity pauses, and recovery invariants. |
| `api_contract_reviewer` | GraphQL, MCP, report/readback, raw-string forward compatibility, migrations, and DTO contracts are explicit proposal surfaces. |
| `macos_ui_reviewer` | Governed macOS read surfaces, adapter authority, presentation components, accessibility, notifications, and Dock/menu behavior are explicit proposal surfaces. |
| `observability_rollout_reviewer` | Rollout contract, metrics, gates, rollback, runbooks, migration drills, and release decision readback are explicit acceptance surfaces. |

Rejected close alternatives: `security_reviewer` was not selected as a standalone reviewer because the inspected security-sensitive payload validation and authz behavior are covered under API/architecture evidence in this audit; `performance_reviewer` was not selected because no performance hot-path benchmark claim was newly introduced beyond metric/SLO readiness; `product_reviewer` was not selected because product gates are represented as rollout/observability acceptance criteria rather than a separate product experiment.

## Proposal State And Contract Summary

Proposal state: Active implementation proposal. The proposal remains present under `docs/proposals/`, has `Status: refined_after_write_boundary_blocker_resolved`, and has not been retired into reference-only truth.

Contract summary:

- Define repo-owned `escalation_policy_v1` using backend profile ids, ordered tier kinds, typed trigger vocabulary, strict compile validation, and frozen policy truth in `RunPlan`.
- Keep Rust control plane as the only escalation authority for policy resolution, classification, tier advancement, pauses, capacity, persistence, recovery, and kill-switch behavior.
- Persist ledger, execution metadata, runtime facts, and event journal rows with idempotency and no overlapping active tier.
- Expose raw-string, forward-compatible GraphQL, MCP, report, and macOS readback.
- Implement fail-closed operational behavior for deadlines, capacity probes, force-detach, shutdown drain/replay, launch-recycle storms, late frames, policy drift, and unsafe side effects.
- Provide governed macOS read/subscription presentation, with a read-only `EscalationReadAdapter`, status/banner/lineage/pause/trace/drift/inspector surfaces, accessibility behavior, Dock/attention/notification/menu affordances, and no policy-drift mutation.
- Emit and surface P058 metrics and rollout decision fields, and prove the proposal through `./scripts/test-gate.sh proposal-058`.

## Platform / Product Scope

| Scope | Classification |
| --- | --- |
| Apple platform | macOS |
| Backend/service | Rust control-plane service, worker/scheduler, recovery, SQLite data layer |
| API | GraphQL readback, MCP `runs.get`/resource readback, report/readback lanes |
| Data | SQLite migrations, ledgers, execution metadata, runtime facts, event journal |
| Rollout/ops | Gate, fixture, metrics, runbooks, rollback, migration/recovery evidence |

## Primary Implementation Flows

1. Policy compile and run start: YAML catalog/workflow defines `escalation_policy_v1`; compiler validates bindings, unsafe stages, unknown profiles, strict fields, and freezes policy hash/binding truth into the run plan.
2. Agent execution claim/start: scheduler-owned claim creates one `agent_executions` row, one escalation ledger, one metadata row, and artifact/source-generation ownership in one transaction.
3. Failure classification and tier advancement: completed executions classify failure triggers, write redacted runtime facts/events, advance durable tier state, and enqueue/suppress retries according to the frozen policy.
4. Recovery and fail-closed pauses: kill switch, deadlines, capacity probe failures, launch-recycle storms, provider force-detach, shutdown replay, late-frame handling, and policy drift pause instead of mutating tiers locally.
5. Operator readback: GraphQL/MCP/report/macOS surfaces show raw-string escalation state, redacted event trace, pause reasons, action hints, runbook anchors, metrics/rollout fields, and read-only drift workflow handoff.

## Proposal Fidelity / Divergence Inventory

### Matches

- The P058 gate is registered and passed on the audited worktree.
- Migrations `063`, `064`, and `065` create escalation tables, require redaction versions, and enforce chain/idempotency uniqueness.
- Rust code includes schema/domain validation, strict payload validation, forward-compatible raw strings, and unsafe side-effect compile rejection.
- Claim/start and scheduler tests prove single-row ownership, durable tier advancement, `same_backend_retry`, `backend_profile`, `lead_mediation`, and `pause` handling.
- Recovery now includes startup force-detach replay for running escalation executions and late-frame event journaling in the same transaction as runtime facts.
- GraphQL and MCP readback include capped chain/event/meta arrays, event-derived fields, and non-Operator summary redaction.
- Swift now includes a governed read adapter, raw-string DTOs, presentation snapshot, read-only components, and focused Swift tests.
- Rollout fixture now reports `pass` / `release` across `run_report`, `mcp`, `release_receipt`, and `graphql` lanes.

### Divergences

- The target worktree proposal copy diverges from the user-supplied proposal and removes shutdown-drain wording. That creates specification drift inside the implementation branch.
- The macOS adapter still lists remaining integration work for GraphQL subscription/transport stale handling, readback refreshes, runbook opening, AppKit attention, Dock badge updates, and notifications.
- The implemented macOS surface is mostly constructible components and DTO tests, not a wired run-detail/menu-bar/notification experience with remote visual/runtime proof.
- Several metric names are declared and some events increment counters, but proposal-level histogram/SLO semantics and producers for all required metrics are not proven.
- The rollout fixture says `release` while still listing remote visual/runtime soak, long-run metric trending, and migration/recovery drills as next steps.

### Ambiguities / Evidence Gaps

- No live daemon run was executed during this audit; evidence is from focused unit/integration tests and fixtures.
- No remote UI visual/runtime test was executed for the governed macOS surfaces.
- No live SIGTERM/operator-restart soak was executed; the implemented proof is startup recovery unit/integration coverage.
- No full repository regression gate was executed; the canonical proposal gate passed, which is sufficient for the proposal lane but not a broad release proof.

## Residual Scope / Follow-up Ownership

| Residual item | Owner in audited proposal | Blocks conformance/readiness? | Evidence |
| --- | --- | --- | --- |
| macOS live integration for subscription/stale states, readback refresh, runbook opening, AppKit attention, Dock badge, and notifications | No concrete follow-up proposal; code comments call it remaining integration work | Blocks readiness; partial conformance | `EscalationReadAdapter.swift` lines 19-24 |
| Full proposal macOS component matrix, including command rows, MenuBarExtra, focus/tab-order behavior, contrast assets, and multi-window/scene restoration proof | No concrete follow-up proposal | Blocks readiness; partial conformance | Proposal lines 68-215; Swift components/tests cover only a subset |
| Metric semantics and surfaces for SLO histograms/rates, including `provider_session_kill_latency_seconds`, false escalation, time-to-success, dwell, outage credit, and threshold trending | Release evidence wording exists, but no concrete follow-up proposal path | Blocks readiness; partial conformance | `metrics.rs` declares names and increments counters for event kinds only |
| Live release/operational evidence: remote visual soak, long-run metrics, migration/recovery drill pack | Proposal says release evidence may add these, but no separate owner artifact was found | Blocks broad release readiness; not all implementation conformance | rollout fixture next steps |
| Target proposal/spec drift | No owner | Blocks closeout readiness | Source/target proposal diff |

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 9 |
| Partially Implemented | 6 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

Overall conformance: Partial. The core Rust control-plane implementation and same-tree proposal gate are now strong, but the audited proposal still contains explicit UI integration, metric semantics, report/release evidence, and rollout proof commitments that are only partially satisfied.

## Detailed REQ Audit

### REQ-001: `escalation_policy_v1` schema and strict compile validation

- Source: Proposal goals and policy schema, lines 47-49 and 296-316.
- Status: Implemented.
- Evidence types: code, migration, tests-run.
- Evidence references: `workflow/src/escalation_policy.rs`; `workflow/tests/proposal_058_escalation_policy_schema.rs`; `engine/tests/proposal_058_escalation_schema.rs`; P058 gate passed.
- Implementation mapping: YAML parser, trigger/tier enums, unknown-field rejection, backend-profile validation, unsafe-stage compile rejection, and policy-hash tests are covered by the gate.
- Gap/note: None for the compile/schema slice.

### REQ-002: Frozen policy truth in run plan and backend-profile id resolution

- Source: Proposal goals lines 47 and 50; architecture line 250.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `workflow/src/compiler.rs`; `workflow/src/plan.rs`; `engine/tests/proposal_058_claim_start.rs`; P058 gate passed.
- Implementation mapping: the execution path resolves policies from frozen run snapshots and uses backend profile ids rather than hardcoded model names.
- Gap/note: None found in the audited evidence.

### REQ-003: Durable ledger, metadata, event journal, idempotency, and no-overlap invariant

- Source: Proposal goals line 51; defaults/idempotency/no-overlap lines 271-273; persistence lines 274-295.
- Status: Implemented.
- Evidence types: migration, code, tests-run.
- Evidence references: migrations `063_p058_escalation_schema.sql` lines 7-82 and `065_p058_escalation_idempotency.sql` lines 1-18; `engine/tests/proposal_058_claim_start.rs`; `db/tests/proposal_058_runtime_facts.rs`; P058 gate passed.
- Implementation mapping: tables, redaction-version event journal, ledger uniqueness, execution-metadata idempotency, concurrent claim tests, and exact-one claim/start tests are present.
- Gap/note: The no-overlap invariant is primarily proven by persistence uniqueness and focused scheduler tests, not by a live multi-provider stress drill.

### REQ-004: Scheduler-owned ordered tier advancement across same-backend retry, backend-profile escalation, lead mediation, and terminal pause

- Source: Proposal summary/goals lines 21, 48, 54; rollout Phase 2/3 lines 471-475.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `engine/src/shadow_escalation.rs` lines 61-215; `engine/src/orchestrator.rs` scheduler pause/retry paths; `orchestrator::tests::proposal_058_lead_mediation_tier_resolves_system_lead_from_frozen_catalog_snapshot`; P058 gate passed.
- Implementation mapping: completion classification advances the durable ledger and the orchestrator consumes durable tier state for retry/escalation/pause.
- Gap/note: The implementation evidence is focused test coverage rather than a live external-provider run.

### REQ-005: Full typed trigger vocabulary and runtime classification

- Source: Proposal goals line 49; provider classifier contract lines 317-331.
- Status: Partially Implemented.
- Evidence types: code, tests-run.
- Evidence references: `shadow_escalation.rs` lines 21-49 maps quota, transport, contract-output, and stale/no-output classes; `workflow/src/escalation_policy.rs` and workflow tests cover YAML vocabulary for repeated digest and loop budget.
- Implementation mapping: schema-level vocabulary exists and runtime classifier covers the primary provider/output classes.
- Gap/note: runtime firing of `repeated_same_blocker_digest` and `loop_budget_threshold` is not proven by the classifier evidence; they appear as schema/digest mappings, not complete runtime trigger paths.

### REQ-006: Fail-closed operational controls, pause reasons, and runbook anchors

- Source: Proposal goals lines 53-54; defaults lines 253-273; hold conditions lines 422-427; pause reason catalog lines 734+.
- Status: Implemented.
- Evidence types: code, tests-run, docs.
- Evidence references: `engine/src/orchestrator.rs` lines 3309-3420 for deadline, launch storm, capacity, and force-detach pauses; runbook files under `docs/runbooks/escalation/`; P058 gate passed.
- Implementation mapping: kill switch, deadline, launch recycle storm, capacity probe, force-detach, pause reason, operator hint, and runbook anchor behavior are covered by focused tests.
- Gap/note: Runtime policy override `in_flight_toggle_behavior` was not deeply inspected beyond rollout/readback evidence.

### REQ-007: Shutdown drain force-detach replay and late-frame journaling

- Source: Proposal summary line 24; recovery contract line 330; migration evidence plan lines 725-730.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `engine/src/recovery.rs` lines 405-433 and 1066-1212; `engine/tests/proposal_058_claim_start.rs` lines 3027-3267; `engine/src/executor.rs` lines 9636-9698; `db/tests/proposal_058_runtime_facts.rs` lines 306-448.
- Implementation mapping: startup repair detects running escalation executions, commits failed execution/runtime facts/paused ledger/event/stage/run updates transactionally, cancels pending InvokeAgent relaunch, and journals late frames with redacted event payloads.
- Gap/note: Live SIGTERM drain/soak was not executed. The proposal allows live operator-restart soak as release evidence, but it remains readiness evidence for broad rollout.

### REQ-008: GraphQL and MCP raw-string escalation readback parity with authorization

- Source: Proposal goals line 52; rollout readback fields lines 378-402; wire contracts lines 355-366 in reference docs.
- Status: Implemented.
- Evidence types: code, tests-run, API.
- Evidence references: `graphql-server/src/types/escalation.rs` lines 12-305; `graphql-server/tests/proposal_058_runtime_facts.rs` lines 713-875; `mcp-server/src/tools/runs.rs` lines 1089-1248 and 2055-2410; P058 gate passed.
- Implementation mapping: GraphQL and MCP expose raw strings, capped arrays, event-derived retry/drift/trace fields, digest inputs, and summary-only non-Operator readback.
- Gap/note: No live daemon request was executed during this audit; the focused tests are the proof.

### REQ-009: Report, release receipt, and rollout readback lanes

- Source: Proposal rollout readback lanes and operator report fields lines 378-421.
- Status: Partially Implemented.
- Evidence types: config, tests-run, fixture.
- Evidence references: `docs/evidence/rollout-contract/operator-readback/p058-full-surface.fixture.json` lines 1-107; P058 gate includes MCP/GraphQL tests and fixture/docs checks.
- Implementation mapping: the fixture now reports `pass` / `release` for run_report, MCP, release_receipt, and GraphQL lane shapes.
- Gap/note: the audit did not verify live generated run reports or release receipts from a daemon execution; this is fixture and focused-readback proof, not live end-to-end report generation.

### REQ-010: Rollout contract, rollback disposition, and canonical proposal gate

- Source: Proposal rollout contract lines 341-457.
- Status: Implemented.
- Evidence types: config, tests-run.
- Evidence references: `scripts/test-gate.sh` lines 4829-4872; `docs/reference/test-gates.md` lines 1373-1402; rollout fixture lines 7-30; P058 gate passed.
- Implementation mapping: the gate covers Swift read surface tests, Rust schema/runtime/API tests, metric inventory declaration, payload validation, and cargo checks; rollback disposition is present in the fixture.
- Gap/note: The gate is focused and canonical for P058, not a full repository regression suite.

### REQ-011: Metrics inventory, emission, and surfaces

- Source: Proposal metrics lines 356-377 and 480-557.
- Status: Partially Implemented.
- Evidence types: code, tests-run, docs.
- Evidence references: `db/src/metrics.rs` lines 95-115 declares all 19 names; lines 149-205 map event kinds to counters; `docs/reference/rust-control-plane.md` line 720 states durable ledger/event emission; P058 gate passed metric-name test.
- Implementation mapping: required metric names are declared, ledger/event-backed counters are incremented for several escalation event kinds, and tests cover declaration plus some event emission.
- Gap/note: several proposal metrics require rates/histograms/SLO samples or operator adjudication (`provider_session_kill_latency_seconds`, `time_to_success_after_escalation_seconds`, `false_escalation_rate`, dwell/share metrics). The inspected implementation increments counters for event kinds and does not prove full histogram/rate semantics or GraphQL/report metric surfaces.

### REQ-012: Governed macOS authority boundary and forward-compatible DTOs

- Source: Proposal macOS authority boundary lines 87-93 and non-goal line 62.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `EscalationReadAdapter.swift` lines 4-17; `EscalationState.swift` lines 5-127; `Proposal058Tests.swift` lines 12-209; P058 gate passed 16 Swift tests.
- Implementation mapping: the adapter is MainActor-published, registry-keyed by run id, DTOs decode raw strings, and Swift tests prove no local active escalation is synthesized from claim-start ledgers.
- Gap/note: Authority comments are strong, but subscription source wiring is still remaining integration work.

### REQ-013: Governed macOS visual/read surface component matrix

- Source: Proposal UI notes lines 65-246.
- Status: Partially Implemented.
- Evidence types: code, tests-run.
- Evidence references: `EscalationReadSurfaceViews.swift` lines 1-424; `Proposal058Tests.swift` lines 213-290.
- Implementation mapping: status capsule, banner stack, lineage view, pause card, trace timeline, drift review sheet, trace pasteboard copy, and inspector are implemented enough to compile and construct from adapter snapshots.
- Gap/note: the proposal also commits command presentation, MenuBarExtra, precise focus/tab order, contrast asset fixtures, scene restoration, multi-window sharing proof, Dock badge aggregation, attention requests, and notification constraints. These were not proven in code or runtime UI evidence.

### REQ-014: macOS live integration, read-pipeline states, notifications, Dock badge, and runbook actions

- Source: Proposal authority/components/fixtures/notifications lines 88, 195-215, and read-pipeline states lines 216-231.
- Status: Partially Implemented.
- Evidence types: code, tests-found, tests-run.
- Evidence references: `EscalationReadAdapter.swift` lines 19-24 explicitly lists remaining GraphQL subscription/transport-stale handling, readback refreshes, runbook URL opening, AppKit attention, Dock badge updates, and notifications.
- Implementation mapping: a presentation component skeleton exists; tests validate DTO, snapshot, constructibility, accessibility summary, and pasteboard copy.
- Gap/note: the live app integration and the proposal's full operator interaction/accessibility matrix remain unproven.

### REQ-015: Migration, rollback, and operational drill evidence

- Source: Proposal migration evidence plan lines 725-732; rollback lines 428-435.
- Status: Partially Implemented.
- Evidence types: migration, tests-run, docs.
- Evidence references: migrations `063`-`065`; `docs/reference/escalation-policies.md` lines 317-324; rollout fixture next steps lines 27-30.
- Implementation mapping: migrations exist, rollback mode is data-preserving, and focused startup replay tests cover the core shutdown-drain recovery invariant.
- Gap/note: populated migration drill, live recovery drill pack, and release evidence pack were not produced or inspected.

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial | explicit macOS integration and metric semantics are not complete | Medium-High |
| Rust architecture | Pass with minor residual risk | broad dirty diff and no live daemon run | High |
| Rust reliability | Pass with residual release-evidence risk | live shutdown/operator-restart soak not executed | Medium-High |
| API contract | Pass with report-lane caveat | GraphQL/MCP proven, live run_report/release_receipt not exercised | Medium |
| macOS UI | Partial | component-only/read-surface integration gap | High |
| Observability/rollout | Partial | release fixture overstates readiness relative to remaining metrics/drill evidence | High |
| Implementation readiness | Not Ready | broad release/readiness blockers remain despite green proposal gate | High |

## Routed Specialist Findings

### READY-001: Target proposal copy diverges from the audited proposal

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-007, REQ-010, REQ-015
- Evidence types: diff, proposal
- Evidence references: source/target proposal diff removed `shutdown drain` in summary and changed Phase 3 shutdown-drain scope.
- Why it matters: closeout cannot treat the implementation branch as proposal-aligned while the branch carries a different proposal contract than the user-supplied audited proposal.
- Recommended action: sync or remove the target worktree's divergent proposal copy, then re-run the audit against the single intended contract.
- Acceptance criteria: `diff -u <source proposal> <target proposal>` is empty or the user explicitly asks to audit the changed target proposal.

### UI-001: Governed macOS surface is not fully wired to the live app contract

- Reviewer: `macos_ui_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-013, REQ-014
- Evidence types: code, tests-run
- Evidence references: proposal lines 68-215; `EscalationReadAdapter.swift` lines 19-24; `EscalationReadSurfaceViews.swift` lines 1-424; `Proposal058Tests.swift` lines 213-290.
- Why it matters: the proposal requires more than constructible views. Operators need read-pipeline states, shared subscriptions, runbook actions, Dock badge/attention/notifications, command/menu affordances, and accessibility/focus behavior to trust the escalation state.
- Recommended action: wire the adapter into the run-detail/menu/notification surfaces, implement the remaining adapter capabilities, and add UI/runtime proof for the full component matrix.
- Acceptance criteria: proposal fixtures/tests cover subscription establishing, decode, transport disconnected, stale snapshot, ready states, Dock badge, requestUserAttention, MenuBarExtra, command rows, focus/tab order, scene restoration, multi-window sharing, and remote runtime/screenshot evidence.

### OPS-001: P058 metric implementation is not semantically complete

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-011, REQ-015
- Evidence types: code, tests-run
- Evidence references: proposal lines 356-377 and 480-557; `db/src/metrics.rs` lines 95-115 and 149-205; `rg` found several required metric names only in `metrics.rs` tests/mappings.
- Why it matters: the rollout plan gates broader adoption on false escalation rate, tier success, shadow match, latency regression, and other metric decisions. Counter-name presence is not enough to make those rollout decisions defensible.
- Recommended action: implement producers and readback/report surfaces for the rate/histogram/SLO metrics, including samples/labels needed for threshold decisions, and add tests that verify values not just names.
- Acceptance criteria: proposal gate or a follow-up gate proves metric samples for force-detach latency, time-to-success, false escalation, dwell/share, outage credit, shadow match, and report/GraphQL surfaces.

### OPS-002: Rollout fixture says release while still carrying release-readiness next steps

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-009, REQ-010, REQ-015
- Evidence types: fixture, docs
- Evidence references: rollout fixture lines 7-30.
- Why it matters: an operator decision surface that reports `release` should not simultaneously depend on unowned evidence work for remote visual/runtime soak, long-run threshold trending, and migration/recovery drills without a concrete release owner or waiver.
- Recommended action: either downgrade the rollout decision to hold until the evidence exists, or attach explicit owner/waiver/follow-up artifacts that make those next steps non-blocking for implementation closeout.
- Acceptance criteria: fixture, report lane, and reference docs agree on whether the remaining evidence is a blocker, waiver, or named follow-up.

### API-001: Report/release receipt parity is fixture-proven, not live end-to-end proven

- Reviewer: `api_contract_reviewer`
- Severity: Minor
- Confidence: Medium
- Related REQs: REQ-009
- Evidence types: tests-run, fixture
- Evidence references: GraphQL/MCP readback tests passed; rollout fixture contains run_report/release_receipt lanes.
- Why it matters: GraphQL/MCP live readback is strong, but proposal readback lanes also include operator reports and release receipts. Fixture shape alone can miss serialization or generation drift in live report-producing paths.
- Recommended action: add a focused report/release receipt generation test or daemon integration proof that consumes real escalation ledger/events and emits the same rollout fields.
- Acceptance criteria: same-tree test or evidence artifact proves live run_report and release_receipt generation for P058.

### REL-001: Live shutdown/operator-restart soak remains unexecuted

- Reviewer: `rust_reliability_reviewer`
- Severity: Minor
- Confidence: Medium
- Related REQs: REQ-007, REQ-015
- Evidence types: tests-run, docs
- Evidence references: `engine/tests/proposal_058_claim_start.rs` lines 3027-3267; `docs/reference/escalation-policies.md` lines 317-324.
- Why it matters: the startup replay implementation is now covered, but the proposal's operational confidence also depends on operator restart behavior under real daemon shutdown timing.
- Recommended action: keep the focused implementation tests, then add a live SIGTERM/restart evidence pack before broad rollout.
- Acceptance criteria: evidence shows no InvokeAgent relaunch, paused ledger, runtime facts, failed stage, blocked run, `force_detach_replay`, and late-frame metrics under an operator restart.

## Readiness Checklist

| Check | Status | Notes |
| --- | --- | --- |
| Canonical proposal gate on audited tree/HEAD | Passed | `./scripts/test-gate.sh proposal-058` passed on target worktree HEAD `ce9e7e8...` |
| Swift build/focused tests | Passed | 16 `Proposal058Tests` passed |
| Rust domain/db/engine/workflow/API tests | Passed | P058 gate passed all focused cargo tests and cargo checks |
| GraphQL/MCP readback parity | Passed | Focused tests passed for live parity fields, event payload round-trip, auth redaction, and caps |
| Core runtime recovery tests | Passed | startup force-detach replay and late-frame transaction tests passed |
| macOS live UI runtime/screenshot/remote proof | Not provided | Required for complete UI readiness |
| Empty/loading/error/offline/read-pipeline UI states | Partial | Proposal defines states; adapter still lists transport-stale/subscription work as remaining |
| Accessibility/focus/contrast/keyboard proof | Partial | accessibility summary test exists; full focus/tab/contrast fixtures not found |
| Metrics values and threshold decision proof | Partial | metric inventory and event counters exist; full SLO/rate/histogram surfaces not proven |
| Migration/recovery/live drill evidence | Partial | migrations and startup recovery tests exist; populated migration and live restart drill evidence not inspected |
| Proposal source consistency | Failing | target proposal copy diverges from audited proposal |
| Broad full regression | Not run | Only canonical P058 proposal gate was run |

## Verification Log

| Command / inspection | Result |
| --- | --- |
| `report_path.py docs/proposals/058-configurable-agent-escalation-chains.md` | Produced `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R3.md` |
| `discover_prior_review.py docs/proposals/058-configurable-agent-escalation-chains.md` | No prior proposal-review artifacts found |
| `git rev-parse HEAD` in target worktree | `ce9e7e825cb3777e89c5cb08b619dd0aa863d033` |
| `git merge-base origin/main HEAD` in target worktree | `3a93e76332512fc07e8b7bec50882ee83d703c2f` |
| `git status --short` in target worktree | Dirty tree with broad modified Rust/Swift/docs/migrations and untracked proposal/view files |
| `diff -u` source proposal vs target proposal copy | Differences found around shutdown-drain wording |
| `rg` for shutdown/replay/late-frame/no-overlap/metrics evidence | Found runtime code/tests for startup replay and late frames; metric evidence is primarily declarations/event counters |
| `rg` for macOS read-surface components and integration | Components/tests found; adapter lists remaining integration work |
| `./scripts/test-gate.sh proposal-058` | Passed. Included Swift `Proposal058Tests` (16 tests), Rust domain/db/engine/workflow/graphql/mcp focused tests, metric-name test, payload validation tests, and cargo checks for engine/graphql-server/mcp-server. Warnings were dead-code/lifetime warnings plus a Swift main-actor warning, not gate failures. |

## Final Verdict

Overall conformance: Partial.

Overall implementation readiness: Not Ready.

Audit confidence: Medium-High.

The implementation has moved materially beyond the prior hold state: the canonical P058 gate is green, the Rust control-plane core now covers durable tier advancement, readback parity, provider force-detach fail-closed behavior, launch-storm pauses, startup force-detach replay, late-frame journaling, redaction validation, and focused macOS read components. However, the proposal's remaining macOS live integration, full UI/accessibility matrix, metric semantics, live report/release evidence, operational drill evidence, and proposal-copy consistency are not yet clean enough for an Implemented/Ready verdict.

Recommended next actions:

1. Resolve target proposal drift before closeout.
2. Finish and prove the macOS live read surface integration, including subscription/stale states, runbook actions, Dock badge, attention, notifications, MenuBarExtra/command rows, focus/contrast, and remote runtime evidence.
3. Upgrade metric implementation from name/event-counter proof to proposal-level rate/histogram/SLO/report surfaces.
4. Add live report/release receipt generation proof and migration/recovery drill evidence, or record explicit owner/waiver/follow-up artifacts.
5. Re-run `./scripts/test-gate.sh proposal-058` and re-audit once those blockers are addressed.
