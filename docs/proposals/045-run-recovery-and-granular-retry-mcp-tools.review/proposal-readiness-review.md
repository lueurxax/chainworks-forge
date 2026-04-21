# P045 Run Recovery and Granular Retry MCP Tools - Proposal Readiness Review

Proposal: `docs/proposals/045-run-recovery-and-granular-retry-mcp-tools.md`  
Review mode: `proposal-readiness` via `rust-proposal-review-triad`  
Evidence pack: `docs/proposals/045-run-recovery-and-granular-retry-mcp-tools.review/evidence-pack.md`  
Reviewed on: 2026-04-17  
Git SHA: `bf06b30f4a6c439dc046410756b9d18a972b25b2`  
Working tree: Dirty; this is a repo-local proposal-readiness review, not a runtime implementation audit.

## Verdict

Readiness: **Red**.

The operator goal is valid and the target recovery gaps are real, but the proposal is not implementation-ready as written. It frames the work as MCP-first and claims no schema or workflow/transition changes, while the core behaviors require durable resume cursor truth, agent-level retry lineage fields, executor/orchestrator changes, MCP namespace registration, capability/auth policy, and a new proof lane. Several required contracts are also internally inconsistent: `SuggestRecoveryCmd` is listed but not migrated, read-only tools are said to journal like commands, the suggestion engine is both "AI-ranked" and deterministic, and the proposal number collides with the existing deterministic-release P045 gate.

Evidence completeness: **Complete** for proposal-readiness.  
Confidence: **High**.  
External research: **Not used**; no external research trigger was found.  
Runtime evidence: **Not collected**; default `proposal-readiness` does not require build/test execution.

## Discipline Scorecard

| Discipline | Status | Confidence | Evidence Completeness | Primary Risk |
|---|---|---:|---|---|
| Architecture | Red | High | Complete | Core behaviors require schema/runtime/tooling changes that the proposal excludes or omits. |
| Reliability | Red | High | Complete | Resume/retry/skip semantics are not atomic or durable enough to prevent duplicate work, skipped artifacts, or ambiguous lineage. |
| Performance | Green | Medium | Complete for proposal-readiness | The work is operator-triggered and read-heavy; no hot-path performance blocker found. |
| Security | Red | High | Complete | New mutating recovery tools lack explicit capability/auth policy and `stages.skip` can bypass critical workflow states. |
| Product | Amber | Medium | Complete for proposal-readiness | Operator pain is clear, but recommendation semantics and safe-action boundaries need sharpening. |

## Findings

### ARCH-045-01 - Critical - `runs.resume` depends on non-existent durable cursor fields while the proposal forbids the required schema/runtime changes

Evidence: DOC-01, DOC-05, DOC-06, MAP-04, MAP-05, SURF-04, DATA-02, REAL-01.

The proposal defines `runs.resume` around `transition_cursor` and `settlement_state = next_state_scheduled_not_started`, but the current Rust `Run` model/schema only exposes `current_state`; no cursor or settlement-state fields exist. Current `RecoveryService` is startup repair/catchup, not an on-demand durable cursor resume API. Because the proposal simultaneously says "No schema changes" and excludes workflow execution/transition logic changes, the specified resume behavior cannot be implemented without violating the proposal.

Required fix: Either add an explicit dependency on a landed durable-transition-cursor contract and map the exact fields/API to Rust, or expand this proposal to add the missing run/state cursor schema, repo functions, atomic resume claim, and orchestrator/executor integration.

Acceptance criteria:

- The proposal names the durable resume owner fields and their migration, or cites the exact stable reference that already owns them.
- `runs.resume` has an atomic duplicate-work guard that checks active work and claims resume ownership in one transaction-equivalent path.
- Tests prove cursor resume, latest-stage catchup, active-work rejection, terminal-run rejection, and repeated `runs.resume` idempotency.

