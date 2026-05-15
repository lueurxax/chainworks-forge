use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;

use crate::ids::{RunId, StageExecutionId};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryAuthorityEntryKind {
    FullStageRetry,
    TargetedAgentRetry,
    HistoricalOrphanRecovery,
}

impl std::fmt::Display for RetryAuthorityEntryKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullStageRetry => write!(f, "full_stage_retry"),
            Self::TargetedAgentRetry => write!(f, "targeted_agent_retry"),
            Self::HistoricalOrphanRecovery => write!(f, "historical_orphan_recovery"),
        }
    }
}

impl std::str::FromStr for RetryAuthorityEntryKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "full_stage_retry" => Ok(Self::FullStageRetry),
            "targeted_agent_retry" => Ok(Self::TargetedAgentRetry),
            "historical_orphan_recovery" => Ok(Self::HistoricalOrphanRecovery),
            other => Err(format!("Unknown RetryAuthorityEntryKind: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryAuthorityState {
    Active,
    Terminalized,
    Superseded,
    RecoveredOrphan,
    Invalid,
}

impl std::fmt::Display for RetryAuthorityState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Active => write!(f, "active"),
            Self::Terminalized => write!(f, "terminalized"),
            Self::Superseded => write!(f, "superseded"),
            Self::RecoveredOrphan => write!(f, "recovered_orphan"),
            Self::Invalid => write!(f, "invalid"),
        }
    }
}

