//! P089 Managed Temporary Artifact Inventory domain types.
//!
//! Defines the canonical DTO vocabulary, mode enum, and daemon-process
//! configuration for temp artifact inventory readback. The scanner, HMAC
//! redaction key, and path-hash computation live in the graphql-server /
//! mcp-server crates; this module holds only mode-agnostic domain vocabulary.

use serde::{Deserialize, Serialize};

/// Backend operating mode for temp artifact inventory diagnostics.
/// Sourced from `CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE` at daemon start.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TempArtifactInventoryMode {
    /// All lanes return disabled disposition; no root scan occurs.
    #[default]
    Disabled,
    /// Backend APIs expose inventory readback for tests and automation;
    /// packaged app navigation hides the surface.
    HiddenReadback,
    /// Backend readback available; packaged app surface visible when the
    /// CFPreferences key `TempArtifactDiagnosticsVisible` is true.
    OperatorVisible,
}

impl TempArtifactInventoryMode {
    pub fn from_env_str(s: &str) -> Option<Self> {
        match s {
            "disabled" => Some(Self::Disabled),
            "hidden_readback" => Some(Self::HiddenReadback),
            "operator_visible" => Some(Self::OperatorVisible),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::HiddenReadback => "hidden_readback",
            Self::OperatorVisible => "operator_visible",
        }
    }

    pub fn is_disabled(self) -> bool {
        matches!(self, Self::Disabled)
    }

    pub fn allows_scan(self) -> bool {
        !self.is_disabled()
    }
}

/// Daemon-level configuration for the temp artifact inventory subsystem.
/// Populated once at daemon process start from the environment.
#[derive(Clone, Debug)]
pub struct TempArtifactInventoryConfig {
    pub mode: TempArtifactInventoryMode,
}

impl TempArtifactInventoryConfig {
    pub fn from_env() -> Self {
        let mode = std::env::var("CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE")
            .ok()
            .and_then(|v| TempArtifactInventoryMode::from_env_str(&v))
            .unwrap_or(TempArtifactInventoryMode::Disabled);
        Self { mode }
    }

    pub fn disabled() -> Self {
        Self {
            mode: TempArtifactInventoryMode::Disabled,
        }
    }
}

/// Top-level status of a temp artifact inventory response.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryStatus {
    Complete,
    Partial,
    Timeout,
    Cancelled,
    Error,
    Disabled,
    ResourceExhausted,
    Unknown,
}

impl InventoryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Complete => "complete",
            Self::Partial => "partial",
            Self::Timeout => "timeout",
            Self::Cancelled => "cancelled",
            Self::Error => "error",
            Self::Disabled => "disabled",
            Self::ResourceExhausted => "resource_exhausted",
            Self::Unknown => "unknown",
        }
    }
}

/// enabled_state field value.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnabledState {
    Enabled,
    Disabled,
    Unknown,
}

/// Lifecycle classification of a discovered artifact tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleClassification {
    ActiveOrRecent,
    TerminalCandidate,
    OrphanCandidate,
    LegacyUnmanaged,
    ScanError,
    Unknown,
}

/// Advisory dry-run recommendation for a discovered artifact tree.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DryRunRecommendation {
    WouldKeepActive,
    WouldKeepRecent,
    WouldPreserveFailureEvidence,
    WouldDeleteAfterFutureApproval,
    WouldMigrateLegacyManifestAfterFutureMigrationEnabled,
    NeedsOperatorReview,
    NoRecommendation,
    Unknown,
}

/// Root kind: what category of managed root this entry represents.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootKind {
    RunMetaRoot,
    ControlPlaneCache,
    ProviderHomeCopy,
    LegacyChainworksTmp,
    DiagnosticTestRoot,
    Unknown,
}

/// Mutation guard status: proves advisory dry-run cannot mutate.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationGuardStatus {
    Pass,
    Fail,
    Skipped,
    Unknown,
}

/// Scan error code vocabulary.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ScanErrorCode {
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

/// A `ByteCountString` is an unsigned decimal string.
///
/// Canonical contract: "0" or a string of decimal digits with no leading zeros.
/// Values greater than 2 GB (> 2_147_483_647) must remain decimal strings in
/// every external lane (GraphQL, MCP, run report, release receipt).
#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ByteCountString(pub String);

impl ByteCountString {
    /// Parse and validate. Rejects empty strings, non-digit characters,
    /// negative prefixes, leading zeros (except "0"), and whitespace.
    pub fn parse(s: impl Into<String>) -> Result<Self, String> {
        let s = s.into();
        if s.is_empty() {
            return Err("ByteCountString must not be empty".to_string());
        }
        if !s.chars().all(|c| c.is_ascii_digit()) {
            return Err(format!(
                "ByteCountString must contain only ASCII decimal digits: {:?}",
                s
            ));
        }
        if s.len() > 1 && s.starts_with('0') {
            return Err(format!(
                "ByteCountString must not have leading zeros: {:?}",
                s
            ));
        }
        Ok(Self(s))
    }

