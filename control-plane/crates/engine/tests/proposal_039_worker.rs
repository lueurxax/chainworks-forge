use anyhow::Result;
use async_trait::async_trait;
use db::repos::run_continuations::{self as continuations, Continuation, Reservation};
use domain::run_carry_forward::ContentDigest;
use engine::run_carry_forward::{
    MaterializationReceiptV1, PreparationContext, PreparationFinalizer, PreparationJob,
    PreparationLane, PreparationObserver, PreparedManifest, PreviewOptions, WorkspacePlanner,
    WorkspacePreview,
};
use sqlx::SqlitePool;
use std::{
    fs,
    path::Path,
    process::Command,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Duration,
};
use tokio::sync::Notify;
use uuid::Uuid;

fn git(root: &Path, args: &[&str]) -> Vec<u8> {
    let result = Command::new("/usr/bin/git")
        .args([
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(root)
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{args:?}: {}",
        String::from_utf8_lossy(&result.stderr)
    );
    result.stdout
}

struct Fixture {
    root: tempfile::TempDir,
    pool: SqlitePool,
    preview: Arc<WorkspacePreview>,
}

impl Fixture {
    async fn new() -> Self {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("source");
        fs::create_dir(&source).unwrap();
        git(&source, &["init", "-b", "main"]);
        git(&source, &["config", "user.name", "P039 Worker"]);
        git(&source, &["config", "user.email", "worker@example.invalid"]);
        fs::write(source.join("code.txt"), "original\n").unwrap();
        git(&source, &["add", "."]);
        git(&source, &["commit", "-m", "fixture"]);
        fs::write(source.join("code.txt"), "dirty bytes\n").unwrap();
        let preview = Arc::new(
            WorkspacePlanner::preview(&source, PreviewOptions::default())
                .await
                .unwrap(),
        );
        let pool = db::pool::create_pool(&format!(
            "sqlite://{}?mode=rwc",
            root.path().join("fixture.db").display()
        ))
        .await
        .unwrap();
        db::writer::register_shared_writer(
            &pool,
            Arc::new(db::writer::DbWriter::new(pool.clone())),
        )
        .await
        .unwrap();
        Self {
            root,
            pool,
            preview,
        }
    }

    async fn reserve(&self) -> Result<Continuation> {
        let idea = Uuid::new_v4().to_string();
        let source = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Worker','Body','2026-09-20')",
        )
        .bind(&idea)
        .execute(&self.pool)
        .await?;
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','workflow','Workflow','/fixture','/meta','2026-09-20')")
            .bind(&source).bind(&idea).execute(&self.pool).await?;
        continuations::reserve(
            &self.pool,
            &Reservation {
                source_run_id: source,
                caller_fingerprint: "worker-test".into(),
                caller_request_id: Uuid::new_v4().to_string(),
                intent_sha256: "a".repeat(64),
                plan_ref: "plan.json".into(),
                plan_sha256: "b".repeat(64),
                target_ref: "target.json".into(),
                source_witness_sha256: self
                    .preview
                    .digest()
                    .as_str()
                    .strip_prefix("sha256:")
                    .unwrap()
                    .into(),
                reason: "fixture".into(),
            },
        )
        .await
    }

    fn job(&self, op: &Continuation) -> PreparationJob {
        PreparationJob::new(
            Uuid::parse_str(&op.operation_id).unwrap(),
            self.preview.clone(),
            self.root.path().to_path_buf(),
            Arc::new(FixtureFinalizer),
        )
    }

    async fn read(&self, op: &Continuation) -> Continuation {
        continuations::find(&self.pool, &op.operation_id)
            .await
            .unwrap()
            .unwrap()
    }

    async fn expired_reservation(&self) -> Continuation {
        // Seed a historical queued reservation at INSERT time; immutable deadlines stay immutable.
        let idea = Uuid::new_v4().to_string();
        let source = Uuid::new_v4().to_string();
        let id = Uuid::new_v4().to_string();
        sqlx::query(
            "INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Expired','Body','2020-01-01')",
        )
        .bind(&idea)
        .execute(&self.pool)
        .await
        .unwrap();
        sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'blocked','workflow','Workflow','/fixture','/meta','2020-01-01')").bind(&source).bind(&idea).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO command_journal(id,command_type,payload_json,created_at) VALUES (?,'ContinueBlocked','{}','2020-01-01')").bind(&id).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO run_continuations(operation_id,source_run_id,idea_id,reserved_successor_run_id,caller_fingerprint,caller_request_id,intent_sha256,profile,plan_ref,plan_sha256,target_ref,source_witness_sha256,journal_id,created_at,updated_at,deadline_at) VALUES (?,?,?,?,'fixture',?,?,'implementation_restart_v1','plan',?,'target',?,?,'2020-01-01T00:00:00Z','2020-01-01T00:00:00Z','2020-01-01T00:15:00Z')")
            .bind(&id).bind(&source).bind(&idea).bind(Uuid::new_v4().to_string()).bind(&id).bind("a".repeat(64)).bind("b".repeat(64)).bind("c".repeat(64)).bind(&id).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO run_execution_fences(source_run_id,operation_id,generation,disposition,created_at) VALUES (?,?,1,'reserved','2020-01-01T00:00:00Z')").bind(&source).bind(&id).execute(&self.pool).await.unwrap();
        sqlx::query("INSERT INTO work_items(id,kind,payload_json,created_at,scheduled_at) VALUES (?,'prepare_run_continuation',?,'2020-01-01T00:00:00Z','2020-01-01T00:00:00Z')")
            .bind(Uuid::new_v4().to_string()).bind(serde_json::json!({"operation_id":id,"generation":1}).to_string()).execute(&self.pool).await.unwrap();
        continuations::find(&self.pool, &id).await.unwrap().unwrap()
    }
}

