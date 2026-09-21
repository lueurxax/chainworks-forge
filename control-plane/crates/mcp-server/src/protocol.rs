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

    /// Build an error response with structured `data`. Used for INTERNAL errors
    /// so callers can include a stable code and request_id without leaking raw
    /// error strings.
    pub fn error_with_data(
        id: Option<serde_json::Value>,
        code: i32,
        message: impl Into<String>,
        data: serde_json::Value,
    ) -> Self {
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: Some(data),
            }),
        }
    }

    /// Build a policy-denial error response per P081 MCP contract:
    /// known-but-denied tools return -32004 with structured data containing
    /// reason_code, caller_class, row_id, and boundary_policy_version.
    pub fn policy_denial(
        id: Option<serde_json::Value>,
        reason_code: &str,
        caller_class: &str,
        row_id: Option<&str>,
        policy_version: &str,
    ) -> Self {
        let data = serde_json::json!({
            "reason_code": reason_code,
            "caller_class": caller_class,
            "row_id": row_id,
            "boundary_policy_version": policy_version,
        });
        JsonRpcResponse {
            jsonrpc: "2.0".to_string(),
            id,
            result: None,
            error: Some(JsonRpcError {
                code: -32004,
                message: "tool denied".to_string(),
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
        // P039's uncertain-commit data is an exact closed recovery instruction.
        // Correlation remains in transport headers/log context, not this payload.
        if self.data.as_ref().is_some_and(|data| {
            serde_json::from_value::<domain::run_carry_forward_api::P039TransportErrorV1>(
                data.clone(),
            )
            .is_ok()
        }) {
            return self;
        }
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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<serde_json::Value>,
}

#[derive(Default)]
pub(crate) struct RawRequestCheck {
    pub is_continuation: bool,
    pub duplicate_key: Option<String>,
    invalid_envelope: bool,
}

#[derive(Clone, Copy)]
enum JsonPosition {
    Root,
    Params,
    ToolName,
    Other,
}

struct RawSeed<'a> {
    check: &'a mut RawRequestCheck,
    position: JsonPosition,
}

impl<'de> serde::de::DeserializeSeed<'de> for RawSeed<'_> {
    type Value = ();
    fn deserialize<D: serde::Deserializer<'de>>(self, deserializer: D) -> Result<(), D::Error> {
        deserializer.deserialize_any(self)
    }
}

impl<'de> serde::de::Visitor<'de> for RawSeed<'_> {
    type Value = ();
    fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("JSON value")
    }
    fn visit_bool<E: serde::de::Error>(self, _: bool) -> Result<(), E> {
        Ok(())
    }
    fn visit_i64<E: serde::de::Error>(self, _: i64) -> Result<(), E> {
        Ok(())
    }
    fn visit_u64<E: serde::de::Error>(self, _: u64) -> Result<(), E> {
        Ok(())
    }
    fn visit_f64<E: serde::de::Error>(self, _: f64) -> Result<(), E> {
        Ok(())
    }
    fn visit_unit<E: serde::de::Error>(self) -> Result<(), E> {
        Ok(())
    }
    fn visit_str<E: serde::de::Error>(self, value: &str) -> Result<(), E> {
        if matches!(self.position, JsonPosition::ToolName)
            && crate::tools::run_continuations::is_tool(value)
        {
            self.check.is_continuation = true;
        }
        Ok(())
    }
    fn visit_seq<A: serde::de::SeqAccess<'de>>(self, mut seq: A) -> Result<(), A::Error> {
        while seq
            .next_element_seed(RawSeed {
                check: self.check,
                position: JsonPosition::Other,
            })?
            .is_some()
        {}
        Ok(())
    }
    fn visit_map<A: serde::de::MapAccess<'de>>(self, mut map: A) -> Result<(), A::Error> {
        let mut keys = std::collections::BTreeSet::new();
        while let Some(key) = map.next_key::<String>()? {
            if !keys.insert(key.clone()) && self.check.duplicate_key.is_none() {
                self.check.duplicate_key = Some(key.clone());
            }
            let position = match (self.position, key.as_str()) {
                (JsonPosition::Root, "params") => JsonPosition::Params,
                (JsonPosition::Params, "name") => JsonPosition::ToolName,
                (JsonPosition::Root, key) if !matches!(key, "jsonrpc" | "id" | "method") => {
                    self.check.invalid_envelope = true;
                    JsonPosition::Other
                }
                _ => JsonPosition::Other,
            };
            map.next_value_seed(RawSeed {
                check: self.check,
                position,
            })?;
        }
        Ok(())
    }
}

