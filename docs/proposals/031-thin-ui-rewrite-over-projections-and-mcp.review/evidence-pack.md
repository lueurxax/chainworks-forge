# Proposal Evidence Pack

Proposal: `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md`
Mode: `auto` -> `proposal-readiness`
Generated: 2026-04-17
Validation posture: local evidence review only. No build, UI run, daemon startup, simulator, benchmark, load, fuzz, or test-gate execution was run.
Router: `proposal-review-router` with repo-local `.codex/review-router.yaml` and `.codex/reviewers/chainworks-execution-truth.yaml` overlay.

## 1. Intake status

| Item | Status | Evidence IDs | Notes |
|---|---|---|---|
| Proposal file | Complete | DOC-01 | P031 draft reviewed in full. |
| Adjacent/dependency docs | Complete for affected slices | DEP-01, DEP-02, DEP-03, BASE-01 through BASE-05 | P029, P041, P042, P043 and current baseline docs consumed. P027 proposal file was not present under `docs/proposals/027-*` in this read pass; current baseline docs cover the implemented server/control-plane shape. |
| Baseline | Reused and narrowly refreshed | BASE-01, BASE-02 | `.review-baselines/current-system-baseline.md` plus current references were sufficient; no full repo remap. |
| Prior review artifacts | Absent/empty | HIST-01 | P031 `.review/` artifacts were not populated before this pass. |
| Current code-path map | Complete for proposal readiness | CODE-01 through CODE-10 | Mapped only affected Swift UI/local-state, GraphQL projection, MCP command, capability, subscription, and gate slices. |
| External research | Not used | N/A | Local repo evidence was sufficient. |

## 2. Evidence inventory

| Evidence ID | Source | Provenance | Key facts |
|---|---|---|---|
| DOC-01 | `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md:1-162` | Proposal | P031 proposes a user-visible cutover where UI renders read models, invokes MCP commands, stops owning workflow truth, and migrates in three phases. The draft is high-level and has no file inventory, GraphQL query matrix, MCP tool matrix, fallback/rollback plan, or proposal-specific gate. |
| DEP-01 | `docs/proposals/029-mcp-northbound-control-plane-server.md` | Dependency proposal | P029 defines current MCP tools and explicitly defers second-wave tools such as `sessions.reset_agent`, `reports.compare`, runtime health, automations, experiments, clone, and agent retry to later owner proposals. |
| DEP-02 | `docs/proposals/041-server-parity-harness-golden-runs-and-behavioral-diff.md` and `docs/proposals/042-local-daemon-lifecycle-supervision-and-packaging.md` | Dependency proposals | P041 and P042 remain draft-level prerequisites for parity proof and daemon lifecycle; P031 names them as cutover gates but does not bind concrete gate names or evidence artifacts. |
| DEP-03 | `docs/proposals/043-query-projections-and-client-consumption-contract.md` | Dependency proposal | P043 states GraphQL is the read path and says each UI surface needs projection owner, query contract, freshness, and forbidden client inferences, but it is itself high-level and does not fill P031's concrete surface matrix. |
| BASE-01 | `.review-baselines/current-system-baseline.md` | Baseline | Current review posture should prefer stable references and refresh only affected slices. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Baseline | Current system includes macOS operator shell, live ACP execution, MCP policy truth, canonical execution truth/recovery, GraphQL/MCP readback, run surface IA, repo-backed delivery, and Steward analysis. |
| BASE-03 | `docs/reference/operator-experience.md` | Reference | Operator shell requires truthful actions, runtime provenance, recovery, artifact inspection, reports, and comparison. |
| BASE-04 | `docs/reference/run-surface-information-architecture-and-artifact-hierarchy.md` | Reference | Runs Home and Idea run progress have distinct pane contracts; run artifact hierarchy is a browsing projection, not a second truth lane. |
| BASE-05 | `docs/reference/execution-truth-and-recovery.md` | Reference | Recovery/report readers must prefer persisted execution truth and stage-owned recovery evidence instead of heuristic reconstruction. |
| CONFIG-01 | `.codex/review-router.yaml` | Repo-local routing overlay | MacOS/Apple UI paths, Rust GraphQL/MCP paths, and execution-truth seams route to Apple UX/architecture, API contract, rollout, and execution-truth reviewers when evidenced. |
| CONFIG-02 | `.codex/reviewers/chainworks-execution-truth.yaml` | Repo-local reviewer plugin | Projection truth, MCP truth, command journal, Run/stage/agent/artifact/recovery truth trigger the repo-local execution-truth reviewer. |
| CODE-01 | `Chainworks Forge/Views/RunsHomeView.swift:9-24`, `:71-190` | Current Swift UI slice | Runs Home is still SwiftData-backed through `@Query`, `modelContext`, local selection state, and local sheet routing for recovery, comparison, and report view. |
| CODE-02 | `Chainworks Forge/Views/RecoverySheet.swift:239-322` | Current Swift command slice | Recovery actions directly invoke `RecoveryCoordinator`, `RunPlanCompiler`, and `ExecutionService` instead of an MCP command boundary. |
| CODE-03 | `Chainworks Forge/Views/BlockedRunRecoveryView.swift:752-825` | Current Swift command slice | Blocked recovery repeats direct local retry/resume/reset action ownership through Swift services. |
| CODE-04 | `control-plane/crates/db/src/repos/projections.rs:12-35`, `:56-104`, `:156-200`, `:202-252` | Current Rust projection slice | Run projection rows and stage summary rows exist; run list/detail projection functions exist; stage projection function exists. |
| CODE-05 | `control-plane/crates/graphql-server/src/schema.rs:67-105` | Current GraphQL query slice | `runs` reads use projection rows, but `stages(runID:)` reads canonical stage rows directly through `db::repos::stages::list_by_run`, not `projections::list_stages_projection`. |
| CODE-06 | `control-plane/crates/graphql-server/src/types/stage.rs:6-26`, `:93-136` | Current GraphQL type slice | `GqlStageExecution` has optional projection-only fields, but the canonical `StageExecution` conversion sets them to `None`; only `StageSummaryRow` populates them. |
| CODE-07 | `control-plane/crates/mcp-server/src/tools/mod.rs:11-46`, `control-plane/crates/domain/src/capabilities.rs:3-18` | Current MCP capability/tool registry | Current first-wave MCP tools are ideas create/list, runs start/list/get/cancel, approvals list/resolve, stages retry, reports get, and Steward tools. No reset-session, compare, runtime-health, experiment, clone, or agent retry tool IDs exist. |
| CODE-08 | `control-plane/crates/mcp-server/src/tools/runs.rs:11-175`, `approvals.rs:11-93`, `stages.rs:10-54`, `reports.rs:11-70` | Current MCP tool implementations | Command tools exist for runs start/cancel, approvals resolve, stages retry, and Steward analysis; read/direct tools exist for list/get/report surfaces. |
| CODE-09 | `control-plane/crates/graphql-server/src/server.rs:58-85`, `schema.rs:530-575` | Current subscription/auth slice | GraphQL WS auth and `run_status_changed`/stage subscription structure exists, but P031 does not state freshness/offline/staleness semantics for the thin client. |
| CODE-10 | `scripts/test-gate.sh`, `docs/reference/test-gates.md` targeted scan | Gate registry | No `proposal-031|p031` gate was found. Existing relevant gates are `ui-smoke`, `proposal-029-mcp`, and other proposal-specific control-plane/UI gates. |
| HIST-01 | `docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.review/*` | Prior artifacts | Existing review artifacts were absent or empty before this pass. |