struct FixtureFinalizer;

#[async_trait]
impl PreparationFinalizer for FixtureFinalizer {
    async fn finalize(
        &self,
        context: &PreparationContext,
        workspace: &MaterializationReceiptV1,
    ) -> Result<PreparedManifest> {
        // Real independent seed and compiled-target bytes, not an I1 receipt promotion.
        context
            .publish_file(
                &workspace.operation_root,
                "proposal.md",
                b"Selected proposal\n",
            )
            .await?;
        context
            .publish_file(
                &workspace.operation_root,
                "target.json",
                b"{\"entry\":\"fresh_review\"}",
            )
            .await?;
        let preserved_bytes = retained_files(&workspace.preservation_root.join("workspace"))
            .values()
            .map(|bytes| bytes.len() as u64)
            .sum::<u64>()
            + ["index", "staged.patch", "unstaged.patch"]
                .into_iter()
                .map(|name| {
                    fs::metadata(workspace.preservation_root.join(name))
                        .unwrap()
                        .len()
                })
                .sum::<u64>()
            + b"Selected proposal\n".len() as u64;
        let bytes = serde_json::to_vec(&serde_json::json!({
            "schema_version": "run_carry_forward_manifest_v1", "operation_id": workspace.operation_id,
            "preserved_bytes": preserved_bytes,
            "workspace": workspace, "selected_inputs": [{"path":"proposal.md", "sha256":ContentDigest::of(b"Selected proposal\n")}],
            "compiled_target": {"path":"target.json", "sha256":ContentDigest::of(b"{\"entry\":\"fresh_review\"}")},
        }))?;
        context
            .publish_file(&workspace.operation_root, "complete-manifest.json", &bytes)
            .await?;
        Ok(PreparedManifest {
            manifest_ref: workspace.operation_root.join("complete-manifest.json"),
            manifest_sha256: ContentDigest::of(&bytes),
        })
    }
}

struct Pause {
    label: &'static str,
    reached: Notify,
    released: Mutex<bool>,
    release: Condvar,
}

struct CrashCheckpoint {
    pool: SqlitePool,
    operation: String,
    label: &'static str,
    step: &'static str,
    reached: AtomicBool,
}

#[async_trait]
impl PreparationObserver for CrashCheckpoint {
    async fn before_effect(&self, label: &str) -> Result<()> {
        if label == self.label {
            let (status, intent, destination): (String, String, String) = sqlx::query_as("SELECT status,intent_json,destination_identity FROM run_continuation_steps WHERE operation_id=? AND step_key=?")
                .bind(&self.operation).bind(self.step).fetch_one(&self.pool).await?;
            assert_eq!(status, "dispatching", "intent must precede {label}");
            assert!(serde_json::from_str::<serde_json::Value>(&intent)?.is_object());
            assert!(!destination.is_empty());
            self.reached.store(true, Ordering::SeqCst);
            continuations::recover_preparations(&self.pool).await?;
        }
        Ok(())
    }
}

fn retained_files(root: &Path) -> std::collections::BTreeMap<std::path::PathBuf, Vec<u8>> {
    fn walk(
        root: &Path,
        relative: &Path,
        result: &mut std::collections::BTreeMap<std::path::PathBuf, Vec<u8>>,
    ) {
        for item in fs::read_dir(root.join(relative)).unwrap() {
            let item = item.unwrap();
            let path = relative.join(item.file_name());
            if item.file_type().unwrap().is_dir() {
                walk(root, &path, result);
            } else if item.file_type().unwrap().is_symlink() {
                result.insert(
                    path,
                    fs::read_link(item.path())
                        .unwrap()
                        .as_os_str()
                        .as_encoded_bytes()
                        .to_vec(),
                );
            } else {
                result.insert(path, fs::read(item.path()).unwrap());
            }
        }
    }
    let mut result = std::collections::BTreeMap::new();
    if root.exists() {
        walk(root, Path::new(""), &mut result);
    }
    result
}

