//! P017 Phase B/C: Lead conflict mediation domain types.
//!
//! This module defines the durable mediation lifecycle, operator confirmation,
//! settlement, and owner-aware execution identity types required by P017.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

// ── Owner-aware execution identity ─────────────────────────────────────

/// Execution ownership discriminator. Every persistence, quota, retry,
/// source-generation, transcript, runtime-facts, and readback path must
/// key off durable owner identity.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OwnerKind {
    StageExecution,
    LeadConflictMediation,
}

impl std::fmt::Display for OwnerKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            OwnerKind::StageExecution => "stage_execution",
            OwnerKind::LeadConflictMediation => "lead_conflict_mediation",
        })
    }
}

impl std::str::FromStr for OwnerKind {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "stage_execution" => OwnerKind::StageExecution,
            "lead_conflict_mediation" => OwnerKind::LeadConflictMediation,
            other => return Err(format!("Unknown OwnerKind: {other}")),
        })
    }
}

// ── Lead conflict mediation lifecycle ──────────────────────────────────

/// Status enum for the `lead_conflict_mediations` table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LeadMediationStatus {
    Pending,
    Queued,
    Running,
    AwaitingOutputValidation,
    OperatorConfirmationRequired,
    Settled,
    TerminalUnverifiable,
    Canceled,
    Superseded,
}

impl std::fmt::Display for LeadMediationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            LeadMediationStatus::Pending => "pending",
            LeadMediationStatus::Queued => "queued",
            LeadMediationStatus::Running => "running",
            LeadMediationStatus::AwaitingOutputValidation => "awaiting_output_validation",
            LeadMediationStatus::OperatorConfirmationRequired => "operator_confirmation_required",
            LeadMediationStatus::Settled => "settled",
            LeadMediationStatus::TerminalUnverifiable => "terminal_unverifiable",
            LeadMediationStatus::Canceled => "canceled",
            LeadMediationStatus::Superseded => "superseded",
        })
    }
}

impl std::str::FromStr for LeadMediationStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "pending" => LeadMediationStatus::Pending,
            "queued" => LeadMediationStatus::Queued,
            "running" => LeadMediationStatus::Running,
            "awaiting_output_validation" => LeadMediationStatus::AwaitingOutputValidation,
            "operator_confirmation_required" => LeadMediationStatus::OperatorConfirmationRequired,
            "settled" => LeadMediationStatus::Settled,
            "terminal_unverifiable" => LeadMediationStatus::TerminalUnverifiable,
            "canceled" => LeadMediationStatus::Canceled,
            "superseded" => LeadMediationStatus::Superseded,
            other => return Err(format!("Unknown LeadMediationStatus: {other}")),
        })
    }
}

impl LeadMediationStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            LeadMediationStatus::Settled
                | LeadMediationStatus::TerminalUnverifiable
                | LeadMediationStatus::Canceled
                | LeadMediationStatus::Superseded
        )
    }

    pub fn graphql_name(&self) -> &'static str {
        match self {
            LeadMediationStatus::Pending => "PENDING",
            LeadMediationStatus::Queued => "QUEUED",
            LeadMediationStatus::Running => "RUNNING",
            LeadMediationStatus::AwaitingOutputValidation => "AWAITING_OUTPUT_VALIDATION",
            LeadMediationStatus::OperatorConfirmationRequired => "OPERATOR_CONFIRMATION_REQUIRED",
            LeadMediationStatus::Settled => "SETTLED",
            LeadMediationStatus::TerminalUnverifiable => "TERMINAL_UNVERIFIABLE",
            LeadMediationStatus::Canceled => "CANCELED",
            LeadMediationStatus::Superseded => "SUPERSEDED",
        }
    }
}

