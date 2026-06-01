# Escalation Policies

This document owns the current escalation-policy contract for Chainworks Forge. It covers `escalation_policy_v1`, durable tier advancement, readback, macOS presentation boundaries, and operational controls for auditable ordered escalation chains.

## Current Contract

The current system implements configurable escalation policies and scheduler-owned tier execution:

- Domain enums (`EscalationTierKind`, `EscalationTrigger`, `EscalationPauseReason`) and the `escalation_policy_v1` YAML schema with strict compile validation (`escalation_policy_unknown_backend_profile`, `escalation_policy_ambiguous_at_compile`, `escalation_policy_unsafe_for_side_effect_stage`).
- `policy_hash` and binding data frozen into `RunPlan` at compile time.
- Durable SQLite tables created by migrations `076_p058_escalation_schema.sql` (`escalation_ledger`, `escalation_execution_metadata`, `escalation_events`) and `077_p058_escalation_redaction_version.sql` (`redaction_version` enforced at insert time), with repository-layer JSON validation that holds even without sqlite json1.
- Idempotency enforced at the persistence layer by migration `078_p058_escalation_idempotency.sql`: a unique index on `escalation_ledger(run_id, stage_id, agent_id, policy_id)` prevents duplicate chain creation, and a unique index on `escalation_execution_metadata(escalation_ledger_id, tier_id, tier_attempt_index)` enforces the escalation idempotency key (`run_id`, `stage_id`, `ledger_id`, `tier_id`, `tier_attempt_index`) at the tier-attempt grain.
- Runtime resolution at agent-execution start (`control-plane/crates/engine/src/executor.rs`) reads the frozen `RunPlan` snapshot only — no live YAML fallback — and commits the `escalation_ledger` (insert-or-ignore), `agent_executions` row with escalation columns populated, and the first `escalation_execution_metadata` row inside a single SQLite transaction (per *Scheduler Transaction* below). A snapshot, repo, or policy-resolution error fails the agent execution closed rather than starting with null escalation fields.
- Policy matching prefers the InvokeAgent payload's `backend_profile_id` (the actual invoked agent's profile) over the stage owner binding, so non-owner tasks, retries, dynamic reviewers, and mediation invokes attribute to a `backend_profile`-bound policy correctly.
- GraphQL `runEscalationReadback` query and MCP `runs.get` escalation-readback projection share field shape (see *Readback shape and authorization* below).
- Pause-reason vocabulary (13 catalog entries) with `operator_action_hint` and `runbook_anchor` surfaced through readback; runbook stubs live under [`docs/runbooks/escalation/`](../runbooks/escalation/).

Tier selection is wired end-to-end (`control-plane/crates/engine/src/shadow_escalation.rs`): on agent-execution completion, the executor classifies the trigger from `AgentFailureKind`, looks up the frozen policy in `RunPlan`, writes `would_select_tier_id`, `would_select_trigger_raw`, and `would_select_decision_json` into `agent_execution_runtime_facts`, and advances the durable `escalation_ledger` plus `escalation_events` journal when the completed execution still owns the active tier. The follow-up scheduler path (`control-plane/crates/engine/src/orchestrator.rs`) consumes the durable current tier and can enqueue `same_backend_retry`, `backend_profile`, or `lead_mediation` attempts; `pause`/exhausted tiers block the run and suppress legacy auto-retry. Writes are best-effort and never block primary execution: failures are logged and swallowed. The classifier covers `provider_quota_exhausted`, `transport_failure`, `contract_output_failure`, and `stale_no_output`; non-escalatable failure kinds (operator cancellation, supersession, permission stalls) skip selection entirely.

### Readback shape and authorization

GraphQL `runEscalationReadback` and MCP `runs.get` produce a parity-matched escalation readback with the following Phase 1 contract:

