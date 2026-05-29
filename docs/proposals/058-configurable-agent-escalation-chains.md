# Proposal 058: Configurable Agent Escalation Chains

> Source: current unfinished-run proposal artifact.

## Metadata

- **Source run:** `6764a0c2-456c-4643-95da-06e213a6dc91`
- **Source artifact:** `.chainworks/runs/6764a0c2-456c-4643-95da-06e213a6dc91/proposals/approved/proposal.md`
- **Source md5:** `ba2dcc83cc8fad84f33f1be4f91c0dfc`
- **Proposal Revision Id:** p058-r14-2026-05-07
- **Schema Version:** proposal_current_v2
- **Document Format:** proposal_json_v1
- **Status:** implementation_reaudit_ready
- **Date:** 2026-05-07
- **Run Id:** 6764a0c2-456c-4643-95da-06e213a6dc91
- **Source Review Pass Id:** p058-r13-aggregate-2026-05-07-state_4_proposal_reviewed
- **Previous Proposal Revision Id:** p058-r13-2026-05-06

## Summary

- Introduce repo-owned escalation_policy_v1 declarations for ordered retry and escalation tiers across same-backend retry, backend_profile escalation, lead mediation, and terminal human pause.
- Rust control plane remains the only authority for policy resolution, trigger classification, tier advancement, pause/resume legality, capacity checks, persistence, recovery, and kill-switch behavior.
- Governed macOS is a read/subscription presentation surface for escalation state. The r13 DriftAcknowledgementSheet blocker is resolved by making the sheet read-only and routing drift acknowledgement through the existing MCP/operator workflow; SwiftUI performs no policy-drift mutation in v1.
- The proposal now pins implementation-grade contracts for deadlines, force-detach, shutdown drain, SQLite contention, outage credit, repeated-digest observability, macOS actor ownership, SF Symbols, attention requests, dock badges, pasteboard writes, narrow layouts, keyboard focus, read-pipeline states, command rows, menu-bar layout, and shadow-row visuals.

## Implementation Sync

- **Last synchronized:** 2026-05-29
- **Implementation worktree:** `.chainworks/worktrees/cw-configurable-agent-escalation-6764a0c2`
- **Current proof gate:** `./scripts/test-gate.sh proposal-058`
- **Current implemented runtime slice:** scheduler-owned durable tier advancement for `same_backend_retry`, `backend_profile`, `lead_mediation`, and `pause`; post-invoke authority uses durable `p058_claimed` identity; startup recovery preserves P058 claims; provider `retry_after` blocks claim/start capacity; `runs.start` path inputs are canonicalized and root-confined; escalation readback is derived from durable events; `CHAINWORKS_ESCALATION_FORCE_PRIMARY`, chain wall-clock deadline, and capacity-probe threshold pauses are enforced before launching an escalation retry; GraphQL/MCP focused parity proves non-null scheduler readback fields from durable event/runtime data.
- **Current governed macOS slice:** `EscalationReadAdapter` is the sole governed SwiftUI source for status capsule, banner stack, lineage, pause card, trace timeline, drift review sheet, trace pasteboard copy, menu/attention aggregation, and inspector presentation. Focused Swift tests in `proposal-058` now cover the component presentation contracts, all-run attention aggregation, MenuBarExtra overflow routing, retained inspector/shared-adapter behavior, trace pasteboard copy, command disabled-reason parity, drift diff presentation, actual `DriftReviewSheet` structured tier/trigger/max-attempt inputs, compact banner co-occurrence summarization, pause-card ultra-narrow fallback, non-collapsed lineage disclosure controls, required SF Symbol resolution, pause countdown formatting, lineage duration/ref disclosure, and status field-order/truncation/accessibility behavior.
- **Implementation closeout status:** the implementation contract is ready for repeat audit after `./scripts/test-gate.sh proposal-058` passed with the expanded 45-test Swift P058 suite and control-plane gate. Release-only evidence that needs a live/remote environment is explicitly moved to P096 (`096-p058-release-evidence-and-macos-runtime-proof.md`): remote visual soak, Full Keyboard Access runtime walk, contrast/reduced-motion screenshots, long-run metric-threshold trending, and live operational drills. Those items are GA/release proof, not missing P058 implementation paths.

## Problem

### Impacts
- same backend profile can be retried despite repeated blockers
- operators lack durable tier, trigger, pause, and policy attribution in run state and readback
- provider quota, transport failures, stale outputs, and contract-output failures do not consistently select a different recovery path
- safe rollout needs strict compile validation, operational levers, readback parity, and UI clarity before scheduler behavior changes
- without a stable read/write boundary, macOS could accidentally become an escalation lifecycle authority
- **Statement:** Chainworks currently has isolated conflict mediation and same-profile implementation retries, but operators cannot declare an auditable ordered escalation chain. Repeated blockers can burn time and quota without changing capability tier or producing durable lineage.

## Goals

- Define escalation_policy_v1 in repo-owned catalog or workflow data using backend_profile ids, not engine-hardcoded model names.
- Support ordered tiers with tier kinds same_backend_retry, backend_profile, lead_mediation, and pause.
- Support typed triggers for repeated blocker digest, contract output failure, stale or no-output attempt, quota, transport failure, loop-budget threshold, and reserved operator-forced vocabulary.
- Freeze policy_hash, digest_version, policy binding, tier order, trigger vocabulary, and rollout override state into RunPlanSnapshot and RunPlan compiled truth.
- Persist escalation ledger, execution metadata, runtime facts, and event journal rows with stable idempotency and no overlapping active tier.
- Expose raw-string GraphQL, MCP, report, and macOS readback with forward compatibility for future trigger, tier, pause, and event values.
- Make all non-progress states visible through pause reasons, operator_action_hint, runbook_anchor, metrics, and readback.
- Roll out through compile-only, readback, shadow selection, gated code_writer behavior, quota/mediation, and broader non-side-effect catalog adoption.

## Non Goals

- Do not hardcode model names in Rust engine or Swift UI code beyond catalog data.
- Do not auto-escalate release, publishing, or other side-effect stages unless side-effect safety checks explicitly allow the policy at compile time.
- Do not bypass capability, permission, provider, quota, transport, or recovery validation for an escalated tier.
- Do not ship arbitrary operator-forced tier mutation in v1; operator_forced_escalation remains reserved rejected vocabulary.
- Do not add a governed macOS GraphQL mutation for policy-drift acknowledgement in v1.
- Do not implement destructive database rollback; rollback is behavior-disabling and data-preserving.

