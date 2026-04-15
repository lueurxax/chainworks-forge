use async_graphql::*;
use domain::artifact::Artifact;
use domain::validation::ValidationFailureRecord;
use db::repos::projections::ArtifactIndexRow;
use db::repos::validation;
use serde::{Deserialize, Serialize};
use sqlx::SqlitePool;

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
    pub file_path: String,
    pub checksum_sha256: Option<String>,
    pub size_bytes: Option<i64>,
    pub provider: String,
    pub model: Option<String>,
    pub created_at: String,
    pub is_pinned: bool,
    pub report_kind: Option<String>,
    pub report_version: Option<i64>,
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
        GqlArtifact {
            id: ID(a.id.to_string()),
            run_id: ID(a.run_id.to_string()),
            stage_id: a.stage_id,
            agent_id: a.agent_id,
            name: a.name,
            contract_id: a.contract_id,
            format: a.format.to_string(),
            file_path: a.file_path,
            checksum_sha256: a.checksum_sha256,
            size_bytes: a.size_bytes,
            provider: a.provider,
            model: a.model,
            created_at: a.created_at.to_rfc3339(),
            is_pinned: a.is_pinned,
            report_kind: a.report_kind,
            report_version: a.report_version,
        }
    }
}

impl From<ArtifactIndexRow> for GqlArtifact {
    fn from(r: ArtifactIndexRow) -> Self {
        GqlArtifact {
            id: ID(r.id),
            run_id: ID(r.run_id),
            stage_id: r.stage_id,
            agent_id: r.agent_id,
            name: r.name,
            contract_id: r.contract_id,
            format: r.format,
            file_path: r.file_path,
            checksum_sha256: r.checksum_sha256,
            size_bytes: r.size_bytes,
            provider: r.provider,
            model: r.model,
            created_at: r.created_at,
            is_pinned: r.is_pinned,
            report_kind: r.report_kind,
            report_version: r.report_version,
        }
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
        Ok(record.map(Into::into))
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
        }
    }
}
