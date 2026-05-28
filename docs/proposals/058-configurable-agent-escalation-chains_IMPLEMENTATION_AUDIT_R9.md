# Proposal 058 Implementation Audit R9

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Report | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R9.md` |
| Audit timestamp | 2026-05-28T19:25:02Z |
| Worktree | `/Users/user/Documents/Chainworks Forge` |
| Target branch | `main` |
| Target HEAD | `926e205a2a650643eba288fcd659d8036cf59073` (`Close P058 menu attention aggregation gaps`) |
| Merge base with `origin/main` | `926e205a2a650643eba288fcd659d8036cf59073` |
| Compare basis | Implicit current worktree; R9 inspected delta from prior audited baseline `9f60c7088ed7fe11233356ba9b5594ddd2d49807..HEAD` |
| Worktree before report write | Clean |
| Proposal state | Active (`Status: refined_after_write_boundary_blocker_resolved`) |

## Verdict

Overall conformance: **Partially Implemented**

Overall implementation readiness: **Not Ready**

Reviewer-selection reuse: **Not reused**. No prior proposal-review artifacts were found. Prior implementation audits were used only as historical context.

Audit confidence: **High** for code-level MenuBarExtra/registry deltas and canonical gate status; **Medium** for macOS visual/runtime fidelity because no runtime screenshot, remote UI run, Full Keyboard Access fixture, scene-restoration fixture, or multi-window UI evidence was produced in this audit.

R9 materially improves the R8 attention surface. `EscalationReadAdapterRegistry` now notifies attention observers on adapter snapshot changes, `P031ThinReadDashboardModel` subscribes to that registry, `EscalationMenuBarPresenter` filters active escalation snapshots, sorts by latest chain update, caps rows at five, and reports overflow, and the P058 Swift suite grew to 31 passing tests.

The implementation is still not ready for full P058 closeout. The MenuBarExtra list contract is closer but the compact menu-bar label still uses `NotificationService.pendingAttentionCount`, which is the global sum of run approvals, blocked runs, operator alerts, and escalation attention, not the P058 aggregate paused/escalation-run count. The all-run source remains based on Runs Home/detail refresh coverage rather than proven live subscriptions for every relevant run state. Several explicit component contracts and release-closeout fixtures are still partial.

## Prior Proposal-Review Reuse

Discovery command:

```bash
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md"
```

Result:

```json
{
  "artifacts": [],
  "proposal_path": "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md",
  "repo_root": "/Users/user/Documents/Chainworks Forge"
}
```

Reuse classification: **Not reused**.

## Implementation Target And Delta

Delta since the prior audited implementation baseline:

- `926e205a Close P058 menu attention aggregation gaps`

Changed files in `9f60c708..HEAD`:

- `Chainworks Forge/Chainworks_ForgeApp.swift`
- `Chainworks Forge/Engine/EscalationReadAdapter.swift`
- `Chainworks Forge/Views/EscalationReadSurfaceViews.swift`
- `Chainworks Forge/Views/RunsHomeView.swift`
- `Chainworks ForgeTests/Proposal058Tests.swift`
- `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R8.md`

Material improvements since R8:

- `EscalationReadAdapter.swift:123-223` adds attention observers and notifies them from `applyChains`, `applyVisibleRunChains`, `reset`, and `removeAdapter`.
- `RunsHomeView.swift:1513-1520`, `RunsHomeView.swift:1664-1700`, and `RunsHomeView.swift:1819-1837` ensure the Runs Home read model subscribes to registry attention updates and no longer manually assigns attention snapshots after each registry call.
- `EscalationReadSurfaceViews.swift:604-795` adds `EscalationMenuBarPresenter` with attention filtering, latest-updated sorting, five-row cap, overflow count, aggregate count, row state pills, and accessibility labels.
- `Chainworks_ForgeApp.swift:93-105` adds a numeric count to the MenuBarExtra compact label.
- `Proposal058Tests.swift:416-453` proves menu presenter filtering/sort/cap/overflow behavior.
- `Proposal058Tests.swift:490-522` proves registry attention observers fire as adapter snapshots change.

## Proposal Contract Summary

P058 commits to a cross-stack escalation system:

- Rust control plane owns policy resolution, trigger classification, tier advancement, pause/resume legality, capacity checks, persistence, recovery, and kill-switch behavior.
- GraphQL, MCP, reports, and macOS readback expose forward-compatible raw strings and redacted, caller-appropriate escalation state.
- Governed macOS is read/subscription presentation only and must not become an escalation lifecycle authority.
- `EscalationReadAdapter` is the sole governed UI source for run detail, inspectors, notifications, shortcuts, command enablement, trace copy, banner state, pause cards, and lineage views.
- All windows and inspectors for the same run subscribe to one shared adapter keyed by `run_id`.
- Dock badge and human-tier attention derive from live aggregation across runs in paused/exhausted/force-detached/recovery/policy-drift states and are recomputed on every adapter snapshot.
- MenuBarExtra renders aggregate paused-run count, at most five rows sorted by most-recent escalation transition, overflow after five rows, and compact count/state semantics.
- macOS components have explicit layout, accessibility, keyboard, menu, trace, drift, density, and responsive behavior contracts.
- Fixtures must prove presentation states, symbol resolution, strict concurrency, scene restoration, multi-window shared publisher, dock aggregation, user attention cancellation, pasteboard atomicity, Full Keyboard Access order, and read-only drift handoff.

## Platform And Product Scope

Apple scope: **macOS**

Backend/service scope: **cross-stack Rust control-plane, GraphQL/MCP readback, persistence, metrics, rollout, and macOS read surface**

Primary product flow: operators can see why a run escalated, what tier/trigger/pause state applies, what attention is required, and what read-only diagnostic or handoff action is available without the SwiftUI app mutating escalation state.

## Primary Flows Audited

1. Escalation policy execution and durable readback in the Rust control plane.
2. GraphQL/MCP/report boundary readback with redaction and caller-appropriate fields.
3. macOS run-detail and inspector rendering from the governed adapter.
4. Dock badge, MenuBarExtra, and background user-attention aggregation.
5. Read-only drift, trace, command, pause, lineage, and accessibility fixture readiness.

## Proposal Fidelity Inventory

### Matches

- Backend/control-plane P058 gate passes, including schema, runtime fact, readback, redaction, claim/start, recovery, MCP/report, and build checks.
- P031 no longer constructs or exports a direct `EscalationSnapshot`; adapter snapshot derivation remains centralized in `EscalationReadAdapter`.
- Registry attention observers now recompute attention snapshots as adapter snapshots change.
- The MenuBarExtra list now includes active escalation snapshots, sorts by chain `updatedAt`, caps visible rows at five, reports overflow, and exposes an aggregate count in the menu content.
- Informational user attention request/cancel behavior is implemented and tested.
- Trace pasteboard copy is tested for `.string` and `public.json` atomicity.
- Lineage retry collapse and shadow-row display logic exist and are tested.
- The canonical P058 gate passes on the audited HEAD.

### Divergences

- The MenuBarExtra compact label uses `pendingAttentionCount`, a global app attention count, for an "Escalation attention" label. That can show approval/operator-alert counts as escalation counts and can diverge from the P058 aggregate list.
- MenuBarExtra empty text is `No escalation runs need attention`, not the proposal's explicit `No paused escalation runs`; overflow text includes a count suffix and is not an actionable "Show all paused runs..." command.
- All-run source freshness is still not proven. The observer fires for every registered adapter snapshot, but non-selected runs are still populated through Runs Home/detail refresh paths, not proven live subscriptions for every run in the proposal's attention states.
- `EscalationStatusCapsule` still renders a single state label; tier and trigger are only in help text for non-compact density, so explicit visible field order and collapse behavior remain incomplete.
- `EscalationPauseCard` lacks the proposal's countdown formatting and narrow-width responsive fallbacks.
- `EscalationCommandMirrorRow` lacks disabled reason parity in subtitle/help/accessibility/tooltip, 48-character middle truncation, and optional state badge support.
- `DriftReviewSheet` shows hashes and external command controls, not structured tier/attempt/trigger/run-id diff details.
- `EscalationLineageView` has retry collapse and shadow styling, but does not prove the fixed column policy, duration field, narrow-width no-scroll behavior, or expanded digest/runtime fact refs required by the proposal.

### Ambiguities / Evidence Gaps

- No runtime screenshot, remote UI run, or snapshot fixture evidence was produced for P058 macOS visual fidelity.
- No P058-specific Full Keyboard Access tab-order fixture was found.
- No P058-specific scene-restoration fixture proving restored windows wait for the shared adapter publisher was found.
- No P058-specific multi-window fixture proving all inspectors receive the same adapter update was found.
- No P058-specific contrast or reduced-motion fixture evidence was found for the escalation components.
- No evidence proves long-run metric-threshold trending or operational drill artifacts.

## Residual Scope / Follow-up Ownership

| Residual item | Current owner | Concrete follow-up proposal? | Blocks conformance/readiness? |
| --- | --- | --- | --- |
| Live source coverage for every run in P058 attention states, beyond registered/refresh-populated adapters | P058 implementation | None found | Blocks both |
| MenuBarExtra compact count from P058 aggregate, exact empty/overflow semantics, and actionable overflow command | P058 implementation | None found | Blocks both |
| Status capsule, pause card, command row, drift sheet, and lineage detailed layout/accessibility contracts | P058 implementation | None found | Blocks both |
| Scene restoration and multi-window shared-adapter fixtures | P058 release closeout | None found | Blocks readiness and leaves REQ-008 partial |
| Remote visual/runtime evidence, Full Keyboard Access, contrast, and reduced-motion evidence | P058 release closeout | None found | Blocks readiness and leaves REQ-017 partial |
| Long-run metric-threshold trending and operational drill artifacts | P058 release closeout | None found | Blocks full release-closeout evidence |

The proposal labels several evidence items as release-closeout items rather than missing backend implementation paths. Under the implementation-audit tail gate, they still prevent `Overall Conformance = Implemented` and `Overall Implementation Readiness = Ready` unless completed or moved to a concrete follow-up proposal.

## Reviewer Selection

Selected reviewers:

| Reviewer | Why selected | Scope audited |
| --- | --- | --- |
| `apple_arch_reviewer` | P058 locks adapter ownership, MainActor publication, shared `run_id` registry, and no local truth reconstruction. | Adapter registry observers, Runs Home model subscription, compact menu data flow. |
| `macos_ui_reviewer` | P058 has detailed macOS component, menu, keyboard, focus, density, and visual contracts. | MenuBarExtra, status capsule, lineage, pause card, command row, drift sheet, fixtures. |
| `api_contract_reviewer` | P058 is a cross-boundary DTO/readback contract with GraphQL/MCP/report parity and raw-string compatibility. | P031 readback shape, Swift DTO presentation boundary, focused gate coverage. |
| `observability_rollout_reviewer` | P058 depends on metrics, kill switch, rollout stages, release-closeout evidence, and operational drills. | Gate evidence, metric declaration proof, residual release evidence. |
| `rust_reliability_reviewer` | Backend P058 still owns retry, pause, capacity, force-detach, idempotency, recovery, and runtime facts. | Canonical P058 control-plane gate status. |

Rejected close alternatives:

- `apple_ux_reviewer`: UX/accessibility concerns are explicit, but the current delta is primarily menu/component implementation and is covered by `macos_ui_reviewer`.
- `rust_arch_reviewer`: no new Rust architecture delta appeared in `9f60c708..HEAD`.
- `rust_security_reviewer`: no new auth/secret/public parsing surface appeared in this delta.
- `rust_performance_reviewer`: no new benchmark or hot-path performance claim was introduced.
- `product_reviewer`: no separate product metric decision was needed for this audit round.

## Requirement Summary

| Requirement | Status |
| --- | --- |
| REQ-001 Policy/tier schema and compile validation | Implemented |
| REQ-002 Durable ledger/runtime facts/event journal/readback | Implemented |
| REQ-003 Caller-appropriate GraphQL/MCP/report readback | Implemented |
| REQ-004 Redaction and sensitive-field exclusion | Implemented |
| REQ-005 Metrics/observability declarations | Implemented |
| REQ-006 Governed macOS drift write boundary is read-only | Implemented |
| REQ-007 `EscalationReadAdapter` is the sole governed UI source | Implemented |
| REQ-008 Shared adapter keyed by `run_id` for all windows/inspectors | Partially Implemented |
| REQ-009 Dock/menu attention from live all-run adapter aggregation | Partially Implemented |
| REQ-010 Informational user attention request/cancel | Implemented |
| REQ-011 MenuBarExtra badge/list/overflow/compact contract | Partially Implemented |
| REQ-012 Lineage retry collapse, disclosure, shadow rows, layout | Partially Implemented |
| REQ-013 Status capsule field order/color/suppression/truncation | Partially Implemented |
| REQ-014 Pause card countdown and responsive layout | Partially Implemented |
| REQ-015 Command mirror disabled reason/truncation/state badge | Partially Implemented |
| REQ-016 Drift review structured diff and handoff details | Partially Implemented |
| REQ-017 Required macOS fixtures and release evidence | Partially Implemented |

## Detailed Requirement Audit

### REQ-001 Policy/tier schema and compile validation

Source: Goals and Architecture policy schema sections.  
Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: `./scripts/test-gate.sh proposal-058` passed.  
Implementation mapping: Control-plane schema, policy, and compile validation tests passed in the canonical proposal gate.  
Gap / note: No R9 regression observed.

### REQ-002 Durable ledger/runtime facts/event journal/readback

Source: Goals and Architecture persistence/readback commitments.  
Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: `./scripts/test-gate.sh proposal-058` passed; runtime facts, claim/start, recovery, schema, and readback groups passed.  
Implementation mapping: Durable escalation ledger/runtime facts/event journal behavior remains covered by the focused control-plane gate.  
Gap / note: No R9 regression observed.

### REQ-003 Caller-appropriate GraphQL/MCP/report readback

Source: Goals and boundary readback commitments.  
Status: **Implemented**  
Evidence: `tests-run`, `code`  
Evidence references: `P031ThinGraphQLReadBoundary.swift:6650-6684`; `Proposal058Tests.swift:645-696`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: GraphQL run detail readback includes P058 escalation chains and redacted trace; presenter preserves chains for adapter-owned presentation.  
Gap / note: No caller-visibility regression observed in the gate.

### REQ-004 Redaction and sensitive-field exclusion

Source: Notifications forbidden fields and readback redaction commitments.  
Status: **Implemented**  
Evidence: `tests-run`, `code`  
Evidence references: `Proposal058Tests.swift:731-740`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Trace copy uses redacted JSON; P058 gate includes payload-shape, credential/path rejection, and security readback checks.  
Gap / note: No raw sensitive-field rendering was found in inspected P058 macOS components.

### REQ-005 Metrics/observability declarations

Source: rollout/metrics commitments.  
Status: **Implemented**  
Evidence: `tests-run`, `telemetry`  
Evidence references: `./scripts/test-gate.sh proposal-058`; gate output included `metrics::tests::proposal_058_required_metric_names_are_declared`.  
Implementation mapping: Required P058 metric names remain declared and covered by the canonical gate.  
Gap / note: Long-run threshold trending remains release-closeout evidence under REQ-017.

### REQ-006 Governed macOS drift write boundary is read-only

Source: macOS Authority Boundary and DriftReviewSheet Write Boundary.  
Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:838+`; `Proposal058Tests.swift:731-740`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: The sheet exposes copy/open/close actions and contains no governed macOS mutation call.  
Gap / note: The sheet is read-only, but structured diff content remains incomplete under REQ-016.

