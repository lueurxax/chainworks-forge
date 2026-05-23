use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsonRpcRequest {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    pub method: String,
    pub params: Option<serde_json::Value>,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsonRpcResponse {
    pub jsonrpc: String,
    pub id: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<serde_json::Value>, result: serde_json::Value) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: Some(result),
            error: None,
        }
    }

    pub fn error(id: Option<serde_json::Value>, code: i32, message: String) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: None,
            }),
        }
    }

    pub fn error_with_data(
        id: Option<serde_json::Value>,
        code: i32,
        message: String,
        data: serde_json::Value,
    ) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message,
                data: Some(data),
            }),
        }
    }

    /// R12 API-001: ensure every outbound error response carries the
    /// ambient request id so an operator can pivot from a failed MCP
    /// call to logs and `command_journal` rows. No-op when the
    /// response is a success or when `request_id` is `None`.
    pub fn with_error_request_id(mut self, request_id: Option<&str>) -> Self {
        if let Some(rid) = request_id {
            if let Some(err) = self.error.take() {
                self.error = Some(err.with_request_id(rid));
            }
        }
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct JsonRpcError {
    pub code: i32,
    pub message: String,
    /// JSON-RPC 2.0 `error.data` auxiliary field. P042 §9.3 / AC-15
    /// stores the ambient `X-Request-ID` here so operators can
    /// correlate a failed MCP call with log lines and
    /// `command_journal.request_id` rows.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<serde_json::Value>,
}

impl JsonRpcError {
    /// Attach (or overwrite) the request id on this error's `data`
    /// object. If `data` is already a JSON object we merge into it;
    /// otherwise we replace with `{"request_id": ...}`.
    pub fn with_request_id(mut self, request_id: &str) -> Self {
        let entry = serde_json::json!({ "request_id": request_id });
        match self.data.take() {
            Some(serde_json::Value::Object(mut obj)) => {
                obj.insert(
                    "request_id".to_string(),
                    serde_json::Value::String(request_id.to_string()),
                );
                self.data = Some(serde_json::Value::Object(obj));
            }
            _ => {
                self.data = Some(entry);
            }
        }
        self
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct McpTool {
    pub name: String,
    pub description: String,
    pub input_schema: serde_json::Value,
}
