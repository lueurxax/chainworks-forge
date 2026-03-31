# Output Contracts, Failure Evidence, and Narrow Recovery

Stable reference for the implemented output-contract, failure-evidence, retry-lineage, and bounded proposal-resilience slice that was previously tracked by Proposal 013.

## Purpose

The runtime must be able to say, with persisted evidence rather than inference:

- which output contract was authoritative,
- whether a reviewer or aggregate step produced contract-valid artifacts,
- what canonical evidence survives when validation fails after generation,
- which same-run retry path remains valid before clone-run,
- and which declarative contract controls are enforced versus rejected.

For implementation and proof status, use [../evidence/output-contracts-failure-evidence-and-recovery-proof.md](../evidence/output-contracts-failure-evidence-and-recovery-proof.md).

## Scope

This reference covers:

- catalog-backed output-contract authority,
- strict proposal-review and aggregate summary contract enforcement,
- canonical validation-failure and failed-stage evidence,
- same-run retry lineage and artifact namespace rules,
- narrow recovery and report/export evidence references,
- declarative Tier 1 contract enforcement for `contracts.*` and `backend_profiles.*.structured_output`,
- and bounded proposal-drafting compaction truth.

It does not replace:

- lower-layer settlement and transport truth in [execution-truth-and-recovery.md](execution-truth-and-recovery.md),
- broader orchestrator topology in [workflow-execution-engine.md](workflow-execution-engine.md),
- frozen run state and artifact boundaries in [runtime-contract.md](runtime-contract.md),
- or operator-shell interaction rules in [operator-experience.md](operator-experience.md).

## Core Rules

### One contract authority

`AgentCatalog.contracts` remains the single contract authority for this slice.

`OutputContractResolverV2` is the only runtime reader that normalizes that authority for:

- `WorkflowOrchestrator`,
- `ArtifactManager`,
- `GooseSessionBridge`,
- `RunReportBuilder`,
- and recovery/report surfaces.

No runtime component may keep a second contract registry or silently override catalog truth with output-name heuristics.

### Mandatory adopters

The mandatory contract adopters in this slice are:

- `proposal_review_ui`
- `proposal_review_ux`
- `proposal_review_architect`
- `proposal_review_po`
- `proposal_review_summary`

The aggregate `proposal_review_summary` output is a first-class contract, not an implicit transition side effect.

### Structured-output modes are explicit

The runtime-normalized schema for this slice includes:

- `machine_format`
- `human_format`
- `validation_mode`
- `required_fields`
- `raw_artifact_name`
- `normalized_artifact_name`

Supported validation modes are:

- `strict_structured`
- `structured_with_human_companion`
- `human_only`

Rules:

- `strict_structured` may not silently accept prose in place of the machine payload.
- `structured_with_human_companion` must persist both the machine-valid artifact and the human companion artifact.
- if the product wants markdown, the contract must say markdown; if the contract says JSON, the runtime must require JSON.

### Aggregate inputs must already be contract-valid

Aggregate steps consume only normalized, contract-valid reviewer outputs.

Raw invalid reviewer artifacts remain evidence only and must not be treated as aggregate inputs.

This keeps aggregate transition truth tied to validated stage artifacts instead of markdown or partially parsed payloads that happened to exist on disk.

### Failure evidence survives post-generation validation failure

When validation fails after output generation, the runtime preserves canonical evidence rather than collapsing to summary-only status.

The durable evidence path for this slice includes:

- raw output artifacts,
- receipt artifacts,
- transcript artifacts,
- `ValidationFailureRecord`,
- and the stage-owned failed-stage evidence packet.

`ArtifactPersistenceOrderingPolicy` keeps the persistence order explicit: raw artifacts first, validation and evidence second, settlement last.

Reports, exports, and recovery surfaces should reference the canonical failed-stage evidence object rather than reconstructing the failure from loose file scans.

Because canonical evidence may contain sensitive data, operator-visible summaries should default to summarized or redacted presentation until explicit inspection is requested.

### Same-run retry keeps lineage and inspectable history

Same-run retry is distinct from clone-run.

For this slice:

- the failed attempt remains inspectable,
- the retry stays on the same logical frozen snapshot,
- retry artifacts use a disjoint namespace rather than overwriting prior attempt artifacts,
- artifact lineage metadata and reused-sibling references remain persisted,
- and recovery surfaces explain why same-run retry is valid before clone-run.

This retry truth is stage-owned and depends on the lower execution-truth substrate documented in [execution-truth-and-recovery.md](execution-truth-and-recovery.md).

### Recovery is narrow before clone-run

The canonical recovery surfaces remain:

- `RecoverySheet`
- `BlockedRunRecoveryView`

They must prefer the narrowest valid next action from canonical stage evidence:

- `Retry Failed Agent`
- `Retry Failed Stage`
- `Retry Aggregate Step`
- `Clone Frozen Snapshot`
- `Clone Current Config`

Clone-run is not an acceptable default when narrower recovery is still valid.

### Declarative Tier 1 contract fields are enforce-or-reject

The mandatory declarative runtime surfaces in this slice are:

- `contracts.*`
- `backend_profiles.*.structured_output`

Rules:

- no Tier 1 field may silently no-op,
- unsupported provider/schema combinations must fail in preflight,
- successful transport-level structured-output support does not remove post-generation contract validation,
- metadata-only or later-slice declarations must stay explicitly tiered rather than overclaimed.

`DeclarativeCoverageReport` is the persisted/testable inventory of that tiering.

### Proposal drafting compaction is explicit

`ProposalDraftCompactionPolicy` bounds oversized proposal outputs without silently dropping useful drafts.

When compaction is invoked, the runtime preserves:

- raw draft artifacts,
- compacted or normalized artifacts,
- compaction metadata,
- and outcome truth about whether the stage succeeded with compaction or failed despite compaction.

## Operator-Visible Outcomes

After this slice, a blocked proposal-review or aggregate stage should make all of the following explicit:

- whether an individual reviewer failed contract validation,
- whether the aggregate `proposal_review_summary` step failed or never produced its required output,
- where raw outputs, receipts, and transcripts live,
- which canonical failed-stage evidence object explains the block,
- which narrow recovery action is valid and why,
- and which declarative contract controls were actually enforced.

## Adjacent References

Use:

- [execution-truth-and-recovery.md](execution-truth-and-recovery.md) for lower-layer outcome and settlement truth,
- [workflow-execution-engine.md](workflow-execution-engine.md) for orchestrator and executor topology,
- [runtime-contract.md](runtime-contract.md) for frozen snapshot and artifact boundaries,
- [operator-experience.md](operator-experience.md) for shell and recovery presentation rules,
- [test-gates.md](test-gates.md) for the canonical `proposal-013` proof lane.
