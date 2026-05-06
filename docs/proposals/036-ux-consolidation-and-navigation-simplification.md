# Proposal 036: UX Consolidation and Navigation Simplification

| Field | Value |
|---|---|
| Date | 2026-04-09 |
| Status | Draft |
| Author | Andrey Khasanov |
| Depends on | [../reference/domain-model.md](../reference/domain-model.md), [../reference/query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md), [021-run-transition-notifications-and-attention-routing.md](021-run-transition-notifications-and-attention-routing.md) |
| Scope | Reduce the seven-tab navigation to four tabs by eliminating view duplication, inlining approvals into their natural contexts, merging reference views, and absorbing readiness checks into Settings. |
| Goal | Every tab owns a single clear purpose. The operator never sees the same information in two places and never needs more than two clicks to reach any surface. |

---

## 1. Context and Motivation

The current Chainworks navigation shell exposes seven top-level tabs:

| # | Tab | Purpose |
|---|-----|---------|
| 1 | Runs Home | Operator dashboard: waiting approval, blocked, running, completed |
| 2 | Ideas | Create/manage ideas, launch runs, view idea-scoped run history |
| 3 | Approvals | Standalone approval inbox |
| 4 | Agent Catalog | Browse agents.yaml, inspect validation |
| 5 | Workflow Inspector | Browse workflow.yaml, inspect states |
| 6 | Pilot Readiness | Pre-flight checklist: config paths, provider health, capabilities |
| 7 | Settings | Provider management, Goose server, first-run wizard, import/export |

This structure was reasonable when each feature shipped independently, but it now creates real friction:

1. **Runs Home and Ideas show overlapping run information.** Runs Home groups runs by status. Ideas shows the same runs nested under their parent idea with the same status badges, approval bars, and action buttons. The operator sees the same waiting-approval run in both places and must decide which surface to act from.

2. **Approvals is a dead-end tab.** The approval inbox shows pending requests, but Runs Home already has a "Waiting Approval" section with an "Approve Gate" action, and Ideas shows an approval bar on ideas with pending runs. Three surfaces for one action.

3. **Agent Catalog and Workflow Inspector are reference views used infrequently.** Neither is part of the operator's daily loop. Giving each a top-level tab inflates the sidebar and pushes the two reference views apart when they are most useful together (validating a catalog + workflow pair before launching a run).

4. **Agent Catalog shows a flat list.** With 15+ agents across orchestration, review, implementation, release, quality, and steward roles, a flat list makes it hard to find the right agent or understand the system's structure at a glance.

5. **Workflow Inspector does not sort by execution order.** States appear in source-file order, which does not match the actual execution flow. The operator must mentally reconstruct the path through transitions.

6. **Pilot Readiness duplicates Settings.** Both show provider health status, Goose server management, and configuration paths. The operator checks health in Pilot Readiness, then switches to Settings to fix an issue, then switches back to verify the fix.

The combined effect is a navigation that feels wider than it needs to be, with information scattered across tabs instead of consolidated where the operator actually works.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. Can the operator manage all run-lifecycle actions (monitor, recover, approve, compare, report) from a single Runs surface without needing to visit Ideas for run context?
2. Can the operator manage all idea-lifecycle actions (create, configure, archive, launch) from a single Ideas surface without needing to visit Runs Home for run status?
3. Can approvals be resolved inline in both Runs and Ideas without a separate Approvals tab?
4. Can the operator browse both agents and workflow states from one "Definitions" surface with one extra click?
5. Can agents be browsed by logical group rather than as a flat list?
6. Can workflow states be displayed in execution order rather than source order?
7. Can readiness checks be accessed from Settings without a separate top-level tab?
8. Does the reduced tab count (four tabs) feel faster for the daily operator loop?
9. Can the Live Timeline show tool calls as single merged cards instead of start/finish pairs?
10. Does batching timeline updates at 2-second intervals eliminate visible UI jitter during heavy tool use?
11. Does the timeline automatically clean up tool noise when an agent switches to text output?
12. Does agent completion produce a single summary card instead of a scroll of 30+ individual events?

---

## 3. Scope

This proposal includes:

- Consolidation of the seven-tab shell into four tabs.
- Clear responsibility assignment for each remaining tab.
- Inline approval resolution in Runs and Ideas views.
- A merged "Definitions" tab combining Agent Catalog and Workflow Inspector.
- Agent grouping by functional role within the catalog.
- Execution-order sorting in the Workflow Inspector.
- Absorption of Pilot Readiness into Settings as a collapsible section.
- Removal of the standalone Approvals tab.
- Removal of the standalone Pilot Readiness tab.
- Migration of the `ContentView.Tab` enum and associated routing.
- Live Timeline rework: tool card merging, batched UI updates, tool cleanup on text arrival, agent collapse on finish.

This proposal does **not** include:

- Changes to data models (`Run`, `Idea`, `Approval`, `Artifact`).
- Changes to `ExecutionService` or engine-layer execution logic.
- Changes to agent.yaml or workflow.yaml schema.
- New features beyond reorganization (no new data surfaces, no new actions).
- Mobile or compact layout adaptation (macOS primary).

### 3.1 Thin UI visual-parity handoff

The GraphQL-only thin UI boundary is established in [query-projections-and-client-consumption-contract.md](../reference/query-projections-and-client-consumption-contract.md). P036 now owns the visual and navigation restoration work over that read model.