### ARCH-045-02 - Critical - `agents.retry` requires durable agent retry lineage fields that do not exist

Evidence: DOC-01, DOC-06, MAP-02, MAP-03, MAP-04, MAP-08, DATA-01, REAL-02.

The proposal requires `agent_attempt_number`, `supersedes_agent_execution_id`, and `reused_sibling_execution_ids`, but current `AgentExecution` and `agent_executions` persistence do not contain those fields. `InvokeAgent` currently creates a fresh `AgentExecution` from work-item payload and does not consume a pre-created execution ID. As written, a same-stage agent retry cannot durably preserve sibling reuse or supersession truth, and the verification bullets cannot pass without schema and executor contract changes.

Required fix: Add the retry-lineage persistence contract to the proposal: fields, migration, repository APIs, executor payload semantics, and report/evidence readback. If the intent is to reuse existing `owner_execution_lineage_id` instead, specify the replacement contract and remove the nonexistent field names.

Acceptance criteria:

- Agent retry lineage survives DB readback with attempt number, superseded execution, and sibling reuse metadata.
- `InvokeAgent` can execute a command-created retry without creating an unrelated lineage entry.
- Stage completion logic ignores superseded failed attempts after a successful retry while preserving historical evidence.
- Focused tests cover single failed sibling retry, repeated agent retry, cancelled-agent retry, and running-agent rejection.

### REL-045-03 - High - `stages.skip` is underspecified and conflicts with artifact-driven transition truth

Evidence: DOC-01, DOC-06, SURF-06, MAP-06, MAP-07, DATA-04, INT-04.

The proposal says `stages.skip` should "evaluate transitions as if the stage completed normally" and force-advance to the next state, but current transition evaluation reads canonical artifact paths and fails closed when required artifacts are absent. The current workflow has many artifact/approval-dependent transitions and no `skippable` metadata. A comment-only guard plus warning in `recovery.suggest` does not define how the runtime safely bypasses implementation, review, release, or artifact-producing states.

Required fix: Define a first-class skip policy before implementation: skippable metadata, critical-state defaults, downstream artifact dependency checks, transition override semantics, and what evidence artifact records the operator override.

Acceptance criteria:

- `stages.skip` rejects non-skippable/manual release/approval-critical/end states by default.
- The command computes downstream artifact dependencies before mutating state.
- If skip is allowed, the runtime records explicit skip evidence and either uses a defined synthetic transition reason or fails closed without changing state.
- Tests prove artifact-dependent transitions cannot be silently bypassed.

### SEC-045-04 - High - MCP capability/auth and namespace registration are missing for the new recovery tools

Evidence: SURF-01, SURF-02, MAP-01, MAP-09, DATA-05, REAL-03.

The migration says to add `recovery.rs` with all six tools, but current MCP dispatch has no `agents.*` or `recovery.*` namespace and the typed capability/auth model has no IDs or class policy for the proposed tools. These tools are security-sensitive because they mutate run state, retry work, re-open approvals, and skip stages. Without explicit capability IDs and class policy, implementation can either fail closed at discovery/execution or accidentally bypass least-privilege review.

Required fix: Expand the proposal to cover `CapabilityToolId` variants, `auth` default capabilities, allowed principal classes, `tools::capability_id_for`, dispatch routing, and discovery tests for each tool.

Acceptance criteria:

- Every new tool has a typed capability ID and explicit operator/agent/observer policy.
- Mutating tools are operator-only unless the proposal explicitly justifies otherwise.
- Read-only evidence/suggest tools have deliberate observer access policy.
- MCP tests cover tool discovery, allowed execution, forbidden execution, and unknown namespace rejection.

### TEST-045-05 - High - Proposal number and proof lane collide with existing deterministic-release P045

Evidence: DOC-07, INT-01, TEST-01, REAL-04.

