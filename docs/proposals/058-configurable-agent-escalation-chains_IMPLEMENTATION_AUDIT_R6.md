# Proposal 058 Implementation Audit R6

Proposal: `docs/proposals/058-configurable-agent-escalation-chains.md`  
Audit date: 2026-05-28T17:44:05Z  
Target worktree: `/Users/user/Documents/Chainworks Forge`  
Target branch: `main`  
Target HEAD: `17efd85a10e29df13ac96c2f50f889034f2491e1` (`Close P058 macOS escalation UI gaps`)  
Merge base with `origin/main`: `17efd85a10e29df13ac96c2f50f889034f2491e1`  
Prior implementation audit context: R5 at `0559ff1afa298c4ce34512368c16b194c47ec8a5`; not used for reviewer selection.  
Generated report path: `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R6.md`

## Verdict

Overall conformance: **Partially Implemented**

Overall implementation readiness: **Not Ready for full P058 closeout / broad release**

The implementation is materially stronger than R5. The P058 gate passes, a macOS `MenuBarExtra` now exists, informational attention request/cancel behavior is implemented and tested, and lineage retry collapse plus shadow-row display logic has landed. Backend/control-plane escalation chains, redacted readback, boundary caller behavior, and focused regression coverage still look strong.

The remaining blockers are in the macOS governed read surface and readiness evidence. Production run detail still bypasses the proposal's sole-source `EscalationReadAdapter` contract; dock/menu attention is fed by the currently selected run detail rather than live aggregation across runs; and several explicit macOS layout/accessibility/component requirements remain partial or unproven.

## Prior Review Reuse

Reuse classification: **Not reused**

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

Prior implementation audits were treated as context only, per the audit workflow.

## Delta Since R5

Reviewed delta: `0559ff1afa298c4ce34512368c16b194c47ec8a5..17efd85a10e29df13ac96c2f50f889034f2491e1`

Changed files:

- `Chainworks Forge/Chainworks_ForgeApp.swift`
- `Chainworks Forge/ContentView.swift`
- `Chainworks Forge/Engine/NotificationService.swift`
- `Chainworks Forge/Support/AutomationFallbackAppDelegate.swift`
- `Chainworks Forge/Views/EscalationReadSurfaceViews.swift`
- `Chainworks ForgeTests/Proposal058Tests.swift`
- `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R5.md`

Material improvements:

- `Chainworks_ForgeApp.swift:85-101` adds a P058 `MenuBarExtra` driven by `NotificationService.shared`.
- `NotificationService.swift:43-64` injects request/cancel attention hooks and cancels P058 attention on app activation.
- `NotificationService.swift:159-170` applies escalation snapshots to dock/menu attention state.
- `NotificationService.swift:265-279` requests `.informationalRequest` and cancels the held token.
- `EscalationReadSurfaceViews.swift:203-235` adds retry-collapse display rows for repeated same-backend retries.
- `Proposal058Tests.swift:349-405` covers dock/menu aggregation and informational attention request/cancel behavior.
- `Proposal058Tests.swift:408-465` covers retry collapse and shadow-row marking.

## Proposal State And Contract Summary

P058 promises configurable escalation chains spanning Rust control-plane behavior, boundary readback, metrics/observability, rollout evidence, and a governed macOS read-only presentation layer.

The relevant macOS commitments are explicit, not polish-only:

- Proposal lines 87-92: `EscalationReadAdapter` is the sole governed UI source for run detail, inspectors, notifications, shortcuts, command enablement, trace copy, banner state, pause cards, and lineage views.
- Proposal lines 95-160: component-specific requirements for drift review, banner stack, command presentation, lineage, pause card, status capsule, trace timeline, and `MenuBarExtra`.
- Proposal lines 195-205: fixture requirements including presentation snapshots, scene restoration, multi-window shared publisher, dock aggregation, attention request/cancel, pasteboard atomicity, Full Keyboard Access order, and drift review no-mutation proof.
- Proposal lines 206-214: notifications/dock badge derived from live aggregation across runs, recomputed on every adapter snapshot, and informational user attention for backgrounded paused runs.

