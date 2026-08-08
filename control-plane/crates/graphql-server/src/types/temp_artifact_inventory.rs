use std::sync::{Arc, OnceLock};

use async_graphql::{
    Enum, InputObject, InputValueError, InputValueResult, Scalar, ScalarType, SimpleObject, Value,
};
use domain::temp_artifact_inventory::{
    DryRunRecommendation, EnabledState, InventoryErrorCode, InventoryMode, InventoryStatus,
    LifecycleClassification, MutationGuardStatus, RootKind, TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
};

// ── Backend parity wiring ──────────────────────────────────────────────────────
//
// GraphQL must be a lossless projection of the same canonical DTO the MCP tool
// and `chainworks://runs/{run_id}/temp-artifact-inventory` resource lane produce
// (readback parity requirement). Rather than duplicating the scanner, mode
// handling, redaction, and mutation guard here, the daemon installs a live
// backend at startup (see `install_backend`) that delegates to
// `mcp_server::tools::temp_artifacts::inventory_preview`. graphql-server cannot
// depend on mcp-server directly (mcp-server already depends on graphql-server),
// so this trait + process-static handle inverts the dependency without a
// circular crate reference.

/// Implemented by the MCP-side inventory preview so the GraphQL resolver can
/// reuse the identical mode-check/validation/scan/redaction/mutation-guard path.
#[async_trait::async_trait]
pub trait TempArtifactInventoryBackend: Send + Sync {
    async fn inventory_preview(
        &self,
        params: serde_json::Value,
        principal: &auth::Principal,
    ) -> anyhow::Result<serde_json::Value>;
}

static BACKEND: OnceLock<Arc<dyn TempArtifactInventoryBackend>> = OnceLock::new();

/// Installs the live inventory backend. Called once at daemon startup, before the
/// GraphQL server accepts requests. A no-op if already installed.
pub fn install_backend(backend: Arc<dyn TempArtifactInventoryBackend>) {
    let _ = BACKEND.set(backend);
}

/// Returns the installed backend, if any. `None` in test builds and any process
/// that never calls `install_backend` (e.g. unit tests building a bare schema).
pub fn backend() -> Option<Arc<dyn TempArtifactInventoryBackend>> {
    BACKEND.get().cloned()
}

/// Converts the typed GraphQL input into the snake_case params map the MCP-side
/// `inventory_preview` expects, so both lanes share one request contract.
pub fn to_backend_params(input: &GqlTempArtifactInventoryInput) -> serde_json::Value {
    serde_json::json!({
        "run_id": input.run_id.as_ref().map(|id| id.to_string()),
        "workspace_context": input.workspace_context.as_ref().map(|w| {
            serde_json::json!({ "workspace_root": w.workspace_root })
        }),
        "limit": input.limit,
        "timeout_ms": input.timeout_ms,
        "include_dry_run": input.include_dry_run,
        "test_root_override": input.test_root_override,
    })
}

/// Parses the canonical snake_case `temp_artifact_inventory_v1` JSON DTO (as
/// produced by the MCP lane) into the camelCase GraphQL projection.
///
/// This boundary is deliberately fail closed. GraphQL output values do not pass
/// through scalar input parsing, so accepting missing or malformed canonical
/// fields here would let a corrupt backend payload masquerade as a successful,
/// lossless projection. Any structural, scalar, or timestamp failure returns the
/// redacted integrity-error DTO.
pub fn from_canonical_json(raw: &serde_json::Value) -> GqlTempArtifactInventory {
    match try_from_canonical_json(raw) {
        Ok(inv) => inv,
        Err(()) => build_integrity_error_inventory(),
    }
}