## Ux Ui Notes

### Macos
- **Accessibility**
  - **Activation:** Space or Return toggles lineage retry groups and row disclosures; Esc cancels sheets only; tier capsule context menu is reachable through standard menu focus
  - **Contrast:** NSColor asset-catalog roles must meet contrast >= 4.5:1 under Light, Dark, and Increase Contrast
  - **Focus:** when a blocking banner appears, focus moves to the highest-precedence banner and returns to prior focus when it clears
  - **Full Keyboard Access Tab Order**
    - highest-precedence banner stack
    - status capsule context menu
    - lineage rows top-to-bottom
    - expanded lineage disclosure controls
    - pause-card Open runbook
    - pause-card Copy diagnostic bundle
    - trace disclosure
    - external drift workflow controls
  - **Reduced Motion:** tier-advance transitions use crossfade instead of movement
  - **Voiceover Order**
    - state
    - tier label
    - trigger label
    - full raw ids
- **Authority Boundary**
  - EscalationReadAdapter is the sole governed UI source for run detail, inspectors, notifications, shortcuts, command enablement, trace copy, banner state, pause cards, and lineage views.
  - No SwiftData, local workflow compiler output, WorkflowOrchestrator fallback, context-strategy escalation counters, logs, or local model-tier selection may appear as escalation_policy_v1 truth.
  - DTO decode and trace normalization stay off-MainActor and Sendable. Presentation publication occurs on MainActor. SwiftUI bodies consume immutable presentation models and never parse JSON.
  - All windows and inspectors for the same run subscribe to one shared adapter keyed by run_id. Restored scenes render a loading escalation state until the shared publisher emits a current snapshot.
  - waitingRetryAfterUntil countdown expiry is display-only and triggers fresh GraphQL readback; it never locally enables commands or advances state.
  - Policy drift acknowledgement is not a governed macOS write in v1. The drift review surface is read-only and hands off to MCP/operator workflow.
- **Components**
  - **Driftreviewsheet**
    - **Actions**
      - Copy acknowledgement command details
      - Open external workflow
      - Cancel
    - **Dismissal:** interactiveDismissDisabled-equivalent; Esc maps to Cancel/Close only; parent window close while sheet is presented requires explicit operator dismissal and never acknowledges drift
    - **Empty Diff State:** Drift cleared while loading; only Close/Cancel enabled
    - **Entry Point:** Review drift action on policy_drift banner
    - **Layout:** sheet attached to the run-detail window with structured frozen/current policy diff: tier list added/removed/changed badges, max_chain_attempts delta, trigger list delta, policy_hash values, run_id, and external acknowledgement command details
    - **Write Boundary:** read-only in governed macOS; no server mutation is called
  - **Escalationbannerstack**
    - **Compact Cooccurrence:** highest-precedence symbol plus numeric count chip (+N) in tertiaryLabel role; tooltip lists suppressed banner titles in precedence order
    - **Dismissibility:** blocking banners are not dismissible while the underlying GraphQL state remains active; shadow_mode is per-session dismissible only
    - **Placement:** stable vertical top-of-content strip below the run-detail toolbar; never inside a card
    - **Precedence Top To Bottom**
      - kill_switch
      - policy_drift
      - policy_disabled
      - recovery_inconsistent
      - capacity_probe_failed
      - shadow_mode
  - **Escalationcommandpresentation**
    - **Disabled Reason Rule:** titles remain stable; unavailable reason appears in subtitle, help, accessibilityHint, and tooltip
    - **Max Title Length:** 48 displayed characters with middle truncation; full value in help/accessibility
    - **Mirror Rule:** CommandPalette and context-menu mirrors carry identical slot values; only surrounding chrome differs
    - **Slots**
      - stable title
      - subtitle or disabled reason
      - optional state badge
  - **Escalationlineageview**
    - **Columns:** tier_id and outcome fixed minimums; humanized trigger flexes; attempt_index and duration right-align with monospace digits
    - **Layout:** vertical stepper with Tier 0 baseline at top and ledger rows ordered by created_at below it
    - **Narrow Width:** below 480pt, collapse to two-line card-style rows; horizontal scroll is forbidden
    - **Retry Collapse:** when 3 or more consecutive same_backend_retry attempts occur on the same tier_id, collapse into one expandable row labeled Retry n / max_chain_attempts with latest trigger shown
    - **Row Disclosure:** expanded row reveals digest_inputs, redacted_evidence_ref, redaction_version, and runtime fact refs; raw evidence is never rendered
    - **Shadow Rows:** 50% opacity, leading dashed vertical rule in systemGray, italic trigger label, and eye SF Symbol prefix; shadow rows are aligned to the actual row they would have replaced and never use active-row fill weight
  - **Escalationpausecard**
    - **Countdown Format:** <60s as Ns, <1h as M:SS, <24h as H:MM:SS, >=24h as Dd Hh; clamp at 0 and replace with Ready to retry or Deadline elapsed after server state changes
    - **Layout Bounds:** ideal width 480pt, minimum readable width 320pt; below 360pt, affordance row stacks vertically and metadata moves under body; below 280pt, render a one-line summary with Open inspector affordance rather than horizontal scroll
    - **Slots**
      - title/code row
      - plain-text body
      - affordance row
      - metadata strip
  - **Escalationstatuscapsule**
    - **Collapse Order**
      - drop trigger label
      - drop tier label
      - never drop state pill
    - **Field Order:** state pill, middle-dot separator, tier label, middle-dot separator, trigger label
    - **Same Backend Retry Color Rule:** Tier 0 baseline uses secondaryLabel; same_backend_retry active uses controlAccentColor; same_backend_retry exhausted-but-not-yet-advanced uses systemOrange
    - **Slots**
      - state pill
      - tier label
      - trigger label
    - **Suppression:** suppressed entirely when policy_id is null; when trigger is null on a configured policy, show state plus Tier 0 or Standard Execution in Standard and Detailed densities
    - **Tier Symbols Catalog:** SF Symbols resolved against macOS 26.2 / Xcode 26.3 baseline by fixture; missing-symbol fallback fails the fixture
    - **Truncation:** 24-character middle truncation applies only to displayed labels at layout time; full ids remain in tooltip, help, and accessibilityLabel
  - **Escalationtracetimeline**
    - **Export Rule:** Copy Escalation Trace writes .string and public.json UTF-8 in one NSPasteboard declareTypes/setData generation using byte-identical escalationTraceJsonRedacted content and the same redaction_version stamp
    - **Placement:** sibling section under EscalationLineageView in EscalationInspector with section header Trace and disclosure-collapsed default; lineage and timeline never move to separate tabs
  - **Menubarextra**
    - **Badge Format:** numeric count when >0, no badge when 0
    - **Empty State:** No paused escalation runs
    - **Layout:** aggregate paused-run count badge plus at most 5 per-run rows sorted by most-recent escalation transition
    - **Overflow:** after 5 rows, show Show all paused runs...
    - **Width Budget:** state pill and count only in the menu-bar item; tier and trigger appear in tooltip or menu rows, not the compact item