Visual baseline commit: `1cca56b9abd622ad7dc4e38304985cbf49e66780` (`2026-04-19T19:05:19+03:00`, `Add P060: lead-driven reviewer routing and expanded reviewer catalog`).

This is the last pre-control-plane visual/ergonomic baseline selected for P036 comparison work. It is the parent of the first large control-plane land commit `a17b1cd04ac38f46f61111c647911f03844b4a33` from 2026-04-21. Use it only as evidence for visual/ergonomic behavior, not as the target UI. Old Swift-local mutation operations from that baseline are not part of P036 unless a separate write-transport proposal approves them.

P036 must not produce "the old UI restored." The implementation target is the final P036 design over the current GraphQL read model. The old baseline is useful because it shows screens and inspection affordances that existed before the control-plane cutover, but every baseline delta must be reinterpreted through this proposal's navigation, definitions, read-only approval, and thin-client constraints.

Before implementation changes, P036 must produce a short prep artifact under `docs/proposals/036-artifacts/` with this comparison sequence:

1. **Current vs pre-control-plane baseline.** Compare the current UI against commit `1cca56b9abd622ad7dc4e38304985cbf49e66780` and list lost, degraded, unchanged, and intentionally removed screen affordances. This pass is descriptive only; it must not mark old affordances as automatically required.
2. **Pre-control-plane baseline vs P036 target.** Compare the old UI against this proposal's target design and classify old affordances as `carry_forward`, `replace_with_p036_design`, `drop_as_legacy_write_path`, or `defer_until_projection_exists`.
3. **Current UI vs P036 target.** Revisit the current-vs-old differences through the P036 target classification and produce the actual implementation backlog. Only this third pass is implementation-authoritative.

The prep artifact must include file/symbol references for current screens, git references for baseline screens, and a decision rationale for every old affordance that is not carried forward. The backlog must avoid two separate visual migrations: P036 should move directly from the current UI to the final proposal-shaped UI, not first recreate the old UI and then redesign it.

P036 must restore or consciously replace these thin-client stop-tail surfaces:

| Area | Required visual/product outcome |
| --- | --- |
| Runs Home | Restore stronger scanability: status/attention lanes, clear selected row treatment, dense but readable row cards, run title clarity, progress/status chips, and idea origin context. |
| Run detail | Rebuild a proper inspection layout instead of a single proof-of-read stack: summary, stages, transitions, artifacts, reports, approvals diagnostics, recovery/evidence, and daemon state should be visually distinct but not nested-card heavy. |
| Stage cards and transitions | Replace the current thin vertical transition list with a richer GraphQL-backed stage map/card treatment inspired by the old `WorkflowMapView`: stage cards, current-stage emphasis, transition arrows/labels, loop progress, attempt/iteration metadata, and compact occurrence summaries. |
| Handoffs and agent execution context | Bring back readable handoff/agent panels when GraphQL exposes enough projection data; otherwise show explicit deferred states. |
| Artifact hierarchy | Productize the GraphQL artifact browser: independent list/document scrolling, grouping by stage/agent/type, filters, search, promoted/latest artifacts, status badges, and predictable detail selection. |
| Artifact rendering | Preserve content-based format detection: a `.json` artifact may contain markdown/plain text and must render by payload shape, not filename alone. Markdown and JSON rendering should remain readable in a split inspector. |
| Ideas/catalog surfaces | Restore GraphQL-only idea browsing and catalog/definitions inspection so the operator can identify the idea/run context and inspect workflow/agent definitions without leaving the app. |
| Definitions | Fold Agent Catalog and Workflow Inspector into the P036 Definitions design, including agent grouping and workflow execution-order sorting. |
| Approvals | Keep the governed thin UI read-only for non-approval commands, with only the approval mutation exception defined by [ui-action-boundary.md](../reference/ui-action-boundary.md), while making approval context visually useful in the Runs flow. |

Acceptance for this handoff:

- The app remains GraphQL-read-only for governed workflow truth.
- No UI MCP calls, GraphQL mutations, local workflow mutations, raw artifact directory truth, or old Swift orchestrator fallback are reintroduced.
- Visual parity is judged against operator inspection ergonomics and the P036 target design, not against pixel or layout equality with the old baseline and not against removed write controls.
- The `docs/proposals/036-artifacts/` prep artifact exists before code changes and proves the three comparison passes above were performed.
- Every visual change traces to the third-pass implementation backlog, not directly to the old baseline.
- Dogfood should not be claimed until the restored P036 surfaces let an operator identify the idea, inspect run/stage state, inspect artifacts/reports, understand catalog/workflow context, and diagnose daemon/read freshness from the app.

---

## 4. Problem Statement

### 4.1 Runs Home and Ideas show the same runs with the same actions

Runs Home groups all runs by status (`waitingApproval`, `blocked`, `running`, `completed`). Ideas shows those same runs nested under their parent idea, with status badges, approval bars, and inline action buttons.

An operator looking at a waiting-approval run sees it in:
- Runs Home "Waiting Approval" section with an "Approve Gate" button,
- Ideas approval bar with a pending-approval count,
- Ideas detail view with per-run status and approval actions.

The operator must choose which surface to use. Neither surface is wrong, but neither is clearly the canonical place to act.

### 4.2 Approvals exist in three places

The standalone Approvals tab (`ApprovalInboxView`) shows pending requests with context artifacts and approve/reject controls. But the same approval action is available from:
- Runs Home via the `resolveApprovalGate` action,
- Ideas via the inline approval bar and run detail.

