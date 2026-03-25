# Proposal 010: Operator Clarity — Idea Archive, Provider Troubleshooting, and Live Workflow Visualization

| Field | Value |
|---|---|
| Date | 2026-03-25 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/operator-experience.md](../reference/operator-experience.md), [reference/provider-platform.md](../reference/provider-platform.md), [reference/goose-server-transport.md](../reference/goose-server-transport.md) |
| Goal | Make the product easier to operate day to day by adding archive semantics for ideas, fixing Goose-backed provider setup clarity for Codex and Claude Code, and giving the operator an explicit live workflow map with agent activity, handoff counters, and visible loops. |

---

## 1. Context

The current shell already has the right major pieces:

- ideas,
- runs,
- approvals,
- reports,
- provider configuration,
- diagnostics,
- run progress.

But the operator experience still has three practical gaps:

1. ideas accumulate without a proper archive lifecycle,
2. provider setup for `codex` and `claude_code` is too opaque when direct `goosed`-backed setup works outside the app but not inside it,
3. live run progress does not make workflow topology and agent activity legible enough.

Those are not "nice to have" polish issues.
They directly affect whether the product feels trustworthy and navigable during real use.

This proposal does not introduce new execution intelligence.
It makes the existing system explain itself better.

### 1.1 Current UI quality findings on current HEAD

The current UI already proves functionality, but it still reads too much like an internal control panel.

Observed quality problems that Proposal 010 must address:

1. `Start New Run` is too vertically heavy for a primary action:
   - core choices and advanced/debug controls compete for attention,
   - hierarchy is weak,
   - compile/preflight information arrives as stacked detail instead of a guided flow.

2. provider configuration feels like a raw admin form instead of a setup experience:
   - configured-provider rows are text-heavy,
   - status is visible but not well-prioritized,
   - remediation is not first-class,
   - path/config fields dominate the screen before the operator reaches provider intent.

3. the app shell is too flat:
   - many peer tabs compete equally,
   - operational importance is not reflected strongly enough in navigation weight,
   - system setup, pilot readiness, and live run operation feel adjacent rather than intentionally staged.

4. workflow progress is still more log-shaped than topology-shaped:
   - the operator can inspect state,
   - but cannot immediately read structure, direction, concurrency, and loops from the screen itself.

### 1.2 Screen-level critique that Proposal 010 must correct

#### `Start New Run`

- the sheet is too long and reads like a configuration dump,
- the visual difference between primary choices and optional configuration is too small,
- the `Compile` / `Start Run` actions arrive too late in the reading order,
- explanatory text is present, but not shaped into a confident guided decision flow,
- the screen feels closer to a debug launcher than to a production operator action,
- the default visual emphasis still favors `Simulated` over `Live`, even when the operator's real question is "can this run on Goose right now?",
- compile/preflight placeholders create large inert zones before useful evidence exists.

#### `Provider Settings`

- configured providers do not feel like strong status objects,
- setup and maintenance are mixed into one dense surface,
- raw file paths and low-level fields dominate the top half of the screen,
- health state exists, but urgency and next action are still under-signaled,
- the form does not yet feel like a trusted setup assistant,
- provider intent is visually subordinate to storage/config internals,
- the current layout reads as one long admin list instead of "status first, remediation second, advanced configuration last".

#### App shell

- too many top-level destinations sit at the same visual rank,
- the shell exposes system internals early instead of staging the operator journey,
- high-frequency destinations and low-frequency setup destinations are not separated strongly enough,
- sidebars allow critical labels to truncate too aggressively,
- empty detail canvases appear before the operator has enough context to know what action to take next.

Proposal 010 should treat these as product-quality issues, not cosmetic polish.

### 1.3 Preview-review requirement

Before Proposal 010 is considered implementation-ready, the current owner-path surfaces must be reviewed in live SwiftUI preview, macOS `My Mac` XCUITest attachments, or app-rendered screenshots, not only from source and screenshots.