#[tokio::test]
async fn every_git_copy_and_manifest_boundary_retains_evidence_and_never_replays_after_restart() {
    for (label, step) in [
        (
            "published:complete-manifest.json",
            "selected_inputs_and_target",
        ),
        ("copy_done:archive:code.txt", "preservation"),
        ("copy_done:checkout:code.txt", "workspace_overlay"),
        ("copy:link", "preservation"),
        ("copy_done:archive:link.txt", "preservation"),
        ("copy_done:checkout:link.txt", "workspace_overlay"),
        ("publish:owner.json", "preservation"),
        ("published:owner.json", "preservation"),
        ("publish:manifest.json", "preservation"),
        ("publish_ready:manifest.json", "preservation"),
        ("published:manifest.json", "preservation"),
        ("copy:archive:code.txt", "preservation"),
        ("git:worktree_add", "git_worktree"),
        ("git:worktree_added", "git_worktree"),
        ("git:read_tree", "git_index"),
        ("git:read_tree_completed", "git_index"),
        ("copy:checkout:code.txt", "workspace_overlay"),
        ("publish:verified-receipt.json", "workspace_receipt"),
        ("publish_ready:verified-receipt.json", "workspace_receipt"),
        ("published:verified-receipt.json", "workspace_receipt"),
        (
            "publish:complete-manifest.json",
            "selected_inputs_and_target",
        ),
        (
            "publish_ready:complete-manifest.json",
            "selected_inputs_and_target",
        ),
    ] {
        let mut f = Fixture::new().await;
        std::os::unix::fs::symlink("code.txt", f.preview.source().join("link.txt")).unwrap();
        f.preview = Arc::new(
            WorkspacePlanner::preview(f.preview.source(), PreviewOptions::default())
                .await
                .unwrap(),
        );
        let op = f.reserve().await.unwrap();
        let source_index = fs::read(f.preview.source().join(".git/index")).unwrap();
        let observer = Arc::new(CrashCheckpoint {
            pool: f.pool.clone(),
            operation: op.operation_id.clone(),
            label,
            step,
            reached: AtomicBool::new(false),
        });
        let lane = PreparationLane::new(f.pool.clone());
        let result = lane
            .try_submit(f.job(&op).with_observer(observer.clone()))
            .unwrap()
            .wait()
            .await;
        lane.shutdown().await.unwrap();
        assert!(
            observer.reached.load(Ordering::SeqCst),
            "missing checkpoint {label}"
        );
        assert!(result.is_err(), "stale worker published after {label}");
        let held = f.read(&op).await;
        assert_eq!(held.phase, "needs_reconciliation", "{label}");
        assert!(
            held.worker_claimed,
            "restart cannot prove settlement: {label}"
        );
        assert_eq!(held.generation, op.generation + 1);
        assert!(held.manifest_ref.is_none());
        assert_eq!(
            fs::read(f.preview.source().join(".git/index")).unwrap(),
            source_index
        );
        assert_eq!(
            fs::read(f.preview.source().join("code.txt")).unwrap(),
            b"dirty bytes\n"
        );
        let root = f.root.path().join(&op.operation_id);
        let evidence = retained_files(&root);
        let pin = git(
            f.preview.source(),
            &[
                "branch",
                "--list",
                &format!("carry-forward-{}", op.operation_id),
            ],
        );
        f.pool.close().await;
        let reopened = db::pool::create_pool(&format!(
            "sqlite://{}?mode=rwc",
            f.root.path().join("fixture.db").display()
        ))
        .await
        .unwrap();
        db::writer::register_shared_writer(
            &reopened,
            Arc::new(db::writer::DbWriter::new(reopened.clone())),
        )
        .await
        .unwrap();
        let restart = PreparationLane::new(reopened.clone());
        assert!(restart
            .try_submit(f.job(&op))
            .unwrap()
            .wait()
            .await
            .is_err());
        restart.shutdown().await.unwrap();
        assert_eq!(
            retained_files(&root),
            evidence,
            "restart changed retained bytes: {label}"
        );
        assert_eq!(
            git(
                f.preview.source(),
                &[
                    "branch",
                    "--list",
                    &format!("carry-forward-{}", op.operation_id)
                ]
            ),
            pin
        );
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key=?"
            )
            .bind(&op.operation_id)
            .bind(step)
            .fetch_one(&reopened)
            .await
            .unwrap(),
            "unknown"
        );
        assert_eq!(sqlx::query_scalar::<_,i64>("SELECT attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?").bind(&op.operation_id).fetch_one(&reopened).await.unwrap(), 1);
        assert!(
            continuations::check_source_guard(&reopened, &op.source_run_id)
                .await
                .is_err()
        );
        reopened.close().await;
    }
}

impl Pause {
    fn at(label: &'static str) -> Arc<Self> {
        Arc::new(Self {
            label,
            reached: Notify::new(),
            released: Mutex::new(false),
            release: Condvar::new(),
        })
    }
    async fn reached(&self) {
        tokio::time::timeout(Duration::from_secs(10), self.reached.notified())
            .await
            .expect("worker reached barrier");
    }
    fn resume(&self) {
        *self.released.lock().unwrap() = true;
        self.release.notify_all();
    }
}

#[async_trait]
impl PreparationObserver for Pause {
    async fn before_effect(&self, label: &str) -> Result<()> {
        if label == self.label {
            self.reached.notify_one();
            let (released, _) = self
                .release
                .wait_timeout_while(
                    self.released.lock().unwrap(),
                    Duration::from_secs(10),
                    |v| !*v,
                )
                .unwrap();
            anyhow::ensure!(
                *released,
                "copy barrier timed out: blocking work stalled executor"
            );
        }
        Ok(())
    }
}