- **Cooccurrence Resolution:** Capsule state reflects underlying scheduling state, not banner severity. Primary action is taken from the highest-precedence banner with a non-null action. Example kill_switch + policy_drift + shadow_mode shows kill-switch banner first, policy-drift review second, shadow ghost rows in inspector, and no local tier mutation.
- **Density Ladder**
  - **Compact**
    - **Call Sites**
      - RunListRow
      - SidebarRunCell
      - MenuBarExtra
      - notification subtitle
    - **Rules**
      - suppress Tier 0 baseline
      - suppress chain track
      - suppress shadow ghost rows
      - show state-only capsule when policy_id is present and non-idle
      - MenuBarExtra renders aggregate paused-run count and state pill only, never tier or trigger text
  - **Detailed**
    - **Call Sites**
      - EscalationInspector
      - ApprovalQueueItem
    - **Rules**
      - render complete lineage
      - render shadow ghosting
      - render digest disclosure
      - render pause-card metadata
      - always include Tier 0 in accessibility labels
  - **Standard**
    - **Call Sites**
      - RunDetailHeader
    - **Rules**
      - render Tier 0 as Standard Execution
      - render EscalationStatusCapsule
      - render banner stack below toolbar
      - render one-line chain summary
  - **Selection Rule:** Density is fixed per call site and asserted in fixtures; it is not chosen responsively at runtime.
- **Fixtures**
  - presentation snapshot per component, density, screen state, read-pipeline state, and co-occurrence tuple
  - SF Symbol resolution fixture for every referenced symbol against macOS 26.2 / Xcode 26.3
  - Swift strict-concurrency guard for Sendable DTOs, off-MainActor decode, and MainActor publication
  - scene restoration fixture proving restored windows wait for the shared run_id publisher
  - multi-window fixture proving all inspectors receive the same adapter update
  - Dock badge aggregation fixture across pause/resume/session restart
  - NSApp.requestUserAttention informational request and cancellation-token fixture
  - NSPasteboard fixture proving .string and public.json are written atomically with byte-identical redacted trace
  - Full Keyboard Access tab-order fixture
  - DriftReviewSheet fixture proving no mutation is called and external handoff details are copyable
- **Notifications**
  - **Dock Badge:** derived from live aggregation across runs in paused_runtime, paused_compile, exhausted, force_detached, recovery_inconsistent, and policy_drift; recomputed on every adapter snapshot and never driven by notification click handlers
  - **Forbidden Fields**
    - escalationTraceJsonRedacted
    - digest_inputs
    - redacted_evidence_ref
    - trigger candidate fragments
    - raw JSON payloads
  - **Human Tier Attention:** paused runs requiring operator action increment Dock badge and call NSApp.requestUserAttention(.informationalRequest) when backgrounded; MainActor-held cancellation token is cancelled on app activation or pause clear
- **Presentation Style Owner:** EscalationPresentationStyle centralizes tier and severity color roles, SF Symbol names, truncation helpers, humanized label lookup, shadow-row styling, and accessibility raw-code formatting. Capsules, banners, lineage, pause cards, command rows, notifications, and MenuBarExtra consume this shared layer.
- **Read Pipeline States**
  1. **Item**
     - **Presentation:** skeleton capsule in Standard/Detailed, no Compact badge, commands disabled with pending reason
     - **State:** subscription_establishing
  2. **Item**
     - **Presentation:** last-known snapshot remains visible dimmed if available; otherwise skeleton; no local state changes
     - **State:** dto_decoding
  3. **Item**
     - **Presentation:** highest-precedence transport banner above escalation banners; last-known snapshot labelled stale; Copy trace disabled unless cached redacted trace is still marked current
     - **State:** transport_disconnected
  4. **Item**
     - **Presentation:** dimmed stale indicator with refresh readback affordance; countdowns pause visually and do not synthesize readiness
     - **State:** snapshot_stale
  5. **Item**
     - **Presentation:** normal component rendering from immutable presentation model
     - **State:** snapshot_ready
- **Run Detail Header Layout:** row 1 toolbar trailing EscalationStatusCapsule, row 2 full-width EscalationBannerStack below toolbar, row 3 leading one-line EscalationLineageSummary, with 8pt vertical gaps
- **Screen State Matrix**
  - no_policy
  - idle
  - probing
  - retry_waiting
  - tier_advancing
  - paused_compile
  - paused_runtime
  - policy_drift
  - shadow_only
  - exhausted
  - force_detached
  - recovery_inconsistent
  - kill_switch

## Architecture

- **Authority:** Rust control plane owns policy resolution, trigger classification, blocker digest calculation, capacity probing, tier advancement, retry budgets, pause/resume legality, kill-switch evaluation, shadow decisions, persistence, and recovery. Governed macOS renders frozen GraphQL DTOs and never reconstructs escalation truth from local workflow files, logs, SwiftData, compiler output, context-strategy counters, or local caches.
- **Binding Precedence:** Workflow-stage policy binding wins over agent binding only when explicit. Agent and backend_profile bindings with equal specificity are ambiguous unless a policy declares deterministic precedence. Ambiguity pauses at compile/preflight rather than selecting silently.
- **Blocker Digest:** digest_version escalation_blocker_digest_v1 uses typed inputs failure_kind, output_settlement_state, validation_evidence_kind, and redacted_message_fragment_hash. Readback exposes digest_inputs and redacted_evidence_ref, not raw evidence.
### Defaults
- **Abort After Repeated Digest Tier Count:** unset means no extra pause beyond per-tier retry budget and max_chain_attempts; when set, counter is per-tier
- **Capacity Probe:** 10s timeout; 3 consecutive failures pause with capacity_probe_failed
- **Capacity Probe Counter:** persisted in escalation_execution_metadata; resets on first successful probe, explicit operator resume after capacity pause, or new chain; probe attempts do not consume tier_attempt_index or max_chain_attempts but debit chain wall clock
- **Fan Out Blocked Dwell:** observability-only in v1; max_chain_wall_clock_seconds remains the hard bound; escalation_drift_pending_ack_dwell_seconds alerts on drift pauses
- **Human Tier Max Wall Clock Seconds:** `86400`
- **Launch Recycle Storm:** 3 recycles within 300s chain-wide pauses with launch storm event
- **Max Chain Wall Clock Seconds:** policy required; all waits, probes, retries, and outage credit accounting are bounded by this hard chain limit
- **Non Human Tier Deadline:** bounded by min(provider attempt timeout, remaining_chain_budget divided by remaining non-pause tiers) unless policy explicitly declares tier_max_wall_clock_seconds_by_kind
- **Outage Credit Cap Seconds:** `3600`
- **Outage Credit Pool:** applies independently per deadline, including human-tier deadline, whether or not an operator notification was delivered during the daemon outage; each pool is capped by outage_credit_cap_seconds
- **Retry After Clamp Seconds**
  - `5`
  - `900`
