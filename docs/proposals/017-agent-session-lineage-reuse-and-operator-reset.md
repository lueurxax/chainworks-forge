# Proposal 017: Agent Session Lineage Reuse and Operator Reset

| Field | Value |
|---|---|
| Date | 2026-03-29 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md](016-transport-outcome-truth-stage-settlement-and-resume-idempotency.md), [015-skill-resolution-and-runtime-injection.md](015-skill-resolution-and-runtime-injection.md), [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [reference/current-system-baseline.md](../reference/current-system-baseline.md) |
| Scope | Reusable provider session lineage per agent within a run, session ownership and invalidation rules, persisted session binding truth, operator-triggered session reset, and app/runtime surfaces that make reuse inspectable and safe |
| Goal | Stop paying full cold-start token cost every time the same agent is invoked within one run by reusing a valid provider session when safe, while giving the operator an explicit way to reset that session when context has drifted or the conversation is no longer trustworthy. |

---

## 1. Context

The current runtime still inherits the Proposal 004 / ARCH-027 rule:

- one live session per `AgentExecution`
- no session reuse across agents
- no session reuse across iterations
- no dependence on hidden session memory

That rule was correct for the first live slice because it minimized ambiguity.
It is now too expensive.

In the current host system:

1. the same agent can be invoked repeatedly in one run,
2. the app reconstructs context from artifacts each time,
3. every retry / repeated stage entry creates a brand-new provider session,
4. token usage grows unnecessarily because the agent must rebuild local conversational state from scratch.

This is especially painful in proposal-loop and refinement-heavy runs where:

- `proposal_writer` is called multiple times,
- aggregate review may be retried,
- the same reviewer may be retried after contract failure,
- interrupted runs resume into logically continuous work but still create fresh provider sessions.

The product now needs a middle layer between:

- **unsafe hidden conversational carry-over**, and
- **wasteful cold-start on every attempt**.

Proposal 017 introduces that layer as **session lineage reuse**.

### 1.1 What this proposal changes

Proposal 017 intentionally revises the earlier ARCH-027 statement in one bounded way:

- we still do **not** reuse sessions across different agents,
- we still do **not** reuse sessions across different runs,
- but we **do** allow reuse of a provider session for the same agent lineage within the same run when the binding and workspace contract are still compatible.

### 1.2 Why this is a separate proposal

This is not the same problem as:

- transport outcome truth,
- stage settlement truth,
- contract alignment,
- or report correctness.

Those are execution-truth and recovery problems.

Proposal 017 is about **conversation continuity and token efficiency**:

- how a session is owned,
- when it can be safely reused,
- when it must be invalidated,
- and how the operator resets it explicitly.

### 1.3 What this proposal is not

Proposal 017 is **not**:

- cross-run memory,
- provider-specific long-term chat persistence outside a run,
- session sharing across different agents,
- a general prompt-cache proposal,
- a provider-routing redesign,
- or a replacement for persisted artifacts as the canonical source of truth.

Artifacts, receipts, and canonical outcomes remain the durable truth.
Session reuse is an execution optimization and continuity aid, not the authoritative record.

---

## 2. Product questions this proposal must answer

After Proposal 017, the engineer must be able to answer all of these with code truth:

1. When the same agent is invoked again in the same run, do we reuse a compatible existing provider session instead of always creating a new one?
2. What exact conditions make a session reusable versus invalid?
3. Can the operator explicitly reset the session for one agent without cloning the whole run?
4. If a session is reset, does the next invocation create a fresh session deterministically?
5. Can reports and operator surfaces show whether an execution used a reused session, a fresh session, or a manually reset session?
6. Does retrying the same agent reuse its session lineage when safe, rather than starting from a cold session every time?
7. Do clone-run boundaries still create fresh session lineages so session memory never leaks across runs?

Proposal 017 is done only when all seven answers are yes with persisted evidence and test coverage.

---

## 3. What we build

Proposal 017 delivers four tightly coupled layers.

### Layer A: Session Lineage Ownership

| Component | Responsibility |
|---|---|
| **AgentSessionLineage** | Persisted ownership record for one reusable provider session lineage inside one run for one agent |
| **SessionReusePolicy** | Pure decision layer that answers whether an existing session lineage is reusable for the next invocation |
| **SessionBindingFingerprint** | Stable compatibility fingerprint over provider/model/working-directory/policy/skill injection context |
| **SessionReuseDecision** | Typed decision: reuse existing session, create fresh session, or require explicit reset |

### Layer B: Runtime Reuse and Invalidation

| Component | Responsibility |
|---|---|
| **GooseSessionReuseBridge** | Extends `GooseSessionBridge` to reuse a valid session instead of always calling `createSession` |
| **SessionInvalidationRules** | Invalidates reuse when binding, workspace, or safety context changes, or when runtime marks the session unusable |
| **SessionTerminationRecorder** | Persists whether the prior invocation closed the session, left it alive, or marked it unusable |
| **SessionResetCoordinator** | Closes and retires one session lineage when the operator requests reset |

### Layer C: Operator and App Surfaces

| Component | Responsibility |
|---|---|
| **AgentSessionInspector** | Shows current session lineage, generation, provider session ID, reuse count, reset history, and invalidation reason |
| **Reset Agent Session Action** | Operator action in the app that resets one agent’s reusable session lineage |
| **SessionReuseBadge** | Shows whether the latest execution was `fresh`, `reused`, `reused_after_resume`, or `fresh_after_reset` |

### Layer D: Provenance and Reporting

| Component | Responsibility |
|---|---|
| **SessionLineageReportBridge** | Makes run reports and blocked-run views show session reuse truth explicitly |
| **SessionReuseReceiptFields** | Extends receipts and execution metadata with session lineage provenance |
| **SessionResetAuditTrail** | Persists operator-triggered session reset history for later debugging |

---

## 4. Core execution model

### 4.1 Reuse boundary

Proposal 017 chooses this exact reuse boundary:

- reuse is allowed only for the **same `Run`**
- reuse is allowed only for the **same `agentID`**
- reuse is allowed only when the **session binding fingerprint still matches**
- reuse is allowed across repeated invocations, retries, and later stage entries for that same agent within the run
- reuse is **not** allowed across different runs, even if the idea and workflow are the same

This means the unit of reuse is:

> one logical agent-session lineage inside one run

### 4.2 Reuse does not replace durable truth

Even when a provider session is reused:

- inputs still remain explicit,
- artifacts still remain canonical,
- receipts still remain persisted,
- execution truth still remains on `AgentExecution` and `StageExecution`,
- reports must still be reconstructable without needing live provider memory.

Session reuse is additive, not authoritative.

### 4.3 Session lineage rules

Each reusable lineage is identified by:

- `runID`
- `agentID`
- `lineageID`
- `generation`

Where:

- `lineageID` stays stable while the operator has not reset the session
- `generation` increments whenever the lineage is explicitly reset or invalidated into a fresh session

Rules:

1. Same-run repeated invocation of the same agent reuses the current lineage when compatible.
2. Same-run retry of the same agent reuses the same lineage when compatible.
3. Startup resume may reuse the lineage if the prior session is still valid and the binding fingerprint still matches.
4. Clone run always creates a new lineage.
5. Operator reset always creates a new generation and forces a fresh session on the next invocation.

---

## 5. Persisted model changes

### 5.1 New persisted model

Proposal 017 adds a new persisted model:

| Model | Field | Purpose |
|---|---|---|
| `AgentSessionLineage` | `id` | Primary key |
| `AgentSessionLineage` | `runID` | Owner run |
| `AgentSessionLineage` | `agentID` | Owner agent |
| `AgentSessionLineage` | `lineageID` | Stable logical session lineage key |
| `AgentSessionLineage` | `generation` | Monotonic generation after reset/invalidation |
| `AgentSessionLineage` | `providerSessionID` | Current live provider session ID |
| `AgentSessionLineage` | `bindingFingerprint` | Compatibility fingerprint for safe reuse |
| `AgentSessionLineage` | `workingDirectory` | Last approved working directory |
| `AgentSessionLineage` | `workspaceMode` | `read_only` / `read_write` |
| `AgentSessionLineage` | `runtimeProvider` | Actual provider family used by this lineage |
| `AgentSessionLineage` | `runtimeModel` | Actual model used by this lineage |
| `AgentSessionLineage` | `status` | `active`, `invalidated`, `closed`, `reset` |
| `AgentSessionLineage` | `reuseCount` | Number of executions that reused this lineage |
| `AgentSessionLineage` | `lastUsedAt` | Last successful reuse timestamp |
| `AgentSessionLineage` | `invalidatedAt` | When the lineage stopped being reusable |
| `AgentSessionLineage` | `invalidationReason` | Why it stopped being reusable |
| `AgentSessionLineage` | `resetByOperatorAt` | Explicit operator reset timestamp |

### 5.2 `AgentExecution` additions

`AgentExecution` gains:

| Field | Purpose |
|---|---|
| `sessionLineageID` | Links the execution to the reused or fresh session lineage |
| `sessionGeneration` | Records which lineage generation this execution used |
| `sessionReuseDisposition` | `fresh`, `reused`, `reused_after_resume`, `fresh_after_reset`, `fresh_after_invalidation` |
| `sessionResetReason` | Optional human/runtime explanation when a fresh session was forced |

### 5.3 `Run` additions

`Run` gains:

| Field | Purpose |
|---|---|
| `sessionResetAuditJSON` | Persisted audit history of per-agent resets during the run |

---

## 6. Binding compatibility and invalidation

### 6.1 Session binding fingerprint

A session may be reused only when this fingerprint still matches:

- `agentID`
- resolved provider family
- resolved model
- resolved effort
- permission profile
- workspace mode (`read_only` vs `read_write`)
- effective working directory
- skill snapshot hash / runtime injected skill content hash
- relevant system prompt framing version

If any one of those changes, reuse is invalid.

### 6.2 Mandatory invalidation cases

The next invocation must create a fresh session when:

1. provider family changed
2. model changed
3. permission profile changed
4. workspace mode changed
5. working directory changed
6. skill content or role-injected prompt framing changed
7. provider session was explicitly closed
8. provider reported the session as expired / invalid / missing
9. operator pressed `Reset Session`
10. clone run created a new run boundary

### 6.3 Non-default invalidation

By default, these do **not** force reset on their own:

- new input artifacts
- new stage ID
- new iteration
- retry of the same agent

The point of reuse is to carry local conversational continuity through those repeated invocations.

---

## 7. Operator reset

### 7.1 Required app behavior

The app must expose **Reset Session** for a specific agent.

Minimum surfaces:

- blocked run recovery surface
- run detail / agent inspector surface

### 7.2 Reset semantics

When the operator resets an agent session:

1. the current provider session is closed if still live,
2. the current `AgentSessionLineage` is marked `reset`,
3. `generation` increments,
4. the next invocation of that agent creates a fresh provider session,
5. the reset is recorded in run-level audit history,
6. no other agent sessions are affected.

### 7.3 What reset is not

Reset Session is **not**:

- clone run,
- retry stage,
- retry aggregate step,
- or approval resolution.

It only resets conversational continuity for one agent lineage.

---

## 8. Runtime execution rules

### 8.1 Invocation flow

For each live agent invocation:

1. compute the session binding fingerprint
2. look up the current `AgentSessionLineage` for `(runID, agentID)`
3. if there is no active lineage, create one and create a fresh provider session
4. if there is an active lineage and fingerprint matches, reuse its provider session
5. if there is an active lineage and fingerprint mismatches, invalidate it and create a fresh generation
6. persist the `sessionReuseDisposition` on the resulting `AgentExecution`

### 8.2 Resume behavior

On app relaunch / resume:

- the app does **not** assume session reuse blindly
- it consults the stored lineage and fingerprint
- if the provider session is gone or unverifiable, the lineage becomes `invalidated`
- the next invocation gets `fresh_after_invalidation`

This keeps reuse opportunistic but never magical.

### 8.3 Failure behavior

By default:

- ordinary canonical failures do **not** automatically reset the session lineage
- provider session invalid / expired / missing does reset reuse eligibility
- operator can always override with Reset Session

This keeps retries cheap when the conversation is still useful, but does not hide provider-side session loss.

---

## 9. Reporting and operator visibility

Reports and operator surfaces must show:

- whether the execution used a fresh or reused session
- the current session lineage ID
- the generation
- the provider session ID when available
- whether the lineage was invalidated or manually reset
- why a fresh session was forced

Run reports must never make session reuse invisible.
It must be possible to explain:

- why tokens were saved,
- why a fresh session was forced,
- and whether the operator manually reset the lineage.

---

## 10. Acceptance criteria

Proposal 017 is complete only when all of the following are true.

### AC-1 Same-agent same-run reuse

When the same agent is invoked again in the same run with the same binding fingerprint, the runtime reuses the existing provider session instead of always creating a new session.

### AC-2 Retry reuse

Retrying the same agent in the same run reuses the existing session lineage when compatible.

### AC-3 Explicit reset

The operator can reset the session for one agent in the app, and the next invocation creates a fresh provider session.

### AC-4 No cross-run leakage

Clone run or new run creation always creates a fresh session lineage, even when the same agent is used again.

### AC-5 Compatibility invalidation

If provider/model/permission/working-directory/skill injection context changes, reuse is rejected and a fresh session is created.

### AC-6 Visibility

Run reports and operator surfaces show fresh versus reused session truth and session reset history.

### AC-7 Durable provenance

Each `AgentExecution` persists enough session lineage provenance that report/debug surfaces do not need to infer reuse heuristically.

---

## 11. Verification

Implementation must include all of:

1. unit tests for `SessionReusePolicy`
2. unit tests for binding fingerprint invalidation
3. unit tests for operator reset semantics
4. integration tests proving same-agent retry reuses session when compatible
5. integration tests proving clone run creates a fresh lineage
6. integration tests proving invalid provider session forces fresh generation
7. report/read-model tests proving session reuse truth is visible

Required motivating proofs:

- repeated `proposal_writer` invocations in one run do not create a new provider session each time
- retrying `proposal_reviewer_architect` after contract failure can reuse the same session lineage when safe
- operator reset forces the next invocation of that reviewer to create a new session

---

## 12. Non-goals and guardrails

Proposal 017 must **not**:

- reuse sessions across different agents
- reuse sessions across different runs
- make provider session memory the canonical source of truth
- hide reset/invalidation decisions from the operator
- silently ignore binding drift

This proposal should reduce token burn and improve conversational continuity, but it must stay bounded and inspectable.

---

## 13. Implementation plan shape

The intended implementation order is:

1. persisted `AgentSessionLineage` model and `AgentExecution` lineage fields
2. `SessionReusePolicy` and `SessionBindingFingerprint`
3. `GooseSessionBridge` reuse path
4. invalidation rules
5. operator `Reset Session`
6. report/operator visibility
7. motivating regression tests on repeated same-agent invocation

Phase 1 of implementation should prove token-saving reuse on the proposal loop before broadening to other workflows.
