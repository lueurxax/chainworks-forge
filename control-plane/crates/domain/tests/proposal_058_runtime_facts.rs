use std::str::FromStr;

use chrono::Utc;
use domain::agent::{
    AgentExecutionRuntimeFacts, AgentFailureKind, AgentOutputSettlement, ArtifactSourceClaimState,
    OperatorActionHint,
};
use domain::ids::AgentExecutionId;

#[test]
fn proposal_058_failure_and_settlement_enums_round_trip_snake_case() {
    let cases = [
        (AgentFailureKind::ProviderQuota, "provider_quota"),
        (
            AgentFailureKind::ProviderPermissionRequired,
            "provider_permission_required",
        ),
        (
            AgentFailureKind::ProviderPermissionRejected,
            "provider_permission_rejected",
        ),
        (AgentFailureKind::ProviderTimeout, "provider_timeout"),
        (
            AgentFailureKind::ProviderInternalError,
            "provider_internal_error",
        ),
        (AgentFailureKind::TransportEpipe, "transport_epipe"),
        (
            AgentFailureKind::TransportProtocolError,
            "transport_protocol_error",
        ),
        (AgentFailureKind::TransportClosed, "transport_closed"),
        (AgentFailureKind::McpStartupTimeout, "mcp_startup_timeout"),
        (
            AgentFailureKind::McpPermissionModalStall,
            "mcp_permission_modal_stall",
        ),
        (
            AgentFailureKind::XcodeHostEnvironmentError,
            "xcode_host_environment_error",
        ),
        (
            AgentFailureKind::MissingRequiredOutputs,
            "missing_required_outputs",
        ),
        (
            AgentFailureKind::InvalidOutputContract,
            "invalid_output_contract",
        ),
        (
            AgentFailureKind::CancelledByOperator,
            "cancelled_by_operator",
        ),
        (AgentFailureKind::SupersededByRetry, "superseded_by_retry"),
        (AgentFailureKind::Unknown, "unknown"),
    ];
    for (kind, raw) in cases {
        assert_eq!(kind.to_string(), raw);
        assert_eq!(AgentFailureKind::from_str(raw).unwrap(), kind);
    }

    assert!(AgentFailureKind::from_str("ignored_late_outputs").is_err());
    assert_eq!(
        AgentOutputSettlement::from_str("ignored_late_outputs").unwrap(),
        AgentOutputSettlement::IgnoredLateOutputs
    );
    assert!(AgentOutputSettlement::from_str("superseded_outputs").is_err());
}

#[test]
fn proposal_058_runtime_fact_defaults_are_legacy_safe() {
    let now = Utc::now();
    let facts = AgentExecutionRuntimeFacts::defaults_for(AgentExecutionId::new(), now);
    assert_eq!(facts.failure_kind, None);
    assert_eq!(facts.failure_kind_raw_debug, None);
    assert_eq!(facts.failure_kind_version, 1);
    assert_eq!(facts.failure_message_redaction_version, 1);
    assert_eq!(facts.output_settlement, AgentOutputSettlement::None);
    assert!(!facts.valid_required_outputs);
    assert_eq!(facts.late_output_count, 0);
    assert_eq!(facts.ignored_late_output_count, 0);
}

#[test]
fn proposal_058_action_hints_and_claim_states_are_executable_contracts() {
    assert_eq!(
        OperatorActionHint::WaitUntilRetryAfter.to_string(),
        "wait_until_retry_after"
    );
    assert_eq!(
        OperatorActionHint::from_str("authorize_xcode").unwrap(),
        OperatorActionHint::AuthorizeXcode
    );
    assert_eq!(
        ArtifactSourceClaimState::SupersededPendingRetry.to_string(),
        "superseded_pending_retry"
    );
    assert_eq!(
        ArtifactSourceClaimState::from_str("closed").unwrap(),
        ArtifactSourceClaimState::Closed
    );
}
