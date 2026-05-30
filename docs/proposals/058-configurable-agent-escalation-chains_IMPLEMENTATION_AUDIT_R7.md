# Proposal 058 Implementation Audit R7

| Field | Value |
| --- | --- |
| Proposal | `docs/proposals/058-configurable-agent-escalation-chains.md` |
| Report | `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R7.md` |
| Audit timestamp | 2026-05-28T18:05:13Z |
| Worktree | `/Users/user/Documents/Chainworks Forge` |
| Target branch | `main` |
| Target HEAD | `64bd78d57365d205010bac3d72833ea461fe737c` (`Route P058 UI through escalation adapter`) |
| Merge base with `origin/main` | `64bd78d57365d205010bac3d72833ea461fe737c` |
| Compare basis | Implicit current worktree; R7 inspected delta from R6 baseline `17efd85a10e29df13ac96c2f50f889034f2491e1..HEAD` |
| Worktree before report write | Clean |
| Proposal state | Active (`Status: refined_after_write_boundary_blocker_resolved`) |

## Verdict

Overall conformance: **Partially Implemented**

Overall implementation readiness: **Not Ready**

Reviewer-selection reuse: **Not reused**. No prior proposal-review artifacts were found; prior implementation audits were used only as historical context.

Audit confidence: **High** for code-level conformance gaps and gate status; **Medium** for visual/runtime UI fidelity because no screenshot, remote UI, or accessibility runtime evidence was produced in this audit.

R7 improves the most important R6 architecture gap: run detail now pushes `runDetail.escalationChains` into `EscalationReadAdapterRegistry`, the inspector wraps a registry-backed adapter, and notifications consume `EscalationReadAdapterRegistry.shared.attentionSnapshots`. The P058 canonical gate passes on the audited HEAD.

The implementation is still not ready for P058 closeout. The macOS path is now adapter-routed for selected run detail and inspector rendering, but the proposal's all-run live aggregation, menu-bar contract, component layout/accessibility rules, and fixture evidence remain partial. A direct `escalationSnapshot` build also remains exposed in the P031 presentation model, so the sole-source adapter contract is not fully enforced.

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

- `64bd78d5 Route P058 UI through escalation adapter`

Changed files in `17efd85a..HEAD`:

- `Chainworks Forge/ContentView.swift`
- `Chainworks Forge/Engine/EscalationReadAdapter.swift`
- `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift`
- `Chainworks Forge/Views/EscalationReadSurfaceViews.swift`
- `Chainworks Forge/Views/RunsHomeView.swift`
- `Chainworks ForgeTests/Proposal058Tests.swift`
- `docs/proposals/058-configurable-agent-escalation-chains_IMPLEMENTATION_AUDIT_R6.md`

Material improvements since R6:

- `ContentView.swift:238-250` routes run-detail escalation chains through `EscalationReadAdapterRegistry.shared.applyChains` and feeds notifications from registry attention snapshots.
- `EscalationReadAdapter.swift:120-160` now exposes shared registry snapshot and attention snapshot aggregation helpers.
- `RunsHomeView.swift:376-381` renders `EscalationInspectorAdapterView` from run-detail chains.
- `EscalationReadSurfaceViews.swift:757-779` creates inspector views from the shared registry adapter.
- `Proposal058Tests.swift:391-417` proves registry attention snapshot aggregation across two run IDs.
- `./scripts/test-gate.sh proposal-058` passes on current HEAD.

## Proposal Contract Summary

P058 commits to a cross-stack escalation system:

- Rust control plane owns policy resolution, trigger classification, tier advancement, persistence, recovery, capacity checks, kill switch behavior, and pause/resume legality.
- GraphQL/MCP/report readback exposes forward-compatible raw strings and redacted, caller-appropriate escalation state.
- Governed macOS is read/subscription presentation only. It must not become an escalation lifecycle authority.
- `EscalationReadAdapter` is the sole governed UI source for run detail, inspectors, notifications, shortcuts, command enablement, trace copy, banner state, pause cards, and lineage views.
- Dock badge and human-tier attention derive from live aggregation across runs and are recomputed on every adapter snapshot.
- macOS components have explicit layout, accessibility, keyboard, menu, trace, drift, and density contracts.
- Fixtures must prove presentation states, symbol resolution, strict concurrency, scene restoration, multi-window shared publisher, dock aggregation, user attention cancellation, pasteboard atomicity, Full Keyboard Access order, and read-only drift handoff.