Three surfaces for the same action creates decision overhead without adding capability.

### 4.3 Agent Catalog and Workflow Inspector serve the same audience at the same time

Both are reference/validation views used when preparing or debugging a run configuration. They are rarely used during active run monitoring. Giving each a top-level tab adds two entries to the sidebar that the operator scrolls past 90% of the time.

### 4.4 Agents appear as a flat unsorted list

The catalog currently shows all agents in a single `NavigationSplitView` list sorted by agent ID. With agents spanning orchestration, review (PO, UX, UI, Architect), implementation, audit, release, docs, and steward roles, the list requires scanning to find a specific agent.

### 4.5 Workflow states are not sorted by execution order

The Workflow Inspector lists states in the order they appear in the YAML source file. This may or may not match the actual transition graph. The operator must read transition targets and mentally reconstruct the execution path.

### 4.6 Pilot Readiness and Settings show overlapping provider/config data

Pilot Readiness shows:
- Configuration paths (workflow, catalog, run storage, Goose URL, worktree base),
- Managed Goose server state and startup control,
- Provider health snapshots (name, family, health, blocking issues, check time),
- Capabilities validation.

Settings shows:
- Managed Goose server section (identical),
- Provider health status (identical),
- Configuration and diagnostic refresh.

The operator diagnoses a problem in Pilot Readiness, switches to Settings to fix it, then switches back to verify. The two views share the same underlying data (`GooseServerManager`, `ProviderRegistry.healthSnapshot`).

### 4.7 Live Timeline is noisy and causes UI jitter

The Live Timeline (`RunTimelineInspectorView`) renders every `ExecutionEvent` as a separate card. Tool calls produce two cards each (`toolCallStarted` + `toolCallFinished`). During a Code Writer session that performs 20 file operations, this means 40+ tool cards scroll past in rapid succession, interspersed with text chunks.

Current throttling: only `textChunk` events are coalesced (350ms window). All other events — including tool calls — commit directly to `liveTimeline` and trigger an immediate SwiftUI re-render. The spring animation (0.45s response) is fast enough to complete before the next event arrives, but barely. The cumulative effect is a flickering, hard-to-read stream.

Additionally, tool cards remain in the timeline indefinitely (up to the 40-entry cap), so old tool results from a previous phase sit alongside current text output, making it hard to find the latest agent response.

---

## 5. Core Product Behavior

### 5.1 New four-tab navigation

Replace the seven-tab `ContentView.Tab` enum with four tabs:

| # | Tab | Responsibility | Contains |
|---|-----|---------------|----------|
| 1 | **Runs** | Monitor and act on active/recent runs | Current Runs Home + inline approval resolution |
| 2 | **Ideas** | Create, configure, and launch ideas | Current Ideas (stripped of run-monitoring duplication) |
| 3 | **Definitions** | Browse and validate agent catalog + workflow | Agent Catalog + Workflow Inspector (segmented) |
| 4 | **Settings** | Provider config, readiness, system health | Current Settings + absorbed Pilot Readiness |

The default landing tab remains **Runs** (operator dashboard, per P005-OPS SS5).

### 5.2 Runs tab: single source of truth for run lifecycle

The Runs tab keeps the current Runs Home structure (Waiting Approval, Blocked, Running, Recently Completed) and gains:

- **Inline approval panel.** When the operator selects a waiting-approval run, the detail pane shows the full approval context (preceding artifacts, comment field, approve/reject buttons, keyboard shortcuts). This replaces the need to navigate to a separate Approvals tab. The existing `ApprovalGateView` content is embedded directly in the run detail area.

- **Idea origin badge.** Each run row shows a small badge or subtitle linking back to its parent idea. Tapping the badge navigates to that idea in the Ideas tab. This preserves traceability without duplicating idea management.

The Runs tab does **not** show idea creation, idea archiving, or idea configuration.

### 5.3 Ideas tab: single source of truth for idea lifecycle

The Ideas tab keeps the current idea list/detail structure and gains:

- **Lightweight run status summary.** Instead of showing full run rows with action buttons, Ideas shows a compact status strip per idea: "2 completed, 1 running, 1 waiting approval". This gives context without duplicating the Runs tab's monitoring role.

- **Inline approval shortcut.** If an idea has a run waiting for approval, the status strip shows an "Approve" chip that deep-links to that run in the Runs tab (using the existing `chainworksOpenRunInRunsHome` notification). The operator approves in one place (Runs), not two.

- **Quick-launch action.** The primary action on an idea remains "Start Run", which creates the run and optionally navigates to the Runs tab to monitor it.

The Ideas tab does **not** show full run recovery flows, run comparison, or run reports.

### 5.4 Remove the standalone Approvals tab

Delete `ApprovalInboxView` as a top-level tab. The approval inbox concept survives as:

- The "Waiting Approval" section in Runs (already exists),
- The inline approval panel in run detail (new in SS5.2),
- The `ForegroundBannerView` attention banner (already exists, shows pending approval count and allows one-tap navigation).

If the operator wants a list of all pending approvals, the "Waiting Approval" section in Runs already provides that.

### 5.5 Definitions tab: merged Agent Catalog + Workflow Inspector

Create a new `DefinitionsView` with a segmented picker at the top:

```
[ Agent Catalog | Workflow ]
```

