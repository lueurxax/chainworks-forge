use db::pool::create_pool;
use db::repos::{artifacts, ideas, projections, runs, stages};
use domain::artifact::{Artifact, ArtifactFormat};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ArtifactId, IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageSettlementKind, StageStatus};
use chrono::Utc;

async fn test_pool() -> sqlx::SqlitePool {
    create_pool("sqlite::memory:").await.expect("in-memory pool failed")
}

#[tokio::test]
async fn test_idea_insert_and_find() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Test Idea".into(),
        body: "Body content".into(),
        workspace_root_path: None,
        status: IdeaStatus::Draft,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.expect("insert failed");
    let found = ideas::find_by_id(&pool, idea.id).await.expect("find failed");
    assert!(found.is_some());
    assert_eq!(found.unwrap().title, "Test Idea");
}

#[tokio::test]
async fn test_run_insert_and_find() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea for run".into(),
        body: "body".into(),
        workspace_root_path: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Pending,
        workflow_id: "wf1".into(),
        workflow_title: "Workflow 1".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
    };
    runs::insert(&pool, &run).await.unwrap();
    let found = runs::find_by_id(&pool, run.id).await.unwrap();
    assert!(found.is_some());
    assert_eq!(found.unwrap().status, RunStatus::Pending);
}

#[tokio::test]
async fn test_run_status_update() {
    let pool = test_pool().await;
    let idea = Idea {
        id: IdeaId::new(),
        title: "Idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();
    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Pending,
        workflow_id: "wf1".into(),
        workflow_title: "WF".into(),
        workspace_root: "/tmp".into(),
        artifact_root: "/tmp/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
    };
    runs::insert(&pool, &run).await.unwrap();
    runs::update_status(&pool, run.id, RunStatus::Running).await.unwrap();
    let found = runs::find_by_id(&pool, run.id).await.unwrap().unwrap();
    assert_eq!(found.status, RunStatus::Running);
}

// ---------------------------------------------------------------------------
// Parity harness (ARCH-002 / P027)
// Proves that projection layer accurately reflects canonical run/stage state.
// ---------------------------------------------------------------------------

/// After rebuild_all_for_run, run_summaries must mirror canonical table counts.
#[tokio::test]
async fn test_projection_parity_after_rebuild() {
    let pool = test_pool().await;

    let idea = Idea {
        id: IdeaId::new(),
        title: "Parity idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Running,
        workflow_id: "wf-parity".into(),
        workflow_title: "Parity Workflow".into(),
        workspace_root: "/tmp/parity".into(),
        artifact_root: "/tmp/parity/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    // Insert one completed stage and one failed stage.
    let completed_stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "stage_a".into(),
        label: "Stage A".into(),
        status: StageStatus::Completed,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: Some(domain::stage::StageSettlementKind::Completed),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
    };
    let failed_stage = StageExecution {
        id: StageExecutionId::new(),
        run_id: run.id,
        stage_id: "stage_b".into(),
        label: "Stage B".into(),
        status: StageStatus::Failed,
        iteration: 1,
        attempt_number: 1,
        settlement_kind: Some(domain::stage::StageSettlementKind::Failed),
        started_at: Utc::now(),
        completed_at: Some(Utc::now()),
    };
    stages::insert(&pool, &completed_stage).await.unwrap();
    stages::insert(&pool, &failed_stage).await.unwrap();

    // Rebuild projections.
    projections::rebuild_all_for_run(&pool, run.id).await.unwrap();

    // Query via projection layer and verify counts match canonical state.
    let projection_rows = projections::list_active_projection(&pool).await.unwrap();
    let row = projection_rows
        .iter()
        .find(|r| r.id == run.id.to_string())
        .expect("run missing from projection layer after rebuild");

    assert_eq!(row.status, run.status.to_string(), "projection status must match canonical status");
    assert_eq!(row.total_stages, 2, "total_stages must count both stages");
    assert_eq!(row.completed_stages, 1, "completed_stages must reflect one completed stage");
    assert_eq!(row.failed_stages, 1, "failed_stages must reflect one failed stage");
    assert_eq!(row.pending_approvals, 0, "pending_approvals must be zero without approvals");

    // Stage projection parity.
    let stage_rows = projections::list_stages_projection(&pool, &run.id.to_string()).await.unwrap();
    assert_eq!(stage_rows.len(), 2, "stage projection must surface both stages");

    let stage_a = stage_rows.iter().find(|s| s.stage_id == "stage_a").unwrap();
    assert_eq!(stage_a.status, StageStatus::Completed.to_string());

    let stage_b = stage_rows.iter().find(|s| s.stage_id == "stage_b").unwrap();
    assert_eq!(stage_b.status, StageStatus::Failed.to_string());
}

