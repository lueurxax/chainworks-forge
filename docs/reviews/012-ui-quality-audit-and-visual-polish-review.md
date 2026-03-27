# Proposal 012: UI Quality Audit and Visual Polish Review

| Field | Value |
|---|---|
| Proposal | `docs/proposals/012-ui-quality-audit-and-visual-polish.md` |
| Repository Root | `.` |
| Git SHA | `63f5270` |
| Reviewed At | `2026-03-26T23:31:00+0200` |
| Review Mode | `full-review` |
| Product Overlay | `omitted` |
| Overall Status | `Evidence Gap Review` |
| Readiness | `Red` |
| Confidence | `High` |
| Evidence Completeness | `Partial` |

## 0. Review Mode and Evidence Summary

- Mode used: `full-review`
- Evidence completeness: `Partial`
- Documents / repo inputs reviewed:
  - `docs/proposals/012-ui-quality-audit-and-visual-polish.md`
  - current `Views/` SwiftUI surfaces cited by the proposal
- Freshness check:
  - the proposal was read against current `HEAD`
  - no prior written Proposal 012 review existed in `docs/reviews/`
  - current-round build and targeted UI evidence were attempted fresh
- Build/run attempts used in this round:
  - fresh `xcodebuild -project 'Chainworks Forge.xcodeproj' -scheme 'Chainworks Forge' -destination 'platform=macOS' -derivedDataPath /tmp/p012-build-dd -resultBundlePath /tmp/p012-build.xcresult build` passed
  - fresh targeted UI slice attempted at [`/tmp/p012-ui.xcresult`](/tmp/p012-ui.xcresult), but the runner failed before product proof with `Timed out while enabling automation mode`
- Screenshots / attachments in scope:
  - no authoritative current-round UI screenshots were produced because the macOS UI runner failed to initialize
- Code areas inspected:
  - `ContentView`
  - `RunsHomeView`
  - `IdeaListView`
  - `ProviderSettingsView`
  - `PilotReadinessView`
  - `FirstRunSetupWizard`
  - `ArchivedIdeasView` / `IdeasArchiveView`
  - `GooseProviderConnectionAssistantView`
  - `ReleaseGateView`
  - `WorkflowMapView`
  - `DeliveryPreflightReportView`

## 1. Executive Summary

- Overall readiness: `Red`
- Confidence: `High`
- Remaining blockers to handoff:
  1. the dependency table understates the real baseline: the proposal audits and intends to polish surfaces delivered by Proposals 006, 010, and 011, but the draft still claims dependency only on 007 and 008
  2. the audit methodology and scope counts are stale and internally inconsistent, so the proposal currently overclaims review coverage
  3. at least one catalogued issue (`L-11`) is already closed on the correct presentation layer and is assigned to the wrong owner file
- Top risks:
  1. implementation can waste time polishing already-closed issues instead of the remaining live defects
  2. future readers can mistake the current issue catalogue for a complete, current-state audit when its inventory is already out of date
  3. proposal sequencing can break if 012 is read as runnable before the surfaces from 006/010/011 are guaranteed

Verdict: Proposal 012 has a useful direction, but the current draft is not yet trustworthy as a complete UI audit artifact. The current round also lacks fresh product screenshots because the targeted macOS UI runner failed before automation mode initialized.

## 2. Discipline Scorecard

| Discipline | Readiness | Confidence | Evidence Completeness | Critical | High | Medium | Low |
|---|---|---|---|---:|---:|---:|---:|
| UI | Red | High | Partial | 0 | 1 | 1 | 0 |
| UX | Yellow | Medium | Partial | 0 | 0 | 1 | 0 |
| iOS Architecture | Red | High | Partial | 0 | 1 | 0 | 0 |

## 3. Findings by Discipline

### UI

#### Finding UI-01 — Audit inventory is stale and internally inconsistent

- Severity: `High`
- Confidence: `High`
- Location: `docs/proposals/012-ui-quality-audit-and-visual-polish.md:26-33`
- Why it matters:
  - the proposal claims a systematic preview audit of `all 12 previewable surfaces` and `all 30 view files in Views/`
  - current `HEAD` does not match that inventory
  - repo search in this round found `14` named `#Preview("...")` definitions, `15` total `#Preview` blocks, and `28` Swift files under `Views/`
  - line `33` also contradicts line `9`: it says `30` files in `Views/`, plus `ContentView.swift` and `Chainworks_ForgeApp.swift`, which would imply `32` total review targets rather than `30`
- Recommendation:
  - rebaseline the audit inventory against current `HEAD`
  - either list the exact preview set audited or remove the completeness claim
  - make the scope counts internally consistent before treating the issue catalogue as authoritative

#### Finding UI-02 — `L-11` is already closed on the presentation layer and points at the wrong owner

