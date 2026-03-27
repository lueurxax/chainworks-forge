# Proposal 011: Run Control, Working Directory Ownership, and Provider Binding Truth Implementation Audit R3

| Field | Value |
|---|---|
| Proposal | docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md |
| Repository Root | . |
| Git SHA | 63f5270 |
| Working Tree | dirty |
| Audited At | 2026-03-26T23:10:45+0200 |
| Proposal State | Active (Draft) |
| Overall Status | Partial |

## Verdict

Proposal 011 is no longer `Not Implemented` on current `HEAD`, but it is still not fully implemented. The big R2 blocker is closed: frozen binding provenance now exists as a persisted run snapshot, start-run freezes it at run creation, resume revalidates the frozen workspace contract, and the focused macOS UI slice passed `5/5`. The remaining gaps are narrower but still real: cancellation settlement is still not truthful to the proposal contract because `RunCancellationCoordinator` marks runs settled before Goose session closure is confirmed, and the fresh targeted unit slice still fails in two cancel-path tests. On the provider/model side, frozen provenance is persisted, but provenance explanation has not propagated to all run-centric surfaces yet.

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
- Active ideas expose a stop action from the idea owner path.
- Stopping an idea cancels the active run and propagates cancellation to in-flight agent work.
- `RunCancellationCoordinator` confirms all four settlement criteria before writing `cancellationSettledAt`.
- A run with `cancellationRequestedAt` set but `cancellationSettledAt` nil displays as `cancelling…`.
- `cancellationSettlementLog` records per-agent terminal status and session-close outcome.
- `requires_project_access` is declared in workflow YAML and parsed into `RunPlan`.
- Start Run, preflight, compiler, and resume all read the same `requiresProjectAccess` selector.
- Each project-backed idea has one explicit working directory / project root contract.
- Live start fails closed when the required idea directory is missing or invalid.
- Frozen run/workspace state stores the idea-owned directory used for that run.
- Agent execution does not rely on ambient app cwd for project selection.
- Run surfaces show resolved provider/model truth from the frozen binding.
- `FrozenBindingProvenance` is persisted per agent binding at run start.
- The UI can explain backend-profile default vs configured-provider default vs run override using frozen data only.
- Historical runs remain correctly explained after provider settings drift.
- Cross-family provider/model mismatches are blocked or surfaced as explicit warnings.

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
| Implemented | 5 |
| Partially Implemented | 5 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Active ideas expose a dedicated stop action separate from archive affordances
- Proposal Source: `4.3 Operator surface rules` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:146-151`, `342-348`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:433`
  - `Chainworks Forge/Views/IdeaListView.swift:438`
  - `Chainworks Forge/Views/IdeaListView.swift:444`
- Gap / Note: The idea owner path exposes a dedicated destructive `Stop` action with its own confirmation copy, separate from archive behavior.

### REQ-002 Cancellation is settled, recorded, and shown as `cancelling…` before truthful terminal `cancelled`
- Proposal Source: `4.2 Stop semantics` / `4.2.1 Cancellation settlement criteria` / `7. Data and runtime model additions` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:101-145`, `315-333`, `343-346`)
- Status: Partially Implemented
- Evidence Type: code, tests-run
- Evidence:
  - `Chainworks Forge/Models/Run.swift:54`
  - `Chainworks Forge/Models/Run.swift:139`
  - `Chainworks Forge/Engine/RunCancellationCoordinator.swift:32`
  - `Chainworks Forge/Engine/ExecutionService.swift:267`
  - `Chainworks Forge/Views/IdeaListView.swift:458`
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r3-unit-dd -resultBundlePath /tmp/p011-audit-r3-unit.xcresult test -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testMalformedReviewJSONFailsBeforeTransitionEvaluation'` (`Failed: 37 tests, 2 failed`)
- Gap / Note: The model fields and presentation status exist, but `settleSync()` still writes `cancellationSettledAt` and terminal `.cancelled` before Goose session closure is attempted, and it persists `sessionCloseSucceeded: nil` for open sessions instead of confirmed close outcomes. The same cancel-path proof is still unstable in fresh tests: `ResumeManagerTests/executionServiceCancelRun()` and `ResumeManagerTests/executionServiceUsesLiveExecutorForLiveWorkflow()` failed in `/tmp/p011-audit-r3-unit.xcresult`.

