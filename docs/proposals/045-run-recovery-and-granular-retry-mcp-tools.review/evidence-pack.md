# Proposal Evidence Pack

Proposal: `docs/proposals/045-run-recovery-and-granular-retry-mcp-tools.md`  
Mode: `proposal-readiness` via `rust-proposal-review-triad`  
Verified on: 2026-04-17  
Git SHA: `bf06b30f4a6c439dc046410756b9d18a972b25b2`  
Working tree: Dirty; broad control-plane/doc changes are present. This review is proposal-readiness only, not a runtime implementation audit.

## A. Repo-Local Proposal / Document Inventory

| Evidence ID | Source / Path / Artifact | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---:|---|---|---|---|
| DOC-01 | `docs/proposals/045-run-recovery-and-granular-retry-mcp-tools.md` | 2026-04-17 | High | Draft proposal adds `runs.resume`, `agents.retry`, `approvals.rearm`, `stages.skip`, `recovery.evidence`, and `recovery.suggest`. It also claims no schema changes and no workflow execution/transition changes. | Review could miss contract gaps hidden behind "MCP-only" framing. | Primary artifact. |
| DOC-02 | `docs/reference/p041-generated-artifact-schemas.md` and `docs/reference/test-gates.md#proposal-041p041` | 2026-04-17 | Medium | The dependency now resolves to the stable server parity harness schema and gate contracts, which focus on parity harness/golden runs rather than durable resume cursor or granular retry persistence. | Proposal could assume unavailable prerequisite behavior. | Dependency validation. |
| DOC-03 | `.review-baselines/current-system-baseline.md` | 2026-04-17 | High | Reviews should prefer stable references and current repo reality over proposal lineage. | Could rely on stale proposal assumptions. | Baseline intake. |
| DOC-04 | `docs/reference/current-system-baseline.md` | 2026-04-17 | High | Current baseline already includes execution truth/recovery, ACP transport, MCP policy, operator shell recovery, and Rust control-plane surfaces. | Proposal could duplicate or conflict with current baseline. | Current subsystem map. |
| DOC-05 | `docs/reference/execution-truth-and-recovery.md` | 2026-04-17 | High | Stable recovery truth is stage-owned; recovery uses narrowest valid next action and resume is fail-closed. | New MCP tools could bypass stage-owned recovery truth. | Recovery semantics. |
| DOC-06 | `docs/reference/runtime-contract.md` | 2026-04-17 | High | Existing runtime policy says each retry creates a new stage attempt and new artifacts; external side-effect stages never auto-resume silently. | Same-stage agent retry and skip semantics may conflict if not explicitly narrowed. | Runtime invariants. |
| DOC-07 | `docs/reference/test-gates.md` and `scripts/test-gate.sh` | 2026-04-17 | High | `proposal-045` currently names deterministic release operations, not run recovery/retry MCP tools. | The proposal has no unambiguous proof lane. | Verification governance. |

## B. Reusable Baseline Inputs

| Evidence ID | Artifact / Slice | Status | Covered Surfaces | Verified On | Confidence | Freshness Notes | Relevance |
|---|---|---|---|---:|---|---|---|
| BASE-01 | `.review-baselines/current-system-baseline.md` | Reused | review precedence and stable reference posture | 2026-04-17 | High | Fresh for source-of-truth rules. | Baseline. |
| BASE-02 | `docs/reference/current-system-baseline.md` | Reused | current Rust/control-plane/operator recovery subsystem map | 2026-04-17 | High | Fresh enough for baseline orientation; targeted code mapping performed for proposal-specific surfaces. | System context. |
| BASE-03 | `docs/reference/execution-truth-and-recovery.md` | Reused | stage-owned evidence, recovery snapshots, fail-closed resume | 2026-04-17 | High | Current reference is Swift-heavy in wording but describes stable recovery ownership expected by current repo. | Recovery contract. |

## C. Scope, Out-of-Scope, and Intentional Deferrals

- In scope: proposal-readiness for new Rust MCP recovery tools; command/domain/MCP/capability/auth/queue/recovery/evidence/transition surfaces.
- Out of scope: implementation audit, running `cargo test`, simulator or Swift app validation, external research.
- Deferred intentionally: runtime gate execution; default `proposal-readiness` does not require build/run evidence.
- Assumption: the user wants a readiness review of the proposal as written, not implementation of the tools in this turn.

## D. Affected Runtime / Entry-Point / Protocol Slice

