use chrono::Utc;
use domain::xcode_runtime::{
    McpBrokerObservation, McpBrokerStatusUpdate, XcodeHostExecutorEvent, XcodeRuntimeFailureClass,
    XcodeRuntimeObservation, XcodeRuntimeObservationUpdate, XcodeShimEvent,
    XcodeShimInvocationEvent, XcodeShimWarningEvent,
};

#[test]
fn runtime_observation_redacts_bearer_and_shim_tokens_before_persistence() {
    let mut observation = XcodeRuntimeObservation::default();

    observation.apply_update(XcodeRuntimeObservationUpdate::McpBrokerObservation(
        McpBrokerObservation {
            source: "xcode_mcp_broker".into(),
            backend_start_disposition: "spawned".into(),
            pool_id: Some("pool-1".into()),
            lease_id: Some("lease-1".into()),
            xcode_pid: Some("4242".into()),
            backend_process_id: Some(5252),
            http_endpoint: Some("http://127.0.0.1:4000/xcode-mcp/lease-1?token=raw-token".into()),
            xcode_home_disposition: Some("host_user_home".into()),
            xcode_tmpdir_disposition: Some("host_user_temp".into()),
            simulator_selection: None,
            sibling_leases_at_spawn: Some(1),
            backend_initialize_wait_ms: Some(7),
            backend_startup_latency_ms: Some(11),
            http_session_startup_latency_ms: Some(13),
            backend_failure_class: None,
            originating_execution_id: None,
            prompt_cycle_index: Some(0),
            status_update: Some("forwarded with Authorization=Bearer raw-bearer".into()),
        },
    ));
    observation.apply_update(XcodeRuntimeObservationUpdate::XcodeShimEvent(
        XcodeShimEvent::ShimInvocation(XcodeShimInvocationEvent {
            ts: Utc::now(),
            tool: "xcodebuild".into(),
            via_xcrun: false,
            argv: vec![
                "xcodebuild".into(),
                "-auth".into(),
                "Bearer raw-argv-bearer".into(),
                "xcode-lease-raw-shim-token".into(),
            ],
            cwd: "/tmp/workspace".into(),
            policy_decision: "allow".into(),
            policy_reason: "provider token xcode-lease-raw-policy-token accepted".into(),
            derived_peer_pid: 42,
            derived_peer_uid: 501,
            claimed_provider_pid: 42,
            peer_pid_mismatch: false,
            exit_status: 0,
        }),
    ));
    observation.apply_update(XcodeRuntimeObservationUpdate::XcodeShimEvent(
        XcodeShimEvent::Warning(XcodeShimWarningEvent {
            ts: Utc::now(),
            policy_reason: "residual_absolute_path".into(),
            source_field: "session_update".into(),
            matched_substring: "Authorization=Bearer raw-warning-bearer".into(),
            excerpt: "provider mentioned xcode-lease-raw-warning-token".into(),
        }),
    ));
    observation.apply_update(XcodeRuntimeObservationUpdate::XcodeHostExecutorEvent(
        XcodeHostExecutorEvent {
            ts: Utc::now(),
            tool: "simctl".into(),
            argv: vec!["simctl".into(), "token=raw-host-token".into()],
            cwd: "/tmp/workspace".into(),
            host_env_disposition: "allowlist_applied".into(),
            env_allowlist_applied: vec!["SCHEME".into()],
            env_dropped_from_provider: vec!["AUTH_TOKEN".into()],
            selected_simulator_id: Some("SIM-123".into()),
            exit_status: 0,
            duration_ms: 10,
        },
    ));
    observation.apply_update(XcodeRuntimeObservationUpdate::McpBrokerStatusUpdate(
        McpBrokerStatusUpdate {
            lease_id: "lease-1".into(),
            backend_failure_class: XcodeRuntimeFailureClass::XcodeMcpInitializeTimeout,
            status_update: "retry with bearer_token=raw-status-token".into(),
        },
    ));

    let serialized = serde_json::to_string(&observation).unwrap();
    assert!(!serialized.contains("raw-token"));
    assert!(!serialized.contains("raw-bearer"));
    assert!(!serialized.contains("raw-argv-bearer"));
    assert!(!serialized.contains("raw-shim-token"));
    assert!(!serialized.contains("raw-policy-token"));
    assert!(!serialized.contains("raw-warning-bearer"));
    assert!(!serialized.contains("raw-warning-token"));
    assert!(!serialized.contains("raw-host-token"));
    assert!(!serialized.contains("raw-status-token"));
    assert!(serialized.contains("Bearer <redacted>"));
    assert!(serialized.contains("xcode-lease-<redacted>"));
    assert!(serialized.contains("token=<redacted>"));
}