Required preview-review targets:

- app shell navigation,
- `Start New Run`,
- provider setup / troubleshooting.

Once the new Proposal 010 screens exist, the same requirement extends to:

- archive surfaces,
- workflow map and agent activity panels.

This proposal should not be signed off on layout confidence alone from source inspection.

### 1.4 Xcode preview findings captured on 2026-03-25

Fresh Xcode MCP preview coverage is materially better than before this proposal pass.
Current preview pack on HEAD now covers:

- `ContentView` (`2` previews),
- `RunsHomeView`,
- `IdeaListView` (`2` previews),
- `ProviderSettingsView`,
- `PilotReadinessView`,
- `FirstRunSetupWizard`,
- `RunStartOverridesView`.

That is enough to do a real visual pass on the current operator shell.
It is still not enough for full sign-off because preview coverage is still missing for:

- archive-specific screens,
- approval inbox / approval gate surfaces,
- workflow-map / live topology surfaces.

Findings from the live previews:

#### `ContentView` preview

- the shell is more legible than before, but the top navigation still gives every destination nearly identical weight,
- `Runs Home` lands in a large empty detail state without giving the operator an immediate next action,
- the shell reads as a collection of tools rather than one intentionally staged operator workspace.

#### `RunsHomeView` preview

- the left rail is too narrow and aggressively truncates the most important run labels,
- the row composition is over-dense: badges, state glyphs, and micro-actions compete inside too little horizontal space,
- the default detail state is visually dead; the product should not ask the operator to decode an empty canvas after already showing urgent run groups.

#### `IdeaListView` preview

- the idea list inherits the same truncation problem as `RunsHomeView`,
- the summary strip is too compressed to serve as a calm orientation surface,
- archive intent is still absent from the visual hierarchy, which matters because archive is one of Proposal 010's core slices.

#### `ProviderSettingsView` preview

- raw configuration paths dominate the first screenful and push provider intent below the fold,
- configured providers read as list rows instead of durable status cards with a clear primary next action,
- `Add Provider` is visually underpowered compared with the configuration dump above it,
- the screen communicates "settings database" better than it communicates "fix Codex/Claude setup".

#### `PilotReadinessView` preview

- the screen reads as a labeled-value dump, not as a readiness dashboard,
- provider health, diagnostics, and operator status do not have a clear narrative hierarchy,
- primary actions are present, but the surface does not yet express what should be checked first.

#### `FirstRunSetupWizard` preview

- the wizard has a large dead zone above the first section, which makes the screen feel visually broken before any interaction starts,
- field labels, body copy, and action buttons collapse into one narrow central column with weak grouping,
- the top-right `Save` action is visually detached from the actual setup journey,
- the screen does not yet feel like a guided first-run sequence; it feels like a form dropped into a modal shell.

#### `Start New Run — Live` preview

- hiding overrides from the primary path was correct and materially improved the sheet,
- the sheet still has too much inert vertical space between the workflow block and the bottom action bar,
- compile/preflight sections look like empty placeholders instead of evidence-bearing checkpoints,
- the current layout still does not clearly privilege the operator's main question: "is this safe and ready to launch live?"

#### `RunStartOverridesView` preview

- the screen is overwhelmingly repetitive and form-heavy,
- every override row has nearly identical weight, so scanning cost is very high,
- labels, fields, and separators create a long "settings spreadsheet" instead of a guided decision tool,
- there is no meaningful compression of repeated structure,
- this surface strongly validates the decision to hide overrides from the default run-start path and keep it as an advanced/debug-only surface.

### 1.5 Minimum evidence gate before implementation-ready

Proposal 010 is implementation-ready only after the current owner-path baseline has been reviewed and one minimum draft-readiness gate is closed.

Required draft-readiness evidence:

1. fresh macOS `My Mac` XCUITest attachments, preview renders, or app-rendered screenshots proving the current shell can reach the canonical provider/settings owner path,
2. fresh review of the current `Ideas` -> `Start New Run` -> run-detail owner path, including any instability discovered on HEAD,
3. preview or app-rendered review of the current shell owner path for provider remediation and run detail,
4. design sign-off that the existing owner-path surfaces can absorb the new archive and workflow-map slices without requiring new top-level destinations.

If this gate is not met, the proposal may still be discussed, but it should remain a draft rather than a safe implementation handoff.

### 1.6 Post-implementation sign-off gate

Proposal 010 may be implemented after section `1.5` is closed, but it may not be marked implemented until the new-surface evidence is closed as well.

Required post-implementation evidence:

1. fresh macOS `My Mac` XCUITest attachments, preview renders, or app-rendered screenshots of the archive flow,
2. fresh macOS `My Mac` XCUITest attachments, preview renders, or app-rendered screenshots of the workflow-map primary state from a test environment where workflow compilation succeeds,
3. at least one non-happy-path workflow-map screenshot showing empty/unavailable/fallback presentation,
4. preview or app-rendered review of the final archive, provider-remediation, and workflow-map owner paths.

These preview findings should be treated as live visual evidence, not just code-derived opinion.

---

## 2. Product questions this proposal must answer

After Proposal 010, the engineer should be able to:

1. archive an idea cleanly once it is irrelevant or terminal,
2. understand why `codex` or `claude_code` does not connect in the app even if direct `goosed` usage appears healthy,
3. configure those providers through the same Goose-backed path the runtime actually uses,
4. watch a live run as a workflow map instead of a flat status feed,
5. see which agents are currently active, which have completed, which have not started, and where loops are happening,
6. reason about communication volume between stages/agents without pretending the system is a hidden free-form multi-agent chat.

### Definition of done

Proposal 010 is done only when all of the following are true:

1. ideas can be archived only under explicit valid lifecycle rules,
2. provider setup surfaces explain the exact failing step for `codex` and `claude_code`,
3. provider setup for those families uses the Goose-backed runtime path instead of a disconnected direct-CLI configuration path,
4. the app renders a live workflow graph with current position, stage state, active agents, and visible loop direction,
5. the run UI exposes deterministic handoff/communication counters and agent execution states,
6. the operator can distinguish "not started", "thinking", "waiting", "completed", "failed", and "skipped" without opening raw logs.

---

## 3. What we build

Three tightly scoped slices.

### 3.1 Shell ownership is explicit

Proposal 010 does not add new top-level shell destinations.
Each slice extends one existing shell owner:

| Slice | Shell owner | Canonical operator entry | Explicit non-goal |
|---|---|---|---|
| Idea archive | `Ideas` flow | `Ideas` list -> archived filter/list -> idea detail | no separate archive tab |
| Provider troubleshooting | existing provider setup/readiness journey | `ProviderSettingsView` or `FirstRunSetupWizard` -> Goose assistant/evidence -> `PilotReadinessView` refresh | no standalone provider-troubleshooting destination |
| Live workflow map | run detail / `WorkflowRunProgressView` | `RunsHomeView` or idea run detail -> run progress -> workflow map pane | no peer shell tab for workflow map |

This is a navigation constraint, not a design preference.
If a Proposal 010 surface cannot be reached from its declared owner path, the slice is out of contract.

### Layer M: Idea Lifecycle Hygiene

| Component | Responsibility |
|---|---|
| `IdeaArchivePolicy` | Enforce when an idea may be archived or restored |
| `IdeaArchiveService` | Apply archive/unarchive actions and protect active work |
| `IdeasArchiveView` | Archived ideas list with restore and search/filter support |
| `IdeaLifecycleBadge` | Surface draft / active / terminal / archived status clearly in idea rows |

### Layer N: Provider Setup Troubleshooting