impl std::str::FromStr for RetryAuthorityState {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "active" => Ok(Self::Active),
            "terminalized" => Ok(Self::Terminalized),
            "superseded" => Ok(Self::Superseded),
            "recovered_orphan" => Ok(Self::RecoveredOrphan),
            "invalid" => Ok(Self::Invalid),
            other => Err(format!("Unknown RetryAuthorityState: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryStageExecutionAuthority {
    pub id: String,
    pub run_id: RunId,
    pub stage_id: String,
    pub target_stage_execution_id: StageExecutionId,
    pub entry_kind: RetryAuthorityEntryKind,
    pub source_command_journal_id: Option<String>,
    pub source_retry_work_item_id: Option<String>,
    pub source_invoke_work_item_id: Option<String>,
    pub source_agent_execution_id: Option<String>,
    pub authority_state: RetryAuthorityState,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub terminal_reason: Option<String>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvanceRunTargetMode {
    LegacyRunScoped,
    TargetedRetry,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AdvanceRunEnqueueReason {
    NormalAdvance,
    RetryStage,
    TargetedAgentRetry,
    PostInvokeCompletion,
    PostInvokeFailure,
    StartupRecovery,
    AbandonedAdvanceRequeue,
}

impl std::fmt::Display for AdvanceRunEnqueueReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NormalAdvance => write!(f, "normal_advance"),
            Self::RetryStage => write!(f, "retry_stage"),
            Self::TargetedAgentRetry => write!(f, "targeted_agent_retry"),
            Self::PostInvokeCompletion => write!(f, "post_invoke_completion"),
            Self::PostInvokeFailure => write!(f, "post_invoke_failure"),
            Self::StartupRecovery => write!(f, "startup_recovery"),
            Self::AbandonedAdvanceRequeue => write!(f, "abandoned_advance_requeue"),
        }
    }
}

impl std::str::FromStr for AdvanceRunEnqueueReason {
    type Err = String;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "normal_advance" => Ok(Self::NormalAdvance),
            "retry_stage" => Ok(Self::RetryStage),
            "targeted_agent_retry" => Ok(Self::TargetedAgentRetry),
            "post_invoke_completion" => Ok(Self::PostInvokeCompletion),
            "post_invoke_failure" => Ok(Self::PostInvokeFailure),
            "startup_recovery" => Ok(Self::StartupRecovery),
            "abandoned_advance_requeue" => Ok(Self::AbandonedAdvanceRequeue),
            other => Err(format!("Unknown AdvanceRunEnqueueReason: {other}")),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AdvanceRunPayloadV1 {
    pub schema_version: Option<String>,
    pub run_id: RunId,
    pub stage_id: Option<String>,
    pub target_stage_execution_id: Option<StageExecutionId>,
    pub retry_authority_id: Option<String>,
    pub source_stage_execution_id: Option<StageExecutionId>,
    pub source_work_item_id: Option<String>,
    pub source_invoke_work_item_id: Option<String>,
    pub source_agent_execution_id: Option<String>,
    pub enqueue_reason: Option<AdvanceRunEnqueueReason>,
    pub reason: Option<String>,
}

#[derive(Clone, Debug, Error, PartialEq, Eq)]
pub enum AdvanceRunPayloadError {
    #[error("advance_run payload is not an object")]
    NotObject,
    #[error("advance_run payload is missing run_id")]
    MissingRunId,
    #[error("advance_run payload has invalid run_id")]
    InvalidRunId,
    #[error("advance_run payload has unsupported schema_version")]
    UnsupportedSchemaVersion,
    #[error("advance_run payload has invalid {field}")]
    InvalidId { field: &'static str },
    #[error("advance_run targeted retry payload is partial: {field} is missing")]
    MissingTargetField { field: &'static str },
    #[error("advance_run payload has invalid enqueue_reason")]
    InvalidEnqueueReason,
    #[error("advance_run payload has schema_version but no enqueue_reason")]
    MissingEnqueueReason,
    #[error("advance_run payload uses targeted fields for non-targeted enqueue_reason")]
    TargetedFieldsNotAllowed,
    #[error("advance_run payload lost target fields for retry-owned source work")]
    TargetLost,
    #[error("advance_run payload target is required for retry-linked source")]
    TargetRequired,
    #[error("advance_run post-invoke payload is missing {field}")]
    MissingPostInvokeField { field: &'static str },
    #[error("advance_run post-invoke source stage does not match target stage")]
    SourceTargetMismatch,
    #[error("advance_run enqueue_reason is invalid for this work item")]
    InvalidEntryKind,
}

impl AdvanceRunPayloadError {
    pub fn code(&self) -> &'static str {
        match self {
            Self::NotObject => "advance_run_payload_malformed",
            Self::MissingRunId => "advance_run_payload_missing_run_id",
            Self::InvalidRunId => "advance_run_payload_invalid_run_id",
            Self::UnsupportedSchemaVersion => "advance_run_payload_unsupported_schema_version",
            Self::InvalidId {
                field: "target_stage_execution_id",
            } => "advance_run_payload_invalid_target_stage_execution_id",
            Self::InvalidId {
                field: "source_stage_execution_id",
            } => "advance_run_payload_invalid_source_stage_execution_id",
            Self::InvalidId { .. } => "advance_run_payload_invalid_id",
            Self::MissingTargetField { field: "stage_id" } => {
                "advance_run_payload_missing_stage_id"
            }
            Self::MissingTargetField {
                field: "retry_authority_id",
            } => "advance_run_payload_missing_retry_authority",
            Self::MissingTargetField {
                field: "target_stage_execution_id",
            } => "advance_run_payload_missing_target_for_authority",
            Self::MissingTargetField { .. } => "advance_run_payload_missing_target_field",
            Self::InvalidEnqueueReason => "advance_run_payload_invalid_enqueue_reason",
            Self::MissingEnqueueReason => "advance_run_payload_missing_enqueue_reason",
            Self::TargetedFieldsNotAllowed => {
                "advance_run_payload_target_not_allowed_for_normal_advance"
            }
            Self::TargetLost => "advance_run_payload_target_lost",
            Self::TargetRequired => "advance_run_payload_target_required",
            Self::MissingPostInvokeField {
                field: "source_invoke_work_item_id",
            } => "advance_run_payload_missing_source_invoke_work_item_id",
            Self::MissingPostInvokeField {
                field: "source_stage_execution_id",
            } => "advance_run_payload_missing_source_stage_execution_id",
            Self::MissingPostInvokeField { .. } => "advance_run_payload_missing_post_invoke_field",
            Self::SourceTargetMismatch => "advance_run_source_target_mismatch",
            Self::InvalidEntryKind => "advance_run_invalid_entry_kind",
        }
    }
}

impl AdvanceRunPayloadV1 {
    pub fn parse_json(raw: &str) -> Result<Self, AdvanceRunPayloadError> {
        let value: Value =
            serde_json::from_str(raw).map_err(|_| AdvanceRunPayloadError::NotObject)?;
        Self::parse_value(&value)
    }

    pub fn parse_value(value: &Value) -> Result<Self, AdvanceRunPayloadError> {
        let object = value.as_object().ok_or(AdvanceRunPayloadError::NotObject)?;
        let run_id = object
            .get("run_id")
            .and_then(Value::as_str)
            .ok_or(AdvanceRunPayloadError::MissingRunId)?
            .parse::<RunId>()
            .map_err(|_| AdvanceRunPayloadError::InvalidRunId)?;
        let schema_version = optional_string(object.get("schema_version"));
        if !matches!(
            schema_version.as_deref(),
            None | Some("advance_run_payload.v1")
        ) {
            return Err(AdvanceRunPayloadError::UnsupportedSchemaVersion);
        }
        let stage_id = optional_string(object.get("stage_id"));
        let target_stage_execution_id = optional_uuid_string(
            object.get("target_stage_execution_id"),
            "target_stage_execution_id",
        )?;
        let source_stage_execution_id = optional_uuid_string(
            object.get("source_stage_execution_id"),
            "source_stage_execution_id",
        )?;
        let retry_authority_id = optional_string(object.get("retry_authority_id"));
        let raw_enqueue_reason = optional_string(object.get("enqueue_reason"));
        let enqueue_reason = raw_enqueue_reason
            .as_deref()
            .map(|raw| raw.parse::<AdvanceRunEnqueueReason>())
            .transpose()
            .map_err(|_| AdvanceRunPayloadError::InvalidEnqueueReason)?;
        let payload = Self {
            schema_version,
            run_id,
            stage_id,
            target_stage_execution_id,
            retry_authority_id,
            source_stage_execution_id,
            source_work_item_id: optional_string(object.get("source_work_item_id")),
            source_invoke_work_item_id: optional_string(object.get("source_invoke_work_item_id")),
            source_agent_execution_id: optional_string(object.get("source_agent_execution_id")),
            enqueue_reason,
            reason: optional_string(object.get("reason")),
        };
        payload.validate()?;
        Ok(payload)
    }

    pub fn target_mode(&self) -> AdvanceRunTargetMode {
        if self.target_stage_execution_id.is_some() {
            AdvanceRunTargetMode::TargetedRetry
        } else {
            AdvanceRunTargetMode::LegacyRunScoped
        }
    }

    pub fn validate(&self) -> Result<(), AdvanceRunPayloadError> {
        if self.schema_version.is_some() && self.enqueue_reason.is_none() {
            return Err(AdvanceRunPayloadError::MissingEnqueueReason);
        }
        if matches!(
            self.enqueue_reason,
            Some(AdvanceRunEnqueueReason::TargetedAgentRetry)
        ) {
            return Err(AdvanceRunPayloadError::InvalidEntryKind);
        }

        let targeted_reason = matches!(
            self.enqueue_reason,
            Some(
                AdvanceRunEnqueueReason::RetryStage
                    | AdvanceRunEnqueueReason::PostInvokeCompletion
                    | AdvanceRunEnqueueReason::PostInvokeFailure
                    | AdvanceRunEnqueueReason::StartupRecovery
                    | AdvanceRunEnqueueReason::AbandonedAdvanceRequeue
            )
        );

        if self.target_stage_execution_id.is_some() {
            if !targeted_reason {
                return Err(AdvanceRunPayloadError::TargetedFieldsNotAllowed);
            }
            if self.stage_id.as_deref().unwrap_or_default().is_empty() {
                return Err(AdvanceRunPayloadError::MissingTargetField { field: "stage_id" });
            }
            if self
                .retry_authority_id
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                return Err(AdvanceRunPayloadError::MissingTargetField {
                    field: "retry_authority_id",
                });
            }
            if matches!(
                self.enqueue_reason,
                Some(
                    AdvanceRunEnqueueReason::PostInvokeCompletion
                        | AdvanceRunEnqueueReason::PostInvokeFailure
                )
            ) && self
                .source_invoke_work_item_id
                .as_deref()
                .unwrap_or_default()
                .is_empty()
            {
                return Err(AdvanceRunPayloadError::MissingPostInvokeField {
                    field: "source_invoke_work_item_id",
                });
            }
            if matches!(
                self.enqueue_reason,
                Some(
                    AdvanceRunEnqueueReason::PostInvokeCompletion
                        | AdvanceRunEnqueueReason::PostInvokeFailure
                )
            ) {
                let source_stage_execution_id = self.source_stage_execution_id.ok_or(
                    AdvanceRunPayloadError::MissingPostInvokeField {
                        field: "source_stage_execution_id",
                    },
                )?;
                if Some(source_stage_execution_id) != self.target_stage_execution_id {
                    return Err(AdvanceRunPayloadError::SourceTargetMismatch);
                }
            }
            return Ok(());
        }

        if self.retry_authority_id.is_some()
            || self.source_stage_execution_id.is_some()
            || self.source_invoke_work_item_id.is_some()
            || self.source_agent_execution_id.is_some()
        {
            return Err(AdvanceRunPayloadError::MissingTargetField {
                field: "target_stage_execution_id",
            });
        }
        if self.schema_version.is_some()
            && self.source_work_item_id.is_some()
            && targeted_reason
            && self.stage_id.is_none()
        {
            return Err(AdvanceRunPayloadError::TargetLost);
        }
        if self.schema_version.is_some()
            && self.source_work_item_id.is_some()
            && targeted_reason
            && self.stage_id.is_some()
        {
            return Err(AdvanceRunPayloadError::TargetRequired);
        }
        Ok(())
    }
}

fn optional_string(value: Option<&Value>) -> Option<String> {
    value.and_then(Value::as_str).map(str::to_owned)
}

fn optional_uuid_string(
    value: Option<&Value>,
    field: &'static str,
) -> Result<Option<StageExecutionId>, AdvanceRunPayloadError> {
    value
        .and_then(Value::as_str)
        .map(|raw| {
            raw.parse::<StageExecutionId>()
                .map_err(|_| AdvanceRunPayloadError::InvalidId { field })
        })
        .transpose()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn run_scoped_advance_payload_remains_valid_for_legacy_work() {
        let run_id = RunId::new();
        let payload = AdvanceRunPayloadV1::parse_value(&json!({
            "run_id": run_id.to_string(),
            "reason": "startup_catchup"
        }))
        .unwrap();

        assert_eq!(payload.run_id, run_id);
        assert_eq!(payload.target_mode(), AdvanceRunTargetMode::LegacyRunScoped);
    }

    #[test]
    fn targeted_advance_payload_requires_full_authority_triplet() {
        let run_id = RunId::new();
        let target = StageExecutionId::new();

        let missing_authority = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "target_stage_execution_id": target.to_string(),
            "enqueue_reason": "retry_stage"
        }))
        .unwrap_err();
        assert!(matches!(
            missing_authority,
            AdvanceRunPayloadError::MissingTargetField {
                field: "retry_authority_id"
            }
        ));