## 3. Fingerprint

| Tag type | Tag | Evidence IDs | Rationale |
|---|---|---|---|
| Stack | `macos` | DOC-01, CODE-01, CODE-02, CODE-03 | P031 changes the macOS operator client boundary and its primary views/actions. |
| Stack | `apple-client` | DOC-01, CODE-01, CODE-02, CODE-03 | SwiftUI/SwiftData local-state ownership is the migration target. |
| Stack | `rust-backend` | DOC-01, CODE-04, CODE-05, CODE-07, CODE-08 | P031 depends on Rust GraphQL projections and MCP command tools. |
| Stack | `shared-api` | DOC-01, DEP-01, DEP-03, CODE-05, CODE-07 | The cutover relies on stable GraphQL and MCP contracts. |
| Stack | `cross-stack` | DOC-01, BASE-03, BASE-05 | It moves user-visible truth from the Apple client to server projections/control commands. |
| Surface | `ui` | DOC-01, CODE-01 | Primary user-visible screens are in scope. |
| Surface | `ux` | DOC-01, BASE-03 | Operator confidence, debuggability, and recovery clarity are explicit goals. |
| Surface | `navigation` | DOC-01, BASE-04, CODE-01 | Runs home, run detail, stage detail, approvals, artifacts, reports, comparison, and health surfaces are all named. |
| Surface | `architecture` | DOC-01, CODE-01, CODE-02, CODE-03 | Source-of-truth and ownership boundaries change. |
| Surface | `state-management` | DOC-01, CODE-01 | UI-local state must become disposable while SwiftData currently backs run views. |
| Surface | `api-contract` | DOC-01, DEP-01, DEP-03, CODE-05, CODE-07 | GraphQL read and MCP command contracts are central. |
| Surface | `rollout` | DOC-01, DEP-02, CODE-10 | P031 is a phased user-visible cutover with prerequisites but no concrete gate. |
| Risk | `backward-compatibility` | DOC-01, CODE-01, CODE-10 | Existing local SwiftData/client-owned flows need coexistence or cutover semantics. |
| Risk | `data-loss` | BASE-05, CODE-02, CODE-03 | Recovery/retry/reset actions affect durable execution truth if incorrectly routed. |
| Risk | `multi-service-coordination` | DOC-01, DEP-01, DEP-03 | Thin UI spans macOS client, local daemon, GraphQL, MCP, and command journal. |
| Risk | `operability-sensitive` | DOC-01, BASE-03, CODE-09 | Operator shell must remain debuggable during daemon/projection updates and failures. |
| Risk | `user-trust` | DOC-01, BASE-03 | The user must trust the UI no longer invents workflow truth while still showing safe actions. |

