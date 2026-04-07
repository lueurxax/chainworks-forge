# Proposal 027 Implementation Audit R1

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` |
| Proposal MD5 | `cabcbc58e41d6d08a016e6919b7202a9` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-05T19:42:08+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P027` is materially implemented on the current dirty tree, but it does not yet satisfy the full proposal contract. The shared read-only renderer exists, the four targeted surfaces now compose it, fail-closed local-only Markdown image handling is live, and focused same-tree proof passed `9/9` tests in `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-193346.xcresult`. Two proposal-owned seams remain. First, the JSON tree deliberately sorts object keys alphabetically instead of preserving source order where possible. Second, the Markdown document path is only partially proven against the proposal's table-quality bar: native attributed Markdown emits table presentation intents, but the shared renderer still hands those blocks to generic `Text(attributed)`, and the local UI proof attempt failed before live visual verification could run. Readiness also stays red because there is no canonical `proposal-027` gate in `scripts/test-gate.sh`, and the local UI bundle `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-ui-20260405-193455.xcresult` failed with `Timed out while enabling automation mode`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | JSON tree ordering diverges from the proposal's source-order contract, and Markdown document fidelity is only partially proven | `High` |
| Architecture | `At Risk` | The tree parser throws away source order by sorting dictionary keys at render time | `High` |
| Product | `At Risk` | Operators still lack proven table-grade document rendering across live artifact/report surfaces | `Medium` |
| UI | `At Risk` | Local UI proof did not initialize, so the visual contract is not closed on the current tree | `High` |
| UX | `Acceptable` | Fail-closed local image handling and safe malformed-content fallback align with the proposal | `Medium` |
| Readiness | `Not Ready` | No canonical `proposal-027` gate exists, and same-tree UI proof is red | `High` |

## Proposal Contract

### Scope

- Replace raw JSON/Markdown fallbacks with one shared read-only rendering pipeline.
- Render Markdown as documents rather than payload dumps.
- Render JSON as collapsible inspectable structure.
- Migrate the existing artifact/report/comparison surfaces to the shared renderer.
- Keep artifact truth text-first on disk and avoid any editing semantics.

### Locked Decisions

- `ArtifactContentRenderer` is the only entry point higher-level screens should use.
- Artifact-backed screens inherit canonical `Artifact.format` truth rather than re-detecting per screen.
- Markdown image loading is local-only and fail-closed in v1.
- JSON tree state stays local to the current view session.
- Raw HTML execution inside Markdown is out of scope.

### Primary User Flows

1. Open a Markdown artifact or report and read it as a document instead of raw source text.
2. Open a JSON artifact or receipt and inspect it as a collapsible tree with summaries.
3. Move between artifact inspector, workflow artifact inspector, run report, and comparison surfaces without learning different rendering rules for the same content type.
4. Open malformed or unsafe content without crashing the app or triggering a remote fetch path.

### UI / UX Commitments

- Markdown surfaces use document typography, heading hierarchy, code-block separation, list indentation, links, tables, and safe images.
- JSON surfaces use disclosure affordances, counts, collapsed summaries, and controlled expansion defaults.
- No editing affordances are introduced.
- Unsupported or malformed content degrades to safe read-only fallback.

### Test / Evidence Requirements

- Shared renderer foundation plus migration of the named primary surfaces.
- Artifact-backed rendering uses canonical format truth.
- Markdown image handling is local-only and fail-closed.
- Malformed content does not crash.
- For any successful audit roll-up, same-tree full regression would also be required.

## Proposal Fidelity / Divergence

### Matches

- The current tree defines `ArtifactContentRenderer`, `MarkdownDocumentView`, `JSONTreeDocumentView`, `PlainTextArtifactView`, and `DiffArtifactView` in one shared file.
- `ArtifactInspectorView`, `WorkflowArtifactInspectorView`, `RunReportView`, and `RunComparisonView` now route through the shared renderer instead of screen-local Markdown/JSON shortcuts.
- Artifact-backed render context inherits canonical `Artifact.format` truth and local roots from the artifact/run owner path.
- Markdown image loading is constrained to local artifact/workspace roots, with remote URLs rejected.
- Focused same-tree proof for `Proposal027Tests` is green.

### Divergences

- JSON object entries are rendered in alphabetical order, not source order where possible.
- The Markdown path is still only partially proven against the proposal's explicit table-rendering bar.

### Ambiguities / Evidence Gaps

- The local UI proof bundle failed before tests could assert live rendering behavior, so this audit could not close the screen-level visual contract with runtime evidence.
- `scripts/test-gate.sh` does not define a canonical `proposal-027` gate, so there is no repeatable proposal-owned lane for future same-tree proof.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 7 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Shared read-only renderer foundation exists