- **Retry After Parser Order:** numeric seconds, then HTTP-date, then parse_anomaly runtime fact with lower-bound clamp; past-due values clamp to 5s
- **Retry After Precedence:** chain, human, and per-tier deadlines win over provider Retry-After; if clamped Retry-After plus outage credit exceeds a deadline, pause immediately with escalation_deadline_elapsed or human_tier_deadline_elapsed instead of waiting
- **Shutdown Drain Seconds:** `30`
- **Sqlite Busy Retry:** max 8 scheduler-transaction retries on SQLITE_BUSY or SQLITE_LOCKED, exponential backoff with full jitter, total contention wait <= 200ms, non-attempt-consuming, emits escalation.commit_contention.retry and escalation_commit_contention_total
- **Idempotency Key:** run_id, stage_id, ledger_id, tier_id, tier_attempt_index. launch_recycle_index is excluded so recycle replay targets the same committed attempt.
- **Operational Controls:** Global CHAINWORKS_ESCALATION_FORCE_PRIMARY forces tier=primary at scheduling time. Per-policy enabled state comes from frozen policy plus runtime_policy_overrides. in_flight_toggle_behavior supports continue, pin, and pause; default is continue. The selected behavior is stamped on the ledger and surfaced through featureFlagState.
- **Overlap Free Tier Invariant:** escalation.tier_advanced is emitted only after the previous tier has a settled terminal outcome. No ledger may have more than one active tier, and force-detach windows cannot double-charge provider quota.
### Persistence
- **Agent Execution Columns**
  - escalation_policy_id
  - escalation_policy_hash
  - escalation_tier_id
  - escalation_tier_kind_raw
  - escalation_trigger_raw
  - escalation_digest_version
  - escalation_ledger_id
- **Fk Targets**
  - runs(id)
  - agent_executions(id)
- **Json Validation:** repository layer rejects malformed JSON even without sqlite json1
- **Shadow Columns**
  - agent_execution_runtime_facts.would_select_tier_id
  - agent_execution_runtime_facts.would_select_trigger_raw
  - agent_execution_runtime_facts.would_select_decision_json
- **Tables**
  - escalation_ledger
  - escalation_execution_metadata
  - escalation_events
- **Policy Drift:** Resume compares frozen policy_hash and binding data with current repo catalog. Drift opens escalation_policy_drift pause. In v1, acknowledgement is external through MCP/operator workflow, not a governed macOS write. After acknowledgement, Rust control plane records the durable state transition and GraphQL/MCP readback refreshes clients.
### Policy Schema
- **Required Fields**
  - policy_id
  - schema_version
  - enabled_default
  - applies_to
  - max_chain_attempts
  - max_chain_wall_clock_seconds
  - triggers
  - tiers
- **Strictness**
  - unknown escalation fields fail compile
  - unknown backend profiles fail compile with escalation_policy_unknown_backend_profile
  - ambiguous bindings fail compile with escalation_policy_ambiguous_at_compile
  - unsafe side-effect stage bindings fail compile with escalation_policy_unsafe_for_side_effect_stage
  - missing tier permission or runtime validation pauses fail-closed before scheduling
- **Tier Kinds**
  - same_backend_retry
  - backend_profile
  - lead_mediation
  - pause
### Provider Classifier Contract
- **Adapter Inputs**
  - terminal_status_code
  - provider_error_class
  - retry_after
  - transport_state
- **Ambiguous Default:** fail closed as transport_failure pause; never auto-advance to a quota tier from ambiguous adapter data
- **Phase:** required before Phase 3 quota behavior
- **Precedence**
  - operator_forced_reserved_rejected
  - quota_with_valid_retry_after
  - transport_failure
  - ambiguous_failure
- **Recovery:** Recovery-inconsistent triggers pause fail-closed with escalation_recovery_inconsistent. v1 unstick is cancellation with a recovery-cancelled marker preserving ledger order, originating trigger code, report successor immutability, partial-progress signal, and unstick latency metrics. The recovery-cancelled marker is committed inside the same scheduler transaction as the cancellation event, so replay observes either the full cancellation or none of it. Cancellation invokes provider force-detach with the same 120s ceiling. Provider sessions that have not settled by shutdown_drain_seconds are treated as crash-interrupted; on next start, force_detach_replay reissues the in-flight force-detach with the same idempotency key, and late frames are dropped and journaled as escalation.provider_late_frame_after_detach.
- **Scheduler Transaction:** Settlement, trigger selection, digest calculation, frozen policy lookup, ledger lookup, readiness validation, capacity validation, ledger/event/metadata updates, and work-queue insert commit in one SQLite transaction. Provider launch occurs only after commit. Housekeeping uses the same compare-and-swap path and is idempotent.

## Runtime Invocation

- **Agent Execution Id:** 01f9a1e6-b403-4259-b62c-8a8110ff2f8b
- **Session Generation Id:** 0b1586fa-3eee-484b-8677-ebfba8b53030
- **Stage Execution Id:** f76b774c-7363-42c8-b75c-67cce98f748c
- **Stage Id:** state_5_proposal_refined
- **Work Item Id:** p058-invoke:f76b774c-7363-42c8-b75c-67cce98f748c:0

## Rollout Contract V1

- **Schema Version:** rollout_contract_v1
- **Applicability:** required
### Gate Aliases
- proposal-058
- p058
### Commands
- **Allowlist**
  - ./scripts/test-gate.sh proposal-058
  - ./scripts/test-gate.sh p058
