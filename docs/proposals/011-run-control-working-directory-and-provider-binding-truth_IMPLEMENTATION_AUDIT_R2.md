# Proposal 011: Run Control, Working Directory Ownership, and Provider Binding Truth Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md |
| Repository Root | . |
| Git SHA | 63f5270 |
| Working Tree | clean |
| Audited At | 2026-03-26T22:47:08+0200 |
| Proposal State | Active (Draft) |
| Overall Status | Not Implemented |

## Verdict

Proposal 011 is much closer to implementation than in R1, but it is still not fully implemented on current `HEAD`. The big structural gaps from the first audit are closed: stop/archive are separated in the idea owner path, cancellation settlement fields and coordinator exist, `requires_project_access` is parsed into `RunPlan`, ideas now own `workspaceRootPath`, and the start/preflight path fails closed for project-backed flows. The remaining blocker is still core Proposal 011 scope rather than polish: frozen binding provenance is not persisted at all, and the fresh targeted unit audit also surfaced a real runtime crash in the new cancellation/live-executor path.

## Proposal Contract

### Scope
- Separate `Stop` from `Archive` as distinct lifecycle actions with truthful cancellation settlement.
- Attach one explicit idea-owned working directory / project-root contract to project-backed workflows.
- Make run-centric provider/model truth come from frozen resolved binding plus frozen provenance.

### Locked Decisions
- `Stop` and `Archive` are separate lifecycle actions.
- `requires_project_access` is the single authoritative selector shared by Start Run, preflight, compiler, and resume.
- Run-centric surfaces must prefer frozen resolved binding truth over catalog shorthand.
- Provenance must be frozen at run start and must never be reconstructed from mutable current provider settings.

### Acceptance Criteria
- Active ideas expose a stop action from the owner path and stopping propagates cancellation truthfully.
- Cancellation becomes terminal `cancelled` only after settlement is recorded.
- Project-backed runs fail closed without a valid idea-owned working directory.
- Run/workspace freeze the idea-owned directory used for that run.
- Run surfaces show resolved binding truth plus provenance and do not normalize ambiguous mixed labels as ordinary truth.

### Test / Evidence Requirements
- Focused Apple-platform proof for stop/run-control behavior.
- Focused proof for working-directory fail-closed behavior and frozen workspace state.
- Focused proof for frozen provider binding/provenance behavior and historical explanation after settings drift.

### Explicit Exclusions
- No new provider families.
- No new archive taxonomy.
- No workflow-map redesign.
- No multi-user repository assignment.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 4 |
| Partially Implemented | 5 |
| Missing | 1 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Active ideas expose a dedicated stop action separate from archive affordances
- Proposal Source: `2. Product questions` / `4.3 Operator surface rules` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:36-37`, `148-151`, `342-349`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:406`
  - `Chainworks Forge/Views/IdeaListView.swift:419`
  - `Chainworks Forge/Views/IdeaListView.swift:438`
- Gap / Note: The idea owner path now has a dedicated destructive `Stop Run` action and confirmation flow, separate from archive affordances.

### REQ-002 Cancellation is settled, recorded, and shown as `cancelling…` before truthful terminal `cancelled`
- Proposal Source: `4.2 Stop semantics` / `4.2.1 Cancellation settlement criteria` / `7. Data and runtime model additions` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:103-145`, `315-333`, `343-346`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:49`
  - `Chainworks Forge/Models/Run.swift:139`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:18`
  - `Chainworks Forge/Engine/ExecutionService.swift:267`
  - `Chainworks Forge/Views/IdeaListView.swift:419`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r2-unit-dd -resultBundlePath /tmp/p011-audit-r2-unit.xcresult test -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testMalformedReviewJSONFailsBeforeTransitionEvaluation'` (`Failed`)
- Gap / Note: The settlement model is present in code, but the fresh targeted unit audit crashed in `ResumeManagerTests/executionServiceCancelRun()` and `ResumeManagerTests/executionServiceUsesLiveExecutorForLiveWorkflow()`. That keeps truthful cancellation from being production-ready.

### REQ-003 Cancelled runs remain visible as terminal history and archive eligibility stays separate
- Proposal Source: `4.3 Operator surface rules` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:148-151`, `347-349`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:360`
  - `Chainworks Forge/Views/IdeaListView.swift:458`
  - `Chainworks Forge/Views/RunsHomeView.swift:202`
  - `Chainworks Forge/Engine/IdeaArchiveService.swift:53`
- Gap / Note: Archive remains its own lifecycle path and run-centric surfaces still treat cancelled runs as visible terminal history.

### REQ-004 Workflow YAML declares `requires_project_access`, and compilation parses it into `RunPlan.requiresProjectAccess`
- Proposal Source: `5.2 Project access requirement selector` / `7. Data and runtime model additions` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:163-191`, `324-326`, `352`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/DSL/WorkflowDefinition.swift:103`
  - `Chainworks Forge/DSL/WorkflowDefinition.swift:118`
  - `Chainworks Forge/Engine/RunPlan.swift:28`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:45`
- Gap / Note: The YAML field now exists and is compiled into `RunPlan`.

### REQ-005 Start Run, preflight, compiler, and resume all consume one authoritative project-access selector
- Proposal Source: `5.2 Project access requirement selector` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:182-191`, `353`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1243`
  - `Chainworks Forge/Engine/PreflightService.swift:89`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:45`
  - `Chainworks Forge/Engine/ResumeManager.swift:73`