## Platform And Product Scope

Apple scope: **macOS**

Backend/service scope: **cross-stack Rust control-plane, GraphQL/MCP readback, persistence, metrics, rollout, and macOS read surface**

Primary product flow: operators can see why a run escalated, what tier/trigger/pause state applies, what attention is required, and what read-only diagnostic/handoff action is available without the SwiftUI app mutating escalation state.

## Primary Flows Audited

1. Escalation policy execution and durable readback in the Rust control plane.
2. GraphQL/MCP/report boundary readback with redaction and caller-appropriate fields.
3. macOS run-detail and inspector rendering from the governed adapter.
4. dock badge, menu-bar extra, and background user attention aggregation.
5. read-only drift/trace/command/pause operator workflow and accessibility fixture readiness.

## Proposal Fidelity Inventory

### Matches

- Backend/control-plane P058 gate passes, including schema, runtime fact, readback, redaction, claim/start, recovery, and MCP/report readback tests.
- Governed drift sheet remains read-only; no macOS mutation call was found in the sheet.
- Trace pasteboard copy is tested for `.string` and `public.json` atomicity in the P058 suite.
- Registry-backed adapter routing now exists for selected run-detail sync, inspector rendering, and notification snapshot source.
- Informational user attention request/cancel behavior is implemented and tested.
- Lineage retry collapse and shadow-row display logic exist and are tested.

### Divergences

- P031 still constructs and exposes `P031RunDetailPresentation.escalationSnapshot` directly via `EscalationSnapshot.build` at `P031ThinGraphQLReadBoundary.swift:6653-6656`; this is outside the adapter and weakens the "sole governed UI source" guarantee.
- Dock/menu attention is recomputed when selected run detail syncs; it is not proven as live aggregation across all paused runs in the app.
- `NotificationService.applyP058EscalationSnapshots` still counts paused chains plus drift plus kill switch flags, which can overcount a single run relative to the proposal's aggregate paused-run count.
- Menu bar list lacks numeric badge semantics, row cap, transition-recency sort, overflow item, and exact empty-state string.
- Status capsule, pause card, command row, and drift sheet do not implement all explicit component slots/layout rules.

### Ambiguities / Evidence Gaps

- No runtime screenshot, remote UI run, or snapshot fixture evidence was produced for P058 macOS visual fidelity.
- No P058-specific Full Keyboard Access tab-order fixture was found.
- No P058-specific scene restoration fixture proving restored windows wait for shared adapter publisher was found.
- No P058-specific multi-window fixture proving all inspectors receive the same adapter update was found.
- No contrast or reduced-motion fixture evidence was found for the P058 escalation components.

## Requirement Summary

| Requirement | Status |
| --- | --- |
| REQ-001 Policy/tier schema and compile validation | Implemented |
| REQ-002 Durable ledger/runtime facts/event journal/readback | Implemented |
| REQ-003 Caller-appropriate GraphQL/MCP/report readback | Implemented |
| REQ-004 Redaction and sensitive-field exclusion | Implemented |
| REQ-005 Metrics/observability declarations | Implemented |
| REQ-006 Governed macOS drift write boundary is read-only | Implemented |
| REQ-007 `EscalationReadAdapter` is the sole governed UI source | Partially Implemented |
| REQ-008 Shared adapter keyed by `run_id` for all windows/inspectors | Partially Implemented |
| REQ-009 Dock/menu attention from live all-run adapter aggregation | Partially Implemented |
| REQ-010 Informational user attention request/cancel | Implemented for current service path |
| REQ-011 MenuBarExtra badge/list/overflow/compact contract | Partially Implemented |
| REQ-012 Lineage retry collapse, disclosure, shadow rows, layout | Partially Implemented |
| REQ-013 Status capsule field order/color/suppression/truncation | Partially Implemented |
| REQ-014 Pause card countdown and responsive layout | Partially Implemented |
| REQ-015 Command mirror disabled reason/truncation/state badge | Partially Implemented |
| REQ-016 Drift review structured diff and handoff details | Partially Implemented |
| REQ-017 Required macOS fixtures and release evidence | Partially Implemented |