| Evidence ID | Surface / Entry Point / Runtime Boundary | Source | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---:|---|---|---|---|
| SURF-01 | MCP tool dispatch | `control-plane/crates/mcp-server/src/server.rs` | 2026-04-17 | High | Dispatch currently routes `ideas.*`, `runs.*`, `approvals.*`, `stages.*`, `reports.*`, and `steward.*`; no `agents.*` or `recovery.*` namespace is routed. | New tools may be registered but unreachable. | MCP surface. |
| SURF-02 | MCP tool registry | `control-plane/crates/mcp-server/src/tools/mod.rs` | 2026-04-17 | High | Registered capability tool IDs are fixed at 13 and do not include the six proposed tools. | Capability/auth gating will fail closed or omit tools. | MCP discovery/auth. |
| SURF-03 | Command handler | `control-plane/crates/engine/src/command_handler.rs` | 2026-04-17 | High | Existing command variants cover start, approve, reject, retry stage, cancel, reset session, steward analysis. | Proposal must add more than a new MCP module. | Command path. |
| SURF-04 | Startup recovery | `control-plane/crates/engine/src/recovery.rs` | 2026-04-17 | High | Current recovery scans active runs, marks running stages blocked, and enqueues `AdvanceRun` when no active work exists; it does not expose an on-demand resume command. | `runs.resume` cannot be a thin wrapper around an existing exact API. | Resume semantics. |
| SURF-05 | Work queue | `control-plane/crates/db/src/repos/work_items.rs` | 2026-04-17 | High | Work items support pending/running/completed/failed/cancelled and list-by-run active work checks are possible. | Race guards need explicit use. | Active-work guard. |
| SURF-06 | Transition evaluator | `control-plane/crates/engine/src/orchestrator.rs` | 2026-04-17 | High | Transitions evaluate artifact existence and artifact-field expressions from canonical filesystem paths; missing artifacts fail closed for known artifacts. | Stage skip may not advance if it does not create or override required artifact truth. | Skip semantics. |

## E. Impacted Crates / Modules / Code-Path Map

| Evidence ID | File Path / Crate / Module / Symbol | Layer | Role in Flow | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---:|---|---|---|---|
| MAP-01 | `control-plane/crates/domain/src/commands.rs` | Domain | `Command` and command payload types | 2026-04-17 | High | No `ResumeRunCmd`, `RetryAgentCmd`, `RearmApprovalCmd`, `SkipStageCmd`, or `SuggestRecoveryCmd` exists today. | Proposal undercounts domain-command changes. | Command model. |
| MAP-02 | `control-plane/crates/domain/src/agent.rs` | Domain | `AgentExecution` model | 2026-04-17 | High | `AgentExecution` has no `agent_attempt_number`, `supersedes_agent_execution_id`, or `reused_sibling_execution_ids`. | `agents.retry` acceptance cannot be implemented without new durable fields or a changed contract. | Agent retry. |
| MAP-03 | `control-plane/crates/db/src/repos/agent_executions.rs` | Persistence | Agent execution insert/select/readback | 2026-04-17 | High | Agent execution persistence does not read/write retry attempt or supersession fields. | Same-stage retry lineage would be non-durable. | Agent retry persistence. |
| MAP-04 | `control-plane/crates/db/migrations/*.sql` | Persistence | Schema history | 2026-04-17 | High | Existing migrations add session/MCP/steward fields but no agent retry attempt/supersession fields and no transition cursor fields. | `No schema changes` is false for proposal behavior as written. | Migration truth. |
| MAP-05 | `control-plane/crates/domain/src/run.rs` and `db/src/repos/runs.rs` | Domain/persistence | Run state | 2026-04-17 | High | Run has `current_state` but no `transition_cursor` or `settlement_state` field. | `runs.resume` cursor behavior cannot be implemented as written. | Resume cursor. |
| MAP-06 | `control-plane/crates/domain/src/stage.rs` and `db/src/repos/stages.rs` | Domain/persistence | Stage status and settlement | 2026-04-17 | High | `StageSettlementKind::Skipped` exists and `stages::settle` maps it to `StageStatus::Skipped`. | Some skip primitive exists, but not sufficient for safe force-advance. | Stage skip. |
| MAP-07 | `control-plane/crates/engine/src/orchestrator.rs` | Engine | Stage settlement, transition evaluation, failed-stage evidence | 2026-04-17 | High | Failed multi-agent stages persist recovery/evidence and block the run; skipped stages call existing transition evaluation. | Proposal must specify how granular retry/skip interacts with these paths. | Engine behavior. |
| MAP-08 | `control-plane/crates/engine/src/executor.rs` | Engine | Agent execution creation | 2026-04-17 | High | `InvokeAgent` creates a fresh `AgentExecution` from work-item payload, but payload does not identify a pre-created agent execution id. | `agents.retry` cannot simply create the execution then enqueue a generic `InvokeAgent` without executor contract changes. | Agent retry runtime. |
| MAP-09 | `control-plane/crates/domain/src/capabilities.rs` and `auth/src/lib.rs` | Auth/capability | Tool capability IDs and class policy | 2026-04-17 | High | Capability enum and auth mapping omit proposed tools. | Unauthorized tools may be invisible, or mutating recovery tools may skip least-privilege review. | Security/auth. |
| MAP-10 | `control-plane/crates/mcp-server/src/tools/reports.rs`, `db/src/repos/validation.rs`, `engine/src/evidence.rs` | Evidence/report | Failure evidence and validation records | 2026-04-17 | High | Validation records and failed-stage evidence already exist, but no single `recovery.evidence` tool assembles the proposed response shape. | Proposal should reuse these owners rather than inventing conflicting truth. | Evidence endpoint. |

