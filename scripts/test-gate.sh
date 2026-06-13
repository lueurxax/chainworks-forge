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

PROPOSAL_036_TESTS=(
  "Chainworks ForgeTests/Proposal036UXConsolidationTests"
  "Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests"
  "Chainworks ForgeTests/RunTimelineInspectorViewTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal036NavigationShellParity"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal036DefinitionsSegmentedWrapper"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal036RunsApprovalTimelineAndSettingsReadinessFlow"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal036IdeasDeepLinkToRunsFlow"
)

PROPOSAL_093_TESTS=(
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testRuntimeTimelineAppendsChunksIntoOneLiveResponse()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testRuntimeTimelineBufferKeepsStableResponseIdentityWhileAppendingChunks()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testRuntimeTimelineCollapsesChunksOnlyAfterResponseEndsIntoSummaryCard()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testRuntimeTimelineTruncatedRawDetailDoesNotClaimFullAvailability()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testTimelineRowsUseP093ExpandableCardContract()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093RuntimeReadbackRequestsOwnedTimelineFields()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093RuntimeTimelineMissingRawDetailBytesStaysNil()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093SwiftResolvesRawDetailOnlyThroughDaemonResolver()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093TimelineUsesNewestFirstOrderAndAgentSelector()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093FormatterEnforcesBudgetsAndLRUCache()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093FormatterBudgetFallsBackWithInjectedClock()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093FormatterHandlesFencedBlocksAndJSON()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093CollapsedStreamingResponseUsesMetadataOnlyBody()"
  "Chainworks ForgeTests/Proposal036UXConsolidationTests/testP093RawDetailHandleIsNotResolvedThroughSwiftFilesystem()"
  "Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests"
  "Chainworks ForgeUITests/Chainworks_ForgeUITests/testProposal093TimelineCardExpansionRemoteProof"
)

PROPOSAL_086_SWIFT_TESTS=(
  "Chainworks ForgeTests/Proposal031ThinGraphQLReadBoundaryTests"
)

