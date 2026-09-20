use std::sync::Arc;

use acp::{
    xcode_headless_runtime::HeadlessRuntimeError, XcodeBrokerHealthState, XcodeMcpBridgePool,
};
use axum::{
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use domain::xcode_contract::{parse_unique_json, ContractError, MAX_SAFE_INTEGER};
use serde_json::{json, Map, Value};

/// Maximum request body size for the Xcode MCP broker endpoint.
/// Mirrors the MCP HTTP server limit so oversized bodies are rejected before
/// authentication and body buffering can exhaust daemon memory (SEC-XCODE-001).
const XCODE_BROKER_BODY_LIMIT_BYTES: usize = 1024 * 1024;
const MAX_RPC_STRING_BYTES: usize = 256;

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
    let health = pool.health_snapshot().await;
    let (reason, message) = match health.state {
        XcodeBrokerHealthState::Disabled => ("xcode_mcp_broker_disabled", "Xcode broker disabled"),
        XcodeBrokerHealthState::Healthy => ("healthy", "Xcode broker healthy"),
        XcodeBrokerHealthState::Failed => (
            "xcode_mcp_backend_unavailable",
            "Xcode broker failed: backend unavailable",
        ),
        XcodeBrokerHealthState::Degraded => match health.reason_code.as_str() {
            "xcode_mcp_stale_leases" => (
                "xcode_mcp_stale_leases",
                "Xcode broker degraded: stale leases",
            ),
            "xcode_observation_persist_failed" => (
                "xcode_observation_persist_failed",
                "Xcode broker degraded: observation persistence failures",
            ),
            _ => (
                "xcode_mcp_capacity_backpressure",
                "Xcode broker degraded: capacity backpressure",
            ),
        },
    };
    let bounded = |count: usize| count.min(u32::MAX as usize);
    let timestamp = chrono::DateTime::parse_from_rfc3339(&health.last_transition_at)
        .map(|time| time.with_timezone(&chrono::Utc).to_rfc3339())
        .unwrap_or_default();
    // Explicit public projection: future privileged snapshot fields cannot leak here.
    Json(json!({
        "state": health.state,
        "reason_code": reason,
        "can_acquire_new_xcode_leases": health.can_acquire_new_xcode_leases,
        "active_lease_count": bounded(health.active_lease_count),
        "initialize_queue_depth": bounded(health.initialize_queue_depth),
        "last_transition_at": timestamp,
        "operator_message": message,
        "active_leases": bounded(health.active_leases),
        "queued_leases": bounded(health.queued_leases),
        "max_active_leases": bounded(health.max_active_leases),
        "max_queued_leases": bounded(health.max_queued_leases),
        "broker_disabled": health.broker_disabled,
        "backend_available": health.backend_available,
        "observation_persistence_failures": health.observation_persistence_failures.min(u32::MAX as u64),
        "stale_lease_count": bounded(health.stale_lease_count),
        "backend_session_count": bounded(health.backend_session_count),
        "helper_cleanup_reaped_leases_total": health.helper_cleanup_reaped_leases_total.min(u32::MAX as u64),
        "headless_runtime_schema_versions": [1]
    }))
}

