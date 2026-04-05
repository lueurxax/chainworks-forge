# Consolidated Proposal Review

## 0. Review Mode and Proposal Evidence Summary
- Mode used: `proposal-readiness`
- Evidence completeness: `Complete`
- Proposal / docs reviewed:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/operator-experience.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/live-provider-execution-slice.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/domain-model.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/project-workspace-contract.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/runtime-contract.md`
- Reusable baseline used:
  - `/Users/user/Documents/Chainworks Forge/.review-baselines/current-system-baseline.md`
  - `/Users/user/Documents/Chainworks Forge/docs/reference/current-system-baseline.md`
- Baseline reused:
  - operator-shell ownership for artifact inspection, reports, and comparison
  - artifact-on-disk / read-only rendering assumptions
  - workspace and artifact-root boundary assumptions
- Baseline refreshed:
  - current artifact rendering code in `ArtifactInspectorView`
  - current workflow artifact preview in `WorkflowArtifactInspectorView`
  - current `RunReportView` and `RunComparisonView` raw-content paths
  - current artifact format detection and persistence contract
- Baseline freshness: `Partially refreshed`
- Proposal-specific integration context:
  - targeted code map only; no separate `integration-context.md`
- Targeted context refresh performed: `Yes`
- External research used: `Reused`
- Research pack:
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.review/research-pack.md`
- Sources reused:
  - stable refs plus current code paths
  - same-day Apple/CommonMark primary-source research from `R1`
- Sources refreshed:
  - proposal text against current local blockers
- Time-sensitive external guidance:
  - Apple Markdown API behavior if the app changes OS baseline or renderer choice
- Code areas inspected:
  - artifact inspector rendering
  - workflow artifact preview rendering
  - run report summary/history rendering
  - run comparison resolved-skill content rendering
  - artifact format detection and persistence
  - current UI proof-owning artifact inspector tests
- Current repo contradictions found:
  - none that remain live in the current proposal text
- Runtime evidence used: `None`
- Provenance of key evidence:
  - `/Users/user/Documents/Chainworks Forge/docs/reviews/027-unified-read-only-json-and-markdown-rendering-evidence-pack.md`
  - `/Users/user/Documents/Chainworks Forge/docs/proposals/027-unified-read-only-json-and-markdown-rendering.review/research-pack.md`
- Remaining assumptions:
  - unified read-only rendering extends existing artifact/report/comparison surfaces rather than opening a parallel viewer lane
  - `P027` is intentionally presentation-only and does not change artifact persistence or output contracts
- Remaining blockers:
  - none

## 1. Executive Summary
- Overall readiness: `Green`
- Confidence: `High`
- Proposal completeness signal: `Strong`
- What changed since the last local review:
  1. `§6.1` now clearly anchors artifact-backed rendering to canonical `Artifact.format` / `ArtifactFormat.detect(...)`.
  2. `§6.2`, `§8`, `§10`, and `§11` now make Markdown image handling fail-closed and local-only in v1.
  3. the same-day research pack supports the current native-rendering direction instead of weakening it.
- Residual non-blocking hygiene:
  1. `§6.3` could optionally name `OutlineGroup` / recursive `DisclosureGroup` as the preferred implementation shape.
  2. `§6.2` could optionally mention Apple presentation-intent semantics more explicitly.
  3. raw HTML could optionally be called out as text-fallback-only in v1.

## 2. Proposal Scope and Completeness
- In scope:
  - unified read-only rendering for Markdown and JSON
  - shared rendering entry point
  - migration of artifact/report/comparison surfaces
  - collapsible JSON tree viewer
  - proper Markdown document rendering
- Out of scope:
  - editing
  - WYSIWYG authoring
  - schema-aware JSON forms
  - arbitrary HTML execution
- Deferred intentionally:
  - outline navigation
  - search / copy-by-path
  - inline editing
- Most important baseline refreshes performed:
  - verified current artifact/report/comparison surfaces still use divergent local rendering logic
  - verified current artifact format truth is already persisted and centrally detected
  - verified current artifact surfaces are local file/workspace readers with shell-owned entry points
- Most important confirmations against current repo:
  - shared rendering is now subordinate to current artifact format authority
  - image/source safety is now explicitly local-only and fail-closed
  - artifact truth remains text-first on disk and presentation-only in the UI
- Most important non-blocking follow-ups:
  - implementation notes could be slightly more concrete about native hierarchy primitives and Markdown semantic styling

## 4. Discipline Scorecard
| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Green | High | Complete | 0 | 0 | 0 | 0 |
| UX | Green | High | Complete | 0 | 0 | 0 | 0 |
| iOS Architecture | Green | High | Complete | 0 | 0 | 0 | 0 |

## 5. Findings by Discipline

### 5.1 UI Findings
- No live UI findings in the current reread.

### 5.2 UX Findings
- No live UX findings in the current reread.

### 5.3 iOS Architecture Findings
- No live architecture findings in the current reread.

## 6. Cross-Discipline Conflicts and Decisions
- The previous format-authority conflict is closed.
  Decision:
  artifact-backed surfaces now clearly inherit canonical format truth from `Artifact.format` / `ArtifactFormat.detect(...)`, while non-artifact content remains an explicit narrow exception.

- The previous image/source-trust conflict is closed.
  Decision:
  v1 is now explicitly local-only and fail-closed, with remote image loading out of scope unless a future proposal introduces a new source-trust policy.

## 7. Prioritized Action Backlog
| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source Findings |
|---|---|---|---|---|---|---|---|
| P3 | Optionally name native hierarchical `List` / `OutlineGroup` / recursive `DisclosureGroup` as the preferred JSON-tree implementation shape | iOS Architecture | Proposal text | Optional pre-handoff polish | Research reuse only | Implementation handoff becomes more concrete without changing scope | `APP-01` |
| P3 | Optionally mention Apple presentation-intent semantics in the Markdown implementation note | UI / Architecture | Proposal text | Optional pre-handoff polish | Research reuse only | Native Markdown direction becomes easier to audit later | `APP-02` |
| P3 | Optionally spell out raw HTML as text-fallback-only in v1 | UX / Architecture | Proposal text | Optional pre-handoff polish | Research reuse only | Safety expectations become slightly crisper | `APP-03` |

## 8. Validation and Measurement Plan
| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Rollback / Hold Criteria |
|---|---|---|---|---|---|
| Format authority | Shared renderer consumes canonical artifact format instead of screen-local guesses | artifact-backed viewers pass persisted format metadata | No new content sniffers appear in migrated surfaces | Implementation audit | Hold if format truth can differ by surface |
| Image/source safety | Markdown image support stays within the local-only policy | renderer rejects or badges unsupported remote images | No silent network/document fetch path appears in read-only artifact surfaces | Implementation audit | Hold if remote or unsafe sources load without an explicit approved policy |
| Surface unification | Artifact/report/comparison surfaces converge on one renderer without losing shell ownership | duplicate pretty-print / raw markdown branches shrink | No parallel viewer lane is introduced | Implementation audit | Hold if migration creates a second inspector/report pathway |

## 9. Evidence Gaps and Open Questions

### Evidence Gaps
- `GAP-01`: No blocking evidence gaps remain for proposal-readiness. Current docs, code, baseline slices, and reused research are enough to judge the proposal text.

### Open Questions
- `QUESTION-01`: Does the team want `§6.3` to name a preferred native hierarchy primitive explicitly, or is the current typed JSON-tree contract sufficient?
- `QUESTION-02`: Does the team want to make raw HTML fallback explicit in proposal text now, or leave that to implementation notes and audit?
