# Proposal 058 Implementation Audit R10

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Report | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R10.md` |
| Audit timestamp | 2026-05-28T20:21:01Z |
| Worktree | `/Users/user/Documents/Chainworks Forge` |
| Target branch | `main` |
| Target HEAD | `7bc4c43810eca2a60835cc8edeaab984d2c7f896` (`Fix P058 menu bar escalation count`) |
| Merge base with `origin/main` | `7bc4c43810eca2a60835cc8edeaab984d2c7f896` |
| Compare basis | Implicit current worktree; R10 inspected delta from prior audited baseline `926e205a2a650643eba288fcd659d8036cf59073..HEAD` |
| Worktree before report write | Clean |
| Proposal state | Active (`Status: refined_after_write_boundary_blocker_resolved`) |

## Verdict

Overall conformance: **Partially Implemented**

Overall implementation readiness: **Not Ready**

Reviewer-selection reuse: **Not reused**. No prior proposal-review artifacts were found. Prior implementation audits were used only as historical context.

Audit confidence: **High** for the R10 compact MenuBarExtra count delta and canonical gate result; **Medium** for macOS visual/runtime fidelity because no runtime screenshot, remote UI run, Full Keyboard Access fixture, scene-restoration fixture, or multi-window UI evidence was produced in this audit.

R10 closes the primary R9 MenuBarExtra compact-count blocker. `NotificationService` now exposes `p058EscalationAttentionCount`, the compact `MenuBarExtra` label reads that P058-specific count instead of global `pendingAttentionCount`, and the P058 Swift suite includes a regression test proving one P058 paused run stays distinct from three non-P058 attention items.

The implementation is still not ready for full P058 closeout. The remaining blockers are narrower but still in-scope under the proposal tail gate: all-run live attention source coverage is not proven, MenuBarExtra exact empty/overflow command semantics remain partial, component layout/accessibility contracts remain incomplete, and required runtime/release evidence is still missing or only documented as closeout work without a concrete follow-up proposal.

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

- `7bc4c438 Fix P058 menu bar escalation count`

Changed files in `926e205a..HEAD`:

- `Chainworks Forge/Chainworks_ForgeApp.swift`
- `Chainworks Forge/Engine/NotificationService.swift`
- `Chainworks ForgeTests/Proposal058Tests.swift`
- `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R9.md`

Material improvements since R9:

- `NotificationService.swift:25-34` adds `p058EscalationAttentionCount` beside global `pendingAttentionCount`.
- `NotificationService.swift:161-174` computes the P058 count from paused, policy-drift, kill-switch, and active-escalation snapshots, then feeds the existing dock/menu/user-attention path.
- `Chainworks_ForgeApp.swift:93-105` uses `p058EscalationAttentionCount` for the compact `MenuBarExtra` icon state and numeric count.
- `Proposal058Tests.swift:378-412` asserts both the P058 count and global dock count, including a mixed case where global pending attention is `4` while the P058 compact count is `1`.
- `./scripts/test-gate.sh proposal-058` now reports 32 passing Swift P058 tests and the control-plane P058 gate passed.

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
- Registry attention observers recompute attention snapshots as registered adapter snapshots change.
- The MenuBarExtra compact count is now P058-specific rather than the global attention total.
- The MenuBarExtra list includes active escalation snapshots, sorts by chain `updatedAt`, caps visible rows at five, reports overflow count, and exposes an aggregate count in the menu content.
- Informational user attention request/cancel behavior is implemented and tested.
- Trace pasteboard copy is tested for `.string` and `public.json` atomicity.
- Lineage retry collapse and shadow-row display logic exist and are tested.
- The canonical P058 gate passes on the audited HEAD.

### Divergences

- MenuBarExtra empty text is `No escalation runs need attention`, not the proposal's explicit `No paused escalation runs`; overflow text includes a count suffix and is a non-actionable `Text`, not an actionable `Show all paused runs...` command.
- All-run source freshness is still not proven. The observer fires for every registered adapter snapshot, but non-selected runs are still populated through Runs Home/detail refresh paths, not proven live subscriptions for every run in the proposal's attention states.
- `EscalationStatusCapsule` still renders a single state label; tier and trigger are only in help text for non-compact density, so explicit visible field order and collapse behavior remain incomplete.
- `EscalationPauseCard` lacks the proposal's countdown formatting and narrow-width responsive fallbacks.
- `EscalationCommandMirrorRow` lacks disabled reason parity in subtitle/help/accessibility/tooltip, 48-character middle truncation, and optional state badge support.
- `DriftReviewSheet` shows hashes and external command controls, not structured tier/attempt/trigger/run-id diff details.
- `EscalationLineageView` has retry collapse and shadow styling, but does not prove the fixed column policy, duration field, narrow-width no-scroll behavior, or expanded digest/runtime fact refs required by the proposal.
- The Swift gate still emits actor-isolation warnings from `EscalationMenuBarPresenter.presentation(for:)` calling static presenter helpers in a synchronous context.

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
| MenuBarExtra exact empty state, overflow text, and actionable overflow command | P058 implementation | None found | Blocks conformance for REQ-011 |
| Status capsule, pause card, command row, drift sheet, and lineage detailed layout/accessibility contracts | P058 implementation | None found | Blocks both |
| Swift strict-concurrency cleanup for current actor-isolation warnings in the P058 menu presenter | P058 implementation | None found | Blocks readiness evidence |
| Scene restoration and multi-window shared-adapter fixtures | P058 release closeout | None found | Blocks readiness and leaves REQ-008 partial |
| Remote visual/runtime evidence, Full Keyboard Access, contrast, and reduced-motion evidence | P058 release closeout | None found | Blocks readiness and leaves REQ-017 partial |
| Long-run metric-threshold trending and operational drill artifacts | P058 release closeout | None found | Blocks full release-closeout evidence |

The proposal labels several evidence items as release-closeout items rather than missing backend implementation paths. Under the implementation-audit tail gate, they still prevent `Overall Conformance = Implemented` and `Overall Implementation Readiness = Ready` unless completed or moved to a concrete follow-up proposal.

## Reviewer Selection

Selected reviewers:

| Reviewer | Why selected | Scope audited |
| --- | --- | --- |
| `apple_arch_reviewer` | P058 locks adapter ownership, MainActor publication, shared `run_id` registry, and no local truth reconstruction. | Adapter registry observers, Runs Home model subscription, compact menu data flow, actor-isolation warnings. |
| `macos_ui_reviewer` | P058 has detailed macOS component, menu, keyboard, focus, density, and visual contracts. | MenuBarExtra, status capsule, lineage, pause card, command row, drift sheet, fixtures. |
| `api_contract_reviewer` | P058 is a cross-boundary DTO/readback contract with GraphQL/MCP/report parity and raw-string compatibility. | P031 readback shape, Swift DTO presentation boundary, focused gate coverage. |
| `observability_rollout_reviewer` | P058 depends on metrics, kill switch, rollout stages, release-closeout evidence, and operational drills. | Gate evidence, metric declaration proof, residual release evidence. |
| `rust_reliability_reviewer` | Backend P058 still owns retry, pause, capacity, force-detach, idempotency, recovery, and runtime facts. | Canonical P058 control-plane gate status. |

Rejected close alternatives:

- `apple_ux_reviewer`: UX/accessibility concerns are explicit, but the current delta is primarily menu/component implementation and is covered by `macos_ui_reviewer`.
- `rust_arch_reviewer`: no new Rust architecture delta appeared in `926e205a..HEAD`.
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

Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Control-plane schema, policy, and compile validation tests passed in the canonical proposal gate.  
Gap / note: No R10 regression observed.

### REQ-002 Durable ledger/runtime facts/event journal/readback

Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Durable escalation ledger/runtime facts/event journal behavior remains covered by the focused control-plane gate.  
Gap / note: No R10 regression observed.

### REQ-003 Caller-appropriate GraphQL/MCP/report readback

Status: **Implemented**  
Evidence: `tests-run`, `code`  
Evidence references: `Proposal058Tests.swift:694-720`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: GraphQL run detail readback includes P058 escalation chains and redacted trace; presenter preserves chains for adapter-owned presentation.  
Gap / note: No caller-visibility regression observed in the gate.

### REQ-004 Redaction and sensitive-field exclusion

Status: **Implemented**  
Evidence: `tests-run`, `code`  
Evidence references: `Proposal058Tests.swift:755-763`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Trace copy uses redacted JSON; P058 gate includes payload-shape, credential/path rejection, and security readback checks.  
Gap / note: No raw sensitive-field rendering was found in inspected P058 macOS components.

### REQ-005 Metrics/observability declarations

Status: **Implemented**  
Evidence: `tests-run`, `telemetry`  
Evidence references: `./scripts/test-gate.sh proposal-058`; gate output included `metrics::tests::proposal_058_required_metric_names_are_declared`.  
Implementation mapping: Required P058 metric names remain declared and covered by the canonical gate.  
Gap / note: Long-run threshold trending remains release-closeout evidence under REQ-017.

### REQ-006 Governed macOS drift write boundary is read-only

Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:838-884`; `Proposal058Tests.swift:300-306`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: The sheet exposes copy/open/close actions and contains no governed macOS mutation call.  
Gap / note: The sheet is read-only, but structured diff content remains incomplete under REQ-016.

