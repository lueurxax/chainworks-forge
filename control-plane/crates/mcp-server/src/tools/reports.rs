use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::{agent_executions, artifacts, scheduler, validation};
use domain::artifact::Artifact;
use domain::ids::RunId;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;

pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "reports.get".to_string(),
        description: "Get report and release artifact payloads for a run".to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "required": ["run_id"],
            "properties": {
                "run_id": { "type": "string", "description": "The run ID to retrieve report artifacts for" }
            }
        }),
    }]
}

pub async fn execute(
    tool_name: &str,
    params: serde_json::Value,
    pool: &SqlitePool,
    _cmd_handler: &CommandHandler,
) -> Result<serde_json::Value> {
    match tool_name {
        "reports.get" => {
            let run_id: RunId = params["run_id"]
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("Missing 'run_id'"))?
                .parse()?;

            let all_artifacts = artifacts::list_by_run(pool, run_id).await?;
            let mut reports = Vec::new();
            for artifact in all_artifacts.into_iter() {
                if artifact.report_kind.is_some() || is_release_report_artifact(&artifact.name) {
                    reports.push(artifact_report_json(pool, &artifact).await?);
                }
            }
            reports.push(serde_json::json!({
                "id": uuid::Uuid::new_v4().to_string(),
                "run_id": run_id.to_string(),
                "stage_id": "__run__",
                "agent_id": "system",
                "name": "mcp_execution_truth",
                "contract_id": "mcp_execution_truth",
                "format": "json",
                "file_path": "",
                "checksum_sha256": serde_json::Value::Null,
                "size_bytes": serde_json::Value::Null,
                "provider": "system",
                "model": serde_json::Value::Null,
                "created_at": chrono::Utc::now().to_rfc3339(),
                "is_pinned": false,
                "report_kind": "mcp_execution_truth",
                "report_version": 1,
                "agent_executions": execution_mcp_truth_json(pool, run_id).await?,
                "scheduler": scheduler_mcp_truth_json(pool, run_id).await?,
            }));

            Ok(serde_json::Value::Array(reports))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

pub(crate) async fn execution_mcp_truth_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let executions = agent_executions::list_by_run(pool, run_id).await?;
    Ok(serde_json::Value::Array(
        executions
            .into_iter()
            .map(|execution| {
                serde_json::json!({
                    "agent_execution_id": execution.id.to_string(),
                    "stage_execution_id": execution.stage_execution_id.to_string(),
                    "agent_id": execution.agent_id,
                    "provider": execution.provider,
                    "model": execution.model,
                    "status": execution.status.to_string(),
                    "backend_profile_id": execution.backend_profile_id,
                    "requested_mcp_extensions_json": execution.requested_mcp_extensions_json,
                    "predicted_mcp_extensions_json": execution.predicted_mcp_extensions_json,
                    "predicted_mcp_runtime_ids_json": execution.predicted_mcp_runtime_ids_json,
                    "actual_mcp_extensions_json": execution.actual_mcp_extensions_json,
                    "actual_mcp_runtime_ids_json": execution.actual_mcp_runtime_ids_json,
                    "denied_mcp_extensions_json": execution.denied_mcp_extensions_json,
                    "mcp_blocking_issues_json": execution.mcp_blocking_issues_json,
                    "actual_mcp_observation_json": execution.actual_mcp_observation_json,
                    "mcp_session_startup_latency_ms": execution.mcp_session_startup_latency_ms,
                })
            })
            .collect(),
    ))
}

pub(crate) async fn scheduler_mcp_truth_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    let latest = scheduler::latest_health_snapshot(pool).await?;
    let health = latest.map(|snapshot| {
        let is_stale = snapshot.is_stale_at(chrono::Utc::now());
        serde_json::json!({
            "queued_count": snapshot.queued_count,
            "oldest_queued_age_ms": snapshot.oldest_queued_age_ms,
            "global_queue_depth": snapshot.global_queue_depth,
            "active_agent_executions": snapshot.active_agent_executions,
            "db_writer_wait_p95_ms": snapshot.db_writer_wait_p95_ms,
            "command_latency_p95_ms_json": snapshot.command_latency_p95_ms_json,
            "last_host_interruption_epoch_id": snapshot.last_host_interruption_epoch_id,
            "sustained_backpressure_state": snapshot.sustained_backpressure_state,
            "stale_after_ms": snapshot.stale_after_ms,
            "updated_at": snapshot.updated_at.to_rfc3339(),
            "is_stale": is_stale,
        })
    });
    let queue_summaries = scheduler::list_queue_summaries_by_run(pool, &run_id.to_string())
        .await?
        .into_iter()
        .map(|summary| {
            let is_stale = summary.is_stale_at(chrono::Utc::now());
            serde_json::json!({
                "scope": summary.scope,
                "scope_id": summary.scope_id,
                "run_id": summary.run_id,
                "stage_execution_id": summary.stage_execution_id,
                "provider_family": summary.provider_family,
                "top_reason": summary.top_reason,
                "queued_count": summary.queued_count,
                "oldest_queued_age_ms": summary.oldest_queued_age_ms,
                "global_queue_depth": summary.global_queue_depth,
                "stale_after_ms": summary.stale_after_ms,
                "updated_at": summary.updated_at.to_rfc3339(),
                "is_stale": is_stale,
            })
        })
        .collect::<Vec<_>>();
    let active_execution_counts_by_provider =
        scheduler::list_active_execution_counts_by_provider(pool)
            .await?
            .into_iter()
            .map(|count| {
                serde_json::json!({
                    "provider_family": count.provider_family,
                    "active_count": count.active_count,
                })
            })
            .collect::<Vec<_>>();
    let run_queue_position_hint = scheduler::queue_position_hint_by_run(pool, &run_id.to_string())
        .await?
        .map(|hint| {
            serde_json::json!({
                "scope": hint.scope,
                "scope_id": hint.scope_id,
                "run_id": hint.run_id,
                "stage_execution_id": hint.stage_execution_id,
                "queue_position": hint.queue_position,
                "global_queue_depth": hint.global_queue_depth,
                "queued_ahead_count": hint.queued_ahead_count,
                "scoped_queued_count": hint.scoped_queued_count,
                "oldest_queued_age_ms": hint.oldest_queued_age_ms,
                "updated_at": hint.updated_at.to_rfc3339(),
            })
        });
    let host_interruption_epochs =
        scheduler::list_host_interruption_epochs_by_run(pool, &run_id.to_string())
            .await?
            .into_iter()
            .map(|readback| {
                let affected_executions = readback
            .affected_executions
            .into_iter()
            .map(|affected| {
                serde_json::json!({
                    "epoch_id": affected.epoch_id,
                    "agent_execution_id": affected.agent_execution_id,
                    "run_id": affected.run_id,
                    "stage_execution_id": affected.stage_execution_id,
                    "provider_family": affected.provider_family,
                    "action": affected.action,
                    "retry_enqueued_at": affected.retry_enqueued_at.map(|value| value.to_rfc3339()),
                    "created_at": affected.created_at.to_rfc3339(),
                })
            })
            .collect::<Vec<_>>();
                serde_json::json!({
                    "id": readback.epoch.id,
                    "kind": readback.epoch.kind,
                    "started_at": readback.epoch.started_at.to_rfc3339(),
                    "ended_at": readback.epoch.ended_at.map(|value| value.to_rfc3339()),
                    "monotonic_gap_ms": readback.epoch.monotonic_gap_ms,
                    "wall_clock_gap_ms": readback.epoch.wall_clock_gap_ms,
                    "details_json": readback.epoch.details_json,
                    "created_at": readback.epoch.created_at.to_rfc3339(),
                    "affected_executions": affected_executions,
                })
            })
            .collect::<Vec<_>>();

    let db_writer_contention_summary = health.as_ref().map(|health| {
        serde_json::json!({
            "db_writer_wait_p95_ms": health["db_writer_wait_p95_ms"],
            "stale_after_ms": health["stale_after_ms"],
            "updated_at": health["updated_at"],
            "is_stale": health["is_stale"],
        })
    });
    let command_latency_summary = health.as_ref().map(|health| {
        serde_json::json!({
            "command_latency_p95_ms_json": health["command_latency_p95_ms_json"],
            "stale_after_ms": health["stale_after_ms"],
            "updated_at": health["updated_at"],
            "is_stale": health["is_stale"],
        })
    });
    let sustained_backpressure_notification = scheduler::latest_backpressure_notification(pool)
        .await?
        .map(|notification| {
            let is_stale = notification.is_stale_at(chrono::Utc::now());
            serde_json::json!({
                "method": "scheduler.backpressure.changed",
                "params": {
                    "run_id": notification.run_id,
                    "stage_execution_id": notification.stage_execution_id,
                    "provider_family": notification.provider_family,
                    "top_reason": notification.top_reason,
                    "queued_count": notification.queued_count,
                    "oldest_queued_age_ms": notification.oldest_queued_age_ms,
                    "global_queue_depth": notification.global_queue_depth,
                    "state": notification.state,
                    "updated_at": notification.updated_at.to_rfc3339(),
                    "is_stale": is_stale,
                }
            })
        });

    Ok(serde_json::json!({
        "health": health,
        "queue_summaries": queue_summaries,
        "active_execution_counts_by_provider": active_execution_counts_by_provider,
        "run_queue_position_hint": run_queue_position_hint,
        "db_writer_contention_summary": db_writer_contention_summary,
        "command_latency_summary": command_latency_summary,
        "sustained_backpressure_notification": sustained_backpressure_notification,
        "host_interruption_epochs": host_interruption_epochs,
    }))
}