## F. Data / Protocol / Persistence / Auth Touchpoints

| Evidence ID | Touchpoint | File / Module / Doc | Direction | Verified On | Confidence | Key Fact | Risk if Wrong | Relevance |
|---|---|---|---|---:|---|---|---|
| DATA-01 | Agent retry lineage | `AgentExecution`, `agent_executions` table | command -> DB -> executor/report | 2026-04-17 | High | Proposal names fields not present in domain or schema. | Retry truth cannot be audited. | Core retry contract. |
| DATA-02 | Resume cursor | `Run`, `runs` table | recovery command -> scheduler | 2026-04-17 | High | Proposal names `transition_cursor` and `settlement_state`, but current Rust run model has only `current_state`. | Resume may duplicate or skip work. | Resume contract. |
| DATA-03 | Approval re-arm | `approvals`, `StageExecution`, `RunStatus` | command -> DB -> event -> projection | 2026-04-17 | Medium | Approval records support pending/requested/granted/rejected, but no lineage/rearm counter exists. | Infinite or ambiguous rearm lineage. | Approval recovery. |
| DATA-04 | Stage skip decision | `command_journal`, stage settlement, transition evaluator | operator command -> audit -> transition | 2026-04-17 | High | Comment can be journaled, but no workflow-owned `skippable` policy or downstream dependency model exists. | Operator can bypass critical gates/artifact requirements. | Skip safety. |
| DATA-05 | Tool capability IDs | `domain::CapabilityToolId`, `auth` | principal -> tool discovery/execution | 2026-04-17 | High | New tools need explicit capability IDs and class policy. | Security policy incomplete. | MCP auth. |

## G. Current Host-System Integration Surfaces

| Evidence ID | Surface / Seam / Owner | Source | Verified On | Confidence | Key Fact | Conflict / Proposal Risk | Relevance |
|---|---|---|---:|---|---|---|---|
| INT-01 | Proposal numbering / test gate | `docs/reference/test-gates.md`, `scripts/test-gate.sh` | 2026-04-17 | High | `proposal-045` is already deterministic release operations. | This new P045 cannot claim the `proposal-045` lane without replacing the existing stable gate. | Verification governance. |
| INT-02 | Existing stage retry | `CommandHandler::RetryStage` | 2026-04-17 | High | Current retry settles old stage as skipped and inserts a new stage attempt. | Same-stage agent retry is a new engine lineage model, not a minor MCP wrapper. | Retry semantics. |
| INT-03 | Failed-stage evidence | `engine::evidence`, `reports.get` | 2026-04-17 | High | Failed-stage evidence and validation records already have canonical owners. | `recovery.evidence` must read canonical owners and preserve precedence. | Evidence truth. |
| INT-04 | Current workflow topology | `examples/workflows/workflow.yaml` | 2026-04-17 | High | Many transitions depend on artifacts or approval truth; no `skippable` metadata exists. | Skip action needs explicit policy and dependency analysis. | Skip safety. |

## H. State and Failure Coverage Matrix

