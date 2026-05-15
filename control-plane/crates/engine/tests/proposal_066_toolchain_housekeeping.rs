//! P066 T19: Generated-state housekeeping for run-scoped Xcode toolchain roots.
//!
//! Verifies:
//! - Terminal run-scoped Xcode roots are pruned by housekeeping.
//! - Active run-scoped Xcode roots are preserved.
//! - Unknown run-scoped directories are preserved.
//! - A ToolchainCacheHousekeepingReadback row is written after the sweep.

use std::fs;

use chrono::Utc;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = db::pool::create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .expect("test shared DbWriter registration failed");
    pool
}

async fn insert_run(pool: &sqlx::SqlitePool, status: &str) -> String {
    let idea_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO ideas (id, title, body, status, created_at) VALUES (?1, 'T', 'B', 'draft', ?2)",
    )
    .bind(&idea_id)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    let run_id = uuid::Uuid::new_v4().to_string();
    sqlx::query(
        "INSERT INTO runs (id, idea_id, status, workflow_id, workflow_title, workspace_root, artifact_root, started_at) VALUES (?1, ?2, ?3, 'wf', 'W', '/ws', '/art', ?4)",
    )
    .bind(&run_id)
    .bind(&idea_id)
    .bind(status)
    .bind(Utc::now().to_rfc3339())
    .execute(pool)
    .await
    .unwrap();

    run_id
}

fn make_xcode_root(toolchain_home: &std::path::Path, run_id: &str) -> std::path::PathBuf {
    let root = toolchain_home.join("providers").join("xcode").join(run_id);
    fs::create_dir_all(&root).unwrap();
    fs::write(root.join("sentinel"), b"x").unwrap();
    root
}

#[tokio::test]
async fn p066_t19_terminal_run_xcode_root_is_pruned() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();

    let run_id = insert_run(&pool, "completed").await;
    let xcode_root = make_xcode_root(toolchain_home, &run_id);
    assert!(xcode_root.exists());

    // Sweep without setting CHAINWORKS_TOOLCHAIN_HOME env var — call helper directly.
    use engine::housekeeping::sweep_xcode_toolchain_roots as sweep;
    sweep(&pool, toolchain_home).await.unwrap();

    assert!(
        !xcode_root.exists(),
        "terminal run Xcode root must be pruned"
    );
}

#[tokio::test]
async fn p066_t19_active_run_xcode_root_is_preserved() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();

    let run_id = insert_run(&pool, "running").await;
    let xcode_root = make_xcode_root(toolchain_home, &run_id);

    use engine::housekeeping::sweep_xcode_toolchain_roots as sweep;
    sweep(&pool, toolchain_home).await.unwrap();

    assert!(
        xcode_root.exists(),
        "active run Xcode root must be preserved"
    );
}

#[tokio::test]
async fn p066_t19_unknown_run_xcode_root_is_preserved() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();

    // Run not in DB at all.
    let xcode_root = make_xcode_root(toolchain_home, "run-not-in-db");

    use engine::housekeeping::sweep_xcode_toolchain_roots as sweep;
    sweep(&pool, toolchain_home).await.unwrap();

    assert!(
        xcode_root.exists(),
        "unknown run Xcode root must be preserved"
    );
}

#[tokio::test]
async fn p066_t19_mixed_roots_only_terminal_pruned() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();

    let completed_id = insert_run(&pool, "completed").await;
    let failed_id = insert_run(&pool, "failed").await;
    let running_id = insert_run(&pool, "running").await;

    let completed_root = make_xcode_root(toolchain_home, &completed_id);
    let failed_root = make_xcode_root(toolchain_home, &failed_id);
    let running_root = make_xcode_root(toolchain_home, &running_id);

    use engine::housekeeping::sweep_xcode_toolchain_roots as sweep;
    let readback = sweep(&pool, toolchain_home).await.unwrap();

    assert!(
        !completed_root.exists(),
        "completed run root must be pruned"
    );
    assert!(!failed_root.exists(), "failed run root must be pruned");
    assert!(running_root.exists(), "running run root must be preserved");

    assert_eq!(
        readback.run_scoped_roots_pruned, 2,
        "2 terminal roots pruned"
    );
    assert_eq!(readback.run_scoped_prune_failures, 0);
}

#[tokio::test]
async fn p066_t19_readback_is_recorded_to_db() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let toolchain_home = tmp.path();

    let run_id = insert_run(&pool, "cancelled").await;
    make_xcode_root(toolchain_home, &run_id);

    use engine::housekeeping::sweep_xcode_toolchain_roots as sweep;
    let readback = sweep(&pool, toolchain_home).await.unwrap();

    // Persist the readback (mirrors what run_once does).
    db::repos::toolchain_cache_housekeeping::insert(&pool, &readback)
        .await
        .unwrap();

    let latest = db::repos::toolchain_cache_housekeeping::latest(&pool)
        .await
        .unwrap()
        .expect("readback must be present after insert");

    assert_eq!(latest.run_scoped_roots_pruned, 1);
    assert_eq!(latest.run_scoped_prune_failures, 0);
}
