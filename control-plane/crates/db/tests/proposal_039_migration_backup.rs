//! CF-21 backup/upgrade evidence and the CF-22 old-migrator refusal simulation.
//! Every database is disposable; version 100 is built by the real SQLx migrator.

use db::migrate::{run_preflight, DbStateKind, MigrationError, MIGRATOR};
use sqlx::migrate::{MigrateError, Migrator};
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use sqlx::SqlitePool;
use std::future::{poll_fn, Future};
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::task::Poll;
use std::time::Duration;
use tempfile::TempDir;

struct HistoricalDb {
    dir: TempDir,
    path: PathBuf,
    url: String,
    pool: SqlitePool,
}

impl HistoricalDb {
    async fn version_100() -> Self {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("historical.db");
        let url = format!("sqlite://{}", path.display());
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .min_connections(1)
            .idle_timeout(None)
            .max_lifetime(None)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(&path)
                    .create_if_missing(true)
                    .foreign_keys(false)
                    .journal_mode(SqliteJournalMode::Wal)
                    .pragma("wal_autocheckpoint", "0"),
            )
            .await
            .unwrap();
        MIGRATOR.run_to(100, &pool).await.unwrap();
        sqlx::query("PRAGMA foreign_keys=ON")
            .execute(&pool)
            .await
            .unwrap();
        assert_schema(&pool, 100, false).await;

        insert_history(&pool, "checkpointed").await;
        let checkpoint: (i64, i64, i64) = sqlx::query_as("PRAGMA wal_checkpoint(TRUNCATE)")
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(checkpoint, (0, 0, 0));
        let main_bytes = std::fs::read(&path).unwrap();
        insert_history(&pool, "wal-only").await;
        assert_eq!(std::fs::read(&path).unwrap(), main_bytes);
        assert!(
            std::fs::metadata(path.with_extension("db-wal"))
                .unwrap()
                .len()
                > 32
        );

        // Immutable read deliberately ignores the source WAL. This proves that
        // a main-file copy would lose a committed historical run, not just bytes.
        let main_only = open_immutable(&path).await;
        assert_eq!(run_ids(&main_only).await, ["checkpointed"]);
        main_only.close().await;
        assert_eq!(run_ids(&pool).await, ["checkpointed", "wal-only"]);
        Self {
            dir,
            path,
            url,
            pool,
        }
    }

    fn backups(&self) -> PathBuf {
        self.dir.path().join("backups")
    }
}

async fn insert_history(pool: &SqlitePool, id: &str) {
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?, 'Historical idea', 'Preserve the original', '2026-09-19')")
        .bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at,current_state,workflow_snapshot_json,catalog_snapshot_json) VALUES (?,?,'blocked','old-workflow','Historical workflow','/fixture/checkout','/fixture/meta','2026-09-19','implementation','{\"revision\":\"old-workflow\"}','{\"revision\":\"old-catalog\"}')")
        .bind(id).bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,started_at) VALUES (?,?,'implementation','Historical stage','blocked','2026-09-19')")
        .bind(id).bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO agent_executions(id,stage_execution_id,agent_id,provider,status,started_at) VALUES (?,?,'implementer','codex','failed','2026-09-19')")
        .bind(id).bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO approvals(id,run_id,stage_id,decision,requested_at,decided_at,comment) VALUES (?,?,'old-gate','approved','2026-09-18','2026-09-19','Historical only')")
        .bind(id).bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO artifacts(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at,agent_execution_id) VALUES (?,?,'implementation','implementer','proposal','proposal-v1','md','/fixture/meta/proposal.md','codex','2026-09-19',?)")
        .bind(id).bind(id).bind(id).execute(&mut *tx).await.unwrap();
    sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,run_id,created_at,result_status) VALUES (?,'StartRun','{\"historical\":true}',?,'2026-09-19','succeeded')")
        .bind(id).bind(id).execute(&mut *tx).await.unwrap();
    tx.commit().await.unwrap();
}

async fn open_immutable(path: &Path) -> SqlitePool {
    SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(path)
                .read_only(true)
                .immutable(true),
        )
        .await
        .unwrap()
}

