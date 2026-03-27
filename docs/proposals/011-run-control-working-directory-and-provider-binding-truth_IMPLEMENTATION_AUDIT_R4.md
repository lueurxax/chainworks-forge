# Proposal 011: Run Control, Working Directory Ownership, and Provider Binding Truth Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md |
| Repository Root | . |
| Git SHA | 63f5270 |
| Working Tree | dirty |
| Audited At | 2026-03-27T06:55:20+0200 |
| Platform Scope | macOS |
| Proposal State | Active |
| Overall Conformance | Partial |
| Overall Readiness | Not Ready |
| Audit Confidence | High |

## Executive Verdict

Proposal 011 is materially closer to done than in R3. The working-directory contract, frozen binding truth, frozen provenance persistence, run-surface provenance display, and focused macOS proof are now all in good shape. The remaining blocker is concentrated but important: cancellation settlement is still not truthful to the proposal contract, because the runtime records `cancellationSettledAt` and terminal `cancelled` before Goose session closure is actually attempted and observed. That keeps Overall Conformance at `Partial` and keeps the slice `Not Ready` for sign-off.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | Partial | Cancellation settlement is marked complete before session-close outcome exists | High |
| Architecture | At Risk | `RunCancellationCoordinator` still splits settlement truth from transport cleanup | High |
| Product | At Risk | Cross-family binding anomalies are still warned mainly in preflight, not as explicit run-surface warning states | Medium |
| UI | Acceptable | No new UI-specific contract failures surfaced in this audit | Medium |
| UX | At Risk | Stop feedback can tell the operator work is fully cancelled before remote cleanup is confirmed | High |
| Readiness | Not Ready | Core stop/control contract remains only partially implemented despite green focused proof | High |

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

### Explicit Exclusions

- New provider families.
- New archive taxonomy.
- Workflow-map redesign.
- Multi-user repo assignment.

## Proposal Fidelity / Divergence

### Matches

- Stop and archive are separate operator actions in the idea owner path.
- `requires_project_access` is modeled in workflow YAML and propagated into `RunPlan`.
- Start Run, preflight, and resume all consume the same project-access selector.
- Live start freezes `providerBindingSnapshotJSON`, `bindingProvenanceJSON`, and `frozenWorkspaceRootPath`.
- Run report, stage detail, and run comparison now read frozen provider/model truth and provenance.
- Focused build, unit, and macOS UI proof all passed in this audit.

### Divergences

- Cancellation settlement is still recorded before Goose `closeSession` attempts complete, which violates the proposal's criterion 3 and criterion 4 linkage.
- Cross-family mismatch handling is still mostly a preflight warning path; run-centric surfaces do not elevate unusual bindings into an explicit warning state.

### Ambiguities / Evidence Gaps

- No additional major evidence gaps surfaced in the focused Proposal 011 slices; the current blocker is a direct code-path divergence, not a missing proof artifact.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Stop action is separate from archive and exposed from the idea owner path
- Proposal Source: `4.3 Operator surface rules`, `8. Acceptance criteria > Stop / run control`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `/tmp/p011-r4-ui.xcresult`
- Gap / Note: The idea owner path exposes a dedicated destructive stop action and focused UI proof passed.

### REQ-002 Cancellation is settled truthfully and shown as `cancelling…` until settlement is actually confirmed
- Proposal Source: `4.2 Stop semantics`, `4.2.1 Cancellation settlement criteria`, `7. Data and runtime model additions`, `8. Acceptance criteria > Stop / run control`
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:54`
  - `Chainworks Forge/Models/Run.swift:139`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:40`
  - `Chainworks Forge/Engine/ExecutionService.swift:271`
  - `/tmp/p011-r4-unit.xcresult`
- Gap / Note: The model fields, presentation status, and focused test slice exist, but `RunCancellationCoordinator.settle()` still writes `cancellationSettledAt` and terminal `.cancelled` before `closeGooseSessions()` is executed. The settlement log also records `sessionCloseSucceeded: nil` for open sessions rather than actual close outcomes.

### REQ-003 Cancelled runs remain visible as terminal history and archive remains separate
- Proposal Source: `4.3 Operator surface rules`, `8. Acceptance criteria > Stop / run control`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/RunsHomeView.swift`
  - `Chainworks Forge/Engine/IdeaArchiveService.swift`
- Gap / Note: Cancelled runs remain visible as terminal history and archive eligibility remains a separate lifecycle rule.

### REQ-004 Workflow YAML declares `requires_project_access` and compilation parses it into `RunPlan.requiresProjectAccess`
- Proposal Source: `5.2 Project access requirement selector`, `7. Data and runtime model additions`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/DSL/WorkflowDefinition.swift:114`
  - `Chainworks Forge/Engine/RunPlan.swift:27`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:53`
- Gap / Note: The YAML field is modeled in `ExecutionConfig`, propagated into `RunPlan`, and defaults to `false` when absent.

### REQ-005 Start Run, preflight, compiler, and resume all consume one authoritative project-access selector
- Proposal Source: `5.2 Project access requirement selector`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1251`
  - `Chainworks Forge/Engine/PreflightService.swift:89`
  - `Chainworks Forge/Engine/ResumeManager.swift:111`
  - `/tmp/p011-r4-unit.xcresult`
