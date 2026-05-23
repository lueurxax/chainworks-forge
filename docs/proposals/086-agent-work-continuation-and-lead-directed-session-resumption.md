# Proposal 086: Agent Work Continuation and Lead-Directed Same-Session Resumption

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | [Local persistence write-budget contract](../reference/rust-control-plane.md#sqlite-write-serialization-and-gateway-dbwriter), [durable side-effect ledger](../reference/rust-control-plane.md#durable-side-effect-ledger) for side-effect lanes, P081 Boundary Matrix / UI Action Boundary |
| Related | P079 Contract-Aware Output Repair, P080 Continuous Stale Execution Reconciliation, P093 Phase 5 Expansion Soak, Session Lineage Reference, ACP Runtime Transport |
| Scope | Add an in-band way to continue useful agent work through server-owned provider session continuity instead of forcing a fresh retry that re-discovers the proposal, repository, blockers, and current diff. Support live-handle continuation first, define provider-session resurrection by known provider `session_id` for adapters that can attach/resume that provider session, fail closed for adapters that cannot, and allow both operator-triggered continuation through MCP and lead-directed automatic continuation under strict eligibility and safety rules. |
| Out of scope | Phase 5 expansion/soak is split into Proposal 093. Current P086 closeout must finish implementation and evidence for phases 1-4 only. |
| Goal | Reduce wasted time and model/runtime burn on implementation work by preserving useful provider-session continuity while still recording Chainworks truth, evidence, provenance, and safety boundaries. |

---

## 1. Why this proposal exists

The current retry model is too expensive for implementation work.

When a code-writing agent partially completes useful work but stops too early or fails output settlement, a normal retry often creates a fresh provider session. That fresh session frequently redoes expensive work:

- rereads the proposal,
- rereads audit/review artifacts,
- re-explores the repository,
- rediscovers current blockers,
- re-understands the same worktree diff,
- burns time and subscription/runtime limits before doing new work.

A direct manual experiment showed that using the same provider session context can work much better: the agent continued its previous mental/work context, wrote more code, ran tests, and did not restart discovery.

However, the manual direct-ACP continuation was out-of-band:

- Chainworks did not record it as an execution event,
- no durable continuation receipt existed,
- no lineage/provenance was recorded,
- no scheduler/recovery truth advanced,
- later workflow retry still had to re-accept/verify the work.

This proposal turns that successful manual pattern into a first-class, server-owned operation.

Experiment evidence is captured in [P086 Evidence: Manual Provider-Session Resurrection Experiment](../evidence/086-agent-work-continuation-and-session-resumption/manual-provider-session-resurrection-2026-05-08.md). The important lesson from that experiment is that we did not merely use an already-live Chainworks runtime handle. We revived/continued a provider session by known provider `session_id` outside Chainworks ownership. P086 therefore needs to model both live runtime-handle continuation and provider-session resurrection by provider `session_id`.

Provider-session resurrection does **not** mean keeping an old OS-level ACP
subprocess alive after daemon restart. Any ACP subprocess orphaned by daemon
restart must be terminated/reaped first and recorded as recovery evidence. The
resurrection path then starts a new Chainworks-managed ACP process/handle and
asks the provider adapter to attach/resume the known provider `session_id`, with
an attach receipt linked to the continuation.

---

## 2. Key distinction

This proposal introduces a third operation that must remain distinct from retry and output repair.

## 2.1 Retry

Retry means:

> create a new execution attempt and run the task again.

Retry may create a new session or reuse one only if normal session policy allows it.

Retry is appropriate when the prior attempt is invalid or should be replaced.

## 2.2 Output repair

Output repair means:

> the agent likely did the work, but the required machine output envelope is missing or invalid.

Output repair asks the same session to return the missing contract payload.

This is owned by P079.

## 2.3 Work continuation

Work continuation means:

> the agent has useful provider-session context and likely useful partial work; send another task turn into the same provider session so the agent can continue the implementation work instead of starting over.

Work continuation is not a retry and not output repair.

It is an additional same-session implementation turn with explicit evidence, guardrails, and provenance.

## 2.4 Live continuation vs provider-session resurrection

This proposal must distinguish two transport paths.

**Live-handle continuation** means Chainworks already owns a live ACP handle in `AcpRuntimeManager` for the target `session_generation_id`. The server validates that the live handle still matches the recorded provider session id, then sends another prompt into that handle.

**Provider-session resurrection** means Chainworks no longer owns a live handle, but the operator or durable run truth has a known provider `session_id`, and the provider adapter can attach/resume that provider session by id. This is the behavior exercised manually during the continuation investigation. It must be represented as an explicit continuation mode, not as ordinary retry, output repair, or checkpoint rehydration.

Provider-session resurrection is provider-session continuity, not process
continuity. If daemon restart left an old ACP subprocess alive, RecoveryService
must terminate/reap that orphan and persist the outcome before any
provider-session resurrection can be accepted. A supported resurrection creates
a new managed ACP process/handle and attaches that new handle to the recorded
provider session id.

The current implemented reference path only documents live-handle reuse and checkpoint rehydration. See [Session Lineage Reuse and Operator Reset](../reference/session-lineage-reuse-and-operator-reset.md#live-acp-session-ownership). `AcpRuntimeManager::prompt_session` currently prompts an existing live generation and does not attach to a provider session solely by provider `session_id`. P086 must add that attach/resume path for provider adapters that expose the capability; profiles without that adapter support must report the mode as unsupported and fail closed.

---

## 3. Core decision

Add a new server-owned operation:

```text
agents.continue_work
```

This operation sends a continuation prompt for a specific agent execution through one of two validated continuation modes:

1. an existing live ACP session generation owned by `AcpRuntimeManager`;
2. a provider-session resurrection path, when the adapter explicitly supports attach/resume by known provider `session_id`.

It is available in two ways:

1. **Operator-triggered continuation** through MCP.
2. **Lead-directed automatic continuation** when the lead produces a valid continuation decision and the server validates it.

SwiftUI must not invoke this operation directly and must not render an in-app
Continue command surface for P086. Operator-triggered continuation is an MCP
operator action outside the governed app UI.

GraphQL may show continuation state and evidence, but there is no GraphQL mutation for continuation.

---

## 4. Initial scope

Phase 5 expansion is no longer part of this implementation run. The 14-day
no-hold soak window, SLO-budget expansion, and 100-continuation/30-run
graduation evidence are owned by Proposal 093 after P086 phases 1-4 are
implemented and validated.

## 4.1 First supported agent class

Initial support is for:

- `code_writer`

Optional later support:

- `docs_guardian`
- bounded implementation helper agents

## 4.2 Explicitly not supported initially

Do not allow continuation for:

- `lead_orchestrator`
- proposal reviewers
- proposal aggregators
- security checker
- prepush reviewer
- release agents
- commit/push agents
- Connect/upload/distribution agents
- any stage with unresolved external side-effect ledger entries

## 4.3 Why code writer first

The code writer is the role where same-session continuity is most valuable:

- it remembers current diff,
- remembers which files were inspected,
- remembers tests already run,
- remembers blockers,
- can continue editing without full rediscovery.

At the same time, code-writing work can be read back through deterministic worktree evidence:

- changed files,
- diff summary,
- tests,
- generated artifacts.

That makes it safer than release/publish side effects.

---

## 5. Relationship to existing session lineage rules

The existing session-lineage contract intentionally fails closed when ownership, binding fingerprint, invocation owner, retry instruction hash, worktree, MCP inventory, or output contract changes.

That is correct for ordinary reuse.

Work continuation does not weaken those rules globally.

Instead, it introduces a **targeted continuation command** that:

- points to a specific existing generation,
- validates compatibility,
- records an explicit one-shot continuation,
- sends a bounded prompt through the selected provider-continuity transport,
- records the result as continuation evidence,
- does not pretend this was an ordinary retry.

This keeps normal session reuse strict while allowing operator/lead-controlled continuity where it is actually useful.

---

## 6. Eligibility

A continuation request is eligible only if all are true:

1. source run exists;
2. source stage execution exists;
3. source agent execution exists;
4. agent role is continuation-capable;
5. session generation exists;
6. provider session id is present;
7. requested continuation mode is valid;
8. generation belongs to the same run;
9. generation belongs to the same agent or an explicitly compatible continuation family;
10. worktree/workdir matches the target execution;
11. runtime profile / adapter family is compatible;
12. no unresolved side-effect ledger entry exists for the run/stage/agent;
13. target stage is not release/publish/git-push/upload/distribution;
14. continuation count is within policy limits;
15. prompt mode-reset guard can be applied;
16. the selected transport path passes its mode-specific checks.

For `live_handle_continuation`, mode-specific checks are:

1. ACP runtime manager has a live handle for the session generation;
2. live handle provider session id matches the recorded provider session id.

For `provider_session_resurrection`, mode-specific checks are:

1. trigger is `operator_mcp` in the first implementation;
2. provider session id is recorded and explicitly supplied or resolved from run truth;
3. adapter/runtime profile declares provider-session resurrection support;
4. resurrection target belongs to the same run, agent execution, worktree, and provider family;
5. restart recovery has no unreaped ACP subprocess for the target session generation, provider session id, worktree, or agent execution;
6. if an orphan ACP subprocess was found, it was terminated/reaped and the reap outcome was persisted before terminalizing stale runtime truth;
7. a new managed ACP process/handle can be created for the continuation;
8. attach/resume receipt can be persisted before prompt execution.

If provider-session resurrection is requested but unsupported by the adapter/runtime, continuation must fail closed with `provider_session_resurrection_unsupported`. It must not silently fall back to fresh retry or checkpoint rehydration.

If any check fails, continuation must fail closed.

## 7.1 Daemon restart and orphan ACP recovery

Daemon restart creates a hard boundary between process continuity and provider
session continuity:

1. `AcpRuntimeManager` live handles from the prior daemon generation are dead.
2. Any surviving ACP subprocess from that generation is an orphan, not a valid
   continuation handle.
3. RecoveryService must locate known child/provider helper processes using the
   durable supervised-process/session-generation registry.
4. RecoveryService must terminate/reap matching orphan ACP subprocesses before
   it marks stale continuations terminal or accepts provider-session
   resurrection for the same run/stage/agent/provider session.
5. The reap attempt must produce durable evidence: old pid, session generation,
   provider session id when known, signal/deadline, outcome, and timestamp.
6. If reap fails or cannot be proven, continuation fails closed with
   `orphan_acp_reap_failed` or `orphan_acp_reap_unverified`.
7. A successful provider-session resurrection then starts a new
   Chainworks-managed ACP process/handle and attaches/resumes the known provider
   `session_id`; it never reuses the orphan OS process.

This is the implementation form of the manual evidence: preserve provider
context by attaching a new managed ACP runtime to the old provider session id,
while still cleaning up unsafe orphan subprocesses after restart.

---

## 7. Triggers

## 7.1 Operator-triggered continuation

MCP tool:

```text
agents.continue_work
```

Operator provides:

- run id,
- stage execution id,
- agent execution id,
- session generation id,
- continuation mode,
- optional provider session id for provider-session resurrection,
- operator instruction,
- optional max turn/time budget,
- optional explicit blockers to continue from.

The server validates eligibility before sending anything to ACP.

## 7.2 Lead-directed automatic continuation

The lead may request continuation by emitting:

```text
lead_continuation_decision_v1
```

This is a recommendation, not authority.

The server must validate it before execution.

The lead may request continuation when:

- code writer produced useful partial work,
- changed files exist or meaningful implementation progress exists,
- blockers remain,
- normal retry would likely lose valuable live context,
- current session is live for automatic continuation,
- task is not a release/publish side-effect lane,
- continuation policy allows lead-triggered continuation.

The lead must not request continuation to bypass output contracts or safety gates.

## 7.3 Automatic continuation policy limits

Default limits:

- max 1 lead-directed continuation per agent execution,
- max 2 lead-directed continuations per stage execution,
- no lead-directed continuation after unresolved side-effect warning,
- no lead-directed continuation after provider-mode mismatch classified as strict-output incompatible,
- no lead-directed continuation for release agents.

Operator-triggered continuation may have a separate higher limit, but still requires validation.

---

## 8. Data model

## 8.1 `agent_work_continuations`

Suggested table:

```sql
CREATE TABLE agent_work_continuations (
  id TEXT PRIMARY KEY,
  run_id TEXT NOT NULL,
  stage_execution_id TEXT NOT NULL,
  agent_execution_id TEXT NOT NULL,
  session_lineage_id TEXT NOT NULL,
  session_generation_id TEXT NOT NULL,
  provider_session_id TEXT NOT NULL,
  continuation_mode TEXT NOT NULL,
  resurrected_from_provider_session_id TEXT,
  live_handle_required INTEGER NOT NULL DEFAULT 1,
  adapter_resume_capability TEXT,
  trigger_kind TEXT NOT NULL,
  requested_by TEXT,
  idempotency_scope TEXT NOT NULL,
  idempotency_key TEXT NOT NULL,
  request_fingerprint_sha256 TEXT NOT NULL,
  canonical_request_artifact_id TEXT,
  response_fingerprint_sha256 TEXT,
  response_artifact_id TEXT,
  conflict_count INTEGER NOT NULL DEFAULT 0,
  lead_decision_artifact_id TEXT,
  operator_instruction_sha256 TEXT,
  prompt_template_version TEXT NOT NULL,
  status TEXT NOT NULL,
  started_at TEXT,
  completed_at TEXT,
  failed_at TEXT,
  failure_reason TEXT,
  attach_receipt_artifact_id TEXT,
  evidence_bundle_artifact_id TEXT,
  worktree_readback_artifact_id TEXT,
  continuation_report_artifact_id TEXT,
  created_at TEXT NOT NULL,
  updated_at TEXT NOT NULL
);

CREATE UNIQUE INDEX idx_agent_work_continuations_idempotency
ON agent_work_continuations(idempotency_scope, idempotency_key);
```

`idempotency_key` is not meaningful by itself. It is valid only with persisted
`idempotency_scope` and `request_fingerprint_sha256`.

The default scope is:

```text
agents.continue_work:{caller_principal_id}
```

The canonical request fingerprint includes run, stage, agent, session,
provider, policy, and prompt-template identity, so two different continuation
targets using the same key cannot collapse into one replay.

## 8.2 Canonical request fingerprint

`agents.continue_work` computes `request_fingerprint_sha256` from canonical
JSON, not from raw request bytes.

The canonical request must include:

```json
{
  "command": "agents.continue_work",
  "run_id": "...",
  "stage_execution_id": "...",
  "agent_execution_id": "...",
  "session_generation_id": "...",
  "provider_session_id": "...",
  "trigger_kind": "operator_mcp | lead_auto",
  "caller_principal_id": "...",
  "operator_instruction_sha256": "...",
  "lead_decision_artifact_id": "...",
  "lead_decision_artifact_sha256": "...",
  "prompt_template_version": "...",
  "max_turns": 1,
  "max_wall_clock_seconds": 1800,
  "worktree_root": "...",
  "runtime_profile_id": "...",
  "continuation_policy_version": "..."
}
```

Canonical JSON rules:

- sorted keys;
- normalized defaults;
- normalized line endings before hashing instruction text;
- stable null versus absent handling;
- no timestamps, random request ids, display labels, or raw long instruction
  text when an artifact pointer plus SHA is available.

For lead-directed continuation, the server may derive `idempotency_key` as:

```text
lead-auto:{lead_decision_id}
```

The fingerprint still must include `lead_decision_artifact_id`,
`lead_decision_artifact_sha256`, `agent_execution_id`,
`session_generation_id`, `continuation_instruction_sha256`, and
`continuation_policy_version`. If the lead decision artifact changes while the
same idempotency key is reused, the request is an idempotency conflict.

## 8.3 Replay versus conflict semantics

| Existing row | Incoming request | Result |
|---|---|---|
| same `idempotency_scope` + key, same fingerprint, terminal success | replay | return previous response |
| same scope + key, same fingerprint, `requested` / `preflight_passed` / `prompt_sent` / `observing` | replay | return existing continuation status; do not start another task |
| same scope + key, same fingerprint, `failed` / `preflight_failed` | replay | return the same failure unless caller uses a new idempotency key |
| same scope + key, different fingerprint | conflict | return `idempotency_conflict`; do not mutate |
| no row | new request | create row and continue |

Conflict response:

```json
{
  "error": "idempotency_conflict",
  "idempotency_key": "...",
  "existing_continuation_id": "...",
  "existing_request_fingerprint": "...",
  "incoming_request_fingerprint": "...",
  "message": "Same idempotency key was used with a different canonical request."
}
```

If a duplicate request reaches a continuation that already passed
`prompt_sent`, the server must not send the continuation prompt again. It
returns the existing status or enters continuation reconciliation.

## 8.4 Prompt-sent crash window

`agents.continue_work` can change the worktree. If the daemon crashes after
the prompt is delivered to ACP but before result settlement, the retry path
must assume the agent may already have edited files.

State progression:

```text
requested
-> preflight_passed
-> prompt_sent
-> observing
-> worktree_observed
-> settled
```

When recovery sees `prompt_sent` or `observing` without a settled result, it
must not resend the prompt. It moves the continuation to
`needs_continuation_reconciliation` and gathers readback:

- worktree diff and ownership evidence;
- ACP transcript/tool evidence if available;
- continuation report or no-progress evidence if present;
- relevant tests or gate outputs if they were produced.

Reconciliation settles the existing continuation record. It does not create a
new continuation turn and does not reuse the same idempotency key for a second
provider send.

## 8.5 `trigger_kind`

Allowed values:

- `operator_mcp`
- `lead_auto`

## 8.6 `continuation_mode`

Allowed values:

- `live_handle_continuation`
- `provider_session_resurrection`

## 8.7 `status`

Allowed values:

- `requested`
- `preflight_passed`
- `preflight_failed`
- `provider_session_resurrection_unsupported`
- `provider_session_not_found`
- `provider_session_attach_failed`
- `orphan_acp_reap_failed`
- `orphan_acp_reap_unverified`
- `prompt_sent`
- `observing`
- `worktree_observed`
- `needs_continuation_reconciliation`
- `settled`
- `completed`
- `failed`
- `rejected`
- `cancelled`

## 8.8 New artifacts

- `agent_continuation_evidence_bundle`
- `agent_continuation_report`
- `worktree_continuation_readback`
- `lead_continuation_decision`
- `provider_session_attach_receipt`
- `continuation_canonical_request`
- `continuation_response_snapshot`

---

## 9. Evidence model

Continuation must not depend on the agent returning a strict `CHAINWORKS_OUTPUT` envelope.

The server records truth through readback:

- ACP transcript evidence,
- tool trace evidence,
- worktree diff summary,
- changed files manifest,
- tests run and results,
- generated artifacts,
- final human summary if the provider produced one.

The continuation itself does not automatically settle the stage as complete.

It creates evidence that the normal workflow can later validate, review, or retry.

---

## 10. Persistence and write-budget rules

Continuation must obey the implemented write-budget contract.

Rules:

- ACP transcript goes to file spool;
- tool traces go to file spool;
- stdout/stderr go to file spool;
- SQLite stores compact metadata and artifact pointers only;
- do not create one SQLite row per stream chunk;
- continuation metadata is a compact record;
- worktree readback is a compact artifact pointer.

---

## 11. Side-effect safety

Continuation must obey the durable side-effect ledger.

If unresolved side-effect ledger entries exist, continuation fails with:

```text
requires_effect_reconciliation
```

Continuation is forbidden for:

- release agents,
- git push stages,
- Connect upload stages,
- publish/distribution stages,
- stages that may have already changed the external world.

The only allowed initial lane is implementation/code editing without irreversible external release effects.

---

## 12. MCP tools

## 12.1 `agents.continue_work`

Input:

```json
{
  "run_id": "...",
  "stage_execution_id": "...",
  "agent_execution_id": "...",
  "idempotency_key": "...",
  "session_generation_id": "...",
  "continuation_mode": "live_handle_continuation",
  "provider_session_id": "...",
  "operator_instruction": "...",
  "max_turns": 1,
  "max_wall_clock_seconds": 1800
}
```

`continuation_mode` may be:

- `live_handle_continuation`, requiring an existing live `AcpRuntimeManager` handle;
- `provider_session_resurrection`, requiring adapter support for attach/resume by provider `session_id`.

If `provider_session_resurrection` is requested and unsupported, the output status must be `provider_session_resurrection_unsupported`.

Output is an admission response, not a terminal execution response. The
command validates eligibility, records or replays the canonical request, and
queues/returns the continuation row. It must not block the MCP request until the
provider turn completes.

```json
{
  "outcome": "accepted",
  "continuation_id": "...",
  "request_fingerprint_sha256": "...",
  "status": "accepted"
}
```

`outcome` is one of:

- `accepted` — a new continuation was admitted and queued;
- `replay` — the same idempotency key and canonical request already exist, so
  the command returns the existing continuation id/status without another
  provider send;
- `rejected` — admission failed before a row was queued, with a bounded machine
  error object.

Terminal fields are readback, not command output. After `accepted` or `replay`,
clients must read `agents.continuation_status`, `continuations(runId:)`, or
`continuationStatus(agentExecutionId:)` to obtain:

- `continuation_mode`;
- terminal `status`;
- `response_fingerprint_sha256`;
- `session_generation_id`;
- `provider_session_id`;
- `canonical_request_artifact_id`;
- `response_artifact_id`;
- `attach_receipt_artifact_id`;
- `evidence_bundle_artifact_id`;
- `worktree_readback_artifact_id`;
- `continuation_report_artifact_id`;
- `result_or_no_progress_artifact_id`.

This split is intentional: admission is transactional and bounded, while the
provider turn is asynchronous and may complete, fail, be cancelled, or reconcile
after daemon restart.

## 12.2 `agents.continuation_status`

Reads status and evidence for a continuation.

## 12.3 `agents.continuation_candidates`

Optional first version.

Returns executions with:

- live compatible session,
- resumable provider session candidate when adapter capability exists,
- useful partial work signals,
- no unresolved side effects,
- continuation-capable role.

---

## 13. GraphQL readback

GraphQL exposes continuation state for UI inspection only. The governed SwiftUI
app is read-only for P086 continuation: it may show that continuation happened,
is running, failed, or produced evidence, but it must not offer a Continue button,
menu item, keyboard shortcut, or any other command affordance.

Suggested fields:

```graphql
type AgentContinuation {
  id: ID!
  status: AgentContinuationStatus!
  triggerKind: ContinuationTriggerKind!
  continuationMode: ContinuationMode!
  agentExecutionId: ID!
  sessionGenerationId: ID!
  providerSessionId: String!
  resurrectedFromProviderSessionId: String
  startedAt: DateTime
  completedAt: DateTime
  failureReason: String
  attachReceipt: Artifact
  evidenceBundle: Artifact
  worktreeReadback: Artifact
  continuationReport: Artifact
}
```

No GraphQL mutation.

SwiftUI may show:

- continuation history,
- evidence bundle,
- changed files,
- tests,
- whether continuation was operator-triggered or lead-directed,
- failure/no-progress reason and resulting evidence after an attempted continuation.

SwiftUI must not show a recommended next MCP action or any call-to-action that
looks executable inside the app. The UI contract is readback only: it reports the
fact and result of a continuation already performed by MCP/lead orchestration.

---

## 14. Lead continuation decision contract

The lead must emit a structured decision when requesting automatic continuation.
When a stage-owned lead agent completes, the control plane inspects its newly
materialized artifacts for `lead_continuation_decision_v1`. A valid decision is
not advisory-only: after server-side target, hash, safety, capability, side
effect, and approval checks pass, the engine admits the continuation through the
same durable `agent_work_continuations` admission transaction and enqueues
`ProcessContinuation`. Invalid, stale, or unsafe lead decisions fail closed and
do not require an operator to call MCP manually.

## 14.1 `lead_continuation_decision_v1`

Required fields:

```json
{
  "decision_id": "...",
  "run_id": "...",
  "stage_execution_id": "...",
  "agent_execution_id": "...",
  "agent_id": "code_writer",
  "session_generation_id": "...",
  "reason": "...",
  "continuation_instruction": "...",
  "expected_next_work": [
    "..."
  ],
  "known_completed_work": [
    "..."
  ],
  "known_blockers": [
    "..."
  ],
  "safety_checks": {
    "no_release_side_effect": true,
    "no_unresolved_effect_ledger": true,
    "same_worktree_required": true
  },
  "stop_conditions": [
    "..."
  ],
  "max_turns": 1,
  "max_wall_clock_seconds": 1800
}
```

The server must verify every safety condition.
The lead decision cannot override server policy.

---

## 15. Prompt design

This proposal includes canonical prompt templates because the success of continuation depends heavily on mode reset.

The most important issue discovered manually:

> if the prior turn was output-contract repair, the provider session may remain mentally stuck in “return corrected machine output” mode.

Every continuation prompt must therefore explicitly reset the mode.

---

# 15.1 Common Mode Reset Header

This header is prepended to every continuation prompt.

```text
You are continuing implementation work in an existing Chainworks agent session.

This is NOT output-contract repair.
Do NOT try to fix or return CHAINWORKS_OUTPUT.
Do NOT respond with only a JSON object.
Do NOT write or edit Chainworks run metadata under .chainworks/runs unless explicitly instructed.
Do NOT restart full project/proposal discovery unless you need a specific missing fact.

You are continuing coding work in the existing worktree.
Use the context already established in this session:
- files already inspected,
- diffs already made,
- tests already run,
- blockers already found,
- reviewer/audit/prepush findings already read.

Your job is to continue useful implementation work from the current state, not to start over.
```

---

# 15.2 Operator-Triggered Continuation Prompt

```text
{COMMON_MODE_RESET_HEADER}

Operator continuation request
=============================

Run:
{run_id}

Stage execution:
{stage_execution_id}

Agent execution:
{agent_execution_id}

Worktree:
{worktree_root}

Operator instruction:
{operator_instruction}

Known current context
=====================

Previously completed or partially completed work:
{known_completed_work}

Known remaining blockers:
{known_blockers}

Relevant review/audit/prepush findings:
{relevant_findings}

Rules for this continuation
===========================

1. Continue from the existing patch and session context.
2. Do not redo broad repository discovery unless necessary for a specific missing fact.
3. Prefer making concrete code changes over writing plans.
4. Do not touch release, git push, upload, publish, or external distribution operations.
5. Do not commit.
6. Do not push.
7. Do not modify run control-plane metadata.
8. If you need to inspect files, inspect only the minimum necessary.
9. If tests are available and relevant, run focused tests first.
10. Stop when you have made the next meaningful unit of progress or when a real blocker prevents progress.

Closeout requirements
=====================

At the end, provide a concise human summary with these sections:

- Changed files
- Tests run
- Remaining blockers with file-level evidence
- What should happen next

Do not claim implementation complete unless:
- the requested code changes are made,
- relevant tests are green or their failure is explained,
- remaining blockers are explicitly listed.
```

---

# 15.3 Lead-Directed Automatic Continuation Prompt

```text
{COMMON_MODE_RESET_HEADER}

Lead-directed continuation
==========================

The lead has determined that this agent should continue work in the same live session instead of starting a fresh retry.

Reason:
{lead_reason}

Current implementation goal:
{implementation_goal}

Known completed work:
{known_completed_work}

Known remaining blockers:
{known_blockers}

Expected next work:
{expected_next_work}

Stop conditions:
{stop_conditions}

Safety constraints
==================

- This is implementation continuation only.
- Do not perform release, git push, upload, publish, or external distribution.
- Do not attempt output-contract repair.
- Do not return CHAINWORKS_OUTPUT.
- Do not write plans instead of doing code work unless a real blocker prevents coding.
- Do not restart broad discovery.
- Continue from the existing patch.

Closeout requirements
=====================

At the end, provide a concise human summary with:

- Changed files
- Tests run
- Remaining blockers with file-level evidence
- Whether another continuation is useful
- Whether normal workflow retry/validation should run next
```

---

# 15.4 Anti-Planning Guard

If the provider starts writing a plan instead of doing implementation work, the continuation supervisor should treat that as a weak/no-progress result unless the plan identifies a real blocker.

Heuristic signals:

- plan file created but no code diff,
- long “I will do” response,
- no changed files,
- no test execution,
- no blocker with file-level evidence.

Result classification:

```text
continuation_no_useful_progress
```

The engine may then:

- stop automatic continuations,
- ask operator,
- or fall back to normal retry.

---

# 15.5 Closeout Evaluation Prompt for Lead

This prompt is for the lead to evaluate continuation evidence, not to perform coding.

```text
You are evaluating a continuation turn after a code-writing agent continued work in the same provider session.

Do not perform code changes.
Do not request another continuation unless the evidence shows useful progress and a clear next unit of work.

Inputs:
- continuation report
- worktree readback
- changed files
- test results
- remaining blockers
- original implementation goal

Return a decision object with:

{
  "continue_again": true | false,
  "reason": "...",
  "next_instruction": "...",
  "blocking_issues": [...],
  "ready_for_normal_validation": true | false
}

Rules:
- continue_again may be true only if another continuation is likely to produce concrete code progress.
- do not continue just because the agent wrote a plan.
- prefer normal workflow validation when the worktree has meaningful completed work.
- never request continuation for release or external side-effect stages.
```

---

## 16. Automatic continuation policy

Automatic continuation must be conservative.

## 16.1 Allowed automatic trigger

The lead may trigger continuation when all are true:

- agent is continuation-capable;
- live session exists;
- worktree has useful partial progress or previous transcript shows useful context;
- lead identifies a concrete next unit of work;
- normal retry would likely waste rediscovery effort;
- no unresolved effect ledger entry exists;
- continuation budget remains.

## 16.2 Forbidden automatic trigger

Lead must not auto-continue when:

- stage is release/publish/upload/git-push related;
- side-effect ledger has unresolved entries;
- provider session is dead;
- worktree changed outside expected root;
- prior continuation made no useful progress;
- the next step is approval/review/release rather than implementation;
- max continuation count reached.

## 16.3 Continuation budget

Suggested defaults:

```yaml
continuation_policy:
  code_writer:
    enabled: true
    lead_auto_enabled: true
    operator_mcp_enabled: true
    max_lead_continuations_per_agent_execution: 1
    max_lead_continuations_per_stage_execution: 2
    max_operator_continuations_per_agent_execution: 3
    max_wall_clock_seconds: 1800
    max_turns_per_continuation: 1
```

Operator continuation can be more permissive than lead auto continuation, but still must pass safety preflight.

Lead-directed automatic continuation is initially limited to `live_handle_continuation`. Provider-session resurrection is operator-triggered only until adapter support, attach receipts, and readback behavior have enough production evidence for automation.

---

## 17. Agent configuration

Add continuation capability to agent catalog.

Example:

```yaml
agents:
  - id: code_writer
    continuation_capability:
      enabled: true
      allowed_triggers:
        - operator_mcp
        - lead_auto
      allowed_session_scope:
        - same_agent
        - same_agent_family_within_run
      forbidden_stage_kinds:
        - release
        - publish
        - git_push
        - upload
      live_handle_continuation:
        enabled: true
        require_live_session: true
      provider_session_resurrection:
        enabled: false
        allowed_triggers:
          - operator_mcp
        require_recorded_provider_session_id: true
        fail_closed_when_unsupported: true
      require_same_worktree: true
      require_no_unresolved_side_effects: true
```

For agents without this field, continuation is disabled.

`provider_session_resurrection.enabled: false` is intentional for adapters that cannot attach/resume by provider `session_id`. The server still needs the field so an operator request can fail with an explicit unsupported status instead of falling back to a fresh retry and losing the provider context the operator asked to preserve.

---

## 18. Relationship to P079

P079 handles contract-aware output repair.

P086 handles work continuation.

If a code writer produced useful work but failed final output settlement, the runtime may choose among:

1. output repair, if the work is done and only contract payload is missing;
2. continuation, if useful work exists but implementation is not done;
3. normal retry, if the attempt is invalid or session is unusable.

A failed output repair should not automatically cause another repair attempt if the provider is stuck in repair mode.

Instead, the engine may classify:

```text
repair_mode_contaminated_session
```

and choose:

- continuation with mode reset, if code work should continue;
- fallback retry, if only machine output is needed;
- operator review, if ambiguous.

---

## 19. Relationship to P080

P080 stale execution reconciliation may recommend continuation only for stale or blocked work that has:

- live provider session,
- useful partial implementation context,
- no side-effect risk,
- continuation-capable agent role.

P080 must not auto-continue release side-effect lanes.

---

## 20. Metrics

Track:

- continuation count per run/stage/agent;
- fresh session avoided count;
- average time saved vs fresh retry estimate;
- continuation useful-progress rate;
- no-progress continuation rate;
- tests passed after continuation;
- changed files after continuation;
- operator-triggered vs lead-triggered continuation success rate;
- follow-up normal validation success rate;
- provider/session budget impact.
- orphan ACP subprocesses found/reaped after daemon restart;
- provider-session resurrection attach success/failure after orphan reap.

These metrics should feed future limit observability work.

---

## 21. Tests

Required tests:

1. `agents.continue_work` uses same live provider session.
2. No new session generation is created for continuation.
3. Wrong run generation is rejected.
4. Wrong agent generation is rejected.
5. Dead provider session is rejected.
6. Release/publish/git-push stage is rejected.
7. Unresolved side-effect ledger entry returns `requires_effect_reconciliation`.
8. Last turn output-repair contamination results in prompt mode reset.
9. Lead decision can trigger continuation only when policy allows.
10. Lead decision cannot override server safety policy.
11. Continuation evidence is spooled to files, not high-volume SQLite rows.
12. GraphQL exposes continuation readback but no mutation.
12a. SwiftUI exposes continuation readback/history only and never renders an
    in-app Continue command surface.
13. No-progress plan-only continuation is classified as no useful progress.
14. Worktree readback captures changed files after continuation.
15. Provider-session resurrection request fails closed when adapter capability is absent.
16. Supported provider-session resurrection attaches/resumes by known provider session id without an existing live handle and records `provider_session_resurrection`.
17. Provider-session resurrection is not recorded as ordinary retry, output repair, checkpoint rehydration, or normal `SessionReuseDisposition::Reused`.
18. Lead-directed automatic continuation cannot request provider-session resurrection until policy explicitly enables it.
19. Daemon restart recovery terminates/reaps orphan ACP subprocesses before
    terminalizing stale continuation truth for that provider session.
20. Provider-session resurrection after restart starts a new managed ACP process
    and attaches/resumes the recorded provider `session_id`; it never reuses the
    orphan OS process.
21. Provider-session resurrection fails closed when orphan ACP subprocess reap
    is unverified or fails.
22. Same idempotency key plus same canonical request returns the previous
    continuation id/status without another provider send; terminal output fields
    remain available through continuation readback.
23. Same idempotency key plus different canonical request returns
    `idempotency_conflict`.
24. Same idempotency key plus same request at `prompt_sent` does not send
    another provider prompt.
25. Crash after prompt delivery but before settlement becomes
    `needs_continuation_reconciliation`.
26. Continuation reconciliation reads worktree/transcript evidence and settles
    without sending another continuation turn.
27. Lead-auto continuation stores `lead_decision_artifact_sha256` in the
    canonical request artifact.
28. Operator instruction changes with the same idempotency key are rejected as
    `idempotency_conflict`.

---

## 22. Acceptance criteria

P086 is complete when:

1. operator can continue a code writer in an existing live ACP session through MCP;
2. provider-session resurrection by known provider `session_id` is either implemented for supported adapters or explicitly exposed as a fail-closed unsupported continuation mode;
3. lead can request automatic continuation through a structured decision artifact;
4. server validates all continuation eligibility and safety conditions;
5. continuation prompts use the canonical mode reset template;
6. release and side-effect stages are fail-closed;
7. continuation evidence is recorded in Chainworks truth;
8. no high-volume evidence is inserted into SQLite;
9. UI can read continuation status through GraphQL but cannot invoke continuation;
10. normal retry remains separate from continuation;
11. output repair remains separate from continuation;
12. checkpoint rehydration remains separate from provider-session resurrection.
13. idempotent replay is based on persisted canonical request fingerprints, not
    uniqueness alone;
14. duplicate requests after `prompt_sent` never resend the provider prompt and
    use continuation reconciliation instead.
15. `agents.continue_work` returns a bounded admission response; terminal
    response/session/provider/artifact fields are exposed through MCP/GraphQL
    continuation readback, not by blocking the initial command.

---

## 23. Final recommendation

Chainworks currently overuses retry for situations where continuation is the better primitive.

Retry is for replacing an attempt.
Output repair is for fixing missing machine payloads.
Continuation is for preserving useful implementation context and pushing the same provider coding context forward.

This proposal adds continuation without weakening durable execution truth.

It should reduce wasted rediscovery, lower session churn, improve code-writer throughput, and keep the operator in control.
