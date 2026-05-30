# Proposal 058 Implementation Audit R11

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Report | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R11.md` |
| Audit timestamp | 2026-05-29T04:23:36Z |
| Worktree | `/Users/user/Documents/Chainworks Forge` |
| Target branch | `main` |
| Target HEAD | `70ab4d9d246714e4b7854dc89a127e0ed7b25242` (`Close P058 menu overflow contracts`) |
| Merge base with `origin/main` | `70ab4d9d246714e4b7854dc89a127e0ed7b25242` |
| Compare basis | Implicit current worktree; R11 inspected delta from prior audited baseline `7bc4c43810eca2a60835cc8edeaab984d2c7f896..HEAD` |
| Worktree before report write | Clean |
| Proposal state | Active (`Status: refined_after_write_boundary_blocker_resolved`) |

## Verdict

Overall conformance: **Partially Implemented**

Overall implementation readiness: **Not Ready**

Reviewer-selection reuse: **Not reused**. No prior proposal-review artifacts were found. Prior implementation audits were used only as historical context.

Audit confidence: **High** for the R11 MenuBarExtra code/test delta; **Low** for current-build verification because the canonical `proposal-058` gate aborts before tests start on this machine.

R11 materially improves the R10 state. The MenuBarExtra presenter now uses the exact empty title `No paused escalation runs`, exposes the overflow title `Show all paused runs...`, tracks overflow run IDs, renders overflow as an actionable button, wires that button to the Runs tab, and marks previously warning-producing presentation helpers `nonisolated`.

The implementation is still not ready for full P058 closeout. The canonical proof gate could not be completed: `./scripts/test-gate.sh proposal-058` aborted twice at `xcodebuild` startup with DVT file-event/cache I/O errors before any Swift test executed. Separately, all-run live attention source coverage, component layout/accessibility contracts, and required runtime/release evidence remain incomplete or unproven.

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

- `70ab4d9d Close P058 menu overflow contracts`

Changed files in `7bc4c438..HEAD`:

- `Chainworks Forge/Chainworks_ForgeApp.swift`
- `Chainworks Forge/Views/EscalationReadSurfaceViews.swift`
- `Chainworks ForgeTests/Proposal058Tests.swift`
- `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R10.md`

Material improvements since R10:

- `Chainworks_ForgeApp.swift:85-98` wires the MenuBarExtra overflow action to select the Runs tab.
- `EscalationReadSurfaceViews.swift:604-681` adds `onShowAllPausedRuns`, renders the overflow as a button, and disables it only when no callback is supplied.
- `EscalationReadSurfaceViews.swift:685-748` adds `overflowRunIDs`, exact `emptyTitle`, and exact `overflowTitle` to the presenter contract.
- `EscalationReadSurfaceViews.swift:8-80` and `EscalationReadSurfaceViews.swift:726-810` mark pure presentation helpers `nonisolated`, addressing the R10 actor-isolation warning source in code.
- `Proposal058Tests.swift:444-484` asserts exact empty/overflow titles and overflow run IDs.

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

- MenuBarExtra compact count remains P058-specific through `NotificationService.p058EscalationAttentionCount`.
- MenuBarExtra content now has exact empty-state text, exact overflow title, five-row cap, latest-update sort, active-escalation filtering, overflow count, and overflow run IDs.
- MenuBarExtra overflow is now represented as an actionable button and the app wires it to the Runs tab.
- The presenter helpers that produced R10 actor-isolation warnings are now marked `nonisolated` in code.
- P031 still does not expose a direct `EscalationSnapshot`; adapter snapshot derivation remains centralized in `EscalationReadAdapter`.
- Registry attention observers recompute attention snapshots as registered adapter snapshots change.
- Informational user attention request/cancel behavior, trace pasteboard copy, lineage retry collapse, and shadow-row display logic remain present in code/tests.

### Divergences

- The current canonical P058 gate is **not verified**. Both attempts aborted before tests started due local `xcodebuild` DVT file-event/cache I/O errors.
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
- Because `xcodebuild` aborted before compilation, R11 could not verify that the new `nonisolated` annotations fully clear the prior Swift actor-isolation warnings.

## Residual Scope / Follow-up Ownership

| Residual item | Current owner | Concrete follow-up proposal? | Blocks conformance/readiness? |
| --- | --- | --- | --- |
| Canonical `proposal-058` gate completion on current HEAD | P058 implementation / local verification environment | None found | Blocks readiness |
| Live source coverage for every run in P058 attention states, beyond registered/refresh-populated adapters | P058 implementation | None found | Blocks both |
| Status capsule, pause card, command row, drift sheet, and lineage detailed layout/accessibility contracts | P058 implementation | None found | Blocks both |
| Scene restoration and multi-window shared-adapter fixtures | P058 release closeout | None found | Blocks readiness and leaves REQ-008 partial |
| Remote visual/runtime evidence, Full Keyboard Access, contrast, and reduced-motion evidence | P058 release closeout | None found | Blocks readiness and leaves REQ-017 partial |
| Long-run metric-threshold trending and operational drill artifacts | P058 release closeout | None found | Blocks full release-closeout evidence |

The proposal labels several evidence items as release-closeout items rather than missing backend implementation paths. Under the implementation-audit tail gate, they still prevent `Overall Conformance = Implemented` and `Overall Implementation Readiness = Ready` unless completed or moved to a concrete follow-up proposal.

## Reviewer Selection

Selected reviewers:

| Reviewer | Why selected | Scope audited |
| --- | --- | --- |
| `apple_arch_reviewer` | P058 locks adapter ownership, MainActor publication, shared `run_id` registry, and no local truth reconstruction. | Adapter registry observers, Runs Home model subscription, menu data flow, actor-isolation source fix. |
| `macos_ui_reviewer` | P058 has detailed macOS component, menu, keyboard, focus, density, and visual contracts. | MenuBarExtra, status capsule, lineage, pause card, command row, drift sheet, fixtures. |
| `api_contract_reviewer` | P058 is a cross-boundary DTO/readback contract with GraphQL/MCP/report parity and raw-string compatibility. | P031 readback shape, Swift DTO presentation boundary, focused gate coverage. |
| `observability_rollout_reviewer` | P058 depends on metrics, kill switch, rollout stages, release-closeout evidence, and operational drills. | Gate evidence, metric declaration proof, residual release evidence. |
| `rust_reliability_reviewer` | Backend P058 still owns retry, pause, capacity, force-detach, idempotency, recovery, and runtime facts. | Canonical gate status and unchanged Rust delta. |

Rejected close alternatives:

- `apple_ux_reviewer`: UX/accessibility concerns are explicit, but the current delta is primarily menu/component implementation and is covered by `macos_ui_reviewer`.
- `rust_arch_reviewer`: no new Rust architecture delta appeared in `7bc4c438..HEAD`.
- `rust_security_reviewer`: no new auth/secret/public parsing surface appeared in this delta.
- `rust_performance_reviewer`: no new benchmark or hot-path performance claim was introduced.
- `product_reviewer`: no separate product metric decision was needed for this audit round.

## Requirement Summary

| Requirement | Status |
| --- | --- |
| REQ-001 Policy/tier schema and compile validation | Implemented; not reverified in R11 |
| REQ-002 Durable ledger/runtime facts/event journal/readback | Implemented; not reverified in R11 |
| REQ-003 Caller-appropriate GraphQL/MCP/report readback | Implemented; not reverified in R11 |
| REQ-004 Redaction and sensitive-field exclusion | Implemented; not reverified in R11 |
| REQ-005 Metrics/observability declarations | Implemented; not reverified in R11 |
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
| REQ-018 Current canonical proof gate | Not Verifiable |

## Detailed Requirement Audit

### REQ-001 Policy/tier schema and compile validation

Status: **Implemented; not reverified in R11**  
Evidence: `code-unchanged`, `prior-tests-run`  
Evidence references: no `control-plane/` delta in `7bc4c438..HEAD`; R10 recorded a passing `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Control-plane schema, policy, and compile validation behavior is unchanged in the R11 delta.  
Gap / note: Current canonical gate did not reach Rust verification because Swift `xcodebuild` aborted first.

