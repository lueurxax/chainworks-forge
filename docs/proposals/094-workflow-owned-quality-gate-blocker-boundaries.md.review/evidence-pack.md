# Proposal Evidence Pack

Proposal: `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md`  
Mode: `auto`  
Reviewed on: `2026-06-15`  
Reviewer router version: `proposal-review-router`  
Working tree note: `clean before writing this evidence pack`

## A. Repo-local proposal and document inventory

| Evidence ID | Source / path / artifact | Verified on | Confidence | Key fact | Risk if wrong | Relevance |
|---|---|---:|---|---|---|---|
| DOC-01 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:1` | 2026-06-15 | High | P094 is a draft/rewrite that makes workflow declarations the only transition authority and limits human approval to accept/reject comments. | Review would target the wrong behavior. | Primary proposal. |
| DOC-02 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:304` | 2026-06-15 | High | Runtime blocker-boundary assessment consumes audit, tests, completion receipts, active artifact contracts, side effects, worktree fingerprints, and execution ids. | Misses cross-stack durability scope. | Drives reviewer routing. |
| DOC-03 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:417` | 2026-06-15 | High | `QualityGateBoundaryEvaluator` is server-owned and emits `blocker_boundary_status_v1`, with lower-layer statuses before approval. | Server/client authority could be misread. | Architecture and contract scope. |
| DOC-04 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:549` | 2026-06-15 | High | Proposal examples route via workflow YAML conditions over `blocker_boundary_status` and approval decisions. | Undefined condition fields can fail closed. | Workflow contract scope. |
| DOC-05 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:817` | 2026-06-15 | High | Proposal adds new readback artifacts and GraphQL/MCP/report surfaces but no new GraphQL mutation. | Contract compatibility and storage ownership risk. | API/reports scope. |
| DOC-06 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:998` | 2026-06-15 | High | Proposal defines metrics and success as fewer repeated implementation-refine loops without more false closeouts. | Rollout proof could be under-specified. | Observability scope. |
| DOC-07 | `docs/reference/workflow-execution-engine.md:14` | 2026-06-15 | High | Governed macOS UI is a thin GraphQL read/approval client; approval resolution is the only governed mutation path. | Proposal could accidentally add UI control authority. | UI boundary. |
| DOC-08 | `docs/reference/workflow-execution-engine.md:251` | 2026-06-15 | High | Current transition evaluator supports canonical patterns such as `approval.granted == true`, `approval.rejected == true`, artifact-field comparisons, and fail-closed unknown expressions. | New fields/decision values must be explicit. | Workflow compatibility. |
| DOC-09 | `docs/reference/output-contracts-failure-evidence-and-recovery.md:235` | 2026-06-15 | High | Machine-consumed workflow artifacts are canonical artifact contracts; SQLite-owned generations choose active truth and export projections. | New P094 artifacts could become a second authority. | Artifact truth. |
| DOC-10 | `docs/reference/ui-action-boundary.md:8` | 2026-06-15 | High | SwiftUI may only use GraphQL reads/subscriptions plus `approveApproval` and `rejectApproval`; all other operations are MCP/outside UI. | P094 approvals must fit existing action boundary. | Approval/API scope. |
| DOC-11 | `docs/reference/rust-control-plane.md:121` | 2026-06-15 | High | GraphQL currently exposes read/subscription surfaces and only `approveApproval` / `rejectApproval` mutations; MCP owns non-approval commands. | Proposal readback must be additive and compatible. | Rust/API scope. |
| DOC-12 | `docs/proposals/095-two-phase-agent-invocation-and-deferred-output-settlement.md:335` | 2026-06-15 | High | P095 explicitly states missing output routes through output collection/repair before P094 blocker-boundary classification. | P094 must not preempt output settlement. | Dependency fit. |
| DOC-13 | `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md:197` | 2026-06-15 | High | P088 completion receipts record output-settlement/freshness but do not grant transition authority. | P094 must consume, not override, freshness truth. | Dependency fit. |
| DOC-14 | `scripts/test-gate.sh` and `docs/reference/test-gates.md` search for `proposal-094|p094` | 2026-06-15 | High | No current retained P094 gate is registered in the repo. | Proposal proof plan is future-state only. | Proof-gate mapping. |

## B. Reusable baseline inputs

| Evidence ID | Artifact / slice | Status | Covered surfaces | Verified on | Confidence | Freshness notes | Relevance |
|---|---|---|---|---:|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Partially refreshed | macOS operator shell, Rust daemon, workflow/catalog, provider families, reference-doc preference | 2026-06-15 | Medium | Baseline still says "live Goose-backed" while repo instructions/reference now emphasize ACP and Rust parity; affected slices were refreshed from reference docs/code. | Prevents over-mapping unrelated repo areas. |

