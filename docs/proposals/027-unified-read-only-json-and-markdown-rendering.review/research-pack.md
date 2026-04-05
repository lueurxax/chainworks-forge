# Proposal Research Pack

## 0. Review Target and Local Context Consumed
- Proposal:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md`
- Research round:
  - `R1`
- Proposal evidence pack used:
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/027-unified-read-only-json-and-markdown-rendering-evidence-pack.md`
- Current-system baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Proposal-specific integration context used:
  - no separate `integration-context.md`
  - targeted code map from the evidence pack and current code reads
- Existing research pack reused:
  - none
- Adjacent docs consumed:
  - `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/domain-model.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/project-workspace-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
- Current code / module mapping consumed:
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Artifact.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/ArtifactManager.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunReportBuilder.swift`
- Local evidence IDs that triggered research:
  - `REAL-01`
  - `REAL-02`
  - `INT-02`
  - `INT-03`
  - `MAP-01`
  - `MAP-02`
  - `MAP-03`
  - `MAP-04`
  - `DATA-03`
- Notes on baseline freshness or local contradictions:
  - the earlier repo-local review surfaced two research-worthy questions: whether native Apple rendering primitives are strong enough for the shared JSON/Markdown viewer, and whether a fail-closed Markdown image policy is the right direction for the current local artifact/workspace boundary
  - current `P027` already incorporates the locally preferred direction in `§6.1`, `§6.2`, `§6.3`, and `§8`; this research round checks whether primary external guidance supports or contradicts that tightened text