fn is_release_report_artifact(name: &str) -> bool {
    matches!(
        name,
        "release_manifest"
            | "git_push_receipt"
            | "release_bundle_manifest"
            | "connect_upload_receipt"
            | "delivery_receipt"
    )
}

pub(crate) async fn artifact_report_json(
    pool: &SqlitePool,
    artifact: &Artifact,
) -> Result<serde_json::Value> {
    let mut value = serde_json::to_value(artifact)?;

    if artifact.report_kind.as_deref() == Some("validation_failure") {
        if let Some(record) = validation::find_by_artifact_id(pool, artifact.id).await? {
            let agent_execution_id = record.agent_execution_id;
            let mut payload = ValidationFailureRecordPayload::from(record);
            if let Some(execution) =
                db::repos::agent_executions::find_by_id(pool, agent_execution_id).await?
            {
                payload.session_reuse_disposition = execution.session_reuse_disposition;
                payload.session_reset_reason = execution.session_reset_reason;
            }

            if let serde_json::Value::Object(ref mut map) = value {
                map.insert(
                    "validation_failure_record".to_string(),
                    serde_json::to_value(payload)?,
                );
            }
        } else if let serde_json::Value::Object(ref mut map) = value {
            map.insert(
                "validation_failure_record".to_string(),
                serde_json::Value::Null,
            );
        }
    }
    if artifact.report_kind.as_deref() == Some("failed_stage_evidence") {
        let payload = std::fs::read_to_string(&artifact.file_path)
            .ok()
            .and_then(|content| serde_json::from_str::<serde_json::Value>(&content).ok());
        if let serde_json::Value::Object(ref mut map) = value {
            map.insert(
                "failed_stage_evidence".to_string(),
                payload.unwrap_or(serde_json::Value::Null),
            );
        }
    }

    Ok(value)
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ValidationFailureRecordPayload {
    id: String,
    timestamp: String,
    #[serde(rename = "agentID")]
    agent_id: String,
    #[serde(rename = "stageID")]
    stage_id: String,
    #[serde(rename = "runID")]
    run_id: String,
    output_results: Vec<OutputValidationResultPayload>,
    failure_summary: String,
    failure_class: String,
    contract_metadata: Vec<ContractValidationMetadataPayload>,
    raw_output_exists: bool,
    receipt_exists: bool,
    transcript_exists: bool,
    recovery_recommendation: RecoveryRecommendationPayload,
    session_reuse_disposition: Option<String>,
    session_reset_reason: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct OutputValidationResultPayload {
    output_name: String,
    #[serde(rename = "contractID")]
    contract_id: Option<String>,
    status: String,
    missing_fields: Vec<String>,
    validation_error: Option<String>,
    raw_payload_size: i64,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct ContractValidationMetadataPayload {
    output_name: String,
    #[serde(rename = "contractID")]
    contract_id: String,
    machine_format: String,
    validation_mode: String,
    required_field_count: i64,
    raw_artifact_name: Option<String>,
    normalized_artifact_name: Option<String>,
}

#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct RecoveryRecommendationPayload {
    action: String,
    explanation: String,
    #[serde(default)]
    source: Option<String>,
}

impl From<domain::validation::ValidationFailureRecord> for ValidationFailureRecordPayload {
    fn from(record: domain::validation::ValidationFailureRecord) -> Self {
        ValidationFailureRecordPayload {
            id: record.id,
            timestamp: record.timestamp.to_rfc3339(),
            agent_id: record.agent_id,
            stage_id: record.stage_id,
            run_id: record.run_id.to_string(),
            output_results: record
                .output_results
                .into_iter()
                .map(|output| OutputValidationResultPayload {
                    output_name: output.output_name,
                    contract_id: output.contract_id,
                    status: match output.status {
                        domain::validation::ValidationStatus::Passed => "passed".to_string(),
                        domain::validation::ValidationStatus::Failed => "failed".to_string(),
                        domain::validation::ValidationStatus::NoContractDeclared => {
                            "no_contract_declared".to_string()
                        }
                    },
                    missing_fields: output.missing_fields,
                    validation_error: output.validation_error,
                    raw_payload_size: output.raw_payload_size as i64,
                })
                .collect(),
            failure_summary: record.failure_summary,
            failure_class: match record.failure_class {
                domain::validation::ValidationFailureClass::OutputContractMismatch => {
                    "output_contract_mismatch".to_string()
                }
                domain::validation::ValidationFailureClass::NoOutputProduced => {
                    "no_output_produced".to_string()
                }
                domain::validation::ValidationFailureClass::EmptyOutput => {
                    "empty_output".to_string()
                }
                domain::validation::ValidationFailureClass::PersistenceFailure => {
                    "persistence_failure".to_string()
                }
            },
            contract_metadata: record
                .contract_metadata
                .into_iter()
                .map(|meta| ContractValidationMetadataPayload {
                    output_name: meta.output_name,
                    contract_id: meta.contract_id,
                    machine_format: meta.machine_format,
                    validation_mode: meta.validation_mode,
                    required_field_count: meta.required_field_count as i64,
                    raw_artifact_name: meta.raw_artifact_name,
                    normalized_artifact_name: meta.normalized_artifact_name,
                })
                .collect(),
            raw_output_exists: record.raw_output_exists,
            receipt_exists: record.receipt_exists,
            transcript_exists: record.transcript_exists,
            recovery_recommendation: RecoveryRecommendationPayload {
                action: record.recovery_recommendation.action,
                explanation: record.recovery_recommendation.explanation,
                source: None,
            },
            session_reuse_disposition: None,
            session_reset_reason: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{artifacts, ideas, runs, scheduler, validation};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
    use engine::event_bus;
    use engine::work_queue::WorkQueue;

    fn make_idea(id: IdeaId) -> Idea {
        Idea {
            id,
            title: "Test idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        }
    }

    fn validation_failure_payload(run_id: RunId) -> serde_json::Value {
        serde_json::json!({
            "id": "11111111-1111-1111-1111-111111111111",
            "timestamp": "2026-04-15T09:30:00Z",
            "agentID": "validation_agent",
            "stageID": "stage_1",
            "runID": run_id.to_string(),
            "outputResults": [{
                "outputName": "report",
                "contractID": "report_v1",
                "status": "failed",
                "missingFields": ["summary"],
                "validationError": "Missing required fields: summary",
                "rawPayloadSize": 17
            }],
            "failureSummary": "report: Missing required fields: summary",
            "failureClass": "output_contract_mismatch",
            "contractMetadata": [{
                "outputName": "report",
                "contractID": "report_v1",
                "machineFormat": "json",
                "validationMode": "strict_structured",
                "requiredFieldCount": 1,
                "rawArtifactName": "report_raw",
                "normalizedArtifactName": "report"
            }],
            "rawOutputExists": true,
            "receiptExists": false,
            "transcriptExists": true,
            "recoveryRecommendation": {
                "action": "retry_failed_agent",
                "explanation": "Retry the agent with the same inputs.",
                "source": "runtime_policy"
            }
        })
    }

    fn validation_failure_record(
        artifact_id: ArtifactId,
        run_id: RunId,
        stage_execution_id: domain::ids::StageExecutionId,
        agent_execution_id: domain::ids::AgentExecutionId,
    ) -> ValidationFailureRecord {
        ValidationFailureRecord {
            id: "11111111-1111-1111-1111-111111111111".to_string(),
            artifact_id,
            timestamp: chrono::DateTime::parse_from_rfc3339("2026-04-15T09:30:00Z")
                .unwrap()
                .with_timezone(&Utc),
            agent_id: "validation_agent".to_string(),
            stage_id: "stage_1".to_string(),
            stage_execution_id,
            agent_execution_id,
            run_id,
            output_results: vec![OutputValidationResult {
                output_name: "report".to_string(),
                contract_id: Some("report_v1".to_string()),
                status: ValidationStatus::Failed,
                missing_fields: vec!["summary"].into_iter().map(String::from).collect(),
                validation_error: Some("Missing required fields: summary".to_string()),
                raw_payload_size: 17,
            }],
            failure_summary: "report: Missing required fields: summary".to_string(),
            failure_class: ValidationFailureClass::OutputContractMismatch,
            contract_metadata: vec![ContractValidationMetadata {
                output_name: "report".to_string(),
                contract_id: "report_v1".to_string(),
                machine_format: "json".to_string(),
                validation_mode: "strict_structured".to_string(),
                required_field_count: 1,
                raw_artifact_name: Some("report_raw".to_string()),
                normalized_artifact_name: Some("report".to_string()),
            }],
            raw_output_exists: true,
            receipt_exists: false,
            transcript_exists: true,
            recovery_recommendation: RecoveryRecommendation {
                action: "retry_failed_agent".to_string(),
                explanation: "Retry the agent with the same inputs.".to_string(),
            },
        }
    }

    async fn test_pool() -> sqlx::SqlitePool {
        create_pool("sqlite::memory:")
            .await
            .expect("in-memory pool failed")
    }

    async fn seed_validation_attempt(
        pool: &sqlx::SqlitePool,
        run_id: RunId,
    ) -> (domain::ids::StageExecutionId, domain::ids::AgentExecutionId) {
        let stage_execution_id = domain::ids::StageExecutionId::new();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        db::repos::stages::insert(
            pool,
            &domain::stage::StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "stage_1".to_string(),
                label: "Stage 1".to_string(),
                status: domain::stage::StageStatus::Failed,
                iteration: 1,
                attempt_number: 1,
                settlement_kind: Some(domain::stage::StageSettlementKind::Failed),
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                owner_agent: Some("validation_agent".to_string()),
                provider: Some("system".to_string()),
                model: None,
                stage_type: None,
                validation_failure_json: None,
                evidence_packet_json: None,
                recovery_snapshot_json: None,
                retry_reason: None,
            },
        )
        .await
        .unwrap();
        db::repos::agent_executions::insert(
            pool,
            &domain::agent::AgentExecution {
                id: agent_execution_id,
                stage_execution_id,
                agent_id: "validation_agent".to_string(),
                provider: "system".to_string(),
                model: None,
                started_at: Utc::now(),
                completed_at: Some(Utc::now()),
                status: domain::agent::AgentStatus::Failed,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: Some("reused".into()),
                session_reset_reason: Some("operator_reset".into()),
                backend_profile_id: Some("codex_with_mcp".into()),
                requested_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                predicted_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                actual_mcp_extensions_json: Some(r#"["filesystem"]"#.into()),
                actual_mcp_runtime_ids_json: Some(r#"["fs-runtime"]"#.into()),
                denied_mcp_extensions_json: Some("[]".into()),
                mcp_blocking_issues_json: Some("[]".into()),
                actual_mcp_observation_json: Some(
                    r#"{"source":"provider_session_new_response"}"#.into(),
                ),
                mcp_session_startup_latency_ms: Some(17),
            },
        )
        .await
        .unwrap();
        (stage_execution_id, agent_execution_id)
    }

    fn make_command_handler(pool: sqlx::SqlitePool) -> CommandHandler {
        let events = event_bus::new_bus(64);
        let work_queue = WorkQueue::new(pool.clone());
        CommandHandler::new(pool, events, work_queue)
    }

    fn make_run(id: RunId, idea_id: IdeaId) -> domain::run::Run {
        domain::run::Run {
            id,
            idea_id,
            status: domain::run::RunStatus::Ready,
            workflow_id: "wf-release".into(),
            workflow_title: "Release".into(),
            workspace_root: "/tmp/ws".into(),
            artifact_root: "/tmp/art".into(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
            cancellation_settlement_log: None,
            current_state: None,
            workflow_yaml_path: None,
            agent_catalog_yaml_path: None,
            worktree_root: None,
            base_branch: None,
            base_revision: None,
            target_branch: None,
            delivery_configuration_json: Some(
                "{\"repo_identifier\":\"repo-1\",\"repo_root\":\"/repo\",\"base_branch\":\"main\",\"worktree_base_path\":\"/tmp/worktrees\",\"target_branch\":\"cw/release\"}"
                    .into(),
            ),
            delivery_preflight_json: None,
            workflow_family: None,
            project_key: None,
            risk_class: None,
            stack: None,
            workflow_snapshot_hash: None,
            catalog_snapshot_hash: None,
            workflow_snapshot_json: None,
            catalog_snapshot_json: None,
            drift_detected_at: None,
            drift_details_json: None,
        }
    }

    fn make_artifact(run_id: RunId, name: &str, report_kind: Option<&str>) -> Artifact {
        Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_12_release".into(),
            agent_id: "release_agent".into(),
            name: name.into(),
            contract_id: name.into(),
            format: ArtifactFormat::Json,
            file_path: format!("/tmp/art/{name}.json"),
            checksum_sha256: None,
            size_bytes: None,
            provider: "custom".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: report_kind.map(|s| s.to_string()),
            report_version: None,
        }
    }

    #[tokio::test]
    async fn reports_get_includes_release_artifacts_and_report_kind_entries() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        artifacts::insert(&pool, &make_artifact(run_id, "release_manifest", None))
            .await
            .unwrap();
        artifacts::insert(&pool, &make_artifact(run_id, "delivery_receipt", None))
            .await
            .unwrap();
        artifacts::insert(
            &pool,
            &make_artifact(run_id, "execution_report", Some("execution_report")),
        )
        .await
        .unwrap();
        artifacts::insert(&pool, &make_artifact(run_id, "other_blob", None))
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
        )
        .await
        .unwrap();

        let reports: Vec<Artifact> = serde_json::from_value(result).unwrap();
        let names: Vec<String> = reports.into_iter().map(|artifact| artifact.name).collect();

        assert!(names.contains(&"release_manifest".to_string()));
        assert!(names.contains(&"delivery_receipt".to_string()));
        assert!(names.contains(&"execution_report".to_string()));
        assert!(!names.contains(&"other_blob".to_string()));
    }

    #[tokio::test]
    async fn reports_get_decodes_validation_failure_payload() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();

        let payload = validation_failure_payload(run_id);
        let payload_path = std::env::temp_dir().join(format!("validation-failure-{}.json", run_id));
        std::fs::write(&payload_path, serde_json::to_vec(&payload).unwrap()).unwrap();
        let (stage_execution_id, agent_execution_id) = seed_validation_attempt(&pool, run_id).await;

        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "stage_1".into(),
            agent_id: "validation_agent".into(),
            name: "validation_failure_validation_agent".into(),
            contract_id: "validation_failure_record".into(),
            format: ArtifactFormat::Json,
            file_path: payload_path.to_string_lossy().to_string(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("validation_failure".into()),
            report_version: None,
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        validation::insert(
            &pool,
            &validation_failure_record(artifact.id, run_id, stage_execution_id, agent_execution_id),
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("array");
        let validation_failure = reports
            .iter()
            .find(|artifact| artifact["report_kind"] == serde_json::json!("validation_failure"))
            .expect("validation failure artifact");

        assert_eq!(
            validation_failure["validation_failure_record"]["failureSummary"],
            serde_json::json!("report: Missing required fields: summary")
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["outputResults"][0]["missingFields"],
            serde_json::json!(["summary"])
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["sessionReuseDisposition"],
            serde_json::json!("reused")
        );
        assert_eq!(
            validation_failure["validation_failure_record"]["sessionResetReason"],
            serde_json::json!("operator_reset")
        );
    }

    #[tokio::test]
    async fn reports_mcp_resolution_truth_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        seed_validation_attempt(&pool, run_id).await;

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let execution = &mcp_truth["agent_executions"][0];

        assert_eq!(
            execution["backend_profile_id"],
            serde_json::json!("codex_with_mcp")
        );
        assert_eq!(
            execution["requested_mcp_extensions_json"],
            serde_json::json!(r#"["filesystem"]"#)
        );
        assert_eq!(
            execution["actual_mcp_runtime_ids_json"],
            serde_json::json!(r#"["fs-runtime"]"#)
        );
    }

    #[tokio::test]
    async fn proposal_061_reports_get_includes_scheduler_readback() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let now = Utc::now();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        scheduler::insert_health_snapshot(
            &pool,
            &scheduler::SchedulerHealthSnapshot {
                id: "scheduler-health-mcp".into(),
                queued_count: 2,
                oldest_queued_age_ms: 305_000,
                global_queue_depth: 2,
                active_agent_executions: 20,
                db_writer_wait_p95_ms: Some(44),
                command_latency_p95_ms_json: Some(
                    r#"{"approve_stage":110,"retry_stage":170,"cancel_run":80}"#.into(),
                ),
                last_host_interruption_epoch_id: Some("mcp-host-epoch".into()),
                sustained_backpressure_state: "active".into(),
                stale_after_ms: 60_000,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        scheduler::upsert_queue_summary(
            &pool,
            &scheduler::SchedulerQueueSummary {
                scope: "run".into(),
                scope_id: run_id.to_string(),
                run_id: Some(run_id.to_string()),
                stage_execution_id: None,
                provider_family: Some("gemini".into()),
                top_reason: "provider_capacity".into(),
                queued_count: 2,
                oldest_queued_age_ms: 305_000,
                global_queue_depth: 2,
                stale_after_ms: 60_000,
                updated_at: now,
            },
        )
        .await
        .unwrap();
        let queued_payload = serde_json::json!({
            "run_id": run_id.to_string(),
            "stage_id": "stage-p061",
            "stage_execution_id": domain::ids::StageExecutionId::new().to_string(),
            "agent_id": "gemini-agent",
            "provider": "gemini",
        });
        sqlx::query(
            r#"INSERT INTO work_items
               (id, kind, payload_json, status, run_id, stage_id, created_at, scheduled_at)
               VALUES ('mcp-target-work', 'invoke_agent', ?1, 'pending', ?2, 'stage-p061', ?3, ?4)"#,
        )
        .bind(queued_payload.to_string())
        .bind(run_id.to_string())
        .bind((now - chrono::Duration::seconds(10)).to_rfc3339())
        .bind((now - chrono::Duration::seconds(10)).to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        let stage_execution_id = domain::ids::StageExecutionId::new();
        let agent_execution_id = domain::ids::AgentExecutionId::new();
        sqlx::query(
            r#"INSERT INTO stage_executions
               (id, run_id, stage_id, label, status, started_at)
               VALUES (?1, ?2, 'stage-host', 'Host interruption stage', 'running', ?3)"#,
        )
        .bind(stage_execution_id.to_string())
        .bind(run_id.to_string())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        sqlx::query(
            r#"INSERT INTO agent_executions
               (id, stage_execution_id, agent_id, provider, provider_family, status, started_at)
               VALUES (?1, ?2, 'agent-host', 'gemini', 'gemini', 'running', ?3)"#,
        )
        .bind(agent_execution_id.to_string())
        .bind(stage_execution_id.to_string())
        .bind(now.to_rfc3339())
        .execute(&pool)
        .await
        .unwrap();
        scheduler::insert_host_interruption_epoch(
            &pool,
            &scheduler::HostInterruptionEpoch {
                id: "mcp-host-epoch".into(),
                kind: "sleep_wake".into(),
                started_at: now - chrono::Duration::seconds(120),
                ended_at: Some(now - chrono::Duration::seconds(60)),
                monotonic_gap_ms: Some(60_000),
                wall_clock_gap_ms: Some(120_000),
                details_json: Some(r#"{"source":"sleep_wake"}"#.into()),
                created_at: now,
            },
        )
        .await
        .unwrap();
        scheduler::insert_host_interruption_affected_execution(
            &pool,
            &scheduler::HostInterruptionAffectedExecution {
                epoch_id: "mcp-host-epoch".into(),
                agent_execution_id: agent_execution_id.to_string(),
                run_id: Some(run_id.to_string()),
                stage_execution_id: stage_execution_id.to_string(),
                provider_family: Some("gemini".into()),
                action: "recovering_from_system_sleep".into(),
                retry_enqueued_at: Some(now + chrono::Duration::seconds(5)),
                created_at: now,
            },
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");

        assert_eq!(
            mcp_truth["scheduler"]["health"]["sustained_backpressure_state"],
            serde_json::json!("active")
        );
        assert_eq!(
            mcp_truth["scheduler"]["health"]["db_writer_wait_p95_ms"],
            serde_json::json!(44)
        );
        assert_eq!(
            mcp_truth["scheduler"]["health"]["last_host_interruption_epoch_id"],
            serde_json::json!("mcp-host-epoch")
        );
        assert_eq!(
            mcp_truth["scheduler"]["db_writer_contention_summary"]["db_writer_wait_p95_ms"],
            serde_json::json!(44)
        );
        assert_eq!(
            mcp_truth["scheduler"]["command_latency_summary"]["command_latency_p95_ms_json"],
            serde_json::json!(r#"{"approve_stage":110,"retry_stage":170,"cancel_run":80}"#)
        );
        assert_eq!(
            mcp_truth["scheduler"]["sustained_backpressure_notification"]["method"],
            serde_json::json!("scheduler.backpressure.changed")
        );
        assert_eq!(
            mcp_truth["scheduler"]["sustained_backpressure_notification"]["params"]["run_id"],
            serde_json::json!(run_id.to_string())
        );
        assert_eq!(
            mcp_truth["scheduler"]["sustained_backpressure_notification"]["params"]
                ["provider_family"],
            serde_json::json!("gemini")
        );
        assert_eq!(
            mcp_truth["scheduler"]["sustained_backpressure_notification"]["params"]["state"],
            serde_json::json!("active")
        );
        assert_eq!(
            mcp_truth["scheduler"]["queue_summaries"][0]["provider_family"],
            serde_json::json!("gemini")
        );
        assert_eq!(
            mcp_truth["scheduler"]["active_execution_counts_by_provider"],
            serde_json::json!([{
                "provider_family": "gemini",
                "active_count": 1
            }])
        );
        assert_eq!(
            mcp_truth["scheduler"]["run_queue_position_hint"]["queue_position"],
            serde_json::json!(1)
        );
        assert_eq!(
            mcp_truth["scheduler"]["host_interruption_epochs"][0]["kind"],
            serde_json::json!("sleep_wake")
        );
        assert_eq!(
            mcp_truth["scheduler"]["host_interruption_epochs"][0]["affected_executions"][0]
                ["action"],
            serde_json::json!("recovering_from_system_sleep")
        );
    }

    #[tokio::test]
    async fn reports_failed_stage_evidence_contract_tests() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let payload_path =
            std::env::temp_dir().join(format!("failed-stage-evidence-{run_id}.json"));
        std::fs::write(
            &payload_path,
            serde_json::to_vec(&serde_json::json!({
                "schema_version": 1,
                "report_kind": "failed_stage_evidence",
                "run_id": run_id.to_string(),
                "stage_id": "stage_1",
                "failure_summary": "failed",
                "recovery_snapshot": { "status": "available" }
            }))
            .unwrap(),
        )
        .unwrap();
        artifacts::insert(
            &pool,
            &Artifact {
                id: ArtifactId::new(),
                run_id,
                stage_id: "stage_1".into(),
                agent_id: "agent_1".into(),
                name: "failed_stage_evidence_stage_1".into(),
                contract_id: "failed_stage_evidence".into(),
                format: ArtifactFormat::Json,
                file_path: payload_path.to_string_lossy().to_string(),
                checksum_sha256: None,
                size_bytes: None,
                provider: "system".into(),
                model: None,
                created_at: Utc::now(),
                is_pinned: false,
                report_kind: Some("failed_stage_evidence".into()),
                report_version: Some(1),
            },
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
        )
        .await
        .unwrap();
        let reports = result.as_array().expect("reports array");
        let evidence = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("failed_stage_evidence"))
            .expect("failed-stage evidence report");

        assert_eq!(
            evidence["failed_stage_evidence"]["report_kind"],
            serde_json::json!("failed_stage_evidence")
        );
        assert_eq!(
            evidence["failed_stage_evidence"]["recovery_snapshot"]["status"],
            serde_json::json!("available")
        );
    }
}
