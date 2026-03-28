# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `docs/proposals/012-ui-quality-audit-and-visual-polish.md` | 2026-03-28 | High | Proposal 012 now rebaselines against current `HEAD`, aligns with stable reference docs, assigns Section 3 state ownership, and closes the earlier audited-surface / verification / shared-primitive gaps. | The review could report stale blockers that no longer exist. | Primary document under review. |
| DOC-02 | pre-refresh review artifact at `docs/reviews/012-ui-quality-audit-and-visual-polish-review.md` | 2026-03-28 | High | The immediately previous review revision at the same path carried the Amber verdict driven by incomplete Appendix A coverage, missing explicit keyboard/interaction verification, and an inconsistent `StatusCapsule` contract. | The new round could fail to verify whether those exact blockers were fixed. | Review-history input only. |
| DOC-03 | pre-refresh evidence pack at `docs/reviews/012-ui-quality-audit-and-visual-polish-evidence-pack.md` | 2026-03-28 | High | The immediately previous evidence-pack revision at the same path made the formerly open gaps explicit enough to verify closure. | Old blockers could stay sticky without an explicit before/after check. | Prior-artifact input only. |
| DOC-04 | `docs/reference/idea-lifecycle.md` | 2026-03-28 | High | Stable reference still defines current archive/restore truth for the `Ideas` flow. | Scope and UX assumptions for archive surfaces could drift. | Supports current operator-surface baseline. |
| DOC-05 | `docs/reference/live-workflow-map.md` | 2026-03-28 | High | Stable reference still defines current workflow-map topology baseline and proof expectations. | Workflow-map polish or fallback semantics could be mis-scoped. | Supports current workflow-map authority. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Missing | repo-level host-system baseline | 2026-03-28 | High | No reusable repo baseline artifact exists in the current repo. | The review relied on direct code mapping instead. |
| BASE-02 | `<proposal>.review/integration-context.md` | Missing | proposal-local current-system slice | 2026-03-28 | High | No proposal-local integration-context artifact exists for Proposal 012. | Narrow current-surface mapping stayed in this round's evidence pack. |
| BASE-03 | prior 012 review/evidence artifacts | Reused | proposal-local history and prior blockers | 2026-03-28 | Medium | Useful for verifying closure of the previous findings; not authoritative over current source. | Prevents re-reporting already-closed blockers. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - proposal readiness for the current macOS UI polish slice
  - audited-surface alignment against current source
  - verification-plan completeness for the proposal's live deliverables
  - shared primitive contract consistency
- Out of scope:
  - build/run/simulator validation
  - product KPI overlay
  - implementation/runtime readiness proof
  - engine/business-logic changes
- Deferred intentionally:
  - external research
  - reusable baseline creation, since code mapping was sufficient for this round
- Assumptions:
  - proposal readiness can be judged from proposal/docs/code evidence without preview execution
  - current SwiftUI source is authoritative for deciding whether Appendix A and the issue catalogue still match `HEAD`
