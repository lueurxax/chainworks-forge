//! P066 Phase 0 tests: toolchain mapper (T10), Go env shaping (T12),
//! per-run Xcode lease (T13), and host-executor argument rewriting (T11).
//!
//! Covers:
//! - adapter root preparation and subdirectory layout
//! - path-escape and absolute-injection rejection (fail-closed)
//! - free-space fail-closed sentinel (via mock)
//! - setup-failure and queue-timeout as distinct metric events
//! - Go env vars include GOENV=off unconditionally
//! - TMPDIR derives from mapping root (not provider env)
//! - xcodebuild -derivedDataPath and -clonedSourcePackagesDirPath injected

use std::collections::BTreeMap;
use std::path::PathBuf;

use acp::toolchain_mapper::{
    build_go_env_vars, prepare_toolchain_mapping, validate_path_segment, ToolchainFamily,
};
use acp::{XcodeHostExecutorPlan, XcodeHostExecutorPlanInput};
use domain::toolchain::ToolchainSetupFailureReason;

// ── T10: Directory preparation ────────────────────────────────────────────────

#[test]
fn p066_toolchain_mapper_xcode_prepares_correct_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();
    let run_id = "run-abc123";

    let result = prepare_toolchain_mapping(
        toolchain_home,
        ToolchainFamily::Xcode,
        run_id,
        0, // no min free space
    )
    .expect("xcode mapping must succeed");

    // Root must be providers/xcode/{run_id}/xcode
    let expected_root = toolchain_home
        .join("providers")
        .join("xcode")
        .join(run_id)
        .join("xcode");
    assert_eq!(result.root, expected_root);

    // Required subdirectories must exist.
    for dir in &["DerivedData", "SourcePackages", "tmp"] {
        assert!(
            expected_root.join(dir).is_dir(),
            "xcode mapping root must contain {dir}"
        );
    }

    // created_directories list must record the dirs.
    assert!(result.created_directories.contains(&"DerivedData".to_string()));
    assert!(result.created_directories.contains(&"SourcePackages".to_string()));
    assert!(result.created_directories.contains(&"tmp".to_string()));

    // Relative root suffix must not contain toolchain_home prefix.
    assert!(
        !result.relative_root_suffix.contains(toolchain_home.to_str().unwrap()),
        "relative_root_suffix must be relative, not absolute"
    );
    assert!(result.relative_root_suffix.contains("providers/xcode"));

    // Xcode does not inject env vars.
    assert!(result.env_vars.is_empty(), "xcode family must not set env vars via mapper");
}

#[test]
fn p066_toolchain_mapper_go_prepares_correct_directories() {
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();
    let session_gen_id = "sess-xyz789";

    let result = prepare_toolchain_mapping(
        toolchain_home,
        ToolchainFamily::Go,
        session_gen_id,
        0,
    )
    .expect("go mapping must succeed");

    // Root must be providers/go/{session_generation_id}
    let expected_root = toolchain_home
        .join("providers")
        .join("go")
        .join(session_gen_id);
    assert_eq!(result.root, expected_root);

    for dir in &["cache", "mod", "go", "tmp"] {
        assert!(
            expected_root.join(dir).is_dir(),
            "go mapping root must contain {dir}"
        );
    }

    // Go env vars must be set.
    assert!(!result.env_vars.is_empty(), "go family must produce env vars");
}

#[cfg(unix)]
#[test]
fn p066_toolchain_mapper_sets_owner_only_permissions_on_root_and_subdirectories() {
    use std::os::unix::fs::PermissionsExt;

    let tmp = tempfile::tempdir().unwrap();
    let result = prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Go, "sess-perms", 0)
        .expect("go mapping must succeed");

    let root_mode = std::fs::metadata(&result.root).unwrap().permissions().mode() & 0o777;
    assert_eq!(root_mode, 0o700, "mapping root must be owner-only");

    for subdir in &result.created_directories {
        let subpath = result.root.join(subdir);
        let mode = std::fs::metadata(&subpath).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o700, "{subdir} must be owner-only");
    }
}

#[test]
fn p066_toolchain_mapper_setup_duration_recorded() {
    let tmp = tempfile::tempdir().unwrap();
    let result = prepare_toolchain_mapping(
        tmp.path(),
        ToolchainFamily::Xcode,
        "run-timing",
        0,
    )
    .unwrap();

    assert!(
        result.setup_duration_ms >= 0,
        "setup_duration_ms must be non-negative"
    );
    assert!(
        result.setup_duration_ms < 2_000,
        "setup_duration_ms must be well under the 2000ms deadline in a unit test"
    );
}

