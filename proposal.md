{
  "schema_version": "proposal_document_v1",
  "proposal_id": "P083",
  "proposal_revision_id": "P083-r65-refined-97d0ecda",
  "title": "Execution-Truth Ownership and Invariant Model",
  "date": "2026-06-02",
  "status": "Revise-Required. This revision is rebased onto score_lift_backlog review_pass_id proposal-review-P083-r64-refined-3da64326-97d0ecda-4b31-40f2-91c0-bf047d39e1f8. It addresses every current R64 score-lift backlog item inline, removes older R63 reviewer mappings as active authority, and does not claim Ready; implementation may start only after a later review returns Ready for this exact revision.",
  "author": "Codex",
  "source_idea": "Implement Proposal 083: Execution-Truth Ownership and Invariant Model.",
  "canonical_proposal_path": "docs/proposals/083-execution-truth-ownership-invariant-model.md",
  "source_review_pass_id": "proposal-review-P083-r64-refined-3da64326-97d0ecda-4b31-40f2-91c0-bf047d39e1f8",
  "review_basis": {
    "authoritative_backlog_review_pass_id": "proposal-review-P083-r64-refined-3da64326-97d0ecda-4b31-40f2-91c0-bf047d39e1f8",
    "authoritative_backlog_item_ids": [
      "SCORE-LIFT-API-P083-R64-001",
      "SCORE-LIFT-API-P083-R64-002",
      "SCORE-LIFT-API-P083-R64-003",
      "SCORE-LIFT-API-P083-R64-004",
      "SCORE-LIFT-API-P083-R64-005",
      "SCORE-LIFT-APPLE-P083-R64-001",
      "SCORE-LIFT-MACOS-P083-R64-001",
      "SCORE-LIFT-MACOS-P083-R64-002",
      "SCORE-LIFT-MACOS-P083-R64-003",
      "SCORE-LIFT-REL-P083-R64-001",
      "SCORE-LIFT-REL-P083-R64-002",
      "SCORE-LIFT-REL-P083-R64-003",
      "SCORE-LIFT-UI-P083-R64-001"
    ],
    "stale_material_policy": "Active sections cite only the current R64 score_lift_backlog item ids. Older R63 reviewer mappings, closure narratives, and self-summaries are not carried forward as active authority. Earlier-resolved contract surfaces remain in the proposal text only because they are still part of the active contract; their R63 backlog tags have been removed from addresses arrays.",
    "current_review_basis_summary": "The current R64 pass resolves three remaining API-contract blockers by (a) adding rollout_contract_shutdown_deadline_config_state and rollout_contract_command_lease_ttl_config_state to the GraphQL SDL, MCP JSON Schema, required-with-null rules, run_report and release_receipt parity, and lane-coverage fixtures of rollout_readback_api_parity_v1; (b) unifying rollback disposition under a single RollbackDispositionJSON opaque versioned shape across GraphQL and MCP with additionalProperties=false and a negative fixture for extra MCP properties; and (c) adding p083.set_enforcement_mode to metric_labels_contract_v1.bounded_label_domains.command and keeping all three metric mirrors byte-equal with a fixture proving both P083 lifecycle mutations use bounded command labels. It also carries ten R64 advisories: executable p083_007 DDL summary, explicit GraphQL date-time validation policy, named Swift projection snapshot comparator, applicationShouldTerminate budget mapping for bounded shutdown waves, NSWindow automatic-tabbing fixtures, NSMenuValidation policy when no lifecycle window is key, bounded shutdown queue_rank storage rule, identity_ambiguous recovery rule for requested intents with null shutdown_epoch, canonical normalized-key wording on the overflow latch acceptance criterion, and UI density/spacing tokens for compact disabled reason rows."
  },
  "active_readiness_narrative": {
    "active_backlog_item_count": 13,
    "blocking_backlog_item_count": 3,
    "advisory_backlog_item_count": 10,
    "addressed_backlog_item_count": 13,
    "addressed_blocking_item_count": 3,
    "addressed_advisory_item_count": 10,
    "unresolved_blocker_count": 0,
    "deferred_blocker_count": 0,
    "disputed_blocker_count": 0,
    "implementation_may_start": false,
    "implementation_may_start_after": "A subsequent review pass marks this exact revision Ready.",
    "single_authority_pointer": "reviewer_feedback_resolution maps each current R64 score_lift_backlog item to active contract sections."
  },
  "executive_summary": "P083 names durable storage as execution-truth authority for runs, stages, agents, approvals, artifacts, side effects, provider sessions, command idempotency, shutdown receipts, rollout state, and operator readback. This revision resolves the current R64 API-contract blockers by extending rollout_readback_api_parity_v1 to declare both reliability hard-limit config-state readback fields in GraphQL, MCP, run_report, and release_receipt lanes; collapsing rollback disposition onto one opaque RollbackDispositionJSON shape mirrored byte-equal across GraphQL and MCP with additionalProperties=false; and adding p083.set_enforcement_mode to the bounded command metric label domain so the lifecycle mutation can emit command_idempotency metrics under the same bounded contract as p083.rollback_execution. The revision also closes ten R64 advisories spanning executable p083_007 DDL, GraphQL date-time policy, a named Swift projection comparator, applicationShouldTerminate-aware bounded shutdown waves with stored queue_rank, NSWindow tabbing and menu-validation fixtures, an identity_ambiguous recovery rule for requested cancellation intents, canonical overflow latch acceptance wording, and UI density tokens.",
  "problem": [
    "Lifecycle state crosses GraphQL, MCP, SQLite rows, frozen workflow snapshots, stage and agent attempts, provider sessions, approvals, artifacts, side-effect receipts, reports, and SwiftUI projections.",
    "Without a strict ownership model, caller payloads, projections, provider transcripts, or filesystem scans can be mistaken for durable execution truth.",
    "Retry, cancel, shutdown, and rollback paths need idempotency and receipt constraints that survive crashes and SQLite uniqueness rules.",
    "The macOS shell must present lifecycle truth without becoming a write authority or depending on SwiftUI scene callbacks that run too late for pre-presentation invariants.",
    "Rollout state must be machine-evaluable, observable, reversible, and auditable before enforcement mode can be enabled."
  ],
  "goals": [
    "Define one authoritative durable record for every execution-truth identifier.",
    "Classify caller-supplied identifiers as authority, selector, diagnostic, service_owned, or forbidden.",
    "Require lifecycle mutations to carry a CallerRequestId and execute through durable idempotency rows.",
    "Publish executable contracts for GraphQL, MCP JSON Schema, SQLite migrations, artifact lineage, metrics, recovery readback, shutdown, late output, and Swift projection mapping.",
    "Keep the macOS app read-only for P083 lifecycle enforcement while providing accurate readback and safe copy/export affordances.",
    "Include a strict inline rollout_contract_v1 with gate aliases, migrations, metrics, readback, hold conditions, rollback disposition, and negative fixtures."
  ],
  "non_goals": [
    "Do not add authentication, RBAC, token rotation, credential prompts, or Keychain behavior beyond checking existing principal-class helpers.",
    "Do not change workflow YAML or agent catalog YAML semantics or require new YAML keys.",
    "Do not remove historical artifacts, transcripts, or failed-attempt evidence.",
    "Do not make SwiftUI, GraphQL payloads, MCP payloads, provider transcripts, or filesystem scans authoritative for execution truth.",
    "Do not add a native macOS write path for side_effects.force_reconcile in P083.",
    "Do not introduce destructive migrations or backfill that rewrites historical run evidence.",
    "Do not expand Goose compatibility; ACP remains the canonical transport path."
  ],
  "target_users_and_trigger": {
    "primary_user": "Chainworks Forge operator running long-lived agent workflows from the macOS app.",
    "implementation_user": "Engine, API, persistence, projection, and UI engineers changing lifecycle state or readback.",
    "trigger": "Review churn around provenance drift, stale identifiers, duplicate commands, inactive approvals, external side effects, provider shutdown, and rollout enforcement."
  },
  "ux_ui_notes": {
    "truth_readback": "SwiftUI renders backend readback as read-only truth. Mutation affordances are disabled unless backend actionability is true and projection_integrity is fresh.",
    "typed_denials": "Typed denials render inline beside the affected run, stage, approval, artifact, side-effect, or provider-session row. Unknown denial codes render a generic validation message and no optimistic action.",
    "historical_evidence": "Active artifacts appear first. Historical Evidence is collapsed by default and labels rows Superseded, Failed, Cancelled, or Quarantined without active-transition controls.",
    "copy_controls": "Copy controls use a native NSButton bridge, expose the local-clipboard disclosure, never include secrets, and generate presentation-only CopyableCommandRequestId values that cannot be promoted into lifecycle request ids.",
    "export_controls": "Export Text writes to a user-selected file through NSSavePanel. It does not write to NSPasteboard unless the operator chooses a separate Copy Export Text action that follows the same current-host-only pasteboard path as Copy.",
    "accessibility": "All banners and disabled controls keep stable focus targets, wrap at accessibility text sizes, and use minHeight/fixedSize behavior instead of fixed text heights."
  },
  "ownership_model": {
    "rule": "Every lifecycle identifier has exactly one authoritative durable record. Callers may provide authority or selector ids only where the ownership matrix permits them; service-owned identifiers are never accepted from caller payload as truth.",
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
    ],
    "transaction_rule": "For mutating lifecycle commands, request acquisition, authoritative row reload, lifecycle CAS, side-effect receipt write, and terminal command outcome commit in one SQLite transaction unless the contract explicitly defines an earlier denial path."
  },
  "architecture": {
    "rust_control_plane_modules_touched": [
      "control-plane/crates/domain: nominal ids, denial codes, ProjectionIntegrity compatibility structs, lifecycle vocabulary enums",
      "control-plane/crates/db: additive migrations for artifact_lineage.report_kind, command idempotency generations, shutdown receipts, overflow latch rows, enforcement mode state, rollback audit rows",
      "control-plane/crates/engine: idempotent command execution, recovery readback, shutdown state machine, late-output caps, enforcement preflight",
      "control-plane/crates/graphql-server: versioned projection-integrity fields, cutover and rollback mutations, readback fields",
      "control-plane/crates/mcp-server: matching MCP schemas/tools, rollout readback, rollback tool, bounded metrics",
      "control-plane/crates/workflow: RunPlan compatibility validation including xhigh effort values"
    ],
    "swift_modules_touched": [
      "Chainworks Forge/AppLifecycle: app-owned lifecycle window coordinator and pre-order presentation hooks",
      "Chainworks Forge/Projection: RunProjectionSnapshotStore and field mapping manifest validation",
      "Chainworks Forge/CopyControls: CopyButtonRepresentable and current-host-only pasteboard writer",
      "Chainworks Forge/RequestIds: distinct LifecycleRequestId and CopyableCommandRequestId nominal types",
      "Chainworks Forge/Accessibility: denial clusters, projection badges, no-clipping layout fixtures"
    ],
    "data_authority_rule": "SQLite rows are authoritative. GraphQL, MCP, filesystem artifacts, report JSON, and SwiftData projections are readback or evidence surfaces only.",
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
    "graphql_projection_integrity_compatibility_v1": {
      "addresses": [
        "SCORE-LIFT-API-P083-R64-002"
      ],
      "rule": "The old enum field and the new extensible field are separate fields. The old field type never changes, so existing enum clients continue to validate. Future values are exposed only through the v2 object field and degrade to UNKNOWN on the legacy field.",
      "concrete_sdl": [
        "enum ProjectionIntegrity { FRESH STALE MISSING UNKNOWN }",
        "type ProjectionIntegrityValue { schemaVersion: String! value: String! knownV1Value: ProjectionIntegrity! actionable: Boolean! }",
        "interface HasProjectionIntegrity { projectionIntegrity: ProjectionIntegrity! projectionIntegrityV2: ProjectionIntegrityValue! }",
        "type RunProjection implements HasProjectionIntegrity { projectionIntegrity: ProjectionIntegrity! projectionIntegrityV2: ProjectionIntegrityValue! }"
      ],
      "legacy_mapping": {
        "fresh": "FRESH",
        "stale": "STALE",
        "missing": "MISSING",
        "unknown": "UNKNOWN",
        "tampered": "UNKNOWN",
        "future_value": "UNKNOWN"
      },
      "future_value_rule": "Adding a new value updates only ProjectionIntegrityValue.value registry and schemaVersion. ProjectionIntegrity enum remains closed for v1 clients.",
      "fixtures": [
        "docs/evidence/083/graphql/projection-integrity-v1-client-validates.fixture.json",
        "docs/evidence/083/graphql/projection-integrity-v2-client-reads-raw-value.fixture.json",
        "docs/evidence/083/graphql/projection-integrity-future-value-degrades-to-unknown.fixture.json",
        "docs/evidence/083/graphql/projection-integrity-same-field-type-change-rejected.fixture.json"
      ],
      "tampered_compatibility_rule": "P083-r61 treats the legacy projectionIntegrity enum as closed and single-sourced: FRESH, STALE, MISSING, UNKNOWN. Stored tampered is never emitted on v1 unless a deployed-SDL evidence fixture proves TAMPERED pre-existed P083; absent that evidence, tampered is exposed only through projectionIntegrityV2.value=\"tampered\" with knownV1Value=UNKNOWN.",
      "legacy_enum_values": [
        "FRESH",
        "STALE",
        "MISSING",
        "UNKNOWN"
      ]
    },
    "mcp_schema_rule": "MCP result schemas expose projection_integrity_v1 as the closed lowercase enum and projection_integrity_v2 as an object with schema_version, value, known_v1_value, and actionable. The v1 field follows the same fallback as GraphQL."
  },
  "run_plan_compatibility_schema_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-002"
    ],
    "schema_version": "run_plan_compatibility_schema_v1",
    "effort_values": [
      "low",
      "medium",
      "high",
      "xhigh"
    ],
    "mapping_rule": "No compile-time lossy mapping is allowed. Rust and Swift compilers preserve xhigh byte-for-byte in frozen RunPlanSnapshot serialization.",
    "loop_rule": "loop is required on every compiled state and is either null or an object with required counter and max fields.",
    "null_present_optional_rule": "Optional extension fields are required with null value when absent semantically; omission fails canonical serialization validation.",
    "fixtures": [
      "docs/evidence/083/runplan/effort-xhigh-positive.fixture.json",
      "docs/evidence/083/runplan/effort-unknown-rejected.fixture.json",
      "docs/evidence/083/runplan/swift-rust-xhigh-byte-equal.fixture.json",
      "docs/evidence/083/runplan/loop-null-present-positive.fixture.json",
      "docs/evidence/083/runplan/loop-required-omission-negative.fixture.json",
      "docs/evidence/083/runplan/historical-missing-loop-normalized.fixture.json",
      "docs/evidence/083/runplan/historical-required-field-missing-blocks-resume.fixture.json"
    ],
    "historical_read_compatibility": "Frozen RunPlanSnapshot records with snapshot_version before P083 may omit loop or null-present optional extension fields. Readback normalizes missing loop to null and missing optional extension fields to null for comparison only; the original snapshot bytes remain unchanged. Resume blocks as schema_incompatible only when required semantic fields such as state owner, run tasks, transitions, agent provider, model, effort, or prompt are absent or invalid.",
    "snapshot_version_handshake": "Compiled snapshots written after P083 carry snapshot_schema_version=\"run_plan_compatibility_schema_v1\" and capabilities=[\"effort_xhigh\",\"loop_null_present\",\"optional_extensions_null_present\"]. Readers accept historical snapshots without capabilities through historical_read_compatibility but write only the new canonical form."
  },
  "artifact_lineage_contract_v1": {
    "addresses": [],
    "schema_version": "artifact_lineage_v1",
    "required_fields": [
      "artifact_id",
      "run_id",
      "logical_name",
      "artifact_role",
      "report_kind",
      "artifact_path",
      "content_hash",
      "active",
      "created_at",
      "projection_integrity"
    ],
    "report_kind": {
      "type": "string_or_null",
      "required_when": "artifact_role == 'report'",
      "allowed_values": [
        "proposal_current",
        "proposal_revision_summary",
        "proposal_feedback_coverage",
        "review_summary",
        "run_report",
        "release_receipt",
        "evidence_pack"
      ],
      "null_when": "artifact_role != 'report'"
    },
    "active_report_uniqueness": "CREATE UNIQUE INDEX artifact_lineage_active_report_kind_uniq ON artifact_lineage(run_id, report_kind) WHERE active = 1 AND artifact_role = 'report' AND report_kind IS NOT NULL;",
    "fixtures": [
      "docs/evidence/083/artifact-lineage/report-kind-required-positive.fixture.json",
      "docs/evidence/083/artifact-lineage/report-kind-missing-rejected.fixture.json",
      "docs/evidence/083/artifact-lineage/active-report-kind-duplicate-rejected.fixture.json",
      "docs/evidence/083/artifact-lineage/non-report-report-kind-null-positive.fixture.json"
    ],
    "report_kind_historical_exception": "report_kind is required for every new report write and every active report row after P083 migration. Historical inactive report rows may keep report_kind null only when no safe derivation exists; they are non-actionable and excluded from active report-kind uniqueness.",
    "new_write_validation": "Application validation rejects artifact_role=report writes with null/unknown report_kind before insert. Migration also installs a CHECK or trigger equivalent where SQLite expression support permits: report_kind IS NOT NULL when artifact_role=report AND active=1.",
    "enforcement_ddl": "CREATE TRIGGER artifact_lineage_report_kind_required BEFORE INSERT ON artifact_lineage WHEN NEW.artifact_role = 'report' AND NEW.active = 1 AND (NEW.report_kind IS NULL OR NEW.report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack')) BEGIN SELECT RAISE(ABORT, 'artifact_lineage.report_kind required for active report'); END;",
    "verification_query": "SELECT artifact_id FROM artifact_lineage WHERE artifact_role = 'report' AND active = 1 AND (report_kind IS NULL OR report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack'));",
    "insert_update_enforcement_ddl": [
      "CREATE TRIGGER artifact_lineage_report_kind_required_insert BEFORE INSERT ON artifact_lineage WHEN NEW.artifact_role = 'report' AND NEW.active = 1 AND (NEW.report_kind IS NULL OR NEW.report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack')) BEGIN SELECT RAISE(ABORT, 'artifact_lineage.report_kind required for active report'); END;",
      "CREATE TRIGGER artifact_lineage_report_kind_required_update BEFORE UPDATE OF active, report_kind, artifact_role ON artifact_lineage WHEN NEW.artifact_role = 'report' AND NEW.active = 1 AND (NEW.report_kind IS NULL OR NEW.report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack')) BEGIN SELECT RAISE(ABORT, 'artifact_lineage.report_kind required for active report'); END;",
      "CREATE UNIQUE INDEX artifact_lineage_active_report_kind_uniq ON artifact_lineage(run_id, report_kind) WHERE active = 1 AND artifact_role = 'report' AND report_kind IS NOT NULL;"
    ],
    "negative_fixtures": [
      "docs/evidence/083/artifact-lineage/update-to-active-null-report-kind-rejected.fixture.json",
      "docs/evidence/083/artifact-lineage/update-to-unknown-report-kind-rejected.fixture.json",
      "docs/evidence/083/artifact-lineage/duplicate-active-null-report-kind-rejected.fixture.json"
    ],
    "canonical_sql_rule": "Every proposal, migration, fixture, and evidence snippet uses exactly active_report_uniqueness for artifact_lineage_active_report_kind_uniq; alternate SQL text is treated as drift."
  },
  "metric_labels_contract_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-003"
    ],
    "schema_version": "metric_labels_contract_v1",
    "authority_rule": "This section is the source metric inventory and owns bounded label domains. rollout_contract_v1.metrics carries only the rollout-template metric fields while referencing this section as the metric authority; feature contracts may reference metric names here but may not declare independent label vocabularies.",
    "bounded_label_domains": {
      "surface": [
        "graphql",
        "mcp",
        "run_report",
        "release_receipt",
        "swift_ui"
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
      "outcome": [
        "acquired",
        "replayed",
        "denied",
        "committed",
        "failed",
        "abandoned",
        "expired_reacquired"
      ],
      "provider": [
        "codex",
        "claude",
        "gemini",
        "auggie",
        "junie"
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
        "tamper_detected"
      ],
      "reason": [
        "auth_dependency_missing",
        "hold_condition_present",
        "projection_not_fresh",
        "migration_not_applied",
        "rollback_ttl_expired",
        "gate_failed"
      ],
      "action": [
        "disable_to_permissive",
        "permissive_to_enforce",
        "enforce_to_permissive",
        "rollback_disable",
        "reenable_after_rollback"
      ],
      "receipt_state": [
        "created",
        "reused",
        "duplicate_denied"
      ],
      "debug_state": [
        "default_off",
        "privileged_enabled",
        "emitted",
        "denied"
      ],
      "state": [
        "fresh",
        "stale",
        "missing",
        "unknown",
        "tampered"
      ],
      "proposal_id": [
        "P083"
      ],
      "shutdown_interrupted_state": [
        "grace_deadline_expired",
        "kill_signal_issued",
        "kill_pid_exit_observed",
        "queued_no_signal",
        "shutdown_interrupted"
      ],
      "signal_kind": [
        "graceful",
        "kill"
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
      "cancellation_reason": [
        "operator_cancel",
        "backpressure_cutoff",
        "shutdown_recovery"
      ]
    },
    "operational_metric_label_signatures": [
      "artifact_lineage_projection_integrity_total{surface,state}",
      "provider_session_legacy_id_read_total{surface}",
      "provider_session_lifecycle_total{provider,lifecycle_state}",
      "command_idempotency_lease_acquire_total{command,outcome}",
      "command_idempotency_replay_total{command,outcome}",
      "command_idempotency_reacquire_total{command,outcome}",
      "command_idempotency_intent_duplicate_total{command,outcome}",
      "command_idempotency_mismatch_denial_total{command}",
      "shutdown_interrupted_receipt_total{provider,receipt_state}",
      "shutdown_duplicate_signal_suppressed_total{provider}",
      "cancel_late_output_overflow_total{provider,scope,overflow_kind}",
      "cancel_late_output_dropped_total{provider,scope,overflow_kind}",
      "rollout_contract_lint_total{proposal_id,status,failure_reason}",
      "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}",
      "p083_enforcement_mode_transition_total{transition,enforcement_mode}",
      "p083_rollback_execution_total{action,status,reason}",
      "p083_debug_metric_total{debug_state}",
      "shutdown_interrupted_state_total{provider,shutdown_interrupted_state}",
      "shutdown_signal_side_effect_total{provider,signal_kind,intent_state}",
      "backpressure_cutoff_total{provider,overflow_kind}",
      "backpressure_process_fate_total{provider,process_fate}",
      "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}"
    ],
    "adoption_metric": {
      "name": "p083_applicable_runs_with_passing_execution_truth_preflight_percent",
      "numerator": "runs where the P083 preflight for the active proposal revision passed in the last scrape window",
      "denominator": "runs where P083 is applicable and not explicitly waived in the last scrape window",
      "scrape_interval_seconds": 60,
      "staleness_after_seconds": 180,
      "waived_missing_or_tamper_detected_handling": "excluded from numerator; included in denominator unless the waiver is valid and unexpired"
    },
    "fixtures": [
      "docs/evidence/083/metrics/metric-inventory-single-source-positive.fixture.json",
      "docs/evidence/083/metrics/local-feature-metric-declaration-rejected.fixture.json",
      "docs/evidence/083/metrics/unbounded-label-rejected.fixture.json",
      "docs/evidence/083/metrics/debug-metrics-default-off.fixture.json",
      "docs/evidence/083/metrics/debug-metrics-privileged-enable.fixture.json",
      "docs/evidence/083/metrics/p083-lifecycle-mutations-bounded-command-labels.fixture.json",
      "docs/evidence/083/metrics/p083-set-enforcement-mode-idempotency-metrics.fixture.json"
    ],
    "process_fate_metric_rule": "backpressure_cutoff_shutdown_pending appears only in process_fate labels, never lifecycle_state labels. cancellation_requested appears only as provider_cancellation_intents intent/readback state, never provider lifecycle state.",
    "single_source_export_rule": "operational_metric_label_signatures is the only source metric inventory. metrics.operational_metrics_reference and rollout_contract_v1.metrics.operational_metrics are generated mirrors and must be byte-equal; no feature section may declare an extra metric list.",
    "lifecycle_mutation_bounded_label_rule": "Every CallerRequestId lifecycle mutation that participates in command_idempotency_* metrics has a bounded entry in bounded_label_domains.command. P083 lifecycle mutations p083.rollback_execution and p083.set_enforcement_mode are both bounded command labels; emitting command_idempotency_lease_acquire_total, command_idempotency_replay_total, command_idempotency_reacquire_total, command_idempotency_intent_duplicate_total, or command_idempotency_mismatch_denial_total with command=p083.set_enforcement_mode passes the metric_unbounded lint and the lifecycle mutation idempotency fixture below.",
    "metrics_release_receipt_lane_rule": "release_receipt is an authoritative readback lane for rollout state. Operational metrics themselves are scraped from the daemon; release_receipt content reflects the same readback shape declared by rollout_readback_api_parity_v1."
  },
  "swift_app_projection_contract_v1": {
    "presentation_boundary": {
      "addresses": [
        "SCORE-LIFT-MACOS-P083-R64-002"
      ],
      "rule": "Lifecycle-bearing windows are created and ordered only through an app-owned LifecycleWindowCoordinator installed from NSApplicationDelegate.applicationWillFinishLaunching. SwiftUI WindowGroup roots may host content but are not presentation gates for lifecycle-bearing windows.",
      "pre_order_sequence": [
        "create NSWindow without ordering",
        "attach NSHostingController content",
        "run pasteboard/copy-state preflight hooks without mutating unrelated clipboard contents",
        "register projection store and teardown observer",
        "call makeKeyAndOrderFront from LifecycleWindowCoordinator"
      ],
      "restoration_rule": "State restoration returns an un-ordered lifecycle window token to LifecycleWindowCoordinator; the same pre_order_sequence runs before ordering.",
      "fixtures": [
        "docs/evidence/083/swift/lifecycle-window-coordinator-preorders-cold-start.fixture.json",
        "docs/evidence/083/swift/windowgroup-onappear-not-presentation-gate.fixture.json",
        "docs/evidence/083/swift/restoration-preorders-through-coordinator.fixture.json",
        "docs/evidence/083/swift/settings-first-window-not-lifecycle-bearing.fixture.json"
      ]
    },
    "teardown_contract": {
      "addresses": [
        "SCORE-LIFT-MACOS-P083-R64-002"
      ],
      "rule": "Production teardown is notification-first and does not replace existing NSWindowDelegate instances. WindowTeardownRegistry is @MainActor, observes NSWindow.willCloseNotification and NSApplication.willTerminateNotification, and suppresses duplicates by lifecycle_window_id.",
      "objective_c_forwarder_scope": "No generic Swift forwarding is used. If a future lifecycle window must own a delegate, it must use a small Objective-C CWWindowDelegateForwarder with an explicit selector allowlist and fixtures before entering scope.",
      "fixtures": [
        "docs/evidence/083/swift/willclose-notification-exactly-once.fixture.json",
        "docs/evidence/083/swift/upstream-delegate-not-replaced.fixture.json",
        "docs/evidence/083/swift/teardown-registry-mainactor.fixture.json",
        "docs/evidence/083/swift/application-terminate-final-teardown.fixture.json",
        "docs/evidence/083/swift/objective-c-forwarder-selector-allowlist-negative.fixture.json"
      ]
    },
    "pasteboard_contract": {
      "addresses": [
        "SCORE-LIFT-MACOS-P083-R64-002"
      ],
      "copy_mechanism": "Copy uses NSPasteboard.general.prepareForNewContents(with: [.currentHostOnly]) followed by writing NSString pasteboard items. The concrete Universal Clipboard opt-out is NSPasteboard.ContentsOptions.currentHostOnly.",
      "launch_rule": "P083 does not clear NSPasteboard.general at launch and does not destroy unrelated user clipboard contents. Clipboard mutation happens only after an explicit operator Copy action.",
      "export_rule": "Export Text writes to a file selected by NSSavePanel. It is not a pasteboard operation. Copy Export Text is a distinct command and uses the same current-host-only pasteboard path as Copy.",
      "change_count_rule": "Copy fixtures assert pasteboard changeCount advances after prepareForNewContents/writeObjects. There is no pre-window global changeCount requirement.",
      "fixtures": [
        "docs/evidence/083/swift/copy-uses-current-host-only.fixture.json",
        "docs/evidence/083/swift/export-text-uses-save-panel-not-pasteboard.fixture.json",
        "docs/evidence/083/swift/copy-export-text-current-host-only.fixture.json",
        "docs/evidence/083/swift/launch-does-not-clear-user-clipboard.fixture.json",
        "docs/evidence/083/swift/pasteboard-changecount-advances-on-copy.fixture.json"
      ]
    },
    "projection_mapping_and_accessibility": {
      "addresses": [
        "SCORE-LIFT-UI-P083-R64-001"
      ],
      "mapping_rule": "Every Swift field read by P083 projection views appears in swift_projection_mapping_manifest_v1 or is explicitly marked app_local_non_authoritative.",
      "layout_rule": "P083 views use wrapping Text, minHeight, and fixedSize(horizontal:false, vertical:true); fixed text heights are rejected.",
      "keyboard_and_focus_fixtures": [
        "docs/evidence/083/swift/focus-keyboard-cmdw-settings-window.fixture.json",
        "docs/evidence/083/swift/accessibility-size-no-clipping.fixture.json"
      ]
    },
    "projection_concurrency_contract": {
      "addresses": [
        "SCORE-LIFT-APPLE-P083-R64-001"
      ],
      "rule": "Swift projection view models and snapshot publication are @MainActor. GraphQL/MCP network I/O and JSON decoding run off-main, then cross through explicit MainActor hops with lifecycle_window_id plus run_id subscription identity. A window close cancels only matching subscription keys.",
      "monotonic_snapshot_rule": "Every backend projection snapshot carries lifecycle_window_id, run_id, backend_revision, observed_at, and journal_cursor. The MainActor store tracks the latest tuple per lifecycle_window_id/run_id and drops any frame whose backend_revision or journal_cursor is older than the published value.",
      "stale_frame_fixtures": [
        "docs/evidence/083/swift/stale-projection-frame-dropped.fixture.json",
        "docs/evidence/083/swift/out-of-order-journal-cursor-dropped.fixture.json",
        "docs/evidence/083/swift/newer-observed-at-same-cursor-no-regression.fixture.json"
      ],
      "named_snapshot_comparator": {
        "name": "P083ProjectionSnapshotOrdering",
        "swift_signature": "static func compare(_ lhs: P083ProjectionSnapshot, _ rhs: P083ProjectionSnapshot) -> ComparisonResult",
        "total_order_rule": "Snapshots compare strictly by (backend_revision, journal_cursor, observed_at) in that order. A later backend_revision strictly dominates; on equal backend_revision, a later journal_cursor strictly dominates; on equal backend_revision and journal_cursor, a later observed_at strictly dominates. Two snapshots that are equal across all three components are treated as the same logical frame and the older Swift-side wall-clock arrival is retained; no field is allowed to act as a fourth tie-breaker.",
        "acceptance_rule": "Every MainActor projection store across all P083 views (RunsHomeView, lifecycle detail surfaces, approvals, side-effects readback, artifact lineage) calls P083ProjectionSnapshotOrdering.compare to decide whether to accept a new frame. Local ad-hoc comparators are rejected by the lint fixture below.",
        "fixtures": [
          "docs/evidence/083/swift/projection-snapshot-comparator-named.fixture.json",
          "docs/evidence/083/swift/projection-snapshot-comparator-total-order.fixture.json",
          "docs/evidence/083/swift/projection-store-uses-named-comparator.fixture.json",
          "docs/evidence/083/swift/local-ad-hoc-comparator-rejected.fixture.json"
        ]
      }
    },
    "ui_pending_and_disabled_contract": {
      "addresses": [
        "SCORE-LIFT-UI-P083-R64-001"
      ],
      "pending_lease_rendering": "When a lifecycle command has a pending idempotency lease, the triggering control remains visible but disabled, shows an inline indeterminate ProgressView or native spinner inside the control group, and exposes accessibilityValue=\"Pending\". Duplicate triggers are disabled and route to the same request readback instead of issuing a new lifecycle request.",
      "disabled_reason_rendering": "Mutation controls disabled because projection_integrity != fresh show a compact status badge \"Projection stale\" plus a tooltip/popover explaining that the action is unavailable until readback refreshes. Controls disabled because backend actionability is false show the backend denial reason or \"Action unavailable for current state\" when the reason is unknown. Disabled captions are focusable static text targets for Full Keyboard Access and VoiceOver.",
      "historical_evidence_empty_state": "Historical Evidence renders a collapsed section with \"No historical evidence\" when empty, rather than disappearing.",
      "generic_validation_message": "Unknown denial codes render inline with system error icon, red/secondary hierarchy matching severity, escaped plain text, and no optimistic action.",
      "local_clipboard_disclosure": "Copy controls expose current-host-only clipboard behavior through a hover/focus tooltip and VoiceOver hint, not persistent instructional body text.",
      "fixtures": [
        "docs/evidence/083/ui/pending-lease-spinner-disabled-trigger.fixture.json",
        "docs/evidence/083/ui/projection-stale-disabled-reason.fixture.json",
        "docs/evidence/083/ui/actionability-false-disabled-reason.fixture.json",
        "docs/evidence/083/ui/historical-evidence-empty-state.fixture.json",
        "docs/evidence/083/ui/unknown-denial-inline-error.fixture.json",
        "docs/evidence/083/ui/focused-window-command-routing.fixture.json",
        "docs/evidence/083/ui/sheet-modal-export.fixture.json",
        "docs/evidence/083/ui/pending-terminal-accessibility-announcement.fixture.json",
        "docs/evidence/083/ui/current-host-only-clipboard-wording.fixture.json",
        "docs/evidence/083/ui/semantic-color-token-usage.fixture.json",
        "docs/evidence/083/ui/no-evidence-disabled-styling.fixture.json",
        "docs/evidence/083/ui/initial-projection-loading-state.fixture.json",
        "docs/evidence/083/ui/remediation-popover-focus-return.fixture.json",
        "docs/evidence/083/ui/inline-spinner-baseline-alignment.fixture.json",
        "docs/evidence/083/ui/remediation-popover-max-width.fixture.json",
        "docs/evidence/083/ui/compact-disabled-reason-row-spacing-tokens.fixture.json",
        "docs/evidence/083/ui/inline-spinner-disabled-label-gap-token.fixture.json",
        "docs/evidence/083/ui/raw-point-literal-rejected.fixture.json"
      ],
      "disabled_reason_standard": "Use a focusable inline compact reason row as the persistent standard and a tooltip for the same text on hover/focus. Popovers are reserved for multi-line remediation details from backend next_step_code, not for ordinary disabled reasons.",
      "pending_spinner_placement": "Spinner appears inside the command control group immediately leading the disabled trigger label; it does not resize the toolbar or move adjacent controls.",
      "remediation_popover_trigger": "Multi-line remediation details are opened only by an explicit info button adjacent to the compact disabled reason row. Hover/focus tooltip repeats the compact reason and never contains the full remediation body.",
      "primary_empty_states": {
        "active_runs": "No active runs",
        "stage_executions": "No stages started",
        "approvals": "No approvals pending",
        "artifacts": "No artifacts yet",
        "side_effects": "No side effects recorded",
        "historical_evidence": "No historical evidence"
      },
      "long_denial_text_rule": "Inline denial rows are capped to three lines with stable line-height and a More details control that opens the remediation popover. Full denial text remains selectable inside the popover and is escaped plain text.",
      "standard_menu_names": "Panel-opening command labels use standard macOS ellipsis form, including Export Text... .",
      "pending_terminal_accessibility_rule": "Pending-to-terminal transitions announce committed, replayed, denied, and failed outcomes without moving focus unless the focused control disappears.",
      "semantic_color_rule": "Disabled, pending, denied, failed, and no-evidence states use semantic color tokens from the app design system; hard-coded red/gray-only styling is rejected by fixture.",
      "no_evidence_disabled_styling_rule": "No-evidence states are styled as disabled-but-readable with a clear empty state and no hidden action affordance.",
      "initial_projection_loading_state": "Lifecycle detail surfaces render a native ProgressView with label Loading latest readback while the first projection is missing. Mutation controls are disabled until the first fresh actionable snapshot arrives.",
      "remediation_popover_behavior": "Popover max width is 420pt, dismisses on Esc, outside click, or action completion, and returns focus to the info button that opened it unless that control disappeared.",
      "inline_spinner_alignment": "Inline spinners align to first text baseline within the command control group and reserve stable width so pending state does not shift adjacent controls.",
      "spacing_and_density_tokens": {
        "compact_disabled_reason_row_density": "compact (28pt minHeight, 6pt vertical inset, 10pt horizontal inset)",
        "compact_disabled_reason_row_text_style": "footnote with secondaryLabelColor and minimum tap target of 28pt",
        "compact_disabled_reason_row_icon_to_text_gap_pt": 6,
        "compact_disabled_reason_row_between_rows_gap_pt": 4,
        "inline_spinner_to_disabled_trigger_label_gap_pt": 6,
        "inline_spinner_size": "small (16x16pt) ProgressView, leading the label baseline",
        "info_button_to_compact_reason_row_gap_pt": 8,
        "remediation_popover_padding_pt": 16,
        "design_token_source": "AppDesignSystem.Spacing.{xs=4,sm=6,md=8,lg=12,xl=16} and AppDesignSystem.Density.compactRow; raw point literals other than these tokens are rejected by fixture"
      }
    },
    "copy_redaction_contract": {
      "addresses": [
        "SCORE-LIFT-MACOS-P083-R64-002"
      ],
      "rule": "Copied strings are presentation-only. Copy output excludes secrets, bearer-token-like strings, lifecycle CallerRequestId values, reusable command identifiers, provider transcripts, raw evidence_uri values, and principal identifiers. CopyableCommandRequestId cannot parse as CallerRequestId and is never accepted by mutation schemas.",
      "fixtures": [
        "docs/evidence/083/swift/copy-redacts-secrets-and-request-ids.fixture.json",
        "docs/evidence/083/swift/copyable-id-not-caller-request-id.fixture.json",
        "docs/evidence/083/swift/provider-transcript-not-copied.fixture.json"
      ]
    }
  },
  "swift_projection_mapping_manifest_v1": {
    "addresses": [],
    "schema_version": "swift_projection_mapping_manifest_v1",
    "models_in_scope": [
      "RunProjectionRecord",
      "StageExecutionProjection",
      "AgentExecutionProjection",
      "ApprovalProjection",
      "ArtifactLineageProjection",
      "SideEffectProjection",
      "ProviderSessionProjection",
      "RolloutReadbackProjection"
    ],
    "manifest_rule": "Each field entry lists swift_field, swift_type, rust_readback_source, rust_field, transform, nullable, default, and authority_class. Missing or extra read fields fail the proposal-083 gate.",
    "excluded_app_local_models": [
      "DraftNote",
      "UIPreferenceProfile",
      "OperatorBookmark",
      "WindowFrameMemo",
      "AnnotationDraft",
      "OperatorChecklistEntry"
    ],
    "fixtures": [
      "docs/evidence/083/swift/projection-manifest-complete.fixture.json",
      "docs/evidence/083/swift/projection-manifest-extra-field-rejected.fixture.json",
      "docs/evidence/083/swift/projection-manifest-nullability-mismatch-rejected.fixture.json"
    ]
  },
  "command_idempotency_pending_lease_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-001"
    ],
    "schema_version": "command_idempotency_pending_lease_v1",
    "tables": {
      "command_idempotency": [
        "command_record_id TEXT PRIMARY KEY",
        "principal_id TEXT NOT NULL",
        "request_id TEXT NOT NULL",
        "lease_generation INTEGER NOT NULL",
        "command TEXT NOT NULL",
        "canonical_intent_hash TEXT NOT NULL",
        "lease_state TEXT NOT NULL CHECK(lease_state IN ('pending','committed','failed','abandoned'))",
        "expires_at_monotonic_ms INTEGER NOT NULL",
        "outcome_payload_blob BLOB",
        "created_at TEXT NOT NULL",
        "updated_at TEXT NOT NULL"
      ],
      "command_request_aliases": [
        "principal_id TEXT NOT NULL",
        "request_id TEXT NOT NULL",
        "command_record_id TEXT NOT NULL",
        "alias_reason TEXT NOT NULL",
        "created_at TEXT NOT NULL",
        "PRIMARY KEY(principal_id, request_id)"
      ]
    },
    "indexes": [
      "CREATE UNIQUE INDEX command_request_active_uniq ON command_idempotency(principal_id, request_id) WHERE lease_state IN ('pending','committed','failed');",
      "CREATE UNIQUE INDEX command_intent_active_uniq ON command_idempotency(principal_id, command, canonical_intent_hash) WHERE lease_state IN ('pending','committed');",
      "CREATE UNIQUE INDEX command_generation_uniq ON command_idempotency(principal_id, request_id, lease_generation);"
    ],
    "reacquire_rule": "Expired pending rows are updated in place to abandoned with lease_generation retained, then a new pending row is inserted with lease_generation = previous + 1. The partial unique index permits the insert because abandoned rows are outside the active request uniqueness set.",
    "same_intent_new_request_behavior": {
      "pending": "Return command_intent_already_pending denial and do not mutate lifecycle state.",
      "committed": "Insert command_request_aliases row for the new request_id and replay the committed outcome without re-executing the command.",
      "failed": "Allow a new request_id to acquire a new pending row because failed rows are outside command_intent_active_uniq; same request_id still replays the failed outcome.",
      "abandoned": "Allow a new request_id or a reacquired generation to acquire a fresh pending row; abandoned history remains queryable."
    },
    "same_request_different_intent_behavior": "Return command_request_id_reuse_mismatch and do not write lifecycle state.",
    "clock_rule": "Lease expiry uses durable_monotonic_clock_v1 only; wall-clock timestamps are audit metadata and cannot expire a lease.",
    "fixtures": [
      "docs/evidence/083/idempotency/expiry-to-generation-reacquire.fixture.json",
      "docs/evidence/083/idempotency/same-intent-new-request-pending-denied.fixture.json",
      "docs/evidence/083/idempotency/same-intent-new-request-committed-alias-replay.fixture.json",
      "docs/evidence/083/idempotency/same-intent-new-request-failed-retry.fixture.json",
      "docs/evidence/083/idempotency/same-intent-new-request-abandoned-reacquire.fixture.json"
    ],
    "reacquire_transaction_v1": {
      "isolation": "BEGIN IMMEDIATE SQLite transaction around lookup, CAS update, insert, and conflict readback.",
      "steps": [
        "Load current row by principal_id, request_id ordered by lease_generation desc.",
        "If no row exists, insert pending generation=1 when canonical_intent_hash does not violate active intent uniqueness.",
        "If latest row is pending and expires_at_monotonic_ms > durable_now, return typed denial command_idempotency_pending.",
        "If latest row is pending and expired, UPDATE command_idempotency SET lease_state='abandoned', updated_at=:now WHERE command_record_id=:id AND lease_state='pending' AND lease_generation=:generation AND expires_at_monotonic_ms <= :durable_now.",
        "Require changed_rows=1 before inserting the new pending row with lease_generation = previous_generation + 1.",
        "Insert new pending row. On UNIQUE conflict, SELECT the winning active row and return loser_outcome.",
        "Commit only after authoritative lifecycle row reload succeeds or terminal denial is persisted."
      ],
      "loser_outcome": "Concurrent loser never sees a raw SQLite error. If the winner committed, replay committed outcome. If the winner is pending, return command_idempotency_pending with winning command_record_id and retry_after_seconds. If the winner failed, replay failed outcome for same request or allow new request per same_intent_new_request_behavior.",
      "fixtures": [
        "docs/evidence/083/idempotency/concurrent-expired-reacquire-single-winner.fixture.json",
        "docs/evidence/083/idempotency/reacquire-loser-typed-pending.fixture.json",
        "docs/evidence/083/idempotency/reacquire-conflict-readback-no-raw-sqlite-error.fixture.json"
      ]
    },
    "alias_first_lookup_rule": "Before acquiring a new row, load command_request_aliases by principal_id and request_id. If an alias exists, load the target command_idempotency row and compare incoming canonical_intent_hash and command. Replay only when both match; otherwise return REQUEST_REUSE_MISMATCH and write no lifecycle state. Alias lookup runs before pending lease acquisition and before same-intent duplicate handling."
  },
  "shutdown_contract_v1": {
    "addresses": [
      "SCORE-LIFT-REL-P083-R64-001"
    ],
    "schema_version": "shutdown_contract_v1",
    "states": [
      "not_required",
      "requested",
      "graceful_signal_issued",
      "grace_deadline_expired",
      "kill_signal_issued",
      "kill_pid_exit_observed",
      "self_exit_observed",
      "orphan_settled",
      "finalized",
      "queued_no_signal",
      "shutdown_interrupted"
    ],
    "receipt_table": {
      "name": "shutdown_interrupted_receipts",
      "columns": [
        "receipt_id TEXT PRIMARY KEY",
        "provider_session_id TEXT NOT NULL",
        "shutdown_epoch INTEGER NOT NULL",
        "receipt_generation INTEGER NOT NULL",
        "interrupted_state TEXT NOT NULL",
        "created_at TEXT NOT NULL",
        "recovered_at TEXT",
        "final_readback_rank INTEGER NOT NULL"
      ],
      "constraints": [
        "UNIQUE(provider_session_id, shutdown_epoch, receipt_generation)",
        "CHECK(interrupted_state IN ('grace_deadline_expired','kill_signal_issued','kill_pid_exit_observed','queued_no_signal','shutdown_interrupted'))",
        "No UNIQUE(provider_session_id). No per-session receipt reuse. Duplicate writer reuse is valid only for the same provider_session_id, shutdown_epoch, and receipt_generation."
      ]
    },
    "recovery_readback_rule": "Recovery readback never selects or reuses a shutdown receipt without the complete tuple (provider_session_id, shutdown_epoch, receipt_generation). Duplicate recovery first computes that tuple from durable shutdown_signal_side_effects plus receipt history, then SELECTs by the complete identity; if the exact row exists, it reuses that receipt_id, otherwise it inserts a new immutable receipt for the computed epoch and generation.",
    "deadline_accounting": "Per-host shutdown deadline is split into graceful and kill-observation waves. Unused graceful budget carries forward to kill observation; queued sessions receive queued_no_signal receipts only after the host deadline is exhausted.",
    "fixtures": [
      "docs/evidence/083/shutdown/duplicate-receipt-after-restart-reused.fixture.json",
      "docs/evidence/083/shutdown/deadline-wave-accounting.fixture.json",
      "docs/evidence/083/shutdown/queued-no-signal-vocabulary-positive.fixture.json",
      "docs/evidence/083/shutdown/queued-no-signal-restart-live-process-retries.fixture.json",
      "docs/evidence/083/shutdown/undeclared-interrupted-state-rejected.fixture.json",
      "docs/evidence/083/shutdown/per-epoch-receipt-history.fixture.json",
      "docs/evidence/083/shutdown/queued-no-signal-then-retry-epoch-interrupted.fixture.json",
      "docs/evidence/083/shutdown/duplicate-writer-reuses-receipt.fixture.json",
      "docs/evidence/083/shutdown/bounded-receipt-history-page.fixture.json",
      "docs/evidence/083/shutdown/duplicate-same-epoch-generation-reuses-receipt.fixture.json",
      "docs/evidence/083/shutdown/no-provider-session-only-reuse.fixture.json",
      "docs/evidence/083/shutdown/per-epoch-history-latest-derived.fixture.json",
      "docs/evidence/083/shutdown/provider-session-only-receipt-lookup-rejected.fixture.json",
      "docs/evidence/083/shutdown/recovery-reuses-only-same-epoch-generation.fixture.json"
    ],
    "interrupted_state_vocabulary": [
      "grace_deadline_expired",
      "kill_signal_issued",
      "kill_pid_exit_observed",
      "queued_no_signal",
      "shutdown_interrupted"
    ],
    "queued_no_signal_recovery_rule": "queued_no_signal opens a new shutdown_epoch only after process identity is rechecked. If process is absent, recovery writes a later shutdown_interrupted receipt for the same or next epoch with final_readback_rank higher than queued_no_signal. If process identity still matches, recovery opens retry epoch N+1 and writes signal side-effect receipts before any later interruption. If identity is ambiguous, it writes no signal and holds with manual_process_identity_check.",
    "receipt_model": "immutable_per_epoch_history_only",
    "receipt_readback_rule": "Readback derives latest_receipt from immutable bounded history ordered by shutdown_epoch, receipt_generation, final_readback_rank, and created_at. receipt_history_page is capped at 25 rows with next_cursor. A later retry epoch can write a later interrupted receipt without overwriting queued_no_signal history.",
    "stale_language_ban": "Active contracts, fixtures, and acceptance criteria must not require one interrupted receipt per provider session or UNIQUE(provider_session_id) for shutdown_interrupted_receipts.",
    "duplicate_writer_lookup_rule": "Duplicate writer lookup key is exactly (provider_session_id, shutdown_epoch, receipt_generation). Writers first SELECT by that full key; if found, they reuse that receipt_id. latest_receipt is never a write key.",
    "latest_receipt_rule": "latest_receipt is derived readback over bounded immutable history only. It is computed by ordering shutdown_epoch DESC, receipt_generation DESC, final_readback_rank DESC, created_at DESC and does not imply mutable per-session storage.",
    "fixture_naming_rule": "Fixture names must use per-epoch-history, duplicate-same-epoch-generation, or bounded-history terms. Names containing receipt-unique-per-session or per-session-reuse are stale and fail the vocabulary lint.",
    "receipt_identity_rule": "The only idempotency and duplicate-writer key for shutdown_interrupted_receipts is (provider_session_id, shutdown_epoch, receipt_generation). This key is required for lookup, reuse, recovery repair, metrics, fixtures, and release receipt readback.",
    "forbidden_lookup_rule": "Any code path, recovery matrix row, fixture, or API readback that uses provider_session_id as the sole shutdown receipt lookup or reuse key fails the proposal-083 gate."
  },
  "provider_lifecycle_vocabulary_authority_v1": {
    "addresses": [],
    "schema_version": "provider_lifecycle_vocabulary_authority_v1",
    "canonical_values": [
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
    "authority_rule": "Provider sessions, provider launch events, recovery readback, metrics, and fixtures use only canonical_values. Any value outside this list fails the vocabulary lint.",
    "fixture_rule": "Fixture filenames and JSON bodies under docs/evidence/083/provider-lifecycle-vocab are scanned for non-canonical lifecycle values.",
    "fixtures": [
      "docs/evidence/083/provider-lifecycle-vocab/canonical-values-positive.fixture.json",
      "docs/evidence/083/provider-lifecycle-vocab/noncanonical-value-rejected.fixture.json",
      "docs/evidence/083/provider-lifecycle-vocab/cross-surface-parity.fixture.json"
    ]
  },
  "post_cancel_late_output_contract_v1": {
    "addresses": [
      "SCORE-LIFT-REL-P083-R64-003"
    ],
    "schema_version": "post_cancel_late_output_contract_v1",
    "caps": {
      "max_messages_per_session": 64,
      "max_bytes_per_session": 1048576,
      "max_elapsed_seconds_after_cancellation": 30,
      "max_aggregate_bytes_per_run": 4194304,
      "max_aggregate_bytes_global": 67108864
    },
    "overflow_latch_table": {
      "name": "cancel_late_output_overflow",
      "columns": [
        "overflow_id TEXT PRIMARY KEY",
        "scope TEXT NOT NULL CHECK(scope IN ('session','run','global'))",
        "run_id TEXT NULL",
        "provider_session_id TEXT NULL",
        "cancellation_epoch INTEGER NOT NULL",
        "overflow_kind TEXT NOT NULL CHECK(overflow_kind IN ('message_count','session_bytes','elapsed_time','run_bytes','global_bytes'))",
        "normalized_run_id TEXT GENERATED ALWAYS AS (COALESCE(run_id, '__global__')) STORED",
        "normalized_provider_session_id TEXT GENERATED ALWAYS AS (COALESCE(provider_session_id, '__aggregate__')) STORED",
        "first_observed_at TEXT NOT NULL",
        "last_observed_at TEXT NOT NULL",
        "dropped_message_count INTEGER NOT NULL DEFAULT 0",
        "dropped_byte_count INTEGER NOT NULL DEFAULT 0",
        "quarantine_uri TEXT",
        "reservation_release_state TEXT NOT NULL CHECK(reservation_release_state IN ('not_required','released','release_failed','held_for_recovery'))",
        "CHECK((scope='session' AND provider_session_id IS NOT NULL) OR (scope IN ('run','global') AND provider_session_id IS NULL))",
        "CHECK((scope='global' AND run_id IS NULL) OR (scope IN ('session','run') AND run_id IS NOT NULL))"
      ],
      "unique_index": "CREATE UNIQUE INDEX cancel_late_output_overflow_latch_uniq ON cancel_late_output_overflow(scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind);",
      "indexes": [
        "CREATE INDEX cancel_late_output_overflow_scope_idx ON cancel_late_output_overflow(scope, normalized_run_id, overflow_kind);",
        "CREATE INDEX cancel_late_output_overflow_session_idx ON cancel_late_output_overflow(provider_session_id, cancellation_epoch) WHERE provider_session_id IS NOT NULL;"
      ]
    },
    "aggregation_rule": "After the first overflow row is latched for a session, cancellation epoch, and kind, later drops update counters and last_observed_at on that row instead of inserting more rows.",
    "reservation_rule": "Frame reservations are released in the same read-loop turn that latches or updates overflow accounting.",
    "fixtures": [
      "docs/evidence/083/post-cancel/overflow-latch-single-row.fixture.json",
      "docs/evidence/083/post-cancel/overflow-subsequent-drops-update-counters.fixture.json",
      "docs/evidence/083/post-cancel/reservation-release-after-latch.fixture.json",
      "docs/evidence/083/post-cancel/run-scope-overflow-latch.fixture.json",
      "docs/evidence/083/post-cancel/global-scope-overflow-latch.fixture.json",
      "docs/evidence/083/post-cancel/duplicate-normalized-overflow-row-rejected.fixture.json"
    ],
    "canonical_scope_rule": "One table owns all post-cancel late-output overflow latches. session rows key a single provider_session_id; run rows aggregate across sessions for one run with provider_session_id null; global rows aggregate across all runs with run_id and provider_session_id null. All three scopes use generated normalized key columns and one executable unique index; no SQLite PRIMARY KEY expression is used.",
    "compatibility_rule": "If older evidence contains session-only overflow rows, migration reads them as scope=session rows. Run/global aggregate rows are created only by new cap evaluation or deterministic aggregation repair; duplicate historical aggregate rows are merged by normalized key before the unique index is created."
  },
  "enforcement_mode_cutover_contract_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-001"
    ],
    "schema_version": "enforcement_mode_cutover_contract_v1",
    "persisted_state_table": {
      "name": "p083_enforcement_mode_state",
      "columns": [
        "proposal_id TEXT PRIMARY KEY",
        "proposal_revision_id TEXT NOT NULL",
        "mode TEXT NOT NULL",
        "mode_reason TEXT NOT NULL",
        "burn_in_started_at TEXT",
        "burn_in_completed_at TEXT",
        "updated_by_principal_id TEXT NOT NULL",
        "updated_request_id TEXT NOT NULL"
      ]
    },
    "allowed_transitions": [
      "disabled->permissive",
      "permissive->enforce",
      "enforce->permissive",
      "permissive->disabled"
    ],
    "denied_transitions": [
      "disabled->enforce"
    ],
    "preflight_requirements_for_enforce": [
      "P029 bearer principal boundary active",
      "force reconcile principal-class helper present",
      "approval resolve principal-class helper present",
      "proposal-083 gate passed for this revision",
      "rollout_contract_v1 lint passed",
      "minimum 24 hour permissive burn-in complete",
      "zero hold conditions active",
      "rollback_execution_v1 valid",
      "metric scrape freshness under 180 seconds"
    ],
    "audit_table": {
      "name": "p083_enforcement_mode_audit",
      "columns": [
        "audit_id TEXT PRIMARY KEY",
        "from_mode TEXT NOT NULL",
        "to_mode TEXT NOT NULL",
        "proposal_revision_id TEXT NOT NULL",
        "principal_id TEXT NOT NULL",
        "request_id TEXT NOT NULL",
        "preflight_hash TEXT NOT NULL",
        "decision TEXT NOT NULL",
        "created_at TEXT NOT NULL"
      ]
    },
    "fixtures": [
      "docs/evidence/083/rollout/enforce-preflight-pass.fixture.json",
      "docs/evidence/083/rollout/enforce-denied-auth-missing.fixture.json",
      "docs/evidence/083/rollout/enforce-denied-hold-condition.fixture.json",
      "docs/evidence/083/rollout/enforce-denied-burnin-incomplete.fixture.json",
      "docs/evidence/083/rollout/mode-transition-audit.fixture.json"
    ]
  },
  "rollback_execution_v1": {
    "addresses": [],
    "schema_version": "rollback_execution_v1",
    "graphql_mutation": "p083RollbackExecution(input: P083RollbackExecutionInput!): P083MutationPayload!",
    "mcp_tool": "p083.rollback_execution",
    "principal_class": [
      "operator",
      "control_plane_self"
    ],
    "payload_schema": {
      "type": "object",
      "additionalProperties": false,
      "required": [
        "proposal_revision_id",
        "request_id",
        "target_mode",
        "reason_code",
        "ttl_seconds"
      ],
      "properties": {
        "proposal_revision_id": {
          "const": "P083-r65-refined-97d0ecda"
        },
        "request_id": {
          "$ref": "caller-request-id.v1"
        },
        "target_mode": {
          "enum": [
            "permissive",
            "disabled"
          ]
        },
        "reason_code": {
          "enum": [
            "gate_regression",
            "metric_regression",
            "operator_hold",
            "recovery_fault",
            "manual_emergency"
          ]
        },
        "ttl_seconds": {
          "type": "integer",
          "minimum": 300,
          "maximum": 86400
        },
        "evidence_uri": {
          "type": [
            "string",
            "null"
          ]
        }
      }
    },
    "idempotency_behavior": "Uses command_idempotency_pending_lease_v1 with command p083.rollback_execution. Same request replays; same intent new request follows the idempotency same-intent rules.",
    "audit_rows": [
      "p083_rollback_audit",
      "p083_enforcement_mode_audit"
    ],
    "ttl_reenable_policy": "Rollback to permissive or disabled requires a TTL. On expiry, enforcement remains non-enforce until a new audited permissive burn-in and enforce preflight pass; automatic re-enable is forbidden.",
    "readback_assertions": [
      "rollout_contract_enabled_state reflects target mode",
      "rollback_ttl_expires_at is present",
      "rollback_reason_code is present",
      "operator_next_step names the required re-enable gate"
    ],
    "fixtures": [
      "docs/evidence/083/rollback/graphql-rollback-positive.fixture.json",
      "docs/evidence/083/rollback/mcp-rollback-positive.fixture.json",
      "docs/evidence/083/rollback/non-operator-denied.fixture.json",
      "docs/evidence/083/rollback/idempotent-replay.fixture.json",
      "docs/evidence/083/rollback/ttl-expiry-requires-new-preflight.fixture.json",
      "docs/evidence/083/rollback/graphql-rollback-returns-p083-mutation-payload.fixture.json",
      "docs/evidence/083/rollback/no-p083-rollback-execution-payload-symbol.fixture.json",
      "docs/evidence/083/rollback/mcp-rollback-result-parity-p083-mutation-payload.fixture.json"
    ],
    "return_contract": "P083MutationPayload is the only rollback execution return contract for GraphQL and MCP parity. P083RollbackExecutionPayload is not defined or used.",
    "mcp_result_schema_ref": "api_mutation_contracts_v1.mcp_json_schemas.p083.mutation.result"
  },
  "recovery_readback_contract_v1": {
    "addresses": [
      "SCORE-LIFT-REL-P083-R64-003",
      "SCORE-LIFT-API-P083-R64-001"
    ],
    "pagination_rule": "Recovery readback arrays are capped at page_size <= 100 with opaque next_cursor. Truncated responses include truncated=true and next_step_code.",
    "clock_source_rule": "Leases, shutdown deadlines, heartbeat freshness, and elapsed cancellation caps use durable_monotonic_clock_v1. Wall-clock timestamps are display/audit only.",
    "migration_readback_rule": "Release receipts include applied migration id, filename, sha256, dependency status, applied_at, schema_version, and state."
  },
  "rollout": {
    "phases": [
      {
        "phase": "design_freeze",
        "entry": "proposal review marks Ready",
        "exit": "proposal-083 gate and rollout contract lint pass"
      },
      {
        "phase": "additive_migrations",
        "entry": "Ready proposal",
        "exit": "migration readback fixture passes"
      },
      {
        "phase": "permissive_burn_in",
        "entry": "mode transition disabled->permissive audited",
        "exit": "24 hours with zero hold conditions and fresh metrics"
      },
      {
        "phase": "enforce_cutover",
        "entry": "preflight requirements pass",
        "exit": "mode transition permissive->enforce audited"
      },
      {
        "phase": "rollback_if_needed",
        "entry": "hold condition or operator emergency",
        "exit": "rollback_execution_v1 readback and audit rows present"
      }
    ],
    "hold_conditions": [
      "projection_integrity not fresh for any lifecycle-bearing readback",
      "rollout contract lint failure",
      "metric scrape stale beyond 180 seconds",
      "auth dependency missing",
      "shutdown receipt duplicate violation",
      "post-cancel overflow latch violation",
      "migration readback missing or hash mismatch"
    ],
    "fixture_readiness_rule": "rollout_contract_v1 declares P083-owned fixture paths, and scripts/lint-rollout-contract must pass against those paths before design freeze. Missing P083 readback or negative fixtures are a release hold. Each P083 fixture must assert proposal_id=P083 plus the active proposal_revision_id.",
    "cutover_timestamp_semantics": "rollout_contract_v1.cutover_policy.effective_timestamp_iso8601 is an earliest eligible cutover planning timestamp for lint/readback. It is not an actual release timestamp and does not imply this non-Ready revision has been deployed.",
    "decision_vocabulary_compatibility": "rollout_contract_v1.decision_vocabulary remains the legacy lint field required by the template. Runtime status and decision domains are separated and normalized by rollout_readback_api_parity_v1."
  },
  "metrics": {
    "authority": "metric_labels_contract_v1.operational_metric_label_signatures",
    "adoption_metric": "p083_applicable_runs_with_passing_execution_truth_preflight_percent",
    "operational_metrics_reference": [
      "artifact_lineage_projection_integrity_total{surface,state}",
      "provider_session_legacy_id_read_total{surface}",
      "provider_session_lifecycle_total{provider,lifecycle_state}",
      "command_idempotency_lease_acquire_total{command,outcome}",
      "command_idempotency_replay_total{command,outcome}",
      "command_idempotency_reacquire_total{command,outcome}",
      "command_idempotency_intent_duplicate_total{command,outcome}",
      "command_idempotency_mismatch_denial_total{command}",
      "shutdown_interrupted_receipt_total{provider,receipt_state}",
      "shutdown_duplicate_signal_suppressed_total{provider}",
      "cancel_late_output_overflow_total{provider,scope,overflow_kind}",
      "cancel_late_output_dropped_total{provider,scope,overflow_kind}",
      "rollout_contract_lint_total{proposal_id,status,failure_reason}",
      "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}",
      "p083_enforcement_mode_transition_total{transition,enforcement_mode}",
      "p083_rollback_execution_total{action,status,reason}",
      "p083_debug_metric_total{debug_state}",
      "shutdown_interrupted_state_total{provider,shutdown_interrupted_state}",
      "shutdown_signal_side_effect_total{provider,signal_kind,intent_state}",
      "backpressure_cutoff_total{provider,overflow_kind}",
      "backpressure_process_fate_total{provider,process_fate}",
      "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}"
    ],
    "no_additional_metric_declarations": true,
    "single_source_rule": "This top-level metrics object is a readback mirror of metric_labels_contract_v1.operational_metric_label_signatures. Proposal lint compares it byte-for-byte with rollout_contract_v1.metrics.operational_metrics and fails on drift.",
    "bounded_command_domain_mirror_note": "p083.set_enforcement_mode is included in the bounded command label domain so command_idempotency_* metrics may emit command=p083.set_enforcement_mode without violating metric_unbounded lint. Operational metric names themselves do not change."
  },
  "risks_and_mitigations": [
    {
      "risk": "Versioned GraphQL readback adds two fields for one concept.",
      "mitigation": "Legacy enum field remains stable; v2 field carries raw extensible value and schema version."
    },
    {
      "risk": "App-owned lifecycle windows diverge from existing SwiftUI scene ergonomics.",
      "mitigation": "Only lifecycle-bearing windows move behind LifecycleWindowCoordinator; non-lifecycle utility windows may remain SwiftUI-owned."
    },
    {
      "risk": "Same-intent idempotency behavior may surprise callers using new request ids.",
      "mitigation": "Pending duplicates deny, committed duplicates replay through request aliases, failed/abandoned states have explicit retry semantics."
    },
    {
      "risk": "Rollback disables enforcement while additive schema remains present.",
      "mitigation": "Rollback is audited, TTL-bound, read back to every rollout lane, and requires a new preflight before enforce mode returns."
    }
  ],
  "acceptance_criteria": [
    "proposal-083 and p083 gates exist and run the P083 contract suite.",
    "GraphQL compatibility fixtures prove old enum clients validate and future v2 values degrade to UNKNOWN on the legacy field.",
    "RunPlan compatibility accepts xhigh and preserves it in Swift/Rust byte-equal serialization.",
    "artifact_lineage.report_kind migration and active report-kind uniqueness fixtures pass.",
    "metric_labels_contract_v1 and rollout_contract_v1 contain the same active metric inventory and bounded label domains.",
    "Lifecycle-bearing macOS windows are ordered only by LifecycleWindowCoordinator, not WindowGroup onAppear.",
    "Production teardown uses notification-first @MainActor registry and does not replace existing delegates.",
    "Copy uses NSPasteboard.ContentsOptions.currentHostOnly; Export Text uses NSSavePanel unless the explicit copy-export command is chosen.",
    "Idempotency expiry-to-reacquire, same-intent/new-request, and same-request/different-intent fixtures pass against generated SQLite DDL.",
    "Provider lifecycle vocabulary lint accepts only canonical values.",
    "Post-cancel overflow accounting latches one row per scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, and overflow_kind across session, run, and global scopes; the older session-only latch wording is rejected.",
    "Enforcement-mode preflight and rollback_execution_v1 fixtures pass with audited readback.",
    "P083-specific rollout readback and negative fixtures exist at every path declared by rollout_contract_v1 before the proposal is frozen for implementation.",
    "migration_plan_v1 release-receipt rows and verification queries are present for every P083 additive migration.",
    "Rollback execution and enforcement-mode transitions have complete GraphQL SDL and MCP JSON Schema fixtures.",
    "queued_no_signal is part of shutdown interrupted-state vocabulary with recovery/readback fixtures.",
    "LifecycleWindowCoordinator migration fixtures prove environment injection, command routing, restoration, tabbing policy, and independent projection subscriptions.",
    "durable_monotonic_clock_v1 fixtures cover restart, reboot, wall-clock jump, and suspend/resume behavior.",
    "Expired-pending idempotency reacquire has SQLite CAS and concurrent loser fixtures.",
    "Shutdown graceful and kill signals persist intent/receipt rows with process identity guards and crash/PID reuse fixtures.",
    "UI pending lease and disabled-reason states have fixtures for spinner, disabled trigger, stale projection, and false backend actionability.",
    "scripts/lint-rollout-contract passes for the inline rollout_contract_v1 after the P083 readback fixture and all P083 negative fixtures are created with proposal_id/proposal_revision_id assertions.",
    "MCP rollback and enforcement input schemas are exact Draft 2020-12 JSON Schema objects with additionalProperties=false and fixtures for invalid target modes, reason codes, malformed request ids rejected before result emission, and denial shapes.",
    "projectionIntegrity observed-current-vocabulary table proves stable v1 mapping and v2 raw tampered readback.",
    "shutdown_interrupted_receipts uses immutable per-epoch history with bounded readback and duplicate-writer reuse fixtures.",
    "enforcement transition recovery has durable transition journal rows with commit markers and verification queries.",
    "backpressure_cutoff creates or resumes shutdown with process-fate readback before terminal settlement.",
    "Shutdown interrupted receipts use immutable per-epoch history keyed by provider_session_id, shutdown_epoch, and receipt_generation; latest readback is derived from bounded history.",
    "Legacy projectionIntegrity v1 SDL contains only FRESH, STALE, MISSING, UNKNOWN; tampered appears only in projectionIntegrityV2 unless deployed evidence proves existing v1 support.",
    "Shutdown receipts are immutable per-epoch history rows; latest receipt readback is derived from bounded history and duplicate-writer reuse applies only to the same epoch and generation.",
    "Rollout readback GraphQL and MCP schemas prove vocabulary separation, casing normalization, nullability, and unknown-value handling.",
    "SwiftData lifecycle write guardrails reject unapproved ModelContext mutations for lifecycle truth models.",
    "A single composed AppDelegate installs LifecycleWindowCoordinator before lifecycle windows can be ordered while preserving automation fallback behavior.",
    "backpressure_cutoff_shutdown_pending is process_fate, not provider lifecycle state, and terminal settlement waits for process absence or interrupted receipt.",
    "MCP p083.mutation.result validates as oneOf committed, replayed, or denied, with explicit denial/readback nullability and additionalProperties=false in every branch.",
    "Rollout readback schemas require every declared field; nullable fields must be present with explicit null across GraphQL, MCP, run_report, and release_receipt lanes.",
    "artifact_lineage report_kind triggers reject insert and update paths that would create null, unknown, or duplicate active report kinds.",
    "Coordinator-owned lifecycle roots do not receive a mutable lifecycle SwiftData modelContext, and production mutation_origin guardrails reject lifecycle @Model writes.",
    "Shutdown receipt duplicate writer reuse is exact to provider_session_id, shutdown_epoch, and receipt_generation only.",
    "cancellation_requested is a durable intent table row, not provider lifecycle_state, and backpressure pending recovery cannot settle terminal before absent_verified or interrupted_receipt_recorded.",
    "Malformed MCP request_id fails input validation before command acquisition and before any p083.mutation.result payload is emitted.",
    "GraphQL P083EnforcementReadback.auditId is nullable and default/denied no-audit states match MCP audit_id=null.",
    "cancel_late_output_overflow migration uses executable SQLite generated columns plus unique indexes; duplicate normalized session/run/global rows are rejected by fixtures.",
    "Post-cancel overflow has one canonical session/run/global latch schema and compatibility handling for session-only historical evidence.",
    "Shutdown receipt lookup, reuse, recovery idempotency, metrics, and fixtures require provider_session_id plus shutdown_epoch plus receipt_generation.",
    "Lifecycle roots receive only read-only projection dependencies or app-local contexts excluding lifecycle schemas; app-scoped lifecycle modelContext leakage is a negative fixture.",
    "provider_cancellation_intents and provider_sessions.process_fate have migration and restart fixtures for pending backpressure shutdown.",
    "Recovery repair for enforcement transitions reads transition_journal and command_idempotency instead of inferring from stale state-table or audit-row wording.",
    "SwiftData migration inventory names concrete modules/service methods with target disposition for every lifecycle mutation path.",
    "macOS/UI fixtures cover focused-window command routing, sheet-modal export, accessibility announcements, current-host-only clipboard wording, semantic color tokens, and no-evidence disabled styling.",
    "p083RollbackExecution returns P083MutationPayload in GraphQL and MCP parity fixtures; P083RollbackExecutionPayload is absent from generated schema.",
    "Top-level metrics.operational_metrics_reference, metric_labels_contract_v1.operational_metric_label_signatures, and rollout_contract_v1.metrics.operational_metrics are byte-equal generated mirrors.",
    "Retained WindowGroup surfaces register no lifecycle truth models, use projection-only value types, disable lifecycle autosave paths, and have negative fixtures proving no lifecycle truth writes.",
    "provider_cancellation_intents includes nullable shutdown_epoch with transition rules, indexes, readback fields, and restart fixtures for null requested and non-null shutdown_started states.",
    "recovery_repair_matrix_v1 uses scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, and overflow_kind for overflow recovery across session/run/global scopes.",
    "rollout_contract_v1 exposes shutdown_deadline_config_invalid and command_lease_ttl_config_invalid as hold conditions, readback fields, and negative fixtures.",
    "artifact_lineage_active_report_kind_uniq SQL appears in one canonical final text across proposal, migration, and fixtures.",
    "Rollback disposition readback is a schema_versioned opaque object mirrored by GraphQL and MCP schemas with additionalProperties=false.",
    "Date-time fields use explicit parser validation fixtures; JSON Schema format=date-time is annotation-only.",
    "Projection snapshots carry backend_revision, observed_at, and journal_cursor, and stale frames are dropped per lifecycle_window_id/run_id.",
    "LifecycleWindowCoordinator is installed by the single NSApplicationDelegateAdaptor before restoration or open-run ordering while automation fallback remains composed.",
    "Shutdown waves have bounded concurrency, deterministic ordering, and many-session fixtures for signaled sessions and queued_no_signal receipts.",
    "UI fixtures cover initial projection loading, popover dismissal/focus return, spinner baseline alignment, and popover max width.",
    "macOS fixtures cover focused-window routing, asynchronous termination, responder-chain copy precedence, and export revalidation after focus/run/projection changes.",
    "rollout_readback_api_parity_v1 declares rollout_contract_shutdown_deadline_config_state and rollout_contract_command_lease_ttl_config_state in GraphQL SDL, MCP required, MCP properties, required_with_null_rule, run_report parity, and release_receipt parity; eight lane-coverage fixtures pass.",
    "Rollback disposition is exposed as exactly one symbol, scalar RollbackDispositionJSON, across the SDL surface; MCP rollout_contract_rollback_disposition is additionalProperties=false with the versioned schema; the extra-property negative fixture is rejected and the undefined RollbackDisposition symbol is absent from generated schema.",
    "p083.set_enforcement_mode appears in metric_labels_contract_v1.bounded_label_domains.command alongside p083.rollback_execution; both P083 lifecycle mutations emit command_idempotency_* metrics under bounded command labels and the bounded-label fixture passes.",
    "migration_plan_v1.migrations[p083_007] ddl_summary contains the executable CREATE TABLE provider_cancellation_intents statement, the two ALTER TABLE provider_sessions statements, and the supporting indexes.",
    "rollout_readback_api_parity_v1.graphql_date_time_validation_policy is declared and links to the same RFC3339 parser fixtures used for MCP date-time fields.",
    "P083ProjectionSnapshotOrdering is the single named total-order comparator used by every MainActor projection store; local ad-hoc comparators are rejected.",
    "applicationShouldTerminate returns terminateLater and waits up to host_total_ms plus a 1000ms tail for queued_no_signal receipt flush; the host_total_ms-to-AppKit-budget fixture and Force Quit fixture pass.",
    "Lifecycle window tabbing denies cross-run partners and Merge All Windows across distinct run_ids; same-run distinct-role tabbing is allowed only via explicit Merge All Windows.",
    "NSMenuValidation returns false for lifecycle menu items when no lifecycle window is key (Settings-only or no-window state); menu items render disabled, never hidden.",
    "Bounded shutdown queue_rank is stored on shutdown_interrupted_receipts and exposed as p083_shutdown_queue_rank (GraphQL p083ShutdownQueueRank); many-session, deterministic-order, and restart-preserved fixtures pass.",
    "Recovery for provider_cancellation_intents.requested with shutdown_epoch IS NULL and ambiguous identity holds the intent and returns operator_next_step_code=manual_process_identity_check.",
    "Compact disabled reason rows render at 28pt minHeight using AppDesignSystem.Spacing tokens; the inline spinner has a 6pt leading gap to the disabled trigger label; raw point literals outside the token set are rejected by fixture."
  ],
  "open_questions": [
    "Should the permissive burn-in duration remain fixed at 24 hours or become a release-channel setting after P083 lands?",
    "Should non-lifecycle utility windows eventually move to LifecycleWindowCoordinator for consistency, or stay SwiftUI-owned?",
    "Which dashboards should own long-term alert thresholds for the new P083 operational metrics?"
  ],
  "reviewer_feedback_resolution": {
    "SCORE-LIFT-API-P083-R64-001": {
      "disposition": "addressed",
      "priority": "blocking",
      "source_issue_id": "API-P083-R64-BLOCK-001",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_api_contract",
      "required_change": "Add rollout_contract_shutdown_deadline_config_state and rollout_contract_command_lease_ttl_config_state to RolloutContractReadback SDL, MCP required/properties, required-with-null rules, run_report and release_receipt parity rules, and fixtures proving visibility in every readback lane.",
      "expected_lift": "Remove rollout readback parity drift and make hard-limit hold state machine-evaluable across GraphQL, MCP, reports, and receipts.",
      "addressed_by_sections": [
        "rollout_readback_api_parity_v1.graphql_sdl",
        "rollout_readback_api_parity_v1.mcp_schema.required",
        "rollout_readback_api_parity_v1.mcp_schema.properties",
        "rollout_readback_api_parity_v1.required_with_null_rule",
        "rollout_readback_api_parity_v1.readback_field_lane_coverage_rule",
        "rollout_readback_api_parity_v1.config_state_vocabulary",
        "rollout_contract_v1.readback_fields",
        "rollout_contract_v1.operator_report_fields"
      ],
      "resolution_notes": "Both config-state fields are declared as non-null enum (valid|invalid|unknown) across SDL, MCP schema, run_report, and release_receipt; eight new lane-coverage fixtures cover both fields in each lane."
    },
    "SCORE-LIFT-API-P083-R64-002": {
      "disposition": "addressed",
      "priority": "blocking",
      "source_issue_id": "API-P083-R64-BLOCK-002",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_api_contract",
      "required_change": "Choose one rollback disposition GraphQL shape and use it everywhere. If opaque JSON is intended, define RollbackDispositionJSON in RolloutContractReadback SDL and mirror the same versioned schema in MCP with additionalProperties=false. Remove the undefined RollbackDisposition reference and add a negative fixture for extra MCP properties.",
      "expected_lift": "Eliminate GraphQL/MCP schema conflict and make rollback disposition validation executable.",
      "addressed_by_sections": [
        "rollout_readback_api_parity_v1.graphql_sdl",
        "rollout_readback_api_parity_v1.mcp_schema.properties.rollout_contract_rollback_disposition",
        "rollout_readback_api_parity_v1.rollback_disposition_schema_policy"
      ],
      "resolution_notes": "Exactly one symbol, scalar RollbackDispositionJSON, is used across the SDL surface; the undefined RollbackDisposition reference is forbidden by lint and by docs/evidence/083/api/rollback-disposition-graphql-rolloutdisposition-symbol-absent.fixture.json. MCP rollback_disposition is additionalProperties=false with the required schema_version/mode/data_loss_risk/steps fields; docs/evidence/083/api/rollback-disposition-mcp-extra-property-rejected.fixture.json is the negative fixture for extra MCP properties."
    },
    "SCORE-LIFT-API-P083-R64-003": {
      "disposition": "addressed",
      "priority": "blocking",
      "source_issue_id": "API-P083-R64-BLOCK-003",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_api_contract",
      "required_change": "Add p083.set_enforcement_mode to metric_labels_contract_v1.bounded_label_domains.command, keep all generated operational metric mirrors byte-equal, and add a fixture proving both P083 lifecycle mutations use bounded command labels.",
      "expected_lift": "Allow p083SetEnforcementMode idempotency metrics without violating bounded label contracts.",
      "addressed_by_sections": [
        "metric_labels_contract_v1.bounded_label_domains.command",
        "metric_labels_contract_v1.lifecycle_mutation_bounded_label_rule",
        "metric_labels_contract_v1.single_source_export_rule",
        "metrics.bounded_command_domain_mirror_note",
        "rollout_contract_v1.bounded_command_domain_mirror_note"
      ],
      "resolution_notes": "p083.set_enforcement_mode is appended to bounded_label_domains.command; the operational_metric_label_signatures list (and its byte-equal mirrors in metrics.operational_metrics_reference and rollout_contract_v1.metrics.operational_metrics) are unchanged because the bounded label widening does not introduce new metric names. Fixture docs/evidence/083/metrics/p083-lifecycle-mutations-bounded-command-labels.fixture.json proves both p083.rollback_execution and p083.set_enforcement_mode emit command_idempotency_* metrics under bounded command labels."
    },
    "SCORE-LIFT-API-P083-R64-004": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "API-P083-R64-NB-001",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_api_contract",
      "required_change": "Make p083_007 migration ddl_summary match the executable DDL style used by earlier migrations, including CREATE TABLE provider_cancellation_intents, primary key, and provider_sessions ALTER statements.",
      "expected_lift": "Reduce migration summary drift risk during implementation.",
      "addressed_by_sections": [
        "migration_plan_v1.migrations[p083_007_provider_cancellation_intent_and_process_fate].ddl_summary",
        "migration_plan_v1.migrations[p083_007_provider_cancellation_intent_and_process_fate].ddl_summary_style_rule"
      ],
      "resolution_notes": "p083_007 ddl_summary now contains the full CREATE TABLE provider_cancellation_intents statement with primary key, the two ALTER TABLE provider_sessions statements for process_fate and process_fate_updated_at, and the supporting indexes."
    },
    "SCORE-LIFT-API-P083-R64-005": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "API-P083-R64-NB-002",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_api_contract",
      "required_change": "Add a short GraphQL date-time validation policy next to the JSON Schema date-time policy, or introduce a DateTime scalar if the implementation already has one.",
      "expected_lift": "Avoid inconsistent date-time validation assumptions between SDL and parser fixtures.",
      "addressed_by_sections": [
        "rollout_readback_api_parity_v1.graphql_date_time_validation_policy",
        "api_mutation_contracts_v1.json_schema_format_assertion_policy"
      ],
      "resolution_notes": "GraphQL date-time fields use the String scalar and rely on the same RFC3339 parser fixtures already declared in api_mutation_contracts_v1. Policy is explicit that SDL type identity does not enforce date-time format and that any future DateTime scalar must wrap the same parser."
    },
    "SCORE-LIFT-APPLE-P083-R64-001": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "APPLE-P083-R64-NB-001",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_apple_architect",
      "required_change": "Define one named total-order comparator for MainActor projection snapshot acceptance across backend_revision, journal_cursor, and observed_at tie-breaking.",
      "expected_lift": "Prevent divergent stale-frame decisions across Swift projection stores.",
      "addressed_by_sections": [
        "swift_app_projection_contract_v1.projection_concurrency_contract.named_snapshot_comparator"
      ],
      "resolution_notes": "P083ProjectionSnapshotOrdering.compare is the single named total-order comparator used by every MainActor projection store; lint rejects local ad-hoc comparators."
    },
    "SCORE-LIFT-MACOS-P083-R64-001": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "MACOS-P083-R64-OBS-001",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_macos",
      "required_change": "Document how bounded shutdown wave host_total_ms relates to AppKit applicationShouldTerminate budget, or add a fixture proving queued_no_signal receipts are flushed durably before termination returns.",
      "expected_lift": "Improve shutdown observability under Force Quit, logout, and system shutdown constraints.",
      "addressed_by_sections": [
        "reliability_deadline_overflow_contract_v1.bounded_shutdown_wave_policy.host_total_ms_to_appkit_budget_rule",
        "reliability_deadline_overflow_contract_v1.fixtures"
      ],
      "resolution_notes": "applicationShouldTerminate uses terminateLater and waits up to host_total_ms plus a 1000ms tail; queued_no_signal receipts are flushed to SQLite under WAL before reply(toApplicationShouldTerminate:). Fixtures cover the normal terminate path, Force Quit, and the queued_no_signal flush invariant."
    },
    "SCORE-LIFT-MACOS-P083-R64-002": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "MACOS-P083-R64-OBS-002",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_macos",
      "required_change": "Add fixtures for NSWindow automatic tabbing and Merge All Windows behavior across distinct run_ids and same-run same-restoration-role windows.",
      "expected_lift": "Improve multi-window predictability for lifecycle surfaces.",
      "addressed_by_sections": [
        "macos_ui_implementation_fixtures_v1.automatic_tabbing_rule",
        "macos_ui_implementation_fixtures_v1.required_fixtures"
      ],
      "resolution_notes": "Cross-run tabbing and Merge All Windows are denied with cross_run_tabbing_denied; same-run distinct-role tabbing is allowed only when the operator merges windows explicitly. Four new fixtures cover both directions."
    },
    "SCORE-LIFT-MACOS-P083-R64-003": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "MACOS-P083-R64-OBS-003",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_macos",
      "required_change": "Specify NSMenuValidation behavior when no lifecycle window is key so lifecycle menu items render disabled rather than hidden or silently no-op.",
      "expected_lift": "Clarify keyboard/menu behavior for Settings-only or no-window states.",
      "addressed_by_sections": [
        "macos_ui_implementation_fixtures_v1.nsmenu_validation_rule",
        "macos_ui_implementation_fixtures_v1.required_fixtures"
      ],
      "resolution_notes": "validateMenuItem returns false for lifecycle menu items when no lifecycle window is key; menu items render disabled (never hidden), and accidental selection is a silent no-op without side effects."
    },
    "SCORE-LIFT-REL-P083-R64-001": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "REL-P083-R64-NB-001",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_reliability",
      "required_change": "State whether bounded shutdown queue_rank is stored as final_readback_rank, derived from shutdown_epoch/provider_session_id ordering, or represented as a separate receipt/readback field; add a many-session queued rank fixture.",
      "expected_lift": "Prevent readback and fixture ambiguity for bounded shutdown waves.",
      "addressed_by_sections": [
        "reliability_deadline_overflow_contract_v1.bounded_shutdown_wave_policy.queue_rank_storage_rule",
        "shutdown_signal_side_effect_contract_v1.queue_rank_field",
        "rollout_readback_api_parity_v1.mcp_schema.properties.p083_shutdown_queue_rank",
        "rollout_readback_api_parity_v1.graphql_sdl"
      ],
      "resolution_notes": "queue_rank is stored on shutdown_interrupted_receipts and exposed as p083_shutdown_queue_rank (GraphQL p083ShutdownQueueRank). Three new fixtures cover many-session storage, deterministic order, and restart preservation."
    },
    "SCORE-LIFT-REL-P083-R64-002": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "REL-P083-R64-NB-002",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_reliability",
      "required_change": "Add an explicit requested/null-shutdown_epoch identity_ambiguous recovery rule that holds the intent and returns manual_process_identity_check.",
      "expected_lift": "Close a stuck-state interpretation gap in cancellation intent recovery.",
      "addressed_by_sections": [
        "provider_cancellation_intent_contract_v1.identity_ambiguous_recovery_rule",
        "recovery_repair_matrix_v1.rows"
      ],
      "resolution_notes": "Requested intents with null shutdown_epoch and ambiguous identity are held with intent_state=requested, process_fate=identity_ambiguous, and operator_next_step_code=manual_process_identity_check. A new recovery_repair_matrix row encodes the rule."
    },
    "SCORE-LIFT-REL-P083-R64-003": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "REL-P083-R64-NB-003",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_reliability",
      "required_change": "Replace the older session-only overflow latch acceptance criterion with the canonical scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind wording.",
      "expected_lift": "Keep acceptance criteria aligned with normalized session/run/global latch keys.",
      "addressed_by_sections": [
        "acceptance_criteria",
        "recovery_repair_matrix_v1.overflow_recovery_key_rule",
        "reliability_deadline_overflow_contract_v1.aggregate_overflow_latch_owner"
      ],
      "resolution_notes": "The acceptance line about post-cancel overflow now reads 'one row per scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, and overflow_kind across session, run, and global scopes'; the older session-only wording is removed."
    },
    "SCORE-LIFT-UI-P083-R64-001": {
      "disposition": "addressed",
      "priority": "advisory",
      "source_issue_id": "UI-P083-R64-NB-001",
      "source_reviewer_id": "dynamic_review_proposal_reviewer_ui",
      "required_change": "Specify standard spacing and density tokens for compact disabled reason rows and the gap between inline spinner and disabled trigger label.",
      "expected_lift": "Improve visual consistency without changing proposal readiness.",
      "addressed_by_sections": [
        "swift_app_projection_contract_v1.ui_pending_and_disabled_contract.spacing_and_density_tokens"
      ],
      "resolution_notes": "Compact disabled reason rows use 28pt minHeight with footnote text and AppDesignSystem.Spacing tokens; inline spinner is 16x16pt with a 6pt leading gap to the disabled trigger label. Raw point literals outside the token set are rejected by fixture."
    }
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
      "justification": "P083 has additive SQLite migrations; canonical migration details are declared in proposal architecture and feature contracts, while this rollout object stays within rollout_contract_v1 schema."
    },
    "metrics": {
      "adoption_metric": "p083_applicable_runs_with_passing_execution_truth_preflight_percent",
      "operational_metrics": [
        "artifact_lineage_projection_integrity_total{surface,state}",
        "provider_session_legacy_id_read_total{surface}",
        "provider_session_lifecycle_total{provider,lifecycle_state}",
        "command_idempotency_lease_acquire_total{command,outcome}",
        "command_idempotency_replay_total{command,outcome}",
        "command_idempotency_reacquire_total{command,outcome}",
        "command_idempotency_intent_duplicate_total{command,outcome}",
        "command_idempotency_mismatch_denial_total{command}",
        "shutdown_interrupted_receipt_total{provider,receipt_state}",
        "shutdown_duplicate_signal_suppressed_total{provider}",
        "cancel_late_output_overflow_total{provider,scope,overflow_kind}",
        "cancel_late_output_dropped_total{provider,scope,overflow_kind}",
        "rollout_contract_lint_total{proposal_id,status,failure_reason}",
        "rollout_contract_run_start_block_total{proposal_id,reason,enforcement_mode}",
        "p083_enforcement_mode_transition_total{transition,enforcement_mode}",
        "p083_rollback_execution_total{action,status,reason}",
        "p083_debug_metric_total{debug_state}",
        "shutdown_interrupted_state_total{provider,shutdown_interrupted_state}",
        "shutdown_signal_side_effect_total{provider,signal_kind,intent_state}",
        "backpressure_cutoff_total{provider,overflow_kind}",
        "backpressure_process_fate_total{provider,process_fate}",
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
      "p083_rollback_ttl_expires_at",
      "p083_last_preflight_hash",
      "rollout_contract_shutdown_deadline_config_state",
      "rollout_contract_command_lease_ttl_config_state",
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
      "p083_rollback_ttl_expires_at",
      "p083_last_preflight_hash",
      "rollout_contract_shutdown_deadline_config_state",
      "rollout_contract_command_lease_ttl_config_state",
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
      "command_lease_ttl_config_invalid"
    ],
    "hold_conditions_detail": {
      "projection_integrity_not_fresh": "Any lifecycle-bearing readback is not actionable.",
      "metric_scrape_stale": "Required metric scrape is older than 180 seconds.",
      "auth_dependency_missing": "Required principal-class helper or bearer boundary is unavailable.",
      "shutdown_deadline_config_invalid": "Configured shutdown deadline exceeds reliability_deadline_overflow_contract_v1 hard maximum or has invalid wave ordering.",
      "command_lease_ttl_config_invalid": "Configured command lease TTL is outside reliability_deadline_overflow_contract_v1 bounds."
    },
    "rollback_disposition": {
      "mode": "p083.rollback_execution_to_permissive_or_disabled",
      "data_loss_risk": "none",
      "steps": [
        "Call p083RollbackExecution or p083.rollback_execution with operator principal class and CallerRequestId.",
        "Persist rollback audit and enforcement-mode audit rows.",
        "Expose disabled/permissive state and TTL in every readback lane.",
        "Require a fresh permissive burn-in and enforce preflight before returning to enforce mode."
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
      "shutdown_deadline_config_invalid": "docs/evidence/rollout-contract/negative/p083-shutdown-deadline-config-invalid.json",
      "command_lease_ttl_config_invalid": "docs/evidence/rollout-contract/negative/p083-command-lease-ttl-config-invalid.json"
    },
    "cutover_policy": {
      "revision": "p083-rollout-cutover-v21-r64",
      "enforcement_mode_at_cutover": "enforce",
      "applicable_to": "post_cutover_implementation_starts",
      "grandfathered_rendering": "not_applicable",
      "effective_timestamp_iso8601": "2026-06-02T00:00:00Z"
    }
  },
  "migration_plan_v1": {
    "addresses": [
      "SCORE-LIFT-REL-P083-R64-003"
    ],
    "schema_version": "migration_plan_v1",
    "ordering_rule": "Migrations are applied in listed order. Every release receipt and operator readback exposes logical_id, filename, sha256, dependencies, applied_at, schema_version, state, and verification_query_result for each row.",
    "rollback_rule": "Rollback never drops P083 additive schema. Rollback changes enforcement mode to permissive or disabled through rollback_execution_v1 and leaves all evidence queryable.",
    "migrations": [
      {
        "logical_id": "p083_001_artifact_lineage_report_kind",
        "filename": "control-plane/crates/db/migrations/20260602_p083_001_artifact_lineage_report_kind.sql",
        "depends_on": [],
        "ddl_summary": [
          "ALTER TABLE artifact_lineage ADD COLUMN report_kind TEXT NULL",
          "CREATE TRIGGER artifact_lineage_report_kind_required_insert BEFORE INSERT ON artifact_lineage WHEN NEW.artifact_role = 'report' AND NEW.active = 1 AND (NEW.report_kind IS NULL OR NEW.report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack')) BEGIN SELECT RAISE(ABORT, 'artifact_lineage.report_kind required for active report'); END;",
          "CREATE TRIGGER artifact_lineage_report_kind_required_update BEFORE UPDATE OF active, report_kind, artifact_role ON artifact_lineage WHEN NEW.artifact_role = 'report' AND NEW.active = 1 AND (NEW.report_kind IS NULL OR NEW.report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack')) BEGIN SELECT RAISE(ABORT, 'artifact_lineage.report_kind required for active report'); END;",
          "CREATE UNIQUE INDEX artifact_lineage_active_report_kind_uniq ON artifact_lineage(run_id, report_kind) WHERE active = 1 AND artifact_role = 'report' AND report_kind IS NOT NULL;"
        ],
        "defaults_backfills": "Existing report rows derive report_kind from logical_name when it maps to the allowed report_kind registry; otherwise report_kind remains null, active is set false, and projection_integrity becomes stale until repaired. Non-report rows stay null.",
        "verification_query": "SELECT artifact_id FROM artifact_lineage WHERE artifact_role = 'report' AND active = 1 AND (report_kind IS NULL OR report_kind NOT IN ('proposal_current','proposal_revision_summary','proposal_feedback_coverage','review_summary','run_report','release_receipt','evidence_pack'));",
        "expected_verification_result": "zero rows",
        "rollback_disposition": "additive_schema_retained_enforcement_can_disable",
        "release_receipt_row": "applied_migrations[p083_001_artifact_lineage_report_kind]"
      },
      {
        "logical_id": "p083_002_command_idempotency_generations",
        "filename": "control-plane/crates/db/migrations/20260602_p083_002_command_idempotency_generations.sql",
        "depends_on": [
          "p083_001_artifact_lineage_report_kind"
        ],
        "ddl_summary": [
          "CREATE TABLE command_idempotency(...)",
          "CREATE TABLE command_request_aliases(...)",
          "CREATE UNIQUE INDEX command_request_active_uniq ... WHERE lease_state IN ('pending','committed','failed')",
          "CREATE UNIQUE INDEX command_intent_active_uniq ... WHERE lease_state IN ('pending','committed')",
          "CREATE UNIQUE INDEX command_generation_uniq ON command_idempotency(principal_id, request_id, lease_generation)"
        ],
        "defaults_backfills": "No historical command rows are synthesized. Pre-P083 command journal entries remain evidence only and are not promoted into idempotency authority.",
        "verification_query": "SELECT principal_id, request_id, lease_generation, COUNT(*) FROM command_idempotency GROUP BY principal_id, request_id, lease_generation HAVING COUNT(*) > 1;",
        "expected_verification_result": "zero rows",
        "rollback_disposition": "additive_schema_retained_pending_rows_settle_by_existing_recovery",
        "release_receipt_row": "applied_migrations[p083_002_command_idempotency_generations]"
      },
      {
        "logical_id": "p083_003_shutdown_receipts_and_signals",
        "filename": "control-plane/crates/db/migrations/20260602_p083_003_shutdown_receipts_and_signals.sql",
        "depends_on": [
          "p083_002_command_idempotency_generations"
        ],
        "ddl_summary": [
          "CREATE TABLE shutdown_interrupted_receipts(...) with immutable per-epoch history",
          "CREATE UNIQUE INDEX shutdown_interrupted_receipts_epoch_generation_uniq ON shutdown_interrupted_receipts(provider_session_id, shutdown_epoch, receipt_generation)",
          "CREATE TABLE shutdown_signal_side_effects(...)",
          "CREATE UNIQUE INDEX shutdown_signal_side_effect_unique ON shutdown_signal_side_effects(provider_session_id, shutdown_epoch, signal_kind, generation)"
        ],
        "defaults_backfills": "Existing provider sessions do not get synthetic signal receipts. On first recovery, missing signal receipts are treated as unknown side-effect state and follow shutdown_signal_side_effect_contract_v1 recovery rules.",
        "verification_query": "SELECT provider_session_id, shutdown_epoch, receipt_generation, COUNT(*) FROM shutdown_interrupted_receipts GROUP BY provider_session_id, shutdown_epoch, receipt_generation HAVING COUNT(*) > 1;",
        "expected_verification_result": "zero rows",
        "rollback_disposition": "additive_schema_retained_shutdown_recovery_still_reads_rows",
        "release_receipt_row": "applied_migrations[p083_003_shutdown_receipts_and_signals]"
      },
      {
        "logical_id": "p083_004_cancel_late_output_overflow",
        "filename": "control-plane/crates/db/migrations/20260602_p083_004_cancel_late_output_overflow.sql",
        "depends_on": [
          "p083_003_shutdown_receipts_and_signals"
        ],
        "ddl_summary": [
          "CREATE TABLE cancel_late_output_overflow(overflow_id TEXT PRIMARY KEY, scope TEXT NOT NULL CHECK(scope IN ('session','run','global')), run_id TEXT NULL, provider_session_id TEXT NULL, cancellation_epoch INTEGER NOT NULL, overflow_kind TEXT NOT NULL, normalized_run_id TEXT GENERATED ALWAYS AS (COALESCE(run_id, '__global__')) STORED, normalized_provider_session_id TEXT GENERATED ALWAYS AS (COALESCE(provider_session_id, '__aggregate__')) STORED, first_observed_at TEXT NOT NULL, last_observed_at TEXT NOT NULL, dropped_message_count INTEGER NOT NULL DEFAULT 0, dropped_byte_count INTEGER NOT NULL DEFAULT 0, quarantine_uri TEXT, reservation_release_state TEXT NOT NULL, CHECK((scope='session' AND provider_session_id IS NOT NULL) OR (scope IN ('run','global') AND provider_session_id IS NULL)), CHECK((scope='global' AND run_id IS NULL) OR (scope IN ('session','run') AND run_id IS NOT NULL)))",
          "CREATE UNIQUE INDEX cancel_late_output_overflow_latch_uniq ON cancel_late_output_overflow(scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind)",
          "CREATE INDEX cancel_late_output_overflow_scope_idx ON cancel_late_output_overflow(scope, normalized_run_id, overflow_kind)"
        ],
        "defaults_backfills": "No historical late-output rows are generated. Existing session-only rows, if present in pre-release test data, are normalized as scope=session. Duplicate normalized keys are merged before creating cancel_late_output_overflow_latch_uniq.",
        "verification_query": "SELECT scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind, COUNT(*) FROM cancel_late_output_overflow GROUP BY scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind HAVING COUNT(*) > 1;",
        "expected_verification_result": "zero rows",
        "rollback_disposition": "additive_schema_retained_late_output_accounting_read_only_when_disabled",
        "release_receipt_row": "applied_migrations[p083_004_cancel_late_output_overflow]",
        "negative_fixture": "docs/evidence/083/migrations/p083-004-duplicate-normalized-overflow-rejected.fixture.json"
      },
      {
        "logical_id": "p083_005_enforcement_and_rollback",
        "filename": "control-plane/crates/db/migrations/20260602_p083_005_enforcement_and_rollback.sql",
        "depends_on": [
          "p083_004_cancel_late_output_overflow"
        ],
        "ddl_summary": [
          "CREATE TABLE p083_enforcement_mode_state(...)",
          "CREATE TABLE p083_enforcement_mode_transition_journal(...)",
          "CREATE TABLE p083_enforcement_mode_audit(...)",
          "CREATE TABLE p083_rollback_audit(...)"
        ],
        "defaults_backfills": "Insert one default p083_enforcement_mode_state row with mode=disabled, mode_reason=initial_migration_disabled, and audit_id null. No initial audit row is synthesized solely to satisfy readback nullability; GraphQL and MCP readback expose auditId/audit_id as nullable.",
        "verification_query": "SELECT COUNT(*) FROM p083_enforcement_mode_transition_journal WHERE transition_state = 'transitioning' AND commit_marker IS NOT NULL;",
        "expected_verification_result": "zero rows",
        "rollback_disposition": "enforcement_mode_transition_only_no_schema_drop",
        "release_receipt_row": "applied_migrations[p083_005_enforcement_and_rollback]"
      },
      {
        "logical_id": "p083_006_durable_monotonic_clock",
        "filename": "control-plane/crates/db/migrations/20260602_p083_006_durable_monotonic_clock.sql",
        "depends_on": [
          "p083_005_enforcement_and_rollback"
        ],
        "ddl_summary": [
          "CREATE TABLE durable_monotonic_clock_samples(...)",
          "CREATE INDEX durable_monotonic_clock_samples_boot_idx ON durable_monotonic_clock_samples(boot_id, observed_at_wall_clock)"
        ],
        "defaults_backfills": "First daemon start after migration records a baseline sample. Existing monotonic deadlines are treated as same-boot only until a sample exists.",
        "verification_query": "SELECT COUNT(*) FROM durable_monotonic_clock_samples WHERE sample_state = 'baseline';",
        "expected_verification_result": "at least one row after daemon start",
        "rollback_disposition": "additive_schema_retained_deadline_recovery_uses_wall_clock_hold_if_uncertain",
        "release_receipt_row": "applied_migrations[p083_006_durable_monotonic_clock]"
      },
      {
        "logical_id": "p083_007_provider_cancellation_intent_and_process_fate",
        "filename": "control-plane/crates/db/migrations/20260602_p083_007_provider_cancellation_intent_and_process_fate.sql",
        "depends_on": [
          "p083_006_durable_monotonic_clock"
        ],
        "ddl_summary": [
          "CREATE TABLE provider_cancellation_intents(provider_session_id TEXT NOT NULL, cancellation_epoch INTEGER NOT NULL, intent_state TEXT NOT NULL CHECK(intent_state IN ('requested','shutdown_started','settled','held')), reason TEXT NOT NULL CHECK(reason IN ('operator_cancel','backpressure_cutoff','shutdown_recovery')), requested_at_monotonic_ms INTEGER NOT NULL, requested_at_wall_clock TEXT NOT NULL, shutdown_epoch INTEGER NULL, shutdown_epoch_assigned_at TEXT NULL, PRIMARY KEY(provider_session_id, cancellation_epoch))",
          "CREATE INDEX provider_cancellation_intents_shutdown_epoch_idx ON provider_cancellation_intents(provider_session_id, shutdown_epoch) WHERE shutdown_epoch IS NOT NULL",
          "CREATE INDEX provider_cancellation_intents_state_idx ON provider_cancellation_intents(intent_state, reason)",
          "ALTER TABLE provider_sessions ADD COLUMN process_fate TEXT NOT NULL DEFAULT 'running' CHECK(process_fate IN ('running','backpressure_cutoff_shutdown_pending','absent_verified','interrupted_receipt_recorded','identity_ambiguous'))",
          "ALTER TABLE provider_sessions ADD COLUMN process_fate_updated_at TEXT NULL",
          "CREATE INDEX provider_sessions_process_fate_idx ON provider_sessions(process_fate)"
        ],
        "defaults_backfills": "Backfill provider_cancellation_intents only from durable cancellation_epoch plus shutdown side-effect evidence. shutdown_epoch is nullable for requested intents until shutdown planning starts; shutdown_started and settled rows with shutdown evidence require non-null shutdown_epoch. Ambiguous legacy rows are held for manual_process_identity_check.",
        "verification_query": "SELECT provider_session_id FROM provider_cancellation_intents WHERE intent_state IN ('shutdown_started','settled') AND reason IN ('operator_cancel','backpressure_cutoff','shutdown_recovery') AND shutdown_epoch IS NULL;",
        "expected_verification_result": "zero rows",
        "rollback_disposition": "additive_schema_retained_recovery_reads_intent_rows",
        "release_receipt_row": "applied_migrations[p083_007_provider_cancellation_intent_and_process_fate]",
        "ddl_summary_style_rule": "ddl_summary statements are executable SQLite text consistent with p083_004 and p083_005, including CREATE TABLE, ALTER TABLE, and CREATE INDEX forms. The migration_plan_v1.migrations[].ddl_summary lint asserts each statement parses as a complete SQLite DDL statement.",
        "addresses": [
          "SCORE-LIFT-API-P083-R64-004"
        ]
      }
    ]
  },
  "api_mutation_contracts_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-002",
      "SCORE-LIFT-API-P083-R64-005"
    ],
    "schema_version": "api_mutation_contracts_v1",
    "graphql_sdl": [
      "scalar CallerRequestId",
      "enum P083EnforcementMode { DISABLED PERMISSIVE ENFORCE }",
      "enum P083RollbackTargetMode { DISABLED PERMISSIVE }",
      "enum P083RollbackReasonCode { GATE_REGRESSION METRIC_REGRESSION OPERATOR_HOLD RECOVERY_FAULT MANUAL_EMERGENCY }",
      "enum P083EnforcementReasonCode { PREFLIGHT_PASSED BURN_IN_COMPLETE OPERATOR_DISABLE OPERATOR_PERMISSIVE RECOVERY_SETTLEMENT }",
      "enum P083MutationOutcome { COMMITTED REPLAYED DENIED }",
      "enum P083DenialCode { UNAUTHORIZED STALE_PROPOSAL_REVISION INVALID_TRANSITION PREFLIGHT_FAILED HOLD_CONDITION_PRESENT ROLLBACK_TTL_INVALID IDEMPOTENCY_PENDING REQUEST_REUSE_MISMATCH PROJECTION_NOT_FRESH INVALID_REASON_CODE }",
      "input P083SetEnforcementModeInput { proposalRevisionId: ID! requestId: CallerRequestId! targetMode: P083EnforcementMode! reasonCode: P083EnforcementReasonCode! evidenceUri: String }",
      "input P083RollbackExecutionInput { proposalRevisionId: ID! requestId: CallerRequestId! targetMode: P083RollbackTargetMode! reasonCode: P083RollbackReasonCode! ttlSeconds: Int! evidenceUri: String }",
      "type P083ExecutionTruthDenial { code: P083DenialCode! requestId: CallerRequestId! operatorMessage: String! projectionIntegrity: ProjectionIntegrity! projectionIntegrityV2: ProjectionIntegrityValue! retryAfterSeconds: Int }",
      "type P083EnforcementReadback { proposalRevisionId: ID! mode: P083EnforcementMode! modeReason: String! auditId: ID rollbackTtlExpiresAt: String projectionIntegrity: ProjectionIntegrity! projectionIntegrityV2: ProjectionIntegrityValue! }",
      "type P083MutationPayload { outcome: P083MutationOutcome! requestId: CallerRequestId! replayed: Boolean! commandRecordId: ID auditIds: [ID!]! denial: P083ExecutionTruthDenial readback: P083EnforcementReadback! }",
      "extend type Mutation { p083SetEnforcementMode(input: P083SetEnforcementModeInput!): P083MutationPayload! p083RollbackExecution(input: P083RollbackExecutionInput!): P083MutationPayload! }"
    ],
    "rollback_target_rule": "Rollback accepts only disabled or permissive. ENFORCE is not in P083RollbackTargetMode and MCP rejects target_mode=enforce with denial code invalid_transition and no side effects.",
    "reason_code_registry": {
      "rollback": [
        "gate_regression",
        "metric_regression",
        "operator_hold",
        "recovery_fault",
        "manual_emergency"
      ],
      "enforcement": [
        "preflight_passed",
        "burn_in_complete",
        "operator_disable",
        "operator_permissive",
        "recovery_settlement"
      ]
    },
    "mcp_json_schemas": {
      "p083.rollback_execution.input": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": [
          "proposal_revision_id",
          "request_id",
          "target_mode",
          "reason_code",
          "ttl_seconds"
        ],
        "properties": {
          "proposal_revision_id": {
            "const": "P083-r65-refined-97d0ecda"
          },
          "request_id": {
            "type": "string",
            "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{12}$"
          },
          "target_mode": {
            "enum": [
              "disabled",
              "permissive"
            ]
          },
          "reason_code": {
            "enum": [
              "gate_regression",
              "metric_regression",
              "operator_hold",
              "recovery_fault",
              "manual_emergency"
            ]
          },
          "ttl_seconds": {
            "type": "integer",
            "minimum": 300,
            "maximum": 86400
          },
          "evidence_uri": {
            "type": [
              "string",
              "null"
            ],
            "maxLength": 2048
          }
        }
      },
      "p083.set_enforcement_mode.input": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "additionalProperties": false,
        "required": [
          "proposal_revision_id",
          "request_id",
          "target_mode",
          "reason_code"
        ],
        "properties": {
          "proposal_revision_id": {
            "const": "P083-r65-refined-97d0ecda"
          },
          "request_id": {
            "type": "string",
            "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{12}$"
          },
          "target_mode": {
            "enum": [
              "disabled",
              "permissive",
              "enforce"
            ]
          },
          "reason_code": {
            "enum": [
              "preflight_passed",
              "burn_in_complete",
              "operator_disable",
              "operator_permissive",
              "recovery_settlement"
            ]
          },
          "evidence_uri": {
            "type": [
              "string",
              "null"
            ],
            "maxLength": 2048
          }
        }
      },
      "p083.mutation.result": {
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "oneOf": [
          {
            "type": "object",
            "additionalProperties": false,
            "required": [
              "outcome",
              "request_id",
              "replayed",
              "command_record_id",
              "audit_ids",
              "denial",
              "readback"
            ],
            "properties": {
              "outcome": {
                "const": "committed"
              },
              "request_id": {
                "type": "string",
                "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{12}$"
              },
              "replayed": {
                "const": false
              },
              "command_record_id": {
                "type": "string",
                "minLength": 1
              },
              "audit_ids": {
                "type": "array",
                "items": {
                  "type": "string",
                  "minLength": 1
                },
                "minItems": 1,
                "maxItems": 8
              },
              "denial": {
                "type": "null"
              },
              "readback": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                  "proposal_revision_id",
                  "mode",
                  "mode_reason",
                  "audit_id",
                  "rollback_ttl_expires_at",
                  "projection_integrity_v1",
                  "projection_integrity_v2"
                ],
                "properties": {
                  "proposal_revision_id": {
                    "const": "P083-r65-refined-97d0ecda"
                  },
                  "mode": {
                    "enum": [
                      "disabled",
                      "permissive",
                      "enforce"
                    ]
                  },
                  "mode_reason": {
                    "type": "string",
                    "maxLength": 160
                  },
                  "audit_id": {
                    "type": [
                      "string",
                      "null"
                    ]
                  },
                  "rollback_ttl_expires_at": {
                    "type": [
                      "string",
                      "null"
                    ],
                    "format": "date-time"
                  },
                  "projection_integrity_v1": {
                    "enum": [
                      "fresh",
                      "stale",
                      "missing",
                      "unknown"
                    ]
                  },
                  "projection_integrity_v2": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                      "schema_version",
                      "value",
                      "known_v1_value",
                      "actionable"
                    ],
                    "properties": {
                      "schema_version": {
                        "const": "projection_integrity_value_v2"
                      },
                      "value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown",
                          "tampered"
                        ]
                      },
                      "known_v1_value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown"
                        ]
                      },
                      "actionable": {
                        "type": "boolean"
                      }
                    }
                  }
                }
              }
            }
          },
          {
            "type": "object",
            "additionalProperties": false,
            "required": [
              "outcome",
              "request_id",
              "replayed",
              "command_record_id",
              "audit_ids",
              "denial",
              "readback"
            ],
            "properties": {
              "outcome": {
                "const": "replayed"
              },
              "request_id": {
                "type": "string",
                "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{12}$"
              },
              "replayed": {
                "const": true
              },
              "command_record_id": {
                "type": "string",
                "minLength": 1
              },
              "audit_ids": {
                "type": "array",
                "items": {
                  "type": "string",
                  "minLength": 1
                },
                "maxItems": 8
              },
              "denial": {
                "type": "null"
              },
              "readback": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                  "proposal_revision_id",
                  "mode",
                  "mode_reason",
                  "audit_id",
                  "rollback_ttl_expires_at",
                  "projection_integrity_v1",
                  "projection_integrity_v2"
                ],
                "properties": {
                  "proposal_revision_id": {
                    "const": "P083-r65-refined-97d0ecda"
                  },
                  "mode": {
                    "enum": [
                      "disabled",
                      "permissive",
                      "enforce"
                    ]
                  },
                  "mode_reason": {
                    "type": "string",
                    "maxLength": 160
                  },
                  "audit_id": {
                    "type": [
                      "string",
                      "null"
                    ]
                  },
                  "rollback_ttl_expires_at": {
                    "type": [
                      "string",
                      "null"
                    ],
                    "format": "date-time"
                  },
                  "projection_integrity_v1": {
                    "enum": [
                      "fresh",
                      "stale",
                      "missing",
                      "unknown"
                    ]
                  },
                  "projection_integrity_v2": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                      "schema_version",
                      "value",
                      "known_v1_value",
                      "actionable"
                    ],
                    "properties": {
                      "schema_version": {
                        "const": "projection_integrity_value_v2"
                      },
                      "value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown",
                          "tampered"
                        ]
                      },
                      "known_v1_value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown"
                        ]
                      },
                      "actionable": {
                        "type": "boolean"
                      }
                    }
                  }
                }
              }
            }
          },
          {
            "type": "object",
            "additionalProperties": false,
            "required": [
              "outcome",
              "request_id",
              "replayed",
              "command_record_id",
              "audit_ids",
              "denial",
              "readback"
            ],
            "properties": {
              "outcome": {
                "const": "denied"
              },
              "request_id": {
                "type": "string",
                "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{12}$"
              },
              "replayed": {
                "const": false
              },
              "command_record_id": {
                "type": [
                  "string",
                  "null"
                ]
              },
              "audit_ids": {
                "type": "array",
                "items": {
                  "type": "string"
                },
                "maxItems": 0
              },
              "denial": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                  "code",
                  "request_id",
                  "operator_message",
                  "projection_integrity_v1",
                  "projection_integrity_v2",
                  "retry_after_seconds"
                ],
                "properties": {
                  "code": {
                    "enum": [
                      "unauthorized",
                      "stale_proposal_revision",
                      "invalid_transition",
                      "preflight_failed",
                      "hold_condition_present",
                      "rollback_ttl_invalid",
                      "idempotency_pending",
                      "request_reuse_mismatch",
                      "projection_not_fresh",
                      "invalid_reason_code"
                    ]
                  },
                  "request_id": {
                    "type": "string",
                    "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{12}$"
                  },
                  "operator_message": {
                    "type": "string",
                    "maxLength": 500
                  },
                  "projection_integrity_v1": {
                    "enum": [
                      "fresh",
                      "stale",
                      "missing",
                      "unknown"
                    ]
                  },
                  "projection_integrity_v2": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                      "schema_version",
                      "value",
                      "known_v1_value",
                      "actionable"
                    ],
                    "properties": {
                      "schema_version": {
                        "const": "projection_integrity_value_v2"
                      },
                      "value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown",
                          "tampered"
                        ]
                      },
                      "known_v1_value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown"
                        ]
                      },
                      "actionable": {
                        "type": "boolean"
                      }
                    }
                  },
                  "retry_after_seconds": {
                    "type": [
                      "integer",
                      "null"
                    ],
                    "minimum": 0,
                    "maximum": 3600
                  }
                }
              },
              "readback": {
                "type": "object",
                "additionalProperties": false,
                "required": [
                  "proposal_revision_id",
                  "mode",
                  "mode_reason",
                  "audit_id",
                  "rollback_ttl_expires_at",
                  "projection_integrity_v1",
                  "projection_integrity_v2"
                ],
                "properties": {
                  "proposal_revision_id": {
                    "const": "P083-r65-refined-97d0ecda"
                  },
                  "mode": {
                    "enum": [
                      "disabled",
                      "permissive",
                      "enforce"
                    ]
                  },
                  "mode_reason": {
                    "type": "string",
                    "maxLength": 160
                  },
                  "audit_id": {
                    "type": [
                      "string",
                      "null"
                    ]
                  },
                  "rollback_ttl_expires_at": {
                    "type": [
                      "string",
                      "null"
                    ],
                    "format": "date-time"
                  },
                  "projection_integrity_v1": {
                    "enum": [
                      "fresh",
                      "stale",
                      "missing",
                      "unknown"
                    ]
                  },
                  "projection_integrity_v2": {
                    "type": "object",
                    "additionalProperties": false,
                    "required": [
                      "schema_version",
                      "value",
                      "known_v1_value",
                      "actionable"
                    ],
                    "properties": {
                      "schema_version": {
                        "const": "projection_integrity_value_v2"
                      },
                      "value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown",
                          "tampered"
                        ]
                      },
                      "known_v1_value": {
                        "enum": [
                          "fresh",
                          "stale",
                          "missing",
                          "unknown"
                        ]
                      },
                      "actionable": {
                        "type": "boolean"
                      }
                    }
                  }
                }
              }
            }
          }
        ]
      }
    },
    "validation_fixtures": [
      "docs/evidence/083/api/mcp-rollback-input-schema-valid.fixture.json",
      "docs/evidence/083/api/mcp-rollback-enforce-target-rejected.fixture.json",
      "docs/evidence/083/api/mcp-result-additional-properties-rejected.fixture.json",
      "docs/evidence/083/api/graphql-caller-request-id-scalar.fixture.json",
      "docs/evidence/083/api/denial-shape-required-fields.fixture.json",
      "docs/evidence/083/api/graphql-mcp-enum-parity.fixture.json",
      "docs/evidence/083/api/mcp-result-oneof-committed.fixture.json",
      "docs/evidence/083/api/mcp-result-oneof-replayed.fixture.json",
      "docs/evidence/083/api/mcp-result-oneof-denied.fixture.json",
      "docs/evidence/083/api/mcp-result-denial-null-rejected.fixture.json",
      "docs/evidence/083/api/graphql-invalid-caller-request-id-scalar-error.fixture.json",
      "docs/evidence/083/api/mcp-invalid-request-id-input-validation-error.fixture.json",
      "docs/evidence/083/api/mcp-invalid-request-id-no-p083-result.fixture.json",
      "docs/evidence/083/api/mcp-invalid-request-id-no-side-effects.fixture.json",
      "docs/evidence/083/api/graphql-rollback-returns-p083-mutation-payload.fixture.json",
      "docs/evidence/083/api/no-p083-rollback-execution-payload-symbol.fixture.json",
      "docs/evidence/083/api/rfc3339-date-time-parser-valid.fixture.json",
      "docs/evidence/083/api/date-time-format-only-not-sufficient.fixture.json",
      "docs/evidence/083/api/date-time-parser-invalid-rejected.fixture.json"
    ],
    "mcp_oneof_result_rule": "p083.mutation.result is exactly one of committed, replayed, or denied. committed has outcome=committed, replayed=false, denial=null, command_record_id non-null, audit_ids non-empty, readback non-null. replayed has outcome=replayed, replayed=true, denial=null, command_record_id non-null, audit_ids present, readback non-null. denied has outcome=denied, replayed=false, denial object required, audit_ids empty, command_record_id nullable, and readback carries latest pre-mutation readback. additionalProperties=false applies at every object level.",
    "caller_request_id_behavior_split": "GraphQL inputs use scalar CallerRequestId. Malformed GraphQL values fail scalar parsing as GraphQL error code INVALID_CALLER_REQUEST_ID before resolver execution and produce no P083MutationPayload, no command_journal row, no command_idempotency row, and no audit row. MCP inputs are validated against the tool input JSON Schema before command acquisition; malformed MCP request_id fails that input-validation step, emits no p083.mutation.result payload, writes no lifecycle side effects, and returns the MCP transport/schema validation error shape declared in mcp_pre_result_validation_error_contract.",
    "graphql_payload_denial_codes": [
      "UNAUTHORIZED",
      "STALE_PROPOSAL_REVISION",
      "INVALID_TRANSITION",
      "PREFLIGHT_FAILED",
      "HOLD_CONDITION_PRESENT",
      "ROLLBACK_TTL_INVALID",
      "IDEMPOTENCY_PENDING",
      "REQUEST_REUSE_MISMATCH",
      "PROJECTION_NOT_FRESH",
      "INVALID_REASON_CODE"
    ],
    "mcp_payload_denial_codes": [
      "unauthorized",
      "stale_proposal_revision",
      "invalid_transition",
      "preflight_failed",
      "hold_condition_present",
      "rollback_ttl_invalid",
      "idempotency_pending",
      "request_reuse_mismatch",
      "projection_not_fresh",
      "invalid_reason_code"
    ],
    "mcp_pre_result_validation_error_contract": {
      "malformed_request_id_behavior": "reject_before_result_payload",
      "emits_p083_mutation_result": false,
      "side_effects": "none: no command_journal row, no command_idempotency row, no transition/audit row, no readback mutation",
      "error_shape": {
        "surface": "mcp_tool_input_validation_error",
        "code": "invalid_request_id",
        "field": "request_id",
        "schema_ref": "caller-request-id.v1",
        "operator_message": "request_id must be lowercase UUIDv4"
      },
      "fixtures": [
        "docs/evidence/083/api/mcp-invalid-request-id-input-validation-error.fixture.json",
        "docs/evidence/083/api/mcp-invalid-request-id-no-p083-result.fixture.json",
        "docs/evidence/083/api/mcp-invalid-request-id-no-side-effects.fixture.json"
      ]
    },
    "graphql_readback_audit_id_rule": "GraphQL P083EnforcementReadback.auditId is nullable. Default disabled, denied, and no-audit readbacks return auditId=null, matching MCP audit_id=null and required-with-null report lanes.",
    "rollback_return_contract_rule": "p083RollbackExecution and p083SetEnforcementMode both return P083MutationPayload. The symbol P083RollbackExecutionPayload is intentionally absent; schema generation and client fixtures fail if it appears.",
    "json_schema_format_assertion_policy": "JSON Schema format is annotation-only for Draft 2020-12 validation in this project. Every date-time field that must be asserted has an explicit parser validation fixture using the app/control-plane RFC3339 parser; schemas may retain format=date-time for documentation only."
  },
  "durable_monotonic_clock_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-005"
    ],
    "schema_version": "durable_monotonic_clock_v1",
    "authority": "SQLite durable_monotonic_clock_samples plus current host monotonic clock reading. Wall clock is audit/display only and never decides expiry by itself.",
    "stored_fields": [
      "sample_id",
      "boot_id",
      "process_start_id",
      "monotonic_ms",
      "wall_clock_utc",
      "observed_at_wall_clock",
      "sample_state"
    ],
    "boot_identity_rule": "boot_id is read from the platform boot-session identity when available and otherwise derived from monotonic reset detection plus process_start_id. A changed boot_id invalidates direct monotonic comparisons across boots.",
    "comparison_rule": "Same boot_id deadlines compare monotonic_ms directly. After daemon restart with same boot_id, recovery records a new sample and compares current monotonic_ms against stored deadline monotonic_ms. After boot_id change or uncertain monotonic reset, recovery fails closed to hold_or_recompute: idempotency leases become expired_recovery_required, shutdown deadlines become shutdown_interrupted with next_step_code=deadline_reconciliation_required, and heartbeat freshness becomes stale until refreshed.",
    "suspend_resume_rule": "Suspend/resume keeps same boot_id. If the platform monotonic clock includes sleep, deadlines may expire during sleep. If it excludes sleep, recovery also checks wall-clock audit delta and marks deadline_reconciliation_required when delta exceeds the configured cap.",
    "wall_clock_rule": "wall_clock_utc is retained only for receipts, audit rows, operator messages, and release receipts. Wall-clock jumps never extend a lease or deadline.",
    "fixtures": [
      "docs/evidence/083/clock/daemon-restart-same-boot-expired-lease.fixture.json",
      "docs/evidence/083/clock/reboot-boot-id-change-holds.fixture.json",
      "docs/evidence/083/clock/wall-clock-jump-does-not-extend.fixture.json",
      "docs/evidence/083/clock/suspend-resume-deadline-reconciles.fixture.json"
    ]
  },
  "shutdown_signal_side_effect_contract_v1": {
    "addresses": [
      "SCORE-LIFT-REL-P083-R64-001"
    ],
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
    "process_identity_guard": "A signal may be issued only when stored process_id and process_start_identity match the current OS process identity. Mismatch records identity_mismatch and holds with operator next_step_code=manual_process_identity_check.",
    "recovery_rules": {
      "crash_before_signal": "planned without issued_at is retried if identity still matches and deadline remains open; otherwise identity_mismatch or shutdown_interrupted is recorded.",
      "crash_after_signal_before_receipt": "on restart, if process is absent, mark observed; if identity still matches, continue observation or next signal wave; if identity differs, suppress duplicate and hold identity_mismatch.",
      "duplicate_signal": "same signal_kind/generation with issued state suppresses duplicate send and emits shutdown_duplicate_signal_suppressed_total.",
      "pid_reuse": "process_start_identity mismatch forbids signal."
    },
    "fixtures": [
      "docs/evidence/083/shutdown-signal/crash-before-sigterm-retries.fixture.json",
      "docs/evidence/083/shutdown-signal/crash-after-sigterm-before-receipt-recovers.fixture.json",
      "docs/evidence/083/shutdown-signal/pid-reuse-guard-denies.fixture.json",
      "docs/evidence/083/shutdown-signal/duplicate-signal-suppressed.fixture.json",
      "docs/evidence/083/shutdown-signal/kill-observation-timeout.fixture.json"
    ],
    "queue_rank_field": "shutdown_interrupted_receipts.queue_rank INTEGER NULL is non-null exactly for receipt_state=queued_no_signal rows. Its value is the deterministic queue rank (shutdown_epoch ASC, provider_session_id ASC) and is stored at the moment of queueing. It is exposed as p083_shutdown_queue_rank in MCP and p083ShutdownQueueRank in GraphQL, with run_report and release_receipt parity."
  },
  "lifecycle_window_migration_contract_v1": {
    "addresses": [
      "SCORE-LIFT-MACOS-P083-R64-002"
    ],
    "schema_version": "lifecycle_window_migration_contract_v1",
    "scope": "Lifecycle-bearing run/operator windows are coordinator-owned NSWindow instances with SwiftUI root views embedded in NSHostingController. SwiftUI WindowGroup scenes are allowed only for non-lifecycle utility windows such as Settings.",
    "coordinator_owned_windows": [
      "RunDetailLifecycleWindow",
      "StageExecutionLifecycleWindow",
      "ArtifactReportLifecycleWindow"
    ],
    "environment_injection": "LifecycleWindowCoordinator constructs lifecycle roots with command router, read-only projection store handle, openURL/openWindow adapters, focus store, operator preference bindings, and optional AppLocalModelContext. It does not inject the app-scoped lifecycle ModelContainer or the standard SwiftUI environment modelContext into lifecycle-bearing roots.",
    "swiftdata_rule": "SwiftData lifecycle truth remains backend/SQLite-owned. Coordinator-owned lifecycle roots receive LifecycleReadOnlyProjectionEnvironment and, only when needed, an AppLocalModelContext whose registered schemas exclude Run, StageExecution, AgentExecution, Approval, ProviderSession, command idempotency, shutdown, and lifecycle state models.",
    "command_routing": "Commands and menu validation route through LifecycleWindowCommandRouter keyed by lifecycle_window_id. Cmd-W closes only the focused lifecycle window; Cmd-, opens Settings through the non-lifecycle WindowGroup; File/New Window and Dock reopen ask the coordinator for an unordered lifecycle token before ordering; Edit/Copy uses the focused responder first and falls back to the focused copy control only when no text responder handles copy.",
    "restoration_and_tabbing": "Restoration stores lifecycle_window_id, run_id, and restoration_role. Restored windows are returned to the coordinator as unordered tokens and run pre-order hooks before makeKeyAndOrderFront. Automatic tabbing is disabled for lifecycle windows unless both windows share run_id and restoration_role.",
    "projection_subscription_lifetime": "Subscriptions are keyed by lifecycle_window_id plus run_id. Closing one window cancels only that key. Multiple windows for the same run may share decoded snapshots but have independent UI publication and teardown.",
    "fixtures": [
      "docs/evidence/083/swift/lifecycle-window-environment-injection.fixture.json",
      "docs/evidence/083/swift/lifecycle-window-command-routing-cmdw.fixture.json",
      "docs/evidence/083/swift/dock-reactivation-unordered-token.fixture.json",
      "docs/evidence/083/swift/second-run-window-independent-subscription.fixture.json",
      "docs/evidence/083/swift/settings-windowgroup-exclusion.fixture.json",
      "docs/evidence/083/swift/nssavepanel-sheet-modal-active-window.fixture.json",
      "docs/evidence/083/swift/responder-chain-copy-not-hijacked.fixture.json",
      "docs/evidence/083/swift/lifecycle-root-app-scoped-modelcontext-leakage-rejected.fixture.json",
      "docs/evidence/083/swift/lifecycle-root-readonly-projection-environment.fixture.json",
      "docs/evidence/083/swift/focused-window-command-routing.fixture.json",
      "docs/evidence/083/swift/sheet-modal-export-active-window.fixture.json",
      "docs/evidence/083/swift/current-host-only-clipboard-wording.fixture.json"
    ],
    "model_context_lifetime_rule": "Lifecycle roots are constructed on MainActor. They receive read-only projection dependencies or an app-local-only ModelContext; they never receive environment(\\.modelContext) backed by the app-scoped lifecycle container. Autosave is disabled for lifecycle roots unless the context is app-local-only and excludes lifecycle schemas.",
    "export_text_sheet_rule": "Export Text uses NSSavePanel.beginSheetModal(for:) on the focused lifecycle NSWindow. On confirmation, the handler revalidates lifecycle_window_id, run_id, projection_integrity == fresh, and backend actionability. Host window close, run mismatch, or stale projection during the sheet cancels or returns a typed denial without writing.",
    "phased_windowgroup_coexistence_rule": "Phase 1 keeps ContentView/RunsHome shell navigation and Settings in SwiftUI WindowGroup as non-lifecycle surfaces. Run/detail windows that expose lifecycle mutation affordances or execution truth move to coordinator-owned NSHostingController roots. A WindowGroup-hosted view may link to a lifecycle window only by asking LifecycleWindowCoordinator for an unordered token.",
    "sandboxed_export_rule": "For sandboxed or hardened-runtime builds, Export Text uses NSSavePanel.beginSheetModal(for:) and writes only to the selected URL after obtaining security-scoped access when required. Failure to obtain access returns a typed denial and writes no partial file.",
    "multi_window_menu_validation_fixture": "docs/evidence/083/swift/multi-window-menu-validation-key-window.fixture.json",
    "pending_terminal_accessibility_fixture": "docs/evidence/083/swift/pending-to-terminal-accessibility-announcement.fixture.json",
    "lifecycle_bearing_view_criteria": "A view is lifecycle-bearing if it renders run/stage/agent/provider lifecycle state, mutation affordances, approval actions, shutdown/cancel state, recovery next steps, or execution-truth report controls. Lifecycle-bearing detailed readback surfaces must be coordinator-owned. Read-only aggregate counts and shell navigation may remain in WindowGroup when they expose no mutation controls and link to details through LifecycleWindowCoordinator.",
    "coexistence_disabled_controls": "During phased coexistence, WindowGroup summary rows disable lifecycle mutation controls and replace them with Open Detail actions that request a coordinator-owned lifecycle window.",
    "macos_command_fixture_rule": "Implementation gates validate focused-window dispatch for Copy, Export Text..., Cmd-W, and lifecycle commands, plus accessible pending-to-terminal announcements for committed, replayed, denied, and failed outcomes."
  },
  "cancellation_backpressure_authority_v1": {
    "addresses": [],
    "schema_version": "cancellation_backpressure_authority_v1",
    "cancellation_epoch_source": "provider_sessions.cancellation_epoch is incremented only by the authoritative cancellation transaction and is copied to late-output and overflow rows.",
    "reservation_owner": "ACP read-loop reservations are owned by provider_session_id plus read_epoch. Startup repair releases reservations whose read_epoch is not live after process identity verification.",
    "cutoff_rule": "Live sessions that exceed cap defaults enter backpressure_cutoff as an authoritative cancellation trigger and must create/resume shutdown_epoch under backpressure_cutoff_process_contract_v1 before terminal readback.",
    "fixtures": [
      "docs/evidence/083/backpressure/cancellation-epoch-authority.fixture.json",
      "docs/evidence/083/backpressure/restart-releases-leaked-reservation.fixture.json",
      "docs/evidence/083/backpressure/live-output-backpressure-cutoff.fixture.json"
    ]
  },
  "recovery_repair_matrix_v1": {
    "addresses": [
      "SCORE-LIFT-REL-P083-R64-002",
      "SCORE-LIFT-REL-P083-R64-003"
    ],
    "schema_version": "recovery_repair_matrix_v1",
    "rows": [
      {
        "durable_state": "command_idempotency.pending",
        "startup_action": "apply durable_monotonic_clock_v1 and reacquire_transaction_v1 or return typed pending denial",
        "idempotency_key": "principal_id,request_id,lease_generation",
        "deadline_source": "expires_at_monotonic_ms",
        "retry_cap": "one reacquire per expired generation",
        "terminal_fallback": "failed",
        "operator_next_step_code": "retry_command_with_same_request_id"
      },
      {
        "durable_state": "provider_cancellation_intents.requested_or_shutdown_started with provider_sessions.live",
        "startup_action": "join provider_cancellation_intents to provider_sessions and resume the durable shutdown_epoch; do not infer intent from lifecycle_state text",
        "idempotency_key": "provider_session_id,cancellation_epoch,shutdown_epoch",
        "deadline_source": "shutdown host deadline monotonic sample",
        "retry_cap": "one graceful and one kill generation per epoch",
        "terminal_fallback": "hold_until_process_fate_absent_or_receipt_recorded",
        "operator_next_step_code": "inspect_provider_shutdown"
      },
      {
        "durable_state": "shutdown_interrupted_receipts.queued_no_signal",
        "startup_action": "verify process absence or open a new shutdown epoch; exact receipt reuse requires provider_session_id, shutdown_epoch, and receipt_generation",
        "idempotency_key": "provider_session_id,shutdown_epoch,receipt_generation",
        "deadline_source": "durable_monotonic_clock_v1",
        "retry_cap": "one recovery retry before hold",
        "terminal_fallback": "shutdown_interrupted",
        "operator_next_step_code": "manual_process_identity_check"
      },
      {
        "durable_state": "cancel_late_output_overflow latched",
        "startup_action": "reload canonical latch by scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, and overflow_kind; update counters in place and release leaked reservations for the matching scope without inserting duplicate rows",
        "idempotency_key": "scope,normalized_run_id,normalized_provider_session_id,cancellation_epoch,overflow_kind",
        "deadline_source": "max_elapsed_seconds_after_cancellation",
        "retry_cap": "not_applicable",
        "terminal_fallback": "backpressure_cutoff",
        "operator_next_step_code": "review_late_output_quarantine"
      },
      {
        "durable_state": "p083_enforcement_mode_transition_journal.transitioning",
        "startup_action": "recover from transition_journal joined to command_idempotency by principal_id, request_id, and lease_generation; committed command completes to committed, denied/failed command leaves mode at from_mode and records recovered, missing command restores from_mode with recovered transition state",
        "idempotency_key": "principal_id,request_id,lease_generation,transition_id",
        "deadline_source": "none",
        "retry_cap": "not_applicable",
        "terminal_fallback": "from_mode",
        "operator_next_step_code": "rerun_enforcement_preflight"
      },
      {
        "durable_state": "stage_executions.running",
        "startup_action": "delegate to existing RecoveryService stage repair and attach P083 operator next_step_code when repair cannot settle",
        "idempotency_key": "stage_execution_id",
        "deadline_source": "existing stage watchdog deadline",
        "retry_cap": "existing RecoveryService policy",
        "terminal_fallback": "stage_failed_recovery_required",
        "operator_next_step_code": "inspect_stage_recovery"
      },
      {
        "durable_state": "agent_executions.running",
        "startup_action": "delegate to existing RecoveryService agent/provider session repair, then reconcile provider_session lifecycle vocabulary",
        "idempotency_key": "agent_execution_id",
        "deadline_source": "existing agent watchdog deadline",
        "retry_cap": "existing RecoveryService policy",
        "terminal_fallback": "agent_failed_recovery_required",
        "operator_next_step_code": "inspect_agent_recovery"
      },
      {
        "durable_state": "shutdown_signal_side_effects.planned",
        "startup_action": "if process identity matches and deadline open, issue signal and mark issued; otherwise mark identity_mismatch or shutdown_interrupted",
        "idempotency_key": "provider_session_id,shutdown_epoch,signal_kind,generation",
        "deadline_source": "durable_monotonic_clock_v1",
        "retry_cap": "one issue attempt per generation",
        "terminal_fallback": "shutdown_interrupted",
        "operator_next_step_code": "inspect_shutdown_signal"
      },
      {
        "durable_state": "shutdown_signal_side_effects.issued",
        "startup_action": "observe process exit, suppress duplicate signal, or advance to next signal wave after deadline",
        "idempotency_key": "provider_session_id,shutdown_epoch,signal_kind,generation",
        "deadline_source": "issued_at_monotonic_ms",
        "retry_cap": "one observation window per generation",
        "terminal_fallback": "shutdown_interrupted",
        "operator_next_step_code": "inspect_shutdown_signal"
      },
      {
        "durable_state": "provider_cancellation_intents.requested with shutdown_epoch IS NULL and ambiguous process identity",
        "startup_action": "hold intent (do not assign shutdown_epoch); set process_fate=identity_ambiguous; never settle terminal until operator resolves identity",
        "idempotency_key": "provider_session_id,cancellation_epoch",
        "deadline_source": "none",
        "retry_cap": "no automatic retry; held until manual resolution",
        "terminal_fallback": "held",
        "operator_next_step_code": "manual_process_identity_check"
      }
    ],
    "fixtures": [
      "docs/evidence/083/recovery/repair-matrix-nonterminal-states.fixture.json",
      "docs/evidence/083/recovery/queued-no-signal-repair.fixture.json",
      "docs/evidence/083/recovery/enforcement-transition-repair.fixture.json",
      "docs/evidence/083/recovery/transition-journal-command-idempotency-repair.fixture.json",
      "docs/evidence/083/recovery/no-audit-row-inference-for-transition-repair.fixture.json",
      "docs/evidence/083/recovery/session-overflow-restart-no-duplicate.fixture.json",
      "docs/evidence/083/recovery/run-overflow-restart-no-duplicate.fixture.json",
      "docs/evidence/083/recovery/global-overflow-restart-no-duplicate.fixture.json",
      "docs/evidence/083/recovery/overflow-restart-releases-leaked-reservation.fixture.json"
    ],
    "alignment_rule": "Recovery for enforcement transitions is based on p083_enforcement_mode_transition_journal plus command_idempotency, not stale state-table flags or audit-row inference. State table readback is derived only after transition recovery settles.",
    "overflow_recovery_key_rule": "Recovery uses the same canonical latch key as storage for every scope: scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind. provider_session_id-only overflow recovery keys are forbidden."
  },
  "caller_request_id_graphql_scalar_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-005"
    ],
    "schema_version": "caller_request_id_graphql_scalar_v1",
    "graphql_scalar": "scalar CallerRequestId",
    "parse_value_rule": "Accept only lowercase UUIDv4 matching ^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{12}$. Reject uppercase, braced, urn-prefixed, whitespace-padded, non-v4, and non-string inputs.",
    "serialize_rule": "Serialize stored CallerRequestId as the same lowercase canonical UUID string. Serialization of malformed stored values fails closed and redacts the value from operator surfaces.",
    "error_contract": {
      "graphql_error_code": "INVALID_CALLER_REQUEST_ID",
      "http_status": 200,
      "side_effects": "no command_journal row, no command_idempotency row, no lifecycle mutation, no audit transition row"
    },
    "fixtures": [
      "docs/evidence/083/api/caller-request-id-scalar-valid.fixture.json",
      "docs/evidence/083/api/caller-request-id-scalar-uppercase-rejected.fixture.json",
      "docs/evidence/083/api/caller-request-id-scalar-no-side-effects.fixture.json",
      "docs/evidence/083/api/mcp-invalid-request-id-input-validation-error.fixture.json",
      "docs/evidence/083/api/mcp-invalid-request-id-no-p083-result.fixture.json",
      "docs/evidence/083/api/mcp-invalid-request-id-no-side-effects.fixture.json"
    ],
    "payload_denial_rule": "GraphQL scalar parse failures are GraphQL request errors, not P083MutationPayload denials. INVALID_CALLER_REQUEST_ID is reserved for GraphQL error extensions.code only; MCP malformed request_id is an input-validation error before any p083.mutation.result payload exists.",
    "mcp_boundary_rule": "MCP request_id uses the same lowercase UUIDv4 pattern in each input JSON Schema. Pattern mismatch fails input validation before resolver execution and before command idempotency acquisition."
  },
  "projection_integrity_observed_vocabulary_v1": {
    "addresses": [
      "SCORE-LIFT-APPLE-P083-R64-001"
    ],
    "schema_version": "projection_integrity_observed_vocabulary_v1",
    "v1_single_source": "ProjectionIntegrity v1 is closed to fresh, stale, missing, unknown for P083-r61. No active contract, fixture, metric, or SDL may add tampered to v1 without deployed evidence proving it already exists.",
    "table": [
      {
        "stored_value": "fresh",
        "graphql_legacy_projectionIntegrity": "FRESH",
        "mcp_projection_integrity_v1": "fresh",
        "projectionIntegrityV2.value": "fresh",
        "projectionIntegrityV2.knownV1Value": "FRESH",
        "actionable": true
      },
      {
        "stored_value": "stale",
        "graphql_legacy_projectionIntegrity": "STALE",
        "mcp_projection_integrity_v1": "stale",
        "projectionIntegrityV2.value": "stale",
        "projectionIntegrityV2.knownV1Value": "STALE",
        "actionable": false
      },
      {
        "stored_value": "missing",
        "graphql_legacy_projectionIntegrity": "MISSING",
        "mcp_projection_integrity_v1": "missing",
        "projectionIntegrityV2.value": "missing",
        "projectionIntegrityV2.knownV1Value": "MISSING",
        "actionable": false
      },
      {
        "stored_value": "unknown",
        "graphql_legacy_projectionIntegrity": "UNKNOWN",
        "mcp_projection_integrity_v1": "unknown",
        "projectionIntegrityV2.value": "unknown",
        "projectionIntegrityV2.knownV1Value": "UNKNOWN",
        "actionable": false
      },
      {
        "stored_value": "tampered",
        "graphql_legacy_projectionIntegrity": "UNKNOWN",
        "mcp_projection_integrity_v1": "unknown",
        "projectionIntegrityV2.value": "tampered",
        "projectionIntegrityV2.knownV1Value": "UNKNOWN",
        "actionable": false
      }
    ],
    "fixtures": [
      "docs/evidence/083/api/projection-integrity-v1-closed.fixture.json",
      "docs/evidence/083/api/tampered-v2-only.fixture.json",
      "docs/evidence/083/api/no-tampered-in-legacy-sdl.fixture.json"
    ]
  },
  "enforcement_transition_state_contract_v1": {
    "addresses": [],
    "schema_version": "enforcement_transition_state_contract_v1",
    "table": "p083_enforcement_mode_transition_journal",
    "columns": [
      "transition_id TEXT PRIMARY KEY",
      "proposal_id TEXT NOT NULL",
      "proposal_revision_id TEXT NOT NULL",
      "from_mode TEXT NOT NULL",
      "to_mode TEXT NOT NULL",
      "request_id TEXT NOT NULL",
      "principal_id TEXT NOT NULL",
      "transition_state TEXT NOT NULL CHECK(transition_state IN ('transitioning','committed','denied','recovered'))",
      "preflight_hash TEXT",
      "audit_id TEXT",
      "commit_marker TEXT",
      "created_at TEXT NOT NULL",
      "updated_at TEXT NOT NULL"
    ],
    "state_rule": "p083_enforcement_mode_state.mode changes only after a transition_journal row has transition_state=committed and non-null commit_marker. A transitioning row after restart is settled by reloading command_idempotency: committed command completes to committed, denied command settles to denied, missing/expired command restores from_mode and records recovered.",
    "verification_queries": [
      "SELECT transition_id FROM p083_enforcement_mode_transition_journal WHERE transition_state='committed' AND commit_marker IS NULL;",
      "SELECT transition_id FROM p083_enforcement_mode_transition_journal WHERE transition_state='transitioning' AND updated_at < datetime('now', '-1 hour');"
    ],
    "fixtures": [
      "docs/evidence/083/rollout/enforcement-transition-committed-marker.fixture.json",
      "docs/evidence/083/rollout/enforcement-transition-restart-recovers-from-mode.fixture.json",
      "docs/evidence/083/rollout/enforcement-transition-denied-no-mode-change.fixture.json"
    ]
  },
  "backpressure_cutoff_process_contract_v1": {
    "addresses": [],
    "schema_version": "backpressure_cutoff_process_contract_v1",
    "cap_defaults": {
      "max_frame_bytes": 1048576,
      "max_session_bytes": 16777216,
      "max_run_bytes": 67108864,
      "max_global_bytes": 268435456,
      "max_messages_per_second_per_session": 20
    },
    "hard_limits": {
      "max_frame_bytes": 4194304,
      "max_session_bytes": 67108864,
      "max_run_bytes": 268435456,
      "max_global_bytes": 1073741824
    },
    "authority_rule": "backpressure_cutoff is an authoritative cancellation trigger for a live provider session. Entering backpressure_cutoff increments or reuses provider_sessions.cancellation_epoch, creates or resumes shutdown_epoch, and writes shutdown_signal_side_effects before terminal settlement unless process absence is verified.",
    "process_fate_rule": "If process identity still matches after backpressure caps are exceeded, set process_fate=backpressure_cutoff_shutdown_pending, create/resume shutdown_epoch, emit metrics, and keep recovery nonterminal. Terminal settlement requires absent_verified or interrupted_receipt_recorded. identity_ambiguous holds with manual_process_identity_check.",
    "readback_fields": [
      "backpressure_cutoff_reason",
      "backpressure_cap_kind",
      "backpressure_observed_value",
      "cancellation_epoch",
      "shutdown_epoch",
      "process_fate",
      "latest_shutdown_receipt_id"
    ],
    "restart_fixtures": [
      "docs/evidence/083/backpressure/live-backpressure-before-cancel-starts-shutdown.fixture.json",
      "docs/evidence/083/backpressure/restart-backpressure-live-process-resumes-shutdown.fixture.json",
      "docs/evidence/083/backpressure/backpressure-process-absent-terminal.fixture.json",
      "docs/evidence/083/backpressure/hard-limit-denied-at-config-load.fixture.json",
      "docs/evidence/083/backpressure/process-fate-migration-positive.fixture.json",
      "docs/evidence/083/backpressure/restart-pending-backpressure-resumes-shutdown.fixture.json",
      "docs/evidence/083/backpressure/lifecycle-state-cancellation-wording-rejected.fixture.json"
    ],
    "canonical_model": "Provider lifecycle_state remains one of provider_lifecycle_vocabulary_authority_v1.canonical_values; cancellation_requested and backpressure_cutoff_shutdown_pending are never lifecycle states. Pending backpressure is stored in provider_sessions.process_fate and provider_cancellation_intents, then terminal lifecycle settlement waits for process_fate=absent_verified or process_fate=interrupted_receipt_recorded.",
    "process_fate_vocabulary": [
      "running",
      "backpressure_cutoff_shutdown_pending",
      "absent_verified",
      "interrupted_receipt_recorded",
      "identity_ambiguous"
    ],
    "metric_rule": "provider_session_lifecycle_total does not use backpressure_cutoff_shutdown_pending as lifecycle_state. Metrics use backpressure_process_fate_total{provider,process_fate} for pending/terminal process fate.",
    "restart_terminal_settlement_rule": "On restart, backpressure_cutoff_shutdown_pending reloads process identity from durable provider_cancellation_intents and shutdown epochs. Matching live process resumes shutdown; absent process sets process_fate=absent_verified before terminal settlement; interrupted receipt sets process_fate=interrupted_receipt_recorded; ambiguous identity records a hold and never settles terminal.",
    "cancellation_intent_rule": "Entering backpressure_cutoff writes provider_cancellation_intents with reason=backpressure_cutoff and intent_state=requested, then starts or resumes shutdown_epoch. cancellation_requested is never written as provider lifecycle_state.",
    "recovery_matrix_fixture": "docs/evidence/083/backpressure/backpressure-process-fate-recovery-matrix.fixture.json",
    "process_fate_storage_rule": "process_fate is durable storage on provider_sessions with values running, backpressure_cutoff_shutdown_pending, absent_verified, interrupted_receipt_recorded, or identity_ambiguous. Recovery updates this field transactionally with shutdown receipt or process-absence evidence."
  },
  "rollout_readback_api_parity_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-001",
      "SCORE-LIFT-API-P083-R64-002",
      "SCORE-LIFT-API-P083-R64-005",
      "SCORE-LIFT-REL-P083-R64-001"
    ],
    "schema_version": "rollout_readback_api_parity_v1",
    "vocabularies": {
      "rollout_status": [
        "pass",
        "fail",
        "waived",
        "not_applicable",
        "timeout",
        "cancelled",
        "missing_contract",
        "tamper_detected",
        "stale"
      ],
      "rollout_decision": [
        "release",
        "hold",
        "waive",
        "not_applicable"
      ],
      "enforcement_mode": [
        "disabled",
        "permissive",
        "enforce"
      ],
      "mutation_outcome": [
        "committed",
        "replayed",
        "denied"
      ],
      "enabled_state": [
        "enabled",
        "disabled"
      ]
    },
    "normalization_rules": [
      "run_report, mcp, and release_receipt use snake_case keys exactly matching operator_readback_v1.",
      "GraphQL uses camelCase key projection with the same string values.",
      "Unknown rollout_status values map to stale and append unknown_rollout_status to failure_reasons.",
      "Unknown rollout_decision values map to hold.",
      "Unknown enforcement_mode values map to disabled and disabled_reason_code=unknown_enforcement_mode.",
      "Nullability is identical across lanes: waiver_expires_at, action_id, disabled_reason_code, rollback_ttl_expires_at, and last_preflight_hash may be null; status, decision, enforcement_mode, enabled_state, projection_integrity, and next_steps are non-null.",
      "Mutation outcome is never used as rollout decision; enforcement mode is never used as rollout status."
    ],
    "graphql_sdl": [
      "scalar RollbackDispositionJSON",
      "type RolloutContractReadback { rolloutContractStatus: String! rolloutContractDecision: String! rolloutContractFailureReasons: [String!]! rolloutContractWaiverState: String! rolloutContractWaiverExpiresAt: String rolloutContractEnforcementMode: String! rolloutContractEnforcementModeReason: String! rolloutContractHoldConditions: [String!]! rolloutContractRollbackDisposition: RollbackDispositionJSON! rolloutContractSourceLane: String! rolloutContractEnabledState: String! rolloutContractDisabledReasonCode: String rolloutContractActionId: String rolloutContractOperatorMessage: String! rolloutContractProjectionIntegrity: String! rolloutContractCutoverPolicyRevision: String! rolloutContractDiagnosticRedaction: String! rolloutContractNextSteps: [String!]! rolloutContractShutdownDeadlineConfigState: String! rolloutContractCommandLeaseTtlConfigState: String! p083RollbackTtlExpiresAt: String p083LastPreflightHash: String p083ShutdownQueueRank: Int }"
    ],
    "mcp_schema": {
      "$schema": "https://json-schema.org/draft/2020-12/schema",
      "type": "object",
      "additionalProperties": false,
      "required": [
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
      "properties": {
        "rollout_contract_status": {
          "enum": [
            "pass",
            "fail",
            "waived",
            "not_applicable",
            "timeout",
            "cancelled",
            "missing_contract",
            "tamper_detected",
            "stale"
          ]
        },
        "rollout_contract_decision": {
          "enum": [
            "release",
            "hold",
            "waive",
            "not_applicable"
          ]
        },
        "rollout_contract_failure_reasons": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "rollout_contract_waiver_state": {
          "type": "string"
        },
        "rollout_contract_waiver_expires_at": {
          "type": [
            "string",
            "null"
          ]
        },
        "rollout_contract_enforcement_mode": {
          "enum": [
            "disabled",
            "permissive",
            "enforce"
          ]
        },
        "rollout_contract_enforcement_mode_reason": {
          "type": "string"
        },
        "rollout_contract_hold_conditions": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "rollout_contract_rollback_disposition": {
          "type": "object",
          "additionalProperties": false,
          "required": [
            "schema_version",
            "mode",
            "data_loss_risk",
            "steps"
          ],
          "properties": {
            "schema_version": {
              "const": "rollback_disposition_v1"
            },
            "mode": {
              "type": "string"
            },
            "data_loss_risk": {
              "enum": [
                "none",
                "low",
                "medium",
                "high"
              ]
            },
            "steps": {
              "type": "array",
              "items": {
                "type": "string"
              },
              "minItems": 1
            }
          }
        },
        "rollout_contract_source_lane": {
          "enum": [
            "run_report",
            "mcp",
            "release_receipt",
            "graphql"
          ]
        },
        "rollout_contract_enabled_state": {
          "enum": [
            "enabled",
            "disabled"
          ]
        },
        "rollout_contract_disabled_reason_code": {
          "type": [
            "string",
            "null"
          ]
        },
        "rollout_contract_action_id": {
          "type": [
            "string",
            "null"
          ]
        },
        "rollout_contract_operator_message": {
          "type": "string"
        },
        "rollout_contract_projection_integrity": {
          "enum": [
            "fresh",
            "stale",
            "missing",
            "unknown"
          ]
        },
        "rollout_contract_cutover_policy_revision": {
          "type": "string"
        },
        "rollout_contract_diagnostic_redaction": {
          "type": "string"
        },
        "rollout_contract_next_steps": {
          "type": "array",
          "items": {
            "type": "string"
          }
        },
        "p083_rollback_ttl_expires_at": {
          "type": [
            "string",
            "null"
          ]
        },
        "p083_last_preflight_hash": {
          "type": [
            "string",
            "null"
          ]
        },
        "rollout_contract_shutdown_deadline_config_state": {
          "enum": [
            "valid",
            "invalid",
            "unknown"
          ]
        },
        "rollout_contract_command_lease_ttl_config_state": {
          "enum": [
            "valid",
            "invalid",
            "unknown"
          ]
        },
        "p083_shutdown_queue_rank": {
          "type": [
            "integer",
            "null"
          ],
          "minimum": 0
        }
      }
    },
    "fixtures": [
      "docs/evidence/083/rollout/graphql-mcp-readback-parity.fixture.json",
      "docs/evidence/083/rollout/unknown-status-normalizes-to-stale.fixture.json",
      "docs/evidence/083/rollout/vocabulary-domains-separated.fixture.json",
      "docs/evidence/083/rollout/mcp-nullable-fields-explicit-null-pass.fixture.json",
      "docs/evidence/083/rollout/mcp-nullable-field-omitted-fails.fixture.json",
      "docs/evidence/083/rollout/graphql-nullable-fields-explicit-null-pass.fixture.json",
      "docs/evidence/083/rollout/run-report-release-receipt-null-parity.fixture.json",
      "docs/evidence/083/rollout/shutdown-deadline-config-state-graphql-lane.fixture.json",
      "docs/evidence/083/rollout/shutdown-deadline-config-state-mcp-lane.fixture.json",
      "docs/evidence/083/rollout/shutdown-deadline-config-state-run-report-lane.fixture.json",
      "docs/evidence/083/rollout/shutdown-deadline-config-state-release-receipt-lane.fixture.json",
      "docs/evidence/083/rollout/command-lease-ttl-config-state-graphql-lane.fixture.json",
      "docs/evidence/083/rollout/command-lease-ttl-config-state-mcp-lane.fixture.json",
      "docs/evidence/083/rollout/command-lease-ttl-config-state-run-report-lane.fixture.json",
      "docs/evidence/083/rollout/command-lease-ttl-config-state-release-receipt-lane.fixture.json",
      "docs/evidence/083/rollout/rollback-disposition-graphql-mcp-byte-equal.fixture.json",
      "docs/evidence/083/rollout/rollback-disposition-mcp-additional-properties-rejected.fixture.json",
      "docs/evidence/083/rollout/shutdown-queue-rank-graphql-mcp-parity.fixture.json"
    ],
    "required_with_null_rule": "Every declared rollout and P083 readback field is required in run_report, MCP, release_receipt, and GraphQL projections. Nullable fields must be present with explicit null. This includes audit_id/auditId, rollback_ttl_expires_at, waiver_expires_at, action_id, disabled_reason_code, last_preflight_hash, and p083_shutdown_queue_rank. The two reliability config-state fields, rollout_contract_shutdown_deadline_config_state and rollout_contract_command_lease_ttl_config_state, are required and non-null in every lane: their value is one of valid, invalid, or unknown.",
    "rollout_contract_v1_decision_vocabulary_rule": "rollout_contract_v1.decision_vocabulary is legacy lint input accepted by the rollout-contract validator. Runtime readback normalizes through rollout_readback_api_parity_v1.vocabularies, which separates rollout_status, rollout_decision, enforcement_mode, mutation_outcome, and enabled_state.",
    "audit_id_nullability_rule": "GraphQL P083EnforcementReadback.auditId is nullable and maps one-to-one to MCP audit_id. All readback lanes include the field; no-audit/default/denied states use explicit null instead of omitting the key or inventing an initial audit row.",
    "rollback_disposition_schema_policy": {
      "strategy": "opaque_versioned_object",
      "schema_version": "rollback_disposition_v1",
      "single_graphql_type_rule": "Exactly one symbol is used across the entire SDL surface: scalar RollbackDispositionJSON. The undefined RollbackDisposition reference is not permitted anywhere in this proposal, generated schema, or fixtures.",
      "graphql_sdl": "scalar RollbackDispositionJSON; type RolloutContractReadback { rolloutContractRollbackDisposition: RollbackDispositionJSON! }",
      "mcp_json_schema": {
        "type": "object",
        "additionalProperties": false,
        "required": [
          "schema_version",
          "mode",
          "data_loss_risk",
          "steps"
        ],
        "properties": {
          "schema_version": {
            "const": "rollback_disposition_v1"
          },
          "mode": {
            "type": "string"
          },
          "data_loss_risk": {
            "enum": [
              "none",
              "low",
              "medium",
              "high"
            ]
          },
          "steps": {
            "type": "array",
            "items": {
              "type": "string"
            },
            "minItems": 1
          }
        }
      },
      "graphql_mcp_parity_rule": "The MCP JSON Schema above and the property rollout_contract_rollback_disposition in rollout_readback_api_parity_v1.mcp_schema are byte-equal versioned shapes. Both reject additionalProperties. The GraphQL scalar RollbackDispositionJSON carries the same versioned JSON value and rejects extra properties at the parser layer.",
      "fixtures": [
        "docs/evidence/083/api/rollback-disposition-versioned-object-graphql.fixture.json",
        "docs/evidence/083/api/rollback-disposition-versioned-object-mcp.fixture.json",
        "docs/evidence/083/api/rollback-disposition-mcp-extra-property-rejected.fixture.json",
        "docs/evidence/083/api/rollback-disposition-graphql-rolloutdisposition-symbol-absent.fixture.json"
      ]
    },
    "graphql_date_time_validation_policy": "GraphQL date-time fields use the String scalar; date-time format is asserted by the same RFC3339 parser fixtures listed in api_mutation_contracts_v1.validation_fixtures. GraphQL SDL type identity does not enforce date-time format. If a DateTime scalar is later introduced, it must wrap the same RFC3339 parser and the fixtures must continue to pass byte-equal.",
    "readback_field_lane_coverage_rule": "Every field declared in rollout_contract_v1.readback_fields is required in all four lanes (graphql, mcp, run_report, release_receipt). Lint compares the SDL field set, the MCP required list, the run_report parity column set, and the release_receipt parity column set against this single readback_fields registry and fails on any missing field. Specifically, rollout_contract_shutdown_deadline_config_state and rollout_contract_command_lease_ttl_config_state appear in all four lanes and in every lane-coverage fixture below.",
    "config_state_vocabulary": {
      "rollout_contract_shutdown_deadline_config_state": [
        "valid",
        "invalid",
        "unknown"
      ],
      "rollout_contract_command_lease_ttl_config_state": [
        "valid",
        "invalid",
        "unknown"
      ]
    }
  },
  "swiftdata_lifecycle_boundary_contract_v1": {
    "addresses": [],
    "schema_version": "swiftdata_lifecycle_boundary_contract_v1",
    "write_path_inventory": [
      {
        "surface": "Run @Model writes",
        "classification": "forbidden_lifecycle_truth_write",
        "migration": "all lifecycle mutations move to Rust/SQLite readback; SwiftData may cache read-only projection snapshots only"
      },
      {
        "surface": "StageExecution/AgentExecution @Model writes",
        "classification": "forbidden_lifecycle_truth_write",
        "migration": "replace direct ModelContext mutation with projection refresh from backend readback"
      },
      {
        "surface": "Approval action writes",
        "classification": "forbidden_lifecycle_truth_write",
        "migration": "approval resolution uses backend mutation plus CallerRequestId; SwiftData updates only after readback"
      },
      {
        "surface": "Artifact metadata/report rows",
        "classification": "allowed_evidence_metadata",
        "migration": "Swift may register local evidence metadata only when artifact content is already canonical on disk and backend lineage readback remains authority"
      },
      {
        "surface": "Recovery/readback UI state",
        "classification": "allowed_app_local_read_model",
        "migration": "local expanded/collapsed state, cursors, and filters may write to SwiftData if marked app_local_non_authoritative"
      },
      {
        "surface": "Session lineage/provider lifecycle",
        "classification": "forbidden_lifecycle_truth_write",
        "migration": "provider_sessions and lifecycle states are backend readback only"
      },
      {
        "surface": "Legacy tests/fixtures",
        "classification": "legacy_test_fixture_only",
        "migration": "test factories may construct models but production scans fail direct lifecycle ModelContext saves"
      }
    ],
    "guardrails": [
      "Static scan rejects ModelContext.insert/save/delete touching Run, StageExecution, AgentExecution, Approval, ProviderSession, or lifecycle fields outside approved projection adapters.",
      "Approved adapters must be named in swift_projection_mapping_manifest_v1 and marked readback_cache_only.",
      "Runtime assertion in debug builds rejects lifecycle ModelContext save when mutation_origin != backend_readback_projection.",
      "Static scan rejects lifecycle @Model registration in the app-scoped ModelContainer used by retained WindowGroup surfaces.",
      "Static scan rejects autosave-enabled ModelContext instances whose schema contains lifecycle truth models.",
      "Production guard rejects retained WindowGroup writes unless mutation_origin is app_local_non_authoritative or backend_readback_projection and target schema is app-local/projection-only."
    ],
    "fixtures": [
      "docs/evidence/083/swift/swiftdata-lifecycle-write-scan.fixture.json",
      "docs/evidence/083/swift/modelcontext-approved-readback-adapter.fixture.json",
      "docs/evidence/083/swift/approval-direct-save-rejected.fixture.json",
      "docs/evidence/083/swift/lifecycle-root-no-standard-modelcontext.fixture.json",
      "docs/evidence/083/swift/direct-lifecycle-model-save-rejected.fixture.json",
      "docs/evidence/083/swift/autosave-lifecycle-schema-rejected.fixture.json",
      "docs/evidence/083/swift/production-mutation-origin-required.fixture.json",
      "docs/evidence/083/swift/lifecycle-root-app-scoped-modelcontext-leakage-rejected.fixture.json",
      "docs/evidence/083/swift/windowgroup-run-model-unavailable.fixture.json",
      "docs/evidence/083/swift/windowgroup-autosave-lifecycle-truth-rejected.fixture.json",
      "docs/evidence/083/swift/windowgroup-projection-value-type-only.fixture.json",
      "docs/evidence/083/swift/retained-windowgroup-direct-lifecycle-save-rejected.fixture.json"
    ],
    "coordinator_root_model_context_rule": "Coordinator-owned lifecycle roots do not receive the standard SwiftUI environment modelContext or the app-scoped lifecycle ModelContainer. They receive LifecycleReadOnlyProjectionEnvironment plus, where app-local UI persistence is required, a dedicated AppLocalModelContext that cannot register schemas for Run, StageExecution, AgentExecution, Approval, ProviderSession, command idempotency, shutdown, or lifecycle fields. Autosave is disabled for lifecycle roots unless the context is app-local-only.",
    "production_mutation_origin_rule": "Every approved SwiftData save from a lifecycle root must carry mutation_origin=app_local_non_authoritative or backend_readback_projection. mutation_origin=lifecycle_truth_write is forbidden in Swift. Production builds include the origin guard, not only debug assertions.",
    "direct_model_mutation_detection": "Guardrails scan for direct assignment plus ModelContext.save on lifecycle @Model types, environment(\\.modelContext) usage in coordinator-owned roots, and autosave-enabled contexts containing lifecycle schemas. Violations fail proposal-083 gate.",
    "implementation_migration_table": [
      {
        "module_or_service": "Chainworks Forge/App/ChainworksForgeApp.swift scene setup",
        "current_write": "App-scoped modelContainer/modelContext is available to WindowGroup roots and can leak into lifecycle-bearing views.",
        "target_disposition": "Keep for non-lifecycle shell/settings only; lifecycle roots are opened through LifecycleWindowCoordinator with read-only projection dependencies or AppLocalModelContext."
      },
      {
        "module_or_service": "Chainworks Forge/App/AppDelegateLifecycleCoordinator.swift applicationWillFinishLaunching",
        "current_write": "May compose lifecycle services while SwiftData environment is globally available.",
        "target_disposition": "Install LifecycleWindowCoordinator before ordering lifecycle windows and pass no lifecycle ModelContext into coordinator roots."
      },
      {
        "module_or_service": "Run detail and stage detail view models",
        "current_write": "Direct ModelContext.save can update Run, StageExecution, AgentExecution, or approval lifecycle fields.",
        "target_disposition": "Replace with backend mutation plus readback projection; local saves limited to app_local_non_authoritative UI state."
      },
      {
        "module_or_service": "Approval resolution service methods",
        "current_write": "Approval status may be locally mutated before backend readback.",
        "target_disposition": "Use backend approval mutation with CallerRequestId; SwiftData updates only from backend_readback_projection adapter."
      },
      {
        "module_or_service": "Artifact/report registration service methods",
        "current_write": "Artifact metadata saves may imply active report lineage authority.",
        "target_disposition": "Allowed only through evidence metadata adapter after canonical filesystem artifact and backend lineage readback exist."
      },
      {
        "module_or_service": "Recovery/session lineage display services",
        "current_write": "Provider session, recovery, or lifecycle rows may be cached in SwiftData.",
        "target_disposition": "Projection cache marked backend_readback_projection only; no lifecycle truth writes."
      },
      {
        "module_or_service": "LifecycleWindowCoordinator root construction",
        "current_write": "Environment may include modelContainer/modelContext by copying the standard scene environment.",
        "target_disposition": "Explicitly construct read-only projection environment; negative fixture rejects app-scoped lifecycle modelContext leakage."
      },
      {
        "module_or_service": "Tests and preview factories",
        "current_write": "May construct lifecycle @Model values for fixtures.",
        "target_disposition": "Allowed only under legacy_test_fixture_only paths; production scan rejects direct lifecycle ModelContext insert/save/delete."
      }
    ],
    "negative_lifecycle_context_fixture": "docs/evidence/083/swift/lifecycle-root-app-scoped-modelcontext-leakage-rejected.fixture.json",
    "coexistence_containment_migration_v1": {
      "final_app_scoped_model_container_schema": [
        "AppPreference",
        "WindowRestorationToken",
        "OperatorUIState",
        "ProjectionSnapshotCache",
        "EvidenceMetadataCache"
      ],
      "lifecycle_model_availability_rule": "Run, StageExecution, AgentExecution, Approval, ProviderSession, command idempotency, shutdown, cancellation intent, process_fate, and enforcement/rollback state models are not registered in the app-scoped SwiftData ModelContainer used by retained WindowGroup surfaces.",
      "retained_windowgroup_surfaces": [
        {
          "surface": "ContentView/RunsHome shell",
          "allowed_models": [
            "AppPreference",
            "OperatorUIState",
            "ProjectionSnapshotCache"
          ],
          "lifecycle_access": "projection-only value types RunSummaryProjection and RunStatusProjection"
        },
        {
          "surface": "Settings WindowGroup",
          "allowed_models": [
            "AppPreference"
          ],
          "lifecycle_access": "none"
        },
        {
          "surface": "Historical evidence browser shell",
          "allowed_models": [
            "EvidenceMetadataCache",
            "OperatorUIState"
          ],
          "lifecycle_access": "read-only ArtifactEvidenceProjection values"
        }
      ],
      "projection_only_value_types": [
        "RunSummaryProjection",
        "RunStatusProjection",
        "StageSummaryProjection",
        "ApprovalBadgeProjection",
        "ProviderSessionStatusProjection",
        "ArtifactEvidenceProjection"
      ],
      "autosave_prevention": "Retained WindowGroup ModelContext has autosave disabled for ProjectionSnapshotCache writes and cannot register lifecycle schemas. Projection cache writes go through ProjectionCacheWriter with mutation_origin=backend_readback_projection and monotonic cursor checks.",
      "object_mutation_prevention": "Lifecycle projection value types are structs detached from SwiftData @Model classes. No retained WindowGroup surface receives mutable lifecycle @Model references.",
      "negative_fixtures": [
        "docs/evidence/083/swift/windowgroup-run-model-unavailable.fixture.json",
        "docs/evidence/083/swift/windowgroup-autosave-lifecycle-truth-rejected.fixture.json",
        "docs/evidence/083/swift/windowgroup-projection-value-type-only.fixture.json",
        "docs/evidence/083/swift/retained-windowgroup-direct-lifecycle-save-rejected.fixture.json"
      ]
    }
  },
  "app_delegate_lifecycle_composition_contract_v1": {
    "addresses": [],
    "schema_version": "app_delegate_lifecycle_composition_contract_v1",
    "composition_strategy": "Single NSApplicationDelegate adapter: ChainworksForgeAppDelegate owns AutomationFallbackAppDelegate behavior as a child service and installs LifecycleWindowCoordinator during applicationWillFinishLaunching before any lifecycle window token can be ordered. There is no second competing app delegate.",
    "startup_sequence": [
      "applicationWillFinishLaunching creates AutomationFallbackService and LifecycleWindowCoordinator",
      "install coordinator pre-order hooks and restoration registry",
      "register non-lifecycle WindowGroup exclusions such as Settings and shell navigation",
      "applicationDidFinishLaunching runs automation fallback setup without ordering lifecycle windows",
      "first lifecycle run window request goes through coordinator unordered-token path"
    ],
    "automation_rule": "UI automation host keeps existing fallback behavior through AutomationFallbackService; fixtures assert automation launch can still open non-lifecycle shell and then request coordinator-owned lifecycle windows.",
    "termination_rule": "applicationShouldTerminate first asks LifecycleWindowCoordinator to flush projection subscriptions and window teardown receipts, then delegates automation fallback termination policy.",
    "fixtures": [
      "docs/evidence/083/swift/appdelegate-single-composition-cold-launch.fixture.json",
      "docs/evidence/083/swift/automation-host-launch-no-regression.fixture.json",
      "docs/evidence/083/swift/restoration-before-ordering.fixture.json",
      "docs/evidence/083/swift/termination-composition.fixture.json",
      "docs/evidence/083/swift/nsapplicationdelegateadaptor-single-owner.fixture.json",
      "docs/evidence/083/swift/automation-fallback-service-composed.fixture.json",
      "docs/evidence/083/swift/restoration-after-coordinator-install.fixture.json"
    ],
    "nsapplicationdelegateadaptor_integration": "ChainworksForgeApp declares one @NSApplicationDelegateAdaptor(ChainworksForgeAppDelegate.self). ChainworksForgeAppDelegate owns AutomationFallbackService as a child object and installs LifecycleWindowCoordinator in applicationWillFinishLaunching before restoration, open-run URL handling, Dock reopen, or first lifecycle-window token ordering.",
    "automation_fallback_delegate_rule": "The previous automation fallback delegate behavior is composed as AutomationFallbackService, not installed as a second NSApplicationDelegate. It receives launch/termination callbacks from ChainworksForgeAppDelegate after lifecycle coordinator pre-order hooks are installed."
  },
  "reliability_deadline_overflow_contract_v1": {
    "addresses": [
      "SCORE-LIFT-API-P083-R64-001",
      "SCORE-LIFT-MACOS-P083-R64-001",
      "SCORE-LIFT-REL-P083-R64-001",
      "SCORE-LIFT-REL-P083-R64-003"
    ],
    "schema_version": "reliability_deadline_overflow_contract_v1",
    "expired_recovery_required": "expired_recovery_required is a derived operator next_step_code, not a stored command lease_state. Stored lease_state remains pending, committed, failed, or abandoned.",
    "shutdown_deadline_defaults": {
      "graceful_ms": 15000,
      "kill_observation_ms": 15000,
      "host_total_ms": 30000,
      "max_host_total_ms": 120000
    },
    "deadline_config_validation": "Config above hard maximum is rejected at startup and surfaced as rollout hold condition shutdown_deadline_config_invalid.",
    "aggregate_overflow_latch_owner": "session, run, and global late-output overflow decisions are owned by cancel_late_output_overflow rows keyed by scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, and overflow_kind. Multi-session recovery updates counters in place and never inserts per-message or duplicate aggregate rows.",
    "fixtures": [
      "docs/evidence/083/reliability/expired-recovery-required-derived-next-step.fixture.json",
      "docs/evidence/083/reliability/shutdown-deadline-config-hard-limit.fixture.json",
      "docs/evidence/083/reliability/multi-session-overflow-aggregate-latch.fixture.json",
      "docs/evidence/083/reliability/lease-ttl-config-over-limit-denied.fixture.json",
      "docs/evidence/083/reliability/pending-replay-retry-after-seconds.fixture.json",
      "docs/evidence/083/reliability/run-global-overflow-aggregate-row.fixture.json",
      "docs/evidence/083/reliability/duplicate-session-overflow-row-rejected.fixture.json",
      "docs/evidence/083/reliability/duplicate-run-overflow-row-rejected.fixture.json",
      "docs/evidence/083/reliability/duplicate-global-overflow-row-rejected.fixture.json",
      "docs/evidence/083/reliability/shutdown-wave-max-concurrent-signals.fixture.json",
      "docs/evidence/083/reliability/shutdown-wave-deterministic-order.fixture.json",
      "docs/evidence/083/reliability/many-session-queued-no-signal-receipts.fixture.json",
      "docs/evidence/083/reliability/host-total-ms-applicationshouldterminate-budget.fixture.json",
      "docs/evidence/083/reliability/queued-no-signal-flushed-before-terminate.fixture.json",
      "docs/evidence/083/reliability/force-quit-honors-host-total-ms.fixture.json",
      "docs/evidence/083/reliability/many-session-queue-rank-stored.fixture.json",
      "docs/evidence/083/reliability/queue-rank-deterministic-order.fixture.json",
      "docs/evidence/083/reliability/queue-rank-restart-preserved.fixture.json"
    ],
    "aggregate_overflow_schema": "cancel_late_output_overflow uses generated normalized_run_id and normalized_provider_session_id columns plus UNIQUE(scope, normalized_run_id, normalized_provider_session_id, cancellation_epoch, overflow_kind). This is executable SQLite syntax and replaces all PRIMARY KEY(COALESCE(...)) wording.",
    "lease_ttl_defaults": {
      "lifecycle_mutation_seconds": 120,
      "approval_resolution_seconds": 300,
      "rollback_execution_seconds": 120,
      "copyable_command_regenerate_seconds": 30,
      "max_configurable_seconds": 900
    },
    "lease_ttl_validation": "Configured TTLs above max_configurable_seconds or below 5 seconds fail startup with rollout hold condition command_lease_ttl_config_invalid. pending replay retry_after_seconds is min(expires_at-now, configured command TTL cap).",
    "duplicate_row_negative_fixtures": [
      "docs/evidence/083/reliability/duplicate-session-overflow-row-rejected.fixture.json",
      "docs/evidence/083/reliability/duplicate-run-overflow-row-rejected.fixture.json",
      "docs/evidence/083/reliability/duplicate-global-overflow-row-rejected.fixture.json"
    ],
    "machine_evaluable_hold_conditions": [
      "shutdown_deadline_config_invalid",
      "command_lease_ttl_config_invalid"
    ],
    "bounded_shutdown_wave_policy": {
      "max_concurrent_graceful_signals": 8,
      "max_concurrent_kill_signals": 4,
      "ordering": "oldest shutdown_epoch first, then provider_session_id lexical tie-break",
      "fairness": "no run may consume more than half of signal slots while other runs have queued shutdown work",
      "queued_receipt_rule": "sessions not signaled before host_total_ms expires receive queued_no_signal receipts with deterministic queue_rank",
      "host_total_ms_to_appkit_budget_rule": "host_total_ms is the daemon-side bounded shutdown wave budget. On macOS, ChainworksForgeAppDelegate.applicationShouldTerminate returns NSApplication.TerminateReply.terminateLater and then calls NSApplication.shared.reply(toApplicationShouldTerminate:) after waiting up to host_total_ms milliseconds for the bounded shutdown wave to complete. The AppKit-visible terminate budget always covers host_total_ms, plus a fixed 1000ms tail for queued_no_signal receipt flush and SwiftUI WindowGroup teardown acknowledgement. Force Quit, logout, and system shutdown surfaces honor the same deadline through LifecycleWindowCoordinator teardown; queued_no_signal receipts are flushed durably to SQLite under WAL before applicationShouldTerminate returns.",
      "queue_rank_storage_rule": "queue_rank is stored as shutdown_interrupted_receipts.queue_rank INTEGER NOT NULL for queued_no_signal receipts. It is also exposed as final_readback_rank (GraphQL p083ShutdownQueueRank, MCP p083_shutdown_queue_rank). queue_rank is derived at the moment a receipt is queued and never recomputed; the derivation is a deterministic total order over (shutdown_epoch ASC, provider_session_id ASC). Restart recovery reloads queue_rank from the stored column and never reorders existing queued receipts."
    }
  },
  "provider_cancellation_intent_contract_v1": {
    "addresses": [
      "SCORE-LIFT-REL-P083-R64-002"
    ],
    "schema_version": "provider_cancellation_intent_contract_v1",
    "model": "cancellation_requested is a durable intent flag, not a provider lifecycle_state. Provider lifecycle_state remains one of provider_lifecycle_vocabulary_authority_v1.canonical_values.",
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
    "metric_rule": "Cancellation intent metrics use provider_cancellation_intent_total{provider,intent_state,reason}; provider_session_lifecycle_total does not include cancellation_requested.",
    "recovery_rule": "On restart, sessions with provider_cancellation_intents.intent_state in requested/shutdown_started or provider_sessions.process_fate=backpressure_cutoff_shutdown_pending resume shutdown through provider_cancellation_intents.shutdown_epoch when non-null. If requested has null shutdown_epoch and process identity still matches, recovery creates the next shutdown_epoch transactionally and updates the intent to shutdown_started; if identity is absent, recovery sets process_fate=absent_verified without inventing a shutdown_epoch.",
    "fixtures": [
      "docs/evidence/083/cancellation/cancellation-requested-not-lifecycle-state.fixture.json",
      "docs/evidence/083/cancellation/backpressure-pending-restart-resumes-shutdown.fixture.json",
      "docs/evidence/083/cancellation/backpressure-pending-not-terminal.fixture.json",
      "docs/evidence/083/cancellation/migration-creates-provider-cancellation-intents.fixture.json",
      "docs/evidence/083/cancellation/no-lifecycle-state-derivation.fixture.json",
      "docs/evidence/083/cancellation/pending-backpressure-shutdown-restart.fixture.json",
      "docs/evidence/083/cancellation/shutdown-epoch-null-requested.fixture.json",
      "docs/evidence/083/cancellation/shutdown-started-requires-shutdown-epoch.fixture.json",
      "docs/evidence/083/cancellation/restart-resumes-intent-shutdown-epoch.fixture.json",
      "docs/evidence/083/cancellation/requested-null-epoch-starts-new-shutdown.fixture.json",
      "docs/evidence/083/cancellation/requested-null-epoch-identity-ambiguous-held.fixture.json",
      "docs/evidence/083/cancellation/requested-null-epoch-manual-process-identity-check.fixture.json",
      "docs/evidence/083/cancellation/requested-null-epoch-no-shutdown-epoch-assigned.fixture.json"
    ],
    "migration_rule": "Migration p083_007_provider_cancellation_intent_and_process_fate creates provider_cancellation_intents and adds provider_sessions.process_fate/process_fate_updated_at. No historical cancellation intent is synthesized unless a pre-existing durable cancellation_epoch plus shutdown side-effect row proves a requested cancellation; ambiguous legacy rows are marked held with reason=shutdown_recovery and require manual_process_identity_check.",
    "derivation_rule": "On restart, cancellation intent is derived only from provider_cancellation_intents joined to provider_sessions.cancellation_epoch and shutdown_signal_side_effects. Lifecycle state text is never parsed to infer cancellation intent.",
    "indexes": [
      "CREATE INDEX provider_cancellation_intents_shutdown_epoch_idx ON provider_cancellation_intents(provider_session_id, shutdown_epoch) WHERE shutdown_epoch IS NOT NULL;",
      "CREATE INDEX provider_cancellation_intents_state_idx ON provider_cancellation_intents(intent_state, reason);"
    ],
    "shutdown_epoch_transition_rules": [
      "requested: shutdown_epoch may be null before shutdown planning begins",
      "shutdown_started: shutdown_epoch is non-null and points to the canonical shutdown epoch being resumed or observed",
      "settled: shutdown_epoch remains the final epoch used for settlement evidence when a shutdown was required",
      "held: shutdown_epoch is nullable; if present it identifies the epoch requiring manual_process_identity_check"
    ],
    "readback_fields": [
      "provider_session_id",
      "cancellation_epoch",
      "intent_state",
      "reason",
      "shutdown_epoch",
      "process_fate",
      "latest_shutdown_receipt_id",
      "operator_next_step_code"
    ],
    "identity_ambiguous_recovery_rule": "On restart, an intent with intent_state=requested and shutdown_epoch IS NULL is evaluated against current provider process identity. If process identity is ambiguous (cannot be confirmed live, absent, or interrupted), recovery does NOT advance the intent to shutdown_started, does NOT assign a shutdown_epoch, and does NOT settle terminal. The intent is held: intent_state remains requested, process_fate=identity_ambiguous, and operator_next_step_code returns manual_process_identity_check. The held state persists until operator action or further process evidence resolves identity. This rule applies symmetrically to requested intents from operator_cancel, backpressure_cutoff, and shutdown_recovery reason codes."
  },
  "macos_ui_implementation_fixtures_v1": {
    "addresses": [
      "SCORE-LIFT-MACOS-P083-R64-002",
      "SCORE-LIFT-MACOS-P083-R64-003",
      "SCORE-LIFT-UI-P083-R64-001"
    ],
    "schema_version": "macos_ui_implementation_fixtures_v1",
    "required_fixtures": [
      "docs/evidence/083/ui/focused-window-command-routing.fixture.json",
      "docs/evidence/083/ui/sheet-modal-export.fixture.json",
      "docs/evidence/083/ui/pending-terminal-accessibility-announcement.fixture.json",
      "docs/evidence/083/ui/current-host-only-clipboard-wording.fixture.json",
      "docs/evidence/083/ui/semantic-color-token-usage.fixture.json",
      "docs/evidence/083/ui/no-evidence-disabled-styling.fixture.json",
      "docs/evidence/083/macos/focused-window-command-routing.fixture.json",
      "docs/evidence/083/macos/asynchronous-termination-handling.fixture.json",
      "docs/evidence/083/macos/responder-chain-copy-precedence.fixture.json",
      "docs/evidence/083/macos/export-revalidates-after-focus-run-projection-change.fixture.json",
      "docs/evidence/083/macos/automatic-tabbing-cross-run-denied.fixture.json",
      "docs/evidence/083/macos/automatic-tabbing-same-run-distinct-role-allowed.fixture.json",
      "docs/evidence/083/macos/merge-all-windows-cross-run-denied.fixture.json",
      "docs/evidence/083/macos/merge-all-windows-same-run-allowed.fixture.json",
      "docs/evidence/083/macos/nsmenu-validation-no-lifecycle-window-disabled.fixture.json",
      "docs/evidence/083/macos/nsmenu-validation-settings-key-window-disabled.fixture.json",
      "docs/evidence/083/macos/nsmenu-validation-disabled-not-hidden.fixture.json"
    ],
    "routing_rule": "Copy, Export Text..., Cmd-W, approval commands, rollback commands, and lifecycle commands dispatch through the focused lifecycle_window_id. Background windows cannot receive command side effects.",
    "sheet_rule": "Export Text presents NSSavePanel as a sheet on the focused lifecycle NSWindow and revalidates lifecycle_window_id plus projection freshness before writing.",
    "accessibility_rule": "Pending-to-terminal transitions announce committed, replayed, denied, and failed outcomes without moving focus unless the focused control is removed.",
    "termination_rule": "Asynchronous application termination waits for LifecycleWindowCoordinator teardown acknowledgements up to the configured deadline, then follows bounded shutdown wave policy and records queued_no_signal where appropriate.",
    "copy_precedence_rule": "Responder-chain copy wins over focused copy controls when an editable text responder handles copy; lifecycle copy controls are fallback only.",
    "export_revalidation_rule": "Export revalidates focused window, run_id, projection freshness, and backend actionability after focus, run, or projection changes before writing.",
    "automatic_tabbing_rule": "Lifecycle-bearing windows declare NSWindow.tabbingMode = .disallowed when lifecycle_window_id and run_id differ between candidate tab partners. Two windows with the same run_id but distinct restoration_role values may merge as tabs only when the operator invokes Merge All Windows explicitly and the coordinator confirms identical run_id; cross-run tabbing and Merge All Windows across distinct run_ids are denied with a typed denial reason cross_run_tabbing_denied. Background lifecycle commands route only to the key window's lifecycle_window_id regardless of tab grouping.",
    "nsmenu_validation_rule": "NSMenuValidation for lifecycle menu items asks the key window's lifecycle_window_id whether the action is available. When no lifecycle window is key (Settings-only state, no-window state), every lifecycle menu item returns false from validateMenuItem and renders as disabled; menu items are never hidden, and selecting a disabled item is a silent no-op without side effects. The Settings window is excluded from lifecycle command routing and lifecycle menus are disabled while it is key."
  }
}