async fn run_ids(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT id FROM runs ORDER BY id")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn assert_schema(pool: &SqlitePool, version: i64, p039_present: bool) {
    let actual: i64 = sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations")
        .fetch_one(pool)
        .await
        .unwrap();
    assert_eq!(actual, version);
    let p039: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM sqlite_schema WHERE type='table' AND name='run_continuations'",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(p039, i64::from(p039_present));
}

async fn assert_integrity(pool: &SqlitePool) {
    let integrity: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check")
        .fetch_all(pool)
        .await
        .unwrap();
    assert_eq!(integrity, ["ok"]);
    assert!(sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(pool)
        .await
        .unwrap()
        .is_empty());
}

async fn history(pool: &SqlitePool) -> Vec<Vec<String>> {
    // Stable SQL encodings preserve NULL versus text and snapshot/approval bytes.
    let mut result = Vec::new();
    for query in [
        "SELECT json_array(id,title,body,status,created_at) FROM ideas ORDER BY id",
        "SELECT json_array(id,idea_id,status,current_state,workflow_id,workflow_title,workspace_root,artifact_root,started_at,workflow_snapshot_json,catalog_snapshot_json) FROM runs ORDER BY id",
        "SELECT json_array(id,run_id,stage_id,label,status,started_at) FROM stage_executions ORDER BY id",
        "SELECT json_array(id,stage_execution_id,agent_id,provider,status,started_at) FROM agent_executions ORDER BY id",
        "SELECT json_array(id,run_id,stage_id,decision,requested_at,decided_at,comment) FROM approvals ORDER BY id",
        "SELECT json_array(id,run_id,stage_id,agent_id,name,contract_id,format,file_path,provider,created_at,agent_execution_id) FROM artifacts ORDER BY id",
        "SELECT json_array(id,command_type,payload_json,run_id,created_at,result_status) FROM command_journal ORDER BY id",
        "SELECT json_array(version,description,installed_on,success,hex(checksum),execution_time) FROM _sqlx_migrations WHERE version<=100 ORDER BY version",
    ] {
        result.push(sqlx::query_scalar(query).fetch_all(pool).await.unwrap());
    }
    result
}

fn entries(path: &Path) -> Vec<PathBuf> {
    let mut paths: Vec<_> = std::fs::read_dir(path)
        .unwrap()
        .map(|entry| entry.unwrap().path().canonicalize().unwrap())
        .collect();
    paths.sort();
    paths
}

