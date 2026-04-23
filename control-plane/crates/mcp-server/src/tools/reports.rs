use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashMap;

use db::repos::{
    agent_execution_discovery_diagnostics, agent_execution_runtime_facts, agent_executions,
    artifact_contracts, artifacts, legacy_discovery_overrides, sessions, validation,
    workflow_conflicts,
};
use domain::agent::{AgentExecution, AgentExecutionRuntimeFacts};
use domain::artifact::Artifact;
use domain::ids::RunId;
use engine::command_handler::CommandHandler;

use crate::protocol::McpTool;

fn fresh_provider_process_for_disposition(disposition: Option<&str>) -> Option<bool> {
    match disposition {
        Some("reused") => Some(false),
        Some("fresh")
        | Some("reused_after_resume")
        | Some("fresh_after_reset")
        | Some("fresh_after_invalidation")
        | Some("fresh_after_budget")
        | Some("fresh_after_compaction")
        | Some("fresh_after_transport_error")
        | Some("fresh_after_timeout")
        | Some("fresh_session_required")
        | Some("unverifiable_session_history") => Some(true),
        Some(_) | None => None,
    }
}

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
    principal: &auth::Principal,
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
                "agent_executions": execution_mcp_truth_json(
                    pool,
                    run_id,
                    principal.class == auth::PrincipalClass::Operator,
                )
                .await?,
                "workflow_conflict": workflow_conflict_json(pool, run_id).await?,
                "implementation_handoff_status": implementation_handoff_status_json(pool, run_id).await?,
                "implementation_self_assessment_summary": implementation_self_assessment_summary_json(pool, run_id).await?,
            }));
            if let Some(projection) =
                db::repos::artifact_contracts::find_run_state_projection(pool, run_id).await?
            {
                let overrides = db::repos::artifact_contracts::list_overrides(pool, run_id).await?;
                reports.push(serde_json::json!({
                    "id": uuid::Uuid::new_v4().to_string(),
                    "run_id": run_id.to_string(),
                    "stage_id": "__run__",
                    "agent_id": "system",
                    "name": "canonical_artifact_contracts",
                    "contract_id": "canonical_artifact_contracts",
                    "format": "json",
                    "file_path": projection.exported_active_index_path,
                    "checksum_sha256": serde_json::Value::Null,
                    "size_bytes": serde_json::Value::Null,
                    "provider": "system",
                    "model": serde_json::Value::Null,
                    "created_at": projection.updated_at.to_rfc3339(),
                    "is_pinned": true,
                    "report_kind": "canonical_artifact_contracts",
                    "report_version": 1,
                    "active_index": projection.active_index_json,
                    "run_state_projection": projection.run_state_json,
                    "operator_overrides": overrides,
                    "legacy_discovery_overrides": legacy_discovery_overrides::list_by_run(pool, run_id).await?,
                }));
            }

            Ok(serde_json::Value::Array(reports))
        }

        _ => Err(anyhow::anyhow!("Unknown tool: {tool_name}")),
    }
}

pub(crate) async fn workflow_conflict_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    match workflow_conflicts::get_current_blocking_conflict(pool, run_id).await? {
        Some(conflict) => Ok(serde_json::to_value(conflict)?),
        None => Ok(serde_json::Value::Null),
    }
}

pub(crate) async fn implementation_handoff_status_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    match workflow_conflicts::get_implementation_handoff_status(pool, run_id).await? {
        Some(status) => Ok(serde_json::to_value(status)?),
        None => Ok(serde_json::Value::Null),
    }
}