| State | Proposal Status | Evidence IDs | Repo Touchpoints | Notes / Risks |
|---|---|---|---|---|
| Entry | Partial | DOC-01, SURF-01, SURF-02 | MCP server and capability registry | Tool registration/auth is underspecified. |
| Happy path | Partial | DOC-01, MAP-01, MAP-06, MAP-10 | Command handler, evidence repos | Tool behavior is sketched, but several target fields/APIs do not exist. |
| Loading / inflight | Partial | DOC-01, SURF-05 | Work queue | Active-work guard is specified only for `runs.resume`; retry/skip/rearm races need equivalent guards. |
| Timeout | Missing | DOC-01, DOC-05 | Resume/retry runtime | No timeout classification for on-demand resume vs stuck provider work. |
| Validation error | Specified | DOC-01, MAP-10 | validation records/evidence | Good evidence source exists, but response shape must map canonical owners. |
| Dependency error | Partial | DOC-01, INT-04 | workflow artifact dependencies | Stage skip only warns; execution guard is missing. |
| Retry / replay | Incomplete | DOC-01, MAP-02, MAP-03, INT-02 | agent executions, stage retry | Agent-level retry needs durable lineage schema and executor contract. |
| Cancellation / shutdown | Partial | DOC-01, SURF-04 | startup recovery, work queue | On-demand resume may race with startup catchup/active work unless atomic claim semantics are specified. |
| Overload / backpressure | Missing | DOC-01 | work queue | Repeated `runs.resume` / `agents.retry` calls lack idempotency keys or duplicate enqueue guards. |
| Degraded / offline | Partial | DOC-01, DOC-05 | recovery snapshots | Suggestions can be deterministic, but missing data should fail closed. |
| Auth / permission failure | Incomplete | MAP-09, DATA-05 | capabilities/auth | Proposed tools lack capability IDs and class policy. |
| Rollback / migration failure | Incomplete | DOC-01, MAP-04 | migrations | Proposal says no schema changes despite missing durable fields. |

## I. Feature Flags / Rollout / Migration / Rollback

| Evidence ID | Mechanism / Flag | Scope | Rollout Plan | Rollback Path | Verified On | Confidence | Notes |
|---|---|---|---|---|---:|---|---|
| FLAG-01 | None specified | MCP recovery tools | Direct landing | Not described | 2026-04-17 | Medium | Mutating recovery tools may need gateable rollout or at least feature-scoped proof lane. |
| MIG-01 | "No schema changes" claim | Agent retry/resume | None | None | 2026-04-17 | High | Conflicts with missing agent retry and resume cursor fields. |

## J. Telemetry / Instrumentation

| Evidence ID | Event / Signal | Purpose | Trigger Point | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---:|---|---|
| METRIC-01 | `command_journal` rows | Audit mutating recovery commands | CommandHandler | 2026-04-17 | High | Proposal says all tools record caller identity, but read-only evidence/suggest command behavior is inconsistent. |
| METRIC-02 | `DomainEvent::ApprovalRequested` | Re-arm visibility | Approval rearm | 2026-04-17 | High | Event exists, but projection rebuild and duplicate pending approval handling need explicit acceptance. |
| METRIC-03 | Recovery suggestion ranking | Operator decision support | `recovery.suggest` | 2026-04-17 | Medium | Proposal says "AI-ranked" in scope but later says deterministic/no LLM. |

## K. Testing Strategy

| Evidence ID | Layer | Covered Surface | Current Coverage | Proposed Additions | Verified On | Confidence | Gap / Risk |
|---|---|---|---|---|---:|---|---|
| TEST-01 | Existing `proposal-045` gate | Deterministic release operations | Current docs/script map P045 to release, not recovery | New gate name/number needed | 2026-04-17 | High | Current verification section has no runnable owner. |
| TEST-02 | Engine integration tests | Existing approve/reject/retry/cancel/startup recovery | Existing tests cover stage retry and startup repair | Add focused tests for each new command and race/idempotency guards | 2026-04-17 | High | Proposal verification bullets are good starts but not mapped to files/gate. |
| TEST-03 | MCP server tests | Existing MCP run/report/approval/stage surfaces | Existing tests cover registered tools only | Add tool discovery, auth capability, execution, and forbidden-principal tests | 2026-04-17 | High | New tools are security-sensitive. |
| TEST-04 | Parity/golden harness | Server parity harness dependency | Stable schema/gate docs and golden harness tests exist in engine integration | Add recovery golden scenarios only if the server parity harness becomes the current proof owner for recovery scenarios | 2026-04-17 | Medium | Dependency is not enough by itself. |

## L. Current Repo Reality / Contradictions

