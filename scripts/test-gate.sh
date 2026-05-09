#!/usr/bin/env bash
set -euo pipefail

DEFAULT_ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
if [[ -n "${CHAINWORKS_TEST_GATE_ROOT_DIR:-}" ]]; then
  ROOT_DIR="$(cd "$CHAINWORKS_TEST_GATE_ROOT_DIR" && pwd)"
else
  ROOT_DIR="$DEFAULT_ROOT_DIR"
fi
PROJECT_PATH="$ROOT_DIR/Chainworks Forge.xcodeproj"
SCHEME_NAME="Chainworks Forge"
DESTINATION="platform=macOS"
TMP_BASE="${TMPDIR:-/tmp}/chainworks-test-gates"
TEST_PLANS_DIR="$ROOT_DIR/TestPlans"
UNSIGNED_BUILD_ARGS=(
  CODE_SIGNING_ALLOWED=NO
  CODE_SIGNING_REQUIRED=NO
  CODE_SIGN_IDENTITY=
)

P077_ROLLOUT_EVIDENCE_PATH="docs/reference/p077-rollout-dependency-evidence.md"
P077_UI_EVIDENCE_PATH="docs/reference/p077-closeout-readiness-ui-evidence.md"

FAST_TESTS=(
  "Chainworks ForgeTests/ProviderPlatformTests"
  "Chainworks ForgeTests/OrchestratorTests"
  "Chainworks ForgeTests/ResumeManagerTests"
  "Chainworks ForgeTests/ArtifactManagerTests"
  "Chainworks ForgeTests/RunTests"
  "Chainworks ForgeTests/AgentSessionTests"
)

UI_SMOKE_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalInboxReachable"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testCompletedRunExportHubSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testStartRunSheetUI"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface"
)

P077_UI_TESTS=(
  "Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal077CloseoutReadinessRuntimeAccessibilityProof"
)

PROPOSAL_006_TESTS=(
  "Chainworks ForgeTests/ProviderPlatformTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface"
)

PROPOSAL_012_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRuntimeAssistantSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testWorkflowMapSurfaceShowsAfterRunStart"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AdopterSliceAccessibilityProof"
)

PROPOSAL_013_TESTS=(
  "Chainworks ForgeTests/Proposal013Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal013AppProofSurface"
)

PROPOSAL_014_TESTS=(
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal014ShellBrandHeaderVisible"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal014ForegroundBannerVisible"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testApprovalGateViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testLiveRuntimeUnavailableShowsRecoveryGuidance"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsWizardFlowSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProviderSettingsExportSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testPilotReadinessRefreshSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRuntimeAssistantSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testWorkflowMapSurfaceShowsAfterRunStart"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testReleaseGateSurfaceShowsDecisionContextActions"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AdopterSliceAccessibilityProof"
)

PROPOSAL_015_TESTS=(
  "Chainworks ForgeTests/Proposal015Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal015SkillVisibilityProofSurface"
)

PROPOSAL_015_NON_UI_TESTS=(
  "Chainworks ForgeTests/Proposal015Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
)

PROPOSAL_017_SWIFT_TESTS=(
  "Chainworks ForgeTests/Proposal017Tests"
)

PROPOSAL_018_TESTS=(
  "Chainworks ForgeTests/AgentSessionTests"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests"
)

PROPOSAL_019_TESTS=(
  "Chainworks ForgeTests/Proposal019Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests"
  "Chainworks ForgeTests/OrchestratorTests"
)

PROPOSAL_022_TESTS=(
  "Chainworks ForgeTests/Proposal022Tests"
  "Chainworks ForgeTests/Proposal022ScaffoldingTests"
)

PROPOSAL_024_TESTS=(
  "Chainworks ForgeTests/Proposal024RunSurfaceTests"
  "Chainworks ForgeTests/RunArtifactHierarchyBuilderTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal012AppendixAMinWindowOwnersAt1024x768"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testCompletedRunExportHubSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testRunProgressViewSurface"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal024FocusedTimelineInspectorSurface"
)

PROPOSAL_025_TESTS=(
  "Chainworks ForgeTests/Proposal025Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/Chainworks_ForgeTests"
)

PROPOSAL_026_TESTS=(
  "Chainworks ForgeTests/Proposal026Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests"
  "Chainworks ForgeTests/ProviderPlatformTests"
)

PROPOSAL_027_TESTS=(
  "Chainworks ForgeTests/Proposal027Tests"
)

PROPOSAL_029_TESTS=(
  "Chainworks ForgeTests/Proposal029Tests"
  "Chainworks ForgeTests/Proposal026Tests"
  "Chainworks ForgeTests/ProviderPlatformTests"
)

PROPOSAL_032_TESTS=(
  "Chainworks ForgeTests/Proposal032Tests"
  "Chainworks ForgeTests/ResumeManagerTests"
  "Chainworks ForgeTests/RecoveryCoordinatorTests"
  "Chainworks ForgeTests/WorkflowMapProjectionTests"
)

PROPOSAL_033_TESTS=(
  "Chainworks ForgeTests/Proposal033Tests"
  "Chainworks ForgeTests/RuntimeSessionBridgeTests"
  "Chainworks ForgeTests/LiveACPConnectionProofTests"
  "Chainworks ForgeTests/MVPGoldenRunTests"
  "Chainworks ForgeTests/ProviderPlatformTests"
)

PROPOSAL_037_TESTS=(
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorFailClosesACPProposalReviewReadLoopStallsBeforeWatchdogAndEmitsDurableFailureEvidence()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/acpProposalReviewerReadLoopStallFailsEarlyWithDurableFailureEvidence()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorSurfacesWatchdogFirstProgressHangsWithoutPerformingRetryLineageItself()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorFailsClosedWhenMutatingToolSuccessProducesNoFilesystemSideEffect()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterRunawayGuardrailTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterOversizedRawToolPayloadGuardrailTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterRuntimeHomeGrowthGuardrailTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesCodexACPAfterSessionHistoryTokenBudgetTrips()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorPreservesCodexACPSessionReuseScopeInsteadOfForcingNone()"
  "Chainworks ForgeTests/RuntimeAgentExecutorTests/executorRetriesSilentCodexEOFBeforeFinalResultWithAFreshSession()"
  "Chainworks ForgeTests/OrchestratorTests/sequentialWatchdogFailuresCreateDurableSameStageRetryLineageBeforeSucceeding()"
  "Chainworks ForgeTests/OrchestratorTests/downstreamStageMaterializationIsDurablyVisibleBeforeFirstAgentResult()"
  "Chainworks ForgeTests/OrchestratorTests/sequentialAgentExecutionIsDurablyVisibleBeforeFirstAgentResult()"
  "Chainworks ForgeTests/OrchestratorTests/parallelAgentExecutionsAreDurablyVisibleBeforeFirstAgentResult()"
  "Chainworks ForgeTests/OrchestratorTests/orchestratorCreatesTheCursorScheduledStageIterationInsteadOfReusingAStaleRunningStage()"
  "Chainworks ForgeTests/OrchestratorTests/implementationPartialArtifactSetRecoversFailedCodeWriterIntoContinuePath()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileImmediatelyAfterAllFanoutReviewersSettle()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceReconcilesExpiredPostFanoutSettlement()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileFreshStartedDownstreamStageBeforeFirstAgentWork()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceReconcilesTrulyStaleStartedDownstreamStageAfterExtendedGrace()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileNewlyStartedStageFromPreviousSessionClose()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceDoesNotReconcileWhileParallelStageAgentsAreStillRunning()"
  "Chainworks ForgeTests/ResumeManagerTests/executionServiceReconcilesTrulyStaleRunningAgentStageAfterExtendedGrace()"
  "Chainworks ForgeTests/RecoveryCoordinatorTests"
  "Chainworks ForgeTests/Proposal013Tests"
  "Chainworks ForgeTests/Proposal019Tests"
  "Chainworks ForgeTests/LiveProposalWorkflowTests"
  "Chainworks ForgeTests/WorkflowMapProjectionTests"
  "Chainworks ForgeTests/RunTimelineInspectorViewTests"
)

PROPOSAL_044_TESTS=(
  "test_approve_manual_gate_with_post_approval_tasks_sets_running"
  "test_approve_simple_manual_gate_settles_completed"
  "test_compile_n_phase_ordering"
  "test_post_approval_tasks_enqueued_after_approval"
  "test_end_state_with_tasks_does_not_short_circuit"
  "test_n_phase_sequence_ordering"
  "test_post_approval_retry_requires_fresh_approval"
  "test_simple_manual_gate_no_regression"
  "test_state_11_to_state_12_happy_path"
)

PROPOSAL_084_SWIFT_TESTS=(
  "Chainworks ForgeTests/Proposal084Tests"
)

PROPOSAL_085_SWIFT_TESTS=(
  "Chainworks ForgeTests/Proposal085Tests"
)

P060_PROPOSAL_REVISION_ID="P060-r16-2026-04-22"
PROPOSAL_060_CONTROL_ARTIFACT_DIR="docs/proposals/060-control-artifacts"
PROPOSAL_060_CONTROL_ARTIFACT_SPECS=(
  "proposal-060-baseline|proposal-review-baseline.v1.json|proposal_review_baseline_v1"
  "proposal-060-storage|storage-compatibility-matrix.v1.json|storage_compatibility_matrix_v1"
  "proposal-060-router-fixtures|routing-contract-fixtures.v1.json|routing_contract_fixtures_v1"
  "proposal-060-snapshot-inventory|frozen-snapshot-helper-inventory.v1.json|frozen_snapshot_helper_inventory_v1"
  "proposal-060-fixed-quartet|fixed-quartet-inventory.v1.json|hardcoded_fixed_quartet_inventory_v1"
  "proposal-060-ticket-map|implementation-ticket-map.v1.json|implementation_ticket_map_v1"
  "proposal-060-calibration|routing-calibration-report.v1.json|routing_calibration_report_v1"
)

# PROPOSAL_029_MCP_TESTS must be the *exact* inventory from P029 §9.1 — no
# elided and no added tests without a proposal amendment. Useful adjacent
# guards (mutation_name_converter_covers_command_mutations,
# mcp_tool_converter_covers_registered_tools,
# test_mcp_resource_uri_parser_maps_templates_at_server_boundary,
# principal_carries_typed_capability_sets,
# typed_filters_and_resource_match_share_principal_sets,
# test_principals_path_rejects_empty_env) are NOT listed here on purpose —
# they are kept alive via the workspace regression run at the end of this
# gate and in their owning proposal's test lanes, not here.

# PROPOSAL_042_TESTS is the authoritative focused inventory for the
# daemon lifecycle / supervision / packaging surface. Rust-side only in
# this iteration — the Swift-side client + diagnostics bundle tests from
# §10.2's "Client-side lifecycle consumption + local diagnostics" block
# depend on Xcode build-phase work (embedding the daemon binary, adding
# SMAppService integration) and will be wired in when that lands. The
# current Rust inventory covers: lifecycle types, lifecycle reporter,
# migration preflight three-branch classification + backup, PID lock
# (three-case algorithm), crash-budget (with degraded-serve entry),
# failed-serve mode, /health vs /ready status-code matrix, and packaging
# MODE dispatch + port allocation + build-sha writer.
PROPOSAL_042_TESTS=(
  # Lifecycle types (§4.1)
  "domain daemon_lifecycle_state_serializes_snake_case"
  "domain daemon_lifecycle_state_predicates"
  "domain degraded_kind_and_failure_kind_are_disjoint_type_level"
  "domain daemon_status_initial_has_no_failure"
  "domain daemon_status_failure_invariant_catches_violations"
  "domain failure_reason_backup_path_round_trips_through_json"
  "domain daemon_status_omits_empty_degraded_and_none_failure_in_json"

  # Lifecycle reporter (§5.1)
  "engine set_state_broadcasts_new_snapshot"
  "engine set_ready_populates_started_at_once"
  "engine raise_degraded_idempotent_on_same_kind"
  "engine clear_last_degraded_returns_to_ready"
  "engine set_failed_populates_failure_and_clears_degraded"
  "engine ready_clears_prior_failure_field"

  # /health vs /ready status-code matrix (§5.2 / §5.3)
  "graphql-server test_health_endpoint_returns_200_when_ready"
  "graphql-server test_health_endpoint_returns_200_in_degraded"
  "graphql-server test_health_endpoint_returns_503_only_when_starting_failed_or_shutdown"
  "graphql-server test_ready_endpoint_returns_200_only_when_ready"
  "graphql-server test_ready_endpoint_returns_503_in_degraded"
  "graphql-server test_daemon_status_failure_field_populated_only_when_failed"

  # daemonStatus GraphQL query + daemonStatusChanged subscription (§5.2)
  "graphql-server test_daemon_status_query_includes_build_sha_and_schema_versions"
  "graphql-server daemon_status_query_is_operator_only"
  "graphql-server daemon_status_query_populates_failure_field_when_failed"
  "graphql-server daemon_status_changed_subscription_receives_transitions"
  "graphql-server test_daemon_status_changed_subscription_auth_required"
  "graphql-server test_daemon_status_changed_subscription_rejects_non_operator_principal"

  # Supervisor PID lock + crash budget (§6.1 / §6.2)
  "daemon pid_lock_acquires_on_fresh_path"
  "daemon pid_lock_drop_removes_file_and_releases_flock"
  "daemon pid_lock_rejects_duplicate_live_holder"
  "daemon pid_lock_reclaims_stale_file_after_crash"
  "daemon crash_budget_absent_file_is_clean"
  "daemon crash_budget_single_crash_is_warn"
  "daemon crash_budget_five_crashes_in_60s_is_exhausted"
  "daemon crash_budget_stale_window_is_window_expired"
  "daemon record_crash_after_window_expiry_starts_new_window"
  "daemon reset_crash_budget_removes_file_idempotently"

  # Packaging mode + paths + port fallback + build-sha (§7)
  "daemon mode_from_env_var_variants"
  "daemon mode_is_packaged_predicate"
  "daemon resolve_paths_packaged_app_uses_application_support"
  "daemon enforce_loopback_rewrites_packaged_non_loopback"
  "daemon enforce_loopback_leaves_dev_mode_alone"
  "daemon bind_with_fallback_writes_daemon_port_file"
  "daemon write_build_sha_creates_file_with_content"

  # Migration preflight + backup (§8)
  "db run_preflight_missing_db_clean_installs_and_applies_all"
  "db run_preflight_zero_byte_db_clean_installs_and_applies_all"
  "db run_preflight_existing_db_without_tracker_fails_closed"
  "db run_preflight_applied_equals_binary_is_noop"
  "db run_preflight_subset_classifies_correctly_and_writes_backup"
  "db run_preflight_newer_than_binary_fails_closed"
  "db run_preflight_interleaved_divergence_fails_closed"
  "db binary_schema_version_matches_migrator_max"
  "db classify_memory_db_is_missing_or_zero_byte"
  "db classify_missing_file_is_clean_install"
  "db classify_zero_byte_file_is_clean_install"
  "db classify_populated_no_tracker_is_existing_without_tracker"
  "db classify_fully_migrated_is_tracked_equal"

  # Failed-serve mode (§8.7 / §6.2)
  "daemon test_failed_serve_health_returns_503_with_failure"
  "daemon test_failed_serve_ready_returns_503_with_failure"
  "daemon test_failed_serve_mutation_refused_with_typed_envelope"
  "daemon test_failed_serve_daemon_status_query_resolves_with_snapshot"
  "daemon test_failed_serve_non_status_query_refused_with_typed_envelope"
  "daemon test_failed_serve_listener_variant_binds_and_serves_status"

  # FailureKind round-trip (§4.1 / §8.4 AC-12)
  "daemon test_failure_migration_failed_round_trips_through_health_and_status"
  "daemon test_failure_schema_newer_than_binary_round_trips"
  "daemon test_failure_backup_failed_round_trips"
  "daemon test_failure_crash_loop_budget_exhausted_round_trips"
  "daemon test_failure_reentering_ready_clears_failure_and_preserves_started_at"

  # main.rs §8.4 migration-error → lifecycle FailureKind mapping
  "daemon map_migration_error_covers_every_variant"
  "daemon map_migration_error_lock_failed_extracts_backup_path_when_present"
  "daemon map_migration_error_lock_failed_without_backup_hint_has_none"

  # main.rs §6.3 shutdown drain protocol (DrainOutcome three-case matrix)
  "daemon shutdown_drain_completes_within_deadline_exits_zero"
  "daemon shutdown_drain_exceeds_deadline_reports_timeout"
  "daemon shutdown_serve_returning_before_signal_reports_serve_first"

  # main.rs §9.1 log sink routing (file vs stderr by mode)
  "daemon packaged_mode_with_log_path_would_install_rolling_appender"
  "daemon dev_mode_has_no_log_path_so_stderr_is_used"

  # §9.2 log redaction policy (bearer tokens + home-absolute paths)
  "daemon redact_bearer_replaces_canonical_header"
  "daemon redact_bearer_is_case_insensitive_on_bearer_literal"
  "daemon redact_bearer_keeps_non_matching_text"
  "daemon redact_message_replaces_home_with_tilde"
  "daemon redact_message_handles_bearer_and_home_together"
  "daemon redact_message_empty_home_leaves_paths_unchanged"
  "daemon redact_message_no_secrets_is_identity"
  "daemon redact_message_unicode_stays_valid"

  # §9.2 AC-14 write-time global redactor (RedactingMakeWriter).
  "daemon redacting_writer_strips_bearer_token_before_writing"
  "daemon redacting_writer_replaces_home_prefix_before_writing"
  "daemon redacting_make_writer_factory_threads_home_to_each_writer"
  "daemon redacting_writer_passes_through_binary_bytes_unchanged"

  # §9.2 R11 expanded redaction: principal_token + packaged DB path.
  "daemon redact_principal_token_query_pair"
  "daemon redact_principal_token_case_insensitive"
  "daemon redact_principal_token_without_value_still_tagged"
  "daemon redact_principal_token_leaves_prose_like_names_alone"
  "daemon redact_packaged_db_absolute_path"
  "daemon redact_packaged_db_tilde_path"
  "daemon redact_packaged_db_basename_after_scheme"
  "daemon redact_packaged_db_basename_leaves_identifier_like_prose_alone"
  "daemon redact_message_handles_bearer_principal_and_db_together"

  # §9.1 R11 log retention: 50 MB total / 7 days / 5 files caps.
  "daemon age_cap_deletes_files_older_than_seven_days"
  "daemon count_cap_keeps_newest_five"
  "daemon size_cap_drops_oldest_when_over_max_total_bytes"
  "daemon live_file_is_never_considered_for_deletion"
  "daemon sweep_is_idempotent_and_noop_on_clean_dir"
  "daemon missing_directory_returns_empty_report_without_panicking"
  "daemon unrelated_files_in_log_dir_are_ignored"

  # §9.4 R12 OPS-001 build SHA resolution: compile-time + runtime
  # fallback. Prevents the packaged daemon from reporting `dev` when
  # the Xcode embed script forgot to export GIT_SHA before cargo build.
  "daemon write_build_sha_value_roundtrips_caller_supplied_sha"
  "daemon resolved_build_sha_honors_runtime_override_and_dev_fallback"

  # §7 R13 OPS-002 packaged cwd contract: `main.rs` chdirs to $HOME
  # only in packaged modes; dev/test/mcp keep the launcher cwd.
  "daemon packaged_cwd_target_returns_home_for_packaged_modes"
  "daemon packaged_cwd_target_is_none_for_non_packaged_modes"
  "daemon packaged_cwd_target_is_none_when_home_is_unknown"

  # §8.4 R13 OPS-001: `ApplyFailed` on the tracked-subset branch
  # preserves the already-created backup path so
  # `DaemonStatus.failure.backup_path` surfaces in operator UI.
  "daemon map_migration_error_apply_failed_extracts_backup_path_when_present"
  "daemon map_migration_error_apply_failed_without_backup_hint_stays_none"

  # §8.7 R13 API-001 / API-002: failed-serve now enforces bearer auth
  # on `/graphql` and returns JSON-RPC-shaped `-32000` on `/mcp`.
  "daemon test_failed_serve_graphql_rejects_missing_authorization"
  "daemon test_failed_serve_graphql_rejects_unknown_bearer_token"
  "daemon test_failed_serve_mcp_returns_jsonrpc_error_with_request_id"
  "daemon test_failed_serve_mcp_notification_returns_202"

  # §9.3 R12 API-001 request-id in error envelopes. The "span visible
  # in logs" contract is covered by the layered `tracing` primitives
  # used in `request_id::layer` (info_span + Instrument + fmt::json)
  # and documented at the test module's tail in
  # crates/graphql-server/src/request_id.rs; no unit test fires here
  # because the `tracing` callsite cache is process-global and flakes
  # under `cargo test --workspace`.
  "mcp-server test_mcp_http_error_includes_request_id_in_error_data"
  "mcp-server test_mcp_http_parse_error_includes_request_id"

  # §9.3 R12 API-002 request-id through artifacts.override_contract.
  "mcp-server artifacts_override_contract_attaches_ambient_mcp_request_id_to_journal"

  # §9.3 request-id correlation: middleware + MCP task-local +
  # cross-surface propagation into command_journal.request_id.
  "graphql-server middleware_generates_fresh_uuid_when_header_absent"
  "graphql-server middleware_passes_through_safe_header_value"
  "graphql-server middleware_overrides_unsafe_header_with_fresh_id"
  "graphql-server is_safe_request_id_limits_length_and_charset"
  "mcp-server mcp_caller_is_unscoped_outside_request_body"
  "mcp-server mcp_caller_picks_up_scoped_request_id"
  "mcp-server scope_request_id_none_is_transparent"
  "graphql-server inbound_request_id_propagates_through_graphql_into_command_journal"
  "graphql-server missing_inbound_request_id_still_produces_and_persists_a_fresh_uuid"
  "graphql-server request_id_propagates_through_graphql_and_mcp_and_journal"
)

require_p077_rollout_dependency_evidence() {
  local evidence_file="$ROOT_DIR/$P077_ROLLOUT_EVIDENCE_PATH"
  [[ -f "$evidence_file" ]] || die "Missing P077 rollout/dependency evidence: $P077_ROLLOUT_EVIDENCE_PATH"

  local required_patterns=(
    "dependency | owner | pass_rule | proof | fallback | waiver_authority | evidence_status"
    "metric | numerator | denominator | threshold | owner | source | go_no_go_action"
    "false_ready_prevented"
    "post_release_closeout_gap_reversals"
    "false_blocks"
    "pause_to_action"
    "code_writer_loops_avoided"
    "rollback_trigger_false_blocks"
    "rollback_trigger_closeout_gap_reversal"
    "rollback_action"
    "p077_rollout_metric_events"
    "p077_rollout_decisions"
    "p077_rollout_advisory_migrations"
    "decision_type"
    "cohort"
    "eligible_closeouts"
    "primary_metric_values_json"
    "diagnostic_metric_snapshot_json"
    "dependency_checklist_snapshot_id"
    "fingerprint_p95_threshold_ms"
    "measurement_window"
    "waivers_json"
    "next_review_date"
    "readiness_links_json"
    "rollback_execution_fixture"
    "in_flight_policy"
    "neutral_observation_rule"
  )

  local pattern
  for pattern in "${required_patterns[@]}"; do
    if ! grep -Fq "$pattern" "$evidence_file"; then
      die "P077 rollout/dependency evidence is missing required field: $pattern"
    fi
  done
}

require_p077_ui_evidence() {
  local evidence_file="$ROOT_DIR/$P077_UI_EVIDENCE_PATH"
  [[ -f "$evidence_file" ]] || die "Missing P077 UI evidence: $P077_UI_EVIDENCE_PATH"

  local required_patterns=(
    "readiness_state | tone_token | icon | typography | surface | breakpoint_behavior | interaction"
    "contrast_decision"
    "measured_contrast_ratio"
    "cardElevated"
    "compactCapsule"
    "High Contrast"
    "Reduce Transparency"
    "Differentiate Without Color"
    "compactActivationAccessibilityLabel"
    "diagnosticsAccessibilityLabel"
    "copyFailureFallbackText"
    "voiceOverAnnouncementPolicy"
    "p077-closeout-readiness-announcement-priority"
    "keyboardTraversalOrder"
    "recoveryLifecycleText"
    "recoveryLifecycleAcknowledgementText"
    "recoveryLifecycleCorrelationText"
    "recoveryLifecycleFreshnessBudgetText"
    "recoveryLifecycleActionRows"
    "recoveryLifecycleCopyTemplate"
    "p077-closeout-readiness-recovery-copy-template"
    "backlinkRouteLabel"
    "p077-closeout-readiness-compact-action"
    "p077-closeout-readiness-compact-status"
    "p077-closeout-readiness-return"
    "proposal-077-ui"
  )

  local pattern
  for pattern in "${required_patterns[@]}"; do
    if ! grep -Fq "$pattern" "$evidence_file"; then
      die "P077 UI evidence is missing required field: $pattern"
    fi
  done
}

PROPOSAL_054_SWIFT_TESTS=(
  "Chainworks ForgeTests/Proposal025Tests/implementationSelfAssessmentAdapterDerivesBlockedVerificationStatus()"
  "Chainworks ForgeTests/Proposal025Tests/implementationSelfAssessmentAdapterPrefersEmbeddedCanonicalReviewSummary()"
  "Chainworks ForgeTests/Proposal025Tests/implementationSelfAssessmentAdapterPrefersRunCanonicalProjection()"
  "Chainworks ForgeTests/Proposal025Tests/implementationSelfAssessmentProjectionExposesTransitionStatusesFromCanonicalSummaries()"
  "Chainworks ForgeTests/Proposal025Tests/implementationSelfAssessmentProjectionIgnoresRawV2Artifacts()"
  "Chainworks ForgeTests/FullMVPDeliveryTests"
  "Chainworks ForgeTests/RunPlanCompilerTests"
  "Chainworks ForgeTests/TransitionEvaluatorTests"
  "Chainworks ForgeTests/OrchestratorTests"
)

PROPOSAL_061_TESTS=(
  "provider"
  "capacity"
  "start_run_closes_journal_with_run_wake_and_scheduler_refresh"
  "approve_and_reject_stage_close_journal_with_stage_mutation_and_scheduler_refresh"
  "approve_retry_cancel_p95_latency_stays_below_two_seconds_under_twenty_active_fake_agents"
  "cancel_run_closes_journal_with_cancellation_settlement_and_scheduler_refresh"
  "reset_session_closes_journal_with_repair_wake_and_scheduler_refresh"
  "startup_repair_blocks_stale_running_stage_enqueues_wake_and_scheduler_refresh"
  "retry_stage_injected_crashes_roll_back_and_startup_repair_clears_stale_running_executions"
  "invoke_agent_claim_skips_provider_at_capacity_and_claims_next_eligible_provider"
  "invoke_agent_capacity_precheck_reports_when_all_pending_work_is_blocked"
  "invoke_agent_claim_prefers_least_recently_served_run_within_candidate_window"
  "retry_stage_capacity_refresh_clears_superseded_invoke_backpressure"
  "work_queue_refresh_publishes_scheduler_backpressure_domain_event_on_transition"
  "host_interruption_records_epoch_cancels_execution_and_requeues_invoke_work"
  "host_interruption_late_output_from_superseded_attempt_cannot_promote_over_retry_generation"
  "host_interruption_requires_runtime_cleanup_before_retry_enqueue"
  "host_interruption_retry_does_not_consume_provider_quota_budget"
  "active_and_blocked_run_targets_are_preserved"
  "terminal_run_cleanup_preserves_worktree_sources_artifacts_and_databases"
  "terminal_run_cleanup_skips_unmanaged_worktree_targets"
  "stale_unreferenced_acp_home_is_removed"
)

PROPOSAL_065_TESTS=(
  "domain retry_instruction"
  "engine command_journal_redact"
  "engine proposal_065_operator_retry_instruction"
  "mcp-server tools::stages"
)

PROPOSAL_029_MCP_TESTS=(
  # Principal table bootstrap (auth/tests/principals_bootstrap.rs)
  "auth test_principals_file_created_with_owner_only_permissions"
  "auth test_principals_bootstrap_token_logged_once_on_first_start"
  "auth test_principals_daemon_refuses_empty_principals_file"

  # Transport auth — MCP HTTP (mcp-server/src/http.rs)
  "mcp-server test_mcp_http_rejects_missing_authorization_header"
  "mcp-server test_mcp_http_rejects_unknown_bearer_token"

  # Transport auth — MCP stdio (daemon/tests/mcp_stdio.rs)
  "daemon test_mcp_stdio_rejects_first_frame_other_than_initialize"
  "daemon test_mcp_stdio_rejects_initialize_without_principal_token"
  "daemon test_mcp_stdio_rejects_initialize_with_unknown_principal_token"
  "daemon test_mcp_stdio_binds_principal_for_session_lifetime"
  "daemon test_mcp_stdio_rejects_reinitialize_mid_session"

  # Transport auth — GraphQL (graphql-server/src/server.rs)
  "graphql-server test_graphql_rejects_missing_authorization_header"
  "graphql-server test_graphql_rejects_unknown_bearer_token"
  "graphql-server test_graphql_mutation_reads_principal_from_context"
  "graphql-server test_graphql_observer_class_cannot_invoke_start_run"
  "graphql-server test_graphql_ws_rejects_missing_connection_init_auth"
  "graphql-server test_graphql_ws_rejects_unknown_connection_init_token"
  "graphql-server test_graphql_ws_accepts_valid_connection_init_token"

  # MCP capability filtering (mcp-server/src/server.rs :: p029_capability_tests)
  "mcp-server test_mcp_tools_list_filtered_for_operator"
  "mcp-server test_mcp_tools_list_filtered_for_agent"
  "mcp-server test_mcp_tools_list_filtered_for_observer"
  "mcp-server test_mcp_tools_call_denied_returns_method_not_found"
  "mcp-server test_mcp_resources_list_is_capability_filtered"
  "mcp-server test_mcp_resources_read_denied_returns_not_found"

  # MCP Steward capability policy
  "mcp-server test_mcp_tools_list_includes_steward_trio_for_operator"
  "mcp-server test_mcp_tools_list_includes_steward_readers_for_observer"
  "mcp-server test_mcp_tools_list_excludes_steward_entirely_for_agent"
  "mcp-server test_mcp_tools_call_steward_run_analysis_denied_for_observer_returns_method_not_found"
  "mcp-server test_mcp_tools_call_steward_run_analysis_denied_for_agent_returns_method_not_found"
  "mcp-server test_mcp_resources_list_includes_steward_analysis_template_for_operator_and_observer"
  "mcp-server test_mcp_resources_list_excludes_steward_analysis_template_for_agent"
  "mcp-server test_mcp_resources_read_steward_analysis_denied_for_agent_returns_not_found"

  # Command journal audit rows (engine/tests/command_journal_audit.rs)
  "engine test_command_journal_row_has_caller_mcp_for_runs_start"
  "engine test_command_journal_row_has_caller_mcp_for_approvals_resolve"
  "engine test_command_journal_row_has_caller_mcp_for_steward_run_analysis"
  "engine test_command_journal_row_has_caller_graphql_for_start_run"
  "engine test_command_journal_row_has_caller_graphql_for_approve_stage"
  "engine test_command_journal_caller_columns_nullable_for_pre_p029_rows"

  # Command journal redaction matrix §8.1 (engine/src/command_journal_redact.rs)
  "engine test_redact_start_run_redacts_delivery_configuration_json"
  "engine test_redact_start_run_preserves_ids_and_paths"
  "engine test_redact_approve_stage_redacts_comment"
  "engine test_redact_approve_stage_preserves_run_and_stage_ids"
  "engine test_redact_reject_stage_redacts_comment"
  "engine test_redact_reject_stage_preserves_run_and_stage_ids"
  "engine test_redact_retry_stage_preserves_all_fields"
  "engine test_redact_cancel_run_preserves_run_id"
  "engine test_redact_reset_session_preserves_all_fields"
  "engine test_redact_run_steward_analysis_preserves_reason_and_artifact_base"
  "engine test_redaction_matrix_covers_all_command_variants"

  # journal_id surfacing on MCP (mcp-server/src/server.rs :: tests)
  "mcp-server test_mcp_tools_call_response_includes_journal_id_in_content_text"
  "mcp-server test_mcp_read_only_tool_response_omits_journal_id"
  "mcp-server test_mcp_steward_run_analysis_response_includes_journal_id"
  "mcp-server test_mcp_steward_list_analyses_response_omits_journal_id"
  "mcp-server test_mcp_steward_get_analysis_response_omits_journal_id"

  # journal_id surfacing on GraphQL (graphql-server/src/schema.rs :: tests)
  "graphql-server test_graphql_start_run_started_variant_includes_journal_id"
  "graphql-server test_graphql_start_run_blocked_variant_includes_journal_id"
  "graphql-server test_graphql_approve_stage_returns_payload_with_approval_and_journal_id"
  "graphql-server test_graphql_retry_stage_returns_payload_with_retried_and_journal_id"
  "graphql-server test_graphql_cancel_run_returns_payload_with_cancelled_and_journal_id"
  "graphql-server test_response_omits_journal_id_when_capability_check_fails"

  # GraphQL blocked-startRun payload contract §4.4.b
  "graphql-server graphql_start_run_blocked_payload_contract_tests"

  # Cross-surface parity (engine/tests/cross_surface_parity.rs)
  "engine test_graphql_and_mcp_produce_identical_run_for_start_run"

  # Dogfood .mcp.json + CLAUDE.md consistency (daemon/tests/dogfood_config.rs)
  "daemon test_dogfood_mcp_json_contains_chainworks_server_with_auth_header"
  "daemon test_dogfood_claude_md_matches_committed_mcp_json"
)

DEFAULT_REMOTE_UI_TEST_HOSTS=("SMacBook.local" "SMacBook")
LAST_BUILD_DERIVED_DATA_PATH=""

log() {
  printf '==> %s\n' "$*"
}

p041_prefixed_run() {
  local status
  set +e
  "$@" 2>&1 | while IFS= read -r line; do
    case "$line" in
      \[INFO\]*|\[PASS\]*|\[WARN\]*|\[FAIL\]*)
        printf '%s\n' "$line"
        ;;
      *)
        printf '[INFO] %s\n' "$line"
        ;;
    esac
  done
  status=${PIPESTATUS[0]}
  set -e
  return "$status"
}

