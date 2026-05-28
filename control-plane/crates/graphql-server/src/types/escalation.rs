use async_graphql::*;

use db::repos::escalation as escalation_repo;
use domain::ids::RunId;

/// P058 SEC-003: Row caps for GraphQL escalation readback.
/// Must match MCP caps in mcp-server/src/tools/runs.rs to maintain parity.
const ESCALATION_MAX_LEDGERS: usize = 50;
const ESCALATION_MAX_EVENTS_PER_LEDGER: usize = 200;
const ESCALATION_MAX_EXEC_METAS_PER_LEDGER: usize = 100;

/// Read-only escalation chain state for one ledger record.
/// All enum-like fields are authoritative raw strings for forward compatibility.
/// Unknown future trigger/tier_kind/pause_reason/event values must round-trip unchanged.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "EscalationChainState", rename_fields = "camelCase")]
pub struct GqlEscalationChainState {
    pub id: ID,
    pub run_id: ID,
    pub stage_id: String,
    pub agent_id: String,
    pub policy_id: String,
    pub policy_hash: String,
    /// Raw status: active | paused | exhausted | cancelled
    pub status_raw: String,
    pub current_tier_id: Option<String>,
    /// Raw tier kind: same_backend_retry | backend_profile | lead_mediation | pause
    pub current_tier_kind_raw: Option<String>,
    pub chain_attempt_index: i64,
    /// Raw trigger vocabulary — forward-compatible, may contain future values.
    pub trigger_raw: Option<String>,
    /// Raw pause reason — see pause_reason_catalog in proposal.
    pub pause_reason_raw: Option<String>,
    /// Human-readable operator action hint (server-owned, stable).
    pub operator_action_hint: Option<String>,
    /// Runbook anchor slug for docs/runbooks lookup.
    pub runbook_anchor: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    /// P058 Phase 2+: ISO8601 timestamp until which the chain is waiting on a Retry-After hint.
    /// Null until Phase 2 scheduler sets it.
    pub waiting_retry_after_until: Option<String>,
    /// P058 Phase 2+: Why the trace is unavailable, if applicable.
    pub trace_unavailable_reason_raw: Option<String>,
    /// P058 Phase 2+: Redacted escalation trace JSON (stripped of raw evidence).
    /// Requires redaction_version stamp; null until Phase 2 scheduler populates.
    pub escalation_trace_json_redacted: Option<String>,
    /// P058 Phase 2+: Policy drift state for this chain (e.g. "pending_ack", "acknowledged").
    pub policy_drift_state: Option<String>,
    /// P058 Phase 2+: External acknowledgement command reference for policy drift.
    pub external_acknowledgement_ref: Option<String>,
    /// P058 Phase 2+: Feature flag / kill-switch state frozen on the ledger at scheduling time.
    /// Example values: "kill_switch_engaged", "policy_disabled", "in_flight_continue".
    pub feature_flag_state: Option<String>,
    /// Ordered event journal for this chain (capped at ESCALATION_MAX_EVENTS_PER_LEDGER).
    pub events: Vec<GqlEscalationEventEntry>,
    /// True when event count exceeds the cap; clients must paginate for full history.
    pub events_truncated: bool,
    /// Total event count before cap was applied.
    pub events_total: i64,
    /// Per-execution attribution rows for this chain (capped at ESCALATION_MAX_EXEC_METAS_PER_LEDGER).
    pub execution_metas: Vec<GqlEscalationExecutionMeta>,
    /// True when execution_metas count exceeds the cap.
    pub execution_metas_truncated: bool,
    /// Total execution_metas count before cap was applied.
    pub execution_metas_total: i64,
}

/// Per-execution escalation attribution.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "EscalationExecutionMeta", rename_fields = "camelCase")]
pub struct GqlEscalationExecutionMeta {
    pub agent_execution_id: ID,
    pub escalation_ledger_id: ID,
    pub tier_id: String,
    pub tier_kind_raw: String,
    pub tier_attempt_index: i64,
    pub trigger_raw: Option<String>,
    pub digest_version: Option<String>,
    pub capacity_probe_counter: i64,
    pub created_at: String,
    pub updated_at: String,
    /// P058 Phase 1b+: Shadow tier selection — which tier would have been selected.
    /// Populated by Phase 1b shadow writer; null in Phase 0/1.
    pub would_select_tier_id: Option<String>,
    /// P058 Phase 1b+: Shadow trigger classification — which trigger would have fired.
    pub would_select_trigger_raw: Option<String>,
    /// P058 Phase 1b+: Shadow decision JSON — full decision context; redacted before write.
    pub would_select_decision_json: Option<String>,
    /// P058 Phase 2+: Digest inputs for the escalation_blocker_digest_v1 for this attempt.
    /// JSON object with typed fields: failure_kind, output_settlement_state, etc.
    pub digest_inputs: Option<String>,
}