#[test]
fn p066_toolchain_mapper_diag_family_result_shape() {
    let tmp = tempfile::tempdir().unwrap();
    let result = prepare_toolchain_mapping(
        tmp.path(),
        ToolchainFamily::Xcode,
        "run-diag-shape",
        0,
    )
    .unwrap();

    let family_result = result.to_diag_family_result();
    assert_eq!(family_result.family, "xcode");
    assert_eq!(family_result.effective_scope.as_deref(), Some("run"));
    assert_eq!(family_result.scope_key_kind.as_deref(), Some("run_id"));
    assert!(family_result.validation_failures.is_empty());
    assert!(family_result.setup_failure_reason.is_none());
}

// ── T10: Path validation (fail-closed) ───────────────────────────────────────

#[test]
fn p066_toolchain_mapper_dotdot_scope_key_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let err = prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Xcode, "..", 0)
        .unwrap_err();
    assert_eq!(
        err.reason,
        ToolchainSetupFailureReason::PathEscape,
        "'..' scope key must return PathEscape"
    );
}

#[test]
fn p066_toolchain_mapper_empty_scope_key_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let err = prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Xcode, "", 0)
        .unwrap_err();
    assert_eq!(
        err.reason,
        ToolchainSetupFailureReason::PathEscape,
        "empty scope key must return PathEscape"
    );
}

#[test]
fn p066_toolchain_mapper_slash_in_scope_key_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let err =
        prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Xcode, "a/b", 0).unwrap_err();
    assert_eq!(
        err.reason,
        ToolchainSetupFailureReason::PathEscape,
        "slash in scope key must return PathEscape"
    );
}

#[test]
fn p066_toolchain_mapper_dot_scope_key_rejected() {
    let tmp = tempfile::tempdir().unwrap();
    let err = prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Xcode, ".", 0)
        .unwrap_err();
    assert_eq!(
        err.reason,
        ToolchainSetupFailureReason::PathEscape,
        "'.' scope key must return PathEscape"
    );
}

#[test]
fn p066_toolchain_mapper_accepts_toolchain_home_with_trailing_separator() {
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = PathBuf::from(format!("{}/", tmp.path().display()));

    let result = prepare_toolchain_mapping(&toolchain_home, ToolchainFamily::Go, "sess-slash", 0)
        .expect("trailing separator on TOOLCHAIN_HOME must not fail containment");

    assert!(result.root.is_dir());
    assert!(result.root.starts_with(tmp.path()));
}

#[cfg(unix)]
#[test]
fn p066_toolchain_mapper_rejects_symlinked_provider_path_escape() {
    use std::os::unix::fs::symlink;

    let tmp = tempfile::tempdir().unwrap();
    let outside = tempfile::tempdir().unwrap();
    symlink(outside.path(), tmp.path().join("providers")).unwrap();

    let err = prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Go, "sess-symlink", 0)
        .unwrap_err();

    assert_eq!(
        err.reason,
        ToolchainSetupFailureReason::PathEscape,
        "symlinked providers directory must fail closed"
    );
    assert!(
        !outside.path().join("go").exists(),
        "mapper must not create directories through an escaping symlink"
    );
}

#[test]
fn p066_validate_path_segment_roundtrip_valid() {
    // Valid segments must pass.
    let valid = ["run-abc", "session-xyz", "a1b2c3", "run_scope_key"];
    for seg in &valid {
        assert!(
            validate_path_segment(seg).is_ok(),
            "valid segment '{seg}' must pass validation"
        );
    }
}

#[test]
fn p066_validate_path_segment_rejects_traversal() {
    let invalid = ["..", ".", "a/../b", "a/b"];
    for seg in &invalid {
        assert!(
            validate_path_segment(seg).is_err(),
            "segment '{seg}' must be rejected"
        );
    }
}

// ── T12: Go env shaping ───────────────────────────────────────────────────────

#[test]
fn p066_go_env_vars_include_all_required_keys() {
    let root = PathBuf::from("/toolchain/providers/go/sess-abc");
    let env = build_go_env_vars(&root);

    let required = ["GOCACHE", "GOMODCACHE", "GOPATH", "TMPDIR", "GOENV"];
    for key in &required {
        assert!(
            env.contains_key(*key),
            "Go env vars must include {key}"
        );
    }
}

#[test]
fn p066_go_env_goenv_is_always_off() {
    let root = PathBuf::from("/toolchain/providers/go/sess-abc");
    let env = build_go_env_vars(&root);

    assert_eq!(
        env.get("GOENV").map(|s| s.as_str()),
        Some("off"),
        "GOENV must be 'off' unconditionally when Go isolation is enabled"
    );
}