- Proposal Source: `§5.3`, `§6.1`, `§9 Phase 1`, `§10 AC3`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-193346.xcresult`
- Gap / Note: The shared renderer and typed document views are present on the current tree and compile under focused same-tree proof.

### REQ-002 Primary artifact/report/comparison surfaces use the shared renderer

- Proposal Source: `§6.5`, `§9 Phase 2`, `§10 AC1-AC3`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
- Gap / Note: The four named migration targets now compose `ArtifactContentRenderer` instead of keeping local Markdown/JSON display branches.

### REQ-003 Artifact-backed surfaces consume canonical `Artifact.format` truth

- Proposal Source: `§6.1`, `§10 AC4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-193346.xcresult`
- Gap / Note: `ArtifactRenderContext.artifactBacked(...)` carries canonical `artifact.format` plus local roots, and the focused test suite asserts that owner path directly.

### REQ-004 Markdown renders as a document rather than raw text

- Proposal Source: `§6.2`, `§7.1`, `§10 AC1`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`, `runtime`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - Xcode MCP `ExecuteSnippet` against `Chainworks Forge/Views/ArtifactContentRenderer.swift` during this audit showed native Markdown table presentation intents for `AttributedString(markdown:)`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-ui-20260405-193455.xcresult`
- Gap / Note: Headings/lists/links/images flow through the shared Markdown path, and native Markdown preserves table semantics at the attributed-string layer. The unresolved gap is the proposal's stricter visual contract that tables should render as tables rather than plaintext approximations. The live UI proof failed before runtime verification, and the current renderer still relies on generic `Text(attributed)` rather than a table-specific block renderer.

### REQ-005 JSON renders as a collapsible tree with summaries and safe fallback

- Proposal Source: `§6.3`, `§7.2`, `§8`, `§10 AC2`, `§10 AC6`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-193346.xcresult`
- Gap / Note: The JSON path uses recursive disclosure groups, container counts, collapsed summaries, seeded expansion defaults, and a parse-failed fallback to plain text.

### REQ-006 JSON key ordering preserves source order where possible

- Proposal Source: `§6.3`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: The current tree guarantees stable ordering, but it achieves that by sorting dictionary keys alphabetically (`dictionary.keys.sorted()`). That does not satisfy the proposal's stronger requirement to preserve source order where possible.

### REQ-007 Markdown image handling is local-only and fail-closed in v1

- Proposal Source: `§6.2`, `§8`, `§10 AC5`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-193346.xcresult`
- Gap / Note: Remote URLs are rejected, file URLs are constrained to allowed local roots, and disallowed sources degrade to safe placeholder/source text instead of fetch.

### REQ-008 No editing affordances are introduced

- Proposal Source: `§3`, `§4`, `§10 AC7`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The shared renderer is read-only: `Text`, `DisclosureGroup`, `Image`, and placeholders only. No editor or authoring affordance is introduced on the audited paths.

### REQ-009 Artifact and report truth remain text-first and unchanged on disk

- Proposal Source: `§6.4`, `§10 AC8`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
- Gap / Note: The renderer is presentation-only. Report/artifact persistence still writes Markdown/JSON text files without introducing render-derived storage.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 JSON tree rendering discards source-order truth

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-006`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Why It Matters: Operators inspect receipts and reports in the order authors or upstream emitters chose. Alphabetizing keys creates a second presentation order that can separate related fields and weaken visual traceability back to source artifacts.
- Recommended Action: Preserve parse order end-to-end when JSON decoding makes it available, or carry an ordered object representation into the tree builder instead of calling `sorted()`.

## Product Review

**Summary:** `At Risk`

### PROD-001 Document-quality Markdown is still weaker than the proposal's stated bar

- Severity: `Major`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-004`
- Evidence Type: `code`, `runtime`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - Xcode MCP `ExecuteSnippet` against `Chainworks Forge/Views/ArtifactContentRenderer.swift` during this audit
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-ui-20260405-193455.xcresult`
- Why It Matters: The proposal did not just ask for richer text. It promised that operators could read Markdown as documents and specifically called out tables as first-class structure. The current implementation likely improves common prose, but it still lacks live proof that tables are rendered with genuine document readability across the audited surfaces.
- Recommended Action: Add a dedicated visual proof path for Markdown tables on the shared renderer, and move to a stronger block renderer if `Text(AttributedString)` does not meet the contract in runtime UI.

## UI Review

**Summary:** `At Risk`

### UI-001 Live screen-level proof for the new renderer is missing on the current tree

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-002`, `REQ-004`, `REQ-005`
- Evidence Type: `runtime`
- Evidence:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-ui-20260405-193455.xcresult`
  - failure text: `Timed out while enabling automation mode.`
- Why It Matters: The proposal is explicitly UI-facing. Without a passing live artifact/report surface proof, the audit cannot close the visual consistency contract even though code-level migration is real.
- Recommended Action: Restore a reliable UI proof path for artifact inspector/report surfaces before claiming the proposal is fully delivered.

## UX Review

**Summary:** `Acceptable`

### UX-001 Safety and fallback behavior align with the v1 trust model

- Severity: `Note`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-005`, `REQ-007`, `REQ-008`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Why It Matters: The current tree already protects operators from malformed JSON crashes and remote image fetch surprises while keeping interaction simple and read-only.
- Recommended Action: Keep this fail-closed posture as the renderer grows richer; do not widen source trust implicitly.

## Readiness Review

**Summary:** `Not Ready`

### READY-001 There is no canonical proposal-owned gate, and local UI proof is red

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: Test / Evidence Requirements
- Evidence Type: `code`, `runtime`
- Evidence:
  - `scripts/test-gate.sh`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-ui-20260405-193455.xcresult`
- Why It Matters: Even if the remaining conformance gaps were closed, the current tree still lacks a repeatable proposal-owned gate and a passing same-tree UI proof lane. That makes regression detection and future re-audits weaker than the repo's newer proposal baselines.
- Recommended Action: Add a canonical `proposal-027` gate that exercises the shared renderer paths, and keep at least one passing UI proof on the same tree before rolling this proposal up to success.

## Roll-Up

- `Overall Conformance = Partial`
- `Overall Readiness = Not Ready`
- `Audit Confidence = High`

`P027` has real implementation behind it, but the current tree is not yet at a successful audit state. The immediate proposal-owned fixes are to preserve source-order key handling in the JSON tree and to close the Markdown table/document-quality bar with live proof rather than generic attributed-text best effort.