## Detailed Requirement Audit

### REQ-001 Policy/tier schema and compile validation

Source: Goals, Architecture, policy schema sections.  
Status: **Implemented**  
Evidence: `tests-run`, `code`  
Evidence references: `./scripts/test-gate.sh proposal-058`; Rust schema tests passed, including 60 escalation schema tests and 30 escalation policy schema tests.  
Note: No R7 regression observed.

### REQ-002 Durable ledger/runtime facts/event journal/readback

Source: Goals and Architecture persistence/readback commitments.  
Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: proposal gate passed; runtime facts, claim/start, recovery, and readback tests passed.  
Note: No R7 regression observed.

### REQ-003 Caller-appropriate GraphQL/MCP/report readback

Source: Goals and boundary readback commitments.  
Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: proposal gate passed; MCP/report tests for escalation readback and caller visibility passed.  
Note: No R7 regression observed.

### REQ-004 Redaction and sensitive-field exclusion

Source: Notifications forbidden fields and readback redaction commitments.  
Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: proposal gate passed; payload-shape, credential/path rejection, and security readback tests passed.  
Note: No raw sensitive-field rendering was found in the inspected macOS P058 components.

### REQ-005 Metrics/observability declarations

Source: rollout/metrics commitments.  
Status: **Implemented**  
Evidence: `tests-run`  
Evidence references: `metrics::tests::proposal_058_required_metric_names_are_declared` passed during gate.  
Note: Long-run metric-threshold trending remains release evidence, not proof of missing metric declarations.

### REQ-006 Governed macOS drift write boundary is read-only

Source: macOS Authority Boundary and DriftReviewSheet Write Boundary.  
Status: **Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `EscalationReadSurfaceViews.swift:687-732`; `Proposal058Tests.swift:568-575` continues trace copy testing; no mutation call found in `DriftReviewSheet`.  
Note: The sheet is read-only, but its structured diff content is incomplete under REQ-016.

### REQ-007 `EscalationReadAdapter` is the sole governed UI source

Source: macOS Authority Boundary lines requiring `EscalationReadAdapter` as sole governed UI source.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `ContentView.swift:238-250`, `EscalationReadAdapter.swift:120-160`, `EscalationReadSurfaceViews.swift:757-779`, `P031ThinGraphQLReadBoundary.swift:5680-5682`, `P031ThinGraphQLReadBoundary.swift:6653-6656`, `Proposal058Tests.swift:391-417`, `Proposal058Tests.swift:520-565`.  
Implementation mapping: selected run detail and inspector now route through registry-backed adapters; notifications now consume registry attention snapshots.  
Gap: P031 still constructs and exposes `escalationSnapshot` directly from GraphQL readback, and the test suite still asserts that direct presentation snapshot. The code does not enforce that all UI-facing snapshot construction lives behind the adapter.

### REQ-008 Shared adapter keyed by `run_id` for all windows/inspectors

Source: macOS Authority Boundary shared-adapter and restored-scene requirements.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `EscalationReadAdapter.swift:120-167`; `EscalationReadSurfaceViews.swift:757-779`; `Proposal058Tests.swift:225-231`.  
Implementation mapping: registry returns the same adapter for the same run ID; inspector wrapper uses registry adapter.  
Gap: no scene restoration fixture and no multi-window fixture proves restored windows wait for the shared publisher or that all inspectors receive the same update in production.

### REQ-009 Dock/menu attention from live all-run adapter aggregation