    pub fn from_u64(v: u64) -> Self {
        Self(v.to_string())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn is_zero(&self) -> bool {
        self.0 == "0"
    }
}

impl std::fmt::Display for ByteCountString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Build the canonical disabled-mode inventory readback JSON.
///
/// Called by GraphQL, MCP, run report, and release receipt lanes when
/// `TempArtifactInventoryMode::Disabled` is in effect. No scanning occurs.
pub fn disabled_inventory_response(disabled_reason_code: Option<&str>) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "temp_artifact_inventory_v1",
        "status": "disabled",
        "enabled_state": "disabled",
        "disabled_reason_code": disabled_reason_code,
        "generated_at": null,
        "limits_applied": null,
        "summary": null,
        "rows": [],
        "errors": [],
        "dry_run": null,
        "mutation_guard": {
            "status": "skipped",
            "reason": "disabled_mode"
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p089_temp_inventory_mode_roundtrip() {
        for (s, expected) in [
            ("disabled", TempArtifactInventoryMode::Disabled),
            ("hidden_readback", TempArtifactInventoryMode::HiddenReadback),
            (
                "operator_visible",
                TempArtifactInventoryMode::OperatorVisible,
            ),
        ] {
            let parsed = TempArtifactInventoryMode::from_env_str(s);
            assert_eq!(parsed, Some(expected), "mode parse {s:?}");
            assert_eq!(expected.as_str(), s, "mode as_str {s:?}");
        }
    }

    #[test]
    fn p089_temp_inventory_mode_unknown_returns_none() {
        assert_eq!(TempArtifactInventoryMode::from_env_str("DISABLED"), None);
        assert_eq!(TempArtifactInventoryMode::from_env_str(""), None);
        assert_eq!(TempArtifactInventoryMode::from_env_str("enabled"), None);
    }

    #[test]
    fn p089_temp_inventory_mode_disabled_no_scan() {
        assert!(TempArtifactInventoryMode::Disabled.is_disabled());
        assert!(!TempArtifactInventoryMode::Disabled.allows_scan());
        assert!(!TempArtifactInventoryMode::HiddenReadback.is_disabled());
        assert!(TempArtifactInventoryMode::HiddenReadback.allows_scan());
    }

    #[test]
    fn p089_byte_count_string_valid() {
        assert_eq!(ByteCountString::parse("0").unwrap().as_str(), "0");
        assert_eq!(ByteCountString::parse("1").unwrap().as_str(), "1");
        assert_eq!(
            ByteCountString::parse("4294967296").unwrap().as_str(),
            "4294967296"
        );
        // Over 2 GB — must remain a decimal string
        assert_eq!(
            ByteCountString::parse("2147483648").unwrap().as_str(),
            "2147483648"
        );
        assert_eq!(
            ByteCountString::parse("999999999999999").unwrap().as_str(),
            "999999999999999"
        );
    }

    #[test]
    fn p089_byte_count_string_rejects_invalid() {
        assert!(ByteCountString::parse("").is_err(), "empty string");
        assert!(ByteCountString::parse("-1").is_err(), "negative");
        assert!(ByteCountString::parse("01").is_err(), "leading zero");
        assert!(ByteCountString::parse("1.5").is_err(), "decimal point");
        assert!(ByteCountString::parse("1e9").is_err(), "scientific");
        assert!(ByteCountString::parse(" 1").is_err(), "leading space");
        assert!(ByteCountString::parse("1 ").is_err(), "trailing space");
        assert!(ByteCountString::parse("0x10").is_err(), "hex prefix");
    }

    #[test]
    fn p089_byte_count_string_from_u64() {
        assert_eq!(ByteCountString::from_u64(0).as_str(), "0");
        assert_eq!(
            ByteCountString::from_u64(u64::MAX).as_str(),
            "18446744073709551615"
        );
    }

    #[test]
    fn p089_disabled_inventory_response_schema() {
        let resp = disabled_inventory_response(None);
        assert_eq!(resp["schema_version"], "temp_artifact_inventory_v1");
        assert_eq!(resp["status"], "disabled");
        assert_eq!(resp["enabled_state"], "disabled");
        assert!(resp["rows"].as_array().unwrap().is_empty());
        assert!(resp["errors"].as_array().unwrap().is_empty());
        assert!(resp["generated_at"].is_null());
        assert!(resp["dry_run"].is_null());
        assert_eq!(resp["mutation_guard"]["status"], "skipped");
    }

    #[test]
    fn p089_disabled_inventory_response_with_reason() {
        let resp = disabled_inventory_response(Some("mode_disabled"));
        assert_eq!(resp["disabled_reason_code"], "mode_disabled");
    }
}