### REQ-002 Durable ledger/runtime facts/event journal/readback

Status: **Implemented; not reverified in R11**  
Evidence: `code-unchanged`, `prior-tests-run`  
Evidence references: no `control-plane/` delta in `7bc4c438..HEAD`; R10 recorded a passing `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Durable escalation ledger/runtime facts/event journal behavior remains unchanged in the R11 delta.  
Gap / note: Current same-tree gate did not complete.

### REQ-003 Caller-appropriate GraphQL/MCP/report readback

Status: **Implemented; not reverified in R11**  
Evidence: `code-unchanged`, `prior-tests-run`  
Evidence references: no GraphQL/MCP boundary delta in `7bc4c438..HEAD`; R10 recorded a passing gate.  
Implementation mapping: GraphQL run detail readback includes P058 escalation chains and redacted trace; presenter preserves chains for adapter-owned presentation.  
Gap / note: Current same-tree gate did not complete.

### REQ-004 Redaction and sensitive-field exclusion

Status: **Implemented; not reverified in R11**  
Evidence: `code`, `prior-tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:845-851`; `Proposal058Tests.swift:763-765`.  
Implementation mapping: Trace copy uses redacted JSON; no raw sensitive-field rendering was found in inspected P058 macOS components.  
Gap / note: Current same-tree gate did not complete.

### REQ-005 Metrics/observability declarations

Status: **Implemented; not reverified in R11**  
Evidence: `code-unchanged`, `prior-tests-run`  
Evidence references: no metrics/Rust delta in `7bc4c438..HEAD`; R10 recorded a passing gate including metric declaration proof.  
Implementation mapping: Required P058 metric names remain unchanged.  
Gap / note: Long-run threshold trending remains release-closeout evidence under REQ-017.

### REQ-006 Governed macOS drift write boundary is read-only

Status: **Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:854-899`; `Proposal058Tests.swift:300-306`.  
Implementation mapping: The sheet exposes copy/open/close actions and contains no governed macOS mutation call.  
Gap / note: The sheet is read-only, but structured diff content remains incomplete under REQ-016.