#[test]
fn runtime_observation_redacts_existing_payload_before_readback_surfaces() {
    let observation = XcodeRuntimeObservation {
        mcp_broker_observations: vec![McpBrokerObservation {
            source: "xcode_mcp_broker".into(),
            backend_start_disposition: "spawned".into(),
            pool_id: Some("pool-1".into()),
            lease_id: Some("lease-1".into()),
            xcode_pid: Some("4242".into()),
            backend_process_id: Some(5252),
            http_endpoint: Some(
                "http://127.0.0.1:4000/xcode-mcp/lease-1?token=raw-readback-token".into(),
            ),
            xcode_home_disposition: Some("host_user_home".into()),
            xcode_tmpdir_disposition: Some("host_user_temp".into()),
            simulator_selection: None,
            sibling_leases_at_spawn: Some(1),
            backend_initialize_wait_ms: Some(7),
            backend_startup_latency_ms: Some(11),
            http_session_startup_latency_ms: Some(13),
            backend_failure_class: None,
            originating_execution_id: None,
            prompt_cycle_index: Some(0),
            status_update: Some("forwarded Bearer raw-readback-bearer".into()),
        }],
        xcode_shim_events: vec![
            XcodeShimEvent::ShimInvocation(XcodeShimInvocationEvent {
                ts: Utc::now(),
                tool: "xcodebuild".into(),
                via_xcrun: false,
                argv: vec!["xcode-lease-raw-readback-shim-token".into()],
                cwd: "/tmp/workspace?access_token=raw-readback-cwd-token".into(),
                policy_decision: "allow".into(),
                policy_reason: "token=raw-readback-policy-token".into(),
                derived_peer_pid: 42,
                derived_peer_uid: 501,
                claimed_provider_pid: 42,
                peer_pid_mismatch: false,
                exit_status: 0,
            }),
            XcodeShimEvent::Warning(XcodeShimWarningEvent {
                ts: Utc::now(),
                policy_reason: "residual_absolute_path".into(),
                source_field: "session_update".into(),
                matched_substring: "bearer_token=raw-readback-warning-token".into(),
                excerpt: "Bearer raw-readback-warning-bearer".into(),
            }),
        ],
        xcode_host_executor_events: vec![XcodeHostExecutorEvent {
            ts: Utc::now(),
            tool: "simctl".into(),
            argv: vec!["token=raw-readback-host-token".into()],
            cwd: "/tmp/workspace?authorization=raw-readback-host-cwd-token".into(),
            host_env_disposition: "allowlist_applied".into(),
            env_allowlist_applied: vec!["SCHEME".into()],
            env_dropped_from_provider: vec!["AUTH_TOKEN".into()],
            selected_simulator_id: Some("SIM-123".into()),
            exit_status: 0,
            duration_ms: 10,
        }],
        ..XcodeRuntimeObservation::default()
    };

    let serialized = serde_json::to_string(&observation.redacted_for_surface()).unwrap();
    assert!(!serialized.contains("raw-readback-token"));
    assert!(!serialized.contains("raw-readback-bearer"));
    assert!(!serialized.contains("raw-readback-shim-token"));
    assert!(!serialized.contains("raw-readback-cwd-token"));
    assert!(!serialized.contains("raw-readback-policy-token"));
    assert!(!serialized.contains("raw-readback-warning-token"));
    assert!(!serialized.contains("raw-readback-warning-bearer"));
    assert!(!serialized.contains("raw-readback-host-token"));
    assert!(!serialized.contains("raw-readback-host-cwd-token"));
    assert!(serialized.contains("Bearer <redacted>"));
    assert!(serialized.contains("token=<redacted>"));
    assert!(serialized.contains("xcode-lease-<redacted>"));
}
