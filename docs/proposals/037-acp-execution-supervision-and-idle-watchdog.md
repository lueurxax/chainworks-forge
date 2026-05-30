# Proposal 037: ACP Execution Supervision and Idle-Hang Watchdog

| Field | Value |
|---|---|
| Date | 2026-04-10 |
| Status | Draft |
| Author | Codex |
| Depends on | [../reference/acp-runtime-transport.md#implemented-transport-families](../reference/acp-runtime-transport.md#implemented-transport-families), [../reference/runtime-contract.md](../reference/runtime-contract.md), [../reference/execution-truth-and-recovery.md#atomic-transition-settlement-and-cursor-authority](../reference/execution-truth-and-recovery.md#atomic-transition-settlement-and-cursor-authority) |
| Scope | Introduce one ACP-only execution supervision contract that detects idle hangs, performs one automatic fresh retry, and surfaces deterministic truth in receipts, reports, recovery, and operator UI. |
| Goal | No ACP execution may remain indefinitely in `running` after progress has stopped. All ACP runtime families use the same watchdog contract and the same retry semantics. |

---

## 1. Context and Motivation

ACP has replaced Goose for the active runtime path, but execution supervision is still shaped by one coarse mechanism:

- a broad wall-clock timeout (`1800s`),
- a few narrow special cases such as proposal-review read-loop detection,
- and operator interpretation of whether a still-running process is "thinking" or "stuck".

Observed failures now cluster into one family:

1. session creation succeeds,
2. `prompt_submitted` is recorded,
3. the runtime either produces no meaningful progress or produces some early progress and then goes silent,
4. the run remains in `running` far longer than it should,
5. and the eventual failure is emitted as a generic timeout instead of a deterministic supervision reason.

This creates three product problems:

- operators cannot distinguish "slow but healthy" from "idle hang",
- the engine keeps dead executions alive for too long,
- and reports/recovery surfaces describe the failure too vaguely.

Proposal 037 fixes that by introducing one explicit ACP execution supervision contract.

---

## 2. Product Questions This Proposal Must Answer

After implementation, the system must be able to answer:

1. What counts as meaningful ACP execution progress?
2. When should an ACP execution be declared hung before the global `1800s` timeout?
3. Can the system perform one deterministic fresh retry without human intervention?
4. Can operator surfaces explain the difference between:
   - no progress after prompt submission,
   - progress followed by silence,
   - read-loop churn without advancement,
   - silence immediately after the first mutating tool boundary (`edit`, `apply_patch`, equivalent),
   - and mutating-tool success that never materializes into a real filesystem change?
5. Can all ACP families use the same supervision model and the same retry policy?

---

## 3. Scope

This proposal includes:

- one ACP-wide supervision contract based on `RuntimeStreamEvent`,
- explicit definitions of `meaningful progress`,
- watchdog deadlines for first progress, idle-after-progress, and weak read-loop progress,
- fail-closed verification for mutating-tool side effects,
- one automatic fresh retry for watchdog-triggered hangs,
- deterministic failure truth and reporting after retry exhaustion,
- operator-facing timeline/report/receipt semantics for watchdog intervention,
- acceptance tests and proof requirements for the new contract.

This proposal does **not** include:

- provider-specific watchdog policies,
- CPU-, socket-, or process-inspection heuristics as primary truth,
- changes to workflow YAML semantics,
- changes to stage transition selection,
- or any broader transport redesign beyond supervision and recovery behavior.

---

## 4. Core Contract

The engine must supervise ACP execution by one rule:

> ACP execution is considered alive only while the runtime emits meaningful progress events within the allowed supervision windows.

This proposal intentionally rejects process-level heuristics as canonical truth.
The following signals are diagnostic only and must not determine hang classification:

- subprocess PID still exists,
- CPU is non-zero or zero,
- sockets are open,
- child process is blocked in `kevent`,
- stdin/stdout file descriptors are still attached.

Canonical supervision truth is derived only from the `RuntimeStreamEvent` stream.

The contract applies equally to:

- `claude_agent_acp`
- `gemini_cli_acp`
- `codex_acp`
- `auggie_cli_acp`
- `junie_cli_acp`

No family-specific policy matrix is allowed in this proposal.

---

## 5. Meaningful Progress Model

### 5.1 Event classes

ACP events are divided into three classes.

**Terminal events**

- `finish`
- `final_output`
- `session_closed`
- `error`

These end or settle execution, but do not themselves count as continued progress.

**Meaningful progress events**

- `text_chunk`
- `tool_call_started`
- `tool_call_finished`
- future structured-output progress events, if introduced later

These events advance `lastMeaningfulProgressAt`.

**Bookkeeping events**

- `session_started`
- `prompt_submitted`
- transport-level notifications with no execution payload

These mark lifecycle phase but do not extend liveness.

### 5.2 Strong and weak progress

Not all meaningful progress is equally strong.

**Strong progress**

- `text_chunk`
- non-read `tool_call_started`
- non-read `tool_call_finished`

**Weak progress**

- repeated `read`
- repeated `permission:read`
- equivalent ACP read-only loop activity

Weak progress may delay failure briefly, but must not keep an execution alive indefinitely.

### 5.3 First mutating tool boundary

The supervision contract must explicitly track the first mutating tool boundary.

Examples:

- `edit`
- `apply_patch`
- `write`
- `permission:edit`
- future equivalent file-mutating ACP tool calls

This proposal treats the first mutating tool boundary as a special execution milestone because the observed Codex failure pattern is:

1. long discovery churn (`search`, `read`, `execute`),
2. first mutating tool call,
3. silence with no further meaningful progress.

That pattern must not be collapsed into generic `idle_hang_after_progress` if the telemetry can prove it occurred after the first mutating tool boundary.

### 5.4 Explicit non-goals

The supervision contract must not attempt to infer "the model is still thinking" from:

- lack of events plus a live process,
- intermittent OS-level activity,
- process memory growth,
- or external network sockets.

If the runtime does not emit meaningful progress, the execution is not healthy enough to remain indefinitely in `running`.

### 5.5 Mutating tool success is not self-proving

For file-mutating tool boundaries, provider-side success is not sufficient proof that the side effect actually happened.

Observed failure mode:

1. the runtime emits a mutating tool success (`edit`, `apply_patch`, equivalent),
2. the session continues reasoning as if the patch landed,
3. the real worktree remains unchanged,
4. token usage continues to grow,
5. and the execution later dies as an apparent idle hang or runaway session.

This proposal therefore requires a second integrity rule:

> mutating tool success counts as durable progress only if the expected filesystem side effect is observable in the worktree.

If no side effect is observable, the execution must fail closed as a mutation-integrity error rather than continue accruing tokens on a false state.

---

## 6. Watchdog Policy

This proposal defines one ACP-wide two-phase watchdog with fixed initial thresholds.

### 6.1 First-progress deadline

After `prompt_submitted`, the execution must emit at least one meaningful progress event within:

- `first-progress deadline = 120s`

If not, the execution is classified as:

- `idle_hang_before_first_progress`

### 6.2 Idle-after-progress deadline

After the first meaningful progress event arrives, the execution must continue emitting meaningful progress within:

- `idle-after-progress deadline = 300s`

If meaningful progress stops for longer than that window, the execution is classified as:

- `idle_hang_after_progress`

### 6.3 Weak read-loop deadline

If the execution keeps emitting only weak progress (`read`, `permission:read`, equivalent read-loop churn) without advancing into strong progress or terminal settlement, the execution must fail faster under:

- `read-loop weak-progress deadline = 120s`

Such executions are classified as:

- `idle_hang_read_loop`

### 6.4 Global timeout stays as outer guard

The existing broad execution timeout remains as a last-resort outer guard:

- `execution timeout = 1800s`

But once Proposal 037 is implemented, ordinary ACP hangs should almost always fail earlier via the supervision contract rather than via generic `Execution timed out`.

### 6.5 First-edit silence deadline

If execution has crossed the first mutating tool boundary and then emits no further meaningful progress within:

- `first-edit silence deadline = 120s`

the execution is classified as:

- `idle_hang_after_first_edit`

This classification is more specific than `idle_hang_after_progress` and is intended to catch the observed coding-session stall pattern where the runtime survives discovery, survives the first write authorization, and then goes silent before continuing productive work.

### 6.6 Mutating-side-effect verification

After the first mutating tool boundary, the engine must verify that a real filesystem mutation occurred within a short integrity window.

Initial rule:

- `mutating side-effect verification window = 30s`

Expected signals may include:

- a newly created file exists,
- a previously existing file has changed mtime and content,
- a patch/edit target now contains the expected textual delta,
- or another durable worktree-side mutation artifact exists.

If the provider reports mutating-tool success but no such side effect becomes observable within the window, the execution is classified as:

- `mutation_side_effect_missing`

This classification is not an idle-hang subtype. It is an execution-integrity failure.

---

## 7. Automatic Recovery Policy

### 7.1 Default retry policy

For all three watchdog classifications:

- `idle_hang_before_first_progress`
- `idle_hang_after_progress`
- `idle_hang_read_loop`
- `idle_hang_after_first_edit`
- `mutation_side_effect_missing`

the default recovery policy is:

- `automatic fresh retry count = 1`

### 7.2 Fresh retry semantics

The automatic retry must be a true fresh retry, not session reuse.

Required behavior:

1. invalidate the existing ACP session lineage/generation for that attempt,
2. close or terminate the stale session/subprocess,
3. create a new session,
4. resubmit the same execution packet,
5. record that the watchdog consumed the one automatic retry.

### 7.3 Durable retry-lineage owner

This proposal does not allow watchdog retry to remain executor-local.

The durable owner model is:

- `RuntimeAgentExecutor` may detect the watchdog fire and emit the retry request,
- `StageRetryCoordinator` owns creation of the durable retry lineage,
- `StageExecution` remains the owner of stage-level retry/recovery truth,
- `ResumeManager` and transition/cursor logic must treat an in-flight watchdog retry as normal same-stage work, not as interruption truth.

The first automatic watchdog retry must therefore be represented as:

- a new `AgentExecution`,
- inside the same `StageExecution`,
- with the same `StageExecution.attemptNumber`,
- with incremented `AgentExecution.agentAttemptNumber`,
- with `supersedesAgentExecutionID` pointing at the watchdog-failed attempt,
- and with `retryReason` set to a watchdog-specific value such as `automatic_watchdog_retry`.

This proposal explicitly rejects:

- hidden executor-local retries with no persisted lineage,
- automatic creation of a new `StageExecution` for the first watchdog retry,
- or any retry path that makes reports/recovery infer retry history from transport logs alone.

### 7.4 Retry exhaustion

If the fresh retry is also terminated by the same supervision contract, the execution settles as a normal recoverable failure.

It must not:

- loop into infinite retries,
- remain indefinitely `running`,
- or be rewritten as an interrupted-transition failure.

The resulting run/stage state should follow the standard failure/recovery path.

---

## 8. Truth and Persistence Contract

### 8.1 Persisted supervision-classification design

Proposal 037 does **not** extend `AgentCanonicalOutcome` with new `idle_hang_*` enum cases.

The stable canonical terminal-outcome contract in
[../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md)
remains intact.

Instead, watchdog-specific truth must be persisted in a new dedicated field on `AgentExecution`:

- `supervisionClassification`

Expected values:

- `idle_hang_before_first_progress`
- `idle_hang_after_progress`
- `idle_hang_read_loop`
- `idle_hang_after_first_edit`
- `mutation_side_effect_missing`

This field is nullable and backward-compatible:

- old rows leave it unset,
- non-watchdog executions leave it unset,
- watchdog-driven executions must set it durably before settlement completes.

### 8.2 Canonical outcome pairing

`supervisionClassification` refines, but does not replace, the stable canonical outcome.

For watchdog-triggered failures:

- attempts with no durable output settle with existing canonical failure truth such as `failed_before_output`,
- attempts with durable output already present settle with the existing after-output failure path already used by the repo,
- `providerStopReason` may preserve supporting provider detail,
- but watchdog-specific operator/report wording must come from `supervisionClassification` when it exists.

### 8.3 Reader precedence

Readers must use this precedence order:

1. `AgentExecution.canonicalOutcome`
2. `AgentExecution.supervisionClassification` when canonical outcome is a failure-compatible terminal state
3. `transportErrorKind`
4. `providerStopReason`
5. supporting envelopes / receipts / transcripts

This keeps the stable execution-truth model compatible while making watchdog classifications durable and operator-visible.

### 8.4 Attempt-level truth

The first watchdog fire is attempt-level truth, not immediately run-level blocked truth.

That means:

- the original attempt is failed by supervision,
- a new fresh retry attempt is created automatically,
- the run remains live while that retry is in progress.

### 8.5 Final execution truth after retry exhaustion

Once the automatic retry is exhausted, the durable watchdog-specific truth must remain on
`AgentExecution.supervisionClassification`.

The stable terminal-outcome field remains `canonicalOutcome`; it continues to carry the repo's
existing failure-compatible terminal state.

The watchdog-specific refinement that must survive retry exhaustion is:

- `idle_hang_before_first_progress`
- `idle_hang_after_progress`
- `idle_hang_read_loop`
- `idle_hang_after_first_edit`
- `mutation_side_effect_missing`

The engine must not down-convert this into:

- generic timeout,
- app restart interruption,
- session-closed transition stall,
- or plain missing outputs without supervision context.

### 8.6 Stage-owned recovery truth

When retry exhaustion occurs, the stage-level durable owner remains `StageExecution`.

Required durable effects:

- the failed retry attempt persists its `supervisionClassification`,
- the containing `StageExecution` persists the recovery recommendation in `recoverySnapshotJSON`,
- `RunReportBuilder` and recovery surfaces read stage-owned `recoverySnapshotJSON` only after agent-level execution truth,
- `ResumeManager` must not reinterpret exhausted watchdog retry as app-restart interruption unless separate interruption evidence exists.

`recoverySnapshotJSON` is therefore secondary truth for next-action guidance.
It does not replace or outrank:

- `canonicalOutcome`
- `supervisionClassification`
- `transportErrorKind`
- `providerStopReason`

### 8.7 Receipt fields

Receipts and execution records must preserve at least:

- watchdog classification,
- first prompt timestamp,
- last meaningful progress timestamp,
- first mutating tool timestamp, when present,
- last mutating tool name, when present,
- whether a post-mutation filesystem verification was attempted,
- whether a durable side effect was observed,
- the first verified mutated path or mutation target, when present,
- silence duration at watchdog fire,
- whether automatic retry was consumed,
- whether the final outcome came from the original attempt or the retry.

---

## 9. Reporting, Recovery, and Operator Surfaces

### 9.1 Timeline behavior

The live timeline must make automatic supervision visible.

This proposal does not create a new top-level watchdog-history surface.
Timeline visibility must extend the current run-detail / workflow-map / focused-timeline owner path
described by the stable run-surface references. Watchdog history is subordinate to that existing
shell spine, not a parallel diagnostics lane.

Operator-facing timeline/history should show:

- execution started,
- watchdog classified the execution as hung,
- automatic fresh retry started,
- and whether the retry succeeded or also failed.

The engine must not make the retry appear as a mysterious second session with no explanation.

The durable data path for this history is:

- failed original `AgentExecution` with `supervisionClassification`,
- replacement `AgentExecution` with incremented `agentAttemptNumber` and watchdog-specific `retryReason`,
- optional stage-level `recoverySnapshotJSON` once retry exhaustion occurs.

### 9.2 Run reports

Run reports must use the supervision reason directly.

Expected operator-facing language:

- "No meaningful progress was observed within 120s after prompt submission."
- "Execution stopped making progress for 300s after earlier progress."
- "Execution remained in a read loop for 120s without advancing."
- "Execution stopped making progress for 120s after the first mutating tool boundary."
- "The runtime reported a mutating tool success, but no corresponding filesystem change was observed."

Reports must not collapse these into generic `Execution timed out` when a watchdog classification exists.

`RunReportBuilder` must therefore read:

1. `canonicalOutcome`
2. `supervisionClassification`
3. stage-owned `recoverySnapshotJSON`

before consulting looser transport or interruption narrative.

In that order:

- `canonicalOutcome` answers the stable terminal-state question,
- `supervisionClassification` answers the watchdog-specific "what kind of hang was this?" question,
- `recoverySnapshotJSON` answers the stage-owned "what should the operator do next?" question.

### 9.3 Recovery surfaces

If retry exhaustion leads to failure, recovery UI must treat it as a standard retryable execution failure.

It must not be presented as:

- interrupted transition,
- stale blocked report truth,
- or app restart unless separate evidence actually supports that conclusion.

### 9.4 Superseded behavior remains

If the automatic retry succeeds and the run continues, earlier watchdog-triggered failure surfaces become historical snapshots and may be marked superseded under the existing superseded-report model.

---

## 10. Telemetry and Proof

### 10.1 Required proof paths

Acceptance must require proof for all of the following:

1. **No first progress**
   - `prompt_submitted`
   - no meaningful progress for `120s`
   - automatic fresh retry starts

2. **Progress then silence**
   - at least one strong progress event
   - no later meaningful progress for `300s`
   - automatic fresh retry starts

3. **Read-loop stall**
   - repeated weak progress only
   - no strong progress
   - watchdog fires at `120s`

4. **First-edit silence**
   - discovery churn occurs
   - first mutating tool boundary is observed
   - no later meaningful progress for `120s`
   - watchdog fires as `idle_hang_after_first_edit`

5. **Successful auto-retry recovery**
   - first attempt watchdog-fails
   - fresh retry succeeds
   - run remains live and continues

6. **Retry exhaustion**
   - first attempt watchdog-fails
   - retry watchdog-fails again
   - execution settles as recoverable failure with explicit supervision reason

### 10.2 ACP-wide taxonomy owner

The shared owner for weak/strong/mutating progress taxonomy is the transport-neutral event-normalization layer currently centered on `ExecutionEventBridge`.

Required contract:

- adapters normalize raw provider events into generic tool names,
- the shared classifier maps those names into progress classes before watchdog logic runs,
- watchdog logic consumes progress classes, not provider-specific tool strings.

Initial canonical mapping:

- weak read:
  - `read`
  - `permission:read`
- weak discovery:
  - `search`
  - `execute`
  - `permission:execute`
- strong non-mutating:
  - `text_chunk`
  - non-discovery `tool_call_started`
  - non-discovery `tool_call_finished`
- mutating:
  - `edit`
  - `apply_patch`
  - `write`
  - `permission:edit`

This mapping is intentionally aligned with the current shared classifier in `ExecutionEventBridge` and the watchdog consumers in `RuntimeAgentExecutor`:

- `search`, `execute`, and `permission:execute` remain weak/discovery progress,
- only text output and non-discovery tool activity count as strong progress,
- mutating tools remain their own class because they trigger both `idle_hang_after_first_edit` and side-effect verification.

Future ACP families may add raw names only by extending this shared classifier; they must not introduce family-specific watchdog policy.

### 10.3 Repo-owned proof lane

This proposal chooses one explicit proof lane:

- `proposal-037`

It must be added to both:

- [../reference/test-gates.md](../reference/test-gates.md)
- [../../scripts/test-gate.sh](../../scripts/test-gate.sh)

Initial target coverage for the lane:

- focused `RuntimeAgentExecutorTests` cases for watchdog classification, mutation-side-effect verification, Codex session-history budgets, and fresh-session retry after watchdog trip
- focused `OrchestratorTests` cases for durable stage/agent materialization and same-stage watchdog retry lineage
- focused `ResumeManagerTests` cases for extended-grace reconcile behavior at post-session and post-fanout boundaries
- `RecoveryCoordinatorTests`
- `Proposal013Tests`
- `Proposal019Tests`
- `LiveProposalWorkflowTests`
- `WorkflowMapProjectionTests`
- `RunTimelineInspectorViewTests`

Required coverage owned by `proposal-037`:

- event classification and timestamp tracking
- watchdog firing at `120s / 300s / 120s / 120s`
- fail-closed mutation verification after mutating-tool success with no real worktree delta
- automatic retry creating durable same-stage `AgentExecution` lineage
- retry exhaustion persisting `supervisionClassification` and stage recovery truth
- report/recovery readers preferring supervision truth over generic timeout/interruption wording
- no infinite `running` after ACP silence

---

## 11. Implementation Status and Remaining Slices

Proposal 037 is no longer greenfield. Parts of the substrate already exist in the repository and must be treated as landed baseline, not future intent.

### 11.1 Already-landed substrate

The following pieces are already present and proposal readers must treat them as baseline:

- `AgentExecution.supervisionClassification` exists and is the durable agent-level refinement field,
- the `proposal-037` test gate already exists in:
  - [../reference/test-gates.md](../reference/test-gates.md)
  - [../../scripts/test-gate.sh](../../scripts/test-gate.sh)
- shared weak/strong/mutating taxonomy is already enforced by the normalization layer centered on `ExecutionEventBridge`,
- watchdog-specific proof cases already live in the targeted suites named in §10.3.

### 11.2 Remaining implementation slices

Remaining work should proceed in four slices:

1. **Event supervision hardening**
   - keep the shared progress classifier aligned across `ExecutionEventBridge`, `RuntimeAgentExecutor`, and proof fixtures
   - track meaningful progress timestamps
   - finish the two-phase watchdog and first-edit silence watchdog
   - keep post-mutation filesystem verification fail-closed

2. **Retry machinery**
   - invalidate stale session generation
   - force fresh retry
   - persist watchdog retry through stage-owned `AgentExecution` lineage
   - persist retry-consumed truth

3. **Truth and surfaces**
   - finish receipts, reports, recovery UI, and live timeline markers on top of the already-landed `supervisionClassification` substrate
   - raise the stable watchdog-specific truth contract into:
     - [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md)

4. **Proof and hardening**
   - keep `proposal-037` gate aligned with the real proof lane
   - targeted tests
   - fixture transport scenarios
   - long-run verification
   - first-edit stall fixtures for coding agents
   - mutation-success-without-filesystem-delta fixtures

---

## 12. Acceptance Criteria

This proposal is complete only when all of the following are true:

1. All ACP families use the same supervision contract.
2. ACP execution with no first progress fails at `120s`, not only at `1800s`.
3. ACP execution with early progress then silence fails at `300s`, not only at `1800s`.
4. ACP read-loop churn fails at `120s` under the weak-progress policy.
5. ACP execution that goes silent for `120s` after the first mutating tool boundary fails as `idle_hang_after_first_edit`.
6. ACP execution that reports mutating-tool success without a real filesystem delta fails as `mutation_side_effect_missing`.
7. The first watchdog or mutation-integrity failure triggers exactly one automatic fresh retry.
8. The retry invalidates old session state and creates a new session.
9. Retry exhaustion produces explicit supervision failure truth, not generic timeout.
10. Reports and recovery surfaces show supervision-specific reasons.
11. Successful auto-retry keeps the run alive and does not leave false blocked truth behind.
12. No ACP execution can remain indefinitely in `running` after meaningful progress has stopped or after a false mutating-tool success with no durable side effect.

---

## 13. Decision

Proposal 037 chooses one deterministic supervision model:

- ACP execution liveness is defined by meaningful stream progress,
- fixed deadlines are `120s / 300s / 120s`,
- an additional `120s` deadline applies after the first mutating tool boundary,
- mutating-tool success must be verified against real filesystem side effects within `30s`,
- every ACP family is treated the same,
- one automatic fresh retry is allowed,
- and supervision truth must remain explicit all the way through receipts, reports, and operator recovery.

This proposal intentionally prefers deterministic operator-visible behavior over process-level guesswork.
