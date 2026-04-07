# Proposal Research Pack

## 0. Review Target and Local Context Consumed
- Proposal:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md`
- Research round:
  - `R2`
- Proposal evidence pack used:
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/027-unified-read-only-json-and-markdown-rendering-evidence-pack.md`
- Current-system baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Proposal-specific integration context used:
  - no standalone `integration-context.md`
  - targeted current-tree code map from the refreshed evidence pack
- Existing research pack reused:
  - same path, previous `R1` conclusions reused where still fresh
- Adjacent docs consumed:
  - `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/project-workspace-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering_IMPLEMENTATION_AUDIT_R1.md`
- Current code / module mapping consumed:
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunReportView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/RunComparisonView.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Models/Artifact.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks Forge/Engine/RunReportBuilder.swift`
  - `/Users/user/Documents/Chainworks Forge/Chainworks ForgeTests/Proposal027Tests.swift`
  - `/Users/user/Documents/Chainworks Forge/scripts/test-gate.sh`
- Local evidence IDs that triggered this round:
  - `MAP-02`
  - `MAP-03`
  - `INT-02`
  - `INT-03`
  - `REAL-01`
  - `REAL-02`
  - `REAL-03`
- Notes on baseline freshness or local contradictions:
  - the baseline and `R1` research remain fresh for shell ownership, native JSON-tree direction, and local-only image policy
  - current `P027` tightened its text contract and now explicitly prefers an AppKit/TextKit-backed Markdown document surface
  - fresh implementation audit evidence narrowed the unresolved questions to two seams: document-grade Markdown on macOS and JSON ordering semantics

## 1. Research Questions Derived from Local Evidence
| Question ID | Derived From (`Proposal gap | Baseline constraint | Host-system integration risk | Unresolved tradeoff`) | Local Evidence IDs | Research Question | Why Local Evidence Is Not Enough | Priority |
|---|---|---|---|---|---|
| RQ-01 | Unresolved tradeoff | `MAP-02`, `INT-02`, `REAL-01` | Do Apple-native text-system docs support `P027`'s stronger choice of an AppKit/TextKit-backed read-only Markdown document surface over a generic SwiftUI `Text(AttributedString)` path for technical document reading? | Local evidence shows the implementation gap, but not the primary-platform recommendation or the features Apple expects from a real document text surface. | High |
| RQ-02 | Unresolved tradeoff | `MAP-02`, `REAL-01` | What do Apple text-system sources imply about table-grade rendering for Markdown content, and do they support the proposal's bar that tables should render as tables rather than plaintext approximations? | Local evidence proves only that native Markdown can carry semantic hints; external guidance is needed on whether the Cocoa text system actually offers first-class table/layout support. | High |
| RQ-03 | Host-system integration risk | `MAP-03`, `INT-03`, `REAL-02` | What do primary JSON standards imply about object member ordering, and how should `P027` frame source-order fidelity versus deterministic canonical ordering? | Local evidence shows the mismatch, but not the standards-backed framing for a durable proposal contract. | High |
| RQ-04 | Reuse / freshness check | `REAL-03` | Does any fresh primary source contradict the current local-only fail-closed Markdown image policy? | `R1` already answered this on the same day; this round only needs a freshness check while broader research continues. | Medium |

## 2. Source Ledger
| Source ID | Title | Publisher / Authority | URL or Reference | Published Date | Last Updated Date | Accessed / Verified Date | Why This Source Matters | Temporal Volatility / Freshness Risk | Confidence |
|---|---|---|---|---|---|---|---|---|---|
| `SRC-01` | `Text System Organization` | Apple Developer Library (archive) | `https://developer.apple.com/library/archive/documentation/TextFonts/Conceptual/CocoaTextArchitecture/TextSystemArchitecture/ArchitectureOverview.html` | Not stated | Archived | 2026-04-05 | Describes the Cocoa text system architecture, `NSTextView`, `NSTextStorage`, `NSLayoutManager`, `NSTextContainer`, and document-viewer capabilities like selection, editability control, wrapping, and embedded graphics. | Low | High |
| `SRC-02` | `Using Text Tables` | Apple Developer Library (archive) | `https://developer.apple.com/library/archive/documentation/Cocoa/Conceptual/TextLayout/Articles/TextTables.html` | Not stated | Archived | 2026-04-05 | Shows that the Cocoa text system has first-class table support through `NSTextTable`, `NSTextTableBlock`, and built-in `NSTextView` support. | Low | High |
| `SRC-03` | `init(markdown:options:baseURL:)` | Apple Developer Documentation | `https://developer.apple.com/documentation/foundation/nsattributedstring/init(markdown:options:baseurl:)` | Not stated | Not stated | 2026-04-05 (reused from `R1`) | Confirms Apple ships a native Markdown parsing entry point with explicit base-URL handling. | Medium | Medium |
| `SRC-04` | `NSAttributedString` | Apple Developer Documentation | `https://developer.apple.com/documentation/foundation/nsattributedstring` | Not stated | Not stated | 2026-04-05 (reused from `R1`) | Apple states Markdown-created attributed strings carry presentation-intent attributes and default styling in system views. | Medium | High |
| `SRC-05` | `NSPresentationIntent` | Apple Developer Documentation | `https://developer.apple.com/documentation/foundation/nspresentationintent` | Not stated | Not stated | 2026-04-05 (reused from `R1`) | Apple defines semantic block roles including headers, lists, code blocks, block quotes, and table-related structures. | Low | High |
| `SRC-06` | `RFC 8259: The JavaScript Object Notation (JSON) Data Interchange Format` | IETF / RFC Editor | `https://www.rfc-editor.org/rfc/rfc8259.txt` | 2017-12 | 2017-12 | 2026-04-05 | Defines JSON objects as unordered collections and warns that implementations differ in whether member ordering is visible. | Low | High |
| `SRC-07` | `RFC 8785: JSON Canonicalization Scheme (JCS)` | IETF / RFC Editor | `https://www.rfc-editor.org/rfc/rfc8785.txt` | 2020-06 | 2020-06 | 2026-04-05 | Defines a standards-based deterministic alternative: recursively sorted object properties for canonical JSON serialization. | Low | High |
| `SRC-08` | `CommonMark Spec 0.31.2` | CommonMark | `https://spec.commonmark.org/0.31.2/` | 2024-01-28 | 2024-01-28 | 2026-04-05 (reused from `R1`) | Still the relevant primary source for image and raw-HTML semantics; reused for freshness check only in this round. | Medium | High |

