# Proposal 011: Run Control, Working Directory Ownership, and Provider Binding Truth Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md |
| Repository Root | . |
| Git SHA | 0b2ca31 |
| Working Tree | dirty (43 modified, 14 untracked) |
| Audited At | 2026-03-26T21:53:30+0200 |
| Proposal State | Active (Draft) |
| Overall Status | Not Implemented |

## Verdict

Proposal 011 is not implemented on the current `HEAD`. The repository is healthy enough to build and pass focused owner-path and provider-platform tests in this audit, but the core Proposal 011 contracts are still absent: there is no stop owner-path in the idea UI, cancellation still flips directly to settled `cancelled` without settlement evidence, project-backed ideas still have no explicit persisted working-directory contract, and frozen provider bindings still do not persist provenance truth.

## Proposal Contract

### Scope
- Separate `Stop` from `Archive` as distinct lifecycle actions, with truthful terminal cancellation semantics.
- Attach one explicit idea-owned working directory / project-root contract to every project-backed workflow.
- Make run-centric provider/model truth come from the frozen resolved binding plus frozen provenance.

### Locked Decisions
- Archive never implies stop; active work must settle before archive becomes eligible.
- `requires_project_access` is the single authoritative selector shared by Start Run, preflight, compiler, and resume.
- Provider/model truth must prefer frozen resolved runtime binding over catalog shorthand.
- Provenance must be frozen at run start and must never be reconstructed from mutable current provider settings.

### Acceptance Criteria
- Active ideas expose a stop action from the idea owner path, and stop/archive remain distinct.
- Cancellation is propagated and only becomes truthful `cancelled` after settlement is recorded.
- Project-backed runs fail closed without one explicit valid idea-owned working directory.
- Run/workspace freeze the idea directory used for the run and do not rely on ambient app cwd.
- Run surfaces show resolved provider/model truth and frozen provenance, and cross-family ambiguity is blocked or explicitly warned.

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
| Implemented | 0 |
| Partially Implemented | 3 |
| Missing | 7 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Active ideas expose a dedicated stop action separate from archive affordances
- Proposal Source: `2. Product questions` / `4.3 Operator surface rules` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:36-37`, `148-151`, `342-349`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:311`
  - `Chainworks Forge/Views/IdeaListView.swift:353`
  - `Chainworks Forge/Engine/ExecutionService.swift:267`
  - `rg -n "cancelRun\\(|Stop Idea|Stop Run" "Chainworks Forge"` (only `ExecutionService.cancelRun` surfaced)
- Gap / Note: The idea owner path exposes archive controls and `Start New Run`, but no stop control is wired from UI to `ExecutionService.cancelRun`.

### REQ-002 Cancellation is settled, recorded, and shown as `cancelling…` before truthful terminal `cancelled`
- Proposal Source: `4.2 Stop semantics` / `4.2.1 Cancellation settlement criteria` / `7. Data and runtime model additions` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:103-145`, `315-333`, `343-346`)
- Status: Missing
- Evidence Type: code, tests-found
- Evidence:
  - `Chainworks Forge/Models/Run.swift:44`
  - `Chainworks Forge/Models/Run.swift:113`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:147`
  - `Chainworks Forge/Engine/ExecutionService.swift:267`
  - `Chainworks ForgeTests/OrchestratorTests.swift:617`
- Gap / Note: `WorkflowOrchestrator.cancel()` immediately marks the run `.cancelled`; there are no `cancellationRequestedAt`, `cancellationSettledAt`, `cancellationSettlementLog`, or `cancelling…` presentation state. The existing cancellation test only proves the immediate flip to `.cancelled`.

### REQ-003 Cancelled runs remain visible as terminal history and archive eligibility stays separate
- Proposal Source: `4.3 Operator surface rules` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:148-151`, `347-349`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:113`
  - `Chainworks Forge/Engine/IdeaArchiveService.swift:34`
  - `Chainworks Forge/Engine/IdeaArchiveService.swift:73`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:120`
- Gap / Note: Cancelled runs are treated as terminal history and archive policy already excludes only active statuses, but the missing settlement contract means current `.cancelled` truth is not yet trustworthy in the Proposal 011 sense.

### REQ-004 Workflow YAML declares `requires_project_access`, and compilation parses it into `RunPlan.requiresProjectAccess`
- Proposal Source: `5.2 Project access requirement selector` / `7. Data and runtime model additions` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:163-191`, `324-326`, `352`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunPlan.swift:7`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:17`
  - `rg -n "requiresProjectAccess|requires_project_access" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests" examples/workflows` (no hits)
- Gap / Note: The compiled plan and compiler path do not carry any `requiresProjectAccess` selector, and the repository has no implementation evidence for YAML parsing of this field.

### REQ-005 Start Run, preflight, compiler, and resume all consume one authoritative project-access selector
- Proposal Source: `5.2 Project access requirement selector` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:182-191`, `353`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:1122`
  - `Chainworks Forge/Engine/PreflightService.swift:44`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:74`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:101`
- Gap / Note: These paths do not share any typed project-access selector today. Preflight only checks run-storage and provider state; it does not consume idea-owned workspace requirements.

### REQ-006 Each project-backed idea owns one explicit working-directory contract, and live start fails closed when it is missing or invalid
- Proposal Source: `5.1 Core rule` / `5.3 Rules` / `7. Data and runtime model additions` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:157-199`, `311-314`, `354-356`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Idea.swift:4`
  - `Chainworks Forge/Views/IdeaListView.swift:1148`
  - `Chainworks Forge/Engine/PreflightService.swift:78`
  - `rg -n "workspaceRootPath|IdeaWorkspaceEditor|WorkspaceReadinessProbe" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"` (no hits)