### REQ-007 `EscalationReadAdapter` is the sole governed UI source

Source: macOS Authority Boundary, proposal lines 87-90.  
Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:20-36`; `EscalationReadAdapter.swift:120-223`; `P031ThinGraphQLReadBoundary.swift:5680-5681`; `P031ThinGraphQLReadBoundary.swift:6650-6684`; `Proposal058Tests.swift:653-714`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: P031 passes DTO chains and redacted trace only; snapshot derivation lives in `EscalationReadAdapter`; inspector uses the registry adapter; tests verify no direct `escalationSnapshot` field on P031 run detail presentation.  
Gap / note: Future read surfaces still need to preserve the same boundary.

### REQ-008 Shared adapter keyed by `run_id` for all windows/inspectors

Source: macOS Authority Boundary, proposal line 91; fixtures lines 199-200.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:120-223`; `EscalationReadSurfaceViews.swift:757-780`; `Proposal058Tests.swift:455-522`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: The registry returns one adapter per run ID and now supports observers over registry attention snapshots.  
Gap / note: No scene-restoration or multi-window runtime fixture proves restored windows wait for the shared publisher or that all inspectors receive the same update in production. `applyVisibleRunChains` still removes adapters not in the visible run-ID set, which needs runtime proof that it cannot break separate inspectors/restored scenes.

