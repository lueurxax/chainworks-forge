# Proposal 086: Provider Session Resurrection Completion

| Field | Value |
|---|---|
| Date | 2026-05-07 |
| Status | Draft |
| Author | Engineer (single-engineer project) |
| Depends on | Implemented live-handle continuation contract in [Agent Work Continuation](../reference/agent-work-continuation.md), [ACP Runtime Transport](../reference/acp-runtime-transport.md), [Local persistence write-budget contract](../reference/rust-control-plane.md#sqlite-write-serialization-and-gateway-dbwriter), [durable side-effect ledger](../reference/rust-control-plane.md#durable-side-effect-ledger) |
| Related | P079 Contract-Aware Output Repair, P080 Continuous Stale Execution Reconciliation, P093 Agent Work Continuation Expansion Soak, P095 Two-Phase Agent Invocation |
| Scope | Complete the remaining `provider_session_resurrection` implementation: start a new Chainworks-managed ACP process and attach/resume a known provider session id for adapters that support it, while preserving the existing fail-closed unsupported behavior for adapters that do not. |
| Non-goal | Do not re-implement already shipped live-handle continuation, continuation readback, lead-auto live-handle admission, continuation metrics plumbing, or Phase 5 soak/scale evidence. P093 owns soak/scale only and must not be treated as owning provider-session resurrection implementation. |
| Goal | Allow an operator to continue useful code-writer work from a provider session id even after Chainworks no longer owns the live ACP handle, without falling back to a fresh retry or losing provenance. |

---

## 1. Current Implemented Baseline

The following behavior is already implemented and is therefore not the remaining
P086 work:

- `agents.continue_work` exists as the server-owned MCP command.
- `live_handle_continuation` can send an additional prompt through an existing
  live ACP handle when Chainworks still owns that handle.
- Continuation rows, request fingerprints, idempotent admission, side-effect
  ledger ordering, supervised-worker records, continuation artifacts, metrics,
  readback, and GraphQL query surfaces exist.
- `lead_auto` is limited to live-handle continuation and validates a structured
  decision artifact before admission.
- Unsupported `provider_session_resurrection` requests fail closed with
  `provider_session_resurrection_unsupported` and must not become ordinary
  retry, output repair, checkpoint rehydration, or `SessionReuseDisposition::Reused`.
- The SwiftUI app remains read-only for continuation state and does not expose
  a continuation mutation surface.

This proposal keeps that baseline intact and closes the remaining successful
resurrection path.

## 2. Problem

The current system has the typed `provider_session_resurrection` mode but no
adapter can successfully attach/resume a provider session id after the live ACP
handle is gone. This leaves a real product gap:

- If output repair or continuation should happen after the ACP subprocess was
  closed, the only durable context is the provider session id.
- A normal retry starts a fresh provider session and can spend substantial time
  rediscovering proposal context, repository state, tests, and current blockers.
- The explicit fail-closed unsupported mode is safer than a silent retry, but it
  is not a full implementation of provider-session continuity.

The remaining P086 work is to turn the explicit mode from "recognized but
unsupported" into a working adapter-backed continuation path where provider
support exists.

## 3. Required Behavior

### 3.0 Continuity Mode Architecture

P086 must make the continuation modes explicit and mutually distinguishable.
Provider-session resurrection is not ordinary retry and must not be implemented
as a best-effort variant of live-session reuse.

The runtime must classify every post-failure continuation attempt as exactly one
of these modes:

| Mode | Meaning | Allowed session source |
|---|---|---|
| `normal_fresh_execution` | Start a new execution attempt with no provider memory dependency. | New provider session only. |
| `normal_live_reuse` | Send another prompt through a live ACP handle that Chainworks still owns and whose previous prompt boundary settled cleanly. | Existing live ACP handle. |
| `provider_session_resurrection` | Start a new Chainworks-managed ACP subprocess and attach/resume a recorded provider session id after the old ACP handle is gone. | New ACP process attached to old provider session id. |
| `output_only_recovery` | Recover missing or malformed required outputs from useful work already performed; do not redo implementation work unless explicitly allowed. | Live reuse or provider-session resurrection, with explicit recovery receipt. |

A normal retry must never silently reuse an ambiguous provider session after
`prompt_closed_during_stream`, `transport_closed`, `provider_timeout`, failed
settlement, or cancellation. Those cases may still preserve token savings, but
only through an explicit resurrection or output-only recovery path with durable
proof. This preserves the session-continuity benefit without letting retry,
reuse, and recovery collapse into an un-auditable mixed mode.

The execution and report surfaces must expose the selected mode, the reason it
was selected, and why other modes were rejected. In particular:

- failed or ambiguous prompt boundaries make the current live ACP handle
  ineligible for `normal_live_reuse`;
- the same provider session id may still be eligible for
  `provider_session_resurrection` if adapter and catalog gates pass;
- output-only recovery must state whether source edits are forbidden or
  explicitly allowed;
- any fallback from resurrection/recovery to a fresh retry requires a separate
  operator-visible decision and must not happen automatically.

### 3.1 Adapter Capability Contract

Each ACP adapter must declare whether it supports provider-session resurrection.

The canonical owner is the Rust ACP adapter boundary in
`control-plane/crates/acp/src/adapters/mod.rs`. Add a versioned contract such
as `ProviderSessionResurrectionCapability` and expose it through `AcpAdapter`
before MCP admission or the continuation worker can attempt resurrection.

The capability declaration must include:

- provider family and adapter id;
- capability schema/version string;
- whether attach/resume by provider session id is supported;
- required launch arguments, session/new fields, or environment values;
- the typed attach request shape accepted by the adapter;
- the typed attach result shape returned by the adapter;
- whether the adapter can prove the resumed provider session id after attach;
- the authoritative proof source used for requested-vs-actual identity;
- whether the resumed session is safe for write-enabled code-writer work;
- failure classes for unsupported, rejected, expired, mismatched, or unverifiable
  provider sessions.

The failure classes must be typed, persisted, and visible to the MCP admission
path and continuation worker. At minimum they must distinguish:

- `unsupported`;
- `provider_rejected`;
- `expired_or_missing_session`;
- `actual_session_mismatch`;
- `identity_unverifiable`;
- `quota_hold`;
- `auth_failure`;
- `launch_failed`;
- `orphan_reap_failed`;
- `attach_receipt_persist_failed`.

Adapters with no proven attach/resume mechanism must continue to fail closed
with `provider_session_resurrection_unsupported`.

### 3.2 Frozen Run Catalog Capability Gate

Adapter support is necessary but not sufficient. Provider-session resurrection
may be admitted only when the frozen run catalog for the target run explicitly
opts the `code_writer` into that mode.

The admission gate must require both:

1. The selected ACP adapter declares a supported, enabled
   `ProviderSessionResurrectionCapability`.
2. The run's frozen catalog snapshot contains
   `code_writer.continuation_capability.enabled = true` and
   `code_writer.continuation_capability.provider_session_resurrection.enabled =
   true`, with the requested `trigger_kind` allowed and any required session
   fields satisfied.

Old snapshots, missing `continuation_capability`, missing
`provider_session_resurrection`, disabled subtrees, malformed catalog JSON,
missing `code_writer`, trigger mismatch, or unsatisfied
`require_recorded_provider_session_id` must fail closed before any resurrection
work item is enqueued.

This gate uses frozen run truth, not the current repository catalog. Updating
`examples/agents/agents.yaml` only affects new runs; existing runs remain bound
to their captured catalog snapshot unless an explicit governed snapshot-migration
mechanism is added by a separate proposal.

Existing `live_handle_continuation` catalog behavior remains unchanged. Enabling
provider-session resurrection must not weaken the current live-handle checks for
`continuation_capability.enabled`, allowed triggers, live-handle mode opt-in, or
required live session presence.

### 3.3 Claude Provider Resurrection

Claude is the first required supported adapter because current Chainworks runs
use Claude code-writer sessions and the provider exposes session-id continuity.

The Claude ACP adapter must be able to:

1. Start a new Chainworks-managed `claude-agent-acp` subprocess.
2. Ask Claude to resume the recorded provider session id.
3. Prove that the new ACP session is attached to the requested provider session
   id before sending the continuation prompt.
4. Reject the continuation if the provider reports a different session id, an
   expired session, missing local session store, quota hold, auth failure, or an
   unsupported resume path.

The implementation must not depend on the old ACP subprocess still being alive.

The proposal is not satisfied by a launch flag alone. The Claude adapter must
define the exact proof source that binds the new ACP process to the requested
provider session id. The first acceptable implementation must use a provider
session identity returned or observable after attach, for example a
`session/new` response field, an ACP event field, or a Claude session-store
readback that can be tied to the newly opened ACP session. The attach result
must record both:

- `requested_provider_session_id`;
- `actual_provider_session_id`;
- `identity_proof_source`;
- `identity_proof_observed_at`;
- `identity_proof_artifact_id` when the proof is read from a file or transcript.

If the adapter cannot observe an actual provider session id and prove it equals
the requested id, the resurrection must fail with `identity_unverifiable` before
any continuation prompt is sent.

Claude resurrection must also support provider session-store recovery as a
first-class evidence source. When ACP loses the terminal response or closes
during an active prompt, Chainworks must be able to read the Claude session
store for the requested provider session id, bind the observed transcript to the
target run/stage/agent execution, and recover terminal work/output evidence when
possible.

Session-store recovery must record:

- provider session-store root and resolved transcript path;
- transcript read timestamp and digest;
- latest observed provider turn/message id if available;
- latest observed tool/background-task activity if available;
- whether a terminal answer, `CHAINWORKS_OUTPUT`, or direct-file manifest was
  recovered;
- whether recovered content belongs to the target request by prompt marker,
  request fingerprint, stage execution id, agent execution id, or another
  documented proof source;
- the reason recovery failed when the transcript is missing, truncated,
  ambiguous, or belongs to another execution.

The recovered transcript may be used to settle outputs only when ownership is
machine-checkable. If ownership cannot be proven, resurrection may still ask a
short output-only repair question in the same provider session, but must not
pretend the transcript was canonical settlement evidence.

### 3.4 Generic Resurrection Flow

`agents.continue_work` with `continuation_mode=provider_session_resurrection`
must:

1. Validate the target `agent_execution_id`, run, stage execution, agent id,
   provider family, model family, worktree root, and provider session id.
2. Verify frozen catalog opt-in for `code_writer` provider-session resurrection
   before enqueueing work.
3. Verify the target is a stage-owned `code_writer` execution and the stage is
   not a release, publish, upload, distribution, commit, push, security,
   prepush-review, or lead-orchestration lane.
4. Reject if unresolved side-effect ledger rows, pending approvals, active
   continuations, mismatched worktree, or provider-family quota holds make the
   continuation unsafe.
5. If daemon restart left a stale ACP subprocess, reap or fail closed with
   durable orphan-reap evidence before attempting resurrection.
6. Start a new managed ACP process through the adapter resurrection path.
7. Persist a provider-session attach receipt before the continuation prompt is
   sent.
8. Persist a prompt-turn marker that includes the continuation id, target
   `agent_execution_id`, target `stage_execution_id`, request fingerprint, and
   provider request/turn id when the adapter exposes one.
9. Send the canonical P086 mode-reset continuation prompt through the resumed
   session.
10. Correlate every terminal response or recovered transcript with the persisted
   prompt-turn marker before settlement.
11. Settle the continuation using the existing continuation artifact/readback
   path, never by creating a normal stage retry.

Request ids are necessary but not sufficient. JSON-RPC request ids prove the
ACP transport response while the transport is intact. After process closure or
session-store recovery, the engine must correlate by a stronger prompt-turn
receipt: Chainworks continuation id, request fingerprint, stage execution id,
agent execution id, provider session id, provider request/turn id when
available, and transcript proof source. If any required correlation field is
missing or contradictory, settlement must fail closed instead of attributing an
old answer to a new attempt.

### 3.5 Output Repair Use Case

Provider-session resurrection must support the P079/P088 output-repair shape:

- a code-writer produced useful work or a useful completion;
- the machine-readable `CHAINWORKS_OUTPUT` was malformed or incomplete;
- Chainworks no longer has a live ACP handle;
- the operator wants a short continuation that returns only corrected required
  outputs, using the provider session that already did the work.

The repair prompt must explicitly forbid additional code edits unless the
operator instruction allows them.

Output-only recovery is a distinct continuation purpose, not a generic retry.
It must ask only for the missing or invalid required outputs, include the
already captured work summary when available, and avoid repeating the full
implementation task. If canonical direct-file artifacts already exist and pass
contract validation, the engine must preserve them and request only the missing
pieces.

Output-only repair also needs machine-checkable proof, not only prompt wording.
For output-only resurrection requests, Chainworks must capture a pre/post
worktree source snapshot or equivalent diff summary and record
`changed_source_files == 0`. If source edits are intentionally allowed by the
operator, the request and receipt must say so explicitly and list the changed
source files.

### 3.6 Durable State And Replay Contract

Provider-session resurrection must be recoverable across daemon crashes without
duplicating the provider prompt and without falling back to retry.

The existing `agent_work_continuations.status` lifecycle remains authoritative
and backward-compatible. Do not add these resurrection phases as new `status`
values. The current status contract (`accepted`, `queued`, `starting`,
`running`, `preflight_passed`, `prompt_sent`, `observing`,
`worktree_observed`, `needs_continuation_reconciliation`, `finalizing`,
`cancelling`, and terminal states) continues to drive generic continuation
readback.

Provider-session resurrection adds a separate nullable typed substate:

```text
agent_work_continuations.resurrection_phase TEXT NULL
```

The field is populated only when
`mode = 'provider_session_resurrection'`. It must have a DB `CHECK` constraint,
typed Rust enum, MCP/GraphQL/report readback, and receipt mirroring. Existing
`live_handle_continuation` rows must keep `resurrection_phase = NULL`.

Allowed resurrection phases:

1. `admitted`: request fingerprint accepted; no process spawned.
2. `launching`: managed ACP process spawn requested.
3. `launched`: new child pid/process group recorded.
4. `attaching`: adapter attach/resume request started.
5. `attached_unprompted`: requested and actual provider session ids verified;
   attach receipt persisted; no continuation prompt has been sent.
6. `prompting`: continuation prompt send has started and prompt artifact id is
   durable.
7. `settling`: provider response is being collected and artifacts are being
   settled.
8. `completed` or `failed_closed`.

Compatibility mapping to the existing status lifecycle:

| `resurrection_phase` | Compatible `status` values |
|---|---|
| `admitted` | `accepted`, `queued` |
| `launching` | `starting`, `running` |
| `launched` | `starting`, `running` |
| `attaching` | `starting`, `running`, `preflight_passed` |
| `attached_unprompted` | `preflight_passed` |
| `prompting` | `prompt_sent` |
| `settling` | `observing`, `worktree_observed`, `needs_continuation_reconciliation`, `finalizing` |
| `completed` | `succeeded`, `no_progress` |
| `failed_closed` | `failed`, `cancelled` |

`attached_unprompted` is the critical replay boundary: identity proof and the
attach receipt are durable, but the provider prompt has not been sent.
`prompting` may be entered only after the prompt-send marker and prompt artifact
id are durable and `status` has advanced to `prompt_sent`.

Replay rules:

- A crash in `admitted` can retry admission idempotently.
- A crash in `launching` or `launched` must reap the managed child if it exists,
  record orphan-reap evidence, and restart attach from a clean process.
- A crash in `attaching` must either prove the child never received a prompt or
  fail closed; it must not assume prompt safety.
- A crash in `attached_unprompted` may send the prompt once after verifying the
  attach receipt, live child ownership, and prompt-not-sent marker.
- A crash in `prompting` or later must not send the prompt again unless the
  persisted prompt-send record proves the provider did not receive it. If that
  cannot be proven, fail closed and require operator action.
- No replay path may create a normal retry or output-repair attempt as a
  fallback for resurrection failure.

The claim/replay implementation must classify both fields together. A row with
`status = prompt_sent` or later is never rewound to a pre-prompt phase. A row
with `resurrection_phase = attached_unprompted` may advance to prompt only
after revalidating managed process ownership, provider identity proof, and
prompt-not-sent evidence.

## 4. Data And Evidence

A successful resurrection must write durable evidence that lets an operator and
future audit distinguish it from retry:

- continuation id;
- source `agent_execution_id`;
- source `session_generation_id`;
- requested provider session id;
- actual provider session id after attach;
- adapter capability version;
- old ACP process/orphan reap outcome if applicable;
- new ACP child pid and process-group id;
- attach attempt timestamps and result;
- canonical request artifact id;
- attach receipt artifact id;
- response/result artifact ids;
- no-progress or failure reason when the attach or prompt does not complete.

The provider-session attach receipt schema must be tightened or versioned. A
passing resurrection receipt cannot rely on `additionalProperties` to carry the
important audit fields. Either evolve
`docs/reference/p086/schemas/artifacts/provider_session_attach_receipt_v1.schema.json`
for resurrection-specific required fields or add
`provider_session_attach_receipt_v2.schema.json`.

For `mode = provider_session_resurrection`, the receipt schema must require:

- `requested_provider_session_id`;
- `actual_provider_session_id`;
- `identity_proof_source`;
- `identity_proof_observed_at`;
- `adapter_id`;
- `adapter_capability_version`;
- `attach_request_id` or equivalent idempotency key;
- `managed_child_pid`;
- `managed_process_group_id`;
- `target_agent_execution_id`;
- `target_stage_execution_id`;
- `request_fingerprint_sha256`;
- `prompt_turn_marker_id`;
- `provider_request_id` or `provider_turn_id` when exposed by the adapter;
- `session_store_transcript_path` when session-store recovery was attempted;
- `session_store_transcript_digest` when session-store recovery was attempted;
- `session_store_recovery_result`;
- `process_started_at`;
- `attach_started_at`;
- `attach_completed_at`;
- `prompt_sent_at` as nullable or absent until prompt send;
- `resurrection_phase`;
- `orphan_reap_required`;
- `orphan_reap_verified`;
- typed `failure_class` when attach fails.

Readback must expose the same fields through MCP/GraphQL/report surfaces without
requiring operators to inspect raw JSON artifacts.

Metric/readback surfaces must count:

- resurrection requested;
- unsupported;
- attach success;
- attach failure;
- prompt sent after resurrection;
- no-progress after resurrection;
- useful-progress after resurrection;
- fresh retry avoided.

Existing metrics that only count unsupported attempts are insufficient for this
proposal.

## 5. Safety Rules

Provider-session resurrection must fail closed when any of these are true:

- adapter capability is absent or disabled;
- provider session id is missing, malformed, redacted, expired, or does not
  match the target agent execution;
- provider resumes a different session id than requested;
- worktree root or provider family does not match durable target truth;
- unresolved side-effect rows or pending approvals exist for the target run or
  stage;
- stale ACP orphan reap is required but cannot be verified;
- quota/auth/runtime health prevents a safe attach;
- the continuation would touch a release, publish, upload, distribution, commit,
  push, prepush-review, security, or lead-orchestration lane.

Fail-closed means no provider prompt is sent and no fresh retry is scheduled as
a fallback.

## 6. Tests

Required tests and evidence:

1. Unit test: unsupported adapters still reject
   `provider_session_resurrection` with `provider_session_resurrection_unsupported`.
2. MCP admission test: a frozen catalog with
   `code_writer.continuation_capability.provider_session_resurrection.enabled =
   false` rejects provider-session resurrection before work enqueue.
3. MCP admission test: old snapshots, missing `continuation_capability`, missing
   `provider_session_resurrection`, malformed catalog JSON, missing
   `code_writer`, trigger mismatch, or missing required provider session id fail
   closed before work enqueue.
4. Compatibility test: existing `live_handle_continuation` catalog behavior
   remains unchanged while provider-session resurrection is independently
   gated.
5. Unit test: Claude adapter builds the correct resume/attach launch/session
   request for a provider session id.
6. Integration test: supported resurrection starts a new managed ACP process and
   records requested and actual provider session ids.
7. Integration test: mismatched resumed provider session id rejects before
   prompt send.
8. Integration test: missing/expired provider session rejects before prompt
   send with a typed failure reason.
9. Integration test: stale ACP process is reaped or the resurrection fails
   closed before attach.
10. Continuation worker test: resurrection is recorded as
   `provider_session_resurrection`, not retry, output repair, checkpoint
   rehydration, or normal session reuse.
11. Mode classification test: `prompt_closed_during_stream`,
    `transport_closed`, `provider_timeout`, failed settlement, and cancellation
    are ineligible for silent `normal_live_reuse`; they may only enter
    `provider_session_resurrection` or `output_only_recovery` after explicit
    admission gates pass.
12. Prompt-turn correlation test: settlement rejects recovered terminal output
    when request fingerprint, stage execution id, agent execution id, provider
    session id, or provider request/turn id proof is missing or contradictory.
13. Claude session-store recovery test: a lost ACP terminal response can be
    recovered from the Claude session transcript only when the transcript is
    bound to the target execution by prompt marker/request fingerprint and the
    recovered output passes contract validation.
14. Claude session-store ambiguity test: transcript evidence from the same
    provider session but a different target execution fails closed and cannot be
    attributed to the current retry.
15. Output-repair test: malformed `CHAINWORKS_OUTPUT` can be corrected through a
   resurrected provider session without changing source files when the operator
   asked for output-only repair.
16. Output-only repair test: source snapshot evidence records
   `changed_source_files == 0`; a deliberately allowed source-edit request must
   record the explicit operator allowance and changed file list.
17. Crash/replay tests: crashes in `launching`, `launched`, `attaching`,
    `attached_unprompted`, and `prompting` follow the replay rules without
    duplicate prompt send and without normal retry fallback.
18. DB/API compatibility test: existing live-handle continuation statuses remain
    accepted by the DB and readback surfaces; provider-session resurrection rows
    persist a typed `resurrection_phase` without adding new
    `agent_work_continuations.status` values.
19. Replay classification test: `attached_unprompted` is distinguishable from
    `prompt_sent` through DB, MCP, GraphQL, reports, and receipt readback.
20. Receipt schema test: resurrection receipts fail schema validation if they
    omit requested id, actual id, proof source, adapter capability version,
    prompt-turn marker, target execution ids, request fingerprint, process
    ownership evidence, session-store recovery result, timestamps, or typed
    failure class.
21. Readback test: MCP/GraphQL/report surfaces expose attach receipt, actual
    provider session id, resurrection phase, and resurrection result.
22. Proposal gate: `./scripts/test-gate.sh proposal-086` covers the above or a
    focused `proposal-086-resurrection` gate is added and documented.

## Relationship to P095: Two-Phase Agent Invocation

P095 makes work and output settlement separate normal phases. In that model,
P086 continuation is another work turn, not an output collection turn.

Continuation prompts must follow P095 prompt minimalism:

- continuation prompts carry the work objective and completion target;
- continuation prompts do not include output artifact paths;
- continuation prompts do not include `CHAINWORKS_OUTPUT` instructions;
- continuation prompts do not rely on long negative-rule lists for safety.

After a continuation turn completes or blocks, the server may still need to run
normal deterministic readback and output collection before the execution can
settle. P086 extends useful work context through live-handle or provider-session
continuity; P095 defines the normal work/output separation that follows any
work turn, including continuation.

## 7. Acceptance Criteria

1. At least Claude provider-session resurrection is implemented and enabled by
   both explicit adapter capability and frozen run catalog
   `code_writer.continuation_capability.provider_session_resurrection` opt-in.
2. `agents.continue_work` can continue a code-writer execution by known provider
   session id after Chainworks no longer owns the live ACP handle.
3. The resumed provider session id is verified before any continuation prompt is
   sent.
4. Unsupported adapters and unsafe targets continue to fail closed without
   falling back to fresh retry.
5. Old snapshots, missing/malformed catalog capability fields, disabled
   provider-session resurrection, trigger mismatch, or missing required provider
   session id reject before work enqueue.
6. Existing live-handle catalog behavior remains unchanged.
7. Resurrection writes attach receipts, process ownership evidence, output
   artifacts, and readback data sufficient to audit the path end to end.
8. Resurrection is separately identifiable in metrics and reports.
9. The output-repair use case is proven for malformed final
   `CHAINWORKS_OUTPUT`, including machine-checkable no-source-change proof for
   output-only repair.
10. Prompt-turn correlation proves recovered outputs belong to the target
   continuation before settlement; request id alone is not treated as sufficient
   after ACP transport loss.
11. Claude session-store recovery can recover terminal output or fail closed
   with explicit ambiguity/missing-transcript evidence.
12. Crash/replay evidence proves no duplicate prompt send and no fresh retry
   fallback across resurrection phases.
13. Existing continuation `status` values remain backward-compatible; resurrection
   replay uses typed `resurrection_phase` readback instead of overloading the
   generic status lifecycle.
14. Canonical proposal gate evidence passes on the same tree.

## 8. Non-Goals And Follow-Up Ownership

P093 owns only soak/scale after this implementation is complete:

- 14-day no-hold soak;
- SLO-budget validation;
- 100 successful continuations across 30 runs;
- expansion readiness decisions.

P093 does not own implementation of provider-session resurrection. Until this
proposal is implemented, provider-session resurrection is unfinished P086 scope,
not an acceptable "Ready with Risks" tail.

Future support for additional providers may be added by separate provider
adapter proposals after Claude is proven, but the generic contract and at least
one production-relevant supported adapter must be completed here.
