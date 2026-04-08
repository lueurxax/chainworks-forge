# Artifact Content Rendering

Stable reference for the unified read-only rendering slice for Markdown and JSON artifacts.

## Purpose

This document defines the long-lived contract for how the app renders artifact content that is already text on disk but should be readable in operator surfaces.

The implementation goal is simple:

- use one renderer entry point for Markdown and JSON surfaces,
- keep artifact truth owned by models and files,
- provide document-grade Markdown readability without edits,
- provide structured, navigable JSON inspection without editing,
- keep source trust and failures bounded to local artifacts and explicit local roots.

## Scope

This slice currently covers:

- `ArtifactContentRenderer` entry-point routing,
- Markdown rendering in `ArtifactInspectorView`, `IdeaListView`, `RunReportView`, and `RunComparisonView`,
- JSON rendering via disclosure-driven tree views on the same surfaces,
- payload-mismatch rescue for JSON-like markdown/report text,
- local-only markdown image policy with safe placeholder fallback.

It does not define:

- JSON mutation,
- markdown editor or WYSIWYG write path,
- HTML execution or remote image fetching,
- schema-aware JSON forms,
- global artifact-search features.

## Canonical ownership contract

Artifact-backed content keeps canonical format ownership in persisted artifact metadata:

- `Artifact.format` remains the format truth source.
- `ArtifactRenderContext.artifactBacked(artifact:run:)` carries that truth into rendering.
- `ArtifactRenderContext.explicit(format:)` is limited to non-artifact callers.

No screen should independently sniff or rewrite artifact format.

## Rendering contract

`ArtifactContentRenderer` is the single entry point and resolves one of four intents:

1. Markdown document
2. JSON tree
3. diff artifact
4. plain text fallback

### Markdown document path

For `.markdown` content, the renderer uses the AppKit/TextKit-backed document surface:

- `MarkdownDocumentTextView` is an `NSViewRepresentable` wrapper over `NSTextView`,
- `NSTextView` is non-editable and non-interactive except for copy/select,
- text is laid out in document-like blocks with heading/list/code/table/image roles.

The Markdown parser is currently explicit-block based (`#`, paragraphs, list, quote, code block, tables, image syntax). Tables and images are represented as structured blocks and mapped to dedicated views.

Local image policy is enforced by `MarkdownImageSourcePolicy.v1`:

- local artifact-root and workspace-root files are allowed,
- unsafe extensions are rejected,
- protected package ancestry is rejected,
- remote URLs are rejected.

### JSON document path

For `.json` content, the renderer displays a navigable disclosure tree (`JSONTreeDocumentView`):

- top-level objects and arrays parse from JSON text,
- nested nodes render with key/value summaries and expandable disclosure,
- arrays/objects get compact collapsed summaries,
- parse failures fall back to monospaced plain text with parse-failed fallback.

Current implementation ordering for object members is deterministic lexical sorting of parsed keys.
This keeps presentation stable across runs with Foundation-backed parsing and avoids unstable dictionary order assumptions.

### Payload-mismatch rescue

The resolver rescues malformed content-class declarations when the payload is actually JSON:

- `.markdown` or `.report` containing top-level object/array is rendered through JSON tree intent,
- canonical artifact format (`Artifact.format`) remains unchanged.

### Failure and fallback path

- malformed JSON and malformed markdown blocks do not crash the renderer,
- unsupported markdown image sources render safe placeholders,
- unknown markdown blocks preserve readable plain-text fallbacks.

## Surface coverage

The stable surface list is:

- `ArtifactInspectorView`
- workflow artifact cards in `IdeaListView`
- `RunReportView`
- `RunComparisonView`

Any new markdown/json artifact surface must consume `ArtifactContentRenderer` and provide a valid `ArtifactRenderContext`.

## Performance, state, and safety

Presentation-only rendering does not persist UI expansion state.
The JSON tree keeps per-view local disclosure state and does not write tree state to artifacts.

Parsing and rendering are local to the view lifecycle; no network fetch path exists in renderer markdown/image policy v1.

## Acceptance expectations (stable contract)

This slice is treated as implemented when all of the following remain true:

- one renderer entry point owns all primary artifact-bearing surfaces,
- markdown is rendered by the read-only AppKit/TextKit-backed document path,
- JSON artifacts render as disclosure trees,
- format truth remains artifact-backed and does not mutate artifact storage,
- local image and parse-fallback safety remain fail-closed,
- local tree state stays ephemeral.

## Verification baseline

Current stable verification for this slice is:

- dedicated renderer capability regression coverage on the current tree
- capability verification covers:
  - markdown document rendering,
  - JSON tree rendering,
  - payload-mismatch rescue,
  - safe image-source fallback,
  - timeline/provider-error presentation polish,
- same-tree `full` remains the repository-level regression backstop.

## Adjacent references

- [operator-experience.md](operator-experience.md)
- [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md)
- [output-contracts-failure-evidence-and-recovery.md](output-contracts-failure-evidence-and-recovery.md)
