use anyhow::{anyhow, bail, Result};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::escalation::{EscalationEvent, EscalationExecutionMetadata, EscalationLedger};
use domain::ids::RunId;

/// Validate that `value` is well-formed JSON if present.
/// The proposal requires repository-layer JSON rejection even without sqlite json1.
fn validate_json_field(field_name: &str, value: &Option<String>) -> Result<()> {
    if let Some(json_str) = value {
        serde_json::from_str::<serde_json::Value>(json_str)
            .map_err(|e| anyhow!("field {field_name} contains malformed JSON: {e}"))?;
    }
    Ok(())
}

pub async fn insert_ledger(pool: &SqlitePool, ledger: &EscalationLedger) -> Result<()> {
    let mut tx = pool.begin().await?;
    insert_ledger_tx(&mut tx, ledger).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn insert_ledger_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger: &EscalationLedger,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO escalation_ledger
           (id, run_id, stage_id, agent_id, policy_id, policy_hash,
            status_raw, current_tier_id, current_tier_kind_raw,
            chain_attempt_index, trigger_raw, pause_reason_raw,
            operator_action_hint, runbook_anchor, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14, ?15, ?16)"#,
    )
    .bind(&ledger.id)
    .bind(ledger.run_id.to_string())
    .bind(&ledger.stage_id)
    .bind(&ledger.agent_id)
    .bind(&ledger.policy_id)
    .bind(&ledger.policy_hash)
    .bind(&ledger.status_raw)
    .bind(&ledger.current_tier_id)
    .bind(&ledger.current_tier_kind_raw)
    .bind(ledger.chain_attempt_index)
    .bind(&ledger.trigger_raw)
    .bind(&ledger.pause_reason_raw)
    .bind(&ledger.operator_action_hint)
    .bind(&ledger.runbook_anchor)
    .bind(ledger.created_at.to_rfc3339())
    .bind(ledger.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn update_ledger_tx(
    tx: &mut Transaction<'_, Sqlite>,
    ledger: &EscalationLedger,
) -> Result<()> {
    sqlx::query(
        r#"UPDATE escalation_ledger SET
           status_raw = ?1,
           current_tier_id = ?2,
           current_tier_kind_raw = ?3,
           chain_attempt_index = ?4,
           trigger_raw = ?5,
           pause_reason_raw = ?6,
           operator_action_hint = ?7,
           runbook_anchor = ?8,
           updated_at = ?9
           WHERE id = ?10"#,
    )
    .bind(&ledger.status_raw)
    .bind(&ledger.current_tier_id)
    .bind(&ledger.current_tier_kind_raw)
    .bind(ledger.chain_attempt_index)
    .bind(&ledger.trigger_raw)
    .bind(&ledger.pause_reason_raw)
    .bind(&ledger.operator_action_hint)
    .bind(&ledger.runbook_anchor)
    .bind(ledger.updated_at.to_rfc3339())
    .bind(&ledger.id)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_ledgers_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<EscalationLedger>> {
    let rows = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, policy_id, policy_hash,
                  status_raw, current_tier_id, current_tier_kind_raw,
                  chain_attempt_index, trigger_raw, pause_reason_raw,
                  operator_action_hint, runbook_anchor, created_at, updated_at
           FROM escalation_ledger
           WHERE run_id = ?
           ORDER BY created_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let run_id_str: String = row.try_get("run_id")?;
            let created_at_str: String = row.try_get("created_at")?;
            let updated_at_str: String = row.try_get("updated_at")?;
            Ok(EscalationLedger {
                id: row.try_get("id")?,
                run_id: run_id_str
                    .parse()
                    .map_err(|e| anyhow!("bad run_id: {e}"))?,
                stage_id: row.try_get("stage_id")?,
                agent_id: row.try_get("agent_id")?,
                policy_id: row.try_get("policy_id")?,
                policy_hash: row.try_get("policy_hash")?,
                status_raw: row.try_get("status_raw")?,
                current_tier_id: row.try_get("current_tier_id")?,
                current_tier_kind_raw: row.try_get("current_tier_kind_raw")?,
                chain_attempt_index: row.try_get("chain_attempt_index")?,
                trigger_raw: row.try_get("trigger_raw")?,
                pause_reason_raw: row.try_get("pause_reason_raw")?,
                operator_action_hint: row.try_get("operator_action_hint")?,
                runbook_anchor: row.try_get("runbook_anchor")?,
                created_at: created_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad created_at: {e}"))?,
                updated_at: updated_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad updated_at: {e}"))?,
            })
        })
        .collect()
}

