use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::{AgentExecutionId, RunId};

/// Stable tier kind vocabulary for escalation_policy_v1.
/// Raw strings are authoritative for wire/readback; this enum provides typed helpers.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationTierKind {
    SameBackendRetry,
    BackendProfile,
    LeadMediation,
    Pause,
}

impl std::fmt::Display for EscalationTierKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscalationTierKind::SameBackendRetry => write!(f, "same_backend_retry"),
            EscalationTierKind::BackendProfile => write!(f, "backend_profile"),
            EscalationTierKind::LeadMediation => write!(f, "lead_mediation"),
            EscalationTierKind::Pause => write!(f, "pause"),
        }
    }
}

impl std::str::FromStr for EscalationTierKind {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "same_backend_retry" => Ok(EscalationTierKind::SameBackendRetry),
            "backend_profile" => Ok(EscalationTierKind::BackendProfile),
            "lead_mediation" => Ok(EscalationTierKind::LeadMediation),
            "pause" => Ok(EscalationTierKind::Pause),
            other => Err(format!("Unknown EscalationTierKind: {other}")),
        }
    }
}

/// Stable trigger vocabulary for escalation_policy_v1.
/// Raw strings are authoritative for wire/readback; unknown values pass through as-is.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationTrigger {
    RepeatedSameBlockerDigest,
    ContractOutputFailure,
    StaleNoOutput,
    ProviderQuotaExhausted,
    TransportFailure,
    LoopBudgetThreshold,
    OperatorForcedReservedRejected,
}

impl std::fmt::Display for EscalationTrigger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscalationTrigger::RepeatedSameBlockerDigest => {
                write!(f, "repeated_same_blocker_digest")
            }
            EscalationTrigger::ContractOutputFailure => write!(f, "contract_output_failure"),
            EscalationTrigger::StaleNoOutput => write!(f, "stale_no_output"),
            EscalationTrigger::ProviderQuotaExhausted => write!(f, "provider_quota_exhausted"),
            EscalationTrigger::TransportFailure => write!(f, "transport_failure"),
            EscalationTrigger::LoopBudgetThreshold => write!(f, "loop_budget_threshold"),
            EscalationTrigger::OperatorForcedReservedRejected => {
                write!(f, "operator_forced_reserved_rejected")
            }
        }
    }
}

/// Stable pause reason vocabulary for escalation_policy_v1.
/// Raw strings are authoritative; this enum provides typed helpers for Phase 0 compile validation.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EscalationPauseReason {
    EscalationPolicyUnknownBackendProfile,
    EscalationPolicyAmbiguousAtCompile,
    EscalationPolicyUnsafeForSideEffectStage,
    EscalationPolicyDisabled,
    EscalationKillSwitchEngaged,
    EscalationChainExhausted,
    CapacityProbeFailed,
    ProviderSessionForceDetached,
    EscalationRecoveryInconsistent,
    EscalationRepeatedDigestNoProgress,
    EscalationDeadlineElapsed,
    HumanTierDeadlineElapsed,
    EscalationPolicyDrift,
}

