// P066: Domain types for toolchain cache mapping failure classification.
//
// Two distinct failure kinds:
// - ToolchainMappingSetupFailed: directory preparation, validation, free-space precheck failed.
// - XcodeRunScopeQueueTimeout: waiting for the per-run Xcode exclusive lease timed out.
//
// These must not overlap. Queue timeout MUST NOT increment mapping_setup_latency_p95_ms.

use serde::{Deserialize, Serialize};

/// P066: Typed subreasons for toolchain_mapping_setup_failed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ToolchainSetupFailureReason {
    /// TOOLCHAIN_HOME volume free space is below the configured minimum.
    DiskFull,
    /// OS rejected the directory creation or permission set with EACCES/EPERM.
    PermissionDenied,
    /// Directory preparation did not complete within the 2000 ms deadline.
    Timeout,
    /// A prepared root path failed containment validation (path segment invalid).
    InvalidRoot,
    /// A derived path conflicts with an existing sibling root.
    PathConflict,
    /// Prepared roots could not be registered with the launch-resource rollback guard.
    CleanupRegistrationFailed,
    /// A derived path segment attempts to escape TOOLCHAIN_HOME (path traversal).
    PathEscape,
}

impl ToolchainSetupFailureReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::DiskFull => "mapping_setup_disk_full",
            Self::PermissionDenied => "mapping_setup_permission_denied",
            Self::Timeout => "mapping_setup_timeout",
            Self::InvalidRoot => "mapping_setup_invalid_root",
            Self::PathConflict => "mapping_setup_path_conflict",
            Self::CleanupRegistrationFailed => "mapping_setup_cleanup_registration_failed",
            Self::PathEscape => "mapping_setup_path_escape",
        }
    }
}

impl std::fmt::Display for ToolchainSetupFailureReason {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

impl std::str::FromStr for ToolchainSetupFailureReason {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "mapping_setup_disk_full" => Ok(Self::DiskFull),
            "mapping_setup_permission_denied" => Ok(Self::PermissionDenied),
            "mapping_setup_timeout" => Ok(Self::Timeout),
            "mapping_setup_invalid_root" => Ok(Self::InvalidRoot),
            "mapping_setup_path_conflict" => Ok(Self::PathConflict),
            "mapping_setup_cleanup_registration_failed" => Ok(Self::CleanupRegistrationFailed),
            "mapping_setup_path_escape" => Ok(Self::PathEscape),
            other => Err(format!("Unknown ToolchainSetupFailureReason: {other}")),
        }
    }
}

/// P066: Failure kind emitted when toolchain cache mapping setup fails.
/// Carries a typed subreason from the seven defined values.
/// Deadline: 2000 ms. Never overlaps with XcodeRunScopeQueueTimeout.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolchainMappingSetupFailed {
    pub reason: ToolchainSetupFailureReason,
    pub family: String,
    pub detail: Option<String>,
}

impl ToolchainMappingSetupFailed {
    pub fn new(reason: ToolchainSetupFailureReason, family: impl Into<String>) -> Self {
        Self {
            reason,
            family: family.into(),
            detail: None,
        }
    }

    pub fn with_detail(mut self, detail: impl Into<String>) -> Self {
        self.detail = Some(detail.into());
        self
    }

    pub fn failure_kind_str() -> &'static str {
        "toolchain_mapping_setup_failed"
    }
}

impl std::fmt::Display for ToolchainMappingSetupFailed {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "toolchain_mapping_setup_failed: family={}, reason={}",
            self.family, self.reason
        )?;
        if let Some(detail) = &self.detail {
            write!(f, ", detail={detail}")?;
        }
        Ok(())
    }
}

/// P066: Failure kind emitted when waiting for the per-run Xcode exclusive lease
/// exceeds min(300_000 ms, remaining request runtime budget).
/// This is NOT a setup failure and MUST NOT increment mapping_setup_latency_p95_ms.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct XcodeRunScopeQueueTimeout {
    pub run_id: String,
    pub wait_ms: u64,
    pub deadline_ms: u64,
}

impl XcodeRunScopeQueueTimeout {
    pub fn failure_kind_str() -> &'static str {
        "xcode_run_scope_queue_timeout"
    }
}

impl std::fmt::Display for XcodeRunScopeQueueTimeout {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "xcode_run_scope_queue_timeout: run_id={}, waited_ms={}, deadline_ms={}",
            self.run_id, self.wait_ms, self.deadline_ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn p066_toolchain_setup_failure_reason_roundtrip() {
        let reasons = [
            ToolchainSetupFailureReason::DiskFull,
            ToolchainSetupFailureReason::PermissionDenied,
            ToolchainSetupFailureReason::Timeout,
            ToolchainSetupFailureReason::InvalidRoot,
            ToolchainSetupFailureReason::PathConflict,
            ToolchainSetupFailureReason::CleanupRegistrationFailed,
            ToolchainSetupFailureReason::PathEscape,
        ];
        for reason in reasons {
            let s = reason.as_str();
            let parsed: ToolchainSetupFailureReason = s.parse().unwrap();
            assert_eq!(parsed, reason, "roundtrip failed for {s}");
        }
    }

    #[test]
    fn p066_toolchain_mapping_setup_failed_display() {
        let err =
            ToolchainMappingSetupFailed::new(ToolchainSetupFailureReason::PathEscape, "xcode")
                .with_detail("../../../etc/passwd");
        let s = err.to_string();
        assert!(s.contains("toolchain_mapping_setup_failed"));
        assert!(s.contains("xcode"));
        assert!(s.contains("mapping_setup_path_escape"));
    }

    #[test]
    fn p066_xcode_queue_timeout_display_has_correct_kind() {
        let err = XcodeRunScopeQueueTimeout {
            run_id: "run-abc".to_string(),
            wait_ms: 300_001,
            deadline_ms: 300_000,
        };
        assert!(err.to_string().contains("xcode_run_scope_queue_timeout"));
        assert_eq!(
            XcodeRunScopeQueueTimeout::failure_kind_str(),
            "xcode_run_scope_queue_timeout"
        );
    }

    #[test]
    fn p066_setup_and_queue_timeout_failure_kinds_are_distinct() {
        assert_ne!(
            ToolchainMappingSetupFailed::failure_kind_str(),
            XcodeRunScopeQueueTimeout::failure_kind_str(),
            "toolchain_mapping_setup_failed and xcode_run_scope_queue_timeout must be distinct"
        );
    }
}
