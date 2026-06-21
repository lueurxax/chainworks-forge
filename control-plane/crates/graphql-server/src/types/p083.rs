//! P083 GraphQL types: RollbackDispositionJSON scalar with output validation.
//!
//! rollback_disposition_json_v1 contract: resolvers validate every
//! rollback_disposition output value against rollback_disposition_v1 before
//! serialization. The schema_version wrapper is added by the readback builder;
//! the inline rollout_contract_v1 rollback_disposition object does not carry it.

use async_graphql::{
    Enum, InputValueError, InputValueResult, Json, Scalar, ScalarType, SimpleObject, Union, Value,
};
use serde::{Deserialize, Serialize};

/// P083 CallerRequestId input scalar.
///
/// The public GraphQL SDL must expose this as a distinct scalar rather than a
/// plain String so lifecycle callers cannot accidentally treat request ids as
/// free-form diagnostic text.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallerRequestId(String);

impl CallerRequestId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for CallerRequestId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn validate_caller_request_id_scalar(raw: &str) -> Result<(), String> {
    if raw.trim() != raw {
        return Err("CallerRequestId must not include leading or trailing whitespace".into());
    }
    if raw.to_ascii_lowercase() != raw {
        return Err("CallerRequestId must be lowercase".into());
    }
    let parsed = uuid::Uuid::parse_str(raw)
        .map_err(|_| "CallerRequestId must be a dashed UUID".to_string())?;
    if parsed.get_version_num() != 4 {
        return Err("CallerRequestId must be a UUIDv4".into());
    }
    if parsed.hyphenated().to_string() != raw {
        return Err("CallerRequestId must use canonical dashed UUID form".into());
    }
    Ok(())
}

#[Scalar(name = "CallerRequestId")]
impl ScalarType for CallerRequestId {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(raw) => {
                validate_caller_request_id_scalar(&raw).map_err(InputValueError::custom)?;
                Ok(Self(raw))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.clone())
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "DenialReason", rename_items = "snake_case")]
pub enum DenialReason {
    MissingCallerRequestId,
    MalformedRequestId,
    PrincipalClassNotAllowed,
    LifecycleStateInvalid,
    SchemaInvalid,
    AdditionalPropertiesRejected,
    RollbackTargetRequired,
    RollbackTargetInvalid,
    RequestIntentMismatch,
    IdempotencyInFlight,
    IdempotencyReplayed,
    IdempotencyExpiredReacquired,
    OperatorRequired,
    P083OperatorRequired,
    ProviderSessionNotFound,
    RunNotFound,
    StageNotRetryable,
    ApprovalNotActionable,
    SideEffectNotReconcilable,
    EnforcementModeTransitionDenied,
    IdentityAmbiguous,
    IdempotencyReplayCorrupt,
    IdempotencyTerminalFailure,
    Internal,
}

#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "DenialPayload")]
pub struct DenialPayload {
    pub reason: DenialReason,
    pub message: String,
    pub retry_after_seconds: Option<i32>,
}

/// Required fields in a rollback_disposition_v1 value.
/// Per rollback_disposition_json_v1.required_fields (schema_version, mode, data_loss_risk, steps).
const REQUIRED_FIELDS: &[&str] = &["schema_version", "mode", "data_loss_risk", "steps"];
/// Allowed fields per additionalProperties=false in rollback_disposition_v1.
const ALLOWED_FIELDS: &[&str] = &["schema_version", "mode", "data_loss_risk", "steps"];
const ALLOWED_DATA_LOSS_RISK: &[&str] = &["none", "low", "medium", "high"];

/// Validation error codes surfaced in the GraphQL error extensions when a
/// rollback_disposition value fails output validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RollbackDispositionValidationError {
    MissingField(&'static str),
    InvalidSchemaVersion,
    InvalidModeType,
    InvalidStepsType,
    /// data_loss_risk is present but is not a JSON string (e.g., a number, null, or object).
    InvalidDataLossRiskType,
    InvalidDataLossRisk(String),
    NotAnObject,
    /// Per additionalProperties=false: extra fields beyond the four schema-mandated ones.
    ExtraProperty(String),
}

impl std::fmt::Display for RollbackDispositionValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingField(field) => write!(f, "rollback_disposition_v1 missing required field: {field}"),
            Self::InvalidSchemaVersion => write!(f, "rollback_disposition_v1 schema_version must be \"rollback_disposition_v1\""),
            Self::InvalidModeType => write!(f, "rollback_disposition_v1 mode must be a string"),
            Self::InvalidStepsType => write!(f, "rollback_disposition_v1 steps must be an array"),
            Self::InvalidDataLossRiskType => write!(f, "rollback_disposition_v1 data_loss_risk must be a string (none, low, medium, or high)"),
            Self::InvalidDataLossRisk(v) => write!(f, "rollback_disposition_v1 invalid data_loss_risk: {v:?}"),
            Self::NotAnObject => write!(f, "rollback_disposition_v1 value must be a JSON object"),
            Self::ExtraProperty(k) => write!(f, "rollback_disposition_v1 extra property not allowed (additionalProperties=false): {k}"),
        }
    }
}

