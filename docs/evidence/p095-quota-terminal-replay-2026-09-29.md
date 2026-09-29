# P095 quota facts lost during terminal preclaim replay

Scope: local repair on top of `80ca9f6964b6ffbf8bc6372cf99385d3e51c2932`.
This report records implementation and local validation before integration.
It does not claim daemon deployment or live-run recovery.

## Observed failure

Read-only evidence captured at 2026-09-29 06:57:44 UTC identifies:

- run `49e8cb61-9b8c-4682-9546-b56e75602c23`;
- stage execution `c8383391-ef3f-4b7b-ab51-b91a45e26276`;
- failed proposal writer `1753dbb6-8e97-4e67-960b-0710ffc93798`;
- quota ledger `6494ae14-a83d-49d7-a9e1-bdab28b7c00e`;
- work item `p058-invoke:c8383391-ef3f-4b7b-ab51-b91a45e26276:0`.

The original receipt at 06:13:46 records `provider_quota` and a provider reset
at 13:00 Asia/Nicosia (10:00 UTC). The ledger instead held the 30-minute fallback
06:43:46.036439 UTC. At 06:13:52.933795, the daemon explicitly logged a transient
SQLite requeue because `projections.refresh_run_list_readbacks` timed out in
DbWriter. The queue retained the original `p058_claimed` execution.

At 06:43:46 the elapsed fallback allowed the same work item to be claimed again.
Session bookkeeping used a default runtime-facts object in a full upsert, clearing
failure kind, retry deadline, and diagnostics while retaining `quota_ledger_id`.
The exact facts update was 06:43:46.384556. A new session generation was created;
the late result was discarded because the agent was already failed, and strict
work-item completion refused the missing valid-output proof. The run remained
blocked at `state_5_proposal_refined`. The automatic retry query correctly excluded
NULL failure facts, so its existing explicit-provider-reset deferral never ran.

Local sanitized evidence: `/private/tmp/cw-p095-quota-reset-evidence.json`, SHA-256
`43467111e09bda7fe91419f85db868516ffba177689cc0311672f6a5d1153ae3`.
The daemon log proves the requeue trigger; no second provider receipt was retained.

## Change and safety boundaries

- Detect a terminal preclaimed execution before session policy, headless
  preparation, prompt updates, or provider invocation; finish only queue settlement.
- Preserve strict current-attempt, same-owner claim and valid-output completion
  checks. Failed attempts without sufficient output evidence fail the work item
  through its existing settlement path.
- Use the existing narrow session-reuse metadata update at both executor callsites.
- Preserve automatic quota candidate eligibility. NULL facts are not quota proof.

Claim bookkeeping and capacity/quota admission precede the new executor guard;
this change does not claim a mutation-free claim phase. It also does not fix the
underlying projection write timeout or initial fallback-reset selection.

## Verification

Two executor regressions failed before the fix: failed terminal replay entered ACP
again and overwrote quota evidence, and verified failed outputs did not settle.
Both passed after the fix. The final focused quota suite passed 11 tests, including
foreign-claim rejection, normal explicit recovery with NULL historical facts,
provider-reset deferral, idempotent auto retry, active-work exclusion and cancellation.

Independent review found no blocking issue. The changed Rust files pass rustfmt
and `git diff --check`. After committing, standalone
`./scripts/check-boundary-coverage.sh --base 80ca9f6964b6ffbf8bc6372cf99385d3e51c2932`
passed against the actual committed diff. The full `test-gate.sh guardrails`
launcher returned exit 2 because the installed app was running; its idle guard
was not bypassed and the app was not stopped. This is not claimed as a completed
post-commit guardrails gate. Log: `/private/tmp/cw-p095-quota-guardrails-committed.log`.

The full managed-Cargo workspace run uses `--locked --offline --workspace
--no-fail-fast -- --test-threads=1`. Three failures in unchanged baseline crates
were separately reproduced with exact compiled-binary runs (exit 101 each):

- `auth::tests::p083_bootstrap_uses_approval_only_and_full_set_still_loads`:
  the approval-only mutation whitelist rejects a lifecycle mutation before the
  later full-set compatibility check.
- `db::repos::work_items::tests::p091_claim_next_quarantines_malformed_typed_advance_and_claims_next`;
- `db::repos::work_items::tests::p091_claim_next_quarantines_source_work_item_only_retry_advance`:
  both DB tests schedule at the current time and claim immediately, before the
  scheduler's whole-second `now - 1 second` cutoff, then unwrap `None`.

The auth/db/domain sources and Cargo inputs are unchanged from the base revision.
The isolated rerun log is `/private/tmp/cw-p095-quota-baseline-failures.log`.

The broad engine unit target also reports three failures outside the changed
executor replay path (637 tests pass):

- `executor::tests::provider_quota_runtime_receipt_preserves_explicit_claude_reset_time`:
  the unchanged receipt helper uses wall-clock `Utc::now()` instead of the fixture's
  historical timestamp (September 29 versus expected August 30).
