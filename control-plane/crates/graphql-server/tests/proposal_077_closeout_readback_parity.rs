use std::sync::Arc;

use async_graphql::Request;
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
use engine::command_handler::CommandHandler;
use engine::event_bus;
use engine::lifecycle_reporter::LifecycleReporter;
use engine::work_queue::WorkQueue;
use graphql_server::schema::build_schema;

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
    let pool = create_pool("sqlite::memory:").await.unwrap();
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

#[tokio::test]
async fn graphql_run_detail_exposes_p077_documented_and_legacy_closeout_summary_names() {
    let (pool, run_id, fingerprint_hash) = seed_run_with_closeout_summary().await;
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
                r#"{{
                    run(id: "{run_id}") {{
                        closeoutReadinessSummaryJson
                        implementationCloseoutReadinessSummary
                    }}
                }}"#
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
    let legacy = &run["closeoutReadinessSummaryJson"];
    let documented = &run["implementationCloseoutReadinessSummary"];

    assert_eq!(documented, legacy);
    assert_eq!(documented["readiness_generation_id"], "readiness-gen-p077");
    assert_eq!(documented["readiness_status"], "ready");
    assert_eq!(documented["readiness_decision"], "enter_manual_release");
    assert_eq!(documented["gate_generation_id"], "gate-gen-p077");
    assert_eq!(documented["gate_status"], "passed");
    assert_eq!(documented["fingerprint_hash"], fingerprint_hash);
}