### REQ-007 `EscalationReadAdapter` is the sole governed UI source

Status: **Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadAdapter.swift:117-223`; `EscalationReadSurfaceViews.swift:924-946`; `Proposal058Tests.swift:702-728`.  
Implementation mapping: P031 passes DTO chains and redacted trace only; snapshot derivation lives in `EscalationReadAdapter`; inspector uses the registry adapter; tests verify no direct `escalationSnapshot` field on P031 run detail presentation.  
Gap / note: Future read surfaces still need to preserve the same boundary.

### REQ-008 Shared adapter keyed by `run_id` for all windows/inspectors

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadAdapter.swift:117-223`; `EscalationReadSurfaceViews.swift:924-946`; `Proposal058Tests.swift:487-530`.  
Implementation mapping: The registry returns one adapter per run ID and supports observers over registry attention snapshots.  
Gap / note: No scene-restoration or multi-window runtime fixture proves restored windows wait for the shared publisher or that all inspectors receive the same update in production. `applyVisibleRunChains` still removes adapters not in the visible run-ID set, which needs runtime proof that it cannot break separate inspectors/restored scenes.

### REQ-009 Dock/menu attention from live all-run adapter aggregation

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadAdapter.swift:204-223`; `RunsHomeView.swift:1512-1520`; `RunsHomeView.swift:1664-1700`; `RunsHomeView.swift:1816-1838`; `NotificationService.swift:161-174`; `Proposal058Tests.swift:487-530`.  
Implementation mapping: Registry observers notify attention subscribers on adapter snapshot changes, and the Runs Home model publishes those snapshots for `ContentView` to sync into `NotificationService`.  
Gap / note: The observer covers every registered adapter snapshot, but the implementation still does not prove live source coverage for every run in the proposal's attention states. Non-selected runs are populated through Runs Home/detail refresh paths rather than a demonstrated all-run live escalation subscription.

### REQ-010 Informational user attention request/cancel

Status: **Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `NotificationService.swift:270-283`; `Proposal058Tests.swift:590+`.  
Implementation mapping: P058 attention requests use `NSApp.requestUserAttention(.informationalRequest)` through injectable hooks and cancel on activation or pause clear.  
Gap / note: Current same-tree gate did not complete.

### REQ-011 MenuBarExtra badge/list/overflow/compact contract

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `verification-blocked`  
Evidence references: `Chainworks_ForgeApp.swift:85-112`; `EscalationReadSurfaceViews.swift:604-810`; `Proposal058Tests.swift:444-484`.  
Implementation mapping: Compact count uses the P058 aggregate; menu content has aggregate count, row cap, latest-update sort, active-escalation filter, row state pills, exact empty title, exact overflow title, overflow run IDs, and an actionable overflow button wired to Runs.  
Gap / note: The code/test contract is substantially complete, but current same-tree gate did not compile or execute the tests. The overflow action opens the Runs tab, but no runtime/UI fixture proves it focuses or filters the full paused-run set.

### REQ-012 Lineage retry collapse, disclosure, shadow rows, layout

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:282-428`; `Proposal058Tests.swift:628-650`.  
Implementation mapping: Retry collapse and shadow-row styling exist and are tested in source.  
Gap / note: Fixed columns, duration field, right-aligned monospace attempt/duration fields, narrow-width no-horizontal-scroll behavior, and expanded digest/runtime fact refs are not fully implemented or proven.

