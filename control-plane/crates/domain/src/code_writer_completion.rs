use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentExecutionId, RunId, StageExecutionId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWriterCompletionReceiptRecord {
    pub id: String,
    pub run_id: RunId,
    pub stage_execution_id: StageExecutionId,
    pub agent_execution_id: AgentExecutionId,
    pub session_generation_id: Option<String>,
    pub original_runtime_receipt_id: Option<String>,
    pub completion_repair_runtime_receipt_id: Option<String>,
    pub provider: String,
    pub model: Option<String>,
    pub completion_mode: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub activation_source: String,
    pub ingestion_boundary_failure: Option<String>,
    pub work_change_kind: Option<String>,
    pub pre_prompt_worktree_fingerprint_path: Option<String>,
    pub post_prompt_worktree_fingerprint_path: Option<String>,
    pub pre_prompt_worktree_fingerprint_sha256: Option<String>,
    pub post_prompt_worktree_fingerprint_sha256: Option<String>,
    pub current_attempt_changed_path_count: i64,
    pub preexisting_dirty_path_count: i64,
    pub completion_status: String,
    pub failure_class: Option<String>,
    pub provider_runtime_family: Option<String>,
    pub completion_boundary_subtype: Option<String>,
    pub final_payload_status: Option<String>,
    pub progress_before_handoff: Option<String>,
    pub runtime_preflight_phase: Option<String>,
    pub runtime_tool_path_preflight_json: Option<String>,
    pub final_completion_payload_capture_json: Option<String>,
    pub engine_failure_envelope_json: Option<String>,
    pub repair_failure_envelope_json: Option<String>,
    pub repair_materialization_summary_json: Option<String>,
    pub repair_materialization_mode: Option<String>,
    pub strict_final_payload_enabled: bool,
    pub staged_repair_settlement_enabled: bool,
    pub terminal_response_status: Option<String>,
    pub completion_turn_attempted: bool,
    pub completion_turn_result: Option<String>,
    pub completion_text_capture_count: i64,
    pub completion_text_absence_count: i64,
    pub completion_repair_text_status: Option<String>,
    pub completion_repair_raw_text_artifact_path: Option<String>,
    pub completion_repair_redacted_text_artifact_path: Option<String>,
    pub completion_repair_text_absence_reason: Option<String>,
    pub fresh_required_output_count: i64,
    pub stale_required_output_count: i64,
    pub missing_required_output_count: i64,
    pub control_plane_output_count: i64,
    pub completion_repair_turn_count: i64,
    pub generic_repair_turn_count: i64,
    pub missing_outputs: Vec<String>,
    pub stale_outputs: Vec<String>,
    pub transcript_status: Option<String>,
    pub transcript_absence_reason: Option<String>,
    pub receipt_artifact_path: Option<String>,
    pub failed_stage_evidence_path: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWriterCompletionTextCaptureRecord {
    pub receipt_id: String,
    pub prompt_kind: String,
    pub turn_index: i64,
    pub terminal_response_status: Option<String>,
    pub completion_text_status: String,
    pub completion_text_capture_source: Option<String>,
    pub completion_text_raw_byte_limit: Option<i64>,
    pub completion_text_captured_byte_count: Option<i64>,
    pub completion_text_truncated: bool,
    pub extraction_input_truncated: bool,
    pub extraction_input_sha256: Option<String>,
    pub raw_text_artifact_path: Option<String>,
    pub redacted_text_artifact_path: Option<String>,
    pub text_absence_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWriterCompletionOutputDecisionRecord {
    pub receipt_id: String,
    pub output_name: String,
    pub contract_id: Option<String>,
    pub canonical_path: String,
    pub pre_prompt_sha256: Option<String>,
    pub post_prompt_sha256: Option<String>,
    pub content_sha256: Option<String>,
    pub settlement_source: Option<String>,
    pub validation_status: Option<String>,
    pub rejection_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWriterOutputSettlementRow {
    pub id: String,
    pub receipt_id: String,
    pub run_id: RunId,
    pub stage_id: String,
    pub stage_execution_id: StageExecutionId,
    pub agent_execution_id: AgentExecutionId,
    pub session_generation_id: Option<String>,
    pub repair_attempt: i64,
    pub output_name: String,
    pub contract_id: String,
    pub source_kind: String,
    pub source_generation_owner: String,
    pub candidate_digest: Option<String>,
    pub staging_path: Option<String>,
    pub canonical_path: String,
    pub canonical_before_sha256: Option<String>,
    pub canonical_after_sha256: Option<String>,
    pub decision: String,
    pub rejection_reason: Option<String>,
    pub materialization_state: String,
    pub active_pointer_generation_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub committed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWriterCompletionReceiptReadback {
    pub receipt: CodeWriterCompletionReceiptRecord,
    pub text_captures: Vec<CodeWriterCompletionTextCaptureRecord>,
    pub output_decisions: Vec<CodeWriterCompletionOutputDecisionRecord>,
    pub settlement_rows: Vec<CodeWriterOutputSettlementRow>,
    pub prompt_evidence: Option<CodeWriterCompletionPromptEvidenceReadback>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CodeWriterCompletionPromptEvidenceReadback {
    pub runtime_receipt_id: String,
    pub prompt_kind: String,
    pub turn_index: i64,
    pub prompt_template_id: Option<String>,
    pub prompt_template_version: Option<i64>,
    pub prompt_sha256: Option<String>,
    pub redacted_prompt_artifact_path: Option<String>,
    pub expected_output_contract_snapshot_sha256: Option<String>,
    pub expected_output_contract_snapshot_path: Option<String>,
    pub repair_or_settlement_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PublicEnumReadback {
    pub value: String,
    pub raw: Option<String>,
    pub known: bool,
}

impl PublicEnumReadback {
    fn from_optional(raw: Option<&str>, known_values: &[&str], default_known: &str) -> Self {
        match raw {
            Some(value) if known_values.contains(&value) => Self {
                value: value.to_string(),
                raw: Some(value.to_string()),
                known: true,
            },
            Some(value) => Self {
                value: "unknown".to_string(),
                raw: Some(value.to_string()),
                known: false,
            },
            None => Self {
                value: default_known.to_string(),
                raw: None,
                known: true,
            },
        }
    }

    fn from_required(raw: &str, known_values: &[&str]) -> Self {
        Self::from_optional(Some(raw), known_values, "unknown")
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationCompletionTextCaptureReadback {
    pub prompt_kind: String,
    pub turn_index: i64,
    pub terminal_response_status: Option<String>,
    pub completion_text_status: String,
    pub completion_text_capture_source: Option<String>,
    pub completion_text_raw_byte_limit: Option<i64>,
    pub completion_text_captured_byte_count: Option<i64>,
    pub completion_text_truncated: bool,
    pub extraction_input_truncated: bool,
    pub extraction_input_sha256: Option<String>,
    pub redacted_text_artifact_path: Option<String>,
    pub text_absence_reason: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImplementationCompletionSummary {
    pub status: PublicEnumReadback,
    pub failure_class: Option<String>,
    pub provider_runtime_family: Option<String>,
    pub completion_boundary_subtype: PublicEnumReadback,
    pub final_payload_status: Option<String>,
    pub progress_before_handoff: Option<String>,
    pub runtime_preflight_phase: Option<String>,
    pub runtime_tool_path_preflight_json: Option<String>,
    pub final_completion_payload_capture_json: Option<String>,
    pub failure_envelope_authority: Option<String>,
    pub engine_failure_envelope_json: Option<String>,
    pub repair_failure_envelope_json: Option<String>,
    pub repair_materialization_summary_json: Option<String>,
    pub repair_materialization_mode: Option<String>,
    pub strict_final_payload_enabled: bool,
    pub staged_repair_settlement_enabled: bool,
    pub work_change_kind: Option<String>,
    pub activation_source: Option<String>,
    pub completion_mode: Option<String>,
    pub published_at: Option<DateTime<Utc>>,
    pub ingestion_boundary_failure: PublicEnumReadback,
    pub pre_prompt_worktree_fingerprint_path: Option<String>,
    pub post_prompt_worktree_fingerprint_path: Option<String>,
    pub completion_turn_attempted: bool,
    pub completion_turn_result: PublicEnumReadback,
    pub terminal_response_status: Option<String>,
    pub completion_text_captures: Vec<ImplementationCompletionTextCaptureReadback>,
    pub prompt_template_id: Option<String>,
    pub prompt_template_version: Option<i64>,
    pub prompt_sha256: Option<String>,
    pub redacted_prompt_artifact_path: Option<String>,
    pub expected_output_contract_snapshot_sha256: Option<String>,
    pub repair_or_settlement_reason: Option<String>,
    pub fresh_required_output_count: i64,
    pub stale_required_output_count: i64,
    pub missing_required_output_count: i64,
    pub control_plane_output_count: i64,
    pub completion_repair_turn_count: i64,
    pub generic_repair_turn_count: i64,
    pub missing_outputs: Vec<String>,
    pub stale_outputs: Vec<String>,
    pub completion_repair_text_status: Option<String>,
    pub completion_repair_redacted_text_artifact_path: Option<String>,
    pub completion_repair_text_absence_reason: Option<String>,
    pub transcript_status: Option<String>,
    pub transcript_absence_reason: Option<String>,
    pub receipt_artifact_path: Option<String>,
    pub failed_stage_evidence_path: Option<String>,
    pub next_operator_action: PublicEnumReadback,
}

const STATUS_VALUES: &[&str] = &[
    "not_applicable",
    "not_attempted",
    "succeeded",
    "failed",
    "blocked",
    "skipped_no_live_session",
    "partial_evidence",
    "unknown",
];

const INGESTION_BOUNDARY_FAILURE_VALUES: &[&str] = &[
    "none",
    "acp_final_text_not_collected",
    "chainworks_output_not_extracted",
    "declared_output_settlement_rejected_usable_payload",
    "terminal_response_capture_truncated_before_output",
    "extraction_input_truncated",
    "unknown",
];

const COMPLETION_TURN_RESULT_VALUES: &[&str] = &[
    "not_attempted",
    "succeeded",
    "failed_missing_outputs",
    "failed_schema_validation",
    "failed_unexpected_worktree_mutation",
    "generic_repair_already_failed_completion_contract_required",
    "skipped_ineligible",
    "skipped_no_live_session",
    "skipped_usable_final_output_settled",
    "unknown",
];

const NEXT_OPERATOR_ACTION_VALUES: &[&str] = &[
    "none",
    "inspect_outputs_then_retry",
    "inspect_truncated_completion_text",
    "inspect_prompt_and_expected_output_contract",
    "materialize_fixtures_before_implementation",
    "retry_with_completion_recovery",
    "fix_acp_final_text_collection",
    "fix_chainworks_output_extraction",
    "fix_declared_output_settlement",
    "do_not_retry_preexisting_dirty_timeout",
    "unknown",
];

const COMPLETION_BOUNDARY_SUBTYPE_VALUES: &[&str] = &[
    "none",
    "junie_final_response_missing",
    "junie_final_response_truncated",
    "junie_progress_without_terminal_handoff",
    "junie_repair_returned_narrative",
    "junie_repair_returned_malformed_json",
    "junie_repair_outputs_partially_materialized",
    "junie_runtime_tool_path_failure_before_publication",
    "provider_envelope_unrecognized",
    "provider_envelope_identity_mismatch",
    "provider_authored_engine_failure_spoof_rejected",
    "unknown",
];

pub fn project_implementation_completion(
    readbacks: &[CodeWriterCompletionReceiptReadback],
) -> ImplementationCompletionSummary {
    let Some(readback) = readbacks
        .iter()
        .max_by_key(|readback| readback.receipt.created_at)
    else {
        return not_attempted_implementation_completion();
    };
    let receipt = &readback.receipt;
    let status =
        PublicEnumReadback::from_required(derive_public_completion_status(receipt), STATUS_VALUES);
    let ingestion_boundary_failure = PublicEnumReadback::from_optional(
        receipt.ingestion_boundary_failure.as_deref(),
        INGESTION_BOUNDARY_FAILURE_VALUES,
        "none",
    );
    let completion_turn_result = PublicEnumReadback::from_optional(
        receipt.completion_turn_result.as_deref(),
        COMPLETION_TURN_RESULT_VALUES,
        "not_attempted",
    );
    let next_operator_action = PublicEnumReadback::from_required(
        derive_next_operator_action(
            receipt,
            &readback.text_captures,
            &status,
            &ingestion_boundary_failure,
            &completion_turn_result,
        ),
        NEXT_OPERATOR_ACTION_VALUES,
    );

    ImplementationCompletionSummary {
        status,
        failure_class: receipt.failure_class.clone(),
        provider_runtime_family: receipt.provider_runtime_family.clone(),
        completion_boundary_subtype: PublicEnumReadback::from_optional(
            receipt.completion_boundary_subtype.as_deref(),
            COMPLETION_BOUNDARY_SUBTYPE_VALUES,
            "none",
        ),
        final_payload_status: receipt.final_payload_status.clone(),
        progress_before_handoff: receipt.progress_before_handoff.clone(),
        runtime_preflight_phase: receipt.runtime_preflight_phase.clone(),
        runtime_tool_path_preflight_json: receipt.runtime_tool_path_preflight_json.clone(),
        final_completion_payload_capture_json: receipt
            .final_completion_payload_capture_json
            .clone(),
        failure_envelope_authority: p090_failure_envelope_authority_from_capture_json(
            receipt.final_completion_payload_capture_json.as_deref(),
        ),
        engine_failure_envelope_json: receipt.engine_failure_envelope_json.clone(),
        repair_failure_envelope_json: receipt.repair_failure_envelope_json.clone(),
        repair_materialization_summary_json: receipt.repair_materialization_summary_json.clone(),
        repair_materialization_mode: receipt.repair_materialization_mode.clone(),
        strict_final_payload_enabled: receipt.strict_final_payload_enabled,
        staged_repair_settlement_enabled: receipt.staged_repair_settlement_enabled,
        work_change_kind: receipt.work_change_kind.clone(),
        activation_source: Some(receipt.activation_source.clone()),
        completion_mode: receipt.completion_mode.clone(),
        published_at: receipt.published_at,
        ingestion_boundary_failure,
        pre_prompt_worktree_fingerprint_path: receipt.pre_prompt_worktree_fingerprint_path.clone(),
        post_prompt_worktree_fingerprint_path: receipt
            .post_prompt_worktree_fingerprint_path
            .clone(),
        completion_turn_attempted: receipt.completion_turn_attempted,
        completion_turn_result,
        terminal_response_status: receipt.terminal_response_status.clone(),
        completion_text_captures: readback
            .text_captures
            .iter()
            .cloned()
            .map(ImplementationCompletionTextCaptureReadback::from)
            .collect(),
        prompt_template_id: readback
            .prompt_evidence
            .as_ref()
            .and_then(|evidence| evidence.prompt_template_id.clone())
            .or_else(|| {
                receipt
                    .completion_turn_attempted
                    .then(|| "code_writer_completion_repair_v1".to_string())
            }),
        prompt_template_version: readback
            .prompt_evidence
            .as_ref()
            .and_then(|evidence| evidence.prompt_template_version)
            .or_else(|| receipt.completion_turn_attempted.then_some(1)),
        prompt_sha256: readback
            .prompt_evidence
            .as_ref()
            .and_then(|evidence| evidence.prompt_sha256.clone()),
        redacted_prompt_artifact_path: readback
            .prompt_evidence
            .as_ref()
            .and_then(|evidence| evidence.redacted_prompt_artifact_path.clone()),
        expected_output_contract_snapshot_sha256: readback
            .prompt_evidence
            .as_ref()
            .and_then(|evidence| evidence.expected_output_contract_snapshot_sha256.clone()),
        repair_or_settlement_reason: readback
            .prompt_evidence
            .as_ref()
            .and_then(|evidence| evidence.repair_or_settlement_reason.clone())
            .or_else(|| receipt.failure_class.clone()),
        fresh_required_output_count: receipt.fresh_required_output_count,
        stale_required_output_count: receipt.stale_required_output_count,
        missing_required_output_count: receipt.missing_required_output_count,
        control_plane_output_count: receipt.control_plane_output_count,
        completion_repair_turn_count: receipt.completion_repair_turn_count,
        generic_repair_turn_count: receipt.generic_repair_turn_count,
        missing_outputs: receipt.missing_outputs.clone(),
        stale_outputs: receipt.stale_outputs.clone(),
        completion_repair_text_status: receipt.completion_repair_text_status.clone(),
        completion_repair_redacted_text_artifact_path: receipt
            .completion_repair_redacted_text_artifact_path
            .clone(),
        completion_repair_text_absence_reason: receipt
            .completion_repair_text_absence_reason
            .clone(),
        transcript_status: receipt.transcript_status.clone(),
        transcript_absence_reason: receipt.transcript_absence_reason.clone(),
        receipt_artifact_path: receipt.receipt_artifact_path.clone(),
        failed_stage_evidence_path: receipt.failed_stage_evidence_path.clone(),
        next_operator_action,
    }
}

fn derive_public_completion_status(receipt: &CodeWriterCompletionReceiptRecord) -> &str {
    if receipt.completion_turn_result.as_deref() == Some("skipped_no_live_session") {
        return "skipped_no_live_session";
    }
    if receipt.failure_class.is_some()
        || receipt.missing_required_output_count > 0
        || receipt.stale_required_output_count > 0
    {
        return match receipt.completion_status.as_str() {
            "partial" | "partial_evidence" if receipt.fresh_required_output_count > 0 => {
                "partial_evidence"
            }
            "blocked" => "blocked",
            "skipped_no_live_session" => "skipped_no_live_session",
            _ => "failed",
        };
    }
    match receipt.completion_status.as_str() {
        "complete" | "succeeded" => "succeeded",
        "not_applicable" => "not_applicable",
        "not_attempted" => "not_attempted",
        "partial" | "partial_evidence" => "partial_evidence",
        "blocked" => "blocked",
        "skipped_no_live_session" => "skipped_no_live_session",
        _ => "unknown",
    }
}

fn not_attempted_implementation_completion() -> ImplementationCompletionSummary {
    ImplementationCompletionSummary {
        status: PublicEnumReadback::from_required("not_attempted", STATUS_VALUES),
        failure_class: None,
        provider_runtime_family: None,
        completion_boundary_subtype: PublicEnumReadback::from_required(
            "none",
            COMPLETION_BOUNDARY_SUBTYPE_VALUES,
        ),
        final_payload_status: None,
        progress_before_handoff: None,
        runtime_preflight_phase: None,
        runtime_tool_path_preflight_json: None,
        final_completion_payload_capture_json: None,
        failure_envelope_authority: None,
        engine_failure_envelope_json: None,
        repair_failure_envelope_json: None,
        repair_materialization_summary_json: None,
        repair_materialization_mode: None,
        strict_final_payload_enabled: false,
        staged_repair_settlement_enabled: false,
        work_change_kind: None,
        activation_source: None,
        completion_mode: None,
        published_at: None,
        ingestion_boundary_failure: PublicEnumReadback::from_required(
            "none",
            INGESTION_BOUNDARY_FAILURE_VALUES,
        ),
        pre_prompt_worktree_fingerprint_path: None,
        post_prompt_worktree_fingerprint_path: None,
        completion_turn_attempted: false,
        completion_turn_result: PublicEnumReadback::from_required(
            "not_attempted",
            COMPLETION_TURN_RESULT_VALUES,
        ),
        terminal_response_status: None,
        completion_text_captures: Vec::new(),
        prompt_template_id: None,
        prompt_template_version: None,
        prompt_sha256: None,
        redacted_prompt_artifact_path: None,
        expected_output_contract_snapshot_sha256: None,
        repair_or_settlement_reason: None,
        fresh_required_output_count: 0,
        stale_required_output_count: 0,
        missing_required_output_count: 0,
        control_plane_output_count: 0,
        completion_repair_turn_count: 0,
        generic_repair_turn_count: 0,
        missing_outputs: Vec::new(),
        stale_outputs: Vec::new(),
        completion_repair_text_status: None,
        completion_repair_redacted_text_artifact_path: None,
        completion_repair_text_absence_reason: None,
        transcript_status: None,
        transcript_absence_reason: None,
        receipt_artifact_path: None,
        failed_stage_evidence_path: None,
        next_operator_action: PublicEnumReadback::from_required(
            "none",
            NEXT_OPERATOR_ACTION_VALUES,
        ),
    }
}

fn p090_failure_envelope_authority_from_capture_json(json: Option<&str>) -> Option<String> {
    let value = serde_json::from_str::<serde_json::Value>(json?).ok()?;
    value
        .get("failure_envelope_authority")
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

fn derive_next_operator_action<'a>(
    receipt: &CodeWriterCompletionReceiptRecord,
    text_captures: &[CodeWriterCompletionTextCaptureRecord],
    status: &PublicEnumReadback,
    ingestion_boundary_failure: &PublicEnumReadback,
    completion_turn_result: &PublicEnumReadback,
) -> &'a str {
    if !status.known || !ingestion_boundary_failure.known || !completion_turn_result.known {
        return "unknown";
    }
    if status.value == "succeeded" {
        return "none";
    }
    if receipt.work_change_kind.as_deref() == Some("preexisting_dirty_work") {
        return "do_not_retry_preexisting_dirty_timeout";
    }
    if text_captures
        .iter()
        .any(|capture| capture.completion_text_truncated || capture.extraction_input_truncated)
    {
        return "inspect_truncated_completion_text";
    }
    match ingestion_boundary_failure.value.as_str() {
        "acp_final_text_not_collected" => "fix_acp_final_text_collection",
        "chainworks_output_not_extracted" => "fix_chainworks_output_extraction",
        "declared_output_settlement_rejected_usable_payload" => "fix_declared_output_settlement",
        "terminal_response_capture_truncated_before_output" | "extraction_input_truncated" => {
            "inspect_truncated_completion_text"
        }
        _ if receipt.completion_turn_attempted
            && (receipt.missing_required_output_count > 0
                || receipt.stale_required_output_count > 0) =>
        {
            "inspect_prompt_and_expected_output_contract"
        }
        _ if receipt.work_change_kind.as_deref() == Some("current_attempt_diff") => {
            "retry_with_completion_recovery"
        }
        _ => "inspect_outputs_then_retry",
    }
}

impl From<CodeWriterCompletionTextCaptureRecord> for ImplementationCompletionTextCaptureReadback {
    fn from(capture: CodeWriterCompletionTextCaptureRecord) -> Self {
        Self {
            prompt_kind: capture.prompt_kind,
            turn_index: capture.turn_index,
            terminal_response_status: capture.terminal_response_status,
            completion_text_status: capture.completion_text_status,
            completion_text_capture_source: capture.completion_text_capture_source,
            completion_text_raw_byte_limit: capture.completion_text_raw_byte_limit,
            completion_text_captured_byte_count: capture.completion_text_captured_byte_count,
            completion_text_truncated: capture.completion_text_truncated,
            extraction_input_truncated: capture.extraction_input_truncated,
            extraction_input_sha256: capture.extraction_input_sha256,
            redacted_text_artifact_path: capture.redacted_text_artifact_path,
            text_absence_reason: capture.text_absence_reason,
            created_at: capture.created_at,
        }
    }
}
