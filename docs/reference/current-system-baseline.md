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
- GraphQL-only thin UI boundary ensuring governed SwiftUI workflow truth is read from server projections,
- boundary-first API authorization and audit policy through a daemon-injected `BoundaryPolicy` shared by GraphQL, MCP, and approval actionability paths,
- lead conflict mediation for same-run resolution of workflow conflicts,
- capacity-aware scheduling, fairness, executor backpressure, SQLite write serialization, and host interruption recovery (Rust daemon),
- DbWriter bounded MPSC executor with priority lanes, deadlines, busy-retry classification, heartbeat, lane-starvation watchdog, graceful shutdown drain with populated terminal-operation admission allowlist, Class B coalescing buffer (500 ms drain-all flush, 64-merge force-flush, 1024-key saturation reject), per-lane oldest-enqueued reporting on `WriteRejected`, evidence file spool module (canonical `evidence/runs/...` layout, run-id-bound write-time path ownership check, symlink-escape rejection, `0o600`/`0o700` POSIX modes, checksum + double `fsync` + atomic no-replace commit), bounded `sweep_evidence_orphans` walk that backfills `recovered_orphan` metadata for crash-orphaned evidence files, stream-hashes candidates, skips over-budget candidates before read, and is exposed via the `storage.reconcile_evidence_orphans` MCP tool, evidence spool metadata schema, storage write-pressure snapshots, typed operator-only GraphQL `storageHealth`, operator-only MCP `storage.health` / `storage.write_pressure` / `storage.evidence_spool_summary` diagnostics, fail-closed stale/degraded storage readback when live writer health is unavailable, and a fail-closed write-budget registry gate that rejects temporary rollout bypasses, production runtime transaction paths outside DbWriter-owned entrypoints, and operation-registry drift,
- catalog-owned skill resolution with frozen runtime injection and operator-visible skill truth,
- live ACP-backed execution for real provider sessions,
- ACP-only runtime transport with adapter-specific subprocess execution,
- runtime-owned bounded tool output and safe-search enforcement with shared policy versions, generated/build-root denylist, typed `tool_output_budget_preflight_denied` errors before provider context damage, wrapper-enforced line/byte caps, budget/unbounded-output classification before generic provider fallback, quarantine for poisoned sessions, and `runtime.health.toolOutputGuard` policy/enforcement readback,
- Junie `code_writer` runtime hardening with strict completion-boundary subtypes, engine-synthesized failure envelopes, staged per-output repair settlement, runtime preflight/remediation, and post-preflight provider launch capacity leasing,
- targeted retry authority with exact stage-execution retry settlement, authority-history readback, startup orphan retry repair, and retry payload recovery diagnostics,
- P082 recovery/retry state-machine matrix readbacks for operator MCP/report/run-report/release diagnostic lanes, covering startup repair, retry rejection, stale ownership, side-effect holds, cancellation interleavings, late-output quarantine, and crash/replay proof gates,
- observe-only auto-retry observation ledger with JSONL observations, canonical known-issue catalog, MCP readback, and rollup tooling,
- bounded artifact discovery and engine-owned settlement pipeline,
- provider toolchain cache mapping for isolated Xcode and Go build roots,
- per-agent MCP policy resolution with persisted requested/predicted/actual/denied truth,
- canonical execution-truth, recovery, and report-read behavior for settled attempts,
- provider settings, diagnostics, and frozen provider bindings,
- an operator shell with consolidated Runs, Ideas, Definitions, and Settings surfaces; run progress, recovery, comparison, artifact inspection, and approvals,
- segmented run surfaces with deterministic pane routing, a focused timeline inspector, and shared hierarchical artifact browsing,
- a proposal-loop feedback-fidelity layer with review-corpus bundling, backlog carry-forward, writer coverage, and targeted rereview,
- an implemented Forge design-system and brand-application layer across shell, run, setup, and recovery surfaces,
- idea archive/restore lifecycle,
- workflow-topology rendering in run detail,
- run-start rollout-contract preflight that blocks implementation work enqueue under enforce mode and exposes a four-lane operator readback (run report, MCP, release receipt, GraphQL),
- repo-backed full delivery with dedicated worktrees and manual release,
- worktree mutation barrier protecting concurrent read/write and orchestrated sync (Proposal 064),
- run worktree main sync and cross-run knowledge capsules (Proposal 064 Phase 0 contract freeze),
- implementation completeness and handoff contract with structured status and verification truth,
- fail-closed server parity harness with generation-scoped storage and runtime publication,
- rejected implementation approval loopback to proposal refinement,
- MVP benchmark/sign-off state and replayable `GO/HOLD` decision logic,
- Forge Steward system-health analysis,
- a stable design-kit authority for future visual changes,
- agent work continuation and lead-directed same-session resumption: `agents.continue_work`, `agents.continuation_status`, and `agents.continuation_candidates` MCP commands for eligible stage-owned `code_writer` agent executions, persisted continuation/side-effect ledger/supervised-worker/provider-process tables and durable metric events (SQLite migrations `065_p086_agent_work_continuations.sql`, `066_p086_supervised_worker_provider_process.sql`, and `067_p086_continuation_metric_events.sql`), materialized Draft 2020-12 JSON Schemas for canonical requests/responses and continuation artifacts, admission with `live_handle_continuation` and decision-artifact-validated `lead_auto`, frozen-catalog `continuation_capability` opt-in, release/publish/git-push/upload/distribution stage rejection, unresolved P078 side-effect rejection, and a background worker. `BackgroundExecutor` also inspects completed lead-agent artifacts for `lead_continuation_decision_v1`; a valid decision with matching `continuation_instruction` hash is admitted through the same durable continuation transaction and enqueues `WorkItemKind::ProcessContinuation` without a manual MCP call. `BackgroundExecutor` runs a continuation admission-timeout sweeper and processes `WorkItemKind::ProcessContinuation` items through `run_continuation_worker`, which walks the `accepted → queued → starting → running → prompt_sent → observing → worktree_observed → finalizing → succeeded | no_progress | failed` state machine, registers the live ACP provider pid/process-group for restart recovery, inserts the `provider_send` side-effect ledger row idempotently before the durable `prompt_sent` transition, builds the canonical P086 mode-reset prompt from admitted context, and sends it through the existing ACP live-session reuse path. Startup recovery uses the durable provider process binding to verify or fail-closed orphan ACP reap after daemon restart and records signal/deadline evidence for orphan reap attempts. Duplicate prompt replay reconciles only from post-continuation worktree evidence paired with a committed `provider_send` ledger row; mutation without provider-send evidence settles as `no_progress`. GraphQL exposes passive continuation status, candidate, run-history, and metrics-summary readback, including useful-progress/no-progress rates, trigger-specific success rates, follow-up validation success rate, average time saved, provider/session budget impact, and resurrection attach success/failure totals. The macOS Overview card renders those server-owned fields without adding UI command authority. Per-adapter `provider_session_resurrection` remains an explicit fail-closed unsupported mode until a provider declares attach/resume support.

