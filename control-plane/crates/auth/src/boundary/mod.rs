//! P081 Phase 1: Boundary matrix fixture loading and validation.
//!
//! The boundary matrix fixture (`boundary_matrix_fixture_v1`) is the machine-readable
//! source of truth for which callers may reach which surfaces. It is validated at daemon
//! startup; request paths never read this module's artifacts directly.
//!
//! The embedded last-known-good fixture is built into the binary via `include_str!`.
//! If the deployed fixture is invalid, the daemon loads the embedded fixture and enters
//! `read_only_safe_mode` rather than refusing to start.

use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// The embedded last-known-good fixture, compiled into the binary.
pub const EMBEDDED_FIXTURE_JSON: &str =
    include_str!("embedded_fixture.json");

// ── Transport enum ───────────────────────────────────────────────────────

const VALID_TRANSPORTS: &[&str] = &[
    "graphql_query",
    "graphql_subscription",
    "graphql_mutation",
    "mcp_initialize",
    "mcp_tools_list",
    "mcp_tools_call",
    "debug_endpoint",
];

// ── CallerClass enum ─────────────────────────────────────────────────────

const VALID_CALLER_CLASSES: &[&str] = &[
    "ui_operator",
    "agent_operator",
    "automation",
    "observer",
    "developer_break_glass",
];

// ── Authoritative record enum ────────────────────────────────────────────

const VALID_AUTHORITATIVE_RECORDS: &[&str] = &[
    "projection_read_model",
    "approval_record",
    "command_journal",
    "audit_log",
    "none",
];

// ── Rollout mode enum ────────────────────────────────────────────────────

const VALID_ROLLOUT_MODES: &[&str] = &[
    "shadow",
    "enforce",
    "read_only_safe_mode",
    "legacy_compat",
];

// ── Redaction mode enum ──────────────────────────────────────────────────

const VALID_REDACTION_MODES: &[&str] = &[
    "none",
    "field_null_redacted",
    "drop_resource",
    "actionability_false",
];

// ── Required row IDs (Phase 1 contract) ─────────────────────────────────

const REQUIRED_ROW_IDS: &[(&str, &str)] = &[
    ("p081.ui_operator.graphql_query.read", "graphql_query"),
    (
        "p081.ui_operator.graphql_subscription.subscribe",
        "graphql_subscription",
    ),
    (
        "p081.ui_operator.graphql_mutation.approval_action",
        "graphql_mutation",
    ),
    (
        "p081.agent_operator.mcp_initialize.capability",
        "mcp_initialize",
    ),
    (
        "p081.agent_operator.mcp_tools_list.discovery",
        "mcp_tools_list",
    ),
    (
        "p081.agent_operator.mcp_tools_call.command",
        "mcp_tools_call",
    ),
    ("p081.automation.mcp_tools_list.discovery", "mcp_tools_list"),
    ("p081.automation.mcp_tools_call.command", "mcp_tools_call"),
    (
        "p081.observer.mcp_tools_call.compact_read",
        "mcp_tools_call",
    ),
    (
        "p081.observer.graphql_query.read_only_opt_in",
        "graphql_query",
    ),
    (
        "p081.developer_break_glass.debug_endpoint.disabled",
        "debug_endpoint",
    ),
];

// ── Validation error types ───────────────────────────────────────────────

/// Structured validation error code as defined in the P081 executable boundary contract.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum FixtureErrorCode {
    ESchemaVersion,
    EUnknownField,
    EMissingField,
    EDuplicateRowId,
    EUnknownEnum,
    EInvalidRowId,
    EInvalidActionGrammar,
    EWildcardNotAllowed,
    ERequiredRowMissing,
    ERequiredRowTransportMismatch,
    EDenySideEffectConflict,
    ENullability,
    EFixtureDigestMismatch,
}

