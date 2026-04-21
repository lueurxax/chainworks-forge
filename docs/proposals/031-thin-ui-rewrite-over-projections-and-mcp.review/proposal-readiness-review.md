# Consolidated Proposal Review

Proposal: `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md`
Mode: `auto` via `proposal-review-router` -> `proposal-readiness`
Generated: 2026-04-17
Validation posture: no build, UI run, daemon startup, benchmark, load, fuzz, or test-gate execution was attempted.
Evidence pack: `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/evidence-pack.md`

## Routing

Selected reviewers:

| Reviewer | Role in this review | Evidence |
|---|---|---|
| `apple_ux_reviewer` | Operator confidence, usability, recovery clarity, and debuggability during the user-visible thin-client cutover. | `DOC-01`, `BASE-03`, `BASE-04` |
| `apple_arch_reviewer` | SwiftUI/SwiftData state ownership, navigation, local service teardown, and client cache/presentation boundary. | `DOC-01`, `CODE-01`, `CODE-02`, `CODE-03` |
| `api_contract_reviewer` | GraphQL read projections, subscriptions/freshness, MCP command tools, capability IDs, and audit payload contracts. | `DOC-01`, `DEP-01`, `DEP-03`, `CODE-04`-`CODE-09` |
| `observability_rollout_reviewer` | Cutover prerequisites, gate bundle, daemon unavailable/degraded states, rollback/hold criteria. | `DOC-01`, `DEP-02`, `CODE-10` |
| `chainworks_execution_truth_reviewer` | Projection truth, MCP truth, command journal, and durable run/stage/approval/recovery ownership. | `CONFIG-02`, `BASE-05`, `CODE-02`-`CODE-08` |

Rejected close alternatives:

| Reviewer | Reason rejected |
|---|---|
| `macos_ui_reviewer` | The draft does not specify concrete visual layout, component, accessibility, or platform UI changes; UX/architecture risks dominate. |
| `rust_arch_reviewer` | Rust implementation shape is represented through API/projection contract findings; P031 does not specify new Rust module boundaries. |
| `rust_reliability_reviewer` | Reliability risk is primarily cutover/execution-truth ownership here, covered by rollout and the repo-local execution-truth reviewer. |
| `rust_security_reviewer` | P031 does not change northbound auth/capability policy; P029 owns that surface. |
| `product_reviewer` | Metrics, launch experiments, prioritization, and decision checkpoints are not central to the draft. |
| Go reviewers | No Go backend/module seam is implicated. |

## Fingerprint summary

| Tag type | Tags |
|---|---|
| Stack tags | `macos`, `apple-client`, `rust-backend`, `shared-api`, `cross-stack` |
| Surface tags | `ui`, `ux`, `navigation`, `architecture`, `state-management`, `api-contract`, `rollout` |
| Risk tags | `backward-compatibility`, `data-loss`, `multi-service-coordination`, `operability-sensitive`, `user-trust` |

All tags trace to evidence IDs in the evidence pack.

## Baseline status

Baseline status: fresh enough for proposal-readiness.

The review reused `.review-baselines/current-system-baseline.md`, current operator/run-surface/execution-truth references, and narrow Swift/GraphQL/MCP/code-gate slices. No full repo remap was performed.

## Proposal completeness judgment

Overall readiness: `Red`
Confidence: `High`
Release blockers: `3`
Non-blocking issues: `1`

P031 is directionally coherent, but not implementation-ready. It describes the desired ownership boundary, yet it does not define the concrete read contracts, command/action matrix, client-state teardown plan, or proof gate needed to execute a safe user-visible cutover.

## Findings

### [P1] Stage/read projection contract is not executable

Evidence: `DOC-01`, `DEP-03`, `CODE-04`, `CODE-05`, `CODE-06`.

P031 says views render from GraphQL projections/queries and requires run/stage/approval/artifact/report state from service projections. The draft does not define which GraphQL query/projection owns each named surface. Current GraphQL already shows the ambiguity: `runs` uses projection rows, but `stages(runID:)` reads canonical stage rows directly and leaves projection-only fields empty unless a different path is chosen.

Required proposal change: add a surface-by-surface read contract for Runs home, Run detail, Stage detail, Approval inbox, Artifact viewer, Report viewer, Experiment comparison, and Runtime health.

Acceptance criteria: each surface names query/subscription, projection owner, required fields, freshness/staleness behavior, and forbidden client inference; stage detail either consumes `StageSummaryRow` or explicitly documents why it does not.

### [P1] MCP action surface names commands that do not exist or are explicitly deferred