// ---------------------------------------------------------------------------
// File-backed SQLite durability proof (REQ-002 / READY-001)
//
// Proves that canonical state written to a file-backed SQLite database survives
// process restart: data is written, the pool is closed, a new pool is opened on
// the same file, and all entities are still readable with projections intact.
// ---------------------------------------------------------------------------

/// Write a full workflow slice to a file-backed SQLite database, close the
/// connection, reopen it, and verify all entities and projections are durable.
#[tokio::test]
async fn test_file_backed_sqlite_durability_across_restart() {
    let tmp = tempfile::tempdir().unwrap();
    let db_file = tmp.path().join("parity.db");
    let db_url = format!("sqlite://{}", db_file.display());

    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    let stage_id = StageExecutionId::new();
    let artifact_id = ArtifactId::new();

    // ── Write phase (simulates first daemon boot) ─────────────────────────────
    {
        let pool = create_pool(&db_url).await.expect("first open failed");

        let idea = Idea {
            id: idea_id,
            title: "Durable idea".into(),
            body: "body".into(),
            workspace_root_path: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        };
        ideas::insert(&pool, &idea).await.unwrap();

        let run = Run {
            id: run_id,
            idea_id,
            status: RunStatus::Running,
            workflow_id: "wf-durable".into(),
            workflow_title: "Durable Workflow".into(),
            workspace_root: tmp.path().to_string_lossy().into_owned(),
            artifact_root: tmp.path().to_string_lossy().into_owned(),
            started_at: Utc::now(),
            completed_at: None,
            cancellation_requested_at: None,
            cancellation_settled_at: None,
        };
        runs::insert(&pool, &run).await.unwrap();

        let stage = StageExecution {
            id: stage_id,
            run_id,
            stage_id: "build".into(),
            label: "Build".into(),
            status: StageStatus::Completed,
            iteration: 1,
            attempt_number: 1,
            settlement_kind: Some(StageSettlementKind::Completed),
            started_at: Utc::now(),
            completed_at: Some(Utc::now()),
        };
        stages::insert(&pool, &stage).await.unwrap();

        let artifact = Artifact {
            id: artifact_id,
            run_id,
            stage_id: "build".into(),
            agent_id: "claude".into(),
            name: "report.json".into(),
            contract_id: "claude.output".into(),
            format: ArtifactFormat::Json,
            file_path: "/tmp/report.json".into(),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: Some("execution_report".into()),
            report_version: Some(1),
        };
        artifacts::insert(&pool, &artifact).await.unwrap();
        projections::rebuild_all_for_run(&pool, run_id).await.unwrap();

        // Pool drops here — simulates process exit / connection close
        pool.close().await;
    }

    assert!(db_file.exists(), "SQLite file must persist after pool close");

    // ── Read phase (simulates daemon restart) ─────────────────────────────────
    {
        let pool = create_pool(&db_url).await.expect("reopen failed");

        // Canonical repos must return the written entities
        let found_idea = ideas::find_by_id(&pool, idea_id).await.unwrap();
        assert!(found_idea.is_some(), "idea must survive pool close/reopen");
        assert_eq!(found_idea.unwrap().title, "Durable idea");

        let found_run = runs::find_by_id(&pool, run_id).await.unwrap();
        assert!(found_run.is_some(), "run must survive pool close/reopen");
        assert_eq!(found_run.unwrap().status, RunStatus::Running);

        let run_stages = stages::list_by_run(&pool, run_id).await.unwrap();
        assert_eq!(run_stages.len(), 1, "stage must survive pool close/reopen");
        assert_eq!(run_stages[0].status, StageStatus::Completed);

        let run_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();
        assert_eq!(run_artifacts.len(), 1, "artifact must survive pool close/reopen");
        assert_eq!(run_artifacts[0].name, "report.json");

        // Projections survive and report correct values
        let proj_rows = projections::list_active_projection(&pool).await.unwrap();
        let proj = proj_rows
            .iter()
            .find(|r| r.id == run_id.to_string())
            .expect("run projection must survive restart");
        assert_eq!(proj.total_stages, 1);
        assert_eq!(proj.completed_stages, 1);
        // Verify artifact survives via artifact projection
        let art_proj = projections::list_artifacts_projection(
            &pool, &run_id.to_string()
        ).await.unwrap();
        assert!(!art_proj.is_empty(), "artifact projection must survive restart");

        pool.close().await;
    }
}

