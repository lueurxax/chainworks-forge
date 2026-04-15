use anyhow::Result;
use sqlx::SqlitePool;

use db::repos::{artifacts, validation};
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

            Ok(serde_json::Value::Array(reports))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
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
            if let serde_json::Value::Object(ref mut map) = value {
                map.insert(
                    "validation_failure_record".to_string(),
                    serde_json::to_value(ValidationFailureRecordPayload::from(record))?,
                );
            }
        } else if let serde_json::Value::Object(ref mut map) = value {
            map.insert("validation_failure_record".to_string(), serde_json::Value::Null);
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use chrono::Utc;
    use db::pool::create_pool;
    use db::repos::{artifacts, ideas, runs, validation};
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
        create_pool("sqlite::memory:").await.expect("in-memory pool failed")
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
        runs::insert(&pool, &make_run(run_id, idea_id)).await.unwrap();

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
        let names: Vec<String> = reports
            .into_iter()
            .map(|artifact| artifact.name)
            .collect();

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
        runs::insert(&pool, &make_run(run_id, idea_id)).await.unwrap();

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
            &validation_failure_record(
                artifact.id,
                run_id,
                stage_execution_id,
                agent_execution_id,
            ),
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
    }
}
