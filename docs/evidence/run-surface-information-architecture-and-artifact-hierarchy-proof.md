# Run Surface Information Architecture and Artifact Hierarchy Proof

Current implementation and proof status for the run-surface information architecture and artifact-hierarchy slice consolidated from Proposal 024.

## Status

| Field | Value |
|---|---|
| Slice | Run Surface Information Architecture and Artifact Hierarchy |
| Source contract | [../reference/run-surface-information-architecture-and-artifact-hierarchy.md](../reference/run-surface-information-architecture-and-artifact-hierarchy.md) |
| Current implementation status | Implemented |
| Current readiness | Ready |
| Primary evidence owner | approved-host `proposal-024` gate plus focused local pane-routing and hierarchy suites |
| Last consolidated documentation refresh | `2026-04-03` |

## What is considered proven

The accepted proof story for this slice supports these claims:

- `Runs Home` and idea-side run progress now use segmented shells instead of one long detail stack,
- the two run surfaces intentionally keep different pane sets and different operator emphasis,
- action-critical states use deterministic pane-priority routing instead of relying on stale remembered pane state,
- the focused timeline inspector is subordinate to the workflow-map owner path rather than a parallel run tool,
- artifact browsing in both surfaces is driven by one shared hierarchical projection,
- promoted artifacts remain first-class,
- metadata demotion did not break the surviving run-owned report/export/repo-backed continuity path,
- and the canonical `proposal-024` gate exists and passed with honest approved-host UI execution.

## Accepted evidence sources

The current proof story is built from:

- the stable contract in [../reference/run-surface-information-architecture-and-artifact-hierarchy.md](../reference/run-surface-information-architecture-and-artifact-hierarchy.md),
- a green local macOS `build-for-testing` run on the implemented tree,
- green focused non-UI suites:
  - `Proposal024RunSurfaceTests`
  - `RunArtifactHierarchyBuilderTests`
- a green approved-host targeted repro for:
  - `testCompletedRunExportHubSurface`
  - `testProposal024FocusedTimelineInspectorSurface`
- a green approved-host `./scripts/test-gate.sh proposal-024` run on a same-tree proof workspace.

Accepted result-bundle anchors from the final closeout:

- targeted UI repro bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-targeted/p024-fix-3-20260403.xcresult`
- canonical non-UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-024-non-ui-20260403-101652.xcresult`
- canonical UI bundle: `/var/folders/hh/ztmrr5z96xnbxvlcxyf1vxsc0000gp/T/chainworks-test-gates/proposal-024-ui-20260403-101724.xcresult`

## Current interpretation

This slice should now be treated as implemented baseline behavior, not as an active proposal.

The important progression was:

1. the segmented run-shell refactor and hierarchy builder landed in code,
2. workflow-map ownership and pane routing were made explicit instead of implicit overflow,
3. metadata demotion was closed without losing repo-backed delivery/export continuity,
4. the approved-host canonical gate was added and passed on the same implemented tree.

## Remaining caution

The remaining caution is operational, not contractual:

- the strongest UI proof still depends on the approved remote macOS host,
- later heads should rerun `proposal-024` instead of inheriting a prior green bundle by assumption,
- the canonical gate keeps the historical proposal label for reproducibility even though the slice is no longer proposal-owned documentation.

That caution does not reopen the contract.

## Recommended usage

Use:

- [../reference/run-surface-information-architecture-and-artifact-hierarchy.md](../reference/run-surface-information-architecture-and-artifact-hierarchy.md) for the stable run-surface contract,
- [../reference/operator-experience.md](../reference/operator-experience.md) for the wider operator-shell baseline,
- [../reference/live-workflow-map.md](../reference/live-workflow-map.md) for topology semantics,
- [../reference/test-gates.md](../reference/test-gates.md) for the canonical approved-host proof lane.
