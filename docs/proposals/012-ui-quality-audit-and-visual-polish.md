# Proposal 012: UI Quality Audit and Visual Polish

| Field | Value |
|---|---|
| Date | 2026-03-26 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/full-mvp-delivery.md](../reference/full-mvp-delivery.md), [reference/mvp-sign-off.md](../reference/mvp-sign-off.md), [idea-lifecycle](../reference/idea-lifecycle.md), [live-workflow-map](../reference/live-workflow-map.md) |
| Scope | UI/UX visual quality, consistency, and polish across the current operator-facing macOS surfaces listed in Appendix A |
| Goal | Elevate the Chainworks Forge UI from functional prototype to production-quality macOS citizen by fixing truncation, information density, visual consistency, and trust-bearing non-happy-path feedback while introducing a bounded lightweight design-system slice. |

---

## 1. Context

The current UI was built incrementally across the MVP delivery and hardening slices.
Runtime behaviour and correctness were correctly prioritised ahead of visual polish, while some UI baseline truth has since moved into stable reference documents such as `idea-lifecycle.md` and `live-workflow-map.md`.
The result is a **functionally complete but visually inconsistent** interface that has accumulated multiple categories of UI debt:

- Severe text truncation in sidebars making content unreadable.
- Information-dense screens with no visual hierarchy.
- Inconsistent badge, color, and typography patterns across views.
- No formal design system — colors, fonts, spacing are all ad-hoc.
- Missing empty-state experiences and visual feedback cues.

This revision is intentionally rebaselined against current `HEAD`.
It no longer treats stale repo-wide counts as the source of truth.
Instead, Appendix A defines the audited operator surfaces that drive this proposal, and the open backlog below excludes items already addressed since the earlier audit snapshot.

### 1.1 Audit Methodology

All issues were identified via:

1. **Xcode Preview rendering** of the current preview-backed operator surfaces listed in Appendix A.
2. **Code review** of the current operator-facing SwiftUI view files in `Views/`, plus `ContentView.swift` and `Chainworks_ForgeApp.swift`, without treating a raw file count as the source of truth.
3. **Cross-view consistency analysis** comparing badge styles, color usage, font scales, layout patterns, and async feedback treatment.
4. **Reference-baseline cross-check** against current stable docs where surface behaviour is already codified outside proposal-number lineage.

### 1.2 Baseline Authorities

This proposal depends on four baseline authorities:

1. `docs/reference/full-mvp-delivery.md` for the delivered MVP operator shell and repo-backed delivery workflow foundations.
2. `docs/reference/mvp-sign-off.md` for MVP hardening expectations and sign-off posture.
3. `docs/reference/idea-lifecycle.md` for current idea/archive lifecycle truth used by `IdeaListView` and related archive surfaces.
4. `docs/reference/live-workflow-map.md` for current workflow-map topology and agent-state presentation truth.

Where older proposal snapshots and current stable references diverge, **current `HEAD` plus the stable reference docs win**.

### 1.3 Rebaseline Outcomes

The earlier audit snapshot contained items that are no longer safe to carry as open work:

- The earlier `30 view files / 12 previews` inventory claim is retired; Appendix A is now the audited-surface source of truth for this proposal revision.
- `C-02` is removed from the active backlog because `IdeaListView` already uses the wider sidebar width at current `HEAD`.
- `L-02` is removed from the active backlog because the idea detail empty state already uses `ContentUnavailableView`.
- `L-11` is reclassified as a preview ergonomics note, not a production defect in `DeliveryPreflightReportView`.
- `L-09` is narrowed from "no keyboard shortcuts exist" to "coverage is incomplete on key operator flows."

---

## 2. Issue Catalogue

### CRITICAL — Content Unreadable

#### C-01: RunsHomeView sidebar — severe text truncation

**Observed:** All run titles in the sidebar render as "Provider tr...", "Delivery dr...", "Retire old...", "Archive fin...". Section headers truncate as "Recently Comp...". Status badges, stage labels, elapsed time, cost, and provenance badges are all compressed into unreadable micro-fragments.

**Root cause:** `RunsHomeView` does not set `navigationSplitViewColumnWidth`. The default macOS NavigationSplitView sidebar is ~180pt, but `RunsHomeRow` contains 4 vertical lines of dense content (title, status row with capsule + stage, parent badge, metadata row with 3+ labels).

