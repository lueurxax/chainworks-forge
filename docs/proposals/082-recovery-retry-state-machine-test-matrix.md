# Proposal 082: Recovery and Retry State-Machine Test Matrix

> Source: current unfinished-run proposal artifact.

## Metadata

- **Source run:** `a09a1918-d091-43eb-b01c-fab43271ec22`
- **Source artifact:** `.chainworks/runs/a09a1918-d091-43eb-b01c-fab43271ec22/proposals/approved/proposal.md`
- **Source md5:** `7c338d9dd04456f4f7a7be894f2e0af0`
- **Proposal Id:** P082
- **Proposal Revision Id:** P082-r4-4d5cc83d-20260521
- **Schema Version:** proposal_document_v1
- **Status:** approved_for_implementation_review
- **Source Review Pass Id:** 4d5cc83d-d626-4bda-b6c9-f2eb15d477a1
- **Source Idea:** Implement docs/proposals/082-recovery-retry-state-machine-test-matrix.md end-to-end.
- **Gate Aliases:** ["proposal-082", "p082"]

## Summary

P082 creates the canonical recovery/retry state-machine matrix and proof gate for restart, retry, cancellation, stale startup, late output, side-effect, approval, session, and mediation boundaries. This revision preserves the no-migration posture, keeps rejected-command readback in command_journal.error typed envelopes, adds backward-compatible parsing rules, makes optional GraphQL and Swift consumption tolerant and diagnostic-only, adds explicit held-state and cancel-late-output coverage, defines long-held observability thresholds, and records future UI constraints without expanding P082 into UI implementation.

## Problem

Recovery and retry behavior is spread across startup repair, targeted retry, ACP startup, late output handling, retry identifiers, side-effect reconciliation, cancellation, mediation, and session ownership. Without one matrix and one proof gate, a fix can strengthen one path while weakening another invariant. P082 provides the shared state-machine matrix and focused proof gate that future recovery work must extend before changing behavior.

## Goals

- Add docs/reference/recovery-retry-state-machine-test-matrix.md with scenario IDs, setup, expected repair or rejection, DB assertion, engine assertion, readback requirement, durable storage owner, projection path, crash/replay proof, and long-held observability expectations.
- Add proposal-082 and p082 aliases to scripts/test-gate.sh and document them in docs/reference/test-gates.md.
- Add DB and engine tests proving validation before mutation, unique active ownership, idempotent replay, cancellation convergence, provider cleanup evidence, late-output quarantine, and no blind automatic retry.
- Add MCP/report/run-report tests for exact p082_recovery_matrix_readback_v1 lane placement and parity.
- Keep release side-effect retry fail-closed while unresolved side-effect ledger rows exist.
- Define shared reason-code constants and fixture-enforced nested schemas for retry identifier guidance, late-output settlement, startup repair summaries, and rejected command error envelopes.
- Keep SwiftUI/macOS read-only and tolerant of additive/null/absent daemon fields; do not add app-side recovery authority.
- Carry future UI display constraints as a prerequisite for any later Forge screen that consumes P082 readback.

## Non Goals

- Do not add blind automatic retry.
- Do not auto-resolve human approvals.
- Do not retry release side effects while unresolved durable side-effect ledger entries exist.
- Do not make SwiftUI, app-local RecoveryCoordinator paths, or GraphQL read models recovery mutation authority for P082.
- Do not implement new Forge recovery screens, native notifications, Dock badges, or keyboard/context-menu UI affordances in P082.
- Do not infer recovery truth from logs or loose artifact scans when persisted stage, work-item, runtime-fact, retry-authority, approval, command-journal, mediation, session, or side-effect records exist.
- Do not require a schema migration. If required readback cannot be stored unambiguously in existing durable owners, implementation must stop and this proposal must be amended.

## Ux Ui Notes

- **Approval Behavior:** Pending human approval rows remain pending across restart and cancellation unless the operator explicitly resolves them through the existing approval path.
- **Future Ui Contract:** Before any Forge screen consumes P082 readback, a separate UI proposal must define reason-code display names, severity mapping, ForgeStatusColor treatment for held versus cancelled, accessibility labels/values, text scaling behavior, Xcode grace presentation, empty/null/unavailable states, recovery_next_action display limits, redaction behavior, singular versus plural UI usage, and routing through RunsWorkbenchPresentationModel or its successor on MainActor. Future macOS affordances such as UserNotifications, Dock badges, keyboard shortcuts, and context menus are tracked as follow-up UI scope, not P082 implementation scope.
### Operator Surfaces
- MCP runs.get exposes both result.p082_recovery_matrix_readback and result.p082_recovery_matrix_readbacks.
- MCP reports.get exposes result.p082_recovery_matrix_readbacks and each aggregated report entry may expose report.p082_recovery_matrix_readbacks; reports.get must not expose a singular P082 field.
- report://{run_id} exposes p082_recovery_matrix_readbacks only and must match reports.get snake_case payloads.
- Generated run report JSON exposes p082_recovery_matrix_readbacks only.
- Release receipts expose rollout_contract_readback and may include p082_recovery_matrix_readbacks as additive diagnostic context; they are not a recovery command lane.
- GraphQL is advisory only. If added, it must expose p082RecoveryMatrixReadbackJson and/or p082RecoveryMatrixReadbacksJson as lossless camelCase diagnostic projections with tolerant absent/null/additive/unknown reason-code decode tests.
- **Side Effect Behavior:** Side-effect blocked readback points operators to effects reconciliation tools and never offers retry while unresolved prepared, executing, externally_observed, needs_reconciliation, conflict, or unrecoverable ledger entries exist.
- **Swift Macos Contract:** SwiftUI is out of implementation scope for P082. Existing app-local RecoveryCoordinator behavior is not P082 authority and must not consume P082 readback as an action source. If advisory GraphQL or any Swift app-facing P082 path is added during implementation, focused Swift tests must cover absent fields, null fields, additive fields, unknown reason codes, and MainActor projection delivery. The app must not add app-side retry, approval auto-resolution, or side-effect retry affordances.
- **Xcode Startup Message:** For Xcode-required ACP startup grace, P082-R05 readback must include a non-null recovery_operator_message naming Xcode startup as the reason, the12 minute grace, the cutoff timestamp, and the next check/backoff state. The full-surface fixture must include this Xcode path.

## Architecture

### Documentation
- **Reference Doc:** docs/reference/recovery-retry-state-machine-test-matrix.md
- **Required Content**
  - scenario ID convention and append-only reason-code vocabulary
  - required matrix columns: setup, expected repair or reject, DB assertion, engine assertion, readback requirement, durable owner, projection path, crash/replay proof, and observability threshold
  - instructions for adding rows before future recovery behavior changes
  - typed rejected-command envelope contract for command_journal.error
  - backward-compatible command_journal.error parsing rules
  - lane placement for singular/plural readback fields
  - nested schema contracts
  - fail-closed side-effect behavior
  - late-output quarantine generation semantics
  - approval restart behavior
  - cancellation and crash-during-repair semantics
  - startup_requeue_exhausted held-state behavior
  - cancel-then-late-output behavior
  - provider subprocess cleanup proof expectations
  - Swift/macOS read-only boundary and future UI prerequisites
  - proof gate ownership and command aliases