### REQ-007 `EscalationReadAdapter` is the sole governed UI source

Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:117-223`; `EscalationReadSurfaceViews.swift:908-930`; `Proposal058Tests.swift:694-720`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: P031 passes DTO chains and redacted trace only; snapshot derivation lives in `EscalationReadAdapter`; inspector uses the registry adapter; tests verify no direct `escalationSnapshot` field on P031 run detail presentation.  
Gap / note: Future read surfaces still need to preserve the same boundary.

### REQ-008 Shared adapter keyed by `run_id` for all windows/inspectors

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:117-223`; `EscalationReadSurfaceViews.swift:908-930`; `Proposal058Tests.swift:455-522`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: The registry returns one adapter per run ID and supports observers over registry attention snapshots.  
Gap / note: No scene-restoration or multi-window runtime fixture proves restored windows wait for the shared publisher or that all inspectors receive the same update in production. `applyVisibleRunChains` still removes adapters not in the visible run-ID set, which needs runtime proof that it cannot break separate inspectors/restored scenes.

### REQ-009 Dock/menu attention from live all-run adapter aggregation

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:204-223`; `RunsHomeView.swift:1512-1520`; `RunsHomeView.swift:1664-1700`; `RunsHomeView.swift:1816-1837`; `NotificationService.swift:161-174`; `Proposal058Tests.swift:490-522`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Registry observers notify attention subscribers on adapter snapshot changes, and the Runs Home model publishes those snapshots for `ContentView` to sync into `NotificationService`.  
Gap / note: The observer covers every registered adapter snapshot, but the implementation still does not prove live source coverage for every run in the proposal's attention states. Non-selected runs are populated through Runs Home/detail refresh paths rather than a demonstrated all-run live escalation subscription.

### REQ-010 Informational user attention request/cancel

Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `NotificationService.swift:270-283`; `Proposal058Tests.swift:582+`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: P058 attention requests use `NSApp.requestUserAttention(.informationalRequest)` through injectable hooks and cancel on activation or pause clear.  
Gap / note: No R10 regression observed.

### REQ-011 MenuBarExtra badge/list/overflow/compact contract

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `Chainworks_ForgeApp.swift:85-107`; `NotificationService.swift:25-34`; `NotificationService.swift:161-174`; `EscalationReadSurfaceViews.swift:604-795`; `Proposal058Tests.swift:378-412`; `Proposal058Tests.swift:440-476`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Compact count now uses the P058 aggregate; menu content has aggregate count, row cap, latest-update sort, active-escalation filter, row state pills, and overflow count.  
Gap / note: Empty text and overflow command semantics still do not match the explicit proposal strings/actions. The Swift gate also warns about actor-isolated static presenter helper calls.

### REQ-012 Lineage retry collapse, disclosure, shadow rows, layout

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:282-428`; `Proposal058Tests.swift:620-642`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Retry collapse and shadow-row styling exist and are tested.  
Gap / note: Fixed columns, duration field, right-aligned monospace attempt/duration fields, narrow-width no-horizontal-scroll behavior, and expanded digest/runtime fact refs are not fully implemented or proven.