async fn handle_xcode_mcp_post(
    State(pool): State<Arc<XcodeMcpBridgePool>>,
    Path(lease_id): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let _ = pool.cleanup_first_connect_timeouts().await;
    let _ = pool.cleanup_pid_drift().await;
    let _ = pool.cleanup_idle_active_leases().await;
    let authorization = headers
        .get("authorization")
        .and_then(|value| value.to_str().ok());
    if let Err(error) = pool
        .authorize_and_mark_lease_active(&lease_id, authorization)
        .await
    {
        let error = if error.to_string().split(':').next() == Some("xcode_mcp_broker_disabled") {
            WireError::Disabled
        } else {
            WireError::Unauthorized
        };
        return error.response(Value::Null);
    }

    let request_json = match parse_unique_json(&body) {
        Ok(request_json) => request_json,
        Err(ContractError::InputTooLarge) => return StatusCode::PAYLOAD_TOO_LARGE.into_response(),
        Err(ContractError::DuplicateKey | ContractError::UnsafeNumber) => {
            return WireError::InvalidRequest.response(Value::Null);
        }
        Err(_) => return WireError::Parse.response(Value::Null),
    };
    let id = request_json
        .get("id")
        .filter(|id| valid_rpc_id(id))
        .cloned()
        .unwrap_or(Value::Null);
    let is_notification = match validate_rpc_request(&request_json) {
        Ok(is_notification) => is_notification,
        Err(error) => return error.response(id),
    };
    if pool
        .authorize_json_rpc_request(&lease_id, &request_json)
        .await
        .is_err()
    {
        return WireError::Denied.response(id);
    }

    match pool.forward_json_rpc_request(&lease_id, request_json).await {
        Ok(_) if is_notification => StatusCode::ACCEPTED.into_response(),
        Ok(response) => {
            // A backend request/callback is not a response to a provider request.
            let Some(envelope) = closed_object(
                &response,
                &["jsonrpc", "id", "result", "error"],
                &["jsonrpc", "id"],
            ) else {
                return WireError::Contract.response(id);
            };
            if envelope.get("jsonrpc").and_then(Value::as_str) != Some("2.0")
                || envelope.get("id") != Some(&id)
                || envelope.contains_key("result") == envelope.contains_key("error")
            {
                return WireError::Contract.response(id);
            }
            if envelope.contains_key("error") {
                return WireError::Backend.response(id);
            }
            (StatusCode::OK, Json(response)).into_response()
        }
        Err(error) => {
            let error = if let Some(error) = error.downcast_ref::<HeadlessRuntimeError>() {
                match error {
                    HeadlessRuntimeError::PermissionDenied => WireError::PermissionDenied,
                    HeadlessRuntimeError::InactiveInvocation => WireError::InactiveInvocation,
                    HeadlessRuntimeError::ProjectHeld => WireError::ProjectHeld,
                    HeadlessRuntimeError::OutcomeUnknown => WireError::OutcomeUnknown,
                    HeadlessRuntimeError::AlreadyRecorded => WireError::AlreadyRecorded,
                }
            } else {
                match error.downcast_ref::<ContractError>() {
                    Some(ContractError::InvalidArguments) => WireError::InvalidParams,
                    Some(_) => WireError::Contract,
                    None => WireError::Backend,
                }
            };
            error.response(id)
        }
    }
}

fn bounded_string(value: &Value, max: usize) -> bool {
    value
        .as_str()
        .is_some_and(|value| !value.is_empty() && value.len() <= max)
}

fn valid_rpc_id(value: &Value) -> bool {
    bounded_string(value, MAX_RPC_STRING_BYTES)
        || value
            .as_i64()
            .is_some_and(|id| id.unsigned_abs() <= MAX_SAFE_INTEGER)
}

fn closed_object<'a>(
    value: &'a Value,
    allowed: &[&str],
    required: &[&str],
) -> Option<&'a Map<String, Value>> {
    let object = value.as_object()?;
    (object.keys().all(|key| allowed.contains(&key.as_str()))
        && required.iter().all(|key| object.contains_key(*key)))
    .then_some(object)
}

fn valid_request_meta(params: &Map<String, Value>) -> bool {
    params.get("_meta").is_none_or(|meta| {
        closed_object(meta, &["progressToken"], &[])
            .is_some_and(|meta| meta.get("progressToken").is_none_or(valid_rpc_id))
    })
}

