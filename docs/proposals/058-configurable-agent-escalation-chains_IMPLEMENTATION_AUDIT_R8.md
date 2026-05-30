# Proposal 058 Implementation Audit R8

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Report | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R8.md` |
| Audit timestamp | 2026-05-28T18:47:02Z |
| Worktree | `/Users/user/Documents/Chainworks Forge` |
| Target branch | `main` |
| Target HEAD | `9f60c7088ed7fe11233356ba9b5594ddd2d49807` (`Close P058 escalation adapter gaps`) |
| Merge base with `origin/main` | `9f60c7088ed7fe11233356ba9b5594ddd2d49807` |
| Compare basis | Implicit current worktree; R8 inspected delta from prior audited baseline `64bd78d57365d205010bac3d72833ea461fe737c..HEAD` |
| Worktree before report write | Clean |
| Proposal state | Active (`Status: refined_after_write_boundary_blocker_resolved`) |

## Verdict

Overall conformance: **Partially Implemented**

Overall implementation readiness: **Not Ready**

Reviewer-selection reuse: **Not reused**. No prior proposal-review artifacts were found. Prior implementation audits were used only as historical context, not as reviewer-selection input.

Audit confidence: **High** for code-level adapter ownership, notification counting, and backend/control-plane gate status; **Medium** for macOS visual/runtime fidelity because no runtime screenshot, remote UI run, Full Keyboard Access fixture, scene-restoration fixture, or multi-window UI evidence was produced in this audit.

R8 closes the highest-severity R7 architecture blocker. `P031RunDetailPresentation` no longer exports a direct `escalationSnapshot`, the P031 presenter now passes only escalation chains and redacted trace through to the adapter-owned UI path, and the P058 test suite includes a source scan proving `P031ThinGraphQLReadBoundary.swift` does not call `EscalationSnapshot.build`. Dock attention counting also now counts one contributing run once rather than summing paused chains plus drift/kill-switch conditions.

The implementation is still not ready for P058 closeout. The current macOS aggregation is scoped to Runs Home visible rows and selected-run refreshes, while the proposal commits to live aggregation across runs in specific attention states and recomputation on every adapter snapshot. The MenuBarExtra contract and multiple component-level layout/accessibility contracts remain incomplete, and the proposal's fixture/release-closeout evidence is still only partially present.

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

- `9f60c708 Close P058 escalation adapter gaps`

Changed files in `64bd78d5..HEAD`:

- `Chainworks Forge/ContentView.swift`
- `Chainworks Forge/Engine/EscalationReadAdapter.swift`
- `Chainworks Forge/Engine/NotificationService.swift`
- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- `Chainworks Forge/Views/RunsHomeView.swift`
- `Chainworks ForgeTests/Proposal058Tests.swift`
- `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R7.md`

Material improvements since R7:

- `P031ThinGraphQLReadBoundary.swift:5680-5681` exports `escalationChains` plus `escalationTraceJSONRedacted`; it no longer exports `escalationSnapshot`.
- `P031ThinGraphQLReadBoundary.swift:6650-6684` maps GraphQL escalation readback into chain arrays and redacted trace only.
- `Proposal058Tests.swift:626-636` scans the P031 source and asserts it contains neither `let escalationSnapshot` nor `EscalationSnapshot.build`.
- `NotificationService.swift:159-170` counts P058 attention per run snapshot and refreshes dock/menu/user attention from that count.
- `Proposal058Tests.swift:391-414` proves two paused chains on one run contribute one dock-badge unit.
- `EscalationReadAdapter.swift:143-155` adds `applyVisibleRunChains`, and `RunsHomeView.swift:1673-1691` refreshes registry snapshots for available Runs Home rows.
- `ContentView.swift:132-140` and `ContentView.swift:247-250` sync notification state from `runsModel.escalationAttentionSnapshots`.

## Proposal Contract Summary

P058 commits to a cross-stack escalation system:

- Rust control plane owns policy resolution, trigger classification, tier advancement, pause/resume legality, capacity checks, persistence, recovery, and kill-switch behavior.
- GraphQL, MCP, reports, and macOS readback expose forward-compatible raw strings and redacted, caller-appropriate escalation state.
- Governed macOS is read/subscription presentation only. It must not become an escalation lifecycle authority.
- `EscalationReadAdapter` is the sole governed UI source for run detail, inspectors, notifications, shortcuts, command enablement, trace copy, banner state, pause cards, and lineage views.
- All windows and inspectors for the same run subscribe to one shared adapter keyed by `run_id`.
- Dock badge and human-tier attention derive from live aggregation across runs in paused/exhausted/force-detached/recovery/policy-drift states and are recomputed on every adapter snapshot.
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
- P031 no longer constructs or exports a direct `EscalationSnapshot`; this substantially restores the proposal's adapter-source boundary for the current production read path.
- Governed drift sheet remains read-only; no macOS mutation call was found in the sheet.
- Trace pasteboard copy is tested for `.string` and `public.json` atomicity.
- Registry-backed adapter routing exists for selected run-detail sync, inspector rendering, visible-run snapshot replacement, and notification snapshot source.
- Informational user attention request/cancel behavior is implemented and tested.
- Lineage retry collapse and shadow-row display logic exist and are tested.
- Dock attention count now treats each contributing run snapshot as one unit.

### Divergences

- Dock/menu attention aggregation is driven by Runs Home visible rows and selected-run refresh paths, not proven as live aggregation across all runs in the proposal's attention states.
- `EscalationMenuBarList` still lacks the exact empty state, at-most-five row cap, most-recent transition sort, overflow row, compact state-pill/count semantics, and consistent active-escalation filtering.
- `EscalationStatusCapsule` renders a single state label; tier and trigger are only in help text for non-compact density, so the explicit field order and collapse behavior are incomplete.
- `EscalationPauseCard` lacks the proposal's countdown formatting and narrow-width responsive fallbacks.
- `EscalationCommandMirrorRow` lacks disabled reason parity in subtitle/help/accessibility/tooltip, 48-character middle truncation, and optional state badge support.
- `DriftReviewSheet` shows only frozen/current hashes and external command controls, not structured tier/attempt/trigger/run-id diff details.
- `EscalationLineageView` has retry collapse and shadow styling, but does not prove the fixed column policy, right-aligned monospace duration/attempt fields, narrow-width no-scroll behavior, or expanded digest/runtime fact refs required by the proposal.

### Ambiguities / Evidence Gaps

- No runtime screenshot, remote UI run, or snapshot fixture evidence was produced for P058 macOS visual fidelity.
- No P058-specific Full Keyboard Access tab-order fixture was found.
- No P058-specific scene-restoration fixture proving restored windows wait for the shared adapter publisher was found.
- No P058-specific multi-window fixture proving all inspectors receive the same adapter update was found.
- No contrast or reduced-motion fixture evidence was found for the P058 escalation components.
- No evidence proves MenuBarExtra row recency sorting by escalation transition because the current snapshot model does not expose a clear sort key for that list.

## Residual Scope / Follow-up Ownership

| Residual item | Current owner | Concrete follow-up proposal? | Blocks conformance/readiness? |
| --- | --- | --- | --- |
| Live all-run dock/menu aggregation across the proposal's paused/exhausted/force-detached/recovery/policy-drift states | P058 implementation | None found | Blocks both |
| MenuBarExtra badge/list/overflow/empty-state/compact contract | P058 implementation | None found | Blocks both |
| Status capsule, pause card, command row, drift sheet, and lineage detailed layout/accessibility contracts | P058 implementation | None found | Blocks both |
| Scene restoration and multi-window shared-adapter fixtures | P058 release closeout | None found | Blocks readiness and leaves REQ-008 partial |
| Remote visual/runtime evidence, Full Keyboard Access, contrast, and reduced-motion evidence | P058 release closeout | None found | Blocks readiness and leaves REQ-017 partial |
| Long-run metric-threshold trending and operational drill artifacts | P058 release closeout | None found | Blocks full release closeout evidence, but not the core code path already proven by the focused gate |

The proposal itself labels several items as release-closeout evidence rather than missing implementation paths. Under the implementation-audit tail gate, they still prevent `Overall Conformance = Implemented` and `Overall Implementation Readiness = Ready` unless they are completed or moved to a concrete follow-up proposal.

## Reviewer Selection

Selected reviewers:

| Reviewer | Why selected | Scope audited |
| --- | --- | --- |
| `apple_arch_reviewer` | The proposal locks adapter ownership, MainActor publication, shared `run_id` registry, and no local truth reconstruction. | P031 presenter, adapter registry, Runs Home model, notification sync. |
| `macos_ui_reviewer` | The proposal has detailed macOS component, menu, keyboard, focus, density, and visual contracts. | Escalation status, lineage, pause card, command row, menu bar, drift sheet, fixtures. |
| `api_contract_reviewer` | P058 is a cross-boundary DTO/readback contract with GraphQL/MCP/report parity and raw-string compatibility. | P031 GraphQL readback shape, Swift DTO presentation boundary, focused gate coverage. |
| `observability_rollout_reviewer` | P058 depends on metrics, kill switch, rollout stages, release-closeout evidence, and operational drills. | Gate evidence, metric declaration proof, residual release evidence. |
| `rust_reliability_reviewer` | The backend slice owns retry, pause, capacity, force-detach, idempotency, recovery, and runtime facts. | Canonical P058 control-plane gate status and reliability residuals. |

Rejected close alternatives:

- `apple_ux_reviewer`: UX/accessibility concerns are explicit, but `macos_ui_reviewer` covers the concrete component and keyboard contracts for this audit round.
- `rust_arch_reviewer`: backend architecture is already exercised by the proposal gate; no new Rust architecture delta appeared in `64bd78d5..HEAD`.
- `rust_security_reviewer`: redaction and forbidden-field checks are relevant, but no new auth/secret/public parsing surface appeared in this delta.
- `rust_performance_reviewer`: no new benchmark or hot-path performance claim was introduced in this delta.
- `product_reviewer`: product value is represented by the P058 operator flow and release-closeout evidence, but no separate product metric decision was needed for the current audit.

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
Gap / note: No R8 regression observed.

### REQ-002 Durable ledger/runtime facts/event journal/readback

Source: Goals and Architecture persistence/readback commitments.  
Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: `./scripts/test-gate.sh proposal-058` passed; observed runtime facts, claim/start, recovery, schema, and readback groups passing.  
Implementation mapping: Durable escalation ledger/runtime facts/event journal behavior remains covered by the focused control-plane gate.  
Gap / note: No R8 regression observed.

### REQ-003 Caller-appropriate GraphQL/MCP/report readback

Source: Goals and boundary readback commitments.  
Status: **Implemented**  
Evidence: `tests-run`, `code`  
Evidence references: `P031ThinGraphQLReadBoundary.swift:6650-6684`; `Proposal058Tests.swift:572-624`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: GraphQL run detail readback includes P058 escalation chains and redacted trace; presenter now preserves chains for adapter-owned presentation.  
Gap / note: No caller-visibility regression observed in the gate.

### REQ-004 Redaction and sensitive-field exclusion

Source: Notifications forbidden fields and readback redaction commitments.  
Status: **Implemented**  
Evidence: `tests-run`, `code`  
Evidence references: `Proposal058Tests.swift:638+`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Trace copy uses redacted JSON; P058 gate includes payload-shape, credential/path rejection, and security readback checks.  
Gap / note: No raw sensitive-field rendering was found in inspected P058 macOS components.

### REQ-005 Metrics/observability declarations

Source: rollout/metrics commitments.  
Status: **Implemented**  
Evidence: `tests-run`, `telemetry`  
Evidence references: `./scripts/test-gate.sh proposal-058`; gate output included metric declaration coverage.  
Implementation mapping: Required P058 metric names remain declared and covered by the canonical gate.  
Gap / note: Long-run threshold trending remains release-closeout evidence under REQ-017.

### REQ-006 Governed macOS drift write boundary is read-only

Source: macOS Authority Boundary and DriftReviewSheet Write Boundary.  
Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:687-732`; `Proposal058Tests.swift:638+`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: The sheet exposes copy/open/close actions and contains no governed macOS mutation call.  
Gap / note: The sheet is read-only, but its structured diff content remains incomplete under REQ-016.

