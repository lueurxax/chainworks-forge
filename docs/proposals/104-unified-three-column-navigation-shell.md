# Proposal 104: Unified Three-Column Navigation Shell and Sidebar-Toggle Consolidation

| Field | Value |
|---|---|
| Date | 2026-06-23 |
| Status | Draft |
| Author | Engineering (operator-shell chrome) |
| Depends on | Proposal 074 (UI design system / brand migration), [`docs/brand/ui_kits/macos/App.jsx`](../brand/ui_kits/macos/App.jsx), [UI action boundary](../reference/ui-action-boundary.md), GraphQL thin-read contract |
| Target State | Native macOS SwiftUI client over GraphQL read projections; one window-level navigation container; no nested `NavigationSplitView` anywhere in the operator shell |
| Goal | Replace the current custom `HStack` app-nav shell + per-tab `NavigationSplitView`s with a single 3-column `NavigationSplitView` (app-nav │ list │ detail) + inspector, so the window exposes **one** native leading sidebar toggle instead of two look-alike collapse controls — without re-introducing the nested-split-view crash. |

---

## 1. Why this proposal exists

The operator window currently presents **two visually near-identical left-side collapse controls**:

1. the **app-navigation menu** collapse — a custom button in `P036ShellSidebar` (`ContentView.swift`), icon `sidebar.leading`; and
2. the **runs-list** collapse — the native sidebar toggle auto-injected by `RunsHomeView`'s own `NavigationSplitView`.

Operators read these as duplicate "hamburgers." The root cause is structural: the shell is a custom `HStack` app-nav sidebar wrapping content, and the `.runs` tab is itself a `NavigationSplitView` — i.e. **two independent sidebars → two toggles**.

Two prior attempts to consolidate them failed at runtime:

- A native `TabView(.sidebarAdaptable)` shell, and
- a direct outer `NavigationSplitView` with per-tab content in the detail column,

both **crash on sidebar collapse** with:

```
*** Terminating app due to uncaught exception 'NSGenericException',
reason: 'The window has been marked as needing another Update Constraints in Window pass…'
… -[NSSplitViewController _collapse:splitViewItem:forceOverlay:completionHandler:]
… SwiftUI.SplitViewCoordinator.updateSidebarItem(…setCollapsed:)
```

Both `TabView(.sidebarAdaptable)` and `NavigationSplitView` are backed by `NSSplitView` on macOS. Nesting `RunsHomeView`'s `NavigationSplitView` (and the `Definitions` tab's `AgentCatalogView`/`WorkflowInspectorView`, which each own a `NavigationSplitView`) inside another split-view shell produces an `NSSplitView`-in-`NSSplitView` hierarchy whose constraint-update pass recurses past the window's view-count limit during the collapse live-resize. The custom `HStack` shell avoided the crash only because `HStack` is not an `NSSplitView`.

**Therefore the only safe path to a single leading toggle is to have exactly one `NavigationSplitView` in the window and remove every nested one.** This proposal makes that the explicit architecture and invariant.

The intended visual is already canonical in the repo: `docs/brand/ui_kits/macos/App.jsx` (sidebar + content + inspector).

---

## 2. Principles

1. **One split per window.** Exactly one `NavigationSplitView` exists in the operator shell. **No nested `NavigationSplitView` is permitted** under it — this is the load-bearing invariant that prevents the constraint-recursion crash.
2. **One native leading toggle.** Menu/list collapse is expressed through the single split's `NavigationSplitViewVisibility`; the custom app-nav collapse button and any per-tab native sidebar toggle are removed.
3. **Boundaries unchanged.** Thin GraphQL read boundary and blessed-command mutations (e.g. `settleApproval`) are preserved; no orchestration, ACP, MCP, or daemon changes.
4. **Behaviour parity.** Tab routing (`chainworksSelectTab`, deep links, `Cmd+1..4`, MenuBarExtra), single-active-run-per-idea, and run/idea/definition/settings functionality are preserved.
5. **Brand-first, operation-first.** Match `docs/brand/ui_kits/macos/App.jsx`; visuals support the run lifecycle.
6. **Verify before merge.** Nothing merges to `main` until the collapse drill (below) passes on **every** tab.

---

## 3. Scope of work

### In scope

1. **Shell rewrite (`ContentView.swift`)** — replace `P036MainShell` (custom `HStack`) + `P036ShellSidebar` with a single 3-column `NavigationSplitView`:
   - **Column 1 (sidebar)** — app navigation (`Runs`/`Ideas`/`Definitions`/`Settings`) as a native `List(selection:)` with `Section`s (`Workspace`/`Catalog`); brand mark + approval/blocked/running counts surfaced via sidebar header/footer.
   - **Column 2 (content)** and **Column 3 (detail)** — driven by the selected tab (see per-tab fitting below).
   - `.inspector(isPresented:)` — the existing Run snapshot / Approval gate / Recovery surface.
2. **`RunsHomeView` un-nest** — dissolve its internal `NavigationSplitView` so the runs list becomes the shell's column 2 and run detail becomes column 3; the inspector moves to the shell-level `.inspector`. The 7-section switcher stays in the detail pane (not the toolbar).
3. **`DefinitionsView` un-nest** — `AgentCatalogView` and `WorkflowInspectorView` currently each own a `NavigationSplitView`. Convert their master/detail into a **non-split** form (e.g. selection-driven `List` feeding the shared detail column, or `NavigationStack`) so no nested split exists.
4. **`SettingsView` / `Ideas` fitting** — `SettingsView` (segmented `List`s) and the Ideas master-detail (`HStack`, already non-split) render within the shared content/detail columns without introducing a split.
5. **Toggle consolidation** — remove `p036-main-navigation-collapse-button` and any per-tab native sidebar toggle; collapse is the single split's leading toggle + `columnVisibility`.
6. **Accessibility / UI-test contract** — update `Chainworks ForgeUITests/AppScreen.swift` tab selection to native title-based matching; preserve per-tab root identifiers (`runs-home-owner-view`, `run-detail-panel`, `ideas-root-view`, `definitions-view`, `settings-view`, `approval-approve-button`, etc.) and the `Refresh` label.
7. **Crash verification harness** — a manual/automated collapse drill on every tab (see §6).