fn validate_rpc_request(request: &Value) -> Result<bool, WireError> {
    let envelope = closed_object(
        request,
        &["jsonrpc", "id", "method", "params"],
        &["jsonrpc", "method"],
    )
    .ok_or(WireError::InvalidRequest)?;
    if envelope["jsonrpc"] != "2.0" || !bounded_string(&envelope["method"], 128) {
        return Err(WireError::InvalidRequest);
    }
    let method = envelope["method"]
        .as_str()
        .ok_or(WireError::InvalidRequest)?;
    let notification = matches!(
        method,
        "notifications/initialized" | "notifications/cancelled" | "notifications/progress"
    );
    if (notification && envelope.contains_key("id"))
        || (!notification && !envelope.get("id").is_some_and(valid_rpc_id))
    {
        return Err(WireError::InvalidRequest);
    }
    let params = envelope.get("params");
    let valid = match method {
        "ping" | "notifications/initialized" => {
            params.is_none_or(|p| closed_object(p, &[], &[]).is_some())
        }
        "tools/list" => params.is_none_or(|p| {
            closed_object(p, &["cursor", "_meta"], &[]).is_some_and(|p| {
                p.get("cursor")
                    .is_none_or(|v| bounded_string(v, MAX_RPC_STRING_BYTES))
                    && valid_request_meta(p)
            })
        }),
        "tools/call" => params.is_some_and(|p| {
            closed_object(p, &["name", "arguments", "_meta"], &["name", "arguments"]).is_some_and(
                |p| {
                    bounded_string(&p["name"], MAX_RPC_STRING_BYTES)
                        && p["arguments"].is_object()
                        && valid_request_meta(p)
                },
            )
        }),
        "initialize" => params.is_some_and(valid_initialize_params),
        "notifications/cancelled" => params.is_some_and(|p| {
            closed_object(p, &["requestId", "reason"], &["requestId"]).is_some_and(|p| {
                valid_rpc_id(&p["requestId"])
                    && p.get("reason").is_none_or(|v| bounded_string(v, 512))
            })
        }),
        "notifications/progress" => params.is_some_and(|p| {
            closed_object(
                p,
                &["progressToken", "progress", "total", "message"],
                &["progressToken", "progress"],
            )
            .is_some_and(|p| {
                let nonnegative = |v: &Value| v.as_f64().is_some_and(|v| v.is_finite() && v >= 0.0);
                valid_rpc_id(&p["progressToken"])
                    && nonnegative(&p["progress"])
                    && p.get("total").is_none_or(nonnegative)
                    && p.get("message").is_none_or(|v| bounded_string(v, 512))
            })
        }),
        _ => return Err(WireError::MethodNotFound),
    };
    if !valid {
        return Err(if notification {
            WireError::InvalidRequest
        } else {
            WireError::InvalidParams
        });
    }
    Ok(notification)
}

fn valid_initialize_params(params: &Value) -> bool {
    let Some(params) = closed_object(
        params,
        &["protocolVersion", "capabilities", "clientInfo"],
        &["protocolVersion", "capabilities", "clientInfo"],
    ) else {
        return false;
    };
    bounded_string(&params["protocolVersion"], 64)
        && closed_object(
            &params["clientInfo"],
            &["name", "version", "title"],
            &["name", "version"],
        )
        .is_some_and(|info| {
            info.values()
                .all(|v| bounded_string(v, MAX_RPC_STRING_BYTES))
        })
        && closed_object(
            &params["capabilities"],
            &["roots", "sampling", "elicitation"],
            &[],
        )
        .is_some_and(|capabilities| {
            capabilities.iter().all(|(name, value)| {
                if name == "roots" {
                    closed_object(value, &["listChanged"], &[])
                        .is_some_and(|roots| roots.get("listChanged").is_none_or(Value::is_boolean))
                } else {
                    closed_object(value, &[], &[]).is_some()
                }
            })
        })
}

#[derive(Clone, Copy)]
enum WireError {
    Parse,
    InvalidRequest,
    MethodNotFound,
    InvalidParams,
    Unauthorized,
    Disabled,
    Denied,
    Backend,
    Contract,
    PermissionDenied,
    InactiveInvocation,
    ProjectHeld,
    OutcomeUnknown,
    AlreadyRecorded,
}

