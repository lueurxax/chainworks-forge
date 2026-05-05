//! P066 T10/T12: Toolchain cache mapping — directory preparation and Go env shaping.
//!
//! Layout: {TOOLCHAIN_HOME}/providers/{family}/{scope_key}/...
//!   Xcode run scope:   providers/xcode/{run_id}/xcode/{DerivedData,SourcePackages,tmp}
//!   Go session scope:  providers/go/{session_generation_id}/{cache,mod,go,tmp}
//!
//! Fail-closed contract: setup exceeding 2000 ms deadline, path validation failure,
//! permission failure, or disk-full condition → ToolchainMappingSetupFailed.
//! Diagnostics MUST be emitted even on setup failure (proposal §failure_contract).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};
use std::time::{Duration, Instant};

use domain::toolchain::{ToolchainMappingSetupFailed, ToolchainSetupFailureReason};
use domain::toolchain_diagnostics::{DiagFamilyResult, DiagPathMode};

/// Setup deadline: 2000 ms (proposal §failure_contract.setup_scope.deadline_ms).
pub const TOOLCHAIN_SETUP_DEADLINE: Duration = Duration::from_millis(2_000);

/// Default minimum free bytes before mapping setup fails closed (500 MB).
pub const DEFAULT_MIN_FREE_BYTES: u64 = 500 * 1024 * 1024;

/// Toolchain family — determines subdirectory layout and scope key kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ToolchainFamily {
    /// Xcode run-scoped: providers/xcode/{run_id}/xcode/
    Xcode,
    /// Go session-scoped: providers/go/{session_generation_id}/
    Go,
    /// Swift best-effort session-scoped: providers/swift_best_effort/{session_generation_id}/
    SwiftBestEffort,
}

impl ToolchainFamily {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Xcode => "xcode",
            Self::Go => "go",
            Self::SwiftBestEffort => "swift_best_effort",
        }
    }

    pub fn scope_key_kind(self) -> &'static str {
        match self {
            Self::Xcode => "run_id",
            Self::Go | Self::SwiftBestEffort => "session_generation_id",
        }
    }

    pub fn effective_scope(self) -> &'static str {
        match self {
            Self::Xcode => "run",
            Self::Go | Self::SwiftBestEffort => "session",
        }
    }

    fn subdirectories(self) -> &'static [&'static str] {
        match self {
            Self::Xcode => &["DerivedData", "SourcePackages", "tmp"],
            Self::Go => &["cache", "mod", "go", "tmp"],
            Self::SwiftBestEffort => &["tmp"],
        }
    }

    fn path_mode(self) -> DiagPathMode {
        match self {
            Self::Xcode => DiagPathMode::ArgumentsAndEnv,
            Self::Go => DiagPathMode::EnvironmentOnly,
            Self::SwiftBestEffort => DiagPathMode::BestEffort,
        }
    }
}

/// Result of a successful toolchain mapping preparation.
#[derive(Debug)]
pub struct ToolchainMappingResult {
    /// Root directory: {TOOLCHAIN_HOME}/providers/{family}/{scope_key}[/xcode]
    pub root: PathBuf,
    pub family: ToolchainFamily,
    pub scope_key: String,
    /// Relative root suffix (TOOLCHAIN_HOME prefix stripped).
    pub relative_root_suffix: String,
    /// Subdirectory names that were created under root.
    pub created_directories: Vec<String>,
    /// Setup duration in milliseconds.
    pub setup_duration_ms: i64,
    /// Environment variables to set (non-empty for Go family).
    pub env_vars: BTreeMap<String, String>,
}

impl ToolchainMappingResult {
    /// Build a `DiagFamilyResult` suitable for the diagnostics document.
    pub fn to_diag_family_result(&self) -> DiagFamilyResult {
        DiagFamilyResult {
            family: self.family.as_str().to_string(),
            effective_scope: Some(self.family.effective_scope().to_string()),
            requested_scope: Some(self.family.effective_scope().to_string()),
            scope_key_kind: Some(self.family.scope_key_kind().to_string()),
            path_mode: self.family.path_mode(),
            relative_root_suffix: self.relative_root_suffix.clone(),
            created_directories: self.created_directories.clone(),
            unsupported_mappings: vec![],
            validation_failures: vec![],
            setup_failure_reason: None,
        }
    }

    /// Path to DerivedData under the Xcode mapping root.
    pub fn xcode_derived_data_path(&self) -> PathBuf {
        self.root.join("DerivedData")
    }

    /// Path to SourcePackages under the Xcode mapping root.
    pub fn xcode_source_packages_path(&self) -> PathBuf {
        self.root.join("SourcePackages")
    }

    /// Path to tmp under the mapping root (used as TMPDIR for Xcode).
    pub fn tmpdir_path(&self) -> PathBuf {
        self.root.join("tmp")
    }
}