Evidence: `DOC-01`, `DEP-01`, `CODE-07`, `CODE-08`.

P031 says all operator mutations flow through MCP and lists reset session, compare, and run experiments alongside start/approve/retry/cancel. Current first-wave MCP tools do not include reset-session, compare, experiments, clone, runtime-health, or agent/session second-wave commands; P029 explicitly defers those. The draft does not say whether P031 creates those tools or removes/defer-disables the UI actions.

Required proposal change: add an operator action matrix mapping every action to MCP tool, command/direct owner, audit/journal behavior, capability ID, and status (`in P031`, `deferred`, or `removed from UI`).

Acceptance criteria: every mutation reachable from P031-owned screens either routes through a registered MCP command tool or is explicitly deferred with a proposal owner and disabled/fallback UI behavior.

### [P1] SwiftData/local service teardown lacks an ownership and cutover map

Evidence: `DOC-01`, `CODE-01`, `CODE-02`, `CODE-03`, `BASE-05`.

The migration strategy says to remove obsolete local state stores and client-owned orchestration, but it does not inventory current owners. Runs Home still reads SwiftData through `@Query`, while recovery sheets directly call `RecoveryCoordinator`, `RunPlanCompiler`, and `ExecutionService` for retry/resume/reset/clone. Without an owner map, P031 can leave the old local runtime active behind a thin read facade.

Required proposal change: add a Swift cutover inventory for models, views, services, previews/test seeds, and action owners. For each owner, state whether it is deleted, retained as presentation/cache only, replaced by GraphQL read model, replaced by MCP command, or deferred.

Acceptance criteria: P031-owned screens have no direct client-owned mutation path; retained SwiftData is presentation/cache only; old direct service paths are guarded by tests/static checks or explicitly out of scope.

### [P2] Cutover prerequisites are named but not enforceable

Evidence: `DOC-01`, `DEP-02`, `DEP-03`, `CODE-10`.

P031 says it must wait for parity proof, daemon lifecycle dependability, and query/projection contract readiness, but those prerequisites remain draft-level here and no `proposal-031|p031` gate is registered. The draft also lacks hold/rollback criteria for a user-visible cutover.

Required proposal change: define a canonical P031 gate or explicit gate bundle that names prerequisite evidence from P041/P042/P043, GraphQL/MCP contract checks, daemon unavailable/degraded UI proof, and UI smoke coverage.

Acceptance criteria: `./scripts/test-gate.sh proposal-031` exists or P031 names an equivalent canonical bundle; the gate proves projection parity, MCP action routing, daemon degraded/offline UI behavior, and main-view operator smoke.

## Completeness matrix

| Area | Judgment | Notes |
|---|---|---|
| Problem framing | Complete | The desired thin-client ownership boundary is clear. |
| Read model contract | Incomplete | Surface-level GraphQL/projection/query/subscription matrix is missing. |
| MCP mutation contract | Incomplete | Action list exceeds current MCP tools without ownership or deferral. |
| Swift client teardown | Incomplete | Existing SwiftData/service owners are not inventoried. |
| UX/debuggability | Partial | Goals are stated, but stale/offline/freshness and recovery states are not specified. |
| Rollout/proof | Incomplete | No P031 gate, hold criteria, rollback path, or concrete prerequisite evidence list. |

## State and failure coverage

| Scenario | Covered? | Notes |
|---|---|---|
| GraphQL projection stale/missing | No | Proposal does not define stale state or fallback behavior. |
| Daemon unavailable/degraded | No | P042 is named, but P031 does not state UI behavior. |
| MCP command rejected/unauthorized | Partial | P029 owns auth; P031 does not define UI action-state handling. |
| Existing local SwiftData state during cutover | No | No dual-read/cache invalidation/rollback map. |
| Recovery action path after cutover | No | Current local recovery actions exist; P031 does not map each to MCP/deferred. |
| User-visible rollback to old client path | No | No rollback/kill-switch plan. |

## Metrics and checkpoint

Leading metric:

- Percentage of P031-owned screens whose visible state is sourced only from named GraphQL read models/projections.

Guardrail metric:

- Zero P031-owned operator mutations bypass MCP/CommandHandler/audit unless explicitly deferred and disabled in the UI.

Decision checkpoint:

- Do not start implementation until P031 has a read-model matrix, MCP action matrix, Swift cutover inventory, and canonical gate bundle.

## Evidence gaps

No evidence gap blocked routing or findings. The proposal itself is the incomplete artifact: the current repo has enough local evidence to show which contracts P031 must pin before implementation.