/// Validate a JSON value against rollback_disposition_v1.
///
/// Per output_validation_rule: called before serialization at the resolver level.
/// Asserts schema_version, type-checks mode and steps, and rejects invalid data_loss_risk.
pub fn validate_rollback_disposition_v1(
    value: &serde_json::Value,
) -> Result<(), RollbackDispositionValidationError> {
    let obj = value
        .as_object()
        .ok_or(RollbackDispositionValidationError::NotAnObject)?;

    for field in REQUIRED_FIELDS {
        if !obj.contains_key(*field) {
            return Err(RollbackDispositionValidationError::MissingField(field));
        }
    }

    // schema_version must equal the canonical value.
    if obj.get("schema_version").and_then(|v| v.as_str()) != Some("rollback_disposition_v1") {
        return Err(RollbackDispositionValidationError::InvalidSchemaVersion);
    }

    // mode must be a string.
    if !obj.get("mode").map_or(false, |v| v.is_string()) {
        return Err(RollbackDispositionValidationError::InvalidModeType);
    }

    // steps must be an array.
    if !obj.get("steps").map_or(false, |v| v.is_array()) {
        return Err(RollbackDispositionValidationError::InvalidStepsType);
    }

    // data_loss_risk is required (listed in REQUIRED_FIELDS) and must be a string.
    // Using .and_then(|v| v.as_str()) would silently ignore numeric/null/object values,
    // so we first confirm the value is a string type before checking its domain.
    let dlr_val = obj
        .get("data_loss_risk")
        .expect("checked in REQUIRED_FIELDS loop above");
    match dlr_val.as_str() {
        None => {
            return Err(RollbackDispositionValidationError::InvalidDataLossRiskType);
        }
        Some(dlr) => {
            if !ALLOWED_DATA_LOSS_RISK.contains(&dlr) {
                return Err(RollbackDispositionValidationError::InvalidDataLossRisk(
                    dlr.to_string(),
                ));
            }
        }
    }

    // Per rollback_disposition_json_v1.mcp_schema_rule: additionalProperties=false.
    // Reject any key beyond the four schema-mandated fields.
    for key in obj.keys() {
        if !ALLOWED_FIELDS.contains(&key.as_str()) {
            return Err(RollbackDispositionValidationError::ExtraProperty(
                key.clone(),
            ));
        }
    }

    Ok(())
}

/// Wrap an inline rollout_contract_v1 rollback_disposition (which lacks
/// schema_version) into a rollback_disposition_v1 readback object.
///
/// The canonical schema_version is set by this function and is never
/// overridden by an inline field. Steps are NOT synthesised when absent —
/// callers validate the result via `RollbackDispositionJSON::validated` and
/// treat validation failure as None rather than silently propagating empty
/// provenance (P083-SEC-LOW-005).
/// SEC-P083-HIGH-001: Only the four schema-mandated fields (mode, data_loss_risk,
/// steps, schema_version) are allowed in readback output. All other keys from the
/// stored inline object are silently stripped to prevent unexpected diagnostic or
/// secret-bearing fields from leaking through GraphQL/MCP/run_report/release_receipt.
/// The canonical schema_version is always set by this function and is never
/// overridden by an inline field.
pub fn wrap_rollback_disposition_for_readback(inline: &serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::json!({
        "schema_version": "rollback_disposition_v1"
    });
    if let Some(obj) = inline.as_object() {
        // Strict whitelist: only mode, data_loss_risk, steps pass through.
        // camelCase variants (from prior graphql lane round-trips) are normalized to snake_case.
        for (k, v) in obj {
            match k.as_str() {
                "schema_version" => {
                    // Already set above; never let inline override canonical value.
                }
                "mode" => {
                    out["mode"] = v.clone();
                }
                "data_loss_risk" | "dataLossRisk" => {
                    out["data_loss_risk"] = v.clone();
                }
                "steps" => {
                    out["steps"] = v.clone();
                }
                // All other keys are rejected — additionalProperties=false per rollback_disposition_v1.
                _ => {}
            }
        }
    }
    out
}

/// RollbackDispositionJSON: a GraphQL output scalar that validates every
/// rollback_disposition value against rollback_disposition_v1 before serialization.
///
/// Per rollback_disposition_json_v1.output_validation_rule: validation happens at
/// the resolver level, not in the scalar parser.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RollbackDispositionJSON(pub serde_json::Value);

