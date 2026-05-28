# Proposal 058 Implementation Audit R4

## Metadata

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Proposal revision | `p058-r14-2026-05-07` |
| Audit timestamp | `2026-05-28T11:43:17Z` |
| Audit mode | `proposal-implementation-audit` |
| Audited target | `main` |
| Audited HEAD | `d67161ef91417552b2494e402e8b5d4d51a99e8f` |
| Compare base | `origin/main` merge-base = `d67161ef91417552b2494e402e8b5d4d51a99e8f` |
| Worktree status before report write | Clean |
| Report path | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R4.md` |

## Implementation Target / Compare Base

The audit target is the repository `main` worktree at HEAD `d67161ef91417552b2494e402e8b5d4d51a99e8f`. The merge-base with `origin/main` is the same SHA, so this is a same-tree audit of current main rather than a branch-diff audit.

The report path was resolved with the bundled `report_path.py` helper. Existing prior implementation audit reports R1-R3 were left untouched.

## Prior Proposal-Review Reuse Summary

Reviewer-selection reuse: **Not reused**.

The prior-review discovery helper returned no proposal-review artifacts for P058. Sibling implementation audit reports exist, but the skill requires ignoring prior `IMPLEMENTATION_AUDIT` reports for reviewer selection unless explicit comparison was requested.

## Selected Reviewers

| Reviewer | Reason |
| --- | --- |
| `chainworks_execution_truth_reviewer` | P058 changes durable Run, StageExecution, AgentExecution, escalation ledger, recovery, projection, MCP, and GraphQL truth. |
| `rust_reliability_reviewer` | Proposal centers on retries, deadlines, idempotency, force-detach, recovery replay, capacity checks, and shutdown behavior. |
| `api_contract_reviewer` | GraphQL, MCP, report/readback DTO, YAML schema, raw-string forward compatibility, and migrations are in scope. |
| `macos_ui_reviewer` | P058 explicitly specifies governed macOS SwiftUI read surfaces, focus, accessibility, Dock badge, MenuBarExtra, and command presentation. |
| `observability_rollout_reviewer` | Proposal includes metrics, rollout contract, release evidence, migration drills, rollback, and gate evidence. |

## Rejected Close Alternatives

| Reviewer | Why rejected |
| --- | --- |
| `rust_arch_reviewer` | Covered by `chainworks_execution_truth_reviewer` plus `rust_reliability_reviewer` for this run/stage/ledger architecture slice. |
| `apple_arch_reviewer` | Useful, but hard cap favored `macos_ui_reviewer` because the unresolved Apple risk is the explicit UI contract rather than broad Swift architecture. |
| `apple_ux_reviewer` | UX findings are captured under the macOS UI and readiness lenses to stay within the hard cap. |
| `rust_security_reviewer` | Security-sensitive redaction/auth checks are covered through API-contract evidence and targeted tests; no new unsafe/auth mutation surface was found. |
| `product_reviewer` | Metrics and rollout are central, but the concrete risk is instrumentation/readiness, covered by observability rollout. |

## Proposal State and Contract Summary

Proposal state: **Active / implementation-synced**, not retired. The proposal metadata status is `refined_after_write_boundary_blocker_resolved` and the document still contains implementation-grade requirements.

Core contract extracted from the proposal:

- Define repo-owned `escalation_policy_v1` with ordered tiers for `same_backend_retry`, `backend_profile`, `lead_mediation`, and `pause` (`docs/proposals/058-configurable-agent-escalation-chains.md:47`).
- Keep Rust control plane as the sole authority for policy resolution, trigger classification, tier advancement, pause/resume legality, capacity checks, persistence, recovery, and kill-switch behavior (`docs/proposals/058-configurable-agent-escalation-chains.md:21`, `:250`).
- Freeze `policy_hash`, binding, tier order, trigger vocabulary, and rollout override state into compiled run truth (`docs/proposals/058-configurable-agent-escalation-chains.md:50`).
- Persist ledger, execution metadata, runtime facts, and event journal rows with idempotency and no overlapping active tier (`docs/proposals/058-configurable-agent-escalation-chains.md:51`, `:271`).
- Expose raw-string GraphQL, MCP, report, and macOS readback (`docs/proposals/058-configurable-agent-escalation-chains.md:52`).
- Provide governed macOS read-only components, accessibility, focus, Dock badge, attention requests, command presentation, read-pipeline states, and MenuBarExtra behavior (`docs/proposals/058-configurable-agent-escalation-chains.md:68`, `:88`, `:116`, `:156`, `:206`, `:216`).
- Prove rollout through the P058 gate, metrics, readback fixture, hold conditions, migration/recovery evidence, and release evidence (`docs/proposals/058-configurable-agent-escalation-chains.md:341`, `:480`, `:725`).

## Platform / Product Scope

Apple scope: **macOS**.

Backend/service scope: **Rust control-plane service, worker/scheduler, GraphQL API, MCP API, SQLite data layer, workflow YAML compiler, rollout/telemetry**.

Product scope: operator trust and recovery: the operator should see durable escalation lineage, non-progress state, recovery instructions, and rollout evidence without macOS becoming lifecycle authority.

Leading metric: `escalation_chains_started_total`.

Guardrail metric: `false_escalation_rate`.

Decision checkpoint: Phase 4 broader adoption gates: `false_escalation_rate < 5%`, tier success `> 0.6`, shadow match `> 0.95`, primary p95 wall-clock regression `< 10%`, and 100% runbook coverage (`docs/proposals/058-configurable-agent-escalation-chains.md:480`, `docs/reference/escalation-policies.md:278`).

## Primary Implementation Flows

1. Compile catalog/workflow escalation policy into frozen RunPlan truth, rejecting unknown/unsafe/ambiguous policy declarations.
2. Claim an `InvokeAgent` item and atomically create agent execution, escalation ledger, execution metadata, runtime facts, and artifact source-generation claim.
3. Classify a failed execution, select/advance the next durable tier, schedule retry/alternate backend/lead mediation, or pause terminally with runbook/action hints.
4. Recover force-detached or shutdown-interrupted provider sessions without relaunching the same execution, and expose durable pause/readback.
5. Operator reads escalation state through GraphQL, MCP, reports, and macOS System tab read-only UI, with rollout metrics and release evidence.

## Proposal Fidelity / Divergence Inventory

### Matches

- Policy schema, strict YAML validation, policy hash freezing, and known tier/trigger vocabularies are implemented and tested in `control-plane/crates/workflow/src/escalation_policy.rs` and `control-plane/crates/workflow/tests/proposal_058_escalation_policy_schema.rs`.
- Ledger, execution metadata, event journal, redaction version, JSON validation, idempotency indexes, and chain-key uniqueness exist in migrations `076_p058_escalation_schema.sql` and `078_p058_escalation_idempotency.sql`.
- Claim/start commits escalation ledger plus execution metadata in one transaction (`control-plane/crates/engine/src/executor.rs:928`, `:1000`, `:1081`).
- Scheduler enforcement exists for kill-switch, paused/exhausted tiers, chain deadline, launch recycle storm, capacity-probe threshold, and provider force-detach (`control-plane/crates/engine/src/orchestrator.rs:3260`, `:3277`, `:3310`, `:3347`, `:3372`, `:3399`).
- Startup force-detach replay writes runtime facts, pauses ledger, cancels pending invokes, settles stage failed, and blocks the run (`control-plane/crates/engine/src/recovery.rs:1823`, `:1883`, `:1914`, `:1945`, `:1953`, `:1960`).
- GraphQL and MCP readback expose parity fields, row caps, redacted events, shadow readback, and non-Operator summary-only MCP readback (`control-plane/crates/graphql-server/src/types/escalation.rs:133`, `:218`, `:264`; `control-plane/crates/mcp-server/src/tools/runs.rs:1101`, `:1231`).
- Swift has a governed read adapter and constructible read-only components wired into the System tab (`Chainworks Forge/Engine/EscalationReadAdapter.swift:30`, `Chainworks Forge/Views/EscalationReadSurfaceViews.swift:78`, `Chainworks Forge/Views/RunsHomeView.swift:372`).
- Same-tree `./scripts/test-gate.sh proposal-058` passed.

### Divergences

- The full macOS UI contract is broader than the implemented Swift surface. The code provides an inspector and simple components, but not the full specified command-presentation mirrors, MenuBarExtra paused-run list, focus movement, contrast fixtures, retry collapse, detailed lineage columns, narrow-width behavior, shadow-row styling, or full screen-state matrix.
- The P058 metric gate line invokes `cargo test -p db proposal_058_required_metric_names_are_declared --lib`, but that filter currently lists zero tests. The metric inventory constants exist; the intended gate test does not.
- `docs/reference/escalation-policies.md` claims current head uses migrations `063`, `064`, and `065`, while current main has `076`, `077`, and `078`.
- Runtime evidence for UI remains release-closeout evidence rather than in-tree runtime/screenshot proof.

### Ambiguities / Evidence Gaps

- `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` is concrete release evidence tracking, but it is not a follow-up proposal/spec. It can own broad-release evidence collection, but it does not remove the still-present macOS UI contract from the audited proposal.
- The gate passes, but several Swift warnings point at actor isolation around P031/P058 readback mapping. This is not a current build failure, but it weakens future Swift 6 readiness.
- Metric declarations and generic event-to-metric recording exist, but the proposal's per-metric source and GraphQL/report surface claims are not fully proven by the current P058 gate.

## Residual Scope / Follow-up Ownership

| Residual item | Current owner artifact | Blocks conformance? | Blocks broad release/readiness? | Notes |
| --- | --- | --- | --- | --- |
| Remote macOS visual/runtime proof for System-tab inspector, stale/disconnected state, keyboard focus, accessibility | `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` | No for implemented runtime slice | Yes | Explicitly tracked as release evidence, not missing code path. |
| Long-running metric thresholds and provider force-detach latency trend | `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` | No for implemented runtime slice | Yes | Needed before broad release. |
| Live SIGTERM/operator restart drill and populated migration drill | `docs/evidence/058-configurable-agent-escalation-chains/release-closeout-followups.json` | No for implementation closeout | Yes | Proposal allows live SIGTERM soak as release evidence. |
| Full macOS UI contract: MenuBarExtra, command rows, focus/contrast fixtures, retry collapse, shadow rows, narrow layout, full keyboard tab order | None found as concrete follow-up proposal | Yes | Yes | Still promised in proposal; current Swift covers only a focused read surface. |
| Actual P058 metric-inventory gate test | None | Yes for metric/gate requirement | Yes | Gate filter lists zero tests for `proposal_058_required_metric_names_are_declared`. |
| Reference doc migration-number correction | None | No | Yes, minor | Current reference doc contradicts current migration sequence. |

## Requirement Summary

| Status | Count |
| --- | ---: |
| Implemented | 11 |
| Partially Implemented | 6 |
| Missing | 0 |
| Not Verifiable | 0 |
| Out of Scope | 0 |

Overall conformance is **Partial** because several explicit UI, metrics, and validation-gate commitments remain partially implemented or unproven by executable evidence.

## Detailed REQ Audit

| ID | Requirement | Proposal source | Status | Evidence | Gap / note |
| --- | --- | --- | --- | --- | --- |
| REQ-001 | Define `escalation_policy_v1` with required fields, strict validation, backend-profile ids, tier kinds, and trigger vocabulary. | Lines 47-50, 296-323 | Implemented | `control-plane/crates/workflow/src/escalation_policy.rs`; workflow tests 30 passed in gate | Strict parser and compile validation are covered. |
| REQ-002 | Rust control plane is sole authority for escalation truth and macOS remains read/subscription-only. | Lines 21-24, 87-93, 250 | Implemented | `EscalationReadAdapter.swift:13`; GraphQL/MCP readback; no Swift mutation path found | Drift sheet is read-only and routes external acknowledgement. |
| REQ-003 | Persist ledger, execution metadata, event journal, runtime facts, idempotency, and no duplicate chain creation. | Lines 51, 271, 274-295 | Implemented | Migrations `076`, `077`, `078`; `executor.rs:1000-1104`; gate claim-start tests | Atomic claim/start and uniqueness are tested. |
| REQ-004 | Support scheduler-owned tier behavior for same-backend retry, backend-profile escalation, lead mediation, and pause. | Lines 48, 31, 317-330 | Implemented | `orchestrator.rs:3202-3435`; engine tests 14 passed | Runtime slice covers current tier advancement and fail-closed pauses. |
| REQ-005 | Enforce kill switch, deadlines, capacity probe threshold, launch storm, provider force-detach, shutdown drain, and late-frame journaling. | Lines 253-273, 725-729 | Implemented | `orchestrator.rs:3260-3412`; `recovery.rs:1823-1966`; claim-start test `p058_startup_recovery_force_detaches_running_escalation_execution` | Same-tree gate covers these paths. |
| REQ-006 | Expose raw-string GraphQL/MCP/report readback with forward compatibility, row caps, redaction, and auth summary for non-Operators. | Lines 52, 378-427 | Implemented | `graphql-server/src/types/escalation.rs:12-131`; `mcp-server/src/tools/runs.rs:1087-1247`; GraphQL/MCP tests | Operator and non-Operator readback paths are tested. |
| REQ-007 | Preserve redacted evidence boundaries: no raw evidence in trace/readback; validate payload JSON and redaction version. | Lines 129, 153-155, 206-214 | Implemented | `db/src/repos/escalation.rs` payload validation tests; schema tests 60 passed | Strong negative tests cover secret/path/URL-shaped payloads. |
| REQ-008 | Swift adapter is shared per run and publishes immutable snapshots from decoded DTOs. | Lines 88-92 | Implemented | `EscalationReadAdapter.swift:20-36`, `:119-139`; `Proposal058Tests.swift:155-232` | Registry and snapshot behavior are unit-tested. |
| REQ-009 | Provide the full governed macOS component and layout contract. | Lines 94-205, 232-246 | Partially Implemented | `EscalationReadSurfaceViews.swift:78-424`; `RunsHomeView.swift:372-380`; Swift P058 tests construct components | Missing/unproven: command presentation mirrors, MenuBarExtra, focus movement, contrast fixture, retry collapse, shadow rows, detailed columns, narrow layout, screen-state matrix. |
| REQ-010 | Dock badge and human-tier attention derive from live paused escalation aggregation and are not click-handler driven. | Lines 156-161, 201-214 | Partially Implemented | `EscalationReadAdapter.swift:75`; `NotificationService.swift:111-215` | Per-snapshot count and generic notification service exist, but no P058 live cross-run aggregation, attention cancellation-token proof, or MenuBarExtra implementation was found. |
| REQ-011 | Read-pipeline states render skeleton/stale/disconnected/ready behavior and disable commands appropriately. | Lines 216-231 | Partially Implemented | `EscalationReadAdapter.swift:38-68`; `Proposal058Tests.swift:202-216` | Adapter states exist for stale/disconnected/decodeFailed, but UI presentation states, command disablement, skeleton/dimmed rendering, and cached trace rules lack runtime/UI evidence. |
| REQ-012 | Metrics inventory and production metric emission cover all proposal metrics and surfaces. | Lines 356-377, 480-557 | Partially Implemented | `db/src/metrics.rs:136-156`, `:367-462`; `release-closeout-followups.json:21-30` | Constants and generic event recording exist; long-run threshold evidence is deferred; actual gate test for P058 metric inventory is absent. |
| REQ-013 | Rollout contract readback fixture includes required lanes, fields, pass status, rollback disposition, next steps, and release-followup ownership. | Lines 341-457 | Implemented | `docs/evidence/rollout-contract/operator-readback/p058-full-surface.fixture.json:1-130` | Fixture is present and complete enough for readback review. |
| REQ-014 | Migration/recovery evidence proves startup force-detach replay and no InvokeAgent relaunch. | Lines 725-732 | Implemented | `control-plane/crates/engine/tests/proposal_058_claim_start.rs:3027-3267`; gate passed | Live SIGTERM drill remains release evidence. |
| REQ-015 | Gate documentation and script define the canonical P058 proving path. | Lines 30, 348-352 | Partially Implemented | `scripts/test-gate.sh:5180-5223`; `docs/reference/test-gates.md:1471-1518` | The canonical gate passes, but the P058 metric subfilter lists zero tests. |
| REQ-016 | Reference documentation describes current implemented system truth. | Proposal implementation sync plus doc map policy | Partially Implemented | `docs/reference/escalation-policies.md:7-12`; migrations `076-078` | Reference doc says current head uses migrations `063-065`, which is stale. |
| REQ-017 | Swift concurrency and actor ownership around macOS read surface remain clean under the audited toolchain. | Lines 88-90, 195-199 | Partially Implemented | Gate build warning: `P031ThinGraphQLReadBoundary.swift:6651` calls MainActor `EscalationSnapshot.build` from nonisolated context | Build passes today, but warning is future Swift 6 readiness debt on the P058 read path. |

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Proposal conformance | Partial | UI and metric/gate commitments are only partially proven. | High |
| Chainworks execution truth | Strong | Runtime authority and durable ledger semantics are implemented. | High |
| Rust reliability | Strong | Force-detach, recovery replay, idempotency, and fail-closed pauses have focused tests. | High |
| API contract | Strong | GraphQL/MCP parity and redaction paths have direct tests. | High |
| macOS UI | Partial | The implemented System-tab inspector is much narrower than the full UI contract. | Medium |
| Observability/rollout | Partial | Metrics/test-gate evidence and reference docs have gaps; broad-release evidence remains outstanding. | High |
| Overall readiness | Not Ready | Same-tree gate passes, but full P058 handoff still has unresolved major findings. | High |

## Routed Specialist Findings

### UI-001 - Full macOS UI contract is not implemented or runtime-proven

- Reviewer: `macos_ui_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-009, REQ-010, REQ-011
- Evidence types: proposal, code, tests-found, tests-run
- Evidence references: proposal lines 68-215 and 216-246; `EscalationReadSurfaceViews.swift:78-424`; `RunsHomeView.swift:372-380`; `Proposal058Tests.swift:266-300`
- Why it matters: The proposal does not just ask for a status component. It pins focus behavior, contrast, command presentation mirrors, MenuBarExtra, Dock badge aggregation, attention requests, retry collapse, detailed lineage layout, shadow rows, narrow-width behavior, screen states, and fixtures. Current main has a useful read-only System-tab inspector and unit-constructible components, but that does not satisfy the full UI contract.
- Recommended action: Either implement and test the remaining UI behaviors, or narrow the proposal by moving them into a real follow-up proposal/spec.
- Acceptance criteria: Remote/runtime or snapshot evidence covers the component inventory, focus order, contrast, MenuBarExtra, Dock badge aggregation, command mirrors, read-pipeline presentations, and screen-state matrix; or a named follow-up proposal owns that scope.