### REQ-007 `EscalationReadAdapter` is the sole governed UI source

Source: macOS Authority Boundary, proposal lines 87-90.  
Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:20-36`; `EscalationReadAdapter.swift:120-175`; `P031ThinGraphQLReadBoundary.swift:5680-5681`; `P031ThinGraphQLReadBoundary.swift:6650-6684`; `EscalationReadSurfaceViews.swift:757-780`; `Proposal058Tests.swift:580-636`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: P031 now passes DTO chains and redacted trace only; snapshot derivation lives in `EscalationReadAdapter`; inspector uses the registry adapter; tests scan for forbidden direct P031 snapshot construction.  
Gap / note: This requirement is marked implemented for the audited production read path. Future read surfaces still need to preserve the same boundary.

### REQ-008 Shared adapter keyed by `run_id` for all windows/inspectors

Source: macOS Authority Boundary, proposal line 91; fixtures lines 199-200.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:120-181`; `EscalationReadSurfaceViews.swift:757-780`; `Proposal058Tests.swift:416-449`; `Proposal058Tests.swift:451+`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: The registry returns one adapter per run ID, can replace visible-run chains, and aggregates attention snapshots across multiple run IDs.  
Gap / note: No scene-restoration or multi-window runtime fixture proves restored windows wait for the shared publisher or that all inspectors receive the same update in production.