## Platform And Product Scope

In scope:

- Rust control-plane escalation policy/chains and redacted readback.
- GraphQL/MCP readback and caller boundary behavior.
- macOS read-only escalation surfaces, including run detail, inspector-style presentation, menu bar attention, dock badge attention, drift review handoff, and component accessibility.
- Focused proposal gate evidence and release closeout readiness.

Out of scope for this audit:

- Generic refactors unrelated to P058.
- Prior implementation audit files except as historical context.
- UI screenshot review; no screenshot/UI automation artifact was produced during this audit.

## Flow Audit

1. Backend escalation chain execution and readback: **Implemented / high confidence**. The P058 gate exercises control-plane readback, redaction, metrics, and caller boundary paths.
2. Operator run-detail read surface: **Partially Implemented**. Run detail receives escalation readback and renders P058 views, but production presentation constructs snapshots directly instead of routing through the adapter source promised by proposal lines 87-92.
3. Dock/menu/operator attention: **Partially Implemented**. A menu extra and informational attention request/cancel path exist, but they are driven from selected run detail snapshots rather than live all-run adapter aggregation.
4. Detailed macOS component contract: **Partially Implemented**. Lineage retry collapse and shadow-row handling improved. Status capsule, pause card, command rows, drift sheet, menu list behavior, and accessibility fixture coverage remain incomplete or unproven against explicit proposal text.
5. Release closeout: **Not Ready**. Focused gate passes, but unresolved proposal requirements have no named follow-up proposal owning the remaining scope.

## Requirement Conformance

| Requirement | Proposal commitment | Status | Evidence |
| --- | --- | --- | --- |
| REQ-001 | Configurable escalation policy/tier model exists and is wired through control-plane behavior. | Implemented | P058 gate passed; prior backend evidence remains intact at current HEAD. |
| REQ-002 | Escalation chain state is durably represented and exposed as redacted readback. | Implemented | P058 gate passed; `runs_get_escalation_readback_*` tests passed in `./scripts/test-gate.sh proposal-058`. |
| REQ-003 | Boundary callers receive class-appropriate escalation readback. | Implemented | P058 gate passed, including MCP readback tests for full vs summary readback. |
| REQ-004 | Sensitive fields/raw payloads are not exposed in governed read surfaces. | Implemented | P058 gate passed; `db` payload-shape tests passed during the gate. |
| REQ-005 | Metrics/observability surfaces exist for escalation decisions. | Implemented | Covered by prior implementation and focused gate; no regression observed in R6 delta. |
| REQ-006 | macOS read surface is read-only for governed drift acknowledgement. | Implemented for write boundary | `DriftReviewSheet` only exposes copy/open/close actions at `EscalationReadSurfaceViews.swift:687-732`; no mutation call found in the sheet. |
| REQ-007 | `EscalationReadAdapter` is the sole governed macOS UI source. | Partially Implemented | Adapter and registry exist, but production run detail builds snapshots directly in `P031ThinGraphQLReadBoundary.swift:6650-6652`; `rg` finds adapter usage only in its own file and tests. |
| REQ-008 | All windows/inspectors for the same run subscribe to a shared adapter keyed by `run_id`. | Partially Implemented | Registry exists and is tested, but production run detail does not subscribe through it. |
| REQ-009 | Dock badge/menu attention derives from live aggregation across runs and every adapter snapshot. | Partially Implemented | `NotificationService.applyP058EscalationSnapshots` exists at `NotificationService.swift:159-170`, but `ContentView.swift:238-245` feeds only the selected run detail snapshot. |
| REQ-010 | `NSApp.requestUserAttention(.informationalRequest)` fires for backgrounded paused runs and cancels on activation/clear. | Implemented for single-service path | Implemented at `NotificationService.swift:43-64` and `NotificationService.swift:265-279`; tested at `Proposal058Tests.swift:376-405`. |
| REQ-011 | `MenuBarExtra` shows numeric badge/count behavior, at most 5 sorted rows, overflow, compact state pill/count only, and empty state. | Partially Implemented | Menu extra exists at `Chainworks_ForgeApp.swift:85-101`; list at `EscalationReadSurfaceViews.swift:604-644` lacks cap/sort/overflow and compact label does not render the numeric count/state-pill contract. |
| REQ-012 | Lineage view supports retry collapse and shadow row styling. | Partially Implemented | Retry collapse and shadow-row flags landed at `EscalationReadSurfaceViews.swift:203-235` and tests at `Proposal058Tests.swift:408-465`; detailed column/min-width and disclosure content coverage remains partial. |
| REQ-013 | Status capsule renders field order, collapse order, color rules, raw ids in help/accessibility, and suppression rules. | Partially Implemented | `EscalationStatusCapsule` still renders a single label at `EscalationReadSurfaceViews.swift:86-102`; tier/trigger are not visibly rendered per field-order requirement. |
| REQ-014 | Pause card implements countdown formatting and responsive layout bounds. | Partially Implemented | `EscalationPauseCard` at `EscalationReadSurfaceViews.swift:431-468` has basic content/actions but no countdown or width-breakpoint behavior. |
| REQ-015 | Command mirror rows implement disabled reason rule, 48-char middle truncation, state badge, help/accessibility/tooltip parity. | Partially Implemented | `EscalationCommandMirrorRow` at `EscalationReadSurfaceViews.swift:484-514` is a basic copy row without these rules. |
| REQ-016 | Drift review sheet renders structured policy diff and external acknowledgement handoff details. | Partially Implemented | `DriftReviewSheet` at `EscalationReadSurfaceViews.swift:687-732` shows hash values and copy/open actions, but not tier added/removed/changed badges, max-chain-attempt deltas, trigger deltas, or run id. |
| REQ-017 | Required fixtures prove presentation, shared publisher, dock aggregation, attention cancellation, pasteboard atomicity, FKA tab order, and drift no-mutation. | Partially Implemented | Focused tests were added, but no P058-specific Full Keyboard Access, multi-window shared-publisher production test, screenshot fixture, or complete component fixture evidence was found. |