/// Escalation event journal entry.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "EscalationEventEntry", rename_fields = "camelCase")]
pub struct GqlEscalationEventEntry {
    pub id: ID,
    pub escalation_ledger_id: ID,
    /// Raw event kind — forward-compatible. See wire_contracts.events in proposal.
    pub event_kind_raw: String,
    pub tier_id: Option<String>,
    pub tier_kind_raw: Option<String>,
    pub trigger_raw: Option<String>,
    pub pause_reason_raw: Option<String>,
    /// Redacted event payload. Writers must strip all raw evidence before populating;
    /// only digest refs and tier metadata are permitted. Phase 2+ callers must hold
    /// a valid redaction_version stamp before writing this field.
    pub payload_json: Option<String>,
    /// Redaction version stamp. Required per proposal on every event write.
    pub redaction_version: Option<String>,
    pub created_at: String,
}

/// Aggregate escalation readback for a run.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "EscalationRunReadback", rename_fields = "camelCase")]
pub struct GqlEscalationRunReadback {
    pub run_id: ID,
    /// Chains capped at ESCALATION_MAX_LEDGERS; inspect chains_truncated for overflow.
    pub chains: Vec<GqlEscalationChainState>,
    /// True when chain count exceeds the cap; clients must paginate for full history.
    pub chains_truncated: bool,
    /// Total chain count before cap was applied.
    pub chains_total: i64,
    pub paused_chain_count: i64,
    pub has_active_escalation: bool,
    /// Dominant pause_reason_raw from first paused chain, if any.
    pub dominant_pause_reason_raw: Option<String>,
}

