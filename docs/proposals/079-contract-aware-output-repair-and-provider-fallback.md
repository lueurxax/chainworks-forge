# Proposal 079: Contract-Aware Output Repair and Provider Fallback

| Field | Value |
|---|---|
| Date | 2026-04-30 |
| Status | Draft |
| Author | Codex |
| Depends on | P057/P058 output settlement and artifact claims, P063 MCP field shaping, P065 operator retry instructions, P076 auto-retry observation ledger |
| Related | P017 workflow conflict mediation, P069 discovery diagnostics UI, `docs/reference/output-contracts-failure-evidence-and-recovery.md`, `docs/reference/artifact-discovery-and-settlement-optimization.md` |
| Scope | Make missing/invalid required agent outputs recoverable inside the invocation lifecycle before a run becomes durably blocked. |
| Non-goal | No automatic human approvals, no acceptance of invalid artifacts, no live-provider-only acceptance gate, and no replacement for P076 observation/cooldown policy. |

## 1. Problem

Chainworks runs repeatedly stall on output-contract failures:

- `missing_required_outputs`
- `no_output_produced`
- `empty_output`
- contract enum/shape mismatches
- proposal/review agents that complete useful reasoning but omit the required artifact envelope

P076 correctly observes and deduplicates these signatures, but observation does not remove the root cause. Today, a run can become blocked even when the same provider session still has enough context to produce the missing artifact with a narrow corrective instruction.

The expensive failure mode is:

1. an agent performs substantial work,
2. the provider response does not satisfy the declared output contract,
3. the stage is marked failed/blocked,
4. auto-retry or the operator starts a fresh attempt,
5. useful context is lost or repeated, and the workflow spends another full provider invocation.

This is not a human approval problem and not a generic retry problem. It is an invocation settlement problem: before the system closes the invocation as failed, it should try to repair contract output in the same session, validate the repaired output through the same P057/P058 contract pipeline, and only then fall back to controlled retry or provider fallback.

## 2. Current Implementation Baseline

The repository already contains a partial output-contract repair path in the Rust executor:

- runtime prompts list exact required output names and canonical output paths,
- the executor can issue an `Output Contract Repair` prompt to the same ACP session,
- fixture tests cover a repair turn that emits a valid `CHAINWORKS_OUTPUT` block,
- P057/P058 settlement values distinguish `missing_required_outputs`, `invalid_required_outputs`, and valid outputs from failed executions.

That baseline is useful but not yet a complete proposal-level contract. The missing durable behavior is:

- role coverage is not specified;
- provider fallback after same-session repair failure is not specified;
- transcript/provider-envelope extraction is not specified;
- repair attempt evidence/readback is not specified;
- retry budget interaction is not specified;
- provider mode mismatch is not classified separately from ordinary missing output;
- P076 does not have a runtime owner to route repeated output-contract signatures to.

P079 makes this behavior explicit and bounded.

## 3. Goals

- Attempt same-session corrective output repair before durable stage failure for eligible output-contract failures.
- Recover contract-valid outputs already present in the transcript or provider envelope when safe.
- Fall back to a controlled alternate-provider attempt only after same-session repair is unavailable or unsuccessful.
- Cover the high-volume first-stage roles:
  - `proposal_writer`
  - `proposal_reviewer_*`
  - `lead_orchestrator`
- Preserve P057/P058 as the only authority for artifact validity and source-generation ownership.
- Preserve human approval gates as human gates.
- Expose repair/fallback evidence through MCP/GraphQL reports so auto-retry can stop treating every missing output as a generic blocked run.
- Validate with deterministic fixture ACP transports, not live providers.

## 4. Non-Goals

- Do not accept invalid, partial, stale, or schema-mismatched output.
- Do not infer operator approval from repaired output.
- Do not rerun arbitrary implementation work inside the repair prompt.
- Do not make live Claude/Gemini/Codex behavior part of the required gate.
- Do not replace P076 observation ledger, retry cooldown, or known-issue catalog.
- Do not change release side-effect retry safety; that belongs to the implemented durable side-effect ledger contract.
- Do not broaden legacy artifact discovery beyond the declared output contract.

