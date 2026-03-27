# Proposal 011: Run Control, Working Directory Ownership, and Provider Binding Truth Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md |
| Repository Root | . |
| Git SHA | 63f5270 |
| Working Tree | dirty |
| Audited At | 2026-03-27T07:41:19+0200 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Implemented |
| Overall Readiness | Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 011 is now implemented on the current tree. The two live R4 gaps are closed: cancellation settlement is now a truthful two-phase path that keeps the run in `cancelling` until real Goose close outcomes are recorded, and cross-family binding anomalies are now surfaced explicitly in run-centric operator surfaces. Fresh focused proof on the current `HEAD` is green: `build` passed, the focused unit slice passed `33/33`, and the focused macOS UI slice passed `3/3`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Implemented | No live proposal-contract divergence remains in the focused slice | High |
| Architecture | Acceptable | Residual risk is broad-tree drift outside Proposal 011 scope | High |
| Product | Acceptable | Proposal-specific operator truth is now consistent across start, resume, comparison, and reports | High |
| UI | Acceptable | Focused owner-path proof is green on macOS | High |
| UX | Acceptable | Stop, working-directory, and provenance behavior now match the operator contract | High |
| Readiness | Ready | Proposal-scoped build, unit, and UI evidence are green | High |

## Proposal Contract

### Scope

- Separate `Stop` from `Archive` as distinct lifecycle actions.
- Attach one explicit idea-owned working directory / project-root contract to project-backed workflows.
- Make provider/model truth in run-centric surfaces come from frozen resolved binding plus frozen provenance.

### Locked Decisions

- `Stop` and `Archive` are separate lifecycle actions.
- `requires_project_access` is the single authoritative selector shared by Start Run, preflight, compiler, and resume.
- Run surfaces must prefer frozen resolved binding truth over catalog shorthand.
- Provenance is frozen at run start and must never be reconstructed from mutable current provider settings.

### Primary User Flows

1. Stop an active idea from the idea owner path and trust the run enters truthful cancellation flow.
2. Configure a project-backed idea with one explicit working directory and fail closed if it is missing or invalid.
3. Start a live run and freeze the working directory plus provider/model binding into run state.
4. Resume an interrupted project-backed run using the frozen workspace contract rather than ambient cwd.
5. Inspect run-centric surfaces and understand provider/model truth plus provenance from frozen data only.

### UI Commitments

- Active ideas expose a dedicated stop action from the idea owner path.
- Stop is not hidden behind archive affordances.
- Cancelled runs remain visible in run-centric surfaces with truthful status.
- Run-centric metadata shows resolved provider/model truth plus provenance.

### UX Commitments

- Stop confirmation explains that history/artifacts remain intact.
- Project-backed live start fails closed without a valid idea-owned directory.
- Historical runs remain explainable after provider settings drift.
- Unusual provider/model bindings must be blocked or surfaced as explicit warnings.

### Acceptance Criteria

- `RunCancellationCoordinator` confirms all four settlement criteria before writing `cancellationSettledAt`.
- `cancellationSettlementLog` records per-agent terminal status and session-close outcome.
- `requires_project_access` is declared in workflow YAML and parsed into `RunPlan`.
- Start Run, preflight, compiler, and resume all read the same `requiresProjectAccess` selector.
- Frozen run/workspace state stores the idea-owned directory used for that run.
- Agent execution does not rely on ambient app cwd for project selection.
- Run surfaces show resolved binding truth from the frozen binding.
- `FrozenBindingProvenance` is persisted per agent binding at run start.
- Historical runs remain correctly explained after provider settings drift.

### Test / Evidence Requirements

- Focused Apple-platform proof for stop/run-control behavior.
- Focused proof for working-directory fail-closed behavior and frozen workspace state.
- Focused proof for frozen provider binding/provenance behavior and historical explanation after settings drift.

## Proposal Fidelity / Divergence

### Matches

- Stop and archive are separate operator actions in the idea owner path.
- `requires_project_access` is modeled in workflow YAML and propagated into `RunPlan`.
- Start Run, preflight, compiler, and resume all consume the same project-access selector.
- Live start freezes `providerBindingSnapshotJSON`, `bindingProvenanceJSON`, and `frozenWorkspaceRootPath`.
- `RunCancellationCoordinator` now separates preliminary cancellation from final settlement and writes terminal cancellation truth only after real session-close outcomes are observed.
- Run report, stage detail, and run comparison now read frozen provider/model truth and frozen provenance.
- Cross-family mismatches are surfaced explicitly in run detail and run comparison surfaces.
- Fresh focused build, unit, and macOS UI proof all passed on the current tree.

### Divergences

- None material to Proposal 011 surfaced in this audit.

### Ambiguities / Evidence Notes