### Durable Storage Mapping
- **No Migration Posture:** All matrix readback uses existing durable owners. Rejected-command rows use command_journal.error typed envelopes, not command_journal.payload_json. If an implementation cannot persist a required field without mutating write-once payloads or adding ambiguous JSON, it must stop and amend P082 before adding a migration.
- **Projection Accessor:** Add recovery_matrix::readbacks_for_run(pool, run_id) or equivalent. MCP, reports, run report generation, release receipt diagnostics, and advisory GraphQL must call this accessor. The accessor must parse p082_rejected_command_error_v1 defensively and treat legacy command_journal.error text as safe fallback input, never as raw operator display JSON.
- **Readback Field Ownership**
  1. **Item**
     - **Field:** recovery_matrix_scenario_id
     - **Owner:** row-specific JSON owner listed in canonical_matrix.storage_owner.source_json_key or accessor-derived scenario_id
  2. **Item**
     - **Field:** recovery_decision
     - **Owner:** row-specific JSON owner; rejected commands use command_journal.error.p082_recovery_matrix_readback.recovery_decision
  3. **Item**
     - **Field:** recovery_reason_code
     - **Owner:** shared domain recovery_matrix constants; rejected commands mirror it in command_journal.error.reason_code
  4. **Item**
     - **Field:** recovery_next_action
     - **Owner:** row-specific JSON owner; rejected commands use command_journal.error.p082_recovery_matrix_readback.recovery_next_action
  5. **Item**
     - **Field:** recovery_hold_conditions
     - **Owner:** row-specific JSON owner; rejected commands use command_journal.error.p082_recovery_matrix_readback.recovery_hold_conditions
  6. **Item**
     - **Field:** recovery_projection_integrity
     - **Owner:** shared accessor derives from authoritative row presence, validated envelope shape, legacy fallback state, and source consistency; persisted where the owning JSON snapshot is written
### Fixtures
- **Negative Schema:** Every negative fixture must contain schema_version=p082_negative_fixture_v1, fixture_id, expected_failure_code, mutated_contract_or_matrix, and assertion explaining which gate check must fail.
- **Paths**
  - docs/evidence/rollout-contract/operator-readback/p082-full-surface.fixture.json
  - docs/evidence/rollout-contract/negative/p082-missing-matrix-row.json
  - docs/evidence/rollout-contract/negative/p082-missing-db-engine-readback-assertion.json
  - docs/evidence/rollout-contract/negative/p082-release-side-effect-retry-not-fail-closed.json
  - docs/evidence/rollout-contract/negative/p082-blind-automatic-retry.json
  - docs/evidence/rollout-contract/negative/p082-missing-readback-reason.json
  - docs/evidence/rollout-contract/negative/p082-missing-rollout-contract-operator-fields.json
  - docs/evidence/rollout-contract/negative/p082-graphql-required-without-contract.json
  - docs/evidence/rollout-contract/negative/p082-duplicate-requeue-without-idempotency.json
  - docs/evidence/rollout-contract/negative/p082-missing-cancel-crash-rows.json
  - docs/evidence/rollout-contract/negative/p082-rejected-command-payload-mutation.json
  - docs/evidence/rollout-contract/negative/p082-lane-field-name-drift.json
  - docs/evidence/rollout-contract/negative/p082-missing-nested-subcontract.json
  - docs/evidence/rollout-contract/negative/p082-xcode-grace-missing-operator-message.json
  - docs/evidence/rollout-contract/negative/p082-malformed-command-error-envelope.json
  - docs/evidence/rollout-contract/negative/p082-missing-startup-requeue-exhausted-row.json
  - docs/evidence/rollout-contract/negative/p082-cancel-late-output-mutates-active-projection.json
- **Positive Schema:** docs/evidence/rollout-contract/operator-readback/p082-full-surface.fixture.json must contain schema_version=p082_operator_readback_fixture_v1, rollout_contract_readback with required operator_readback_v1 fields, lane payloads for runs_get, reports_get, report_resource, run_report, and release_receipt, exact singular/plural field-name assertions, every nested subcontract, a rejected-command row stored through command_journal.error, a legacy plain-text command_journal.error fallback row, an Xcode startup grace row with non-null recovery_operator_message, startup_requeue_exhausted held-state row, cancel-then-late-output row, and fixture_assertions naming every required reason code.
### Gate
- **Aliases**
  - proposal-082
  - p082
- **Expected Commands**
  - CARGO_TARGET_DIR=target/proposal-082-gate cargo test -p db --test proposal_082_recovery_retry_matrix -- --nocapture
  - CARGO_TARGET_DIR=target/proposal-082-gate cargo test -p engine --test proposal_082_recovery_retry_matrix -- --nocapture
  - CARGO_TARGET_DIR=target/proposal-082-gate cargo test -p mcp-server --test proposal_082_recovery_readback -- --nocapture
- **Fail Closed Conditions**
  - canonical reference matrix missing
  - any required scenario ID absent
  - any row lacks DB, engine, readback, storage owner, projection path, observability threshold, or crash/replay proof
  - post-validation rejected-command P082 readback is mapped to command_journal.payload_json
  - p082_rejected_command_error_v1 parsing exposes raw JSON, rejects legacy plain-text errors unsafely, or panics on malformed JSON
  - p082_recovery_matrix_readback_v1 or any nested subcontract schema is missing or invalid
  - singular/plural lane field names do not match the lane contract
  - rollout_contract_v1 missing required rollout_contract_* operator fields
  - required positive or negative fixtures missing or malformed
  - release side-effect retry is permitted while unresolved effects exist
  - blind automatic retry is added
  - approval restart auto-resolves a human approval
  - late output from superseded or cancelled execution updates active truth or leaves superseded source work pending/running
  - second startup requeue creates duplicate work instead of replaying or holding
  - startup_requeue_exhausted held-state row is absent
  - Xcode startup grace lacks a non-null operator message
  - provider subprocess cleanup proof is absent from cancellation/crash-repair tests
  - GraphQL is listed as a required readback lane or implemented without tolerant diagnostic-only tests
  - Swift app-facing consumption is implemented without absent/null/additive/MainActor tolerance tests
- **Optional Commands**
  - CARGO_TARGET_DIR=target/proposal-082-gate cargo test -p graphql-server proposal_082_ -- --nocapture, only if advisory GraphQL P082 readback is implemented
  - Swift absent/null/additive/MainActor decode tests, only if a Swift app-facing P082 path is implemented
- **Script:** scripts/test-gate.sh
### Metrics
- **Adoption Metric:** p082_recovery_matrix_rows_with_db_engine_readback_coverage_percent
- **Emission Sites**
  1. **p082_recovery_matrix_gate_result_total{scenario_id,status}**
     - **Metric:** p082_recovery_matrix_gate_result_total{scenario_id,status}
     - **Site:** proposal-082 gate harness after each scenario assertion group
  2. **p082_recovery_reason_readback_total{reason_code,lane}**
     - **Metric:** p082_recovery_reason_readback_total{reason_code,lane}
     - **Site:** shared readback accessor when emitting MCP/report/run-report/release diagnostic payloads
  3. **p082_recovery_mutation_rejected_total{reason_code,command}**
     - **Metric:** p082_recovery_mutation_rejected_total{reason_code,command}
     - **Site:** engine command handler before mutation on retry/cancel/recovery rejection and command_journal.error envelope write
  4. **p082_release_side_effect_retry_block_total{effect_status,command}**
     - **Metric:** p082_release_side_effect_retry_block_total{effect_status,command}
     - **Site:** side-effect retry eligibility check before scheduling release work
  5. **p082_late_output_quarantine_total{settlement,source_generation}**
     - **Metric:** p082_late_output_quarantine_total{settlement,source_generation}
     - **Site:** artifact/output settlement path when a superseded or cancelled generation output is ignored
  6. **p082_recovery_idempotency_replay_total{scenario_id,result}**
     - **Metric:** p082_recovery_idempotency_replay_total{scenario_id,result}
     - **Site:** startup repair, retry payload recovery, cancellation replay, and crash-loop replay paths when an idempotency key already exists
  7. **p082_recovery_state_age_seconds{scenario_id,reason_code}**
     - **Metric:** p082_recovery_state_age_seconds{scenario_id,reason_code}
     - **Site:** readback accessor for pending approvals, side-effect reconciliation holds, startup repair holds, startup_requeue_exhausted, and Xcode startup grace holds