- Gap / Note: The same `requiresProjectAccess` flag now drives start gating, preflight, compiled plan shape, and resume validation.

### REQ-006 Each project-backed idea owns one explicit working-directory contract, and live start fails closed when it is missing or invalid
- Proposal Source: `5.1 Core rule`, `5.3 Rules`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Idea.swift`
  - `Chainworks Forge/Views/IdeaListView.swift:1251`
  - `Chainworks Forge/Engine/PreflightService.swift:307`
  - `/tmp/p011-r4-ui.xcresult`
- Gap / Note: The workspace root is persisted on the idea, validated before live start, and enforced as fail-closed when required.

### REQ-007 The run/workspace freezes the idea-owned directory and execution does not rely on ambient cwd for project selection
- Proposal Source: `5.3 Rules`, `5.4 Frozen run contract`, `8. Acceptance criteria > Working directory`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:50`
  - `Chainworks Forge/Views/IdeaListView.swift:1251`
  - `Chainworks Forge/Views/IdeaListView.swift:1286`
  - `Chainworks Forge/Views/IdeaListView.swift:1292`
  - `Chainworks Forge/Engine/ResumeManager.swift:111`
- Gap / Note: The execution path now freezes `frozenWorkspaceRootPath` and the delivery path no longer falls back to ambient cwd for repo selection.

### REQ-008 Run-centric surfaces show resolved provider/model truth from the frozen binding snapshot
- Proposal Source: `6.1 Problem statement`, `6.2 Four facts the UI must distinguish`, `7. Data and runtime model additions`, `8. Acceptance criteria > Provider/model truth`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:2041`
  - `Chainworks Forge/Engine/RunComparisonService.swift:114`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:145`
- Gap / Note: Stage-detail metadata, comparison, and reports now all prefer frozen binding truth before falling back to mutable agent fields.

### REQ-009 Frozen binding provenance is persisted per agent and historical surfaces explain model origin using frozen data only
- Proposal Source: `6.4 Provenance`, `6.5 Provenance must be reproducible from frozen data only`, `7. Data and runtime model additions`, `8. Acceptance criteria > Provider/model truth`
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:47`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:86`
  - `Chainworks Forge/Views/IdeaListView.swift:2042`
  - `Chainworks Forge/Views/RunComparisonView.swift:212`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:147`
- Gap / Note: Provenance is frozen at run start, persists `backend_profile` / `configured_provider` / `run_override` / `unverifiable`, and is now surfaced from frozen data in run detail, comparison, and report output.

### REQ-010 Cross-family mismatches are blocked or surfaced as explicit warnings, and ambiguous mixed labels are not shown as normal truth
- Proposal Source: `6.3 Coherence policy`, `8. Acceptance criteria > Provider/model truth`
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:281`
  - `Chainworks Forge/Views/IdeaListView.swift:2041`
  - `Chainworks Forge/Views/RunComparisonView.swift:206`
- Gap / Note: Preflight now emits an explicit coherence warning, and run-centric surfaces show provenance source, but unusual bindings are still not elevated into a dedicated warning state inside run-detail/comparison views. The proposal's warning-with-provenance contract is therefore only partially carried through the operator surfaces.

## Architecture Review

**Summary:** At Risk

### ARCH-001 Cancellation settlement is still recorded ahead of criterion-3 completion
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `4.2.1 Cancellation settlement criteria`, `REQ-002`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:40`
  - `Chainworks Forge/Engine/ExecutionService.swift:271`
- Why It Matters: The proposal explicitly makes settlement depend on recorded propagation, including `closeSession` attempts for sessions open at cancellation-request time. The current implementation still treats session cleanup as post-settlement best-effort work, which means persisted run truth can claim the stop is complete before the transport boundary has even been touched.
- Recommended Action: Move session-close attempt/result recording into the settlement boundary itself, or persist an intermediate not-yet-settled state until criterion 3 is actually confirmed and reflected in `cancellationSettlementLog`.

## Product Review

**Summary:** At Risk

### PROD-001 Binding-coherence warnings still stop at preflight more than they reach historical run surfaces
- Severity: Minor
- Confidence: Medium
- Related Proposal Items / Requirements: `6.3 Coherence policy`, `REQ-010`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:281`
  - `Chainworks Forge/Views/IdeaListView.swift:2041`
  - `Chainworks Forge/Views/RunComparisonView.swift:206`
