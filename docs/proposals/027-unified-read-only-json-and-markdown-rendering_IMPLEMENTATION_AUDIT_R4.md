# Proposal 027: Unified Read-Only JSON And Markdown Rendering Multi-Lens Audit R4

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` |
| Proposal MD5 | `829a3ab472c3cb6b95a509870d0df882` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `a0bb075` |
| Working Tree | `Clean` |
| Audited At | `2026-04-07T11:30:43+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

`P027` no longer has the stale compile blocker from the prior audit basis. The fresh same-tree focused renderer slice is green: `Proposal027Tests` passed `17/17`, and the current code still carries the intended shared renderer, native Markdown document path, JSON tree path, structured JSON rescue, and local-only image safety model. The audit still cannot roll up green because the proposal lacks a canonical `proposal-027` gate in `scripts/test-gate.sh`, and the synced approved-host `full` gate is currently unavailable: it aborts with `signing_args[@]: unbound variable` before launching regression. Under the updated audit skill, that keeps `P027` at `Partial` / `Not Ready` even though the in-scope proposal slice itself is green.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | In-scope renderer proof is green, but no same-tree full regression proof exists | `High` |
| Architecture | `Acceptable` | Shared renderer ownership remains coherent | `High` |
| Product | `Acceptable` | No fresh behavior contradiction surfaced in the renderer contract | `High` |
| UI | `Acceptable` | Shared read-only surfaces remain aligned with the proposal | `Medium` |
| UX | `Acceptable` | Local-only fail-closed document behavior remains intact | `Medium` |
| Readiness | `Not Ready` | Missing canonical gate and unavailable same-tree full regression block sign-off | `High` |

## Proposal Contract

### Scope

- Replace raw text fallbacks for JSON and Markdown with a unified read-only renderer.
- Render Markdown as documents and JSON as collapsible trees.
- Migrate existing artifact/report/comparison readers to the shared renderer.
- Keep artifact truth textual on disk and avoid editing semantics.

### Locked Decisions

- `ArtifactContentRenderer` is the shared entry point.
- Artifact-backed rendering consumes canonical `Artifact.format` truth.
- Markdown uses an AppKit/TextKit-backed document surface.
- Markdown images are local-only and fail-closed in v1.
- JSON preserves source order where possible and otherwise falls back deterministically.

### Primary User Flows

1. Open a Markdown artifact/report and read it as a document.
2. Open a JSON artifact/report and inspect it as a collapsible tree.
3. Move between artifact inspector, report, and comparison surfaces with one renderer contract.
4. Open malformed or unsafe content without crashing or remote fetch.

### UI Commitments

- Document-grade Markdown hierarchy, code blocks, tables, links, and safe images.
- Disclosure-based JSON tree with bounded default expansion.
- Consistent rendering behavior across named operator surfaces.

### UX Commitments

- Read-only means read-only.
- Malformed content degrades safely.
- Remote image fetch remains disabled in v1.

### Acceptance Criteria

- Shared renderer foundation exists.
- Primary surfaces use the shared renderer.
- Artifact-backed rendering follows canonical format truth.
- JSON rescue and local-only image safety are preserved.
- No editing affordances are introduced.

### Test / Evidence Requirements

- Focused proof for the shared renderer and migrated surfaces.
- Passing same-tree `full` regression for any successful audit.

### Explicit Exclusions

- No Markdown editing.
- No JSON editing.
- No arbitrary HTML execution.

## Proposal Fidelity / Divergence

### Matches

- `ArtifactContentRenderer`, `MarkdownDocumentView`, `MarkdownDocumentTextView`, `JSONTreeDocumentView`, and `Proposal027Tests` remain present and wired.
- Fresh same-tree focused proof is green at `17/17`.
- Local-only image handling and structured JSON rescue remain intact in code and tests.

### Divergences

- There is still no canonical `proposal-027` lane in `scripts/test-gate.sh`.
- Same-tree full regression proof is still unavailable because the canonical `full` gate aborts before launching `xcodebuild`.

### Ambiguities / Evidence Gaps

- No executed same-tree full regression exists for the current clean tree.

## Requirement Summary

| Status | Count |
|---|---:|
| Implemented | 6 |
| Partially Implemented | 0 |
| Missing | 0 |
| Not Verifiable | 0 |

## Requirement Audit

### REQ-001 Shared read-only renderer foundation exists and is used by primary artifact/report surfaces
- Proposal Source: `§5.3`, `§6.1`, `§6.5`, `§10 AC1-AC3`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - local result bundle: `/tmp/p027-audit.St5YQL/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-09-+0300.xcresult`
- Gap / Note: The current same-tree focused renderer slice passed `17/17`.

### REQ-002 Artifact-backed rendering follows canonical format truth and rescues structured JSON correctly
- Proposal Source: `§6.1`, `§6.3`, `§10 AC4-AC5`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - local result bundle: `/tmp/p027-audit.St5YQL/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-09-+0300.xcresult`
- Gap / Note: Focused proof covers artifact-backed format handling plus Markdown/report JSON rescue.