## 5. Eligibility

### 5.1 Eligible Failure Classes

Same-session repair is allowed only when all of the following are true:

- the invocation has declared required outputs;
- validation failed with one of:
  - `no_output_produced`
  - `empty_output`
  - `missing_required_outputs`
  - `output_contract_mismatch`
  - `invalid_required_outputs` where the invalidity is schema/enum/field repairable;
  - `provider_mode_mismatch` when the provider produced work in a mode that cannot satisfy the declared output channel.
- the source-generation claim for the invocation is still active;
- the provider session generation is still live or explicitly repairable;
- no unresolved human approval or workflow conflict is the actual blocking condition.

`provider_mode_mismatch` has typed subtypes:

| Subtype | Meaning |
|---|---|
| `plan_event_instead_of_output` | Provider emitted a planning event or plan-mode response instead of a required output payload. |
| `empty_submit_after_plan` | Provider wrote or described a plan, then submitted an empty or output-less final response. |
| `file_plan_written_instead_of_payload` | Provider wrote a plan file, such as `.junie/plans/*.md`, but did not emit the required canonical output. |
| `repair_repeated_plan_behavior` | A repair prompt repeated plan-mode behavior instead of producing the missing output payload. |

### 5.2 Ineligible Cases

Repair must be skipped when:

- the run is waiting on a human approval;
- a workflow conflict requires operator transition selection;
- the source-generation claim was superseded or closed;
- the missing output belongs to an old attempt;
- output validation failed because an artifact was intentionally rejected by an operator override;
- the provider failure class indicates unsafe continuation, such as an unrecoverable transport/session corruption that invalidates session context.

### 5.3 Junie Strict Structured Output Exception

Junie plan-mode behavior is useful evidence, but it is not reliable as a same-session repair path for strict structured outputs.

If all of the following are true:

- adapter family is `junie`;
- required output mode is `strict_structured`;
- the failure class is `provider_mode_mismatch` or the captured material only contains provider plan evidence;

then same-session repair must be skipped:

```json
{
  "same_session_repair": {
    "attempted": false,
    "result": "skipped_ineligible",
    "reason": "provider_mode_mismatch_risk"
  }
}
```

When an allowed fallback provider is configured, provider fallback is immediately eligible after this skip. The runtime must not spend the single repair turn asking Junie to convert a plan-mode response into a strict structured output, because repeated repair prompts have already shown a risk of producing another plan rather than the required payload.

## 6. Same-Session Repair

When an eligible invocation fails output validation, the executor must not immediately settle the stage as failed.

Instead it issues one narrow corrective turn to the same ACP session:

- include only the failed output names and contract ids;
- include exact canonical output paths;
- include validation errors and missing fields;
- instruct the agent not to redo unrelated work;
- require `CHAINWORKS_OUTPUT` blocks only for the failed outputs;
- preserve runtime identifiers from the original invocation contract;
- reject output that targets a non-canonical path.

The repair turn may produce only corrected outputs. Any additional narrative is ignored unless it is inside the declared output envelope.

If a repair turn produces another provider plan instead of the missing payload, the failure class becomes `provider_mode_mismatch` with subtype `repair_repeated_plan_behavior`.

Default same-session repair budget:

```text
max_same_session_repair_turns_per_invocation = 1
```

This can be increased later only through a separate proposal or explicit config gate.

## 7. Transcript and Provider-Envelope Recovery

Before invoking provider fallback, the executor may recover a missing artifact from already captured provider material if the payload is contract-valid.

Allowed recovery sources:

- ACP agent message chunks captured for the current invocation;
- provider result envelope fields captured for the current invocation;
- bounded transcript excerpts already associated with the current agent execution.

Required safeguards:

- recovered payload must pass the same contract validator as normal output;
- recovered payload must map to a declared output name and canonical target path;
- recovered payload must be attributed to the current `agent_execution_id`;
- recovered payload must not come from previous stage attempts, prior session memory, or broad workspace scanning;
- recovery must record `recovery_source = transcript` or `provider_envelope` in readback evidence.

Transcript/provider-envelope recovery is not a substitute for contract validation. It is only a parser for material already emitted by the current invocation.

### 7.1 Provider Plan Evidence Is Not Output Truth

Provider plan files are preserved evidence, not canonical required outputs.

For Junie and any future plan-mode provider:

```text
.junie/plans/*.md
```

must be handled as:

- human evidence artifact;
- provider plan evidence;
- diagnostic context for the next invocation or operator review.

It must not be handled as:

- a valid required output;
- transition truth;
- a substitute for `CHAINWORKS_OUTPUT` or another declared strict structured payload;
- source material that updates active artifact truth without passing the declared output contract.

If `.junie/plans/*.md` is the only material produced for a required output, the settlement result remains missing/invalid output and should be classified as `provider_mode_mismatch:file_plan_written_instead_of_payload`.

## 8. Controlled Provider Fallback

If same-session repair is unavailable or fails, and the role is in the P079 initial role allowlist, the engine may schedule one controlled provider fallback attempt.

Fallback rules:

- Fallback is a fresh invocation with the same declared output contract.
- Fallback receives a compact, sanitized context packet:
  - original task summary,
  - validation failure summary,
  - required output contract,
  - relevant prior valid artifacts,
  - no raw secrets,
  - no operator rationale.
- Fallback must not mutate human approval decisions.
- Fallback must not bypass loop budgets or workflow conflict gates.
- Fallback must be counted separately from ordinary stage retry.
- Fallback must preserve source-generation ownership; only the successful fallback execution may update active artifact truth.

Default fallback budget:

```text
max_provider_fallback_attempts_per_invocation = 1
```

Provider selection is deterministic and policy-driven. The runtime may choose an alternate provider only from the agent catalog/backend profile policy for that role. If no allowed alternate provider exists, fallback is skipped and the original failure proceeds to normal stage failure.

Initial lead-orchestrator fallback policy:

| Role | Failed provider profile | Fallback provider profile |
|---|---|---|
| `lead_orchestrator` | `gemini_reasoning_pro_high` | `claude_orchestrator_high` |

This fallback is only for output-contract settlement failures covered by P079, including `provider_mode_mismatch` and Junie strict structured output skips. It is not a general quality retry, capacity retry, or approval bypass.

## 9. Role Coverage

Initial P079 coverage is intentionally narrow:

| Role family | Repair | Transcript recovery | Provider fallback |
|---|---:|---:|---:|
| `proposal_writer` | yes | yes | yes |
| `proposal_reviewer_*` | yes | yes | yes |
| `lead_orchestrator` | yes | yes | yes |
| `docs_guardian` | yes | yes | follow-up |
| `code_writer` | repair only for structured status artifacts | yes | follow-up |
| `security_checker` | yes | yes | yes |
| `prepush_code_reviewer` | yes | yes | yes |
| release agents | no | no | no |

Release agents are excluded because they may have external side effects. The implemented durable side-effect ledger owns that retry/reconciliation lane.

## 10. Durable Evidence

Each repair/fallback decision must be visible in run reports and diagnostic readback.

Minimum evidence fields:

```json
{
  "schema_version": "output_contract_repair.v1",
  "run_id": "...",
  "stage_execution_id": "...",
  "agent_execution_id": "...",
  "role": "proposal_writer",
  "initial_failure_class": "missing_required_outputs",
  "initial_failure_subtype": null,
  "adapter_family": "codex",
  "required_output_mode": "strict_structured",
  "required_outputs": ["proposal_current"],
  "same_session_repair": {
    "attempted": true,
    "result": "accepted",
    "turn_count": 1,
    "reason": null
  },
  "transcript_recovery": {
    "attempted": false,
    "result": "not_needed"
  },
  "provider_fallback": {
    "attempted": false,
    "result": "not_needed",
    "fallback_provider": null,
    "fallback_profile": null
  },
  "provider_plan_evidence": {
    "artifact_paths": [],
    "accepted_as_output": false
  },
  "final_output_settlement": "valid_outputs_from_completed_execution"
}
```