### REQ-009 Dock/menu attention from live all-run adapter aggregation

Source: Notifications Dock Badge and Human Tier Attention, proposal lines 207-214.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `RunsHomeView.swift:1541-1554`; `RunsHomeView.swift:1661-1691`; `RunsHomeView.swift:1810-1828`; `ContentView.swift:132-140`; `ContentView.swift:247-250`; `NotificationService.swift:159-170`; `Proposal058Tests.swift:391-449`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Runs Home refresh builds registry snapshots for available rows, notifications consume `runsModel.escalationAttentionSnapshots`, and the dock count is now per contributing run.  
Gap / note: Aggregation is not proven as live all-run coverage across all paused/exhausted/force-detached/recovery/policy-drift runs. Only the selected run has live subscriptions in inspected code, and non-selected row snapshots are refreshed through Runs Home/detail refresh loops rather than every adapter snapshot.

### REQ-010 Informational user attention request/cancel

Source: Human Tier Attention, proposal line 214.  
Status: **Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `NotificationService.swift:159-172`; `Proposal058Tests.swift:485+`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: P058 attention requests use `NSApp.requestUserAttention(.informationalRequest)` through injectable hooks and cancel on activation or pause clear.  
Gap / note: No R8 regression observed.

### REQ-011 MenuBarExtra badge/list/overflow/compact contract