| Evidence ID | Repo Surface | Proposal Claim | Current Repo Reality | Verified On | Confidence | Implication |
|---|---|---|---|---:|---|---|
| REAL-01 | Resume cursor | `transition_cursor` and `settlement_state` drive `runs.resume` | Rust run schema/model has no such fields | 2026-04-17 | High | Proposal must add schema/dependency or rewrite resume behavior. |
| REAL-02 | Agent retry schema | `agent_attempt_number`, `supersedes_agent_execution_id`, `reused_sibling_execution_ids` are used | Agent execution domain/schema lacks those fields | 2026-04-17 | High | `No schema changes` is not credible. |
| REAL-03 | MCP registration | Add `recovery.rs` with all six tools | Current namespaces require dispatch and registry/capability/auth changes; `agents.*` is a new namespace | 2026-04-17 | High | Migration section is incomplete. |
| REAL-04 | Test gate | Proposal is numbered 045 | `proposal-045` remains deterministic release operations in docs/script | 2026-04-17 | High | Verification ownership collides. |
| REAL-05 | Tool count / commands | Scope says 5 command variants including `SuggestRecoveryCmd` | Migration lists only 4 commands, while suggestion engine is pure/no side effects | 2026-04-17 | High | Command/journal contract is internally inconsistent. |
| REAL-06 | Suggestion model | Scope says AI-ranked suggestions | Risk mitigation says deterministic/no LLM | 2026-04-17 | Medium | Product behavior and testability need one contract. |

## M. Proposal Completeness Matrix

| Dimension | Status | Evidence IDs | Notes |
|---|---|---|---|
| Problem / user intent | Specified | DOC-01 | Operator recovery gaps are clear. |
| Scope boundaries | Contradictory | DOC-01, REAL-01, REAL-02 | Claims MCP-only/no execution/schema changes while requiring both. |
| Reusable baseline coverage | Complete | BASE-01, BASE-02, BASE-03 | Baseline and targeted code map were sufficient. |
| Runtime / entry-point definition | Partial | SURF-01, SURF-02, MAP-01 | Tool shapes exist, registration/auth incomplete. |
| State and failure handling | Partial | H matrix | Core states listed, race/idempotency/timeout coverage weak. |
| Data / protocol contract | Incomplete | DATA-01, DATA-02, DATA-05 | Missing durable fields and auth IDs. |
| Persistence / caching | Incomplete | MAP-02, MAP-03, MAP-04 | Schema claim conflicts with behavior. |
| Async/runtime assumptions | Incomplete | SURF-04, SURF-05, H matrix | On-demand resume races not specified atomically. |
| Permissions / auth expiry | Incomplete | MAP-09, DATA-05 | New mutating tools lack capability model. |
| Feature flags / rollout / rollback | Partial | FLAG-01, MIG-01 | No rollout/rollback beyond tests. |
| Telemetry / instrumentation | Partial | METRIC-01, METRIC-02 | Journal/event intent is present but inconsistent for read-only tools. |
| Testing / perf validation strategy | Incomplete | TEST-01, TEST-02, TEST-03 | Gate collision; no focused proof lane. |
| Dependencies / integration points | Incomplete | DOC-02, INT-01, INT-02 | Missing dependency on durable cursor/retry-lineage work. |
| Security / trust boundaries | Incomplete | MAP-09, DATA-04, DATA-05 | Stage skip/capability policy is underspecified. |

## N. Assumptions, Open Questions, and Blockers

- ASSUMP-01: The new P045 file is intended as an active proposal, not an archive artifact.
- ASSUMP-02: The dirty tree may include in-flight P050/P029 changes; current code mapping is still valid enough for proposal-readiness because missing fields/namespaces are direct repo facts.
- QUESTION-01: Should this proposal be renumbered to avoid the existing former/current P045 deterministic release gate?
- QUESTION-02: Is `recovery.suggest` intended to be deterministic or LLM/AI-ranked?
- QUESTION-03: Should read-only tools like `recovery.evidence` and `recovery.suggest` be command-journaled, or should only mutating commands return `journal_id`?
- BLOCKER-01: Resume behavior references durable cursor fields not present in the Rust model/schema.
- BLOCKER-02: Agent-level retry references durable lineage fields not present in the Rust model/schema.
- BLOCKER-03: New MCP tools lack capability/auth/dispatch/registration coverage.
- BLOCKER-04: Proposal/test-gate number collides with existing deterministic release P045.

## O. Research Triggers / External Questions

No external research trigger. All readiness findings are repo-local and do not require current external Rust or MCP guidance.
