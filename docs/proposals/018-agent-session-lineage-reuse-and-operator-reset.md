# Proposal 018: Agent Session Lineage Reuse and Operator Reset

| Field | Value |
|---|---|
| Date | 2026-03-29 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [reference/execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md), [015-skill-resolution-and-runtime-injection.md](015-skill-resolution-and-runtime-injection.md), [reference/runtime-contract.md](../reference/runtime-contract.md), [reference/live-provider-execution-slice.md](../reference/live-provider-execution-slice.md), [reference/current-system-baseline.md](../reference/current-system-baseline.md) |
| Scope | Reusable provider session lineage per agent within a run, scope-controlled reuse (`same_invocation_owner` and opt-in `same_agent_family_within_run`), budget-driven invalidation and compaction, persisted session binding truth, operator-triggered session reset, checkpoint artifacts for fresh rehydration, and app/runtime surfaces that make reuse inspectable and safe |
| Goal | Reduce both cold-start tax and runaway reused-session burn for repeated same-agent work within one run by reusing a valid provider session when safe, compacting or resetting it when reuse stops paying for itself, and rehydrating fresh generations from durable checkpoint artifacts rather than opaque chat history. |

---

## 1. Context

The current runtime still inherits the Proposal 004 / ARCH-027 rule:

- one live session per `AgentExecution`
- no session reuse across agents
- no session reuse across iterations
- no dependence on hidden session memory

That rule was correct for the first live slice because it minimized ambiguity.
It is now too expensive.

The burn problem is not only cold-start.
If a reused session keeps dragging an ever-growing transcript, the next invocation can become more expensive than a fresh session unless the runtime has explicit budget and compaction rules.

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

Proposal 018 introduces that layer as **session lineage reuse**.

### 1.1 What this proposal changes

Proposal 018 intentionally revises the earlier ARCH-027 statement in one bounded way:

- we still do **not** reuse sessions across different agents,
- we still do **not** reuse sessions across different runs,
- we **do** allow reuse of a provider session for the same agent lineage within the same run when the binding and workspace contract are still compatible,
- and we add explicit budget/compaction rules so reuse is allowed only while it stays economically useful, not merely technically possible.

### 1.2 Why this is a separate proposal

This is not the same problem as:

- transport outcome truth,
- stage settlement truth,
- contract alignment,
- or report correctness.

Those are execution-truth and recovery problems.

Proposal 018 is about **conversation continuity and token efficiency**:

- how a session is owned,
- when it can be safely reused,
- when it should be compacted or invalidated because burn is no longer acceptable,
- when it must be invalidated,
- and how the operator resets it explicitly.

### 1.3 What this proposal is not

Proposal 018 is **not**:

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

After Proposal 018, the engineer must be able to answer all of these with code truth:

1. When the same agent is invoked again in the same run, do we reuse a compatible existing provider session instead of always creating a new one?
2. What exact conditions make a session reusable versus invalid?
3. Can the operator explicitly reset the session for one agent without cloning the whole run?
4. If a session is reset, does the next invocation create a fresh session deterministically?
5. Can reports and operator surfaces show whether an execution used a reused session, a fresh session, or a manually reset session?
6. Does retrying the same agent reuse its session lineage when safe, rather than starting from a cold session every time?
7. Can adjacent same-agent proposal-loop steps opt into shared family reuse inside one run without opening cross-agent or cross-run memory?
8. When a reused session exceeds budget, does the runtime compact or invalidate it instead of dragging an ever-growing transcript forever?
9. Do clone-run boundaries still create fresh session lineages so session memory never leaks across runs?

Proposal 018 is done only when all nine answers are yes with persisted evidence and test coverage.

---

## 3. What we build

Proposal 018 delivers four tightly coupled layers.

### Layer A: Session Lineage Ownership

| Component | Responsibility |
|---|---|
| **AgentSessionLineage** | Persisted ownership record for one reusable provider session lineage inside one run for one agent |
| **SessionReusePolicy** | Pure decision layer that answers whether an existing session lineage is reusable for the next invocation |
| **SessionReuseScope** | Explicit reuse scope: `none`, `same_invocation_owner`, or opt-in `same_agent_family_within_run` |
| **SessionFamilyID** | Stable family key for adjacent same-agent work that should share continuity inside one run |
| **SessionBindingFingerprint** | Stable compatibility fingerprint over provider/model/working-directory/policy/skill injection context |
| **SessionReuseDecision** | Typed decision: reuse existing session, create fresh session, or require explicit reset |