impl RollbackDispositionJSON {
    /// Construct with output validation. Returns Err if value fails
    /// rollback_disposition_v1 validation.
    pub fn validated(value: serde_json::Value) -> Result<Self, RollbackDispositionValidationError> {
        validate_rollback_disposition_v1(&value)?;
        Ok(Self(value))
    }

    pub fn inner(&self) -> &serde_json::Value {
        &self.0
    }
}

#[Scalar(name = "RollbackDispositionJSON")]
impl ScalarType for RollbackDispositionJSON {
    fn parse(_value: Value) -> InputValueResult<Self> {
        // This scalar is output-only in the current P083 contract.
        // Input parsing is not required; return error if someone attempts it.
        Err(InputValueError::custom(
            "RollbackDispositionJSON is output-only in P083",
        ))
    }

    fn to_value(&self) -> Value {
        // Re-validate before serialization to catch any bypass of validated().
        // Per rollback_disposition_json_v1.output_validation_rule: invalid payloads must
        // not escape to callers; return Null and log rather than serializing invalid data.
        if validate_rollback_disposition_v1(&self.0).is_err() {
            tracing::error!(
                "RollbackDispositionJSON: invalid value reached to_value(); returning Null (bug: bypassed validated() constructor)"
            );
            return Value::Null;
        }
        async_graphql::Value::from_json(self.0.clone()).unwrap_or(Value::Null)
    }
}

/// Default rollback disposition returned when no run-specific disposition is available.
/// Per rollout_readback_api_parity_v1: rolloutContractRollbackDisposition is String! so
/// a valid non-null object is required even when no terminal check row exists.
fn default_rollback_disposition() -> RollbackDispositionJSON {
    RollbackDispositionJSON::validated(serde_json::json!({
        "schema_version": "rollback_disposition_v1",
        "mode": "not_applicable",
        "data_loss_risk": "none",
        "steps": []
    }))
    .expect("default rollback disposition is always valid")
}

/// P083 rollout contract readback type — rollout_readback_api_parity_v1 full field set.
///
/// Field names follow rollout_readback_api_parity_v1 with rollout_contract_* prefix;
/// async-graphql converts them to camelCase in the GraphQL schema
/// (rollout_contract_status → rolloutContractStatus).
///
/// Non-null fields match the approved SDL String! requirements from graphql_sdl.
/// Nullable fields (Option) map to SDL nullable types (no !).
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlP083RolloutContractReadback {
    /// Always "rollout_contract_readback_p083_v1".
    pub schema_version: String,
    /// rollout_contract_status: pass / fail / waived / not_applicable. (SDL: String!)
    pub rollout_contract_status: String,
    /// rollout_contract_decision: release / hold / waive / not_applicable. (SDL: String!)
    pub rollout_contract_decision: String,
    /// rollout_contract_failure_reasons from the terminal check row.
    pub rollout_contract_failure_reasons: Vec<String>,
    /// rollout_contract_waiver_state: active / expired / none. (SDL: String!)
    pub rollout_contract_waiver_state: String,
    /// rollout_contract_waiver_expires_at: RFC3339 expiry or null. (SDL: nullable)
    pub rollout_contract_waiver_expires_at: Option<String>,
    /// rollout_contract_enforcement_mode: disabled / permissive / enforce. (SDL: String!)
    pub rollout_contract_enforcement_mode: String,
    /// rollout_contract_enforcement_mode_reason: human-readable reason string. (SDL: String!)
    pub rollout_contract_enforcement_mode_reason: String,
    /// rollout_contract_hold_conditions: list of active hold condition codes.
    pub rollout_contract_hold_conditions: Vec<String>,
    /// rollout_contract_rollback_disposition validated against rollback_disposition_v1.
    /// Defaults to a valid not_applicable disposition when no terminal check row exists.
    /// (SDL: RollbackDispositionJSON!)
    pub rollout_contract_rollback_disposition: RollbackDispositionJSON,
    /// rollout_contract_source_lane: graphql / mcp / run_report / release_receipt. (SDL: String!)
    pub rollout_contract_source_lane: String,
    /// rollout_contract_enabled_state: enabled / disabled. (SDL: String!)
    pub rollout_contract_enabled_state: String,
    /// rollout_contract_disabled_reason_code or null. (SDL: nullable)
    pub rollout_contract_disabled_reason_code: Option<String>,
    /// rollout_contract_action_id unique identifier for this readback row. (SDL: nullable)
    pub rollout_contract_action_id: Option<String>,
    /// rollout_contract_operator_message: human-readable decision summary. (SDL: String!)
    pub rollout_contract_operator_message: String,
    /// rollout_contract_projection_integrity: fresh / stale / tampered. (SDL: String!)
    pub rollout_contract_projection_integrity: String,
    /// rollout_contract_cutover_policy_revision. (SDL: String!)
    pub rollout_contract_cutover_policy_revision: String,
    /// rollout_contract_diagnostic_redaction state. (SDL: String!)
    pub rollout_contract_diagnostic_redaction: String,
    /// rollout_contract_next_steps: list of recommended action strings.
    pub rollout_contract_next_steps: Vec<String>,
    /// rollout_contract_shutdown_deadline_config_state: configured / not_configured. (SDL: String!)
    pub rollout_contract_shutdown_deadline_config_state: String,
    /// rollout_contract_command_lease_ttl_config_state: configured / not_configured. (SDL: String!)
    pub rollout_contract_command_lease_ttl_config_state: String,
    /// p083_rollback_ttl_expires_at: RFC3339 timestamp when the rollback TTL expires, or null.
    /// Null until a rollback is executed; thereafter set to the TTL expiry.
    pub p083_rollback_ttl_expires_at: Option<String>,
    /// p083_last_preflight_hash: SHA-256 hex of the last passing preflight check, or null.
    /// Null until the first preflight pass; used to detect in-flight enforcement-mode changes.
    pub p083_last_preflight_hash: Option<String>,
    /// p083_shutdown_queue_rank: [integer, null] — latest queue_rank for queued_no_signal
    /// receipts associated with this run, or null when no such receipt exists.
    pub p083_shutdown_queue_rank: Option<i64>,
    /// applied_migrations: P083 migration readback array per migration_plan_v1.
    /// Each element exposes logical_id, filename, sha256, depends_on, applied_at,
    /// schema_version, state, and verification_query_result for every P083 migration.
    /// Per MISSING-009 / rollout_readback_api_parity_v1.
    pub applied_migrations: Option<Json<Vec<serde_json::Value>>>,
    // Legacy shorthand fields preserved for existing clients during transition.
    /// Alias for rollout_contract_rollback_disposition.
    pub rollback_disposition: Option<RollbackDispositionJSON>,
    /// Alias for rollout_contract_enforcement_mode.
    pub enforcement_mode: Option<String>,
    /// Alias for rollout_contract_decision.
    pub contract_decision: Option<String>,
    /// Alias for rollout_contract_hold_conditions.
    pub hold_conditions: Vec<String>,
    /// Alias for rollout_contract_projection_integrity.
    pub projection_integrity: Option<String>,
}