// ── Path validation ──────────────────────────────────────────────────────────

/// Validate a single path segment for use under TOOLCHAIN_HOME.
///
/// Rejects:
/// - Empty string (invalid)
/// - "." or ".." (path traversal)
/// - Any segment containing '/' or '\' (injection of separator)
///
/// Returns `PathEscape` for traversal-style rejections,
/// `InvalidRoot` for absolute-prefix injections.
pub fn validate_path_segment(seg: &str) -> Result<(), ToolchainSetupFailureReason> {
    if seg.is_empty() || seg == "." || seg == ".." {
        return Err(ToolchainSetupFailureReason::PathEscape);
    }
    // Reject embedded separators (injection).
    if seg.contains('/') || seg.contains('\\') {
        return Err(ToolchainSetupFailureReason::PathEscape);
    }
    // Reject leading slash (absolute path injection).
    if seg.starts_with('/') || seg.starts_with('\\') {
        return Err(ToolchainSetupFailureReason::InvalidRoot);
    }
    Ok(())
}

/// Verify that `candidate` is contained under `root`.
/// Uses path component containment and rejects existing symlink components
/// below the root so new mapping directories cannot be created through an
/// escaping link.
pub fn validate_path_containment(
    candidate: &Path,
    root: &Path,
) -> Result<(), ToolchainSetupFailureReason> {
    let canonical_root =
        std::fs::canonicalize(root).map_err(|_| ToolchainSetupFailureReason::PathEscape)?;
    let relative_candidate = candidate
        .strip_prefix(root)
        .map_err(|_| ToolchainSetupFailureReason::PathEscape)?;

    let mut checked = canonical_root.clone();
    for component in relative_candidate.components() {
        match component {
            Component::CurDir => {}
            Component::Normal(segment) => {
                checked.push(segment);
                if checked.strip_prefix(&canonical_root).is_err() {
                    return Err(ToolchainSetupFailureReason::PathEscape);
                }
                if let Ok(metadata) = std::fs::symlink_metadata(&checked) {
                    if metadata.file_type().is_symlink() {
                        return Err(ToolchainSetupFailureReason::PathEscape);
                    }
                    let canonical_checked = std::fs::canonicalize(&checked)
                        .map_err(|_| ToolchainSetupFailureReason::PathEscape)?;
                    if canonical_checked.strip_prefix(&canonical_root).is_err() {
                        return Err(ToolchainSetupFailureReason::PathEscape);
                    }
                    checked = canonical_checked;
                }
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ToolchainSetupFailureReason::PathEscape);
            }
        }
    }
    Ok(())
}

// ── Free space check ─────────────────────────────────────────────────────────

/// Check available free space on the volume containing `path`.
/// Returns `DiskFull` if available bytes < `min_free_bytes`.
/// On non-unix platforms or if stat fails, the check is skipped (fail-open for the check itself).
#[cfg(unix)]
pub fn check_free_space(
    path: &Path,
    min_free_bytes: u64,
) -> Result<(), ToolchainSetupFailureReason> {
    use std::ffi::CString;
    use std::os::unix::ffi::OsStrExt;

    let c_path = match CString::new(path.as_os_str().as_bytes()) {
        Ok(p) => p,
        Err(_) => return Ok(()), // non-fatal: cannot build C string
    };

    let mut stat: libc::statvfs = unsafe { std::mem::zeroed() };
    let ret = unsafe { libc::statvfs(c_path.as_ptr(), &mut stat) };
    if ret != 0 {
        return Ok(()); // cannot stat — skip check
    }

    let available = stat.f_bavail as u64 * stat.f_frsize as u64;
    if available < min_free_bytes {
        return Err(ToolchainSetupFailureReason::DiskFull);
    }
    Ok(())
}

#[cfg(not(unix))]
pub fn check_free_space(
    _path: &Path,
    _min_free_bytes: u64,
) -> Result<(), ToolchainSetupFailureReason> {
    Ok(())
}

// ── Directory preparation ─────────────────────────────────────────────────────