### REQ-009 Dock/menu attention from live all-run adapter aggregation

Source: Notifications Dock Badge and Human Tier Attention, proposal lines 207-214.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:204-223`; `RunsHomeView.swift:1513-1520`; `RunsHomeView.swift:1664-1700`; `RunsHomeView.swift:1819-1837`; `NotificationService.swift:159-170`; `Proposal058Tests.swift:490-522`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Registry observers notify attention subscribers on adapter snapshot changes, and the Runs Home model publishes those snapshots for `ContentView` to sync into `NotificationService`.  
Gap / note: The observer covers every registered adapter snapshot, but the implementation still does not prove live source coverage for every run in the proposal's attention states. Non-selected runs are populated through Runs Home/detail refresh paths rather than a demonstrated all-run live escalation subscription.

### REQ-010 Informational user attention request/cancel

Source: Human Tier Attention, proposal line 214.  
Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `NotificationService.swift:267-280`; `Proposal058Tests.swift:558+`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: P058 attention requests use `NSApp.requestUserAttention(.informationalRequest)` through injectable hooks and cancel on activation or pause clear.  
Gap / note: No R9 regression observed.

### REQ-011 MenuBarExtra badge/list/overflow/compact contract

Source: MenuBarExtra, proposal lines 156-161; density rules lines 164-175.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `Chainworks_ForgeApp.swift:85-107`; `EscalationReadSurfaceViews.swift:604-795`; `NotificationService.swift:25-32`; `NotificationService.swift:256-264`; `Proposal058Tests.swift:416-453`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Menu content now has aggregate count, row cap, latest-update sort, active-escalation filter, row state pills, and overflow count.  
Gap / note: The compact item count uses global `pendingAttentionCount`, not the P058 escalation aggregate. Empty text and overflow text also do not match the explicit proposal strings/command semantics.

### REQ-012 Lineage retry collapse, disclosure, shadow rows, layout

Source: EscalationLineageView, proposal lines 124-130.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:282-428`; `Proposal058Tests.swift:620-642`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Retry collapse and shadow-row styling exist and are tested.  
Gap / note: Fixed columns, duration field, right-aligned monospace attempt/duration fields, narrow-width no-horizontal-scroll behavior, and expanded digest/runtime fact refs are not fully implemented or proven.