pub async fn find_ledger_by_id(
    pool: &SqlitePool,
    ledger_id: &str,
) -> Result<Option<EscalationLedger>> {
    let row = sqlx::query(
        r#"SELECT id, run_id, stage_id, agent_id, policy_id, policy_hash,
                  status_raw, current_tier_id, current_tier_kind_raw,
                  chain_attempt_index, trigger_raw, pause_reason_raw,
                  operator_action_hint, runbook_anchor, created_at, updated_at
           FROM escalation_ledger
           WHERE id = ?"#,
    )
    .bind(ledger_id)
    .fetch_optional(pool)
    .await?;

    row.map(|row| {
        let run_id_str: String = row.try_get("run_id")?;
        let created_at_str: String = row.try_get("created_at")?;
        let updated_at_str: String = row.try_get("updated_at")?;
        Ok(EscalationLedger {
            id: row.try_get("id")?,
            run_id: run_id_str
                .parse()
                .map_err(|e| anyhow!("bad run_id: {e}"))?,
            stage_id: row.try_get("stage_id")?,
            agent_id: row.try_get("agent_id")?,
            policy_id: row.try_get("policy_id")?,
            policy_hash: row.try_get("policy_hash")?,
            status_raw: row.try_get("status_raw")?,
            current_tier_id: row.try_get("current_tier_id")?,
            current_tier_kind_raw: row.try_get("current_tier_kind_raw")?,
            chain_attempt_index: row.try_get("chain_attempt_index")?,
            trigger_raw: row.try_get("trigger_raw")?,
            pause_reason_raw: row.try_get("pause_reason_raw")?,
            operator_action_hint: row.try_get("operator_action_hint")?,
            runbook_anchor: row.try_get("runbook_anchor")?,
            created_at: created_at_str
                .parse()
                .map_err(|e| anyhow!("bad created_at: {e}"))?,
            updated_at: updated_at_str
                .parse()
                .map_err(|e| anyhow!("bad updated_at: {e}"))?,
        })
    })
    .transpose()
}

