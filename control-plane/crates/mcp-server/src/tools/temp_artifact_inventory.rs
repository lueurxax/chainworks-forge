//! P089 Managed Temporary Artifact Inventory MCP tool and resource.
//!
//! Tool: `temp_artifacts.inventory.preview`
//! Resource: `chainworks://runs/{run_id}/temp-artifact-inventory`
//!
//! In disabled mode (default), returns a disabled disposition without scanning
//! any roots. Hidden-readback and operator_visible modes are not yet implemented
//! (P089-IMPL-004); they also return a disabled response with a distinct
//! `disabled_reason_code` until the scanner is wired.

use crate::protocol::McpTool;

/// MCP tool spec for `temp_artifacts.inventory.preview`.
///
/// The input schema matches the proposal §api_contract.mcp contract:
/// - Exactly one of `run_id` or `workspace_context` required.
/// - `limit` integer 0–500, default 500.
/// - `timeout_ms` integer 1–5000, default 5000.
/// - `include_dry_run` boolean, default true.
/// - `test_root_override` string (diagnostic test-root mode only).
pub fn tool_specs() -> Vec<McpTool> {
    vec![McpTool {
        name: "temp_artifacts.inventory.preview".to_string(),
        description: "Read-only advisory preview of managed temporary artifact inventory. \
            Returns classified rows with redacted path display and HMAC path hashes. \
            Dry-run recommendations are advisory only; no cleanup, deletion, or mutation occurs. \
            Returns disabled disposition when the backend mode is 'disabled'."
            .to_string(),
        input_schema: serde_json::json!({
            "type": "object",
            "properties": {
                "run_id": {
                    "type": "string",
                    "description": "Run ID context for the inventory scan. Exactly one of run_id or workspace_context is required."
                },
                "workspace_context": {
                    "type": "object",
                    "description": "Workspace context for the inventory scan. Exactly one of run_id or workspace_context is required.",
                    "properties": {
                        "workspace_root": { "type": "string" }
                    }
                },
                "limit": {
                    "type": "integer",
                    "description": "Maximum rows to return (0–500, no clamping).",
                    "default": 500,
                    "minimum": 0,
                    "maximum": 500
                },
                "timeout_ms": {
                    "type": "integer",
                    "description": "Total request timeout in ms (1–5000); queue wait is included.",
                    "default": 5000,
                    "minimum": 1,
                    "maximum": 5000
                },
                "include_dry_run": {
                    "type": "boolean",
                    "description": "When false, dry_run is null and rows carry no dry_run_recommendation.",
                    "default": true
                },
                "test_root_override": {
                    "type": "string",
                    "description": "Absolute path override for diagnostic test-root mode (authorized callers only).",
                    "maxLength": 4096
                }
            },
            "additionalProperties": false
        }),
        output_schema: Some(serde_json::json!({
            "type": "object",
            "required": ["schema_version", "status", "enabled_state", "rows", "errors", "mutation_guard"],
            "properties": {
                "schema_version": { "type": "string", "const": "temp_artifact_inventory_v1" },
                "status": {
                    "type": "string",
                    "enum": ["complete", "partial", "timeout", "cancelled", "error", "disabled", "resource_exhausted", "unknown"]
                },
                "enabled_state": {
                    "type": "string",
                    "enum": ["enabled", "disabled", "unknown"]
                },
                "disabled_reason_code": { "type": ["string", "null"] },
                "generated_at": { "type": ["string", "null"] },
                "limits_applied": { "type": ["object", "null"] },
                "summary": { "type": ["object", "null"] },
                "rows": {
                    "type": "array",
                    "items": { "type": "object" }
                },
                "errors": {
                    "type": "array",
                    "items": {
                        "type": "object",
                        "required": ["error_code", "redacted_message"],
                        "properties": {
                            "error_code": { "type": "string" },
                            "redacted_message": { "type": "string" }
                        }
                    }
                },
                "dry_run": { "type": ["object", "null"] },
                "mutation_guard": {
                    "type": "object",
                    "required": ["status"],
                    "properties": {
                        "status": {
                            "type": "string",
                            "enum": ["pass", "fail", "skipped", "unknown"]
                        },
                        "reason": { "type": ["string", "null"] }
                    }
                }
            },
            "additionalProperties": false
        })),
    }]
}