pub(crate) async fn execution_mcp_truth_json(
    pool: &SqlitePool,
    run_id: RunId,
    include_operator_debug: bool,
) -> Result<serde_json::Value> {
    let executions = agent_executions::list_by_run(pool, run_id).await?;
    let runtime_facts = agent_execution_runtime_facts::list_by_run(pool, run_id).await?;
    let runtime_facts_by_execution_id: HashMap<_, _> = runtime_facts
        .into_iter()
        .map(|facts| {
            let execution_id = facts.agent_execution_id.to_string();
            (execution_id, facts)
        })
        .collect::<HashMap<_, _>>();
    let discovery_diagnostics =
        agent_execution_discovery_diagnostics::list_readback_by_run(pool, run_id).await?;
    let discovery_diagnostics_by_execution_id: HashMap<_, _> = discovery_diagnostics
        .into_iter()
        .map(|readback| (readback.diagnostics.agent_execution_id.clone(), readback))
        .collect::<HashMap<_, _>>();
    let mut items = Vec::with_capacity(executions.len());
    for execution in executions.into_iter() {
        let execution_id = execution.id.to_string();
        let discovery_diagnostics_readback =
            discovery_diagnostics_by_execution_id.get(&execution_id);
        let reconciliation_pending =
            discovery_diagnostics_readback.is_some_and(|readback| readback.reconciliation_pending);
        let runtime_facts = match runtime_facts_by_execution_id.get(&execution_id) {
            Some(facts) => {
                let mut facts = facts.clone();
                if reconciliation_pending {
                    facts.valid_required_outputs = false;
                }
                runtime_facts_json(pool, &execution, &facts, include_operator_debug).await?
            }
            None => {
                let mut facts =
                    AgentExecutionRuntimeFacts::defaults_for(execution.id, chrono::Utc::now());
                if reconciliation_pending {
                    facts.valid_required_outputs = false;
                }
                runtime_facts_json(pool, &execution, &facts, include_operator_debug).await?
            }
        };
        let discovery_diagnostics = match discovery_diagnostics_readback {
            Some(readback) => {
                let diagnostics = &readback.diagnostics;
                serde_json::json!({
                "agent_execution_id": diagnostics.agent_execution_id.clone(),
                "discovery_schema_version": diagnostics.discovery_schema_version.clone(),
                "legacy_broad_discovery_used": diagnostics.legacy_broad_discovery_used,
                "missing_required_output_count": diagnostics.missing_required_output_count,
                "rejected_output_count": diagnostics.rejected_output_count,
                "stale_output_count": diagnostics.stale_output_count,
                "meta_discovery_truncated": diagnostics.meta_discovery_truncated,
                "git_manifest_status": diagnostics.git_manifest_status.clone(),
                "resume_warning_count": diagnostics.resume_warning_count,
                "reconciliation_pending": readback.reconciliation_pending,
                "reconciliation_warnings": readback.reconciliation_warnings.clone(),
                "runtime_facts_present": readback.runtime_facts_present,
                "matching_active_artifact_generation_count": readback.matching_active_artifact_generation_count,
                "payload": readback.projected_payload(),
                "created_at": diagnostics.created_at.to_rfc3339(),
                "updated_at": diagnostics.updated_at.to_rfc3339(),
                })
            }
            None => serde_json::Value::Null,
        };
        items.push(serde_json::json!({
            "agent_execution_id": execution_id,
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
            "runtime_facts": runtime_facts,
            "discovery_diagnostics": discovery_diagnostics,
        }));
    }
    Ok(serde_json::Value::Array(items))
}