#[tokio::test]
async fn complete_manifest_is_required_before_prepared() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let result = lane.try_submit(f.job(&op)).unwrap().wait().await;
    lane.shutdown().await.unwrap();
    assert!(
        result.is_ok(),
        "preparation must publish the complete fixture manifest: {result:?}"
    );
    let prepared = result.unwrap();
    assert_eq!(prepared.phase, "prepared");
    assert!(prepared
        .manifest_ref
        .unwrap()
        .ends_with("complete-manifest.json"));
}

struct WorkspaceOnlyFinalizer;

#[async_trait]
impl PreparationFinalizer for WorkspaceOnlyFinalizer {
    async fn finalize(
        &self,
        _: &PreparationContext,
        workspace: &MaterializationReceiptV1,
    ) -> Result<PreparedManifest> {
        let path = workspace.operation_root.join("verified-receipt.json");
        Ok(PreparedManifest {
            manifest_sha256: ContentDigest::of(&fs::read(&path)?),
            manifest_ref: path,
        })
    }
}

#[tokio::test]
async fn workspace_only_receipt_cannot_be_promoted_by_finalizer() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let job = PreparationJob::new(
        Uuid::parse_str(&op.operation_id).unwrap(),
        f.preview.clone(),
        f.root.path().to_path_buf(),
        Arc::new(WorkspaceOnlyFinalizer),
    );
    assert!(lane
        .try_submit(job)
        .unwrap()
        .wait()
        .await
        .unwrap_err()
        .to_string()
        .contains("complete_manifest_required"));
    lane.shutdown().await.unwrap();
    let held = f.read(&op).await;
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(held.manifest_ref.is_none());
}

#[tokio::test]
async fn abort_after_finalizer_wait_cannot_publish_manifest_or_prepared() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("publish_ready:complete-manifest.json");
    let ticket = lane
        .try_submit(f.job(&op).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    continuations::begin_abort(&f.pool, &op.operation_id, f.read(&op).await.version)
        .await
        .unwrap();
    pause.resume();
    assert!(ticket.wait().await.is_err());
    lane.shutdown().await.unwrap();
    let held = f.read(&op).await;
    assert_eq!(held.phase, "aborting");
    assert!(held.manifest_ref.is_none());
    assert!(!f
        .root
        .path()
        .join(&op.operation_id)
        .join("complete-manifest.json")
        .exists());
    assert_eq!(
        fs::read(f.root.path().join(&op.operation_id).join("proposal.md")).unwrap(),
        b"Selected proposal\n"
    );
}

#[tokio::test]
async fn crash_after_real_git_effect_retains_pin_and_unknown_without_replay() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("git:worktree_added");
    let ticket = lane
        .try_submit(f.job(&op).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    let branch = format!("carry-forward-{}", op.operation_id);
    assert!(!git(f.preview.source(), &["branch", "--list", &branch]).is_empty());
    continuations::recover_preparations(&f.pool).await.unwrap();
    pause.resume();
    assert!(ticket.wait().await.is_err());
    lane.shutdown().await.unwrap();
    let restart = PreparationLane::new(f.pool.clone());
    assert!(restart
        .try_submit(f.job(&op))
        .unwrap()
        .wait()
        .await
        .is_err());
    restart.shutdown().await.unwrap();
    assert!(!git(f.preview.source(), &["branch", "--list", &branch]).is_empty());
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), 1);
    assert_eq!(sqlx::query_scalar::<_, String>("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key='git_worktree'").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "unknown");
    assert!(f.read(&op).await.manifest_ref.is_none());
}

#[tokio::test]
async fn cooperative_cancellation_does_not_release_source_or_allow_next_effect() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("copy:archive:code.txt");
    let ticket = lane
        .try_submit(f.job(&op).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    ticket.cancel();
    pause.resume();
    assert!(ticket
        .wait()
        .await
        .unwrap_err()
        .to_string()
        .contains("continuation_worker_cancelled"));
    lane.shutdown().await.unwrap();
    let held = f.read(&op).await;
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(!held.worker_claimed);
    assert!(!f
        .root
        .path()
        .join(&op.operation_id)
        .join("preservation/workspace/code.txt")
        .exists());
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
}

struct UnknownFinalizer;

#[async_trait]
impl PreparationFinalizer for UnknownFinalizer {
    async fn finalize(
        &self,
        context: &PreparationContext,
        workspace: &MaterializationReceiptV1,
    ) -> Result<PreparedManifest> {
        context
            .publish_file(
                &workspace.operation_root,
                "retained-evidence",
                b"uncertain outcome",
            )
            .await?;
        anyhow::bail!("effect_outcome_unknown: fixture could not prove settlement")
    }
}