pub async fn insert_execution_metadata_tx(
    tx: &mut Transaction<'_, Sqlite>,
    meta: &EscalationExecutionMetadata,
) -> Result<()> {
    sqlx::query(
        r#"INSERT INTO escalation_execution_metadata
           (agent_execution_id, escalation_ledger_id, tier_id, tier_kind_raw,
            tier_attempt_index, trigger_raw, digest_version, capacity_probe_counter,
            created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(meta.agent_execution_id.to_string())
    .bind(&meta.escalation_ledger_id)
    .bind(&meta.tier_id)
    .bind(&meta.tier_kind_raw)
    .bind(meta.tier_attempt_index)
    .bind(&meta.trigger_raw)
    .bind(&meta.digest_version)
    .bind(meta.capacity_probe_counter)
    .bind(meta.created_at.to_rfc3339())
    .bind(meta.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_execution_metadata_by_ledger(
    pool: &SqlitePool,
    escalation_ledger_id: &str,
) -> Result<Vec<EscalationExecutionMetadata>> {
    use domain::ids::AgentExecutionId;
    let rows = sqlx::query(
        r#"SELECT agent_execution_id, escalation_ledger_id, tier_id, tier_kind_raw,
                  tier_attempt_index, trigger_raw, digest_version, capacity_probe_counter,
                  created_at, updated_at
           FROM escalation_execution_metadata
           WHERE escalation_ledger_id = ?
           ORDER BY created_at ASC"#,
    )
    .bind(escalation_ledger_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let exec_id_str: String = row.try_get("agent_execution_id")?;
            let created_at_str: String = row.try_get("created_at")?;
            let updated_at_str: String = row.try_get("updated_at")?;
            Ok(EscalationExecutionMetadata {
                agent_execution_id: exec_id_str
                    .parse::<AgentExecutionId>()
                    .map_err(|e| anyhow!("bad agent_execution_id: {e}"))?,
                escalation_ledger_id: row.try_get("escalation_ledger_id")?,
                tier_id: row.try_get("tier_id")?,
                tier_kind_raw: row.try_get("tier_kind_raw")?,
                tier_attempt_index: row.try_get("tier_attempt_index")?,
                trigger_raw: row.try_get("trigger_raw")?,
                digest_version: row.try_get("digest_version")?,
                capacity_probe_counter: row.try_get("capacity_probe_counter")?,
                created_at: created_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad created_at: {e}"))?,
                updated_at: updated_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad updated_at: {e}"))?,
            })
        })
        .collect()
}

pub async fn insert_event_tx(
    tx: &mut Transaction<'_, Sqlite>,
    event: &EscalationEvent,
) -> Result<()> {
    // Reject malformed JSON before writing — required by proposal even without sqlite json1.
    validate_json_field("payload_json", &event.payload_json)?;
    // Reject missing or unrecognized redaction_version — proposal mandates a known stamp on every event write.
    // Allowlist prevents arbitrary strings from satisfying the not-null contract.
    const KNOWN_REDACTION_VERSIONS: &[&str] = &["redaction_v1"];
    match event.redaction_version.as_deref() {
        None => bail!("escalation_events.redaction_version is required; caller must supply a redaction stamp"),
        Some(v) if !KNOWN_REDACTION_VERSIONS.contains(&v) => bail!(
            "escalation_events.redaction_version '{}' is not in the known allowlist {:?}",
            v, KNOWN_REDACTION_VERSIONS
        ),
        _ => {}
    }

    sqlx::query(
        r#"INSERT INTO escalation_events
           (id, escalation_ledger_id, event_kind_raw, tier_id, tier_kind_raw,
            trigger_raw, pause_reason_raw, payload_json, redaction_version, created_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)"#,
    )
    .bind(&event.id)
    .bind(&event.escalation_ledger_id)
    .bind(&event.event_kind_raw)
    .bind(&event.tier_id)
    .bind(&event.tier_kind_raw)
    .bind(&event.trigger_raw)
    .bind(&event.pause_reason_raw)
    .bind(&event.payload_json)
    .bind(&event.redaction_version)
    .bind(event.created_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_events_by_ledger(
    pool: &SqlitePool,
    escalation_ledger_id: &str,
) -> Result<Vec<EscalationEvent>> {
    let rows = sqlx::query(
        r#"SELECT id, escalation_ledger_id, event_kind_raw, tier_id, tier_kind_raw,
                  trigger_raw, pause_reason_raw, payload_json, redaction_version, created_at
           FROM escalation_events
           WHERE escalation_ledger_id = ?
           ORDER BY created_at ASC"#,
    )
    .bind(escalation_ledger_id)
    .fetch_all(pool)
    .await?;

    rows.into_iter()
        .map(|row| {
            let created_at_str: String = row.try_get("created_at")?;
            Ok(EscalationEvent {
                id: row.try_get("id")?,
                escalation_ledger_id: row.try_get("escalation_ledger_id")?,
                event_kind_raw: row.try_get("event_kind_raw")?,
                tier_id: row.try_get("tier_id")?,
                tier_kind_raw: row.try_get("tier_kind_raw")?,
                trigger_raw: row.try_get("trigger_raw")?,
                pause_reason_raw: row.try_get("pause_reason_raw")?,
                payload_json: row.try_get("payload_json")?,
                redaction_version: row.try_get("redaction_version")?,
                created_at: created_at_str
                    .parse()
                    .map_err(|e| anyhow!("bad created_at: {e}"))?,
            })
        })
        .collect()
}

/// Validate and persist the shadow escalation columns on agent_execution_runtime_facts.
/// `would_select_decision_json` must be well-formed JSON if present — proposal mandate.
pub async fn update_shadow_escalation_columns_tx(
    tx: &mut Transaction<'_, Sqlite>,
    agent_execution_id: &str,
    would_select_tier_id: Option<&str>,
    would_select_trigger_raw: Option<&str>,
    would_select_decision_json: Option<&str>,
) -> Result<()> {
    validate_json_field("would_select_decision_json", &would_select_decision_json.map(str::to_owned))?;

    let rows_affected = sqlx::query(
        r#"UPDATE agent_execution_runtime_facts SET
           would_select_tier_id     = ?1,
           would_select_trigger_raw = ?2,
           would_select_decision_json = ?3
           WHERE agent_execution_id = ?4"#,
    )
    .bind(would_select_tier_id)
    .bind(would_select_trigger_raw)
    .bind(would_select_decision_json)
    .bind(agent_execution_id)
    .execute(&mut **tx)
    .await?
    .rows_affected();

    if rows_affected == 0 {
        bail!("no agent_execution_runtime_facts row found for agent_execution_id={agent_execution_id}");
    }
    Ok(())
}
