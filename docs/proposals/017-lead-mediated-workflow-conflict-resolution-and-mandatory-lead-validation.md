{
  "proposal_revision_id": "p017-r7-review-pass-5-closure",
  "source_review_pass_id": "review-pass-5",
  "title": "Proposal 017: Lead-Mediated Workflow Conflict Resolution and Mandatory Lead Validation",
  "status": "Revised for implementation-readiness review",
  "run_id": "4d3faec6-b668-47b9-9474-9732c94a94f4",
  "source_proposal": "docs/proposals/017-lead-mediated-workflow-conflict-resolution-and-mandatory-lead-validation.md",
  "date": "2026-04-22",
  "canonical_gate": "./scripts/test-gate.sh proposal-017",
  "post_ui_db_cutover_amendment": {
    "date": "2026-04-23",
    "decision": "P017 implementation scope is control-plane-only after the UI DB cutover.",
    "canonical_implementation_target": "Rust control-plane: SQLite persistence, domain contracts, workflow compiler/engine behavior, MCP report/debug readback, and GraphQL read projections.",
    "removed_implementation_targets": [
      "SwiftData UI storage",
      "Swift WorkflowOrchestrator transition authority",
      "Swift RunReportBuilder workflow_conflict payload generation",
      "Swift JSON report parity as an acceptance surface",
      "SwiftData bridge migration as a future P017 requirement",
      "UI conflict details, recovery action, timeline, or mediation surfaces"
    ],
    "ui_boundary": "The macOS UI must not read or write any app database for P017 truth. UI readback is a future thin-client concern over GraphQL projections only. MCP remains for operator/agent/debug control and must not be used by UI.",
    "audit_instruction": "Future P017 audits must not treat missing SwiftData, Swift UI, or legacy Swift runtime implementation as a P017 blocker. They should audit whether control-plane truth and GraphQL/MCP readback satisfy the control-plane contract, and separately report any remaining legacy Swift code as deletion/quarantine work outside P017 conformance.",
    "preserved_green_slice": "The already-merged Phase A control-plane implementation remains valid and should be preserved unless replaced by an equivalent or stricter control-plane implementation."
  },
  "review_readiness": {
    "target": "aggregate score above 9",
    "previous_review": {
      "review_pass_id": "review-pass-5",
      "aggregate_score_from_individual_reviews": 9.225,
      "blocker_count": 0,
      "reviewer_scores": {
        "product_owner": 9.4,
        "ux_designer": 9.6,
        "ui_designer": 8.5,
        "architect": 9.4
      },
      "remaining_non_blocking_issues": [
        "ARCH-017-R5-001",
        "PO5-001",
        "PO5-002",
        "PO5-003",
        "UX-017-R6-001",
        "UI-017-001",
        "UI-017-002"
      ]
    },
    "why_ready_after_revision": [
      "The sole review-pass-3 blocker is resolved by a concrete Phase B mediation-owned execution migration contract: Rust AgentExecution gains a general owner_kind/owner_id model, stage_execution_id becomes nullable only for non-stage owners, existing stage-owned execution read paths remain unchanged, and owner-aware run/report/cancellation paths include mediation attempts.",
      "Repository, cancellation, MCP reports.get, GraphQL, runtime-facts, artifact, transcript, and cost behavior for mediation-owned executions are now specified instead of left to implementer interpretation.",
      "Aggregate artifact field authority is deterministic. proposal_review_summary pass/blocker fields are transition-authoritative, next_action/next_stage are advisory-only, and the D4F404B7 replay has a single expected outcome when the refinement transition exists: graph-authoritative refinement plus WorkflowAdvisoryRejectionRecord for the invalid advisory next_stage.",
      "CandidateTransitionEvaluation now explicitly replaces the current Rust unknown-artifact exists() fallback: unknown catalog artifact references never evaluate true and instead classify as invalid_expression or missing_input with parity fixtures.",
      "The workflow_conflict northbound contract now separates canonical semantics from per-surface shapes: MCP reports.get uses snake_case, GraphQL uses typed fields and GraphQL enum casing, and parity fixtures must cover all translations.",
      "The bundled workflow scan now has an action threshold: non-zero simultaneous matches in bundled workflows block Phase A merge unless affected transitions are re-authored or an explicit operator-approved known-issues migration record exists.",
      "Phase C rollout now defines release cycle, inventory artifact, warning/waiver decision record, and migration warning evidence.",
      "The P053 implementation-entry incident class is now covered: approved_proposal freeze and implementation-start handoff truth are engine-owned, code_writer is not reported as started before claim/start, and lead/orchestrator timeouts block with precise blocked-before-code readback.",
      "Operator-feedback metrics now include recovery_action_chosen_total, workflow_conflict_time_to_resolution_seconds, and conflict_reason_to_action_outcome_total.",
      "Review-pass-4 cursor/resume feedback is resolved by requiring every selected transition, blocking conflict, lead mediation result, and terminal-unverifiable state to settle through the existing transition cursor boundary with fixture coverage.",
      "Review-pass-4 owner-adjacent Rust feedback is resolved by extending Phase B owner_kind/owner_id migration to retry budget ledgers and artifact source-generation claims instead of leaving mediation-owned executions as table-specific exceptions.",
      "Product review-pass-4 feedback is resolved by adding measurable Phase B dogfood exit criteria, a concrete external catalog discovery/attestation mechanism, and a minimal operator-approved known-issues migration record schema.",
      "UX review-pass-4 suggestions are resolved by requiring started-at and relative-duration mediation labels plus direct terminal-unverifiable manual-resolution actions.",
      "Architect review-pass-5 handoff specificity feedback is resolved by adding a scoped implementation-entry handoff sub-contract with durable storage owner, source-of-truth fields, northbound latest-summary/MCP/GraphQL shapes, redaction, and fixture expectations.",
      "Product review-pass-5 feedback is resolved by adding a Phase B readiness checkpoint, allowing intentionally-constructed dogfood scenarios to cover rare conflict types while preserving organic-run evidence, and defining the scope delta if Q-003 approves broader rationale export.",
      "UX review-pass-6 mediation transparency feedback is superseded by the UI DB cutover: sanitized mediation status remains a GraphQL/MCP readback requirement, while concrete UI surfaces move to a future thin-client proposal.",
      "UI feedback is retained as historical review context but is no longer a P017 implementation or audit target after the UI DB cutover."
    ]
  },
  "problem": {
    "summary": "Run D4F404B7-8D3D-483A-956E-5C95F201FD63 exposed a workflow-authority defect: aggregate review truth said the run should loop to proposal refinement, while an agent-authored advisory next_stage named state_3_proposal_drafted, a state absent from the compiled declarative workflow graph. The runtime let advisory progression evidence drift from workflow truth and then blocked without durable conflict ownership.",
    "evidence": [
      "Aggregate review verdict was pass=false with one blocker and should have routed back to refinement.",
      "Persisted run_state recorded next_action=revise_proposal and next_stage=state_3_proposal_drafted.",
      "state_3_proposal_drafted did not exist in the compiled workflow graph.",
      "The runtime had no first-class workflow conflict record, no non-blocking advisory rejection history, and no explicit conflict owner for ambiguous or unverifiable transition outcomes."
    ],
    "operator_impact": [
      "Operators see a blocked run without knowing whether the next step is retry, revise, lead escalation, clone, approval, or manual decision.",
      "Agent-authored transition hints can look authoritative even though they are only evidence.",
      "Reviewer-loop workflows can deadlock or route incorrectly when aggregate truth, advisory hints, and graph truth diverge.",
      "Downstream Proposal 060 remains blocked on the conflict-truth infrastructure delivered by this proposal."
    ],
    "positioning": "P017 makes compiled workflow truth authoritative, persists workflow conflict and advisory rejection evidence as durable runtime truth, and requires a bounded system lead path for conflicts that cannot be resolved by the graph alone."
  },
  "goals": [
    "Treat the compiled workflow graph as the sole authority for legal stage progression.",
    "Treat agent-authored next_stage, next_action, run_state.json, and narrative transition fields as advisory evidence only.",
    "Persist blocking invalid, ambiguous, missing-input, aggregate-conflict, or unverifiable transition outcomes as WorkflowConflictRecord.",
    "Persist rejected advisory hints that do not block graph-authoritative advancement as WorkflowAdvisoryRejectionRecord.",
    "Define one shared control-plane CandidateTransitionEvaluation contract with typed results, provenance, and sanitized diagnostics.",
    "Keep Phase A independently shippable so the D4F404B7-class bug is fixed before lead mediation and mandatory lead validation land.",
    "Escalate same-run-resolvable blocking workflow conflicts to exactly one system lead before broad clone fallback once Phase B is enabled.",
    "Require every executable workflow/catalog pair to resolve exactly one valid system lead escalation path by Phase C.",
    "Expose conflict reason, current state, advisory rejection, lead owner, valid next action class, mediation progress, and resolution history in operator and report surfaces.",
    "Register and maintain ./scripts/test-gate.sh proposal-017 as the canonical gate.",
    "Make Phase B mediation-owned AgentExecution persistence implementable against current Rust, GraphQL, MCP, cancellation, runtime-facts, artifact, and cost boundaries before provider work begins.",
    "Make aggregate artifact field authority and unknown transition-input classification single-valued in the control-plane.",
    "Make implementation-start handoff deterministic: approved proposal freeze, handoff artifact paths, worktree/provisioning metadata, and implementation-start readback are engine-owned and cannot be lost behind a lead/orchestrator ACP timeout.",
    "Define per-surface report/API naming and enum translation so semantic parity does not rely on one casing rule.",
    "Keep transition cursor, resume, recovery, and workflow_conflict truth atomically aligned for legal advancement, blocking conflicts, terminal outcomes, and lead-mediated settlement.",
    "Make Phase B retry-budget and artifact-source-claim ownership explicit for mediation-owned executions before provider-backed mediation is enabled.",
    "Make rollout decisions measurable with dogfood exit gates, external catalog discovery evidence, and typed known-issues migration records."
  ],
  "non_goals": [
    "No runtime creation of workflow states not present in the compiled graph.",
    "No replacement of declarative transition evaluation with lead judgment.",
    "No broad multi-lead routing model; exactly one system lead remains required.",
    "No new scoring system or reviewer-voting semantics.",
    "No UI implementation. P017 owns control-plane truth and readback only; UI replacement belongs to thin-client GraphQL projection work.",
    "No local UI smoke tests, Xcode, simulator, daemon startup, cargo tests, benchmarks, load tests, or fuzzing in proposal-readiness review mode.",
    "No synthetic StageExecution for lead mediation; mediation-owned executions are owner-kind records, not workflow graph states.",
    "No preservation of Rust exists(unknown_artifact) behavior for graph-authoritative transition decisions."
  ],
  "current_system_anchors": [
    "Control-plane transition paths currently evaluate compiled plans and use first-match or generic blocking behavior rather than typed multi-candidate conflict classification.",
    "P057 already treats agent-authored run_state.json as advisory and preserves DB-owned run_state_projection and artifact_contract_advisories for readback.",
    "Control-plane report/readback payloads currently expose blockedReason, recovery paths, failure evidence, and transition cursor fields, but no typed workflow_conflict object.",
    "Rust GraphQL and MCP report surfaces already expose run_state_projection_json or report payloads that can receive additive optional fields.",
    "Example catalogs rely on lead_orchestrator by convention; P017 makes system_role=lead explicit executable catalog truth.",
    "Rust AgentExecution currently requires stage_execution_id in the domain, SQLite schema, repository joins, cancellation, GraphQL conversion, and stage readback; Phase B must migrate this explicitly before mediation execution work starts.",
    "Rust currently has an unknown-artifact exists() fallback that can evaluate true; P017 overrides this behavior with fail-closed candidate input classification.",
    "Canonical workflow_conflict JSON uses snake_case for storage and MCP report JSON; GraphQL exposes typed fields with GraphQL casing.",
    "P053 dogfood exposed a related workflow-entry defect: after implementation approval, state_7_implementation_started invoked lead_orchestrator to freeze approved_proposal and prepare implementation handoff artifacts; the ACP session idled out before producing outputs, code_writer never started, and run_state artifact readback could still imply running implementation while canonical DB state was blocked.",
    "Transition settlement must persist transition-boundary truth through control-plane cursor/resume state while replacing first-match or generic blocked behavior.",
    "Rust retry budget and artifact source-generation claim paths are currently stage-scoped through stage_execution_id and agent_execution_id; Phase B mediation-owned executions must either migrate these adjacent tables to owner_kind/owner_id or use explicit mediation-specific equivalents with fixtures."
  ],
  "architecture": {
    "layer_w_declarative_workflow_authority": {
      "components": [
        "TransitionAuthorityResolver",
        "CandidateTransitionEvaluation",
        "WorkflowConflictClassifier",
        "WorkflowConflictRecord",
        "WorkflowAdvisoryRejectionRecord",
        "AdvisoryHintExtraction"
      ],
      "authority_rules": [
        "The compiled workflow graph is the only authority for legal next state selection.",
        "Agent-authored next_stage, next_action, run_state.json, and narrative transition hints are advisory evidence only.",
        "A legal declarative transition always beats a conflicting advisory hint.",
        "An advisory next_stage absent from the graph never creates a synthetic state.",
        "Multiple matched declarative transitions are blocking conflicts unless an explicit tie-break syntax exists in the compiled workflow.",
        "P017 default for multi-match is to block with a typed conflict; future tie-break syntax is deferred to Q-001."
      ],
      "candidate_transition_evaluation": {
        "result_enum": [
          "matched",
          "not_matched",
          "missing_input",
          "invalid_expression",
          "evaluation_error"
        ],
        "fields": [
          {
            "name": "transition_id",
            "type": "string",
            "nullable": false
          },
          {
            "name": "from_state_id",
            "type": "string",
            "nullable": false
          },
          {
            "name": "to_state_id",
            "type": "string",
            "nullable": false
          },
          {
            "name": "condition_expression_id",
            "type": "string",
            "nullable": true
          },
          {
            "name": "result",
            "type": "enum",
            "nullable": false
          },
          {
            "name": "required_artifacts",
            "type": "array<string>",
            "nullable": false
          },
          {
            "name": "missing_artifacts",
            "type": "array<string>",
            "nullable": false
          },
          {
            "name": "missing_fields",
            "type": "array<string>",
            "nullable": false
          },
          {
            "name": "source_artifact_ids",
            "type": "array<string>",
            "nullable": false
          },
          {
            "name": "source_agent_execution_id",
            "type": "string",
            "nullable": true
          },
          {
            "name": "sanitized_diagnostic",
            "type": "string",
            "nullable": true
          }
        ],
        "mapping_rules": [
          "One matched transition and no blocking graph conflict selects that declarative transition.",
          "Zero matched transitions with only not_matched results emits no_declarative_transition_matched.",
          "Zero matched transitions with missing_input emits required_artifact_or_field_missing_for_transition unless invalid_expression or evaluation_error makes the result unverifiable.",
          "Any invalid_expression or evaluation_error that prevents a confident graph decision emits workflow_conflict_unverifiable.",
          "More than one matched transition without explicit tie-break emits multiple_declarative_transitions_matched_without_tie_break.",
          "Aggregate artifacts with contradictory pass, blocker, next_action, or next_stage truth emit aggregate_transition_truth_conflicted.",
          "A rejected advisory hint with a legal graph transition writes WorkflowAdvisoryRejectionRecord and does not write workflow_conflict_current or blockedReason.",
          "A rejected advisory hint with no legal graph transition contributes advisory provenance to the blocking WorkflowConflictRecord.",
          "Unknown catalog artifact references are never treated as true. exists(unknown_artifact) and unknown_artifact.field classify as invalid_expression when the artifact id is not declared by the workflow/catalog contract, or missing_input when the artifact is declared but absent at evaluation time.",
          "Aggregate next_action and next_stage fields are advisory-only unless the artifact contract explicitly marks them transition-authoritative. Advisory-only aggregate fields can create advisory rejection history but cannot by themselves create aggregate_transition_truth_conflicted when graph-authoritative fields select a legal transition.",
          "aggregate_transition_truth_conflicted is reserved for contradictions among transition-authoritative or contradiction-bearing fields inside the same aggregate contract, such as pass=true with non-empty blocking_issues when the contract says those fields must agree."
        ]
      },
      "advisory_hint_extraction": {
        "purpose": "Reuse P057 advisory/projection infrastructure instead of creating parallel advisory truth.",
        "inputs": [
          "run_state_projection_v1",
          "artifact_contract_advisories",
          "agent-authored run_state.json when present as artifact evidence",
          "aggregate review artifacts that contain next_action or next_stage hints"
        ],
        "output_fields": [
          "source_artifact_id",
          "source_agent_execution_id",
          "advisory_path",
          "raw_value_hash",
          "redacted_value",
          "graph_membership_result",
          "superseded_by_projection",
          "included_in_candidate_transition_hash"
        ],
        "rules": [
          "Raw advisory values are hashed for fingerprints and redacted for public/operator reports.",
          "If run_state_projection supersedes an agent-authored run_state.json value, the supersession is recorded in advisory provenance.",
          "Rust must consume existing artifact_contract_advisories and run_state_projections where possible.",
          "Legacy Swift bridge fields are not a P017 implementation target after the UI DB cutover."
        ]
      },
      "aggregate_artifact_field_authority": {
        "purpose": "Resolve ARCH-017-R3-002 by making aggregate contract fields deterministic for transition evaluation and D4F404B7 replay parity.",
        "authority_classes": [
          "transition_authoritative",
          "advisory_only",
          "contradiction_bearing",
          "non_authoritative"
        ],
        "proposal_review_summary_v1": [
          {
            "field": "pass",
            "authority": "transition_authoritative",
            "use": "Primary pass/fail branch input for review-loop transitions."
          },
          {
            "field": "blocker_count",
            "authority": "transition_authoritative",
            "use": "Confirms whether failed review must route to refinement."
          },
          {
            "field": "blocking_issues",
            "authority": "transition_authoritative",
            "use": "Provides blocker presence and issue refs for failed-review transition conditions."
          },
          {
            "field": "required_changes",
            "authority": "transition_authoritative",
            "use": "Provides concrete blocker/remediation evidence for failed-review transition conditions."
          },
          {
            "field": "decision",
            "authority": "contradiction_bearing",
            "use": "May indicate internal aggregate inconsistency when it conflicts with pass, blocker_count, or blocking_issues."
          },
          {
            "field": "next_action",
            "authority": "advisory_only",
            "use": "Recorded as advisory transition evidence; never selects a graph transition alone."
          },
          {
            "field": "next_stage",
            "authority": "advisory_only",
            "use": "Graph membership is checked for advisory rejection evidence; absent states never become legal transitions."
          },
          {
            "field": "summary",
            "authority": "non_authoritative",
            "use": "Operator explanation only; not transition input."
          }
        ],
        "d4f404b7_expected_outcome": "When the compiled graph contains the failed-review refinement transition, the replay selects that graph transition from pass=false/blocker evidence and writes WorkflowAdvisoryRejectionRecord for advisory next_stage=state_3_proposal_drafted. It does not block as aggregate_transition_truth_conflicted merely because advisory next_stage is absent from the graph.",
        "extension_rule": "Any future aggregate artifact used by TransitionAuthorityResolver must register a field-authority table before it can contribute transition-authoritative inputs."
      },
      "transition_input_dependency_classification": {
        "purpose": "Resolve ARCH-017-R3-003 by replacing the Rust unknown-artifact true fallback with shared fail-closed behavior.",
        "rules": [
          "Extract artifact and field dependencies from each transition condition before evaluation.",
          "If a referenced artifact id is not declared by the workflow/catalog artifact contract, classify the candidate as invalid_expression and do not match the transition.",
          "If a referenced artifact id is declared but no current artifact instance is available, classify the candidate as missing_input and do not match the transition.",
          "If a referenced field is absent from a known structured artifact schema, classify as invalid_expression when the schema is authoritative and missing_input when the schema is intentionally open or unavailable.",
          "exists(unknown_artifact) never returns true in graph-authoritative evaluation.",
          "Control-plane fixtures must cover exists(unknown_artifact), unknown_artifact.field, declared-but-absent artifact, declared artifact with missing field, and declared artifact with present field."
        ]
      },
      "transition_cursor_and_resume_invariant": {
        "purpose": "Resolve ARCH-017-R4-001 by keeping TransitionAuthorityResolver decisions, workflow_conflict persistence, and existing transition cursor/resume truth in one settlement boundary.",
        "rules": [
          "Every graph-authoritative selected transition must settle through the existing transition cursor boundary before current_state_id changes or the next stage is queued.",
          "When a blocking WorkflowConflictRecord is persisted with status unresolved, lead_mediation_pending, or operator_confirmation_required, the transition cursor remains anchored at the completed/current state and records conflict_id, conflict_fingerprint, current_state_id, candidate_transition_hash, and resume_policy=await_conflict_resolution.",
          "The transition cursor is terminal only for terminal_unverifiable conflicts, irrecoverable evaluation errors, or explicit operator/manual-resolution paths that cannot re-enter same-run graph settlement.",
          "A lead resolution cannot mutate run state directly. It re-enters TransitionAuthorityResolver with the original conflict fingerprint and fresh durable evidence; current_state_id and cursor truth update atomically only after graph settlement succeeds.",
          "Resume and recovery readback prefer canonical DB/cursor/workflow_conflict state over agent-authored run_state.json, and stale advisory artifacts cannot make the cursor appear advanced.",
          "Superseded conflicts update the cursor conflict reference only when the candidate_transition_hash or conflict_fingerprint changes because new durable evidence arrived."
        ],
        "required_fixtures": [
          "D4F404B7 legal refinement settles cursor and current_state through graph-authoritative transition and records non-blocking advisory rejection.",
          "No-match blocking conflict leaves cursor anchored at current state with resume_policy=await_conflict_resolution and non-null workflow_conflict_current.",
          "Lead-resolved same-run continuation re-enters resolver and updates cursor/current_state only after graph-authoritative settlement.",
          "Terminal_unverifiable marks cursor terminal with terminal_failure_reason and does not advertise retry or next-state advancement.",
          "Process restart after unresolved conflict rebuilds report and recovery surfaces from cursor plus WorkflowConflictRecord without consulting advisory run_state as authority."
        ]
      }
    },
    "durable_records": {
      "workflow_conflict_reason_enum": [
        "invalid_next_stage_hint",
        "no_declarative_transition_matched",
        "multiple_declarative_transitions_matched_without_tie_break",
        "required_artifact_or_field_missing_for_transition",
        "aggregate_transition_truth_conflicted",
        "workflow_conflict_unverifiable",
        "implementation_handoff_unavailable"
      ],
      "workflow_conflict_status_enum": [
        "unresolved",
        "lead_mediation_pending",
        "operator_confirmation_required",
        "resolved",
        "superseded",
        "terminal_unverifiable"
      ],
      "workflow_conflict_record_fields": [
        "conflict_id",
        "conflict_fingerprint",
        "run_id",
        "stage_execution_id",
        "lineage_id",
        "current_state_id",
        "reason",
        "operator_label",
        "status",
        "candidate_transitions",
        "candidate_transition_hash",
        "advisory_evidence_refs",
        "lead_agent_id",
        "mediation_record_id",
        "created_at",
        "updated_at",
        "resolved_at",
        "superseded_by_conflict_id",
        "resolution_record_json",
        "terminal_failure_reason",
        "diagnostic_redaction_tier"
      ],
      "workflow_advisory_rejection_record": {
        "purpose": "Durable non-blocking truth for advisory hints rejected while the graph advanced legally.",
        "fields": [
          "rejection_id",
          "run_id",
          "stage_execution_id",
          "lineage_id",
          "current_state_id",
          "selected_transition_id",
          "selected_next_state_id",
          "advisory_next_stage_hint",
          "advisory_next_action",
          "advisory_hint_hash",
          "advisory_hint_provenance",
          "graph_membership_result",
          "created_at"
        ],
        "readback_rules": [
          "Does not appear in workflow_conflict_current.",
          "Does not set or modify blockedReason.",
          "Appears in workflow_conflict_history as a non_blocking_advisory_rejection event.",
          "Appears in MCP reports.get and GraphQL report readback under workflow_conflict.advisory_rejections.",
          "Counts in advisory_rejection_total and invalid_next_stage_hint_non_blocking_total metrics.",
          "Is included in control-plane fixtures for the D4F404B7 class when the graph can still advance."
        ]
      },
      "lifecycle": {
        "conflict_fingerprint": "sha256(run_id + current_state_id + stage_execution_id_or_lineage_id + reason + candidate_transition_hash + advisory_evidence_hash)",
        "uniqueness_rule": "For a given run_id, current_state_id, and conflict_fingerprint, repeated blocking evaluations upsert the same current unresolved record.",
        "current_record_invariant": "At most one non-resolved, non-superseded blocking WorkflowConflictRecord may be current for a run and current_state_id.",
        "status_transitions": [
          "unresolved -> lead_mediation_pending when a valid system lead exists and same-run mediation is safe",
          "unresolved -> terminal_unverifiable when no safe classification, diagnostics, or lead path exists",
          "lead_mediation_pending -> operator_confirmation_required when valid lead output requires operator confirmation",
          "lead_mediation_pending -> resolved when valid lead output needs no operator confirmation and graph-authoritative settlement succeeds",
          "lead_mediation_pending -> terminal_unverifiable when lead output is absent, malformed, mismatched, or watchdog-expired",
          "operator_confirmation_required -> resolved when the operator confirms and settlement succeeds",
          "unresolved -> superseded when later durable evidence changes the fingerprint before resolution"
        ],
        "history_rule": "Resolved, superseded, terminal_unverifiable conflicts and non-blocking advisory rejections remain in history. Only the current unresolved, lead_mediation_pending, or operator_confirmation_required WorkflowConflictRecord drives blocked-run recovery."
      }
    },
    "lead_mediation": {
      "decision": "Lead mediation owns a distinct LeadConflictMediationRecord and executes the lead through a normal AgentExecution linked by mediation_owner_token and lead_mediation_record_id.",
      "why_not_synthetic_stage": "A synthetic StageExecution would create a runtime step absent from the declarative graph and blur workflow truth.",
      "why_not_recovery_action_only": "A recovery action cannot own retries, watchdog expiry, transcripts, runtime facts, provider facts, cost, output validation, or durable rationale.",
      "execution_ownership": {
        "queue_work_item_type": "lead_conflict_mediation",
        "agent_execution_shape": "normal AgentExecution with owner_kind=lead_conflict_mediation, owner_id=mediation_record_id, stage_execution_id null, run_id, mediation_owner_token, lead_mediation_record_id, lead_agent_id, provider_profile_id, model_profile_id, cost fields, transcript refs, runtime facts, and output artifacts. Stage-owned executions keep owner_kind=stage_execution and non-null stage_execution_id.",
        "watchdog_owner": "LeadConflictMediationRecord owns mediation timeout policy; AgentExecution watchdog classification is reused for provider/session failure details.",
        "cancellation": "Cancelling mediation cancels the linked AgentExecution when active, marks the mediation record canceled, and returns the conflict to unresolved unless operator confirmation or terminal_unverifiable has already been persisted.",
        "retry_budget": "Retries reuse the same mediation record and create a new AgentExecution attempt with incremented attempt number unless the conflict fingerprint is superseded.",
        "resume_repair": "Resume rehydrates by idempotency_key and mediation_owner_token, then avoids duplicate queued lead sessions.",
        "runtime_facts": "Provider facts, model facts, token/cost usage, watchdog outcome, transcript refs, and contract validation outcome are persisted through existing AgentExecution/runtime-facts infrastructure.",
        "transcript_and_artifact_location": "Lead transcript and LeadResolutionContract artifact live under the mediation record artifact namespace and are referenced by the linked AgentExecution.",
        "cost_attribution": "Cost is attributed to the run and lead mediation record, with source=lead_conflict_mediation for reporting.",
        "migration_boundary": "Phase B may not begin provider/runtime mediation implementation until the Rust owner-kind migration contract under persistence_contract.rust.phase_b_mediation_owned_execution_migration is implemented and fixture-proven."
      },
      "lead_resolution_contract": {
        "required_fields": [
          "conflict_id",
          "conflict_fingerprint",
          "current_state_id",
          "conflict_reason",
          "resolution_mode",
          "chosen_action",
          "chosen_next_state_id",
          "requires_operator_confirmation",
          "rationale",
          "evidence_conflict_fingerprint"
        ],
        "resolution_mode_enum": [
          "same_run_continue",
          "same_state_retry",
          "approval_or_operator_decision",
          "clone_or_manual_resolution",
          "unverifiable"
        ],
        "validation_rules": [
          "chosen_next_state_id must be null, current_state_id, or a compiled graph state reachable from current_state_id through a legal transition.",
          "The lead may not choose an absent state.",
          "The lead may not bypass a required operator approval gate.",
          "The contract must echo conflict_id and conflict_fingerprint.",
          "If no legal same-run action exists, resolution_mode must be clone_or_manual_resolution or unverifiable and requires_operator_confirmation must be true."
        ],
        "failure_mode": "Malformed, partial, mismatched, absent, or watchdog-expired LeadResolutionContract output sets conflict status to terminal_unverifiable and surfaces operator decision with terminal_failure_reason."
      }
    },
    "persistence_contract": {
      "control_plane_phase_a": {
        "decision": "Persist workflow conflict truth in Rust control-plane tables only.",
        "tables": [
          "workflow_conflicts",
          "workflow_advisory_rejections",
          "workflow_transition_cursors"
        ],
        "constraints": [
          "Logical fields match the shared WorkflowConflictRecord and WorkflowAdvisoryRejectionRecord contracts.",
          "Writes are append/upsert by conflict_fingerprint or rejection_id.",
          "Readback is rebuilt from control-plane persistence, not from SwiftData bridge records or agent-authored run_state.json.",
          "Phase A may ship without automatic lead mediation because lead mediation remains disabled or manual-only."
        ]
      },
      "legacy_swift_bridge": {
        "decision": "Deleted from P017 implementation scope after the UI DB cutover.",
        "audit_rule": "Missing Swift workflow_conflict_records_json_v1, SwiftData bridge migration, or Swift report generation is not a P017 conformance blocker.",
        "replacement": "GraphQL read projections and MCP reports.get expose the control-plane truth."
      },
      "rust": {
        "workflow_conflicts_table": "SQLite table keyed by conflict_id with indexes on run_id, status, current_state_id, and conflict_fingerprint.",
        "workflow_advisory_rejections_table": "SQLite table keyed by rejection_id with indexes on run_id, stage_execution_id, advisory_hint_hash, and selected_transition_id.",
        "lead_mediation_table": "SQLite table keyed by mediation_record_id with idempotency_key, status, attempt, linked AgentExecution ids, lead output, and failure reason.",
        "repository_methods": [
          "upsert_conflict_by_fingerprint(record)",
          "insert_advisory_rejection(record)",
          "get_current_blocking_conflict(run_id, current_state_id)",
          "list_conflict_history_for_run(run_id)",
          "transition_conflict_status(conflict_id, expected_status, next_status, payload)",
          "create_or_reuse_lead_mediation(conflict_id, lead_agent_id, idempotency_key)",
          "link_mediation_agent_execution(mediation_record_id, agent_execution_id, mediation_owner_token)"
        ],
        "phase_b_mediation_owned_execution_migration": {
          "decision": "Adopt a general AgentExecution owner model rather than synthetic StageExecution rows. stage_execution_id becomes nullable only as a compatibility field; owner_kind and owner_id become the authoritative ownership fields for all AgentExecution rows.",
          "owner_kinds": [
            {
              "owner_kind": "stage_execution",
              "owner_id": "stage_execution_id",
              "stage_execution_id": "non-null",
              "lead_mediation_record_id": "null",
              "behavior": "Existing stage-owned execution semantics remain unchanged."
            },
            {
              "owner_kind": "lead_conflict_mediation",
              "owner_id": "lead_mediation_record_id",
              "stage_execution_id": "null",
              "lead_mediation_record_id": "non-null",
              "behavior": "Execution belongs to a LeadConflictMediationRecord and is reported through workflow_conflict mediation readback, not through a stage execution list."
            }
          ],
          "sqlite_migration": [
            "Add owner_kind TEXT NOT NULL DEFAULT stage_execution, owner_id TEXT, mediation_owner_token TEXT NULL, lead_mediation_record_id TEXT NULL, and run_id TEXT NOT NULL if not already directly present on agent_executions.",
            "Backfill owner_id = stage_execution_id and owner_kind = stage_execution for all existing rows.",
            "Rebuild or migrate agent_executions so stage_execution_id is nullable, with repository-level and database CHECK invariants requiring stage_execution_id for owner_kind=stage_execution and forbidding it for owner_kind=lead_conflict_mediation.",
            "Add indexes on (run_id, owner_kind), (owner_kind, owner_id), (lead_mediation_record_id), and mediation_owner_token.",
            "Keep the stage_executions foreign key for stage-owned rows; mediation-owned rows reference lead_conflict_mediations(mediation_record_id)."
          ],
          "domain_model": [
            "Replace the single non-optional StageExecutionId owner in Rust AgentExecution with AgentExecutionOwner { StageExecution(StageExecutionId), LeadConflictMediation(MediationRecordId) } while preserving helper accessors for stage-owned call sites.",
            "Serialization includes owner_kind and owner_id. stage_execution_id remains present only for stage-owned rows and is null for mediation-owned rows.",
            "Legacy Swift optional StageExecution relationship is outside P017 conformance after the UI DB cutover."
          ],
          "repository_semantics": {
            "list_by_run": "Owner-aware run listing reads agent_executions by run_id and left joins stage_executions and lead_conflict_mediations. It returns both stage-owned and mediation-owned executions unless the caller explicitly requests stage_only.",
            "find_by_stage": "Filters owner_kind=stage_execution and stage_execution_id=<id>. It never returns mediation-owned executions.",
            "cancel_running_by_run": "Cancels all active AgentExecution rows for the run by run_id regardless of owner_kind. For owner_kind=lead_conflict_mediation it also transitions the linked LeadConflictMediationRecord to canceled in the same repository transaction.",
            "stage_scoped_lists": "Existing stage-scoped API lists remain backed by owner_kind=stage_execution rows only, preserving current behavior for existing consumers.",
            "cost_aggregation": "Run-level cost totals include both stage_execution and lead_conflict_mediation owners. Stage-level cost totals include stage_execution owners only. Mediation cost totals filter owner_kind=lead_conflict_mediation and owner_id=mediation_record_id."
          },
          "owner_adjacent_tables": {
            "purpose": "Resolve ARCH-017-R4-002 by making retry/quota ledgers, source-generation claims, output validation, and late-output settlement work for mediation-owned executions without hidden stage_execution_id exceptions.",
            "agent_retry_budget_ledger": {
              "decision": "Migrate to owner_kind/owner_id plus agent_execution_id and run_id, with stage_execution_id retained as nullable compatibility data only for owner_kind=stage_execution.",
              "idempotency_key": "provider_profile_id + owner_kind + owner_id + agent_id + conflict_fingerprint_or_stage_attempt + quota_window",
              "rules": [
                "Stage-owned retry behavior remains byte-for-byte compatible after backfill.",
                "Mediation-owned retry attempts consume mediation-scoped retry budget and never require a synthetic StageExecution.",
                "Quota retry readback groups lead mediation attempts under workflow_conflict.current.lead_mediation, not stage retry panels."
              ]
            },
            "artifact_source_generation_claims": {
              "decision": "Migrate source-generation claims to owner_kind/owner_id plus source_work_item_id, run_id, agent_execution_id, and artifact_contract_id.",
              "primary_key_shape": "run_id + owner_kind + owner_id + source_work_item_id + artifact_contract_id",
              "rules": [
                "Stage-owned source claims keep existing stage_execution_id readback and supersession behavior through compatibility fields.",
                "LeadResolutionContract generation uses owner_kind=lead_conflict_mediation and owner_id=mediation_record_id.",
                "Late output settlement for a mediation-owned AgentExecution validates the LeadResolutionContract against the current conflict_fingerprint before it can resolve or supersede a WorkflowConflictRecord.",
                "A stale mediation output whose conflict fingerprint has been superseded is recorded as ignored_late_output and cannot mutate cursor or run state."
              ]
            },
            "allowed_alternative": "If implementation chooses mediation-specific retry/claim tables instead of migrating shared tables, the Phase B design record must explain why existing retry, quota, validation, and late-output recovery paths are not needed and must provide equivalent fixtures. Provider-backed mediation may not start until one path is fixture-proven.",
            "required_fixtures": [
              "Mediation-owned provider quota retry increments and exhausts mediation-scoped budget without stage_execution_id.",
              "Mediation-owned source claim creation, idempotent reuse, and supersession work by owner_kind/owner_id.",
              "LeadResolutionContract output validation succeeds for a current conflict fingerprint and fails closed for stale or mismatched fingerprints.",
              "Late output from a canceled or superseded mediation attempt is ignored and recorded without changing cursor, run state, or conflict status.",
              "Backfilled stage-owned retry ledger and source-generation claim rows retain unchanged stage-scoped readback."
            ]
          },
          "northbound_behavior": {
            "mcp_reports_get": "Stage execution sections remain stage-owned only. workflow_conflict.current.lead_mediation.execution_attempts exposes mediation-owned AgentExecution attempts with owner_kind, owner_id, nullable stage_execution_id, watchdog outcome, transcript refs, runtime facts summary, and cost.",
            "graphql": "Existing Stage.agentExecutions/GqlAgentExecution remains stage-scoped and non-null for stage_execution_id. A mediation-owned execution must not be serialized through that field. Owner-aware run/report readback uses a new GqlRunAgentExecution or GqlLeadMediationExecution shape with ownerKind, ownerId, nullable stageExecutionId, mediationRecordId, status, provider/model refs, timing, and cost.",
            "runtime_facts": "Runtime facts attach to agent_execution_id independent of owner_kind. Query helpers may filter by owner_kind but do not duplicate fact tables.",
            "artifacts_and_transcripts": "LeadResolutionContract artifacts and lead transcripts are stored in the mediation artifact namespace and linked to the AgentExecution attempt by artifact refs; stage artifact contracts are not mutated.",
            "reports_and_receipts": "Run reports include mediation execution attempts under workflow_conflict, while release/sign-off receipts summarize mediation status and cost without exposing full debug rationale unless Q-003 permits it."
          },
          "migration_fixtures": [
            "Existing stage-owned AgentExecution row remains visible in find_by_stage, stage GraphQL readback, stage report sections, runtime facts, artifact refs, and stage-level cost totals after migration.",
            "Mediation-owned AgentExecution row with null stage_execution_id is visible in owner-aware list_by_run, cancel_running_by_run, MCP workflow_conflict mediation readback, GraphQL mediation readback, runtime facts, artifact refs, transcript refs, and run-level cost totals.",
            "Stage-scoped GraphQL and repository calls do not return mediation-owned executions.",
            "Cancelling a run with one stage-owned execution and one mediation-owned execution cancels both AgentExecution rows and marks the mediation record canceled exactly once.",
            "Backfilled rows have owner_kind=stage_execution, owner_id=stage_execution_id, and unchanged public readback compared with the pre-migration fixture."
          ]
        }
      }
    },
    "mandatory_lead_validation": {
      "schema_rule": "Each executable workflow/catalog pair must resolve exactly one system lead by Phase C.",
      "catalog_field": "system_role=lead",
      "validator_api": {
        "lead_presence_validator": "validate_catalog_has_exactly_one_system_lead(catalog) -> LeadValidationResult",
        "system_lead_resolver": "resolve_system_lead(catalog, workflow) -> LeadAgentRef | LeadValidationError",
        "workflow_lead_coverage_gate": "validate_workflow_lead_coverage(workflow, catalog, provider_profiles, permission_profiles, output_contract_registry) -> LeadCoverageResult"
      },
      "error_codes": [
        "lead_missing",
        "lead_ambiguous",
        "lead_provider_unavailable",
        "lead_permission_profile_invalid",
        "lead_backend_profile_invalid",
        "lead_resolution_contract_missing",
        "lead_not_allowed_for_workflow",
        "lead_escalation_path_unreachable"
      ],
      "rollout_scope": "Parser compatibility remains for old catalogs. Fail-closed validation applies to executable workflow/catalog pairs entering runtime."
    },
    "implementation_start_handoff_authority": {
      "purpose": "Apply P017 workflow-authority rules to the boundary between implementation approval and the first code-writing invocation so a run cannot appear to have started implementation when only a fragile lead/orchestrator handoff was attempted.",
      "incident_class": "P053-style implementation entry: a graph-authoritative approval transition enters an implementation-start state, but required deterministic handoff artifacts are delegated to an LLM lead invocation and can disappear behind provider timeout before code_writer starts.",
      "engine_owned_handoff_artifacts": [
        "approved_proposal copied or snapshotted from proposal_current with source artifact id and content digest",
        "implementation handoff namespace and required target paths such as implementation_plan and implementation_backlog",
        "worktree/provisioning metadata, owner ids, idempotency keys, and claim/start attempt ids",
        "run/report projection fields that distinguish graph state, deterministic handoff status, and code_writer start status"
      ],
      "authority_rules": [
        "The engine owns deterministic freeze/provision work. An LLM lead may summarize, plan, or refine implementation context, but it must not be the only actor capable of creating approved_proposal or durable implementation handoff truth.",
        "A graph transition to an implementation-entry state does not by itself mean code implementation has started. Control-plane readback must expose implementation_handoff_status and code_writer_start_status so operators and future UI clients can distinguish entered_graph_state, handoff_ready, code_writer_running, and blocked_before_code.",
        "The first code_writer invocation may be queued only after engine-owned handoff truth exists or after the workflow explicitly declares that no handoff artifacts are required.",
        "If an optional lead planning invocation times out after deterministic handoff is complete, the run remains retryable from the planning/code boundary without losing approved_proposal truth.",
        "If deterministic handoff cannot be produced, the run blocks with typed workflow_conflict or handoff-failure readback instead of a generic provider timeout and without leaving run_state.json as the apparent source of truth.",
        "Agent-authored run_state.json remains advisory. Canonical run, stage, report, and workflow_conflict projections must be rebuilt when implementation handoff fails or is retried so readback does not claim running implementation while the DB state is blocked."
      ],
      "failure_readback": {
        "recommended_conflict_reason": "implementation_handoff_unavailable",
        "operator_label": "Implementation handoff was not created",
        "required_fields": [
          "current_state_id",
          "handoff_status",
          "approved_proposal_artifact_id",
          "missing_handoff_outputs",
          "last_handoff_agent_execution_id",
          "code_writer_started",
          "retryable_from"
        ],
        "retry_semantics": "Retry may target the handoff/planning invocation, but must reuse or verify the deterministic approved_proposal snapshot instead of asking an LLM to recreate it from memory."
      },
      "durable_storage_and_api_contract": {
        "purpose": "Resolve ARCH-017-R5-001 by mapping implementation-entry handoff readback to durable sources and northbound surfaces with the same precision expected for workflow_conflict.",
        "storage_owner": "RunExecutionState owns implementation_handoff_status and code_writer_start_status. Approved proposal snapshot and handoff files are artifact records owned by the run handoff namespace. Optional lead planning attempts remain AgentExecution records and are not authoritative for deterministic handoff truth.",
        "source_of_truth_fields": [
          {
            "name": "implementation_handoff_status",
            "values": [
              "not_required",
              "pending",
              "ready",
              "blocked_before_code",
              "superseded"
            ],
            "owner": "RunExecutionState / transition cursor projection"
          },
          {
            "name": "code_writer_start_status",
            "values": [
              "not_queued",
              "queued",
              "claimed",
              "running",
              "blocked_before_code"
            ],
            "owner": "RunExecutionState plus code_writer AgentExecution claim/start facts"
          },
          {
            "name": "approved_proposal_artifact_id",
            "owner": "engine-created artifact record in implementation handoff namespace"
          },
          {
            "name": "approved_proposal_digest",
            "owner": "engine-created artifact record digest"
          },
          {
            "name": "missing_handoff_outputs",
            "owner": "handoff validator result stored on transition cursor projection"
          },
          {
            "name": "last_handoff_agent_execution_id",
            "owner": "optional lead planning AgentExecution, nullable"
          },
          {
            "name": "retryable_from",
            "owner": "transition cursor resume policy"
          }
        ],
        "northbound_shapes": {
          "latest_summary_report": "Adds implementation_handoff_status, code_writer_start_status, approved_proposal_artifact_id, and retryable_from when the run is at an implementation-entry boundary.",
          "mcp_reports_get": "implementation_handoff object in snake_case with missing/null optional planning execution ids preserved.",
          "graphql": "GqlImplementationHandoff typed field on run/report readback with enum values for handoffStatus and codeWriterStartStatus plus nullable artifact/execution refs."
        },
        "redaction": [
          "approved_proposal_digest and artifact id are public/operator tier.",
          "approved proposal content is not duplicated into report payloads.",
          "missing_handoff_outputs lists contract ids or output names, not prompt text or artifact bodies.",
          "last_handoff_agent_execution_id links to normal execution readback subject to existing transcript redaction rules."
        ],
        "fixtures": [
          "Engine-created approved_proposal snapshot survives optional lead planning timeout and is visible by artifact id/digest.",
          "code_writer_start_status remains not_queued or blocked_before_code until a code_writer AgentExecution is actually queued/claimed/started.",
          "Latest summary, MCP reports.get, and GraphQL expose equivalent implementation handoff semantics after casing and enum translation.",
          "Retry from handoff/planning boundary reuses the approved proposal snapshot and transition cursor resume policy.",
          "Stale agent-authored run_state.json cannot override implementation_handoff_status or code_writer_start_status."
        ]
      },
      "workflow_authoring_rule": "Workflow labels such as implementation_started should not be the only operator-facing truth. If the state contains pre-code handoff work, control-plane readback must describe the substate precisely, or the workflow should split deterministic handoff and code implementation into separate states."
    },
    "northbound_report_api_contract": {
      "schema_versioning_decision": "Add workflow_conflict as an optional semantic object on RunReportPayload. No Phase A report schema version bump is required, but each northbound surface has an explicit shape and casing translation.",
      "object_name": "workflow_conflict",
      "enum_casing": "snake_case",
      "old_report_behavior": "Older reports without workflow_conflict display no conflict summary and remain readable.",
      "fields": [
        {
          "name": "current",
          "type": "WorkflowConflictSummary",
          "nullable": true,
          "tier": "public"
        },
        {
          "name": "history",
          "type": "array<WorkflowConflictSummary>",
          "nullable": false,
          "tier": "operator"
        },
        {
          "name": "advisory_rejections",
          "type": "array<WorkflowAdvisoryRejectionSummary>",
          "nullable": false,
          "tier": "operator"
        },
        {
          "name": "blocked_reason",
          "type": "string",
          "nullable": true,
          "tier": "public"
        },
        {
          "name": "lead_owner",
          "type": "string",
          "nullable": true,
          "tier": "public"
        },
        {
          "name": "valid_next_action_class",
          "type": "string",
          "nullable": true,
          "tier": "public"
        },
        {
          "name": "candidate_transition_matrix",
          "type": "array<CandidateTransitionEvaluation>",
          "nullable": false,
          "tier": "debug"
        },
        {
          "name": "resolution_record_json",
          "type": "object",
          "nullable": true,
          "tier": "debug"
        }
      ],
      "summary_fields": [
        "conflict_id",
        "reason",
        "operator_label",
        "status",
        "current_state_id",
        "lead_agent_id",
        "mediation_record_id",
        "created_at",
        "updated_at",
        "terminal_failure_reason",
        "operator_required_action"
      ],
      "readback_surfaces": [
        "MCP reports.get",
        "GraphQL run/report readback",
        "control-plane latest summary/report projection"
      ],
      "redaction": [
      "Raw advisory values are never public tier.",
      "Operator tier receives redacted advisory values and natural-language summaries.",
      "Full lead rationale and resolution_record_json remain local debug tier until Q-003 privacy review is complete.",
      "Live mediation status updates are operator-tier summaries derived from explicit progress events only; they must not expose chain-of-thought, hidden reasoning, provider prompts, or raw transcript text.",
      "Provider prompts, secrets, credentials, and unrelated artifact payload text must not be copied into workflow_conflict fields."
      ],
      "per_surface_shapes": {
        "semantic_contract": "Canonical proposal text uses snake_case field names and snake_case JSON enum values for storage, MCP JSON, fixtures, and documentation tables.",
        "mcp_reports_get": {
          "object_key": "workflow_conflict",
          "field_casing": "snake_case",
          "enum_encoding": "snake_case strings",
          "required_keys": [
            "current",
            "history",
            "advisory_rejections",
            "blocked_reason",
            "lead_owner",
            "valid_next_action_class",
            "candidate_transition_matrix",
            "resolution_record_json"
          ],
          "old_report_behavior": "Missing workflow_conflict is omitted from synthetic summaries and must not fail reports.get."
        },
        "graphql": {
          "field": "workflowConflict",
          "shape": "typed GqlWorkflowConflict with current, history, advisoryRejections, blockedReason, leadOwner, validNextActionClass, candidateTransitionMatrix, resolutionRecordJson, and leadMediationExecutionAttempts where available",
          "enum_encoding": "GraphQL enum values use SCREAMING_SNAKE_CASE while JSON string payloads retain snake_case.",
          "old_report_behavior": "Null workflowConflict means the report predates P017 or has no conflict data."
        },
        "parity_rule": "Fixtures assert semantic equality after casing and enum translation for MCP reports.get, GraphQL, and control-plane latest summary/report readback."
      }
    }
  },
  "ux_ui_notes": {
    "status": "out_of_scope_after_ui_db_cutover",
    "decision": "P017 does not implement macOS UI surfaces. Operator-facing display belongs to a future thin UI proposal that consumes GraphQL read projections only.",
    "operator_story": "When a run blocks because workflow truth is invalid, ambiguous, missing, or unverifiable, control-plane readback exposes typed workflow_conflict data that a future UI can render.",
    "required_readback_for_future_ui": [
      "conflict reason and status",
      "current state and valid next action class",
      "lead owner or no-lead indicator",
      "advisory rejection summary",
      "terminal failure reason when present",
      "sanitized mediation progress when Phase B is enabled"
    ],
    "forbidden_paths": [
      "SwiftData UI readback",
      "Swift UI writes to workflow truth",
      "UI use of MCP command/debug surfaces",
      "UI reconstruction of conflict state from local files or agent-authored run_state.json"
    ],
    "audit_rule": "Do not fail P017 for missing Conflict Details GroupBox, timeline conflict icon, recovery button, Swift accessibility label, or UI smoke evidence. Those are thin-client requirements for a separate proposal."
  },
  "implementation_plan": [
    {
      "phase": "Phase 0: gate and fixtures",
      "scope": [
        "Register ./scripts/test-gate.sh proposal-017.",
        "Add fixture groups for blocking conflicts, non-blocking advisory rejections, control-plane report/API readback, and Phase B mediation replay/resume.",
        "Add sanitized D4F404B7-class replay evidence."
      ],
      "exit_criteria": [
        "proposal-017 gate is discoverable through ./scripts/test-gate.sh list.",
        "Fixture names document expected conflict reasons, statuses, advisory rejection behavior, and report fields.",
        "No UI smoke test requirement is introduced by the proposal-readiness gate."
      ]
    },
    {
      "phase": "Phase A: authority, conflict truth, and advisory rejection truth",
      "independently_shippable": true,
      "scope": [
        "Implement TransitionAuthorityResolver and CandidateTransitionEvaluation in the Rust control-plane.",
        "Integrate AdvisoryHintExtraction with P057 run_state_projection and artifact_contract_advisories.",
        "Persist blocking WorkflowConflictRecord by fingerprint.",
        "Persist non-blocking WorkflowAdvisoryRejectionRecord when graph truth advances despite a bad advisory hint.",
        "Expose workflow_conflict report object, blockedReason, recovery readback, and parity fixtures.",
        "Keep automatic lead mediation disabled or manual-only.",
        "Add aggregate artifact field-authority tables for proposal_review_summary_v1 and any related aggregate contract used by transition evaluation.",
        "Replace Rust unknown-artifact exists() true fallback for graph-authoritative decisions and add control-plane fixtures.",
        "Preserve transition cursor/resume invariants: selected graph transitions, blocking conflicts, and terminal_unverifiable outcomes settle through one cursor boundary before report or recovery readback changes.",
        "Add implementation-entry handoff authority fixtures and readback: approved_proposal freeze is engine-owned, code_writer is not reported as started before claim/start, and P053-style lead timeout blocks with implementation_handoff_unavailable or equivalent typed handoff failure."
      ],
      "exit_criteria": [
        "Invalid advisory next_stage cannot advance to an absent state.",
        "A legal graph transition with a rejected advisory hint advances and writes advisory rejection history, not workflow_conflict_current.",
        "No-match, multi-match, missing-input, aggregate-conflict, and unverifiable graph outcomes persist typed blocking WorkflowConflictRecord.",
        "D4F404B7-class replay yields either graph-authoritative refinement plus advisory rejection history or a typed blocking conflict with non-null blockedReason.",
        "Control-plane resolver, advisory rejection, report, and conflict persistence fixtures pass.",
        "Transition cursor fixtures prove D4F404B7 legal refinement, no-match blocking conflict, lead-resolved continuation, terminal_unverifiable, and restart/resume readback stay aligned with WorkflowConflictRecord truth.",
        "D4F404B7-class replay has one expected outcome in parity fixtures: graph-authoritative refinement plus advisory rejection when the refinement transition exists.",
        "exists(unknown_artifact) and unknown_artifact.field never match a transition in the control-plane.",
        "MCP reports.get, GraphQL, and control-plane latest summary/readback pass per-surface workflow_conflict shape fixtures.",
        "P053-style implementation-entry replay proves deterministic approved_proposal handoff survives a lead/orchestrator provider timeout and readback says blocked_before_code, not code implementation started."
      ]
    },
    {
      "phase": "Phase B: lead mediation",
      "scope": [
        "Add LeadConflictMediationRecord persistence and mediation_owner_token.",
        "Execute lead sessions as normal AgentExecution records with owner_kind=lead_conflict_mediation.",
        "Validate LeadResolutionContract output and re-enter TransitionAuthorityResolver for settlement.",
        "Implement watchdog, retry, cancellation, resume repair, transcript, runtime facts, cost attribution, and report readback.",
        "Expose mediation progress and terminal failure context through GraphQL/MCP readback, not UI-owned DB state.",
        "Run the Rust AgentExecution owner-kind migration before provider mediation sessions: owner_kind/owner_id become authoritative, stage_execution_id is nullable only for mediation-owned executions, and stage-scoped readback remains unchanged.",
        "Migrate or equivalently replace owner-adjacent retry budget ledgers and artifact source-generation claims before provider-backed mediation uses quota retry, output validation, or late-output settlement.",
        "Emit sanitized mediation progress events in GraphQL/MCP readback without exposing hidden reasoning or raw transcript text."
      ],
      "entry_criteria": [
        "After Phase A ships, run a Phase B readiness checkpoint that orders owner migration, retry/claim migration, Q-003 privacy review, GraphQL/MCP readback, and dogfood work by dependency, effort, and parallelizable work.",
        "Q-003 privacy review is complete or default debug-only rationale policy is enforced.",
        "Rust mediation-owned execution migration fixtures pass for list_by_run, find_by_stage, cancel_running_by_run, MCP reports.get, GraphQL mediation readback, runtime facts, artifacts, transcripts, and cost aggregation.",
        "Rust owner-adjacent retry ledger and artifact source-generation claim fixtures pass for mediation-owned provider quota retry, source claim idempotency, LeadResolutionContract validation, and late-output settlement.",
        "Runtime flag default-on decision is blocked until Phase B dogfood exit criteria are met."
      ],
      "exit_criteria": [
        "Lead mediation record is idempotent per run/conflict/lead/fingerprint.",
        "Every mediation attempt has linked AgentExecution runtime facts, transcript refs, watchdog outcome, and cost attribution.",
        "Valid lead output cannot mutate run state directly and must settle through graph authority.",
        "Invalid, absent, mismatched, or watchdog-expired lead output sets terminal_unverifiable and exposes terminal_failure_reason.",
        "No mediation-owned AgentExecution is exposed through stage-scoped GqlAgentExecution or find_by_stage readback.",
        "Run-level cancellation and cost aggregation include both stage-owned and mediation-owned AgentExecution rows exactly once.",
        "Provider quota retry, source-generation claim supersession, LeadResolutionContract validation, and late-output settlement behave for mediation-owned executions without requiring stage_execution_id.",
        "Dogfood exit criteria are met or mediation remains runtime-flag-gated."
      ]
    },
    {
      "phase": "Phase C: fail-closed lead validation",
      "scope": [
        "Add system_role=lead catalog schema support.",
        "Add LeadPresenceValidator, SystemLeadResolver, and WorkflowLeadCoverageGate.",
        "Fail executable workflow/catalog validation for missing, duplicate, unreachable, or invalid lead escalation paths.",
        "Apply strict validation to bundled examples and new executable workflows first.",
        "Produce Phase C lead-validation enforcement inventory: bundled catalog scan, external active catalog discovery or operator attestation, warning-window or waiver decision, and typed migration warnings."
      ],
      "entry_criteria": [
        "Compatibility window decision is recorded: external legacy catalogs receive two release cycles of warning before Phase C fail-closed enforcement, unless no external active catalogs exist at ship time.",
        "GraphQL/MCP readback fixtures prove validation error facts and typed codes are exposed for future thin-client rendering.",
        "Release cycle definition is recorded as two tagged releases carrying validation warnings and at least 60 calendar days unless the inventory proves no active external catalogs exist.",
        "External catalog discovery mechanism is recorded as automated catalog registry/usage telemetry when available, otherwise operator attestation with scanned paths, owner, last-used evidence, and approval."
      ],
      "exit_criteria": [
        "agents.yaml without explicit system_role=lead fails executable workflow/catalog validation.",
        "Duplicate system leads fail validation.",
        "Provider, permission, backend profile, workflow permission, and output-contract gaps emit typed error codes.",
        "Bundled examples and proposal-017 fixtures remain strict."
      ]
    }
  ],
  "rollout": {
    "sequencing": [
      "Land Phase 0 gate and fixtures first.",
      "Ship Phase A independently to fix graph authority, conflict persistence, advisory rejection history, report readback, and D4F404B7-class behavior.",
      "Enable Phase B lead mediation behind a runtime flag until ownership, watchdog, retry, cancellation, cost, transcript, report, and GraphQL/MCP readback gates pass.",
      "Enable Phase C fail-closed validation for bundled examples and new executable workflows first.",
      "Apply external legacy catalog enforcement after a two-release-cycle warning window unless no external active catalogs exist at Phase C ship time."
    ],
    "migration_notes": [
      "Phase A may surface workflows that relied on implicit first-match ordering as ambiguous next step conflicts. The fix is explicit tie-break syntax from a future proposal or transition re-authoring.",
      "Before Phase A merges, scan bundled workflow YAML examples for transition guards that can match simultaneously and document the count.",
      "Legacy Swift Phase A JSON bridge behavior is superseded by the UI DB cutover and is not part of P017 conformance.",
      "Catalog parser compatibility remains, but runtime-entry validation becomes fail-closed for executable pairs in Phase C.",
      "Bundled workflow scan action threshold: if simultaneous transition matches are found in bundled workflows, Phase A merge is blocked until those transitions are re-authored. Shipping with known issues requires an explicit operator-approved migration record naming each affected workflow and expected conflict label.",
      "For Phase C, one release cycle means one tagged app/control-plane release that carries runtime validation warnings and release-note migration guidance. The two-release-cycle window means two such tagged releases and at least 60 calendar days, unless the enforcement inventory proves no active external legacy catalogs exist.",
      "Phase C enforcement inventory must record bundled catalog scan results, external active catalog scan results or operator attestation, warning or waiver decision, and typed migration warnings."
    ],
    "phase_b_dogfood_exit_criteria": {
      "purpose": "Resolve PO4-001 and PO5-002 by making the runtime-flag-to-default-on decision measurable while allowing rare conflict types to be intentionally exercised.",
      "minimum_sample": "At least 10 mediation-exercising local dogfood runs across at least two workflows, including one no-match or missing-input conflict, one same-run continuation, and one terminal_unverifiable or operator-confirmation path.",
      "scenario_source_rule": "The sample may combine organic dogfood runs and intentionally-constructed conflict scenarios. At least five runs must use normal operator workflows, while rare conflict types such as aggregate_transition_truth_conflicted or multiple_declarative_transitions_matched_without_tie_break may be covered by seeded fixtures or intentionally-authored workflow variants if the decision record labels them as constructed.",
      "coverage_matrix": [
        "no_declarative_transition_matched or required_artifact_or_field_missing_for_transition",
        "same_run_continue with valid LeadResolutionContract",
        "terminal_unverifiable or operator_confirmation_required",
        "one rare conflict type covered organically or by constructed scenario when available"
      ],
      "minimum_completion_rate": "At least 90 percent of mediation attempts reach resolved, operator_confirmation_required, or terminal_unverifiable with non-null terminal_failure_reason within the configured watchdog window.",
      "quality_gates": [
        "Zero duplicate mediation sessions for the same conflict fingerprint.",
        "Zero mediation-owned AgentExecution rows leak into stage-scoped find_by_stage or Stage.agentExecutions readback.",
        "No regression in workflow_conflict_time_to_resolution_seconds against the Phase A clone/manual fallback baseline for comparable conflict types.",
        "At least 80 percent of dogfood operator feedback events choose a proposed recovery action or mark the guidance sufficient."
      ],
      "decision_record": "A Phase B dogfood exit record stores run ids, workflows, conflict reasons, scenario_source=organic|constructed, mediation outcomes, timing metrics, operator feedback summary, and the default-on or remain-flagged decision."
    },
    "known_issues_migration_record": {
      "purpose": "Resolve PO4-003 by defining the approval artifact required when Phase A ships with any bundled simultaneous transition matches.",
      "storage": "docs/proposals/017-known-workflow-conflict-migrations.yaml or a run-local gated approval artifact copied into the release receipt before merge.",
      "required_fields": [
        "record_id",
        "workflow_path",
        "workflow_id",
        "from_state_id",
        "transition_ids",
        "expected_conflict_reason",
        "operator_label",
        "why_not_reauthored_before_merge",
        "mitigation_or_followup_issue",
        "approver",
        "approved_at",
        "expires_at_or_release"
      ],
      "validation": "The proposal-017 gate fails when bundled simultaneous matches are non-zero and no matching approved record exists for every affected workflow/from_state/transition set.",
      "default_decision": "Re-author bundled workflows before Phase A merge. Known-issues records are an exception path, not the default rollout path."
    },
    "external_catalog_discovery": {
      "purpose": "Resolve PO4-002 by making Phase C external legacy catalog inventory evidence explicit.",
      "preferred_mechanism": "Automated scan of catalog registry, recent run metadata, configured workspace paths, and usage telemetry where available.",
      "fallback_mechanism": "Operator attestation when no registry or telemetry exists.",
      "attestation_fields": [
        "attestor",
        "attested_at",
        "scanned_paths",
        "catalog_count",
        "active_external_catalog_count",
        "last_used_evidence",
        "unknown_coverage_risks",
        "warning_window_decision",
        "approval_ref"
      ],
      "waiver_rule": "Immediate Phase C fail-closed enforcement for external catalogs is allowed only when automated discovery or operator attestation records active_external_catalog_count=0 and unknown_coverage_risks are accepted by the operator."
    },
    "rollback": [
      "If additive report fields affect strict consumers, disable display of optional workflow_conflict fields while keeping persistence and blockedReason truth.",
      "If Phase B mediation creates duplicate sessions or bad resume behavior, disable automatic lead mediation and retain Phase A operator-visible conflict surfaces.",
      "If Phase C external catalog migration blocks legitimate work, use the two-release-cycle warning path for external legacy catalogs only; bundled examples and new workflows remain strict.",
      "If owner-kind AgentExecution migration causes owner-aware readback regressions, keep Phase B mediation disabled and retain Phase A conflict/advisory rejection behavior while stage-owned execution paths remain active.",
      "If per-surface report translation causes client breakage, hide the new optional surface field for that client while preserving storage, MCP/debug readback, and blockedReason truth."
    ]
  },
  "metrics": {
    "baseline": {
      "known_d4f404b7_class_incidents": "Observed at least once in known run history.",
      "current_invalid_next_stage_behavior": "Can persist an advisory next_stage absent from the graph and block without durable conflict owner.",
      "current_null_or_generic_blocked_reason_rate": "Not globally measured before Phase A instrumentation.",
      "lead_mediation_timing_baseline": "Unknown until Phase B dogfood; initial timing targets are aspirational estimates."
    },
    "primary_success_metrics": [
      "Zero runs advance to a state absent from the compiled workflow graph.",
      "Zero D4F404B7-class replay fixtures block with null blockedReason.",
      "One hundred percent of blocking no-match, multi-match, missing-input, aggregate-conflict, invalid-hint-with-no-legal-transition, and unverifiable fixtures persist WorkflowConflictRecord.",
      "One hundred percent of legal-transition plus rejected advisory hint fixtures persist WorkflowAdvisoryRejectionRecord and do not set workflow_conflict_current.",
      "Control-plane candidate-transition matrices, advisory rejection readback, and conflict reason mappings match proposal-017 fixtures.",
      "One hundred percent of Phase C executable workflow/catalog validation cases resolve exactly one system lead or fail with a typed error.",
      "One hundred percent of D4F404B7-class replay fixtures select the same control-plane outcome for aggregate field authority.",
      "One hundred percent of unknown transition-input parity fixtures fail closed and never match through exists(unknown_artifact).",
      "Zero mediation-owned AgentExecution rows appear in stage-scoped GraphQL or find_by_stage readback, while one hundred percent appear in owner-aware run/report readback."
    ],
    "operational_metrics": [
      "advisory_rejection_total",
      "invalid_next_stage_hint_non_blocking_total",
      "workflow_conflict_current_total by reason and status",
      "terminal_unverifiable_total by terminal_failure_reason",
      "lead_mediation_attempt_total by result",
      "duplicate_mediation_session_total",
      "report_readback_completeness for current conflict, history, advisory rejections, lead owner, valid action class, and terminal failure reason",
      "external_catalog_warning_total during Phase C rollout",
      "recovery_action_chosen_total by conflict_reason, action_class, source_surface, and result",
      "workflow_conflict_time_to_resolution_seconds by conflict_reason and resolution_mode",
      "conflict_reason_to_action_outcome_total by conflict_reason, action_class, and terminal status",
      "phase_c_lead_inventory_external_catalog_total by inventory_result and enforcement_decision",
      "phase_b_dogfood_mediation_completion_rate by workflow_id and conflict_reason",
      "phase_b_dogfood_operator_guidance_sufficient_total by action_class and result",
      "mediation_late_output_ignored_total by reason",
      "mediation_retry_budget_exhausted_total by provider_profile_id and conflict_reason"
    ],
    "timing_targets": [
      "Median lead mediation completion target below 5 minutes in local dogfood runs where the lead completes normally.",
      "P95 lead mediation completion target below 15 minutes, excluding explicit operator-confirmation waits.",
      "These targets are aspirational until recalibrated against first Phase B dogfood data."
    ]
  },
  "risks_and_tradeoffs": [
    {
      "risk": "Legacy Swift Phase A JSON bridge behavior remains in old code and is mistaken for P017 truth.",
      "mitigation": "P017 conformance treats legacy Swift bridge behavior as deletion or quarantine work outside this proposal. Audits must evaluate the control-plane persistence and GraphQL/MCP readback contract instead."
    },
    {
      "risk": "Separate advisory rejection records add another durable readback path.",
      "mitigation": "The split prevents non-blocking rejected hints from falsely appearing as active conflicts while still preserving history, metrics, and parity readback."
    },
    {
      "risk": "Multi-match conflict classification changes workflows that relied on first-match ordering.",
      "mitigation": "This incompatibility is intentional; rollout requires a pre-ship bundled workflow scan and migration guidance."
    },
    {
      "risk": "Lead mediation could bypass existing runtime truth.",
      "mitigation": "Lead sessions execute as normal AgentExecution records with mediation ownership, watchdog, runtime facts, transcripts, cost, cancellation, and report provenance."
    },
    {
      "risk": "Exactly-one-lead validation creates pressure for lightweight or legacy catalogs.",
      "mitigation": "Fail-closed validation applies to executable workflow/catalog pairs, while external legacy catalogs receive a two-release-cycle warning window if active catalogs exist."
    },
    {
      "risk": "Report payloads may expose advisory text or lead rationale.",
      "mitigation": "Public/operator tiers are redacted and summary-only; full rationale and resolution_record_json remain local debug tier until Q-003 privacy review records a different outcome."
    },
    {
      "risk": "Legacy Swift implementation diverges from control-plane workflow truth.",
      "mitigation": "Legacy Swift implementation is not a P017 acceptance surface after the UI DB cutover. P017 requires one control-plane implementation, fixed report schema, GraphQL/MCP readback parity, advisory extraction integration, and proposal-017 gate assertions."
    },
    {
      "risk": "General AgentExecution ownership migration touches more Rust/API code than a mediation-only nullable field.",
      "mitigation": "The broader owner_kind/owner_id model is chosen deliberately because it keeps stage semantics unchanged, avoids synthetic workflow states, and prevents future non-stage system tasks from adding a second migration. Phase B remains gated until fixtures prove existing stage-owned behavior is unchanged."
    },
    {
      "risk": "Keeping stage-scoped GraphQL GqlAgentExecution non-null while adding mediation-owned executions creates two readback shapes.",
      "mitigation": "The proposal makes the split explicit: stage-scoped fields remain backward-compatible and mediation attempts appear only in owner-aware run/report mediation shapes with parity fixtures."
    },
    {
      "risk": "Fail-closed unknown-artifact classification can expose existing invalid workflow guards that previously matched in Rust.",
      "mitigation": "Phase A fixtures and validation warnings make this visible before merge; unknown artifacts are workflow authoring errors and should not silently advance graph state."
    },
    {
      "risk": "Engine-owned implementation handoff adds responsibility to workflow transition code that was previously delegated to lead_orchestrator prompts.",
      "mitigation": "P017 intentionally moves deterministic freeze/provision truth out of LLM ownership. LLM lead planning remains allowed after handoff, while approved_proposal, handoff ids, retry identity, and blocked-before-code readback are fixture-proven engine responsibilities."
    },
    {
      "risk": "WorkflowConflictRecord truth can drift from transition cursor or resume readback if settlement is implemented in separate steps.",
      "mitigation": "Phase A requires a single transition cursor boundary for graph advancement, blocking conflict persistence, terminal_unverifiable outcomes, and lead-mediated settlement re-entry, with restart/resume fixtures."
    },
    {
      "risk": "Phase B owner_kind/owner_id migration succeeds for agent_executions but fails around retry ledgers or source-generation claims.",
      "mitigation": "Provider-backed mediation is gated until retry budget ledger and artifact source-generation claim ownership are migrated or equivalently replaced, including quota retry, output validation, and late-output fixtures."
    },
    {
      "risk": "Phase B default-on decision becomes subjective if timing targets are treated as sufficient dogfood evidence.",
      "mitigation": "Runtime-flag removal requires a dogfood exit record with sample size, organic versus constructed scenario labels, rare conflict coverage, completion rate, duplicate-session, readback, time-to-resolution, and operator-feedback gates."
    },
    {
      "risk": "Phase B entry gates are numerous enough to delay automated mediation after Phase A ships.",
      "mitigation": "A Phase B readiness checkpoint after Phase A sequences entry criteria by dependency, effort, and parallelizable work while keeping the safety gates intact."
    },
    {
      "risk": "Operator demand for live mediation transparency could expose private rationale or hidden reasoning.",
      "mitigation": "GraphQL/MCP mediation readback exposes only sanitized progress events and timestamps. Full rationale, raw transcript text, prompts, and hidden reasoning remain redacted unless Q-003 explicitly approves a narrower redacted export scope with fixtures."
    }
  ],
  "acceptance_criteria": [
    {
      "id": "AC-001",
      "criterion": "An invalid agent-authored next_stage cannot advance a run to an absent graph state."
    },
    {
      "id": "AC-002",
      "criterion": "A legal graph transition with a rejected advisory next_stage advances by graph truth and persists WorkflowAdvisoryRejectionRecord without setting workflow_conflict_current or blockedReason."
    },
    {
      "id": "AC-003",
      "criterion": "No-match declarative transition results persist WorkflowConflictRecord instead of generic blocking."
    },
    {
      "id": "AC-004",
      "criterion": "Multiple matched transitions without explicit tie-break persist multiple_declarative_transitions_matched_without_tie_break."
    },
    {
      "id": "AC-005",
      "criterion": "Missing artifacts or fields persist required_artifact_or_field_missing_for_transition with source provenance."
    },
    {
      "id": "AC-006",
      "criterion": "Reports and recovery surfaces show workflow conflict reason label, current state, status, lead owner or no-lead indicator, valid next action class, and terminal failure reason when relevant."
    },
    {
      "id": "AC-007",
      "criterion": "MCP reports.get, GraphQL readback, and control-plane latest summary/report projection expose the same optional workflow_conflict object semantics."
    },
    {
      "id": "AC-008",
      "criterion": "A workflow conflict with a valid same-run resolution path escalates to the system lead before clone fallback once Phase B is enabled."
    },
    {
      "id": "AC-009",
      "criterion": "Every lead mediation attempt has a linked normal AgentExecution preserving runtime facts, watchdog outcome, transcript refs, cost attribution, and output validation."
    },
    {
      "id": "AC-010",
      "criterion": "Lead mediation is idempotent across retry, resume, and restart for the same conflict fingerprint."
    },
    {
      "id": "AC-011",
      "criterion": "The D4F404B7 replay no longer blocks with null or vague reason; blockedReason includes conflict_reason, operator_label, current_state_id, and lead_agent_id or no_lead_available when a blocking conflict exists."
    },
    {
      "id": "AC-012",
      "criterion": "agents.yaml without explicit system_role=lead fails executable workflow/catalog validation in Phase C."
    },
    {
      "id": "AC-013",
      "criterion": "A workflow/catalog pair that cannot escalate workflow conflicts safely fails validation with typed lead coverage error codes."
    },
    {
      "id": "AC-014",
      "criterion": "./scripts/test-gate.sh proposal-017 exists, is listed by the gate wrapper, and exercises control-plane fixtures for resolver, conflict record, advisory rejection, report, mediation, and validation behavior."
    },
    {
      "id": "AC-015",
      "criterion": "Before Phase B mediation provider work starts, Rust AgentExecution owner_kind/owner_id migration fixtures prove existing stage-owned executions are unchanged and mediation-owned executions are visible in owner-aware run/report/cancellation/cost paths with null stage_execution_id."
    },
    {
      "id": "AC-016",
      "criterion": "proposal_review_summary_v1 field authority is fixture-proven: pass, blocker_count, blocking_issues, and required_changes are transition-authoritative; next_action and next_stage are advisory-only; D4F404B7 replay has one control-plane outcome."
    },
    {
      "id": "AC-017",
      "criterion": "exists(unknown_artifact), unknown_artifact.field, and declared-but-absent artifact fixtures fail closed in the control-plane and never produce a matched transition."
    },
    {
      "id": "AC-018",
      "criterion": "The pre-ship bundled workflow simultaneous-match scan blocks Phase A merge for non-zero bundled results unless each affected workflow is re-authored or covered by an explicit operator-approved known-issues migration record."
    },
    {
      "id": "AC-019",
      "criterion": "Post-launch operator feedback metrics record recovery action choice and time-to-resolution for workflow-conflict blocked runs by conflict reason and action class."
    },
    {
      "id": "AC-020",
      "criterion": "A P053-style implementation-entry replay cannot lose approved_proposal or imply code implementation started when lead_orchestrator times out before planning outputs; deterministic handoff truth is engine-owned, code_writer_started=false is visible, and retry resumes from the handoff/planning boundary."
    },
    {
      "id": "AC-021",
      "criterion": "Transition cursor and resume fixtures prove graph-authoritative advancement, unresolved conflicts, lead-resolved continuation, terminal_unverifiable, and restart readback stay consistent with WorkflowConflictRecord and WorkflowAdvisoryRejectionRecord truth."
    },
    {
      "id": "AC-022",
      "criterion": "Before Phase B provider-backed mediation starts, retry budget ledger and artifact source-generation claim ownership is migrated or equivalently replaced for mediation-owned executions, with quota retry, source claim, output validation, and late-output fixtures."
    },
    {
      "id": "AC-023",
      "criterion": "Lead mediation remains runtime-flag-gated until the dogfood exit record meets minimum sample, completion-rate, duplicate-session, readback, time-to-resolution, and operator-feedback gates."
    },
    {
      "id": "AC-024",
      "criterion": "Phase C external catalog enforcement inventory records automated discovery evidence or operator attestation before waiving the two-release-cycle warning window."
    },
    {
      "id": "AC-025",
      "criterion": "Known bundled simultaneous-transition matches cannot ship without either re-authoring or a validated operator-approved known-issues migration record containing the required workflow, transition, rationale, mitigation, approval, and expiry fields."
    },
    {
      "id": "AC-026",
      "criterion": "Implementation-entry handoff readback maps implementation_handoff_status, code_writer_start_status, approved proposal artifact refs, missing outputs, last planning execution, and retryable_from to durable storage and equivalent latest-summary/MCP/GraphQL surfaces."
    },
    {
      "id": "AC-027",
      "criterion": "GraphQL/MCP mediation readback exposes sanitized live status updates with timestamps and attempt number, while hidden reasoning, prompts, raw transcripts, and full rationale remain redacted according to Q-003 policy."
    }
  ],
  "validation": {
    "canonical_command": "./scripts/test-gate.sh proposal-017",
    "required_proof": [
      "Graph-authoritative transition tests showing advisory next_stage cannot override the compiled graph.",
      "Non-blocking advisory rejection fixture proving workflow_conflict_current remains null when the graph advances legally.",
      "Blocking conflict fixtures for no-match, multi-match, missing-input, aggregate conflict, invalid expression, and evaluation error.",
      "D4F404B7-class replay fixture.",
      "AdvisoryHintExtraction fixtures consuming run_state_projection and artifact_contract_advisories.",
      "Conflict persistence and fingerprint upsert tests.",
      "Report/API readback fixtures for latest summary, MCP reports.get, and GraphQL.",
      "Lead mediation fixtures for valid output, malformed output, absent output, watchdog expiry, retry, cancellation, resume, AgentExecution linkage, and cost/transcript/runtime facts.",
      "Lead mediation status update fixture proving GraphQL/MCP readback exposes sanitized progress events and never exposes hidden reasoning, prompts, or raw transcripts.",
      "Transition cursor and resume fixtures for selected legal transition, no-match unresolved conflict, lead-resolved continuation, terminal_unverifiable, and restart readback.",
      "Validation tests for missing lead, duplicate lead, invalid provider/profile/permission coverage, and missing LeadResolutionContract coverage.",
      "Aggregate field-authority fixtures for proposal_review_summary_v1, including D4F404B7 single-outcome replay.",
      "Unknown transition-input fixtures for exists(unknown_artifact), unknown_artifact.field, declared-but-absent artifact, and missing field behavior.",
      "Per-surface workflow_conflict shape fixtures for MCP snake_case reports.get, GraphQL typed fields/enums, and latest summary.",
      "Rust AgentExecution owner-kind migration fixtures proving stage-owned rows remain unchanged and mediation-owned rows work in owner-aware list_by_run, cancel_running_by_run, MCP, GraphQL mediation readback, runtime facts, artifacts, transcripts, and cost aggregation.",
      "Rust retry budget ledger and artifact source-generation claim ownership fixtures for mediation-owned provider quota retry, source claim creation/reuse/supersession, LeadResolutionContract validation, and ignored late outputs.",
      "Bundled workflow simultaneous-match scan fixture with non-zero threshold behavior.",
      "Known-issues migration record validation fixture proving non-zero bundled simultaneous matches fail the gate unless every affected workflow/from_state/transition set has an approved record.",
      "Phase C external catalog discovery or operator-attestation fixture proving warning-window waiver and enforcement decisions are evidence-backed.",
      "Phase B dogfood exit record fixture covering sample size, organic versus constructed scenario source, rare conflict coverage, completion rate, duplicate-session, readback, time-to-resolution, and operator-feedback gates.",
      "Operator feedback metrics fixture for recovery_action_chosen_total and workflow_conflict_time_to_resolution_seconds.",
      "P053-style implementation-entry handoff fixture proving approved_proposal freeze is engine-owned, lead/orchestrator timeout before planning does not erase handoff truth, run_state.json remains advisory, and reports expose blocked_before_code with code_writer_started=false.",
      "Implementation-entry handoff northbound shape fixtures proving latest summary, MCP reports.get, and GraphQL expose equivalent handoff status, code writer start status, approved proposal refs, missing outputs, last planning execution, and retryable_from semantics."
    ],
    "not_required_in_proposal_readiness_mode": [
      "Xcode build",
      "simulator or UI smoke tests",
      "daemon startup",
      "cargo test",
      "benchmarks",
      "load tests",
      "fuzzing"
    ]
  },
  "open_questions": [
    {
      "id": "Q-001",
      "question": "Should a future proposal add explicit transition tie-break syntax?",
      "status": "deferred",
      "default_for_p017": "Treat multi-match as a workflow conflict."
    },
    {
      "id": "Q-002",
      "question": "Should external legacy catalogs receive a warning window before Phase C fail-closed enforcement?",
      "status": "resolved_for_p017",
      "default_for_p017": "Use a two-release-cycle warning window for active external legacy catalogs. One release cycle means a tagged app/control-plane release carrying validation warnings and release-note migration guidance; two cycles also require at least 60 calendar days. If the Phase C enforcement inventory proves no active external catalogs exist at ship time, waive the window and enforce strict validation immediately."
    },
    {
      "id": "Q-003",
      "question": "Should full lead rationale be exportable outside local debug-tier reports?",
      "status": "scheduled_phase_b_readiness_gate",
      "default_for_p017": "No. Public and operator tiers remain summary-only; resolution_record_json and full rationale remain local debug tier unless privacy review approves broader exposure before Phase B readiness closes.",
      "scope_delta_if_approved": "If privacy review approves broader export, Phase B scope expands only to explicitly-redacted rationale summaries, GraphQL/MCP/report parity fixtures, and updated acceptance criteria proving prompts, secrets, hidden reasoning, and unrelated artifact payloads remain excluded. Provider-backed mediation may still proceed with debug-only default if this expanded scope is not approved or not implemented."
    },
    {
      "id": "Q-004",
      "question": "Should owner_kind/owner_id become the long-term model for all future non-stage system executions beyond lead conflict mediation?",
      "status": "resolved_for_p017_scope",
      "default_for_p017": "Yes for P017 data modeling. The migration uses general owner_kind/owner_id now, but only stage_execution and lead_conflict_mediation owner kinds are introduced by this proposal."
    }
  ],
  "resolved_review_decisions": [
    {
      "source": "ARCH-017-R2-001 and SLB-R2-001",
      "decision": "Rejected advisory hints with legal graph advancement use WorkflowAdvisoryRejectionRecord, appear in history and report readback, do not appear as current conflicts, do not set blockedReason, and count in advisory metrics."
    },
    {
      "source": "ARCH-017-R2-002 and SLB-R2-002",
      "decision": "Lead mediation uses LeadConflictMediationRecord for mediation state and normal AgentExecution with mediation_owner_token for provider/runtime truth."
    },
    {
      "source": "ARCH-017-R2-003 and SLB-R2-003",
      "decision": "AdvisoryHintExtraction consumes P057 run_state_projection and artifact_contract_advisories rather than inventing parallel advisory truth."
    },
    {
      "source": "ARCH-017-R2-004 and SLB-R2-004",
      "decision": "Superseded by the UI DB cutover. Swift bridge migration is no longer a P017 implementation requirement; P017 truth is control-plane persistence plus GraphQL/MCP readback."
    },
    {
      "source": "ARCH-017-R2-005 and SLB-R2-005",
      "decision": "The workflow_conflict northbound report object freezes field names, nullability, enum casing, tier membership, old-report behavior, and parity surfaces."
    },
    {
      "source": "ARCH-017-R2-006 and SLB-R2-006",
      "decision": "The reason remains workflow_conflict_unverifiable, while the terminal blocking status is renamed terminal_unverifiable."
    },
    {
      "source": "UX-017-01 and SLB-R2-007",
      "decision": "Operator-facing natural-language labels are specified as readback payload labels for every conflict reason and advisory hint terminology. Concrete UI rendering is outside P017."
    },
    {
      "source": "UX-017-02 and SLB-R2-008",
      "decision": "Lead mediation pending readback exposes active duration and lead agent name for future UI rendering."
    },
    {
      "source": "UX-017-03 and SLB-R2-009",
      "decision": "terminal_failure_reason is required in GraphQL/MCP recovery readback for terminal_unverifiable conflicts."
    },
    {
      "source": "UI-017-001 and SLB-R2-010",
      "decision": "Superseded by the UI DB cutover. The underlying conflict metadata remains required in GraphQL/MCP readback, but no P017 UI placement is required."
    },
    {
      "source": "UI-017-002 and SLB-R2-011",
      "decision": "Superseded by the UI DB cutover. Icon and styling choices are future thin-client UI work, not P017 conformance."
    },
    {
      "source": "PO2-001 and SLB-R2-012",
      "decision": "Superseded by the UI DB cutover. GraphQL/MCP readback coverage is the P017 merge gate; future thin-client UI work owns UX/UI sign-off."
    },
    {
      "source": "PO2-002 and SLB-R2-013",
      "decision": "External legacy catalogs receive a two-release-cycle warning window unless no active external catalogs exist."
    },
    {
      "source": "PO2-003 and SLB-R2-014",
      "decision": "Q-003 privacy review must complete before Phase B readiness closes; debug-only rationale export remains default."
    },
    {
      "source": "PO2-004",
      "decision": "Lead mediation timing targets are marked aspirational until Phase B dogfood data exists."
    },
    {
      "source": "PO2-005",
      "decision": "A pre-ship bundled workflow scan for implicit first-match reliance is added to rollout migration work."
    },
    {
      "source": "ARCH-017-R3-001 and SLB-R3-001",
      "decision": "Phase B mediation-owned execution uses a general Rust AgentExecution owner model. owner_kind/owner_id are authoritative, stage_execution_id is nullable only for non-stage owners, existing stage-owned readback remains unchanged, mediation-owned executions are excluded from stage-scoped GqlAgentExecution/find_by_stage and included in owner-aware run/report/cancellation/cost paths."
    },
    {
      "source": "ARCH-017-R3-002 and SLB-R3-002",
      "decision": "proposal_review_summary_v1 now has a field-authority table. pass/blocker fields are transition-authoritative; next_action/next_stage are advisory-only; D4F404B7 replay resolves to graph-authoritative refinement plus advisory rejection when the refinement transition exists."
    },
    {
      "source": "ARCH-017-R3-003 and SLB-R3-003",
      "decision": "CandidateTransitionEvaluation now fail-closes unknown artifact dependencies. Rust exists(unknown_artifact) true fallback is explicitly rejected for graph-authoritative decisions and covered by parity fixtures."
    },
    {
      "source": "ARCH-017-R3-004 and SLB-R3-004",
      "decision": "workflow_conflict readback now has per-surface shapes for MCP snake_case JSON and GraphQL typed fields with enum casing, plus semantic parity fixtures across translations. Swift JSON is no longer a P017 acceptance surface."
    },
    {
      "source": "ARCH-017-R3-005",
      "decision": "Phase C implementation now requires an enforcement inventory artifact covering bundled catalog scan, external active catalog scan when available, waiver/warning decision, and typed migration warnings."
    },
    {
      "source": "PO3-001 and SLB-R3-006",
      "decision": "Pre-ship bundled workflow scan now has an action threshold: non-zero bundled simultaneous matches block Phase A merge unless re-authored or explicitly accepted through an operator-approved known-issues migration record."
    },
    {
      "source": "PO3-002",
      "decision": "A release cycle is now defined as a tagged app/control-plane release carrying validation warnings and release-note migration guidance; two cycles require at least 60 calendar days unless no active external catalogs exist."
    },
    {
      "source": "PO3-003",
      "decision": "Post-launch operator feedback metrics now track recovery action choices, time-to-resolution, and conflict reason to action outcome."
    },
    {
      "source": "UI-017-001 and SLB-R3-005",
      "decision": "Superseded by the UI DB cutover. Conflict detail field order remains useful future UI guidance, but P017 audits must not require a Conflict Details GroupBox."
    },
    {
      "source": "ARCH-017-R4-001",
      "decision": "TransitionAuthorityResolver now has an explicit cursor and resume invariant. Legal graph transitions, unresolved conflicts, lead-mediated settlement, terminal_unverifiable outcomes, supersession, and restart readback must settle through or rebuild from cursor plus durable conflict truth."
    },
    {
      "source": "ARCH-017-R4-002",
      "decision": "Phase B owner-kind migration now covers owner-adjacent retry budget ledgers and artifact source-generation claims. Provider-backed mediation is gated until quota retry, source claims, LeadResolutionContract validation, and late-output settlement work for mediation-owned executions without synthetic stage rows."
    },
    {
      "source": "PO4-001",
      "decision": "Phase B dogfood exit criteria now define sample size, workflow coverage, completion rate, duplicate-session/readback gates, time-to-resolution comparison, operator feedback threshold, and a required decision record before default-on."
    },
    {
      "source": "PO4-002",
      "decision": "Phase C external catalog discovery now specifies automated registry/usage telemetry where available and an operator attestation fallback with required evidence fields and waiver rules."
    },
    {
      "source": "PO4-003",
      "decision": "Known-issues migration records now have storage, required fields, validation behavior, and a default re-author-before-merge policy."
    },
    {
      "source": "SUG-UX-017-001 and SUG-UX-017-002",
      "decision": "Active mediation readback now includes started-at or relative-time context, and terminal_unverifiable recovery now exposes direct allowed manual-resolution actions."
    },
    {
      "source": "ARCH-017-R5-001",
      "decision": "Implementation-entry handoff authority now has a durable storage and API sub-contract. implementation_handoff_status, code_writer_start_status, approved_proposal refs, missing outputs, last planning execution, and retryable_from map to RunExecutionState, artifact records, transition cursor resume policy, latest-summary readback, MCP reports.get, and GraphQL readback."
    },
    {
      "source": "PO5-001",
      "decision": "Phase B now requires a readiness checkpoint after Phase A ships to order entry gates by dependency, effort, and parallelizable work. The breadth of the checklist remains intentional, but sequencing is no longer left as an unordered backlog."
    },
    {
      "source": "PO5-002",
      "decision": "Phase B dogfood criteria now allow intentionally-constructed rare conflict scenarios alongside organic workflow runs. The decision record labels scenario_source=organic or constructed and retains at least five normal operator workflow runs."
    },
    {
      "source": "PO5-003",
      "decision": "Q-003 now has an explicit scope-delta rule: if broader rationale export is approved, Phase B must add redaction, report/API parity, and acceptance fixtures before exposing it; otherwise debug-only remains default. UI disclosure is future thin-client work."
    },
    {
      "source": "UX-017-R6-001",
      "decision": "Mediation readback now exposes sanitized live status updates with timestamps and attempt number, while hidden reasoning, prompts, raw transcript text, and full rationale remain redacted according to Q-003 policy. Inspect UI rendering is outside P017."
    },
    {
      "source": "SUG-UX-017-003",
      "decision": "Lead confidence score is not added to P017. The proposal preserves typed resolution_mode, requires_operator_confirmation, rationale, and validation outcome instead of introducing a new confidence scale that would need calibration."
    },
    {
      "source": "SUG-UX-017-004",
      "decision": "Advisory rejection history is required in durable readback. Timeline presentation is future thin-client UI work."
    },
    {
      "source": "UI-017-001 and UI-017-002 current review artifact",
      "decision": "Superseded by the UI DB cutover. Repeated UI findings are retained as historical context only and are not P017 implementation or audit requirements."
    }
  ],
  "recommendation": "Proceed to implementation-readiness review for the control-plane scope. Phase 0 and Phase A remain implementation-planning ready, now with explicit transition cursor/resume invariants, implementation-entry handoff storage/API contracts, graph authority, advisory rejection truth, field-authority, fail-closed input classification, per-surface report shapes, and rollout thresholds. Broad Phase B provider-backed mediation remains gated on owner_kind/owner_id AgentExecution migration, owner-adjacent retry/claim fixtures, privacy/default-debug policy or approved Q-003 scope delta, GraphQL/MCP readback evidence, readiness-checkpoint sequencing, and dogfood exit evidence. Phase C remains gated on enforcement inventory, external catalog discovery or attestation, release-window evidence, and GraphQL/MCP validation readback."
}