#[tokio::test]
async fn unknown_outcome_retains_claim_and_records_hold_without_claiming_settlement() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let job = PreparationJob::new(
        Uuid::parse_str(&op.operation_id).unwrap(),
        f.preview.clone(),
        f.root.path().to_path_buf(),
        Arc::new(UnknownFinalizer),
    );
    assert!(lane.try_submit(job).unwrap().wait().await.is_err());
    lane.shutdown().await.unwrap();
    let held = f.read(&op).await;
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(held.worker_claimed);
    assert_eq!(held.generation, op.generation);
    assert!(held.holds_json.contains("effect_outcome_unknown"));
    assert_eq!(sqlx::query_scalar::<_, String>("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key='selected_inputs_and_target'").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "unknown");
    assert_eq!(sqlx::query_scalar::<_, String>("SELECT status FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "cancelled");
    assert_eq!(
        fs::read(
            f.root
                .path()
                .join(&op.operation_id)
                .join("retained-evidence")
        )
        .unwrap(),
        b"uncertain outcome"
    );
    assert!(
        continuations::claim_preparation_for(&f.pool, &op.operation_id)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn queued_hold_requires_current_unclaimed_generation_and_retains_fence() {
    let f = Fixture::new().await;
    let queued = f.reserve().await.unwrap();
    for (generation, reason) in [
        (queued.generation + 1, "effect_outcome_unknown"),
        (queued.generation, "unbounded_private_reason"),
    ] {
        assert!(continuations::hold_queued_preparation(
            &f.pool,
            &queued.operation_id,
            generation,
            reason
        )
        .await
        .is_err());
        let unchanged = f.read(&queued).await;
        assert_eq!(unchanged.phase, "preparing");
        assert_eq!(unchanged.version, queued.version);
        assert_eq!(unchanged.generation, queued.generation);
        assert!(!unchanged.worker_claimed);
    }
    continuations::hold_queued_preparation(
        &f.pool,
        &queued.operation_id,
        queued.generation,
        "effect_outcome_unknown",
    )
    .await
    .unwrap();
    let held = f.read(&queued).await;
    assert_eq!(held.phase, "needs_reconciliation");
    assert_eq!(held.version, queued.version + 1);
    assert_eq!(held.generation, queued.generation);
    assert!(!held.worker_claimed);
    assert!(
        continuations::check_source_guard(&f.pool, &held.source_run_id)
            .await
            .is_err()
    );
    assert!(continuations::hold_queued_preparation(
        &f.pool,
        &held.operation_id,
        held.generation,
        "effect_outcome_unknown"
    )
    .await
    .is_err());
    assert_eq!(f.read(&held).await.version, held.version);

    let claimed = f.reserve().await.unwrap();
    let owner = continuations::claim_preparation_for(&f.pool, &claimed.operation_id)
        .await
        .unwrap()
        .unwrap();
    assert!(continuations::hold_queued_preparation(
        &f.pool,
        &owner.operation_id,
        owner.generation,
        "effect_outcome_unknown"
    )
    .await
    .is_err());
    let unchanged = f.read(&owner).await;
    assert_eq!(unchanged.phase, "preparing");
    assert_eq!(unchanged.version, owner.version);
    assert!(unchanged.worker_claimed);
    let queue: (String, i64) = sqlx::query_as("SELECT status,attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?")
        .bind(&owner.operation_id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(queue, ("running".into(), 1));
}

#[tokio::test]
async fn queued_jobs_after_unknown_owner_are_durably_held_without_effects() {
    let f = Fixture::new().await;
    let first = f.reserve().await.unwrap();
    let queued = [
        f.reserve().await.unwrap(),
        f.reserve().await.unwrap(),
        f.reserve().await.unwrap(),
    ];
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("copy:archive:code.txt");
    let job = PreparationJob::new(
        Uuid::parse_str(&first.operation_id).unwrap(),
        f.preview.clone(),
        f.root.path().to_path_buf(),
        Arc::new(UnknownFinalizer),
    )
    .with_observer(pause.clone());
    let first_ticket = lane.try_submit(job).unwrap();
    pause.reached().await;
    let mut tickets = Vec::new();
    for op in &queued {
        tickets.push(lane.try_submit(f.job(op)).unwrap());
    }
    pause.resume();
    assert!(first_ticket.wait().await.is_err());
    for ticket in tickets {
        assert!(ticket.wait().await.is_err());
    }
    lane.shutdown().await.unwrap();
    let owner = f.read(&first).await;
    assert_eq!(owner.phase, "needs_reconciliation");
    assert!(
        owner.worker_claimed,
        "no guessed settlement of uncertain effects"
    );
    for op in &queued {
        let held = f.read(op).await;
        assert_eq!(
            held.phase, "needs_reconciliation",
            "accepted job lost without a durable outcome"
        );
        assert!(!held.worker_claimed);
        assert!(held.holds_json.contains("effect_outcome_unknown"));
        assert!(held.manifest_ref.is_none());
        assert!(!f.root.path().join(&op.operation_id).exists());
        assert!(
            continuations::check_source_guard(&f.pool, &op.source_run_id)
                .await
                .is_err()
        );
        let queue: (String,i64) = sqlx::query_as("SELECT status,attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?")
            .bind(&op.operation_id).fetch_one(&f.pool).await.unwrap();
        assert_eq!(queue, ("cancelled".into(), 0));
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "SELECT COUNT(*) FROM run_continuation_steps WHERE operation_id=?"
            )
            .bind(&op.operation_id)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
            0
        );
    }
}

#[tokio::test]
async fn dropped_lane_durably_holds_queued_job_without_claiming_or_copying() {
    let f = Fixture::new().await;
    let active = f.reserve().await.unwrap();
    let queued = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("copy:archive:code.txt");
    let active_ticket = lane
        .try_submit(f.job(&active).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    let queued_ticket = lane.try_submit(f.job(&queued)).unwrap();
    drop(lane);
    pause.resume();
    assert!(active_ticket.wait().await.is_err());
    assert!(queued_ticket.wait().await.is_err());
    let held = f.read(&queued).await;
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(!held.worker_claimed);
    assert_eq!(held.generation, queued.generation);
    assert!(held.holds_json.contains("effect_outcome_unknown"));
    assert!(held.manifest_ref.is_none());
    assert!(!f.root.path().join(&queued.operation_id).exists());
    assert!(
        continuations::check_source_guard(&f.pool, &queued.source_run_id)
            .await
            .is_err()
    );
    let queue: (String, i64) = sqlx::query_as("SELECT status,attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?")
        .bind(&queued.operation_id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(queue, ("cancelled".into(), 0));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuation_steps WHERE operation_id=?"
        )
        .bind(&queued.operation_id)
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        0
    );
}

#[tokio::test]
async fn new_reservation_is_rejected_while_global_unknown_worker_retains_claim() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let job = PreparationJob::new(
        Uuid::parse_str(&op.operation_id).unwrap(),
        f.preview.clone(),
        f.root.path().to_path_buf(),
        Arc::new(UnknownFinalizer),
    );
    assert!(lane.try_submit(job).unwrap().wait().await.is_err());
    lane.shutdown().await.unwrap();
    let result = f.reserve().await;
    assert!(
        result.is_err(),
        "accepted a new reservation without an available owner"
    );
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("effect_outcome_unknown"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_continuations")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_execution_fences")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        1
    );
}