## Code-Derived Baseline Inventory

When implementation landed outside proposal closeout, the following code surfaces are the primary current-truth inventory:

| Surface | Code owner | Reference owner |
|---|---|---|
| MCP tool and capability registry | `control-plane/crates/domain/src/capabilities.rs`, `control-plane/crates/mcp-server/src/tools/mod.rs`, `control-plane/crates/mcp-server/src/tools/` | [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md), [rust-control-plane.md](rust-control-plane.md) |
| GraphQL read, approval mutation, and subscription boundary | `control-plane/crates/graphql-server/src/schema.rs` | [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md), [ui-action-boundary.md](ui-action-boundary.md) |
| Swift local GraphQL enforcement and approval-only mutation path | `Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift` | [ui-action-boundary.md](ui-action-boundary.md), [swift-macos-boundary-contract.md](swift-macos-boundary-contract.md) |
| Daemon lifecycle, endpoint publication, failed-serve mode, and audit checkpointing | `control-plane/crates/daemon/src/main.rs`, `Chainworks Forge/Views/DaemonLifecycleSurface.swift` | [local-daemon-lifecycle-supervision-and-packaging.md](local-daemon-lifecycle-supervision-and-packaging.md), [rust-control-plane.md](rust-control-plane.md) |
| Storage health, hot-read guards, evidence spool, maintenance repair, and projection maintenance | `control-plane/crates/db/src/`, `control-plane/crates/mcp-server/src/tools/storage.rs`, `control-plane/crates/graphql-server/src/types/storage.rs` | [rust-control-plane.md](rust-control-plane.md), [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md) |
| Agent continuation and lead-directed resumption | `control-plane/crates/mcp-server/src/tools/agents.rs`, `control-plane/crates/engine/src/` | [agent-work-continuation.md](agent-work-continuation.md) |
| Boundary policy, caller class, audit log, idempotency, and operator alerts | `control-plane/crates/auth/src/`, `control-plane/crates/db/src/repos/audit_log.rs`, `control-plane/crates/graphql-server/src/schema.rs`, `control-plane/crates/mcp-server/src/tools/runtime.rs` | [boundary-first-api-auth-contract.md](boundary-first-api-auth-contract.md), [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md), [ui-action-boundary.md](ui-action-boundary.md) |
| Bounded tool-output and safe-search guard | `control-plane/crates/domain/src/tool_policy.rs`, `control-plane/crates/acp/src/transport.rs`, `control-plane/crates/acp/src/adapters/codex.rs`, `control-plane/crates/mcp-server/src/tools/runtime.rs` | [bounded-tool-output-and-safe-search-policy.md](bounded-tool-output-and-safe-search-policy.md), [acp-runtime-transport.md](acp-runtime-transport.md), [mcp-northbound-control-plane-server.md](mcp-northbound-control-plane-server.md) |

