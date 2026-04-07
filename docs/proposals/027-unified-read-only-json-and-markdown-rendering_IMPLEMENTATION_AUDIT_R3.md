# Proposal 027: Unified Read-Only JSON And Markdown Rendering Multi-Lens Audit R3

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` |
| Proposal MD5 | `829a3ab472c3cb6b95a509870d0df882` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Dirty (49 modified, 6 untracked)` |
| Audited At | `2026-04-07T10:34:28+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P027` stays `Partial` / `Not Ready` on the current tree. The proposal’s renderer code is still present and aligned in structure: `ArtifactContentRenderer`, `MarkdownDocumentView`, `MarkdownDocumentTextView`, `JSONTreeDocumentView`, structured JSON rescue, and local-only image handling all remain in code on the intended surfaces. But the fresh same-tree proof basis is red. The targeted `Proposal027Tests` macOS run could not complete because the app target failed to compile first in the shared Goose/runtime path with `Cannot find 'RuntimeStreamEventMapper' in scope`. `P027` still has no canonical `proposal-027` gate in `scripts/test-gate.sh`, and successful roll-up remains impossible because same-tree `full` regression is remote-only from this host and the approved remote host was not reachable from this environment.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Renderer slice is present in code, but fresh same-tree proof is blocked by a shared compile failure | `High` |
| Architecture | `At Risk` | The proposal depends on a globally buildable app target, and current tree is not buildable | `High` |
| Product | `Acceptable` | No new renderer-design contradiction surfaced in code | `Medium` |
| UI | `Acceptable` | Shared renderer still owns the intended artifact/report surfaces | `Medium` |
| UX | `Acceptable` | Local-only fail-closed safety model remains intact in code | `Medium` |
| Readiness | `Not Ready` | No canonical `proposal-027` gate and no same-tree `full` regression proof | `High` |

## Proposal Contract

### Scope

- Replace raw text fallbacks for JSON and Markdown with a unified read-only renderer.
- Render Markdown as documents and JSON as collapsible trees.
- Migrate existing artifact/report/comparison surfaces to the shared renderer.
- Keep artifact truth textual on disk and avoid editing semantics.

### Locked Decisions

- `ArtifactContentRenderer` is the shared entry point.
- Artifact-backed rendering consumes canonical `Artifact.format` truth.
- Markdown uses an AppKit/TextKit-backed document surface.
- Markdown images are local-only and fail-closed in v1.
- JSON preserves source order where possible and otherwise falls back to deterministic sorting.

### Primary User Flows

1. Open a Markdown artifact/report and read it as a document.
2. Open a JSON artifact/report and inspect it as a collapsible tree.
3. Move between artifact inspector, run report, and comparison surfaces without learning different rendering rules.
4. Open malformed or unsafe content without crashing or triggering remote fetch.

### UI Commitments

- Document-grade Markdown hierarchy, code blocks, tables, links, and safe images.
- Disclosure-based JSON tree with bounded default expansion.
- Consistent rendering behavior across named operator surfaces.

### UX Commitments

- Read-only means read-only.
- Malformed content degrades safely.
- Remote image fetch remains disabled in v1.

### Acceptance Criteria

- Shared renderer foundation.
- Primary surfaces use the shared renderer.
- Artifact-backed rendering follows canonical format truth.
- JSON rescue and local-only image safety are preserved.
- No editing affordances are introduced.

### Test / Evidence Requirements

- Focused proof for the shared renderer and migrated surfaces.
- Same-tree successful `full` regression for any successful audit.

### Explicit Exclusions

- No Markdown editing.
- No JSON editing.
- No arbitrary HTML execution.

## Proposal Fidelity / Divergence

### Matches

- `ArtifactContentRenderer`, `MarkdownDocumentView`, `MarkdownDocumentTextView`, `JSONTreeDocumentView`, and `Proposal027Tests` all exist on the current tree.
- Artifact-backed rendering still uses `ArtifactRenderContext.artifactBacked(...)`.
- Structured JSON rescue and local-only image handling remain in the shared renderer code.

### Divergences

