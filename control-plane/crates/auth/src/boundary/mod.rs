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
pub const EMBEDDED_FIXTURE_JSON: &str = include_str!("embedded_fixture.json");

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

const VALID_ROLLOUT_MODES: &[&str] = &["shadow", "enforce", "read_only_safe_mode", "legacy_compat"];

// ── Redaction mode enum ──────────────────────────────────────────────────

const VALID_REDACTION_MODES: &[&str] = &[
    "none",
    "field_null_redacted",
    "drop_resource",
    "actionability_false",
];

// ── Denial reason code enum (deny.reason_code) ───────────────────────────

const VALID_DENIAL_REASON_CODES: &[&str] = &[
    "UNAUTHENTICATED",
    "AMBIGUOUS_CALLER",
    "CAPABILITY_OUT_OF_SCOPE",
    "NON_APPROVAL_MUTATION",
    "APPROVAL_NOT_ACTIONABLE",
    "OBSERVER_SCOPE",
    "BREAK_GLASS_DISABLED",
    "MATRIX_NO_ROW",
    "E_AUDIT_UNAVAILABLE",
    "E_FIXTURE_DIGEST_MISMATCH",
    "SQLITE_CONTENTION_RETRY_EXHAUSTED",
    "IDEMPOTENCY_CONFLICT",
];

// ── Client visibility enum (deny.client_visibility) ──────────────────────
// Extends the denial_reason_code set with HTTP/GraphQL layer codes.

const VALID_CLIENT_VISIBILITY_CODES: &[&str] = &[
    "UNAUTHENTICATED",
    "AMBIGUOUS_CALLER",
    "CAPABILITY_OUT_OF_SCOPE",
    "NON_APPROVAL_MUTATION",
    "APPROVAL_NOT_ACTIONABLE",
    "OBSERVER_SCOPE",
    "BREAK_GLASS_DISABLED",
    "MATRIX_NO_ROW",
    "E_AUDIT_UNAVAILABLE",
    "E_FIXTURE_DIGEST_MISMATCH",
    "SQLITE_CONTENTION_RETRY_EXHAUSTED",
    "IDEMPOTENCY_CONFLICT",
    // HTTP/GraphQL layer codes for denial visibility at the response level.
    "FORBIDDEN",
    "UNAUTHORIZED",
];

// ── Valid enum_casing values ─────────────────────────────────────────────

const VALID_ENUM_CASINGS: &[&str] = &["snake_case", "SCREAMING_SNAKE_CASE"];

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
#[serde(deny_unknown_fields)]
pub struct BoundaryMatrixFixture {
    pub schema_version: u32,
    pub matrix_id: String,
    pub generated_from: String,
    pub enum_casing: String,
    pub rows: Vec<BoundaryMatrixRow>,
}

/// Typed read_model_delta metadata.
/// Replaces serde_json::Value so unknown fields are rejected at parse time (M-003).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ReadModelDelta {
    #[serde(default)]
    pub scope: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deny_behavior: Option<String>,
}

/// Typed allow condition placeholder.
/// No conditions are defined in P081 Phase 1-3; non-empty arrays are rejected
/// by validate_fixture. The empty struct with deny_unknown_fields ensures that
/// any object element (e.g. {"type": "..."}) is rejected at parse time.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowCondition {}

/// Typed deny side-effect entry.
/// Only "audit_log_row" is a valid side effect on deny paths.
/// deny_unknown_fields ensures unknown properties within side-effect objects
/// are rejected at parse time (M-003).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenySideEffect {
    #[serde(rename = "type")]
    pub effect_type: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub event_type: Option<String>,
}