Because REQ-007 through REQ-017 remain partially implemented and have no concrete follow-up proposal owner, the audit cannot mark P058 as implemented or ready.

## Reviewer Routing

Selected reviewers:

- `apple_arch_reviewer`: production macOS source-of-truth and shared adapter contract.
- `macos_ui_reviewer`: explicit macOS component, attention, menu bar, accessibility, and platform-fit requirements.
- `rust_reliability_reviewer`: escalation execution/retry/readback reliability and regression gate interpretation.
- `api_contract_reviewer`: GraphQL/MCP readback and caller boundary behavior.
- `observability_rollout_reviewer`: metrics, gate evidence, and release closeout readiness.

Rejected reviewers:

- `rust_arch_reviewer`: backend architecture was not the primary R6 delta; reliability/API coverage was enough for this pass.
- `security_reviewer`: no new auth/credential handling delta was introduced in R6; redaction boundary remains covered by P058 gate tests.
- `performance_reviewer`: no hot-path performance change was introduced in R6.
- `product_reviewer`: remaining product issues are direct proposal-readiness gaps covered by macOS UI and rollout review.

## Specialist Scorecard

| Lens | Score | Rationale |
| --- | --- | --- |
| Apple architecture | Not Ready | Production UI still bypasses the adapter source promised by the proposal. |
| macOS UI/accessibility | Not Ready | New attention/menu/lineage pieces help, but several named component and accessibility contracts remain partial. |
| Rust reliability | Ready with risks | Focused gate passes; R6 did not introduce backend reliability changes. |
| API contract | Ready with risks | Readback/caller gate evidence passes; macOS consumption still has source-boundary drift. |
| Observability/rollout | Not Ready | Focused gate passes, but release closeout is blocked by unresolved proposal requirements. |

## Findings

### ARCH-001 [Major] Production macOS read path bypasses `EscalationReadAdapter`