- Early `/tmp/p011-r5-*.xcresult` compile failures were discarded because the dirty working tree changed mid-audit. The effective evidence for this report is the fresh rerun set on the final current `HEAD`: `/tmp/p011-r5b-build.xcresult`, `/tmp/p011-r5c-unit.xcresult`, and `/tmp/p011-r5b-ui.xcresult`.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Stop action is separate from archive and exposed from the idea owner path
- Proposal Source: `4.3 Operator surface rules`, `8. Acceptance criteria > Stop / run control`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `/tmp/p011-r5b-ui.xcresult`
- Gap / Note: The idea owner path exposes a dedicated stop action, distinct from archive behavior, and the focused macOS UI slice passed.

### REQ-002 Cancellation is settled truthfully and shown as `cancelling…` until settlement is actually confirmed
- Proposal Source: `4.2 Stop semantics`, `4.2.1 Cancellation settlement criteria`, `7. Data and runtime model additions`, `8. Acceptance criteria > Stop / run control`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:55`
  - `Chainworks Forge/Models/Run.swift:56`
  - `Chainworks Forge/Models/Run.swift:57`
  - `Chainworks Forge/Models/Run.swift:142`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:53`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:117`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:173`
  - `Chainworks Forge/Engine/ExecutionService.swift:276`
  - `/tmp/p011-r5c-unit.xcresult`
- Gap / Note: The run now remains in `cancelling` until `finalizeSettlement(sessionOutcomes:)` writes truthful session-close outcomes and only then stamps `cancellationSettledAt` and terminal `.cancelled`.

### REQ-003 Cancelled runs remain visible as terminal history and archive remains separate
- Proposal Source: `4.3 Operator surface rules`, `8. Acceptance criteria > Stop / run control`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Engine/IdeaArchiveService.swift`
- Gap / Note: Cancelled runs remain visible as terminal history and archive eligibility stays a separate lifecycle rule.

### REQ-004 Workflow YAML declares `requires_project_access` and compilation parses it into `RunPlan.requiresProjectAccess`
- Proposal Source: `5.2 Project access requirement selector`, `7. Data and runtime model additions`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/DSL/WorkflowDefinition.swift`
  - `Chainworks Forge/Engine/RunPlan.swift`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift`
- Gap / Note: The YAML field is modeled and propagated into compiled run plans.

### REQ-005 Start Run, preflight, compiler, and resume all consume one authoritative project-access selector
- Proposal Source: `5.2 Project access requirement selector`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1254`
  - `Chainworks Forge/Engine/PreflightService.swift:129`
  - `Chainworks Forge/Engine/ResumeManager.swift:112`
  - `/tmp/p011-r5c-unit.xcresult`
- Gap / Note: The same `requiresProjectAccess` selector now drives start gating, preflight, compilation, and resume validation.

### REQ-006 Each project-backed idea owns one explicit working-directory contract, and live start fails closed when it is missing or invalid
- Proposal Source: `5.1 Core rule`, `5.3 Rules`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Idea.swift`
  - `Chainworks Forge/Views/IdeaListView.swift:1254`
  - `Chainworks Forge/Engine/PreflightService.swift`
  - `/tmp/p011-r5b-ui.xcresult`
- Gap / Note: The workspace root is persisted on the idea, validated before live start, and enforced as fail-closed when required.

### REQ-007 The run/workspace freezes the idea-owned directory and execution does not rely on ambient cwd for project selection
- Proposal Source: `5.3 Rules`, `5.4 Frozen run contract`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:52`
  - `Chainworks Forge/Views/IdeaListView.swift:1289`
  - `Chainworks Forge/Views/IdeaListView.swift:1295`
  - `Chainworks Forge/Engine/ResumeManager.swift:112`
- Gap / Note: The execution path freezes `frozenWorkspaceRootPath`, resume revalidates that frozen path, and project-backed start no longer falls back to ambient cwd.

### REQ-008 Run-centric surfaces show resolved provider/model truth from the frozen binding snapshot
- Proposal Source: `6.1 Problem statement`, `6.2 Four facts the UI must distinguish`, `7. Data and runtime model additions`, `8. Acceptance criteria > Provider/model truth`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:2045`
  - `Chainworks Forge/Engine/RunComparisonService.swift:126`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:415`
- Gap / Note: Stage-detail metadata, comparison, and reports all prefer frozen binding truth before mutable fallback fields.

