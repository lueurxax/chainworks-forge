# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md` | 2026-03-26 | High | The current draft still requires one happy-path and one non-happy-path repo-backed dogfood run plus exported evidence before Proposal 007 can be considered fully signed off. | The review could overread narrower runtime proof as full proposal closure. | Primary proposal under review. |
| DOC-02 | `docs/reference/live-provider-execution-slice.md` | 2026-03-26 | High | The live provider baseline still owns the real provider-backed proposal-loop substrate consumed by Proposal 007. | The review could misattribute repo-backed runtime responsibilities. | Important adjacent baseline. |
| DOC-03 | `docs/reference/operator-experience.md` | 2026-03-26 | High | Recovery/report/comparison remain operator-shell ownership baselines that Proposal 007 explicitly extends rather than replaces. | The review could misread repo-backed additions as a new shell. | Important for UI/UX boundary checking. |
| DOC-04 | `docs/reference/provider-platform.md` | 2026-03-26 | High | Provider settings, diagnostics, and pilot-readiness surfaces remain baseline dependencies for the repo-backed delivery slice. | The review could review the wrong control-plane boundary. | Important for start/preflight handoff. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None used | 2026-03-26 | High | Local repo evidence was sufficient for this round. | Low. | Keeps the review grounded in current `HEAD`. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `Chainworks Forge` build via `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p007p008-build-dd -resultBundlePath /tmp/p007p008-build.xcresult build` | `My Mac` / macOS | 2026-03-26 | Passed | N/A | warnings only; result bundle at [`/tmp/p007p008-build.xcresult`](/tmp/p007p008-build.xcresult) | High | Fresh repo-health proof for this round. |
| RUN-02 | Targeted Proposal 007 slice via `xcodebuild ... -resultBundlePath /tmp/p007-review.xcresult test` with `FullMVPDeliveryTests`, `DeliveryServicesTests`, `WorktreeProvisionerTests`, `testApprovalGateViewSurface`, `testRunProgressViewSurface`, `testStartRunSheetUI`, `testFullProductCheckpointCanonicalExecution` | `My Mac` / macOS | 2026-03-26 | Passed | Passed; targeted UI portion executed `4` tests with `1` skip and `0` failures | `testFullProductCheckpointCanonicalExecution()` still skips in this headless environment; result bundle at [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult) | High | Fresh proposal-scoped runtime/UI proof. |

## D. Xcode / UI Visual Evidence
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Approval gate owner path | Passed | 2026-03-26 | High | `testApprovalGateViewSurface()` passed in the current-round targeted slice. | Low. | Confirms the approval owner path remains reachable. |
| SCR-02 | [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | `Ideas -> Start Run` | Passed | 2026-03-26 | High | `testStartRunSheetUI()` passed in the current-round targeted slice. | Low. | Confirms the repo-backed preset entry surface is reachable. |
| SCR-03 | [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | `Ideas -> Run Progress` | Passed | 2026-03-26 | High | `testRunProgressViewSurface()` passed in the current-round targeted slice. | Low. | Confirms the current worktree-aware progress shell is reachable. |
| SCR-04 | [`/tmp/p007-review.xcresult`](/tmp/p007-review.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Canonical product checkpoint | Skipped | 2026-03-26 | High | `testFullProductCheckpointCanonicalExecution()` skipped with the explicit headless macOS toolbar-discovery limitation. | The review cannot treat the skipped checkpoint as full end-to-end proof. | Keeps the evidence gap explicit. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | Repo-backed workflow preset and explicit gate topology | `examples/workflows/full-mvp-live.yaml`, `Chainworks Forge/Engine/RunPlan.swift` | Fixture + Engine | 2026-03-26 | High | Current `HEAD` now really contains the repo-backed `full-mvp-live.yaml` preset and explicit approval-policy preservation the older review said were missing. | The review could stay anchored to stale absence claims. | Confirms the earlier topology finding is closed in code. |
| CODE-02 | Delivery configuration and preflight boundary | `Chainworks Forge/Engine/DeliveryConfiguration.swift`, `Chainworks Forge/Engine/DeliveryPreflightService.swift`, `Chainworks Forge/Views/IdeaListView.swift` | Engine + UI | 2026-03-26 | High | Start Run now has a real delivery-configuration draft / freeze boundary and repo-backed preflight owner path. | The review could understate how much of the pre-run delivery contract now exists. | Confirms the config-boundary critique from older rounds is stale. |
| CODE-03 | Worktree/runtime/release integration slice | `Chainworks Forge/Engine/WorkflowOrchestrator.swift`, `Chainworks Forge/Engine/WorktreeProvisioner.swift`, `Chainworks Forge/Engine/RepoSafetyGuard.swift`, `Chainworks Forge/Engine/ReleaseOpsCoordinator.swift`, `Chainworks Forge/Engine/GitReleaseService.swift`, `Chainworks Forge/Engine/ConnectPublishService.swift`, `Chainworks Forge/Engine/DeliveryReceiptBuilder.swift` | Engine | 2026-03-26 | High | Current `HEAD` now contains the major Proposal 007 runtime/service types the previous review marked absent. | The review could keep overstating implementation absence after the repo moved on. | Grounds the updated “stale old review” conclusion. |
| CODE-04 | Repo-backed UI/export surfaces | `Chainworks Forge/Views/ReleaseGateView.swift`, `Chainworks Forge/Views/RunsHomeView.swift`, `Chainworks Forge/Engine/EvidencePackBuilder.swift` | UI + Engine | 2026-03-26 | High | The release-gate surface and evidence-pack export path exist on current `HEAD`, even though this round still lacks a real full dogfood evidence pack from an in-app session. | The review could confuse code presence with complete end-to-end proof. | Explains why the report is yellow rather than green. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in UI | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | `DOC-01`, `CODE-01`, `CODE-02`, `CODE-03`, `CODE-04` | 2026-03-26 | The old “Proposal 007 runtime/services are absent” review baseline is no longer true on current `HEAD`. | No | High | The repo has materially advanced since the earlier review. | Keeping the old absence-based report would now be misleading. | Justifies rewriting the stale review. |
| BASE-02 | `RUN-01`, `RUN-02`, `SCR-01`, `SCR-02`, `SCR-03` | 2026-03-26 | Current targeted Proposal 007 build/runtime/UI evidence is green except for one explicit environment skip. | Yes | High | The nearest owner-path proof for repo-backed preset entry, progress, and approval is now stable. | The review could understate current maturity. | Confirms the earlier UI blocker is closed. |
| BASE-03 | `DOC-01`, `RUN-02`, `SCR-04` | 2026-03-26 | Full dogfood sign-off evidence is still missing even though the targeted slices now pass. | No | High | Proposal 007’s own final evidence bar is still higher than the current round’s proof. | The review could overstate readiness if it equated targeted slices with full end-to-end delivery evidence. | Explains why evidence completeness remains partial. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | None collected | 2026-03-26 | High | This round was a triad review without explicit product-overlay scope. | Low. | Explains why KPI / rollout analysis is omitted. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: the current targeted Proposal 007 slice is sufficient to replace the stale older absence-based review, even though it does not yet satisfy the proposal’s own final happy/non-happy dogfood evidence bar.
- ASSUMP-02: the headless skip on the canonical checkpoint is environmental rather than a demonstrated product failure.
- OPEN-01: none blocking the current draft reread.
- BLOCKER-01: no current-round happy-path repo-backed dogfood evidence pack was produced from inside the app.
- BLOCKER-02: no current-round non-happy-path repo-backed recovery run was produced from inside the app.
- BLOCKER-03: the canonical product-checkpoint UI proof is still skipped in this environment.