### Layer B: Runtime Reuse and Invalidation

| Component | Responsibility |
|---|---|
| **GooseSessionReuseBridge** | Extends `GooseSessionBridge` to reuse a valid session instead of always calling `createSession` |
| **ContextBudgetGuard** | Invalidates or compacts a generation when accumulated history is no longer cost-effective |
| **SessionCompactionPolicy** | Chooses whether to keep reusing, create a checkpoint, or force a fresh generation |
| **AgentSessionCheckpointBuilder** | Emits a distilled checkpoint artifact before budget-driven refresh or explicit reset |
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

Proposal 018 chooses this exact reuse boundary:

- reuse is allowed only for the **same `Run`**
- reuse is allowed only for the **same `agentID`**
- reuse is allowed only when the **session binding fingerprint still matches**
- reuse is never allowed across different agents or different runs
- reuse scope is explicit, not inferred
- reuse is **not** allowed across different runs, even if the idea and workflow are the same

Proposal 018 supports three explicit reuse scopes:

- `none`
- `same_invocation_owner`
- `same_agent_family_within_run`

`same_invocation_owner` is the default safe baseline.
`same_agent_family_within_run` is an explicit opt-in for adjacent same-agent work inside one run where the product wants continuity that is wider than one invocation owner but still narrower than whole-run memory.

This means the default unit of reuse is:

> one logical agent-session lineage inside one run for one immutable invocation owner

And the wider, opt-in unit of reuse is:

> one logical agent-session lineage inside one run for one agent plus one explicit `sessionFamilyID`

The invocation owner must be persisted, not inferred from current UI state.
Proposal 018 introduces `invocationOwnerKey` as the canonical ownership key for reuse decisions.

`invocationOwnerKey` is the immutable tuple:

- `runID`
- `agentID`
- `stageLineageID`
- `taskName`
- `ownerExecutionLineageID`

Where:

- `stageLineageID` is the canonical stage lineage from execution truth
- `taskName` prevents one agent from reusing a session across unrelated responsibilities inside the same run
- `ownerExecutionLineageID` ties reuse to one recovery branch and prevents reuse from silently hopping across clone/retry/reset branches

### 4.1.1 Authority relation for `ownerExecutionLineageID`

`ownerExecutionLineageID` is not owned by the session layer.

Its authority comes from the execution-truth substrate already defined in [execution-truth-and-recovery.md](../reference/execution-truth-and-recovery.md):

- canonical `AgentExecution`
- canonical `StageExecution.lineageID`
- canonical recovery / retry branch truth

Proposal 018 must therefore treat `ownerExecutionLineageID` as a read-only imported authority.

Session reuse code is allowed to:

- read it,
- persist it on `invocationOwnerKey`,
- compare it during reuse decisions,
- surface it in operator inspection and reports

Session reuse code is not allowed to:

- mint a new `ownerExecutionLineageID`,
- repair or rewrite an existing execution lineage,
- infer one from session events alone,
- or treat session history as the source of truth for execution-branch identity

If execution truth does not provide a trustworthy `ownerExecutionLineageID`, Proposal 018 must fail closed to `fresh_session_required` or `unverifiable_session_history`, not synthesize a substitute lineage authority.

Reuse is therefore not automatically "same agent called again in the same run."
By default it is "same invocation owner called again in the same run."

The only wider mode allowed in this proposal is explicit `same_agent_family_within_run`, where:

- `runID`, `agentID`, `sessionFamilyID`, and binding fingerprint must still match,
- the workflow must opt in deliberately,
- and security/reviewer/audit-style agents remain `same_invocation_owner` or `none` unless a later proposal says otherwise.

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
- `invocationOwnerKey`
- `sessionReuseScope`
- `sessionFamilyID`

Where:

- `lineageID` stays stable while the operator has not reset the session
- `generation` increments whenever the lineage is explicitly reset or invalidated into a fresh session
- `invocationOwnerKey` is immutable for the life of one reusable lineage generation
- `sessionReuseScope` is explicit and immutable for the life of one generation
- `sessionFamilyID` is set only when the scope is `same_agent_family_within_run`

