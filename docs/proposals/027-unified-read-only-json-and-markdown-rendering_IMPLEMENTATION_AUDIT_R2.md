# Proposal 027 Implementation Audit R2

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` |
| Proposal MD5 | `829a3ab472c3cb6b95a509870d0df882` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Dirty (9 modified, 1 untracked)` |
| Audited At | `2026-04-05T20:55:04+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P027` is proposal-complete on the current dirty tree, but this audit cannot roll up to a successful verdict. The shared renderer is live across the named artifact/report/comparison surfaces, the Markdown path is now AppKit/TextKit-backed with dedicated table and image handling, the JSON path matches the tightened deterministic ordering fallback contract, and focused same-tree proof passed `13/13` in `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`.

The verdict still fails closed at roll-up time. Under the audit skill, any successful outcome requires same-tree full regression evidence. On this host, `./scripts/test-gate.sh full` is unavailable because the repository marks full UI-inclusive regression as remote-only and this machine is not an approved remote UI host (`approved remote hosts: smacbook.local, smacbook`; `observed host names: 0000659.localdomain, 0000659`). Separately, `scripts/test-gate.sh` still has no canonical `proposal-027` lane, so there is no repeatable proposal-scoped signoff path yet.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | All audited `REQ-*` items are implemented, but successful roll-up is blocked by missing same-tree full regression evidence | `High` |
| Architecture | `Acceptable` | No live proposal-owned architecture divergence remained on the current tree | `High` |
| Product | `Acceptable` | Remaining risk is delivery proof, not the renderer contract itself | `Medium` |
| UI | `Acceptable` | No proposal-scoped runtime UI gate exists yet for repeatable surface proof | `Medium` |
| UX | `Acceptable` | Safety/fallback behavior is strong, but final confidence still depends on broader signoff proof | `Medium` |
| Readiness | `Not Ready` | `full` regression is remote-only and unavailable on the current host, and no canonical `proposal-027` gate exists | `High` |

## Proposal Contract

### Scope

- Replace raw text fallbacks for JSON and Markdown with a unified read-only renderer.
- Render Markdown as proper documents rather than payload dumps.
- Render JSON as collapsible inspectable structure.
- Migrate the current artifact/report/comparison surfaces to the shared renderer.
- Keep artifact truth text-first on disk and avoid editing semantics.

### Locked Decisions

- `ArtifactContentRenderer` is the shared entry point for higher-level screens.
- Artifact-backed screens inherit canonical `Artifact.format` truth rather than screen-local format detection.
- Markdown display uses an AppKit/TextKit-backed document presentation path, not weak `Text(...)` fallback.
- Markdown images are local-only and fail-closed in v1.
- JSON ordering should preserve source order where possible, with deterministic ascending sort fallback when preservation is unavailable.
- Artifact truth remains textual on disk.

### Primary User Flows

1. Open a Markdown artifact/report and read it as a document in existing operator surfaces.
2. Open a JSON artifact/report and inspect it as a collapsible tree.
3. Move between artifact inspector, workflow detail pane, run report, and comparison surfaces without learning different rendering rules for the same content type.
4. Open malformed or unsafe content without crashing the app or triggering a remote fetch path.

### UI Commitments

- Markdown uses document typography, hierarchy, code-block separation, table rendering, link styling, and safe image handling.
- JSON uses disclosure affordances, counts, collapsed summaries, and bounded default expansion.
- Rendering is visually consistent across the named operator surfaces.

### UX Commitments

- Read-only means read-only.
- Malformed content degrades safely.
- Large content remains inspectable without exploding layout or opening unsafe fetch paths.

### Acceptance Criteria

1. Markdown artifacts render as proper documents in every existing artifact/report/comparison surface.
2. JSON artifacts render as collapsible trees in every existing artifact/report/comparison surface.
3. One shared rendering entry point replaces local format heuristics.
4. Artifact-backed surfaces consume canonical `Artifact.format` truth.
5. Declared Markdown/report JSON payloads rescue into structured JSON presentation without mutating artifact truth.
6. Markdown image handling is fail-closed and local-only in v1.
7. Malformed Markdown and malformed JSON fall back safely without crashing.
8. No editing affordances are introduced.
9. Existing artifact truth and report truth remain text-first and unchanged on disk.

### Test / Evidence Requirements