- **`has_active_escalation`** is true only when a chain has a failure trigger fired (`trigger_raw IS NOT NULL`) or has advanced beyond its initial `active` status. A chain inserted at claim time with a policy configured but no trigger does not flip this flag.
- **Row caps** prevent unbounded payloads: at most 50 ledgers per run (`chains_truncated`/`chains_total`), 200 events per ledger (`events_truncated`/`events_total`), and 100 execution metadata rows per ledger (`execution_metas_truncated`/`execution_metas_total`). Aggregate counts come from separate `COUNT(*)` queries so `chains_total` and `paused_chain_count` remain accurate even past the cap; clients must paginate via dedicated chain/event queries to recover full history.
- **Scheduler readback fields** (`waiting_retry_after_until`, `trace_unavailable_reason_raw`, `escalation_trace_json_redacted`, `policy_drift_state`, `external_acknowledgement_ref`, `feature_flag_state`, and per-attempt `digest_inputs`) are derived from durable, redacted `escalation_events` and ledger state. Missing redacted events remain `null`; truncated event pages report `trace_unavailable_reason_raw = event_cap_exceeded`.
- **Authorization** (MCP only): Operator principals receive the full chain payload; Agent and Observer principals receive a summary projection with `chains_redacted: true` exposing only `paused_chain_count` and `has_active_escalation`. The summary intentionally omits `dominant_pause_reason_raw` to avoid leaking operator-hint intent. The same `runs.get` invocation also redacts operator-only snapshot fields (`workflow_snapshot_json`, `catalog_snapshot_json`, `delivery_*_json`, `drift_details_json`, local filesystem paths) from non-Operator callers.

The runtime enforces kill-switch pauses, chain wall-clock deadline pauses, launch-recycle storm pauses, provider force-detach pauses, late-frame event journaling, startup force-detach replay, and the default three-failure capacity-probe pause threshold before launching an escalation retry. The governed macOS read surface is adapter-owned and covered by the retained escalation proof gate documented in [test-gates](test-gates.md) for status capsule field order/truncation/accessibility, pause-card countdown/metadata and ultra-narrow fallback, command disabled-reason parity, actual `DriftReviewSheet` structured tier/trigger/max-attempt inputs, compact banner co-occurrence summarization, non-collapsed lineage disclosures, SF Symbol availability, drift diff presentation, lineage retry collapse/duration/ref disclosure, MenuBarExtra overflow routing, all-run attention aggregation, retained inspector adapters, and atomic trace pasteboard copy. Remote visual soak, long-run threshold trending, and live operator-restart drills are tracked by P096 as release evidence.

## Architecture

The Rust control plane serves as the authoritative source for policy resolution, trigger classification, blocker digest calculation, capacity probing, tier advancement, retry budgets, pause/resume legality, kill-switch evaluation, shadow decisions, persistence, and recovery. Governed macOS renders frozen GraphQL DTOs and never reconstructs escalation truth from local workflow files, logs, SwiftData, compiler output, context-strategy counters, or local caches.

### Binding Precedence

Workflow-stage policy binding wins over agent binding only when explicit. Agent and backend_profile bindings with equal specificity are ambiguous unless a policy declares deterministic precedence. Ambiguity pauses at compile/preflight rather than selecting silently.

### Blocker Digest

`digest_version escalation_blocker_digest_v1` uses typed inputs `failure_kind`, `output_settlement_state`, `validation_evidence_kind`, and `redacted_message_fragment_hash`. Readback exposes `digest_inputs` and `redacted_evidence_ref`, not raw evidence.

### Idempotency Key

The idempotency key is composed of `run_id`, `stage_id`, `ledger_id`, `tier_id`, `tier_attempt_index`. `launch_recycle_index` is excluded so recycle replay targets the same committed attempt.

### Operational Controls

Global `CHAINWORKS_ESCALATION_FORCE_PRIMARY` forces `tier=primary` at scheduling time. Per-policy enabled state comes from frozen policy plus `runtime_policy_overrides`. `in_flight_toggle_behavior` supports `continue`, `pin`, and `pause`; default is `continue`. The selected behavior is stamped on the ledger and surfaced through `featureFlagState`.

### Overlap Free Tier Invariant

`escalation.tier_advanced` is emitted only after the previous tier has a settled terminal outcome. No ledger may have more than one active tier, and force-detach windows cannot double-charge provider quota.

### Persistence

The system persists escalation data across several tables:

-   `escalation_ledger`
-   `escalation_execution_metadata`
-   `escalation_events`

**Agent Execution Columns:**
`escalation_policy_id`, `escalation_policy_hash`, `escalation_tier_id`, `escalation_tier_kind_raw`, `escalation_trigger_raw`, `escalation_digest_version`, `escalation_ledger_id`

**Foreign Key Targets:**
`runs(id)`, `agent_executions(id)`

**Shadow Columns:**
`agent_execution_runtime_facts.would_select_tier_id`, `agent_execution_runtime_facts.would_select_trigger_raw`, `agent_execution_runtime_facts.would_select_decision_json`

**JSON Validation:**
Repository layer rejects malformed JSON even without sqlite json1. `escalation_events.payload_json` is additionally enforced against an allowlist before commit:

- Top level must be a JSON object. Permitted keys: `digest_inputs`, `redacted_evidence_ref`, `tier_id`, `tier_kind_raw`, `trigger_raw`, `pause_reason_raw`, `event_kind_raw`, `policy_id`, `chain_attempt_index`, `digest_version`. Unknown keys (including alternate spellings) are rejected.
- `digest_inputs` sub-object accepts only `failure_kind`, `output_settlement_state`, `validation_evidence_kind`, and `redacted_message_fragment_hash`.
- `redacted_evidence_ref` and `redacted_message_fragment_hash` must start with an approved hash/ref prefix (`sha256:`, `sha3-256:`, `sha3-384:`, `sha3-512:`, `hmac-sha256:`, `blake2:`, `blake3:`, or `ref/` for evidence refs). Hash prefixes require a pure-hex suffix; `ref/` requires a relative path with no `://`, leading `/`, or `..`. URL schemes, absolute paths, and bare credentials are rejected.
- Per-value byte caps: 512 bytes for general string values, 256 bytes for `redacted_evidence_ref`, 128 bytes for `redacted_message_fragment_hash`. Identifier fields reject whitespace, control characters, and credential-shaped tokens.
- Duplicate JSON keys at any depth are rejected before parsing; the canonical re-serialized form is what gets stored, so writers cannot smuggle a second value past redaction by repeating a key.

Ledger and execution-metadata writers apply the same identifier and credential-pattern checks plus per-field byte caps (`policy_id`, `tier_id`, `trigger_raw`, `pause_reason_raw`, etc. capped at 256 bytes; `operator_action_hint` and `runbook_anchor` capped at 1 KiB and credential-shape-checked). The shadow column writer in `agent_execution_runtime_facts` (`would_select_tier_id`, `would_select_trigger_raw`, `would_select_decision_json`) is held to the same JSON validation and credential-pattern checks, so absolute filesystem paths (including `/tmp/...`, `/private/tmp/...`, `/opt/...`), URL schemes, and bare credentials are rejected before commit. Bumps to redaction rules require a new `redaction_version`.

**Dominant Pause Reason:**
When multiple chains for a run are paused or exhausted, the server-side `dominant_pause_reason_for_run` query selects the highest-severity pause reason using the banner precedence ordering (`escalation_kill_switch_engaged` > `escalation_policy_drift` > `escalation_policy_disabled` > `escalation_recovery_inconsistent` > `capacity_probe_failed` > `shadow_mode`, then earliest unknown by `created_at`). This is the value surfaced through Operator-principal readback as `dominant_pause_reason_raw`; Agent/Observer principals do not receive it.

### Policy Drift

Resume compares frozen `policy_hash` and binding data with current repo catalog. Drift opens `escalation_policy_drift` pause. In v1, acknowledgement is external through MCP/operator workflow, not a governed macOS write. After acknowledgement, Rust control plane records the durable state transition and GraphQL/MCP readback refreshes clients.

### Recovery

Recovery-inconsistent triggers pause fail-closed with `escalation_recovery_inconsistent`. V1 unstick is cancellation with a recovery-cancelled marker preserving ledger order, originating trigger code, report successor immutability, partial-progress signal, and unstick latency metrics. The recovery-cancelled marker is committed inside the same scheduler transaction as the cancellation event, so replay observes either the full cancellation or none of it. Cancellation invokes provider force-detach with the same 120s ceiling. Provider sessions that have not settled by `shutdown_drain_seconds` are treated as crash-interrupted; on next start, `force_detach_replay` reissues the in-flight force-detach with the same idempotency key, and late frames are dropped and journaled as `escalation.provider_late_frame_after_detach`.

### Scheduler Transaction

Settlement, trigger selection, digest calculation, frozen policy lookup, ledger lookup, readiness validation, capacity validation, ledger/event/metadata updates, and work-queue insert commit in one SQLite transaction. Provider launch occurs only after commit. Housekeeping uses the same compare-and-swap path and is idempotent.

## Policy Schema

The escalation policy schema is strictly defined to ensure compile-time validation and predictable runtime behavior.

**Required Fields:**
`policy_id`, `schema_version`, `enabled_default`, `applies_to`, `max_chain_attempts`, `max_chain_wall_clock_seconds`, `triggers`, `tiers`