**Fix:**
1. Add `.navigationSplitViewColumnWidth(min: 280, ideal: 340)` to the sidebar list.
2. Make `RunsHomeRow` adaptive: collapse the metadata HStack into a single summarised line at narrow widths.
3. Use `.lineLimit(1)` with `.truncationMode(.tail)` on the title and show full title in the detail panel.

**Affected file:** `Views/RunsHomeView.swift` (lines 23–121, 261–329)

---

### HIGH — Information Overload / Poor Visual Hierarchy

#### H-01: ProviderSettingsView — wall of undifferentiated content

**Observed:** The Provider Settings screen packs Managed Goose Server section, all configured providers (each with nested health snapshots, troubleshooting panels, model lists, and action buttons), the "Add Provider" form, a Settings Transfer section, and an Advanced Configuration section into a single scrolling List with no visual prioritization.

**Fix:**
1. Move "Add Provider" into a dedicated sheet triggered by a toolbar `+` button, reducing inline noise.
2. Wrap each configured provider in a `GroupBox` with clear visual boundary.
3. Collapse "Advanced Configuration" behind a `DisclosureGroup` — these are infrequently changed paths.
4. Move "Settings Transfer" into the toolbar or a secondary panel.

**Affected file:** `Views/ProviderSettingsView.swift` (lines 22–94, 170–300)

---

#### H-02: PilotReadinessView — dense flat list with no summary

**Observed:** Pilot Readiness renders as a wall of LabeledContent items. Raw file system paths consume significant vertical space. No at-a-glance status summary exists at the top. An operator cannot immediately see "is the system ready?"

**Fix:**
1. Add a hero status banner at the top: green checkmark or red warning with one-line verdict ("System Ready" / "3 Issues Found").
2. Wrap Configuration paths in a collapsible `DisclosureGroup("Configuration Paths")`.
3. Add visual progress indicator showing readiness completion (e.g. "5/7 checks pass").
4. Group provider cards with clear GroupBox boundaries (currently providers list has no visual separation).

**Affected file:** `Views/PilotReadinessView.swift` (lines 21–277)

---

#### H-03: FirstRunSetupWizard — linear form without step progression

**Observed:** The wizard is a single long Form with 7 sections scrolling vertically. There is no step indicator, progress bar, or visual milestone. The "Save and Launch Sample Run" primary CTA is buried at the very bottom after dozens of fields.

**Fix:**
1. Add a step indicator strip at the top showing: Workspace → Providers → Verification → Launch.
2. Group the form into logical steps with `TabView` or progressive disclosure.
3. Elevate the primary CTA "Save and Launch Sample Run" — either keep it visible via a floating footer, or show it prominently in the final step.
4. Add visual validation marks (green checkmarks) on sections that pass validation.

**Affected file:** `Views/FirstRunSetupWizard.swift` (lines 26–255)

---

#### H-04: RunsHomeRow — too much information in row

**Observed:** Each `RunsHomeRow` contains: title, attention icon, workflow subtitle, status capsule, stage label, parent idea badge, elapsed time, cost, last progress time, and provenance badge — all packed into a VStack with 4pt vertical spacing. In the sidebar, this creates visual noise where no single data point is legible.

**Fix:**
1. Reduce the sidebar row to: title (1 line), status capsule + one metadata item (1 line), elapsed time (small).
2. Move the full details (parent idea badge, provenance badge, cost, last progress) to the detail panel.
3. Apply a 2-line maximum density rule for sidebar rows.

**Affected file:** `Views/RunsHomeView.swift` (lines 261–329)

---

### MEDIUM — Visual Inconsistency

#### M-01: Badge/capsule implementation fragmented across 6+ views

**Observed:** Status badges are implemented independently in:
- `RunsHomeRow` (lines 276–282) — `.padding(.horizontal, 6) .padding(.vertical, 2)`
- `RuntimeProvenanceBadge` (lines 384–395) — `.padding(.horizontal, 6) .padding(.vertical, 2)`
- `ParentIdeaArchiveBadge` (lines 440–446) — `.padding(.horizontal, 8) .padding(.vertical, 3)`
- `ReleaseGateView` status badge (lines 93–100) — `.padding(.horizontal, 8) .padding(.vertical, 4)`
- `WorkflowMapStatusBadge` (lines 216–221) — `.padding(.horizontal, 8) .padding(.vertical, 4)`
- `DeliveryPreflightReportView` (lines 18–24) — `.padding(.horizontal, 8) .padding(.vertical, 4)`
- `IdeaLifecycleBadge` — separate implementation