p041_supervised_run() {
  local _args_json
  _args_json="$(python3 -c 'import json, sys; print(json.dumps(sys.argv[1:]))' "$@")"
  P041_SUPERVISED_ARGS_JSON="$_args_json" python3 - <<'PY'
import json
import os
import select
import signal
import subprocess
import sys
import tempfile
import time
from pathlib import Path

args = json.loads(os.environ["P041_SUPERVISED_ARGS_JSON"])
if not args:
    raise SystemExit("p041_supervised_run: missing command")

_base_dir = Path.cwd()
if (_base_dir / "control-plane").is_dir():
    _base_dir = _base_dir / "control-plane"
control_root = _base_dir / "target/parity-control"
lease_path = control_root / "lease.json"
current_step_path = control_root / "current-step.json"
interruption_marker_path = control_root / "interruption-marker.json"
timeout_marker_path = control_root / "timeout-marker.json"
publication_current_root = _base_dir / "target/parity/publication/current"

def _prefix(line: str) -> None:
    line = line.rstrip("\n")
    if line.startswith(("[INFO]", "[PASS]", "[WARN]", "[FAIL]")):
        print(line, flush=True)
    else:
        print(f"[INFO] {line}", flush=True)

def _target_boundary_is_safe(path: Path) -> bool:
    target = _base_dir / "target"
    try:
        if target.is_symlink():
            return False
        if path.is_symlink():
            return False
        anchor = target.resolve()
        resolved = path.resolve()
        return str(resolved).startswith(str(anchor))
    except OSError:
        return False

def _darwin_fullfsync(fd: int) -> bool:
    if sys.platform != "darwin":
        return False
    try:
        import ctypes
        import ctypes.util

        libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
        return libc.fcntl(fd, 51) == 0
    except Exception:
        return False

def _atomic_json(path: Path, value: dict) -> None:
    if not _target_boundary_is_safe(path):
        raise SystemExit(f"proposal-041: refusing write outside control-plane/target: {path}")
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(value, handle, indent=2)
            handle.write("\n")
            handle.flush()
            raw_fd = handle.fileno()
            if not _darwin_fullfsync(raw_fd):
                os.fsync(raw_fd)
        os.replace(tmp, path)
        try:
            dir_fd = os.open(str(path.parent), os.O_RDONLY)
            try:
                os.fsync(dir_fd)
            finally:
                os.close(dir_fd)
        except OSError:
            pass
    finally:
        try:
            if os.path.exists(tmp):
                os.unlink(tmp)
        except OSError:
            pass

def _read_json(path: Path):
    try:
        return json.loads(path.read_text())
    except (FileNotFoundError, json.JSONDecodeError, OSError):
        return None

def _write_lease_pgid(pgid: int) -> None:
    lease = _read_json(lease_path)
    if not lease:
        return
    lease["pgid"] = pgid
    lease["heartbeat_unix_ms"] = int(time.time() * 1000)
    lease["control_sequence"] = int(lease.get("control_sequence", 0)) + 1
    _atomic_json(lease_path, lease)

def _write_current_step(pgid=None) -> None:
    step = _read_json(current_step_path) or {
        "schema_version": "parity-control-current-step.v1",
        "generation": os.environ.get("P041_PUBLICATION_GENERATION_ID", ""),
        "fixture": None,
        "step": os.environ.get("P041_STEP_NAME", "supervised_command"),
        "surface": os.environ.get("P041_STEP_SURFACE") or None,
        "mode": "gate",
        "elapsed_ms": 0,
    }
    step["heartbeat_unix_ms"] = int(time.time() * 1000)
    step["command"] = args
    step["deadline_at_unix_ms"] = int(os.environ.get("P041_GATE_DEADLINE_UNIX_MS", "0") or 0)
    if pgid is not None:
        step["pgid"] = pgid
    _atomic_json(current_step_path, step)

def _write_timeout_marker(pgid, descendant_absent: bool) -> None:
    _atomic_json(timeout_marker_path, {
        "schema_version": "parity-control-timeout-marker.v1",
        "overall_status": "blocked_timeout",
        "generation_id": os.environ.get("P041_PUBLICATION_GENERATION_ID", ""),
        "active_fixture": os.environ.get("P041_STEP_FIXTURE") or None,
        "active_surface": os.environ.get("P041_STEP_SURFACE") or None,
        "descendant_pgid": pgid,
        "descendant_absent": descendant_absent,
        "written_at_unix_ms": int(time.time() * 1000),
    })

def _write_interruption_marker(pgid, descendant_absent: bool, signal_name: str) -> None:
    _atomic_json(interruption_marker_path, {
        "schema_version": "parity-control-interruption-marker.v1",
        "overall_status": "blocked_interrupted",
        "generation_id": os.environ.get("P041_PUBLICATION_GENERATION_ID", ""),
        "signal": signal_name,
        "descendant_pgid": pgid,
        "descendant_absent": descendant_absent,
        "written_at_unix_ms": int(time.time() * 1000),
    })

def _write_status_publication(status: str, reason: str) -> None:
    generation_id = os.environ.get("P041_PUBLICATION_GENERATION_ID", "")
    if not generation_id:
        return
    provenance = {
        "commit_sha": os.environ.get("P041_GIT_COMMIT_SHA", "interrupted-before-provenance"),
        "tree_id": os.environ.get("P041_GIT_TREE_ID", "interrupted-before-provenance"),
        "tree_clean": os.environ.get("P041_GIT_TREE_CLEAN") == "true",
        "status_snapshot_sha256": os.environ.get("P041_GIT_STATUS_SNAPSHOT_SHA256", "interrupted-before-provenance"),
        "status_snapshot_line_count": int(os.environ.get("P041_GIT_STATUS_SNAPSHOT_LINE_COUNT", "1") or 1),
        "generated_at": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "gate": "./scripts/test-gate.sh proposal-041",
    }
    detail = {
        "schema_version": "p031-p041-parity-evidence.v1",
        "overall_status": status,
        "publication_generation_id": generation_id,
        "publication_state": "diagnostic_blocked",
        "required_fixtures": [
            "proposal-loop-basic",
            "implementation-refine-review",
            "approval-pause-resume",
            "retry-recovery-flow",
            "cancelled-or-blocked-run",
            "terminal-report-evidence",
            "projection-readback-surface",
        ],
        "required_surfaces": [
            "canonical_domain_state",
            "projections",
            "graphql_readback",
            "mcp_report_readback",
            "artifact_identity",
            "operator_summary",
        ],
        "fixtures": [],
        "blocking_reasons": [reason],
        "missing_evidence": [],
        "provenance": provenance,
    }
    row = {
        "schema_version": "p031-phase-0-runtime-manifest-row.v1",
        "id": "p041_parity_evidence",
        "runtime_detail_path": "control-plane/target/parity/publication/current/p031-p041-parity-evidence.json",
        "reference_detail_path": "docs/reference/p031-p041-parity-evidence.json",
        "validation_status": status,
        "publication_state": "diagnostic_blocked",
        "publication_generation_id": generation_id,
        "detail_schema_version": "p031-p041-parity-evidence.v1",
        "provenance": provenance,
    }
    _atomic_json(publication_current_root / "p031-p041-parity-evidence.json", detail)
    _atomic_json(publication_current_root / "p031-phase-0-manifest-row.json", row)

global_deadline_ms = int(os.environ.get("P041_GATE_DEADLINE_UNIX_MS", "0") or 0)
command_deadline_seconds = int(os.environ.get("P041_COMMAND_DEADLINE_SECONDS", "0") or 0)
deadline_candidates = [value for value in [global_deadline_ms] if value > 0]
if command_deadline_seconds > 0:
    deadline_candidates.append(int(time.time() * 1000) + command_deadline_seconds * 1000)
deadline_ms = min(deadline_candidates) if deadline_candidates else 0
drain_seconds = int(os.environ.get("P041_DRAIN_GRACE_SECONDS", "30") or 30)
if deadline_ms and int(time.time() * 1000) >= deadline_ms:
    _write_timeout_marker(None, True)
    _write_status_publication("blocked_timeout", "gate_deadline_expired_before_command_start")
    raise SystemExit("proposal-041: gate deadline expired before starting command")

interrupted_signal = None

def _handle_signal(signum, _frame):
    global interrupted_signal
    interrupted_signal = signal.Signals(signum).name

signal.signal(signal.SIGINT, _handle_signal)
signal.signal(signal.SIGTERM, _handle_signal)

process = subprocess.Popen(
    args,
    stdout=subprocess.PIPE,
    stderr=subprocess.STDOUT,
    text=True,
    bufsize=1,
    start_new_session=True,
)
pgid = os.getpgid(process.pid)
_write_lease_pgid(pgid)
_write_current_step(pgid)

timed_out = False
try:
    assert process.stdout is not None
    while True:
        ready, _, _ = select.select([process.stdout], [], [], 0.25)
        if ready:
            line = process.stdout.readline()
            if line:
                _prefix(line)
        rc = process.poll()
        if rc is not None:
            for line in process.stdout:
                _prefix(line)
            raise SystemExit(rc)
        if deadline_ms and int(time.time() * 1000) >= deadline_ms:
            timed_out = True
            break
        if interrupted_signal is not None:
            break
except KeyboardInterrupt:
    interrupted_signal = "SIGINT"

if timed_out or interrupted_signal is not None:
    try:
        os.killpg(pgid, signal.SIGTERM)
    except ProcessLookupError:
        pass
    deadline = time.time() + drain_seconds
    while time.time() < deadline:
        if process.poll() is not None:
            break
        time.sleep(0.1)
    descendant_absent = process.poll() is not None
    if not descendant_absent:
        try:
            os.killpg(pgid, signal.SIGKILL)
        except ProcessLookupError:
            pass
        try:
            process.wait(timeout=5)
        except subprocess.TimeoutExpired:
            pass
        descendant_absent = process.poll() is not None
    if interrupted_signal is not None:
        _write_interruption_marker(pgid, descendant_absent, interrupted_signal)
        _write_status_publication("blocked_interrupted", "gate_interrupted_by_signal")
        _prefix(
            f"[WARN] proposal-041: command interrupted by {interrupted_signal}; pgid={pgid} "
            f"descendant_absent={str(descendant_absent).lower()}"
        )
        raise SystemExit(130 if interrupted_signal == "SIGINT" else 143)
    _write_timeout_marker(pgid, descendant_absent)
    _write_status_publication("blocked_timeout", "gate_or_command_deadline_expired")
    _prefix(
        f"[FAIL] proposal-041: command timed out; pgid={pgid} "
        f"descendant_absent={str(descendant_absent).lower()}"
    )
    raise SystemExit(124)
PY
}

# Refresh parity-control/lease.json heartbeat_unix_ms and increment control_sequence.
# Called between cargo phases so a live-but-stalled owner can be detected (Section 6.3
# A1/A2 freshness rule).  Reads the existing lease, bumps control_sequence, updates
# heartbeat_unix_ms, and atomically writes it back with the same durable contract.
p041_update_lease_heartbeat() {
  python3 -c "
import ctypes, ctypes.util, json, os, sys, tempfile, time
from pathlib import Path

ctrl = Path('target/parity-control')
target = Path('target')
anchor = (Path.cwd() / 'target').resolve()
if target.is_symlink():
    raise SystemExit('proposal-041: target is a symlink; refusing parity-control heartbeat write')
if ctrl.exists() and (ctrl.is_symlink() or not str(ctrl.resolve()).startswith(str(anchor))):
    raise SystemExit('proposal-041: target/parity-control boundary check failed before heartbeat write')
lease_path = ctrl / 'lease.json'
if not lease_path.exists():
    sys.exit(0)  # No lease yet; nothing to refresh.
try:
    lease = json.loads(lease_path.read_text())
except (json.JSONDecodeError, OSError):
    sys.exit(0)  # Unreadable; leave as-is; reclaim logic handles on next startup.

lease['heartbeat_unix_ms'] = int(time.time() * 1000)
lease['control_sequence'] = int(lease.get('control_sequence', 1)) + 1

fd, tmp = tempfile.mkstemp(prefix='.lease.', suffix='.tmp', dir=str(ctrl))
try:
    with os.fdopen(fd, 'w') as h:
        json.dump(lease, h, indent=2)
        h.write('\n')
        h.flush()
        raw_fd = h.fileno()
        if sys.platform == 'darwin':
            try:
                libc = ctypes.CDLL(ctypes.util.find_library('c'), use_errno=True)
                if libc.fcntl(raw_fd, 51) != 0:
                    os.fsync(raw_fd)
            except Exception:
                os.fsync(raw_fd)
        else:
            os.fsync(raw_fd)
    os.replace(tmp, str(lease_path))
    try:
        dfd = os.open(str(ctrl), os.O_RDONLY)
        try:
            os.fsync(dfd)
        finally:
            os.close(dfd)
    except OSError:
        pass
finally:
    try:
        if os.path.exists(tmp):
            os.unlink(tmp)
    except OSError:
        pass
"
}

# Write parity-control/current-step.json with the full durable atomic write
# contract (Section 6.3): same-dir tempfile, F_FULLFSYNC on Darwin, atomic
# rename, parent-dir fsync. Replaces bare os.fsync+os.replace inline blocks.
# Args: $1=step_name $2=surface_name_or_empty
p041_update_current_step() {
  local _step="$1" _surface="${2:-}"
  P041_STEP_NAME="$_step" P041_STEP_SURFACE="$_surface" python3 -c "
import ctypes, ctypes.util, json, os, sys, tempfile, time
from pathlib import Path

def _atomic_current_step(ctrl, step, surface):
    target_root = Path('target')
    anchor = (Path.cwd() / 'target').resolve()
    if target_root.is_symlink():
        raise SystemExit('proposal-041: target is a symlink; refusing current-step write')
    if ctrl.exists() and (ctrl.is_symlink() or not str(ctrl.resolve()).startswith(str(anchor))):
        raise SystemExit('proposal-041: target/parity-control boundary check failed before current-step write')
    ctrl.mkdir(parents=True, exist_ok=True)
    data = {
        'schema_version': 'parity-control-current-step.v1',
        'generation': os.environ.get('P041_PUBLICATION_GENERATION_ID', ''),
        'fixture': None,
        'step': step,
        'surface': surface if surface else None,
        'mode': 'gate',
        'elapsed_ms': 0,
        'heartbeat_unix_ms': int(time.time() * 1000),
    }
    target = ctrl / 'current-step.json'
    fd, tmp = tempfile.mkstemp(prefix='.current-step.', suffix='.tmp', dir=str(ctrl))
    try:
        with os.fdopen(fd, 'w') as h:
            json.dump(data, h, indent=2)
            h.write('\n')
            h.flush()
            raw_fd = h.fileno()
            if sys.platform == 'darwin':
                try:
                    libc = ctypes.CDLL(ctypes.util.find_library('c'), use_errno=True)
                    if libc.fcntl(raw_fd, 51) != 0:
                        os.fsync(raw_fd)
                except Exception:
                    os.fsync(raw_fd)
            else:
                os.fsync(raw_fd)
        os.replace(tmp, str(target))
        try:
            dfd = os.open(str(ctrl), os.O_RDONLY)
            try:
                os.fsync(dfd)
            finally:
                os.close(dfd)
        except OSError:
            pass
    finally:
        try:
            if os.path.exists(tmp):
                os.unlink(tmp)
        except OSError:
            pass

_atomic_current_step(
    Path('target/parity-control'),
    os.environ.get('P041_STEP_NAME', ''),
    os.environ.get('P041_STEP_SURFACE', ''),
)
"
}

should_use_unsigned_ui_tests() {
  local configured="${CHAINWORKS_USE_UNSIGNED_UI_TESTS:-}"
  if [[ -n "$configured" ]]; then
    [[ "$configured" == "1" ]]
    return
  fi

  if [[ -n "${SSH_CONNECTION:-}" ]]; then
    return 1
  fi

  return 0
}

append_xcodebuild_signing_args() {
  local gate_name="${1:-}"
  local includes_ui="${2:-0}"

  if [[ "$includes_ui" == "1" ]] && ! should_use_unsigned_ui_tests; then
    return 0
  fi

  if [[ "$gate_name" == "full" ]] && ! should_use_unsigned_ui_tests; then
    return 0
  fi

  printf '%s\0' "${UNSIGNED_BUILD_ARGS[@]}"
}

die() {
  printf 'error: %s\n' "$*" >&2
  exit 1
}

latest_crash_log() {
  ls -1t "$HOME/Library/Logs/DiagnosticReports"/Chainworks\ Forge-*.ips 2>/dev/null | head -1 || true
}

normalize_host() {
  printf '%s' "$1" | tr '[:upper:]' '[:lower:]' | sed 's/^[[:space:]]*//; s/[[:space:]]*$//'
}

approved_remote_ui_hosts() {
  if [[ -n "${CHAINWORKS_REMOTE_UI_TEST_HOSTS:-}" ]]; then
    IFS=',' read -r -a hosts <<<"$CHAINWORKS_REMOTE_UI_TEST_HOSTS"
    printf '%s\n' "${hosts[@]}"
  else
    printf '%s\n' "${DEFAULT_REMOTE_UI_TEST_HOSTS[@]}"
  fi
}

gate_requires_remote_ui_host() {
  case "${1:-}" in
    ui-smoke|proposal-006|p006|proposal-012|p012|proposal-013|p013|proposal-014|p014|proposal-015|p015|proposal-022|p022|proposal-024|p024|proposal-077-ui|p077-ui|full)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

should_wrap_gate_in_terminal_gui_session() {
  local gate_name="${1:-}"
  gate_requires_remote_ui_host "$gate_name" || return 1
  [[ -n "${SSH_CONNECTION:-}" ]] || return 1
  [[ "${CHAINWORKS_GUI_SESSION_WRAPPED:-0}" != "1" ]] || return 1
  command -v open >/dev/null 2>&1 || return 1
  return 0
}

emit_forwarded_chainworks_env() {
  local key
  local -a allowed_chainworks_env=(
    CHAINWORKS_REMOTE_UI_TEST_HOSTS
    CHAINWORKS_USE_UNSIGNED_UI_TESTS
    CHAINWORKS_GUI_GATE_TIMEOUT_SECONDS
    CHAINWORKS_CODESIGN_KEYCHAIN
    CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD
    CHAINWORKS_P013_UI_SUCCESS_GRACE_SECONDS
    CHAINWORKS_P013_UI_HARD_TIMEOUT_SECONDS
    CHAINWORKS_P015_UI_SUCCESS_GRACE_SECONDS
    CHAINWORKS_P015_UI_HARD_TIMEOUT_SECONDS
    CHAINWORKS_P022_UI_SUCCESS_GRACE_SECONDS
    CHAINWORKS_P022_UI_HARD_TIMEOUT_SECONDS
  )

  for key in "${allowed_chainworks_env[@]}"; do
    if [[ -n ${!key+x} ]]; then
      printf 'export %s=%q\n' "$key" "${!key}"
    fi
  done

  if [[ -n ${USE_TEST_PLANS+x} ]]; then
    printf 'export %s=%q\n' "USE_TEST_PLANS" "$USE_TEST_PLANS"
  fi
}

run_gate_in_terminal_gui_session() {
  local gate_name="$1"
  local stamp command_path log_path rc_path resolved_unsigned_ui_tests
  stamp="$(make_stamp)"
  command_path="$TMP_BASE/${gate_name}-${stamp}-gui.command"
  log_path="$TMP_BASE/${gate_name}-${stamp}-gui.log"
  rc_path="$TMP_BASE/${gate_name}-${stamp}-gui.rc"
  mkdir -p "$TMP_BASE"

  if should_use_unsigned_ui_tests; then
    resolved_unsigned_ui_tests=1
  else
    resolved_unsigned_ui_tests=0
  fi

  {
    printf '#!/bin/zsh\n'
    printf 'cd %q || exit 97\n' "$ROOT_DIR"
    printf 'export CHAINWORKS_GUI_SESSION_WRAPPED=1\n'
    printf 'export CHAINWORKS_USE_UNSIGNED_UI_TESTS=%q\n' "$resolved_unsigned_ui_tests"
    printf 'trap "" HUP\n'
    emit_forwarded_chainworks_env
    printf 'nohup ./scripts/test-gate.sh %q > %q 2>&1\n' "$gate_name" "$log_path"
    printf 'printf %%s \"$?\" > %q\n' "$rc_path"
  } >"$command_path"
  chmod +x "$command_path"

  log "Re-executing gate '$gate_name' in Terminal GUI session"
  open -a Terminal "$command_path" >/dev/null 2>&1

  local offset=0
  local start_epoch timeout_seconds now size rc_value
  start_epoch="$(date +%s)"
  timeout_seconds="${CHAINWORKS_GUI_GATE_TIMEOUT_SECONDS:-7200}"

  while true; do
    if [[ -f "$log_path" ]]; then
      size="$(wc -c <"$log_path" | tr -d '[:space:]')"
      if [[ -n "$size" ]] && (( size > offset )); then
        tail -c "+$((offset + 1))" "$log_path"
        offset="$size"
      fi
    fi

    if [[ -f "$rc_path" ]]; then
      rc_value="$(cat "$rc_path")"
      return "${rc_value:-1}"
    fi

    now="$(date +%s)"
    if (( now - start_epoch >= timeout_seconds )); then
      printf 'error: terminal GUI session timed out after %ss for gate %s\n' "$timeout_seconds" "$gate_name" >&2
      return 124
    fi

    sleep 2
  done
}

observed_host_names() {
  {
    hostname 2>/dev/null || true
    scutil --get LocalHostName 2>/dev/null || true
    scutil --get ComputerName 2>/dev/null || true
  } | while IFS= read -r host; do
    host="$(normalize_host "$host")"
    [[ -n "$host" ]] && printf '%s\n' "$host"
  done | awk '!seen[$0]++'
}

default_codesign_keychain() {
  local test_keychain="$HOME/Library/Keychains/test.keychain-db"
  local login_keychain="$HOME/Library/Keychains/login.keychain-db"
  if [[ -n "${CHAINWORKS_CODESIGN_KEYCHAIN:-}" ]]; then
    printf '%s\n' "$CHAINWORKS_CODESIGN_KEYCHAIN"
  elif [[ -f "$test_keychain" ]]; then
    printf '%s\n' "$test_keychain"
  else
    printf '%s\n' "$login_keychain"
  fi
}

prepare_codesign_keychain() {
  if should_use_unsigned_ui_tests; then
    return 0
  fi

  local keychain password
  local login_keychain system_keychain
  local -a search_list
  keychain="$(default_codesign_keychain)"
  password="${CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD:-}"
  login_keychain="$HOME/Library/Keychains/login.keychain-db"
  system_keychain="/Library/Keychains/System.keychain"

  [[ -f "$keychain" ]] || return 0

  search_list=("$keychain")
  if [[ -f "$login_keychain" && "$login_keychain" != "$keychain" ]]; then
    search_list+=("$login_keychain")
  fi
  if [[ -f "$system_keychain" ]]; then
    search_list+=("$system_keychain")
  fi

  security list-keychains -d user -s "${search_list[@]}" >/dev/null
  security default-keychain -d user -s "$keychain" >/dev/null

  if [[ -z "$password" ]]; then
    security show-keychain-info "$keychain" >/dev/null 2>&1 || \
      die "codesign keychain is locked: $keychain. Set CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD for remote UI gates."
    return 0
  fi

  log "Unlocking codesign keychain: $keychain"
  security unlock-keychain -p "$password" "$keychain"
  security set-keychain-settings -lut 21600 "$keychain"
  security set-key-partition-list -S apple-tool:,apple: -s -k "$password" "$keychain" >/dev/null
}

require_remote_ui_host() {
  local approved observed host
  approved=()
  while IFS= read -r host; do
    approved+=("$host")
  done < <(
    approved_remote_ui_hosts \
      | while IFS= read -r host; do
          printf '%s\n' "$(normalize_host "$host")"
        done \
      | awk '!seen[$0]++'
  )

  observed=()
  while IFS= read -r host; do
    observed+=("$host")
  done < <(observed_host_names)

  for host in "${observed[@]}"; do
    local allowed
    for allowed in "${approved[@]}"; do
      if [[ "$host" == "$allowed" ]]; then
        return 0
      fi
    done
  done

  printf 'error: UI tests are remote-only and may not run on this host.\n' >&2
  printf 'approved remote hosts: %s\n' "$(IFS=', '; printf '%s' "${approved[*]}")" >&2
  printf 'observed host names: %s\n' "$(IFS=', '; printf '%s' "${observed[*]}")" >&2
  exit 3
}

check_idle_environment() {
  local mode="${1:-strict}"
  local matches
  matches="$(
    {
      ps -axo pid=,comm=,args= \
        | awk -v mode="$mode" '
            {
              pid = $1
              comm = $2
              $1 = ""
              $2 = ""
              sub(/^[[:space:]]+/, "", $0)
              args = $0

              if (comm ~ /^(xcodebuild|xctest|XCTest|debugserver)$/) {
                print pid " " args
                next
              }

              if (mode == "strict" && args ~ /Chainworks Forge\.app\/Contents\/MacOS\/Chainworks Forge/) {
                print pid " " args
              }
            }
          '
    } || true
  )"
  if [[ -n "$matches" ]]; then
    printf 'Refusing to start gate while test/app processes are already running:\n%s\n' "$matches" >&2
    exit 2
  fi
}

guard_direct_run_insertion() {
  log "Guard: no direct Run construction outside RunRepository"
  python3 - "$ROOT_DIR/Chainworks Forge" <<'PY'
from pathlib import Path
import re
import sys

root = Path(sys.argv[1])
pattern = re.compile(r"(?<![A-Za-z0-9_])Run\s*\(")
block_comments = re.compile(r"/\*.*?\*/", re.S)
string_literals = re.compile(r'"(?:\\.|[^"\\])*"')
exempt = {"RunRepository.swift", "Run.swift"}
violations = []

for file in root.rglob("*.swift"):
    if file.name in exempt:
        continue
    content = file.read_text(encoding="utf-8")
    has_exemption_marker = "// RunRepository-exempt" in content
    content = block_comments.sub("", content)
    sanitized_lines = []
    for line in content.splitlines():
        stripped = line.lstrip()
        if stripped.startswith("//"):
            continue
        sanitized_lines.append(string_literals.sub('""', line))
    sanitized = "\n".join(sanitized_lines)
    if (
        pattern.search(sanitized)
        and "RunStatus" not in sanitized
        and "RunRepositoryError" not in sanitized
        and not has_exemption_marker
    ):
        violations.append(str(file.relative_to(root.parent)))

if violations:
    print("Direct Run construction found outside RunRepository:", file=sys.stderr)
    for violation in violations:
        print(violation, file=sys.stderr)
    sys.exit(1)
PY
}

guard_portability_paths() {
  log "Guard: portability-sensitive sources avoid hardcoded user paths"
  python3 - "$ROOT_DIR/Chainworks Forge" "$ROOT_DIR/Chainworks ForgeTests" <<'PY'
from pathlib import Path
import sys

app_root = Path(sys.argv[1])
test_root = Path(sys.argv[2])
violations = []

sensitive_files = [
    app_root / "Support/PreviewSupport.swift",
    app_root / "Views/DeliveryPreflightReportView.swift",
    app_root / "Views/ReleaseGateView.swift",
    app_root / "Views/IdeaListView.swift",
    test_root / "Chainworks_ForgeTests.swift",
    test_root / "RuntimeSessionBridgeTests.swift",
]

for f in sensitive_files:
    if not f.exists():
        continue
    content = f.read_text(encoding="utf-8")
    if "/Users/user/" in content:
        violations.append(f"{f.name}: contains hardcoded /Users/user/ path")

cwd_sensitive_files = [
    app_root / "Chainworks_ForgeApp.swift",
    app_root / "Engine/SampleRunLauncher.swift",
]
forbidden_fragments = [
    "repoRoot: FileManager.default.currentDirectoryPath",
    "run.repoRoot = FileManager.default.currentDirectoryPath",
    "workspaceRootPath: FileManager.default.currentDirectoryPath",
]

for f in cwd_sensitive_files:
    if not f.exists():
        continue
    content = f.read_text(encoding="utf-8")
    for frag in forbidden_fragments:
        if frag in content:
            violations.append(f"{f.name}: derives repo truth from cwd via: {frag}")

if violations:
    print("Portability violations:", file=sys.stderr)
    for v in violations:
        print(f"  {v}", file=sys.stderr)
    sys.exit(1)
PY
}

guard_plan_tag_sync() {
  log "Guard: test-plan selectedTests match Swift Testing tags"
  python3 - "$ROOT_DIR" <<'PY'
"""Verify that .xctestplan selectedTests lists stay in sync with Swift Testing tags.

Xcode test plans do not natively support Swift Testing Tag-based filtering
(as of Xcode 26 / Swift 6). The project uses selectedTests as the
bridging mechanism; this guardrail ensures the lists track the actual
@Tag declarations in source so tags remain the single source of truth.
"""
from pathlib import Path
import json
import re
import sys

root = Path(sys.argv[1])
test_dir = root / "Chainworks ForgeTests"
plans_dir = root / "TestPlans"

# ── Scan source for tagged suites ──────────────────────────────────
# Matches:  @Suite("...", .tags(.fast))  or  @Suite("...", .serialized, .tags(.fast, .provider))
suite_re = re.compile(r"@Suite\([^)]*\)")
tag_re = re.compile(r"\.tags\(([^)]+)\)")
struct_re = re.compile(r"struct\s+(\w+)")

tag_to_suites: dict[str, set[str]] = {}

for swift_file in sorted(test_dir.glob("*.swift")):
    content = swift_file.read_text(encoding="utf-8")
    lines = content.splitlines()
    for i, line in enumerate(lines):
        m_suite = suite_re.search(line)
        if not m_suite:
            continue
        m_tags = tag_re.search(m_suite.group())
        if not m_tags:
            continue
        tags = [t.strip().lstrip(".") for t in m_tags.group(1).split(",")]
        # Find the struct name on this line or the next few lines
        struct_name = None
        for j in range(i, min(i + 4, len(lines))):
            m_struct = struct_re.search(lines[j])
            if m_struct:
                struct_name = m_struct.group(1)
                break
        if struct_name:
            for tag in tags:
                tag_to_suites.setdefault(tag, set()).add(struct_name)

# ── Verify each plan ──────────────────────────────────────────────
plan_tag_map = {
    "FastGate.xctestplan": "fast",
    "ProviderGate.xctestplan": "provider",
}

errors = []
for plan_name, expected_tag in plan_tag_map.items():
    plan_path = plans_dir / plan_name
    if not plan_path.exists():
        errors.append(f"{plan_name}: file not found")
        continue

    plan = json.loads(plan_path.read_text(encoding="utf-8"))
    plan_suites: set[str] = set()
    for target in plan.get("testTargets", []):
        for entry in target.get("selectedTests", []):
            # Entries may be "SuiteName" or "SuiteName/method()"
            plan_suites.add(entry.split("/")[0])

    expected_suites = tag_to_suites.get(expected_tag, set())

    missing_from_plan = expected_suites - plan_suites
    extra_in_plan = plan_suites - expected_suites

    if missing_from_plan:
        errors.append(
            f"{plan_name}: tagged .{expected_tag} in source but missing from selectedTests: "
            + ", ".join(sorted(missing_from_plan))
        )
    if extra_in_plan:
        errors.append(
            f"{plan_name}: in selectedTests but NOT tagged .{expected_tag} in source: "
            + ", ".join(sorted(extra_in_plan))
        )

if errors:
    print("Test-plan / tag sync violations:", file=sys.stderr)
    for e in errors:
        print(f"  • {e}", file=sys.stderr)
    sys.exit(1)
PY
}

proposal060_canonical_gate_name() {
  case "${1:-}" in
    proposal-060|p060) printf '%s\n' "proposal-060" ;;
    proposal-060-baseline|p060-baseline) printf '%s\n' "proposal-060-baseline" ;;
    proposal-060-storage|p060-storage) printf '%s\n' "proposal-060-storage" ;;
    proposal-060-router-fixtures|p060-router-fixtures) printf '%s\n' "proposal-060-router-fixtures" ;;
    proposal-060-snapshot-inventory|p060-snapshot-inventory) printf '%s\n' "proposal-060-snapshot-inventory" ;;
    proposal-060-fixed-quartet|p060-fixed-quartet) printf '%s\n' "proposal-060-fixed-quartet" ;;
    proposal-060-ticket-map|p060-ticket-map) printf '%s\n' "proposal-060-ticket-map" ;;
    proposal-060-calibration|p060-calibration) printf '%s\n' "proposal-060-calibration" ;;
    *) return 1 ;;
  esac
}

proposal060_control_artifact_spec_for_gate() {
  local canonical_gate="$1"
  local spec gate
  for spec in "${PROPOSAL_060_CONTROL_ARTIFACT_SPECS[@]}"; do
    gate="${spec%%|*}"
    if [[ "$gate" == "$canonical_gate" ]]; then
      printf '%s\n' "$spec"
      return 0
    fi
  done
  return 1
}

run_proposal060_control_artifact_gate() {
  local gate_name canonical_gate spec filename expected_schema artifact_path filename_and_schema
  gate_name="$1"
  canonical_gate="$(proposal060_canonical_gate_name "$gate_name")" || die "unknown Proposal 060 gate: $gate_name"
  spec="$(proposal060_control_artifact_spec_for_gate "$canonical_gate")" || die "no Proposal 060 artifact spec for gate: $canonical_gate"

  filename_and_schema="${spec#*|}"
  filename="${filename_and_schema%%|*}"
  expected_schema="${filename_and_schema#*|}"
  artifact_path="$ROOT_DIR/$PROPOSAL_060_CONTROL_ARTIFACT_DIR/$filename"

  if [[ ! -f "$artifact_path" ]]; then
    die "$canonical_gate: missing control artifact $artifact_path"
  fi

  run_proposal060_validate_control_artifact_gate "$canonical_gate"
}

run_proposal060_validate_control_artifact_gate() {
  local gate_name canonical_gate spec filename expected_schema artifact_path validator_path filename_and_schema
  gate_name="$1"
  canonical_gate="$(proposal060_canonical_gate_name "$gate_name")" || die "unknown Proposal 060 gate: $gate_name"
  spec="$(proposal060_control_artifact_spec_for_gate "$canonical_gate")" || die "no Proposal 060 artifact spec for gate: $canonical_gate"

  filename_and_schema="${spec#*|}"
  filename="${filename_and_schema%%|*}"
  expected_schema="${filename_and_schema#*|}"
  artifact_path="$ROOT_DIR/$PROPOSAL_060_CONTROL_ARTIFACT_DIR/$filename"
  validator_path="$DEFAULT_ROOT_DIR/scripts/proposal060_control_artifact_gate.py"

  log "Proposal 060 control artifact gate: $canonical_gate"
  if [[ ! -f "$artifact_path" ]]; then
    die "$canonical_gate: missing control artifact $artifact_path"
  fi
  if [[ ! -f "$validator_path" ]]; then
    die "Proposal 060 validator helper missing: $validator_path"
  fi

  python3 "$validator_path" "$artifact_path" "$expected_schema" "$P060_PROPOSAL_REVISION_ID" "$canonical_gate"
  log "Proposal 060 control artifact gate passed: $canonical_gate"
}