#[tokio::test]
async fn cf21_version_100_upgrade_restores_wal_history_without_source_sidecars() {
    let fixture = HistoricalDb::version_100().await;
    let before = history(&fixture.pool).await;
    let outcome = run_preflight(&fixture.url, Some(&fixture.backups()))
        .await
        .unwrap();
    assert_eq!(outcome.classified_as, DbStateKind::TrackedSubset);
    assert!(outcome.applied_migrations);
    assert_eq!(outcome.schema_version, 101);
    let backup = outcome.backup_path.unwrap();
    assert_eq!(
        std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(entries(&fixture.backups()), [backup.clone()]);
    assert_schema(&fixture.pool, 101, true).await;
    assert_integrity(&fixture.pool).await;
    assert_eq!(history(&fixture.pool).await, before);

    // Copy only the completed snapshot to a different directory. No WAL/SHM,
    // source connection, or subsequent migration is available to the restore.
    let restore_dir = TempDir::new().unwrap();
    let restore_path = restore_dir.path().join("restored.sqlite");
    std::fs::copy(&backup, &restore_path).unwrap();
    let restored = open_immutable(&restore_path).await;
    assert_schema(&restored, 100, false).await;
    assert_integrity(&restored).await;
    assert_eq!(run_ids(&restored).await, ["checkpointed", "wal-only"]);
    assert_eq!(history(&restored).await, before);
    restored.close().await;
    assert_eq!(
        entries(restore_dir.path()),
        [restore_path.canonicalize().unwrap()]
    );

    let backup_bytes = std::fs::read(&backup).unwrap();
    let again = run_preflight(&fixture.url, Some(&fixture.backups()))
        .await
        .unwrap();
    assert!(!again.applied_migrations);
    assert!(again.backup_path.is_none());
    assert_eq!(std::fs::read(&backup).unwrap(), backup_bytes);
    assert_eq!(entries(&fixture.backups()), [backup]);
    fixture.pool.close().await;
}

#[tokio::test]
async fn cf21_relative_filename_upgrade_restores_wal_history_without_source_sidecars() {
    const CHILD: &str = "CHAINWORKS_P039_RELATIVE_BACKUP_CHILD";
    const TEST: &str =
        "cf21_relative_filename_upgrade_restores_wal_history_without_source_sidecars";
    if std::env::var_os(CHILD).is_none() {
        let fixture = HistoricalDb::version_100().await;
        let before = history(&fixture.pool).await;
        let dir = fixture.dir.path().to_path_buf();
        // Only the child changes working directory; the live parent connection
        // keeps committed historical rows in the WAL throughout the upgrade.
        let output = tokio::task::spawn_blocking(move || {
            std::process::Command::new(std::env::current_exe().unwrap())
                .args(["--exact", TEST, "--nocapture"])
                .env(CHILD, "1")
                .current_dir(dir)
                .output()
                .unwrap()
        })
        .await
        .unwrap();
        assert!(
            output.status.success(),
            "relative-path child failed: {}\nstdout:\n{}\nstderr:\n{}",
            output.status,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert_schema(&fixture.pool, 101, true).await;
        assert_integrity(&fixture.pool).await;
        assert_eq!(history(&fixture.pool).await, before);
        fixture.pool.close().await;
        return;
    }

    let source = SqlitePoolOptions::new()
        .max_connections(1)
        .connect_with(
            SqliteConnectOptions::new()
                .filename("historical.db")
                .read_only(true),
        )
        .await
        .unwrap();
    assert_schema(&source, 100, false).await;
    assert_eq!(run_ids(&source).await, ["checkpointed", "wal-only"]);
    let before = history(&source).await;
    let main_only = open_immutable(Path::new("historical.db")).await;
    assert_eq!(run_ids(&main_only).await, ["checkpointed"]);
    main_only.close().await;

    let outcome = run_preflight("sqlite:historical.db", None).await.unwrap();
    assert_eq!(outcome.classified_as, DbStateKind::TrackedSubset);
    assert!(outcome.applied_migrations);
    assert_eq!(outcome.schema_version, 101);
    let backup = outcome.backup_path.unwrap();
    assert_eq!(
        backup.parent().unwrap(),
        Path::new(".").canonicalize().unwrap()
    );
    assert_eq!(
        std::fs::metadata(&backup).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert!(!entries(Path::new(".")).iter().any(|path| path
        .extension()
        .is_some_and(|extension| extension == "pending")));
    assert_schema(&source, 101, true).await;
    assert_integrity(&source).await;
    assert_eq!(history(&source).await, before);
    source.close().await;

    let restore_dir = TempDir::new().unwrap();
    let restore_path = restore_dir.path().join("restored.sqlite");
    std::fs::copy(&backup, &restore_path).unwrap();
    let restored = open_immutable(&restore_path).await;
    assert_schema(&restored, 100, false).await;
    assert_integrity(&restored).await;
    assert_eq!(run_ids(&restored).await, ["checkpointed", "wal-only"]);
    assert_eq!(history(&restored).await, before);
    restored.close().await;
    assert_eq!(
        entries(restore_dir.path()),
        [restore_path.canonicalize().unwrap()]
    );

    let backup_bytes = std::fs::read(&backup).unwrap();
    let entries_before = entries(Path::new("."));
    let again = run_preflight("sqlite:historical.db", None).await.unwrap();
    assert!(!again.applied_migrations);
    assert!(again.backup_path.is_none());
    assert_eq!(std::fs::read(&backup).unwrap(), backup_bytes);
    assert_eq!(entries(Path::new(".")), entries_before);
}

#[tokio::test]
async fn cf21_backup_failure_blocks_migration_and_preserves_version_100() {
    let fixture = HistoricalDb::version_100().await;
    let before = history(&fixture.pool).await;
    let main_before = std::fs::read(&fixture.path).unwrap();
    let wal_path = fixture.path.with_extension("db-wal");
    let wal_before = std::fs::read(&wal_path).unwrap();
    let blocked_dir = fixture.backups();
    std::fs::write(&blocked_dir, b"existing operator file").unwrap();

    let error = run_preflight(&fixture.url, Some(&blocked_dir))
        .await
        .unwrap_err();
    assert!(
        matches!(error, MigrationError::BackupFailed(_)),
        "{error:?}"
    );
    assert_schema(&fixture.pool, 100, false).await;
    assert_integrity(&fixture.pool).await;
    assert_eq!(history(&fixture.pool).await, before);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), main_before);
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal_before);
    assert_eq!(
        std::fs::read(&blocked_dir).unwrap(),
        b"existing operator file"
    );

    // A new, valid destination can retry; the failed attempt left no tracker lie.
    run_preflight(&fixture.url, Some(&fixture.dir.path().join("retry")))
        .await
        .unwrap();
    assert_schema(&fixture.pool, 101, true).await;
    assert_eq!(history(&fixture.pool).await, before);
    fixture.pool.close().await;
}