Each uses different padding values, font sizes (.caption, .caption2, .caption2.bold()), and background opacity levels (.12, .14, .15).

**Fix:** Extract a reusable `StatusCapsule` view:
```swift
struct StatusCapsule: View {
    let text: String
    let color: Color
    var size: Size = .regular

    enum Size {
        case small   // caption2, px:6/py:2
        case regular // caption2, px:8/py:3
    }

    var body: some View {
        Text(text)
            .font(.caption2.bold())
            .padding(.horizontal, size == .small ? 6 : 8)
            .padding(.vertical, size == .small ? 2 : 3)
            .background(color.opacity(0.15), in: Capsule())
            .foregroundStyle(color)
    }
}
```

Shared status affordances must not rely on color alone.
Badges, chips, and status cards must preserve a textual/icon cue and remain legible under Differentiate Without Color Alone, Increase Contrast, and Reduce Transparency.

**Affected files:** `RunsHomeView.swift`, `ReleaseGateView.swift`, `WorkflowMapView.swift`, `DeliveryPreflightReportView.swift`, `IdeaListView.swift`

---

#### M-02: Color palette used without semantic naming

**Observed:** Hardcoded color references appear throughout:
- `.orange` — used for approvals (13 occurrences), warnings, AND some buttons
- `.red` — used for blocked, failed, AND destructive actions
- `.green` — used for completed, active ideas, healthy status, AND approve buttons
- `.blue` — used for running status AND informational elements
- `.secondary` — used for pending/unknown, descriptive text, AND disabled states

No semantic distinction between "status color" and "action color" and "informational color".

**Fix:** Create a `DesignTokens` namespace:
```swift
enum DesignTokens {
    enum Status {
        static let success = Color.green
        static let warning = Color.orange
        static let error = Color.red
        static let running = Color.blue
        static let neutral = Color.secondary
        static let cancelled = Color.gray
    }
    enum Action {
        static let primary = Color.accentColor
        static let destructive = Color.red
        static let approve = Color.green
        static let caution = Color.orange
    }
}
```

**Affected files:** Phase 3 first adopters only in the initial pass: `RunsHomeView.swift`, `WorkflowMapView.swift`, `ReleaseGateView.swift`, `DeliveryPreflightReportView.swift`, and `IdeaListView.swift`. Expansion to the rest of the app is deferred until the adopter slice passes verification.

---

#### M-03: Font scale applied inconsistently

**Observed:**
- Titles use `.title`, `.title2`, `.title3`, and `.title3.bold()` interchangeably across screens.
- Body text alternates between `.body`, `.subheadline`, and `.callout` with no consistent rule.
- Metadata uses `.caption`, `.caption2`, and `.caption2.bold()` without a clear hierarchy.
- Some views apply `.font(.headline)` to section headers while others use `.font(.subheadline.bold())`.

**Fix:** Define a typography scale:
| Semantic | SwiftUI font | Usage |
|---|---|---|
| Screen title | `.title2.bold()` | NavigationTitle (handled by system) |
| Section header | `.headline` | All GroupBox/Section titles |
| Card title | `.subheadline.weight(.semibold)` | Row titles, provider names |
| Body | `.body` | Primary content text |
| Supporting | `.caption` | Secondary/descriptive text |
| Micro | `.caption2` | Timestamps, metadata, badge text |

Apply the scale first to the Phase 3 adopter slice and to any surface touched in Phases 1-2.
Repo-wide migration is explicitly deferred until the adopter slice is stable.

---

#### M-04: Foreground banner animation direction mismatch

**Observed:** `ForegroundBannerView` is positioned at `.bottom` via overlay alignment (ContentView line 114) but uses `.transition(.move(edge: .top))` (ForegroundBannerView line 66). The banner slides in from the top while being placed at the bottom.

**Fix:** Change to `.transition(.move(edge: .bottom).combined(with: .opacity))`.

**Affected file:** `Views/ForegroundBannerView.swift` (line 66)

---

### LOW — Polish and Experience

#### L-01: Empty states lack visual personality