### OPS-001 - P058 metric inventory gate invokes a zero-test filter

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-012, REQ-015
- Evidence types: tests-run, code, config
- Evidence references: `scripts/test-gate.sh:5194`; `control-plane/crates/db/src/metrics.rs:136-156`; `cargo test -p db proposal_058_required_metric_names_are_declared --lib -- --list` returned `0 tests, 0 benchmarks`
- Why it matters: The proposal requires a metrics inventory and the gate documentation claims P058 metric inventory coverage, but the named test does not exist. A passing gate can therefore miss metric-name regression in this slice.
- Recommended action: Add a real `proposal_058_required_metric_names_are_declared` test and make the gate fail closed on zero-test filters where practical.
- Acceptance criteria: The named P058 metric test appears in `-- --list`, asserts all 19 P058 metrics, and the canonical P058 gate runs it.

### OPS-002 - Reference doc has stale P058 migration numbers

- Reviewer: `observability_rollout_reviewer`
- Severity: Minor
- Confidence: High
- Related REQs: REQ-016
- Evidence types: code, docs
- Evidence references: `docs/reference/escalation-policies.md:11-12`; `control-plane/crates/db/migrations/076_p058_escalation_schema.sql`; `077_p058_escalation_redaction_version.sql`; `078_p058_escalation_idempotency.sql`
- Why it matters: `docs/reference/` is canonical implemented-system truth in this repository. It currently points operators and implementers to migration numbers that do not exist on main for P058.
- Recommended action: Update the reference doc to use `076`, `077`, and `078`, or describe the migration-number renumbering if historical context matters.
- Acceptance criteria: Reference docs match current migration filenames and `rg "063_p058|064_p058|065_p058" docs/reference/escalation-policies.md` returns no current-head claim.