/// One row in the boundary matrix.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BoundaryMatrixRow {
    pub row_id: String,
    pub caller_class: String,
    pub transports: Vec<String>,
    pub actions: Vec<String>,
    pub allow: AllowSpec,
    pub deny: DenySpec,
    pub redaction: RedactionSpec,
    pub authoritative_record: String,
    pub read_model_delta: ReadModelDelta,
    pub required_tests: Vec<String>,
    pub rollout_mode: String,
    pub deprecated_after_phase: Option<u32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AllowSpec {
    pub enabled: bool,
    pub wildcard: bool,
    pub conditions: Vec<AllowCondition>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DenySpec {
    pub reason_code: String,
    pub side_effects: Vec<DenySideEffect>,
    pub client_visibility: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
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
        if part.is_empty()
            || !part
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
        {
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
    Ok((
        caller_class_segment.to_string(),
        transport_segment.to_string(),
    ))
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

    // enum_casing must be a known value.
    if !VALID_ENUM_CASINGS.contains(&fixture.enum_casing.as_str()) {
        errors.push(FixtureValidationError {
            code: FixtureErrorCode::EUnknownEnum,
            context: format!(
                "unknown enum_casing '{}'; expected one of {:?}",
                fixture.enum_casing, VALID_ENUM_CASINGS
            ),
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

    // Collect row_ids and caller_class+transport pairs to check for duplicates.
    // Duplicate caller_class+transport rows make policy evaluation order-dependent (M-001 fix).
    let mut seen_row_ids: HashSet<&str> = HashSet::new();
    let mut seen_caller_transport: HashSet<String> = HashSet::new();

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

        // Duplicate caller_class+transport check (M-001): duplicate pairs make
        // BoundaryPolicy::evaluate order-dependent and ambiguous.
        for transport in &row.transports {
            let key = format!("{}:{}", row.caller_class, transport);
            if !seen_caller_transport.insert(key) {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EDuplicateRowId,
                    context: format!(
                        "row '{}': duplicate caller_class '{}' + transport '{}' combination; split into separate rows or deduplicate",
                        row.row_id, row.caller_class, transport
                    ),
                });
            }
        }

        // caller_class enum.
        if !VALID_CALLER_CLASSES.contains(&row.caller_class.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!(
                    "row '{}': unknown caller_class '{}'",
                    row.row_id, row.caller_class
                ),
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

        // Action grammar and wildcard checks.
        for action in &row.actions {
            // Global '*' is always invalid.
            if action == "*" {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EInvalidActionGrammar,
                    context: format!(
                        "row '{}': global wildcard '*' is not a valid action",
                        row.row_id
                    ),
                });
                continue;
            }
            // No whitespace allowed in any action string.
            if action.chars().any(|c| c.is_whitespace()) {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EInvalidActionGrammar,
                    context: format!(
                        "row '{}': action '{}' contains whitespace; use exact action ids (e.g. runs.get) or namespace wildcards (e.g. runs.*)",
                        row.row_id, action
                    ),
                });
                continue;
            }
            // Wildcard suffix '.*' requires allow.wildcard=true.
            if is_wildcard_action(action) {
                if !row.allow.wildcard {
                    errors.push(FixtureValidationError {
                        code: FixtureErrorCode::EWildcardNotAllowed,
                        context: format!(
                            "row '{}': action '{}' uses wildcard but allow.wildcard is false",
                            row.row_id, action
                        ),
                    });
                }
                // The namespace prefix (before '.*') must be non-empty and contain only
                // lowercase alphanumerics and underscores.
                let prefix = &action[..action.len() - 2]; // strip '.*'
                if prefix.is_empty()
                    || !prefix
                        .chars()
                        .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_')
                {
                    errors.push(FixtureValidationError {
                        code: FixtureErrorCode::EInvalidActionGrammar,
                        context: format!(
                            "row '{}': wildcard action '{}' has an invalid namespace prefix; expected [a-z0-9_]+",
                            row.row_id, action
                        ),
                    });
                }
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
                context: format!(
                    "row '{}': unknown rollout_mode '{}'",
                    row.row_id, row.rollout_mode
                ),
            });
        }

        // deny.reason_code enum.
        if !VALID_DENIAL_REASON_CODES.contains(&row.deny.reason_code.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!(
                    "row '{}': unknown deny.reason_code '{}'",
                    row.row_id, row.deny.reason_code
                ),
            });
        }

        // deny.client_visibility enum (denial_reason_code registry plus HTTP/GraphQL layer codes).
        if !VALID_CLIENT_VISIBILITY_CODES.contains(&row.deny.client_visibility.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!(
                    "row '{}': unknown deny.client_visibility '{}'",
                    row.row_id, row.deny.client_visibility
                ),
            });
        }

        // deprecated_after_phase must be in range 1..=6 when present.
        if let Some(phase) = row.deprecated_after_phase {
            if !(1..=6).contains(&phase) {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::ENullability,
                    context: format!(
                        "row '{}': deprecated_after_phase {} is out of range 1..=6",
                        row.row_id, phase
                    ),
                });
            }
        }

        // redaction.mode enum.
        if !VALID_REDACTION_MODES.contains(&row.redaction.mode.as_str()) {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::EUnknownEnum,
                context: format!(
                    "row '{}': unknown redaction.mode '{}'",
                    row.row_id, row.redaction.mode
                ),
            });
        }

        // required_tests non-empty.
        if row.required_tests.is_empty() {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::ENullability,
                context: format!("row '{}': required_tests must be non-empty", row.row_id),
            });
        }

        // allow.conditions must be empty (conditions are not implemented in P081 Phase 1-3).
        if !row.allow.conditions.is_empty() {
            errors.push(FixtureValidationError {
                code: FixtureErrorCode::ENullability,
                context: format!(
                    "row '{}': allow.conditions must be empty; conditions are not \
                     implemented in P081 Phase 1-3",
                    row.row_id
                ),
            });
        }

        // deny.side_effects: validate each typed entry.
        // - Unknown side-effect types are rejected (only audit_log_row is valid).
        // - command_journal_write and approval_settlement are explicitly forbidden on deny paths.
        const VALID_SIDE_EFFECT_TYPES: &[&str] = &["audit_log_row"];
        for (idx, effect) in row.deny.side_effects.iter().enumerate() {
            if matches!(
                effect.effect_type.as_str(),
                "command_journal_write" | "approval_settlement"
            ) {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EDenySideEffectConflict,
                    context: format!(
                        "row '{}': deny.side_effects[{}] type '{}' is forbidden \
                         on deny paths — only allow paths may write command_journal \
                         or approval_settlement side effects",
                        row.row_id, idx, effect.effect_type
                    ),
                });
            } else if !VALID_SIDE_EFFECT_TYPES.contains(&effect.effect_type.as_str()) {
                errors.push(FixtureValidationError {
                    code: FixtureErrorCode::EUnknownField,
                    context: format!(
                        "row '{}': deny.side_effects[{}] has unknown effect_type '{}'; \
                         valid types: {:?}",
                        row.row_id, idx, effect.effect_type, VALID_SIDE_EFFECT_TYPES
                    ),
                });
            }
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
pub fn parse_and_validate(
    json: &str,
) -> Result<(BoundaryMatrixFixture, FixtureValidationResult), String> {
    let fixture: BoundaryMatrixFixture =
        serde_json::from_str(json).map_err(|e| format!("fixture parse error: {e}"))?;
    let result = validate_fixture(&fixture);
    Ok((fixture, result))
}

/// Validate the embedded last-known-good fixture.
///
/// Returns `Err` if the embedded JSON cannot be parsed, or `Ok(result)` with
/// validation findings. Callers (daemon startup) must treat a parse error as a
/// fatal startup condition and enter safe mode or exit cleanly rather than
/// panicking.
pub fn validate_embedded_fixture() -> Result<FixtureValidationResult, String> {
    let (_, result) = parse_and_validate(EMBEDDED_FIXTURE_JSON)?;
    Ok(result)
}