## 4. Reviewer routing

Selected reviewers: 5 of 5 hard cap. The hard cap is justified because P031 is a cross-stack user-visible cutover: macOS UX, Apple client state architecture, GraphQL/MCP API contracts, rollout/proof, and repo-specific execution truth all materially differ.

| Reviewer | Selected because | Evidence IDs |
|---|---|---|
| `apple_ux_reviewer` | User-visible cutover must preserve operator confidence, recovery clarity, and debuggability while making the UI thinner. | DOC-01, BASE-03, BASE-04 |
| `apple_arch_reviewer` | SwiftUI/SwiftData source-of-truth, navigation, state disposal, and local service teardown are central. | DOC-01, CODE-01, CODE-02, CODE-03 |
| `api_contract_reviewer` | P031 relies on GraphQL projections and MCP commands, but their exact surface contracts are not enumerated in the draft. | DOC-01, DEP-01, DEP-03, CODE-04 through CODE-09 |
| `observability_rollout_reviewer` | The migration is phased, prerequisite-gated, and user-visible; it needs cutover/rollback/proof ownership. | DOC-01, DEP-02, CODE-10 |
| `chainworks_execution_truth_reviewer` | The proposal changes projection truth, MCP command truth, recovery/action ownership, and the client/server execution-truth boundary. | CONFIG-02, DOC-01, BASE-05, CODE-02 through CODE-08 |

Rejected close alternatives:

| Reviewer | Rejected because | Evidence IDs |
|---|---|---|
| `macos_ui_reviewer` | P031 does not propose concrete visual layout/control changes; UX and architecture risks dominate. | DOC-01 |
| `rust_arch_reviewer` | Rust implementation details are represented through API/projection contract findings; no new Rust module design is specified. | DOC-01, CODE-04 through CODE-08 |
| `rust_reliability_reviewer` | Reliability concerns are execution-truth/cutover concerns here; no Rust async queue/retry implementation change is specified in P031. | DOC-01, CONFIG-02 |
| `rust_security_reviewer` | P031 does not change auth policy itself; P029 owns northbound auth/capability. | DEP-01 |
| `product_reviewer` | Product metrics/experiments are not central to the proposal; product review remains opt-in. | DOC-01, CONFIG-01 |
| Go reviewers | No Go service/module seam is implicated. | CONFIG-01 |

## 5. Proposal completeness matrix

| Required lane for P031 | Status | Evidence IDs | Gap |
|---|---|---|---|
| Surface-by-surface GraphQL read contract | Incomplete | DOC-01, DEP-03, CODE-04, CODE-05, CODE-06 | P031 names surfaces but does not bind each to a query/projection/subscription contract; current stage query does not use projection rows. |
| MCP command/action matrix | Incomplete | DOC-01, DEP-01, CODE-07, CODE-08 | P031 lists actions beyond current first-wave MCP tools without defining or deferring missing tools. |
| Swift local-state teardown map | Incomplete | DOC-01, CODE-01, CODE-02, CODE-03 | P031 says remove local state/orchestration, but does not inventory current SwiftData/service owners or dual-read/cutover sequencing. |
| Daemon lifecycle/offline UI behavior | Partial | DOC-01, DEP-02, CODE-09 | P031 depends on dependable lifecycle but does not define client stale/offline/error behavior. |
| Cutover/rollback/proof gate | Incomplete | DOC-01, DEP-02, CODE-10 | P031 has phases and prerequisites but no canonical `proposal-031` gate or hold/rollback criteria. |
| Operator usability/debuggability | Partial | DOC-01, BASE-03, BASE-04 | Goals are stated, but concrete summaries, action states, and projection freshness are not specified. |

## 6. Findings

### P1: Stage/read projection contract is not executable