run_proposal060_all_control_artifacts() {
  local spec gate
  log "Proposal 060 wrapper gate: Phase 0a/0b control artifacts"
  for spec in "${PROPOSAL_060_CONTROL_ARTIFACT_SPECS[@]}"; do
    gate="${spec%%|*}"
    run_proposal060_control_artifact_gate "$gate"
  done
  log "Proposal 060 wrapper gate passed"
}

make_stamp() {
  date +"%Y%m%d-%H%M%S"
}

run_build() {
  local gate_name="$1"
  local stamp derived_data
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  LAST_BUILD_DERIVED_DATA_PATH="$derived_data"
  mkdir -p "$TMP_BASE"
  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "$gate_name" "0")
  log "Build gate: $gate_name"
  xcodebuild \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -derivedDataPath "$derived_data" \
    ${signing_args[@]+"${signing_args[@]}"} \
    build
}

run_proposal022_app_proof() {
  local derived_data="$1"
  local stamp app_binary result_path log_path timeout_seconds pid app_status
  stamp="$(make_stamp)"
  app_binary="$derived_data/Build/Products/Debug/Chainworks Forge.app/Contents/MacOS/Chainworks Forge"
  result_path="$TMP_BASE/proposal-022-app-proof-${stamp}.json"
  log_path="$TMP_BASE/proposal-022-app-proof-${stamp}.log"
  timeout_seconds="${CHAINWORKS_P022_APP_PROOF_TIMEOUT_SECONDS:-90}"

  [[ -x "$app_binary" ]] || die "Proposal 022 app proof binary not found: $app_binary"

  log "App proof gate: proposal-022"
  rm -f "$result_path" "$log_path"

  env \
    CHAINWORKS_IN_MEMORY_STORE=1 \
    CHAINWORKS_FIXTURE_MODE=proposal022_feedback_cycle \
    CHAINWORKS_P022_APP_PROOF_AUTORUN=1 \
    CHAINWORKS_P022_APP_PROOF_RESULT_PATH="$result_path" \
    "$app_binary" >"$log_path" 2>&1 &
  pid=$!

  local deadline=$((SECONDS + timeout_seconds))
  while kill -0 "$pid" 2>/dev/null; do
    if (( SECONDS >= deadline )); then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
      printf 'Proposal 022 app proof timed out after %s seconds.\n' "$timeout_seconds" >&2
      if [[ -f "$log_path" ]]; then
        printf '--- app proof log ---\n' >&2
        cat "$log_path" >&2
      fi
      exit 1
    fi
    sleep 1
  done

  wait "$pid"
  app_status=$?
  if [[ $app_status -ne 0 ]]; then
    printf 'Proposal 022 app proof process exited with status %s.\n' "$app_status" >&2
    if [[ -f "$log_path" ]]; then
      printf '--- app proof log ---\n' >&2
      cat "$log_path" >&2
    fi
    exit 1
  fi

  [[ -f "$result_path" ]] || {
    printf 'Proposal 022 app proof did not produce result JSON at %s.\n' "$result_path" >&2
    if [[ -f "$log_path" ]]; then
      printf '--- app proof log ---\n' >&2
      cat "$log_path" >&2
    fi
    exit 1
  }

  python3 - "$result_path" <<'PY'
import json
import sys
from pathlib import Path

result_path = Path(sys.argv[1])
payload = json.loads(result_path.read_text(encoding="utf-8"))
result = payload.get("result") or {}
summary = payload.get("summary") or {}

checks = [
    (result.get("refineCorpusInputCount") == 5, "refine corpus count must be 5"),
    (result.get("reviewCorpusBundleExists") is True, "review corpus bundle must exist"),
    (result.get("reviewCorpusBundleConsumed") is True, "review corpus bundle must be consumed"),
    (result.get("scoreLiftBacklogExists") is True, "score lift backlog must exist"),
    (result.get("scoreLiftBacklogMergeProvenanceExists") is True, "merge provenance must exist"),
    (result.get("proposalFeedbackCoverageExists") is True, "proposal feedback coverage must exist"),
    (bool(result.get("unresolvedBacklogItemIDs")), "unresolved backlog items must remain visible"),
    (bool((result.get("targetedRerunRationale") or "").strip()), "targeted rerun rationale must be present"),
    ("PASS" in (result.get("proofStatus") or ""), "proof status must be PASS"),
    (summary.get("reviewCorpusBundlePresent") is True, "summary must surface review corpus bundle"),
    ((summary.get("mergeProvenanceItemCount") or 0) > 0, "summary must surface merge provenance"),
]

failed = [message for ok, message in checks if not ok]
if failed:
    print("Proposal 022 app proof validation failed:", file=sys.stderr)
    for message in failed:
        print(f"  - {message}", file=sys.stderr)
    sys.exit(1)

print(f"Proposal 022 app proof result: {result_path}")
PY
}

run_proposal015_app_proof() {
  local derived_data="$1"
  local stamp app_binary result_path log_path timeout_seconds pid app_status
  stamp="$(make_stamp)"
  app_binary="$derived_data/Build/Products/Debug/Chainworks Forge.app/Contents/MacOS/Chainworks Forge"
  result_path="$TMP_BASE/proposal-015-app-proof-${stamp}.json"
  log_path="$TMP_BASE/proposal-015-app-proof-${stamp}.log"
  timeout_seconds="${CHAINWORKS_P015_APP_PROOF_TIMEOUT_SECONDS:-90}"

  [[ -x "$app_binary" ]] || die "Proposal 015 app proof binary not found: $app_binary"

  log "App proof gate: proposal-015"
  rm -f "$result_path" "$log_path"

  env \
    CHAINWORKS_IN_MEMORY_STORE=1 \
    CHAINWORKS_P015_APP_PROOF_AUTORUN=1 \
    CHAINWORKS_P015_APP_PROOF_RESULT_PATH="$result_path" \
    "$app_binary" >"$log_path" 2>&1 &
  pid=$!

  local deadline=$((SECONDS + timeout_seconds))
  while kill -0 "$pid" 2>/dev/null; do
    if (( SECONDS >= deadline )); then
      kill "$pid" 2>/dev/null || true
      wait "$pid" 2>/dev/null || true
      printf 'Proposal 015 app proof timed out after %s seconds.\n' "$timeout_seconds" >&2
      if [[ -f "$log_path" ]]; then
        printf '%s\n' '--- app proof log ---' >&2
        cat "$log_path" >&2
      fi
      exit 1
    fi
    sleep 1
  done

  wait "$pid"
  app_status=$?
  if [[ $app_status -ne 0 ]]; then
    printf 'Proposal 015 app proof process exited with status %s.\n' "$app_status" >&2
    if [[ -f "$log_path" ]]; then
      printf '%s\n' '--- app proof log ---' >&2
      cat "$log_path" >&2
    fi
    exit 1
  fi

  [[ -f "$result_path" ]] || {
    printf 'Proposal 015 app proof did not produce result JSON at %s.\n' "$result_path" >&2
    if [[ -f "$log_path" ]]; then
      printf '%s\n' '--- app proof log ---' >&2
      cat "$log_path" >&2
    fi
    exit 1
  }

  python3 - "$result_path" <<'PY'
import json
import sys
from pathlib import Path

result_path = Path(sys.argv[1])
payload = json.loads(result_path.read_text(encoding="utf-8"))
result = payload.get("result") or {}

checks = [
    (result.get("proofAgentID") == "proposal_reviewer_product_owner", "proof agent id must be proposal_reviewer_product_owner"),
    (result.get("reportSkillRef") == "proposal_review_triad", "report skill ref must be proposal_review_triad"),
    (result.get("reportSkillRole") == "product_owner", "report skill role must be product_owner"),
    (result.get("comparisonSkillRole") == "architect", "comparison skill role must be architect"),
    (result.get("primaryArtifactName") == "proposal_current", "primary artifact must be proposal_current"),
    (result.get("primaryArtifactExists") is True, "primary artifact must exist on disk"),
    (result.get("summaryMentionsSkillTruth") is True, "summary must mention skill truth"),
    (result.get("injectedSkillHashPresent") is True, "injected skill hash must be present"),
    ("PASS" in (result.get("proofStatus") or ""), "proof status must be PASS"),
]

failed = [message for ok, message in checks if not ok]
if failed:
    print("Proposal 015 app proof validation failed:", file=sys.stderr)
    for message in failed:
        print(f"  - {message}", file=sys.stderr)
    sys.exit(1)

print(f"Proposal 015 app proof result: {result_path}")
PY
}

run_test_plan() {
  local gate_name="$1"
  local plan_name="$2"

  local stamp derived_data result_bundle
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/${gate_name}-${stamp}.xcresult"
  mkdir -p "$TMP_BASE"
  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "$gate_name" "1")

  log "Test gate (test plan): $gate_name — plan=$plan_name"
  xcodebuild test \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -testPlan "$plan_name" \
    -parallel-testing-enabled NO \
    -maximum-parallel-testing-workers 1 \
    -derivedDataPath "$derived_data" \
    -resultBundlePath "$result_bundle" \
    ${signing_args[@]+"${signing_args[@]}"}
  log "Result bundle: $result_bundle"
}

