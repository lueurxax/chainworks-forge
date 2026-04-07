# Proposal 027: Unified Read-Only JSON And Markdown Rendering Multi-Lens Audit R5

| Field | Value |
|---|---|
| Proposal | `docs/proposals/027-unified-read-only-json-and-markdown-rendering.md` |
| Proposal MD5 | `829a3ab472c3cb6b95a509870d0df882` |
| Proposal State | `Active` |
| Platform Scope | `macOS` |
| Repository Root | `/Users/user/Documents/Chainworks Forge` |
| Git SHA | `d8ccf4b` |
| Working Tree | `Dirty` |
| Audited At | `2026-04-07T12:13:50+0300` |
| Overall Conformance | `Partial` |
| Overall Readiness | `Not Ready` |
| Audit Confidence | `High` |

## Executive Verdict

The old gate-readiness blocker from `R4` is closed: `scripts/test-gate.sh` now includes a canonical `proposal-027` lane. The audit is still not green because fresh same-tree execution proof is currently blocked by a broader regression envelope. Both the fresh local `proposal-025` gate and the fresh synced approved-host `proposal-015` gate fail while compiling the shared `Chainworks ForgeTests` target, specifically in `Proposal026Tests.swift`. `proposal-027` uses that same test target with `PROPOSAL_027_TESTS=("Chainworks ForgeTests/Proposal027Tests")`, so it cannot currently produce a trustworthy same-tree pass until the shared compile drift is cleared. The renderer implementation remains present in code, but proof-readiness is still `Partial` / `Not Ready`.

## Lens Scorecard

| Lens | Assessment | Top Risk | Confidence |
|---|---|---|---|
| Conformance | `Partial` | Fresh proposal-owned proof is blocked by shared test-target regression | `High` |
| Architecture | `Acceptable` | Shared renderer ownership remains coherent in code | `High` |
| Product | `Acceptable` | No fresh renderer-specific contradiction surfaced | `High` |
| UI | `Acceptable` | Shared artifact/report/comparison rendering surfaces remain in place | `Medium` |
| UX | `Acceptable` | Local-only fail-closed rendering model remains intact in code | `Medium` |
| Readiness | `Not Ready` | Canonical gate now exists, but same-tree proof is still blocked before execution | `High` |

## Proposal Contract

### Scope

- Replace raw text fallbacks for JSON and Markdown with a unified read-only renderer.
- Render Markdown as documents and JSON as collapsible trees.
- Migrate artifact, report, and comparison readers to the shared renderer.
- Keep artifact truth textual on disk and avoid editing semantics.

### Locked Decisions

- `ArtifactContentRenderer` is the shared entry point.
- Artifact-backed rendering consumes canonical `Artifact.format` truth.
- Markdown uses an AppKit/TextKit-backed document surface.
- Markdown images are local-only and fail-closed in v1.
- JSON preserves source order where possible and otherwise falls back deterministically.

### Acceptance Criteria

- Shared renderer foundation exists.
- Primary surfaces use the shared renderer.
- Artifact-backed rendering follows canonical format truth.
- JSON rescue and local-only image safety are preserved.
- No editing affordances are introduced.
- Canonical same-tree `proposal-027` gate passes.

## Proposal Fidelity / Divergence

### Matches

- `ArtifactContentRenderer`, `MarkdownDocumentView`, `MarkdownDocumentTextView`, and `JSONTreeDocumentView` remain present on the current tree.
- The old missing-gate blocker is closed: `test-gate.sh` now declares and wires `proposal-027`.
- No fresh code-level contradiction surfaced against the renderer contract.

### Divergences

- Fresh same-tree renderer proof did not complete in this pass because the shared `Chainworks ForgeTests` target is currently red in `Proposal026Tests.swift`.
- A successful same-tree execution pass for `Proposal027Tests` is therefore unavailable on the current tree.

### Explicit Inference

The blocking relationship above is an inference from the current build graph, not a separate completed `proposal-027` result bundle. I am inferring it because:

- `scripts/test-gate.sh` wires `proposal-027` through `run_targeted_tests "proposal-027" "${PROPOSAL_027_TESTS[@]}"`.
- `PROPOSAL_027_TESTS` contains `Chainworks ForgeTests/Proposal027Tests`.
- fresh same-tree local and approved-host runs already prove that the shared `Chainworks ForgeTests` target compile-fails in `Proposal026Tests.swift` before test execution begins.

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
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks Forge/Views/RunReportView.swift`
  - `Chainworks Forge/Views/RunComparisonView.swift`
  - fresh same-tree gate builds in this pass compiled the renderer and consuming surfaces
- Gap / Note: The implementation contract remains present; proof execution is blocked elsewhere.

### REQ-002 Artifact-backed rendering follows canonical format truth and rescues structured JSON correctly
- Proposal Source: `§6.1`, `§6.3`, `§10 AC4-AC5`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
  - fresh same-tree local/remote gates compiled these files on the current tree
- Gap / Note: No fresh code-level contradiction surfaced against format-truth or JSON rescue behavior.

### REQ-003 Markdown uses a document-grade native surface
- Proposal Source: `§6.2`, `§7.1`, `§10 AC1`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: The AppKit/TextKit-backed path remains present in code; the current blocker is test-target compilation, not renderer design.

### REQ-004 JSON renders as a collapsible tree with deterministic fallback behavior
- Proposal Source: `§6.3`, `§7.2`, `§10 AC2`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: No fresh code-level contradiction surfaced against the JSON tree/fallback contract.

### REQ-005 Markdown image handling is local-only and fail-closed
- Proposal Source: `§6.2`, `§8`, `§10 AC6`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: The local-only image/source policy remains explicit in the implementation.

### REQ-006 Rendering remains read-only and presentation-only
- Proposal Source: `§3`, `§4`, `§6.4`, `§10 AC8-AC9`
- Status: `Implemented`
- Evidence Type: `code`, `build-run`
- Evidence:
  - `Chainworks Forge/Views/ArtifactContentRenderer.swift`
  - `Chainworks Forge/Views/ArtifactInspectorView.swift`
  - `Chainworks ForgeTests/Proposal027Tests.swift`
- Gap / Note: No fresh code-level contradiction surfaced against the read-only contract.

## Architecture Review

**Summary:** `Acceptable`

No new architecture finding surfaced. The shared renderer still centralizes read-only JSON/Markdown handling under one owner, and the prior missing-gate readiness gap is closed.

## Product Review

**Summary:** `Acceptable`

No fresh renderer-specific behavior contradiction surfaced in this pass. The active blocker is cross-cutting test-target health, not a newly discovered P027 contract gap.

## Delivery / Readiness Review

**Summary:** `Not Ready`

### READY-001 Canonical `proposal-027` gate now exists, but fresh proof is blocked by shared compile drift
- Severity: `Critical`
- Confidence: `High`
- Related Proposal Items / Requirements: `Acceptance Criteria`, `Test / Evidence Requirements`
- Evidence Type: `code`, `tests-run`, `inference-from-tests-run`
- Evidence:
  - `scripts/test-gate.sh:1196-1205`
  - `scripts/test-gate.sh:119-121`
  - local same-tree canonical failure:
    `/var/folders/fj/v77kf6rs4dz1ybsm1_1_qhb00000gn/T/chainworks-test-gates/proposal-025-20260407-120343.xcresult`
  - synced approved-host same-tree canonical failure:
    `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-015-non-ui-20260407-121208.xcresult`
  - common failure text:
    `No exact matches in call to initializer`
    `Cannot infer contextual base in reference to member 'empty'`
    `Cannot infer contextual base in reference to member 'operatorGrade'`
    `Cannot infer contextual base in reference to member 'legacyOperatorGrade'`
  - common failing compile unit:
    `Chainworks ForgeTests/Proposal026Tests.swift`
- Why It Matters: `P027` can no longer be blocked on missing gate wiring, but it still cannot produce fresh same-tree proof while the shared test target is red.
- Recommended Action: Repair the shared `Proposal026Tests.swift` compile drift, then rerun `./scripts/test-gate.sh proposal-027`.

### READY-002 Same-tree full regression was not attempted after the focused proof envelope was already red
- Severity: `Medium`
- Confidence: `High`
- Related Proposal Items / Requirements: `Test / Evidence Requirements`
- Evidence Type: `audit-policy`
- Evidence:
  - shared canonical proof envelope above is already red on the current tree
- Why It Matters: A successful roll-up is already impossible before considering `full`.
- Recommended Action: Clear the focused proof blocker first; only then spend time on full-regression roll-up.

## Readiness Checklist

| Check | Status | Evidence / Note |
|---|---|---|
| Build succeeds on targeted platform(s) | `Pass` | Fresh same-tree gates compiled the renderer implementation on the current tree. |
| Core user flow runtime-validated | `Fail` | No fresh successful `Proposal027Tests` execution exists in this pass. |
| Empty/loading/error states covered | `Not Checked` | The gate did not reach fresh renderer assertions. |
| Critical tests executed | `Fail` | Shared compile drift prevents the proposal-owned test slice from running. |
| Full regression suite passed on same tree/HEAD | `Not Run` | Not attempted after the focused proof envelope was already red. |

## Verification Log

- `git rev-parse --short HEAD`
- `git status --short`
- `nl -ba 'scripts/test-gate.sh' | sed -n '960,1015p'`
- `rg -n "proposal-027|proposal-025|proposal-015" 'scripts/test-gate.sh'`
- `./scripts/test-gate.sh proposal-025`
- `ssh -o BatchMode=yes test@SMacBook.local 'hostname'`
- `rsync -az --delete --exclude='.git' --exclude='DerivedData' --exclude='.build' --exclude='*.xcresult' '/Users/user/Documents/Chainworks Forge/' 'test@SMacBook.local:/Users/test/chainworks-remote/'`
- `ssh -o BatchMode=yes test@SMacBook.local 'cd /Users/test/chainworks-remote && ./scripts/test-gate.sh proposal-015'`

## Recommended Next Actions

1. Fix the shared `Proposal026Tests.swift` compile drift that currently blocks all same-tree proof lanes using `Chainworks ForgeTests`.
2. Rerun `./scripts/test-gate.sh proposal-027` once the shared test target is green.
3. Only after a fresh `proposal-027` pass, reassess whether a same-tree `full` regression run is needed for a successful audit roll-up.