**Observed:** All `ContentUnavailableView` instances use system SF Symbols:
- "Select a Run" → `sidebar.left`
- "No Runs" → `tray`
- "Select an idea" → `lightbulb`
- "Select an archived idea" → `archivebox`
- "No Pending Approvals" → `checkmark.seal`

These are generic and create a flat, utilitarian feel.

**Fix:**
1. Use `.symbolRenderingMode(.multicolor)` where available for richer icons.
2. Add `.font(.system(size: 48))` to empty-state icons for better visual presence.
3. Consider adding subtle hint text with specific call-to-action: "Create your first run" with a button vs. just "No Runs".

**Affected files:** `RunsHomeView.swift`, `IdeaListView.swift`, `ArchivedIdeasView.swift`, `ApprovalGateView.swift`

---

#### L-03: Summary strip in IdeaListView is hard to parse

**Observed:** The summary strip at the top of IdeaListView packs idea count, draft count, active count, archived count, running count, and live runtime readiness status into a single horizontal line with `.font(.caption)`. At narrow widths, this wraps unpredictably.

**Fix:**
1. Split into two rows: counts on the left, runtime status on the right.
2. Use pill-shaped chips for each count category instead of concatenated text.
3. Give the runtime readiness indicator its own visual treatment (bordered pill, not just a label).

**Affected file:** `Views/IdeaListView.swift` (lines 110–151)

---

#### L-04: GooseProviderConnectionAssistantView — no progress/journey visualization

**Observed:** The assistant has a "Journey" section that shows Origin and State as plain LabeledContent text. The concept of a guided journey exists in the code (`GooseProviderJourneyState`) but has no visual representation of progression.

**Fix:** Add a simple 3-step progress indicator: Configure → Verify → Connected, with the current step highlighted. This makes the assistant feel guided rather than like another settings form.

**Affected file:** `Views/GooseProviderConnectionAssistantView.swift` (lines 29–39)

---

#### L-05: WorkflowMapView topology cards lack interactive affordance

**Observed:** The topology section shows stage cards in a horizontal scroll with chevron separators. The cards are informative but purely static. The user cannot tap a card to navigate to stage details.

**Fix:** Make stage cards tappable to show a popover or navigate to `StageDetailView`. Add subtle hover effect (`.onHover` with highlight) to signal interactivity.

**Affected file:** `Views/WorkflowMapView.swift` (lines 95–209)

---

#### L-06: ReleaseGateView review items — "Missing" in orange/red creates false alarm

**Observed:** In the Release Gate, review summary items show "Missing" in orange text with an empty circle icon. For a release that hasn't yet reached those stages, "Missing" reads as an error when it's actually an expected pre-condition.

**Fix:** Distinguish between:
- "Not yet produced" (neutral/secondary, expected during in-progress runs)
- "Missing" (warning/orange, expected artifact was not generated)
- "Available" (green, artifact exists)

**Affected file:** `Views/ReleaseGateView.swift` (lines 147–158)

---

#### L-07: New Idea sheet uses raw VStack instead of Form

**Observed:** The "New Idea" sheet (IdeaListView lines 170–199) uses a plain `VStack` with manual `TextField` styling and `.border()` for the `TextEditor`. This doesn't match the macOS Form style used everywhere else in the app.

**Fix:** Wrap in `Form` with proper sections for consistency:
```swift
Form {
    Section("Details") {
        TextField("Title", text: $newTitle)
        TextEditor(text: $newBody)
            .frame(minHeight: 100)
    }
    Section("Attachment") {
        HStack {
            TextField("Path (optional)", text: $newAttachmentPath)
            Button("Browse...") { browseAttachment() }
        }
    }
}
```

**Affected file:** `Views/IdeaListView.swift` (lines 170–199)

---

#### L-08: Seven tabs may overwhelm — consider grouping

**Observed:** The tab bar contains 7 tabs: Runs Home, Ideas, Approvals, Agent Catalog, Workflow Inspector, Pilot Readiness, Settings. On narrower windows, tab labels may truncate. "Pilot Readiness" and "Settings" serve similar administrative purposes.

**Recommendation (non-breaking):** Consider grouping "Pilot Readiness" as a section within "Settings" or as a toolbar button within Settings, reducing tabs to 6. Alternatively, use a segmented control within a combined "Configuration" tab.