## 3. Findings by Theme

### Apple / macOS Text-System Guidance
- Finding ID:
  - `F-APL-01`
  - Research question IDs:
    - `RQ-01`
  - Source IDs:
    - `SRC-01`
  - Source-backed finding:
    - Apple describes the Cocoa text system as a layered stack built around `NSTextView`, `NSTextStorage`, `NSLayoutManager`, and `NSTextContainer`. Apple explicitly notes that `NSTextView` can control whether the user can select or edit text, wrap text, display graphic images within text, read/write rich text with attachments, and cooperate with scroll views for long document flows.
  - Model inference / host-system note:
    - This strongly supports the current `P027` shift toward an AppKit/TextKit-backed read-only Markdown document surface. Apple’s native document-viewer stack is materially richer than generic SwiftUI `Text(attributed)` when the product bar includes long technical prose, wrapping, selection, images, and scrollable document reading.
  - Host-system surface touched:
    - `ArtifactContentRenderer`
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
    - `SRC-02`
    - `SRC-04`
    - `SRC-05`
  - Source-backed finding:
    - Apple’s Cocoa text system has first-class text-table support via `NSTextTable` and `NSTextTableBlock`, and `NSTextView` has built-in support for text tables. Apple also documents that Markdown-created attributed strings carry presentation-intent attributes, while `NSPresentationIntent` includes table-related semantics.
  - Model inference / host-system note:
    - This supports `P027`’s stronger “tables are rendered as tables” bar, but it also sharpens the implementation consequence: plain `Text(AttributedString)` is unlikely to be the right final surface if the app wants real table-grade layout. A true AppKit/TextKit document renderer is the externally supported path.
  - Host-system surface touched:
    - Markdown document lane inside `ArtifactContentRenderer`
    - all artifact/report/comparison surfaces consuming Markdown
  - Time-sensitive:
    - `Low`
  - Confidence:
    - `High`