### REQ-009 Frozen binding provenance is persisted per agent and historical surfaces explain model origin using frozen data only
- Proposal Source: `6.4 Provenance`, `6.5 Provenance must be reproducible from frozen data only`, `7. Data and runtime model additions`, `8. Acceptance criteria > Provider/model truth`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:48`
  - `Chainworks Forge/Models/Run.swift:172`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:108`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:141`
  - `Chainworks Forge/Views/IdeaListView.swift:2054`
  - `Chainworks Forge/Views/RunComparisonView.swift:224`
  - `Chainworks Forge/Engine/RunComparisonService.swift:140`
- Gap / Note: Provenance is frozen at run start and now records `backendProfileDefault`, `configuredProviderDefault`, `runOverride`, or `unverifiable` without false reconstruction from mutable settings.

### REQ-010 Cross-family mismatches are blocked or surfaced as explicit warnings, and ambiguous mixed labels are not shown as normal truth
- Proposal Source: `6.3 Coherence policy`, `8. Acceptance criteria > Provider/model truth`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:129`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:19`
  - `Chainworks Forge/Views/IdeaListView.swift:2061`
  - `Chainworks Forge/Views/RunComparisonView.swift:190`
- Gap / Note: Preflight warns early, and run-detail/comparison surfaces now render an explicit warning icon and help text for cross-family mismatches instead of presenting them as ordinary truth.

## Architecture Review

**Summary:** Acceptable

No live architecture-level divergence remained after the R5 rerun. The central change is that cancellation is now a real two-phase contract, not a local flag flip dressed up as final settlement.

## Product Review

**Summary:** Acceptable

The product truth that matters for Proposal 011 is now consistent end to end: start fails closed when project access is required but missing, resume trusts frozen workspace state, and run-centric surfaces explain frozen binding plus provenance truthfully.

## UI Review

**Summary:** Acceptable

The focused macOS UI slice for approval gate, start-run sheet, and run-progress owner path passed on the current tree.

## UX Review

**Summary:** Acceptable

Operator-facing stop semantics now align with persisted truth. The model exposes `cancelling` until settlement is actually recorded, and historical provider/model explanations no longer depend on mutable settings drift.

## Delivery / Readiness Review

**Summary:** Ready

Proposal-scoped proof is green and the last R4 contract blocker is closed. No proposal-specific readiness blocker remains in the audited slice.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `/tmp/p011-r5b-build.xcresult` |
| Core user flow runtime-validated | Pass | `/tmp/p011-r5b-ui.xcresult` passed `3/3` |
| Empty/loading/error states covered | Partial | This audit stayed focused on Proposal 011 owner paths rather than broad shell-state enumeration |
| Accessibility risk acceptable | Not Checked | No dedicated accessibility audit in this pass |
| Localization risk acceptable | Not Checked | No localization proof in this pass |
| Critical tests executed | Pass | `/tmp/p011-r5c-unit.xcresult` passed `33/33`; `/tmp/p011-r5b-ui.xcresult` passed `3/3` |
| Privacy/permissions/entitlements reviewed | Not Checked | Not a primary contract of Proposal 011 |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `stat -f 'mtime: %Sm' -t '%Y-%m-%d %H:%M:%S %z' docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md`
- `md5 -q docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md`
- `rg -n "cancellationRequestedAt|cancellationSettledAt|cancellationSettlementLog|presentationStatus|enum BindingProvenanceSource|bindingProvenanceJSON|frozenWorkspaceRootPath" 'Chainworks Forge/Models/Run.swift'`
- `rg -n "beginSettlement|finalizeSettlement|closeGooseSessionsWithOutcomes|sessionCloseTimedOutOutcome|signalCancellation" 'Chainworks Forge/Engine/RunCancellationCoordinator.swift'`
- `rg -n "cancelRun\\(|closeGooseSessionsWithOutcomes|finalizeSettlement|removeActiveRun|cancelApproval" 'Chainworks Forge/Engine/ExecutionService.swift'`
- `rg -n "requiresProjectAccess|workspaceRootPath|providerBindingSnapshotJSON|bindingProvenanceJSON|frozenWorkspaceRootPath|hasCrossFamilyMismatch|warning.triangle" 'Chainworks Forge/Views/IdeaListView.swift'`
- `rg -n "provenanceSource|hasCrossFamilyMismatch|warning.triangle|bindingSummaryRow" 'Chainworks Forge/Views/RunComparisonView.swift'`
- `rg -n "resolveProvenances|unverifiable|hasCrossFamilyMismatch|ResolvedBindingProvenance|configuredProviderDefault|backendProfileDefault|runOverride" 'Chainworks Forge/Providers/BackendProfileResolverV2.swift'`
- `rg -n "requiresProjectAccess|frozenWorkspaceRootPath|workspaceRoot" 'Chainworks Forge/Engine/ResumeManager.swift'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /tmp/p011-r5b-build.xcresult build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /tmp/p011-r5c-unit.xcresult test -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/OrchestratorTests/cancellation'`
- `xcrun xcresulttool get test-results summary --path /tmp/p011-r5c-unit.xcresult`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /tmp/p011-r5b-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface'`
- `xcrun xcresulttool get test-results summary --path /tmp/p011-r5b-ui.xcresult`

## Recommended Next Actions

1. Keep the focused Proposal 011 build/unit/UI slice in the regular regression pack so cancellation-settlement truth and frozen binding provenance do not silently drift.
2. No additional proposal-specific corrective action is required before treating Proposal 011 as implemented.