### REQ-003 Cancelled runs remain visible as terminal history and archive eligibility stays separate
- Proposal Source: `4.3 Operator surface rules` / `8. Acceptance criteria > Stop / run control` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:148-151`, `347-348`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Views/IdeaListView.swift:448`
  - `Chainworks Forge/Views/RunsHomeView.swift:276`
  - `Chainworks Forge/Engine/IdeaArchiveService.swift:74`
- Gap / Note: Cancelled runs remain visible as terminal history and archive eligibility remains a separate lifecycle rule.

### REQ-004 Workflow YAML declares `requires_project_access`, and compilation parses it into `RunPlan.requiresProjectAccess`
- Proposal Source: `5.2 Project access requirement selector` / `7. Data and runtime model additions` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:163-191`, `324-326`, `352`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/DSL/WorkflowDefinition.swift:114`
  - `Chainworks Forge/Engine/RunPlan.swift:27`
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:44`
- Gap / Note: The YAML field is modeled in `ExecutionConfig`, propagated into `RunPlan`, and defaults to `false` for backward compatibility.

### REQ-005 Start Run, preflight, compiler, and resume all consume one authoritative project-access selector
- Proposal Source: `5.2 Project access requirement selector` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:165-191`, `353`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/RunPlanCompiler.swift:53`
  - `Chainworks Forge/Views/IdeaListView.swift:1251`
  - `Chainworks Forge/Engine/PreflightService.swift:89`
  - `Chainworks Forge/Engine/ResumeManager.swift:111`
- Gap / Note: The same `requiresProjectAccess` flag now drives Start Run, preflight, compiled plan shape, and resume validation.

### REQ-006 Each project-backed idea owns one explicit working-directory contract, and live start fails closed when it is missing or invalid
- Proposal Source: `5.1 Core rule` / `5.3 Rules` / `7. Data and runtime model additions` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:157-199`, `311-314`, `354-355`)
- Status: Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Idea.swift:9`
  - `Chainworks Forge/Views/IdeaListView.swift:313`
  - `Chainworks Forge/Views/IdeaListView.swift:1251`
  - `Chainworks Forge/Engine/PreflightService.swift:307`
- Gap / Note: `Idea.workspaceRootPath` is persisted, editable from the idea owner path, validated in preflight, and enforced as fail-closed at live start.

### REQ-007 Run/workspace freeze the idea-owned directory and execution no longer relies on ambient cwd for project selection
- Proposal Source: `5.3 Rules` / `5.4 Frozen run contract` / `8. Acceptance criteria > Working directory` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:195-207`, `356-357`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:50`
  - `Chainworks Forge/Views/IdeaListView.swift:1286`
  - `Chainworks Forge/Views/IdeaListView.swift:1293`
  - `Chainworks Forge/Views/IdeaListView.swift:1364`
- Gap / Note: The normal start path freezes `run.frozenWorkspaceRootPath`, but the delivery-specific `fullMVPLive` branch still retains `FileManager.default.currentDirectoryPath` fallback when both `idea.workspaceRootPath` and `deliveryRepoRoot` are absent. That keeps one in-scope project-selection path dependent on ambient cwd.

### REQ-008 Run-centric surfaces show resolved provider/model truth from the frozen binding snapshot
- Proposal Source: `6.1 Problem statement` / `6.2 Four facts the UI must distinguish` / `7. Data and runtime model additions` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:213-247`, `331-334`, `361`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:44`
  - `Chainworks Forge/Views/IdeaListView.swift:1279`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:145`
  - `Chainworks Forge/Engine/RunComparisonService.swift:114`
  - `Chainworks Forge/Views/IdeaListView.swift:2022`
