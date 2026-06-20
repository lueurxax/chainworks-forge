use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentExecutionId, ArtifactId, RunId, StageExecutionId};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationStatus {
    Passed,
    Failed,
    NoContractDeclared,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OutputValidationResult {
    pub output_name: String,
    pub contract_id: Option<String>,
    pub status: ValidationStatus,
    pub missing_fields: Vec<String>,
    pub validation_error: Option<String>,
    pub raw_payload_size: usize,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationFailureClass {
    OutputContractMismatch,
    NoOutputProduced,
    EmptyOutput,
    PersistenceFailure,
    MissingRequiredOutputs,
    InvalidRequiredOutputs,
    ProviderModeMismatch,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ContractValidationMetadata {
    pub output_name: String,
    pub contract_id: String,
    pub machine_format: String,
    pub validation_mode: String,
    pub required_field_count: usize,
    pub raw_artifact_name: Option<String>,
    pub normalized_artifact_name: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RecoveryRecommendation {
    pub action: String,
    pub explanation: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ValidationFailureRecord {
    pub id: String,
    pub artifact_id: ArtifactId,
    pub timestamp: DateTime<Utc>,
    pub agent_id: String,
    pub stage_id: String,
    pub stage_execution_id: StageExecutionId,
    pub agent_execution_id: AgentExecutionId,
    pub run_id: RunId,
    pub output_results: Vec<OutputValidationResult>,
    pub failure_summary: String,
    pub failure_class: ValidationFailureClass,
    pub contract_metadata: Vec<ContractValidationMetadata>,
    pub raw_output_exists: bool,
    pub receipt_exists: bool,
    pub transcript_exists: bool,
    pub recovery_recommendation: RecoveryRecommendation,
}
