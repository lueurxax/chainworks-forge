//! P089 Managed Temporary Artifact Inventory GraphQL types.
//!
//! The GraphQL layer is a lossless camelCase projection of the canonical
//! snake_case domain DTO. All byte counts are ByteCountString scalars;
//! all timestamps use the async-graphql built-in DateTime scalar (RFC3339).
//! Enum values here mirror the bounded domain vocabulary exactly.

use async_graphql::*;

/// P089 custom scalar: unsigned decimal byte-count string.
///
/// Accepts "0" or a non-empty string of ASCII decimal digits with no leading
/// zeros. Rejects numeric values, negative strings, leading-zero strings,
/// empty strings, and whitespace. Values greater than 2 GB remain decimal
/// strings in every external lane.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ByteCountString(pub String);

#[Scalar(name = "ByteCountString")]
impl ScalarType for ByteCountString {
    fn parse(value: Value) -> InputValueResult<Self> {
        match value {
            Value::String(s) => domain::temp_artifact_inventory::ByteCountString::parse(&s)
                .map(|b| ByteCountString(b.0))
                .map_err(|e| InputValueError::custom(e)),
            _ => Err(InputValueError::expected_type(value)),
        }
    }

    fn to_value(&self) -> Value {
        Value::String(self.0.clone())
    }
}