### REQ-013 Status capsule field order/color/suppression/truncation

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:86-114`; `Proposal058Tests.swift:288`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: A compact status label, symbol, color, help text, and accessibility label exist.  
Gap / note: Visible field order does not show state, tier, and trigger as separate slots; collapse order, exact same-backend retry color states, and 24-character middle truncation are not proven by fixtures.

### REQ-014 Pause card countdown and responsive layout

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:431-482`; `Proposal058Tests.swift:291`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Pause title, body/action hint, runbook button, diagnostic copy button, and metadata strip exist.  
Gap / note: Countdown formatting and responsive breakpoints below 360pt/280pt are not implemented or proven.

### REQ-015 Command mirror disabled reason/truncation/state badge

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:484-514`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Command row title/subtitle/copy action exists.  
Gap / note: Disabled reason parity in subtitle/help/accessibility/tooltip, 48-character middle truncation, and optional state badge are not implemented or proven.

### REQ-016 Drift review structured diff and handoff details

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:838-884`; `Proposal058Tests.swift:300-306`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Read-only sheet provides frozen/current hashes, copy acknowledgement command, open external workflow, close, and interactive dismiss disabled.  
Gap / note: It does not render tier added/removed/changed badges, max-chain-attempts delta, trigger deltas, run ID, or richer structured external handoff details.