impl GqlP083RolloutContractReadback {
    pub const SCHEMA_VERSION: &'static str = "rollout_contract_readback_p083_v1";

    /// Construct from a camelCase GraphQL-lane readback JSON.
    ///
    /// Returns Err when the rollback_disposition field fails rollback_disposition_v1 validation.
    /// Per rollback_disposition_json_v1.output_validation_rule: invalid outputs MUST be rejected
    /// before GraphQL serialization, not silently replaced with a default.
    ///
    /// `queue_rank` is loaded separately by the async resolver since the DB query
    /// cannot be run inside this sync constructor.
    pub fn from_readback_json(
        readback: &serde_json::Value,
        queue_rank: Option<i64>,
    ) -> anyhow::Result<Self> {
        let str_field = |camel: &str| -> Option<String> {
            readback
                .get(camel)
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        };
        let str_arr = |camel: &str| -> Vec<String> {
            readback
                .get(camel)
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|v| v.as_str())
                        .map(|s| s.to_string())
                        .collect()
                })
                .unwrap_or_default()
        };

        // Build and validate rollback_disposition.
        // The graphql lane camelCases rollback_disposition → rollbackDisposition.
        //
        // Per rollback_disposition_json_v1.output_validation_rule: invalid outputs must be
        // REJECTED before GraphQL serialization — return Err, never fall back to a default.
        let raw_disposition = readback
            .get("rollbackDisposition")
            .or_else(|| readback.get("rollback_disposition"));
        let validated_disposition = match raw_disposition {
            None => {
                // No disposition in readback — use the not_applicable default only when
                // the readback row genuinely has no rollback_disposition field.
                default_rollback_disposition()
            }
            Some(inline) => {
                let wrapped = wrap_rollback_disposition_for_readback(inline);
                RollbackDispositionJSON::validated(wrapped).map_err(|e| {
                    db::metrics::record_p083_rollout_contract_lint(
                        "P083",
                        "fail",
                        Some("rollback_contract_invalid"),
                    );
                    anyhow::anyhow!(
                        "P083 rollback_disposition_v1 output validation failed (rejected before GraphQL serialization): {e}"
                    )
                })?
            }
        };

        // The graphql lane camelCases most keys, but the p083_* and
        // rollout_contract_*_config_state keys were not in the mapping and stay snake_case.
        let shutdown_deadline_config = str_field("rollout_contract_shutdown_deadline_config_state");
        let command_lease_ttl_config = str_field("rollout_contract_command_lease_ttl_config_state");

        Ok(GqlP083RolloutContractReadback {
            schema_version: Self::SCHEMA_VERSION.to_string(),
            // rollout_readback_api_parity_v1 fields — non-null String! fields use fallback defaults
            rollout_contract_status: str_field("status")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_decision: str_field("backendDecision")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_failure_reasons: str_arr("failureReasons"),
            rollout_contract_waiver_state: str_field("waiverState")
                .unwrap_or_else(|| "none".to_string()),
            rollout_contract_waiver_expires_at: str_field("waiverExpiresAt"),
            rollout_contract_enforcement_mode: str_field("enforcementMode")
                .unwrap_or_else(|| "disabled".to_string()),
            rollout_contract_enforcement_mode_reason: str_field("enforcementModeReason")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_hold_conditions: str_arr("holdConditions"),
            rollout_contract_rollback_disposition: validated_disposition.clone(),
            rollout_contract_source_lane: str_field("sourceLane")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_enabled_state: str_field("enabledState")
                .unwrap_or_else(|| "disabled".to_string()),
            rollout_contract_disabled_reason_code: str_field("disabledReasonCode"),
            rollout_contract_action_id: str_field("actionId"),
            rollout_contract_operator_message: str_field("operatorMessage")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_projection_integrity: str_field("projectionIntegrity")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_cutover_policy_revision: str_field("cutoverPolicyRevision")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_diagnostic_redaction: str_field("diagnosticRedaction")
                .unwrap_or_else(|| "not_applicable".to_string()),
            rollout_contract_next_steps: str_arr("nextSteps"),
            rollout_contract_shutdown_deadline_config_state: shutdown_deadline_config
                .unwrap_or_else(|| "not_configured".to_string()),
            rollout_contract_command_lease_ttl_config_state: command_lease_ttl_config
                .unwrap_or_else(|| "not_configured".to_string()),
            p083_rollback_ttl_expires_at: str_field("p083_rollback_ttl_expires_at"),
            p083_last_preflight_hash: str_field("p083_last_preflight_hash"),
            p083_shutdown_queue_rank: queue_rank,
            // applied_migrations: populated by the async resolver in schema.rs after DB query.
            applied_migrations: None,
            // Legacy alias fields (same values, preserved for existing callers).
            rollback_disposition: Some(validated_disposition),
            enforcement_mode: str_field("enforcementMode"),
            contract_decision: str_field("backendDecision"),
            hold_conditions: str_arr("holdConditions"),
            projection_integrity: str_field("projectionIntegrity"),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_rollback_disposition_passes() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "feature_flag_disable_or_enforcement_mode_permissive",
            "data_loss_risk": "none",
            "steps": ["Move enforcement mode through an audited mutation."]
        });
        assert!(validate_rollback_disposition_v1(&v).is_ok());
    }

    #[test]
    fn missing_schema_version_rejected() {
        let v = serde_json::json!({
            "mode": "feature_flag_disable_or_enforcement_mode_permissive",
            "data_loss_risk": "none",
            "steps": []
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(matches!(
            err,
            RollbackDispositionValidationError::MissingField("schema_version")
        ));
    }

    #[test]
    fn wrong_schema_version_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v2",
            "mode": "test",
            "data_loss_risk": "none",
            "steps": []
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(matches!(
            err,
            RollbackDispositionValidationError::InvalidSchemaVersion
        ));
    }

    #[test]
    fn non_string_mode_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": 42,
            "data_loss_risk": "none",
            "steps": []
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(matches!(
            err,
            RollbackDispositionValidationError::InvalidModeType
        ));
    }

    #[test]
    fn non_array_steps_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "test",
            "data_loss_risk": "none",
            "steps": "not_an_array"
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(matches!(
            err,
            RollbackDispositionValidationError::InvalidStepsType
        ));
    }

    #[test]
    fn missing_steps_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "feature_flag_disable_or_enforcement_mode_permissive",
            "data_loss_risk": "none"
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(matches!(
            err,
            RollbackDispositionValidationError::MissingField("steps")
        ));
    }

    #[test]
    fn invalid_data_loss_risk_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "test",
            "data_loss_risk": "extreme",
            "steps": []
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(matches!(
            err,
            RollbackDispositionValidationError::InvalidDataLossRisk(_)
        ));
    }

    #[test]
    fn numeric_data_loss_risk_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "test",
            "data_loss_risk": 0,
            "steps": []
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(
            matches!(
                err,
                RollbackDispositionValidationError::InvalidDataLossRiskType
            ),
            "numeric data_loss_risk must return InvalidDataLossRiskType, got: {err:?}"
        );
    }

    #[test]
    fn null_data_loss_risk_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "test",
            "data_loss_risk": null,
            "steps": []
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(
            matches!(
                err,
                RollbackDispositionValidationError::InvalidDataLossRiskType
            ),
            "null data_loss_risk must return InvalidDataLossRiskType, got: {err:?}"
        );
    }

    #[test]
    fn object_data_loss_risk_rejected() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "test",
            "data_loss_risk": {"level": "none"},
            "steps": []
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(
            matches!(
                err,
                RollbackDispositionValidationError::InvalidDataLossRiskType
            ),
            "object data_loss_risk must return InvalidDataLossRiskType, got: {err:?}"
        );
    }

    #[test]
    fn wrap_adds_schema_version_does_not_inject_steps() {
        let inline = serde_json::json!({
            "mode": "feature_flag_disable_or_enforcement_mode_permissive",
            "data_loss_risk": "none"
        });
        let wrapped = wrap_rollback_disposition_for_readback(&inline);
        assert_eq!(
            wrapped.get("schema_version").and_then(|v| v.as_str()),
            Some("rollback_disposition_v1")
        );
        // steps must NOT be injected — caller validates and treats failure as None.
        assert!(wrapped.get("steps").is_none());
        assert!(validate_rollback_disposition_v1(&wrapped).is_err());
    }

    #[test]
    fn wrap_does_not_allow_inline_to_override_schema_version() {
        let inline = serde_json::json!({
            "schema_version": "malicious_v99",
            "mode": "test",
            "data_loss_risk": "none",
            "steps": []
        });
        let wrapped = wrap_rollback_disposition_for_readback(&inline);
        assert_eq!(
            wrapped.get("schema_version").and_then(|v| v.as_str()),
            Some("rollback_disposition_v1"),
            "inline schema_version must not override canonical value"
        );
    }

    #[test]
    fn wrap_preserves_existing_steps() {
        let inline = serde_json::json!({
            "mode": "p083.rollback_execution_to_permissive_or_disabled",
            "data_loss_risk": "none",
            "steps": ["Call p083.rollback_execution with operator principal."]
        });
        let wrapped = wrap_rollback_disposition_for_readback(&inline);
        let steps = wrapped.get("steps").and_then(|v| v.as_array()).unwrap();
        assert_eq!(steps.len(), 1);
        assert!(validate_rollback_disposition_v1(&wrapped).is_ok());
    }

    #[test]
    fn rollback_disposition_json_validated_rejects_missing_field() {
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "data_loss_risk": "none",
            "steps": []
        });
        assert!(RollbackDispositionJSON::validated(v).is_err());
    }

    #[test]
    fn from_readback_json_rejects_invalid_disposition_not_fallback() {
        // Per rollback_disposition_json_v1.output_validation_rule: from_readback_json
        // must return Err for a readback that contains an invalid rollback_disposition,
        // not silently substitute a default value.
        let readback = serde_json::json!({
            "rollbackDisposition": {
                "mode": "some_mode",
                "data_loss_risk": "none"
                // missing steps — invalid
            },
            "status": "pass",
            "backendDecision": "release"
        });
        let result = GqlP083RolloutContractReadback::from_readback_json(&readback, None);
        assert!(
            result.is_err(),
            "from_readback_json must return Err for invalid rollback_disposition, not a default"
        );
    }

    #[test]
    fn from_readback_json_succeeds_with_no_disposition_field() {
        // When readback has no rollback_disposition field, from_readback_json
        // uses the not_applicable default (legitimate absence, not validation failure).
        let readback = serde_json::json!({
            "status": "not_applicable",
            "backendDecision": "not_applicable"
        });
        let result = GqlP083RolloutContractReadback::from_readback_json(&readback, None);
        assert!(
            result.is_ok(),
            "from_readback_json must succeed when rollback_disposition is absent"
        );
        let obj = result.unwrap();
        assert_eq!(
            obj.rollout_contract_rollback_disposition
                .inner()
                .get("mode")
                .and_then(|v| v.as_str()),
            Some("not_applicable"),
            "absent disposition must use not_applicable default"
        );
    }

    #[test]
    fn from_readback_json_succeeds_with_valid_disposition() {
        let readback = serde_json::json!({
            "rollbackDisposition": {
                "mode": "p083.rollback_execution_to_permissive_or_disabled",
                "data_loss_risk": "none",
                "steps": ["Call p083RollbackExecution."]
            },
            "status": "pass"
        });
        let result = GqlP083RolloutContractReadback::from_readback_json(&readback, None);
        assert!(result.is_ok());
    }

    #[test]
    fn extra_property_rejected_additional_properties_false() {
        // Per rollback_disposition_json_v1.mcp_schema_rule: additionalProperties=false.
        // Extra keys beyond the four schema-mandated fields must be rejected.
        let v = serde_json::json!({
            "schema_version": "rollback_disposition_v1",
            "mode": "p083.rollback_execution_to_permissive_or_disabled",
            "data_loss_risk": "none",
            "steps": [],
            "extra_diagnostic": "leaked field"
        });
        let err = validate_rollback_disposition_v1(&v).unwrap_err();
        assert!(
            matches!(err, RollbackDispositionValidationError::ExtraProperty(ref k) if k == "extra_diagnostic"),
            "validate_rollback_disposition_v1 must reject extra property extra_diagnostic, got: {err:?}"
        );
    }

    #[test]
    fn wrap_strips_extra_properties_before_validation() {
        // wrap_rollback_disposition_for_readback whitelists only 4 fields;
        // extra keys in stored inline disposition are silently stripped.
        let inline = serde_json::json!({
            "mode": "p083.rollback_execution_to_permissive_or_disabled",
            "data_loss_risk": "none",
            "steps": ["Step"],
            "extra_field": "should be stripped"
        });
        let wrapped = wrap_rollback_disposition_for_readback(&inline);
        assert!(
            wrapped.get("extra_field").is_none(),
            "extra_field must be stripped by wrap"
        );
        // After wrapping, validation should pass (schema_version is added, extra removed).
        assert!(validate_rollback_disposition_v1(&wrapped).is_ok());
    }
}

