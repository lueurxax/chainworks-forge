# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/011-run-control-working-directory-and-provider-binding-truth.md` | 2026-03-26 | High | The current draft now defines truthful provenance fallback via `.unverifiable`, explicit `requiresProjectAccess`, explicit cancellation-settlement logging, and typed runtime additions. | The review could overstate remaining proposal gaps. | Primary document under review. |
| DOC-02 | `docs/reference/runtime-contract.md` | 2026-03-26 | High | Current baseline already guarantees run-scoped workspaces and frozen run snapshots. | The review could misread how much of Proposal 011 is net-new. | Important for working-directory and run-control composition. |
| DOC-03 | `docs/reference/provider-platform.md` | 2026-03-26 | High | Provider baseline already prefers fail-closed or explicit truth over silent fallback. | The review could be too permissive about ambiguous provider/model history. | Important for binding-truth review. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None used | 2026-03-26 | High | Local repo and local docs were sufficient for this review round. | Low. | Keeps the review grounded in current HEAD. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' build` | `My Mac` / macOS | 2026-03-26 | Passed | No launch | warnings only | High | Fresh repo-health proof for this round. |
| RUN-02 | Targeted `Chainworks ForgeTests` slice: `ProviderPlatformTests`, `ResumeManagerTests`, `OrchestratorTests/testMalformedReviewJSONFailsBeforeTransitionEvaluation` | `My Mac` / macOS | 2026-03-26 | Passed | Passed | none; result bundle at [`/tmp/p011-r3-unit.xcresult`](/tmp/p011-r3-unit.xcresult) | High | Fresh code-level proof for provider truth, resume/cancellation seams, and orchestrator validation nearest to Proposal 011. |
| RUN-03 | Targeted `Chainworks ForgeUITests` slice: `testProviderSettingsTabReachable`, `testPilotReadinessRefreshSurface`, `testStartRunSheetUI`, `testRunProgressViewSurface`, `testApprovalGateViewSurface` | `My Mac` / macOS | 2026-03-26 | Passed | Passed | none; result bundle at [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult) | High | Fresh runtime/UI proof for the closest current owner shell touched by Proposal 011. |

## D. Xcode / UI Visual Evidence
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Provider settings root | Passed | 2026-03-26 | High | `testProviderSettingsTabReachable()` passed in the current-round focused UI slice. | Low. | Confirms the owner shell for provider-bound start configuration is reachable. |
| SCR-02 | [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Pilot readiness refresh | Passed | 2026-03-26 | High | `testPilotReadinessRefreshSurface()` passed in the current-round focused UI slice. | Low. | Confirms the settings / preflight shell remains reachable. |
| SCR-03 | [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | `Ideas -> Start Run` | Passed | 2026-03-26 | High | `testStartRunSheetUI()` passed in the current-round focused UI slice. | Low. | Closes the previous owner-path reachability blocker. |
| SCR-04 | [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | `Ideas -> Run Progress` | Passed | 2026-03-26 | High | `testRunProgressViewSurface()` passed in the current-round focused UI slice. | Low. | Confirms the primary nearby run-control shell is now reachable. |
| SCR-05 | [`/tmp/p011-r3-ui.xcresult`](/tmp/p011-r3-ui.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Approval owner path | Passed | 2026-03-26 | High | `testApprovalGateViewSurface()` passed in the current-round focused UI slice. | Low. | Confirms the adjacent approval shell remains stable. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | Workflow execution contract baseline | [WorkflowDefinition.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/DSL/WorkflowDefinition.swift#L41), [RunPlan.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunPlan.swift#L4) | DSL + Engine | 2026-03-26 | High | Current DSL/runtime still exposes `workflow.execution` and `RunPlan` as the right shared seam for Proposal 011's `requiresProjectAccess` selector. | The review could misread the proposal as targeting a non-existent seam. | Confirms the draft composes with real runtime boundaries. |
| CODE-02 | Frozen binding truth and historical surfaces | [BackendProfileResolverV2.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Providers/BackendProfileResolverV2.swift#L38), [RunReportBuilder.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunReportBuilder.swift#L145), [RunComparisonService.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunComparisonService.swift#L114) | Providers + Engine | 2026-03-26 | High | Current runtime still centers on resolved binding plus explicit overrides; the new Proposal 011 frozen provenance seam is still largely a draft contract, not a landed runtime field. | The review could confuse clean draft design with existing runtime truth. | Explains why the report stays partial despite no live draft findings. |
| CODE-03 | Search for new 011-specific persistence / runtime hits | repo search for `bindingProvenanceJSON`, `BindingProvenanceSource`, `cancellationSettlementLog`, `requiresProjectAccess`, `workspaceRootPath` | Repo-wide | 2026-03-26 | High | Current repo inspection still does not show the full Proposal 011 typed runtime slice landed on `HEAD`. | The report could overclaim implementation-adjacent evidence as implementation closure. | Anchors the remaining evidence gap. |
| CODE-04 | Current operator owner path | [IdeaListView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift#L311), [IdeaListView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift#L1175) | UI | 2026-03-26 | High | The closest current owner path for Proposal 011 still runs through `Ideas` and `Start New Run`, matching the focused UI rerun scope. | Wrong owner-path targeting would weaken the UI evidence. | Confirms the rerun targeted the right nearby shell. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in UI | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | `DOC-01`, `DOC-02`, `DOC-03` | 2026-03-26 | Proposal 011 now composes cleanly with the current runtime and provider references. | No | High | The old live proposal finding is closed. | The review could stay artificially blocked on already-fixed text issues. | Justifies the shift to no-live-findings. |
| BASE-02 | `RUN-01`, `RUN-02`, `RUN-03` | 2026-03-26 | Current repo health is green enough for focused proposal-adjacent proof. | Yes | High | Build, targeted unit, and targeted UI proof all passed in the current round. | The review could overstate instability that no longer exists. | Confirms the old owner-path UI blocker is closed. |
| BASE-03 | `CODE-02`, `CODE-03` | 2026-03-26 | The remaining limitation is runtime completeness, not proposal correctness. | No | High | Proposal 011-specific typed seams are still not fully implemented on current `HEAD`. | The review could overclaim full flow closure from adjacent-shell proof alone. | Explains why evidence remains partial. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | None collected | 2026-03-26 | High | This round was a proposal triad review, not a KPI/product overlay. | Low. | Explains why no product metrics are included. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: the fresh green owner-path UI slice is authoritative for the nearest current shell touched by Proposal 011, even though the proposal's full dedicated runtime slice is not landed yet.
- ASSUMP-02: the repo search for 011-specific fields is sufficient to classify the remaining limitation as implementation evidence, not proposal-text quality.
- OPEN-01: none blocking the current draft reread.
- BLOCKER-01: current `HEAD` still lacks the full landed Proposal 011 runtime slice, so the review cannot move beyond partial implementation evidence.