/// P089 inventory status: outcome of the scan request.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "TempInventoryStatus")]
pub enum GqlInventoryStatus {
    #[graphql(name = "COMPLETE")]
    Complete,
    #[graphql(name = "PARTIAL")]
    Partial,
    #[graphql(name = "TIMEOUT")]
    Timeout,
    #[graphql(name = "CANCELLED")]
    Cancelled,
    #[graphql(name = "ERROR")]
    Error,
    #[graphql(name = "DISABLED")]
    Disabled,
    #[graphql(name = "RESOURCE_EXHAUSTED")]
    ResourceExhausted,
    /// Additive-unknown sentinel — new values decode here.
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

/// P089 enabled_state: whether the subsystem is enabled, disabled, or unknown.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "TempInventoryEnabledState")]
pub enum GqlEnabledState {
    #[graphql(name = "ENABLED")]
    Enabled,
    #[graphql(name = "DISABLED")]
    Disabled,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

/// P089 lifecycle classification of a discovered artifact tree.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "LifecycleClassification")]
pub enum GqlLifecycleClassification {
    #[graphql(name = "ACTIVE_OR_RECENT")]
    ActiveOrRecent,
    #[graphql(name = "TERMINAL_CANDIDATE")]
    TerminalCandidate,
    #[graphql(name = "ORPHAN_CANDIDATE")]
    OrphanCandidate,
    #[graphql(name = "LEGACY_UNMANAGED")]
    LegacyUnmanaged,
    #[graphql(name = "SCAN_ERROR")]
    ScanError,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

/// P089 advisory dry-run recommendation.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "DryRunRecommendation")]
pub enum GqlDryRunRecommendation {
    #[graphql(name = "WOULD_KEEP_ACTIVE")]
    WouldKeepActive,
    #[graphql(name = "WOULD_KEEP_RECENT")]
    WouldKeepRecent,
    #[graphql(name = "WOULD_PRESERVE_FAILURE_EVIDENCE")]
    WouldPreserveFailureEvidence,
    #[graphql(name = "WOULD_DELETE_AFTER_FUTURE_APPROVAL")]
    WouldDeleteAfterFutureApproval,
    #[graphql(name = "WOULD_MIGRATE_LEGACY_MANIFEST_AFTER_FUTURE_MIGRATION_ENABLED")]
    WouldMigrateLegacyManifestAfterFutureMigrationEnabled,
    #[graphql(name = "NEEDS_OPERATOR_REVIEW")]
    NeedsOperatorReview,
    #[graphql(name = "NO_RECOMMENDATION")]
    NoRecommendation,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

/// P089 root kind: category of managed root.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "TempArtifactRootKind")]
pub enum GqlRootKind {
    #[graphql(name = "RUN_META_ROOT")]
    RunMetaRoot,
    #[graphql(name = "CONTROL_PLANE_CACHE")]
    ControlPlaneCache,
    #[graphql(name = "PROVIDER_HOME_COPY")]
    ProviderHomeCopy,
    #[graphql(name = "LEGACY_CHAINWORKS_TMP")]
    LegacyChainworksTmp,
    #[graphql(name = "DIAGNOSTIC_TEST_ROOT")]
    DiagnosticTestRoot,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

/// P089 mutation guard status.
#[derive(Enum, Copy, Clone, Eq, PartialEq, Debug)]
#[graphql(name = "MutationGuardStatus")]
pub enum GqlMutationGuardStatus {
    #[graphql(name = "PASS")]
    Pass,
    #[graphql(name = "FAIL")]
    Fail,
    #[graphql(name = "SKIPPED")]
    Skipped,
    #[graphql(name = "UNKNOWN")]
    Unknown,
}

/// P089 mutation guard summary in the top-level inventory response.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempInventoryMutationGuard", rename_fields = "camelCase")]
pub struct GqlMutationGuard {
    pub status: GqlMutationGuardStatus,
    pub reason: Option<String>,
}

/// P089 limits applied to this scan request.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempInventoryLimitsApplied", rename_fields = "camelCase")]
pub struct GqlLimitsApplied {
    pub limit: i32,
    pub timeout_ms: i32,
    pub include_dry_run: bool,
    /// ISO-8601 deadline for the scan (null when disabled).
    pub scan_deadline_at: Option<chrono::DateTime<chrono::Utc>>,
    pub queue_wait_ms: Option<i64>,
}

/// P089 scan-level error record.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempInventoryScanError", rename_fields = "camelCase")]
pub struct GqlScanError {
    pub error_code: String,
    pub redacted_message: String,
    pub root_kind: Option<GqlRootKind>,
}

/// P089 inventory summary counters.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempInventorySummary", rename_fields = "camelCase")]
pub struct GqlInventorySummary {
    pub artifact_tree_count: i32,
    pub estimated_bytes: ByteCountString,
    pub active_or_recent_count: i32,
    pub terminal_candidate_count: i32,
    pub orphan_candidate_count: i32,
    pub legacy_unmanaged_count: i32,
    pub scan_error_count: i32,
    pub dry_run_candidate_count: Option<i32>,
    pub truncated: bool,
}

/// P089 advisory dry-run summary (null when includeDryRun=false).
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempInventoryDryRun", rename_fields = "camelCase")]
pub struct GqlDryRunSummary {
    pub generated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub mutation_guard: GqlMutationGuard,
    /// Aggregate recommendation for the dry-run (null when no candidates).
    pub top_recommendation: Option<GqlDryRunRecommendation>,
    /// Recommendation counts by value.
    pub recommendation_counts: serde_json::Value,
}

/// P089 top-level temp artifact inventory response.
///
/// In disabled mode, only `status`, `enabledState`, `disabledReasonCode`,
/// `rows`, `errors`, `mutationGuard`, and `schemaVersion` are populated.
#[derive(SimpleObject, Debug, Clone)]
#[graphql(name = "TempArtifactInventory", rename_fields = "camelCase")]
pub struct GqlTempArtifactInventory {
    pub schema_version: String,
    pub status: GqlInventoryStatus,
    pub enabled_state: GqlEnabledState,
    pub disabled_reason_code: Option<String>,
    pub generated_at: Option<chrono::DateTime<chrono::Utc>>,
    pub limits_applied: Option<GqlLimitsApplied>,
    pub summary: Option<GqlInventorySummary>,
    pub errors: Vec<GqlScanError>,
    pub dry_run: Option<GqlDryRunSummary>,
    pub mutation_guard: GqlMutationGuard,
}

impl GqlTempArtifactInventory {
    /// Build the canonical disabled-mode response for GraphQL.
    pub fn disabled(disabled_reason_code: Option<&str>) -> Self {
        Self {
            schema_version: "temp_artifact_inventory_v1".to_string(),
            status: GqlInventoryStatus::Disabled,
            enabled_state: GqlEnabledState::Disabled,
            disabled_reason_code: disabled_reason_code.map(|s| s.to_string()),
            generated_at: None,
            limits_applied: None,
            summary: None,
            errors: vec![],
            dry_run: None,
            mutation_guard: GqlMutationGuard {
                status: GqlMutationGuardStatus::Skipped,
                reason: Some("disabled_mode".to_string()),
            },
        }
    }
}

/// Input for the `tempArtifactInventory` query.
#[derive(InputObject, Debug)]
#[graphql(name = "TempArtifactInventoryInput")]
pub struct GqlTempArtifactInventoryInput {
    /// Exactly one of runId or workspaceContext must be provided.
    pub run_id: Option<String>,
    /// Maximum rows to return: 0 through 500 (no clamping).
    #[graphql(default = 500)]
    pub limit: i32,
    /// Total request timeout in ms (1–5000); queue wait included.
    #[graphql(default = 5000)]
    pub timeout_ms: i32,
    /// When false, dry_run is null and rows carry no dry_run_recommendation.
    #[graphql(default = true)]
    pub include_dry_run: bool,
    /// Authorized diagnostic test-root override (diagnostic test-root mode only).
    pub test_root_override: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p089_graphql_byte_count_string_scalar_accepts_valid() {
        let v = <ByteCountString as ScalarType>::parse(Value::String("0".to_string())).unwrap();
        assert_eq!(v.0, "0");

        let v = <ByteCountString as ScalarType>::parse(Value::String("2147483648".to_string()))
            .unwrap();
        assert_eq!(v.0, "2147483648");

        let v = <ByteCountString as ScalarType>::parse(Value::String(
            "18446744073709551615".to_string(),
        ))
        .unwrap();
        assert_eq!(v.0, "18446744073709551615");
    }