- **Commentary:** P058 gate validates escalation policy persistence and readback without live external side effects.
### Migrations
- **Not Applicable:** `false`
- **Description:** Escalation policy ledger, execution metadata, runtime facts, and readback/report projections are required for P058.
### Metrics
- **Adoption Metric:** escalation_chains_started_total
- **Operational Metrics**
  - escalation_chains_started_total
  - escalation_tier_success_rate
  - time_to_success_after_escalation_seconds
  - escalation_pause_total
  - false_escalation_rate
  - policy_compile_failure_total
  - shadow_tier_selection_match_rate
  - provider_session_kill_latency_seconds
  - daemon_outage_credit_seconds_total
  - fan_out_blocked_dwell_seconds
  - launch_recycle_storm_total
  - capacity_probe_failure_total
  - escalation_drift_pending_ack_dwell_seconds
  - tier_dwell_share_of_chain
  - chain_exhausted_total_by_terminal_tier_kind
  - escalation_repeated_digest_no_progress_total
  - escalation_commit_contention_total
  - escalation_retry_after_parse_anomaly_total
  - escalation_provider_late_frame_after_detach_total
### Readback Lanes
- run_report
- mcp
- release_receipt
- graphql
### Readback Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
- **Readback Fixture:** docs/evidence/rollout-contract/operator-readback/p058-full-surface.fixture.json
### Operator Report Fields
- rollout_contract_status
- rollout_contract_decision
- rollout_contract_failure_reasons
- rollout_contract_waiver_state
- rollout_contract_waiver_expires_at
- rollout_contract_enforcement_mode
- rollout_contract_enforcement_mode_reason
- rollout_contract_hold_conditions
- rollout_contract_rollback_disposition
- rollout_contract_source_lane
- rollout_contract_enabled_state
- rollout_contract_disabled_reason_code
- rollout_contract_action_id
- rollout_contract_operator_message
- rollout_contract_projection_integrity
- rollout_contract_cutover_policy_revision
- rollout_contract_diagnostic_redaction
- rollout_contract_next_steps
### Hold Conditions
- Escalation policy schema compile and readback parity is missing.
- Escalation ledger, runtime facts, or event journal rows are not durable.
- Provider force detach, capacity probe, retry-after, or deadline behavior lacks recovery evidence.
- Governed macOS reconstructs escalation truth or exposes policy drift mutation controls.
- P058 gate or readback fixture fails.
### Rollback Disposition
- **Mode:** disable_escalation_policy_behavior_keep_readback
- **Data Loss Risk:** low
- **Steps**
  - Disable escalation policy resolution and tier advancement behavior through the kill switch.
  - Keep escalation ledger, runtime facts, and readback/report projections available for audit.
  - Drop only projection consumers that depend on enabled behavior; preserve committed tables and columns.
  - Run migration and parity drills before re-enabling escalation behavior.
### Decision Vocabulary
- pass
- fail
- waived
- not_applicable
- timeout
- cancelled
- missing_contract
- tamper_detected
- stale
- release
- hold
- waive
### Negative Fixtures
- **Missing Escalation Readback:** docs/evidence/rollout-contract/negative/p058-missing-escalation-readback.json
### Cutover Policy
- **Revision:** p058-cutover-v1
- **Enforcement Mode At Cutover:** enforce
- **Applicable To:** post_cutover_implementation_starts
- **Grandfathered Rendering:** not_applicable
- **Effective Timestamp Iso8601:** 2026-05-07T00:00:00Z
- **Operator Message:** P058 rollout contract requires escalation policy behavior, recovery evidence, and operator readback parity before release.

## Rollout Plan

1. **Item**
   - **Phase:** Phase 0
   - **Scope:** strict schema compile-only, SDL/report/YAML/JSON fixtures, pause_reason vocabulary, event catalog, runs.preflight compile diagnostics, no scheduler behavior change
2. **Item**
   - **Phase:** Phase 1
   - **Scope:** persistence/readback, EscalationReadAdapter, GraphQL/MCP/report parity, migration drill, redaction_version, pre-escalation null-tolerant resume, read-only DriftReviewSheet external handoff
3. **Item**
   - **Phase:** Phase 1b
   - **Scope:** shadow tier selection persists would_select_tier_id/trigger/decision without acting; same-profile retry continues
4. **Item**
   - **Phase:** Phase 2
   - **Scope:** gated code_writer behavior with kill-switch fixtures, in_flight_toggle_behavior fixtures, deadlines, capacity probes, force-detach, launch storm, shadow match rate > 0.95
5. **Item**
   - **Phase:** Phase 3
   - **Scope:** provider quota and lead mediation, dwell/timeouts, recovery invariants per trigger, graceful shutdown drain, classifier contract required
6. **Item**
   - **Phase:** Phase 4
   - **Scope:** broader non-side-effect catalog adoption gated by false_escalation_rate < 5%, tier success > 0.6, shadow match > 0.95, primary p95 wall-clock regression < 10%, and 100% runbook coverage

## Metrics Emission

1. **escalation_chains_started_total**
   - **Name:** escalation_chains_started_total
   - **Source:** escalation_ledger insert
   - **Surface:** runtime_facts -> GraphQL/MCP/report
2. **escalation_tier_success_rate**
   - **Name:** escalation_tier_success_rate
   - **Source:** agent_executions grouped by escalation_tier_id/outcome
   - **Surface:** GraphQL/report
3. **time_to_success_after_escalation_seconds**
   - **Name:** time_to_success_after_escalation_seconds
   - **Source:** ledger.created_at to first success after tier advancement
   - **Surface:** GraphQL/report histogram
4. **escalation_pause_total**
   - **Name:** escalation_pause_total
   - **Source:** escalation_events.pause_reason_raw
   - **Surface:** GraphQL/MCP/report
5. **false_escalation_rate**
   - **Name:** false_escalation_rate
   - **Source:** escalated success where prior tier would have succeeded after followup adjudication
   - **Surface:** report; gates Phase 4
6. **policy_compile_failure_total**
   - **Name:** policy_compile_failure_total
   - **Source:** runs.preflight diagnostics
   - **Surface:** GraphQL/preflight
7. **shadow_tier_selection_match_rate**
   - **Name:** shadow_tier_selection_match_rate
   - **Source:** shadow decision compared with reviewer/operator adjudication
   - **Surface:** runtime_facts/report; gates Phase 2
8. **provider_session_kill_latency_seconds**
   - **Name:** provider_session_kill_latency_seconds
   - **Source:** force-detach request to terminal commit
   - **Surface:** GraphQL histogram with p95 < 30s and max < 120s SLO
