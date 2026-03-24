# Consolidated Review

## 0. Review Mode and Evidence Summary
- Mode used: `full-review` without product overlay
- Evidence completeness: `Partial`
- Review round note: repeat review round. The proposal source changed after the prior pass, so this round re-read the proposal, refreshed macOS build and targeted operator-baseline tests, exported a fresh screenshot pack, and re-checked current runtime models against HEAD.
- Documents / repo inputs reviewed:
  - [005-operator-experience-reports-recovery-and-run-comparison.md](../proposals/005-operator-experience-reports-recovery-and-run-comparison.md)
  - [002-workflow-execution-engine.md](../proposals/002-workflow-execution-engine.md)
  - [live-provider-execution-slice.md](../reference/live-provider-execution-slice.md)
  - [005-goose-server-transport-adapter.md](../proposals/005-goose-server-transport-adapter.md)
  - [007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md](../proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md)
  - [chainworks-forge-mvp.md](../ps/chainworks-forge-mvp.md)
- External sources reviewed: none
- Build/run results (full commands in [evidence pack](005-operator-experience-reports-recovery-and-run-comparison-evidence-pack.md)):
  - Fresh macOS build: passed
  - Fresh targeted operator-baseline slice: passed
    - `ResumeManagerTests.testExecutionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage()` passed
    - `testApprovalInboxReachable` passed
    - `testRunProgressViewSurface` passed
    - `testStageDetailViewSurface` skipped in headless macOS
    - `testArtifactInspectorViewSurface` skipped in headless macOS
- Fresh screenshot status:
  - current-head screenshots now exist for approvals inbox and run progress in [proposal-005-ops-r4-ui-2026-03-24](../reviews/artifacts/proposal-005-ops-r4-ui-2026-03-24)
  - no screenshots exist for Proposal 005-specific screens such as `RunsHomeView`, `RunReportView`, `RunComparisonView`, `RecoverySheet`, or notification surfaces because those states are still not implemented at current HEAD
- Code areas inspected:
  - App shell: [ContentView.swift](../../Chainworks%20Forge/ContentView.swift), [IdeaListView.swift](../../Chainworks%20Forge/Views/IdeaListView.swift), [ApprovalInboxView.swift](../../Chainworks%20Forge/Views/ApprovalInboxView.swift)
  - Runtime models: [Run.swift](../../Chainworks%20Forge/Models/Run.swift), [StageExecution.swift](../../Chainworks%20Forge/Models/StageExecution.swift), [AgentExecution.swift](../../Chainworks%20Forge/Models/AgentExecution.swift), [Artifact.swift](../../Chainworks%20Forge/Models/Artifact.swift)
  - Runtime provenance: [ExecutionReceiptBuilder.swift](../../Chainworks%20Forge/Engine/ExecutionReceiptBuilder.swift), [ArtifactManager.swift](../../Chainworks%20Forge/Engine/ArtifactManager.swift), [WorkflowOrchestrator.swift](../../Chainworks%20Forge/Engine/WorkflowOrchestrator.swift), [ResumeManager.swift](../../Chainworks%20Forge/Engine/ResumeManager.swift), [ExecutionService.swift](../../Chainworks%20Forge/Engine/ExecutionService.swift)
  - Current operator surfaces: [RunProgressView.swift](../../Chainworks%20Forge/Views/RunProgressView.swift), [StageDetailView.swift](../../Chainworks%20Forge/Views/StageDetailView.swift), [ArtifactInspectorView.swift](../../Chainworks%20Forge/Views/ArtifactInspectorView.swift)
  - Tests: [Chainworks_ForgeUITests.swift](../../Chainworks%20ForgeUITests/Chainworks_ForgeUITests.swift), [ResumeManagerTests.swift](../../Chainworks%20ForgeTests/ResumeManagerTests.swift)
- Remaining assumptions:
  - Proposal 005 is still intended to land before the full dedicated operator shell exists in code.
  - Report and comparison summaries are supposed to be deterministic from persisted runtime truth, not opportunistic in-memory state.

## 1. Executive Summary
- Overall readiness: `Yellow`
- Confidence: `Medium`
- Release blockers:
  1. The proposal still does not name a single deterministic source for agent-level `provider / model / effort` bindings in reports and comparison (`ARCH-004`).
- Top risks:
  1. `RunReportBuilder` and `RunComparisonService` can diverge on agent provenance if one reads `AgentExecution` and another reads receipt or output artifacts.
  2. Missing receipt artifacts or multiple attempts can make `model` ambiguous because the proposal currently requires the field but does not define a canonical fallback.
  3. Proposal 005-specific shell surfaces are still absent, so current runtime evidence proves only the inherited operator baseline, not the new command-center shell.
- Top opportunities:
  1. The earlier findings about universal row actions and “final report” semantics are now closed in the proposal text.
  2. The inherited waiting-approval recovery baseline is green again in the fresh rerun.
  3. Fresh screenshots prove approvals inbox and run progress on current HEAD, so the baseline from which Proposal 005 grows is real and stable.

## Closed Since Prior Pass
- `UX-001` closed: section 5.4 now defines contextual row actions instead of a universal always-visible action strip.
- `ARCH-002` closed: sections 6.2 and 6.3 now separate immutable report history from mutable latest summary and define versioned report checkpoints for recovery/re-arm events.
- `GAP-03` closed as runtime evidence: `ResumeManagerTests.testExecutionServiceResumeWaitingApprovalRestoresPendingApprovalWithoutReexecutingStage()` passed in the fresh rerun, so the prior waiting-approval restore regression is no longer live.

