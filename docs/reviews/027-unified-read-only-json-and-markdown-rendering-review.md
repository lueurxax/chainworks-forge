# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md`
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/project-workspace-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.review/research-pack.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/027-unified-read-only-json-and-markdown-rendering-evidence-pack.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering_IMPLEMENTATION_AUDIT_R1.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline reused:
  - shell-owned ownership of artifact/report/comparison surfaces
  - text-first artifact persistence boundary
  - local artifact/workspace trust boundary
- Baseline refreshed:
  - current `P027` text after the latest edits
  - current shared renderer/code-path map already recorded in the refreshed evidence pack
  - same-day `R2` research conclusions on AppKit/TextKit and JSON ordering semantics
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - targeted code map only; no separate `integration-context.md`
- Targeted context refresh performed: `Yes`
- External research used: `Reused + refreshed`
- Research pack:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.review/research-pack.md`
- Sources reused:
  - same-day `R1` work on local-only image policy and native JSON-tree direction
- Sources refreshed:
  - Apple text-system guidance for AppKit/TextKit document surfaces and text tables
  - RFC guidance on JSON ordering vs canonicalization
- Time-sensitive external guidance:
  - Apple text-system expectations if renderer choice or OS baseline changes
- Code areas inspected:
  - `ArtifactContentRenderer`
  - renderer migration targets already captured in the evidence pack
  - current proposal-owned implementation audit for live local tensions
- Current repo contradictions found:
  - current proposal still leaves the JSON ordering fallback under-specified
  - current proposal still leaves one structural ambiguity in the Markdown backend/display-surface decision
- Runtime evidence used: `None required`
- Provenance of key evidence:
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/027-unified-read-only-json-and-markdown-rendering-evidence-pack.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.review/research-pack.md`
- Remaining assumptions:
  - unified read-only rendering extends existing shell-owned surfaces rather than opening a parallel viewer lane
  - payload-mismatch rescue remains presentation-only and never mutates canonical artifact truth
- Remaining blockers:
  - JSON ordering fallback is still ambiguous
  - Markdown open-decision structure can still be read as making AppKit/TextKit optional

## 1. Executive Summary
- Overall readiness: `Amber`
- Confidence: `High`
- Proposal completeness signal: `Strong but not yet closed`
- What changed since the last local review:
  1. `P027` is materially stronger on Markdown display quality than the older green basis; it now explicitly prefers an AppKit/TextKit-backed document surface and rejects `Text(AttributedString)` as the final quality bar.
  2. same-day deeper research validated that direction and also clarified the JSON standards tension between source-fidelity inspection and canonical deterministic ordering.
  3. that deeper evidence reopened two live proposal-text gaps that the previous green review no longer captures.
- What still blocks `Green`:
  1. `§6.3` still says “stable key ordering based on source order where possible” without defining the fallback when source order is unavailable through the chosen parser.
  2. `§11` still mixes parser choices and display-surface choice in one `or` list, which can be read as making AppKit/TextKit optional rather than the preferred v1 display class.

## 2. Proposal Scope and Completeness
- In scope:
  - unified read-only rendering for Markdown and JSON
  - shared rendering entry point
  - payload-mismatch rescue into JSON presentation
  - migration of artifact/report/comparison surfaces
  - document-grade Markdown display and tree-grade JSON display
- Out of scope:
  - editing
  - WYSIWYG authoring
  - schema-aware JSON forms
  - arbitrary HTML execution
- Deferred intentionally:
  - outline navigation
  - search / copy-by-path
  - inline editing
- Most important confirmations against current repo:
  - shared rendering still remains subordinate to canonical `Artifact.format` truth
  - local-only fail-closed image policy still matches the host-system trust boundary
  - the stronger AppKit/TextKit direction is now aligned with primary Apple guidance
- Most important remaining incompleteness:
  - JSON ordering contract still does not tell implementation what to do when source order cannot be preserved
  - Open Decisions still structurally blur parser choice and display-surface choice

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Amber | High | Complete | 0 | 0 | 1 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Amber | High | Complete | 0 | 0 | 1 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings

#### UI-001 Markdown backend and display-surface decision is still structurally ambiguous
- Severity: `Medium`
- Confidence: `High`
- Evidence IDs:
  - `DOC-01`
  - `MAP-02`
  - `REAL-01`
  - `APP-01`
  - `APP-02`
