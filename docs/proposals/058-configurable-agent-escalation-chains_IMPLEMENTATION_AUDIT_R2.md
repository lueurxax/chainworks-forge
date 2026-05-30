# Proposal 058 Implementation Audit R2

## Verdict

| Field | Result |
|---|---|
| Overall Conformance | Not Implemented |
| Overall Implementation Readiness | Not Ready |
| Reviewer Selection Reuse | Not reused |
| Audit Confidence | High for readiness blockers; Medium-High for remaining runtime/UI/API scope |

R2 supersedes R1 for the current target state. Since R1, the implementation target has added governed macOS read-surface components, focused Swift tests, GraphQL/MCP non-null scheduler readback parity tests, provider force-detach fail-closed pauses, and launch-recycle storm pauses. The canonical P058 gate now passes across Swift and control-plane checks.

The full proposal still is not complete. The current rollout fixture remains `hold` because shutdown drain/replay, late-frame handling, no-overlap drill evidence, live daemon/report parity, remote UI proof, metrics semantics, migration drill, shutdown drain drill, and recovery artifact drills remain incomplete or unproven.

## Metadata

| Field | Value |
|---|---|
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Proposal revision | `p058-r14-2026-05-07` |
| Report | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R2.md` |
| Audit timestamp | `2026-05-27T17:51:00Z` |
| Source proposal tree | `/Users/user/Documents/Chainworks Forge` |
| Implementation target | `/Users/user/Documents/Chainworks Forge/.chainworks/worktrees/cw-configurable-agent-escalation-6764a0c2` |
| Target branch | `cw/configurable-agent-escalation/6764a0c2` |
| Target HEAD | `ce9e7e825cb3777e89c5cb08b619dd0aa863d033` |
| Compare base | `origin/main...HEAD`, merge base `3a93e76332512fc07e8b7bec50882ee83d703c2f` |
| Target worktree status | Dirty; staged, unstaged, and untracked target changes included in audit scope |
| Report path source | `/Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py` |

## Implementation Target / Compare Base

The target worktree now contains an untracked copy of `docs/proposals/058-configurable-agent-escalation-chains.md`. It is byte-identical to the source proposal used for report path generation. This report is written beside the source proposal, while auditing the specified implementation worktree.

Proposal state: `Active` for this audit. The target rollout fixture is still `hold`, so P058 is not ready for closeout or retirement as fully implemented.

## Prior Proposal-Review Reuse

Reviewer-selection reuse: `Not reused`.

No prior P058 proposal-review artifacts were found by the helper discovery script. Existing P058 implementation audit reports were ignored for reviewer selection as required.

## Selected Reviewers

| Reviewer | Reason |
|---|---|
| `chainworks_execution_truth_reviewer` | Durable run/stage/agent execution truth, recovery, MCP readback, and run-report truth are central. |
| `rust_reliability_reviewer` | Scheduler tier advancement, retry-after, capacity probes, force-detach, launch storm, idempotency, shutdown, replay, and recovery are central. |
| `api_contract_reviewer` | GraphQL, MCP, report, YAML schema, raw-string vocabulary, and auth redaction are explicit contracts. |
| `observability_rollout_reviewer` | Metrics, rollout phases, fixture gates, migration drills, rollback, and decision gates are explicit. |
| `macos_ui_reviewer` | Governed macOS read surfaces, accessibility, notification, dock, pasteboard, and menu-bar behavior are explicit. |

Rejected close alternatives:

- `apple_arch_reviewer`: relevant, but the concrete client risk is UI/read-surface completeness and was covered by `macos_ui_reviewer`.
- `rust_arch_reviewer`: relevant, but execution truth plus Rust reliability better matched the remaining backend risk.
- `rust_security_reviewer`: auth/redaction/path validation were inspected through API and execution-truth lenses; no new dominant security blocker required a separate reviewer.
- `product_reviewer`: rollout metrics and decision gates were reviewed through observability/rollout.

## Proposal Contract Summary

Platform/product scope:

- Apple: macOS.
- Backend/service: Rust control-plane service, worker/scheduler, API, data, rollout, and cross-stack readback scope.

Locked decisions:

- `escalation_policy_v1` is repo-owned YAML using `backend_profile` ids.
- Rust is the only authority for policy resolution, trigger classification, tier advancement, pause/resume legality, capacity checks, persistence, recovery, and kill-switch behavior.
- macOS is read/subscription-only and must not mutate escalation lifecycle truth.
- Run snapshots freeze policy hash, digest version, binding, tier order, trigger vocabulary, and rollout override state.
- Ledger, execution metadata, runtime facts, and events are durable and forward-compatible.
- Rollout requires Phase 0-4 gates, metrics, readback parity, UI proof, migration, shutdown, recovery, and rollback evidence.

Primary service/user flows:

1. Compile workflow/catalog policy with strict validation and frozen policy truth.
2. Start/claim agent execution with durable escalation ledger/metadata ownership.
3. Classify failed executions and advance ordered tiers without overlapping active attempts.
4. Read escalation state through GraphQL/MCP/report/macOS with raw-string compatibility and principal redaction.
5. Operate rollout/recovery through kill switch, retry-after, capacity, force-detach, launch storm, shutdown drain, replay, metrics, and drills.

## Fidelity / Divergence Inventory

Matches:

- Schema compile validation, unsafe binding rejection, policy hashes, persistence schema, event redaction, idempotency, and raw-string vocabulary are implemented and covered by the P058 gate.
- Claim/start, startup recovery, retry-after claim blocking, capacity threshold, chain deadline, force-primary kill switch, provider force-detach pause, launch-recycle storm pause, lead mediation, and pause tier paths are covered by focused tests.
- GraphQL/MCP readback exposes capped ledgers/events/metas, digest inputs, non-null scheduler parity fields in focused fixtures, and non-Operator redaction.
- Governed macOS read components now exist: status capsule, banner stack, lineage view, pause card, trace timeline, drift review sheet, inspector, and pasteboard writer. Swift tests prove constructibility, raw-id accessibility summary, snapshot behavior, and pasteboard JSON/string writes.

Divergences:

- The rollout fixture remains `hold`, not `release`.
- Remaining Phase 2/3 runtime hardening still needs shutdown drain/replay, late-frame handling, and no-overlap drills.
- macOS read surface is not proven with remote visual/runtime evidence and is not wired to subscription/stale refresh, runbook opening, AppKit attention, dock badge, notifications, or menu-bar presentation.
- Metrics are declared and partially mapped, but not release-proven with authoritative producers and required rate/histogram semantics.
- Migration, live MCP/GraphQL parity, shutdown drain, and recovery artifact drills are not complete.

Ambiguities / Evidence Gaps:

- `docs/reference/rust-control-plane.md` still says Phase 2+ readback fields emit `null` until the scheduler populates them, while current GraphQL/MCP focused parity tests and rollout fixture lanes say scheduler readback parity passes. Reference docs need synchronization.
- The Swift read-surface tests construct views but do not render screenshots or exercise app navigation, focus order, narrow layout, notifications, dock badge, or subscription refresh.
- The gate is strong and canonical for P058, but it does not replace the rollout fixture's required operational drill evidence.

## Residual Scope / Follow-up Ownership

| Residual Item | Owner Proposal | Blocks Conformance/Readiness |
|---|---:|---:|
| Shutdown drain/replay, late-frame handling, and no-overlap operational drills | None found | Yes |
| Phase 3 recovery invariants and graceful shutdown drain | None found | Yes |
| Remote visual/runtime proof for governed macOS read surfaces, including narrow layout, accessibility, stale/loading/error, runbook, notification, dock, and menu-bar states | None found | Yes |
| Live daemon/report parity evidence beyond focused GraphQL/MCP unit fixtures | None found | Yes |
| Metrics emitted from authoritative producers with bounded labels and required rate/histogram semantics | None found | Yes |
| Migration drill, live MCP/GraphQL parity test, shutdown drain drill, and recovery artifact drill | None found | Yes |

No concrete follow-up proposal/spec was found owning the deferred tail. These items remain in P058 scope and block a successful conformance/readiness roll-up.

## Requirement Summary

| REQ | Requirement | Status |
|---|---|---|
| REQ-001 | Policy schema and compile validation | Implemented |
| REQ-002 | Frozen policy truth and Rust authority | Implemented |
| REQ-003 | Durable ledger, metadata, events, redaction, idempotency | Implemented |
| REQ-004 | Claim/start and Phase 0-1 runtime foundation | Implemented |
| REQ-005 | Ordered tier advancement for retry/profile/lead/pause | Implemented |
| REQ-006 | Runtime guardrails: retry-after, capacity, deadlines, kill switch, force-detach, launch storm | Partially Implemented |
| REQ-007 | Shutdown drain/replay, late-frame handling, and no-overlap drill evidence | Missing |
| REQ-008 | GraphQL/MCP/report raw-string readback and auth redaction | Partially Implemented |
| REQ-009 | Governed macOS read surfaces and UI/UX commitments | Partially Implemented |
| REQ-010 | Metrics and rollout decision gates | Partially Implemented |
| REQ-011 | Migration, live parity, shutdown, and recovery drills | Missing |
| REQ-012 | Non-goal boundaries: no hardcoded models, no unsafe side-effect escalation, no macOS write authority, data-preserving rollback | Partially Implemented |

## Detailed REQ Audit

### REQ-001 - Policy schema and compile validation

- Proposal source: Goals lines 46-49; non-goals lines 57-59; rollout Phase 0.
- Status: Implemented.
- Evidence types: proposal, code, tests-run.
- Evidence references: `control-plane/crates/workflow/tests/proposal_058_escalation_policy_schema.rs`; `control-plane/crates/engine/tests/proposal_058_escalation_schema.rs`; `./scripts/test-gate.sh proposal-058` passed.
- Implementation mapping: policy parse, hash, unknown backend profile rejection, ambiguity rejection, unsafe side-effect binding rejection, unknown applies_to rejection, and future raw-value handling are covered.
- Gap/note: None for schema/compile scope.

### REQ-002 - Frozen policy truth and Rust authority

- Proposal source: Summary lines 21-24; Goals lines 49-51.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: claim/start tests in the P058 gate; `EscalationReadAdapter.swift` lines 13-17 states macOS read-only authority boundary.
- Implementation mapping: claim/start paths use frozen plan truth, store policy/ledger ids, and keep macOS as presentation-only.
- Gap/note: Remaining macOS integration proof is tracked under REQ-009.

### REQ-003 - Durable ledger, metadata, events, redaction, idempotency

- Proposal source: Goals line 50; persistence and migration sections.
- Status: Implemented.
- Evidence types: code, migration, tests-run.
- Evidence references: migrations `063`, `064`, `065`; DB schema tests; payload-shape tests; `./scripts/test-gate.sh proposal-058` passed.
- Implementation mapping: ledger, execution metadata, event journal, redaction version, idempotency keys, and JSON allowlist validation are present.
- Gap/note: Runtime producers for all later operational events remain incomplete.

### REQ-004 - Claim/start and Phase 0-1 runtime foundation

- Proposal source: Implementation Sync lines 28-32.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: P058 claim/start tests, startup recovery tests, retry-after claim blocking tests, and Swift/GraphQL/MCP focused tests in the P058 gate.
- Implementation mapping: durable claim identity, sessionless invoke fail-closed, startup repair, quota retry-after claim blocking, and readback projection are covered.
- Gap/note: This is foundation scope, not the full P058 tail.

### REQ-005 - Ordered tier advancement for retry/profile/lead/pause

- Proposal source: Summary lines 21-22; Goals lines 47-48; Implementation Sync line 31.
- Status: Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/shadow_escalation.rs`; engine tests for scheduler selection, lead mediation, pause tier, and ledger advancement; P058 gate passed.
- Implementation mapping: completion classification writes `would_select_*`, advances the durable ledger, writes a redacted event, and lets the scheduler enqueue the next tier or pause.
- Gap/note: Later no-overlap and replay drills remain separate.