#[tokio::test]
async fn cf22_version_100_migrator_refuses_actual_p039_schema_without_data_loss() {
    let fixture = HistoricalDb::version_100().await;
    let older = Migrator::with_migrations(
        MIGRATOR
            .iter()
            .filter(|migration| migration.version <= 100)
            .cloned()
            .collect(),
    );
    older.run(&fixture.pool).await.unwrap();
    run_preflight(&fixture.url, Some(&fixture.backups()))
        .await
        .unwrap();
    let before = history(&fixture.pool).await;
    let main_before = std::fs::read(&fixture.path).unwrap();
    let wal_path = fixture.path.with_extension("db-wal");
    let wal_before = std::fs::read(&wal_path).unwrap();

    let error = older.run(&fixture.pool).await.unwrap_err();
    assert!(
        matches!(error, MigrateError::VersionMissing(101)),
        "{error:?}"
    );
    assert_schema(&fixture.pool, 101, true).await;
    assert_integrity(&fixture.pool).await;
    assert_eq!(history(&fixture.pool).await, before);
    assert_eq!(std::fs::read(&fixture.path).unwrap(), main_before);
    assert_eq!(std::fs::read(&wal_path).unwrap(), wal_before);
    fixture.pool.close().await;
}

#[tokio::test]
async fn cf21_upgraded_database_enforces_continuation_uniqueness_and_foreign_keys() {
    let fixture = HistoricalDb::version_100().await;
    // Version 100 permits multiple blocked historical runs for one idea.
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES ('same-idea','wal-only','blocked','old-workflow','Historical workflow','/fixture/other','/fixture/other-meta','2026-09-19')")
        .execute(&fixture.pool).await.unwrap();
    let before = history(&fixture.pool).await;
    run_preflight(&fixture.url, Some(&fixture.backups()))
        .await
        .unwrap();
    let insert = "INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,'wal-only',?,'operator',?,?,'implementation_restart_v1','plan.json',?,'target.json',?,'wal-only','2026-09-20','2026-09-20','2026-09-21')";
    for (id, source, expected) in [
        ("missing-source", "absent", Some("foreign-key")),
        ("first", "wal-only", None),
        ("duplicate-source", "wal-only", Some("unique")),
        ("duplicate-idea", "same-idea", Some("unique")),
    ] {
        let result = sqlx::query(insert)
            .bind(id)
            .bind(source)
            .bind(format!("successor-{id}"))
            .bind(id)
            .bind("a".repeat(64))
            .bind("b".repeat(64))
            .bind("c".repeat(64))
            .execute(&fixture.pool)
            .await;
        match expected {
            None => {
                result.unwrap();
            }
            Some(kind) => {
                let error = result.unwrap_err();
                let error = error.as_database_error().unwrap();
                assert!(
                    match kind {
                        "foreign-key" => error.is_foreign_key_violation(),
                        "unique" => error.is_unique_violation(),
                        _ => unreachable!(),
                    },
                    "{id}: {error}"
                );
            }
        }
    }
    let operations: Vec<String> = sqlx::query_scalar("SELECT operation_id FROM run_continuations")
        .fetch_all(&fixture.pool)
        .await
        .unwrap();
    assert_eq!(operations, ["first"]);
    assert_integrity(&fixture.pool).await;
    assert_eq!(history(&fixture.pool).await, before);
    fixture.pool.close().await;
}

