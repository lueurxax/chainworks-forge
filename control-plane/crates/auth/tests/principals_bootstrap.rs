//! Proposal 029 §7.2 / §8 principal-table bootstrap tests.
//!
//! The daemon calls `PrincipalTable::load_or_bootstrap` on startup:
//!
//! - If the principals file does not exist, a default operator principal
//!   is generated, the file is written with mode `0o600`. The bootstrap
//!   token is written to disk only and is NOT emitted to logs (HIGH-002
//!   redaction: tokens must never appear in log output).
//! - If the file exists and is well-formed, it is loaded without touching
//!   disk and no re-bootstrap occurs.
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

/// AC-15: bootstrap runs only once per file creation. We prove this by
/// calling `load_or_bootstrap` twice on the same path and asserting the
/// on-disk content is identical across calls — proof that no second
/// bootstrap (and therefore no second token write to disk) took place.
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

/// HIGH-002: an existing principals.json with permissions other than 0o600
/// must be rejected before the content is read.
#[test]
#[cfg(unix)]
fn test_high_002_principals_file_with_loose_permissions_is_rejected() {
    use std::io::Write;
    use std::os::unix::fs::OpenOptionsExt;

    let dir = tempfile::tempdir().expect("create tmp dir");
    let path = dir.path().join("principals.json");

    // Write a valid file with 0o644 (world-readable) permissions.
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o644)
            .open(&path)
            .expect("create principals.json with 0o644");
        f.write_all(br#"{"principals":[{"token":"t","id":"op","class":"operator"}]}"#)
            .expect("write");
    }

    let err = PrincipalTable::load_or_bootstrap(&path)
        .expect_err("loose-permissions file must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("0600") || msg.contains("mode"),
        "error must mention the mode requirement, got {msg:?}"
    );
}

/// HIGH-002: a principals.json path that is a symlink must be rejected.
#[test]
#[cfg(unix)]
fn test_high_002_symlinked_principals_file_is_rejected() {
    let dir = tempfile::tempdir().expect("create tmp dir");
    let real_path = dir.path().join("real_principals.json");
    let link_path = dir.path().join("principals.json");

    // Write a valid file with correct permissions at a different path.
    {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&real_path)
            .expect("create real_principals.json");
        f.write_all(br#"{"principals":[{"token":"t","id":"op","class":"operator"}]}"#)
            .expect("write");
    }

    // Place a symlink at the expected path pointing to the real file.
    std::os::unix::fs::symlink(&real_path, &link_path).expect("create symlink");

    let err = PrincipalTable::load_or_bootstrap(&link_path)
        .expect_err("symlinked principals file must be rejected");
    let msg = format!("{err}");
    assert!(
        msg.contains("symlink"),
        "error must mention symlink rejection, got {msg:?}"
    );
}

/// AC-15: an empty-principals file must fail closed, NOT silently allow
/// every caller through. This protects against the "delete the file, get
/// open access" failure mode.
#[test]
fn test_principals_daemon_refuses_empty_principals_file() {
    let dir = tempfile::tempdir().expect("create tmp dir");
    let path = dir.path().join("principals.json");
    // Write with owner-only permissions so SEC-005 mode check passes.
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        // SEC-005: parent directory must be 0700 for load_or_bootstrap to proceed.
        std::fs::set_permissions(dir.path(), std::fs::Permissions::from_mode(0o700))
            .expect("set dir perms to 0700");
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)
            .expect("create principals.json with 0o600");
        f.write_all(br#"{"principals":[]}"#)
            .expect("write empty principals file");
    }
    #[cfg(not(unix))]
    std::fs::write(&path, r#"{"principals":[]}"#).expect("write empty principals file");

    let err =
        PrincipalTable::load_or_bootstrap(&path).expect_err("zero-principal file must fail closed");
    let msg = format!("{err}");
    assert!(
        msg.contains("zero entries") || msg.contains("empty"),
        "error must clearly describe the empty-table failure, got {msg:?}"
    );
}
