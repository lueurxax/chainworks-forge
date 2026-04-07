# Proposal Evidence Pack

## A. Repo-Local Proposal / Document Inventory
| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|
| DOC-01 | `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` | 2026-04-05 | High | Current `P027` now explicitly prefers an AppKit/TextKit-backed Markdown document surface, payload-mismatch JSON rescue, and source-order-friendly JSON inspection. | Research could target stale proposal text. | Primary proposal source. |
| DOC-02 | `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md` | 2026-04-05 | High | The repo baseline is a stable macOS operator tool with shell-owned artifact/report/comparison surfaces. | Proposal could be judged against the wrong host-system baseline. | Review entry point. |
| DOC-03 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | 2026-04-05 | High | Stable subsystem references remain the right acceleration layer for proposal review. | Local context could be rebuilt from scratch unnecessarily. | Baseline accelerator. |
| DOC-04 | `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md` | 2026-04-05 | High | Artifact inspection, reports, and comparison already belong to the existing shell-owned operator spine. | Research could drift into inventing a parallel viewer lane. | Ownership baseline. |
| DOC-05 | `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md` | 2026-04-05 | High | Artifact/report truth remains text-first on disk and presentation-only in the UI. | Research could blur rendering and persistence concerns. | Read-only boundary. |
| DOC-06 | `/Users/user/Documents/Chainworks Forge/docs/reference/project-workspace-contract.md` | 2026-04-05 | High | Artifact/workspace access is explicitly local and bounded; current surfaces are not open network document browsers. | Image/source-policy research could ignore the local trust boundary. | Safety boundary. |
| DOC-07 | `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.review/research-pack.md` | 2026-04-05 | High | Same-day `R1` research already validated the native JSON-tree direction and fail-closed local-only image policy. | A second research round could waste effort re-answering already-closed questions. | Reusable research basis. |
| DOC-08 | `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering_IMPLEMENTATION_AUDIT_R1.md` | 2026-04-05 | High | Fresh implementation audit isolated two live seams relevant to deeper research: document-grade Markdown surface choice and JSON ordering fidelity. | New research questions could miss the most current local tension. | Fresh local trigger. |

## B. Reusable Baseline Inputs
| Evidence ID | Artifact / Slice | Status (`Reused | Partially refreshed | Missing`) | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---|---|---|---|
| BASE-01 | `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md` | Reused | overall operator-shell ownership and repo review conventions | 2026-04-05 | High | Still fresh for this slice. | Baseline entry point. |
| BASE-02 | `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md` | Reused | stable subsystem map | 2026-04-05 | High | Still fresh. | Stable context. |
| BASE-03 | Proposal-local integration context | Partially refreshed | current renderer implementation and migration state | 2026-04-05 | High | No standalone `integration-context.md`; current code reads were needed because renderer reality changed on this tree. | Narrow refresh. |
| BASE-04 | Same-day `R1` research pack | Reused after freshness check | native JSON-tree direction and local-only image policy | 2026-04-05 | High | Still valid; no contradiction surfaced on those questions. | Research reuse. |

## C. Scope, Out-of-Scope, and Intentional Deferrals
- In scope:
  - one shared read-only renderer for Markdown and JSON
  - AppKit/TextKit-grade Markdown document display
  - payload-mismatch rescue into structured JSON
  - JSON tree inspection behavior and ordering contract
  - migration of the named artifact/report/comparison surfaces
- Out of scope:
  - editing
  - WYSIWYG authoring
  - arbitrary HTML execution
  - schema-aware JSON forms
- Deferred intentionally:
  - search
  - copy-by-path
  - outline navigation
- Assumptions:
  - the unified renderer remains subordinate to existing shell-owned surfaces
  - image/source trust stays inside current local artifact/workspace boundaries in v1
- Open questions:
  - does `P027` want human-friendly source-preserving JSON inspection, deterministic canonical ordering, or an explicit documented fallback hierarchy between them?
  - how explicit should the proposal be that `Text(AttributedString)` is below the final document-grade bar?
- Blockers:
  - none for research mode

## D. Affected Screens / Navigation / Entry-Point Slice
| Evidence ID | Screen / Surface / Entry Point | Source (`Baseline | Targeted refresh | Proposal`) | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|
| NAV-01 | `ArtifactInspectorView` | Baseline + targeted refresh | 2026-04-05 | High | The artifact inspector now consumes `ArtifactContentRenderer` with artifact-backed context. | Research could assume the old fragmented path is still live. | Primary migration target. |
| NAV-02 | `WorkflowArtifactInspectorView` inside `IdeaListView` | Targeted refresh | 2026-04-05 | High | Workflow artifact preview now also goes through the shared renderer. | Shared-renderer reach could be understated. | Primary migration target. |
| NAV-03 | `RunReportView` | Baseline + targeted refresh | 2026-04-05 | High | Latest summary and selected immutable report content now route through the shared renderer with artifact-backed or explicit context. | Research could misread the remaining markdown/report seam. | Primary migration target. |
| NAV-04 | `RunComparisonView` | Baseline + targeted refresh | 2026-04-05 | High | Resolved skill content now uses the shared renderer through an explicit Markdown render request. | Proposal scope could be mis-scoped. | Primary migration target. |
| NAV-05 | Artifact inspector UI proof lane | Targeted refresh | 2026-04-05 | Medium | Local UI proof attempt failed before automation mode initialized, so live screen-level document rendering remains unproven on this tree. | Research could overstate runtime confidence. | Relevant constraint, not a blocker. |

