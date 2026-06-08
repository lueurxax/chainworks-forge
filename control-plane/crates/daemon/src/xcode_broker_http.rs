use std::sync::Arc;

use acp::{XcodeBrokerHttpRouteState, XcodeMcpBridgePool};
use axum::{
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};

/// Maximum request body size for the Xcode MCP broker endpoint.
/// Mirrors the MCP HTTP server limit so oversized bodies are rejected before
/// authentication and body buffering can exhaust daemon memory (SEC-XCODE-001).
const XCODE_BROKER_BODY_LIMIT_BYTES: usize = 1024 * 1024;
use serde_json::json;

pub fn routes(pool: Arc<XcodeMcpBridgePool>) -> Router {
    Router::new()
        .route("/xcode-mcp/health", get(handle_health))
        .route("/xcode-mcp/{lease_id}", post(handle_xcode_mcp_post))
        // SEC-XCODE-001: reject oversized bodies before authentication and body buffering
        // to prevent pre-auth memory pressure / DoS via unbounded body accumulation.
        .layer(DefaultBodyLimit::max(XCODE_BROKER_BODY_LIMIT_BYTES))
        .with_state(pool)
}

async fn handle_health(State(pool): State<Arc<XcodeMcpBridgePool>>) -> impl IntoResponse {
    Json(pool.health_snapshot().await)
}

async fn handle_xcode_mcp_post(
    State(pool): State<Arc<XcodeMcpBridgePool>>,
    Path(lease_id): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Response {
    let _ = pool.cleanup_first_connect_timeouts().await;
    let _ = pool.cleanup_pid_drift().await;
    let _ = pool.cleanup_idle_active_leases().await;
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    let route_state = match pool
        .authorize_and_mark_lease_active(&lease_id, authorization)
        .await
    {
        Ok(state) => state,
        Err(error) => return route_error_response(&error.to_string()),
    };

    let request_json = match serde_json::from_str::<serde_json::Value>(&body) {
        Ok(request_json) => request_json,
        Err(error) => return route_parse_error_response(&error.to_string()),
    };
    if let Err(error) = pool
        .authorize_json_rpc_request(&lease_id, &request_json)
        .await
    {
        return route_policy_denied_response(&request_json, &error.to_string());
    }
    let id = request_json
        .get("id")
        .cloned()
        .unwrap_or(serde_json::Value::Null);
    let is_notification = request_json.get("id").is_none()
        || matches!(request_json.get("id"), Some(serde_json::Value::Null));
    let activation = match route_state {
        XcodeBrokerHttpRouteState::Activated => "activated",
        XcodeBrokerHttpRouteState::AlreadyActive => "already_active",
    };

    match pool.forward_json_rpc_request(&lease_id, request_json).await {
        Ok(_) if is_notification => StatusCode::ACCEPTED.into_response(),
        Ok(response) => (StatusCode::OK, Json(response)).into_response(),
        Err(error) => route_backend_error_response(id, &lease_id, activation, &error.to_string()),
    }
}

fn route_backend_error_response(
    id: serde_json::Value,
    lease_id: &str,
    activation: &str,
    error: &str,
) -> Response {
    (
        StatusCode::OK,
        Json(json!({
            "jsonrpc": "2.0",
            "id": id,
            "error": {
                "code": -32003,
                "message": error,
                "data": {
                    "lease_id": lease_id,
                    "lease_state": activation
                }
            }
        })),
    )
        .into_response()
}

fn route_parse_error_response(error: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "jsonrpc": "2.0",
            "id": serde_json::Value::Null,
            "error": {
                "code": -32700,
                "message": format!("invalid_json_rpc_request: {error}")
            }
        })),
    )
        .into_response()
}

fn route_policy_denied_response(request_json: &serde_json::Value, error: &str) -> Response {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "jsonrpc": "2.0",
            "id": request_json.get("id").cloned().unwrap_or(serde_json::Value::Null),
            "error": {
                "code": -32004,
                "message": error
            }
        })),
    )
        .into_response()
}