// ── BoundaryPolicy service (P081 Phase 3) ────────────────────────────────

/// Operational mode for the boundary policy service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyMode {
    /// Shadow mode: policy evaluated but not enforced; violations logged only.
    Shadow,
    /// Enforce mode: policy enforced; denied calls are rejected.
    Enforce,
    /// Read-only safe mode: entered when deployed fixture is invalid; the
    /// embedded last-known-good fixture governs and only read-only paths proceed.
    ReadOnlySafeMode,
    /// Legacy compat mode: P081 matrix not yet active; pre-existing P072 guards remain
    /// authoritative. Used for emergency rollback via CHAINWORKS_BOUNDARY_POLICY=legacy.
    LegacyCompat,
}

impl PolicyMode {
    pub fn as_str(&self) -> &'static str {
        match self {
            PolicyMode::Shadow => "shadow",
            PolicyMode::Enforce => "enforce",
            PolicyMode::ReadOnlySafeMode => "read_only_safe_mode",
            PolicyMode::LegacyCompat => "legacy_compat",
        }
    }

    /// Parse from the CHAINWORKS_BOUNDARY_POLICY env var value.
    /// Returns None for unrecognized values so callers can default.
    pub fn from_env_value(v: &str) -> Option<Self> {
        match v {
            "shadow" => Some(PolicyMode::Shadow),
            "enforce" => Some(PolicyMode::Enforce),
            "read_only_safe_mode" => Some(PolicyMode::ReadOnlySafeMode),
            "legacy" | "legacy_compat" => Some(PolicyMode::LegacyCompat),
            _ => None,
        }
    }
}

impl std::fmt::Display for PolicyMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// The outcome of a boundary policy evaluation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PolicyDecision {
    /// Transport and caller_class are allowed. `row_id` is the matched matrix row;
    /// callers must inspect `row_id` to apply field-level restrictions (e.g. observer
    /// opt-in redaction) rather than treating all Allow decisions as equivalent.
    Allow { row_id: Option<String> },
    Deny {
        reason_code: String,
        client_visibility: String,
        row_id: Option<String>,
    },
    /// Shadow mode: policy matched but decision is advisory only. Callers in shadow mode
    /// should log the disagreement and fall through to legacy guards rather than deny.
    Shadow {
        matched_decision: Box<PolicyDecision>,
    },
    /// Legacy compat mode: matrix not active; calls pass through to legacy guards.
    LegacyPassthrough,
}

/// P081 Phase 3: Immutable validated boundary policy service.
///
/// One instance is constructed at daemon startup by calling
/// `BoundaryPolicy::from_fixture` or `BoundaryPolicy::from_embedded`. It is
/// then shared (via `Arc`) across GraphQL, MCP, and approval actionability
/// paths. Request paths never reload or re-parse the fixture; only daemon
/// restart rebuilds the service.
#[derive(Debug)]
pub struct BoundaryPolicy {
    fixture: BoundaryMatrixFixture,
    mode: PolicyMode,
    fixture_digest: String,
}