### REQ-006 - Runtime guardrails: retry-after, capacity, deadlines, kill switch, force-detach, launch storm

- Proposal source: Implementation Sync line 31; defaults/recovery sections; rollout Phase 2.
- Status: Partially Implemented.
- Evidence types: code, tests-run.
- Evidence references: `control-plane/crates/engine/src/orchestrator.rs` lines 3310-3419 for deadline, launch storm, capacity, and force-detach pauses; lines 7287-7304 for force-detach classification; P058 gate tests for capacity, deadline, kill switch, provider force-detach, launch storm, and retry-after passed.
- Implementation mapping: several fail-closed guardrails now pause before launching unsafe escalation retry.
- Gap/note: This is not yet the complete hardening story because shutdown/replay, late frames, and drill-level no-overlap proof remain open.

### REQ-007 - Shutdown drain/replay, late-frame handling, and no-overlap drill evidence

- Proposal source: Summary line 24; recovery/defaults; migration evidence; rollout Phase 2-3.
- Status: Missing.
- Evidence types: proposal, code, tests-found, tests-run.
- Evidence references: rollout fixture lines 10, 21, 39-46; `docs/reference/escalation-policies.md` lines 29, 273, 320-321.
- Implementation mapping: docs and enum/metric names exist, plus generic daemon shutdown tests exist, but P058-specific shutdown drain/replay, late-frame handling, and no-overlap drill evidence are still absent.
- Gap/note: This missing tail forces Overall Conformance to `Not Implemented`.

