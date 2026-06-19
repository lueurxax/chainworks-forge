{
  "schema_version": "proposal_document_v1",
  "proposal_id": "P083",
  "proposal_revision_id": "P083-r68-refined-r66-score-lift",
  "title": "Execution-Truth Ownership and Invariant Model",
  "date": "2026-06-02",
  "status": "Revise-Required pending current review. This revision is rebased onto score_lift_backlog review_pass_id proposal-review-P083-r66-3dcd1e20-16b7-477e-8588-08ce9c25ac1f and addresses every current R66 score-lift item inline. It does not claim Ready; implementation may start only after a later review marks this exact revision Ready.",
  "author": "Codex",
  "source_idea": "Implement Proposal 083: Execution-Truth Ownership and Invariant Model.",
  "canonical_proposal_path": "docs/proposals/083-execution-truth-ownership-invariant-model.md",
  "source_review_pass_id": "proposal-review-P083-r66-3dcd1e20-16b7-477e-8588-08ce9c25ac1f",
  "review_basis": {
    "authoritative_backlog_review_pass_id": "proposal-review-P083-r66-3dcd1e20-16b7-477e-8588-08ce9c25ac1f",
    "authoritative_backlog_item_ids": [
      "SCORE-LIFT-API-P083-R66-001",
      "SCORE-LIFT-API-P083-R66-002",
      "SCORE-LIFT-REL-P083-R66-001",
      "SCORE-LIFT-UI-P083-R66-001",
      "SCORE-LIFT-API-P083-R66-NB-001",
      "SCORE-LIFT-REL-P083-R66-NB-001",
      "SCORE-LIFT-REL-P083-R66-NB-002",
      "SCORE-LIFT-APPLE-P083-R66-NB-001",
      "SCORE-LIFT-MACOS-P083-R66-NB-001"
    ],
    "stale_material_policy": "Active feedback mappings, revision summaries, and readiness claims cite only the current R66 score_lift_backlog item ids. Older backlog ids and closure narratives are not used as active authority.",
    "current_review_basis_summary": "The R66 backlog requires rollback_disposition schema-version parity, complete migration surface enumeration or a narrowed migration claim, executable command idempotency and lease recovery, a concrete manual_process_identity_check UI state, bounded metric label domains, late-output overflow latch behavior, shutdown signal generation replay semantics, representative SwiftData transition fixtures, and native command validation/accessibility behavior."
  },
  "active_readiness_narrative": {
    "active_backlog_item_count": 9,
    "blocking_backlog_item_count": 4,
    "advisory_backlog_item_count": 5,
    "proposal_text_items_addressed": 9,
    "unresolved_proposal_text_blocker_count": 0,
    "deferred_blocker_count": 0,
    "disputed_blocker_count": 0,
    "implementation_may_start": false,
    "implementation_may_start_after": "A subsequent review pass verifies this exact revision and returns Ready.",
    "single_authority_pointer": "reviewer_feedback_resolution maps every current R66 score_lift_backlog item to active contract sections."
  },
  "executive_summary": "P083 names durable storage as the execution-truth authority for runs, stages, agents, approvals, artifacts, side effects, provider sessions, command idempotency, shutdown receipts, rollout state, and operator readback. This revision closes the R66 gaps by making rollback_disposition_v1 byte-consistent across rollout, GraphQL, MCP, run_report, and release_receipt; enumerating the full additive migration surface; adding an executable command_idempotency_contract_v1; defining the manual_process_identity_check UI state; bounding every operational metric label; specifying late-output overflow latches; clarifying shutdown signal generation replay; requiring representative SwiftData transition evidence; and making native command validation/accessibility behavior explicit.",
  "problem": [
    "Execution truth currently crosses GraphQL, MCP, SQLite rows, frozen workflow snapshots, stage and agent attempts, provider sessions, approvals, artifacts, side-effect receipts, reports, and SwiftUI projections.",
    "Without an ownership model, caller payloads, projections, provider transcripts, filesystem scans, or UI caches can be mistaken for durable truth.",
    "Retry, cancel, shutdown, rollback, and enforcement cutover need idempotency and receipt constraints that survive crashes and SQLite uniqueness rules.",
    "macOS lifecycle handling must distinguish graceful AppKit callbacks from abrupt process termination where no delegate callback is guaranteed.",
    "Proposal approval has been blocked by stale security and observability rollout artifacts that reviewed older proposal revisions rather than the current one."
  ],
  "goals": [
    "Define one authoritative durable record for every execution-truth identifier.",
    "Classify caller-supplied identifiers as authority, selector, diagnostic, service_owned, or forbidden.",
    "Require lifecycle mutations to carry CallerRequestId and execute through durable idempotency rows.",
    "Publish executable contracts for GraphQL, MCP JSON Schema, SQLite migrations, artifact lineage, metrics, recovery readback, shutdown, late output, and Swift projection mapping.",
    "Keep the macOS app read-only for P083 lifecycle enforcement while providing accurate readback and safe copy/export affordances.",
    "Provide a strict inline rollout_contract_v1 with gate aliases, migration posture, metrics, readback lanes, hold conditions, rollback disposition, and negative fixtures.",
    "Require current-revision security and observability rollout reviews before Ready can be claimed.",
    "Specify executable command idempotency and lease recovery for every lifecycle command that uses CallerRequestId.",
    "Define operator-visible manual recovery UX for identity-ambiguous provider cancellation holds."
  ],
  "non_goals": [
    "Do not add authentication, RBAC, token rotation, credential prompts, or Keychain behavior beyond checking existing principal-class helpers.",
    "Do not change workflow YAML or agent catalog YAML semantics or require new YAML keys.",
    "Do not remove historical artifacts, transcripts, or failed-attempt evidence.",
    "Do not make SwiftUI, GraphQL payloads, MCP payloads, provider transcripts, reports, or filesystem scans authoritative for execution truth.",
    "Do not add a native macOS write path for side_effects.force_reconcile in P083.",
    "Do not introduce destructive migrations or backfill that rewrites historical run evidence.",
    "Do not claim stale R53 or R55 review artifacts as approval evidence for this revision."
  ],
  "target_users_and_trigger": {
    "primary_user": "Chainworks Forge operator running long-lived agent workflows from the macOS app.",
    "implementation_user": "Engine, API, persistence, projection, and UI engineers changing lifecycle state or readback.",
    "trigger": "Repeated review churn around provenance drift, stale identifiers, duplicate commands, inactive approvals, external side effects, provider shutdown, rollout enforcement, and stale reviewer artifacts."
  },
  "ux_ui_notes": {
    "truth_readback": "SwiftUI renders backend readback as read-only truth. Mutation affordances are disabled unless backend actionability is true and projection_integrity is fresh.",
    "typed_denials": "Typed denials render inline beside the affected run, stage, approval, artifact, side-effect, or provider-session row. Unknown denial codes render a generic validation message and no optimistic action.",
    "historical_evidence": "Active artifacts appear first. Historical Evidence is collapsed by default and labels rows Superseded, Failed, Cancelled, or Quarantined without active-transition controls.",
    "shutdown_readback": "Graceful AppKit shutdown progress and abrupt restart recovery are shown as different states. Abrupt termination never claims that applicationShouldTerminate ran; it shows the durable intent or side-effect row that recovery used.",
    "identity_ambiguous_hold": "Provider cancellation rows with intent_state=held and process_fate=identity_ambiguous show operator_next_step_code=manual_process_identity_check and no automatic retry spinner.",
    "copy_export_controls": "Copy controls use NSPasteboard.ContentsOptions.currentHostOnly and never include secrets. Export Text writes only through NSSavePanel unless the operator separately chooses a Copy Export Text action.",
    "manual_process_identity_check": "A ManualProcessIdentityCheckBanner appears in the provider-session detail and any run/stage surface containing the held provider. It shows title 'Process identity needs review', body copy explaining that Forge could not prove the provider process is still the same process, disabled automatic retry with no spinner, and actions: Copy Diagnostic, Mark Process Absent, Retry Identity Check, and Open Provider Session Evidence. VoiceOver reads the title, provider name, reason, and focused action; disabled lifecycle commands remain visible with adjacent reason text."
  },
  "ownership_model": {
    "rule": "Every lifecycle identifier has exactly one authoritative durable record. Callers may provide authority or selector ids only where the ownership matrix permits them; service-owned identifiers are never accepted from caller payload as truth.",
    "data_authority_rule": "SQLite rows are authoritative. GraphQL, MCP, filesystem artifacts, report JSON, and SwiftData projections are readback or evidence surfaces only.",
    "transaction_rule": "For mutating lifecycle commands, request acquisition, authoritative row reload, lifecycle compare-and-set, side-effect receipt write, and terminal command outcome commit happen in one SQLite transaction unless the contract explicitly defines an earlier denial path.",
    "ownership_matrix": [
      {
        "identifier": "run_id",
        "authoritative_record": "runs.id",
        "caller_classification": "authority"
      },
      {
        "identifier": "stage_execution_id",
        "authoritative_record": "stage_executions.id",
        "caller_classification": "service_owned"
      },
      {
        "identifier": "agent_execution_id",
        "authoritative_record": "agent_executions.id",
        "caller_classification": "service_owned"
      },
      {
        "identifier": "provider_session_id",
        "authoritative_record": "provider_sessions.provider_session_id",
        "caller_classification": "service_owned"
      },
      {
        "identifier": "request_id",
        "authoritative_record": "command_idempotency and command_request_aliases",
        "caller_classification": "authority"
      },
      {
        "identifier": "approval_id",
        "authoritative_record": "approvals.id",
        "caller_classification": "selector"
      },
      {
        "identifier": "artifact_id",
        "authoritative_record": "artifact_lineage.artifact_id",
        "caller_classification": "selector"
      },
      {
        "identifier": "side_effect_id",
        "authoritative_record": "side_effects.id",
        "caller_classification": "selector"
      }
    ]
  },
  "architecture": {
    "rust_control_plane_modules_touched": [
      "control-plane/crates/domain: nominal ids, denial codes, ProjectionIntegrity compatibility structs, lifecycle vocabulary enums",
      "control-plane/crates/db: additive migrations for artifact_lineage.report_kind, command idempotency generations, shutdown receipts, queue_rank, overflow latch rows, enforcement mode state, rollback audit rows, provider cancellation intents",
      "control-plane/crates/engine: idempotent command execution, recovery readback, shutdown state machine, late-output caps, abrupt termination recovery, enforcement preflight",
      "control-plane/crates/graphql-server: versioned projection-integrity fields, cutover and rollback mutations, readback fields, RollbackDispositionJSON output validation",
      "control-plane/crates/mcp-server: matching MCP schemas/tools, rollout readback, rollback tool, bounded metrics",
      "control-plane/crates/workflow: RunPlan compatibility validation including xhigh effort values",
      "control-plane/crates/engine command idempotency: command_idempotency rows, request aliases, TTL recovery, committed replay, conflict denial, and pending lease reacquire"
    ],
    "swift_modules_touched": [
      "Chainworks Forge/AppLifecycle: app-owned lifecycle window coordinator and graceful applicationShouldTerminate handling",
      "Chainworks Forge/Projection: RunProjectionSnapshotStore and field mapping manifest validation",
      "Chainworks Forge/CopyControls: CopyButtonRepresentable and current-host-only pasteboard writer",
      "Chainworks Forge/RequestIds: distinct LifecycleRequestId and CopyableCommandRequestId nominal types",
      "Chainworks Forge/SwiftDataBoundary: projection-only/app-local containers and leakage guardrails",
      "Chainworks Forge/ProviderRecoveryUI: ManualProcessIdentityCheckBanner, focused command validation, and VoiceOver-readable denial states"
    ],
    "migration_rule": "All migrations are additive. Rollback disables enforcement or returns to permissive mode; it does not drop columns, delete evidence, or rewrite historical rows."
  },
  "api_contracts": {
    "caller_request_id_v1": {
      "json_type": "string",
      "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
      "rejected_forms": [
        "uppercase_uuid",
        "surrounding_whitespace",
        "urn_prefix",
        "braced_uuid"
      ],
      "additional_properties_policy": "Containers that hold request_id set additionalProperties:false."
    },
    "rollback_disposition_json_v1": {
      "addresses": [
        "SCORE-LIFT-API-P083-R66-001"
      ],
      "graphql_symbol": "RollbackDispositionJSON",
      "schema_version": "rollback_disposition_v1",
      "output_validation_rule": "Rollout readback resolvers validate every rollback_disposition output value against rollback_disposition_v1 before GraphQL serialization. The GraphQL scalar parser performs the same validation only when the scalar is used in a future input position; parser validation is not the enforcement point for current output-only fields.",
      "mcp_schema_rule": "MCP rollout_contract_rollback_disposition is a Draft 2020-12 object with additionalProperties=false and required schema_version, mode, data_loss_risk, and steps fields. GraphQL and MCP carry byte-equal JSON values.",
      "negative_fixtures": [
        "docs/evidence/083/api/rollback-disposition-missing-schema-version-rejected.fixture.json",
        "docs/evidence/083/api/rollback-disposition-output-invalid-rejected-before-graphql.fixture.json",
        "docs/evidence/083/api/rollback-disposition-mcp-extra-property-rejected.fixture.json"
      ],
      "required_fields": [
        "schema_version",
        "mode",
        "data_loss_risk",
        "steps"
      ],
      "rollout_contract_rule": "rollout_contract_v1.rollback_disposition stays strict-template-compatible and does not include schema_version because docs/reference/executable-rollout-gate-template.md permits only mode, data_loss_risk, and optional steps inside the inline rollback_disposition object. The generated RollbackDispositionJSON readback value for run_report, MCP, release_receipt, and GraphQL wraps the inline disposition with schema_version:'rollback_disposition_v1'. Missing schema_version fails readback fixture validation, not inline rollout_contract_v1 lint."
    }
  },
  "shutdown_contract_v1": {
    "addresses": [],
    "schema_version": "shutdown_contract_v1",
    "termination_classification": {
      "graceful_appkit": [
        "normal_quit",
        "logout_or_system_shutdown_when_applicationShouldTerminate_is_invoked"
      ],
      "abrupt_external": [
        "force_quit",
        "sigkill",
        "process_crash",
        "host_power_loss"
      ],
      "rule": "terminateLater plus host_total_ms is a guarantee only for graceful_appkit paths where NSApplicationDelegate.applicationShouldTerminate is invoked. Abrupt_external paths cannot assume delegate callbacks, SwiftUI teardown, or post-signal receipt flush from the terminating process."
    },
    "durable_intent_before_side_effect_rule": "Every provider shutdown side effect must have a durable planned row before the OS signal or provider cancellation side effect is attempted. Restart recovery may rely on shutdown_signal_side_effects or provider_cancellation_intents, but never on an assumption that applicationShouldTerminate ran.",
    "receipt_table": {
      "name": "shutdown_interrupted_receipts",
      "columns": [
        "receipt_id TEXT PRIMARY KEY",
        "provider_session_id TEXT NOT NULL",
        "shutdown_epoch INTEGER NOT NULL",
        "receipt_generation INTEGER NOT NULL",
        "interrupted_state TEXT NOT NULL",
        "queue_rank INTEGER NULL",
        "created_at TEXT NOT NULL",
        "recovered_at TEXT"
      ],
      "constraints": [
        "UNIQUE(provider_session_id, shutdown_epoch, receipt_generation)",
        "CHECK(interrupted_state IN ('grace_deadline_expired','kill_signal_issued','kill_pid_exit_observed','queued_no_signal','shutdown_interrupted'))",
        "CHECK((interrupted_state = 'queued_no_signal' AND queue_rank IS NOT NULL) OR (interrupted_state <> 'queued_no_signal' AND queue_rank IS NULL))"
      ],
      "stored_queue_rank_rule": "queue_rank is the only stored queued shutdown ordering column. It is nullable and non-null exactly when interrupted_state equals queued_no_signal.",
      "final_readback_rank_rule": "final_readback_rank is not a stored column in P083. If a legacy readback helper keeps that name during implementation, it is a derived presentation value equal to queue_rank for queued_no_signal rows and null otherwise."
    },
    "recovery_rules": {
      "graceful_appkit": "applicationShouldTerminate returns terminateLater, waits up to host_total_ms for the bounded daemon wave plus a fixed 1000ms tail for queued_no_signal receipt flush, then replies to AppKit. This path is covered for normal Quit and logout/system shutdown only when AppKit invokes the delegate callback.",
      "force_quit_sigkill_crash": "On restart, recovery reloads durable shutdown_signal_side_effects and provider_cancellation_intents. planned without issued_at is retried only if process identity still matches and deadline policy permits. issued without observed_exit_at is settled by process absence, continued observation, or identity_mismatch hold. No fixture may assert that Force Quit or SIGKILL honored host_total_ms in-process.",
      "queued_no_signal": "queued_no_signal opens a later shutdown_epoch only after process identity is rechecked. If process is absent, recovery writes a later shutdown_interrupted receipt. If identity still matches, recovery opens retry epoch N+1 and writes signal side-effect rows before issuing a signal. If identity is ambiguous, recovery holds with manual_process_identity_check."
    },
    "fixtures": [
      "docs/evidence/083/shutdown/graceful-appkit-terminate-later-host-budget.fixture.json",
      "docs/evidence/083/shutdown/logout-system-shutdown-delegate-invoked.fixture.json",
      "docs/evidence/083/shutdown/force-quit-no-delegate-assumption-restart-recovers-from-signal-side-effects.fixture.json",
      "docs/evidence/083/shutdown/sigkill-no-delegate-assumption-restart-recovers-from-provider-cancellation-intents.fixture.json",
      "docs/evidence/083/shutdown/crash-after-planned-before-signal-retries-with-matching-identity.fixture.json",
      "docs/evidence/083/shutdown/crash-after-signal-before-receipt-settles-from-side-effect-row.fixture.json",
      "docs/evidence/083/shutdown/queue-rank-null-except-queued-no-signal.fixture.json",
      "docs/evidence/083/shutdown/queue-rank-restart-preserved.fixture.json"
    ]
  },
  "shutdown_signal_side_effect_contract_v1": {
    "schema_version": "shutdown_signal_side_effect_contract_v1",
    "table": "shutdown_signal_side_effects",
    "columns": [
      "signal_effect_id TEXT PRIMARY KEY",
      "provider_session_id TEXT NOT NULL",
      "shutdown_epoch INTEGER NOT NULL",
      "process_id INTEGER NOT NULL",
      "process_start_identity TEXT NOT NULL",
      "signal_kind TEXT NOT NULL CHECK(signal_kind IN ('graceful','kill'))",
      "generation INTEGER NOT NULL",
      "intent_state TEXT NOT NULL CHECK(intent_state IN ('planned','issued','observed','suppressed','identity_mismatch'))",
      "issued_at_monotonic_ms INTEGER",
      "issued_at_wall_clock TEXT",
      "observed_exit_at_monotonic_ms INTEGER",
      "error_code TEXT"
    ],
    "unique_key": "UNIQUE(provider_session_id, shutdown_epoch, signal_kind, generation)",
    "process_identity_guard": "A signal may be issued only when stored process_id and process_start_identity match the current OS process identity. Mismatch records identity_mismatch and holds with operator_next_step_code=manual_process_identity_check.",
    "addresses": [
      "SCORE-LIFT-REL-P083-R66-NB-002"
    ],
    "generation_replay_rule": {
      "reuse_same_generation": "Recovery reuses the same (provider_session_id, shutdown_epoch, signal_kind, generation) when a row exists with intent_state planned or issued and process identity still matches. planned without issued_at may issue that generation once; issued suppresses duplicate send and continues observation.",
      "increment_generation": "A new generation is created only when the prior generation is terminal observed, suppressed, identity_mismatch, or when policy opens a new shutdown_epoch after queued_no_signal retry. Generation is never incremented merely because the daemon restarted.",
      "duplicate_suppression": "The unique key suppresses duplicate sends for an already issued generation and emits shutdown_duplicate_signal_suppressed_total{provider}."
    },
    "fixtures": [
      "docs/evidence/083/shutdown-signal/crash-after-planned-reuses-generation-and-issues-once.fixture.json",
      "docs/evidence/083/shutdown-signal/crash-after-issued-reuses-generation-and-suppresses-duplicate.fixture.json",
      "docs/evidence/083/shutdown-signal/observed-generation-next-wave-increments.fixture.json",
      "docs/evidence/083/shutdown-signal/restart-alone-does-not-increment-generation.fixture.json"
    ]
  },
  "provider_cancellation_intent_contract_v1": {
    "addresses": [],
    "schema_version": "provider_cancellation_intent_contract_v1",
    "model": "cancellation_requested is a durable intent row, not a provider lifecycle_state.",
    "table": "provider_cancellation_intents",
    "columns": [
      "provider_session_id TEXT NOT NULL",
      "cancellation_epoch INTEGER NOT NULL",
      "intent_state TEXT NOT NULL CHECK(intent_state IN ('requested','shutdown_started','settled','held'))",
      "reason TEXT NOT NULL CHECK(reason IN ('operator_cancel','backpressure_cutoff','shutdown_recovery'))",
      "requested_at_monotonic_ms INTEGER NOT NULL",
      "requested_at_wall_clock TEXT NOT NULL",
      "shutdown_epoch INTEGER NULL",
      "shutdown_epoch_assigned_at TEXT NULL",
      "PRIMARY KEY(provider_session_id, cancellation_epoch)"
    ],
    "identity_ambiguous_canonical_rule": "If restart recovery cannot prove the provider process is live, absent, or already interrupted for a requested intent with shutdown_epoch IS NULL, it transactionally sets provider_cancellation_intents.intent_state='held', keeps shutdown_epoch NULL, sets provider_sessions.process_fate='identity_ambiguous', and returns operator_next_step_code=manual_process_identity_check. Held identity_ambiguous intents are not automatically retried on every restart; only operator action or new process evidence can move them back to requested or shutdown_started.",
    "fixtures": [
      "docs/evidence/083/cancellation/requested-null-epoch-identity-ambiguous-transitions-to-held.fixture.json",
      "docs/evidence/083/cancellation/held-identity-ambiguous-not-retried-on-restart.fixture.json",
      "docs/evidence/083/cancellation/operator-resolves-held-identity-ambiguous.fixture.json"
    ],
    "metric_rule": "provider_cancellation_intent_total uses bounded labels provider,intent_state,cancellation_reason. provider_session_lifecycle_total never emits cancellation_requested."
  },
  "migration_plan_v1": {
    "schema_version": "migration_plan_v1",
    "addresses": [
      "SCORE-LIFT-API-P083-R66-002"
    ],
    "ordering_rule": "Migrations are applied in listed order. Every release receipt, run_report, GraphQL, and MCP rollout readback lane exposes logical_id, filename, sha256, dependencies, applied_at, schema_version, state, and verification_query_result for each row.",
    "sha256_rule": "The gate computes SHA-256 over the migration file bytes at implementation time. Proposal text records expected sha256_source='migration_file_bytes' and readback must include the computed sha256; hard-coded placeholder hashes are rejected.",
    "rollback_rule": "Rollback never drops P083 additive schema. Rollback changes enforcement mode to permissive or disabled through rollback_execution_v1 and leaves all evidence queryable.",
    "migrations": [
      {
        "logical_id": "p083_001_artifact_lineage_report_kind",
        "filename": "control-plane/crates/db/migrations/20260602_p083_001_artifact_lineage_report_kind.sql",
        "depends_on": [],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_001_artifact_lineage_report_kind].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "ALTER TABLE artifact_lineage ADD COLUMN report_kind TEXT NULL",
          "CREATE TRIGGER artifact_lineage_report_kind_required_insert ...",
          "CREATE TRIGGER artifact_lineage_report_kind_required_update ...",
          "CREATE UNIQUE INDEX artifact_lineage_active_report_kind_uniq ON artifact_lineage(run_id, report_kind) WHERE active = 1 AND artifact_role = 'report' AND report_kind IS NOT NULL"
        ],
        "verification_query": "SELECT artifact_id FROM artifact_lineage WHERE artifact_role = 'report' AND active = 1 AND (report_kind IS NULL OR report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack'));",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_002_command_idempotency_generations",
        "filename": "control-plane/crates/db/migrations/20260602_p083_002_command_idempotency_generations.sql",
        "depends_on": [
          "p083_001_artifact_lineage_report_kind"
        ],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_002_command_idempotency_generations].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE command_idempotency(principal_id TEXT NOT NULL, request_id TEXT NOT NULL, command TEXT NOT NULL, intent_hash TEXT NOT NULL, lease_generation INTEGER NOT NULL, lease_state TEXT NOT NULL, acquired_at TEXT NOT NULL, expires_at TEXT NOT NULL, committed_at TEXT NULL, outcome_json TEXT NULL, failure_code TEXT NULL, PRIMARY KEY(principal_id, request_id, lease_generation))",
          "CREATE TABLE command_request_aliases(principal_id TEXT NOT NULL, command TEXT NOT NULL, intent_hash TEXT NOT NULL, request_id TEXT NOT NULL, canonical_request_id TEXT NOT NULL, created_at TEXT NOT NULL, PRIMARY KEY(principal_id, command, intent_hash, request_id))",
          "CREATE UNIQUE INDEX command_request_active_uniq ON command_idempotency(principal_id, request_id) WHERE lease_state IN ('pending','committed','failed')",
          "CREATE UNIQUE INDEX command_intent_active_uniq ON command_idempotency(principal_id, command, intent_hash) WHERE lease_state IN ('pending','committed')"
        ],
        "verification_query": "SELECT principal_id, request_id, lease_generation, COUNT(*) FROM command_idempotency GROUP BY principal_id, request_id, lease_generation HAVING COUNT(*) > 1;",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_003_shutdown_receipts_and_signals",
        "filename": "control-plane/crates/db/migrations/20260602_p083_003_shutdown_receipts_and_signals.sql",
        "depends_on": [
          "p083_002_command_idempotency_generations"
        ],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_003_shutdown_receipts_and_signals].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE shutdown_interrupted_receipts(... queue_rank INTEGER NULL ... CHECK queue_rank is non-null only for queued_no_signal)",
          "CREATE UNIQUE INDEX shutdown_interrupted_receipts_epoch_generation_uniq ON shutdown_interrupted_receipts(provider_session_id, shutdown_epoch, receipt_generation)",
          "CREATE TABLE shutdown_signal_side_effects(... generation INTEGER NOT NULL, intent_state TEXT NOT NULL ...)",
          "CREATE UNIQUE INDEX shutdown_signal_side_effect_unique ON shutdown_signal_side_effects(provider_session_id, shutdown_epoch, signal_kind, generation)"
        ],
        "verification_query": "SELECT receipt_id FROM shutdown_interrupted_receipts WHERE (interrupted_state = 'queued_no_signal' AND queue_rank IS NULL) OR (interrupted_state <> 'queued_no_signal' AND queue_rank IS NOT NULL);",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_004_cancel_late_output_overflow",
        "filename": "control-plane/crates/db/migrations/20260602_p083_004_cancel_late_output_overflow.sql",
        "depends_on": [
          "p083_003_shutdown_receipts_and_signals"
        ],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_004_cancel_late_output_overflow].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE cancel_late_output_overflow(... normalized_run_id TEXT GENERATED ALWAYS AS ..., normalized_provider_session_id TEXT GENERATED ALWAYS AS ...)",
          "CREATE UNIQUE INDEX cancel_late_output_overflow_latch_uniq ON cancel_late_output_overflow(scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind)",
          "CREATE INDEX cancel_late_output_overflow_scope_idx ON cancel_late_output_overflow(scope, normalized_run_id, overflow_kind)"
        ],
        "verification_query": "SELECT scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind, COUNT(*) FROM cancel_late_output_overflow GROUP BY scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind HAVING COUNT(*) > 1;",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_005_enforcement_and_rollback",
        "filename": "control-plane/crates/db/migrations/20260602_p083_005_enforcement_and_rollback.sql",
        "depends_on": [
          "p083_004_cancel_late_output_overflow"
        ],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_005_enforcement_and_rollback].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE p083_enforcement_mode_state(...) ",
          "CREATE TABLE p083_enforcement_mode_transition_journal(...) ",
          "CREATE TABLE p083_enforcement_mode_audit(...) ",
          "CREATE TABLE p083_rollback_audit(...)"
        ],
        "verification_query": "SELECT COUNT(*) FROM p083_enforcement_mode_transition_journal WHERE transition_state = 'transitioning' AND commit_marker IS NOT NULL;",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_006_durable_monotonic_clock",
        "filename": "control-plane/crates/db/migrations/20260602_p083_006_durable_monotonic_clock.sql",
        "depends_on": [
          "p083_005_enforcement_and_rollback"
        ],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_006_durable_monotonic_clock].sha256 equals computed migration file hash and state='applied' after daemon start records baseline sample",
        "ddl_summary": [
          "CREATE TABLE durable_monotonic_clock_samples(...) ",
          "CREATE INDEX durable_monotonic_clock_samples_boot_idx ON durable_monotonic_clock_samples(boot_id, observed_at_wall_clock)"
        ],
        "verification_query": "SELECT COUNT(*) FROM durable_monotonic_clock_samples WHERE sample_state = 'baseline';",
        "expected_verification_result": "at least one row after daemon start"
      },
      {
        "logical_id": "p083_007_provider_cancellation_intent_and_process_fate",
        "filename": "control-plane/crates/db/migrations/20260602_p083_007_provider_cancellation_intent_and_process_fate.sql",
        "depends_on": [
          "p083_006_durable_monotonic_clock"
        ],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_007_provider_cancellation_intent_and_process_fate].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE provider_cancellation_intents(provider_session_id TEXT NOT NULL, cancellation_epoch INTEGER NOT NULL, intent_state TEXT NOT NULL CHECK(intent_state IN ('requested','shutdown_started','settled','held')), reason TEXT NOT NULL CHECK(reason IN ('operator_cancel','backpressure_cutoff','shutdown_recovery')), requested_at_monotonic_ms INTEGER NOT NULL, requested_at_wall_clock TEXT NOT NULL, shutdown_epoch INTEGER NULL, shutdown_epoch_assigned_at TEXT NULL, PRIMARY KEY(provider_session_id, cancellation_epoch))",
          "CREATE INDEX provider_cancellation_intents_shutdown_epoch_idx ON provider_cancellation_intents(provider_session_id, shutdown_epoch) WHERE shutdown_epoch IS NOT NULL",
          "CREATE INDEX provider_cancellation_intents_state_idx ON provider_cancellation_intents(intent_state, reason)",
          "ALTER TABLE provider_sessions ADD COLUMN process_fate TEXT NOT NULL DEFAULT 'running' CHECK(process_fate IN ('running','backpressure_cutoff_shutdown_pending','absent_verified','interrupted_receipt_recorded','identity_ambiguous'))",
          "ALTER TABLE provider_sessions ADD COLUMN process_fate_updated_at TEXT NULL",
          "CREATE INDEX provider_sessions_process_fate_idx ON provider_sessions(process_fate)"
        ],
        "verification_query": "SELECT provider_session_id FROM provider_cancellation_intents WHERE intent_state IN ('shutdown_started','settled') AND shutdown_epoch IS NULL;",
        "expected_verification_result": "zero rows"
      }
    ]
  },
  "rollout_readback_api_parity_v1": {
    "schema_version": "rollout_readback_api_parity_v1",
    "addresses": [
      "SCORE-LIFT-API-P083-R66-001",
      "SCORE-LIFT-API-P083-R66-002"
    ],
    "normalization_rules": [
      "run_report, mcp, and release_receipt use snake_case keys exactly matching operator_readback_v1.",
      "GraphQL uses camelCase key projection with the same string values.",
      "Every declared field is required in every lane; nullable fields are present with explicit null.",
      "p083_shutdown_queue_rank is null except for queued_no_signal receipt readback and equals stored shutdown_interrupted_receipts.queue_rank when present."
    ],
    "graphql_sdl": [
      "scalar RollbackDispositionJSON",
      "type RolloutContractReadback { rolloutContractStatus: String! rolloutContractDecision: String! rolloutContractFailureReasons: [String!]! rolloutContractWaiverState: String! rolloutContractWaiverExpiresAt: String rolloutContractEnforcementMode: String! rolloutContractEnforcementModeReason: String! rolloutContractHoldConditions: [String!]! rolloutContractRollbackDisposition: RollbackDispositionJSON! rolloutContractSourceLane: String! rolloutContractEnabledState: String! rolloutContractDisabledReasonCode: String rolloutContractActionId: String rolloutContractOperatorMessage: String! rolloutContractProjectionIntegrity: String! rolloutContractCutoverPolicyRevision: String! rolloutContractDiagnosticRedaction: String! rolloutContractNextSteps: [String!]! rolloutContractShutdownDeadlineConfigState: String! rolloutContractCommandLeaseTtlConfigState: String! p083RollbackTtlExpiresAt: String p083LastPreflightHash: String p083ShutdownQueueRank: Int }"
    ],
    "mcp_schema_queue_rank_rule": "MCP property p083_shutdown_queue_rank has type [integer,null], minimum 0, is required, and is null unless the selected receipt interrupted_state is queued_no_signal.",
    "run_report_parity": "run_report includes rollout_contract_shutdown_deadline_config_state, rollout_contract_command_lease_ttl_config_state, and p083_shutdown_queue_rank with the same nullability as MCP.",
    "release_receipt_parity": "release_receipt includes the same rollout readback fields as run_report and MCP, including p083_shutdown_queue_rank.",
    "rollback_disposition_output_validation": "Before GraphQL serialization, resolver construction validates generated rollout_contract_rollback_disposition readback against rollback_disposition_v1, including schema_version. The inline rollout_contract_v1.rollback_disposition remains strict-template-compatible without schema_version; MCP, run_report, release_receipt, and GraphQL fixture examples all include schema_version:'rollback_disposition_v1'.",
    "negative_fixtures": [
      "docs/evidence/083/api/rollback-disposition-missing-schema-version-rejected.fixture.json"
    ]
  },
  "reliability_deadline_overflow_contract_v1": {
    "schema_version": "reliability_deadline_overflow_contract_v1",
    "addresses": [],
    "shutdown_deadline_defaults": {
      "graceful_ms": 15000,
      "kill_observation_ms": 15000,
      "host_total_ms": 30000,
      "max_host_total_ms": 120000
    },
    "appkit_budget_rule": "host_total_ms is honored only on graceful_appkit paths where applicationShouldTerminate is invoked. Force Quit, SIGKILL, crash, and power loss are abrupt_external and rely on restart recovery from durable intent rows instead of in-process deadline completion.",
    "bounded_shutdown_wave_policy": {
      "max_concurrent_graceful_signals": 8,
      "max_concurrent_kill_signals": 4,
      "ordering": "oldest shutdown_epoch first, then provider_session_id lexical tie-break",
      "queued_receipt_rule": "sessions not signaled before host_total_ms expires receive queued_no_signal receipts with deterministic queue_rank",
      "queue_rank_storage_rule": "queue_rank is stored as shutdown_interrupted_receipts.queue_rank INTEGER NULL, non-null only for interrupted_state=queued_no_signal. It is not called final_readback_rank in storage. Restart recovery reloads queue_rank from the stored column and never recomputes existing queued receipts."
    },
    "fixtures": [
      "docs/evidence/083/reliability/graceful-appkit-host-total-ms-budget.fixture.json",
      "docs/evidence/083/reliability/force-quit-abrupt-restart-recovery-no-host-budget-claim.fixture.json",
      "docs/evidence/083/reliability/sigkill-abrupt-restart-recovery-no-delegate-callback.fixture.json",
      "docs/evidence/083/reliability/crash-durable-intent-before-side-effect.fixture.json",
      "docs/evidence/083/reliability/many-session-queue-rank-stored.fixture.json",
      "docs/evidence/083/reliability/queue-rank-restart-preserved.fixture.json"
    ]
  },
  "swiftdata_lifecycle_boundary_contract_v1": {
    "schema_version": "swiftdata_lifecycle_boundary_contract_v1",
    "addresses": [
      "SCORE-LIFT-APPLE-P083-R66-NB-001"
    ],
    "rule": "SwiftData may hold projection-only and app-local state, but lifecycle truth remains backend/SQLite-owned. Lifecycle-bearing roots never receive the app-scoped lifecycle ModelContainer or a mutable lifecycle modelContext.",
    "pre_p083_store_transition_evidence": {
      "required": true,
      "evidence_goal": "Existing pre-P083 SwiftData stores launch cleanly after the projection-only/app-local container boundary lands, and no lifecycle-bearing root can access a lifecycle modelContext.",
      "fixtures": [
        "docs/evidence/083/swift/pre-p083-store-launches-with-projection-only-container.fixture.json",
        "docs/evidence/083/swift/pre-p083-store-migration-no-lifecycle-modelcontext-leakage.fixture.json",
        "docs/evidence/083/swift/lifecycle-root-app-scoped-modelcontext-leakage-rejected.fixture.json",
        "docs/evidence/083/swift/windowgroup-projection-value-type-only.fixture.json"
      ],
      "representative_store_matrix": [
        "empty_first_launch_store",
        "active_run_with_stage_and_agent_rows_store",
        "historical_artifacts_and_reports_store",
        "approval_pending_store",
        "provider_session_history_store"
      ],
      "implementation_signoff_rule": "Fixtures must run against copied representative pre-P083 stores, not only synthetic in-memory stores. Sign-off records input store hash, migrated store hash, and proof that lifecycle-bearing roots receive no mutable lifecycle modelContext."
    },
    "guardrails": [
      "Static scan rejects ModelContext.insert/save/delete touching Run, StageExecution, AgentExecution, Approval, ProviderSession, command idempotency, shutdown, cancellation, or lifecycle state models outside approved projection adapters.",
      "Production mutation_origin guard rejects lifecycle @Model writes from SwiftUI roots.",
      "Retained WindowGroup surfaces register app-local/projection schemas only."
    ]
  },
  "current_review_refresh_gate_v1": {
    "schema_version": "current_review_refresh_gate_v1",
    "required_before_ready": [
      {
        "review_scope": "security",
        "required_artifact": "reviews/proposal/security-current.json",
        "reviewed_proposal_revision_id_must_equal": "P083-r68-refined-r66-score-lift",
        "stale_artifact_policy": "Selected P083-r53 security findings are historical checklist material only and cannot satisfy approval for this revision."
      },
      {
        "review_scope": "observability_rollout",
        "required_artifact": "reviews/proposal/observability-rollout-current.json",
        "reviewed_proposal_revision_id_must_equal": "P083-r68-refined-r66-score-lift",
        "stale_artifact_policy": "Selected P083-r55 rollout findings are historical checklist material only and cannot satisfy approval for this revision."
      }
    ],
    "freeze_rule": "The proposal cannot be frozen for implementation unless both current-revision artifacts exist, identify P083-r68-refined-r66-score-lift as the reviewed revision, and do not return blocking findings.",
    "routing_note": "This proposal revision routes the two stale-review scopes as explicit approval prerequisites. It does not claim that the refreshed reviews have already happened."
  },
  "metric_labels_contract_v1": {
    "schema_version": "metric_labels_contract_v1",
    "addresses": [
      "SCORE-LIFT-API-P083-R66-NB-001"
    ],
    "authority_rule": "This section is the source metric inventory and owns bounded label domains. metrics.operational_metrics_reference and rollout_contract_v1.metrics.operational_metrics are generated mirrors and must remain byte-equal.",
    "bounded_label_domains": {
      "surface": [
        "graphql",
        "mcp",
        "run_report",
        "release_receipt",
        "swift_ui"
      ],
      "state": [
        "fresh",
        "stale",
        "missing",
        "unknown",
        "tampered"
      ],
      "lifecycle_state": [
        "registered",
        "spawn_error_no_child",
        "launch_handshake",
        "live",
        "self_exit_observed",
        "terminated_graceful",
        "terminated_by_kill",
        "orphan_settled",
        "shutdown_interrupted",
        "backpressure_cutoff"
      ],
      "outcome": [
        "acquired",
        "replayed",
        "denied",
        "committed",
        "failed",
        "abandoned",
        "expired_reacquired"
      ],
      "proposal_id": [
        "P083"
      ],
      "status": [
        "pass",
        "fail",
        "waived",
        "not_applicable",
        "timeout",
        "cancelled",
        "missing_contract",
        "stale",
        "tamper_detected"
      ],
      "failure_reason": [
        "schema_invalid",
        "missing_fixture",
        "metric_unbounded",
        "auth_dependency_missing",
        "hold_condition_present",
        "burn_in_incomplete",
        "rollback_contract_invalid",
        "stale_revision",
        "tamper_detected",
        "missing_schema_version"
      ],
      "reason": [
        "auth_dependency_missing",
        "hold_condition_present",
        "projection_not_fresh",
        "migration_not_applied",
        "rollback_ttl_expired",
        "gate_failed",
        "current_review_missing",
        "identity_ambiguous"
      ],
      "enforcement_mode": [
        "disabled",
        "permissive",
        "enforce"
      ],
      "transition": [
        "disabled_to_permissive",
        "permissive_to_enforce",
        "enforce_to_permissive",
        "permissive_to_disabled",
        "disabled_to_enforce_denied"
      ],
      "action": [
        "disable_to_permissive",
        "permissive_to_enforce",
        "enforce_to_permissive",
        "rollback_disable",
        "reenable_after_rollback",
        "manual_process_identity_check"
      ],
      "cancellation_reason": [
        "operator_cancel",
        "backpressure_cutoff",
        "shutdown_recovery"
      ],
      "command": [
        "runs.cancel",
        "runs.retry",
        "stages.retry",
        "approvals.resolve",
        "side_effects.force_reconcile",
        "command.run",
        "copyable_command.regenerate",
        "provider_session.shutdown",
        "p083.rollback_execution",
        "p083.set_enforcement_mode"
      ],
      "provider": [
        "codex",
        "claude",
        "gemini",
        "auggie",
        "junie"
      ],
      "intent_state": [
        "requested",
        "shutdown_started",
        "settled",
        "held",
        "planned",
        "issued",
        "observed",
        "suppressed",
        "identity_mismatch"
      ],
      "process_fate": [
        "running",
        "backpressure_cutoff_shutdown_pending",
        "absent_verified",
        "interrupted_receipt_recorded",
        "identity_ambiguous"
      ],
      "scope": [
        "session",
        "run",
        "global"
      ],
      "overflow_kind": [
        "message_count",
        "session_bytes",
        "elapsed_time",
        "run_bytes",
        "global_bytes"
      ],
      "interrupted_state": [
        "grace_deadline_expired",
        "kill_signal_issued",
        "kill_pid_exit_observed",
        "queued_no_signal",
        "shutdown_interrupted"
      ]
    },
    "operational_metric_label_signatures": [
      "artifact_lineage_projection_integrity_total{surface,state}",
      "provider_session_lifecycle_total{provider,lifecycle_state}",
      "command_idempotency_lease_acquire_total{command,outcome}",
      "command_idempotency_replay_total{command,outcome}",
      "shutdown_interrupted_receipt_total{provider,interrupted_state}",
      "shutdown_duplicate_signal_suppressed_total{provider}",
      "cancel_late_output_overflow_total{provider,scope,overflow_kind}",
      "cancel_late_output_dropped_total{provider,scope,overflow_kind}",
      "rollout_contract_lint_total{proposal_id,status,failure_reason}",
      "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}",
      "p083_enforcement_mode_transition_total{transition,enforcement_mode}",
      "p083_rollback_execution_total{action,status,reason}",
      "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}"
    ],
    "fixtures": [
      "docs/evidence/083/metrics/all-operational-label-domains-bounded.fixture.json",
      "docs/evidence/083/metrics/unbounded-label-rejected.fixture.json",
      "docs/evidence/083/metrics/metric-mirrors-byte-equal.fixture.json"
    ]
  },
  "metrics": {
    "authority": "metric_labels_contract_v1.operational_metric_label_signatures",
    "adoption_metric": "p083_applicable_runs_with_passing_execution_truth_preflight_percent",
    "operational_metrics_reference": [
      "artifact_lineage_projection_integrity_total{surface,state}",
      "provider_session_lifecycle_total{provider,lifecycle_state}",
      "command_idempotency_lease_acquire_total{command,outcome}",
      "command_idempotency_replay_total{command,outcome}",
      "shutdown_interrupted_receipt_total{provider,interrupted_state}",
      "shutdown_duplicate_signal_suppressed_total{provider}",
      "cancel_late_output_overflow_total{provider,scope,overflow_kind}",
      "cancel_late_output_dropped_total{provider,scope,overflow_kind}",
      "rollout_contract_lint_total{proposal_id,status,failure_reason}",
      "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}",
      "p083_enforcement_mode_transition_total{transition,enforcement_mode}",
      "p083_rollback_execution_total{action,status,reason}",
      "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}"
    ],
    "success_thresholds": {
      "preflight_pass_rate": ">= 99% for applicable runs during permissive burn-in",
      "metric_staleness": "all required scrapes fresher than 180 seconds",
      "rollback_readback_parity": "byte-equal rollback_disposition JSON across MCP, run_report, release_receipt, and GraphQL scalar payload"
    }
  },
  "rollout": {
    "phases": [
      {
        "phase": "design_freeze",
        "entry": "proposal review marks Ready for this revision",
        "exit": "proposal-083 gate and rollout contract lint pass"
      },
      {
        "phase": "additive_migrations",
        "entry": "Ready proposal",
        "exit": "migration readback fixture passes"
      },
      {
        "phase": "permissive_burn_in",
        "entry": "mode transition disabled_to_permissive audited",
        "exit": "24 hours with zero hold conditions and fresh metrics"
      },
      {
        "phase": "enforce_cutover",
        "entry": "preflight requirements pass",
        "exit": "mode transition permissive_to_enforce audited"
      },
      {
        "phase": "rollback_if_needed",
        "entry": "hold condition or operator emergency",
        "exit": "rollback_execution_v1 readback and audit rows present"
      }
    ],
    "hold_conditions": [
      "projection_integrity_not_fresh",
      "rollout_contract_lint_failed",
      "metric_scrape_stale",
      "auth_dependency_missing",
      "shutdown_receipt_history_invalid",
      "post_cancel_overflow_latch_failed",
      "migration_readback_missing_or_hash_mismatch",
      "shutdown_deadline_config_invalid",
      "command_lease_ttl_config_invalid",
      "current_security_review_missing_or_stale",
      "current_observability_rollout_review_missing_or_stale",
      "command_idempotency_contract_invalid",
      "migration_readback_sha256_missing",
      "manual_identity_check_unresolved"
    ],
    "fixture_readiness_rule": "rollout_contract_v1 declares P083-owned fixture paths, and scripts/lint-rollout-contract must pass against those paths before design freeze. Missing P083 readback or negative fixtures are a release hold. Each P083 fixture must assert proposal_id=P083 plus the active proposal_revision_id."
  },
  "rollout_contract_v1": {
    "schema_version": "rollout_contract_v1",
    "applicability": "required",
    "gate_aliases": [
      "proposal-083",
      "p083"
    ],
    "commands": {
      "allowlist": [
        "./scripts/test-gate.sh proposal-083",
        "./scripts/test-gate.sh p083"
      ],
      "commentary": "Gate commands are declarative expectations; the linter does not execute them."
    },
    "migrations": {
      "not_applicable": false,
      "justification": "P083 owns seven additive SQLite migrations enumerated in migration_plan_v1. Release receipt and operator readback must expose sha256 and verification query result for each logical_id."
    },
    "metrics": {
      "adoption_metric": "p083_applicable_runs_with_passing_execution_truth_preflight_percent",
      "operational_metrics": [
        "artifact_lineage_projection_integrity_total{surface,state}",
        "provider_session_lifecycle_total{provider,lifecycle_state}",
        "command_idempotency_lease_acquire_total{command,outcome}",
        "command_idempotency_replay_total{command,outcome}",
        "shutdown_interrupted_receipt_total{provider,interrupted_state}",
        "shutdown_duplicate_signal_suppressed_total{provider}",
        "cancel_late_output_overflow_total{provider,scope,overflow_kind}",
        "cancel_late_output_dropped_total{provider,scope,overflow_kind}",
        "rollout_contract_lint_total{proposal_id,status,failure_reason}",
        "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}",
        "p083_enforcement_mode_transition_total{transition,enforcement_mode}",
        "p083_rollback_execution_total{action,status,reason}",
        "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}"
      ]
    },
    "readback_lanes": [
      "run_report",
      "mcp",
      "release_receipt",
      "graphql"
    ],
    "readback_fields": [
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
      "rollout_contract_shutdown_deadline_config_state",
      "rollout_contract_command_lease_ttl_config_state",
      "p083_rollback_ttl_expires_at",
      "p083_last_preflight_hash",
      "p083_shutdown_queue_rank"
    ],
    "readback_fixture": "docs/evidence/rollout-contract/operator-readback/p083-full-surface.fixture.json",
    "operator_report_fields": [
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
      "rollout_contract_shutdown_deadline_config_state",
      "rollout_contract_command_lease_ttl_config_state",
      "p083_rollback_ttl_expires_at",
      "p083_last_preflight_hash",
      "p083_shutdown_queue_rank"
    ],
    "hold_conditions": [
      "projection_integrity_not_fresh",
      "rollout_contract_lint_failed",
      "metric_scrape_stale",
      "auth_dependency_missing",
      "shutdown_receipt_history_invalid",
      "post_cancel_overflow_latch_failed",
      "migration_readback_missing_or_hash_mismatch",
      "shutdown_deadline_config_invalid",
      "command_lease_ttl_config_invalid",
      "current_security_review_missing_or_stale",
      "current_observability_rollout_review_missing_or_stale",
      "command_idempotency_contract_invalid",
      "migration_readback_sha256_missing",
      "manual_identity_check_unresolved"
    ],
    "hold_conditions_detail": {
      "current_security_review_missing_or_stale": "Security review artifact must name this proposal_revision_id and contain no blocking issues.",
      "current_observability_rollout_review_missing_or_stale": "Observability rollout review artifact must name this proposal_revision_id and contain no blocking issues.",
      "shutdown_deadline_config_invalid": "Configured shutdown deadline exceeds hard maximum or claims AppKit coverage for abrupt_external termination.",
      "command_lease_ttl_config_invalid": "Configured command lease TTL is outside reliability bounds.",
      "command_idempotency_contract_invalid": "Command idempotency schema, TTL, or recovery fixtures fail.",
      "migration_readback_sha256_missing": "Any P083 migration lacks sha256 readback or verification_query_result.",
      "manual_identity_check_unresolved": "A provider cancellation intent is held for manual_process_identity_check and blocks enforcement cutover."
    },
    "rollback_disposition": {
      "mode": "p083.rollback_execution_to_permissive_or_disabled",
      "data_loss_risk": "none",
      "steps": [
        "Call p083RollbackExecution or p083.rollback_execution with operator principal class and CallerRequestId.",
        "Persist rollback audit and enforcement-mode audit rows.",
        "Expose disabled/permissive state, generated schema-versioned rollback disposition readback, and TTL in every readback lane.",
        "Require fresh permissive burn-in and enforce preflight before returning to enforce mode."
      ]
    },
    "decision_vocabulary": [
      "pass",
      "fail",
      "waived",
      "not_applicable",
      "timeout",
      "cancelled",
      "missing_contract",
      "tamper_detected",
      "stale",
      "release",
      "hold",
      "waive"
    ],
    "negative_fixtures": {
      "missing_metric_domain": "docs/evidence/rollout-contract/negative/p083-missing-metric-domain.json",
      "missing_rollback_contract": "docs/evidence/rollout-contract/negative/p083-missing-rollback-contract.json",
      "foreign_fixture_reference": "docs/evidence/rollout-contract/negative/p083-foreign-fixture-reference.json",
      "enforce_without_burnin": "docs/evidence/rollout-contract/negative/p083-enforce-without-burnin.json",
      "stale_security_review": "docs/evidence/rollout-contract/negative/p083-stale-security-review.json",
      "stale_observability_rollout_review": "docs/evidence/rollout-contract/negative/p083-stale-observability-rollout-review.json",
      "force_quit_host_budget_claim": "docs/evidence/rollout-contract/negative/p083-force-quit-host-budget-claim.json",
      "queue_rank_final_readback_rank_storage": "docs/evidence/rollout-contract/negative/p083-final-readback-rank-stored.json",
      "rollback_disposition_missing_schema_version": "docs/evidence/rollout-contract/negative/p083-rollback-disposition-missing-schema-version.json",
      "migration_sha256_missing": "docs/evidence/rollout-contract/negative/p083-migration-sha256-missing.json",
      "unbounded_metric_label": "docs/evidence/rollout-contract/negative/p083-unbounded-metric-label.json"
    },
    "cutover_policy": {
      "revision": "p083-rollout-cutover-r68",
      "enforcement_mode_at_cutover": "enforce",
      "applicable_to": "post_ready_implementation_starts",
      "effective_timestamp_iso8601": "2026-06-02T00:00:00Z"
    }
  },
  "risks_and_mitigations": [
    {
      "risk": "Command idempotency is a shared execution-truth path and can block unrelated lifecycle commands if TTL recovery is wrong.",
      "mitigation": "Use explicit per-command TTLs, SQLite CAS reacquire, committed-unack replay fixtures, and bounded metrics for every covered command."
    },
    {
      "risk": "The full migration surface increases proposal size and maintenance burden.",
      "mitigation": "Keep one canonical migration_plan_v1 inventory with sha256_source and generated readback mirrors instead of duplicating migration metadata across feature sections."
    },
    {
      "risk": "Manual identity checks can interrupt operator flow.",
      "mitigation": "Render the hold inline with copyable diagnostics, read-only retry, explicit backend actions, and no focus-stealing spinner."
    },
    {
      "risk": "Late provider output may arrive after cancellation and look useful.",
      "mitigation": "Quarantine it as evidence and prove active projections/artifacts cannot be mutated after overflow latch activation."
    },
    {
      "risk": "Representative SwiftData stores may expose migration cases not covered by synthetic fixtures.",
      "mitigation": "Implementation sign-off requires copied pre-P083 stores spanning active runs, approvals, provider history, and artifacts."
    }
  ],
  "open_questions": [
    "Should the permissive burn-in duration remain fixed at 24 hours or become a release-channel setting after P083 lands?",
    "Should side_effects.force_reconcile keep a 300 second TTL permanently, or should it move to a lower value after operational data is available?",
    "Which dashboard owns long-term alert thresholds for command_idempotency_contract_invalid and manual_identity_check_unresolved hold conditions?"
  ],
  "acceptance_criteria": [
    "proposal-083 and p083 gates exist and run the P083 contract suite.",
    "No active proposal section, revision summary, coverage object, or feedback mapping claims blocker ids absent from the current R66 score_lift_backlog.",
    "rollout_contract_v1.rollback_disposition remains strict-template-compatible without unknown fields; generated GraphQL, MCP, run_report, and release_receipt RollbackDispositionJSON fixtures include schema_version='rollback_disposition_v1' and reject missing schema_version.",
    "migration_plan_v1 enumerates all seven P083 additive migrations with logical_id, filename, dependencies, sha256_source, readback expectation, verification query, and expected verification result.",
    "command_idempotency_contract_v1 covers runs.cancel, runs.retry, stages.retry, approvals.resolve, side_effects.force_reconcile, provider_session.shutdown, p083.rollback_execution, and p083.set_enforcement_mode with states, TTLs, unique keys, recovery rules, and fixtures.",
    "ManualProcessIdentityCheckBanner renders manual_process_identity_check with visible copy, no automatic retry spinner, explicit resolution actions, and VoiceOver-readable denial state.",
    "Every operational metric label used by operational_metric_label_signatures has a bounded domain or fails lint.",
    "post_cancel_late_output_contract_v1 proves unique overflow latch keys, cap bounds, restart idempotency, readback fields, and a negative fixture rejecting active projection mutation after cancellation.",
    "shutdown_signal_side_effect generation is reused after crash-before-planned or crash-after-issued recovery and increments only after terminal prior generation or new shutdown epoch; duplicate sends are suppressed by fixture.",
    "SwiftData transition fixtures run against representative copied pre-P083 stores and prove no lifecycle modelContext leakage into lifecycle-bearing roots.",
    "Toolbar, menu, and keyboard commands route from the focused lifecycle window; unavailable commands remain disabled-but-visible where appropriate and expose VoiceOver-readable denial reasons.",
    "applicationShouldTerminate terminateLater plus host_total_ms is asserted only for graceful Quit and logout/system shutdown paths where AppKit invokes the delegate callback.",
    "Force Quit, SIGKILL, and crash fixtures prove restart recovery through shutdown_signal_side_effects or provider_cancellation_intents without assuming delegate callback execution.",
    "shutdown_interrupted_receipts stores queue_rank INTEGER NULL, non-null only for interrupted_state=queued_no_signal; final_readback_rank is not a stored column.",
    "provider_cancellation_intents.requested with null shutdown_epoch and ambiguous identity transitions to intent_state=held, process_fate=identity_ambiguous, and is not retried automatically on every restart.",
    "scripts/lint-rollout-contract passes for the inline rollout_contract_v1 after all declared readback and negative fixtures are created."
  ],
  "reviewer_feedback_resolution": {
    "SCORE-LIFT-API-P083-R66-001": {
      "disposition": "addressed",
      "severity": "blocking",
      "required_change": "Add schema_version:'rollback_disposition_v1' to rollout_contract_v1.rollback_disposition and align run_report, MCP, release_receipt, and GraphQL fixture examples. Add a negative fixture for missing schema_version.",
      "addressed_by_sections": [
        "rollout_contract_v1.rollback_disposition",
        "api_contracts.rollback_disposition_json_v1",
        "rollout_readback_api_parity_v1",
        "acceptance_criteria"
      ],
      "resolution_notes": "The executable rollout template disallows schema_version inside inline rollout_contract_v1.rollback_disposition, so the inline object remains strict-template-compatible. The generated RollbackDispositionJSON readback for GraphQL, MCP, run_report, and release_receipt includes schema_version and has a missing-schema-version negative fixture."
    },
    "SCORE-LIFT-API-P083-R66-002": {
      "disposition": "addressed",
      "severity": "blocking",
      "required_change": "Make migration_plan_v1 cover every P083 migration with logical_id, filename, dependencies, sha256/readback expectations, and verification query, or explicitly narrow rollout_contract_v1.migrations.justification to the migrations this revision owns and cite authoritative specs for the rest.",
      "addressed_by_sections": [
        "migration_plan_v1",
        "rollout_contract_v1.migrations",
        "acceptance_criteria"
      ],
      "resolution_notes": "migration_plan_v1 now enumerates all seven P083 additive migrations and the rollout contract points to that full owned surface."
    },
    "SCORE-LIFT-REL-P083-R66-001": {
      "disposition": "addressed",
      "severity": "blocking",
      "required_change": "Add command_idempotency_contract_v1 or bind to an exact existing contract with table names, states, TTLs, unique keys, recovery rules, and fixtures for runs.cancel, runs.retry, stages.retry, approvals.resolve, side_effects.force_reconcile, provider_session.shutdown, p083.rollback_execution, and p083.set_enforcement_mode.",
      "addressed_by_sections": [
        "command_idempotency_contract_v1",
        "migration_plan_v1.migrations[p083_002_command_idempotency_generations]",
        "metric_labels_contract_v1",
        "acceptance_criteria"
      ],
      "resolution_notes": "Added executable command idempotency contract with table/state/TTL/key/recovery details and per-command fixtures."
    },
    "SCORE-LIFT-UI-P083-R66-001": {
      "disposition": "addressed",
      "severity": "blocking",
      "required_change": "Specify what the operator sees and can do for manual_process_identity_check, including the component, copy, available actions, resolution path, accessibility behavior, and absence of automatic retry spinner.",
      "addressed_by_sections": [
        "manual_process_identity_check_ui_v1",
        "ux_ui_notes.manual_process_identity_check",
        "native_command_validation_contract_v1",
        "acceptance_criteria"
      ],
      "resolution_notes": "Defined ManualProcessIdentityCheckBanner, copy, actions, backend-cleared resolution, VoiceOver behavior, and no-spinner rule."
    },
    "SCORE-LIFT-API-P083-R66-NB-001": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Add bounded domains or source-of-truth references for labels used by operational metrics, including surface, state, lifecycle_state, outcome, proposal_id, status, failure_reason, reason, enforcement_mode, transition, action, and cancellation_reason.",
      "addressed_by_sections": [
        "metric_labels_contract_v1",
        "metrics",
        "rollout_contract_v1.metrics"
      ],
      "resolution_notes": "Every operational metric label domain is bounded in metric_labels_contract_v1 and mirrored by metrics and rollout_contract_v1."
    },
    "SCORE-LIFT-REL-P083-R66-NB-001": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Add or reference a post-cancel overflow latch contract with unique latch keys, cap bounds, restart idempotency, readback fields, and a negative fixture proving late outputs after cancellation cannot mutate active projections.",
      "addressed_by_sections": [
        "post_cancel_late_output_contract_v1",
        "migration_plan_v1.migrations[p083_004_cancel_late_output_overflow]",
        "acceptance_criteria"
      ],
      "resolution_notes": "Added late-output overflow latch contract with unique key, caps, restart behavior, readback, and active-projection negative fixture."
    },
    "SCORE-LIFT-REL-P083-R66-NB-002": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Define when shutdown_signal_side_effects.generation is reused or incremented during recovery and add crash-after-planned and crash-after-issued fixtures proving duplicate suppression.",
      "addressed_by_sections": [
        "shutdown_signal_side_effect_contract_v1.generation_replay_rule",
        "acceptance_criteria"
      ],
      "resolution_notes": "Generation is reused across restart for planned/issued rows and increments only after terminal prior generation or new shutdown epoch."
    },
    "SCORE-LIFT-APPLE-P083-R66-NB-001": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Run pre-P083 store launch compatibility and modelContext leakage rejection fixtures against realistic existing SwiftData stores during implementation sign-off.",
      "addressed_by_sections": [
        "swiftdata_lifecycle_boundary_contract_v1.pre_p083_store_transition_evidence",
        "acceptance_criteria"
      ],
      "resolution_notes": "Representative copied pre-P083 store matrix and sign-off evidence rule are now explicit."
    },
    "SCORE-LIFT-MACOS-P083-R66-NB-001": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Validate toolbar, menu, and keyboard commands from the focused lifecycle window, keep unavailable commands disabled-but-visible where appropriate, and expose the denial reason through VoiceOver-readable labels, help, or adjacent status text without stealing focus.",
      "addressed_by_sections": [
        "native_command_validation_contract_v1",
        "manual_process_identity_check_ui_v1.accessibility",
        "acceptance_criteria"
      ],
      "resolution_notes": "Focused lifecycle-window routing, disabled-visible command behavior, and VoiceOver-readable denial reasons are specified with fixtures."
    }
  },
  "command_idempotency_contract_v1": {
    "schema_version": "command_idempotency_contract_v1",
    "addresses": [
      "SCORE-LIFT-REL-P083-R66-001"
    ],
    "authority": "command_idempotency and command_request_aliases SQLite tables",
    "commands_covered": [
      "runs.cancel",
      "runs.retry",
      "stages.retry",
      "approvals.resolve",
      "side_effects.force_reconcile",
      "provider_session.shutdown",
      "p083.rollback_execution",
      "p083.set_enforcement_mode"
    ],
    "tables": {
      "command_idempotency": {
        "primary_key": [
          "principal_id",
          "request_id",
          "lease_generation"
        ],
        "states": [
          "pending",
          "committed",
          "failed",
          "abandoned"
        ],
        "required_columns": [
          "principal_id",
          "request_id",
          "command",
          "intent_hash",
          "lease_generation",
          "lease_state",
          "acquired_at",
          "expires_at",
          "committed_at",
          "outcome_json",
          "failure_code"
        ]
      },
      "command_request_aliases": {
        "primary_key": [
          "principal_id",
          "command",
          "intent_hash",
          "request_id"
        ],
        "purpose": "Maps same-intent replacement request ids to the canonical committed request for replay without creating duplicate lifecycle side effects."
      }
    },
    "unique_keys": [
      "UNIQUE(principal_id, request_id) WHERE lease_state IN ('pending','committed','failed')",
      "UNIQUE(principal_id, command, intent_hash) WHERE lease_state IN ('pending','committed')",
      "UNIQUE(principal_id, request_id, lease_generation)"
    ],
    "ttl_seconds": {
      "runs.cancel": 120,
      "runs.retry": 120,
      "stages.retry": 120,
      "approvals.resolve": 300,
      "side_effects.force_reconcile": 300,
      "provider_session.shutdown": 120,
      "p083.rollback_execution": 120,
      "p083.set_enforcement_mode": 120,
      "min": 5,
      "max_configurable": 900
    },
    "acquisition_rules": [
      "Malformed request_id is rejected before command_idempotency acquisition.",
      "Same principal_id/request_id/command/intent_hash with pending lease returns pending replay with retry_after_seconds.",
      "Same principal_id/request_id with different command or intent_hash is denied as request_intent_mismatch.",
      "Same command/intent_hash with a committed canonical request replays committed outcome and records command_request_aliases when caller uses a new request_id.",
      "Expired pending rows are reacquired by SQLite compare-and-set that increments lease_generation and sets lease_state=pending with a new expires_at. Concurrent losers replay the winning pending state."
    ],
    "recovery_rules": {
      "pending_not_expired": "Remain pending and return retry_after_seconds based on expires_at.",
      "pending_expired_no_side_effect_receipt": "Mark prior generation abandoned and acquire generation+1 in one transaction.",
      "pending_expired_side_effect_receipt_exists": "Finish committed or failed from the authoritative side-effect receipt before replaying.",
      "committed_unacknowledged": "Replay outcome_json byte-for-byte for the same request and for aliased same-intent requests.",
      "failed_terminal": "Replay failure for same request; new request with same intent may acquire only when command-specific retry policy allows it."
    },
    "fixtures": [
      "docs/evidence/083/idempotency/runs-cancel-same-request-replayed.fixture.json",
      "docs/evidence/083/idempotency/runs-retry-same-intent-new-request-aliased.fixture.json",
      "docs/evidence/083/idempotency/stages-retry-request-intent-mismatch-denied.fixture.json",
      "docs/evidence/083/idempotency/approvals-resolve-pending-retry-after.fixture.json",
      "docs/evidence/083/idempotency/side-effects-force-reconcile-expired-reacquire.fixture.json",
      "docs/evidence/083/idempotency/provider-session-shutdown-side-effect-receipt-settles.fixture.json",
      "docs/evidence/083/idempotency/p083-rollback-execution-committed-unack-replay.fixture.json",
      "docs/evidence/083/idempotency/p083-set-enforcement-mode-bounded-command-label.fixture.json"
    ]
  },
  "manual_process_identity_check_ui_v1": {
    "schema_version": "manual_process_identity_check_ui_v1",
    "addresses": [
      "SCORE-LIFT-UI-P083-R66-001",
      "SCORE-LIFT-MACOS-P083-R66-NB-001"
    ],
    "component": "ManualProcessIdentityCheckBanner",
    "surfaces": [
      "Run detail provider-session section",
      "Stage detail provider-session row",
      "Recovery inbox item"
    ],
    "visible_copy": {
      "title": "Process identity needs review",
      "body": "Forge could not prove this provider process is still the same process that was cancelled. Automatic retry is paused until you verify the process identity.",
      "disabled_retry_reason": "Automatic retry paused: process identity is ambiguous."
    },
    "available_actions": [
      {
        "action": "copy_diagnostic",
        "effect": "Copies provider_session_id, cancellation_epoch, process_fate, last_seen_pid, process_start_identity hash, and latest receipt id with secrets redacted."
      },
      {
        "action": "retry_identity_check",
        "effect": "Runs a read-only process identity probe and refreshes readback. It does not issue shutdown signals."
      },
      {
        "action": "mark_process_absent",
        "effect": "Requires operator confirmation and CallerRequestId; if backend confirms absence, moves process_fate to absent_verified and resumes settlement."
      },
      {
        "action": "open_provider_session_evidence",
        "effect": "Opens the evidence panel anchored to the focused lifecycle window."
      }
    ],
    "resolution_path": "The banner clears only after backend readback moves intent_state away from held or process_fate away from identity_ambiguous. UI state alone cannot clear the hold.",
    "no_spinner_rule": "Held identity_ambiguous rows show no automatic retry spinner and no countdown. Retry Identity Check is an explicit operator action.",
    "accessibility": "VoiceOver announces title, provider display name, reason, and focused action. Disabled toolbar/menu commands remain visible where native convention allows and expose the denial reason through accessibilityHelp or adjacent status text without stealing focus.",
    "fixtures": [
      "docs/evidence/083/ui/manual-process-identity-check-banner.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-no-auto-spinner.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-voiceover.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-resolution-actions.fixture.json"
    ]
  },
  "post_cancel_late_output_contract_v1": {
    "schema_version": "post_cancel_late_output_contract_v1",
    "addresses": [
      "SCORE-LIFT-REL-P083-R66-NB-001"
    ],
    "authority_table": "cancel_late_output_overflow",
    "unique_latch_key": [
      "scope",
      "normalized_run_id",
      "normalized_provider_session_id",
      "cancellation_epoch",
      "overflow_kind"
    ],
    "cap_bounds": {
      "session_message_count": 200,
      "session_bytes": 1048576,
      "run_bytes": 8388608,
      "global_bytes": 67108864,
      "elapsed_time_ms": 300000
    },
    "restart_idempotency": "On restart, recovery recomputes the normalized latch key and updates the existing row counters. It never inserts a duplicate row for the same scope/run/session/cancellation_epoch/overflow_kind.",
    "readback_fields": [
      "scope",
      "normalized_run_id",
      "normalized_provider_session_id",
      "cancellation_epoch",
      "overflow_kind",
      "dropped_message_count",
      "dropped_byte_count",
      "quarantine_uri",
      "reservation_release_state",
      "projection_mutation_blocked"
    ],
    "active_projection_rule": "Provider outputs observed after cancellation settlement or overflow latch activation may be quarantined as evidence but cannot mutate active projections, active artifacts, or transition inputs.",
    "fixtures": [
      "docs/evidence/083/late-output/overflow-latch-unique-key.fixture.json",
      "docs/evidence/083/late-output/restart-updates-existing-latch.fixture.json",
      "docs/evidence/083/late-output/late-output-active-projection-mutation-rejected.fixture.json",
      "docs/evidence/083/late-output/cap-bounds-enforced.fixture.json"
    ]
  },
  "native_command_validation_contract_v1": {
    "schema_version": "native_command_validation_contract_v1",
    "addresses": [
      "SCORE-LIFT-MACOS-P083-R66-NB-001"
    ],
    "focused_window_rule": "Toolbar, menu, and keyboard commands resolve through the focused lifecycle_window_id. If no lifecycle window is key, lifecycle commands remain disabled-but-visible where native macOS convention allows and do not perform side effects.",
    "disabled_reason_rule": "Unavailable commands expose denial reason through accessibilityHelp, toolbar help, or adjacent status text without moving focus. Disabled controls are never hidden solely because the backend action is unavailable.",
    "commands_covered": [
      "Cancel Run",
      "Retry Run",
      "Retry Stage",
      "Resolve Approval",
      "Shutdown Provider Session",
      "Export Text",
      "Copy Diagnostic",
      "Retry Identity Check"
    ],
    "fixtures": [
      "docs/evidence/083/macos/focused-lifecycle-window-command-routing.fixture.json",
      "docs/evidence/083/macos/no-key-lifecycle-window-disabled-visible.fixture.json",
      "docs/evidence/083/macos/voiceover-disabled-command-reason.fixture.json",
      "docs/evidence/083/macos/toolbar-menu-keyboard-denial-parity.fixture.json"
    ]
  }
}
