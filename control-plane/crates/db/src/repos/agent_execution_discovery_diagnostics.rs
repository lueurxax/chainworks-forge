use anyhow::{Context, Result};
use chrono::{DateTime, Utc};
use sqlx::{Row, Sqlite, SqlitePool, Transaction};

use domain::discovery::{AgentExecutionDiscoveryDiagnostics, DiscoveryDiagnosticsV1};
use domain::ids::{AgentExecutionId, RunId};

#[derive(Clone, Debug)]
pub struct AgentExecutionDiscoveryDiagnosticsReadback {
    pub diagnostics: AgentExecutionDiscoveryDiagnostics,
    pub reconciliation_pending: bool,
    pub reconciliation_warnings: Vec<String>,
    pub runtime_facts_present: bool,
    pub matching_active_artifact_generation_count: i64,
}

impl AgentExecutionDiscoveryDiagnosticsReadback {
    pub fn projected_payload(&self) -> DiscoveryDiagnosticsV1 {
        let mut payload = self.diagnostics.payload.clone();
        payload.acp_reconciliation_pending = Some(self.reconciliation_pending);
        if self.reconciliation_pending
            && !payload
                .resume_warnings
                .iter()
                .any(|warning| warning == "reconciliation_pending")
        {
            payload
                .resume_warnings
                .push("reconciliation_pending".into());
        }
        for warning in &self.reconciliation_warnings {
            if !payload.warnings.iter().any(|existing| existing == warning) {
                payload.warnings.push(warning.clone());
            }
        }
        if payload.acp_resume_discovery_warning.is_none() {
            payload.acp_resume_discovery_warning = payload.resume_warnings.first().cloned();
        }
        payload
    }
}

pub async fn upsert(
    pool: &SqlitePool,
    diagnostics: &AgentExecutionDiscoveryDiagnostics,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    upsert_tx(&mut tx, diagnostics).await?;
    tx.commit().await?;
    Ok(())
}

pub async fn upsert_tx(
    tx: &mut Transaction<'_, Sqlite>,
    diagnostics: &AgentExecutionDiscoveryDiagnostics,
) -> Result<()> {
    let payload_json = serde_json::to_string(&diagnostics.payload)
        .context("serialize discovery diagnostics payload")?;
    sqlx::query(
        r#"INSERT INTO agent_execution_discovery_diagnostics
           (agent_execution_id, discovery_diagnostics_json, discovery_schema_version,
            legacy_broad_discovery_used, missing_required_output_count, rejected_output_count,
            stale_output_count, meta_discovery_truncated, git_manifest_status,
            resume_warning_count, created_at, updated_at)
           VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
           ON CONFLICT(agent_execution_id) DO UPDATE SET
             discovery_diagnostics_json = excluded.discovery_diagnostics_json,
             discovery_schema_version = excluded.discovery_schema_version,
             legacy_broad_discovery_used = excluded.legacy_broad_discovery_used,
             missing_required_output_count = excluded.missing_required_output_count,
             rejected_output_count = excluded.rejected_output_count,
             stale_output_count = excluded.stale_output_count,
             meta_discovery_truncated = excluded.meta_discovery_truncated,
             git_manifest_status = excluded.git_manifest_status,
             resume_warning_count = excluded.resume_warning_count,
             updated_at = excluded.updated_at"#,
    )
    .bind(&diagnostics.agent_execution_id)
    .bind(payload_json)
    .bind(&diagnostics.discovery_schema_version)
    .bind(bool_to_i64(diagnostics.legacy_broad_discovery_used))
    .bind(diagnostics.missing_required_output_count)
    .bind(diagnostics.rejected_output_count)
    .bind(diagnostics.stale_output_count)
    .bind(bool_to_i64(diagnostics.meta_discovery_truncated))
    .bind(&diagnostics.git_manifest_status)
    .bind(diagnostics.resume_warning_count)
    .bind(diagnostics.created_at.to_rfc3339())
    .bind(diagnostics.updated_at.to_rfc3339())
    .execute(&mut **tx)
    .await?;
    Ok(())
}

pub async fn find_by_execution_id(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
) -> Result<Option<AgentExecutionDiscoveryDiagnostics>> {
    let row = sqlx::query(
        r#"SELECT agent_execution_id, discovery_diagnostics_json, discovery_schema_version,
                  legacy_broad_discovery_used, missing_required_output_count,
                  rejected_output_count, stale_output_count, meta_discovery_truncated,
                  git_manifest_status, resume_warning_count, created_at, updated_at
           FROM agent_execution_discovery_diagnostics
           WHERE agent_execution_id = ?1"#,
    )
    .bind(agent_execution_id.to_string())
    .fetch_optional(pool)
    .await?;
    row.map(|row| parse_discovery_diagnostics_row(&row))
        .transpose()
}

pub async fn find_readback_by_execution_id(
    pool: &SqlitePool,
    agent_execution_id: AgentExecutionId,
) -> Result<Option<AgentExecutionDiscoveryDiagnosticsReadback>> {
    let Some(diagnostics) = find_by_execution_id(pool, agent_execution_id).await? else {
        return Ok(None);
    };
    readback_for_diagnostics(pool, diagnostics).await.map(Some)
}