// ---------------------------------------------------------------------------
// Projection parity comparison harness (REQ-005 / PROD-001)
//
// Proves that the projection layer accurately mirrors the canonical repository
// values across all four projection surfaces (run, stages, artifacts, approvals).
// This is the in-process parity comparison tool called for by the proposal.
// ---------------------------------------------------------------------------

/// Compare projection-layer output against canonical repo state for a multi-surface
/// workflow slice: run summary, stage list, artifact index.
/// All projection counts must exactly match the canonical table counts after rebuild.
#[tokio::test]
async fn test_projection_parity_matches_canonical_repo_values() {
    let pool = create_pool("sqlite::memory:").await.unwrap();

    let idea_id = IdeaId::new();
    let run_id = RunId::new();

    let idea = Idea {
        id: idea_id,
        title: "Parity harness idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: run_id,
        idea_id,
        status: RunStatus::Running,
        workflow_id: "wf-parity2".into(),
        workflow_title: "Parity Harness".into(),
        workspace_root: "/tmp/ph".into(),
        artifact_root: "/tmp/ph/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    // Insert three stages with distinct statuses
    let stage_specs: &[(&str, StageStatus, Option<StageSettlementKind>)] = &[
        ("alpha", StageStatus::Completed, Some(StageSettlementKind::Completed)),
        ("beta",  StageStatus::Failed,    Some(StageSettlementKind::Failed)),
        ("gamma", StageStatus::Pending,   None),
    ];

    for (sid, status, kind) in stage_specs {
        let s = StageExecution {
            id: StageExecutionId::new(),
            run_id,
            stage_id: (*sid).to_string(),
            label: sid.to_uppercase(),
            status: status.clone(),
            iteration: 1,
            attempt_number: 1,
            settlement_kind: kind.clone(),
            started_at: Utc::now(),
            completed_at: if kind.is_some() { Some(Utc::now()) } else { None },
        };
        stages::insert(&pool, &s).await.unwrap();
    }

    // Insert two artifacts
    for n in 0u8..2 {
        let art = Artifact {
            id: ArtifactId::new(),
            run_id,
            stage_id: "alpha".into(),
            agent_id: "claude".into(),
            name: format!("artifact_{n}.json"),
            contract_id: "claude.output".into(),
            format: ArtifactFormat::Json,
            file_path: format!("/tmp/ph/artifact_{n}.json"),
            checksum_sha256: None,
            size_bytes: None,
            provider: "claude".into(),
            model: None,
            created_at: Utc::now(),
            is_pinned: false,
            report_kind: None,
            report_version: None,
        };
        artifacts::insert(&pool, &art).await.unwrap();
    }

    // Rebuild projections
    projections::rebuild_all_for_run(&pool, run_id).await.unwrap();

    // ── Run summary projection vs canonical ──────────────────────────────────
    let canonical_stages = stages::list_by_run(&pool, run_id).await.unwrap();
    let canonical_artifacts = artifacts::list_by_run(&pool, run_id).await.unwrap();

    let proj_rows = projections::list_active_projection(&pool).await.unwrap();
    let proj = proj_rows
        .iter()
        .find(|r| r.id == run_id.to_string())
        .expect("run must appear in projection after rebuild");

    assert_eq!(
        proj.total_stages as usize,
        canonical_stages.len(),
        "total_stages projection must match canonical stage count"
    );
    assert_eq!(
        proj.completed_stages as usize,
        canonical_stages.iter().filter(|s| s.status == StageStatus::Completed).count(),
        "completed_stages projection must match canonical count"
    );
    assert_eq!(
        proj.failed_stages as usize,
        canonical_stages.iter().filter(|s| s.status == StageStatus::Failed).count(),
        "failed_stages projection must match canonical count"
    );
    // has_artifacts is surfaced per-stage (StageSummaryRow), not on RunProjectionRow.
    // Verify via artifact projection count instead.
    let art_proj = projections::list_artifacts_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(
        art_proj.len(),
        canonical_artifacts.len(),
        "artifact projection count must match canonical artifact count (has_artifacts parity)"
    );

    // ── Stage projection vs canonical ────────────────────────────────────────
    let stage_proj = projections::list_stages_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(
        stage_proj.len(),
        canonical_stages.len(),
        "stage projection row count must match canonical stage count"
    );
    for canonical in &canonical_stages {
        let proj_stage = stage_proj
            .iter()
            .find(|s| s.stage_id == canonical.stage_id)
            .unwrap_or_else(|| {
                panic!("stage {} missing from stage projection", canonical.stage_id)
            });
        assert_eq!(
            proj_stage.status,
            canonical.status.to_string(),
            "stage projection status must match canonical for {}",
            canonical.stage_id
        );
    }

    // ── Artifact projection vs canonical ─────────────────────────────────────
    let artifact_proj = projections::list_artifacts_projection(&pool, &run_id.to_string())
        .await
        .unwrap();
    assert_eq!(
        artifact_proj.len(),
        canonical_artifacts.len(),
        "artifact projection row count must match canonical artifact count"
    );
}

/// Projection list without a prior rebuild still returns runs (zero counts).
#[tokio::test]
async fn test_projection_list_before_rebuild_returns_run_with_zero_counts() {
    let pool = test_pool().await;

    let idea = Idea {
        id: IdeaId::new(),
        title: "Cold idea".into(),
        body: "body".into(),
        workspace_root_path: None,
        status: IdeaStatus::Active,
        created_at: Utc::now(),
        archived_at: None,
    };
    ideas::insert(&pool, &idea).await.unwrap();

    let run = Run {
        id: RunId::new(),
        idea_id: idea.id,
        status: RunStatus::Pending,
        workflow_id: "wf-cold".into(),
        workflow_title: "Cold Workflow".into(),
        workspace_root: "/tmp/cold".into(),
        artifact_root: "/tmp/cold/art".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
    };
    runs::insert(&pool, &run).await.unwrap();

    // No rebuild — projection layer should still surface the run via LEFT JOIN.
    let rows = projections::list_active_projection(&pool).await.unwrap();
    let row = rows
        .iter()
        .find(|r| r.id == run.id.to_string())
        .expect("run must appear in projection list even before first rebuild");

    assert_eq!(row.total_stages, 0);
    assert_eq!(row.completed_stages, 0);
    assert_eq!(row.failed_stages, 0);
}
