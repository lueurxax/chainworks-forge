//! P065: Operator Retry Instruction domain types.
//!
//! Durable one-shot operator guidance bound to a specific retry attempt.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::ids::{AgentExecutionId, RunId, StageExecutionId};

// ── Scope kind ────────────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryInstructionScopeKind {
    FullStageRetry,
    TargetedRetry,
    TargetedRetryFallbackFullStage,
}

impl std::fmt::Display for RetryInstructionScopeKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::FullStageRetry => write!(f, "full_stage_retry"),
            Self::TargetedRetry => write!(f, "targeted_retry"),
            Self::TargetedRetryFallbackFullStage => {
                write!(f, "targeted_retry_fallback_full_stage")
            }
        }
    }
}

impl std::str::FromStr for RetryInstructionScopeKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "full_stage_retry" => Ok(Self::FullStageRetry),
            "targeted_retry" => Ok(Self::TargetedRetry),
            "targeted_retry_fallback_full_stage" => Ok(Self::TargetedRetryFallbackFullStage),
            _ => Err(format!("unknown retry instruction scope kind: {s}")),
        }
    }
}

// ── Delivery status ───────────────────────────────────────────────────

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RetryInstructionDeliveryStatus {
    Pending,
    Delivered,
    Failed,
}

impl std::fmt::Display for RetryInstructionDeliveryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Pending => write!(f, "pending"),
            Self::Delivered => write!(f, "delivered"),
            Self::Failed => write!(f, "failed"),
        }
    }
}

impl std::str::FromStr for RetryInstructionDeliveryStatus {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "pending" => Ok(Self::Pending),
            "delivered" => Ok(Self::Delivered),
            "failed" => Ok(Self::Failed),
            _ => Err(format!("unknown retry instruction delivery status: {s}")),
        }
    }
}

// ── Parent binding row ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryOperatorInstructionBinding {
    pub binding_id: String,
    pub journal_id: String,
    pub run_id: RunId,
    pub stage_id: String,
    pub source_stage_execution_id: StageExecutionId,
    pub retry_stage_execution_id: StageExecutionId,
    pub retry_attempt_number: i64,
    pub target_agent_execution_id: Option<AgentExecutionId>,
    pub scope_kind: RetryInstructionScopeKind,
    pub instruction_text: String,
    pub instruction_sha256: String,
    pub created_by_principal_id: String,
    pub created_by_principal_class: String,
    pub created_at: DateTime<Utc>,
}

// ── Child delivery row ────────────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RetryOperatorInstructionDelivery {
    pub delivery_id: String,
    pub binding_id: String,
    pub work_item_id: Option<String>,
    pub agent_execution_id: Option<AgentExecutionId>,
    pub status: RetryInstructionDeliveryStatus,
    pub failure_reason: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub delivered_at: Option<DateTime<Utc>>,
}

// ── Input for creating a binding ──────────────────────────────────────

#[derive(Clone, Debug)]
pub struct RetryInstructionBindingInput {
    pub journal_id: String,
    pub run_id: RunId,
    pub stage_id: String,
    pub source_stage_execution_id: StageExecutionId,
    pub retry_stage_execution_id: StageExecutionId,
    pub retry_attempt_number: i64,
    pub target_agent_execution_id: Option<AgentExecutionId>,
    pub scope_kind: RetryInstructionScopeKind,
    pub instruction_text: String,
    pub created_by_principal_id: String,
    pub created_by_principal_class: String,
}

// ── Work-item payload metadata ────────────────────────────────────────

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OperatorRetryInstructionPayload {
    pub binding_id: String,
    pub journal_id: String,
    pub scope_kind: RetryInstructionScopeKind,
    pub instruction: String,
    pub instruction_sha256: String,
}

// ── Validation ────────────────────────────────────────────────────────