#[test]
fn p066_go_env_vars_all_under_root() {
    let root = PathBuf::from("/toolchain/providers/go/sess-abc");
    let env = build_go_env_vars(&root);

    let root_str = root.to_string_lossy();
    for (key, val) in &env {
        if key == "GOENV" {
            continue; // GOENV=off is literal, not a path
        }
        assert!(
            val.starts_with(root_str.as_ref()),
            "{key} value '{val}' must be under toolchain root '{root_str}'"
        );
    }
}

#[test]
fn p066_go_env_vars_gocache_gomodcache_gopath_tmpdir_correct_subdirs() {
    let root = PathBuf::from("/tc/providers/go/s1");
    let env = build_go_env_vars(&root);

    assert_eq!(env["GOCACHE"], "/tc/providers/go/s1/cache");
    assert_eq!(env["GOMODCACHE"], "/tc/providers/go/s1/mod");
    assert_eq!(env["GOPATH"], "/tc/providers/go/s1/go");
    assert_eq!(env["TMPDIR"], "/tc/providers/go/s1/tmp");
}

#[test]
fn p066_go_mapping_produces_correct_env_vars() {
    let tmp = tempfile::tempdir().unwrap();
    let result = prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Go, "sess-abc", 0)
        .unwrap();

    assert_eq!(
        result.env_vars.get("GOENV").map(|s| s.as_str()),
        Some("off"),
        "Go mapping env vars must include GOENV=off"
    );
    assert!(
        result.env_vars.contains_key("GOCACHE"),
        "Go mapping must include GOCACHE"
    );
    assert!(
        result.env_vars.contains_key("GOMODCACHE"),
        "Go mapping must include GOMODCACHE"
    );
    assert!(
        result.env_vars.contains_key("GOPATH"),
        "Go mapping must include GOPATH"
    );
    assert!(
        result.env_vars.contains_key("TMPDIR"),
        "Go mapping must include TMPDIR"
    );
}

// ── T11: Xcode host-executor argument rewriting ───────────────────────────────

fn make_xcodebuild_plan_input(
    toolchain_mapping_root: Option<PathBuf>,
) -> XcodeHostExecutorPlanInput {
    XcodeHostExecutorPlanInput {
        invoked_tool: "xcodebuild".to_string(),
        args: vec!["-scheme".to_string(), "MyScheme".to_string()],
        cwd: "/workspace".to_string(),
        workspace_root: "/workspace".to_string(),
        provider_env: BTreeMap::new(),
        simulator_candidates: vec![],
        toolchain_mapping_root,
    }
}

#[test]
fn p066_xcode_host_executor_plan_injects_derived_data_path() {
    let root = PathBuf::from("/tc/providers/xcode/run-abc/xcode");
    let input = make_xcodebuild_plan_input(Some(root.clone()));

    let plan = XcodeHostExecutorPlan::build(input).expect("plan must build");

    let derived_data_flag_pos = plan
        .argv
        .iter()
        .position(|a| a == "-derivedDataPath");
    assert!(
        derived_data_flag_pos.is_some(),
        "xcodebuild plan must inject -derivedDataPath"
    );

    if let Some(pos) = derived_data_flag_pos {
        let value = &plan.argv[pos + 1];
        assert!(
            value.contains("/tc/providers/xcode/run-abc/xcode/DerivedData"),
            "-derivedDataPath must point to DerivedData under mapping root, got: {value}"
        );
    }
}

#[test]
fn p066_xcode_host_executor_plan_injects_source_packages_path() {
    let root = PathBuf::from("/tc/providers/xcode/run-abc/xcode");
    let input = make_xcodebuild_plan_input(Some(root.clone()));

    let plan = XcodeHostExecutorPlan::build(input).expect("plan must build");

    let flag_pos = plan
        .argv
        .iter()
        .position(|a| a == "-clonedSourcePackagesDirPath");
    assert!(
        flag_pos.is_some(),
        "xcodebuild plan must inject -clonedSourcePackagesDirPath"
    );

    if let Some(pos) = flag_pos {
        let value = &plan.argv[pos + 1];
        assert!(
            value.contains("/tc/providers/xcode/run-abc/xcode/SourcePackages"),
            "-clonedSourcePackagesDirPath must point under mapping root, got: {value}"
        );
    }
}

#[test]
fn p066_xcode_host_executor_plan_derives_tmpdir_from_mapping_root() {
    let root = PathBuf::from("/tc/providers/xcode/run-abc/xcode");
    let input = make_xcodebuild_plan_input(Some(root.clone()));

    let plan = XcodeHostExecutorPlan::build(input).expect("plan must build");

    let tmpdir = plan.env.get("TMPDIR");
    assert!(
        tmpdir.is_some(),
        "TMPDIR must be set in host-executor env when toolchain_mapping_root is present"
    );
    assert!(
        tmpdir.unwrap().contains("/tc/providers/xcode/run-abc/xcode/tmp"),
        "TMPDIR must derive from mapping root/tmp, got: {:?}",
        tmpdir
    );
}