### REQ-013 Status capsule field order/color/suppression/truncation

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:86-114`; `Proposal058Tests.swift:288`.  
Implementation mapping: A compact status label, symbol, color, help text, and accessibility label exist.  
Gap / note: Visible field order does not show state, tier, and trigger as separate slots; collapse order, exact same-backend retry color states, and 24-character middle truncation are not proven by fixtures.

### REQ-014 Pause card countdown and responsive layout

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:431-482`; `Proposal058Tests.swift:291`.  
Implementation mapping: Pause title, body/action hint, runbook button, diagnostic copy button, and metadata strip exist.  
Gap / note: Countdown formatting and responsive breakpoints below 360pt/280pt are not implemented or proven.

### REQ-015 Command mirror disabled reason/truncation/state badge

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:484-514`.  
Implementation mapping: Command row title/subtitle/copy action exists.  
Gap / note: Disabled reason parity in subtitle/help/accessibility/tooltip, 48-character middle truncation, and optional state badge are not implemented or proven.

### REQ-016 Drift review structured diff and handoff details

Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:854-899`; `Proposal058Tests.swift:300-306`.  
Implementation mapping: Read-only sheet provides frozen/current hashes, copy acknowledgement command, open external workflow, close, and interactive dismiss disabled.  
Gap / note: It does not render tier added/removed/changed badges, max-chain-attempts delta, trigger deltas, run ID, or richer structured external handoff details.

### REQ-017 Required macOS fixtures and release evidence

