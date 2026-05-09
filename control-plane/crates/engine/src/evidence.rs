use anyhow::Result;
use chrono::{DateTime, Utc};

use db::evidence_spool::write_spool_file;
use db::repos::evidence_spool_refs::{
    insert_idempotent_via_dbwriter, EvidenceKind, EvidenceSpoolRef, EvidenceSpoolRefStatus,
};
use db::repos::{agent_executions, artifacts, stages};
use db::write_class::WriteResult;
use db::writer::DbWriter;
use domain::artifact::{Artifact, ArtifactFormat};
use domain::ids::{AgentExecutionId, StageExecutionId};
use domain::run::Run;
use domain::stage::StageExecution;

pub struct FailedStageEvidenceInput<'a> {
    pub run: &'a Run,
    pub stage_id: &'a str,
    pub stage_execution_id: StageExecutionId,
    pub agent_id: &'a str,
    pub agent_execution_id: AgentExecutionId,
    pub provider: &'a str,
    pub model: Option<String>,
    pub failed_at: DateTime<Utc>,
    pub db_writer: Option<&'a DbWriter>,
}

pub async fn build_and_persist_failed_stage_evidence(
    pool: &sqlx::SqlitePool,
    input: FailedStageEvidenceInput<'_>,
) -> Result<Artifact> {
    let stage = stages::find_by_id(pool, input.stage_execution_id).await?;
    let executions = agent_executions::find_by_stage(pool, input.stage_execution_id).await?;
    let current_execution = executions
        .iter()
        .find(|execution| execution.id == input.agent_execution_id)
        .or_else(|| executions.last());

    let validation_failure = stage
        .as_ref()
        .and_then(|stage| stage.validation_failure_json.as_deref())
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?;
    let recovery_snapshot = stage
        .as_ref()
        .and_then(|stage| stage.recovery_snapshot_json.as_deref())
        .map(serde_json::from_str::<serde_json::Value>)
        .transpose()?;

    let failure_summary = validation_failure
        .as_ref()
        .and_then(|value| value.get("failure_summary"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("Agent execution failed before producing a valid completed stage outcome.");
    let failure_class = validation_failure
        .as_ref()
        .and_then(|value| value.get("failure_class"))
        .and_then(serde_json::Value::as_str)
        .unwrap_or("agent_execution_failed");
    let raw_outputs_exist = validation_failure
        .as_ref()
        .and_then(|value| value.get("raw_output_exists"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let receipt_exists = validation_failure
        .as_ref()
        .and_then(|value| value.get("receipt_exists"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let transcript_exists = validation_failure
        .as_ref()
        .and_then(|value| value.get("transcript_exists"))
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let output_envelopes = validation_failure
        .as_ref()
        .and_then(|value| value.get("output_results"))
        .and_then(serde_json::Value::as_array)
        .map(|items| {
            items
                .iter()
                .map(|item| {
                    serde_json::json!({
                        "output_name": item.get("output_name").cloned().unwrap_or(serde_json::Value::Null),
                        "status": item.get("status").cloned().unwrap_or(serde_json::Value::Null),
                        "contract_id": item.get("contract_id").cloned().unwrap_or(serde_json::Value::Null),
                        "raw_payload_size": item.get("raw_payload_size").cloned().unwrap_or(serde_json::Value::Null),
                    })
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    let packet_id = uuid::Uuid::new_v4().to_string();
    let packet_timestamp = Utc::now();

    let packet = serde_json::json!({
        "id": packet_id,
        "schema_version": 1,
        "report_kind": "failed_stage_evidence",
        "timestamp": packet_timestamp,
        "run_id": input.run.id.to_string(),
        "stage_id": input.stage_id,
        "stage_execution_id": input.stage_execution_id.to_string(),
        "stage_label": stage.as_ref().map(|stage| stage.label.as_str()),
        "stage_attempt_number": stage.as_ref().map(|stage| stage.attempt_number),
        "agent_id": input.agent_id,
        "failed_agent_title": serde_json::Value::Null,
        "agent_execution_id": input.agent_execution_id.to_string(),
        "failure_summary": failure_summary,
        "failure_class": failure_class,
        "supervision_classification": "stage_failed",
        "canonical_outcome": serde_json::Value::Null,
        "transport_error_kind": serde_json::Value::Null,
        "output_presence": {
            "raw_outputs_exist": raw_outputs_exist,
            "receipt_exists": receipt_exists,
            "transcript_exists": transcript_exists,
        },
        "raw_outputs_exist": raw_outputs_exist,
        "receipt_exists": receipt_exists,
        "transcript_exists": transcript_exists,
        "validation_failure": validation_failure,
        "output_envelopes": output_envelopes,
        "timing": {
            "stage_started_at": stage.as_ref().map(|stage| stage.started_at),
            "stage_completed_at": Some(input.failed_at),
            "agent_started_at": current_execution.map(|execution| execution.started_at),
            "agent_completed_at": current_execution.and_then(|execution| execution.completed_at).or(Some(input.failed_at)),
            "agent_duration_seconds": current_execution.map(|execution| {
                input.failed_at
                    .signed_duration_since(execution.started_at)
                    .num_milliseconds() as f64 / 1000.0
            }),
        },
        "recovery_snapshot": recovery_snapshot,
    });
    let encoded = serde_json::to_string_pretty(&packet)?;
    let artifact_root = std::path::Path::new(&input.run.artifact_root);
    tokio::fs::create_dir_all(artifact_root).await?;
    let relative_path = format!(
        "evidence/runs/{}/stages/{}/agents/{}/runtime_event/failed-stage-evidence-{}.json",
        input.run.id, input.stage_execution_id, input.agent_execution_id, packet_id
    );
    let spool_output = write_spool_file(
        artifact_root,
        &input.run.id.to_string(),
        &relative_path,
        encoded.as_bytes(),
    )
    .await?;
    let spool_ref_id = format!("evsp_failed_stage_{}", packet_id.replace('-', ""));
    let spool_ref = EvidenceSpoolRef {
        id: spool_ref_id.clone(),
        metadata_version: 1,
        run_id: input.run.id.to_string(),
        stage_execution_id: Some(input.stage_execution_id.to_string()),
        stage_id: Some(input.stage_id.to_string()),
        agent_execution_id: Some(input.agent_execution_id.to_string()),
        agent_id: Some(input.agent_id.to_string()),
        kind: EvidenceKind::RuntimeEvent,
        relative_path: spool_output.relative_path.clone(),
        size_bytes: spool_output.size_bytes as i64,
        checksum_algorithm: spool_output.checksum_algorithm.to_string(),
        checksum: spool_output.checksum.clone(),
        producer_operation: "failed_stage_evidence_spool".to_string(),
        content_type: Some("application/json".to_string()),
        summary_json: Some(
            serde_json::json!({
                "byte_count": spool_output.size_bytes,
                "producer_label": "failed_stage_evidence",
                "started_at": packet_timestamp.to_rfc3339(),
                "finished_at": packet_timestamp.to_rfc3339(),
            })
            .to_string(),
        ),
        created_at: packet_timestamp,
        status: EvidenceSpoolRefStatus::Available,
    };
    let shared_writer;
    let writer = if let Some(writer) = input.db_writer {
        writer
    } else {
        shared_writer = db::writer::shared_writer_for(pool)
            .await
            .ok_or_else(|| anyhow::anyhow!("P075 shared DbWriter is not registered"))?;
        shared_writer.as_ref()
    };
    let metadata_result = insert_idempotent_via_dbwriter(writer, spool_ref).await;
    if metadata_result != WriteResult::Committed {
        anyhow::bail!(
            "failed to persist failed-stage evidence spool metadata via DbWriter: {}",
            metadata_result.as_str()
        );
    }
    let packet_pointer = serde_json::json!({
        "schema_version": 2,
        "report_kind": "failed_stage_evidence",
        "storage": "evidence_spool",
        "packet_id": packet_id,
        "evidence_spool_ref_id": spool_ref_id,
        "relative_path": spool_output.relative_path,
        "checksum_algorithm": spool_output.checksum_algorithm,
        "checksum": spool_output.checksum,
        "size_bytes": spool_output.size_bytes,
        "failure_summary": failure_summary,
        "failure_class": failure_class,
    });
    stages::update_evidence_packet_json(
        pool,
        input.stage_execution_id,
        &packet_pointer.to_string(),
    )
    .await?;

    let artifact = Artifact {
        id: domain::ids::ArtifactId::new(),
        run_id: input.run.id,
        stage_id: input.stage_id.to_string(),
        agent_id: input.agent_id.to_string(),
        name: format!("failed_stage_evidence_{}", input.stage_execution_id),
        contract_id: "failed_stage_evidence".to_string(),
        format: ArtifactFormat::Json,
        file_path: spool_output.absolute_path.to_string_lossy().into_owned(),
        checksum_sha256: None,
        size_bytes: Some(spool_output.size_bytes as i64),
        provider: input.provider.to_string(),
        model: input.model,
        created_at: Utc::now(),
        is_pinned: false,
        report_kind: Some("failed_stage_evidence".to_string()),
        report_version: Some(1),
        agent_execution_id: None,
    };
    artifacts::insert(pool, &artifact).await?;
    Ok(artifact)
}

pub async fn build_failed_stage_evidence_for_latest_execution(
    pool: &sqlx::SqlitePool,
    run: &Run,
    stage: &StageExecution,
    failed_at: DateTime<Utc>,
) -> Result<Option<Artifact>> {
    let executions = agent_executions::find_by_stage(pool, stage.id).await?;
    let Some(execution) = executions.last() else {
        return Ok(None);
    };
    build_and_persist_failed_stage_evidence(
        pool,
        FailedStageEvidenceInput {
            run,
            stage_id: &stage.stage_id,
            stage_execution_id: stage.id,
            agent_id: &execution.agent_id,
            agent_execution_id: execution.id,
            provider: &execution.provider,
            model: execution.model.clone(),
            failed_at,
            db_writer: None,
        },
    )
    .await
    .map(Some)
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{agent_executions, ideas, runs, stages};
    use domain::agent::{AgentExecution, AgentStatus};
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{AgentExecutionId, IdeaId, RunId, StageExecutionId};
    use domain::run::{Run, RunStatus};
    use domain::stage::{StageExecution, StageSettlementKind, StageStatus};

    #[tokio::test]
    async fn failed_stage_evidence_packet_tests() {
        let pool = create_pool("sqlite::memory:").await.unwrap();
        let db_writer = db::writer::DbWriter::new(pool.clone());
        let tmp = tempfile::tempdir().unwrap();
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        let stage_execution_id = StageExecutionId::new();
        let agent_execution_id = AgentExecutionId::new();
        let now = Utc::now();

        ideas::insert(
            &pool,
            &Idea {
                id: idea_id,
                title: "P048".into(),
                body: "failed evidence".into(),
                workspace_root_path: None,
                project_key: None,
                status: IdeaStatus::Active,
                created_at: now,
                archived_at: None,
            },
        )
        .await
        .unwrap();
        let run = Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf".into(),
            workflow_title: "Workflow".into(),
            workspace_root: tmp.path().to_string_lossy().into_owned(),
            artifact_root: tmp.path().join("artifacts").to_string_lossy().into_owned(),
            started_at: now,
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
            delivery_configuration_json: None,
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
            chainworks_meta_root: None,
            review_routing_json: None,
            closeout_readiness_mode: None,
        };
        runs::insert(&pool, &run).await.unwrap();
        stages::insert(
            &pool,
            &StageExecution {
                id: stage_execution_id,
                run_id,
                stage_id: "stage_1".into(),
                label: "Stage 1".into(),
                status: StageStatus::Failed,
                iteration: 1,
                attempt_number: 2,
                settlement_kind: Some(StageSettlementKind::Failed),
                started_at: now,
                completed_at: Some(now),
                owner_agent: Some("agent_1".into()),
                provider: Some("system".into()),
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
        agent_executions::insert(
            &pool,
            &AgentExecution {
                id: agent_execution_id,
                stage_execution_id: Some(stage_execution_id),
                agent_id: "agent_1".into(),
                provider: "system".into(),
                model: None,
                started_at: now,
                completed_at: Some(now),
                status: AgentStatus::Failed,
                owner_execution_lineage_id: None,
                session_lineage_id: None,
                session_generation_id: None,
                rehydrated_from_checkpoint_artifact_id: None,
                invocation_owner_key: None,
                session_reuse_scope: None,
                session_family_id: None,
                session_reuse_disposition: None,
                session_reset_reason: None,
                backend_profile_id: None,
                requested_mcp_extensions_json: None,
                predicted_mcp_extensions_json: None,
                predicted_mcp_runtime_ids_json: None,
                actual_mcp_extensions_json: None,
                actual_mcp_runtime_ids_json: None,
                denied_mcp_extensions_json: None,
                mcp_blocking_issues_json: None,
                actual_mcp_observation_json: None,
                actual_xcode_runtime_observation_json: None,
                mcp_session_startup_latency_ms: None,
                owner_kind: None,
                owner_id: None,
                lead_mediation_record_id: None,
                origin_stage_execution_id: None,
                total_cost_cents: None,
                input_tokens: None,
                output_tokens: None,
                cached_input_tokens: None,
                transcript_artifact_id: None,
                actual_toolchain_mapping_diagnostics_json: None,
                escalation_policy_id: None,
                escalation_policy_hash: None,
                escalation_tier_id: None,
                escalation_tier_kind_raw: None,
                escalation_trigger_raw: None,
                escalation_digest_version: None,
                escalation_ledger_id: None,
            },
        )
        .await
        .unwrap();
        stages::update_validation_failure_json(
            &pool,
            stage_execution_id,
            &serde_json::json!({
                "failure_summary": "report: Missing required fields: summary",
                "failure_class": "output_contract_mismatch",
                "raw_output_exists": true,
                "receipt_exists": false,
                "transcript_exists": true,
                "output_results": [{
                    "output_name": "report",
                    "status": "failed",
                    "contract_id": "report_v1",
                    "raw_payload_size": 17
                }]
            })
            .to_string(),
        )
        .await
        .unwrap();
        crate::recovery::persist_failed_stage_recovery_snapshot(&pool, stage_execution_id, now)
            .await
            .unwrap();

        let artifact = build_and_persist_failed_stage_evidence(
            &pool,
            FailedStageEvidenceInput {
                run: &run,
                stage_id: "stage_1",
                stage_execution_id,
                agent_id: "agent_1",
                agent_execution_id,
                provider: "system",
                model: None,
                failed_at: now,
                db_writer: Some(&db_writer),
            },
        )
        .await
        .unwrap();
        let stage = stages::find_by_id(&pool, stage_execution_id)
            .await
            .unwrap()
            .unwrap();
        let packet_pointer: serde_json::Value =
            serde_json::from_str(stage.evidence_packet_json.as_deref().unwrap()).unwrap();
        let refs = db::repos::evidence_spool_refs::list_by_run_id(&pool, &run_id.to_string())
            .await
            .unwrap();
        let full_packet: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&artifact.file_path).unwrap()).unwrap();

        assert_eq!(
            artifact.report_kind.as_deref(),
            Some("failed_stage_evidence")
        );
        assert_eq!(packet_pointer["schema_version"], serde_json::json!(2));
        assert_eq!(
            packet_pointer["storage"],
            serde_json::json!("evidence_spool")
        );
        assert_eq!(refs.len(), 1);
        assert_eq!(refs[0].kind, EvidenceKind::RuntimeEvent);
        assert_eq!(refs[0].status, EvidenceSpoolRefStatus::Available);
        assert_eq!(
            refs[0].relative_path,
            packet_pointer["relative_path"].as_str().unwrap()
        );
        assert!(refs[0]
            .relative_path
            .starts_with(&format!("evidence/runs/{run_id}/")));
        assert_eq!(full_packet["schema_version"], serde_json::json!(1));
        assert!(full_packet["id"].as_str().is_some());
        assert!(full_packet["timestamp"].as_str().is_some());
        assert_eq!(full_packet["stage_label"], serde_json::json!("Stage 1"));
        assert_eq!(full_packet["stage_attempt_number"], serde_json::json!(2));
        assert_eq!(full_packet["raw_outputs_exist"], serde_json::json!(true));
        assert_eq!(full_packet["receipt_exists"], serde_json::json!(false));
        assert_eq!(full_packet["transcript_exists"], serde_json::json!(true));
        assert_eq!(full_packet["output_envelopes"][0]["output_name"], "report");
        assert!(full_packet["recovery_snapshot"].is_object());
    }
}