| Component | Responsibility |
|---|---|
| `GooseProviderConnectionAssistant` | Guided Goose-backed setup and verification flow for Codex / Claude Code |
| `GooseProviderHandshakeProbe` | Test each Goose-backed setup step the app actually depends on |
| `ProviderFailureExplainer` | Convert transport/setup failures into actionable remediation |
| `ProviderSetupEvidencePanel` | Show raw Goose handshake facts, endpoint, auth mode, provider resolution, model resolution, and latest failure |

### Layer O: Live Workflow Map

| Component | Responsibility |
|---|---|
| `WorkflowMapView` | Visual topology of the active workflow during a run |
| `WorkflowMapLayoutEngine` | Deterministic node/edge layout for stages and agent tasks |
| `AgentActivityPanel` | Current/queued/completed/not-started agent list with per-agent state |
| `HandoffTelemetryService` | Count deterministic handoff events between stages/agents |
| `LoopTraceOverlay` | Explicitly render loop direction and current iteration |

---

## 4. Idea archive

### 4.1 Why

The idea list should represent live operator attention, not become a graveyard of abandoned drafts and already-finished work.

### 4.2 Archive eligibility

An idea may be archived only if one of the following is true:

1. the idea is still `draft` and has no active run,
2. the latest run for the idea is in a terminal state,
3. the idea has no run history at all and the operator wants to remove it from the active list.

An idea may **not** be archived when:

- it has an active run,
- it is blocked on approval for a still-active run,
- it is currently selected as the live focus of a running session.

### 4.3 Archive semantics

Archiving an idea:

- removes it from the default active ideas list,
- preserves all runs, artifacts, reports, and receipts,
- does not delete data,
- does not mutate historical run state,
- remains reversible via `Restore`.

### 4.4 Required surfaces

- active ideas list shows archive action only when policy allows,
- archived ideas are visible in a dedicated archive view,
- archive view supports search and restore,
- archived ideas are visually distinct from active and terminal-but-not-archived ideas.
- archive affordances do not visually compete with the primary "start or inspect active work" path.

### 4.5 Cross-surface archive rules

Archive is owned by the `Ideas` flow, but archived ideas must remain truthful in run-centric surfaces.

Rules:

- `RunsHomeView`, run detail, reports, and artifact inspection continue to show completed historical work for archived ideas,
- those surfaces show that the parent idea is archived,
- restore is initiated from the archive lane in `Ideas`, not from every run-centric surface,
- archived ideas do not reappear in the default active ideas list unless explicitly restored.

---

## 5. Provider setup troubleshooting

### 5.1 Problem statement

The current provider/platform baseline says the app can configure `codex`, `claude_code`, and `gemini`.
But that is not enough if the operator cannot understand why a provider fails in-app while a direct `goosed` path seems to work.

For Proposal 010, `codex` and `claude_code` must become explainable, not merely configurable.

### 5.1.1 Locked direction for Proposal 010

For `codex` and `claude_code`, Proposal 010 standardizes on the Goose-backed path as the primary integration path.

That means:

- the setup flow should configure what the live runtime actually uses,
- the app should stop treating direct CLI discovery as the canonical operator path for those providers,
- provider troubleshooting should explain Goose session/provider selection failures first,
- a local direct CLI path may still exist for diagnostics or fallback, but it is not the primary operator story.

### 5.1.2 Authoritative migration from the current provider platform

Proposal 010 extends the current provider-platform baseline; it does not replace it with a second Goose-only configuration stack.

One provider source of truth remains in place:

- `ConfiguredProvider` stays the durable configuration object,
- `ProviderSettingsStore` remains the only persisted owner of provider edits,
- `ProviderRegistry` remains the in-memory owner of derived provider health,
- `PreflightService`, `PilotReadinessView`, and run-start readiness continue to consume the same derived facts.

For `codex` and `claude_code`, Proposal 010 adds Goose-primary specialization on top of that baseline:

- Goose endpoint, auth mode, provider identifier, model resolution, and handshake step results are derived diagnostics attached to the existing provider-health flow,
- `ProviderHealthSnapshot` keeps the top-level health truth (`unknown`, `healthy`, `degraded`, `unavailable`),
- Goose-specific failure attribution is progressive disclosure, not a parallel top-level provider state store,
- `gemini` stays on the same generic provider platform and does not get a fake Goose-specific journey.

Illustrative shape:

```swift
struct GooseDiagnosticDetails: Codable {
    let endpoint: String?
    let providerIdentifier: String?
    let resolvedModel: String?
    let failingLayer: String?      // endpoint/auth/provider/model/policy/etc.
    let latestProbeAt: Date?
    let rawEvidenceJSON: Data?
}
```

This detail is derived from probes and registry refreshes.
It is not a second durable provider configuration object.

### 5.1.3 Provider ownership matrix

Proposal 010 assigns one owner per responsibility in the current shell:

| Responsibility | Owner | Allowed to mutate config? | Notes |
|---|---|---|---|
| provider creation/editing | `ProviderSettingsView` | yes | canonical persistent config editor |
| first machine bootstrap | `FirstRunSetupWizard` | yes | guided entry into the same config model |
| Goose-specific verification/remediation | `GooseProviderConnectionAssistant` launched from settings/wizard | no direct parallel persistence | uses the same provider record, then hands back |
| health summary and re-check | `PilotReadinessView` | no | summary + refresh + return path |
| raw failure evidence | `ProviderSetupEvidencePanel` | no | advanced disclosure only |

No provider mutation or verification logic may exist in a parallel flow with separate truth.

### 5.2 Core principle

The app must diagnose the exact path it depends on, not a nearby happy-path that happens to work in a terminal.

That means the troubleshooting flow must make visible:

- configured Goose endpoint,
- transport style,
- expected auth mode,
- handshake target,
- Goose provider identifier,
- TLS/certificate expectations if relevant,
- model resolution path,
- latest probe result,
- the exact step that failed.

### 5.2.1 Canonical troubleshooting journey

For `codex` and `claude_code`, the operator journey is:

1. unhealthy provider row or first-run bootstrap step,
2. guided Goose-backed verification/remediation,
3. advanced evidence disclosure only if needed,
4. return to `PilotReadinessView` or provider summary with refreshed health.

The operator should never need to rediscover context by hopping across unrelated tabs to finish one remediation task.

### 5.3 Troubleshooting state machine

Each configured provider instance should expose one of:

1. `not_configured`
2. `configured_unverified`
3. `probing`
4. `verified`
5. `degraded`
6. `failing`

This is a Goose-specific operator journey layered under the existing top-level provider health model.
It does not replace `ProviderHealthSnapshot`.

Suggested mapping:

- `not_configured` / `configured_unverified` -> top-level health remains `unknown`,
- `probing` -> top-level health remains transient `unknown` or `degraded`,
- `verified` -> top-level health becomes `healthy`,
- `degraded` -> top-level health is `degraded`,
- `failing` -> top-level health is `degraded` or `unavailable` depending on whether any valid execution path remains.

Failure state must be attributed to a specific layer:

- `binary_or_runtime_missing`
- `endpoint_unreachable`
- `goose_provider_not_available`
- `auth_failed`
- `model_resolution_failed`
- `capability_mismatch`
- `policy_mismatch`
- `unknown`

### 5.4 Guided flow for Codex / Claude Code

The setup assistant must walk through:

1. provider selection
2. Goose transport selection
3. Goose endpoint/runtime discovery
4. auth expectations
5. Goose provider identifier resolution
6. model resolution
7. live handshake probe
8. save only after clear verification or explicit operator acknowledgement

### 5.5 Evidence panel

The operator should not need Xcode console logs to understand setup failure.

Each provider entry must expose a detail panel containing:

- family
- transport
- endpoint
- Goose provider identifier
- selected/default model
- latest checked time
- latest verification result
- actionable remediation text
- optional raw probe details for advanced debugging

### 5.6 Explicit non-goal

This proposal does not add new provider families.
It makes existing MVP families legible and operable.

### 5.7 Multi-provider Goose sessions

Proposal 010 explicitly assumes that different agent executions may run through separate Goose sessions with different provider/model bindings in the same run.

The operator-facing troubleshooting and visualization must stay honest to that model:

- each agent execution may resolve to a different provider family,
- multiple active agent sessions may exist at once,
- provider badges and agent activity should reflect the resolved per-agent binding,
- the app must not imply that one global provider choice applies to every agent in the run.

---

## 6. Live workflow visualization

### 6.1 Why

The current run surface is informative, but it still behaves too much like a structured log.
The operator needs a topology view.

### 6.2 What the map must show

The workflow map renders:

- stage nodes,
- task/agent execution nodes where useful,
- directional edges,
- current active path,
- blocked nodes,
- completed nodes,
- not-started nodes,
- skipped nodes,
- loop-back edges,
- current iteration count when inside a loop.

When multiple Goose-backed agent sessions are active at once, the map and side panels should also show:

- per-agent provider family,
- per-agent model when available,
- concurrent active agent count,
- per-edge or per-node handoff counts without collapsing everything into one monolithic "run is busy" state.

### 6.3 Agent execution states

Per agent/task the operator must be able to distinguish:

1. `not_started`
2. `ready`
3. `thinking`
4. `waiting_input`
5. `completed`
6. `failed`
7. `skipped`

`thinking` is an operator-facing presentation state for currently executing agent work.
It does not change the underlying runtime state model.

`waiting_input` is intentionally narrow.
Proposal 010 may render `waiting_input` only when there is an explicit runtime signal that the agent or stage is waiting for external/operator input.
If current runtime records cannot justify that state, the UI must fall back to `thinking`, `ready`, or `waiting_approval`-adjacent presentation instead of inventing a new state.

### 6.4 Communication counters

Chainworks should not pretend that agents are secretly chatting peer-to-peer if the real architecture is artifact/handoff driven.

For this reason, Proposal 010 defines communication counters as deterministic handoff telemetry:

- artifact produced for another stage/task,
- transition-triggering structured output,
- orchestrator-mediated review packet fan-out,
- loop re-entry caused by review/findings,
- approval request / approval decision handoff.

The UI may label this as `handoffs` or `communications`, but the underlying meaning must stay deterministic and auditable.

### 6.5 Edge direction and loops

The map must make loops obvious:

- forward edges and loop-back edges are visually distinct,
- the active edge/path is highlighted,
- repeated transitions increment visible counters,
- the current loop iteration is rendered near the loop segment.

### 6.6 Required run surfaces

During an active run the operator can access:

- map view,
- active agents panel,
- completed agents panel,
- pending/not-started agents panel,
- handoff counters,
- current stage details,
- raw logs when needed.

The map must complement the existing progress surface, not replace it blindly.

### 6.7 Presentation quality requirements

The workflow map and related panels must improve information hierarchy, not merely add more telemetry.

Required presentation rules:

- current active path must dominate visually over completed history,
- concurrent active agents must be legible at a glance,
- loop-back paths must be distinguishable without reading raw text labels,
- side panels must separate `active`, `completed`, and `not started` clearly,
- debug-level details belong behind progressive disclosure, not in the primary scan path.

---

## 7. Data and runtime model additions

Illustrative additions:

```swift
// Idea
var isArchived: Bool
var archivedAt: Date?

// AgentExecution or derived view model
// Derived first; persisted only if a field cannot be recomputed from run history.
var presentationState: String?   // "not_started" | "ready" | "thinking" | "waiting_input" | "completed" | "failed" | "skipped"

// Run / telemetry store
var workflowTelemetryJSON: Data? // only non-recomputable map facts, if any remain after derivation
```

