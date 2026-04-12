use db::pool::create_pool;
use db::repos::{ideas, runs, stages, projections};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId, StageExecutionId};
use domain::run::{Run, RunStatus};
use domain::stage::{StageExecution, StageStatus};
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