9. **daemon_outage_credit_seconds_total**
   - **Name:** daemon_outage_credit_seconds_total
   - **Source:** ClockProvider outage credit applied to deadlines
   - **Surface:** runtime_facts -> GraphQL
10. **fan_out_blocked_dwell_seconds**
   - **Name:** fan_out_blocked_dwell_seconds
   - **Source:** sibling-block dwell start/end
   - **Surface:** GraphQL histogram
11. **launch_recycle_storm_total**
   - **Name:** launch_recycle_storm_total
   - **Source:** escalation_launch_recycle_storm event
   - **Surface:** GraphQL/report
12. **capacity_probe_failure_total**
   - **Name:** capacity_probe_failure_total
   - **Source:** capacity probe failures/timeouts
   - **Surface:** runtime_facts -> GraphQL
13. **escalation_drift_pending_ack_dwell_seconds**
   - **Name:** escalation_drift_pending_ack_dwell_seconds
   - **Source:** escalation_policy_drift pause opened to external acknowledgement command commit
   - **Surface:** GraphQL/report alert threshold in Phase 1
14. **tier_dwell_share_of_chain**
   - **Name:** tier_dwell_share_of_chain
   - **Source:** agent_execution_runtime_facts tier dwell divided by remaining chain budget
   - **Surface:** GraphQL/report
15. **chain_exhausted_total_by_terminal_tier_kind**
   - **Name:** chain_exhausted_total_by_terminal_tier_kind
   - **Source:** escalation_chain_exhausted grouped by final tier_kind
   - **Surface:** GraphQL/report counter
16. **escalation_repeated_digest_no_progress_total**
   - **Name:** escalation_repeated_digest_no_progress_total
   - **Source:** repeated_digest_no_progress reaching per-tier or chain ceiling, labelled by terminal_tier_kind and trigger_raw
   - **Surface:** GraphQL/report; informs v2 default for abort_after_repeated_digest_tier_count
17. **escalation_commit_contention_total**
   - **Name:** escalation_commit_contention_total
   - **Source:** scheduler transaction SQLITE_BUSY/LOCKED bounded retries
   - **Surface:** runtime_facts -> GraphQL/report
18. **escalation_retry_after_parse_anomaly_total**
   - **Name:** escalation_retry_after_parse_anomaly_total
   - **Source:** Retry-After parse anomaly runtime facts
   - **Surface:** GraphQL/report
19. **escalation_provider_late_frame_after_detach_total**
   - **Name:** escalation_provider_late_frame_after_detach_total
   - **Source:** late provider frames dropped after force-detached commit
   - **Surface:** event journal/report

## Risks

1. **Item**
   - **Mitigation:** DriftReviewSheet is read-only; external MCP/operator workflow owns acknowledgement; SwiftUI has fixture coverage proving no mutation path.
   - **Risk:** macOS write-boundary leak
2. **Item**
   - **Mitigation:** global and per-policy kill-switch, shadow phase, numeric promotion gates, and in-flight toggle fixtures.
   - **Risk:** scheduler regression after Phase 2
3. **Item**
   - **Mitigation:** forward-only migration, projection consumer rollback, populated migration drill, and null-policy compatibility for pre-escalation runs.
   - **Risk:** buggy migration projection
4. **Item**
   - **Mitigation:** server-owned pause_reason/operator_action_hint/runbook_anchor catalog plus client presentation style for labels and raw-code accessibility.
   - **Risk:** pause strings diverge
5. **Item**
   - **Mitigation:** digest_version, typed digest_inputs, redacted evidence refs, and repeated-digest exhaustion metric with terminal labels.
   - **Risk:** opaque repeated blocker triggers
6. **Item**
   - **Mitigation:** force-detach pauses with provider_session_force_detached, no automatic advancement, replay idempotency, and late-frame drop event.
   - **Risk:** provider cancellation advances incorrectly
7. **Item**
   - **Mitigation:** SIGTERM drain through CAS; operator restarts inside drain do not increment storm accounting.
   - **Risk:** planned restart trips storm detection
8. **Item**
   - **Mitigation:** shadow rows use 50% opacity, dashed rule, italic trigger, eye prefix, and never active fill or capsule truth color.
   - **Risk:** operators confuse shadow predictions with executed truth
9. **Item**
   - **Mitigation:** default non-human per-tier wall-clock share prevents starvation unless policy explicitly overrides.
   - **Risk:** one slow tier consumes full chain budget
10. **Item**
   - **Mitigation:** shared EscalationPresentationStyle, named component inventory, density ladder, state matrix, and snapshot fixtures.
   - **Risk:** UI divergence across surfaces

## Review Feedback Resolution

1. **APPLE-BLOCK-001**
   - **Id:** APPLE-BLOCK-001
   - **Resolution:** Removed the governed macOS server mutation. DriftReviewSheet is read-only and routes acknowledgement through MCP/operator workflow. Wire contracts now explicitly forbid SwiftUI policy-drift acknowledgement in v1.
   - **Status:** addressed
2. **APPLE-NB-001**
   - **Id:** APPLE-NB-001
   - **Resolution:** Pinned EscalationReadAdapter ownership: off-MainActor decode/normalization, MainActor publication, shared adapter keyed by run_id, and scene restoration loading state until publisher emits.
   - **Status:** addressed
3. **APPLE-NB-002**
   - **Id:** APPLE-NB-002
   - **Resolution:** Added EscalationPresentationStyle as the shared owner for colors, symbols, labels, truncation, shadow styling, and accessibility raw-code formatting.
   - **Status:** addressed
4. **REL-R13-N1**
   - **Id:** REL-R13-N1
   - **Resolution:** Specified shutdown drain handoff: unsettled provider sessions after 30s are crash-interrupted and force_detach_replay reissues with the same idempotency key.
   - **Status:** addressed
5. **REL-R13-N2**
   - **Id:** REL-R13-N2
   - **Resolution:** Pinned SQLITE_BUSY/LOCKED retry to max 8 attempts, exponential full jitter, total contention wait <= 200ms, non-attempt-consuming.
   - **Status:** addressed
6. **REL-R13-N3**
   - **Id:** REL-R13-N3
   - **Resolution:** Stated recovery-cancelled marker commits in the same scheduler transaction as the cancellation event.
   - **Status:** addressed
7. **REL-R13-N4**
   - **Id:** REL-R13-N4
   - **Resolution:** Added escalation_repeated_digest_no_progress_total labelled by terminal_tier_kind and trigger_raw.
   - **Status:** addressed