    #[test]
    fn p089_graphql_byte_count_string_scalar_rejects_invalid() {
        assert!(<ByteCountString as ScalarType>::parse(Value::String("".to_string())).is_err());
        assert!(<ByteCountString as ScalarType>::parse(Value::String("-1".to_string())).is_err());
        assert!(<ByteCountString as ScalarType>::parse(Value::String("01".to_string())).is_err());
        assert!(<ByteCountString as ScalarType>::parse(Value::Number(1.into())).is_err());
        assert!(<ByteCountString as ScalarType>::parse(Value::String("1.5".to_string())).is_err());
    }

    #[test]
    fn p089_graphql_byte_count_string_to_value() {
        let b = ByteCountString("123".to_string());
        assert_eq!(
            async_graphql::ScalarType::to_value(&b),
            Value::String("123".to_string())
        );
    }

    #[test]
    fn p089_graphql_disabled_inventory_response_fields() {
        let r = GqlTempArtifactInventory::disabled(None);
        assert_eq!(r.schema_version, "temp_artifact_inventory_v1");
        assert!(matches!(r.status, GqlInventoryStatus::Disabled));
        assert!(matches!(r.enabled_state, GqlEnabledState::Disabled));
        assert!(r.disabled_reason_code.is_none());
        assert!(r.generated_at.is_none());
        assert!(r.limits_applied.is_none());
        assert!(r.summary.is_none());
        assert!(r.errors.is_empty());
        assert!(r.dry_run.is_none());
        assert!(matches!(
            r.mutation_guard.status,
            GqlMutationGuardStatus::Skipped
        ));
    }

    #[test]
    fn p089_graphql_disabled_inventory_response_with_reason() {
        let r = GqlTempArtifactInventory::disabled(Some("mode_disabled"));
        assert_eq!(r.disabled_reason_code.as_deref(), Some("mode_disabled"));
    }

    #[test]
    fn p089_graphql_temp_artifact_inventory_sdl_contains_types() {
        use async_graphql::Schema;
        struct Query;
        #[Object]
        impl Query {
            async fn temp_artifact_inventory(
                &self,
                _input: GqlTempArtifactInventoryInput,
            ) -> GqlTempArtifactInventory {
                GqlTempArtifactInventory::disabled(None)
            }
        }
        struct Mutation;
        #[Object]
        impl Mutation {
            async fn _noop(&self) -> bool {
                false
            }
        }
        let schema = Schema::build(Query, Mutation, async_graphql::EmptySubscription).finish();
        let sdl = schema.sdl();
        assert!(
            sdl.contains("ByteCountString"),
            "SDL must contain ByteCountString scalar"
        );
        assert!(
            sdl.contains("TempArtifactInventory"),
            "SDL must contain TempArtifactInventory type"
        );
        assert!(
            sdl.contains("TempInventoryStatus"),
            "SDL must contain TempInventoryStatus enum"
        );
        assert!(
            sdl.contains("DryRunRecommendation"),
            "SDL must contain DryRunRecommendation enum"
        );
        assert!(
            sdl.contains("UNKNOWN"),
            "SDL enum must contain UNKNOWN additive sentinel"
        );
    }
}