**Strictness:**
-   Unknown escalation fields fail compile.
-   Unknown backend profiles fail compile with `escalation_policy_unknown_backend_profile`.
-   Ambiguous bindings fail compile with `escalation_policy_ambiguous_at_compile`.
-   Unsafe side-effect stage bindings fail compile with `escalation_policy_unsafe_for_side_effect_stage`. The check is fail-closed across all three `applies_to` axes: a policy is rejected if it binds (a) directly to a stage flagged unsafe by `is_unsafe_for_escalation()` (manual gates and non-compute stage types — `release`, `side_effect`, `publish`, etc.), (b) to an agent that owns or runs tasks in such a stage, or (c) to a `backend_profile_id` used by any agent in such a stage. Unknown `applies_to.stage_id` selectors also fail compile.
-   Missing tier permission or runtime validation pauses fail-closed before scheduling.
-   Policy structural validation (identifier safety, control-character rejection) runs before catalog-level validators so malformed identifiers cannot be interpolated into compile-failure diagnostics.

**Tier Kinds:**
`same_backend_retry`, `backend_profile`, `lead_mediation`, `pause`

### YAML Policy Example

```yaml
applies_to:
  agent_id: code_writer
enabled_default: false
max_chain_attempts: 6
max_chain_wall_clock_seconds: 7200
policy_id: code_writer_default_escalation
schema_version: escalation_policy_v1
tiers:
  - kind: same_backend_retry
    max_attempts: 2
    tier_id: primary_retry
  - backend_profile_id: claude_builder_frontier
    kind: backend_profile
    max_attempts: 1
    tier_id: frontier_profile
  - backend_profile_id: codex_implementer_high
    kind: backend_profile
    max_attempts: 1
    tier_id: codex_profile
  - kind: lead_mediation
    max_attempts: 1
    tier_id: lead_review
  - kind: pause
    tier_id: human_pause
triggers:
  - repeated_same_blocker_digest
  - contract_output_failure
  - stale_no_output
  - provider_quota_exhausted
  - transport_failure
  - loop_budget_threshold
```

## Pause Reason Catalog

The system defines a catalog of pause reasons with corresponding operator action hints and runbook anchors.

| Code                                     | Operator Action Hint                                                                         | Phase     | Runbook Anchor                                   |
| :--------------------------------------- | :------------------------------------------------------------------------------------------- | :-------- | :----------------------------------------------- |
| `escalation_policy_unknown_backend_profile` | Define the missing backend_profile or remove the tier.                                       | `compile` | `escalation/policy-unknown-backend-profile`      |
| `escalation_policy_ambiguous_at_compile` | Resolve the policy binding ambiguity or set explicit precedence.                             | `compile` | `escalation/policy-ambiguous`                    |
| `escalation_policy_unsafe_for_side_effect_stage` | Remove the policy from the side-effect stage or split the stage.                             | `compile` | `escalation/policy-unsafe-side-effect`           |
| `escalation_policy_disabled`             | Re-enable the policy through runtime override or catalog edit plus external drift acknowledgement. | `runtime` | `escalation/policy-disabled`                     |
| `escalation_kill_switch_engaged`         | Disable CHAINWORKS_ESCALATION_FORCE_PRIMARY to resume normal tier advancement.               | `runtime` | `escalation/kill-switch-engaged`                 |
| `escalation_chain_exhausted`             | Extend the chain or accept terminal pause.                                                   | `runtime` | `escalation/chain-exhausted`                     |
| `capacity_probe_failed`                  | Inspect provider transport health and retry once cleared.                                    | `runtime` | `escalation/capacity-probe-failed`               |
| `provider_session_force_detached`        | Inspect provider state and resume manually; tier does not auto-advance.                      | `runtime` | `escalation/provider-session-force-detached`     |
| `escalation_recovery_inconsistent`       | Use v1 cancellation unstick; marker records originating trigger.                             | `runtime` | `escalation/recovery-inconsistent`               |
| `escalation_repeated_digest_no_progress` | Inspect digest evidence and adjust policy or pause.                                          | `runtime` | `escalation/repeated-digest-no-progress`         |
| `escalation_deadline_elapsed`            | Extend wall-clock budget or accept pause.                                                    | `runtime` | `escalation/deadline-elapsed`                    |
| `human_tier_deadline_elapsed`            | Resume with operator decision or extend human tier deadline.                                 | `runtime` | `escalation/human-tier-deadline-elapsed`         |
| `escalation_policy_drift`                | Acknowledge escalation policy drift through the external MCP/operator workflow or restart with the new policy. | `runtime` | `escalation/policy-drift`                        |

## Metrics Emission

The system emits various metrics to monitor the health and performance of the escalation chains.