### READY-001 - Broad-release evidence remains outstanding

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-009, REQ-012, REQ-014
- Evidence types: proposal, config
- Evidence references: `release-closeout-followups.json:5-43`; `p058-full-surface.fixture.json:27-53`
- Why it matters: The follow-up artifact explicitly says remote UI soak, metric threshold trending, and operational drills are non-blocking for implementation closeout but blocking for broad release. This is a legitimate separation, but it means full release readiness is not achieved yet.
- Recommended action: Treat P058 as implementation-closeout ready only for the proven runtime slice; do not mark broad user-facing release complete until these follow-ups are collected.
- Acceptance criteria: The release evidence pack contains remote UI/runtime proof, long-running metric threshold trends, and operational drill artifacts, or a follow-up proposal changes the release requirement.

### ARCH-001 - P058 readback mapping has a Swift actor-isolation warning

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Minor
- Confidence: Medium
- Related REQs: REQ-008, REQ-017
- Evidence types: tests-run, code
- Evidence references: Gate warning for `P031ThinGraphQLReadBoundary.swift:6651`; `EscalationSnapshot.build` in `Chainworks Forge/Models/EscalationState.swift:175`
- Why it matters: The code currently builds, but the compiler warns that `EscalationSnapshot.build` is MainActor-isolated and called from a synchronous nonisolated context. P058 explicitly cares about off-MainActor decode and MainActor publication.
- Recommended action: Make the actor boundary explicit: move mapping onto MainActor, mark the pure snapshot builder nonisolated if safe, or split pure DTO-to-snapshot derivation from MainActor publication.
- Acceptance criteria: P058 gate still passes and no P058/P031 readback actor-isolation warning remains.