### Nested Subcontracts
- **P082 Late Output Settlement V1**
  - **Claim State Values**
    - superseded
    - closed
    - ignored
  - **Invariants**
    - active_projection_changed must be false
    - source_work_item_terminal_status must be completed or failed, never pending or running, after ignored late output is settled
    - cancelled_provider_session is true when output arrived from a cancelled provider session
  - **Output Settlement Values**
    - ignored
    - quarantined
  - **Required Fields**
    - schema_version=p082_late_output_settlement_v1
    - source_agent_execution_id
    - source_work_item_id
    - source_session_generation_id
    - active_session_generation_id
    - claim_state
    - output_settlement
    - ignored_late_output_count
    - source_work_item_terminal_status
    - active_projection_changed
    - cancelled_provider_session
- **P082 Rejected Command Error V1**
  - **Backward Compatibility:** The shared readback accessor must detect this envelope only when command_journal.error parses as JSON, schema_version equals p082_rejected_command_error_v1, and p082_recovery_matrix_readback validates. Legacy plain-text errors remain operator-safe summaries with no raw JSON exposure, recovery_projection_integrity=unavailable or stale as appropriate, and no panic.
  - **Owner:** command_journal.error text column
  - **Required Fields**
    - schema_version=p082_rejected_command_error_v1
    - reason_code
    - command_type
    - redaction
    - operator_safe_summary
    - p082_recovery_matrix_readback
  - **Rule:** Post-validation rejected-command readback must be written to command_journal.error as this typed redacted JSON envelope. command_journal.payload_json remains the inserted command input and must not be mutated for P082 readback.
- **P082 Retry Identifier Guidance V1**
  - **Expected Identifier Kind Values**
    - workflow_stage_id
    - stage_execution_uuid
    - retry_authority_id
    - work_item_id
  - **No Mutation:** must be true for rejected identifier mismatch rows
  - **Provided Identifier Kind Values**
    - workflow_stage_id
    - stage_execution_uuid
    - retry_authority_id
    - work_item_id
    - unknown
  - **Required Fields**
    - schema_version=p082_retry_identifier_guidance_v1
    - command
    - provided_identifier
    - provided_identifier_kind
    - expected_identifier_kind
    - valid_identifier_examples
    - no_mutation
- **P082 Startup Repair Summary V1**
  - **Required Fields**
    - schema_version=p082_startup_repair_summary_v1
    - startup_repair_id
    - source_work_item_id
    - source_command_journal_id
    - requeue_generation
    - max_requeue_generation
    - replayed
    - stale_after_ms
    - stale_cutoff
    - xcode_required
    - next_retry_or_backoff_time
    - backpressure_scope
  - **Semantics**
    - max_requeue_generation is1 for P082 startup requeue proof
    - a second startup requeue attempt for the same idempotency key must not enqueue duplicate work and must emit startup_requeue_exhausted or replayed=true depending on whether the existing generation is still valid
    - next_retry_or_backoff_time is null when no pending startup-recovery work remains; otherwise it is the minimum scheduled_at among pending startup recovery/backpressure work for the run
### Observability Thresholds
- **Metric:** p082_recovery_state_age_seconds{scenario_id,reason_code}
- **Rule:** Thresholds are proof and readback expectations for P082 metrics. They do not add paging policy or UI notifications in this proposal.
- **Thresholds**
  1. **Item**
     - **Critical Seconds:** `259200`
     - **Operator Message:** Approval has been pending for more than the expected review window.
     - **State:** pending approval
     - **Warning Seconds:** `86400`
  2. **Item**
     - **Critical Seconds:** `14400`
     - **Operator Message:** Side-effect reconciliation is blocking retry or cancellation settlement.
     - **State:** side-effect reconciliation hold
     - **Warning Seconds:** `3600`
  3. **Item**
     - **Critical Seconds:** `1800`
     - **Operator Message:** Startup recovery remains held after the expected repair window.
     - **State:** startup repair hold
     - **Warning Seconds:** `900`
  4. **Item**
     - **Critical Seconds:** `900`
     - **Operator Message:** Xcode startup grace exceeded the12 minute window; inspect Xcode broker/session startup.
     - **State:** Xcode startup grace
     - **Warning Seconds:** `720`
  5. **Item**
     - **Critical Seconds:** `300`
     - **Operator Message:** Startup requeue exhausted and requires operator clearance through existing recovery inspection paths.
     - **State:** startup_requeue_exhausted
     - **Warning Seconds:** `0`
### P082 Recovery Matrix Readback V1
- **Casing:** run_report, mcp, reports.get, report resources, and release receipts use snake_case. Advisory GraphQL uses camelCase with canonical string enum values.
- **Parity Requirements**
  - A single readback accessor must build MCP runs.get, reports.get, report resource, run report, and release receipt diagnostic payloads.
  - Fixtures must assert exact singular/plural field names per lane.
  - If advisory GraphQL is implemented, tests must prove camelCase conversion is lossless, tolerant, diagnostic-only, and not scheduling authority.
- **Redaction And Absent Data:** Readback may include identifiers, reason codes, counts, and operator-safe paths relative to the run meta-root. It must not include provider transcripts, raw stderr, auth material, raw diagnostics, or unredacted command payloads. Missing optional subcontracts are null, not omitted. Missing arrays are empty arrays. Unavailable authoritative records use scenario_status held or pending, recovery_projection_integrity unavailable, and a reason code explaining the missing owner. Operator displays must render sanitized summaries and must not print raw command_journal.error envelope JSON.
- **Required Fields**
  1. **schema_version**
     - **Name:** schema_version
     - **Type:** string
     - **Value:** p082_recovery_matrix_readback_v1
  2. **scenario_id**
     - **Name:** scenario_id
     - **Null Behavior:** never null
     - **Type:** string
  3. **scenario_status**
     - **Name:** scenario_status
     - **Null Behavior:** never null
     - **Type:** enum
     - **Values**
       - repaired
       - rejected
       - held
       - pending
       - cancelled
       - not_applicable
  4. **recovery_decision**
     - **Name:** recovery_decision
     - **Null Behavior:** never null
     - **Type:** enum
     - **Values**
       - retry
       - wait
       - reconcile_side_effects
       - operator_approval_required
       - inspect_duplicate_owner
       - cancel
       - no_mutation
  5. **recovery_reason_code**
     - **Name:** recovery_reason_code
     - **Null Behavior:** never null
     - **Type:** string enum
  6. **recovery_next_action**
     - **Name:** recovery_next_action
     - **Null Behavior:** empty string only when scenario_status is not_applicable
     - **Type:** string
  7. **recovery_hold_conditions**
     - **Name:** recovery_hold_conditions
     - **Null Behavior:** empty array when none
     - **Type:** string array
  8. **recovery_side_effect_blocking_status**
     - **Name:** recovery_side_effect_blocking_status
     - **Null Behavior:** null when scenario is not side-effect-owned
     - **Type:** string or null
  9. **recovery_retry_identifier_guidance**
     - **Name:** recovery_retry_identifier_guidance
     - **Null Behavior:** null except identifier guidance scenarios
     - **Type:** p082_retry_identifier_guidance_v1 or null
  10. **recovery_late_output_settlement**
     - **Name:** recovery_late_output_settlement
     - **Null Behavior:** null except late-output scenarios
     - **Type:** p082_late_output_settlement_v1 or null
  11. **recovery_startup_repair_summary**
     - **Name:** recovery_startup_repair_summary
     - **Null Behavior:** null except startup or crash-repair scenarios
     - **Type:** p082_startup_repair_summary_v1 or null
  12. **recovery_operator_message**
     - **Name:** recovery_operator_message
     - **Null Behavior:** non-null for Xcode startup grace, startup_requeue_exhausted, side-effect holds, and other operator-held states; otherwise may be null
     - **Type:** escaped plain text or null
  13. **recovery_projection_integrity**
     - **Name:** recovery_projection_integrity
     - **Null Behavior:** never null
     - **Type:** enum
     - **Values**
       - valid
       - stale
       - tamper_detected
       - unavailable
  14. **source_table**
     - **Name:** source_table
     - **Null Behavior:** never null
     - **Type:** string
  15. **source_repository**
     - **Name:** source_repository
     - **Null Behavior:** never null
     - **Type:** string
  16. **source_identifier**
     - **Name:** source_identifier
     - **Null Behavior:** never null
     - **Type:** string
  17. **source_json_key**
     - **Name:** source_json_key
     - **Null Behavior:** null only when the source is a typed column rather than JSON text
     - **Type:** string or null
  18. **updated_at**
     - **Name:** updated_at
     - **Null Behavior:** never null
     - **Type:** ISO-8601 string
  19. **diagnostic_redaction**
     - **Name:** diagnostic_redaction
     - **Null Behavior:** never null
     - **Type:** enum
     - **Values**
       - none
       - partial
       - full
