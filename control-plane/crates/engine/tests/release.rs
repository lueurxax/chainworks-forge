use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use chrono::Utc;
use db::pool::create_pool;
use db::repos::{agent_executions, artifacts, ideas, runs, stages};
use db::writer::{register_shared_writer, DbWriter};
use domain::agent::AgentStatus;
use domain::artifact::ArtifactFormat;
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::run::{DeliveryConfiguration, Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
use engine::event_bus;
use engine::executor::BackgroundExecutor;
use engine::orchestrator::Orchestrator;
use engine::release::connect::ConnectPublishService;
use engine::release::git::GitReleaseService;
use engine::release::receipt::DeliveryReceiptBuilder;
use engine::work_queue::WorkQueue;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:")
        .await
        .expect("in-memory pool failed");
    register_shared_writer(&pool, Arc::new(DbWriter::new(pool.clone())))
        .await
        .expect("register shared writer");
    pool
}

fn enable_release_side_effects() {
    static ENABLE: std::sync::Once = std::sync::Once::new();
    ENABLE.call_once(|| unsafe {
        std::env::set_var("CHAINWORKS_RELEASE_SIDE_EFFECTS_ENABLED", "true");
    });
}

fn make_idea(id: IdeaId) -> Idea {
    Idea {
        id,
        title: "Release idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        project_key: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    }
}

fn make_run(id: RunId, idea_id: IdeaId, workspace_root: &str, artifact_root: &str) -> Run {
    Run {
        id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-release".into(),
        workflow_title: "Release Workflow".into(),
        workspace_root: workspace_root.into(),
        artifact_root: artifact_root.into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: None,
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: Some(workspace_root.into()),
        base_branch: Some("main".into()),
        base_revision: None,
        target_branch: Some("release/test".into()),
        delivery_configuration_json: Some(
            serde_json::to_string(&DeliveryConfiguration {
                repo_identifier: "repo/test".into(),
                repo_root: workspace_root.into(),
                base_branch: "main".into(),
                worktree_base_path: workspace_root.into(),
                target_branch: "release/test".into(),
                release_target_id: Some("sandbox-target".into()),
                release_mode: Some("sandbox".into()),
            })
            .expect("delivery config json"),
        ),
        delivery_preflight_json: None,
        workflow_family: None,
        project_key: None,
        risk_class: None,
        stack: None,
        workflow_snapshot_hash: None,
        catalog_snapshot_hash: None,
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
        review_routing_json: None,
        closeout_readiness_mode: None,
    }
}

fn make_stage(id: StageExecutionId, run_id: RunId, stage_id: &str) -> StageExecution {
    StageExecution {
        id,
        run_id,
        stage_id: stage_id.into(),
        label: "Release stage".into(),
        status: StageStatus::Running,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: None,
        started_at: Utc::now(),
        completed_at: None,
        owner_agent: Some(stage_id.into()),
        provider: Some("claude".into()),
        model: Some("test".into()),
        stage_type: Some("release".into()),
        validation_failure_json: None,
        evidence_packet_json: None,
        recovery_snapshot_json: None,
        retry_reason: None,
    }
}

fn git(dir: &Path, args: &[&str]) {
    let status = Command::new("git")
        .current_dir(dir)
        .args(args)
        .status()
        .expect("git command");
    assert!(status.success(), "git {:?} failed", args);
}

fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(dir)
        .args(args)
        .output()
        .expect("git command");
    assert!(output.status.success(), "git {:?} failed", args);
    String::from_utf8(output.stdout).expect("utf8 git output")
}

fn init_release_repo_on_branch(root: &Path, branch: &str) -> (String, String) {
    let repo_dir = root.join("repo");
    let remote_dir = root.join("origin.git");
    std::fs::create_dir_all(&repo_dir).unwrap();
    git(root, &["init", "--bare", "origin.git"]);
    git(&repo_dir, &["init"]);
    git(&repo_dir, &["config", "user.email", "release@test.local"]);
    git(&repo_dir, &["config", "user.name", "Release Tester"]);
    git(&repo_dir, &["checkout", "-b", branch]);
    std::fs::write(repo_dir.join("README.md"), "initial\n").unwrap();
    git(&repo_dir, &["add", "-A"]);
    git(&repo_dir, &["commit", "-m", "initial"]);
    git(
        &repo_dir,
        &["remote", "add", "origin", remote_dir.to_str().unwrap()],
    );
    std::fs::write(repo_dir.join("release.txt"), "changed\n").unwrap();
    (
        repo_dir.to_string_lossy().into_owned(),
        remote_dir.to_string_lossy().into_owned(),
    )
}