Source: Notifications Dock Badge and Human Tier Attention; MenuBarExtra layout.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`, `tests-run`  
Evidence references: `ContentView.swift:238-250`, `EscalationReadAdapter.swift:147-160`, `NotificationService.swift:159-170`, `Proposal058Tests.swift:349-417`.  
Implementation mapping: notification service consumes registry attention snapshots; registry can aggregate snapshots for multiple run IDs once adapters exist.  
Gap: app-level aggregation is still driven from run-detail sync and registry entries already touched by the UI. There is no live all-run source proving paused runs that were never selected are included. `NotificationService` also sums paused chains plus drift/kill-switch flags instead of counting attention runs once.

### REQ-010 Informational user attention request/cancel

Source: Fixtures and Notifications Human Tier Attention.  
Status: **Implemented for current service path**  
Evidence: `code`, `tests-run`  
Evidence references: `NotificationService.swift:265-279`; `Proposal058Tests.swift:420-446`; proposal gate passed.  
Note: This path is bounded by REQ-009's aggregation limitation.

### REQ-011 MenuBarExtra badge/list/overflow/compact contract

Source: MenuBarExtra component and Compact density rules.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-found`  
Evidence references: `Chainworks_ForgeApp.swift:85-101`; `EscalationReadSurfaceViews.swift:604-644`.  
Implementation mapping: menu extra exists and renders attention rows.  
Gap: label does not render the proposal's numeric badge/count/state-pill compact contract; list does not cap at five rows, sort by most-recent escalation transition, or show `Show all paused runs...`; empty text says `No paused escalation chains` instead of the promised `No paused escalation runs`.

### REQ-012 Lineage retry collapse, disclosure, shadow rows, layout

Source: Escalationlineageview component rules.  
Status: **Partially Implemented**  
Evidence: `code`, `tests-run`  
Evidence references: `EscalationReadSurfaceViews.swift:203-430`; `Proposal058Tests.swift:448-505`; proposal gate passed.  
Implementation mapping: retry collapse and shadow-row flags/styles exist and are tested.  
Gap: detailed fixed-column layout, duration/right-aligned values, and full disclosure content for digest inputs, redacted evidence refs, redaction version, and runtime fact refs remain incomplete or unproven.

### REQ-013 Status capsule field order/color/suppression/truncation

Source: Escalationstatuscapsule component rules.  
Status: **Partially Implemented**  
Evidence: `code`  
Evidence references: `EscalationReadSurfaceViews.swift:86-112`.  
Implementation mapping: capsule renders state label, symbol, help, and accessibility summary.  
Gap: visible field order remains a single `Label`; tier and trigger are not rendered as state pill + separator + tier + separator + trigger in standard/detailed densities. Proposal-specific color, suppression, symbol fixture, and 24-character middle-truncation coverage is incomplete.

### REQ-014 Pause card countdown and responsive layout

Source: Escalationpausecard component rules.  
Status: **Partially Implemented**  
Evidence: `code`  
Evidence references: `EscalationReadSurfaceViews.swift:431-468`.  
Implementation mapping: pause card exists with title/body/actions/metadata.  
Gap: no countdown formatting, no ideal/minimum width behavior, no below-360 stacking rule, and no below-280 summary/open-inspector fallback.

### REQ-015 Command mirror disabled reason/truncation/state badge

Source: Escalationcommandpresentation component rules.  
Status: **Partially Implemented**  
Evidence: `code`  
Evidence references: `EscalationReadSurfaceViews.swift:484-514`.  
Implementation mapping: command rows exist and copy commands.  
Gap: no stable disabled-reason rule across subtitle/help/accessibilityHint/tooltip, no 48-character middle truncation, and no optional state badge.

### REQ-016 Drift review structured diff and handoff details

Source: Driftreviewsheet component rules.  
Status: **Partially Implemented**  
Evidence: `code`  
Evidence references: `EscalationReadSurfaceViews.swift:687-732`.  
Implementation mapping: read-only sheet shows frozen/current hashes and copy/open actions.  
Gap: no structured tier added/removed/changed badges, no `max_chain_attempts` delta, no trigger-list delta, and no `run_id` display.

### REQ-017 Required macOS fixtures and release evidence