### 7.1 Derivation contract

Proposal 010 treats the workflow map as a derived view first.

Derived from existing runtime records whenever possible:

- stage/node status from run + stage state,
- agent state from `AgentExecution` status and live timeline,
- loop counters from `Run.loopCounters`,
- provider/model badges from frozen per-agent execution metadata,
- handoff counts from artifacts, approvals, and deterministic transitions already recorded in run history.

New persistence is allowed only for facts that cannot be recomputed later from the frozen run snapshot and execution history.

Telemetry should be append-only or recomputable from run history.
The map must not invent state that cannot be traced back to real execution records.

---

## 8. File structure

```text
Chainworks Forge/
  Engine/
    IdeaArchiveService.swift
    ProviderConnectionAssistant.swift
    ProviderHandshakeProbe.swift
    ProviderFailureExplainer.swift
    HandoffTelemetryService.swift
    WorkflowMapLayoutEngine.swift

  Models/
    Idea.swift                       // archive flags
    Run.swift                        // workflow telemetry snapshot/reference
    AgentExecution.swift             // presentation-state support if persisted

  Views/
    IdeasArchiveView.swift
    ProviderConnectionAssistantView.swift
    ProviderSetupEvidencePanel.swift
    WorkflowMapView.swift
    AgentActivityPanel.swift
    LoopTraceOverlay.swift

  Views/Run/
    RunProgressView.swift            // integrate map and agent activity panels
```

---

## 9. Acceptance criteria

### Idea archive

- [ ] An idea can be archived only if it is draft with no active run or its latest run is terminal
- [ ] Active or approval-blocked ideas cannot be archived
- [ ] Archive is reversible and does not delete run history or artifacts
- [ ] Active and archived idea lists are clearly separated in the UI

### Provider troubleshooting

- [ ] Codex and Claude Code setup exposes step-by-step Goose-backed verification instead of one generic failure state
- [ ] Proposal 010 extends the current `ConfiguredProvider` / `ProviderRegistry` / `PreflightService` baseline instead of creating a second provider source of truth
- [ ] The operator can see whether failure is endpoint, auth, model, capability, or policy related
- [ ] The operator can see whether failure is specifically a Goose provider selection / Goose endpoint problem
- [ ] The provider detail surface shows latest probe evidence and remediation guidance
- [ ] The operator can distinguish "configured but unverified" from "verified" and "failing"
- [ ] Codex and Claude Code no longer depend on direct CLI discovery as the primary operator configuration path
- [ ] Provider setup uses a guided task flow instead of exposing a raw configuration form as the primary experience
- [ ] Configuration paths and low-level details are available, but do not dominate the first screen
- [ ] `ProviderSettingsView`, `FirstRunSetupWizard`, `GooseProviderConnectionAssistant`, `ProviderSetupEvidencePanel`, and `PilotReadinessView` have non-overlapping ownership and one canonical handoff path
- [ ] `gemini` behavior under the same provider platform is explicitly documented and remains coherent with the mixed-provider shell

### Live workflow map

- [ ] An active run exposes a visual workflow map with stage directionality
- [ ] The map clearly shows active, completed, failed, skipped, and not-started states
- [ ] The operator can see which agents are currently thinking
- [ ] The operator can see which agents completed and which have not started
- [ ] The operator can see per-agent provider bindings when agents use different Goose-backed providers
- [ ] Multiple simultaneously active agent sessions remain legible in the UI
- [ ] Loop-back edges and current iteration are visually explicit
- [ ] Deterministic handoff/communication counters are visible and update during the run
- [ ] The primary scan path favors structure and status, while logs and low-level detail stay secondary
- [ ] `waiting_input` appears only when supported by an explicit runtime signal
- [ ] Workflow-map state, counters, and provider badges are derived from existing runtime records first; new telemetry is persisted only by exception