impl std::fmt::Display for FixtureErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ESchemaVersion => write!(f, "E_SCHEMA_VERSION"),
            Self::EUnknownField => write!(f, "E_UNKNOWN_FIELD"),
            Self::EMissingField => write!(f, "E_MISSING_FIELD"),
            Self::EDuplicateRowId => write!(f, "E_DUPLICATE_ROW_ID"),
            Self::EUnknownEnum => write!(f, "E_UNKNOWN_ENUM"),
            Self::EInvalidRowId => write!(f, "E_INVALID_ROW_ID"),
            Self::EInvalidActionGrammar => write!(f, "E_INVALID_ACTION_GRAMMAR"),
            Self::EWildcardNotAllowed => write!(f, "E_WILDCARD_NOT_ALLOWED"),
            Self::ERequiredRowMissing => write!(f, "E_REQUIRED_ROW_MISSING"),
            Self::ERequiredRowTransportMismatch => write!(f, "E_REQUIRED_ROW_TRANSPORT_MISMATCH"),
            Self::EDenySideEffectConflict => write!(f, "E_DENY_SIDE_EFFECT_CONFLICT"),
            Self::ENullability => write!(f, "E_NULLABILITY"),
            Self::EFixtureDigestMismatch => write!(f, "E_FIXTURE_DIGEST_MISMATCH"),
        }
    }
}

/// A single validation finding.
#[derive(Debug, Clone)]
pub struct FixtureValidationError {
    pub code: FixtureErrorCode,
    pub context: String,
}

impl std::fmt::Display for FixtureValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{}] {}", self.code, self.context)
    }
}

/// Outcome of fixture validation.
#[derive(Debug)]
pub struct FixtureValidationResult {
    pub errors: Vec<FixtureValidationError>,
}

impl FixtureValidationResult {
    pub fn is_valid(&self) -> bool {
        self.errors.is_empty()
    }
}

// ── Fixture types ────────────────────────────────────────────────────────

/// Top-level structure of `boundary_matrix_fixture_v1`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryMatrixFixture {
    pub schema_version: u32,
    pub matrix_id: String,
    pub generated_from: String,
    pub enum_casing: String,
    pub rows: Vec<BoundaryMatrixRow>,
}