### REQ-013 Status capsule field order/color/suppression/truncation

Source: EscalationStatusCapsule, proposal lines 139-152.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:86-114`; `Proposal058Tests.swift:288`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: A compact status label, symbol, color, help text, and accessibility label exist.  
Gap / note: Visible field order does not show state, tier, and trigger as separate slots; collapse order, exact same-backend retry color states, and 24-character middle truncation are not proven by fixtures.

### REQ-014 Pause card countdown and responsive layout

Source: EscalationPauseCard, proposal lines 131-138.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:431-482`; `Proposal058Tests.swift:291`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Pause title, body/action hint, runbook button, diagnostic copy button, and metadata strip exist.  
Gap / note: Countdown formatting and responsive breakpoints below 360pt/280pt are not implemented or proven.

### REQ-015 Command mirror disabled reason/truncation/state badge

Source: EscalationCommandPresentation, proposal lines 116-123.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:484-514`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Command row title/subtitle/copy action exists.  
Gap / note: Disabled reason parity in subtitle/help/accessibility/tooltip, 48-character middle truncation, and optional state badge are not implemented or proven.

### REQ-016 Drift review structured diff and handoff details

Source: DriftReviewSheet, proposal lines 95-104.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:838+`; `Proposal058Tests.swift:300-306`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Read-only sheet provides frozen/current hashes, copy acknowledgement command, open external workflow, close, and interactive dismiss disabled.  
Gap / note: It does not render tier added/removed/changed badges, max-chain-attempts delta, trigger deltas, run ID, or richer structured external handoff details.