- **Schema Version:** p082_recovery_matrix_readback_v1
### Readback Lane Contract
- **Lanes**
  1. **Item**
     - **Fields**
       - **P082 Recovery Matrix Readback:** object or null, latest applicable row
       - **P082 Recovery Matrix Readbacks:** array, all applicable rows sorted by updated_at then scenario_id
     - **Fixture Assertion:** Fixture must assert both exact field names are present.
     - **Lane:** mcp runs.get
  2. **Item**
     - **Fields**
       - **P082 Recovery Matrix Readbacks:** array at result level and optional per-report array
     - **Fixture Assertion:** Fixture must assert plural field is present and singular p082_recovery_matrix_readback is absent from reports.get.
     - **Lane:** mcp reports.get
  3. **Item**
     - **Fields**
       - **P082 Recovery Matrix Readbacks:** array only
     - **Fixture Assertion:** Fixture must assert byte-equivalent snake_case rows with reports.get.
     - **Lane:** report resource report://{run_id}
  4. **Item**
     - **Fields**
       - **P082 Recovery Matrix Readbacks:** array only
     - **Fixture Assertion:** Fixture must assert exact field name and parity with report resource.
     - **Lane:** run report JSON
  5. **Item**
     - **Fields**
       - **P082 Recovery Matrix Readbacks:** array diagnostic, may be empty
       - **Rollout Contract Readback:** operator_readback_v1 object
     - **Fixture Assertion:** Fixture must assert release receipt keeps rollout_contract_* fields and does not expose recovery command affordances.
     - **Lane:** release receipt
  6. **Item**
     - **Fields**
       - **P082Recoverymatrixreadbackjson:** object or null if implemented
       - **P082Recoverymatrixreadbacksjson:** array if implemented
     - **Fixture Assertion:** Only required if GraphQL is implemented; must assert camelCase lossless projection, absent/null/additive/unknown reason-code tolerance, and diagnostic-only semantics.
     - **Lane:** advisory GraphQL
- **Latest Selection:** When a lane exposes singular p082_recovery_matrix_readback, it is the non-not_applicable row with the latest updated_at; ties sort by scenario_id ascending and choose the last row after sorting. If no row applies, singular value is null and plural value is an empty array.
### Reason Code Ownership
- **Codes**
  - resume_claim_status
  - startup_requeue_once
  - startup_requeue_exhausted
  - invalid_stage_for_retry
  - ignored_late_outputs
  - duplicate_owner_repaired
  - startup_stalled
  - stale_repaired
  - needs_effect_reconciliation
  - requires_effect_reconciliation
  - valid_identifier_guidance
  - approval_pending_operator_action_required
  - duplicate_mediation_owner_rejected
  - cancel_active_stage_requested
  - cancel_pending_approval_preserved
  - cancel_side_effect_reconciliation_required
  - cancel_startup_repair_converged
  - cancelled_provider_late_output_ignored
  - repair_crash_resume_idempotent
- **Module:** control-plane/crates/domain/src/recovery_matrix.rs or equivalent shared domain constants module
- **Rule:** Reason codes are append-only public strings consumed by engine, DB tests, MCP reports, run reports, release diagnostics, and advisory GraphQL tests. UI display names and severity mapping are future UI proposal scope.
### Reliability Semantics
- **Cancellation Replay:** Cancellation tests must replay after partial settlement and prove no duplicate work, owners, side effects, or provider sessions. Provider session cleanup must be tied to durable session_generations terminalization/session_events evidence and existing ACP transport subprocess lifecycle evidence: observable provider subprocess reaping, terminal response, or bounded absence proof from the ACP transport contract.
- **Crash Injection:** DB and engine tests must inject or simulate crash/restart after each durable write boundary: after idempotency row insert, after session invalidation, after work item status/payload mutation, after command_journal.error settlement, after cancellation_settlement_log update, after side-effect hold recording, and after readback/projection write.
- **Crash Loop Variant:** P082-R15 must include a repeated-crash variant where the same idempotency key is observed across multiple restarts and convergence is still single-owner, single-work-item, and single-readback-row.
- **Late Output Terminalization:** P082-R03 must prove ignored late output terminalizes or closes the superseded source work_item and source generation claim. P082-R17 proves the same invariant when output arrives after provider-session cancellation.
- **Restart Backpressure:** Startup requeue flood control uses existing scheduler queue/backpressure projections. P082 does not add a new scheduler cap, but requires readback to expose queued_under_startup_recovery_backpressure_count and next_retry_or_backoff_time semantics via startup_recovery_readbacks.
- **Startup Requeue:** P082-R01 permits one startup repair requeue generation per source command/work item idempotency key. Replaying the same generation is idempotent. A second attempt after generation1 has already been consumed must fail closed with startup_requeue_exhausted unless the existing generation is simply being replayed after a crash. P082-R16 proves the held state explicitly.
- **Xcode Grace:** P082-R05 uses standard3 minute stale ACP startup grace in tests and existing12 minute Xcode-required grace for Xcode startup. Xcode path requires non-null recovery_operator_message and a fixture row.

## Canonical Matrix

1. **P082-R01**
   - **Db Assertion:** One pending/running work item exists for the command target; startup_repairs contains one idempotency row; work_items.payload_json.p061_startup_recovery carries source_command_journal_id, source_work_item_id, startup_repair_id, requeue_generation, requeued_at, reason, and max_requeue_generation=1.
   - **Engine Assertion:** No duplicate stage execution, agent execution, session generation, retry authority, or side effect is created; crash after each durable write boundary converges on restart.
   - **Expected Repair Or Reject:** Write or confirm startup_repairs.id=p082-requeue:{command_journal.id}:{source_work_item_id}:1 before mutation. Requeue generation1 exactly once, replay idempotently after crash, or hold with startup_requeue_exhausted if a second non-replay requeue would be required.
   - **Id:** P082-R01
   - **Readback Requirement:** p082_startup_repair_summary_v1 populated; next_retry_or_backoff_time follows startup_recovery_readbacks semantics.
   - **Reason Code:** startup_requeue_once
   - **Scenario:** Restart mid command
   - **Setup:** command_journal has an accepted command; related work item is unsettled after daemon restart.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R01]
     - **Source Identifier:** startup_repairs.id=p082-requeue:{command_journal.id}:{source_work_item_id}:1
     - **Source Json Key:** startup_repairs.notes.p082_recovery_matrix_readback and work_items.payload_json.p061_startup_recovery
     - **Source Repository:** startup_repairs, work_items, command_journal
     - **Source Table:** startup_repairs, work_items, command_journal, startup_recovery_readbacks
2. **P082-R02**
   - **Db Assertion:** No new stage execution, work item, retry authority, or instruction binding exists; command_journal.result_status is rejected/failed and command_journal.error contains p082_rejected_command_error_v1. command_journal.payload_json is not mutated for P082 readback.
   - **Engine Assertion:** Command handler validates retry eligibility before enqueue or authority creation and writes the redacted error envelope as terminal command readback.
   - **Expected Repair Or Reject:** Reject before mutation with typed denial.
   - **Id:** P082-R02
   - **Readback Requirement:** Readback names invalid_stage_for_retry, recovery_decision=no_mutation, and uses command_journal.error.p082_recovery_matrix_readback as source.
   - **Reason Code:** invalid_stage_for_retry
   - **Scenario:** Reject non-retryable stage retry
   - **Setup:** stages.retry targets a stage/status that policy does not allow.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R02]
     - **Source Identifier:** command_journal.id for rejected retry command
     - **Source Json Key:** command_journal.error.p082_recovery_matrix_readback
     - **Source Repository:** command_journal, stages
     - **Source Table:** command_journal, stage_executions