### REQ-017 Required macOS fixtures and release evidence

Status: **Partially Implemented**  
Evidence: `tests-found`, `tests-run`, `code`  
Evidence references: `Proposal058Tests.swift`; `./scripts/test-gate.sh proposal-058`; targeted `rg` over `Chainworks Forge`, `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.  
Implementation mapping: P058 focused Swift tests cover adapter source ownership, registry observer aggregation, menu presenter cap/sort/overflow, dock per-run count, compact P058 count separation, user attention hooks, trace pasteboard copy, and component construction paths.  
Gap / note: No P058-specific remote visual/runtime evidence, Full Keyboard Access fixture, scene-restoration fixture, multi-window fixture, contrast proof, reduced-motion proof, long-run metric-threshold trending, or operational drill artifact was found. The P058 Swift build currently emits actor-isolation warnings in the menu presenter.

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Objective proposal conformance | Partial | Multiple explicit macOS UI/evidence requirements remain partial. | High |
| Apple architecture | Partial | Registry observer is improved, but all-run live source coverage and multi-window/restored-scene behavior are not proven. | Medium |
| macOS UI | Partial | Compact count is fixed; component layout/accessibility contracts and exact menu empty/overflow semantics remain incomplete. | High |
| API contract | Pass with residual guard | P031 adapter boundary is corrected; future read surfaces need the same guard pattern. | High |
| Observability/rollout | Partial | Release-closeout evidence, long-run trending, and operational drill artifacts are not present. | Medium |
| Rust reliability | Pass | Canonical P058 control-plane gate passed; no new Rust reliability delta in R10. | High |
| Readiness | Not Ready | Remaining major findings and missing release evidence block closeout. | High |

## Routed Specialist Findings

### ARCH-001 [Major] All-run attention is observer-driven only after adapters are populated

Reviewer: `apple_arch_reviewer`  
Confidence: **Medium**  
Related requirements: REQ-008, REQ-009  
Evidence types: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:204-223`, `RunsHomeView.swift:1664-1700`, `RunsHomeView.swift:1816-1837`, `Proposal058Tests.swift:490-522`.

Why it matters: The implementation recomputes attention when registered adapters change, but the proposal asks for live aggregation across runs in named attention states. The inspected code still relies on Runs Home/detail refresh paths to populate non-selected adapters. A run that changes escalation state without a refresh or adapter update can remain absent or stale in the badge/menu aggregation.

Recommended action: Add a dedicated all-run escalation attention read/subscription source, or prove Runs Home delivers all relevant run IDs and refreshes their escalation chains whenever any run enters/leaves a P058 attention state.

Acceptance criteria: A test or integration fixture shows a non-selected run entering and clearing each P058 attention state updates the registry observer, Dock badge, MenuBarExtra content, and user-attention token without manual notification-click handlers or selected-run refresh.

### UI-001 [Minor] MenuBarExtra exact empty and overflow command semantics remain partial