Rules:

1. `none` always creates a fresh session.
2. `same_invocation_owner` reuses the current lineage only when the `invocationOwnerKey` still matches.
3. `same_agent_family_within_run` allows reuse across different invocation owners only when `runID`, `agentID`, `sessionFamilyID`, and binding fingerprint still match.
4. Same-run retry of the same agent may reuse the lineage when the chosen scope still authorizes reuse for that retry branch.
5. Startup resume may reuse the lineage only if the prior session is still valid, the binding fingerprint still matches, and the chosen scope still authorizes reuse.
6. Clone run always creates a new lineage.
7. Operator reset always creates a new generation and forces a fresh session on the next invocation.
8. Any change in binding fingerprint always forces a fresh session even if scope still matches.

---

## 5. Persisted model changes

### 5.1 Persisted ownership model

Proposal 018 must avoid mutable-row ambiguity.
One mutable `AgentSessionLineage` row is not enough because reset/invalidation/reuse would overwrite history and make later inspection/reporting ambiguous.

Proposal 018 therefore splits session persistence into:

- one stable owner record
- one immutable generation record per fresh session epoch
- one append-only event stream for reset/invalidation/close/reuse history

This is the minimum structure that keeps lineage history inspectable and audit-safe.

### 5.2 New persisted models

| Model | Field | Purpose |
|---|---|---|
| `AgentSessionLineage` | `id` | Primary key |
| `AgentSessionLineage` | `runID` | Owner run |
| `AgentSessionLineage` | `agentID` | Owner agent |
| `AgentSessionLineage` | `lineageID` | Stable logical lineage key |
| `AgentSessionLineage` | `invocationOwnerKey` | Immutable ownership key for safe reuse |
| `AgentSessionLineage` | `sessionReuseScope` | `none`, `same_invocation_owner`, `same_agent_family_within_run` |
| `AgentSessionLineage` | `sessionFamilyID` | Optional family key for wider same-agent reuse inside one run |
| `AgentSessionLineage` | `activeGenerationID` | Pointer to the current generation |
| `AgentSessionLineage` | `createdAt` | Creation timestamp |
| `AgentSessionLineage` | `closedAt` | Final closure timestamp when the lineage is retired completely |
| `AgentSessionGeneration` | `id` | Primary key |
| `AgentSessionGeneration` | `lineageID` | Parent lineage |
| `AgentSessionGeneration` | `generation` | Monotonic generation number |
| `AgentSessionGeneration` | `providerSessionID` | Live provider session ID for this generation |
| `AgentSessionGeneration` | `bindingFingerprint` | Compatibility fingerprint for this generation |
| `AgentSessionGeneration` | `workingDirectory` | Approved working directory |
| `AgentSessionGeneration` | `workspaceMode` | `read_only` / `read_write` |
| `AgentSessionGeneration` | `runtimeProvider` | Actual provider family |
| `AgentSessionGeneration` | `runtimeModel` | Actual model |
| `AgentSessionGeneration` | `status` | `active`, `invalidated`, `closed`, `reset` |
| `AgentSessionGeneration` | `turnCount` | Number of invocations attached to this generation |
| `AgentSessionGeneration` | `estimatedInputTokens` | Latest estimated input-token load for the active prompt |
| `AgentSessionGeneration` | `cumulativePromptTokens` | Total prompt tokens accumulated by this generation |
| `AgentSessionGeneration` | `cumulativeCostCents` | Total estimated cost accumulated by this generation |
| `AgentSessionGeneration` | `lastCheckpointArtifactID` | Latest checkpoint artifact emitted for compaction or reset |
| `AgentSessionGeneration` | `createdAt` | Generation start timestamp |
| `AgentSessionGeneration` | `endedAt` | Generation end timestamp |
| `AgentSessionGeneration` | `endReason` | Why this generation stopped being reusable |
| `AgentSessionEvent` | `id` | Primary key |
| `AgentSessionEvent` | `lineageID` | Parent lineage |
| `AgentSessionEvent` | `generationID` | Generation active at event time |
| `AgentSessionEvent` | `eventType` | `created`, `reused`, `invalidated`, `closed`, `operator_reset`, `resume_reused`, `resume_rejected`, `checkpoint_created`, `budget_exceeded`, `compacted` |
| `AgentSessionEvent` | `recordedAt` | Append-only event timestamp |
| `AgentSessionEvent` | `detailsJSON` | Supporting structured details |