**Affected file:** `ContentView.swift` (lines 66–112)

---

#### L-09: Keyboard shortcut coverage is incomplete on key operator flows

**Observed:** Current code already includes keyboard shortcuts such as new-idea creation and standard cancel actions, so the issue is no longer "none exist." The remaining gap is inconsistent coverage on high-value approval, release, recovery, and first-run confirmation flows.

**Fix:**
1. Preserve existing shortcuts and do not conflict with system-defined macOS shortcuts.
2. Add shortcuts only to the highest-value confirmation and dismissal flows.
3. Modal confirm/dismiss surfaces must remain fully operable for keyboard-only users, including `Escape` for dismissal where appropriate.
4. Document ownership per surface so approval, release, recovery, and wizard flows converge on one convention set.

**Affected files:** `ApprovalGateView.swift`, `ReleaseGateView.swift`, `FirstRunSetupWizard.swift`, `RecoverySheet.swift`

---

#### L-10: RunDetailPanel — actions at bottom are easy to miss

**Observed:** In `RunDetailPanel`, the contextual action buttons (Recover, Compare, View Report, Export Evidence Pack) appear at the very bottom of a `ScrollView`. When the stage list is long, these actions are below the fold.

**Fix:** Move important action buttons into the toolbar or a sticky footer outside the `ScrollView`, ensuring they stay above the fold while rare or advanced controls remain in disclosure or secondary destinations.

**Affected file:** `Views/RunsHomeView.swift` (lines 536–570)

---

#### L-12: No loading/progress states during async operations

**Observed:** Multiple views trigger async operations (provider diagnostics refresh, preflight checks, sample run launch, Goose verification) but show no visual loading indicator during the operation. The `isProbing` state in GooseAssistant disables buttons but shows no spinner.

**Fix:**
1. `ProviderSettingsView`: inline progress row inside the affected provider card, disabled action copy, and an inline retry affordance on failure.
2. `PilotReadinessView`: section-local progress plus a top summary banner that distinguishes loading, degraded, and failed readiness checks.
3. `GooseProviderConnectionAssistantView`: step-local spinner and status copy inside the Journey section, not only disabled buttons.
4. `FirstRunSetupWizard`: blocking footer progress for "Save and Launch Sample Run" with explicit success/failure copy and preserved form state on failure.
5. Recoverable loading and error states stay inline/local to the initiating surface; alerts are reserved for destructive, irreversible, or explicit acknowledgement flows.

**Affected files:** `PilotReadinessView.swift`, `ProviderSettingsView.swift`, `GooseProviderConnectionAssistantView.swift`, `FirstRunSetupWizard.swift`

---

## 3. State and Feedback Contract

Proposal 012 is not only a happy-path readability pass.
For every touched operator surface, the implementation must either define the non-happy-path treatment below or explicitly defer it.

### 3.1 Surface State Matrix

| Surface | Validation | Backend / Offline / Auth | Retry / Recovery | Cancellation / Rollback |
|---|---|---|---|---|
| `IdeaListView` / `NewIdeaSheetView` | Invalid title or attachment input must show inline field messaging and block save until valid. | Create/archive failures use inline banner copy; auth-expiry handling stays owned by the provider platform and is only surfaced here if already raised upstream. | Failed create/archive actions keep local draft state and offer retry without data loss. | Cancel closes the sheet without partial persistence; archive/restore remains reversible through the existing archive flow. |
| `ProviderSettingsView` | Invalid provider settings must be called out inline in the affected form section. | Distinguish misconfiguration, backend failure, offline/degraded transport, and auth-expiry/reconnect-required states instead of collapsing them into generic warnings. | Every diagnostics/probe action needs a local retry or refresh action on the same card. | Cancelling a refresh/probe must leave existing saved configuration intact. |
| `PilotReadinessView` | Validation failures from required configuration must appear in the summary banner and the owning section. | Readiness must differentiate blocking backend failure, degraded/offline checks, and auth-required reconnect states. | Failed checks must expose rerun/retry affordances without forcing navigation away from the surface. | Dismissing readiness UI never rolls back configuration; it only ends the current review pass. |
| `FirstRunSetupWizard` | Section-level validation summary plus inline field errors; launch stays disabled while invalid. | Save/launch failures must differentiate local validation, backend/probe failure, offline transport, and auth-expiry during provider verification. | Operators can rerun verification and relaunch without re-entering already valid sections. | Cancelling the wizard exits without creating a run; rollback of partially persisted settings is out of scope and must not be implied. |
| `GooseProviderConnectionAssistantView` | Invalid/manual inputs must show local guidance before probing starts. | Probe results must distinguish unreachable server, degraded transport, auth/reconnect, and unexpected backend errors. | Journey states need explicit retry/reprobe affordances with preserved context. | Cancelling or dismissing the assistant must not mutate provider state. |
| `WorkflowMapView` / `ReleaseGateView` / `DeliveryPreflightReportView` | Not form-driven; validation rules are represented as artifact availability and check-result semantics. | Distinguish "not yet produced", "fetch failed", "backend unavailable", and "requires re-auth" where artifacts depend on remote data. | Allow reload/recheck from the same surface where data is fetched. | Release cancellation/rollback remains a workflow concern outside this proposal; this proposal owns only the display semantics and dismissal behaviour. |
| `RunsHomeView` / `RunDetailPanel` | Not form-driven. | Stale or failed refresh states must not look identical to a legitimate empty list. | Refresh/recover actions must remain visible even when long detail content pushes the fold. | Destructive recovery actions keep their existing explicit confirmation boundaries. |