-   **`escalation_chains_started_total`**: Source: `escalation_ledger` insert. Surface: `runtime_facts` -> GraphQL/MCP/report.
-   **`escalation_tier_success_rate`**: Source: `agent_executions` grouped by `escalation_tier_id`/outcome. Surface: GraphQL/report.
-   **`time_to_success_after_escalation_seconds`**: Source: `ledger.created_at` to first success after tier advancement. Surface: GraphQL/report histogram.
-   **`escalation_pause_total`**: Source: `escalation_events.pause_reason_raw`. Surface: GraphQL/MCP/report.
-   **`false_escalation_rate`**: Source: escalated success where prior tier would have succeeded after followup adjudication. Surface: report; gates Phase 4.
-   **`policy_compile_failure_total`**: Source: `runs.preflight` diagnostics. Surface: GraphQL/preflight.
-   **`shadow_tier_selection_match_rate`**: Source: shadow decision compared with reviewer/operator adjudication. Surface: `runtime_facts`/report; gates Phase 2.
-   **`provider_session_kill_latency_seconds`**: Source: force-detach request to terminal commit. Surface: GraphQL histogram with p95 < 30s and max < 120s SLO.
-   **`daemon_outage_credit_seconds_total`**: Source: `ClockProvider` outage credit applied to deadlines. Surface: `runtime_facts` -> GraphQL.
-   **`fan_out_blocked_dwell_seconds`**: Source: sibling-block dwell start/end. Surface: GraphQL histogram.
-   **`launch_recycle_storm_total`**: Source: `escalation_launch_recycle_storm` event. Surface: GraphQL/report.
-   **`capacity_probe_failure_total`**: Source: capacity probe failures/timeouts. Surface: `runtime_facts` -> GraphQL.
-   **`escalation_drift_pending_ack_dwell_seconds`**: Source: `escalation_policy_drift` pause opened to external acknowledgement command commit. Surface: GraphQL/report alert threshold in Phase 1.
-   **`tier_dwell_share_of_chain`**: Source: `agent_execution_runtime_facts` tier dwell divided by remaining chain budget. Surface: GraphQL/report.
-   **`chain_exhausted_total_by_terminal_tier_kind`**: Source: `escalation_chain_exhausted` grouped by final `tier_kind`. Surface: GraphQL/report counter.
-   **`escalation_repeated_digest_no_progress_total`**: Source: `repeated_digest_no_progress` reaching per-tier or chain ceiling, labelled by `terminal_tier_kind` and `trigger_raw`. Surface: GraphQL/report; informs v2 default for `abort_after_repeated_digest_tier_count`.
-   **`escalation_commit_contention_total`**: Source: scheduler transaction `SQLITE_BUSY`/`LOCKED` bounded retries. Surface: `runtime_facts` -> GraphQL/report.
-   **`escalation_retry_after_parse_anomaly_total`**: Source: `Retry-After` parse anomaly runtime facts. Surface: GraphQL/report.
-   **`escalation_provider_late_frame_after_detach_total`**: Source: late provider frames dropped after force-detached commit. Surface: event journal/report.

## Provider Classifier Contract

This contract defines how provider responses are classified to determine escalation behavior.

**Adapter Inputs:**
`terminal_status_code`, `provider_error_class`, `retry_after`, `transport_state`

**Ambiguous Default:**
Fail closed as `transport_failure` pause; never auto-advance to a quota tier from ambiguous adapter data.

**Phase:**
Required before Phase 3 quota behavior.

**Precedence:**
`operator_forced_reserved_rejected`, `quota_with_valid_retry_after`, `transport_failure`, `ambiguous_failure`

## Defaults