/// Durable lead conflict mediation record.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeadConflictMediationRecord {
    pub id: String,
    pub run_id: String,
    pub conflict_id: String,
    pub conflict_fingerprint: String,
    pub lead_agent_id: String,
    pub status: LeadMediationStatus,
    pub settlement_result: Option<String>,
    pub recovery_action: Option<String>,
    pub chosen_action: Option<String>,
    pub chosen_next_state_id: Option<String>,
    pub chosen_next_state_label: Option<String>,
    pub operator_rationale: Option<String>,
    pub sanitized_progress: Option<String>,
    pub validation_errors_json: Option<String>,
    pub cost_summary_json: Option<String>,
    pub metric_event_id: Option<String>,
    pub superseded_by_event_ref: Option<String>,
    pub agent_execution_id: Option<String>,
    pub confirmation_subject_id: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub settled_at: Option<DateTime<Utc>>,
}

// ── Operator confirmation for mediation ────────────────────────────────

/// Status enum for the `lead_mediation_confirmations` table.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediationConfirmationStatus {
    Pending,
    Resolved,
    Superseded,
    Expired,
    Canceled,
}

impl std::fmt::Display for MediationConfirmationStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MediationConfirmationStatus::Pending => "pending",
            MediationConfirmationStatus::Resolved => "resolved",
            MediationConfirmationStatus::Superseded => "superseded",
            MediationConfirmationStatus::Expired => "expired",
            MediationConfirmationStatus::Canceled => "canceled",
        })
    }
}

impl std::str::FromStr for MediationConfirmationStatus {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "pending" => MediationConfirmationStatus::Pending,
            "resolved" => MediationConfirmationStatus::Resolved,
            "superseded" => MediationConfirmationStatus::Superseded,
            "expired" => MediationConfirmationStatus::Expired,
            "canceled" => MediationConfirmationStatus::Canceled,
            other => return Err(format!("Unknown MediationConfirmationStatus: {other}")),
        })
    }
}

/// Durable mediation confirmation record (separate store from stage approvals).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LeadMediationConfirmation {
    pub id: String,
    pub mediation_record_id: String,
    pub run_id: String,
    pub conflict_id: String,
    pub conflict_fingerprint: String,
    pub status: MediationConfirmationStatus,
    pub suggested_action: Option<String>,
    pub requested_at: DateTime<Utc>,
    pub deadline_at: Option<DateTime<Utc>>,
    pub readback_ref: Option<String>,
    pub idempotency_scope_key: Option<String>,
    pub resolved_at: Option<DateTime<Utc>>,
    pub resolved_by_principal_id: Option<String>,
    pub resolution_decision: Option<String>,
    pub resolution_comment: Option<String>,
}

// ── Mediation confirmation decision codes ──────────────────────────────

/// Decision codes for resolving a lead mediation confirmation via
/// `approvals.resolve` with `subject_kind=lead_mediation_confirmation`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediationConfirmationDecision {
    Confirm,
    ManualFallback,
}

impl std::fmt::Display for MediationConfirmationDecision {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MediationConfirmationDecision::Confirm => "confirm",
            MediationConfirmationDecision::ManualFallback => "manual_fallback",
        })
    }
}

impl std::str::FromStr for MediationConfirmationDecision {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "confirm" => MediationConfirmationDecision::Confirm,
            "manual_fallback" => MediationConfirmationDecision::ManualFallback,
            other => return Err(format!("Unknown MediationConfirmationDecision: {other}")),
        })
    }
}

// ── MCP inbox union types ──────────────────────────────────────────────

/// Subject kind discriminator for the mixed approval inbox returned by
/// `approvals.list`.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalSubjectKind {
    StageApproval,
    LeadMediationConfirmation,
}

impl std::fmt::Display for ApprovalSubjectKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            ApprovalSubjectKind::StageApproval => "stage_approval",
            ApprovalSubjectKind::LeadMediationConfirmation => "lead_mediation_confirmation",
        })
    }
}