### 3.2 Async Feedback by Surface

| Surface | Loading Treatment | Success Feedback | Failure Feedback |
|---|---|---|---|
| `ProviderSettingsView` | Inline spinner inside the affected provider card; disable only the relevant actions. | Short health/status refresh copy in the same card. | Inline error block with retry action and preserved diagnostics context. |
| `PilotReadinessView` | Section-local progress plus top summary banner. | Updated readiness verdict and refreshed completion count. | Banner-level failure/degraded summary plus section-local explanation. |
| `GooseProviderConnectionAssistantView` | Journey-local spinner and in-progress copy on the verification step. | Advance the journey state with explicit connected/verified language. | Retryable guidance card that preserves the previous probe context. |
| `FirstRunSetupWizard` | Footer-level blocking progress during save/launch; section content remains readable. | Clear launch confirmation and next-step guidance. | Non-destructive error summary plus section-local indicators for what must be fixed or retried. |

### 3.3 Deferred State Ownership

The following remain outside Proposal 012 and must not be invented locally:

- Global account/session recovery flows beyond the surface-level presentation of an upstream auth-expiry state.
- Engine-level rollback semantics for release or run-control operations.
- New persistence, caching, or transport contracts.

---

## 4. Design System Foundation

### 4.1 Proposed File Structure

```
Support/
├── DesignTokens.swift       // Colors, spacing, corner radii
├── StatusCapsule.swift       // Reusable badge component
├── EmptyStateView.swift      // Standardized empty state wrapper
└── PreviewSupport.swift      // (existing)
```

### 4.2 Spacing Tokens

| Token | Value | Usage |
|---|---|---|
| `compact` | 4pt | Tight inline spacing |
| `small` | 8pt | Between related items |
| `medium` | 12pt | Between sections within a group |
| `large` | 16pt | Between GroupBoxes/sections |
| `section` | 20pt | Between major content blocks |

### 4.3 Corner Radius Tokens

| Token | Value | Usage |
|---|---|---|
| `badge` | `Capsule()` | Status capsules, tags |
| `card` | 14pt continuous | Stage cards, agent panels |
| `panel` | 16pt continuous | Larger containers |
| `sheet` | system default | Sheets, popovers |

---

### 4.4 First-Adopter Slice and Migration Guardrails

The initial design-system rollout is intentionally bounded.
Phase 3 is limited to the surfaces already carrying live visual inconsistency risk:

- `RunsHomeView` status, provenance, and archive badges
- `WorkflowMapView` status badges
- `ReleaseGateView` status and artifact-semantics badges
- `DeliveryPreflightReportView` status badges
- `IdeaListView` summary-strip chips if they are touched by the readability pass

Guardrails for the first pass:

1. No business-logic changes and no navigation changes are allowed inside the shared primitive extraction.
2. Existing `accessibilityIdentifier` values and keyboard behaviors must remain stable.
3. New shared primitives (`StatusCapsule`, `DesignTokens`, typography helpers) may be adopted only on the surfaces above in the first pass.
4. Badges, chips, and status cards in the adopter slice must continue to differentiate state without color alone and preserve non-text contrast under Increase Contrast and Reduce Transparency.
5. Expansion to the rest of the app is allowed only after the adopter slice passes previews, min-window checks, VoiceOver labels/traits, focus order, and non-text contrast verification without regressions.