#[tokio::test]
async fn manifest_change_during_prepared_publication_wait_is_rejected() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("prepared:publish");
    let ticket = lane
        .try_submit(f.job(&op).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    fs::write(
        f.root
            .path()
            .join(&op.operation_id)
            .join("complete-manifest.json"),
        b"changed after verification",
    )
    .unwrap();
    pause.resume();
    let result = ticket.wait().await;
    lane.shutdown().await.unwrap();
    assert!(result.is_err(), "changed manifest was promoted: {result:?}");
    assert!(result
        .unwrap_err()
        .to_string()
        .contains("prepared_material_changed"));
    assert!(f.read(&op).await.manifest_ref.is_none());
}

#[tokio::test]
async fn paused_copy_is_off_executor_and_capacity_is_one_active_plus_three_queued() {
    let f = Fixture::new().await;
    let lane = PreparationLane::new(f.pool.clone());
    let first = f.reserve().await.unwrap();
    let pause = Pause::at("copy:archive:code.txt");
    let mut tickets = vec![lane
        .try_submit(f.job(&first).with_observer(pause.clone()))
        .unwrap()];
    pause.reached().await;
    for _ in 0..3 {
        let op = f.reserve().await.unwrap();
        tickets.push(lane.try_submit(f.job(&op)).unwrap());
    }
    assert!(lane
        .try_submit(f.job(&first))
        .err()
        .unwrap()
        .to_string()
        .contains("continuation_capacity"));
    assert!(f
        .reserve()
        .await
        .unwrap_err()
        .to_string()
        .contains("continuation_capacity"));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM run_execution_fences")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        4
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM run_continuations WHERE worker_claimed=1"
        )
        .fetch_one(&f.pool)
        .await
        .unwrap(),
        1
    );
    let idea = Uuid::new_v4().to_string();
    let run = domain::ids::RunId::new();
    let stage = Uuid::new_v4().to_string();
    let normal_root = f.root.path().join("ordinary");
    fs::create_dir(&normal_root).unwrap();
    sqlx::query("INSERT INTO ideas(id,title,body,created_at) VALUES (?,'Ordinary','Independent idea','2026-09-20T00:00:00Z')")
        .bind(&idea).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO runs(id,idea_id,status,workflow_id,workflow_title,workspace_root,artifact_root,started_at) VALUES (?,?,'pending','flat','Ordinary',?,?,'2026-09-20T00:00:00Z')")
        .bind(run.to_string()).bind(idea).bind(normal_root.to_str().unwrap()).bind(normal_root.join("meta").to_str().unwrap()).execute(&f.pool).await.unwrap();
    sqlx::query("INSERT INTO stage_executions(id,run_id,stage_id,label,status,iteration,attempt_number,started_at,provider,owner_agent) VALUES (?,?,'prepare','Prepare','pending',0,1,'2026-09-20T00:00:00Z','codex','fixture')")
        .bind(&stage).bind(run.to_string()).execute(&f.pool).await.unwrap();
    let events = engine::event_bus::new_bus(16);
    let queue = engine::work_queue::WorkQueue::new(f.pool.clone());
    // The queue's indexed due-time cutoff intentionally lags by one second.
    let due = chrono::Utc::now() - chrono::Duration::seconds(5);
    db::repos::work_items::enqueue(
        &f.pool,
        &db::work_item::WorkItem {
            id: "ordinary".into(),
            kind: db::work_item::WorkItemKind::AdvanceRun,
            payload_json: serde_json::json!({"run_id":run}).to_string(),
            status: db::work_item::WorkItemStatus::Pending,
            run_id: Some(run),
            stage_id: None,
            created_at: due,
            scheduled_at: due,
            attempt_count: 0,
            last_error: None,
        },
    )
    .await
    .unwrap();
    let orchestrator = Arc::new(engine::orchestrator::Orchestrator::new(
        f.pool.clone(),
        events.clone(),
        queue.clone(),
    ));
    let executor = engine::executor::BackgroundExecutor::new(
        f.pool.clone(),
        queue,
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );
    assert!(
        tokio::time::timeout(Duration::from_secs(5), executor.process_next_item())
            .await
            .unwrap()
            .unwrap()
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM work_items WHERE id='ordinary'")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "completed"
    );
    assert_eq!(
        db::repos::runs::find_by_id(&f.pool, run)
            .await
            .unwrap()
            .unwrap()
            .status,
        domain::run::RunStatus::Running
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("SELECT status FROM stage_executions WHERE id=?")
            .bind(&stage)
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        "running"
    );
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items WHERE run_id=? AND kind='invoke_agent' AND status='pending'")
        .bind(run.to_string()).fetch_one(&f.pool).await.unwrap(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM provider_sessions")
            .fetch_one(&f.pool)
            .await
            .unwrap(),
        0
    );
    assert!(
        !*pause.released.lock().unwrap(),
        "unrelated run must advance before copy resumes"
    );
    pause.resume();
    for ticket in tickets {
        assert_eq!(ticket.wait().await.unwrap().phase, "prepared");
    }
    lane.shutdown().await.unwrap();
    let prepared = f.read(&first).await;
    assert!(!prepared.worker_claimed);
    let manifest = fs::read(prepared.manifest_ref.unwrap()).unwrap();
    assert_eq!(
        ContentDigest::of(&manifest),
        ContentDigest::from_sha256_hex(&prepared.manifest_sha256.unwrap()).unwrap()
    );
    assert_eq!(
        fs::read(
            f.root
                .path()
                .join(&first.operation_id)
                .join("checkout/code.txt")
        )
        .unwrap(),
        b"dirty bytes\n"
    );
    assert_eq!(
        fs::read(f.root.path().join(&first.operation_id).join("proposal.md")).unwrap(),
        b"Selected proposal\n"
    );
}