/// Validate the `temp_artifacts.inventory.preview` request shape.
///
/// Returns `Err(reason)` if the input fails validation.
pub fn validate_input(params: &serde_json::Value) -> Result<(), String> {
    let has_run_id = params.get("run_id").and_then(|v| v.as_str()).is_some();
    let has_workspace = params.get("workspace_context").is_some();

    if has_run_id && has_workspace {
        return Err(
            "Exactly one of run_id or workspace_context must be provided, not both".to_string(),
        );
    }

    if let Some(limit) = params.get("limit").and_then(|v| v.as_i64()) {
        if !(0..=500).contains(&limit) {
            return Err(format!("limit must be 0 through 500, got {limit}"));
        }
    }

    if let Some(timeout_ms) = params.get("timeout_ms").and_then(|v| v.as_i64()) {
        if !(1..=5000).contains(&timeout_ms) {
            return Err(format!(
                "timeout_ms must be 1 through 5000, got {timeout_ms}"
            ));
        }
    }

    if let Some(override_path) = params.get("test_root_override").and_then(|v| v.as_str()) {
        if override_path.len() > 4096 {
            return Err("test_root_override must not exceed 4096 bytes".to_string());
        }
        if override_path.contains('\0') {
            return Err("test_root_override must not contain NUL bytes".to_string());
        }
        if !override_path.starts_with('/') {
            return Err("test_root_override must be an absolute path".to_string());
        }
        let normalized = normalize_path_lexical(override_path);
        if normalized != override_path {
            return Err("test_root_override must not contain traversal components".to_string());
        }
    }

    Ok(())
}

/// Lexical path normalization: resolve `.` and `..` components without filesystem access.
fn normalize_path_lexical(path: &str) -> String {
    let mut components: Vec<&str> = Vec::new();
    for component in path.split('/') {
        match component {
            "" | "." => {}
            ".." => {
                components.pop();
            }
            c => components.push(c),
        }
    }
    format!("/{}", components.join("/"))
}