## 1. Research Questions Derived from Local Evidence
| Question ID | Derived From (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Research Question | Why Local Evidence Is Not Enough | Priority |
|---|---|---|---|---|---|
| RQ-01 | Unresolved tradeoff | `REAL-01`, `MAP-01`, `MAP-02`, `MAP-03`, `MAP-04` | Do Apple-native hierarchy primitives support a collapsible JSON document viewer well enough for operator surfaces, or is a heavier browser-style tree still justified? | Local code shows the current gap, but not the current platform-fit recommendation. | High |
| RQ-02 | Unresolved tradeoff | `REAL-01`, `INT-02`, `MAP-01`, `MAP-02` | Do Apple-native Markdown parsing and semantic-text APIs provide enough structure for document-style rendering of headings, lists, code blocks, and tables without a web view? | Local code shows only partial current usage of `AttributedString(markdown:)`; external platform guidance is needed to judge whether the proposal's native direction is realistic. | High |
| RQ-03 | Host-system integration risk | `REAL-02`, `INT-03`, `DATA-03` | What do primary Markdown and Apple text-system sources imply about safe handling of Markdown images and raw HTML for a read-only local artifact viewer? | Local evidence establishes the current local-boundary model, but external standards/platform semantics are needed to justify a v1 fail-closed source policy. | High |

## 2. Source Ledger
| Source ID | Title | Publisher / Authority | URL or Reference | Published Date | Last Updated Date | Accessed / Verified Date | Why This Source Matters | Temporal Volatility / Freshness Risk | Confidence |
|---|---|---|---|---|---|---|---|---|---|
| `SRC-01` | `List.init(_:children:rowContent:)` | Apple Developer Documentation | `https://developer.apple.com/documentation/swiftui/list/init(_:children:rowcontent:)` | Not stated | Not stated | 2026-04-05 | Apple documents hierarchical `List` as producing an `OutlineGroup` and computing rows on demand. | Low | High |
| `SRC-02` | `Stacks, Grids, and Outlines in SwiftUI` | Apple Developer / WWDC20 | `https://developer.apple.com/videos/play/wwdc2020/10031/` | 2020 | Not stated | 2026-04-05 | WWDC guidance explains `DisclosureGroup` / outline progressive disclosure and notes that SwiftUI evaluates opened disclosure content lazily. | Low | High |
| `SRC-03` | `init(markdown:options:baseURL:)` | Apple Developer Documentation | `https://developer.apple.com/documentation/foundation/nsattributedstring/init(markdown:options:baseurl:)` | Not stated | Not stated | 2026-04-05 | Confirms Apple ships a native Markdown parsing entry point with a `baseURL` parameter. | Medium | Medium |
| `SRC-04` | `NSAttributedString` | Apple Developer Documentation | `https://developer.apple.com/documentation/foundation/nsattributedstring` | Not stated | Not stated | 2026-04-05 | Apple states Markdown-created attributed strings receive presentation-intent attributes and default styling in system views. | Medium | High |
| `SRC-05` | `NSPresentationIntent` | Apple Developer Documentation | `https://developer.apple.com/documentation/foundation/nspresentationintent` | Not stated | Not stated | 2026-04-05 | Apple defines semantic block roles such as paragraphs, headers, lists, code blocks, block quotes, and parts of tables. | Low | High |
| `SRC-06` | `CommonMark Spec 0.31.2` | CommonMark | `https://spec.commonmark.org/0.31.2/` | 2024-01-28 | 2024-01-28 | 2026-04-05 | CommonMark defines image syntax, autolinks, and raw HTML behavior, which directly affects source-policy and safety decisions. | Medium | High |

## 3. Findings by Theme

### Apple / iOS Platform Conventions
- Finding ID:
  - `F-APL-01`
  - Research question IDs:
    - `RQ-01`
  - Source IDs:
    - `SRC-01`
    - `SRC-02`
  - Source-backed finding:
    - Apple documents hierarchical `List` as creating an `OutlineGroup` and computing rows on demand. WWDC20 further states that SwiftUI evaluates `DisclosureGroup` content only after it is opened, so progressive disclosure avoids doing all nested work eagerly.
  - Model inference / host-system note:
    - This strongly supports `P027`'s native JSON-tree direction. A recursive `JSONTreeDocumentView` built from `OutlineGroup` or `DisclosureGroup` is platform-fit for read-only structured inspection, and it directly matches the proposal's collapsed-by-default large-branch behavior.
  - Host-system surface touched:
    - `ArtifactInspectorView`
    - `WorkflowArtifactInspectorView`
    - `RunReportView`
    - `RunComparisonView`
  - Time-sensitive:
    - `Low`
  - Confidence:
    - `High`

- Finding ID:
  - `F-APL-02`
  - Research question IDs:
    - `RQ-02`
  - Source IDs:
    - `SRC-03`
    - `SRC-04`
    - `SRC-05`
  - Source-backed finding:
    - Apple ships native Markdown parsing for attributed strings. Apple also documents that Markdown-generated attributed strings carry presentation-intent attributes, and `NSPresentationIntent` covers semantic structures including paragraphs, headers, lists, code blocks, block quotes, and parts of tables.
  - Model inference / host-system note:
    - This supports a native Markdown renderer for v1 rather than a web-view baseline. The proposal should keep acceptance focused on semantic document rendering and legibility, while allowing app-owned theming and selective custom treatment for tables or code blocks where default system styling is not enough.
  - Host-system surface touched:
    - `WorkflowArtifactInspectorView`
    - `ArtifactInspectorView`
    - `RunReportView`
    - `RunComparisonView`
  - Time-sensitive:
    - `Medium`
  - Confidence:
    - `High`

### Security / Privacy / PII Handling
- Finding ID:
  - `F-SEC-01`
  - Research question IDs:
    - `RQ-03`
  - Source IDs:
    - `SRC-03`
    - `SRC-06`
  - Source-backed finding:
    - CommonMark treats images as URL-bearing inline constructs, and its raw-HTML section says HTML-looking tags are rendered without escaping in HTML output, including custom tags. Apple's native Markdown initializer exposes a `baseURL`, which means reference resolution is part of the API surface and not something the app should leave implicit.
  - Model inference / host-system note:
    - For `Chainworks Forge`, where current artifact/report surfaces are local file and workspace readers rather than network document browsers, the safe v1 rule is the one now written in `P027`: allow local artifact/workspace-relative sources only, never fetch remote URLs, and render unsupported sources as badges/placeholders/text.
  - Host-system surface touched:
    - all read-only artifact/report/comparison surfaces
  - Time-sensitive:
    - `Medium`
  - Confidence:
    - `High`

### Architecture / State / Concurrency / Offline / Sync Patterns
- Finding ID:
  - `F-ARCH-01`
  - Research question IDs:
    - `RQ-01`
    - `RQ-02`
    - `RQ-03`
  - Source IDs:
    - `SRC-01`
    - `SRC-02`
    - `SRC-03`
    - `SRC-04`
    - `SRC-05`
    - `SRC-06`
  - Source-backed finding:
    - None of the primary sources suggest that screen-local content sniffing is a required part of native rendering. Instead, they describe typed hierarchy views and typed semantic text models.
  - Model inference / host-system note:
    - This strengthens the repo-local conclusion that `P027` should stay anchored to existing canonical format truth on `Artifact.format` / `ArtifactFormat.detect(...)` and use explicit typed render requests only for non-artifact content. External research does not introduce a better second format authority.
  - Host-system surface touched:
    - renderer boundary in `§6.1`
    - artifact-backed viewers
    - non-artifact render requests such as resolved skill content
  - Time-sensitive:
    - `Low`
  - Confidence:
    - `High`

## 4. Host-System Applicability Matrix
| Insight ID | Source IDs | Classification (`Adopt | Adapt | Watch | Reject`) | Proposal Area Affected | Host-System Surface Touched | Why It Applies or Does Not Apply | Concrete Recommended Change |
|---|---|---|---|---|---|---|
| `APP-01` | `SRC-01`, `SRC-02` | `Adopt` | `§6.3`, `§7.2`, `§8` | JSON rendering across artifact/report/comparison surfaces | Apple-native hierarchy and disclosure APIs are a direct fit for read-only JSON inspection and support progressive disclosure without a browser widget. | Keep the native JSON tree direction and make `OutlineGroup` / recursive `DisclosureGroup` the baseline implementation shape for `JSONTreeDocumentView`. |
| `APP-02` | `SRC-03`, `SRC-04`, `SRC-05` | `Adapt` | `§6.2`, `§7.1`, `§11` | Markdown rendering across artifact/report/comparison surfaces | Apple-native Markdown semantics are strong enough for v1, but visual fidelity still depends on app-owned styling and explicit handling of complex blocks like tables/code. | Keep the native direction, but phrase acceptance in terms of semantic document rendering and legibility rather than tying v1 to one exact rendering library. |
| `APP-03` | `SRC-03`, `SRC-06` | `Adopt` | `§6.2`, `§8`, `§11` | Markdown image handling and safety boundary | CommonMark image/raw-HTML semantics and Apple's `baseURL` entry point both imply that source resolution is a policy choice, not a harmless default. Current repo reality is local-only. | Keep the current fail-closed local-only policy: local artifact-root and workspace-relative sources only, remote disabled, unsupported sources rendered as safe placeholders or text. |
| `APP-04` | `SRC-01`, `SRC-03`, `SRC-04`, `SRC-05` | `Adopt` | `§6.1`, `§6.5`, `§10` | Renderer contract and migration shape | External guidance is typed and structured; it does not suggest per-screen re-detection. That fits the repo's existing `Artifact.format` authority. | Preserve current `§6.1` wording that artifact-backed surfaces must pass canonical `ArtifactFormat` instead of sniffed format guesses. |

## 5. Proposal Deltas / Recommended Updates
| Delta ID | Proposal Section / Decision | Recommended Update | Why It Helps | Supporting Source IDs | Supporting Local Evidence IDs | Priority |
|---|---|---|---|---|---|---|
| `DELTA-01` | `§6.3` implementation note | Add one sentence that the preferred implementation shape for `JSONTreeDocumentView` is native hierarchical `List` / `OutlineGroup` or recursive `DisclosureGroup`, chosen specifically for progressive disclosure and on-demand expansion. | Makes the native recommendation more concrete and directly reusable for implementation/audit. | `SRC-01`, `SRC-02` | `MAP-01`, `MAP-02`, `MAP-03`, `MAP-04`, `REAL-01` | `P2` |
| `DELTA-02` | `§6.2` implementation note | Add one sentence that native Markdown parsing should preserve semantic structure via Apple presentation-intent APIs even when app-owned styling customizes the final appearance. | Connects the product contract to current primary-platform semantics and clarifies why native is acceptable without a browser stack. | `SRC-03`, `SRC-04`, `SRC-05` | `MAP-01`, `MAP-02`, `REAL-01` | `P2` |
| `DELTA-03` | `§8` and acceptance criteria | Keep the existing fail-closed local-only image policy and consider explicitly naming raw HTML as non-rendered or text-fallback-only in v1. | CommonMark raw HTML semantics are broader than the app's current trust model; making the fallback explicit reduces implementation ambiguity. | `SRC-06`, `SRC-03` | `REAL-02`, `INT-03`, `DATA-03` | `P2` |

## 6. Freshness Risks / Recheck Triggers
| Trigger ID | Claim / Recommendation | Why It Is Time-Sensitive | What Must Be Rechecked | Recheck Trigger / Window | Source IDs |
|---|---|---|---|---|---|
| `FRESH-01` | Native Apple Markdown APIs are sufficient for v1 semantic rendering | Apple text/rendering behavior can evolve, especially around Markdown presentation and tables | Recheck Apple docs if the implementation moves to a newer OS baseline or adopts a different text/rendering stack | on OS-baseline change or before implementation audit | `SRC-03`, `SRC-04`, `SRC-05` |
| `FRESH-02` | CommonMark-based fail-closed image/raw-HTML policy remains the correct safety boundary | Markdown specs and chosen parser/library behavior may drift | Recheck spec/library behavior if the implementation introduces a third-party Markdown engine or HTML-backed rendering path | when parser/renderer choice changes | `SRC-06` |

## 7. Remaining Open Questions
- `QUESTION-01`: Is `P027` comfortable making raw HTML an explicit text-fallback case in v1, or does it want that deferred to implementation notes only?
- `QUESTION-02`: Does the team want a small proof-owner note tying at least one artifact-inspector UI test and one report-surface test to the unified renderer migration, or is that better left to implementation audit?
