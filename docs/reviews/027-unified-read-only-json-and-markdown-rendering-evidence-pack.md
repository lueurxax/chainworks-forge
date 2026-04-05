# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` | 2026-04-05 | High | `P027` introduces one shared read-only renderer for Markdown and JSON across artifact/report/comparison surfaces. | The review could judge stale proposal text. | Primary proposal source. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md` | 2026-04-05 | High | Artifact inspection, run reports, and comparison are already shell-owned operator surfaces. | The proposal could be judged as inventing new viewer owners when it should extend existing ones. | Needed for ownership review. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md` | 2026-04-05 | High | The artifact inspector already renders both user-facing markdown and raw structured/provider-facing outputs in the live slice. | Current viewer expectations could be misread. | Grounds the approval/artifact viewer baseline. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/reference/domain-model.md` | 2026-04-05 | High | `Artifact` persists `format` as durable metadata with values `json`, `markdown`, `diff`, and `report`. | Renderer ownership could be assigned to the wrong layer. | Core format-truth evidence. |
| DOC-05 | `/Users/user/Documents/Chainworks Forge/docs/reference/project-workspace-contract.md` | 2026-04-05 | High | Project-backed execution and artifact access stay tied to explicit local workspace truth. | Image/source policy could ignore current local-boundary assumptions. | Grounds source-safety review. |
| DOC-06 | `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md` | 2026-04-05 | High | Artifacts remain textual on disk and the artifact store is separate from presentation. | Renderer could be mistaken for a new persistence layer. | Supports the read-only presentation boundary. |
| DOC-07 | `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.review/research-pack.md` | 2026-04-05 | High | Same-day Apple/CommonMark research supports native hierarchy rendering, native Markdown semantics, and fail-closed local-only image policy. | The reread could carry forward outdated local concerns after the proposal changed. | Reused research basis. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md` | Reused | operator shell, artifact inspection, current review assumptions | 2026-04-05 | High | Still fresh for shell ownership. | Review entry point. |
| BASE-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | Reused | stable subsystem map | 2026-04-05 | High | Still fresh for the reviewed surfaces. | Confirms stable ref set. |
| BASE-03 | Proposal-local integration context | Partially refreshed | rendering code, artifact format ownership, current viewer tests | 2026-04-05 | High | Fresh code reads were still needed because `P027` is a component proposal over existing stable surfaces. | Explains targeted code refresh. |
| BASE-04 | Same-day research pack | Reused after freshness check | Apple hierarchy primitives, Apple Markdown semantics, CommonMark image/raw-HTML semantics | 2026-04-05 | High | Still fresh for the unchanged affected surfaces. | Supports the reread. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - read-only Markdown and JSON rendering
  - one shared rendering entry point
  - migration of artifact/report/comparison surfaces
  - performance and safe fallbacks
- Out of scope:
  - editing
  - WYSIWYG
  - schema-aware forms
  - arbitrary HTML execution
- Deferred intentionally:
  - search
  - copy-by-path
  - outline navigation
- Assumptions:
  - the new renderer extends existing shell-owned surfaces rather than creating a new viewer lane
  - artifact content remains text-first on disk
- Open questions:
  - should `§6.3` name a preferred native hierarchy primitive explicitly?
  - should raw HTML fallback be made explicit now or left to implementation notes?
- Blockers:
  - none

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ArtifactInspectorView` | Baseline + targeted refresh | 2026-04-05 | High | The current artifact inspector is a shell-owned surface with format-aware rendering, provenance chips, and local file open actions. | The proposal could ignore current surface ownership or safety assumptions. | Primary migration target. |
| NAV-02 | `WorkflowArtifactInspectorView` inside `IdeaListView` | Targeted refresh | 2026-04-05 | High | Workflow artifact preview already does local Markdown rendering via `AttributedString(markdown:)`, but JSON/report remain monospaced text. | The proposal could overstate or mis-scope the current gap. | Primary migration target. |
| NAV-03 | `RunReportView` | Baseline + targeted refresh | 2026-04-05 | High | Latest summary and immutable history currently render loaded strings through raw `Text(...)` monospaced views. | Format-truth handling could stay too weak during migration. | Primary migration target. |
| NAV-04 | `RunComparisonView` | Baseline + targeted refresh | 2026-04-05 | High | Resolved skill content currently renders as raw monospaced disclosure text. | Shared renderer scope could be under-specified. | Primary migration target. |
| NAV-05 | Artifact inspector UI tests | Targeted refresh | 2026-04-05 | Medium | Current UI tests already treat artifact inspector reachability as a proof-owning surface. | Viewer proof expectations could remain implicit. | Useful for implementation handoff, not a blocker. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactInspectorView.swift` | Operator UI | Current artifact inspector renderer | 2026-04-05 | High | `formatAwareRenderer` renders Markdown as raw `Text(content)`, JSON as pretty-printed text, diff line-by-line, and report as monospaced text. | The current gap could be misstated. | Primary rendering seam. |
| MAP-02 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift` | Operator UI | Current workflow artifact preview | 2026-04-05 | High | `WorkflowArtifactInspectorView` uses `AttributedString(markdown:)` for Markdown but falls back to monospaced text for JSON/report. | Proposal may miss current partial implementation. | Primary rendering seam. |
| MAP-03 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift` | Operator UI | Current report summary/history viewer | 2026-04-05 | High | `summaryContent` and `selectedReportContent` are string-only state rendered via raw monospaced `Text(...)`. | Migration complexity can be understated. | Primary rendering seam. |
| MAP-04 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift` | Operator UI | Current resolved-skill content viewer | 2026-04-05 | High | Resolved skill content is still raw monospaced text inside a disclosure group. | Shared renderer scope can be understated. | Migration target. |
| MAP-05 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Artifact.swift` | Persistence | Canonical artifact format owner | 2026-04-05 | High | `Artifact.format` is durable metadata and `ArtifactFormat.detect(from:contract:)` is the current canonical detection rule. | Proposal could reopen a second format-detection lane if it regresses. | Core format-truth evidence. |
| MAP-06 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ArtifactManager.swift` | Engine | Artifact detection / read path | 2026-04-05 | High | `ArtifactManager` delegates format detection to `ArtifactFormat.detect(...)`. | Screen-local sniffing would contradict existing engine truth. | Core format-truth evidence. |
| MAP-07 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunReportBuilder.swift` | Engine | Report artifact emission | 2026-04-05 | High | Stable reports are emitted as `.md` Markdown and `.json` JSON artifacts, each with explicit `ArtifactFormat`. | Report-view migration should preserve artifact format truth. | Supports format-owner confirmation. |
| MAP-08 | `/Users/user/Documents/Chainworks Forge/Chainworks ForgeUITests/Chainworks_ForgeUITests.swift` | UI proof | Current artifact inspector proof owners | 2026-04-05 | Medium | UI tests already cover artifact inspector reachability and opening structured artifacts. | Viewer proof expectations could remain implicit. | Implementation handoff context. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Artifact format truth | `Artifact.format`, `ArtifactFormat.detect(...)`, `ArtifactManager` | Persisted metadata -> viewer | 2026-04-05 | High | Current repo already has a canonical format owner. | Shared renderer could reopen format drift if proposal text regresses. | Core guardrail. |
| DATA-02 | Artifact content on disk | `runtime-contract.md`, `RunReportBuilder`, artifact views | Disk -> read-only presentation | 2026-04-05 | High | Artifact truth is text-first on disk; rendering is presentation only. | Proposal could blur presentation and persistence. | Supports design boundary. |
| DATA-03 | Local artifact/workspace access | `project-workspace-contract.md`, artifact views using file paths and `ArtifactManager.readArtifact(...)` | Local file/workspace -> viewer | 2026-04-05 | High | Current viewer model is local and explicit, not an open network document browser. | Markdown image support could accidentally widen trust boundaries if proposal text regresses. | Core guardrail. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Artifact/report/comparison shell ownership | Baseline + current repo | 2026-04-05 | High | Existing viewer surfaces are already shell-owned. | The shared renderer should extend them, not create a new lane. | Ownership context. |
| INT-02 | Canonical format detection | Current repo | 2026-04-05 | High | Format truth already lives in persisted metadata plus centralized detection logic. | Any return to screen-level `detected format` wording would reopen a second authority. | Architecture guardrail. |
| INT-03 | Local file/workspace viewer boundary | Current repo + stable refs | 2026-04-05 | High | Current viewer surfaces read from artifact files and workspace roots; they do not define remote fetch policy. | Any return to open-ended remote image wording would widen trust boundaries. | UX guardrail. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01..04 | renderer migration entry points | Migration targets are concrete. |
| Happy path | Specified | DOC-01, MAP-01..07, INT-02, DOC-07 | renderer + artifact metadata | Shared renderer and artifact format authority are now clear. |
| Loading | Deferred intentionally | DOC-01 | N/A | No separate loading policy is central here. |
| Empty | Specified | DOC-01, MAP-01..04 | no content / unsupported content | Safe fallback behavior is explicit. |
| Validation error | Specified | DOC-01 | malformed markdown/json fallback | Safety fallback is explicitly in scope. |
| Backend error | Deferred intentionally | DOC-01 | N/A | Rendering proposal, not runtime backend slice. |
| Offline / degraded | Specified | DOC-01, DATA-03, INT-03, DOC-07 | local vs remote sources | Image/source safety is now explicitly local-only and fail-closed. |
| Retry / recovery | Deferred intentionally | DOC-01 | N/A | Not central to this slice. |
| Auth / permission expiry | Deferred intentionally | DOC-01 | N/A | Still out of scope, which is acceptable because the renderer does not open a new remote lane. |
| Rollback / cancellation | Deferred intentionally | DOC-01 | N/A | Not central here. |

## I. Feature Flags / Rollout / Rollback
| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---|---|---|
| FLAG-01 | Phased renderer migration | viewer surfaces | Partial | Partial | 2026-04-05 | Medium | Migration phases are clear enough for proposal-readiness; no blocker here. |

## J. Analytics / Instrumentation
| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|
| METRIC-01 | None explicit | N/A | N/A | 2026-04-05 | Medium | No instrumentation blocker for proposal-readiness; this is a presentation unification slice. |

## K. Testing Strategy
| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---|---|---|
| TEST-01 | Existing UI tests | artifact inspector reachability | Current artifact inspector UI tests exist | Partial | 2026-04-05 | Medium | A dedicated proof-owner note would help later audits, but it is not needed for proposal readiness. |

## L. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Shared renderer format input | Artifact-backed screens pass canonical `ArtifactFormat`, and only non-artifact content may use an explicit render request | Current repo already centralizes format truth on `Artifact.format` and `ArtifactFormat.detect(...)` | 2026-04-05 | High | Current proposal text now matches repo reality. |
| REAL-02 | Markdown image support | v1 image handling is fail-closed and local-only, with remote fetch out of scope | Current viewer surfaces are local artifact/workspace readers with no explicit remote fetch authority | 2026-04-05 | High | Current proposal text now matches repo reality. |
| REAL-03 | Fragmented rendering motivation | Rendering is inconsistent and often weak | Current repo still has materially divergent rendering paths across artifact/report/comparison surfaces | 2026-04-05 | High | Motivation remains grounded in current repo reality. |

## M. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, MAP-01..04, REAL-03 | Motivation is strong and local. |
| Scope boundaries | Specified | DOC-01 | In/out-of-scope lines are clear. |
| Reusable baseline coverage | Specified | DOC-02..07, BASE-01..04 | The right stable refs and same-day research were available and relevant. |
| Screen / surface definition | Specified | NAV-01..04 | Primary migration targets are named. |
| Navigation / entry points | Specified | NAV-01..04, INT-01 | Existing shell owners are identifiable. |
| State handling | Specified | H matrix, REAL-01..02, DOC-07 | Format authority and image/source policy are now explicit. |
| Data / API contract | Specified | MAP-05..07, DATA-01..03, REAL-01..02 | Renderer input authority is now explicit enough. |
| Persistence / caching | Specified | DOC-04, DOC-06, DATA-02 | Presentation-only boundary is explicit. |
| Permissions / auth expiry | Deferred intentionally | DOC-01 | Still out of scope, appropriately for a local-only renderer. |
| Feature flags / rollout / rollback | Partial | FLAG-01 | Acceptable for readiness; not a blocker. |
| Analytics / instrumentation | Deferred intentionally | METRIC-01 | Not central to readiness. |
| Testing strategy | Partial | TEST-01 | Enough for readiness; proof-owner hygiene remains optional. |
| Dependencies / integration points | Specified | MAP-01..08, INT-01..03, DOC-07 | Main seams are mapped and externally reinforced. |

## N. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The shared renderer extends existing shell-owned artifact/report/comparison viewers.
- ASSUMP-02: Artifact truth remains textual on disk and read-only in the UI.
- QUESTION-01: Should `§6.3` explicitly name `OutlineGroup` / recursive `DisclosureGroup` as the preferred implementation shape?
- QUESTION-02: Should raw HTML fallback be made explicit in proposal text now, or left to implementation notes and audit?
- BLOCKERS:
  - none

## O. Research Triggers / External Questions
| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Resolved in `R1` | `REAL-01`, `REAL-02`, `INT-02`, `INT-03` | Apple-native hierarchy / Markdown semantics and CommonMark image/raw-HTML semantics | Same-day primary-source research confirmed the native and fail-closed direction already reflected in the current proposal text. | Recheck only if renderer choice or OS baseline changes. |