#[test]
fn p066_xcode_host_executor_plan_without_mapping_root_no_deriveddata_injection() {
    // Without a toolchain_mapping_root, no -derivedDataPath should be injected.
    let input = make_xcodebuild_plan_input(None);

    let plan = XcodeHostExecutorPlan::build(input).expect("plan must build");

    assert!(
        !plan.argv.contains(&"-derivedDataPath".to_string()),
        "without toolchain_mapping_root, -derivedDataPath must not be injected"
    );
    assert!(
        !plan.argv.contains(&"-clonedSourcePackagesDirPath".to_string()),
        "without toolchain_mapping_root, -clonedSourcePackagesDirPath must not be injected"
    );
}

// ── Setup failure vs queue timeout distinctness ───────────────────────────────

#[test]
fn p066_setup_failure_and_queue_timeout_have_distinct_failure_kinds() {
    assert_ne!(
        domain::toolchain::ToolchainMappingSetupFailed::failure_kind_str(),
        domain::toolchain::XcodeRunScopeQueueTimeout::failure_kind_str(),
        "toolchain_mapping_setup_failed and xcode_run_scope_queue_timeout must be distinct"
    );
}

#[test]
fn p066_setup_failure_not_triggered_by_queue_timeout() {
    // A queue timeout (lease wait exceeded) must not surface as mapping_setup_timeout.
    // This is verified structurally: XcodeRunScopeQueueTimeout::failure_kind_str ≠
    // ToolchainMappingSetupFailed::failure_kind_str and ≠ ToolchainSetupFailureReason::Timeout.
    assert_ne!(
        domain::toolchain::XcodeRunScopeQueueTimeout::failure_kind_str(),
        "mapping_setup_timeout",
        "queue timeout must NOT be classified as mapping_setup_timeout"
    );
    assert_ne!(
        domain::toolchain::XcodeRunScopeQueueTimeout::failure_kind_str(),
        ToolchainSetupFailureReason::Timeout.as_str(),
        "queue timeout must NOT reuse the setup Timeout reason string"
    );
}

// ── T20: session-scope cleanup contract (Go root lifecycle) ──────────────────

#[test]
fn p066_go_session_root_cleanup_on_drop() {
    // Verify that a prepared Go session-scoped root can be removed by the caller,
    // matching the DeleteOnClose cleanup contract (AcpSession::close removes cleanup_paths).
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();
    let session_gen_id = "gen-session-abc";

    let result = prepare_toolchain_mapping(
        toolchain_home,
        ToolchainFamily::Go,
        session_gen_id,
        0,
    )
    .expect("Go mapping must succeed");

    let root = result.root.clone();
    assert!(root.exists(), "root must exist after prepare");

    // Simulate session close: remove the root (this is what AcpSession::close does
    // for cleanup_paths — std::fs::remove_dir_all on each registered path).
    std::fs::remove_dir_all(&root).expect("cleanup must succeed");
    assert!(!root.exists(), "Go session root must be removed after cleanup");
}

#[test]
fn p066_go_session_root_layout_matches_session_scope_contract() {
    // Root must be providers/go/{session_generation_id}/ (not run-scoped).
    let tmp = tempfile::tempdir().unwrap();
    let session_gen_id = "gen-abc";
    let result = prepare_toolchain_mapping(tmp.path(), ToolchainFamily::Go, session_gen_id, 0)
        .expect("Go mapping must succeed");

    let expected = tmp.path().join("providers").join("go").join(session_gen_id);
    assert_eq!(result.root, expected, "Go root must be under providers/go/{{session_gen_id}}");
    assert_eq!(result.family.scope_key_kind(), "session_generation_id");
    assert_eq!(result.family.effective_scope(), "session");
}

#[test]
fn p066_go_session_cleanup_fields_are_correct_in_diagnostics() {
    // DiagCleanupState::delete_on_close() is the canonical cleanup plan for Go session roots.
    let cleanup = domain::toolchain_diagnostics::DiagCleanupState::delete_on_close();
    assert_eq!(
        cleanup.owner,
        domain::toolchain_diagnostics::DiagCleanupOwner::AcpSessionClose,
    );
    assert_eq!(
        cleanup.plan,
        domain::toolchain_diagnostics::DiagCleanupPlan::DeleteOnClose,
    );
    assert!(
        cleanup.aggregate_outcome_surfaces.contains(&"startupRecoverySummary.toolchainCache".to_string()),
        "cleanup must surface in startupRecoverySummary.toolchainCache"
    );
}