/// Streaming structural pass BEFORE constructing any serde_json::Value. Decode
/// escaped keys with serde, retain duplicate evidence, and scan the entire bounded
/// input even when the tool name follows a duplicate field. serde's depth bound stays on.
pub(crate) fn check_raw_request(bytes: &[u8]) -> Result<RawRequestCheck, JsonRpcResponse> {
    use serde::de::DeserializeSeed;
    if bytes.len() > domain::run_carry_forward_api::MAX_REQUEST_BYTES {
        return Err(JsonRpcResponse::error(
            None,
            -32602,
            "request exceeds 1 MiB".into(),
        ));
    }
    let mut check = RawRequestCheck::default();
    let mut deserializer = serde_json::Deserializer::from_slice(bytes);
    RawSeed {
        check: &mut check,
        position: JsonPosition::Root,
    }
    .deserialize(&mut deserializer)
    .and_then(|()| deserializer.end())
    .map_err(|_| JsonRpcResponse::error(None, -32700, "invalid JSON".into()))?;
    if check.is_continuation && (check.duplicate_key.is_some() || check.invalid_envelope) {
        return Err(JsonRpcResponse::error(
            None,
            -32602,
            "invalid continuation parameters".into(),
        ));
    }
    Ok(check)
}

pub(crate) fn authentication_denial(id: Option<serde_json::Value>, p039: bool) -> JsonRpcResponse {
    if p039 {
        JsonRpcResponse::policy_denial(
            id,
            "CAPABILITY_OUT_OF_SCOPE",
            "unknown",
            None,
            "p081-boundary-matrix-v1",
        )
    } else {
        JsonRpcResponse::error(id, -32000, "unauthorized".into())
    }
}

#[cfg(test)]
mod p039_tests {
    use super::*;

    #[test]
    fn p039_raw_scan_handles_duplicate_envelopes_late_names_and_unicode() {
        for raw in [
            r#"{"params":{"arguments":{"x":0,"x":1},"name":"runs_continuation_get"},"method":"tools/call"}"#,
            r#"{"method":"tools/call","method":"initialize","params":{"name":"runs.continuation_get"}}"#,
            r#"{"params":{"name":"runs.continuation_get","name":"runs.get"}}"#,
            r#"{"params":{"arguments":{"é":0,"\u00e9":1},"name":"runs\u002econtinuation_get"}}"#,
            r#"{"params":{"name":"runs.continuation_get"},"params":{"name":"runs.get"}}"#,
            r#"{"params":{"name":"runs.continuation_get"},"caller_class":"operator"}"#,
        ] {
            assert_eq!(
                check_raw_request(raw.as_bytes())
                    .err()
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                -32602,
                "{raw}"
            );
        }
        assert!(check_raw_request(br#"{"params":{"name":"runs.continuation_get","arguments":{"cursor":"escaped \\\" duplicate text"}}}"#).is_ok());
    }

    #[test]
    fn p039_raw_scan_rejects_trailing_malformed_overdeep_and_oversized_json() {
        for raw in [
            "{}{}".to_owned(),
            "{".to_owned(),
            format!("{}0{}", "[".repeat(130), "]".repeat(130)),
        ] {
            assert_eq!(
                check_raw_request(raw.as_bytes())
                    .err()
                    .unwrap()
                    .error
                    .unwrap()
                    .code,
                -32700
            );
        }
        assert_eq!(
            check_raw_request(&vec![b' '; 1024 * 1024 + 1])
                .err()
                .unwrap()
                .error
                .unwrap()
                .code,
            -32602
        );
        assert!(check_raw_request(b"{\"x\":\"\xff\"}").is_err());
    }
}