## E. Impacted Modules / Code-Path Map
| Evidence ID | File Path / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| MAP-01 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactContentRenderer.swift` | Shared UI | Current renderer owner | 2026-04-05 | High | `ArtifactContentRenderer` resolves presentation intent, routes Markdown to `MarkdownDocumentView`, routes JSON to `JSONTreeDocumentView`, and enforces local-only image resolution. | Research could target the wrong implementation seam. | Primary code seam. |
| MAP-02 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactContentRenderer.swift` `MarkdownTextBlockView` | Shared UI | Current Markdown document surface | 2026-04-05 | High | Markdown blocks still render through `Text(AttributedString(markdown: ...))`, not an AppKit/TextKit-backed document viewer. | Proposal/app mismatch could be missed. | Primary research trigger. |
| MAP-03 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactContentRenderer.swift` `JSONTreeNode.build` | Shared UI | Current JSON ordering behavior | 2026-04-05 | High | JSON object keys are sorted alphabetically with `dictionary.keys.sorted()`. | The proposal's source-order language could be judged without noticing the live implementation tradeoff. | Primary research trigger. |
| MAP-04 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Artifact.swift` | Persistence | Canonical format owner | 2026-04-05 | High | `Artifact.format` and `ArtifactFormat.detect(from:contract:)` remain the canonical format-truth lane. | Research could reopen a second format authority. | Architecture guardrail. |
| MAP-05 | `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunReportBuilder.swift` | Engine | Report artifact emission | 2026-04-05 | High | Report truth is still emitted as `.md` and `.json` text artifacts with explicit formats. | Research could blur renderer and persistence boundaries. | Persistence guardrail. |
| MAP-06 | `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal027Tests.swift` | Tests | Focused renderer proof | 2026-04-05 | High | Current focused tests prove canonical format ownership, payload-mismatch rescue, JSON summaries, and local-only image handling. | External research could overstate unproven local gaps. | Local proof basis. |
| MAP-07 | `/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh` | Delivery / readiness | Canonical gate coverage | 2026-04-05 | High | No `proposal-027` gate exists yet. | Research could assume a canonical proof lane already exists. | Readiness constraint. |

## F. Data / API / Persistence / Auth Touchpoints
| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---|---|---|---|---|
| DATA-01 | Canonical format truth | `Artifact.format`, `ArtifactFormat.detect(...)`, `ArtifactRenderContext.artifactBacked(...)` | Persisted metadata -> renderer | 2026-04-05 | High | Format authority is already centralized and durable. | Research might solve the wrong problem if it assumes screen-level detection. | Core architectural guardrail. |
| DATA-02 | Artifact text on disk | `runtime-contract.md`, `RunReportBuilder`, artifact files | Disk -> presentation-only UI | 2026-04-05 | High | Rendering does not change persisted artifact truth. | Research could conflate UI ordering with persisted semantic truth. | Persistence boundary. |
| DATA-03 | Local file/workspace source policy | `project-workspace-contract.md`, `MarkdownImageSourcePolicy` | Local roots -> safe markdown images | 2026-04-05 | High | Current source policy is explicitly local-only and bounded to artifact/workspace roots. | Research could drift into browser-like remote source assumptions. | Safety boundary. |

## G. Current Host-System Integration Surfaces
| Evidence ID | Surface / Seam / Owner | Source (`Baseline | Targeted refresh | Current repo`) | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---|---|---|---|---|
| INT-01 | Shell-owned artifact/report/comparison readers | Baseline + current repo | 2026-04-05 | High | The unified renderer extends existing viewers; it does not create a second reader lane. | Research should stay renderer-focused, not owner-rediscovery focused. | Ownership baseline. |
| INT-02 | Current Markdown display stack | Current repo | 2026-04-05 | High | The live implementation still uses SwiftUI `Text(AttributedString)` inside the shared renderer. | Proposal now sets a higher AppKit/TextKit bar than the current tree meets. | Core local tension. |
| INT-03 | Current JSON ordering stack | Current repo | 2026-04-05 | High | The live implementation sorts keys alphabetically after `JSONSerialization` parsing. | Proposal's source-order language now sits in tension with the current generic parsing approach. | Core local tension. |
| INT-04 | Local-only image boundary | Current repo + stable refs | 2026-04-05 | High | Current repo reality still matches the proposal's local-only fail-closed image policy. | Reopening remote image trust would contradict the current host-system boundary. | Closed local question. |