**Agent Catalog segment** retains the current `NavigationSplitView` (sidebar list + detail pane) but adds agent grouping (see SS5.6).

**Workflow segment** retains the current Workflow Inspector UI (full workflow / compact preview toggle, state list + detail) but adds execution-order sorting (see SS5.7).

Both segments share a toolbar area for file picker overrides and validation summary.

### 5.6 Agent Catalog: group by functional role

Group agents into collapsible sections based on their `mode` field or a new optional `group` field in agents.yaml:

| Group | Agents |
|-------|--------|
| **Orchestration** | Lead / Orchestrator |
| **Proposal** | Proposal Writer, Proposal Reviewer / PO, Proposal Reviewer / UX, Proposal Reviewer / UI, Proposal Reviewer / Architect |
| **Aggregation** | Review Summary Aggregator, Score Lift Aggregator, Feedback Coverage Checker, Fact Digest Validator |
| **Implementation** | Code Writer, Proposal vs Implementation Auditor |
| **Quality** | Security Checker, Pre-push Code Reviewer, Docs Quality Guardian |
| **Release** | Commit and Push to GitHub, Connect Publisher |
| **Steward** | Steward |

Grouping logic, in order of preference:

1. If the agent has an explicit `group` field in agents.yaml, use that.
2. Otherwise, derive the group from the `mode` field using a mapping table:
   - `orchestration` -> Orchestration
   - `proposal_authoring` -> Proposal
   - `proposal_review.*` -> Proposal
   - `aggregation.*` or `summary.*` -> Aggregation
   - `implementation` -> Implementation
   - `audit` -> Implementation
   - `security`, `prepush_review`, `docs` -> Quality
   - `release_git`, `release_publish` -> Release
   - `steward` -> Steward
3. Fallback group: "Other".

Each group header shows the count of agents and a collapse/expand toggle. The default state is all groups expanded.

### 5.7 Workflow Inspector: execution-order sorting

When displaying the state list in the Workflow Inspector, sort states by their position in the execution graph rather than source-file order:

1. Start from the workflow's `initial_state`.
2. Follow the primary transition path (first `transitions` entry per state, or `on_success` target).
3. Assign each visited state an ordinal position.
4. States not reachable from the primary path (error handlers, conditional branches) appear after the primary path, sorted alphabetically, with a visual separator.

Display the ordinal position as a number badge in the state list sidebar (e.g., "1", "2", "3"...) so the operator can immediately see the execution flow.

Optional: add a toggle "Sort by: Execution Order | Source Order" for cases where the operator wants to cross-reference the YAML file directly.

### 5.8 Settings tab: absorb Pilot Readiness

Add a new collapsible section to `ProviderSettingsView`:

```
v System Readiness
  Hero status banner (green/yellow/red overall readiness)
  Configuration paths (workflow, catalog, run storage, worktree base)
  Capabilities validation results
  [Refresh Readiness] button
```

This section uses the same data sources as the current `PilotReadinessView` but lives inside Settings. The existing Settings content (provider management, Goose server, first-run wizard, import/export) remains unchanged.

The "Refresh Readiness" action refreshes both readiness checks and provider health in a single operation, eliminating the current split between "refresh readiness" (Pilot Readiness) and "refresh diagnostics" (Settings).

### 5.9 ForegroundBannerView continues to work

The bottom attention banner (`ForegroundBannerView`) already shows approval/blocked/failed counts with tap-to-navigate. It should continue to work, navigating to the Runs tab (renamed from `runsHome` to `runs`).

### 5.10 Additional UX improvements

**5.10.1 Run detail as a sheet or inspector pane, not tab-internal navigation.**
Currently, opening a run detail in Runs Home pushes a NavigationStack destination. For frequent back-and-forth between the run list and run detail, an inspector-style side pane (already used by Agent Catalog) or a sheet would reduce navigation friction. The run list stays visible while the detail pane updates.

**5.10.2 Global keyboard shortcut for approvals.**
Since approvals are now inline rather than a dedicated tab, add a global keyboard shortcut (e.g., Cmd+Shift+A) that jumps to the next pending approval in the Runs tab. This preserves the "quick approval" workflow without needing a dedicated tab.

**5.10.3 Tab badges.**
Show unread/pending counts on tab icons:
- Runs: count of waiting-approval + blocked runs.
- Ideas: count of ideas with active runs (optional, lower priority).
- Definitions: validation error count (if any).
- Settings: health issue count (if any).

### 5.11 Live Timeline rework

The current Live Timeline (`RunTimelineInspectorView`) renders every `ExecutionEvent` as an individual card in a `GroupBox("Live Stream")`. This creates a noisy, fast-scrolling stream that is hard to follow during active agent execution. Three specific problems and their solutions:

#### 5.11.1 Merge tool call started/finished into a single card

**Current behavior:** `toolCallStarted` and `toolCallFinished` render as two separate cards. The operator sees:

```
┌─────────────────────────────────────┐
│ Code Writer           toolCallStarted│
│ Tool: file_write                     │
│ implementation · sess_abc · 14:32:01 │
└─────────────────────────────────────┘
┌─────────────────────────────────────┐
│ Code Writer          toolCallFinished│
│ Tool completed: file_write           │
│ implementation · sess_abc · 14:32:03 │
└─────────────────────────────────────┘
```

**New behavior:** Merge into a single card that shows the tool name, duration, and current state (in-flight or completed):