This inventory is intentionally code-first. If a proposal file is missing, stale, or never closed out, update the matching reference doc from these owners rather than re-promoting proposal text.

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
| Bounded tool output and safe search | [bounded-tool-output-and-safe-search-policy.md](bounded-tool-output-and-safe-search-policy.md) |
| Execution truth and recovery | [execution-truth-and-recovery.md](execution-truth-and-recovery.md) |
| Recovery/retry matrix and proof gate | [recovery-retry-state-machine-test-matrix.md](recovery-retry-state-machine-test-matrix.md) |
| Rust control plane, scheduler, targeted retry authority, and retry payload recovery | [rust-control-plane.md](rust-control-plane.md) |
| Escalation policy and chain management | [escalation-policies.md](escalation-policies.md) |
| Auto-retry observation ledger | [auto-retry-observation-ledger.md](auto-retry-observation-ledger.md) |
| API/auth boundary matrix, audit, and idempotency | [boundary-first-api-auth-contract.md](boundary-first-api-auth-contract.md), [swift-macos-boundary-contract.md](swift-macos-boundary-contract.md) |
| Proposal-loop feedback fidelity | [proposal-loop-feedback-fidelity-and-rereview.md](proposal-loop-feedback-fidelity-and-rereview.md) |
| Live provider-backed proposal loop | [live-provider-execution-slice.md](live-provider-execution-slice.md) |
| Operator shell | [operator-experience.md](operator-experience.md) |
| macOS operator navigation and read-model UX | [macos-operator-navigation.md](macos-operator-navigation.md) |
| Run surface IA and artifact hierarchy | [run-surface-information-architecture-and-artifact-hierarchy.md](run-surface-information-architecture-and-artifact-hierarchy.md) |
| Provider/settings platform | [provider-platform.md](provider-platform.md) |
| Provider/model provenance | [provider-binding-truth.md](provider-binding-truth.md) |
| Stop/cancel truth | [run-control.md](run-control.md) |
| Idea-owned workspace root | [project-workspace-contract.md](project-workspace-contract.md) |
| Idea archive/restore | [idea-lifecycle.md](idea-lifecycle.md) |
| Parity harness | [p041-generated-artifact-schemas.md](p041-generated-artifact-schemas.md) |
| Workflow map | [live-workflow-map.md](live-workflow-map.md) |
| Repo-backed full delivery | [full-mvp-delivery.md](full-mvp-delivery.md) |
| Run-start rollout-contract preflight | [executable-rollout-gate-template.md](executable-rollout-gate-template.md) |
| MVP sign-off | [mvp-sign-off.md](mvp-sign-off.md) |
| Steward | [forge-steward.md](forge-steward.md) |
| Test strategy and gates | [test-suite-architecture.md](test-suite-architecture.md), [test-gates.md](test-gates.md), [agent-ui-test-execution.md](agent-ui-test-execution.md) |
| Design-system adoption | [design-system-and-brand-application.md](design-system-and-brand-application.md) |
| UI/brand design authority | [chainworks_forge_design_kit_v1.md](chainworks_forge_design_kit_v1.md) |
| UI action boundary | [ui-action-boundary.md](ui-action-boundary.md) |
| GraphQL read contract | [query-projections-and-client-consumption-contract.md](query-projections-and-client-consumption-contract.md) |
| Agent work continuation API contracts | [agent-work-continuation.md](agent-work-continuation.md) |

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
P081 changes authorization, audit, idempotency, and actionability semantics for
that boundary; it does not add non-approval UI mutations or move non-approval
operator control off MCP.

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
12. benchmark/sign-off evaluation and export,
13. durable side-effect ledger, release settlement, and reconciliation,
14. targeted retry authority, exact retry-stage settlement, startup orphan retry repair, and retry payload recovery diagnostics,
15. boundary runtime diagnostics and operator alerts derived from `BoundaryPolicy` and audit-log health,
16. storage health, write-pressure, evidence-spool, orphan-reconciliation, maintenance-slot, and projection-maintenance readback,
17. session observability queries/subscriptions with live auth recheck and fail-closed reload behavior,
18. agent continuation candidate/status/history/metrics readback plus operator/lead-directed continuation admission,
19. escalation readback, attention surfacing, and runbook handoff without UI-side escalation command authority.

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