-   **`abort_after_repeated_digest_tier_count`**: unset means no extra pause beyond per-tier retry budget and `max_chain_attempts`; when set, counter is per-tier.
-   **`capacity_probe`**: 10s timeout; 3 consecutive failures pause with `capacity_probe_failed`.
-   **`capacity_probe_counter`**: persisted in `escalation_execution_metadata`; resets on first successful probe, explicit operator resume after capacity pause, or new chain; probe attempts do not consume `tier_attempt_index` or `max_chain_attempts` but debit chain wall clock.
-   **`fan_out_blocked_dwell`**: observability-only in v1; `max_chain_wall_clock_seconds` remains the hard bound; `escalation_drift_pending_ack_dwell_seconds` alerts on drift pauses.
-   **`human_tier_max_wall_clock_seconds`**: 86400.
-   **`launch_recycle_storm`**: 3 recycles within 300s chain-wide pauses with launch storm event.
-   **`max_chain_wall_clock_seconds`**: policy required; all waits, probes, retries, and outage credit accounting are bounded by this hard chain limit.
-   **`non_human_tier_deadline`**: bounded by `min(provider attempt timeout, remaining_chain_budget divided by remaining non-pause tiers)` unless policy explicitly declares `tier_max_wall_clock_seconds_by_kind`.
-   **`outage_credit_cap_seconds`**: 3600.
-   **`outage_credit_pool`**: applies independently per deadline, including human-tier deadline, whether or not an operator notification was delivered during the daemon outage; each pool is capped by `outage_credit_cap_seconds`.
-   **`retry_after_clamp_seconds`**: [5, 900].
-   **`retry_after_parser_order`**: numeric seconds, then HTTP-date, then `parse_anomaly` runtime fact with lower-bound clamp; past-due values clamp to 5s.
-   **`retry_after_precedence`**: chain, human, and per-tier deadlines win over provider `Retry-After`; if clamped `Retry-After` plus outage credit exceeds a deadline, pause immediately with `escalation_deadline_elapsed` or `human_tier_deadline_elapsed` instead of waiting.
-   **`shutdown_drain_seconds`**: 30.
-   **`sqlite_busy_retry`**: max 8 scheduler-transaction retries on `SQLITE_BUSY` or `SQLITE_LOCKED`, exponential backoff with full jitter, total contention wait <= 200ms, non-attempt-consuming, emits `escalation.commit_contention.retry` and `escalation_commit_contention_total`.

## Operating Rationale

Escalation policies prevent repeated blockers from spending time and quota on the same capability tier without a durable recovery path. Operators need stable tier, trigger, pause, and policy attribution in run state and readback so they can distinguish retryable runtime failures from policy drift, quota exhaustion, recovery inconsistency, and human-pause states.

The stable boundary is intentional: Rust owns escalation lifecycle authority, while governed macOS only renders readback, attention, diagnostics, and external handoff affordances.

## Stable Guarantees

- Define `escalation_policy_v1` in repo-owned catalog or workflow data using `backend_profile` ids, not engine-hardcoded model names.
- Support ordered tiers with tier kinds `same_backend_retry`, `backend_profile`, `lead_mediation`, and `pause`.
- Support typed triggers for repeated blocker digest, contract output failure, stale or no-output attempt, quota, transport failure, loop-budget threshold, and reserved operator-forced vocabulary.
- Freeze `policy_hash`, `digest_version`, policy binding, tier order, trigger vocabulary, and rollout override state into `RunPlanSnapshot` and `RunPlan` compiled truth.
- Persist escalation ledger, execution metadata, runtime facts, and event journal rows with stable idempotency and no overlapping active tier.
- Expose raw-string GraphQL, MCP, report, and macOS readback with forward compatibility for unknown trigger, tier, pause, and event values.
- Make all non-progress states visible through pause reasons, `operator_action_hint`, `runbook_anchor`, metrics, and readback.
- Preserve compile validation, readback parity, gated `code_writer` behavior, quota/mediation safety, and non-side-effect catalog adoption controls.

## Boundaries

- Do not hardcode model names in Rust engine or Swift UI code beyond catalog data.
- Do not auto-escalate release, publishing, or other side-effect stages unless side-effect safety checks explicitly allow the policy at compile time.
- Do not bypass capability, permission, provider, quota, transport, or recovery validation for an escalated tier.
- Do not ship arbitrary operator-forced tier mutation in v1; `operator_forced_escalation` remains reserved rejected vocabulary.
- Do not add a governed macOS GraphQL mutation for policy-drift acknowledgement in v1.
- Do not implement destructive database rollback; rollback is behavior-disabling and data-preserving.

## Rollout And Adoption State

### Phase 0
- Implemented scope: strict schema compile-only, SDL/report/YAML/JSON fixtures, `pause_reason` vocabulary, event catalog, `runs.preflight` compile diagnostics, no scheduler behavior change.

### Phase 1
- Implemented scope: persistence/readback, `EscalationReadAdapter`, GraphQL/MCP/report parity, migration drill, `redaction_version`, pre-escalation null-tolerant resume, read-only `DriftReviewSheet` external handoff.

### Phase 1b
- Historical scope: shadow tier selection persisted `would_select_tier_id`/trigger/decision without acting; this has been superseded by scheduler-owned durable tier advancement while retaining the `would_select_*` fields as compatibility readback.

### Phase 2
- Implemented scope: gated `code_writer` escalation behavior with scheduler-owned `same_backend_retry`, `backend_profile`, `lead_mediation`, and `pause` tier handling. The current runtime path enforces kill-switch, chain-deadline, provider force-detach, launch-storm, late-frame event journaling, default capacity-probe-threshold pauses, and shadow/readback agreement.