### JSON / Data Semantics
- Finding ID:
  - `F-ARCH-01`
  - Research question IDs:
    - `RQ-03`
  - Source IDs:
    - `SRC-06`
  - Source-backed finding:
    - RFC 8259 defines a JSON object as “an unordered collection” and therefore does not make source member order semantic JSON truth.
  - Model inference / host-system note:
    - `P027` can still choose source-order-friendly rendering for human inspection, but it should present that as a viewer-fidelity policy, not as a semantic property guaranteed by JSON itself.
  - Host-system surface touched:
    - `JSONTreeDocumentView`
    - payload-mismatch rescue path
    - future JSON report/receipt viewers
  - Time-sensitive:
    - `Low`
  - Confidence:
    - `High`

- Finding ID:
  - `F-ARCH-02`
  - Research question IDs:
    - `RQ-03`
  - Source IDs:
    - `SRC-06`
    - `SRC-07`
  - Source-backed finding:
    - RFC 8259 does not make ordering semantic, while RFC 8785 defines a different explicit goal for canonical JSON: properties “MUST be sorted recursively.”
  - Model inference / host-system note:
    - These are two distinct policies. Source-preserving human inspection and deterministic canonical ordering should not be mixed into one implicit contract. If `P027` wants source-order fidelity, it needs an ordered parser or token-preserving representation. If it wants deterministic canonical behavior, it should say so explicitly and prefer a documented sorted fallback.
  - Host-system surface touched:
    - `JSONTreeDocumentView`
    - implementation audit / future proposal text around key-order guarantees
  - Time-sensitive:
    - `Low`
  - Confidence:
    - `High`

### Security / Trust Boundary
- Finding ID:
  - `F-SEC-01`
  - Research question IDs:
    - `RQ-04`
  - Source IDs:
    - `SRC-03`
    - `SRC-08`
  - Source-backed finding:
    - Same-day `R1` research already showed that Markdown image handling and raw HTML semantics are broad enough that source resolution must remain an explicit policy choice.
  - Model inference / host-system note:
    - No fresh primary source in this round contradicts the current local-only fail-closed policy. `P027` should keep that direction unchanged.
  - Host-system surface touched:
    - all Markdown-consuming artifact/report/comparison surfaces
  - Time-sensitive:
    - `Medium`
  - Confidence:
    - `High`

## 4. Host-System Applicability Matrix
| Insight ID | Source IDs | Classification (`Adopt | Adapt | Watch | Reject`) | Proposal Area Affected | Host-System Surface Touched | Why It Applies or Does Not Apply | Concrete Recommended Change |
|---|---|---|---|---|---|---|
| `APP-01` | `SRC-01` | `Adopt` | `§5.3`, `§6.2`, `§7.1`, `§11` | Markdown document renderer across artifact/report/comparison surfaces | Apple’s native document-viewer stack already solves selection/editability/wrapping/images/scrolling problems that the proposal cares about. | Keep the AppKit/TextKit-backed direction; frame it as the preferred display surface for document-grade Markdown on macOS. |
| `APP-02` | `SRC-02`, `SRC-04`, `SRC-05` | `Adopt` | `§6.2`, `§7.1`, `§11` | Markdown table rendering | Apple-native table semantics exist, but they point toward a real text-system surface rather than generic `Text`. | Keep the table-grade bar and explicitly treat `Text(AttributedString)` as below the final quality bar. |
| `APP-03` | `SRC-06`, `SRC-07` | `Adapt` | `§6.3`, `§10` | JSON ordering contract | JSON semantics and JSON canonicalization are different goals. The proposal should distinguish them. | Rewrite the ordering rule so source-order preservation is “when an ordered parse is available,” with a documented deterministic fallback rather than an implied semantic guarantee. |
| `APP-04` | `SRC-03`, `SRC-08` | `Adopt` | `§6.2`, `§8`, `§10`, `§11` | Markdown image/source policy | No fresh contradiction surfaced; the local-only fail-closed rule still matches the host-system trust boundary. | Keep the current image/raw-HTML safety wording unchanged. |