/// Prepare toolchain mapping directories for a given family and scope key.
///
/// Validates scope_key, derives the mapping root, checks free space, creates
/// the root and its subdirectories with owner-only permissions (0o700).
/// The entire operation must complete within `TOOLCHAIN_SETUP_DEADLINE` (2000 ms).
///
/// Returns `ToolchainMappingSetupFailed` if any step fails.
/// The caller MUST persist diagnostics even on failure (proposal §failure_contract).
pub fn prepare_toolchain_mapping(
    toolchain_home: &Path,
    family: ToolchainFamily,
    scope_key: &str,
    min_free_bytes: u64,
) -> Result<ToolchainMappingResult, ToolchainMappingSetupFailed> {
    let start = Instant::now();
    let family_str = family.as_str();

    // Validate scope_key.
    validate_path_segment(scope_key)
        .map_err(|reason| ToolchainMappingSetupFailed::new(reason, family_str))?;

    // Derive the root path.
    // Xcode: providers/xcode/{run_id}/xcode/
    // Others: providers/{family}/{scope_key}/
    let root = if family == ToolchainFamily::Xcode {
        toolchain_home
            .join("providers")
            .join(family_str)
            .join(scope_key)
            .join("xcode")
    } else {
        toolchain_home
            .join("providers")
            .join(family_str)
            .join(scope_key)
    };

    // Validate containment.
    validate_path_containment(&root, toolchain_home)
        .map_err(|reason| ToolchainMappingSetupFailed::new(reason, family_str))?;

    // Free space check on TOOLCHAIN_HOME volume.
    check_free_space(toolchain_home, min_free_bytes)
        .map_err(|reason| ToolchainMappingSetupFailed::new(reason, family_str))?;

    deadline_check(start, family_str)?;

    // Create root directory with owner-only permissions.
    std::fs::create_dir_all(&root).map_err(|e| {
        let reason = if e.kind() == std::io::ErrorKind::PermissionDenied {
            ToolchainSetupFailureReason::PermissionDenied
        } else {
            ToolchainSetupFailureReason::InvalidRoot
        };
        ToolchainMappingSetupFailed::new(reason, family_str).with_detail(e.to_string())
    })?;

    set_owner_only_permissions(&root, family_str)?;

    // Create subdirectories.
    let mut created_directories = Vec::new();
    for &subdir in family.subdirectories() {
        let subpath = root.join(subdir);
        std::fs::create_dir_all(&subpath).map_err(|e| {
            let reason = if e.kind() == std::io::ErrorKind::PermissionDenied {
                ToolchainSetupFailureReason::PermissionDenied
            } else {
                ToolchainSetupFailureReason::InvalidRoot
            };
            ToolchainMappingSetupFailed::new(reason, family_str).with_detail(e.to_string())
        })?;
        set_owner_only_permissions(&subpath, family_str)?;
        created_directories.push(subdir.to_string());
    }

    deadline_check(start, family_str)?;

    let setup_duration_ms = start.elapsed().as_millis() as i64;

    let relative_root_suffix = root
        .strip_prefix(toolchain_home)
        .unwrap_or(&root)
        .to_string_lossy()
        .to_string();

    let env_vars = if family == ToolchainFamily::Go {
        build_go_env_vars(&root)
    } else {
        BTreeMap::new()
    };

    Ok(ToolchainMappingResult {
        root,
        family,
        scope_key: scope_key.to_string(),
        relative_root_suffix,
        created_directories,
        setup_duration_ms,
        env_vars,
    })
}

fn deadline_check(start: Instant, family_str: &str) -> Result<(), ToolchainMappingSetupFailed> {
    if start.elapsed() > TOOLCHAIN_SETUP_DEADLINE {
        return Err(ToolchainMappingSetupFailed::new(
            ToolchainSetupFailureReason::Timeout,
            family_str,
        ));
    }
    Ok(())
}

#[cfg(unix)]
fn set_owner_only_permissions(
    path: &Path,
    family_str: &str,
) -> Result<(), ToolchainMappingSetupFailed> {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o700)).map_err(|e| {
        ToolchainMappingSetupFailed::new(ToolchainSetupFailureReason::PermissionDenied, family_str)
            .with_detail(e.to_string())
    })
}

#[cfg(not(unix))]
fn set_owner_only_permissions(
    _path: &Path,
    _family_str: &str,
) -> Result<(), ToolchainMappingSetupFailed> {
    Ok(())
}

// ── Go env shaping ────────────────────────────────────────────────────────────

/// Build Go environment variables for toolchain isolation.
///
/// GOENV=off is unconditional whenever Go isolation is enabled so that
/// host-global GOENV state cannot override the mapped directories.
pub fn build_go_env_vars(root: &Path) -> BTreeMap<String, String> {
    let mut env = BTreeMap::new();
    env.insert(
        "GOCACHE".to_string(),
        root.join("cache").to_string_lossy().to_string(),
    );
    env.insert(
        "GOMODCACHE".to_string(),
        root.join("mod").to_string_lossy().to_string(),
    );
    env.insert(
        "GOPATH".to_string(),
        root.join("go").to_string_lossy().to_string(),
    );
    env.insert(
        "TMPDIR".to_string(),
        root.join("tmp").to_string_lossy().to_string(),
    );
    // Unconditional: prevent host-global GOENV from overriding mapped dirs.
    env.insert("GOENV".to_string(), "off".to_string());
    env
}