Proposal lines 87-92 make `EscalationReadAdapter` the sole governed UI source for run detail, inspectors, notifications, shortcuts, command enablement, trace copy, banner state, pause cards, and lineage views. The current production run-detail presenter still constructs snapshots directly:

- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift:6650-6652`

```swift
let escalationSnapshot = detail.runEscalationReadback.map {
  EscalationSnapshot.build(runId: $0.runID, chains: $0.chains)
}
```

`ContentView.swift:238-245` then sends that selected run detail snapshot directly to `NotificationService`. A targeted search found `EscalationReadAdapter` and `EscalationReadAdapterRegistry` used in their own implementation and tests, but not as the production run-detail/notification source.

Impact: the implementation does not prove the shared per-`run_id` publisher, restored-scene loading state, off-main decode/MainActor publication boundary, or "all surfaces consume the adapter" contract. Multiple windows and notifications can drift from the proposal's single governed source.

Recommended action: route GraphQL escalation readback through `EscalationReadAdapterRegistry.adapter(for:)`, publish immutable presentation snapshots from that shared adapter, and make run detail, inspector, banner, pause, lineage, command, trace, dock, and menu consumers subscribe to the adapter source. Add a production test that fails if run detail constructs `EscalationSnapshot` directly outside the adapter/presenter boundary.

### UI-001 [Major] Menu/dock aggregation is selected-run detail, not live all-run adapter aggregation

Proposal lines 156-161 require a `MenuBarExtra` with numeric badge/count behavior, at most five rows sorted by most recent escalation transition, overflow after five rows, and compact state-pill/count-only behavior. Proposal lines 206-214 require dock badge attention to be derived from live aggregation across runs and recomputed on every adapter snapshot.

R6 adds a menu extra:

- `Chainworks Forge/Chainworks_ForgeApp.swift:85-101`

But the data source is selected run detail:

- `Chainworks Forge/ContentView.swift:238-245`

And the count formula counts paused chains plus drift/kill-switch flags within supplied snapshots:

- `Chainworks Forge/Engine/NotificationService.swift:159-170`

The menu list itself filters all supplied snapshots but does not cap at five, sort by transition recency, render overflow, or expose the compact numeric count/state-pill item:

- `Chainworks Forge/Views/EscalationReadSurfaceViews.swift:604-644`

Impact: a paused run that is not currently selected can be absent from dock/menu attention, and a single run with multiple paused chains plus drift/kill switch can be overcounted relative to the proposal's aggregate paused-run count intent.

Recommended action: feed `NotificationService` from an all-run escalation aggregation source keyed by adapter snapshots, count each attention run once for the menu/dock badge, sort rows by most recent transition, cap visible rows at five, and render the proposal's overflow item and compact item semantics.

### UI-002 [Major] Explicit macOS component and accessibility contracts remain partial

R6 improves lineage retry collapse and shadow markers, but other named component contracts remain below proposal fidelity:

- Status capsule renders one `Label` at `EscalationReadSurfaceViews.swift:86-102`; it does not visibly render the proposal's state pill, tier label, trigger label field order/collapse order.
- Pause card at `EscalationReadSurfaceViews.swift:431-468` lacks countdown formatting, min/ideal width behavior, and below-280pt one-line summary behavior.
- Command mirror row at `EscalationReadSurfaceViews.swift:484-514` lacks disabled reason placement in subtitle/help/accessibility/tooltip, 48-character middle truncation, and state badge.
- Drift sheet at `EscalationReadSurfaceViews.swift:687-732` lacks structured tier added/removed/changed badges, max-chain-attempt deltas, trigger deltas, and run-id details.
- No P058-specific proof was found for Full Keyboard Access order, complete VoiceOver order, contrast, reduced-motion transitions, multi-window shared adapter publication, or presentation snapshot coverage.

Impact: the macOS surface still does not match the explicit operator-facing proposal contract, and the lack of fixture proof makes regressions likely.

Recommended action: complete each component slot/rule from proposal lines 95-160, then add focused tests or screenshot/fixture evidence for density, keyboard order, accessibility labels, reduced motion, and shared adapter publication.

### READY-001 [Major] P058 still lacks a closeout path for unfinished scope

The focused gate now passes, but the proposal still contains promised behavior with only partial implementation and no named follow-up proposal that owns the unfinished macOS scope. The implementation-audit tail gate does not allow "future work" to satisfy proposal conformance.

Impact: P058 cannot be retired into reference docs as fully implemented without either completing the remaining scope or explicitly moving it to concrete follow-up proposals.

Recommended action: either complete the blocked macOS scope in P058 or create a specific follow-up proposal that narrows and owns the remaining adapter/aggregation/component fixture work. Until then, keep P058 active or mark it as partial in closeout materials.

## Residual Scope / Follow-up Ownership

| Residual item | Owner found? | Blocks conformance/readiness? |
| --- | --- | --- |
| Production `EscalationReadAdapter` as sole source for governed UI surfaces. | No named follow-up found. | Yes |
| All-run live adapter aggregation for dock badge and menu extra. | No named follow-up found. | Yes |
| Menu extra cap/sort/overflow/numeric badge/state-pill compact contract. | No named follow-up found. | Yes |
| Status capsule field-order/collapse/color/truncation proof. | No named follow-up found. | Yes |
| Pause card countdown and responsive layout behavior. | No named follow-up found. | Yes |
| Command mirror disabled reason/truncation/state badge/help parity. | No named follow-up found. | Yes |
| Drift review structured diff details. | No named follow-up found. | Yes |
| Full Keyboard Access, VoiceOver order, reduced motion, multi-window shared publisher, and component snapshot fixtures. | No named follow-up found. | Yes |

## Readiness Checklist

- [x] P058 focused gate passes at current HEAD.
- [x] Backend/control-plane readback and redaction regression tests pass in the focused gate.
- [x] R6 macOS attention request/cancel behavior is covered by focused tests.
- [x] R6 lineage retry collapse and shadow-row behavior is covered by focused tests.
- [ ] Production macOS read surface uses `EscalationReadAdapter` as the sole governed source.
- [ ] Dock/menu badge is derived from live aggregation across all runs and recomputed on adapter snapshots.
- [ ] Menu extra matches numeric badge, row limit, sorting, overflow, and compact state-pill/count behavior.
- [ ] Status capsule, pause card, command rows, and drift sheet satisfy the explicit component contracts.
- [ ] Required accessibility, multi-window, scene restoration, pasteboard, and presentation fixtures are complete.
- [ ] Residual P058 scope has either been implemented or assigned to a concrete follow-up proposal.

## Verification Log

Commands and checks performed:

```bash
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md"
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md"
git status --short
git branch --show-current
git rev-parse HEAD
git merge-base HEAD origin/main
git show --stat --oneline --decorate --no-renames 0559ff1afa298c4ce34512368c16b194c47ec8a5..HEAD
rg -n "EscalationReadAdapterRegistry|EscalationReadAdapter\\(|\\.applyChains\\(|EscalationSnapshot\\.build|applyP058EscalationSnapshots|MenuBarExtra|requestUserAttention|EscalationMenuBarList|StatusCapsule|EscalationPauseCard|EscalationCommandMirrorRow|DriftReviewSheet" "Chainworks Forge" "Chainworks ForgeTests"
./scripts/test-gate.sh proposal-058
```

Gate result:

- `./scripts/test-gate.sh proposal-058`: **passed**
- Notable non-blocking output: existing Swift and Rust compiler warnings, including unused/deprecated warnings and Rust dead-code/unused-variable warnings. No P058 gate failure was observed.

Worktree before report write:

- Clean. `git status --short` produced no entries before adding this audit report.

## Final Action

Do not retire P058 as fully implemented yet. Treat the backend/control-plane slice and the newly added macOS attention/lineage pieces as passing, but keep the proposal open until the governed adapter source, all-run aggregation, menu/dock fidelity, component contracts, and required macOS fixture evidence are completed or explicitly moved to a concrete follow-up proposal.