3. **P082-R03**
   - **Db Assertion:** artifact_source_generation_claims.claim_state is superseded/closed; agent_execution_runtime_facts.ignored_late_output_count increments; active artifact generations remain tied to active source_session_generation_id/source_work_item_id; superseded source work_item is completed or failed, never pending/running.
   - **Engine Assertion:** Active stage projection and artifact links are not regressed by superseded output.
   - **Expected Repair Or Reject:** Ignore or quarantine old output, preserve superseded evidence, and terminalize/close the superseded source work item.
   - **Id:** P082-R03
   - **Readback Requirement:** p082_late_output_settlement_v1 populated with active_projection_changed=false.
   - **Reason Code:** ignored_late_outputs
   - **Scenario:** Late output after supersede
   - **Setup:** Old agent output arrives after retry creates a superseding attempt and source generation claim is superseded.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R03]
     - **Source Identifier:** artifact_source_generation_claims primary key plus work_items.id
     - **Source Json Key:** stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback
     - **Source Repository:** agent_execution_runtime_facts, artifact_contracts, work_items, agent_executions
     - **Source Table:** agent_execution_runtime_facts, artifact_source_generation_claims, artifact_contract_generations, work_items, agent_executions
4. **P082-R04**
   - **Db Assertion:** One active session_lineages.active_generation_id and one active session_generations row for invocation_owner_key; duplicate session_events row is terminal/rejected evidence.
   - **Engine Assertion:** Scheduler capacity and provider startup are not double counted.
   - **Expected Repair Or Reject:** Keep one durable owner; reject or terminalize duplicate evidence.
   - **Id:** P082-R04
   - **Readback Requirement:** Readback names duplicate_owner_repaired and inspect_duplicate_owner next action when evidence needs review.
   - **Reason Code:** duplicate_owner_repaired
   - **Scenario:** Duplicate session/startup claim
   - **Setup:** Two startup claims or session starts target the same active work item.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R04]
     - **Source Identifier:** session_generations.invocation_owner_key and work_items.id
     - **Source Json Key:** session_events.details_json.p082_recovery_matrix_readback
     - **Source Repository:** sessions, work_items
     - **Source Table:** session_lineages, session_generations, session_events, work_items
5. **P082-R05**
   - **Db Assertion:** session_generations ended/invalidated with end_reason=stale_acp_startup_without_provider_session; session_events records invalidation; work_items.payload_json.p061_startup_recovery.reason is startup_repair_stale_acp_startup or startup_repair_stale_acp_pre_session_startup; only one replacement pending item exists.
   - **Engine Assertion:** Startup and watchdog paths share eligibility and idempotency; provider capacity is not consumed twice.
   - **Expected Repair Or Reject:** Invalidate startup generation and requeue once when eligible. Standard grace is3 minutes in tests; Xcode-required startup uses12 minutes and requires operator message.
   - **Id:** P082-R05
   - **Readback Requirement:** p082_startup_repair_summary_v1 populated. If xcode_required=true, recovery_operator_message is non-null and names Xcode startup grace and cutoff.
   - **Reason Code:** startup_stalled
   - **Scenario:** Stale ACP startup
   - **Setup:** Running invoke work has no provider_session_id and no last_activity_at after stale grace.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R05]
     - **Source Identifier:** work_items.id and session_generations.id
     - **Source Json Key:** work_items.payload_json.p061_startup_recovery
     - **Source Repository:** work_items, sessions, startup_repairs
     - **Source Table:** work_items, session_generations, session_events, startup_recovery_readbacks
6. **P082-R06**
   - **Db Assertion:** work_items status changes only through recorded transition; side_effects unresolved rows remain unchanged.
   - **Engine Assertion:** Capacity is freed only through recorded transition; side-effected stages are held.
   - **Expected Repair Or Reject:** Repair through explicit transition or hold for reconciliation; never blind retry release work or side-effected stages.
   - **Id:** P082-R06
   - **Readback Requirement:** Reason is stale_repaired when repaired or needs_effect_reconciliation when held.
   - **Reason Code:** stale_repaired
   - **Scenario:** Stale scheduler ownership
   - **Setup:** Running work item has no live executor owner.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R06]
     - **Source Identifier:** work_items.id plus optional startup_repairs.id
     - **Source Json Key:** work_items.payload_json.p061_startup_recovery or startup_repairs.notes.p082_recovery_matrix_readback
     - **Source Repository:** work_items, startup_repairs, side_effects
     - **Source Table:** work_items, startup_repairs, side_effects
7. **P082-R07**
   - **Db Assertion:** side_effects.status unchanged by retry attempt; no side_effect_attempts retry row; no release work item scheduled; rejected retry command stores p082_rejected_command_error_v1 in command_journal.error if command-scoped.
   - **Engine Assertion:** No duplicate push/upload/archive/tag/publish/commit mutation is scheduled.
   - **Expected Repair Or Reject:** Block retry and route to side-effect reconciliation.
   - **Id:** P082-R07
   - **Readback Requirement:** recovery_side_effect_blocking_status populated and recovery_operator_message non-null for held state.
   - **Reason Code:** requires_effect_reconciliation
   - **Scenario:** Release side-effect drift
   - **Setup:** Unresolved side_effects row exists for run or target stage.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R07]
     - **Source Identifier:** side_effects.idempotency_key or command_journal.id
     - **Source Json Key:** command_journal.error.p082_recovery_matrix_readback for rejected command; side_effects.status typed column for blocking status
     - **Source Repository:** side_effects, command_journal
     - **Source Table:** side_effects, side_effect_attempts, side_effect_settlements, command_journal
8. **P082-R08**
   - **Db Assertion:** No retry mutation, work item, authority, or instruction binding exists; command_journal.error contains p082_rejected_command_error_v1 with p082_retry_identifier_guidance_v1; command_journal.payload_json is not mutated.
   - **Engine Assertion:** MCP error and report readback name expected identifier kind and examples.
   - **Expected Repair Or Reject:** Reject with deterministic guidance before mutation.
   - **Id:** P082-R08
   - **Readback Requirement:** p082_retry_identifier_guidance_v1 populated with no_mutation=true.
   - **Reason Code:** valid_identifier_guidance
   - **Scenario:** Retry identifier mismatch
   - **Setup:** Operator supplies wrong identifier kind for retry command.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R08]
     - **Source Identifier:** command_journal.id or retry_payload_recovery_events.idempotency_key
     - **Source Json Key:** command_journal.error.p082_recovery_matrix_readback or retry_payload_recovery_events.diagnostic_json.p082_recovery_matrix_readback
     - **Source Repository:** command_journal, retry_payload_recovery_events
     - **Source Table:** command_journal, retry_payload_recovery_events
9. **P082-R09**
   - **Db Assertion:** approvals.decision remains pending; approval_inbox contains pending approval; decided_at remains null.
   - **Engine Assertion:** Orchestrator waits at approval gate and does not synthesize approval/rejection.
   - **Expected Repair Or Reject:** Restore pending approval visibility without auto-resolution.
   - **Id:** P082-R09
   - **Readback Requirement:** recovery_decision=operator_approval_required and next action points to existing approval path.
   - **Reason Code:** approval_pending_operator_action_required
   - **Scenario:** Pending human approval restart
   - **Setup:** Daemon restarts while approval is pending.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R09]
     - **Source Identifier:** approvals.id
     - **Source Json Key:** stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback
     - **Source Repository:** approvals, projections, stages
     - **Source Table:** approvals, approval_inbox, stage_executions
