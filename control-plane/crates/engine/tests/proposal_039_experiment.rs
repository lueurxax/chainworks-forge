//! I1 composition proof only. No successor Run, queue item, or successful review is fabricated.
use chrono::Utc;
use db::repos::{approvals, ideas, runs};
use domain::{
    approval::{Approval, ApprovalDecision},
    idea::{Idea, IdeaStatus},
    ids::{ApprovalId, IdeaId, RunId, StageExecutionId},
    run::{Run, RunStatus},
    run_carry_forward::{canonical_digest, ApprovalBindingV1, ApprovalEvidenceV1, ContentDigest},
};
use engine::run_carry_forward::{materialize, PreviewOptions, WorkspacePlanner};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{fs, path::PathBuf, process::Command, sync::Arc};
use uuid::Uuid;

#[tokio::test]
async fn isolated_source_history_is_unchanged_and_current_target_needs_a_distinct_approval() {
    let temp = tempfile::tempdir().unwrap();
    let source = temp.path().join("source");
    fs::create_dir(&source).unwrap();
    for args in [
        vec!["init", "-b", "main"],
        vec!["config", "user.email", "p039@example.invalid"],
        vec!["config", "user.name", "P039 Test"],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&source)
            .output()
            .unwrap()
            .status
            .success());
    }
    fs::write(source.join("proposal.md"), "Approved historical proposal\n").unwrap();
    fs::write(source.join("code.txt"), "original\n").unwrap();
    for args in [
        vec!["add", "."],
        vec![
            "-c",
            "core.hooksPath=/dev/null",
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "fixture",
        ],
    ] {
        assert!(Command::new("git")
            .args(args)
            .current_dir(&source)
            .output()
            .unwrap()
            .status
            .success());
    }
    fs::write(source.join("code.txt"), "preserved implementation\n").unwrap();
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let catalog = repo.join("examples/agents/agents.yaml");
    let baseline = workflow::compiler::compile_for_new_run_v1(
        repo.join("examples/workflows/workflow.yaml")
            .to_str()
            .unwrap(),
        catalog.to_str().unwrap(),
    )
    .unwrap()
    .into_plan();
    let mut old_catalog: Value = serde_json::from_str(&baseline.catalog_snapshot_json).unwrap();
    old_catalog["permission_profiles"]["CODE_WRITE"]
        .as_object_mut()
        .unwrap()
        .remove("xcode_headless");
    let old_catalog_json = serde_json::to_string(&old_catalog).unwrap();
    let old_catalog_hash = format!("{:x}", Sha256::digest(old_catalog_json.as_bytes()));
    let mut wf: Value = serde_json::from_str(&baseline.workflow_snapshot_json).unwrap();
    wf["blocked_run_continuation"] = json!({
        "schema_version":"blocked_run_continuation_profile_v1", "profile":"implementation_restart_v1",
        "review_state":"state_4_proposal_reviewed", "approval_state":"state_6_implementation_approval",
        "preparation_state":"state_7_implementation_started", "proposal_input":"proposal_current",
        "preparation_task":"freeze_approved_proposal_and_prepare_worktree"
    });
    let target_workflow = temp.path().join("target.yaml");
    fs::write(&target_workflow, serde_yaml::to_string(&wf).unwrap()).unwrap();
    let target = workflow::carry_forward::compile_for_continuation_v1(
        target_workflow.to_str().unwrap(),
        catalog.to_str().unwrap(),
    )
    .unwrap();
    assert_ne!(target.plan.catalog_snapshot_hash, old_catalog_hash);
    assert_eq!(target.plan.initial_state, "state_4_proposal_reviewed");

    let pool = db::pool::create_pool(&format!(
        "sqlite://{}",
        temp.path().join("fixture.db").display()
    ))
    .await
    .unwrap();
    db::writer::register_shared_writer(&pool, Arc::new(db::writer::DbWriter::new(pool.clone())))
        .await
        .unwrap();
    let idea_id = IdeaId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P039 disposable source".into(),
            body: "Fixture".into(),
            workspace_root_path: Some(source.display().to_string()),
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    let source_id = RunId::new();
    let old_run: Run = serde_json::from_value(json!({
        "id":source_id, "idea_id":idea_id, "status":"blocked", "workflow_id":"proposal_to_release",
        "workflow_title":"fixture", "workspace_root":source, "artifact_root":temp.path().join("old-artifacts"),
        "started_at":Utc::now(), "current_state":"state_7_implementation_started", "worktree_root":source,
        "workflow_snapshot_json":baseline.workflow_snapshot_json, "workflow_snapshot_hash":baseline.workflow_snapshot_hash,
        "catalog_snapshot_json":old_catalog_json, "catalog_snapshot_hash":old_catalog_hash
    })).unwrap();
    runs::insert(&pool, &old_run).await.unwrap();
    let old_approval = Approval {
        id: ApprovalId::new(),
        run_id: source_id,
        stage_id: "state_6_implementation_approval".into(),
        decision: ApprovalDecision::Granted,
        requested_at: Utc::now(),
        decided_at: Some(Utc::now()),
        comment: None,
        expires_at: None,
    };
    approvals::insert(&pool, &old_approval).await.unwrap();
    let before =
        serde_json::to_value(runs::find_by_id(&pool, source_id).await.unwrap().unwrap()).unwrap();
    let preview = WorkspacePlanner::preview(&source, PreviewOptions::default())
        .await
        .unwrap();
    let receipt = materialize(&preview, temp.path(), Uuid::new_v4())
        .await
        .unwrap();
    let successor_id = RunId::new();
    let evidence = ApprovalEvidenceV1 {
        source_run_id: source_id,
        successor_run_id: successor_id,
        manifest_sha256: receipt.manifest_sha256,
        proposal_sha256: ContentDigest::of(&fs::read(source.join("proposal.md")).unwrap()),
        code_sha256: preview.digest().clone(),
        workflow_snapshot_hash: ContentDigest::from_sha256_hex(&target.plan.workflow_snapshot_hash)
            .unwrap(),
        catalog_snapshot_hash: ContentDigest::from_sha256_hex(&target.plan.catalog_snapshot_hash)
            .unwrap(),
        capability_delta_sha256: canonical_digest(
            &json!({"old":old_catalog_hash,"new":target.plan.catalog_snapshot_hash}),
        )
        .unwrap(),
        unresolved_findings_sha256: canonical_digest(&json!([])).unwrap(),
    };
    let new_approval_id = ApprovalId::new();
    let stage = StageExecutionId::new();
    let binding = ApprovalBindingV1::pending(new_approval_id, stage, evidence.clone()).unwrap();
    assert_ne!(new_approval_id, old_approval.id);
    assert!(!binding.authorizes(&evidence, stage));
    assert_eq!(
        approvals::find_by_id(&pool, old_approval.id)
            .await
            .unwrap()
            .unwrap()
            .decision,
        ApprovalDecision::Granted
    );
    let after = runs::find_by_id(&pool, source_id).await.unwrap().unwrap();
    assert_eq!(after.status, RunStatus::Blocked);
    assert_eq!(before, serde_json::to_value(after).unwrap());
    assert!(runs::find_by_id(&pool, successor_id)
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("SELECT COUNT(*) FROM work_items")
            .fetch_one(&pool)
            .await
            .unwrap(),
        0
    );
    pool.close().await;
}