- Gap / Note: Start Run, preflight, and compiler all consume `plan.requiresProjectAccess`, but the resume path still does not explicitly validate `frozenWorkspaceRootPath` or re-enforce project-access failure semantics when the frozen directory disappears.

### REQ-006 Each project-backed idea owns one explicit working-directory contract, and live start fails closed when it is missing or invalid
- Proposal Source: `5.1 Core rule` / `5.3 Rules` / `7. Data and runtime model additions` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:157-199`, `311-314`, `354-356`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Idea.swift:10`
  - `Chainworks Forge/Views/IdeaListView.swift:326`
  - `Chainworks Forge/Engine/PreflightService.swift:283`
  - `Chainworks Forge/Views/IdeaListView.swift:1249`
- Gap / Note: Ideas now persist `workspaceRootPath`, the idea owner path edits it, preflight validates it, and live start blocks when the required directory is missing or invalid.

### REQ-007 Run/workspace freeze the idea-owned directory and execution no longer relies on ambient cwd for project selection
- Proposal Source: `5.3 Rules` / `5.4 Frozen run contract` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:195-207`, `356-357`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:49`
  - `Chainworks Forge/Views/IdeaListView.swift:1283`
  - `Chainworks Forge/Views/IdeaListView.swift:1289`
- Gap / Note: The run now freezes `frozenWorkspaceRootPath`, but the delivery branch still retains an ambient `currentDirectoryPath` fallback in code. Even if normal project-backed flows now guard against it, the implementation is not yet as fail-closed as the proposal requires.

### REQ-008 Run-centric surfaces show resolved provider/model truth from the frozen binding snapshot
- Proposal Source: `6.1 Problem statement` / `6.2 Four facts the UI must distinguish` / `7. Data and runtime model additions` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:213-247`, `320-334`, `361`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:44`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:120`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift:23`
  - `Chainworks Forge/Views/IdeaListView.swift:2011`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:150`
  - `Chainworks Forge/Engine/RunComparisonService.swift:124`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1200`
  - `xcodebuild ... /tmp/p011-audit-r2-unit.xcresult` (`Failed`; however `Sample run launcher creates frozen provider binding snapshot` passed before the later crash)
- Gap / Note: Frozen binding snapshots exist and are consumed by parts of runtime/UI, but report and comparison surfaces still fall back to `resolvedBackendProfileID` when `resolvedModel` is absent. That keeps binding truth inconsistent across run-centric surfaces.

### REQ-009 Frozen binding provenance is persisted per agent and historical surfaces explain model origin using frozen data only
- Proposal Source: `6.4 Provenance` / `6.5 Provenance must be reproducible from frozen data only` / `7. Data and runtime model additions` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:249-303`, `320-334`, `362-364`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `rg -n "FrozenBindingProvenance|BindingProvenanceSource|bindingProvenanceJSON|unverifiable" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"` (no hits)
  - `Chainworks Forge/Models/Run.swift:44`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:31`
- Gap / Note: The repository still freezes only `ResolvedProviderBinding`. There is no persisted provenance source, no `bindingProvenanceJSON`, and no historical explanation path for backend-profile default vs configured-provider default vs run override.

### REQ-010 Cross-family mismatches are blocked or surfaced as explicit warnings, and ambiguous mixed labels are not shown as normal truth
- Proposal Source: `6.3 Coherence policy` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:240-247`, `365-366`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:41`
  - `Chainworks Forge/Engine/PreflightService.swift:107`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:150`
  - `Chainworks Forge/Engine/RunComparisonService.swift:124`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:317`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:367`
- Gap / Note: Resolver/preflight already block some invalid provider/model situations, but the runtime still lacks a dedicated warning/provenance path for unusual bindings, and some report/comparison surfaces continue to normalize fallback labels as ordinary truth.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md`
- `git rev-parse --short HEAD`
- `git status --porcelain | wc -l`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md docs/proposals docs/reviews`
- `rg -n "IdeaStopService|RunCancellationCoordinator|workspaceRootPath|requiresProjectAccess|requires_project_access|bindingProvenanceJSON|FrozenBindingProvenance|BindingProvenanceSource|cancellationRequestedAt|cancellationSettledAt|cancellationSettlementLog|Stop Run|cancelling" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"`
- `rg -n "providerBindingSnapshotJSON|resolvedModel|resolvedProviderFamily|resolvedBackendProfileID|configuredProviderDefaultModel|runOverrideModel|unverifiable" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r2-build-dd -resultBundlePath /tmp/p011-audit-r2-build.xcresult build`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r2-unit-dd -resultBundlePath /tmp/p011-audit-r2-unit.xcresult test -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testMalformedReviewJSONFailsBeforeTransitionEvaluation'`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r2-ui-dd -resultBundlePath /tmp/p011-audit-r2-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface'`

## Recommended Next Actions

- Persist a real `FrozenBindingProvenance` model and wire historical run/report/comparison surfaces to it instead of inferring origin from current settings or falling back to backend-profile labels.
- Fix the fresh cancellation/live-executor crash surfaced in `ResumeManagerTests/executionServiceCancelRun()` and `ResumeManagerTests/executionServiceUsesLiveExecutorForLiveWorkflow()`.
- Remove the remaining ambient cwd fallback from project-selection codepaths and make resume re-validate the frozen workspace contract explicitly.