Source: MenuBarExtra, proposal lines 156-161; density rules lines 164-175.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:604-644`; `NotificationService.swift:159-170`; `Proposal058Tests.swift:391-414`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Menu-bar enablement and attention snapshots exist; dock count is per run.  
Gap / note: List text still says `No paused escalation chains` instead of `No paused escalation runs`; rows are not capped at five, no overflow row exists, sorting is not by most-recent escalation transition, compact item state-pill/count behavior is not proven, and menu list filtering excludes `hasActiveEscalation` even though registry attention includes it.

### REQ-012 Lineage retry collapse, disclosure, shadow rows, layout

Source: EscalationLineageView, proposal lines 124-130.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:282-355`; `Proposal058Tests.swift:565-570`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Retry collapse and shadow-row styling exist and are tested.  
Gap / note: Fixed columns, right-aligned monospace attempt/duration fields, narrow-width no-horizontal-scroll behavior, and expanded digest/runtime fact refs are not fully implemented or proven.

### REQ-013 Status capsule field order/color/suppression/truncation

Source: EscalationStatusCapsule, proposal lines 139-152.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:86-114`; `Proposal058Tests.swift`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: A compact status label, symbol, color, help text, and accessibility label exist.  
Gap / note: Visible field order does not show state, tier, and trigger as separate slots; collapse order is not proven; exact same-backend retry color states and 24-character middle truncation are not proven by fixtures.

### REQ-014 Pause card countdown and responsive layout

Source: EscalationPauseCard, proposal lines 131-138.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:431-482`; `./scripts/test-gate.sh proposal-058`.  
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
Evidence references: `EscalationReadSurfaceViews.swift:687-732`; `Proposal058Tests.swift:638+`; `./scripts/test-gate.sh proposal-058`.  
Implementation mapping: Read-only sheet provides frozen/current hashes, copy acknowledgement command, open external workflow, close, and interactive dismiss disabled.  
Gap / note: It does not render tier added/removed/changed badges, max-chain-attempts delta, trigger deltas, run ID, or richer structured external handoff details.

