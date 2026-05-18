use async_graphql::*;
use db::repos::projections::ArtifactIndexRow;
use db::repos::validation;
use domain::artifact::Artifact;
use domain::validation::ValidationFailureRecord;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

use crate::types::p031::{
    is_report_metadata, GqlFreshnessState, GqlPayloadAvailabilityState,
    GqlPayloadUnavailableReasonCode,
};
use crate::types::stage::AgentOutputSettlement;

pub const P085_NO_DEADLINE_JUSTIFICATION: &str =
    "No server staleness deadline applies: this is a bounded read projection state, not an active payload-generation job.";

#[derive(SimpleObject, Clone, Debug)]
#[graphql(complex)]
pub struct GqlArtifact {
    pub id: ID,
    pub run_id: ID,
    pub stage_id: String,
    pub agent_id: String,
    pub name: String,
    pub contract_id: String,
    pub format: String,
    pub artifact_metadata_pointer: Json<serde_json::Value>,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub provider: String,
    pub model: Option<String>,
    pub created_at: String,
    pub is_pinned: bool,
    pub report_kind: Option<String>,
    pub report_version: Option<i64>,
    pub artifact_generation_id: Option<String>,
    pub source_agent_execution_id: Option<String>,
    pub source_stage_execution_id: Option<String>,
    pub source_session_generation_id: Option<String>,
    pub source_work_item_id: Option<String>,
    pub supersedes_artifact_generation_id: Option<String>,
    pub output_settlement: Option<AgentOutputSettlement>,
    pub source_generation_verified: Option<bool>,
    pub freshness_state: GqlFreshnessState,
    pub payload_availability_state: GqlPayloadAvailabilityState,
    pub payload_unavailable_reason_code: Option<GqlPayloadUnavailableReasonCode>,
    pub payload_text: Option<String>,
    pub diagnostic_id: Option<String>,
    pub server_debug_detail: Option<String>,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[graphql(rename_fields = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct GqlValidationFailureRecord {
    pub id: String,
    pub timestamp: String,
    #[graphql(name = "agentID")]
    #[serde(rename = "agentID")]
    pub agent_id: String,
    #[graphql(name = "stageID")]
    #[serde(rename = "stageID")]
    pub stage_id: String,
    #[graphql(name = "runID")]
    #[serde(rename = "runID")]
    pub run_id: String,
    pub output_results: Vec<GqlOutputValidationResult>,
    pub failure_summary: String,
    pub failure_class: String,
    pub contract_metadata: Vec<GqlContractValidationMetadata>,
    pub raw_output_exists: bool,
    pub receipt_exists: bool,
    pub transcript_exists: bool,
    pub recovery_recommendation: GqlRecoveryRecommendation,
    pub session_reuse_disposition: Option<String>,
    pub session_reset_reason: Option<String>,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[graphql(rename_fields = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct GqlOutputValidationResult {
    pub output_name: String,
    #[graphql(name = "contractID")]
    #[serde(rename = "contractID")]
    pub contract_id: Option<String>,
    pub status: String,
    pub missing_fields: Vec<String>,
    pub validation_error: Option<String>,
    pub raw_payload_size: i64,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[graphql(rename_fields = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct GqlContractValidationMetadata {
    pub output_name: String,
    #[graphql(name = "contractID")]
    #[serde(rename = "contractID")]
    pub contract_id: String,
    pub machine_format: String,
    pub validation_mode: String,
    pub required_field_count: i64,
    pub raw_artifact_name: Option<String>,
    pub normalized_artifact_name: Option<String>,
}

#[derive(SimpleObject, Clone, Debug, Serialize, Deserialize, PartialEq)]
#[graphql(rename_fields = "camelCase")]
#[serde(rename_all = "camelCase")]
pub struct GqlRecoveryRecommendation {
    pub action: String,
    pub explanation: String,
    pub source: Option<String>,
}

impl From<Artifact> for GqlArtifact {
    fn from(a: Artifact) -> Self {
        let artifact_id = a.id.to_string();
        let is_report = is_report_metadata(&a.format.to_string(), a.report_kind.as_deref());
        let checksum_sha256 = a.checksum_sha256;
        let size_bytes = a.size_bytes;
        GqlArtifact {
            id: ID(artifact_id.clone()),
            run_id: ID(a.run_id.to_string()),
            stage_id: a.stage_id,
            agent_id: a.agent_id,
            name: a.name,
            contract_id: a.contract_id,
            format: a.format.to_string(),
            artifact_metadata_pointer: Json(artifact_metadata_pointer(
                &artifact_id,
                checksum_sha256.as_deref(),
                size_bytes,
            )),
            checksum_sha256,
            size_bytes,
            provider: a.provider,
            model: a.model,
            created_at: a.created_at.to_rfc3339(),
            is_pinned: a.is_pinned,
            report_kind: a.report_kind,
            report_version: a.report_version,
            artifact_generation_id: None,
            source_agent_execution_id: None,
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_artifact_generation_id: None,
            output_settlement: None,
            source_generation_verified: None,
            freshness_state: GqlFreshnessState::Live,
            payload_availability_state: if is_report {
                GqlPayloadAvailabilityState::MetadataOnly
            } else {
                GqlPayloadAvailabilityState::Available
            },
            payload_unavailable_reason_code: if is_report {
                Some(GqlPayloadUnavailableReasonCode::PayloadDeferredByP031)
            } else {
                None
            },
            payload_text: None,
            diagnostic_id: is_report.then_some(artifact_id),
            server_debug_detail: is_report.then_some(format!(
                "P031 exposes report metadata only; payload rendering is deferred. {P085_NO_DEADLINE_JUSTIFICATION}"
            )),
        }
    }
}

impl From<ArtifactIndexRow> for GqlArtifact {
    fn from(r: ArtifactIndexRow) -> Self {
        let output_settlement = r
            .output_settlement
            .as_deref()
            .and_then(output_settlement_from_db);
        let is_report = is_report_metadata(&r.format, r.report_kind.as_deref());
        let checksum_sha256 = r.checksum_sha256;
        let size_bytes = r.size_bytes;
        GqlArtifact {
            id: ID(r.id.clone()),
            run_id: ID(r.run_id),
            stage_id: r.stage_id,
            agent_id: r.agent_id,
            name: r.name,
            contract_id: r.contract_id,
            format: r.format,
            artifact_metadata_pointer: Json(artifact_metadata_pointer(
                &r.id,
                checksum_sha256.as_deref(),
                size_bytes,
            )),
            checksum_sha256,
            size_bytes,
            provider: r.provider,
            model: r.model,
            created_at: r.created_at,
            is_pinned: r.is_pinned,
            report_kind: r.report_kind,
            report_version: r.report_version,
            artifact_generation_id: r.artifact_generation_id,
            source_agent_execution_id: r.source_agent_execution_id,
            source_stage_execution_id: r.source_stage_execution_id,
            source_session_generation_id: r.source_session_generation_id,
            source_work_item_id: r.source_work_item_id,
            supersedes_artifact_generation_id: r.supersedes_artifact_generation_id,
            output_settlement,
            source_generation_verified: r.source_generation_verified,
            freshness_state: GqlFreshnessState::Live,
            payload_availability_state: if is_report {
                GqlPayloadAvailabilityState::MetadataOnly
            } else {
                GqlPayloadAvailabilityState::Available
            },
            payload_unavailable_reason_code: if is_report {
                Some(GqlPayloadUnavailableReasonCode::PayloadDeferredByP031)
            } else {
                None
            },
            payload_text: None,
            diagnostic_id: is_report.then_some(r.id),
            server_debug_detail: is_report.then_some(format!(
                "P031 exposes report metadata only; payload rendering is deferred. {P085_NO_DEADLINE_JUSTIFICATION}"
            )),
        }
    }
}

fn artifact_metadata_pointer(
    artifact_id: &str,
    checksum_sha256: Option<&str>,
    size_bytes: Option<i64>,
) -> serde_json::Value {
    serde_json::json!({
        "schemaVersion": "artifact_metadata_pointer.v1",
        "artifactId": artifact_id,
        "checksumSha256": checksum_sha256,
        "sizeBytes": size_bytes,
        "authorizedPayloadRoute": format!("/artifacts/{artifact_id}/payload"),
        "payloadPathRedacted": true,
        "forbiddenFields": ["absolutePath", "filesystemPath", "rawPayload"]
    })
}

fn output_settlement_from_db(value: &str) -> Option<AgentOutputSettlement> {
    match value {
        "none" => Some(AgentOutputSettlement::None),
        "missing_required_outputs" => Some(AgentOutputSettlement::MissingRequiredOutputs),
        "invalid_required_outputs" => Some(AgentOutputSettlement::InvalidRequiredOutputs),
        "valid_outputs_from_completed_execution" => {
            Some(AgentOutputSettlement::ValidOutputsFromCompletedExecution)
        }
        "valid_outputs_from_failed_execution" => {
            Some(AgentOutputSettlement::ValidOutputsFromFailedExecution)
        }
        "ignored_late_outputs" => Some(AgentOutputSettlement::IgnoredLateOutputs),
        _ => None,
    }
}

#[ComplexObject]
impl GqlArtifact {
    async fn validation_failure_record(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Option<GqlValidationFailureRecord>> {
        if self.report_kind.as_deref() != Some("validation_failure") {
            return Ok(None);
        }

        let pool = ctx.data::<SqlitePool>()?;
        let artifact_id: domain::ids::ArtifactId = self
            .id
            .parse()
            .map_err(|e: uuid::Error| Error::new(e.to_string()))?;
        let record = validation::find_by_artifact_id(pool, artifact_id).await?;
        if let Some(record) = record {
            let mut gql_record: GqlValidationFailureRecord = record.clone().into();
            if let Some(execution) =
                db::repos::agent_executions::find_by_id(pool, record.agent_execution_id).await?
            {
                gql_record.session_reuse_disposition = execution.session_reuse_disposition;
                gql_record.session_reset_reason = execution.session_reset_reason;
            }
            Ok(Some(gql_record))
        } else {
            Ok(None)
        }
    }
}

impl From<ValidationFailureRecord> for GqlValidationFailureRecord {
    fn from(record: ValidationFailureRecord) -> Self {
        GqlValidationFailureRecord {
            id: record.id,
            timestamp: record.timestamp.to_rfc3339(),
            agent_id: record.agent_id,
            stage_id: record.stage_id,
            run_id: record.run_id.to_string(),
            output_results: record
                .output_results
                .into_iter()
                .map(|output| GqlOutputValidationResult {
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
                .map(|meta| GqlContractValidationMetadata {
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
            recovery_recommendation: GqlRecoveryRecommendation {
                action: record.recovery_recommendation.action,
                explanation: record.recovery_recommendation.explanation,
                source: None,
            },
            session_reuse_disposition: None,
            session_reset_reason: None,
        }
    }
}