### REQ-017 Required macOS fixtures and release evidence

Source: Fixtures, Notifications, and Implementation Sync, proposal lines 32-33 and 195-205.  
Status: **Partially Implemented**  
Evidence: `tests-found`, `tests-run`, `code`  
Evidence references: `Proposal058Tests.swift`; `./scripts/test-gate.sh proposal-058`; targeted `rg` over `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.  
Implementation mapping: P058 focused Swift tests cover adapter source ownership, registry observer aggregation, menu presenter cap/sort/overflow, dock per-run count, user attention hooks, trace pasteboard copy, and component construction paths.  
Gap / note: No P058-specific remote visual/runtime evidence, Full Keyboard Access fixture, scene-restoration fixture, multi-window fixture, contrast proof, reduced-motion proof, long-run metric-threshold trending, or operational drill artifact was found.

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Objective proposal conformance | Partial | Multiple explicit macOS UI/evidence requirements remain partial. | High |
| Apple architecture | Partial | Registry observer is improved, but all-run live source coverage and multi-window/restored-scene behavior are not proven. | Medium |
| macOS UI | Partial | Compact MenuBarExtra count uses global pending attention, and component layout/accessibility contracts remain incomplete. | High |
| API contract | Pass with residual guard | P031 adapter boundary is corrected; future read surfaces need the same guard pattern. | High |
| Observability/rollout | Partial | Release-closeout evidence, long-run trending, and operational drill artifacts are not present. | Medium |
| Rust reliability | Pass | Canonical P058 control-plane gate passed; no new Rust reliability delta in R9. | High |
| Readiness | Not Ready | Remaining major findings and missing release evidence block closeout. | High |

## Routed Specialist Findings

### ARCH-001 [Major] All-run attention is observer-driven only after adapters are populated

Reviewer: `apple_arch_reviewer`  
Confidence: **Medium**  
Related requirements: REQ-008, REQ-009  
Evidence types: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:204-223`, `RunsHomeView.swift:1664-1700`, `RunsHomeView.swift:1819-1837`, `Proposal058Tests.swift:490-522`.

