# P036 Comparison Passes

## Pass 1: Current vs. Pre-Control-Plane Baseline

- **Lost affordances**: Local workflow creation/mutation (intentionally removed), direct filesystem artifact manipulation.
  - *Rationale*: Shift to control-plane ownership and durable implementation worktrees.
  - *Citations*: The legacy `New Idea`, `Archive`, and `Start New Run` controls are no longer reachable from the consolidated navigation surface; `ContentView.swift` retains only read-first Ideas affordances via `P031IdeasCompatibilitySurface`.
- **Degraded affordances**: Timeline noise during high-concurrency tool runs (to be fixed in P036).
  - *Fix*: `RunTimelineInspectorView.swift` `buildFocusedTimelineSpineEntries` performs tool reconciliation and text-chunk collapse.
- **Unchanged affordances**: Run monitoring, artifact viewing (basic), settings.
- **Intentionally removed**: Local SwiftData orchestrator fallback, raw artifact truth for workflow state.

## Pass 2: Baseline vs. P036 Target

- **Carry forward**: GraphQL read model, thin-client discipline, P085 affordance logic.
- **Replace with P036 design**:
    - Top-level navigation (7 tabs -> 4 tabs).
    - Runs workbench (unified monitoring/inspection).
    - Ideas (compact status strips).
    - Definitions (segmented catalog/workflow).
    - Readiness (absorbed into Settings).
    - Timeline (batched/collapsed).
- **Drop as legacy write path**: Any remaining local create/launch buttons in Ideas/Runs that don't go through approved write transports.
- **Defer until projection exists**: Artifact/report payload rendering where GraphQL projection is missing.

## Pass 3: Current vs. P036 Target (Implementation-Authoritative Backlog)

1. **Navigation Shell**: 4 tabs (Runs, Ideas, Definitions, Settings).
2. **Runs Workbench**: Attention lanes, stage map, inline approvals, artifacts/reports in a DisclosureGroup structure.
3. **Ideas Compaction**: Status strips deep-linking to Runs; no local mutations.
4. **Definitions**: Segmented Agent Catalog and execution-sorted Workflow.
5. **System Readiness**: Settings section replacing Pilot Readiness tab.
6. **Live Timeline**: Batched flushes, merged tool cards, summary cards, Reduce Motion support.
7. **Parity**: Standalone Approvals removal after verified inline parity.
