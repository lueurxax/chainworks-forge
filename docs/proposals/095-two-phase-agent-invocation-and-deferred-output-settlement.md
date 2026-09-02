# Proposal 095: Two-Phase Agent Invocation and Deferred Output Settlement

| Field | Value |
|---|---|
| Date | 2026-05-28 |
| Status | Draft |
| Author | Codex |
| Depends on | implemented [P079 output repair/fallback contract](../reference/output-contracts-failure-evidence-and-recovery.md#p079-output-contract-repair-and-fallback-details), implemented P086 agent work continuation and provider-session resumption baseline, implemented [code-writer completion freshness and repair contract](../reference/output-contracts-failure-evidence-and-recovery.md#code-writer-completion-freshness-and-repair-p088-retained-alias) (retained P088 alias), implemented workflow-owned quality-gate boundary contract in [`workflow-execution-engine.md`](../reference/workflow-execution-engine.md#quality-gate-blocker-boundary-transitions) and [`output-contracts-failure-evidence-and-recovery.md`](../reference/output-contracts-failure-evidence-and-recovery.md#workflow-owned-quality-gate-boundary-contracts), implemented storage/evidence-spooling baseline |
| Related | `docs/reference/acp-runtime-transport.md`, `docs/reference/rust-control-plane.md`, `docs/reference/workflow-execution-engine.md`, `docs/reference/output-contracts-failure-evidence-and-recovery.md`, `docs/reference/agent-work-continuation.md` |
| Scope | Define the normal `code_writer` invocation lifecycle as a short work turn, server-owned deterministic readback, separate output collection turn, and strict settlement. |
| Non-goal | No Rust/Swift implementation in this proposal, no loosening of output contracts, no new SwiftUI mutations, and no change to release/publish/git-push/upload side-effect safety. |

---

## 1. Problem

Current agent invocations often ask one provider turn to do too many jobs at
once:

- perform substantial implementation work;
- keep repository, proposal, workflow, and prior review context in mind;
- run tests or reason about test evidence;
- satisfy strict Chainworks output contracts;
- return machine-readable `CHAINWORKS_OUTPUT` in the same turn.

That overload makes the most expensive failure mode more likely: useful code
work happens, but the structured output is missing, malformed, stale, or
otherwise unsettled. The system then spends fresh retries rediscovering
proposal and repository context, consumes provider/session limits, and creates
stale-output hazards around old artifacts.

The root issue is not that prompts need longer warning sections. The root issue
is that Chainworks currently mixes two different intents:

1. do the work;
2. publish the canonical machine-readable output.

Those intents should be separated. Runtime safety must be enforced by server
policy, path guards, tool availability, and durable side-effect settlement, not
by repeated long negative instruction lists.

## 2. Core Idea

P095 defines a two-phase invocation lifecycle with deterministic readback
between the phases. "Two-phase" refers to the two model-facing turns: work and
output collection. The server-owned readback and final settlement are separate
runtime phases.

### Phase 1: Work Turn

The work turn asks the agent to do one thing: make progress on the assigned
work.

Required behavior:

- the agent receives a clear work objective;
- the agent works in the current worktree;
- the prompt does not include output artifact paths;
- the prompt does not include `CHAINWORKS_OUTPUT` instructions;
- the prompt does not include long negative-rule lists;
- the turn does not settle the agent execution.

The result of this phase is provider work plus runtime evidence. It is not
output settlement.

### Phase 2: Deterministic Readback

After the work turn completes or blocks, the server reads the worktree and
evidence surfaces directly.

The readback must include, when available:

- changed files;
- diff summary;
- tests run and test outcomes;
- generated artifacts;
- tool traces;
- blockers;
- pre-worktree fingerprint;
- post-worktree fingerprint.

This readback is server-owned evidence. Model-authored claims are useful
context but are not truth for changed files, tests, artifacts, or freshness.

### Phase 3: Output Collection Turn

The output collection turn asks the agent to do one thing: produce the required
Chainworks output object from the server readback and declared output contract.

Required behavior:

- reuse the same provider session when possible;
- pass the deterministic worktree readback into the output prompt;
- include the required output schema or field list;
- request only the required output object;
- do not request implementation work in this turn;
- prefer read-only tool policy where the runtime/provider supports it.

If same-session reuse is unavailable, the server may use a fresh output
collector session from deterministic readback when it is safe to do so. This is
still output collection, not a fresh implementation retry.

### Phase 4: Settlement

The agent execution can settle only after valid output collection or a valid
repair path produces fresh contract-valid output.

If output collection fails, the system routes to P079 output repair/fallback
rules. It must not automatically perform a full fresh code retry merely because
the output turn failed.

## 3. Prompt Minimalism Rule

Agent-facing prompts must be single-intent.

Work prompts:

- contain only the work objective, current context, and completion target;
- do not include output-contract instructions;
- do not include output artifact paths;
- do not include long lists of negative rules.

Output prompts:

- contain only the output/reporting contract and server-owned readback;
- do not include implementation instructions;
- do not ask the agent to modify source files.

Safety constraints are enforced by:

- runtime permissions;
- tool availability;
- path/worktree guards;
- stage type;
- server policy;
- durable side-effect settlement/reconciliation.

Release, push, upload, and publish safety must not depend on prompt warnings.

## 4. Canonical Prompt Templates

### Work Turn

```text
Implementation turn.

Task:
{task}

Current context:
{current_context}

Completion target:
{completion_target}

Finish with:
DONE or BLOCKED, followed by one short sentence.
```

### Continuation Turn

```text
Implementation continuation.

Task:
{task}

Continue from the current session and worktree state.

Completion target:
{completion_target}

Finish with:
DONE or BLOCKED, followed by one short sentence.
```

### Output Collection Turn

```text
Output collection.

Use the worktree readback below to produce the required Chainworks output.
Return only the required output object.

Worktree readback:
{readback}

Required output:
{schema_or_field_list}
```

### Blocker Clarification Turn

```text
Blocker clarification.

State the blocker as one concrete missing fact, external condition, or failing command.
Return only:
- blocker
- evidence
- suggested next action
```

## 5. State Model

P095 proposes the following durable execution-state vocabulary:

- `work_prompt_pending`
- `work_prompt_sent`
- `work_turn_completed`
- `work_readback_completed`
- `output_collection_pending`
- `output_prompt_sent`
- `output_settled`
- `output_collection_failed`
- `needs_session_resurrection`
- `needs_output_repair`
- `failed`

This is proposal-level design only. It does not implement a migration.

The compact turn record can be modeled as:

```text
agent_invocation_turns
  id
  agent_execution_id
  turn_kind: work | output_collection | repair | continuation
  session_generation_id
  provider_session_id
  prompt_artifact_id
  result_artifact_id
  pre_worktree_fingerprint
  post_worktree_fingerprint
  status
  started_at
  completed_at
```

The record should point to file-backed prompt/result evidence rather than store
large transcripts or streamed chunks inline.

## 6. Restart and Daemon Recovery

The critical crash case is: daemon crashes after the work turn but before
output collection.

Required behavior:

- the server must know work happened or may have happened;
- the server must not automatically fresh-retry implementation work;
- the server should attempt provider-session resurrection by provider session
  id when the adapter and frozen run catalog support it;
- if same-session resurrection is unavailable, the server should use
  deterministic readback and a fresh output-collector session where safe;
- if output cannot be safely collected, the run blocks with an explicit reason
  instead of retrying the code work.

The recovery decision is server-owned. A missing output after a work turn is not
proof that the work did not happen.

## 7. Agent Scope

Initial supported role:

- `code_writer`

Optional future roles:

- `docs_guardian`;
- bounded implementation helper agents.

Explicitly excluded:

- release agents;
- git push, upload, publish, and distribution stages;
- proposal reviewers;
- lead aggregation tasks where the output itself is the work.

Release, publish, git-push, upload, and distribution safety belongs to durable
side-effect settlement and reconciliation. P095 must not become a side-effect
retry or publication safety mechanism.

## 8. Runtime-Owned Boundaries

P095 preserves the current Chainworks boundary model:

- Rust server owns orchestration and domain truth;
- ACP is the southbound runtime interface to agents;
- GraphQL is the only SwiftUI UI API;
- SwiftUI mutations remain approval-only;
- MCP owns non-approval operator/control actions.

Runtime boundary rules:

- do not rely on prompt text to prohibit release, push, upload, or publish;
- disable or omit release tools in work turns;
- prefer read-only tool policy for output collection turns;
- enforce path/worktree guards outside the prompt;
- enforce side-effect restrictions through durable side-effect policy;
- SwiftUI remains GraphQL-only and does not trigger this operation directly.

## 9. Evidence Storage

Work, readback, and output-turn evidence must follow the implemented
storage/evidence-spooling baseline:

- transcripts and tool traces go to file-backed evidence;
- prompt artifacts and result artifacts are file-backed;
- SQLite stores compact canonical state, metadata, and pointers;
- high-volume stream data is not stored as one row per stream chunk.

This keeps active read paths compact while preserving durable evidence for
audit, recovery, and output freshness checks.

## 10. Relationship to Existing Proposals

### P079 Contract-Aware Output Repair and Provider Fallback

P095 output collection is the normal second phase. P079 repair/fallback begins
only after output collection is missing, invalid, failed, or unavailable. P079
must not be used as the default first attempt to collect output, and provider
fallback must not replace deterministic readback.

### P086 Agent Work Continuation and Lead-Directed Same-Session Resumption

Continuation is another work turn. Continuation prompts must follow prompt
minimalism and must not include output-contract instructions. After
continuation, normal deterministic readback and output collection may be
required. P086 preserves useful work context; P095 defines the normal
work/output separation.

### Code-Writer Completion Contract and Output Freshness

P095 makes work completion and output settlement separate facts. Changed files,
test results, and tool traces prove work happened; they do not prove output
settled. Fresh output must come from the P095 output collection turn or a valid
[P079 repair or retained-P088 completion path](../reference/output-contracts-failure-evidence-and-recovery.md#code-writer-completion-freshness-and-repair-p088-retained-alias).
Stale artifacts from previous attempts remain invalid.

### Workflow-Owned Quality-Gate Boundary Contract

Missing output after a work turn should route through P095 output collection and
P079 repair before blocker-boundary classification. Boundary assessment must
distinguish:

- work not done;
- work done but output not collected;
- output collected but the gate still blocked.

Workflow transitions remain workflow-owned, and human approval remains
accept/reject only. The stable behavior is owned by
[`workflow-execution-engine.md`](../reference/workflow-execution-engine.md#quality-gate-blocker-boundary-transitions)
and
[`output-contracts-failure-evidence-and-recovery.md`](../reference/output-contracts-failure-evidence-and-recovery.md#workflow-owned-quality-gate-boundary-contracts).

## 11. Acceptance Criteria

P095 is implementation-ready only when a later implementation pass can prove:

- `code_writer` can run a work turn with no output contract injection;
- server readback captures changed files, tests, blockers, generated artifacts,
  traces, and pre/post fingerprints;
- output collection turn can settle required outputs in the same provider
  session;
- daemon restart between work and output collection does not cause automatic
  fresh code retry;
- session resurrection is attempted only when the runtime/provider supports it;
- output collection can fall back to a fresh output-collector session from
  deterministic readback when same-session resurrection is unavailable and
  safe;
- output collection turn uses read-only policy where supported;
- release, publish, git-push, upload, and distribution stages are excluded;
- stale artifacts cannot satisfy fresh output;
- GraphQL exposes readback only, with no new SwiftUI mutations;
- P079 repair is not used as the default first output-collection attempt.

## 12. Non-Goals

- Do not implement code changes in this proposal.
- Do not change release side-effect safety.
- Do not weaken output contracts.
- Do not replace the implemented P079 repair, P086 continuation, code-writer completion-freshness, or workflow-owned quality-gate boundary contracts.
- Do not add SwiftUI mutations.
- Do not require this mode for all agents.
- Do not create a generic prompt-rule engine.
- Do not move high-volume evidence into SQLite rows.

## 13. Open Questions

1. Which providers can support same-session output collection without keeping a
   live ACP handle?
2. Should output collection turns have a separate provider/session budget from
   work turns?
3. Should the first implementation gate be limited to `code_writer`, or should
   it include a docs-only helper fixture to prove the future extension shape?
