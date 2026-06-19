use chrono::Utc;
use db::pool::create_pool;
use db::repos::{closeout, ideas, runs};
use domain::closeout_readiness::{
    CloseoutFingerprint, CloseoutReadiness, CloseoutReadinessDecision, CloseoutReadinessStatus,
};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{IdeaId, RunId};
use domain::proposal_gate_result::{ProposalGateResult, ProposalGateStatus};
use domain::run::{Run, RunStatus};

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

fn principal(class: auth::PrincipalClass) -> auth::Principal {
    auth::Principal::new("p077-test", class)
}

fn make_run(run_id: RunId, idea_id: IdeaId) -> Run {
    Run {
        id: run_id,
        idea_id,
        status: RunStatus::Ready,
        workflow_id: "proposal-077".into(),
        workflow_title: "P077".into(),
        workspace_root: "/tmp/ws".into(),
        artifact_root: "/tmp/artifacts".into(),
        started_at: Utc::now(),
        completed_at: None,
        cancellation_requested_at: None,
        cancellation_settled_at: None,
        cancellation_settlement_log: None,
        current_state: Some("state_9".into()),
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
        closeout_readiness_mode: Some("enforcement".into()),
    }
}

async fn seed_run_with_closeout_summary() -> (sqlx::SqlitePool, RunId, String) {
    let pool = test_pool().await;
    let idea_id = IdeaId::new();
    let run_id = RunId::new();
    ideas::insert(
        &pool,
        &Idea {
            id: idea_id,
            title: "P077 idea".into(),
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

    let fingerprint = CloseoutFingerprint {
        proposal_or_freeze_digest: "sha256:proposal".into(),
        run_id: run_id.to_string(),
        stage_id: "state_9".into(),
        workflow_digest: "sha256:workflow".into(),
        worktree_head: "abc123".into(),
        dirty_or_changed_file_digest: "sha256:dirty".into(),
        upstream_active_generation_ids: vec!["upstream-gen".into()],
        contract_version: "v1".into(),
        computed_at: Utc::now(),
        latency_ms: 7,
    };
    let fingerprint_hash = fingerprint.short_hash();
    let gate = ProposalGateResult {
        gate_id: "p077:077".into(),
        proposal_id: "077".into(),
        run_id: run_id.to_string(),
        stage_id: "state_9".into(),
        status: ProposalGateStatus::Passed,
        generation_id: "gate-gen-p077".into(),
        diagnostic_reason: None,
        executor_version: Some("test".into()),
        evidence_digest: None,
        exit_code: Some(0),
        elapsed_ms: Some(12),
        settled_at: Utc::now(),
        authorization_lineage: None,
        failure_classification: None,
    };
    let readiness = CloseoutReadiness {
        run_id: run_id.to_string(),
        stage_id: "state_9".into(),
        status: CloseoutReadinessStatus::Ready,
        decision: CloseoutReadinessDecision::EnterManualRelease,
        generation_id: "readiness-gen-p077".into(),
        readiness_mode: "enforcement".into(),
        diagnostic_reason: None,
        primary_unblock: Some("proposal gate passed".into()),
        code_blocker_count: 0,
        handoff_owner: None,
        risk_settlement_required: false,
        fingerprint: Some(fingerprint),
        synthesized_at: Utc::now(),
    };
    closeout::execute_closeout_transaction(
        &pool,
        closeout::CloseoutTransactionInputs {
            gate_result: &gate,
            readiness: &readiness,
            accepted_risks: &[],
            blocker_digest: None,
        },
    )
    .await
    .unwrap();

    (pool, run_id, fingerprint_hash)
}

fn assert_summary_fields(summary: &serde_json::Value, fingerprint_hash: &str) {
    assert_eq!(summary["readiness_generation_id"], "readiness-gen-p077");
    assert_eq!(summary["readiness_status"], "ready");
    assert_eq!(summary["readiness_decision"], "enter_manual_release");
    assert_eq!(summary["gate_generation_id"], "gate-gen-p077");
    assert_eq!(summary["gate_status"], "passed");
    assert_eq!(summary["fingerprint_hash"], fingerprint_hash);
}

#[tokio::test]
async fn runs_get_and_list_expose_p077_documented_and_legacy_closeout_summary_names() {
    let (pool, run_id, fingerprint_hash) = seed_run_with_closeout_summary().await;
    let handler = engine::command_handler::CommandHandler::new(
        pool.clone(),
        engine::event_bus::new_bus(16),
        engine::work_queue::WorkQueue::new(pool.clone()),
    );

    let get_payload = mcp_server::tools::runs::execute(
        "runs.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &handler,
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();
    assert_eq!(
        get_payload["implementation_closeout_readiness_summary"],
        get_payload["closeout_readiness_summary"]
    );
    assert_summary_fields(
        &get_payload["implementation_closeout_readiness_summary"],
        &fingerprint_hash,
    );

    // runs.list is projection-based (P087): rebuild projections so closeout summary is baked in.
    db::repos::projections::rebuild_all_for_run(&pool, run_id)
        .await
        .unwrap();

    let list_payload = mcp_server::tools::runs::execute(
        "runs.list",
        serde_json::json!({}),
        &pool,
        &handler,
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();
    let listed = list_payload
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == run_id.to_string())
        .expect("seeded run must appear in runs.list");
    assert_eq!(
        listed["implementation_closeout_readiness_summary"],
        listed["closeout_readiness_summary"]
    );
    assert_summary_fields(
        &listed["implementation_closeout_readiness_summary"],
        &fingerprint_hash,
    );
}

#[tokio::test]
async fn runs_list_uses_projected_p077_closeout_summary_without_detail_lookup() {
    let (pool, run_id, fingerprint_hash) = seed_run_with_closeout_summary().await;
    db::repos::projections::rebuild_all_for_run(&pool, run_id)
        .await
        .unwrap();

    sqlx::query("DELETE FROM closeout_gate_generations WHERE run_id = ?1")
        .bind(run_id.to_string())
        .execute(&pool)
        .await
        .unwrap();

    let handler = engine::command_handler::CommandHandler::new(
        pool.clone(),
        engine::event_bus::new_bus(16),
        engine::work_queue::WorkQueue::new(pool.clone()),
    );
    let list_payload = mcp_server::tools::runs::execute(
        "runs.list",
        serde_json::json!({}),
        &pool,
        &handler,
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();
    let listed = list_payload
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["id"] == run_id.to_string())
        .expect("seeded run must appear in runs.list");

    assert_eq!(
        listed["implementation_closeout_readiness_summary"],
        listed["closeout_readiness_summary"]
    );
    assert_summary_fields(
        &listed["implementation_closeout_readiness_summary"],
        &fingerprint_hash,
    );
}

#[tokio::test]
async fn reports_get_exposes_p077_documented_and_legacy_closeout_summary_names() {
    let (pool, run_id, fingerprint_hash) = seed_run_with_closeout_summary().await;
    let handler = engine::command_handler::CommandHandler::new(
        pool.clone(),
        engine::event_bus::new_bus(16),
        engine::work_queue::WorkQueue::new(pool.clone()),
    );

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &handler,
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();
    let mcp_truth = payload["reports"]
        .as_array()
        .unwrap()
        .iter()
        .find(|report| report["report_kind"] == "mcp_execution_truth")
        .expect("mcp_execution_truth report must be present");

    assert_eq!(
        mcp_truth["implementation_closeout_readiness_summary"],
        mcp_truth["closeout_readiness_summary"]
    );
    assert_summary_fields(
        &mcp_truth["implementation_closeout_readiness_summary"],
        &fingerprint_hash,
    );
}