### REQ-017 Required macOS fixtures and release evidence

Source: Fixtures, Notifications, and Implementation Sync, proposal lines 32-33 and 195-205.  
Status: **Partially Implemented**  
Evidence: `tests-found`, `tests-run`, `code`  
Evidence references: `Proposal058Tests.swift`; `./scripts/test-gate.sh proposal-058`; targeted `rg` over `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.  
Implementation mapping: P058 focused Swift tests cover adapter source ownership, registry aggregation, dock per-run count, user attention hooks, trace pasteboard copy, and several component construction paths.  
Gap / note: No P058-specific remote visual/runtime evidence, Full Keyboard Access fixture, scene-restoration fixture, multi-window fixture, contrast proof, reduced-motion proof, long-run metric-threshold trending, or operational drill artifact was found.

## Routed Specialist Findings

### ARCH-001 [Major] Visible-row aggregation is still narrower than the proposal's live all-run adapter aggregation

Reviewer: `apple_arch_reviewer`  
Evidence: `RunsHomeView.swift:1541-1554`, `RunsHomeView.swift:1661-1691`, `RunsHomeView.swift:1810-1828`, `ContentView.swift:132-140`, `NotificationService.swift:159-170`.

R8 adds meaningful visible-run aggregation, but the proposal requires dock/menu attention to derive from live aggregation across runs in the named attention states and to recompute on every adapter snapshot. The inspected app path refreshes snapshots for Runs Home rows and selected-run subscriptions; it does not prove all relevant runs are subscribed, continuously current, or recomputed from every adapter snapshot. This leaves stale or missing badge/menu attention possible when a non-selected run changes escalation state outside the refresh cycle.

### UI-001 [Major] MenuBarExtra still does not satisfy the promised badge/list/overflow contract

Reviewer: `macos_ui_reviewer`  
Evidence: `EscalationReadSurfaceViews.swift:604-644`; proposal lines 156-161 and 164-175.

The menu renders an uncapped list, uses the wrong empty-state copy, lacks overflow behavior after five rows, does not sort by most-recent escalation transition, and does not prove compact state-pill/count-only behavior. Its filter also omits `hasActiveEscalation`, while the registry attention filter includes it. This keeps REQ-011 partial even though the dock count fix landed.

### UI-002 [Major] Component-specific macOS layout and accessibility contracts remain incomplete

Reviewer: `macos_ui_reviewer`  
Evidence: `EscalationReadSurfaceViews.swift:86-114`, `EscalationReadSurfaceViews.swift:282-355`, `EscalationReadSurfaceViews.swift:431-514`, `EscalationReadSurfaceViews.swift:687-732`; proposal lines 95-152.

The status capsule, lineage view, pause card, command row, and drift sheet all implement useful slices, but they do not yet match the explicit field, truncation, countdown, responsive, structured diff, keyboard/focus, and accessibility evidence requirements in the proposal. These are not stylistic preferences; they are named P058 acceptance contracts.

### API-001 [Minor] Adapter boundary is now corrected, but future generated/readback surfaces need the same source-scan guard

Reviewer: `api_contract_reviewer`  
Evidence: `P031ThinGraphQLReadBoundary.swift:5680-5681`, `P031ThinGraphQLReadBoundary.swift:6650-6684`, `Proposal058Tests.swift:626-636`.

The R7 direct-snapshot gap is closed for P031. The remaining API-contract risk is regression containment: the current source scan protects `P031ThinGraphQLReadBoundary.swift`, but new readback presenters or generated adapters could reintroduce direct UI snapshot construction unless they are routed through the same guard pattern.

### READY-001 [Major] Required P058 runtime, fixture, and release-closeout evidence is still incomplete

Reviewer: `observability_rollout_reviewer`  
Evidence: proposal lines 32-33 and 195-205; targeted search over `Chainworks ForgeTests`, `docs/reference`, and `docs/evidence`.

The canonical proposal gate passes, but the release-closeout evidence named by the proposal is not complete. No P058-specific remote visual/runtime evidence, Full Keyboard Access order, scene restoration, multi-window shared-publisher proof, contrast/reduced-motion fixture, long-run threshold trending, or operational drill artifact was found. This blocks `Ready` even though the core backend/control-plane path is healthy.

## Readiness Checklist

| Gate | Status | Evidence |
| --- | --- | --- |
| Proposal file exists and is active | Pass | `docs/proposals/058-configurable-agent-escalation-chains.md:13` |
| Prior proposal-review selection discovered | None | helper returned no artifacts |
| Current implementation target identified | Pass | branch `main`, HEAD `9f60c7088ed7fe11233356ba9b5594ddd2d49807` |
| Same-tree canonical proposal gate | Pass | `./scripts/test-gate.sh proposal-058` |
| Swift P058 focused tests | Pass | observed 29 tests, 0 failures |
| Control-plane P058 focused tests/builds | Pass | final gate line: `Proposal 058 control-plane gate passed` |
| Adapter sole-source blocker from R7 | Pass | P031 source no longer exports direct snapshot; source-scan test added |
| Dock per-run attention count | Pass | `Proposal058Tests.swift:391-414` |
| Live all-run dock/menu aggregation | Partial | visible-row refresh exists; all-run live coverage not proven |
| MenuBarExtra UI contract | Partial | list component lacks cap/sort/overflow/exact empty state |
| Component UI/accessibility contract | Partial | several explicit slots/layout fixtures missing |
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
git show --stat --oneline --decorate --no-renames 64bd78d57365d205010bac3d72833ea461fe737c..HEAD
git log --oneline --decorate --no-renames 64bd78d57365d205010bac3d72833ea461fe737c..HEAD
rg -n "Full Keyboard|Keyboard Access|scene restoration|multi-window|contrast|Reduced Motion|requestUserAttention|Dock badge|Menubar|MenuBarExtra|p058" "Chainworks ForgeTests" docs/reference docs/evidence -g "*.swift" -g "*.md"
./scripts/test-gate.sh proposal-058
```