- Fresh same-tree targeted proof is red because the app target no longer compiles cleanly.
- There is still no canonical `proposal-027` lane in `scripts/test-gate.sh`.

### Ambiguities / Evidence Gaps

- Same-tree `full` regression could not be run from this host, and the approved remote host was unreachable from this environment.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 8 |
| Partially Implemented | 2 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Shared read-only renderer foundation exists
- Proposal Source: `§5.3`, `§6.1`, `§9 Phase 1`, `§10 AC3`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-found`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-FVgaNs/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
- Gap / Note: The shared renderer exists in code, but fresh same-tree proof did not build because the wider app target failed first.

### REQ-002 Primary artifact/report/comparison surfaces use the shared renderer
- Proposal Source: `§6.5`, `§9 Phase 2`, `§10 AC1-AC3`
- Status: `Partially Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/IdeaListView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-FVgaNs/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
- Gap / Note: The named surfaces still route through the shared renderer in code, but the fresh same-tree proof did not reach execution.

### REQ-003 Artifact-backed surfaces consume canonical `Artifact.format` truth
- Proposal Source: `§6.1`, `§10 AC4`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: `ArtifactRenderContext.artifactBacked(...)` remains the canonical artifact-backed entry point.

### REQ-004 Markdown renders as a document-grade AppKit/TextKit-backed surface
- Proposal Source: `§6.2`, `§7.1`, `§10 AC1`, `§11`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
- Gap / Note: `MarkdownDocumentView` and `MarkdownDocumentTextView` are still present and AppKit/TextKit-backed in the current code.

### REQ-005 JSON renders as a collapsible tree with deterministic fallback ordering
- Proposal Source: `§6.3`, `§7.2`, `§10 AC2`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
- Gap / Note: `JSONTreeDocumentView` remains present in the shared renderer path, and the proposal-owned tests still target the deterministic ordering behavior.

### REQ-006 Declared Markdown/report JSON payloads rescue into structured JSON presentation
- Proposal Source: `§6.1`, `§6.3`, `§10 AC5`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: `StructuredPayloadProbe` is still used to rescue top-level JSON content without mutating format truth.

### REQ-007 Markdown image handling is local-only and fail-closed in v1
- Proposal Source: `§6.2`, `§8`, `§10 AC6`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
- Gap / Note: The current renderer code still constrains image handling to local-only safe paths.

### REQ-008 Malformed Markdown and malformed JSON fall back safely without crash
- Proposal Source: `§8`, `§10 AC7`
- Status: `Implemented`
- Evidence Type: `code`, `tests-found`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: Fallback branches remain present in the renderer code even though fresh execution proof was blocked.

### REQ-009 No editing affordances are introduced
- Proposal Source: `§3`, `§4`, `§10 AC8`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
- Gap / Note: The shared renderer remains read-only in structure and intent.

### REQ-010 Artifact and report truth remain text-first and unchanged on disk
- Proposal Source: `§6.4`, `§10 AC9`
- Status: `Implemented`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Models/Artifact.swift`
  - `Chainworks Forge/Engine/RunReportBuilder.swift`
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
- Gap / Note: The renderer remains presentation-only; it does not rewrite stored artifact/report payloads.

## Architecture Review

**Summary:** `At Risk`

### ARCH-001 Renderer conformance is currently blocked by a wider app-target compile failure
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`, `REQ-002`
- Evidence Type: `tests-run`, `code`
- Evidence:
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-FVgaNs/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
  - `Chainworks Forge/Engine/GooseServerTransport.swift:293`
- Why It Matters: The renderer code may still match the proposal, but the current tree is not buildable enough to prove it. That makes the implementation non-signoff-ready.
- Recommended Action: Fix the shared Goose/runtime compile break first, then rerun `Proposal027Tests`.

## Product Review

**Summary:** `Acceptable`

