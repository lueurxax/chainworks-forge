{
  "schema_version": "proposal_document_v1",
  "proposal_id": "P083",
  "proposal_revision_id": "P083-r70-refined-r69-score-lift",
  "title": "Execution-Truth Ownership and Invariant Model",
  "date": "2026-06-04",
  "status": "Implementation in progress. The R69 proposal review summary for proposal_revision_id P083-r69-refined-r68-score-lift returned decision=revise_required with aggregate_score=8.8 and min_individual_score=8.1. Four reviewers (api_contract, apple_architect, macos, ui) approve; the reliability reviewer flagged one blocking cross-surface contradiction (REL-P083-R69-BLOCK-001): p083.rollback_execution intent_hash was computed from target_enforcement_mode while neither GraphQL nor MCP accepted a rollback target. This R70 refine pass reconciles that contradiction (Option A from the backlog: a non-null targetEnforcementMode is now part of GraphQL, MCP, rollback_disposition steps, and intent_hash composition; rollback fixtures cover same-request replay, same-intent aliasing, and mismatch denial), promotes the five R69 advisory items into active executable contracts, and removes every R68 reviewer_feedback_resolution mapping that is no longer authoritative. Closeout still requires a fresh aggregate implementation review against this revision with decision=approve, blocker_count=0, and current-revision evidence.",
  "author": "Codex",
  "source_idea": "Implement Proposal 083: Execution-Truth Ownership and Invariant Model.",
  "canonical_proposal_path": "docs/proposals/083-execution-truth-ownership-invariant-model.md",
  "source_review_pass_id": "proposal-review-P083-r69-f091f00c-5964-4f4f-9031-9dea260a75a7",
  "review_basis": {
    "authoritative_backlog_review_pass_id": "proposal-review-P083-r69-f091f00c-5964-4f4f-9031-9dea260a75a7",
    "authoritative_backlog_item_ids": [
      "REL-P083-R69-BLOCK-001",
      "API-P083-R69-NB-001",
      "API-P083-R69-NB-002",
      "REL-P083-R69-NB-001",
      "MACOS-P083-R69-NB-002",
      "UI-P083-R69-NB-001"
    ],
    "stale_material_policy": "Active feedback mappings, revision summaries, and readiness claims cite only the current R69 score_lift_backlog item ids. Prior-pass R53/R55/R65/R66/R67/R68 backlog ids, dispositions, and closure narratives are no longer used as active authority and have been removed from reviewer_feedback_resolution, proposal_revision_summary, and any addresses[] arrays in this revision.",
    "current_review_basis_summary": "The R69 backlog contains exactly six items. One item (REL-P083-R69-BLOCK-001, blocking) is a cross-surface contradiction: p083.rollback_execution GraphQL and MCP signatures accepted only callerRequestId/caller_request_id while command_idempotency_contract_v1 included target_enforcement_mode in the per-command intent_hash. Five items (API-P083-R69-NB-001, API-P083-R69-NB-002, REL-P083-R69-NB-001, MACOS-P083-R69-NB-002, UI-P083-R69-NB-001) are advisory implementation hardening: reconciling rollback SDL/MCP/idempotency, replacing free-form String fields with closed enum domains plus canonical normalization, correlating durable monotonic clock conversions with a baseline generation, pinning macOS Commands menu placement with toolbar parity, and defining a ManualProcessIdentityCheckBanner action hierarchy with loading/copy feedback. This R70 refine pass picks Option A from REL-P083-R69-BLOCK-001 (add a non-null rollback target to GraphQL and MCP and include it in rollback_disposition) and promotes every R69 advisory item into active executable contracts.",
    "blocker_resolution_choice": "Option A: targetEnforcementMode is a non-null required argument on GraphQL p083RollbackExecution and a required_input on MCP p083.rollback_execution, included in rollout_contract_v1.rollback_disposition.steps, included in command_idempotency_contract_v1.per_command_logical_fields[p083.rollback_execution], and covered by three R70 rollback fixtures (same-request replay, same-intent aliasing across new request_id, mismatch denial when a second request changes target_enforcement_mode)."
  },
  "active_readiness_narrative": {
    "active_backlog_item_count": 6,
    "blocking_backlog_item_count": 1,
    "advisory_backlog_item_count": 5,
    "proposal_text_items_addressed": 6,
    "proposal_text_items_addressed_ids": [
      "REL-P083-R69-BLOCK-001",
      "API-P083-R69-NB-001",
      "API-P083-R69-NB-002",
      "REL-P083-R69-NB-001",
      "MACOS-P083-R69-NB-002",
      "UI-P083-R69-NB-001"
    ],
    "out_of_band_routing_items_ids": [],
    "unresolved_proposal_text_blocker_count": 0,
    "deferred_blocker_count": 0,
    "disputed_blocker_count": 0,
    "implementation_may_start": true,
    "implementation_may_start_after": "Implementation is already in progress. Ready/closeout may be claimed only after a fresh aggregate implementation review against this revision returns decision=approve, blocker_count=0, and a corpus-only-current-revision attestation.",
    "single_authority_pointer": "reviewer_feedback_resolution maps every current R69 score_lift_backlog item to active proposal sections. REL-P083-R69-BLOCK-001 is mapped to the reconciled rollback contracts (graphql_sdl_contract_v1, mcp_tool_inventory_contract_v1, command_idempotency_contract_v1, rollout_contract_v1.rollback_disposition).",
    "latest_review_authority": "reviews/proposal/summary.json decision=revise_required, blocker_count=1, aggregate_score=8.8, min_individual_score=8.1; the blocking rollback cross-surface contradiction is resolved in this R70 contract and implementation is in progress pending fresh implementation-review closeout."
  },
  "executive_summary": "P083 names durable storage as the execution-truth authority for runs, stages, agents, approvals, artifacts, side effects, provider sessions, command idempotency, shutdown receipts, rollout state, and operator readback. This R70 revision resolves the R69 blocking contradiction by treating targetEnforcementMode as a first-class non-null caller input on p083.rollback_execution across GraphQL SDL, MCP, intent_hash composition, and rollback_disposition steps, with three new rollback fixtures (same-request replay, same-intent aliasing, mismatch denial). It promotes the five R69 advisory items into active executable contracts: closed enum types (ApprovalResolution, P083EnforcementMode, P083RollbackTargetMode) with canonical lowercase normalization before hashing; durable monotonic clock baseline correlation via baseline_sample_id on deadline-bearing rows and nearest-baseline-at-or-before lookup; pinned macOS Commands menu structure (Run menu) with toolbar enabled-state and accessibility parity; and a ManualProcessIdentityCheckBanner action hierarchy with explicit primary/secondary/tertiary/overflow placement and loading/success/error feedback states.",
  "problem": [
    "Execution truth currently crosses GraphQL, MCP, SQLite rows, frozen workflow snapshots, stage and agent attempts, provider sessions, approvals, artifacts, side-effect receipts, reports, and SwiftUI projections.",
    "Without an ownership model, caller payloads, projections, provider transcripts, filesystem scans, or UI caches can be mistaken for durable truth.",
    "Retry, cancel, shutdown, rollback, and enforcement cutover need idempotency and receipt constraints that survive crashes and SQLite uniqueness rules.",
    "Rollback is safety-critical and cannot tolerate cross-surface contradictions about whether a target is caller-supplied or service-derived.",
    "macOS lifecycle handling must distinguish graceful AppKit callbacks from abrupt process termination where no delegate callback is guaranteed, and must place lifecycle commands in deterministic menu and toolbar locations.",
    "Free-form String fields for lifecycle resolutions and enforcement targets cause normalization drift that corrupts intent_hash composition."
  ],
  "goals": [
    "Define one authoritative durable record for every execution-truth identifier.",
    "Classify caller-supplied identifiers as authority, selector, diagnostic, service_owned, or forbidden.",
    "Require lifecycle mutations to carry CallerRequestId and execute through durable idempotency rows with canonical, per-command intent_hash composition that includes every caller-input enum value the surfaces accept.",
    "Reconcile rollback execution end-to-end: GraphQL, MCP, command idempotency, and rollback_disposition all carry the same non-null targetEnforcementMode value, normalized before hashing.",
    "Publish executable contracts for GraphQL SDL, MCP tool inventory and shared denial vocabulary, SQLite migrations, artifact lineage, metrics, recovery readback, shutdown, late output, durable monotonic clock with baseline correlation, and Swift projection mapping with explicit Swift Concurrency isolation.",
    "Keep the macOS app read-only for P083 lifecycle enforcement while providing accurate readback, deterministic menu/toolbar placement, and safe copy/export affordances.",
    "Provide a strict inline rollout_contract_v1 with gate aliases, migration posture, metrics, readback lanes, hold conditions, rollback disposition, and negative fixtures.",
    "Require corpus integrity (current-revision-only reviewer artifacts) before Ready can be claimed.",
    "Define operator-visible manual recovery UX for identity-ambiguous provider cancellation holds, with an explicit action hierarchy and loading/success/error feedback."
  ],
  "non_goals": [
    "Do not add authentication, RBAC, token rotation, credential prompts, or Keychain behavior beyond checking existing principal-class helpers.",
    "Do not change workflow YAML or agent catalog YAML semantics or require new YAML keys.",
    "Do not remove historical artifacts, transcripts, or failed-attempt evidence.",
    "Do not make SwiftUI, GraphQL payloads, MCP payloads, provider transcripts, reports, or filesystem scans authoritative for execution truth.",
    "Do not add a native macOS write path for side_effects.force_reconcile in P083.",
    "Do not introduce destructive migrations or backfill that rewrites historical run evidence.",
    "Do not derive p083.rollback_execution target from service state implicitly; the caller MUST supply a non-null target_enforcement_mode value."
  ],
  "target_users_and_trigger": {
    "primary_user": "Chainworks Forge operator running long-lived agent workflows from the macOS app.",
    "implementation_user": "Engine, API, persistence, projection, and UI engineers changing lifecycle state or readback.",
    "trigger": "Repeated review churn around provenance drift, stale identifiers, duplicate commands, inactive approvals, external side effects, provider shutdown, rollout enforcement, mixed-revision reviewer corpora, and cross-surface rollback contradictions."
  },
  "ux_ui_notes": {
    "truth_readback": "SwiftUI renders backend readback as read-only truth. Mutation affordances are disabled unless backend actionability is true and projection_integrity is fresh.",
    "typed_denials": "Typed denials render inline beside the affected run, stage, approval, artifact, side-effect, or provider-session row. Unknown denial codes render a generic validation message and no optimistic action. Denial codes come from the shared denial vocabulary in mcp_tool_inventory_contract_v1.shared_denial_vocabulary, which is byte-equal to the GraphQL DenialReason union in graphql_sdl_contract_v1.",
    "closed_enums": "ApprovalResolution, P083EnforcementMode, and P083RollbackTargetMode are GraphQL enums with matching JSON Schema enum constraints in MCP. SwiftUI controls bind to the same enum cases and never send free-form String values for these fields; lowercase canonical form is enforced before transport.",
    "historical_evidence": "Active artifacts appear first. Historical Evidence is collapsed by default and labels rows Superseded, Failed, Cancelled, or Quarantined without active-transition controls.",
    "shutdown_readback": "Graceful AppKit shutdown progress and abrupt restart recovery are shown as different states. Abrupt termination never claims that applicationShouldTerminate ran; it shows the durable intent or side-effect row that recovery used.",
    "identity_ambiguous_hold": "Provider cancellation rows with intent_state=held and process_fate=identity_ambiguous show operator_next_step_code=manual_process_identity_check and no automatic retry spinner.",
    "copy_export_controls": "Copy controls use NSPasteboard.ContentsOptions.currentHostOnly and never include secrets. Export Text writes only through NSSavePanel unless the operator separately chooses a Copy Export Text action.",
    "manual_process_identity_check": "A ManualProcessIdentityCheckBanner appears in the provider-session detail and any run/stage surface containing the held provider, with an explicit action hierarchy (see manual_process_identity_check_ui_v1.action_hierarchy) and loading/success/error feedback states for each action. VoiceOver reads the title, provider name, reason, and focused action; disabled lifecycle commands remain visible with adjacent reason text."
  },
  "ownership_model": {
    "rule": "Every lifecycle identifier has exactly one authoritative durable record. Callers may provide authority or selector ids only where the ownership matrix permits them; service-owned identifiers are never accepted from caller payload as truth.",
    "data_authority_rule": "SQLite rows are authoritative. GraphQL, MCP, filesystem artifacts, report JSON, and SwiftData projections are readback or evidence surfaces only.",
    "transaction_rule": "For mutating lifecycle commands, request acquisition, authoritative row reload, lifecycle compare-and-set, side-effect receipt write, and terminal command outcome commit happen in one SQLite transaction unless the contract explicitly defines an earlier denial path.",
    "ownership_matrix": [
      {"identifier": "run_id", "authoritative_record": "runs.id", "caller_classification": "authority"},
      {"identifier": "stage_execution_id", "authoritative_record": "stage_executions.id", "caller_classification": "service_owned"},
      {"identifier": "agent_execution_id", "authoritative_record": "agent_executions.id", "caller_classification": "service_owned"},
      {"identifier": "provider_session_id", "authoritative_record": "provider_sessions.provider_session_id", "caller_classification": "service_owned"},
      {"identifier": "request_id", "authoritative_record": "command_idempotency and command_request_aliases", "caller_classification": "authority"},
      {"identifier": "approval_id", "authoritative_record": "approvals.id", "caller_classification": "selector"},
      {"identifier": "artifact_id", "authoritative_record": "artifact_lineage.artifact_id", "caller_classification": "selector"},
      {"identifier": "side_effect_id", "authoritative_record": "side_effects.id", "caller_classification": "selector"},
      {"identifier": "target_enforcement_mode", "authoritative_record": "p083_enforcement_mode_state (operator-supplied target, validated against P083RollbackTargetMode)", "caller_classification": "authority"}
    ]
  },
  "architecture": {
    "rust_control_plane_modules_touched": [
      "control-plane/crates/domain: nominal ids, denial codes, ProjectionIntegrity compatibility structs, lifecycle vocabulary enums, shared denial code enum mirrored between GraphQL DenialReason and MCP denial vocabulary, ApprovalResolution, P083EnforcementMode, P083RollbackTargetMode enum types",
      "control-plane/crates/db: additive migrations for artifact_lineage.report_kind, command idempotency generations, shutdown receipts, queue_rank, overflow latch rows, enforcement mode state, rollback audit rows, durable monotonic clock samples, provider cancellation intents",
      "control-plane/crates/engine: idempotent command execution with canonical intent_hash composition (including enum lowercase normalization), recovery readback, shutdown state machine, late-output caps, abrupt termination recovery, enforcement preflight, durable monotonic clock baseline correlation",
      "control-plane/crates/graphql-server: versioned projection-integrity fields, cutover and rollback mutations with non-null targetEnforcementMode, readback fields, RollbackDispositionJSON output validation, lifecycle mutation SDL with shared DenialReason union and closed enum types",
      "control-plane/crates/mcp-server: matching MCP tool inventory with enum-typed inputs, rollout readback, rollback tool, bounded metrics, shared denial vocabulary",
      "control-plane/crates/workflow: RunPlan compatibility validation including xhigh effort values",
      "control-plane/crates/engine command idempotency: command_idempotency rows, request aliases, TTL recovery, committed replay, conflict denial, pending lease reacquire, per-command intent_hash composition with enum canonical lowercase normalization"
    ],
    "swift_modules_touched": [
      "Chainworks Forge/AppLifecycle: app-owned lifecycle window coordinator and graceful applicationShouldTerminate handling",
      "Chainworks Forge/Projection: RunProjectionSnapshotStore and field mapping manifest validation; @MainActor projection ModelContext access; @ModelActor adapter for non-main projection writes; Sendable projection snapshots before crossing back to SwiftUI roots",
      "Chainworks Forge/CopyControls: CopyButtonRepresentable and current-host-only pasteboard writer",
      "Chainworks Forge/RequestIds: distinct LifecycleRequestId and CopyableCommandRequestId nominal types",
      "Chainworks Forge/SwiftDataBoundary: projection-only/app-local containers and leakage guardrails",
      "Chainworks Forge/ProviderRecoveryUI: ManualProcessIdentityCheckBanner with primary/secondary/tertiary/overflow action layout, loading/success/error feedback, focused command validation, and VoiceOver-readable denial states",
      "Chainworks Forge/Commands: Run menu placement of Cancel Run, Retry Run, Retry Stage, Resolve Approval, Shutdown Provider Session, and Retry Identity Check; @FocusedValue parity between menu and toolbar enabled-state and accessibility labels"
    ],
    "migration_rule": "All migrations are additive. Rollback disables enforcement or returns to permissive mode; it does not drop columns, delete evidence, or rewrite historical rows."
  },
  "api_contracts": {
    "caller_request_id_v1": {
      "json_type": "string",
      "pattern": "^[0-9a-f]{8}-[0-9a-f]{4}-4[0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$",
      "rejected_forms": ["uppercase_uuid", "surrounding_whitespace", "urn_prefix", "braced_uuid"],
      "additional_properties_policy": "Containers that hold request_id set additionalProperties:false."
    },
    "rollback_disposition_json_v1": {
      "addresses": [],
      "graphql_symbol": "RollbackDispositionJSON",
      "schema_version": "rollback_disposition_v1",
      "output_validation_rule": "Rollout readback resolvers validate every rollback_disposition output value against rollback_disposition_v1 before GraphQL serialization. The GraphQL scalar parser performs the same validation only when the scalar is used in a future input position; parser validation is not the enforcement point for current output-only fields.",
      "mcp_schema_rule": "MCP rollout_contract_rollback_disposition is a Draft 2020-12 object with additionalProperties=false and required schema_version, mode, data_loss_risk, and steps fields. GraphQL and MCP carry byte-equal JSON values.",
      "negative_fixtures": [
        "docs/evidence/083/api/rollback-disposition-missing-schema-version-rejected.fixture.json",
        "docs/evidence/083/api/rollback-disposition-output-invalid-rejected-before-graphql.fixture.json",
        "docs/evidence/083/api/rollback-disposition-mcp-extra-property-rejected.fixture.json"
      ],
      "required_fields": ["schema_version", "mode", "data_loss_risk", "steps"],
      "rollout_contract_rule": "rollout_contract_v1.rollback_disposition stays strict-template-compatible and does not include schema_version because docs/reference/executable-rollout-gate-template.md permits only mode, data_loss_risk, and optional steps inside the inline rollback_disposition object. The generated RollbackDispositionJSON readback value for run_report, MCP, release_receipt, and GraphQL wraps the inline disposition with schema_version:'rollback_disposition_v1'. Missing schema_version fails readback fixture validation, not inline rollout_contract_v1 lint."
    }
  },
  "graphql_sdl_contract_v1": {
    "schema_version": "graphql_sdl_contract_v1",
    "addresses": ["REL-P083-R69-BLOCK-001", "API-P083-R69-NB-001", "API-P083-R69-NB-002"],
    "authority": "control-plane/crates/graphql-server SDL is the single GraphQL surface authority for P083 lifecycle mutations and is mirrored by mcp_tool_inventory_contract_v1.",
    "non_null_caller_request_id_rule": "Every P083 lifecycle mutation declares callerRequestId: CallerRequestId! as a non-null argument. Optional CallerRequestId is not permitted on lifecycle mutations. Selector ids and authority ids declared by the ownership matrix remain non-null where the matrix marks them required.",
    "closed_enum_rule": "Caller-supplied lifecycle resolutions and enforcement targets are GraphQL enums (ApprovalResolution, P083EnforcementMode, P083RollbackTargetMode). Free-form String inputs are not permitted for these fields. Enum case names use lowercase snake-equivalent identifiers and are byte-equal between GraphQL SDL and MCP JSON Schema enum constraints.",
    "shared_denial_union_rule": "All P083 lifecycle mutations return a payload union where the failure branch is the shared DenialPayload type carrying a non-null DenialReason enum value. DenialReason is byte-equal in name set to mcp_tool_inventory_contract_v1.shared_denial_vocabulary.",
    "lifecycle_mutation_signatures": [
      "scalar CallerRequestId",
      "enum ApprovalResolution { approve reject }",
      "enum P083EnforcementMode { disabled permissive enforce }",
      "enum P083RollbackTargetMode { permissive disabled }",
      "enum DenialReason { missing_caller_request_id malformed_request_id principal_class_not_allowed lifecycle_state_invalid schema_invalid additional_properties_rejected rollback_target_required rollback_target_invalid request_intent_mismatch idempotency_in_flight idempotency_replayed idempotency_expired_reacquired operator_required p083_operator_required provider_session_not_found run_not_found stage_not_retryable approval_not_actionable side_effect_not_reconcilable enforcement_mode_transition_denied identity_ambiguous idempotency_replay_corrupt idempotency_terminal_failure internal }",
      "type DenialPayload { reason: DenialReason! message: String! retryAfterSeconds: Int }",
      "type RunsCancelSuccess { runId: ID! cancellationEpoch: Int! }",
      "union RunsCancelPayload = RunsCancelSuccess | DenialPayload",
      "type RunsRetrySuccess { runId: ID! attemptNumber: Int! }",
      "union RunsRetryPayload = RunsRetrySuccess | DenialPayload",
      "type StagesRetrySuccess { stageExecutionId: ID! attemptNumber: Int! }",
      "union StagesRetryPayload = StagesRetrySuccess | DenialPayload",
      "type ApprovalsResolveSuccess { approvalId: ID! resolution: ApprovalResolution! }",
      "union ApprovalsResolvePayload = ApprovalsResolveSuccess | DenialPayload",
      "type SideEffectsForceReconcileSuccess { sideEffectId: ID! reconciliationId: ID! }",
      "union SideEffectsForceReconcilePayload = SideEffectsForceReconcileSuccess | DenialPayload",
      "type ProviderSessionShutdownSuccess { providerSessionId: ID! shutdownEpoch: Int! }",
      "union ProviderSessionShutdownPayload = ProviderSessionShutdownSuccess | DenialPayload",
      "type P083RollbackExecutionSuccess { actionId: ID! enforcementMode: P083EnforcementMode! targetEnforcementMode: P083RollbackTargetMode! }",
      "union P083RollbackExecutionPayload = P083RollbackExecutionSuccess | DenialPayload",
      "type P083SetEnforcementModeSuccess { actionId: ID! enforcementMode: P083EnforcementMode! }",
      "union P083SetEnforcementModePayload = P083SetEnforcementModeSuccess | DenialPayload",
      "extend type Mutation { runsCancel(runId: ID!, callerRequestId: CallerRequestId!): RunsCancelPayload! runsRetry(runId: ID!, callerRequestId: CallerRequestId!): RunsRetryPayload! stagesRetry(stageExecutionId: ID!, callerRequestId: CallerRequestId!): StagesRetryPayload! approvalsResolve(approvalId: ID!, resolution: ApprovalResolution!, callerRequestId: CallerRequestId!): ApprovalsResolvePayload! sideEffectsForceReconcile(sideEffectId: ID!, callerRequestId: CallerRequestId!): SideEffectsForceReconcilePayload! providerSessionShutdown(providerSessionId: ID!, callerRequestId: CallerRequestId!): ProviderSessionShutdownPayload! p083RollbackExecution(targetEnforcementMode: P083RollbackTargetMode!, callerRequestId: CallerRequestId!): P083RollbackExecutionPayload! p083SetEnforcementMode(targetMode: P083EnforcementMode!, callerRequestId: CallerRequestId!): P083SetEnforcementModePayload! }"
    ],
    "denial_parity_rule": "Every DenialReason enum name appears in mcp_tool_inventory_contract_v1.shared_denial_vocabulary with the identical spelling. CI fixture graphql-mcp-denial-vocabulary-parity proves the byte-equal set.",
    "enum_parity_rule": "ApprovalResolution, P083EnforcementMode, and P083RollbackTargetMode case sets are byte-equal to mcp_tool_inventory_contract_v1.tools[*].enum_constraints[*] sets. CI fixture graphql-mcp-enum-vocabulary-parity proves the byte-equal set across all three enums.",
    "fixtures": [
      "docs/evidence/083/api/graphql-sdl-lifecycle-mutations-non-null-caller-request-id.fixture.json",
      "docs/evidence/083/api/graphql-sdl-denial-union-payload-shape.fixture.json",
      "docs/evidence/083/api/graphql-mcp-denial-vocabulary-parity.fixture.json",
      "docs/evidence/083/api/graphql-sdl-missing-caller-request-id-rejected.fixture.json",
      "docs/evidence/083/api/graphql-mcp-enum-vocabulary-parity.fixture.json",
      "docs/evidence/083/api/graphql-rollback-target-required.fixture.json",
      "docs/evidence/083/api/graphql-rollback-target-invalid-enum-rejected.fixture.json"
    ]
  },
  "mcp_tool_inventory_contract_v1": {
    "schema_version": "mcp_tool_inventory_contract_v1",
    "addresses": ["REL-P083-R69-BLOCK-001", "API-P083-R69-NB-001", "API-P083-R69-NB-002"],
    "authority": "control-plane/crates/mcp-server tool registry mirrors graphql_sdl_contract_v1 lifecycle mutations.",
    "schema_dialect": "JSON Schema Draft 2020-12 with additionalProperties:false on every input and output object schema and explicit required[] arrays.",
    "enum_normalization_rule": "Caller enum string values are normalized to lowercase canonical form by the MCP transport layer before being passed to command_idempotency_contract_v1.intent_hash_composition_rule. Values that fail JSON Schema enum constraint validation (case-sensitive lowercase) are denied with rollback_target_invalid, schema_invalid, or additional_properties_rejected before any side effect.",
    "tools": [
      {"tool": "runs.cancel", "input_schema_path": "docs/reference/mcp/p083/runs.cancel.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/runs.cancel.output.schema.json", "required_input": ["run_id", "caller_request_id"], "enum_constraints": {}, "denial_codes": ["request_intent_mismatch", "malformed_request_id", "lifecycle_state_invalid", "request_id_not_owned", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "runs.retry", "input_schema_path": "docs/reference/mcp/p083/runs.retry.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/runs.retry.output.schema.json", "required_input": ["run_id", "caller_request_id"], "enum_constraints": {}, "denial_codes": ["request_intent_mismatch", "malformed_request_id", "lifecycle_not_actionable", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "stages.retry", "input_schema_path": "docs/reference/mcp/p083/stages.retry.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/stages.retry.output.schema.json", "required_input": ["stage_execution_id", "caller_request_id"], "enum_constraints": {}, "denial_codes": ["request_intent_mismatch", "malformed_request_id", "lifecycle_not_actionable", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "approvals.resolve", "input_schema_path": "docs/reference/mcp/p083/approvals.resolve.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/approvals.resolve.output.schema.json", "required_input": ["approval_id", "resolution", "caller_request_id"], "enum_constraints": {"resolution": ["approve", "reject"]}, "denial_codes": ["approval_not_pending", "malformed_request_id", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "side_effects.force_reconcile", "input_schema_path": "docs/reference/mcp/p083/side_effects.force_reconcile.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/side_effects.force_reconcile.output.schema.json", "required_input": ["side_effect_id", "caller_request_id"], "enum_constraints": {}, "denial_codes": ["side_effect_not_pending", "malformed_request_id", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "provider_session.shutdown", "input_schema_path": "docs/reference/mcp/p083/provider_session.shutdown.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/provider_session.shutdown.output.schema.json", "required_input": ["provider_session_id", "caller_request_id"], "enum_constraints": {}, "denial_codes": ["provider_session_not_cancellable", "identity_ambiguous", "malformed_request_id", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "provider_session.mark_process_absent", "input_schema_path": "docs/reference/mcp/p083/provider_session.mark_process_absent.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/provider_session.mark_process_absent.output.schema.json", "required_input": ["provider_session_id", "cancellation_epoch", "caller_request_id"], "enum_constraints": {}, "denial_codes": ["provider_session_not_found", "identity_ambiguous", "malformed_request_id", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "p083.rollback_execution", "input_schema_path": "docs/reference/mcp/p083/p083.rollback_execution.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/p083.rollback_execution.output.schema.json", "required_input": ["target_enforcement_mode", "caller_request_id"], "enum_constraints": {"target_enforcement_mode": ["permissive", "disabled"]}, "denial_codes": ["enforcement_mode_blocked", "principal_class_not_allowed", "rollback_target_required", "rollback_target_invalid", "schema_invalid", "additional_properties_rejected"]},
      {"tool": "p083.set_enforcement_mode", "input_schema_path": "docs/reference/mcp/p083/p083.set_enforcement_mode.input.schema.json", "output_schema_path": "docs/reference/mcp/p083/p083.set_enforcement_mode.output.schema.json", "required_input": ["target_mode", "caller_request_id"], "enum_constraints": {"target_mode": ["disabled", "permissive", "enforce"]}, "denial_codes": ["enforcement_mode_blocked", "principal_class_not_allowed", "schema_invalid", "additional_properties_rejected"]}
    ],
    "shared_denial_vocabulary": [
      "missing_caller_request_id",
      "malformed_request_id",
      "principal_class_not_allowed",
      "lifecycle_state_invalid",
      "schema_invalid",
      "additional_properties_rejected",
      "rollback_target_required",
      "rollback_target_invalid",
      "request_intent_mismatch",
      "idempotency_in_flight",
      "idempotency_replayed",
      "idempotency_expired_reacquired",
      "operator_required",
      "p083_operator_required",
      "provider_session_not_found",
      "run_not_found",
      "stage_not_retryable",
      "approval_not_actionable",
      "side_effect_not_reconcilable",
      "enforcement_mode_transition_denied",
      "identity_ambiguous",
      "idempotency_replay_corrupt",
      "idempotency_terminal_failure",
      "internal"
    ],
    "additional_properties_policy": "Every tool input and output schema sets additionalProperties:false at every object level. Unknown fields are denied with additional_properties_rejected before any side effect.",
    "parity_rule": "shared_denial_vocabulary names match graphql_sdl_contract_v1.DenialReason values byte-for-byte; CI fixture rejects any drift.",
    "fixtures": [
      "docs/evidence/083/api/mcp-tool-inventory-additional-properties-false.fixture.json",
      "docs/evidence/083/api/mcp-tool-inventory-denial-vocabulary-parity.fixture.json",
      "docs/evidence/083/api/mcp-tool-input-unknown-property-rejected.fixture.json",
      "docs/evidence/083/api/mcp-tool-output-schema-matches-graphql.fixture.json",
      "docs/evidence/083/api/mcp-rollback-target-required.fixture.json",
      "docs/evidence/083/api/mcp-rollback-target-invalid-enum-rejected.fixture.json",
      "docs/evidence/083/api/mcp-approval-resolution-enum-constraint.fixture.json",
      "docs/evidence/083/api/mcp-set-enforcement-mode-enum-constraint.fixture.json"
    ]
  },
  "shutdown_contract_v1": {
    "addresses": [],
    "schema_version": "shutdown_contract_v1",
    "termination_classification": {
      "graceful_appkit": ["normal_quit", "logout_or_system_shutdown_when_applicationShouldTerminate_is_invoked"],
      "abrupt_external": ["force_quit", "sigkill", "process_crash", "host_power_loss"],
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
      "baseline_sample_id TEXT",
      "error_code TEXT"
    ],
    "unique_key": "UNIQUE(provider_session_id, shutdown_epoch, signal_kind, generation)",
    "process_identity_guard": "A signal may be issued only when stored process_id and process_start_identity match the current OS process identity. Mismatch records identity_mismatch and holds with operator_next_step_code=manual_process_identity_check.",
    "addresses": [],
    "baseline_correlation_rule": "When issued_at_monotonic_ms is recorded, baseline_sample_id references the durable_monotonic_clock_samples row used to convert the monotonic timestamp. Recovery code converts using the referenced baseline if its boot_id matches the current OS boot_id; otherwise it falls back to issued_at_wall_clock per durable_monotonic_clock_contract_v1.restart_reboot_rule.",
    "generation_replay_rule": {
      "reuse_same_generation": "Recovery reuses the same (provider_session_id, shutdown_epoch, signal_kind, generation) when a row exists with intent_state planned or issued and process identity still matches. planned without issued_at may issue that generation once; issued suppresses duplicate send and continues observation.",
      "increment_generation": "A new generation is created only when the prior generation is terminal observed, suppressed, identity_mismatch, or when policy opens a new shutdown_epoch after queued_no_signal retry. Generation is never incremented merely because the daemon restarted.",
      "duplicate_suppression": "The unique key suppresses duplicate sends for an already issued generation and emits shutdown_duplicate_signal_suppressed_total{provider}."
    },
    "fixtures": [
      "docs/evidence/083/shutdown-signal/crash-after-planned-reuses-generation-and-issues-once.fixture.json",
      "docs/evidence/083/shutdown-signal/crash-after-issued-reuses-generation-and-suppresses-duplicate.fixture.json",
      "docs/evidence/083/shutdown-signal/observed-generation-next-wave-increments.fixture.json",
      "docs/evidence/083/shutdown-signal/restart-alone-does-not-increment-generation.fixture.json",
      "docs/evidence/083/shutdown-signal/baseline-sample-id-recorded-on-issued.fixture.json"
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
      "baseline_sample_id TEXT",
      "shutdown_epoch INTEGER NULL",
      "shutdown_epoch_assigned_at TEXT NULL",
      "PRIMARY KEY(provider_session_id, cancellation_epoch)"
    ],
    "baseline_correlation_rule": "baseline_sample_id references the durable_monotonic_clock_samples row used to record requested_at_monotonic_ms. If null (legacy/fallback path), recovery uses the nearest baseline at-or-before requested_at_monotonic_ms whose boot_id matches the current OS boot_id, or falls back to requested_at_wall_clock.",
    "identity_ambiguous_canonical_rule": "If restart recovery cannot prove the provider process is live, absent, or already interrupted for a requested intent with shutdown_epoch IS NULL, it transactionally sets provider_cancellation_intents.intent_state='held', keeps shutdown_epoch NULL, sets provider_sessions.process_fate='identity_ambiguous', and returns operator_next_step_code=manual_process_identity_check. Held identity_ambiguous intents are not automatically retried on every restart; only operator action or new process evidence can move them back to requested or shutdown_started.",
    "fixtures": [
      "docs/evidence/083/cancellation/requested-null-epoch-identity-ambiguous-transitions-to-held.fixture.json",
      "docs/evidence/083/cancellation/held-identity-ambiguous-not-retried-on-restart.fixture.json",
      "docs/evidence/083/cancellation/operator-resolves-held-identity-ambiguous.fixture.json",
      "docs/evidence/083/cancellation/baseline-sample-id-recorded-on-requested.fixture.json"
    ],
    "metric_rule": "provider_cancellation_intent_total uses bounded labels provider,intent_state,cancellation_reason. provider_session_lifecycle_total never emits cancellation_requested."
  },
  "migration_plan_v1": {
    "schema_version": "migration_plan_v1",
    "addresses": [],
    "ordering_rule": "Migrations are applied in listed order. Every release receipt, run_report, GraphQL, and MCP rollout readback lane exposes logical_id, filename, sha256, dependencies, applied_at, schema_version, state, and verification_query_result for each row.",
    "sha256_rule": "The gate computes SHA-256 over the migration file bytes at implementation time. Proposal text records expected sha256_source='migration_file_bytes' and readback must include the computed sha256; hard-coded placeholder hashes are rejected.",
    "rollback_rule": "Rollback never drops P083 additive schema. Rollback changes enforcement mode to permissive or disabled through rollback_execution_v1 and leaves all evidence queryable.",
    "migrations": [
      {
        "logical_id": "p083_001_artifact_lineage_report_kind",
        "filename": "control-plane/crates/db/migrations/087_p083_001_artifact_lineage_report_kind.sql",
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
        "filename": "control-plane/crates/db/migrations/088_p083_002_command_idempotency_generations.sql",
        "depends_on": ["p083_001_artifact_lineage_report_kind"],
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
        "filename": "control-plane/crates/db/migrations/089_p083_003_shutdown_receipts_and_signals.sql",
        "depends_on": ["p083_002_command_idempotency_generations"],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_003_shutdown_receipts_and_signals].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE shutdown_interrupted_receipts(... queue_rank INTEGER NULL ... CHECK queue_rank is non-null only for queued_no_signal)",
          "CREATE UNIQUE INDEX shutdown_interrupted_receipts_epoch_generation_uniq ON shutdown_interrupted_receipts(provider_session_id, shutdown_epoch, receipt_generation)",
          "CREATE TABLE shutdown_signal_side_effects(... generation INTEGER NOT NULL, intent_state TEXT NOT NULL, baseline_sample_id TEXT NULL REFERENCES durable_monotonic_clock_samples(sample_id) ...)",
          "CREATE UNIQUE INDEX shutdown_signal_side_effect_unique ON shutdown_signal_side_effects(provider_session_id, shutdown_epoch, signal_kind, generation)"
        ],
        "verification_query": "SELECT receipt_id FROM shutdown_interrupted_receipts WHERE (interrupted_state = 'queued_no_signal' AND queue_rank IS NULL) OR (interrupted_state <> 'queued_no_signal' AND queue_rank IS NOT NULL);",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_004_cancel_late_output_overflow",
        "filename": "control-plane/crates/db/migrations/090_p083_004_cancel_late_output_overflow.sql",
        "depends_on": ["p083_003_shutdown_receipts_and_signals"],
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
        "filename": "control-plane/crates/db/migrations/091_p083_005_enforcement_and_rollback.sql",
        "depends_on": ["p083_004_cancel_late_output_overflow"],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_005_enforcement_and_rollback].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE p083_enforcement_mode_state(...) ",
          "CREATE TABLE p083_enforcement_mode_transition_journal(...) ",
          "CREATE TABLE p083_enforcement_mode_audit(...) ",
          "CREATE TABLE p083_rollback_audit(... target_enforcement_mode TEXT NOT NULL CHECK(target_enforcement_mode IN ('permissive','disabled')) ...)"
        ],
        "verification_query": "SELECT COUNT(*) FROM p083_enforcement_mode_transition_journal WHERE transition_state = 'transitioning' AND commit_marker IS NOT NULL;",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_006_durable_monotonic_clock",
        "filename": "control-plane/crates/db/migrations/092_p083_006_durable_monotonic_clock.sql",
        "depends_on": ["p083_005_enforcement_and_rollback"],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_006_durable_monotonic_clock].sha256 equals computed migration file hash and state='applied' after daemon start records baseline sample",
        "ddl_summary": [
          "CREATE TABLE durable_monotonic_clock_samples(sample_id TEXT PRIMARY KEY, boot_id TEXT NOT NULL, sample_state TEXT NOT NULL CHECK(sample_state IN ('baseline','periodic','fallback_wall_only')), monotonic_ms INTEGER NOT NULL, wall_clock_iso8601 TEXT NOT NULL, observed_at_wall_clock TEXT NOT NULL, clock_skew_ms INTEGER NULL, baseline_generation INTEGER NOT NULL)",
          "CREATE INDEX durable_monotonic_clock_samples_boot_idx ON durable_monotonic_clock_samples(boot_id, observed_at_wall_clock)",
          "CREATE UNIQUE INDEX durable_monotonic_clock_samples_boot_baseline_uniq ON durable_monotonic_clock_samples(boot_id, baseline_generation) WHERE sample_state = 'baseline'"
        ],
        "verification_query": "SELECT COUNT(*) FROM durable_monotonic_clock_samples WHERE sample_state = 'baseline';",
        "expected_verification_result": "at least one row after daemon start"
      },
      {
        "logical_id": "p083_007_provider_cancellation_intent_and_process_fate",
        "filename": "control-plane/crates/db/migrations/093_p083_007_provider_cancellation_intent_and_process_fate.sql",
        "depends_on": ["p083_006_durable_monotonic_clock"],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_007_provider_cancellation_intent_and_process_fate].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "CREATE TABLE provider_cancellation_intents(provider_session_id TEXT NOT NULL, cancellation_epoch INTEGER NOT NULL, intent_state TEXT NOT NULL CHECK(intent_state IN ('requested','shutdown_started','settled','held')), reason TEXT NOT NULL CHECK(reason IN ('operator_cancel','backpressure_cutoff','shutdown_recovery')), requested_at_monotonic_ms INTEGER NOT NULL, requested_at_wall_clock TEXT NOT NULL, baseline_sample_id TEXT NULL REFERENCES durable_monotonic_clock_samples(sample_id), shutdown_epoch INTEGER NULL, shutdown_epoch_assigned_at TEXT NULL, PRIMARY KEY(provider_session_id, cancellation_epoch))",
          "CREATE INDEX provider_cancellation_intents_shutdown_epoch_idx ON provider_cancellation_intents(provider_session_id, shutdown_epoch) WHERE shutdown_epoch IS NOT NULL",
          "CREATE INDEX provider_cancellation_intents_state_idx ON provider_cancellation_intents(intent_state, reason)",
          "ALTER TABLE provider_sessions ADD COLUMN process_fate TEXT NOT NULL DEFAULT 'running' CHECK(process_fate IN ('running','backpressure_cutoff_shutdown_pending','absent_verified','interrupted_receipt_recorded','identity_ambiguous'))",
          "ALTER TABLE provider_sessions ADD COLUMN process_fate_updated_at TEXT NULL",
          "CREATE INDEX provider_sessions_process_fate_idx ON provider_sessions(process_fate)"
        ],
        "verification_query": "SELECT provider_session_id FROM provider_cancellation_intents WHERE intent_state IN ('shutdown_started','settled') AND shutdown_epoch IS NULL;",
        "expected_verification_result": "zero rows"
      },
      {
        "logical_id": "p083_008_signal_dispatching_state",
        "filename": "control-plane/crates/db/migrations/094_p083_008_signal_dispatching_state.sql",
        "depends_on": ["p083_007_provider_cancellation_intent_and_process_fate"],
        "sha256_source": "migration_file_bytes",
        "readback_expectation": "applied_migrations[p083_008_signal_dispatching_state].sha256 equals computed migration file hash and state='applied'",
        "ddl_summary": [
          "Recreate shutdown_signal_side_effects so intent_state CHECK admits 'dispatching'",
          "Preserve existing shutdown_signal_side_effects rows during table recreation",
          "Recreate shutdown_signal_side_effect_unique and shutdown_signal_side_effects_session_idx"
        ],
        "verification_query": "SELECT COUNT(*) FROM shutdown_signal_side_effects WHERE intent_state NOT IN ('planned','dispatching','issued','observed','suppressed','identity_mismatch');",
        "expected_verification_result": "zero rows"
      }
    ]
  },
  "rollout_readback_api_parity_v1": {
    "schema_version": "rollout_readback_api_parity_v1",
    "addresses": [],
    "normalization_rules": [
      "run_report, mcp, and release_receipt use snake_case keys exactly matching operator_readback_v1.",
      "GraphQL uses camelCase key projection with the same string values.",
      "Every declared field is required in every lane; nullable fields are present with explicit null.",
      "p083_shutdown_queue_rank is null except for queued_no_signal receipt readback and equals stored shutdown_interrupted_receipts.queue_rank when present.",
      "p083_rollback_target_enforcement_mode is included in rollout readback lanes when an active rollback action exists; otherwise it is null."
    ],
    "graphql_sdl": [
      "scalar RollbackDispositionJSON",
      "type RolloutContractReadback { rolloutContractStatus: String! rolloutContractDecision: String! rolloutContractFailureReasons: [String!]! rolloutContractWaiverState: String! rolloutContractWaiverExpiresAt: String rolloutContractEnforcementMode: P083EnforcementMode! rolloutContractEnforcementModeReason: String! rolloutContractHoldConditions: [String!]! rolloutContractRollbackDisposition: RollbackDispositionJSON! rolloutContractSourceLane: String! rolloutContractEnabledState: String! rolloutContractDisabledReasonCode: String rolloutContractActionId: String rolloutContractOperatorMessage: String! rolloutContractProjectionIntegrity: String! rolloutContractCutoverPolicyRevision: String! rolloutContractDiagnosticRedaction: String! rolloutContractNextSteps: [String!]! rolloutContractShutdownDeadlineConfigState: String! rolloutContractCommandLeaseTtlConfigState: String! p083RollbackTtlExpiresAt: String p083LastPreflightHash: String p083ShutdownQueueRank: Int p083RollbackTargetEnforcementMode: P083RollbackTargetMode }"
    ],
    "mcp_schema_queue_rank_rule": "MCP property p083_shutdown_queue_rank has type [integer,null], minimum 0, is required, and is null unless the selected receipt interrupted_state is queued_no_signal.",
    "mcp_schema_rollback_target_rule": "MCP property p083_rollback_target_enforcement_mode has type [string,null] with enum constraint ['permissive','disabled',null], is required, and is null unless an active rollback action exists.",
    "run_report_parity": "run_report includes rollout_contract_shutdown_deadline_config_state, rollout_contract_command_lease_ttl_config_state, p083_shutdown_queue_rank, and p083_rollback_target_enforcement_mode with the same nullability as MCP.",
    "release_receipt_parity": "release_receipt includes the same rollout readback fields as run_report and MCP, including p083_shutdown_queue_rank and p083_rollback_target_enforcement_mode.",
    "rollback_disposition_output_validation": "Before GraphQL serialization, resolver construction validates generated rollout_contract_rollback_disposition readback against rollback_disposition_v1, including schema_version. The inline rollout_contract_v1.rollback_disposition remains strict-template-compatible without schema_version; MCP, run_report, release_receipt, and GraphQL fixture examples all include schema_version:'rollback_disposition_v1'.",
    "negative_fixtures": [
      "docs/evidence/083/api/rollback-disposition-missing-schema-version-rejected.fixture.json",
      "docs/evidence/083/api/rollback-target-enforcement-mode-invalid-enum-rejected.fixture.json"
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
  "durable_monotonic_clock_contract_v1": {
    "schema_version": "durable_monotonic_clock_contract_v1",
    "addresses": ["REL-P083-R69-NB-001"],
    "authority_table": "durable_monotonic_clock_samples",
    "boot_id_rule": "boot_id is a stable per-OS-boot identifier sourced from the OS (kern.bootsessionuuid on macOS, /proc/sys/kernel/random/boot_id on Linux for parity testing). The daemon writes a sample_state='baseline' row on every start that captures sample_id, boot_id, baseline_generation, monotonic_ms (CLOCK_MONOTONIC equivalent), wall_clock_iso8601, and observed_at_wall_clock in the same SQLite transaction.",
    "baseline_sample_rule": "Exactly one sample_state='baseline' row exists per (boot_id, baseline_generation). baseline_generation starts at 1 on first daemon start of a given boot_id and increments only when a new baseline is recorded by the clock_skew_rule or clock_rollback_rule. Periodic samples write sample_state='periodic' at most every 60 seconds and reference the active baseline by its sample_id. fallback_wall_only is written when the OS does not expose a monotonic source.",
    "baseline_correlation_rule": "Every deadline-bearing row that records a monotonic timestamp (shutdown_signal_side_effects.issued_at_monotonic_ms, provider_cancellation_intents.requested_at_monotonic_ms, command_idempotency.acquired_at and expires_at when monotonic-anchored) MUST store the baseline_sample_id used to compute the conversion. If recording code cannot record baseline_sample_id (legacy/fallback path), recovery code MUST use the nearest baseline at-or-before the captured monotonic_ms whose boot_id matches the current OS boot_id. Both paths are covered by fixtures.",
    "monotonic_to_wall_clock_conversion": "For any stored monotonic_ms value with baseline_sample_id set: wall_clock_iso8601(baseline by sample_id) + (monotonic_ms - baseline.monotonic_ms) milliseconds, only when baseline.boot_id equals the current OS boot_id. For values without baseline_sample_id, recovery uses the nearest baseline at-or-before monotonic_ms whose boot_id matches.",
    "restart_reboot_rule": "On daemon restart, recovery loads the most recent baseline. If the referenced baseline_sample_id's boot_id differs from the current OS boot_id (reboot), recovery may not convert pre-reboot monotonic_ms values to current wall clock; deadlines computed in the prior boot are re-derived from stored wall_clock_iso8601 fields instead.",
    "clock_skew_rule": "Each periodic sample records clock_skew_ms = wall_clock_now - (baseline.wall_clock_iso8601 + (monotonic_now - baseline.monotonic_ms)). If absolute skew exceeds 5000 ms, recovery emits durable_monotonic_clock_skew_total{boot_id,direction} and writes a new baseline with baseline_generation+1. Negative skew never causes deadlines to fire early; deadlines wait at least the wall-clock target as a floor.",
    "clock_rollback_rule": "If wall clock moves backwards beyond 1000 ms relative to the prior baseline, recovery treats the prior baseline as stale, writes a new baseline marked sample_state='baseline' with baseline_generation+1 and the new wall clock, and emits durable_monotonic_clock_rollback_total{boot_id}. Existing deadlines are recomputed from stored wall_clock_iso8601, not from monotonic_ms.",
    "stale_baseline_fallback_rule": "If no baseline row exists for the current boot_id within 30 seconds of daemon start, recovery enters sample_state='fallback_wall_only' mode and disables monotonic-based deadlines until a baseline is recorded. Lifecycle commands proceed with wall-clock-only deadlines; rollout_contract_lint records durable_monotonic_clock_fallback_active for visibility.",
    "fixtures": [
      "docs/evidence/083/clock/baseline-recorded-on-daemon-start.fixture.json",
      "docs/evidence/083/clock/monotonic-to-wall-clock-conversion-with-baseline-sample-id.fixture.json",
      "docs/evidence/083/clock/legacy-row-without-baseline-sample-id-uses-nearest-at-or-before.fixture.json",
      "docs/evidence/083/clock/reboot-new-boot-id-deadlines-recomputed-from-wall-clock.fixture.json",
      "docs/evidence/083/clock/clock-skew-exceeds-5s-new-baseline-and-metric.fixture.json",
      "docs/evidence/083/clock/clock-rollback-detected-new-baseline-no-early-fire.fixture.json",
      "docs/evidence/083/clock/no-baseline-within-30s-fallback-wall-only.fixture.json"
    ],
    "metric_rule": "durable_monotonic_clock_skew_total{boot_id,direction} and durable_monotonic_clock_rollback_total{boot_id} appear in metric_labels_contract_v1 with bounded label domains; direction is one of {forward,backward} and boot_id is hashed before label emission."
  },
  "swiftdata_lifecycle_boundary_contract_v1": {
    "schema_version": "swiftdata_lifecycle_boundary_contract_v1",
    "addresses": [],
    "rule": "SwiftData may hold projection-only and app-local state, but lifecycle truth remains backend/SQLite-owned. Lifecycle-bearing roots never receive the app-scoped lifecycle ModelContainer or a mutable lifecycle modelContext.",
    "swift_concurrency_isolation_rule": {
      "main_actor_projection_access": "Projection ModelContext instances used to read projection-only stores for SwiftUI rendering are pinned to @MainActor. SwiftUI Views, @ModelContext property wrappers attached to projection schemas, and projection read helpers carry @MainActor isolation.",
      "model_actor_non_main_access": "Non-main projection writes and background projection rebuilds run inside a @ModelActor-annotated adapter (RunProjectionWriter, e.g., @ModelActor actor RunProjectionWriter) whose ModelContext is created from a container instance owned by that actor; main-thread code never touches that ModelContext.",
      "sendable_snapshots_rule": "Values that cross the boundary between the @ModelActor adapter and @MainActor SwiftUI roots are Sendable value-type projection snapshots (structs marked Sendable), never @Model reference types and never ModelContext-bound objects.",
      "guardrails": [
        "Static scan rejects @MainActor-annotated code paths that pass ModelContext, PersistentModel reference values, or non-Sendable types to background tasks.",
        "Static scan rejects passing @ModelActor-owned ModelContext or @Model values to SwiftUI Views.",
        "Build-time check fails if any lifecycle @Model type is constructed outside the @ModelActor adapter."
      ],
      "fixtures": [
        "docs/evidence/083/swift/projection-modelcontext-main-actor-pinned.fixture.json",
        "docs/evidence/083/swift/model-actor-adapter-owns-non-main-context.fixture.json",
        "docs/evidence/083/swift/sendable-projection-snapshot-crosses-boundary.fixture.json",
        "docs/evidence/083/swift/non-sendable-leak-across-actor-boundary-rejected.fixture.json"
      ]
    },
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
    "addresses": [],
    "required_before_ready": [
      "Latest implementation review summary against this revision returns decision=approve, blocker_count=0, and aggregate_score above the freeze threshold.",
      "Selected reviewer artifacts all carry proposal_revision_id equal to the current proposal_revision_id of this file.",
      "Aggregate review summary includes the corpus-only-current-revision attestation."
    ],
    "freeze_rule": "Ready may be claimed only after a fresh aggregate implementation review against this exact proposal_revision_id returns decision=approve and blocker_count=0 with a current-revision-only corpus attestation in the review summary. Until then, status remains implementation_in_progress and closeout cannot proceed.",
    "routing_note": "Corpus integrity remains a rollout precondition; if a future review pass selects stale-revision reviewer artifacts, corpus_mixed_revision is an explicit rollout hold condition with a negative fixture so the gate fails closed.",
    "corpus_only_current_revision_attestation_required_fields": [
      "review_pass_id",
      "selected_reviewer_artifact_ids",
      "selected_reviewer_artifact_proposal_revision_ids",
      "all_match_current_proposal_revision_id_assertion",
      "no_stale_revision_in_corpus_assertion"
    ]
  },
  "metric_labels_contract_v1": {
    "schema_version": "metric_labels_contract_v1",
    "addresses": [],
    "authority_rule": "This section is the source metric inventory and owns bounded label domains. metrics.operational_metrics_reference and rollout_contract_v1.metrics.operational_metrics are generated mirrors and must remain byte-equal.",
    "bounded_label_domains": {
      "surface": ["graphql", "mcp", "run_report", "release_receipt", "swift_ui"],
      "state": ["fresh", "stale", "missing", "unknown", "tampered"],
      "lifecycle_state": [
        "registered", "spawn_error_no_child", "launch_handshake", "live", "self_exit_observed",
        "terminated_graceful", "terminated_by_kill", "orphan_settled", "shutdown_interrupted", "backpressure_cutoff"
      ],
      "outcome": ["acquired", "replayed", "denied", "committed", "failed", "abandoned", "expired_reacquired"],
      "proposal_id": ["P083"],
      "status": [
        "pass", "fail", "waived", "not_applicable", "timeout", "cancelled",
        "missing_contract", "stale", "tamper_detected"
      ],
      "failure_reason": [
        "schema_invalid", "missing_fixture", "metric_unbounded", "auth_dependency_missing",
        "hold_condition_present", "burn_in_incomplete", "rollback_contract_invalid",
        "stale_revision", "tamper_detected", "missing_schema_version", "corpus_mixed_revision",
        "rollback_target_required", "rollback_target_invalid"
      ],
      "reason": [
        "auth_dependency_missing", "hold_condition_present", "projection_not_fresh",
        "migration_not_applied", "rollback_ttl_expired", "gate_failed",
        "current_review_missing", "identity_ambiguous", "corpus_mixed_revision",
        "rollback_target_required", "rollback_target_invalid"
      ],
      "enforcement_mode": ["disabled", "permissive", "enforce"],
      "rollback_target": ["permissive", "disabled"],
      "approval_resolution": ["approve", "reject"],
      "transition": [
        "disabled_to_permissive", "permissive_to_enforce", "enforce_to_permissive",
        "permissive_to_disabled", "disabled_to_enforce_denied"
      ],
      "action": [
        "disable_to_permissive", "permissive_to_enforce", "enforce_to_permissive",
        "rollback_disable", "reenable_after_rollback", "manual_process_identity_check"
      ],
      "cancellation_reason": ["operator_cancel", "backpressure_cutoff", "shutdown_recovery"],
      "command": [
        "runs.cancel", "runs.retry", "stages.retry", "approvals.resolve",
        "side_effects.force_reconcile", "command.run", "copyable_command.regenerate",
        "provider_session.shutdown", "provider_session.mark_process_absent",
        "p083.rollback_execution", "p083.set_enforcement_mode"
      ],
      "provider": ["codex", "claude", "gemini", "auggie", "junie"],
      "intent_state": [
        "requested", "shutdown_started", "settled", "held",
        "planned", "issued", "observed", "suppressed", "identity_mismatch"
      ],
      "process_fate": [
        "running", "backpressure_cutoff_shutdown_pending", "absent_verified",
        "interrupted_receipt_recorded", "identity_ambiguous"
      ],
      "scope": ["session", "run", "global"],
      "overflow_kind": ["message_count", "session_bytes", "elapsed_time", "run_bytes", "global_bytes"],
      "interrupted_state": [
        "grace_deadline_expired", "kill_signal_issued", "kill_pid_exit_observed",
        "queued_no_signal", "shutdown_interrupted"
      ],
      "direction": ["forward", "backward"],
      "boot_id": ["hashed_boot_id"]
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
      "p083_rollback_execution_total{action,status,reason,rollback_target}",
      "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}",
      "durable_monotonic_clock_skew_total{boot_id,direction}",
      "durable_monotonic_clock_rollback_total{boot_id}",
      "approvals_resolve_total{command,outcome,approval_resolution}"
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
      "p083_rollback_execution_total{action,status,reason,rollback_target}",
      "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}",
      "durable_monotonic_clock_skew_total{boot_id,direction}",
      "durable_monotonic_clock_rollback_total{boot_id}",
      "approvals_resolve_total{command,outcome,approval_resolution}"
    ],
    "success_thresholds": {
      "preflight_pass_rate": ">= 99% for applicable runs during permissive burn-in",
      "metric_staleness": "all required scrapes fresher than 180 seconds",
      "rollback_readback_parity": "byte-equal rollback_disposition JSON across MCP, run_report, release_receipt, and GraphQL scalar payload"
    }
  },
  "rollout": {
    "phases": [
      {"phase": "implementation_closeout", "entry": "fresh aggregate implementation review against this revision returns approve with blocker_count=0 and corpus-only-current-revision attestation", "exit": "proposal-083 gate and rollout contract lint pass"},
      {"phase": "additive_migrations", "entry": "Ready proposal", "exit": "migration readback fixture passes"},
      {"phase": "permissive_burn_in", "entry": "mode transition disabled_to_permissive audited", "exit": "24 hours with zero hold conditions and fresh metrics"},
      {"phase": "enforce_cutover", "entry": "preflight requirements pass", "exit": "mode transition permissive_to_enforce audited"},
      {"phase": "rollback_if_needed", "entry": "hold condition or operator emergency", "exit": "rollback_execution_v1 readback and audit rows present with non-null target_enforcement_mode"}
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
      "corpus_mixed_revision",
      "command_idempotency_contract_invalid",
      "migration_readback_sha256_missing",
      "manual_identity_check_unresolved",
      "durable_monotonic_clock_baseline_missing",
      "rollback_target_contract_invalid"
    ],
    "fixture_readiness_rule": "rollout_contract_v1 declares P083-owned fixture paths, and scripts/lint-rollout-contract must pass against those paths before design freeze. Missing P083 readback or negative fixtures are a release hold. Each P083 fixture must assert proposal_id=P083 plus the active proposal_revision_id."
  },
  "rollout_contract_v1": {
    "schema_version": "rollout_contract_v1",
    "applicability": "required",
    "gate_aliases": ["proposal-083", "p083"],
    "commands": {
      "allowlist": ["./scripts/test-gate.sh proposal-083", "./scripts/test-gate.sh p083"],
      "commentary": "Gate commands are declarative expectations; the linter does not execute them."
    },
    "migrations": {
      "not_applicable": false,
      "justification": "P083 owns eight additive SQLite migrations enumerated in migration_plan_v1. Release receipt and operator readback must expose sha256 and verification query result for each logical_id."
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
        "p083_rollback_execution_total{action,status,reason,rollback_target}",
        "provider_cancellation_intent_total{provider,intent_state,cancellation_reason}",
        "durable_monotonic_clock_skew_total{boot_id,direction}",
        "durable_monotonic_clock_rollback_total{boot_id}",
        "approvals_resolve_total{command,outcome,approval_resolution}"
      ]
    },
    "readback_lanes": ["run_report", "mcp", "release_receipt", "graphql"],
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
      "p083_shutdown_queue_rank",
      "p083_rollback_target_enforcement_mode"
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
      "p083_shutdown_queue_rank",
      "p083_rollback_target_enforcement_mode"
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
      "corpus_mixed_revision",
      "command_idempotency_contract_invalid",
      "migration_readback_sha256_missing",
      "manual_identity_check_unresolved",
      "durable_monotonic_clock_baseline_missing",
      "rollback_target_contract_invalid"
    ],
    "hold_conditions_detail": {
      "current_security_review_missing_or_stale": "Security review artifact must name this proposal_revision_id and contain no blocking issues.",
      "current_observability_rollout_review_missing_or_stale": "Observability rollout review artifact must name this proposal_revision_id and contain no blocking issues.",
      "corpus_mixed_revision": "Aggregated review corpus contains any reviewer artifact whose proposal_revision_id does not equal the current proposal_revision_id of this proposal.",
      "shutdown_deadline_config_invalid": "Configured shutdown deadline exceeds hard maximum or claims AppKit coverage for abrupt_external termination.",
      "command_lease_ttl_config_invalid": "Configured command lease TTL is outside reliability bounds.",
      "command_idempotency_contract_invalid": "Command idempotency schema, intent_hash composition, TTL, or recovery fixtures fail.",
      "migration_readback_sha256_missing": "Any P083 migration lacks sha256 readback or verification_query_result.",
      "manual_identity_check_unresolved": "A provider cancellation intent is held for manual_process_identity_check and blocks enforcement cutover.",
      "durable_monotonic_clock_baseline_missing": "durable_monotonic_clock_samples has no baseline row for the current boot_id, or the daemon entered fallback_wall_only mode without recovery.",
      "rollback_target_contract_invalid": "GraphQL, MCP, intent_hash composition, and rollback_disposition do not byte-agree on the p083.rollback_execution target_enforcement_mode field, enum domain, or required[] arrays."
    },
    "rollback_disposition": {
      "mode": "p083.rollback_execution_to_permissive_or_disabled",
      "data_loss_risk": "none",
      "steps": [
        "Call p083RollbackExecution or p083.rollback_execution with operator principal class, a non-null targetEnforcementMode (permissive or disabled), and CallerRequestId.",
        "Persist rollback audit (including target_enforcement_mode) and enforcement-mode audit rows.",
        "Expose disabled/permissive state, generated schema-versioned rollback disposition readback (including p083_rollback_target_enforcement_mode), and TTL in every readback lane.",
        "Require fresh permissive burn-in and enforce preflight before returning to enforce mode."
      ]
    },
    "decision_vocabulary": [
      "pass", "fail", "waived", "not_applicable", "timeout", "cancelled",
      "missing_contract", "tamper_detected", "stale", "release", "hold", "waive"
    ],
    "negative_fixtures": {
      "missing_metric_domain": "docs/evidence/rollout-contract/negative/p083-missing-metric-domain.json",
      "missing_rollback_contract": "docs/evidence/rollout-contract/negative/p083-missing-rollback-contract.json",
      "foreign_fixture_reference": "docs/evidence/rollout-contract/negative/p083-foreign-fixture-reference.json",
      "enforce_without_burnin": "docs/evidence/rollout-contract/negative/p083-enforce-without-burnin.json",
      "stale_security_review": "docs/evidence/rollout-contract/negative/p083-stale-security-review.json",
      "stale_observability_rollout_review": "docs/evidence/rollout-contract/negative/p083-stale-observability-rollout-review.json",
      "corpus_mixed_revision": "docs/evidence/rollout-contract/negative/p083-corpus-mixed-revision.json",
      "force_quit_host_budget_claim": "docs/evidence/rollout-contract/negative/p083-force-quit-host-budget-claim.json",
      "queue_rank_final_readback_rank_storage": "docs/evidence/rollout-contract/negative/p083-final-readback-rank-stored.json",
      "rollback_disposition_missing_schema_version": "docs/evidence/rollout-contract/negative/p083-rollback-disposition-missing-schema-version.json",
      "migration_sha256_missing": "docs/evidence/rollout-contract/negative/p083-migration-sha256-missing.json",
      "unbounded_metric_label": "docs/evidence/rollout-contract/negative/p083-unbounded-metric-label.json",
      "durable_monotonic_clock_baseline_missing": "docs/evidence/rollout-contract/negative/p083-durable-monotonic-clock-baseline-missing.json",
      "rollback_target_contract_invalid": "docs/evidence/rollout-contract/negative/p083-rollback-target-contract-invalid.json"
    },
    "cutover_policy": {
      "revision": "p083-rollout-cutover-r70",
      "enforcement_mode_at_cutover": "enforce",
      "applicable_to": "post_ready_implementation_starts",
      "effective_timestamp_iso8601": "2026-06-04T00:00:00Z"
    }
  },
  "risks_and_mitigations": [
    {
      "risk": "Command idempotency is a shared execution-truth path and can block unrelated lifecycle commands if TTL recovery, intent_hash composition, or enum normalization is wrong.",
      "mitigation": "Use explicit per-command TTLs, per-command intent_hash_composition_rule with RFC 8785 canonical JSON serialization and lowercase enum normalization, SQLite CAS reacquire, committed-unack replay fixtures, and bounded metrics for every covered command."
    },
    {
      "risk": "Rollback execution cross-surface contradictions (e.g., GraphQL accepting one field set while idempotency hashes another) can cause request_intent_mismatch, false replay, missed replay, or unsafe emergency rollback denial.",
      "mitigation": "targetEnforcementMode is a non-null required argument in GraphQL, a required_input in MCP, included in command_idempotency_contract_v1.per_command_logical_fields[p083.rollback_execution], persisted in p083_rollback_audit, and surfaced in rollout readback as p083_rollback_target_enforcement_mode. rollback_target_contract_invalid is a rollout hold condition with a negative fixture; same-request replay, same-intent aliasing, and mismatch denial fixtures all exist."
    },
    {
      "risk": "The full migration surface increases proposal size and maintenance burden.",
      "mitigation": "Keep one canonical migration_plan_v1 inventory with sha256_source and generated readback mirrors instead of duplicating migration metadata across feature sections."
    },
    {
      "risk": "Manual identity checks can interrupt operator flow.",
      "mitigation": "Render the hold inline with copyable diagnostics, read-only retry, explicit backend actions, no focus-stealing spinner, and a deterministic primary/secondary/tertiary/overflow action hierarchy with loading/success/error feedback."
    },
    {
      "risk": "Late provider output may arrive after cancellation and look useful.",
      "mitigation": "Quarantine it as evidence and prove active projections/artifacts cannot be mutated after overflow latch activation."
    },
    {
      "risk": "Representative SwiftData stores may expose migration cases not covered by synthetic fixtures.",
      "mitigation": "Implementation sign-off requires copied pre-P083 stores spanning active runs, approvals, provider history, and artifacts."
    },
    {
      "risk": "Reviewer routing may continue to mix stale-revision artifacts with current-revision artifacts.",
      "mitigation": "current_review_refresh_gate_v1 requires a corpus-only-current-revision attestation in every aggregate review summary before Ready can be claimed; corpus_mixed_revision is an explicit rollout hold condition with a negative fixture."
    },
    {
      "risk": "Monotonic clock skew, reboot, or baseline drift may cause deadlines to fire early or late.",
      "mitigation": "durable_monotonic_clock_contract_v1 records baseline samples per (boot_id, baseline_generation), stores baseline_sample_id on deadline-bearing rows for direct correlation, falls back to nearest baseline at-or-before captured monotonic_ms for legacy rows, recomputes deadlines from stored wall_clock_iso8601 on reboot, and enters fallback_wall_only mode without a baseline."
    },
    {
      "risk": "macOS menu/toolbar drift can leave operators unable to find lifecycle commands or hide accessibility metadata.",
      "mitigation": "native_command_validation_contract_v1 pins the Commands menu structure under a Run menu, asserts enabled-state and accessibility parity between menu and toolbar via @FocusedValue wiring, and includes a parity fixture."
    }
  ],
  "open_questions": [
    "Should the permissive burn-in duration remain fixed at 24 hours or become a release-channel setting after P083 lands?",
    "Should side_effects.force_reconcile keep a 300 second TTL permanently, or should it move to a lower value after operational data is available?",
    "Which dashboard owns long-term alert thresholds for command_idempotency_contract_invalid, manual_identity_check_unresolved, durable_monotonic_clock_baseline_missing, rollback_target_contract_invalid, and corpus_mixed_revision hold conditions?"
  ],
  "acceptance_criteria": [
    "proposal-083 and p083 gates exist and run the P083 contract suite.",
    "No active proposal section, revision summary, coverage object, or feedback mapping claims blocker ids absent from the current R69 score_lift_backlog.",
    "rollout_contract_v1.rollback_disposition remains strict-template-compatible without unknown fields; generated GraphQL, MCP, run_report, and release_receipt RollbackDispositionJSON fixtures include schema_version='rollback_disposition_v1' and reject missing schema_version.",
    "migration_plan_v1 enumerates all eight P083 additive migrations with logical_id, filename, dependencies, sha256_source, readback expectation, verification query, and expected verification result.",
    "command_idempotency_contract_v1 covers runs.cancel, runs.retry, stages.retry, approvals.resolve, side_effects.force_reconcile, provider_session.shutdown, provider_session.mark_process_absent, p083.rollback_execution, and p083.set_enforcement_mode with states, TTLs, unique keys, recovery rules, per-command intent_hash_composition_rule (including enum lowercase normalization), and fixtures.",
    "p083.rollback_execution accepts a non-null targetEnforcementMode in GraphQL SDL, requires target_enforcement_mode in the MCP input schema, includes target_enforcement_mode in command_idempotency_contract_v1.per_command_logical_fields[p083.rollback_execution], persists target_enforcement_mode in p083_rollback_audit, and surfaces p083_rollback_target_enforcement_mode in rollout readback; same-request replay, same-intent aliasing across new request_id, and mismatch denial fixtures all exist.",
    "graphql_sdl_contract_v1 enumerates lifecycle mutation SDL with non-null CallerRequestId arguments, a shared DenialReason union, closed enum types ApprovalResolution/P083EnforcementMode/P083RollbackTargetMode, and byte-parity with mcp_tool_inventory_contract_v1.shared_denial_vocabulary and enum_constraints.",
    "mcp_tool_inventory_contract_v1 lists every P083 MCP tool with Draft 2020-12 input/output schemas, additionalProperties=false, enum_constraints for closed-domain fields, shared denial vocabulary, and parity fixtures with GraphQL.",
    "swiftdata_lifecycle_boundary_contract_v1.swift_concurrency_isolation_rule pins @MainActor for projection ModelContext access, requires @ModelActor for non-main projection writes, and proves Sendable snapshots cross the actor boundary.",
    "durable_monotonic_clock_contract_v1 defines boot_id semantics, baseline samples with baseline_generation, monotonic-to-wall-clock conversion via stored baseline_sample_id (or nearest-at-or-before fallback), clock-skew handling, clock-rollback handling, stale-baseline fallback, and required fixtures.",
    "ManualProcessIdentityCheckBanner renders manual_process_identity_check with visible copy, an explicit primary/secondary/tertiary/overflow action hierarchy, loading/success/error feedback states, no automatic retry spinner, explicit resolution actions, and VoiceOver-readable denial state.",
    "Every operational metric label used by operational_metric_label_signatures has a bounded domain or fails lint; rollback_target and approval_resolution are bounded.",
    "post_cancel_late_output_contract_v1 proves unique overflow latch keys, cap bounds, restart idempotency, readback fields, and a negative fixture rejecting active projection mutation after cancellation.",
    "shutdown_signal_side_effect generation is reused after crash-before-planned or crash-after-issued recovery and increments only after terminal prior generation or new shutdown epoch; duplicate sends are suppressed by fixture; baseline_sample_id is recorded on issued rows.",
    "SwiftData transition fixtures run against representative copied pre-P083 stores and prove no lifecycle modelContext leakage into lifecycle-bearing roots.",
    "native_command_validation_contract_v1 names a deterministic macOS Commands menu structure (Run menu placement for Cancel Run, Retry Run, Retry Stage, Resolve Approval, Shutdown Provider Session, Retry Identity Check), asserts enabled-state and accessibility parity between menu and toolbar via @FocusedValue, and includes a menu/toolbar parity fixture.",
    "applicationShouldTerminate terminateLater plus host_total_ms is asserted only for graceful Quit and logout/system shutdown paths where AppKit invokes the delegate callback.",
    "Force Quit, SIGKILL, and crash fixtures prove restart recovery through shutdown_signal_side_effects or provider_cancellation_intents without assuming delegate callback execution.",
    "shutdown_interrupted_receipts stores queue_rank INTEGER NULL, non-null only for interrupted_state=queued_no_signal; final_readback_rank is not a stored column.",
    "provider_cancellation_intents.requested with null shutdown_epoch and ambiguous identity transitions to intent_state=held, process_fate=identity_ambiguous, and is not retried automatically on every restart.",
    "scripts/lint-rollout-contract passes for the inline rollout_contract_v1 after all declared readback and negative fixtures are created.",
    "current_review_refresh_gate_v1 must be satisfied before Ready; aggregate review summary against this exact proposal_revision_id must include a corpus-only-current-revision attestation.",
    "All implementation_hardening_requirements_v1 requirements are implemented and proven before P083 is marked implementation-complete, closeout-ready, or release-ready.",
    "No implementation_self_assessment, review summary, closeout readiness result, or release gate may classify an implementation_hardening_requirements_v1 item as non-blocking/deferred unless a separate approved successor proposal explicitly owns that exact item and the operator approved the scope reduction.",
    "The P083 proof gate fails when any implementation_hardening_requirements_v1 item lacks code, schema, migration, metric, UI, readback, or fixture evidence required by its required_outcome."
  ],
  "reviewer_feedback_resolution": {
    "REL-P083-R69-BLOCK-001": {
      "disposition": "addressed",
      "severity": "blocking",
      "required_change": "Either add a non-null rollback target to GraphQL and MCP and include it in rollback_disposition, or define an explicit deterministic derived target and remove the unsatisfied caller-input dependency from intent_hash composition. Add fixtures for same-request replay, same-intent aliasing, and mismatch denial.",
      "resolution_choice": "Option A: non-null caller-supplied targetEnforcementMode propagated end-to-end.",
      "addressed_by_sections": [
        "graphql_sdl_contract_v1.lifecycle_mutation_signatures",
        "mcp_tool_inventory_contract_v1.tools[p083.rollback_execution]",
        "command_idempotency_contract_v1.intent_hash_composition_rule.per_command_logical_fields[p083.rollback_execution]",
        "rollout_contract_v1.rollback_disposition.steps",
        "rollout_readback_api_parity_v1.graphql_sdl",
        "rollout_readback_api_parity_v1.mcp_schema_rollback_target_rule",
        "migration_plan_v1.migrations[p083_005_enforcement_and_rollback]",
        "metric_labels_contract_v1.bounded_label_domains[rollback_target]",
        "acceptance_criteria"
      ],
      "resolution_notes": "p083RollbackExecution(targetEnforcementMode: P083RollbackTargetMode!, callerRequestId: CallerRequestId!) and MCP p083.rollback_execution required_input=['target_enforcement_mode','caller_request_id'] (enum_constraints: target_enforcement_mode in {permissive, disabled}) are byte-aligned with command_idempotency_contract_v1.per_command_logical_fields[p083.rollback_execution]=['target_enforcement_mode']. p083_rollback_audit persists target_enforcement_mode. Readback exposes p083_rollback_target_enforcement_mode. Three new fixtures cover same-request replay (docs/evidence/083/idempotency/p083-rollback-same-request-replayed.fixture.json), same-intent aliasing when caller submits a new request_id with identical target (docs/evidence/083/idempotency/p083-rollback-same-intent-new-request-aliased.fixture.json), and mismatch denial when caller submits the same request_id with a different target_enforcement_mode (docs/evidence/083/idempotency/p083-rollback-request-intent-mismatch-denied.fixture.json). rollback_target_contract_invalid is added as a rollout hold condition and negative fixture so the gate fails closed on cross-surface drift."
    },
    "API-P083-R69-NB-001": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Same underlying issue as the reliability blocker; resolve across all API surfaces and command_idempotency_contract_v1.",
      "addressed_by_sections": [
        "graphql_sdl_contract_v1",
        "mcp_tool_inventory_contract_v1",
        "command_idempotency_contract_v1.intent_hash_composition_rule",
        "rollout_contract_v1.rollback_disposition.steps"
      ],
      "resolution_notes": "Resolved jointly with REL-P083-R69-BLOCK-001: SDL, MCP input schema, idempotency logical fields, audit row, and rollout readback all carry the non-null target_enforcement_mode."
    },
    "API-P083-R69-NB-002": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Replace free-form String fields with GraphQL enums and JSON Schema enum constraints, with canonical normalization before hashing.",
      "addressed_by_sections": [
        "graphql_sdl_contract_v1.closed_enum_rule",
        "graphql_sdl_contract_v1.lifecycle_mutation_signatures",
        "mcp_tool_inventory_contract_v1.enum_normalization_rule",
        "mcp_tool_inventory_contract_v1.tools[*].enum_constraints",
        "command_idempotency_contract_v1.intent_hash_composition_rule.canonical_enum_normalization_rule",
        "metric_labels_contract_v1.bounded_label_domains[rollback_target,approval_resolution,enforcement_mode]"
      ],
      "resolution_notes": "ApprovalResolution {approve, reject}, P083EnforcementMode {disabled, permissive, enforce}, and P083RollbackTargetMode {permissive, disabled} are GraphQL enums; MCP enforces matching JSON Schema enum constraints; intent_hash uses lowercase-normalized canonical values; CI fixture graphql-mcp-enum-vocabulary-parity proves byte-equal sets."
    },
    "REL-P083-R69-NB-001": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Record baseline_sample_id or baseline_generation on deadline-bearing rows, or require nearest baseline at or before captured monotonic_ms.",
      "addressed_by_sections": [
        "durable_monotonic_clock_contract_v1.baseline_correlation_rule",
        "durable_monotonic_clock_contract_v1.monotonic_to_wall_clock_conversion",
        "shutdown_signal_side_effect_contract_v1.columns",
        "shutdown_signal_side_effect_contract_v1.baseline_correlation_rule",
        "provider_cancellation_intent_contract_v1.columns",
        "provider_cancellation_intent_contract_v1.baseline_correlation_rule",
        "migration_plan_v1.migrations[p083_003_shutdown_receipts_and_signals]",
        "migration_plan_v1.migrations[p083_007_provider_cancellation_intent_and_process_fate]",
        "migration_plan_v1.migrations[p083_006_durable_monotonic_clock]"
      ],
      "resolution_notes": "shutdown_signal_side_effects.baseline_sample_id and provider_cancellation_intents.baseline_sample_id reference durable_monotonic_clock_samples(sample_id). baseline_generation is recorded on every baseline sample and increments on skew/rollback events. Recovery uses the referenced baseline directly when set, or falls back to the nearest baseline at-or-before the captured monotonic_ms with matching boot_id; both paths are covered by fixtures."
    },
    "MACOS-P083-R69-NB-002": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Name menu-bar structure and assert enabled-state/accessibility parity with toolbar controls.",
      "addressed_by_sections": [
        "native_command_validation_contract_v1.menu_bar_structure",
        "native_command_validation_contract_v1.menu_toolbar_parity_rule",
        "native_command_validation_contract_v1.fixtures",
        "architecture.swift_modules_touched"
      ],
      "resolution_notes": "Lifecycle commands live under a Run menu with deterministic order; @FocusedValue wires identical enabled-state and accessibilityHelp/accessibilityLabel between menu items and toolbar buttons; a menu/toolbar parity fixture proves byte-equal labels and enabled state for every covered command."
    },
    "UI-P083-R69-NB-001": {
      "disposition": "addressed",
      "severity": "advisory",
      "required_change": "Identify primary, secondary, tertiary, or overflow actions and add loading/copy feedback states.",
      "addressed_by_sections": [
        "manual_process_identity_check_ui_v1.action_hierarchy",
        "manual_process_identity_check_ui_v1.feedback_states",
        "manual_process_identity_check_ui_v1.fixtures",
        "ux_ui_notes.manual_process_identity_check"
      ],
      "resolution_notes": "Banner action hierarchy: primary=Retry Identity Check; secondary=Mark Process Absent; tertiary=Copy Diagnostic; overflow=Open Provider Session Evidence. Each action declares idle/loading/success/error feedback states; Copy Diagnostic shows transient 'Copied' confirmation; Mark Process Absent shows confirmation dialog and inline error on failure; Retry Identity Check shows a determinate progress label (not a focus-stealing spinner)."
    }
  },
  "command_idempotency_contract_v1": {
    "schema_version": "command_idempotency_contract_v1",
    "addresses": ["REL-P083-R69-BLOCK-001", "API-P083-R69-NB-001", "API-P083-R69-NB-002"],
    "authority": "command_idempotency and command_request_aliases SQLite tables",
    "commands_covered": [
      "runs.cancel", "runs.retry", "stages.retry", "approvals.resolve",
      "side_effects.force_reconcile", "provider_session.shutdown",
      "provider_session.mark_process_absent", "p083.rollback_execution",
      "p083.set_enforcement_mode"
    ],
    "tables": {
      "command_idempotency": {
        "primary_key": ["principal_id", "request_id", "lease_generation"],
        "states": ["pending", "committed", "failed", "abandoned"],
        "required_columns": [
          "principal_id", "request_id", "command", "intent_hash",
          "lease_generation", "lease_state", "acquired_at", "expires_at",
          "committed_at", "outcome_json", "failure_code"
        ]
      },
      "command_request_aliases": {
        "primary_key": ["principal_id", "command", "intent_hash", "request_id"],
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
      "provider_session.mark_process_absent": 120,
      "p083.rollback_execution": 120,
      "p083.set_enforcement_mode": 120,
      "min": 5,
      "max_configurable": 900
    },
    "intent_hash_composition_rule": {
      "canonical_serialization": "Intent payload is canonicalized using RFC 8785 JSON Canonicalization Scheme (JCS): UTF-8 bytes, object keys sorted lexicographically, no insignificant whitespace, numbers serialized per ECMA-404 minimal form, null preserved, strings escaped per JSON spec. Unknown fields are rejected by additionalProperties:false in the corresponding MCP/GraphQL schema before hashing.",
      "canonical_enum_normalization_rule": "Enum values (ApprovalResolution, P083EnforcementMode, P083RollbackTargetMode) are normalized to lowercase canonical form by the transport layer before hashing. JSON Schema and GraphQL enums use lowercase case names exclusively; mixed-case input is rejected with schema_invalid before hashing, never silently normalized.",
      "hash_function": "SHA-256 over the canonical UTF-8 bytes, lower-case hex output, no truncation.",
      "per_command_logical_fields": {
        "runs.cancel": ["run_id"],
        "runs.retry": ["run_id"],
        "stages.retry": ["stage_execution_id"],
        "approvals.resolve": ["approval_id", "resolution"],
        "side_effects.force_reconcile": ["side_effect_id", "decision_json_digest"],
        "provider_session.shutdown": ["provider_session_id"],
        "provider_session.mark_process_absent": ["provider_session_id", "cancellation_epoch"],
        "p083.rollback_execution": ["target_enforcement_mode"],
        "p083.set_enforcement_mode": ["target_mode"]
      },
      "exclusion_rule": "caller_request_id, request timestamps, principal display names, and diagnostic metadata are excluded from intent_hash inputs so that the same lifecycle intent issued with a fresh request id replays through command_request_aliases without creating a new lease. side_effects.force_reconcile includes a canonical decision_json_digest because the operator reconciliation decision is caller intent, not diagnostic metadata.",
      "fixtures": [
        "docs/evidence/083/idempotency/intent-hash-canonical-serialization-sorted-keys.fixture.json",
        "docs/evidence/083/idempotency/intent-hash-stable-across-payload-formatting-variations.fixture.json",
        "docs/evidence/083/idempotency/intent-hash-different-resolution-different-hash.fixture.json",
        "docs/evidence/083/idempotency/intent-hash-excludes-caller-request-id.fixture.json",
        "docs/evidence/083/idempotency/intent-hash-enum-normalization-uppercase-rejected.fixture.json"
      ]
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
      "docs/evidence/083/idempotency/approvals-resolve-enum-constraint.fixture.json",
      "docs/evidence/083/idempotency/side-effects-force-reconcile-expired-reacquire.fixture.json",
      "docs/evidence/083/idempotency/provider-session-shutdown-side-effect-receipt-settles.fixture.json",
      "docs/evidence/083/idempotency/p083-rollback-execution-committed-unack-replay.fixture.json",
      "docs/evidence/083/idempotency/p083-rollback-same-request-replayed.fixture.json",
      "docs/evidence/083/idempotency/p083-rollback-same-intent-new-request-aliased.fixture.json",
      "docs/evidence/083/idempotency/p083-rollback-request-intent-mismatch-denied.fixture.json",
      "docs/evidence/083/idempotency/p083-set-enforcement-mode-bounded-command-label.fixture.json",
      "docs/evidence/083/idempotency/p083-set-enforcement-mode-enum-constraint.fixture.json"
    ]
  },
  "manual_process_identity_check_ui_v1": {
    "schema_version": "manual_process_identity_check_ui_v1",
    "addresses": ["UI-P083-R69-NB-001"],
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
    "action_hierarchy": {
      "primary": {
        "action": "retry_identity_check",
        "placement": "leading prominent button inside the banner",
        "style": "filled, accent color, default keyboard activation",
        "effect": "Runs a read-only process identity probe and refreshes readback. It does not issue shutdown signals."
      },
      "secondary": {
        "action": "mark_process_absent",
        "placement": "trailing the primary button",
        "style": "outlined, destructive role, requires confirmation dialog",
        "effect": "Requires operator confirmation and CallerRequestId; if backend confirms absence, moves process_fate to absent_verified and resumes settlement."
      },
      "tertiary": {
        "action": "copy_diagnostic",
        "placement": "icon button in the banner footer",
        "style": "borderless, secondary text color",
        "effect": "Copies provider_session_id, cancellation_epoch, process_fate, last_seen_pid, process_start_identity hash, and latest receipt id with secrets redacted; shows transient 'Copied' confirmation for 1500 ms with reduceMotion-aware fade."
      },
      "overflow": {
        "action": "open_provider_session_evidence",
        "placement": "in the banner '...' overflow menu",
        "style": "menu item with disclosure indicator",
        "effect": "Opens the evidence panel anchored to the focused lifecycle window."
      }
    },
    "feedback_states": {
      "retry_identity_check": {
        "idle": "Button label 'Retry Identity Check', no progress indicator.",
        "loading": "Button label 'Checking identity...', inline determinate progress badge inside the button; the rest of the banner remains interactive and VoiceOver focus is not stolen.",
        "success": "Banner clears within one readback cycle; a brief 'Identity refreshed' status text appears in the surface chrome.",
        "error": "Inline error text under the button with the typed denial reason; the button remains enabled for retry."
      },
      "mark_process_absent": {
        "idle": "Button label 'Mark Process Absent'.",
        "loading": "Confirmation dialog disables Confirm and shows 'Confirming...' label.",
        "success": "Banner clears after backend confirms absent_verified.",
        "error": "Inline error in the confirmation dialog with the typed denial reason; dialog stays open."
      },
      "copy_diagnostic": {
        "idle": "Icon button with 'Copy Diagnostic' accessibility label.",
        "loading": "Not applicable; copy completes synchronously.",
        "success": "Transient 'Copied' badge appears next to the button for 1500 ms; VoiceOver announces 'Diagnostic copied'.",
        "error": "Inline error tooltip if pasteboard write fails; accessibilityHelp explains the failure."
      },
      "open_provider_session_evidence": {
        "idle": "Menu item with 'Open Provider Session Evidence' label.",
        "loading": "Not applicable; navigation is synchronous.",
        "success": "Focus moves to the evidence panel anchored to the focused lifecycle window.",
        "error": "Inline error toast if evidence cannot be loaded; banner remains visible."
      }
    },
    "duplicate_banner_rule": "If multiple provider sessions are held with identity_ambiguous, the run/stage detail surface collapses banners into a single grouped banner with a session picker; VoiceOver announces the picker only once, and the action hierarchy applies to the focused session.",
    "resolution_path": "The banner clears only after backend readback moves intent_state away from held or process_fate away from identity_ambiguous. UI state alone cannot clear the hold.",
    "no_spinner_rule": "Held identity_ambiguous rows show no automatic retry spinner and no countdown. Retry Identity Check is an explicit operator action; its loading state is a determinate inline progress badge, not a focus-stealing spinner.",
    "accessibility": "VoiceOver announces title, provider display name, reason, and focused action. Disabled toolbar/menu commands remain visible where native convention allows and expose the denial reason through accessibilityHelp or adjacent status text without stealing focus.",
    "fixtures": [
      "docs/evidence/083/ui/manual-process-identity-check-banner.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-no-auto-spinner.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-voiceover.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-resolution-actions.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-action-hierarchy.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-feedback-states.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-copy-confirmation.fixture.json",
      "docs/evidence/083/ui/manual-process-identity-check-duplicate-rollup.fixture.json"
    ]
  },
  "post_cancel_late_output_contract_v1": {
    "schema_version": "post_cancel_late_output_contract_v1",
    "addresses": [],
    "authority_table": "cancel_late_output_overflow",
    "unique_latch_key": [
      "scope", "normalized_run_id", "normalized_provider_session_id",
      "cancellation_epoch", "overflow_kind"
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
      "scope", "normalized_run_id", "normalized_provider_session_id",
      "cancellation_epoch", "overflow_kind", "dropped_message_count",
      "dropped_byte_count", "quarantine_uri", "reservation_release_state",
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
    "addresses": ["MACOS-P083-R69-NB-002"],
    "focused_window_rule": "Toolbar, menu, and keyboard commands resolve through the focused lifecycle_window_id. If no lifecycle window is key, lifecycle commands remain disabled-but-visible where native macOS convention allows and do not perform side effects.",
    "disabled_reason_rule": "Unavailable commands expose denial reason through accessibilityHelp, toolbar help, or adjacent status text without moving focus. Disabled controls are never hidden solely because the backend action is unavailable.",
    "commands_covered": [
      "Cancel Run", "Retry Run", "Retry Stage", "Resolve Approval",
      "Shutdown Provider Session", "Export Text", "Copy Diagnostic", "Retry Identity Check"
    ],
    "menu_bar_structure": {
      "root_menu_title": "Run",
      "placement_rule": "The Commands menu group lives under a top-level 'Run' menu between 'View' and 'Window'. It contains a 'Lifecycle' submenu (Cancel Run, Retry Run, Retry Stage, Resolve Approval, Shutdown Provider Session) and a 'Recovery' submenu (Retry Identity Check, Copy Diagnostic, Export Text). Order inside each submenu is fixed and lint-checked.",
      "menu_items": [
        {"menu_path": "Run > Lifecycle > Cancel Run", "command": "Cancel Run", "key_equivalent": "Cmd+." , "focused_value_key": "runIdSelection"},
        {"menu_path": "Run > Lifecycle > Retry Run", "command": "Retry Run", "key_equivalent": "Cmd+R", "focused_value_key": "runIdSelection"},
        {"menu_path": "Run > Lifecycle > Retry Stage", "command": "Retry Stage", "key_equivalent": "Shift+Cmd+R", "focused_value_key": "stageExecutionIdSelection"},
        {"menu_path": "Run > Lifecycle > Resolve Approval", "command": "Resolve Approval", "key_equivalent": "Cmd+Return", "focused_value_key": "approvalIdSelection"},
        {"menu_path": "Run > Lifecycle > Shutdown Provider Session", "command": "Shutdown Provider Session", "key_equivalent": "Shift+Cmd+K", "focused_value_key": "providerSessionIdSelection"},
        {"menu_path": "Run > Recovery > Retry Identity Check", "command": "Retry Identity Check", "key_equivalent": "Cmd+I", "focused_value_key": "providerSessionIdSelection"},
        {"menu_path": "Run > Recovery > Copy Diagnostic", "command": "Copy Diagnostic", "key_equivalent": "Shift+Cmd+C", "focused_value_key": "providerSessionIdSelection"},
        {"menu_path": "Run > Recovery > Export Text", "command": "Export Text", "key_equivalent": "Shift+Cmd+E", "focused_value_key": "runIdSelection"}
      ]
    },
    "menu_toolbar_parity_rule": "Every command in commands_covered exposes byte-equal accessibilityLabel, accessibilityHelp, and enabled-state between its menu item and its toolbar button. @FocusedValue keys named above drive both the menu and the toolbar; a lint fixture asserts byte-equal parity strings and enabled-state per focused value.",
    "voiceover_dedup_rule": "When the same command exists in both menu and toolbar, VoiceOver labels remain identical but role identifiers differ (menuItem vs button); duplicate announcements are avoided by ensuring only the focused command surface participates in VoiceOver focus order at any time.",
    "fixtures": [
      "docs/evidence/083/macos/focused-lifecycle-window-command-routing.fixture.json",
      "docs/evidence/083/macos/no-key-lifecycle-window-disabled-visible.fixture.json",
      "docs/evidence/083/macos/voiceover-disabled-command-reason.fixture.json",
      "docs/evidence/083/macos/toolbar-menu-keyboard-denial-parity.fixture.json",
      "docs/evidence/083/macos/run-menu-structure-byte-stable.fixture.json",
      "docs/evidence/083/macos/menu-toolbar-accessibility-parity.fixture.json",
      "docs/evidence/083/macos/focused-value-keys-wire-menu-and-toolbar.fixture.json"
    ]
  },
  "implementation_hardening_requirements_v1": {
    "schema_version": "implementation_hardening_requirements_v1",
    "closure_policy": "Every item in requirements is mandatory implementation scope. The code_writer, implementation reviewer, closeout audit, and release/readiness gates must treat these as must-close requirements before P083 can be marked implementation-complete, closeout-ready, or release-ready. They may not be marked non-blocking, deferred, or handoff unless a separate approved successor proposal explicitly owns the exact item and the operator approves that scope reduction.",
    "nonblocking_override_policy": "blocking=false is invalid for these items while verification_green=false or while any required code, schema, fixture, UI, metric, migration, or readback proof is missing.",
    "scope_rule": "Items previously promoted to active contracts in R69/R70 (GraphQL SDL coverage, MCP tool inventory, SwiftData Swift Concurrency isolation, intent_hash canonicalization, durable monotonic clock baseline correlation, macOS menu placement, banner action hierarchy) are not re-listed here. Items below are pure implementation hardening that active contracts do not yet exhaustively specify.",
    "requirements": [
      {
        "id": "P083-HARDEN-003",
        "title": "artifact_lineage.report_kind backfill posture",
        "required_outcome": "Either implement an additive backfill for pre-existing active report rows or provide executable evidence that no such rows exist before enforcing bounded report_kind values."
      },
      {
        "id": "P083-HARDEN-004",
        "title": "Schema-version evolution policy",
        "required_outcome": "Define append-only schema_version semantics, same-version additive-safe field policy, version bump rules, prior-version readability, and unknown-schema diagnostic behavior."
      },
      {
        "id": "P083-HARDEN-007",
        "title": "Failed-terminal retry policy per lifecycle command",
        "required_outcome": "Add a centralized per-command failed_terminal_retry_policy table and fixtures proving when a new same-intent request may or may not acquire a new lease."
      },
      {
        "id": "P083-HARDEN-008",
        "title": "Atomic late-output counter increments",
        "required_outcome": "Specify and test atomic counter increment and cap enforcement for concurrent late-output writers, including overflow latch behavior."
      },
      {
        "id": "P083-HARDEN-009",
        "title": "External side-effect composition for idempotent commands",
        "required_outcome": "For every idempotent lifecycle command with external effects, name planned rows, receipt rows, and crash-between-commit-and-external-action fixtures."
      },
      {
        "id": "P083-HARDEN-011",
        "title": "Minimum command lease TTL policy",
        "required_outcome": "Raise the global minimum lease TTL or define per-command recommended_min_ttl_seconds with rollout lint warnings below recommendation, especially for provider_session.shutdown."
      }
    ]
  }
}