- Shared renderer foundation plus migration of the named primary surfaces.
- Focused evidence that canonical format truth drives artifact-backed rendering.
- Evidence that local-only image handling and safe fallback remain intact.
- For any successful audit outcome, same-tree full regression evidence must exist.

### Explicit Exclusions

- No JSON editing.
- No Markdown editing.
- No WYSIWYG authoring.
- No arbitrary HTML execution inside Markdown.

## Proposal Fidelity / Divergence

### Matches

- `ArtifactContentRenderer`, `MarkdownDocumentTextView`, `MarkdownDocumentView`, `JSONTreeDocumentView`, `PlainTextArtifactView`, and `DiffArtifactView` exist on the current tree.
- The named primary surfaces now compose the shared renderer.
- Artifact-backed rendering uses canonical `Artifact.format` truth via `ArtifactRenderContext.artifactBacked(...)`.
- Declared Markdown/report payloads that are valid top-level JSON rescue into structured JSON presentation.
- Markdown images are constrained to local artifact/workspace roots and fail closed for remote or out-of-bound sources.
- JSON objects render via recursive disclosure groups with deterministic ascending sort fallback.

### Divergences

- No live proposal-owned implementation divergence was found on the current tree.

### Ambiguities / Evidence Gaps

- The audit could not obtain same-tree `full` regression evidence from this host because `scripts/test-gate.sh full` is remote-only and this machine is not in the approved host list.
- The repository still lacks a canonical `proposal-027` lane in `scripts/test-gate.sh`, so proposal-scoped signoff is not yet repeatable through the standard gate catalog.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 10 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Shared read-only renderer foundation exists

- Proposal Source: `§5.3`, `§6.1`, `§9 Phase 1`, `§10 AC3`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`
- Gap / Note: Shared renderer foundation is live and passes focused same-tree proof.

### REQ-002 Primary artifact/report/comparison surfaces use the shared renderer

- Proposal Source: `§6.5`, `§9 Phase 2`, `§10 AC1-AC3`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
- Gap / Note: The four named migration targets now route content through `ArtifactContentRenderer`.

### REQ-003 Artifact-backed surfaces consume canonical `Artifact.format` truth

- Proposal Source: `§6.1`, `§10 AC4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`
- Gap / Note: `ArtifactRenderContext.artifactBacked(...)` preserves canonical artifact format plus local roots from the owning run/artifact path.

### REQ-004 Markdown renders as a document-grade AppKit/TextKit-backed surface

- Proposal Source: `§6.2`, `§7.1`, `§10 AC1`, `§11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`
- Gap / Note: The Markdown path now uses `MarkdownDocumentTextView` (`NSViewRepresentable` / `NSTextView`) for prose, code, and table cells, with dedicated list, quote, table, and image block views instead of weak `Text(...)` fallback.

### REQ-005 JSON renders as a collapsible tree with deterministic fallback ordering

- Proposal Source: `§6.3`, `§7.2`, `§10 AC2`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`
- Gap / Note: Recursive disclosure-group rendering is live, and `JSONTreeNode.build(...)` now matches the proposal's explicit deterministic ascending-sort fallback when source order cannot be preserved.

### REQ-006 Declared Markdown/report JSON payloads rescue into structured JSON presentation

- Proposal Source: `§6.1`, `§6.3`, `§10 AC5`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`
- Gap / Note: `ArtifactPresentationIntent.resolve(...)` plus `StructuredPayloadProbe` preserve canonical format truth while rescuing valid top-level JSON for presentation.

### REQ-007 Markdown image handling is local-only and fail-closed in v1

- Proposal Source: `§6.2`, `§8`, `§10 AC6`, `§11`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`
- Gap / Note: Remote URLs are rejected, local resolution is constrained to artifact/workspace roots, and disallowed sources degrade to a safe placeholder/source badge.

### REQ-008 Malformed Markdown and malformed JSON fall back safely without crash

- Proposal Source: `§8`, `§10 AC7`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: Malformed JSON falls back to monospaced plain text with a parse-failed label, and Markdown block/attributed parsing uses non-throwing or explicit fallback paths instead of crashing the viewer.

### REQ-009 No editing affordances are introduced

- Proposal Source: `§3`, `§4`, `§10 AC8`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The shared renderer is read-only. `MarkdownDocumentTextView` explicitly sets `isEditable = false`, and the other renderer branches use display-only SwiftUI views.