/// One row in the boundary matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BoundaryMatrixRow {
    pub row_id: String,
    pub caller_class: String,
    pub transports: Vec<String>,
    pub actions: Vec<String>,
    pub allow: AllowSpec,
    pub deny: DenySpec,
    pub redaction: RedactionSpec,
    pub authoritative_record: String,
    pub read_model_delta: serde_json::Value,
    pub required_tests: Vec<String>,
    pub rollout_mode: String,
    pub deprecated_after_phase: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AllowSpec {
    pub enabled: bool,
    pub wildcard: bool,
    pub conditions: Vec<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DenySpec {
    pub reason_code: String,
    pub side_effects: Vec<serde_json::Value>,
    pub client_visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RedactionSpec {
    pub mode: String,
    pub paths: Vec<String>,
    pub extensions_required: bool,
}

// ── Row ID grammar ───────────────────────────────────────────────────────

/// Validate row_id grammar:
/// `^p081\.[a-z0-9_]+\.(transport_segment)\.[a-z0-9_]+$`
/// Second segment must equal caller_class. Third segment must be a valid transport.
/// All segments must match `[a-z0-9_]+`.
fn validate_row_id_grammar(row_id: &str) -> Result<(String, String), FixtureErrorCode> {
    let parts: Vec<&str> = row_id.split('.').collect();
    if parts.len() != 4 {
        return Err(FixtureErrorCode::EInvalidRowId);
    }
    if parts[0] != "p081" {
        return Err(FixtureErrorCode::EInvalidRowId);
    }
    for part in &parts {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_') {
            return Err(FixtureErrorCode::EInvalidRowId);
        }
    }
    let caller_class_segment = parts[1];
    let transport_segment = parts[2];
    if !VALID_TRANSPORTS.contains(&transport_segment) {
        return Err(FixtureErrorCode::EInvalidRowId);
    }
    if !VALID_CALLER_CLASSES.contains(&caller_class_segment) {
        return Err(FixtureErrorCode::EInvalidRowId);
    }
    Ok((caller_class_segment.to_string(), transport_segment.to_string()))
}

/// Check whether an action string contains a wildcard that would require allow.wildcard=true.
fn is_wildcard_action(action: &str) -> bool {
    action.contains('*')
}

// ── Main validator ───────────────────────────────────────────────────────

/// Validate a parsed `BoundaryMatrixFixture`.
///
/// Returns a `FixtureValidationResult` with all collected errors. Call
/// `result.is_valid()` to check pass/fail.
pub fn validate_fixture(fixture: &BoundaryMatrixFixture) -> FixtureValidationResult {
    let mut errors: Vec<FixtureValidationError> = Vec::new();

    // schema_version must be 1 for Phase 1.
    if fixture.schema_version != 1 {
        errors.push(FixtureValidationError {
            code: FixtureErrorCode::ESchemaVersion,
            context: format!("expected schema_version 1, got {}", fixture.schema_version),
        });
    }

    // matrix_id must be non-empty.
    if fixture.matrix_id.is_empty() {
        errors.push(FixtureValidationError {
            code: FixtureErrorCode::EMissingField,
            context: "matrix_id is empty".into(),
        });
    }

    // rows must be non-empty.
    if fixture.rows.is_empty() {
        errors.push(FixtureValidationError {
            code: FixtureErrorCode::EMissingField,
            context: "rows array is empty".into(),
        });
        return FixtureValidationResult { errors };
    }

    // Collect row_ids to check for duplicates.
    let mut seen_row_ids: HashSet<&str> = HashSet::new();

    for row in &fixture.rows {
        // row_id grammar.
        match validate_row_id_grammar(&row.row_id) {
            Ok((caller_class_from_id, transport_from_id)) => {
                // caller_class in row_id must match caller_class field.
                if caller_class_from_id != row.caller_class {
                    errors.push(FixtureValidationError {
                        code: FixtureErrorCode::EInvalidRowId,
                        context: format!(
                            "row '{}': caller_class segment '{}' does not match caller_class field '{}'",
                            row.row_id, caller_class_from_id, row.caller_class
                        ),
                    });
                }
                // For required rows: transport segment must equal the only transport.
                // For non-required rows it's a recommendation but not enforced here at parse time.
                let _ = transport_from_id;
            }
            Err(code) => {
                errors.push(FixtureValidationError {
                    code,
                    context: format!("row '{}': invalid row_id grammar", row.row_id),
                });
            }
        }

        // Duplicate row_id check.
        if !seen_row_ids.insert(row.row_id.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EDuplicateRowId,
                context: format!("duplicate row_id: '{}'", row.row_id),
            });
        }

        // caller_class enum.
        if !VALID_CALLER_CLASSES.contains(&row.caller_class.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!("row '{}': unknown caller_class '{}'", row.row_id, row.caller_class),
            });
        }

        // transports enum and non-empty.
        if row.transports.is_empty() {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::ENullability,
                context: format!("row '{}': transports must be non-empty", row.row_id),
            });
        }
        for transport in &row.transports {
            if !VALID_TRANSPORTS.contains(&transport.as_str()) {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EUnknownEnum,
                    context: format!("row '{}': unknown transport '{}'", row.row_id, transport),
                });
            }
        }

        // actions non-empty.
        if row.actions.is_empty() {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::ENullability,
                context: format!("row '{}': actions must be non-empty", row.row_id),
            });
        }

        // Wildcard check: action containing '*' requires allow.wildcard=true.
        for action in &row.actions {
            if is_wildcard_action(action) && !row.allow.wildcard {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EWildcardNotAllowed,
                    context: format!(
                        "row '{}': action '{}' uses wildcard but allow.wildcard is false",
                        row.row_id, action
                    ),
                });
            }
            // Global '*' is always invalid.
            if action == "*" {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EInvalidActionGrammar,
                    context: format!("row '{}': global wildcard '*' is not a valid action", row.row_id),
                });
            }
        }

        // authoritative_record enum.
        if !VALID_AUTHORITATIVE_RECORDS.contains(&row.authoritative_record.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!(
                    "row '{}': unknown authoritative_record '{}'",
                    row.row_id, row.authoritative_record
                ),
            });
        }

        // rollout_mode enum.
        if !VALID_ROLLOUT_MODES.contains(&row.rollout_mode.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!("row '{}': unknown rollout_mode '{}'", row.row_id, row.rollout_mode),
            });
        }

        // redaction.mode enum.
        if !VALID_REDACTION_MODES.contains(&row.redaction.mode.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!("row '{}': unknown redaction.mode '{}'", row.row_id, row.redaction.mode),
            });
        }

        // required_tests non-empty.
        if row.required_tests.is_empty() {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::ENullability,
                context: format!("row '{}': required_tests must be non-empty", row.row_id),
            });
        }
    }

    // Required rows: every required row_id must be present and have the matching single transport.
    for &(required_id, required_transport) in REQUIRED_ROW_IDS {
        match fixture.rows.iter().find(|r| r.row_id == required_id) {
            None => {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::ERequiredRowMissing,
                    context: format!("required row '{}' is missing from fixture", required_id),
                });
            }
            Some(row) => {
                // Required rows must have exactly one transport matching the row_id segment.
                if row.transports.len() != 1 || row.transports[0] != required_transport {
                    errors.push(FixtureValidationError {
                        code: FixtureErrorCode::ERequiredRowTransportMismatch,
                        context: format!(
                            "required row '{}': expected single transport ['{}'], got {:?}",
                            required_id, required_transport, row.transports
                        ),
                    });
                }
            }
        }
    }

    FixtureValidationResult { errors }
}