#[tokio::test]
async fn abort_during_copy_revokes_next_effect_and_cannot_publish() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("copy:archive:code.txt");
    let ticket = lane
        .try_submit(f.job(&op).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    continuations::begin_abort(&f.pool, &op.operation_id, f.read(&op).await.version)
        .await
        .unwrap();
    pause.resume();
    assert!(ticket
        .wait()
        .await
        .unwrap_err()
        .to_string()
        .contains("continuation_stale_worker"));
    lane.shutdown().await.unwrap();
    let held = f.read(&op).await;
    assert_eq!(held.phase, "aborting");
    assert!(!held.worker_claimed);
    assert!(held.manifest_ref.is_none());
    assert!(!f
        .root
        .path()
        .join(&op.operation_id)
        .join("preservation/workspace/code.txt")
        .exists());
    assert!(!f
        .root
        .path()
        .join(&op.operation_id)
        .join("checkout")
        .exists());
    assert!(
        continuations::finish_abort(&f.pool, &op.operation_id, held.version)
            .await
            .is_err()
    );
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn abort_between_verified_git_steps_prevents_next_git_effect_and_keeps_evidence() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let index = fs::read(f.preview.source().join(".git/index")).unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("git:read_tree");
    let ticket = lane
        .try_submit(f.job(&op).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    let completed: String = sqlx::query_scalar("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key='git_worktree'")
        .bind(&op.operation_id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(completed, "verified");
    let checkout = f.root.path().join(&op.operation_id).join("checkout");
    let next_index = std::path::PathBuf::from(
        String::from_utf8(git(&checkout, &["rev-parse", "--git-path", "index"]))
            .unwrap()
            .trim(),
    );
    assert!(!next_index.exists(), "read-tree has not dispatched");
    continuations::begin_abort(&f.pool, &op.operation_id, f.read(&op).await.version)
        .await
        .unwrap();
    pause.resume();
    assert!(ticket
        .wait()
        .await
        .unwrap_err()
        .to_string()
        .contains("continuation_stale_worker"));
    lane.shutdown().await.unwrap();
    let held = f.read(&op).await;
    assert_eq!(held.phase, "aborting");
    assert!(!held.worker_claimed);
    assert!(!next_index.exists(), "the next Git effect ran after abort");
    assert!(!checkout.join("code.txt").exists());
    assert_eq!(
        fs::read(f.preview.source().join(".git/index")).unwrap(),
        index
    );
    assert_eq!(
        fs::read(
            f.root
                .path()
                .join(&op.operation_id)
                .join("preservation/workspace/code.txt")
        )
        .unwrap(),
        b"dirty bytes\n"
    );
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key='git_worktree'").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "verified");
    assert_eq!(sqlx::query_scalar::<_,String>("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key='git_index'").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "unknown");
    assert!(
        continuations::finish_abort(&f.pool, &op.operation_id, held.version)
            .await
            .is_err()
    );
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn intent_is_durable_before_git_and_restart_never_repeats_unknown_effects() {
    let f = Fixture::new().await;
    let op = f.reserve().await.unwrap();
    let queued = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("git:worktree_add");
    let ticket = lane
        .try_submit(f.job(&op).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    let queued_ticket = lane.try_submit(f.job(&queued)).unwrap();
    let step: (String, String) = sqlx::query_as("SELECT status,intent_json FROM run_continuation_steps WHERE operation_id=? AND step_key='git_worktree'")
        .bind(&op.operation_id).fetch_one(&f.pool).await.unwrap();
    assert_eq!(step.0, "dispatching");
    assert!(step.1.contains(f.preview.head()));
    assert!(git(
        f.preview.source(),
        &[
            "branch",
            "--list",
            &format!("carry-forward-{}", op.operation_id)
        ]
    )
    .is_empty());
    continuations::recover_preparations(&f.pool).await.unwrap();
    pause.resume();
    assert!(ticket.wait().await.is_err());
    assert!(queued_ticket.wait().await.is_err());
    lane.shutdown().await.unwrap();
    assert_eq!(f.read(&op).await.phase, "needs_reconciliation");
    assert!(
        f.read(&op).await.worker_claimed,
        "restart alone cannot prove child settlement"
    );
    let restart = PreparationLane::new(f.pool.clone());
    assert!(restart
        .try_submit(f.job(&op))
        .unwrap()
        .wait()
        .await
        .is_err());
    restart.shutdown().await.unwrap();
    assert_eq!(sqlx::query_scalar::<_, String>("SELECT status FROM run_continuation_steps WHERE operation_id=? AND step_key='git_worktree'").bind(&op.operation_id).fetch_one(&f.pool).await.unwrap(), "unknown");
    assert!(!f
        .root
        .path()
        .join(&op.operation_id)
        .join("checkout")
        .exists());
    assert!(!f.root.path().join(&queued.operation_id).exists());
    assert!(
        continuations::check_source_guard(&f.pool, &op.source_run_id)
            .await
            .is_err()
    );
}

#[tokio::test]
async fn stored_deadline_includes_queue_wait_and_claim_is_for_exact_job() {
    let f = Fixture::new().await;
    let unsubmitted = f.reserve().await.unwrap();
    let first = f.reserve().await.unwrap();
    let queued = f.expired_reservation().await;
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("copy:archive:code.txt");
    let first_ticket = lane
        .try_submit(f.job(&first).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    assert!(!f.read(&unsubmitted).await.worker_claimed);
    let queued_ticket = lane.try_submit(f.job(&queued)).unwrap();
    pause.resume();
    assert_eq!(first_ticket.wait().await.unwrap().phase, "prepared");
    assert!(queued_ticket.wait().await.is_err());
    lane.shutdown().await.unwrap();
    let held = f.read(&queued).await;
    assert_eq!(held.phase, "needs_reconciliation");
    assert!(held.holds_json.contains("continuation_budget_exceeded"));
    assert!(!f.root.path().join(&queued.operation_id).exists());
    assert!(!f.root.path().join(&unsubmitted.operation_id).exists());
}

#[tokio::test]
async fn queued_abort_revokes_job_before_claim_and_ticket_loss_does_not_cancel_accepted_work() {
    let f = Fixture::new().await;
    let first = f.reserve().await.unwrap();
    let queued = f.reserve().await.unwrap();
    let lane = PreparationLane::new(f.pool.clone());
    let pause = Pause::at("copy:archive:code.txt");
    let first_ticket = lane
        .try_submit(f.job(&first).with_observer(pause.clone()))
        .unwrap();
    pause.reached().await;
    let queued_ticket = lane.try_submit(f.job(&queued)).unwrap();
    let abort = continuations::begin_abort(&f.pool, &queued.operation_id, queued.version)
        .await
        .unwrap();
    continuations::finish_abort(&f.pool, &queued.operation_id, abort.version)
        .await
        .unwrap();
    drop(first_ticket);
    pause.resume();
    assert!(queued_ticket.wait().await.is_err());
    lane.shutdown().await.unwrap();
    assert_eq!(f.read(&first).await.phase, "prepared");
    assert_eq!(f.read(&queued).await.phase, "aborted");
    assert!(!f.root.path().join(&queued.operation_id).exists());
    assert_eq!(sqlx::query_scalar::<_, i64>("SELECT attempt_count FROM work_items WHERE kind='prepare_run_continuation' AND json_extract(payload_json,'$.operation_id')=?").bind(&queued.operation_id).fetch_one(&f.pool).await.unwrap(), 0);
}
