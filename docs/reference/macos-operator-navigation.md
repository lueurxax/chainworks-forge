# macOS Operator Navigation and Read-Model UX

Stable reference for the implemented macOS operator shell, consolidated navigation, run workbench, and release proof lane.

## Scope

The macOS app has four top-level operator surfaces:

1. `Runs`
2. `Ideas`
3. `Definitions`
4. `Settings`

The app remains a governed thin client. It reads workflow truth from GraphQL projections, resolves human approval gates through the approved GraphQL approval mutations, and keeps non-approval command/control paths outside SwiftUI. MCP, broad GraphQL mutations, local workflow mutation, SwiftData truth fallback, and raw artifact-directory truth are not UI authority.

This reference owns the durable navigation and read-model UX behavior. Adjacent owners remain:

- [operator-experience.md](operator-experience.md) for the broader operator shell baseline.
- [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md) for GraphQL projection and subscription shape.
- [thin-client-read-model-affordance-contract.md](thin-client-read-model-affordance-contract.md) for approval/actionability and payload/freshness affordances.
- [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md) for run detail panes, timeline placement, and artifact hierarchy.
- [test-gates.md](test-gates.md) for the retained historical alias names `proposal-036` / `p036` and `proposal-093` / `p093`.

## Navigation Shell

`ContentView.Tab` exposes only `Runs`, `Ideas`, `Definitions`, and `Settings`.

Legacy routes remain compatibility inputs, not top-level destinations:

| Legacy route | Current target |
|---|---|
| `runsHome`, `Runs Home` | `Runs` |
| `approvals`, `Approvals` | `Runs`, focused on the waiting-approval lane |
| `agentCatalog`, `Agent Catalog` | `Definitions`, Agents segment |
| `workflowInspector`, `Workflow Inspector` | `Definitions`, Workflows segment |
| `pilotReadiness`, `Pilot Readiness` | `Settings` |
| `providerSettings` | `Settings` |

The same mapping applies to `CHAINWORKS_UI_TEST_INITIAL_TAB`, `chainworksSelectTab` notifications, and `chainworks://` deep links. The `chainworks://approvals` path posts the waiting-approval focus request before Runs renders so the selected lane survives the tab-switch render cycle.

## Surface Responsibilities

### Runs

Runs is the canonical operator workbench for run lifecycle inspection. It owns:

- attention lanes: waiting approval, blocked or failed, running, recently completed, and status unknown;
- selected run summary and freshness/readback state;
- stage topology and stage occurrence cards;
- inline approval rows with fail-closed P085 actionability;
- artifacts and reports with authorized payload state;
- recovery/evidence rows and daemon/system readiness context;
- active-agent Timeline readback.

`RunsWorkbenchPresentationModel` is the typed presenter boundary between GraphQL/P085 read models and SwiftUI rows. SwiftUI views consume presentation rows rather than interpreting raw GraphQL, P085, filesystem, or local workflow state directly.

### Ideas

Ideas is read-first. It shows idea metadata and compact run status strips sourced from daemon projections. It does not expose executable SwiftUI create, configure, archive, launch, start-run, or local workflow-write controls.

Run status strips use the typed run-lane vocabulary. Unknown server statuses render as an explicit `Status Unknown` deferred lane rather than being bucketed heuristically.

Absolute workspace paths outside `$HOME` are redacted to `<redacted>` when shown as reference metadata.

### Definitions

Definitions combines Agent Catalog and Workflow Inspector in one segmented surface. Agent grouping is deterministic: explicit supported group field, then mode, then profile, then role, then `Other`. Workflow ordering starts at the initial state, follows declared transition order, avoids cycles, preserves stable source-order tie breaks, places branch-only states after the primary traversal, and places unreachable states after a separator.

### Settings

Settings owns System Readiness, provider configuration, provider health, scheduler health, daemon lifecycle state, diagnostics, and configuration paths. Blocked or failed Runs can route operators to System Readiness without performing remediation inside SwiftUI. The Settings path may expose a return affordance back to the originating run.

Raw PID and similar technical diagnostics remain hidden behind an explicit diagnostics-detail toggle.

## Approval and Payload Affordances

Approval rows use P085 freshness and disabled reason state before enabling any action. Stale, projection-lag, unauthorized, redacted, unavailable, unsupported, conflict, duplicate, already-resolved, and ambiguous states render explicit deferred rows and disable approval buttons.

Redacted approval rows suppress body text, copy items, follow-up identifiers, and upstream accessibility labels that may contain sensitive detail. VoiceOver receives a generic restricted label instead of the raw server text.