## 2. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Yellow | Medium | Partial | 0 | 0 | 0 | 0 |
| UX | Green | Medium | Partial | 0 | 0 | 0 | 0 |
| iOS Architecture | Yellow | High | Partial | 0 | 0 | 1 | 0 |

## 3. Findings by Discipline

### 3.1 UI Findings
- No live Proposal 005 UI findings surfaced against running code because Proposal 005-specific screens are still not implemented at current HEAD.
- Current screenshots and tests prove approvals inbox and run progress again, but there is still no dedicated `RunsHomeView`, `RunReportView`, `RunComparisonView`, `RecoverySheet`, notification service, or menu bar surface to review.

### 3.2 UX Findings
- No live UX findings in the current proposal text.
- The previous contextual-actions issue is closed in section 5.4, and the inherited baseline is strong enough to support the proposal's calmer-operator intent.

### 3.3 iOS Architecture Findings
- Finding ID: `ARCH-004`
  Status: `Open`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-02`, `CODE-03`, `CODE-05`
  Why it matters: section 6.4 requires run reports to list each agent's `provider`, `model`, and `effort`, and section 8.2 requires comparison over the same bindings. But section 6.5 adds report metadata only to `Artifact` and `Run`; it does not add `model` to `AgentExecution` or otherwise define where those bindings come from. Current runtime truth is split: `AgentExecution` stores `provider` and `effort`, while execution receipts and artifacts store `provider / model / effort`. Without a declared canonical source and fallback rule, deterministic reports and comparison can disagree across retries, attempts, or missing receipt artifacts.
  Recommended fix: either persist `model` directly on `AgentExecution` and declare `AgentExecution` canonical for reports/comparison, or explicitly define that `RunReportBuilder` and `RunComparisonService` reconstruct bindings from the latest successful execution receipt or artifact provenance with absence/error rules.
  Acceptance criteria: the proposal names one canonical source of truth plus fallback behavior for agent-level `provider / model / effort` bindings used in reports and comparison.
  Confidence: `High`

## 4. Cross-Discipline Conflicts and Decisions
- The current runtime baseline is stronger than in the previous pass:
  - approvals inbox is reachable,
  - run progress is reachable,
  - waiting-approval restore is green again under targeted test.
- The current runtime baseline is still not the Proposal 005 shell:
  - the top-level app still exposes only `Ideas`, `Approvals`, `Agent Catalog`, and `Workflow Inspector`,
  - repo-wide search still finds no `RunsHomeView`, `RunReportView`, `RunComparisonView`, `RecoveryCoordinator`, or `NotificationService`.
- Decision:
  - keep this pass in `Evidence Gap Review` mode
  - drop the previously closed proposal findings
  - keep one live architecture finding about the agent-provenance source of truth

## 5. Prioritized Action Backlog
| Priority | Item | Discipline | Horizon | Dependencies | Success Metric | Source |
|---|---|---|---|---|---|---|
| P0 | Define the canonical source and fallback rules for agent-level `provider / model / effort` in reports and comparison | Architecture | Immediate | report/comparison data contract | two builders cannot disagree on provenance for the same run | `ARCH-004` |
| P1 | Implement Proposal 005 shell surfaces and capture current-head screenshots for Runs Home, report view, comparison view, recovery sheet, and notification states | UI / Architecture | Next | Proposal 005 implementation | evidence pack covers the real operator shell instead of only the inherited baseline | `GAP-01` |
| P1 | Add targeted tests for report generation and comparison provenance once the canonical source is chosen | Architecture | Next | finalized provenance rule | tests fail if report/comparison provenance drifts across retries or missing receipts | `ARCH-004` |

## 6. Validation and Measurement Plan
| Area | Leading Indicators | Guardrails | Hold Criteria |
|---|---|---|---|
| Agent provenance contract | report and comparison tests derive identical `provider / model / effort` tuples for the same run | one canonical source plus explicit fallback | hold if `model` can be missing or derived differently per surface |
| Operator shell implementation | Runs Home, report, comparison, recovery, and notification states are reachable on `My Mac` | no hidden debug-only path | hold if only inherited approvals/run-progress surfaces are provable |
| Recovery baseline | waiting-approval resume remains green without re-executing the paused stage | no regression in pending-approval restoration | hold if resume loses approval state again |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: No Proposal 005-specific UI states are reachable at current HEAD. There is still no dedicated `RunsHomeView`, `RunReportView`, `RunComparisonView`, `RecoverySheet`, notification service, or menu bar surface in code.
- `GAP-02`: Fresh screenshots exist only for the inherited operator baseline. Proposal 005 target screens are still absent.

### Open Questions
- `QUESTION-01`: Should the canonical report/comparison provenance source be `AgentExecution`, receipt artifacts, or another normalized runtime record?
- `QUESTION-02`: If receipt artifacts are canonical, what exact fallback behavior applies when a run attempt is missing a receipt but still has output artifacts?

### Partial-Confidence Assessment
- What can already be said with confidence: the proposal text is materially stronger than the prior pass, the old quick-action and report-lifecycle findings are closed, and the inherited operator baseline is green again.
- What is still missing for a defensible full review: real Proposal 005 screens, report generation, comparison UI, recovery sheet behavior, and notification surfaces.
- What this review can still say with partial confidence: the remaining design problem is narrow and architectural. The proposal now mainly needs an explicit provenance source-of-truth rule before implementation starts.