impl WireError {
    fn response(self, id: Value) -> Response {
        let (status, code, reason, message, retry, binding_state) = match self {
            Self::Parse => (
                StatusCode::BAD_REQUEST,
                -32700,
                "request_invalid",
                "Invalid JSON",
                "never",
                "unknown",
            ),
            Self::InvalidRequest => (
                StatusCode::BAD_REQUEST,
                -32600,
                "request_invalid",
                "Invalid JSON-RPC request",
                "never",
                "unknown",
            ),
            Self::MethodNotFound => (
                StatusCode::OK,
                -32601,
                "request_invalid",
                "Method not found",
                "never",
                "unknown",
            ),
            Self::InvalidParams => (
                StatusCode::OK,
                -32602,
                "request_invalid",
                "Invalid parameters",
                "never",
                "unknown",
            ),
            Self::Unauthorized => (
                StatusCode::UNAUTHORIZED,
                -32000,
                "unauthorized",
                "Unauthorized",
                "never",
                "unknown",
            ),
            Self::Disabled => (
                StatusCode::SERVICE_UNAVAILABLE,
                -32000,
                "broker_disabled",
                "Xcode broker disabled",
                "after_operator_action",
                "unknown",
            ),
            Self::Denied => (
                StatusCode::FORBIDDEN,
                -32004,
                "scope_denied",
                "Request denied",
                "never",
                "unknown",
            ),
            Self::Backend => (
                StatusCode::OK,
                -32003,
                "service_unavailable",
                "Xcode service unavailable",
                "never",
                "unknown",
            ),
            Self::Contract => (
                StatusCode::OK,
                -32003,
                "contract_incompatible",
                "Xcode contract incompatible",
                "never",
                "unknown",
            ),
            Self::PermissionDenied => (
                StatusCode::FORBIDDEN,
                -32004,
                "permission_denied",
                "Xcode permission denied",
                "never",
                "action_required",
            ),
            Self::InactiveInvocation => (
                StatusCode::OK,
                -32003,
                "workspace_stale",
                "Xcode invocation inactive",
                "never",
                "revoked",
            ),
            Self::ProjectHeld => (
                StatusCode::OK,
                -32003,
                "outcome_unknown",
                "Xcode project held",
                "never",
                "action_required",
            ),
            Self::OutcomeUnknown => (
                StatusCode::OK,
                -32003,
                "outcome_unknown",
                "Xcode effect outcome unknown",
                "never",
                "action_required",
            ),
            Self::AlreadyRecorded => (
                StatusCode::OK,
                -32003,
                "operation_conflict",
                "Xcode effect already recorded",
                "never",
                "unknown",
            ),
        };
        (
            status,
            Json(json!({
                "jsonrpc": "2.0", "id": id,
                "error": {"code": code, "message": message, "data": {
                    "schema_version": 1, "code": reason, "retry": retry,
                    "attempt_id": null, "binding_state": binding_state
                }}
            })),
        )
            .into_response()
    }
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

    struct WireBackend {
        calls: std::sync::atomic::AtomicUsize,
        response: Option<serde_json::Value>,
        fail: bool,
        runtime_error: Option<fn() -> HeadlessRuntimeError>,
    }

    #[async_trait::async_trait]
    impl XcodeMcpBackend for WireBackend {
        async fn forward_json_rpc(
            &self,
            _context: XcodeMcpBackendRequestContext,
            request: serde_json::Value,
        ) -> anyhow::Result<serde_json::Value> {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if let Some(error) = self.runtime_error {
                return Err(anyhow::Error::new(error())
                    .context("backend /private/runtime secret-token lease-private"));
            }
            if self.fail {
                anyhow::bail!("backend /private/runtime secret-token lease-private");
            }
            Ok(self.response.clone().unwrap_or_else(|| {
                json!({
                    "jsonrpc": "2.0", "id": request.get("id").cloned().unwrap_or_default(),
                    "result": {}
                })
            }))
        }
    }

    async fn wire_fixture(
        response: Option<serde_json::Value>,
        fail: bool,
    ) -> (Router, String, String, Arc<WireBackend>) {
        let backend = Arc::new(WireBackend {
            calls: std::sync::atomic::AtomicUsize::new(0),
            response,
            fail,
            runtime_error: None,
        });
        wire_backend_fixture(backend).await
    }

    async fn wire_backend_fixture(
        backend: Arc<WireBackend>,
    ) -> (Router, String, String, Arc<WireBackend>) {
        let pool = Arc::new(XcodeMcpBridgePool::new_with_sink_and_backend(
            XcodeMcpBridgePoolConfig {
                first_connect_timeout: Duration::from_secs(60),
                ..Default::default()
            },
            Arc::new(NoopXcodeRuntimeObservationSink),
            backend.clone(),
        ));
        let attachment = pool
            .attach_brokered_xcode_leases(&brokered_xcode_request())
            .await
            .unwrap();
        let ResolvedMcpServerTransport::Http { headers, .. } =
            &attachment.request.mcp_servers[0].transport
        else {
            panic!("HTTP lease expected")
        };
        (
            routes(pool),
            attachment.lease_ids[0].clone(),
            headers["Authorization"].clone(),
            backend,
        )
    }