8. **REL-R13-N5**
   - **Id:** REL-R13-N5
   - **Resolution:** Pinned human-tier outage credit as deadline credit capped at 3600s regardless of notification delivery during outage.
   - **Status:** addressed
9. **MAC-R13-N01**
   - **Id:** MAC-R13-N01
   - **Resolution:** Pinned SF Symbols resolution against macOS 26.2 / Xcode 26.3 with missing-symbol fixture.
   - **Status:** addressed
10. **MAC-R13-N02**
   - **Id:** MAC-R13-N02
   - **Resolution:** Specified NSApp.requestUserAttention(.informationalRequest) and cancellation token lifecycle.
   - **Status:** addressed
11. **MAC-R13-N03**
   - **Id:** MAC-R13-N03
   - **Resolution:** Pinned DriftReviewSheet dismissal policy: no interactive acknowledgement, Esc cancels only, parent-window close requires explicit dismissal.
   - **Status:** addressed
12. **MAC-R13-N04**
   - **Id:** MAC-R13-N04
   - **Resolution:** Dock badge derives from live EscalationReadAdapter aggregation across paused/escalation states and recomputes on every snapshot.
   - **Status:** addressed
13. **MAC-R13-N05**
   - **Id:** MAC-R13-N05
   - **Resolution:** MenuBarExtra compact item renders aggregate count plus state pill only, with tier/trigger in tooltip or menu rows.
   - **Status:** addressed
14. **MAC-R13-N06**
   - **Id:** MAC-R13-N06
   - **Resolution:** Pause card now has ideal/min/below-min behavior and forbids horizontal scroll.
   - **Status:** addressed
15. **MAC-R13-N07**
   - **Id:** MAC-R13-N07
   - **Resolution:** Added Full Keyboard Access tab order and activation behavior for banners, capsule menu, lineage disclosures, pause actions, trace, and drift controls.
   - **Status:** addressed
16. **MAC-R13-N08**
   - **Id:** MAC-R13-N08
   - **Resolution:** Pinned atomic NSPasteboard write for .string and public.json from byte-identical redacted trace.
   - **Status:** addressed
17. **UI-R13-001**
   - **Id:** UI-R13-001
   - **Resolution:** Added read_pipeline_states matrix for subscription, decode, transport disconnect, stale snapshot, and ready states.
   - **Status:** addressed
18. **UI-R13-002**
   - **Id:** UI-R13-002
   - **Resolution:** Defined EscalationCommandPresentation slots, truncation, disabled reason, and mirror rule.
   - **Status:** addressed
19. **UI-R13-003**
   - **Id:** UI-R13-003
   - **Resolution:** Defined MenuBarExtra aggregate count, per-run rows, sort order, overflow, empty state, and badge format.
   - **Status:** addressed
20. **UI-R13-004**
   - **Id:** UI-R13-004
   - **Resolution:** Covered pause-card narrow-width behavior under MAC-R13-N06.
   - **Status:** addressed
21. **UI-R13-005**
   - **Id:** UI-R13-005
   - **Resolution:** Pinned lineage column width policy and below-480pt card-row fallback.
   - **Status:** addressed
22. **UI-R13-006**
   - **Id:** UI-R13-006
   - **Resolution:** Defined Compact banner cooccurrence count chip and tooltip listing suppressed banners.
   - **Status:** addressed
23. **UI-R13-007**
   - **Id:** UI-R13-007
   - **Resolution:** Pinned shadow-row opacity, dashed rule, italic trigger, eye prefix, and inactive fill weight.
   - **Status:** addressed
24. **UI-R13-008**
   - **Id:** UI-R13-008
   - **Resolution:** Clarified same_backend_retry baseline, active, and exhausted color roles.
   - **Status:** addressed
25. **UI-R13-009**
   - **Id:** UI-R13-009
   - **Resolution:** Pinned RunDetailHeader vertical order and 8pt spacing.
   - **Status:** addressed
26. **UI-R13-010**
   - **Id:** UI-R13-010
   - **Resolution:** Placed EscalationTraceTimeline as disclosure-collapsed sibling section under lineage.
   - **Status:** addressed
27. **UI-R13-011**
   - **Id:** UI-R13-011
   - **Resolution:** Added co-occurrence resolution for capsule state and primary action.
   - **Status:** addressed
28. **UI-R13-012**
   - **Id:** UI-R13-012
   - **Resolution:** Defined drift policy summary as structured diff and added empty-diff state.
   - **Status:** addressed

## Open Questions

1. **Item**
   - **Default:** defer; v1 reserves and rejects vocabulary only
   - **Question:** Should reserved operator_forced_escalation become an authorized command_journal-backed MCP/GraphQL mutation later?
2. **Item**
   - **Default:** keep 120s hard ceiling until provider_session_kill_latency_seconds proves stable
   - **Question:** Should provider force-detach hard ceiling become policy-configurable?
3. **Item**
   - **Default:** keep observability-only in v1; max_chain_wall_clock_seconds is the hard bound
   - **Question:** Should fan_out_blocked_dwell_seconds become a hard pause condition after soak?
4. **Item**
   - **Default:** client owns labels in v1 while raw strings remain authoritative; revisit if other clients need identical wording
   - **Question:** Should server own humanized labels instead of the macOS frozen label catalog?
5. **Item**
   - **Default:** defer; requires explicit update to docs/reference/ui-action-boundary.md, mutation authorization, command journal fields, audit model, failure modes, and fixtures
   - **Question:** Should policy-drift acknowledgement become a governed macOS action in a later proposal?

## Migration Evidence Plan

- **Drill:** populated SQLite fixture with mixed pre/post-escalation runs, forward migration, projection rebuild, MCP/GraphQL/report parity assertions, no row-count drift on agent_executions, escalation_ledger empty for pre-escalation runs, redaction_version stamped
- **Phase 3 Artifact Drill:** release evidence keeps fixtures proving recovery does not silently discard settled mediation_response or human_decision artifacts before broad GA
- **Phase 3 Shutdown Drill:** implementation tests prove startup force-detach replay for a running escalation execution, no InvokeAgent relaunch, paused ledger, runtime facts, failed stage, and blocked run; release evidence may add a live SIGTERM soak showing force_detach_replay and late_frame_after_detach metrics under operator restart conditions
- **Resume:** pre-escalation snapshots read null policy_id as inactive ledger; later escalation_policies edits produce escalation_policy_drift requiring external operator acknowledgement
- **Rollback:** data-preserving rollback disables behavior via kill-switch, drops projection consumers, and leaves committed columns/tables intact
- **Stance:** forward-only; no destructive down-migration