Reviewer: `macos_ui_reviewer`  
Confidence: **High**  
Related requirements: REQ-011  
Evidence types: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:604-795`, `Proposal058Tests.swift:440-476`.

Why it matters: R10 fixed the compact P058 count, but the proposal pins exact MenuBarExtra empty and overflow behavior. Current empty text is `No escalation runs need attention`, while the proposal says `No paused escalation runs`. Current overflow is non-actionable text with `+\(overflowCount)`, while the proposal says to show `Show all paused runs...` after five rows.

Recommended action: Align the empty-state text and make overflow an actionable command that opens the full paused/escalation runs view while preserving the five-row cap.

Acceptance criteria: Focused tests assert exact empty text, exact overflow command label, and the overflow action target for six or more attention runs.

### UI-002 [Major] Component-specific macOS layout and accessibility contracts remain incomplete

Reviewer: `macos_ui_reviewer`  
Confidence: **High**  
Related requirements: REQ-012, REQ-013, REQ-014, REQ-015, REQ-016, REQ-017  
Evidence types: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:86-114`, `EscalationReadSurfaceViews.swift:282-514`, `EscalationReadSurfaceViews.swift:838-884`, `Proposal058Tests.swift:288-306`.

Why it matters: The proposal names field order, truncation, countdown, responsive breakpoints, command disabled reason parity, structured drift diff, keyboard focus, and fixture evidence. Current tests mostly prove construction and selected presentation helpers, not the required macOS behavior and layout details.

Recommended action: Finish the component implementations and add focused fixtures for field order, truncation, narrow widths, keyboard order, reduced motion, contrast, structured drift diff, and no-horizontal-scroll lineage.

Acceptance criteria: P058 tests or UI fixtures assert the exact component slots and states from proposal lines 95-152 and 195-205.

### ARCH-002 [Minor] P058 MenuBarExtra presenter emits Swift actor-isolation warnings

Reviewer: `apple_arch_reviewer`  
Confidence: **High**  
Related requirements: REQ-011, REQ-017  
Evidence types: `tests-run`, `code`  
Evidence references: gate output for `./scripts/test-gate.sh proposal-058`; `EscalationReadSurfaceViews.swift:714-795`.

Why it matters: The canonical gate passes, but Swift warns that `EscalationMenuBarPresenter.presentation(for:)` calls main-actor-isolated static helper methods from a synchronous nonisolated context. P058 explicitly calls for strict-concurrency evidence around DTO decode, MainActor publication, and immutable presentation consumption, so warnings in the P058 presenter weaken closeout confidence.

Recommended action: Make the presenter's actor isolation explicit and warning-free, for example by marking pure helpers appropriately or isolating the presenter consistently with the call sites.

Acceptance criteria: The P058 Swift gate runs without actor-isolation warnings from `EscalationReadSurfaceViews.swift`.

### API-001 [Minor] Adapter boundary guard should cover future presentation surfaces

Reviewer: `api_contract_reviewer`  
Confidence: **Medium**  
Related requirements: REQ-007  
Evidence types: `code`, `tests-found`  
Evidence references: `Proposal058Tests.swift:694-720`.

Why it matters: The earlier direct-snapshot gap is closed for P031, but the current guard verifies the P031 presentation field shape. Future generated/readback presenters could reintroduce direct UI snapshot construction unless the guard pattern remains in place.

Recommended action: Keep source or type-level tests that fail if UI-facing readback surfaces outside `EscalationReadAdapter` construct/export `EscalationSnapshot`.

Acceptance criteria: Adding a direct `EscalationSnapshot` field or `EscalationSnapshot.build` call outside the adapter-owned path fails a focused P058 guard test.

### READY-001 [Major] Required P058 runtime, fixture, and release-closeout evidence is still incomplete