- Why it matters:
  - `§6.2` correctly strengthens the proposal toward an AppKit/TextKit-backed document viewer, but `§11` still lists parser choices and display-surface choice in one `or` group. That wording can still be read as “pick any one of these,” which reopens the exact weak-path ambiguity the proposal is trying to close.
- Fix:
  - Separate parser choice from display-surface choice explicitly.
  - Example shape:
    - choose one parser/backend
    - and use a document-grade AppKit/TextKit-backed display surface for v1
- Acceptance signal:
  - a reader can no longer interpret `Text(AttributedString)` or another lightweight surface as an equally acceptable final v1 path.

### 5.2 UX Findings
- No live UX findings in the current reread.

### 5.3 iOS Architecture Findings

#### ARCH-001 JSON ordering contract still lacks an explicit fallback rule
- Severity: `Medium`
- Confidence: `High`
- Evidence IDs:
  - `DOC-01`
  - `MAP-03`
  - `INT-03`
  - `REAL-02`
  - `F-ARCH-01`
  - `F-ARCH-02`
  - `APP-03`
- Why it matters:
  - `§6.3` says key ordering should follow source order where possible, but RFC 8259 treats object members as unordered and the current repo implementation uses a generic parsed dictionary path. Without an explicit fallback, the proposal still leaves implementation free to choose between source-preserving inspection, parser-native order, or deterministic sorting, and each choice leads to different UI behavior.
- Fix:
  - Split the ordering rule into two steps:
    1. preserve source-member order when the chosen parser/representation exposes it
    2. otherwise apply one documented deterministic fallback order
  - If canonical deterministic ordering is desired for some surfaces, say that separately rather than implying it inside the source-order rule.
- Acceptance signal:
  - two different implementers would produce the same JSON ordering behavior even when source-token order is unavailable.

## 6. Cross-Discipline Conflicts and Decisions
- The earlier format-authority and image/source-policy conflicts remain closed.
  Decision:
  current text still correctly keeps artifact-backed rendering subordinate to canonical format truth and local-only trust boundaries.

- The current live tension is no longer about broad renderer direction.
  Decision:
  the remaining issues are narrow contract-shape problems:
  one around JSON ordering fallback, one around how the Markdown display-surface choice is expressed.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P1 | Rewrite `§6.3` ordering language to distinguish source-preserving inspection from deterministic fallback behavior | iOS Architecture | Proposal text | Before next review | `APP-03` | JSON ordering behavior is unambiguous even when source order is unavailable | `ARCH-001` |
| P1 | Split parser choice and display-surface choice in `§11` so AppKit/TextKit is not read as optional | UI / Architecture | Proposal text | Before next review | `APP-01`, `APP-02` | v1 Markdown display class is unambiguous to implementers | `UI-001` |
| P3 | Optionally name the preferred native hierarchy primitive more explicitly in `§6.3` | iOS Architecture | Proposal text | Optional polish | research reuse only | implementation handoff becomes slightly more concrete | prior same-day research |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Markdown display class | final renderer actually uses a document-grade AppKit/TextKit surface | no final design path relies on plain `Text(AttributedString)` | document-quality bar stays explicit across all migrated surfaces | next proposal reread and later implementation audit | hold if final text still leaves lightweight text surfaces as equally valid |
| JSON ordering | viewer ordering stays predictable and reviewable | proposal names source-preserving rule and fallback rule separately | no silent drift between source-order and sorted-order expectations | next proposal reread | hold if two reasonable implementations can still diverge |
| Trust boundary | image/source policy remains local-only and fail-closed | no new remote-fetch language appears | shell-owned local artifact/workspace boundary stays intact | implementation audit | hold if remote image loading reappears without a separate proposal |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: No blocking evidence gaps remain for proposal-readiness. The remaining issues are proposal-text ambiguities, not missing local context.

### Open Questions
- `QUESTION-01`: Does the team want source-order fidelity whenever technically available, or should some JSON surfaces explicitly prefer deterministic canonical sorting instead?
- `QUESTION-02`: Should `§11` name `NSTextView` explicitly, or is “AppKit/TextKit-backed document surface” the preferred abstraction level?