Status: **Partially Implemented**  
Evidence: `tests-found`, `code`, `verification-blocked`  
Evidence references: `Proposal058Tests.swift`; targeted `rg` over `Chainworks Forge`, `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.  
Implementation mapping: P058 focused Swift tests exist for adapter source ownership, registry observer aggregation, menu presenter cap/sort/overflow, dock per-run count, compact P058 count separation, user attention hooks, trace pasteboard copy, and component construction paths.  
Gap / note: No P058-specific remote visual/runtime evidence, Full Keyboard Access fixture, scene-restoration fixture, multi-window fixture, contrast proof, reduced-motion proof, long-run metric-threshold trending, or operational drill artifact was found. Current same-tree gate did not execute.

### REQ-018 Current canonical proof gate

Status: **Not Verifiable**  
Evidence: `tests-run`  
Evidence references: two attempted `./scripts/test-gate.sh proposal-058` runs on 2026-05-29.  
Implementation mapping: The canonical gate is the proposal's current proof path.  
Gap / note: Both runs aborted before Swift tests began with `DVTFilePathFSEvents: Failed to start fs event stream`, `DVTDeveloperPaths: Failed to get length of DARWIN_USER_CACHE_DIR`, and `Abort trap: 6`.

## Reviewer / Lens Scorecard

| Lens | Score | Top risk | Confidence |
| --- | --- | --- | --- |
| Objective proposal conformance | Partial | Explicit macOS component/evidence requirements remain partial; current proof gate is blocked. | High |
| Apple architecture | Partial | Registry observer is improved, but all-run live source coverage and multi-window/restored-scene behavior are not proven. | Medium |
| macOS UI | Partial | MenuBarExtra is substantially improved; component layout/accessibility contracts remain incomplete. | High |
| API contract | Pass with residual guard | P031 adapter boundary is corrected; future read surfaces need the same guard pattern. | High |
| Observability/rollout | Partial | Release-closeout evidence, long-run trending, operational drills, and current gate completion are not present. | Medium |
| Rust reliability | Pass with verification caveat | No Rust delta in R11, but current canonical gate did not reach Rust checks. | Medium |
| Readiness | Not Ready | Verification blocker plus remaining major findings block closeout. | High |

## Routed Specialist Findings

### READY-001 [Major] Current canonical P058 gate aborts before tests start

Reviewer: `observability_rollout_reviewer`  
Confidence: **High**  
Related requirements: REQ-018  
Evidence types: `tests-run`  
Evidence references: two `./scripts/test-gate.sh proposal-058` attempts on 2026-05-29.

Why it matters: P058 names `./scripts/test-gate.sh proposal-058` as the current proof gate. Both R11 attempts aborted at `xcodebuild` startup before Swift tests ran, so the current HEAD is not same-tree verified.

Recommended action: Repair or rerun in a healthy macOS/Xcode environment, then rerun `./scripts/test-gate.sh proposal-058`.

Acceptance criteria: The canonical gate completes on current HEAD, including Swift P058 tests and the control-plane P058 gate.

### ARCH-001 [Major] All-run attention is observer-driven only after adapters are populated

Reviewer: `apple_arch_reviewer`  
Confidence: **Medium**  
Related requirements: REQ-008, REQ-009  
Evidence types: `code`, `tests-found`  
Evidence references: `EscalationReadAdapter.swift:204-223`, `RunsHomeView.swift:1664-1700`, `RunsHomeView.swift:1816-1838`, `Proposal058Tests.swift:487-530`.

Why it matters: The implementation recomputes attention when registered adapters change, but the proposal asks for live aggregation across runs in named attention states. The inspected code still relies on Runs Home/detail refresh paths to populate non-selected adapters. A run that changes escalation state without a refresh or adapter update can remain absent or stale in the badge/menu aggregation.

Recommended action: Add a dedicated all-run escalation attention read/subscription source, or prove Runs Home delivers all relevant run IDs and refreshes their escalation chains whenever any run enters/leaves a P058 attention state.

Acceptance criteria: A test or integration fixture shows a non-selected run entering and clearing each P058 attention state updates the registry observer, Dock badge, MenuBarExtra content, and user-attention token without manual notification-click handlers or selected-run refresh.

### UI-001 [Minor] MenuBarExtra overflow action is code-wired but not runtime-proven as full paused-run navigation

Reviewer: `macos_ui_reviewer`  
Confidence: **Medium**  
Related requirements: REQ-011  
Evidence types: `code`, `tests-found`, `verification-blocked`  
Evidence references: `Chainworks_ForgeApp.swift:85-98`, `EscalationReadSurfaceViews.swift:662-675`, `Proposal058Tests.swift:476-484`.

Why it matters: R11 fixes the exact overflow title and makes it actionable, but the callback selects the broad Runs tab. No UI/runtime fixture proves this actually presents the full paused/escalation-run set that the operator expects after choosing "Show all paused runs...".

Recommended action: Add a focused presenter/action test or UI fixture proving the overflow command opens the correct all-paused-runs view or applies the expected Runs tab filter.

Acceptance criteria: With more than five P058 attention runs, activating the overflow command presents all paused/escalation runs, not just an unrelated broad destination.

### UI-002 [Major] Component-specific macOS layout and accessibility contracts remain incomplete

Reviewer: `macos_ui_reviewer`  
Confidence: **High**  
Related requirements: REQ-012, REQ-013, REQ-014, REQ-015, REQ-016, REQ-017  
Evidence types: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:86-114`, `EscalationReadSurfaceViews.swift:282-514`, `EscalationReadSurfaceViews.swift:854-899`, `Proposal058Tests.swift:288-306`.