run_targeted_tests() {
  local gate_name="$1"
  shift

  local stamp derived_data result_bundle log_path automation_log_path previous_automation_log_path
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/${gate_name}-${stamp}.xcresult"
  log_path="$TMP_BASE/${gate_name}-${stamp}.log"
  mkdir -p "$TMP_BASE"

  local cmd=(
    xcodebuild
    test
    -project "$PROJECT_PATH"
    -scheme "$SCHEME_NAME"
    -destination "$DESTINATION"
    -parallel-testing-enabled NO
    -maximum-parallel-testing-workers 1
    -derivedDataPath "$derived_data"
  )

  local includes_ui=0

  local test_id
  for test_id in "$@"; do
    cmd+=("-only-testing:$test_id")
    if [[ "$test_id" == Chainworks\ ForgeUITests/* ]]; then
      includes_ui=1
    fi
  done

  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "$gate_name" "$includes_ui")

  if [[ $includes_ui -eq 0 ]]; then
    cmd+=("-resultBundlePath" "$result_bundle")
    cmd+=(${signing_args[@]+"${signing_args[@]}"})
    cmd+=("-skip-testing:Chainworks ForgeUITests")
  else
    automation_log_path="$TMP_BASE/${gate_name}-${stamp}-automation.log"
    previous_automation_log_path="${CHAINWORKS_UI_AUTOMATION_LOG_PATH:-}"
    export CHAINWORKS_UI_AUTOMATION_LOG_PATH="$automation_log_path"
    cmd+=(${signing_args[@]+"${signing_args[@]}"})
    if [[ "$gate_name" != "proposal-013-ui" ]]; then
      cmd+=("-resultBundlePath" "$result_bundle")
    fi
  fi

  log "Test gate: $gate_name"
  if [[ "$gate_name" == "proposal-013-ui" || "$gate_name" == "proposal-015-ui" || "$gate_name" == "proposal-022-ui" ]]; then
    # This lane currently hangs on the approved host after it has already
    # printed a successful XCTest summary. Run it through a narrow watchdog
    # that only accepts success after the canonical pass markers.
    python3 - "$gate_name" "$log_path" "${cmd[@]}" <<'PY'
import os
import select
import signal
import subprocess
import sys
import time
from pathlib import Path

gate_name = sys.argv[1]
log_path = sys.argv[2]
cmd = sys.argv[3:]
automation_log_path = Path(os.environ.get("CHAINWORKS_UI_AUTOMATION_LOG_PATH", "/tmp/chainworks-ui-automation.log"))

def dump_automation_log():
    if not automation_log_path.exists():
        return
    try:
        lines = automation_log_path.read_text(encoding="utf-8", errors="replace").splitlines()
    except Exception as exc:
        print(f"warning: failed to read UI automation log {automation_log_path}: {exc}", file=sys.stderr)
        return

    tail = lines[-80:]
    if not tail:
        return
    print(f"--- UI automation log tail: {automation_log_path} ---", file=sys.stderr)
    for line in tail:
        print(line, file=sys.stderr)

if gate_name == "proposal-013-ui":
    marker_test_passed = "Test Case '-[Chainworks_ForgeUITests.Chainworks_ForgeUITests testProposal013AppProofSurface]' passed"
    suite_markers = ("Executed 1 test, with 0 failures", "** TEST SUCCEEDED **")
    success_label = "Proposal 013 UI watchdog"
    grace_seconds = float(os.environ.get("CHAINWORKS_P013_UI_SUCCESS_GRACE_SECONDS", "15"))
    hard_timeout_seconds = float(os.environ.get("CHAINWORKS_P013_UI_HARD_TIMEOUT_SECONDS", "1800"))
elif gate_name == "proposal-022-ui":
    marker_test_passed = "Test Case '-[Chainworks_ForgeUITests.Chainworks_ForgeUITests testProposal022AppProofSurface]' passed"
    suite_markers = ("Executed 1 test, with 0 failures", "** TEST SUCCEEDED **")
    success_label = "Proposal 022 gate watchdog"
    grace_seconds = float(os.environ.get("CHAINWORKS_P022_UI_SUCCESS_GRACE_SECONDS", "10"))
    hard_timeout_seconds = float(os.environ.get("CHAINWORKS_P022_UI_HARD_TIMEOUT_SECONDS", "1800"))
elif gate_name == "proposal-015-ui":
    marker_test_passed = "Test Case '-[Chainworks_ForgeUITests.Chainworks_ForgeUITests testProposal015SkillVisibilityProofSurface]' passed"
    suite_markers = ("Executed 1 test, with 0 failures", "** TEST SUCCEEDED **")
    success_label = "Proposal 015 UI watchdog"
    grace_seconds = float(os.environ.get("CHAINWORKS_P015_UI_SUCCESS_GRACE_SECONDS", "15"))
    hard_timeout_seconds = float(os.environ.get("CHAINWORKS_P015_UI_HARD_TIMEOUT_SECONDS", "1800"))
else:
    raise SystemExit(f"unsupported watchdog gate: {gate_name}")

test_passed = False
suite_passed = False
success_at = None
start = time.time()
known_failure_markers = (
    "before establishing connection",
    "Early unexpected exit",
    "signal kill",
)

with open(log_path, "w", encoding="utf-8") as log:
    proc = subprocess.Popen(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
        bufsize=1,
        start_new_session=True,
    )
    try:
        while True:
            ready, _, _ = select.select([proc.stdout], [], [], 0.5)
            line = proc.stdout.readline() if ready else ""
            if line:
                sys.stdout.write(line)
                sys.stdout.flush()
                log.write(line)
                log.flush()
                if marker_test_passed in line:
                    test_passed = True
                if any(marker in line for marker in suite_markers):
                    suite_passed = True
                if any(marker in line for marker in known_failure_markers):
                    print(f"error: {success_label} saw known launch failure marker", file=sys.stderr)
                    dump_automation_log()
                    try:
                        os.killpg(proc.pid, signal.SIGTERM)
                    except ProcessLookupError:
                        pass
                    time.sleep(2)
                    if proc.poll() is None:
                        try:
                            os.killpg(proc.pid, signal.SIGKILL)
                        except ProcessLookupError:
                            pass
                    raise SystemExit(65)
                if test_passed and suite_passed and success_at is None:
                    success_at = time.time()
                continue

            if proc.poll() is not None:
                if proc.returncode != 0:
                    dump_automation_log()
                raise SystemExit(proc.returncode)

            now = time.time()
            if success_at is not None and now - success_at >= grace_seconds:
                try:
                    os.killpg(proc.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                time.sleep(2)
                if proc.poll() is None:
                    try:
                        os.killpg(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                print(f"==> {success_label}: xcodebuild hung after successful proof; terminating stale process and accepting gate")
                raise SystemExit(0)

            if now - start >= hard_timeout_seconds:
                try:
                    os.killpg(proc.pid, signal.SIGTERM)
                except ProcessLookupError:
                    pass
                time.sleep(2)
                if proc.poll() is None:
                    try:
                        os.killpg(proc.pid, signal.SIGKILL)
                    except ProcessLookupError:
                        pass
                dump_automation_log()
                print(f"error: {success_label} hit hard timeout before success markers", file=sys.stderr)
                raise SystemExit(124)

    finally:
        try:
            proc.stdout.close()
        except Exception:
            pass
PY
    log "Watchdog log: $log_path"
    local derived_result
    derived_result="$(find "$derived_data/Logs/Test" -name '*.xcresult' -print 2>/dev/null | sort | tail -1 || true)"
    if [[ -n "$derived_result" ]]; then
      log "Result bundle: $derived_result"
    fi
  else
    "${cmd[@]}"
    log "Result bundle: $result_bundle"
  fi

  if [[ $includes_ui -eq 1 ]]; then
    if [[ -n "$previous_automation_log_path" ]]; then
      export CHAINWORKS_UI_AUTOMATION_LOG_PATH="$previous_automation_log_path"
    else
      unset CHAINWORKS_UI_AUTOMATION_LOG_PATH
    fi
  fi
}

run_split_targeted_gate() {
  local gate_name="$1"
  shift

  local non_ui_tests=()
  local ui_tests=()
  local test_id
  for test_id in "$@"; do
    if [[ "$test_id" == Chainworks\ ForgeUITests/* ]]; then
      ui_tests+=("$test_id")
    else
      non_ui_tests+=("$test_id")
    fi
  done

  if [[ ${#non_ui_tests[@]} -gt 0 ]]; then
    run_targeted_tests "${gate_name}-non-ui" "${non_ui_tests[@]}"
  fi

  if [[ ${#ui_tests[@]} -gt 0 ]]; then
    run_targeted_tests "${gate_name}-ui" "${ui_tests[@]}"
  fi
}

run_full_suite() {
  local stamp derived_data result_bundle
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/full-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/full-${stamp}.xcresult"
  mkdir -p "$TMP_BASE"
  while IFS= read -r -d '' arg; do
    signing_args+=("$arg")
  done < <(append_xcodebuild_signing_args "full" "1")

  log "Full gate: xcodebuild test"
  xcodebuild \
    test \
    -project "$PROJECT_PATH" \
    -scheme "$SCHEME_NAME" \
    -destination "$DESTINATION" \
    -parallel-testing-enabled NO \
    -maximum-parallel-testing-workers 1 \
    -derivedDataPath "$derived_data" \
    -resultBundlePath "$result_bundle" \
    ${signing_args[@]+"${signing_args[@]}"}
  log "Result bundle: $result_bundle"
}

print_usage() {
  cat <<'EOF'
Usage: ./scripts/test-gate.sh <gate>

Available gates:
  list            Show available gates
  guardrails      Run cheap source-tree guardrails only
  build           Build the app only
  fast            Guardrails + build + high-ROI unit/runtime tests
  ui-smoke        Focused operator-shell UI smoke tests
  proposal-006    Proposal 006 settings/provider/readiness gate
  proposal-013    Proposal 013 contract/evidence/recovery gate
  proposal-014    Proposal 014 design-system and brand adoption gate
  proposal-015    Proposal 015 skill resolution and runtime injection gate
  proposal-017    Workflow authority, conflict truth, and lead mediation gate (retained alias)
  proposal-018    Proposal 018 session lineage reuse and operator reset gate
  proposal-019    Proposal 019 context-strategy framework gate
  proposal-022    Proposal 022 feedback fidelity score lift and rereview proof gate
  proposal-024    Proposal 024 run-surface information architecture gate
  proposal-025    Proposal 025 per-agent MCP policy and runtime validation gate
  proposal-026    Proposal 026 ACP-first runtime transport and Goose decoupling gate
  proposal-027    Proposal 027 Rust+SQLite local control-plane extraction gate
  proposal-027r   Proposal 027 unified read-only JSON/markdown rendering gate (legacy renderer)
  proposal-029    Proposal 029 second-wave ACP runtime profiles gate
  proposal-029-mcp  Proposal 029 MCP northbound auth and capability gate
  proposal-031,p031  Thin GraphQL-only UI inventory/static guard/write-path guide gate
  proposal-072,p072  UI action boundary gate: approval-only GraphQL UI mutations and MCP-only command routing
  proposal-077,p077  Proposal 077 closeout readiness gates (Rust domain/db/engine plus GraphQL/MCP readback parity; UI remote evidence separate)
  proposal-077-ui,p077-ui  Proposal 077 remote macOS compact/focus/backlink/accessibility runtime proof
  proposal-031-readiness,p031-readiness  Thin UI closeout readiness gate
  proposal-032    Proposal 032 atomic transition settlement and durable resume cursor gate
  proposal-033    Proposal 033 ACP-only runtime architecture gate
  proposal-037    Proposal 037 ACP execution supervision and idle watchdog gate
  proposal-041    Proposal 041 server parity harness and behavioral diff gate
  proposal-042    Proposal 042 daemon lifecycle / supervision / packaging gate (Rust)
  proposal-042-swift  Proposal 042 Swift-side gate (DaemonLifecycleClient + DiagnosticsBundle + PackagedBinary)
  proposal-042-packaging  Proposal 042 release-host packaging lane (codesign/notarize/Gatekeeper)
  proposal-043    Proposal 043 GraphQL projection read contract gate
  proposal-044    Proposal 044 post-approval task execution and release gate completion gate
  proposal-045    Proposal 045 deterministic release operations gate
  proposal-047    Proposal 047 control-plane workspace verification gate
  proposal-048    Proposal 048 evidence/preflight/MCP resolution gate
  proposal-049    Proposal 049 steward analysis system gate
  proposal-050    Proposal 050 per-run workspace isolation gate
  p051-scaffold   Proposal 051 scaffold gate for shared Xcode MCP bridge pool substrate
  proposal-051|p051  Proposal 051 shared Xcode MCP bridge pool fixture/readback gate
  proposal-053    Proposal 053 bounded ACP artifact discovery gate
  proposal-057    Proposal 057 canonical artifact contracts and run-state projection gate
  proposal-058    Proposal 058 ACP provider failure classification and artifact ownership gate
  proposal-060|p060  Proposal 060 Phase 0a/0b control artifact wrapper gate
  proposal-060-baseline|p060-baseline
                  Proposal 060 proposal-review baseline control artifact gate
  proposal-060-storage|p060-storage
                  Proposal 060 storage compatibility control artifact gate
  proposal-060-router-fixtures|p060-router-fixtures
                  Proposal 060 routing contract fixtures control artifact gate
  proposal-060-snapshot-inventory|p060-snapshot-inventory
                  Proposal 060 frozen snapshot inventory control artifact gate
  proposal-060-fixed-quartet|p060-fixed-quartet
                  Proposal 060 fixed quartet inventory control artifact gate
  proposal-060-ticket-map|p060-ticket-map
                  Proposal 060 implementation ticket map control artifact gate
  proposal-060-calibration|p060-calibration
                  Proposal 060 routing calibration control artifact gate
  proposal-061    Proposal 061 SQLite write serialization and scheduler backpressure gate
  proposal-064|p064  Proposal 064 Phase 0 main-sync and knowledge readback contract gate
  proposal-065|p065  Proposal 065 operator retry instruction contract gate
  proposal-066|p066  Proposal 066 Phase 0 toolchain cache mapping scaffold gate
  proposal-075|p075  Proposal 075 Phase 1 local persistence write budget scaffold gate
  proposal-054|p054  Proposal 054 implementation completeness and handoff contract gate
  proposal-054-v1-retirement|p054-v1-retirement
                  Proposal 054 release-cut check for zero active non-terminal v1-only runs
  proposal-084|p084  Proposal 084 executable rollout gates and observability contract gate
  proposal-085|p085  Proposal 085 thin-client read-model parity and affordance contract gate
  full            Full xcodebuild test sign-off gate
EOF
}

BEFORE_CRASH_LOG="$(latest_crash_log)"
trap '
  status=$?
  after_crash_log="$(latest_crash_log)"
  if [[ $status -ne 0 ]]; then
    if [[ -n "$after_crash_log" && "$after_crash_log" != "$BEFORE_CRASH_LOG" ]]; then
      printf "Latest new crash log: %s\n" "$after_crash_log" >&2
    fi
  fi
' EXIT

GATE="${1:-list}"

if should_wrap_gate_in_terminal_gui_session "$GATE"; then
  run_gate_in_terminal_gui_session "$GATE"
  exit $?
fi

case "$GATE" in
  list|-h|--help)
    print_usage
    ;;
  guardrails)
    check_idle_environment
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    guard_plan_tag_sync
    ;;
  build)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "build"
    ;;
  fast)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "fast"
    if [[ "${USE_TEST_PLANS:-}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/FastGate.xctestplan" ]]; then
      run_test_plan "fast" "FastGate"
    else
      run_targeted_tests "fast" "${FAST_TESTS[@]}"
    fi
    ;;
  ui-smoke)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "ui-smoke" "${UI_SMOKE_TESTS[@]}"
    ;;
  proposal-006|p006)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    if [[ "${USE_TEST_PLANS:-}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/ProviderGate.xctestplan" ]]; then
      run_test_plan "proposal-006" "ProviderGate"
    else
      run_targeted_tests "proposal-006" "${PROPOSAL_006_TESTS[@]}"
    fi
    ;;
  proposal-012|p012)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "proposal-012" "${PROPOSAL_012_TESTS[@]}"
    ;;
  proposal-013|p013)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_split_targeted_gate "proposal-013" "${PROPOSAL_013_TESTS[@]}"
    ;;
  proposal-014|p014)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "proposal-014" "${PROPOSAL_014_TESTS[@]}"
    ;;
  proposal-015|p015)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-015"
    run_targeted_tests "proposal-015-non-ui" "${PROPOSAL_015_NON_UI_TESTS[@]}"
    run_proposal015_app_proof "$LAST_BUILD_DERIVED_DATA_PATH"
    ;;
  proposal-017|p017)
    log "Workflow conflict gate: workflow authority, conflict truth, and lead mediation"
    check_idle_environment allow_app

    # Phase 0 Contract Freeze: verify existence of required backend artifacts
    log "Verifying Phase 0 backend contract artifacts..."
    required_artifacts=(
      "docs/reference/workflow-conflict-evidence/phase-0-approval-mediation-contract.json"
      "docs/reference/workflow-conflict-evidence/phase-0-mediation-execution-identity-contract.md"
      "docs/reference/workflow-conflict-evidence/phase-0-work-item-execution-owner-contract.json"
      "docs/reference/workflow-conflict-evidence/phase-0-phase-b-lead-resolver.json"
      "docs/reference/workflow-conflict-evidence/phase-0-settlement-service-boundary.md"
      "docs/reference/workflow-conflict-evidence/phase-0-artifact-manifest.json"
    )
    for art in "${required_artifacts[@]}"; do
      if [[ ! -f "$ROOT_DIR/$art" ]]; then
        die "Missing required Phase 0 artifact: $art"
      fi
    done
    python3 - "$ROOT_DIR/docs/reference/workflow-conflict-evidence/phase-0-phase-b-lead-resolver.json" <<'PY'
import json
import sys

path = sys.argv[1]
with open(path, "r", encoding="utf-8") as fh:
    payload = json.load(fh)
entries = payload.get("entries") or []
if not entries:
    raise SystemExit("Phase B lead resolver must have at least one attested entry")

seen = set()
for index, entry in enumerate(entries, 1):
    required = [
        "workflow_source_path",
        "catalog_source_path",
        "lead_agent_id",
        "lead_resolution_contract_ref",
        "mapping_owner",
        "entry_attested_by",
        "reviewed_at",
    ]
    missing = [field for field in required if not entry.get(field)]
    if missing:
        raise SystemExit(f"Phase B lead resolver entry {index} missing: {', '.join(missing)}")
    key = (entry["workflow_source_path"], entry["catalog_source_path"])
    if key in seen:
        raise SystemExit(
            "Phase B lead resolver has duplicate workflow/catalog entry: "
            f"{entry['workflow_source_path']} + {entry['catalog_source_path']}"
        )
    seen.add(key)
PY

    run_targeted_tests "proposal-017-swift" "${PROPOSAL_017_SWIFT_TESTS[@]}"
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-017-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      # Phase A: Workflow authority and conflict truth
      cargo test -p workflow proposal_017_ -- --test-threads=1 --nocapture
      cargo test -p workflow --test proposal_017_evidence_gate -- --test-threads=1 --nocapture
      cargo test -p domain --test proposal_017_workflow_conflict -- --test-threads=1 --nocapture
      cargo test -p db --test proposal_017_workflow_conflict_persistence -- --test-threads=1 --nocapture
      cargo test -p mcp-server proposal_017_ -- --test-threads=1 --nocapture
      cargo test -p graphql-server proposal_017_ -- --test-threads=1 --nocapture

      # Phase B/C: Lead mediation and owner-aware execution
      cargo test -p engine proposal_017_ -- --test-threads=1 --nocapture
      cargo test -p engine p017_mediation_ -- --test-threads=1 --nocapture
    )

    # P017 R2 audit closures: REL-001 (cancel-cascade) + API-001
    # (execution_attempts readback). These checks fail the gate if the
    # specific tests that prove the closure are absent. They are the
    # canonical readiness signals the audit asked for under READY-001.
    log "Verifying P017 R2 audit closure tests are present..."
    REL_001_TEST="p017_mediation_cancel_run_cascade"
    API_001_MCP_TEST="proposal_017_workflow_conflict_lead_mediation_execution_attempts"
    API_001_GQL_TEST="proposal_017_run_query_exposes_lead_mediation_execution_attempts"
    if ! grep -q "$REL_001_TEST" "$ROOT_DIR/control-plane/crates/engine/tests/integration.rs"; then
      die "P017 REL-001 closure missing: expected test $REL_001_TEST in engine integration tests"
    fi
    if ! grep -q "$API_001_MCP_TEST" "$ROOT_DIR/control-plane/crates/mcp-server/src/tools/reports.rs"; then
      die "P017 API-001 closure missing: expected MCP test $API_001_MCP_TEST"
    fi
    if ! grep -q "$API_001_GQL_TEST" "$ROOT_DIR/control-plane/crates/graphql-server/src/schema.rs"; then
      die "P017 API-001 closure missing: expected GraphQL test $API_001_GQL_TEST"
    fi
    # Also assert the source-level contracts: execution_attempts must be
    # named in both readback projections (catches accidental removal even
    # if a copycat test still passes).
    if ! grep -q "execution_attempts" "$ROOT_DIR/control-plane/crates/mcp-server/src/tools/reports.rs"; then
      die "P017 API-001: MCP lead_mediation_readback_json missing 'execution_attempts'"
    fi
    if ! grep -q "execution_attempts" "$ROOT_DIR/control-plane/crates/graphql-server/src/types/run.rs"; then
      die "P017 API-001: GraphQL GqlLeadMediation missing 'execution_attempts'"
    fi
    if ! grep -q "cancel_active_by_run_tx" "$ROOT_DIR/control-plane/crates/engine/src/cancellation.rs"; then
      die "P017 REL-001: cancellation.rs does not invoke cancel_active_by_run_tx for lead_conflict_mediations"
    fi

    # ARCH-001: equivalence record + proof test must both be present.
    ARCH_001_DOC="docs/reference/workflow-conflict-evidence/phase-b-mediation-execution-fields-equivalence.md"
    ARCH_001_TEST="p017_mediation_execution_fields_equivalence"
    if [[ ! -f "$ROOT_DIR/$ARCH_001_DOC" ]]; then
      die "P017 ARCH-001: missing equivalence record at $ARCH_001_DOC"
    fi
    if ! grep -q "$ARCH_001_TEST" "$ROOT_DIR/control-plane/crates/engine/tests/integration.rs"; then
      die "P017 ARCH-001: missing equivalence proof test $ARCH_001_TEST"
    fi

    # OPS-001: every committed P017 metric name must have at least one
    # production caller (not just the existing tx-helper) AND a unit test
    # that proves it inserts a metric_event row with the right labels.
    OPS_001_PHASE_C_TEST="p017_phase_c_validation_outcome_metric_emits"
    OPS_001_ATTEMPT_TEST="p017_lead_mediation_attempt_metric_emits"
    OPS_001_EXTERNAL_TEST="p017_external_catalog_warning_metric_emits"
    OPS_001_DB_PATH="$ROOT_DIR/control-plane/crates/db/tests/proposal_017_workflow_conflict_persistence.rs"
    for t in "$OPS_001_PHASE_C_TEST" "$OPS_001_ATTEMPT_TEST" "$OPS_001_EXTERNAL_TEST"; do
      if ! grep -q "$t" "$OPS_001_DB_PATH"; then
        die "P017 OPS-001: missing metric emit test $t in $OPS_001_DB_PATH"
      fi
    done
    # Production caller presence: all three helpers must be invoked outside
    # the helper definition itself (so the metric actually fires in real runs).
    if ! grep -q "record_phase_c_validation_outcome_tx" "$ROOT_DIR/control-plane/crates/engine/src/command_handler.rs"; then
      die "P017 OPS-001: phase_c_validation_outcome_total has no production caller in command_handler.rs"
    fi
    if ! grep -q "record_lead_mediation_attempt_tx" "$ROOT_DIR/control-plane/crates/engine/src/executor.rs"; then
      die "P017 OPS-001: lead_mediation_attempt_total has no production caller in executor.rs"
    fi
    if ! grep -q "record_external_catalog_warning_tx" "$ROOT_DIR/control-plane/crates/engine/src/command_handler.rs"; then
      die "P017 OPS-001: external_catalog_warning_total has no production caller in command_handler.rs"
    fi

    # ── R4 closure guards (API-002 + OPS-002) ──────────────────────────
    # API-002: per-attempt cost + transcript_ref populated by executor.
    if ! grep -q "update_attempt_attribution" "$ROOT_DIR/control-plane/crates/engine/src/executor.rs"; then
      die "P017 R4 API-002: executor must call update_attempt_attribution for mediation completions"
    fi
    if ! grep -q "p017_per_attempt_cost_and_transcript_persisted" \
        "$ROOT_DIR/control-plane/crates/db/tests/proposal_017_workflow_conflict_persistence.rs"; then
      die "P017 R4 API-002: missing per-attempt cost+transcript persistence test"
    fi
    P017_R4_MIGRATION="$ROOT_DIR/control-plane/crates/db/migrations/031_p017_metric_inventory_and_attempt_attribution.sql"
    if [[ ! -f "$P017_R4_MIGRATION" ]]; then
      die "P017 R4 API-002/OPS-002: missing migration 031_p017_metric_inventory_and_attempt_attribution.sql"
    fi
    for col in transcript_artifact_id total_cost_cents input_tokens output_tokens; do
      if ! grep -q "$col" "$P017_R4_MIGRATION"; then
        die "P017 R4 API-002: migration 031 must add $col column"
      fi
    done

    # OPS-002: 6 new metric helpers (5 audit-named + Phase C fail path).
    P017_R4_HELPERS=(
      "record_phase_c_validation_failure_tx"
      "record_duplicate_mediation_session_tx"
      "record_report_readback_completeness_tx"
      "record_phase_c_lead_inventory_external_catalog_tx"
      "record_mediation_late_output_ignored_tx"
      "record_mediation_retry_budget_exhausted_tx"
      "record_phase_b_dogfood_mediation_completion_rate_tx"
      "record_phase_b_dogfood_operator_guidance_sufficient_tx"
    )
    for h in "${P017_R4_HELPERS[@]}"; do
      if ! grep -q "$h" "$ROOT_DIR/control-plane/crates/db/src/repos/workflow_conflicts.rs"; then
        die "P017 R4 OPS-002: missing helper $h in workflow_conflicts.rs"
      fi
    done
    P017_R4_TESTS=(
      "p017_phase_c_validation_failure_metric_emits_without_run"
      "p017_duplicate_mediation_session_metric_emits"
      "p017_report_readback_completeness_metric_emits"
      "p017_phase_c_lead_inventory_external_catalog_metric_emits"
      "p017_mediation_late_output_ignored_metric_emits"
      "p017_mediation_retry_budget_exhausted_metric_emits"
      "p017_phase_b_dogfood_mediation_completion_rate_metric_emits"
      "p017_phase_b_dogfood_operator_guidance_sufficient_metric_emits"
    )
    for t in "${P017_R4_TESTS[@]}"; do
      if ! grep -q "$t" "$OPS_001_DB_PATH"; then
        die "P017 R4 OPS-002: missing metric emit test $t"
      fi
    done
    # Production callers for the new emissions.
    if ! grep -q "record_phase_c_validation_failure" "$ROOT_DIR/control-plane/crates/engine/src/command_handler.rs"; then
      die "P017 R4 OPS-002: phase_c_validation_outcome_total fail path has no production caller"
    fi
    if ! grep -q "record_phase_c_lead_inventory_external_catalog_tx" "$ROOT_DIR/control-plane/crates/engine/src/command_handler.rs"; then
      die "P017 R4 OPS-002: phase_c_lead_inventory_external_catalog_total has no production caller"
    fi
    if ! grep -q "record_duplicate_mediation_session_tx" "$ROOT_DIR/control-plane/crates/engine/src/orchestrator.rs"; then
      die "P017 R4 OPS-002: duplicate_mediation_session_total has no production caller"
    fi
    if ! grep -q "record_report_readback_completeness_tx" "$ROOT_DIR/control-plane/crates/mcp-server/src/tools/reports.rs"; then
      die "P017 R4 OPS-002: report_readback_completeness has no production caller"
    fi
    if ! grep -q "record_mediation_late_output_ignored_tx" "$ROOT_DIR/control-plane/crates/engine/src/executor.rs"; then
      die "P017 R4 OPS-002: mediation_late_output_ignored_total has no production caller"
    fi
    if ! grep -q "record_mediation_retry_budget_exhausted_tx" "$ROOT_DIR/control-plane/crates/engine/src/executor.rs"; then
      die "P017 R6 OPS-001: mediation_retry_budget_exhausted_total has no production caller"
    fi
    if ! grep -q "record_phase_b_dogfood_mediation_completion_rate_tx" "$ROOT_DIR/control-plane/crates/engine/src/command_handler.rs"; then
      die "P017 R6 OPS-001: phase_b_dogfood_mediation_completion_rate has no production caller"
    fi
    if ! grep -q "record_phase_b_dogfood_operator_guidance_sufficient_tx" "$ROOT_DIR/control-plane/crates/engine/src/command_handler.rs"; then
      die "P017 R6 OPS-001: phase_b_dogfood_operator_guidance_sufficient_total has no production caller"
    fi

    # ── R5 closure guards (API-003 + REL-002 + OPS-003) ────────────────
    # API-003: per-attempt artifact direct linkage.
    P017_R5_ARTIFACT_MIGRATION="$ROOT_DIR/control-plane/crates/db/migrations/032_p017_per_attempt_artifact_linkage.sql"
    if [[ ! -f "$P017_R5_ARTIFACT_MIGRATION" ]]; then
      die "P017 R5 API-003: missing migration 032_p017_per_attempt_artifact_linkage.sql"
    fi
    if ! grep -q "agent_execution_id" "$P017_R5_ARTIFACT_MIGRATION"; then
      die "P017 R5 API-003: migration 032 must add artifacts.agent_execution_id column"
    fi
    if ! grep -q "list_by_agent_execution" "$ROOT_DIR/control-plane/crates/db/src/repos/artifacts.rs"; then
      die "P017 R5 API-003: artifacts repo missing list_by_agent_execution"
    fi
    if ! grep -q "list_by_agent_execution" "$ROOT_DIR/control-plane/crates/mcp-server/src/tools/reports.rs"; then
      die "P017 R5 API-003: MCP execution_attempts.artifacts must use list_by_agent_execution"
    fi
    if ! grep -q "list_by_agent_execution" "$ROOT_DIR/control-plane/crates/graphql-server/src/types/run.rs"; then
      die "P017 R5 API-003: GraphQL execution_attempts.artifacts must use list_by_agent_execution"
    fi
    if ! grep -q "execution_id_direct" "$ROOT_DIR/control-plane/crates/mcp-server/src/tools/reports.rs"; then
      die "P017 R5 API-003: MCP must label tier-2 artifacts as execution_id_direct"
    fi
    # REL-002: attribution + completion atomic in single transaction.
    if ! grep -q "mediation.complete_with_attribution" "$ROOT_DIR/control-plane/crates/engine/src/executor.rs"; then
      die "P017 R5 REL-002: executor must commit completion+attribution in a single transaction"
    fi
    if ! grep -q "update_attempt_attribution_tx" "$ROOT_DIR/control-plane/crates/engine/src/executor.rs"; then
      die "P017 R5 REL-002: executor must call update_attempt_attribution_tx (transactional variant)"
    fi
    if ! grep -q "artifacts::insert_tx(&mut completion_tx" "$ROOT_DIR/control-plane/crates/engine/src/executor.rs"; then
      die "P017 R6 REL-001: mediation transcript artifact row must be inserted inside mediation.complete_with_attribution tx"
    fi
    # OPS-003: 4 new metric helpers + production callers + tests.
    P017_R5_HELPERS=(
      "record_advisory_rejection_tx"
      "record_invalid_next_stage_hint_non_blocking_tx"
      "record_workflow_conflict_current_tx"
      "record_terminal_unverifiable_tx"
    )
    for h in "${P017_R5_HELPERS[@]}"; do
      if ! grep -q "$h" "$ROOT_DIR/control-plane/crates/db/src/repos/workflow_conflicts.rs"; then
        die "P017 R5 OPS-003: missing helper $h"
      fi
    done
    P017_R5_TESTS=(
      "p017_advisory_rejection_metrics_emit"
      "p017_workflow_conflict_current_metric_emits"
      "p017_terminal_unverifiable_metric_emits"
    )
    for t in "${P017_R5_TESTS[@]}"; do
      if ! grep -q "$t" "$OPS_001_DB_PATH"; then
        die "P017 R5 OPS-003: missing metric emit test $t"
      fi
    done

    log "Workflow conflict gate passed"
    ;;
  proposal-018|p018)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-018"
    run_targeted_tests "proposal-018" "${PROPOSAL_018_TESTS[@]}"
    ;;
  proposal-019|p019)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-019"
    run_targeted_tests "proposal-019" "${PROPOSAL_019_TESTS[@]}"
    ;;
  proposal-022|p022)
    check_idle_environment strict
    require_remote_ui_host
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-022"
    run_split_targeted_gate "proposal-022" "${PROPOSAL_022_TESTS[@]}"
    run_proposal022_app_proof "$LAST_BUILD_DERIVED_DATA_PATH"
    ;;
  proposal-024|p024)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-024"
    run_split_targeted_gate "proposal-024" "${PROPOSAL_024_TESTS[@]}"
    ;;
  proposal-025|p025)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    guard_portability_paths
    run_build "proposal-025"
    run_targeted_tests "proposal-025" "${PROPOSAL_025_TESTS[@]}"
    ;;
  proposal-026|p026)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-026"
    run_targeted_tests "proposal-026" "${PROPOSAL_026_TESTS[@]}"
    ;;
  proposal-027|p027)
    log "Proposal 027 control-plane gate: Rust+SQLite daemon test suite"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test --workspace 2>&1
    )
    log "Proposal 027 control-plane gate passed"
    ;;
  proposal-027r|p027r)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-027r"
    run_targeted_tests "proposal-027r" "${PROPOSAL_027_TESTS[@]}"
    ;;
  proposal-029|p029)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-029"
    run_targeted_tests "proposal-029" "${PROPOSAL_029_TESTS[@]}"
    ;;
  proposal-029-mcp|p029-mcp)
    log "Proposal 029-MCP control-plane gate: auth + capability + audit"
    log "  running ${#PROPOSAL_029_MCP_TESTS[@]} focused tests from §9.1 inventory"
    (
      cd "$ROOT_DIR/control-plane"
      # Per §9.1 gate-wrapper rule: every test in the inventory must run
      # and pass. Drift (rename/delete/typo) must fail the gate — we enforce
      # this by post-checking each cargo invocation for a matching
      # `test <name>` line. A final workspace regression run follows, but
      # is NOT a substitute for the enumerated inventory.
      #
      # Cleanup is explicit (no EXIT trap) so the subshell cannot emit any
      # trailing noise after the "gate passed" log line.
      tmp_log="$(mktemp)"
      for spec in "${PROPOSAL_029_MCP_TESTS[@]}"; do
        crate="${spec%% *}"
        test_name="${spec#* }"
        : >"$tmp_log"
        if ! cargo test -p "$crate" "$test_name" -- --nocapture 2>&1 | tee -a "$tmp_log"; then
          echo "proposal-029-mcp: FAIL — $crate::$test_name returned a non-zero exit"
          rm -f "$tmp_log"
          exit 1
        fi
        # Enforce that the named test actually ran. Cargo prints one of:
        #   test <name> ... ok          (integration test at binary root)
        #   test <mod>::<name> ... ok   (unit test inside a module)
        # The trailing whitespace + `...` guards against prefix matches
        # (e.g. "test_x" vs "test_x_other").
        if ! grep -E "^test ([A-Za-z0-9_]+::)*${test_name}[[:space:]]" "$tmp_log" >/dev/null; then
          echo "proposal-029-mcp: FAIL — no test named '$test_name' produced output in crate '$crate' (renamed, deleted, or typo'd?)"
          rm -f "$tmp_log"
          exit 1
        fi
      done
      rm -f "$tmp_log"
      cargo test --workspace 2>&1
    )
    log "Proposal 029-MCP control-plane gate passed"
    ;;
  proposal-032|p032)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-032"
    run_targeted_tests "proposal-032" "${PROPOSAL_032_TESTS[@]}"
    ;;
  proposal-033|p033)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    log "Prerequisite: proposal-029 gate (second-wave ACP)"
    run_targeted_tests "proposal-029-prereq" "${PROPOSAL_029_TESTS[@]}"
    run_build "proposal-033"
    run_targeted_tests "proposal-033" "${PROPOSAL_033_TESTS[@]}"
    ;;
  proposal-037|p037)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-037"
    run_split_targeted_gate "proposal-037" "${PROPOSAL_037_TESTS[@]}"
    ;;
  proposal-041|p041)
    echo "[INFO] Proposal 041 control-plane gate: server parity harness, golden fixtures, and behavioral diff"
    export P041_GATE_DEADLINE_SECONDS="${P041_GATE_DEADLINE_SECONDS:-1500}"
    export P041_DRAIN_GRACE_SECONDS="${P041_DRAIN_GRACE_SECONDS:-30}"
    export P041_REPLAY_DEADLINE_SECONDS="${P041_REPLAY_DEADLINE_SECONDS:-60}"
    export P041_READBACK_DEADLINE_SECONDS="${P041_READBACK_DEADLINE_SECONDS:-30}"
    export P041_SHADOW_DEADLINE_SECONDS="${P041_SHADOW_DEADLINE_SECONDS:-60}"
    _p041_deadline_now="$(date +%s)"
    export P041_GATE_DEADLINE_UNIX_MS="$(( (_p041_deadline_now + P041_GATE_DEADLINE_SECONDS) * 1000 ))"
    _p041_cap_idx=0
    for fixture_id in \
      proposal-loop-basic \
      implementation-refine-review \
      approval-pause-resume \
      retry-recovery-flow \
      cancelled-or-blocked-run \
      terminal-report-evidence \
      projection-readback-surface; do
      _p041_cap_idx=$((_p041_cap_idx + 1))
      echo "[INFO] [${_p041_cap_idx}/7] validate-capture ${fixture_id}"
      P041_STEP_FIXTURE="$fixture_id" \
      P041_COMMAND_DEADLINE_SECONDS="$P041_REPLAY_DEADLINE_SECONDS" \
        p041_supervised_run "$ROOT_DIR/scripts/parity/capture-golden-run.sh" "$fixture_id" --validate || exit $?
      echo "[PASS] [${_p041_cap_idx}/7] validate-capture ${fixture_id}"
    done
    (
      cd "$ROOT_DIR/control-plane"
      # Capture git provenance once before any replay work begins (Section 6.2/6.3).
      # Exported as P041_GIT_* env vars so cargo test can embed the same provenance
      # in per-fixture server-replay.json and behavioral-diff-report.json artifacts.
      if ! _p041_commit=$(git rev-parse HEAD 2>/dev/null); then
        echo "[FAIL] proposal-041: unable to capture git HEAD provenance" >&2
        exit 1
      fi
      if ! _p041_tree=$(git rev-parse 'HEAD^{tree}' 2>/dev/null); then
        echo "[FAIL] proposal-041: unable to capture git tree provenance" >&2
        exit 1
      fi
      if ! _p041_status=$(git status --porcelain=v1 --untracked-files=all 2>/dev/null); then
        echo "[FAIL] proposal-041: unable to capture git status snapshot" >&2
        exit 1
      fi
      _p041_clean=$([ -z "$_p041_status" ] && echo true || echo false)
      _p041_line_count=$(printf '%s' "$_p041_status" | awk 'NF { count += 1 } END { print count + 0 }')
      # sha256: prefer shasum (macOS), fall back to sha256sum (Linux)
      if _p041_sha256=$(printf '%s' "$_p041_status" | shasum -a 256 2>/dev/null | cut -d' ' -f1); then
        :
      elif _p041_sha256=$(printf '%s' "$_p041_status" | sha256sum 2>/dev/null | cut -d' ' -f1); then
        :
      else
        echo "[FAIL] proposal-041: unable to hash git status snapshot" >&2
        exit 1
      fi
      if [ -z "$_p041_commit" ] || [ -z "$_p041_tree" ] || [ -z "$_p041_sha256" ]; then
        echo "[FAIL] proposal-041: captured git provenance is incomplete" >&2
        exit 1
      fi
      export P041_GIT_COMMIT_SHA="$_p041_commit"
      export P041_GIT_TREE_ID="$_p041_tree"
      export P041_GIT_TREE_CLEAN="$_p041_clean"
      export P041_GIT_STATUS_SNAPSHOT_LINE_COUNT="$_p041_line_count"
      export P041_GIT_STATUS_SNAPSHOT_SHA256="$_p041_sha256"
      # Generate a unique publication_generation_id for this gate run.
      # Exported so all cargo test binaries embed the same generation ID in their
      # artifacts (server-replay.json, behavioral-diff-report.json, row/detail).
      _p041_gen_ts=$(date -u '+%Y-%m-%dT%H:%M:%SZ' 2>/dev/null || echo "ts-unknown")
      _p041_gen_rand=$(od -vAn -N4 -tx4 /dev/urandom 2>/dev/null | tr -d ' \n' | cut -c1-8 \
                       || printf '%08x' $((RANDOM * RANDOM % 0xffffffff)))
      export P041_PUBLICATION_GENERATION_ID="p041-${_p041_gen_ts}-${_p041_gen_rand}"
      echo "[INFO] p041 generation ${P041_PUBLICATION_GENERATION_ID}"
      p041_prefixed_run python3 - <<'PY' &&
import json
import os
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

generation_id = os.environ["P041_PUBLICATION_GENERATION_ID"]
commit_sha = os.environ["P041_GIT_COMMIT_SHA"]
tree_id = os.environ["P041_GIT_TREE_ID"]
tree_clean = os.environ["P041_GIT_TREE_CLEAN"] == "true"
status_sha = os.environ["P041_GIT_STATUS_SNAPSHOT_SHA256"]
status_lines = int(os.environ["P041_GIT_STATUS_SNAPSHOT_LINE_COUNT"])
now_ms = int(time.time() * 1000)
now_iso = time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime())

parity_root = Path("target/parity")
control_root = Path("target/parity-control")
current_root = parity_root / "publication/current"
generation_root = parity_root / "publication/generations" / generation_id

# Symlink boundary hardening (SEC-P041-003): reject before any mkdir/write if
# target, parity_root, or control_root is a symlink or resolves outside
# control-plane/target/. This keeps publication writes and pruning from escaping
# the expected tree.
_target_anchor = Path("target").resolve()
def _validate_p041_target_boundary(_check_path: Path, _label: str) -> None:
    if Path("target").is_symlink():
        raise SystemExit(
            "proposal-041: target is a symlink; refusing to write parity artifacts "
            "through a redirected root (SEC-P041-003)."
        )
    if _check_path.is_symlink():
        raise SystemExit(
            f"proposal-041: {_label} is a symlink; refusing to write parity artifacts "
            f"to a redirected root (SEC-P041-003). Remove the symlink and rerun."
        )
    _resolved_check = _check_path.resolve()
    if not str(_resolved_check).startswith(str(_target_anchor)):
        raise SystemExit(
            f"proposal-041: {_label} resolves to {_resolved_check}, which is outside "
            f"control-plane/target/; refusing to write (SEC-P041-003)."
        )

for _check_path, _label in (
    (parity_root, "target/parity"),
    (control_root, "target/parity-control"),
    (current_root, "target/parity/publication/current"),
    (generation_root, "target/parity/publication/generations/<generation>"),
):
    _validate_p041_target_boundary(_check_path, _label)

for root in (parity_root, control_root, current_root, generation_root):
    root.mkdir(parents=True, exist_ok=True)
for root in (parity_root, control_root):
    (root / ".metadata_never_index").touch()

def _get_process_birth_unix_ms(pid: int) -> int:
    """Return process birth time in Unix ms via proc_pidinfo on Darwin.

    Falls back to current wall-clock time on non-Darwin or if proc_pidinfo
    is unavailable. Per proposal Section 6.3, ps is forbidden.
    """
    if sys.platform == "darwin":
        try:
            import ctypes, ctypes.util, struct
            libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
            # struct proc_bsdinfo layout (first two timing fields):
            # pbi_start_tvsec (uint64) at offset 168, pbi_start_tvusec (uint32) at offset 176
            # We allocate the full struct size (416 bytes) and read start time fields.
            PROC_PIDTBSDINFO = 3
            buf_size = 416
            buf = ctypes.create_string_buffer(buf_size)
            ret = libc.proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, buf, buf_size)
            if ret >= buf_size:
                # pbi_start_tvsec is at offset 168 (uint64_t), pbi_start_tvusec at 176 (uint32_t)
                sec = struct.unpack_from("<Q", buf, 168)[0]
                usec = struct.unpack_from("<I", buf, 176)[0]
                ms = sec * 1000 + usec // 1000
                if ms > 0:
                    return ms
        except Exception:
            pass
    return int(time.time() * 1000)

def _darwin_fullfsync(fd: int) -> bool:
    """Try F_FULLFSYNC on Darwin; return True if successful."""
    try:
        import ctypes, ctypes.util
        libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
        F_FULLFSYNC = 51  # Darwin-specific fcntl cmd
        return libc.fcntl(fd, F_FULLFSYNC) == 0
    except Exception:
        return False

def atomic_json(path, value):
    """Write value as pretty JSON via same-dir temp + durable flush + atomic rename.

    Satisfies the durable-write contract (Section 6.3): temp file in the same
    directory (same-volume rule), durable flush (F_FULLFSYNC on Darwin, fsync
    fallback), atomic rename, best-effort parent-dir fsync.
    """
    _validate_p041_target_boundary(path.parent, f"{path.parent}")
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(value, handle, indent=2)
            handle.write("\n")
            handle.flush()
            raw_fd = handle.fileno()
            if sys.platform == "darwin":
                if not _darwin_fullfsync(raw_fd):
                    os.fsync(raw_fd)
            else:
                os.fsync(raw_fd)
        os.replace(tmp, path)
        # Best-effort parent-dir durability barrier.
        try:
            dir_fd = os.open(str(path.parent), os.O_RDONLY)
            try:
                os.fsync(dir_fd)
            finally:
                os.close(dir_fd)
        except OSError:
            pass
    finally:
        try:
            if os.path.exists(tmp):
                os.unlink(tmp)
        except OSError:
            pass

provenance = {
    "commit_sha": commit_sha,
    "tree_id": tree_id,
    "tree_clean": tree_clean,
    "status_snapshot_sha256": status_sha,
    "status_snapshot_line_count": status_lines,
    "generated_at": now_iso,
    "gate": "./scripts/test-gate.sh proposal-041",
}
fixtures = [
    "proposal-loop-basic",
    "implementation-refine-review",
    "approval-pause-resume",
    "retry-recovery-flow",
    "cancelled-or-blocked-run",
    "terminal-report-evidence",
    "projection-readback-surface",
]
surfaces = [
    "canonical_domain_state",
    "projections",
    "graphql_readback",
    "mcp_report_readback",
    "artifact_identity",
    "operator_summary",
]
detail = {
    "schema_version": "p031-p041-parity-evidence.v1",
    "overall_status": "blocked_in_progress",
    "publication_generation_id": generation_id,
    "publication_state": "revoked_for_rerun",
    "required_fixtures": fixtures,
    "required_surfaces": surfaces,
    "fixtures": [
        {
            "fixture_id": fixture_id,
            "report_path": f"control-plane/target/parity/reports/{generation_id}/{fixture_id}/behavioral-diff-report.json",
            "replay_path": f"control-plane/target/parity/work/{generation_id}/{fixture_id}/server-replay.json",
            "shadow_report_path": f"control-plane/target/parity/shadow/{generation_id}/{fixture_id}/live-shadow-report.json",
            "verdict": "blocked_in_progress",
        }
        for fixture_id in fixtures
    ],
    "blocking_reasons": ["rerun_in_progress"],
    "missing_evidence": [],
    "provenance": provenance,
}
row = {
    "schema_version": "p031-phase-0-runtime-manifest-row.v1",
    "id": "p041_parity_evidence",
    "runtime_detail_path": "control-plane/target/parity/publication/current/p031-p041-parity-evidence.json",
    "reference_detail_path": "docs/reference/p031-p041-parity-evidence.json",
    "validation_status": "blocked_in_progress",
    "publication_state": "revoked_for_rerun",
    "publication_generation_id": generation_id,
    "detail_schema_version": "p031-p041-parity-evidence.v1",
    "provenance": provenance,
}

def _publish_blocked_manual_recovery(reason: str) -> None:
    """Publish blocked_manual_recovery runtime row+detail to current/ before an early exit.

    Proposal Section 6.3 requires the CLI and runtime detail artifact to surface the
    unresolved owner state so downstream consumers (p031-thin-ui-gate.py) do not continue
    trusting stale current/ artifacts as if they were authoritative.
    """
    _mr_detail = dict(detail)
    _mr_detail["overall_status"] = "blocked_manual_recovery"
    _mr_detail["publication_state"] = "diagnostic_blocked"
    _mr_detail["blocking_reasons"] = [reason]
    _mr_row = dict(row)
    _mr_row["validation_status"] = "blocked_manual_recovery"
    _mr_row["publication_state"] = "diagnostic_blocked"
    try:
        atomic_json(current_root / "p031-p041-parity-evidence.json", _mr_detail)
        atomic_json(current_root / "p031-phase-0-manifest-row.json", _mr_row)
    except OSError as _pub_err:
        print(f"[WARN] proposal-041: unable to publish blocked_manual_recovery artifacts: {_pub_err}")

# Reclaim check: inspect existing lease and reclaim-marker before proceeding.
# Implements proposal Section 6.3 reclaim matrix (Cases A/B/C/D).
_reclaim_marker_path = control_root / "reclaim-marker.json"
_existing_lease_path = control_root / "lease.json"
_release_marker_path = control_root / "release-marker.json"
_deadline_ms = int(os.environ.get("P041_GATE_DEADLINE_UNIX_MS", "0") or 0)

def _remaining_deadline_ms() -> int:
    if _deadline_ms <= 0:
        return 120_000
    return max(0, _deadline_ms - int(time.time() * 1000))

def _freshness_window_ms() -> int:
    remaining = _remaining_deadline_ms()
    if remaining <= 0:
        return 1_000
    return max(1_000, min(120_000, remaining // 4))

def _pgid_has_observable_descendants(pgid: int, owner_pid: int = 0) -> bool:
    if pgid <= 0:
        return False
    try:
        out = subprocess.check_output(
            ["ps", "-axo", "pid=,pgid="],
            stderr=subprocess.DEVNULL,
            text=True,
        )
    except Exception:
        return True
    own_pid = os.getpid()
    for line in out.splitlines():
        parts = line.split()
        if len(parts) < 2:
            continue
        try:
            pid = int(parts[0])
            proc_pgid = int(parts[1])
        except ValueError:
            continue
        if proc_pgid == pgid and pid not in {own_pid, owner_pid}:
            return True
    return False

def _preserved_generation_root(gen_id: str):
    candidate = Path("target/parity/publication/generations") / gen_id
    return str(candidate) if candidate.exists() else None

def _write_reclaim_marker(
    abandoned_lease: dict,
    overall_status: str,
    observation_count: int,
    freshness_window_ms: int,
    missing_pgid_metadata: bool,
    observable_descendants: bool,
    diagnostic_message: str,
) -> None:
    old_pgid = int(abandoned_lease.get("pgid", 0) or 0)
    old_gen = abandoned_lease.get("publication_generation_id", "unknown")
    atomic_json(_reclaim_marker_path, {
        "schema_version": "parity-control-reclaim-marker.v1",
        "overall_status": overall_status,
        "abandoned_generation_id": old_gen,
        "preserved_generation_root": _preserved_generation_root(old_gen),
        "owner_pid": int(abandoned_lease.get("pid", 0) or 0),
        "owner_process_birth_unix_ms": int(abandoned_lease.get("process_birth_unix_ms", 0) or 0),
        "owner_pgid": old_pgid if old_pgid > 0 else None,
        "owner_hostname": abandoned_lease.get("hostname", "unknown"),
        "owner_last_heartbeat_unix_ms": int(abandoned_lease.get("heartbeat_unix_ms", 0) or 0),
        "owner_last_control_sequence": int(abandoned_lease.get("control_sequence", 0) or 0),
        "observation_count": observation_count,
        "freshness_window_ms": freshness_window_ms,
        "missing_pgid_metadata": missing_pgid_metadata,
        "observable_descendants": observable_descendants,
        "written_at_unix_ms": int(time.time() * 1000),
        "diagnostic_message": diagnostic_message,
    })
if _reclaim_marker_path.exists():
    try:
        _rm = json.loads(_reclaim_marker_path.read_text())
        if _rm.get("overall_status") == "blocked_manual_recovery":
            # Publish blocked_manual_recovery to current/ so downstream consumers see the
            # real state rather than stale artifacts from the last completed run.
            _publish_blocked_manual_recovery("blocked_manual_recovery_reclaim_marker_present")
            print(
                f"[FAIL] proposal-041: found blocked_manual_recovery reclaim marker "
                f"at {_reclaim_marker_path}. Manual recovery required before rerun. "
                f"Inspect: abandoned_generation_id={_rm.get('abandoned_generation_id')} "
                f"owner_pid={_rm.get('owner_pid')} "
                f"preserved_generation_root={_rm.get('preserved_generation_root')}",
                file=sys.stderr,
            )
            raise SystemExit(
                "proposal-041: blocked_manual_recovery; remove reclaim-marker.json "
                "after resolving the stale owner"
            )
        # reclaim_allowed or unknown status: proceed (existing reclaim was documented)
    except (json.JSONDecodeError, OSError) as _reclaim_err:
        # Unreadable reclaim marker — fail closed per Section 6.3.
        _publish_blocked_manual_recovery("unreadable_reclaim_marker")
        print(
            f"[FAIL] proposal-041: reclaim-marker.json exists but is unreadable "
            f"({_reclaim_err}). Remove or repair {_reclaim_marker_path} before retrying.",
            file=sys.stderr,
        )
        raise SystemExit(
            "proposal-041: unreadable reclaim-marker.json; fail-closed "
            "(remove and rerun after verifying no active gate processes remain)"
        )

if _existing_lease_path.exists():
    try:
        _old_lease = json.loads(_existing_lease_path.read_text())
        _old_pid = int(_old_lease.get("pid", 0))
        _old_gen = _old_lease.get("publication_generation_id", "unknown")
        _old_birth_ms = int(_old_lease.get("process_birth_unix_ms", 0))
        _old_pgid = int(_old_lease.get("pgid", 0))
        _old_heartbeat_ms = int(_old_lease.get("heartbeat_unix_ms", 0))
        _old_seq = int(_old_lease.get("control_sequence", 0))

        # Check if old owner PID is still alive.
        _pid_alive = False
        if _old_pid:
            try:
                os.kill(_old_pid, 0)
                _pid_alive = True
            except (OSError, ProcessLookupError):
                pass

        if _pid_alive:
            # Verify birth time to distinguish a recycled PID from the original owner.
            _current_birth_ms = _get_process_birth_unix_ms(_old_pid)
            _birth_matches = (
                _old_birth_ms > 0
                and abs(_old_birth_ms - _current_birth_ms) < 5000
            )
            if _birth_matches:
                _window_ms = _freshness_window_ms()
                time.sleep(_window_ms / 1000.0)
                try:
                    _latest_lease = json.loads(_existing_lease_path.read_text())
                except (json.JSONDecodeError, OSError):
                    _publish_blocked_manual_recovery("unreadable_lease_after_stall_observation")
                    raise SystemExit("proposal-041: lease unreadable after Case A observation")
                _latest_heartbeat = int(_latest_lease.get("heartbeat_unix_ms", 0) or 0)
                _latest_seq = int(_latest_lease.get("control_sequence", 0) or 0)
                if _latest_heartbeat == _old_heartbeat_ms and _latest_seq == _old_seq:
                    _observable = _pgid_has_observable_descendants(_old_pgid, _old_pid)
                    _write_reclaim_marker(
                        _old_lease,
                        "blocked_manual_recovery",
                        2,
                        _window_ms,
                        _old_pgid <= 0,
                        _observable,
                        "Case A2: owner PID alive but heartbeat and control_sequence were unchanged across two observations.",
                    )
                    _publish_blocked_manual_recovery("case_a2_stalled_owner_requires_manual_recovery")
                    print(
                        f"[FAIL] proposal-041: gate owner PID {_old_pid} stalled "
                        f"for {_window_ms}ms (Case A2). Written blocked_manual_recovery "
                        f"marker at {_reclaim_marker_path}.",
                        file=sys.stderr,
                    )
                    raise SystemExit("proposal-041: stalled active gate owner (Case A2); fail-closed")
                # Case A: owner alive but fresh — fail closed as in-progress.
                atomic_json(current_root / "p031-p041-parity-evidence.json", detail)
                atomic_json(current_root / "p031-phase-0-manifest-row.json", row)
                print(
                    f"[FAIL] proposal-041: gate owner PID {_old_pid} is alive and fresh "
                    f"(Case A). Wait for the active run to complete. "
                    f"heartbeat_unix_ms={_latest_heartbeat} control_sequence={_latest_seq}",
                    file=sys.stderr,
                )
                raise SystemExit("proposal-041: active gate owner still running (Case A); fail-closed")
            # PID alive but birth time does not match: OS recycled the PID.
            # Fall through to PID-gone reclaim handling below.
            _pid_alive = False

        # PID is gone or recycled. Apply Cases B/C/D using pgid descendant proof.
        if _old_pgid <= 0:
            _write_reclaim_marker(
                _old_lease,
                "blocked_manual_recovery",
                1,
                0,
                True,
                False,
                "Case B: owner PID gone but process-group metadata is missing; descendant absence cannot be proven.",
            )
            _publish_blocked_manual_recovery("case_b_missing_pgid_metadata")
            print(
                f"[FAIL] proposal-041: previous owner PID {_old_pid} is gone but "
                f"lease for generation {_old_gen} has no pgid metadata (Case B). "
                f"Written blocked_manual_recovery marker at {_reclaim_marker_path}.",
                file=sys.stderr,
            )
            raise SystemExit("proposal-041: missing pgid metadata (Case B); fail-closed")
        _observable_descendants = _pgid_has_observable_descendants(_old_pgid, _old_pid)
        if _observable_descendants:
            _write_reclaim_marker(
                _old_lease,
                "blocked_manual_recovery",
                1,
                0,
                False,
                True,
                "Case C: owner PID gone but process-group descendants are still observable.",
            )
            _publish_blocked_manual_recovery("case_c_observable_descendants")
            print(
                f"[FAIL] proposal-041: previous owner PID {_old_pid} is gone but "
                f"pgid {_old_pgid} still has observable descendants (Case C).",
                file=sys.stderr,
            )
            raise SystemExit("proposal-041: observable descendants (Case C); fail-closed")
        _write_reclaim_marker(
            _old_lease,
            "reclaim_allowed",
            1,
            0,
            False,
            False,
            "Case D: owner PID gone, pgid metadata present, and descendant absence proven.",
        )
        print(
            f"[INFO] proposal-041: reclaimed generation {_old_gen} "
            f"(Case D: owner PID {_old_pid} gone, pgid {_old_pgid} descendant absence proven)"
        )

    except SystemExit:
        raise
    except (json.JSONDecodeError, OSError) as _lease_err:
        # Unreadable lease — fail closed per Section 6.3.
        print(
            f"[FAIL] proposal-041: lease.json exists but is unreadable ({_lease_err}). "
            f"Remove or repair {_existing_lease_path} before retrying.",
            file=sys.stderr,
        )
        raise SystemExit(
            "proposal-041: unreadable lease.json; fail-closed (remove and rerun)"
        )

own_pid = os.getpid()
own_birth_ms = _get_process_birth_unix_ms(own_pid)
own_pgid = os.getpgrp()
lease = {
    "schema_version": "parity-control-lease.v1",
    "pid": own_pid,
    "process_birth_unix_ms": own_birth_ms,
    "pgid": own_pgid,
    "hostname": socket.gethostname(),
    "commit_sha": commit_sha,
    "tree_id": tree_id,
    "heartbeat_unix_ms": now_ms,
    "publication_generation_id": generation_id,
    "control_sequence": 1,
}
current_step = {
    "schema_version": "parity-control-current-step.v1",
    "generation": generation_id,
    "fixture": None,
    "step": "pre_cleanup_publication",
    "surface": None,
    "mode": "gate",
    "elapsed_ms": 0,
    "heartbeat_unix_ms": now_ms,
}
atomic_json(current_root / "p031-p041-parity-evidence.json", detail)
atomic_json(current_root / "p031-phase-0-manifest-row.json", row)
atomic_json(generation_root / "p031-p041-parity-evidence.json", detail)
atomic_json(generation_root / "p031-phase-0-manifest-row.json", row)
atomic_json(control_root / "lease.json", lease)
atomic_json(control_root / "current-step.json", current_step)
print("[INFO] runtime publication revoked_for_rerun current generation updated")
PY
      echo "[INFO] [phase: prebuild] compiling P041 test binaries before per-fixture deadlines" &&
      p041_update_current_step 'prebuild' '' &&
      p041_update_lease_heartbeat &&
      p041_supervised_run cargo test -p engine --test proposal_041_parity --no-run &&
      p041_supervised_run cargo test -p graphql-server --lib --no-run &&
      p041_supervised_run cargo test -p mcp-server --lib --no-run &&
      echo "[INFO] [phase: fixture-inventory] validating all 7 fixture schemas and required elements" &&
      p041_update_current_step 'fixture_inventory' '' &&
      p041_update_lease_heartbeat &&
      p041_supervised_run cargo test -p engine --test proposal_041_parity proposal_041_fixture_inventory_and_schema_contract -- --exact --nocapture &&
      echo "[INFO] [phase: offline-replay] replaying all 7 fixtures into generation-scoped SQLite databases" &&
      for fixture_id in proposal-loop-basic implementation-refine-review approval-pause-resume retry-recovery-flow cancelled-or-blocked-run terminal-report-evidence projection-readback-surface; do
        echo "[INFO] [phase: offline-replay] fixture=${fixture_id} deadline=${P041_REPLAY_DEADLINE_SECONDS}s"
        p041_update_current_step 'offline_replay' '' &&
        p041_update_lease_heartbeat &&
        P041_ONLY_FIXTURE="$fixture_id" P041_STEP_FIXTURE="$fixture_id" P041_COMMAND_DEADLINE_SECONDS="$P041_REPLAY_DEADLINE_SECONDS" \
          p041_supervised_run cargo test -p engine --test proposal_041_parity proposal_041_offline_replay_emits_behavioral_diff_reports -- --exact --nocapture || exit $?
      done &&
      echo "[INFO] [phase: shadow-validation] checking live-shadow side-effect policy for all 7 fixtures" &&
      for fixture_id in proposal-loop-basic implementation-refine-review approval-pause-resume retry-recovery-flow cancelled-or-blocked-run terminal-report-evidence projection-readback-surface; do
        echo "[INFO] [phase: shadow-validation] fixture=${fixture_id} deadline=${P041_SHADOW_DEADLINE_SECONDS}s"
        p041_update_current_step 'shadow_validation' '' &&
        p041_update_lease_heartbeat &&
        P041_ONLY_FIXTURE="$fixture_id" P041_STEP_FIXTURE="$fixture_id" P041_COMMAND_DEADLINE_SECONDS="$P041_SHADOW_DEADLINE_SECONDS" \
          p041_supervised_run cargo test -p engine --test proposal_041_parity proposal_041_shadow_side_effect_policy_is_fail_closed -- --exact --nocapture || exit $?
      done &&
      echo "[INFO] [phase: graphql-readback] reading back all 7 fixtures through GraphQL parity surface" &&
      for fixture_id in proposal-loop-basic implementation-refine-review approval-pause-resume retry-recovery-flow cancelled-or-blocked-run terminal-report-evidence projection-readback-surface; do
        echo "[INFO] [phase: graphql-readback] fixture=${fixture_id} deadline=${P041_READBACK_DEADLINE_SECONDS}s"
        p041_update_current_step 'graphql_readback' 'graphql_readback' &&
        p041_update_lease_heartbeat &&
        P041_ONLY_FIXTURE="$fixture_id" P041_STEP_FIXTURE="$fixture_id" P041_STEP_SURFACE="graphql_readback" P041_COMMAND_DEADLINE_SECONDS="$P041_READBACK_DEADLINE_SECONDS" \
          p041_supervised_run cargo test -p graphql-server --lib proposal_041_graphql_readback_parity_surfaces -- --nocapture || exit $?
      done &&
      echo "[INFO] [phase: mcp-readback] reading back all 7 fixtures through MCP parity surfaces" &&
      for fixture_id in proposal-loop-basic implementation-refine-review approval-pause-resume retry-recovery-flow cancelled-or-blocked-run terminal-report-evidence projection-readback-surface; do
        echo "[INFO] [phase: mcp-readback] fixture=${fixture_id} deadline=${P041_READBACK_DEADLINE_SECONDS}s"
        p041_update_current_step 'mcp_report_readback' 'mcp_report_readback' &&
        p041_update_lease_heartbeat &&
        P041_ONLY_FIXTURE="$fixture_id" P041_STEP_FIXTURE="$fixture_id" P041_STEP_SURFACE="mcp_report_readback" P041_COMMAND_DEADLINE_SECONDS="$P041_READBACK_DEADLINE_SECONDS" \
          p041_supervised_run cargo test -p mcp-server --lib proposal_041_report_resource_readback_parity_surface -- --nocapture || exit $?
      done &&
      echo "[INFO] [phase: handoff-artifact] validating P031 handoff artifact contract" &&
      p041_update_lease_heartbeat &&
      p041_supervised_run cargo test -p engine --test proposal_041_parity proposal_041_handoff_artifact_contract_is_ready -- --exact --nocapture &&
      echo "[INFO] [phase: runtime-publication] computing and writing final runtime row and detail artifacts" &&
      p041_update_current_step 'runtime_publication' '' &&
      p041_update_lease_heartbeat &&
      p041_supervised_run cargo test -p engine --test proposal_041_parity proposal_041_runtime_publication_contract_is_valid -- --exact --nocapture &&
      p041_prefixed_run python3 - <<'PY'
import json
import hashlib
import os
import shutil
import subprocess
import sys
import tempfile
import time
from pathlib import Path

fixtures = [
    "proposal-loop-basic",
    "implementation-refine-review",
    "approval-pause-resume",
    "retry-recovery-flow",
    "cancelled-or-blocked-run",
    "terminal-report-evidence",
    "projection-readback-surface",
]
generation_id = os.environ.get("P041_PUBLICATION_GENERATION_ID", "unscoped-fixture-replay")

# ── Darwin F_FULLFSYNC helper (duplicate of setup block to keep this block self-contained) ──
def _darwin_fullfsync(fd: int) -> bool:
    try:
        import ctypes, ctypes.util
        libc = ctypes.CDLL(ctypes.util.find_library("c"), use_errno=True)
        return libc.fcntl(fd, 51) == 0  # F_FULLFSYNC = 51
    except Exception:
        return False

def _validate_p041_final_target_boundary(path: Path, label: str) -> None:
    target = Path("target")
    anchor = target.resolve()
    if target.is_symlink():
        raise SystemExit(
            "proposal-041: target is a symlink; refusing to prune or publish "
            "through a redirected root (SEC-P041-003)."
        )
    if path.is_symlink():
        raise SystemExit(
            f"proposal-041: {label} is a symlink; refusing to prune or publish "
            "through a redirected root (SEC-P041-003)."
        )
    resolved = path.resolve()
    if not str(resolved).startswith(str(anchor)):
        raise SystemExit(
            f"proposal-041: {label} resolves outside control-plane/target/; "
            "refusing to prune or publish (SEC-P041-003)."
        )

def atomic_json(path, value):
    """Write value as JSON via same-dir temp + durable flush + atomic rename."""
    _validate_p041_final_target_boundary(path.parent, f"{path.parent}")
    path.parent.mkdir(parents=True, exist_ok=True)
    fd, tmp = tempfile.mkstemp(prefix=f".{path.name}.", suffix=".tmp", dir=path.parent)
    try:
        with os.fdopen(fd, "w") as handle:
            json.dump(value, handle, indent=2)
            handle.write("\n")
            handle.flush()
            raw_fd = handle.fileno()
            if sys.platform == "darwin":
                if not _darwin_fullfsync(raw_fd):
                    os.fsync(raw_fd)
            else:
                os.fsync(raw_fd)
        os.replace(tmp, path)
        try:
            dir_fd = os.open(str(path.parent), os.O_RDONLY)
            try:
                os.fsync(dir_fd)
            finally:
                os.close(dir_fd)
        except OSError:
            pass
    finally:
        try:
            if os.path.exists(tmp):
                os.unlink(tmp)
        except OSError:
            pass

# ── Retention, pruning, and 500MB budget check (proposal Section 6.4) ──────────────────────
# Read current reclaim marker to determine if last run parked in blocked_manual_recovery.
control_root = Path("target/parity-control")
reclaim_marker_path = control_root / "reclaim-marker.json"
last_reclaim_status = None
if reclaim_marker_path.exists():
    try:
        _rm = json.loads(reclaim_marker_path.read_text())
        last_reclaim_status = _rm.get("overall_status")
    except (json.JSONDecodeError, OSError):
        pass

def _remove_abandoned_sqlite_triple(sqlite_path: Path) -> None:
    """Remove a parity.sqlite together with .sqlite-wal and .sqlite-shm (proposal Section 6.3)."""
    for suffix in ("", "-wal", "-shm"):
        p = sqlite_path.parent / (sqlite_path.name + suffix)
        try:
            p.unlink()
        except FileNotFoundError:
            pass

parity_root = Path("target/parity")
work_root = parity_root / "work"
shadow_root = parity_root / "shadow"
reports_root = parity_root / "reports"
generations_root = parity_root / "publication/generations"
control_root = Path("target/parity-control")

# Symlink boundary check before pruning (SEC-P041-003): same guard as setup block.
_target_anchor = Path("target").resolve()
for _prune_root, _prune_label in (
    (parity_root, "target/parity"),
    (control_root, "target/parity-control"),
):
    if _prune_root.is_symlink():
        raise SystemExit(
            f"proposal-041: {_prune_label} is a symlink; refusing to prune or publish "
            f"through a redirected root (SEC-P041-003)."
        )
    if _prune_root.exists():
        _resolved_prune = _prune_root.resolve()
        if not str(_resolved_prune).startswith(str(_target_anchor)):
            raise SystemExit(
                f"proposal-041: {_prune_label} resolves outside control-plane/target/; "
                f"refusing to prune or publish (SEC-P041-003)."
            )

def _dir_size_bytes(d: Path) -> int:
    total = 0
    try:
        for p in d.rglob("*"):
            if p.is_file():
                try:
                    total += p.stat().st_size
                except OSError:
                    pass
    except OSError:
        pass
    return total

# Enumerate known generation IDs from work, shadow, reports, publication/generations.
known_generations: set[str] = set()
for root_dir in (work_root, shadow_root, reports_root, generations_root):
    if root_dir.is_dir():
        for entry in root_dir.iterdir():
            if entry.is_dir():
                known_generations.add(entry.name)

# Collect per-generation info for pruning decisions.
ready_generations = []
blocked_manual_recovery_generations = []
other_blocked_generations = []

for gen_id in sorted(known_generations):
    if gen_id == generation_id:
        continue  # Never prune the current active generation.
    gen_detail_path = generations_root / gen_id / "p031-p041-parity-evidence.json"
    gen_status = None
    if gen_detail_path.exists():
        try:
            _gd = json.loads(gen_detail_path.read_text())
            gen_status = _gd.get("overall_status")
        except (json.JSONDecodeError, OSError):
            pass
    if gen_status == "ready_same_tree_verified":
        ready_generations.append(gen_id)
    elif gen_status == "blocked_manual_recovery":
        blocked_manual_recovery_generations.append(gen_id)
    else:
        other_blocked_generations.append(gen_id)

# Apply pruning policy (Section 6.4):
# - Never prune blocked_manual_recovery generations.
# - Retain the newest ready generation.
# - Retain the newest non-manual blocked diagnostic generation.
# - Prune older generations (oldest-first) if there is a newer one.
generations_pruned = []
ready_generations_sorted = sorted(ready_generations)
other_blocked_sorted = sorted(other_blocked_generations)

# Prune older ready generations (keep the newest only).
for gen_id in ready_generations_sorted[:-1] if len(ready_generations_sorted) > 1 else []:
    for sub_root in (work_root, shadow_root, reports_root):
        gen_dir = sub_root / gen_id
        if gen_dir.is_dir():
            # Remove abandoned SQLite triples before rmtree.
            for sqlite_file in gen_dir.rglob("parity.sqlite"):
                _remove_abandoned_sqlite_triple(sqlite_file)
            try:
                shutil.rmtree(gen_dir)
            except OSError as e:
                print(f"[WARN] proposal-041: unable to prune {gen_dir}: {e}")
    gen_pub_dir = generations_root / gen_id
    if gen_pub_dir.is_dir():
        try:
            shutil.rmtree(gen_pub_dir)
        except OSError as e:
            print(f"[WARN] proposal-041: unable to prune publication dir {gen_pub_dir}: {e}")
    generations_pruned.append(gen_id)

# Prune older non-manual blocked generations (keep the newest only).
for gen_id in other_blocked_sorted[:-1] if len(other_blocked_sorted) > 1 else []:
    for sub_root in (work_root, shadow_root, reports_root):
        gen_dir = sub_root / gen_id
        if gen_dir.is_dir():
            for sqlite_file in gen_dir.rglob("parity.sqlite"):
                _remove_abandoned_sqlite_triple(sqlite_file)
            try:
                shutil.rmtree(gen_dir)
            except OSError as e:
                print(f"[WARN] proposal-041: unable to prune {gen_dir}: {e}")
    gen_pub_dir = generations_root / gen_id
    if gen_pub_dir.is_dir():
        try:
            shutil.rmtree(gen_pub_dir)
        except OSError as e:
            print(f"[WARN] proposal-041: unable to prune publication dir {gen_pub_dir}: {e}")
    generations_pruned.append(gen_id)

if generations_pruned:
    print(f"[INFO] proposal-041: pruned {len(generations_pruned)} eligible abandoned generation(s): {generations_pruned}")
if blocked_manual_recovery_generations:
    print(
        f"[WARN] proposal-041: {len(blocked_manual_recovery_generations)} blocked_manual_recovery "
        f"generation(s) preserved (never auto-pruned): {blocked_manual_recovery_generations}"
    )

# 500 MB storage budget check (warn, not a hard failure per Section 6.4).
_BUDGET_BYTES = 500 * 1024 * 1024
_total_parity_bytes = _dir_size_bytes(parity_root)
if _total_parity_bytes > _BUDGET_BYTES:
    _mb = _total_parity_bytes / (1024 * 1024)
    print(f"[WARN] proposal-041: parity artifact storage {_mb:.1f} MB exceeds 500 MB budget")
    print(f"[WARN] proposal-041: preserved generation roots:")
    for gen_id in sorted(known_generations):
        gen_work = work_root / gen_id
        gen_shadow = shadow_root / gen_id
        gen_reports = reports_root / gen_id
        gen_total = sum(_dir_size_bytes(d) for d in (gen_work, gen_shadow, gen_reports) if d.is_dir())
        if gen_total > 0:
            print(f"[WARN]   generation={gen_id} size={gen_total / (1024*1024):.1f} MB")
for _fix_idx, fixture_id in enumerate(fixtures, 1):
    print(f"[INFO] [{_fix_idx}/7] parity {fixture_id}")
    report_path = Path("target/parity/reports") / generation_id / fixture_id / "behavioral-diff-report.json"
    replay_path = Path("target/parity/work") / generation_id / fixture_id / "server-replay.json"
    if not report_path.is_file():
        print(
            f"[FAIL] [{_fix_idx}/7] missing-evidence {fixture_id}"
            f" missing_path=control-plane/{report_path}"
            f" expected_producer=replay"
            f" affected_fixture_or_surface={fixture_id}"
            f" next_action=rerun-after-replay-restored"
        )
        raise SystemExit(f"proposal-041: missing generated report {report_path}")
    if not replay_path.is_file():
        print(
            f"[FAIL] [{_fix_idx}/7] missing-evidence {fixture_id}"
            f" missing_path=control-plane/{replay_path}"
            f" expected_producer=replay"
            f" affected_fixture_or_surface={fixture_id}"
            f" next_action=rerun-after-replay-restored"
        )
        raise SystemExit(f"proposal-041: missing generated server replay {replay_path}")
    report = json.loads(report_path.read_text())
    replay = json.loads(replay_path.read_text())
    if report.get("schema_version") != "behavioral-diff-report.v1":
        raise SystemExit(f"proposal-041: bad schema_version in {report_path}")
    if replay.get("schema_version") != "server-replay.v1":
        raise SystemExit(f"proposal-041: bad schema_version in {replay_path}")
    if report.get("run_fixture_id") != fixture_id:
        raise SystemExit(f"proposal-041: fixture mismatch in {report_path}")
    if replay.get("fixture_id") != fixture_id:
        raise SystemExit(f"proposal-041: fixture mismatch in {replay_path}")
    expected_replay_ref = f"control-plane/target/parity/work/{generation_id}/{fixture_id}/server-replay.json"
    if report.get("server_replay_ref") != expected_replay_ref:
        raise SystemExit(f"proposal-041: bad server_replay_ref in {report_path}")
    if report.get("verdict") != "ready":
        raise SystemExit(f"proposal-041: non-ready verdict in {report_path}")
    if report.get("summary", {}).get("blocking_count") != 0:
        raise SystemExit(f"proposal-041: blocking divergences in {report_path}")
    required_surfaces = {
        "canonical_domain_state",
        "projections",
        "graphql_readback",
        "mcp_report_readback",
        "artifact_identity",
        "operator_summary",
    }
    compared = {
        item.get("surface")
        for item in report.get("surface_comparisons", [])
        if item.get("status") == "matched"
    }
    if compared != required_surfaces:
        raise SystemExit(
            f"proposal-041: fixture-bound surface comparisons incomplete in {report_path}: {sorted(compared)}"
        )
    comparison_by_surface = {
        item.get("surface"): item
        for item in report.get("surface_comparisons", [])
    }
    if "graphql-server::schema::build_schema" not in json.dumps(comparison_by_surface["graphql_readback"].get("actual", {})):
        raise SystemExit(f"proposal-041: graphql_readback was not collected through GraphQL owner in {report_path}")
    if "mcp-server::tools::reports::execute" not in json.dumps(comparison_by_surface["mcp_report_readback"].get("actual", {})):
        raise SystemExit(f"proposal-041: mcp_report_readback was not collected through MCP owner in {report_path}")
    executable_inputs = report.get("executable_inputs", {})
    for key in (
        "frozen_workflow_snapshot_ref",
        "frozen_agent_catalog_snapshot_ref",
        "provider_profile_ref",
        "runtime_events_ref",
        "operator_decisions_ref",
    ):
        if not executable_inputs.get(key):
            raise SystemExit(f"proposal-041: missing executable input {key} in {report_path}")
    shadow_report_path = Path("target/parity/shadow") / generation_id / fixture_id / "live-shadow-report.json"
    if not shadow_report_path.is_file():
        raise SystemExit(f"proposal-041: missing live shadow report {shadow_report_path}")
    shadow_report = json.loads(shadow_report_path.read_text())
    if shadow_report.get("schema_version") != "live-shadow-report.v1":
        raise SystemExit(f"proposal-041: bad live shadow schema_version in {shadow_report_path}")
    if shadow_report.get("mode") != "live_shadow":
        raise SystemExit(f"proposal-041: bad shadow mode in {shadow_report_path}")
    if shadow_report.get("shadow_contract", {}).get("settles_production_stages") is not False:
        raise SystemExit(f"proposal-041: shadow report does not prove non-settlement in {shadow_report_path}")
    shadow = shadow_report.get("shadow_contract", {})
    for key in ("source_run_id", "shadow_run_id", "fixture_or_capture_id", "idempotency_key"):
        if not shadow.get(key):
            raise SystemExit(f"proposal-041: shadow report missing correlation key {key} in {shadow_report_path}")
    if shadow.get("fixture_or_capture_id") != fixture_id:
        raise SystemExit(f"proposal-041: shadow report fixture correlation mismatch in {shadow_report_path}")
    print(f"[PASS] [{_fix_idx}/7] parity {fixture_id}")

# ── Summary grid (Section 5.1): 7×6 fixture-by-surface matrix ───────────────
# Build grid from surface_comparisons in each behavioral-diff-report.json.
SURFACE_ORDER = [
    "canonical_domain_state", "projections", "graphql_readback",
    "mcp_report_readback", "artifact_identity", "operator_summary",
]
# status → grid token (Section 5 table)
_STATUS_TOKEN = {"matched": "PASS", "diverged": "FAIL", "timed_out": "TIMEOUT", "missing_evidence": "MISS"}

grid = {}
for _fid in fixtures:
    grid[_fid] = {}
    _rp = Path("target/parity/reports") / generation_id / _fid / "behavioral-diff-report.json"
    if _rp.is_file():
        _r = json.loads(_rp.read_text())
        for _cmp in _r.get("surface_comparisons", []):
            _surf = _cmp.get("surface", "")
            _grid_tok = _STATUS_TOKEN.get(_cmp.get("status", ""), "FAIL")
            if _surf:
                grid[_fid][_surf] = _grid_tok
    # Surfaces absent from surface_comparisons → MISS
    for _surf in SURFACE_ORDER:
        grid[_fid].setdefault(_surf, "MISS")

import shutil as _shutil
_term_w = _shutil.get_terminal_size((80, 24)).columns
_CELL = 9  # each cell is 9 chars wide including trailing space (fits "TIMEOUT  ")
_max_fix_len = max(len(f) for f in fixtures)
_wide_needed = _max_fix_len + 2 + _CELL * len(SURFACE_ORDER)

if _term_w >= _wide_needed:
    # Wide grid — full fixture ids, 9-char cells
    _surf_abbr = ["canonical", "projection", "graphql", "mcp", "artifact", "summary"]
    _hdr = f"{'fixture':<{_max_fix_len}}  " + "".join(f"{a:<{_CELL}}" for a in _surf_abbr)
    print(f"[INFO] {_hdr}")
    for _fid in fixtures:
        _row = f"{_fid:<{_max_fix_len}}  " + "".join(
            f"{grid[_fid].get(s, 'MISS'):<{_CELL}}" for s in SURFACE_ORDER
        )
        print(f"[INFO] {_row}")
else:
    # Narrow-terminal fallback: two lines per fixture, single-char tokens
    _abbrev = {
        "canonical_domain_state": "canon", "projections": "proj",
        "graphql_readback": "gql", "mcp_report_readback": "mcp",
        "artifact_identity": "art", "operator_summary": "sum",
    }
    _TOKEN = {"PASS": "P", "FAIL": "F", "MISS": "M", "SKIP": "S", "TIMEOUT": "T"}
    for _fid in fixtures:
        print(f"[INFO] {_fid}")
        _g = grid[_fid]
        _s1 = SURFACE_ORDER[:3]
        _s2 = SURFACE_ORDER[3:]
        print("[INFO]   " + " ".join(f"{_abbrev[s]}={_TOKEN.get(_g.get(s,'MISS'),'?')}" for s in _s1))
        print("[INFO]   " + " ".join(f"{_abbrev[s]}={_TOKEN.get(_g.get(s,'MISS'),'?')}" for s in _s2))
    print("[INFO] Legend: P=PASS F=FAIL")
    print("[INFO] Legend: M=MISS S=SKIP T=TIMEOUT")

# Compute per-fixture pass/fail counts for the final summary line.
passed_count = sum(
    1 for _fid in fixtures
    if all(grid[_fid].get(s, "MISS") == "PASS" for s in SURFACE_ORDER)
)
failed_count = sum(
    1 for _fid in fixtures
    if any(grid[_fid].get(s, "MISS") not in ("PASS", "TIMEOUT") for s in SURFACE_ORDER)
)
missing_count = sum(
    1 for _fid in fixtures
    if any(grid[_fid].get(s, "MISS") == "MISS" for s in SURFACE_ORDER)
)

# Validate runtime row and detail artifacts (Phase C P031 cutover acceptance)
row_path = Path("target/parity/publication/current/p031-phase-0-manifest-row.json")
detail_path = Path("target/parity/publication/current/p031-p041-parity-evidence.json")
if not row_path.is_file():
    raise SystemExit(f"proposal-041: runtime row missing at {row_path}")
if not detail_path.is_file():
    raise SystemExit(f"proposal-041: runtime detail missing at {detail_path}")

row = json.loads(row_path.read_text())
detail = json.loads(detail_path.read_text())

if row.get("schema_version") != "p031-phase-0-runtime-manifest-row.v1":
    raise SystemExit(f"proposal-041: bad row schema_version: {row.get('schema_version')}")
if detail.get("schema_version") != "p031-p041-parity-evidence.v1":
    raise SystemExit(f"proposal-041: bad detail schema_version: {detail.get('schema_version')}")
if row.get("id") != "p041_parity_evidence":
    raise SystemExit(f"proposal-041: row.id must be p041_parity_evidence, got {row.get('id')}")
if row.get("detail_schema_version") != detail.get("schema_version"):
    raise SystemExit(
        f"proposal-041: row.detail_schema_version {row.get('detail_schema_version')} "
        f"!= detail.schema_version {detail.get('schema_version')}"
    )
if row.get("validation_status") != detail.get("overall_status"):
    raise SystemExit(
        f"proposal-041: row.validation_status {row.get('validation_status')} "
        f"!= detail.overall_status {detail.get('overall_status')}"
    )
if row.get("publication_state") != detail.get("publication_state"):
    raise SystemExit("proposal-041: row.publication_state != detail.publication_state")
if row.get("publication_generation_id") != detail.get("publication_generation_id"):
    raise SystemExit("proposal-041: row.publication_generation_id != detail.publication_generation_id")

required_fixtures = [
    "proposal-loop-basic", "implementation-refine-review", "approval-pause-resume",
    "retry-recovery-flow", "cancelled-or-blocked-run", "terminal-report-evidence",
    "projection-readback-surface",
]
required_surfaces = [
    "canonical_domain_state", "projections", "graphql_readback",
    "mcp_report_readback", "artifact_identity", "operator_summary",
]
if detail.get("required_fixtures") != required_fixtures:
    raise SystemExit(f"proposal-041: detail.required_fixtures mismatch")
if detail.get("required_surfaces") != required_surfaces:
    raise SystemExit(f"proposal-041: detail.required_surfaces mismatch")

# Live-checkout comparison: required when claiming ready_same_tree_verified.
# Blocked artifacts (including schema-validation-only test artifacts) do not
# require checkout comparison — only ready publication must match the live tree.
# No sentinel bypass: any row claiming ready_same_tree_verified must carry real
# provenance that matches the live HEAD or the gate fails closed.
if row.get("validation_status") == "ready_same_tree_verified":
    def _git_output(args, label):
        try:
            return subprocess.check_output(
                args, stderr=subprocess.DEVNULL, text=True
            )
        except (subprocess.CalledProcessError, FileNotFoundError):
            raise SystemExit(
                f"proposal-041: ready_same_tree_verified requires live git {label}"
            )

    live_commit = _git_output(["git", "rev-parse", "HEAD"], "HEAD").strip()
    live_tree = _git_output(["git", "rev-parse", "HEAD^{tree}"], "HEAD^{tree}").strip()
    live_status = _git_output(
        ["git", "status", "--porcelain=v1", "--untracked-files=all"],
        "status snapshot",
    )
    live_line_count = sum(1 for line in live_status.splitlines() if line)
    live_sha256 = hashlib.sha256(live_status.encode()).hexdigest()
    prov = row.get("provenance", {})
    if not isinstance(prov, dict):
        raise SystemExit("proposal-041: row.provenance must be an object")
    row_commit = prov.get("commit_sha", "")
    row_tree = prov.get("tree_id", "")
    row_status_sha256 = prov.get("status_snapshot_sha256", "")
    for field, value in (
        ("commit_sha", row_commit),
        ("tree_id", row_tree),
        ("status_snapshot_sha256", row_status_sha256),
    ):
        if not isinstance(value, str) or not value.strip():
            raise SystemExit(
                f"proposal-041: row.provenance.{field} is required for "
                "ready_same_tree_verified"
            )
    if row_commit != live_commit:
        raise SystemExit(
            f"proposal-041: runtime row commit_sha {row_commit[:12]} does not match "
            f"live HEAD {live_commit[:12]}; rerun the gate on the same clean tree"
        )
    if row_tree != live_tree:
        raise SystemExit(
            f"proposal-041: runtime row tree_id {row_tree[:12]} does not match "
            f"live HEAD^{{tree}} {live_tree[:12]}; rerun the gate on the same clean tree"
        )
    prov_line_count = prov.get("status_snapshot_line_count", 1)
    if not prov.get("tree_clean") or prov_line_count != 0:
        raise SystemExit(
            f"proposal-041: ready_same_tree_verified requires tree_clean=true and "
            f"status_snapshot_line_count=0; got tree_clean={prov.get('tree_clean')} "
            f"line_count={prov_line_count}"
        )
    if live_line_count != 0:
        raise SystemExit("proposal-041: ready_same_tree_verified requires clean live git status")
    if prov_line_count != live_line_count:
        raise SystemExit(
            f"proposal-041: row.provenance.status_snapshot_line_count {prov_line_count} "
            f"does not match live git status line count {live_line_count}"
        )
    if row_status_sha256 != live_sha256:
        raise SystemExit(
            "proposal-041: row.provenance.status_snapshot_sha256 does not match live git status"
        )

# Row vs detail provenance agreement (Section 6.2): required for ready publication.
if row.get("validation_status") == "ready_same_tree_verified":
    row_prov = row.get("provenance", {})
    detail_prov = detail.get("provenance", {})
    for prov_field in (
        "commit_sha", "tree_id", "tree_clean",
        "status_snapshot_sha256", "status_snapshot_line_count",
    ):
        rv, dv = row_prov.get(prov_field), detail_prov.get(prov_field)
        if rv != dv:
            raise SystemExit(
                f"proposal-041: row.provenance.{prov_field} ({rv!r}) != "
                f"detail.provenance.{prov_field} ({dv!r}); "
                "row and detail must agree on all provenance fields for ready publication"
            )

# Explicit detail.provenance vs live checkout (Section 6.6 Decision 4).
# Checking row.provenance against live is not sufficient — a stale detail that
# agrees with row on status/state/generation could still carry stale provenance.
if row.get("validation_status") == "ready_same_tree_verified":
    detail_prov = detail.get("provenance", {})
    if not isinstance(detail_prov, dict):
        raise SystemExit("proposal-041: detail.provenance must be an object")
    try:
        _detail_live_commit = live_commit
        _detail_live_tree = live_tree
        _detail_live_line_count = live_line_count
        _detail_live_sha256 = live_sha256
    except NameError:
        raise SystemExit("proposal-041: ready_same_tree_verified requires live git provenance")
    detail_commit = detail_prov.get("commit_sha", "")
    detail_tree = detail_prov.get("tree_id", "")
    detail_status_sha256 = detail_prov.get("status_snapshot_sha256", "")
    for field, value in (
        ("commit_sha", detail_commit),
        ("tree_id", detail_tree),
        ("status_snapshot_sha256", detail_status_sha256),
    ):
        if not isinstance(value, str) or not value.strip():
            raise SystemExit(
                f"proposal-041: detail.provenance.{field} is required for "
                "ready_same_tree_verified"
            )
    if detail_commit != _detail_live_commit:
        raise SystemExit(
            f"proposal-041: detail.provenance.commit_sha "
            f"{detail_commit[:12]} does not match "
            f"live HEAD {_detail_live_commit[:12]}; rerun the gate on the same clean tree"
        )
    if detail_tree != _detail_live_tree:
        raise SystemExit(
            f"proposal-041: detail.provenance.tree_id "
            f"{detail_tree[:12]} does not match "
            f"live HEAD^{{tree}} {_detail_live_tree[:12]}; rerun the gate on the same clean tree"
        )
    if detail_prov.get("status_snapshot_line_count") != _detail_live_line_count:
        raise SystemExit(
            "proposal-041: detail.provenance.status_snapshot_line_count "
            "does not match live git status"
        )
    if detail_status_sha256 != _detail_live_sha256:
        raise SystemExit(
            "proposal-041: detail.provenance.status_snapshot_sha256 "
            "does not match live git status"
        )

# Status-to-prefix mapping per proposal Section 5 table.
STATUS_TO_PREFIX = {
    "ready_same_tree_verified": "PASS",
    "blocked_missing_evidence": "FAIL",
    "blocked_divergence": "FAIL",
    "blocked_manual_recovery": "FAIL",
    "blocked_dirty_tree": "WARN",
    "blocked_timeout": "WARN",
    "blocked_interrupted": "WARN",
    "blocked_in_progress": "INFO",
}
validation_status = row.get("validation_status", "")
prefix = STATUS_TO_PREFIX.get(validation_status, "FAIL")

now_ms = int(time.time() * 1000)
# control_root is already defined above in the pruning section.
atomic_json(control_root / "current-step.json", {
    "schema_version": "parity-control-current-step.v1",
    "generation": generation_id,
    "fixture": None,
    "step": "final_summary",
    "surface": None,
    "mode": "gate",
    "elapsed_ms": 0,
    "heartbeat_unix_ms": now_ms,
})
atomic_json(control_root / "release-marker.json", {
    "schema_version": "parity-control-release-marker.v1",
    "overall_status": validation_status,
    "generation_id": generation_id,
    # True: all cargo phases ran synchronously via bash && chaining (p041_prefixed_run
    # waits for each process). No background subprocesses were spawned independently.
    # When this block executes, all cargo test invocations have already returned.
    # Descendant quiescence is therefore proven by the synchronous execution model.
    "descendant_quiescent": True,
    "written_at_unix_ms": now_ms,
})

# Print blocking diagnostics from the runtime detail artifact (Section 5.1 / 6.2).
# Operators see these before the final status line so they can act without opening
# the JSON file. blocking_reasons and missing_evidence are populated by the
# runtime publication test for every non-ready publication.
if validation_status != "ready_same_tree_verified":
    for reason in detail.get("blocking_reasons", []):
        print(f"[{prefix}] blocking_reason={reason}")
    for item in detail.get("missing_evidence", []):
        if isinstance(item, dict):
            print(
                f"[{prefix}] missing_evidence"
                f" missing_path={item.get('missing_path', 'unknown')}"
                f" expected_producer={item.get('expected_producer', 'unknown')}"
                f" affected_fixture_or_surface={item.get('affected_fixture_or_surface', 'unknown')}"
                f" next_action={item.get('next_action', 'unknown')}"
            )
        else:
            print(f"[{prefix}] missing_evidence={item}")

# The markdown companion is not generated in the current implementation.
# Per Section 5.1: when the markdown companion is absent, the final footer
# must say "JSON-only evidence" before printing the detail= path.
print(f"[{prefix}] status={validation_status} passed_fixtures={passed_count} failed_fixtures={failed_count} missing_evidence={missing_count}")
print(f"[INFO] JSON-only evidence")
print(f"[INFO] row={row_path}")
print(f"[INFO] detail={detail_path}")
if validation_status != "ready_same_tree_verified":
    raise SystemExit(
        f"proposal-041: gate status is {validation_status}, not ready_same_tree_verified; "
        f"inspect runtime detail at {detail_path}"
    )
PY
    )
    echo "[PASS] Proposal 041 control-plane gate: ready_same_tree_verified"
    ;;
  proposal-043|p043)
    log "Proposal 043 control-plane gate: GraphQL projection read contract"
    (
      cd "$ROOT_DIR"
      cd control-plane
      CARGO_TARGET_DIR=target/proposal-043-gate cargo test -p graphql-server --lib proposal_043_ -- --test-threads=1 --nocapture
      cd "$ROOT_DIR"
      python3 - <<'PY'
import re
from pathlib import Path

artifact = Path("docs/reference/query-projections-and-client-consumption-contract.md")
if not artifact.is_file():
    raise SystemExit(f"proposal-043: missing reference contract {artifact}")

text = artifact.read_text()

def require_contains(needle, label=None):
    if needle not in text:
        raise SystemExit(f"proposal-043: reference contract missing {label or needle}")

def require_row(cells, label):
    row = "| " + " | ".join(cells) + " |"
    if row not in text:
        raise SystemExit(f"proposal-043: reference contract missing row for {label}: {row}")

require_row(["Implementation status", "Implemented"], "implementation status")
require_row(["Readiness", "Ready with Risks"], "readiness")

require_row(["Contract schema", "`p043-read-contract-v1`"], "contract schema")
require_row(["Gate", "`./scripts/test-gate.sh proposal-043`"], "canonical gate")
require_row(["Alias", "`./scripts/test-gate.sh p043`"], "gate alias")

matrix_rows = {
    "Runs home": "Implemented",
    "Run detail": "Implemented",
    "Stage list / progress": "Implemented",
    "Stage detail": "Implemented",
    "Approval inbox": "Implemented",
    "Artifact viewer": "Implemented",
    "Report viewer": "Partial",
    "Runtime health": "Deferred",
    "Experiment comparison": "Deferred",
}
for surface, status in matrix_rows.items():
    pattern = re.compile(rf"^\| {re.escape(surface)} \| .* \| {re.escape(status)} \| .* \|$", re.M)
    if not pattern.search(text):
        raise SystemExit(f"proposal-043: matrix row for {surface} must exist with status {status}")

budget_rows = {
    "Initial read timeout": ("5 seconds", "unavailable"),
    "Command-completion refresh timeout": ("3 seconds", "stale"),
    "Foreground/reconnect refresh timeout": ("5 seconds", "refreshing"),
    "Projection-lag grace window": ("2 seconds", "projection_lag"),
    "Subscription disconnect grace window": ("10 seconds", "refreshing_disconnected"),
    "Bounded polling interval without subscription": ("5 seconds", "visible surfaces"),
    "Bounded polling backoff": ("5s, 10s, 20s, then 30s max", "fail closed"),
    "Stale/action-safety disable threshold": ("immediate", "Disable destructive/state-changing controls"),
    "Cutover rollback threshold": ("3 consecutive command-completion refresh timeouts or 2 minutes continuous `unavailable`", "Hold or roll back"),
}
for budget, (value, behavior_fragment) in budget_rows.items():
    pattern = re.compile(rf"^\| {re.escape(budget)} \| {re.escape(value)} \| .*{re.escape(behavior_fragment)}.* \|$", re.M)
    if not pattern.search(text):
        raise SystemExit(f"proposal-043: freshness budget row invalid or missing for {budget}")

for behavior in [
    "Initial query failure to `unavailable` or `stale`",
    "Command-completion refresh timeout to `stale`",
    "Foreground/reconnect refresh timeout",
    "Projection lag action safety",
    "Subscription disconnect action safety",
    "Bounded polling fallback",
    "Unauthorized read behavior",
    "Stale/action-safety disable threshold",
]:
    require_contains(behavior, f"freshness behavior {behavior}")

for freshness in [
    "Projection freshness fields",
    "`GqlRun` | `projectionPresent`, `projectionUpdatedAt`, `projectionLag`",
    "`GqlStageExecution` | `projectionPresent`, `projectionUpdatedAt`, `projectionLag`",
    "proposal_043_missing_projection_rows_are_explicit_lag_state",
    "projectionPresent=false",
    "projectionLag=true",
]:
    require_contains(freshness, f"projection freshness {freshness}")

for subscription in [
    "`runStatusChanged(runId:)`",
    "`stageStatusChanged(runId:)`",
    "`approvalRequested`",
    "`approvalResolved`",
    "`runtimeStatusChanged`",
]:
    require_contains(subscription, f"subscription posture {subscription}")

for proof in [
    "Run status subscription",
    "Stage status subscription",
    "Approval resolved subscription",
    "Missing projection rows",
    "Sufficient for P031 event patching",
]:
    require_contains(proof, f"GraphQL field proof {proof}")

for phrase in [
    "operator-only V1",
    "Projection parity",
    "Known holds",
    "Governed macOS UI surfaces ship",
    # Scope narrowing: P031 is a read-only consumer. It does not issue
    # MCP mutations, so rows that reference disabled controls apply to a
    # future command-UI consumer, not to P031.
    "read-only consumer",
    "Scope boundary:",
]:
    require_contains(phrase, phrase)

for forbidden in [
    "pending_external_readback",
    "local transcript files as a workaround",
]:
    if forbidden in text:
        raise SystemExit(f"proposal-043: reference contract contains forbidden stale placeholder: {forbidden}")
PY
    )
    log "Proposal 043 control-plane gate passed"
    ;;
  proposal-031|p031)
    log "Thin UI gate: GraphQL-only inventory/static guard/write-path guide"
    "$0" proposal-043

    python3 "$ROOT_DIR/scripts/p031-thin-ui-gate.py" --repo-root "$ROOT_DIR"

    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-031-gate cargo test -p graphql-server --lib proposal_031_ -- --test-threads=1 --nocapture
      CARGO_TARGET_DIR=target/proposal-031-gate cargo test -p graphql-server --test proposal_031_authorization -- --test-threads=1 --nocapture
    )

    (
      cd "$ROOT_DIR"
      python3 - <<'PY'
from pathlib import Path
import json

def require_file(path):
    p = Path(path)
    if not p.is_file():
        raise SystemExit(f"proposal-031: missing required artifact {path}")

require_file("docs/reference/p031-thin-ui-inventory.json")
require_file("docs/reference/p031-operator-write-path-guide.json")
require_file("docs/reference/p031-phase-0-artifact-manifest.json")

manifest_path = Path("docs/reference/p031-phase-0-artifact-manifest.json")
manifest = json.loads(manifest_path.read_text())
for entry in manifest.get("entries", []):
    require_file(entry["path"])
PY
    )
    log "Thin UI gate passed"
    ;;
  proposal-072|p072)
    log "UI action boundary gate: approval-only GraphQL UI mutation boundary"
    "$0" proposal-031

    run_targeted_tests "proposal-072-swift" \
      "Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests"

    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p domain operator_action_routing -- --nocapture
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p auth v2_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p auth is_mutation_allowed_by_surface_policy_checks -- --nocapture
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p graphql-server approve_approval -- --test-threads=1 --nocapture
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p graphql-server reject_approval -- --test-threads=1 --nocapture
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p graphql-server ui_principals_denied_non_approval_mutations -- --test-threads=1 --nocapture
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p graphql-server legacy_default_operator_denied_non_approval_mutations -- --test-threads=1 --nocapture
      CARGO_TARGET_DIR=target/proposal-072-gate cargo test -p graphql-server missing_graphql_surface_policy_principals_denied_non_approval_mutations -- --test-threads=1 --nocapture
    )

    (
      cd "$ROOT_DIR"
      python3 - <<'PY'
from pathlib import Path

contract = Path("docs/reference/query-projections-and-client-consumption-contract.md").read_text()
boundary = Path("docs/reference/ui-action-boundary.md").read_text()
combined = contract + "\n" + boundary
required = [
    "The governed SwiftUI app is a GraphQL-only observer and approval console.",
    "approveApproval",
    "rejectApproval",
    "Non-approval GraphQL mutations are prohibited from governed UI code.",
]
for phrase in required:
    if phrase not in combined:
        raise SystemExit(f"proposal-072: stable UI boundary docs missing reconciliation phrase: {phrase}")

for forbidden in [
    "Approval rows are diagnostic-read-only in P031. Interactive approval decisions require a separate non-MCP, non-GraphQL UI transport proposal.",
    "Governed macOS UI has no MCP calls, no GraphQL mutations, and no local mutation fallback.",
    "GraphQL mutation usage: 0 GraphQL mutations defined or invoked by governed UI code.",
]:
    if forbidden in combined:
        raise SystemExit(f"proposal-072: stable UI boundary docs contain stale approval-boundary text: {forbidden}")

inventory = Path("docs/reference/p031-thin-ui-inventory.json").read_text()
for operation in ["P072ApproveApproval", "P072RejectApproval"]:
    if operation not in inventory:
        raise SystemExit(f"proposal-072: P031 inventory missing allowed approval operation {operation}")
PY
    )

    log "UI action boundary gate passed"
    ;;
  proposal-031-readiness|p031-readiness)
    log "Thin UI readiness gate: closeout evidence"
    "$0" proposal-031

    (
      cd "$ROOT_DIR"
      git ls-files --error-unmatch docs/evidence/p031-runtime/report-payload-live-evidence-2026-04-25.json >/dev/null
      git ls-files --error-unmatch docs/evidence/p031-runtime/p031-runtime-ui-chainworks-restored-db-degraded-sanitized-2026-04-24.png >/dev/null
      python3 - <<'PY'
from pathlib import Path
import json
import re

failures = []

manifest = json.loads(Path("docs/reference/p031-phase-0-artifact-manifest.json").read_text())
manifest_status = str(manifest.get("status", ""))
if "pending" in manifest_status.lower():
    failures.append(f"manifest status is not closeout-ready: {manifest_status}")

entries = {entry.get("id"): entry for entry in manifest.get("entries", [])}
for required in (
    "degraded_state_sanitized_screenshot",
    "report_payload_live_evidence",
    "dogfood_signoff_template",
):
    if required not in entries:
        failures.append(f"manifest missing closeout artifact entry: {required}")

for artifact_id, entry in entries.items():
    status = str(entry.get("validation_status", ""))
    if re.search(r"(pending|template|limitation)", status, re.IGNORECASE):
        failures.append(f"{artifact_id} validation_status is not closeout-ready: {status}")

dogfood = Path("docs/evidence/p031-dogfood-signoff.md").read_text()
status_match = re.search(r"^Status:\s*(.+)$", dogfood, re.MULTILINE)
dogfood_status = status_match.group(1).strip() if status_match else "<missing>"
if not re.search(r"(SIGNED|APPROVED|COMPLETE)", dogfood_status, re.IGNORECASE):
    failures.append(f"dogfood signoff is not signed/complete: {dogfood_status}")
if re.search(r"^- \[ \]", dogfood, re.MULTILINE):
    failures.append("dogfood checklist still has unchecked items")

for path in (
    "docs/evidence/p031-degraded-state-evidence.md",
    "docs/evidence/p031-freshness-baseline.md",
    "docs/evidence/p031-ux-accessibility-signoff.md",
):
    text = Path(path).read_text()
    if re.search(r"(waiver pending|dogfood confirmation pending|assistive access limitation|No VoiceOver pass)", text, re.IGNORECASE):
        failures.append(f"{path} still contains release-closeout qualification")

if failures:
    raise SystemExit("proposal-031-readiness failed:\n- " + "\n- ".join(failures))
PY
    )
    log "Thin UI readiness gate passed"
    ;;
  proposal-044|p044)
    log "Proposal 044 control-plane gate: post-approval + N-phase + end-state"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test --workspace 2>&1
    )
    log "Proposal 044 control-plane gate passed"
    ;;
  proposal-045|p045)
    log "Proposal 045 control-plane gate: deterministic release operations"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p engine --test integration test_start_run_persists_delivery_configuration_json -- --exact --nocapture &&
      cargo test -p engine --test release -- --nocapture &&
      cargo test -p graphql-server -- --nocapture &&
      cargo test -p mcp-server -- --nocapture
    )
    log "Proposal 045 control-plane gate passed"
    ;;
  proposal-047|p047)
    log "Proposal 047 control-plane gate: Rust workspace test suite"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test --workspace 2>&1
    )
    log "Proposal 047 control-plane gate passed"
    ;;
  proposal-048|p048)
    log "Proposal 048 control-plane gate: evidence packs, delivery preflight, and MCP resolution"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p db --test integration proposal_048_persistence_fields_round_trip -- --exact --nocapture &&
      cargo test -p engine --test integration delivery_preflight -- --nocapture &&
      cargo test -p graphql-server --lib delivery_preflight_graphql_readback_tests -- --nocapture &&
      cargo test -p mcp-server --lib delivery_preflight_mcp_readback_tests -- --nocapture &&
      cargo test -p acp --test integration mcp_servers_session_new_serialization_tests -- --exact --nocapture &&
      cargo test -p engine --test integration mcp_resolution_persistence_tests -- --nocapture &&
      cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture &&
      cargo test -p mcp-server --lib reports_failed_stage_evidence_contract_tests -- --nocapture &&
      cargo test -p mcp-server --lib report_resource_decodes_failed_stage_evidence_payload -- --nocapture &&
      cargo test -p graphql-server --lib graphql_start_run_blocked_payload_contract_tests -- --nocapture &&
      cargo test -p graphql-server --lib execution_mcp_truth_contract_tests -- --nocapture &&
      cargo test -p mcp-server --lib reports_mcp_resolution_truth_tests -- --nocapture &&
      cargo test -p mcp-server --lib report_resource_exposes_mcp_execution_truth -- --nocapture
	    )
	    log "Proposal 048 control-plane gate passed"
	    ;;
	  proposal-049|p049)
    log "Proposal 049 control-plane gate: steward analysis system"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p workflow steward_metadata_contract_tests -- --nocapture &&
      cargo test -p db steward -- --nocapture &&
      cargo test -p daemon steward_runtime_bootstrap_tests -- --nocapture &&
      cargo test -p engine steward -- --nocapture &&
      cargo test -p engine test_start_run_persists_delivery_configuration_json -- --exact --nocapture &&
      cargo test -p graphql-server steward_graphql -- --nocapture &&
      cargo test -p mcp-server steward_mcp -- --nocapture
    )
    log "Proposal 049 control-plane gate passed"
    ;;
  proposal-050|p050)
    log "Proposal 050 control-plane gate: per-run workspace isolation"
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-050-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      # Focused P050 proof inventory
      cargo test -p engine --test integration test_resolve_path_template_uses_run_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_resolve_path_template_null_meta_root_uses_template_default -- --exact --nocapture &&
      cargo test -p engine --test integration test_resolve_path_template_does_not_consult_process_env_for_runs -- --exact --nocapture &&
      cargo test -p engine --test integration test_normalize_path_for_worktree_skips_meta_root_paths -- --exact --nocapture &&
      cargo test -p engine --test integration test_normalize_path_for_worktree_still_normalizes_source_paths -- --exact --nocapture &&
      cargo test -p engine --test integration test_exists_checks_per_run_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_artifact_field_reads_per_run_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_execution_request_carries_chainworks_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_normalize_artifacts_ignores_stale_flat_artifact_root_for_post_p050_runs -- --exact --nocapture &&
      cargo test -p engine --test integration test_normalize_artifacts_uses_run_scoped_source_dir_for_post_p050_runs -- --exact --nocapture &&
      cargo test -p engine --test integration test_normalize_artifacts_preserves_flat_root_fallback_for_null_legacy_runs -- --exact --nocapture &&
      cargo test -p engine --test integration test_mcp_runs_get_exposes_chainworks_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_mcp_runs_list_projection_exposes_chainworks_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_runs_start_does_not_accept_chainworks_meta_root_override -- --exact --nocapture &&
      cargo test -p engine --test integration test_new_run_gets_isolated_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_stale_workspace_artifacts_not_visible_to_new_run -- --exact --nocapture &&
      cargo test -p engine --test integration test_prompt_input_paths_point_to_per_run_meta_root -- --exact --nocapture &&
      cargo test -p engine --test integration test_run_serde_includes_chainworks_meta_root -- --exact --nocapture &&
      # GraphQL readback proof (P050 §2f AC-12)
      cargo test -p graphql-server test_graphql_run_exposes_chainworks_meta_root -- --nocapture &&
      # Full workspace regression
      mkdir -p "$ROOT_DIR/reports/test-gates"
      workspace_log="$ROOT_DIR/reports/test-gates/proposal-050-workspace.log"
      printf 'Running P050 full workspace regression; log: %s\n' "$workspace_log"
      : >"$workspace_log"
      for package in domain db acp auth engine graphql-server mcp-server workflow daemon; do
        printf ' [%s]' "$package"
        {
          printf '\n=== cargo test -p %s ===\n' "$package"
          cargo test -p "$package" -- --test-threads=1
        } >>"$workspace_log" 2>&1 || {
          tail -200 "$workspace_log" >&2
          exit 1
        }
      done
      printf '\n'
      if ! grep -q 'test_claude_adapter_receives_chainworks_meta_root_env ... ok' "$workspace_log"; then
        printf 'P050 full workspace regression did not include the Claude CHAINWORKS_META_ROOT env proof\n' >&2
        exit 1
      fi
      printf 'P050 full workspace regression log: %s\n' "$workspace_log"
    )
    log "Proposal 050 control-plane gate passed"
    ;;
  p051-scaffold)
    log "Proposal 051 scaffold gate: shared Xcode MCP bridge pool substrate"
    python3 - <<'PY'
from pathlib import Path

source = Path("docs/reference/xcode-mcp-bridge-pool.md")
if not source.exists():
    raise SystemExit(f"p051-scaffold: missing stable bridge-pool reference {source}")

lines = source.read_text().splitlines()
stale_checks = [
    ("no SwiftUI changes", ["no swiftui changes", "no swift app ui changes", "no ui changes"]),
    ("debug_assert-only capability enforcement", ["debug_assert"]),
    ("path+mtime+size-only binary fingerprinting", ["path+mtime+size", "path, mtime, and size", "path mtime size"]),
    ("drop-on-corrupt observation behavior", ["drop-on-corrupt", "drop on corrupt", "drop corrupt"]),
    ("direct pgrep newest-Xcode selection", ["pgrep", "newest xcode"]),
    ("unbound same-uid-only shim authorization", ["same-uid-only", "same uid only"]),
]
strict_stale_checks = [
    ("backend per provider HTTP lease", ["per provider http lease", "per-provider http lease", "per active provider http lease"]),
    ("independent same-target leases/backends", ["independent leases and backends", "across independent leases and backends", "isolated leases/backends"]),
    ("lease-owned stdio backend", ["each lease has one stdio backend"]),
    ("per-lease backend failure semantics", ["fail only that lease", "fail only that lease with per-lease backend failure"]),
]
allowed_context_markers = [
    "absent",
    "concern",
    "fail",
    "forbid",
    "not only",
    "prohibit",
    "reject",
    "replace",
    "required",
    "resolution",
    "resolved",
    "security review",
    "scope_change",
    "stale",
    "strengthened",
    "threshold",
    "tightened",
]
stale = []
for label, needles in stale_checks:
    offending_lines = []
    for line_number, line in enumerate(lines, start=1):
        normalized = line.lower()
        if not any(needle in normalized for needle in needles):
            continue
        if any(marker in normalized for marker in allowed_context_markers):
            continue
        offending_lines.append(line_number)
    if offending_lines:
        stale.append(label)

for label, needles in strict_stale_checks:
    offending_lines = []
    for line_number, line in enumerate(lines, start=1):
        normalized = line.lower()
        if any(needle in normalized for needle in needles):
            offending_lines.append(line_number)
    if offending_lines:
        stale.append(f"{label} at lines {offending_lines}")

if stale:
    raise SystemExit(
        "p051-scaffold: docs/reference/xcode-mcp-bridge-pool.md still contains "
        "stale contrary guidance: " + ", ".join(stale)
    )
PY
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-051-scaffold-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      cargo test -p workflow --test integration p051_ -- --nocapture &&
      cargo test -p db --test integration proposal_051_xcode_runtime_observation -- --nocapture &&
      cargo test -p acp --test integration brokered_xcode_probe_accepts_http_but_requires_lease_conversion -- --exact --nocapture &&
      cargo test -p acp --test integration xcode_mcp_bridge_pool_ -- --nocapture &&
      cargo test -p acp --test integration runtime_manager_attaches_brokered_xcode_http_lease_before_session_new -- --exact --nocapture &&
      cargo test -p engine --test integration xcode_broker_fail_closed_observation_is_persisted_from_acp_sink -- --exact --nocapture &&
      cargo check -p graphql-server &&
      cargo check -p mcp-server
    )
    log "Proposal 051 scaffold gate passed"
    ;;
  proposal-051|p051)
    log "Proposal 051 gate: shared Xcode MCP bridge pool"
    "$0" p051-scaffold
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-051-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      cargo test -p domain --test artifact_contracts -- --nocapture &&
      cargo test -p workflow --test integration p051_ -- --nocapture &&
      cargo test -p db --test integration proposal_051_xcode_runtime_observation -- --nocapture &&
      cargo test -p acp --test integration xcode_mcp_bridge_pool_ -- --nocapture &&
      cargo test -p engine --test integration xcode_broker_fail_closed_observation_is_persisted_from_acp_sink -- --exact --nocapture &&
      cargo check -p graphql-server &&
      cargo check -p mcp-server
    )
    run_targeted_tests "proposal-051-swift" \
      "Chainworks ForgeTests/RunTimelineInspectorViewTests" \
      "Chainworks ForgeTests/DaemonLifecycleClientTests"
    log "Proposal 051 gate passed"
    ;;
  proposal-057|p057)
    log "Proposal 057 control-plane gate: canonical artifact contracts and run-state projection"
    mkdir -p "$ROOT_DIR/reports/test-gates"
    log "P057 P037 prerequisite evidence is control-plane-only: failed/partial provider settlement is proved by the P057-local engine degraded-output tests."
    "$0" proposal-043
    "$0" proposal-050
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-057-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      cargo test -p domain --test proposal_057_contracts -- --test-threads=1 --nocapture &&
      cargo test -p workflow proposal_057_ -- --test-threads=1 --nocapture &&
      cargo test -p db --test proposal_057_contracts -- --test-threads=1 --nocapture &&
      cargo test -p engine proposal_057_ -- --test-threads=1 --nocapture &&
      cargo test -p graphql-server --test proposal_057_contracts -- --test-threads=1 --nocapture &&
      cargo test -p mcp-server --test proposal_057_contracts -- --test-threads=1 --nocapture
    )
    python3 - <<'PY'
from pathlib import Path

text = Path("docs/reference/test-gates.md").read_text()
required = [
    "### `proposal-057|p057`",
    "canonical artifact contracts",
    "active-index SQLite owner",
    "degraded output policy",
    "typed operator overrides",
    "GraphQL/MCP readback parity",
    "P037 control-plane evidence bucket",
    "Same-tree composed gates: `proposal-043` and `proposal-050`",
    "P057 prerequisite waiver: P054",
    "P057 prerequisite waiver: P056",
]
for item in required:
    if item not in text:
        raise SystemExit(f"proposal-057: docs/reference/test-gates.md missing {item}")
PY
    log "Proposal 057 control-plane gate passed"
    ;;
  proposal-053|p053)
    log "Proposal 053 control-plane gate: bounded ACP artifact discovery"
    python3 - <<'PY'
import json
from datetime import datetime
from pathlib import Path

root = Path.cwd()
evidence = root / "docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency"
cap = evidence / "cap-validation.json"
security = evidence / "security-checklist.md"
manual_latency = evidence / "manual-latency-spot-check.md"
operator_clarity = evidence / "operator-clarity-evidence.md"
retrospective = evidence / "phase-1-retrospective.md"
if not cap.exists():
    raise SystemExit("proposal-053: missing docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/cap-validation.json")
if not security.exists():
    raise SystemExit("proposal-053: missing docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/security-checklist.md")
if not manual_latency.exists():
    raise SystemExit("proposal-053: missing docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/manual-latency-spot-check.md")
if not operator_clarity.exists():
    raise SystemExit("proposal-053: missing docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/operator-clarity-evidence.md")
if not retrospective.exists():
    raise SystemExit("proposal-053: missing docs/evidence/053-bounded-acp-artifact-discovery-and-startup-latency/phase-1-retrospective.md")
data = json.loads(cap.read_text())
required = {
    "schema_version",
    "proposal_revision_id",
    "sampled_execution_ids",
    "source_query_or_extraction_method",
    "sample_coverage_by_workflow_template",
    "sample_coverage_by_agent_provider",
    "dependency_readiness_recorded_within_two_working_days",
    "dependency_escalations",
    "narrow_adapter_owner_when_needed",
    "phase_1_exposure_mode",
    "production_data_availability_status",
    "production_data_fallback_decision",
    "excluded_outputs",
    "per_output_bytes_p50",
    "per_output_bytes_p90",
    "per_output_bytes_p99",
    "aggregate_bytes_p50",
    "aggregate_bytes_p90",
    "aggregate_bytes_p99",
    "expected_output_spec_count_p90",
    "dependency_readiness",
    "chosen_max_expected_output_specs",
    "chosen_pre_prompt_metadata_timeout_ms",
    "chosen_pre_prompt_digest_budget_bytes",
    "chosen_max_exact_output_bytes",
    "chosen_max_provider_envelope_bytes",
    "chosen_max_aggregate_declared_output_bytes",
    "chosen_provider_envelope_buffer_policy",
    "workflow_output_size_policy_required",
    "fresh_and_reused_session_metadata_semantics_frozen",
    "discovery_filesystem_owner",
    "chosen_aggregate_acceptance_cap_bytes",
    "interface_freeze",
    "reviewer_signoff",
    "generated_at",
}
missing = sorted(required.difference(data))
if missing:
    raise SystemExit(f"proposal-053: cap-validation missing fields: {missing}")
if not isinstance(data["sampled_execution_ids"], list) or not data["sampled_execution_ids"]:
    raise SystemExit("proposal-053: sampled_execution_ids must be a non-empty list")
if data["dependency_readiness_recorded_within_two_working_days"] is not True:
    raise SystemExit("proposal-053: dependency_readiness_recorded_within_two_working_days must be true")
if data["phase_1_exposure_mode"] not in {"gate_only_internal", "production_exposed"}:
    raise SystemExit("proposal-053: phase_1_exposure_mode must be gate_only_internal or production_exposed")
if data["phase_1_exposure_mode"] == "production_exposed":
    if data.get("phase_1_exposure_decision", {}).get("production_shippable") is not True:
        raise SystemExit("proposal-053: production_exposed mode requires production_shippable=true")
    if data.get("production_data_availability_status") != "approved_replacement_sample":
        raise SystemExit("proposal-053: production_exposed mode requires approved replacement sample evidence")
    for key in [
        "per_output_bytes_p50",
        "per_output_bytes_p90",
        "per_output_bytes_p99",
        "aggregate_bytes_p50",
        "aggregate_bytes_p90",
        "aggregate_bytes_p99",
        "expected_output_spec_count_p90",
    ]:
        if data.get(key) is None:
            raise SystemExit(f"proposal-053: production_exposed mode requires {key}")
try:
    datetime.fromisoformat(data["generated_at"].replace("Z", "+00:00"))
except Exception as exc:  # noqa: BLE001
    raise SystemExit(f"proposal-053: generated_at must be ISO-8601 ({exc})")
for dep in ["P037", "P050", "P057", "P058"]:
    if dep not in data["dependency_readiness"]:
        raise SystemExit(f"proposal-053: dependency_readiness missing {dep}")
    if data["dependency_readiness"][dep].get("status") not in {"ready", "ready_with_adapter", "blocked"}:
        raise SystemExit(f"proposal-053: dependency_readiness.{dep}.status invalid")
for key in [
    "expected_output_spec",
    "pre_prompt_expected_output_metadata",
    "output_discovery_decision",
    "discovery_filesystem_operation_recorder",
    "git_manifest_runner",
    "captured_output_builder",
    "settle_agent_outputs_from_discovery_decisions",
    "discovery_filesystem_trait",
    "discovery_filesystem_fake",
]:
    if data["interface_freeze"].get(key) is not True:
        raise SystemExit(f"proposal-053: interface_freeze.{key} must be true")
text = security.read_text()
for needle in [
    "Production exposure | Approved for P053 control-plane/API/readback behavior",
    "proposal_053_gate_uses_discovery_filesystem_trait_fake",
    "StaleExpectedOutput",
]:
    if needle not in text:
        raise SystemExit(f"proposal-053: security checklist missing {needle!r}")
for path, required_needles in [
    (
        manual_latency,
        [
            "# P053 Manual Latency Spot-Check",
            "Reference Workspace Measurement",
            "8.9 GB",
            "126,643",
            "acp_pre_initialize_local_latency_ms=0",
            "Result:",
            "pre-`initialize`",
        ],
    ),
    (operator_clarity, ["# P053 Operator Clarity Evidence", "Result:", "provider latency"]),
    (retrospective, ["# P053 Phase 1 Retrospective", "Decision:", "P069"]),
]:
    text = path.read_text()
    for needle in required_needles:
        if needle not in text:
            raise SystemExit(f"proposal-053: {path.name} missing {needle!r}")
PY
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-053-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      cargo test -p domain discovery::tests::generated_state_denylist_matches_p053_roots -- --exact --nocapture &&
      cargo test -p domain discovery::tests::proposal_053_operation_recorder_observes_bounded_discovery_without_generated_state_reads -- --exact --nocapture &&
      cargo test -p domain discovery::tests::proposal_053_gate_uses_discovery_filesystem_trait_fake -- --exact --nocapture &&
      cargo test -p domain discovery::tests::proposal_053_operation_recorder_orders_metadata_before_file_read -- --exact --nocapture &&
      cargo test -p domain discovery::tests::expected_output_spec_serializes_p053_policy_fields -- --exact --nocapture &&
      cargo test -p domain bounded_pre_prompt_metadata -- --nocapture &&
      cargo test -p domain proposal_053_bounded_meta_root -- --nocapture &&
      cargo test -p domain proposal_053_legacy_broad_discovery -- --nocapture &&
      cargo test -p db proposal_053_discovery_diagnostics --test proposal_053_discovery_diagnostics -- --nocapture &&
      cargo test -p workflow proposal_053_output_policies -- --nocapture &&
      cargo test -p workflow proposal_053_legacy_broad_discovery_policy -- --nocapture &&
      cargo test -p acp caps_declared_payload_before_settlement -- --nocapture &&
      cargo test -p acp proposal_053_acp_prompt_metadata_uses_discovery_filesystem_fake --lib -- --nocapture &&
      cargo test -p acp test_claude_adapter_keeps_legacy_broad_discovery_disabled_by_default --test integration -- --nocapture &&
      cargo test -p acp test_claude_adapter_executes_subprocess_and_returns_artifacts --test integration -- --nocapture &&
      cargo test -p engine expected_output_specs -- --nocapture &&
      cargo test -p engine proposal_053_must_produce_does_not_accept_unchanged_existing_output --lib -- --nocapture &&
      cargo test -p engine proposal_053_bounded_meta_root_artifact_paths_are_supplemental_only --lib -- --nocapture &&
      cargo test -p engine proposal_053_engine_settlement_uses_discovery_filesystem_fake_for_exact_path --lib -- --nocapture &&
      cargo test -p engine proposal_053_engine_stale_detection_uses_discovery_filesystem_fake --lib -- --nocapture &&
      cargo test -p engine proposal_053_bounded_meta_root_uses_discovery_filesystem_fake --lib -- --nocapture &&
      cargo test -p engine proposal_053_git_manifest_runner -- --nocapture &&
      cargo test -p engine proposal_053_declared_manifest_preserves_agent_authored_file -- --nocapture &&
      cargo test -p engine test_retry_stage_legacy_discovery_override_validation_failure_leaves_no_journal --test integration -- --nocapture &&
      cargo test -p graphql-server proposal_053_agent_execution_projects_discovery_reconciliation_pending --test proposal_058_runtime_facts -- --nocapture &&
      cargo test -p mcp-server proposal_053_reports_get_projects_discovery_reconciliation_pending --test proposal_058_runtime_facts -- --nocapture
    )
    log "Proposal 053 control-plane gate passed"
    ;;
  proposal-058|p058)
    log "Proposal 058 control-plane gate: ACP provider failure classification and session artifact ownership"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p domain --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      cargo test -p engine proposal_058 --lib -- --test-threads=1 --nocapture &&
      cargo test -p db --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR="$TMP_BASE/proposal-058-db-claim-target" cargo test -p db --test proposal_058_claim_start -- --test-threads=1 --nocapture &&
      cargo test -p engine --test proposal_058_claim_start -- --test-threads=1 --nocapture &&
      cargo test -p graphql-server --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      CARGO_BUILD_JOBS=1 CARGO_TARGET_DIR="$TMP_BASE/proposal-058-mcp-target" cargo test -p mcp-server --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      cargo check -p engine &&
      cargo check -p graphql-server &&
      cargo check -p mcp-server
    )
    python3 - <<'PY'
from pathlib import Path

text = Path("docs/reference/test-gates.md").read_text()
required = [
    "### `proposal-058|p058`",
    "ACP provider failure classification",
    "runtime facts",
    "artifact source-generation claims",
    "superseded_pending_retry",
    "GraphQL/MCP runtime-facts parity",
]
for item in required:
    if item not in text:
        raise SystemExit(f"proposal-058: docs/reference/test-gates.md missing {item}")
PY
    log "Proposal 058 control-plane gate passed"
    ;;
  proposal-060|p060)
    run_proposal060_all_control_artifacts
    log "Proposal 060 Rust gate: Phase 1 + Phase 2 + Phase 3 focused tests"
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-060-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      cargo test -p domain --lib routing -- --test-threads=1 --nocapture
      cargo test -p engine --lib proposal_review_router -- --test-threads=1 --nocapture
      cargo test -p engine --lib command_handler::tests::p060 -- --test-threads=1 --nocapture
      cargo test -p engine --test integration test_start_run_persists_delivery_configuration_json -- --test-threads=1 --nocapture
      cargo test -p engine --test integration p060_legacy_and_shadow_modes_dispatch_fixed_quartet_on_dynamic_workflow -- --test-threads=1 --nocapture
      cargo test -p graphql-server --lib start_run_accepts_delivery_configuration_json -- --test-threads=1 --nocapture
      cargo test -p mcp-server --lib runs_start_persists_delivery_configuration_json -- --test-threads=1 --nocapture
      cargo test -p workflow --test integration p060 -- --test-threads=1 --nocapture
    )
    xcodebuild test -project "$ROOT_DIR/Chainworks Forge.xcodeproj" -scheme "Chainworks Forge" -destination "platform=macOS" -only-testing:"Chainworks ForgeTests/ProposalReviewRoutingTests" CODE_SIGNING_ALLOWED=NO CODE_SIGNING_REQUIRED=NO CODE_SIGN_IDENTITY=

    # Phase 3 closure guards (audit ARCH/OPS-style).
    # These die before the gate exits if any of the Phase 3 contracts
    # have been removed even if a copycat test still passes structurally.
    log "Verifying P060 Phase 3 closure surfaces..."
    P060_AUTHORIZER_FILE="$ROOT_DIR/control-plane/crates/domain/src/routing.rs"
    P060_ORCHESTRATOR_FILE="$ROOT_DIR/control-plane/crates/engine/src/orchestrator.rs"
    if ! grep -q "RoutingEvidenceProjectionAuthorizer" "$P060_AUTHORIZER_FILE"; then
      die "P060 Phase 3: missing RoutingEvidenceProjectionAuthorizer in domain::routing"
    fi
    if ! grep -q "PrincipalClassDebugRoutingHook" "$P060_AUTHORIZER_FILE"; then
      die "P060 Phase 3: missing PrincipalClassDebugRoutingHook trait in domain::routing"
    fi
    if ! grep -q "CHAINWORKS_OPERATOR_DEBUG_ROUTING_EVIDENCE" "$P060_AUTHORIZER_FILE"; then
      die "P060 Phase 3: authorizer missing CHAINWORKS_OPERATOR_DEBUG_ROUTING_EVIDENCE env gate"
    fi
    if ! grep -q "resolve_effective_routing_mode" "$P060_AUTHORIZER_FILE"; then
      die "P060 Phase 3: missing resolve_effective_routing_mode in domain::routing"
    fi
    if ! grep -q "CHAINWORKS_P060_ROUTING_MODE_OVERRIDE" "$P060_AUTHORIZER_FILE"; then
      die "P060 Phase 3: feature-flag env name CHAINWORKS_P060_ROUTING_MODE_OVERRIDE missing"
    fi
    if ! grep -q "is_shadow" "$P060_ORCHESTRATOR_FILE"; then
      die "P060 Phase 3: orchestrator missing shadow-mode dispatch handling"
    fi
    if ! grep -q "shadow_succeeded\|shadow_failed" "$P060_ORCHESTRATOR_FILE"; then
      die "P060 Phase 3: orchestrator missing shadow_succeeded/shadow_failed RoutingCompleted labels"
    fi
    if ! grep -q "resolve_effective_routing_mode" "$P060_ORCHESTRATOR_FILE"; then
      die "P060 Phase 3: orchestrator does not consult resolve_effective_routing_mode"
    fi
    # Phase 3 closure tests: at least one test name per contract must exist.
    P060_DOMAIN_TEST_PATTERNS=(
      "routing_evidence_projection_authorizer_default_is_redacted"
      "routing_evidence_projection_authorizer_full_preserves_fields"
      "routing_evidence_projection_authorizer_redacts_for_non_operators"
      "routing_evidence_projection_authorizer_grants_operator_with_env"
      "resolve_effective_routing_mode_no_env_returns_per_run_mode"
      "resolve_effective_routing_mode_env_legacy_overrides_dynamic"
      "resolve_effective_routing_mode_env_shadow_overrides_legacy"
      "resolve_effective_routing_mode_unrecognized_env_falls_back_to_per_run"
    )
    for t in "${P060_DOMAIN_TEST_PATTERNS[@]}"; do
      if ! grep -q "$t" "$P060_AUTHORIZER_FILE"; then
        die "P060 Phase 3: closure test '$t' missing from domain::routing tests"
      fi
    done
    log "Proposal 060 gate passed"
    ;;
  proposal-060-baseline|p060-baseline|proposal-060-storage|p060-storage|proposal-060-router-fixtures|p060-router-fixtures|proposal-060-snapshot-inventory|p060-snapshot-inventory|proposal-060-fixed-quartet|p060-fixed-quartet|proposal-060-ticket-map|p060-ticket-map|proposal-060-calibration|p060-calibration)
    run_proposal060_control_artifact_gate "$GATE"
    ;;
  proposal-061|p061)
    log "Proposal 061 control-plane gate: provider normalization and capacity defaults"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p domain "${PROPOSAL_061_TESTS[0]}" -- --nocapture
      cargo test -p workflow proposal_061_catalog --test integration -- --nocapture
      cargo test -p db proposal_061_hot_index_query_plans_use_indexes_at_fixture_scale -- --nocapture
      for test_name in "${PROPOSAL_061_TESTS[@]:1}"; do
        cargo test -p engine "$test_name" -- --nocapture
      done
      cargo test -p graphql-server proposal_061 -- --nocapture
      cargo test -p mcp-server proposal_061 -- --nocapture
    )
    log "Proposal 061 control-plane gate passed"
    ;;
  proposal-064|p064)
    log "Proposal 064 Phase 0 gate: main-sync and knowledge readback contracts"
    python3 - <<'PY'
import json
from pathlib import Path

root = Path.cwd()
required = {
    "docs/proposals/064-control-artifacts/dogfood-baseline-20260421.v1.json": "p064_dogfood_baseline_v1",
    "docs/proposals/064-control-artifacts/phase-0-kickoff.v1.json": "p064_phase_0_kickoff_v1",
}
for rel, schema in required.items():
    path = root / rel
    if not path.exists():
        raise SystemExit(f"proposal-064: missing {rel}")
    data = json.loads(path.read_text())
    if data.get("schema_version") != schema:
        raise SystemExit(f"proposal-064: {rel} schema_version must be {schema}")
    if data.get("status") != "recorded":
        raise SystemExit(f"proposal-064: {rel} status must be recorded")

migration = (root / "control-plane/crates/db/migrations/033_p064_main_sync_and_knowledge_capsules.sql").read_text()
for needle in [
    "main_sync_attempts",
    "worktree_mutation_barriers",
    "run_knowledge_capsules",
    "run_knowledge_capsule_attachments",
    "ALTER TABLE work_items ADD COLUMN worktree_access_mode",
    "ALTER TABLE background_leases ADD COLUMN worktree_resource_key",
    "idx_background_leases_worktree_owner",
]:
    if needle not in migration:
        raise SystemExit(f"proposal-064: migration missing {needle}")
PY
    (
      cd "$ROOT_DIR/control-plane"
      export CARGO_TARGET_DIR=target/proposal-064-gate
      export CARGO_BUILD_JOBS="${CARGO_BUILD_JOBS:-1}"
      cargo test -p domain main_sync -- --nocapture &&
      cargo test -p engine main_sync_fixtures -- --nocapture &&
      cargo test -p mcp-server p064_operator_tools_are_registered_but_hidden_until_modes_enable_runtime -- --nocapture &&
      cargo test -p mcp-server test_mcp_tools_call_denied_returns_method_not_found -- --nocapture &&
      cargo test -p graphql-server proposal_064_run_query_exposes_sync_and_capsule_readback -- --nocapture
    )
    log "Proposal 064 Phase 0 gate passed"
    ;;
  proposal-065|p065)
    log "Proposal 065 control-plane gate: operator retry instruction contract"
    (
      cd "$ROOT_DIR/control-plane"
      # R11 speed-up: per-crate batching
      for spec in "${PROPOSAL_065_TESTS[@]}"; do
        crate="${spec%% *}"
        test_name="${spec#* }"
        log "proposal-065: focused crate=$crate test=$test_name"
        if ! cargo test -p "$crate" "$test_name" -- --nocapture; then
          echo "proposal-065: FAIL — $crate::$test_name returned a non-zero exit"
          exit 1
        fi
      done
    )
    log "Proposal 065 control-plane gate passed"
    ;;
  proposal-066|p066)
    log "Proposal 066 Phase 0 gate: toolchain cache mapping scaffold"

    # T06 — Swift scan guardrail: ToolchainMappingReadAdapter must exist and be the
    # sole owner of toolchain policy decoding. Consumer files must not access
    # .toolchainCachePolicy directly (that field is declared in AgentCatalog.swift
    # and decoded only through ToolchainMappingReadAdapter).
    log "P066 T06: Swift scan — ToolchainMappingReadAdapter ownership guardrail"
    ADAPTER_FILE="$ROOT_DIR/Chainworks Forge/Engine/ToolchainMappingReadAdapter.swift"
    if [ ! -f "$ADAPTER_FILE" ]; then
      fail "P066 T06: ToolchainMappingReadAdapter.swift missing — must exist before Phase 1"
    fi
    CONSUMER_FILES=(
      "$ROOT_DIR/Chainworks Forge/Engine/RunPlanCompiler.swift"
      "$ROOT_DIR/Chainworks Forge/Engine/ExecutionService.swift"
      "$ROOT_DIR/Chainworks Forge/Engine/RunReportBuilder.swift"
      "$ROOT_DIR/Chainworks Forge/Engine/RunComparisonService.swift"
    )
    for f in "${CONSUMER_FILES[@]}"; do
      if [ ! -f "$f" ]; then
        fail "P066 T06: expected consumer file missing: $f"
      fi
      # Consumer files must not directly access .toolchainCachePolicy on a decoded
      # AgentDefinition outside of ToolchainMappingReadAdapter.swift itself.
      # The pattern to forbid is direct property access like `agent.toolchainCachePolicy`
      # or `.toolchainCachePolicy` in these operator-facing service files.
      if grep -qE '\.toolchainCachePolicy\b' "$f"; then
        fail "P066 T06: $f directly accesses .toolchainCachePolicy — route through ToolchainMappingReadAdapter instead"
      fi
    done
    log "P066 T06: Swift scan passed — ToolchainMappingReadAdapter is sole bridge"

    (
      cd "$ROOT_DIR/control-plane"
      log "P066: workflow crate — YAML schema, compatibility gates, snapshot types"
      cargo test -p workflow --test proposal_066_toolchain_cache_policy -- --nocapture

      log "P066: domain crate — toolchain failure kinds"
      cargo test -p domain --lib toolchain:: -- --nocapture

      log "P066: db crate — migration 037 and diagnostics column"
      cargo test -p db --test proposal_066_toolchain_cache_mapping -- --nocapture

      log "P066: graphql-server crate — northbound synthesis (active, disabled, legacy)"
      cargo test -p graphql-server --test proposal_066_toolchain_mapping -- --nocapture

      log "P066: mcp-server crate — northbound synthesis (active, disabled, legacy)"
      cargo test -p mcp-server --test proposal_066_toolchain_mapping -- --nocapture

      log "P066 T17/T18: graphql-server — startupRecoverySummary.toolchainCache and toolchainCacheHousekeepingSummary"
      cargo test -p graphql-server --test proposal_066_cleanup_readbacks -- --nocapture

      log "P066 T22: graphql-server — migration drill (≥10 legacy NULL rows + ≥10 post-migration rows)"
      cargo test -p graphql-server --test proposal_066_migration_drill -- --nocapture

      log "P066 T10/T11/T12/T13: acp crate — toolchain mapper, host-executor rewriting, Go env, lease"
      cargo test -p acp --test proposal_066_toolchain_mapper -- --nocapture

      log "P066 T13: acp crate — per-run Xcode lease unit tests"
      cargo test -p acp --lib toolchain_lease -- --nocapture

      log "P066 T14: engine crate — startup recovery toolchain sweep (Go orphan reclaim + Xcode quarantine)"
      cargo test -p engine --test proposal_066_toolchain_recovery -- --nocapture

      log "P066 T19: engine crate — housekeeping pruning of terminal run-scoped Xcode roots"
      cargo test -p engine --test proposal_066_toolchain_housekeeping -- --nocapture
    )
    log "Proposal 066 Phase 0 gate passed"
    ;;
  proposal-042|p042)
    log "Proposal 042 control-plane gate: Rust focused + Swift focused + workspace regression"
    log "  running ${#PROPOSAL_042_TESTS[@]} focused Rust tests from the §10.2 Layer A inventory"
    log "  Swift focused lane composes with Rust lane per §10.3; release"
    log "  readiness additionally requires proposal-042-packaging on the release host."
    # R11 speed-up: two structural changes keep the gate's contract
    # intact while cutting wall time roughly in half on typical
    # workstations.
    #
    #   1. Per-crate batching. The old loop issued one
    #      `cargo test -p <crate> <name>` per inventory entry (~112
    #      invocations) — each one relinks the test binary for its
    #      crate even when the test itself is a microsecond. We now
    #      group entries by crate and pass every test name to a single
    #      invocation: `cargo test -p <crate> name1 name2 …`. Cargo
    #      OR-combines positional args as filters, so semantics are
    #      identical. The §10.3 strict post-check (every named test
    #      must produce output) runs against the combined log
    #      afterward, so rename/typo/delete still fails the gate.
    #
    #   2. Parallel Swift lane + workspace regression. The focused
    #      Rust loop must finish first (its strict check is the
    #      shape-enforcing layer). Once that completes, `cargo test
    #      --workspace` and `"$0" proposal-042-swift` are launched in
    #      parallel. They use disjoint caches (cargo target/ vs the
    #      Swift gate's `-derivedDataPath`) so file contention is
    #      minimal. Both must succeed for the gate to pass.
    rust_gate_stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    focused_log="${TMP_BASE}/p042-focused-${rust_gate_stamp}.log"
    workspace_log="${TMP_BASE}/p042-workspace-${rust_gate_stamp}.log"
    swift_parallel_log="${TMP_BASE}/p042-swift-parallel-${rust_gate_stamp}.log"
    swift_parallel_exit="${TMP_BASE}/p042-swift-parallel-${rust_gate_stamp}.exit"
    workspace_exit="${TMP_BASE}/p042-workspace-${rust_gate_stamp}.exit"
    mkdir -p "$TMP_BASE"
    : >"$focused_log"

    # R13 diagnostic (see follow-up thread): sharing `control-plane/target`
    # with other agents' `cargo` processes (e.g. Codex running its own
    # proposal gate in the same checkout) produces `rustc` processes that
    # hang at 0% CPU — mid-compile fingerprints collide, the artifact lock
    # stays stale after an external SIGKILL, and subsequent `cargo build`
    # attempts wait indefinitely. The operator's own workaround was a
    # dedicated `CARGO_TARGET_DIR`. We bake that into the gate: every
    # `proposal-042|p042` run uses `target/p042-gate` (matching the
    # `target/proposal-NNN-gate` naming other proposals already use).
    # First run on a fresh host is a cold compile; subsequent runs reuse
    # the cache and stay under ~3 min. Critically, no other agent can
    # pollute this target dir.
    p042_cargo_target="$ROOT_DIR/control-plane/target/p042-gate"
    export CARGO_TARGET_DIR="$p042_cargo_target"
    log "  gate-owned CARGO_TARGET_DIR: $p042_cargo_target"

    # ── 1. Per-crate batched focused run ────────────────────────────
    #
    # Collect unique crate names in first-appearance order without
    # using associative arrays — macOS ships bash 3.2 which does not
    # support `declare -A`. O(n²) here is trivial (≲700 iterations)
    # and keeps the script runnable on unmodified macOS.
    p042_crates=()
    for spec in "${PROPOSAL_042_TESTS[@]}"; do
      c="${spec%% *}"
      already_seen=0
      for existing in "${p042_crates[@]:-}"; do
        if [[ "$existing" == "$c" ]]; then
          already_seen=1
          break
        fi
      done
      if [[ "$already_seen" == "0" ]]; then
        p042_crates+=("$c")
      fi
    done
    (
      cd "$ROOT_DIR/control-plane"
      for crate in "${p042_crates[@]}"; do
        # Gather this crate's tests from the flat inventory. The
        # positional args cargo test accepts are OR-combined filters,
        # so passing all names in one invocation has identical
        # semantics to the old per-test loop — we just pay the
        # relink cost once per crate instead of once per test.
        tests=()
        for spec in "${PROPOSAL_042_TESTS[@]}"; do
          if [[ "${spec%% *}" == "$crate" ]]; then
            tests+=("${spec#* }")
          fi
        done
        log "proposal-042: focused crate=$crate tests=${#tests[@]}"
        # `cargo test` takes at most one positional TESTNAME filter;
        # additional filters go to the test binary AFTER the `--`.
        # libtest treats multiple positional args as OR-combined
        # substring filters, so passing every inventory name in one
        # invocation runs exactly that subset.
        if ! cargo test -p "$crate" -- --nocapture "${tests[@]}" 2>&1 | tee -a "$focused_log"; then
          echo "proposal-042: FAIL — focused cargo test for $crate returned a non-zero exit"
          exit 1
        fi
      done
    ) || exit 1

    # Strict post-check: every inventory entry must have produced a
    # matching `^test …::<name>` output line. Renames/typos/deletions
    # fail the gate here, before the parallel regression launches.
    for spec in "${PROPOSAL_042_TESTS[@]}"; do
      crate="${spec%% *}"
      test_name="${spec#* }"
      if ! grep -E "^test ([A-Za-z0-9_]+::)*${test_name}[[:space:]]" "$focused_log" >/dev/null; then
        echo "proposal-042: FAIL — no test named '$test_name' (crate '$crate') produced output (renamed, deleted, or typo'd?)"
        exit 1
      fi
    done
    log "proposal-042: focused inventory (${#PROPOSAL_042_TESTS[@]} tests across ${#p042_crates[@]} crates) passed"

    # ── 2. Parallel Swift lane + workspace regression ────────────────
    #
    # Launch the workspace regression in the background and the Swift
    # lane in the foreground so the script keeps showing live
    # `xcodebuild` output. `wait` below rendezvous back with the
    # workspace process; both exits must be 0.
    log "proposal-042: launching workspace regression + Swift lane in parallel"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test --workspace
    ) >"$workspace_log" 2>&1 &
    workspace_pid=$!

    # Swift lane runs in foreground so the operator sees progress. We
    # capture its exit status into an adjacent file.
    if "$0" proposal-042-swift 2>&1 | tee "$swift_parallel_log"; then
      echo 0 >"$swift_parallel_exit"
    else
      echo 1 >"$swift_parallel_exit"
    fi

    # Rendezvous with workspace regression.
    if wait "$workspace_pid"; then
      echo 0 >"$workspace_exit"
    else
      echo 1 >"$workspace_exit"
    fi

    swift_rc="$(cat "$swift_parallel_exit")"
    ws_rc="$(cat "$workspace_exit")"
    if [[ "$ws_rc" != "0" ]]; then
      echo "proposal-042: FAIL — cargo test --workspace exited $ws_rc"
      echo "  workspace log tail:"
      tail -30 "$workspace_log"
      exit 1
    fi
    if [[ "$swift_rc" != "0" ]]; then
      echo "proposal-042: FAIL — proposal-042-swift exited $swift_rc"
      echo "  swift lane log tail:"
      tail -30 "$swift_parallel_log"
      exit 1
    fi
    log "Proposal 042 control-plane gate passed"
    ;;
  proposal-042-swift|p042-swift)
    # P042 Swift-side focused lane: runs the three XCTest suites that
    # cover `DaemonLifecycleClient`, `DiagnosticsBundle`, and the
    # packaged-binary presence check. Requires Xcode + the `Chainworks
    # Forge` scheme. `PackagedBinaryTests` skips on dev builds; run it
    # under the Release configuration as part of the packaging lane.
    #
    # R10 READY-002: use a gate-owned `-derivedDataPath` so a
    # concurrent Xcode.app session or cargo process can't lock the
    # shared `Chainworks_Forge-*` DerivedData and spoil the build.
    # The directory is stamped with a timestamp so successive gate
    # runs don't trample each other either.
    log "Proposal 042 Swift gate: DaemonLifecycleClient + DiagnosticsBundle + PackagedBinary"
    swift_gate_stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    swift_gate_derived_data="${TMP_BASE}/p042-swift-${swift_gate_stamp}-DerivedData"
    # R13 disk-hygiene: each gate run stamps a fresh DerivedData dir
    # (~2.4 GB per run). Prune prior p042-swift-* stamps before the
    # new one is created so they don't silently accumulate — left
    # unchecked they had grown to ~45 GB across prior gate runs. Keep
    # the 2 most recent in case an in-flight build or the `xcresult`
    # viewer still has a handle on one.
    if [[ -d "$TMP_BASE" ]]; then
      compgen -G "$TMP_BASE/p042-swift-*-DerivedData" >/dev/null && \
      ls -dt "$TMP_BASE"/p042-swift-*-DerivedData 2>/dev/null \
        | tail -n +3 \
        | while read -r stale; do
            rm -rf "$stale"
          done
    fi
    mkdir -p "$swift_gate_derived_data"
    log "  gate-owned DerivedData: $swift_gate_derived_data"
    (
      cd "$ROOT_DIR"
      xcodebuild test \
        -scheme "Chainworks Forge" \
        -destination 'platform=macOS,arch=arm64' \
        -configuration Debug \
        -derivedDataPath "$swift_gate_derived_data" \
        -only-testing:"Chainworks ForgeTests/DaemonLifecycleClientTests" \
        -only-testing:"Chainworks ForgeTests/DiagnosticsBundleTests" \
        -only-testing:"Chainworks ForgeTests/PackagedBinaryTests" \
        -only-testing:"Chainworks ForgeTests/SupervisorTests" \
        -only-testing:"Chainworks ForgeTests/CrashBudgetResetTests" \
        2>&1
    )
    log "Proposal 042 Swift gate passed"
    ;;
  proposal-042-packaging|p042-packaging)
    # P042 §10.5 release-host lane. Implements the proposal's full
    # packaging scope (R11 READY-002 / REQ-017): build the Release
    # archive if one is not supplied, export it, validate signing /
    # authority / notarization / Gatekeeper, verify the Team ID
    # matches the release-host-owned allow list, and run a
    # launch-to-Ready proof on the packaged daemon. Each run writes an
    # evidence log under `docs/evidence/042-local-daemon-lifecycle/`
    # so a release can be audited after the fact.
    #
    # Safety interlock: the lane refuses to run unless the release
    # host explicitly opts in via `scripts/packaging.env` (see
    # `scripts/packaging.env.example` for the template). A developer
    # workstation that runs `./scripts/test-gate.sh proposal-042-packaging`
    # gets a clear "not on a release host" message and exit 2.
    packaging_env_file="$ROOT_DIR/scripts/packaging.env"
    if [[ ! -f "$packaging_env_file" ]]; then
      log "proposal-042-packaging: not on a release host"
      log "  scripts/packaging.env is missing. Copy scripts/packaging.env.example"
      log "  to scripts/packaging.env on the release host and fill in the"
      log "  P042_EXPECTED_TEAM_ID + notarization credentials."
      log "  On a developer workstation this is expected; run proposal-042"
      log "  for the Rust-side contract instead."
      exit 2
    fi
    # shellcheck disable=SC1090
    set -a
    . "$packaging_env_file"
    set +a
    if [[ "${P042_PACKAGING_RELEASE_HOST:-}" != "1" ]]; then
      log "proposal-042-packaging: not on a release host"
      log "  P042_PACKAGING_RELEASE_HOST must be '1' in scripts/packaging.env."
      exit 2
    fi
    if [[ -z "${P042_EXPECTED_TEAM_ID:-}" ]]; then
      echo "proposal-042-packaging: FAIL — P042_EXPECTED_TEAM_ID missing from packaging.env"
      exit 1
    fi

    evidence_dir="$ROOT_DIR/docs/evidence/042-local-daemon-lifecycle"
    mkdir -p "$evidence_dir"
    evidence_stamp="$(date -u +%Y%m%dT%H%M%SZ)"
    evidence_log="$evidence_dir/release-gate-${evidence_stamp}.log"
    log "proposal-042-packaging: writing evidence to $evidence_log"
    {
      echo "P042 §10.5 release packaging evidence"
      echo "timestamp_utc=$evidence_stamp"
      echo "git_sha=$(git -C "$ROOT_DIR" rev-parse HEAD 2>/dev/null || echo unknown)"
      echo "expected_team_id=${P042_EXPECTED_TEAM_ID}"
    } >"$evidence_log"

    # ── 1. Build + export the Release archive (unless supplied) ────────
    bundle="${P042_SIGNED_APP_BUNDLE:-}"
    if [[ -z "$bundle" ]]; then
      log "proposal-042-packaging: archiving fresh Release build"
      archive_stamp="${TMP_BASE}/p042-packaging-${evidence_stamp}"
      archive_path="${archive_stamp}/Chainworks Forge.xcarchive"
      export_dir="${archive_stamp}/Export"
      export_opts="${archive_stamp}/ExportOptions.plist"
      mkdir -p "$archive_stamp"
      cat >"$export_opts" <<PLIST
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
  <key>method</key><string>developer-id</string>
  <key>teamID</key><string>${P042_EXPECTED_TEAM_ID%%,*}</string>
  <key>signingStyle</key><string>manual</string>
</dict>
</plist>
PLIST
      {
        echo ""
        echo "=== archive ==="
        /usr/bin/xcodebuild archive \
          -project "$PROJECT_PATH" \
          -scheme "$SCHEME_NAME" \
          -configuration Release \
          -archivePath "$archive_path" 2>&1
        echo ""
        echo "=== exportArchive ==="
        /usr/bin/xcodebuild -exportArchive \
          -archivePath "$archive_path" \
          -exportPath "$export_dir" \
          -exportOptionsPlist "$export_opts" 2>&1
      } >>"$evidence_log" 2>&1
      bundle="$export_dir/Chainworks Forge.app"
      if [[ ! -d "$bundle" ]]; then
        echo "proposal-042-packaging: FAIL — export did not produce .app bundle at $bundle" | tee -a "$evidence_log"
        exit 1
      fi
    else
      echo "using pre-supplied bundle=$bundle" >>"$evidence_log"
    fi
    daemon_bin="$bundle/Contents/MacOS/chainworks-forge-daemon"
    if [[ ! -x "$daemon_bin" ]]; then
      echo "proposal-042-packaging: FAIL — embedded daemon binary missing or not executable: $daemon_bin" | tee -a "$evidence_log"
      exit 1
    fi

    # ── 2. Code signing + authority match ─────────────────────────────
    log "proposal-042-packaging: verifying bundle $bundle"
    {
      echo ""
      echo "=== codesign --verify --deep --strict (app) ==="
      /usr/bin/codesign --verify --deep --strict --verbose=4 "$bundle" 2>&1 || true
      echo ""
      echo "=== codesign -dvvv (app) ==="
      /usr/bin/codesign -dvvv "$bundle" 2>&1 || true
      echo ""
      echo "=== codesign -dvvv (daemon) ==="
      /usr/bin/codesign -dvvv "$daemon_bin" 2>&1 || true
    } >>"$evidence_log" 2>&1
    if ! /usr/bin/codesign --verify --deep --strict "$bundle"; then
      echo "proposal-042-packaging: FAIL — codesign --verify --deep --strict rejected the bundle" | tee -a "$evidence_log"
      exit 1
    fi
    app_authority="$(/usr/bin/codesign -dvvv "$bundle" 2>&1 | grep -E '^Authority=Developer ID Application' | head -1)"
    daemon_authority="$(/usr/bin/codesign -dvvv "$daemon_bin" 2>&1 | grep -E '^Authority=Developer ID Application' | head -1)"
    if [[ -z "$app_authority" ]]; then
      echo "proposal-042-packaging: FAIL — app bundle lacks Developer ID Application authority" | tee -a "$evidence_log"
      exit 1
    fi
    if [[ "$app_authority" != "$daemon_authority" ]]; then
      echo "proposal-042-packaging: FAIL — embedded daemon authority differs from app bundle" | tee -a "$evidence_log"
      printf "  app:    %s\n  daemon: %s\n" "$app_authority" "$daemon_authority" | tee -a "$evidence_log"
      exit 1
    fi

    # ── 3. Team ID allow-list match ───────────────────────────────────
    actual_team_id="$(/usr/bin/codesign -dvvv "$bundle" 2>&1 | awk -F'=' '/^TeamIdentifier=/ { print $2 }' | head -1)"
    echo "actual_team_id=$actual_team_id" >>"$evidence_log"
    if [[ -z "$actual_team_id" ]]; then
      echo "proposal-042-packaging: FAIL — could not read TeamIdentifier from signed bundle" | tee -a "$evidence_log"
      exit 1
    fi
    IFS=',' read -ra allowed_ids <<<"$P042_EXPECTED_TEAM_ID"
    team_id_ok=0
    for allowed in "${allowed_ids[@]}"; do
      if [[ "$actual_team_id" == "$allowed" ]]; then
        team_id_ok=1
        break
      fi
    done
    if [[ "$team_id_ok" != "1" ]]; then
      echo "proposal-042-packaging: FAIL — actual Team ID '$actual_team_id' not in P042_EXPECTED_TEAM_ID='$P042_EXPECTED_TEAM_ID'" | tee -a "$evidence_log"
      exit 1
    fi

    # ── 4. Notarization + Gatekeeper ──────────────────────────────────
    if ! /usr/bin/stapler validate "$bundle" >>"$evidence_log" 2>&1; then
      echo "proposal-042-packaging: FAIL — notarization staple missing or invalid" | tee -a "$evidence_log"
      exit 1
    fi
    if ! /usr/sbin/spctl --assess --type execute --verbose=4 "$bundle" >>"$evidence_log" 2>&1; then
      echo "proposal-042-packaging: FAIL — Gatekeeper rejected the bundle" | tee -a "$evidence_log"
      exit 1
    fi
    # Fetch notarization log if a submission id was supplied.
    if [[ -n "${P042_NOTARIZATION_SUBMISSION_ID:-}" \
       && -n "${P042_NOTARIZATION_APPLE_ID:-}" \
       && -n "${P042_NOTARIZATION_TEAM_ID:-}" \
       && -n "${P042_NOTARIZATION_PASSWORD_KEYCHAIN_PROFILE:-}" ]]; then
      {
        echo ""
        echo "=== notarytool log ==="
        /usr/bin/xcrun notarytool log "$P042_NOTARIZATION_SUBMISSION_ID" \
          --apple-id "$P042_NOTARIZATION_APPLE_ID" \
          --team-id "$P042_NOTARIZATION_TEAM_ID" \
          --keychain-profile "$P042_NOTARIZATION_PASSWORD_KEYCHAIN_PROFILE" 2>&1 || true
      } >>"$evidence_log" 2>&1
    fi

    # ── 5. Launch-to-Ready proof ──────────────────────────────────────
    # Spawn the packaged app, wait for the daemon to bind and report
    # `state == ready`, then terminate cleanly. A stalled Ready window
    # is the most common real-world ship blocker — the unit tests
    # prove the lifecycle types, this step proves the packaged daemon
    # actually reaches Ready end-to-end.
    log "proposal-042-packaging: launch-to-Ready proof"
    port_file="$HOME/Library/Application Support/Chainworks Forge/daemon.port"
    # Remove any stale port file so we can tell this run apart from a
    # previous sessions' residue.
    rm -f "$port_file"
    /usr/bin/open -a "$bundle"
    launched_at=$(date +%s)
    port=""
    for _ in $(seq 1 60); do
      if [[ -f "$port_file" ]]; then
        port="$(tr -d '[:space:]' <"$port_file")"
        break
      fi
      sleep 1
    done
    if [[ -z "$port" ]]; then
      echo "proposal-042-packaging: FAIL — daemon.port never appeared within 60s of app launch" | tee -a "$evidence_log"
      /usr/bin/osascript -e 'tell application "Chainworks Forge" to quit' >/dev/null 2>&1 || true
      exit 1
    fi
    echo "daemon_port=$port" >>"$evidence_log"
    ready_ok=0
    for _ in $(seq 1 30); do
      health="$(/usr/bin/curl -sf "http://127.0.0.1:${port}/health" || true)"
      if [[ "$health" == *'"state":"ready"'* ]] || [[ "$health" == *'"ready":true'* ]]; then
        ready_ok=1
        break
      fi
      sleep 1
    done
    echo "health_response=${health:-<empty>}" >>"$evidence_log"
    /usr/bin/osascript -e 'tell application "Chainworks Forge" to quit' >/dev/null 2>&1 || true
    if [[ "$ready_ok" != "1" ]]; then
      echo "proposal-042-packaging: FAIL — daemon did not reach state=ready within 30s" | tee -a "$evidence_log"
      exit 1
    fi
    launch_elapsed=$(( $(date +%s) - launched_at ))
    echo "launch_to_ready_elapsed_seconds=$launch_elapsed" >>"$evidence_log"

    echo "OVERALL=PASS" >>"$evidence_log"
    log "proposal-042-packaging: signing + Team ID + notarization + Gatekeeper + launch-to-Ready checks passed"
    log "proposal-042-packaging: evidence written to $evidence_log"
    ;;
  proposal-054|p054)
    log "Proposal 054 gate: implementation completeness and handoff contract"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p domain --test artifact_contracts -- --nocapture &&
      cargo test -p db --test integration artifact_contract_summary -- --nocapture &&
      cargo test -p db --test integration v1_fallback_retirement_check -- --nocapture &&
      cargo test -p workflow --test integration p054 -- --nocapture &&
      cargo test -p engine contracts::tests::validate_output_rejects_invalid_nested_v2_self_assessment_fields --lib -- --nocapture &&
      cargo test -p engine --test integration test_invoke_agent_imports_implementation_self_assessment_summary -- --exact --nocapture &&
      cargo test -p engine --test integration blocked_implementation_assessment_synthesizes_release_hold_review_summary -- --exact --nocapture &&
      cargo test -p graphql-server run_query_exposes_implementation_self_assessment_summary -- --nocapture &&
      cargo test -p mcp-server runs_get_returns_implementation_self_assessment_summary -- --nocapture &&
      cargo test -p mcp-server runs_list_includes_implementation_self_assessment_summary -- --nocapture
    )
    run_targeted_tests "proposal-054" "${PROPOSAL_054_SWIFT_TESTS[@]}"
    log "Proposal 054 gate passed"
    ;;
  proposal-075|p075)
    log "Proposal 075 gate: local persistence write budget, evidence spooling, and fail-closed registry"

    log "P075: db crate persistence contract tests (write_class, writer, allowlist, registry, spool refs)"
    (
      cd "$ROOT_DIR/control-plane"

      log "P075: write_class types — WriteClass, WriteOperation, WriteResult, SpoolWriteOutcome"
      cargo test -p db write_class:: -- --nocapture

      log "P075: writer — DbWriter constants, lane order, bounded executor, coalescing, shutdown"
      cargo test -p db writer:: -- --nocapture

      log "P075: bypass_allowlist — parser, expiry, canonical file"
      cargo test -p db bypass_allowlist:: -- --nocapture

      log "P075: operation_registry — parser, validation, canonical file"
      cargo test -p db operation_registry:: -- --nocapture

      log "P075: evidence_spool_refs — migration + repo round-trips, CHECK constraints"
      cargo test -p db repos::evidence_spool_refs:: -- --nocapture

      log "P075: db crate full regression (all db tests must pass)"
      cargo test -p db -- --nocapture

      log "P075: engine producer adoption — failed-stage evidence spools full packet and stores compact SQLite pointer"
      cargo test -p engine failed_stage_evidence_packet_tests -- --nocapture

      log "P075: daemon startup orphan sweep wiring"
      cargo test -p daemon storage_startup -- --nocapture

      log "P075: GraphQL typed storageHealth contract"
      cargo test -p graphql-server proposal_075_storage_health_is_typed_graphql_contract -- --nocapture
      cargo test -p graphql-server proposal_075_storage_health_reads_live_dbwriter_heartbeat -- --nocapture

      log "P075: auth capability boundary for operator-only storage diagnostics"
      cargo test -p auth sec004_observer_cannot_access_mcp_storage_diagnostics -- --nocapture

      log "P075: MCP storage diagnostics parameter semantics"
      cargo test -p mcp-server storage::tests:: -- --nocapture

      log "P075: MCP storage typed error contract (invalid_input, stale, unavailable, maintenance_disabled, unauthorized)"
      cargo test -p mcp-server storage::tests::reconcile_evidence_orphans_returns_invalid_input -- --nocapture
      cargo test -p mcp-server storage::tests::storage_health_returns_typed_stale_error -- --nocapture
      cargo test -p mcp-server storage::tests::reconcile_evidence_orphans_returns_maintenance_disabled -- --nocapture
      cargo test -p mcp-server storage::tests::typed_error_helper_produces_correct_shape -- --nocapture
      cargo test -p mcp-server proposal_075_storage_tool_dispatch -- --nocapture
    )

    # Fail-closed contract check: allowlist and operation registry must be present,
    # parseable, complete, non-expired for the current P075 closeout phase, and
    # every non-test db/src WriteOperation literal must be registered.
    P075_ALLOWLIST="$ROOT_DIR/control-plane/crates/db/write-bypass-allowlist.toml"
    P075_REGISTRY="$ROOT_DIR/control-plane/crates/db/write-operation-registry.toml"
    if [ ! -f "$P075_ALLOWLIST" ]; then
      fail "P075: write-bypass-allowlist.toml missing at $P075_ALLOWLIST"
    fi
    if [ ! -f "$P075_REGISTRY" ]; then
      fail "P075: write-operation-registry.toml missing at $P075_REGISTRY"
    fi
    python3 - "$ROOT_DIR" "$P075_ALLOWLIST" "$P075_REGISTRY" <<'PY'
import pathlib
import re
import sys
import tomllib

root = pathlib.Path(sys.argv[1])
allowlist_path = pathlib.Path(sys.argv[2])
registry_path = pathlib.Path(sys.argv[3])
current_phase = 8

allowlist = tomllib.loads(allowlist_path.read_text())
registry = tomllib.loads(registry_path.read_text())

required_bypass_fields = {
    "id", "owner", "reason", "scope", "path_pattern",
    "allowed_context", "retirement_criteria", "expires_after_phase",
}
allowed_scopes = {"migrations", "tests", "startup_repair"}
seen_ids = set()
for idx, row in enumerate(allowlist.get("bypasses", []), start=1):
    missing = sorted(required_bypass_fields - row.keys())
    if missing:
        raise SystemExit(f"P075 allowlist row {idx} missing fields: {missing}")
    if row["id"] in seen_ids:
        raise SystemExit(f"P075 duplicate bypass id: {row['id']}")
    seen_ids.add(row["id"])
    for field in required_bypass_fields - {"expires_after_phase"}:
        if not str(row[field]).strip():
            raise SystemExit(f"P075 bypass {row['id']} has empty {field}")
    if row["scope"] == "temporary_rollout":
        raise SystemExit(
            f"P075 bypass {row['id']} is a temporary_rollout entry; "
            "Phase 8 requires retiring all temporary rollout bypasses"
        )
    if row["scope"] not in allowed_scopes:
        raise SystemExit(f"P075 bypass {row['id']} has invalid scope {row['scope']}")
    if int(row["expires_after_phase"]) < current_phase:
        raise SystemExit(
            f"P075 bypass {row['id']} expired after phase {row['expires_after_phase']} "
            f"(current phase {current_phase})"
        )

required_operation_fields = {
    "operation_name", "class", "replay_policy",
    "idempotency_key_kind", "duplicate_application_test_path",
}
allowed_classes = {"A", "B", "C", "D"}
allowed_replay = {
    "natural_key", "last_writer_wins", "checksum_idempotent",
    "caller_guarded", "telemetry_merge",
}
registered = set()
class_by_operation = {}
replay_by_operation = {}
for idx, row in enumerate(registry.get("operations", []), start=1):
    missing = sorted(required_operation_fields - row.keys())
    if missing:
        raise SystemExit(f"P075 operation row {idx} missing fields: {missing}")
    name = str(row["operation_name"]).strip()
    if not name:
        raise SystemExit(f"P075 operation row {idx} has empty operation_name")
    if name in registered:
        raise SystemExit(f"P075 duplicate operation_name: {name}")
    registered.add(name)
    class_by_operation[name] = row["class"]
    replay_by_operation[name] = row["replay_policy"]
    if row["class"] not in allowed_classes:
        raise SystemExit(f"P075 operation {name} has invalid class {row['class']}")
    if row["replay_policy"] not in allowed_replay:
        raise SystemExit(f"P075 operation {name} has invalid replay_policy {row['replay_policy']}")
    if not str(row["idempotency_key_kind"]).strip():
        raise SystemExit(f"P075 operation {name} missing idempotency_key_kind")
    duplicate_path = str(row["duplicate_application_test_path"]).strip()
    if row["replay_policy"] == "caller_guarded":
        if not duplicate_path:
            raise SystemExit(f"P075 caller_guarded operation {name} missing duplicate test")
        if duplicate_path == "scripts/test-gate.sh::proposal-075_operation_registry_enforcement":
            raise SystemExit(
                f"P075 caller_guarded operation {name} uses generic duplicate proof path"
            )
        if (
            "operation-duplicate-application-matrix.md#" in duplicate_path
            and name.replace(".", "-").replace("_", "-") not in duplicate_path
        ):
            raise SystemExit(
                f"P075 caller_guarded operation {name} has non-specific duplicate proof path"
            )

expected_class_replay = {
    "projections.rebuild_approval_inbox": ("B", "last_writer_wins"),
    "projections.rebuild_run_summary": ("B", "last_writer_wins"),
    "projections.rebuild_stage_summaries": ("B", "last_writer_wins"),
    "projections.upsert_artifact_index_entry": ("B", "last_writer_wins"),
    "scheduler.record_db_writer_wait_observation": ("D", "telemetry_merge"),
    "storage_health.insert_write_pressure_snapshot": ("D", "telemetry_merge"),
}
for name, (expected_class, expected_replay) in expected_class_replay.items():
    if class_by_operation.get(name) != expected_class or replay_by_operation.get(name) != expected_replay:
        raise SystemExit(
            f"P075 operation {name} must be Class {expected_class}/{expected_replay}; "
            f"got {class_by_operation.get(name)}/{replay_by_operation.get(name)}"
        )

observed = set()
for rel_root in [
    "control-plane/crates/db/src",
    "control-plane/crates/engine/src",
    "control-plane/crates/mcp-server/src",
]:
    for path in (root / rel_root).rglob("*.rs"):
        if "tests" in path.parts:
            continue
        text = path.read_text().split("\n#[cfg(test)]", 1)[0]
        for match in re.finditer(
            r'(?:operation_name:\s*"([^"]+)"|class_a_operation\(\s*"([^"]+)"|begin_repository_transaction\(\s*pool\s*,\s*"([^"]+)"|execute_repository_write!\(\s*pool\s*,\s*"([^"]+)")',
            text,
        ):
            name = match.group(1) or match.group(2) or match.group(3) or match.group(4)
            if name.startswith("test_"):
                continue
            observed.add(name)
unregistered = sorted(observed - registered)
if unregistered:
    raise SystemExit(f"P075 unregistered WriteOperation.operation_name literals: {unregistered}")

runtime_direct_write_re = re.compile(
    r'(?:\bpool\.begin\(\)\.await\b|pool\.begin_with\(\s*"BEGIN IMMEDIATE"\s*\)|begin_immediate_with_retry\(|\.execute\((?:pool|&pool|&self\.pool)\))'
)
db_repo_direct_write_re = re.compile(
    r'(?:\bpool\.begin\(\)\.await\b|pool\.begin_with\(\s*"BEGIN IMMEDIATE"\s*\)|begin_immediate_with_retry\(|\.execute\((?:pool|&pool|&self\.pool)\))'
)
runtime_direct_write_sites = []
for rel_root in [
    "control-plane/crates/db/src/repos",
    "control-plane/crates/engine/src",
    "control-plane/crates/daemon/src",
    "control-plane/crates/graphql-server/src",
    "control-plane/crates/mcp-server/src",
]:
    for path in (root / rel_root).rglob("*.rs"):
        text = path.read_text().split("\n#[cfg(test)]", 1)[0]
        direct_write_re = db_repo_direct_write_re if rel_root == "control-plane/crates/db/src/repos" else runtime_direct_write_re
        for match in direct_write_re.finditer(text):
            line = text.count("\n", 0, match.start()) + 1
            runtime_direct_write_sites.append(
                f"{path.relative_to(root / 'control-plane').as_posix()}:{line}"
            )
if runtime_direct_write_sites:
    raise SystemExit(
        "P075 runtime direct SQL write sites must route through DbWriter: "
        + ", ".join(sorted(runtime_direct_write_sites))
    )

daemon_main = (root / "control-plane/crates/daemon/src/main.rs").read_text()
for required in [
    "db::writer::register_shared_writer(&pool, db_writer.clone()).await?",
    "CommandHandler::new_with_acp_capacity_and_db_writer",
    "Orchestrator::new_with_db_writer",
    "BackgroundExecutor::new_with_steward_runtime_inputs_and_db_writer",
    "RecoveryService::new_with_db_writer",
    "HostInterruptionService::with_capacity_config_runtime_cleanup_and_db_writer",
]:
    if required not in daemon_main:
        raise SystemExit(f"P075 daemon startup must register/inject shared DbWriter: missing {required}")

writer_text = (root / "control-plane/crates/db/src/writer.rs").read_text()
if "P075 shared DbWriter is not registered" not in writer_text:
    raise SystemExit("P075 file-backed registered transaction path must fail closed without shared DbWriter")
if "shared_writer_for(pool).await" not in writer_text:
    raise SystemExit("P075 registered transaction path must consult shared DbWriter registry")
if "insert_idempotent_via_dbwriter" in (root / "control-plane/crates/db/src/repos/evidence_spool_refs.rs").read_text():
    evidence_refs_text = (root / "control-plane/crates/db/src/repos/evidence_spool_refs.rs").read_text().split("\n#[cfg(test)]", 1)[0]
    via_plain_insert = evidence_refs_text.split("pub async fn insert_via_dbwriter", 1)[1].split("pub async fn", 1)[0]
    via_insert = evidence_refs_text.split("pub async fn insert_idempotent_via_dbwriter", 1)[1].split("pub async fn", 1)[0]
    via_update = evidence_refs_text.split("pub async fn update_status_via_dbwriter", 1)[1].split("pub async fn", 1)[0]
    for fn_name, body in [
        ("insert_via_dbwriter", via_plain_insert),
        ("insert_idempotent_via_dbwriter", via_insert),
        ("update_status_via_dbwriter", via_update),
    ]:
        if "begin_registered_immediate_transaction" in body or "insert_idempotent(&" in body:
            raise SystemExit(
                f"P075 {fn_name} must not re-enter registered transactions inside DbWriter work"
            )

local_writer_sites = []
for rel_root in [
    "control-plane/crates/engine/src",
    "control-plane/crates/mcp-server/src",
]:
    for path in (root / rel_root).rglob("*.rs"):
        text = path.read_text().split("\n#[cfg(test)]", 1)[0]
        for pattern in [r"let\s+local_writer\s*=", r"let\s+writer\s*=\s*db::writer::DbWriter::new\(pool\.clone\(\)\)"]:
            for match in re.finditer(pattern, text):
                local_writer_sites.append(
                    f"{path.relative_to(root / 'control-plane').as_posix()}:{text.count(chr(10), 0, match.start()) + 1}"
                )
if local_writer_sites:
    raise SystemExit(
        "P075 production runtime must not create per-call local DbWriter instances: "
        + ", ".join(sorted(local_writer_sites))
    )

raw_evidence_patterns = [
    r'update_evidence_packet_json\([^;]+&encoded',
    r'evidence_packet_json\s*=\s*\?\d+[^;]+encoded',
]
for path in (root / "control-plane/crates/engine/src").rglob("*.rs"):
    text = path.read_text()
    for pattern in raw_evidence_patterns:
        if re.search(pattern, text, re.DOTALL):
            rel = path.relative_to(root / "control-plane").as_posix()
            raise SystemExit(
                f"P075 raw high-volume evidence appears to be written directly to SQLite in {rel}"
            )

baseline_path = root / "docs/evidence/p075/phase1-baseline.md"
if not baseline_path.exists():
    raise SystemExit("P075 baseline evidence missing: docs/evidence/p075/phase1-baseline.md")
baseline_text = baseline_path.read_text()
if "pending_live_canary" in baseline_text:
    raise SystemExit("P075 baseline evidence still contains pending_live_canary")
for required in [
    "write_lock_wait_p50",
    "write_lock_wait_p95",
    "busy_retry_rate",
    "command_latency_p50",
    "command_latency_p95",
    "wal_size_bytes",
    "transactionDurationP50Ms",
    "transactionDurationP95Ms",
    "storage_health_file_backed_canary_reports_lock_wal_and_writer_metrics",
]:
    if required not in baseline_text:
        raise SystemExit(f"P075 baseline evidence missing required marker: {required}")
baseline_numeric_markers = [
    r"\|\s*write_lock_wait_p50\s*\|[^|]*\|[^|]*\|\s*0\s*\|",
    r"\|\s*write_lock_wait_p95\s*\|[^|]*\|[^|]*\|\s*1\s*\|",
    r"\|\s*busy_retry_rate\s*\|[^|]*\|[^|]*\|\s*0\.0\s*\|",
    r"\|\s*command_latency_p50\s*\|[^|]*\|[^|]*\|\s*0\s*\|",
    r"\|\s*command_latency_p95\s*\|[^|]*\|[^|]*\|\s*2\s*\|",
    r"\|\s*wal_size_bytes\s*\|[^|]*\|[^|]*\|\s*45352\s*\|",
]
for marker in baseline_numeric_markers:
    if not re.search(marker, baseline_text):
        raise SystemExit(f"P075 baseline evidence missing numeric baseline matching {marker}")

producer_inventory_path = root / "docs/evidence/p075/producer-inventory.md"
if not producer_inventory_path.exists():
    raise SystemExit("P075 high-volume evidence producer inventory is missing")
producer_inventory = producer_inventory_path.read_text()
for required in [
    "Failed-stage diagnostic packet",
    "ACP transcript capture",
    "tool_trace",
    "stdout",
    "stderr",
    "runtime_event",
    "model_delta",
    "delivery_readback",
]:
    if required not in producer_inventory:
        raise SystemExit(f"P075 producer inventory missing required marker: {required}")

print(
    f"P075 fail-closed registry check passed: "
    f"{len(seen_ids)} bypasses, {len(registered)} operations, "
    f"{len(observed)} observed db/src operation literals, "
    f"0 temporary rollout bypasses, runtime direct SQL scan clean"
)
PY

    log "Proposal 075 gate passed"
    ;;
  proposal-054-v1-retirement|p054-v1-retirement)
    if [[ -z "${DATABASE_URL:-}" ]]; then
      printf 'error: DATABASE_URL is required for proposal-054-v1-retirement\n' >&2
      exit 2
    fi
    log "Proposal 054 v1 fallback retirement release-cut check"
    (
      cd "$ROOT_DIR/control-plane"
      cargo run -p db --bin p054_v1_retirement_check
    )
    log "Proposal 054 v1 fallback retirement check passed"
    ;;
  proposal-084|p084)
    log "Proposal 084 gate: executable rollout gates and observability contract"
    python3 - <<'PY'
import json
import subprocess
import sys
from pathlib import Path

root = Path.cwd()

# AC-005: Fresh DB init must have unique SQLx migration versions.
migrations_dir = root / "control-plane/crates/db/migrations"
if not migrations_dir.exists():
    raise SystemExit("proposal-084: missing control-plane/crates/db/migrations")
versions = {}
for migration in sorted(migrations_dir.glob("*.sql")):
    prefix = migration.name.split("_", 1)[0]
    if prefix.isdigit():
        versions.setdefault(prefix, []).append(migration.name)
duplicates = {version: names for version, names in versions.items() if len(names) > 1}
if duplicates:
    details = "; ".join(
        f"{version}: {', '.join(names)}" for version, names in sorted(duplicates.items())
    )
    raise SystemExit(f"proposal-084: duplicate migration version prefix: {details}")

# AC-001: Template exists and contains required sections
template = root / "docs/reference/executable-rollout-gate-template.md"
if not template.exists():
    raise SystemExit("proposal-084: missing docs/reference/executable-rollout-gate-template.md")
text = template.read_text()
for required in [
    "rollout_contract_v1",
    "rollout_contract_check_v1",
    "operator_readback_v1",
    "schema_version",
    "applicability",
    "gate_aliases",
    "hold_conditions",
    "rollback_disposition",
    "metrics",
    "cutover",
    "safe path",
]:
    if required.lower() not in text.lower():
        raise SystemExit(
            f"proposal-084: template missing required section or term: {required!r}"
        )
status_line = next(
    (line for line in text.splitlines() if line.startswith("| `status` | enum |")),
    "",
)
for status in [
    "pass",
    "fail",
    "waived",
    "not_applicable",
    "timeout",
    "cancelled",
    "missing_contract",
    "tamper_detected",
    "stale",
]:
    if f"`{status}`" not in status_line:
        raise SystemExit(
            f"proposal-084: rollout_contract_check_v1 status enum missing {status!r}"
        )

# AC-002, AC-003, AC-007: Linter exists and correctly rejects negative fixtures
linter = root / "scripts/lint-rollout-contract"
if not linter.exists():
    raise SystemExit("proposal-084: missing scripts/lint-rollout-contract")

linter_negative_fixtures = [
    "docs/evidence/rollout-contract/negative/missing-hold-and-rollback.json",
    "docs/evidence/rollout-contract/negative/missing-metrics-p017-style.json",
    "docs/evidence/rollout-contract/negative/missing-operator-decision-fields.json",
    "docs/evidence/rollout-contract/negative/invalid-cutover-applicable-to.json",
    "docs/evidence/rollout-contract/negative/missing-required-surfaces.json",
    "docs/evidence/rollout-contract/negative/unsafe-path-and-command.json",
    "docs/evidence/rollout-contract/negative/windows-traversal-path.json",
]
for fixture_path in linter_negative_fixtures:
    full = root / fixture_path
    if not full.exists():
        raise SystemExit(f"proposal-084: missing negative fixture {fixture_path}")
    result = subprocess.run(
        [sys.executable, str(linter), str(full)],
        capture_output=True,
        text=True,
    )
    if result.returncode == 0:
        raise SystemExit(
            f"proposal-084: linter passed on negative fixture {fixture_path} "
            f"— expected failure but got: {result.stdout.strip()}"
        )

# Documentation-only negative fixtures: existence + valid JSON
doc_only_fixtures = [
    "docs/evidence/rollout-contract/negative/missing-template.json",
    "docs/evidence/rollout-contract/negative/run-start-missing-contract-enqueues-work.json",
    "docs/evidence/rollout-contract/negative/p084-self-contract-missing-readback-field.json",
]
for fixture_path in doc_only_fixtures:
    full = root / fixture_path
    if not full.exists():
        raise SystemExit(f"proposal-084: missing negative fixture {fixture_path}")
    try:
        json.loads(full.read_text())
    except json.JSONDecodeError as exc:
        raise SystemExit(f"proposal-084: invalid JSON in {fixture_path}: {exc}") from exc

# AC-004, AC-006: P084 self-contract fixture has all required operator readback fields
p084_fixture = (
    root / "docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json"
)
if not p084_fixture.exists():
    raise SystemExit(
        "proposal-084: missing "
        "docs/evidence/rollout-contract/operator-readback/p084-full-surface.fixture.json"
    )
try:
    fixture = json.loads(p084_fixture.read_text())
except json.JSONDecodeError as exc:
    raise SystemExit(f"proposal-084: invalid JSON in p084-full-surface.fixture.json: {exc}") from exc

required_fields = [
    "rollout_contract_status",
    "rollout_contract_decision",
    "rollout_contract_failure_reasons",
    "rollout_contract_waiver_state",
    "rollout_contract_waiver_expires_at",
    "rollout_contract_enforcement_mode",
    "rollout_contract_enforcement_mode_reason",
    "rollout_contract_hold_conditions",
    "rollout_contract_rollback_disposition",
    "rollout_contract_source_lane",
    "rollout_contract_enabled_state",
    "rollout_contract_disabled_reason_code",
    "rollout_contract_action_id",
    "rollout_contract_operator_message",
    "rollout_contract_projection_integrity",
    "rollout_contract_cutover_policy_revision",
    "rollout_contract_diagnostic_redaction",
    "rollout_contract_next_steps",
]
for field in required_fields:
    if field not in fixture:
        raise SystemExit(
            f"proposal-084: p084-full-surface.fixture.json missing required field: {field}"
        )

graphql_field_map = {
    "rollout_contract_status": "rolloutContractStatus",
    "rollout_contract_decision": "rolloutContractDecision",
    "rollout_contract_failure_reasons": "rolloutContractFailureReasons",
    "rollout_contract_waiver_state": "rolloutContractWaiverState",
    "rollout_contract_waiver_expires_at": "rolloutContractWaiverExpiresAt",
    "rollout_contract_enforcement_mode": "rolloutContractEnforcementMode",
    "rollout_contract_enforcement_mode_reason": "rolloutContractEnforcementModeReason",
    "rollout_contract_hold_conditions": "rolloutContractHoldConditions",
    "rollout_contract_rollback_disposition": "rolloutContractRollbackDisposition",
    "rollout_contract_source_lane": "rolloutContractSourceLane",
    "rollout_contract_enabled_state": "rolloutContractEnabledState",
    "rollout_contract_disabled_reason_code": "rolloutContractDisabledReasonCode",
    "rollout_contract_action_id": "rolloutContractActionId",
    "rollout_contract_operator_message": "rolloutContractOperatorMessage",
    "rollout_contract_projection_integrity": "rolloutContractProjectionIntegrity",
    "rollout_contract_cutover_policy_revision": "rolloutContractCutoverPolicyRevision",
    "rollout_contract_diagnostic_redaction": "rolloutContractDiagnosticRedaction",
    "rollout_contract_next_steps": "rolloutContractNextSteps",
}
parity_lanes = fixture.get("parity_lanes")
if not isinstance(parity_lanes, dict):
    raise SystemExit("proposal-084: p084-full-surface.fixture.json missing parity_lanes object")
for lane in ["mcp", "release_receipt"]:
    lane_payload = parity_lanes.get(lane)
    if not isinstance(lane_payload, dict):
        raise SystemExit(f"proposal-084: parity_lanes.{lane} must be an object")
    missing = [field for field in required_fields if field not in lane_payload]
    if missing:
        raise SystemExit(
            f"proposal-084: parity_lanes.{lane} missing required fields: {', '.join(missing)}"
        )
graphql_payload = parity_lanes.get("graphql")
if not isinstance(graphql_payload, dict):
    raise SystemExit("proposal-084: parity_lanes.graphql must be an object")
missing_graphql = [
    mapped for field, mapped in graphql_field_map.items() if mapped not in graphql_payload
]
if missing_graphql:
    raise SystemExit(
        "proposal-084: parity_lanes.graphql missing required camelCase fields: "
        + ", ".join(missing_graphql)
    )

# AC-005: Orchestrator must run rollout preflight before code_writer enqueue and
# fail closed by blocking the stage/run when the preflight action is Hold.
orchestrator = root / "control-plane/crates/engine/src/orchestrator.rs"
if not orchestrator.exists():
    raise SystemExit("proposal-084: missing control-plane/crates/engine/src/orchestrator.rs")
orchestrator_text = orchestrator.read_text()
for required in [
    "implementation_run_start_rollout_contract_preflight",
    "block_implementation_run_start_if_rollout_contract_hold",
    "refine_implementation_from_findings",
    "refine_implementation",
    "RolloutContractPreflightAction::Hold",
    "rollout_contract_preflight_hold",
    "stages::update_status(&self.pool, stage.id, StageStatus::Blocked)",
    "runs::update_status(&self.pool, run_id, RunStatus::Blocked)",
    "Rollout contract preflight held code_writer before enqueue",
    "return Ok(true);",
]:
    if required not in orchestrator_text:
        raise SystemExit(
            f"proposal-084: orchestrator missing AC-005 no-enqueue guard: {required!r}"
        )
preflight_boundary_index = orchestrator_text.find(
    "block_implementation_run_start_if_rollout_contract_hold("
)
worktree_provision_index = orchestrator_text.find("WorktreeProvisioner::provision(")
if preflight_boundary_index == -1 or worktree_provision_index == -1:
    raise SystemExit("proposal-084: orchestrator missing preflight/provisioning boundary markers")
if preflight_boundary_index > worktree_provision_index:
    raise SystemExit(
        "proposal-084: implementation run-start preflight must be reached before "
        "WorktreeProvisioner::provision"
    )

rollout_migration = root / "control-plane/crates/db/migrations/044_p084_rollout_contract.sql"
if not rollout_migration.exists():
    raise SystemExit(
        "proposal-084: missing control-plane/crates/db/migrations/044_p084_rollout_contract.sql"
    )
migration_text = rollout_migration.read_text()
if "redaction_state IN ('none', 'partial', 'full')" not in migration_text:
    raise SystemExit("proposal-084: rollout_contract_checks.redaction_state must be CHECK-constrained")

# AC-002: test-gates.md documents the gate
gates_doc = root / "docs/reference/test-gates.md"
if not gates_doc.exists():
    raise SystemExit("proposal-084: missing docs/reference/test-gates.md")
gates_text = gates_doc.read_text()
for required in [
    "### `proposal-084|p084`",
    "rollout_contract_v1",
    "negative fixture",
    "lint-rollout-contract",
]:
    if required not in gates_text:
        raise SystemExit(
            f"proposal-084: docs/reference/test-gates.md missing required content: {required!r}"
        )

print("proposal-084 all gate checks passed")
PY
    log "Proposal 084 gate: Rust rollout-contract preflight and migration regressions"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p engine rollout_contract_preflight --lib
      cargo test -p db rollout_contract_checks --lib
      cargo test -p db run_preflight_missing_db_clean_installs_and_applies_all
      cargo test -p db binary_schema_version_matches_migrator_max
    )
    run_targeted_tests "proposal-084" "${PROPOSAL_084_SWIFT_TESTS[@]}"
    log "Proposal 084 gate passed"
    ;;
  full)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "full"
    if [[ "${USE_TEST_PLANS:-1}" == "1" ]] && [[ -f "$TEST_PLANS_DIR/FullGate.xctestplan" ]]; then
      run_test_plan "full" "FullGate"
    else
      run_full_suite
    fi
    ;;
  proposal-077|p077)
    # P077 closeout readiness gate: Rust domain/db/engine unit and proof-gate
    # tests plus GraphQL/MCP readback parity through the shared accessor.
    # NOT covered: macOS UI/accessibility remote-host evidence.
    # See docs/reference/test-gates.md for narrowed coverage statement.
    require_p077_rollout_dependency_evidence
    require_p077_ui_evidence
    log "Proposal 077 closeout readiness gate (Rust domain/db/engine + GraphQL/MCP parity)"
    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-077-gate cargo test -p domain proposal_077_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-077-gate cargo test -p db closeout_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-077-gate cargo test -p db p077_rollout -- --nocapture
      CARGO_TARGET_DIR=target/proposal-077-gate cargo test -p engine proposal_077_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-077-gate cargo test -p graphql-server --test proposal_077_closeout_readback_parity -- --nocapture
      CARGO_TARGET_DIR=target/proposal-077-gate cargo test -p mcp-server --test proposal_077_closeout_readback_parity -- --nocapture
      CARGO_TARGET_DIR=target/proposal-077-gate cargo test --test p077_proof_gate -- --nocapture
    )
    log "Proposal 077 closeout readiness gate passed"
    ;;
  proposal-077-ui|p077-ui)
    check_idle_environment strict
    require_remote_ui_host
    prepare_codesign_keychain
    require_p077_ui_evidence
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    run_targeted_tests "proposal-077-ui" "${P077_UI_TESTS[@]}"
    log "Proposal 077 remote macOS closeout-readiness UI gate passed"
    ;;
  proposal-085|p085)
    log "Proposal 085 gate: thin-client read-model parity and affordance contract"
    python3 - <<'PY'
import json
import sys
from pathlib import Path

root = Path.cwd()

# 1. Contract document exists and contains required sections
contract_doc = root / "docs/reference/thin-client-read-model-affordance-contract.md"
if not contract_doc.exists():
    raise SystemExit(
        "proposal-085: missing docs/reference/thin-client-read-model-affordance-contract.md"
    )
contract_text = contract_doc.read_text()
for required in [
    "thin_client_affordance_contract_v1",
    "artifact.preview.listLabel",
    "artifact.preview.detail",
    "report.payload.metadata",
    "freshness.badge.run",
    "freshness.badge.stage",
    "freshness.badge.approval",
    "freshness.badge.artifact",
    "approval.resolve.approve",
    "approval.resolve.reject",
    "diagnostic.copy",
    "external.command.placeholder",
    "approveApproval",
    "rejectApproval",
    "payload_deferred",
    "metadata_only",
    "payloadAvailabilityState",
    "freshnessState",
    "disabledReasonCode",
    "writePathState",
    "diagnosticId",
    "P085AffordancePresenter",
    "canDrivePayloadAvailability",
    "canDriveApprovalActionability",
]:
    if required not in contract_text:
        raise SystemExit(
            f"proposal-085: contract doc missing required term: {required!r}"
        )

# 2. Negative fixtures exist as valid JSON
p085_negative_fixtures = [
    "docs/evidence/rollout-contract/negative/p085-approval-actionability-mismatch.json",
    "docs/evidence/rollout-contract/negative/p085-approval-stale-double-submit-conflict.json",
    "docs/evidence/rollout-contract/negative/p085-missing-affordance-row.json",
    "docs/evidence/rollout-contract/negative/p085-missing-schema-symbol.json",
    "docs/evidence/rollout-contract/negative/p085-payload-deferred-marked-unavailable.json",
    "docs/evidence/rollout-contract/negative/p085-payload-deferred-no-deadline.json",
    "docs/evidence/rollout-contract/negative/p085-unknown-enum-optimistic-action.json",
    "docs/evidence/rollout-contract/negative/p085-unsafe-local-truth-fallback.json",
]
for fixture_path in p085_negative_fixtures:
    full = root / fixture_path
    if not full.exists():
        raise SystemExit(f"proposal-085: missing negative fixture {fixture_path}")
    try:
        data = json.loads(full.read_text())
    except json.JSONDecodeError as exc:
        raise SystemExit(f"proposal-085: invalid JSON in {fixture_path}: {exc}") from exc
    if "contract_violation" not in data:
        raise SystemExit(
            f"proposal-085: negative fixture {fixture_path} missing 'contract_violation' field"
        )

# 3. test-gates.md documents the gate
gates_doc = root / "docs/reference/test-gates.md"
if not gates_doc.exists():
    raise SystemExit("proposal-085: missing docs/reference/test-gates.md")
gates_text = gates_doc.read_text()
for required in [
    "### `proposal-085|p085`",
    "thin_client_affordance_contract_v1",
    "P085AffordancePresenter",
    "negative fixture",
]:
    if required not in gates_text:
        raise SystemExit(
            f"proposal-085: docs/reference/test-gates.md missing required content: {required!r}"
        )

# 4. Swift presenter file exists
presenter = root / "Chainworks Forge/Support/P085AffordancePresenter.swift"
if not presenter.exists():
    raise SystemExit(
        "proposal-085: missing Chainworks Forge/Support/P085AffordancePresenter.swift"
    )
presenter_text = presenter.read_text()
for required in [
    "P085AffordancePresenter",
    "P085ArtifactAffordanceState",
    "P085ApprovalAffordanceState",
    "P085FreshnessAffordanceState",
    "P085DiagnosticAffordanceState",
    "canDrivePayloadAvailability",
    "canDriveApprovalActionability",
    "mergedAffordance",
    "payloadPresentation(fromRaw",
    "static func fromRaw",
    "case .unknown",
]:
    if required not in presenter_text:
        raise SystemExit(
            f"proposal-085: P085AffordancePresenter.swift missing required term: {required!r}"
        )

print("proposal-085 all gate checks passed")
PY
    run_targeted_tests "proposal-085" "${PROPOSAL_085_SWIFT_TESTS[@]}"
    log "Proposal 085 gate passed"
    ;;
  *)
    print_usage >&2
    die "Unknown gate: $GATE"
    ;;
esac