### Out of scope

1. Backend/daemon, ACP, MCP, GraphQL contracts, orchestration, mutation model.
2. New workflows or feature flags.
3. Features already shipped independently and unaffected: the trailing **Inspector** (Run snapshot/Approval gate/Recovery), approval **reorder/swipe**, and **Liquid Glass** adoption. These are reused, not redesigned here.
4. Visual restyle of run-detail cards beyond what unification requires.

---

## 4. Canonical references

- `docs/brand/ui_kits/macos/App.jsx`, `Shell.jsx`, `Inspector.jsx`, `RunDetail.jsx`
- Proposal 074 (UI design system refresh)
- `Chainworks Forge/ContentView.swift` (`P036MainShell`, `P036ShellSidebar`, `Tab`, `selectedTabContent`, routing)
- `Chainworks Forge/Views/RunsHomeView.swift` (`NavigationSplitView`, `.inspector`, toolbar)
- `Chainworks Forge/Views/DefinitionsView.swift`, `AgentCatalogView.swift`, `WorkflowInspectorView.swift` (nested `NavigationSplitView`s to remove)
- `Chainworks ForgeUITests/AppScreen.swift` (tab-selection page object)

---

## 5. Required implementation outcomes (phased; worktree-isolated)

Each phase must `BuildProject` clean, render via `RenderPreview`, and pass the collapse drill for the surfaces it touches before the next phase starts. Nothing merges to `main` until §6 passes on all tabs.

### 5.1 Phase 1 — Runs columns
Single 3-column `NavigationSplitView` shell with app-nav as column 1; `RunsHomeView` un-nested into columns 2/3 + shell `.inspector`. Verify: one leading toggle; collapse menu and list without `NSException`.

### 5.2 Phase 2 — Definitions un-nest
Remove the `NavigationSplitView` from `AgentCatalogView` and `WorkflowInspectorView`; render Agents/Workflows master/detail inside the shared columns. Verify: switch to Definitions, collapse the sidebar — no crash.

### 5.3 Phase 3 — Settings / Ideas fitting
Render `SettingsView` and the Ideas surface inside the shared columns without a nested split. Verify collapse on both.

### 5.4 Phase 4 — Contract + verification
Update `AppScreen.swift`; preserve/migrate accessibility identifiers; run `./scripts/test-gate.sh fast` and the remote `ui-smoke` gate; run the full collapse drill (§6).

---

## 6. Acceptance criteria

1. **Single toggle.** The window exposes exactly one native leading sidebar toggle; the custom `p036-main-navigation-collapse-button` is gone.
2. **No nested split.** A source scan confirms exactly one `NavigationSplitView` in the operator shell and zero nested ones (Runs/Definitions/Settings/Ideas).
3. **Collapse drill — no crash.** On each tab (Runs, Ideas, Definitions, Settings), launching via `RunProject` and collapsing/expanding the sidebar produces no `NSGenericException` / "Update Constraints in Window pass" in the console.
4. **Behaviour parity.** `Cmd+1..4`, `chainworksSelectTab`, deep links, MenuBarExtra still switch tabs; approvals still settle through `settleApproval`; single-active-run-per-idea preserved.
5. **A11y / UI-tests.** Per-tab root identifiers preserved; `AppScreen.selectTab` works against native sidebar items; `./scripts/test-gate.sh fast` green and remote `ui-smoke` green.
6. **Visual conformance.** Layout matches `docs/brand/ui_kits/macos/App.jsx`; verified via `RenderPreview` (light + dark).

---

## 7. Risks

1. **Nested-split crash (primary).** The whole point. Mitigation: the §2.1 invariant (one split, zero nesting) + the §6.3 per-tab collapse drill as a hard gate. This is the failure already observed twice; treat any nested `NavigationSplitView` as a release blocker.
2. **Definitions/Settings un-nest is real work and risk.** Their master/detail must be reimplemented without a split. Mitigation: phase isolation; convert one tab at a time with a build+render+collapse check.
3. **Heterogeneous column counts.** Tabs differ in whether they need a content column; a phantom empty middle column on non-Runs tabs must be avoided via per-selection column configuration. Mitigation: drive `columnVisibility`/content per selected tab and verify each.
4. **Environment verification flakiness.** Local builds/runs intermittently fail at the Rust daemon-embed phase (OOM under concurrent load), and preview rendering is intermittent. Mitigation: worktree isolation, retry in memory windows, prefer prebuilt-daemon path for the gate.
5. **UI-test churn (remote-only).** Tab-selection and any shell-identifier assertions change; UI tests run only on the approved remote host. Mitigation: update `AppScreen.swift` to title-based matching and schedule a remote `ui-smoke` run before merge.

---

## 8. Completion signal

The operator window has a single native leading sidebar toggle; collapsing/expanding the sidebar on every tab raises no exception; tab routing, approvals, and per-tab functionality are unchanged; `fast` and remote `ui-smoke` gates are green; and the layout matches the brand `App.jsx`. An implementation audit trail (`101-…_IMPLEMENTATION_AUDIT_RN.md`) records the per-phase collapse-drill evidence.
