# Proposal 012: UI Quality Audit and Visual Polish

| Field | Value |
|---|---|
| Date | 2026-03-26 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [007-full-mvp-delivery-slice](007-full-mvp-delivery-slice-worktrees-implementation-loop-manual-release-and-dogfooding.md), [008-mvp-hardening-and-sign-off](008-mvp-hardening-and-sign-off.md) |
| Scope | UI/UX visual quality, consistency, and polish across all 30 SwiftUI view files |
| Goal | Elevate the Chainworks Forge UI from functional prototype to production-quality macOS citizen by fixing truncation, information density, visual consistency, and establishing a lightweight design system. |

---

## 1. Context

The current UI was built incrementally across Proposals 002 through 011.
Each proposal correctly prioritised runtime behaviour and correctness over visual polish.
The result is a **functionally complete but visually inconsistent** interface that has accumulated multiple categories of UI debt:

- Severe text truncation in sidebars making content unreadable.
- Information-dense screens with no visual hierarchy.
- Inconsistent badge, color, and typography patterns across views.
- No formal design system — colors, fonts, spacing are all ad-hoc.
- Missing empty-state experiences and visual feedback cues.

This proposal catalogues every issue found during a systematic Xcode Preview audit of all 12 previewable surfaces and code review of all 30 view files, then prescribes a structured remediation plan.

### 1.1 Audit Methodology

All issues were identified via:

1. **Xcode Preview rendering** of all 12 `#Preview` definitions at their declared frame sizes.
2. **Code review** of all 30 SwiftUI view files in `Views/`, plus `ContentView.swift` and `Chainworks_ForgeApp.swift`.
3. **Cross-view consistency analysis** comparing badge styles, color usage, font scales, and layout patterns.

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

#### C-02: IdeaListView sidebar — narrow column truncates titles

**Observed:** IdeaListView explicitly sets `.navigationSplitViewColumnWidth(min: 200, ideal: 250)` which is too narrow for idea titles + lifecycle badges + attachment/run indicators.

**Fix:** Increase to `min: 260, ideal: 320`. Match the pattern used by `ArchivedIdeasView` (which correctly uses `min: 260, ideal: 320`).