### UI quality

- [ ] `Start New Run` uses progressive disclosure so advanced/debug controls do not crowd the primary launch path
- [ ] Primary screens show one dominant action and one clear reading order
- [ ] Shell navigation reflects operational priority rather than flattening all destinations equally
- [ ] Settings, readiness, and run-operation surfaces feel like staged tasks, not one undifferentiated admin surface
- [ ] `ContentView`, `RunsHomeView`, `IdeaListView`, run-start flow, provider troubleshooting, archive surfaces, and workflow-map surfaces all have maintained SwiftUI preview coverage for design review
- [ ] Proposal 010 introduces no new top-level tab
- [ ] Archive, provider troubleshooting, and workflow map each have one declared existing shell owner and one canonical navigation path

### General

- [ ] Existing operator shell, provider-platform, and live runtime tests do not regress
- [ ] The new workflow map does not require opening raw logs to understand where the run is stuck
- [ ] Provider setup failures can be diagnosed from in-app surfaces without Xcode console access
- [ ] Draft-readiness gate is closed before the proposal is marked implementation-ready:
  - provider/settings shell reachability proof on macOS
  - current `Ideas` -> `Start New Run` -> run-detail owner-path review
  - preview/app-rendered review of provider remediation and run detail
- [ ] Post-implementation sign-off gate is closed before Proposal 010 is marked implemented:
  - archive proof on macOS
  - workflow-map primary-state proof from a compilable test environment
  - one workflow-map fallback-state proof

---

## 10. Out of scope

| Exclusion | Reason |
|---|---|
| New provider families | Proposal 010 is about clarity, not provider expansion |
| Autonomous agent-to-agent chat visualization | The system remains artifact/handoff driven, not free-form hidden chat |
| Repo-backed release visualization redesign | Proposal 007/008 own full release and sign-off completion surfaces |
| Hard delete of ideas and data-retention policy | Archive only, not destructive lifecycle management |
| Cloud/shared inbox or multi-user assignment | Still a single-engineer local-first product |

---

## 11. Locked decisions

| ID | Decision | Rationale |
|---|---|---|
| OPS-101 | Idea archive is reversible and non-destructive | Historical runs and receipts must remain intact |
| OPS-102 | Active or approval-blocked ideas cannot be archived | Prevent hiding work that still needs attention |
| OPS-103 | Provider troubleshooting must expose the exact failing layer | Avoid generic "could not connect" dead ends |
| OPS-104 | Codex and Claude Code use Goose-backed configuration as the primary operator path | Match setup UX to the runtime architecture that actually executes live runs |
| OPS-105 | Workflow communications are modeled as deterministic handoffs, not fictional peer chat | Keep the UI honest to the runtime architecture |
| OPS-106 | Loop visualization is a first-class requirement, not an optional ornament | Loops are one of the most confusing parts of live operation |
| OPS-107 | The UI must stay legible when multiple Goose-backed agent sessions are active with different provider bindings | Mixed-provider concurrency is a core part of the intended runtime model |
| OPS-108 | Primary operator screens use progressive disclosure for advanced/debug controls | Prevent internal tooling detail from overwhelming the main workflow |
| OPS-109 | Information hierarchy must privilege actionability over raw configuration density | The operator should know what to do next before seeing every implementation detail |
| OPS-110 | Proposal 010 extends the current provider platform and shell owners instead of creating parallel setup or navigation stacks | Reduces migration risk and keeps the operator journey coherent |
| OPS-111 | Workflow-map presentation is derived from runtime truth first and persists only minimum irreducible telemetry | Prevents UI-only state drift |

---

## 12. What this proposal enables

Proposal 010 makes the app calmer and more legible in the moments that matter most:

- when the idea list is getting noisy,
- when provider setup does not work,
- and when a live run enters a complex loop.

That does not change the engine.
It changes whether the operator can actually trust what the engine is doing.
