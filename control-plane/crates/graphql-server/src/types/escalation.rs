use async_graphql::*;

use db::repos::escalation as escalation_repo;
use domain::ids::RunId;

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
    pub payload_json: Option<String>,
    pub created_at: String,
}

/// Aggregate escalation readback for a run.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "EscalationRunReadback", rename_fields = "camelCase")]
pub struct GqlEscalationRunReadback {
    pub run_id: ID,
    pub chains: Vec<GqlEscalationChainState>,
    pub paused_chain_count: i64,
    pub has_active_escalation: bool,
    /// Dominant pause_reason_raw from first paused chain, if any.
    pub dominant_pause_reason_raw: Option<String>,
}

pub async fn run_escalation_readback(
    pool: &sqlx::SqlitePool,
    run_id: RunId,
) -> async_graphql::Result<GqlEscalationRunReadback> {
    let ledgers = escalation_repo::find_ledgers_by_run(pool, run_id)
        .await
        .map_err(|e| async_graphql::Error::new(format!("escalation readback failed: {e}")))?;

    let chains: Vec<GqlEscalationChainState> = ledgers
        .iter()
        .map(|l| GqlEscalationChainState {
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
        })
        .collect();

    let paused = chains
        .iter()
        .filter(|c| c.status_raw == "paused" || c.status_raw == "exhausted")
        .count() as i64;

    let dominant_pause_reason = chains
        .iter()
        .find(|c| c.status_raw == "paused" || c.status_raw == "exhausted")
        .and_then(|c| c.pause_reason_raw.clone());

    Ok(GqlEscalationRunReadback {
        run_id: ID(run_id.to_string()),
        has_active_escalation: !chains.is_empty(),
        paused_chain_count: paused,
        dominant_pause_reason_raw: dominant_pause_reason,
        chains,
    })
}