## Readiness Checklist

| Item | Status | Evidence |
| --- | --- | --- |
| Canonical build/gate | Passed | `./scripts/test-gate.sh proposal-058` passed on audited HEAD. |
| Core Rust service flow validation | Passed | Domain, engine, db, workflow, GraphQL, MCP P058 tests passed. |
| macOS UI compile/focused tests | Passed with warnings | Swift P058 suite ran 20 tests and passed. |
| UI runtime/screenshot proof | Partial | Release-closeout follow-up owns remote UI soak. No runtime screenshot collected in this audit. |
| Empty/loading/error/offline/permission states | Partial | Read adapter states exist; full UI presentation state matrix is not proven. |
| Accessibility/focus/keyboard | Partial | Raw accessibility summary tested; focus order, contrast, Full Keyboard Access, and runtime VoiceOver/focus behavior not proven. |
| Localization/privacy/permissions/entitlements | Partial | Privacy/redaction strong on Rust/API; macOS notification/attention P058-specific permissions not proven. |
| Security/redaction | Passed | Payload validation, redaction version, non-Operator MCP summary tests passed. |
| Migration/recovery | Passed for implementation tests | Startup force-detach replay test passed; live drills remain release evidence. |
| Metrics/rollout | Partial | Metric constants and event recording exist, but P058 metric gate test is absent and long-run threshold evidence is pending. |
| Full same-tree regression/canonical gate | Passed | Same-tree `proposal-058` gate passed, but one metric subfilter listed zero tests. |