fn init_release_repo(root: &Path) -> (String, String) {
    init_release_repo_on_branch(root, "release/test")
}

fn repo_root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("repo root")
        .to_path_buf()
}

fn configure_release_plan_paths(run: &mut Run) {
    let repo_root = repo_root();
    run.workflow_yaml_path = Some(
        repo_root
            .join("examples/workflows/full-mvp-live.yaml")
            .to_string_lossy()
            .into_owned(),
    );
    run.agent_catalog_yaml_path = Some(
        repo_root
            .join("examples/agents/agents.yaml")
            .to_string_lossy()
            .into_owned(),
    );
}

async fn persist_json_artifact<T: serde::Serialize>(
    pool: &sqlx::SqlitePool,
    run_id: RunId,
    stage_id: &str,
    agent_id: &str,
    artifact_root: &Path,
    name: &str,
    value: &T,
) {
    let file_path = artifact_root.join(format!("{name}.json"));
    std::fs::create_dir_all(artifact_root).unwrap();
    std::fs::write(&file_path, serde_json::to_string_pretty(value).unwrap()).unwrap();
    artifacts::insert(
        pool,
        &domain::artifact::Artifact {
            id: domain::ids::ArtifactId::new(),
            run_id,
            stage_id: stage_id.into(),
            agent_id: agent_id.into(),
            name: name.into(),
            contract_id: name.into(),
            format: ArtifactFormat::Json,
            file_path: file_path.to_string_lossy().into_owned(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        },
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn git_release_service_commits_and_pushes_to_expected_branch() {
    let tmp = tempfile::tempdir().unwrap();
    let (repo_dir, remote_dir) = init_release_repo(tmp.path());

    let service = GitReleaseService;
    let (manifest, receipt) = service
        .commit_and_push(&repo_dir, "release/test", "release commit")
        .await
        .expect("git release should succeed");

    assert_eq!(manifest.branch, "release/test");
    assert_eq!(receipt.status, "success");
    assert_eq!(receipt.branch, "release/test");
    assert_eq!(receipt.commit_sha, manifest.commit_sha);
    assert!(!manifest.commit_sha.is_empty());
    assert!(manifest.files_changed >= 1);
    assert!(manifest.insertions >= 1);
    assert!(Path::new(&remote_dir).exists());

    let remote_sha = git_output(
        Path::new(&remote_dir),
        &["rev-parse", "refs/heads/release/test"],
    );
    assert_eq!(remote_sha.trim(), manifest.commit_sha);
}

#[tokio::test]
async fn git_release_service_rejects_main_and_master_targets() {
    let tmp = tempfile::tempdir().unwrap();
    let (main_repo_dir, _remote_dir) = init_release_repo_on_branch(tmp.path(), "main");
    let (master_repo_dir, _remote_dir2) =
        init_release_repo_on_branch(&tmp.path().join("master"), "master");

    let service = GitReleaseService;
    let main_error = service
        .commit_and_push(&main_repo_dir, "main", "release commit")
        .await
        .expect_err("main must be rejected");
    assert!(main_error
        .to_string()
        .contains("push target 'main' is not allowed"));

    let master_error = service
        .commit_and_push(&master_repo_dir, "master", "release commit")
        .await
        .expect_err("master must be rejected");
    assert!(master_error
        .to_string()
        .contains("push target 'master' is not allowed"));
}

#[tokio::test]
async fn connect_publish_service_creates_receipts_without_failing_on_missing_xcodeproj() {
    let tmp = tempfile::tempdir().unwrap();
    let worktree_root = tmp.path().join("worktree");
    std::fs::create_dir_all(&worktree_root).unwrap();
    std::fs::write(worktree_root.join("artifact.txt"), "hello").unwrap();

    let git_receipt = engine::release::git::GitPushReceipt {
        commit_sha: "abc123".into(),
        branch: "release/test".into(),
        remote: "origin".into(),
        status: "success".into(),
        failure_reason: None,
        timestamp: Utc::now(),
    };
    let manifest = engine::release::git::ReleaseManifest {
        commit_sha: "abc123".into(),
        branch: "release/test".into(),
        remote: "origin".into(),
        commit_message: "release commit".into(),
        files_changed: 1,
        insertions: 1,
        deletions: 0,
        timestamp: Utc::now(),
    };
    let delivery_config = DeliveryConfiguration {
        repo_identifier: "repo/test".into(),
        repo_root: worktree_root.to_string_lossy().into_owned(),
        base_branch: "main".into(),
        worktree_base_path: worktree_root.to_string_lossy().into_owned(),
        target_branch: "release/test".into(),
        release_target_id: Some("sandbox-target".into()),
        release_mode: Some("sandbox".into()),
    };

    let service = ConnectPublishService;
    let (bundle, receipt) = service
        .build_and_distribute(
            &worktree_root.to_string_lossy(),
            &git_receipt,
            &manifest,
            &delivery_config,
        )
        .await
        .expect("publish service should succeed in safe mode");

    assert_eq!(receipt.release_target_id, "sandbox-target");
    assert_eq!(receipt.release_mode, "sandbox");
    assert_eq!(receipt.destination, "sandbox://sandbox-target");
    assert!(receipt.status == "success" || receipt.status == "build_warning");
    assert_eq!(bundle.build_number, "abc123");
    assert!(!bundle.checksum_sha256.is_empty());
}

#[tokio::test]
async fn delivery_receipt_builder_rejects_metadata_only_backfill_without_release_lineage() {
    let tmp = tempfile::tempdir().unwrap();
    let workspace_root = tmp.path().join("workspace");
    std::fs::create_dir_all(&workspace_root).unwrap();
    let run = Run {
        id: RunId::new(),
        idea_id: IdeaId::new(),
        status: RunStatus::Running,
        workflow_id: "wf-release".into(),
        workflow_title: "Release Workflow".into(),
        workspace_root: workspace_root.to_string_lossy().into_owned(),
        artifact_root: workspace_root.to_string_lossy().into_owned(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: None,
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: Some(workspace_root.to_string_lossy().into_owned()),
        base_branch: Some("main".into()),
        base_revision: Some("base-rev".into()),
        target_branch: Some("release/test".into()),
        delivery_configuration_json: Some(
            serde_json::to_string(&DeliveryConfiguration {
                repo_identifier: "repo/test".into(),
                repo_root: workspace_root.to_string_lossy().into_owned(),
                base_branch: "main".into(),
                worktree_base_path: workspace_root.to_string_lossy().into_owned(),
                target_branch: "release/test".into(),
                release_target_id: Some("sandbox-target".into()),
                release_mode: Some("sandbox".into()),
            })
            .unwrap(),
        ),
        delivery_preflight_json: None,
        workflow_family: None,
        project_key: None,
        risk_class: None,
        stack: None,
        workflow_snapshot_hash: None,
        catalog_snapshot_hash: None,
        workflow_snapshot_json: None,
        catalog_snapshot_json: None,
        drift_detected_at: None,
        drift_details_json: None,
        chainworks_meta_root: None,
        review_routing_json: None,
        closeout_readiness_mode: None,
    };
    let delivery_config = DeliveryConfiguration {
        repo_identifier: "repo/test".into(),
        repo_root: workspace_root.to_string_lossy().into_owned(),
        base_branch: "main".into(),
        worktree_base_path: workspace_root.to_string_lossy().into_owned(),
        target_branch: "release/test".into(),
        release_target_id: Some("sandbox-target".into()),
        release_mode: Some("sandbox".into()),
    };

    let receipt = DeliveryReceiptBuilder::build_receipt(
        &run,
        &delivery_config,
        None,
        None,
        None,
        "Release idea",
        None,
    );

    assert!(
        receipt.is_none(),
        "metadata-only receipt must not be synthesized"
    );
}

#[tokio::test]
async fn background_executor_routes_release_agents_natively() {
    enable_release_side_effects();
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (repo_dir, _remote_dir) = init_release_repo(tmp.path());
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(
        run_id,
        idea_id,
        &repo_dir,
        artifact_root.to_string_lossy().as_ref(),
    );
    configure_release_plan_paths(&mut run);
    runs::insert(&pool, &run).await.unwrap();
    stages::insert(
        &pool,
        &make_stage(stage_exec_id, run_id, "commit_and_push_to_github"),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("commit_and_push_to_github".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "commit_and_push_to_github",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "commit_and_push_to_github",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 2,
            }),
        )
        .await
        .unwrap();

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("build_archive_and_push_connect".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "build_archive_and_push_connect",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "build_archive_and_push_connect",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 2,
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let after_git = artifacts::list_by_run(&pool, run_id).await.unwrap();
    let release_manifest = after_git
        .iter()
        .find(|a| a.name == "release_manifest")
        .unwrap();
    let git_push_receipt = after_git
        .iter()
        .find(|a| a.name == "git_push_receipt")
        .unwrap();
    assert!(release_manifest
        .file_path
        .ends_with(".chainworks/release/manifest.json"));
    assert!(git_push_receipt
        .file_path
        .ends_with(".chainworks/release/git-push.json"));
    assert!(!after_git.iter().any(|a| a.name == "delivery_receipt"));

    assert!(executor.process_next_item().await.unwrap());
    assert!(executor.process_next_item().await.unwrap());
    assert!(executor.process_next_item().await.unwrap());

    let final_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    let release_bundle_manifest = final_artifacts
        .iter()
        .find(|a| a.name == "release_bundle_manifest")
        .unwrap();
    let connect_upload_receipt = final_artifacts
        .iter()
        .find(|a| a.name == "connect_upload_receipt")
        .unwrap();
    assert!(release_bundle_manifest
        .file_path
        .ends_with(".chainworks/release/bundle.json"));
    assert!(connect_upload_receipt
        .file_path
        .ends_with(".chainworks/release/connect-upload.json"));
    assert!(final_artifacts.iter().any(|a| a.name == "delivery_receipt"));

    let delivery = final_artifacts
        .iter()
        .find(|a| a.name == "delivery_receipt")
        .expect("delivery_receipt artifact");
    assert_eq!(delivery.format, ArtifactFormat::Json);
    assert!(delivery
        .file_path
        .ends_with(".chainworks/release/delivery-receipt.json"));
}

#[tokio::test]
async fn background_executor_release_uses_provisioned_run_branch_over_stale_delivery_config() {
    enable_release_side_effects();
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (repo_dir, remote_dir) = init_release_repo_on_branch(tmp.path(), "release/actual");
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(
        run_id,
        idea_id,
        &repo_dir,
        artifact_root.to_string_lossy().as_ref(),
    );
    run.target_branch = Some("release/actual".into());
    run.delivery_configuration_json = Some(
        serde_json::to_string(&DeliveryConfiguration {
            repo_identifier: "repo/test".into(),
            repo_root: repo_dir.clone(),
            base_branch: "main".into(),
            worktree_base_path: repo_dir.clone(),
            target_branch: "release/stale-from-start".into(),
            release_target_id: Some("sandbox-target".into()),
            release_mode: Some("sandbox".into()),
        })
        .unwrap(),
    );
    configure_release_plan_paths(&mut run);
    runs::insert(&pool, &run).await.unwrap();
    stages::insert(
        &pool,
        &make_stage(stage_exec_id, run_id, "commit_and_push_to_github"),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("commit_and_push_to_github".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "commit_and_push_to_github",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "commit_and_push_to_github",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 1,
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    let receipt_artifact = artifacts
        .iter()
        .find(|artifact| artifact.name == "git_push_receipt")
        .expect("git push receipt should be persisted");
    let receipt_json = std::fs::read_to_string(&receipt_artifact.file_path).unwrap();
    let receipt: engine::release::git::GitPushReceipt =
        serde_json::from_str(&receipt_json).unwrap();
    assert_eq!(receipt.branch, "release/actual");

    let remote_sha = git_output(
        Path::new(&remote_dir),
        &["rev-parse", "refs/heads/release/actual"],
    );
    assert_eq!(remote_sha.trim(), receipt.commit_sha);
}

#[tokio::test]
async fn background_executor_persists_delivery_receipt_on_git_failure() {
    enable_release_side_effects();
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (repo_dir, _remote_dir) = init_release_repo_on_branch(tmp.path(), "main");
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(
        run_id,
        idea_id,
        &repo_dir,
        artifact_root.to_string_lossy().as_ref(),
    );
    let bad_config = DeliveryConfiguration {
        repo_identifier: "repo/test".into(),
        repo_root: repo_dir.clone(),
        base_branch: "main".into(),
        worktree_base_path: repo_dir.clone(),
        target_branch: "main".into(),
        release_target_id: Some("sandbox-target".into()),
        release_mode: Some("sandbox".into()),
    };
    run.delivery_configuration_json = Some(serde_json::to_string(&bad_config).unwrap());
    run.target_branch = Some("main".into());
    configure_release_plan_paths(&mut run);
    runs::insert(&pool, &run).await.unwrap();
    stages::insert(
        &pool,
        &make_stage(stage_exec_id, run_id, "commit_and_push_to_github"),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("commit_and_push_to_github".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "commit_and_push_to_github",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "commit_and_push_to_github",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 1,
            }),
        )
        .await
        .unwrap();

    let err = executor
        .process_next_item()
        .await
        .expect_err("git step should fail");
    assert!(err
        .to_string()
        .contains("push target 'main' is not allowed"));

    let persisted = artifacts::list_by_run(&pool, run_id).await.unwrap();
    assert!(!persisted
        .iter()
        .any(|artifact| artifact.name == "release_manifest"));
    assert!(!persisted
        .iter()
        .any(|artifact| artifact.name == "git_push_receipt"));
    let delivery = persisted
        .iter()
        .find(|artifact| artifact.name == "delivery_receipt")
        .expect("delivery receipt must be persisted on git failure");
    assert!(delivery
        .file_path
        .ends_with(".chainworks/release/delivery-receipt.json"));
    let receipt: engine::release::receipt::DeliveryReceipt =
        serde_json::from_str(&std::fs::read_to_string(&delivery.file_path).unwrap()).unwrap();
    let release_result = receipt.release_result.expect("release result");
    assert_eq!(release_result.succeeded, false);
    assert_eq!(release_result.failure_stage.as_deref(), Some("git_commit"));
    assert!(release_result.commit_sha.is_none());
}

#[tokio::test]
async fn background_executor_persists_delivery_receipt_on_publish_failure() {
    enable_release_side_effects();
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (repo_dir, _remote_dir) = init_release_repo(tmp.path());
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let git_stage_exec_id = StageExecutionId::new();
    let publish_stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(
        run_id,
        idea_id,
        &repo_dir,
        artifact_root.to_string_lossy().as_ref(),
    );
    let bad_config = DeliveryConfiguration {
        repo_identifier: "repo/test".into(),
        repo_root: repo_dir.clone(),
        base_branch: "main".into(),
        worktree_base_path: repo_dir.clone(),
        target_branch: "release/test".into(),
        release_target_id: Some("sandbox-target".into()),
        release_mode: Some("production".into()),
    };
    run.delivery_configuration_json = Some(serde_json::to_string(&bad_config).unwrap());
    configure_release_plan_paths(&mut run);
    runs::insert(&pool, &run).await.unwrap();
    stages::insert(
        &pool,
        &make_stage(git_stage_exec_id, run_id, "commit_and_push_to_github"),
    )
    .await
    .unwrap();
    stages::insert(
        &pool,
        &make_stage(
            publish_stage_exec_id,
            run_id,
            "build_archive_and_push_connect",
        ),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("commit_and_push_to_github".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "commit_and_push_to_github",
                "stage_execution_id": git_stage_exec_id.to_string(),
                "agent_id": "commit_and_push_to_github",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 1,
            }),
        )
        .await
        .unwrap();
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("build_archive_and_push_connect".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "build_archive_and_push_connect",
                "stage_execution_id": publish_stage_exec_id.to_string(),
                "agent_id": "build_archive_and_push_connect",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 1,
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());
    assert!(executor.process_next_item().await.unwrap());
    assert!(executor.process_next_item().await.unwrap());

    let err = executor
        .process_next_item()
        .await
        .expect_err("publish step should fail with invalid release mode");
    assert!(err.to_string().contains("unsupported release mode"));

    let persisted = artifacts::list_by_run(&pool, run_id).await.unwrap();
    assert!(persisted
        .iter()
        .any(|artifact| artifact.name == "release_manifest"));
    assert!(persisted
        .iter()
        .any(|artifact| artifact.name == "git_push_receipt"));
    assert!(!persisted
        .iter()
        .any(|artifact| artifact.name == "release_bundle_manifest"));
    assert!(!persisted
        .iter()
        .any(|artifact| artifact.name == "connect_upload_receipt"));
    let delivery = persisted
        .iter()
        .rev()
        .find(|artifact| artifact.name == "delivery_receipt")
        .expect("delivery receipt must be persisted on publish failure");
    let receipt: engine::release::receipt::DeliveryReceipt =
        serde_json::from_str(&std::fs::read_to_string(&delivery.file_path).unwrap()).unwrap();
    let release_result = receipt.release_result.expect("release result");
    assert_eq!(release_result.succeeded, false);
    assert_eq!(
        release_result.failure_stage.as_deref(),
        Some("build_archive")
    );
    assert_eq!(release_result.branch.as_deref(), Some("release/test"));
    assert!(release_result.commit_sha.is_some());
}

#[tokio::test]
async fn advance_run_backfills_delivery_receipt_when_terminal_release_lineage_exists() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let artifact_root = tmp.path().join("artifacts");
    let workspace_root = tmp.path().join("workspace");
    std::fs::create_dir_all(&artifact_root).unwrap();
    std::fs::create_dir_all(&workspace_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let mut run = make_run(
        run_id,
        idea_id,
        workspace_root.to_string_lossy().as_ref(),
        artifact_root.to_string_lossy().as_ref(),
    );
    run.status = RunStatus::Completed;
    run.current_state = Some("state_12_finalization".into());
    runs::insert(&pool, &run).await.unwrap();

    persist_json_artifact(
        &pool,
        run_id,
        "commit_and_push_to_github",
        "commit_and_push_to_github",
        &artifact_root,
        "release_manifest",
        &engine::release::git::ReleaseManifest {
            commit_sha: "abc123".into(),
            branch: "release/test".into(),
            remote: "origin".into(),
            commit_message: "release commit".into(),
            files_changed: 2,
            insertions: 10,
            deletions: 1,
            timestamp: Utc::now(),
        },
    )
    .await;
    persist_json_artifact(
        &pool,
        run_id,
        "commit_and_push_to_github",
        "commit_and_push_to_github",
        &artifact_root,
        "git_push_receipt",
        &engine::release::git::GitPushReceipt {
            commit_sha: "abc123".into(),
            branch: "release/test".into(),
            remote: "origin".into(),
            status: "success".into(),
            failure_reason: None,
            timestamp: Utc::now(),
        },
    )
    .await;
    persist_json_artifact(
        &pool,
        run_id,
        "build_archive_and_push_connect",
        "build_archive_and_push_connect",
        &artifact_root,
        "release_bundle_manifest",
        &engine::release::connect::ReleaseBundleManifest {
            bundle_identifier: "com.chainworks.forge.sandbox".into(),
            bundle_version: "1.0.0".into(),
            build_number: "abc123".into(),
            archive_path: Some(
                artifact_root
                    .join("app.xcarchive")
                    .to_string_lossy()
                    .into_owned(),
            ),
            checksum_sha256: "deadbeef".into(),
            size_bytes: 12345,
            timestamp: Utc::now(),
        },
    )
    .await;
    persist_json_artifact(
        &pool,
        run_id,
        "build_archive_and_push_connect",
        "build_archive_and_push_connect",
        &artifact_root,
        "connect_upload_receipt",
        &engine::release::connect::ConnectUploadReceipt {
            artifact_id: "artifact-123".into(),
            destination: "sandbox://sandbox-target".into(),
            release_target_id: "sandbox-target".into(),
            release_mode: "sandbox".into(),
            status: "success".into(),
            failure_reason: None,
            timestamp: Utc::now(),
        },
    )
    .await;

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::AdvanceRun,
            Some(run_id),
            None,
            serde_json::json!({ "run_id": run_id.to_string() }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let final_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    let delivery = final_artifacts
        .iter()
        .find(|artifact| artifact.name == "delivery_receipt")
        .expect("delivery receipt backfilled");
    let receipt: engine::release::receipt::DeliveryReceipt =
        serde_json::from_str(&std::fs::read_to_string(&delivery.file_path).unwrap()).unwrap();
    assert_eq!(receipt.release_result.as_ref().unwrap().succeeded, true);
    assert_eq!(
        receipt
            .release_result
            .as_ref()
            .unwrap()
            .commit_sha
            .as_deref(),
        Some("abc123")
    );
}

#[tokio::test]
async fn advance_run_does_not_backfill_delivery_receipt_without_release_lineage() {
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let artifact_root = tmp.path().join("artifacts");
    let workspace_root = tmp.path().join("workspace");
    std::fs::create_dir_all(&artifact_root).unwrap();
    std::fs::create_dir_all(&workspace_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();

    let mut run = make_run(
        run_id,
        idea_id,
        workspace_root.to_string_lossy().as_ref(),
        artifact_root.to_string_lossy().as_ref(),
    );
    run.status = RunStatus::Blocked;
    run.current_state = Some("state_12_finalization".into());
    runs::insert(&pool, &run).await.unwrap();

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::AdvanceRun,
            Some(run_id),
            None,
            serde_json::json!({ "run_id": run_id.to_string() }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let final_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
    assert!(!final_artifacts
        .iter()
        .any(|artifact| artifact.name == "delivery_receipt"));
}

#[tokio::test]
async fn background_executor_fails_closed_without_delivery_configuration_json_and_writes_no_receipt(
) {
    enable_release_side_effects();
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (repo_dir, _remote_dir) = init_release_repo(tmp.path());
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(
        run_id,
        idea_id,
        &repo_dir,
        artifact_root.to_string_lossy().as_ref(),
    );
    run.delivery_configuration_json = None;
    configure_release_plan_paths(&mut run);
    runs::insert(&pool, &run).await.unwrap();
    stages::insert(
        &pool,
        &make_stage(stage_exec_id, run_id, "commit_and_push_to_github"),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("commit_and_push_to_github".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "commit_and_push_to_github",
                "stage_execution_id": stage_exec_id.to_string(),
                "agent_id": "commit_and_push_to_github",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 1,
            }),
        )
        .await
        .unwrap();

    let err = executor
        .process_next_item()
        .await
        .expect_err("missing delivery config must fail closed");
    assert!(err
        .to_string()
        .contains("Release agent requires delivery_configuration_json"));

    let persisted = artifacts::list_by_run(&pool, run_id).await.unwrap();
    assert!(!persisted
        .iter()
        .any(|artifact| artifact.name == "delivery_receipt"));
    assert!(!persisted
        .iter()
        .any(|artifact| artifact.name == "release_manifest"));
    assert!(!persisted
        .iter()
        .any(|artifact| artifact.name == "git_push_receipt"));

    let executions = agent_executions::find_by_stage(&pool, stage_exec_id)
        .await
        .unwrap();
    let execution = executions
        .iter()
        .find(|execution| execution.agent_id == "commit_and_push_to_github")
        .expect("release invoke should create agent execution truth");
    assert_eq!(execution.status, AgentStatus::Failed);
    assert!(
        execution.completed_at.is_some(),
        "failed release agent execution must be settled"
    );
}

#[tokio::test]
async fn background_executor_preserves_existing_delivery_receipt_without_overwrite() {
    enable_release_side_effects();
    let pool = test_pool().await;
    let tmp = tempfile::tempdir().unwrap();
    let (repo_dir, _remote_dir) = init_release_repo(tmp.path());
    let artifact_root = tmp.path().join("artifacts");
    std::fs::create_dir_all(&artifact_root).unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let git_stage_exec_id = StageExecutionId::new();
    let publish_stage_exec_id = StageExecutionId::new();
    ideas::insert(&pool, &make_idea(idea_id)).await.unwrap();
    let mut run = make_run(
        run_id,
        idea_id,
        &repo_dir,
        artifact_root.to_string_lossy().as_ref(),
    );
    let bad_config = DeliveryConfiguration {
        repo_identifier: "repo/test".into(),
        repo_root: repo_dir.clone(),
        base_branch: "main".into(),
        worktree_base_path: repo_dir.clone(),
        target_branch: "release/test".into(),
        release_target_id: Some("sandbox-target".into()),
        release_mode: Some("production".into()),
    };
    run.delivery_configuration_json = Some(serde_json::to_string(&bad_config).unwrap());
    configure_release_plan_paths(&mut run);
    runs::insert(&pool, &run).await.unwrap();
    stages::insert(
        &pool,
        &make_stage(git_stage_exec_id, run_id, "commit_and_push_to_github"),
    )
    .await
    .unwrap();
    stages::insert(
        &pool,
        &make_stage(
            publish_stage_exec_id,
            run_id,
            "build_archive_and_push_connect",
        ),
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(16);
    let work_queue = WorkQueue::new(pool.clone());
    let orchestrator = Arc::new(Orchestrator::new(
        pool.clone(),
        events.clone(),
        work_queue.clone(),
    ));
    let executor = BackgroundExecutor::new(
        pool.clone(),
        work_queue.clone(),
        orchestrator,
        Arc::new(acp::AcpRuntimeManager::new_with_adapters(vec![])),
        events,
    );

    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("commit_and_push_to_github".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "commit_and_push_to_github",
                "stage_execution_id": git_stage_exec_id.to_string(),
                "agent_id": "commit_and_push_to_github",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 1,
            }),
        )
        .await
        .unwrap();
    work_queue
        .enqueue(
            db::work_item::WorkItemKind::InvokeAgent,
            Some(run_id),
            Some("build_archive_and_push_connect".into()),
            serde_json::json!({
                "run_id": run_id.to_string(),
                "stage_id": "build_archive_and_push_connect",
                "stage_execution_id": publish_stage_exec_id.to_string(),
                "agent_id": "build_archive_and_push_connect",
                "provider": "claude",
                "model": "test",
                "effort": "low",
                "session_reuse_scope": null,
                "total_tasks": 1,
            }),
        )
        .await
        .unwrap();

    assert!(executor.process_next_item().await.unwrap());

    let preexisting_receipt_path =
        Path::new(&repo_dir).join(".chainworks/release/delivery-receipt.json");
    std::fs::create_dir_all(preexisting_receipt_path.parent().unwrap()).unwrap();
    let sentinel_receipt = engine::release::receipt::DeliveryReceipt {
        run_id: run_id.to_string(),
        workflow_id: "wf-release".into(),
        idea_title: "Sentinel".into(),
        delivery_config: DeliveryConfiguration {
            repo_identifier: "repo/test".into(),
            repo_root: repo_dir.clone(),
            base_branch: "main".into(),
            worktree_base_path: repo_dir.clone(),
            target_branch: "release/test".into(),
            release_target_id: Some("sandbox-target".into()),
            release_mode: Some("sandbox".into()),
        },
        worktree_root: repo_dir.clone(),
        base_revision: Some("sentinel-base".into()),
        release_result: Some(engine::release::receipt::ReleaseResultSummary {
            commit_sha: Some("sentinel-sha".into()),
            branch: Some("release/test".into()),
            remote: Some("origin".into()),
            files_changed: Some(1),
            succeeded: false,
            failure_stage: Some("sentinel".into()),
            failure_reason: Some("keep me".into()),
        }),
        rollout_contract_readback: None,
        p080_reconciliation: None,
        implementation_review_status: Some("sentinel".into()),
        timestamp: Utc::now(),
    };
    let sentinel_json = serde_json::to_string_pretty(&sentinel_receipt).unwrap();
    std::fs::write(&preexisting_receipt_path, &sentinel_json).unwrap();
    artifacts::insert(
        &pool,
        &domain::artifact::Artifact {
            id: domain::ids::ArtifactId::new(),
            run_id,
            stage_id: "state_11_manual_release".into(),
            agent_id: "system_delivery".into(),
            name: "delivery_receipt".into(),
            contract_id: "delivery_receipt".into(),
            format: ArtifactFormat::Json,
            file_path: preexisting_receipt_path.to_string_lossy().into_owned(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "system".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
            agent_execution_id: None,
        },
    )
    .await
    .unwrap();

    assert!(executor.process_next_item().await.unwrap());
    assert!(executor.process_next_item().await.unwrap());

    let err = executor
        .process_next_item()
        .await
        .expect_err("publish step should fail with invalid release mode");
    assert!(err.to_string().contains("unsupported release mode"));

    let persisted = artifacts::list_by_run(&pool, run_id).await.unwrap();
    assert_eq!(
        persisted
            .iter()
            .filter(|artifact| artifact.name == "delivery_receipt")
            .count(),
        1
    );
    let stored = std::fs::read_to_string(&preexisting_receipt_path).unwrap();
    assert_eq!(stored, sentinel_json);
}
