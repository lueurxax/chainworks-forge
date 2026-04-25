{
  "proposal_revision_id": "p051-r30",
  "source_review_pass_id": "p051-review-pass-r29",
  "title": "Proposal 051 Implementation Plan: Shared Xcode MCP Bridge Pool",
  "status": "Fixture/readback implementation is schedulable against current reference/gate truth; broad shim_enforced rollout remains blocked until live dogfood/sign-off evidence is attached.",
  "review_readiness": {
    "target_score": "aggregate score above 9",
    "why_ready": [
      "The checked-in proposal is now the canonical implementation contract, and the scaffold gate must fail stale contrary guidance.",
      "The three architect blockers now have explicit owner-crate, caller, failure-mode, and acceptance-test contracts.",
      "Product rollout ambiguity is resolved with staged gates, broker enablement states, and a rollback switch that preserves non-Xcode workflows.",
      "UX/UI visibility concerns are resolved by making a narrow read-only UI/readback slice part of acceptance instead of leaving runtime truth in raw JSON only.",
      "R27 residual implementation risks now have named boundaries: XcodeTargetResolver, provider-process-bound shim dispatch, BrokerMcpPolicy, LaunchResourceGuard, and bounded observation append semantics.",
      "R28 proceed-with-preconditions feedback is now captured as concrete contracts for direct-command declaration scanning, broker subsystem health, target resolver ownership, observation storage write path, and operator-facing progress propagation.",
      "R29 proceed-with-preconditions feedback is now captured as mandatory implementation-start controls: source reconciliation is the first scaffold task, dependency audit has a required artifact schema and blocking rules, and observation hot-row pressure has explicit dogfood thresholds.",
      "Residual risks are named as trade-offs with explicit defaults, metrics, and dogfood evidence rather than hidden behind broad claims."
    ],
    "reviewer_specific_checks": [
      {
        "reviewer": "architect",
        "must_confirm": "Source-of-truth policy, observation sink, broker registry intent, Xcode target resolver, release-mode capability fingerprint, capacity policy, ordered request pump, shim process binding, broker MCP policy, launch resource guard, and Xcode signal fields are precise enough to implement without inventing cross-crate ownership."
      },
      {
        "reviewer": "product_owner",
        "must_confirm": "Rollout can proceed in scaffold/full milestones, dogfood has pass/fail evidence, and rollback preserves non-Xcode workflows."
      },
      {
        "reviewer": "ux_designer",
        "must_confirm": "Operators see waiting/starting states, policy warnings, friendly recovery text, and enough runtime evidence to understand Xcode-specific failures."
      },
      {
        "reviewer": "ui_designer",
        "must_confirm": "The UI scope maps to existing surfaces and avoids raw JSON dumping while staying narrow enough for P051."
      }
    ]
  },
  "run_id": "43460545-7f6a-4943-811a-dabad0f1c592",
  "source_proposal": "docs/proposals/051-shared-xcode-mcp-bridge-pool.md",
  "implementation_source_of_truth": {
    "controlling_artifact": "/Users/user/Documents/Chainworks Forge/docs/proposals/051-shared-xcode-mcp-bridge-pool.md",
    "source_proposal_status": "Canonical source-of-truth is checked in at `docs/proposals/051-shared-xcode-mcp-bridge-pool.md`; the .chainworks run proposal is provenance only.",
    "reconciliation_required_before_implementation": "Before any P051 implementation PR begins, confirm this checked-in proposal is canonical and remove stale implementation-source guidance from the checked-in source.",
    "static_gate_requirement": "`p051-scaffold` fails if the checked-in source proposal still contains stale contrary guidance: no SwiftUI changes, debug_assert-only capability enforcement, path+mtime+size-only binary fingerprinting, drop-on-corrupt-observation behavior, direct pgrep newest-Xcode selection, or unbound same-uid-only shim authorization."
  },
  "research_artifact": "docs/proposals/051-shared-xcode-mcp-bridge-pool.review/http-streaming-feasibility.md",
  "research_verdict": "Proceed with scoped architecture",
  "primary_surfaces": [
    "Rust control-plane ACP runtime, engine, daemon, workflow/catalog, DB/domain, GraphQL/MCP reports, and test gates",
    "Minimal macOS SwiftUI read-only presentation of Xcode runtime observations, failure strings, and catalog infrastructure flags"
  ],
  "problem": {
    "summary": "Parallel Xcode-capable ACP sessions currently fail because provider-owned Xcode processes race Xcode authorization and because fake-home ACP providers run host-bound Xcode tooling outside the real GUI user's Xcode environment.",
    "details": [
      "Parallel ACP providers can each start their own direct `xcrun mcpbridge`, causing duplicated Xcode consent or modal handling during the first-start fan-out window.",
      "ACP providers run in isolated fake-home environments, while Xcode, CoreSimulator, simdiskimaged, DerivedData, and Xcode MCP authorization are tied to the macOS GUI user's real `~/Library` and Darwin temp state.",
      "Direct `xcodebuild`, `simctl`, or `mcpbridge` execution from fake-home provider processes can produce false runtime discovery failures even when the same command succeeds from the host shell.",
      "The correct boundary is not to give ACP providers the host home. Providers keep fake-home isolation; Xcode-bound work crosses a narrow Chainworks-owned host-session boundary."
    ],
    "positioning": "P051 is a reliability boundary first and a modal-deduplication optimization second."
  },
  "goals": [
    "Serve Xcode MCP to ACP providers through Chainworks-owned HTTP streaming endpoints instead of provider-owned `xcrun mcpbridge` stdio entries.",
    "Share one initialized backend `xcrun mcpbridge` subprocess per `run_id + Xcode pid + developer_dir`, under the host-user Xcode environment, while keeping provider HTTP leases and policies isolated at the broker facade.",
    "Serialize first backend spawn plus MCP `initialize` per Xcode target, return cached initialize results to sibling leases, and route same run/target `tools/*` calls through one ordered backend pump while different run/target keys use independent backend processes.",
    "Preserve provider fake-home isolation for ACP/model/client state.",
    "Fail closed before lease, token, backend process, shim token, or `session/new` payload allocation when the provider does not advertise HTTP MCP capability.",
    "Guard direct Xcode shell commands through PATH shims and catalog lint for the enforced boundary: PATH-based commands and catalog-declared structured commands.",
    "Keep `mcpbridge` broker-only inside the enforced boundary, with no diagnostic bypass.",
    "Preserve per-agent permission policy, session reuse semantics, and recoverability.",
    "Add durable, typed Xcode runtime observations so operators can prove what happened without log scraping.",
    "Expose those observations through GraphQL, MCP `reports.get`, and a minimum read-only Forge UI surface.",
    "Register staged gates `p051-scaffold` and `proposal-051|p051`."
  ],
  "non_goals": [
    "No changes to XcodeBuildMCP itself.",
    "No provider-facing stdio proxy fallback for brokered Xcode MCP.",
    "No cross-daemon Xcode bridge sharing.",
    "No general-purpose pooling for non-Xcode MCP servers.",
    "No broad removal of fake-home isolation for ACP providers.",
    "No OS-level prevention of every possible absolute-path command an LLM might invent at prompt time; P051 observes and warns on that residual, and a future libc-audit or sandbox-exec proposal would be needed to block it.",
    "No broad SwiftUI redesign. P051 includes only the minimum read-only UI/status/error mappings needed to make the new runtime truth actionable."
  ],
  "ux_ui_notes": {
    "scope_change_from_r24": "R24 said no Swift app UI changes. UX and UI review showed that durable observations without in-app presentation are not usable by operators. This revision keeps the Rust control-plane work primary but adds a small UI slice.",
    "operator_visible_behavior": [
      "Parallel Xcode-capable reviewers still run as separate ACP sessions and produce separate outputs.",
      "Each session receives an HTTP Xcode MCP server entry backed by Chainworks.",
      "The first Xcode process may require one consent interaction; sibling leases against the same Xcode PID should not trigger additional modals.",
      "Broker, host-environment, capability, shim, simulator-selection, and provider-transport failures are classified separately instead of being presented as generic agent failures.",
      "Run timeline/status surfaces distinguish `Waiting for Xcode Bridge lock` from `Starting Xcode Bridge` so initialize serialization does not look like a hang.",
      "If bridge startup is blocked for more than five seconds while Xcode consent is plausible, the timeline/readback surface shows an `Action Required: Check Xcode` state before the initialize timeout fires.",
      "`Action Required: Check Xcode` includes workspace identity and Xcode PID when available from `XcodeTargetSnapshot`, so operators with multiple Xcode windows know which instance needs attention.",
      "Parallel DerivedData or build-system contention is identified as Xcode build concurrency, not broker infrastructure failure."
    ],
    "minimum_ui_surface": [
      {
        "surface": "RunTimelineInspectorView",
        "content": "Add an `Xcode Runtime` read-only section only when `actual_xcode_runtime_observation_json` is present, showing lease state, backend PID, backend start disposition, host-home disposition, initialize wait, broker health, shim route/reject decisions, and selected simulator UUID when present. Use structured rows rather than raw JSON."
      },
      {
        "surface": "Run timeline or log",
        "content": "Render Xcode residual path warnings as high-visibility `Policy Warning` timeline events, not only as JSON. Use SF Symbol `exclamationmark.shield`, orange foreground, coalesce repeated identical warnings per execution, and collapse unique residual paths behind a `View all residual paths` disclosure once more than five unique warnings appear for one execution."
      },
      {
        "surface": "FailedStageEvidencePanel and timeline error presentation",
        "content": "Map P051 technical failure classes to localized friendly strings and suggested actions."
      },
      {
        "surface": "AgentCatalogView",
        "content": "Add an `Infrastructure` or `Xcode Broker` metadata section that shows `xcode_broker_required`, `xcode_shim_injection_signal`, and `requires_xcode_host_execution` before a run starts. When `xcode_broker_required` is true, show a small first-run note that a one-time Xcode consent interaction may be required."
      },
      {
        "surface": "Daemon lifecycle or diagnostics surface",
        "content": "Show broker health as Disabled, Healthy, Degraded, or Failed when the daemon reports it."
      },
      {
        "surface": "Accessibility and test hooks",
        "content": "Add stable identifiers for the minimum UI slice: `xcode-runtime-section`, `xcode-lease-id`, `xcode-backend-pid`, `xcode-policy-warning`, `xcode-friendly-failure`, `infrastructure-xcode-broker-required`, `infrastructure-xcode-shim-injection-signal`, and `infrastructure-requires-xcode-host-execution`."
      },
      {
        "surface": "Visual style",
        "content": "Use the existing ForgePanel/GroupBox style used by neighboring inspector sections; no raw JSON block, custom dashboard, or new visual system is introduced."
      }
    ],
    "friendly_failure_mapping": [
      {
        "class": "provider_http_mcp_unsupported",
        "title": "Provider does not support HTTP MCP",
        "suggested_action": "Use Codex ACP, Claude Agent ACP, or Gemini CLI with the verified HTTP MCP version, or mark the provider out of Xcode-broker scope until capability is proven."
      },
      {
        "class": "xcode_mcp_registry_stale_stdio",
        "title": "Xcode MCP registry entry uses direct stdio",
        "suggested_action": "Update the machine MCP registry to the brokered Xcode entry or remove stale `xcrun mcpbridge` command fields."
      },
      {
        "class": "xcode_mcp_registry_ambiguous",
        "title": "Multiple Xcode MCP registry entries match",
        "suggested_action": "Keep one canonical `xcode` broker entry and remove duplicate or ambiguous entries."
      },
      {
        "class": "host_env_unavailable",
        "title": "Host Xcode environment unavailable",
        "suggested_action": "Confirm the daemon runs as the GUI user or configure the operator-home override, then retry the step."
      },
      {
        "class": "pool_pid_drift",
        "title": "Xcode process changed during run",
        "suggested_action": "Ensure the intended Xcode workspace is open and retry the failed execution."
      },
      {
        "class": "xcode_mcp_capacity_exhausted",
        "title": "Xcode bridge capacity reached",
        "suggested_action": "Wait for active Xcode reviewers to finish, reduce fan-out, or raise the runtime-profile limit deliberately."
      },
      {
        "class": "xcode_mcp_initialize_timeout",
        "title": "Xcode bridge initialization timed out",
        "suggested_action": "Check for an Xcode consent modal, confirm Xcode is responsive, and retry."
      },
      {
        "class": "xcode_mcp_action_required",
        "title": "Check Xcode to continue",
        "suggested_action": "Bring Xcode to the foreground and respond to any consent or authorization prompt."
      },
      {
        "class": "xcode_mcp_first_connect_timeout",
        "title": "Provider did not connect to Xcode bridge",
        "suggested_action": "Retry the execution; if repeated, inspect provider HTTP MCP support and session/new payload logs."
      },
      {
        "class": "xcode_shim_no_active_prompt",
        "title": "Xcode command ran outside the active prompt",
        "suggested_action": "Retry the execution; if repeated, disable session reuse for that agent or inspect background shell activity."
      },
      {
        "class": "simulator_destination_ambiguous",
        "title": "Simulator destination is ambiguous",
        "suggested_action": "Choose one of the listed simulator UUIDs or remove duplicate simulator name/OS matches."
      },
      {
        "class": "xcode_build_concurrency_contention",
        "title": "Xcode build resources are busy",
        "suggested_action": "Retry after sibling builds finish, use a different DerivedData path, or reduce parallel build fan-out for this workflow."
      }
    ]
  },
  "resolved_reviewer_feedback": [
    {
      "issues": [
        "ARCH-001",
        "SLB-001"
      ],
      "resolution": "Added an explicit observation persistence boundary: domain owns typed observation models, db owns the transactional append repository API, engine owns the concrete sink and injects an async sink handle into acp, while acp calls only the sink trait and never depends on db."
    },
    {
      "issues": [
        "ARCH-002",
        "SLB-002"
      ],
      "resolution": "Added a concrete `BrokeredXcodeMcpIntent` and registry migration contract, including canonical server-id matching, compatibility handling for existing `xcrun mcpbridge` entries, stale/ambiguous fail-closed errors, and redacted predicted/actual MCP truth."
    },
    {
      "issues": [
        "ARCH-003",
        "SLB-003"
      ],
      "resolution": "Replaced the debug assertion with release-mode `CapabilitySliceFingerprint` enforcement immediately before provider launch. Credential-only env mutations are excluded by construction and covered by tests."
    },
    {
      "issues": [
        "PO-001",
        "PO-002",
        "ARCH-006",
        "SLB-004"
      ],
      "resolution": "Added broker enablement states and `CHAINWORKS_XCODE_BROKER_DISABLED=1` as a rollback switch. Non-Xcode daemon functionality remains available when the broker is disabled or degraded; final brokered Xcode MCP still has no stdio fallback."
    },
    {
      "issues": [
        "ARCH-004",
        "SLB-005"
      ],
      "resolution": "Added capacity defaults, bounded initialize queue policy, queue timeout, capacity failure class, and required metrics/observations."
    },
    {
      "issues": [
        "PO-003",
        "SLB-006"
      ],
      "resolution": "Promoted modal dedup to a pre-ship dogfood acceptance criterion with named minimum evidence."
    },
    {
      "issues": [
        "PO-004",
        "SLB-007"
      ],
      "resolution": "Split delivery into `p051-scaffold` and full `proposal-051|p051` gates."
    },
    {
      "issues": [
        "UX-001",
        "UI-001",
        "SLB-008"
      ],
      "resolution": "Added a minimum read-only UI surface for Xcode runtime evidence rather than raw JSON-only exposure."
    },
    {
      "issues": [
        "UX-003",
        "UI-002",
        "SLB-009"
      ],
      "resolution": "Added friendly error titles and suggested actions for new broker, shim, registry, and simulator failure classes."
    },
    {
      "issues": [
        "ARCH-007",
        "UI-003",
        "SLB-010",
        "SLB-015"
      ],
      "resolution": "Added exact field names, serde names, defaults, propagation, session fingerprint participation, and Agent Catalog visibility for Xcode broker/shim signals."
    },
    {
      "issues": [
        "ARCH-005",
        "SLB-011"
      ],
      "resolution": "Clarified that same run/target leases share one initialized stdio backend with an ordered request pump; cross-lease progress is enforced at the broker facade, and different run/target keys use independent backend processes."
    },
    {
      "issues": [
        "UX-002",
        "SLB-012"
      ],
      "resolution": "Residual path warnings become high-visibility policy warnings in the timeline/log and remain persisted in the observation JSON."
    },
    {
      "issues": [
        "UX-004",
        "SLB-013"
      ],
      "resolution": "Added explicit progress states for waiting on the per-PID initialize lock."
    },
    {
      "issues": [
        "PO-005",
        "SLB-014"
      ],
      "resolution": "Added pass/fail thresholds for capability cache fixtures, dogfood scenario size, observation completeness, modal count, and approver evidence."
    },
    {
      "issues": [
        "ARCH-SUG-004"
      ],
      "resolution": "Auggie and Junie are out of P051 launch scope for Xcode MCP until HTTP MCP capability is proven by the initialize-only probe; they fail closed for brokered Xcode MCP and may continue non-Xcode work."
    },
    {
      "issues": [
        "PO-ASM-004"
      ],
      "resolution": "Strengthened provider binary fingerprinting from path+mtime+size to path+size+mtime+content SHA-256 when readable, with explicit fallback observation if hashing is unavailable."
    },
    {
      "issues": [
        "ARCH-R27-001"
      ],
      "resolution": "Checked-in `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` is the controlling contract; stale contradictory guidance fails the `p051-scaffold` static gate."
    },
    {
      "issues": [
        "ARCH-R27-002"
      ],
      "resolution": "Added an `XcodeTargetResolver` boundary with deterministic Xcode PID/workspace/developer-dir selection and fail-closed ambiguity handling before backend spawning."
    },
    {
      "issues": [
        "ARCH-R27-003",
        "PO-R27-003"
      ],
      "resolution": "Tightened shim dispatch authority from token plus same-uid to provider process identity binding, with same-uid cross-session replay, stale token, forged PID, and process-tree mismatch fixtures required before host executor merge."
    },
    {
      "issues": [
        "ARCH-R27-004"
      ],
      "resolution": "Added a broker-side `BrokerMcpPolicy` interface at the HTTP facade to filter `tools/list`, deny unauthorized `tools/call`, persist denied truth, and isolate sibling leases."
    },
    {
      "issues": [
        "ARCH-R27-005"
      ],
      "resolution": "Bounded append-heavy observation storage with event/byte limits, retry caps, corrupt-json quarantine behavior, and a normalized-storage escape hatch if limits conflict with evidence completeness."
    },
    {
      "issues": [
        "ARCH-R27-006"
      ],
      "resolution": "Added `LaunchResourceGuard` ownership for fake-home, temp, and generated config resources across probes, real-session transfer, and rollback before lease allocation."
    },
    {
      "issues": [
        "PO-R27-001"
      ],
      "resolution": "Promoted upstream dependency audit for P025, P026, P029, P037, and P049 to a named pre-scheduling and `p051-scaffold` precondition; current implemented-system reference/gate truth supersedes missing historical proposal-lineage files."
    },
    {
      "issues": [
        "PO-R27-002"
      ],
      "resolution": "Clarified rollback behavior: enabling `CHAINWORKS_XCODE_BROKER_DISABLED=1` causes in-flight and new Xcode-brokered executions to fail closed immediately while non-Xcode workflows continue after restart."
    },
    {
      "issues": [
        "UX-005"
      ],
      "resolution": "Added an `Action Required: Check Xcode` state after five seconds of likely modal-blocked bridge startup."
    },
    {
      "issues": [
        "UX-006"
      ],
      "resolution": "Added failure classification and recovery copy for DerivedData/build-system concurrency contention so it is not misdiagnosed as broker failure."
    },
    {
      "issues": [
        "UX-007",
        "UI-NB-001",
        "UI-NB-002",
        "UI-SUG-001",
        "UI-SUG-002",
        "UI-SUG-003"
      ],
      "resolution": "Specified conditional Xcode Runtime rendering, structured rows, policy-warning event treatment, coalescing behavior, and required accessibility identifiers."
    },
    {
      "issues": [
        "ARCH-R27-001"
      ],
      "resolution": "Checked-in proposal text is the controlling contract; stale contradictory source text is a `p051-scaffold` failure."
    },
    {
      "issues": [
        "ARCH-R27-002"
      ],
      "resolution": "Added `XcodeTargetResolver` as the deterministic target-selection boundary and prohibited pgrep-newest-Xcode selection for brokered paths."
    },
    {
      "issues": [
        "ARCH-R27-003",
        "PO-R27-003"
      ],
      "resolution": "Bound shim dispatch authority to the launched provider process identity or tracked descendant set and made targeted security review a third-PR gate."
    },
    {
      "issues": [
        "ARCH-R27-004"
      ],
      "resolution": "Added `BrokerMcpPolicy` at the HTTP facade for tools/list filtering, tools/call denial, denied-observation persistence, and sibling-lease isolation."
    },
    {
      "issues": [
        "ARCH-R27-005"
      ],
      "resolution": "Added observation event/byte bounds, retry limits, corrupt-json recovery, truncation signaling, and late-append refresh semantics."
    },
    {
      "issues": [
        "ARCH-R27-006"
      ],
      "resolution": "Added `LaunchResourceGuard` ownership for fake-home/temp/config resources across probes, real sessions, and rollback."
    },
    {
      "issues": [
        "PO-R27-001"
      ],
      "resolution": "Added dependency-audit precondition before scheduling; current reference/gate evidence now distinguishes fixture/readback schedulability from broad rollout dogfood/sign-off."
    },
    {
      "issues": [
        "PO-R27-002"
      ],
      "resolution": "Rollback text now states in-flight and new Xcode-brokered executions fail closed immediately while non-Xcode workflows continue."
    },
    {
      "issues": [
        "UX-005",
        "UX-SUG-001"
      ],
      "resolution": "Added an `Action Required: Check Xcode` operator state when bridge startup is blocked and consent is plausible."
    },
    {
      "issues": [
        "UX-006"
      ],
      "resolution": "Added DerivedData/build-system contention as an explicit friendly failure class separate from broker failures."
    },
    {
      "issues": [
        "UX-007",
        "UI-NB-001",
        "UI-NB-002",
        "UI-SUG-001",
        "UI-SUG-002",
        "UI-SUG-003"
      ],
      "resolution": "Specified Policy Warning icon/color/coalescing, conditional Xcode Runtime rendering, structured rows, and accessibility identifiers."
    },
    {
      "issues": [
        "ARCH-R28-001"
      ],
      "resolution": "Added `DirectCommandDeclarationScanner` as the normalized scanner contract over raw workflow/catalog YAML plus typed agent entries, with fixtures for current permission-profile shell allow entries and `agents[].required_tools`."
    },
    {
      "issues": [
        "ARCH-R28-002"
      ],
      "resolution": "Added `XcodeBrokerHealthSnapshot` as subsystem health separate from global daemon readiness, so Disabled/Degraded broker state gates only brokered Xcode requests."
    },
    {
      "issues": [
        "ARCH-R28-003"
      ],
      "resolution": "Clarified `XcodeTargetResolver` as a trait/service boundary with engine-provided selection inputs and acp-owned host-environment probing that returns immutable `XcodeTargetSnapshot`."
    },
    {
      "issues": [
        "ARCH-R28-004"
      ],
      "resolution": "Kept the bounded JSON envelope for P051 but made the append repository API the only write path and documented the normalized event-table migration trigger if dogfood shows hot-row pressure."
    },
    {
      "issues": [
        "UX-ISS-001"
      ],
      "resolution": "`Action Required: Check Xcode` now includes workspace identity and Xcode PID when available from `XcodeTargetSnapshot`."
    },
    {
      "issues": [
        "UX-ISS-002"
      ],
      "resolution": "Bridge initialization states are required in the run timeline/inspector readback surface; broad high-level summary propagation is deferred until the GraphQL run-list projection carries Xcode observation summaries."
    },
    {
      "issues": [
        "UX-NB-001"
      ],
      "resolution": "Unique residual path warnings collapse behind a `View all residual paths` disclosure after more than five unique warnings per execution."
    },
    {
      "issues": [
        "UX-SUG-001",
        "UX-SUG-002"
      ],
      "resolution": "Agent Catalog shows a first-run Xcode consent note for broker-required agents, and Xcode Runtime UI uses existing ForgePanel/GroupBox styling."
    },
    {
      "issues": [
        "ARCH-R29-001"
      ],
      "resolution": "Made source proposal reconciliation or explicit redirect the first scaffold task and a hard implementation-start precondition; PR review must fail if implementation starts from stale checked-in proposal text."
    },
    {
      "issues": [
        "ARCH-R29-002"
      ],
      "resolution": "Expanded dependency audit into a required pre-scheduling artifact with proposal id, owner, gate status, remaining gaps, and parallel-versus-sequential classification; implemented-system reference/gate truth now determines fixture schedulability."
    },
    {
      "issues": [
        "ARCH-R29-003"
      ],
      "resolution": "Kept bounded JSON observation storage but defined dogfood hot-row pressure thresholds and normalized event-table migration trigger before broad `shim_enforced` rollout."
    }
  ],
  "reviewer_feedback_resolution_matrix": [
    {
      "reviewer": "product_owner",
      "issue_id": "PO-001",
      "concern": "Daemon hard-fail on broker mount failure removes all Chainworks functionality.",
      "proposal_resolution": "Daemon can start in Xcode Broker Disabled or Degraded state while continuing non-Xcode workflows. Only the shared daemon listener failing remains process-fatal.",
      "where_resolved": [
        "rollout_plan.enablement_states",
        "failure_semantics",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-002",
      "concern": "No rollback gate or kill switch for Xcode brokering.",
      "proposal_resolution": "`CHAINWORKS_XCODE_BROKER_DISABLED=1` disables broker lease allocation and shim injection while preserving non-Xcode daemon functionality. It is explicitly a rollback switch, not a production stdio fallback.",
      "where_resolved": [
        "rollout_plan.rollback",
        "risks_and_mitigations",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-003",
      "concern": "Modal-dedup success was metric-only and not an acceptance criterion.",
      "proposal_resolution": "Dogfood acceptance now requires at most one Xcode consent modal per Xcode process for a parallel Xcode-capable stage.",
      "where_resolved": [
        "metrics",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-004",
      "concern": "Rollout had no intermediate delivery milestones.",
      "proposal_resolution": "Delivery is split into `p051-scaffold` and full `proposal-051|p051` gates, with first/second/third PR handoff slices.",
      "where_resolved": [
        "rollout_plan.milestones",
        "implementation_handoff"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-005",
      "concern": "Probe cache and dogfood metrics lacked pass/fail thresholds.",
      "proposal_resolution": "Metrics now specify cache-hit/miss thresholds, minimum dogfood scenario, modal count, observation completeness, and sign-off evidence.",
      "where_resolved": [
        "metrics"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-001",
      "concern": "Runtime observations were inaccessible to operators in-app.",
      "proposal_resolution": "A minimum `Xcode Runtime` section in RunTimelineInspectorView and related read-only surfaces are now in scope.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface",
        "implementation_inventory.swift_ui",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-002",
      "concern": "Prompt-time residual path soft warnings could be ignored.",
      "proposal_resolution": "Residual path warnings become high-visibility `Policy Warning` timeline/log events and remain persisted in observations.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface",
        "architecture.catalog_lint_and_residual_observer"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-003",
      "concern": "Broker-specific failures lacked recovery guidance.",
      "proposal_resolution": "P051 failure classes now map to friendly titles and suggested actions for the error presentation layer.",
      "where_resolved": [
        "ux_ui_notes.friendly_failure_mapping",
        "failure_semantics"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-004",
      "concern": "Initialize serialization could look like a hang.",
      "proposal_resolution": "Operator status distinguishes `Waiting for Xcode Bridge lock` from `Starting Xcode Bridge`.",
      "where_resolved": [
        "ux_ui_notes.operator_visible_behavior",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "ui_designer",
      "issue_id": "UI-001",
      "concern": "Dense runtime JSON had no concrete rendering plan.",
      "proposal_resolution": "UI maps typed observation fields to RunTimelineInspectorView rather than displaying raw JSON.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface",
        "implementation_inventory.swift_ui"
      ]
    },
    {
      "reviewer": "ui_designer",
      "issue_id": "UI-002",
      "concern": "Technical failure classes lacked human-readable UI mapping.",
      "proposal_resolution": "Friendly failure mapping covers broker, shim, registry, and simulator classes with suggested actions.",
      "where_resolved": [
        "ux_ui_notes.friendly_failure_mapping"
      ]
    },
    {
      "reviewer": "ui_designer",
      "issue_id": "UI-003",
      "concern": "Agent capability signals were not visible in the catalog UI.",
      "proposal_resolution": "AgentCatalogView shows Xcode infrastructure flags before run start.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface",
        "architecture.direct_command_guard.domain_fields",
        "implementation_inventory.swift_ui"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-001",
      "concern": "Broker-to-persistence ownership was underspecified across acp, engine, db, and daemon.",
      "proposal_resolution": "Domain owns types, db owns transactional append, engine owns concrete sink and attribution, and acp depends only on an injected sink trait.",
      "where_resolved": [
        "architecture.runtime_ownership",
        "architecture.durable_observation_schema"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-002",
      "concern": "Brokered Xcode MCP registry/resolution contract was not concrete.",
      "proposal_resolution": "`BrokeredXcodeMcpIntent` defines canonical registry shape, compatibility migration, stale/ambiguous fail-closed errors, and redacted MCP truth.",
      "where_resolved": [
        "architecture.brokered_xcode_mcp_resolution"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-003",
      "concern": "Probe/session invariant was only a debug assertion.",
      "proposal_resolution": "`CapabilitySliceFingerprint` is computed after launch-spec preparation and rechecked in release mode immediately before provider launch.",
      "where_resolved": [
        "architecture.provider_capability_preflight",
        "architecture.seven_phase_executor_flow",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-004",
      "concern": "Shared initialized bridge backend model needed capacity, backpressure, and deterministic ref-counted cleanup.",
      "proposal_resolution": "Defaults and failure classes are specified for active leases, initialize queue, queue timeout, spawn/initialize timeout, first-connect deadline, and shared-backend ref-count cleanup.",
      "where_resolved": [
        "architecture.capacity_and_backpressure",
        "metrics"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-005",
      "concern": "`tools/*` parallelism could be misread as concurrent writes to one shared stdio backend.",
      "proposal_resolution": "Cross-lease progress is required, but same run/target leases share one ordered request pump; different run/target keys use independent backend processes.",
      "where_resolved": [
        "architecture.broker_backend_model",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-006",
      "concern": "Startup and rollout were too all-or-nothing.",
      "proposal_resolution": "Enablement states and rollback switch allow disabled/degraded operation without non-Xcode downtime while preserving no stdio fallback for final Xcode MCP.",
      "where_resolved": [
        "rollout_plan.enablement_states",
        "failure_semantics"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-007",
      "concern": "Catalog lint and shim signal propagation needed concrete model fields.",
      "proposal_resolution": "Exact serde field names, defaults, propagation rules, and session fingerprint inputs are specified.",
      "where_resolved": [
        "architecture.direct_command_guard.domain_fields",
        "architecture.session_reuse"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-008",
      "concern": "Supplemental artifacts were missing in the earlier review environment.",
      "proposal_resolution": "Current pass consumed available review corpus, score-lift backlog, fact digest, and scope plan; `run-state.json` remains missing and is recorded explicitly rather than inferred.",
      "where_resolved": [
        "revision-summary.missing_inputs",
        "feedback-coverage.notes"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-001",
      "concern": "Review-path and checked-in proposal guidance disagreed on implementation contracts.",
      "proposal_resolution": "Checked-in `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` is the implementation source for P051; `p051-scaffold` must fail stale contradictory guidance.",
      "where_resolved": [
        "implementation_source_of_truth",
        "rollout_plan.preconditions",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-002",
      "concern": "Xcode target selection was implicit and could fall back to newest-process heuristics.",
      "proposal_resolution": "`XcodeTargetResolver` returns pid, workspace identity, developer dir, confidence, and failure class, and fails closed on no match or ambiguity.",
      "where_resolved": [
        "architecture.xcode_target_resolver",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-003",
      "concern": "Shim dispatch authority was token plus same-uid, which does not isolate sibling provider sessions.",
      "proposal_resolution": "Dispatch leases bind to launched provider pid/process group/tracked descendants, and same-uid cross-session replay is rejected.",
      "where_resolved": [
        "architecture.shim_dispatch_and_host_executor",
        "security_and_isolation",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-004",
      "concern": "Broker-side MCP policy enforcement lacked a named interface.",
      "proposal_resolution": "`BrokerMcpPolicy` at the HTTP facade filters `tools/list`, denies unauthorized `tools/call`, persists denial truth, and keeps sibling leases isolated.",
      "where_resolved": [
        "architecture.broker_mcp_policy",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-005",
      "concern": "Append-heavy observation storage lacked bounds and corrupt-json recovery semantics.",
      "proposal_resolution": "Observation appends now have event/byte limits, retry caps, corrupt-json quarantine, truncation summaries, and a normalized-storage trigger.",
      "where_resolved": [
        "architecture.durable_observation_schema",
        "risks_and_mitigations",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-006",
      "concern": "Provider probe and launch resource cleanup was implicit.",
      "proposal_resolution": "`LaunchResourceGuard` owns probe fake-home/temp/config resources, transfers ownership to real sessions, and rolls back before lease allocation on capability failure.",
      "where_resolved": [
        "architecture.provider_capability_preflight",
        "architecture.seven_phase_executor_flow",
        "implementation_handoff"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-R27-001",
      "concern": "Upstream dependency status was not a named scheduling gate.",
      "proposal_resolution": "Dependency audit is a pre-scheduling and `p051-scaffold` precondition; P025 or P026 incompleteness blocks scheduling unless PR1 is explicitly narrowed.",
      "where_resolved": [
        "rollout_plan.preconditions",
        "open_questions",
        "implementation_handoff"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-R27-002",
      "concern": "Rollback did not state what happens to in-flight brokered Xcode executions.",
      "proposal_resolution": "Rollback now explicitly fails in-flight and new Xcode-brokered executions closed after restart while preserving non-Xcode workflows.",
      "where_resolved": [
        "rollout_plan.rollback",
        "failure_semantics"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-R27-003",
      "concern": "Security claims remain unaudited.",
      "proposal_resolution": "A focused security review is required before the host-executor/shim-dispatch PR merges.",
      "where_resolved": [
        "rollout_plan.preconditions",
        "implementation_handoff.third_pr",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-005",
      "concern": "A single Xcode consent modal could still look like a random timeout.",
      "proposal_resolution": "Startup blocked for more than five seconds shows `Action Required: Check Xcode` with explicit recovery copy.",
      "where_resolved": [
        "ux_ui_notes.operator_visible_behavior",
        "ux_ui_notes.friendly_failure_mapping"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-006",
      "concern": "Parallel DerivedData contention could be mistaken for broker failure.",
      "proposal_resolution": "Build concurrency contention gets a distinct failure class and recovery guidance.",
      "where_resolved": [
        "ux_ui_notes.friendly_failure_mapping",
        "risks_and_mitigations"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-007",
      "concern": "High-visibility residual path warnings could clutter the timeline.",
      "proposal_resolution": "Repeated identical policy warnings are coalesced per execution.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface"
      ]
    },
    {
      "reviewer": "ui_designer",
      "issue_id": "UI-NB-001/UI-NB-002/UI-SUG-001/UI-SUG-002/UI-SUG-003",
      "concern": "Policy warning styling, conditional rendering, accessibility IDs, and structured observation rendering needed explicit UI contracts.",
      "proposal_resolution": "Specified policy-warning symbol/color/event type, conditional Xcode Runtime section, structured rows, and accessibility identifiers.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface",
        "implementation_inventory.swift_ui"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-001",
      "concern": "Review-path and checked-in source guidance disagreed on implementable contracts.",
      "proposal_resolution": "Checked-in `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` is authoritative for implementation; stale contrary source guidance fails `p051-scaffold`.",
      "where_resolved": [
        "implementation_source_of_truth",
        "rollout_plan.preconditions",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-002",
      "concern": "Xcode PID/workspace target selection relied on implicit newest-Xcode assumptions.",
      "proposal_resolution": "`XcodeTargetResolver` returns pid, workspace identity, developer dir, host env, confidence, and fail-closed classes.",
      "where_resolved": [
        "architecture.xcode_target_resolver",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-003",
      "concern": "Same-uid shim authorization was not enough to isolate sibling provider sessions.",
      "proposal_resolution": "Shim dispatch leases bind to provider child pid/process group/descendant set and reject cross-session replay or process mismatch.",
      "where_resolved": [
        "architecture.shim_dispatch_and_host_executor",
        "metrics",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-004",
      "concern": "Broker-side MCP policy enforcement lacked a named boundary.",
      "proposal_resolution": "`BrokerMcpPolicy` filters tools/list, denies tools/call before forwarding, persists denials, and isolates sibling leases.",
      "where_resolved": [
        "architecture.broker_mcp_policy",
        "metrics",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-005",
      "concern": "Append-heavy observation storage lacked bounds and corrupt-json semantics.",
      "proposal_resolution": "Storage bounds, retry cap, corrupt-json recovery, truncation signaling, and late-append refresh are specified.",
      "where_resolved": [
        "architecture.durable_observation_schema.storage_bounds",
        "metrics",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R27-006",
      "concern": "Probe/session fake-home resources lacked ownership and cleanup semantics.",
      "proposal_resolution": "`LaunchResourceGuard` owns probe/session resources and defines cleanup, transfer, rollback, and crash cleanup.",
      "where_resolved": [
        "architecture.launch_resource_ownership",
        "implementation_handoff"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-R27-001",
      "concern": "Dependency audit lacked owner/deadline/blocking threshold.",
      "proposal_resolution": "Dependency audit is a pre-scheduling precondition; current reference/gate evidence determines fixture schedulability, while live dogfood/sign-off gates broad rollout.",
      "where_resolved": [
        "rollout_plan.preconditions",
        "metrics"
      ]
    },
    {
      "reviewer": "product_owner",
      "issue_id": "PO-R27-002",
      "concern": "Rollback communication omitted in-flight Xcode behavior.",
      "proposal_resolution": "Rollback states that in-flight and new Xcode-brokered executions fail closed immediately while non-Xcode workflows continue.",
      "where_resolved": [
        "rollout_plan.rollback",
        "failure_semantics"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-005",
      "concern": "A single consent modal can still feel like a hang.",
      "proposal_resolution": "Added `Action Required: Check Xcode` state before initialize timeout when consent is plausible.",
      "where_resolved": [
        "ux_ui_notes.operator_visible_behavior",
        "ux_ui_notes.friendly_failure_mapping"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-006",
      "concern": "Parallel DerivedData contention could be mistaken for broker failure.",
      "proposal_resolution": "Added Xcode build concurrency contention as a separate operator-visible class.",
      "where_resolved": [
        "ux_ui_notes.operator_visible_behavior",
        "ux_ui_notes.friendly_failure_mapping"
      ]
    },
    {
      "reviewer": "ui_designer",
      "issue_id": "UI-NB-001/UI-SUG-001",
      "concern": "Policy warnings needed distinct visual treatment.",
      "proposal_resolution": "Specified `exclamationmark.shield`, orange foreground, coalescing, and event type/accessibility hooks.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface",
        "implementation_inventory.swift_ui"
      ]
    },
    {
      "reviewer": "ui_designer",
      "issue_id": "UI-NB-002/UI-SUG-003",
      "concern": "Xcode UI should avoid empty placeholders and raw envelope dumps.",
      "proposal_resolution": "Xcode Runtime renders only when observations exist, uses structured rows, and exposes stable accessibility identifiers.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R28-001",
      "concern": "Direct-command containment needs a scanner contract for current catalog shapes.",
      "proposal_resolution": "`DirectCommandDeclarationScanner` normalizes raw workflow/catalog YAML and typed agent entries into declarations with source path, tokenization mode, matched tool, policy decision, and shim-signal contribution.",
      "where_resolved": [
        "architecture.catalog_lint_and_residual_observer.direct_command_declaration_scanner",
        "implementation_handoff.second_pr",
        "acceptance_criteria"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R28-002",
      "concern": "Broker health must not collapse into global daemon readiness.",
      "proposal_resolution": "`XcodeBrokerHealthSnapshot` is exposed alongside daemon status; global readiness follows shared listener and non-Xcode serve ability while broker health gates only Xcode broker requests.",
      "where_resolved": [
        "architecture.runtime_ownership.daemon",
        "architecture.broker_subsystem_health",
        "failure_semantics",
        "metrics"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R28-003",
      "concern": "XcodeTargetResolver ownership still spanned engine policy and acp host mechanics.",
      "proposal_resolution": "`XcodeTargetResolver` is a trait/service dependency: engine supplies frozen selection inputs, acp performs host probing, and the immutable `XcodeTargetSnapshot` is used for lease keys, observations, and backend spawn.",
      "where_resolved": [
        "architecture.xcode_target_resolver",
        "implementation_inventory.acp",
        "implementation_inventory.engine"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R28-004",
      "concern": "Bounded observation storage remains a hot-row append design.",
      "proposal_resolution": "P051 keeps the bounded envelope but requires the append repository API as the only write path and keeps a normalized event-table migration trigger if dogfood shows retry exhaustion or truncation pressure.",
      "where_resolved": [
        "architecture.durable_observation_schema.storage_bounds",
        "risks_and_mitigations",
        "metrics"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-ISS-001",
      "concern": "`Action Required: Check Xcode` could be ambiguous with multiple Xcode windows.",
      "proposal_resolution": "Recovery text includes workspace identity and Xcode PID when available from `XcodeTargetSnapshot`.",
      "where_resolved": [
        "ux_ui_notes.operator_visible_behavior",
        "ux_ui_notes.friendly_failure_mapping"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-ISS-002",
      "concern": "Bridge progress states must be visible outside the inspector.",
      "proposal_resolution": "`Waiting for Xcode Bridge lock`, `Starting Xcode Bridge`, and `Action Required: Check Xcode` propagate to the run timeline/inspector readback surface; the current app has no distinct production high-level progress view.",
      "where_resolved": [
        "ux_ui_notes.operator_visible_behavior",
        "implementation_inventory.swift_ui"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-NB-001",
      "concern": "Many unique residual paths can still clutter the timeline.",
      "proposal_resolution": "After five unique residual path warnings in one execution, the timeline shows a summary plus `View all residual paths` disclosure.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface"
      ]
    },
    {
      "reviewer": "ux_designer",
      "issue_id": "UX-SUG-001/UX-SUG-002",
      "concern": "First-run consent and visual consistency needed explicit UX guidance.",
      "proposal_resolution": "Agent Catalog shows a one-time consent note for broker-required agents and Xcode Runtime uses existing ForgePanel/GroupBox styling.",
      "where_resolved": [
        "ux_ui_notes.minimum_ui_surface"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R29-001",
      "concern": "Source-of-truth drift remains an implementation-start hazard until the scaffold gate enforces reconciliation.",
      "proposal_resolution": "Source reconciliation or explicit redirect is the first scaffold task, and implementation PR review must fail if work begins from stale checked-in proposal text without the R30 pointer or sync.",
      "where_resolved": [
        "implementation_source_of_truth",
        "rollout_plan.preconditions",
        "implementation_handoff.first_pr",
        "metrics"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R29-002",
      "concern": "Upstream dependency readiness is declared as a gate but not proven in reviewed artifacts.",
      "proposal_resolution": "Dependency audit is a required pre-scheduling artifact with owner, gate status, remaining gap, and parallel-versus-sequential classification; current implemented-system references and gate registrations determine fixture/readback schedulability.",
      "where_resolved": [
        "rollout_plan.preconditions",
        "metrics",
        "implementation_handoff.preconditions"
      ]
    },
    {
      "reviewer": "architect",
      "issue_id": "ARCH-R29-003",
      "concern": "The bounded JSON observation envelope is acceptable for P051 but remains a monitored migration risk.",
      "proposal_resolution": "Dogfood must capture retry exhaustion and truncation counts; sustained pressure triggers a normalized event-table migration before broad `shim_enforced` rollout.",
      "where_resolved": [
        "architecture.durable_observation_schema.storage_bounds",
        "metrics",
        "risks_and_mitigations"
      ]
    }
  ],
  "architecture": {
    "runtime_ownership": {
      "daemon": [
        "Owns the shared loopback listener and mounts the broker router at daemon startup unless `CHAINWORKS_XCODE_BROKER_DISABLED=1` is set.",
        "Constructs shared broker state and broker health reporting.",
        "Starts non-Xcode routes even when Xcode broker state is Disabled or Degraded.",
        "Reports `XcodeBrokerHealthSnapshot` as subsystem health beside, not inside, global daemon readiness."
      ],
      "acp_runtime_manager": [
        "Owns `XcodeMcpBridgePool`, `ProviderCapabilityCache`, lease reservation/rollback guards, shim dispatch lease minting, provider-session lifecycle hooks, and injected observation sink handle.",
        "Does not depend on db and does not write persistence directly."
      ],
      "engine": [
        "Owns the seven-phase executor flow, current execution attribution, release-mode capability-slice checks, and injection of a concrete observation sink into acp.",
        "Updates the active prompt window and `current_execution_id` pointer for reusable provider sessions."
      ],
      "db_domain_api": [
        "domain owns `XcodeRuntimeObservation` typed models and serde names.",
        "db owns migration and transactional append repository API for `agent_executions.actual_xcode_runtime_observation_json`.",
        "GraphQL and MCP expose typed readback without raw token values."
      ]
    },
    "provider_capability_preflight": {
      "adapter_api": [
        "`prepare_launch_spec(&ExecutionRequest) -> ProviderLaunchSpec` derives binary path, ordered argv, capability env, and adapter settings. `credential_env` is empty.",
        "`prepare_session_new_spec(&ExecutionRequest, &[ResolvedMcpServer]) -> SessionNewSpec` constructs `session/new` only after MCP resolution and lease attachment.",
        "`open_session_with_specs(&ProviderLaunchSpec, &SessionNewSpec) -> AcpSessionHandle` launches without deriving or mutating env, args, or MCP payloads."
      ],
      "launch_resource_guard": [
        "`ProviderLaunchSpec` carries or references a `LaunchResourceGuard` that owns fake-home, temp, cache, generated config, and provider runtime-home resources created for a probe or session.",
        "Initialize-only probes use the guard and clean it on probe close or capability failure before any broker lease is allocated.",
        "Real sessions transfer the guard into `AcpSessionHandle` on successful `open_session_with_specs`; cancellation, launch failure, or prelaunch drift failure drops the guard and removes owned resources.",
        "Credential-only additions in Phase 6 do not create unmanaged resources outside the guard."
      ],
      "probe_key": {
        "fields": [
          "adapter_family",
          "runtime_profile_id",
          "binary_fingerprint",
          "ordered launch args fingerprint",
          "capability env fingerprint",
          "adapter settings fingerprint"
        ],
        "binary_fingerprint": "canonical binary path, file size, mtime, and SHA-256 content hash when readable; if content hashing is unavailable, record `binary_hash_unavailable` and include path+size+mtime only.",
        "cache_lifetime": "in-memory for daemon lifetime; any component change forces a fresh initialize-only probe"
      },
      "release_mode_contract": [
        "Compute `CapabilitySliceFingerprint` immediately after `prepare_launch_spec`.",
        "Recompute immediately before `open_session_with_specs`.",
        "Fail before provider launch with `provider_launch_spec_capability_drift` if binary path, ordered argv, capability env, runtime profile, or adapter settings changed.",
        "Exclude `credential_env` from the fingerprint and test that shim-token and socket additions do not affect it."
      ],
      "unsupported_providers": "Codex ACP, Claude Agent ACP, and Gemini CLI are verified for launch scope. Auggie and Junie fail closed for brokered Xcode MCP until their probes prove HTTP MCP support."
    },
    "brokered_xcode_mcp_resolution": {
      "canonical_registry_contract": {
        "registry_entry": "The canonical Xcode MCP entry uses server id or runtime id `xcode` and `type: xcode_broker` or `transport: xcode_broker`; it must not contain executable `command` fields in the final migrated shape.",
        "intent_type": "engine resolves the canonical entry to `BrokeredXcodeMcpIntent`, not directly to stdio.",
        "intent_fields": [
          "extension_id",
          "runtime_id",
          "server_id",
          "workspace_root",
          "xcode_pid_selector",
          "runtime_profile_id",
          "permission_profile_id",
          "resolved_tool_allowlist_hash",
          "provider_http_required"
        ]
      },
      "compatibility_migration": [
        "An existing registry entry keyed as `xcode` with `command = xcrun` and first non-option arg `mcpbridge` is accepted only as a migration compatibility trigger and converted to `BrokeredXcodeMcpIntent` with warning `xcode_mcp_registry_stdio_migrated`.",
        "Any non-canonical direct `xcrun mcpbridge`, direct `mcpbridge`, or absolute-path mcpbridge registry shape fails closed with `xcode_mcp_registry_stale_stdio`.",
        "Multiple enabled canonical Xcode entries fail closed with `xcode_mcp_registry_ambiguous`.",
        "Registry entries that request Xcode MCP but lack a canonical broker id fail closed with `xcode_mcp_registry_missing_canonical_id`."
      ],
      "resolved_transport": "After capability preflight, `attach_broker_leases` mutates placeholder broker intent into an HTTP MCP server payload with real URL and bearer header. Bearers are never persisted in predicted or actual MCP truth."
    },
    "seven_phase_executor_flow": [
      "Phase 1: `prepare_launch_spec` returns a launch spec with empty `credential_env`, a `LaunchResourceGuard`, and records `CapabilitySliceFingerprint`.",
      "Phase 2: `ensure_provider_capabilities` probes or reads cache; HTTP false fails closed.",
      "Phase 3: `resolve_mcp_servers` is pure and returns stdio/platform transports or `BrokeredXcodeMcpIntent` placeholders.",
      "Phase 4: `attach_broker_leases` is the only broker-state mutation for MCP leases. `LeaseReservationGuard` rolls back on drop until committed.",
      "Phase 5: `prepare_session_new_spec` constructs the real `session/new` payload.",
      "Phase 6: `attach_session_credentials` mints shim dispatch token and writes `credential_env` only when `xcode_shim_injection_signal` is true.",
      "Phase 7: release-mode capability fingerprint is rechecked, provider session opens through `open_session_with_specs`, the launch-resource guard transfers into the session handle, and the lease guard commits only after session open succeeds."
    ],
    "http_transport_contract": {
      "resolved_transport_variant": "`ResolvedMcpServerTransport::Http { url, headers }`",
      "session_new_shape": {
        "type": "http",
        "name": "xcode",
        "url": "http://127.0.0.1:<daemon-port>/xcode-mcp/<lease_id>",
        "headers": [
          {
            "name": "Authorization",
            "value": "Bearer <redacted-lease-token>"
          }
        ]
      },
      "stdio_requirement": "Stdio entries also include `type: stdio` so all providers receive canonical ACP discriminated-union shape."
    },
    "broker_backend_model": {
      "process_model": "One initialized backend `xcrun mcpbridge` subprocess is shared per `run_id + Xcode pid + developer_dir`. Leases remain independent HTTP bearer/policy records; the backend registry maps leases to shared session keys, ref-counts ownership, and closes the backend only after the last mapped lease releases.",
      "initialize_serialization": "A per-Xcode-target mutex covers first backend spawn plus real MCP `initialize`. Later leases on the same run/target return the cached initialize result with the caller's JSON-RPC id and do not forward a duplicate `initialize` to `mcpbridge`.",
      "initialized_notification_handling": "Only the first `notifications/initialized` reaches the backend. Duplicate initialized notifications from sibling leases are synthetic no-ops at the broker facade.",
      "tools_parallelism": "Cross-lease tools progress is required, but same run/target leases share one ordered stdio request pump rather than independent backend processes. Requests to one shared backend are serialized by the backend mutex; leases for different run/target keys use independent backend processes. Per-lease HTTP authorization and `BrokerMcpPolicy` filtering/denial happen before any shared backend forwarding.",
      "host_env": [
        "`HOME=<operator home from getpwuid(getuid()) or daemon config override>`",
        "`TMPDIR=<operator Darwin user temp dir>`",
        "`DEVELOPER_DIR=<xcode-select -p>`",
        "`USER` and `LOGNAME` for the operator account",
        "`PATH=/usr/bin:/bin:/usr/sbin:/sbin:<DEVELOPER_DIR>/usr/bin`",
        "`MCP_XCODE_PID=<target Xcode PID>`",
        "No `CODEX_HOME`, no `XDG_CACHE_HOME`, no `CHAINWORKS_*`"
      ],
      "pid_drift": "Xcode PID drift closes stale leases for the old PID with `pool_pid_drift`; new leases target the new PID."
    },
    "capacity_and_backpressure": {
      "defaults": {
        "max_active_leases_per_xcode_pid": 8,
        "max_initialize_queue_per_xcode_pid": 16,
        "max_initializing_per_xcode_pid": 1,
        "initialize_queue_timeout_seconds": 45,
        "backend_spawn_initialize_timeout_seconds": 30,
        "first_connect_deadline_seconds": 60
      },
      "configuration": "Runtime profiles may lower limits; raising limits requires explicit config and is recorded in broker health diagnostics.",
      "failure_classes": [
        "xcode_mcp_capacity_exhausted",
        "xcode_mcp_initialize_queue_timeout",
        "xcode_mcp_initialize_timeout",
        "xcode_mcp_first_connect_timeout"
      ],
      "observations": "Queued, dequeued, rejected, timed-out, and capacity-limit events are appended to `mcp_broker_observations[]` with wait duration and limit values."
    },
    "broker_subsystem_health": {
      "type": "XcodeBrokerHealthSnapshot",
      "owner": "daemon owns publishing; acp broker owns state transitions; UI/API read it as Xcode subsystem health.",
      "states": [
        "Disabled",
        "Healthy",
        "Degraded",
        "Failed"
      ],
      "fields": [
        "state",
        "reason_code",
        "can_acquire_new_xcode_leases",
        "active_lease_count",
        "initialize_queue_depth",
        "last_transition_at",
        "operator_message"
      ],
      "readiness_contract": [
        "Global `/ready` and daemon Ready remain tied to shared listener health and ability to serve non-Xcode routes.",
        "Xcode broker Disabled or Degraded does not make non-Xcode readiness fail.",
        "Brokered Xcode lease acquisition checks `can_acquire_new_xcode_leases` and fails closed with a broker-specific failure class when false.",
        "SwiftUI and MCP/GraphQL read broker health from the subsystem snapshot instead of inferring it from global daemon state."
      ]
    },
    "lease_and_token_lifecycle": {
      "ownership": "Leases are provider-session-owned, not execution-owned. A successful execution does not release its lease if the provider session remains reuse-compatible.",
      "states": [
        "reserved",
        "active",
        "closing",
        "released",
        "orphaned"
      ],
      "release_triggers": [
        "ACP `session/close` or provider stdin EOF",
        "cancellation",
        "timeout",
        "provider crash",
        "operator reset",
        "reuse-incompatible supersession"
      ],
      "http_token_rules": [
        "Bearer tokens are session-lifetime, not single-use.",
        "First connect must occur before the deadline.",
        "After initialize, the lease binds to MCP session id.",
        "Subsequent requests require bearer plus matching `Mcp-Session-Id`.",
        "Only one active HTTP stream per lease is allowed.",
        "Reconnect is allowed only when active stream count is zero and session id matches."
      ]
    },
    "direct_command_guard": {
      "domain_fields": [
        {
          "field": "xcode_broker_required",
          "type": "bool",
          "serde": "xcode_broker_required",
          "default": false,
          "meaning": "Agent requested Xcode MCP; drives broker lease and HTTP MCP entry only."
        },
        {
          "field": "xcode_shim_injection_signal",
          "type": "bool",
          "serde": "xcode_shim_injection_signal",
          "default": false,
          "meaning": "Agent can invoke Xcode shell commands directly; drives PATH shim, shim dispatch lease, and FreshSessionRequired."
        },
        {
          "field": "requires_xcode_host_execution",
          "type": "bool",
          "serde": "requires_xcode_host_execution",
          "default": false,
          "meaning": "Direct Xcode shell commands may be routed through host executor instead of rejected."
        },
        {
          "field": "xcode_prompt_lint_warnings",
          "type": "Vec<XcodePromptLintWarning>",
          "serde": "xcode_prompt_lint_warnings",
          "default": [],
          "meaning": "Soft warnings for prompt/system/description text."
        }
      ],
      "propagation": "Workflow catalog lint writes these fields into compiled plan/domain `ResolvedAgent`; engine copies them into session fingerprints, reports, recovery truth, and Agent Catalog UI metadata.",
      "fingerprint_inputs": "Session reuse fingerprint includes `xcode_broker_required`, broker contract hash, permission profile id, resolved tool allowlist hash, and `xcode_shim_injection_signal`. Any shim-enabled execution forces FreshSessionRequired.",
      "shim_executables": [
        "xcodebuild",
        "simctl",
        "mcpbridge",
        "xcrun"
      ],
      "xcrun_rules": [
        "Run mode: default, `-r <tool>`, `--run <tool>`.",
        "Find mode: `-f <tool>`, `--find <tool>`; finding `xcodebuild`, `simctl`, or `mcpbridge` is rejected to prevent find-then-exec bypass.",
        "Show mode: `--show-sdk-*` passes through.",
        "`--sdk` and `--toolchain` consume the next token.",
        "`-l`, `--log`, `--verbose`, `--no-cache`, `--kill-cache`, `--help`, `-h`, and `--version` do not consume the next token.",
        "Unknown flags fail closed with `xcrun_unknown_option`."
      ],
      "diagnostic_mode": "`CHAINWORKS_XCODE_DIRECT_DIAGNOSTIC=1` may passthrough `xcodebuild`, `simctl`, and non-mcpbridge `xcrun` branches for local debugging. `mcpbridge` is never bypassed."
    },
    "catalog_lint_and_residual_observer": {
      "direct_command_declaration_scanner": {
        "type": "DirectCommandDeclarationScanner",
        "owner": "workflow/catalog crate",
        "inputs": [
          "raw parsed workflow YAML",
          "raw parsed agent catalog YAML",
          "typed agent entries",
          "typed runtime and permission profile ids when available"
        ],
        "output_fields": [
          "source_document",
          "source_path",
          "declaration_kind",
          "tokenization_mode",
          "raw_value",
          "argv_tokens",
          "matched_xcode_tool",
          "policy_decision",
          "contributes_to_xcode_shim_injection_signal",
          "warning_or_error_code"
        ],
        "current_fixture_coverage": [
          "examples/agents/agents.yaml permission profile shell allow entries that mention xcodebuild or simctl",
          "examples/agents/agents.yaml agents[].required_tools entries that mention Xcode tools",
          "run block command paths and argv",
          "tools.shell.commands and adapter-specific shell capability fields represented as raw YAML"
        ],
        "contract": "The scanner operates over raw YAML plus typed structs so fields currently stored as serde_yaml::Value cannot escape linting. Catalog compilation consumes scanner output to set `xcode_shim_injection_signal`, emit hard failures, and persist prompt warnings."
      },
      "hard_fail_fields": [
        "run block command path and argv",
        "run block env values paired with Xcode tools",
        "shell_allowlist",
        "allowed_commands",
        "tools.shell.commands",
        "adapter-specific shell-capability fields",
        "required-tool declarations if represented separately"
      ],
      "hard_fail_patterns": [
        "/usr/bin/xcrun",
        "/usr/bin/xcodebuild",
        "/Applications/Xcode.app/Contents/Developer/",
        "/Applications/Xcode*.app/Contents/Developer/",
        "DEVELOPER_DIR=...xcodebuild",
        "DEVELOPER_DIR=...simctl",
        "direct or absolute `mcpbridge`"
      ],
      "soft_warning_fields": "Prompt, system instruction, and description text only.",
      "residual_observer": "`XcodeResidualPathObserver` watches ACP `session/update` shell-tool events and emits warning observations for prompt-time absolute Xcode paths. It does not block execution. If argv is unavailable, it records `observer_unavailable`."
    },
    "shim_dispatch_and_host_executor": {
      "token": "Separate `XcodeShimDispatchToken`, minted per provider session, readable by the agent and treated as a session identifier rather than a secret.",
      "provider_identity_binding": [
        "The dispatch lease records the provider child pid returned by `open_session_with_specs`, its process group when available, and a tracked descendant set for shim subprocesses.",
        "Socket peers must match the recorded provider process, process group, or tracked descendant policy for that session; same uid alone is insufficient.",
        "Forged claimed provider pid, stale token after close, same-uid cross-session token replay, and process-tree mismatch are rejected before command parsing."
      ],
      "socket_authorization": [
        "Unix-domain socket peer pid/uid/gid verification.",
        "Peer uid must match daemon uid before token validation.",
        "Active prompt window is required.",
        "Workspace root and host-execution policy are frozen at lease creation.",
        "Cwd outside workspace root is rejected.",
        "Only `xcodebuild` and `simctl` may route to host executor.",
        "`mcpbridge` never routes.",
        "Bind each shim dispatch lease to the launched provider child pid plus tracked process group or descendant set created by `open_session_with_specs`.",
        "Reject same-uid peers outside that process identity with `xcode_shim_peer_process_mismatch`; do not accept forged claimed provider pid values as warnings-only.",
        "Fixtures cover same-uid cross-session token replay, stale token after close, forged pid, and process-tree mismatch."
      ],
      "host_executor_env": "Build from scratch, override host-sensitive values, drop provider/internal env, and propagate only explicit build-input allowlist values such as `SCHEME`, `CONFIGURATION`, `DESTINATION`, `CODE_SIGN_STYLE`, and `DEVELOPMENT_TEAM`."
    },
    "simulator_destination_stability": [
      "Pass through `id=<UUID>` destinations.",
      "Rewrite unique `platform=iOS Simulator,name=<name>,OS=<os>` matches to `platform=iOS Simulator,id=<UUID>`.",
      "Reject ambiguous name/OS matches with candidate UUIDs.",
      "Reject not-found and unparseable simulator destinations.",
      "Pass through macOS and generic destinations.",
      "Record original and rewritten destination in `xcode_host_executor_events[]`."
    ],
    "durable_observation_schema": {
      "migration": "Add nullable `agent_executions.actual_xcode_runtime_observation_json TEXT`; legacy and non-Xcode rows read as GraphQL null.",
      "envelope": {
        "mcp_broker_observations": [],
        "xcode_shim_events": [],
        "xcode_host_executor_events": []
      },
      "ownership_contract": [
        "domain crate defines typed envelope, event enums, failure-class serde names, redaction model, and version field.",
        "db crate owns `append_xcode_runtime_observation(agent_execution_id, event)` using transactional read-modify-write and optimistic retry.",
        "engine crate implements the concrete `XcodeRuntimeObservationSink` using the db repo and injects an async handle into `AcpRuntimeManager`.",
        "acp crate owns only the sink trait/callback and calls it for broker, shim, host executor, and residual observer events.",
        "Late async shim events use the active prompt attribution pointer. Idle-window events are rejected and written to an orphan observation bucket when attributable to run/session but not an active execution."
      ],
      "persistence_failure_policy": [
        "Observation append failure never silently drops the event: emit tracing error with metric marker `xcode_observation_persist_failed_total`, include warning marker `observation_persistence_degraded`, increment broker health failure count, and mark broker health Degraded.",
        "If the same DB append path is unavailable, do not attempt a recursive warning append through that failed sink; the degraded health snapshot and error-level metric marker become the operator evidence path until persistence recovers.",
        "Persistence failure does not by itself fail an otherwise successful Xcode command, but the full P051 gate requires persistence success in fixtures."
      ],
      "api_readback": [
        "GraphQL exposes typed `actualXcodeRuntimeObservation`.",
        "MCP `reports.get` returns the same envelope at `execution.xcode_runtime_observation`.",
        "No raw bearer token or shim token is stored or returned."
      ],
      "storage_bounds": {
        "max_events_per_execution_default": 1000,
        "max_bytes_per_execution_default": 1048576,
        "append_retry_limit": 3,
        "retry_policy": "optimistic retry with short bounded backoff; exceeding the retry limit emits `observation_persistence_degraded` and increments `xcode_observation_persist_failed_total`",
        "corrupt_json_recovery": "Do not drop the new event. Preserve the corrupt prior blob in failure evidence when possible, replace the stored envelope with a recovery envelope containing `observation_prior_json_corrupt`, and append the new event after redaction.",
        "truncation_policy": "When limits are reached, retain summary counters plus the newest events needed for acceptance evidence; emit `xcode_observation_truncated` and expose truncation in GraphQL/MCP/UI.",
        "late_append_notification": "Late async appends publish an execution update so GraphQL/MCP/UI readback can refresh without requiring log scraping.",
        "single_write_path": "All writers use the db append repository API; direct whole-column replacement is prohibited outside migration/backfill code.",
        "normalized_table_escape_hatch": "If dogfood shows sustained pressure, file and prioritize a normalized execution-event table migration before broad `shim_enforced` rollout. Sustained pressure means retry exhaustion in any dogfood run, truncation in more than 1 percent of brokered Xcode executions, or repeated append latency/backoff spikes across two consecutive dogfood runs."
      }
    },
    "session_reuse": [
      "A live provider session is reuse-compatible only if existing reuse disposition allows reuse, the accepted MCP server set matches, the live Xcode broker lease is active, the broker contract hash matches, and neither side has `xcode_shim_injection_signal = true`.",
      "Pure Xcode MCP agents can reuse sessions and append a new broker observation with `backend_start_disposition = reused_existing_provider_session_lease`.",
      "Shim-enabled agents force FreshSessionRequired to preserve per-execution observation truth and avoid cross-prompt attribution ambiguity."
    ],
    "xcode_target_resolver": {
      "owner": "trait/service dependency shared across engine and acp: engine supplies frozen selection inputs from compiled run/runtime truth; acp owns host-env probing and Xcode process inspection; neither side reconstructs the other's state.",
      "input": [
        "XcodeTargetSelectionInput from engine: workspace_root, runtime_profile_id, optional configured Xcode app or pid selector, permission_profile_id, broker contract hash",
        "HostProbeContext from acp: current macOS GUI session, candidate Xcode processes, host user/home/temp/developer-dir probes"
      ],
      "output": [
        "XcodeTargetSnapshot",
        "xcode_pid",
        "workspace_identity",
        "developer_dir",
        "operator_home",
        "darwin_tmpdir",
        "selection_confidence",
        "failure_class"
      ],
      "snapshot_contract": "The immutable `XcodeTargetSnapshot` is stored on the lease, included in lease keys and observations, and passed to backend spawn. acp must not infer workspace/runtime policy from global state after the snapshot is created.",
      "selection_rules": [
        "Prefer an explicit runtime-profile Xcode PID or app/workspace binding when configured and verify it is alive.",
        "When no explicit PID is configured, match visible Xcode processes by workspace identity instead of newest-process heuristics.",
        "Fail closed with `xcode_target_not_found` when no Xcode process matches the workspace.",
        "Fail closed with `xcode_target_ambiguous` when multiple live Xcode processes match and no selector disambiguates them.",
        "Fail closed with `host_env_unavailable` when the selected process does not belong to the expected GUI user or host home cannot be resolved.",
        "Record target resolver evidence in `mcp_broker_observations[]` before backend spawn."
      ],
      "stale_behavior": "Direct `pgrep -n -x Xcode` selection is prohibited for brokered P051 paths and is a stale-guidance pattern in the scaffold static gate."
    },
    "broker_mcp_policy": {
      "owner": "broker HTTP facade in acp; constructed from permission_profile_id and resolved_tool_allowlist_hash",
      "interface": "BrokerMcpPolicy { filter_tools_list(lease, tools) -> tools; authorize_tools_call(lease, tool_name) -> Allow|Deny }",
      "rules": [
        "Filter `tools/list` per lease before returning provider-visible tools.",
        "Deny `tools/call` for tools outside the resolved allowlist even if the backend advertises them.",
        "Persist denied calls as broker observations without forwarding them to `mcpbridge`.",
        "Keep sibling leases isolated: a denial or filtered list for one lease does not mutate another lease policy.",
        "Redact bearer values while preserving requested, predicted, actual, and denied MCP truth separately."
      ],
      "acceptance_fixtures": [
        "tools_list_filtered_per_lease",
        "tools_call_denied_without_backend_forward",
        "denied_observation_persisted",
        "sibling_lease_policy_isolation"
      ]
    },
    "launch_resource_ownership": {
      "owner": "ProviderLaunchSpec owns a LaunchResourceGuard for fake-home, temp, cache, and generated provider config resources created before probes or sessions.",
      "lifecycle": [
        "Probe path: guard creates resources, initialize-only probe runs, stdin closes, guard cleans up unless cache logic marks reusable immutable resources safe to keep.",
        "Real session path: guard transfers ownership to AcpSessionHandle only after `open_session_with_specs` succeeds.",
        "Failure before provider launch: guard drops and removes resources before broker leases or shim credentials are allocated.",
        "Failure after lease reservation but before session open: LaunchResourceGuard and LeaseReservationGuard both roll back.",
        "Session close/crash/cancel: AcpSessionHandle cleanup releases resources and broker leases in deterministic order."
      ],
      "acceptance_fixtures": [
        "probe_resource_cleanup",
        "real_session_resource_transfer",
        "capability_failure_no_resource_leak",
        "lease_failure_resource_rollback"
      ]
    }
  },
  "failure_semantics": [
    {
      "failure": "Provider lacks HTTP MCP capability",
      "behavior": "Fail before lease/token/backend/session-new/shim allocation with `provider_http_mcp_unsupported`."
    },
    {
      "failure": "Broker disabled by kill switch",
      "behavior": "Daemon serves non-Xcode workflows after restart; in-flight and new Xcode broker requests fail closed with `xcode_broker_disabled` and no stdio fallback."
    },
    {
      "failure": "Broker route or host env unavailable at startup",
      "behavior": "Daemon global readiness may remain Ready when non-Xcode routes are healthy; `XcodeBrokerHealthSnapshot` becomes Degraded and Xcode lease acquisition fails closed until health recovers."
    },
    {
      "failure": "Shared daemon listener cannot start",
      "behavior": "Daemon exits non-zero because no workflows can be served."
    },
    {
      "failure": "Capacity exhausted",
      "behavior": "Reject new lease reservations with `xcode_mcp_capacity_exhausted`; existing leases continue."
    },
    {
      "failure": "Backend bridge crashes",
      "behavior": "Fail the request with backend failure, remove the crashed shared backend session and all mapped lease-to-session bindings, then let surviving leases retry through a fresh backend. Leases on different run/target backends continue."
    },
    {
      "failure": "Xcode PID drift",
      "behavior": "Close stale-PID leases and backends with `pool_pid_drift`; new leases target new PID."
    },
    {
      "failure": "Provider never connects after session-new",
      "behavior": "First-connect deadline expires; lease becomes orphaned and backend closes."
    },
    {
      "failure": "Xcode consent likely blocking startup",
      "behavior": "After five seconds in bridge startup, emit `xcode_mcp_action_required`; if the initialize deadline still expires, fail with `xcode_mcp_initialize_timeout`."
    },
    {
      "failure": "Second concurrent HTTP stream on same lease",
      "behavior": "Reject with `xcode_mcp_concurrent_stream_rejected`."
    },
    {
      "failure": "Broker policy denies MCP tool call",
      "behavior": "Reject at the HTTP facade with `xcode_mcp_tool_denied`, persist denied MCP truth, and do not forward to backend stdio."
    },
    {
      "failure": "Shim peer process does not match provider session",
      "behavior": "Reject with `xcode_shim_peer_process_mismatch`; same-uid cross-session requests are not accepted as warnings-only."
    },
    {
      "failure": "Shim dispatch outside active prompt",
      "behavior": "Reject with `xcode_shim_no_active_prompt` and write orphan observation when attributable."
    },
    {
      "failure": "Simulator name/OS ambiguous",
      "behavior": "Reject with `simulator_destination_ambiguous` and candidate UUIDs."
    },
    {
      "failure": "Parallel build resource contention",
      "behavior": "Classify as `xcode_build_concurrency_contention`; do not mark broker health degraded unless broker events also failed."
    }
  ],
  "security_and_isolation": [
    "ACP provider processes keep fake `HOME`, isolated `CODEX_HOME`, and provider-scoped temp/cache state.",
    "Only broker-owned Xcode subprocesses receive host-user Xcode environment.",
    "HTTP MCP uses per-lease bearer, MCP session-id binding, and single-active-stream enforcement.",
    "Shim dispatch uses a separate Unix socket token, peer uid verification, and provider process/process-group/descendant binding.",
    "Tokens are redacted in logs, observations, GraphQL, and MCP reports.",
    "The host executor allowlist is limited to `xcodebuild` and `simctl`.",
    "`mcpbridge` is never routed to host executor and is never exposed as a provider shell capability inside the enforced boundary.",
    "Provider env is never passed wholesale to host Xcode subprocesses.",
    "Permission profile id and resolved tool allowlist hash are part of the broker contract and reuse/fingerprint checks.",
    "Broker HTTP facade enforces MCP tool policy before backend forwarding so the broker cannot become a transparent policy bypass.",
    "A focused security review of bearer lifecycle, shim token replay, same-uid socket peers, provider process binding, cwd policy, env allowlist, token redaction, diagnostic mode, and loopback surface is required before the host-executor PR merges."
  ],
  "rollout_plan": {
    "enablement_states": [
      {
        "state": "disabled",
        "meaning": "`CHAINWORKS_XCODE_BROKER_DISABLED=1`; broker lease allocation and shim injection are disabled, non-Xcode workflows continue, and Xcode broker requests fail closed."
      },
      {
        "state": "shadow_observe",
        "meaning": "Catalog lint and capability probes run and write observations, but Xcode MCP brokering is not required. This is for local dogfood only and cannot satisfy final acceptance."
      },
      {
        "state": "broker_required_for_xcode_mcp",
        "meaning": "Xcode MCP requests must use brokered HTTP; no stdio fallback."
      },
      {
        "state": "shim_enforced",
        "meaning": "Direct-command lint, PATH shim, dispatch socket, host executor, and residual observer are enforced."
      }
    ],
    "milestones": [
      {
        "gate": "p051-scaffold",
        "scope": [
          "schema/domain/API scaffolding behind nullable readback",
          "adapter API split",
          "initialize-only capability probe/cache",
          "release-mode capability fingerprint invariant",
          "HTTP MCP transport serialization",
          "broker route construction",
          "host-user env fixture tests",
          "registry-to-`BrokeredXcodeMcpIntent` resolution and stale/ambiguous fail-closed fixtures"
        ]
      },
      {
        "gate": "proposal-051|p051",
        "scope": [
          "full broker backend lifecycle",
          "capacity and backpressure policy",
          "PATH shims and catalog lint",
          "shim dispatch socket",
          "host executor and simulator UUID rewrite",
          "durable observations with GraphQL/MCP/UI readback",
          "catalog migration",
          "dogfood evidence",
          "static research consistency check"
        ]
      }
    ],
    "steps": [
      "Land typed domain models and nullable DB/API readback.",
      "Add observation sink trait, db append API, and engine-injected sink with persistence-failure handling.",
      "Add adapter API split, initialize-only probe/cache, stronger binary fingerprinting, and release-mode capability slice check.",
      "Add `BrokeredXcodeMcpIntent`, registry migration contract, HTTP transport enum, and provider capability fail-closed behavior.",
      "Add broker route, lease model, enablement states, kill switch, health, and capacity/backpressure policy.",
      "Add host-user Xcode env builder and backend `mcpbridge` lifecycle with per-PID initialize serialization.",
      "Add per-backend ordered request pump, id rewriting, cancellation handling, and crash attribution.",
      "Add direct-command catalog lint, Xcode signals, prompt warnings, PATH shims, and shim dispatch socket.",
      "Add host executor routing for `xcodebuild`/`simctl` and simulator destination rewrite.",
      "Add residual path observer and high-visibility UI warning presentation.",
      "Add RunTimelineInspectorView, FailedStageEvidencePanel, AgentCatalogView, and broker health presentation mappings.",
      "Migrate example agent catalogs to explicit `requires_xcode_host_execution` where needed.",
      "Register `p051-scaffold`, `proposal-051`, and `p051` in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.",
      "Run scaffold gate, full fixture gate, then dogfood on a parallel Gemini UX/UI proposal-review stage."
    ],
    "rollback": "Set `CHAINWORKS_XCODE_BROKER_DISABLED=1` and restart the daemon to preserve non-Xcode workflows while refusing Xcode broker requests. During rollback, all in-flight and new Xcode-brokered executions fail closed immediately with `xcode_broker_disabled`; this is a rollback switch, not a production fallback for brokered Xcode MCP.",
    "preconditions": [
      "Before scheduling implementation, create `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md` with one row per dependency (P025, P026, P029, P037, P049) covering proposal id, canonical evidence link, owner, gate status, remaining gaps, parallel-vs-sequential decision, and blocker threshold. P025 or P026 incompleteness blocks the scaffold sprint unless the audit explicitly narrows PR1 to independent work.",
      "Before the first implementation PR starts, confirm the checked-in proposal is the canonical source by removing stale source-of-truth guidance. This is the first scaffold task, and implementation review fails if PRs begin from stale checked-in proposal text.",
      "Before the third PR merges, assign a security reviewer for bearer lifecycle, shim token replay, provider process binding, Unix socket peer checks, loopback HTTP surface, cwd policy, env allowlist, token redaction, and diagnostic-mode bypasses."
    ]
  },
  "metrics": [
    {
      "metric": "p051-scaffold gate",
      "threshold": "`./scripts/test-gate.sh p051-scaffold` passes."
    },
    {
      "metric": "Full P051 gate",
      "threshold": "`./scripts/test-gate.sh proposal-051` and `./scripts/test-gate.sh p051` pass."
    },
    {
      "metric": "Provider capability cache correctness",
      "threshold": "A second call with an identical `ProbeKey` is a cache hit; any binary, argv, capability env, runtime profile, or adapter settings change is a miss."
    },
    {
      "metric": "Unsupported-provider fail-closed behavior",
      "threshold": "Fixture proves no lease, token, backend, shim token, or `session/new` allocation occurs when HTTP MCP is false."
    },
    {
      "metric": "Parallel Xcode MCP startup",
      "threshold": "At least two parallel sessions against the same Xcode PID get isolated HTTP leases and policies, share one initialized backend for the same run/target, forward only one real initialize, and both complete `tools/list`/`tools/call` through the broker."
    },
    {
      "metric": "Modal dedup dogfood",
      "threshold": "Dogfood on at least two parallel Gemini Xcode-capable sessions against the same Xcode process shows at most one Xcode consent modal per Xcode process."
    },
    {
      "metric": "Parallel startup latency evidence",
      "threshold": "Dogfood sign-off records wall-clock time from first provider launch to both sessions initialized, per-session initialize wait, and whether any `Action Required: Check Xcode` state was shown."
    },
    {
      "metric": "Fake-home failures",
      "threshold": "Zero catalog-declared or PATH-based Xcode fake-home failures in the dogfood run."
    },
    {
      "metric": "Observation completeness",
      "threshold": "100 percent of brokered Xcode executions have `mcp_broker_observations[]`; 100 percent of shim rejections/routes have `xcode_shim_events[]`; 100 percent of routed commands have `xcode_host_executor_events[]`."
    },
    {
      "metric": "UI visibility",
      "threshold": "Dogfood evidence includes screenshots or UI proof showing Xcode Runtime details, policy warnings, friendly failure text, and Agent Catalog infrastructure flags."
    },
    {
      "metric": "Token redaction",
      "threshold": "No raw MCP bearer or shim token appears in logs, tracing, GraphQL, MCP reports, stored observations, or UI."
    },
    {
      "metric": "Dogfood approver",
      "threshold": "Named operator or release owner signs off with run id, provider versions, Xcode PID, session count, modal count, and observation completeness evidence."
    },
    {
      "metric": "Source proposal reconciliation",
      "threshold": "`p051-scaffold` fails until checked-in `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` is the canonical implementation source and stale guidance is removed. No implementation PR may begin until no stale no-UI, debug_assert-only, path+mtime+size-only, drop-on-corrupt, pgrep-newest-Xcode, and same-uid-only shim guidance remains."
    },
    {
      "metric": "Dependency audit readiness",
      "threshold": "Written table exists at `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md` before scheduling with proposal id, owner, gate status, remaining gaps, parallel-vs-sequential classification, and blocking threshold; current reference/gate truth distinguishes fixture blockers from broad rollout blockers."
    },
    {
      "metric": "Broker policy enforcement",
      "threshold": "Fixtures prove per-lease tools/list filtering, tools/call denial without backend forwarding, denied-observation persistence, and sibling-lease isolation."
    },
    {
      "metric": "Shim dispatch identity binding",
      "threshold": "Fixtures reject same-uid cross-session token replay, stale token after session close, forged provider pid, and peer process-tree mismatch."
    },
    {
      "metric": "Observation storage bounds",
      "threshold": "Fixtures cover retry exhaustion, corrupt prior JSON recovery without dropping the new event, truncation signaling, and late-append UI/API refresh."
    },
    {
      "metric": "Dogfood latency evidence",
      "threshold": "Dogfood sign-off records p50 and max wall-clock startup latency for the parallel Xcode session pair; max startup latency must stay below the configured initialize queue timeout and must be included in release evidence."
    },
    {
      "metric": "Residual path warning ratio",
      "threshold": "Dogfood reports residual prompt-time absolute-path warnings by provider and agent. If residual warnings dominate Xcode failures, create and complete rollout follow-up `P051-FU-01` before enabling `shim_enforced` broadly."
    },
    {
      "metric": "Direct command scanner coverage",
      "threshold": "Fixtures prove `DirectCommandDeclarationScanner` detects Xcode commands in permission profile shell allow entries, `agents[].required_tools`, run blocks, tools.shell commands, and adapter-specific raw YAML command fields."
    },
    {
      "metric": "Broker subsystem health separation",
      "threshold": "Fixture proves Xcode broker Disabled/Degraded changes `XcodeBrokerHealthSnapshot` and Xcode lease behavior without failing global daemon readiness for non-Xcode routes."
    },
    {
      "metric": "Target resolver boundary",
      "threshold": "Fixture proves engine-provided `XcodeTargetSelectionInput` and acp host probing produce an immutable `XcodeTargetSnapshot`; acp does not reconstruct workspace/runtime selection policy from global state."
    },
    {
      "metric": "Observation hot-row escape trigger",
      "threshold": "Dogfood report records observation append retry exhaustion, truncation counts, and append backoff/latency spikes. Any retry exhaustion, truncation above 1 percent of brokered Xcode executions, or repeated append pressure across two dogfood runs creates rollout follow-up `P051-FU-02` (normalized event-table work) before broad `shim_enforced` rollout."
    }
  ],
  "risks_and_mitigations": [
    {
      "risk": "Proposal source drift creates multiple potential implementation contracts.",
      "mitigation": "The checked-in proposal is the canonical source; source reconciliation is a pre-implementation condition and stale contradictory guidance fails `p051-scaffold`."
    },
    {
      "risk": "Providers change HTTP MCP wire shape or capability reporting.",
      "mitigation": "Probe each unique launch shape, key cache by full capability slice, and fail closed with fixtures for supported providers."
    },
    {
      "risk": "Capability probe diverges from real session launch.",
      "mitigation": "Adapter API split plus release-mode capability fingerprint check before provider launch."
    },
    {
      "risk": "Shared backend serializes same-target tools traffic and may couple sibling leases through one stdio process.",
      "mitigation": "Per-lease policy and authorization are enforced before forwarding, backend failure closes all mapped leases so retry gets a fresh backend, observations record backend PID/disposition, and dogfood proves two sibling leases complete `tools/list`/`tools/call` while showing at most one modal per Xcode process."
    },
    {
      "risk": "Initialize serialization becomes a bottleneck.",
      "mitigation": "Lock only first spawn plus initialize, return cached initialize results to sibling leases, report waiting status, record wait time, and keep different run/target keys on independent backend processes."
    },
    {
      "risk": "Host-user env builder picks wrong GUI user.",
      "mitigation": "Resolve via `getpwuid(getuid())`, allow daemon config override, warn at startup on mismatch, and show `host_env_unavailable` action text."
    },
    {
      "risk": "Multiple Xcode processes or workspaces make target selection ambiguous.",
      "mitigation": "Use `XcodeTargetResolver` with engine-provided selection inputs and acp host probing, fail closed on ambiguity/no match, record immutable snapshot evidence, and prohibit newest-process production heuristics."
    },
    {
      "risk": "Raw catalog fields or permission profiles hide direct Xcode commands from lint.",
      "mitigation": "`DirectCommandDeclarationScanner` scans raw YAML plus typed agent entries and fixtures current permission profile shell allow entries and `agents[].required_tools`."
    },
    {
      "risk": "Prompt-time absolute paths still execute outside the enforced boundary.",
      "mitigation": "Document residual, add high-visibility policy warnings, measure residual frequency in dogfood, and file a sandbox follow-up if warnings dominate failures."
    },
    {
      "risk": "Agents read shim token from env.",
      "mitigation": "Do not rely on secrecy; enforce server-side token validity, provider process binding, peer uid, workspace root, active prompt, and frozen host-execution policy."
    },
    {
      "risk": "Broker relays MCP calls that policy should deny.",
      "mitigation": "`BrokerMcpPolicy` filters `tools/list`, denies unauthorized `tools/call` before backend forwarding, and persists denied MCP truth."
    },
    {
      "risk": "Observation writes race under fan-out or parallel shim calls.",
      "mitigation": "Transactional db append API with optimistic retry, event/byte limits, corrupt-json recovery, truncation summaries, and fixtures for concurrent shim observation appends."
    },
    {
      "risk": "Bounded JSON observation envelope becomes a hot row under dogfood load.",
      "mitigation": "Append repository API is the only write path; dogfood records retry/truncation pressure and triggers a normalized event-table follow-up before broad rollout if pressure appears."
    },
    {
      "risk": "Xcode broker health degrades global daemon readiness.",
      "mitigation": "`XcodeBrokerHealthSnapshot` is subsystem health; global readiness remains tied to shared listener and non-Xcode serve ability."
    },
    {
      "risk": "Parallel agents contend on DerivedData or Xcode build resources.",
      "mitigation": "Classify contention separately from broker failures, expose recovery guidance, and measure in dogfood before tuning parallel fan-out."
    },
    {
      "risk": "Rollout destabilizes non-Xcode workflows.",
      "mitigation": "Broker disabled/degraded states preserve non-Xcode daemon functionality; rollback switch refuses Xcode broker requests without stdio fallback."
    },
    {
      "risk": "Minimum UI scope expands implementation size.",
      "mitigation": "Limit UI to read-only existing surfaces: timeline inspector, failed-stage evidence, catalog detail metadata, and broker health indicator."
    }
  ],
  "acceptance_criteria": [
    "Research artifact exists, has an allowed verdict, and implemented scope matches `Proceed with scoped architecture`.",
    "`BrokeredXcodeMcpIntent` registry contract is implemented with canonical id matching, stale stdio migration warning, stale/ambiguous fail-closed errors, and redacted predicted/actual MCP truth.",
    "`ProviderCapabilityCache` and initialize-only probe are implemented and keyed by full `ProbeKey` including strengthened binary fingerprint.",
    "Adapter API split is complete; old `open_session(&ExecutionRequest)` is retired.",
    "`CapabilitySliceFingerprint` is enforced in release mode before provider launch.",
    "Executor follows the seven-phase flow and rolls back reserved state on failure.",
    "HTTP MCP transport is serialized in canonical ACP discriminated-union shape.",
    "Provider HTTP unsupported fails before lease/token/backend/session-new/shim allocation.",
    "Parallel Xcode MCP sessions get isolated HTTP leases and policies mapped to one shared initialized backend per run/Xcode target; only one real `initialize` reaches `mcpbridge`; sibling lease `tools/*` calls complete through the ordered backend pump.",
    "Each shared backend uses a per-backend ordered request pump for the stdio process and leases for different run/target keys use independent backend processes.",
    "Broker capacity and backpressure defaults are enforced and observable.",
    "Broker enablement states and `CHAINWORKS_XCODE_BROKER_DISABLED=1` rollback behavior are implemented without adding a stdio fallback.",
    "Broker-owned Xcode subprocesses run with host-user `HOME` and `TMPDIR`; ACP providers keep fake-home isolation.",
    "PATH shim injection happens only for `xcode_shim_injection_signal`, not for pure `xcode_broker_required`.",
    "Catalog lint hard-fails structured absolute Xcode paths and soft-warns prompt text.",
    "Direct `mcpbridge` is rejected for PATH-based and catalog-declared structured invocations regardless of host-execution opt-in or diagnostic mode.",
    "`XcodeShimDispatchToken` is separate from MCP bearer, provider-session-scoped, and server-side authorized.",
    "Shim-enabled executions force fresh provider sessions; pure Xcode MCP remains session-reuse-compatible.",
    "Host executor routes only `xcodebuild` and `simctl`, with cwd boundary, env allowlist, and simulator UUID handling.",
    "`actual_xcode_runtime_observation_json` exists and exposes typed append-only evidence through DB/domain, GraphQL, MCP reports, and the minimum UI surface.",
    "Observation sink ownership respects crate boundaries: acp has no db dependency, engine injects sink, db owns transactional append.",
    "Failure classes have friendly titles and suggested actions in the UI error presentation layer.",
    "Xcode PID drift, broker infrastructure failure, backend crash, first-connect timeout, capacity exhaustion, and observation persistence failure have deterministic semantics.",
    "Example catalogs are migrated to explicit `requires_xcode_host_execution` where needed.",
    "`p051-scaffold`, `proposal-051`, and `p051` are registered in `scripts/test-gate.sh` and `docs/reference/test-gates.md`.",
    "Dogfood pass on a parallel Xcode-capable stage shows at most one Xcode consent modal per Xcode process, zero enforced-boundary fake-home failures, and 100 percent observation completeness.",
    "Checked-in `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` is the canonical P051 implementation source; `p051-scaffold` fails on stale contrary source-proposal guidance.",
    "`XcodeTargetResolver` replaces newest-process pgrep selection for brokered paths and fails closed on no match, ambiguity, GUI-user mismatch, or workspace drift.",
    "`BrokerMcpPolicy` filters `tools/list`, denies unauthorized `tools/call` before backend forwarding, persists denied observations, and isolates sibling leases.",
    "`LaunchResourceGuard` owns provider fake-home/temp/config resources across initialize-only probes, real-session transfer, rollback, cancellation, and crash cleanup.",
    "Shim dispatch authorization binds token use to the launched provider process identity or tracked descendant set, not only same-uid peer credentials.",
    "Observation append semantics define event/byte bounds, retry limits, corrupt-json recovery, truncation signaling, and late-append readback refresh.",
    "Run timeline shows `Action Required: Check Xcode` when bridge startup is blocked long enough that an Xcode consent modal is likely.",
    "Policy warnings use distinct timeline treatment and coalesce repeated identical residual warnings per execution.",
    "Simulator ambiguity UI exposes candidate UUIDs in operator-usable form rather than only a generic error.",
    "A targeted security review is assigned before host executor/shim dispatch socket merge.",
    "`DirectCommandDeclarationScanner` covers raw catalog/workflow YAML and typed agent entries, including permission profile shell allow entries and `agents[].required_tools`.",
    "`XcodeBrokerHealthSnapshot` is subsystem health separate from global daemon readiness and gates only brokered Xcode lease acquisition.",
    "`XcodeTargetResolver` uses engine-provided selection inputs plus acp host probing to return immutable `XcodeTargetSnapshot`; acp does not reconstruct runtime/workspace policy from global state.",
    "Observation writes go only through the append repository API, with dogfood retry/truncation pressure recorded as the trigger for normalized event-table follow-up.",
    "RunTimelineInspectorView and related run readback surfaces show bridge lock/start/action-required states; there is no distinct production high-level progress view in the current app.",
    "`Action Required: Check Xcode` recovery text includes workspace identity and Xcode PID when available.",
    "Residual path warnings collapse behind `View all residual paths` after more than five unique warnings per execution.",
    "Agent Catalog shows a first-run Xcode consent note for broker-required agents and uses existing ForgePanel/GroupBox styling for the runtime section."
    ,"Source proposal reconciliation or explicit redirect is the first scaffold task; PR review fails if implementation begins from stale checked-in P051 proposal text."
    ,"Dependency audit artifact exists before scheduling with proposal id, owner, gate status, remaining gaps, parallel-vs-sequential classification, and blocking threshold for P025/P026/P029/P037/P049, using implemented-system reference/gate truth."
    ,"Observation dogfood evidence includes retry exhaustion, truncation percentage, and append backoff/latency spikes; sustained pressure triggers normalized event-table follow-up before broad `shim_enforced` rollout."
  ],
  "open_questions": [
    {
      "question": "Should broker idle grace remain fixed at 60 seconds or become runtime-profile configurable?",
      "default_for_implementation": "Keep fixed at 60 seconds for scaffold; resolve before final dogfood."
    },
    {
      "question": "Should live broker debug state later be exposed through a northbound status endpoint beyond durable execution observations?",
      "default_for_implementation": "Not required for P051 acceptance; UI health indicator can use daemon health state."
    },
    {
      "question": "Does Xcode `mcpbridge` expose workspace-sensitive state that requires invalidating `tools/list` on more than PID drift and workspace switch?",
      "default_for_implementation": "Invalidate on PID drift and workspace switch; add observation for cache invalidation reason."
    },
    {
      "question": "Should policy-separated leases key on permission profile id, resolved tool allowlist hash, or both?",
      "default_for_implementation": "Neither for backend identity in P051. Per-lease `BrokerMcpPolicy` remains authoritative at the HTTP facade; the shared backend key intentionally excludes permission profile and allowlist hash until a follow-up proves `mcpbridge` has policy-sensitive mutable state. Denied calls never forward to the backend."
    },
    {
      "question": "Are P025, P026, P029, P037, and P049 all implemented enough for P051 to start?",
      "default_for_implementation": "Create a dependency status audit at `docs/proposals/051-shared-xcode-mcp-bridge-pool.review/dependency-audit.md` before scheduling P051 implementation; P025 or P026 incompleteness blocks scheduling, while partial status in P037/P049 requires an explicit parallel-vs-sequential plan."
    }
  ],
  "implementation_handoff": {
    "first_pr": {
      "goal": "Wire-safe substrate with no real Xcode backend spawning yet.",
      "deliverables": [
        "Checked-in proposal is canonical source-of-truth and stale guidance is removed",
        "static stale-guidance check",
        "domain/db/API nullable observation scaffold",
        "observation sink trait and engine-injected db sink",
        "adapter API split",
        "initialize-only provider probe and cache",
        "release-mode `CapabilitySliceFingerprint` check",
        "HTTP MCP transport serialization",
        "`BrokeredXcodeMcpIntent` registry resolution and fail-closed fixtures",
        "LaunchResourceGuard for probe/session resources"
      ],
      "gate": "p051-scaffold"
    },
    "second_pr": {
      "goal": "Broker lease lifecycle and host-user backend execution under fixture backends.",
      "deliverables": [
        "daemon route and broker health states",
        "lease reservation/commit/rollback",
        "capacity and backpressure enforcement",
        "host-user env builder",
        "backend lifecycle abstraction",
        "per-backend ordered request pump",
        "cancellation, crash, first-connect timeout, and PID drift fixtures",
        "XcodeTargetResolver service boundary with engine-provided selection inputs, acp host probing, immutable snapshot, and fail-closed ambiguity/no-match behavior",
        "BrokerMcpPolicy tools/list and tools/call enforcement fixtures"
      ],
      "gate": "p051-scaffold plus focused broker fixture tests"
    },
    "third_pr": {
      "goal": "Direct-command containment and durable operator evidence.",
      "deliverables": [
        "catalog lint and Xcode signal propagation",
        "DirectCommandDeclarationScanner over raw YAML and typed agent entries",
        "PATH shims and shim dispatch socket",
        "host executor for `xcodebuild` and `simctl`",
        "simulator UUID rewrite",
        "residual path observer",
        "typed GraphQL/MCP readback",
        "minimum SwiftUI read-only surfaces and friendly failure mapping",
        "provider-process-bound shim dispatch authorization",
        "bounded observation append/truncation/recovery semantics",
        "targeted security review evidence"
      ],
      "gate": "proposal-051|p051"
    },
    "pre_ship": {
      "goal": "Dogfood and sign-off.",
      "deliverables": [
        "parallel Gemini Xcode-capable dogfood run",
        "modal count evidence",
        "observation completeness evidence",
        "no token leakage proof",
        "operator or release-owner sign-off"
      ],
      "gate": "proposal-051|p051 plus dogfood acceptance"
    },
    "rollout_followups": [
      {
        "followup_id": "P051-FU-01",
        "trigger": "Residual absolute-path warnings dominate Xcode failures or residual warnings remain unresolved after two dogfood runs.",
        "owner": "chainworks security hardening",
        "scope": "Implement and land sandbox/libc-audit command-boundary hardening with command-origin attribution, warning suppression evidence, and follow-up regression coverage.",
        "acceptance": "No unmitigated residual warning channel remains before broad `shim_enforced` rollout; follow-up evidence is attached to rollout package with sign-off."
      },
      {
        "followup_id": "P051-FU-02",
        "trigger": "Observation append retry exhaustion or append pressure threshold is exceeded in dogfood evidence.",
        "owner": "control-plane persistence team",
        "scope": "Implement normalized observation event-table path, migration guardrails, and backfill/runbook updates for high-volume append resilience.",
        "acceptance": "Retry exhaustion and truncation pressure fall to accepted thresholds on the same workload profile before enabling broad `shim_enforced`; post-follow-up evidence is attached to rollout package."
      }
    ],
    "preconditions": [
      "Before scheduling implementation, complete a dependency audit for P025, P026, P029, P037, and P049 with owner, implementation gate status, remaining gaps, and parallel-versus-sequential constraint. P025 or P026 incompleteness blocks the scaffold sprint.",
      "Before the first implementation PR starts, confirm `docs/proposals/051-shared-xcode-mcp-bridge-pool.md` is the checked-in canonical source and remove stale source-of-truth guidance before implementation begins.",
      "Before the third PR merges, assign a security reviewer for bearer lifecycle, shim token replay, provider process binding, Unix socket peer checks, loopback HTTP surface, cwd policy, env allowlist, token redaction, and diagnostic-mode bypasses."
    ]
  },
  "implementation_inventory": {
    "acp": [
      "manager: broker, capability cache, lease guards, shim credential attachment, injected observation sink",
      "provider_probe: initialize-only capability probe and ProbeKey",
      "launch_spec: shared launch spec and CapabilitySliceFingerprint",
      "xcode_mcp_broker: pool, leases, backend lifecycle, id mapping, per-PID initialize mutex, capacity",
      "xcode_mcp_http: streamable HTTP facade, bearer middleware, session-id binding",
      "xcode_host_env: operator home/temp/developer-dir resolution",
      "xcode_host_dispatch: Unix socket dispatch and peer credential verification",
      "xcode_host_executor: route/reject policy, env allowlist, simulator destination rewrite",
      "xcode_shim: shim executables or generated thin binaries",
      "xcode_residual_observer: ACP session/update residual absolute-path warnings",
      "transport: canonical ACP MCP server discriminated union serialization",
      "xcode_target_resolver: host probing half of the resolver service and immutable XcodeTargetSnapshot production",
      "xcode_broker_health: XcodeBrokerHealthSnapshot subsystem state for lease acquisition and UI/API readback",
      "broker_mcp_policy: per-lease tools/list filtering and tools/call denial",
      "launch_resource_guard: fake-home/temp/config lifecycle for probes and sessions"
    ],
    "engine": [
      "mcp: provider capability gating, BrokeredXcodeMcpIntent, registry migration/fail-closed behavior",
      "executor: seven-phase flow, lease guard commit/rollback, active prompt attribution, observation sink implementation",
      "xcode_target_selection: frozen XcodeTargetSelectionInput passed to the acp resolver service",
      "session/fingerprint: broker contract hash and shim-signal reuse constraints",
      "recovery: invalidate session generations whose live broker leases are gone"
    ],
    "workflow_catalog": [
      "catalog_lint: hard-fail structured absolute paths and soft-warn prompt paths",
      "direct_command_declaration_scanner: normalized scanner for raw YAML and typed agent entries, including permission profile shell allow entries and agents[].required_tools",
      "catalog/domain/plan: xcode_broker_required, xcode_shim_injection_signal, requires_xcode_host_execution, xcode_prompt_lint_warnings",
      "examples/agents: migrate direct Xcode commands to explicit host-execution policy"
    ],
    "daemon": [
      "mount /xcode-mcp/{lease_id}",
      "construct shared broker state",
      "set router and dispatch-socket readiness",
      "expose Disabled/Healthy/Degraded/Failed XcodeBrokerHealthSnapshot without changing global daemon readiness for non-Xcode routes"
    ],
    "db_domain_api": [
      "migration adding actual_xcode_runtime_observation_json",
      "domain XcodeRuntimeObservation typed envelope",
      "repository append API",
      "GraphQL typed projection",
      "MCP reports.get projection",
      "bounded observation append semantics, corrupt-json recovery, truncation signaling, late-append refresh event"
    ],
    "swift_ui": [
      "RunTimelineInspectorView Xcode Runtime section",
      "FailedStageEvidencePanel friendly failure mapping",
      "Policy Warning treatment for residual paths",
      "AgentCatalogView infrastructure flags",
      "Broker health indicator",
      "Action Required Xcode modal state",
      "RunTimelineInspectorView bridge state propagation",
      "Policy Warning icon/color/coalescing",
      "View all residual paths disclosure after warning threshold",
      "AgentCatalogView first-run Xcode consent note",
      "ForgePanel/GroupBox styling for Xcode Runtime section",
      "stable accessibility identifiers for Xcode runtime fields"
    ],
    "gates_docs": [
      "scripts/test-gate.sh aliases p051-scaffold, proposal-051, p051",
      "docs/reference/test-gates.md",
      "static check for research artifact verdict and stale guidance patterns, including checked-in source-proposal drift from this controlling contract"
    ]
  }
}
