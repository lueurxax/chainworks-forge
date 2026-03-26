# Live Workflow Map

Stable reference for the run-topology visualization baseline that was previously carried by Proposal 010's live workflow-visualization slice.

## Purpose

Run progress should be readable as workflow topology, not only as a structured log.

The operator should be able to see:

- where execution is now,
- which agents are active,
- which work is completed,
- which work has not started,
- where loops are happening,
- and how many deterministic handoffs have occurred.

## Scope

This reference covers:

- workflow-map entry points,
- topology rendering expectations,
- state vocabulary,
- agent activity grouping,
- handoff counters,
- loop visibility,
- fallback behavior when a map cannot be derived.

It does not define repo-backed delivery or release topology.

## Shell ownership

The workflow map is not a new top-level destination.

Canonical operator path:

1. `RunsHomeView` or idea detail,
2. run progress / run detail,
3. workflow-map pane.

The map extends the existing run-detail owner path.

## Data model

The map is derived from frozen run/workflow state and live execution state.

Relevant layers:

- `WorkflowMapProjectionService`
- `WorkflowMapView`
- `AgentActivityPanel`
- handoff/loop telemetry attached to run progress

The map must remain deterministic for the same frozen run state.

## State vocabulary

The map and its related activity panels use this operator-facing state vocabulary:

- `not_started`
- `ready`
- `thinking`
- `waiting_input`
- `completed`
- `failed`
- `skipped`

The operator should not need to inspect raw logs just to distinguish those states.

## Topology requirements

The primary map surface must show:

- stages as legible nodes,
- directed edges between stages,
- current stage emphasis,
- loop direction,
- current iteration when looping,
- deterministic handoff counts,
- active concurrency when more than one agent is working.

## Agent activity panel

Per-agent activity is grouped into:

1. active,
2. completed,
3. not started.

Each entry should expose:

- agent title,
- task name,
- provider/model/effort binding as currently resolved,
- current execution state,
- lightweight timing/counter context.

## Communication counters

The product does not invent hidden free-form chat between agents.

The visible counters represent deterministic handoff/communication events derived from the workflow and orchestrator, not speculative conversation volume.

## Fallback behavior

When a full topology cannot be derived, the operator still needs a truthful surface.

Fallback state must:

- say that the workflow map is unavailable,
- avoid pretending a topology exists,
- keep run detail usable,
- preserve the rest of the run-progress view.

## Proof expectations

The workflow-map baseline is only trustworthy when both of these states are proven:

- primary map rendering,
- fallback/unavailable rendering.

Those proofs belong in focused macOS UI evidence, not only in source review.