### REQ-008 - GraphQL/MCP/report raw-string readback and auth redaction

- Proposal source: Goals line 51; wire contracts; rollout contract.
- Status: Partially Implemented.
- Evidence types: code, schema, tests-run.
- Evidence references: GraphQL `runEscalationReadback`; MCP `runs.get`; P058 gate tests `proposal_058_graphql_run_escalation_readback_exposes_live_parity_fields`, MCP readback tests, non-Operator redaction tests; fixture lines 49-129.
- Implementation mapping: GraphQL/MCP focused parity now passes for scheduler readback fields; MCP/GraphQL lanes are `pass/release` inside the fixture.
- Gap/note: run_report still requires live daemon/report parity evidence beyond focused fixtures; reference docs still contain stale null-field wording.

### REQ-009 - Governed macOS read surfaces and UI/UX commitments

- Proposal source: macOS UI lines 66-214; Goals line 51; non-goal line 61.
- Status: Partially Implemented.
- Evidence types: code, tests-run.
- Evidence references: `EscalationReadAdapter.swift` lines 4-24; `EscalationReadSurfaceViews.swift` lines 78-424; `Proposal058Tests.swift` lines 213-290; rollout fixture lines 11, 22, 35, 43.
- Implementation mapping: read-only adapter, status capsule, banner stack, lineage view, pause card, trace timeline, pasteboard writer, drift review sheet, and inspector exist and compile. Focused Swift tests cover constructibility, raw-id accessibility summary, snapshot behavior, and pasteboard writes.
- Gap/note: The views are not found wired into live app screens outside their own inspector; subscription/stale refresh, runbook opening, AppKit attention, dock badge, notifications, menu-bar presentation, narrow-layout proof, and remote visual/runtime evidence remain pending.