/// Handle a `temp_artifacts.inventory.preview` tool call.
///
/// Returns the canonical disabled-mode DTO until P089-IMPL-004 implements
/// the hidden-readback scanner. All modes currently return a disabled response.
pub async fn execute(params: serde_json::Value) -> anyhow::Result<serde_json::Value> {
    if let Err(reason) = validate_input(&params) {
        return Ok(serde_json::json!({
            "schema_version": "temp_artifact_inventory_v1",
            "status": "error",
            "enabled_state": "unknown",
            "disabled_reason_code": null,
            "generated_at": null,
            "limits_applied": null,
            "summary": null,
            "rows": [],
            "errors": [{
                "error_code": "invalid_root_override",
                "redacted_message": reason
            }],
            "dry_run": null,
            "mutation_guard": {
                "status": "skipped",
                "reason": "validation_error"
            }
        }));
    }

    let mode = std::env::var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE")
        .ok()
        .and_then(|v| domain::temp_artifact_inventory::TempArtifactInventoryMode::from_env_str(&v))
        .unwrap_or(domain::temp_artifact_inventory::TempArtifactInventoryMode::Disabled);

    let disabled_reason = if mode.is_disabled() {
        "mode_disabled"
    } else {
        // Hidden-readback/operator_visible scanner not yet implemented.
        "scanner_not_implemented"
    };

    Ok(domain::temp_artifact_inventory::disabled_inventory_response(Some(disabled_reason)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p089_mcp_tool_spec_name_matches_proposal() {
        let specs = tool_specs();
        assert_eq!(specs.len(), 1);
        assert_eq!(specs[0].name, "temp_artifacts.inventory.preview");
    }

    #[test]
    fn p089_mcp_tool_spec_output_schema_has_required_fields() {
        let specs = tool_specs();
        let output_schema = specs[0].output_schema.as_ref().unwrap();
        let required = output_schema["required"].as_array().unwrap();
        let required_names: Vec<&str> = required.iter().filter_map(|v| v.as_str()).collect();
        assert!(
            required_names.contains(&"schema_version"),
            "output schema must require schema_version"
        );
        assert!(
            required_names.contains(&"status"),
            "output schema must require status"
        );
        assert!(
            required_names.contains(&"mutation_guard"),
            "output schema must require mutation_guard"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_rejects_relative_test_root() {
        let params = serde_json::json!({
            "run_id": "test-run",
            "test_root_override": "relative/path"
        });
        assert!(
            validate_input(&params).is_err(),
            "relative test_root_override must be rejected"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_rejects_traversal() {
        let params = serde_json::json!({
            "run_id": "test-run",
            "test_root_override": "/tmp/../etc/passwd"
        });
        assert!(
            validate_input(&params).is_err(),
            "traversal in test_root_override must be rejected"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_rejects_over_length() {
        let long_path = format!("/{}", "a".repeat(4097));
        let params = serde_json::json!({
            "run_id": "test-run",
            "test_root_override": long_path
        });
        assert!(
            validate_input(&params).is_err(),
            "over-length test_root_override must be rejected"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_rejects_nul_byte() {
        let params = serde_json::json!({
            "run_id": "test-run",
            "test_root_override": "/tmp/foo\0bar"
        });
        assert!(
            validate_input(&params).is_err(),
            "NUL byte in test_root_override must be rejected"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_rejects_both_run_id_and_workspace() {
        let params = serde_json::json!({
            "run_id": "r1",
            "workspace_context": { "workspace_root": "/tmp/ws" }
        });
        assert!(
            validate_input(&params).is_err(),
            "run_id and workspace_context cannot both be set"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_rejects_limit_out_of_range() {
        let params_high = serde_json::json!({ "run_id": "r1", "limit": 501 });
        assert!(
            validate_input(&params_high).is_err(),
            "limit > 500 must be rejected"
        );

        let params_neg = serde_json::json!({ "run_id": "r1", "limit": -1 });
        assert!(
            validate_input(&params_neg).is_err(),
            "negative limit must be rejected"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_rejects_timeout_out_of_range() {
        let params_high = serde_json::json!({ "run_id": "r1", "timeout_ms": 5001 });
        assert!(
            validate_input(&params_high).is_err(),
            "timeout_ms > 5000 must be rejected"
        );

        let params_zero = serde_json::json!({ "run_id": "r1", "timeout_ms": 0 });
        assert!(
            validate_input(&params_zero).is_err(),
            "timeout_ms = 0 must be rejected"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_accepts_valid() {
        let params = serde_json::json!({
            "run_id": "r1",
            "limit": 100,
            "timeout_ms": 3000,
            "include_dry_run": false
        });
        assert!(
            validate_input(&params).is_ok(),
            "valid params must pass validation"
        );
    }

    #[test]
    fn p089_mcp_tool_input_validate_accepts_valid_absolute_test_root() {
        let params = serde_json::json!({
            "run_id": "r1",
            "test_root_override": "/tmp/chainworks-test-root"
        });
        assert!(
            validate_input(&params).is_ok(),
            "absolute non-traversal path must pass"
        );
    }

    #[tokio::test]
    async fn p089_mcp_tool_execute_disabled_mode_returns_correct_response() {
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let result = execute(serde_json::json!({ "run_id": "test-run" }))
            .await
            .unwrap();
        assert_eq!(result["schema_version"], "temp_artifact_inventory_v1");
        assert_eq!(result["status"], "disabled");
        assert_eq!(result["enabled_state"], "disabled");
        assert!(result["rows"].as_array().unwrap().is_empty());
        assert!(result["errors"].as_array().unwrap().is_empty());
        assert_eq!(result["mutation_guard"]["status"], "skipped");
    }

    #[tokio::test]
    async fn p089_mcp_tool_execute_validation_error_returns_error_status() {
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        let result = execute(serde_json::json!({
            "run_id": "r1",
            "test_root_override": "not-absolute"
        }))
        .await
        .unwrap();
        assert_eq!(result["status"], "error");
        let errors = result["errors"].as_array().unwrap();
        assert!(!errors.is_empty());
        assert_eq!(errors[0]["error_code"], "invalid_root_override");
    }

    #[test]
    fn p089_path_normalize_lexical_resolves_dot_dot() {
        assert_eq!(normalize_path_lexical("/tmp/../etc"), "/etc");
        assert_eq!(normalize_path_lexical("/tmp/./foo"), "/tmp/foo");
        assert_eq!(normalize_path_lexical("/tmp/foo/"), "/tmp/foo");
        assert_eq!(normalize_path_lexical("/tmp/foo"), "/tmp/foo");
    }
}
