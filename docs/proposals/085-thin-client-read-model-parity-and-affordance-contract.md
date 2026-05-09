{
  "affordance_contract": {
    "compatibility_policy": {
      "additive_graphql_fields": "Allowed only when optional or nullable for old clients and covered by SDL/introspection proof plus Swift decoding tests.",
      "enum_value_additions": "Allowed only when Swift has unknown-case handling that fails closed to a diagnostic/unknown state and never maps to optimistic actionability or generic unavailable.",
      "fallback_copy_changes": "Allowed when disabled reason codes remain stable. Copy-only changes must update presenter snapshot/render tests and accessibility help text tests.",
      "persisted_projection_state": "If implementation needs stored fields for payload deadlines, stall reasons, or typed action availability, the rollout contract migrations section must be revised before implementation freeze.",
      "removed_or_retyped_fields": "Breaking change. Requires explicit proposal revision, gate update, and migration-contract update if persisted projection shape changes.",
      "renamed_affordance_ids": "Breaking change. Must retain old id as deprecated alias for one contract version or update all tests, accessibility identifiers, P036 citations, and negative fixtures before freeze."
    },
    "contract_schema": "thin_client_affordance_contract_v1",
    "reference_document": "docs/reference/thin-client-read-model-affordance-contract.md",
    "required_graphql_symbols_for_schema_proof": [
      "payloadAvailabilityState",
      "payloadUnavailableReasonCode",
      "freshnessState",
      "disabledReasonCode",
      "writePathState",
      "diagnosticId",
      "serverDebugDetail",
      "availableActions or typed replacement",
      "approveApproval",
      "rejectApproval",
      "artifact list/detail payload readback fields named by contract rows",
      "report metadata/payload readback fields named by contract rows",
      "stalenessDeadlineAt or the equivalent server-owned deadline field if introduced",
      "stalledReasonCode or the equivalent typed stalled diagnostic field if introduced"
    ],
    "required_rows": [
      {
        "affordance_id": "artifact.preview.listLabel",
        "mutation_availability": "none",
        "required_behavior": "List rows distinguish available, metadata_only, payload_deferred, generating, and unavailable. payload_deferred means previewable/deferred, not unavailable. current backend-emitted payload_deferred states must include an explicit no-deadline justification; generating must not be emitted unless the server also emits a deadline or typed stalled/timed-out diagnostic.",
        "source_fields": [
          "artifacts(runId:)",
          "payloadAvailabilityState",
          "payloadUnavailableReasonCode",
          "freshnessState",
          "diagnosticId",
          "serverDebugDetail",
          "stalenessDeadlineAt or equivalent when introduced"
        ],
        "supported_interactions": [
          "list_row",
          "context_menu",
          "keyboard_default_open",
          "quick_look_when_detail_authorized"
        ]
      },
      {
        "affordance_id": "artifact.preview.detail",
        "cancellation_policy": "cancel_on_surface_dismiss, cancel_on_run_switch, cancel_on_selection_change, clear_in_flight_on_cancel_or_deadline",
        "mutation_availability": "none",
        "required_behavior": "Detail preview may render payload text only from GraphQL/server-authorized readback and must merge richer detail state over deferred list state for the selected artifact only. Stale async responses must not overwrite a newer selection.",
        "source_fields": [
          "artifact(id:) or current artifact detail entrypoint",
          "payloadText or server-owned payload field when present",
          "payloadAvailabilityState",
          "payloadUnavailableReasonCode",
          "disabledReasonCode",
          "diagnosticId",
          "serverDebugDetail"
        ],
        "supported_interactions": [
          "detail_pane",
          "quick_look",
          "context_menu"
        ]
      },
      {
        "affordance_id": "report.payload.metadata",
        "mutation_availability": "none",
        "required_behavior": "Report rows show metadata-only status unless a dedicated server-owned payload query exists. Metadata-only, deferred, generating, and unavailable remain distinct. Current backend-emitted deferred metadata states include explicit no-deadline justification; generating remains schema-reserved until server-owned deadline or stalled diagnostics exist.",
        "source_fields": [
          "report metadata through artifacts(runId:)",
          "payloadAvailabilityState",
          "payloadUnavailableReasonCode",
          "diagnosticId",
          "serverDebugDetail"
        ]
      },
      {
        "affordance_id": "freshness.badge.run",
        "required_behavior": "Freshness communicates recency only and preserves existing states live, refreshing, refreshing_disconnected, projection_lag, stale, unavailable, and unauthorized. It cannot drive payload availability, approval actionability, authorization, or mutation availability by itself.",
        "source_fields": [
          "freshnessState",
          "projectionPresent",
          "projectionUpdatedAt",
          "projectionLag"
        ]
      },
      {
        "affordance_id": "freshness.badge.stage",
        "required_behavior": "Same freshness semantics as run badge, scoped to stage readback and projection-owned stage fields.",
        "source_fields": [
          "freshnessState",
          "projectionPresent",
          "projectionUpdatedAt",
          "projectionLag"
        ]
      },
      {
        "affordance_id": "freshness.badge.approval",
        "required_behavior": "Projection lag is diagnostic context. Approval actionability still depends on durable approval state, caller policy, typed action availability, and conflict-aware mutation behavior.",
        "source_fields": [
          "freshnessState",
          "projectionUpdatedAt",
          "projectionLag",
          "availableActions or typed replacement"
        ]
      },
      {
        "affordance_id": "freshness.badge.artifact",
        "required_behavior": "Freshness never means payload presence. Payload availability is read from payload availability fields only.",
        "source_fields": [
          "freshnessState",
          "payloadAvailabilityState",
          "payloadUnavailableReasonCode"
        ]
      },
      {
        "affordance_id": "approval.resolve.approve",
        "mutation_availability": "approveApproval",
        "mutation_idempotency": "Duplicate, stale, conflicting, or non-pending submit returns success or already_resolved with current durable decision when the backend can journal the attempt. Transient transport/server failures stay GraphQL errors and are not silently retried by Swift; unknown future conflict codes fail closed.",
        "required_behavior": "Approve renders actionable only when durable approval state is pending/requested, caller policy allows approve, typed action availability includes approve, and approveApproval is authorized. Stale or duplicate submits must be idempotent or return typed already_resolved without clobbering durable state.",
        "source_fields": [
          "approvalInbox",
          "approval durable state",
          "caller policy readback",
          "availableActions or typed replacement",
          "disabledReasonCode",
          "writePathState",
          "diagnosticId",
          "serverDebugDetail"
        ],
        "stale_projection_policy": "Projection_lag may keep the control visible from last known actionable state, but mutation conflict handling is mandatory and presenter maps conflicts to the same disabled reason used by a fresh non-actionable read model."
      },
      {
        "affordance_id": "approval.resolve.reject",
        "mutation_availability": "rejectApproval",
        "required_behavior": "Reject follows the same durable state, caller policy, typed availability, mutation authorization, idempotency, stale-submit, and transient-error behavior as approve, using rejectApproval.",
        "source_fields": [
          "approvalInbox",
          "approval durable state",
          "caller policy readback",
          "availableActions or typed replacement",
          "disabledReasonCode",
          "writePathState",
          "diagnosticId",
          "serverDebugDetail"
        ]
      },
      {
        "affordance_id": "diagnostic.copy",
        "mutation_availability": "none",
        "required_behavior": "Copy affordances use server-provided diagnosticId and authorized serverDebugDetail only, respect redaction by principal class, and invalidate cached diagnostic state on auth-error transitions.",
        "source_fields": [
          "diagnosticId",
          "serverDebugDetail",
          "disabledReasonCode",
          "freshnessState"
        ],
        "supported_interactions": [
          "trailing_info_button",
          "context_menu",
          "detail_pane_copy"
        ]
      },
      {
        "affordance_id": "external.command.placeholder",
        "mutation_availability": "external_transport_only",
        "required_behavior": "Start, cancel, retry, reset, compact, clone, recovery, experiment, runtime-profile, and other command actions remain hidden or externally managed under ui-action-boundary.md. They do not become SwiftUI mutations.",
        "source_fields": [
          "writePathState",
          "disabledReasonCode",
          "diagnosticId"
        ]
      }
    ],
    "row_schema_columns": [
      "affordance_id",
      "surface",
      "graphql_entrypoint",
      "source_graphql_fields",
      "field_nullability_and_enum_domains",
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
      "proof_tests"
    ]
  },
  "architecture": {
    "backend_read_model_and_schema": [
      "P085 extends the existing P031/P043 read contract rather than replacing it. GraphQL remains the server-owned read plane, and MCP remains the non-approval command/control plane.",
      "GraphQL enums remain typed domains for freshness, disabled reason, write path state, payload availability, payload unavailable reason, diagnostics, typed action availability, and mutation conflict result codes.",
      "The p085 gate must assert SDL/schema or introspection snapshots for every field and enum domain named by contract rows, including approveApproval and rejectApproval mutations.",
      "Approval actionability is derived from durable approval state plus caller policy plus typed mutation availability. Backend tests must prove read-model action availability, disabledReasonCode, writePathState, mutation authorization, already_resolved, stale-projection submit, duplicate-submit, and transient failure surfaces agree.",
      "Artifact/report preview state is derived from artifact index/projection metadata and authorized payload readback. List preview budget limits are represented as payload_deferred, not unavailable.",
      "Payload-deferred no-deadline justifications and any future generating stalled/timed-out states are server-owned. Swift timers may cancel local requests, but cannot decide that a server state is stalled.",
      "Unauthorized reads return typed GraphQL auth errors or redacted fields according to P081. Swift never falls back to local storage, SwiftData, raw artifact scans, or filesystem report reads as truth."
    ],
    "documentation_and_gate": [
      "docs/reference/thin-client-read-model-affordance-contract.md becomes implemented-system truth after the p085 gate passes.",
      "docs/reference/test-gates.md and scripts/test-gate.sh must register both proposal-085 and p085 aliases to the same proof slice.",
      "The p085 gate composes cheap existing P031/P043/P081 proof where useful, adds P085-local backend schema/projection/mutation fixtures, and adds Swift presenter/render tests. UI smoke remains remote-only by repository policy and is not required for proposal-readiness."
    ],
    "swift_presentation_layer": [
      "Swift owns presentation mapping only. Candidate implementation points include P031ThinGraphQLReadBoundary.swift, RunsHomeView.swift, ArtifactContentRenderer.swift, approval inbox/presenter code, and a focused Proposal085 test suite.",
      "Implementation must centralize mapping in a canonical presenter such as P085AffordancePresenter or extensions near P031ThinGraphQLReadBoundary.",
      "Presenter outputs must be immutable, Equatable, and Sendable values consumed by SwiftUI: label text, enabled/disabled state, action id, disabled reason, mutation operation, diagnostic copy availability, supported interactions, and accessibility/help text.",
      "Swift decoding must preserve or safely classify unknown enum values for payload availability, disabled reason, freshness, write path, mutation availability, and mutation conflict result. Unknown values fail closed to diagnostic/unknown states and never become optimistic actionability.",
      "Presenter tests must cover artifact list/detail merge, stale task completion after selection changes, cancellation on dismiss/run switch, in-flight flag clearing, auth policy transitions, unknown enum values, approval conflict mapping, and tooltips/help text derived from disabled reasons."
    ]
  },
  "author": "Codex",
  "date": "2026-05-08",
  "depends_on": [
    "docs/reference/query-projections-and-client-consumption-contract.md",
    "docs/reference/ui-action-boundary.md",
    "Proposal 081 boundary matrix",
    "P072 approval mutation semantics"
  ],
  "disagreements_resolved": [
    "API review offered typed action enum or frozen availableActions. The proposal accepts both implementation paths but requires one to be chosen and proven before freeze; typed enum remains preferred.",
    "Reliability warned that freshness should not disable approval actionability by itself but stale projection must not enable unsafe clobbering. The proposal resolves this by keeping projection_lag diagnostic while requiring conflict-aware mutations and stable presenter mapping.",
    "The earlier draft treated migrations as not applicable unless a missing field is discovered during implementation. The revision tightens this: any persisted projection field need revises the migration section before freeze."
  ],
  "document_format": "proposal_json_v1",
  "feedback_disposition": [
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-001",
      "resolution": "Added thin_client_affordance_contract_v1 and compatibility policy for additive fields, enum additions, renamed affordance ids, fallback-copy changes, and breaking field changes.",
      "source": "api-contract"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-002",
      "resolution": "Required SDL/introspection proof for all named fields, enum domains, availableActions or typed replacement, approveApproval, and rejectApproval.",
      "source": "api-contract"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-003",
      "resolution": "Defined approval mutation idempotency/conflict behavior for duplicate, stale, already-resolved, conflicting, and transient cases, plus Swift mapping requirements.",
      "source": "reliability"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-004",
      "resolution": "Added server-owned staleness deadline/stuck-state policy for generating and payload_deferred states with backend fixture requirements.",
      "source": "reliability"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-005",
      "resolution": "The proposal now requires typed per-mutation availability or frozen availableActions values with schema and Swift unknown-value tests.",
      "source": "api-contract"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-006",
      "resolution": "Artifact/report rows now require exact list/detail fields and migration-contract revision before freeze if persisted projection state is needed.",
      "source": "api-contract"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-007",
      "resolution": "Swift mapping is centralized in immutable Equatable Sendable presenter DTOs consumed by SwiftUI.",
      "source": "apple-architect"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-008",
      "resolution": "Added stale async response, selection/detail merge, cancel-on-dismiss, run-switch, and in-flight clearing tests.",
      "source": "apple-architect,reliability"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-009",
      "resolution": "Unknown GraphQL enum values fail closed to diagnostic/unknown states and are covered by Swift tests.",
      "source": "apple-architect,api-contract"
    },
    {
      "decision": "accepted",
      "feedback_id": "P085-LIFT-010",
      "resolution": "Added visual hierarchy, supportedInteractions, context menu, keyboard, tooltip/help, accessibility, and Quick Look guidance.",
      "source": "ui,macos"
    }
  ],
  "gate": [
    "./scripts/test-gate.sh proposal-085",
    "./scripts/test-gate.sh p085"
  ],
  "goals": [
    "Create docs/reference/thin-client-read-model-affordance-contract.md as the canonical implemented-system contract for GraphQL-driven Swift affordances.",
    "Version the contract as thin_client_affordance_contract_v1 and define compatibility rules for additive fields, enum additions, renamed affordance ids, fallback-copy changes, and old Swift-client behavior.",
    "Define required rows for artifact preview, report payload metadata, approval actionability, freshness badges, diagnostic copy affordances, and externally managed command placeholders.",
    "Require every row to name source GraphQL fields, field nullability and enum domains, Swift-local presentation state, actionable state, disabled reason code, fallback text, mutation availability, mutation idempotency/conflict behavior, staleness deadline, cancellation policy, stale list/detail behavior, unauthorized behavior, supported interactions, and proof tests.",
    "Require p085 gate SDL or introspection assertions for every GraphQL field, enum domain, and mutation named by contract rows.",
    "Define approveApproval and rejectApproval stale-submit, duplicate-submit, already-resolved, state-conflict, transient-error, and presenter-mapping behavior.",
    "Add server-owned no-deadline justifications for current deferred artifact/report states and require deadline/stuck-state transitions before generating can be emitted.",
    "Centralize Swift affordance mapping in immutable Equatable Sendable DTOs so SwiftUI views consume tested affordance states instead of raw strings.",
    "Preserve P036 as a consumer of P085; restored visual, navigation, context-menu, keyboard, tooltip, and Quick Look affordances must cite P085 rows or stay deferred."
  ],
  "metrics": {
    "metric_definitions": [
      "p085_graphql_driven_swift_affordances_with_contract_rows_percent: percent of GraphQL-driven Swift affordances covered by a P085 row.",
      "p085_affordance_contract_gate_total{status,failure_reason}: gate health and failure classification.",
      "p085_affordance_schema_snapshot_total{status,symbol}: SDL/introspection coverage for fields, enum domains, and mutations named by rows.",
      "p085_affordance_drift_detected_total{surface,affordance_id,reason}: mismatches caught by backend or Swift tests.",
      "p085_approval_actionability_parity_total{status,caller_class}: parity between read actionability and mutation authorization.",
      "p085_approval_mutation_conflict_total{operation,result_code}: duplicate/stale submit handling for approveApproval and rejectApproval.",
      "p085_payload_state_mapping_total{status,payload_availability_state}: payload/list/detail mapping coverage.",
      "p085_payload_stalled_transition_total{surface,state,result_code}: server-owned no-deadline justification and future stalled/deadline transitions for any future generating states; current deferred states report no-deadline justification.",
      "p085_unknown_enum_fail_closed_total{enum_domain,surface}: unknown enum cases rendered as diagnostic/unknown instead of optimistic actionability."
    ],
    "success_criteria": [
      "Every initial required row has backend and Swift proof.",
      "Both proposal-085 and p085 gates are registered and run the same proof slice.",
      "SDL/introspection proof covers all fields, enum domains, and mutations named by P085 rows.",
      "Approval duplicate/stale-submit and conflict behavior is typed and mapped by Swift to stable disabled/fallback output.",
      "Backend-emitted payload_deferred states have server-owned no-deadline justifications; generating remains gated on deadline/stuck-state diagnostics.",
      "Future P036 affordance work can cite P085 rows instead of rediscovering boundary rules."
    ]
  },
  "non_goals": [
    "Do not add broad GraphQL mutations or restore local Swift workflow mutation fallback.",
    "Do not make Swift local state authoritative for run, stage, approval, artifact, report, recovery, queue, or projection truth.",
    "Do not force report or artifact payloads into every list query. Deferred payloads remain valid when explicitly modeled.",
    "Do not implement start, cancel, retry, reset, compact, clone, recovery, experiment, or runtime-profile command UI.",
    "Do not rewrite P031, P036, P072, or P081. P085 adds the missing affordance parity layer over their accepted boundaries.",
    "Do not make Swift timers authoritative for stalled server states. Client request deadlines are presentation safety only; stuck-state transitions are server-owned."
  ],
  "open_questions": [
    {
      "question": "Does implementation introduce new persisted projection fields for stalenessDeadlineAt or stalled reason codes?",
      "resolution_path": "If yes, revise rollout_contract_v1.migrations before freeze and add DB migration proof. If no, document the existing server-owned derivation and prove it with backend fixtures."
    },
    {
      "question": "Will approval action availability use a new typed GraphQL enum or freeze existing availableActions values?",
      "resolution_path": "Either is acceptable for P085 if the selected vocabulary is SDL-proven, versioned, and covered by Swift unknown-value tests. A typed enum is preferred."
    },
    {
      "question": "What exact artifact detail entrypoint carries authorized payloadText or equivalent?",
      "resolution_path": "The implementation must name the list and detail entrypoints in the contract row and add schema/projection proof before Swift depends on the field."
    }
  ],
  "problem": {
    "risks_observed": [
      "Artifact and report rows can collapse partial list payloads into permanent unavailable labels even when detail readback can render richer state.",
      "Freshness states such as live, stale, projection_lag, and refreshing_disconnected describe recency and connectivity, but can be misused as payload availability, approval actionability, authorization, or mutation availability.",
      "Approval buttons can appear actionable from local row selection or display text while durable approval state, caller policy, mutation authorization, or stale projection state disagree.",
      "Deferred and generating payload states can become operator stuck states if the server does not own a deadline/no-deadline justification and diagnostic transition.",
      "P036 visual/navigation restoration needs stable affordance semantics rather than per-view reinterpretation of GraphQL fields."
    ],
    "summary": "The governed macOS app is now a thin GraphQL read-side client plus the two approval mutations allowed by ui-action-boundary.md. That fixed local workflow truth drift, but Swift affordances can still make inconsistent promises when GraphQL fields, local presentation state, fallback labels, authorization, mutation availability, and stale projections are handled separately.",
    "why_now": "P085 is the narrow missing parity layer over the existing P031/P043/P081 boundary. Without it, future Swift UI restoration can remain thin-client in architecture while still drifting in visible behavior."
  },
  "proposal_revision_id": "p085-r2-2026-05-08",
  "related": [
    "P036 UI restoration",
    "P068/P081 boundary work",
    "P072 approval mutations",
    "Chainworks Forge/Support/P031ThinGraphQLReadBoundary.swift",
    "Chainworks Forge/Views/RunsHomeView.swift",
    "control-plane/crates/graphql-server/src/schema.rs"
  ],
  "review_context": {
    "aggregate_score": 8.18,
    "blocker_count_before_revision": 4,
    "decision_before_revision": "revise",
    "revision_goal": "Resolve the four blocking review issues and incorporate high-value non-blocking Apple, UI, and macOS guidance without widening P085 beyond thin-client read-model parity and approval-only governed mutations.",
    "selected_reviewers": [
      "apple-architect",
      "api-contract",
      "reliability",
      "ui",
      "macos"
    ]
  },
  "risks_and_mitigations": [
    {
      "mitigation": "The p085 gate fails on missing rows, missing tests, missing schema symbols, and known bad fixtures.",
      "risk": "Contract becomes a doc-only checklist."
    },
    {
      "mitigation": "Swift decodes typed GraphQL enum cases with unknown-case tests and immutable presenter DTOs.",
      "risk": "Swift presenters overfit current enum strings."
    },
    {
      "mitigation": "P085 treats P081/P072 as authority and tests parity between read actionability, caller policy, mutation authorization, and conflict results.",
      "risk": "Approval actionability duplicates P081 incorrectly."
    },
    {
      "mitigation": "approveApproval/rejectApproval must be idempotent or return typed already_resolved, and Swift maps those results to stable disabled/fallback output.",
      "risk": "Projection lag causes duplicate or stale approval submits."
    },
    {
      "mitigation": "Contract explicitly allows payload_deferred and metadata_only; detail preview stays server-authorized.",
      "risk": "Payload preview work expands into bulk payload loading."
    },
    {
      "mitigation": "Relevant rows require server-owned no-deadline justification for current deferred states, and staleness deadlines or explicit stalled diagnostics before generating is emitted.",
      "risk": "Generating or deferred payloads become indefinite operator stuck states."
    },
    {
      "mitigation": "P036-dependent controls must cite a P085 row or remain hidden/deferred.",
      "risk": "P036 bypasses the contract to restore a visual affordance quickly."
    },
    {
      "mitigation": "Keep p085 focused on schema/projection fixtures and Swift presenter/render tests; compose only cheap existing gates unless blast radius requires more.",
      "risk": "Rollout proof adds too much suite time."
    }
  ],
  "rollout_contract_v1": {
    "applicability": "required",
    "commands": {
      "allowlist": [
        "./scripts/test-gate.sh proposal-085",
        "./scripts/test-gate.sh p085"
      ],
      "commentary": "Gate commands are declarative expectations; the linter does not execute them."
    },
    "decision_vocabulary": [
      "pass",
      "fail",
      "waived",
      "not_applicable",
      "timeout"
    ],
    "gate_aliases": [
      "proposal-085",
      "p085"
    ],
    "hold_conditions": [
      "P085 gate is missing or not registered under both proposal-085 and p085 aliases",
      "Affordance contract row missing for a GraphQL field that drives Swift UI state",
      "SDL/introspection proof missing for a field, enum domain, or approval mutation named by a contract row",
      "Approval actionability read model and mutation authorization disagree",
      "approveApproval or rejectApproval lacks idempotent or typed conflict behavior for duplicate/stale submits",
      "Swift presenter maps mutation conflict to optimistic actionability or unstable fallback copy",
      "Swift presenter maps payload_deferred or metadata_only to an unavailable/no-preview state",
      "Generated payload state lacks server-owned no-deadline/deadline/stalled diagnostic policy",
      "Unknown GraphQL enum value crashes, maps to optimistic actionability, or collapses into generic unavailable",
      "Unauthorized or redacted readback falls back to local workflow truth or raw filesystem truth"
    ],
    "hold_conditions_detail": [
      "approval_conflict_missing: Duplicate and stale approval submits must be idempotent or return typed already_resolved results without clobbering durable state.",
      "approval_parity_mismatch: Approve/reject buttons may not render unless durable approval state, caller policy, typed action availability, and GraphQL mutation authorization agree.",
      "missing_affordance_row: Every GraphQL-driven Swift affordance must have source fields, local presentation state, disabled/fallback copy, mutation availability, mutation idempotency when applicable, stale behavior, unauthorized behavior, supported interactions, and tests.",
      "missing_gate_alias: Both proposal-085 and p085 must run the same proof slice before implementation can freeze.",
      "missing_schema_proof: The p085 gate must snapshot or introspect every field and enum domain consumed by the row matrix so Swift presenters do not depend on undocumented vocabulary.",
      "payload_deadline_missing: Backend-emitted deferred states need no-deadline justification, and any generated states need deadline or stuck diagnostic semantics so the operator UI cannot wait indefinitely with no escalation path.",
      "payload_state_mismatch: Deferred list payloads and metadata-only report states must be represented honestly and not collapsed into generic unavailable copy.",
      "unauthorized_fallback_violation: Unauthorized reads must deny or redact through the server contract; Swift cannot recover by reading local storage or artifacts as truth.",
      "unknown_enum_unsafe: Unknown server enum values must fail closed to diagnostic/unknown presentation, not crash or enable actions."
    ],
    "metrics": {
      "adoption_metric": "p085_graphql_driven_swift_affordances_with_contract_rows_percent",
      "operational_metrics": [
        "p085_affordance_contract_gate_total{status,failure_reason}",
        "p085_affordance_schema_snapshot_total{status,symbol}",
        "p085_affordance_drift_detected_total{surface,affordance_id,reason}",
        "p085_approval_actionability_parity_total{status,caller_class}",
        "p085_approval_mutation_conflict_total{operation,result_code}",
        "p085_payload_state_mapping_total{status,payload_availability_state}",
        "p085_payload_stalled_transition_total{surface,state,result_code}",
        "p085_unknown_enum_fail_closed_total{enum_domain,surface}"
      ]
    },
    "migrations": {
      "justification": "P085 is expected to define documentation, GraphQL/Swift affordance contracts, tests, and gate registration. If implementation later emits generating states or requires persisted projection fields for deadline/stalled-state or typed action availability, this migrations section must be revised before implementation freeze and cannot be treated as a discovery-time exception.",
      "not_applicable": true
    },
    "negative_fixtures": {
      "approval_actionability_mismatch": "docs/evidence/rollout-contract/negative/p085-approval-actionability-mismatch.json",
      "approval_stale_double_submit_conflict": "docs/evidence/rollout-contract/negative/p085-approval-stale-double-submit-conflict.json",
      "missing_affordance_row": "docs/evidence/rollout-contract/negative/p085-missing-affordance-row.json",
      "missing_schema_symbol": "docs/evidence/rollout-contract/negative/p085-missing-schema-symbol.json",
      "payload_deferred_marked_unavailable": "docs/evidence/rollout-contract/negative/p085-payload-deferred-marked-unavailable.json",
      "payload_deferred_no_deadline": "docs/evidence/rollout-contract/negative/p085-payload-deferred-no-deadline.json",
      "unknown_enum_optimistic_action": "docs/evidence/rollout-contract/negative/p085-unknown-enum-optimistic-action.json",
      "unsafe_local_truth_fallback": "docs/evidence/rollout-contract/negative/p085-unsafe-local-truth-fallback.json"
    },
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
      "rollout_contract_next_steps"
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
      "rollout_contract_next_steps"
    ],
    "readback_fixture": "docs/evidence/rollout-contract/operator-readback/p085-full-surface.fixture.json",
    "readback_lanes": [
      "run_report",
      "mcp",
      "release_receipt",
      "graphql"
    ],
    "rollback_disposition": {
      "data_loss_risk": "none",
      "mode": "disable_p085_affordance_changes_or_revert_to_existing_read_only_rendering",
      "steps": [
        "Disable newly introduced P085 presenter mappings behind the implementation feature flag or revert the narrow UI mapping commit.",
        "Keep GraphQL read fields server-owned and leave existing P031/P043 read contract behavior intact.",
        "Hold P036 affordances that depend on failed P085 rows until parity tests pass.",
        "Attach p085 gate failure evidence and repair the contract row, backend fixture, schema proof, or Swift presenter test before re-enable."
      ]
    },
    "schema_version": "rollout_contract_v1"
  },
  "rollout_plan": [
    {
      "step": 1,
      "task": "Add docs/reference/thin-client-read-model-affordance-contract.md with thin_client_affordance_contract_v1, compatibility policy, row schema, initial required rows, and reviewer-resolved reliability columns."
    },
    {
      "step": 2,
      "task": "Add SDL/introspection proof for every GraphQL field, enum domain, and approval mutation named by P085 rows."
    },
    {
      "step": 3,
      "task": "Add backend fixtures for artifact/report payload states, server-owned no-deadline justification and future stalled/deadline transitions, freshness/action separation, approval actionability, duplicate-submit, stale-submit, already_resolved and transient error GraphQL surfaces, and unauthorized/redacted behavior."
    },
    {
      "step": 4,
      "task": "Add Swift canonical presenter DTOs and tests for list/detail merge, stale async responses, cancellation policy, unknown enum fail-closed behavior, auth transition invalidation, approval conflict mapping, diagnostic copy, context-menu/keyboard/Quick Look affordance state, and help/accessibility labels."
    },
    {
      "step": 5,
      "task": "Register proposal-085 and p085 aliases in scripts/test-gate.sh and docs/reference/test-gates.md with a bounded proof slice and documented timeout budget."
    },
    {
      "step": 6,
      "task": "Update P036-facing notes so restored navigation, visual, context-menu, keyboard, tooltip, and Quick Look affordances cite P085 rows or stay hidden/deferred."
    },
    {
      "step": 7,
      "task": "Run ./scripts/test-gate.sh proposal-085 as the proposal proof gate, with broader gates chosen by implementation blast radius."
    }
  ],
  "run_id": "daa93eeb-9bdb-4e24-aed4-3eef211a99a2",
  "scope": "Add one executable, versioned affordance contract for every SwiftUI affordance driven by GraphQL read models, including source fields, local presentation state, actionability, disabled and fallback copy, mutation availability, mutation conflict behavior, stale list/detail behavior, unauthorized behavior, staleness deadlines, cancellation policy, supported interactions, and proof tests.",
  "source_review_pass_id": "d8da0da6-cc18-452f-b945-7032189fe459",
  "status": "implemented",
  "title": "Proposal 085: Thin-Client Read-Model Parity and Affordance Contract",
  "ux_ui_notes": {
    "principles": [
      "Operator copy must be precise rather than optimistic. A list row with deferred detail says Open to preview or equivalent, not Unavailable.",
      "Freshness badges remain visually separate from payload status and actionability. A live row can have no preview, and a projection_lag row can still show last-known metadata while disabling only fields that depend on missing projection-owned decision facts.",
      "The visual hierarchy for dense rows is primary object identity first, primary action or payload state second, freshness and diagnostic badges third. Diagnostic copy must not crowd the primary row label.",
      "payload_deferred uses an interactive deferred/skeleton treatment; metadata_only uses a neutral metadata label; unavailable uses terminal disabled styling with stable fallback copy.",
      "disabledReasonCode maps to native SwiftUI help/tooltips for buttons and interactive elements. Diagnostic copy is exposed through a trailing info control, context menu item, or detail pane, not through noisy inline text."
    ],
    "projection_lag_behavior": "When approval freshness is projection_lag, Swift may keep the button visible if the last read model says it is actionable, but the mutation must be conflict-aware and the presenter must surface lag as diagnostic context rather than treating freshness alone as durable approval truth.",
    "supported_interactions": [
      "List rows, detail panes, context menus, keyboard shortcuts, tooltips/help, accessibility labels, and Quick Look must all consume the same P085 affordance row for a given action.",
      "Artifact preview detail must support native Spacebar Quick Look only when the P085 row says the detail readback is authorized and previewable.",
      "Approval approve/reject may define focused keyboard shortcuts such as Cmd-Return and Cmd-Delete only when the corresponding presenter action is available; disabled shortcuts expose the same disabled reason as the button.",
      "Accessibility identifiers derive from affordance ids so P036 and render tests can cite stable rows."
    ]
  }
}