- `executor::tests::targeted_p058_retry_refreshes_stale_tier_from_the_durable_ledger`:
  the fixture's `idea-p058` value fails the run reader's UUID parser.
- `orchestrator::tests::persisted_dynamic_health_fallback_payload_uses_frozen_target_profile_authority`:
  frozen-authority validation rejects the fixture's differing `model` field.

These are not presented as green tests or silently repaired within this patch.

## Broad-run limits and verified baseline comparison

The workspace run was intentionally bounded after more than 30 minutes and 69
completed test targets: **2882 passed, 54 failed, 2 ignored**. It exited 130 when
only its verified Cargo/test process group was interrupted during
`proposal_039_approval_runtime`. Later workspace targets were not completed.
The full workspace is not green. No live daemon process was interrupted.

The engine integration target completed with **169 passed / 48 failed**; all 11
quota tests, including the four new regressions, passed in that serial broad run.
Independent callgraph review excluded the changed executor path for 46 failures:
29 call other components directly and 17 return `Ok(false)` before `process_item`.
Two tests discard their first executor result and required direct baseline proof.

All **48** exact failing tests were then rerun serially on unchanged
`80ca9f6964b6ffbf8bc6372cf99385d3e51c2932`: **0 passed / 48 failed / 165 filtered**,
exit 101, 64.20 seconds. Every panic diagnostic matches the patched run after
normalizing generated UUIDs and excluding panic source locations. This includes
both tests with hidden executor results. No patch-related blocking regression was
identified; the separate baseline defects remain unfixed.

The first attempted baseline build incorrectly reused a shared Cargo-cache binary
containing the four new tests. That result was discarded. The accepted comparison
forced engine recompilation from clean `cf23/main` by refreshing only the two
source timestamps, then verified the binary lists **213 original tests** and none
of the four P095 additions before executing the 48-test subset. Source contents
and the baseline commit were unchanged. The patched binary lists 217 tests.

Verification logs and comparison:

- `/private/tmp/cw-p095-quota-focused.log` (11 passing quota tests);
- `/private/tmp/cw-p095-quota-workspace.log` (bounded broad run);
- `/private/tmp/cw-p095-quota-baseline-build-forced.log` (accepted base rebuild);
- `/private/tmp/cw-p095-quota-baseline-integration-verified.log` (48 base failures);
- `/private/tmp/cw-p095-quota-baseline-comparison.json` (48 matching diagnostics);
- `/private/tmp/cw-p095-quota-engine-baseline-reruns.log` (three engine unit failures).

## Current-run recovery assessment

Do not infer provider availability from the elapsed 06:43 fallback. After independently
verifying the 10:00 UTC receipt deadline and fresh normal run/stage safety prerequisites,
an authorized ordinary `stages.retry` for workflow stage `state_5_proposal_refined`,
with `consume_quota_budget_now=false`, can create a new stage execution and retry
authority. Use a fresh request ID and recheck that no newer attempt is active.

The normal command consults the durable ledger directly and does not require
`facts.failure_kind`. Its behavior is proven on an isolated database fixture with
NULL historical facts. It does not reconstruct those facts or restore the old
ledger to automatic eligibility. No historical database rewrite is needed to
continue the run, and no automatic repair/migration is included in this patch.

## Integration failure comparison details

All rows below also fail on the verified baseline with matching panic diagnostics.
Excluding a P095 regression does not claim every older defect is fully diagnosed.

### 29 failures outside the changed executor path