Why it matters: The proposal names field order, truncation, countdown, responsive breakpoints, command disabled reason parity, structured drift diff, keyboard focus, and fixture evidence. Current tests mostly prove construction and selected presentation helpers, not the required macOS behavior and layout details.

Recommended action: Finish the component implementations and add focused fixtures for field order, truncation, narrow widths, keyboard order, reduced motion, contrast, structured drift diff, and no-horizontal-scroll lineage.

Acceptance criteria: P058 tests or UI fixtures assert the exact component slots and states from proposal lines 95-152 and 195-205.

### API-001 [Minor] Adapter boundary guard should cover future presentation surfaces

Reviewer: `api_contract_reviewer`  
Confidence: **Medium**  
Related requirements: REQ-007  
Evidence types: `code`, `tests-found`  
Evidence references: `Proposal058Tests.swift:702-728`.

Why it matters: The earlier direct-snapshot gap is closed for P031, but the current guard verifies the P031 presentation field shape. Future generated/readback presenters could reintroduce direct UI snapshot construction unless the guard pattern remains in place.

Recommended action: Keep source or type-level tests that fail if UI-facing readback surfaces outside `EscalationReadAdapter` construct/export `EscalationSnapshot`.

Acceptance criteria: Adding a direct `EscalationSnapshot` field or `EscalationSnapshot.build` call outside the adapter-owned path fails a focused P058 guard test.

### READY-002 [Major] Required P058 runtime, fixture, and release-closeout evidence is still incomplete