10. **P082-R10**
   - **Db Assertion:** lead_conflict_mediations active fingerprint uniqueness preserved; lead_mediation_confirmations has at most one pending row per mediation.
   - **Engine Assertion:** No duplicate lead conflict settlement possible.
   - **Expected Repair Or Reject:** Keep one active mediation owner and preserve duplicate evidence.
   - **Id:** P082-R10
   - **Readback Requirement:** Readback points to current mediation owner.
   - **Reason Code:** duplicate_mediation_owner_rejected
   - **Scenario:** Duplicate mediation attempt
   - **Setup:** Duplicate mediation/session attempt is observed for same conflict owner.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R10]
     - **Source Identifier:** lead_conflict_mediations.conflict_fingerprint and workflow_conflicts.conflict_id
     - **Source Json Key:** lead_conflict_mediations.validation_errors_json.p082_recovery_matrix_readback or workflow_conflicts.record_json.p082_recovery_matrix_readback
     - **Source Repository:** lead_conflict_mediations, lead_mediation_confirmations, workflow_conflicts, agent_executions
     - **Source Table:** lead_conflict_mediations, lead_mediation_confirmations, workflow_conflicts, agent_executions
11. **P082-R11**
   - **Db Assertion:** runs.cancellation_requested_at set; cancellation_settlement_log contains one action_id; work_items settled exactly once; session_generations/session_events record terminalization or shutdown evidence; no duplicate retry authority.
   - **Engine Assertion:** No duplicate work, owners, side effects, or orphaned provider sessions remain after replay. Provider subprocess cleanup proof cites ACP transport lifecycle evidence: terminal response, observable reaping, or bounded absence proof.
   - **Expected Repair Or Reject:** Settle cancellation through existing path; prevent new invoke work and terminalize active provider session record through durable session evidence.
   - **Id:** P082-R11
   - **Readback Requirement:** scenario_status=cancelled or held with clear held-vs-cancelled message.
   - **Reason Code:** cancel_active_stage_requested
   - **Scenario:** Cancel interleaved with active stage or retry work
   - **Setup:** Operator cancels while active stage, retry authority, invoke work item, or provider session is running.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R11]
     - **Source Identifier:** runs.id and runs.cancellation_settlement_log.action_id
     - **Source Json Key:** runs.cancellation_settlement_log.p082_recovery_matrix_readback
     - **Source Repository:** runs, work_items, retry_stage_execution_authorities, sessions
     - **Source Table:** runs, work_items, retry_stage_execution_authorities, session_generations, session_events
12. **P082-R12**
   - **Db Assertion:** approvals.decided_at remains null unless explicit operator decision exists; approval_inbox changes are tied to cancellation settlement, not approval synthesis.
   - **Engine Assertion:** Approval gate does not resume work after cancellation.
   - **Expected Repair Or Reject:** Cancel run without converting approval decision to approved/rejected.
   - **Id:** P082-R12
   - **Readback Requirement:** Next action describes cancellation settlement, not approval retry.
   - **Reason Code:** cancel_pending_approval_preserved
   - **Scenario:** Cancel interleaved with pending approval
   - **Setup:** Operator cancels while approval is pending.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R12]
     - **Source Identifier:** runs.id and approvals.id
     - **Source Json Key:** runs.cancellation_settlement_log.p082_recovery_matrix_readback
     - **Source Repository:** runs, approvals, projections
     - **Source Table:** runs, approvals, approval_inbox
13. **P082-R13**
   - **Db Assertion:** side_effects.status unchanged except explicit reconciliation; no side_effect_attempts retry row; cancellation_settlement_log records hold.
   - **Engine Assertion:** No duplicate external side effect scheduled and cancellation does not mask reconciliation.
   - **Expected Repair Or Reject:** Cancel scheduling and hold external-effect settlement for reconciliation; do not retry or implicitly settle effects.
   - **Id:** P082-R13
   - **Readback Requirement:** recovery_decision=reconcile_side_effects and recovery_operator_message non-null.
   - **Reason Code:** cancel_side_effect_reconciliation_required
   - **Scenario:** Cancel interleaved with unresolved side effects
   - **Setup:** Operator cancels while unresolved side_effects rows exist.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R13]
     - **Source Identifier:** side_effects.idempotency_key and runs.cancellation_settlement_log.action_id
     - **Source Json Key:** runs.cancellation_settlement_log.p082_recovery_matrix_readback
     - **Source Repository:** runs, side_effects
     - **Source Table:** runs, side_effects, side_effect_attempts, side_effect_settlements
14. **P082-R14**
   - **Db Assertion:** startup_repairs idempotency row remains single; work_items do not contain cancelled and pending duplicates for same source; cancellation_settlement_log records interaction.
   - **Engine Assertion:** Replay in either order converges without duplicate work, owners, or provider sessions.
   - **Expected Repair Or Reject:** Cancellation wins for future scheduling; already-journaled repair converges idempotently.
   - **Id:** P082-R14
   - **Readback Requirement:** p082_startup_repair_summary_v1 names repair idempotency key and replay state.
   - **Reason Code:** cancel_startup_repair_converged
   - **Scenario:** Cancel interleaved with startup repair
   - **Setup:** Operator cancel request races with startup recovery requeue or stale repair.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R14]
     - **Source Identifier:** startup_repairs.id plus runs.cancellation_settlement_log.action_id
     - **Source Json Key:** startup_repairs.notes.p082_recovery_matrix_readback and runs.cancellation_settlement_log.p082_recovery_matrix_readback
     - **Source Repository:** runs, startup_repairs, work_items, sessions
     - **Source Table:** runs, startup_repairs, work_items, session_generations
15. **P082-R15**
   - **Db Assertion:** Exactly one durable repair key exists for affected subsystem. Repeated recovery pass creates no duplicates. Tests cover crash boundary after each durable write listed in reliability_semantics.crash_injection and at least one crash-loop replay variant.
   - **Engine Assertion:** Recovery service resumes from every injected crash point and leaves projections consistent. Provider subprocess cleanup proof is tied to existing ACP transport lifecycle evidence when a provider session is involved.
   - **Expected Repair Or Reject:** Replay repair using subsystem idempotency key and converge without duplicate mutation, including repeated crashes on the same idempotency key across restarts.
   - **Id:** P082-R15
   - **Readback Requirement:** recovery_projection_integrity=valid and replayed=true when duplicate key observed.
   - **Reason Code:** repair_crash_resume_idempotent
   - **Scenario:** Daemon crash during repair
   - **Setup:** Daemon crashes after repair eligibility check and after one durable write, before final readback/projection settlement.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R15]
     - **Source Identifier:** row-specific idempotency key named in test case
     - **Source Json Key:** row-specific p082_recovery_matrix_readback JSON owner or command_journal.error.p082_recovery_matrix_readback for rejected commands
     - **Source Repository:** startup_repairs, retry_payload_recovery_events, side_effects, lead_conflict_mediations, lead_mediation_confirmations, runs, command_journal
     - **Source Table:** startup_repairs, retry_payload_recovery_events, side_effects, lead_conflict_mediations, lead_mediation_confirmations, runs, command_journal
16. **P082-R16**
   - **Db Assertion:** startup_repairs retains one idempotency row; no second pending work item exists for the same source_work_item_id; recovery readback carries scenario_status=held and non-null recovery_operator_message.
   - **Engine Assertion:** No duplicate generations or provider sessions are created; scheduler capacity is not consumed for a forbidden second requeue.
   - **Expected Repair Or Reject:** Hold without enqueueing duplicate work, without creating a new session generation, and without mutating side effects. Operator clearance must use existing recovery inspection or cancellation paths, not a new P082 command.
   - **Id:** P082-R16
   - **Readback Requirement:** scenario_status=held, recovery_reason_code=startup_requeue_exhausted, p082_startup_repair_summary_v1 populated, operator message names the existing clearance path.
   - **Reason Code:** startup_requeue_exhausted
   - **Scenario:** Startup requeue exhausted held state
   - **Setup:** Startup recovery observes the same source command/work item after the allowed requeue generation has already been consumed and the existing replay cannot be proven valid.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R16]
     - **Source Identifier:** startup_repairs.id=p082-requeue:{command_journal.id}:{source_work_item_id}:1
     - **Source Json Key:** startup_repairs.notes.p082_recovery_matrix_readback
     - **Source Repository:** startup_repairs, work_items
     - **Source Table:** startup_repairs, work_items, startup_recovery_readbacks