pub async fn list_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<AgentExecutionDiscoveryDiagnostics>> {
    let rows = sqlx::query(
        r#"SELECT addi.agent_execution_id, addi.discovery_diagnostics_json,
                  addi.discovery_schema_version, addi.legacy_broad_discovery_used,
                  addi.missing_required_output_count, addi.rejected_output_count,
                  addi.stale_output_count, addi.meta_discovery_truncated,
                  addi.git_manifest_status, addi.resume_warning_count,
                  addi.created_at, addi.updated_at
           FROM agent_execution_discovery_diagnostics addi
           INNER JOIN agent_executions ae ON ae.id = addi.agent_execution_id
           INNER JOIN stage_executions se ON se.id = ae.stage_execution_id
           WHERE se.run_id = ?1
           ORDER BY ae.started_at ASC"#,
    )
    .bind(run_id.to_string())
    .fetch_all(pool)
    .await?;
    rows.iter().map(parse_discovery_diagnostics_row).collect()
}

pub async fn list_readback_by_run(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<Vec<AgentExecutionDiscoveryDiagnosticsReadback>> {
    let diagnostics = list_by_run(pool, run_id).await?;
    let mut readbacks = Vec::with_capacity(diagnostics.len());
    for item in diagnostics {
        readbacks.push(readback_for_diagnostics(pool, item).await?);
    }
    Ok(readbacks)
}

async fn readback_for_diagnostics(
    pool: &SqlitePool,
    diagnostics: AgentExecutionDiscoveryDiagnostics,
) -> Result<AgentExecutionDiscoveryDiagnosticsReadback> {
    let runtime_row = sqlx::query(
        r#"SELECT output_settlement, valid_required_outputs
           FROM agent_execution_runtime_facts
           WHERE agent_execution_id = ?1"#,
    )
    .bind(&diagnostics.agent_execution_id)
    .fetch_optional(pool)
    .await?;
    let runtime_facts_present = runtime_row.is_some();
    let runtime_valid_required_outputs = runtime_row
        .as_ref()
        .is_some_and(|row| row.get::<i64, _>("valid_required_outputs") != 0);
    let runtime_output_settlement = runtime_row
        .as_ref()
        .map(|row| row.get::<String, _>("output_settlement"));
    let matching_active_artifact_generation_count: i64 = sqlx::query_scalar(
        r#"SELECT COUNT(*)
           FROM active_artifact_contracts active
           INNER JOIN artifact_contract_generations generation
             ON generation.generation_id = active.generation_id
           WHERE generation.source_agent_execution_id = ?1
             AND generation.valid = 1"#,
    )
    .bind(&diagnostics.agent_execution_id)
    .fetch_one(pool)
    .await?;

    let accepted_decision_count = diagnostics
        .payload
        .decisions
        .iter()
        .filter(|decision| decision.is_accepted())
        .count();
    let mut reconciliation_warnings = Vec::new();
    if !runtime_facts_present {
        reconciliation_warnings.push(
            "reconciliation_pending: discovery diagnostics exist without matching runtime facts"
                .to_string(),
        );
    }
    if diagnostics.missing_required_output_count > 0 && runtime_valid_required_outputs {
        reconciliation_warnings.push(
            "reconciliation_pending: runtime facts mark required outputs valid while discovery diagnostics report missing outputs"
                .to_string(),
        );
    }
    if accepted_decision_count > 0
        && runtime_valid_required_outputs
        && matching_active_artifact_generation_count == 0
    {
        reconciliation_warnings.push(
            "reconciliation_pending: runtime facts mark required outputs valid without matching active artifact generation truth"
                .to_string(),
        );
    }
    if matches!(
        runtime_output_settlement.as_deref(),
        Some("valid_outputs_from_completed_execution" | "valid_outputs_from_failed_execution")
    ) && matching_active_artifact_generation_count == 0
        && accepted_decision_count > 0
    {
        reconciliation_warnings.push(
            "reconciliation_pending: valid output settlement has no matching active artifact generation truth"
                .to_string(),
        );
    }
    reconciliation_warnings.sort();
    reconciliation_warnings.dedup();
    let reconciliation_pending = !reconciliation_warnings.is_empty();

    Ok(AgentExecutionDiscoveryDiagnosticsReadback {
        diagnostics,
        reconciliation_pending,
        reconciliation_warnings,
        runtime_facts_present,
        matching_active_artifact_generation_count,
    })
}

fn parse_discovery_diagnostics_row(
    row: &sqlx::sqlite::SqliteRow,
) -> Result<AgentExecutionDiscoveryDiagnostics> {
    let payload_json: String = row.get("discovery_diagnostics_json");
    let payload: DiscoveryDiagnosticsV1 =
        serde_json::from_str(&payload_json).context("parse discovery diagnostics payload")?;
    let created_at_raw: String = row.get("created_at");
    let updated_at_raw: String = row.get("updated_at");
    Ok(AgentExecutionDiscoveryDiagnostics {
        agent_execution_id: row.get("agent_execution_id"),
        payload,
        discovery_schema_version: row.get("discovery_schema_version"),
        legacy_broad_discovery_used: row.get::<i64, _>("legacy_broad_discovery_used") != 0,
        missing_required_output_count: row.get("missing_required_output_count"),
        rejected_output_count: row.get("rejected_output_count"),
        stale_output_count: row.get("stale_output_count"),
        meta_discovery_truncated: row.get::<i64, _>("meta_discovery_truncated") != 0,
        git_manifest_status: row.get("git_manifest_status"),
        resume_warning_count: row.get("resume_warning_count"),
        created_at: DateTime::parse_from_rfc3339(&created_at_raw)?.with_timezone(&Utc),
        updated_at: DateTime::parse_from_rfc3339(&updated_at_raw)?.with_timezone(&Utc),
    })
}

fn bool_to_i64(value: bool) -> i64 {
    if value {
        1
    } else {
        0
    }
}
