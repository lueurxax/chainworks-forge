use std::collections::HashMap;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use domain::artifact::ArtifactFormat;
use domain::validation::{
    ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
    ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
};
use workflow::plan::OutputSchema;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeclaredOutput {
    pub output_name: String,
    pub target_path: String,
    pub schema: Option<OutputSchema>,
    pub companion_output_name: Option<String>,
    pub companion_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CapturedOutput {
    pub declared: DeclaredOutput,
    pub machine_bytes: Option<Vec<u8>>,
    pub companion_bytes: Option<Vec<u8>>,
}

#[derive(Clone, Debug)]
pub struct TaskValidationSummary {
    pub output_results: Vec<OutputValidationResult>,
    pub contract_metadata: Vec<ContractValidationMetadata>,
    pub raw_output_exists: bool,
    pub failure_class: Option<ValidationFailureClass>,
    pub failure_summary: Option<String>,
}

pub fn validation_mode(schema: &OutputSchema) -> &str {
    schema.validation_mode.as_deref().unwrap_or("strict_structured")
}

pub fn machine_format(schema: &OutputSchema) -> &str {
    schema.machine_format
        .as_deref()
        .unwrap_or(schema.format.as_str())
}

pub fn human_format(schema: &OutputSchema) -> Option<&str> {
    schema.human_format.as_deref().or_else(|| {
        (validation_mode(schema) == "structured_with_human_companion").then_some("markdown")
    })
}

pub fn artifact_format_for_machine_output(schema: Option<&OutputSchema>) -> ArtifactFormat {
    match schema.map(machine_format) {
        Some("markdown") => ArtifactFormat::Markdown,
        Some("diff") | Some("patch") => ArtifactFormat::Diff,
        Some("report") | Some("text") => ArtifactFormat::Report,
        _ => ArtifactFormat::Json,
    }
}

pub fn artifact_format_for_companion_output(schema: &OutputSchema) -> ArtifactFormat {
    match human_format(schema) {
        Some("markdown") => ArtifactFormat::Markdown,
        Some("json") => ArtifactFormat::Json,
        Some("diff") | Some("patch") => ArtifactFormat::Diff,
        _ => ArtifactFormat::Report,
    }
}

pub fn validate_output(
    output_name: &str,
    content: &[u8],
    schema: Option<&OutputSchema>,
) -> OutputValidationResult {
    let Some(schema) = schema else {
        return OutputValidationResult {
            output_name: output_name.to_string(),
            contract_id: None,
            status: ValidationStatus::NoContractDeclared,
            missing_fields: Vec::new(),
            validation_error: None,
            raw_payload_size: content.len(),
        };
    };

    let mode = validation_mode(schema);
    if mode == "human_only" {
        return OutputValidationResult {
            output_name: output_name.to_string(),
            contract_id: Some(schema.contract_id.clone()),
            status: ValidationStatus::Passed,
            missing_fields: Vec::new(),
            validation_error: None,
            raw_payload_size: content.len(),
        };
    }

    if machine_format(schema) == "json" {
        let parsed = serde_json::from_slice::<serde_json::Value>(content)
            .ok()
            .and_then(|value| value.as_object().cloned());

        let Some(obj) = parsed else {
            return OutputValidationResult {
                output_name: output_name.to_string(),
                contract_id: Some(schema.contract_id.clone()),
                status: ValidationStatus::Failed,
                missing_fields: Vec::new(),
                validation_error: Some("not valid JSON or not a JSON object".to_string()),
                raw_payload_size: content.len(),
            };
        };

        let missing_fields: Vec<String> = schema
            .required_fields
            .iter()
            .filter(|field| !obj.contains_key(field.as_str()))
            .cloned()
            .collect();

        let validation_error = (!missing_fields.is_empty()).then(|| {
            format!("Missing required fields: {}", missing_fields.join(", "))
        });

        OutputValidationResult {
            output_name: output_name.to_string(),
            contract_id: Some(schema.contract_id.clone()),
            status: if missing_fields.is_empty() {
                ValidationStatus::Passed
            } else {
                ValidationStatus::Failed
            },
            missing_fields,
            validation_error,
            raw_payload_size: content.len(),
        }
    } else {
        let is_empty = content.iter().all(|byte| byte.is_ascii_whitespace());
        OutputValidationResult {
            output_name: output_name.to_string(),
            contract_id: Some(schema.contract_id.clone()),
            status: if is_empty {
                ValidationStatus::Failed
            } else {
                ValidationStatus::Passed
            },
            missing_fields: Vec::new(),
            validation_error: is_empty.then_some("output content is empty".to_string()),
            raw_payload_size: content.len(),
        }
    }
}

pub fn validate_task_outputs(captured_outputs: &[CapturedOutput]) -> TaskValidationSummary {
    let mut output_results = Vec::with_capacity(captured_outputs.len());
    let mut contract_metadata = Vec::new();
    let mut raw_output_exists = false;
    let mut failure_class = None;
    let mut failure_summary = None;

    for captured in captured_outputs {
        if captured.machine_bytes.is_some() || captured.companion_bytes.is_some() {
            raw_output_exists = true;
        }

        let schema = captured.declared.schema.as_ref();
        if let Some(schema) = schema {
            contract_metadata.push(ContractValidationMetadata {
                output_name: captured.declared.output_name.clone(),
                contract_id: schema.contract_id.clone(),
                machine_format: machine_format(schema).to_string(),
                validation_mode: validation_mode(schema).to_string(),
                required_field_count: schema.required_fields.len(),
                raw_artifact_name: schema.raw_artifact_name.clone(),
                normalized_artifact_name: schema.normalized_artifact_name.clone(),
            });
        }

        let result = match captured.machine_bytes.as_deref() {
            None => OutputValidationResult {
                output_name: captured.declared.output_name.clone(),
                contract_id: schema.map(|s| s.contract_id.clone()),
                status: ValidationStatus::Failed,
                missing_fields: Vec::new(),
                validation_error: Some("required output was not produced".to_string()),
                raw_payload_size: 0,
            },
            Some(bytes) if bytes.iter().all(|byte| byte.is_ascii_whitespace()) => {
                OutputValidationResult {
                    output_name: captured.declared.output_name.clone(),
                    contract_id: schema.map(|s| s.contract_id.clone()),
                    status: ValidationStatus::Failed,
                    missing_fields: Vec::new(),
                    validation_error: Some("output content is empty".to_string()),
                    raw_payload_size: bytes.len(),
                }
            }
            Some(bytes) => validate_output(&captured.declared.output_name, bytes, schema),
        };

        if failure_class.is_none() && result.status == ValidationStatus::Failed {
            let class = if result.raw_payload_size == 0 && captured.machine_bytes.is_none() {
                ValidationFailureClass::NoOutputProduced
            } else if result.raw_payload_size == 0
                || result.validation_error.as_deref() == Some("output content is empty")
            {
                ValidationFailureClass::EmptyOutput
            } else {
                ValidationFailureClass::OutputContractMismatch
            };
            failure_summary = Some(match &result.validation_error {
                Some(err) => format!("{}: {}", captured.declared.output_name, err),
                None => format!("{} failed validation", captured.declared.output_name),
            });
            failure_class = Some(class);
        }

        if let Some(schema) = schema {
            if validation_mode(schema) == "structured_with_human_companion" {
                let companion_missing = captured
                    .declared
                    .companion_path
                    .as_ref()
                    .is_some_and(|_| captured.companion_bytes.is_none());
                if companion_missing && failure_class.is_none() {
                    failure_class = Some(ValidationFailureClass::NoOutputProduced);
                    let companion_name = captured
                        .declared
                        .companion_output_name
                        .clone()
                        .unwrap_or_else(|| "companion artifact".to_string());
                    failure_summary = Some(format!(
                        "{}: missing required human companion {}",
                        captured.declared.output_name, companion_name
                    ));
                }
            }
        }

        output_results.push(result);
    }

    TaskValidationSummary {
        output_results,
        contract_metadata,
        raw_output_exists,
        failure_class,
        failure_summary,
    }
}

pub fn build_validation_failure_record(
    artifact_id: domain::ids::ArtifactId,
    run_id: domain::ids::RunId,
    stage_id: String,
    stage_execution_id: domain::ids::StageExecutionId,
    agent_id: String,
    agent_execution_id: domain::ids::AgentExecutionId,
    validation: TaskValidationSummary,
    receipt_exists: bool,
    transcript_exists: bool,
) -> Result<ValidationFailureRecord> {
    let failure_class = validation
        .failure_class
        .clone()
        .context("validation failure record requires a failing class")?;
    let failure_summary = validation
        .failure_summary
        .clone()
        .context("validation failure record requires a failure summary")?;

    Ok(ValidationFailureRecord {
        id: uuid::Uuid::new_v4().to_string(),
        artifact_id,
        timestamp: chrono::Utc::now(),
        agent_id,
        stage_id,
        stage_execution_id,
        agent_execution_id,
        run_id,
        output_results: validation.output_results,
        failure_summary,
        failure_class,
        contract_metadata: validation.contract_metadata,
        raw_output_exists: validation.raw_output_exists,
        receipt_exists,
        transcript_exists,
        recovery_recommendation: RecoveryRecommendation {
            action: "retry_failed_agent".to_string(),
            explanation: "Retry the failed agent with the same declared outputs and inspect the validation failure payload.".to_string(),
        },
    })
}

pub fn load_declared_output_bytes(
    declared_outputs: &[DeclaredOutput],
) -> Result<Vec<CapturedOutput>> {
    declared_outputs
        .iter()
        .map(|declared| {
            let machine_bytes = std::fs::read(&declared.target_path).ok();
            let companion_bytes = declared
                .companion_path
                .as_ref()
                .and_then(|path| std::fs::read(path).ok());
            Ok(CapturedOutput {
                declared: declared.clone(),
                machine_bytes,
                companion_bytes,
            })
        })
        .collect()
}

pub fn declared_output_paths(
    declared_outputs: &[DeclaredOutput],
) -> HashMap<String, String> {
    declared_outputs
        .iter()
        .map(|declared| (declared.output_name.clone(), declared.target_path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn structured_schema() -> OutputSchema {
        OutputSchema {
            contract_id: "proposal_review_v1".to_string(),
            format: "json".to_string(),
            human_format: Some("markdown".to_string()),
            machine_format: Some("json".to_string()),
            validation_mode: Some("structured_with_human_companion".to_string()),
            normalized_artifact_name: Some("proposal_review".to_string()),
            raw_artifact_name: Some("proposal_review_raw".to_string()),
            required_fields: vec!["summary".to_string(), "status".to_string()],
        }
    }

    #[test]
    fn validate_output_rejects_missing_required_fields() {
        let schema = structured_schema();
        let result = validate_output("proposal_review", br#"{"summary":"ok"}"#, Some(&schema));
        assert_eq!(result.status, ValidationStatus::Failed);
        assert_eq!(result.missing_fields, vec!["status"]);
    }

    #[test]
    fn validate_task_outputs_requires_companion_for_companion_mode() {
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: "/tmp/proposal_review.json".to_string(),
            schema: Some(structured_schema()),
            companion_output_name: Some("proposal_review_raw".to_string()),
            companion_path: Some("/tmp/proposal_review.md".to_string()),
        };
        let captured = CapturedOutput {
            declared,
            machine_bytes: Some(br#"{"summary":"ok","status":"green"}"#.to_vec()),
            companion_bytes: None,
        };
        let summary = validate_task_outputs(&[captured]);
        assert_eq!(
            summary.failure_class,
            Some(ValidationFailureClass::NoOutputProduced)
        );
    }
}