### REQ-003 Markdown uses a document-grade native surface
- Proposal Source: `§6.2`, `§7.1`, `§10 AC1`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - local result bundle: `/tmp/p027-audit.St5YQL/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-09-+0300.xcresult`
- Gap / Note: The AppKit/TextKit-backed Markdown path is live in code and covered by the green focused slice.

### REQ-004 JSON renders as a collapsible tree with deterministic fallback behavior
- Proposal Source: `§6.3`, `§7.2`, `§10 AC2`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - local result bundle: `/tmp/p027-audit.St5YQL/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-09-+0300.xcresult`
- Gap / Note: The green focused slice covers JSON summaries and ordering fallback behavior.

### REQ-005 Markdown image handling is local-only and fail-closed
- Proposal Source: `§6.2`, `§8`, `§10 AC6`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - local result bundle: `/tmp/p027-audit.St5YQL/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-09-+0300.xcresult`
- Gap / Note: The focused slice explicitly covers local absolute, workspace-relative, remote-rejected, and out-of-bound-rejected image sources.

### REQ-006 Rendering remains read-only and presentation-only
- Proposal Source: `§3`, `§4`, `§6.4`, `§10 AC8-AC9`
- Status: `Implemented`
- Evidence Type: `code`, `tests-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - local result bundle: `/tmp/p027-audit.St5YQL/Logs/Test/Test-Chainworks Forge-2026.04.07_11-22-09-+0300.xcresult`
- Gap / Note: No fresh evidence reopened editing semantics or on-disk mutation risk.

## Architecture Review

**Summary:** `Acceptable`

No new architecture finding surfaced. The shared renderer still centralizes JSON/Markdown rendering under one owner, and the focused slice is green on the current tree.

## Product Review

**Summary:** `Acceptable`

No fresh product contradiction surfaced. The renderer behavior promised by `P027` is present and passing in focused proof.

## UI Review

**Summary:** `Acceptable`

No fresh UI contradiction surfaced. The intended artifact/report/comparison surfaces still route through the shared renderer.

## UX Review

**Summary:** `Acceptable`

No fresh UX contradiction surfaced. The local-only fail-closed model remains intact and the renderer remains read-only.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 `P027` still has no canonical proposal gate in `test-gate.sh`
- Severity: `Major`
- Confidence: `High`
- Related Proposal Items / Requirements: `Test / Evidence Requirements`
- Evidence Type: `code`
- Evidence:
  - `scripts/test-gate.sh:968-977`
  - usage list contains `proposal-015`, `proposal-025`, and `full`, but no `proposal-027`
- Why It Matters: The implementation may be correct, but the repo still lacks the proposal-scoped reproducible proof lane expected for audit/readiness work.
- Recommended Action: Add a canonical `proposal-027` gate that runs the focused renderer slice.

### READY-002 Same-tree full regression is unavailable, so a successful audit roll-up is forbidden
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `Test / Evidence Requirements`
- Evidence Type: `runtime`
- Evidence:
  - synced approved-host invocation: `./scripts/test-gate.sh full`
  - output: `./scripts/test-gate.sh: line 563: signing_args[@]: unbound variable`
  - local script reference: `scripts/test-gate.sh:562-568`
- Why It Matters: The updated audit skill requires passing same-tree full regression for any successful verdict. That proof is currently unavailable on the clean synced tree.
- Recommended Action: Fix the canonical `full` gate and rerun it on the same synced tree before attempting another green audit.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Local focused renderer slice built and passed. |
| Core user flow runtime-validated | `Pass` | `Proposal027Tests` passed `17/17` on the current tree. |
| Empty/loading/error states covered | `Pass` | Focused slice covers malformed content, JSON rescue, and local-only image policy. |
| Accessibility risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Localization risk acceptable | `Not Checked` | Not reassessed in this pass. |
| Critical tests executed | `Pass` | Local focused renderer result bundle is green. |
| Full regression suite / canonical full gate passed on same tree/HEAD | `Fail` | Synced approved-host `full` gate aborts before regression starts. |
| Privacy/permissions/entitlements reviewed | `Pass` | Local-only image/source policy remains fail-closed in the focused proof. |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `python3 /Users/user/.agents/skills/proposal-implementation-audit/scripts/report_path.py '/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md'`
- `xcodebuild -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p027-audit.St5YQL test -only-testing:'Chainworks ForgeTests/Proposal027Tests'`
- `rg -n "proposal-027|proposal-015|proposal-025|full\\)" scripts/test-gate.sh`
- `rsync -az --delete --exclude='.git' --exclude='DerivedData' --exclude='.build' --exclude='*.xcresult' '/Users/user/Documents/Chainworks Forge/' 'test@SMacBook.local:/Users/test/chainworks-remote/'`
- `ssh test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh full'`

## Recommended Next Actions

1. Add a canonical `proposal-027` gate to `scripts/test-gate.sh`.
2. Fix the `full` gate shell expansion bug so same-tree regression can actually run.
3. After the gate fixes, rerun synced approved-host `full` to unlock a successful audit roll-up.
