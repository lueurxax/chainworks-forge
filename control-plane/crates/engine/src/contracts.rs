use std::collections::HashMap;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use domain::artifact::ArtifactFormat;
use domain::artifact_contracts::{
    contract_status_allowed_values, parse_implementation_self_assessment_v2,
    proposal_review_summary_v2_validation_error, ContractParseContext,
    ImplementationSelfAssessmentStatus, IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH,
    IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID, PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID,
};
use domain::commands::{KnowledgeCapsulesMode, MainSyncMode};
use domain::discovery::{
    AuthorizedRoot, ExpectedOutputRole, ExpectedOutputSpec, OutputDiscoveryDecision,
    OutputDiscoveryStatus, OutputReusePolicy, OutputRootClass, SourceGenerationOwner,
};
use domain::validation::{
    ContractValidationMetadata, OutputValidationResult, RecoveryRecommendation,
    ValidationFailureClass, ValidationFailureRecord, ValidationStatus,
};
use workflow::plan::OutputSchema;

pub use domain::artifact_contracts::{
    normalize_contract_status, DegradedOutputPolicy, DegradedOutputPolicyMode,
    FailedExecutionSettlement,
};

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DeclaredOutput {
    pub output_name: String,
    pub target_path: String,
    pub schema: Option<OutputSchema>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reuse_policy: Option<OutputReusePolicy>,
    pub companion_output_name: Option<String>,
    pub companion_path: Option<String>,
}

#[derive(Clone, Debug)]
pub struct CapturedOutput {
    pub declared: DeclaredOutput,
    pub machine_bytes: Option<Vec<u8>>,
    pub companion_bytes: Option<Vec<u8>>,
}

