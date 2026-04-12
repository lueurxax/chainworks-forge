# Output Contracts, Failure Evidence, and Narrow Recovery Proof

Current implementation and proof status for the output-contract, failure-evidence, retry-lineage, and bounded proposal-resilience slice consolidated from Proposal 013.

## Status

| Field | Value |
|---|---|
| Slice | Output Contracts, Failure Evidence, and Narrow Recovery |
| Source contract | [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md) |
| Current implementation status | Implemented |
| Current readiness | Ready with Risks |
| Primary proof owners | `Proposal013Tests`, `RuntimeSessionBridgeTests`, canonical `proposal-013` gate |
| Last consolidated documentation refresh | `2026-03-31` |

## What is considered proven

The accepted proof story for this slice supports these claims:

- proposal-review fan-out outputs are enforced against catalog-backed contract truth,
- `proposal_review_summary` is validated as a first-class aggregate contract,
- markdown or otherwise invalid reviewer and aggregate artifacts are preserved as evidence but rejected as canonical outputs,
- failed-stage evidence survives post-generation validation failure,
- same-run narrow retry remains available before clone-run when canonical stage evidence supports it,
- reports and recovery surfaces read canonical failed-stage evidence rather than summary-only heuristics,
- Tier 1 declarative contract fields are enforced or fail closed,
- oversized proposal drafting uses explicit compaction truth rather than silent collapse,
- and one canonical app-launched proof lane demonstrates blocked aggregate evidence plus narrow recovery through the shell-owned UI path.

## Accepted current-head proof owners

The strongest current-head proof owners are:

- `Proposal013Tests`
- `RuntimeSessionBridgeTests`
- `RecoveryCoordinatorTests`
- `Chainworks_ForgeUITests.testProposal013AppProofSurface`
- `scripts/test-gate.sh proposal-013`

High-signal proof examples on the current tree include:

- strict rejection of markdown-only proposal-review artifacts,
- strict rejection of markdown-only aggregate `proposal_review_summary`,
- fixture-backed blocked aggregate proof with canonical evidence and narrow retry,
- declarative coverage report persistence,
- compaction policy truth,
- and the app-launched Proposal 013 proof surface routed through the repo-owned UI-proof lane.

## Canonical app-level proof lane

The accepted app-level proof owner for this slice is singular:

- `ContentView.UISurface`
- `Chainworks_ForgeApp`
- `UITestDirectSurfaces`
- `Chainworks_ForgeUITests`
- `scripts/test-gate.sh proposal-013`

A proposal-local surface counts only when it is routed through that owner chain.

Standalone previews, ad-hoc harnesses, or orphaned UI scaffolds are useful development tools, but they do not satisfy slice acceptance by themselves.

## Consolidation note

The old Proposal 013 draft, review, evidence pack, research pack, and proposal-local implementation audits were implementation-trail artifacts.

They have been superseded by:

- [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md)
- this proof document

This slice should now be treated as stable reference behavior rather than an active proposal dependency.

## Remaining caution

The remaining caution is about proof packaging, not slice ownership:

- the canonical UI owner on approved hosts is green,
- but the `proposal-013` gate still relies on the built-in watchdog in `scripts/test-gate.sh` to terminate a stale post-success `xcodebuild` hang after success markers are already printed.

That keeps readiness at `Ready with Risks` rather than fully frictionless `Ready`.

## Recommended usage

Use:

- [../reference/output-contracts-failure-evidence-and-recovery.md](../reference/output-contracts-failure-evidence-and-recovery.md) for the stable contract,
- [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) for lower-layer settlement truth,
- [../reference/runtime-contract.md](../reference/runtime-contract.md) for snapshot and artifact rules,
- [../reference/test-gates.md](../reference/test-gates.md) for the canonical `proposal-013` verification lane.
