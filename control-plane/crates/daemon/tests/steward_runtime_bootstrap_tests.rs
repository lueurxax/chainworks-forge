use db::pool::create_pool;
use std::sync::Arc;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .expect("register shared writer");
    pool
}

#[test]
fn steward_runtime_bootstrap_tests_invalid_config_falls_back_to_default() {
    let temp = tempfile::tempdir().unwrap();
    let config_path = temp.path().join("steward_config.yaml");
    std::fs::write(
        &config_path,
        r#"
schema_version: 1
windows:
  observation_window_size: 1
  baseline_window_size: 1
  minimum_window_size: 5
  maximum_window_age_days: 90
triggers:
  post_run_hook:
    enabled: true
    run_interval: 0
  on_config_change:
    enabled: true
  schedule:
    enabled: false
    cron: "0 8 * * 1"
"#,
    )
    .unwrap();

    let effective = daemon::steward_runtime::load_effective_config(Some(&config_path));
    assert!(effective.used_default);
    assert_eq!(effective.config.windows.minimum_window_size, 5);
    assert!(effective.config.triggers.post_run_hook.enabled);
    assert_eq!(effective.config.triggers.post_run_hook.run_interval, 1);
    assert_eq!(effective.hash.len(), 64);
}

#[tokio::test]
async fn steward_runtime_bootstrap_tests_config_change_sets_pending_without_work_item() {
    let pool = test_pool().await;
    let temp = tempfile::tempdir().unwrap();
    let catalog_path = temp.path().join("agents.yaml");
    std::fs::write(
        &catalog_path,
        r#"
schema_version: 1
agents:
  - id: system_steward
    backend_profile: steward
backend_profiles:
  steward:
    provider: claude
"#,
    )
    .unwrap();
    db::repos::steward::set_runtime_state(&pool, "steward_config_hash", "old-hash")
        .await
        .unwrap();

    let effective =
        daemon::steward_runtime::bootstrap_steward_runtime(&pool, None, Some(&catalog_path))
            .await
            .unwrap();

    assert_ne!(effective.steward_config_hash, "old-hash");
    let pending = db::repos::steward::take_config_change_pending(&pool)
        .await
        .unwrap()
        .expect("config change should be pending");
    assert_eq!(
        pending.config_hash.as_deref(),
        Some(effective.steward_config_hash.as_str())
    );
    assert_eq!(
        pending.catalog_hash.as_deref(),
        Some(effective.agent_catalog_hash.as_str())
    );
    assert_eq!(
        db::repos::steward::post_run_trigger_config(&pool)
            .await
            .unwrap(),
        (true, 1),
        "default bootstrap must persist every-run Steward analysis"
    );
    let queued = db::repos::work_items::claim_next(&pool).await.unwrap();
    assert!(
        queued.is_none(),
        "bootstrap must set pending state only, not execute Steward analysis"
    );
}