// Poll the real preflight only to an observable private-snapshot boundary, then
// stop polling it. No background task or timing sleep can race publication.
async fn pause_at_written_snapshot<F: Future>(
    mut future: Pin<&mut F>,
    directory: &Path,
) -> PathBuf {
    tokio::time::timeout(
        Duration::from_secs(10),
        poll_fn(|cx| {
            assert!(
                future.as_mut().poll(cx).is_pending(),
                "preflight published before the test boundary"
            );
            if directory.exists() {
                for path in entries(directory) {
                    if path.extension().is_some_and(|ext| ext == "pending")
                        && std::fs::metadata(&path).unwrap().len() > 0
                    {
                        return Poll::Ready(path);
                    }
                }
            }
            Poll::Pending
        }),
    )
    .await
    .expect("preflight did not reach a private snapshot boundary")
}

#[tokio::test]
async fn cf21_cancelled_snapshot_remains_private_and_is_not_reused_on_retry() {
    let fixture = HistoricalDb::version_100().await;
    let before = history(&fixture.pool).await;
    let directory = fixture.backups();
    let mut migration = Box::pin(run_preflight(&fixture.url, Some(&directory)));
    let pending = pause_at_written_snapshot(migration.as_mut(), &directory).await;
    drop(migration);

    assert_eq!(
        std::fs::metadata(&pending).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_eq!(entries(&directory), [pending.clone()]);
    assert_schema(&fixture.pool, 100, false).await;
    assert_eq!(history(&fixture.pool).await, before);

    let outcome = run_preflight(&fixture.url, Some(&directory)).await.unwrap();
    let backup = outcome.backup_path.unwrap();
    assert_ne!(backup, pending);
    assert!(
        pending.exists(),
        "retry must not consume the interrupted snapshot"
    );
    let mut expected = vec![pending, backup.clone()];
    expected.sort();
    assert_eq!(entries(&directory), expected);
    let restored = open_immutable(&backup).await;
    assert_schema(&restored, 100, false).await;
    assert_integrity(&restored).await;
    assert_eq!(history(&restored).await, before);
    restored.close().await;
    assert_schema(&fixture.pool, 101, true).await;
    assert_eq!(history(&fixture.pool).await, before);
    fixture.pool.close().await;
}

#[tokio::test]
async fn cf21_publication_failure_blocks_migration_and_retains_private_snapshot() {
    let fixture = HistoricalDb::version_100().await;
    let before = history(&fixture.pool).await;
    let directory = fixture.backups();
    let mut migration = Box::pin(run_preflight(&fixture.url, Some(&directory)));
    let pending = pause_at_written_snapshot(migration.as_mut(), &directory).await;
    let original_permissions = std::fs::metadata(&directory).unwrap().permissions();
    // Existing snapshot remains readable; creating its final hard link is denied.
    std::fs::set_permissions(&directory, std::fs::Permissions::from_mode(0o500)).unwrap();
    let result = migration.await;
    std::fs::set_permissions(&directory, original_permissions).unwrap();
    let error = result.unwrap_err();
    assert!(
        matches!(&error, MigrationError::BackupFailed(message) if message.starts_with("publish snapshot:")),
        "{error:?}"
    );
    assert_eq!(entries(&directory), [pending.clone()]);
    assert_eq!(
        std::fs::metadata(&pending).unwrap().permissions().mode() & 0o777,
        0o600
    );
    assert_schema(&fixture.pool, 100, false).await;
    assert_eq!(history(&fixture.pool).await, before);
    let restored_pending = open_immutable(&pending).await;
    assert_integrity(&restored_pending).await;
    assert_eq!(history(&restored_pending).await, before);
    restored_pending.close().await;
    fixture.pool.close().await;
}