### REQ-010 - Metrics and rollout decision gates

- Proposal source: Metrics section; rollout contract.
- Status: Partially Implemented.
- Evidence types: telemetry, tests-run.
- Evidence references: `control-plane/crates/db/src/metrics.rs`; fixture lines 13, 24, 44; `docs/reference/rust-control-plane.md` line 720.
- Implementation mapping: required metric names and several event mappings exist; the gate proves metric names are declared.
- Gap/note: The fixture still says metrics are not release-proven with authoritative producers, bounded labels, and rate/histogram semantics.

### REQ-011 - Migration, live parity, shutdown, and recovery drills

- Proposal source: rollout contract; migration evidence lines 724-731.
- Status: Missing.
- Evidence types: proposal, tests-run, log-or-trace.
- Evidence references: fixture lines 14, 25, 45-46; no passing drill artifacts found.
- Implementation mapping: migrations and focused tests exist, but the required operational drill artifacts are missing.
- Gap/note: This independently blocks readiness.

### REQ-012 - Non-goal boundaries

- Proposal source: non-goals lines 57-62.
- Status: Partially Implemented.
- Evidence types: code, tests-run.
- Evidence references: unsafe side-effect rejection tests; `EscalationReadAdapter.swift` lines 13-17; fixture rollback mode lines 27-30.
- Implementation mapping: observed code respects the major non-goal boundaries.
- Gap/note: Full proof across live macOS surfaces and report/live daemon readback awaits final integration evidence.