Why it matters: R9 fixes "recompute on adapter snapshot" for registered adapters, but the proposal asks for live aggregation across runs in named attention states. The inspected code still relies on Runs Home/detail refresh paths to populate non-selected adapters. A run that changes escalation state without a refresh or adapter update can remain absent or stale in the badge/menu aggregation.

Recommended action: Add a dedicated all-run escalation attention read/subscription source, or prove Runs Home delivers all relevant run IDs and refreshes their escalation chains whenever any run enters/leaves a P058 attention state.

Acceptance criteria: A test or integration fixture shows a non-selected run entering and clearing each P058 attention state updates the registry observer, Dock badge, MenuBarExtra content, and user-attention token without manual notification-click handlers or selected-run refresh.

### UI-001 [Major] MenuBarExtra compact count is not the P058 aggregate count

Reviewer: `macos_ui_reviewer`  
Confidence: **High**  
Related requirements: REQ-011  
Evidence types: `code`, `tests-found`  
Evidence references: `Chainworks_ForgeApp.swift:93-105`, `NotificationService.swift:25-32`, `NotificationService.swift:256-264`, `EscalationReadSurfaceViews.swift:604-795`.

Why it matters: The MenuBarExtra label says "Escalation attention" and its menu content is built from P058 snapshots, but the compact label count uses `pendingAttentionCount`, which includes waiting approvals, blocked runs, operator alerts, and escalation attention. This can display a non-P058 count next to a P058-only menu and violates the proposal's aggregate paused-run count contract.