macOS command-placement rule for all phases:

- Keep high-value commands above the fold or in a toolbar/sticky footer.
- Move rare advanced controls into `DisclosureGroup` sections or secondary destinations instead of leaving them in the primary reading path.

---

## 5. Implementation Plan

### Phase 1: Current-HEAD Readability Fixes (estimated: 1 day)
- [ ] C-01: Fix RunsHomeView sidebar width and row density
- [ ] M-04: Fix banner animation direction
- [ ] L-03: Redesign IdeaListView summary strip
- [ ] L-06: Fix ReleaseGate review status semantics
- [ ] L-10: Move RunDetailPanel actions out of the fold

### Phase 2: Operator State and Feedback Contracts (estimated: 1–2 days)
- [ ] H-01: Restructure ProviderSettingsView and define per-state hierarchy
- [ ] H-02: Add hero status to PilotReadinessView with degraded/error semantics
- [ ] H-03: Add step progression and validation summary to FirstRunSetupWizard
- [ ] L-04: Add journey visualization and probing feedback to GooseAssistant
- [ ] L-09: Complete keyboard shortcut coverage on primary operator flows
- [ ] L-12: Implement the per-surface loading, success, and failure treatments defined in Section 3

### Phase 3: First-Adopter Shared Primitives (estimated: 1 day)
- [ ] M-01: Extract `StatusCapsule` for the bounded adopter slice
- [ ] M-02: Create `DesignTokens` with semantic colors and spacing for the bounded adopter slice
- [ ] M-03: Apply the agreed typography scale to the bounded adopter slice
- [ ] Expand beyond the adopter slice only if Phase 3 guardrails pass unchanged-behaviour checks

### Phase 4: Secondary Polish (estimated: 1 day)
- [ ] L-01: Enhance empty states
- [ ] L-05: Make WorkflowMap topology cards interactive
- [ ] L-07: Convert New Idea sheet to Form
- [ ] H-04: Simplify RunsHomeRow for long-term sidebar density cleanup
- [ ] L-08: Reassess tab grouping only after the higher-priority operator flows are stable

---

## 6. Verification Criteria

Each fix must be verified via:

1. **Xcode Preview rendering** for every preview-backed surface named in Appendix A at the declared preview frame size — no truncation, no overflow.
2. **Minimum window size test** — resize to 1024×768 and verify all surfaces in Appendix A that declare `Min-window` proof ownership remain usable.
3. **State contract verification** — validation, backend failure, offline/degraded, auth-required, retry, and cancellation semantics must either match Section 3 or be explicitly deferred.
4. **Cross-view consistency** — badges, colors, and fonts must match the bounded design tokens after migration.
5. **Keyboard and interaction verification** — `ApprovalGateView`, `ReleaseGateView`, `FirstRunSetupWizard`, and `RecoverySheet` must expose the intended primary confirm/dismiss bindings without colliding with system-defined shortcuts, modal dismiss flows must work keyboard-only including `Escape` where appropriate, and `RunDetailPanel` high-value actions must remain discoverable without scrolling the full detail body.
6. **Accessibility settings audit** — on the bounded adopter slice, verify Differentiate Without Color Alone, Increase Contrast, and Reduce Transparency behavior for badges, chips, and status cards, including non-text contrast and focus visibility.
7. **Accessibility audit** — VoiceOver must read status information, labels, and traits correctly rather than relying only on visual cues.
8. **Implementation evidence handoff** — after code lands, runtime screenshots and live interaction proof move to the follow-up implementation evidence review rather than staying implicit here.

---

## 7. Out of Scope

- Custom illustrations or branded iconography (post-MVP).
- Light mode support (needs separate design pass).
- Animation/motion design beyond fixing the banner transition bug.
- Localization readiness (separate proposal).
- iOS/iPadOS adaptation (macOS-only for MVP).

---

## 8. Risk Assessment