## Reviewer / Lens Scorecard

| Lens | Conformance | Top Risk | Confidence |
|---|---|---|---|
| Objective REQ audit | Not Implemented | Missing shutdown/replay/late-frame/drill evidence | High |
| Execution truth | Partial | Foundation is durable, but operational tail is still unowned | Medium-High |
| Rust reliability | Not Ready | Replay, late frames, no-overlap drills, and shutdown evidence are not complete | High |
| API contract | Partial | Focused GraphQL/MCP parity passes, but live daemon/report parity and docs sync remain open | Medium-High |
| Observability/rollout | Not Ready | Fixture remains `hold`; metrics and drills incomplete | High |
| macOS UI | Partial | Components compile, but live integration and remote visual/accessibility proof are pending | Medium-High |

## Routed Specialist Findings

### READY-001 - Rollout contract remains on hold

- Reviewer: `observability_rollout_reviewer`
- Severity: Critical
- Confidence: High
- Related REQs: REQ-007, REQ-008, REQ-009, REQ-010, REQ-011
- Evidence types: proposal, tests-run, telemetry
- Evidence references: `docs/evidence/rollout-contract/operator-readback/p058-full-surface.fixture.json` lines 3-46; `./scripts/test-gate.sh proposal-058` passed.
- Why it matters: The canonical gate is green, but the release decision artifact still blocks release.
- Recommended action: Keep P058 active until every run_report hold condition is satisfied or assigned to a concrete follow-up proposal.
- Acceptance criteria: run_report and release_receipt lanes move from `hold` to `pass/release` with no waivers.

### REL-001 - Shutdown/replay and late-frame handling remain missing

- Reviewer: `rust_reliability_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-007, REQ-011
- Evidence types: code, tests-found, tests-run
- Evidence references: fixture lines 10, 21, 39-46; `docs/reference/escalation-policies.md` lines 29, 273, 320-321.
- Why it matters: Without drill-proven replay and late-frame handling, crash/restart and detached-provider cases can still threaten double-advance, lost terminal evidence, or ambiguous quota attribution.
- Recommended action: Implement P058-specific shutdown drain/replay and late-frame drop/journal behavior, then add operational drills.
- Acceptance criteria: SIGTERM/restart, force-detach replay, late-frame arrival, no double-advance, and no double-charge are proven by tests or evidence artifacts.

### UI-001 - macOS components exist but are not release-proven as a live surface

- Reviewer: `macos_ui_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-009, REQ-012
- Evidence types: code, tests-run
- Evidence references: `EscalationReadSurfaceViews.swift` lines 78-424; `Proposal058Tests.swift` lines 213-290; adapter remaining integration work lines 19-24; fixture lines 11, 22, 43.
- Why it matters: Constructible views are necessary but not enough for the operator workflow. The proposal requires live read/subscription presentation, stale/loading/error behavior, runbook/notification/dock/menu affordances, accessibility/focus, and layout proof.
- Recommended action: Wire the inspector/components into the run detail surface, add remote visual/runtime evidence, and cover stale/loading/error/narrow/accessibility states.
- Acceptance criteria: screenshot/UI evidence proves all mandated states and controls render from server readback without macOS write authority.

### API-001 - Focused readback parity passes, but release parity evidence is incomplete

- Reviewer: `api_contract_reviewer`
- Severity: Major
- Confidence: Medium-High
- Related REQs: REQ-008
- Evidence types: code, tests-run, design-reference
- Evidence references: GraphQL/MCP focused parity tests passed; fixture lines 49-129; `docs/reference/rust-control-plane.md` line 132 still contains stale null-field wording.
- Why it matters: The API path has improved, but handoff still depends on live daemon/report parity and synchronized reference docs.
- Recommended action: Add live daemon/report parity evidence and update stale reference text to match the implemented readback semantics.
- Acceptance criteria: GraphQL, MCP, report, and reference docs agree on live/non-null field behavior for the same scenario.

### OPS-001 - Metrics remain name-present but not release-proven