/// P083: Identity-ambiguous provider session awaiting manual identity verification.
/// Returned by p083IdentityHoldSessions query per manual_process_identity_check_ui_v1.
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlP083IdentityHoldSession {
    pub provider_session_id: String,
    pub provider_name: String,
    pub cancellation_epoch: Option<i64>,
    pub last_seen_pid: Option<i64>,
    pub process_start_identity_hash: Option<String>,
    pub live_probe_status: String,
    pub live_probe_detail: String,
    pub latest_receipt_id: Option<String>,
    pub reason_detail: Option<String>,
}

/// P083: GraphQL payload returned by the providerSessionShutdown mutation.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "ProviderSessionShutdownSuccess")]
pub struct GqlP083ProviderSessionShutdownSuccess {
    /// true when process signal dispatch is active (process_id was known at command time).
    /// false means the intent is held pending manual identity check (ProviderSessionShutdownHeld).
    pub scheduled: bool,
    /// Authoritative provider session ID from provider_sessions.
    pub provider_session_id: String,
    /// Service-assigned epoch ms for the cancellation intent.
    pub cancellation_epoch: i64,
    /// Command journal entry ID for auditability.
    pub journal_id: String,
    /// Replayed CallerRequestId confirming idempotency lease.
    pub request_id: String,
    /// Non-null when process_id was unknown at command time (SEC-P083-HIGH-001).
    /// Value: "manual_process_identity_check". Null when scheduled=true.
    pub operator_next_step_code: Option<String>,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "ProviderSessionShutdownPayload")]
