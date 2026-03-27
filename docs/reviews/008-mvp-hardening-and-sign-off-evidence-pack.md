# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/008-mvp-hardening-and-sign-off.md` | 2026-03-26 | High | The current draft explicitly blocks 008 until Proposal 007 is current-head green and keeps attachments reference-only, recovery shell-owned, and sign-off replayable from persisted benchmark records. | The review could overstate remaining proposal gaps. | Primary proposal under review. |
| DOC-02 | `docs/ps/chainworks-forge-mvp.md` | 2026-03-26 | High | The PS still defines MVP success around a 50% reduction in manual orchestration time per idea. | The review could assess the sign-off loop against the wrong KPI. | Grounds the benchmark/sign-off framing. |
| DOC-03 | `docs/reference/runtime-contract.md` | 2026-03-26 | High | Current runtime contract still preserves approval/recovery/report semantics that Proposal 008 extends rather than replaces. | The review could misread the existing shell/runtime baseline. | Important adjacent baseline. |
| DOC-04 | `docs/reviews/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding-review.md` | 2026-03-26 | High | Current Proposal 007 evidence is much stronger than the old future-state-only baseline, but it is still partial because full repo-backed dogfood proof is not closed. | Proposal 008 could be judged against either a stale too-red or overly optimistic 007 baseline. | Critical dependency evidence for sequencing. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None used | 2026-03-26 | High | Local repo and local docs were sufficient for this round. | Low. | Keeps the review grounded in current `HEAD`. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `Chainworks Forge` build via `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007p008-build-dd -resultBundlePath /tmp/p007p008-build.xcresult build` | `My Mac` / macOS | 2026-03-26 | Passed | N/A | warnings only; result bundle at [`/tmp/p007p008-build.xcresult`](/tmp/p007p008-build.xcresult) | High | Fresh repo-health proof for the round. |
| RUN-02 | Targeted Proposal 008 shell pass via `xcodebuild ... -resultBundlePath /tmp/p008-review.xcresult test` with `testApprovalInboxReachable`, `testProviderSettingsTabReachable`, `testPilotReadinessRefreshSurface`, `testStartRunSheetUI`, `testRunProgressViewSurface` | `My Mac` / macOS | 2026-03-26 | Passed | Passed: `5` tests, `0` failures | none; result bundle at [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult) | High | Fresh shell/runtime proof for the current operator baseline Proposal 008 builds upon. |