/// Unified inbox item returned by `approvals.list`. Carries the union
/// of fields from both stage approvals and mediation confirmations.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ApprovalInboxItem {
    pub subject_kind: ApprovalSubjectKind,
    pub subject_id: String,
    pub run_id: String,
    pub status: String,
    pub requested_at: String,
    pub deadline_at: Option<String>,
    pub readback_ref: Option<String>,
    // Stage-approval-specific fields
    pub stage_id: Option<String>,
    pub decision: Option<String>,
    // Mediation-confirmation-specific fields
    pub conflict_id: Option<String>,
    pub conflict_fingerprint: Option<String>,
    pub suggested_action: Option<String>,
    pub resolution_mode: Option<String>,
}

// ── Resolution mode for readback ───────────────────────────────────────

/// Resolution mode exposed in mediation readback. Describes how the
/// mediation was or will be resolved.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MediationResolutionMode {
    SystemLead,
    OperatorConfirmation,
    AutoSettled,
    Expired,
    Superseded,
    Canceled,
}

impl std::fmt::Display for MediationResolutionMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            MediationResolutionMode::SystemLead => "system_lead",
            MediationResolutionMode::OperatorConfirmation => "operator_confirmation",
            MediationResolutionMode::AutoSettled => "auto_settled",
            MediationResolutionMode::Expired => "expired",
            MediationResolutionMode::Superseded => "superseded",
            MediationResolutionMode::Canceled => "canceled",
        })
    }
}

// ── Shared resolution_mode derivation ─────────────────────────────────

/// Derive the resolution_mode string for a mediation record.
/// Used by both MCP approvals.list and GraphQL readback to ensure
/// consistent derivation (MC-004).
pub fn derive_resolution_mode(record: &LeadConflictMediationRecord) -> Option<String> {
    Some(
        match record.status {
            LeadMediationStatus::OperatorConfirmationRequired => "operator_confirmation",
            LeadMediationStatus::Settled => {
                // If settled with a settlement_result, it went through operator confirmation
                if record.settlement_result.is_some() {
                    "operator_confirmation"
                } else {
                    "system_lead"
                }
            }
            LeadMediationStatus::TerminalUnverifiable => {
                record.recovery_action.as_deref().unwrap_or("expired")
            }
            LeadMediationStatus::Superseded => "superseded",
            LeadMediationStatus::Canceled => "canceled",
            // For active states (Pending, Queued, Running, AwaitingOutputValidation),
            // the resolution mode is "system_lead" (the lead is working on it).
            _ => "system_lead",
        }
        .to_string(),
    )
}

// ── Cost summary ───────────────────────────────────────────────────────

/// Structured cost summary for mediation readback, aligned to existing
/// cents-based cost breakdown conventions.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediationCostSummary {
    pub total_cost_cents: i64,
    pub input_tokens: Option<i64>,
    pub output_tokens: Option<i64>,
    pub cached_input_tokens: Option<i64>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_kind_roundtrip() {
        for kind in &[OwnerKind::StageExecution, OwnerKind::LeadConflictMediation] {
            let s = kind.to_string();
            let parsed: OwnerKind = s.parse().unwrap();
            assert_eq!(kind, &parsed);
        }
    }

    #[test]
    fn lead_mediation_status_terminal() {
        assert!(!LeadMediationStatus::Pending.is_terminal());
        assert!(!LeadMediationStatus::Running.is_terminal());
        assert!(LeadMediationStatus::Settled.is_terminal());
        assert!(LeadMediationStatus::TerminalUnverifiable.is_terminal());
        assert!(LeadMediationStatus::Canceled.is_terminal());
        assert!(LeadMediationStatus::Superseded.is_terminal());
    }

    #[test]
    fn mediation_confirmation_decision_roundtrip() {
        for d in &[
            MediationConfirmationDecision::Confirm,
            MediationConfirmationDecision::ManualFallback,
        ] {
            let s = d.to_string();
            let parsed: MediationConfirmationDecision = s.parse().unwrap();
            assert_eq!(d, &parsed);
        }
    }
}