`AgentSessionLineage` is allowed to move its active pointer.
`AgentSessionGeneration` and `AgentSessionEvent` are immutable after insert.

### 5.3 `AgentExecution` additions

`AgentExecution` gains:

| Field | Purpose |
|---|---|
| `sessionLineageID` | Links the execution to the reused or fresh session lineage |
| `sessionGenerationID` | Records which immutable generation this execution used |
| `invocationOwnerKey` | Records the exact owner tuple that authorized reuse or forced freshness |
| `sessionReuseScope` | Effective scope used for this invocation |
| `sessionFamilyID` | Optional family key used when scope widens beyond one invocation owner |
| `sessionReuseDisposition` | `fresh`, `reused`, `reused_after_resume`, `fresh_after_reset`, `fresh_after_invalidation`, `fresh_after_budget`, `fresh_after_compaction` |
| `sessionResetReason` | Optional human/runtime explanation when a fresh session was forced |

### 5.4 `Run` additions

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
- static task/instruction scaffold
- system prompt framing
- tool inventory and tool configuration
- permission profile
- workspace mode (`read_only` vs `read_write`)
- effective working directory
- skill snapshot hash / runtime injected skill content hash
- relevant system prompt framing version

If any one of those changes, reuse is invalid.

### 6.2 Reuse scope and family policy

Proposal 018 makes wider reuse opt-in and explicit.

Allowed values:

- `none`
- `same_invocation_owner`
- `same_agent_family_within_run`

Rules:

1. `same_invocation_owner` is the repository default.
2. `same_agent_family_within_run` is allowed only when the workflow/catalog explicitly names a `sessionFamilyID`.
3. `same_agent_family_within_run` is intended for bounded same-agent continuity such as proposal authoring or aggregation inside one run.
4. reviewer, security, audit, and other trust-sensitive agents stay at `same_invocation_owner` or `none` in this proposal unless explicitly justified.
5. this proposal still forbids cross-agent and cross-run reuse.
6. `sessionFamilyID` alone is never sufficient to authorize reuse.
7. family reuse must fail closed whenever the reusable static prefix implied by task framing, system instructions, tool contract, workspace policy, skill injection, or binding inputs no longer matches in practice.
8. if the current invocation contract and the carried session context disagree, the current invocation contract wins and the decision degrades to fresh generation.

### 6.3 ContextBudgetGuard and SessionCompactionPolicy

Reuse is not automatically a savings win.
Proposal 018 therefore adds an explicit budget guard over every active generation.

Minimum tracked thresholds:

- max estimated input tokens per active session generation
- max turns per generation
- max cumulative prompt tokens
- max cumulative cost
- max idle age
- max transcript size

These caps are guardrails, not the primary decision authority.
`ContextBudgetGuard` must be driven by measured reuse economics:

- cached-token share or equivalent provider cache-hit signal,
- effective prompt size at the current turn,
- compaction / truncation churn,
- cumulative prompt tokens and cumulative cost,
- normalized savings versus a fresh-baseline estimate.

Transcript size or turn count alone is not enough to justify keep-reuse versus refresh.

If the measured signals show that continued reuse is lower value than checkpoint-plus-fresh, the active generation must not continue silently dragging history forward forever.
The policy result becomes one of:

- continue reuse,
- create checkpoint and compact into a fresh generation,
- invalidate into `fresh_after_budget`,
- require operator review before any further reuse.

### 6.4 AgentSessionCheckpoint artifact

Before forced reset or budget-driven invalidation, the runtime should emit a canonical checkpoint artifact for fresh rehydration.

Minimum checkpoint contents:

- short machine summary,
- explicit next steps,
- learned constraints / durable learnings,
- unresolved blockers and open decisions,
- open questions / unresolved constraints,
- selected artifact references,
- last validated aggregate state,
- owner and binding context needed for deterministic rehydration,
- scope / family context when reuse was wider than one invocation owner,
- optional compacted conversation state when it is safe and provider-agnostic.

This keeps continuity anchored in durable artifacts instead of hidden provider memory.
Fresh generations should prefer rehydration from the checkpoint artifact plus canonical run artifacts, not from replaying a long raw transcript.
A checkpoint is therefore not just a recap.
It is a continuation artifact that preserves enough explicit state for a fresh generation to continue deterministically without opaque provider memory.

