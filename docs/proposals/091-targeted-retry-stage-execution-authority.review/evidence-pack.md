# Proposal Evidence Pack

Proposal: `docs/proposals/091-targeted-retry-stage-execution-authority.md`  
Mode: `auto`  
Reviewed on: `2026-05-14`  
Reviewer router version: `proposal-review-router`  
Reviewed proposal md5: `5b631c727f80f297cb7034dbf42dfcf6`  
Repository revision: `225bac4e47b135d92c4fe2de243dd13c4647be19`  
Branch: `main`  
Working tree note: `dirty before review; unrelated Swift support change plus proposal/evidence/test-gate artifacts were present and not reverted`

## A. Repo-local proposal and document inventory

| Evidence ID | Source / path / artifact | Verified on | Confidence | Key fact | Relevance |
|---|---|---:|---|---|---|
| DOC-01 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:13` | 2026-05-14 | High | P091 targets a `RetryStage` lineage defect where a new concrete `stage_execution` can be stranded while orchestration continues by logical `stage_id`. | Problem framing. |
| DOC-02 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:29` | 2026-05-14 | High | P091 preserves P086 evidence and requires fixture `docs/evidence/091/targeted-retry-authority/p086-orphaned-retry-readback.fixture.json`. | Evidence baseline. |
| DOC-03 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:130` | 2026-05-14 | High | Decision now distinguishes full-stage retry as `AdvanceRun`-first and targeted-agent retry as `InvokeAgent`-first while preserving durable retry authority. | Entry lifecycle. |
| DOC-04 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:144` | 2026-05-14 | High | Proposed `retry_stage_execution_authorities` table includes `retry_stage_execution_authorities_one_active` partial unique index and transaction-level race handling. | Persistence / concurrency invariant. |
| DOC-05 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:214` | 2026-05-14 | High | Target authority must propagate through full-stage retry, targeted-agent retry, post-invoke completion/failure, and abandoned-work recovery requeues. | Work queue propagation. |
| DOC-06 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:234` | 2026-05-14 | High | Work-item repository helpers must preserve typed target payloads and avoid collapsing targeted advances to run scope. | Repository semantics. |
| DOC-07 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:248` | 2026-05-14 | High | Targeted mode verifies exact target stage execution, active authority, event truth, and summary selection before falling back to legacy state-level behavior. | Orchestrator authority. |
| DOC-08 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:296` | 2026-05-14 | High | Historical orphan repair now settles as `status = skipped`, records `terminal_reason = stale_retry_recovered`, and creates a non-active recovered authority provenance row. | Recovery truth. |
| DOC-09 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:328` | 2026-05-14 | High | Recovery exclusions cover approvals, side effects, transition cursors, recovery/backpressure holds, queued work, provider capacity, retry-after/quota, and wait-oriented snapshots. | Recovery safety. |
| DOC-10 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:359` | 2026-05-14 | High | P091 defines `AdvanceRunPayloadV1` and fail-closed parse/error semantics for malformed or wrong-target targeted payloads. | API contract. |
| DOC-11 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:411` | 2026-05-14 | High | Authority lifecycle now separately covers full-stage retry, targeted-agent retry, later retry supersession, and historical orphan recovery. | Authority lifecycle. |
| DOC-12 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:445` | 2026-05-14 | High | Readback adds current `retryAuthority` plus `retryAuthorityHistory` for terminalized, superseded, and recovered authorities. | Shared API/readback. |
| DOC-13 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:494` | 2026-05-14 | High | Acceptance criteria now include full/targeted retry propagation, projection rebuild, history readback, recovery exclusions, duplicate-active prevention, and `proposal-091` gate. | Proof plan. |
| DOC-14 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:648` | 2026-05-14 | Medium | Rollout is four steps: schema/payload/readback, targeted emissions, projection/readback, then startup orphan recovery. | Rollout sequencing. |
| DOC-15 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:689` | 2026-05-14 | High | Open questions are closed for implementation readiness. | Proposal completeness. |

## B. Reusable baseline inputs

| Evidence ID | Artifact / slice | Status | Verified on | Confidence | Key fact | Relevance |
|---|---|---|---:|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Stale | 2026-05-14 | High | Reusable baseline still mentions live Goose-backed execution and older provider wording. | Consumed but not treated as current truth. |
| BASE-02 | `docs/reference/current-system-baseline.md:24` | Reused | 2026-05-14 | High | Current reference baseline includes GraphQL thin UI, capacity scheduling, recovery, SQLite write serialization, side-effect ledger, and live ACP execution. | Current-system context. |
| REF-01 | `docs/reference/execution-truth-and-recovery.md:67` | Reused | 2026-05-14 | High | Persisted execution-truth columns outrank diagnostic envelopes/receipts; durable truth must not be reconstructed from transient evidence. | Durable authority/readback precedence. |
| REF-02 | `docs/reference/query-projections-and-client-consumption-contract.md:70` | Reused | 2026-05-14 | High | Clients must not infer retry, recovery, terminality, or projection freshness locally; server readback owns that truth. | GraphQL/MCP/readback contract. |
| REF-03 | `docs/reference/rust-control-plane.md:203` | Reused | 2026-05-14 | High | Retry, targeted retry, cancellation, scheduler advancement, and startup recovery fail closed when unresolved side effects exist. | Recovery exclusions. |

## C. Prior proposal artifacts and evidence consumed

| Evidence ID | Artifact | Status | Key fact | Relevance |
|---|---|---|---|---|
| ART-01 | Prior `docs/proposals/091-targeted-retry-stage-execution-authority.review/evidence-pack.md` | Superseded | Prior pack reviewed md5 `133e6fbb7682c228aba10a182e490707`; current md5 is `5b631c727f80f297cb7034dbf42dfcf6`. | Proposal changed; fresh review required by md5 guard. |
| ART-02 | `docs/evidence/091/targeted-retry-authority/evidence-index.json` | Reused | Evidence index declares readiness inventory schema, P086 fixture SHA-256, required contract terms, and `implementation_gate_upgrade_required = true`. | Evidence inventory and gate status. |
| ART-03 | `docs/evidence/091/targeted-retry-authority/p086-orphaned-retry-readback.fixture.json` | Reused | Fixture records pending retry execution, settled sibling, zero live work items, zero active agent executions, and blocked truth preserving the orphan. | Historical regression shape. |
| ART-04 | `./scripts/test-gate.sh proposal-091` | Passed on 2026-05-14 | Gate output: `proposal-091 evidence inventory validation passed` and `Proposal 091 gate passed`. | Readiness inventory validation. |

## D. Current repo / code-path map

| Evidence ID | Surface / entry point | File / module / manifest | Layer | Key fact | Relevance |
|---|---|---|---|---|---|
| MAP-01 | Full-stage `RetryStage` | `control-plane/crates/engine/src/command_handler.rs:2336`, `:2658` | Engine command | Current full-stage retry creates a new pending `StageExecution` and enqueues `AdvanceRun` with only `run_id` and `stage_id`. | Root defect and target payload delta. |
| MAP-02 | Fallback full-stage retry helper | `control-plane/crates/engine/src/command_handler.rs:4050`, `:4118` | Engine command | Fallback full-stage retry creates another pending retry stage and follows the same legacy work-item lineage. | Secondary retry path. |
| MAP-03 | Targeted-agent retry | `control-plane/crates/engine/src/command_handler.rs:4399`, `:4575` | Engine command | Targeted-agent retry creates a running `StageExecution` and enqueues `InvokeAgent`, not initial `AdvanceRun`. | Entry lifecycle. |
| MAP-04 | Current `AdvanceRun` executor | `control-plane/crates/engine/src/executor.rs:3724` | Work queue executor | `AdvanceRun` processing extracts only `run_id` and calls `orchestrator.advance_run(run_id)`. | Typed payload/API change. |
| MAP-05 | Current stage selection | `control-plane/crates/engine/src/orchestrator.rs:204` | Orchestrator | Workflow advancement lists all run stages and uses the last stage matching logical current state. | Same-state sibling risk. |
| MAP-06 | Post-invoke follow-up advances | `control-plane/crates/db/src/repos/work_items.rs:1530`, `:1639` | Work queue repo | Completion/failure enqueue `AdvanceRun` payloads with run/source invoke id but no target or retry authority; row `stage_id` is `NULL`. | Target propagation. |
| MAP-07 | Run/stage-scoped requeue/cancel helpers | `control-plane/crates/db/src/repos/work_items.rs:638`, `:1166`, `:1819` | Work queue repo | Existing helpers operate by `run_id` or `stage_id`, not parsed target authority. | Target-aware repository work. |
| MAP-08 | Stage projection schema | `control-plane/crates/db/migrations/002_projections.sql:67`, `control-plane/crates/db/src/repos/projections.rs:110`, `:275` | DB projection | `stage_summaries` and `StageSummaryRow` currently have no retry-authority or terminal-reason fields. | Readback migration. |
| MAP-09 | GraphQL stage readback | `control-plane/crates/graphql-server/src/types/stage.rs:108`, `:831` | GraphQL API | `GqlStageExecution` exposes status, settlement kind, projection flags, validation/evidence/recovery JSON, but no retry authority or terminal reason. | API/readback compatibility. |
| MAP-10 | Stage model / settlement | `control-plane/crates/domain/src/stage.rs:83`, `control-plane/crates/db/src/repos/stages.rs:166` | Domain/DB repo | `StageExecution` has `settlement_kind`, retry/recovery JSON, and no durable `terminal_reason` field; `settle_tx` only writes status, settlement_kind, completed_at. | Recovered-orphan explanation storage. |
| MAP-11 | Startup recovery order | `control-plane/crates/engine/src/recovery.rs:355`, `:362`, `:799` | Recovery service | Current startup recovery rebuilds operator projections before `repair_run`, then can enqueue generic run-scoped `startup_catchup` `AdvanceRun`. | Orphan repair ordering risk. |
| MAP-12 | Proposal gate implementation | `scripts/test-gate.sh:7095`, `docs/reference/test-gates.md:2051` | Test gate/docs | Current `proposal-091` gate validates evidence inventory and required proposal terms; docs explicitly say it is not proof of runtime implementation. | Gate readiness vs implementation proof. |

## E. Fingerprint summary

| Tag type | Tag | Evidence IDs | Reason |
|---|---|---|---|
| Stack | `rust-backend` | DOC-03, DOC-04, MAP-01, MAP-04 | P091 changes Rust engine, DB, work queue, recovery, and projections. |
| Stack | `shared-api` | DOC-10, DOC-12, REF-02, MAP-08, MAP-09 | Proposal changes stored payload contract and public readback fields. |
| Stack | `cross-stack` | DOC-12, REF-02 | Swift/operator consumers rely on server-owned projection truth. |
| Surface | `architecture` | DOC-03, DOC-07, MAP-05 | Stage execution authority changes orchestrator state selection. |
| Surface | `state-management` | DOC-03, DOC-08, MAP-01 | Retry truth and orphan recovery mutate stage/run state. |
| Surface | `background-work` | DOC-05, DOC-06, MAP-06, MAP-07 | Work item propagation/requeue/cancel paths are affected. |
| Surface | `concurrency` | DOC-04, DOC-11, MAP-07 | Concurrent retries and requeues need conflict-safe durable authority. |
| Surface | `api-contract` | DOC-10, DOC-12, MAP-04, MAP-09 | Typed payload and GraphQL/MCP/readback changes are contract changes. |
| Surface | `persistence` | DOC-04, DOC-08, DOC-12, MAP-08, MAP-10 | Durable authority table, provenance, and projection changes are required. |
| Surface | `migration` | DOC-04, DOC-12, DOC-14, MAP-08 | New tables/columns/readback compatibility are required. |
| Surface | `rollout` | DOC-13, DOC-14, MAP-12 | Proposal requires focused gate upgrade and staged rollout. |
| Surface | `telemetry` | DOC-12, DOC-13, ART-03 | Operator diagnostics must expose active and recovered retry authority. |
| Risk | `backward-compatibility` | DOC-10, DOC-13 | Legacy `AdvanceRun` payloads remain processable. |
| Risk | `idempotency` | DOC-04, DOC-11, MAP-07 | Retry authority creation, supersession, recovery, and requeue must be replay safe. |
| Risk | `data-loss` | DOC-08, DOC-09, MAP-10 | Orphan recovery writes terminal state to historical stage executions. |
| Risk | `availability-sensitive` | DOC-09, MAP-11 | Recovery must not mutate legitimate waits or capacity/backpressure holds. |
| Risk | `operability-sensitive` | DOC-12, DOC-13, ART-03 | Operators need clear readback for active and recovered retry authority. |
| Risk | `multi-service-coordination` | DOC-12, REF-02 | GraphQL, MCP, reports, DB projections, and engine must agree. |
| Risk | `user-trust` | DOC-01, ART-03 | A retry command that strands the intended attempt directly violates operator expectations. |

## F. Routing decision

Selected reviewers:

| Reviewer ID | Mode | Evidence IDs | Why selected | Repo-local agent used? |
|---|---|---|---|---|
| `chainworks_execution_truth_reviewer` | architecture-only | DOC-03, DOC-07, DOC-08, REF-01, MAP-05, MAP-11 | Proposal changes durable Run/StageExecution/retry/projection truth. | No, rubric used in main thread. |
| `rust_reliability_reviewer` | reliability-only | DOC-04, DOC-05, DOC-09, MAP-06, MAP-07, MAP-11 | Proposal changes retry, idempotency, requeue, recovery, and background work behavior. | No, rubric used in main thread. |
| `api_contract_reviewer` | api-contract-only | DOC-10, DOC-12, MAP-04, MAP-08, MAP-09 | Proposal changes stored `AdvanceRun` payload shape and public readback fields. | No, rubric used in main thread. |
| `observability_rollout_reviewer` | observability-rollout-only | DOC-12, DOC-13, DOC-14, ART-04, MAP-12 | Proposal adds recovery mutation/readback and requires gate upgrade/rollout sequencing. | No, rubric used in main thread. |

Rejected close alternatives:

| Reviewer ID | Evidence IDs | Why not selected |
|---|---|---|
| `rust_arch_reviewer` | DOC-03, MAP-05 | Covered by repo-local execution-truth reviewer plus Rust reliability; no separate generic architecture pass needed. |
| `rust_security_reviewer` | REF-03 | Retry preflights touch side-effect safety, but no new auth, secret, permission, parsing, or external-command boundary is proposed beyond existing operator-command surfaces. |
| `apple_arch_reviewer` | REF-02 | Swift client remains read-side consumer; P091 primarily changes server-owned truth/readback. |
| `product_reviewer` | DOC-01 | User trust matters, but no product metric, experiment, prioritization, or adoption decision is central. |

Routing cap status: `4 selected; target 2-4; hard cap 5.`

## G. State and failure coverage matrix

| State / failure class | Proposal coverage | Evidence IDs | Gap / risk | Reviewer owner |
|---|---|---|---|---|
| Entry / setup | Ready | DOC-03, DOC-11, MAP-01, MAP-03 | Old targeted-agent lifecycle gap is now closed. | chainworks_execution_truth_reviewer |
| Happy path | Ready | DOC-03, DOC-07, DOC-13 | Full-stage and targeted-agent retry intended target behavior is specified. | chainworks_execution_truth_reviewer |
| Loading / in-flight | Ready with conditions | DOC-05, DOC-06, MAP-06 | Post-invoke propagation is specified; target-aware repository tests remain required implementation proof. | rust_reliability_reviewer |
| Timeout / cancellation | Partial | DOC-06, DOC-09, MAP-07 | Run/stage-scoped cancel/requeue helpers must be converted or guarded by authority-specific variants. | rust_reliability_reviewer |
| Retry / replay / idempotency | Ready with conditions | DOC-04, DOC-11 | Active-authority uniqueness is DB-enforced; recovered-orphan repair still needs atomic/idempotent insertion+settlement proof. | rust_reliability_reviewer |
| Persistence / migration | Partial | DOC-04, DOC-08, DOC-12, MAP-08, MAP-10 | Terminal reason/readback storage should be pinned to exact schema/API fields. | api_contract_reviewer |
| Dependency failure | Ready with conditions | DOC-09, REF-03 | Exclusion classes are listed; implementation must wire concrete source tables. | rust_reliability_reviewer |
| Rollback / recovery | Partial | DOC-08, DOC-09, MAP-11 | Startup recovery ordering must be pinned against existing projection rebuild and generic catch-up paths. | chainworks_execution_truth_reviewer |
| Observability / support | Partial | DOC-12, ART-04, MAP-09, MAP-12 | Authority history readback is specified; current gate is inventory-only and must be upgraded for implementation closeout. | observability_rollout_reviewer |

## H. Proposal completeness matrix

| Dimension | Status | Evidence IDs | Notes |
|---|---|---|---|
| Problem and target user | Ready | DOC-01, ART-03 | Problem is concrete and evidence-backed. |
| Scope and non-goals | Ready | DOC-03, DOC-15 | Scope avoids broad workflow redesign. |
| Current-system fit | Ready with conditions | MAP-01, MAP-03, MAP-05, MAP-11 | Root cause and main entry paths match current code; startup recovery insertion point needs tightening. |
| Data / state model | Ready with conditions | DOC-04, DOC-08, DOC-12, MAP-10 | Authority row and unique active index are specified; terminal reason storage/readback needs exact placement. |
| API / contract compatibility | Partial | DOC-10, DOC-12, MAP-04, MAP-09 | Legacy compatibility is specified; partially targeted typed payload cases need a complete classification matrix. |
| Runtime / concurrency semantics | Ready with conditions | DOC-04, DOC-05, DOC-06, MAP-07 | Main races are addressed; repository helper conversion is acceptance-critical. |
| Failure handling | Ready with conditions | DOC-09, REF-03 | Exclusion list is strong; implementation must map every exclusion to current data source. |
| Security / privacy / auth | Not applicable | REF-03 | No new security boundary beyond existing operator command and side-effect preflight rules. |
| Migration / rollout / rollback | Partial | DOC-14, MAP-12 | Rollout sequence exists; runtime gate upgrade and recovery rollback/forward-fix behavior remain implementation conditions. |
| Observability / diagnostics | Ready with conditions | DOC-12, MAP-09 | Authority readback and history are specified; stage-level terminal reason visibility needs exact API contract. |
| Test / proof gate | Partial | DOC-13, ART-04, MAP-12 | Readiness gate passes; implementation must upgrade it with focused DB/engine/recovery/API tests. |
| Product metrics / decision checkpoint | Not applicable | DOC-01 | Product reviewer not selected. |

## I. Evidence gaps and fallback decisions

| Gap ID | Missing evidence | Blocks routing or finding? | Next local artifact or file to inspect | Integration-context refresh needed? |
|---|---|---|---|---|
| GAP-01 | P091 implementation migration/repositories/runtime tests do not exist yet. | Does not block proposal-readiness review. | Future DB migration, retry-authority repo, typed payload parser, recovery pass, GraphQL/MCP readback implementation, upgraded `proposal-091` gate. | No. |
| GAP-02 | Exact terminal-reason API placement is not fully pinned. | Does not block routing; informs F-02. | Proposal section 9.3/9.4 should specify whether terminal reason is stage-owned, authority-history-owned, or projected into both. | No. |
| GAP-03 | Current gate is readiness inventory only. | Does not block proposal-readiness review; would block implementation closeout. | Upgrade `scripts/test-gate.sh proposal-091` with DB/engine/recovery/work-item/API tests during implementation. | No. |

## J. Research triggers

| Trigger ID | Local evidence IDs | Question local evidence cannot settle | Required source type | Why it matters |
|---|---|---|---|---|
| RES-01 | None | None. | N/A | Local repo evidence is sufficient for this review. |

## K. Findings ledger

| Finding ID | Reviewer | Severity | Evidence IDs | File / lines | Summary | Confidence |
|---|---|---|---|---|---|---|
| F-01 | chainworks_execution_truth_reviewer, rust_reliability_reviewer | P1 | DOC-08, DOC-13, MAP-11 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:353`, `control-plane/crates/engine/src/recovery.rs:355`, `control-plane/crates/engine/src/recovery.rs:799` | Startup orphan repair is conceptually specified, but not pinned against the current recovery order that rebuilds projections before `repair_run` and can enqueue a generic run-scoped `startup_catchup` `AdvanceRun`. | 0.80 |
| F-02 | api_contract_reviewer, observability_rollout_reviewer | P2 | DOC-08, DOC-12, MAP-08, MAP-09, MAP-10 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:308`, `docs/proposals/091-targeted-retry-stage-execution-authority.md:445`, `control-plane/crates/graphql-server/src/types/stage.rs:108` | `terminal_reason = stale_retry_recovered` is required, but the proposal does not pin whether that reason is stage-owned, authority-history-owned, or projected into stage readback. | 0.76 |
| F-03 | api_contract_reviewer, rust_reliability_reviewer | P2 | DOC-10, MAP-04, MAP-06, MAP-07 | `docs/proposals/091-targeted-retry-stage-execution-authority.md:391`, `control-plane/crates/engine/src/executor.rs:3724` | The typed payload contract does not fully classify partially targeted payloads, so `retry_authority_id`/target/source-field mismatches can be implemented inconsistently. | 0.74 |

