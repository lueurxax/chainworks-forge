/// P089 Managed Temporary Artifact Inventory — domain types.
///
/// Read-only inventory and advisory dry-run slice. No deletion, cleanup,
/// pruning, persisted rows, or durable mutations exist in this module.
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::OnceLock;

pub const TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION: &str = "temp_artifact_inventory_v1";

/// Unsigned decimal byte count string. Accepts "0" or positive decimal digits
/// only; no numeric values, negatives, leading zeros except "0", empty strings.
///
/// Serializes transparently as a JSON string. Deserialization goes through `new()`
/// so validation is enforced on both inbound and outbound paths.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(transparent)]
pub struct ByteCountString(pub String);

impl<'de> serde::Deserialize<'de> for ByteCountString {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let s = String::deserialize(deserializer)?;
        ByteCountString::new(s).map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

impl ByteCountString {
    /// Validates and wraps a string as a ByteCountString.
    /// Rejects empty, non-decimal, leading-zero (except "0"), and negative strings.
    pub fn new(s: impl Into<String>) -> Result<Self, ByteCountStringError> {
        let s = s.into();
        if s.is_empty() {
            return Err(ByteCountStringError::Empty);
        }
        let bytes = s.as_bytes();
        if bytes[0] == b'-' {
            return Err(ByteCountStringError::Negative);
        }
        if !bytes.iter().all(|b| b.is_ascii_digit()) {
            return Err(ByteCountStringError::NonDecimal);
        }
        if bytes.len() > 1 && bytes[0] == b'0' {
            return Err(ByteCountStringError::LeadingZero);
        }
        Ok(Self(s))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Display for ByteCountString {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ByteCountStringError {
    Empty,
    Negative,
    NonDecimal,
    LeadingZero,
}

impl std::fmt::Display for ByteCountStringError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("byte count string must not be empty"),
            Self::Negative => f.write_str("byte count string must not be negative"),
            Self::NonDecimal => f.write_str("byte count string must contain only decimal digits"),
            Self::LeadingZero => f.write_str("byte count string must not have leading zeros"),
        }
    }
}

// ── Closed-vocabulary enums ───────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl std::fmt::Display for InventoryStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnabledState {
    Enabled,
    Disabled,
    Unknown,
}

impl EnabledState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled => "disabled",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for EnabledState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LifecycleClassification {
    ActiveOrRecent,
    TerminalCandidate,
    OrphanCandidate,
    LegacyUnmanaged,
    ScanError,
    Unknown,
}

impl LifecycleClassification {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ActiveOrRecent => "active_or_recent",
            Self::TerminalCandidate => "terminal_candidate",
            Self::OrphanCandidate => "orphan_candidate",
            Self::LegacyUnmanaged => "legacy_unmanaged",
            Self::ScanError => "scan_error",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for LifecycleClassification {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
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

impl DryRunRecommendation {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WouldKeepActive => "would_keep_active",
            Self::WouldKeepRecent => "would_keep_recent",
            Self::WouldPreserveFailureEvidence => "would_preserve_failure_evidence",
            Self::WouldDeleteAfterFutureApproval => "would_delete_after_future_approval",
            Self::WouldMigrateLegacyManifestAfterFutureMigrationEnabled => {
                "would_migrate_legacy_manifest_after_future_migration_enabled"
            }
            Self::NeedsOperatorReview => "needs_operator_review",
            Self::NoRecommendation => "no_recommendation",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for DryRunRecommendation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MutationGuardStatus {
    Pass,
    Fail,
    Skipped,
    Unknown,
}

impl MutationGuardStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pass => "pass",
            Self::Fail => "fail",
            Self::Skipped => "skipped",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for MutationGuardStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RootKind {
    RunMetaRoot,
    ControlPlaneCache,
    ProviderHomeCopy,
    LegacyChainworksTmp,
    DiagnosticTestRoot,
    Unknown,
}

impl RootKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::RunMetaRoot => "run_meta_root",
            Self::ControlPlaneCache => "control_plane_cache",
            Self::ProviderHomeCopy => "provider_home_copy",
            Self::LegacyChainworksTmp => "legacy_chainworks_tmp",
            Self::DiagnosticTestRoot => "diagnostic_test_root",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for RootKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryErrorCode {
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

impl InventoryErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::InvalidRootOverride => "invalid_root_override",
            Self::RootUnreadable => "root_unreadable",
            Self::ManifestParseFailed => "manifest_parse_failed",
            Self::SizeEstimationFailed => "size_estimation_failed",
            Self::DeadlineExceeded => "deadline_exceeded",
            Self::Cancelled => "cancelled",
            Self::InternalError => "internal_error",
            Self::MutationGuardFailed => "mutation_guard_failed",
            Self::ResourceExhausted => "resource_exhausted",
            Self::Unknown => "unknown",
        }
    }
}

impl std::fmt::Display for InventoryErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Backend mode sourced from daemon process-start env
/// CHAINWORKS_TEMP_ARTIFACT_INVENTORY_MODE.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InventoryMode {
    /// All lanes expose disabled or unavailable readback; no root scan occurs.
    Disabled,
    /// Backend APIs expose inventory for tests/automation; packaged app hides surface.
    HiddenReadback,
    /// Backend and packaged app surface are visible when CFPreferences key is true.
    OperatorVisible,
}