## Pause Reason Catalog

1. **Item**
   - **Code:** escalation_policy_unknown_backend_profile
   - **Operator Action Hint:** Define the missing backend_profile or remove the tier.
   - **Phase:** compile
   - **Runbook Anchor:** escalation/policy-unknown-backend-profile
2. **Item**
   - **Code:** escalation_policy_ambiguous_at_compile
   - **Operator Action Hint:** Resolve the policy binding ambiguity or set explicit precedence.
   - **Phase:** compile
   - **Runbook Anchor:** escalation/policy-ambiguous
3. **Item**
   - **Code:** escalation_policy_unsafe_for_side_effect_stage
   - **Operator Action Hint:** Remove the policy from the side-effect stage or split the stage.
   - **Phase:** compile
   - **Runbook Anchor:** escalation/policy-unsafe-side-effect
4. **Item**
   - **Code:** escalation_policy_disabled
   - **Operator Action Hint:** Re-enable the policy through runtime override or catalog edit plus external drift acknowledgement.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/policy-disabled
5. **Item**
   - **Code:** escalation_kill_switch_engaged
   - **Operator Action Hint:** Disable CHAINWORKS_ESCALATION_FORCE_PRIMARY to resume normal tier advancement.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/kill-switch-engaged
6. **Item**
   - **Code:** escalation_chain_exhausted
   - **Operator Action Hint:** Extend the chain or accept terminal pause.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/chain-exhausted
7. **Item**
   - **Code:** capacity_probe_failed
   - **Operator Action Hint:** Inspect provider transport health and retry once cleared.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/capacity-probe-failed
8. **Item**
   - **Code:** provider_session_force_detached
   - **Operator Action Hint:** Inspect provider state and resume manually; tier does not auto-advance.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/provider-session-force-detached
9. **Item**
   - **Code:** escalation_recovery_inconsistent
   - **Operator Action Hint:** Use v1 cancellation unstick; marker records originating trigger.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/recovery-inconsistent
10. **Item**
   - **Code:** escalation_repeated_digest_no_progress
   - **Operator Action Hint:** Inspect digest evidence and adjust policy or pause.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/repeated-digest-no-progress
11. **Item**
   - **Code:** escalation_deadline_elapsed
   - **Operator Action Hint:** Extend wall-clock budget or accept pause.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/deadline-elapsed
12. **Item**
   - **Code:** human_tier_deadline_elapsed
   - **Operator Action Hint:** Resume with operator decision or extend human tier deadline.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/human-tier-deadline-elapsed
13. **Item**
   - **Code:** escalation_policy_drift
   - **Operator Action Hint:** Acknowledge escalation policy drift through the external MCP/operator workflow or restart with the new policy.
   - **Phase:** runtime
   - **Runbook Anchor:** escalation/policy-drift

## Wire Contracts

### Events
- escalation.tier_selected
- escalation.tier_rejected
- escalation.tier_advanced
- escalation.chain_exhausted
- escalation.deadline_elapsed
- escalation.ledger.status_changed
- escalation.recovery_inconsistent.cancelled
- escalation.kill_switch.force_primary_engaged
- escalation.kill_switch.policy_disabled
- escalation.shadow.tier_selected
- escalation.operator_forced.reserved_rejected
- escalation.compile_failed
- escalation.provider_late_frame_after_detach
- escalation.retry_after.parse_anomaly
- escalation.commit_contention.retry
- escalation.drift.pending_ack_dwell_threshold
### Graphql
- raw status, trigger, tier, tier_kind, pause_reason, and event fields are authoritative strings
- nullable known helpers are compatibility sugar only
- featureFlagState, wouldSelectTierId, wouldSelectTriggerRaw, digestVersion, digestInputs, redactionVersion, waitingRetryAfterUntil, traceUnavailableReasonRaw, escalationTraceJsonRedacted, policyDriftState, and externalAcknowledgementRef appear in SDL snapshots
- future unknown trigger, pause_reason, tier_kind, and event strings must round-trip through Swift DTOs, MCP, reports, and snapshots
### Macos Write Boundary
- Governed SwiftUI reads GraphQL/subscription DTOs only for escalation state.
- EscalationReadAdapter may request readback refreshes, copy redacted traces to pasteboard, open https runbook URLs, present notifications, and request AppKit attention.
- EscalationReadAdapter may not call policy-drift acknowledgement, tier mutation, retry, resume, cancel, or force-primary mutations.
- DriftAcknowledgementSheet is renamed in implementation intent to DriftReviewSheet if needed; it is read-only, with Copy acknowledgement command details and Open external workflow affordances. UI updates only after GraphQL readback reports drift cleared.
### Mcp
- runs.get and run://{run_id} expose the same frozen escalation fields as GraphQL
- runs.preflight surfaces compile-time policy failures with pause_reason codes, operator_action_hint, and runbook_anchor
- MCP readback never reconstructs truth from current YAML, logs, or mutable registries
- policy drift acknowledgement for v1 is an external MCP/operator command path with command journal, idempotency key, principal, frozen policy_hash, current policy_hash, decision, created_at, and result; it is not called by governed macOS
### Reports
- active reports share resource id
- terminal v1 reports remain immutable
- successor reports mark superseded_by
- redaction_version stamps each escalation projection and report write
### Yaml Policy Example
- **Applies To**
  - **Agent Id:** code_writer
- **Enabled Default:** `false`
- **Max Chain Attempts:** `6`
- **Max Chain Wall Clock Seconds:** `7200`
- **Policy Id:** code_writer_default_escalation
- **Schema Version:** escalation_policy_v1
- **Tiers**
  1. **Item**
     - **Kind:** same_backend_retry
     - **Max Attempts:** `2`
     - **Tier Id:** primary_retry
  2. **Item**
     - **Backend Profile Id:** claude_builder_frontier
     - **Kind:** backend_profile
     - **Max Attempts:** `1`
     - **Tier Id:** frontier_profile
  3. **Item**
     - **Backend Profile Id:** codex_implementer_high
     - **Kind:** backend_profile
     - **Max Attempts:** `1`
     - **Tier Id:** codex_profile
  4. **Item**
     - **Kind:** lead_mediation
     - **Max Attempts:** `1`
     - **Tier Id:** lead_review
  5. **Item**
     - **Kind:** pause
     - **Tier Id:** human_pause
- **Triggers**
  - repeated_same_blocker_digest
  - contract_output_failure
  - stale_no_output
  - provider_quota_exhausted
  - transport_failure
  - loop_budget_threshold
