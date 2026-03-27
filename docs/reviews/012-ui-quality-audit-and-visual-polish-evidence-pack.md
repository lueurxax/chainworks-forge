# Evidence Pack

## A. Local / Repo Inputs
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/012-ui-quality-audit-and-visual-polish.md` | 2026-03-26 | High | Proposal 012 currently claims a systematic audit of `12` previewable surfaces and `30` SwiftUI view files, depends only on Proposals 007 and 008, and keeps `L-11` open as a view-level defect. | The review could miss stale or mis-scoped proposal claims. | Primary document under review. |
| DOC-02 | Current view files cited by the proposal | 2026-03-26 | High | The issue catalogue materially touches `RunsHomeView`, `IdeaListView`, `ProviderSettingsView`, `PilotReadinessView`, `FirstRunSetupWizard`, `ArchivedIdeasView`, `GooseProviderConnectionAssistantView`, `ReleaseGateView`, `WorkflowMapView`, and `DeliveryPreflightReportView`. | The review could misjudge proposal scope or ownership. | Establishes what the proposal actually audits. |

## B. External Sources
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| WEB-01 | None used | 2026-03-26 | High | Local repo inspection and local build/test evidence were sufficient for this review round. | Low. | Keeps the review grounded in current `HEAD`. |

## C. Build and Run Log
| Evidence ID | Scheme / Target | Device / OS | Verified On | Build Result | Run Result | Blockers | Confidence | Relevance |
|---|---|---|---|---|---|---|---|---|
| RUN-01 | `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p012-build-dd -resultBundlePath /tmp/p012-build.xcresult build` | `My Mac` / macOS | 2026-03-26 | Passed | No launch | warnings only | High | Fresh repo-health proof for this round. |
| RUN-02 | Targeted `Chainworks ForgeUITests` slice for provider settings, wizard, Goose assistant, pilot readiness, archive, workflow map, start-run sheet, and run-progress surfaces | `My Mac` / macOS | 2026-03-26 | Passed | Failed before test execution | runner init failed with `Timed out while enabling automation mode`; result bundle at [`/tmp/p012-ui.xcresult`](/tmp/p012-ui.xcresult) | High | Explains why current-round UI screenshots / attachments are missing. |

## D. Xcode / UI Visual Evidence
| Evidence ID | Source / Path / Artifact | Scheme / Target | Device / OS | Flow Step | State | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|---|---|
| SCR-01 | [`/tmp/p012-ui.xcresult`](/tmp/p012-ui.xcresult) | `Chainworks ForgeUITests` | `My Mac` / macOS | Targeted UI rerun | Blocked | 2026-03-26 | High | The runner failed before automation mode enabled, so no authoritative current-round UI screenshots were produced. | Medium. | Keeps the review honest about evidence completeness. |

## E. Code / Architecture Evidence
| Evidence ID | Source / Path / Artifact | File Path / Module | Layer | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| CODE-01 | Preview / file inventory repo search | repo search for `#Preview` and `Views/*.swift` | Repo-wide | 2026-03-26 | High | Current `HEAD` has `14` named `#Preview("...")` definitions, `15` total `#Preview` blocks, and `28` Swift files under `Views/`. | The review could wrongly accept the proposal's completeness claims. | Supports the stale-inventory finding. |
| CODE-02 | Proposal dependency table versus audited surfaces | [012-ui-quality-audit-and-visual-polish.md](/Users/user/Documents/Chainworks Forge/docs/proposals/012-ui-quality-audit-and-visual-polish.md#L8), [ProviderSettingsView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ProviderSettingsView.swift), [PilotReadinessView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/PilotReadinessView.swift), [FirstRunSetupWizard.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/FirstRunSetupWizard.swift), [ArchivedIdeasView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArchivedIdeasView.swift), [GooseProviderConnectionAssistantView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift), [WorkflowMapView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/WorkflowMapView.swift) | Proposal + UI | 2026-03-26 | High | The catalogue materially audits provider/settings and operator-clarity surfaces that are not implied by dependency on 007/008 alone. | The review could miss a sequencing problem in the proposal. | Supports the dependency-baseline finding. |
| CODE-03 | Delivery preflight presentation ownership | [IdeaListView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift#L997), [PilotReadinessView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/PilotReadinessView.swift#L248), [FirstRunSetupWizard.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/FirstRunSetupWizard.swift#L224), [DeliveryPreflightReportView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/DeliveryPreflightReportView.swift) | UI | 2026-03-26 | High | Minimum presentation frames for preflight reports are already applied at the sheet presentation sites, so `L-11` is stale or wrongly owned as a defect in `DeliveryPreflightReportView.swift`. | The review could preserve a closed issue in the open backlog. | Supports the `L-11` finding. |
| CODE-04 | Current live issues still visible in code | [RunsHomeView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunsHomeView.swift), [IdeaListView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift), [GooseProviderConnectionAssistantView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift), [ReleaseGateView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ReleaseGateView.swift), [WorkflowMapView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/WorkflowMapView.swift) | UI | 2026-03-26 | Medium | Several other catalogue items still look plausible on code inspection, which is why the review narrows findings to the strongest stale / mis-scoped issues rather than invalidating the whole proposal. | Medium. | Prevents overclaiming that the entire catalogue is wrong. |

## F. Current-State Baseline
| Evidence ID | Source / Path / Artifact | Verified On | Observed State | Verified in UI | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| BASE-01 | `RUN-01` | 2026-03-26 | Current repo build health is green. | No | High | Proposal 012 is not blocked by repo compile failure in this round. | Low. | Separates proposal issues from build health. |
| BASE-02 | `RUN-02`, `SCR-01` | 2026-03-26 | Current-round visual proof is incomplete because the UI runner failed before automation mode enabled. | No | High | The review must stay partial for visual evidence. | Low. | Justifies `Evidence Gap Review`. |
| BASE-03 | `CODE-01`, `CODE-02`, `CODE-03` | 2026-03-26 | The proposal's own audit baseline is stale and at least one catalogued issue is already closed. | No | High | Proposal 012 cannot yet be trusted as a complete, current-state audit artifact. | Medium. | Explains the `Red` readiness. |

## G. Product / Data / Ops Evidence (Optional)
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DATA-01 | None collected | 2026-03-26 | High | This round was a proposal triad review, not a product KPI review. | Low. | Explains why no metrics overlay is included. |

## H. Assumptions, Open Questions, and Blockers
- ASSUMP-01: current repo search for previews and `Views/*.swift` is sufficient to classify the proposal's completeness claims as stale.
- ASSUMP-02: the lack of current-round UI screenshots is caused by the macOS UI harness, not by absence of the audited shells themselves.
- OPEN-01: how many other catalogue items besides `L-11` are already closed on current `HEAD` once the audit inventory is rebaselined?
- BLOCKER-01: the targeted macOS UI runner failed before automation mode initialized, so current-round visual evidence is incomplete.