- Severity: `Medium`
- Confidence: `High`
- Location: `docs/proposals/012-ui-quality-audit-and-visual-polish.md:371-377`
- Why it matters:
  - `L-11` says `DeliveryPreflightReportView` needs a minimum sheet frame and assigns the work to `Views/DeliveryPreflightReportView.swift`
  - current code already applies minimum sheet frames where the report is presented:
    - [IdeaListView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/IdeaListView.swift#L997) -> `.frame(minWidth: 480, minHeight: 360)`
    - [PilotReadinessView.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/PilotReadinessView.swift#L248) -> `.frame(minWidth: 520, minHeight: 420)`
    - [FirstRunSetupWizard.swift](/Users/user/Documents/Chainworks Forge/Chainworks Forge/Views/FirstRunSetupWizard.swift#L224) -> `.frame(minWidth: 520, minHeight: 420)`
  - that means the issue is either already fixed or belongs to preview sizing, not to the report view itself
- Recommendation:
  - remove `L-11` from the open backlog, or rewrite it as a preview-sizing / presentation-consistency note owned by the presentation sites

### UX

No separate live UX text blocker beyond the stale issue-catalog ownership above surfaced in this round.

Evidence-level note:

- the targeted macOS UI rerun did not reach runtime assertions because the runner failed to initialize automation mode, so fresh visual proof for the proposed polish areas is still missing

### Architecture

#### Finding ARCH-01 — Dependency baseline is understated for the audited surfaces

- Severity: `High`
- Confidence: `High`
- Location: `docs/proposals/012-ui-quality-audit-and-visual-polish.md:8-10`
- Why it matters:
  - the proposal says it depends only on `007` and `008`
  - the actual issue catalogue and remediation plan materially rely on surfaces introduced later in the stack, including:
    - `ProviderSettingsView`, `PilotReadinessView`, `FirstRunSetupWizard` from the provider/settings path
    - `ArchivedIdeasView`, `GooseProviderConnectionAssistantView`, `WorkflowMapView` from the operator-clarity path
    - current `Ideas -> Start Run` and runtime shells shaped by Proposal 011
  - if 012 is sequenced or read literally from its dependency table, a large part of the audited UI would not exist yet
- Recommendation:
  - either expand the dependency table to reflect the real baseline, or narrow the proposal to the subset of shells guaranteed by 007/008

## 4. Cross-Discipline Conflicts and Decisions

- Conflict: the draft presents itself as a complete UI audit, but the current repo evidence shows both stale inventory and already-closed issue entries.
  Tradeoff: treat the catalogue as mostly good enough versus require a precise rebaseline before implementation.
  Decision: require rebaseline. Proposal 012 is itself an audit artifact, so stale completeness claims materially weaken the document.
  Owner: proposal author

- Conflict: current-round build health is green, but current-round visual proof is incomplete because the macOS UI runner failed before automation mode initialized.
  Tradeoff: infer visual proof from code alone versus keep the review partial.
  Decision: keep the round partial and explicit about the missing screenshots / attachments.
  Owner: review process / environment

## 5. Prioritized Action Backlog

| Priority | Item | Discipline | Owner | Horizon | Dependencies | Success Metric | Source |
|---|---|---|---|---|---|---|---|
| P1 | Fix the dependency table so it matches the real audited baseline or narrow the proposal scope to 007/008-owned shells only | Architecture | Proposal author | Immediate | none | dependency table no longer understates required surfaces | ARCH-01 |
| P1 | Rebaseline the audit methodology and appendix counts against current `HEAD` | UI | Proposal author | Immediate | none | preview/file counts and completeness wording are accurate | UI-01 |
| P2 | Remove or rewrite `L-11` so it no longer tracks an already-closed presentation concern as an open view-level defect | UI / UX | Proposal author | Immediate | none | `L-11` no longer points at the wrong owner or a closed issue | UI-02 |
| P2 | Rerun the targeted macOS UI slice once automation mode is stable so the evidence pack includes current-round attachments | UI | Review owner | Next rereview | environment stable | current-round xcresult contains product proof instead of runner-init failure | Evidence gap only |

## 6. Validation and Measurement Plan

| Area | What Will Be Measured | Leading Indicators | Guardrails | Review Checkpoint | Hold Criteria |
|---|---|---|---|---|---|
| Audit inventory truth | whether the proposal accurately enumerates the surfaces it audited | preview count and file count match repo search | do not claim completeness without enumerating the audited set | next rereview | hold if counts are still stale |
| Issue catalogue freshness | whether open issues still correspond to current live UI problems | removed stale items such as `L-11` | do not preserve already-fixed issues just because they existed in an earlier pass | next rereview | hold if closed issues remain in the backlog |
| Sequencing truth | whether dependencies match the actual UI baseline required by the proposal | dependency table includes the true prerequisite proposals or narrowed scope | do not allow proposal order to imply missing surfaces exist | next rereview | hold if dependency table still understates the baseline |
| Visual proof | whether targeted macOS UI evidence produces current-round attachments | UI runner reaches automation mode and runs the targeted slice | do not substitute code inspection for missing screenshot proof when testing is available | next rereview | hold if runner init still fails |

## 7. Evidence Gaps and Open Questions

### Evidence Gaps

- `GAP-01`: no current-round authoritative screenshots / attachments were produced because the macOS UI runner failed to initialize for automation mode
- `GAP-02`: because the proposal itself is an audit document, the stale inventory weakens confidence in uncited or borderline issue entries beyond the ones explicitly rechecked in this round

### Open Questions

- Are there more already-closed catalogue items beyond `L-11` that should be culled once the inventory is rebaselined against current `HEAD`?

## Evidence Gap Review Fallback

- What was attempted:
  - reread Proposal 012 end-to-end against current `HEAD`
  - reran fresh build proof
  - attempted a fresh targeted macOS UI slice for provider settings, wizard, Goose assistant, pilot readiness, archive, workflow map, and run-start/run-progress surfaces
  - spot-checked the concrete files named in the issue catalogue
- What is missing:
  - current-round product screenshots / attachments
  - a fully trustworthy, up-to-date issue inventory inside the proposal itself
- Blockers:
  - the UI runner failed with `Timed out while enabling automation mode`
  - the draft still contains stale / mis-scoped audit claims
- Confidence: `High`
- What can still be said with partial confidence:
  - current `HEAD` build health is green
  - the dependency table is understated
  - the audit inventory is stale
  - `L-11` is already addressed at the presentation layer
- What evidence is required to finish the full review:
  - corrected proposal scope / inventory
  - a fresh successful targeted macOS UI rerun with attachments