```
┌─────────────────────────────────────┐
│ Code Writer                    tool  │
│ ✓ file_write                   2.1s  │
│ implementation · sess_abc · 14:32:01 │
└─────────────────────────────────────┘
```

Implementation:

- When `toolCallStarted` arrives, create a new card with a spinner and the tool name. Track this card by `(agentID, toolName, requestID)`.
- When the matching `toolCallFinished` arrives, update the existing card in-place: replace the spinner with a checkmark, show elapsed duration.
- **30 seconds after `toolCallFinished`, remove the card entirely** (fade-out animation). This keeps the timeline clean while giving the operator time to notice which tools ran.
- If multiple tools are in-flight concurrently for the same agent, each gets its own tracked card.

Data model change in `WorkflowOrchestrator`:

```swift
/// Tracks in-flight tool cards for merge-on-finish.
private var liveToolCardsByKey: [ToolCardKey: Int] = [:]
// ToolCardKey = (agentID: String, toolName: String, requestID: String?)
```

#### 5.11.2 Batched UI updates at 2-second intervals

**Current behavior:** Events are committed to `liveTimeline` immediately on arrival (main thread). The only throttling is a 350ms coalescing window for consecutive `textChunk` events from the same agent. Non-text events (`toolCallStarted`, `toolCallFinished`, `sessionStarted`, etc.) bypass throttling entirely and trigger an immediate SwiftUI re-render per event. During heavy tool use (e.g., Code Writer running 20 file operations), this produces rapid-fire card insertions that cause visible UI jitter.

**New behavior:** Buffer all incoming events and flush to `liveTimeline` at a fixed 2-second tick:

1. All events arriving via `recordLiveExecutionEvent` go into a per-agent buffer instead of directly modifying `liveTimeline`.
2. A `Timer` fires every 2 seconds on the main actor.
3. On each tick, the buffer is drained: new cards are inserted, existing cards are updated (tool merge, text accumulation), and expired cards are removed.
4. One SwiftUI re-render per tick instead of per event.
5. The animation for the batch uses an `easeInOut` curve with a **1.5-second duration**, so the visual transition overlaps with the next tick and creates a smooth, continuous feel rather than discrete jumps.

```swift
// Replace current per-event commit with buffered approach
private var pendingEventBuffer: [(agentID: String, event: ExecutionEvent)] = []

private func startLiveTimelineTick() {
    liveTimelineTimer = Timer.scheduledTimer(withTimeInterval: 2.0, repeats: true) { [weak self] _ in
        Task { @MainActor [weak self] in
            self?.flushLiveTimelineBuffer()
        }
    }
}

private func flushLiveTimelineBuffer() {
    guard !pendingEventBuffer.isEmpty else { return }
    let batch = pendingEventBuffer
    pendingEventBuffer.removeAll()

    withAnimation(.easeInOut(duration: 1.5)) {
        for (agentID, event) in batch {
            applyEventToTimeline(agentID: agentID, event: event)
        }
    }
}
```

The existing 350ms text-chunk coalescing remains as a pre-filter before the buffer (reduces buffer size for high-frequency token streams). The 2-second tick is the UI-level gate.

#### 5.11.3 Tool history cleanup on text/non-tool events; agent collapse on finish

**Current behavior:** All event cards accumulate in the timeline up to the 40-entry cap, removed only when the cap is exceeded (oldest first). Tool cards from 5 minutes ago sit alongside the current text stream, making it hard to find the latest output.

**New behavior — two cleanup rules:**

**Rule A: On text arrival, clear preceding tool-only cards.**
When a `textChunk`, `finalOutput`, or any non-tool event arrives for an agent, all **tool cards** (`toolCallStarted` / `toolCallFinished` type) preceding it in the timeline are removed during the next batch flush. Non-tool cards (text, errors, session events) are preserved. This means the operator sees tools while they're running, but once the agent starts producing text output, the tool noise disappears and the focus shifts to the content.

```
Before text_chunk arrives:
  [tool: file_read ✓ 1.2s]  [tool: file_write ✓ 0.8s]  [tool: grep ✓ 0.3s]

After text_chunk arrives (next 2s flush):
  [Streaming output: "I've updated the AuthService to..."]
```

**Rule B: On agent finish, collapse all remaining cards into one summary card.**
When a `finish` or `sessionClosed` event arrives for an agent, all of that agent's cards in the timeline are replaced with a single summary card:

```
┌─────────────────────────────────────┐
│ Code Writer              completed   │
│ 12 tool calls · 3 text chunks       │
│ Duration: 2m 34s                     │
│ implementation · sess_abc · 14:34:35 │
└─────────────────────────────────────┘
```

The summary card records:
- Agent title and completion status (completed / failed / cancelled).
- Count of tool calls executed.
- Count of text chunks produced.
- Total agent duration (first event to last event).
- Stage and session metadata.

This means the timeline after a multi-agent stage shows one compact card per agent rather than a scroll of 30+ individual event cards.

#### 5.11.4 Updated timeline constants

| Constant | Current | New | Reason |
|----------|---------|-----|--------|
| `liveTextChunkCoalescingWindow` | 0.35s | 0.35s (unchanged) | Pre-filter before buffer |
| UI flush interval | immediate | 2.0s | Batch rendering |
| Animation duration | spring 0.45s/0.82 | easeInOut 1.5s | Smooth overlap between ticks |
| Tool card expiry after finish | none | 30s | Auto-cleanup |
| Max timeline entries | 40 | 40 (unchanged) | Still needed as backstop |
| Text accumulation limit | 2,000 chars | 2,000 chars (unchanged) | Memory bound |