fn try_from_canonical_json(raw: &serde_json::Value) -> Result<GqlTempArtifactInventory, ()> {
    let required_str = |v: &serde_json::Value, k: &str| -> Result<String, ()> {
        v.get(k)
            .and_then(serde_json::Value::as_str)
            .map(ToString::to_string)
            .ok_or(())
    };
    let optional_str = |v: &serde_json::Value, k: &str| -> Result<Option<String>, ()> {
        match v.get(k).ok_or(())? {
            serde_json::Value::Null => Ok(None),
            serde_json::Value::String(value) => Ok(Some(value.clone())),
            _ => Err(()),
        }
    };
    let required_i32 = |v: &serde_json::Value, k: &str| -> Result<i32, ()> {
        let value = v.get(k).and_then(serde_json::Value::as_i64).ok_or(())?;
        i32::try_from(value).map_err(|_| ())
    };
    let required_bool = |v: &serde_json::Value, k: &str| -> Result<bool, ()> {
        v.get(k).and_then(serde_json::Value::as_bool).ok_or(())
    };
    let byte_count_field = |v: &serde_json::Value, k: &str| -> Result<GqlByteCountString, ()> {
        let value = v.get(k).and_then(serde_json::Value::as_str).ok_or(())?;
        validate_byte_count_string(value).map_err(|_| ())?;
        Ok(GqlByteCountString(value.to_string()))
    };

    let schema_version = required_str(raw, "schema_version")?;
    if schema_version != TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION {
        return Err(());
    }
    let status_value = required_str(raw, "status")?;
    let status = Some(status_value)
        .and_then(|value| serde_json::from_value(serde_json::Value::String(value)).ok())
        .unwrap_or(InventoryStatus::Unknown);
    let enabled_state_value = required_str(raw, "enabled_state")?;
    let enabled_state = Some(enabled_state_value)
        .and_then(|value| serde_json::from_value(serde_json::Value::String(value)).ok())
        .unwrap_or(EnabledState::Unknown);
    let mode = InventoryMode::from_env_str(&required_str(raw, "mode")?).ok_or(())?;
    let generated_at = parse_output_datetime(&required_str(raw, "generated_at")?)?;
    let disabled_reason_code = optional_str(raw, "disabled_reason_code")?;

    let limits_raw = raw
        .get("limits_applied")
        .filter(|v| v.is_object())
        .ok_or(())?;
    let limits_applied = GqlTempArtifactLimitsApplied {
        limit: required_i32(limits_raw, "limit")?,
        timeout_ms: required_i32(limits_raw, "timeout_ms")?,
        scan_deadline_at: optional_str(limits_raw, "scan_deadline_at")?
            .map(|value| parse_output_datetime(&value))
            .transpose()?,
        queue_wait_ms: required_i32(limits_raw, "queue_wait_ms")?,
    };

    let summary_raw = raw.get("summary").filter(|v| v.is_object()).ok_or(())?;
    let summary = GqlTempArtifactSummary {
        artifact_tree_count: required_i32(summary_raw, "artifact_tree_count")?,
        estimated_bytes: byte_count_field(summary_raw, "estimated_bytes")?,
        active_or_recent_count: required_i32(summary_raw, "active_or_recent_count")?,
        terminal_candidate_count: required_i32(summary_raw, "terminal_candidate_count")?,
        orphan_candidate_count: required_i32(summary_raw, "orphan_candidate_count")?,
        legacy_unmanaged_count: required_i32(summary_raw, "legacy_unmanaged_count")?,
        scan_error_count: required_i32(summary_raw, "scan_error_count")?,
        dry_run_candidate_count: required_i32(summary_raw, "dry_run_candidate_count")?,
        truncated: required_bool(summary_raw, "truncated")?,
        queue_wait_ms: required_i32(summary_raw, "queue_wait_ms")?,
    };

    let rows = raw
        .get("rows")
        .and_then(|v| v.as_array())
        .ok_or(())?
        .iter()
        .map(|row| -> Result<GqlTempArtifactRow, ()> {
            let root_kind = required_str(row, "root_kind")?;
            let lifecycle_classification = required_str(row, "lifecycle_classification")?;
            let partial_errors = row
                .get("partial_errors")
                .and_then(serde_json::Value::as_array)
                .ok_or(())?
                .iter()
                .map(|value| value.as_str().map(ToString::to_string).ok_or(()))
                .collect::<Result<Vec<_>, ()>>()?;
            Ok(GqlTempArtifactRow {
                path_display: required_str(row, "path_display")?,
                path_hash: required_str(row, "path_hash")?,
                path_hash_short: required_str(row, "path_hash_short")?,
                correlation_key: required_str(row, "correlation_key")?,
                root_kind: GqlRootKind::from_canonical_str(Some(&root_kind)),
                artifact_kind: optional_str(row, "artifact_kind")?,
                manifest_state: optional_str(row, "manifest_state")?,
                lifecycle_classification: GqlLifecycleClassification::from_canonical_str(Some(
                    &lifecycle_classification,
                )),
                dry_run_recommendation: GqlDryRunRecommendation::from_canonical_str(
                    optional_str(row, "dry_run_recommendation")?.as_deref(),
                ),
                estimated_size_bytes: byte_count_field(row, "estimated_size_bytes")?,
                last_touched_at: optional_str(row, "last_touched_at")?
                    .map(|value| parse_output_datetime(&value))
                    .transpose()?,
                active_process_evidence: optional_str(row, "active_process_evidence")?,
                owner: optional_str(row, "owner")?,
                owner_inference: optional_str(row, "owner_inference")?,
                status_token: required_str(row, "status_token")?,
                generated_at: parse_output_datetime(&required_str(row, "generated_at")?)?,
                partial_errors,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;
    let errors = raw
        .get("errors")
        .and_then(|v| v.as_array())
        .ok_or(())?
        .iter()
        .map(|error| -> Result<GqlTempArtifactError, ()> {
            let code = required_str(error, "code")?;
            let root_kind = optional_str(error, "root_kind")?;
            Ok(GqlTempArtifactError {
                code: GqlInventoryErrorCode::from_canonical_str(Some(&code)),
                message: required_str(error, "message")?,
                root_kind: root_kind
                    .as_deref()
                    .map(|value| GqlRootKind::from_canonical_str(Some(value))),
                phase: optional_str(error, "phase")?,
            })
        })
        .collect::<Result<Vec<_>, ()>>()?;

    let dry_run = match raw.get("dry_run").ok_or(())? {
        serde_json::Value::Null => None,
        dr if dr.is_object() => {
            let guard_raw = dr
                .get("mutation_guard")
                .filter(|value| value.is_object())
                .ok_or(())?;
            let guard_status_value = required_str(guard_raw, "status")?;
            let guard_status = Some(guard_status_value)
                .and_then(|value| serde_json::from_value(serde_json::Value::String(value)).ok())
                .unwrap_or(MutationGuardStatus::Unknown);
            let recommendation_counts = dr
                .get("recommendation_counts")
                .filter(|value| value.is_object())
                .cloned()
                .ok_or(())?;
            Some(GqlTempArtifactDryRun {
                schema_version: required_str(dr, "schema_version")?,
                generated_at: optional_str(dr, "generated_at")?
                    .map(|value| parse_output_datetime(&value))
                    .transpose()?,
                recommendation_counts: async_graphql::Json(recommendation_counts),
                mutation_guard: GqlTempArtifactDryRunMutationGuard {
                    status: GqlMutationGuardStatus::from(guard_status),
                    checked_at: parse_output_datetime(&required_str(guard_raw, "checked_at")?)?,
                },
            })
        }
        _ => return Err(()),
    };

    let mg_raw = raw
        .get("mutation_guard")
        .filter(|value| value.is_object())
        .ok_or(())?;
    let mg_status_value = required_str(mg_raw, "status")?;
    let mg_status = Some(mg_status_value)
        .and_then(|value| serde_json::from_value(serde_json::Value::String(value)).ok())
        .unwrap_or(MutationGuardStatus::Unknown);
    let mutation_guard = GqlTempArtifactMutationGuard {
        status: GqlMutationGuardStatus::from(mg_status),
        checked_at: parse_output_datetime(&required_str(mg_raw, "checked_at")?)?,
        no_delete: required_bool(mg_raw, "no_delete")?,
        no_prune: required_bool(mg_raw, "no_prune")?,
        no_chmod: required_bool(mg_raw, "no_chmod")?,
        no_persist: required_bool(mg_raw, "no_persist")?,
        no_retry: required_bool(mg_raw, "no_retry")?,
    };

    Ok(GqlTempArtifactInventory {
        schema_version,
        status: GqlInventoryStatus::from(status),
        enabled_state: GqlEnabledState::from(enabled_state),
        mode: GqlInventoryMode::from(mode),
        disabled_reason_code,
        generated_at,
        limits_applied,
        summary,
        rows,
        errors,
        dry_run,
        mutation_guard,
    })
}

fn parse_output_datetime(value: &str) -> Result<GqlDateTime, ()> {
    let parsed = chrono::DateTime::parse_from_rfc3339(value).map_err(|_| ())?;
    Ok(GqlDateTime(
        parsed
            .with_timezone(&chrono::Utc)
            .to_rfc3339_opts(chrono::SecondsFormat::AutoSi, true),
    ))
}

/// Explicit "backend not wired" signal, distinct from a real `mode_disabled`
/// readback, for any process that builds a GraphQL schema without installing
/// the live backend (e.g. a unit test schema). Never returned by the running
/// daemon once `install_backend` has run.
pub fn build_not_wired_inventory(include_dry_run: bool) -> GqlTempArtifactInventory {
    let mut inv = build_disabled_inventory(include_dry_run);
    inv.disabled_reason_code = Some("backend_not_wired".to_string());
    inv
}

/// Safe substitute for a canonical DTO that failed `try_from_canonical_json`'s
/// `ByteCountString` contract check — an integrity failure distinct from a real
/// `mode_disabled`/error readback, so it never claims a scan actually ran.
pub fn build_integrity_error_inventory() -> GqlTempArtifactInventory {
    let now = GqlDateTime::now();
    let mut inv = build_disabled_inventory(false);
    inv.status = GqlInventoryStatus::from(InventoryStatus::Error);
    inv.enabled_state = GqlEnabledState::from(EnabledState::Unknown);
    // An integrity/parse failure can happen in any backend mode, not only
    // `disabled` — report the real process mode rather than the `disabled`
    // default `build_disabled_inventory` set above.
    inv.mode = GqlInventoryMode::from(current_inventory_mode());
    inv.disabled_reason_code = None;
    inv.generated_at = now.clone();
    inv.errors = vec![GqlTempArtifactError {
        code: GqlInventoryErrorCode::InternalError,
        message: "<redacted>".to_string(),
        root_kind: None,
        phase: None,
    }];
    inv
}

// ── Scalars ───────────────────────────────────────────────────────────────────

/// P089 ByteCountString scalar: unsigned decimal string only.
/// Accepts "0" or positive decimal digits without leading zeros.
/// Rejects numeric values, negatives, leading zeros, empty, and whitespace.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GqlByteCountString(pub String);

#[Scalar(name = "ByteCountString")]
impl ScalarType for GqlByteCountString {
    fn parse(value: Value) -> InputValueResult<Self> {
        match &value {
            Value::String(s) => {
                validate_byte_count_string(s).map_err(InputValueError::custom)?;
                Ok(Self(s.clone()))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.clone())
    }
}

fn validate_byte_count_string(s: &str) -> Result<(), String> {
    if s.is_empty() {
        return Err("ByteCountString must not be empty".into());
    }
    let bytes = s.as_bytes();
    if bytes[0] == b'-' {
        return Err("ByteCountString must not be negative".into());
    }
    if !bytes.iter().all(|b| b.is_ascii_digit()) {
        return Err("ByteCountString must contain only decimal digits".into());
    }
    if bytes.len() > 1 && bytes[0] == b'0' {
        return Err("ByteCountString must not have leading zeros".into());
    }
    Ok(())
}

/// P089 DateTime scalar: ISO-8601/RFC3339 UTC timestamps.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GqlDateTime(pub String);

#[Scalar(name = "DateTime")]
impl ScalarType for GqlDateTime {
    fn parse(value: Value) -> InputValueResult<Self> {
        match &value {
            Value::String(s) => {
                chrono::DateTime::parse_from_rfc3339(s)
                    .map_err(|e| InputValueError::custom(format!("invalid DateTime: {e}")))?;
                Ok(Self(s.clone()))
            }
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.clone())
    }
}

impl GqlDateTime {
    pub fn now() -> Self {
        Self(chrono::Utc::now().to_rfc3339())
    }
}

// ── Enums ─────────────────────────────────────────────────────────────────────

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "InventoryStatus", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlInventoryStatus {
    Complete,
    Partial,
    Timeout,
    Cancelled,
    Error,
    Disabled,
    ResourceExhausted,
    Unknown,
}

impl From<InventoryStatus> for GqlInventoryStatus {
    fn from(s: InventoryStatus) -> Self {
        match s {
            InventoryStatus::Complete => Self::Complete,
            InventoryStatus::Partial => Self::Partial,
            InventoryStatus::Timeout => Self::Timeout,
            InventoryStatus::Cancelled => Self::Cancelled,
            InventoryStatus::Error => Self::Error,
            InventoryStatus::Disabled => Self::Disabled,
            InventoryStatus::ResourceExhausted => Self::ResourceExhausted,
            InventoryStatus::Unknown => Self::Unknown,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "InventoryEnabledState", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlEnabledState {
    Enabled,
    Disabled,
    Unknown,
}

impl From<EnabledState> for GqlEnabledState {
    fn from(s: EnabledState) -> Self {
        match s {
            EnabledState::Enabled => Self::Enabled,
            EnabledState::Disabled => Self::Disabled,
            EnabledState::Unknown => Self::Unknown,
        }
    }
}

/// The daemon process-start backend mode (`CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE`),
/// distinct from `enabled_state`: `hidden_readback` and `operator_visible` both
/// report `enabled_state: ENABLED`, but only `operator_visible` authorizes the
/// packaged app to show the diagnostics surface. Swift composes this field with
/// its local `TempArtifactDiagnosticsVisibilityStore` preference rather than
/// trusting the local preference alone.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "InventoryMode", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlInventoryMode {
    Disabled,
    HiddenReadback,
    OperatorVisible,
}

impl From<InventoryMode> for GqlInventoryMode {
    fn from(m: InventoryMode) -> Self {
        match m {
            InventoryMode::Disabled => Self::Disabled,
            InventoryMode::HiddenReadback => Self::HiddenReadback,
            InventoryMode::OperatorVisible => Self::OperatorVisible,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "MutationGuardStatus", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlMutationGuardStatus {
    Pass,
    Fail,
    Skipped,
    Unknown,
}

impl From<MutationGuardStatus> for GqlMutationGuardStatus {
    fn from(s: MutationGuardStatus) -> Self {
        match s {
            MutationGuardStatus::Pass => Self::Pass,
            MutationGuardStatus::Fail => Self::Fail,
            MutationGuardStatus::Skipped => Self::Skipped,
            MutationGuardStatus::Unknown => Self::Unknown,
        }
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "RootKind", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlRootKind {
    RunMetaRoot,
    ControlPlaneCache,
    ProviderHomeCopy,
    LegacyChainworksTmp,
    DiagnosticTestRoot,
    Unknown,
}

impl From<RootKind> for GqlRootKind {
    fn from(s: RootKind) -> Self {
        match s {
            RootKind::RunMetaRoot => Self::RunMetaRoot,
            RootKind::ControlPlaneCache => Self::ControlPlaneCache,
            RootKind::ProviderHomeCopy => Self::ProviderHomeCopy,
            RootKind::LegacyChainworksTmp => Self::LegacyChainworksTmp,
            RootKind::DiagnosticTestRoot => Self::DiagnosticTestRoot,
            RootKind::Unknown => Self::Unknown,
        }
    }
}

impl GqlRootKind {
    /// Parses the canonical snake_case JSON string for this field, falling back to
    /// `Unknown` (never erroring the whole payload) for an absent or unrecognized
    /// value — the enum's evolvability contract (proposal: "externally evolvable
    /// GraphQL enums include UNKNOWN").
    fn from_canonical_str(s: Option<&str>) -> Self {
        s.and_then(|s| {
            serde_json::from_value::<RootKind>(serde_json::Value::String(s.to_string())).ok()
        })
        .map(Self::from)
        .unwrap_or(Self::Unknown)
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(
    name = "LifecycleClassification",
    rename_items = "SCREAMING_SNAKE_CASE"
)]
pub enum GqlLifecycleClassification {
    ActiveOrRecent,
    TerminalCandidate,
    OrphanCandidate,
    LegacyUnmanaged,
    ScanError,
    Unknown,
}

impl From<LifecycleClassification> for GqlLifecycleClassification {
    fn from(s: LifecycleClassification) -> Self {
        match s {
            LifecycleClassification::ActiveOrRecent => Self::ActiveOrRecent,
            LifecycleClassification::TerminalCandidate => Self::TerminalCandidate,
            LifecycleClassification::OrphanCandidate => Self::OrphanCandidate,
            LifecycleClassification::LegacyUnmanaged => Self::LegacyUnmanaged,
            LifecycleClassification::ScanError => Self::ScanError,
            LifecycleClassification::Unknown => Self::Unknown,
        }
    }
}

impl GqlLifecycleClassification {
    fn from_canonical_str(s: Option<&str>) -> Self {
        s.and_then(|s| {
            serde_json::from_value::<LifecycleClassification>(serde_json::Value::String(
                s.to_string(),
            ))
            .ok()
        })
        .map(Self::from)
        .unwrap_or(Self::Unknown)
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "DryRunRecommendation", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlDryRunRecommendation {
    WouldKeepActive,
    WouldKeepRecent,
    WouldPreserveFailureEvidence,
    WouldDeleteAfterFutureApproval,
    WouldMigrateLegacyManifestAfterFutureMigrationEnabled,
    NeedsOperatorReview,
    NoRecommendation,
    Unknown,
}

impl From<DryRunRecommendation> for GqlDryRunRecommendation {
    fn from(s: DryRunRecommendation) -> Self {
        match s {
            DryRunRecommendation::WouldKeepActive => Self::WouldKeepActive,
            DryRunRecommendation::WouldKeepRecent => Self::WouldKeepRecent,
            DryRunRecommendation::WouldPreserveFailureEvidence => {
                Self::WouldPreserveFailureEvidence
            }
            DryRunRecommendation::WouldDeleteAfterFutureApproval => {
                Self::WouldDeleteAfterFutureApproval
            }
            DryRunRecommendation::WouldMigrateLegacyManifestAfterFutureMigrationEnabled => {
                Self::WouldMigrateLegacyManifestAfterFutureMigrationEnabled
            }
            DryRunRecommendation::NeedsOperatorReview => Self::NeedsOperatorReview,
            DryRunRecommendation::NoRecommendation => Self::NoRecommendation,
            DryRunRecommendation::Unknown => Self::Unknown,
        }
    }
}

impl GqlDryRunRecommendation {
    /// Returns `None` only when the canonical field itself is absent/null (a real
    /// "no recommendation was computed" case, e.g. `include_dry_run=false`); an
    /// unrecognized non-null string still falls back to `Some(Unknown)` rather
    /// than being conflated with "absent".
    fn from_canonical_str(s: Option<&str>) -> Option<Self> {
        s.map(|s| {
            serde_json::from_value::<DryRunRecommendation>(serde_json::Value::String(s.to_string()))
                .ok()
                .map(Self::from)
                .unwrap_or(Self::Unknown)
        })
    }
}

#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "InventoryErrorCode", rename_items = "SCREAMING_SNAKE_CASE")]
pub enum GqlInventoryErrorCode {
    InvalidRootOverride,
    RootUnreadable,
    ManifestParseFailed,
    SizeEstimationFailed,
    DeadlineExceeded,
    Cancelled,
    InternalError,
    MutationGuardFailed,
    ResourceExhausted,
    Unknown,
}

impl From<InventoryErrorCode> for GqlInventoryErrorCode {
    fn from(s: InventoryErrorCode) -> Self {
        match s {
            InventoryErrorCode::InvalidRootOverride => Self::InvalidRootOverride,
            InventoryErrorCode::RootUnreadable => Self::RootUnreadable,
            InventoryErrorCode::ManifestParseFailed => Self::ManifestParseFailed,
            InventoryErrorCode::SizeEstimationFailed => Self::SizeEstimationFailed,
            InventoryErrorCode::DeadlineExceeded => Self::DeadlineExceeded,
            InventoryErrorCode::Cancelled => Self::Cancelled,
            InventoryErrorCode::InternalError => Self::InternalError,
            InventoryErrorCode::MutationGuardFailed => Self::MutationGuardFailed,
            InventoryErrorCode::ResourceExhausted => Self::ResourceExhausted,
            InventoryErrorCode::Unknown => Self::Unknown,
        }
    }
}

impl GqlInventoryErrorCode {
    fn from_canonical_str(s: Option<&str>) -> Self {
        s.and_then(|s| {
            serde_json::from_value::<InventoryErrorCode>(serde_json::Value::String(s.to_string()))
                .ok()
        })
        .map(Self::from)
        .unwrap_or(Self::Unknown)
    }
}

// ── Input types ───────────────────────────────────────────────────────────────

#[derive(InputObject, Debug)]
#[graphql(name = "TempArtifactWorkspaceContextInput")]
pub struct GqlTempArtifactWorkspaceContextInput {
    /// Required (non-null) in the SDL: a `workspace_context` selector with no root
    /// is not a meaningful request shape, so this must reject at the GraphQL
    /// validation layer rather than reach the resolver as `null`.
    pub workspace_root: String,
}

#[derive(InputObject, Debug)]
#[graphql(name = "TempArtifactInventoryInput")]
pub struct GqlTempArtifactInventoryInput {
    pub run_id: Option<async_graphql::ID>,
    pub workspace_context: Option<GqlTempArtifactWorkspaceContextInput>,
    #[graphql(default = 500)]
    pub limit: i32,
    #[graphql(default = 5000)]
    pub timeout_ms: i32,
    #[graphql(default = true)]
    pub include_dry_run: bool,
    pub test_root_override: Option<String>,
}

// ── Output types ──────────────────────────────────────────────────────────────

#[derive(SimpleObject, Debug, Clone)]
#[graphql(
    name = "TempArtifactInventoryLimitsApplied",
    rename_fields = "camelCase"
)]
pub struct GqlTempArtifactLimitsApplied {
    pub limit: i32,
    pub timeout_ms: i32,
    pub scan_deadline_at: Option<GqlDateTime>,
    /// Queue wait time in milliseconds before scan admission.
    pub queue_wait_ms: i32,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactInventorySummary", rename_fields = "camelCase")]
pub struct GqlTempArtifactSummary {
    pub artifact_tree_count: i32,
    pub estimated_bytes: GqlByteCountString,
    pub active_or_recent_count: i32,
    pub terminal_candidate_count: i32,
    pub orphan_candidate_count: i32,
    pub legacy_unmanaged_count: i32,
    pub scan_error_count: i32,
    pub dry_run_candidate_count: i32,
    pub truncated: bool,
    pub queue_wait_ms: i32,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactRow", rename_fields = "camelCase")]
pub struct GqlTempArtifactRow {
    pub path_display: String,
    pub path_hash: String,
    pub path_hash_short: String,
    pub correlation_key: String,
    pub root_kind: GqlRootKind,
    pub artifact_kind: Option<String>,
    pub manifest_state: Option<String>,
    pub lifecycle_classification: GqlLifecycleClassification,
    pub dry_run_recommendation: Option<GqlDryRunRecommendation>,
    pub estimated_size_bytes: GqlByteCountString,
    pub last_touched_at: Option<GqlDateTime>,
    pub active_process_evidence: Option<String>,
    pub owner: Option<String>,
    pub owner_inference: Option<String>,
    pub status_token: String,
    pub generated_at: GqlDateTime,
    pub partial_errors: Vec<String>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactError", rename_fields = "camelCase")]
pub struct GqlTempArtifactError {
    pub code: GqlInventoryErrorCode,
    pub message: String,
    pub root_kind: Option<GqlRootKind>,
    pub phase: Option<String>,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactMutationGuard", rename_fields = "camelCase")]
pub struct GqlTempArtifactMutationGuard {
    pub status: GqlMutationGuardStatus,
    pub checked_at: GqlDateTime,
    pub no_delete: bool,
    pub no_prune: bool,
    pub no_chmod: bool,
    pub no_persist: bool,
    pub no_retry: bool,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactDryRunMutationGuard", rename_fields = "camelCase")]
pub struct GqlTempArtifactDryRunMutationGuard {
    pub status: GqlMutationGuardStatus,
    pub checked_at: GqlDateTime,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactDryRun", rename_fields = "camelCase")]
pub struct GqlTempArtifactDryRun {
    pub schema_version: String,
    pub generated_at: Option<GqlDateTime>,
    /// Keyed by dry_run_recommendation enum value; counts per recommendation.
    pub recommendation_counts: async_graphql::Json<serde_json::Value>,
    pub mutation_guard: GqlTempArtifactDryRunMutationGuard,
}

#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactInventory", rename_fields = "camelCase")]
pub struct GqlTempArtifactInventory {
    pub schema_version: String,
    pub status: GqlInventoryStatus,
    pub enabled_state: GqlEnabledState,
    /// Backend process-start mode; see `GqlInventoryMode` for how this differs
    /// from `enabled_state`.
    pub mode: GqlInventoryMode,
    pub disabled_reason_code: Option<String>,
    pub generated_at: GqlDateTime,
    pub limits_applied: GqlTempArtifactLimitsApplied,
    pub summary: GqlTempArtifactSummary,
    /// Rows are empty in disabled mode; populated in hidden_readback/operator_visible.
    pub rows: Vec<GqlTempArtifactRow>,
    /// Top-level errors; empty in disabled mode.
    pub errors: Vec<GqlTempArtifactError>,
    /// Dry-run result; null when include_dry_run=false.
    pub dry_run: Option<GqlTempArtifactDryRun>,
    pub mutation_guard: GqlTempArtifactMutationGuard,
}

// ── Builder helpers ───────────────────────────────────────────────────────────

pub fn build_disabled_inventory(include_dry_run: bool) -> GqlTempArtifactInventory {
    let now = GqlDateTime::now();
    let guard_status = GqlMutationGuardStatus::from(MutationGuardStatus::Skipped);
    GqlTempArtifactInventory {
        schema_version: TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION.to_string(),
        status: GqlInventoryStatus::from(InventoryStatus::Disabled),
        enabled_state: GqlEnabledState::from(EnabledState::Disabled),
        mode: GqlInventoryMode::from(InventoryMode::Disabled),
        disabled_reason_code: Some("mode_disabled".to_string()),
        generated_at: now.clone(),
        limits_applied: GqlTempArtifactLimitsApplied {
            limit: 0,
            timeout_ms: 0,
            scan_deadline_at: None,
            queue_wait_ms: 0,
        },
        summary: GqlTempArtifactSummary {
            artifact_tree_count: 0,
            estimated_bytes: GqlByteCountString("0".to_string()),
            active_or_recent_count: 0,
            terminal_candidate_count: 0,
            orphan_candidate_count: 0,
            legacy_unmanaged_count: 0,
            scan_error_count: 0,
            dry_run_candidate_count: 0,
            truncated: false,
            queue_wait_ms: 0,
        },
        rows: vec![],
        errors: vec![],
        dry_run: if include_dry_run {
            Some(GqlTempArtifactDryRun {
                schema_version: "temp_artifact_dry_run_v1".to_string(),
                generated_at: Some(now.clone()),
                recommendation_counts: async_graphql::Json(serde_json::json!({})),
                mutation_guard: GqlTempArtifactDryRunMutationGuard {
                    status: guard_status,
                    checked_at: now.clone(),
                },
            })
        } else {
            None
        },
        mutation_guard: GqlTempArtifactMutationGuard {
            status: guard_status,
            checked_at: now,
            no_delete: true,
            no_prune: true,
            no_chmod: true,
            no_persist: true,
            no_retry: true,
        },
    }
}

pub fn current_inventory_mode() -> InventoryMode {
    std::env::var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE")
        .ok()
        .and_then(|s| InventoryMode::from_env_str(&s))
        .unwrap_or(InventoryMode::Disabled)
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn canonical_empty_json() -> serde_json::Value {
        serde_json::json!({
            "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
            "status": "complete",
            "enabled_state": "enabled",
            "mode": "hidden_readback",
            "disabled_reason_code": null,
            "generated_at": "2026-01-01T00:00:00Z",
            "limits_applied": {
                "limit": 500,
                "timeout_ms": 5000,
                "scan_deadline_at": null,
                "queue_wait_ms": 0
            },
            "summary": {
                "artifact_tree_count": 0,
                "estimated_bytes": "0",
                "active_or_recent_count": 0,
                "terminal_candidate_count": 0,
                "orphan_candidate_count": 0,
                "legacy_unmanaged_count": 0,
                "scan_error_count": 0,
                "dry_run_candidate_count": 0,
                "truncated": false,
                "queue_wait_ms": 0
            },
            "rows": [],
            "errors": [],
            "dry_run": null,
            "mutation_guard": {
                "status": "pass",
                "checked_at": "2026-01-01T00:00:00Z",
                "no_delete": true,
                "no_prune": true,
                "no_chmod": true,
                "no_persist": true,
                "no_retry": true
            }
        })
    }

    fn canonical_row_json() -> serde_json::Value {
        serde_json::json!({
            "path_display": "<redacted:aaaaaaaaaaaa>",
            "path_hash": "a".repeat(64),
            "path_hash_short": "a".repeat(12),
            "correlation_key": "a".repeat(64),
            "root_kind": "run_meta_root",
            "artifact_kind": "run_output",
            "manifest_state": "unknown",
            "lifecycle_classification": "active_or_recent",
            "dry_run_recommendation": null,
            "estimated_size_bytes": "0",
            "last_touched_at": null,
            "active_process_evidence": null,
            "owner": null,
            "owner_inference": null,
            "status_token": "complete",
            "generated_at": "2026-01-01T00:00:00Z",
            "partial_errors": []
        })
    }

    #[test]
    fn p089_gql_byte_count_string_accepts_zero() {
        assert!(validate_byte_count_string("0").is_ok());
    }

    #[test]
    fn p089_gql_byte_count_string_accepts_positive() {
        assert!(validate_byte_count_string("12345").is_ok());
    }

    #[test]
    fn p089_gql_byte_count_string_accepts_over_2gb() {
        assert!(validate_byte_count_string("3000000000").is_ok());
    }

    #[test]
    fn p089_gql_byte_count_string_rejects_empty() {
        assert!(validate_byte_count_string("").is_err());
    }

    #[test]
    fn p089_gql_byte_count_string_rejects_negative() {
        assert!(validate_byte_count_string("-1").is_err());
    }

    #[test]
    fn p089_gql_byte_count_string_rejects_non_decimal() {
        assert!(validate_byte_count_string("1.5").is_err());
        assert!(validate_byte_count_string("1e9").is_err());
    }

    #[test]
    fn p089_gql_byte_count_string_rejects_leading_zero() {
        assert!(validate_byte_count_string("01").is_err());
    }

    #[test]
    fn p089_gql_disabled_inventory_has_correct_status() {
        let inv = build_disabled_inventory(true);
        assert_eq!(inv.schema_version, TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION);
        assert_eq!(inv.status, GqlInventoryStatus::Disabled);
        assert_eq!(inv.enabled_state, GqlEnabledState::Disabled);
        assert_eq!(inv.mode, GqlInventoryMode::Disabled);
        assert!(inv.disabled_reason_code.is_some());
        assert!(inv.rows.is_empty());
        assert!(inv.errors.is_empty());
    }

    #[test]
    fn p089_gql_mode_is_distinct_from_enabled_state_for_hidden_readback() {
        // Regression: `enabled_state` alone cannot distinguish hidden_readback from
        // operator_visible (both are ENABLED). Swift composes visibility from this
        // `mode` field plus its local preference, so the two must decode distinctly.
        let raw = serde_json::json!({
            "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
            "status": "complete",
            "enabled_state": "enabled",
            "mode": "hidden_readback",
            "disabled_reason_code": null,
            "generated_at": "2026-01-01T00:00:00Z",
            "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": null, "queue_wait_ms": 0},
            "summary": {"artifact_tree_count": 0, "estimated_bytes": "0", "active_or_recent_count": 0, "terminal_candidate_count": 0, "orphan_candidate_count": 0, "legacy_unmanaged_count": 0, "scan_error_count": 0, "dry_run_candidate_count": 0, "truncated": false, "queue_wait_ms": 0},
            "rows": [],
            "errors": [],
            "dry_run": null,
            "mutation_guard": {"status": "skipped", "checked_at": "2026-01-01T00:00:00Z", "no_delete": true, "no_prune": true, "no_chmod": true, "no_persist": true, "no_retry": true}
        });
        let inv = from_canonical_json(&raw);
        assert_eq!(inv.enabled_state, GqlEnabledState::Enabled);
        assert_eq!(inv.mode, GqlInventoryMode::HiddenReadback);
    }

    #[test]
    fn p089_gql_mode_operator_visible_decodes_distinctly_from_hidden_readback() {
        let mut raw = serde_json::json!({
            "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
            "status": "complete",
            "enabled_state": "enabled",
            "mode": "operator_visible",
            "disabled_reason_code": null,
            "generated_at": "2026-01-01T00:00:00Z",
            "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": null, "queue_wait_ms": 0},
            "summary": {"artifact_tree_count": 0, "estimated_bytes": "0", "active_or_recent_count": 0, "terminal_candidate_count": 0, "orphan_candidate_count": 0, "legacy_unmanaged_count": 0, "scan_error_count": 0, "dry_run_candidate_count": 0, "truncated": false, "queue_wait_ms": 0},
            "rows": [],
            "errors": [],
            "dry_run": null,
            "mutation_guard": {"status": "skipped", "checked_at": "2026-01-01T00:00:00Z", "no_delete": true, "no_prune": true, "no_chmod": true, "no_persist": true, "no_retry": true}
        });
        let inv = from_canonical_json(&raw);
        assert_eq!(inv.mode, GqlInventoryMode::OperatorVisible);

        raw["mode"] = serde_json::json!("hidden_readback");
        let inv2 = from_canonical_json(&raw);
        assert_eq!(inv2.mode, GqlInventoryMode::HiddenReadback);
        assert_ne!(inv.mode, inv2.mode);
    }

    #[test]
    fn p089_gql_missing_mode_field_defaults_to_disabled_fail_closed() {
        let raw = serde_json::json!({
            "schema_version": TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
            "status": "complete",
            "enabled_state": "enabled",
            "disabled_reason_code": null,
            "generated_at": "2026-01-01T00:00:00Z",
            "limits_applied": {"limit": 500, "timeout_ms": 5000, "scan_deadline_at": null, "queue_wait_ms": 0},
            "summary": {"artifact_tree_count": 0, "estimated_bytes": "0", "active_or_recent_count": 0, "terminal_candidate_count": 0, "orphan_candidate_count": 0, "legacy_unmanaged_count": 0, "scan_error_count": 0, "dry_run_candidate_count": 0, "truncated": false, "queue_wait_ms": 0},
            "rows": [],
            "errors": [],
            "dry_run": null,
            "mutation_guard": {"status": "skipped", "checked_at": "2026-01-01T00:00:00Z", "no_delete": true, "no_prune": true, "no_chmod": true, "no_persist": true, "no_retry": true}
        });
        let inv = from_canonical_json(&raw);
        assert_eq!(inv.status, GqlInventoryStatus::Error);
        assert_eq!(inv.enabled_state, GqlEnabledState::Unknown);
    }

    #[test]
    fn p089_gql_disabled_inventory_mutation_guard_no_delete() {
        let inv = build_disabled_inventory(true);
        assert!(inv.mutation_guard.no_delete);
        assert!(inv.mutation_guard.no_prune);
        assert!(inv.mutation_guard.no_chmod);
        assert!(inv.mutation_guard.no_persist);
        assert!(inv.mutation_guard.no_retry);
    }

    #[test]
    fn p089_gql_include_dry_run_false_gives_null() {
        let inv = build_disabled_inventory(false);
        assert!(inv.dry_run.is_none());
    }

    #[test]
    fn p089_gql_include_dry_run_true_includes_dry_run() {
        let inv = build_disabled_inventory(true);
        assert!(inv.dry_run.is_some());
        assert_eq!(
            inv.dry_run.unwrap().schema_version,
            "temp_artifact_dry_run_v1"
        );
    }

    #[test]
    fn p089_gql_summary_estimated_bytes_is_zero_string() {
        let inv = build_disabled_inventory(false);
        assert_eq!(inv.summary.estimated_bytes.0, "0");
    }

    #[test]
    fn p089_gql_mode_defaults_to_disabled() {
        std::env::remove_var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE");
        assert_eq!(current_inventory_mode(), InventoryMode::Disabled);
    }

    #[test]
    fn p089_gql_canonical_rows_and_errors_are_typed_losslessly() {
        let raw = serde_json::json!({
            "schema_version": "temp_artifact_inventory_v1",
            "status": "partial",
            "enabled_state": "enabled",
            "mode": "hidden_readback",
            "disabled_reason_code": null,
            "generated_at": "2026-07-25T10:00:00Z",
            "limits_applied": {
                "limit": 500,
                "timeout_ms": 5000,
                "scan_deadline_at": "2026-07-25T10:00:05Z",
                "queue_wait_ms": 3
            },
            "summary": {
                "artifact_tree_count": 1,
                "estimated_bytes": "3000000000",
                "active_or_recent_count": 1,
                "terminal_candidate_count": 0,
                "orphan_candidate_count": 0,
                "legacy_unmanaged_count": 0,
                "scan_error_count": 1,
                "dry_run_candidate_count": 0,
                "truncated": false,
                "queue_wait_ms": 3
            },
            "rows": [{
                "path_display": "<redacted:abcdef12>",
                "path_hash": "a".repeat(64),
                "path_hash_short": "a".repeat(12),
                "correlation_key": "a".repeat(64),
                "root_kind": "run_meta_root",
                "artifact_kind": "run_output",
                "manifest_state": "present",
                "lifecycle_classification": "active_or_recent",
                "dry_run_recommendation": "would_keep_active",
                "estimated_size_bytes": "3000000000",
                "last_touched_at": "2026-07-25T09:59:00Z",
                "active_process_evidence": null,
                "owner": "run",
                "owner_inference": "manifest",
                "status_token": "complete",
                "generated_at": "2026-07-25T10:00:00Z",
                "partial_errors": ["size_estimation_failed"]
            }],
            "errors": [{
                "code": "size_estimation_failed",
                "message": "<redacted>",
                "root_kind": "run_meta_root",
                "phase": null
            }],
            "dry_run": null,
            "mutation_guard": {
                "status": "pass",
                "checked_at": "2026-07-25T10:00:00Z",
                "no_delete": true,
                "no_prune": true,
                "no_chmod": true,
                "no_persist": true,
                "no_retry": true
            }
        });

        let projected = from_canonical_json(&raw);
        assert_eq!(projected.rows.len(), 1);
        assert_eq!(projected.rows[0].estimated_size_bytes.0, "3000000000");
        assert_eq!(
            projected.rows[0].artifact_kind.as_deref(),
            Some("run_output")
        );
        assert_eq!(
            projected.rows[0].partial_errors,
            vec!["size_estimation_failed"]
        );
        assert_eq!(projected.rows[0].root_kind, GqlRootKind::RunMetaRoot);
        assert_eq!(
            projected.rows[0].lifecycle_classification,
            GqlLifecycleClassification::ActiveOrRecent
        );
        assert_eq!(
            projected.rows[0].dry_run_recommendation,
            Some(GqlDryRunRecommendation::WouldKeepActive)
        );
        assert_eq!(projected.errors.len(), 1);
        assert_eq!(
            projected.errors[0].code,
            GqlInventoryErrorCode::SizeEstimationFailed
        );
        assert_eq!(
            projected.errors[0].root_kind,
            Some(GqlRootKind::RunMetaRoot)
        );
    }

    #[test]
    fn p089_gql_unrecognized_root_kind_falls_back_to_unknown() {
        // Externally evolvable enums: a backend value this build doesn't recognize
        // must not error the whole payload — it degrades to UNKNOWN.
        let mut raw = canonical_empty_json();
        let mut row = canonical_row_json();
        row["root_kind"] = serde_json::json!("some_future_root_kind");
        row["lifecycle_classification"] = serde_json::json!("some_future_classification");
        row["dry_run_recommendation"] = serde_json::json!("some_future_recommendation");
        raw["rows"] = serde_json::json!([row]);
        raw["errors"] = serde_json::json!([{
            "code": "some_future_error_code",
            "message": "<redacted>",
            "root_kind": null,
            "phase": null
        }]);
        let projected = from_canonical_json(&raw);
        assert_eq!(projected.rows[0].root_kind, GqlRootKind::Unknown);
        assert_eq!(
            projected.rows[0].lifecycle_classification,
            GqlLifecycleClassification::Unknown
        );
        assert_eq!(
            projected.rows[0].dry_run_recommendation,
            Some(GqlDryRunRecommendation::Unknown)
        );
        assert_eq!(projected.errors[0].code, GqlInventoryErrorCode::Unknown);
    }

    #[test]
    fn p089_gql_missing_dry_run_recommendation_is_none_not_unknown() {
        // A canonical null field is distinct from an unrecognized value.
        let mut raw = canonical_empty_json();
        raw["rows"] = serde_json::json!([canonical_row_json()]);
        let projected = from_canonical_json(&raw);
        assert_eq!(projected.rows[0].dry_run_recommendation, None);
    }

    #[test]
    fn p089_gql_from_canonical_json_fails_closed_on_malformed_summary_byte_count() {
        // Regression for the fail-open ByteCountString defect: a malformed decimal
        // string in the canonical DTO must not be silently wrapped and forwarded to
        // the GraphQL client, bypassing the scalar's own validation contract.
        let mut raw = canonical_empty_json();
        raw["summary"]["estimated_bytes"] = serde_json::json!("-5");
        let projected = from_canonical_json(&raw);
        assert_eq!(projected.status, GqlInventoryStatus::Error);
        assert!(projected.rows.is_empty());
        assert_eq!(projected.errors.len(), 1);
        assert_eq!(
            projected.errors[0].code,
            GqlInventoryErrorCode::InternalError
        );
    }

    #[test]
    fn p089_gql_from_canonical_json_fails_closed_on_malformed_row_byte_count() {
        let mut raw = canonical_empty_json();
        let mut row = canonical_row_json();
        row["estimated_size_bytes"] = serde_json::json!("01");
        raw["rows"] = serde_json::json!([row]);
        let projected = from_canonical_json(&raw);
        assert_eq!(projected.status, GqlInventoryStatus::Error);
        assert!(projected.rows.is_empty());
    }

    #[test]
    fn p089_gql_from_canonical_json_rejects_missing_required_byte_count() {
        let mut raw = canonical_empty_json();
        raw["summary"]
            .as_object_mut()
            .expect("summary object")
            .remove("estimated_bytes");
        let projected = from_canonical_json(&raw);
        assert_eq!(projected.status, GqlInventoryStatus::Error);
        assert_eq!(projected.summary.estimated_bytes.0, "0");
    }

    #[test]
    fn p089_gql_from_canonical_json_normalizes_output_datetimes_to_utc() {
        let mut raw = canonical_empty_json();
        raw["generated_at"] = serde_json::json!("2026-01-01T02:30:00+02:30");
        raw["limits_applied"]["scan_deadline_at"] = serde_json::json!("2026-01-01T02:30:05+02:30");
        let mut row = canonical_row_json();
        row["generated_at"] = serde_json::json!("2026-01-01T02:30:00+02:30");
        row["last_touched_at"] = serde_json::json!("2026-01-01T02:29:00+02:30");
        raw["rows"] = serde_json::json!([row]);

        let projected = from_canonical_json(&raw);
        assert_eq!(projected.status, GqlInventoryStatus::Complete);
        assert_eq!(projected.generated_at.0, "2026-01-01T00:00:00Z");
        assert_eq!(
            projected.limits_applied.scan_deadline_at.unwrap().0,
            "2026-01-01T00:00:05Z"
        );
        assert_eq!(
            projected.rows[0].last_touched_at.as_ref().unwrap().0,
            "2025-12-31T23:59:00Z"
        );
    }

    #[test]
    fn p089_gql_from_canonical_json_rejects_malformed_required_datetime() {
        let mut raw = canonical_empty_json();
        raw["generated_at"] = serde_json::json!("not-a-timestamp");

        let projected = from_canonical_json(&raw);
        assert_eq!(projected.status, GqlInventoryStatus::Error);
        assert_eq!(
            projected.errors[0].code,
            GqlInventoryErrorCode::InternalError
        );
    }
}
