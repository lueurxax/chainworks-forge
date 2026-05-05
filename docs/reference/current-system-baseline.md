# Current System Baseline

Stable reference map for the current implemented Chainworks Forge system.

## Purpose

The repository now has enough implemented slices that proposal and audit work should not need to reconstruct the product from scattered code and partial proposal history each time.

This document defines the current-system baseline for `HEAD`:

- what the product currently is,
- which reference docs are authoritative for each subsystem,
- which flows are implemented and considered stable,
- and which boundaries should be treated as baseline truth during review.

It is not a future-state proposal.

## Product snapshot

Chainworks Forge is a macOS operator tool for turning an idea into a controlled multi-agent workflow, with explicit approvals, persisted evidence, and repo-backed delivery for the current MVP path.

At the current baseline, the product includes:

- YAML-defined workflows and agent catalogs,
- a compiled execution engine with resume, approvals, loops, and artifact persistence,
- thin GraphQL-only UI rewrite (P031) ensuring all production truth is read from server projections,
- lead conflict mediation for same-run resolution of workflow conflicts,
- capacity-aware scheduling, fairness, executor backpressure, SQLite write serialization, and host interruption recovery (Rust daemon),
- DbWriter scaffold and evidence spool metadata schema (P075 Phase 1; runtime routing, file spooling, and storageHealth diagnostics deferred to Phases 2–8),
- catalog-owned skill resolution with frozen runtime injection and operator-visible skill truth,
- live ACP-backed execution for real provider sessions,
- ACP-only runtime transport with adapter-specific subprocess execution,
- bounded artifact discovery and engine-owned settlement pipeline,
- provider toolchain cache mapping for isolated Xcode and Go build roots,
- per-agent MCP policy resolution with persisted requested/predicted/actual/denied truth,
- canonical execution-truth, recovery, and report-read behavior for settled attempts,
- provider settings, diagnostics, and frozen provider bindings,
- an operator shell with run progress, recovery, comparison, artifact inspection, and approvals,
- segmented run surfaces with deterministic pane routing, a focused timeline inspector, and shared hierarchical artifact browsing,
- a proposal-loop feedback-fidelity layer with review-corpus bundling, backlog carry-forward, writer coverage, and targeted rereview,
- an implemented Forge design-system and brand-application layer across shell, run, setup, and recovery surfaces,
- idea archive/restore lifecycle,
- workflow-topology rendering in run detail,
- repo-backed full delivery with dedicated worktrees and manual release,
- worktree mutation barrier protecting concurrent read/write and orchestrated sync (Proposal 064),
- run worktree main sync and cross-run knowledge capsules (Proposal 064 Phase 0 contract freeze),
- implementation completeness and handoff contract with structured status and verification truth,
- fail-closed server parity harness with generation-scoped storage and runtime publication,
- rejected implementation approval loopback to proposal refinement,
- MVP benchmark/sign-off state and replayable `GO/HOLD` decision logic,
- Forge Steward system-health analysis,
- a stable design-kit authority for future visual changes.

## Canonical subsystem map

Use these reference docs as the current source of truth:

| Area | Authoritative doc |
|---|---|
| Persistence model | [domain-model.md](domain-model.md) |
| YAML and catalog parsing | [yaml-dsl-parser.md](yaml-dsl-parser.md) |
| Execution engine | [workflow-execution-engine.md](workflow-execution-engine.md) |
| Artifact discovery and settlement | [artifact-discovery-and-settlement-optimization.md](artifact-discovery-and-settlement-optimization.md) |
| Frozen runtime and resume truth | [runtime-contract.md](runtime-contract.md) |
| Skill resolution and runtime injection | [skill-resolution-and-runtime-integration.md](skill-resolution-and-runtime-integration.md) |
| Per-agent MCP policy and runtime validation | [per-agent-mcp-policy-and-runtime-validation.md](per-agent-mcp-policy-and-runtime-validation.md) |
| ACP runtime transport | [acp-runtime-transport.md](acp-runtime-transport.md) |
| Execution truth and recovery | [execution-truth-and-recovery.md](execution-truth-and-recovery.md) |
| Proposal-loop feedback fidelity | [proposal-loop-feedback-fidelity-and-rereview.md](proposal-loop-feedback-fidelity-and-rereview.md) |
| Live provider-backed proposal loop | [live-provider-execution-slice.md](live-provider-execution-slice.md) |
| Operator shell | [operator-experience.md](operator-experience.md) |
| Run surface IA and artifact hierarchy | [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md) |
| Provider/settings platform | [provider-platform.md](provider-platform.md) |
| Provider/model provenance | [provider-binding-truth.md](provider-binding-truth.md) |
| Stop/cancel truth | [run-control.md](run-control.md) |
| Idea-owned workspace root | [project-workspace-contract.md](project-workspace-contract.md) |
| Idea archive/restore | [idea-lifecycle.md](idea-lifecycle.md) |
| Parity harness | [p041-generated-artifact-schemas.md](p041-generated-artifact-schemas.md) |
| Workflow map | [live-workflow-map.md](live-workflow-map.md) |
| Repo-backed full delivery | [full-mvp-delivery.md](full-mvp-delivery.md) |
| MVP sign-off | [mvp-sign-off.md](mvp-sign-off.md) |
| Steward | [forge-steward.md](forge-steward.md) |
| Test strategy and gates | [test-suite-architecture.md](test-suite-architecture.md), [test-gates.md](test-gates.md), [agent-ui-test-execution.md](agent-ui-test-execution.md) |
| Design-system adoption | [design-system-and-brand-application.md](design-system-and-brand-application.md) |
| UI/brand design authority | [chainworks_forge_design_kit_v1.md](chainworks_forge_design_kit_v1.md) |
| UI action boundary | [ui-action-boundary.md](ui-action-boundary.md) |
| GraphQL read contract | [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md) |

