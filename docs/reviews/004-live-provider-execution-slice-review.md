# Consolidated Review

## 0. Review Mode and Evidence Summary
- Mode used: `full-review` without product overlay
- Evidence completeness: `Partial`
- Review round note:
  - This is a repeat review round. I refreshed only the proposal-adjacent app/runtime evidence that materially changed and did not carry forward stale findings from the prior version.
- Documents / repo inputs reviewed:
  - [004-live-provider-execution-slice.md](/Users/user/Documents/Chainworks Forge/docs/proposals/004-live-provider-execution-slice.md)
  - [002-workflow-execution-engine.md](/Users/user/Documents/Chainworks Forge/docs/proposals/002-workflow-execution-engine.md)
  - [chainworks-forge-mvp.md](/Users/user/Documents/Chainworks Forge/docs/ps/chainworks-forge-mvp.md)
  - [workspace-isolation-risk.md](/Users/user/Documents/Chainworks Forge/docs/reference/workspace-isolation-risk.md)
- External sources reviewed: none
- Build/run attempts:
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-proposal004-build build` passed
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-proposal004-tests test -only-testing:'Chainworks ForgeTests/LiveProposalWorkflowTests' -only-testing:'Chainworks ForgeTests/GooseAgentExecutorTests' -only-testing:'Chainworks ForgeTests/GooseSessionBridgeTests'` passed; the dedicated live-workflow tests now execute instead of skipping
  - `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-proposal004-ui test -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testProductCheckpointExecutionFlowReachable' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface' -only-testing:'Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface'` failed; approval inbox evidence was captured, but the execution-surface tests failed before Start Run / run-progress capture
- Screenshots captured:
  - approval inbox screenshot `REQ011_ApprovalGate` in `/tmp/codex-dd-proposal004-ui/Logs/Test/Test-Chainworks Forge-2026.03.23_09-09-07-+0200.xcresult`
  - no current screenshots for the live Start Run sheet, live progress surface, artifact inspector, or a live non-happy path
- Code areas inspected:
  - [Chainworks_ForgeApp.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Chainworks_ForgeApp.swift)
  - [ExecutionService.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ExecutionService.swift)
  - [WorkflowOrchestrator.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/WorkflowOrchestrator.swift)
  - [ExecutionEventBridge.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ExecutionEventBridge.swift)
  - [IdeaListView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift)
  - [LiveProposalWorkflowTests.swift](/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/LiveProposalWorkflowTests.swift)
  - [Chainworks_ForgeUITests.swift](/Users/user/Documents/Chainworks Forge/Chainworks ForgeUITests/Chainworks_ForgeUITests.swift)
- Remaining assumptions:
  - Proposal 004 is intended to be launched from the main app UI, not only validated at the unit-test layer
  - the first live slice may still be engineer-facing, but it should be operable without hidden repo-path or shell-state assumptions
- Remaining blockers:
  - current review runtime has no `CHAINWORKS_GOOSE_BASE_URL`, so the live path could not be exercised as a real provider-backed run
  - the app-launched live proposal loop still has no successful runtime proof in this round
  - the execution-surface UI tests still fail before Start Run / live-progress evidence capture

## 1. Executive Summary
- Overall readiness: `Red`
- Confidence: `High`
- Release blockers:
  1. Proposal 004 still lacks the one piece of evidence that matters most: a successful app-launched `proposal_loop_live` run using Goose-backed execution, with screenshots and persisted runtime artifacts.
- Top risks:
  1. The live path is now environment-gated rather than missing, but the current product still depends on out-of-band Goose configuration and cannot self-complete the core task in an unconfigured runtime.
  2. The live workflow resource is not bundled with the app, so launchability depends on repo-relative paths or a specific home-directory layout.
  3. UI evidence for Start Run and live progress remains unstable because the execution-surface tests fail before they can capture those states.
- Top opportunities:
  1. The prior round's biggest code blockers are closed: per-run Goose executor selection exists, the live workflow is selectable in the Start Run sheet, live timeline plumbing exists, and the dedicated live-workflow unit tests now pass.
  2. The remaining work is concentrated in operability, packaging, and end-to-end runtime proof instead of missing runtime primitives.
  3. The proposal is now close enough to real use that a single deterministic integration test and screenshot pass would retire several evidence gaps at once.

## 2. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | Medium | Partial | 0 | 0 | 1 | 1 |
| UX | Red | Medium | Partial | 0 | 1 | 1 | 0 |
| iOS Architecture | Red | High | Partial | 0 | 1 | 2 | 0 |

## 3. Findings by Discipline