17. **P082-R17**
   - **Db Assertion:** session_generations/session_events show cancellation or terminalization; artifact_source_generation_claims is superseded/closed; source work_item terminal; agent_execution_runtime_facts.ignored_late_output_count increments; active artifact/projection rows unchanged.
   - **Engine Assertion:** Cancelled provider output cannot update active artifacts, reports, stage projections, retry authority, or side-effect state.
   - **Expected Repair Or Reject:** Classify as late output, quarantine or ignore, preserve evidence, and make no active projection mutation.
   - **Id:** P082-R17
   - **Readback Requirement:** p082_late_output_settlement_v1 populated with cancelled_provider_session=true and active_projection_changed=false.
   - **Reason Code:** cancelled_provider_late_output_ignored
   - **Scenario:** Cancel then late provider output
   - **Setup:** A provider session is cancelled or terminalized, then output arrives from that cancelled/superseded source generation.
   - **Storage Owner**
     - **Projection Path:** p082_recovery_matrix_readbacks[scenario_id=P082-R17]
     - **Source Identifier:** session_generations.id plus source_work_item_id
     - **Source Json Key:** stage_executions.recovery_snapshot_json.p082_recovery_matrix_readback
     - **Source Repository:** sessions, artifact_contracts, agent_execution_runtime_facts, work_items
     - **Source Table:** session_generations, session_events, artifact_source_generation_claims, agent_execution_runtime_facts, work_items

## Rollout Contract V1

- **Applicability:** required
### Commands
- **Allowlist**
  - ./scripts/test-gate.sh proposal-082
  - ./scripts/test-gate.sh p082
- **Commentary:** Gate commands are declarative expectations; the linter must not execute them.
### Decision Vocabulary
- release
- hold
- waive
- not_applicable
- timeout
### Gate Aliases
- proposal-082
- p082
### Hold Conditions
- Canonical matrix document is missing or omits a required scenario row
- Any matrix row lacks DB, engine, operator readback, durable storage owner, projection path, observability threshold, or crash/replay proof
- Rejected-command readback is stored in command_journal.payload_json instead of command_journal.error typed envelope
- p082_rejected_command_error_v1 parsing is not backward-compatible with legacy plain-text command_journal.error values
- p082_recovery_matrix_readback_v1 or nested subcontract schema is missing or invalid
- Lane singular/plural field placement drifts from the proposal contract
- Release retry proceeds while unresolved side-effect ledger entries exist
- Recovery path mutates state before validating eligibility
- Implementation adds blind automatic retry or auto-resolves human approvals
- Late output from a superseded or cancelled execution can update active truth or leave source work pending/running
- Startup requeue generation2 creates duplicate work instead of replaying or holding
- Startup requeue exhausted held-state coverage is absent
- Xcode startup grace lacks non-null operator message
- Provider subprocess cleanup proof is absent from cancellation/crash-repair rows
- GraphQL is implemented without tolerant diagnostic-only tests
- Swift app-facing P082 consumption is implemented without absent/null/additive/MainActor tolerance tests
### Hold Conditions Detail
- The matrix is the authoritative checklist for recovery and retry behavior changes.
- command_journal.payload_json is the inserted command input and is not a post-validation readback owner for P082.
- GraphQL is advisory and intentionally omitted from required readback_lanes unless this proposal is amended.
- Approval restart recovery may restore pending state but must never synthesize an approval decision.
- Future UI/native notification affordances are not part of P082 and must be covered by a separate UI proposal before implementation.
### Metrics
- **Adoption Metric:** p082_recovery_matrix_rows_with_db_engine_readback_coverage_percent
- **Operational Metrics**
  - p082_recovery_matrix_gate_result_total{scenario_id,status}
  - p082_recovery_reason_readback_total{reason_code,lane}
  - p082_recovery_mutation_rejected_total{reason_code,command}
  - p082_release_side_effect_retry_block_total{effect_status,command}
  - p082_late_output_quarantine_total{settlement,source_generation}
  - p082_recovery_idempotency_replay_total{scenario_id,result}
  - p082_recovery_state_age_seconds{scenario_id,reason_code}
### Migrations
- **Justification:** P082 adds reference documentation, tests, fixtures, shared readback accessors, and additive JSON payloads using existing durable owners. Rejected-command readback uses command_journal.error typed redacted JSON envelopes and does not mutate command_journal.payload_json. If a required field cannot be persisted unambiguously in existing storage, the proposal must be amended before adding a migration.
- **Not Applicable:** `true`
### Negative Fixtures
- **Blind Automatic Retry:** docs/evidence/rollout-contract/negative/p082-blind-automatic-retry.json
- **Cancel Late Output Mutates Active Projection:** docs/evidence/rollout-contract/negative/p082-cancel-late-output-mutates-active-projection.json
- **Duplicate Requeue Without Idempotency:** docs/evidence/rollout-contract/negative/p082-duplicate-requeue-without-idempotency.json
- **Graphql Required Without Contract:** docs/evidence/rollout-contract/negative/p082-graphql-required-without-contract.json
- **Lane Field Name Drift:** docs/evidence/rollout-contract/negative/p082-lane-field-name-drift.json
- **Malformed Command Error Envelope:** docs/evidence/rollout-contract/negative/p082-malformed-command-error-envelope.json
- **Missing Cancel Crash Rows:** docs/evidence/rollout-contract/negative/p082-missing-cancel-crash-rows.json
- **Missing Db Engine Readback Assertion:** docs/evidence/rollout-contract/negative/p082-missing-db-engine-readback-assertion.json
- **Missing Matrix Row:** docs/evidence/rollout-contract/negative/p082-missing-matrix-row.json
- **Missing Nested Subcontract:** docs/evidence/rollout-contract/negative/p082-missing-nested-subcontract.json
- **Missing Readback Reason:** docs/evidence/rollout-contract/negative/p082-missing-readback-reason.json
- **Missing Rollout Contract Operator Fields:** docs/evidence/rollout-contract/negative/p082-missing-rollout-contract-operator-fields.json
- **Missing Startup Requeue Exhausted Row:** docs/evidence/rollout-contract/negative/p082-missing-startup-requeue-exhausted-row.json
- **Rejected Command Payload Mutation:** docs/evidence/rollout-contract/negative/p082-rejected-command-payload-mutation.json
- **Release Side Effect Retry Not Fail Closed:** docs/evidence/rollout-contract/negative/p082-release-side-effect-retry-not-fail-closed.json
- **Xcode Grace Missing Operator Message:** docs/evidence/rollout-contract/negative/p082-xcode-grace-missing-operator-message.json
### Operator Report Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
- p082_recovery_matrix_readback
- p082_recovery_matrix_readbacks
### Readback Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
- p082_recovery_matrix_readback
- p082_recovery_matrix_readbacks
- **Readback Fixture:** docs/evidence/rollout-contract/operator-readback/p082-full-surface.fixture.json
### Readback Lanes
- run_report
- mcp
- release_receipt
### Rollback Disposition
- **Data Loss Risk:** none
- **Mode:** disable_p082_gate_alias_or_revert_test_matrix_only
- **Steps**
  - Remove or disable only the proposal-082\|p082 gate alias if the proof suite blocks unrelated work due to test harness defects.
  - Keep existing runtime recovery behavior unchanged unless a P082 test exposed a real regression already present in main.
  - Retain side-effect fail-closed behavior during rollback.
  - Keep additive P082 readback fields tolerant to absence during rollback.
  - Repair the matrix or fixture and rerun ./scripts/test-gate.sh proposal-082 before re-enabling the alias.
