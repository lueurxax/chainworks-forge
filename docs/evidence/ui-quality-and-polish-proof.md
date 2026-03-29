# UI Quality and Visual Polish Proof

Current implementation and proof status for the UI quality, accessibility, and visual polish slice that was previously tracked by Proposal 012.

## Status

| Field | Value |
|---|---|
| Slice | UI Quality and Visual Polish |
| Source contract | [../reference/ui-quality-and-polish.md](../reference/ui-quality-and-polish.md) |
| Current implementation status | Implemented |
| Current readiness | Ready with Risks |
| Primary evidence owner | approved-host current-head UI proof plus local build and preview-backed owner renders |
| Last consolidated audit | `R7` on `2026-03-29` |

## What is considered proven

The current proof set supports these claims:

- previously audited operator surfaces now follow the implemented readability and density contract,
- the bounded adopter slice uses shared status semantics instead of fragmented badge drift,
- the current `1024×768` minimum-window proof is attached to real owner surfaces rather than proxy screens,
- provider/setup-adjacent UI proof runs on the approved host on the same head,
- the approved-host `proposal-012` gate exercises secondary runtime surfaces and bounded accessibility proof,
- preview-backed Appendix A surfaces were re-rendered on the implemented tree,
- shell-level UI smoke remains green on the approved host.

## Accepted evidence sources

The accepted proof story for this slice is built from:

- the implemented contract in [../reference/ui-quality-and-polish.md](../reference/ui-quality-and-polish.md),
- a green local macOS build on the implemented tree,
- fresh Xcode Preview renders for the preview-backed owner surfaces,
- a green approved-host `proposal-006` gate on the same synced tree,
- a green approved-host `proposal-012` gate on the same synced tree,
- a green approved-host `ui-smoke` gate on the same synced tree.

## Current-head proof interpretation

The critical transition in the final rounds was evidentiary, not architectural.

Early audits showed that the UI work existed but proof did not yet line up with the actual owner surfaces or the bounded adopter slice. Those gaps were later closed by:

1. moving the min-window proof onto real `RunsHomeView` and `IdeaListView` owners,
2. moving the adopter-slice accessibility proof away from proxy surfaces,
3. rerunning the canonical approved-host gates on the same current tree,
4. rerendering the preview-backed Appendix A surfaces after the latest UI changes.

The slice should now be treated as implemented reference behavior rather than an active proposal.

## Canonical proof gates

The implemented proving path uses these gates:

- `./scripts/test-gate.sh build`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-006"`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh proposal-012"`
- `ssh test@SMacBook.local "cd '/Users/test/chainworks-remote' && ./scripts/test-gate.sh ui-smoke"`

Interpretation:

- `proposal-006` proves provider/settings/readiness/onboarding-adjacent runtime surfaces,
- `proposal-012` proves the bounded adopter slice, explicit `1024×768` owner checks, and the secondary runtime surfaces named by the UI quality slice,
- `ui-smoke` proves shell continuity and top-level operator reachability,
- preview rerenders remain required because not every audited surface is runtime-driven in the same way.

## What remains risky

The remaining risk is operational rather than contractual.

- the strongest runtime proof still depends on the approved remote host,
- later trees must be reproved rather than inheriting these green results by assumption,
- some lower-level implementation comments still reference Proposal 012 lineage even though the contract is now stable documentation.

These do not reopen the UI quality contract itself.
They only constrain how broadly one current-head proof bundle should be generalized without rerun.

## Recommended usage

Use:

- [../reference/ui-quality-and-polish.md](../reference/ui-quality-and-polish.md) for the stable implemented UI-quality contract,
- [../reference/chainworks_forge_design_kit_v1.md](../reference/chainworks_forge_design_kit_v1.md) for the visual-system authority,
- [../reference/operator-experience.md](../reference/operator-experience.md) and [../reference/provider-platform.md](../reference/provider-platform.md) for adjacent functional baselines.

Do not treat the old Proposal 012 reviews and audits as the primary source anymore.
Those were implementation-trail artifacts on the way to the stable reference and proof pair above.