## Canonical product boundaries

### Operator boundary

The app is not a chat shell.
It is an operator surface where the engineer should be able to answer:

- what is running,
- what is blocked,
- what requires approval,
- what evidence exists,
- and what safe next action is available.

That boundary is owned by [operator-experience.md](operator-experience.md).

### UI Action Boundary

The macOS UI is a GraphQL-only observer and approval console.

Current baseline:

- production workflow truth is read from GraphQL projections,
- UI state is limited to presentation, server-derived caches, and freshness handling,
- UI mutations are limited to `approveApproval` and `rejectApproval`,
- all non-approval operator actions are MCP-only,
- governed UI screens provide diagnostic identifiers and instructions for MCP-owned workflows when an action is external.

That boundary is owned by [ui-action-boundary.md](ui-action-boundary.md), with read-shape details in [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md).

### Provider boundary

The current MVP provider families are:

1. `claude`
2. `gemini`
3. `codex`
4. `auggie`
5. `junie`

That provider set is baseline truth across settings, run start, binding freeze, diagnostics, and sign-off.

### Workspace boundary

Project-backed runs must not infer their source tree from app cwd.

Current baseline:

- ideas own explicit workspace/project roots,
- runs freeze workspace truth at creation time,
- repo-backed delivery provisions one dedicated writable worktree per run,
- read-only repo-backed stages use explicit frozen project roots.

### Delivery boundary

The current repo-backed execution path is the `Full MVP Live` slice:

- proposal loop,
- implementation loop,
- implementation review/refine,
- explicit manual release gate,
- deterministic release services,
- evidence export.

That is baseline truth, not an aspirational proposal.

### Sign-off boundary

MVP sign-off is a separate persisted layer outside the operational `Run` aggregate.

Current baseline requires:

- benchmark records,
- replayable decision snapshots,
- current-head evidence,
- explicit `GO/HOLD`.

## Canonical flows implemented at the current baseline

The following flows should be treated as implemented system behavior:

1. idea creation and archive/restore,
2. provider setup and pilot-readiness validation,
3. live proposal-loop execution with approval pause/resume,
4. lead conflict mediation for same-run resolution of workflow conflicts,
5. run progress, artifact inspection, and recovery from the operator shell,
6. workflow-map rendering and fallback handling,
7. provider toolchain cache mapping and isolated build execution,
8. repo-backed full delivery using dedicated worktrees and manual release,
9. implementation self-assessment and handoff routing,
10. rejected implementation approval loopback to proposal refinement,
11. evidence-pack export for repo-backed runs,
12. benchmark/sign-off evaluation and export.

## Current review posture

When reviewing a proposal or implementation on the current repository, start from these assumptions unless the reviewed artifact says otherwise:

1. the product already has a stable operator shell,
2. provider/settings/remediation are already baseline features,
3. repo-backed delivery is already baseline behavior,
4. MVP sign-off is already a stable reference layer,
5. removed proposal files should not be treated as active dependencies once their truth has been promoted into `docs/reference/`.

In other words:

- prefer current reference docs over old proposal lineage,
- treat review work as delta analysis on top of the implemented baseline,
- and only fall back to source archaeology when a stable doc is genuinely missing.

## Known intentional gaps in this baseline document

This baseline map is intentionally not a full architecture book.
It does not restate every field, type, or UI detail from each subsystem doc.

Use it to orient review and planning work quickly, then jump to the subsystem references above for detailed contracts.

## Verification posture

Subsystem-level verification baselines are summarized inside the subsystem reference docs.
Use those documents as the current verification and contract source of truth rather than older proposal, audit, review, or evidence trails.