Accepted result values:

- `not_needed`
- `attempted`
- `accepted`
- `rejected_invalid`
- `unavailable`
- `skipped_ineligible`
- `failed_transport`
- `budget_exhausted`

This evidence must be available through:

- MCP `reports.get`;
- MCP `report://{run_id}`;
- GraphQL run/stage diagnostic readback if the affected surface already exposes execution diagnostics.

P076 may then classify repeated output failures as:

- `repair_unavailable`
- `repair_rejected_invalid`
- `fallback_unavailable`
- `fallback_failed`
- `repair_succeeded`
- `provider_mode_mismatch`
- `provider_plan_evidence_only`
- `fallback_succeeded_after_provider_mode_mismatch`

instead of treating every recurrence as a generic `missing_required_outputs` retry.

## 11. Settlement Rules

P079 must preserve the existing P057/P058 settlement model.

- Valid repaired output settles as normal valid output for the current source generation.
- Invalid repaired output is rejected and does not update active artifact truth.
- Late repair output after source-claim supersession becomes `ignored_late_outputs`.
- Provider fallback creates a distinct agent execution and source-generation claim.
- The previous failed execution remains visible with its original validation failure and repair evidence.
- Stage success requires final required outputs to be contract-valid.
- Provider plan evidence, including `.junie/plans/*.md`, may be attached to the failed execution as evidence but must not settle required outputs or drive transition truth.

No repair or fallback path may write directly to active artifact truth without going through the existing output discovery, contract validation, and source-generation claim path.

## 12. MCP and Operator Semantics

P079 does not add a new broad operator command in the initial slice.

The runtime behavior is automatic inside eligible invocation settlement. Operator-visible surfaces should explain what happened and what remains:

- “same-session repair produced valid required output”
- “same-session repair failed; provider fallback scheduled”
- “provider fallback unavailable; stage blocked”
- “repair skipped because approval/workflow conflict is the blocker”

Future MCP follow-up may add targeted controls such as:

- `agent_outputs.repair`
- `agent_outputs.fallback`

Those are out of scope for P079 unless implementation discovers that automatic runtime repair needs an explicit operator command boundary.

## 13. Acceptance Criteria

- Eligible `proposal_writer`, `proposal_reviewer_*`, and `lead_orchestrator` invocations receive exactly one same-session corrective prompt before durable missing-output failure.
- `provider_mode_mismatch` is a first-class failure class with subtypes for plan-mode output failures.
- Junie strict structured output failures skip same-session repair as `skipped_ineligible(provider_mode_mismatch_risk)` and may immediately use configured provider fallback.
- `.junie/plans/*.md` is preserved as human/provider plan evidence but never accepted as required output or transition truth.
- `lead_orchestrator` supports `gemini_reasoning_pro_high -> claude_orchestrator_high` fallback for P079 output-contract failures.
- Same-session repaired output is accepted only when it passes the existing contract validator and source-generation claim checks.
- Contract-valid output embedded in the current transcript/provider envelope can be recovered without a fresh provider invocation.
- If same-session repair fails and an allowed alternate provider exists, one controlled fallback attempt is scheduled and attributed as a distinct execution.
- Human approvals and workflow conflicts are never auto-resolved by repair/fallback.
- Release agents are excluded from provider fallback in the initial slice.
- Repair/fallback evidence is visible in MCP reports and does not require scraping raw logs.
- P076 auto-retry rollups can distinguish repair/fallback outcomes from raw missing-output failures.