## 5. Proposal Deltas / Recommended Updates
| Delta ID | Proposal Section / Decision | Recommended Update | Why It Helps | Supporting Source IDs | Supporting Local Evidence IDs | Priority |
|---|---|---|---|---|---|---|
| `DELTA-01` | `§6.2`, `§7.1`, `§11` | Keep the current AppKit/TextKit direction and make one sentence explicit: a generic SwiftUI `Text(AttributedString)` path is acceptable only as a temporary fallback or prototype, not as the final document-grade renderer. | Converts the strongest external conclusion into a directly auditable contract. | `SRC-01`, `SRC-02`, `SRC-04`, `SRC-05` | `MAP-02`, `INT-02`, `REAL-01` | `P1` |
| `DELTA-02` | `§6.3` ordering bullet | Split “stable key ordering based on source order where possible” into an explicit two-part rule: preserve source member order when the parser exposes it; otherwise apply a documented deterministic fallback order. | Aligns the proposal with JSON standards and avoids implying that generic object parsing preserves source order. | `SRC-06` | `MAP-03`, `INT-03`, `REAL-02` | `P1` |
| `DELTA-03` | `§6.3` / `§10` implementation note | If deterministic canonical ordering is desired for some surfaces, name that as a separate policy choice rather than silently conflating it with source-preserving inspection. | Keeps human-inspection UX and deterministic serialization from fighting each other. | `SRC-06`, `SRC-07` | `MAP-03`, `REAL-02` | `P2` |
| `DELTA-04` | `§8`, `§10`, `§11` | Keep the current local-only fail-closed image policy unchanged; no fresh delta is needed beyond maintaining the existing wording. | Confirms that this area is now externally supported and should not be reopened. | `SRC-03`, `SRC-08` | `REAL-03` | `P3` |

## 6. Freshness Risks / Recheck Triggers
| Trigger ID | Claim / Recommendation | Why It Is Time-Sensitive | What Must Be Rechecked | Recheck Trigger / Window | Source IDs |
|---|---|---|---|---|---|
| `FRESH-01` | AppKit/TextKit remains the best native document-grade choice for macOS Markdown reading | Apple text-system guidance can evolve with future platform APIs | Recheck if the app changes its minimum OS target or adopts a different Apple text stack | on macOS baseline change or before implementation audit if renderer design changes materially | `SRC-01`, `SRC-02`, `SRC-04`, `SRC-05` |
| `FRESH-02` | Local-only fail-closed image policy remains correct | Source semantics may change if a third-party Markdown engine or HTML renderer is introduced | Recheck parser/rendering behavior if implementation moves beyond Apple-native Markdown parsing | when parser/renderer choice changes | `SRC-03`, `SRC-08` |
| `FRESH-03` | Source-order fidelity versus canonical ordering remains a proposal-level choice, not a standards mandate | Low volatility, but implementation strategy may change | Recheck only if the team deliberately adopts canonical JSON export or an ordered JSON parser | when JSON parser/canonicalization strategy changes | `SRC-06`, `SRC-07` |

## 7. Remaining Open Questions
- `QUESTION-01`: Does `P027` want its JSON viewer to optimize for source-fidelity inspection, deterministic canonical ordering, or an explicit documented fallback hierarchy between the two?
- `QUESTION-02`: Should the proposal name `NSTextView` / Cocoa text-system primitives more concretely, or is “AppKit/TextKit-backed document surface” the right level of abstraction?
- `QUESTION-03`: Is there any v1 surface that should be allowed to stay on a weaker Markdown path temporarily, or does the proposal want a uniform document-grade bar across all migrated surfaces from day one?
