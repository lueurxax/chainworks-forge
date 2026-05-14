use std::sync::Arc;

use async_graphql::Request;
use chrono::Utc;
use db::pool::create_pool;
use db::repos::{artifact_contracts, ideas, runs};
use domain::artifact_contracts::{ActiveArtifactGenerationInput, ArtifactContractOverrideInput};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::run::{Run, RunStatus};
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

async fn test_pool() -> sqlx::SqlitePool {
    let pool = create_pool("sqlite::memory:").await.unwrap();
    db::writer::register_shared_writer(
        &pool,
        std::sync::Arc::new(db::writer::DbWriter::new(pool.clone())),
    )
    .await
    .unwrap();
    pool
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Ready,
        workflow_id: "wf".into(),
        workflow_title: "Workflow".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_10".into()),
        workflow_yaml_path: None,
        agent_catalog_yaml_path: None,
        worktree_root: None,
        base_branch: None,
        base_revision: None,
        target_branch: None,
        delivery_configuration_json: None,
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

#[tokio::test]
async fn proposal_057_graphql_run_detail_exposes_canonical_artifact_contract_truth() {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "Idea".into(),
            body: "Body".into(),
            workspace_root_path: None,
            project_key: None,
            status: IdeaStatus::Active,
            created_at: Utc::now(),
            archived_at: None,
        },
    )
    .await
    .unwrap();
    runs::insert(&pool, &make_run(run_id, idea_id))
        .await
        .unwrap();
    artifact_contracts::upsert_generation_and_rebuild(
        &pool,
        ActiveArtifactGenerationInput {
            run_id,
            artifact_id: ArtifactId::new(),
            contract_id: "prepush_review_v1".into(),
            canonical_path: "review/prepush.json".into(),
            raw_path: "review/prepush.json".into(),
            raw_status: "PASS_WITH_NOTES".into(),
            generation_id: "gen-1".into(),
            source_agent_execution_id: None,
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement: domain::agent::AgentOutputSettlement::None,
            partial: false,
            warnings: vec![],
        },
    )
    .await
    .unwrap();
    artifact_contracts::create_override_and_rebuild(
        &pool,
        ArtifactContractOverrideInput {
            run_id,
            contract_id: "audit_report_v1".into(),
            override_type: "implementation_status".into(),
            from_status: "needs_code_fixes".into(),
            to_status: "implemented".into(),
            reason: "operator verified".into(),
            owner: "operator".into(),
            source_artifacts: vec![],
            expires_at_stage: "state_11_manual_release".into(),
            journal_id: "journal-1".into(),
        },
    )
    .await
    .unwrap();

    let events = event_bus::new_bus(16);
    let handler = Arc::new(CommandHandler::new(
        pool.clone(),
        events.clone(),
        WorkQueue::new(pool.clone()),
    ));
    let schema = build_schema(
        pool,
        handler,
        events.clone(),
        auth::PrincipalTable::test_fixture(),
        LifecycleReporter::new(15, "test-build", events.clone()),
    );
    let response = schema
        .execute(
            Request::new(format!(
                r#"{{ run(id: "{}") {{ activeArtifactIndexJson runStateProjectionJson operatorOverridesJson }} }}"#,
                run_id
            ))
            .data(auth::Principal::new(
                "operator",
                auth::PrincipalClass::Operator,
            )),
        )
        .await;
    assert!(
        response.errors.is_empty(),
        "graphql errors: {:?}",
        response.errors
    );
    let data = response.data.into_json().unwrap();
    let run = data.get("run").unwrap();
    let active: serde_json::Value = serde_json::from_str(
        run.get("activeArtifactIndexJson")
            .and_then(serde_json::Value::as_str)
            .unwrap(),
    )
    .unwrap();
    let overrides: serde_json::Value = serde_json::from_str(
        run.get("operatorOverridesJson")
            .and_then(serde_json::Value::as_str)
            .unwrap(),
    )
    .unwrap();
    assert_eq!(active["contracts"]["prepush_review_v1"]["status"], "pass");
    assert_eq!(overrides[0]["to_status"], "implemented");
}
