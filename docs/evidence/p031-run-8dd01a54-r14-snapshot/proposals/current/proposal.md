{
  "proposal_revision_id": "031-2026-04-21-run-8dd01a54-r14-review-packet-and-tradeoffs",
  "source_review_pass_id": "rp-031-001",
  "title": "Proposal 031: Thin GraphQL-Only UI Rewrite Over Server Projections",
  "status": "Ready for aggregate re-review after r13 clarity refinement",
  "run_id": "8dd01a54-0791-43e0-b526-5ed92c95b34f",
  "date": "2026-04-21",
  "source_proposal": "docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md",
  "primary_proof_lane": "./scripts/test-gate.sh proposal-031 and ./scripts/test-gate.sh p031",
  "executive_summary": "P031 cuts the macOS operator app over from client-owned workflow truth to a thin, GraphQL-only UI over server-owned projections. The Swift app renders GraphQL read models and keeps only presentation state, read-refresh state, and freshness markers. P031-owned UI must not call MCP, define GraphQL mutations, execute local workflow writes, construct command payloads, or own command receipts. MCP remains available for agents, CLI/operator diagnostics, automation, and debug/control workflows outside the app UI. r14 preserves the read-only boundary and turns reviewer conditions into implementation-ready gates: one governing checked-in addendum, executable schema-or-defer decisions, a machine-readable UI inventory, a versioned operator write-path guide, quantitative rollback drill criteria, report-payload priority evidence, explicit dogfood trigger review, and legacy expiry that cannot remove rollback before write-path readiness or a dated release-owner waiver. r14 adds a concise review packet and explicit disagreement/trade-off table so aggregate reviewers can verify the proposal without reconstructing decisions from the full architecture body.",
  "problem": {
    "current_state": "The macOS operator app still has UI-facing paths that can read or infer workflow truth from SwiftData, local compiled plans, recovery services, local execution services, raw artifacts, and prior control affordances. The risk is no longer proposal direction; reviewers agree with the GraphQL-only cutover. The remaining risk is implementation drift from stale GraphQL+MCP handoff artifacts, undefined gate inputs, ambiguous external write-path recipes, and release timing that could remove legacy rollback before critical write paths are restored or explicitly waived.",
    "why_now": "The server projection layer and daemon lifecycle read surfaces are mature enough for the first safe visible cutover. The operator correction narrows P031 to projection-backed inspection first, avoiding the higher-risk UI command surface until a separate proposal owns write transport and safety.",
    "desired_state": "After P031, visible workflow truth comes from GraphQL read models, P031-owned UI has no MCP client usage, no GraphQL mutations, and no local workflow mutation fallback. Swift-local state is limited to presentation, server-derived caches, read-refresh state, and freshness handling. Before Swift migration, implementers have one governing checked-in artifact, one executable P031 gate, one machine-readable UI inventory, one server-owned schema decision for every visible field, and one versioned operator write-path guide. Before dogfood, those artifacts are present in a single handoff manifest and validated against copied UI identifiers. Before legacy removal, release ownership has either merged gate-green critical write-path restoration or a dated waiver with a hard write-restoration deadline."
  },
  "review_ready_packet": {
    "decision_summary": "Approve P031 for Phase 0 contract hardening only. Do not begin governed Swift screen migration, dogfood, flag removal, or legacy rollback removal until the phase-specific gates below are satisfied.",
    "what_changes_for_operators": [
      "Runs, stages, approvals, artifacts, reports metadata, daemon lifecycle, freshness, and degraded state are inspected through GraphQL projections.",
      "Create, start, cancel, retry, approval resolution, Steward, runtime/session, clone, compare, and experiment actions are not available as P031 macOS UI writes.",
      "The UI explains removed write paths through diagnostic copy, copyable identifiers, and the external operator guide rather than disabled primary buttons.",
      "Reports show metadata and payload availability status; full payload rendering remains a follow-up unless a server-owned GraphQL payload read lands and is gate-covered."
    ],
    "implementation_stop_signs": [
      "Stop before Swift migration if the checked-in governing r14/addendum, P043 reconciliation, P031 gate, UI inventory, and schema decision record are not present.",
      "Stop before dogfood if the operator write-path guide, manifest, rollback evidence or waiver, report-payload priority decision, copied-identifier validation, and UX/accessibility checks are incomplete.",
      "Stop before legacy removal if critical write-path readiness is not merged and gate-green and no dated release-owner waiver accepts the remaining gap."
    ],
    "reviewer_acceptance_arguments": [
      "Product risk is contained by legacy-expiry dependency, quantitative rollback criteria, operator workflow-completion notes, and report-payload default P0 follow-up.",
      "UX risk is contained by first-run orientation, diagnostic-only approval treatment, direct/copyable guide access, and explicit copied identifiers.",
      "UI risk is contained by fixed Syncing slots, stable report payload indicator width, distinct diagnostic banner styling, subtle reduced-motion-aware activity, and complete-sentence VoiceOver copy.",
      "Architecture risk is contained by the GraphQL-only boundary, no UI MCP/mutations/local writes, P043 reconciliation, executable schema-or-defer decisions, gate-consumed UI inventory, and Phase 0 artifact manifest."
    ]
  },
  "explicit_disagreement_resolution": [
    {
      "topic": "Whether P031 should restore UI writes through MCP",
      "decision": "Do not restore UI writes in P031. The macOS UI is GraphQL read-only; MCP remains for agents, CLI/operator diagnostics, automation, and debug/control workflows outside governed UI.",
      "reviewer_feedback_resolved": [
        "ARCH-R9-01",
        "ARCH-R10-01",
        "ARCH-R10-02"
      ],
      "tradeoff": "Operators temporarily leave the UI for writes, but P031 avoids shipping an ambiguous second control surface."
    },
    {
      "topic": "Approval decision UX",
      "decision": "Approval rows are diagnostic-only unless a separately approved non-MCP, non-GraphQL UI transport exists. No disabled primary Approve or Reject button is shown as the main state.",
      "reviewer_feedback_resolved": [
        "ARCH-R9-04",
        "UX-R9-01",
        "UI-02",
        "PO-R9-04"
      ],
      "tradeoff": "Approval completion is deferred, but the UI no longer teases unavailable in-app action."
    },
    {
      "topic": "Legacy rollback removal timing",
      "decision": "Legacy rollback removal is blocked until critical write-path readiness is merged and gate-green or a release-owner waiver names the gap and sets a hard write-restoration deadline.",
      "reviewer_feedback_resolved": [
        "PO-R10-01"
      ],
      "tradeoff": "Legacy code can remain longer, but operators do not lose both rollback and write capability by schedule accident."
    },
    {
      "topic": "Report payload priority",
      "decision": "Full report payload rendering is outside P031, but its follow-up defaults to P0 unless Phase 0d provides evidence that metadata-only report inspection is low-frequency and acceptable.",
      "reviewer_feedback_resolved": [
        "PO-R10-02",
        "UI-03"
      ],
      "tradeoff": "The proposal stays read-migration focused while preventing report payload restoration from being silently deprioritized."
    },
    {
      "topic": "Observer and unauthorized diagnostic visibility",
      "decision": "Diagnostic/debug fields are operator-only by default; observer-visible behavior is deferred unless a separate authorization policy and redaction tests land.",
      "reviewer_feedback_resolved": [
        "ARCH-R10-06"
      ],
      "tradeoff": "P031 avoids accidental auth expansion, but future observer diagnostics require a separate decision."
    },
    {
      "topic": "Whether proposal completeness should depend on prose only",
      "decision": "Phase 0 requires a machine-readable UI inventory, gate-consumed operator guide JSON, and Phase 0 artifact manifest so implementation readiness is executable.",
      "reviewer_feedback_resolved": [
        "ARCH-R10-04",
        "ARCH-R10-05",
        "PO-R10-04",
        "REREAD-R12-01"
      ],
      "tradeoff": "Phase 0 has more structured artifacts, but handoff and review drift are reduced."
    }
  ],
  "goals": [
    "Replace P031-owned SwiftUI workflow reads with GraphQL projection read models and freshness metadata.",
    "Make the macOS UI GraphQL-only: queries, subscriptions, bounded polling, and targeted read refresh only.",
    "Strictly prohibit MCP usage from P031-owned macOS UI code. MCP remains for agents, CLI/operator diagnostics, automation, and debug/control workflows outside the UI.",
    "Remove or disable Create Idea, Start Run, Cancel Run, Stage Retry, Steward, runtime-health, session, clone, compare, and experiment write controls from P031-owned screens.",
    "Make approval behavior binary: in P031 it is read-only diagnostic guidance unless a separately approved non-MCP, non-GraphQL transport is named and gate-covered. No disabled primary approval button that looks actionable.",
    "Preserve operator inspection ergonomics for Runs Home, Run Detail, stages, approvals, artifacts, report metadata, daemon lifecycle, and recovery/evidence readback.",
    "Ship report metadata inspection as the P031 report deliverable with list-level payload availability indicators and stable density rules.",
    "Reconcile P043/P031 reference and gate language so command-completion refresh, command receipts, and MCP control rules are outside P031-owned UI.",
    "Define concrete GraphQL fields, enum cases, nullability, redaction, Swift presenter ownership, and tests for disabled/report/approval metadata.",
    "Define the Phase 0 P031-owned UI file/type inventory so static guards are precise and do not accidentally miss bypasses or block isolated legacy rollback code.",
    "Publish a pre-dogfood operator write-path guide that maps each removed UI write control to an external workflow or marks it unavailable with a follow-up.",
    "Capture user-outcome evidence during dogfood, not only technical compliance metrics.",
    "Provide fail-closed rollout modes, dogfood edge-case coverage, rollback drill evidence or waiver, freshness measurement, hold criteria, rollback criteria, sign-off authority, and a concrete business-day legacy rollback expiry.",
    "Treat stale GraphQL+MCP handoff text as historical and require one checked-in governing r14/addendum artifact before Swift screen migration.",
    "Make the P031 UI inventory machine-checkable through a gate-consumed JSON or YAML allowlist/glob contract.",
    "Define a minimum-viable operator write-path guide row schema that external workflows and UI copy affordances can both satisfy before dogfood.",
    "Gate legacy rollback code removal on write-path availability or a release-owner waiver with a dated write-restoration deadline.",
    "Define quantitative rollback drill pass/fail criteria for time-to-rollback, state consistency, and operator confirmation.",
    "Default the report payload follow-up to P0 unless Phase 0d provides operator workflow evidence that metadata-only report inspection is low-frequency and acceptable.",
    "Require Phase 3 sign-off to explicitly review additional-evidence triggers before release acceptance.",
    "Define critical write-path readiness precisely so legacy expiry cannot be satisfied by a draft-only follow-up.",
    "Add a Phase 0 implementation-readiness checklist that reviewers and implementers can evaluate without reading every proposal section.",
    "Publish a Phase 0 artifact manifest that lists the governing proposal/addendum, UI inventory, operator guide, schema decision record, gate evidence, rollback evidence, and sign-off evidence.",
    "Make unresolved items explicit external dependencies rather than hidden proposal gaps."
  ],
  "non_goals": [
    "P031 does not redefine workflow execution semantics.",
    "P031 does not create a second control plane.",
    "P031 does not make MCP available to the macOS UI for reads or writes.",
    "P031 does not add GraphQL mutations or any other UI write transport.",
    "P031 does not route UI actions through MCP command tools.",
    "P031 does not add command journaling, CommandHandler wiring, command receipt recovery, ActionInvocationIdentity, CommandLegality, Check Status, Reissue Command, client_command_id command correlation, or MCP parameter mapping for UI writes.",
    "P031 does not implement Create Idea, Start Run, Cancel Run, Stage Retry, reset-session, resume, clone, comparison, experiment launch, runtime-health actions, agent reset, Steward actions, or second-wave MCP tools in the UI.",
    "P031 does not ship full report payload rendering unless a server-owned GraphQL report payload query lands first and is added to the P031 gate.",
    "P031 does not expand non-operator GraphQL read authorization.",
    "P031 does not run local UI smoke tests outside the repository remote-host UI policy.",
    "P031 does not declare external CLI/MCP command recipes complete; it defines the guide schema and blocks dogfood until recipes are named and validated.",
    "P031 does not remove legacy rollback solely because a write-path follow-up has started drafting."
  ],
  "scope": {
    "in_scope_reads": [
      "RunsHomeView reads runs from GraphQL runs projections.",
      "RunDetailView reads run(id:) and runStatusChanged(runID:) from GraphQL.",
      "Stages surfaces read stages(runID:) and stage detail read models from GraphQL.",
      "Artifacts surfaces read artifacts(runID:) from GraphQL.",
      "Reports surfaces read report metadata, list-level payload availability, and payload unavailable reasons from GraphQL.",
      "Approvals queue reads approval rows, ambiguity state, write-path state, and diagnostic identifiers from GraphQL.",
      "Daemon lifecycle and projection freshness are read from server-owned GraphQL/lifecycle surfaces."
    ],
    "in_scope_operator_controls": [
      "Targeted read refresh controls for projection-backed surfaces.",
      "Copy Diagnostic ID and Technical Details affordances for approval diagnostics and unavailable write paths.",
      "First-run dogfood orientation banner explaining the read-only transition and external write workflows."
    ],
    "removed_or_disabled_writes": [
      "ideas.create / Create Idea.",
      "runs.start / Start Run.",
      "runs.cancel / Cancel Run.",
      "stages.retry / Retry Stage.",
      "approvals.resolve unless a separately approved non-MCP, non-GraphQL transport is named outside P031.",
      "steward.run_analysis / Queue Steward analysis.",
      "reset, resume, clone, compare, experiment launch, runtime-health, and session actions.",
      "Any local Swift recovery or execution mutation path.",
      "Any UI MCP command path.",
      "Any UI GraphQL mutation path."
    ],
    "preserved_operator_surfaces": [
      "Runs Home as read/inspect surface.",
      "Run Detail as read/inspect surface.",
      "Stages panel as read/inspect surface.",
      "Artifacts inspector as read/inspect surface.",
      "Reports reader metadata view with payload availability status.",
      "Approvals queue as read/diagnostic surface.",
      "Daemon lifecycle and degraded-state visibility."
    ]
  },
  "architecture": {
    "p043_reconciliation_contract": {
      "decision": "P043 remains the GraphQL projection read contract, but any P043 language that assigns MCP command-control, command receipts, or command-completion refresh behavior to P031-owned UI must be amended or explicitly scoped to non-P031/non-UI surfaces before Phase 1.",
      "phase_0a_required_work": [
        "Amend docs/reference/query-projections-and-client-consumption-contract.md or add a checked-in addendum stating that P031-owned UI consumes GraphQL reads only.",
        "Remove P031 UI ownership of command-completion refresh behavior, command receipt display, and MCP command-control rules from composed P031 gate language.",
        "Keep MCP command behavior documented for non-UI agents, CLI, automation, and diagnostics.",
        "Label any remaining P043 command-related freshness or budget rows as non-P031/non-UI or remove them from the P031 dependency chain.",
        "Update proposal-031/p031 gate expectations so the gate fails on UI MCP calls, UI GraphQL mutations, UI command receipt state, client_command_id command correlation, identity-to-MCP mapping, and local mutation fallbacks.",
        "Mark the run-local idea brief MCP-write acceptance criteria as superseded by the r14 GraphQL-only contract for implementation purposes.",
        "Check in one governing implementation addendum or synchronized proposal that names r14 as the source of truth before Swift screen migration."
      ],
      "exit_criteria": [
        "P043/P031 reference docs state that P031 UI owns no command-completion or command-receipt behavior.",
        "proposal-031/p031 gate checks the GraphQL-only no-UI-write boundary.",
        "Implementation tickets link one governing r14 contract or checked-in addendum with the same Phase 0 obligations.",
        "Stale GraphQL+MCP source artifacts are either updated or explicitly marked historical/superseded in the implementation handoff.",
        "No implementation ticket links the old idea brief as the active behavior contract without also linking the r14/addendum supersession."
      ]
    },
    "read_plane": {
      "decision": "GraphQL is the only macOS UI data plane for workflow truth.",
      "rules": [
        "The UI may query, subscribe, and poll GraphQL read models.",
        "The UI may trigger read refreshes that refetch GraphQL data.",
        "The UI must not use GraphQL mutations.",
        "The UI must not use MCP clients, MCP tools, MCP read helpers, or MCP write helpers.",
        "The UI must not read workflow truth from SwiftData, local compiled plans, local recovery services, raw artifact directories, raw report files, or local execution services.",
        "MCP read/control tools may remain for agents, CLI/operator tooling, automation, and diagnostics, but they are not part of the macOS UI contract.",
        "If a visible workflow field is missing from GraphQL, implementation must add the field to the server read model or disable/defer the UI surface."
      ]
    },
    "ui_write_prohibition": {
      "decision": "P031-owned UI is read-only except for read refreshes and diagnostic copy affordances; P031 adds no write transport.",
      "rules": [
        "No P031-owned Swift view, view model, reducer, store, or service may call an MCP tool.",
        "No P031-owned Swift view, view model, reducer, store, or service may issue a GraphQL mutation.",
        "No P031-owned Swift code may call local RecoveryCoordinator, RunPlanCompiler, ExecutionService, direct DB, filesystem, or daemon-control mutation paths for workflow truth.",
        "Create Idea, Start Run, Cancel Run, Stage Retry, Steward, runtime-health, session, clone, compare, experiment, and approval write affordances are hidden, removed, or replaced with diagnostic guidance.",
        "MCP remains explicitly allowed for agents and CLI/operator debug workflows outside the macOS UI."
      ],
      "static_guards": [
        "Fail the P031 gate if governed UI imports or instantiates MCPCommandClient, MCPPolicyRuntime, MCP transport, or any MCP tool wrapper.",
        "Fail the P031 gate if governed UI contains GraphQL mutation operations, generated mutation calls, or mutation client types.",
        "Fail the P031 gate if governed UI calls ideas.create, runs.start, runs.cancel, stages.retry, approvals.resolve, steward.run_analysis, session/reset, clone, compare, experiment, runtime-health, or local recovery/execution mutation paths.",
        "Fail the P031 gate if governed UI constructs MCP parameter dictionaries, ActionInvocationIdentity payloads, client_command_id command correlation, command receipt state, or WorkflowCommandInvocationBuilder-style adapters."
      ]
    },
    "schema_matrix": [
      {
        "field": "freshnessState",
        "graphql_type_or_query": "GqlRun, GqlRunProjectionRow, GqlStageExecution, GqlApproval, GqlArtifact, GqlReportMetadata",
        "enum_cases": [
          "live",
          "refreshing",
          "projection_lag",
          "stale",
          "unavailable",
          "unauthorized"
        ],
        "nullability": "non_null",
        "principal_redaction": "operator-visible in production macOS reads; unauthorized receives a denied or unavailable surface, not local fallback detail; observer behavior deferred unless separately authorized",
        "swift_presenter_owner": "WorkflowFreshnessReducer",
        "tests": [
          "operator read",
          "observer redaction where applicable",
          "unauthorized denial",
          "projection_lag disables writes"
        ]
      },
      {
        "field": "disabledReasonCode",
        "graphql_type_or_query": "GqlApproval and optional read-only action/deferred metadata rows on run/stage surfaces",
        "enum_cases": [
          "WRITE_PATH_NOT_AVAILABLE",
          "MANAGED_OUTSIDE_UI",
          "AMBIGUOUS_APPROVAL_IDENTITY",
          "STALE_READ",
          "PROJECTION_LAG",
          "UNAUTHORIZED",
          "UNSUPPORTED_ACTION"
        ],
        "nullability": "nullable when no disabled/deferred explanation is needed",
        "principal_redaction": "operator-visible; unauthorized omitted or generic unavailable; observer diagnostic detail deferred unless separately authorized",
        "swift_presenter_owner": "DisabledReasonPresenter",
        "tests": [
          "operator copy mapping",
          "observer generic reason",
          "unauthorized omission",
          "no Swift status inference"
        ]
      },
      {
        "field": "writePathState",
        "graphql_type_or_query": "GqlApproval",
        "enum_cases": [
          "read_only_diagnostic",
          "write_path_not_available",
          "external_transport_required",
          "hidden"
        ],
        "nullability": "non_null for approval rows",
        "principal_redaction": "operator-visible; unauthorized omitted; observer transport hints deferred unless separately authorized",
        "swift_presenter_owner": "DisabledReasonPresenter and ApprovalDiagnosticPresenter",
        "tests": [
          "no disabled primary button",
          "diagnostic banner rendered",
          "no MCP or mutation call",
          "approval ambiguity diagnostic"
        ]
      },
      {
        "field": "diagnosticId",
        "graphql_type_or_query": "GqlApproval, GqlReportMetadata when needed",
        "enum_cases": [],
        "nullability": "nullable",
        "principal_redaction": "operator-only by default; observer and unauthorized omitted unless an explicit auth/redaction policy lands",
        "swift_presenter_owner": "ApprovalDiagnosticPresenter",
        "tests": [
          "copy affordance present for operator",
          "observer redaction",
          "unauthorized redaction"
        ]
      },
      {
        "field": "payloadAvailabilityState",
        "graphql_type_or_query": "GqlReportMetadata in reports(runID:) and report detail query",
        "enum_cases": [
          "available",
          "metadata_only",
          "payload_deferred",
          "generating",
          "unavailable"
        ],
        "nullability": "non_null",
        "principal_redaction": "visible to authorized operator report readers; unauthorized denied or omitted; observer behavior deferred unless separately authorized",
        "swift_presenter_owner": "PayloadUnavailableReasonPresenter",
        "tests": [
          "reports list indicator",
          "detail metadata state",
          "stable icon width",
          "no raw file probing"
        ]
      },
      {
        "field": "payloadUnavailableReasonCode",
        "graphql_type_or_query": "GqlReportMetadata",
        "enum_cases": [
          "PAYLOAD_DEFERRED_BY_P031",
          "GENERATING",
          "NOT_INDEXED",
          "NOT_AUTHORIZED",
          "NOT_AVAILABLE",
          "UNKNOWN"
        ],
        "nullability": "nullable unless payloadAvailabilityState is not available",
        "principal_redaction": "operator sees precise reason; unauthorized omitted or generic not available; observer precision deferred unless separately authorized",
        "swift_presenter_owner": "PayloadUnavailableReasonPresenter",
        "tests": [
          "operator copy",
          "observer generic copy",
          "targeted read refresh copy",
          "metadata-only dogfood evidence"
        ]
      },
      {
        "field": "serverDebugDetail",
        "graphql_type_or_query": "GqlApproval and GqlReportMetadata diagnostic extension only",
        "enum_cases": [],
        "nullability": "nullable",
        "principal_redaction": "operator-only diagnostic extension; observer and unauthorized omitted",
        "swift_presenter_owner": "DiagnosticDetailsPresenter",
        "tests": [
          "operator visible when present",
          "observer omitted",
          "unauthorized omitted",
          "not used as primary UI copy"
        ]
      }
    ],
    "ui_ownership_inventory": {
      "governed_swift_files": [
        "Chainworks Forge/Views/RunsHomeView.swift",
        "Chainworks Forge/Views/RunDetailPanel.swift if present; otherwise the Run Detail surface inside RunsHomeView.swift",
        "Chainworks Forge/Views/StageDetailView.swift",
        "Chainworks Forge/Views/ApprovalGateView.swift",
        "Chainworks Forge/Views/ArtifactInspectorView.swift",
        "Chainworks Forge/Views/RunArtifactHierarchyView.swift",
        "Chainworks Forge/Views/RunReportView.swift",
        "Chainworks Forge/Views/BlockedRunRecoveryView.swift",
        "Chainworks Forge/Views/RecoverySheet.swift",
        "Chainworks Forge/Views/RunComparisonView.swift",
        "Chainworks Forge/Views/WorkflowInspectorView.swift",
        "Chainworks Forge/Views/WorkflowMapView.swift",
        "Chainworks Forge/Views/DaemonLifecycleSurface.swift",
        "New P031 GraphQL read stores/reducers/presenters under Chainworks Forge/Support or Chainworks Forge/Views"
      ],
      "legacy_only_files_or_types": [
        "Chainworks Forge/Engine/ExecutionService.swift",
        "Chainworks Forge/Engine/RecoveryCoordinator.swift",
        "Chainworks Forge/Engine/RunPlanCompiler.swift",
        "Chainworks Forge/Engine/StageRetryCoordinator.swift",
        "Chainworks Forge/Engine/RunComparisonService.swift",
        "Chainworks Forge/Engine/WorkflowOrchestrator.swift",
        "Chainworks Forge/Engine/MCPPolicyRuntime.swift"
      ],
      "generated_graphql_locations": [
        "Any generated GraphQL operation files introduced for P031 under the app target",
        "Any checked-in .graphql documents introduced for P031",
        "Existing DaemonLifecycleClient GraphQL operations used as the read-client pattern"
      ],
      "forbidden_patterns": [
        "import or instantiate MCPCommandClient, MCPPolicyRuntime, or MCP transport/tool wrappers from governed files",
        "GraphQL mutation operation definitions or generated mutation calls from governed files",
        "calls to ExecutionService, RecoveryCoordinator, RunPlanCompiler, StageRetryCoordinator, RunComparisonService, WorkflowOrchestrator mutation paths from governed files",
        "direct DB writes, raw artifact truth parsing, or filesystem report payload probing from governed files",
        "MCP parameter dictionary construction, ActionInvocationIdentity construction, client_command_id command correlation, or CommandReceiptStore usage from governed files"
      ],
      "explicit_exclusions": [
        "Non-workflow daemon diagnostics that only read lifecycle status",
        "Agent/CLI/MCP code outside the macOS UI target",
        "Legacy rollback code isolated behind CHAINWORKS_THIN_UI_MODE=legacy and unreachable from thin-read/dogfood screens"
      ],
      "machine_readable_contract": {
        "artifact_path": "docs/reference/p031-thin-ui-inventory.json or docs/reference/p031-thin-ui-inventory.yaml",
        "required_before": "Phase 0b exit and any governed Swift screen migration",
        "requirements": [
          "The P031 gate consumes this artifact directly instead of duplicating path lists in prose.",
          "The artifact enumerates governed Swift views, presenters, reducers, stores, checked-in GraphQL documents, generated GraphQL client output locations, legacy exclusions, and forbidden pattern groups.",
          "Adding a governed Swift view, presenter, reducer, store, or GraphQL operation without inventory coverage fails the P031 gate.",
          "Legacy rollback exclusions must be tied to CHAINWORKS_THIN_UI_MODE=legacy and proven unreachable from thin-read and dogfood screens."
        ]
      },
      "gate_owned_globs": [
        "Chainworks Forge/Views/**/*.swift for governed workflow operator screens listed in the inventory",
        "Chainworks Forge/Support/**/*GraphQL*.swift and new P031 read stores/reducers/presenters",
        "Checked-in P031 .graphql documents and generated GraphQL client output directories",
        "Explicit legacy rollback exclusions only when inventory marks them legacy_mode_only"
      ]
    },
    "approval_diagnostic_contract": {
      "decision": "Approval decisions are binary in P031: either hidden/read-only diagnostics only, or enabled by a separately approved non-MCP, non-GraphQL transport outside this proposal. Since no such transport is provided in the input artifacts, P031 renders approval rows as diagnostic-only.",
      "ui_rules": [
        "Do not render a permanently disabled primary Approve or Reject button in thin-read/dogfood.",
        "Render an inline diagnostic banner or terminal-styled callout instead of a disabled primary button.",
        "The diagnostic element states the external workflow from the operator guide when one exists, for example Execute via CLI, and includes copyable run_id, stage_id, approval_id or diagnosticId as available.",
        "If no external workflow is documented, the element says Approval write path unavailable and links to P031-FOLLOWUP-APPROVAL-WRITE-PATH.",
        "Swift must not infer approval write availability from status strings, local recovery state, or local services."
      ],
      "tests": [
        "No approval primary action button in thin-read/dogfood absent an approved transport.",
        "Approval diagnostic banner renders server writePathState and diagnosticId.",
        "Approval UI cannot call MCP, GraphQL mutation, ExecutionService.resolveApproval, or local approval helpers.",
        "Dogfood captures at least one approval diagnostics comprehension observation."
      ]
    },
    "read_refresh_contract": {
      "decision": "The r7 Check Status recovery model is superseded because P031-owned UI issues no commands. The remaining operator-triggered refresh is a targeted GraphQL read refresh.",
      "rules": [
        "Read refresh may refetch the current run, reports list/detail, approvals queue, artifacts, stages, or visible read surface.",
        "Read refresh displays active feedback such as Checking latest data or Refreshing metadata, with a stable indicator and no layout shift.",
        "Read refresh cannot execute MCP, GraphQL mutation, local recovery, daemon-control, or workflow mutation paths.",
        "If the refresh returns no newer projection, the UI keeps last authoritative server values and updates freshness timestamp or stale reason."
      ]
    },
    "source_artifact_governance": {
      "decision": "The run-local r14 artifact is the governing implementation contract for this review cycle until a checked-in implementation addendum or synchronized proposal copies the Phase 0 obligations and explicitly supersedes stale GraphQL+MCP handoff text.",
      "rules": [
        "Implementation handoff must link exactly one governing artifact.",
        "Before Swift screen migration, either synchronize r14 into docs/proposals/031-thin-ui-rewrite-over-projections-and-mcp.md or add a checked-in implementation addendum that copies P043 reconciliation, schema execution contract, UI inventory, approval diagnostic contract, static guards, dogfood obligations, rollback drill criteria, legacy-expiry dependency, follow-up priorities, review-ready packet, explicit disagreement resolution, and the Phase 0 artifact manifest.",
        "The short checked-in proposal must not be the sole implementation contract while it omits r14 Phase 0 obligations.",
        "This refinement task writes only the required run-local artifacts.",
        "The original idea brief MCP-write acceptance criteria are historical context only after r14 and must not be used as implementation acceptance criteria.",
        "The checked-in addendum must name the active revision id and include the GraphQL-only no-UI-write boundary, P043 reconciliation, gate registration, schema execution contract, machine-readable inventory, operator guide contract, rollback drill criteria, and legacy-expiry dependency.",
        "Implementation tickets must link exactly one governing artifact and must not mix stale GraphQL+MCP acceptance text with r14 scope."
      ]
    },
    "schema_contract_execution": {
      "decision": "The r14 schema matrix is an implementation contract, not aspirational UI copy. Phase 0a either adds each field to a server-owned GraphQL read model with redaction tests or explicitly marks the dependent UI state disabled/deferred.",
      "rules": [
        "Every visible P031 workflow field has exactly one named GraphQL source or a disabled/deferred state before Swift migration.",
        "Approval diagnostic copy must be server-owned enough for Swift to render without status-string inference.",
        "Reports use one named read contract: first-class reports(runID:)/GqlReportMetadata if implemented, otherwise an explicitly documented artifact-backed GraphQL metadata projection. Swift must not infer report truth from raw files, filenames, or directory shape.",
        "Diagnostic and debug fields are operator-only by default. Observer visibility is deferred unless a separate authorization policy and redaction tests land before the field ships.",
        "Unauthorized reads return server denial or redacted GraphQL surfaces and never fall back to local storage."
      ],
      "exit_criteria": [
        "Schema fields, enum cases, and nullability are present in the executable GraphQL contract or listed in disabled/deferred UI states.",
        "Operator and unauthorized redaction tests cover diagnostic, payload, and debug fields.",
        "Any observer-visible behavior is explicitly introduced by an auth/redaction change outside the default P031 operator-only contract."
      ]
    },
    "operator_write_path_contract": {
      "decision": "The operator write-path guide is a versioned contract artifact consumed by UI diagnostics and dogfood evidence, not only release documentation.",
      "artifact_path": "docs/reference/p031-operator-write-path-guide.json is preferred for gate consumption; docs/reference/p031-operator-write-path-guide.md may be generated from it for operators.",
      "minimum_viable_row_schema": [
        "removed_control_id",
        "removed_control_label",
        "external_workflow_kind: MCP terminal, CLI, automation, non-P031 UI, or temporarily_unavailable",
        "external_workflow_name_or_tool",
        "required_identifiers exposed by GraphQL/UI copy affordances",
        "minimum_parameter_shape or unavailable_reason",
        "expected_success_output or follow_up_id",
        "operator_notes and validation_status"
      ],
      "rules": [
        "Every removed P031 UI write control has one row before Phase 0d exit.",
        "Rows may be minimum viable before dogfood: tool/workflow name, required identifiers, expected output, and unavailable follow-up are sufficient even if polished step-by-step recipes come later.",
        "The UI may link to the guide and copy identifiers, but it must not construct MCP payloads or execute the external workflow.",
        "Dogfood evidence includes at least one approval diagnostic and one removed-control workflow where copied identifiers match the guide row.",
        "The JSON guide is the gate-consumed source of truth; markdown may be generated for operator readability.",
        "Guide rows with external_workflow_kind temporarily_unavailable must include a follow_up_id and operator-safe unavailable copy."
      ]
    },
    "phase_0_artifact_manifest": {
      "decision": "Phase 0 exits through one manifest that lists the concrete artifacts reviewers and implementers must use. The manifest prevents r14 from being treated as another prose-only proposal and gives the P031 gate a single place to verify handoff completeness.",
      "preferred_artifact_path": "docs/reference/p031-phase-0-artifact-manifest.json",
      "required_before": "Phase 0d exit and dogfood start; migration-only rows are required before the affected Swift screen migration starts.",
      "required_entries": [
        "governing_contract: checked-in r14 synchronized proposal or implementation addendum that supersedes stale GraphQL+MCP text",
        "p043_reconciliation_evidence: reference and gate language no longer assigning command-completion, command receipts, MCP control, or command correlation to P031-owned UI",
        "p031_gate_evidence: proposal-031 and p031 aliases registered, documented, and failing closed for UI MCP, GraphQL mutations, command plumbing, local writes, raw truth probing, and enabled removed controls",
        "ui_inventory: gate-consumed P031 UI inventory JSON/YAML with governed paths, generated GraphQL locations, forbidden patterns, and legacy exclusions",
        "schema_decision_record: every visible field mapped to one executable GraphQL source or explicit disabled/deferred state with redaction tests",
        "operator_write_path_guide: gate-consumed JSON guide with one row per removed control and validation status",
        "rollback_evidence: rollback drill result or dated release-owner waiver",
        "report_payload_priority_decision: default P0 or evidence-backed downgrade",
        "dogfood_signoff_template: Phase 3 checklist including additional-evidence trigger review and critical write-path readiness or waiver status"
      ],
      "gate_rules": [
        "The P031 gate may fail Phase 0d if the manifest omits any required entry or references stale/unversioned artifacts.",
        "Manifest entries must include artifact path, revision or commit identifier when available, owner role, validation_status, and blocking_phase.",
        "Rows may be pending during early Phase 0 only when their blocking_phase has not arrived; they cannot be pending at or after their stated blocking phase."
      ]
    },
    "implementation_readiness_checklist": {
      "required_before_swift_screen_migration": [
        "Checked-in r14 implementation addendum or synchronized proposal supersedes stale GraphQL+MCP handoff text.",
        "Phase 0 artifact manifest has entries for the governing contract, P043 reconciliation evidence, P031 gate evidence, UI inventory, and schema decision record.",
        "scripts/test-gate.sh registers proposal-031 and p031 aliases and docs/reference/test-gates.md documents them.",
        "P043/P031 validation no longer assigns command-completion refresh, command receipts, MCP command-control, or command-correlation behavior to P031-owned UI.",
        "Machine-readable P031 UI inventory exists and is consumed by the P031 gate.",
        "Every P031 visible field has an executable GraphQL read source or an explicit disabled/deferred UI state.",
        "Diagnostic and debug read fields are operator-only by default, with unauthorized denial/redaction tests.",
        "Legacy rollback exclusions are reachable only in legacy mode and unreachable from thin-read/dogfood screens."
      ],
      "required_before_dogfood": [
        "Operator write-path guide exists at the agreed artifact path and has a row for every removed control.",
        "At least one approval diagnostic and one non-approval removed-control workflow are validated end-to-end against copied UI identifiers.",
        "Rollback drill meets quantitative criteria or has a dated release-owner waiver.",
        "Report payload follow-up priority is recorded as default P0 or downgraded with operator usage evidence.",
        "Phase 0 artifact manifest has no pending entries whose blocking_phase is Phase 0d or dogfood start.",
        "First-run read-only orientation, approval diagnostics, report payload indicators, and Syncing accessibility checks are complete."
      ],
      "required_before_legacy_removal": [
        "Critical write-path readiness is satisfied by a merged gate-green follow-up for approval resolution and at least one run control path, or a release-owner waiver explicitly accepts the gap.",
        "Phase 0 artifact manifest links Phase 3 sign-off evidence and critical_write_path_readiness or waiver status.",
        "Waiver, if used, includes a dated write-restoration deadline and a dated legacy-removal extension decision.",
        "Phase 3 sign-off reviews all dogfood additional-evidence triggers."
      ]
    },
    "critical_write_path_readiness": {
      "definition": "For legacy expiry purposes, critical write-path readiness means a merged, reviewed, and gate-green follow-up restores or replaces the operator ability to resolve approvals and perform at least one run-control workflow needed for full-mvp-live operations, without using P031-owned UI MCP or GraphQL mutations.",
      "minimum_without_waiver": [
        "Approval resolution path is defined, merged, gate-green, and documented for operators.",
        "At least one run-control path needed by full-mvp-live operations, such as start/cancel/retry depending on current operational criticality, is defined, merged, gate-green, and documented.",
        "The external/operator guide has been updated so UI diagnostic identifiers match the restored path inputs.",
        "Release owner records that remaining unavailable controls are non-critical for the next release window or covered by explicit follow-up dates."
      ],
      "waiver_requirements": [
        "Names the unavailable critical write path.",
        "States why removing legacy rollback is acceptable despite the gap.",
        "Sets a hard write-restoration deadline.",
        "Sets or extends a dated legacy-removal deadline.",
        "Is attached to Phase 3 sign-off evidence."
      ]
    }
  },
  "ux_ui_notes": {
    "information_hierarchy": [
      "Runs Home remains the entry screen with run list, status, selection, filters, freshness, first-run read-only orientation, and drill-in.",
      "Run Detail keeps status summary, stage progress, approvals, artifacts, report metadata, and recovery/evidence context. Cancel/retry/recovery write controls are removed or replaced with diagnostic orientation.",
      "Reports list rows show payload availability before drill-in.",
      "Approvals queue is a read/diagnostic surface. It uses a diagnostic banner/callout, not disabled primary approval buttons, unless a separately approved transport exists."
    ],
    "syncing_placement": {
      "runs_home_rows": "Fixed transparent slot immediately after StatusCapsule.",
      "stage_rows": "Fixed transparent slot immediately after StatusCapsule.",
      "run_detail": "Fixed header slot at the top trailing edge of the Run Detail header, aligned with the status summary row.",
      "artifacts_view": "Fixed toolbar/header slot at the top trailing edge of the Artifacts inspector header.",
      "reports_view": "Fixed list header slot for list refresh and per-row reserved payload/status slot for payload state.",
      "approvals_queue": "Fixed header slot for queue refresh; row-level diagnostics do not shift primary text columns."
    },
    "report_payload_indicators": {
      "stable_width": "Each Reports list row reserves a 96 point trailing payload-status slot.",
      "text_rules": "Use title-case labels. Truncate middle only after 96 points. Do not wrap in compact rows.",
      "states": [
        {
          "state": "available",
          "sf_symbol": "doc.text.fill",
          "label": "Payload"
        },
        {
          "state": "metadata_only",
          "sf_symbol": "doc.text",
          "label": "Metadata"
        },
        {
          "state": "payload_deferred",
          "sf_symbol": "clock.badge.exclamationmark",
          "label": "Deferred"
        },
        {
          "state": "generating",
          "sf_symbol": "arrow.triangle.2.circlepath",
          "label": "Generating"
        },
        {
          "state": "unavailable",
          "sf_symbol": "exclamationmark.triangle",
          "label": "Unavailable"
        }
      ]
    },
    "approval_diagnostic_ui": [
      "Use an inline banner or terminal-styled callout for disabled approval decision state.",
      "Do not show a disabled primary Approve or Reject button as the main visual element.",
      "Show Execute via CLI only when the operator write-path guide names CLI as the approved external workflow.",
      "Always include a copy affordance for diagnosticId or available run/stage/approval identifiers.",
      "VoiceOver label must describe the state as diagnostic guidance, not as an unavailable button."
    ],
    "first_run_orientation": [
      "Dogfood mode shows a dismissible Runs Home banner on first thin-read launch.",
      "Banner copy states that this build is read-only and control actions are performed through the operator guide's external workflow.",
      "Banner includes a link or affordance to open the operator guide."
    ],
    "copy_rules": [
      "Disabled controls name the server condition in operator terms.",
      "Write-removed copy must not say Retry, Reissue, Start, Cancel, Approve, Reject, or Create as available in-app commands.",
      "Report metadata-only limitation is communicated before dogfood and visible in the Reports list and report viewer.",
      "Read refresh copy confirms active work: Checking latest data, Refreshing reports, or Updating approvals."
    ],
    "approval_diagnostic_visual_treatment": {
      "contrast_rule": "Approval diagnostic banners/callouts use an informational diagnostic treatment, not the primary error-alert treatment. They must be visually distinct from destructive errors to avoid alarm fatigue.",
      "terminal_callout_typography": "If terminal-styled copy is used, monospace text size matches surrounding body text density and does not create a separate oversized visual block.",
      "allowed_tones": [
        "diagnostic",
        "managed outside UI",
        "write path unavailable"
      ],
      "disallowed_tones": [
        "critical error unless the server reports an actual error",
        "disabled primary action tease"
      ]
    },
    "syncing_motion": {
      "rule": "Syncing indicators in fixed transparent slots use subtle, low-amplitude motion only while an active refresh or projection_lag state is present.",
      "constraints": [
        "No per-field spinners",
        "No layout shift",
        "No motion in stable live state",
        "Honor reduced-motion preferences"
      ]
    },
    "first_run_banner_dismissal": {
      "rule": "The first-run orientation banner has a clear trailing dismiss control aligned with the Runs Home header rhythm.",
      "persistence": "Dismissal is local presentation state and must not affect server workflow truth.",
      "guide_link": "The operator guide affordance is directly clickable or copyable."
    },
    "report_empty_states": [
      "When no report metadata is available yet, Reports shows an empty state that names metadata generation/indexing status from GraphQL if available.",
      "The empty state includes targeted read refresh only; it does not probe raw report files.",
      "VoiceOver reads report payload state as a complete sentence, for example Report payload is currently generating."
    ]
  },
  "implementation_plan": [
    {
      "phase": "Phase 0a - Dependency/reference reconciliation, GraphQL read contract, source governance, and UI boundary",
      "owner": "P031 Rust control-plane owner and P031 macOS thin UI owner",
      "size_estimate": "1.5-2.5 implementation days",
      "required_work": [
        "Reconcile P043/P031 reference and gate language so command-completion refresh, command receipts, and MCP command-control are explicitly outside P031-owned UI.",
        "Confirm every P031 visible field has a GraphQL query/subscription/polling read path or mark the surface disabled/deferred.",
        "Add or confirm schema fields from the schema_matrix.",
        "Mark this run-local r14 artifact as governing for Phase 0 or synchronize/copy obligations into a checked-in implementation addendum before implementation handoff.",
        "Add operator/observer/unauthorized redaction tests for metadata, diagnostic, payload, and debug fields.",
        "Convert the schema matrix into executable GraphQL fields or explicit disabled/deferred UI states before affected Swift screens migrate.",
        "Narrow diagnostic/debug authorization to operator-only by default and defer observer semantics unless a separate auth policy lands.",
        "Check in the governing r14 implementation addendum or synchronized proposal and mark stale GraphQL+MCP idea-brief acceptance criteria as superseded.",
        "Create the Phase 0 artifact manifest and seed it with governing contract, P043 reconciliation, P031 gate, UI inventory, schema decision, operator guide, rollback, report-priority, and sign-off entries."
      ],
    "exit_criteria": "P043/P031 contract conflict is resolved, server read schema and redaction tests are merged, the artifact manifest is seeded, and one governing implementation artifact is linked before Swift screen migration starts."
    },
    {
      "phase": "Phase 0b - UI ownership inventory and write-path removal guards",
      "owner": "P031 macOS thin UI owner",
      "size_estimate": "1-1.5 implementation days",
      "required_work": [
        "Check in or gate-load the P031-owned UI file/type inventory.",
        "Add static guards for UI MCP imports/calls, GraphQL mutations, local recovery/execution mutation services, command receipts, client_command_id command correlation, and identity-to-MCP mapping.",
        "Add one negative test per removed write control: create, start, cancel, retry, steward, runtime, session, clone, compare, experiment, and approval decision.",
        "Isolate legacy rollback code behind legacy mode and prove it is unreachable from thin-read/dogfood screens.",
        "Check in a machine-readable P031 UI inventory artifact consumed by the P031 gate.",
        "Ensure adding a governed Swift view or GraphQL operation without inventory coverage fails closed."
      ],
      "exit_criteria": "P031 gate fails closed for UI MCP usage, GraphQL mutations, command plumbing, local write fallback, and out-of-inventory governed surfaces."
    },
    {
      "phase": "Phase 0c - Swift GraphQL-only boundary and test doubles",
      "owner": "P031 macOS thin UI owner",
      "size_estimate": "1-2 implementation days",
      "required_work": [
        "Introduce GraphQL read clients, subscription clients, stores, reducers, presenters, P043 freshness constants, and read-only test doubles.",
        "Register proposal-031 and p031 aliases in scripts/test-gate.sh.",
        "Add reducer tests for freshness, targeted read refresh, disabled/deferred reasons, projection lag, authorization, report payload availability, approval diagnostics, and first-run orientation."
      ],
      "exit_criteria": "Thin-read mode fails closed unless GraphQL read contracts and UI write-removal guards are green."
    },
    {
      "phase": "Phase 0d - Operator guide, UX sign-off, rollback drill, and freshness baseline",
      "owner": "P031 release owner",
      "size_estimate": "1-1.5 implementation days",
      "required_work": [
        "Publish an operator write-path guide mapping every removed control to an external workflow or temporarily unavailable follow-up reference.",
        "Complete UX review of Syncing placement, targeted read-refresh feedback, approval diagnostics, first-run orientation, Reports payload indicators, compact density, and accessibility hints.",
        "Measure representative GraphQL projection freshness p50/p95 under full-mvp-live or closest available workflow.",
        "Execute a controlled rollback drill from dogfood/thin-read mode to legacy mode, or attach a release-owner waiver explaining why the drill is not possible before dogfood.",
        "Re-estimate Phases 1-3 with Phase 0 findings and record go/no-go for screen migration.",
        "Publish the operator write-path guide at the agreed artifact path using the minimum-viable row schema for every removed control.",
        "Validate at least one approval diagnostic and one removed-control workflow against copied identifiers before dogfood.",
        "Define rollback drill pass/fail evidence: thin-read/dogfood to legacy mode within 60 seconds, no projection data loss or conflicting stale truth visible after rollback, and operator confirmation that legacy surfaces are functional.",
        "Decide P031-FOLLOWUP-REPORT-PAYLOAD priority from operator workflow data; default to P0 unless current usage or dogfood evidence shows report payload inspection is low-frequency and metadata-only is acceptable.",
        "Add the Phase 3 sign-off checklist that explicitly reviews all additional-evidence triggers."
      ],
      "exit_criteria": "Operator guide, UX sign-off, GraphQL freshness measurement, rollback drill result or waiver, and Phase 1 go/no-go are attached before screen migration starts."
    },
    {
      "phase": "Phase 1 - Read-only thin screens",
      "owner": "P031 macOS thin UI owner",
      "size_estimate": "3-5 implementation days",
      "required_work": [
        "Add GraphQL-backed stores for Runs Home, Run Detail, stages, approvals, artifacts, report metadata, and daemon lifecycle banner.",
        "Render freshness and projection lag with the shared Syncing pattern and detail-view header slots.",
        "Add active targeted read-refresh feedback for relevant read surfaces.",
        "Add Reports list payload availability indicators using the specified symbols and stable slot.",
        "Replace approval primary buttons with diagnostic banner/callout unless a separate approved transport exists.",
        "Add first-run dogfood orientation banner on Runs Home.",
        "Keep legacy path only in legacy mode for rollback."
      ],
      "exit_criteria": "Read surfaces render from GraphQL or are explicitly disabled/deferred; no P031-owned thin-mode screen reads or writes workflow truth locally."
    },
    {
      "phase": "Phase 2 - Local truth and write-control teardown",
      "owner": "P031 macOS thin UI owner",
      "size_estimate": "1-2 implementation days",
      "required_work": [
        "Remove direct local service calls from P031-owned screens.",
        "Remove SwiftData @Query production truth from P031-owned screens.",
        "Remove MCP command clients, command receipt code paths, identity-to-MCP adapters, and GraphQL mutation paths from P031-owned UI code if any exist.",
        "Retain only presentation state, read-refresh state, and server-derived caches."
      ],
      "exit_criteria": "No P031-owned production screen can decide or mutate workflow truth locally, through MCP, or through GraphQL mutation."
    },
    {
      "phase": "Phase 3 - Dogfood, release, and flag removal",
      "owner": "P031 release owner",
      "size_estimate": "1-2 implementation days",
      "required_work": [
        "Run same-tree prerequisite gates for P027, P041, P042, and reconciled P043.",
        "Run proposal-031 and p031 gates.",
        "Use the operator write-path guide during dogfood.",
        "Run two full-mvp-live dogfood runs in GraphQL-only thin UI mode for the assumed small internal operator population.",
        "Capture structured operator workflow-completion notes after each run.",
        "Capture degraded-state recovery and approval-queue diagnostic evidence at least once across the dogfood set.",
        "Capture targeted read-refresh feedback, Reports list payload indicators, report metadata inspection, accessibility spot check, GraphQL freshness, projection correctness, rollback drill result or waiver, and rollback readiness evidence.",
        "P031 release owner accepts the evidence package and initiates flag removal.",
        "Before sign-off, review each additional-evidence trigger and record whether it fired, what extra evidence was collected, or why it did not apply.",
        "Do not remove legacy rollback code unless write-path readiness is available through a merged gate-green follow-up or the release owner signs a dated waiver with a hard write-restoration deadline."
      ],
      "exit_criteria": "Release handoff includes gate results, dogfood evidence, operator outcome notes, sign-off, hold/rollback status, metrics, and a legacy removal deadline within 10 business days of flag removal unless extended in writing by the release owner."
    }
  ],
  "effort_estimate": {
    "phase_0_total": "4.5-7.5 implementation days",
    "phase_1": "3-5 implementation days",
    "phase_2": "1-2 implementation days",
    "phase_3": "1-2 implementation days",
    "total": "9.5-16.5 implementation days",
    "assumption": "Estimates assume the GraphQL-only scope, existing projection compatibility after P043 reconciliation, and mostly sequential work for a single primary implementer. Phase 0d includes re-estimation before screen migration."
  },
  "rollout": {
    "mode_enum": [
      {
        "mode": "legacy",
        "reads": "Legacy UI read path remains active only for rollback.",
        "writes": "Legacy local writes may exist only before thin mode migration and must not coexist with GraphQL-only thin mode.",
        "use": "Emergency rollback or pre-cutover default."
      },
      {
        "mode": "thin-read",
        "reads": "GraphQL read models are active.",
        "writes": "P031 state-changing actions are removed, hidden, or diagnostic-only.",
        "use": "P031 dogfood and release mode."
      },
      {
        "mode": "dogfood",
        "reads": "GraphQL read models are active.",
        "writes": "Same as thin-read; no UI MCP and no GraphQL mutations.",
        "use": "Two-run full-mvp-live dogfood and sign-off for the assumed small internal operator population."
      }
    ],
    "operator_write_path_guide": {
      "required_before": "Phase 0d exit and dogfood start",
      "coverage_target": "100 percent of removed P031 UI write controls mapped",
      "mapping_rules": [
        "For each removed control, name the current external workflow: MCP terminal, CLI, automation, non-P031 UI, or temporarily unavailable.",
        "If temporarily unavailable, include the follow-up proposal id.",
        "If an external workflow exists, include the minimum identifiers the UI should expose for copy: run_id, stage_id, approval_id, workflow_id, or diagnosticId.",
        "The guide is referenced by the first-run orientation banner and approval diagnostic banners.",
        "Minimum viable rows must include workflow/tool name, required identifiers, minimum parameter shape, expected success output, and validation_status.",
        "The guide must explicitly state when polished step-by-step recipes are pending but minimum invocation data is validated.",
        "The JSON guide is the gate-consumed source of truth; markdown may be generated for operator readability.",
        "Rows marked temporarily_unavailable must include follow_up_id and operator-safe unavailable copy."
      ],
      "artifact_path": "docs/reference/p031-operator-write-path-guide.json is preferred for gate consumption; docs/reference/p031-operator-write-path-guide.md may be generated from it for operators.",
      "minimum_viable_row_schema": [
        "removed_control_id",
        "removed_control_label",
        "external_workflow_kind: MCP terminal, CLI, automation, non-P031 UI, or temporarily_unavailable",
        "external_workflow_name_or_tool",
        "required_identifiers exposed by GraphQL/UI copy affordances",
        "minimum_parameter_shape or unavailable_reason",
        "expected_success_output or follow_up_id",
        "operator_notes and validation_status"
      ]
    },
    "dogfood_population": {
      "assumption": "Two full-mvp-live runs are calibrated for a small internal dogfood group of one to three operators using the same local operator workflow family.",
      "additional_evidence_triggers": [
        "More than three distinct operators use the thin UI before legacy expiry.",
        "Dogfood covers more than one workflow family beyond full-mvp-live.",
        "A new approval shape, degraded daemon state, report payload state, or projection lag failure appears that was not exercised in the first two runs.",
        "The release owner expands availability beyond the initial internal group."
      ],
      "required_edge_coverage": [
        "At least one observed degraded-state recovery, such as daemon restart or projection lag.",
        "At least one approval-queue encounter with diagnostic-only decision guidance.",
        "Reports list payload status observed before report detail drill-in."
      ]
    },
    "hold_criteria": [
      "Any prerequisite P027/P041/P042/reconciled-P043 gate is red on the same tree.",
      "P043/P031 reference language still assigns command-completion refresh, command receipts, or MCP control behavior to P031-owned UI.",
      "Run-local r14 governance or checked-in addendum is not accepted before implementation handoff.",
      "Any governed UI code imports or invokes MCP.",
      "Any governed UI code defines or executes GraphQL mutation.",
      "Any governed UI code calls local recovery/execution mutation paths.",
      "Any governed UI constructs MCP parameter dictionaries, command receipt state, client_command_id command correlation, or ActionInvocationIdentity payloads for UI writes.",
      "Create Idea, Start Run, Cancel Run, Stage Retry, Steward, runtime-health, session, clone, compare, experiment, or approval write control remains enabled in a governed screen without a separate approved transport.",
      "Reports list lacks payload availability status.",
      "Operator write-path guide is missing before dogfood.",
      "Unauthorized, stale, unavailable, projection_lag, targeted_read_refresh, report payload state, approval diagnostics, or write_path_removed reducer tests fail.",
      "Machine-readable UI inventory artifact is missing or not consumed by the P031 gate before governed screen migration.",
      "Operator write-path guide lacks minimum viable rows for any removed control before dogfood.",
      "Rollback drill has no quantitative pass/fail result or waiver before dogfood evidence acceptance.",
      "Phase 0d has not recorded report payload follow-up priority from usage evidence or defaulted it to P0.",
      "Phase 3 sign-off does not explicitly review additional-evidence triggers."
    ],
    "rollback_criteria": [
      "GraphQL read model diverges from server projection/canonical truth in parity checks or dogfood evidence.",
      "Daemon lifecycle causes repeated unavailable state on normal app launch.",
      "An operator can trigger a client-owned local mutation, MCP command, or GraphQL mutation from a governed screen.",
      "The app is continuously unavailable for two minutes in normal dogfood conditions.",
      "Targeted read refresh fails to update freshness state or visibly complete under normal daemon conditions.",
      "A dogfood run is blocked because write-path guidance is missing or approval diagnostics are not understood.",
      "Rollback drill exceeds 60 seconds to visible legacy mode or leaves stale/conflicting truth visible as authoritative.",
      "Copied identifiers from UI diagnostics do not match the external workflow guide during dogfood validation."
    ],
    "rollback_action": "Downgrade to legacy mode and disable affected thin UI surfaces. No rollback mode may write conflicting workflow truth while GraphQL-only thin mode is active.",
    "legacy_expiry": "Legacy rollback code is removed within 10 business days of flag removal only if critical_write_path_readiness is satisfied or the P031 release owner signs a dated waiver. A follow-up proposal being drafted is not sufficient. Without readiness or waiver, legacy expiry is extended in writing with a new dated removal deadline.",
    "rollback_drill_success_criteria": {
      "required_before": "Phase 0d exit unless release-owner waiver is attached",
      "max_time_to_legacy_mode": "60 seconds from rollback trigger to visible legacy mode confirmation under normal local dogfood conditions",
      "consistency_assertions": [
        "No projection data loss is caused by mode switch.",
        "No stale GraphQL-only truth remains visible as authoritative after rollback.",
        "Legacy surfaces are functional for read and any pre-existing legacy write surfaces under the rollback policy."
      ],
      "operator_confirmation": "A participating operator confirms the legacy Runs Home/Run Detail path is usable after rollback.",
      "failure_result": "Failure blocks dogfood sign-off unless release owner attaches a dated waiver and mitigation."
    },
    "phase_3_signoff_checklist": [
      "Confirm no additional-evidence trigger fired, or attach the extra evidence collected for each fired trigger.",
      "Confirm operator write-path guide rows were validated against copied UI identifiers.",
      "Confirm rollback drill met quantitative criteria or waiver is attached.",
      "Confirm report payload follow-up priority decision is recorded with usage evidence or default P0 priority.",
      "Confirm legacy expiry dependency is satisfied by write-path readiness or dated release-owner waiver.",
      "Confirm critical_write_path_readiness is satisfied by merged gate-green follow-up work, not only a draft proposal.",
      "If using a waiver, confirm it names unavailable paths, accepts the operator gap, sets a hard write-restoration deadline, and extends or confirms the legacy-removal deadline."
    ],
    "legacy_expiry_dependency": {
      "definition": "For legacy expiry purposes, critical write-path readiness means a merged, reviewed, and gate-green follow-up restores or replaces the operator ability to resolve approvals and perform at least one run-control workflow needed for full-mvp-live operations, without using P031-owned UI MCP or GraphQL mutations.",
      "minimum_without_waiver": [
        "Approval resolution path is defined, merged, gate-green, and documented for operators.",
        "At least one run-control path needed by full-mvp-live operations, such as start/cancel/retry depending on current operational criticality, is defined, merged, gate-green, and documented.",
        "The external/operator guide has been updated so UI diagnostic identifiers match the restored path inputs.",
        "Release owner records that remaining unavailable controls are non-critical for the next release window or covered by explicit follow-up dates."
      ],
      "waiver_requirements": [
        "Names the unavailable critical write path.",
        "States why removing legacy rollback is acceptable despite the gap.",
        "Sets a hard write-restoration deadline.",
        "Sets or extends a dated legacy-removal deadline.",
        "Is attached to Phase 3 sign-off evidence."
      ]
    }
  },
  "metrics": [
    {
      "metric": "GraphQL read ownership coverage",
      "target": "100 percent of P031-owned visible workflow fields source from named GraphQL read models or are disabled/deferred."
    },
    {
      "metric": "P043/P031 contract reconciliation",
      "target": "0 P031 UI command-completion, command receipt, or MCP control obligations remain in reconciled reference or gate language."
    },
    {
      "metric": "GraphQL projection correctness",
      "target": "0 parity divergences between GraphQL read model fields and engine authoritative state observed in P031 gate runs or dogfood."
    },
    {
      "metric": "UI MCP usage",
      "target": "0 MCP imports, clients, tool calls, wrappers, ActionInvocationIdentity builders, command receipt stores, client_command_id command correlation, or MCP parameter serializers reachable from governed macOS UI code."
    },
    {
      "metric": "GraphQL mutation usage",
      "target": "0 GraphQL mutations defined or invoked by governed macOS UI code."
    },
    {
      "metric": "Removed write controls",
      "target": "0 enabled create/start/cancel/retry/steward/runtime/session/clone/compare/experiment/approval-write controls in governed screens unless approval has a separately approved transport outside P031."
    },
    {
      "metric": "Operator write-path documentation coverage",
      "target": "100 percent of removed P031 UI write controls mapped to an external workflow or unavailable follow-up before dogfood."
    },
    {
      "metric": "Operator workflow viability",
      "target": "2 of 2 dogfood runs include operator workflow-completion notes stating whether the run completed end-to-end without improvised workarounds."
    },
    {
      "metric": "Approval diagnostic comprehension",
      "target": "At least one approval-queue encounter includes observation that the operator understood diagnostic-only guidance without external help."
    },
    {
      "metric": "Dogfood edge coverage",
      "target": "Dogfood evidence covers degraded-state recovery, approval diagnostics, Reports payload status, targeted read refresh, and metadata-only report inspection."
    },
    {
      "metric": "GraphQL freshness baseline",
      "target": "Dogfood evidence includes p50 and p95 projection freshness for visible read surfaces."
    },
    {
      "metric": "Targeted read-refresh feedback",
      "target": "100 percent of operator-triggered read refreshes visibly enter an active refresh state and complete without invoking MCP, GraphQL mutation, or local mutation paths."
    },
    {
      "metric": "Report payload visibility",
      "target": "100 percent of Reports list rows show payload availability or metadata-only/deferred state before drill-in."
    },
    {
      "metric": "Time to usable Runs Home",
      "target": "During dogfood, 95 percent of foreground launches reach live or stale-but-visible Runs Home state within 2 seconds under normal daemon conditions."
    },
    {
      "metric": "Syncing visual stability",
      "target": "0 per-field spinners and 0 layout shifts from incremental refreshing, targeted read refresh, or projection_lag in Runs Home, Run Detail, Artifacts, Reports, and Approvals."
    },
    {
      "metric": "Disabled reason accessibility",
      "target": "100 percent of visible diagnostic/disabled P031 states expose localized operator-friendly text as VoiceOver-accessible hints."
    },
    {
      "metric": "Operator guide contract completeness",
      "target": "100 percent of removed controls have gate-consumed JSON guide rows with workflow/tool name or follow_up_id, required identifiers, minimum parameter shape or unavailable reason, expected success output, and validation_status before dogfood."
    },
    {
      "metric": "Rollback drill readiness",
      "target": "Rollback drill completes thin-read/dogfood to visible legacy mode within 60 seconds, passes state consistency assertions, and records operator confirmation before dogfood sign-off or has a dated waiver."
    },
    {
      "metric": "Legacy expiry safety",
      "target": "0 legacy rollback removals occur before critical_write_path_readiness is merged and gate-green or before release-owner waiver sets a dated write-restoration deadline and legacy-removal decision."
    },
    {
      "metric": "Report payload priority decision",
      "target": "Phase 0d records either P0 report-payload follow-up priority or usage evidence proving metadata-only report inspection is low-frequency and acceptable."
    },
    {
      "metric": "Phase 3 trigger review",
      "target": "100 percent of dogfood additional-evidence triggers are explicitly reviewed in the sign-off checklist."
    },
  {
    "metric": "Phase 0 implementation readiness",
    "target": "100 percent of architecture.implementation_readiness_checklist items are satisfied for the relevant phase before Swift migration, dogfood, and legacy removal."
  },
  {
    "metric": "Phase 0 artifact manifest completeness",
    "target": "100 percent of required manifest entries have artifact path, owner role, validation_status, and blocking_phase, with no stale or pending rows at or after the relevant blocking phase."
  }
  ],
  "validation": {
    "gate_requirements": [
      "proposal-031 and p031 are registered in scripts/test-gate.sh and documented in docs/reference/test-gates.md.",
      "Gate validates P043/P031 reconciliation, GraphQL read schema coverage, schema_matrix fields, read-only metadata redaction, P043 shared constants, report metadata-only behavior, Reports list payload status, source-governance decision, UI ownership inventory, and local-truth static guards.",
      "Gate fails if governed UI imports or invokes MCP clients/tools/wrappers.",
      "Gate fails if governed UI defines or executes GraphQL mutations.",
      "Gate fails if governed UI calls local recovery/execution mutation paths.",
      "Gate fails if governed UI constructs command receipt state, client_command_id command correlation, identity-to-MCP mapping, or MCP parameter dictionaries for UI writes.",
      "Gate fails if create/start/cancel/retry/steward/runtime/session/clone/compare/experiment/approval-write controls remain enabled without a separate approved transport.",
      "Gate proves targeted read refresh uses GraphQL reads only and shows active refresh feedback.",
      "Gate includes one negative test per removed write control.",
      "Gate consumes the machine-readable P031 UI inventory and fails closed on unlisted governed views, presenters, reducers, stores, GraphQL documents, or generated client locations.",
    "Gate validates that the checked-in r14/addendum supersedes stale GraphQL+MCP handoff text before screen migration.",
      "Gate validates operator-only default redaction for diagnostic/debug fields unless an explicit auth expansion is present.",
    "Gate validates the Phase 0 implementation-readiness checklist artifacts exist before corresponding migration or dogfood milestones.",
    "Gate validates the Phase 0 artifact manifest exists, references versioned artifacts, and has no pending rows at or after each row's blocking phase.",
    "Gate validates the operator write-path guide JSON covers every removed control and every temporarily unavailable row has follow_up_id plus unavailable copy.",
      "Gate rejects legacy expiry evidence that points only to a draft follow-up rather than critical_write_path_readiness or waiver."
    ],
    "dogfood_evidence_minimum": [
      "Two full-mvp-live runs in dogfood mode for the assumed one-to-three-operator internal dogfood group.",
      "Additional evidence if dogfood population or workflow diversity exceeds the stated assumption.",
      "Operator write-path guide used during dogfood.",
      "Per-run operator workflow-completion note from at least one participating operator.",
      "Approval queue readback plus diagnostic-only guidance comprehension observation.",
      "At least one degraded-state recovery, such as daemon restart or projection lag.",
      "Targeted read-refresh active feedback evidence.",
      "Reports list payload availability indicator evidence.",
      "Report metadata inspection evidence.",
      "Pre-dogfood operator communication that full report payload rendering and write controls are deferred/outside P031 UI.",
      "Accessibility spot check for disabled reasons, read-refresh labels, report payload status, approval diagnostics, and compact status indicators.",
      "Projection correctness and GraphQL freshness evidence.",
      "Rollback drill result or release-owner waiver.",
      "Rollback readiness evidence.",
      "Operator write-path guide minimum viable rows validated against copied UI identifiers for at least one approval diagnostic and one removed-control workflow.",
      "Rollback drill quantitative result: time-to-legacy-mode, state consistency assertions, and operator confirmation, or dated release-owner waiver.",
      "Report payload follow-up priority decision with usage evidence or default P0 classification.",
      "Phase 3 sign-off checklist reviewing each additional-evidence trigger.",
    "Phase 0 implementation-readiness checklist snapshot for the dogfood milestone.",
    "Phase 0 artifact manifest snapshot showing no pending dogfood-blocking rows.",
    "Critical write-path readiness or waiver status recorded even if legacy removal is not yet happening."
    ]
  },
  "risks_and_mitigations": [
    {
      "risk": "P043 reference language keeps imposing command/control obligations on P031 UI.",
      "impact": "Implementers can satisfy stale dependency docs while violating the GraphQL-only proposal.",
      "mitigation": "P043/P031 reconciliation is a Phase 0a exit criterion and a gate requirement before Swift screen migration."
    },
    {
      "risk": "UI grows a hidden MCP control path despite the GraphQL-only contract.",
      "impact": "The app becomes a second control surface and violates the operator architecture decision.",
      "mitigation": "P031 gate has static guards for MCP imports/calls, identity-to-MCP mapping, command receipt state, client_command_id command correlation, and command payload construction."
    },
    {
      "risk": "GraphQL mutations are added to compensate for removed MCP UI writes.",
      "impact": "The UI still mutates workflow truth, only through another transport.",
      "mitigation": "P031 gate fails on GraphQL mutation definitions/invocations in governed UI code."
    },
    {
      "risk": "Static guards miss governed UI surfaces or block valid legacy rollback code.",
      "impact": "Local truth can remain reachable or rollback can become unusable.",
      "mitigation": "Phase 0 owns a maintained UI file/type inventory with explicit legacy-only files and exclusions."
    },
    {
      "risk": "Approval diagnostics look like broken primary actions.",
      "impact": "Operators can stall at approval gates or lose trust.",
      "mitigation": "P031 uses a diagnostic banner/callout rather than disabled primary buttons and captures dogfood comprehension evidence."
    },
    {
      "risk": "Operators cannot complete write workflows during dogfood.",
      "impact": "Dogfood validates rendering but not operator viability.",
      "mitigation": "Operator write-path guide maps every removed control before dogfood, and dogfood captures per-run workflow-completion notes."
    },
    {
      "risk": "Report metadata-only behavior is perceived as a regression.",
      "impact": "Operators lose trust in the cutover.",
      "mitigation": "Communicate the limitation before dogfood, show payload availability status in Reports list rows, show payloadUnavailableReasonCode in detail, and keep full payload rendering behind a high-priority follow-up."
    },
    {
      "risk": "Implementation follows a shorter checked-in proposal and misses run-local obligations.",
      "impact": "Phase 0 requirements are skipped.",
    "mitigation": "Source governance requires either synchronized checked-in addendum or explicit run-local r14 governing link before implementation handoff."
    },
    {
      "risk": "Legacy rollback code is removed before write-path restoration is ready.",
      "impact": "Operators could lose both the rollback escape hatch and in-app write controls during the post-P031 transition.",
      "mitigation": "Legacy expiry now requires a merged gate-green critical write-path follow-up or a dated release-owner waiver with a hard write-restoration deadline."
    },
    {
      "risk": "Rollback drill is treated as a checkbox.",
      "impact": "Release evidence can claim rollback readiness without proving timely recovery or state consistency.",
      "mitigation": "Phase 0d defines pass/fail criteria for 60-second time-to-legacy-mode, state consistency, and operator confirmation."
    },
    {
      "risk": "Operator guide rows and UI copyable identifiers drift.",
      "impact": "Dogfood can pass no-write checks while external write workflows fail in practice.",
      "mitigation": "The guide is a versioned contract artifact with minimum row schema and dogfood validation against copied identifiers."
    },
    {
      "risk": "Report payload metadata-only mode is underestimated.",
      "impact": "Operators may be forced to raw filesystem workflows for frequent report inspection.",
      "mitigation": "Report payload follow-up defaults to P0 unless Phase 0d usage evidence supports lower priority."
    },
    {
      "risk": "Reviewers cannot tell which conditions are implementation gates versus future follow-ups.",
      "impact": "The proposal can look comprehensive but still be hard to execute or re-review.",
    "mitigation": "r14 retains the implementation-readiness checklist grouped by Swift migration, dogfood, and legacy-removal milestones and adds a review-ready packet, explicit disagreement resolution, and Phase 0 artifact manifest."
    }
  ],
  "review_feedback_resolution": [
    {
      "issue_id": "ARCH-R9-01",
      "status": "addressed",
      "resolution": "Added p043_reconciliation_contract with Phase 0a required work and exit criteria. P043/P031 reference and gate language must remove command-completion refresh, command receipt display, and MCP command-control obligations from P031-owned UI before Phase 1.",
      "tradeoff": "Phase 0 now includes documentation/gate reconciliation before screen migration."
    },
    {
      "issue_id": "ARCH-R9-02",
      "status": "addressed",
      "resolution": "Added schema_matrix mapping freshnessState, disabledReasonCode, writePathState, diagnosticId, payloadAvailabilityState, payloadUnavailableReasonCode, and serverDebugDetail to GraphQL types/queries, enum cases, nullability, redaction, Swift presenter ownership, and tests.",
      "tradeoff": "More schema work is explicit in Phase 0a, but Swift no longer has to infer these states."
    },
    {
      "issue_id": "ARCH-R9-03",
      "status": "addressed",
      "resolution": "Added ui_ownership_inventory with governed Swift files, legacy-only files/types, generated GraphQL locations, forbidden patterns, and explicit exclusions.",
      "tradeoff": "The guard boundary must be maintained as files move."
    },
    {
      "issue_id": "ARCH-R9-04",
      "status": "addressed",
      "resolution": "Approval behavior is now binary. In P031, absent a separately approved transport, approvals are diagnostic-only with no disabled primary Approve/Reject buttons and no Swift write-availability inference.",
      "tradeoff": "Approval decisions are not completed in this UI until a follow-up transport proposal exists."
    },
    {
      "issue_id": "ARCH-R9-05",
      "status": "addressed",
    "resolution": "Source governance now requires a synchronized checked-in proposal/addendum or an explicit run-local r14 governing link before Phase 1. The addendum must copy Phase 0 obligations.",
      "tradeoff": "This refinement task still writes only run-local required outputs."
    },
    {
      "issue_id": "PO-R9-01",
      "status": "addressed",
      "resolution": "Added operator_write_path_guide as a Phase 0d deliverable mapping 100 percent of removed controls to external workflows or unavailable follow-ups before dogfood.",
      "tradeoff": "This documents existing external paths but does not add UI writes."
    },
    {
      "issue_id": "PO-R9-02",
      "status": "addressed",
      "resolution": "Added operator workflow viability metric and dogfood requirement for per-run workflow-completion notes.",
      "tradeoff": "Dogfood evidence now includes qualitative operator input."
    },
    {
      "issue_id": "PO-R9-03",
      "status": "addressed",
      "resolution": "Follow-ups now include priority tiers and expected start timeframes.",
      "tradeoff": "Timelines are planning expectations, not implementation commitments."
    },
    {
      "issue_id": "PO-R9-04",
      "status": "addressed",
      "resolution": "Dogfood evidence now requires at least one approval diagnostics comprehension observation and zero sign-off if confusion blocks a run.",
      "tradeoff": "Dogfood needs an approval queue encounter or targeted evidence."
    },
    {
      "issue_id": "PO-R9-05",
      "status": "addressed",
      "resolution": "Dogfood edge coverage now requires degraded-state recovery and approval-queue diagnostic evidence, not just two happy-path runs.",
      "tradeoff": "Dogfood may need targeted setup to exercise these states."
    },
    {
      "issue_id": "UX-R9-01",
      "status": "addressed",
      "resolution": "Disabled primary approval buttons are replaced by diagnostic banner/callout UI with copyable identifiers and external workflow copy when documented.",
      "tradeoff": "The UI is honest about diagnostics instead of implying in-app approval is possible."
    },
    {
      "issue_id": "UX-R9-02",
      "status": "addressed",
      "resolution": "Added first-run dogfood orientation banner and operator write-path guide link so Managed outside this UI has a concrete destination.",
      "tradeoff": "Operators must use external workflows until follow-ups restore UI controls."
    },
    {
      "issue_id": "UI-01",
      "status": "addressed",
      "resolution": "Syncing placement is now specified for Runs Home rows, stage rows, Run Detail, Artifacts, Reports, and Approvals.",
      "tradeoff": "Each surface reserves a small fixed slot for visual stability."
    },
    {
      "issue_id": "UI-02",
      "status": "addressed",
      "resolution": "Approval UI uses diagnostic banner/callout visual treatment rather than permanently disabled primary buttons.",
      "tradeoff": "The primary action is not shown as disabled; the surface becomes explicitly diagnostic."
    },
    {
      "issue_id": "UI-03",
      "status": "addressed",
      "resolution": "Reports payload indicators now specify SF Symbols, labels, stable 96 point trailing slot, and truncation rules.",
      "tradeoff": "The slot consumes fixed row width to preserve alignment."
    },
    {
      "issue_id": "PO-R10-01",
      "status": "addressed",
      "resolution": "Legacy expiry now requires either at least one critical write-path follow-up merged and gate-green or a dated release-owner waiver accepting the no-in-app-write gap with a hard write-restoration deadline.",
      "tradeoff": "Release ownership can still waive the dependency, but the gap is explicit, dated, and accountable."
    },
    {
      "issue_id": "PO-R10-02",
      "status": "addressed",
      "resolution": "Report payload follow-up now defaults to P0 unless Phase 0d provides operator workflow evidence that metadata-only report inspection is low-frequency and acceptable.",
      "tradeoff": "This may pull report payload work earlier if usage data is unavailable."
    },
    {
      "issue_id": "PO-R10-03",
      "status": "addressed",
      "resolution": "Rollback drill success criteria now require visible legacy mode within 60 seconds, state consistency assertions, and operator confirmation.",
      "tradeoff": "Rollback readiness evidence is more expensive but can meaningfully fail."
    },
    {
      "issue_id": "PO-R10-04",
      "status": "addressed",
      "resolution": "The operator guide now has a minimum-viable row schema with workflow/tool name, required identifiers, parameter shape, expected output, and validation status.",
      "tradeoff": "Polished recipes can follow later, but minimum executable contract data is required before dogfood."
    },
    {
      "issue_id": "PO-R10-05",
      "status": "addressed",
      "resolution": "Phase 3 sign-off now includes explicit review of each additional-evidence trigger.",
      "tradeoff": "The trigger check can remain manual, but it is part of sign-off evidence."
    },
    {
      "issue_id": "UX-R10-01",
      "status": "addressed",
      "resolution": "First-run guide affordance is directly clickable or copyable, and guide rows guarantee copied identifiers match external workflow inputs.",
      "tradeoff": "Operators still context switch to external tools during P031."
    },
    {
      "issue_id": "UX-R10-02",
      "status": "addressed",
      "resolution": "Reports VoiceOver copy now reads payload availability as complete sentences.",
      "tradeoff": "Accessibility copy is slightly longer than compact row labels."
    },
    {
      "issue_id": "UI-04",
      "status": "addressed",
      "resolution": "Approval diagnostics use informational diagnostic treatment distinct from primary error alerts.",
      "tradeoff": "Urgency is lower by default unless server reports an actual error."
    },
    {
      "issue_id": "UI-05",
      "status": "addressed",
      "resolution": "Syncing motion is constrained to subtle active-state motion and honors reduced-motion preferences.",
      "tradeoff": "The indicator is intentionally less visually prominent."
    },
    {
      "issue_id": "UI-06",
      "status": "addressed",
      "resolution": "First-run banner has a clear trailing dismissal control aligned with the Runs Home header.",
      "tradeoff": "Dismissal remains local presentation state only."
    },
    {
      "issue_id": "ARCH-R10-01",
      "status": "addressed",
    "resolution": "Source governance now requires a checked-in r14/addendum to supersede stale GraphQL+MCP idea-brief and P043/P031 text before screen migration.",
      "tradeoff": "Phase 0 cannot hand off implementation from stale artifacts."
    },
    {
      "issue_id": "ARCH-R10-02",
      "status": "addressed",
      "resolution": "P031 gate registration and P043 reconciliation remain Phase 0 blockers, with gate checks for no UI MCP, no GraphQL mutations, no command receipts/correlation, and no local write fallbacks.",
      "tradeoff": "Screen migration waits for executable guardrails."
    },
    {
      "issue_id": "ARCH-R10-03",
      "status": "addressed",
      "resolution": "Schema matrix must become executable GraphQL contract or explicit disabled/deferred UI state before affected screens migrate.",
      "tradeoff": "Missing fields block the corresponding UI migration instead of allowing Swift inference."
    },
    {
      "issue_id": "ARCH-R10-04",
      "status": "addressed",
      "resolution": "UI ownership inventory now has a machine-readable artifact path, gate-owned globs, and fail-closed rules for new governed files or operations.",
      "tradeoff": "Path inventory maintenance becomes an explicit implementation responsibility."
    },
    {
      "issue_id": "ARCH-R10-05",
      "status": "addressed",
      "resolution": "Operator write-path guide is now a versioned contract artifact with row schema and dogfood validation against copied identifiers.",
      "tradeoff": "External workflow documentation becomes part of release evidence, not optional docs."
    },
    {
      "issue_id": "ARCH-R10-06",
      "status": "addressed",
      "resolution": "Diagnostic/debug fields are operator-only by default; observer semantics are deferred unless a separate authorization policy lands.",
      "tradeoff": "P031 avoids expanding read authorization while preserving future observer support as a separate decision."
    },
    {
      "issue_id": "REREAD-R11-01",
      "status": "addressed",
    "resolution": "r14 retains the r12 clarification that critical write-path readiness requires merged gate-green restoration/replacement of approval resolution and at least one operationally critical run-control path; a draft follow-up is not sufficient.",
      "tradeoff": "This makes legacy removal harder to schedule but prevents an unsafe no-rollback/no-write window."
    },
    {
      "issue_id": "REREAD-R11-02",
      "status": "addressed",
    "resolution": "r14 retains the r12 implementation-readiness checklist grouped by Swift migration, dogfood, and legacy-removal milestones and adds the Phase 0 artifact manifest.",
      "tradeoff": "The proposal repeats some gates for readability, but reviewers and implementers get a concise execution checklist."
    },
  {
    "issue_id": "REREAD-R11-03",
    "status": "addressed",
    "resolution": "r14 keeps the operator write-path guide JSON as the gate-consumed source of truth and allows markdown to be generated for operator readability.",
    "tradeoff": "A structured artifact is required before dogfood, but it prevents prose drift from UI diagnostics."
  },
  {
    "issue_id": "REREAD-R12-01",
    "status": "addressed",
    "resolution": "r14 retains phase_0_artifact_manifest so reviewers and implementers have a single auditable list of the governing contract, P043 reconciliation, P031 gate, UI inventory, schema decision record, operator guide, rollback evidence, report-priority decision, and sign-off template.",
    "tradeoff": "Phase 0 gains one more structured artifact, but it reduces handoff ambiguity and gives the gate a clear completeness target."
  }
  ],
  "open_questions": [
    {
      "id": "OQ-031-01",
      "question": "Which exact external workflows should the operator write-path guide name for each removed write control?",
    "why_open": "r14 defines the minimum-viable guide row schema, manifest entry, and validation contract, but the input artifacts still do not provide the actual CLI/MCP/automation workflow names for every removed control.",
      "blocking_phase": "Phase 0d exit and dogfood start"
    },
    {
      "id": "OQ-031-02",
      "question": "Who is the named individual behind the P031 macOS thin UI owner and P031 release owner roles?",
      "why_open": "Reviewer feedback asked for named ownership, but the input artifacts do not include staffing assignments.",
      "blocking_phase": "Phase 0 start"
    },
    {
      "id": "OQ-031-03",
      "question": "What measured p95 GraphQL projection freshness should be used for dogfood readiness?",
      "why_open": "The measurement must be captured from representative runtime conditions.",
      "blocking_phase": "Phase 0d exit and dogfood sign-off"
    },
    {
      "id": "OQ-031-04",
    "question": "Will the checked-in proposal be expanded to the full r14 contract or receive a concise implementation addendum?",
    "why_open": "r14 requires one checked-in governing artifact before screen migration but does not choose the repository documentation strategy inside this run-local output task.",
      "blocking_phase": "Implementation handoff before Phase 1"
    },
    {
      "id": "OQ-031-05",
      "question": "Does Phase 0d usage evidence justify keeping report payload restoration below P0?",
    "why_open": "No operator report-inspection frequency data is present in the input artifacts, so r14 defaults the follow-up to P0 unless evidence proves metadata-only is acceptable.",
      "blocking_phase": "Phase 0d exit and Phase 3 flag removal decision"
    }
  ],
  "follow_ups": [
    {
      "id": "P031-FOLLOWUP-APPROVAL-WRITE-PATH",
      "priority": "P0 immediate next proposal",
      "expected_start": "Draft starts before P031 Phase 3 flag removal decision; legacy expiry cannot proceed without critical write-path readiness or waiver.",
      "description": "Define an approved non-MCP, non-GraphQL-mutation approval decision transport if interactive approval decisions must remain in the macOS UI."
    },
    {
      "id": "P031-FOLLOWUP-UI-CONTROL-SURFACE",
      "priority": "P1 next control-surface proposal, with legacy-expiry dependency",
      "expected_start": "Draft starts before legacy rollback code expiry, and legacy removal requires either critical write-path readiness or dated waiver.",
      "description": "Separately propose any future UI control surface for start/cancel/retry/create with an explicitly approved transport and safety model. MCP remains excluded from UI unless a future operator decision reverses this contract."
    },
    {
      "id": "P031-FOLLOWUP-REPORT-PAYLOAD",
      "priority": "P0 by default; may downgrade to P1 only with Phase 0d usage evidence that report payload inspection is low-frequency and metadata-only is acceptable",
      "expected_start": "Priority decision recorded before Phase 0d exit; if default P0 holds, draft starts before P031 Phase 3 flag removal decision.",
      "description": "Add server-owned GraphQL report payload readback and full payload UI rendering."
    }
  ],
  "final_recommendation": "Proceed to aggregate re-review as r14. The proposal remains intentionally GraphQL-only and read-only for the macOS UI. Reviewer conditions are now expressed as concrete gates: checked-in source supersession, executable schema-or-defer decisions, machine-readable UI inventory, gate-consumed operator guide JSON, Phase 0 artifact manifest, quantitative rollback drill criteria, explicit report-payload priority evidence, Phase 3 trigger review, and legacy expiry tied to critical_write_path_readiness or dated release-owner waiver. Do not begin Swift screen migration until the Phase 0 migration checklist is complete. Do not start dogfood until the guide, manifest, rollback, report-priority, and UX/accessibility checklist items are complete. Do not remove legacy rollback on the basis of draft follow-ups alone."
}