## C. Prior proposal artifacts consumed

| Evidence ID | Artifact | Status | Key fact | Relevance |
|---|---|---|---|---|
| ART-01 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md.review/integration-context.md` | Missing | No prior integration-context artifact exists for this proposal. | Local code/docs were narrowly refreshed instead. |
| ART-02 | Prior P094 evidence/research packs | Missing | No prior review artifacts found beside this proposal. | This is a fresh review pass. |

## D. Current repo / code-path map

| Evidence ID | Surface / entry point | File / module / manifest | Layer | Key fact | Risk if wrong | Relevance |
|---|---|---|---|---|---|---|
| MAP-01 | Approval domain | `control-plane/crates/domain/src/approval.rs:6` | Rust domain | `ApprovalDecision` is `pending/requested/granted/rejected/expired`, not `accept/reject`. | New P094 payload values may not fit storage/API. | Approval contract. |
| MAP-02 | Approval transition evaluation | `control-plane/crates/engine/src/orchestrator.rs:5931` | Rust engine | Workflow conditions check `approval.granted == true` and `approval.rejected == true`. | `approval.decision == "accept"` needs explicit support or mapping. | Transition compatibility. |
| MAP-03 | MCP approval resolve | `control-plane/crates/mcp-server/src/tools/approvals.rs:70` | MCP API | `approvals.resolve` accepts `granted/rejected` for stage approvals, plus mediation decisions. | `accept` is not an accepted stage approval decision today. | API compatibility. |
| MAP-04 | GraphQL approval mutations | `control-plane/crates/graphql-server/src/schema.rs:4334` | GraphQL API | GraphQL exposes `approveApproval` and `rejectApproval`; command handling maps Approved/Rejected to Granted/Rejected. | Boundary approval must map to existing mutation semantics. | UI/API compatibility. |
| MAP-05 | UI action routing registry | `control-plane/crates/domain/src/operator_action_routing.rs:55` | Rust domain | UI operator allowlist has exactly `approveApproval` and `rejectApproval`; non-approval actions are forbidden. | P094 cannot add ad hoc UI actions. | UI boundary. |
| MAP-06 | Canonical artifact contract read | `control-plane/crates/db/src/repos/artifact_contracts.rs:898` | DB repo | Active contract fields read from `active_artifact_contracts` or closeout gate generations; uncontrolled aliases are not transition truth. | New P094 status artifacts must be declared or transitions fail closed. | Artifact truth. |
| MAP-07 | Run-state projection export | `control-plane/crates/db/src/repos/artifact_contracts.rs:1073` and `:1437` | DB repo | Projection rebuild exports `active-artifact-index.v1` and `run-state-projection.v1` with `owner: sqlite`. | Readback divergence if P094 bypasses projections. | Readback parity. |
| MAP-08 | Rust control-plane crate owner | `docs/reference/rust-control-plane.md:82` | Reference | `engine`, `db`, `graphql-server`, and `mcp-server` own the affected runtime, storage, and boundary layers. | Wrong implementation owner. | Reviewer routing. |

## E. Fingerprint summary

| Tag type | Tag | Evidence IDs | Reason |
|---|---|---|---|
| Stack | `rust-backend` | DOC-02, DOC-03, DOC-11, MAP-08 | Server-owned evaluator, Rust daemon readback, DB/engine/API changes. |
| Stack | `shared-api` | DOC-05, DOC-10, DOC-11, MAP-03, MAP-04 | GraphQL, MCP, report, workflow condition, and approval payload contracts change. |
| Stack | `cross-stack` | DOC-07, DOC-10, DOC-11, MAP-05 | macOS UI reads/approves while Rust owns authoritative state and MCP owns commands. |
| Surface | `architecture` | DOC-01, DOC-03, MAP-08 | New server-owned evaluator and routing statuses. |
| Surface | `state-management` | DOC-04, DOC-08, MAP-02 | Workflow state transitions consume new status fields. |
| Surface | `background-work` | DOC-02, DOC-12 | Runtime loops, retries, output settlement, and recovery paths are affected. |
| Surface | `api-contract` | DOC-05, MAP-01, MAP-03, MAP-04 | New artifact payloads and approval/readback fields. |
| Surface | `persistence` | DOC-02, DOC-09, MAP-06, MAP-07 | Active artifact contracts, generations, and projections are required. |
| Surface | `telemetry` | DOC-06 | Metrics are a required part of the proposal. |
| Surface | `rollout` | DOC-06, DOC-14 | Phased rollout and retained proof gate are proposed. |
| Surface | `security-boundary` | DOC-10, MAP-05 | UI action boundary and approval-only mutation surface must be preserved. |
| Risk | `backward-compatibility` | MAP-01, MAP-02, MAP-03, MAP-04 | Approval decision vocabulary and workflow condition fields may break existing clients. |
| Risk | `idempotency` | DOC-02, DOC-12, DOC-13 | Repeated blocker signatures, freshness, and output settlement must be replay-safe. |
| Risk | `data-loss` | DOC-09, MAP-06 | Wrong active artifact truth could mask missing outputs or stale evidence. |
| Risk | `availability-sensitive` | DOC-01, DOC-06 | Goal is preventing endless loops without false closeout. |
| Risk | `operability-sensitive` | DOC-06, DOC-14 | Metrics and gates are needed to diagnose rollout. |
| Risk | `multi-service-coordination` | DOC-05, DOC-10, DOC-11 | Swift UI, GraphQL, MCP, reports, and Rust engine must agree. |
| Risk | `user-trust` | DOC-01, DOC-10 | Human boundary approval must not imply waiver or route authority. |

## F. Routing decision

Selected reviewers:

| Reviewer ID | Mode | Evidence IDs | Why selected | Repo-local agent used? |
|---|---|---|---|---|
| `chainworks_execution_truth_reviewer` | architecture-only | DOC-01, DOC-02, DOC-03, DOC-09, MAP-06, MAP-07 | Proposal changes durable run/stage/approval/artifact/recovery truth. | Yes: `.codex/agents/chainworks_execution_truth_reviewer.md` |
| `rust_arch_reviewer` | architecture-only | DOC-03, DOC-11, MAP-08 | Rust engine/db/server ownership is directly affected. | Yes: `.codex/agents/rust_arch_reviewer.md` |
| `rust_reliability_reviewer` | reliability-only | DOC-02, DOC-06, DOC-12, DOC-13 | Retry/no-progress/output-settlement reliability is central. | Yes: `.codex/agents/rust_reliability_reviewer.md` |
| `api_contract_reviewer` | api-contract-only | DOC-04, DOC-05, MAP-01, MAP-02, MAP-03, MAP-04 | Workflow, approval, GraphQL/MCP/report payload contracts change. | Yes: `.codex/agents/api_contract_reviewer.md` |
| `observability_rollout_reviewer` | observability-rollout-only | DOC-06, DOC-14 | Proposal includes rollout phases, metrics, and retained gates. | Yes: `.codex/agents/observability_rollout_reviewer.md` |

Rejected close alternatives:

| Reviewer ID | Evidence IDs | Why not selected |
|---|---|---|
| `apple_arch_reviewer` | DOC-07, DOC-10, MAP-05 | macOS surface is intentionally passive/read-only approval; API/action boundary reviewers cover it without adding a sixth reviewer. |
| `macos_ui_reviewer` | DOC-10 | No new UI affordance, layout, navigation, or window behavior is proposed. |
| `rust_security_reviewer` | DOC-10, MAP-05 | Security-boundary evidence is approval/action allowlist compatibility, not new auth/token/permission logic. |
| `product_reviewer` | DOC-06 | Metrics exist, but product decision/experiment/adoption risk is not central and product review is opt-in. |

Routing cap status: `5 selected; target 2-4; hard cap 5`

## G. State and failure coverage matrix

| State / failure class | Proposal coverage | Evidence IDs | Gap / risk | Reviewer owner |
|---|---|---|---|---|
| Entry / setup | Partial | DOC-02, DOC-03 | Evaluator owner is named; storage/schema registration is not pinned. | rust_arch_reviewer |
| Happy path | Partial | DOC-04 | Status fields route to approval/closeout, but schema fields are inconsistent. | api_contract_reviewer |
| Loading / in-flight | Partial | DOC-12 | Output collection before P094 is acknowledged through P095 relationship. | rust_reliability_reviewer |
| Timeout / cancellation | Partial | DOC-12, DOC-13 | Lower-layer recovery is named but exact status enum/repair path is incomplete. | rust_reliability_reviewer |
| Retry / replay / idempotency | Partial | DOC-06, DOC-13 | No-progress threshold and measurable progress source are open. | rust_reliability_reviewer |
| Persistence / migration | Missing | DOC-05, DOC-09, MAP-06, MAP-07 | No migration/schema/active-contract registration plan for new artifacts. | chainworks_execution_truth_reviewer |
| Auth / permission failure | Ready | DOC-10, MAP-05 | Proposal preserves no new UI actions; approval mapping still needs compatibility. | api_contract_reviewer |
| Dependency failure | Partial | DOC-12, DOC-13 | P088/P095 dependencies are acknowledged. | rust_reliability_reviewer |
| Rollback / recovery | Partial | DOC-06, DOC-14 | Phases exist but rollback/kill switch/downgrade behavior is sparse. | observability_rollout_reviewer |
| Observability / support | Partial | DOC-06 | Metrics are named, but decision thresholds and alert/readback ownership are not. | observability_rollout_reviewer |

## H. Proposal completeness matrix

| Dimension | Status | Evidence IDs | Notes |
|---|---|---|---|
| Problem and target user | Ready | DOC-01 | Clear operator pain and desired loop behavior. |
| Scope and non-goals | Ready | DOC-01 | Strong no-waiver/no-new-actions boundary. |
| Current-system fit | Partial | DOC-07, DOC-08, DOC-09, DOC-10, DOC-11 | Fits intent, but approval vocabulary and storage model need alignment. |
| Data / state model | Partial | DOC-02, DOC-03, DOC-05 | New artifacts listed, but canonical schema/persistence/enum details are incomplete. |
| API / contract compatibility | Partial | MAP-01, MAP-02, MAP-03, MAP-04 | Accept/reject needs explicit compatibility mapping. |
| Runtime / concurrency semantics | Partial | DOC-02, DOC-12, DOC-13 | Lower-layer preconditions are correct; no-progress determinism remains open. |
| Failure handling | Partial | DOC-03, DOC-06 | Many failure classes named; exact status enum and field derivations missing. |
| Security / privacy / auth | Partial | DOC-10, MAP-05 | UI boundary preserved; approval owner question remains open. |
| Migration / rollout / rollback | Partial | DOC-06, DOC-14 | Rollout phases and metrics exist; rollback/registration details need more. |
| Observability / diagnostics | Partial | DOC-06 | Metrics are named; thresholds/dashboards/readback not pinned. |
| Test / proof gate | Partial | DOC-14 | Proposal names gate and fixture cases; current repo does not yet register it. |
| Product metrics / decision checkpoint | Partial | DOC-06 | Success/guardrail exist, but decision checkpoint is not explicit. |

## I. Evidence gaps and fallback decisions

| Gap ID | Missing evidence | Blocks routing or finding? | Next local artifact or file to inspect | Integration-context refresh needed? |
|---|---|---|---|---|
| GAP-01 | No proposal-specific integration context exists. | No | Create only if a future review needs a complete host-system map. | No |
| GAP-02 | No P094 implementation/gate exists in current repo. | No | `scripts/test-gate.sh`, `docs/reference/test-gates.md`, future P094 tests. | No |

## J. Research triggers

| Trigger ID | Local evidence IDs | Question local evidence cannot settle | Required source type | Why it matters |
|---|---|---|---|---|
| RES-01 | N/A | N/A | N/A | No external research required for proposal-readiness review. |

## K. Findings ledger

| Finding ID | Reviewer | Severity | Evidence IDs | File / lines | Summary | Confidence |
|---|---|---|---|---|---|---|
| P1-API-001 | api_contract_reviewer | P1 | DOC-01, DOC-04, MAP-01, MAP-02, MAP-03, MAP-04 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:123` | Approval decision vocabulary conflicts with current granted/rejected approval contract. | 0.92 |
| P1-EXEC-001 | chainworks_execution_truth_reviewer | P1 | DOC-03, DOC-04, DOC-08, MAP-06 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:549` | Workflow examples reference status fields/booleans that are not defined as a complete canonical schema. | 0.88 |
| P1-DATA-001 | rust_arch_reviewer | P1 | DOC-02, DOC-05, DOC-09, MAP-06, MAP-07 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:817` | New machine-consumed P094 artifacts are not tied to canonical artifact-contract storage/projection ownership. | 0.86 |
| P2-REL-001 | rust_reliability_reviewer | P2 | DOC-06, DOC-12, DOC-13 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:725` | No-progress rule leaves the repeat threshold and measurable-progress source unresolved. | 0.80 |
| P2-OPS-001 | observability_rollout_reviewer | P2 | DOC-06, DOC-14 | `docs/proposals/094-workflow-owned-quality-gate-blocker-boundaries.md:998` | Metrics are listed, but rollout decision thresholds and support ownership are not explicit. | 0.74 |