| Exact test | Panic line | Failing call path / observation |
|---|---:|---|
| `auto_contract_retry_authority_gap_recovery_completes_valid_legacy_invoke` | 3341 | RecoveryService only; Pending instead of Completed. Missing strict source-generation claim proof; repair error is logged, generic startup requeue leaves Pending. |
| `blocked_implementation_assessment_routes_back_to_refinement` | 13870 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `governed_catalog_snapshot_retrofit_updates_blocked_run_escalation_policy_only` | 421 | CommandHandler retrofit only; current unchanged catalog has two enabled state-7 policies, fixture expects one. |
| `implementation_review_needs_code_fixes_blocks_manual_release_even_when_self_assessment_has_no_code_tasks` | 15578 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `p091_post_invoke_completion_failure_settle_exact_retry_target_end_to_end` | 4205 | Direct WorkQueue.fail/complete; strict same-owner + active-or-closed source claim + valid-output proof rejected before any executor. |
| `p092_live_helper_honors_configured_batch_limit` | 4611 | P092 recovery helper only; strict same-owner/source-claim completion proof rejected. |
| `p092_live_helper_promotes_existing_diagnostic_row_idempotently` | 4562 | P092 recovery helper only; strict same-owner/source-claim completion proof rejected. |
| `p092_startup_diagnostic_records_stranded_retry_and_skips_generic_requeue` | 4362 | RecoveryService only; Pending instead of Running after startup repair. |
| `p092_startup_enforce_completes_stranded_retry_and_enqueues_targeted_advance` | 4507 | RecoveryService only; Pending instead of Completed after startup repair. |
| `proposal_087_projection_cache_rebuilds_after_restart` | 853 | RecoveryService and projection readback only; 0 artifact rows instead of 2. Precise baseline projection cause not needed for patch exclusion. |
| `startup_recovery_completes_normal_invoke_blocked_by_stale_targeted_authority` | 3463 | RecoveryService only; Pending instead of Completed after startup repair. |
| `steward_trigger_tests_completed_run_consumes_config_change_pending_first` | 11241 | Orchestrator then direct work_items::claim_next; no claimed steward work item. |
| `steward_trigger_tests_manual_command_enqueues_shared_work_item` | 11160 | CommandHandler then direct work_items::claim_next; no claimed steward work item. |
| `steward_trigger_tests_post_run_hook_honors_interval` | 11329 | Orchestrator then direct work_items::claim_next; no claimed second steward work item. |
| `targeted_blocked_review_synthesis_reuses_retry_stage` | 2885 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_n_phase_does_not_advance_after_failed_agent_execution` | 18752 | CommandHandler and direct Orchestrator advancement; phase-1 item present unexpectedly. |
| `test_n_phase_sequence_ordering` | 18670 | CommandHandler and direct Orchestrator advancement; phase-1 item present before expected completion. |
| `test_retry_stage_on_blocked_implementation_review_targets_single_failed_agent_by_default` | 6640 | RetryStage CommandHandler only; stage Pending instead of Running. |
| `test_retry_stage_on_blocked_review_stage_retries_only_failed_reviewer` | 6800 | RetryStage CommandHandler only; stage Pending instead of Running. |
| `test_state_11_to_state_12_happy_path` | 19576 | CommandHandler and direct Orchestrator advancement; state_11 remains Running instead of Completed. |
| `test_targeted_retry_falls_back_from_claude_quota_proposal_aggregation_backend` | 8430 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_falls_back_from_claude_quota_proposal_reviewer_backend` | 7598 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_falls_back_from_claude_security_checker_backend` | 7471 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_falls_back_from_codex_timeout_proposal_reviewer_backend` | 7863 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_falls_back_from_codex_timeout_proposal_writer_backend` | 8305 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_falls_back_from_gemini_docs_guardian_backend` | 7333 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_falls_back_from_gemini_proposal_reviewer_backend` | 7062 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_falls_back_from_junie_code_writer_quota_to_sonnet` | 7731 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |
| `test_targeted_retry_uses_current_catalog_binding_when_agent_profile_changed` | 7195 | CommandHandler/Orchestrator frozen-snapshot validation rejects incomplete JSON/hash quartet before executor dispatch. |

### 17 failures before process_item

| Exact test | Panic line | Observed result |
|---|---:|---|
| `invoke_agent_repairs_missing_required_output_in_same_live_session` | 14333 | `process_next_item -> Ok(false)`; changed path not entered |
| `mcp_resolution_persistence_tests` | 11574 | `process_next_item -> Ok(false)`; changed path not entered |
| `proposal_057_failed_provider_result_settles_valid_outputs_by_degraded_policy` | 13412 | `process_next_item -> Ok(false)`; changed path not entered |
| `proposal_057_invoke_agent_imports_declared_contract_output_into_active_index` | 13200 | `process_next_item -> Ok(false)`; changed path not entered |
| `steward_executor_tests_work_item_runs_active_catalog_agents_through_acp` | 11112 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_advance_run_rebuilds_projection_after_blocking_run` | 8515 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_end_to_end_with_fixture_binary` | 12058 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_imports_implementation_self_assessment_summary` | 13643 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_missing_live_handle_falls_back_to_fresh_generation` | 15920 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_persists_declared_machine_artifact_under_normalized_name` | 13012 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_persists_runtime_cost_and_next_policy_invalidates_on_cost_budget` | 14816 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_persists_undeclared_envelope_output_as_generic_artifact` | 12840 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_rehydrates_from_checkpointed_generation_and_persists_checkpoint_artifact` | 18064 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_reuses_live_session_generation_end_to_end` | 14094 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_invoke_agent_startup_error_marks_agent_execution_failed` | 12178 | `process_next_item -> Ok(false)`; changed path not entered |
| `test_junie_post_preflight_capacity_requeues_without_failing_stage` | 16165 | `process_next_item -> Ok(false)`; changed path not entered |
| `xcode_broker_fail_closed_observation_is_persisted_from_acp_sink` | 11702 | `process_next_item -> Ok(false)`; changed path not entered |

### Two hidden-result tests, now reproduced on the verified baseline

| Exact test | Hidden call / panic | Observation |
|---|---|---|
| `test_cancel_run_finalize_closes_live_session_via_runtime_manager` | integration.rs:15035 / 15041 | `let _ = process_next_item().await`; then missing lineage |
| `test_invoke_agent_persists_budget_snapshot_and_next_policy_uses_it` | integration.rs:14559 / 14565 | `let _ = process_next_item().await`; then missing lineage |