| Risk | Mitigation |
|---|---|
| Design system extraction causes regression | Shared primitives are limited to the Phase 3 adopter slice first; no repo-wide migration occurs until the adopter slice passes unchanged-behaviour verification. |
| Sidebar width changes break existing layouts | Test with preview seeds at min/ideal/max widths. |
| Wizard step progression changes break UI tests | Maintain existing `accessibilityIdentifier` values on all elements. |
| Time overrun on polish items | Phase 1 and 2 target live operator-facing issues first; Phase 4 remains independently shippable follow-on polish. |
| Non-happy-path states drift between surfaces | Section 3 defines per-surface state ownership and must be treated as an implementation gate, not a suggestion. |

---

## Appendix A: Current Audited Surface Reference

Appendix A is the authoritative audited-surface list for this proposal revision.
It replaces the earlier stale repo-wide count claim.
Subordinate sheets and panels are listed separately whenever they carry their own proof obligation.

| Surface | Proof Owner | Proof Asset / Check | Key Open Issues |
|---|---|---|---|
| `ContentView` tab shell | Preview | `Content Shell — Seeded` | L-08 shell grouping discussion, M-04 banner ownership |
| `RunsHomeView` sidebar/list | Preview + Min-window | `Runs Home — Mixed States`; 1024×768 resize | C-01, H-04 (truncation, row density) |
| `RunDetailPanel` | Interaction checklist + Min-window | 1024×768 resize plus detail-action visibility check in the parent runs surface | L-10 action discoverability below the fold |
| `IdeaListView` | Preview + Min-window | `Ideas — Operator List` | L-03 dense summary strip; top-level idea flow remains in scope |
| `NewIdeaSheetView` | Preview | `New Idea Sheet — Empty`, `New Idea Sheet — Ready` | L-07 raw `VStack` form treatment |
| `ProviderSettingsView` | Preview | `Provider Settings — Configured` | H-01 information overload, L-12 inline async feedback |
| `PilotReadinessView` | Preview | `Pilot Readiness — Seeded` | H-02 wall of text, L-12 summary/loading semantics |
| `FirstRunSetupWizard` | Preview + Interaction checklist | `First Run Setup — Seeded` plus keyboard binding check | H-03 step progression, L-09 shortcut ownership, L-12 launch feedback |
| `ArchivedIdeasView` | Preview | `Archived Ideas — Seeded` | L-01 generic empty-state treatment |
| `DeliveryPreflightReportView` positive state | Preview | `Delivery Preflight — All Passing` | Shared badge/token alignment on happy-path report rendering |
| `DeliveryPreflightReportView` negative state | Preview | `Delivery Preflight — Issues Found` | Negative-state readability and status semantics |
| `GooseProviderConnectionAssistantView` | Preview | `Goose Assistant` | L-04 journey visualization, L-12 probing feedback |
| `ApprovalGateView` | Interaction checklist | Confirm/dismiss binding check plus approval action discoverability | L-09 shortcut ownership for approval decisions |
| `ReleaseGateView` | Preview + Interaction checklist | `Release Gate — Sandbox` plus keyboard binding check | L-06 missing-vs-not-yet-produced semantics, L-09 shortcut ownership |
| `RecoverySheet` | Interaction checklist | Confirm/dismiss binding check plus recovery-action discoverability | L-09 shortcut ownership for recovery flows |
| `RunStartOverridesView` | Preview | `Override List — 8 agents` | Adequate — no critical issues |
| `WorkflowMapView` | Preview | `Workflow Map — Proposal Loop` | L-05 static topology cards |

---

## Appendix B: Badge Padding Audit

| Component | File | px H | py V | Font | Opacity |
|---|---|---|---|---|---|
| RunsHomeRow status | RunsHomeView:278 | 6 | 2 | .caption | 0.15 |
| RuntimeProvenanceBadge | RunsHomeView:392 | 6 | 2 | .caption2 | 0.12 |
| ParentIdeaArchiveBadge | RunsHomeView:442 | 8 | 3 | .caption2.semibold | 0.14 |
| ReleaseGate statusBadge | ReleaseGateView:95 | 8 | 4 | .caption.bold | 0.15 |
| WorkflowMapStatusBadge | WorkflowMapView:217 | 8 | 4 | .caption2.bold | 0.15 |
| DeliveryPreflight badge | DeliveryPreflightReportView:20 | 8 | 4 | .caption.bold | 0.15 |
| **Proposed unified** | StatusCapsule | **8** | **3** | **.caption2.bold** | **0.15** |