Recommended action: Expose a P058-specific aggregate count from `NotificationService` or from `EscalationMenuBarPresenter`, and use that value for the P058 MenuBarExtra compact count and symbol state.

Acceptance criteria: A test covers mixed pending approvals/operator alerts plus one P058 escalation snapshot and proves the P058 MenuBarExtra compact count equals the P058 aggregate, not global pending attention.

### UI-002 [Major] Component-specific macOS layout and accessibility contracts remain incomplete

Reviewer: `macos_ui_reviewer`  
Confidence: **High**  
Related requirements: REQ-012, REQ-013, REQ-014, REQ-015, REQ-016, REQ-017  
Evidence types: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:86-114`, `EscalationReadSurfaceViews.swift:282-514`, `EscalationReadSurfaceViews.swift:838+`, `Proposal058Tests.swift:288-306`.

Why it matters: The proposal names field order, truncation, countdown, responsive breakpoints, command disabled reason parity, structured drift diff, keyboard focus, and fixture evidence. Current tests mostly prove construction and selected presentation helpers, not the required macOS behavior and layout details.

Recommended action: Finish the component implementations and add focused fixtures for field order, truncation, narrow widths, keyboard order, reduced motion, contrast, structured drift diff, and no-horizontal-scroll lineage.

Acceptance criteria: P058 tests or UI fixtures assert the exact component slots and states from proposal lines 95-152 and 195-205.

### API-001 [Minor] Adapter boundary guard should cover future presentation surfaces

Reviewer: `api_contract_reviewer`  
Confidence: **Medium**  
Related requirements: REQ-007  
Evidence types: `code`, `tests-found`  
Evidence references: `P031ThinGraphQLReadBoundary.swift:5680-5681`, `P031ThinGraphQLReadBoundary.swift:6650-6684`, `Proposal058Tests.swift:697-714`.

Why it matters: The R7 direct-snapshot gap is closed for P031, but the current guard verifies the P031 presentation field shape. Future generated/readback presenters could reintroduce direct UI snapshot construction unless the guard pattern remains in place.

Recommended action: Keep source or type-level tests that fail if UI-facing readback surfaces outside `EscalationReadAdapter` construct/export `EscalationSnapshot`.

Acceptance criteria: Adding a direct `EscalationSnapshot` field or `EscalationSnapshot.build` call outside the adapter-owned path fails a focused P058 guard test.

### READY-001 [Major] Required P058 runtime, fixture, and release-closeout evidence is still incomplete

Reviewer: `observability_rollout_reviewer`  
Confidence: **High**  
Related requirements: REQ-017  
Evidence types: `proposal`, `tests-found`, `tests-run`  
Evidence references: proposal lines 32-33 and 195-205; targeted search over `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.

Why it matters: The canonical proposal gate passes, but the release-closeout evidence named by P058 is not complete. Without visual/runtime, accessibility, restoration, multi-window, contrast, reduced-motion, trending, and drill artifacts, the implementation cannot be closed out as ready.

Recommended action: Produce or attach the missing release evidence, or move it to a concrete follow-up proposal if the project intentionally defers it.

Acceptance criteria: The P058 closeout evidence set includes remote visual/runtime proof, Full Keyboard Access order, scene restoration, multi-window shared publisher, contrast/reduced-motion fixtures, long-run metric-threshold trending, and operational drill artifacts.

## Readiness Checklist