- **Schema Version:** rollout_contract_v1

## Rollout Plan

- Update the reference matrix document first, including corrected command_journal.error rejected-command ownership, legacy error fallback, lane field placement, nested subcontracts, Swift/macOS boundary, optional GraphQL tolerance, reliability edge cases, startup_requeue_exhausted, cancel-then-late-output, provider cleanup proof, long-held thresholds, and Xcode startup message requirements.
- Add positive and negative rollout fixtures before tightening the gate, especially rejected-command payload mutation, malformed command error envelope, lane field-name drift, missing nested subcontract, Xcode missing-message, missing startup_requeue_exhausted, and cancel-late-output active projection mutation negatives.
- Add proposal-082\|p082 aliases that validate matrix rows, storage owners, field schemas, exact lane names, fixture shapes, rollout contract shape, long-held thresholds, and documentation markers.
- Add DB proof for all rows, including no-mutation assertions for rejected commands, backward-compatible command_journal.error parsing, idempotency replay assertions at each durable write boundary, startup_requeue_exhausted held-state proof, and cancel-late-output terminalization proof.
- Add engine proof for startup repair, retry validation, late output quarantine, pending approval restart, duplicate ownership, cancellation interleavings, provider subprocess cleanup evidence, side-effect fail-closed retry, Xcode grace messaging, crash-loop replay, and crash-during-repair recovery.
- Add MCP/report/run-report/release diagnostic readback proof for p082_recovery_matrix_readback_v1 parity and exact singular/plural lane placement.
- Optionally add advisory GraphQL JSON parity and Swift tolerance tests only if implementation chooses to expose those app-facing paths; otherwise leave them out of the required gate and retain the documented future UI prerequisites.

## Metrics

- p082_recovery_matrix_rows_with_db_engine_readback_coverage_percent
- p082_recovery_matrix_gate_result_total{scenario_id,status}
- p082_recovery_reason_readback_total{reason_code,lane}
- p082_recovery_mutation_rejected_total{reason_code,command}
- p082_release_side_effect_retry_block_total{effect_status,command}
- p082_late_output_quarantine_total{settlement,source_generation}
- p082_recovery_idempotency_replay_total{scenario_id,result}
- p082_recovery_state_age_seconds{scenario_id,reason_code}

## Risks

1. **Item**
   - **Impact:** Violates command journal contract and makes implementation unsafe
   - **Mitigation:** Use command_journal.error p082_rejected_command_error_v1; gate has a negative fixture for payload mutation.
   - **Risk:** Rejected command readback drifts into write-once payload storage
2. **Item**
   - **Impact:** Existing runs with plain-text errors could panic or expose confusing output
   - **Mitigation:** Accessor validates envelope schema before use, falls back safely for plain text, and never displays raw envelope JSON.
   - **Risk:** Legacy command_journal.error values break the readback accessor
3. **Item**
   - **Impact:** Read model could influence scheduling or mutation semantics
   - **Mitigation:** GraphQL remains advisory, diagnostic-only, tolerant, optional, and omitted from required rollout lanes.
   - **Risk:** Optional GraphQL becomes de facto authority
4. **Item**
   - **Impact:** Unauthorized client-side recovery mutation
   - **Mitigation:** Declare app-local RecoveryCoordinator non-authoritative for P082 and require tolerance/read-only constraints for any app-facing path.
   - **Risk:** SwiftUI treats diagnostic readback as authority
5. **Item**
   - **Impact:** Duplicate work or provider sessions
   - **Mitigation:** One requeue generation, idempotent replay, startup_requeue_exhausted hold, crash-boundary tests, and crash-loop replay.
   - **Risk:** Startup requeue repeats after partial crash
6. **Item**
   - **Impact:** Old output could mutate active projection truth
   - **Mitigation:** P082-R17 proves cancelled-provider output settles as late-output quarantine with active_projection_changed=false.
   - **Risk:** Cancelled provider session emits late output
7. **Item**
   - **Impact:** Operators cannot distinguish valid waits from stuck recovery
   - **Mitigation:** p082_recovery_state_age_seconds thresholds and non-null operator messages for held states.
   - **Risk:** Long-held recovery states become invisible
8. **Item**
   - **Impact:** Recovery diagnostics could be unclear or inaccessible
   - **Mitigation:** Future UI contract prerequisites cover display names, severity, ForgeStatusColor, accessibility, text scaling, null/unavailable states, and MainActor presentation routing.
   - **Risk:** Future UI implementation lacks display discipline

## Feedback Resolution

1. **Item**
   - **Backlog Id:** SLB-082-R3-001
   - **Resolution:** Addressed by requiring Swift absent/null/additive/unknown reason-code and MainActor projection tests if any app-facing P082 path is added during implementation.
2. **Item**
   - **Backlog Id:** SLB-082-R3-002
   - **Resolution:** Addressed by adding backward-compatible p082_rejected_command_error_v1 parsing rules: validate schema before use, fall back safely for legacy plain-text command_journal.error, and never expose raw envelope JSON in operator displays.
3. **Item**
   - **Backlog Id:** SLB-082-R3-003
   - **Resolution:** Addressed by making optional GraphQL diagnostic-only and requiring tolerant absent/null/additive/unknown reason-code tests if GraphQL fields are implemented.
4. **Item**
   - **Backlog Id:** SLB-082-R3-004
   - **Resolution:** Addressed by adding P082-R16 for startup_requeue_exhausted held-state proof with non-null operator message and no duplicate generations.
5. **Item**
   - **Backlog Id:** SLB-082-R3-005
   - **Resolution:** Addressed by defining p082_recovery_state_age_seconds warning/critical thresholds for pending approvals, side-effect reconciliation holds, startup repair holds, Xcode startup grace, and startup_requeue_exhausted.
6. **Item**
   - **Backlog Id:** SLB-082-R3-006
   - **Resolution:** Addressed by strengthening R11/R15 to require provider subprocess cleanup proof tied to existing ACP transport lifecycle evidence.
7. **Item**
   - **Backlog Id:** SLB-082-R3-007
   - **Resolution:** Addressed by adding a crash-loop replay variant under R15 for repeated crashes on the same idempotency key.
8. **Item**
   - **Backlog Id:** SLB-082-R3-008
   - **Resolution:** Addressed by adding P082-R17 for cancel-then-late-output settlement with active_projection_changed=false and no mutation.
9. **Item**
   - **Backlog Id:** SLB-082-R3-009
   - **Resolution:** Addressed by carrying future macOS UI constraints for text scaling, ForgeStatusColor, accessibility labels/values, verbatim operator messages, and MainActor routing through RunsWorkbenchPresentationModel or successor.
10. **Item**
   - **Backlog Id:** SLB-082-R3-010
   - **Resolution:** Addressed by tracking UserNotifications, Dock badge, keyboard, and context-menu affordances as future UI/operator-notifications scope, not P082 implementation scope.
11. **Item**
   - **Backlog Id:** SLB-082-R3-011
   - **Resolution:** Addressed by adding a minimal future UI contract requiring reason-code display names, severity, held/cancelled treatment, Xcode grace surface, empty/null/unavailable states, recovery_next_action limits, redaction behavior, and singular/plural UI usage before any Forge screen consumes P082 readback.

## Open Questions

- Should a later schema-evolution proposal promote p082_recovery_matrix_readback_v1 from existing JSON/text owners into a dedicated typed table after the proof gate stabilizes?
- Should advisory GraphQL P082 readback be added during implementation if the projection is straightforward, or deferred to a GraphQL-focused proposal?
- Should future recovery proposals be blocked by static scan requiring matrix edits, or is proposal-082 gate plus reference documentation sufficient initially?
- Should native macOS notifications, Dock badges, and keyboard/context-menu affordances for long recovery holds be part of a future UI proposal or an operator-notifications proposal?