### 6.5 Explicit invalidation table

Proposal 018 makes invalidation and reuse bans explicit:

| Terminal / runtime condition | Default generation result | Reuse policy |
|---|---|---|
| `limit_exhausted_before_output` | invalidate current generation | next invocation is `fresh_after_budget` |
| `limit_exhausted_after_output` | invalidate or operator-review-only | no silent auto-reuse |
| `policy_stop` / `safety_stop` | invalidate current generation | never reuse |
| `failed_after_output_validation` | retain history but mark generation non-auto-reusable | operator reset or explicit override required |
| contradictory receipt / transcript / runtime truth | mark `unverifiable_session_history` | never reuse automatically |
| binding mismatch | invalidate current generation | fresh required |
| budget threshold exceeded | checkpoint then compact or invalidate | `fresh_after_compaction` or `fresh_after_budget` |
| explicit operator reset | end current generation | `fresh_after_reset` |

### 6.6 Reuse decision ownership contract

`SessionReusePolicy` must answer one narrow question:

> may this exact invocation owner or family-authorized invocation reuse this exact active generation?

It must not answer the broader and weaker question:

> has this agent been seen before in this run?

The policy therefore reads, in order:

1. persisted `invocationOwnerKey`
2. active `AgentSessionLineage`
3. immutable `AgentSessionGeneration`
4. current binding fingerprint
5. current recovery branch / owner execution lineage imported from execution truth

If any of those are missing or contradictory, the result is `fresh_session_required`, not `reuse`.

`SessionReusePolicy` is explicitly downstream of execution truth here.
It must not resolve branch identity from session history when execution truth disagrees or is absent.

### 6.7 Fail-closed lineage history rule

Inspection and reporting must never reconstruct lineage history from the last mutable row alone.

Rules:

1. report and UI surfaces read `AgentSessionLineage` for ownership only
2. they read `AgentSessionGeneration` for current and historical generation truth
3. they read `AgentSessionEvent` for reset/invalidation/reuse narrative
4. if history is incomplete, the session state must degrade to `unverifiable_session_history`, not guessed

This is the session equivalent of fail-closed runtime truth from Proposal 016.

### 6.8 Operator reset and inspection ownership

Proposal 018 must not introduce a parallel operator surface for session work.

The canonical owner for reset/inspection is the existing shell recovery spine:

- `RecoveryCoordinator`
- `RecoverySheet`
- `BlockedRunRecoveryView`
- existing run report / blocked-run evidence surfaces

That means:

1. `Reset Agent Session` is a typed recovery action, not a standalone settings action
2. `AgentSessionInspector` is an inspection panel attached to the same recovery/report surfaces that already own retry/clone/inspect
3. suggested versus allowed versus blocked session actions must use the same recovery-policy contract as retry and clone actions
4. operator reset must emit the same persisted evidence quality as any other recovery action

Proposal-local previews, debug tools, or side panels do not satisfy this requirement on their own.

### 6.9 Reset semantics

Operator reset performs exactly this:

1. append `operator_reset` to `AgentSessionEvent`
2. mark the current `AgentSessionGeneration` as ended with `endReason = operator_reset`
3. advance `AgentSessionLineage.activeGenerationID` to nil until the next invocation creates a fresh generation
4. persist a recovery-surface-visible audit record
5. force the next invocation for that `invocationOwnerKey` to `fresh_after_reset`

Reset does not mutate prior generations in place and does not erase lineage history.

---

## 7. Success Metrics

Proposal 018 is not successful merely because reuse exists.
It must show measurable burn reduction.

Minimum KPIs:

- percent of executions using a reused session
- `cold_start_tokens_saved`
- average input tokens per invocation by agent
- `session_growth_tokens`
- forced resets due to budget
- token savings versus fresh baseline

Interpretation rules:

1. higher reuse rate alone is not success if average prompt cost keeps rising,
2. checkpoint-plus-fresh can be better than continued reuse when history growth dominates,
3. same-agent-family reuse should be justified with measured savings, not only intuition.
4. reuse counts as a savings win only when measured provider signals or normalized fresh-baseline comparison stay favorable after compaction/truncation effects are included.