pub async fn run_escalation_readback(
    pool: &sqlx::SqlitePool,
    run_id: RunId,
) -> async_graphql::Result<GqlEscalationRunReadback> {
    // Fetch the capped page of ledgers plus aggregate totals from separate COUNT queries
    // so that chains_total / paused_chain_count are accurate even when >ESCALATION_MAX_LEDGERS
    // rows exist (SEC-P058-004, SEC-P058-005).
    let all_ledgers = escalation_repo::find_ledgers_by_run(pool, run_id)
        .await
        .map_err(|e| async_graphql::Error::new(format!("escalation readback failed: {e}")))?;

    let chains_total = escalation_repo::count_ledgers_by_run(pool, run_id)
        .await
        .map_err(|e| async_graphql::Error::new(format!("escalation chain count failed: {e}")))?;

    let paused_chain_count = escalation_repo::count_paused_ledgers_by_run(pool, run_id)
        .await
        .map_err(|e| {
            async_graphql::Error::new(format!("escalation paused chain count failed: {e}"))
        })?;

    // BLOCK-2: has_active_escalation must reflect actual escalation activity, not just whether
    // a policy was configured. A ledger inserted at claim time (trigger_raw NULL, status 'active')
    // does not indicate active escalation — it means the first attempt has started with a policy
    // configured. has_active_escalation is true only when a failure trigger has fired (trigger_raw
    // IS NOT NULL) or the chain has advanced beyond its initial state (status != 'active').
    let triggered_chain_count = escalation_repo::count_triggered_ledgers_by_run(pool, run_id)
        .await
        .map_err(|e| {
            async_graphql::Error::new(format!("escalation triggered chain count failed: {e}"))
        })?;

    let dominant_pause_reason = escalation_repo::dominant_pause_reason_for_run(pool, run_id)
        .await
        .map_err(|e| {
            async_graphql::Error::new(format!("escalation dominant pause reason failed: {e}"))
        })?;

    let ledger_page_len = all_ledgers.len();
    let chains_truncated =
        ledger_page_len >= ESCALATION_MAX_LEDGERS && chains_total > ESCALATION_MAX_LEDGERS as i64;
    let ledgers = &all_ledgers[..ledger_page_len.min(ESCALATION_MAX_LEDGERS)];
    let has_active = triggered_chain_count > 0;

    let mut chains: Vec<GqlEscalationChainState> = Vec::with_capacity(ledgers.len());

    for l in ledgers {
        let all_events = escalation_repo::find_events_by_ledger(pool, &l.id)
            .await
            .map_err(|e| {
                async_graphql::Error::new(format!(
                    "escalation events readback failed for ledger {}: {e}",
                    l.id
                ))
            })?;
        let all_exec_metas = escalation_repo::find_execution_metadata_by_ledger(pool, &l.id)
            .await
            .map_err(|e| {
                async_graphql::Error::new(format!(
                    "escalation execution metadata readback failed for ledger {}: {e}",
                    l.id
                ))
            })?;

        // Use separate COUNT queries so totals are accurate even when fetch is capped.
        let event_total = escalation_repo::count_events_by_ledger(pool, &l.id)
            .await
            .map_err(|e| {
                async_graphql::Error::new(format!(
                    "escalation event count failed for ledger {}: {e}",
                    l.id
                ))
            })?;
        let meta_total = escalation_repo::count_metas_by_ledger(pool, &l.id)
            .await
            .map_err(|e| {
                async_graphql::Error::new(format!(
                    "escalation meta count failed for ledger {}: {e}",
                    l.id
                ))
            })?;
        let events_truncated = all_events.len() >= ESCALATION_MAX_EVENTS_PER_LEDGER
            && event_total > ESCALATION_MAX_EVENTS_PER_LEDGER as i64;
        let metas_truncated = all_exec_metas.len() >= ESCALATION_MAX_EXEC_METAS_PER_LEDGER
            && meta_total > ESCALATION_MAX_EXEC_METAS_PER_LEDGER as i64;
        let runtime_readback =
            escalation_repo::runtime_readback_from_events(l, &all_events, events_truncated);

        let gql_events: Vec<GqlEscalationEventEntry> = all_events
            .iter()
            .take(ESCALATION_MAX_EVENTS_PER_LEDGER)
            .map(|ev| GqlEscalationEventEntry {
                id: ID(ev.id.clone()),
                escalation_ledger_id: ID(ev.escalation_ledger_id.clone()),
                event_kind_raw: ev.event_kind_raw.clone(),
                tier_id: ev.tier_id.clone(),
                tier_kind_raw: ev.tier_kind_raw.clone(),
                trigger_raw: ev.trigger_raw.clone(),
                pause_reason_raw: ev.pause_reason_raw.clone(),
                payload_json: ev.payload_json.clone(),
                redaction_version: ev.redaction_version.clone(),
                created_at: ev.created_at.to_rfc3339(),
            })
            .collect();

        let gql_metas: Vec<GqlEscalationExecutionMeta> = all_exec_metas
            .iter()
            .take(ESCALATION_MAX_EXEC_METAS_PER_LEDGER)
            .map(|m| GqlEscalationExecutionMeta {
                agent_execution_id: ID(m.agent_execution_id.to_string()),
                escalation_ledger_id: ID(m.escalation_ledger_id.clone()),
                tier_id: m.tier_id.clone(),
                tier_kind_raw: m.tier_kind_raw.clone(),
                tier_attempt_index: m.tier_attempt_index,
                trigger_raw: m.trigger_raw.clone(),
                digest_version: m.digest_version.clone(),
                capacity_probe_counter: m.capacity_probe_counter,
                created_at: m.created_at.to_rfc3339(),
                updated_at: m.updated_at.to_rfc3339(),
                // Phase 1b+: shadow columns read from agent_execution_runtime_facts via LEFT JOIN.
                would_select_tier_id: m.would_select_tier_id.clone(),
                would_select_trigger_raw: m.would_select_trigger_raw.clone(),
                would_select_decision_json: m.would_select_decision_json.clone(),
                digest_inputs: escalation_repo::digest_inputs_for_meta_from_events(
                    &all_events,
                    &m.tier_id,
                    m.trigger_raw.as_deref(),
                ),
            })
            .collect();

        chains.push(GqlEscalationChainState {
            id: ID(l.id.clone()),
            run_id: ID(l.run_id.to_string()),
            stage_id: l.stage_id.clone(),
            agent_id: l.agent_id.clone(),
            policy_id: l.policy_id.clone(),
            policy_hash: l.policy_hash.clone(),
            status_raw: l.status_raw.clone(),
            current_tier_id: l.current_tier_id.clone(),
            current_tier_kind_raw: l.current_tier_kind_raw.clone(),
            chain_attempt_index: l.chain_attempt_index,
            trigger_raw: l.trigger_raw.clone(),
            pause_reason_raw: l.pause_reason_raw.clone(),
            operator_action_hint: l.operator_action_hint.clone(),
            runbook_anchor: l.runbook_anchor.clone(),
            created_at: l.created_at.to_rfc3339(),
            updated_at: l.updated_at.to_rfc3339(),
            waiting_retry_after_until: runtime_readback.waiting_retry_after_until,
            trace_unavailable_reason_raw: runtime_readback.trace_unavailable_reason_raw,
            escalation_trace_json_redacted: runtime_readback.escalation_trace_json_redacted,
            policy_drift_state: runtime_readback.policy_drift_state,
            external_acknowledgement_ref: runtime_readback.external_acknowledgement_ref,
            feature_flag_state: runtime_readback.feature_flag_state,
            events: gql_events,
            events_truncated,
            events_total: event_total,
            execution_metas: gql_metas,
            execution_metas_truncated: metas_truncated,
            execution_metas_total: meta_total,
        });
    }

    Ok(GqlEscalationRunReadback {
        run_id: ID(run_id.to_string()),
        chains,
        chains_truncated,
        chains_total,
        has_active_escalation: has_active,
        paused_chain_count,
        dominant_pause_reason_raw: dominant_pause_reason,
    })
}