Source: Fixtures section and Implementation Sync closeout notes.  
Status: **Partially Implemented**  
Evidence: `tests-found`, `tests-run`, `inference`  
Evidence references: proposal gate passed; P058 Swift tests include 26 focused tests.  
Gap: no P058-specific screenshot/runtime visual evidence, Full Keyboard Access fixture, contrast fixture, reduced-motion fixture, scene restoration fixture, or production multi-window shared-publisher fixture was found.

## Reviewer Routing

Selected reviewers:

- `apple_arch_reviewer`: adapter ownership, SwiftUI state flow, shared registry, and production source-of-truth enforcement.
- `macos_ui_reviewer`: explicit menu/status/pause/command/drift/lineage/accessibility component contracts.
- `api_contract_reviewer`: GraphQL/MCP/report readback and caller boundary conformance.
- `rust_reliability_reviewer`: control-plane retry, claim/start, recovery, and idempotency behavior.
- `observability_rollout_reviewer`: gate evidence, metrics, release evidence, and closeout readiness.

Rejected close alternatives:

- `security_reviewer`: redaction/security behavior is covered by P058 gate; no new auth or secret-handling surface was introduced in R7.
- `performance_reviewer`: no new P058 hot-path performance claim or benchmark target changed in R7.
- `product_reviewer`: remaining product/user-value risk is directly tied to explicit proposal readiness and macOS UI contract gaps.

## Reviewer Scorecard

| Lens | Conformance | Readiness | Top risk | Confidence |
| --- | --- | --- | --- | --- |
| Apple architecture | Partial | Not Ready | direct P031 snapshot remains outside adapter; no restored/multi-window proof | High |
| macOS UI | Partial | Not Ready | menu/status/pause/command/drift component contracts incomplete | High |
| API contract | Implemented | Ready with risks | client consumption still has source-boundary residue | High |
| Rust reliability | Implemented | Ready with risks | warnings remain, but P058 reliability tests pass | High |
| Observability/rollout | Partial | Not Ready | release-closeout evidence and UI fixtures incomplete | Medium |

## Routed Findings

### ARCH-001 [Major] Adapter routing is improved, but the sole-source UI boundary is not enforced

Reviewer: `apple_arch_reviewer`  
Confidence: High  
Related requirements: REQ-007, REQ-008  
Evidence types: `code`, `tests-found`, `tests-run`

R7 correctly routes selected run detail through `EscalationReadAdapterRegistry.shared.applyChains` and renders the inspector through `EscalationInspectorAdapterView`. However, the P031 presentation still exposes `escalationSnapshot` and builds it directly:

- `P031ThinGraphQLReadBoundary.swift:5680-5682`
- `P031ThinGraphQLReadBoundary.swift:6653-6656`
- `P031ThinGraphQLReadBoundary.swift:6689-6691`

The P058 test still asserts this direct presentation snapshot at `Proposal058Tests.swift:562-563`. That leaves a second UI-facing snapshot path outside the adapter, contrary to the proposal's "sole governed UI source" rule.

Why it matters: even if current inspector rendering uses the adapter, the presentation model still exports a parallel snapshot that future run detail, shortcuts, command enablement, or banner code can consume without the shared adapter semantics.

Recommended action: remove `P031RunDetailPresentation.escalationSnapshot` or make it explicitly adapter-produced and non-authoritative. Add a regression test that fails if production code calls `EscalationSnapshot.build` outside `EscalationReadAdapter` or intentionally scoped test fixtures.

Acceptance criteria:

- production UI-facing snapshot construction is owned by `EscalationReadAdapter`;
- run detail, inspector, notification, command, trace, banner, pause, and lineage paths consume adapter snapshots or raw DTOs only to feed the adapter;
- multi-window and restored-scene tests prove shared adapter publication.

### UI-001 [Major] Dock/menu aggregation still does not prove live all-run attention

Reviewer: `macos_ui_reviewer`  
Confidence: High  
Related requirements: REQ-009, REQ-011  
Evidence types: `code`, `tests-found`, `tests-run`

`ContentView.swift:238-250` updates the registry from the selected run detail and applies `EscalationReadAdapterRegistry.shared.attentionSnapshots` to `NotificationService`. The registry can aggregate multiple adapters, but it only knows adapters that have been created or updated through the UI path. There is no evidence of a live all-run subscription/readback source that covers paused runs the operator has not selected.