- Why It Matters: Proposal 011 is not just about surfacing frozen truth; it is also about making unusual bindings legible as unusual. Current code warns before start, but once the run exists the operator mostly sees provenance text, not an explicit warning treatment.
- Recommended Action: Carry a dedicated coherence-warning state into run-detail and comparison surfaces so unusual bindings are visibly exceptional, not merely annotated.

## UI Review

**Summary:** Acceptable

No additional UI-specific proposal regressions surfaced in this audit. The focused macOS UI slice for provider settings, pilot readiness, approval gate, start-run sheet, and run-progress owner path all passed.

## UX Review

**Summary:** At Risk

### UX-001 Stop feedback can overstate finality for live work
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `4.2 Stop semantics`, `4.2.1 Cancellation settlement criteria`, `REQ-002`
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:139`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:93`
  - `Chainworks Forge/Engine/ExecutionService.swift:290`
- Why It Matters: The operator-facing contract is "cancelling…" until propagation is settled. Today the data model supports that distinction, but the coordinator collapses too quickly to terminal `cancelled`, which can give false reassurance in live cancellation paths.
- Recommended Action: Keep runs in `cancelling` until session-close attempts have been recorded into settlement evidence, then transition to terminal `cancelled`.

## Delivery / Readiness Review

**Summary:** Not Ready

### READY-001 Focused proof is green, but the last core contract blocker is still in the stop path
- Severity: Major
- Confidence: High
- Related Proposal Items / Requirements: `REQ-002`, `REQ-010`
- Evidence Type: tests-run, code
- Evidence:
  - `/tmp/p011-r4-build.xcresult`
  - `/tmp/p011-r4-unit.xcresult`
  - `/tmp/p011-r4-ui.xcresult`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:40`
- Why It Matters: This audit no longer suffers from stale or failing focused proof. Build, targeted unit tests, and targeted macOS UI tests all passed. The remaining reason the proposal is not ready is therefore a real implementation divergence in a core operator-trust path, not an evidence gap.
- Recommended Action: Fix the settlement boundary first, then rerun the same focused build/unit/UI slice. If that lands cleanly, Proposal 011 should be close to an `Implemented` audit.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | Pass | `/tmp/p011-r4-build.xcresult` |
| Core user flow runtime-validated | Partial | `/tmp/p011-r4-ui.xcresult` passed `5/5`, but the stop/cancellation contract still diverges in code |
| Empty/loading/error states covered | Partial | Start-run and run-progress owner path were exercised; full failure-state UX was not the focus of this audit |
| Accessibility risk acceptable | Not Checked | No dedicated accessibility audit in this pass |
| Localization risk acceptable | Not Checked | No localization proof in this pass |
| Critical tests executed | Pass | `/tmp/p011-r4-unit.xcresult` passed `33`; `/tmp/p011-r4-ui.xcresult` passed `5` |
| Privacy/permissions/entitlements reviewed | Not Checked | Not a primary contract of Proposal 011 |

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\\ Forge/docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `nl -ba docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md | sed -n '1,420p'`
- `rg -n "bindingProvenanceJSON|FrozenBindingProvenance|provenanceSource|requiresProjectAccess|currentDirectoryPath|cancellationSettledAt|cancellationSettlementLog|closeGooseSessions" 'Chainworks Forge' 'Chainworks ForgeTests' 'Chainworks ForgeUITests'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-r4-build-dd -resultBundlePath /tmp/p011-r4-build.xcresult build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-r4-unit-dd -resultBundlePath /tmp/p011-r4-unit.xcresult test -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testCancellationStopsTheRun'`
- `xcrun xcresulttool get test-results summary --path /tmp/p011-r4-unit.xcresult`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-r4-ui-dd -resultBundlePath /tmp/p011-r4-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface'`
- `xcrun xcresulttool get test-results summary --path /tmp/p011-r4-ui.xcresult`

## Recommended Next Actions

1. Make cancellation settlement wait for recorded Goose session-close attempts/results before writing `cancellationSettledAt` and terminal `.cancelled`.
2. Persist explicit session-close outcome into `cancellationSettlementLog` for every session that was open at cancellation-request time.
3. Promote cross-family binding anomalies from preflight-only warnings into explicit warning treatment on run-detail and comparison surfaces.
4. Rerun the same focused build/unit/UI slice after the settlement fix; if it stays green, Proposal 011 should be ready for a near-final audit.
