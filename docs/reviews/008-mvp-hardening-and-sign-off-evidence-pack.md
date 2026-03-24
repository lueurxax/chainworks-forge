# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/008-mvp-hardening-and-sign-off.md` | 2026-03-24 | High | Proposal 008 freezes MVP boundary to `codex`, `claude_code`, `gemini`, adds benchmark/sign-off loop, and proposes new operator closure surfaces plus export/status contracts. | Review could mis-state intended scope or acceptance criteria. | Primary proposal under review. |
| DOC-02 | `docs/ps/chainworks-forge-mvp.md` | 2026-03-24 | High | PS now also fixes MVP providers to Codex, Claude Code, and Gemini, but still leaves the active output/report SLO as `[TBD]`. | Boundary drift or stale PS assumptions would distort product findings. | Checks whether Proposal 008 really closes the remaining MVP contracts. |
| DOC-03 | `docs/reference/runtime-contract.md` | 2026-03-24 | High | Runtime contract now matches the 3-provider MVP boundary and preserves `waiting_approval`/resume semantics. | Proposal could be reviewed against an outdated runtime contract. | Confirms provider freeze alignment and resume-policy baseline. |
| DOC-04 | `docs/reviews/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding-review.md` | 2026-03-24 | High | Current Proposal 007 review is still `Evidence Gap Review`; repo-backed runtime/services and dogfood proof remain future-state on current HEAD. | Proposal 008 could assume a ready dependency that the repo does not yet have. | Critical dependency evidence for sequencing/sign-off feasibility. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None used | 2026-03-24 | High | Local repo evidence was sufficient; no external/platform research was required for this review round. | Low. | Keeps the review grounded in current repo reality. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `Chainworks Forge` build | macOS | 2026-03-24 | Passed via `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/codex-dd-p008-build build` | N/A | Build is green, but Swift 6 isolation warnings remain across DSL/runtime/tests. Log: `/tmp/codex-p008-build.log`. | High | Confirms the proposal is being reviewed against a buildable current head. |
| RUN-02 | `Chainworks ForgeUITests/testApprovalInboxReachable` | My Mac / macOS 26.3.1 | 2026-03-24 | Passed | Passed in 5.445s. xcresult: `/tmp/codex-p008-approval.xcresult`. | Runtime warnings recorded in xcresult, but test body completed and saved screenshot attachment. | High | Fresh UI proof for operator approval surface. |
| RUN-03 | `Chainworks ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance` | My Mac / macOS 26.3.1 | 2026-03-24 | Passed | Passed in 18.394s. xcresult: `/tmp/codex-p008-missing-runtime.xcresult`. | Runtime warnings recorded in xcresult, but non-happy-path UI proof completed and saved screenshot attachment. | High | Fresh non-happy-path proof for runtime-unavailable guidance. |
| RUN-04 | `Chainworks ForgeUITests/testWaitingApprovalRunIsRestoredOnLaunch` | My Mac / macOS 26.3.1 | 2026-03-24 | Passed | Passed in 6.300s. xcresult: `/tmp/codex-p008-waiting-approval.xcresult`. | Runtime warnings recorded in xcresult, but relaunch-at-approval proof completed and saved screenshot attachment. | High | Fresh proof for the approval-resume contract that Proposal 008 formalizes. |
| RUN-05 | Focused multi-test UI rerun attempts | My Mac / macOS 26.3.1 | 2026-03-24 | Passed compile stage | Initial focused reruns under `/tmp/codex-dd-p008-ui2` stalled after launching the UI test runner and were manually terminated before usable result-bundle finalization. | UI automation can still be flaky when stale runners are present. | Medium | Lowers confidence slightly in broad UI-suite stability even though targeted proofs now pass. |