### Phase 3
- Implemented baseline: provider quota waits, dwell/timeouts, recovery invariants per trigger, and classifier contract hardening are covered by the retained gate for the local implementation slice.

### Phase 4
- Broader non-side-effect catalog adoption remains release-governed. Default-enable decisions use `false_escalation_rate` < 5%, tier success > 0.6, shadow match > 0.95, primary p95 wall-clock regression < 10%, 100% runbook coverage, and P096 live evidence.

## Operational Guardrails

- **macOS write-boundary leak:**
  - Mitigation: `DriftReviewSheet` is read-only; external MCP/operator workflow owns acknowledgement; SwiftUI has fixture coverage proving no mutation path.
- **Scheduler regression:**
  - Mitigation: global and per-policy kill-switch, shadow phase, numeric promotion gates, and in-flight toggle fixtures.
- **Buggy migration projection:**
  - Mitigation: forward-only migration, projection consumer rollback, populated migration drill, and null-policy compatibility for pre-escalation runs.
- **Pause strings diverge:**
  - Mitigation: server-owned `pause_reason`/`operator_action_hint`/`runbook_anchor` catalog plus client presentation style for labels and raw-code accessibility.
- **Opaque repeated blocker triggers:**
  - Mitigation: `digest_version`, typed `digest_inputs`, redacted evidence refs, and repeated-digest exhaustion metric with terminal labels.
- **Provider cancellation advances incorrectly:**
  - Mitigation: force-detach pauses with `provider_session_force_detached`, no automatic advancement, replay idempotency, and late-frame drop event.
- **Operator restart trips storm detection:**
  - Mitigation: SIGTERM drain through CAS; operator restarts inside drain do not increment storm accounting.
- **Operators confuse shadow predictions with executed truth:**
  - Mitigation: shadow rows use 50% opacity, dashed rule, italic trigger, eye prefix, and never active fill or capsule truth color.
- **One slow tier consumes full chain budget:**
  - Mitigation: default non-human per-tier wall-clock share prevents starvation unless policy explicitly overrides.
- **UI divergence across surfaces:**
  - Mitigation: shared `EscalationPresentationStyle`, named component inventory, density ladder, state matrix, and snapshot fixtures.

## Deferred Decisions

- **Should reserved `operator_forced_escalation` become an authorized `command_journal`-backed MCP/GraphQL mutation?**
  - Current stance: defer; v1 reserves and rejects vocabulary only.
- **Should provider force-detach hard ceiling become policy-configurable?**
  - Current stance: keep 120s hard ceiling until `provider_session_kill_latency_seconds` proves stable.
- **Should `fan_out_blocked_dwell_seconds` become a hard pause condition after soak?**
  - Current stance: keep observability-only in v1; `max_chain_wall_clock_seconds` is the hard bound.
- **Should server own humanized labels instead of the macOS frozen label catalog?**
  - Current stance: client owns labels in v1 while raw strings remain authoritative; revisit only if another client needs identical wording.
- **Should policy-drift acknowledgement become a governed macOS action?**
  - Current stance: defer; this requires an explicit update to `docs/reference/ui-action-boundary.md`, mutation authorization, command journal fields, audit model, failure modes, and fixtures.

## Migration And Recovery Evidence

- **Drill:** populated SQLite fixture with mixed pre/post-escalation runs, forward migration, projection rebuild, MCP/GraphQL/report parity assertions, no row-count drift on `agent_executions`, `escalation_ledger` empty for pre-escalation runs, `redaction_version` stamped
- **Phase 3 Artifact Drill:** release evidence keeps fixtures proving recovery does not silently discard settled `mediation_response` or `human_decision` artifacts before broad GA
- **Phase 3 Shutdown Drill:** implementation tests prove startup force-detach replay for a running escalation execution, no `InvokeAgent` relaunch, paused ledger, runtime facts, failed stage, and blocked run; release evidence may add a live SIGTERM soak showing `force_detach_replay` and `late_frame_after_detach` metrics under operator restart conditions
- **Resume:** pre-escalation snapshots read null `policy_id` as inactive ledger; later `escalation_policies` edits produce `escalation_policy_drift` requiring external operator acknowledgement
- **Rollback:** data-preserving rollback disables behavior via kill-switch, drops projection consumers, and leaves committed columns/tables intact
- **Stance:** forward-only; no destructive down-migration.

## macOS Read Surface

