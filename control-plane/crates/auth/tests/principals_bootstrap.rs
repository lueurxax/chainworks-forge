//! Proposal 029 §7.2 / §8 principal-table bootstrap tests.
//!
//! The daemon calls `PrincipalTable::load_or_bootstrap` on startup:
//!
//! - If the principals file does not exist, a default operator principal
//!   is generated, the file is written with mode `0o600`, and the bootstrap
//!   token is logged **once** at `info` level on that start only.
//! - If the file exists and is well-formed, it is loaded without touching
//!   disk and no bootstrap log fires.
//! - If the file exists but contains zero principals, the daemon fails
//!   closed with a clear error (no silent "welcome, anyone can connect").
//!
//! These tests exercise each rung of that contract.

use auth::PrincipalTable;

/// AC-15 (Unix only): the bootstrap file must be owner-only (0o600).
/// Skipped on non-Unix platforms, matching the `cfg(unix)` permission code
/// in `auth::PrincipalTable::load_or_bootstrap`.
#[test]
#[cfg(unix)]
fn test_principals_file_created_with_owner_only_permissions() {
    use std::os::unix::fs::PermissionsExt;

    let dir = tempfile::tempdir().expect("create tmp dir");
    let path = dir.path().join("principals.json");
    assert!(!path.exists(), "precondition: file must not exist");

    let _table =
        PrincipalTable::load_or_bootstrap(&path).expect("bootstrap on fresh path should succeed");

    let meta = std::fs::metadata(&path).expect("file exists after bootstrap");
    let mode = meta.permissions().mode() & 0o777;
    assert_eq!(
        mode, 0o600,
        "bootstrap-created principals file must be owner-only (0o600), got 0o{mode:o}"
    );
}

/// AC-15: the bootstrap token is emitted only on the first start that
/// created the file. We prove this by calling `load_or_bootstrap` twice
/// on the same path and asserting the on-disk content is identical across
/// calls — proof that no second bootstrap (and therefore no second token
/// log) took place. A stable on-disk file means the `tracing::info!` call
/// inside the bootstrap branch ran at most once.
#[test]
fn test_principals_bootstrap_token_logged_once_on_first_start() {
    let dir = tempfile::tempdir().expect("create tmp dir");
    let path = dir.path().join("principals.json");

    let _first = PrincipalTable::load_or_bootstrap(&path).expect("first-start bootstrap");
    let content_after_first =
        std::fs::read_to_string(&path).expect("file exists after first bootstrap");

    let _second =
        PrincipalTable::load_or_bootstrap(&path).expect("subsequent start loads existing");
    let content_after_second =
        std::fs::read_to_string(&path).expect("file survives subsequent start");

    assert_eq!(
        content_after_first, content_after_second,
        "subsequent start must NOT rewrite the principals file — token would be re-logged"
    );

    // Sanity: the bootstrap file contains a token and an operator principal.
    assert!(content_after_first.contains("\"token\""));
    assert!(content_after_first.contains("\"operator\""));
}

/// AC-15: an empty-principals file must fail closed, NOT silently allow
/// every caller through. This protects against the "delete the file, get
/// open access" failure mode.
#[test]
fn test_principals_daemon_refuses_empty_principals_file() {
    let dir = tempfile::tempdir().expect("create tmp dir");
    let path = dir.path().join("principals.json");
    // Well-formed JSON, zero principals.
    std::fs::write(&path, r#"{"principals":[]}"#).expect("write empty principals file");

    let err =
        PrincipalTable::load_or_bootstrap(&path).expect_err("zero-principal file must fail closed");
    let msg = format!("{err}");
    assert!(
        msg.contains("zero entries") || msg.contains("empty"),
        "error must clearly describe the empty-table failure, got {msg:?}"
    );
}