**Affected file:** `Views/IdeaListView.swift` (line 80)

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
        case regular // caption, px:8/py:3
    }

    var body: some View {
        Text(text)
            .font(size == .small ? .caption2.bold() : .caption.bold())
            .padding(.horizontal, size == .small ? 6 : 8)
            .padding(.vertical, size == .small ? 2 : 3)
            .background(color.opacity(0.15), in: Capsule())
            .foregroundStyle(color)
    }
}
```

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

**Affected files:** All 30 view files.

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
- "Select an idea" → plain text without icon
- "Select an archived idea" → `archivebox`
- "No Pending Approvals" → `checkmark.seal`

These are generic and create a flat, utilitarian feel.

**Fix:**
1. Use `.symbolRenderingMode(.multicolor)` where available for richer icons.
2. Add `.font(.system(size: 48))` to empty-state icons for better visual presence.
3. Consider adding subtle hint text with specific call-to-action: "Create your first run" with a button vs. just "No Runs".

**Affected files:** `RunsHomeView.swift`, `IdeaListView.swift`, `ArchivedIdeasView.swift`, `ApprovalGateView.swift`

---

#### L-02: Ideas detail pane shows plain text "Select an idea"

**Observed:** When no idea is selected, the detail pane shows `Text("Select an idea").foregroundStyle(.secondary)` (IdeaListView line 103–104). This is not a `ContentUnavailableView` like other screens, creating inconsistency.

**Fix:** Replace with:
```swift
ContentUnavailableView(
    "Select an idea",
    systemImage: "lightbulb",
    description: Text("Choose an idea from the sidebar to view details and launch runs.")
)
```

**Affected file:** `Views/IdeaListView.swift` (lines 102–105)

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

#### L-09: No keyboard shortcuts for primary actions

**Observed:** No views declare `.keyboardShortcut()` on primary action buttons. The approval buttons, recovery actions, and run start actions could benefit from keyboard shortcuts for power users.

**Fix:** Add `.keyboardShortcut(.return, modifiers: .command)` to primary confirmation buttons and `.keyboardShortcut(.escape)` to cancel/dismiss buttons.

**Affected files:** `ApprovalGateView.swift`, `ReleaseGateView.swift`, `FirstRunSetupWizard.swift`, `RecoverySheet.swift`

---

#### L-10: RunDetailPanel — actions at bottom are easy to miss

**Observed:** In `RunDetailPanel`, the contextual action buttons (Recover, Compare, View Report, Export Evidence Pack) appear at the very bottom of a `ScrollView`. When the stage list is long, these actions are below the fold.

**Fix:** Move action buttons into the toolbar or a floating footer outside the ScrollView, ensuring they're always visible.

**Affected file:** `Views/RunsHomeView.swift` (lines 536–570)

---

#### L-11: DeliveryPreflightReportView rendered at minimum size

**Observed:** The Delivery Preflight Report preview renders at only 520px wide with no explicit height. The view itself is compact and well-designed, but when used as a sheet or inline panel, it should have a minimum frame for comfortable reading.

**Fix:** Add `.frame(minWidth: 480, minHeight: 300)` when presented in sheets.

**Affected file:** `Views/DeliveryPreflightReportView.swift`

---

#### L-12: No loading/progress states during async operations

**Observed:** Multiple views trigger async operations (provider diagnostics refresh, preflight checks, sample run launch, Goose verification) but show no visual loading indicator during the operation. The `isProbing` state in GooseAssistant disables buttons but shows no spinner.

**Fix:** Add `ProgressView()` inline or as an overlay during async operations:
```swift
if isProbing {
    ProgressView("Verifying Goose connection...")
        .padding()
}
```

**Affected files:** `PilotReadinessView.swift`, `ProviderSettingsView.swift`, `GooseProviderConnectionAssistantView.swift`, `FirstRunSetupWizard.swift`

---

## 3. Design System Foundation

### 3.1 Proposed File Structure

```
Support/
├── DesignTokens.swift       // Colors, spacing, corner radii
├── StatusCapsule.swift       // Reusable badge component
├── EmptyStateView.swift      // Standardized empty state wrapper
└── PreviewSupport.swift      // (existing)
```

### 3.2 Spacing Tokens

| Token | Value | Usage |
|---|---|---|
| `compact` | 4pt | Tight inline spacing |
| `small` | 8pt | Between related items |
| `medium` | 12pt | Between sections within a group |
| `large` | 16pt | Between GroupBoxes/sections |
| `section` | 20pt | Between major content blocks |

### 3.3 Corner Radius Tokens

| Token | Value | Usage |
|---|---|---|
| `badge` | `Capsule()` | Status capsules, tags |
| `card` | 14pt continuous | Stage cards, agent panels |
| `panel` | 16pt continuous | Larger containers |
| `sheet` | system default | Sheets, popovers |

---

## 4. Implementation Plan

### Phase 1: Critical Fixes (estimated: 1 day)
- [ ] C-01: Fix RunsHomeView sidebar width and row density
- [ ] C-02: Fix IdeaListView sidebar width
- [ ] M-04: Fix banner animation direction
- [ ] L-02: Fix Ideas detail pane empty state

### Phase 2: Design System Extraction (estimated: 1 day)
- [ ] M-01: Extract `StatusCapsule` component
- [ ] M-02: Create `DesignTokens` with semantic colors
- [ ] M-03: Apply consistent typography scale
- [ ] Migrate all views to use shared components

### Phase 3: Visual Hierarchy Improvements (estimated: 2 days)
- [ ] H-01: Restructure ProviderSettingsView
- [ ] H-02: Add hero status to PilotReadinessView
- [ ] H-03: Add step progression to FirstRunSetupWizard
- [ ] H-04: Simplify RunsHomeRow for sidebar density

### Phase 4: Polish and Experience (estimated: 1–2 days)
- [ ] L-01: Enhance empty states
- [ ] L-03: Redesign IdeaListView summary strip
- [ ] L-04: Add journey visualization to GooseAssistant
- [ ] L-05: Make WorkflowMap topology cards interactive
- [ ] L-06: Fix ReleaseGate review status semantics
- [ ] L-07: Convert New Idea sheet to Form
- [ ] L-09: Add keyboard shortcuts
- [ ] L-10: Float RunDetailPanel actions
- [ ] L-12: Add loading states for async operations

---

## 5. Verification Criteria

Each fix must be verified via:

1. **Xcode Preview rendering** at the declared preview frame size — no truncation, no overflow.
2. **Minimum window size test** — resize to 1024×768 and verify all content remains usable.
3. **Cross-view consistency** — badges, colors, and fonts must match the design tokens after migration.
4. **Accessibility audit** — VoiceOver must read status information, not just visual cues.

---

## 6. Out of Scope

- Custom illustrations or branded iconography (post-MVP).
- Light mode support (needs separate design pass).
- Animation/motion design beyond fixing the banner transition bug.
- Localization readiness (separate proposal).
- iOS/iPadOS adaptation (macOS-only for MVP).

---

## 7. Risk Assessment

| Risk | Mitigation |
|---|---|
| Design system extraction causes regression | Phase 2 changes are purely visual — no business logic changes. Each file migrated independently. |
| Sidebar width changes break existing layouts | Test with preview seeds at min/ideal/max widths. |
| Wizard step progression changes break UI tests | Maintain existing `accessibilityIdentifier` values on all elements. |
| Time overrun on polish items | Phase 4 items are independently shippable; any subset improves quality. |

---

## Appendix A: Full Screenshot Audit Reference

| Screen | Preview Name | Key Issues |
|---|---|---|
| ContentView (tab shell) | `Content Shell — Seeded` | 7 tabs, banner overlap |
| RunsHomeView | `Runs Home — Mixed States` | C-01, H-04 (truncation, row density) |
| IdeaListView | `Ideas — Operator List` | C-02, L-02, L-03 (narrow sidebar, plain empty state, dense strip) |
| ProviderSettingsView | `Provider Settings — Configured` | H-01 (information overload) |
| PilotReadinessView | `Pilot Readiness — Seeded` | H-02 (wall of text) |
| FirstRunSetupWizard | `First Run Setup — Seeded` | H-03 (no step progression) |
| ArchivedIdeasView | `Archived Ideas — Seeded` | L-01 (generic empty state) |
| DeliveryPreflightReportView | `Delivery Preflight — All Passing` | L-11 (minimum frame) |
| GooseProviderConnectionAssistantView | `Goose Assistant` | L-04 (no journey visualization) |
| ReleaseGateView | `Release Gate — Sandbox` | L-06 (Missing vs Not Yet Produced) |
| RunStartOverridesView | `Override List — 8 agents` | Adequate — no critical issues |
| WorkflowMapView | `Workflow Map — Proposal Loop` | L-05 (static topology cards) |

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