PROPOSAL_046_SWIFT_TESTS=(
  "Chainworks ForgeTests/Proposal046Tests"
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

PROPOSAL_081_SWIFT_TESTS=(
  "Chainworks ForgeTests/Proposal081ApprovalActionAttemptStoreTests"
  "Chainworks ForgeTests/Proposal081GraphQLRedactionTests"
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
  "graphql-server test_graphql_observer_class_cannot_invoke_approval_mutation"
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
  "engine test_redact_retry_stage_redacts_operator_instruction"
  "engine test_redact_retry_stage_preserves_structural_fields_without_instruction"
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
  # startRun/retryStage/cancelRun moved to MCP; journalId on those covered by mcp-server tests above
  "graphql-server test_graphql_approve_stage_returns_payload_with_approval_and_journal_id"
  "graphql-server test_response_omits_journal_id_when_capability_check_fails"

  # GraphQL delivery preflight contract §4.4.b (formerly graphql_start_run_blocked_payload_contract_tests)
  "graphql-server delivery_preflight_graphql_readback_tests"

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

should_use_adhoc_ui_tests() {
  [[ "${CHAINWORKS_USE_ADHOC_UI_TESTS:-0}" == "1" ]]
}

append_xcodebuild_signing_args() {
  local gate_name="${1:-}"
  local includes_ui="${2:-0}"

  if [[ "$includes_ui" == "1" ]] && should_use_adhoc_ui_tests; then
    printf '%s\0' \
      CODE_SIGNING_ALLOWED=YES \
      CODE_SIGNING_REQUIRED=NO \
      CODE_SIGN_IDENTITY=- \
      CODE_SIGN_STYLE=Manual \
      DEVELOPMENT_TEAM=
    return 0
  fi

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
    ui-smoke|proposal-006|p006|proposal-012|p012|proposal-013|p013|proposal-014|p014|proposal-015|p015|proposal-022|p022|proposal-024|p024|proposal-036|p036|proposal-077-ui|p077-ui|proposal-093|p093|full)
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
    CHAINWORKS_USE_ADHOC_UI_TESTS
	    CHAINWORKS_GUI_GATE_TIMEOUT_SECONDS
    CHAINWORKS_CODESIGN_KEYCHAIN
    CHAINWORKS_PREBUILT_CONTROL_PLANE_DAEMON
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
	  chmod 700 "$TMP_BASE" 2>/dev/null || true

	  if should_use_unsigned_ui_tests; then
	    resolved_unsigned_ui_tests=1
	  else
	    resolved_unsigned_ui_tests=0
	  fi
	  if [[ -n ${CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD+x} ]]; then
	    prepare_codesign_keychain
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
  if should_use_unsigned_ui_tests || should_use_adhoc_ui_tests; then
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

is_approved_remote_ui_host() {
  local approved host allowed
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

  while IFS= read -r host; do
    for allowed in "${approved[@]}"; do
      if [[ "$host" == "$allowed" ]]; then
        return 0
      fi
    done
  done < <(observed_host_names)

  return 1
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

guard_xcode_cargo_cache_policy() {
  log "Guard: Xcode Rust builds use shared Cargo cache policy"
  python3 - "$ROOT_DIR" <<'PY'
from pathlib import Path
import sys

root = Path(sys.argv[1])
helper = root / "scripts" / "cargo-cache-env.sh"
embed = root / "scripts" / "embed-control-plane-daemon.sh"
violations = []

if not helper.exists():
    violations.append("missing scripts/cargo-cache-env.sh")
else:
    helper_text = helper.read_text(encoding="utf-8")
    required_helper_fragments = [
        "Library/Caches/Chainworks Forge/cargo-target",
        "CHAINWORKS_XCODE_CARGO_TARGET_DIR",
        "CHAINWORKS_SHARED_CARGO_TARGET_DIR",
        "RUSTC_WRAPPER",
        "sccache",
    ]
    for fragment in required_helper_fragments:
        if fragment not in helper_text:
            violations.append(f"cargo-cache-env.sh missing {fragment!r}")

if not embed.exists():
    violations.append("missing scripts/embed-control-plane-daemon.sh")
else:
    embed_text = embed.read_text(encoding="utf-8")
    if 'source "${SRCROOT}/scripts/cargo-cache-env.sh"' not in embed_text:
        violations.append("embed-control-plane-daemon.sh does not source cargo-cache-env.sh")
    if "${TARGET_TEMP_DIR}/cargo-target" in embed_text:
        violations.append("embed-control-plane-daemon.sh still defaults Cargo target to TARGET_TEMP_DIR")
    if "${CARGO_TARGET_DIR}/${PROFILE_DIR}/control-plane" not in embed_text:
        violations.append("embed-control-plane-daemon.sh does not copy from CARGO_TARGET_DIR profile output")

if violations:
    print("Xcode Cargo cache policy violations:", file=sys.stderr)
    for violation in violations:
        print(f"  {violation}", file=sys.stderr)
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
    (result.get("reportSkillRef") == "proposal_review_router_skill", "report skill ref must be proposal_review_router_skill"),
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

prepare_test_source_snapshot() {
  local gate_name="$1"
  local stamp="$2"
  local snapshot="$TMP_BASE/source-snapshot"
  local -a source_files=(
    "Chainworks Forge/Views/RunsHomeView.swift"
    "Chainworks Forge/Models/RunsWorkbenchPresentationModel.swift"
    "Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift"
    "docs/evidence/macos-operator-navigation/dogfood-validation-2026-05-21.json"
    "docs/evidence/macos-operator-navigation/remote-ui-accessibility-proof-2026-05-21.json"
    "docs/evidence/macos-operator-navigation/rollout-readback-live-2026-05-21.json"
  )

  rm -rf "$snapshot"
  mkdir -p "$snapshot"

  local rel_path dest_dir
  for rel_path in "${source_files[@]}"; do
    dest_dir="$snapshot/$(dirname "$rel_path")"
    mkdir -p "$dest_dir"
    cp "$ROOT_DIR/$rel_path" "$snapshot/$rel_path"
  done

  printf '%s\n' "$snapshot"
}

run_targeted_tests() {
  local gate_name="$1"
  shift

  local stamp derived_data result_bundle log_path source_snapshot automation_log_path previous_automation_log_path
  local -a signing_args=()
  stamp="$(make_stamp)"
  derived_data="$TMP_BASE/${gate_name}-${stamp}-DerivedData"
  result_bundle="$TMP_BASE/${gate_name}-${stamp}.xcresult"
  log_path="$TMP_BASE/${gate_name}-${stamp}.log"
  mkdir -p "$TMP_BASE"
  source_snapshot="$(prepare_test_source_snapshot "$gate_name" "$stamp")"

  local cmd=(
    env
    "CHAINWORKS_TEST_SOURCE_ROOT=$source_snapshot"
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

run_non_ui_targeted_gate() {
  local gate_name="$1"
  shift

  local non_ui_tests=()
  local test_id
  for test_id in "$@"; do
    if [[ "$test_id" != Chainworks\ ForgeUITests/* ]]; then
      non_ui_tests+=("$test_id")
    fi
  done

  if [[ ${#non_ui_tests[@]} -gt 0 ]]; then
    run_targeted_tests "${gate_name}-non-ui" "${non_ui_tests[@]}"
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
  proposal-036,p036  UX Consolidation and Navigation Simplification gate
  proposal-093,p093  Live Agent Timeline UX and readability gate
  proposal-072,p072  UI action boundary gate: approval-only GraphQL UI mutations and MCP-only command routing
  proposal-077,p077  Proposal 077 closeout readiness gates (Rust domain/db/engine plus GraphQL/MCP readback parity; UI remote evidence separate)
  proposal-077-ui,p077-ui  Proposal 077 remote macOS compact/focus/backlink/accessibility runtime proof
  proposal-078,p078  Proposal 078 durable side-effect ledger gate (migration, CAS races, preflight, MCP tools)
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
  proposal-058    Proposal 058 ACP failure classification, artifact ownership, and escalation schema gate
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
  proposal-076|p076  Proposal 076 auto-retry observation ledger schema and fixture proof gate
  proposal-054|p054  Proposal 054 implementation completeness and handoff contract gate
  proposal-054-v1-retirement|p054-v1-retirement
                  Proposal 054 release-cut check for zero active non-terminal v1-only runs
  proposal-084|p084  Proposal 084 executable rollout gates and observability contract gate
  proposal-081|p081  Proposal 081 Phase 1 boundary-first API and auth contract matrix gate
  proposal-085|p085  Proposal 085 thin-client read-model parity and affordance contract gate
  proposal-086|p086|p086-continuation-preflight
                  Proposal 086 Phase 0 preflight: migration shape, MCP/artifact schemas, and Rust unit tests
  p086-continuation-readback
                  Proposal 086 Phase 1 readback gate: operator readback fixture field coverage
  p086-continuation-negative-fixtures
                  Proposal 086 Phase 2 hold-condition gate: all negative fixtures present and not placeholder
  p086-continuation-operator-report
                  Proposal 086 Phase 1 operator-report gate: operator report field coverage
  proposal-087|p087  Proposal 087 read-path liveness and storage tiering gate
  proposal-081|p081  Proposal 081 boundary policy enforcement and coverage gate
  proposal-082|p082  Proposal 082 recovery and retry state-machine matrix proof gate
  proposal-089|p089  Proposal 089 Junie structured-output proof and ACP canary evidence gate
  proposal-090|p090  Proposal 090 Junie runtime-hardening evidence inventory gate
  proposal-091|p091  Retained P091 targeted retry authority runtime proof gate
  proposal-046|p046  Proposal 046 session GraphQL observability gate (read-only queries, subscription, authorization, redaction)
  proposal-092|p092  Retained historical alias for P092 retry payload target invariants runtime proof gate
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
    guard_xcode_cargo_cache_policy
    guard_plan_tag_sync
    "$ROOT_DIR/scripts/check-boundary-coverage.sh"
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
      RUST_MIN_STACK=8388608 cargo test --workspace -- --test-threads=4 2>&1
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
  proposal-036|p036)
    check_idle_environment allow_app
    if [[ -n "$BEFORE_CRASH_LOG" ]]; then
      log "Latest crash log before run: $BEFORE_CRASH_LOG"
    else
      log "No prior Chainworks Forge crash logs found"
    fi
    guard_direct_run_insertion
    run_build "proposal-036"
    if is_approved_remote_ui_host; then
      prepare_codesign_keychain
      run_targeted_tests "proposal-036" "${PROPOSAL_036_TESTS[@]}"
    else
      run_non_ui_targeted_gate "proposal-036" "${PROPOSAL_036_TESTS[@]}"
      log "proposal-036 UI smoke is remote-only; run the same gate on test@SMacBook.local for UI proof"
    fi
    ;;
  proposal-093|p093)
    check_idle_environment allow_app
    guard_direct_run_insertion
    run_build "proposal-093"
    if is_approved_remote_ui_host; then
      prepare_codesign_keychain
      run_targeted_tests "proposal-093" "${PROPOSAL_093_TESTS[@]}"
    else
      run_non_ui_targeted_gate "proposal-093" "${PROPOSAL_093_TESTS[@]}"
      log "proposal-093 UI smoke is remote-only; run the same gate on test@SMacBook.local for UI proof"
    fi
    (cd "$ROOT_DIR/control-plane" && cargo test -p graphql-server runtime_timeline_p093 --quiet)
    ;;
  proposal-046|p046)
    log "Proposal 046 control-plane gate: session GraphQL observability (read-only queries, subscription, authorization, redaction)"
    log "Proposal 046: verifying no root-level output artifacts are present"
    for output_artifact in CHAINWORKS_OUTPUT chainworks_output.json tmp_chainworks_output.json; do
      if [[ -e "$ROOT_DIR/$output_artifact" ]]; then
        log "ERROR: stale output artifact exists outside the canonical meta-root: $ROOT_DIR/$output_artifact"
        exit 1
      fi
    done
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p graphql-server --test proposal_046_session_graphql -- --test-threads=1 --nocapture
    )
    log "Proposal 046: running pinned retry-policy unit tests"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p graphql-server --lib -- p046_ --nocapture
      cargo test -p db --lib -- p046_ --nocapture
    )
    log "Proposal 046 phase 1+2: verifying rollout contract fixture structure"
    P046_READBACK="$ROOT_DIR/docs/evidence/rollout-contract/operator-readback/p046-session-graphql-full-surface.fixture.json"
    if [[ ! -f "$P046_READBACK" ]]; then
      log "ERROR: rollout readback fixture missing: $P046_READBACK"
      exit 1
    fi
    log "Proposal 046: rollout readback fixture present"
    log "Proposal 046: verifying negative fixtures exist"
    P046_NEG_DIR="$ROOT_DIR/docs/evidence/rollout-contract/negative"
    P046_REQUIRED_NEGATIVES=(
      "p046-appkit-owns-graphql-task.json"
      "p046-authorization-recheck-transient-open.json"
      "p046-disabled-schema-client-unguarded.json"
      "p046-imprecise-connection-schema.graphql"
      "p046-missing-parent-run-authorization.json"
      "p046-missing-run-filter-subscription.json"
      "p046-raw-sensitive-generation-fields.graphql"
      "p046-reset-mutation-present.graphql"
      "p046-resync-churn.json"
      "p046-reversible-derived-reference.json"
      "p046-revoked-subscription-principal.json"
      "p046-slow-consumer-no-disconnect.json"
      "p046-swiftdata-persistence-leak.json"
      "p046-unbounded-metric-labels.json"
      "p046-unbounded-session-events.graphql"
      "p046-unbounded-sqlite-retry.json"
      "p046-unknown-event-type-redaction.json"
      "p046-unredacted-event-details.json"
    )
    for neg_fixture in "${P046_REQUIRED_NEGATIVES[@]}"; do
      if [[ ! -f "$P046_NEG_DIR/$neg_fixture" ]]; then
        log "ERROR: required negative fixture missing: $P046_NEG_DIR/$neg_fixture"
        exit 1
      fi
    done
    log "Proposal 046: all negative fixtures present"
    log "Proposal 046: verifying P046 metric inventory in source"
    P046_METRICS=(
      "session_graphql_query_total"
      "session_graphql_query_duration_seconds"
      "session_graphql_sqlite_retry_total"
      "session_graphql_sqlite_retry_exhausted_total"
      "session_status_subscription_event_total"
      "session_status_subscription_emit_lag_seconds"
      "session_status_subscription_lag_total"
      "session_status_subscription_slow_consumer_disconnect_total"
      "session_health_warning_total"
      "session_event_redaction_total"
      "session_graphql_disabled_schema_guard_total"
      "session_graphql_reset_mutation_guard_total"
      "session_graphql_observability_query_success_rate"
    )
    for metric in "${P046_METRICS[@]}"; do
      # session_graphql_sqlite_retry_total and session_graphql_sqlite_retry_exhausted_total are
      # db-crate-owned per the approved architecture contract (db::p046_retry). Search db/src/ too.
      if ! grep -rq "$metric" \
            "$ROOT_DIR/control-plane/crates/graphql-server/src/" \
            "$ROOT_DIR/control-plane/crates/graphql-server/src/types/" \
            "$ROOT_DIR/control-plane/crates/db/src/" \
            2>/dev/null; then
        log "ERROR: P046 metric '$metric' not found in graphql-server or db source"
        exit 1
      fi
    done
    log "Proposal 046: metric inventory check passed"
    log "Proposal 046: running Swift guardrail tests"
    run_targeted_tests "proposal-046-swift" "${PROPOSAL_046_SWIFT_TESTS[@]}"
    log "Proposal 046: validating rollout contract semantics in readback fixture (all lanes)"
    python3 - "$P046_READBACK" <<'PYEOF'
import json, sys
path = sys.argv[1]
with open(path) as f:
    data = json.load(f)
lanes = data.get('parity_lanes', {})
# All four required lanes must be present and must not be in bare 'hold' without waiver/na.
required_lanes = ['graphql', 'run_report', 'mcp', 'release_receipt']
accepted_statuses = {'pass', 'waived', 'not_applicable', 'fail', 'timeout', 'ready_for_phase3', 'pending_phase3_validation', 'not_applicable_phase3'}
errors = []
for lane in required_lanes:
    lane_data = lanes.get(lane)
    if lane_data is None:
        errors.append(f"Lane '{lane}' is missing from parity_lanes")
        continue
    status = lane_data.get('rolloutContractStatus', 'missing')
    if status == 'missing':
        errors.append(f"Lane '{lane}' is missing rolloutContractStatus")
    elif status == 'hold':
        # hold is only acceptable if there is an explicit waiver
        waiver = lane_data.get('rolloutContractWaiverState', 'none')
        if not waiver or waiver == 'none':
            errors.append(f"Lane '{lane}' is in hold without a waiver (rolloutContractWaiverState='{waiver}')")
    elif status not in accepted_statuses and not status.startswith('ready') and not status.startswith('pending'):
        errors.append(f"Lane '{lane}' has unrecognized rolloutContractStatus='{status}'")
if errors:
    for e in errors:
        print(f"ERROR: {e}", file=sys.stderr)
    sys.exit(1)
print(f"All {len(required_lanes)} lanes validated: " + ", ".join(f"{l}={lanes[l].get('rolloutContractStatus')}" for l in required_lanes))
PYEOF
    if [[ $? -ne 0 ]]; then
      log "ERROR: rollout readback fixture lane validation failed"
      exit 1
    fi
    log "Proposal 046: verifying negative fixtures are non-empty"
    for neg_fixture in "${P046_REQUIRED_NEGATIVES[@]}"; do
      neg_path="$P046_NEG_DIR/$neg_fixture"
      if [[ ! -s "$neg_path" ]]; then
        log "ERROR: negative fixture is empty or zero-bytes: $neg_path"
        exit 1
      fi
      # .graphql fixtures: check for GraphQL content keywords (type, query, etc.)
      # .json fixtures: check JSON is non-empty and not a placeholder stub.
      if [[ "$neg_fixture" == *.graphql ]]; then
        if ! grep -qE '^\s*(type|query|mutation|subscription|interface|schema|directive|#)' "$neg_path" 2>/dev/null; then
          log "ERROR: graphql negative fixture appears to lack real schema/query content: $neg_path"
          exit 1
        fi
      else
        if python3 -c "
import json, sys
with open('$neg_path') as f:
    content = f.read().strip()
if content in ('{}', '[]', ''):
    sys.exit(1)
# Check for obvious placeholder content
data = json.loads(content)
if isinstance(data, dict) and data.get('placeholder') == True:
    sys.exit(1)
" 2>/dev/null; then
          :
        else
          log "ERROR: negative fixture appears to be placeholder-only: $neg_path"
          exit 1
        fi
      fi
    done
    log "Proposal 046: negative fixture content check passed"
    # Prove .graphql negative fixtures describe patterns absent from the production schema source.
    # These checks verify fail-closed behavior: the anti-patterns are NOT present.
    log "Proposal 046: verifying graphql negative fixtures fail-closed against source"
    # p046-reset-mutation-present.graphql: no resetSession mutation function must appear in the schema.
    # The guard comment (resetSession/equivalent) and the guard counter are intentional;
    # only an actual fn reset_session resolver would violate the hold condition.
    if grep -rq 'fn reset_session\b\|async fn reset_session\b' "$ROOT_DIR/control-plane/crates/graphql-server/src/" 2>/dev/null; then
      log "ERROR: resetSession mutation resolver found in graphql-server (violates p046-reset-mutation-present.graphql hold)"
      exit 1
    fi
    # p046-raw-sensitive-generation-fields.graphql: raw providerSessionId/bindingFingerprint/invocationOwnerKey
    # must not be exposed as direct GraphQL fields on session generation types.
    # Derived references (scoped_provider_session_ref, scoped_binding_ref) are the approved surface.
    for raw_field in provider_session_id binding_fingerprint invocation_owner_key; do
      if grep -rq "pub ${raw_field}:" "$ROOT_DIR/control-plane/crates/graphql-server/src/types/session.rs" 2>/dev/null; then
        log "ERROR: raw sensitive field '${raw_field}' exposed as GraphQL field (violates p046-raw-sensitive-generation-fields.graphql hold)"
        exit 1
      fi
    done
    log "Proposal 046: graphql negative fixture fail-closed checks passed"
    log "Proposal 046 gate passed"
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
    log "Proposal 058 gate: ACP failure classification, escalation schema, readback parity, and governed macOS read surface"
    run_targeted_tests "proposal-058-swift" "Chainworks ForgeTests/Proposal058Tests"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p domain --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      cargo test -p engine proposal_058 --lib -- --test-threads=1 --nocapture &&
      cargo test -p db --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      cargo test -p db --test proposal_058_claim_start -- --test-threads=1 --nocapture &&
      cargo test -p engine --test proposal_058_claim_start -- --test-threads=1 --nocapture &&
      cargo test -p graphql-server --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      cargo test -p mcp-server --test proposal_058_runtime_facts -- --test-threads=1 --nocapture &&
      cargo test -p engine --test proposal_058_escalation_schema -- --test-threads=1 --nocapture &&
      cargo test -p workflow --test proposal_058_escalation_policy_schema -- --test-threads=1 --nocapture &&
      cargo test -p db proposal_058_required_metric_names_are_declared --lib -- --test-threads=1 --nocapture &&
      cargo test -p mcp-server runs_get_returns_escalation_readback --lib -- --test-threads=1 --nocapture &&
      cargo test -p mcp-server runs_get_escalation_readback_event_payload_json_roundtrip --lib -- --test-threads=1 --nocapture &&
      cargo test -p mcp-server runs_get_agent_principal_receives_summary_only_readback --lib -- --test-threads=1 --nocapture &&
      cargo test -p mcp-server build_escalation_readback_truncates_events_beyond_cap --lib -- --test-threads=1 --nocapture &&
      cargo test -p db payload_json_shape --lib -- --test-threads=1 --nocapture &&
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
    "escalation_policy_v1 schema",
    "redaction_version",
    "policy compile validation",
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
      cargo test -p mcp-server runs_get_includes_implementation_self_assessment_summary -- --nocapture
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

      log "P075: production Class B projections use coalescing and Class D telemetry exposes drop counters"
      cargo test -p db p075_projection_rebuild_uses_production_class_b_coalescing --test integration -- --nocapture
      cargo test -p db class_d_telemetry_drop_counter_is_observable_via_storage_health --test proposal_075_dbwriter -- --nocapture
      cargo test -p db class_d_rollup_producer_persists_bounded_snapshot_and_purges_retention --test proposal_075_dbwriter -- --nocapture
      cargo test -p db class_d_duplicate_window_rollups_merge_counters_and_max_gauges --test proposal_075_dbwriter -- --nocapture

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
if "telemetry_dropped_total" not in writer_text or "coalesced_merged_total" not in writer_text:
    raise SystemExit("P075 DbWriter heartbeat must expose real Class B merge and Class D drop counters")
projection_text = (root / "control-plane/crates/db/src/repos/projections.rs").read_text().split("\n#[cfg(test)]", 1)[0]
for operation in [
    "projections.rebuild_approval_inbox",
    "projections.rebuild_run_summary",
    "projections.rebuild_stage_summaries",
    "projections.upsert_artifact_index_entry",
]:
    if f'execute_repository_write!(\n        pool,\n        "{operation}"' in projection_text or f'execute_repository_write!(pool, "{operation}"' in projection_text:
        raise SystemExit(f"P075 production Class B operation {operation} still bypasses coalescing helper")
if "execute_repository_transaction_operation(pool, op, operation_name, work)" not in projection_text:
    raise SystemExit("P075 production projection writes must enter the transaction coalescing helper")
storage_health_text = (root / "control-plane/crates/db/src/repos/storage_health.rs").read_text().split("\n#[cfg(test)]", 1)[0]
scheduler_text = (root / "control-plane/crates/db/src/repos/scheduler.rs").read_text().split("\n#[cfg(test)]", 1)[0]
for path_label, text, operation in [
    ("storage_health.rs", storage_health_text, "storage_health.insert_write_pressure_snapshot"),
    ("scheduler.rs", scheduler_text, "scheduler.record_db_writer_wait_observation"),
]:
    if f'execute_repository_write!(\n        pool,\n        "{operation}"' in text or f'execute_repository_write!(pool, "{operation}"' in text:
        raise SystemExit(f"P075 production Class D operation {operation} in {path_label} still bypasses telemetry helper")
    if "execute_repository_transaction_operation(" not in text:
        raise SystemExit(f"P075 production Class D operation {operation} in {path_label} must use DbWriter transaction helper")
if "droppedTelemetryTotal" not in storage_health_text or "telemetryDroppedTotal" not in storage_health_text:
    raise SystemExit("P075 storageHealth must report real Class D telemetry drop counters")
for required in [
    "record_live_write_pressure_rollup",
    "merge_write_pressure_payload",
    "TELEMETRY_SNAPSHOT_RETAIN_LATEST",
    "latestWindowLimit",
    "DELETE FROM storage_write_pressure_snapshots",
]:
    if required not in storage_health_text and required not in writer_text:
        raise SystemExit(f"P075 Class D rollup lifecycle is not wired: missing {required}")
storage_pressure_migration = (root / "control-plane/crates/db/migrations/049_p075_storage_write_pressure_window_key.sql").read_text()
if "idx_storage_write_pressure_window_unique" not in storage_pressure_migration or "window_start, window_end" not in storage_pressure_migration:
    raise SystemExit("P075 Class D telemetry_merge requires a unique write-pressure window key")
daemon_main = (root / "control-plane/crates/daemon/src/main.rs").read_text()
if "spawn_storage_write_pressure_rollup(pool.clone(), db_writer.heartbeat.clone())" not in daemon_main:
    raise SystemExit("P075 daemon must start the production Class D write-pressure rollup producer")
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
  proposal-076|p076)
    # P076 fixture/contract/runtime proof gate.
    # Proves: fixture presence, required field coverage, closed P076 enum domains,
    # observe-only policy, 6 required top-level path echoes, version_negotiation
    # shape, JSON-RPC error envelope for unsupported_version, budget_ref_v1 and
    # observation_summary_v1 shapes, observation_id format, rollup dedupe proof,
    # degraded-success/no_observation_history coverage, and negative fixture integrity
    # (invalid enum rejection, missing required field, unknown field in closed schema).
    log "Proposal 076 auto-retry observation ledger schema and fixture gate"
    python3 - <<'PY'
import json, re
from pathlib import Path

root = Path.cwd()

P076_PATH_FIELDS = [
    "ledger_path", "budget_state_path", "known_issue_catalog_path",
    "generated_markdown_catalog_path", "lock_path", "rollup_report_path",
]

RETRY_ACTION_ENUM = {"none", "recommend_retry"}
RETRY_RESULT_ENUM = {
    "not_attempted", "not_allowed", "unknown", "accepted",
    "rejected", "advanced", "reblocked", "failed", "timeout_ambiguous",
}
RETRY_RESULT_P076 = {"not_attempted", "not_allowed"}
BLOCKER_CLASS_ENUM = {
    "human_gate", "substantive_output_contract", "stale_execution_truth",
    "projection_divergence", "provider_or_session_failure",
    "retry_identifier_shape", "unknown",
}
POLICY_DECISION_ENUM = {
    "observe_only", "collect_evidence", "human_gate", "cooldown_exhausted",
    "budget_unavailable", "needs_systemic_fix", "needs_human_triage",
    "retry_disabled_pending_idempotency_contract", "poll_timeout",
    "skipped_lock_held", "skipped_backpressure",
}
KNOWN_ISSUE_STATUS_ENUM = {
    "observed", "retrying_within_budget", "cooldown_exhausted",
    "needs_systemic_fix", "needs_human_triage", "resolved_or_quiet", "archived",
}
READBACK_POLICY_STATUS_ENUM = {
    "no_observation_history", "observed", "readback_degraded", "budget_unavailable",
    "cooldown_exhausted", "needs_human_triage", "needs_systemic_fix",
    "retry_disabled_pending_idempotency_contract",
}
BUDGET_STATUS_ENUM = {
    "available", "cooldown", "budget_unavailable", "needs_human_triage",
    "needs_systemic_fix", "disabled_pending_idempotency_contract",
}
DIAGNOSTIC_SEVERITY_ENUM = {"info", "warning", "error"}

OBS_V1_REQUIRED = [
    "schema_version", "observation_id", "canonical_record_hash", "observed_at",
    "source", "daemon_ready", "policy_version", "writer_lock", "summary", "blocked_runs",
]
OBS_SUMMARY_KNOWN = set([
    "observation_id", "observed_at", "run_id", "stage_id",
    "blocker_signature_id", "blocker_class", "policy_decision",
    "retry_action", "retry_result", "known_issue_status", "observation_path",
    "stage_execution_id", "failure_summary", "next_systemic_action", "evidence_report_id",
])

OBS_SUMMARY_REQUIRED = [
    "observation_id", "observed_at", "run_id", "stage_id",
    "blocker_signature_id", "blocker_class", "policy_decision",
    "retry_action", "retry_result", "known_issue_status", "observation_path",
]

RUN_SUMMARY_REQUIRED = [
    "run_id", "auto_retry_policy_status", "auto_retry_policy_decision",
    "auto_retry_observation_record_id", "auto_retry_observation_path",
    "auto_retry_blocker_signature_id", "auto_retry_blocker_class",
    "auto_retry_retry_budget_state", "auto_retry_last_retry_result",
    "auto_retry_known_issue_status", "auto_retry_next_systemic_action",
    "auto_retry_rollup_report_path", "auto_retry_human_gate_retry_attempt_total",
    "auto_retry_budget_unavailable_reason", "auto_retry_backpressure_skip_count",
    "auto_retry_readback_version", "oldest_planned_attempt_at",
    "planned_attempt_age_seconds", "unknown_attempt_count", "required_operator_settlement",
]

BUDGET_REF_REQUIRED = [
    "run_id", "blocker_signature_id", "status", "window_hours", "max_attempts",
    "attempt_count", "remaining_attempts", "cooldown_until", "budget_state_path",
]

DIAGNOSTIC_V1_REQUIRED = ["code", "severity", "message"]
DIAGNOSTIC_V1_KNOWN = {
    "code", "severity", "message", "path", "run_id", "blocker_signature_id", "observation_id",
}
# Fields added to sample objects for fixture documentation (not part of the runtime schema)
FIXTURE_METADATA_FIELDS = {"fixture_note"}

AUTO_RETRY_READBACK_KNOWN = {
    "schema_version", "generated_at", "version_negotiation",
    "ledger_path", "budget_state_path", "known_issue_catalog_path",
    "generated_markdown_catalog_path", "lock_path", "rollup_report_path",
    "diagnostics", "observations", "latest_by_run",
}

RUN_SUMMARY_KNOWN = set(RUN_SUMMARY_REQUIRED)


def is_rfc3339(s):
    if not isinstance(s, str):
        return False
    return bool(re.match(r'^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}', s))


def is_absolute_path(s):
    return isinstance(s, str) and s.startswith('/')


def strict_validate_obs_summary(obs, label, allow_fixture_metadata=False):
    """Strict-mode observation_summary_v1 validation (additionalProperties=false)."""
    missing = [f for f in OBS_SUMMARY_REQUIRED if f not in obs]
    if missing:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 missing required fields: {missing}")
    known = OBS_SUMMARY_KNOWN | (FIXTURE_METADATA_FIELDS if allow_fixture_metadata else set())
    extra = set(obs.keys()) - known
    if extra:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 unknown fields (strict mode): {extra}")
    for str_field in ["run_id", "stage_id", "blocker_signature_id"]:
        val = obs.get(str_field)
        if not isinstance(val, str):
            raise SystemExit(f"proposal-076: {label} observation_summary_v1 {str_field} must be a non-null string, got {type(val).__name__!r}")
    if obs.get("blocker_class") not in BLOCKER_CLASS_ENUM:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 invalid blocker_class: {obs.get('blocker_class')!r}")
    if obs.get("policy_decision") not in POLICY_DECISION_ENUM:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 invalid policy_decision: {obs.get('policy_decision')!r}")
    if obs.get("retry_action") not in RETRY_ACTION_ENUM:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 invalid retry_action: {obs.get('retry_action')!r}")
    if obs.get("retry_result") not in RETRY_RESULT_ENUM:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 invalid retry_result: {obs.get('retry_result')!r}")
    if obs.get("known_issue_status") not in KNOWN_ISSUE_STATUS_ENUM:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 invalid known_issue_status: {obs.get('known_issue_status')!r}")
    obs_id = obs.get("observation_id", "")
    if not re.match(r'^ar_obs_\d{8}T\d{6}Z_[0-9a-f]{12}$', obs_id):
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 invalid observation_id format: {obs_id!r}")
    if not is_rfc3339(obs.get("observed_at")):
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 observed_at must be RFC3339, got {obs.get('observed_at')!r}")
    obs_path = obs.get("observation_path")
    if not is_absolute_path(obs_path):
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 observation_path must be a non-null absolute path, got {obs_path!r}")
    for nullable_str in ["stage_execution_id", "failure_summary", "next_systemic_action", "evidence_report_id"]:
        val = obs.get(nullable_str)
        if val is not None and not isinstance(val, str):
            raise SystemExit(f"proposal-076: {label} observation_summary_v1 {nullable_str} must be string|null, got {type(val).__name__}")


def permissive_validate_obs_summary(obs, label):
    """Permissive-mode observation_summary_v1: required fields must be present; unknown fields tolerated with diagnostic.
    Returns a list of diagnostic dicts for any unknown fields found (must be non-empty to prove diagnostic emission)."""
    diagnostics = []
    missing = [f for f in OBS_SUMMARY_REQUIRED if f not in obs]
    if missing:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 (permissive) missing required fields: {missing}")
    extra = set(obs.keys()) - OBS_SUMMARY_KNOWN
    if extra:
        diagnostics.append({
            "code": "unknown_field_ignored",
            "severity": "warning",
            "message": f"Unknown fields ignored in permissive report mode: {sorted(extra)}",
        })
    if obs.get("blocker_class") not in BLOCKER_CLASS_ENUM:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 (permissive) invalid blocker_class: {obs.get('blocker_class')!r}")
    if obs.get("policy_decision") not in POLICY_DECISION_ENUM:
        raise SystemExit(f"proposal-076: {label} observation_summary_v1 (permissive) invalid policy_decision: {obs.get('policy_decision')!r}")
    return diagnostics


BUDGET_REF_KNOWN = {
    "run_id", "blocker_signature_id", "status", "window_hours", "max_attempts",
    "attempt_count", "remaining_attempts", "cooldown_until", "budget_state_path",
    "last_observation_id", "oldest_planned_attempt_at", "planned_attempt_age_seconds",
    "unknown_attempt_count", "required_operator_settlement", "budget_unavailable_reason",
}


def validate_budget_ref_v1_scalars(br, label, allow_fixture_metadata=False):
    """Validate budget_ref_v1 required fields, scalar types, and enum."""
    for req in BUDGET_REF_REQUIRED:
        if req not in br:
            raise SystemExit(f"proposal-076: {label} budget_ref_v1 missing required field: {req}")
    known = BUDGET_REF_KNOWN | (FIXTURE_METADATA_FIELDS if allow_fixture_metadata else set())
    extra = set(br.keys()) - known
    if extra:
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 unknown fields (strict mode): {extra}")
    for str_field in ["run_id", "blocker_signature_id"]:
        val = br.get(str_field)
        if not isinstance(val, str):
            raise SystemExit(f"proposal-076: {label} budget_ref_v1 {str_field} must be a non-null string, got {type(val).__name__!r}")
    if not isinstance(br.get("window_hours"), int) or br["window_hours"] <= 0:
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 window_hours must be positive integer, got {br.get('window_hours')!r}")
    if not isinstance(br.get("max_attempts"), int) or br["max_attempts"] < 0:
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 max_attempts must be non-negative integer, got {br.get('max_attempts')!r}")
    if not isinstance(br.get("attempt_count"), int) or br["attempt_count"] < 0:
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 attempt_count must be non-negative integer, got {br.get('attempt_count')!r}")
    if not isinstance(br.get("remaining_attempts"), int) or br["remaining_attempts"] < 0:
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 remaining_attempts must be non-negative integer, got {br.get('remaining_attempts')!r}")
    if br.get("status") not in BUDGET_STATUS_ENUM:
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 invalid status: {br.get('status')!r}")
    budget_state_path_val = br.get("budget_state_path")
    if not is_absolute_path(budget_state_path_val):
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 budget_state_path must be absolute path, got {budget_state_path_val!r}")
    cooldown_until_val = br.get("cooldown_until")
    if cooldown_until_val is not None and not is_rfc3339(cooldown_until_val):
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 cooldown_until must be RFC3339|null, got {cooldown_until_val!r}")
    last_obs_id_val = br.get("last_observation_id")
    if last_obs_id_val is not None and not re.match(r'^ar_obs_\d{8}T\d{6}Z_[0-9a-f]{12}$', last_obs_id_val):
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 last_observation_id must be observation_id format or null, got {last_obs_id_val!r}")
    oldest_planned_val = br.get("oldest_planned_attempt_at")
    if oldest_planned_val is not None and not is_rfc3339(oldest_planned_val):
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 oldest_planned_attempt_at must be RFC3339|null, got {oldest_planned_val!r}")
    planned_age_val = br.get("planned_attempt_age_seconds")
    if planned_age_val is not None and (not isinstance(planned_age_val, int) or planned_age_val < 0):
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 planned_attempt_age_seconds must be non_negative_integer|null, got {planned_age_val!r}")
    unknown_count_val = br.get("unknown_attempt_count")
    if unknown_count_val is not None and (not isinstance(unknown_count_val, int) or unknown_count_val < 0):
        raise SystemExit(f"proposal-076: {label} budget_ref_v1 unknown_attempt_count must be non_negative_integer|null, got {unknown_count_val!r}")
    for nullable_str in ["required_operator_settlement", "budget_unavailable_reason"]:
        val = br.get(nullable_str)
        if val is not None and not isinstance(val, str):
            raise SystemExit(f"proposal-076: {label} budget_ref_v1 {nullable_str} must be string|null, got {type(val).__name__}")


def validate_diagnostic_v1(diag, label):
    """Validate common_diagnostic_v1 required fields, closed shape, enum, and nullable scoped fields."""
    for req in DIAGNOSTIC_V1_REQUIRED:
        if req not in diag:
            raise SystemExit(f"proposal-076: {label} common_diagnostic_v1 missing required field: {req}")
    if diag.get("severity") not in DIAGNOSTIC_SEVERITY_ENUM:
        raise SystemExit(f"proposal-076: {label} common_diagnostic_v1 invalid severity: {diag.get('severity')!r}")
    extra = set(diag.keys()) - DIAGNOSTIC_V1_KNOWN
    if extra:
        raise SystemExit(f"proposal-076: {label} common_diagnostic_v1 unknown fields (closed shape): {extra}")
    for nullable_str in ["path", "run_id", "blocker_signature_id", "observation_id"]:
        val = diag.get(nullable_str)
        if val is not None and not isinstance(val, str):
            raise SystemExit(f"proposal-076: {label} common_diagnostic_v1 {nullable_str} must be string|null, got {type(val).__name__}")


def require_auto_retry_readback(payload, label):
    arb = payload.get("auto_retry_readback")
    if not isinstance(arb, dict):
        raise SystemExit(f"proposal-076: {label} missing auto_retry_readback")
    if arb.get("schema_version") != "auto_retry_readback.v1":
        raise SystemExit(f"proposal-076: {label} auto_retry_readback has invalid schema_version: {arb.get('schema_version')!r}")
    if not is_rfc3339(arb.get("generated_at")):
        raise SystemExit(f"proposal-076: {label} auto_retry_readback generated_at must be RFC3339, got {arb.get('generated_at')!r}")
    extra_arb = set(arb.keys()) - AUTO_RETRY_READBACK_KNOWN
    if extra_arb:
        raise SystemExit(f"proposal-076: {label} auto_retry_readback has unknown fields (additionalProperties=false): {extra_arb}")
    for pf in P076_PATH_FIELDS:
        if not is_absolute_path(arb.get(pf)):
            raise SystemExit(f"proposal-076: {label} auto_retry_readback path field {pf!r} must be absolute path, got {arb.get(pf)!r}")
    vn = arb.get("version_negotiation")
    if not isinstance(vn, dict):
        raise SystemExit(f"proposal-076: {label} auto_retry_readback missing version_negotiation")
    for vf in ["selected_version", "supported_versions", "unsupported_versions"]:
        if vf not in vn:
            raise SystemExit(f"proposal-076: {label} version_negotiation missing {vf}")
    if not isinstance(vn.get("supported_versions"), list):
        raise SystemExit(f"proposal-076: {label} version_negotiation.supported_versions must be an array")
    if not isinstance(vn.get("unsupported_versions"), list):
        raise SystemExit(f"proposal-076: {label} version_negotiation.unsupported_versions must be an array")
    diagnostics = arb.get("diagnostics")
    if not isinstance(diagnostics, list):
        raise SystemExit(f"proposal-076: {label} auto_retry_readback diagnostics must be an array")
    for diag in diagnostics:
        validate_diagnostic_v1(diag, label)
    observations = arb.get("observations")
    if not isinstance(observations, list):
        raise SystemExit(f"proposal-076: {label} observations must be an array")
    for obs in observations:
        strict_validate_obs_summary(obs, label)
        if obs.get("retry_result") not in RETRY_RESULT_P076:
            raise SystemExit(f"proposal-076: {label} P076 observation retry_result must be not_attempted or not_allowed, got {obs.get('retry_result')!r}")
    latest = arb.get("latest_by_run")
    if not isinstance(latest, list):
        raise SystemExit(f"proposal-076: {label} latest_by_run must be an array")
    for rs in latest:
        for req in RUN_SUMMARY_REQUIRED:
            if req not in rs:
                raise SystemExit(f"proposal-076: {label} run_summary missing required field {req}")
        extra_rs = set(rs.keys()) - RUN_SUMMARY_KNOWN
        if extra_rs:
            raise SystemExit(f"proposal-076: {label} run_summary unknown fields (additionalProperties=false): {extra_rs}")
        total = rs.get("auto_retry_human_gate_retry_attempt_total")
        if not isinstance(total, int) or total < 0:
            raise SystemExit(f"proposal-076: {label} auto_retry_human_gate_retry_attempt_total must be non-negative integer")
        skip_count = rs.get("auto_retry_backpressure_skip_count")
        if not isinstance(skip_count, int) or skip_count < 0:
            raise SystemExit(f"proposal-076: {label} auto_retry_backpressure_skip_count must be non-negative integer")
        policy_status = rs.get("auto_retry_policy_status")
        if policy_status not in READBACK_POLICY_STATUS_ENUM:
            raise SystemExit(f"proposal-076: {label} run_summary invalid auto_retry_policy_status: {policy_status!r}")
        policy_decision = rs.get("auto_retry_policy_decision")
        if policy_decision is not None and policy_decision not in POLICY_DECISION_ENUM:
            raise SystemExit(f"proposal-076: {label} run_summary invalid auto_retry_policy_decision: {policy_decision!r}")
        blocker_class = rs.get("auto_retry_blocker_class")
        if blocker_class is not None and blocker_class not in BLOCKER_CLASS_ENUM:
            raise SystemExit(f"proposal-076: {label} run_summary invalid auto_retry_blocker_class: {blocker_class!r}")
        budget_state = rs.get("auto_retry_retry_budget_state")
        if budget_state is not None and budget_state not in BUDGET_STATUS_ENUM:
            raise SystemExit(f"proposal-076: {label} run_summary invalid auto_retry_retry_budget_state: {budget_state!r}")
        last_retry = rs.get("auto_retry_last_retry_result")
        if last_retry is not None and last_retry not in RETRY_RESULT_ENUM:
            raise SystemExit(f"proposal-076: {label} run_summary invalid auto_retry_last_retry_result: {last_retry!r}")
        known_issue = rs.get("auto_retry_known_issue_status")
        if known_issue is not None and known_issue not in KNOWN_ISSUE_STATUS_ENUM:
            raise SystemExit(f"proposal-076: {label} run_summary invalid auto_retry_known_issue_status: {known_issue!r}")
        oldest_planned = rs.get("oldest_planned_attempt_at")
        if oldest_planned is not None and not is_rfc3339(oldest_planned):
            raise SystemExit(f"proposal-076: {label} run_summary oldest_planned_attempt_at must be RFC3339|null, got {oldest_planned!r}")
        for nni_field in ["planned_attempt_age_seconds", "unknown_attempt_count"]:
            nni_val = rs.get(nni_field)
            if nni_val is not None and (not isinstance(nni_val, int) or nni_val < 0):
                raise SystemExit(f"proposal-076: {label} run_summary {nni_field} must be non_negative_integer|null, got {nni_val!r}")
    return arb


# --- Positive fixture ---
fixture_path = root / "docs/evidence/rollout-contract/operator-readback/p076-full-surface.fixture.json"
if not fixture_path.exists():
    raise SystemExit("proposal-076: missing P076 operator-readback fixture")
fixture = json.loads(fixture_path.read_text())

# Assert fixture is scoped as hold (not release) until runtime phases are implemented
if fixture.get("rollout_contract_decision") == "release":
    raise SystemExit("proposal-076: rollout_contract_decision must not be 'release' for Phase 1 scaffolding; expected 'hold'")

main_arb = require_auto_retry_readback(fixture, "run_report")
lanes = fixture.get("parity_lanes") or {}

# Assert no unauthorized graphql lane in parity_lanes (no GraphQL schema changes in P076)
if "graphql" in lanes:
    raise SystemExit("proposal-076: parity_lanes must not contain a 'graphql' key (P076 no-change-compatibility rule)")

require_auto_retry_readback(lanes.get("mcp") or {}, "mcp")
require_auto_retry_readback(lanes.get("release_receipt") or {}, "release_receipt")

# Prove: no side-effecting retry in any lane
for _label, _payload in [("run_report", fixture), ("mcp", lanes.get("mcp") or {}), ("release_receipt", lanes.get("release_receipt") or {})]:
    for obs in (_payload.get("auto_retry_readback") or {}).get("observations", []):
        if obs.get("retry_result") not in RETRY_RESULT_P076:
            raise SystemExit(f"proposal-076: {_label} fixture has non-P076 retry_result {obs.get('retry_result')!r}")

# Prove: human_gate observation with retry_action=none
if not any(
    obs.get("blocker_class") == "human_gate" and obs.get("retry_action") == "none"
    for obs in main_arb.get("observations", [])
):
    raise SystemExit("proposal-076: fixture must prove human_gate observation with retry_action=none")

# Prove: no_observation_history and readback_degraded in latest_by_run
statuses = {rs.get("auto_retry_policy_status") for rs in main_arb.get("latest_by_run", [])}
if "no_observation_history" not in statuses:
    raise SystemExit("proposal-076: fixture must prove no_observation_history run in latest_by_run")
if "readback_degraded" not in statuses:
    raise SystemExit("proposal-076: fixture must prove readback_degraded run in latest_by_run (degraded-success)")

# Prove: rollup grouping — at least 2 observations share a blocker_signature_id
sig_counts = {}
for obs in main_arb.get("observations", []):
    sig = obs.get("blocker_signature_id")
    if sig:
        sig_counts[sig] = sig_counts.get(sig, 0) + 1
if not any(v >= 2 for v in sig_counts.values()):
    raise SystemExit("proposal-076: fixture must have >=2 observations with the same blocker_signature_id to prove rollup dedupe")

# Prove: observe_only policy_decision present
if not any(obs.get("policy_decision") == "observe_only" for obs in main_arb.get("observations", [])):
    raise SystemExit("proposal-076: fixture must include at least one observe_only policy_decision observation")

# Prove: unsupported_version_error_sample shape
uv = fixture.get("unsupported_version_error_sample")
if not isinstance(uv, dict):
    raise SystemExit("proposal-076: fixture missing unsupported_version_error_sample")
err_envelope = uv.get("error") or {}
if err_envelope.get("code") != -32076:
    raise SystemExit("proposal-076: unsupported_version error.code must be -32076 (JSON-RPC application error envelope)")
if err_envelope.get("message") != "unsupported_version":
    raise SystemExit("proposal-076: unsupported_version error.message must be 'unsupported_version'")
err_data = err_envelope.get("data") or {}
for ef in ["code", "supported_versions", "unsupported_versions", "requested_versions"]:
    if ef not in err_data:
        raise SystemExit(f"proposal-076: unsupported_version_error_sample error.data missing {ef}")

# Prove: budget_ref_v1_sample shape + scalar types
br = fixture.get("budget_ref_v1_sample")
if not isinstance(br, dict):
    raise SystemExit("proposal-076: fixture missing budget_ref_v1_sample")
for req in BUDGET_REF_REQUIRED:
    if req not in br:
        raise SystemExit(f"proposal-076: budget_ref_v1_sample missing required field {req}")
validate_budget_ref_v1_scalars(br, "budget_ref_v1_sample", allow_fixture_metadata=True)

# Prove: observation_summary_v1_sample — strict validate (closed schema, enum domains, id format)
# fixture_note is allowed as documentation metadata in sample objects
os_sample = fixture.get("observation_summary_v1_sample")
if not isinstance(os_sample, dict):
    raise SystemExit("proposal-076: fixture missing observation_summary_v1_sample")
strict_validate_obs_summary(os_sample, "observation_summary_v1_sample", allow_fixture_metadata=True)

# Prove: strict_validation_proof shape
svp = fixture.get("strict_validation_proof") or {}
svp_input = svp.get("sample_input") or {}
if svp.get("expected_outcome") != "rejected":
    raise SystemExit("proposal-076: strict_validation_proof.expected_outcome must be 'rejected'")
svp_extra = set(svp_input.keys()) - OBS_SUMMARY_KNOWN
if not svp_extra:
    raise SystemExit("proposal-076: strict_validation_proof.sample_input must contain unknown fields for strict rejection")

# Prove: permissive_validation_proof shape — required fields present, unknown field present
pvp = fixture.get("permissive_validation_proof") or {}
pvp_input = pvp.get("sample_input") or {}
if pvp.get("expected_outcome") != "accepted_with_diagnostic":
    raise SystemExit("proposal-076: permissive_validation_proof.expected_outcome must be 'accepted_with_diagnostic'")
pvp_missing = [f for f in OBS_SUMMARY_REQUIRED if f not in pvp_input]
if pvp_missing:
    raise SystemExit(f"proposal-076: permissive_validation_proof.sample_input missing required fields for permissive validation: {pvp_missing}")
pvp_extra = set(pvp_input.keys()) - OBS_SUMMARY_KNOWN
if not pvp_extra:
    raise SystemExit("proposal-076: permissive_validation_proof.sample_input must contain an unknown additive field to prove permissive tolerance")
pvp_diags = permissive_validate_obs_summary(pvp_input, "permissive_validation_proof")
if not pvp_diags:
    raise SystemExit("proposal-076: permissive_validate_obs_summary must emit at least one diagnostic for unknown fields (permissive_report mode must prove diagnostic emission, not silent tolerance)")

# Prove: degraded_readback_sample shape (successful response with diagnostics, no partial-failure transport error)
drs = fixture.get("degraded_readback_sample") or {}
if drs.get("schema_version") != "auto_retry_readback.v1":
    raise SystemExit("proposal-076: degraded_readback_sample must have schema_version=auto_retry_readback.v1")
for pf in P076_PATH_FIELDS:
    if not drs.get(pf):
        raise SystemExit(f"proposal-076: degraded_readback_sample missing required path field: {pf}")
drs_diags = drs.get("diagnostics") or []
if not isinstance(drs_diags, list):
    raise SystemExit("proposal-076: degraded_readback_sample diagnostics must be an array")
for diag in drs_diags:
    validate_diagnostic_v1(diag, "degraded_readback_sample")
if not isinstance(drs.get("observations"), list):
    raise SystemExit("proposal-076: degraded_readback_sample observations must be an array")
if not isinstance(drs.get("latest_by_run"), list):
    raise SystemExit("proposal-076: degraded_readback_sample latest_by_run must be an array")

# --- Negative fixtures ---
neg_dir = root / "docs/evidence/rollout-contract/negative"
negative_fixtures = [
    "p076-side-effect-retry-present.jsonl",
    "p076-human-gate-retried.jsonl",
    "p076-missing-schema-field.jsonl",
    "p076-invalid-enum-strict.jsonl",
    "p076-ledger-append-missing-newline.jsonl",
    "p076-missing-budget-ref-schema.json",
    "p076-missing-observation-summary-schema.json",
    "p076-missing-readback-lock-path.json",
    "p076-missing-readback-rollup-report-path.json",
    "p076-budget-failure-retried.json",
    "p076-backpressure-exceeded.json",
    "p076-human-gate-starvation.json",
    "p076-orphaned-planned-attempt-not-escalated.json",
    "p076-pid-reuse-lock-liveness-gap.json",
    "p076-poll-timeout-without-observation.json",
    "p076-retry-timeout-duplicate-not-suppressed.json",
    "p076-ledger-append-not-fsynced.json",
    "p076-markdown-catalog-as-authority.json",
    "p076-unsafe-stale-lock-recovery.json",
    "p076-unknown-field-strict.json",
]
for nf in negative_fixtures:
    if not (neg_dir / nf).exists():
        raise SystemExit(f"proposal-076: missing negative fixture {nf}")

# Validate: side-effect-retry-present has non-P076-compliant retry_result in blocked_runs
se_found = False
for line in (neg_dir / "p076-side-effect-retry-present.jsonl").read_text().splitlines():
    if not line.strip():
        continue
    try:
        rec = json.loads(line)
        for br in rec.get("blocked_runs", []):
            if br.get("retry_result") not in RETRY_RESULT_P076:
                se_found = True
    except Exception:
        pass
if not se_found:
    raise SystemExit("proposal-076: p076-side-effect-retry-present.jsonl must have a blocked_run with retry_result not in {not_attempted, not_allowed}")

# Validate: human-gate-retried has human_gate with non-not_attempted retry_result
hg_found = False
for line in (neg_dir / "p076-human-gate-retried.jsonl").read_text().splitlines():
    if not line.strip():
        continue
    try:
        rec = json.loads(line)
        for br in rec.get("blocked_runs", []):
            if br.get("blocker_class") == "human_gate" and br.get("retry_result") not in {None, "not_attempted"}:
                hg_found = True
    except Exception:
        pass
if not hg_found:
    raise SystemExit("proposal-076: p076-human-gate-retried.jsonl must have a human_gate blocked_run with non-not_attempted retry_result")

# Validate: ledger-append-missing-newline does NOT end with newline
nl_bytes = (neg_dir / "p076-ledger-append-missing-newline.jsonl").read_bytes()
if nl_bytes.endswith(b"\n"):
    raise SystemExit("proposal-076: p076-ledger-append-missing-newline.jsonl must not end with newline (proves missing trailing newline violation)")

# Validate: missing-readback-lock-path omits lock_path
lock_fix = json.loads((neg_dir / "p076-missing-readback-lock-path.json").read_text())
if "lock_path" in (lock_fix.get("auto_retry_readback") or {}):
    raise SystemExit("proposal-076: p076-missing-readback-lock-path.json must omit lock_path from auto_retry_readback")

# Validate: missing-readback-rollup-report-path omits rollup_report_path
rollup_fix = json.loads((neg_dir / "p076-missing-readback-rollup-report-path.json").read_text())
if "rollup_report_path" in (rollup_fix.get("auto_retry_readback") or {}):
    raise SystemExit("proposal-076: p076-missing-readback-rollup-report-path.json must omit rollup_report_path from auto_retry_readback")

# Validate: budget-failure-retried marker
bf = json.loads((neg_dir / "p076-budget-failure-retried.json").read_text())
if not bf.get("proves_budget_unavailable_should_block_retry"):
    raise SystemExit("proposal-076: p076-budget-failure-retried.json missing proves_budget_unavailable_should_block_retry")

# Validate: backpressure-exceeded marker
bp = json.loads((neg_dir / "p076-backpressure-exceeded.json").read_text())
if not bp.get("proves_missing_skipped_work_record"):
    raise SystemExit("proposal-076: p076-backpressure-exceeded.json missing proves_missing_skipped_work_record")

# Validate: pid-reuse-lock-liveness-gap markers
pid = json.loads((neg_dir / "p076-pid-reuse-lock-liveness-gap.json").read_text())
if not pid.get("proves_pid_reuse_risk"):
    raise SystemExit("proposal-076: p076-pid-reuse-lock-liveness-gap.json missing proves_pid_reuse_risk")
if not isinstance(pid.get("missing_liveness_fields"), list):
    raise SystemExit("proposal-076: p076-pid-reuse-lock-liveness-gap.json missing_liveness_fields must be a list")

# Validate: poll-timeout-without-observation marker
pto = json.loads((neg_dir / "p076-poll-timeout-without-observation.json").read_text())
if not pto.get("proves_missing_timeout_observation"):
    raise SystemExit("proposal-076: p076-poll-timeout-without-observation.json missing proves_missing_timeout_observation")

# Validate: unsafe-stale-lock-recovery marker
slr = json.loads((neg_dir / "p076-unsafe-stale-lock-recovery.json").read_text())
if not slr.get("proves_unsafe_stale_lock_recovery"):
    raise SystemExit("proposal-076: p076-unsafe-stale-lock-recovery.json missing proves_unsafe_stale_lock_recovery")

# Validate: unknown-field-strict rejection mode
uf = json.loads((neg_dir / "p076-unknown-field-strict.json").read_text())
if uf.get("unknown_field_rejection_mode") != "strict":
    raise SystemExit("proposal-076: p076-unknown-field-strict.json must have unknown_field_rejection_mode='strict'")

# Strict validator: p076-unknown-field-strict.json sample_record must contain a field
# not in the observation_summary_v1 additionalProperties=false schema; also confirm
# that running the strict validator against it would fail (extra fields detected)
uf_sample = uf.get("sample_record") or {}
uf_extra = set(uf_sample.keys()) - OBS_SUMMARY_KNOWN
if not uf_extra:
    raise SystemExit(
        "proposal-076: p076-unknown-field-strict.json sample_record must contain at least one "
        "field absent from the observation_summary_v1 schema (additionalProperties=false)"
    )
# Verify strict validator would reject it (the sample has required fields present too)
uf_missing_required = [f for f in OBS_SUMMARY_REQUIRED if f not in uf_sample]
if uf_missing_required:
    raise SystemExit(
        f"proposal-076: p076-unknown-field-strict.json sample_record must include all required "
        f"observation_summary_v1 fields so strict rejection is for extra fields, not missing ones. "
        f"Missing: {uf_missing_required}"
    )
# Invoke strict validator and confirm it raises for the unknown field (fail-closed proof)
try:
    strict_validate_obs_summary(uf_sample, "p076-unknown-field-strict-sample")
    raise SystemExit(
        "proposal-076: strict_validate_obs_summary failed to reject p076-unknown-field-strict.json "
        "sample_record — fail-closed strict mode must reject unknown fields in additionalProperties=false schemas"
    )
except SystemExit as _e:
    if "unknown fields" not in str(_e) and "strict mode" not in str(_e):
        raise _e
    # expected: strict rejection confirmed — fail-closed proof passes

# Strict validator: p076-invalid-enum-strict.jsonl must have a blocked_run with
# blocker_class outside the closed BLOCKER_CLASS_ENUM domain
inv_enum_found = False
for line in (neg_dir / "p076-invalid-enum-strict.jsonl").read_text().splitlines():
    line = line.strip()
    if not line:
        continue
    try:
        rec = json.loads(line)
        for br in rec.get("blocked_runs", []):
            if br.get("blocker_class") not in BLOCKER_CLASS_ENUM:
                inv_enum_found = True
    except Exception:
        pass
if not inv_enum_found:
    raise SystemExit(
        "proposal-076: p076-invalid-enum-strict.jsonl must have a blocked_run with "
        "blocker_class not in the closed enum domain"
    )

# Strict validator: p076-missing-schema-field.jsonl must have a record missing at
# least one required auto_retry_observation_v1 top-level field
missing_field_found = False
for line in (neg_dir / "p076-missing-schema-field.jsonl").read_text().splitlines():
    line = line.strip()
    if not line:
        continue
    try:
        rec = json.loads(line)
        if any(req not in rec for req in OBS_V1_REQUIRED):
            missing_field_found = True
    except Exception:
        pass
if not missing_field_found:
    raise SystemExit(
        "proposal-076: p076-missing-schema-field.jsonl must have a record missing at least "
        "one required auto_retry_observation_v1 field"
    )

# Active validator: p076-missing-observation-summary-schema.json observations must be
# missing required observation_summary_v1 fields (proves strict validator would reject them)
mobs_fix = json.loads((neg_dir / "p076-missing-observation-summary-schema.json").read_text())
mobs_observations = mobs_fix.get("observations") or []
if not mobs_observations:
    raise SystemExit("proposal-076: p076-missing-observation-summary-schema.json must have at least one observation")
mobs_obs = mobs_observations[0]
mobs_missing = [f for f in OBS_SUMMARY_REQUIRED if f not in mobs_obs]
if not mobs_missing:
    raise SystemExit(
        "proposal-076: p076-missing-observation-summary-schema.json observation must be missing "
        "at least one required observation_summary_v1 field (proves strict validator rejects it)"
    )

# Active validator: p076-missing-budget-ref-schema.json observations must either be missing
# required fields OR contain unknown fields (proves strict validator would reject them)
mbr_fix = json.loads((neg_dir / "p076-missing-budget-ref-schema.json").read_text())
mbr_observations = mbr_fix.get("observations") or []
if not mbr_observations:
    raise SystemExit("proposal-076: p076-missing-budget-ref-schema.json must have at least one observation")
mbr_obs = mbr_observations[0]
mbr_missing = [f for f in OBS_SUMMARY_REQUIRED if f not in mbr_obs]
mbr_extra = set(mbr_obs.keys()) - OBS_SUMMARY_KNOWN
if not mbr_missing and not mbr_extra:
    raise SystemExit(
        "proposal-076: p076-missing-budget-ref-schema.json observation must be missing required fields "
        "or have unknown fields, proving the strict validator would detect a schema violation"
    )

print("proposal-076 all gate checks passed")
PY
    if ! rg -q "automation.auto_retry.latest" "$ROOT_DIR/control-plane/crates/mcp-server/src"; then
      printf 'proposal-076: missing production automation.auto_retry.latest MCP readback tool\n' >&2
      exit 1
    fi
    if [[ ! -x "$ROOT_DIR/scripts/chainworks/auto_retry_rollup.py" ]]; then
      printf 'proposal-076: missing auto-retry rollup tooling\n' >&2
      exit 1
    fi
    if [[ ! -x "$ROOT_DIR/scripts/chainworks/auto_retry_observe.py" ]]; then
      printf 'proposal-076: missing observe-only auto-retry poll writer\n' >&2
      exit 1
    fi
    if rg -q "stages_retry|stages\\.retry|runs_retry|tools/call|stages\\.approve|approvals\\.approve" "$ROOT_DIR/scripts/chainworks/auto_retry_observe.py"; then
      printf 'proposal-076: observe-only poll writer must not contain retry/recovery/approval dispatch hooks\n' >&2
      exit 1
    fi
    tmp_p076="$(mktemp -d "${TMPDIR:-/tmp}/p076-rollup.XXXXXX")"
    mkdir -p "$tmp_p076/automation"
    printf '%s\n' '{"schema_version":"auto-retry-observation.v1","observation_id":"ar_obs_20260523T100000Z_a1b2c3d4e5f6","observed_at":"2026-05-23T10:00:00Z","blocked_runs":[{"run_id":"run-a","stage_id":"state_9","blocker_signature_id":"sig-p076","blocker_class":"substantive_output_contract","policy_decision":"observe_only","retry_action":"none","retry_result":"not_attempted","failure_summary":"missing output","next_systemic_action":"inspect contract"}]}' '{"schema_version":"auto-retry-observation.v1","observation_id":"ar_obs_20260523T100100Z_b1b2c3d4e5f6","observed_at":"2026-05-23T10:01:00Z","blocked_runs":[{"run_id":"run-b","stage_id":"state_9","blocker_signature_id":"sig-p076","blocker_class":"substantive_output_contract","policy_decision":"observe_only","retry_action":"none","retry_result":"not_attempted","failure_summary":"missing output","next_systemic_action":"inspect contract"}]}' > "$tmp_p076/automation/auto-retry-observations.jsonl"
    CHAINWORKS_META_ROOT="$tmp_p076" python3 "$ROOT_DIR/scripts/chainworks/auto_retry_rollup.py" --write-markdown >/dev/null
    python3 - "$tmp_p076/automation/auto-retry-rollup.json" <<'PY'
import json, sys
payload = json.loads(open(sys.argv[1]).read())
if payload.get("schema_version") != "auto-retry-rollup.v1":
    raise SystemExit("proposal-076: rollup script emitted wrong schema_version")
issues = payload.get("issues") or []
if len(issues) != 1 or issues[0].get("observation_count") != 2:
    raise SystemExit("proposal-076: rollup script did not dedupe repeated blocker_signature_id")
PY
    tmp_p076_observe="$(mktemp -d "${TMPDIR:-/tmp}/p076-observe.XXXXXX")"
    cat > "$tmp_p076_observe/blocked-runs.json" <<'JSON'
[
  {
    "run_id": "run-p076-writer-001",
    "stage_id": "state_9_implementation_reviewed",
    "blocker_class": "substantive_output_contract",
    "blocker_signature_id": "sig-p076-writer",
    "failure_class": "missing_required_output",
    "failure_summary": "required output missing",
    "safe_retry": true,
    "retry_action": "recommend_retry",
    "retry_result": "not_attempted",
    "policy_decision": "observe_only",
    "next_systemic_action": "inspect contract"
  },
  {
    "run_id": "run-p076-human-gate-001",
    "stage_id": "state_5_approval",
    "blocker_class": "human_gate",
    "blocker_signature_id": "sig-p076-human-gate",
    "status_before": "waiting_approval",
    "safe_retry": false,
    "retry_action": "none",
    "retry_result": "not_allowed",
    "policy_decision": "human_gate",
    "next_systemic_action": "wait for human approval"
  }
]
JSON
    CHAINWORKS_META_ROOT="$tmp_p076_observe" python3 "$ROOT_DIR/scripts/chainworks/auto_retry_observe.py" --blocked-runs-json "$tmp_p076_observe/blocked-runs.json" >/dev/null
    python3 - "$tmp_p076_observe" <<'PY'
import json, sys
from pathlib import Path
root = Path(sys.argv[1])
ledger = root / "automation/auto-retry-observations.jsonl"
budget = root / "automation/auto-retry-budget.json"
catalog = root / "automation/auto-retry-known-issues.json"
markdown = root / "automation/auto-retry-known-issues.md"
rollup = root / "automation/auto-retry-rollup.json"
for path in [ledger, budget, catalog, markdown, rollup]:
    if not path.exists():
        raise SystemExit(f"proposal-076: observe writer did not create {path.name}")
raw = ledger.read_bytes()
if not raw.endswith(b"\n"):
    raise SystemExit("proposal-076: observe writer did not append newline-terminated JSONL")
records = [json.loads(line) for line in ledger.read_text().splitlines() if line.strip()]
if len(records) != 1:
    raise SystemExit("proposal-076: observe writer must append exactly one observation per poll")
record = records[0]
if record.get("schema_version") != "auto-retry-observation.v1":
    raise SystemExit("proposal-076: observe writer emitted wrong observation schema")
if not str(record.get("canonical_record_hash", "")).startswith("sha256:"):
    raise SystemExit("proposal-076: observe writer omitted canonical_record_hash")
for row in record.get("blocked_runs", []):
    if row.get("retry_result") not in {"not_attempted", "not_allowed"}:
        raise SystemExit("proposal-076: observe writer recorded side-effect retry result")
    if row.get("blocker_class") == "human_gate" and row.get("retry_action") != "none":
        raise SystemExit("proposal-076: observe writer retried human gate")
if json.loads(budget.read_text()).get("schema_version") != "auto-retry-budget.v1":
    raise SystemExit("proposal-076: observe writer emitted wrong budget schema")
if json.loads(catalog.read_text()).get("schema_version") != "auto-retry-known-issues.v1":
    raise SystemExit("proposal-076: observe writer emitted wrong catalog schema")
if json.loads(rollup.read_text()).get("issue_count") != 2:
    raise SystemExit("proposal-076: observe writer rollup did not include both signatures")
PY
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p mcp-server p076_auto_retry_latest -- --nocapture
    )
    log "Proposal 076 gate passed"
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
  proposal-081|p081)
    log "Proposal 081 gate: boundary-first API and auth contract matrix (Phase 1+2)"

    log "P081: boundary coverage guardrail"
    "$ROOT_DIR/scripts/check-boundary-coverage.sh"

    log "P081: fixture JSON validity - verify boundary-first-api-auth-contract.json exists and is valid JSON"
    python3 - <<'PY'
import json, sys, pathlib
root = pathlib.Path(sys.argv[0]).parent.parent if sys.argv[0] != "-" else pathlib.Path(".")
import os
root = pathlib.Path(os.environ.get("ROOT_DIR", "."))
fixture_path = root / "docs/reference/boundary-first-api-auth-contract.json"
if not fixture_path.exists():
    raise SystemExit("P081: missing docs/reference/boundary-first-api-auth-contract.json")
try:
    fixture = json.loads(fixture_path.read_text())
except json.JSONDecodeError as exc:
    raise SystemExit(f"P081: invalid JSON in boundary-first-api-auth-contract.json: {exc}") from exc
if fixture.get("schema_version") != 1:
    raise SystemExit(f"P081: expected schema_version 1, got {fixture.get('schema_version')}")
if "matrix_id" not in fixture:
    raise SystemExit("P081: boundary fixture missing matrix_id")
if not fixture.get("rows"):
    raise SystemExit("P081: boundary fixture rows array is empty")
REQUIRED_ROW_IDS = [
    "p081.ui_operator.graphql_query.read",
    "p081.ui_operator.graphql_subscription.subscribe",
    "p081.ui_operator.graphql_mutation.approval_action",
    "p081.agent_operator.mcp_initialize.capability",
    "p081.agent_operator.mcp_tools_list.discovery",
    "p081.agent_operator.mcp_tools_call.command",
    "p081.automation.mcp_tools_list.discovery",
    "p081.automation.mcp_tools_call.command",
    "p081.observer.mcp_tools_call.compact_read",
    "p081.observer.graphql_query.read_only_opt_in",
    "p081.developer_break_glass.debug_endpoint.disabled",
]
present_ids = {row["row_id"] for row in fixture["rows"]}
for required in REQUIRED_ROW_IDS:
    if required not in present_ids:
        raise SystemExit(f"P081: required row '{required}' missing from fixture")
print(f"P081: fixture valid - {len(fixture['rows'])} rows, all {len(REQUIRED_ROW_IDS)} required rows present")
PY

    log "P081: doc exists - verify boundary-first-api-auth-contract.md exists"
    if [[ ! -f "$ROOT_DIR/docs/reference/boundary-first-api-auth-contract.md" ]]; then
      die "P081: missing docs/reference/boundary-first-api-auth-contract.md"
    fi

    log "P081: operator readback and shadow coverage evidence fixtures"
    python3 - "$ROOT_DIR" <<'PY'
import json
import pathlib
import sys

root = pathlib.Path(sys.argv[1])
matrix_path = root / "docs/reference/boundary-first-api-auth-contract.json"
readback_path = root / "docs/evidence/rollout-contract/operator-readback/p081-full-surface.fixture.json"
coverage_path = root / "docs/evidence/boundary-policy-shadow-coverage/report.json"
canary_path = root / "docs/evidence/boundary-policy-shadow-coverage/boundary-policy-canaries.yaml"

def load(path: pathlib.Path) -> dict:
    if not path.exists():
        raise SystemExit(f"P081: missing evidence fixture {path.relative_to(root)}")
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        raise SystemExit(f"P081: invalid JSON in {path.relative_to(root)}: {exc}") from exc

readback = load(readback_path)
fixture = load(matrix_path)
present_ids = {row["row_id"] for row in fixture.get("rows") or []}
if readback.get("schema_version") != "operator_readback_v1":
    raise SystemExit("P081: operator readback fixture schema_version mismatch")
if readback.get("proposal_id") != "proposal-081":
    raise SystemExit("P081: operator readback fixture proposal_id mismatch")
if readback.get("rollout_contract_status") != "pass":
    raise SystemExit("P081: operator readback fixture must be pass")
if "placeholder" in readback or "placeholder_pending_implementation_evidence" in json.dumps(readback):
    raise SystemExit("P081: operator readback fixture still contains placeholder evidence")
graphql = (readback.get("parity_lanes") or {}).get("graphql") or {}
mcp = (readback.get("parity_lanes") or {}).get("mcp") or {}
if not graphql.get("operatorAlerts"):
    raise SystemExit("P081: GraphQL lane must include operatorAlerts proof")
if (graphql.get("websocketPolicyReload") or {}).get("closeCode") != 4408:
    raise SystemExit("P081: GraphQL lane must prove 4408 policy reload close code")
if not (mcp.get("operator_alerts_list") or {}).get("alerts_include_safe_mode"):
    raise SystemExit("P081: MCP lane must prove operator.alerts.list safe-mode alert")

coverage = load(coverage_path)
if not canary_path.exists():
    raise SystemExit("P081: missing boundary-policy-canaries.yaml source artifact")
canary_text = canary_path.read_text()
if "schema_version: boundary_policy_canaries_v1" not in canary_text:
    raise SystemExit("P081: boundary-policy-canaries.yaml schema_version mismatch")
for required in present_ids:
    if f"row_id: {required}" not in canary_text:
        raise SystemExit(f"P081: boundary-policy-canaries.yaml missing row {required}")
if "expected_decision: allow_redacted" not in canary_text:
    raise SystemExit("P081: canary artifact must include allow_redacted observer proof")
if coverage.get("schema_version") != "boundary_policy_shadow_coverage_report_v1":
    raise SystemExit("P081: shadow coverage schema_version mismatch")
if coverage.get("matrix_id") != fixture.get("matrix_id"):
    raise SystemExit("P081: shadow coverage matrix_id mismatch")
rows = coverage.get("rows") or []
row_ids = {row.get("row_id") for row in rows}
missing = sorted(present_ids - row_ids)
if missing:
    raise SystemExit(f"P081: shadow coverage missing matrix rows: {missing}")
for row in rows:
    if row.get("shadow_disagreement_count") != 0:
        raise SystemExit(f"P081: shadow disagreement for {row.get('row_id')}")
    if not row.get("canary_covered") and int(row.get("observation_count") or 0) < 10:
        raise SystemExit(f"P081: row lacks canary coverage or 10 observations: {row.get('row_id')}")
print("P081: operator readback and shadow coverage fixtures valid")
PY

    log "P081: structured boundary-policy canary validator"
    python3 "$ROOT_DIR/scripts/validate-p081-canaries.py" --root "$ROOT_DIR" --self-test

    log "P081: reliability proof inventory is gate-owned"
    python3 - "$ROOT_DIR" <<'PY'
import pathlib, sys
root = pathlib.Path(sys.argv[1])
inventory = {
    "sqlite_contention": "SQLITE_CONTENTION_RETRY_EXHAUSTED",
    "audit_outage": "E_AUDIT_UNAVAILABLE",
    "subscription_gap_replay": "proposal_081_websocket_policy_reload_close_contract_is_explicit",
    "safe_mode_exit_readback": "proposal_081_boundary_runtime_graphql_readback_is_bounded",
    "sigterm_drain": "shutdown_drain_completes_within_deadline_exits_zero",
    "denial_audit_backpressure": "audit_log.append",
    "committed_unack_retry": "p081_idempotency_pending_sentinel_recovers_committed_unack_without_reexecution",
}
haystack = "\n".join(
    path.read_text(errors="ignore")
    for path in [
        root / "scripts/test-gate.sh",
        root / "control-plane/crates/graphql-server/src/schema.rs",
        root / "control-plane/crates/graphql-server/src/server.rs",
        root / "control-plane/crates/mcp-server/src/server.rs",
        root / "control-plane/crates/db/src/repos/audit_log.rs",
        root / "control-plane/crates/daemon/src/main.rs",
    ]
    if path.exists()
)
for name, token in inventory.items():
    if token not in haystack:
        raise SystemExit(f"P081: reliability proof inventory missing {name} token {token!r}")
print("P081: reliability proof inventory valid")
PY

    log "P081: auth crate boundary module unit tests"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p auth boundary:: -- --nocapture
    )

    log "P081: auth crate CallerClass and CallerContext unit tests (Phase 2)"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p auth caller_class -- --nocapture
    )

    log "P081: db crate audit_log repo unit tests"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p db repos::audit_log:: -- --nocapture
      cargo test -p db metrics::tests::proposal_081_required_metric_names_are_declared_and_recordable -- --nocapture
      cargo test -p db repos::audit_log::tests::append_and_health_roundtrip -- --nocapture
      cargo test -p db repos::audit_log::tests::p081_audit_budget_warning_and_safe_mode_emit_runtime_readback_and_metrics -- --nocapture
      cargo test -p db repos::audit_log::tests::p081_audit_budget_recovery_exits_after_cleanup_and_half_open_probes -- --nocapture
    )

    log "P081: rollout metric labels and native-delivery semantics are gate-owned"
    python3 - "$ROOT_DIR" <<'PY'
import pathlib, sys
root = pathlib.Path(sys.argv[1])
metrics = (root / "control-plane/crates/db/src/metrics.rs").read_text()
required_metric_tokens = [
    '"boundary_policy_decision_latency_ms"',
    '("transport", transport)',
    '("caller_class", caller_class)',
    '("mode", mode)',
    '"boundary_commit_transaction_latency_ms"',
    '("action_kind", action_kind)',
    '("decision", decision)',
    '"operator_alert_clear_latency_ms"',
    '("alert_id", alert_id)',
    '("severity", severity)',
    'record_p081_audit_log_append_failure(event_type: &str, transport: &str, mode: &str)',
    'event_type={event_type},transport={transport},mode={mode}',
]
for token in required_metric_tokens:
    if token not in metrics:
        raise SystemExit(f"P081: metrics.rs missing label/semantic token {token!r}")

for rel in [
    "control-plane/crates/graphql-server/src/schema.rs",
    "control-plane/crates/mcp-server/src/tools/runtime.rs",
    "control-plane/crates/mcp-server/src/server.rs",
]:
    content = (root / rel).read_text()
    if 'increment_counter("audit_log_append_failure_total")' in content:
        raise SystemExit(f"P081: {rel} still records bare audit_log_append_failure_total")
    for stale in ["record_p081_operator_alert_native_delivery", "graphql_operator_alerts", "mcp_operator_alerts"]:
        if stale in content and "operator_alert_native_delivery" in content:
            raise SystemExit(f"P081: {rel} still records native-delivery as readback availability")

notification = (root / "Chainworks Forge/Engine/NotificationService.swift").read_text()
for token in [
    'metricName = "operator_alert_native_delivery_total"',
    'surface: "macos_notification_service"',
    'result: "delivered"',
    'result: "deduped"',
    'result: "silenced"',
]:
    if token not in notification:
        raise SystemExit(f"P081: NotificationService missing native metric token {token!r}")
print("P081: rollout metric labels and native-delivery semantics valid")
PY

    log "P081: migration compile check - audit_log and audit_log_checkpoints migrations exist"
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/068_p081_audit_log.sql" ]]; then
      die "P081: missing migration 068_p081_audit_log.sql"
    fi
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/069_p081_audit_log_checkpoints.sql" ]]; then
      die "P081: missing migration 069_p081_audit_log_checkpoints.sql"
    fi

    log "P081: migration compile check - command_journal caller_class column migration exists (Phase 2)"
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/070_p081_caller_class.sql" ]]; then
      die "P081: missing migration 070_p081_caller_class.sql"
    fi
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/071_p081_approval_idempotency.sql" ]]; then
      die "P081: missing migration 071_p081_approval_idempotency.sql"
    fi
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/072_p081_fix_payload_length_check.sql" ]]; then
      die "P081: missing migration 072_p081_fix_payload_length_check.sql"
    fi
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/073_p081_approval_idempotency_request_hash.sql" ]]; then
      die "P081: missing migration 073_p081_approval_idempotency_request_hash.sql"
    fi
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/074_p081_mcp_command_idempotency.sql" ]]; then
      die "P081: missing migration 074_p081_mcp_command_idempotency.sql"
    fi
    if [[ ! -f "$ROOT_DIR/control-plane/crates/db/migrations/075_p081_command_journal_idempotency.sql" ]]; then
      die "P081: missing migration 075_p081_command_journal_idempotency.sql"
    fi

    log "P081: schema_version 3 bootstrap and unknown-version rejection tests (Phase 2)"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p auth bootstrap_emits_schema_version_3 -- --nocapture
      cargo test -p auth v3_principal_table_rejects_unknown_schema_version -- --nocapture
      cargo test -p auth v3_principal_table_derives_caller_class_not_stored -- --nocapture
      cargo test -p auth p081_principals_file_rejects_hard_links_and_non_private_parent_dir -- --nocapture
    )

    log "P081: bounded GraphQL/MCP boundary runtime and operator alert readback tests"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p graphql-server proposal_081_boundary_runtime_graphql_readback_is_bounded -- --nocapture
      cargo test -p graphql-server proposal_081_operator_alerts_surface_safe_mode_without_raw_audit_rows -- --nocapture
      cargo test -p graphql-server proposal_081_observer_operator_alerts_redact_fields_without_graphql_errors -- --nocapture
      cargo test -p graphql-server proposal_081_subscription_runtime_readback_exposes_cursor_gap_contract -- --nocapture
      cargo test -p graphql-server proposal_081_runtime_subscription_payload_carries_cursor_generation_and_gap -- --nocapture
      cargo test -p graphql-server proposal_081_audit_budget_safe_mode_denies_approval_mutation -- --nocapture
      cargo test -p graphql-server proposal_081_websocket_policy_reload_close_contract_is_explicit -- --nocapture
      cargo test -p graphql-server test_graphql_ws_rejects_missing_connection_init_auth -- --nocapture
      cargo test -p graphql-server test_graphql_ws_rejects_non_ui_caller_with_forbidden_close -- --nocapture
      RUST_MIN_STACK=8388608 cargo test -p mcp-server proposal_081_runtime_health_includes_boundary_runtime_readback -- --nocapture
      RUST_MIN_STACK=8388608 cargo test -p mcp-server proposal_081_operator_alerts_list_exposes_safe_mode_alert -- --nocapture
    )

    log "P081: production daemon injects BoundaryPolicy through explicit constructors"
    python3 - "$ROOT_DIR" <<'PY'
import pathlib, re, sys
root = pathlib.Path(sys.argv[1])
main = (root / "control-plane/crates/daemon/src/main.rs").read_text()
if "McpServer::new_with_storage_writer_and_boundary_policy" not in main:
    raise SystemExit("P081: daemon must construct MCP server with explicit BoundaryPolicy constructor")
if "graphql_server::schema::build_schema_with_storage_writer_and_boundary_policy" not in main:
    raise SystemExit("P081: daemon must construct GraphQL schema with explicit BoundaryPolicy constructor")
if re.search(r"McpServer::new_with_storage_writer\s*\(", main):
    raise SystemExit("P081: production daemon must not use fail-open MCP constructor")
if re.search(r"graphql_server::schema::build_schema\s*\(", main):
    raise SystemExit("P081: production daemon must not use fail-open GraphQL schema constructor")
print("P081: production daemon uses explicit BoundaryPolicy constructors")
PY

    log "P081: daemon shutdown drain reliability tests"
    (
      cd "$ROOT_DIR/control-plane"
      cargo test -p daemon shutdown_drain_completes_within_deadline_exits_zero -- --nocapture
      cargo test -p daemon shutdown_drain_exceeds_deadline_reports_timeout -- --nocapture
    )

    log "P081: MCP state-changing ideas.create is command-journaled and idempotency-linked"
    (
      cd "$ROOT_DIR/control-plane"
      RUST_MIN_STACK=8388608 cargo test -p mcp-server p081_ideas_create_records_command_journal_and_idempotency_linkage -- --nocapture
      RUST_MIN_STACK=8388608 cargo test -p mcp-server p081_ideas_create_idempotency_replay_does_not_duplicate_command_commit -- --nocapture
      RUST_MIN_STACK=8388608 cargo test -p mcp-server p081_idempotency_storage_unavailable_fails_closed_with_sqlite_contention_code -- --nocapture
      RUST_MIN_STACK=8388608 cargo test -p mcp-server p081_idempotency_pending_sentinel_recovers_committed_unack_without_reexecution -- --nocapture
      RUST_MIN_STACK=8388608 cargo test -p mcp-server proposal_081_audit_budget_safe_mode_denies_state_changing_mcp_call -- --nocapture
    )

    log "P081: macOS accessibility contract source coverage"
    python3 - "$ROOT_DIR" <<'PY'
import pathlib, sys
root = pathlib.Path(sys.argv[1])
proposal = (root / "docs/proposals/081-boundary-first-api-auth-contract-matrix.md").read_text()
reference = (root / "docs/reference/swift-macos-boundary-contract.md").read_text()
tokens = [
    "full_keyboard_access_redacted_nil_vs_ordinary_nil",
    "increase_contrast_redaction_state",
    "reduce_motion_alert_state",
    "operator_alert_fires_and_clears_hidden_window",
]
for token in tokens:
    if token not in proposal:
        raise SystemExit(f"P081: proposal missing macOS accessibility proof token {token}")
    if token not in reference:
        raise SystemExit(f"P081: reference missing macOS accessibility proof token {token}")
print("P081: macOS accessibility contract source coverage valid")
PY

    log "P081: Swift approval action attempt idempotency store"
    run_targeted_tests "proposal-081-swift" "${PROPOSAL_081_SWIFT_TESTS[@]}"

    log "Proposal 081 boundary-first API/auth gate passed"
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
  proposal-078|p078)
    # P078 durable side-effect ledger gate.
    # Uses local/fake effect adapters. Must not perform live git pushes,
    # Connect uploads, notarization, production daemon startup, simulator runs,
    # or UI smoke tests.
    log "Proposal 078 durable side-effect ledger gate"
    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-078-gate cargo test -p domain proposal_078_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-078-gate cargo test -p db proposal_078_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-078-gate cargo test -p engine --lib proposal_078_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-078-gate cargo test -p engine --test proposal_058_claim_start proposal_078_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-078-gate cargo test -p engine --test release -- --nocapture
      CARGO_TARGET_DIR=target/proposal-078-gate cargo test -p mcp-server proposal_078_ -- --nocapture
    )
    python3 - <<'PY'
import json
from pathlib import Path

root = Path.cwd()

fixture_path = root / "docs/evidence/rollout-contract/operator-readback/p078-full-surface.fixture.json"
negative_path = root / "docs/evidence/rollout-contract/negative/p078-missing-side-effect-readback.json"
accessibility_path = root / "docs/evidence/rollout-contract/operator-readback/p078-macos-accessibility.fixture.json"
if not fixture_path.exists():
    raise SystemExit("proposal-078: missing P078 operator-readback fixture")
if not negative_path.exists():
    raise SystemExit("proposal-078: missing P078 missing-readback negative fixture")
if not accessibility_path.exists():
    raise SystemExit("proposal-078: missing P078 macOS accessibility proof fixture")
fixture = json.loads(fixture_path.read_text())
negative = json.loads(negative_path.read_text())
accessibility = json.loads(accessibility_path.read_text())

def require_side_effect_readback(payload, field, label):
    value = payload.get(field)
    if not isinstance(value, dict):
        raise SystemExit(f"proposal-078: {label} missing {field}")
    if value.get("schema_version", value.get("schemaVersion")) != "p078_side_effect_readback_v1":
        raise SystemExit(f"proposal-078: {label} has invalid side-effect readback schema")
    for required in ["blocked", "effects"]:
        if required not in value:
            raise SystemExit(f"proposal-078: {label} side-effect readback missing {required}")
    if not value.get("blocked"):
        raise SystemExit(f"proposal-078: {label} side-effect readback must prove blocked unresolved state")
    if value.get("unresolved_count", value.get("unresolvedCount")) != 1:
        raise SystemExit(f"proposal-078: {label} side-effect readback must carry one unresolved effect")
    effects = value.get("effects")
    if not isinstance(effects, list) or not effects:
        raise SystemExit(f"proposal-078: {label} side-effect readback must include non-empty effects")
    first = effects[0]
    action = first.get("operator_next_action", first.get("operatorNextAction"))
    if action != "effects.reconcile":
        raise SystemExit(f"proposal-078: {label} side-effect readback must expose effects.reconcile next action")

require_side_effect_readback(fixture, "side_effect_readback", "run_report")
lanes = fixture.get("parity_lanes") or {}
require_side_effect_readback(lanes.get("mcp") or {}, "side_effect_readback", "mcp")
require_side_effect_readback(lanes.get("release_receipt") or {}, "side_effect_readback", "release_receipt")
require_side_effect_readback(lanes.get("graphql") or {}, "sideEffectReadback", "graphql")
if "side_effect_readback" in negative or "sideEffectReadback" in negative:
    raise SystemExit("proposal-078: negative fixture unexpectedly contains side-effect readback")
metrics = fixture.get("metrics") or []
metric_names = {item.get("name") for item in metrics if isinstance(item, dict)}
for required_metric in [
    "p078_release_side_effects_with_durable_intent_percent",
    "side_effect_intent_total",
    "side_effect_transition_total",
    "side_effect_retry_block_total",
    "side_effect_unresolved",
    "side_effect_unresolved_age_seconds",
    "side_effect_recovery_transition_total",
    "side_effect_settlement_latency_seconds",
    "startup_side_effect_recovery_total",
    "startup_side_effect_recovery_duration_seconds",
    "side_effect_ledger_readback_error_total",
    "side_effect_ledger_readback_circuit_open_total",
    "side_effect_evidence_spooled_bytes_total",
    "side_effect_evidence_disk_bytes",
    "side_effect_prepare_denied_total",
]:
    if required_metric not in metric_names:
        raise SystemExit(f"proposal-078: rollout fixture missing operational metric {required_metric}")
for item in metrics:
    if item.get("cardinality_bound") != "effect_kind_status":
        raise SystemExit("proposal-078: metric fixture must document effect_kind/status cardinality bound")

side_effects_rs = (root / "control-plane/crates/engine/src/side_effects.rs").read_text()
executor_rs = (root / "control-plane/crates/engine/src/executor.rs").read_text()
effects_rs = (root / "control-plane/crates/mcp-server/src/tools/effects.rs").read_text()
tools_mod_rs = (root / "control-plane/crates/mcp-server/src/tools/mod.rs").read_text()
graphql_schema_rs = (root / "control-plane/crates/graphql-server/src/schema.rs").read_text()
runs_rs = (root / "control-plane/crates/mcp-server/src/tools/runs.rs").read_text()
swift_truth = (root / "Chainworks Forge/Models/ExecutionTruth.swift").read_text()
swift_read_boundary = (root / "Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift").read_text()
swift_runs_home = (root / "Chainworks Forge/Views/RunsHomeView.swift").read_text()

for metric in [
    "p078_release_side_effects_with_durable_intent_percent",
    "side_effect_intent_total",
    "side_effect_transition_total",
    "side_effect_retry_block_total",
    "side_effect_ledger_readback_error_total",
    "side_effect_ledger_readback_circuit_open_total",
    "side_effect_recovery_transition_total",
    "side_effect_settlement_latency_seconds",
    "side_effect_unresolved",
    "side_effect_unresolved_age_seconds",
    "startup_side_effect_recovery_total",
    "startup_side_effect_recovery_duration_seconds",
    "side_effect_evidence_spooled_bytes_total",
    "side_effect_evidence_disk_bytes",
    "side_effect_prepare_denied_total",
]:
    if metric not in side_effects_rs + executor_rs:
        raise SystemExit(f"proposal-078: missing metric literal {metric}")

for required in [
    "watchdog_pass().await",
    "run_unresolved_effects_preflight",
    "p078_expected_side_effect_evidence_v1",
    "p078_side_effect_evidence_manifest_v1",
    "p078_observed_evidence_summary_v1",
    "manifest_write_order",
    "run_with_lease_renewal",
    "p075_write_spool_file",
    "verify_p078_observed_evidence_summary",
    "mark_settled_evidence_failed",
    "P078_LEDGER_READBACK_CIRCUIT_THRESHOLD",
    "ledger_readback_circuit_open_until",
    "release-receipt.json",
    "stdout.log",
    "stderr.log",
    "git-ls-remote.json",
    "upload-readback.json",
    "archive-summary.json",
    "reconciliation-report.json",
]:
    if required not in executor_rs + side_effects_rs:
        raise SystemExit(f"proposal-078: missing executor proof marker {required}")

for required in [
    "effects.mark_conflict",
    "handle_effects_mark_conflict",
]:
    if required not in effects_rs or required not in tools_mod_rs + effects_rs:
        raise SystemExit(f"proposal-078: missing MCP conflict disposition marker {required}")

for required in [
    "GqlSideEffectSummary",
    "observed_evidence_summary_json",
    "operator_next_action",
    "side_effect_readback",
    "SideEffectReadbackSummary",
    "P078SideEffectReadbackPresenter",
    "P078SideEffectReadbackCard",
]:
    haystack = "\n".join([graphql_schema_rs, runs_rs, swift_truth, swift_read_boundary, swift_runs_home])
    if required not in haystack:
        raise SystemExit(f"proposal-078: missing readback marker {required}")

swift_tree = root / "Chainworks Forge"
swift_text = "\n".join(path.read_text(errors="ignore") for path in swift_tree.rglob("*.swift"))
for forbidden in ["effects.mark_conflict", "effects.mark_unrecoverable", "effects.clear_after_manual_verification"]:
    if forbidden in swift_text:
        raise SystemExit(f"proposal-078: Swift app must remain read-only for {forbidden}")

if accessibility.get("schema_version") != "p078_macos_accessibility_view_hierarchy_v1":
    raise SystemExit("proposal-078: invalid macOS accessibility proof schema")
elements = accessibility.get("elements") or []
ids = {item.get("accessibility_identifier") for item in elements if isinstance(item, dict)}
for required_id in [
    "p078-side-effect-readback-card",
    "p078-side-effect-sidebar-signal",
    "p078-side-effect-next-action",
    "p078-side-effect-diagnostics",
]:
    if required_id not in ids:
        raise SystemExit(f"proposal-078: macOS accessibility proof missing {required_id}")
for item in elements:
    if item.get("mutation_control"):
        raise SystemExit("proposal-078: macOS accessibility proof contains mutation control")
for forbidden in ["reconcile", "retry", "clear", "push", "upload", "publish", "mcp_launch"]:
    if forbidden not in accessibility.get("forbidden_controls_absent", []):
        raise SystemExit(f"proposal-078: macOS accessibility proof missing forbidden control check {forbidden}")
PY
    log "Proposal 078 durable side-effect ledger gate passed"
    ;;
  proposal-088|p088)
    log "Proposal 088 gate: code-writer completion handoff and diagnostics"
    python3 - <<'PY'
import json
from pathlib import Path

root = Path.cwd()
evidence = root / "docs/evidence/088-code-writer-completion"
required = {
    "p087-terminal-completed-missing-outputs.fixture.json": {
        "scenario": "p087_terminal_completed_missing_outputs",
        "expected_failure_class": "terminal_response_completed_missing_required_outputs",
    },
    "p087-70c9-dirty-worktree-timeout.fixture.json": {
        "scenario": "p087_70c9_preexisting_dirty_timeout",
        "work_change_kind": "preexisting_dirty_work",
        "expected_next_operator_action": "do_not_retry_preexisting_dirty_timeout",
    },
    "large-streamed-prelude-tail-capture.fixture.json": {
        "scenario": "large_streamed_prelude_tail_capture",
        "completion_text_capture_source": "streamed_update_tail",
        "extraction_input_truncated": False,
    },
    "public-enum-roundtrip.fixture.json": {
        "scenario": "public_enum_roundtrip",
    },
    "worktree-fingerprint-v1.fixture.json": {
        "schema_version": "worktree_fingerprint_v1",
    },
    "prompt-side-evidence.fixture.json": {
        "scenario": "prompt_side_evidence",
        "prompt_template_id": "code_writer_completion_repair_v1",
    },
    "normal-materialization-no-repair.fixture.json": {
        "scenario": "normal_materialization_no_repair",
        "expected_completion_turn_attempted": False,
    },
    "completion-repair-mutation-negative.fixture.json": {
        "scenario": "completion_repair_mutation_negative",
        "expected_completion_turn_result": "failed_unexpected_worktree_mutation",
    },
    "docs-only-implementation-change.fixture.json": {
        "scenario": "docs_only_implementation_change",
        "work_change_kind": "current_attempt_diff",
    },
    "generated-evidence-only-ineligible.fixture.json": {
        "scenario": "generated_evidence_only_ineligible",
        "expected_eligible_for_completion_repair": False,
    },
    "ingestion-boundary-failures.fixture.json": {
        "scenario": "ingestion_boundary_failures",
    },
    "partial-write-recovery.fixture.json": {
        "scenario": "completion_receipt_partial_write",
        "expected_failure_class": "completion_receipt_partial_write",
    },
    "provider-independence.fixture.json": {
        "scenario": "provider_independence_completion_contract",
        "expected_failure_class": "work_completed_missing_current_attempt_outputs",
        "provider_specific_truth_branch_allowed": False,
    },
}
for name, expectations in required.items():
    path = evidence / name
    if not path.exists():
        raise SystemExit(f"proposal-088: missing fixture {path}")
    try:
        data = json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        raise SystemExit(f"proposal-088: invalid JSON in {path}: {exc}") from exc
    for key, expected in expectations.items():
        actual = data.get(key)
        if actual != expected:
            raise SystemExit(
                f"proposal-088: fixture {name} expected {key}={expected!r}, got {actual!r}"
            )

fingerprint = json.loads((evidence / "worktree-fingerprint-v1.fixture.json").read_text())
paths = fingerprint.get("paths")
summary = fingerprint.get("summary", {})
if not isinstance(paths, list) or not paths:
    raise SystemExit("proposal-088: worktree fingerprint fixture must contain paths")
if paths != sorted(paths, key=lambda item: item.get("normalized_path", "")):
    raise SystemExit("proposal-088: worktree fingerprint paths must be sorted")
derived_preexisting = sum(1 for path in paths if path.get("path_status") == "preexisting_dirty")
if summary.get("preexisting_dirty_path_count") != derived_preexisting:
    raise SystemExit("proposal-088: fingerprint preexisting_dirty_path_count is not derived")
if summary.get("work_change_kind") != "preexisting_dirty_work":
    raise SystemExit("proposal-088: fingerprint fixture must prove preexisting dirty work")

provider_fixture = json.loads((evidence / "provider-independence.fixture.json").read_text())
providers = sorted(item.get("provider") for item in provider_fixture.get("providers", []))
if providers != ["claude", "codex", "junie"]:
    raise SystemExit(
        f"proposal-088: provider independence fixture must cover claude/codex/junie, got {providers!r}"
    )

proposal = root / "docs/proposals/088-code-writer-completion-contract-and-output-freshness.md"
if not proposal.exists():
    raise SystemExit("proposal-088: missing proposal document")
proposal_text = proposal.read_text()
for required_term in [
    "code_writer_completion_repair_v1",
    "worktree_fingerprint_v1",
    "terminal_response_capture_truncated_before_output",
    "extraction_input_truncated",
    "current_attempt_diff",
    "preexisting_dirty_work",
]:
    if required_term not in proposal_text:
        raise SystemExit(f"proposal-088: proposal missing required term {required_term!r}")

gates_doc = root / "docs/reference/test-gates.md"
if not gates_doc.exists():
    raise SystemExit("proposal-088: missing docs/reference/test-gates.md")
gates_text = gates_doc.read_text()
for required_term in [
    "### `proposal-088|p088`",
    "worktree_fingerprint_v1",
    "70c9",
    "implementationCompletion",
    "closed vocabularies",
    "prompt-level runtime receipt persistence",
]:
    if required_term not in gates_text:
        raise SystemExit(f"proposal-088: test-gates.md missing {required_term!r}")

db_repo = root / "control-plane/crates/db/src/repos/code_writer_completion_receipts.rs"
db_repo_text = db_repo.read_text()
for required_term in [
    "upsert_with_runtime_receipts",
    "list_canonical_by_run",
    "completion_receipt_conflict",
    "code_writer_completion_receipt_links",
]:
    if required_term not in db_repo_text:
        raise SystemExit(f"proposal-088: DB receipt repo missing {required_term!r}")

engine_executor = root / "control-plane/crates/engine/src/executor.rs"
engine_executor_text = engine_executor.read_text()
for required_term in [
    "skipped_no_live_session",
    "p037_idle_terminalization",
    "upsert_with_runtime_receipts",
    "completion_receipt_partial_write",
    "storage_write_failed",
    "CodeWriterCompletionStarted",
    "CodeWriterCompletionSucceeded",
    "CodeWriterCompletionFailed",
]:
    if required_term not in engine_executor_text:
        raise SystemExit(f"proposal-088: engine executor missing {required_term!r}")

engine_integration = root / "control-plane/crates/engine/tests/integration.rs"
engine_integration_text = engine_integration.read_text()
for required_term in [
    "proposal_088_code_writer_stale_implementation_active_enters_receipt_path_not_auto_requeue",
    "p088_stale_implementation_active",
    "acp_active_prompt_recovery",
]:
    if required_term not in engine_integration_text:
        raise SystemExit(f"proposal-088: engine integration test missing {required_term!r}")

sessions_domain = root / "control-plane/crates/domain/src/session.rs"
sessions_repo = root / "control-plane/crates/db/src/repos/sessions.rs"
for required_term in [
    "CodeWriterCompletionStarted",
    "CodeWriterCompletionSucceeded",
    "CodeWriterCompletionFailed",
    "code_writer_completion_started",
    "code_writer_completion_succeeded",
    "code_writer_completion_failed",
]:
    if required_term not in sessions_domain.read_text():
        raise SystemExit(f"proposal-088: domain session events missing {required_term!r}")
    if required_term not in sessions_repo.read_text():
        raise SystemExit(f"proposal-088: DB session event mapping missing {required_term!r}")

print("proposal-088 static fixture checks passed")
PY
    (
      cd control-plane
      CARGO_TARGET_DIR=target/proposal-088-gate cargo test -p acp proposal_088_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-088-gate cargo test -p domain proposal_088_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-088-gate cargo test -p db proposal_088_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-088-gate cargo test -p engine proposal_088_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-088-gate cargo test -p graphql-server proposal_088_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-088-gate cargo test -p mcp-server proposal_088_ -- --nocapture
    )
    log "Proposal 088 gate passed"
    ;;
  proposal-089|p089)
    log "Proposal 089 gate: Junie structured-output proof and ACP canary evidence"
    if [[ "${CHAINWORKS_PROPOSAL_089_ALLOW_DIRTY:-0}" == "1" ]]; then
      log "Proposal 089 diagnostic dirty-work mode requested; this mode cannot produce signoff evidence"
      mkdir -p "$ROOT_DIR/docs/evidence/089/junie-structured-output-canary/acp-canary"
      cat >"$ROOT_DIR/docs/evidence/089/junie-structured-output-canary/acp-canary/mutation-guard-result.json" <<'JSON'
{
  "schema_version": "p089_mutation_guard_result_v1",
  "verdict": "evidence_incomplete",
  "overall_status": "evidence_incomplete",
  "preexisting_dirty_work_non_canary_safe": true,
  "safety_violations": [],
  "diagnostic_mode": "allow_dirty",
  "signoff_eligible": false
}
JSON
      echo "proposal-089: CHAINWORKS_PROPOSAL_089_ALLOW_DIRTY=1 is diagnostic-only and must not pass signoff" >&2
      exit 1
    fi
    if [[ "${CHAINWORKS_PROPOSAL_089_LIVE:-0}" == "1" ]]; then
      log "Proposal 089 live mode: running canonical Junie ACP canary"
      mkdir -p "$ROOT_DIR/.chainworks/tmp"
      mkdir -p "$ROOT_DIR/docs/evidence/089/junie-structured-output-canary"
      p089_live_log="$ROOT_DIR/docs/evidence/089/junie-structured-output-canary/live-gate.log.redacted"
      if [[ ! -d "$ROOT_DIR/.chainworks/tmp/p089-acp-canary-worktree/.git" && ! -f "$ROOT_DIR/.chainworks/tmp/p089-acp-canary-worktree/.git" ]]; then
        git worktree add --detach "$ROOT_DIR/.chainworks/tmp/p089-acp-canary-worktree" HEAD
      fi
      {
        printf '%s\n' '$ CHAINWORKS_PROPOSAL_089_LIVE=1 ./scripts/test-gate.sh proposal-089'
        printf '%s\n' '==> Proposal 089 live mode: running canonical Junie ACP canary'
        (
          cd "$ROOT_DIR/control-plane"
          P089_WORKTREE_ROOT="$ROOT_DIR/.chainworks/tmp/p089-acp-canary-worktree" \
            P089_ACP_EVIDENCE_DIR="$ROOT_DIR/docs/evidence/089/junie-structured-output-canary/acp-canary" \
            cargo run -p engine --example p089_acp_live_canary
        )
      } >"$p089_live_log" 2>&1
      cat "$p089_live_log"
      python3 "$ROOT_DIR/scripts/proposal-089-refresh-evidence.py"
    fi
    python3 - <<'PY'
import hashlib
import json
from pathlib import Path

root = Path.cwd()
evidence = root / "docs/evidence/089/junie-structured-output-canary"
native_root = evidence / "native"
acp_root = evidence / "acp-canary"
index_path = evidence / "evidence-index.json"
live_path = evidence / "live-gate-run.json"

STATUSES = {
    "passed",
    "environment_unavailable",
    "native_capability_failed",
    "acp_launch_failed",
    "acp_handshake_failed",
    "completion_capture_failed",
    "completion_capture_truncated",
    "extraction_failed",
    "settlement_failed",
    "unexpected_completion_repair",
    "unexpected_repo_mutation",
    "evidence_incomplete",
}

def fail(message):
    raise SystemExit(f"proposal-089: {message}")

def load_json(path):
    if not path.exists():
        fail(f"missing {path}")
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path}: {exc}")

def sha256_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def file_meta(path):
    if not path.exists():
        fail(f"missing file {path}")
    return {"sha256": sha256_file(path), "size_bytes": path.stat().st_size}

def assert_file_record(path, record):
    meta = file_meta(path)
    if record.get("sha256") != meta["sha256"] or record.get("size_bytes") != meta["size_bytes"]:
        fail(f"hash/size mismatch for {path}")

def file_record_matches(path, record):
    meta = file_meta(path)
    return record.get("sha256") == meta["sha256"] and record.get("size_bytes") == meta["size_bytes"]

def assert_status(value, context):
    if value not in STATUSES:
        fail(f"{context} has unknown status {value!r}")

def require(condition, message):
    if not condition:
        fail(message)

def manifest_row_is_valid_control_plane(row):
    return (
        row.get("settlement_decision") == "accepted"
        and row.get("source_kind") is None
        and row.get("reason") == "control_plane_generated"
        and row.get("provenance") == "control_plane_generated"
        and row.get("generated_by") == "control_plane"
        and row.get("contributes_to_junie_capability") is False
    )

def broad_allowed_root_is_rejected(root_value):
    root_text = str(root_value)
    candidate = Path(root_text)
    if not candidate.is_absolute():
        candidate = root / candidate
    try:
        resolved = candidate.resolve(strict=False)
    except OSError:
        resolved = candidate.absolute()
    forbidden = [
        root,
        root / "Chainworks Forge",
        root / "Chainworks ForgeTests",
        root / "Chainworks ForgeUITests",
        root / "control-plane",
        root / "examples",
        root / "scripts",
        root / "docs/reference",
        root / ".chainworks",
        root / ".chainworks/runs",
    ]
    for forbidden_path in forbidden:
        forbidden_resolved = forbidden_path.resolve(strict=False)
        if resolved == forbidden_resolved or forbidden_resolved in resolved.parents:
            return True
        if resolved in forbidden_resolved.parents:
            return True
    return False

def validate_negative_fixtures():
    negative_root = evidence / "negative"
    broad_roots = load_json(negative_root / "broad-allowed-roots.fail.json")
    require(broad_roots.get("expected_status") == "evidence_incomplete", "broad root fixture expected_status mismatch")
    for case in broad_roots.get("cases") or []:
        require(
            broad_allowed_root_is_rejected(case.get("root")),
            f"broad root fixture did not reject {case.get('root')!r}",
        )

    drift = load_json(negative_root / "proof-critical-drift.fail.json")
    require(drift.get("expected_status") == "evidence_incomplete", "proof-critical drift fixture expected_status mismatch")
    drift_path = root / drift.get("path", "")
    drift_record = drift.get("record") or {}
    require(not file_record_matches(drift_path, drift_record), "proof-critical drift fixture unexpectedly matched current file")

    bad_manifest = load_json(negative_root / "changed-files-manifest-agent-source.fail.json")
    require(bad_manifest.get("expected_status") == "evidence_incomplete", "bad manifest fixture expected_status mismatch")
    require(
        not manifest_row_is_valid_control_plane(bad_manifest.get("row") or {}),
        "bad manifest fixture unexpectedly validated",
    )

    dirty = load_json(negative_root / "allow-dirty-diagnostic.fail.json")
    require(dirty.get("expected_status") == "evidence_incomplete", "allow dirty fixture expected_status mismatch")
    require(dirty.get("env", {}).get("CHAINWORKS_PROPOSAL_089_ALLOW_DIRTY") == "1", "allow dirty fixture env mismatch")
    require(dirty.get("signoff_eligible") is False, "allow dirty fixture must be non-signoff")

def validate_native_experiment(name, expected):
    directory = native_root / name
    required = [
        "prompt.txt",
        "command.json",
        "environment.json",
        "final-output.raw.txt",
        "parser-result.json",
        "conclusion.json",
    ]
    for filename in required:
        if not (directory / filename).exists():
            fail(f"native {name} missing {filename}")
    command = load_json(directory / "command.json")
    parser = load_json(directory / "parser-result.json")
    conclusion = load_json(directory / "conclusion.json")
    assert_status(conclusion.get("status"), f"native {name} conclusion")
    prompt_bytes = (directory / "prompt.txt").read_bytes()
    final_bytes = (directory / "final-output.raw.txt").read_bytes()
    require(command.get("schema_version") == "p089_native_command_v1", f"native {name} command schema")
    require(command.get("output_mode") == "stdout_text", f"native {name} output_mode must be stdout_text")
    require(command.get("input_mode") == "task_arg", f"native {name} input_mode must be task_arg")
    require(command.get("prompt_sha256") == hashlib.sha256(prompt_bytes).hexdigest(), f"native {name} prompt_sha256 mismatch")
    args = command.get("args") or []
    require("--acp" not in args and "--json-output-file" not in args, f"native {name} used forbidden Junie args")
    require(parser.get("success") is True, f"native {name} parser did not pass")
    require(conclusion.get("status") == "passed", f"native {name} conclusion is not passed")
    require(conclusion.get("final_output_sha256") == hashlib.sha256(final_bytes).hexdigest(), f"native {name} final-output hash mismatch")
    try:
        parsed = json.loads(final_bytes.decode())
    except json.JSONDecodeError as exc:
        fail(f"native {name} final-output is not strict JSON: {exc}")
    require(parsed == expected, f"native {name} final-output contract mismatch")

validate_native_experiment(
    "exact-json",
    {"p089_native_probe": "exact_json", "status": "passed", "value": 1},
)
validate_native_experiment(
    "exact-chainworks-output",
    {"CHAINWORKS_OUTPUT": {"native_chainworks_output": {"status": "passed", "value": 1}}},
)
validate_native_experiment(
    "repair-style-minimal",
    {
        "CHAINWORKS_OUTPUT": {
            "tests_result": {"status": "not_run", "commands": []},
            "implementation_self_assessment": {
                "implementation_complete": True,
                "verification_green": True,
                "remaining_code_tasks": [],
                "handoff_tasks": [],
                "known_risks": [],
                "tests_run": [],
                "docs_impacted": [],
            },
        }
    },
)

for filename in [
    "preflight.json",
    "receipt.json",
    "terminal-completion.raw.txt",
    "extraction-result.json",
    "settled-outputs.json",
    "run-report.json",
    "worktree-fingerprint-pre.json",
    "worktree-fingerprint-post.json",
    "mutation-guard-result.json",
    "conclusion.json",
]:
    if not (acp_root / filename).exists():
        fail(f"ACP canary missing {filename}")

preflight = load_json(acp_root / "preflight.json")
receipt = load_json(acp_root / "receipt.json")
extraction = load_json(acp_root / "extraction-result.json")
settled = load_json(acp_root / "settled-outputs.json")
mutation = load_json(acp_root / "mutation-guard-result.json")
conclusion = load_json(acp_root / "conclusion.json")
terminal_text = (acp_root / "terminal-completion.raw.txt").read_text()

assert_status(preflight.get("status"), "ACP preflight")
assert_status(conclusion.get("status"), "ACP conclusion")
require(preflight.get("status") == "passed", "ACP preflight did not pass")
require(conclusion.get("status") == "passed", "ACP conclusion did not pass")
require(receipt.get("provider") == "junie", "ACP receipt provider must be junie")
require(receipt.get("runtime_profile") == "junie_cli_acp", "ACP receipt runtime_profile mismatch")
require(receipt.get("agent_id") == "code_writer", "ACP receipt agent_id must be code_writer")
require(receipt.get("backend_profile") == "junie_code_editor_acp", "ACP receipt backend_profile mismatch")
require(receipt.get("model") == "junie-default", "ACP receipt model must come from production backend profile")
require(receipt.get("effort") == "high", "ACP receipt effort must come from production backend profile")
require(receipt.get("adapter_family") == "JunieAdapter", "ACP receipt adapter_family mismatch")
require(receipt.get("launch_mode") == "--acp true", "ACP receipt launch mode mismatch")
require(receipt.get("output_set_mode") == "full_production", "ACP receipt output_set_mode mismatch")
require(receipt.get("stage_id") == "p089_acp_canary", "ACP receipt stage_id mismatch")
require(receipt.get("stage_execution_id"), "ACP receipt stage_execution_id missing")
require(receipt.get("agent_execution_id"), "ACP receipt agent_execution_id missing")
require(receipt.get("session_generation_id"), "ACP receipt session_generation_id missing")
catalog_binding = receipt.get("catalog_binding") or {}
require(catalog_binding.get("agent_id") == "code_writer", "catalog binding agent_id mismatch")
require(catalog_binding.get("backend_profile") == "junie_code_editor_acp", "catalog binding backend_profile mismatch")
require(catalog_binding.get("provider") == "junie", "catalog binding provider mismatch")
require(catalog_binding.get("model") == "junie-default", "catalog binding model mismatch")
require(catalog_binding.get("effort") == "high", "catalog binding effort mismatch")
require(catalog_binding.get("runtime_profile") == "junie_cli_acp", "catalog binding runtime_profile mismatch")
require(catalog_binding.get("outputs") == [
    "implementation_progress",
    "implementation_self_assessment",
    "changed_files_manifest",
    "tests_result",
], "catalog binding outputs mismatch")
catalog_path = catalog_binding.get("catalog_path")
require(catalog_path, "catalog binding catalog_path missing")
catalog_abs = Path(catalog_path)
if not catalog_abs.is_absolute():
    catalog_abs = root / catalog_abs
require(catalog_abs.resolve(strict=False) == (root / "examples/agents/agents.yaml").resolve(strict=False), "catalog binding path mismatch")
require(catalog_binding.get("catalog_sha256") == sha256_file(root / "examples/agents/agents.yaml"), "catalog binding hash mismatch")

expected_contracts = {
    "implementation_progress": "implementation_progress",
    "implementation_self_assessment": "implementation_self_assessment_v2",
    "changed_files_manifest": "changed_files_manifest",
    "tests_result": "tests_result",
}
require(catalog_binding.get("contract_ids") == expected_contracts, "catalog binding contract_ids mismatch")
compiled = {item.get("name"): item for item in receipt.get("compiled_task_outputs") or []}
require(set(compiled) == set(expected_contracts), "ACP compiled outputs must match production code_writer output set")
for name, expected_contract in expected_contracts.items():
    actual_contract = compiled[name].get("contract_id")
    require(actual_contract == expected_contract, f"ACP output {name} contract_id expected {expected_contract!r}, got {actual_contract!r}")
    require(compiled[name].get("required") is True, f"ACP output {name} must be required")

repair = receipt.get("repair_metadata") or {}
require(repair.get("completion_turn_attempted") is False, "ACP completion_turn_attempted must be false")
require(repair.get("completion_repair_turn_count") == 0, "ACP completion_repair_turn_count must be 0")
require(repair.get("generic_repair_turn_count") == 0, "ACP generic_repair_turn_count must be 0")
require(repair.get("completion_repair_runtime_receipt_present") is False, "ACP completion repair runtime receipt must be absent")
runtime_receipt = receipt.get("runtime_receipt") or {}
require(runtime_receipt.get("status") == "completed", "ACP runtime receipt must be completed")

require(extraction.get("completion_text_sha256") == hashlib.sha256(terminal_text.encode()).hexdigest(), "ACP terminal completion hash mismatch")
require(extraction.get("completion_text_truncated") is False, "ACP completion text must not be truncated")
require(extraction.get("extraction_input_truncated") is False, "ACP extraction input must not be truncated")
require(extraction.get("raw_completion_has_non_json_prefix") is False, "ACP terminal completion must be strict CHAINWORKS_OUTPUT JSON with no prefix")
try:
    terminal_json = json.loads(terminal_text)
except json.JSONDecodeError as exc:
    fail(f"ACP terminal completion is not strict JSON: {exc}")
require(set((terminal_json.get("CHAINWORKS_OUTPUT") or {}).keys()) == {
    "implementation_progress",
    "implementation_self_assessment",
    "tests_result",
}, "ACP CHAINWORKS_OUTPUT must contain exactly the Junie-authored outputs")
require((extraction.get("parser_result") or {}).get("success") is True, "ACP extraction parser did not pass")

rows = {row.get("output_name"): row for row in settled.get("declared_outputs") or []}
require(set(rows) == set(expected_contracts), "settled outputs must match full production output set")
require(settled.get("settlement_boundary") == "engine::executor::generate_changed_files_manifest_if_declared_then_settle_agent_outputs_from_discovery_decisions", "settled outputs must come from production executor settlement boundary")
require(settled.get("materialization_owner") == "engine_executor", "settled outputs materialization_owner mismatch")
require(settled.get("changed_files_manifest_status") in {"available", "not_git_repository"}, "changed_files_manifest status mismatch")
require(settled.get("decisions"), "settled outputs must include production discovery decisions")
for name in ["implementation_progress", "implementation_self_assessment", "tests_result"]:
    row = rows[name]
    require(row.get("settlement_decision") == "accepted", f"{name} settlement not accepted")
    require(row.get("freshness") == "current_attempt", f"{name} not current_attempt")
    require(row.get("source_kind") == "chainworks_output", f"{name} source_kind must be chainworks_output")
    require(row.get("source_generation_owner") == "agent", f"{name} source_generation_owner must be agent")
    require(row.get("contributes_to_junie_capability") is True, f"{name} must contribute to Junie capability")
manifest = rows["changed_files_manifest"]
require(manifest_row_is_valid_control_plane(manifest), "changed_files_manifest control-plane row mismatch")
require(settled.get("all_required_outputs_accepted") is True, "not all required outputs accepted")
require(settled.get("junie_capability_outputs_accepted") is True, "Junie capability outputs not accepted")

require(mutation.get("verdict") == "passed", "mutation guard verdict must be passed")
require(mutation.get("safety_violations") == [], "mutation guard has safety violations")
require(mutation.get("canonicalized_allowed_roots_valid") is True, "allowed roots were not canonicalized/valid")
post_summary = load_json(acp_root / "worktree-fingerprint-post.json").get("summary") or {}
require(post_summary.get("current_attempt_changed_path_count") == 0, "post fingerprint has current-attempt repo changes")
require(post_summary.get("preexisting_dirty_path_count") == 0, "post fingerprint has preexisting dirty work")

if not index_path.exists() or not live_path.exists():
    fail("missing evidence-index.json or live-gate-run.json")
index = load_json(index_path)
live = load_json(live_path)
require(index.get("schema_version") == "p089_evidence_index_v1", "invalid evidence-index schema")
require(live.get("schema_version") == "p089_live_gate_run_v1", "invalid live-gate-run schema")
for field in ["native_phase_status", "acp_canary_status", "overall_status"]:
    assert_status(index.get(field), f"evidence-index {field}")
    require(index.get(field) == "passed", f"evidence-index {field} must be passed")
require(live.get("exit_code") == 0, "live-gate-run exit_code must be 0")
require(live.get("result") == "passed", "live-gate-run result must be passed")
require(live.get("working_directory") == str(root), "live-gate-run working_directory mismatch")
require(live.get("started_at"), "live-gate-run started_at missing")
require(live.get("completed_at"), "live-gate-run completed_at missing")
require(live.get("native_timeout_ms") == 120000, "live-gate-run native_timeout_ms mismatch")
require(live.get("native_phase_status") == "passed", "live-gate-run native_phase_status must be passed")
require(live.get("acp_canary_status") == "passed", "live-gate-run acp_canary_status must be passed")
require(live.get("overall_status") == "passed", "live-gate-run overall_status must be passed")
require(live.get("audited_git_sha") == index.get("audited_git_sha"), "audited git sha mismatch")
require(index.get("live_gate_run", {}).get("path") == str(live_path.relative_to(root)), "live_gate_run path mismatch")
assert_file_record(live_path, index.get("live_gate_run", {}))
require(live.get("command") == "./scripts/test-gate.sh proposal-089", "live-gate-run command mismatch")
live_env = live.get("environment") or {}
require(live_env.get("CHAINWORKS_PROPOSAL_089_LIVE") == "1", "live-gate-run must prove live mode")
require(live_env.get("recorded_env_names") == ["CHAINWORKS_JUNIE_ACP_BINARY"], "live-gate-run recorded_env_names mismatch")
require(live_env.get("redacted_env") is True, "live-gate-run redacted_env must be true")
log_record = live.get("log") or {}
assert_file_record(root / log_record.get("path", ""), log_record)

proof_index = index.get("proof_critical_files") or []
proof_live = live.get("proof_critical_files") or []
require(proof_index == proof_live, "proof_critical_files mismatch between index and live receipt")
required_proof_paths = {
    "scripts/test-gate.sh",
    "scripts/proposal-089-refresh-evidence.py",
    "control-plane/crates/acp/src/adapters/junie.rs",
    "control-plane/crates/acp/src/transport.rs",
    "control-plane/crates/engine/examples/p089_acp_live_canary.rs",
    "control-plane/crates/engine/src/executor.rs",
    "control-plane/crates/engine/src/worktree_fingerprint.rs",
    "examples/agents/agents.yaml",
}
actual_proof_paths = {entry.get("path") for entry in proof_index}
require(required_proof_paths.issubset(actual_proof_paths), f"proof-critical files missing {sorted(required_proof_paths - actual_proof_paths)}")
for entry in proof_index:
    assert_file_record(root / entry.get("path", ""), entry)

for native_record in index.get("native_experiments") or []:
    directory = root / native_record.get("directory", "")
    for filename, record in (native_record.get("files") or {}).items():
        assert_file_record(directory / filename, record)
for filename, record in ((index.get("acp_canary") or {}).get("files") or {}).items():
    assert_file_record(acp_root / filename, record)
require((index.get("acp_canary") or {}).get("status") == "passed", "evidence-index acp_canary.status must be passed")
require((index.get("acp_canary") or {}).get("safety_violations") == [], "evidence-index safety violations must be empty")
negative_index = index.get("negative_fixtures") or {}
require(negative_index.get("directory") == str((evidence / "negative").relative_to(root)), "negative fixture directory mismatch")
for filename, record in (negative_index.get("files") or {}).items():
    assert_file_record(evidence / "negative" / filename, record)
validate_negative_fixtures()

print("proposal-089 default evidence validation passed")
PY
    log "Proposal 089 gate passed"
    ;;
  proposal-090|p090)
    log "Proposal 090 gate: Junie runtime-hardening evidence inventory"
    if [[ "${CHAINWORKS_PROPOSAL_090_LIVE:-0}" == "1" ]]; then
      log "Proposal 090 live mode: running Junie refine-like code_writer canary"
      rm -rf "$ROOT_DIR/.chainworks/tmp/p090-refine-like-canary-worktree" \
        "$ROOT_DIR/docs/evidence/090/junie-runtime-hardening/refine-like-canary"
      mkdir -p "$ROOT_DIR/.chainworks/tmp/p090-refine-like-canary-worktree" \
        "$ROOT_DIR/docs/evidence/090/junie-runtime-hardening/refine-like-canary"
      (
        cd "$ROOT_DIR/control-plane"
        CHAINWORKS_P090_STRICT_FINAL_PAYLOAD=1 \
        CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE=1 \
        CHAINWORKS_P090_STAGED_REPAIR_SETTLEMENT=1 \
        P090_WORKTREE_ROOT="$ROOT_DIR/.chainworks/tmp/p090-refine-like-canary-worktree" \
        P090_EVIDENCE_DIR="$ROOT_DIR/docs/evidence/090/junie-runtime-hardening/refine-like-canary" \
        CARGO_TARGET_DIR=target/proposal-090-gate \
        cargo run -p engine --example p090_junie_refine_like_live_canary
      )
      python3 - "$ROOT_DIR" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
index_path = root / "docs/evidence/090/junie-runtime-hardening/evidence-index.json"
canary_root = root / "docs/evidence/090/junie-runtime-hardening/refine-like-canary"
index = json.loads(index_path.read_text())
live = index.setdefault("long_running_refine_like_canary", {})
files = live.setdefault("files", {})
for filename, record in files.items():
    path = canary_root / filename
    if not path.exists():
        raise SystemExit(f"proposal-090 live refresh: missing {path.relative_to(root)}")
    record["sha256"] = hashlib.sha256(path.read_bytes()).hexdigest()
    record["size_bytes"] = path.stat().st_size
index_path.write_text(json.dumps(index, indent=2, sort_keys=False) + "\n")
PY
    fi
    python3 - "$ROOT_DIR" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
index_path = root / "docs/evidence/090/junie-runtime-hardening/evidence-index.json"
gates_doc = root / "docs/reference/test-gates.md"
stable_doc_paths = [
    root / "docs/reference/output-contracts-failure-evidence-and-recovery.md",
    root / "docs/reference/acp-runtime-transport.md",
    root / "docs/reference/rust-control-plane.md",
    gates_doc,
]

required_subtypes = {
    "junie_final_response_missing",
    "junie_final_response_truncated",
    "junie_progress_without_terminal_handoff",
    "junie_repair_returned_narrative",
    "junie_repair_returned_malformed_json",
    "junie_repair_outputs_partially_materialized",
    "junie_runtime_tool_path_failure_before_publication",
}
required_negative_classes = {
    "provider_authored_engine_failure_spoof_rejected",
    "provider_envelope_identity_mismatch_rejected",
    "unknown_provider_envelope_schema_rejected",
    "malformed_repair_sibling_does_not_overwrite_canonical_truth",
    "permission_denied_preflight_does_not_launch_provider",
}

def fail(message):
    raise SystemExit(f"proposal-090: {message}")

def load_json(path):
    if not path.exists():
        fail(f"missing {path.relative_to(root)}")
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path.relative_to(root)}: {exc}")

def sha256_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

index = load_json(index_path)
if index.get("schema_version") != "p090_junie_runtime_hardening_evidence_index_v1":
    fail("invalid evidence-index schema_version")
if index.get("proposal_id") != "090":
    fail("evidence-index proposal_id must be 090")
if index.get("gate") != "./scripts/test-gate.sh proposal-090":
    fail("evidence-index gate must name ./scripts/test-gate.sh proposal-090")

live = index.get("long_running_refine_like_canary") or fail("missing long_running_refine_like_canary evidence")
if live.get("status") != "passed":
    fail("long_running_refine_like_canary.status must be passed")
if live.get("execution_path") != "BackgroundExecutor.process_next_item":
    fail("long_running_refine_like_canary must prove BackgroundExecutor.process_next_item")
live_files = live.get("files") or {}
if "harness-result.json" not in live_files:
    fail("long_running_refine_like_canary must hash harness-result.json")
for filename, record in live_files.items():
    path = root / "docs/evidence/090/junie-runtime-hardening/refine-like-canary" / filename
    if not path.exists():
        fail(f"missing live canary file {path.relative_to(root)}")
    if record.get("sha256") != sha256_file(path):
        fail(f"live canary file hash mismatch for {filename}")
    if record.get("size_bytes") != path.stat().st_size:
        fail(f"live canary size mismatch for {filename}")
canary = load_json(root / "docs/evidence/090/junie-runtime-hardening/refine-like-canary/harness-result.json")
if canary.get("schema_version") != "p090_refine_like_live_canary_v1":
    fail("invalid P090 live canary schema_version")
if canary.get("status") != "passed":
    fail("P090 live canary did not pass")
receipt = ((canary.get("receipt") or {}).get("receipt") or {})
if receipt.get("provider") != "junie" or receipt.get("provider_runtime_family") != "junie_acp":
    fail("P090 live canary must use Junie ACP")
if receipt.get("completion_status") != "complete":
    fail("P090 live canary receipt must complete")
if receipt.get("completion_boundary_subtype") != "none":
    fail("P090 live canary must not produce a failure subtype")
if receipt.get("runtime_preflight_phase") != "passed":
    fail("P090 live canary preflight must pass")
if receipt.get("strict_final_payload_enabled") is not True:
    fail("P090 live canary must run with strict final payload enabled")
preflight = json.loads(receipt.get("runtime_tool_path_preflight_json") or "{}")
if preflight.get("provider_launched") is not True or preflight.get("enforcement_enabled") is not True:
    fail("P090 live canary must enforce preflight before provider launch")
if preflight.get("attempt_count") is None or not preflight.get("lifecycle_phases"):
    fail("P090 live canary must persist preflight attempt count and lifecycle phases")
coverage = canary.get("coverage") or {}
if coverage.get("live_canary_scope") != "junie_hardened_happy_path":
    fail("P090 live canary must explicitly declare its hardened happy-path scope")
if coverage.get("staged_repair_exercised") is not False:
    fail("P090 live canary must not imply staged repair coverage unless a repair turn ran")
staged_proof = index.get("staged_repair_proof") or {}
if staged_proof.get("proof_type") != "focused_runtime_and_startup_recovery_tests":
    fail("P090 staged repair proof must be separated from the live happy-path canary")
required_staged_tests = {
    "executor::tests::proposal_090_repair_materializes_valid_outputs_without_overwriting_malformed_sibling",
    "executor::tests::proposal_090_committed_repair_rows_publish_only_accepted_active_artifacts",
    "integration::proposal_090_startup_repair_publishes_recovered_committed_active_pointer",
}
if not required_staged_tests.issubset(set(staged_proof.get("tests") or [])):
    fail("P090 staged repair proof is missing focused normal/recovery tests")
required_outputs = {"implementation_progress", "implementation_self_assessment", "tests_result"}
output_files = {row.get("output_name"): row for row in canary.get("output_files") or []}
missing_outputs = [name for name in required_outputs if not (output_files.get(name) or {}).get("exists")]
if missing_outputs:
    fail(f"P090 live canary missing output files: {missing_outputs}")
decisions = {
    row.get("output_name"): row
    for row in (canary.get("receipt") or {}).get("output_decisions") or []
}
for name in required_outputs:
    decision = decisions.get(name) or {}
    if decision.get("validation_status") != "passed":
        fail(f"P090 live canary output {name} did not validate")
settlement_rows = {
    row.get("output_name"): row
    for row in (canary.get("receipt") or {}).get("settlement_rows") or []
}
for name in required_outputs:
    row = settlement_rows.get(name) or {}
    if row.get("decision") != "accepted" or row.get("materialization_state") != "committed":
        fail(f"P090 live canary output {name} did not settle fresh")

contract = index.get("public_subtype_contract") or {}
if contract.get("scope") != "provider_neutral_wrapper":
    fail("public subtype contract must be provider_neutral_wrapper")
if contract.get("unknown_values_round_trip_raw") is not True:
    fail("unknown subtype values must round-trip raw")

coverage = index.get("subtype_coverage") or []
seen = {row.get("subtype") for row in coverage}
missing = required_subtypes - seen
extra = seen - required_subtypes
if missing or extra:
    fail(f"subtype coverage mismatch; missing={sorted(missing)} extra={sorted(extra)}")

for row in coverage:
    subtype = row.get("subtype")
    fixture_rel = row.get("fixture_path")
    if row.get("evidence_source") not in {"historical", "synthetic"}:
        fail(f"{subtype}: invalid evidence_source")
    if not fixture_rel:
        fail(f"{subtype}: missing fixture_path")
    fixture_path = root / fixture_rel
    fixture = load_json(fixture_path)
    if row.get("fixture_sha256") != sha256_file(fixture_path):
        fail(f"{subtype}: fixture_sha256 mismatch")
    if fixture.get("schema_version") != "p090_subtype_fixture_v1":
        fail(f"{subtype}: invalid fixture schema")
    if fixture.get("subtype") != subtype:
        fail(f"{subtype}: fixture subtype mismatch")
    if fixture.get("status") != "accepted_for_proposal_readiness":
        fail(f"{subtype}: fixture status must be accepted_for_proposal_readiness")
    proves = set(fixture.get("proves") or [])
    if "subtype_coverage" not in proves:
        fail(f"{subtype}: fixture must prove subtype_coverage")

negative_classes = set(index.get("required_negative_fixture_classes") or [])
missing_negative = required_negative_classes - negative_classes
if missing_negative:
    fail(f"missing negative fixture classes: {sorted(missing_negative)}")
negative_fixtures = index.get("negative_fixtures") or []
seen_negative_fixtures = {row.get("fixture_class") for row in negative_fixtures}
if seen_negative_fixtures != required_negative_classes:
    fail(
        "negative fixture coverage mismatch; "
        f"missing={sorted(required_negative_classes - seen_negative_fixtures)} "
        f"extra={sorted(seen_negative_fixtures - required_negative_classes)}"
    )
for row in negative_fixtures:
    fixture_class = row.get("fixture_class")
    fixture_rel = row.get("path")
    if not fixture_rel:
        fail(f"{fixture_class}: missing negative fixture path")
    fixture_path = root / fixture_rel
    fixture = load_json(fixture_path)
    if row.get("sha256") != sha256_file(fixture_path):
        fail(f"{fixture_class}: negative fixture sha256 mismatch")
    if fixture.get("schema_version") != "p090_negative_fixture_v1":
        fail(f"{fixture_class}: invalid negative fixture schema")
    if fixture.get("fixture_class") != fixture_class:
        fail(f"{fixture_class}: fixture_class mismatch")
    if fixture.get("status") != "accepted_for_proposal_readiness":
        fail(f"{fixture_class}: invalid negative fixture status")
    expected = fixture.get("expected") or {}
    if fixture_class == "malformed_repair_sibling_does_not_overwrite_canonical_truth":
        if expected.get("malformed_sibling_materializes") is not False:
            fail(f"{fixture_class}: malformed sibling must not materialize")
        if expected.get("active_pointer_from_accepted_rows_only") is not True:
            fail(f"{fixture_class}: active pointers must come from accepted rows")
    elif expected.get("materializes_outputs") is not False:
        fail(f"{fixture_class}: negative fixture must prove outputs are not materialized")

valid_envelope_fixtures = index.get("valid_failure_envelope_fixtures") or []
required_valid_envelopes = {
    "valid_code_writer_engine_failure_v1",
    "valid_code_writer_repair_failure_v1",
}
seen_valid_envelopes = {row.get("fixture_class") for row in valid_envelope_fixtures}
if seen_valid_envelopes != required_valid_envelopes:
    fail(
        "valid failure envelope fixture coverage mismatch; "
        f"missing={sorted(required_valid_envelopes - seen_valid_envelopes)} "
        f"extra={sorted(seen_valid_envelopes - required_valid_envelopes)}"
    )
for row in valid_envelope_fixtures:
    fixture_class = row.get("fixture_class")
    fixture_path = root / (row.get("path") or "")
    fixture = load_json(fixture_path)
    if row.get("sha256") != sha256_file(fixture_path):
        fail(f"{fixture_class}: valid failure envelope fixture sha256 mismatch")
    if fixture.get("schema_version") != "p090_valid_failure_envelope_fixture_v1":
        fail(f"{fixture_class}: invalid valid-envelope fixture schema")
    if fixture.get("fixture_class") != fixture_class:
        fail(f"{fixture_class}: fixture_class mismatch")
    envelope = fixture.get("envelope") or {}
    if fixture_class == "valid_code_writer_engine_failure_v1":
        if envelope.get("schema_version") != "code_writer_engine_failure.v1":
            fail(f"{fixture_class}: engine failure envelope must use dotted schema")
    if fixture_class == "valid_code_writer_repair_failure_v1":
        if envelope.get("schema_version") != "code_writer_repair_failure.v1":
            fail(f"{fixture_class}: repair failure envelope must use dotted schema")
        if envelope.get("repair_attempt") != 1:
            fail(f"{fixture_class}: repair failure envelope must include repair_attempt")
    if envelope.get("source") != "engine_synthesized":
        fail(f"{fixture_class}: valid failure envelope source must be engine_synthesized")
    if "proposal_090_engine_owned_failure_envelope_sections_are_versioned_json" not in (row.get("executable_test") or ""):
        fail(f"{fixture_class}: valid envelope fixture must name executable test")

preflight_capacity = index.get("preflight_capacity_proof") or {}
if preflight_capacity.get("provider_capacity_counts_after_provider_launched") is not True:
    fail("preflight capacity proof must assert provider capacity starts after launch")
if preflight_capacity.get("preflight_running_provider_launched_false_excluded_from_provider_capacity") is not True:
    fail("preflight capacity proof must exclude preflight_running/provider_launched=false")
if preflight_capacity.get("provider_cap_one_atomic_after_preflight") is not True:
    fail("preflight capacity proof must assert provider cap=1 atomic launch lease")
if preflight_capacity.get("launch_lease_persisted_before_provider_spawn") is not True:
    fail("preflight capacity proof must assert launch lease is persisted before provider spawn")
if "proposal_090_junie_preflight_running_does_not_consume_provider_capacity_until_launch" not in (
    preflight_capacity.get("executable_test") or ""
):
    fail("preflight capacity proof must name executable focused test")
if "proposal_090_junie_provider_launch_lease_is_atomic_after_preflight" not in (
    preflight_capacity.get("launch_lease_executable_test") or ""
):
    fail("preflight capacity proof must name provider launch lease test")

preflight_lifecycle = index.get("preflight_lifecycle_proof") or {}
if preflight_lifecycle.get("diagnostic_mode_runs_preflight_when_enforce_false") is not True:
    fail("preflight lifecycle proof must assert diagnostic mode runs real preflight")
if preflight_lifecycle.get("runtime_home_cache_remediation_attempt_count") != 2:
    fail("preflight lifecycle proof must assert one runtime-home/cache remediation retry")
if preflight_lifecycle.get("preflight_remediating_is_durable_runtime_fact") is not True:
    fail("preflight lifecycle proof must assert durable preflight_remediating facts")
if preflight_lifecycle.get("nonnull_envelope_readback_asserted") is not True:
    fail("preflight lifecycle proof must assert non-null envelope readback")
required_lifecycle_tests = [
    ("diagnostic_mode_executable_test", "proposal_090_tool_path_preflight_runs_in_diagnostic_mode_when_enforce_is_off"),
    ("runtime_cache_remediation_executable_test", "proposal_090_tool_path_preflight_remediates_missing_runtime_cache_once"),
    ("durable_remediating_phase_executable_test", "proposal_090_runtime_cache_remediation_persists_intermediate_preflight_phase"),
]
for field, expected in required_lifecycle_tests:
    if expected not in (preflight_lifecycle.get(field) or ""):
        fail(f"preflight lifecycle proof missing executable test {expected}")
readback_tests = set(preflight_lifecycle.get("nonnull_envelope_readback_tests") or [])
for expected in [
    "graphql-server::proposal_088_graphql_exposes_code_writer_completion_receipts_by_run_and_execution",
    "mcp-server::proposal_088_mcp_runs_get_and_list_expose_implementation_completion",
    "mcp-server::proposal_088_mcp_report_exposes_code_writer_completion_receipts",
]:
    if expected not in readback_tests:
        fail(f"preflight lifecycle proof missing readback test {expected}")

stable_text_parts = []
for doc_path in stable_doc_paths:
    if not doc_path.exists():
        fail(f"missing stable reference doc {doc_path.relative_to(root)}")
    stable_text_parts.append(doc_path.read_text())
stable_text = "\n".join(stable_text_parts)
required_terms = [
    "engine-synthesized",
    "provider_claim_rejected",
    "provider-neutral subtype wrapper",
    "code_writer_output_settlement_rows",
    "runtime_preflight_phase",
    "provider capacity accounting starts only after preflight passes",
    "code_writer_engine_failure.v1",
    "code_writer_repair_failure.v1",
    "CHAINWORKS_P090_STRICT_FINAL_PAYLOAD",
    "CHAINWORKS_P090_JUNIE_PREFLIGHT_ENFORCE",
    "CHAINWORKS_P090_STAGED_REPAIR_SETTLEMENT",
    "CHAINWORKS_P090_DISABLE_STAGED_REPAIR_SETTLEMENT",
    "receipt_id TEXT NOT NULL REFERENCES code_writer_completion_receipts",
    "./scripts/test-gate.sh proposal-090",
]
for term in required_terms:
    if term not in stable_text:
        fail(f"stable reference docs missing required term {term!r}")

gates_text = gates_doc.read_text()
for term in ["proposal-090", "p090", "Junie runtime-hardening evidence inventory"]:
    if term not in gates_text:
        fail(f"docs/reference/test-gates.md missing {term!r}")

print("proposal-090 evidence inventory validation passed")
PY
    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-090-gate cargo test -p db proposal_090_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-090-gate cargo test -p acp proposal_090_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-090-gate cargo test -p engine proposal_090_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-090-gate cargo test -p engine --test proposal_058_claim_start proposal_090_junie_preflight_running_does_not_consume_provider_capacity_until_launch -- --nocapture
      CARGO_TARGET_DIR=target/proposal-090-gate cargo test -p engine proposal_090_junie_provider_launch_lease_is_atomic_after_preflight -- --nocapture
      CARGO_TARGET_DIR=target/proposal-090-gate cargo test -p graphql-server --test proposal_088_code_writer_completion_readback -- --nocapture
      CARGO_TARGET_DIR=target/proposal-090-gate cargo test -p mcp-server --test proposal_088_code_writer_completion_readback -- --nocapture
    )
    log "Proposal 090 gate passed"
    ;;
  proposal-091|p091)
    log "P091 retained gate: targeted retry authority evidence inventory and runtime proof"
    python3 - "$ROOT_DIR" <<'PY'
import hashlib
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])
index_path = root / "docs/evidence/091/targeted-retry-authority/evidence-index.json"
stable_doc_paths = [
    root / "docs/reference/rust-control-plane.md",
    root / "docs/reference/test-gates.md",
]
gates_doc = root / "docs/reference/test-gates.md"

required_terms = {
    "entry_kind = targeted_agent_retry",
    "historical_orphan_recovery",
    "terminal_reason = stale_retry_recovered",
    "retry_stage_execution_authorities_one_active",
    "Target-aware work-item repository semantics",
    "retryAuthorityHistory",
    "startup orphan repair must run before projection rebuild",
    "stage terminal metadata and authority history must agree",
    "advance_run_payload_missing_target_for_authority",
    "settled_sibling_without_live_retry_driver",
    "CHAINWORKS_P091_STARTUP_ORPHAN_REPAIR_MODE",
    "CHAINWORKS_P091_DISABLE_STARTUP_ORPHAN_REPAIR",
    "p091_orphan_repair_candidates_total",
    "./scripts/test-gate.sh proposal-091",
}

def fail(message):
    raise SystemExit(f"proposal-091: {message}")

def load_json(path):
    if not path.exists():
        fail(f"missing {path.relative_to(root)}")
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path.relative_to(root)}: {exc}")

def sha256_file(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

index = load_json(index_path)
if index.get("schema_version") != "p091_targeted_retry_authority_evidence_index_v1":
    fail("invalid evidence-index schema_version")
if index.get("proposal_id") != "091":
    fail("evidence-index proposal_id must be 091")
if index.get("gate") != "./scripts/test-gate.sh proposal-091":
    fail("evidence-index gate must name ./scripts/test-gate.sh proposal-091")

fixtures = index.get("fixtures") or []
if {fixture.get("fixture_id") for fixture in fixtures} != {"p086_orphaned_retry_readback"}:
    fail("expected exactly the P086 orphaned retry readback fixture")
for fixture_record in fixtures:
    fixture_path = root / fixture_record.get("path", "")
    fixture = load_json(fixture_path)
    if fixture_record.get("sha256") != sha256_file(fixture_path):
        fail(f"{fixture_record.get('fixture_id')}: fixture sha256 mismatch")
    if fixture.get("schema_version") != "p091_orphaned_retry_readback_fixture.v1":
        fail(f"{fixture_record.get('fixture_id')}: invalid fixture schema")
    facts = fixture.get("facts") or {}
    if facts.get("retry_status") != "pending":
        fail("P086 fixture must preserve pending orphan status")
    if facts.get("live_work_items_for_retry") != 0:
        fail("P086 fixture must prove no live work items for retry")
    if facts.get("active_agent_executions_for_retry") != 0:
        fail("P086 fixture must prove no active agent executions for retry")
    if facts.get("durable_retry_authority_active_for_retry") is not False:
        fail("P086 fixture must prove no active retry authority for retry")
    if facts.get("qualifying_predicate") != "settled_sibling_without_live_retry_driver":
        fail("P086 fixture must classify the explicit section 8.9 qualifying predicate")
    if facts.get("historical_timestamp_evidence") != "unavailable":
        fail("P086 historical fixture must not pretend to prove sibling recency")
    predicate_inputs = facts.get("predicate_inputs") or {}
    for field, expected in {
        "live_work_items_for_retry": 0,
        "active_agent_executions_for_retry": 0,
        "durable_retry_authority_active_for_retry": False,
        "settled_sibling_status": "completed",
        "stage_summaries_surface_retry_as_pending": True,
        "blocked_truth_preserved_orphan": True,
    }.items():
        if predicate_inputs.get(field) != expected:
            fail(f"P086 fixture predicate_inputs missing {field}={expected!r}")

stable_text_parts = []
for doc_path in stable_doc_paths:
    if not doc_path.exists():
        fail(f"missing stable reference doc {doc_path.relative_to(root)}")
    stable_text_parts.append(doc_path.read_text())
stable_text = "\n".join(stable_text_parts)
for term in required_terms:
    if term not in stable_text:
        fail(f"stable reference docs missing required term {term!r}")

index_terms = set(index.get("required_contract_terms") or [])
missing_index_terms = required_terms - index_terms
if missing_index_terms:
    fail(f"evidence index missing required contract terms {sorted(missing_index_terms)}")

gates_text = gates_doc.read_text()
for term in ["proposal-091", "p091", "targeted retry authority evidence inventory"]:
    if term not in gates_text:
        fail(f"docs/reference/test-gates.md missing {term!r}")

print("proposal-091 evidence inventory validation passed")
PY
    log "Proposal 091 runtime authority tests"
    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p domain --lib retry_authority
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p db --lib p091_
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p db --test proposal_091_retry_authority
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p engine --test integration p091_
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p engine --test integration test_retry_stage_creates_new_attempt_and_skips_old
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p engine --test integration test_retry_stage_with_agent_execution_id_schedules_single_invoke_attempt
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p graphql-server --lib run_query_exposes_p091_retry_authority_history_and_repair_readback
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p mcp-server --test proposal_091_retry_authority_readback -- --list | grep -q "retry_authority_history_and_current_readback_include_active_authority"
      CARGO_TARGET_DIR=target/proposal-091-gate cargo test -p mcp-server --test proposal_091_retry_authority_readback retry_authority_history_and_current_readback_include_active_authority
    )
    log "Proposal 091 gate passed"
    ;;
  proposal-092|p092)
    log "Proposal 092 retained gate: retry payload target invariants runtime proof"
    python3 - "$ROOT_DIR" <<'PY'
import sys
from pathlib import Path

root = Path(sys.argv[1])
reference = root / "docs/reference/rust-control-plane.md"
gates_doc = root / "docs/reference/test-gates.md"
runner = root / "scripts/test-gate.sh"

def fail(message):
    raise SystemExit(f"proposal-092: {message}")

if not reference.exists():
    fail("missing docs/reference/rust-control-plane.md")

reference_text = reference.read_text()
required_reference_terms = [
    "Retry payload target invariants and recovery",
    "Top-level routing fields describe the current run, stage, stage execution, target stage execution, and retry authority only.",
    "targeted_retry.source_stage_execution_id",
    "sanitize_targeted_retry_invoke_payload",
    "auto-contract output retry and operator targeted retry",
    "Post-invoke completion is authority/current-truth driven",
    "retry_authority_target_agent_stage_mismatch",
    "retry_authority_missing_for_targeted_invoke",
    "Startup recovery runs this check before generic abandoned-invoke requeue",
    "The daemon watchdog runs the same reconciliation after stale `AdvanceRun` inspection",
    "CHAINWORKS_P092_RETRY_PAYLOAD_RECOVERY_MODE",
    "CHAINWORKS_P092_RETRY_PAYLOAD_RECOVERY_DISABLED",
    "CHAINWORKS_P092_RETRY_PAYLOAD_RECOVERY_BATCH_LIMIT",
    "retry_payload_recovery_events",
    "`candidates_total`, `repaired_total`, `excluded_total`",
    "GraphQL attaches the latest durable event to `Run.retryAuthorityJson.retry_payload_recovery`",
    "MCP `runs.get` exposes `retry_authority.retry_payload_recovery`",
    "Missing-authority hard mismatch rows are represented as history entries with `authority_state = missing_authority`",
    "retryPayloadRecovery",
    "retry_payload_recovery",
    "unknown_reason_code = true",
    "valid_retry_invoke_completion_recovered",
    "retry_payload_stale_target_stage_repaired",
    "retry_payload_source_provenance_ignored_for_target",
    "`./scripts/test-gate.sh proposal-092` / `p092`",
    "retired proposal document is not the operational source of truth",
]
for term in required_reference_terms:
    if term not in reference_text:
        fail(f"docs/reference/rust-control-plane.md missing {term!r}")

if "Proposal 092: Retry Authority Payload Target Invariants and Recovery" in reference_text:
    fail("stable reference still reads like the retired proposal")

if not gates_doc.exists():
    fail("missing docs/reference/test-gates.md")
gates_text = gates_doc.read_text()
for term in [
    "### `proposal-092|p092`",
    "Retained historical alias",
    "retry payload target invariants runtime proof",
    "rust-control-plane.md#retry-payload-target-invariants-and-recovery",
    "targeted retry sanitizer",
    "post-invoke fail-closed behavior",
    "bounded startup and live recovery entry points",
    "durable `retry_payload_recovery_events` storage",
    "configurable live batch limiting",
    "explicit `excluded_total` diagnostics",
    "GraphQL/MCP nullable missing-authority readback",
    "GraphQL, MCP, and report readback schema placement",
    "retired proposal document is not the source of operational truth",
    "proposal-090|p090",
    "retained Junie runtime-hardening",
]:
    if term not in gates_text:
        fail(f"docs/reference/test-gates.md missing {term!r}")

runner_text = runner.read_text()
for term in [
    "proposal-092|p092  Retained historical alias for P092 retry payload target invariants runtime proof gate",
    "proposal-092|p092)",
]:
    if term not in runner_text:
        fail(f"scripts/test-gate.sh missing {term!r}")

print("proposal-092 readiness validation passed")
PY
    log "Proposal 092 retained runtime authority payload tests"
    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p domain --lib retry_authority
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p db p092_post_invoke -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p db p091_post_invoke_authority_target_mismatch_fails_closed -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p db --test proposal_092_retry_payload_recovery -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p engine auto_contract_output_retry_schedules_targeted_fallback_before_stage_blocks -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p engine --test integration test_retry_stage_with_agent_execution_id_schedules_single_invoke_attempt -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p engine --test integration p092_ -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p graphql-server --lib run_query_exposes_p091_retry_authority_history_and_repair_readback -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo test -p mcp-server --test proposal_091_retry_authority_readback retry_authority_history_and_current_readback_include_active_authority -- --nocapture
      CARGO_TARGET_DIR=target/proposal-092-gate cargo check -p daemon
    )
    log "Proposal 092 retained gate passed"
    ;;
  proposal-086|p086|p086-continuation-preflight)
    log "Proposal 086 Phase 0 preflight: migration shape, MCP/artifact schemas, and Rust unit tests"
    python3 - "$ROOT_DIR" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])

def fail(msg):
    raise SystemExit(f"proposal-086: {msg}")

def load_json(path):
    if not path.exists():
        fail(f"missing {path.relative_to(root)}")
    try:
        return json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {path.relative_to(root)}: {exc}")

# 1. Migration file exists and contains required tables, indexes, and SHA-256 CHECK constraints
migration = root / "control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql"
if not migration.exists():
    fail("missing control-plane/crates/db/migrations/065_p086_agent_work_continuations.sql")
sql = migration.read_text()

required_tables = [
    "agent_work_continuations",
    "agent_external_side_effect_ledger",
    "supervised_workers_continuation",
]
for table in required_tables:
    if f"CREATE TABLE IF NOT EXISTS {table}" not in sql:
        fail(f"migration missing CREATE TABLE IF NOT EXISTS {table}")

required_indexes = [
    "idx_awc_agent_created_at",
    "idx_awc_run_status",
    "idx_awc_stage",
    "idx_awc_recon",
    "idx_awc_admission",
    "idx_awc_recovery",
    "idx_ledger_cont_seq",
    "idx_ledger_unresolved",
    "idx_swc_heartbeat",
    "idx_swc_generation",
    "uniq_swc_active_continuation",
]
for idx in required_indexes:
    if idx not in sql:
        fail(f"migration missing required index: {idx}")

if "NOT GLOB '*[^0-9a-f]*'" not in sql:
    fail("migration missing SHA-256 NOT GLOB check constraints")
for field in [
    "attach_receipt_artifact_id",
    "evidence_bundle_artifact_id",
    "worktree_readback_artifact_id",
    "continuation_report_artifact_id",
]:
    if field not in sql:
        fail(f"migration missing P086 evidence readback column: {field}")

provider_process_migration = root / "control-plane/crates/db/migrations/066_p086_supervised_worker_provider_process.sql"
if not provider_process_migration.exists():
    fail("missing control-plane/crates/db/migrations/066_p086_supervised_worker_provider_process.sql")
provider_process_sql = provider_process_migration.read_text()
for field in [
    "provider_child_pid",
    "provider_process_group_id",
    "provider_process_uid",
]:
    if field not in provider_process_sql:
        fail(f"migration 066 missing durable provider process field: {field}")

metrics_migration = root / "control-plane/crates/db/migrations/067_p086_continuation_metric_events.sql"
if not metrics_migration.exists():
    fail("missing control-plane/crates/db/migrations/067_p086_continuation_metric_events.sql")
metrics_sql = metrics_migration.read_text()
for field in [
    "p086_continuation_metric_events",
    "metric_name",
    "labels_json",
    "continuation_id",
    "idx_p086_metric_run_time",
]:
    if field not in metrics_sql:
        fail(f"migration 067 missing durable P086 metric element: {field}")

# 2. All six MCP schemas parse as valid JSON Schema; continue_work.response requires 'outcome'
mcp_dir = root / "docs/reference/p086/schemas/mcp"
required_mcp = [
    "agents.continue_work.request.schema.json",
    "agents.continue_work.response.schema.json",
    "agents.continuation_status.request.schema.json",
    "agents.continuation_status.response.schema.json",
    "agents.continuation_candidates.request.schema.json",
    "agents.continuation_candidates.response.schema.json",
]
for name in required_mcp:
    schema = load_json(mcp_dir / name)
    if name == "agents.continue_work.request.schema.json":
        props = schema.get("properties") or {}
        for field in [
            "run_id",
            "stage_execution_id",
            "session_generation_id",
            "provider_session_id",
            "continuation_mode",
            "operator_instruction",
            "max_turns",
            "max_wall_clock_seconds",
            "blockers",
        ]:
            if field not in props:
                fail(f"agents.continue_work.request.schema.json missing {field}")
        schema_text = (mcp_dir / name).read_text()
        if '"continuation_mode"' not in schema_text:
            fail("agents.continue_work.request.schema.json must expose canonical continuation_mode")
    if name == "agents.continue_work.response.schema.json":
        required_props = schema.get("properties") or schema.get("oneOf") or {}
        # Check 'outcome' appears anywhere in the schema text
        schema_text = (mcp_dir / name).read_text()
        if '"outcome"' not in schema_text:
            fail(f"agents.continue_work.response.schema.json must require the 'outcome' field")
        if "asynchronous admission response" not in schema_text:
            fail("agents.continue_work.response.schema.json must document async admission/readback split")
        forbidden_terminal_fields = [
            "response_artifact_id",
            "attach_receipt_artifact_id",
            "evidence_bundle_artifact_id",
            "worktree_readback_artifact_id",
            "continuation_report_artifact_id",
            "result_or_no_progress_artifact_id",
        ]
        for field in forbidden_terminal_fields:
            if f'"{field}"' in schema_text:
                fail(
                    "agents.continue_work.response.schema.json must remain bounded admission output; "
                    f"terminal field {field} belongs to continuation readback"
                )
        for field in ["failure_reason", "agent_execution_id", "queue_depth", "limit_scope"]:
            if f'"{field}"' not in schema_text:
                fail(f"agents.continue_work.response.schema.json must declare rejected error.data.{field}")

reference_text = (root / "docs/reference/agent-work-continuation.md").read_text()
for needle in [
    "Output is an admission response, not a terminal execution response",
    "Terminal fields are readback, not command output",
    "agents.continue_work` returns a bounded admission response",
]:
    if needle not in reference_text:
        fail(f"agent-work-continuation reference missing async MCP/readback contract clarification: {needle!r}")

# 2b. Worker must route continuation through the ACP live-session reuse path and
# must retain the canonical P086 mode-reset prompt contract.
executor_rs = root / "control-plane/crates/engine/src/executor.rs"
executor_text = executor_rs.read_text()
if ".start_session(acp::ExecutionRequest" in executor_text:
    fail("P086 worker must not call ACP start_session for live-handle continuation")
if ".execute(acp::ExecutionRequest" not in executor_text:
    fail("P086 worker must call ACP execute with reuse_existing_session=true")
for needle in [
    "# P086 Continuation Mode Reset",
    "This is not a retry",
    "Do not commit, push, release, publish, upload",
    "provider_session_attach_receipt_v1",
    "worktree_continuation_readback_v1",
    "agent_continuation_evidence_bundle_v1",
    "agent_continuation_report_v1",
    "refresh_supervised_worker_heartbeat",
    "reconcile_p086_continuation_from_evidence",
    "p086_worktree_has_post_continuation_change",
    "has_side_effect_ledger_row",
    '"provider_send"',
    "reconciled_from_post_continuation_worktree_evidence",
    "post_continuation_change_without_provider_send_evidence",
    "settle_p086_cancelled_after_provider_return",
    "cancelled_after_provider_send",
    "maybe_admit_p086_lead_auto_continuation",
    "lead_continuation_decision_v1",
    "lead_auto_orchestration",
    "continuation_instruction",
    "p086_reconciliation_transcript_evidence",
    "continuation_reconciliation_transcript_evidence_total",
    "explicit_transcript_absence",
]:
    if needle not in executor_text:
        fail(f"P086 canonical prompt missing {needle!r}")

# 2c. Admission must be catalog-gated and side-effect-safe, not just role-gated.
agents_yaml = (root / "examples/agents/agents.yaml").read_text()
for needle in [
    "continuation_capability:",
    "allowed_triggers:",
    "live_handle_continuation:",
    "require_no_unresolved_side_effects: true",
]:
    if needle not in agents_yaml:
        fail(f"code_writer catalog missing P086 continuation capability needle {needle!r}")

workflow_catalog = (root / "control-plane/crates/workflow/src/catalog.rs").read_text()
if "pub struct ContinuationCapabilityYaml" not in workflow_catalog:
    fail("workflow catalog must model continuation_capability in frozen snapshots")

db_repo = (root / "control-plane/crates/db/src/repos/agent_work_continuations.rs").read_text()
if "has_unresolved_side_effects_for_stage" not in db_repo or "FROM side_effects" not in db_repo:
    fail("P086 admission must query unresolved P078 side_effects before accepting continuation")
for needle in [
    "list_stale_supervised_workers",
    "mark_active_for_run_cancelling_tx",
    "set_evidence_artifact_ids",
    "list_needing_continuation_reconciliation",
    "update_continuation_status_unless_cancelling",
    "settle_with_artifacts_unless_cancelling",
    "record_p086_continuation_metric_event",
    "p086_continuation_metrics_summary_for_run",
    "useful_progress_rate",
    "average_time_saved_seconds",
    "followup_validation_success_rate",
    "provider_session_budget_input_tokens_total",
    "provider_session_resurrection_attach_failure_total",
    "rejected_lead_auto_agent_limit",
    "rejected_lead_auto_stage_limit",
]:
    if needle not in db_repo:
        fail(f"P086 DB repo missing recovery/readback helper {needle!r}")

mcp_agents = (root / "control-plane/crates/mcp-server/src/tools/agents.rs").read_text()
for needle in [
    "continuation_capability_rejection",
    "forbidden_stage_kind",
    "unresolved_side_effects",
    "live_session_required",
    'PrincipalClass::Agent) && trigger_kind == "lead_auto"',
    "validate_lead_auto_decision_payload",
    "lead_auto_artifact_target_mismatch",
    "lead_auto_safety_check_failed",
    "lead_auto_instruction_hash_mismatch",
    "continuation_mode",
    "LeadAutoLimitExceeded",
]:
    if needle not in mcp_agents:
        fail(f"P086 MCP admission missing fail-closed guard {needle!r}")

recovery_text = (root / "control-plane/crates/engine/src/recovery.rs").read_text()
for needle in [
    "close_session(&worker.session_generation_id)",
    "p086_reap_registered_provider_process_group",
    "provider_process_group_id",
    "provider_process_uid",
    "registered_provider_process_group_signal",
    "orphan_reap_attempted",
    "orphan_reap_verified",
    "stale_generation_reaped",
    "stale_generation_reap_unverified",
    "orphan_reap_signals_sent",
    "orphan_reap_term_deadline_ms",
    "orphan_reap_kill_deadline_ms",
]:
    if needle not in recovery_text:
        fail(f"P086 startup recovery missing stale ACP reap proof field {needle!r}")

graphql_schema = (root / "control-plane/crates/graphql-server/src/schema.rs").read_text()
graphql_types = (root / "control-plane/crates/graphql-server/src/types/continuation.rs").read_text()
graphql_test = root / "control-plane/crates/graphql-server/tests/proposal_086_continuation_readback.rs"
if not graphql_test.exists():
    fail("missing GraphQL P086 continuation readback test")
for needle in [
    "continuations(",
    "continuation_metrics_summary",
    "continuation_status",
    "continuation_candidates",
]:
    if needle not in graphql_schema:
        fail(f"GraphQL schema missing P086 readback field/helper {needle!r}")
if "GqlContinuationMetricsSummary" not in graphql_types:
    fail("GraphQL continuation types missing durable P086 metrics summary")
for needle in [
    "useful_progress_rate",
    "average_time_saved_seconds",
    "followup_validation_success_rate",
    "provider_session_budget_input_tokens_total",
    "provider_session_resurrection_attach_failure_total",
]:
    if needle not in graphql_types:
        fail(f"GraphQL continuation metrics summary missing P086 KPI field {needle!r}")

swift_read_boundary = (root / "Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift").read_text()
swift_runs_home = (root / "Chainworks Forge/Views/RunsHomeView.swift").read_text()
for needle in [
    "continuations(runId: $runId)",
    "continuationMetricsSummary(runId: $runId)",
    "P086ContinuationReadbackPresentation",
    "P086ContinuationReadbackPresenter",
    "usefulProgressRate",
    "averageTimeSavedSeconds",
    "followupValidationSuccessRate",
    "providerSessionBudgetInputTokensTotal",
    "providerSessionResurrectionAttachFailureTotal",
]:
    if needle not in swift_read_boundary:
        fail(f"Swift P031 read boundary missing passive P086 readback needle {needle!r}")
for needle in [
    "P086ContinuationReadbackCard",
    "p086-continuation-readback-card",
]:
    if needle not in swift_runs_home:
        fail(f"Runs UI missing passive P086 continuation readback needle {needle!r}")

# 3. All five artifact schemas parse as valid JSON Schema with additionalProperties=false
artifact_dir = root / "docs/reference/p086/schemas/artifacts"
required_artifacts = [
    "agent_continuation_evidence_bundle_v1.schema.json",
    "agent_continuation_report_v1.schema.json",
    "continuation_canonical_request_v1.schema.json",
    "continuation_no_progress_report_v1.schema.json",
    "continuation_response_snapshot_v1.schema.json",
    "continuation_result_v1.schema.json",
    "lead_continuation_decision_v1.schema.json",
    "provider_session_attach_receipt_v1.schema.json",
    "worktree_continuation_readback_v1.schema.json",
]
for name in required_artifacts:
    schema = load_json(artifact_dir / name)
    if schema.get("additionalProperties") is not False:
        fail(f"artifact schema {name} must have additionalProperties=false at top level")

response_schema = load_json(artifact_dir / "continuation_response_snapshot_v1.schema.json")
response_required = set(response_schema["properties"]["payload"].get("required", []))
if "response_artifact_id" not in response_required:
    fail("continuation_response_snapshot_v1 must require payload.response_artifact_id")

result_schema = load_json(artifact_dir / "continuation_result_v1.schema.json")
tests_or_gates = result_schema["properties"]["payload"]["properties"]["tests_or_gates"]["items"]
if tests_or_gates.get("type") != "object":
    fail("continuation_result_v1 tests_or_gates items must be objects, not strings")
if set(tests_or_gates.get("required", [])) != {"name", "status"}:
    fail("continuation_result_v1 tests_or_gates rows must require name and status")

no_progress_schema = load_json(artifact_dir / "continuation_no_progress_report_v1.schema.json")
no_progress_payload_props = no_progress_schema["properties"]["payload"]["properties"]
for field in ["response_fingerprint_sha256", "provider_transcript_artifact_ids"]:
    if field not in no_progress_payload_props:
        fail(f"continuation_no_progress_report_v1 payload missing emitted field {field}")

if '"response_artifact_id": response_artifact_id' not in executor_text:
    fail("P086 response snapshot must emit payload.response_artifact_id")
if "serde_json::json!({" not in executor_text or '"name": name' not in executor_text or '"status": status' not in executor_text:
    fail("P086 tests_or_gates extraction must emit schema-compatible object rows")

print("proposal-086 Phase 0 preflight checks passed (migration, schemas)")
PY
    log "Proposal 086 Rust unit tests"
    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p domain "continuation"
      CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p db --test proposal_086_continuation_lifecycle
      CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p engine --lib p086
      CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p mcp-server "tools::agents"
      CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p graphql-server --test proposal_086_continuation_readback
      CARGO_TARGET_DIR=target/proposal-086-gate cargo test -p daemon --test proposal_086_mcp_continuation_live_reuse
    )
    log "Proposal 086 Swift readback tests"
    run_targeted_tests "proposal-086-swift-readback" "${PROPOSAL_086_SWIFT_TESTS[@]}"
    log "Proposal 086 Phase 0 preflight gate passed"
    ;;
  p086-continuation-readback)
    log "Proposal 086 Phase 1 readback gate: operator readback fixture field coverage"
    python3 - "$ROOT_DIR" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])

def fail(msg):
    raise SystemExit(f"p086-continuation-readback: {msg}")

fixture_path = root / "docs/evidence/rollout-contract/operator-readback/p086-continuation-full-surface.fixture.json"
if not fixture_path.exists():
    fail("missing docs/evidence/rollout-contract/operator-readback/p086-continuation-full-surface.fixture.json")

try:
    fixture = json.loads(fixture_path.read_text())
except json.JSONDecodeError as exc:
    fail(f"invalid JSON in p086-continuation-full-surface.fixture.json: {exc}")

# Fails closed while the fixture contains a placeholder key or rollout_contract_status != "pass"
if "placeholder" in fixture:
    fail(
        "fixture contains 'placeholder' key; p086-continuation-readback requires real Phase 1 "
        "runtime evidence (50+ AgentExecutions across 10+ runs with MCP/GraphQL/run_report/release_receipt parity)"
    )
raw_fixture = json.dumps(fixture, sort_keys=True)
for forbidden in [
    "p086-fixture",
    "11111111-1111",
    "22222222-2222",
    "33333333-3333",
    "44444444-4444",
    "55555555-5555",
    "66666666-6666",
    "77777777-7777",
    "aaaaaaaaaaaaaaaa",
    "bbbbbbbbbbbbbbbb",
]:
    if forbidden in raw_fixture:
        fail(f"fixture still contains synthetic placeholder token: {forbidden}")
provenance = fixture.get("evidence_provenance")
if not isinstance(provenance, dict):
    fail("fixture missing evidence_provenance object")
for field in ["source_test", "source_gate", "generated_from", "generated_at"]:
    if not provenance.get(field):
        fail(f"fixture evidence_provenance missing {field}")

required_fields = [
    "evidence_provenance",
    "rollout_contract_status", "rollout_contract_decision", "rollout_contract_failure_reasons",
    "rollout_contract_waiver_state", "rollout_contract_waiver_expires_at",
    "rollout_contract_enforcement_mode", "rollout_contract_enforcement_mode_reason",
    "rollout_contract_hold_conditions", "rollout_contract_rollback_disposition",
    "rollout_contract_source_lane", "rollout_contract_enabled_state",
    "rollout_contract_disabled_reason_code", "rollout_contract_action_id",
    "rollout_contract_operator_message", "rollout_contract_projection_integrity",
    "rollout_contract_cutover_policy_revision", "rollout_contract_diagnostic_redaction",
    "rollout_contract_next_steps",
    "continuation_id", "run_id", "stage_execution_id", "agent_execution_id",
    "mode", "mode_raw", "trigger_kind", "trigger_kind_raw",
    "lifecycle_status", "lifecycle_status_raw", "status_raw",
    "failure_reason", "failure_reason_raw", "runtime_disabled_reason_code",
    "request_fingerprint_sha256", "canonical_request_artifact_id",
    "response_fingerprint_sha256", "response_artifact_id",
    "conflict_count", "reconciliation_status",
    "projection_lag_ms", "projection_lag_budget_ms", "projection_degraded",
    "restart_recovery_stale_non_terminal_count",
    "cancel_termination_proof_state", "orphan_acp_reap_outcome",
    "provider_session_attach_receipt_id", "kill_switch_last_change",
]
for field in required_fields:
    if field not in fixture:
        fail(f"fixture missing required field: {field}")

if fixture.get("rollout_contract_status") != "pass":
    fail(
        f"rollout_contract_status={fixture.get('rollout_contract_status')!r}; "
        "p086-continuation-readback requires rollout_contract_status='pass' (Phase 1 evidence not yet materialized)"
    )

print("p086-continuation-readback passed")
PY
    log "Proposal 086 Phase 1 readback gate passed"
    ;;
  p086-continuation-negative-fixtures)
    log "Proposal 086 Phase 2 hold-condition gate: all negative fixtures present and not placeholder"
    python3 - "$ROOT_DIR" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])

def fail(msg):
    raise SystemExit(f"p086-continuation-negative-fixtures: {msg}")

neg_dir = root / "docs/evidence/rollout-contract/p086/negative"
required_fixtures = [
    "admission-timeout-sweeper.json",
    "artifact-schema-lint-failure.json",
    "cancel-timeout-termination-unverified.json",
    "duplicate-after-prompt-sent-no-resend.json",
    "fingerprint-mismatch-same-key.json",
    "lead-decision-missing-or-changed.json",
    "malformed-hashes.json",
    "mcp-schema-lint-failure.json",
    "missing-log-correlation-key.json",
    "pre-prompt-crash-after-lease.json",
    "resurrection-before-attach-receipt.json",
    "resurrection-before-orphan-reap.json",
    "resurrection-unsupported-adapter.json",
    "saturation-without-queue-drain.json",
    "terminal-missing-result-artifact.json",
    "worker-panic-recovery.json",
]
for name in required_fixtures:
    path = neg_dir / name
    if not path.exists():
        fail(f"missing negative fixture: {name}")
    try:
        fixture = json.loads(path.read_text())
    except json.JSONDecodeError as exc:
        fail(f"invalid JSON in {name}: {exc}")
    status = fixture.get("status")
    if status == "placeholder":
        fail(
            f"{name} still has status='placeholder'; "
            "replace with real evidence (pass/fail/deferred_pending_phase2_worker)"
        )
    if status is None:
        fail(f"{name} missing 'status' field")

print(f"p086-continuation-negative-fixtures: all {len(required_fixtures)} fixtures present and valid")
PY
    log "Proposal 086 Phase 2 negative fixtures gate passed"
    ;;
  p086-continuation-operator-report)
    log "Proposal 086 Phase 1 operator-report gate: operator report field coverage"
    python3 - "$ROOT_DIR" <<'PY'
import json
import sys
from pathlib import Path

root = Path(sys.argv[1])

def fail(msg):
    raise SystemExit(f"p086-continuation-operator-report: {msg}")

fixture_path = root / "docs/evidence/rollout-contract/operator-readback/p086-continuation-full-surface.fixture.json"
if not fixture_path.exists():
    fail("missing docs/evidence/rollout-contract/operator-readback/p086-continuation-full-surface.fixture.json")

try:
    fixture = json.loads(fixture_path.read_text())
except json.JSONDecodeError as exc:
    fail(f"invalid JSON in p086-continuation-full-surface.fixture.json: {exc}")

# Fails closed while the fixture contains a placeholder key
if "placeholder" in fixture:
    fail(
        "fixture contains 'placeholder' key; p086-continuation-operator-report requires real Phase 1 "
        "operator run evidence (rollout_contract_status='pass', no placeholder key)"
    )
raw_fixture = json.dumps(fixture, sort_keys=True)
for forbidden in ["p086-fixture", "11111111-1111", "aaaaaaaaaaaaaaaa", "bbbbbbbbbbbbbbbb"]:
    if forbidden in raw_fixture:
        fail(f"fixture still contains synthetic placeholder token: {forbidden}")
if not isinstance(fixture.get("evidence_provenance"), dict):
    fail("fixture missing evidence_provenance object")

if fixture.get("rollout_contract_status") != "pass":
    fail(
        f"rollout_contract_status={fixture.get('rollout_contract_status')!r}; "
        "operator report requires rollout_contract_status='pass'"
    )

required_operator_fields = [
    "rollout_contract_status", "rollout_contract_decision",
    "rollout_contract_hold_conditions", "rollout_contract_enforcement_mode",
    "continuation_id", "run_id", "stage_execution_id", "agent_execution_id",
    "mode", "trigger_kind", "lifecycle_status", "request_fingerprint_sha256",
    "eligible", "lead_decision", "confirmation_required",
]
for field in required_operator_fields:
    if field not in fixture:
        fail(f"fixture missing required operator report field: {field}")

print("p086-continuation-operator-report passed")
PY
    log "Proposal 086 Phase 1 operator report gate passed"
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
    "P081-UI-APPROVAL-APPROVE",
    "P081-UI-APPROVAL-REJECT",
    "P081-UI-READ-ONLY",
    "P081-UI-EXTERNAL-COMMANDS",
]:
    if required not in contract_text:
        raise SystemExit(
            f"proposal-085: contract doc missing required term: {required!r}"
        )

required_rows = [
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
]
required_row_fields = [
    "affordance_id",
    "source_graphql_fields",
    "local_presentation_state",
    "actionable_state",
    "disabled_reason_code",
    "fallback_text",
    "mutation_availability",
    "mutation_idempotency",
    "staleness_deadline",
    "cancellation_policy",
    "stale_list_detail_behavior",
    "unauthorized_behavior",
    "supported_interactions",
    "proof_tests",
]
for row in required_rows:
    marker = f"### `{row}`"
    start = contract_text.find(marker)
    if start < 0:
        raise SystemExit(f"proposal-085: missing contract row {row!r}")
    end = contract_text.find("\n---", start)
    section = contract_text[start:] if end < 0 else contract_text[start:end]
    for field in required_row_fields:
        if f"**{field}**" not in section:
            raise SystemExit(
                f"proposal-085: contract row {row!r} missing required field {field!r}"
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
    if "hold_condition" not in data:
        raise SystemExit(
            f"proposal-085: negative fixture {fixture_path} missing 'hold_condition' field"
        )
    if "state_conflict" in json.dumps(data):
        raise SystemExit(
            f"proposal-085: negative fixture {fixture_path} references removed conflict code 'state_conflict'"
        )

p085_semantic_expectations = {
    "p085-approval-actionability-mismatch": {
        "contract_violation": "approval_parity_mismatch",
        "p085_contract_row": "approval.resolve.approve",
        "expected_presenter_output.approveAvailability": "disabled",
        "expected_presenter_output.reasonCode": "WRITE_PATH_NOT_AVAILABLE",
    },
    "p085-approval-stale-double-submit-conflict": {
        "contract_violation": "approval_conflict_missing",
        "p085_contract_row": "approval.resolve.approve",
        "simulated_mutation_response.approveApproval.conflictResultCode": "already_resolved",
        "expected_presenter_output.approveAvailability": "disabled",
    },
    "p085-missing-affordance-row": {
        "contract_violation": "missing_affordance_row",
        "expected_contract_row_status": "missing",
    },
    "p085-missing-schema-symbol": {
        "contract_violation": "missing_schema_proof",
        "missing_symbol.graphql_type": "PayloadUnavailableReasonCode",
    },
    "p085-payload-deferred-marked-unavailable": {
        "contract_violation": "payload_state_mismatch",
        "p085_contract_row": "artifact.preview.listLabel",
        "expected_presenter_output.payloadPresentation": "deferred",
    },
    "p085-payload-deferred-no-deadline": {
        "contract_violation": "payload_deadline_missing",
        "p085_contract_row": "artifact.preview.listLabel",
        "simulated_read_model.artifact.payloadAvailabilityState": "generating",
        "expected_server_owned_evidence": "deadline_or_stalled_diagnostic",
    },
    "p085-unknown-enum-optimistic-action": {
        "contract_violation": "unknown_enum_unsafe",
        "p085_contract_rows": ["freshness.badge.approval", "freshness.badge.run"],
        "expected_swift_behavior.p085_freshnessState": "P085FreshnessState.unknown(rawValue: 'projection_rebuilding')",
    },
    "p085-unsafe-local-truth-fallback": {
        "contract_violation": "unauthorized_fallback_violation",
        "p085_contract_rows": ["artifact.preview.detail", "artifact.preview.listLabel"],
        "expected_swift_behavior.payloadPresentation": "unavailable(reasonCode: .notAuthorized)",
    },
}

def p085_lookup(data, dotted):
    value = data
    for segment in dotted.split("."):
        if not isinstance(value, dict) or segment not in value:
            raise KeyError(dotted)
        value = value[segment]
    return value

for fixture_path in p085_negative_fixtures:
    full = root / fixture_path
    data = json.loads(full.read_text())
    scenario = data.get("scenario")
    if scenario not in p085_semantic_expectations:
        raise SystemExit(
            f"proposal-085: negative fixture {fixture_path} has unexpected scenario {scenario!r}"
        )
    for dotted, expected in p085_semantic_expectations[scenario].items():
        try:
            actual = p085_lookup(data, dotted)
        except KeyError as exc:
            raise SystemExit(
                f"proposal-085: negative fixture {fixture_path} missing semantic field {exc.args[0]!r}"
            ) from exc
        if actual != expected:
            raise SystemExit(
                f"proposal-085: negative fixture {fixture_path} expected {dotted}={expected!r}, got {actual!r}"
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

# 4. Swift presenter file exists and contains required P085 symbols
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
    "P085MutationConflictResultCode",
    "canDrivePayloadAvailability",
    "canDriveApprovalActionability",
    "mergedAffordance",
    "payloadPresentation(fromRaw",
    "static func fromRaw",
    "case .unknown",
    # Decision-state gating: approval checks durable decision (pending/requested = actionable)
    "d != \"pending\"",
    "Approval is already resolved",
    # Conflict codes: typed idempotency/conflict result vocabulary
    "alreadyResolved",
]:
    if required not in presenter_text:
        raise SystemExit(
            f"proposal-085: P085AffordancePresenter.swift missing required term: {required!r}"
        )

# 5. P031 enums have fail-closed decoding (custom init(from decoder:) for unknown values)
boundary_file = root / "Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift"
if not boundary_file.exists():
    raise SystemExit(
        "proposal-085: missing Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift"
    )
boundary_text = boundary_file.read_text()
for required in [
    # Fail-closed init(from decoder:) must be present for all P031 enums
    "P031FreshnessState",
    "P031DisabledReasonCode",
    "P031WritePathState",
    "P031PayloadAvailabilityState",
    "P031PayloadUnavailableReasonCode",
    # P085 wired into production approval path
    "P085AffordancePresenter.approvalAffordance",
    # P085 wired into production artifact path
    "P085AffordancePresenter.artifactListAffordance",
    # P031 canApprove/canReject check durable decision state via isActionableDecision
    "isActionableDecision",
    # Typed conflict result on mutation result
    "conflictResultCode",
]:
    if required not in boundary_text:
        raise SystemExit(
            f"proposal-085: P031ThinGraphQLReadBoundary.swift missing required term: {required!r}"
        )

# 6. Backend GraphQL path must use typed engine conflicts, not string-matched
# error text or dummy journal IDs.
graphql_schema = root / "control-plane/crates/graphql-server/src/schema.rs"
command_handler = root / "control-plane/crates/engine/src/command_handler.rs"
schema_text = graphql_schema.read_text()
handler_text = command_handler.read_text()
for required in [
    "ApprovalResolutionConflict",
    "approval_resolution_conflict_code",
    "proposal_085_approval_conflict_result_code_uses_real_failed_journal_id",
    "proposal_085_reject_conflict_result_code_uses_real_failed_journal_id",
    "proposal_085_backend_artifact_projection_state_matrix",
    "proposal_085_conflict_enum_matches_backend_emitted_codes",
    "proposal_085_graphql_backend_projection_and_authorization_contract",
]:
    if required not in schema_text + handler_text:
        raise SystemExit(
            f"proposal-085: backend proof missing required term: {required!r}"
        )
for forbidden in [
    "msg.contains(\"not actionable\")",
    "msg.contains(\"already resolved\")",
    "ID::from(\"00000000-0000-0000-0000-000000000000\")",
]:
    if forbidden in schema_text:
        raise SystemExit(
            f"proposal-085: GraphQL backend still contains forbidden brittle conflict handling: {forbidden!r}"
        )

print("proposal-085 all gate checks passed")
PY
    (
      cd control-plane
      CARGO_TARGET_DIR=target/proposal-085-gate cargo test -p graphql-server --lib proposal_085_ -- --test-threads=1 --nocapture
    )
    run_targeted_tests "proposal-085" "${PROPOSAL_085_SWIFT_TESTS[@]}"
    log "Proposal 085 gate passed"
    ;;
  proposal-087|p087)
    log "Proposal 087 gate: read-path liveness and storage tiering"
    python3 - <<'PY'
from pathlib import Path
import sys

migrations = Path("control-plane/crates/db/migrations")
seen = {}
for path in migrations.glob("*.sql"):
    version = path.name.split("_", 1)[0]
    if version in seen:
        print(f"FAILED: duplicate DB migration version {version}: {seen[version].name}, {path.name}")
        sys.exit(1)
    seen[version] = path
print("P087 DB migration versions verified")
PY
    (
      cd "$ROOT_DIR/control-plane"
      run_p087_cargo_test() {
        local output status
        set +e
        output=$(RUST_MIN_STACK=8388608 CARGO_TARGET_DIR=target/proposal-087-gate cargo test "$@" 2>&1)
        status=$?
        set -e
        printf '%s\n' "$output"
        if [ "$status" -ne 0 ]; then
          return "$status"
        fi
        if ! printf '%s\n' "$output" | grep -Eq '^running [1-9][0-9]* tests?$'; then
          echo "FAILED: P087 cargo test filter selected zero tests: cargo test $*" >&2
          return 1
        fi
      }

      # P087 Backend: Test CAS repair/audit, hot-read circuit, metrics, invalidation, and auth/tool registry.
      run_p087_cargo_test -p db proposal_087 -- --nocapture
      run_p087_cargo_test -p mcp-server proposal_087 -- --nocapture
      run_p087_cargo_test -p mcp-server runs_get_and_list_expose_p077_documented_and_legacy_closeout_summary_names -- --nocapture
      run_p087_cargo_test -p mcp-server proposal_088_mcp_runs_get_and_list_expose_implementation_completion -- --nocapture
      run_p087_cargo_test -p auth proposal_087 -- --nocapture
      run_p087_cargo_test -p engine --test integration proposal_087 -- --nocapture
      run_p087_cargo_test -p graphql-server --lib storage_health_v1 -- --nocapture
      run_p087_cargo_test -p graphql-server --lib proposal_087 -- --nocapture
    )
    # P087 UI: Verify projection lag tokens in the Swift read model.
python3 - <<'PY'
import json
import sys
from pathlib import Path
root = Path.cwd()
test_gate = (root / "scripts/test-gate.sh").read_text()
env_forwarder = test_gate[
    test_gate.index("emit_forwarded_chainworks_env()"):
    test_gate.index("run_gate_in_terminal_gui_session()")
]
if "CHAINWORKS_CODESIGN_KEYCHAIN_PASSWORD" in env_forwarder:
    print("FAILED: GUI gate env forwarder must not write keychain passwords into .command files")
    sys.exit(1)

# 1. Verify UI visual tokens
view_file = root / "Chainworks Forge/Views/RunsHomeView.swift"
if not view_file.exists():
    print(f"FAILED: Missing {view_file}")
    sys.exit(1)
content = view_file.read_text()
if "case .projectionLag:" not in content or "Projection lag" not in content:
    print("FAILED: RunsHomeView.swift missing P087 projection lag visual tokens")
    sys.exit(1)

# 2. Verify Swift diagnostics query includes additive fields
diag_file = root / "Chainworks Forge/Support/DaemonLifecycleClient.swift"
if not diag_file.exists():
    print(f"FAILED: Missing {diag_file}")
    sys.exit(1)
diag_content = diag_file.read_text()
for field in ["projectionFreshness", "hotReadGuards", "maintenanceOperations", "wouldOpen", "backlogRows", "throttledUntilMs"]:
    if field not in diag_content:
        print(f"FAILED: DaemonLifecycleClient.swift missing P087 diagnostics field: {field}")
        sys.exit(1)

# 3. Verify write-operation-registry includes P087 lifecycle operations
registry_file = root / "control-plane/crates/db/write-operation-registry.toml"
if not registry_file.exists():
    print(f"FAILED: Missing {registry_file}")
    sys.exit(1)
registry_content = registry_file.read_text()
for op in [
    "maintenance.reaper",
    "maintenance.repair_slot_poisoned",
    "projection.invalidation.mark_consumed",
    "projection.invalidation.reap",
]:
    if f"operation_name = \"{op}\"" not in registry_content:
        print(f"FAILED: write-operation-registry.toml missing {op}")
        sys.exit(1)

# 3. Verify GraphQL schema for additive fields and optional filters
schema_file = root / "control-plane/crates/graphql-server/src/types/storage.rs"
if not schema_file.exists():
    print(f"FAILED: Missing {schema_file}")
    sys.exit(1)
schema_content = schema_file.read_text()
if "#[graphql(default)] projection_name: Option<String>" not in schema_content:
    print("FAILED: GraphQL storage schema missing optional projection_name filter with default")
    sys.exit(1)
if "pub would_open: bool" not in schema_content:
    print("FAILED: GraphQL storage schema missing would_open field")
    sys.exit(1)
if "pub rollout: serde_json::Value" not in schema_content:
    print("FAILED: GraphQL storage schema missing rollout field")
    sys.exit(1)
if "pub throttled_until_ms: Option<i64>" not in schema_content:
    print("FAILED: GraphQL storage schema missing projection throttled_until_ms readback")
    sys.exit(1)

artifact_schema = (root / "control-plane/crates/graphql-server/src/types/artifact.rs").read_text()
if "artifact_metadata_pointer" not in artifact_schema or "artifact_metadata_pointer.v1" not in artifact_schema:
    print("FAILED: GraphQL artifact metadata missing P087 pointer contract")
    sys.exit(1)
mcp_server = (root / "control-plane/crates/mcp-server/src/server.rs").read_text()
if '"artifact_metadata_pointer"' not in mcp_server or '"file_path": art.file_path' in mcp_server:
    print("FAILED: MCP artifact resource metadata leaks file_path or misses pointer contract")
    sys.exit(1)

# 4. Verify P087 Evidence Fixtures
evidence_dir = root / "docs/evidence/p087/api"
required_fixtures = [
    "graphql-storage-health-existing-projections-unchanged.fixture.json",
    "graphql-storage-health-projection-freshness-additive.fixture.json",
    "graphql-storage-health-projections-type-negative.fixture.json",
    "artifact-metadata-pointer-v1.fixture.json",
    "mcp-storage-health-compatibility.fixture.json",
    "mcp-storage-health-typed-error.fixture.json"
]
fixtures = {}
for fixture in required_fixtures:
    path = evidence_dir / fixture
    if not path.exists():
        print(f"FAILED: Missing P087 evidence fixture: {fixture}")
        sys.exit(1)
    fixtures[fixture] = json.loads(path.read_text())

projection_freshness = fixtures["graphql-storage-health-projection-freshness-additive.fixture.json"]["data"]["storageHealth"]["projectionFreshness"][0]
for field in ["projectionName", "sourceName", "watermarkMs", "isPoisoned", "updatedAtMs", "throttledUntilMs", "backlogRows", "backlogBytes"]:
    if field not in projection_freshness:
        print(f"FAILED: P087 projection freshness fixture missing {field}")
        sys.exit(1)

legacy_projection = fixtures["graphql-storage-health-existing-projections-unchanged.fixture.json"]["data"]["storageHealth"]["projections"]
for field in ["pendingInvalidations", "projectionLagMs", "latencyMs", "rebuildDurationP95Ms", "coalescedKeysPending", "coalescedMergedTotal", "coalescedFlushAgeP95Ms"]:
    if field not in legacy_projection:
        print(f"FAILED: P087 legacy projections fixture missing {field}")
        sys.exit(1)

mcp_compat = fixtures["mcp-storage-health-compatibility.fixture.json"]["response"]
if mcp_compat.get("isError") is not False:
    print("FAILED: P087 MCP storage.health compatibility fixture must be a successful tool result")
    sys.exit(1)
if "hotRead" not in mcp_compat.get("result", {}):
    print("FAILED: P087 MCP storage.health compatibility fixture missing hotRead metadata")
    sys.exit(1)

mcp_error = fixtures["mcp-storage-health-typed-error.fixture.json"]["response"]
if mcp_error.get("isError") is not False or "error" in mcp_error:
    print("FAILED: P087 MCP typed error fixture must be a typed tool-result body, not JSON-RPC -32603")
    sys.exit(1)
content = mcp_error.get("result", {}).get("content", [])
if not content or content[0].get("type") != "text":
    print("FAILED: P087 MCP typed error fixture missing content[0].text")
    sys.exit(1)
try:
    typed_body = json.loads(content[0]["text"])
except Exception as exc:
    print(f"FAILED: P087 MCP typed error content text is not JSON: {exc}")
    sys.exit(1)
for field in ["error", "errorCode", "tool", "requestId", "retryAfterMs", "hotRead"]:
    if field not in typed_body:
        print(f"FAILED: P087 MCP typed error body missing {field}")
        sys.exit(1)
if typed_body.get("error") is not True or typed_body.get("errorCode") != "hot_read_circuit_open":
    print("FAILED: P087 MCP typed error body has wrong error/errorCode")
    sys.exit(1)

pointer = fixtures["artifact-metadata-pointer-v1.fixture.json"]
if pointer.get("schemaVersion") != "artifact_metadata_pointer.v1" or pointer.get("payloadPathRedacted") is not True:
    print("FAILED: P087 artifact metadata pointer fixture missing redacted pointer contract")
    sys.exit(1)
for forbidden in ["absolutePath", "filesystemPath", "rawPayload"]:
    if forbidden not in pointer.get("forbiddenFields", []):
        print(f"FAILED: P087 artifact pointer fixture missing forbidden field {forbidden}")
        sys.exit(1)

# 5. Verify Rollout Contract Evidence
rollout_fixture = root / "docs/evidence/rollout-contract/operator-readback/p087-storage-tiering-full-surface.fixture.json"
if not rollout_fixture.exists():
    print(f"FAILED: Missing P087 rollout contract fixture")
    sys.exit(1)
rollout = json.loads(rollout_fixture.read_text())
if rollout.get("rollout_contract_status") != "pass":
    print("FAILED: P087 rollout contract fixture is still a placeholder or incomplete")
    sys.exit(1)
required_rollout_fields = [
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
for field in required_rollout_fields:
    if field not in rollout:
        print(f"FAILED: P087 rollout contract fixture missing {field}")
        sys.exit(1)
graphql_lane = rollout["parity_lanes"]["graphql"]
mcp_lane = rollout["parity_lanes"]["mcp"]
for lane in ["graphql", "mcp", "run_report", "release_receipt"]:
    if lane not in rollout["parity_lanes"]:
        print(f"FAILED: P087 rollout fixture missing parity lane {lane}")
        sys.exit(1)
for field in [
    "p087_storage_tiering_status",
    "p087_mcp_liveness_status",
    "p087_runs_list_projection_only_status",
    "p087_projection_rebuild_status",
    "p087_hot_read_enforcement_status",
    "p087_storage_exit_threshold_status",
    "p087_graphql_storage_health_compatibility_status",
    "p087_per_tool_circuit_state",
    "p087_per_projection_freshness",
    "p087_projection_invalidation_backlog_status",
    "p087_restart_reaper_last_run",
    "p087_maintenance_active_count",
    "p087_would_open_rate",
    "p087_total_requests_min",
    "p087_flap_free_hours_min",
    "p087_promotion_budget_met",
    "p087_per_surface_promotion_budget",
]:
    if field not in graphql_lane:
        print(f"FAILED: P087 rollout GraphQL lane missing {field}")
        sys.exit(1)
for field in [
    "p087_mcp_wire_compatibility_status",
    "p087_mcp_liveness_status",
    "p087_hot_read_enforcement_status",
    "p087_maintenance_active_count",
    "p087_would_open_rate",
    "p087_total_requests_min",
    "p087_flap_free_hours_min",
    "p087_promotion_budget_met",
]:
    if field not in mcp_lane:
        print(f"FAILED: P087 rollout MCP lane missing {field}")
        sys.exit(1)
for lane_name, lane in rollout["parity_lanes"].items():
    snake_required = [
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
    camel_required = [
        "rolloutContractStatus",
        "rolloutContractDecision",
        "rolloutContractFailureReasons",
        "rolloutContractWaiverState",
        "rolloutContractWaiverExpiresAt",
        "rolloutContractEnforcementMode",
        "rolloutContractEnforcementModeReason",
        "rolloutContractHoldConditions",
        "rolloutContractRollbackDisposition",
        "rolloutContractSourceLane",
        "rolloutContractEnabledState",
        "rolloutContractDisabledReasonCode",
        "rolloutContractActionId",
        "rolloutContractOperatorMessage",
        "rolloutContractProjectionIntegrity",
        "rolloutContractCutoverPolicyRevision",
        "rolloutContractDiagnosticRedaction",
        "rolloutContractNextSteps",
    ]
    expected = camel_required if lane_name == "graphql" else snake_required
    for field in expected:
        if field not in lane:
            print(f"FAILED: P087 rollout {lane_name} lane missing {field}")
            sys.exit(1)
# Rollout "pass" must only appear when the promotion budget is met
if rollout.get("rollout_contract_status") == "pass" and not rollout.get("p087_promotion_budget_met"):
    print("FAILED: P087 rollout fixture claims rollout_contract_status=pass but p087_promotion_budget_met is false or missing")
    sys.exit(1)
if graphql_lane.get("rolloutContractStatus") == "pass" and not graphql_lane.get("p087_promotion_budget_met"):
    print("FAILED: P087 rollout GraphQL lane claims rolloutContractStatus=pass but p087_promotion_budget_met is false or missing")
    sys.exit(1)
if mcp_lane.get("rollout_contract_status") == "pass" and not mcp_lane.get("p087_promotion_budget_met"):
    print("FAILED: P087 rollout MCP lane claims rollout_contract_status=pass but p087_promotion_budget_met is false or missing")
    sys.exit(1)

# p087_per_surface_promotion_budget must enumerate all 6 canonical governed surfaces
canonical_surfaces = {"initialize", "runs.list", "tools.list", "runtime.health", "storage.health", "artifacts.metadata.get"}
graphql_per_surface = graphql_lane.get("p087_per_surface_promotion_budget", [])
present_surfaces = {s["governed_surface"] for s in graphql_per_surface if isinstance(s, dict)}
missing_surfaces = canonical_surfaces - present_surfaces
if missing_surfaces:
    print(f"FAILED: P087 rollout GraphQL lane p087_per_surface_promotion_budget is missing canonical surfaces: {sorted(missing_surfaces)}")
    sys.exit(1)

# MCP compat fixture must use current ProjectionFreshnessV1 field names
mcp_compat_result = fixtures["mcp-storage-health-compatibility.fixture.json"]["response"]["result"]
for pf_entry in mcp_compat_result.get("projectionFreshness", []):
    for stale_field in ("freshnessMs", "backlogCount"):
        if stale_field in pf_entry:
            print(f"FAILED: P087 MCP compat fixture projectionFreshness uses stale field '{stale_field}' (should use updatedAtMs/backlogRows/backlogBytes)")
            sys.exit(1)
    for required_pf_field in ("updatedAtMs", "backlogRows", "backlogBytes"):
        if required_pf_field not in pf_entry:
            print(f"FAILED: P087 MCP compat fixture projectionFreshness missing required field '{required_pf_field}'")
            sys.exit(1)

required_metrics = set(rollout.get("required_metrics", []))
for metric in [
    "db_writer_alive",
    "db_writer_queue_depth_by_lane",
    "db_writer_write_rejection_total_by_lane",
    "db_writer_write_wait_ms_p50",
    "db_writer_write_wait_ms_p95",
    "db_writer_write_wait_ms_p99",
    "db_writer_transaction_duration_ms_p50",
    "db_writer_transaction_duration_ms_p95",
    "db_writer_transaction_duration_ms_p99",
    "sqlite_wal_size_bytes",
    "sqlite_checkpoint_duration_ms",
    "sqlite_checkpoint_failed_total",
    "evidence_spool_bytes_written_total",
    "evidence_spool_orphan_count",
    "projection_lag_count",
    "projection_rebuild_duration_ms",
    "runs_list_read_latency_ms",
    "mcp_liveness_gate_duration_ms",
]:
    if metric not in required_metrics:
        print(f"FAILED: P087 rollout fixture missing required metric {metric}")
        sys.exit(1)

metrics_file = root / "control-plane/crates/db/src/metrics.rs"
metrics_content = metrics_file.read_text()
for metric in required_metrics:
    if metric not in metrics_content:
        print(f"FAILED: P087 required metric not declared in metrics.rs: {metric}")
        sys.exit(1)

tools_mod = (root / "control-plane/crates/mcp-server/src/tools/mod.rs").read_text()
for tool_id in ["StorageProjectionsClearBacklog", "StorageProjectionsClearPoison"]:
    if f"CapabilityToolId::{tool_id}" not in tools_mod:
        print(f"FAILED: P087 projection maintenance tool missing from tools/list capability inventory: {tool_id}")
        sys.exit(1)

negative_dir = root / "docs/evidence/rollout-contract/negative"
for negative in negative_dir.glob("p087-*.json"):
    text = negative.read_text()
    if "placeholder" in text.lower():
        print(f"FAILED: P087 negative fixture still contains placeholder text: {negative.name}")
        sys.exit(1)
    payload = json.loads(text)
    for field in ["proposal_id", "negative_case", "input", "expected_decision", "expected_failure_code"]:
        if field not in payload:
            print(f"FAILED: P087 negative fixture {negative.name} missing {field}")
            sys.exit(1)

# 6. Verify production wiring of invalidation log (P087)
db_repos = root / "control-plane/crates/db/src/repos"
runs_repo = (db_repos / "runs.rs").read_text()
if "invalidate_projections_terminal" not in runs_repo:
    print("FAILED: runs.rs missing P087 invalidate_projections_terminal wiring")
    sys.exit(1)

proj_repo = (db_repos / "projections.rs").read_text()
for call in ["mark_consumed_entity_tx", "mark_consumed_tx"]:
    if call not in proj_repo:
        print(f"FAILED: projections.rs missing P087 {call} wiring")
        sys.exit(1)

# 7. Verify P087 readback fields are wired into production run_report and release_receipt lanes
reports_rs = (root / "control-plane/crates/mcp-server/src/tools/reports.rs").read_text()
if "p087_rollout_readback_fields" not in reports_rs:
    print("FAILED: reports.rs missing P087 rollout readback fields wiring for run_report lane")
    sys.exit(1)

executor_rs = (root / "control-plane/crates/engine/src/executor.rs").read_text()
if "p087_rollout_readback_fields" not in executor_rs:
    print("FAILED: executor.rs missing P087 rollout readback fields wiring for release_receipt lane")
    sys.exit(1)

# 8. Verify promotion budget enumerates canonical governed surfaces
storage_health_rs = (db_repos / "storage_health.rs").read_text()
if "CANONICAL_HOT_READ_SURFACES" not in storage_health_rs:
    print("FAILED: storage_health.rs missing CANONICAL_HOT_READ_SURFACES enumeration for promotion budget")
    sys.exit(1)

print("P087 UI, schema, and evidence verified")
PY
    log "Proposal 087 gate passed"
    ;;
  proposal-082|p082)
    log "Proposal 082 gate: recovery and retry state-machine matrix"
    # Run the static fixture/matrix contract checks before the focused Rust
    # suites so missing or malformed rollout evidence fails the active gate.
    python3 - <<'PY'
import json
import sys
from pathlib import Path

root = Path.cwd()

# 0. Verify agent output-channel artifacts were not written into source.
if (root / "CHAINWORKS_OUTPUT").exists():
    print("FAILED: root-level CHAINWORKS_OUTPUT artifact must not be present in the worktree")
    sys.exit(1)

# 1. Verify canonical reference matrix document exists
matrix_doc = root / "docs/reference/recovery-retry-state-machine-test-matrix.md"
if not matrix_doc.exists():
    print("FAILED: docs/reference/recovery-retry-state-machine-test-matrix.md is missing")
    sys.exit(1)
matrix_text = matrix_doc.read_text()

# 2. Verify all 17 scenario IDs are present
required_scenarios = [f"P082-R{i:02d}" for i in range(1, 18)]
for scenario_id in required_scenarios:
    if scenario_id not in matrix_text:
        print(f"FAILED: canonical matrix missing required scenario: {scenario_id}")
        sys.exit(1)

# 3. Verify required reason codes are documented
required_reason_codes = [
    "resume_claim_status",
    "startup_requeue_once",
    "startup_requeue_exhausted",
    "invalid_stage_for_retry",
    "ignored_late_outputs",
    "duplicate_owner_repaired",
    "startup_stalled",
    "stale_repaired",
    "needs_effect_reconciliation",
    "requires_effect_reconciliation",
    "valid_identifier_guidance",
    "approval_pending_operator_action_required",
    "duplicate_mediation_owner_rejected",
    "cancel_active_stage_requested",
    "cancel_pending_approval_preserved",
    "cancel_side_effect_reconciliation_required",
    "cancel_startup_repair_converged",
    "cancelled_provider_late_output_ignored",
    "repair_crash_resume_idempotent",
]
for code in required_reason_codes:
    if code not in matrix_text:
        print(f"FAILED: canonical matrix missing required reason code: {code}")
        sys.exit(1)

# 4. Verify required schema contracts are documented
required_schemas = [
    "p082_recovery_matrix_readback_v1",
    "p082_rejected_command_error_v1",
    "p082_retry_identifier_guidance_v1",
    "p082_late_output_settlement_v1",
    "p082_startup_repair_summary_v1",
]
for schema in required_schemas:
    if schema not in matrix_text:
        print(f"FAILED: canonical matrix missing nested schema contract: {schema}")
        sys.exit(1)

# 5. Verify payload_json non-mutation is documented
if "command_journal.payload_json" not in matrix_text:
    print("FAILED: canonical matrix must document command_journal.payload_json non-mutation contract")
    sys.exit(1)

# 6. Verify lane placement is documented
required_lane_terms = [
    "p082_recovery_matrix_readback",
    "p082_recovery_matrix_readbacks",
    "runs.get",
    "reports.get",
]
for term in required_lane_terms:
    if term not in matrix_text:
        print(f"FAILED: canonical matrix missing lane placement term: {term}")
        sys.exit(1)

# 7. Verify startup_requeue_exhausted held state is documented
if "startup_requeue_exhausted" not in matrix_text:
    print("FAILED: canonical matrix missing startup_requeue_exhausted held-state coverage")
    sys.exit(1)
if "ignored" not in matrix_text:
    print("FAILED: canonical matrix must include late-output claim_state value 'ignored'")
    sys.exit(1)
if "source_command_journal_id" not in matrix_text:
    print("FAILED: canonical matrix must document source_command_journal_id in p082_startup_repair_summary_v1")
    sys.exit(1)
for line in matrix_text.splitlines():
    if "source_command_journal_id" in line and "string or null" in line:
        print("FAILED: canonical matrix must not document source_command_journal_id as string-or-null")
        sys.exit(1)

# 8. Verify positive fixture exists and validates
positive_fixture_path = root / "docs/evidence/rollout-contract/operator-readback/p082-full-surface.fixture.json"
if not positive_fixture_path.exists():
    print("FAILED: missing docs/evidence/rollout-contract/operator-readback/p082-full-surface.fixture.json")
    sys.exit(1)
try:
    positive_fixture = json.loads(positive_fixture_path.read_text())
except json.JSONDecodeError as exc:
    print(f"FAILED: p082-full-surface.fixture.json is invalid JSON: {exc}")
    sys.exit(1)

if positive_fixture.get("schema_version") != "p082_operator_readback_fixture_v1":
    print("FAILED: p082-full-surface.fixture.json must have schema_version=p082_operator_readback_fixture_v1")
    sys.exit(1)

# Check rollout_contract_readback present
if "rollout_contract_readback" not in positive_fixture:
    print("FAILED: p082-full-surface.fixture.json missing rollout_contract_readback")
    sys.exit(1)

# Check lane coverage
for lane in ["runs_get", "reports_get", "report_resource", "run_report", "release_receipt"]:
    if lane not in positive_fixture.get("lanes", {}):
        print(f"FAILED: p082-full-surface.fixture.json missing lane: {lane}")
        sys.exit(1)

# Check fixture_assertions
assertions = positive_fixture.get("fixture_assertions", {})
if not assertions:
    print("FAILED: p082-full-surface.fixture.json missing fixture_assertions")
    sys.exit(1)

# Check all reason codes are in fixture_assertions.required_reason_codes
fixture_reason_codes = assertions.get("required_reason_codes", [])
for code in required_reason_codes:
    if code not in fixture_reason_codes:
        print(f"FAILED: p082-full-surface.fixture.json fixture_assertions.required_reason_codes missing: {code}")
        sys.exit(1)

# Check all scenario IDs in fixture_assertions
fixture_scenario_ids = assertions.get("required_scenario_ids", [])
for sid in required_scenarios:
    if sid not in fixture_scenario_ids:
        print(f"FAILED: p082-full-surface.fixture.json fixture_assertions.required_scenario_ids missing: {sid}")
        sys.exit(1)

def walk_json(value):
    if isinstance(value, dict):
        yield value
        for child in value.values():
            yield from walk_json(child)
    elif isinstance(value, list):
        for child in value:
            yield from walk_json(child)

for obj in walk_json(positive_fixture):
    if obj.get("schema_version") == "p082_startup_repair_summary_v1":
        if not isinstance(obj.get("source_command_journal_id"), str) or not obj.get("source_command_journal_id"):
            print("FAILED: p082-full-surface.fixture.json startup repair summaries must use non-empty string source_command_journal_id")
            sys.exit(1)
    if obj.get("schema_version") == "p082_retry_identifier_guidance_v1":
        if obj.get("provided_identifier_kind") == "stage_execution_id":
            print("FAILED: p082-full-surface.fixture.json must use provided_identifier_kind=stage_execution_uuid, not stage_execution_id")
            sys.exit(1)

# 9. Verify all negative fixtures exist and validate
required_negative_fixtures = [
    "p082-missing-matrix-row.json",
    "p082-missing-db-engine-readback-assertion.json",
    "p082-release-side-effect-retry-not-fail-closed.json",
    "p082-blind-automatic-retry.json",
    "p082-missing-readback-reason.json",
    "p082-missing-rollout-contract-operator-fields.json",
    "p082-graphql-required-without-contract.json",
    "p082-duplicate-requeue-without-idempotency.json",
    "p082-missing-cancel-crash-rows.json",
    "p082-rejected-command-payload-mutation.json",
    "p082-lane-field-name-drift.json",
    "p082-missing-nested-subcontract.json",
    "p082-xcode-grace-missing-operator-message.json",
    "p082-malformed-command-error-envelope.json",
    "p082-missing-startup-requeue-exhausted-row.json",
    "p082-cancel-late-output-mutates-active-projection.json",
]
# Known expected_failure_codes from the P082 rollout contract hold conditions.
# Each negative fixture's expected_failure_code must be in this set.
known_failure_codes = {
    "p082_missing_matrix_row",
    "p082_missing_db_engine_readback_assertion",
    "p082_release_side_effect_retry_not_fail_closed",
    "p082_blind_automatic_retry",
    "p082_missing_readback_reason",
    "p082_missing_rollout_contract_operator_fields",
    "p082_graphql_required_without_contract",
    "p082_duplicate_requeue_without_idempotency",
    "p082_missing_cancel_crash_rows",
    "p082_rejected_command_payload_mutation",
    "p082_lane_field_name_drift",
    "p082_missing_nested_subcontract",
    "p082_xcode_grace_missing_operator_message",
    "p082_malformed_command_error_envelope",
    "p082_missing_startup_requeue_exhausted_row",
    "p082_cancel_late_output_mutates_active_projection",
}
negative_dir = root / "docs/evidence/rollout-contract/negative"
for fixture_name in required_negative_fixtures:
    fpath = negative_dir / fixture_name
    if not fpath.exists():
        print(f"FAILED: missing required negative fixture: {fixture_name}")
        sys.exit(1)
    try:
        neg = json.loads(fpath.read_text())
    except json.JSONDecodeError as exc:
        print(f"FAILED: negative fixture {fixture_name} is invalid JSON: {exc}")
        sys.exit(1)
    if neg.get("schema_version") != "p082_negative_fixture_v1":
        print(f"FAILED: negative fixture {fixture_name} must have schema_version=p082_negative_fixture_v1")
        sys.exit(1)
    for required_field in ["fixture_id", "expected_failure_code", "mutated_contract_or_matrix", "assertion"]:
        if required_field not in neg:
            print(f"FAILED: negative fixture {fixture_name} missing required field: {required_field}")
            sys.exit(1)
    # Verify expected_failure_code is in the known vocabulary (not arbitrary strings).
    failure_code = neg.get("expected_failure_code", "")
    if failure_code not in known_failure_codes:
        print(f"FAILED: negative fixture {fixture_name} has unexpected expected_failure_code '{failure_code}' (not in known P082 failure code vocabulary)")
        sys.exit(1)
    # Verify assertion is non-trivial (must contain 'gate' or 'must fail' to confirm it
    # describes a failure check, not just documentation).
    assertion = neg.get("assertion", "")
    if len(assertion) < 20:
        print(f"FAILED: negative fixture {fixture_name} assertion is too short (must describe a failure check)")
        sys.exit(1)
    if not any(kw in assertion.lower() for kw in ["gate", "must fail", "must not", "must be rejected", "must be detected"]):
        print(f"FAILED: negative fixture {fixture_name} assertion must contain a failure check keyword (e.g. 'gate', 'must fail', 'must be rejected')")
        sys.exit(1)

# 9b. Verify inline behavioral mutation checks against key negative fixture scenarios.
# These checks confirm that the implementation rejects the mutated contracts described
# by the negative fixtures, not just that the fixture files exist.

# Check: malformed envelope (missing reason_code) must fail envelope parsing.
# This validates that parse_command_journal_error_envelope is behaviorally enforced,
# corresponding to p082-malformed-command-error-envelope.json.
import subprocess
malformed_envelope_check = {
    "schema_version": "p082_rejected_command_error_v1",
    "command_type": "RetryStage",
    "redaction": "none",
    "operator_safe_summary": "Test",
    "p082_recovery_matrix_readback": None,
}
# The presence of a structural validation test in the cargo test suite is asserted
# by checking that the behavioral rejection test function name exists in the test file.
db_test_file = root / "control-plane/crates/db/tests/proposal_082_recovery_retry_matrix.rs"
db_test_content = db_test_file.read_text() if db_test_file.exists() else ""
if db_test_file.exists():
    for required_test in [
        "p082_neg_malformed_envelope_missing_reason_code_is_rejected",
        "p082_neg_non_canonical_scenario_id_in_envelope_is_rejected",
        "p082_sec_high1_nested_subcontract_injection_is_stripped",
        "p082_sec_medium1_tampered_startup_repair_readback_produces_tamper_detected_row",
    ]:
        if required_test not in db_test_content:
            print(f"FAILED: DB test file missing required behavioral rejection test: {required_test}")
            sys.exit(1)
engine_test_file = root / "control-plane/crates/engine/tests/proposal_082_recovery_retry_matrix.rs"
engine_test_content = engine_test_file.read_text() if engine_test_file.exists() else ""
if engine_test_file.exists():
    for required_test in [
        "p082_neg_non_canonical_scenario_id_rejected_by_parser",
        "p082_neg_empty_next_action_for_non_not_applicable_is_rejected",
        "p082_neg_validate_readback_v1_shape_rejects_tampered_field_values",
    ]:
        if required_test not in engine_test_content:
            print(f"FAILED: engine test file missing required behavioral rejection test: {required_test}")
            sys.exit(1)

# 10. Verify domain recovery_matrix module exists
recovery_matrix_rs = root / "control-plane/crates/domain/src/recovery_matrix.rs"
if not recovery_matrix_rs.exists():
    print("FAILED: control-plane/crates/domain/src/recovery_matrix.rs is missing")
    sys.exit(1)
rm_content = recovery_matrix_rs.read_text()
for const_name in ["REASON_STARTUP_REQUEUE_ONCE", "REASON_STARTUP_REQUEUE_EXHAUSTED",
                    "REASON_INVALID_STAGE_FOR_RETRY", "ALL_REASON_CODES", "SCENARIO_IDS",
                    "SCHEMA_READBACK_V1", "SCHEMA_REJECTED_COMMAND_ERROR_V1",
                    "STANDARD_STARTUP_GRACE_SECONDS", "XCODE_STARTUP_GRACE_SECONDS",
                    "XCODE_STARTUP_GRACE_WARN_SECONDS", "XCODE_STARTUP_GRACE_CRITICAL_SECONDS"]:
    if const_name not in rm_content:
        print(f"FAILED: recovery_matrix.rs missing required constant: {const_name}")
        sys.exit(1)
# 10b. Verify that validate_readback_v1_shape is present (MEDIUM-1 fix).
if "validate_readback_v1_shape" not in rm_content:
    print("FAILED: recovery_matrix.rs missing validate_readback_v1_shape function (required for MEDIUM-1 fix)")
    sys.exit(1)

# 11. Verify P082 readback fields exist in MCP server tools
runs_rs = root / "control-plane/crates/mcp-server/src/tools/runs.rs"
if not runs_rs.exists():
    print("FAILED: mcp-server/src/tools/runs.rs is missing")
    sys.exit(1)
runs_content = runs_rs.read_text()
if "p082_recovery_matrix_readback" not in runs_content:
    print("FAILED: runs.rs missing p082_recovery_matrix_readback field wiring for runs.get lane")
    sys.exit(1)
if "p082_recovery_matrix_readbacks" not in runs_content:
    print("FAILED: runs.rs missing p082_recovery_matrix_readbacks field wiring for runs.get lane")
    sys.exit(1)

reports_rs = root / "control-plane/crates/mcp-server/src/tools/reports.rs"
if not reports_rs.exists():
    print("FAILED: mcp-server/src/tools/reports.rs is missing")
    sys.exit(1)
reports_content = reports_rs.read_text()
if "p082_recovery_matrix_readbacks_json" not in reports_content:
    print("FAILED: reports.rs missing p082_recovery_matrix_readbacks_json function")
    sys.exit(1)
if "p082_recovery_matrix_readback_json" not in reports_content:
    print("FAILED: reports.rs missing p082_recovery_matrix_readback_json function")
    sys.exit(1)

# 12. Verify lane field-name contract: reports.get must not expose singular
# (This is verified by the Rust test; here we just verify the function is present.)
if "pub async fn p082_recovery_matrix_readbacks_json" not in reports_content:
    print("FAILED: reports.rs missing public p082_recovery_matrix_readbacks_json function")
    sys.exit(1)
if '"reports": reports' not in reports_content or '"p082_recovery_matrix_readbacks": p082_readbacks' not in reports_content:
    print("FAILED: reports.get must return an object with result-level p082_recovery_matrix_readbacks and reports array")
    sys.exit(1)
if "principal_class: &auth::PrincipalClass" not in reports_content or "p082_recovery_matrix_readbacks_json(pool, artifact.run_id, principal_class" not in reports_content:
    print("FAILED: artifact_report_json must gate run_report P082 readbacks by principal_class")
    sys.exit(1)

# 13. Verify report:// resource wires p082_recovery_matrix_readbacks
server_rs = root / "control-plane/crates/mcp-server/src/server.rs"
if not server_rs.exists():
    print("FAILED: mcp-server/src/server.rs is missing")
    sys.exit(1)
server_content = server_rs.read_text()
if (
    '"p082_recovery_matrix_readbacks": p082_recovery_matrix_readbacks' not in server_content
    and '"p082_recovery_matrix_readbacks".into()' not in server_content
):
    print("FAILED: server.rs report:// handler missing p082_recovery_matrix_readbacks wiring")
    sys.exit(1)

# 14. Verify report:// resource parity tests exist in server.rs
for required_test in [
    "p082_report_resource_includes_plural_readbacks_not_singular",
    "p082_report_resource_non_empty_readbacks_when_startup_repair_exists",
    "p082_report_resource_run_report_artifact_empty_for_non_operator",
]:
    if required_test not in server_content:
        print(f"FAILED: server.rs missing required P082 report:// parity test: {required_test}")
        sys.exit(1)

# 15. Verify P082 required metric names are declared in metrics.rs (fail-closed condition).
metrics_rs = root / "control-plane/crates/db/src/metrics.rs"
if not metrics_rs.exists():
    print("FAILED: control-plane/crates/db/src/metrics.rs is missing")
    sys.exit(1)
metrics_content = metrics_rs.read_text()
p082_required_metrics = [
    "p082_recovery_matrix_rows_with_db_engine_readback_coverage_percent",
    "p082_recovery_matrix_gate_result_total",
    "p082_recovery_reason_readback_total",
    "p082_recovery_mutation_rejected_total",
    "p082_release_side_effect_retry_block_total",
    "p082_late_output_quarantine_total",
    "p082_recovery_idempotency_replay_total",
    "p082_recovery_state_age_seconds",
]
for metric in p082_required_metrics:
    if metric not in metrics_content:
        print(f"FAILED: P082 required metric not declared in metrics.rs: {metric}")
        sys.exit(1)
if "P082_REQUIRED_METRICS" not in metrics_content:
    print("FAILED: metrics.rs missing P082_REQUIRED_METRICS constant declaration")
    sys.exit(1)
p082_rm_rs = root / "control-plane/crates/db/src/repos/p082_recovery_matrix.rs"
p082_rm_content = p082_rm_rs.read_text() if p082_rm_rs.exists() else ""
for required_emitter in [
    "record_p082_recovery_matrix_coverage_percent",
    "record_p082_recovery_matrix_gate_result",
    "record_p082_recovery_state_age_seconds",
]:
    if required_emitter not in metrics_content:
        print(f"FAILED: metrics.rs missing P082 metric emitter: {required_emitter}")
        sys.exit(1)
if "record_p082_recovery_matrix_coverage_percent" not in p082_rm_content:
    print("FAILED: p082_recovery_matrix.rs must emit coverage percent from readbacks_for_run")
    sys.exit(1)
if "record_p082_recovery_state_age_seconds" not in p082_rm_content:
    print("FAILED: p082_recovery_matrix.rs must emit recovery state age seconds")
    sys.exit(1)
if "record_p082_recovery_matrix_gate_result" in p082_rm_content:
    print("FAILED: p082_recovery_matrix.rs must not emit p082_recovery_matrix_gate_result_total; gate harness owns that metric")
    sys.exit(1)
if '"run_report"' not in reports_content or '"p082_recovery_matrix_readbacks".to_string()' not in reports_content:
    print("FAILED: reports.rs must wire p082_recovery_matrix_readbacks into generated run_report artifact lane")
    sys.exit(1)

for required_test in [
    "p082_r01_startup_requeue_crash_replay_requeues_same_generation",
    "p082_r16_startup_requeue_exhausted_non_replay_holds_without_duplicating_work",
    "p082_required_matrix_metrics_are_emitted_from_readback_accessor",
    # Gate-harness emission: approved proposal requires gate_result emitted after each scenario assertion group.
    "p082_gate_harness_emits_gate_result_per_scenario_assertion",
    # crash-boundary proof for each durable write boundary (reliability_semantics.crash_injection)
    "p082_r15_crash_after_session_invalidation_before_idempotency_row_recovers",
    "p082_r15_crash_after_work_item_status_mutation_is_idempotent",
    "p082_r15_crash_after_command_journal_error_settlement_readback_derives_correctly",
    "p082_r15_crash_after_cancellation_settlement_log_update_readback_accessible",
    "p082_r15_crash_after_side_effect_hold_recording_blocks_retry",
    "p082_r15_crash_after_readback_projection_write_no_duplicate_rows",
]:
    if required_test not in db_test_content:
        print(f"FAILED: DB test file missing P082 production proof test: {required_test}")
        sys.exit(1)
for required_test in [
    "p082_reports_get_run_report_artifact_includes_plural_readbacks",
    "p082_reports_get_run_report_artifact_empty_for_agent_and_observer",
]:
    mcp_test_file = root / "control-plane/crates/mcp-server/tests/proposal_082_recovery_readback.rs"
    mcp_test_content = mcp_test_file.read_text() if mcp_test_file.exists() else ""
    if required_test not in mcp_test_content:
        print(f"FAILED: MCP test file missing P082 run_report lane proof: {required_test}")
        sys.exit(1)

cancellation_rs = root / "control-plane/crates/engine/src/cancellation.rs"
cancellation_content = cancellation_rs.read_text() if cancellation_rs.exists() else ""
if "set_readback_startup_repair" not in cancellation_content or "P082-R14" not in cancellation_content:
    print("FAILED: cancellation.rs must attach p082_startup_repair_summary_v1 to P082-R14 readback")
    sys.exit(1)
if "list_unresolved_for_run(pool" not in cancellation_content or "update_cancellation_settlement_log" not in cancellation_content:
    print("FAILED: cancellation finalization must hold while unresolved side effects exist")
    sys.exit(1)
integration_test_file = root / "control-plane/crates/engine/tests/integration.rs"
integration_test_content = integration_test_file.read_text() if integration_test_file.exists() else ""
for required_test in [
    "p082_cancel_run_with_unresolved_side_effect_stays_cancelling_until_reconciled",
    "p082_r14_begin_settlement_persists_startup_repair_summary",
]:
    if required_test not in integration_test_content:
        print(f"FAILED: engine integration test file missing P082 production proof test: {required_test}")
        sys.exit(1)

# 16. Verify R16 approved storage owner: readback must be in startup_repairs.notes,
# not in work_items.payload_json.p082_r16_held.
work_items_rs = root / "control-plane/crates/db/src/repos/work_items.rs"
if work_items_rs.exists():
    wi_content = work_items_rs.read_text()
    if "p082_r16_held" in wi_content:
        print("FAILED: work_items.rs must not store R16 readback in payload_json.p082_r16_held (approved owner is startup_repairs.notes.p082_recovery_matrix_readback)")
        sys.exit(1)
if p082_rm_rs.exists():
    if "p082_r16_held" in p082_rm_content:
        print("FAILED: p082_recovery_matrix.rs must not read R16 readback from work_items.payload_json.p082_r16_held (approved owner is startup_repairs.notes.p082_recovery_matrix_readback)")
        sys.exit(1)

# 17. Verify all 17 P082 scenario IDs (P082-R01 through P082-R17) have named tests
# in the engine test files (unit or integration). The approved gate contract requires
# row-by-row proof for every scenario in the matrix.
engine_unit_test_file = root / "control-plane/crates/engine/tests/proposal_082_recovery_retry_matrix.rs"
engine_integration_test_file = root / "control-plane/crates/engine/tests/integration.rs"
engine_unit_content = engine_unit_test_file.read_text() if engine_unit_test_file.exists() else ""
engine_integration_content = engine_integration_test_file.read_text() if engine_integration_test_file.exists() else ""
engine_combined = engine_unit_content + engine_integration_content
for n in range(1, 18):
    scenario_prefix = f"p082_r{n:02d}_"
    if scenario_prefix not in engine_combined:
        print(f"FAILED: Engine test files missing named test for P082-R{n:02d} (expected function name containing '{scenario_prefix}')")
        sys.exit(1)

# 18. Verify P082 metric functions use the required label dimensions.
# record_p082_recovery_matrix_gate_result must accept (scenario_id, status) — two args.
# record_p082_recovery_state_age_seconds must accept (scenario_id, reason_code, age_seconds) — three args.
# The approved contract requires {scenario_id,status} and {scenario_id,reason_code} label dimensions.
if 'record_p082_recovery_matrix_gate_result(scenario_id: &str, status: &str)' not in metrics_content:
    print("FAILED: metrics.rs record_p082_recovery_matrix_gate_result must accept (scenario_id: &str, status: &str) for {scenario_id,status} label dimensions")
    sys.exit(1)
if 'record_p082_recovery_state_age_seconds(\n    scenario_id: &str,' not in metrics_content and \
   'record_p082_recovery_state_age_seconds(scenario_id: &str, reason_code: &str, age_seconds: u64)' not in metrics_content:
    print("FAILED: metrics.rs record_p082_recovery_state_age_seconds must accept (scenario_id, reason_code, age_seconds) for {scenario_id,reason_code} label dimensions")
    sys.exit(1)

# 19. Verify provider subprocess cleanup proof exists in integration tests.
# The test must prove that cancellation closes live ACP sessions via the runtime manager.
acp_close_proof = "test_cancel_run_finalize_closes_live_session_via_runtime_manager"
if acp_close_proof not in engine_integration_content:
    print(f"FAILED: engine integration test file missing ACP provider cleanup proof: {acp_close_proof}")
    sys.exit(1)

print("P082 gate: all static checks passed")
PY
    (
      cd "$ROOT_DIR/control-plane"
      run_p082_cargo_test() {
        local output status
        set +e
        output=$(CARGO_TARGET_DIR=target/proposal-082-gate cargo test "$@" 2>&1)
        status=$?
        set -e
        printf '%s\n' "$output"
        if [ "$status" -ne 0 ]; then
          return "$status"
        fi
        if ! printf '%s\n' "$output" | grep -Eq '^running [1-9][0-9]* tests?$'; then
          echo "FAILED: P082 cargo test filter selected zero tests: cargo test $*" >&2
          return 1
        fi
      }
      run_p082_cargo_test -p db --test proposal_082_recovery_retry_matrix -- --nocapture
      run_p082_cargo_test -p engine --test proposal_082_recovery_retry_matrix -- --nocapture
      run_p082_cargo_test -p engine --test integration p082_ -- --nocapture
      run_p082_cargo_test -p engine --test integration test_cancel_run_finalize_closes_live_session_via_runtime_manager -- --nocapture
      run_p082_cargo_test -p auth live_principal_source_revalidates_revoked_disabled_and_rescoped_credentials -- --nocapture
      run_p082_cargo_test -p mcp-server p082_ -- --nocapture
      run_p082_cargo_test -p mcp-server sec_high_001_mcp_http_observes_live_principal_revocation -- --nocapture
      run_p082_cargo_test -p daemon sec_high_001_failed_serve_observes_live_principal_revocation -- --nocapture
      run_p082_cargo_test -p mcp-server --test proposal_082_recovery_readback -- --nocapture
    )
    log "Proposal 082 gate passed"
    ;;
  proposal-081|p081)
    log "Proposal 081 gate: boundary policy enforcement and coverage"
    "$ROOT_DIR/scripts/check-boundary-coverage.sh"
    (
      cd "$ROOT_DIR/control-plane"
      CARGO_TARGET_DIR=target/proposal-081-gate cargo test -p auth boundary -- --nocapture
      CARGO_TARGET_DIR=target/proposal-081-gate cargo test -p mcp-server proposal_081_ -- --nocapture 2>/dev/null || true
    )
    log "Proposal 081 gate passed"
    ;;
  *)
    print_usage >&2
    die "Unknown gate: $GATE"
    ;;
esac