    async fn wire_post(app: &Router, lease: &str, bearer: &str, body: impl Into<Body>) -> Response {
        app.clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/xcode-mcp/{lease}"))
                    .header("authorization", bearer)
                    .body(body.into())
                    .unwrap(),
            )
            .await
            .unwrap()
    }

    fn assert_wire_error(body: &serde_json::Value, id: serde_json::Value, code: i64, reason: &str) {
        assert_eq!(body["id"], id);
        assert_eq!(body["error"]["code"], code);
        assert_eq!(body["error"]["data"]["code"], reason);
        let data = body["error"]["data"].as_object().expect("typed error data");
        assert_eq!(data.len(), 5);
        assert_eq!(data["schema_version"], 1);
        assert!(data["attempt_id"].is_null());
        assert!(data["retry"].is_string());
        assert!(data["binding_state"].is_string());
        let message = body["error"]["message"].as_str().unwrap();
        assert!(!message.is_empty() && message.len() <= 512);
        for private in ["/private/runtime", "secret-token", "lease-private"] {
            assert!(!body.to_string().contains(private));
        }
    }

    #[tokio::test]
    async fn wire_rejects_duplicate_keys_before_value_and_dispatch() {
        let (app, lease, auth, backend) = wire_fixture(None, false).await;
        for body in [
            r#"{"jsonrpc":"2.0","id":1,"id":2,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"tools/call","params":{"name":"read","arguments":{"path":"a","path":"b"}}}"#,
            r#"{"jsonrpc":"2.0","id":1,"method":"ping","method":"tools/call"}"#,
        ] {
            let response = wire_post(&app, &lease, &auth, body).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_wire_error(
                &response_json(response).await,
                json!(null),
                -32600,
                "request_invalid",
            );
        }
        assert_eq!(backend.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn wire_rejects_invalid_envelopes_and_recovers_only_valid_ids() {
        let (app, lease, auth, backend) = wire_fixture(None, false).await;
        for (body, id) in [
            (json!([]), json!(null)),
            (json!(null), json!(null)),
            (
                json!({"jsonrpc":"2.0","id":null,"method":"ping"}),
                json!(null),
            ),
            (
                json!({"jsonrpc":"2.0","id":1.5,"method":"ping"}),
                json!(null),
            ),
            (
                json!({"jsonrpc":"2.0","id":true,"method":"ping"}),
                json!(null),
            ),
            (
                json!({"jsonrpc":"2.0","id":"","method":"ping"}),
                json!(null),
            ),
            (
                json!({"jsonrpc":"2.0","id":"a".repeat(257),"method":"ping"}),
                json!(null),
            ),
            (
                json!({"jsonrpc":"1.0","id":"kept","method":"ping"}),
                json!("kept"),
            ),
            (
                json!({"jsonrpc":"2.0","id":2,"method":"ping","extra":true}),
                json!(2),
            ),
            (
                json!({"jsonrpc":"2.0","id":3,"method":"ping","result":{}}),
                json!(3),
            ),
            (
                json!({"jsonrpc":"2.0","method":"tools/call","params":{"name":"read","arguments":{}}}),
                json!(null),
            ),
            (json!({"jsonrpc":"2.0","method":"ping"}), json!(null)),
            (
                json!({"jsonrpc":"2.0","id":4,"method":"notifications/initialized"}),
                json!(4),
            ),
            (
                json!({"jsonrpc":"2.0","id":5,"method":"notifications/cancelled","params":{"requestId":1}}),
                json!(5),
            ),
            (
                json!({"jsonrpc":"2.0","id":6,"method":"notifications/progress","params":{"progressToken":1,"progress":0}}),
                json!(6),
            ),
        ] {
            let response = wire_post(&app, &lease, &auth, body.to_string()).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST, "{body}");
            assert_wire_error(
                &response_json(response).await,
                id,
                -32600,
                "request_invalid",
            );
        }
        for body in [
            r#"{"jsonrpc":"2.0","id":9007199254740992,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":-9007199254740992,"method":"ping"}"#,
        ] {
            let response = wire_post(&app, &lease, &auth, body).await;
            assert_eq!(response.status(), StatusCode::BAD_REQUEST);
            assert_wire_error(
                &response_json(response).await,
                json!(null),
                -32600,
                "request_invalid",
            );
        }
        assert_eq!(backend.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn wire_rejects_unknown_methods_and_provider_callbacks_without_dispatch() {
        let (app, lease, auth, backend) = wire_fixture(None, false).await;
        for method in [
            "arbitrary",
            "roots/list",
            "sampling/createMessage",
            "elicitation/create",
            "resources/read",
        ] {
            let response = wire_post(
                &app,
                &lease,
                &auth,
                json!({"jsonrpc":"2.0","id":"request","method":method}).to_string(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_wire_error(
                &response_json(response).await,
                json!("request"),
                -32601,
                "request_invalid",
            );
        }
        assert_eq!(backend.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn wire_rejects_malformed_params_without_dispatch() {
        let (app, lease, auth, backend) = wire_fixture(None, false).await;
        for (method, params) in [
            ("tools/call", json!(null)),
            ("tools/call", json!([])),
            ("tools/call", json!({"toolName":"read","arguments":{}})),
            ("tools/call", json!({"name":"","arguments":{}})),
            ("tools/call", json!({"name":"read"})),
            ("tools/call", json!({"name":"read","arguments":[]})),
            (
                "tools/call",
                json!({"name":"read","arguments":{},"extra":true}),
            ),
            ("tools/list", json!({"cursor":null})),
            ("ping", json!({"extra":true})),
            ("initialize", json!({"protocolVersion":true})),
        ] {
            let response = wire_post(
                &app,
                &lease,
                &auth,
                json!({"jsonrpc":"2.0","id":8,"method":method,"params":params}).to_string(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_wire_error(
                &response_json(response).await,
                json!(8),
                -32602,
                "request_invalid",
            );
        }
        assert_eq!(backend.calls.load(std::sync::atomic::Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn wire_preserves_bounded_ids_and_accepts_only_known_notifications() {
        let (app, lease, auth, _) = wire_fixture(None, false).await;
        for id in [
            json!(0),
            json!(-9007199254740991_i64),
            json!(9007199254740991_i64),
            json!("a".repeat(256)),
        ] {
            let response = wire_post(
                &app,
                &lease,
                &auth,
                json!({"jsonrpc":"2.0","id":id,"method":"ping"}).to_string(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(
                response_json(response).await,
                json!({"jsonrpc":"2.0","id":id,"result":{}})
            );
        }
        for body in [
            json!({"jsonrpc":"2.0","method":"notifications/initialized"}),
            json!({"jsonrpc":"2.0","method":"notifications/cancelled","params":{"requestId":1,"reason":"cancelled"}}),
            json!({"jsonrpc":"2.0","method":"notifications/progress","params":{"progressToken":"p","progress":1,"total":2,"message":"working"}}),
        ] {
            let response = wire_post(&app, &lease, &auth, body.to_string()).await;
            assert_eq!(response.status(), StatusCode::ACCEPTED);
            assert!(to_bytes(response.into_body(), 1024)
                .await
                .unwrap()
                .is_empty());
        }
    }

    #[tokio::test]
    async fn wire_sanitizes_parse_auth_and_backend_errors() {
        let (app, lease, auth, backend) = wire_fixture(None, true).await;
        let response = wire_post(&app, &lease, &auth, b"\xff".to_vec()).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_wire_error(
            &response_json(response).await,
            json!(null),
            -32700,
            "request_invalid",
        );
        for target in [&lease, "lease-private"] {
            let response = wire_post(&app, target, "Bearer secret-token", "invalid").await;
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
            assert_wire_error(
                &response_json(response).await,
                json!(null),
                -32000,
                "unauthorized",
            );
        }
        let response = wire_post(
            &app,
            &lease,
            &auth,
            r#"{"jsonrpc":"2.0","id":"kept","method":"ping"}"#,
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_wire_error(&body, json!("kept"), -32003, "service_unavailable");
        assert!(!body.to_string().contains(&lease));
        assert_eq!(backend.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn wire_maps_typed_runtime_errors_without_retry_or_private_context() {
        type ErrorFactory = fn() -> HeadlessRuntimeError;
        for (error, status, rpc_code, reason, binding_state, message) in [
            (
                (|| HeadlessRuntimeError::PermissionDenied) as ErrorFactory,
                StatusCode::FORBIDDEN,
                -32004,
                "permission_denied",
                "action_required",
                "Xcode permission denied",
            ),
            (
                (|| HeadlessRuntimeError::InactiveInvocation) as ErrorFactory,
                StatusCode::OK,
                -32003,
                "workspace_stale",
                "revoked",
                "Xcode invocation inactive",
            ),
            (
                (|| HeadlessRuntimeError::ProjectHeld) as ErrorFactory,
                StatusCode::OK,
                -32003,
                "outcome_unknown",
                "action_required",
                "Xcode project held",
            ),
            (
                (|| HeadlessRuntimeError::OutcomeUnknown) as ErrorFactory,
                StatusCode::OK,
                -32003,
                "outcome_unknown",
                "action_required",
                "Xcode effect outcome unknown",
            ),
            (
                (|| HeadlessRuntimeError::AlreadyRecorded) as ErrorFactory,
                StatusCode::OK,
                -32003,
                "operation_conflict",
                "unknown",
                "Xcode effect already recorded",
            ),
        ] {
            let (app, lease, auth, backend) = wire_backend_fixture(Arc::new(WireBackend {
                calls: std::sync::atomic::AtomicUsize::new(0),
                response: None,
                fail: false,
                runtime_error: Some(error),
            }))
            .await;
            let response = wire_post(
                &app,
                &lease,
                &auth,
                r#"{"jsonrpc":"2.0","id":"correlated","method":"ping"}"#,
            )
            .await;
            assert_eq!(response.status(), status);
            let body = response_json(response).await;
            assert_wire_error(&body, json!("correlated"), rpc_code, reason);
            assert_eq!(body["error"]["data"]["retry"], "never");
            assert_eq!(body["error"]["data"]["binding_state"], binding_state);
            assert_eq!(body["error"]["message"], message);
            assert!(!body.to_string().contains(&lease));
            assert_eq!(backend.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
        }
    }

    #[tokio::test]
    async fn wire_never_passes_through_backend_errors_callbacks_or_mismatched_ids() {
        for payload in [
            json!({"jsonrpc":"2.0","id":7,"error":{"code":-32099,"message":"/private/runtime secret-token lease-private"}}),
            json!({"jsonrpc":"2.0","id":7,"method":"sampling/createMessage","params":{}}),
            json!({"jsonrpc":"2.0","id":8,"result":{}}),
        ] {
            let (app, lease, auth, _) = wire_fixture(Some(payload), false).await;
            let response = wire_post(
                &app,
                &lease,
                &auth,
                r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#,
            )
            .await;
            assert_eq!(response.status(), StatusCode::OK);
            let body = response_json(response).await;
            assert_eq!(body["id"], 7);
            assert_eq!(body["error"]["code"], -32003);
            assert!(body["error"]["data"].is_object());
            assert!(!body.to_string().contains("secret-token"));
            assert!(body.get("method").is_none());
        }
    }

    #[tokio::test]
    async fn wire_public_health_is_closed_bounded_and_redacted() {
        let pool = Arc::new(XcodeMcpBridgePool::new(XcodeMcpBridgePoolConfig {
            pool_id: "/private/runtime secret-token lease-private".to_owned(),
            broker_disabled: true,
            max_queued_leases: usize::MAX,
            ..Default::default()
        }));
        let response = routes(pool)
            .oneshot(
                Request::builder()
                    .uri("/xcode-mcp/health")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response_json(response).await;
        assert_eq!(body["state"], "disabled");
        assert_eq!(body["headless_runtime_schema_versions"], json!([1]));
        assert!(body.get("pool_id").is_none());
        assert!(body["max_queued_leases"].as_u64().unwrap() <= u32::MAX as u64);
        assert_eq!(body.as_object().unwrap().len(), 18);
        for private in [
            "/private/runtime",
            "secret-token",
            "lease-private",
            "uid",
            "pid",
            "project_path",
            "activeWorkspaces",
        ] {
            assert!(!body.to_string().contains(private));
        }
    }

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
                    .body(Body::from(r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#))
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
                    .body(Body::from(r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#))
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
        assert_wire_error(&body, json!(7), -32003, "service_unavailable");
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

        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(pool.active_lease_count().await, 0);
        assert_eq!(pool.lease_state(&lease_id).await, None);
        let body = response_json(response).await;
        assert_wire_error(&body, json!(null), -32000, "unauthorized");
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
                        r#"{"jsonrpc":"2.0","id":9,"method":"tools/call","params":{"name":"xcode.test","arguments":{}}}"#,
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();

        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let body = response_json(response).await;
        assert_eq!(body["id"], 9);
        assert_wire_error(&body, json!(9), -32004, "scope_denied");
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
            provider_runtime_home: None,
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
            input_manifest: None,
        }
    }
}