impl std::fmt::Display for EscalationPauseReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EscalationPauseReason::EscalationPolicyUnknownBackendProfile => {
                write!(f, "escalation_policy_unknown_backend_profile")
            }
            EscalationPauseReason::EscalationPolicyAmbiguousAtCompile => {
                write!(f, "escalation_policy_ambiguous_at_compile")
            }
            EscalationPauseReason::EscalationPolicyUnsafeForSideEffectStage => {
                write!(f, "escalation_policy_unsafe_for_side_effect_stage")
            }
            EscalationPauseReason::EscalationPolicyDisabled => {
                write!(f, "escalation_policy_disabled")
            }
            EscalationPauseReason::EscalationKillSwitchEngaged => {
                write!(f, "escalation_kill_switch_engaged")
            }
            EscalationPauseReason::EscalationChainExhausted => {
                write!(f, "escalation_chain_exhausted")
            }
            EscalationPauseReason::CapacityProbeFailed => write!(f, "capacity_probe_failed"),
            EscalationPauseReason::ProviderSessionForceDetached => {
                write!(f, "provider_session_force_detached")
            }
            EscalationPauseReason::EscalationRecoveryInconsistent => {
                write!(f, "escalation_recovery_inconsistent")
            }
            EscalationPauseReason::EscalationRepeatedDigestNoProgress => {
                write!(f, "escalation_repeated_digest_no_progress")
            }
            EscalationPauseReason::EscalationDeadlineElapsed => {
                write!(f, "escalation_deadline_elapsed")
            }
            EscalationPauseReason::HumanTierDeadlineElapsed => {
                write!(f, "human_tier_deadline_elapsed")
            }
            EscalationPauseReason::EscalationPolicyDrift => write!(f, "escalation_policy_drift"),
        }
    }
}

impl std::str::FromStr for EscalationPauseReason {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "escalation_policy_unknown_backend_profile" => {
                Ok(EscalationPauseReason::EscalationPolicyUnknownBackendProfile)
            }
            "escalation_policy_ambiguous_at_compile" => {
                Ok(EscalationPauseReason::EscalationPolicyAmbiguousAtCompile)
            }
            "escalation_policy_unsafe_for_side_effect_stage" => {
                Ok(EscalationPauseReason::EscalationPolicyUnsafeForSideEffectStage)
            }
            "escalation_policy_disabled" => Ok(EscalationPauseReason::EscalationPolicyDisabled),
            "escalation_kill_switch_engaged" => {
                Ok(EscalationPauseReason::EscalationKillSwitchEngaged)
            }
            "escalation_chain_exhausted" => Ok(EscalationPauseReason::EscalationChainExhausted),
            "capacity_probe_failed" => Ok(EscalationPauseReason::CapacityProbeFailed),
            "provider_session_force_detached" => {
                Ok(EscalationPauseReason::ProviderSessionForceDetached)
            }
            "escalation_recovery_inconsistent" => {
                Ok(EscalationPauseReason::EscalationRecoveryInconsistent)
            }
            "escalation_repeated_digest_no_progress" => {
                Ok(EscalationPauseReason::EscalationRepeatedDigestNoProgress)
            }
            "escalation_deadline_elapsed" => Ok(EscalationPauseReason::EscalationDeadlineElapsed),
            "human_tier_deadline_elapsed" => Ok(EscalationPauseReason::HumanTierDeadlineElapsed),
            "escalation_policy_drift" => Ok(EscalationPauseReason::EscalationPolicyDrift),
            other => Err(format!("Unknown EscalationPauseReason: {other}")),
        }
    }
}

/// Persisted record for one escalation chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EscalationLedger {
    pub id: String,
    pub run_id: RunId,
    pub stage_id: String,
    /// The concrete stage attempt that owns this chain. Legacy rows predate this field.
    #[serde(default)]
    pub stage_execution_id: Option<String>,
    pub agent_id: String,
    pub policy_id: String,
    pub policy_hash: String,
    /// Raw status string — forward-compatible with future values.
    pub status_raw: String,
    pub current_tier_id: Option<String>,
    pub current_tier_kind_raw: Option<String>,
    pub chain_attempt_index: i64,
    pub trigger_raw: Option<String>,
    pub pause_reason_raw: Option<String>,
    pub operator_action_hint: Option<String>,
    pub runbook_anchor: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// Per-execution escalation attribution row.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EscalationExecutionMetadata {
    pub agent_execution_id: AgentExecutionId,
    pub escalation_ledger_id: String,
    pub tier_id: String,
    pub tier_kind_raw: String,
    pub tier_attempt_index: i64,
    pub trigger_raw: Option<String>,
    pub digest_version: Option<String>,
    pub capacity_probe_counter: i64,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    /// P058 Phase 1b+: Shadow tier selection — which tier would have been selected.
    /// Read from agent_execution_runtime_facts.would_select_tier_id via LEFT JOIN.
    #[serde(default)]
    pub would_select_tier_id: Option<String>,
    /// P058 Phase 1b+: Shadow trigger classification — which trigger would have fired.
    /// Read from agent_execution_runtime_facts.would_select_trigger_raw via LEFT JOIN.
    #[serde(default)]
    pub would_select_trigger_raw: Option<String>,
    /// P058 Phase 1b+: Shadow decision JSON — the full decision context for shadow selection.
    /// Read from agent_execution_runtime_facts.would_select_decision_json via LEFT JOIN.
    /// Redacted and validated before persisting; null until Phase 1b shadow writer is live.
    #[serde(default)]
    pub would_select_decision_json: Option<String>,
}