## Verification Log

| Command / inspection | Result |
| --- | --- |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py docs/proposals/058-configurable-agent-escalation-chains.md` | Resolved R4 report path. |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py docs/proposals/058-configurable-agent-escalation-chains.md` | No proposal-review artifacts found. |
| `git rev-parse HEAD` | `d67161ef91417552b2494e402e8b5d4d51a99e8f`. |
| `git status --short` before report write | Clean. |
| Code/doc inspections with `rg`, `nl`, and targeted file reads | Mapped proposal requirements to Swift, Rust, GraphQL, MCP, migrations, docs, and evidence files. |
| `./scripts/test-gate.sh proposal-058` | Passed. Swift P058 suite: 20 tests passed. Rust/domain, engine, db, GraphQL, MCP, workflow, and cargo check subcommands passed. |
| `cd control-plane && cargo test -p db proposal_058_required_metric_names_are_declared --lib -- --list` | Returned `0 tests, 0 benchmarks`; confirms the P058 metric-name gate filter has no matching test. |

## Final Verdict and Recommended Actions

Overall Conformance: **Partial**.

Overall Implementation Readiness: **Not Ready** for full P058/broad release. The Rust control-plane, persistence, recovery, GraphQL/MCP readback, and focused Swift read-surface slice are substantially implemented and the canonical same-tree gate passes. The full proposal cannot be closed as implemented because explicit macOS UI, metric validation, and documentation/readiness commitments remain partial.

Reviewer Selection Reuse: **Not reused**.

Audit Confidence: **High** for Rust/API/data/rollout findings; **Medium** for macOS runtime behavior because no remote UI/runtime screenshots were collected.

Recommended next actions:

1. Implement or formally move the remaining macOS UI contract into a concrete follow-up proposal/spec: MenuBarExtra, command rows, focus/contrast fixtures, attention lifecycle, Dock badge aggregation, detailed lineage behavior, shadow rows, narrow layout, and full screen-state matrix.
2. Add and run a real `proposal_058_required_metric_names_are_declared` test; update the gate so the metric inventory is actually executable evidence.
3. Update `docs/reference/escalation-policies.md` migration numbers to match current main (`076`, `077`, `078`).
4. Resolve the P031/P058 Swift actor-isolation warning on `EscalationSnapshot.build`.
5. Keep the release-closeout follow-ups as broad-release blockers until remote UI soak, metric-threshold trends, and operational drills are collected.
