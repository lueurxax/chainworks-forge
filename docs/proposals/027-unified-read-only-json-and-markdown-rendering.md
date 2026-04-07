# Proposal 027: Unified Read-Only JSON And Markdown Rendering

| Field | Value |
|---|---|
| Date | 2026-04-05 |
| Status | Draft |
| Author | Codex |
| Depends on | [../reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [015-skill-resolution-and-runtime-injection.md](015-skill-resolution-and-runtime-injection.md), [025-per-agent-mcp-policy-and-runtime-validation.md](025-per-agent-mcp-policy-and-runtime-validation.md) |
| Scope | Replace raw text fallbacks for JSON and Markdown across the app with a single read-only rendering pipeline that supports collapsible JSON trees and proper Markdown document rendering. |
| Goal | Make JSON and Markdown artifacts readable everywhere in `Chainworks Forge` without introducing editing flows, while preserving deterministic artifact truth and keeping operator surfaces fast and consistent. |

---

## 1. Context and Motivation

`Chainworks Forge` already produces and displays many human-facing artifacts:

- proposal drafts and reviews
- run reports and immutable report history
- resolved skill content
- workflow artifacts in idea detail panes
- JSON-based execution truth and receipts

The current rendering story is fragmented and weak:

- Markdown often renders as raw text through `Text(content)`
- JSON is usually pretty-printed text, not structured data
- different screens render the same content types differently
- large artifacts become visually noisy and hard to scan
- operator UX loses information hierarchy exactly where it matters most

Today the problem is visible in at least these surfaces:

- [ArtifactInspectorView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/ArtifactInspectorView.swift)
- [IdeaListView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/IdeaListView.swift)
- [RunReportView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/RunReportView.swift)
- [RunComparisonView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/RunComparisonView.swift)

This creates avoidable operator friction:

1. JSON receipts and reports are hard to inspect because there is no collapsible tree.
2. Markdown documents do not read like documents; they read like raw transport payloads.
3. The same artifact looks different depending on where it is opened.
4. Every new surface is tempted to add yet another local formatting shortcut.

This proposal fixes that by introducing a single app-wide read-only rendering system for structured documents.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can the app render Markdown as an actual document everywhere, not raw text?
2. Can the app render JSON as a navigable tree with collapse and expand behavior?
3. Can all existing artifact/report/comparison surfaces share one rendering path?
4. Can this be done without introducing editing semantics?
5. Can large artifacts remain readable without regressing performance?

---

## 3. Scope

This proposal includes:

- a unified read-only content rendering layer for JSON and Markdown
- app-wide replacement of raw `Text(...)` fallbacks for these formats
- a collapsible JSON tree viewer
- a proper Markdown document renderer
- consistent theming and interaction behavior across artifact surfaces
- graceful fallback for unsupported or malformed input

This proposal does **not** include:

- JSON editing
- Markdown editing
- WYSIWYG document authoring
- outline navigation or section jumping in the first version
- schema-aware JSON forms
- syntax-aware code editing
- arbitrary HTML execution inside Markdown

---

## 4. Design Principles

1. Read-only means read-only.
2. The same content type should render the same way everywhere.
3. JSON should be navigated as structure, not as formatted text.
4. Markdown should be read as a document, not as a payload dump.
5. The renderer should degrade safely on malformed content.
6. Artifact truth stays textual on disk; richer rendering is a presentation concern.
7. New surfaces should compose the shared renderer instead of inventing local formatting logic.

---

## 5. Recommended Approach

Three approaches are possible:

### 5.1 Keep native `Text` plus small polish

- Improve pretty-printing
- Add more fonts and spacing
- Keep local per-screen renderers

This is not enough.
It does not solve structural JSON navigation, it preserves inconsistent rendering, and it keeps the app in the current fragmented state.

### 5.2 Use a web view for everything

- Convert Markdown to HTML
- Render JSON through a browser-style tree widget
- Reuse one embedded web surface everywhere

This would produce decent rendering quality, but it makes document viewing heavier than necessary, complicates theming and accessibility, and introduces a browser stack for routine operator surfaces.

### 5.3 Recommended: Native unified renderer with typed document views

- Introduce one shared `ArtifactContentRenderer`
- Route Markdown through a dedicated AppKit/TextKit-backed document renderer
- Route JSON through a dedicated tree renderer
- Keep diff/report/plain-text paths explicit and separate

This is the recommended option because it fixes the product problem directly without turning artifact viewing into a mini browser.

---

## 6. Architecture

### 6.1 Introduce a shared rendering layer

The app should define a reusable rendering stack:

- `ArtifactContentRenderer`
- `MarkdownDocumentTextView`
- `JSONTreeDocumentView`
- `PlainTextArtifactView`
- `DiffArtifactView`
- `ArtifactRenderTheme`

`ArtifactContentRenderer` becomes the only entry point that higher-level screens use.
For artifact-backed surfaces, the canonical format owner remains the existing repo truth:

- `Artifact.format` when an `Artifact` already exists
- `ArtifactFormat.detect(from:contract:)` at artifact creation or other centralized format-resolution seams

The shared renderer must inherit that authority rather than re-detecting format per screen.
That means artifact-backed screens should provide:

- artifact-backed content
- canonical `ArtifactFormat`
- optional title or artifact metadata
- rendering context if needed for styling

Only non-artifact content may use an explicit render request that names a format directly.
Those cases should be narrow and intentional, for example:

- resolved skill content
- ephemeral preview content before artifact persistence

Screens should **not** provide:

- screen-local sniffed format
- ad hoc Markdown-or-JSON guessing logic
- fallback format rules that compete with `.report`

Screens should stop owning formatting decisions such as:

- whether Markdown is plain text or rich text
- whether JSON is pretty-printed or structured
- which font to use for the same artifact type

The shared renderer should resolve a presentation intent before choosing a concrete view.
That intent layer must support three distinct cases:

- true Markdown document content
- true JSON structured content
- payload-mismatch rescue, where declared Markdown or report content is actually valid top-level JSON and should therefore render as structured JSON without changing canonical artifact truth

This keeps canonical format ownership intact while still letting the operator read malformed or mislabeled payloads sanely.

### 6.2 Markdown becomes a proper document renderer

Markdown rendering must support normal document reading:

- headings
- paragraphs
- emphasis
- lists
- block quotes
- inline code
- fenced code blocks
- links
- tables
- images, only where explicitly allowed by source policy

Markdown in this proposal is a **presentation-intent** surface, not a source-editing surface.
The renderer's job is to preserve document reading semantics and hierarchy for operators while leaving the underlying Markdown text unchanged on disk.

The first acceptable implementation class for this renderer is a native AppKit/TextKit document surface, not a simple SwiftUI `Text(AttributedString)` fallback.
The product quality bar is explicitly document-grade rendering, comparable to a notes/document reader rather than a payload dump.

The renderer should preserve a document feel:

- readable line height
- clear heading hierarchy
- stable spacing between blocks
- code blocks that visually separate from prose
- tables that remain legible without collapsing into plain text
- paragraph spacing that makes dense technical prose readable
- list indentation and continuation wrapping that still reads correctly under long technical lines
- text selection, wrapping, and scrolling behavior consistent with a real document viewer

Implementation note:
the exact parser can still be native markdown parsing or a dedicated markdown library, but the display surface should be a document-grade text system.
For the first version, this proposal prefers an AppKit/TextKit-backed read-only renderer over `Text(AttributedString)` because the latter does not meet the layout-quality bar for long technical documents.

Image/source policy for v1 is fail-closed:

- local artifact files and workspace-relative local files may render
- remote URLs must not be fetched by the renderer
- unsupported image sources should render as a safe placeholder or source badge, not trigger a fetch path
- if source safety cannot be established, the renderer should display the source reference as text rather than loading it

This keeps the renderer aligned with the app's current local artifact/workspace boundary and avoids opening a new network-trust lane inside read-only operator surfaces.

Raw HTML inside Markdown is fallback-only in v1:

- no HTML execution
- no HTML-powered rich rendering contract
- unsupported HTML should degrade to safe text presentation rather than opening a second rendering engine inside the Markdown path

### 6.3 JSON becomes a tree, not a monospaced blob

JSON rendering should parse into a structured value tree and render recursively with disclosure controls.
The preferred native implementation shape is a recursive SwiftUI tree built from `DisclosureGroup`-style nodes, or `OutlineGroup` where it cleanly matches the interaction model.

This JSON lane is also the canonical rescue path for payload-mismatch cases:

- declared `.json` artifacts render as JSON tree by default
- declared `.markdown` or `.report` artifacts that are actually valid top-level JSON object/array should render as JSON tree with presentation-level rescue
- this rescue does not rewrite `Artifact.format` and does not open a second format-truth owner

The viewer should support:

- collapse and expand per node
- stable key ordering based on source order where possible
- if the chosen parser cannot preserve source member order, object keys must fall back to a deterministic ascending string sort for presentation rather than leaving ordering implementation-defined
- arrays and objects with visible counts
- compact previews for collapsed nodes
- scalar rendering for strings, numbers, booleans, and null
- indentation that clearly shows depth

This fallback is presentation-only. It does not redefine artifact truth on disk and does not introduce RFC-8785-style canonicalization as a storage contract.

Recommended interaction defaults:

- root expanded
- first two levels expanded for small payloads
- large nested branches collapsed by default
- malformed JSON falls back to plain monospaced text with a parse-failed badge or note

The first version does not need:

- search
- copy-by-path controls
- schema awareness
- inline editing

### 6.4 Artifact truth stays textual

Artifacts on disk remain exactly what they are now:

- Markdown files remain Markdown
- JSON files remain JSON text

The richer viewer is presentation-only.
This matters because proposal truth, run truth, receipts, and immutable report history should not become dependent on a render cache or a transformed document model.

### 6.5 One renderer, many surfaces

The first migration wave should replace local rendering logic in:

- [ArtifactInspectorView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/ArtifactInspectorView.swift)
- [IdeaListView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/IdeaListView.swift)
- [RunReportView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/RunReportView.swift)
- [RunComparisonView.swift](/Users/user/Documents/Chainworks%20Forge/Chainworks%20Forge/Views/RunComparisonView.swift)

That gives the app one rendering contract for:

- artifact inspectors
- workflow artifact previews
- report summaries
- immutable history entries
- resolved skill content
- future structured document surfaces

---

## 7. UI Commitments

### 7.1 Markdown

Markdown should visually read like a document.
That means:

- body text uses normal document typography, not monospaced fallback
- headings create clear vertical hierarchy
- fenced code blocks use monospaced styling and a distinct container
- lists retain indentation and bullet clarity
- links are visually distinct
- tables are rendered as tables, not plaintext approximations
- long technical paragraphs still read cleanly because paragraph spacing and wrapping are document-grade
- the result should feel closer to Notes/TextEdit document reading than to log text inside a `ScrollView`

### 7.2 JSON

JSON should visually read like inspectable structure.
That means:

- disclosure affordances are obvious
- keys remain easy to scan
- nested values do not explode the whole page by default
- objects and arrays show meaningful collapsed summaries
- long strings wrap or scroll in a controlled way instead of blowing out layout

### 7.3 Consistency

The operator should not have to learn separate reading models for:

- artifact inspectors
- report screens
- comparison views
- workflow detail panes

If the content is Markdown, it should look like the app's Markdown renderer.
If the content is JSON, it should look like the app's JSON renderer.

---

## 8. Performance and Safety

The renderer should be safe and predictable.

Requirements:

- malformed Markdown or JSON must not crash the app
- very large JSON payloads must not eagerly expand every node
- rich Markdown rendering must not execute arbitrary HTML or script content
- image rendering must be fail-closed and local-only in v1
- the renderer should degrade to safe text fallback if parsing fails

For Markdown image sources specifically:

- local artifact-root paths are allowed
- workspace-relative local paths are allowed
- remote HTTP(S) fetch is disabled by default
- any future remote image support requires a separate proposal that introduces an explicit source-trust policy and operator-facing capability contract

For JSON specifically:

- tree state should be local to the current view session
- collapse state should not become durable application state in v1
- deep or large branches should use lazy rendering where needed

---

## 9. Migration Plan

### Phase 1: Shared renderer foundation

- introduce `ArtifactContentRenderer`
- introduce `MarkdownDocumentView`
- introduce `JSONTreeDocumentView`
- keep existing diff and plain text rendering paths available

### Phase 2: Migrate primary artifact surfaces

- replace `formatAwareRenderer` logic in `ArtifactInspectorView`
- replace `WorkflowArtifactInspectorView` local formatting path
- replace report summary/history raw markdown rendering in `RunReportView`
- replace resolved skill raw markdown rendering in `RunComparisonView`

### Phase 3: Remove divergent fallbacks

- delete duplicate Markdown rendering code paths
- delete duplicate JSON pretty-print-only paths where the new renderer applies
- standardize artifact theming and spacing

---

## 10. Acceptance Criteria

This proposal is complete when all of the following are true:

1. Markdown artifacts render as proper documents in every existing artifact/report/comparison surface.
2. JSON artifacts render as collapsible trees in every existing artifact/report/comparison surface.
3. There is one shared rendering entry point rather than multiple local format heuristics.
4. Artifact-backed surfaces consume canonical `Artifact.format` truth and do not re-detect format per screen.
5. Declared Markdown/report payloads that are actually valid top-level JSON object/array render as structured JSON without mutating canonical artifact metadata.
6. Markdown image handling is fail-closed and local-only in v1.
7. Malformed Markdown and malformed JSON fall back safely without crashing.
8. No editing affordances are introduced.
9. Existing artifact truth and report truth remain text-first and unchanged on disk.

---

## 11. Open Decisions

Implementation should explicitly choose one Markdown parser/backend, but the v1 Markdown display surface is not open-ended: it should use an AppKit/TextKit-backed native read-only document presentation surface rather than a weak `Text(...)` fallback.

Allowed parser/backend choices include:

- native attributed markdown with custom block extraction/styling, or
- a dedicated markdown rendering dependency

The parser choice remains open; the display-surface class does not.
If a chosen parser cannot meet the document-quality bar for tables, fenced code blocks, long technical prose, and images when rendered through the AppKit/TextKit-backed surface, the implementation should prefer a stronger parser rather than shipping another weak plaintext compromise.

The proposal does **not** leave image trust policy open:

- v1 is local-only and fail-closed
- remote image loading is out of scope
- any expansion beyond current local artifact/workspace boundaries requires a separate proposal

---

## 12. Expected Outcome

After Proposal 027, `Chainworks Forge` should stop treating JSON and Markdown as inert strings.

Instead:

- JSON becomes inspectable structure
- Markdown becomes readable documentation
- artifact surfaces become consistent
- operators can actually consume the artifacts the system already produces

That is a direct product quality improvement, not cosmetic polish.