## D. Xcode Screenshot Log
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | `/tmp/codex-p008-approval.xcresult` attachment `REQ011_Approvals` (`REQ011_Approvals_0_88274689-5364-465C-AB9B-B47556100F1F.png`) | `Chainworks ForgeUITests/testApprovalInboxReachable` | My Mac / macOS 26.3.1 | Open Approvals tab | Approval inbox rendered | 2026-03-24 | High | Fresh screenshot evidence exists for the current approval surface. | If attachment lookup were wrong, the UI proof would degrade to pass/fail only. | Covers current operator primary approval surface. |
| SCR-02 | `/tmp/codex-p008-missing-runtime.xcresult` attachment `P004_NonHappy_MissingRuntime` (`P004_NonHappy_MissingRuntime_0_2470B7DF-BC1F-481D-A7EE-E5C355AB120E.png`) | `Chainworks ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance` | My Mac / macOS 26.3.1 | Open Start Run without live runtime | Missing-runtime recovery guidance visible | 2026-03-24 | High | Fresh non-happy-path screenshot evidence exists for runtime-unavailable recovery guidance. | Same as above. | Covers an important explicit recovery/error state. |
| SCR-03 | `/tmp/codex-p008-waiting-approval.xcresult` attachment `P004_Resume_ApprovalInbox` (`P004_Resume_ApprovalInbox_0_078D0AC7-7EF5-4FBF-B8F6-FE12927A171A.png`) | `Chainworks ForgeUITests/testWaitingApprovalRunIsRestoredOnLaunch` | My Mac / macOS 26.3.1 | Relaunch into waiting approval | Pending approval restored on launch | 2026-03-24 | High | Fresh screenshot evidence exists for the `waiting_approval` relaunch path. | Same as above. | Directly tests the relaunch behavior Proposal 008 tries to freeze. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | Idea attachment storage/UI display | `Chainworks Forge/Models/Idea.swift`, `Chainworks Forge/Views/IdeaListView.swift` | Models + UI | 2026-03-24 | High | Current app stores only a single `attachmentPath` string and displays it in the idea UI; there is no richer attachment ingestion model. | Proposal could over-promise attachment behavior that runtime does not actually support. | Grounds the attachment-policy finding. |
| CODE-02 | Execution context construction and Goose packet assembly | `Chainworks Forge/Engine/WorkflowOrchestrator.swift`, `Chainworks Forge/Engine/GooseSessionBridge.swift` | Runtime | 2026-03-24 | High | Agent execution currently forwards `inputArtifacts` and `ideaBody`; `attachmentPath` is not injected into the execution context or Goose context attachments. | Proposal could claim support for file types that are only stored as references, not actually passed to agents. | Grounds the attachment-support finding. |
| CODE-03 | Current operator shell surfaces | `Chainworks Forge/Views/RunsHomeView.swift`, `Chainworks Forge/Views/RecoverySheet.swift`, `Chainworks Forge/Views/RunReportView.swift`, `Chainworks Forge/Views/RunComparisonView.swift`, `Chainworks Forge/Views/ForegroundBannerView.swift`, `Chainworks Forge/ContentView.swift` | UI shell | 2026-03-24 | High | The current operator shell already centers on `RunsHomeView`, with `RecoverySheet`, `RunReportView`, `RunComparisonView`, and a `ForegroundBannerView` overlay. | Proposal could fragment the shell if new surfaces are added in parallel instead of integrated into these entry points. | Grounds the UI/UX integration finding. |
| CODE-04 | Current `Run` model | `Chainworks Forge/Models/Run.swift` | Models | 2026-03-24 | High | `Run` already carries trust/report fields, but not Proposal 008 benchmark timing fields or evidence-pack status fields yet. | Review could miss the actual model delta required for sign-off instrumentation. | Shows the concrete persistence delta Proposal 008 would need. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in Simulator | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | Current app shell + UI tests | 2026-03-24 | Current operator landing surface is `Runs Home`, with approvals and recovery routed through the shell rather than isolated standalone windows. | Yes | High | Proposal 008 is layering on top of an already-changed operator shell, not a blank surface. | New views may duplicate instead of extend current navigation. | Important for UI/UX scope correctness. |
| BASE-02 | Current relaunch proof (`RUN-04`, `SCR-03`) | 2026-03-24 | `waiting_approval` restoration is already behaviorally present in the current UI test baseline. | Yes | High | Proposal 008 is formalizing an existing path, not inventing it from scratch. | The proposal can under-specify integration points if it assumes a blank slate. | Important for scoping the approval-resume work. |
| BASE-03 | Current 007 review (`DOC-04`) + code search baseline | 2026-03-24 | Repo-backed 007 runtime and release surfaces are still not implemented on current HEAD. | No | High | Proposal 008 currently depends on a layer that is not yet sign-off-ready. | Sequencing findings could be understated. | Important for roadmap/dependency realism. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | `docs/ps/chainworks-forge-mvp.md` | 2026-03-24 | High | MVP success is still defined as `50%` reduction in manual orchestration time per idea, measured against a fixed idea cohort and checkpoint timings. | Proposal sign-off logic could be reviewed against the wrong KPI. | Grounds product/benchmark findings. |
| DATA-02 | `docs/reviews/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding-review.md` | 2026-03-24 | High | Current repo evidence still says Proposal 007 is future-state in code and missing repo-backed proof. | Proposal 008 could collapse hardening with unfinished core delivery runtime. | Grounds dependency and launch-gate findings. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: Fresh screenshot evidence is represented by attachment names embedded in the `.xcresult` bundles rather than separately exported `.png` files.
- ASSUMP-02: For this review, the relevant "primary flow" for Proposal 008 is the current approval/relaunch operator path, because the proposal is about MVP sign-off, recovery, and shell closure rather than net-new feature execution.
- QUESTION-01: How is manual-baseline evidence ingested into `BenchmarkRunRecorder` / `MVPSignOffEvaluator` so that the `GO/HOLD` decision is reproducible rather than notebook-driven?
- QUESTION-02: Are non-text attachment types in Proposal 008 intended to be truly agent-ingested, or only stored/displayed as local-path references?
- QUESTION-03: Do `BlockedRunRecoverySurface` and `CompletedRunExportHub` replace current shell entry points, or extend `RunsHomeView` / `RecoverySheet` / `RunReportView`?
- BLOCKER-01: Proposal 007 is still `Evidence Gap Review` on current HEAD, so Proposal 008 currently depends on a repo-backed runtime slice that is not yet demonstrated.
- BLOCKER-02: Broad UI reruns were initially flaky while stale UI test runners were present, so current UI confidence is strongest on targeted single-test proofs rather than wide-suite execution.