fn route_error_response(error: &str) -> Response {
    let status = if error.contains("xcode_mcp_unauthorized") {
        StatusCode::UNAUTHORIZED
    } else if error.contains("xcode_mcp_broker_disabled") {
        StatusCode::SERVICE_UNAVAILABLE
    } else {
        StatusCode::NOT_FOUND
    };

    (
        status,
        Json(json!({
            "jsonrpc": "2.0",
            "id": serde_json::Value::Null,
            "error": {
                "code": -32000,
                "message": error
            }
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::*;

    use acp::{
        AcpMcpServerPayload, BrokeredXcodeMcpIntent, ExecutionRequest, HostProbeContext,
        NoopXcodeRuntimeObservationSink, ResolvedMcpServerTransport, XcodeBrokerHealthState,
        XcodeBrokerLeaseAttacher, XcodeMcpBackend, XcodeMcpBackendRequestContext,
        XcodeMcpBridgePoolConfig, XcodeMcpLeaseState, XcodeProcessCandidate,
    };
    use axum::body::{to_bytes, Body};
    use axum::http::Request;
    use domain::discovery::LegacyBroadDiscoveryPolicy;
    use domain::ids::RunId;
    use std::collections::{BTreeMap, BTreeSet};
    use std::time::Duration;
    use tower::ServiceExt;

    #[tokio::test]
    async fn xcode_mcp_route_requires_matching_lease_bearer_and_marks_active() {
        let pool = Arc::new(XcodeMcpBridgePool::new(XcodeMcpBridgePoolConfig {
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            first_connect_timeout: Duration::from_secs(60),
            ..Default::default()
        }));
        let req = brokered_xcode_request();
        let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
        let lease_id = attachment.lease_ids[0].clone();
        let auth_header = match &attachment.request.mcp_servers[0].transport {
            ResolvedMcpServerTransport::Http { headers, .. } => {
                headers.get("Authorization").cloned().unwrap()
            }
            _ => panic!("expected broker lease to become HTTP transport"),
        };

        let app = routes(pool.clone());
        let unauthorized = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":1,"method":"initialize"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(unauthorized.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(
            pool.lease_state(&lease_id).await,
            Some(XcodeMcpLeaseState::Reserved)
        );

        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    .header("authorization", auth_header)
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":7,"method":"initialize"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            pool.lease_state(&lease_id).await,
            Some(XcodeMcpLeaseState::Active)
        );
        let body = response_json(response).await;
        assert_eq!(body["id"], 7);
        assert_eq!(body["error"]["data"]["lease_state"], "activated");
    }

    #[tokio::test]
    async fn xcode_mcp_health_reports_disabled_state() {
        let pool = Arc::new(XcodeMcpBridgePool::new(XcodeMcpBridgePoolConfig {
            broker_disabled: true,
            ..Default::default()
        }));
        let response = routes(pool)
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri("/xcode-mcp/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["state"], "disabled");
        assert_eq!(body["reason_code"], "xcode_mcp_broker_disabled");
        assert_eq!(body["can_acquire_new_xcode_leases"], false);
        assert_eq!(body["active_lease_count"], 0);
        assert_eq!(body["initialize_queue_depth"], 0);
        assert_eq!(
            body["last_transition_at"].as_str().unwrap().is_empty(),
            false
        );
        assert_eq!(body["operator_message"], "Xcode broker disabled");
        assert_eq!(
            serde_json::from_value::<XcodeBrokerHealthState>(body["state"].clone()).unwrap(),
            XcodeBrokerHealthState::Disabled
        );
    }

    #[tokio::test]
    async fn xcode_mcp_route_closes_pid_drift_before_forwarding() {
        let host = HostProbeContext {
            expected_gui_uid: Some(501),
            operator_home: Some("/Users/gui".to_string()),
            darwin_tmpdir: Some("/var/folders/t/tmp".to_string()),
            developer_dir: Some("/Applications/Xcode.app/Contents/Developer".to_string()),
            candidate_xcodes: vec![XcodeProcessCandidate {
                pid: 4242,
                uid: 501,
                workspace_identity: Some("/tmp/workspace".to_string()),
                app_path: Some("/Applications/Xcode.app".to_string()),
                developer_dir: None,
                operator_home: None,
                darwin_tmpdir: None,
                alive: true,
            }],
        };
        let pool = Arc::new(XcodeMcpBridgePool::new(XcodeMcpBridgePoolConfig {
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            first_connect_timeout: Duration::from_secs(60),
            target_probe_context: Some(host.clone()),
            ..Default::default()
        }));
        let req = brokered_xcode_request();
        let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
        let lease_id = attachment.lease_ids[0].clone();
        let auth_header = match &attachment.request.mcp_servers[0].transport {
            ResolvedMcpServerTransport::Http { headers, .. } => {
                headers.get("Authorization").cloned().unwrap()
            }
            _ => panic!("expected broker lease to become HTTP transport"),
        };
        pool.mark_lease_active(&lease_id).await.unwrap();

        pool.replace_target_probe_context(Some(HostProbeContext {
            candidate_xcodes: vec![XcodeProcessCandidate {
                pid: 5252,
                workspace_identity: Some("/tmp/workspace".to_string()),
                ..host.candidate_xcodes[0].clone()
            }],
            ..host
        }))
        .await;

        let response = routes(pool.clone())
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    .header("authorization", auth_header)
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::NOT_FOUND);
        assert_eq!(pool.active_lease_count().await, 0);
        assert_eq!(pool.lease_state(&lease_id).await, None);
        let body = response_json(response).await;
        assert!(body["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("xcode_mcp_first_connect_timeout")));
    }

    #[tokio::test]
    async fn xcode_mcp_route_denies_disallowed_tool_call_before_backend_forward() {
        let pool = Arc::new(XcodeMcpBridgePool::new(XcodeMcpBridgePoolConfig {
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            first_connect_timeout: Duration::from_secs(60),
            tool_allowlists_by_hash: BTreeMap::from([(
                "build-only".to_string(),
                BTreeSet::from(["xcode.build".to_string()]),
            )]),
            ..Default::default()
        }));
        let mut req = brokered_xcode_request();
        let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } =
            &mut req.mcp_servers[0].transport
        else {
            panic!("expected broker intent");
        };
        intent.resolved_tool_allowlist_hash = Some("build-only".to_string());

        let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
        let lease_id = attachment.lease_ids[0].clone();
        let auth_header = match &attachment.request.mcp_servers[0].transport {
            ResolvedMcpServerTransport::Http { headers, .. } => {
                headers.get("Authorization").cloned().unwrap()
            }
            _ => panic!("expected broker lease to become HTTP transport"),
        };

        let response = routes(pool)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    .header("authorization", auth_header)
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"xcode.test"}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(body["id"], 9);
        assert!(body["error"]["message"]
            .as_str()
            .is_some_and(|message| message.contains("xcode_mcp_tool_denied")));
    }

    #[tokio::test]
    async fn xcode_mcp_route_forwards_to_backend_and_filters_tools_list() {
        struct FixtureBackend;

        #[async_trait::async_trait]
        impl XcodeMcpBackend for FixtureBackend {
            async fn forward_json_rpc(
                &self,
                context: XcodeMcpBackendRequestContext,
                request: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                assert!(context.lease_id.starts_with("lease-"));
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": request.get("id").cloned().unwrap_or(serde_json::Value::Null),
                    "result": {
                        "tools": [
                            {"name": "xcode.build"},
                            {"name": "xcode.test"},
                            {"name": "xcode.clean"}
                        ]
                    }
                }))
            }
        }

        let pool = Arc::new(XcodeMcpBridgePool::new_with_sink_and_backend(
            XcodeMcpBridgePoolConfig {
                base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
                first_connect_timeout: Duration::from_secs(60),
                tool_allowlists_by_hash: BTreeMap::from([(
                    "build-only".to_string(),
                    BTreeSet::from(["xcode.build".to_string()]),
                )]),
                ..Default::default()
            },
            Arc::new(NoopXcodeRuntimeObservationSink),
            Arc::new(FixtureBackend),
        ));
        let mut req = brokered_xcode_request();
        let ResolvedMcpServerTransport::XcodeBrokerIntent { intent } =
            &mut req.mcp_servers[0].transport
        else {
            panic!("expected broker intent");
        };
        intent.resolved_tool_allowlist_hash = Some("build-only".to_string());

        let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
        let lease_id = attachment.lease_ids[0].clone();
        let auth_header = match &attachment.request.mcp_servers[0].transport {
            ResolvedMcpServerTransport::Http { headers, .. } => {
                headers.get("Authorization").cloned().unwrap()
            }
            _ => panic!("expected broker lease to become HTTP transport"),
        };

        let response = routes(pool)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    .header("authorization", auth_header)
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","id":11,"method":"tools/list"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["id"], 11);
        assert_eq!(body["result"]["tools"].as_array().unwrap().len(), 1);
        assert_eq!(body["result"]["tools"][0]["name"], "xcode.build");
    }

    #[tokio::test]
    async fn xcode_mcp_route_returns_accepted_without_body_for_notifications() {
        struct RecordingBackend {
            methods: tokio::sync::Mutex<Vec<String>>,
        }

        #[async_trait::async_trait]
        impl XcodeMcpBackend for RecordingBackend {
            async fn forward_json_rpc(
                &self,
                _context: XcodeMcpBackendRequestContext,
                request: serde_json::Value,
            ) -> anyhow::Result<serde_json::Value> {
                self.methods.lock().await.push(
                    request
                        .get("method")
                        .and_then(|method| method.as_str())
                        .unwrap_or("<missing>")
                        .to_string(),
                );
                Ok(json!({
                    "jsonrpc": "2.0",
                    "id": serde_json::Value::Null,
                    "result": serde_json::Value::Null
                }))
            }
        }

        let backend = Arc::new(RecordingBackend {
            methods: tokio::sync::Mutex::new(Vec::new()),
        });
        let pool = Arc::new(XcodeMcpBridgePool::new_with_sink_and_backend(
            XcodeMcpBridgePoolConfig {
                base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
                first_connect_timeout: Duration::from_secs(60),
                ..Default::default()
            },
            Arc::new(NoopXcodeRuntimeObservationSink),
            backend.clone(),
        ));
        let req = brokered_xcode_request();
        let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
        let lease_id = attachment.lease_ids[0].clone();
        let auth_header = match &attachment.request.mcp_servers[0].transport {
            ResolvedMcpServerTransport::Http { headers, .. } => {
                headers.get("Authorization").cloned().unwrap()
            }
            _ => panic!("expected broker lease to become HTTP transport"),
        };

        let response = routes(pool)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    .header("authorization", auth_header)
                    .body(Body::from(
                        r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::ACCEPTED);
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        assert!(bytes.is_empty());
        assert_eq!(
            backend.methods.lock().await.as_slice(),
            ["notifications/initialized"]
        );
    }

    // SEC-XCODE-001: oversized unauthenticated body must be rejected with 413 before authentication.
    #[tokio::test]
    async fn xcode_mcp_route_rejects_oversized_unauthenticated_body() {
        let pool = Arc::new(XcodeMcpBridgePool::new(XcodeMcpBridgePoolConfig {
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            first_connect_timeout: Duration::from_secs(60),
            ..Default::default()
        }));
        let req = brokered_xcode_request();
        let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
        let lease_id = attachment.lease_ids[0].clone();

        let response = routes(pool)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    // No authorization header — unauthenticated
                    .body(Body::from("x".repeat(XCODE_BROKER_BODY_LIMIT_BYTES + 1)))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    // SEC-XCODE-001: oversized authenticated body must also be rejected with 413.
    #[tokio::test]
    async fn xcode_mcp_route_rejects_oversized_authenticated_body() {
        let pool = Arc::new(XcodeMcpBridgePool::new(XcodeMcpBridgePoolConfig {
            base_url: "http://127.0.0.1:8123/xcode-mcp".to_string(),
            first_connect_timeout: Duration::from_secs(60),
            ..Default::default()
        }));
        let req = brokered_xcode_request();
        let attachment = pool.attach_brokered_xcode_leases(&req).await.unwrap();
        let lease_id = attachment.lease_ids[0].clone();
        let auth_header = match &attachment.request.mcp_servers[0].transport {
            ResolvedMcpServerTransport::Http { headers, .. } => {
                headers.get("Authorization").cloned().unwrap()
            }
            _ => panic!("expected broker lease to become HTTP transport"),
        };

        let response = routes(pool)
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease_id}"))
                    .header("authorization", auth_header)
                    .body(Body::from("x".repeat(XCODE_BROKER_BODY_LIMIT_BYTES + 1)))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    async fn response_json(response: Response) -> serde_json::Value {
        let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
        serde_json::from_slice(&bytes).unwrap()
    }

    fn brokered_xcode_request() -> ExecutionRequest {
        ExecutionRequest {
            agent_execution_id: None,
            run_id: RunId::new(),
            stage_execution_id: None,
            stage_id: "stage_xcode".to_string(),
            attempt_number: 1,
            agent_id: "xcode-agent".to_string(),
            provider: "claude".to_string(),
            model: None,
            effort: None,
            workspace_root: "/tmp/workspace".to_string(),
            prompt: "use xcode".to_string(),
            worktree_root: None,
            worktree_write_enabled: false,
            worktree_strategy: None,
            expected_output_paths: Vec::new(),
            expected_outputs: Vec::new(),
            keep_session_alive: false,
            reuse_existing_session: false,
            session_generation_id: None,
            provider_session_id: None,
            mcp_servers: vec![AcpMcpServerPayload {
                id: "xcode-broker".to_string(),
                extension_id: "xcode".to_string(),
                transport: ResolvedMcpServerTransport::XcodeBrokerIntent {
                    intent: BrokeredXcodeMcpIntent {
                        extension_id: "xcode".to_string(),
                        runtime_id: "xcode-broker".to_string(),
                        server_id: "xcode".to_string(),
                        workspace_root: Some("/tmp/workspace".to_string()),
                        xcode_pid_selector: None,
                        runtime_profile_id: Some("profile-xcode".to_string()),
                        permission_profile_id: Some("perm-xcode".to_string()),
                        resolved_tool_allowlist_hash: None,
                        provider_http_required: true,
                    },
                },
            }],
            chainworks_meta_root: None,
            legacy_broad_discovery_policy: LegacyBroadDiscoveryPolicy::Disabled,
            xcode_shim_injection_signal: false,
            requires_xcode_host_execution: false,
            owner_kind: "stage_execution".to_string(),
            owner_id: None,
            origin_stage_id: None,
            origin_stage_execution_id: None,
            mediation_record_id: None,
            toolchain_home: None,
            toolchain_go_scope_enabled: false,

            p079_repair_canonical_paths: None,
        }
    }
}