- Gap / Note: `RunReportBuilder` and `RunComparisonService` now prefer the frozen binding snapshot, but stage-detail / run-progress agent metadata still renders `AgentExecution.resolvedModel` directly and does not read the frozen snapshot first. Provider/model truth is therefore only partially normalized across run-centric surfaces.

### REQ-009 Frozen binding provenance is persisted per agent and historical surfaces explain model origin using frozen data only
- Proposal Source: `6.4 Provenance` / `6.5 Provenance must be reproducible from frozen data only` / `7. Data and runtime model additions` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:249-303`, `320-334`, `362-364`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Models/Run.swift:47`
  - `Chainworks Forge/Providers/BackendProfileResolverV2.swift:83`
  - `Chainworks Forge/Views/IdeaListView.swift:1282`
  - `Chainworks Forge/Engine/RunReportBuilder.swift:147`
  - `Chainworks Forge/Views/RunComparisonView.swift:161`
- Gap / Note: The frozen provenance model now exists and is persisted at run start, and `RunReportBuilder` can read it from the frozen snapshot. But provenance explanation still has not propagated to all operator-facing run surfaces: `RunComparisonView` still renders only `provider / model / effort`, and stage-detail metadata has no source explanation. Historical provenance truth is therefore stored, but not fully surfaced.

### REQ-010 Cross-family mismatches are blocked or surfaced as explicit warnings, and ambiguous mixed labels are not shown as normal truth
- Proposal Source: `6.3 Coherence policy` / `8. Acceptance criteria > Provider/model truth` (`docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md:240-247`, `365-366`)
- Status: Partially Implemented
- Evidence Type: code
- Evidence:
  - `Chainworks Forge/Engine/PreflightService.swift:129`
  - `Chainworks Forge/Engine/PreflightService.swift:281`
  - `Chainworks Forge/Views/RunComparisonView.swift:166`
  - `Chainworks Forge/Views/IdeaListView.swift:2024`
- Gap / Note: Preflight now emits a cross-family warning, but it is still a heuristic prefix-based check and there is no dedicated surfaced warning/provenance panel in the run-detail UI. Operator-facing bindings can still appear as ordinary `provider / model / effort` text without an explicit warning state.

## Verification Log

- `python3 /Users/user/.codex/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks\ Forge/docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md`
- `git rev-parse --short HEAD`
- `git status --short`
- `nl -ba docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md | sed -n '1,380p'`
- `rg -n "bindingProvenanceJSON|FrozenBindingProvenance|BindingProvenanceSource|requiresProjectAccess|workspaceRootPath|cancellationSettledAt|cancellationSettlementLog" "Chainworks Forge" "Chainworks ForgeTests" "Chainworks ForgeUITests"`
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r3-build-dd -resultBundlePath /tmp/p011-audit-r3-build.xcresult build` (`Passed`)
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r3-unit-dd -resultBundlePath /tmp/p011-audit-r3-unit.xcresult test -only-testing:'Chainworks ForgeTests/ProviderPlatformTests' -only-testing:'Chainworks ForgeTests/ResumeManagerTests' -only-testing:'Chainworks ForgeTests/OrchestratorTests/testMalformedReviewJSONFailsBeforeTransitionEvaluation'` (`Failed: 37 tests, 2 failed`)
- `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p011-audit-r3-ui-dd -resultBundlePath /tmp/p011-audit-r3-ui.xcresult test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsTabReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface'` (`Passed: 5 tests`)

## Recommended Next Actions

- Make cancellation settlement truly wait for criterion 3: record actual Goose `closeSession` attempts/results before writing `cancellationSettledAt` and terminal `.cancelled`.
- Remove the remaining `currentDirectoryPath` fallback from the delivery-specific repo-root path.
- Surface frozen provenance source in operator-facing run surfaces such as comparison and stage-detail views, not only in stored payload/report generation.