#### 5.11.5 View changes

`RunTimelineInspectorView` changes:
- Replace `ForEach(projection.liveTimeline)` with a `ForEach` that handles three card variants: **tool card** (merged start/finish with optional spinner), **text card** (existing `StreamingTimelineTextView`), **agent summary card** (new, post-collapse).
- Remove per-entry `transition(.asymmetric(...))` and `animation(.spring(...))` modifiers. Instead, apply one `.animation(.easeInOut(duration: 1.5))` on the `VStack` containing the ForEach, driven by `projection.liveTimeline.map(\.id)`.
- Auto-scroll behavior (`onChange(of: projection.liveTimeline.count)`) uses the same 1.5s easeInOut.

`WorkflowOrchestrator` changes:
- Add `pendingEventBuffer` and 2-second timer.
- Add `liveToolCardsByKey` dictionary for tool merge tracking.
- Add `toolCardExpiryTimers` for 30-second post-finish removal.
- Modify `commitLiveExecutionEvent` to buffer instead of direct-commit.
- Add `flushLiveTimelineBuffer()` with batch application logic.
- Add `collapseAgentCards(agentID:)` for agent-finish summary generation.
- Add tool-card cleanup logic triggered by non-tool events.

---

## 6. Migration

### 6.1 ContentView.Tab enum

```swift
// Before
enum Tab: String, CaseIterable {
    case runsHome = "Runs Home"
    case ideas = "Ideas"
    case approvals = "Approvals"
    case agentCatalog = "Agent Catalog"
    case workflowInspector = "Workflow Inspector"
    case pilotReadiness = "Pilot Readiness"
    case providerSettings = "Settings"
}

// After
enum Tab: String, CaseIterable {
    case runs = "Runs"
    case ideas = "Ideas"
    case definitions = "Definitions"
    case settings = "Settings"
}
```

### 6.2 Notification routing

- `.chainworksSelectTab` payloads referencing removed tabs must be mapped:
  - `.approvals` -> `.runs` (focus on waiting-approval section)
  - `.agentCatalog` -> `.definitions` (activate Agent Catalog segment)
  - `.workflowInspector` -> `.definitions` (activate Workflow segment)
  - `.pilotReadiness` -> `.settings` (scroll to readiness section)
  - `.runsHome` -> `.runs`
  - `.providerSettings` -> `.settings`

### 6.3 UI test environment variables

`CHAINWORKS_UI_TEST_INITIAL_TAB` must accept both old and new tab raw values during a transition period, mapping old values to their new equivalents.

### 6.4 UISurface mapping

Existing `UISurface` cases remain valid. Their tab-switching behavior updates to target the new four-tab structure.

### 6.5 View file changes

| Action | File |
|--------|------|
| Rename + modify | `RunsHomeView.swift` -> `RunsView.swift` (add inline approval panel) |
| Modify | `IdeaListView.swift` (replace full run rows with compact status strip) |
| Delete as tab | `ApprovalGateView.swift` (inline the approval UI into RunsView) |
| Create | `DefinitionsView.swift` (segmented container for catalog + workflow) |
| Modify | `AgentCatalogView.swift` (add grouping) |
| Modify | `WorkflowInspectorView.swift` (add execution-order sorting) |
| Delete as tab | `PilotReadinessView.swift` (move content into ProviderSettingsView) |
| Modify | `ProviderSettingsView.swift` (add readiness section) |
| Modify | `ContentView.swift` (new Tab enum, new tab shell) |
| Modify | `ForegroundBannerView.swift` (update tab targets) |
| Modify | `RunTimelineInspectorView.swift` (three card variants, batched animation) |
| Modify | `WorkflowOrchestrator.swift` (event buffer, 2s tick, tool merge, agent collapse) |

---

## 7. Verification

### 7.1 Navigation contract

- App launches on Runs tab.
- Four tabs visible in sidebar: Runs, Ideas, Definitions, Settings.
- No orphan tabs (Approvals, Pilot Readiness) visible.

### 7.2 Approval flow

- Pending approval visible in Runs "Waiting Approval" section.
- Selecting a waiting-approval run shows inline approval panel with context, comment, approve/reject.
- Cmd+Return approves, Cmd+Delete rejects (preserved from current ApprovalGateView).
- ForegroundBanner tap navigates to the run in Runs tab.
- Ideas status strip shows "1 waiting approval" chip linking to Runs.

### 7.3 Definitions tab

- Segmented picker switches between Agent Catalog and Workflow.
- Agent Catalog shows grouped agents; all agents appear in exactly one group.
- Workflow Inspector shows states sorted by execution order with ordinal badges.
- Validation issues display correctly in both segments.

### 7.4 Settings + Readiness

- Settings shows new "System Readiness" collapsible section.
- Readiness section shows hero banner, config paths, capabilities.
- "Refresh Readiness" button refreshes both readiness and provider health.
- No content duplication between readiness section and provider management section.

### 7.5 Live Timeline