const DEFAULT_EXPECTED_OUTPUT_MAX_BYTES: u64 = 10 * 1024 * 1024;
const DEFAULT_AGGREGATE_ACCEPTANCE_CAP_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct Proposal064RuntimeConfig {
    #[serde(default)]
    pub main_sync: MainSyncRuntimeSection,
    #[serde(default)]
    pub knowledge_capsules: KnowledgeCapsulesRuntimeSection,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct MainSyncRuntimeSection {
    #[serde(default)]
    pub mode: MainSyncMode,
}

impl Default for MainSyncRuntimeSection {
    fn default() -> Self {
        Self {
            mode: MainSyncMode::Off,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct KnowledgeCapsulesRuntimeSection {
    #[serde(default)]
    pub mode: KnowledgeCapsulesMode,
}

impl Default for KnowledgeCapsulesRuntimeSection {
    fn default() -> Self {
        Self {
            mode: KnowledgeCapsulesMode::Off,
        }
    }
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
    schema
        .validation_mode
        .as_deref()
        .unwrap_or("strict_structured")
}

pub fn machine_format(schema: &OutputSchema) -> &str {
    schema
        .machine_format
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

        if schema.contract_id == IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID {
            let summary = parse_implementation_self_assessment_v2(
                &serde_json::Value::Object(obj.clone()),
                ContractParseContext {
                    run_id: String::new(),
                    run_age: None,
                    declared_contract_id: Some(schema.contract_id.clone()),
                    canonical_artifact_path: IMPLEMENTATION_SELF_ASSESSMENT_ARTIFACT_PATH
                        .to_string(),
                    raw_artifact_path: None,
                    source_generation_id: None,
                    artifact_created_at: None,
                    v2_generation_seen_for_run: true,
                    legacy_v1_generation_available: false,
                },
            );
            if summary.status == ImplementationSelfAssessmentStatus::Invalid {
                let validation_error = summary
                    .validation_errors
                    .iter()
                    .map(|issue| {
                        if issue.pointer.is_empty() {
                            issue.message.clone()
                        } else {
                            format!("{}: {}", issue.pointer, issue.message)
                        }
                    })
                    .collect::<Vec<_>>()
                    .join("; ");
                return OutputValidationResult {
                    output_name: output_name.to_string(),
                    contract_id: Some(schema.contract_id.clone()),
                    status: ValidationStatus::Failed,
                    missing_fields: Vec::new(),
                    validation_error: Some(validation_error),
                    raw_payload_size: content.len(),
                };
            }
        }
        if schema.contract_id == PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID {
            if let Some(validation_error) =
                proposal_review_summary_v2_validation_error(&serde_json::Value::Object(obj.clone()))
            {
                return OutputValidationResult {
                    output_name: output_name.to_string(),
                    contract_id: Some(schema.contract_id.clone()),
                    status: ValidationStatus::Failed,
                    missing_fields: Vec::new(),
                    validation_error: Some(validation_error),
                    raw_payload_size: content.len(),
                };
            }
        }

        let missing_fields: Vec<String> = schema
            .required_fields
            .iter()
            .filter(|field| !obj.contains_key(field.as_str()))
            .cloned()
            .collect();

        let mut validation_errors = Vec::new();
        if !missing_fields.is_empty() {
            validation_errors.push(format!(
                "Missing required fields: {}",
                missing_fields.join(", ")
            ));
        }
        if schema.required_fields.iter().any(|field| field == "status") {
            if let Some(raw_status) = obj.get("status").and_then(|value| value.as_str()) {
                if let Some(allowed) = contract_status_allowed_values(&schema.contract_id) {
                    if !allowed.contains(&raw_status) {
                        // P079-SEC-HIGH-003: truncate provider-controlled status to a fixed length
                        // so it cannot be used to inflate or inject into validation error messages.
                        let safe_status: String = raw_status.chars().take(64).collect();
                        validation_errors.push(format!(
                            "Invalid status (truncated) `{}` for contract `{}`; allowed values for status: {}",
                            safe_status,
                            schema.contract_id,
                            allowed
                                .iter()
                                .map(|value| format!("`{value}`"))
                                .collect::<Vec<_>>()
                                .join(", ")
                        ));
                    }
                }
            }
        }

        OutputValidationResult {
            output_name: output_name.to_string(),
            contract_id: Some(schema.contract_id.clone()),
            status: if validation_errors.is_empty() {
                ValidationStatus::Passed
            } else {
                ValidationStatus::Failed
            },
            missing_fields,
            validation_error: (!validation_errors.is_empty()).then(|| validation_errors.join("; ")),
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
    let recovery_recommendation = match failure_class {
        ValidationFailureClass::NoOutputProduced
        | ValidationFailureClass::PersistenceFailure
        | ValidationFailureClass::MissingRequiredOutputs => {
            RecoveryRecommendation {
                action: "operator_inspection".to_string(),
                explanation: "Inspect the agent transcript, receipt, and declared output contract before deciding whether to retry; do not perform a blind identical retry after missing required outputs.".to_string(),
            }
        }
        ValidationFailureClass::EmptyOutput
        | ValidationFailureClass::OutputContractMismatch
        | ValidationFailureClass::InvalidRequiredOutputs
        | ValidationFailureClass::ProviderModeMismatch => {
            RecoveryRecommendation {
                action: "retry_failed_agent".to_string(),
                explanation: "Retry the failed agent with the same declared outputs and inspect the validation failure payload.".to_string(),
            }
        }
    };

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
        recovery_recommendation,
        diagnostic_artifact_paths: Vec::new(),
    })
}

pub fn build_expected_output_specs(
    declared_outputs: &[DeclaredOutput],
    workspace_root: &str,
    worktree_root: Option<&str>,
    chainworks_meta_root: Option<&str>,
    worktree_write_enabled: bool,
) -> Vec<ExpectedOutputSpec> {
    declared_outputs
        .iter()
        .flat_map(|declared| {
            let machine = expected_output_spec(
                &declared.output_name,
                ExpectedOutputRole::Machine,
                &declared.target_path,
                None,
                declared
                    .schema
                    .as_ref()
                    .map(|schema| schema.contract_id.clone()),
                declared.reuse_policy,
                workspace_root,
                worktree_root,
                chainworks_meta_root,
                worktree_write_enabled,
            );
            let companion = declared
                .companion_output_name
                .as_ref()
                .zip(declared.companion_path.as_ref())
                .map(|(name, path)| {
                    expected_output_spec(
                        name,
                        ExpectedOutputRole::Companion,
                        path,
                        Some(declared.output_name.clone()),
                        declared
                            .schema
                            .as_ref()
                            .map(|schema| schema.contract_id.clone()),
                        declared.reuse_policy,
                        workspace_root,
                        worktree_root,
                        chainworks_meta_root,
                        worktree_write_enabled,
                    )
                });
            std::iter::once(machine).chain(companion)
        })
        .collect()
}

fn expected_output_spec(
    output_name: &str,
    output_role: ExpectedOutputRole,
    target_path: &str,
    companion_of: Option<String>,
    contract_id: Option<String>,
    reuse_policy: Option<OutputReusePolicy>,
    workspace_root: &str,
    worktree_root: Option<&str>,
    chainworks_meta_root: Option<&str>,
    worktree_write_enabled: bool,
) -> ExpectedOutputSpec {
    ExpectedOutputSpec {
        output_name: output_name.to_string(),
        output_role,
        target_path: target_path.to_string(),
        companion_of,
        display_label: output_name.replace('_', " "),
        contract_id,
        required: true,
        reuse_policy: reuse_policy.unwrap_or(OutputReusePolicy::MustProduce),
        max_bytes: DEFAULT_EXPECTED_OUTPUT_MAX_BYTES,
        aggregate_acceptance_cap_bytes: DEFAULT_AGGREGATE_ACCEPTANCE_CAP_BYTES,
        authorized_roots: vec![authorized_root_for_output(
            target_path,
            workspace_root,
            worktree_root,
            chainworks_meta_root,
            worktree_write_enabled,
        )],
        source_generation_owner: if output_name == "changed_files_manifest" {
            SourceGenerationOwner::ControlPlane
        } else {
            SourceGenerationOwner::Agent
        },
    }
}

fn authorized_root_for_output(
    target_path: &str,
    workspace_root: &str,
    worktree_root: Option<&str>,
    chainworks_meta_root: Option<&str>,
    worktree_write_enabled: bool,
) -> AuthorizedRoot {
    let meta_root = chainworks_meta_root.map(|root| absolute_root(root, workspace_root));
    if let Some(meta_root) = meta_root.as_ref() {
        if path_is_under(target_path, meta_root) {
            return AuthorizedRoot {
                root_class: OutputRootClass::ChainworksMetaRoot,
                root_path: meta_root.to_string_lossy().into_owned(),
            };
        }
    }

    let worktree_root = worktree_root.map(|root| absolute_root(root, workspace_root));
    if worktree_write_enabled {
        if let Some(worktree_root) = worktree_root.as_ref() {
            return AuthorizedRoot {
                root_class: OutputRootClass::Worktree,
                root_path: worktree_root.to_string_lossy().into_owned(),
            };
        }
    }

    if let Some(worktree_root) = worktree_root.as_ref() {
        if path_is_under(target_path, worktree_root) {
            return AuthorizedRoot {
                root_class: OutputRootClass::Worktree,
                root_path: worktree_root.to_string_lossy().into_owned(),
            };
        }
    }

    AuthorizedRoot {
        root_class: OutputRootClass::Workspace,
        root_path: absolute_root(workspace_root, workspace_root)
            .to_string_lossy()
            .into_owned(),
    }
}

fn absolute_root(root: &str, workspace_root: &str) -> PathBuf {
    let root_path = Path::new(root);
    if root_path.is_absolute() {
        root_path.to_path_buf()
    } else {
        Path::new(workspace_root).join(root_path)
    }
}

fn path_is_under(path: &str, root: &Path) -> bool {
    Path::new(path).starts_with(root)
}

pub fn build_captured_outputs_from_discovery_decisions(
    declared_outputs: &[DeclaredOutput],
    decisions: &[OutputDiscoveryDecision],
    accepted_payloads: &HashMap<String, Vec<u8>>,
) -> Vec<CapturedOutput> {
    declared_outputs
        .iter()
        .map(|declared| {
            let machine_bytes = accepted_decision_payload(
                decisions,
                &declared.output_name,
                ExpectedOutputRole::Machine,
                accepted_payloads,
            );
            let companion_bytes =
                declared
                    .companion_output_name
                    .as_deref()
                    .and_then(|companion_name| {
                        accepted_decision_payload(
                            decisions,
                            companion_name,
                            ExpectedOutputRole::Companion,
                            accepted_payloads,
                        )
                    });

            CapturedOutput {
                declared: declared.clone(),
                machine_bytes,
                companion_bytes,
            }
        })
        .collect()
}

fn accepted_decision_payload(
    decisions: &[OutputDiscoveryDecision],
    output_name: &str,
    output_role: ExpectedOutputRole,
    accepted_payloads: &HashMap<String, Vec<u8>>,
) -> Option<Vec<u8>> {
    let decision = decisions.iter().find(|decision| {
        decision.output_name == output_name && decision.output_role == output_role
    })?;
    if decision.status != OutputDiscoveryStatus::Accepted {
        return None;
    }
    let payload_ref = decision.accepted_payload_ref.as_ref()?;
    accepted_payloads.get(payload_ref).cloned()
}

pub fn declared_output_paths(declared_outputs: &[DeclaredOutput]) -> HashMap<String, String> {
    declared_outputs
        .iter()
        .map(|declared| (declared.output_name.clone(), declared.target_path.clone()))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::commands::{KnowledgeCapsulesMode, MainSyncMode};

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
    fn proposal_064_runtime_config_defaults_modes_to_off() {
        let parsed: Proposal064RuntimeConfig =
            serde_json::from_value(serde_json::json!({})).expect("empty config should parse");
        assert_eq!(parsed.main_sync.mode, MainSyncMode::Off);
        assert_eq!(parsed.knowledge_capsules.mode, KnowledgeCapsulesMode::Off);
    }

    #[test]
    fn proposal_064_runtime_config_parses_frozen_mode_values() {
        let parsed: Proposal064RuntimeConfig = serde_json::from_value(serde_json::json!({
            "main_sync": { "mode": "manual_only" },
            "knowledge_capsules": { "mode": "emit_only" }
        }))
        .expect("explicit modes should parse");
        assert_eq!(parsed.main_sync.mode, MainSyncMode::ManualOnly);
        assert_eq!(
            parsed.knowledge_capsules.mode,
            KnowledgeCapsulesMode::EmitOnly
        );
    }

    #[test]
    fn validate_output_rejects_missing_required_fields() {
        let schema = structured_schema();
        let result = validate_output("proposal_review", br#"{"summary":"ok"}"#, Some(&schema));
        assert_eq!(result.status, ValidationStatus::Failed);
        assert_eq!(result.missing_fields, vec!["status"]);
    }

    #[test]
    fn validate_output_rejects_status_values_outside_contract_enum() {
        let schema = OutputSchema {
            contract_id: "audit_report_v1".to_string(),
            format: "json".to_string(),
            human_format: None,
            machine_format: Some("json".to_string()),
            validation_mode: Some("strict_structured".to_string()),
            normalized_artifact_name: Some("audit_report".to_string()),
            raw_artifact_name: None,
            required_fields: vec![
                "status".to_string(),
                "matches_proposal".to_string(),
                "missing_items".to_string(),
                "extra_items".to_string(),
                "defects".to_string(),
                "required_fixes".to_string(),
            ],
        };
        let result = validate_output(
            "audit_report",
            br#"{"status":"Not Implemented","matches_proposal":false,"missing_items":[],"extra_items":[],"defects":[],"required_fixes":[]}"#,
            Some(&schema),
        );

        assert_eq!(result.status, ValidationStatus::Failed);
        assert!(result
            .validation_error
            .as_deref()
            .unwrap_or_default()
            .contains("allowed values for status"));

        let alias_result = validate_output(
            "audit_report",
            br#"{"status":"Implemented","matches_proposal":true,"missing_items":[],"extra_items":[],"defects":[],"required_fixes":[]}"#,
            Some(&schema),
        );
        assert_eq!(
            alias_result.status,
            ValidationStatus::Failed,
            "runtime output validation should require canonical status values, not legacy aliases"
        );
    }

    #[test]
    fn validate_output_rejects_prepush_status_alias_before_settlement() {
        let schema = OutputSchema {
            contract_id: "prepush_review_v1".to_string(),
            format: "json".to_string(),
            human_format: None,
            machine_format: Some("json".to_string()),
            validation_mode: Some("strict_structured".to_string()),
            normalized_artifact_name: Some("prepush_review_report".to_string()),
            raw_artifact_name: None,
            required_fields: vec![
                "status".to_string(),
                "major_concerns".to_string(),
                "cleanup_items".to_string(),
                "test_coverage_notes".to_string(),
                "release_note".to_string(),
            ],
        };

        let result = validate_output(
            "prepush_review_report",
            br#"{"status":"pass_with_notes","major_concerns":[],"cleanup_items":[],"test_coverage_notes":[],"release_note":"ok"}"#,
            Some(&schema),
        );

        assert_eq!(result.status, ValidationStatus::Failed);
        assert!(result
            .validation_error
            .as_deref()
            .unwrap_or_default()
            .contains("allowed values for status"));
    }

    #[test]
    fn expected_output_specs_include_machine_and_companion_policy() {
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: "/workspace/.chainworks/runs/run-1/proposal_review.json".to_string(),
            schema: Some(structured_schema()),
            reuse_policy: None,
            companion_output_name: Some("proposal_review_raw".to_string()),
            companion_path: Some(
                "/workspace/.chainworks/runs/run-1/proposal_review.md".to_string(),
            ),
        };

        let specs = build_expected_output_specs(
            &[declared],
            "/workspace",
            Some("/workspace/.chainworks/worktrees/impl"),
            Some(".chainworks/runs/run-1"),
            true,
        );

        assert_eq!(specs.len(), 2);
        assert_eq!(specs[0].output_name, "proposal_review");
        assert_eq!(specs[0].output_role, ExpectedOutputRole::Machine);
        assert_eq!(specs[0].contract_id.as_deref(), Some("proposal_review_v1"));
        assert!(specs[0].required);
        assert_eq!(specs[0].reuse_policy, OutputReusePolicy::MustProduce);
        assert_eq!(specs[0].max_bytes, 10 * 1024 * 1024);
        assert_eq!(specs[0].aggregate_acceptance_cap_bytes, 64 * 1024 * 1024);
        assert_eq!(
            specs[0].authorized_roots[0].root_class,
            OutputRootClass::ChainworksMetaRoot
        );
        assert_eq!(
            specs[0].authorized_roots[0].root_path,
            "/workspace/.chainworks/runs/run-1"
        );
        assert_eq!(specs[1].output_role, ExpectedOutputRole::Companion);
        assert_eq!(specs[1].companion_of.as_deref(), Some("proposal_review"));
    }

    #[test]
    fn expected_output_specs_authorize_write_enabled_worktree_outputs() {
        let declared = DeclaredOutput {
            output_name: "changed_files_manifest".to_string(),
            target_path: "/workspace/.chainworks/worktrees/impl/changed-files.json".to_string(),
            schema: None,
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };

        let specs = build_expected_output_specs(
            &[declared],
            "/workspace",
            Some("/workspace/.chainworks/worktrees/impl"),
            Some(".chainworks/runs/run-1"),
            true,
        );

        assert_eq!(specs.len(), 1);
        assert_eq!(
            specs[0].authorized_roots[0].root_class,
            OutputRootClass::Worktree
        );
        assert_eq!(
            specs[0].authorized_roots[0].root_path,
            "/workspace/.chainworks/worktrees/impl"
        );
        assert_eq!(
            specs[0].source_generation_owner,
            SourceGenerationOwner::ControlPlane
        );
    }

    #[test]
    fn expected_output_specs_freeze_declared_reuse_policy() {
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: "/workspace/.chainworks/runs/run-1/proposal_review.json".to_string(),
            schema: Some(structured_schema()),
            reuse_policy: Some(OutputReusePolicy::AllowUnchangedExisting),
            companion_output_name: Some("proposal_review_raw".to_string()),
            companion_path: Some(
                "/workspace/.chainworks/runs/run-1/proposal_review.md".to_string(),
            ),
        };

        let specs = build_expected_output_specs(
            &[declared],
            "/workspace",
            Some("/workspace/.chainworks/worktrees/impl"),
            Some(".chainworks/runs/run-1"),
            false,
        );

        assert_eq!(
            specs[0].reuse_policy,
            OutputReusePolicy::AllowUnchangedExisting
        );
        assert_eq!(
            specs[1].reuse_policy,
            OutputReusePolicy::AllowUnchangedExisting
        );
    }

    #[test]
    fn validate_task_outputs_requires_companion_for_companion_mode() {
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: "/tmp/proposal_review.json".to_string(),
            schema: Some(structured_schema()),
            reuse_policy: None,
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

    #[test]
    fn discovery_decision_builder_does_not_reread_rejected_target_path() {
        let target_path = std::env::temp_dir().join(format!(
            "chainworks-p053-rejected-{}.json",
            uuid::Uuid::new_v4()
        ));
        std::fs::write(&target_path, br#"{"summary":"stale","status":"green"}"#)
            .expect("write stale target");

        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: target_path.to_string_lossy().into_owned(),
            schema: Some(structured_schema()),
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let decisions = vec![domain::discovery::OutputDiscoveryDecision {
            output_name: declared.output_name.clone(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: declared.target_path.clone(),
            companion_of: None,
            status: domain::discovery::OutputDiscoveryStatus::Rejected,
            reason: domain::discovery::OutputDiscoveryReason::StaleExpectedOutput,
            provenance: Some(domain::discovery::OutputDiscoveryProvenance::ExactPath),
            canonical_path: Some(declared.target_path.clone()),
            root_class: Some(domain::discovery::OutputRootClass::ChainworksMetaRoot),
            baseline_status: Some(
                domain::discovery::ExpectedPathBaselineStatus::RegularContentCaptured,
            ),
            size_bytes: Some(36),
            content_digest: Some("sha256:unchanged".to_string()),
            max_bytes_applied: Some(10 * 1024 * 1024),
            aggregate_bytes_after_acceptance: None,
            accepted_payload_ref: None,
            accepted_bytes_sha256: None,
            generated_by: None,
            diagnostics: Default::default(),
            decision_at: chrono::Utc::now(),
        }];

        let captured = build_captured_outputs_from_discovery_decisions(
            &[declared],
            &decisions,
            &HashMap::new(),
        );
        let summary = validate_task_outputs(&captured);

        assert!(!summary.raw_output_exists);
        assert_eq!(
            summary.failure_class,
            Some(ValidationFailureClass::NoOutputProduced)
        );
        let _ = std::fs::remove_file(target_path);
    }

    #[test]
    fn discovery_decision_builder_uses_only_accepted_payload_refs() {
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: "/tmp/proposal_review.json".to_string(),
            schema: Some(structured_schema()),
            reuse_policy: None,
            companion_output_name: None,
            companion_path: None,
        };
        let decisions = vec![domain::discovery::OutputDiscoveryDecision {
            output_name: declared.output_name.clone(),
            output_role: domain::discovery::ExpectedOutputRole::Machine,
            target_path: declared.target_path.clone(),
            companion_of: None,
            status: domain::discovery::OutputDiscoveryStatus::Accepted,
            reason: domain::discovery::OutputDiscoveryReason::ProviderEnvelope,
            provenance: Some(domain::discovery::OutputDiscoveryProvenance::ProviderEnvelope),
            canonical_path: None,
            root_class: Some(domain::discovery::OutputRootClass::ChainworksMetaRoot),
            baseline_status: None,
            size_bytes: Some(33),
            content_digest: None,
            max_bytes_applied: Some(10 * 1024 * 1024),
            aggregate_bytes_after_acceptance: Some(33),
            accepted_payload_ref: Some("provider:proposal_review".to_string()),
            accepted_bytes_sha256: Some("sha256:accepted".to_string()),
            generated_by: None,
            diagnostics: Default::default(),
            decision_at: chrono::Utc::now(),
        }];
        let accepted_payloads = HashMap::from([(
            "provider:proposal_review".to_string(),
            br#"{"summary":"ok","status":"green"}"#.to_vec(),
        )]);

        let captured = build_captured_outputs_from_discovery_decisions(
            &[declared],
            &decisions,
            &accepted_payloads,
        );
        let summary = validate_task_outputs(&captured);

        assert!(summary.raw_output_exists);
        assert_eq!(summary.failure_class, None);
    }

    #[test]
    fn build_validation_failure_record_uses_operator_inspection_for_missing_output() {
        let declared = DeclaredOutput {
            output_name: "proposal_review".to_string(),
            target_path: "/tmp/proposal_review.json".to_string(),
            schema: Some(structured_schema()),
            reuse_policy: None,
            companion_output_name: Some("proposal_review_raw".to_string()),
            companion_path: Some("/tmp/proposal_review.md".to_string()),
        };
        let summary = validate_task_outputs(&[CapturedOutput {
            declared,
            machine_bytes: None,
            companion_bytes: None,
        }]);

        let record = build_validation_failure_record(
            domain::ids::ArtifactId::new(),
            domain::ids::RunId::new(),
            "state_1".to_string(),
            domain::ids::StageExecutionId::new(),
            "agent".to_string(),
            domain::ids::AgentExecutionId::new(),
            summary,
            false,
            false,
        )
        .expect("missing output should still produce a validation failure record");

        assert_eq!(
            record.failure_class,
            ValidationFailureClass::NoOutputProduced
        );
        assert!(!record.transcript_exists);
        assert_eq!(record.recovery_recommendation.action, "operator_inspection");
        assert!(record
            .recovery_recommendation
            .explanation
            .contains("do not perform a blind identical retry"));
    }

    #[test]
    fn validate_output_rejects_invalid_nested_v2_self_assessment_fields() {
        let mut schema = structured_schema();
        schema.contract_id =
            domain::artifact_contracts::IMPLEMENTATION_SELF_ASSESSMENT_V2_CONTRACT_ID.to_string();
        schema.required_fields = vec![
            "implementation_complete".to_string(),
            "verification_green".to_string(),
            "remaining_code_tasks".to_string(),
            "handoff_tasks".to_string(),
            "known_risks".to_string(),
            "tests_run".to_string(),
            "docs_impacted".to_string(),
        ];

        let result = validate_output(
            "implementation_self_assessment",
            br#"{
                "implementation_complete": false,
                "verification_green": false,
                "remaining_code_tasks": [{
                    "summary": "Fix compile error",
                    "owner": "code_writer",
                    "evidence": "cargo test failed before compilation"
                }],
                "handoff_tasks": [],
                "known_risks": [],
                "tests_run": [],
                "docs_impacted": []
            }"#,
            Some(&schema),
        );

        assert_eq!(result.status, ValidationStatus::Failed);
        assert!(
            result
                .validation_error
                .as_deref()
                .unwrap_or_default()
                .contains("/remaining_code_tasks/0/blocking"),
            "nested missing boolean must fail closed: {:?}",
            result.validation_error
        );
    }

    #[test]
    fn validate_output_accepts_v2_proposal_review_summary_with_advisories() {
        let mut schema = structured_schema();
        schema.contract_id = PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID.to_string();
        schema.required_fields = vec![
            "pass".into(),
            "average_score".into(),
            "aggregate_score".into(),
            "min_individual_score".into(),
            "blocker_count".into(),
            "blocking_issues".into(),
            "summary".into(),
            "blocking_required_changes".into(),
            "advisory_follow_ups".into(),
            "recurring_themes".into(),
            "decision".into(),
        ];

        let result = validate_output(
            "proposal_review_summary",
            br#"{
                "pass": true,
                "average_score": 8.5,
                "aggregate_score": 8.5,
                "min_individual_score": 8.0,
                "blocker_count": 0,
                "blocking_issues": [],
                "summary": "approved",
                "blocking_required_changes": [],
                "advisory_follow_ups": ["carry rollout caution into implementation"],
                "recurring_themes": ["durability"],
                "decision": "approved"
            }"#,
            Some(&schema),
        );

        assert_eq!(result.status, ValidationStatus::Passed);
        assert_eq!(result.validation_error, None);
    }

    #[test]
    fn validate_output_rejects_v2_proposal_review_summary_with_blocking_changes_on_pass() {
        let mut schema = structured_schema();
        schema.contract_id = PROPOSAL_REVIEW_SUMMARY_V2_CONTRACT_ID.to_string();
        schema.required_fields = vec![
            "pass".into(),
            "average_score".into(),
            "aggregate_score".into(),
            "min_individual_score".into(),
            "blocker_count".into(),
            "blocking_issues".into(),
            "summary".into(),
            "blocking_required_changes".into(),
            "advisory_follow_ups".into(),
            "recurring_themes".into(),
            "decision".into(),
        ];

        let result = validate_output(
            "proposal_review_summary",
            br#"{
                "pass": true,
                "average_score": 8.5,
                "aggregate_score": 8.5,
                "min_individual_score": 8.0,
                "blocker_count": 0,
                "blocking_issues": [],
                "summary": "approved",
                "blocking_required_changes": ["fix schema mismatch"],
                "advisory_follow_ups": [],
                "recurring_themes": [],
                "decision": "approved"
            }"#,
            Some(&schema),
        );

        assert_eq!(result.status, ValidationStatus::Failed);
        assert!(result
            .validation_error
            .as_deref()
            .unwrap_or_default()
            .contains("pass=true while blocker evidence is non-empty"));
    }
}