async fn runtime_facts_json(
    pool: &SqlitePool,
    execution: &AgentExecution,
    facts: &AgentExecutionRuntimeFacts,
    include_operator_debug: bool,
) -> Result<serde_json::Value> {
    let lineage = match execution.session_lineage_id.as_deref() {
        Some(lineage_id) => sessions::find_lineage_by_id(pool, lineage_id).await?,
        None => None,
    };
    let generation = match execution.session_generation_id.as_deref() {
        Some(generation_id) => sessions::find_generation_by_id(pool, generation_id).await?,
        None => None,
    };
    let provider_session_id = generation
        .as_ref()
        .and_then(|generation| generation.provider_session_id.clone());
    let generation_status = generation.as_ref().map(|generation| {
        sessions::session_generation_status_to_str(&generation.status).to_string()
    });
    let active_session_generation_id = lineage
        .as_ref()
        .and_then(|lineage| lineage.active_generation_id.clone());
    let active_generation_matches_execution =
        match (lineage.as_ref(), execution.session_generation_id.as_deref()) {
            (Some(lineage), Some(execution_generation_id)) => {
                Some(lineage.active_generation_id.as_deref() == Some(execution_generation_id))
            }
            _ => None,
        };
    Ok(serde_json::json!({
        "agent_execution_id": facts.agent_execution_id.to_string(),
        "failure_kind": facts.failure_kind.as_ref().map(ToString::to_string),
        "failure_kind_raw_debug": include_operator_debug.then(|| facts.failure_kind_raw_debug.clone()).flatten(),
        "failure_kind_version": facts.failure_kind_version,
        "failure_message_redacted": facts.failure_message_redacted.clone(),
        "failure_message_redaction_version": facts.failure_message_redaction_version,
        "retry_after": facts.retry_after.as_ref().map(|dt| dt.to_rfc3339()),
        "operator_action_hint": facts.operator_action_hint.as_ref().map(ToString::to_string),
        "provider_exit_status": facts.provider_exit_status,
        "transport_error_code": facts.transport_error_code.clone(),
        "supervision_classification": facts.supervision_classification.clone(),
        "output_settlement": facts.output_settlement.to_string(),
        "valid_required_outputs": facts.valid_required_outputs,
        "late_output_count": facts.late_output_count,
        "ignored_late_output_count": facts.ignored_late_output_count,
        "session_lineage_id": execution.session_lineage_id.clone(),
        "session_generation_id": execution.session_generation_id.clone(),
        "invocation_owner_key": execution.invocation_owner_key.clone(),
        "session_reuse_scope": execution.session_reuse_scope.clone(),
        "session_family_id": execution.session_family_id.clone(),
        "session_reuse_disposition": execution.session_reuse_disposition.clone(),
        "session_reuse_reason": facts.session_reuse_reason.clone(),
        "session_reset_reason": execution.session_reset_reason.clone(),
        "provider_session_id": provider_session_id,
        "active_session_generation_id": active_session_generation_id,
        "active_generation_matches_execution": active_generation_matches_execution,
        "generation_status": generation_status,
        "fresh_provider_process": fresh_provider_process_for_disposition(execution.session_reuse_disposition.as_deref()),
        "rehydrated_from_checkpoint_artifact_id": execution.rehydrated_from_checkpoint_artifact_id.clone(),
        "quota_ledger_id": facts.quota_ledger_id.clone(),
        "created_at": facts.created_at.to_rfc3339(),
        "updated_at": facts.updated_at.to_rfc3339(),
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

pub(crate) async fn implementation_self_assessment_summary_json(
    pool: &SqlitePool,
    run_id: RunId,
) -> Result<serde_json::Value> {
    artifact_contracts::find_active_implementation_self_assessment_summary(pool, run_id)
        .await?
        .map(|stored| {
            let mut summary = stored.summary;
            summary.artifact_path = public_artifact_path(&summary.artifact_path);
            serde_json::to_value(summary)
        })
        .transpose()
        .map(|summary| summary.unwrap_or(serde_json::Value::Null))
        .map_err(Into::into)
}

pub(crate) fn public_artifact_path(path: &str) -> String {
    if path.ends_with("implementation/self-assessment.json") {
        "implementation/self-assessment.json".to_string()
    } else {
        path.to_string()
    }
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
    use db::repos::{artifact_contracts, artifacts, ideas, runs, validation, workflow_conflicts};
    use domain::artifact::{Artifact, ArtifactFormat};
    use domain::artifact_contracts::{
        parse_implementation_self_assessment_v2, ContractParseContext,
        IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
        IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
    };
    use domain::idea::{Idea, IdeaStatus};
    use domain::ids::{ArtifactId, IdeaId, RunId};
    use domain::validation::{
        ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
        ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
    };
    use domain::workflow_conflict::{
        candidate_transition_hash, workflow_conflict_fingerprint, CandidateTransitionEvaluation,
        CandidateTransitionResult, WorkflowConflictReason, WorkflowConflictRecord,
        WorkflowConflictStatus,
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

    fn test_principal() -> auth::Principal {
        auth::Principal::new("test-operator", auth::PrincipalClass::Operator)
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
            chainworks_meta_root: None,
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

    fn make_workflow_conflict(run_id: RunId) -> WorkflowConflictRecord {
        let candidates = vec![CandidateTransitionEvaluation {
            transition_id: "review_to_complete".into(),
            from_state_id: "review".into(),
            to_state_id: "complete".into(),
            condition_expression_id: Some("proposal_review_summary.pass == true".into()),
            result: CandidateTransitionResult::MissingInput,
            required_artifacts: vec!["proposal_review_summary".into()],
            missing_artifacts: vec!["proposal_review_summary".into()],
            missing_fields: vec![],
            source_artifact_ids: vec![],
            source_agent_execution_id: None,
            sanitized_diagnostic: Some("proposal_review_summary is required".into()),
        }];
        let reason = WorkflowConflictReason::RequiredArtifactOrFieldMissingForTransition;
        let candidate_hash = candidate_transition_hash(&candidates);
        WorkflowConflictRecord {
            conflict_id: uuid::Uuid::new_v4().to_string(),
            conflict_fingerprint: workflow_conflict_fingerprint(
                &run_id.to_string(),
                "review",
                &reason,
                &candidate_hash,
                &[],
            ),
            run_id: run_id.to_string(),
            stage_execution_id: None,
            lineage_id: Some("lineage-p017".into()),
            current_state_id: "review".into(),
            reason,
            operator_label: "Required transition input is missing".into(),
            status: WorkflowConflictStatus::Unresolved,
            candidate_transitions: candidates,
            candidate_transition_hash: candidate_hash,
            advisory_evidence_refs: vec![],
            lead_agent_id: None,
            mediation_record_id: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
            resolved_at: None,
            superseded_by_conflict_id: None,
            resolution_record_json: None,
            terminal_failure_reason: None,
            diagnostic_redaction_tier: "operator_safe".into(),
        }
    }

    async fn persist_handoff_required_summary(pool: &sqlx::SqlitePool, run_id: RunId) {
        let artifact = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "state_8_implementation_continued".into(),
            agent_id: "code_writer".into(),
            name: "implementation_self_assessment".into(),
            contract_id: IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/implementation/self-assessment.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "test".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        artifacts::insert(pool, &artifact).await.unwrap();
        let raw = serde_json::json!({
            "contract_id": IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID,
            "implementation_complete": true,
            "verification_green": true,
            "remaining_code_tasks": [],
            "handoff_tasks": [{
                "summary": "Capture release note",
                "owner_class": "release",
                "target_stage": "state_11_manual_release",
                "blocking_review": true,
                "evidence": "release owner must attach the note before go/no-go"
            }],
            "known_risks": [],
            "tests_run": ["proposal-054: green"],
            "docs_impacted": []
        });
        let summary = parse_implementation_self_assessment_v2(
            &raw,
            ContractParseContext {
                run_id: run_id.to_string(),
                run_age: None,
                declared_contract_id: Some(IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.into()),
                canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH.into(),
                raw_artifact_path: Some(artifact.file_path.clone()),
                source_generation_id: None,
                artifact_created_at: Some(artifact.created_at),
                v2_generation_seen_for_run: true,
                legacy_v1_generation_available: false,
            },
        );
        artifact_contracts::persist_implementation_self_assessment_summary(
            pool,
            run_id,
            artifact.id,
            &artifact.contract_id,
            &summary,
            artifact.created_at,
        )
        .await
        .unwrap();
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
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
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
    async fn reports_get_includes_implementation_self_assessment_summary() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        persist_handoff_required_summary(&pool, run_id).await;

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let summary = &mcp_truth["implementation_self_assessment_summary"];

        assert_eq!(summary["status"], serde_json::json!("handoff_required"));
        assert_eq!(
            summary["handoff_tasks"][0]["summary"],
            serde_json::json!("Capture release note")
        );
        assert_eq!(
            summary["owner_class_counts"]["release"],
            serde_json::json!(1)
        );
    }

    #[tokio::test]
    async fn proposal_017_reports_get_includes_current_workflow_conflict() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        let conflict = make_workflow_conflict(run_id);
        workflow_conflicts::upsert_conflict_by_fingerprint(&pool, &conflict)
            .await
            .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();

        let reports = result.as_array().expect("reports array");
        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let workflow_conflict = &mcp_truth["workflow_conflict"];

        assert_eq!(
            workflow_conflict["reason"],
            serde_json::json!("required_artifact_or_field_missing_for_transition")
        );
        assert_eq!(workflow_conflict["status"], serde_json::json!("unresolved"));
        assert_eq!(
            workflow_conflict["candidate_transitions"][0]["result"],
            serde_json::json!("missing_input")
        );
        assert_eq!(
            workflow_conflict["current_state_id"],
            serde_json::json!("review")
        );
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
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
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
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
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
    async fn proposal_041_reports_get_readback_parity_surface() {
        let pool = test_pool().await;
        let idea_id = IdeaId::new();
        let run_id = RunId::new();
        ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
        runs::insert(&pool, &make_run(run_id, idea_id))
            .await
            .unwrap();
        seed_validation_attempt(&pool, run_id).await;
        artifacts::insert(
            &pool,
            &make_artifact(run_id, "p041_operator_report", Some("operator_summary")),
        )
        .await
        .unwrap();

        let handler = make_command_handler(pool.clone());
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
        )
        .await
        .unwrap();
        let reports = result.as_array().expect("reports array");
        assert!(reports.iter().any(|report| {
            report["name"] == serde_json::json!("p041_operator_report")
                && report["report_kind"] == serde_json::json!("operator_summary")
        }));

        let mcp_truth = reports
            .iter()
            .find(|report| report["report_kind"] == serde_json::json!("mcp_execution_truth"))
            .expect("mcp execution truth report");
        let execution = &mcp_truth["agent_executions"][0];
        assert_eq!(
            execution["actual_mcp_runtime_ids_json"],
            serde_json::json!(r#"["fs-runtime"]"#)
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
        let principal = test_principal();
        let result = execute(
            "reports.get",
            serde_json::json!({ "run_id": run_id.to_string() }),
            &pool,
            &handler,
            &principal,
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