- Open questions:
  - none blocking in the current proposal draft
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ContentView` tab shell / foreground banner | Targeted refresh | 2026-03-28 | High | Tab shell still has seven tabs, bottom banner overlay, and preview-backed shell proof. | Shell-level issues could drift out of the audited list. | Supports shell-level polish scope. |
| NAV-02 | `RunsHomeView` + `RunDetailPanel` | Targeted refresh | 2026-03-28 | High | Proposal now treats the detail panel as its own proof-owning subordinate surface. | Run-detail interaction work could otherwise slip back under the parent surface. | Confirms the prior Appendix A gap is closed. |
| NAV-03 | `IdeaListView` + `NewIdeaSheetView` | Targeted refresh | 2026-03-28 | High | `NewIdeaSheetView` remains in scope via `L-07`, has dedicated previews, and is now explicitly listed in Appendix A. | The proposal could regress to top-level-only inventory in later edits. | Confirms the prior Appendix A gap is closed. |
| NAV-04 | `ProviderSettingsView`, `PilotReadinessView`, `FirstRunSetupWizard` | Targeted refresh | 2026-03-28 | High | Provider/settings/readiness/onboarding surfaces remain preview-backed and in scope for hierarchy and async-feedback work. | Could misjudge Phase 2 scope if read selectively. | Supports `H-01`, `H-02`, `H-03`, `L-12`. |
| NAV-05 | `GooseProviderConnectionAssistantView` | Targeted refresh | 2026-03-28 | High | Goose assistant remains a standalone preview-backed guided surface. | Journey/async gaps could be under-scoped. | Supports `L-04`, `L-12`. |
| NAV-06 | `WorkflowMapView`, `ReleaseGateView`, `DeliveryPreflightReportView` | Targeted refresh | 2026-03-28 | High | Workflow/release/preflight surfaces remain distinct proof surfaces with status semantics in scope. | Verification or shared primitive adoption could miss negative states. | Supports `L-05`, `L-06`, `M-01`–`M-03`. |
| NAV-07 | `ApprovalGateView`, `RecoverySheet` | Targeted refresh | 2026-03-28 | High | Interaction-heavy approval and recovery surfaces are now explicitly listed in Appendix A with interaction-checklist proof ownership. | Could lose explicit proof obligations in future edits. | Confirms the prior Appendix A gap is closed. |
| NAV-08 | `ArchivedIdeasView`, `RunStartOverridesView` | Targeted refresh | 2026-03-28 | High | Both surfaces now appear in Appendix A with concrete preview assets and scoped expectations. | Secondary audited surfaces could be forgotten during implementation. | Confirms the audited-surface inventory is now complete enough for handoff. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `Chainworks Forge/ContentView.swift` and `Views/ForegroundBannerView.swift` | macOS SwiftUI shell | Tab shell, foreground banner, top-level preview owner | 2026-03-28 | High | Current shell still has seven tabs and still overlays the banner at `.bottom` while `ForegroundBannerView` transitions from `.top`. | The proposal could over- or under-scope shell polish. | Supports `M-04`, `L-08`, Appendix A shell row. |
| MAP-02 | `Chainworks Forge/Views/RunsHomeView.swift` | macOS SwiftUI shell | Runs split view, row density, detail actions, recovery/report sheets | 2026-03-28 | High | `RunsHomeView` still lacks explicit split-view width and still keeps `RunDetailPanel` actions at the bottom of a scrollable detail pane. | Proposal scope could miss a live detail-surface problem. | Supports `C-01`, `H-04`, `L-10`. |
| MAP-03 | `Chainworks Forge/Views/IdeaListView.swift` and `NewIdeaSheetView` | macOS SwiftUI shell | Ideas list, summary strip, new-idea entry sheet | 2026-03-28 | High | `summaryStrip` remains dense, `NewIdeaSheetView` is still a raw `VStack`, and the file already contains the preview assets and existing shortcuts that Proposal 012 now references correctly. | Proposal wording could drift from current source. | Supports `L-03`, `L-07`, `L-09`. |
| MAP-04 | `Chainworks Forge/Views/ProviderSettingsView.swift` | macOS SwiftUI settings | Provider diagnostics, transfer, advanced configuration, Goose assistant handoff | 2026-03-28 | High | Primary configuration, transfer, and inline add-provider form still share a single `List` surface. | Hierarchy/feedback work remains live. | Supports `H-01`, `L-12`. |
| MAP-05 | `Chainworks Forge/Views/PilotReadinessView.swift` | macOS SwiftUI readiness | Readiness report, provider status, pilot actions | 2026-03-28 | High | Current view is still a long diagnostics/readiness list without a top summary banner or grouped provider cards. | Information-density work remains live. | Supports `H-02`, `L-12`. |
| MAP-06 | `Chainworks Forge/Views/FirstRunSetupWizard.swift` | macOS SwiftUI onboarding | First-run setup, provider suggestion, verification, sample-run launch | 2026-03-28 | High | Wizard remains a long `Form` with no step indicator and no shortcut bindings today. | Phase 2 interaction scope could be under-specified if Section 6 regressed. | Supports `H-03`, `L-09`, `L-12`. |
| MAP-07 | `Chainworks Forge/Views/GooseProviderConnectionAssistantView.swift` | macOS SwiftUI onboarding / remediation | Guided Goose verification and troubleshooting | 2026-03-28 | High | Journey state is still plain `LabeledContent`, and probing still disables buttons without a visible spinner/stepper. | Async-feedback and guided-journey gaps remain live. | Supports `L-04`, `L-12`. |
| MAP-08 | `Chainworks Forge/Views/WorkflowMapView.swift` | macOS SwiftUI workflow visualization | Topology cards and status badge semantics | 2026-03-28 | High | Stage cards remain static and `WorkflowMapStatusBadge` still uses one of the fragmented badge implementations. | Interactivity/design-system work remains live. | Supports `L-05`, `M-01`–`M-03`. |
| MAP-09 | `Chainworks Forge/Views/ReleaseGateView.swift` | macOS SwiftUI release review | Release-artifact review semantics and operator decision surface | 2026-03-28 | High | Review-summary rows still show every absent artifact as `Missing` in orange, and the view still has no keyboard shortcuts today. | UX semantics and Phase 2 shortcut scope remain live. | Supports `L-06`, `L-09`. |
| MAP-10 | `Chainworks Forge/Views/DeliveryPreflightReportView.swift` | macOS SwiftUI delivery | Preflight report presentation and shared status styling | 2026-03-28 | High | Production minimum sizing is handled at presentation sites; the open concern is preview ergonomics and shared badge/token alignment. | Could keep a closed production issue open. | Supports reclassified `L-11` and Phase 3 primitives. |
| MAP-11 | `Chainworks Forge/Views/ApprovalGateView.swift` and `RecoverySheet.swift` | macOS SwiftUI approval / recovery | Interaction-heavy confirmation and recovery flows named by `L-09` | 2026-03-28 | High | Both surfaces are real operator flows, neither declares keyboard shortcuts today, and Proposal 012 now scopes them as explicit interaction-checklist surfaces. | Could regress back to implicit proof ownership in later edits. | Confirms the prior proof-ownership gap is closed. |
| MAP-12 | `Chainworks Forge/Views/ArchivedIdeasView.swift` and `RunStartOverridesView.swift` | macOS SwiftUI secondary operator surfaces | Archive lane and run-start overrides list | 2026-03-28 | High | Both surfaces have concrete previews and are now included in Appendix A with bounded expectations. | Secondary surfaces could be lost during implementation handoff. | Confirms Appendix A completeness. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Provider diagnostics / readiness state | `ProviderSettingsView.swift`, `PilotReadinessView.swift`, `FirstRunSetupWizard.swift`, `GooseProviderConnectionAssistantView.swift` | Read-only presentation over existing upstream services | 2026-03-28 | High | Proposal 012 changes UI treatment and feedback semantics, not provider transport contracts. | Could overstate architecture churn. | Keeps scope at UI/UX layer. |
| DATA-02 | Idea metadata and attachment draft state | `IdeaListView.swift`, `NewIdeaSheetView` | Read/write UI over existing `Idea` model | 2026-03-28 | High | `NewIdeaSheetView` is a real, stateful surface with its own preview-backed proof opportunity and explicit state semantics in Section 3. | Could understate a touched subordinate surface. | Supports Appendix A alignment and state-contract coverage. |
| DATA-03 | Release/preflight artifact availability | `ReleaseGateView.swift`, `DeliveryPreflightReportView.swift` | Read-only presentation | 2026-03-28 | High | Open work is semantic presentation and interaction, not missing data plumbing. | Could misclassify UI work as engine work. | Supports `L-06` and interaction verification scope. |
| DATA-04 | Auth/degraded/offline semantics | Section 3 of proposal + provider/readiness views | Presentation of upstream state | 2026-03-28 | Medium | Proposal now assigns these states to surfaces without inventing new transport contracts. | Could still drift during implementation if Section 3 is ignored. | Shows previous state-gap blockers are closed. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Tab shell and foreground-attention banner | Current repo | 2026-03-28 | High | Banner is owned by `ContentView` overlay and still transitions from the wrong edge. | Shell-level polish still needs execution, but the proposal scopes it correctly. | Supports shell-scope validity. |
| INT-02 | Runs split-view plus detail/recovery/report subordinate flows | Current repo | 2026-03-28 | High | `RunsHomeView` owns both the sidebar issue and subordinate detail/sheet flows. | Could regress if implementation ignores the subordinate proof owners now listed in Appendix A. | Supports Appendix A completeness. |
| INT-03 | Ideas flow plus archive lane and new-idea sheet | Current repo + `idea-lifecycle.md` | 2026-03-28 | High | The Ideas flow already has separate archive truth and a separate new-idea sheet with dedicated previews. | Proposal now matches that ownership. | Confirms current repo alignment. |
| INT-04 | Approval/recovery/release interaction seam | Current repo | 2026-03-28 | High | Approval, release, recovery, and run-detail actions are separate operator surfaces with explicit buttons but incomplete shortcut coverage today. | Proposal now correctly treats this as incomplete coverage, not non-existence. | Confirms Section 6 alignment. |
| INT-05 | Workflow-map truth and fallback seam | Current repo + `live-workflow-map.md` | 2026-03-28 | High | Workflow map remains a child of run detail, not a new top-level destination. | Proposal keeps interactive-map work inside that ownership path. | Confirms current scope validity. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | `DOC-01`, `NAV-01`–`NAV-08` | all touched surfaces | Entry surfaces and proof owners are now clear enough for handoff. |
| Happy path | Specified | `DOC-01`, `MAP-01`–`MAP-12` | major audited surfaces | Happy-path readability/hierarchy work is clearly scoped. |
| Loading | Specified | `DOC-01`, `MAP-04`, `MAP-05`, `MAP-06`, `MAP-07` | provider/readiness/wizard/assistant | Section 3.2 defines async feedback per surface. |
| Empty | Specified | `DOC-01`, `MAP-02`, `MAP-03`, `MAP-11`, `MAP-12` | runs, ideas, approvals, archive | Empty-state work is now attached to explicit surfaces and proof owners. |
| Validation error | Specified | `DOC-01`, `MAP-03`, `MAP-04`, `MAP-05`, `MAP-06`, `MAP-07` | form-driven surfaces | Validation-state ownership remains explicit. |
| Backend error | Specified | `DOC-01`, `MAP-04`, `MAP-05`, `MAP-07`, `MAP-09` | provider/readiness/assistant/release | Surface semantics are explicitly described. |
| Offline / degraded | Specified | `DOC-01`, `MAP-04`, `MAP-05`, `MAP-07` | provider/readiness/assistant | Degraded-state ownership remains explicit. |
| Retry / recovery | Specified | `DOC-01`, `MAP-04`, `MAP-05`, `MAP-07`, `MAP-11` | diagnostics and recovery surfaces | Retry/recovery semantics and interaction proof owners are now both explicit. |
| Auth / permission expiry | Specified | `DOC-01`, `DATA-04` | provider/readiness/assistant | Proposal assigns display ownership without inventing engine work. |
| Rollback / cancellation | Specified | `DOC-01`, `MAP-02`, `MAP-03`, `MAP-06`, `MAP-09`, `MAP-11` | run detail, sheets, wizard, release | Cancellation/rollback boundaries remain explicit at the surface layer. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | bounded phased rollout | Proposal implementation phases | Phase 1–4 sequencing with Phase 3 guardrails | Hold on wider primitive rollout until first-adopter slice passes verification | 2026-03-28 | Medium | Not a feature-flag rollout, but the proposal does define phased containment for shared-primitives work. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | none specified | n/a | n/a | 2026-03-28 | Medium | Product overlay is omitted; no instrumentation requirement is needed for this proposal-readiness round. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Proposal verification plan | Appendix A surfaces plus interaction-checklist surfaces | previews, min-window test, state contract verification, consistency, keyboard/interaction verification, VoiceOver, later implementation-evidence handoff | none material at proposal-readiness level | 2026-03-28 | High | The verification plan now covers the previously missing interaction obligations explicitly. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | audited-surface boundary | Appendix A is the authoritative audited-surface list for this proposal revision | Appendix A now includes the previously missing subordinate surfaces: `RunDetailPanel`, `NewIdeaSheetView`, `ApprovalGateView`, `RecoverySheet`, plus concrete proof assets for `ArchivedIdeasView` and `RunStartOverridesView` | 2026-03-28 | High | The prior authoritative-boundary contradiction is closed. |
| REAL-02 | shared badge primitive | `M-01` defines a `StatusCapsule` contract and Appendix B provides the unified target row | the code snippet and Appendix B now agree on the canonical unified badge style: `.caption2.bold()` with `8/3` padding and `0.15` opacity | 2026-03-28 | High | The prior shared-primitive contradiction is closed. |
| REAL-03 | keyboard/interactions verification | Section 6 must prove the live interaction-heavy deliverables, not defer them implicitly | verification item `5` now explicitly names `ApprovalGateView`, `ReleaseGateView`, `FirstRunSetupWizard`, `RecoverySheet`, and `RunDetailPanel` | 2026-03-28 | High | The prior proof-ownership gap is closed. |
| REAL-04 | shortcut baseline truth | `L-09` says coverage is incomplete rather than nonexistent | current source still has existing shortcuts in `IdeaListView`, while the high-value approval/release/recovery/wizard surfaces still lack them today | 2026-03-28 | High | The proposal now matches current repo reality accurately. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | `DOC-01` | Proposal clearly states the operator-facing polish goal. |
| Scope boundaries | Specified | `DOC-01`, `REAL-01` | The audited-surface boundary now matches the active work set. |
| Reusable baseline coverage | Missing | `BASE-01`, `BASE-02` | No reusable baseline artifacts currently exist for this slice. |
| Screen / surface definition | Specified | `NAV-01`–`NAV-08`, `REAL-01` | Major and subordinate surfaces are now enumerated explicitly. |
| Navigation / entry points | Specified | `NAV-01`–`NAV-08` | Ownership paths and proof owners are clear enough for handoff. |
| State handling | Specified | `DOC-01`, `DATA-04`, state matrix above | Previous non-happy-path gaps remain closed. |
| Data / API contract | Deferred intentionally | `DATA-01`, `DATA-03`, `DATA-04` | Proposal stays at presentation semantics, not engine contracts. |
| Persistence / caching | Deferred intentionally | `DATA-02`, `DATA-04` | No new persistence or caching layer is implied. |
| Permissions / auth expiry | Specified | `DOC-01`, `DATA-04` | Surface-level ownership remains explicit. |
| Feature flags / rollout / rollback | Partial | `FLAG-01` | The phase plan bounds rollout of shared primitives, but not as a formal feature-flag strategy. |
| Analytics / instrumentation | Deferred intentionally | `METRIC-01` | Product overlay is omitted. |
| Testing strategy | Specified | `TEST-01`, `REAL-03` | Verification criteria now explicitly cover the previously missing interaction obligations. |
| Dependencies / integration points | Specified | `DOC-04`, `DOC-05`, `INT-01`–`INT-05` | Dependency framing is aligned with current stable references. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: direct code mapping is sufficient for this proposal-readiness round even without a reusable repo-level baseline artifact.
- ASSUMP-02: current preview declarations are enough to evaluate whether Appendix A matches the preview-backed proof boundary.
- QUESTION-01: none blocking in the current draft.
- BLOCKER-01: none.

## O. Research Triggers / External Questions

- RSH-01: `DOC-01`, `MAP-01`, `MAP-02`, `MAP-04`, `MAP-05`, `MAP-06`, `INT-01`, `INT-02`
  Local question:
  - Proposal 012 now scopes density and hierarchy fixes correctly, but local evidence does not by itself define how far the implementation should lean into current macOS guidance on fewer nested levels, reduced modality, surfaced commands, and always-visible high-value actions.
- RSH-02: `DOC-01`, `MAP-04`, `MAP-05`, `MAP-06`, `MAP-07`, `MAP-09`, `DATA-01`, `DATA-03`
  Local question:
  - Section 3.2 assigns inline loading/success/failure ownership, but local evidence alone does not establish the current Apple-preferred boundary between inline feedback, nonintrusive labels, and modal alerts for recoverable operator errors.
- RSH-03: `DOC-01`, `MAP-06`, `MAP-09`, `MAP-11`, `REAL-03`, `REAL-04`
  Local question:
  - Proposal 012 now treats shortcut coverage as incomplete rather than absent, but local evidence alone does not define the current Apple convention set for confirm/dismiss bindings, keyboard-only workflows, and avoiding system shortcut collisions.
- RSH-04: `DOC-01`, `MAP-08`, `MAP-09`, `MAP-10`, `MAP-11`, `REAL-02`
  Local question:
  - Proposal 012 aligns the `StatusCapsule` contract internally, but local evidence alone does not define the current accessibility expectations for non-color status differentiation, contrast, VoiceOver announcement of state, and focus visibility on custom status or workflow affordances.