## 14. Test Plan

### Fixture ACP Tests

- Agent omits required output, then same session returns valid `CHAINWORKS_OUTPUT` after repair prompt.
- Agent returns invalid enum, then repair prompt returns a contract-valid enum.
- Agent returns useful JSON in transcript without envelope; transcript recovery accepts it only when contract-valid and attributable.
- Agent returns stale output path; repair rejects it.
- Same-session repair fails; deterministic alternate fixture provider succeeds.
- Same-session repair fails; no alternate provider exists; stage fails with repair evidence.
- Junie strict structured output fixture writes `.junie/plans/*.md` and no payload; repair is skipped as `provider_mode_mismatch_risk`, plan file is attached as provider plan evidence, and the required output remains unsettled.
- Lead orchestrator fixture fails under `gemini_reasoning_pro_high` with `provider_mode_mismatch`, then succeeds under `claude_orchestrator_high`.

### Contract and Settlement Tests

- Repaired output goes through the same contract validator as normal output.
- Late repair output after claim supersession is ignored.
- Provider fallback creates a distinct agent execution/source claim.
- Previous failed execution retains original failure evidence.
- Provider plan evidence from `.junie/plans/*.md` is queryable as evidence but is not accepted by the contract validator as a required output.

### Role and Safety Tests

- `proposal_writer`, `proposal_reviewer_*`, and `lead_orchestrator` are eligible.
- Junie strict structured output mode is ineligible for same-session repair and eligible for immediate configured fallback.
- human approval stages are ineligible.
- workflow-conflict states are ineligible.
- release agents are ineligible for fallback.

### Gate

Add a canonical gate:

```text
./scripts/test-gate.sh proposal-079
```

The gate must use deterministic fixture ACP transports only. It must not require live Claude, Gemini, Codex, Xcode, or network availability.

## 15. Rollout

1. Ship same-session repair evidence and fixture tests.
2. Enable same-session repair for the initial role allowlist.
3. Add transcript/provider-envelope recovery.
4. Add controlled provider fallback for proposal writer/reviewer/lead roles, including `lead_orchestrator` fallback from `gemini_reasoning_pro_high` to `claude_orchestrator_high`.
5. Wire P076 classification to the new evidence fields.
6. After two dogfood cycles, evaluate whether `docs_guardian` and selected `code_writer` status artifacts should join fallback coverage.

## 16. Risks and Mitigations

| Risk | Mitigation |
|---|---|
| Repair prompt causes extra unrelated work | Prompt says “do not redo unrelated work”; executor accepts only declared output envelopes. |
| Invalid output is accepted because it “looks close” | All repaired/recovered/fallback output must pass the existing contract validator. |
| Provider fallback hides real proposal defects | Fallback is limited to output-contract failures, records evidence, and does not bypass workflow conflict or approval gates. |
| Junie plan files are mistaken for output truth | `.junie/plans/*.md` is evidence only; strict structured required outputs remain unsettled until a canonical payload validates. |
| Same-session repair repeats provider plan-mode behavior | Junie strict structured output failures skip same-session repair and proceed directly to configured fallback. |
| Retry budget becomes confusing | Repair and provider fallback use separate evidence counters and do not silently consume ordinary operator retry semantics. |
| Release retry safety is weakened | Release agents are excluded; the implemented durable side-effect ledger owns side-effect reconciliation. |
| P076 keeps retrying despite repair failure | P076 must consume repair/fallback outcome evidence and escalate recurring signatures instead of blind retry. |

## 17. Open Questions

- Should provider fallback count against provider quota retry budgets, or should it have a separate `output_contract_fallback_budget`?
- Should transcript recovery require explicit per-role opt-in, or is current-invocation attribution plus contract validation sufficient?
- Should P079 add MCP `agent_outputs.repair` in the first slice, or wait until automatic repair evidence shows a need?
- Should `docs_guardian` be included in initial provider fallback once documentation no-op contracts are stable?