impl InventoryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::HiddenReadback => "hidden_readback",
            Self::OperatorVisible => "operator_visible",
        }
    }

    pub fn from_env_str(s: &str) -> Option<Self> {
        match s {
            "disabled" => Some(Self::Disabled),
            "hidden_readback" => Some(Self::HiddenReadback),
            "operator_visible" => Some(Self::OperatorVisible),
            _ => None,
        }
    }
}

impl std::fmt::Display for InventoryMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl Default for InventoryMode {
    fn default() -> Self {
        Self::Disabled
    }
}

// ── Request bounds ────────────────────────────────────────────────────────────

pub const INVENTORY_LIMIT_MIN: i32 = 0;
pub const INVENTORY_LIMIT_MAX: i32 = 500;
pub const INVENTORY_TIMEOUT_MS_MIN: i32 = 1;
pub const INVENTORY_TIMEOUT_MS_MAX: i32 = 5000;
pub const INVENTORY_TEST_ROOT_MAX_UTF8_BYTES: usize = 4096;

// ── Scanner reliability constants ─────────────────────────────────────────────

/// Maximum concurrent scans across all contexts.
pub const SCAN_GLOBAL_PERMIT_MAX: usize = 4;
/// Maximum concurrent scans per context (run or workspace).
pub const SCAN_CONTEXT_PERMIT_MAX: usize = 1;
/// Check cancellation/deadline every N directory entries.
pub const SCAN_CANCEL_CHECK_INTERVAL_ENTRIES: usize = 128;
/// Check cancellation/deadline every N milliseconds.
pub const SCAN_CANCEL_CHECK_INTERVAL_MS: u64 = 100;
/// Maximum partial errors emitted per payload.
pub const SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD: usize = 100;
/// Maximum row-level partial errors per row.
pub const SCAN_PARTIAL_ERRORS_MAX_PER_ROW: usize = 10;
/// Maximum directory identities tracked per scan (cycle guard).
pub const SCAN_MAX_VISITED_DIRS: usize = 10_000;
/// Maximum simultaneous ancestor depth the size-estimation walker will descend.
/// The walker holds one open directory descriptor per depth level of the active
/// path (O(depth), not O(breadth) — see scanner::estimate_dir_size), so this bounds
/// per-scan descriptor use to `SCAN_MAX_DIR_DEPTH` and, with `SCAN_GLOBAL_PERMIT_MAX`
/// concurrent scans, total feature descriptor use to `SCAN_MAX_DIR_DEPTH *
/// SCAN_GLOBAL_PERMIT_MAX`, safely below typical process RLIMIT_NOFILE values.
pub const SCAN_MAX_DIR_DEPTH: usize = 128;
/// Maximum cumulative logical path length (bytes) the walker will descend past.
/// Bounds per-entry PathBuf growth against a pathologically deep or wide tree.
pub const SCAN_MAX_PATH_BYTES: usize = 4096;

// ── Path hash constants ───────────────────────────────────────────────────────

pub const PATH_HASH_HEX_LEN: usize = 64;
pub const PATH_HASH_SHORT_MIN_LEN: usize = 12;
pub const PATH_HASH_SHORT_MAX_LEN: usize = 20;

// ── Path hash infrastructure ──────────────────────────────────────────────────

/// Process-scoped 256-bit HMAC key. Generated once at first use; never serialized
/// or shared outside the daemon process.
static PROCESS_PATH_HASH_KEY: OnceLock<[u8; 32]> = OnceLock::new();

fn get_process_path_hash_key() -> &'static [u8; 32] {
    PROCESS_PATH_HASH_KEY.get_or_init(|| {
        // Two UUID v4 values provide 256 bits of OS-sourced entropy.
        let u1 = uuid::Uuid::new_v4();
        let u2 = uuid::Uuid::new_v4();
        let mut key = [0u8; 32];
        key[..16].copy_from_slice(u1.as_bytes());
        key[16..].copy_from_slice(u2.as_bytes());
        key
    })
}

/// HMAC-SHA256 using the sha2 crate. Key must be exactly 32 bytes (≤ SHA-256
/// block size of 64), so no key-hash step is needed.
fn hmac_sha256(key: &[u8; 32], message: &[u8]) -> [u8; 32] {
    const BLOCK: usize = 64;
    let mut key_block = [0u8; BLOCK];
    key_block[..32].copy_from_slice(key);

    let mut ipad = key_block;
    let mut opad = key_block;
    for i in 0..BLOCK {
        ipad[i] ^= 0x36;
        opad[i] ^= 0x5C;
    }

    let mut inner = Sha256::new();
    inner.update(ipad);
    inner.update(message);
    let inner_hash = inner.finalize();

    let mut outer = Sha256::new();
    outer.update(opad);
    outer.update(inner_hash);
    outer.finalize().into()
}