Reviewer: `observability_rollout_reviewer`  
Confidence: **High**  
Related requirements: REQ-017  
Evidence types: `proposal`, `tests-found`, `tests-run`  
Evidence references: proposal lines 32-33 and 195-205; targeted search over `Chainworks Forge`, `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.

Why it matters: The canonical proposal gate passes, but the release-closeout evidence named by P058 is not complete. Without visual/runtime, accessibility, restoration, multi-window, contrast, reduced-motion, trending, and drill artifacts, the implementation cannot be closed out as ready.

Recommended action: Produce or attach the missing release evidence, or move it to a concrete follow-up proposal if the project intentionally defers it.

Acceptance criteria: The P058 closeout evidence set includes remote visual/runtime proof, Full Keyboard Access order, scene restoration, multi-window shared publisher, contrast/reduced-motion fixtures, long-run metric-threshold trending, and operational drill artifacts.

## Readiness Checklist

| Gate | Status | Evidence |
| --- | --- | --- |
| Proposal file exists and is active | Pass | `docs/proposals/058-configurable-agent-escalation-chains.md:13` |
| Prior proposal-review selection discovered | None | helper returned no artifacts |
| Current implementation target identified | Pass | branch `main`, HEAD `7bc4c43810eca2a60835cc8edeaab984d2c7f896` |
| Same-tree canonical proposal gate | Pass | `./scripts/test-gate.sh proposal-058` |
| Swift P058 focused tests | Pass | observed 32 tests, 0 failures |
| Control-plane P058 focused tests/builds | Pass | final gate line: `Proposal 058 control-plane gate passed` |
| Adapter sole-source blocker from prior audits | Pass | P031 exports chains/trace only; P031 presentation mirror test guards no direct snapshot field |
| Registry observer recompute on adapter snapshot | Pass | `Proposal058Tests.swift:490-522` |
| MenuBarExtra compact P058 count | Pass | `Chainworks_ForgeApp.swift:93-105`; `Proposal058Tests.swift:393-412` |
| MenuBarExtra list cap/sort/overflow | Pass for list presenter | `Proposal058Tests.swift:440-476` |
| MenuBarExtra exact empty/overflow action | Partial | empty string and overflow command semantics differ |
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
git log --oneline --decorate --no-renames 926e205a2a650643eba288fcd659d8036cf59073..HEAD
git show --stat --oneline --decorate --no-renames 926e205a2a650643eba288fcd659d8036cf59073..HEAD
git diff --name-only 926e205a2a650643eba288fcd659d8036cf59073..HEAD
git diff --no-renames 926e205a2a650643eba288fcd659d8036cf59073..HEAD -- "Chainworks Forge/Chainworks_ForgeApp.swift" "Chainworks Forge/Engine/NotificationService.swift" "Chainworks ForgeTests/Proposal058Tests.swift"
rg -n "Full Keyboard|Keyboard Access|scene restoration|multi-window|contrast|Reduced Motion|reduced motion|requestUserAttention|p058EscalationAttentionCount|No paused escalation runs|Show all paused runs|MenuBarExtra|menu bar|p058" "Chainworks Forge" "Chainworks ForgeTests" docs/reference docs/evidence -g "*.swift" -g "*.md"
rg -n "EscalationStatusCapsule|EscalationPauseCard|EscalationCommandMirrorRow|DriftReviewSheet|EscalationLineageView|EscalationMenuBarPresenter|emptyTitle|overflow" "Chainworks Forge/Views/EscalationReadSurfaceViews.swift" "Chainworks ForgeTests/Proposal058Tests.swift"
./scripts/test-gate.sh proposal-058
```

Important verification results:

- Worktree was clean before writing this report.
- Report path helper returned this R10 path.
- Prior proposal-review helper returned no artifacts.
- Current audited HEAD is `7bc4c43810eca2a60835cc8edeaab984d2c7f896`.
- Delta from R9 is limited to the compact count fix, its tests, and the prior R9 audit file.
- `./scripts/test-gate.sh proposal-058` passed. The Swift P058 suite reported 32 passing tests; the control-plane gate finished with `Proposal 058 control-plane gate passed`.
- Gate output still includes existing Rust warning noise and Swift actor-isolation warnings in `EscalationReadSurfaceViews.swift`, but no failure.

## Final Action Items

1. Prove or implement live all-run escalation attention source coverage for non-selected runs entering/leaving every P058 attention state.
2. Finish exact MenuBarExtra empty/overflow command semantics.
3. Finish explicit macOS component contracts for status capsule, pause card, command row, drift sheet, and lineage layout/accessibility.
4. Clear the P058 MenuBarExtra presenter actor-isolation warnings.
5. Add or attach P058-specific release-closeout evidence for remote visual/runtime behavior, Full Keyboard Access, scene restoration, multi-window shared publisher, contrast, reduced motion, long-run metric thresholds, and operational drills.

## Final Verdict

P058 is **Partially Implemented** and **Not Ready** for full implementation closeout.

R10 is materially better than R9 because the compact MenuBarExtra count now uses a P058-specific aggregate and is covered by a mixed-attention regression test. Remaining blockers are all narrower than the old compact-count bug but still concrete: all-run source freshness is not proven, exact MenuBarExtra empty/overflow semantics remain partial, component contracts remain partial, Swift concurrency warnings remain in the P058 menu presenter, and required runtime/release evidence is still missing.