## H. State Coverage Matrix
| State | Proposal Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Specified | DOC-01, NAV-01..04 | shared renderer migration targets | Concrete and current. |
| Happy path | Specified | MAP-01..06, DATA-01..03 | shared renderer, canonical format, local roots | Renderer contract is explicit. |
| Empty / unsupported content | Specified | DOC-01, MAP-01 | safe fallback / unavailable content | Still explicit. |
| Malformed content | Specified | DOC-01, MAP-01, MAP-06 | JSON parse failure and markdown fallback | Safety path exists. |
| Offline / degraded | Specified | DOC-06, DATA-03, INT-04 | local-only image policy | Browser-like remote fetch remains out of scope. |
| Editing / mutation | Deferred intentionally | DOC-01 | N/A | Properly out of scope. |

## I. Current Repo Reality / Contradictions
| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---|---|---|
| REAL-01 | Markdown document surface | `P027` now prefers an AppKit/TextKit-backed read-only document renderer and explicitly says `Text(AttributedString)` is below the final quality bar | Current tree still renders Markdown through generic SwiftUI `Text(attributed)` inside the shared renderer | 2026-04-05 | High | This is a valid local trigger for deeper external research into the Apple text stack. |
| REAL-02 | JSON ordering fidelity | `P027` asks for stable key ordering based on source order where possible | Current tree sorts object keys alphabetically after generic Foundation parsing | 2026-04-05 | High | This is a valid local trigger for research into JSON ordering semantics and canonicalization. |
| REAL-03 | Image/source policy | `P027` keeps v1 image handling local-only and fail-closed | Current tree already enforces local-only roots and rejects remote URLs | 2026-04-05 | High | This question is closed locally and only needs reuse/freshness checking. |

## J. Proposal Completeness Matrix
| Dimension | Status (`Specified | Partial | Missing | Contradicted by repo | Deferred intentionally`) | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01, INT-01 | Clear and still grounded in repo reality. |
| Scope boundaries | Specified | DOC-01 | Still clear. |
| Screen / surface definition | Specified | NAV-01..04 | Primary migration targets remain explicit. |
| Navigation / ownership | Specified | DOC-04, INT-01 | Existing shell-owned viewers remain the owner path. |
| Data / persistence contract | Specified | MAP-04..05, DATA-01..02 | Renderer is presentation-only and format-authority-safe. |
| Safety boundary | Specified | DATA-03, INT-04, REAL-03 | Local-only image policy is explicit and aligned. |
| JSON ordering contract | Partial | MAP-03, INT-03, REAL-02 | Current proposal text is stronger than the generic parser path; external clarification is warranted. |
| Markdown display-surface contract | Partial | MAP-02, INT-02, REAL-01 | Current proposal text is stronger than the existing SwiftUI text path; external clarification is warranted. |
| Testing / proof shape | Partial | MAP-06..07 | Enough for research mode; no blocker. |

## K. Assumptions, Open Questions, and Blockers
- ASSUMP-01: The unified renderer remains subordinate to existing shell-owned artifact/report/comparison viewers.
- ASSUMP-02: Presentation-level JSON ordering may differ from semantic JSON object meaning; the proposal still needs to choose what it cares about most.
- QUESTION-01: Does `P027` want source-preserving human inspection, deterministic canonical ordering, or an explicit fallback sequence between them?
- QUESTION-02: How explicitly should the proposal name AppKit/TextKit as the required display surface versus allowing any renderer that meets the same document-grade bar?
- BLOCKERS:
  - none for research mode

## L. Research Triggers / External Questions
| Trigger ID | Trigger Type (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Question to Research | Why Local Evidence Is Not Enough | Time Sensitivity / Freshness Risk |
|---|---|---|---|---|---|
| RSH-01 | Unresolved tradeoff | `MAP-02`, `INT-02`, `REAL-01` | Do Apple-native text-system docs support `P027`'s stricter AppKit/TextKit-backed Markdown document surface over `Text(AttributedString)` for read-only technical docs? | Local evidence shows the seam but not the current primary-platform recommendation. | Medium |
| RSH-02 | Unresolved tradeoff | `MAP-02`, `REAL-01` | What do Apple-native text-table / semantic-text APIs imply about rendering Markdown tables as true tables rather than plaintext approximations? | Local evidence proves only a generic attributed-text path today. | Medium |
| RSH-03 | Host-system integration risk | `MAP-03`, `INT-03`, `REAL-02` | What do primary JSON standards imply about member ordering, and how should `P027` express source-order fidelity versus deterministic canonical ordering? | Local evidence shows a mismatch but not the standards-backed framing for a proposal fix. | Low |
| RSH-04 | Reuse / freshness check | `DATA-03`, `INT-04`, `REAL-03` | Does any fresh primary source contradict the existing local-only fail-closed Markdown image policy? | Same-day `R1` research already answered this; only freshness confirmation is needed. | Medium |