/// Validate and normalize an operator instruction string.
///
/// Rules per P065 §1:
/// - Trim leading/trailing whitespace
/// - Reject empty after trim
/// - Reject > 2000 chars after trim
/// - Reject ASCII control characters except tab (0x09) and newline (0x0A)
pub fn validate_operator_instruction(raw: &str) -> Result<String, OperatorInstructionError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(OperatorInstructionError::Empty);
    }
    if trimmed.len() > 2000 {
        return Err(OperatorInstructionError::TooLong(trimmed.len()));
    }
    for (i, ch) in trimmed.chars().enumerate() {
        if ch.is_ascii_control() && ch != '\t' && ch != '\n' {
            return Err(OperatorInstructionError::ForbiddenControl { position: i, ch });
        }
    }
    Ok(trimmed.to_string())
}

#[derive(Debug, Clone)]
pub enum OperatorInstructionError {
    Empty,
    TooLong(usize),
    ForbiddenControl { position: usize, ch: char },
}

impl std::fmt::Display for OperatorInstructionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => write!(f, "operator instruction is empty after trim"),
            Self::TooLong(len) => write!(
                f,
                "operator instruction is {len} chars after trim (max 2000)"
            ),
            Self::ForbiddenControl { position, ch } => write!(
                f,
                "operator instruction contains forbidden control character U+{:04X} at position {position}",
                *ch as u32
            ),
        }
    }
}

impl std::error::Error for OperatorInstructionError {}

// ── SHA-256 helper ────────────────────────────────────────────────────

pub fn instruction_sha256(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    format!("{digest:x}")
}

// ── Tests ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validate_trims_and_accepts_normal_text() {
        let result = validate_operator_instruction("  Focus on the GraphQL slice.  ");
        assert_eq!(result.unwrap(), "Focus on the GraphQL slice.");
    }

    #[test]
    fn validate_allows_tab_and_newline() {
        let result = validate_operator_instruction("line one\n\tline two");
        assert!(result.is_ok());
    }

    #[test]
    fn validate_rejects_empty_after_trim() {
        assert!(matches!(
            validate_operator_instruction("   "),
            Err(OperatorInstructionError::Empty)
        ));
    }

    #[test]
    fn validate_rejects_over_2000_chars() {
        let long = "a".repeat(2001);
        assert!(matches!(
            validate_operator_instruction(&long),
            Err(OperatorInstructionError::TooLong(2001))
        ));
    }

    #[test]
    fn validate_accepts_exactly_2000_chars() {
        let exact = "a".repeat(2000);
        assert!(validate_operator_instruction(&exact).is_ok());
    }

    #[test]
    fn validate_rejects_null_byte() {
        assert!(matches!(
            validate_operator_instruction("hello\0world"),
            Err(OperatorInstructionError::ForbiddenControl { .. })
        ));
    }

    #[test]
    fn validate_rejects_bell_character() {
        assert!(matches!(
            validate_operator_instruction("hello\x07world"),
            Err(OperatorInstructionError::ForbiddenControl { position: 5, .. })
        ));
    }

    #[test]
    fn instruction_sha256_is_deterministic() {
        let a = instruction_sha256("Focus on GraphQL");
        let b = instruction_sha256("Focus on GraphQL");
        assert_eq!(a, b);
        assert_eq!(a.len(), 64); // hex SHA-256
    }

    #[test]
    fn instruction_sha256_changes_with_content() {
        let a = instruction_sha256("Focus on GraphQL");
        let b = instruction_sha256("Focus on MCP readback");
        assert_ne!(a, b);
    }

    #[test]
    fn scope_kind_roundtrip() {
        for kind in &[
            RetryInstructionScopeKind::FullStageRetry,
            RetryInstructionScopeKind::TargetedRetry,
            RetryInstructionScopeKind::TargetedRetryFallbackFullStage,
        ] {
            let s = kind.to_string();
            let parsed: RetryInstructionScopeKind = s.parse().unwrap();
            assert_eq!(&parsed, kind);
        }
    }

    #[test]
    fn delivery_status_roundtrip() {
        for status in &[
            RetryInstructionDeliveryStatus::Pending,
            RetryInstructionDeliveryStatus::Delivered,
            RetryInstructionDeliveryStatus::Failed,
        ] {
            let s = status.to_string();
            let parsed: RetryInstructionDeliveryStatus = s.parse().unwrap();
            assert_eq!(&parsed, status);
        }
    }
}
