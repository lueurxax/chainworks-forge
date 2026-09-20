use domain::execution_root::select_execution_root;

#[test]
fn read_only_explicit_worktrees_are_not_repository_work() {
    for strategy in ["dedicated", "shared_implementation_worktree"] {
        assert_eq!(
            select_execution_root("/repo", Some("/tree"), false, Some(strategy)).unwrap(),
            "/tree"
        );
        assert!(select_execution_root("/repo", None, false, Some(strategy)).is_err());
        assert!(select_execution_root("/repo", Some(""), true, Some(strategy)).is_err());
    }
}

#[test]
fn legacy_write_enabled_fallback_and_read_only_repository_are_preserved() {
    assert_eq!(
        select_execution_root("/repo", None, true, None).unwrap(),
        "/repo"
    );
    assert_eq!(
        select_execution_root("/repo", Some("/tree"), true, None).unwrap(),
        "/tree"
    );
    assert_eq!(
        select_execution_root("/repo", Some("/tree"), false, None).unwrap(),
        "/repo"
    );
    assert!(select_execution_root("", None, false, None).is_err());
}