- Reviewer: `observability_rollout_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-010
- Evidence types: telemetry, tests-run
- Evidence references: fixture lines 13 and 24; `docs/reference/rust-control-plane.md` line 720; `db::metrics::P058_REQUIRED_METRICS`.
- Why it matters: P058 metrics are rollout guardrails. Metric-name tests do not prove authoritative producers, labels, histograms/rates, or alert thresholds.
- Recommended action: Wire each metric to its producer and add evidence for semantic correctness and decision thresholds.
- Acceptance criteria: Every P058 metric has producer tests/evidence, bounded labels, and correct counter/rate/histogram semantics where required.

### READY-002 - Unowned residual scope blocks closeout

- Reviewer: `chainworks_execution_truth_reviewer`
- Severity: Major
- Confidence: High
- Related REQs: REQ-007 through REQ-011
- Evidence types: proposal, tests-run
- Evidence references: residual scope table above; fixture lines 39-46.
- Why it matters: The implementation has made measurable progress, but the remaining tail has no concrete follow-up proposal owner.
- Recommended action: Either keep P058 active until the tail is complete or create named follow-up proposal artifacts and explicitly reduce P058 scope.
- Acceptance criteria: Remaining runtime, UI, metrics, and drill scope is either implemented or owned by concrete follow-up proposals.

## Readiness Checklist

| Check | Result |
|---|---|
| Canonical proposal gate on audited tree/HEAD | Passed: `./scripts/test-gate.sh proposal-058` |
| Swift governed read-surface focused tests | Passed: 16 `Proposal058Tests` |
| Control-plane focused tests | Passed through P058 gate |
| GraphQL/MCP focused scheduler readback parity | Passed in focused tests and fixture parity lanes |
| Full regression suite | Not run; not required for failed readiness verdict |
| Live daemon/report parity | Pending |
| macOS remote visual/runtime proof | Pending |
| Empty/loading/error/offline/stale UI states | Pending |
| Accessibility/focus/narrow-layout proof | Partial; accessibility summary tested, visual/focus proof pending |
| Privacy/auth redaction | Partial; non-Operator MCP redaction tests passed |
| Metrics/alerts/decision gates | Not Ready |
| Migration/shutdown/recovery drills | Missing |
| Rollback disposition | Partial: data-preserving disable behavior remains in fixture |

## Verification Log

| Command / Check | Result |
|---|---|
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py .../058-configurable-agent-escalation-chains.md` | Generated `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R2.md` |
| `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py .../058-configurable-agent-escalation-chains.md` | No prior proposal-review artifacts found |
| `diff -u` between source proposal and target proposal copy | No differences |
| `git rev-parse HEAD` in target | `ce9e7e825cb3777e89c5cb08b619dd0aa863d033` |
| `git merge-base origin/main HEAD` in target | `3a93e76332512fc07e8b7bec50882ee83d703c2f` |
| `python3 -m json.tool docs/evidence/rollout-contract/operator-readback/p058-full-surface.fixture.json` | Passed |
| `./scripts/test-gate.sh proposal-058` in target | Passed, including Swift P058 tests and control-plane gate |
| Search for required macOS components | Components found in `EscalationReadSurfaceViews.swift`; no live app usage outside inspector/tests found |
| Search for force-detach/launch-storm/shutdown/replay paths | Provider force-detach and launch storm paths found; shutdown/replay and late-frame handling remain doc/metric/drill pending |

## Final Verdict

P058 has advanced materially since R1, but it is still not ready to close as fully implemented. The current implementation satisfies the foundation and several Phase 2 slices, and the canonical P058 gate passes. However, in-scope promised behavior remains missing or only partially proven: shutdown/replay, late-frame handling, no-overlap drills, live daemon/report parity, remote UI proof, metric semantics, migration drill, shutdown drain drill, and recovery artifact drill.

Recommended next actions:

1. Keep P058 active or create explicit follow-up proposal artifacts for the remaining tail.
2. Implement and prove P058 shutdown drain/replay, late-frame journal/drop, and no-overlap recovery drills.
3. Wire macOS read components into the live run surface and capture remote visual/runtime evidence.
4. Add live daemon/report parity evidence and sync `docs/reference/rust-control-plane.md` with current readback semantics.
5. Wire authoritative metrics and complete migration/shutdown/recovery drill artifacts.