- Tool call started + finished render as a single card with tool name and duration.
- In-flight tool card shows spinner; completed card shows checkmark.
- 30 seconds after tool completion, the card fades out.
- When a textChunk arrives, all preceding tool-only cards for that agent are removed on the next 2-second flush.
- When an agent finishes, all its cards collapse into one summary card showing tool count, text count, and duration.
- UI updates occur at most every 2 seconds with a 1.5-second easeInOut animation.
- No visible jitter during heavy tool-use sequences (20+ rapid tool calls).
- Timeline does not exceed 40 entries.

### 7.6 Information deduplication

- Run detail (recovery, comparison, report, timeline) appears only in Runs tab.
- Idea management (create, configure, archive) appears only in Ideas tab.
- Provider health appears only in Settings (not duplicated in a separate readiness tab).
- Approval resolution appears only in Runs inline panel (not in a separate tab).

---

## 8. Risks

| Risk | Severity | Mitigation |
|------|----------|------------|
| Operators habituated to the Approvals tab lose muscle memory | Low | ForegroundBanner + Cmd+Shift+A shortcut provide faster alternatives. Migration period maps old tab values. |
| Ideas compact status strip loses useful run detail | Medium | The strip links directly to the run in Runs tab. One click, not zero, but the canonical detail is always fresh. |
| Definitions segmented view feels cramped for complex catalogs | Low | Each segment retains its current NavigationSplitView layout. The segmented picker adds only ~40pt of vertical overhead. |
| Execution-order sorting fails for workflows with complex branching | Medium | Fallback to source order for unreachable states. Toggle allows operator to switch sorting. |
| Pilot Readiness content in Settings makes Settings too long | Low | Collapsible DisclosureGroup keeps readiness hidden by default; operator expands when needed. |
| UI test coverage for removed tabs breaks | Low | Explicit migration of test environment variables and UISurface routing (SS6.3, SS6.4). |
| 2-second batch delay makes timeline feel laggy | Low | 2s is below the threshold for perceived delay in monitoring contexts. The 1.5s animation creates continuous motion rather than discrete jumps. The operator still sees real-time progress, just smoothed. |
| Tool card 30s expiry removes cards the operator wanted to inspect | Low | 30s is generous for a transient tool card. The agent summary card (on finish) preserves the total tool count. Artifact inspection provides full detail post-run. |
| Rapid agent switching causes interleaved collapse | Low | Collapse is per-agent (keyed by agentID). Concurrent agents each get their own summary card independently. |

---

## 9. Summary of Tab Reduction

```
BEFORE (7 tabs)                    AFTER (4 tabs)
─────────────────                  ────────────────
Runs Home          ──────────────► Runs (+ inline approvals)
Ideas              ──────────────► Ideas (compact run status)
Approvals          ──────────────► (removed; inlined into Runs)
Agent Catalog      ──┐
                     ├───────────► Definitions (segmented)
Workflow Inspector ──┘
Pilot Readiness    ──┐
                     ├───────────► Settings (+ readiness section)
Settings           ──┘
```

---

## Appendix A: Visual Evidence from Xcode Previews

This section documents specific observations from rendering Xcode previews of each current view, confirming the duplication patterns and UX issues described in the proposal.

### A.1 Runs Home (`RunsHomeView.swift` — "Runs Home — Mixed States" preview)

**Layout:** NavigationSplitView with sidebar (run list) and detail pane ("Select a Run" placeholder).

**Observed structure:**
- Sidebar groups runs into four sections: **Waiting Approval** (orange header), **Blocked** (red header), **Running**, **Recently Completed** (green header).
- Each run row shows: title, status badge (colored pill: `waitingApproval`, `blocked`, `completed`), and elapsed time (e.g., "40m 0s", "35m 0s").
- Toolbar shows "Resume Interrupted" and "Clear Old Runs" actions.
- Summary strip at top right: "Waiting approval 1, blocked 1, recent completed 2".
- ForegroundBannerView at bottom: orange bar reading "1 waiting approval" with "View in Runs Home ->" link.

**Key finding:** Runs Home is focused and well-structured. The grouping by status is clear and the sidebar-to-detail pattern works well. This view should remain the canonical run monitoring surface.

### A.2 Ideas (`IdeaListView.swift` — "Ideas — Operator List" preview)

**Layout:** Three-column structure: narrow sidebar with idea cards, wider idea detail pane, scrolling form.

**Observed structure — sidebar:**
- Idea cards show: title (bold), run status badges (e.g., orange `waitingApproval`, red `blocked`, green `Completed`), and run count ("1 run").
- Each idea card visually echoes the same status information shown in Runs Home.
- Archived ideas appear in a collapsible section at the bottom.
- "New Idea" button and toolbar actions at top.

**Observed structure — detail pane:**
- Full idea configuration: Title, Status, Created date, Body text, Project Directory, Workspace root path (with Browse button), Archive section.
- **"Runs" section at the bottom** showing individual runs with "Canonical Workflow" label.
- Approval bar at the bottom: "1 pending approval(s)" in orange.

**Key finding — confirmed duplication:** The Ideas sidebar shows run status badges that duplicate Runs Home's grouping. The detail pane's "Runs" section embeds run rows, and the bottom approval bar duplicates the ForegroundBanner. An operator looking at a `waitingApproval` run sees it **three times**: Runs Home sidebar, Ideas sidebar badge, Ideas detail "Runs" section.

### A.3 Start New Run (`IdeaListView.swift` — "Start New Run — Live" preview)

**Layout:** Modal sheet with form fields.