pub enum GqlP083ProviderSessionShutdownPayload {
    Success(GqlP083ProviderSessionShutdownSuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL payload returned by the runsCancel mutation.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "RunsCancelSuccess")]
pub struct GqlRunsCancelSuccess {
    pub run_id: String,
    pub cancellation_epoch: Option<i64>,
    pub journal_id: String,
    pub request_id: String,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "RunsCancelPayload")]
pub enum GqlRunsCancelPayload {
    Success(GqlRunsCancelSuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL payload returned by the stagesRetry mutation.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "StagesRetrySuccess")]
pub struct GqlStagesRetrySuccess {
    pub stage_execution_id: String,
    pub stage_id: String,
    pub journal_id: String,
    pub request_id: String,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "StagesRetryPayload")]
pub enum GqlStagesRetryPayload {
    Success(GqlStagesRetrySuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL payload returned by the sideEffectsForceReconcile mutation.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "SideEffectsForceReconcileSuccess")]
pub struct GqlSideEffectsForceReconcileSuccess {
    pub side_effect_id: String,
    pub journal_id: String,
    pub request_id: String,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "SideEffectsForceReconcilePayload")]
pub enum GqlSideEffectsForceReconcilePayload {
    Success(GqlSideEffectsForceReconcileSuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL payload returned by the p083RollbackExecution mutation.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P083RollbackTargetMode", rename_items = "snake_case")]
pub enum GqlP083RollbackTargetMode {
    Permissive,
    Disabled,
}

impl GqlP083RollbackTargetMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Permissive => "permissive",
            Self::Disabled => "disabled",
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "P083EnforcementMode", rename_items = "snake_case")]
pub enum GqlP083EnforcementMode {
    Disabled,
    Permissive,
    Enforce,
}