### REQ-010 Artifact truth and report truth remain text-first and unchanged on disk

- Proposal Source: `§6.4`, `§10 AC9`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
- Gap / Note: Persistence still writes Markdown/JSON strings directly to disk; the renderer is presentation-only and does not rewrite stored artifact truth.

## Architecture Review

**Summary:** `Acceptable`

No material proposal-owned architecture divergence remained on the current tree. Rendering responsibility is centralized in one shared path, format authority stays with the existing artifact owner model, and the JSON/Markdown special cases no longer fragment into screen-local heuristics.

## Product Review

**Summary:** `Acceptable`

The core product job described by `P027` is now met on the current tree: operators can read Markdown artifacts as documents, inspect JSON as structure, and move across the named surfaces without learning separate content rules.

## UI Review

**Summary:** `Acceptable`

The current implementation meets the proposal's UI contract at code level: document-grade Markdown blocks, table-specific rendering, bounded JSON disclosure defaults, and consistent renderer usage across the audited surfaces. The remaining weakness is proof ownership, not UI composition.

## UX Review

**Summary:** `Acceptable`

Safety and readability are materially better than the raw-text baseline. The local-only image policy, JSON parse-failed fallback, and read-only interaction model align with the proposal's trust and operator-clarity goals.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Successful roll-up is blocked because same-tree full regression is unavailable on this host

- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001` through `REQ-010`
- Evidence Type: `tests-run`, `runtime`
- Evidence:
  - `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult`
  - `./scripts/test-gate.sh full`
  - failure text: `error: UI tests are remote-only and may not run on this host.`
  - failure text: `approved remote hosts: smacbook.local,smacbook`
  - failure text: `observed host names: 0000659.localdomain,0000659`
- Why It Matters: Under the current audit skill, `Implemented` / `Ready` verdicts require passing same-tree full regression. That evidence cannot be produced from this host, so a successful verdict would overstate what was actually proven.
- Recommended Action: Sync the exact same tree to an approved remote UI host and rerun `./scripts/test-gate.sh full` there before attempting a successful audit roll-up.

### READY-002 The repository still lacks a canonical `proposal-027` signoff lane

- Severity: `Minor`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`, `REQ-002`, `REQ-004`, `REQ-005`
- Evidence Type: `code`
- Evidence:
  - `scripts/test-gate.sh`
  - gate catalog around the top-level case list includes `proposal-006`, `proposal-012`, `proposal-013`, `proposal-014`, `proposal-015`, `proposal-022`, `proposal-024`, `proposal-025`, and `full`, but no `proposal-027`
- Why It Matters: Even with focused proof green, the absence of a standard proposal-scoped gate leaves future signoff non-repeatable and weakens delivery hygiene for this slice.
- Recommended Action: Add a canonical `proposal-027` gate that exercises the renderer contract in a repeatable, repo-owned path.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build/test proof for the proposal slice exists | Pass | Focused `Proposal027Tests` passed `13/13` |
| Shared renderer wired across named surfaces | Pass | Code inspection confirms the four named surfaces compose `ArtifactContentRenderer` |
| Proposal-owned safety constraints proven | Pass | Local-only image policy, JSON rescue, deterministic ordering fallback, and read-only behavior are live |
| Same-tree full regression available and passing | Fail | `./scripts/test-gate.sh full` is unavailable from this host because full UI regression is remote-only |
| Canonical proposal-specific gate exists | Fail | No `proposal-027` lane exists in `scripts/test-gate.sh` |

## Verification Log

- `git status --short`
- `rg -n "ArtifactContentRenderer\\(|MarkdownDocumentView\\(|MarkdownDocumentTextView\\(|JSONTreeDocumentView\\(" 'Chainworks Forge' 'Chainworks ForgeTests'`
- `rg -n "027|ArtifactContentRenderer|artifact-inspector-content|MarkdownDocument|JSONTreeDocument" 'Chainworks ForgeUITests' 'Chainworks ForgeTests'`
- `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -resultBundlePath '/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-tests-20260405-2055.xcresult' test -only-testing:'Chainworks ForgeTests/Proposal027Tests'`
- `./scripts/test-gate.sh full`

## Recommended Next Actions

- Run the exact current tree on an approved remote UI host and execute `./scripts/test-gate.sh full`.
- Add a canonical `proposal-027` lane to `scripts/test-gate.sh` so future audits do not depend on ad hoc focused runs.