**Observed structure:**
- Idea name at top.
- Execution Mode: segmented picker (Simulated / Live).
- Workflow: "Canonical Workflow" label.
- Context Strategy: dropdown ("current_mixed_baseline").
- Compilation Preview section (with YAML parse error shown in red).
- Preflight section.
- Cancel / Compile / Start Run buttons at bottom.

**Key finding:** This sheet is well-scoped and does not need changes. It belongs to the Ideas tab.

### A.4 New Idea Sheet (`IdeaListView.swift` — "New Idea Sheet" previews)

**Layout:** Compact modal with two fields: Title and Body.

**Key finding:** Clean and minimal. No changes needed.

### A.5 Pilot Readiness (`PilotReadinessView.swift` — "Pilot Readiness — Seeded" preview)

**Layout:** Long scrolling form, no sidebar.

**Observed structure (top to bottom):**
- Header: "Pilot Readiness" with "Refresh Readiness" and "Open First Run Wizard" buttons.
- "Checking Readiness..." status indicator.
- **Configuration Paths** section: State, Autostart fields.
- **Managed Goose Server** section: "Start Managed Server" button, Enabled status.
- **Providers** section: "Health Refreshed" timestamp.
  - Provider cards (Claude Goose, Codex via Goose) showing:
    - Health status (green "Healthy" / orange "Blocked")
    - Blocking issues in red warning boxes (e.g., "Claude Goose: Goose path needs attention")
    - Detail fields: Configured transport, Active configuration source, Goose server reachability, Co-management, Blocking issues.
- Open Goose Assistant section at bottom.

**Key finding — confirmed heavy duplication with Settings:** Managed Goose Server, provider health cards, and blocking issues are nearly identical between Pilot Readiness and Settings. The Configuration Paths section is the only content unique to Pilot Readiness.

### A.6 Provider Settings (`ProviderSettingsView.swift` — "Provider Settings — Configured" preview)

**Layout:** Long scrolling form, no sidebar.

**Observed structure (top to bottom):**
- Header: "Provider Settings" with "First Run Wizard" button.
- Guidance text explaining purpose.
- **"Open First Run Wizard"** button (prominent).
- **Managed Goose Server** section: State, Base URL, Autostart, Binary, "Start Managed Server · Refresh Server Status" button.
- **Configured Providers** section:
  - Provider rows (Claude Goose, Codex via Goose, Quexia HTTP) with:
    - Capabilities badges.
    - Health status indicators (red warnings for missing API keys).
    - "Remove" buttons.
    - "Open Goose Assistant · Prefer" action links.

**Key finding — confirmed overlap:** Managed Goose Server section is nearly identical between Settings and Pilot Readiness (State, Autostart, start button). Provider health information appears in both. Settings adds provider management actions (Add, Remove, configure API keys). Pilot Readiness adds configuration paths and detailed blocking-issue cards. Merging these eliminates the need to switch tabs to diagnose and fix provider issues.

### A.7 Agent Catalog (`AgentCatalogView.swift` — code review, no preview available)

**Layout:** NavigationSplitView (sidebar + detail).

**Observed code structure:**
- Sidebar: `ForEach(catalog.agents)` — flat list, no grouping (line 47).
- Each agent row: title (`.headline`), ID (`.caption`), backend profile + permission profile (`.caption2` with server.rack and lock.shield icons).
- Summary strip: agent count, backend count, permission count, error/warning counts.
- Detail pane: Form with sections: Identity (ID, Title, Mode), Backend (Profile, Provider, Model, Effort, Max Turns), Permissions, Skill (Ref, Role, Type, Source, Hash, Bundle companions), Content Preview.
- Validation issues section with severity icons.

**Key finding — confirmed flat list:** All 15+ agents appear in a single unsorted list. No grouping by role, phase, or function. Finding the "Security Checker" among orchestrators, reviewers, and release agents requires scanning.

### A.8 Workflow Inspector (`WorkflowInspectorView.swift` — code review, no preview available)

**Layout:** Segmented picker (Full/Compact) at top, then NavigationSplitView below.

**Observed code structure:**
- State list sorted by `workflow.states.keys.sorted()` (line 69) — **alphabetical sort**, not execution order.
- State rows show: emoji by type (play button for start, hand for manual_gate, flag for end) + label + stateID.
- Summary strip: state count, gate count, loop count, errors, warnings.
- Detail pane: state transitions, agent assignments, approval requirements.

**Key finding — confirmed alphabetical sorting:** States like "approved", "draft", "implementation", "review_architect" appear alphabetically rather than in the order the workflow actually executes them. The operator cannot see the execution flow at a glance.

### A.9 Approval Inbox (`ApprovalGateView.swift` — code review, no preview available)

**Layout:** Simple ScrollView with stacked `ApprovalGateView` cards.

**Observed code structure:**
- `ApprovalInboxView`: ScrollView of `ApprovalGateView` cards, sorted by `requestedAt`. Empty state when no pending approvals.
- `ApprovalGateView`: compact card with stage label, run ID (truncated), preceding artifacts list, comment TextField, Reject (Cmd+Delete) and Approve (Cmd+Return) buttons.
- Total view code: ~145 lines.

**Key finding — standalone tab is unnecessary:** The `ApprovalGateView` component (the card itself) is already designed for embedding — it's used inline in `RunProgressView`. The `ApprovalInboxView` wrapper is just a ScrollView + ForEach + empty state. The same functionality is achievable by filtering the Runs Home "Waiting Approval" section and showing the approval card in the detail pane.