The proposal is titled Proposal 045, but `docs/reference/test-gates.md` and `scripts/test-gate.sh` already use `proposal-045` for deterministic release operations. That former P045 is also promoted to `docs/reference/045-deterministic-release-operations.md`. The new proposal has no unambiguous proof lane, so implementers cannot tell what `scripts/test-gate.sh proposal-045` should prove.

Required fix: Renumber the proposal or create a distinct gate alias and reference entry that does not overwrite the deterministic-release gate.

Acceptance criteria:

- The proposal number and filename do not conflict with stable former/current P045 artifacts.
- `docs/reference/test-gates.md` has a dedicated gate for this proposal.
- `scripts/test-gate.sh` has a matching alias whose tests prove the recovery tools, not deterministic release.

### READY-045-06 - Medium - Command/journaling contract is internally inconsistent

Evidence: DOC-01, METRIC-01, REAL-05, REAL-06.

The scope lists five command variants including `SuggestRecoveryCmd`, but migration lists only four new command structs. The suggestion engine is described as pure/no side effects, while verification says all tools record to `command_journal` with caller identity. The proposal also calls suggestions "AI-ranked" in the summary but deterministic/no-LLM in the risk table. These contradictions make the implementation contract and tests ambiguous.

Required fix: Split tools into mutating commands and read-only queries, then define whether read-only recovery tools produce command-journal rows, access-log rows, or no journal. Pick one suggestion model: deterministic rules or LLM/AI-ranked.

Acceptance criteria:

- Tool table states command-backed vs direct-read for all six tools.
- Response schemas consistently include or omit `journal_id`.
- `recovery.suggest` has deterministic ranking rules or an explicit AI/LLM dependency, not both.
- Tests assert journaling behavior per tool.

### SEC-045-07 - Medium - Approval re-arm loop controls are deferred instead of specified

Evidence: DOC-01, DATA-03, METRIC-02, H matrix.

`approvals.rearm` re-opens rejected approval gates, but the proposal only says to reject if the stage has already been retried and logs re-arm count in the command journal; it defers `max_rearms` to the future. Current approval records do not have lineage or rearm-count fields, so repeated pending/rejected approval history can become ambiguous unless the contract defines how to select the active approval and limit loops.

Required fix: Define active approval selection, duplicate pending approval rejection, re-arm lineage, and either a hard max or workflow-level policy in this proposal.

Acceptance criteria:

- Re-arm rejects when a pending/requested approval already exists for the stage.
- Re-arm lineage links old and new approval records or records an explicit journal payload that can be queried.
- Tests cover first rearm, duplicate rearm rejection, already-retried stage rejection, and projection refresh.

## Required Changes Before Implementation

1. Rename or renumber this proposal, or allocate a distinct test-gate alias that does not collide with deterministic-release P045.
2. Add or cite the durable resume cursor contract; remove the "no schema changes" claim if this proposal owns the fields.
3. Add the agent retry lineage schema/executor contract, or rewrite `agents.retry` around fields that actually exist.
4. Define `stages.skip` as a safe workflow-policy feature, not just a stage settlement update.
5. Add full MCP namespace, tool registry, capability/auth, and discovery coverage.
6. Resolve command/journaling/read-only query semantics for `recovery.evidence` and `recovery.suggest`.

## Non-Blocking Notes

- The proposal correctly identifies a real operator gap: MCP currently has stage retry/cancel/approval resolution, but not granular run recovery and evidence tools.
- Existing failed-stage evidence and validation-record infrastructure can support `recovery.evidence`; the proposal should explicitly reuse those canonical owners.
- Performance is not a blocker in readiness review because the tools are operator-triggered and mostly read/query paths, but `recovery.suggest` should bound DB reads by run and latest stage/attempt.

## Suggested Next Step

Revise the proposal before implementation. The minimal viable revision is not another audit pass; it should update the contract to include schema/runtime/auth/test-gate changes and remove the "MCP-only/no schema/no transition changes" framing where it contradicts the desired behavior.