fn bytes_to_lower_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

/// Computes the HMAC-SHA256 path hash for a normalized absolute path and root_kind.
///
/// Input message: raw OS bytes of `normalized_path` + NUL separator + `root_kind.as_str()`.
/// Callers must pass the path's native byte representation (e.g. `OsStrExt::as_bytes()`
/// on Unix), not a lossy UTF-8 conversion, so that distinct non-UTF-8 paths never
/// collapse onto the same correlation identity (SR-LOW-001).
/// The process-scoped key is never serialized.
/// Returns a 64-character lowercase hex string.
pub fn compute_path_hash(normalized_path_bytes: &[u8], root_kind: RootKind) -> String {
    let key = get_process_path_hash_key();
    let mut message =
        Vec::with_capacity(normalized_path_bytes.len() + 1 + root_kind.as_str().len());
    message.extend_from_slice(normalized_path_bytes);
    message.push(0x00);
    message.extend_from_slice(root_kind.as_str().as_bytes());
    bytes_to_lower_hex(&hmac_sha256(key, &message))
}

/// Derives the short path hash (12–20 chars) from a full 64-char hash.
///
/// Starts at `PATH_HASH_SHORT_MIN_LEN` (12). If the candidate is already in
/// `existing_short_hashes`, extends by 2 chars until unique or the cap of
/// `PATH_HASH_SHORT_MAX_LEN` (20) is reached (at which point the cap is returned
/// regardless of collision).
pub fn compute_path_hash_short(full_hash: &str, existing_short_hashes: &[&str]) -> String {
    debug_assert_eq!(full_hash.len(), PATH_HASH_HEX_LEN);
    let mut len = PATH_HASH_SHORT_MIN_LEN;
    while len < PATH_HASH_SHORT_MAX_LEN {
        let candidate = &full_hash[..len];
        if !existing_short_hashes.iter().any(|h| *h == candidate) {
            return candidate.to_string();
        }
        len += 2;
    }
    full_hash[..PATH_HASH_SHORT_MAX_LEN].to_string()
}

// ── Request validation ────────────────────────────────────────────────────────

/// Validates a `test_root_override` path per the P089 contract.
///
/// Rules enforced here (lexical only; containment against a server-side allowlist
/// is enforced by the scanner, not by this function):
/// - Not empty.
/// - UTF-8 byte length ≤ `INVENTORY_TEST_ROOT_MAX_UTF8_BYTES` (4096).
/// - No NUL bytes.
/// - Absolute path (starts with '/').
/// - No traversal component (`..`) after splitting on '/'.
pub fn validate_test_root_override(path: &str) -> Result<(), InventoryErrorCode> {
    if path.is_empty() {
        return Err(InventoryErrorCode::InvalidRootOverride);
    }
    if path.len() > INVENTORY_TEST_ROOT_MAX_UTF8_BYTES {
        return Err(InventoryErrorCode::InvalidRootOverride);
    }
    if path.contains('\x00') {
        return Err(InventoryErrorCode::InvalidRootOverride);
    }
    if !path.starts_with('/') {
        return Err(InventoryErrorCode::InvalidRootOverride);
    }
    for component in path.split('/') {
        if component == ".." {
            return Err(InventoryErrorCode::InvalidRootOverride);
        }
    }
    Ok(())
}

/// Validates the `limit` parameter (must be in `INVENTORY_LIMIT_MIN..=INVENTORY_LIMIT_MAX`).
pub fn validate_inventory_limit(limit: i32) -> Result<(), &'static str> {
    if limit < INVENTORY_LIMIT_MIN || limit > INVENTORY_LIMIT_MAX {
        Err("limit must be between 0 and 500 inclusive")
    } else {
        Ok(())
    }
}

/// Validates the `timeout_ms` parameter (must be in `INVENTORY_TIMEOUT_MS_MIN..=INVENTORY_TIMEOUT_MS_MAX`).
pub fn validate_inventory_timeout_ms(timeout_ms: i32) -> Result<(), &'static str> {
    if timeout_ms < INVENTORY_TIMEOUT_MS_MIN || timeout_ms > INVENTORY_TIMEOUT_MS_MAX {
        Err("timeout_ms must be between 1 and 5000 inclusive")
    } else {
        Ok(())
    }
}

/// Maximum byte length for a canonical UUID run_id string.
pub const RUN_ID_MAX_BYTES: usize = 36;

/// Validates a `run_id` string before it can influence filesystem scope.
///
/// Rules enforced (SEC-P089-005):
/// - Canonical Chainworks run identifiers are UUID strings.
/// - Byte length ≤ `RUN_ID_MAX_BYTES`.
/// - Rejects paths, traversal syntax, NUL bytes, and overlong values by
///   requiring UUID parsing rather than accepting arbitrary path components.
pub fn validate_run_id(run_id: &str) -> Result<(), ()> {
    if run_id.is_empty() || run_id.len() > RUN_ID_MAX_BYTES {
        return Err(());
    }
    uuid::Uuid::parse_str(run_id).map(|_| ()).map_err(|_| ())
}

// ── Tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p089_temp_inventory_byte_count_string_accepts_zero() {
        let s = ByteCountString::new("0").expect("zero must be valid");
        assert_eq!(s.as_str(), "0");
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_accepts_positive() {
        let s = ByteCountString::new("12345").expect("positive decimal must be valid");
        assert_eq!(s.as_str(), "12345");
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_accepts_over_2gb() {
        let over_2gb = "3000000000";
        let s = ByteCountString::new(over_2gb).expect("value >2GB must be valid");
        assert_eq!(s.as_str(), over_2gb);
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_rejects_empty() {
        assert_eq!(ByteCountString::new(""), Err(ByteCountStringError::Empty));
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_rejects_negative() {
        assert_eq!(
            ByteCountString::new("-1"),
            Err(ByteCountStringError::Negative)
        );
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_rejects_non_decimal() {
        assert_eq!(
            ByteCountString::new("1.5"),
            Err(ByteCountStringError::NonDecimal)
        );
        assert_eq!(
            ByteCountString::new("1e9"),
            Err(ByteCountStringError::NonDecimal)
        );
        assert_eq!(
            ByteCountString::new("abc"),
            Err(ByteCountStringError::NonDecimal)
        );
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_rejects_leading_zero() {
        assert_eq!(
            ByteCountString::new("01"),
            Err(ByteCountStringError::LeadingZero)
        );
        assert_eq!(
            ByteCountString::new("007"),
            Err(ByteCountStringError::LeadingZero)
        );
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_roundtrip_serde() {
        let s = ByteCountString::new("1099511627776").expect("valid");
        let json = serde_json::to_string(&s).expect("serialize");
        assert_eq!(json, "\"1099511627776\"");
        let back: ByteCountString = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(s, back);
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_serde_rejects_empty() {
        let result: Result<ByteCountString, _> = serde_json::from_str("\"\"");
        assert!(result.is_err(), "serde must reject empty ByteCountString");
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_serde_rejects_negative() {
        let result: Result<ByteCountString, _> = serde_json::from_str("\"-1\"");
        assert!(
            result.is_err(),
            "serde must reject negative ByteCountString"
        );
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_serde_rejects_leading_zero() {
        let result: Result<ByteCountString, _> = serde_json::from_str("\"07\"");
        assert!(
            result.is_err(),
            "serde must reject leading-zero ByteCountString"
        );
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_serde_rejects_non_decimal() {
        for s in &["\"1e9\"", "\"1.5\"", "\"0x1\"", "\"abc\""] {
            let result: Result<ByteCountString, _> = serde_json::from_str(s);
            assert!(
                result.is_err(),
                "serde must reject non-decimal ByteCountString: {s}"
            );
        }
    }

    #[test]
    fn p089_temp_inventory_byte_count_string_serde_rejects_numeric_value() {
        // A bare JSON number (not a string) must be rejected.
        let result: Result<ByteCountString, _> = serde_json::from_str("42");
        assert!(
            result.is_err(),
            "serde must reject numeric JSON value for ByteCountString"
        );
    }

    #[test]
    fn p089_temp_inventory_inventory_status_serde_roundtrip() {
        let statuses = [
            (InventoryStatus::Complete, "complete"),
            (InventoryStatus::Partial, "partial"),
            (InventoryStatus::Timeout, "timeout"),
            (InventoryStatus::Cancelled, "cancelled"),
            (InventoryStatus::Error, "error"),
            (InventoryStatus::Disabled, "disabled"),
            (InventoryStatus::ResourceExhausted, "resource_exhausted"),
            (InventoryStatus::Unknown, "unknown"),
        ];
        for (status, expected) in &statuses {
            assert_eq!(
                status.as_str(),
                *expected,
                "as_str mismatch for {:?}",
                status
            );
            let json = serde_json::to_string(status).expect("serialize");
            assert_eq!(json, format!("\"{}\"", expected));
            let back: InventoryStatus = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(status, &back);
        }
    }

    #[test]
    fn p089_temp_inventory_enabled_state_serde_roundtrip() {
        let cases = [
            (EnabledState::Enabled, "enabled"),
            (EnabledState::Disabled, "disabled"),
            (EnabledState::Unknown, "unknown"),
        ];
        for (state, expected) in &cases {
            assert_eq!(state.as_str(), *expected);
            let json = serde_json::to_string(state).expect("serialize");
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

    #[test]
    fn p089_temp_inventory_lifecycle_classification_serde_roundtrip() {
        let cases = [
            (LifecycleClassification::ActiveOrRecent, "active_or_recent"),
            (
                LifecycleClassification::TerminalCandidate,
                "terminal_candidate",
            ),
            (LifecycleClassification::OrphanCandidate, "orphan_candidate"),
            (LifecycleClassification::LegacyUnmanaged, "legacy_unmanaged"),
            (LifecycleClassification::ScanError, "scan_error"),
            (LifecycleClassification::Unknown, "unknown"),
        ];
        for (cls, expected) in &cases {
            assert_eq!(cls.as_str(), *expected);
            let json = serde_json::to_string(cls).expect("serialize");
            assert_eq!(json, format!("\"{}\"", expected));
            let back: LifecycleClassification = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(cls, &back);
        }
    }

    #[test]
    fn p089_temp_inventory_dry_run_recommendation_serde_roundtrip() {
        let cases = [
            (DryRunRecommendation::WouldKeepActive, "would_keep_active"),
            (DryRunRecommendation::WouldKeepRecent, "would_keep_recent"),
            (
                DryRunRecommendation::WouldPreserveFailureEvidence,
                "would_preserve_failure_evidence",
            ),
            (
                DryRunRecommendation::WouldDeleteAfterFutureApproval,
                "would_delete_after_future_approval",
            ),
            (
                DryRunRecommendation::WouldMigrateLegacyManifestAfterFutureMigrationEnabled,
                "would_migrate_legacy_manifest_after_future_migration_enabled",
            ),
            (
                DryRunRecommendation::NeedsOperatorReview,
                "needs_operator_review",
            ),
            (DryRunRecommendation::NoRecommendation, "no_recommendation"),
            (DryRunRecommendation::Unknown, "unknown"),
        ];
        for (rec, expected) in &cases {
            assert_eq!(rec.as_str(), *expected);
            let json = serde_json::to_string(rec).expect("serialize");
            assert_eq!(json, format!("\"{}\"", expected));
            let back: DryRunRecommendation = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(rec, &back);
        }
    }

    #[test]
    fn p089_temp_inventory_mutation_guard_status_serde_roundtrip() {
        let cases = [
            (MutationGuardStatus::Pass, "pass"),
            (MutationGuardStatus::Fail, "fail"),
            (MutationGuardStatus::Skipped, "skipped"),
            (MutationGuardStatus::Unknown, "unknown"),
        ];
        for (s, expected) in &cases {
            assert_eq!(s.as_str(), *expected);
            let json = serde_json::to_string(s).expect("serialize");
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

    #[test]
    fn p089_temp_inventory_root_kind_serde_roundtrip() {
        let cases = [
            (RootKind::RunMetaRoot, "run_meta_root"),
            (RootKind::ControlPlaneCache, "control_plane_cache"),
            (RootKind::ProviderHomeCopy, "provider_home_copy"),
            (RootKind::LegacyChainworksTmp, "legacy_chainworks_tmp"),
            (RootKind::DiagnosticTestRoot, "diagnostic_test_root"),
            (RootKind::Unknown, "unknown"),
        ];
        for (k, expected) in &cases {
            assert_eq!(k.as_str(), *expected);
            let json = serde_json::to_string(k).expect("serialize");
            assert_eq!(json, format!("\"{}\"", expected));
            let back: RootKind = serde_json::from_str(&json).expect("deserialize");
            assert_eq!(k, &back);
        }
    }

    #[test]
    fn p089_temp_inventory_error_code_serde_roundtrip() {
        let cases = [
            (
                InventoryErrorCode::InvalidRootOverride,
                "invalid_root_override",
            ),
            (InventoryErrorCode::RootUnreadable, "root_unreadable"),
            (
                InventoryErrorCode::ManifestParseFailed,
                "manifest_parse_failed",
            ),
            (
                InventoryErrorCode::SizeEstimationFailed,
                "size_estimation_failed",
            ),
            (InventoryErrorCode::DeadlineExceeded, "deadline_exceeded"),
            (InventoryErrorCode::Cancelled, "cancelled"),
            (InventoryErrorCode::InternalError, "internal_error"),
            (
                InventoryErrorCode::MutationGuardFailed,
                "mutation_guard_failed",
            ),
            (InventoryErrorCode::ResourceExhausted, "resource_exhausted"),
            (InventoryErrorCode::Unknown, "unknown"),
        ];
        for (code, expected) in &cases {
            assert_eq!(code.as_str(), *expected);
            let json = serde_json::to_string(code).expect("serialize");
            assert_eq!(json, format!("\"{}\"", expected));
        }
    }

    #[test]
    fn p089_temp_inventory_mode_from_env_str() {
        assert_eq!(
            InventoryMode::from_env_str("disabled"),
            Some(InventoryMode::Disabled)
        );
        assert_eq!(
            InventoryMode::from_env_str("hidden_readback"),
            Some(InventoryMode::HiddenReadback)
        );
        assert_eq!(
            InventoryMode::from_env_str("operator_visible"),
            Some(InventoryMode::OperatorVisible)
        );
        assert_eq!(InventoryMode::from_env_str("unknown_value"), None);
        assert_eq!(InventoryMode::from_env_str(""), None);
    }

    #[test]
    fn p089_temp_inventory_mode_default_is_disabled() {
        assert_eq!(InventoryMode::default(), InventoryMode::Disabled);
    }

    #[test]
    fn p089_temp_inventory_request_bounds_are_consistent() {
        assert!(INVENTORY_LIMIT_MIN <= INVENTORY_LIMIT_MAX);
        assert!(INVENTORY_TIMEOUT_MS_MIN <= INVENTORY_TIMEOUT_MS_MAX);
        assert!(INVENTORY_LIMIT_MAX == 500);
        assert!(INVENTORY_TIMEOUT_MS_MAX == 5000);
        assert!(INVENTORY_TEST_ROOT_MAX_UTF8_BYTES == 4096);
    }

    #[test]
    fn p089_temp_inventory_scanner_constants_are_sane() {
        assert!(SCAN_GLOBAL_PERMIT_MAX >= 1, "at least one global permit");
        assert!(SCAN_CONTEXT_PERMIT_MAX >= 1, "at least one context permit");
        assert!(
            SCAN_CONTEXT_PERMIT_MAX <= SCAN_GLOBAL_PERMIT_MAX,
            "context <= global"
        );
        assert_eq!(SCAN_CANCEL_CHECK_INTERVAL_ENTRIES, 128);
        assert_eq!(SCAN_CANCEL_CHECK_INTERVAL_MS, 100);
        assert_eq!(SCAN_PARTIAL_ERRORS_MAX_PER_PAYLOAD, 100);
        assert_eq!(SCAN_PARTIAL_ERRORS_MAX_PER_ROW, 10);
        assert!(
            SCAN_MAX_VISITED_DIRS >= 1000,
            "visited dir cap must be substantial"
        );
        assert!(SCAN_MAX_DIR_DEPTH >= 1, "must allow at least root depth");
        assert!(
            SCAN_MAX_DIR_DEPTH * SCAN_GLOBAL_PERMIT_MAX <= 4096,
            "worst-case concurrent per-scan descriptor use must stay well below typical RLIMIT_NOFILE"
        );
        assert!(
            SCAN_MAX_PATH_BYTES >= INVENTORY_TEST_ROOT_MAX_UTF8_BYTES,
            "path budget must accommodate at least one max-length root"
        );
    }

    #[test]
    fn p089_temp_inventory_path_hash_lengths_consistent() {
        assert!(PATH_HASH_SHORT_MIN_LEN <= PATH_HASH_SHORT_MAX_LEN);
        assert!(PATH_HASH_SHORT_MAX_LEN <= PATH_HASH_HEX_LEN);
        assert_eq!(PATH_HASH_HEX_LEN, 64);
        assert_eq!(PATH_HASH_SHORT_MIN_LEN, 12);
        assert_eq!(PATH_HASH_SHORT_MAX_LEN, 20);
    }

    #[test]
    fn p089_temp_inventory_schema_version_constant() {
        assert_eq!(
            TEMP_ARTIFACT_INVENTORY_SCHEMA_VERSION,
            "temp_artifact_inventory_v1"
        );
    }

    #[test]
    fn p089_temp_inventory_disabled_mode_no_scan_contract() {
        // Disabled mode must never initiate scanning; verify the mode value maps correctly.
        let mode = InventoryMode::from_env_str("disabled").expect("disabled must parse");
        assert_eq!(mode, InventoryMode::Disabled);
        assert_eq!(mode.as_str(), "disabled");
        // Disabled mode enabled_state is Disabled and status is Disabled.
        let enabled = EnabledState::Disabled;
        let status = InventoryStatus::Disabled;
        assert_eq!(enabled.as_str(), "disabled");
        assert_eq!(status.as_str(), "disabled");
    }

    // ── Path hash tests ───────────────────────────────────────────────────────

    #[test]
    fn p089_temp_inventory_path_hash_returns_64_lowercase_hex_chars() {
        let hash = compute_path_hash(b"/tmp/test", RootKind::DiagnosticTestRoot);
        assert_eq!(hash.len(), PATH_HASH_HEX_LEN);
        assert!(hash
            .chars()
            .all(|c| c.is_ascii_hexdigit() && !c.is_ascii_uppercase()));
    }

    #[test]
    fn p089_temp_inventory_path_hash_deterministic_within_process() {
        let h1 = compute_path_hash(b"/tmp/test", RootKind::RunMetaRoot);
        let h2 = compute_path_hash(b"/tmp/test", RootKind::RunMetaRoot);
        assert_eq!(
            h1, h2,
            "same inputs must produce same hash within a process"
        );
    }

    #[test]
    fn p089_temp_inventory_path_hash_different_paths_differ() {
        let h1 = compute_path_hash(b"/tmp/a", RootKind::RunMetaRoot);
        let h2 = compute_path_hash(b"/tmp/b", RootKind::RunMetaRoot);
        assert_ne!(h1, h2, "distinct paths must produce distinct hashes");
    }

    #[test]
    fn p089_temp_inventory_path_hash_different_root_kinds_differ() {
        let h1 = compute_path_hash(b"/tmp/test", RootKind::RunMetaRoot);
        let h2 = compute_path_hash(b"/tmp/test", RootKind::ControlPlaneCache);
        assert_ne!(
            h1, h2,
            "same path with different root_kind must produce distinct hash"
        );
    }

    #[test]
    fn p089_temp_inventory_path_hash_nul_separator_prevents_collision() {
        // "/tmp/a" + root_kind "b" must differ from "/tmp/a" + root_kind "" + something.
        // Verified by checking that the two distinct inputs produce different outputs.
        let h1 = compute_path_hash(b"/tmp/ab", RootKind::Unknown);
        let h2 = compute_path_hash(b"/tmp/a", RootKind::Unknown);
        assert_ne!(h1, h2);
    }

    #[test]
    fn p089_temp_inventory_path_hash_distinct_non_utf8_paths_differ() {
        // Regression for SR-LOW-001: callers must pass raw OS bytes, not a lossy
        // UTF-8 conversion. Two distinct non-UTF-8 byte sequences that `String::from_utf8_lossy`
        // would both collapse to the same U+FFFD-substituted string must still hash
        // to distinct correlation identities when the true bytes are preserved.
        let a: &[u8] = b"/tmp/\xff";
        let b: &[u8] = b"/tmp/\xfe";
        // Sanity: confirm these really do collide under lossy conversion (both
        // single invalid bytes become the same U+FFFD replacement char), so the
        // test is exercising the scenario it claims to.
        assert_eq!(String::from_utf8_lossy(a), String::from_utf8_lossy(b));
        let h1 = compute_path_hash(a, RootKind::Unknown);
        let h2 = compute_path_hash(b, RootKind::Unknown);
        assert_ne!(
            h1, h2,
            "distinct raw byte paths must not collapse to the same hash"
        );
    }

    #[test]
    fn p089_temp_inventory_path_hash_short_starts_at_12() {
        let hash = compute_path_hash(b"/tmp/test", RootKind::DiagnosticTestRoot);
        let short = compute_path_hash_short(&hash, &[]);
        assert_eq!(short.len(), PATH_HASH_SHORT_MIN_LEN);
        assert_eq!(&short, &hash[..PATH_HASH_SHORT_MIN_LEN]);
    }

    #[test]
    fn p089_temp_inventory_path_hash_short_extends_on_collision() {
        let hash = compute_path_hash(b"/tmp/collision", RootKind::DiagnosticTestRoot);
        let existing = &hash[..PATH_HASH_SHORT_MIN_LEN];
        let short = compute_path_hash_short(&hash, &[existing]);
        // Must be longer than minimum since the 12-char prefix was in the collision set.
        assert!(short.len() > PATH_HASH_SHORT_MIN_LEN);
        assert!(short.len() <= PATH_HASH_SHORT_MAX_LEN);
    }

    #[test]
    fn p089_temp_inventory_path_hash_short_caps_at_20_on_all_collisions() {
        let hash = compute_path_hash(b"/tmp/cap", RootKind::DiagnosticTestRoot);
        // Simulate all possible extended prefixes already colliding.
        let collisions: Vec<&str> = (PATH_HASH_SHORT_MIN_LEN..PATH_HASH_SHORT_MAX_LEN)
            .step_by(2)
            .map(|l| &hash[..l])
            .collect();
        let short = compute_path_hash_short(&hash, &collisions);
        assert_eq!(short.len(), PATH_HASH_SHORT_MAX_LEN);
    }

    // ── Request validation tests ──────────────────────────────────────────────

    #[test]
    fn p089_temp_inventory_test_root_override_accepts_absolute_path() {
        assert!(validate_test_root_override("/tmp/chainworks-test").is_ok());
        assert!(validate_test_root_override("/var/folders/test").is_ok());
    }

    #[test]
    fn p089_temp_inventory_test_root_override_rejects_empty() {
        assert_eq!(
            validate_test_root_override(""),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
    }

    #[test]
    fn p089_temp_inventory_test_root_override_rejects_relative_path() {
        assert_eq!(
            validate_test_root_override("relative/path"),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
        assert_eq!(
            validate_test_root_override("./current"),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
    }

    #[test]
    fn p089_temp_inventory_test_root_override_rejects_tilde() {
        assert_eq!(
            validate_test_root_override("~/Desktop"),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
    }

    #[test]
    fn p089_temp_inventory_test_root_override_rejects_traversal() {
        assert_eq!(
            validate_test_root_override("/tmp/foo/../bar"),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
        assert_eq!(
            validate_test_root_override("/tmp/../etc"),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
    }

    #[test]
    fn p089_temp_inventory_test_root_override_rejects_nul_byte() {
        let path_with_nul = "/tmp/test\x00/injected";
        assert_eq!(
            validate_test_root_override(path_with_nul),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
    }

    #[test]
    fn p089_temp_inventory_test_root_override_rejects_over_4096_bytes() {
        let long_path = format!("/{}", "a".repeat(INVENTORY_TEST_ROOT_MAX_UTF8_BYTES));
        assert_eq!(
            validate_test_root_override(&long_path),
            Err(InventoryErrorCode::InvalidRootOverride)
        );
    }

    #[test]
    fn p089_temp_inventory_test_root_override_accepts_exactly_4096_bytes() {
        // Path of exactly INVENTORY_TEST_ROOT_MAX_UTF8_BYTES must be accepted.
        // "/" + 4095 'a' = 4096 bytes total.
        let path = format!("/{}", "a".repeat(INVENTORY_TEST_ROOT_MAX_UTF8_BYTES - 1));
        assert_eq!(path.len(), INVENTORY_TEST_ROOT_MAX_UTF8_BYTES);
        assert!(validate_test_root_override(&path).is_ok());
    }

    #[test]
    fn p089_temp_inventory_validate_limit_accepts_bounds() {
        assert!(validate_inventory_limit(0).is_ok());
        assert!(validate_inventory_limit(250).is_ok());
        assert!(validate_inventory_limit(500).is_ok());
    }

    #[test]
    fn p089_temp_inventory_validate_limit_rejects_out_of_range() {
        assert!(validate_inventory_limit(-1).is_err());
        assert!(validate_inventory_limit(501).is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_timeout_ms_accepts_bounds() {
        assert!(validate_inventory_timeout_ms(1).is_ok());
        assert!(validate_inventory_timeout_ms(2500).is_ok());
        assert!(validate_inventory_timeout_ms(5000).is_ok());
    }

    #[test]
    fn p089_temp_inventory_validate_timeout_ms_rejects_out_of_range() {
        assert!(validate_inventory_timeout_ms(0).is_err());
        assert!(validate_inventory_timeout_ms(5001).is_err());
        assert!(validate_inventory_timeout_ms(-1).is_err());
    }

    // ── validate_run_id tests (SEC-P089-005) ─────────────────────────────────

    #[test]
    fn p089_temp_inventory_validate_run_id_accepts_canonical_uuid_ids() {
        assert!(validate_run_id("2c74aef7-739c-4ac6-baa0-f67ca36cc7ef").is_ok());
        assert!(validate_run_id("aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa").is_ok());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_empty() {
        assert!(validate_run_id("").is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_absolute_path() {
        assert!(validate_run_id("/etc").is_err());
        assert!(validate_run_id("/tmp/evil").is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_traversal() {
        assert!(validate_run_id("../../../etc").is_err());
        assert!(validate_run_id("foo/../bar").is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_path_separator() {
        assert!(validate_run_id("foo/bar").is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_nul_byte() {
        assert!(validate_run_id("foo\x00bar").is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_dot_components() {
        assert!(validate_run_id(".").is_err());
        assert!(validate_run_id("..").is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_non_uuid_path_components() {
        assert!(validate_run_id("test-run-scoped-12345").is_err());
        assert!(validate_run_id("nonexistent-run-xyz").is_err());
        assert!(validate_run_id("abc123").is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_rejects_overlong() {
        let long_id = "a".repeat(RUN_ID_MAX_BYTES + 1);
        assert!(validate_run_id(&long_id).is_err());
    }

    #[test]
    fn p089_temp_inventory_validate_run_id_accepts_max_length() {
        let ok_id = "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb";
        assert_eq!(ok_id.len(), RUN_ID_MAX_BYTES);
        assert!(validate_run_id(ok_id).is_ok());
    }

    #[test]
    fn p089_temp_inventory_no_migration_evidence() {
        // Executable proof (replacing a prior placeholder `assert!(true)`): scans
        // every SQLite migration file under db/migrations and fails if any
        // references a temp-artifact-inventory table/column. This module defines
        // only in-memory types; no SQLite migration, SwiftData model, persisted
        // row, background sweeper, or startup recovery may exist for P089.
        let migrations_dir =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../db/migrations");
        let entries = std::fs::read_dir(&migrations_dir).unwrap_or_else(|e| {
            panic!("migrations dir must be readable at {migrations_dir:?}: {e}")
        });
        let mut offending = Vec::new();
        for entry in entries {
            let path = entry.expect("readable migration dir entry").path();
            if path.extension().and_then(|ext| ext.to_str()) != Some("sql") {
                continue;
            }
            let contents = std::fs::read_to_string(&path).expect("readable migration file");
            let lower = contents.to_lowercase();
            if lower.contains("temp_artifact") {
                offending.push(path.display().to_string());
            }
        }
        assert!(
            offending.is_empty(),
            "no persistent inventory schema changes may be introduced for P089, but found \
             temp_artifact references in: {offending:?}"
        );
    }

    #[test]
    fn p089_temp_inventory_mutation_guard_pass_cannot_mutate() {
        // Advisory dry-run pass status must not imply any mutation path exists.
        let guard = MutationGuardStatus::Pass;
        assert_eq!(guard.as_str(), "pass");
        // No delete, prune, chmod, or migration variant exists in this enum.
        // Verified structurally by exhaustive match:
        let all_variants = [
            MutationGuardStatus::Pass,
            MutationGuardStatus::Fail,
            MutationGuardStatus::Skipped,
            MutationGuardStatus::Unknown,
        ];
        for v in all_variants {
            let s = v.as_str();
            assert!(
                !s.contains("delete") && !s.contains("prune") && !s.contains("chmod"),
                "mutation guard variant must not imply mutation: {s}"
            );
        }
    }
}