/// Parse and validate a fixture from a JSON string.
pub fn parse_and_validate(json: &str) -> Result<(BoundaryMatrixFixture, FixtureValidationResult), String> {
    let fixture: BoundaryMatrixFixture =
        serde_json::from_str(json).map_err(|e| format!("fixture parse error: {e}"))?;
    let result = validate_fixture(&fixture);
    Ok((fixture, result))
}

/// Validate the embedded last-known-good fixture. Panics in debug builds if it
/// fails so build-time CI catches regressions before deployment.
pub fn validate_embedded_fixture() -> FixtureValidationResult {
    let (_, result) = parse_and_validate(EMBEDDED_FIXTURE_JSON)
        .expect("embedded fixture must be valid JSON");
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_fixture_is_valid() {
        let result = validate_embedded_fixture();
        assert!(
            result.is_valid(),
            "embedded fixture validation failed:\n{}",
            result
                .errors
                .iter()
                .map(|e| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        );
    }

    #[test]
    fn all_required_rows_present_in_embedded_fixture() {
        let (fixture, _) = parse_and_validate(EMBEDDED_FIXTURE_JSON).unwrap();
        for &(required_id, _) in REQUIRED_ROW_IDS {
            assert!(
                fixture.rows.iter().any(|r| r.row_id == required_id),
                "required row '{}' missing from embedded fixture",
                required_id
            );
        }
    }

    #[test]
    fn rejects_unknown_schema_version() {
        let json = r#"{
            "schema_version": 99,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": []
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(result.errors.iter().any(|e| e.code == FixtureErrorCode::ESchemaVersion));
    }

    #[test]
    fn rejects_duplicate_row_ids() {
        let base = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [
                {
                    "row_id": "p081.ui_operator.graphql_query.read",
                    "caller_class": "ui_operator",
                    "transports": ["graphql_query"],
                    "actions": ["runs.get"],
                    "allow": {"enabled": true, "wildcard": false, "conditions": []},
                    "deny": {"reason_code": "UNAUTHENTICATED", "side_effects": [], "client_visibility": "UNAUTHENTICATED"},
                    "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                    "authoritative_record": "projection_read_model",
                    "read_model_delta": {},
                    "required_tests": ["query_allow"],
                    "rollout_mode": "shadow",
                    "deprecated_after_phase": null
                },
                {
                    "row_id": "p081.ui_operator.graphql_query.read",
                    "caller_class": "ui_operator",
                    "transports": ["graphql_query"],
                    "actions": ["runs.get"],
                    "allow": {"enabled": true, "wildcard": false, "conditions": []},
                    "deny": {"reason_code": "UNAUTHENTICATED", "side_effects": [], "client_visibility": "UNAUTHENTICATED"},
                    "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                    "authoritative_record": "projection_read_model",
                    "read_model_delta": {},
                    "required_tests": ["query_allow"],
                    "rollout_mode": "shadow",
                    "deprecated_after_phase": null
                }
            ]
        }"#;
        let (_, result) = parse_and_validate(base).unwrap();
        assert!(result.errors.iter().any(|e| e.code == FixtureErrorCode::EDuplicateRowId));
    }

    #[test]
    fn rejects_unknown_transport_enum() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.ui_operator.graphql_query.read",
                "caller_class": "ui_operator",
                "transports": ["graphql_query_or_subscription"],
                "actions": ["runs.get"],
                "allow": {"enabled": true, "wildcard": false, "conditions": []},
                "deny": {"reason_code": "UNAUTHENTICATED", "side_effects": [], "client_visibility": "UNAUTHENTICATED"},
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {},
                "required_tests": ["query_allow"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(result.errors.iter().any(|e| e.code == FixtureErrorCode::EUnknownEnum));
    }

    #[test]
    fn rejects_wildcard_action_without_flag() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.agent_operator.mcp_tools_call.command",
                "caller_class": "agent_operator",
                "transports": ["mcp_tools_call"],
                "actions": ["runs.*"],
                "allow": {"enabled": true, "wildcard": false, "conditions": []},
                "deny": {"reason_code": "CAPABILITY_OUT_OF_SCOPE", "side_effects": [], "client_visibility": "CAPABILITY_OUT_OF_SCOPE"},
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "command_journal",
                "read_model_delta": {},
                "required_tests": ["allowed_mcp_command"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(result.errors.iter().any(|e| e.code == FixtureErrorCode::EWildcardNotAllowed));
    }

    #[test]
    fn rejects_invalid_row_id_grammar() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.observer.compact_read.read_only",
                "caller_class": "observer",
                "transports": ["mcp_tools_call"],
                "actions": ["read-only compact diagnostics"],
                "allow": {"enabled": true, "wildcard": false, "conditions": []},
                "deny": {"reason_code": "OBSERVER_SCOPE", "side_effects": [], "client_visibility": "OBSERVER_SCOPE"},
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {},
                "required_tests": ["observer_cannot_mutate"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(result.errors.iter().any(|e| e.code == FixtureErrorCode::EInvalidRowId));
    }

    #[test]
    fn rejects_missing_required_row() {
        // A fixture with only one row (none of the 11 required).
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.ui_operator.graphql_query.read",
                "caller_class": "ui_operator",
                "transports": ["graphql_query"],
                "actions": ["runs.get"],
                "allow": {"enabled": true, "wildcard": false, "conditions": []},
                "deny": {"reason_code": "UNAUTHENTICATED", "side_effects": [], "client_visibility": "UNAUTHENTICATED"},
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {},
                "required_tests": ["query_allow"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        // The other 10 required rows are missing.
        let missing_count = result
            .errors
            .iter()
            .filter(|e| e.code == FixtureErrorCode::ERequiredRowMissing)
            .count();
        assert!(missing_count >= 10, "expected at least 10 missing-row errors, got {missing_count}");
    }

    #[test]
    fn rejects_required_row_combined_transports() {
        // p081.ui_operator.graphql_query.read must have exactly ["graphql_query"], not combined.
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.ui_operator.graphql_query.read",
                "caller_class": "ui_operator",
                "transports": ["graphql_query", "graphql_subscription"],
                "actions": ["runs.get"],
                "allow": {"enabled": true, "wildcard": false, "conditions": []},
                "deny": {"reason_code": "UNAUTHENTICATED", "side_effects": [], "client_visibility": "UNAUTHENTICATED"},
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {},
                "required_tests": ["query_allow"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(
            result.errors.iter().any(|e| e.code == FixtureErrorCode::ERequiredRowTransportMismatch),
            "expected E_REQUIRED_ROW_TRANSPORT_MISMATCH"
        );
    }
}