        let missing_target = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "retry_authority_id": "auth-1",
            "enqueue_reason": "retry_stage"
        }))
        .unwrap_err();
        assert!(matches!(
            missing_target,
            AdvanceRunPayloadError::MissingTargetField {
                field: "target_stage_execution_id"
            }
        ));

        let payload = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "target_stage_execution_id": target.to_string(),
            "retry_authority_id": "auth-1",
            "enqueue_reason": "retry_stage"
        }))
        .unwrap();
        assert_eq!(payload.target_mode(), AdvanceRunTargetMode::TargetedRetry);
    }

    #[test]
    fn targeted_advance_payload_rejects_invalid_matrix_rows() {
        let run_id = RunId::new();
        let target = StageExecutionId::new();
        let other = StageExecutionId::new();

        let normal_with_target = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "target_stage_execution_id": target.to_string(),
            "retry_authority_id": "auth-1",
            "enqueue_reason": "normal_advance"
        }))
        .unwrap_err();
        assert!(matches!(
            normal_with_target,
            AdvanceRunPayloadError::TargetedFieldsNotAllowed
        ));
        assert_eq!(
            normal_with_target.code(),
            "advance_run_payload_target_not_allowed_for_normal_advance"
        );

        let targeted_agent_retry_advance = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "target_stage_execution_id": target.to_string(),
            "retry_authority_id": "auth-1",
            "enqueue_reason": "targeted_agent_retry"
        }))
        .unwrap_err();
        assert!(matches!(
            targeted_agent_retry_advance,
            AdvanceRunPayloadError::InvalidEntryKind
        ));
        assert_eq!(
            targeted_agent_retry_advance.code(),
            "advance_run_invalid_entry_kind"
        );

        let targeted_agent_retry_without_target = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "enqueue_reason": "targeted_agent_retry"
        }))
        .unwrap_err();
        assert!(matches!(
            targeted_agent_retry_without_target,
            AdvanceRunPayloadError::InvalidEntryKind
        ));

        let schema_v1_reason_only = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "reason": "retry_stage"
        }))
        .unwrap_err();
        assert!(matches!(
            schema_v1_reason_only,
            AdvanceRunPayloadError::MissingEnqueueReason
        ));
        assert_eq!(
            schema_v1_reason_only.code(),
            "advance_run_payload_missing_enqueue_reason"
        );

        let post_invoke_missing_source = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "target_stage_execution_id": target.to_string(),
            "retry_authority_id": "auth-1",
            "enqueue_reason": "post_invoke_completion"
        }))
        .unwrap_err();
        assert!(matches!(
            post_invoke_missing_source,
            AdvanceRunPayloadError::MissingPostInvokeField {
                field: "source_invoke_work_item_id"
            }
        ));

        let post_invoke_missing_source_stage = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "target_stage_execution_id": target.to_string(),
            "retry_authority_id": "auth-1",
            "source_invoke_work_item_id": "invoke-1",
            "enqueue_reason": "post_invoke_completion"
        }))
        .unwrap_err();
        assert!(matches!(
            post_invoke_missing_source_stage,
            AdvanceRunPayloadError::MissingPostInvokeField {
                field: "source_stage_execution_id"
            }
        ));
        assert_eq!(
            post_invoke_missing_source_stage.code(),
            "advance_run_payload_missing_source_stage_execution_id"
        );

        let source_mismatch = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "target_stage_execution_id": target.to_string(),
            "source_stage_execution_id": other.to_string(),
            "retry_authority_id": "auth-1",
            "source_invoke_work_item_id": "invoke-1",
            "enqueue_reason": "post_invoke_failure"
        }))
        .unwrap_err();
        assert!(matches!(
            source_mismatch,
            AdvanceRunPayloadError::SourceTargetMismatch
        ));

        let target_lost = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "source_work_item_id": "advance-lost-target",
            "enqueue_reason": "retry_stage"
        }))
        .unwrap_err();
        assert!(matches!(target_lost, AdvanceRunPayloadError::TargetLost));
        assert_eq!(target_lost.code(), "advance_run_payload_target_lost");

        let target_required = AdvanceRunPayloadV1::parse_value(&json!({
            "schema_version": "advance_run_payload.v1",
            "run_id": run_id.to_string(),
            "stage_id": "implement",
            "source_work_item_id": "advance-source-only",
            "enqueue_reason": "retry_stage"
        }))
        .unwrap_err();
        assert!(matches!(
            target_required,
            AdvanceRunPayloadError::TargetRequired
        ));
        assert_eq!(
            target_required.code(),
            "advance_run_payload_target_required"
        );
    }
}