- Gap / Note: `Idea` has no persisted `workspaceRootPath`, there is no canonical workspace editor/probe, and live start does not fail closed on a missing idea-owned directory.

### REQ-007 Run/workspace freeze the idea-owned directory and execution no longer relies on ambient cwd for project selection
- Proposal Source: `5.3 Rules` / `5.4 Frozen run contract` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:195-207`, `356-357`)
- Status: Missing
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunPlan.swift:45`
  - `Chainworks Forge/Models/Run.swift:22`
  - `Chainworks Forge/Views/IdeaListView.swift:1098`
  - `Chainworks Forge/Views/IdeaListView.swift:1109`
  - `Chainworks Forge/Views/IdeaListView.swift:1176`
  - `Chainworks Forge/Views/IdeaListView.swift:1246`
- Gap / Note: The frozen run/workspace model does not store an idea-owned working directory, and current owner paths still fall back to `FileManager.default.currentDirectoryPath` for project/repo selection.

### REQ-008 Run-centric surfaces show resolved provider/model truth from the frozen binding snapshot
- Proposal Source: `6.1 Problem statement` / `6.2 Four facts the UI must distinguish` / `7. Data and runtime model additions` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:213-247`, `320-334`, `361`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:44`
  - `Chainworks Forge/Views/IdeaListView.swift:1159`
  - `Chainworks Forge/Views/IdeaListView.swift:1169`
  - `Chainworks Forge/Engine/WorkflowOrchestrator.swift:120`
  - `Chainworks Forge/Engine/WorkflowMapProjectionService.swift:23`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:146`
  - `Chainworks Forge/Engine/RunComparisonService.swift:114`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1200`
  - `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -resultBundlePath /tmp/p011-audit-unit.xcresult` (passed)
- Gap / Note: Frozen binding snapshots exist and are consumed by parts of runtime/UI, but report/comparison surfaces still fall back to `resolvedBackendProfileID` when `resolvedModel` is nil, so binding truth is not rendered consistently across all run-centric surfaces.

### REQ-009 Frozen binding provenance is persisted per agent and historical surfaces explain model origin using frozen data only
- Proposal Source: `6.4 Provenance` / `6.5 Provenance must be reproducible from frozen data only` / `7. Data and runtime model additions` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:249-303`, `320-334`, `362-364`)
- Status: Missing
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:44`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:49`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:1277`
  - `rg -n "FrozenBindingProvenance|BindingProvenanceSource|bindingProvenanceJSON" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"` (no hits)
- Gap / Note: Current runtime freezes `ResolvedProviderBinding`, but it does not persist provenance source (`backend_profile`, `configured_provider`, `run_override`, `unverifiable`) or the frozen provider-default snapshot required to explain historical runs after settings drift.

### REQ-010 Cross-family mismatches are blocked or surfaced as explicit warnings, and ambiguous mixed labels are not shown as normal truth
- Proposal Source: `6.3 Coherence policy` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:240-247`, `365-366`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:45`
  - `Chainworks Forge/Engine/PreflightService.swift:96`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:146`
  - `Chainworks Forge/Engine/RunComparisonService.swift:114`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:317`
  - `Chainworks ForgeTests/ProviderPlatformTests.swift:367`
  - `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -resultBundlePath /tmp/p011-audit-unit.xcresult` (passed)
- Gap / Note: Resolver/preflight already reject some unavailable bindings and support family-specific provider selection, but there is still no dedicated coherence policy or explicit provenance/warning path for ambiguous cross-family model truth, and report/comparison surfaces still normalize some ambiguous combinations as ordinary labels.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md`
- `git rev-parse --show-toplevel && git rev-parse --short HEAD && git status --short`
- `rg -n "superseded|deprecated|replaced by|obsolete" docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md docs/proposals docs/reviews`
- `rg -n "cancellationRequestedAt|cancellationSettledAt|cancellationSettlementLog|CancellationSettlementEntry|cancelling|IdeaStopService|RunCancellationCoordinator|stop action|stop-idea|stop-run|cancel" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"`
- `rg -n "workspaceRootPath|requiresProjectAccess|requires_project_access|RunWorkspaceFreezer|IdeaWorkspaceEditor|WorkspaceReadinessProbe|ambient app cwd|currentDirectoryPath" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests" examples/workflows`
- `rg -n "FrozenBindingProvenance|BindingProvenanceSource|bindingProvenanceJSON|providerBindingSnapshotJSON|resolvedModel|resolvedProviderFamily|runOverride|configuredProviderDefault|unverifiable|coherence|cross-family|warning" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"`
- `xcodebuild build -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath /tmp/p011-audit-build.xcresult`
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -resultBundlePath /tmp/p011-audit-ui.xcresult`
- `xcodebuild test -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -resultBundlePath /tmp/p011-audit-unit.xcresult`

## Recommended Next Actions

- Add the stop owner path and a real cancellation-settlement pipeline: `IdeaStopService`, `RunCancellationCoordinator`, settlement fields, settlement log, and a `cancelling…` state.
- Add the typed working-directory contract end-to-end: `workspaceRootPath` on idea-owned state, `requires_project_access` in YAML/`RunPlan`, and fail-closed enforcement in Start Run, preflight, compiler, and resume.
- Freeze binding provenance at run start and remove ambiguous fallback labels from run report/comparison surfaces.