### PROD-001 No fresh renderer-design contradiction surfaced in the current code
- Severity: `Note`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-003`-`REQ-010`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
- Why It Matters: The proposal’s product direction still looks intact; the active blocker is readiness, not a new design drift.
- Recommended Action: No proposal rewrite is needed before fixing the build/proof path.

## UI Review

**Summary:** `Acceptable`

### UI-001 Shared renderer still owns the intended artifact/report surfaces
- Severity: `Note`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-002`, `REQ-004`, `REQ-005`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
- Why It Matters: Ownership and migration intent remain aligned with the proposal.
- Recommended Action: Revalidate visually after the target compiles again.

## UX Review

**Summary:** `Acceptable`

### UX-001 Local-only fail-closed safety policy remains intact in code
- Severity: `Note`
- Confidence: `Medium`
- Related Proposal Items / Requirements: `REQ-006`, `REQ-007`, `REQ-008`
- Evidence Type: `code`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
- Why It Matters: The proposal’s main safety contract did not regress while the broader runtime transport refactor was happening.
- Recommended Action: No UX contract change is required; proof and readiness are the remaining issues.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Fresh same-tree renderer proof is red
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`, `REQ-002`
- Evidence Type: `tests-run`
- Evidence:
  - focused xcodebuild command above
  - result bundle: `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/p027-audit-FVgaNs/Logs/Test/Test-Chainworks Forge-2026.04.07_10-31-37-+0300.xcresult`
- Why It Matters: The current tree cannot currently produce fresh same-tree evidence that the renderer slice is operational.
- Recommended Action: Fix the shared compile break, then rerun `Proposal027Tests`.

### READY-002 There is still no canonical `proposal-027` gate
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`, `REQ-002`
- Evidence Type: `code`
- Evidence:
  - `scripts/test-gate.sh`
- Why It Matters: Even after the compile break is fixed, the repository still lacks a repeatable proposal-scoped signoff lane for `P027`.
- Recommended Action: Add a canonical `proposal-027` gate after the tree is buildable again.

### READY-003 Same-tree `full` regression is unavailable from this host
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `REQ-001`-`REQ-010`
- Evidence Type: `runtime`
- Evidence:
  - `./scripts/test-gate.sh full`
  - output: `error: UI tests are remote-only and may not run on this host.`
  - `ssh -o BatchMode=yes -o ConnectTimeout=5 test@SMacBook.local 'hostname && pwd'`
  - output: `ssh: Could not resolve hostname smacbook.local`
- Why It Matters: The audit skill forbids a successful verdict without same-tree full regression evidence.
- Recommended Action: Restore approved-host reachability and run same-tree `full` after the current compile break is fixed.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Fail` | Targeted `Proposal027Tests` xcodebuild failed during app compilation |
| Core user flow runtime-validated | `Fail` | Fresh renderer test slice did not execute |
| Empty/loading/error states covered | `Partial` | Static fallback code exists; runtime proof absent |
| Accessibility risk acceptable | `Not Checked` | No fresh runtime UI validation in this pass |
| Localization risk acceptable | `Not Checked` | Out of scope for this pass |
| Critical tests executed | `Partial` | Targeted proof command executed but failed during build |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | `full` unavailable from this host and approved host unreachable |
| Privacy/permissions/entitlements reviewed | `Not Checked` | Not proposal-critical in this pass |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py /Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md`
- `rg -n "struct Proposal027Tests|struct MarkdownDocumentView|struct JSONTreeDocumentView|struct ArtifactContentRenderer|ArtifactRenderContext\\.artifactBacked|StructuredPayloadProbe|MarkdownDocumentTextView|JSONTreeNode\\.build" ...`
- `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath "$DERIVED_DATA" test -only-testing:'Chainworks ForgeTests/Proposal027Tests'`
- `./scripts/test-gate.sh full`
- `ssh -o BatchMode=yes -o ConnectTimeout=5 test@SMacBook.local 'hostname && pwd'`

## Recommended Next Actions

1. Fix the shared `RuntimeStreamEventMapper` compile break so the app target builds again.
2. Rerun `Proposal027Tests` on the same tree to restore fresh renderer proof.
3. Add a canonical `proposal-027` lane to `scripts/test-gate.sh` once the target is buildable.
4. Run same-tree `full` regression on an approved host before attempting a successful audit verdict.