The governed macOS surface is a read-only presentation layer over GraphQL/subscription DTOs. `EscalationReadAdapter` owns status capsule, banner stack, command rows, lineage, pause card, trace timeline, drift review sheet, MenuBarExtra aggregation, attention requests, pasteboard trace copy, and inspector presentation. SwiftUI bodies consume immutable presentation models and do not parse escalation JSON or reconstruct policy truth.

The component contract covers accessibility labels with state/tier/trigger/raw IDs, compact co-occurrence banners, density selection by call site, non-collapsed lineage disclosure, shadow rows, pause-card responsive fallback, read-pipeline states, and atomic redacted trace copy.

## Wire Contracts

The escalation system defines strict wire contracts for events, GraphQL, macOS write boundaries, MCP, and reports. These contracts ensure data consistency, forward compatibility, and proper interaction between different components of the system.

### Event Catalog

Raw event names are authoritative; clients must round-trip unknown values through DTOs, MCP, reports, and snapshots:

- `escalation.tier_selected`
- `escalation.tier_rejected`
- `escalation.tier_advanced`
- `escalation.chain_exhausted`
- `escalation.deadline_elapsed`
- `escalation.ledger.status_changed`
- `escalation.recovery_inconsistent.cancelled`
- `escalation.kill_switch.force_primary_engaged`
- `escalation.kill_switch.policy_disabled`
- `escalation.shadow.tier_selected`
- `escalation.operator_forced.reserved_rejected`
- `escalation.compile_failed`
- `escalation.provider_late_frame_after_detach`
- `escalation.retry_after.parse_anomaly`
- `escalation.commit_contention.retry`
- `escalation.drift.pending_ack_dwell_threshold`

### GraphQL

- Raw `status`, `trigger`, `tier`, `tier_kind`, `pause_reason`, and event fields are authoritative strings; nullable known helpers are compatibility sugar only.
- `featureFlagState`, `wouldSelectTierId`, `wouldSelectTriggerRaw`, `digestVersion`, `digestInputs`, `redactionVersion`, `waitingRetryAfterUntil`, `traceUnavailableReasonRaw`, `escalationTraceJsonRedacted`, `policyDriftState`, and `externalAcknowledgementRef` appear in SDL snapshots. Scheduler and event-derived fields are populated from durable escalation ledger/event truth when present and remain `null` for legacy or non-escalated runs.
- Unknown trigger, pause_reason, tier_kind, and event strings must round-trip through Swift DTOs, MCP, reports, and snapshots.

### MCP

- `runs.get` and `run://{run_id}` expose the same frozen escalation fields as GraphQL.
- `runs.preflight` surfaces compile-time policy failures with `pause_reason` codes, `operator_action_hint`, and `runbook_anchor`.
- MCP readback never reconstructs truth from current YAML, logs, or mutable registries.
- Policy drift acknowledgement for v1 is an external MCP/operator command path with command journal, idempotency key, principal, frozen `policy_hash`, current `policy_hash`, decision, `created_at`, and result; it is not called by governed macOS.

### macOS Write Boundary

- Governed SwiftUI reads GraphQL/subscription DTOs only for escalation state.
- `EscalationReadAdapter` may request readback refreshes, copy redacted traces to pasteboard, open https runbook URLs, present notifications, and request AppKit attention.
- `EscalationReadAdapter` may not call policy-drift acknowledgement, tier mutation, retry, resume, cancel, or force-primary mutations.
- `DriftReviewSheet` is read-only, with Copy acknowledgement command details and Open external workflow affordances. UI updates only after GraphQL readback reports drift cleared.

### Reports

- Active reports share resource id; terminal v1 reports remain immutable; successor reports mark `superseded_by`.
- `redaction_version` stamps each escalation projection and report write.

## Rollout Gate

The retained escalation proof gate documented in [test-gates](test-gates.md) is the canonical local proof lane for this contract. It covers schema compile/readback parity, durable ledger/metadata/events, runtime facts, GraphQL/MCP/report projections, governed macOS component fixtures, metric inventory, and idempotency. Its rollback disposition is data-preserving: disable escalation-policy behavior while keeping readback for committed rows.

## Release Evidence

P096 owns the release-proof envelope around this implemented contract: remote visual/runtime proof, Full Keyboard Access traversal, contrast and reduced-motion artifacts, scene restoration and multi-window runtime proof, long-run metric-threshold trend capture, and live operational drills. Missing P096 evidence blocks broad release/default-enable decisions, but does not change the implemented local escalation-policy contract described here.