impl GqlP083EnforcementMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Permissive => "permissive",
            Self::Enforce => "enforce",
        }
    }
}

/// P083: GraphQL payload returned by the p083RollbackExecution mutation.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "P083RollbackExecutionSuccess")]
pub struct GqlP083RollbackExecutionSuccess {
    /// true when the rollback was durably committed to the audit table.
    pub committed: bool,
    /// The rollback target mode: permissive or disabled.
    pub target_enforcement_mode: String,
    /// Command journal entry ID for auditability.
    pub journal_id: String,
    /// Replayed CallerRequestId confirming idempotency lease.
    pub request_id: String,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "P083RollbackExecutionPayload")]
pub enum GqlP083RollbackExecutionPayload {
    Success(GqlP083RollbackExecutionSuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL payload returned by the p083SetEnforcementMode mutation.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "P083SetEnforcementModeSuccess")]
pub struct GqlP083SetEnforcementModeSuccess {
    /// true when the mode change was durably committed.
    pub committed: bool,
    /// The new enforcement mode: disabled, permissive, or enforce.
    pub enforcement_mode: String,
    /// Command journal entry ID for auditability.
    pub journal_id: String,
    /// Replayed CallerRequestId confirming idempotency lease.
    pub request_id: String,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "P083SetEnforcementModePayload")]