Important verification results:

- Worktree was clean before writing this report.
- Report path helper returned this R8 path.
- Prior proposal-review helper returned no artifacts.
- Current audited HEAD is `9f60c7088ed7fe11233356ba9b5594ddd2d49807`.
- `./scripts/test-gate.sh proposal-058` passed. The Swift P058 suite reported 29 passing tests; the control-plane gate finished with `Proposal 058 control-plane gate passed`.
- Gate output still includes existing Rust warning noise for unused/dead-code items, but no failure.

## Final Action Items

1. Complete live all-run attention aggregation so dock/menu attention is derived from every relevant run state, not just selected/visible refresh paths.
2. Finish MenuBarExtra badge/list semantics: exact empty copy, row cap, transition-recency sort, overflow row, compact state-pill/count behavior, and filter parity with registry attention.
3. Finish explicit macOS component contracts for status capsule, pause card, command row, drift sheet, and lineage layout/accessibility.
4. Add or attach P058-specific release-closeout evidence for remote visual/runtime behavior, Full Keyboard Access, scene restoration, multi-window shared publisher, contrast, reduced motion, long-run metric thresholds, and operational drills.
5. Preserve the R8 source-scan guard pattern for any future readback/presentation surface that could bypass `EscalationReadAdapter`.

## Final Verdict

P058 is **Partially Implemented** and **Not Ready** for full implementation closeout.

The current HEAD is materially better than R7: the direct P031 snapshot boundary breach is closed, per-run dock count is corrected, and visible-run adapter aggregation exists. The remaining blockers are now concentrated in live all-run aggregation semantics, MenuBarExtra/component UI fidelity, and required runtime/release evidence.