`NotificationService.swift:159-170` also computes attention as paused chain count plus drift plus kill-switch flags, which can overcount a single run. `EscalationMenuBarList` at `EscalationReadSurfaceViews.swift:604-644` still lacks the promised row cap, transition-recency sort, overflow item, and exact compact menu item semantics.

Why it matters: P058's operator attention promise is global. A paused escalation run should not be invisible because another run is selected, and badge counts should not inflate because one run has multiple active flags.

Recommended action: introduce an all-run escalation attention projection or subscription feeding the registry, count attention runs once for dock/menu purposes, and implement menu row cap/sort/overflow and compact numeric badge/state-pill rendering.

Acceptance criteria:

- a paused run not selected in run detail appears in dock/menu attention;
- one run with multiple paused chains/drift/kill-switch contributes the proposal-defined count;
- menu list shows at most five most-recent rows plus `Show all paused runs...`;
- empty state matches `No paused escalation runs`.

### UI-002 [Major] Named macOS component contracts remain partial

Reviewer: `macos_ui_reviewer`  
Confidence: High  
Related requirements: REQ-012 through REQ-016  
Evidence types: `code`

Several component implementations still do not match explicit proposal rules:

- `EscalationStatusCapsule` at `EscalationReadSurfaceViews.swift:86-112` renders one label rather than state/tier/trigger fields with collapse order.
- `EscalationPauseCard` at `EscalationReadSurfaceViews.swift:431-468` lacks countdown formatting and responsive width behavior.
- `EscalationCommandMirrorRow` at `EscalationReadSurfaceViews.swift:484-514` lacks disabled reason parity, middle truncation, and state badge.
- `DriftReviewSheet` at `EscalationReadSurfaceViews.swift:687-732` lacks the structured policy diff fields promised by the proposal.
- `EscalationLineageView` now has retry collapse and shadow rows, but full fixed-column/disclosure content remains incomplete or unproven.

Why it matters: these are not subjective UI preferences; they are explicit proposal acceptance contracts for operator clarity and accessibility.

Recommended action: complete the named slots/rules for each component and add focused component fixtures or snapshot tests covering density, collapse behavior, keyboard order, accessibility labels, and narrow widths.

Acceptance criteria:

- each component rule in proposal lines 95-160 has code and test/snapshot evidence;
- accessibility ordering and keyboard operation match the proposal;
- reduced motion and contrast commitments are covered by fixtures or runtime evidence.

### READY-001 [Major] P058 still has unowned residual release scope

Reviewer: `observability_rollout_reviewer`  
Confidence: High  
Related requirements: REQ-017  
Evidence types: `tests-run`, `code`, `inference`

The canonical P058 gate passes, but the proposal still promises behavior and evidence that is only partially implemented. No concrete follow-up proposal was found assigning the remaining macOS adapter-enforcement, all-run aggregation, menu/component, and fixture scope elsewhere.

Why it matters: the implementation-audit tail gate does not allow unfinished proposal scope to be treated as "future work" unless an explicit proposal owns it.

Recommended action: either complete the residual scope under P058 or create a named follow-up proposal that explicitly owns the remaining macOS read-surface and release evidence items.

Acceptance criteria:

- all P058 requirements are implemented and verified, or a concrete follow-up proposal owns each deferred requirement;
- closeout docs do not retire P058 as fully implemented while REQ-007 through REQ-017 remain partial.

## Residual Scope / Follow-up Ownership

| Residual item | Owner found? | Blocks conformance/readiness? |
| --- | --- | --- |
| Enforce adapter as sole production UI snapshot source and remove direct P031 snapshot exposure. | No | Yes |
| Prove shared adapter behavior for restored scenes and multiple windows/inspectors. | No | Yes |
| Feed dock/menu from live all-run escalation aggregation, not selected-run sync only. | No | Yes |
| Correct dock/menu count semantics for runs vs chains/drift/kill-switch flags. | No | Yes |
| Implement MenuBarExtra numeric badge, row cap, recency sort, overflow, compact state-pill/count, exact empty state. | No | Yes |
| Complete status capsule visible fields, color rules, suppression, symbol fixture, and truncation behavior. | No | Yes |
| Complete pause card countdown and responsive layout breakpoints. | No | Yes |
| Complete command row disabled reason/truncation/state badge/help parity. | No | Yes |
| Complete drift review structured diff fields. | No | Yes |
| Add P058-specific FKA, VoiceOver/order, contrast, reduced-motion, screenshot/snapshot, and visual runtime evidence. | No | Yes |