### 3.1 UI Findings
- Finding ID: `UI-001`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-03`, `BASE-02`
  Why it matters: the Start Run sheet now exposes live mode, but it still gives the user too little confidence about what will actually launch. Proposal 004 calls for a provider-backed execution slice; the current sheet shows mode, workflow, and runtime status, but not the resolved provider/model/effort or the live agent set the user is about to run.
  Recommended fix: add a compact resolved-configuration block that shows provider, model, effort, resolved live agents, and read-only safety state before launch.
  Acceptance criteria: before pressing `Start Run`, the user can verify the live provider settings and the exact live agent set in one glance, and live mode is visually distinct from simulated mode without depending on explanatory copy.
  Confidence: `Medium`

- Finding ID: `UI-002`
  Severity: `Low`
  Evidence IDs: `DOC-01`, `CODE-04`, `RUN-03`, `SCR-02`
  Why it matters: the live progress surface is now wired, but the current shape is still a flat event log. For a real provider run, raw event type plus detail plus timestamp is harder to scan than a stage-centered progress surface with a single current-state summary.
  Recommended fix: keep the raw event feed, but collapse the primary surface into a stage card or status banner that highlights current phase, latest meaningful event, and next action.
  Acceptance criteria: the first viewport of an active run shows the current stage, latest meaningful progress, and next required action without scrolling; raw metadata remains available on demand.
  Confidence: `Medium`

### 3.2 UX Findings
- Finding ID: `UX-001`
  Severity: `High`
  Evidence IDs: `DOC-01`, `DOC-04`, `CODE-03`, `OPS-01`, `RUN-03`
  Why it matters: the core user journey still depends on a hidden precondition outside the app. A user can choose `Live`, see a warning, and still have no self-serve recovery path inside the flow because Goose configuration lives entirely out-of-band. For Proposal 004's primary task, that remains a task-flow and trust problem.
  Recommended fix: either add an explicit in-app setup path for live runtime configuration, or gate the live mode harder by showing the exact missing prerequisite and how to fix it before the user enters a dead end.
  Acceptance criteria: selecting `Live` always makes the configuration state obvious, names the missing prerequisite when blocked, and provides a clear recovery path; `Start Run` never becomes a silent dead end.
  Confidence: `Medium`

- Finding ID: `UX-002`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `DOC-03`, `CODE-04`, `CODE-05`, `SCR-02`
  Why it matters: the proposal's loop involves repeated draft, review, refine, and approval phases. A raw event timeline is useful supporting detail, but without a compact statement of current phase, pause reason, iteration count, and next action, interruption and recovery remain cognitively heavy.
  Recommended fix: add phase-level summaries or banners for draft, review, refine, approval, blocked, and failed states; keep the live timeline as drill-down evidence.
  Acceptance criteria: when the run is waiting approval, blocked, failed, or resumed, the top of the progress view states the current phase, loop iteration, pause reason, and next action in plain language.
  Confidence: `Low`

### 3.3 iOS Architecture Findings
- Finding ID: `ARCH-001`
  Severity: `Medium`
  Evidence IDs: `DOC-01`, `CODE-07`, `QUESTION-02`, `BLOCKER-01`
  Why it matters: `proposal-loop-live.yaml` is still resolved only from repo cwd or a fixed home-directory checkout path, not from bundle resources. That makes the first live slice brittle outside the exact local development layout and weakens rollout portability.
  Recommended fix: bundle the live workflow with the app or add a deterministic resource resolver with a surfaced failure state and test coverage.
  Acceptance criteria: a clean install resolves `proposal-loop-live.yaml` from bundle resources; CI and local tests do not depend on repo-relative workflow paths; unresolved paths fail fast with a visible config error.
  Confidence: `High`

- Finding ID: `ARCH-002`
  Severity: `Medium`
  Evidence IDs: `CODE-01`, `OPS-01`, `QUESTION-01`, `BLOCKER-01`, `DOC-01`
  Why it matters: the live path is effectively environment-gated, and startup does not perform an explicit, user-facing readiness check beyond the Start Run sheet label. That leaves operability dependent on preconfigured shell state and makes failure discovery later than it should be.
  Recommended fix: add explicit startup validation plus a visible live-config status in the app surface or settings, with live mode unavailable until the provider config is valid.
  Acceptance criteria: missing Goose config is detected before run launch, the UI explains why live mode is unavailable, and a valid configuration path is visible and testable.
  Confidence: `High`

- Finding ID: `ARCH-003`
  Severity: `High`
  Evidence IDs: `RUN-02`, `RUN-03`, `CODE-02`, `CODE-05`, `CODE-06`, `BLOCKER-02`, `BLOCKER-04`
  Why it matters: the core proposal claim is now mostly proven in units and source structure, but not yet at the app integration seam. There is still no successful app-launched `proposal_loop_live` run with live timeline evidence, approval pause, and expected artifacts, so the most important runtime contract remains unverified.
  Recommended fix: add a deterministic integration or UI test that launches `proposal_loop_live` from the app with live config injected, asserts Goose-backed executor selection, and verifies artifact/receipt persistence through approval.
  Acceptance criteria: one clean app-launched live run reaches approval with live timeline evidence and expected artifacts, and the automated harness no longer fails before the live start boundary.
  Confidence: `Medium`

## 4. Cross-Discipline Conflicts and Decisions
- Closed from the prior round:
  - the app no longer lacks a live executor-selection seam
  - the Start Run sheet no longer hardwires only the canonical workflow
  - live event plumbing is now wired into observable run-progress state
  - the dedicated live-workflow tests no longer skip
- Remaining conflict:
  - the code and unit tests now imply Proposal 004 is close to real, but the operational and UI evidence still cannot prove the core app-launched live path end-to-end
- Decision:
  - do not carry forward the previous round's stale "no live path exists" conclusions
  - keep this round in `Evidence Gap Review` mode because the minimum full-review gate still fails on runtime screenshots and successful live app execution

## 5. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P0 | Add a deterministic app-launched integration path for `proposal_loop_live` with injected live Goose config and verify approval, artifacts, and live timeline | Architecture / UX | implementation owner | Immediate | live config fixture or test backend, UI/integration harness stabilization | One automated run launches the Goose-backed live workflow from the app and reaches approval with persisted artifacts and screenshots | `ARCH-003` |
| P1 | Add explicit live configuration readiness UX instead of relying on out-of-band shell setup and a warning label | UX / Architecture | implementation owner | Next | configuration policy decision | Users can see what is missing and how to enable live mode before launch | `UX-001`, `ARCH-002` |
| P1 | Bundle `proposal-loop-live.yaml` with the app or provide a deterministic resolver with surfaced failure states | Architecture | implementation owner | Next | packaging update, resolver tests | Live workflow launch no longer depends on repo cwd or fixed home-directory paths | `ARCH-001` |
| P2 | Improve launch-time and progress-time information hierarchy for live runs | UI / UX | implementation owner | After P0 | stable live run evidence | Users can confirm provider/runtime selection before launch and understand current phase during execution without parsing raw event logs | `UI-001`, `UI-002`, `UX-002` |

## 6. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| App-launched live run | ability to start and reach approval on `proposal_loop_live` from the app | live `workflowID`, Goose session IDs, live timeline entries, persisted receipt/transcript artifacts | no implicit cwd, no missing workflow resource, no silent config failure | rerun this review after the first successful app-launched live run | hold if the app still cannot prove a real live run from the UI |
| UI evidence harness | ability to capture Start Run, progress, and paused/failure screenshots reliably | `createTestIdea(...)` succeeds, `Start Run` sheet appears, run-progress sections render, attachments are captured | no headless-only flake at idea creation, no skipped evidence | rerun after UI harness stabilization | hold if execution-surface screenshots still fail before capture |
| Live configuration readiness | clarity of live-mode availability and recovery path | visible config state, named missing prerequisite, deterministic enable/disable behavior | no hidden shell dependency, no silent blocked state | after config UX is added | hold if live mode is still selectable without actionable setup information |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: No successful app-launched Goose-backed `proposal_loop_live` run was evidenced in this review round.
- `GAP-02`: No current screenshots exist for the live Start Run sheet, live progress surface, artifact inspector, or a live non-happy path.
- `GAP-03`: The current UI harness still fails before execution-surface capture because created ideas are not observed in the list after save.

### Open Questions
- `QUESTION-01`: Is the intended first-pass live configuration surface environment-only, or should the app expose a setup/status surface?
- `QUESTION-02`: Is repo-path workflow resolution acceptable for the first live slice, or should the live workflow be bundled now?

## Evidence Gap Review Fallback
- What was attempted:
  - re-read Proposal 004 and its dependency docs
  - inspected the updated bootstrap, execution-service, run-progress, event-bridge, workflow-selection, and test-harness code
  - ran a clean macOS build
  - ran the focused Proposal 004 unit-test slice
  - ran focused macOS UI tests for the approval inbox and execution-adjacent surfaces
  - ran the required UI, UX, and architecture specialist reviews against the refreshed evidence pack
- What is missing:
  - a successful app-launched `proposal_loop_live` run using real Goose-backed execution
  - screenshot coverage for the live entry path, live progress, artifact inspector, and at least one non-happy path
  - stable execution-surface UI automation that reaches Start Run and run-progress capture
- Blockers:
  - review runtime lacks `CHAINWORKS_GOOSE_BASE_URL`
  - live workflow launch is still unproven in the app
  - execution-surface UI tests fail before capturing the key states
  - screenshot evidence remains incomplete for the full-review gate
- Confidence:
  - `High`
- What can still be said with partial confidence:
  - Proposal 004 is materially closer to readiness than in the previous round
  - the prior code-level blockers around executor selection, workflow selection, live timeline wiring, and live workflow test skips are now closed
  - the remaining risk is mostly concentrated in operability, packaging, and end-to-end runtime proof
- What evidence is required to finish the full review:
  - one successful Goose-backed `proposal_loop_live` run launched from the app
  - fresh screenshot coverage for entry, main path, approval or blocked state, and at least one non-happy path
  - a stable automated or manual proof that the Start Run and run-progress live surfaces are reachable in the current runtime
