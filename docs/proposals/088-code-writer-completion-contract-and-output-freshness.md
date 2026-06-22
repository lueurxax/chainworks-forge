# Proposal 088: Code-Writer Completion Contract, Output Freshness, and Repair Diagnostics

| Field | Value |
|---|---|
| Date | 2026-05-11 |
| Status | Draft |
| Author | Codex |
| Depends on | P037 ACP supervision and idle-hang watchdog, implemented output settlement and strict contract authority in [output contracts, failure evidence, and recovery](../reference/output-contracts-failure-evidence-and-recovery.md), implemented [P079 output repair/fallback contract](../reference/output-contracts-failure-evidence-and-recovery.md#p079-output-contract-repair-and-fallback-details), [execution-truth ownership invariants](../reference/execution-truth-and-recovery.md#durable-execution-truth-ownership) |
| Related | P095 two-phase agent invocation, macOS operator navigation in `docs/reference/macos-operator-navigation.md`, agent work continuation in `docs/reference/agent-work-continuation.md`, storage tiering/read-path liveness in `docs/reference/query-projections-and-client-consumption-contract.md` and `docs/reference/rust-control-plane.md`, `docs/reference/output-contracts-failure-evidence-and-recovery.md` |
| Scope | Contain and diagnose the `code_writer` completion-handoff class where substantive implementation work exists but fresh structured outputs for the current attempt do not settle. |
| Non-goal | No weakening of output contracts, no acceptance of stale files as fresh truth, no provider-specific hotfixes as the main contract, no global missing-output fix for all agents, and no retroactive silent repair of historical blocked runs. |

---

## 1. Problem

Several implementation runs now expose the same expensive boundary:

1. `code_writer` performs real work in the worktree;
2. repository diffs exist and `changed_files_manifest` is persisted;
3. the required structured outputs do not settle for the current attempt:
   - `implementation_progress`
   - `implementation_self_assessment_v2`
   - `tests_result`
4. the attempt either enters failed completion/repair logic or remains stale `implementation_active`;
5. repair either finds only the control-plane-generated manifest or encounters stale files from an earlier attempt;
6. the run becomes blocked or stranded even though useful implementation work already happened.

This is not a rollout problem, not a proposal-freeze problem, and not primarily an MCP/Xcode problem.
It is a completion/handoff problem at the boundary between:

- provider/runtime work,
- exact-path output discovery,
- strict contract validation,
- and durable operator diagnostics.

The current failure is costly because the system repeatedly pays for real implementation work, then loses or strands the attempt at the final structured handoff boundary.
P088 is a completion-handoff containment and diagnostics proposal; it is not a full root-cause fix for every ingestion bug that can prevent outputs from reaching settlement.

## 2. Observed Evidence Baseline

Recent blocked attempts show a repeatable signature:

- `changed_files_manifest` is present and valid;
- `implementation_progress`, `implementation_self_assessment_v2`, and `tests_result` fail as `required output was not produced`;
- `output_contract_repair_started` is emitted;
- repair often reports:
  - `repair_found_count = 1`
  - `repair_missing_count = 3`
  - `repair_stale_count = 3` for older-attempt leftovers;
- `transcript_artifact_id` is unset;
- P087 `agent_execution_runtime_receipts` has no prompt-level completion record for the failing repair prompt;
- the operator can see that code changed, but cannot see a reliable current-attempt completion receipt.

Three concrete examples reached related parts of this boundary:

- `P087` is the direct fixture family for this proposal, not a live-state claim: its evidence must be split between terminal-completed missing-output attempts and a later dirty-worktree provider-timeout attempt;
- The macOS operator navigation evidence seed and the agent work continuation evidence seed are related handoff/readback gaps, but their current local directories are no longer clean missing-output examples because structured outputs now exist and the blockers are different.

Evidence seeds for implementation fixtures:

| Proposal | Run ID | Evidence to fixture before implementation |
|---|---|---|
| macOS operator navigation (`p036` proof alias) | `8456e930-8a67-42f1-a5fe-83e34597857b` | Related seed only. Current local `implementation/self-assessment.json` and `tests.json` exist, so implementation must extract or recreate the older failed-stage evidence where useful work and output settlement diverged rather than treating the live directory as direct proof. |
| Agent work continuation (retained P086 proof alias) | `976f3d1b-31d8-43ef-a3d7-c9940c7086ab` | Related seed only. Current local structured outputs exist and the visible blocker includes handoff/evidence tasks and continuation worker gaps, so implementation must materialize a deterministic fixture from the relevant run-report evidence or capture a fresh equivalent. |
| P087 terminal-completed missing-output attempts | `b4edcf82-0d3f-4281-83b7-f8fe94721721` | Deterministic fixtures for terminal-completed attempts where `changed_files_manifest` passed while `implementation_progress`, `implementation_self_assessment_v2`, and `tests_result` were missing. Seed paths: `artifact_root/validation_failure_code_writer_70260330-a45f-4dbc-a8d2-6f8f5cd219ea.json` and `artifact_root/evidence/runs/b4edcf82-0d3f-4281-83b7-f8fe94721721/stages/2c7b5ea6-88c6-422b-8195-99eccfa2b9b2/agents/bb0d5f53-e74a-4e5c-a48e-8fcb2bfeda7e/runtime_event/failed-stage-evidence-9ffd066d-573b-43cf-93a1-53204b07c036.json`. |
| P087 dirty-worktree timeout attempt | `b4edcf82-0d3f-4281-83b7-f8fe94721721` / attempt prefix `70c9b120` | Negative fixture for pre-existing P087 dirty work followed by provider timeout/no terminal text/no new implementation-owned changes after prompt. It must prove P088 does not classify inherited dirty work as current-attempt completion. |

Before P088 implementation starts, these seeds must be materialized into deterministic fixtures under `docs/evidence/088-code-writer-completion/`, including both P087 terminal-completed missing-output and P087 dirty-worktree timeout shapes.
If any local runtime artifact is no longer available, implementation must capture a fresh equivalent fixture before coding the settlement logic.

This matters because the failure reproduces across providers:

- `junie`
- `claude`

So the root defect is not isolated to one provider adapter.

## 3. Root Cause Model

The current system has six structural gaps.

### 3.1 Work completion is not the same thing as output publication

Today, `code_writer` can leave behind meaningful code changes without publishing a fresh structured output set for the same attempt.
The system therefore knows that work happened, but cannot safely transition because transition truth depends on the structured outputs, not on raw worktree mutation.
It also cannot assume a dirty worktree proves the current attempt did the work; the work-change proof must be scoped to the original prompt's pre/post fingerprints.

### 3.2 `changed_files_manifest` is load-bearing in the wrong way

The control plane auto-generates `changed_files_manifest` before declared-output settlement.
That is useful for evidence, but it also means every broken attempt still looks partially successful:

- one output is always discoverable,
- while the three transition-relevant outputs may still be absent.

This is diagnostically noisy and can hide the real shape of the failure.

### 3.3 Exact-path discovery can detect stale leftovers but cannot prove fresh completion

Older attempts may leave behind valid-looking files at canonical output paths.
The executor already rejects these as stale when their digest matches the pre-prompt baseline, which is correct.
But that leaves a gap:

- the system can say "this file is old",
- yet it has no first-class current-attempt completion proof beyond provider-discovered payloads.

### 3.4 Diagnostics are too weak at the failure boundary

When the attempt fails after doing real work, the durable evidence is incomplete:

- transcript attribution is often absent;
- runtime invocation rows do not provide a useful postmortem;
- repair diagnostics do not distinguish "agent never emitted outputs" from "agent wrote stale leftovers only" from "fresh payload existed but was rejected".

That forces repeated manual investigation instead of deterministic classification.

### 3.5 Active handoff gaps can bypass completion diagnostics

The P087-like failure can remain `implementation_active` after useful work instead of entering failed completion logic.
That means a run may be stranded before P088's receipt, repair, and typed diagnosis machinery is invoked.
P088 must therefore bridge P037 supervision into this contract: an idle or terminalized `code_writer` attempt with real implementation changes and missing required outputs must be finalized into the P088 completion diagnosis path instead of remaining indefinitely active.

### 3.6 Ingestion failure is not the same thing as missing work

If ACP final text contains usable `CHAINWORKS_OUTPUT` for the required outputs, the normal successful path is:

```text
ACP final text collection -> CHAINWORKS_OUTPUT extraction -> declared-output settlement -> validation/import
```

Completion repair must not become the normal materialization path for outputs that were already present in a usable current-attempt final payload.
In that case P088 should record that repair was not used, and any remaining bug belongs to ACP final-text collection, output-envelope extraction, or declared-output settlement.

## 4. Goals

- Make `code_writer` completion deterministic and current-attempt scoped.
- Distinguish "real work happened but completion outputs were not published" from generic provider failure.
- Preserve strict output contracts and fail closed on stale or ambiguous files.
- Ensure repair logic prefers current-attempt truth and never reinterprets prior-attempt files as fresh output.
- Persist enough receipt/transcript/runtime evidence to debug these failures without reopening the whole provider session story.
- Keep the solution provider-independent across `junie`, `claude`, and future `code_writer` backends.

## 5. Non-Goals

- Do not relax `implementation_progress`, `implementation_self_assessment_v2`, or `tests_result` validation.
- Do not allow `changed_files_manifest` to substitute for the missing structured outputs.
- Do not accept unchanged files from a previous attempt as fresh completion proof.
- Do not solve general proposal-review or release-closeout blockers in this proposal.
- Do not solve every `required output was not produced` failure for all agents; proposal writers, reviewers, and other non-`code_writer` agents need separate contract work.
- Do not make historical blocked runs auto-heal silently; repair of old runs remains an explicit operator action.
- Do not introduce provider-specific prompt lore as the canonical contract.

## 6. Alternatives Considered

### Option A: Provider-specific fixes only

Examples:

- tweak Junie prompts,
- tune Claude retry behavior,
- special-case one adapter’s completion path.

Rejected because the same failure appears across multiple providers. This would treat symptoms, not the shared contract defect.

### Option B: Loosen settlement and trust on-disk files more aggressively

Examples:

- accept pre-existing files if they parse,
- treat `changed_files_manifest` plus worktree diff as enough,
- downgrade the missing structured outputs to warnings.

Rejected because it would destroy current-attempt authority and let prior-attempt leftovers become transition truth.

### Option C: Add a dedicated `code_writer` completion contract, current-attempt freshness proof, and stronger diagnostics

Recommended.

This keeps strict validation intact while fixing the missing settlement-evidence layer between "work happened" and "this attempt published valid completion outputs".

## 7. Decision

Introduce a new `code_writer` completion contract with three parts:

1. an engine-generated current-attempt settlement receipt;
2. a `code_writer` completion branch inside the existing one-turn output repair lifecycle;
3. durable receipt/transcript/runtime diagnostics that explain why completion failed.

The runtime must stop treating this class of failure as a generic missing-output event.
It is a specific lifecycle condition:

> implementation work progressed, but current-attempt completion outputs were not durably published.

## 8. Proposed Design

### 8.1 Engine-owned current-attempt completion receipt

Add a new canonical artifact family:

```text
code_writer_completion_receipt_v1
```

Purpose:

- record the engine's output-settlement decision for the current `agent_execution_id`;
- record which required outputs were freshly imported for this attempt;
- record pre-prompt and post-prompt digests, provider-envelope provenance, canonical paths, and validation decisions;
- distinguish fresh publication from stale pre-existing files;
- provide durable operator evidence without becoming a second transition authority.

Authority model:

- Transition truth remains the existing declared-output import, validation, and materialization path.
- Freshness authority comes from engine-owned pre-prompt digest capture, provider-envelope settlement, post-prompt digest comparison, and schema validation.
- The completion receipt is generated by the engine after those checks. It records the decision; it does not authorize files, override validation, or make exact-path outputs fresh by assertion.
- No agent-authored receipt, prompt text, or exact-path file may grant transition authority.
- A missing, malformed, stale, or partial receipt cannot make a run pass; it only improves failure evidence.

Minimum fields:

- `run_id`
- `stage_execution_id`
- `agent_execution_id`
- `session_generation_id`
- `provider`
- `model`
- `completion_mode`
  - `provider_envelope`
  - `acp_final_text_chainworks_output`
  - `exact_path_current_attempt`
  - `code_writer_completion_repair_turn`
  - `mixed`
- `activation_source`
  - `declared_output_settlement_failed`
  - `p037_idle_terminalization`
  - `operator_retry_completion_recovery`
- `ingestion_boundary_failure`
  - `acp_final_text_not_collected`
  - `chainworks_output_not_extracted`
  - `declared_output_settlement_rejected_usable_payload`
  - `terminal_response_capture_truncated_before_output`
  - `extraction_input_truncated`
  - `none`
- `published_outputs`
  - `output_name`
  - `contract_id`
  - `canonical_path`
  - `pre_prompt_sha256`
  - `post_prompt_sha256`
  - `content_sha256`
  - `settlement_source`
  - `validation_status`
  - `rejection_reason`
- `missing_outputs`
- `stale_outputs`
- `published_at`
- `completion_repair_turn_count`
- `generic_repair_turn_count`
- `completion_status`
  - `complete`
  - `partial`
  - `missing_required_outputs`

The receipt is transition evidence, not transition truth.
The executor must derive it from the same settlement decisions that already decide whether required outputs are valid.

### 8.2 Single repair lifecycle with a `code_writer` completion branch

P088 can be entered from either normal output settlement failure or P037 idle/terminalization supervision.
The activation sources are:

- `declared_output_settlement_failed`: the original attempt reached output settlement and required structured outputs were missing or invalid;
- `p037_idle_terminalization`: the attempt is no longer making provider progress, has a real implementation diff, and is still `implementation_active` with required structured outputs missing;
- `operator_retry_completion_recovery`: an explicit retry of a historical run that already has deterministic evidence for the same condition.

When a `code_writer` attempt has:

- a real implementation change; and
- missing required structured outputs,

the runtime must use the existing one same-session repair lifecycle with a `code_writer`-specific completion prompt.

This branch replaces the generic repair prompt for eligible `code_writer` attempts.
It does not run before a second generic repair turn, and it does not get an additional prompt budget.
The maximum total same-attempt repair/finalization budget remains:

```text
max_output_repair_turns_per_attempt = 1
```

The completion branch is a qualitatively different contract from generic output repair:

- contract id: `code_writer_completion_repair_v1`;
- it must use its own system header, allowed-output list, settlement reason, receipt schema, and session events;
- it must not call or reuse the generic `output_contract_repair` prompt template as its primary prompt;
- it may share the live session, declared-output importer, schema validation, budget counter, and materialization pipeline;
- if generic output repair already ran and failed for the same `agent_execution_id` and generation, P088 must not run another generic repair turn;
- when generic repair already failed, eligible `code_writer` attempts may run only `code_writer_completion_repair_v1`; ineligible attempts must block with `generic_repair_already_failed_completion_contract_required`.

State machine:

1. Capture pre-prompt digests for declared exact-path outputs.
2. Capture a pre-prompt worktree fingerprint over implementation-owned paths, including dirty paths that already existed before this prompt.
3. Run the original `code_writer` attempt.
4. Capture a post-prompt worktree fingerprint over the same implementation-owned path set and classify original-prompt work changes.
5. Collect ACP final text and settle any usable current-attempt `CHAINWORKS_OUTPUT` payload through the declared-output import path.
6. Classify exact-path outputs with post-prompt digest comparison.
7. If the attempt remains stale `implementation_active`, P037 must terminalize it into this same settlement path before the completion branch is considered.
8. If required outputs are missing and the attempt is eligible, capture a pre-repair worktree fingerprint.
9. Run one `code_writer_completion_repair_v1` prompt in the same live session.
10. Settle that repair turn through the same provider-envelope/import path.
11. Capture a post-repair worktree fingerprint and compare it with the pre-repair fingerprint.
12. If any non-output repo file changed during the completion repair turn, reject the repair output, write failed-stage evidence, and block with `unexpected_worktree_mutation_during_completion_repair`.
13. Write `code_writer_completion_receipt_v1` and failed-stage evidence.
14. If outputs are still missing or invalid, invalidate/close the active repair generation per existing P079 rules and block with the new diagnostic family.

The completion repair prompt is a narrower finalization step:

- same session when safe;
- no broad re-implementation request;
- no permission-expanding work;
- must emit only the missing structured outputs;
- may summarize already-performed work, tests, and remaining tasks;
- may not change transition authority by inventing outputs unrelated to actual work.

Mutation guard:

- The repair turn must be treated as output-publication only, even though the original `code_writer` profile is write-capable.
- The executor must constrain accepted filesystem changes to declared output paths and engine-owned receipt/evidence paths.
- The executor must capture pre/post worktree state around the repair turn using the same repository diff machinery used for implementation evidence.
- Any change outside declared output targets, receipt artifacts, and failed-stage evidence is a fail-closed settlement error:

```text
unexpected_worktree_mutation_during_completion_repair
```

- This error must preserve raw/redacted completion text or a typed completion-text absence reason for diagnostics but must not merge the repair result into transition truth.
- The preserved diagnostic text must follow the completion-text capture rules in Section 8.6, independent of general transcript capture.

Eligibility requires a current-attempt implementation change:

- pre/post original-prompt worktree fingerprints show new or modified implementation-owned paths after this `agent_execution_id` started; or
- `activation_source=operator_retry_completion_recovery` and the retry references a preserved historical evidence packet that proves the same current-attempt work-change boundary for the original attempt.

An empty manifest, a manifest containing only generated/meta paths, a control-plane-generated manifest with no implementation files, or inherited dirty work that existed before the original prompt is not eligible and must remain `missing_required_outputs` or a more specific timeout/provider diagnosis.
`changed_files_manifest` is supporting evidence only; it cannot prove current-attempt work by itself.

Work-change classification:

- `current_attempt_diff`: implementation-owned path changes appeared between the original prompt pre/post worktree fingerprints;
- `preexisting_dirty_work`: implementation-owned dirty paths were already present before the original prompt and no new implementation-owned changes appeared after it;
- `control_plane_only_manifest`: only `.chainworks/**`, run projections, generated manifests, or control-plane artifacts changed;
- `generated_artifact_only_manifest`: only generated evidence artifacts changed outside deterministic proposal-owned fixtures;
- `none`: no implementation-owned work changed and no eligible historical evidence packet was referenced.

Completion repair eligibility requires `work_change_kind=current_attempt_diff`, except for `activation_source=operator_retry_completion_recovery` with a preserved historical evidence packet.
`preexisting_dirty_work` must not be classified as `work_completed_missing_current_attempt_outputs`; a provider timeout/no terminal text attempt with only inherited dirty work must fail or remain blocked under the provider/terminalization diagnosis, not the completion-handoff diagnosis.

`worktree_fingerprint_v1` artifact schema:

- `schema_version = "worktree_fingerprint_v1"`;
- `run_id`, `stage_execution_id`, `agent_execution_id`, `session_generation_id`;
- `captured_at`, `capture_phase = pre_original_prompt | post_original_prompt | pre_completion_repair | post_completion_repair`;
- `classifier_version`;
- `paths`, sorted bytewise by normalized repository-relative path:
  - `path`
  - `normalized_path`
  - `included = true | false`
  - `include_or_exclude_reason`
  - `path_status = clean | preexisting_dirty | new_after_prompt | modified_after_prompt | deleted_after_prompt | renamed_after_prompt | generated_meta | control_plane_only`
  - `old_path` for renames
  - `content_sha256` when the file exists and can be read
  - `mode`
  - `size_bytes`
  - `source = git_diff | manifest | filesystem_snapshot`
- `summary`:
  - `included_path_count`
  - `excluded_path_count`
  - `current_attempt_changed_path_count`
  - `preexisting_dirty_path_count`
  - `control_plane_only_path_count`
  - `generated_artifact_only_path_count`
  - `deleted_path_count`
  - `renamed_path_count`
  - `work_change_kind`

Counts must be derived deterministically from `paths`, not written as independent authority.
The fingerprint artifact hash is computed over canonical JSON with sorted object keys and the bytewise path order above.
The 70c9-shaped negative fixture must show inherited dirty paths as `preexisting_dirty`, no `new_after_prompt`/`modified_after_prompt` implementation-owned paths, and `work_change_kind=preexisting_dirty_work`.

Implementation-owned path classifier:

- Included implementation paths:
  - `Chainworks Forge/**`
  - `Chainworks ForgeTests/**`
  - `Chainworks ForgeUITests/**`
  - `control-plane/**`
  - `examples/workflows/**`
  - `examples/agents/**`
  - `scripts/**`
  - `docs/reference/**`
  - `docs/proposals/<proposal-id>-artifacts/**`
  - `docs/evidence/<proposal-id>/**`
  - `docs/evidence/rollout-contract/**` when the active proposal owns rollout-contract fixtures
- Excluded generated/meta paths:
  - `.chainworks/**`
  - `.review-baselines/**`
  - `.codex/**`
  - `docs/proposals/088-code-writer-completion-contract-and-output-freshness.md`
  - run-local projections such as `state/run-state.json`, `artifacts/active-index.json`, `review/implementation-summary.json`, and generated `changed_files_manifest` outputs
  - failed-stage evidence, runtime receipts, and validation-failure packets unless they are explicitly copied into deterministic `docs/evidence/**` fixtures
- Docs-only implementation changes are eligible when they update `docs/reference/**`, proposal-owned artifact folders, or proposal-owned evidence fixtures.
- Generated-evidence-only changes outside deterministic `docs/evidence/**` fixtures are not eligible.

### 8.3 Freshness rules for exact-path outputs

Exact-path outputs may be accepted for current-attempt settlement only if one of the following is true:

1. the file was provider-discovered for this attempt;
2. the engine's pre/post digest comparison proves the file changed after the prompt for this `agent_execution_id`;
3. the file was reconstructed by the engine from a current-attempt provider envelope during the `code_writer_completion_repair_v1` path.

The completion receipt may record these outcomes but cannot create them.

Files that existed before the prompt and did not change remain:

```text
stale_previous_attempt_output
```

This classification must be first-class in readback and evidence, not hidden inside a generic missing-output failure.

### 8.4 `changed_files_manifest` becomes advisory for completion diagnosis

`changed_files_manifest` remains useful and should still be auto-generated, but it must no longer distort missing-output diagnostics.

Required behavior:

- it does not count as evidence that completion succeeded;
- it does not reduce the severity of missing transition-relevant outputs;
- repair diagnostics must display it separately as control-plane-generated evidence;
- `repair_found_count` for operator diagnostics must distinguish:
  - `fresh_required_outputs_found`
  - `control_plane_outputs_found`
  - `stale_required_outputs_found`
- eligibility diagnostics must distinguish:
  - `real_implementation_diff`
  - `non_empty_manifest_with_implementation_files`
  - `empty_manifest`
  - `control_plane_only_manifest`
  - `generated_artifact_only_manifest`

### 8.5 Repair ordering and distinct completion contract

For `code_writer` structured outputs, repair ordering must become:

1. ACP final text `CHAINWORKS_OUTPUT` and provider-envelope payloads from the current attempt;
2. exact-path files proven fresh for the current attempt;
3. one `code_writer_completion_repair_v1` turn if eligible;
4. durable failure.

If ACP final text contains usable current-attempt `CHAINWORKS_OUTPUT` and declared-output settlement succeeds, P088 must not run completion repair.
That path is a normal successful materialization path with `completion_repair_turn_count = 0`, `completion_turn_attempted = false`, and no completion failure class.
If usable final payload exists but is not collected, extracted, or settled, the failed receipt must classify the ingestion boundary explicitly as one of:

```text
acp_final_text_not_collected
chainworks_output_not_extracted
declared_output_settlement_rejected_usable_payload
terminal_response_capture_truncated_before_output
extraction_input_truncated
```

The full public vocabulary and unknown handling rules are defined in Section 9.4.
Repair remains a fallback for genuinely missing or invalid outputs, not the primary materialization path for valid final payloads.

For non-`code_writer` agents, the existing generic output repair lifecycle remains unchanged.
For ineligible `code_writer` attempts, the runtime may run the existing generic repair prompt only when the existing P079 rules would already have allowed it; it must not use `changed_files_manifest` as evidence of work completion.
For eligible `code_writer` attempts, `code_writer_completion_repair_v1` replaces generic output repair for this missing-output condition; it is not a retry of the same generic prompt that already failed in prior incidents.

Additional rules:

- stale exact-path files must never count as a repair success candidate;
- the repair result must record output-by-output reasons:
  - `fresh_provider_payload`
  - `fresh_exact_path`
  - `stale_previous_attempt`
  - `control_plane_generated_manifest`
  - `control_plane_only_manifest`
  - `missing_after_completion_turn`
  - `rejected_schema`
- the system must not invalidate the session silently without persisting the completion receipt and completion-text capture status.

### 8.6 Receipt, completion text, and transcript diagnostics

For every failed `code_writer` attempt that reaches repair or completion logic, the runtime must persist:

- one durable P087 runtime receipt for the original prompt;
- one durable P087 runtime receipt for the `code_writer_completion_repair` prompt when it is attempted, even when that repair prompt fails;
- raw/redacted terminal completion text for each prompt attempt that reached terminal settlement, or a typed completion-text absence reason;
- transcript attribution or an explicit transcript absence reason;
- one failed-stage evidence packet whose physical path is resolvable from operator tooling.

P088 does not use the legacy `runtime_invocations` table as the durable evidence owner.
Runtime evidence ownership remains with `agent_execution_runtime_receipts`, extended to preserve multiple prompt attempts under one `agent_execution_id`.

Completion-text capture is prompt-level and independent of the general transcript gate.
A missing transcript must not make either the original terminal response or the completion repair prompt uninvestigable when the provider returned completion text.
For every prompt attempt that reaches terminal settlement, including the original `code_writer` prompt and `code_writer_completion_repair_v1` when attempted, the runtime must store:

```text
completion_text_status = captured | redacted_only
completion_text_redacted_artifact_path = <path>
completion_text_raw_artifact_path = <operator-only path, optional>
completion_text_capture_source = terminal_final_response | streamed_update_tail | session_update_stream
completion_text_raw_byte_limit = <bytes>
completion_text_captured_byte_count = <bytes>
completion_text_truncated = true | false
extraction_input_truncated = true | false
extraction_input_sha256 = <sha256>
```

If no completion text can be stored, the runtime must store:

```text
completion_text_status = unavailable
completion_text_absence_reason = <typed reason>
```

Typed completion-text absence reasons include:

- `provider_did_not_emit_text`
- `terminal_response_without_text`
- `terminal_response_capture_truncated_before_output`
- `extraction_input_truncated`
- `raw_capture_disabled`
- `redaction_failed`
- `storage_write_failed`
- `redacted_storage_write_failed`

Capture and extraction semantics:

- Declared-output extraction must prefer terminal final-response text when the provider exposes it as a distinct response.
- If only streamed session/update text is available, the runtime must preserve a bounded tail buffer in addition to any capped full stream so final `CHAINWORKS_OUTPUT` is not dropped after a long progress prelude.
- The extraction input must have its own source, byte count, truncation flag, and SHA-256 hash; it must not be inferred from transcript availability.
- If extraction runs on incomplete text because a byte cap was hit before the final output marker, failure must be typed as `extraction_input_truncated` or `terminal_response_capture_truncated_before_output`, not generic `missing_required_outputs`.
- A fixture where valid `CHAINWORKS_OUTPUT` appears after a large streamed prelude must either settle through terminal final-response or tail capture, or fail with the truncation-specific reason.

If transcript persistence is unavailable, the runtime must record:

```text
transcript_status = unavailable
transcript_absence_reason = <typed reason>
```

Typed reasons include:

- `provider_did_not_supply`
- `capture_disabled`
- `capture_failed`
- `storage_write_failed`
- `session_reuse_without_terminal_capture`

### 8.7 Operator readback and blocked-reason truth

Operator surfaces must distinguish:

- provider/runtime crash before useful work;
- provider terminal response completed but required structured outputs are missing;
- implementation work completed but current-attempt outputs missing;
- stale previous-attempt outputs present;
- completion turn attempted and failed;
- completion turn skipped as ineligible.

Add blocked/failure classes for this family:

```text
terminal_response_completed_missing_required_outputs
work_completed_missing_current_attempt_outputs
```

`terminal_response_completed_missing_required_outputs` is required when the provider terminal response is `completed` but declared required outputs are absent or invalid.
It must not be reported as `provider_active_without_terminal_response`.
`provider_active_without_terminal_response` is reserved for cases with no durable terminal response or an actually active/timed-out provider handoff.

`work_completed_missing_current_attempt_outputs` remains the operator-facing diagnosis for a failed attempt that made real code changes but did not publish fresh structured outputs.
Neither class is a transition result by itself.

### 8.8 Canary problem closure

P088 must close the P087-like active handoff gap, not only the already-failed repair path.

Required canary shape:

- `code_writer` attempt is still projected as `implementation_active`;
- stage is `state_7_implementation_started`;
- original-prompt pre/post worktree fingerprints prove real current-attempt implementation changes under implementation-owned paths;
- `implementation/progress.md`, `implementation/self-assessment.json`, and `implementation/tests.json` are absent or fail current-attempt settlement;
- transcript linkage and runtime receipt evidence may be missing or unreliable;
- provider is idle, terminalized, or past the P037 supervision threshold.

Required outcome:

- the run must not remain indefinitely `implementation_active`;
- P037 terminalization must route the attempt into P088 settlement;
- if current-attempt final text contains usable `CHAINWORKS_OUTPUT`, normal declared-output settlement must materialize those outputs without completion repair;
- if required outputs are still missing, the run must receive `work_completed_missing_current_attempt_outputs` or `terminal_response_completed_missing_required_outputs`;
- `code_writer_completion_receipt_v1`, prompt-level runtime receipts, prompt-level completion text captures or typed absence reasons, and operator readback must identify whether the break happened in initial completion, final-text ingestion, or completion repair;
- completion repair may succeed only through `code_writer_completion_repair_v1`; otherwise the attempt fails closed with an actionable typed reason.

## 9. Data Model and Contract Changes

### 9.1 New artifact contract and SQLite ownership

Add:

- `code_writer_completion_receipt_v1`

The durable owner is SQLite plus a canonical artifact projection.
Implementation must add migration:

```text
051_p088_code_writer_completion_receipts.sql
```

Required tables:

- extend `agent_execution_runtime_receipts`
  - replace the single-row key with prompt-level identity: `(agent_execution_id, prompt_kind, turn_index)`
  - `prompt_kind`
    - `original`
    - `output_contract_repair`
    - `code_writer_completion_repair`
  - `turn_index`
  - `runtime_receipt_id`
  - `prompt_template_id`
  - `prompt_template_version`
  - `prompt_sha256`
  - `redacted_prompt_artifact_path`
  - `expected_output_contract_snapshot_sha256`
  - `expected_output_contract_snapshot_path`
  - `repair_or_settlement_reason`
  - unique key: `(agent_execution_id, prompt_kind, turn_index)`
  - retain backward-compatible readback by treating legacy single-row receipts as `prompt_kind=original`, `turn_index=0`
- `code_writer_completion_receipts`
  - `id`
  - `run_id`
  - `stage_execution_id`
  - `agent_execution_id`
  - `session_generation_id`
  - `original_runtime_receipt_id`
  - `completion_repair_runtime_receipt_id`
  - `provider`
  - `model`
  - `activation_source`
  - `ingestion_boundary_failure`
  - `work_change_kind`
  - `pre_prompt_worktree_fingerprint_path`
  - `post_prompt_worktree_fingerprint_path`
  - `pre_prompt_worktree_fingerprint_sha256`
  - `post_prompt_worktree_fingerprint_sha256`
  - `current_attempt_changed_path_count`
  - `preexisting_dirty_path_count`
  - `completion_status`
  - `failure_class`
  - `terminal_response_status`
  - `completion_turn_attempted`
  - `completion_turn_result`
  - `completion_text_capture_count`
  - `completion_text_absence_count`
  - `completion_repair_text_status`
  - `completion_repair_raw_text_artifact_path`
  - `completion_repair_redacted_text_artifact_path`
  - `completion_repair_text_absence_reason`
  - `fresh_required_output_count`
  - `stale_required_output_count`
  - `missing_required_output_count`
  - `control_plane_output_count`
  - `transcript_status`
  - `transcript_absence_reason`
  - `receipt_artifact_path`
  - `failed_stage_evidence_path`
  - `created_at`
  - unique key: `agent_execution_id`
- `code_writer_completion_text_captures`
  - `receipt_id`
  - `prompt_kind`
  - `turn_index`
  - `terminal_response_status`
  - `completion_text_status`
  - `completion_text_capture_source`
  - `completion_text_raw_byte_limit`
  - `completion_text_captured_byte_count`
  - `completion_text_truncated`
  - `extraction_input_truncated`
  - `extraction_input_sha256`
  - `raw_text_artifact_path`
  - `redacted_text_artifact_path`
  - `text_absence_reason`
  - `created_at`
  - unique key: `(receipt_id, prompt_kind, turn_index)`
- `code_writer_completion_output_decisions`
  - `receipt_id`
  - `output_name`
  - `contract_id`
  - `canonical_path`
  - `pre_prompt_sha256`
  - `post_prompt_sha256`
  - `content_sha256`
  - `settlement_source`
  - `validation_status`
  - `rejection_reason`
  - unique key: `(receipt_id, output_name)`

Transaction and replay rules:

- Insert or update the completion receipt, prompt-level completion text captures, output decisions, `agent_execution_runtime_receipts` prompt-level links, `agent_executions` linkage, and artifact-contract projection in one DB transaction.
- `id` must be deterministic from `agent_execution_id` and receipt schema version, or the table must enforce one canonical row per `agent_execution_id` with idempotent upsert.
- Replaying the same settlement with byte-identical receipt and output decisions is idempotent success.
- Replaying the same `agent_execution_id` with different output decisions returns `completion_receipt_conflict` and performs no partial write.
- Output decisions cascade on receipt deletion only in tests; production deletion is not a normal lifecycle operation.
- Readback selects the receipt linked from the current active `agent_execution_id`; if multiple historical receipts exist after migration or repair, only the active artifact-contract generation is canonical and older rows remain audit history.
- A crash between artifact write and DB transaction must fail closed to `completion_receipt_partial_write` until startup recovery reconciles or marks the receipt unusable.
- A completion repair prompt must never overwrite the original prompt runtime receipt, and the original prompt must never overwrite the completion repair runtime receipt.

`agent_executions` must link to the completion receipt by `completion_receipt_id` or equivalent FK-backed reference.
The artifact projection under the run directory is readback evidence; SQLite remains the compact query owner.

### 9.2 Runtime facts enrichment

Extend runtime facts and/or failed-stage evidence with:

- `completion_turn_attempted`
- `completion_turn_result`
- `activation_source`
- `ingestion_boundary_failure`
- `pre_prompt_worktree_fingerprint_path`
- `post_prompt_worktree_fingerprint_path`
- `terminal_response_status`
- `fresh_required_output_count`
- `stale_required_output_count`
- `missing_required_output_count`
- `control_plane_output_count`
- `completion_text_captures`
- `prompt_template_id`
- `prompt_template_version`
- `prompt_sha256`
- `redacted_prompt_artifact_path`
- `expected_output_contract_snapshot_sha256`
- `repair_or_settlement_reason`
- `completion_repair_text_status`
- `completion_repair_redacted_text_artifact_path`
- `completion_repair_text_absence_reason`
- `transcript_status`
- `transcript_absence_reason`
- `completion_receipt_id`
- `completion_receipt_artifact_path`
- `original_runtime_receipt_id`
- `completion_repair_runtime_receipt_id`
- `completion_receipt_conflict`
- `completion_receipt_partial_write`
- `work_change_kind`
- `output_decisions`

### 9.3 Session event additions

Add typed session events:

- `code_writer_completion_started`
- `code_writer_completion_succeeded`
- `code_writer_completion_failed`

These are a specialization of the same repair lifecycle, not a separate lifecycle.
They must include the same generation id, repair turn count, and budget accounting used by `output_contract_repair_*`.

### 9.4 Operator readback shape

Public readback vocabularies are closed for known values and forward-compatible for unknown values.
GraphQL must expose enum-like values with an `UNKNOWN` fallback or string-preserving wrapper; MCP and run-report JSON must preserve the raw value and also provide `known=false` for unrecognized future values.

`implementationCompletion.status` values:

- `not_applicable`
- `not_attempted`
- `succeeded`
- `failed`
- `blocked`
- `skipped_no_live_session`
- `partial_evidence`
- `unknown`

`implementationCompletion.ingestion_boundary_failure` values:

- `none`
- `acp_final_text_not_collected`
- `chainworks_output_not_extracted`
- `declared_output_settlement_rejected_usable_payload`
- `terminal_response_capture_truncated_before_output`
- `extraction_input_truncated`
- `unknown`

`implementationCompletion.completion_turn_result` values:

- `not_attempted`
- `succeeded`
- `failed_missing_outputs`
- `failed_schema_validation`
- `failed_unexpected_worktree_mutation`
- `skipped_ineligible`
- `skipped_no_live_session`
- `skipped_usable_final_output_settled`
- `unknown`

`implementationCompletion.next_operator_action` values:

- `none`
- `inspect_outputs_then_retry`
- `inspect_truncated_completion_text`
- `inspect_prompt_and_expected_output_contract`
- `materialize_fixtures_before_implementation`
- `retry_with_completion_recovery`
- `fix_acp_final_text_collection`
- `fix_chainworks_output_extraction`
- `fix_declared_output_settlement`
- `do_not_retry_preexisting_dirty_timeout`
- `unknown`

Expose an additive `implementationCompletion` summary in run report, MCP `runs.get`/`runs.list`, and GraphQL run summary/readback:

- `status`
- `failure_class`
- `work_change_kind`
- `activation_source`
- `ingestion_boundary_failure`
- `pre_prompt_worktree_fingerprint_path`
- `post_prompt_worktree_fingerprint_path`
- `completion_turn_attempted`
- `completion_turn_result`
- `terminal_response_status`
- `completion_text_captures`
- `prompt_template_id`
- `prompt_template_version`
- `prompt_sha256`
- `redacted_prompt_artifact_path`
- `expected_output_contract_snapshot_sha256`
- `repair_or_settlement_reason`
- `fresh_required_output_count`
- `stale_required_output_count`
- `missing_required_output_count`
- `control_plane_output_count`
- `completion_repair_text_status`
- `completion_repair_redacted_text_artifact_path`
- `completion_repair_text_absence_reason`
- `transcript_status`
- `transcript_absence_reason`
- `receipt_artifact_path`
- `failed_stage_evidence_path`
- `next_operator_action`

GraphQL exposure is mandatory because P088 includes operator readback/UI scope and the existing implementation-summary truth is already cross-surface.
GraphQL must not add a retry, repair, continue, or completion mutation for P088.

## 10. Compatibility and Rollout

This proposal is prevention-first.

Behavior for historical runs:

- old runs remain readable;
- old failures are not auto-reclassified retroactively unless the run is explicitly retried;
- explicit operator retry may use the new completion contract on the next attempt.

Rollout order:

1. create fixtures from the macOS operator navigation (`p036`), agent work continuation (retained `P086` proof alias), and P087 evidence seeds;
2. add SQLite receipt tables, artifact contract, transcript/runtime diagnostics, and readback fields;
3. add current-attempt freshness classification using engine pre/post digests and provider-envelope settlement;
4. wire P037 idle/terminalization into P088 for P087-like stale `implementation_active` attempts;
5. prove usable final `CHAINWORKS_OUTPUT` materializes through the normal declared-output path without completion repair;
6. replace the generic repair prompt with the distinct `code_writer_completion_repair_v1` contract for eligible `code_writer` attempts while preserving the one-turn total budget;
7. update operator readback/UI to display the new failure family;
8. then use targeted retries on currently blocked runs.

## 11. Tests

### 11.1 Unit tests

- stale exact-path outputs from a previous attempt are classified as `stale_previous_attempt_output`;
- fresh provider-envelope payloads produce a valid completion receipt;
- usable current-attempt ACP final text `CHAINWORKS_OUTPUT` materializes through normal settlement and does not invoke completion repair;
- usable current-attempt ACP final text that fails collection, extraction, or settlement records the correct `ingestion_boundary_failure`;
- `changed_files_manifest` is counted separately from required outputs;
- original-prompt pre/post worktree fingerprints classify `current_attempt_diff` only when implementation-owned paths changed after prompt start;
- pre-existing dirty implementation work with provider timeout/no terminal text is classified as `preexisting_dirty_work`, not `work_completed_missing_current_attempt_outputs`;
- empty manifests and control-plane-only manifests are not eligible for `code_writer_completion_repair_v1`;
- original-prompt fingerprints with `work_change_kind=current_attempt_diff` are eligible;
- non-empty implementation-file manifests are supporting evidence only and are not eligible without current-attempt fingerprint proof or preserved historical recovery evidence;
- docs-only implementation changes under `docs/reference/**`, proposal-owned artifact folders, or deterministic `docs/evidence/**` fixtures are eligible;
- generated-evidence-only changes outside deterministic `docs/evidence/**` fixtures are not eligible;
- completion repair fails closed with `unexpected_worktree_mutation_during_completion_repair` if the repair turn mutates non-output repo files;
- a completion receipt cannot authorize freshness without engine pre/post digest or provider-envelope proof;
- eligible `code_writer` completion repair uses `code_writer_completion_repair_v1` and does not call the generic `output_contract_repair` prompt template;
- if generic repair already failed for the same execution/generation, an ineligible attempt blocks with `generic_repair_already_failed_completion_contract_required`;
- the `code_writer_completion_repair_v1` branch consumes the same one-turn repair budget as generic repair;
- receipt replay with the same `agent_execution_id` and identical decisions is idempotent;
- receipt replay with the same `agent_execution_id` and changed decisions returns `completion_receipt_conflict`;
- crash between artifact write and DB transaction surfaces `completion_receipt_partial_write`;
- `terminal_response_status=completed` plus missing required outputs is classified as `terminal_response_completed_missing_required_outputs`, not `provider_active_without_terminal_response`;
- prompt-level completion text capture records raw/redacted text or a typed completion-text absence reason independently of transcript status;
- completion text capture records source, byte limit, captured byte count, truncation flags, and extraction input SHA-256;
- valid `CHAINWORKS_OUTPUT` after a large streamed prelude settles through terminal final-response or tail capture, or fails with `completion_text_truncated`/`extraction_input_truncated`;
- prompt-level runtime receipts record prompt template id/version, prompt hash, redacted prompt artifact, expected-output snapshot hash, and settlement/repair reason;
- transcript absence reasons serialize deterministically.

### 11.2 Integration tests

- macOS operator navigation (`p036`)-shaped attempt:
  - diff exists,
  - stale prior structured outputs exist,
  - completion receipt records stale outputs,
  - blocked reason is `work_completed_missing_current_attempt_outputs`.
- `P087`-shaped attempt:
  - diff exists,
  - no prior structured outputs exist,
  - active attempt is terminalized from `implementation_active`,
  - P088 receipt records `activation_source=p037_idle_terminalization`,
  - completion turn is attempted only if normal final-text extraction cannot materialize outputs,
  - failure records `missing_after_completion_turn` or successful recovery records `completion_turn_result=succeeded`.
- Agent work continuation-shaped attempt:
  - provider reused session,
  - dedicated completion turn succeeds and the run advances.

### 11.3 Receipt and evidence tests

- every failed completion attempt persists an original prompt runtime receipt and, when attempted, a separate `code_writer_completion_repair` prompt runtime receipt;
- completion repair failure preserves both runtime receipts without overwrite;
- completion repair success preserves both runtime receipts without overwrite and links both from `code_writer_completion_receipt_v1`;
- original and completion-repair terminal text are preserved as redacted text, optional operator-only raw text, or typed completion-text absence reasons even when transcript capture is unavailable;
- failed evidence can answer what prompt template and expected-output contract were sent, and what text the provider returned or failed to return;
- every failed completion attempt has either transcript linkage or a typed absence reason;
- failed-stage evidence paths returned by readback actually resolve to on-disk artifacts or explicit spool references.

### 11.4 Provider-independence tests

Fixture-backed tests must cover the same completion contract for:

- `junie`
- `claude`
- `codex`

without provider-specific truth branches in the contract logic.

### 11.5 Focused gate

Register and document:

```text
proposal-088|p088
```

The gate must be runnable through `./scripts/test-gate.sh proposal-088` and `./scripts/test-gate.sh p088`.
It must cover:

- stale exact-path output negative fixture;
- empty manifest negative fixture;
- control-plane-only manifest negative fixture;
- missing transcript with typed absence reason fixture;
- prompt-level completion text raw/redacted capture fixture;
- prompt-level completion text typed absence reason fixture independent of transcript absence;
- large streamed prelude with final `CHAINWORKS_OUTPUT` beyond the full-stream cap fixture;
- prompt-side evidence fixture with prompt hash/template/version/redacted prompt artifact/expected-output snapshot;
- public enum round-trip fixtures for `extraction_input_truncated`, `terminal_response_capture_truncated_before_output`, `skipped_no_live_session`, and unknown future values;
- `worktree_fingerprint_v1` schema and deterministic count-derivation fixture;
- completion-turn failure fixture;
- P087-like stale `implementation_active` canary fixture;
- P087 70c9-shaped preexisting dirty work plus provider timeout negative fixture;
- usable final `CHAINWORKS_OUTPUT` normal-materialization fixture with no completion repair;
- ingestion-boundary classification fixtures for final-text collection, extraction, and settlement failures;
- terminal response completed plus missing outputs diagnostic fixture;
- provider-envelope success fixture;
- unexpected worktree mutation during completion repair negative fixture;
- docs-only implementation change eligibility fixture;
- generated-evidence-only ineligibility fixture;
- receipt idempotent replay and conflict fixtures;
- original-plus-completion-repair runtime receipt preservation fixtures for success and failure;
- SQLite migration and round-trip readback checks;
- MCP/run-report/GraphQL readback shape checks.

## 12. Acceptance Criteria

P088 is complete when:

1. `code_writer` attempts that change the worktree but miss structured outputs are classified as a distinct completion failure family rather than generic missing output;
2. stale previous-attempt files cannot satisfy current-attempt output settlement;
3. completion repair eligibility is based on original-prompt pre/post worktree fingerprints and requires `work_change_kind=current_attempt_diff`, except explicit historical recovery with preserved evidence;
4. pre-existing dirty work plus provider timeout/no terminal text is not classified as current-attempt work completion;
5. every such failure persists a completion receipt plus transcript/runtime evidence or typed absence reasons;
6. the `code_writer_completion_repair_v1` branch can recover eligible attempts without a full fresh retry, without reusing the generic repair prompt contract, and without exceeding the existing one-turn repair budget;
7. completion repair cannot mutate non-output repo files; unexpected mutation fails closed with typed evidence;
8. original-attempt and completion-repair runtime receipts are separately persisted under `agent_execution_runtime_receipts` and cannot overwrite each other;
9. SQLite receipt writes are transactionally linked, idempotent on replay, conflict-detecting on drift, and canonically selected for readback;
10. original and completion-repair terminal text are durably inspectable as redacted text, optional operator-only raw text, or typed absence reasons independent of transcript availability;
11. completion text capture records source, byte limits, truncation flags, extraction input SHA-256, and typed truncation failures;
12. prompt-side evidence records prompt template id/version, prompt hash, redacted prompt artifact, expected-output contract snapshot hash, and repair/settlement reason;
13. `worktree_fingerprint_v1` artifacts explain path-level inclusion/exclusion, path status, content digest, and deterministic count derivation for `current_attempt_diff` vs `preexisting_dirty_work`;
14. public GraphQL/MCP/run-report readback has closed vocabularies with forward-compatible unknown handling for status, ingestion boundary failure, completion turn result, and next operator action;
15. operator readback across run report, MCP, and GraphQL can explain exactly which outputs were fresh, stale, missing, or control-plane-generated;
16. completed terminal responses with missing required outputs are classified as `terminal_response_completed_missing_required_outputs`, never as `provider_active_without_terminal_response`;
17. P087-like stale `implementation_active` attempts with real implementation changes and missing structured outputs are terminalized into P088 diagnosis/recovery instead of remaining indefinitely active;
18. usable current-attempt final `CHAINWORKS_OUTPUT` materializes through normal declared-output settlement and does not use completion repair;
19. `docs/evidence/088-code-writer-completion/` contains both P087 terminal-completed missing-output and 70c9-shaped dirty-worktree timeout fixtures before implementation starts;
20. P088 readback does not claim to close missing-output failures for non-`code_writer` agents;
21. `proposal-088|p088` is registered in `scripts/test-gate.sh` and `docs/reference/test-gates.md`;
22. targeted retries for current blocked runs no longer require forensic digging to distinguish provider failure from completion-handoff failure.

## Relationship to P095: Two-Phase Agent Invocation

P095 makes the P088 distinction explicit in the normal invocation lifecycle:
work completion and output settlement are separate facts.

For `code_writer`:

- changed files, tests, generated artifacts, and tool traces can prove that work
  happened;
- those facts do not prove that required structured outputs settled;
- fresh output must come from the P095 output collection turn or from a valid
  P079/P088 repair path;
- stale artifacts from previous attempts remain invalid even when the worktree
  contains useful changes.

P088 keeps owning freshness diagnostics and completion-handoff failure
classification. P095 defines the normal prompt/readback/output sequence that
should reduce how often P088 repair is needed.

## 13. Open Questions

- Should the dedicated completion turn be limited to `code_writer`, or should a later proposal extend the same contract to other write-capable agents with strict structured outputs?
- Should a future proposal generalize the `work_change_kind` eligibility model to docs-only or proposal-writer agents, or should P088 keep implementation-file changes as the only eligible signal?

## 14. Recommendation

Implement P088 as the next containment and diagnostics fix for the `code_writer` completion-handoff class, with P087 as the direct canary and macOS operator navigation plus agent work continuation as related fixture seeds.

It addresses the real failure boundary:

- not provider startup,
- not rollout preflight,
- not proposal freeze,
- but the missing durable proof that the current implementation attempt actually published its required structured completion outputs.