| Gate | Status | Evidence |
| --- | --- | --- |
| Proposal file exists and is active | Pass | `docs/proposals/058-configurable-agent-escalation-chains.md:13` |
| Prior proposal-review selection discovered | None | helper returned no artifacts |
| Current implementation target identified | Pass | branch `main`, HEAD `926e205a2a650643eba288fcd659d8036cf59073` |
| Same-tree canonical proposal gate | Pass | `./scripts/test-gate.sh proposal-058` |
| Swift P058 focused tests | Pass | observed 31 tests, 0 failures |
| Control-plane P058 focused tests/builds | Pass | final gate line: `Proposal 058 control-plane gate passed` |
| Adapter sole-source blocker from R7 | Pass | P031 exports chains/trace only; P031 presentation mirror test guards no direct snapshot field |
| Registry observer recompute on adapter snapshot | Pass | `Proposal058Tests.swift:490-522` |
| MenuBarExtra list cap/sort/overflow | Pass for list presenter | `Proposal058Tests.swift:416-453` |
| MenuBarExtra compact P058 count | Partial | compact item uses global `pendingAttentionCount` |
| Live all-run source coverage | Partial | observer is live for registered adapters; all-run source coverage not proven |
| Component UI/accessibility contract | Partial | several explicit slots/layout/accessibility fixtures missing |
| Runtime/screenshot/accessibility release evidence | Partial | no P058-specific evidence found |

## Verification Log

Commands and checks run:

```bash
git status --short
git branch --show-current
git rev-parse HEAD
git merge-base HEAD origin/main
date -u +%Y-%m-%dT%H:%M:%SZ
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md"
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md"
git log --oneline --decorate --no-renames 9f60c7088ed7fe11233356ba9b5594ddd2d49807..HEAD
git show --stat --oneline --decorate --no-renames 9f60c7088ed7fe11233356ba9b5594ddd2d49807..HEAD
git diff --name-only 9f60c7088ed7fe11233356ba9b5594ddd2d49807..HEAD
rg -n "Full Keyboard|Keyboard Access|scene restoration|multi-window|contrast|Reduced Motion|requestUserAttention|Dock badge|MenuBarExtra|menubar|menu bar|p058" "Chainworks ForgeTests" docs/reference docs/evidence -g "*.swift" -g "*.md"
rg -n "EscalationMenuBarPresenter|addAttentionObserver|ensureEscalationAttentionObserver|EscalationStatusCapsule|EscalationPauseCard|DriftReviewSheet|Full Keyboard|scene restoration|multi-window|Reduce|contrast" "Chainworks Forge" "Chainworks ForgeTests" -g "*.swift"
./scripts/test-gate.sh proposal-058
```

Important verification results:

- Worktree was clean before writing this report.
- Report path helper returned this R9 path.
- Prior proposal-review helper returned no artifacts.
- Current audited HEAD is `926e205a2a650643eba288fcd659d8036cf59073`.
- `./scripts/test-gate.sh proposal-058` passed. The Swift P058 suite reported 31 passing tests; the control-plane gate finished with `Proposal 058 control-plane gate passed`.
- Gate output still includes existing Swift/Rust warning noise, but no failure.

## Final Action Items

1. Use a P058-specific aggregate count for the MenuBarExtra compact label instead of global `pendingAttentionCount`.
2. Prove or implement live all-run escalation attention source coverage for non-selected runs entering/leaving every P058 attention state.
3. Finish exact MenuBarExtra empty/overflow command semantics.
4. Finish explicit macOS component contracts for status capsule, pause card, command row, drift sheet, and lineage layout/accessibility.
5. Add or attach P058-specific release-closeout evidence for remote visual/runtime behavior, Full Keyboard Access, scene restoration, multi-window shared publisher, contrast, reduced motion, long-run metric thresholds, and operational drills.

## Final Verdict

P058 is **Partially Implemented** and **Not Ready** for full implementation closeout.

The current HEAD is materially better than R8: registry observer propagation and MenuBarExtra list presentation are now tested and mostly in place. Remaining blockers are narrower but still concrete: compact MenuBarExtra count uses the wrong aggregate, all-run source freshness is not proven, component contracts remain partial, and required runtime/release evidence is still missing.
