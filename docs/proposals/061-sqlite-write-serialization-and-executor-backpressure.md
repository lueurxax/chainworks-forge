{
  "proposal_revision_id": "p061-r3-generated-state-housekeeping",
  "source_review_pass_id": "b57f18ef-proposal-review-pass-1",
  "title": "Proposal 061: SQLite Write Serialization and Executor Backpressure",
  "status": "Revised for implementation planning",
  "run_id": "b57f18ef-58e4-4bcf-a22f-ae165d5db23b",
  "source_proposal": "docs/proposals/061-sqlite-write-serialization-and-executor-backpressure.md",
  "date": "2026-04-20",
  "primary_area": "Rust control-plane daemon, SQLite persistence, executor scheduling, ACP recovery, GraphQL/MCP readback, macOS operator UI diagnostics, generated-state housekeeping",
  "canonical_gate": "./scripts/test-gate.sh proposal-061",
  "review_readiness": {
    "target": "aggregate score above 9",
    "why_ready": [
      "The two review blockers are resolved with concrete implementation decisions: approve/retry/cancel p95 command latency must stay below 2 seconds under 20 active fake agents, and all prior open questions now have decisions or explicit deferrals.",
      "Backpressure is specified as visible scheduling state rather than failure, including durable summaries, stale markers, top-reason priority, and a push notification path for sustained queue age.",
      "The write coordination, provider normalization, scheduler projection, fairness, host-interruption, rollout, and gate contracts now name owners, durable state, test proof, and trade-offs.",
      "Residual product choices are not hidden: catalog cap overrides, DB-writer admission control, and manual pause/resume scheduling are deferred until the base capacity model has dogfood evidence."
    ],
    "reviewer_specific_checks": [
      {
        "reviewer": "product_owner",
        "must_confirm": "Latency threshold, rollout exits, success metrics, sustained-backpressure alerting, and the former open questions are concrete enough to start implementation."
      },
      {
        "reviewer": "ux_designer",
        "must_confirm": "Queued/backpressured states are discoverable without polling, the displayed top reason is deterministic, and host-interruption language is friendly."
      },
      {
        "reviewer": "ui_designer",
        "must_confirm": "Sidebar badges, Scheduler Health placement, host-interruption visual treatment, collapsible stage details, and stale-data indicators fit existing Forge UI density."
      },
      {
        "reviewer": "architect",
        "must_confirm": "The transaction, projection, fairness, provider-normalization, host-interruption, and failure-injection contracts are precise enough to implement without inventing cross-crate ownership."
      }
    ]
  },
  "problem": {
    "summary": "Chainworks Forge's local-first control plane currently saturates around concurrent proposal runs because executor fan-out, SQLite write contention, ACP provider startup, retry/skip flows, and recovery requeues compete without a single bounded scheduling model.",
    "evidence": [
      "Dogfooding on 2026-04-19 with 6 active proposal runs left 5 runs blocked or failed in non-terminal stage states.",
      "The same 24-hour period accumulated 74 ACP session idle timeout failures across Claude, Gemini, and Codex provider paths.",
      "The practical target is 5 stable active runs, 10 bounded active runs, and no more than 20 active agent executions under explicit backpressure."
    ],
    "user_impact": [
      "Operators see permanent-looking provider failures instead of a clear queue.",
      "Retry and skip can leave stale running execution records that confuse recovery and readback.",
      "Operator commands can stall behind SQLite write pressure.",
      "Laptop sleep, wake, and network migration are misclassified as ordinary provider failures."
    ],
    "positioning": "P061 is not a database migration. It is a local-capacity, single-writer, and recovery-truth proposal that makes overload bounded, visible, and retryable."
  },
  "current_baseline": {
    "already_present_or_partially_present": [
      "SQLite WAL mode, a small connection pool, and a documented 30-second busy timeout.",
      "begin_immediate_with_retry for SQLite write-lock acquisition, bounded SQLITE_BUSY retry, and write wait/write duration logging.",
      "InvokeAgent claim/start separated from the generic queue path with count-based global, provider, and per-run active execution checks.",
      "Read-only capacity precheck before attempting a writer lock when no pending InvokeAgent candidate is eligible.",
      "Claim/start capacity recheck under BEGIN IMMEDIATE.",
      "Idempotent post-completion AdvanceRun wake-up after InvokeAgent completion so fan-in can settle."
    ],
    "remaining_work": [
      "Durable queued/backpressured summaries and GraphQL/MCP parity readback.",
      "Provider alias normalization as one shared scheduler/runtime/API boundary.",
      "Fair candidate selection beyond FIFO window scanning.",
      "Hot-index proof for pending InvokeAgent scans and active-count joins.",
      "Consistent write coordination for multi-row command and recovery mutations.",
      "Retry, supersession, artifact-claim, and startup stale-running repair.",
      "Host sleep/wake and network-migration recovery with jittered retry under caps.",
      "Proposal-061 gate registration and failure-injection proof."
    ]
  },
  "goals": [
    "Keep 5 active proposal runs stable with 0 database-is-locked errors surfacing to GraphQL or MCP.",
    "Allow 10 active runs while surplus InvokeAgent work remains pending/backpressured rather than failed.",
    "Enforce default active-agent caps: global 20, per-run 4, Claude 8, Gemini 4, Codex 10, Auggie 1, Junie 1.",
    "Keep approve, retry, and cancel p95 command latency below 2 seconds under 20 active fake agents in the proposal-061 gate, with a 5-second hard timeout ceiling for any single command assertion.",
    "Make capacity pressure operator-visible by run, stage, provider, reason, count, oldest queued age, queue depth, and freshness.",
    "Notify operator surfaces when sustained backpressure crosses the configured queue-age threshold instead of requiring manual polling.",
    "Ensure retry, skip, cancellation, and supersession leave no stale running agent executions, work items, or artifact source-generation claims.",
    "Make startup recovery capacity-aware so daemon restart does not create a provider handshake storm.",
    "Classify host sleep/wake and network migration as retryable host interruptions, exempt those retries from provider quota budgets, and keep late-output settlement rules intact.",
    "Prove behavior with fake providers and in-process SQLite fixtures; do not require Xcode, live ACP providers, network, simulator, daemon deployment, benchmarks, or load tests for the proposal gate.",
    "Keep generated state from inactive run worktrees and stale ACP runtime homes from becoming an indirect SQLite/provider latency failure by pruning only rebuildable state through a daemon-owned housekeeping loop."
  ],
  "non_goals": [
    "Do not migrate from SQLite to Postgres or a distributed workflow platform.",
    "Do not increase SQLite connection count as the primary scale strategy.",
    "Do not make local provider parallelism unlimited.",
    "Do not implement P051 shared Xcode MCP bridge pooling in this proposal.",
    "Do not implement P060 lead-driven reviewer routing semantics in this proposal.",
    "Do not fix provider CLI bugs beyond control-plane scheduling, timeout, cleanup, and failure classification boundaries.",
    "Do not add operator include/exclude reviewer overrides.",
    "Do not add catalog-authored cap overrides in P061; catalog-specific cap lowering is a follow-up after the daemon-owned model is proven.",
    "Do not turn DB-writer pressure into a hard scheduler admission gate in P061; it starts as observable health with thresholds and hysteresis data for a follow-up decision.",
    "Do not add manual pause/resume scheduler controls in P061.",
    "Deleting worktrees, source files, run artifacts, active run build outputs, or SQLite database/backups through automatic housekeeping."
  ],
  "resolved_decisions": [
    {
      "question": "Where do provider caps live?",
      "decision": "For P061, caps live in daemon typed config and environment-compatible defaults only. Agent catalogs may reference providers, but they cannot lower or raise caps in this slice.",
      "rationale": "Daemon-owned caps keep scheduling, readback, and operator expectations consistent while the base model is still being proven. Catalog cap overrides affect workflow authoring and should be designed separately with UI and policy review."
    },
    {
      "question": "What is the default Codex provider cap?",
      "decision": "Codex defaults to 10 active executions. The earlier value of 3 was too low for the target 5-10 parallel proposal-run operating model and made Codex the bottleneck before global or per-run backpressure could be evaluated.",
      "rationale": "P061 still keeps overload bounded through global 20, per-run 4, provider-specific caps, and queue/readback visibility. Codex is the primary implementation provider in current dogfood, so its default must support the intended run mix while remaining below the global cap."
    },
    {
      "question": "What p95 command-latency threshold does the proposal-061 gate enforce?",
      "decision": "ApproveStage, RetryStage, and CancelRun p95 latency must be below 2 seconds while 20 fake agent executions are active. The gate also fails any single measured command assertion above 5 seconds unless the command is intentionally domain-rejected.",
      "rationale": "A 2-second p95 keeps operator actions visibly responsive while allowing normal local machine variance. The 5-second single-command ceiling prevents one outlier from being hidden by p95."
    },
    {
      "question": "Is db_writer_capacity a scheduler gate on day one?",
      "decision": "No. In P061, db_writer_capacity is read-only health and alert context. It becomes a scheduler gate only in a follow-up if dogfood shows sustained write wait pressure above threshold with acceptable false-positive behavior.",
      "rationale": "SQLite contention is the failure mode, but a hard DB-writer gate can create confusing stalls if driven by noisy samples. P061 first instruments write wait, retry exhaustion, and command latency."
    },
    {
      "question": "How are host-interruption retries batched?",
      "decision": "Retries are batched per provider family, constrained by global free slots, provider free slots, and per-run free slots. The default batch size is at most 2 retries per provider per jitter window, with a 5-30 second jitter range.",
      "rationale": "Provider-local batching prevents one provider family from recreating a handshake storm while still letting independent providers recover in parallel."
    },
    {
      "question": "Should UI add manual pause/resume scheduling per run?",
      "decision": "Deferred.",
      "rationale": "P061 must first prove durable scheduler truth, backpressure visibility, and bounded retries. Pause/resume is a user-control proposal once scheduler state and ownership are stable."
    }
  ],
  "ux_ui_notes": {
    "principle": "Backpressure must read as normal scheduling state, not failure.",
    "operator_visible_states": [
      "Active runs are distinct from active agent executions.",
      "Queued work is distinct from failed work.",
      "Provider capacity, run capacity, global capacity, DB writer pressure, and startup recovery backpressure are distinct concepts.",
      "Host interruption retry is neutral or cautionary, not a critical provider failure."
    ],
    "top_reason_priority": [
      {
        "rank": 1,
        "reason": "run_capacity",
        "display": "Run at agent limit",
        "why_first": "Most directly actionable for the current run."
      },
      {
        "rank": 2,
        "reason": "provider_capacity",
        "display": "Waiting for provider slot",
        "why_first": "Explains provider-specific saturation before global saturation."
      },
      {
        "rank": 3,
        "reason": "global_capacity",
        "display": "System agent limit reached",
        "why_first": "Explains system-wide saturation."
      },
      {
        "rank": 4,
        "reason": "startup_recovery_backpressure",
        "display": "Recovering queued work",
        "why_first": "Explains restart-specific staging."
      },
      {
        "rank": 5,
        "reason": "db_writer_capacity",
        "display": "Database writer busy",
        "why_first": "P061 exposes this only as health or alert context, not a scheduling admission reason."
      }
    ],
    "surfaces": [
      {
        "surface": "RunsHomeRow sidebar",
        "requirement": "Show queued agent count only when greater than zero as a small numeric badge next to the status capsule, or as a second-line secondary label if the badge would collide.",
        "density_constraint": "Must fit in 280-340pt sidebar width without line wrapping or overlap with attention icons."
      },
      {
        "surface": "Run detail",
        "requirement": "Show active agents, queued agents, oldest queued age, top reason, total global queue depth, and a non-ETA queue position hint when available."
      },
      {
        "surface": "Stage detail",
        "requirement": "Show pending/backpressured agents by provider and reason in a collapsed Backpressured Agents disclosure by default so evidence remains prominent."
      },
      {
        "surface": "Scheduler Health",
        "requirement": "Add a Scheduler Health section to PilotReadinessView and link to it from DaemonLifecycleBanner when sustained backpressure, stale projections, or DB writer pressure is detected."
      },
      {
        "surface": "Runtime health banner",
        "requirement": "Show System Busy or Queued when the oldest queued item exceeds the sustained-backpressure threshold. Default threshold is 5 minutes, configurable in daemon config."
      },
      {
        "surface": "Host-interruption timeline/status",
        "requirement": "Map host_interruption to friendly labels such as Recovering from system sleep or Resuming after network change. Use SF Symbol moon.zzz for sleep/wake and wifi.exclamationmark for network migration with neutral gray or caution orange tokens, not failed red."
      },
      {
        "surface": "Projection freshness",
        "requirement": "Show updated_at as relative time and a subtle stale warning if scheduler summaries are older than stale_after. Default stale_after is 60 seconds."
      }
    ],
    "copy_guidelines": [
      "Use Queued, Waiting for provider slot, Run at agent limit, System agent limit reached, Recovering queued work, Database writer busy, Recovering from system sleep, and Resuming after network change.",
      "Do not expose epoch IDs as primary user-facing labels. Epoch IDs remain available in diagnostics."
    ],
    "notifications": {
      "graphQL_subscription": "schedulerBackpressureChanged",
      "mcp_notification": "scheduler.backpressure.changed",
      "default_trigger": "oldest_queued_age >= 5 minutes for two consecutive snapshots",
      "default_clear": "oldest_queued_age < 2 minutes or queued_count == 0 for two consecutive snapshots",
      "payload": [
        "run_id",
        "stage_execution_id",
        "provider_family",
        "top_reason",
        "queued_count",
        "oldest_queued_age_ms",
        "global_queue_depth",
        "updated_at",
        "is_stale"
      ]
    }
  },
  "architecture": {
    "capacity_model": {
      "config_type": "InvokeAgentCapacityConfig",
      "defaults": {
        "global_active_agent_executions": 20,
        "per_run_active_agent_executions": 4,
        "provider_caps": {
          "claude": 8,
          "gemini": 4,
          "codex": 10,
          "auggie": 1,
          "junie": 1
        }
      },
      "behavior_when_full": {
        "global_capacity": "Pending InvokeAgent remains pending and appears in queue summaries.",
        "provider_capacity": "Provider-family work remains pending and appears in provider summaries.",
        "run_capacity": "Run-local surplus remains pending so one fan-out cannot consume all global slots.",
        "startup_recovery_backpressure": "Recovered work re-enters the ordinary scheduler and is staged under caps.",
        "db_writer_capacity": "Health-only in P061; not used to reject or delay claim/start admission."
      },
      "provider_normalization": {
        "owner": "control-plane/crates/domain or workflow shared provider-family module, reused by config load, InvokeAgent payload generation, scheduler checks, agent_executions persistence, GraphQL, and MCP.",
        "unknown_provider_behavior": "Catalog validation or config load fails loudly. Unknown aliases do not receive independent caps and do not silently bypass limits.",
        "canonical_aliases": [
          {
            "canonical": "claude",
            "aliases": [
              "claude",
              "claude_acp",
              "claude_agent",
              "claude_agent_acp"
            ]
          },
          {
            "canonical": "gemini",
            "aliases": [
              "gemini",
              "gemini_acp",
              "gemini_cli",
              "gemini_cli_acp"
            ]
          },
          {
            "canonical": "codex",
            "aliases": [
              "codex",
              "codex_acp",
              "codex_cli",
              "codex_cli_acp",
              "openai_codex"
            ]
          },
          {
            "canonical": "auggie",
            "aliases": [
              "auggie",
              "auggie_acp"
            ]
          },
          {
            "canonical": "junie",
            "aliases": [
              "junie",
              "junie_acp"
            ]
          }
        ],
        "gate_assertion": "Every provider string from examples/agents and proposal-061 fixtures resolves to one canonical family or fails with a typed UnknownProviderFamily error."
      }
    },
    "scheduler_fairness": {
      "policy": [
        "Read a bounded candidate window of pending InvokeAgent work ordered by scheduled_at and rowid.",
        "Annotate each candidate with canonical provider family, run_id, stage_execution_id, active counts, and all applicable capacity reasons.",
        "Select the oldest eligible item from the least-recently-served run within the window.",
        "Break ties deterministically by scheduled_at then rowid.",
        "When every candidate is blocked, refresh queue projections and avoid another writer-lock attempt until a wake, poll interval, or backoff expires."
      ],
      "state_model": {
        "table": "scheduler_service_state",
        "columns": [
          "scope TEXT CHECK(scope IN ('run','provider'))",
          "scope_id TEXT",
          "last_served_at TEXT",
          "last_claimed_work_item_id TEXT",
          "updated_at TEXT"
        ],
        "transaction_rule": "Update last_served_at in the same claim/start transaction that creates the active agent execution.",
        "restart_behavior": "Fairness resumes from durable scheduler_service_state. If rows are absent, derive ordering from active and completed execution timestamps before falling back to scheduled_at,rowid."
      },
      "tests": [
        "Blocked early candidates do not starve later eligible runs.",
        "Restart does not repeatedly favor the same run.",
        "Provider-cap saturation does not starve other provider families.",
        "Candidate windows smaller than total pending rows still make progress across many runs."
      ]
    },
    "work_item_state_and_readback": {
      "state_rule": "Capacity-blocked InvokeAgent work remains work_items.status = pending. P061 does not add a backpressured terminal or semi-terminal work item status.",
      "projection_tables": [
        {
          "name": "scheduler_queue_summaries",
          "purpose": "Durable aggregate readback for queued/backpressured work.",
          "columns": [
            "run_id",
            "stage_execution_id nullable",
            "provider_family nullable",
            "work_kind",
            "reason",
            "queued_count",
            "oldest_scheduled_at",
            "global_queue_depth",
            "position_hint nullable",
            "updated_at",
            "stale_after"
          ]
        },
        {
          "name": "scheduler_health_snapshots",
          "purpose": "Durable health readback for active counts, queued counts, writer pressure, command latency, and host interruption state.",
          "columns": [
            "id",
            "captured_at",
            "global_active_count",
            "provider_active_counts_json",
            "run_active_counts_json",
            "queued_counts_by_reason_json",
            "oldest_queued_at nullable",
            "db_writer_wait_p95_ms nullable",
            "command_latency_p95_ms_json",
            "last_host_interruption_epoch_id nullable",
            "updated_at",
            "stale_after"
          ]
        }
      ],
      "projection_ownership": {
        "owner": "engine scheduler/write-coordination path with db repository helpers shared by GraphQL and MCP read models.",
        "refresh_triggers": [
          "claim/start success",
          "InvokeAgent completion or failure",
          "RetryStage supersession",
          "CancelRun",
          "stage skip/supersession",
          "startup repair",
          "all-blocked scheduler scan",
          "host-interruption settlement and retry enqueue",
          "projection rebuild command used by recovery"
        ],
        "zero_count_rule": "Clear zero-count queue summary rows in the same write unit that observes the zero count.",
        "freshness_rule": "GraphQL, MCP, and UI expose updated_at, stale_after, and is_stale. Default stale_after is 60 seconds."
      }
    },
    "sqlite_write_serialization": {
      "principles": [
        "SQLite remains the source of truth.",
        "Multi-row invariants run in one transaction under BEGIN IMMEDIATE with bounded retry.",
        "Provider I/O, filesystem scans, subprocess waits, and ACP JSON-RPC waits never run inside a DB transaction.",
        "Projection updates that define immediate visible truth commit with the domain mutation.",
        "SQLITE_BUSY is retried with bounded backoff and logged before surfacing as an operator-visible error."
      ],
      "coordinator": {
        "owner_crate": "control-plane/crates/engine",
        "db_boundary": "control-plane/crates/db exposes transaction-scoped repository methods; engine owns command ordering and recovery semantics.",
        "not_an_actor_framework": true
      },
      "operation_contracts": [
        {
          "operation": "RetryStage",
          "owner_module": "engine command_handler and recovery helpers",
          "inside_transaction": [
            "command journal",
            "old stage supersession",
            "agent execution supersession",
            "work item supersession",
            "artifact source claim supersession",
            "new stage attempt",
            "new wake/enqueue",
            "queue projection refresh"
          ],
          "excluded_io": [
            "provider cancellation waits",
            "filesystem artifact scans"
          ],
          "idempotency_or_cas": "stage_attempt_id plus supersession generation",
          "failure_injection": "Crash after each supersession step and verify startup repair converges with no stale running executions.",
          "pr_order": 1
        },
        {
          "operation": "Startup repair",
          "owner_module": "engine recovery",
          "inside_transaction": [
            "stale running detection",
            "work requeue",
            "capacity-aware recovery markers",
            "queue projection refresh",
            "health snapshot"
          ],
          "excluded_io": [
            "provider process cleanup waits"
          ],
          "idempotency_or_cas": "repair epoch id",
          "failure_injection": "Repeated startup repair is idempotent and does not duplicate work.",
          "pr_order": 1
        },
        {
          "operation": "InvokeAgent claim/start/complete/fail",
          "owner_module": "engine executor",
          "inside_transaction": [
            "ownership claim",
            "capacity recheck",
            "agent_execution insert/update",
            "work item status update",
            "scheduler_service_state update",
            "queue projection refresh"
          ],
          "excluded_io": [
            "ACP session launch",
            "provider handshake",
            "model output wait"
          ],
          "idempotency_or_cas": "work_item ownership token and execution id uniqueness",
          "failure_injection": "Crash after claim and before provider launch cannot duplicate execution on recovery.",
          "pr_order": 2
        },
        {
          "operation": "StartRun/ApproveStage/RejectStage/CancelRun/ResetSession",
          "owner_module": "engine command_handler",
          "inside_transaction": [
            "command journal",
            "domain mutation",
            "affected work item updates",
            "projection refresh",
            "recovery recommendation refresh when applicable"
          ],
          "excluded_io": [
            "artifact import scans",
            "provider subprocess cleanup waits"
          ],
          "idempotency_or_cas": "command id and target version checks",
          "failure_injection": "Concurrent saturated executor load does not leak database-is-locked and command p95 remains below 2 seconds.",
          "pr_order": 3
        },
        {
          "operation": "Artifact import and active-index projection export",
          "owner_module": "engine artifact/recovery path",
          "inside_transaction": [
            "artifact record mutation",
            "source claim mutation",
            "active-index projection update when clients depend on immediate visibility"
          ],
          "excluded_io": [
            "filesystem discovery and content hashing"
          ],
          "idempotency_or_cas": "artifact source claim CAS",
          "failure_injection": "Late outputs from superseded attempts cannot promote over newer attempts.",
          "pr_order": 4
        }
      ]
    },
    "hot_indexes": {
      "migration": "Add or verify scheduler hot indexes in control-plane/crates/db/migrations.",
      "indexes": [
        "work_items(kind, status, scheduled_at)",
        "work_items(run_id, status, kind, scheduled_at)",
        "agent_executions(status, provider_family)",
        "agent_executions(status)",
        "stage_executions(run_id, id)",
        "artifact source claim index for retry supersession and late-output CAS paths if query plans show scans"
      ],
      "gate_proof": "proposal-061 asserts EXPLAIN/query-plan coverage for pending InvokeAgent scans, global/provider active counts, and per-run active-count joins with realistic fixtures of at least 1000 pending work items and 500 agent executions."
    },
    "retry_supersession_and_stale_repair": {
      "retry_stage_atomic_steps": [
        "validate command and journal it",
        "mark old stage attempt skipped or superseded",
        "close or supersede active agent executions for the old attempt",
        "supersede pending/running work items owned by the old attempt",
        "supersede active artifact source-generation claims for the old attempt",
        "create the new stage attempt",
        "enqueue the new AdvanceRun or InvokeAgent path",
        "rebuild queue summaries, health snapshots, and recovery recommendations read by clients"
      ],
      "startup_convergence_rule": "A terminal, skipped, or superseded stage must not own a running agent execution after startup repair."
    },
    "startup_recovery": {
      "rule": "Recovered InvokeAgent items enter the same scheduler gate as ordinary work and never bypass global, provider, or per-run caps.",
      "readback": [
        "recovered_item_count",
        "queued_under_startup_recovery_backpressure_count",
        "oldest_recovered_queued_age",
        "affected_run_count",
        "next_retry_or_backoff_time"
      ]
    },
    "host_interruption": {
      "detectors": [
        "runtime heartbeat comparing monotonic and wall-clock timestamps",
        "macOS sleep/wake hooks where available",
        "network path change hooks where available",
        "fallback threshold for large wall-clock gaps when hooks are unavailable"
      ],
      "domain_changes": [
        "Add AgentFailureKind.host_interruption.",
        "Add OperatorActionHint.recovering_from_system_sleep and OperatorActionHint.resuming_after_network_change.",
        "Persist host interruption facts separately from provider failure facts."
      ],
      "schema_contract": [
        {
          "table": "host_interruption_epochs",
          "columns": [
            "id",
            "detected_at",
            "source",
            "monotonic_gap_ms",
            "wall_clock_gap_ms",
            "network_path_changed",
            "sleep_wake_observed",
            "created_at"
          ]
        },
        {
          "table": "host_interruption_affected_executions",
          "columns": [
            "epoch_id",
            "agent_execution_id",
            "run_id",
            "stage_execution_id",
            "provider_family",
            "previous_status",
            "settlement_status",
            "cleanup_status",
            "retry_work_item_id nullable",
            "quota_budget_effect",
            "created_at"
          ]
        }
      ],
      "settlement_rules": [
        "Only executions running across the detected epoch are eligible for host_interruption classification.",
        "If an execution already produced valid promotable outputs under existing settlement rules, keep those rules and do not force retry.",
        "Terminate ACP session and provider process group before retry enqueue is considered complete.",
        "Close or supersede active artifact source-generation claims.",
        "Retries are exempt from provider quota retry budget but still count against active execution capacity.",
        "Late or partial outputs from superseded host-interrupted attempts are skipped unless existing settlement rules allow promotion."
      ],
      "retry_batching": "Default at most 2 retries per provider family per 5-30 second jitter window, further constrained by global, provider, and per-run free slots."
    },
    "api_contracts": {
      "graphql_readback": [
        "schedulerHealthSummary",
        "activeExecutionCountsByProvider",
        "queuedBackpressuredCountsByProviderAndReason",
        "runQueueSummary",
        "stageQueueSummary",
        "oldestQueuedAge",
        "queuePositionHint",
        "schedulerBackpressureChanged subscription",
        "hostInterruptionEpochs",
        "hostInterruptionAffectedExecutions",
        "commandLatencySummary",
        "dbWriterContentionSummary",
        "projection updated_at/stale_after/is_stale"
      ],
      "mcp_readback": [
        "reports.get includes scheduler health, queue summaries, host interruption epochs, contention summaries, freshness, and sustained-backpressure events.",
        "MCP command tools return completed or domain-rejected command results. Raw SQLite contention reaches MCP only after bounded retries are exhausted and is classified as operator-visible DB contention."
      ],
      "shared_read_model_rule": "GraphQL and MCP read from shared db/domain read-model functions so reason names, counts, oldest age, stale markers, and host-interruption facts cannot drift."
    },
    "generated_state_housekeeping": {
      "owner": "engine background executor loop",
      "default_enabled": true,
      "configuration": [
        "CHAINWORKS_GENERATED_STATE_HOUSEKEEPING disables the loop when set to 0/false/off/no.",
        "CHAINWORKS_GENERATED_STATE_HOUSEKEEPING_INTERVAL_SECS controls cadence.",
        "CHAINWORKS_GENERATED_STATE_HOUSEKEEPING_MIN_AGE_SECS controls the minimum age before deletion."
      ],
      "allowed_deletions": [
        "control-plane/target and top-level target directories inside managed .chainworks/worktrees/<run> directories when the owning run is completed, failed, or cancelled.",
        "stale .forge-codex-acp runtime homes under known workspace roots when older than the configured age and not referenced by a live process command line.",
        "stale .git/objects/tmp_obj_* files under known workspace roots."
      ],
      "forbidden_deletions": [
        "worktree directories themselves",
        "source files",
        ".git worktree metadata except tmp_obj_* garbage files",
        ".chainworks/runs artifacts",
        "current SQLite DB, WAL/SHM, or DB backups",
        "generated build outputs for running, blocked, pending, or cancelling runs"
      ],
      "failure_policy": "best-effort warnings only; housekeeping failures must not stop executor scheduling",
      "observability": "structured logs include removed directory/file counts and reclaimed bytes"
    }
  },
  "rollout_plan": [
    {
      "phase": 1,
      "name": "Baseline audit and first gate registration",
      "work": "Confirm existing busy timeout, begin_immediate_with_retry, count-based capacity gate, and post-completion wake-up. Register proposal-061 in scripts/test-gate.sh and docs/reference/test-gates.md.",
      "exit_criteria": "Unit/integration coverage exists for the baseline claims and proposal-061 can be listed by the gate wrapper."
    },
    {
      "phase": 2,
      "name": "Provider normalization and hot indexes",
      "work": "Add shared ProviderFamily normalization and hot-index migrations with query-plan tests.",
      "exit_criteria": "All known catalog provider aliases resolve or fail loudly; EXPLAIN/query-plan assertions use the intended indexes at fixture scale."
    },
    {
      "phase": 3,
      "name": "Scheduler projections, freshness, and readback",
      "work": "Add scheduler_queue_summaries, scheduler_health_snapshots, projection refresh triggers, GraphQL/MCP parity, stale markers, and sustained-backpressure notification.",
      "exit_criteria": "GraphQL and MCP return the same fixture summaries, zero-count rows clear, stale_after is exposed, and subscription/notification fires and clears at thresholds."
    },
    {
      "phase": 4,
      "name": "Fair selection",
      "work": "Add bounded candidate annotation, least-recently-served durable state, and deterministic tie-breaking.",
      "exit_criteria": "Gate fixtures prove blocked early candidates, provider-cap saturation, restart, and many-run windows do not starve eligible runs."
    },
    {
      "phase": 5,
      "name": "Write coordination and stale repair",
      "work": "Convert RetryStage and startup repair first, then claim/start/complete/fail and remaining operator commands through transaction-scoped repository methods.",
      "exit_criteria": "Failure-injection tests show 0 duplicate executions, 0 stale running executions after retry/skip/cancel/startup repair, and approve/retry/cancel p95 below 2 seconds under 20 active fake agents."
    },
    {
      "phase": 6,
      "name": "Host interruption",
      "work": "Add epoch schema, detector lifecycle, ACP cleanup owner, retry-budget exemption, jittered retry under caps, UI/API mapping, and late-output settlement proof.",
      "exit_criteria": "Separate gate checks pass for detection/classification, process cleanup, jittered retry under caps, quota-budget exemption, and late-output handling."
    },
    {
      "phase": 7,
      "name": "Dogfood 5 active runs",
      "work": "Run local dogfood with 5 active proposal runs using conservative defaults.",
      "exit_criteria": "For at least 4 active dogfood hours: 0 database-is-locked escapes, approve/retry/cancel p95 below 2 seconds, 0 stale running executions, and at least 90 percent of runs reach terminal state without manual retry."
    },
    {
      "phase": 8,
      "name": "Dogfood 10 bounded runs",
      "work": "Run 10 active proposal runs with the same caps.",
      "exit_criteria": "For at least 2 active dogfood hours: active executions remain at or below caps, surplus InvokeAgent work stays pending/backpressured rather than failed, sustained-backpressure notifications are accurate, and no provider cap is raised."
    },
    {
      "phase": 9,
      "name": "Cap review",
      "work": "Consider cap changes only after two successful dogfood sessions and proposal-061 gate stability.",
      "exit_criteria": "Any cap change has runtime health evidence, command-latency evidence, and provider-specific failure evidence. Gemini caps must be revisited if P051 changes Xcode/Gemini subprocess lifecycle; per-run caps and fairness window must be revisited if P060 changes reviewer fan-out cardinality."
    }
  ],
  "metrics": {
    "technical_runtime_facts": [
      "DB write lock wait time by operation",
      "DB write transaction duration by operation",
      "SQLITE_BUSY retry count and exhausted retry count",
      "approve/retry/cancel command latency p50/p95/p99",
      "pending/backpressured work by kind, provider, run, stage, and reason",
      "active agent executions by provider and run",
      "scheduler candidate scan count and all-blocked count",
      "scheduler projection updated_at, stale_after, and stale read count",
      "stale execution repair count",
      "retry supersession count",
      "host interruption epoch count and affected execution count",
      "provider session start latency and startup failure count",
      "ACP process cleanup success/failure count",
      "sustained-backpressure notification fire and clear count"
    ],
    "product_success_metrics": [
      {
        "metric": "Terminal completion without manual retry under 5-run dogfood",
        "target": "At least 90 percent of proposal runs reach terminal state without operator retry during the 5-run phase."
      },
      {
        "metric": "Oldest queued age during steady-state dogfood",
        "target": "Median oldest queued age remains below 10 minutes under 5-run dogfood, excluding intentional startup recovery batches."
      },
      {
        "metric": "Operator retry commands per run",
        "target": "Reduce operator retry commands per run by at least 50 percent compared with the 2026-04-19 dogfood baseline once comparable runs are available."
      },
      {
        "metric": "Silent stall prevention",
        "target": "100 percent of sustained-backpressure intervals above the threshold create a GraphQL subscription event or MCP notification with matching UI health state."
      }
    ]
  },
  "acceptance_criteria": [
    "AC-1: 5 active runs produce 0 database-is-locked errors surfaced to GraphQL or MCP.",
    "AC-2: 10 active runs keep surplus InvokeAgent work pending/backpressured, not failed.",
    "AC-3: Default provider-cap test keeps Gemini active executions at or below 4 and Codex active executions at or below 10.",
    "AC-4: Per-run-cap test prevents one fan-out from consuming all global slots.",
    "AC-5: ApproveStage, RetryStage, and CancelRun p95 command latency stays below 2 seconds under 20 active fake agents, with no single assertion above 5 seconds unless domain-rejected.",
    "AC-6: Retry leaves no stale running agent_executions for superseded attempts.",
    "AC-7: Startup recovery respects caps and exposes queued/backpressure readback.",
    "AC-8: Claim/start crash recovery cannot duplicate an agent execution.",
    "AC-9: GraphQL and MCP expose parity scheduler summaries with freshness markers.",
    "AC-10a: Simulated host sleep/wake or network migration creates a host_interruption epoch and classifies only affected running executions.",
    "AC-10b: Host-interrupted executions close ACP sessions, terminate provider process groups, and close or supersede active artifact source claims.",
    "AC-10c: Host-interrupted executions requeue with jitter under global, provider, and per-run caps.",
    "AC-11: Host-interruption retry does not consume provider quota retry budget and does not promote late/partial outputs unless existing settlement rules allow it.",
    "AC-12: DB contention instrumentation is visible in runtime health logs or projections.",
    "AC-13: Provider alias normalization fails unknown provider strings loudly and prevents alias cap bypass.",
    "AC-14: Sustained-backpressure subscription/MCP notification fires and clears at configured thresholds.",
    "AC-15: Query-plan assertions prove hot indexes for pending InvokeAgent scans and active-count joins at fixture scale.",
    "AC-16: Generated-state housekeeping removes only inactive terminal-run target directories, stale unreferenced ACP runtime homes, and stale git tmp objects; it never removes active/blocked run outputs, worktrees, source files, run artifacts, or database files."
  ],
  "test_plan": {
    "gate": "./scripts/test-gate.sh proposal-061",
    "constraints": [
      "Use fake providers and in-process SQLite fixtures.",
      "No Xcode, live ACP providers, network, simulator, daemon deployment, benchmark, or load-test requirement."
    ],
    "coverage": [
      "Capacity config defaults, including Codex default cap 10, and provider alias normalization.",
      "Capacity accounting for global, provider, and per-run caps.",
      "Capacity-aware claim/start leaves blocked work pending and does not create agent_executions.",
      "Fair scheduler selection prevents one run or provider family from starving others.",
      "Hot-index-backed pending InvokeAgent scans and active-count queries with EXPLAIN/query-plan assertions.",
      "ApproveStage, RetryStage, and CancelRun p95 command latency below 2 seconds under 20 active fake agents.",
      "Retry supersedes old attempts, work items, agent executions, and artifact source claims.",
      "Startup repair requeues stale work through capacity gates and exposes backpressure summaries.",
      "Claim/start crash recovery cannot duplicate an agent execution.",
      "Projection freshness, zero-count cleanup, all-blocked scan updates, and stale readback markers.",
      "GraphQL and MCP expose parity scheduler summaries through shared read-model functions.",
      "Sustained-backpressure subscription/MCP notification threshold fire and clear behavior.",
      "Simulated host sleep/wake and network migration classification, process cleanup, jittered retry under caps, quota exemption, and late-output settlement.",
      "DB contention instrumentation appears in runtime health logs or projections.",
      "Housekeeping tests prove active/blocked run targets are preserved while terminal-run generated targets and unreferenced stale ACP homes are pruned."
    ]
  },
  "risks_and_tradeoffs": [
    {
      "risk": "Lower immediate fan-out",
      "tradeoff": "Intentional. Queued work is preferable to failed provider handshakes, stale execution records, and SQLite contention. The mitigation is clear queue state and sustained-backpressure notification."
    },
    {
      "risk": "Scheduler starvation",
      "tradeoff": "FIFO alone is insufficient. P061 adds durable least-recently-served state and restart tests without creating a broad distributed scheduler."
    },
    {
      "risk": "Stale queue projections",
      "tradeoff": "Derived summaries can drift. P061 defines refresh triggers, zero-count cleanup, updated_at, stale_after, and shared read models."
    },
    {
      "risk": "Over-centralized write coordinator",
      "tradeoff": "The coordinator owns transaction boundaries and retry instrumentation only. Domain semantics remain in command/orchestrator code."
    },
    {
      "risk": "db_writer_capacity could be too aggressive as a gate",
      "tradeoff": "P061 keeps it diagnostic. Admission control can be proposed later with dogfood thresholds and hysteresis."
    },
    {
      "risk": "Host interruption misclassification",
      "tradeoff": "Only executions spanning a detected epoch are classified. Valid completed output settlement is preserved."
    },
    {
      "risk": "Provider caps become stale",
      "tradeoff": "Defaults remain conservative and daemon-owned. Cap increases require gate and dogfood evidence."
    },
    {
      "risk": "P051 or P060 changes pressure patterns",
      "tradeoff": "If P051 changes Gemini/Xcode subprocess lifecycle, revisit Gemini cap and host-interruption classification. If P060 changes reviewer fan-out cardinality, revisit per-run caps and fairness window."
    },
    {
      "risk": "Sustained-backpressure alerts become noisy",
      "tradeoff": "Use threshold and clear hysteresis: fire after two snapshots above 5 minutes, clear after two snapshots below 2 minutes or zero queued work."
    },
    {
      "risk": "Over-aggressive cleanup could destroy operator evidence or active build state.",
      "mitigation": "Keep the deletion allowlist narrow, require terminal run status for worktree targets, require age threshold and live-process guard for ACP homes, and never delete worktrees, run artifacts, source files, or database files."
    }
  ],
  "open_questions": [
    {
      "question": "Should catalog-authored provider caps be allowed later?",
      "status": "deferred",
      "reason": "Daemon-owned caps are required for P061 consistency. Catalog lowering/raising affects authoring and policy UX and needs a separate proposal."
    },
    {
      "question": "Should db_writer_capacity become a hard scheduler gate later?",
      "status": "deferred",
      "reason": "P061 records DB writer pressure as health only. A future gate requires measured contention thresholds, hysteresis, and false-positive review."
    },
    {
      "question": "Should operator UI add manual pause/resume scheduling per run?",
      "status": "deferred",
      "reason": "Manual control is useful only after durable scheduler state, readback, and recovery behavior have landed."
    }
  ],
  "reviewer_feedback_resolution": [
    {
      "issues": [
        "PO-001",
        "ARCH-P061-006",
        "LIFT-001"
      ],
      "resolution": "Defined the proposal-061 command-latency gate: approve/retry/cancel p95 below 2 seconds under 20 active fake agents, with a 5-second single-command ceiling."
    },
    {
      "issues": [
        "PO-002",
        "LIFT-002"
      ],
      "resolution": "Resolved former open questions 1-4 and deferred only manual pause/resume plus follow-up catalog cap overrides and DB-writer admission control."
    },
    {
      "issues": [
        "PO-003",
        "LIFT-003"
      ],
      "resolution": "Added measurable rollout exit criteria for every phase, including 4-hour 5-run dogfood and 2-hour 10-run bounded dogfood thresholds."
    },
    {
      "issues": [
        "PO-004",
        "UX-P061-001",
        "LIFT-004"
      ],
      "resolution": "Added GraphQL subscription and MCP notification for sustained backpressure with trigger and clear hysteresis."
    },
    {
      "issues": [
        "UX-P061-002",
        "LIFT-005"
      ],
      "resolution": "Defined deterministic top-reason priority and friendly display strings."
    },
    {
      "issues": [
        "ARCH-P061-001",
        "LIFT-006"
      ],
      "resolution": "Added a write-coordinator operation table with owner modules, transaction contents, excluded I/O, idempotency/CAS, failure injection, and PR order."
    },
    {
      "issues": [
        "ARCH-P061-002",
        "UX-P061-004",
        "UI-003",
        "LIFT-007"
      ],
      "resolution": "Added host-interruption schema, domain variants, retry-budget rules, cleanup ownership, retry batching, API readback, and UI label/icon/color mapping."
    },
    {
      "issues": [
        "ARCH-P061-005",
        "PO-007",
        "LIFT-008"
      ],
      "resolution": "Added canonical provider-family aliases and unknown-provider failure behavior."
    },
    {
      "issues": [
        "PO-005",
        "LIFT-009"
      ],
      "resolution": "Split host-interruption acceptance into AC-10a detection/classification, AC-10b cleanup, and AC-10c jittered retry under caps."
    },
    {
      "issues": [
        "PO-006",
        "LIFT-010"
      ],
      "resolution": "Added product success metrics for terminal completion without manual retry, oldest queued age, operator retry frequency, and silent-stall prevention."
    },
    {
      "issues": [
        "UI-001",
        "UI-002",
        "LIFT-011"
      ],
      "resolution": "Specified sidebar queued badge layout and Scheduler Health placement in PilotReadinessView with DaemonLifecycleBanner linking."
    },
    {
      "issues": [
        "ARCH-P061-003",
        "UI-005",
        "LIFT-012"
      ],
      "resolution": "Defined projection ownership, refresh triggers, zero-count cleanup, updated_at/stale_after, and stale UI/API indicators."
    },
    {
      "issues": [
        "UX-P061-003"
      ],
      "resolution": "Added total global queue depth and queue position hint readback without promising an exact ETA."
    },
    {
      "issues": [
        "UI-004"
      ],
      "resolution": "Specified a collapsed Backpressured Agents disclosure in StageDetailView."
    },
    {
      "issues": [
        "ARCH-P061-004"
      ],
      "resolution": "Added durable scheduler_service_state for least-recently-served fairness and restart behavior."
    },
    {
      "issues": [
        "PO-008"
      ],
      "resolution": "Added P051/P060 dependency risk notes identifying which P061 defaults must be revisited if those proposals change pressure patterns."
    }
  ]
}