/// Event journal entry for an escalation chain.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct EscalationEvent {
    pub id: String,
    pub escalation_ledger_id: String,
    pub event_kind_raw: String,
    pub tier_id: Option<String>,
    pub tier_kind_raw: Option<String>,
    pub trigger_raw: Option<String>,
    pub pause_reason_raw: Option<String>,
    /// Optional JSON payload. Repository layer validates this is well-formed JSON when present.
    pub payload_json: Option<String>,
    /// Redaction version stamp for this event's projection. Proposal requires this on every write.
    pub redaction_version: Option<String>,
    pub created_at: DateTime<Utc>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tier_kind_roundtrip() {
        for kind in &[
            EscalationTierKind::SameBackendRetry,
            EscalationTierKind::BackendProfile,
            EscalationTierKind::LeadMediation,
            EscalationTierKind::Pause,
        ] {
            let s = kind.to_string();
            let parsed: EscalationTierKind = s.parse().expect("should parse");
            assert_eq!(kind, &parsed);
        }
    }

    #[test]
    fn pause_reason_roundtrip() {
        let reasons = [
            EscalationPauseReason::EscalationPolicyUnknownBackendProfile,
            EscalationPauseReason::EscalationPolicyAmbiguousAtCompile,
            EscalationPauseReason::EscalationPolicyUnsafeForSideEffectStage,
            EscalationPauseReason::EscalationPolicyDisabled,
            EscalationPauseReason::EscalationKillSwitchEngaged,
            EscalationPauseReason::EscalationChainExhausted,
            EscalationPauseReason::CapacityProbeFailed,
            EscalationPauseReason::ProviderSessionForceDetached,
            EscalationPauseReason::EscalationRecoveryInconsistent,
            EscalationPauseReason::EscalationRepeatedDigestNoProgress,
            EscalationPauseReason::EscalationDeadlineElapsed,
            EscalationPauseReason::HumanTierDeadlineElapsed,
            EscalationPauseReason::EscalationPolicyDrift,
        ];
        for reason in &reasons {
            let s = reason.to_string();
            let parsed: EscalationPauseReason = s.parse().expect("should parse");
            assert_eq!(reason, &parsed);
        }
    }

    #[test]
    fn trigger_display() {
        assert_eq!(
            EscalationTrigger::RepeatedSameBlockerDigest.to_string(),
            "repeated_same_blocker_digest"
        );
        assert_eq!(
            EscalationTrigger::OperatorForcedReservedRejected.to_string(),
            "operator_forced_reserved_rejected"
        );
    }

    #[test]
    fn escalation_event_redaction_version_field_present() {
        use chrono::Utc;
        let event = EscalationEvent {
            id: "evt-test".into(),
            escalation_ledger_id: "ledger-test".into(),
            event_kind_raw: "escalation.tier_selected".into(),
            tier_id: None,
            tier_kind_raw: None,
            trigger_raw: None,
            pause_reason_raw: None,
            payload_json: None,
            redaction_version: Some("redaction_v1".into()),
            created_at: Utc::now(),
        };
        assert_eq!(event.redaction_version.as_deref(), Some("redaction_v1"));
    }
}