## Readiness Checklist

- [x] Current HEAD recorded.
- [x] Worktree was clean before writing this report.
- [x] Prior proposal-review discovery completed.
- [x] Canonical P058 proposal gate passed on audited HEAD.
- [x] Swift P058 suite passed: 26 tests in the focused suite.
- [x] Rust/control-plane P058 schema, readback, runtime facts, claim/start, recovery, MCP/report, metrics, and redaction tests passed in the gate.
- [x] Selected run-detail and inspector paths now use registry-backed adapter routing.
- [ ] P031 presentation no longer exposes a direct non-adapter `escalationSnapshot`.
- [ ] Dock/menu aggregation covers all runs live, including runs not selected in detail.
- [ ] MenuBarExtra contract is fully implemented and proven.
- [ ] Status capsule, pause card, command row, lineage disclosure content, and drift sheet contracts are fully implemented and proven.
- [ ] P058-specific accessibility, contrast, reduced-motion, restored-scene, multi-window, and visual evidence exists.
- [ ] Residual scope is implemented or assigned to a concrete follow-up proposal.

## Verification Log

Commands run:

```bash
git status --short
git branch --show-current
git rev-parse HEAD
git merge-base HEAD origin/main
date -u +%Y-%m-%dT%H:%M:%SZ
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md"
python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/discover_prior_review.py "/Users/user/Documents/Chainworks Forge/docs/proposals/058-configurable-agent-escalation-chains.md"
git log --oneline --decorate --no-merges 17efd85a10e29df13ac96c2f50f889034f2491e1..HEAD
git show --stat --oneline --decorate --no-renames 17efd85a10e29df13ac96c2f50f889034f2491e1..HEAD
git diff --name-status 17efd85a10e29df13ac96c2f50f889034f2491e1..HEAD
rg -n "EscalationReadAdapterRegistry|EscalationReadAdapter\\(|\\.applyChains\\(|EscalationSnapshot\\.build|applyP058EscalationSnapshots|MenuBarExtra|requestUserAttention|EscalationMenuBarList|EscalationStatusCapsule|EscalationPauseCard|EscalationCommandMirrorRow|DriftReviewSheet|Full Keyboard|Keyboard|reducedMotion|accessibility" "Chainworks Forge" "Chainworks ForgeTests"
rg -n "escalationSnapshot|escalationChains|attentionSnapshots|dockBadgeEscalationCount|MenuBarExtra|pendingAttentionCount|No paused escalation|Show all paused|middle trunc|truncat|Countdown|waitingRetryAfter|interactiveDismissDisabled|EscalationCommandMirrorRow|disabled reason|state badge" "Chainworks Forge" "Chainworks ForgeTests"
./scripts/test-gate.sh proposal-058
```

Gate result:

- `./scripts/test-gate.sh proposal-058`: **passed**
- Swift focused P058 suite: **26 tests passed**
- Rust/control-plane gate: **passed**
- Notable non-blocking output: existing Swift actor-isolation/deprecation warnings and Rust dead-code/unused-variable warnings.

## Final Recommended Actions

1. Keep P058 open as **Partial / Not Ready**.
2. Close the remaining adapter-source gap by removing or de-authoritizing direct `P031RunDetailPresentation.escalationSnapshot` construction.
3. Replace selected-run attention sync with live all-run escalation aggregation and correct dock/menu count semantics.
4. Finish the explicit macOS component contracts and add the missing P058-specific fixture/runtime evidence.
5. If the remaining UI/evidence work is intentionally deferred, create a concrete follow-up proposal that owns each residual item before retiring P058.
