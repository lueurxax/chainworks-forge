use std::collections::BTreeMap;

use anyhow::Result;
use db::repos::{agent_executions, approvals, artifacts, stages};
use domain::run::Run;
use serde::Serialize;
use sqlx::Row;
use sqlx::SqlitePool;

#[derive(Debug, Serialize)]
pub struct RunDossier {
    pub run_id: String,
    pub started_at: String,
    pub completed_at: Option<String>,
    pub status: String,
    pub workflow_snapshot_hash: Option<String>,
    pub catalog_snapshot_hash: Option<String>,
    pub stage_execution_summaries: Vec<StageExecutionSummary>,
    pub approval_history: Vec<ApprovalSummary>,
    pub cost_breakdown: Vec<AgentCostSummary>,
    pub failure_retry_events: Vec<FailureRetryEvent>,
    pub artifact_manifest: Vec<ArtifactSummary>,
    pub loop_counters: Vec<serde_json::Value>,
    pub drift_detected_at: Option<String>,
    pub drift_details: Option<serde_json::Value>,
}

#[derive(Debug, Serialize)]
pub struct StageExecutionSummary {
    pub stage_id: String,
    pub label: String,
    pub status: String,
    pub iteration: i64,
    pub attempt_number: i64,
    pub retry_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ApprovalSummary {
    pub stage_id: String,
    pub decision: String,
    pub requested_at: String,
    pub decided_at: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct AgentCostSummary {
    pub agent_id: String,
    pub provider: String,
    pub model: Option<String>,
    pub status: String,
    pub cost_cents: i64,
}

#[derive(Debug, Serialize)]
pub struct FailureRetryEvent {
    pub stage_id: String,
    pub iteration: i64,
    pub attempt_number: i64,
    pub retry_reason: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct ArtifactSummary {
    pub name: String,
    pub contract_id: String,
    pub file_path: String,
    pub checksum_sha256: Option<String>,
}

pub async fn build_dossier(pool: &SqlitePool, run: &Run) -> Result<RunDossier> {
    let stages = stages::list_by_run(pool, run.id).await?;
    let approvals = approvals::list_by_run(pool, run.id).await?;
    let artifacts = artifacts::list_by_run(pool, run.id).await?;
    let agent_executions = agent_executions::list_by_run(pool, run.id).await?;
    let execution_costs = load_execution_costs(pool, &run.id.to_string()).await?;

    let failure_retry_events = stages
        .iter()
        .filter(|stage| stage.attempt_number > 1 || stage.retry_reason.is_some())
        .map(|stage| FailureRetryEvent {
            stage_id: stage.stage_id.clone(),
            iteration: stage.iteration,
            attempt_number: stage.attempt_number,
            retry_reason: stage.retry_reason.clone(),
        })
        .collect();

    Ok(RunDossier {
        run_id: run.id.to_string(),
        started_at: run.started_at.to_rfc3339(),
        completed_at: run.completed_at.map(|t| t.to_rfc3339()),
        status: run.status.to_string(),
        workflow_snapshot_hash: run.workflow_snapshot_hash.clone(),
        catalog_snapshot_hash: run.catalog_snapshot_hash.clone(),
        stage_execution_summaries: stages
            .iter()
            .map(|stage| StageExecutionSummary {
                stage_id: stage.stage_id.clone(),
                label: stage.label.clone(),
                status: stage.status.to_string(),
                iteration: stage.iteration,
                attempt_number: stage.attempt_number,
                retry_reason: stage.retry_reason.clone(),
            })
            .collect(),
        approval_history: approvals
            .iter()
            .map(|approval| ApprovalSummary {
                stage_id: approval.stage_id.clone(),
                decision: approval.decision.to_string(),
                requested_at: approval.requested_at.to_rfc3339(),
                decided_at: approval.decided_at.map(|t| t.to_rfc3339()),
            })
            .collect(),
        cost_breakdown: agent_executions
            .iter()
            .map(|execution| AgentCostSummary {
                agent_id: execution.agent_id.clone(),
                provider: execution.provider.clone(),
                model: execution.model.clone(),
                status: execution.status.to_string(),
                cost_cents: *execution_costs.get(&execution.id.to_string()).unwrap_or(&0),
            })
            .collect(),
        failure_retry_events,
        artifact_manifest: artifacts
            .iter()
            .map(|artifact| ArtifactSummary {
                name: artifact.name.clone(),
                contract_id: artifact.contract_id.clone(),
                file_path: artifact.file_path.clone(),
                checksum_sha256: artifact.checksum_sha256.clone(),
            })
            .collect(),
        loop_counters: Vec::new(),
        drift_detected_at: run.drift_detected_at.map(|t| t.to_rfc3339()),
        drift_details: run
            .drift_details_json
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok()),
    })
}

async fn load_execution_costs(pool: &SqlitePool, run_id: &str) -> Result<BTreeMap<String, i64>> {
    let rows = sqlx::query(
        r#"
        SELECT ae.id AS agent_execution_id, COALESCE(sg.cumulative_cost_cents, 0) AS cost_cents
        FROM agent_executions ae
        JOIN stage_executions se ON se.id = ae.stage_execution_id
        LEFT JOIN session_generations sg ON sg.id = ae.session_generation_id
        WHERE se.run_id = ?1
        "#,
    )
    .bind(run_id)
    .fetch_all(pool)
    .await?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("agent_execution_id"), row.get("cost_cents")))
        .collect())
}