impl BoundaryPolicy {
    /// Construct from the embedded last-known-good fixture in enforce mode.
    /// Returns `Err` only if the embedded fixture itself fails to parse, which
    /// is a compile-time defect and should be treated as a fatal startup error.
    pub fn from_embedded() -> Result<Self, String> {
        let (fixture, result) = parse_and_validate(EMBEDDED_FIXTURE_JSON)?;
        if !result.is_valid() {
            return Err(format!(
                "embedded fixture has validation errors: {}",
                result
                    .errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        let digest = fixture_digest(EMBEDDED_FIXTURE_JSON);
        Ok(Self {
            fixture,
            mode: PolicyMode::Enforce,
            fixture_digest: digest,
        })
    }

    /// Construct from the embedded fixture with an explicit mode override.
    ///
    /// Used by daemon startup when CHAINWORKS_BOUNDARY_POLICY overrides the default
    /// enforce mode (e.g. `shadow` for shadow-only rollout, `legacy_compat` for rollback).
    pub fn from_embedded_with_mode(mode: PolicyMode) -> Result<Self, String> {
        let (fixture, result) = parse_and_validate(EMBEDDED_FIXTURE_JSON)?;
        if !result.is_valid() {
            return Err(format!(
                "embedded fixture has validation errors: {}",
                result
                    .errors
                    .iter()
                    .map(|e| e.to_string())
                    .collect::<Vec<_>>()
                    .join("; ")
            ));
        }
        let digest = fixture_digest(EMBEDDED_FIXTURE_JSON);
        Ok(Self {
            fixture,
            mode,
            fixture_digest: digest,
        })
    }

    /// Construct from a deployed fixture JSON string.
    ///
    /// If the deployed fixture is invalid, falls back to the embedded fixture
    /// and enters `read_only_safe_mode`. The returned policy is always valid.
    pub fn from_deployed_or_safe_mode(deployed_json: &str) -> Self {
        match parse_and_validate(deployed_json) {
            Ok((fixture, result)) if result.is_valid() => {
                let digest = fixture_digest(deployed_json);
                Self {
                    fixture,
                    mode: PolicyMode::Enforce,
                    fixture_digest: digest,
                }
            }
            Ok((_, result)) => {
                // Deployed fixture has validation errors — fall back to embedded.
                tracing::warn!(
                    error_count = result.errors.len(),
                    "Deployed boundary fixture invalid; entering read_only_safe_mode with embedded fixture"
                );
                Self::embedded_safe_mode()
            }
            Err(e) => {
                tracing::warn!(
                    err = %e,
                    "Deployed boundary fixture failed to parse; entering read_only_safe_mode with embedded fixture"
                );
                Self::embedded_safe_mode()
            }
        }
    }

    /// Construct from a deployed fixture JSON string with an explicit mode override.
    ///
    /// If the deployed fixture is invalid, falls back to embedded + read_only_safe_mode
    /// regardless of the override. If valid, applies the override mode.
    pub fn from_deployed_or_safe_mode_with_override(
        deployed_json: &str,
        mode_override: PolicyMode,
    ) -> Self {
        match parse_and_validate(deployed_json) {
            Ok((fixture, result)) if result.is_valid() => {
                let digest = fixture_digest(deployed_json);
                Self {
                    fixture,
                    mode: mode_override,
                    fixture_digest: digest,
                }
            }
            Ok((_, result)) => {
                tracing::warn!(
                    error_count = result.errors.len(),
                    "Deployed boundary fixture invalid; entering read_only_safe_mode with embedded fixture"
                );
                Self::embedded_safe_mode()
            }
            Err(e) => {
                tracing::warn!(
                    err = %e,
                    "Deployed boundary fixture failed to parse; entering read_only_safe_mode with embedded fixture"
                );
                Self::embedded_safe_mode()
            }
        }
    }

    fn embedded_safe_mode() -> Self {
        let (fixture, _) = parse_and_validate(EMBEDDED_FIXTURE_JSON)
            .expect("embedded fixture must always be parseable");
        let digest = fixture_digest(EMBEDDED_FIXTURE_JSON);
        Self {
            fixture,
            mode: PolicyMode::ReadOnlySafeMode,
            fixture_digest: digest,
        }
    }

    /// Current policy mode.
    pub fn mode(&self) -> &PolicyMode {
        &self.mode
    }

    /// SHA-256 digest of the fixture JSON used to construct this policy.
    pub fn fixture_digest(&self) -> &str {
        &self.fixture_digest
    }

    /// Evaluate whether a caller with the given class may perform `action` via `transport`.
    ///
    /// `action` is the specific operation being attempted (e.g. `"approveApproval"`,
    /// `"tools/list"`, `"initialize"`, a tool name). Pass `None` when checking only at the
    /// transport level (e.g. general subscription guard).
    ///
    /// - `LegacyCompat`: always returns `LegacyPassthrough`; legacy guards stay authoritative.
    /// - `Shadow`: evaluates the matrix but wraps the decision in `Shadow{..}` so callers
    ///   log the result without enforcing it.
    /// - `ReadOnlySafeMode`: state-changing transports are denied for all callers.
    /// - `Enforce`: matrix decision is authoritative.
    pub fn evaluate(
        &self,
        caller_class: &str,
        transport: &str,
        action: Option<&str>,
    ) -> PolicyDecision {
        if self.mode == PolicyMode::LegacyCompat {
            return PolicyDecision::LegacyPassthrough;
        }

        if self.mode == PolicyMode::ReadOnlySafeMode {
            // P081 startup_safety: deny all state-changing transports.
            // GraphQL mutations and MCP tool calls are denied; bounded diagnostic
            // MCP reads remain available so operators can see why safe mode is active.
            if transport == "mcp_tools_call"
                && matches!(
                    action,
                    Some("runtime.health" | "boundary.runtime.get" | "operator.alerts.list")
                )
            {
                return PolicyDecision::Allow {
                    row_id: Some("p081.safe_mode.mcp_diagnostic_read".into()),
                };
            }
            if transport == "graphql_mutation" || transport == "mcp_tools_call" {
                return PolicyDecision::Deny {
                    reason_code: "E_FIXTURE_DIGEST_MISMATCH".into(),
                    client_visibility: "FORBIDDEN".into(),
                    row_id: None,
                };
            }
        }

        let row = self.fixture.rows.iter().find(|r| {
            r.caller_class == caller_class && r.transports.contains(&transport.to_string())
        });

        let decision = match row {
            Some(r) if r.allow.enabled => {
                if action.is_none()
                    && r.caller_class == "observer"
                    && r.transports.iter().any(|t| t == "graphql_query")
                    && r.actions.iter().any(|a| a == "graphql.read_only")
                {
                    return if self.mode == PolicyMode::Shadow {
                        PolicyDecision::Shadow {
                            matched_decision: Box::new(PolicyDecision::Deny {
                                reason_code: "OBSERVER_SCOPE".into(),
                                client_visibility: r.deny.client_visibility.clone(),
                                row_id: Some(r.row_id.clone()),
                            }),
                        }
                    } else {
                        PolicyDecision::Deny {
                            reason_code: "OBSERVER_SCOPE".into(),
                            client_visibility: r.deny.client_visibility.clone(),
                            row_id: Some(r.row_id.clone()),
                        }
                    };
                }
                // Action-level check: when an action is provided, verify it against
                // the row's actions list using exact match or namespace wildcard matching.
                if let Some(act) = action {
                    let action_allowed = r.actions.iter().any(|a| {
                        if a == act {
                            return true;
                        }
                        // Namespace wildcard: pattern `ns.*` matches any action starting with `ns.`
                        if r.allow.wildcard {
                            if let Some(prefix) = a.strip_suffix(".*") {
                                let ns_dot = format!("{}.", prefix);
                                return act.starts_with(ns_dot.as_str());
                            }
                        }
                        false
                    });
                    if !action_allowed {
                        PolicyDecision::Deny {
                            reason_code: "CAPABILITY_OUT_OF_SCOPE".into(),
                            client_visibility: r.deny.client_visibility.clone(),
                            row_id: Some(r.row_id.clone()),
                        }
                    } else {
                        PolicyDecision::Allow {
                            row_id: Some(r.row_id.clone()),
                        }
                    }
                } else {
                    PolicyDecision::Allow {
                        row_id: Some(r.row_id.clone()),
                    }
                }
            }
            Some(r) => PolicyDecision::Deny {
                reason_code: r.deny.reason_code.clone(),
                client_visibility: r.deny.client_visibility.clone(),
                row_id: Some(r.row_id.clone()),
            },
            None => PolicyDecision::Deny {
                reason_code: "MATRIX_NO_ROW".into(),
                client_visibility: "FORBIDDEN".into(),
                row_id: None,
            },
        };

        if self.mode == PolicyMode::Shadow {
            PolicyDecision::Shadow {
                matched_decision: Box::new(decision),
            }
        } else {
            decision
        }
    }

    /// Check whether a specific row (by row_id) is present and allowed.
    pub fn row_allowed(&self, row_id: &str) -> bool {
        self.fixture
            .rows
            .iter()
            .find(|r| r.row_id == row_id)
            .map(|r| r.allow.enabled)
            .unwrap_or(false)
    }
}

/// Compute a short hex digest of a fixture JSON string for identity tracking.
fn fixture_digest(json: &str) -> String {
    use sha2::{Digest, Sha256};
    let hash = Sha256::digest(json.as_bytes());
    format!("{:x}", hash)[..16].to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_fixture_is_valid() {
        let result = validate_embedded_fixture().expect("embedded fixture must be valid JSON");
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

    // ── SEC-001 regression: unknown fields must be rejected at every nested level ──

    #[test]
    fn rejects_unknown_top_level_field() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [],
            "extra_top_level_field": "bad"
        }"#;
        let result = parse_and_validate(json);
        assert!(
            result.is_err(),
            "expected parse error for unknown top-level field"
        );
    }

    #[test]
    fn rejects_unknown_field_in_allow_spec() {
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
                "allow": {"enabled": true, "wildcard": false, "conditions": [], "bypass": true},
                "deny": {"reason_code": "UNAUTHENTICATED", "side_effects": [], "client_visibility": "UNAUTHENTICATED"},
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {},
                "required_tests": ["query_allow"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let result = parse_and_validate(json);
        assert!(
            result.is_err(),
            "expected parse error for unknown field in allow spec (e.g. allow.bypass)"
        );
    }

    #[test]
    fn rejects_unknown_field_in_redaction_spec() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.observer.graphql_query.read_only_opt_in",
                "caller_class": "observer",
                "transports": ["graphql_query"],
                "actions": ["runs.get"],
                "allow": {"enabled": false, "wildcard": false, "conditions": []},
                "deny": {"reason_code": "OBSERVER_SCOPE", "side_effects": [], "client_visibility": "OBSERVER_SCOPE"},
                "redaction": {"mode": "field_null_redacted", "paths": ["$.secret"], "extensions_required": true, "extra": "bad"},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {},
                "required_tests": ["observer_graphql_default_denied"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let result = parse_and_validate(json);
        assert!(
            result.is_err(),
            "expected parse error for unknown field in redaction spec"
        );
    }

    #[test]
    fn rejects_unknown_field_in_row() {
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
                "deprecated_after_phase": null,
                "unknown_row_field": "bad"
            }]
        }"#;
        let result = parse_and_validate(json);
        assert!(
            result.is_err(),
            "expected parse error for unknown field in row"
        );
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
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == FixtureErrorCode::ESchemaVersion));
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
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == FixtureErrorCode::EDuplicateRowId));
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
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == FixtureErrorCode::EUnknownEnum));
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
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == FixtureErrorCode::EWildcardNotAllowed));
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
        assert!(result
            .errors
            .iter()
            .any(|e| e.code == FixtureErrorCode::EInvalidRowId));
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
        assert!(
            missing_count >= 10,
            "expected at least 10 missing-row errors, got {missing_count}"
        );
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
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::ERequiredRowTransportMismatch),
            "expected E_REQUIRED_ROW_TRANSPORT_MISMATCH"
        );
    }

    #[test]
    fn rejects_deny_side_effects_with_command_journal_write() {
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
                "allow": {"enabled": false, "wildcard": false, "conditions": []},
                "deny": {
                    "reason_code": "UNAUTHENTICATED",
                    "side_effects": [{"type": "command_journal_write"}],
                    "client_visibility": "UNAUTHENTICATED"
                },
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {"scope": "test"},
                "required_tests": ["query_allow"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::EDenySideEffectConflict),
            "expected E_DENY_SIDE_EFFECT_CONFLICT for command_journal_write on deny path"
        );
    }

    #[test]
    fn rejects_deny_side_effects_with_approval_settlement() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.ui_operator.graphql_mutation.approval_action",
                "caller_class": "ui_operator",
                "transports": ["graphql_mutation"],
                "actions": ["approveApproval"],
                "allow": {"enabled": false, "wildcard": false, "conditions": []},
                "deny": {
                    "reason_code": "NON_APPROVAL_MUTATION",
                    "side_effects": [{"type": "approval_settlement"}],
                    "client_visibility": "NON_APPROVAL_MUTATION"
                },
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "approval_record",
                "read_model_delta": {"scope": "test"},
                "required_tests": ["denied_non_approval"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::EDenySideEffectConflict),
            "expected E_DENY_SIDE_EFFECT_CONFLICT for approval_settlement on deny path"
        );
    }

    #[test]
    fn allows_deny_side_effects_with_audit_log_row() {
        // audit_log rows are the only permitted deny side effect (durable deny audit).
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.developer_break_glass.debug_endpoint.disabled",
                "caller_class": "developer_break_glass",
                "transports": ["debug_endpoint"],
                "actions": ["debug.preflight"],
                "allow": {"enabled": false, "wildcard": false, "conditions": []},
                "deny": {
                    "reason_code": "BREAK_GLASS_DISABLED",
                    "side_effects": [{"type": "audit_log_row", "event_type": "developer_break_glass_disabled"}],
                    "client_visibility": "BREAK_GLASS_DISABLED"
                },
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "audit_log",
                "read_model_delta": {"scope": "test"},
                "required_tests": ["env_gate_test", "audit_event_assertion", "no_projection_write_test", "audit_unavailable_fail_closed"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (fixture, result) = parse_and_validate(json).unwrap();
        let side_effect_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| e.code == FixtureErrorCode::EDenySideEffectConflict)
            .collect();
        assert!(
            side_effect_errors.is_empty(),
            "audit_log_row side effect should be allowed on deny path, got: {:?}",
            side_effect_errors
        );
        // The row itself is valid (it IS a required row so missing others fire, but not side_effects).
        let _ = fixture;
    }

    #[test]
    fn rejects_deny_side_effects_with_unknown_effect_type() {
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
                "allow": {"enabled": false, "wildcard": false, "conditions": []},
                "deny": {
                    "reason_code": "UNAUTHENTICATED",
                    "side_effects": [{"type": "unknown_future_effect"}],
                    "client_visibility": "UNAUTHENTICATED"
                },
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "projection_read_model",
                "read_model_delta": {"scope": "test"},
                "required_tests": ["query_allow"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::EUnknownField
                    && e.context.contains("unknown_future_effect")),
            "expected E_UNKNOWN_FIELD for unknown deny side-effect type"
        );
    }

    #[test]
    fn rejects_deny_side_effect_with_unknown_nested_field() {
        // deny.side_effects objects must not have unknown fields (deny_unknown_fields)
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.developer_break_glass.debug_endpoint.disabled",
                "caller_class": "developer_break_glass",
                "transports": ["debug_endpoint"],
                "actions": ["debug.preflight"],
                "allow": {"enabled": false, "wildcard": false, "conditions": []},
                "deny": {
                    "reason_code": "BREAK_GLASS_DISABLED",
                    "side_effects": [{"type": "audit_log_row", "event_type": "x", "unknown_extra": "bad"}],
                    "client_visibility": "BREAK_GLASS_DISABLED"
                },
                "redaction": {"mode": "none", "paths": [], "extensions_required": false},
                "authoritative_record": "audit_log",
                "read_model_delta": {"scope": "test"},
                "required_tests": ["env_gate_test"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let result = parse_and_validate(json);
        assert!(
            result.is_err(),
            "expected parse error for unknown field in DenySideEffect"
        );
    }

    #[test]
    fn rejects_read_model_delta_with_unknown_field() {
        // read_model_delta must not have unknown fields (deny_unknown_fields on ReadModelDelta)
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
                "read_model_delta": {"scope": "test", "deny_behavior": "ok", "unknown_key": "bad"},
                "required_tests": ["query_allow"],
                "rollout_mode": "shadow",
                "deprecated_after_phase": null
            }]
        }"#;
        let result = parse_and_validate(json);
        assert!(
            result.is_err(),
            "expected parse error for unknown field in read_model_delta"
        );
    }

    #[test]
    fn rejects_unknown_deny_reason_code() {
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
                "deny": {"reason_code": "INVALID_REASON_CODE", "side_effects": [], "client_visibility": "UNAUTHENTICATED"},
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
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::EUnknownEnum
                    && e.context.contains("deny.reason_code")),
            "expected E_UNKNOWN_ENUM for unknown deny.reason_code"
        );
    }

    #[test]
    fn rejects_unknown_client_visibility() {
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
                "deny": {"reason_code": "UNAUTHENTICATED", "side_effects": [], "client_visibility": "NOT_A_REAL_CODE"},
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
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::EUnknownEnum
                    && e.context.contains("deny.client_visibility")),
            "expected E_UNKNOWN_ENUM for unknown deny.client_visibility"
        );
    }

    #[test]
    fn rejects_unknown_enum_casing() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "camelCase",
            "rows": []
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::EUnknownEnum
                    && e.context.contains("enum_casing")),
            "expected E_UNKNOWN_ENUM for unknown enum_casing value"
        );
    }

    #[test]
    fn rejects_deprecated_after_phase_out_of_range() {
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
                "deprecated_after_phase": 99
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        assert!(
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::ENullability
                    && e.context.contains("deprecated_after_phase")),
            "expected E_NULLABILITY for deprecated_after_phase out of range 1..=6"
        );
    }

    #[test]
    fn accepts_deprecated_after_phase_in_range() {
        // Phase 6 is the max valid value; phase 1 is min.
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
                "deprecated_after_phase": 6
            }]
        }"#;
        let (_, result) = parse_and_validate(json).unwrap();
        // No ENullability errors about deprecated_after_phase.
        let phase_errors: Vec<_> = result
            .errors
            .iter()
            .filter(|e| {
                e.code == FixtureErrorCode::ENullability
                    && e.context.contains("deprecated_after_phase")
            })
            .collect();
        assert!(
            phase_errors.is_empty(),
            "deprecated_after_phase=6 should be valid, got: {:?}",
            phase_errors
        );
    }

    // ── PolicyMode and evaluate behavior ─────────────────────────────────

    #[test]
    fn policy_mode_from_env_value_parses_known_values() {
        assert_eq!(
            PolicyMode::from_env_value("shadow"),
            Some(PolicyMode::Shadow)
        );
        assert_eq!(
            PolicyMode::from_env_value("enforce"),
            Some(PolicyMode::Enforce)
        );
        assert_eq!(
            PolicyMode::from_env_value("read_only_safe_mode"),
            Some(PolicyMode::ReadOnlySafeMode)
        );
        assert_eq!(
            PolicyMode::from_env_value("legacy"),
            Some(PolicyMode::LegacyCompat)
        );
        assert_eq!(
            PolicyMode::from_env_value("legacy_compat"),
            Some(PolicyMode::LegacyCompat)
        );
        assert_eq!(PolicyMode::from_env_value("unknown"), None);
        assert_eq!(PolicyMode::from_env_value(""), None);
    }

    #[test]
    fn policy_mode_as_str_covers_all_variants() {
        assert_eq!(PolicyMode::Shadow.as_str(), "shadow");
        assert_eq!(PolicyMode::Enforce.as_str(), "enforce");
        assert_eq!(PolicyMode::ReadOnlySafeMode.as_str(), "read_only_safe_mode");
        assert_eq!(PolicyMode::LegacyCompat.as_str(), "legacy_compat");
    }

    #[test]
    fn legacy_compat_mode_returns_passthrough_for_all_callers() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::LegacyCompat).unwrap();
        for caller in &[
            "ui_operator",
            "agent_operator",
            "automation",
            "observer",
            "developer_break_glass",
        ] {
            for transport in &[
                "graphql_query",
                "graphql_mutation",
                "mcp_tools_call",
                "mcp_initialize",
            ] {
                assert_eq!(
                    policy.evaluate(caller, transport, None),
                    PolicyDecision::LegacyPassthrough,
                    "expected LegacyPassthrough for {caller}/{transport}"
                );
            }
        }
    }

    #[test]
    fn shadow_mode_wraps_allow_decision() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Shadow).unwrap();
        // ui_operator.graphql_query.read is allow=true in embedded fixture
        let decision = policy.evaluate("ui_operator", "graphql_query", None);
        assert!(
            matches!(
                decision,
                PolicyDecision::Shadow {
                    matched_decision: _
                }
            ),
            "expected Shadow wrapper, got {:?}",
            decision
        );
        if let PolicyDecision::Shadow { matched_decision } = decision {
            assert!(
                matches!(*matched_decision, PolicyDecision::Allow { .. }),
                "expected Allow inside Shadow, got {:?}",
                matched_decision
            );
        }
    }

    #[test]
    fn shadow_mode_wraps_deny_decision() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Shadow).unwrap();
        // observer.graphql_mutation — no row in embedded fixture → MATRIX_NO_ROW
        let decision = policy.evaluate("observer", "graphql_mutation", None);
        assert!(
            matches!(
                decision,
                PolicyDecision::Shadow {
                    matched_decision: _
                }
            ),
            "expected Shadow wrapper for deny, got {:?}",
            decision
        );
        if let PolicyDecision::Shadow { matched_decision } = decision {
            assert!(
                matches!(*matched_decision, PolicyDecision::Deny { .. }),
                "expected Deny inside Shadow"
            );
        }
    }

    #[test]
    fn from_embedded_with_mode_respects_mode() {
        let shadow = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Shadow).unwrap();
        assert_eq!(shadow.mode(), &PolicyMode::Shadow);

        let legacy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::LegacyCompat).unwrap();
        assert_eq!(legacy.mode(), &PolicyMode::LegacyCompat);
    }

    #[test]
    fn safe_mode_denies_mutations_regardless_of_caller() {
        let policy = BoundaryPolicy::from_deployed_or_safe_mode("invalid json");
        assert_eq!(policy.mode(), &PolicyMode::ReadOnlySafeMode);
        // Mutations denied in safe mode
        let decision = policy.evaluate("ui_operator", "graphql_mutation", None);
        assert!(
            matches!(decision, PolicyDecision::Deny { .. }),
            "safe mode should deny mutations"
        );
        // Reads pass through to matrix
        let read_decision = policy.evaluate("ui_operator", "graphql_query", None);
        // ui_operator is allowed on graphql_query in the embedded fixture
        assert!(
            matches!(read_decision, PolicyDecision::Allow { .. }),
            "expected Allow for ui_operator graphql_query, got {:?}",
            read_decision
        );
    }

    // ── Action grammar: whitespace rejection ─────────────────────────────

    #[test]
    fn rejects_action_with_whitespace() {
        let json = r#"{
            "schema_version": 1,
            "matrix_id": "test",
            "generated_from": "test",
            "enum_casing": "snake_case",
            "rows": [{
                "row_id": "p081.ui_operator.graphql_query.read",
                "caller_class": "ui_operator",
                "transports": ["graphql_query"],
                "actions": ["operator-class MCP commands scoped by surface_policies.mcp"],
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
            result
                .errors
                .iter()
                .any(|e| e.code == FixtureErrorCode::EInvalidActionGrammar),
            "expected E_INVALID_ACTION_GRAMMAR for action containing whitespace"
        );
    }

    // ── BoundaryPolicy: namespace wildcard matching ──────────────────────

    #[test]
    fn enforce_mode_wildcard_matches_namespace_prefixed_tools() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Enforce).unwrap();
        // agent_operator mcp_tools_call row has runs.* wildcard → runs.list should be allowed
        assert!(
            matches!(
                policy.evaluate("agent_operator", "mcp_tools_call", Some("runs.list")),
                PolicyDecision::Allow { .. }
            ),
            "runs.list should match runs.* wildcard for agent_operator"
        );
        assert!(
            matches!(
                policy.evaluate("agent_operator", "mcp_tools_call", Some("approvals.list")),
                PolicyDecision::Allow { .. }
            ),
            "approvals.list should match approvals.* wildcard for agent_operator"
        );
        assert!(
            matches!(
                policy.evaluate(
                    "agent_operator",
                    "mcp_tools_call",
                    Some("effects.reconcile")
                ),
                PolicyDecision::Allow { .. }
            ),
            "effects.reconcile should match effects.* wildcard for agent_operator"
        );
    }

    #[test]
    fn enforce_mode_wildcard_denies_unmatched_namespace() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Enforce).unwrap();
        // The mcp_tools_call row for agent_operator only has actual MCP namespaces.
        // An action with no matching namespace should be denied CAPABILITY_OUT_OF_SCOPE.
        let decision =
            policy.evaluate("agent_operator", "mcp_tools_call", Some("graphql.mutation"));
        assert!(
            matches!(decision, PolicyDecision::Deny { ref reason_code, .. } if reason_code == "CAPABILITY_OUT_OF_SCOPE"),
            "graphql.mutation should not match any namespace wildcard for agent_operator, got {:?}",
            decision
        );
    }

    // ── H-001 regression: observer must be denied for mutating MCP actions ──

    #[test]
    fn observer_denied_for_mutating_mcp_actions() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Enforce).unwrap();
        // Observer compact_read row uses explicit read-only actions only.
        // Mutating actions (approvals.resolve, runs.cancel, effects.*, storage.*) must be denied.
        for mutating in &[
            "approvals.resolve",
            "runs.cancel",
            "effects.reconcile_evidence_orphans",
            "storage.reconcile_evidence_orphans",
            "effects.trigger",
            "runs.create",
        ] {
            let decision = policy.evaluate("observer", "mcp_tools_call", Some(mutating));
            assert!(
                matches!(decision, PolicyDecision::Deny { .. }),
                "observer must be denied for mutating action '{}', got {:?}",
                mutating,
                decision
            );
        }
    }

    #[test]
    fn observer_allowed_for_explicit_read_only_actions() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Enforce).unwrap();
        // Observer compact_read row permits only the explicit read-only action ids.
        for read_only in &[
            "runtime.health",
            "runs.list",
            "runs.get",
            "approvals.list",
            "approvals.get",
            "stages.list",
            "stages.get",
            "artifacts.list",
            "artifacts.get",
            "reports.list",
            "reports.get",
            "steward.status",
        ] {
            let decision = policy.evaluate("observer", "mcp_tools_call", Some(read_only));
            assert!(
                matches!(decision, PolicyDecision::Allow { .. }),
                "observer should be allowed for read-only action '{}', got {:?}",
                read_only,
                decision
            );
        }
    }

    #[test]
    fn observer_denied_for_graphql_mutation() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Enforce).unwrap();
        // Observer has no graphql_mutation row; any mutation must be denied MATRIX_NO_ROW.
        let decision = policy.evaluate("observer", "graphql_mutation", Some("approveApproval"));
        assert!(
            matches!(decision, PolicyDecision::Deny { ref reason_code, .. } if reason_code == "MATRIX_NO_ROW"),
            "observer graphql_mutation must return MATRIX_NO_ROW, got {:?}",
            decision
        );
    }

    #[test]
    fn observer_graphql_query_requires_explicit_read_only_opt_in() {
        let policy = BoundaryPolicy::from_embedded_with_mode(PolicyMode::Enforce).unwrap();
        let default_decision = policy.evaluate("observer", "graphql_query", None);
        assert!(
            matches!(default_decision, PolicyDecision::Deny { ref reason_code, .. } if reason_code == "OBSERVER_SCOPE"),
            "observer graphql_query without explicit read-only action must fail closed, got {:?}",
            default_decision
        );
        let opt_in_decision =
            policy.evaluate("observer", "graphql_query", Some("graphql.read_only"));
        assert!(
            matches!(opt_in_decision, PolicyDecision::Allow { ref row_id } if row_id.as_deref() == Some("p081.observer.graphql_query.read_only_opt_in")),
            "observer graphql_query with explicit read-only action must use redacted opt-in row, got {:?}",
            opt_in_decision
        );
    }

    #[test]
    fn safe_mode_denies_state_changing_mcp_tools_call_but_allows_diagnostics() {
        // H-001 fix: safe mode must deny ALL mcp_tools_call to prevent state-changing
        // MCP commands from executing when the deployed fixture is invalid or tampered.
        // The embedded fixture allows agent_operator mcp_tools_call broadly, so without
        // this unconditional denial any principal with mcp capabilities could still
        // dispatch state-changing tools while in safe mode.
        // P081 startup_safety: "Deny all GraphQL mutations, MCP tool calls, and
        // approval actionability mutations."
        let policy = BoundaryPolicy::from_deployed_or_safe_mode("invalid json");
        assert_eq!(policy.mode(), &PolicyMode::ReadOnlySafeMode);

        // graphql_mutation is unconditionally denied in safe mode
        let mutation_decision = policy.evaluate("ui_operator", "graphql_mutation", None);
        assert!(
            matches!(mutation_decision, PolicyDecision::Deny { .. }),
            "safe mode must deny graphql_mutation, got {:?}",
            mutation_decision
        );

        // mcp_tools_call is denied in safe mode except bounded diagnostic reads.
        let mcp_decision = policy.evaluate("observer", "mcp_tools_call", Some("runs.list"));
        assert!(
            matches!(mcp_decision, PolicyDecision::Deny { ref reason_code, .. } if reason_code == "E_FIXTURE_DIGEST_MISMATCH"),
            "safe mode must deny mcp_tools_call (H-001), got {:?}",
            mcp_decision
        );
        let alert_decision = policy.evaluate(
            "agent_operator",
            "mcp_tools_call",
            Some("operator.alerts.list"),
        );
        assert!(
            matches!(alert_decision, PolicyDecision::Allow { .. }),
            "safe mode must keep operator.alerts.list visible, got {:?}",
            alert_decision
        );

        // graphql_query is still allowed (served by embedded fixture)
        let query_decision = policy.evaluate("ui_operator", "graphql_query", None);
        assert!(
            !matches!(query_decision, PolicyDecision::Deny { ref reason_code, .. } if reason_code == "E_FIXTURE_DIGEST_MISMATCH"),
            "safe mode must not unconditionally deny graphql_query, got {:?}",
            query_decision
        );
    }
}