pub enum GqlP083SetEnforcementModePayload {
    Success(GqlP083SetEnforcementModeSuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL payload returned by the runsRetry mutation.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "RunsRetrySuccess")]
pub struct GqlRetryRunSuccess {
    /// true when the AdvanceRun work item was durably enqueued.
    pub queued: bool,
    /// Run ID that was re-queued.
    pub run_id: String,
    /// Command journal entry ID for auditability.
    pub journal_id: String,
    /// Replayed CallerRequestId confirming idempotency lease.
    pub request_id: String,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "RunsRetryPayload")]
pub enum GqlRetryRunPayload {
    Success(GqlRetryRunSuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL payload returned by the p083MarkProviderSessionProcessAbsent mutation.
/// Per manual_process_identity_check_ui_v1.available_actions.mark_process_absent:
/// operator has confirmed the process is absent; process_fate=absent_verified and
/// held intent is re-opened to requested so settlement can resume.
#[derive(SimpleObject, Clone, Debug)]
#[graphql(name = "P083MarkProcessAbsentSuccess")]
pub struct GqlP083MarkProcessAbsentSuccess {
    /// true when process_fate was successfully moved to absent_verified.
    pub marked_absent: bool,
    /// Authoritative provider session ID.
    pub provider_session_id: String,
    /// The cancellation_epoch of the cleared held intent.
    pub cancellation_epoch: i64,
    /// Command journal entry ID for auditability.
    pub journal_id: String,
    /// Replayed CallerRequestId confirming idempotency lease.
    pub request_id: String,
}

#[derive(Union, Clone, Debug)]
#[graphql(name = "P083MarkProcessAbsentPayload")]
pub enum GqlP083MarkProcessAbsentPayload {
    Success(GqlP083MarkProcessAbsentSuccess),
    Denial(DenialPayload),
}

/// P083: GraphQL readback row for a cancel_late_output_overflow latch.
/// Per post_cancel_late_output_contract_v1.readback_fields.
#[derive(SimpleObject, Clone, Debug)]
pub struct GqlP083CancelLateOutputOverflowRow {
    pub overflow_id: String,
    pub scope: String,
    pub run_id: Option<String>,
    pub provider_session_id: Option<String>,
    pub cancellation_epoch: i64,
    pub overflow_kind: String,
    pub dropped_message_count: i64,
    pub dropped_byte_count: i64,
    pub quarantine_uri: Option<String>,
    pub reservation_release_state: String,
    /// true = active projections cannot be mutated by quarantined late output.
    pub projection_mutation_blocked: bool,
    pub normalized_run_id: String,
    pub normalized_provider_session_id: String,
    pub latched_at: String,
}

impl From<db::repos::cancel_late_output_overflow::CancelLateOutputOverflowRow>
    for GqlP083CancelLateOutputOverflowRow
{
    fn from(r: db::repos::cancel_late_output_overflow::CancelLateOutputOverflowRow) -> Self {
        Self {
            overflow_id: r.overflow_id,
            scope: r.scope,
            run_id: r.run_id,
            provider_session_id: r.provider_session_id,
            cancellation_epoch: r.cancellation_epoch,
            overflow_kind: r.overflow_kind,
            dropped_message_count: r.dropped_message_count,
            dropped_byte_count: r.dropped_byte_count,
            quarantine_uri: r.quarantine_uri,
            reservation_release_state: r.reservation_release_state,
            projection_mutation_blocked: r.projection_mutation_blocked,
            normalized_run_id: r.normalized_run_id,
            normalized_provider_session_id: r.normalized_provider_session_id,
            latched_at: r.latched_at,
        }
    }
}
