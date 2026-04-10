# Proposal 037: ACP Execution Supervision and Idle-Hang Watchdog

| Field | Value |
|---|---|
| Date | 2026-04-10 |
| Status | Draft |
| Author | Codex |
| Depends on | [030-acp-second-wave-runtime-profiles-codex-auggie-junie.md](030-acp-second-wave-runtime-profiles-codex-auggie-junie.md), [035-atomic-transition-settlement-and-durable-resume-cursor.md](035-atomic-transition-settlement-and-durable-resume-cursor.md), [../reference/runtime-contract.md](../reference/runtime-contract.md), [../reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md) |
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
   - and read-loop churn without advancement?
5. Can all ACP families use the same supervision model and the same retry policy?

---

## 3. Scope

This proposal includes:

- one ACP-wide supervision contract based on `RuntimeStreamEvent`,
- explicit definitions of `meaningful progress`,
- watchdog deadlines for first progress, idle-after-progress, and weak read-loop progress,
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

### 5.3 Explicit non-goals

The supervision contract must not attempt to infer "the model is still thinking" from:

- lack of events plus a live process,
- intermittent OS-level activity,
- process memory growth,
- or external network sockets.

If the runtime does not emit meaningful progress, the execution is not healthy enough to remain indefinitely in `running`.

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

---

## 7. Automatic Recovery Policy

### 7.1 Default retry policy

For all three watchdog classifications:

- `idle_hang_before_first_progress`
- `idle_hang_after_progress`
- `idle_hang_read_loop`

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

### 7.3 Retry exhaustion

If the fresh retry is also terminated by the same supervision contract, the execution settles as a normal recoverable failure.

It must not:

- loop into infinite retries,
- remain indefinitely `running`,
- or be rewritten as an interrupted-transition failure.

The resulting run/stage state should follow the standard failure/recovery path.

---

## 8. Truth and Persistence Contract

### 8.1 Attempt-level truth

The first watchdog fire is attempt-level truth, not immediately run-level blocked truth.

That means:

- the original attempt is failed by supervision,
- a new fresh retry attempt is created automatically,
- the run remains live while that retry is in progress.

### 8.2 Final execution truth after retry exhaustion

Once the automatic retry is exhausted, the canonical failure reason must remain the supervision reason:

- `idle_hang_before_first_progress`
- `idle_hang_after_progress`
- `idle_hang_read_loop`

The engine must not down-convert this into:

- generic timeout,
- app restart interruption,
- session-closed transition stall,
- or plain missing outputs without supervision context.

### 8.3 Receipt fields

Receipts and execution records must preserve at least:

- watchdog classification,
- first prompt timestamp,
- last meaningful progress timestamp,
- silence duration at watchdog fire,
- whether automatic retry was consumed,
- whether the final outcome came from the original attempt or the retry.

---

## 9. Reporting, Recovery, and Operator Surfaces

### 9.1 Timeline behavior

The live timeline must make automatic supervision visible.

Operator-facing timeline/history should show:

- execution started,
- watchdog classified the execution as hung,
- automatic fresh retry started,
- and whether the retry succeeded or also failed.

The engine must not make the retry appear as a mysterious second session with no explanation.

### 9.2 Run reports

Run reports must use the supervision reason directly.

Expected operator-facing language:

- "No meaningful progress was observed within 120s after prompt submission."
- "Execution stopped making progress for 300s after earlier progress."
- "Execution remained in a read loop for 120s without advancing."

Reports must not collapse these into generic `Execution timed out` when a watchdog classification exists.

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

4. **Successful auto-retry recovery**
   - first attempt watchdog-fails
   - fresh retry succeeds
   - run remains live and continues

5. **Retry exhaustion**
   - first attempt watchdog-fails
   - retry watchdog-fails again
   - execution settles as recoverable failure with explicit supervision reason

### 10.2 Proposal gate sketch

The implementation gate for this proposal should require targeted tests that prove:

- event classification,
- timestamp tracking,
- watchdog firing at the documented thresholds,
- fresh retry invalidating old session state,
- receipts/reporting using supervision truth,
- and no infinite `running` after ACP silence.

The exact gate name and test target list may be finalized during implementation planning.

---

## 11. Implementation Slices

The implementation should likely proceed in four slices:

1. **Event supervision core**
   - define event classes
   - track meaningful progress timestamps
   - implement two-phase watchdog

2. **Retry machinery**
   - invalidate stale session generation
   - force fresh retry
   - persist retry-consumed truth

3. **Truth and surfaces**
   - receipts
   - reports
   - recovery UI
   - live timeline markers

4. **Proof and hardening**
   - targeted tests
   - fixture transport scenarios
   - long-run verification

---

## 12. Acceptance Criteria

This proposal is complete only when all of the following are true:

1. All ACP families use the same supervision contract.
2. ACP execution with no first progress fails at `120s`, not only at `1800s`.
3. ACP execution with early progress then silence fails at `300s`, not only at `1800s`.
4. ACP read-loop churn fails at `120s` under the weak-progress policy.
5. The first watchdog failure triggers exactly one automatic fresh retry.
6. The retry invalidates old session state and creates a new session.
7. Retry exhaustion produces explicit supervision failure truth, not generic timeout.
8. Reports and recovery surfaces show supervision-specific reasons.
9. Successful auto-retry keeps the run alive and does not leave false blocked truth behind.
10. No ACP execution can remain indefinitely in `running` after meaningful progress has stopped.

---

## 13. Decision

Proposal 037 chooses one deterministic supervision model:

- ACP execution liveness is defined by meaningful stream progress,
- fixed deadlines are `120s / 300s / 120s`,
- every ACP family is treated the same,
- one automatic fresh retry is allowed,
- and supervision truth must remain explicit all the way through receipts, reports, and operator recovery.

This proposal intentionally prefers deterministic operator-visible behavior over process-level guesswork.