## D. Xcode / UI Visual Evidence
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Approval inbox | Passed | 2026-03-26 | High | `testApprovalInboxReachable()` passed in the current-round targeted slice. | Low. | Confirms the current approval shell remains reachable. |
| SCR-02 | [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Provider settings root | Passed | 2026-03-26 | High | `testProviderSettingsTabReachable()` passed in the current-round targeted slice. | Low. | Confirms the provider/settings baseline the proposal relies on. |
| SCR-03 | [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Pilot readiness refresh | Passed | 2026-03-26 | High | `testPilotReadinessRefreshSurface()` passed in the current-round targeted slice. | Low. | Confirms the diagnostics/preflight shell baseline. |
| SCR-04 | [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | `Ideas -> Start Run` | Passed | 2026-03-26 | High | `testStartRunSheetUI()` passed in the current-round targeted slice. | Low. | Confirms the current run-start shell is stable. |
| SCR-05 | [`/tmp/p008-review.xcresult`](/tmp/p008-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | `Ideas -> Run Progress` | Passed | 2026-03-26 | High | `testRunProgressViewSurface()` passed in the current-round targeted slice. | Low. | Confirms the current progress shell is stable. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | Current attachment/reference truth | `Chainworks Forge/Models/Idea.swift`, `Chainworks Forge/Views/IdeaListView.swift`, `Chainworks Forge/Engine/GooseSessionBridge.swift` | Models + UI + Engine | 2026-03-26 | High | Current runtime still stores a single `attachmentPath` on `Idea` and does not inject that path as a first-class agent input; Goose context attachments cover workspace, input artifacts, and idea body. | The review could misjudge whether the proposal’s reference-only attachment policy matches reality. | Grounds the now-clean attachment-policy baseline. |
| CODE-02 | Current shell ownership baseline | `Chainworks Forge/ContentView.swift`, `Chainworks Forge/Views/RunsHomeView.swift`, `Chainworks Forge/Views/RecoverySheet.swift`, `Chainworks Forge/Views/RunReportView.swift`, `Chainworks Forge/Views/RunComparisonView.swift`, `Chainworks Forge/Views/ForegroundBannerView.swift`, `Chainworks Forge/Views/ProviderSettingsView.swift`, `Chainworks Forge/Views/PilotReadinessView.swift` | UI shell | 2026-03-26 | High | Current `HEAD` already centers the operator flow on shell-owned run/recovery/report/settings surfaces, matching Proposal 008’s ownership model. | The review could keep stale shell-fragmentation findings alive after the draft corrected them. | Explains why earlier shell-ownership findings no longer surface. |
| CODE-03 | Absence proof for 008-specific benchmark/sign-off implementation | repo search for `BenchmarkCohort`, `BenchmarkExecutionRecord`, `BenchmarkPair`, `MVPSignOffDecisionSnapshot`, `MVPSignOffEvaluator`, `ManualBaselineImport`, `BenchmarkRunRecorder`, `SignOffEvidencePackBuilder`, `MVPBoundaryPolicy`, `BlockedRunRecoveryView`, `CompletedRunExportHub`, `MVPSignOffSummaryView`, `ApprovalResumeRouter`, `OutputRetrievalSLOProbe` | Repo-wide | 2026-03-26 | High | The search returned no hits on current `HEAD`, confirming that Proposal 008-specific runtime/entities/routes are not yet implemented. | The review could confuse adjacent-shell proof with 008-specific implementation closure. | Anchors the remaining evidence gap. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in UI | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | `RUN-01`, `RUN-02`, `SCR-01` through `SCR-05` | 2026-03-26 | Current operator/settings shell is green in the focused round. | Yes | High | Proposal 008 now sits on a much stronger current shell baseline than the older review claimed. | Keeping the old shell-health story would be misleading. | Justifies refreshing the review instead of reusing the stale one. |
| BASE-02 | `DOC-04` plus current 007 rereview | 2026-03-26 | Proposal 007 is no longer “feature absent,” but it is still not fully closed on repo-backed dogfood evidence. | No | High | Proposal 008’s prerequisite is still materially blocked even though the dependency baseline improved. | The review could either over-block or under-block 008 if it used the wrong 007 baseline. | Critical dependency context. |
| BASE-03 | `CODE-03` | 2026-03-26 | Proposal 008-specific benchmark/sign-off runtime is still absent on current `HEAD`. | No | High | The current review can only assess draft readiness and adjacent-shell fit, not implemented-flow quality. | The review could overstate implementation readiness. | Explains why evidence completeness remains partial. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | `docs/ps/chainworks-forge-mvp.md` | 2026-03-26 | High | MVP success is still defined around the 50% manual-orchestration reduction metric that Proposal 008 formalizes into a benchmark/sign-off loop. | Proposal 008 could be evaluated against the wrong target metric. | Grounds the sign-off framing. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: the focused current-round shell/UI slice is sufficient to validate adjacent runtime truth for Proposal 008 even though the proposal’s own benchmark/sign-off surfaces do not yet exist.
- ASSUMP-02: the absence search for 008-specific types is sufficient to classify the remaining limitation as implementation evidence rather than a lingering proposal-text defect.
- OPEN-01: none blocking the current draft reread.
- BLOCKER-01: Proposal 008 is intentionally blocked behind stronger Proposal 007 repo-backed proof than the current repo can yet show.
- BLOCKER-02: current `HEAD` still has no 008-specific benchmark/sign-off implementation slice.