Reviewer: `observability_rollout_reviewer`  
Confidence: **High**  
Related requirements: REQ-017  
Evidence types: `proposal`, `tests-found`, `verification-blocked`  
Evidence references: proposal lines 32-33 and 195-205; targeted search over `Chainworks Forge`, `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.

Why it matters: The proposal names visual/runtime, accessibility, restoration, multi-window, contrast, reduced-motion, trending, and drill artifacts as closeout evidence. Those artifacts were not found in R11, and the canonical gate did not complete.

Recommended action: Produce or attach the missing release evidence, or move it to a concrete follow-up proposal if the project intentionally defers it.

Acceptance criteria: The P058 closeout evidence set includes remote visual/runtime proof, Full Keyboard Access order, scene restoration, multi-window shared publisher, contrast/reduced-motion fixtures, long-run metric-threshold trending, and operational drill artifacts.

## Readiness Checklist

| Gate | Status | Evidence |
| --- | --- | --- |
| Proposal file exists and is active | Pass | `docs/proposals/058-configurable-agent-escalation-chains.md:13` |
| Prior proposal-review selection discovered | None | helper returned no artifacts |
| Current implementation target identified | Pass | branch `main`, HEAD `70ab4d9d246714e4b7854dc89a127e0ed7b25242` |
| Same-tree canonical proposal gate | Fail/Blocked | `xcodebuild` aborts before Swift tests |
| Swift P058 focused tests | Not run | gate aborts before test execution |
| Control-plane P058 focused tests/builds | Not run in R11 | gate aborts before reaching control-plane checks |
| Adapter sole-source blocker from prior audits | Pass by code | P031 exports chains/trace only; P031 presentation mirror test guards no direct snapshot field |
| Registry observer recompute on adapter snapshot | Pass by code/tests-found | `Proposal058Tests.swift:487-530` |
| MenuBarExtra compact P058 count | Pass by code/tests-found | `Chainworks_ForgeApp.swift:99-112`; `Proposal058Tests.swift:393-415` |
| MenuBarExtra exact empty/overflow labels | Pass by code/tests-found | `EscalationReadSurfaceViews.swift:741-748`; `Proposal058Tests.swift:476-484` |
| MenuBarExtra overflow action | Partial | button exists; full paused-run navigation not runtime-proven |
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
git log --oneline --decorate --no-renames -8
git show --stat --oneline --decorate --no-renames 7bc4c43810eca2a60835cc8edeaab984d2c7f896..HEAD
git diff --name-only 7bc4c43810eca2a60835cc8edeaab984d2c7f896..HEAD
git diff --no-renames 7bc4c43810eca2a60835cc8edeaab984d2c7f896..HEAD -- "Chainworks Forge/Chainworks_ForgeApp.swift" "Chainworks Forge/Views/EscalationReadSurfaceViews.swift" "Chainworks ForgeTests/Proposal058Tests.swift"
rg -n "Full Keyboard|Keyboard Access|scene restoration|multi-window|contrast|Reduced Motion|reduced motion|requestUserAttention|p058EscalationAttentionCount|No paused escalation runs|Show all paused runs|MenuBarExtra|menu bar|p058" "Chainworks Forge" "Chainworks ForgeTests" docs/reference docs/evidence -g "*.swift" -g "*.md"
./scripts/test-gate.sh proposal-058
./scripts/test-gate.sh proposal-058
```

Important verification results:

- Worktree was clean before writing this report.
- Report path helper returned this R11 path.
- Prior proposal-review helper returned no artifacts.
- Current audited HEAD is `70ab4d9d246714e4b7854dc89a127e0ed7b25242`.
- Delta from R10 is limited to MenuBarExtra overflow/empty/action wiring, nonisolated presentation helpers, tests for the new presenter contract, and the tracked R10 audit file.
- Both `./scripts/test-gate.sh proposal-058` attempts failed before tests started. The repeated failure signature was `DVTFilePathFSEvents: Failed to start fs event stream`, `DVTDeveloperPaths: Failed to get length of DARWIN_USER_CACHE_DIR`, then `Abort trap: 6`.
- Because the gate aborted before Swift tests, R11 did not produce a current passing Swift test count or control-plane gate result.

## Final Action Items

1. Rerun `./scripts/test-gate.sh proposal-058` in a healthy macOS/Xcode environment and attach the completed result.
2. Prove or implement live all-run escalation attention source coverage for non-selected runs entering/leaving every P058 attention state.
3. Add a runtime/UI fixture proving `Show all paused runs...` opens the full paused/escalation-run set.
4. Finish explicit macOS component contracts for status capsule, pause card, command row, drift sheet, and lineage layout/accessibility.
5. Add or attach P058-specific release-closeout evidence for remote visual/runtime behavior, Full Keyboard Access, scene restoration, multi-window shared publisher, contrast, reduced motion, long-run metric thresholds, and operational drills.

## Final Verdict

P058 is **Partially Implemented** and **Not Ready** for full implementation closeout.

R11 closes the R10 MenuBarExtra exact empty/overflow label gap at the code/test-contract level and likely addresses the presenter actor-isolation warning source. It still cannot be accepted as ready because the canonical proof gate aborts before tests, all-run source freshness is not proven, full paused-run overflow navigation is not runtime-proven, component contracts remain partial, and required runtime/release evidence is still missing.