Artifact and report rendering follows server payload availability and authorized detail reads. Metadata-only, deferred, unauthorized, redacted, stale, and schema-mismatch states fail closed and do not fall back to filename extension or filesystem probing.

## Timeline

The Runs workbench Timeline is populated from control-plane active-agent readback. It does not synthesize timeline rows from stage transitions, approval rows, artifacts, reports, or completed-agent noise.

Timeline is scoped to the selected active agent. When one active agent is
running, the app selects it automatically. When multiple active agents are
running, the selector uses control-plane ordering and keeps the event stream
focused on one agent instead of interleaving every active execution.

Runtime events pass through `P036RuntimeTimelineBuffer`:

- response text chunks append into one live response row;
- terminal response/session events collapse accumulated chunks into a summary;
- provider/tool action starts collapse with matching completion events into one
  terminal action card instead of leaving adjacent `in_progress` and
  `completed` rows;
- events are applied on the main actor through a bounded publish path, at most once every two seconds unless a terminal event forces publication;
- the visible cap is 40 rows after preserving response/session attention;
- out-of-order tool finishes reconcile with matching starts when possible and otherwise render as diagnostic session events.

Timeline cards are newest-first. Each card is collapsed by default and expands
on click, keyboard action, or accessibility action. Only one card is expanded at
a time; expanding another card collapses the previous one. Expanded cards render
a bounded formatted preview for readable markdown/code/JSON-like content and
keep the full or retained raw detail available through explicit copy actions.
State, time, provider, agent, and event identity remain available as metadata
without turning the collapsed card into a dense log row.

Raw detail retention is byte-budgeted. Inline retained raw detail is capped at
512 KiB by UTF-8 byte count, with truncation/readback fields indicating whether
the full raw content is available through the daemon-owned raw-detail resolver.
Swift treats raw-detail handles as opaque control-plane tokens and does not
probe local filesystem paths or artifact directories to recover raw content.

When macOS Reduce Motion is enabled, timeline transitions use opacity or no animation rather than spatial movement.

## Metrics and Readiness Signals

The retained metric names use the historical `p036` prefix:

- `p036_tab_route_resolution_total{source_tab,target_tab,result}`
- `p036_global_attention_indicator_total{attention_kind,count_bucket,freshness_state}`
- `p036_inline_approval_render_total{freshness_state,actionability_state}`
- `p036_operator_task_attempt_total{task_id,result,blocked_reason}`
- `p036_timeline_batch_flush_total{entry_count,flush_latency_bucket,reduce_motion}`
- `p036_timeline_card_collapse_total{agent_status,collapse_reason}`
- `p036_artifact_payload_state_total{payload_availability_state,render_kind}`
- `p036_projection_gap_deferred_total{surface,gap_class}`

UI counters mirror to `UserDefaults` so `MetricsCollector` can read UI event totals and operator task-attempt label buckets even when the thin GraphQL UI cannot mutate a SwiftData `Run`.

Release and focused Timeline evidence for this shell is retained under:

- `docs/evidence/macos-operator-navigation/dogfood-validation-2026-05-21.json`
- `docs/evidence/macos-operator-navigation/remote-ui-accessibility-proof-2026-05-21.json`
- `docs/evidence/macos-operator-navigation/rollout-readback-live-2026-05-21.json`
- `docs/evidence/macos-operator-navigation/p093-remote-ui-proof-2026-05-22.json` for retained historical alias evidence

The rollout operator-readback fixture remains at `docs/evidence/rollout-contract/operator-readback/p036-full-surface.fixture.json` because rollout fixtures and gate aliases retain historical proof names.

## Verification

The canonical proof aliases remain:

```bash
./scripts/test-gate.sh proposal-036
./scripts/test-gate.sh p036
./scripts/test-gate.sh proposal-093 # retained historical alias
./scripts/test-gate.sh p093 # retained historical alias
```

On local hosts, the retained gates run the build plus focused non-UI P036/P031/timeline slices. On the approved remote UI host, the same proof lanes also run the UI smoke/accessibility flow for navigation, Runs inspection, approval context, Ideas-to-Runs handoff, Definitions segments, Settings readiness, and heavy Timeline behavior.

Retained historical alias names such as `proposal-036`, `p036`, `proposal-093`, `p093`, `P036DeferredState`, and `P036RuntimeTimelineBuffer` are implementation and proof identifiers. The stable behavioral source of truth is this reference document, not retired proposal files.