Evidence: `DOC-01`, `DEP-03`, `CODE-04`, `CODE-05`, `CODE-06`.

P031 says views render from GraphQL projections/queries and AC-1 requires run/stage/approval/artifact/report state from service projections. The proposal does not define a per-surface query/projection matrix. Current GraphQL already shows why that matters: `runs` uses projection rows, but `stages(runID:)` reads canonical stage rows and therefore leaves projection-only flags (`hasArtifacts`, `hasPendingApproval`, `hasValidationFailure`) empty. A thin UI can still reconstruct state locally unless the proposal pins which query and projection owns each surface.

Required fix: Add a read contract table for Runs home, Run detail, Stage detail, Approval inbox, Artifact viewer, Report viewer, Experiment comparison, and Runtime health. For each surface, name the GraphQL query/subscription, projection/repo owner, required fields, freshness/staleness behavior, and prohibited client inference.

Acceptance criteria: stage detail and run progress consume server-owned projection rows or a consciously documented non-projection query; all projection-derived UI decisions have non-null server-owned fields; tests compare GraphQL readbacks against projection rows for each primary surface.

### P1: MCP action surface names commands that do not exist or are explicitly deferred

Evidence: `DOC-01`, `DEP-01`, `CODE-07`, `CODE-08`.

P031 says all operator mutations flow through MCP and lists start, approve/reject, retry, reset session, cancel, compare, and run experiments. Current MCP first-wave tools cover only runs start/list/get/cancel, approvals list/resolve, stages retry, reports get, ideas create/list, and Steward tools. P029 explicitly defers reset-session, compare, experiments, clone, runtime health, and agent/session second-wave tools. Without a P031 action matrix, implementers can either leave UI actions on direct Swift services or invent unowned MCP tools.

Required fix: Add an operator action matrix mapping each UI action to `MCP tool`, `CommandHandler` command/direct-read owner, `journal_id`/audit behavior, capability ID, and status (`in P031`, `deferred`, or `removed from UI`). Explicitly defer actions whose MCP owners are not part of P031.

Acceptance criteria: every mutation reachable from the named primary views either routes through a registered MCP command tool with capability and journal behavior or is visibly disabled/deferred with a proposal owner.

### P1: SwiftData/local service teardown lacks an ownership and cutover map

Evidence: `DOC-01`, `CODE-01`, `CODE-02`, `CODE-03`, `BASE-05`.

The migration strategy says to remove obsolete local state stores and client-owned orchestration, but it does not identify the Swift owners that must move. Current `RunsHomeView` is `@Query`/SwiftData-backed and routes recovery, comparison, and report sheets locally. `RecoverySheet` and `BlockedRunRecoveryView` directly call `RecoveryCoordinator`, `RunPlanCompiler`, and `ExecutionService` for retry/resume/reset/clone. If P031 only rewrites some views over GraphQL while these services continue mutating SwiftData, the client remains a second workflow truth lane.

Required fix: Add a Swift cutover inventory covering models, views, services, and preview/test seeds. For each owner, specify whether it is deleted, retained as UI cache/presentation state, replaced by GraphQL read model, replaced by MCP command, or intentionally deferred. Include dual-read and rollback behavior for the transition window.

Acceptance criteria: no operator mutation path remains client-owned unless explicitly deferred; retained SwiftData state is presentation/cache-only and cannot decide run/stage/approval/recovery truth; tests or static guardrails fail when old direct service paths are used from P031-owned screens.

### P2: Cutover prerequisites are named but not enforceable

Evidence: `DOC-01`, `DEP-02`, `DEP-03`, `CODE-10`.

P031 correctly says cutover must wait for parity proof, daemon lifecycle dependability, and an explicit query/projection contract. However P041, P042, and P043 are draft-level documents here, and no `proposal-031|p031` gate is registered. The acceptance criteria say the product remains usable/debuggable, but there is no canonical gate tying parity, daemon health, GraphQL/MCP contracts, and UI smoke together.

Required fix: Define a P031 gate or explicit gate bundle. At minimum, name the prerequisite evidence artifacts from P041/P042/P043, the GraphQL/MCP contract checks, the UI smoke path, and hold/rollback criteria for staged cutover.

Acceptance criteria: `./scripts/test-gate.sh proposal-031` exists or P031 names a canonical manual bundle; the gate proves no client-owned mutation for P031-owned screens, projection parity for read surfaces, daemon unavailable/degraded UI states, and operator smoke for the main cutover views.

## 7. Non-blocking observations

- Product review was not selected. P031 is strategic and user-visible, but it does not define launch metrics, experiments, or product decision checkpoints as central scope.
- MacOS UI review was not selected. The proposal names screens, but not concrete visual design, layout, or platform-specific component behavior.
