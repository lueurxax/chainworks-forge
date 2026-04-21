use chrono::Utc;
use db::pool::create_pool;
use db::repos::{artifact_contracts, ideas, runs};
use domain::artifact_contracts::{ActiveArtifactGenerationInput, ArtifactContractOverrideInput};
use domain::idea::{Idea, IdeaStatus};
use domain::ids::{ArtifactId, IdeaId, RunId};
use domain::run::{Run, RunStatus};

fn principal(class: auth::PrincipalClass) -> auth::Principal {
    auth::Principal::new("p", class)
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
    }
}

#[tokio::test]
async fn proposal_057_reports_get_exposes_canonical_statuses_and_overrides() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
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

    let payload = mcp_server::tools::reports::execute(
        "reports.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &engine::command_handler::CommandHandler::new(
            pool.clone(),
            engine::event_bus::new_bus(16),
            engine::work_queue::WorkQueue::new(pool.clone()),
        ),
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    let canonical = payload
        .as_array()
        .unwrap()
        .iter()
        .find(|item| item["report_kind"] == "canonical_artifact_contracts")
        .unwrap();
    assert_eq!(
        canonical["active_index"]["contracts"]["prepush_review_v1"]["status"],
        "pass"
    );
    assert_eq!(
        canonical["operator_overrides"][0]["to_status"],
        "implemented"
    );
}

#[tokio::test]
async fn proposal_057_runs_get_exposes_canonical_artifact_contract_parity() {
    let pool = create_pool("sqlite::memory:").await.unwrap();
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
            generation_id: "gen-runs-get".into(),
            source_agent_execution_id: None,
            source_stage_execution_id: None,
            source_session_generation_id: None,
            source_work_item_id: None,
            supersedes_generation_id: None,
            output_settlement:
                domain::agent::AgentOutputSettlement::ValidOutputsFromFailedExecution,
            partial: true,
            warnings: vec!["accepted by degraded-output policy".into()],
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
            journal_id: "journal-runs-get".into(),
        },
    )
    .await
    .unwrap();

    let payload = mcp_server::tools::runs::execute(
        "runs.get",
        serde_json::json!({"run_id": run_id.to_string()}),
        &pool,
        &engine::command_handler::CommandHandler::new(
            pool.clone(),
            engine::event_bus::new_bus(16),
            engine::work_queue::WorkQueue::new(pool.clone()),
        ),
        &principal(auth::PrincipalClass::Operator),
    )
    .await
    .unwrap();

    assert_eq!(
        payload["active_artifact_index"]["contracts"]["prepush_review_v1"]["status"],
        "pass"
    );
    assert_eq!(
        payload["active_artifact_index"]["contracts"]["prepush_review_v1"]["output_settlement"],
        "valid_outputs_from_failed_execution"
    );
    assert_eq!(
        payload["run_state_projection"]["active_index_owner"],
        "sqlite"
    );
    assert_eq!(payload["operator_overrides"][0]["to_status"], "implemented");
}

#[tokio::test]
async fn proposal_057_override_tool_is_operator_only() {
    assert_eq!(
        mcp_server::tools::capability_id_for("artifacts.override_contract"),
        Some(domain::CapabilityToolId::ArtifactsOverrideContract)
    );
    assert!(
        auth::filter_tools(
            &principal(auth::PrincipalClass::Operator),
            &[domain::CapabilityToolId::ArtifactsOverrideContract]
        )
        .len()
            == 1
    );
    assert!(auth::filter_tools(
        &principal(auth::PrincipalClass::Observer),
        &[domain::CapabilityToolId::ArtifactsOverrideContract]
    )
    .is_empty());

    let pool = create_pool("sqlite::memory:").await.unwrap();
    let result = mcp_server::tools::artifacts::execute(
        "artifacts.override_contract",
        serde_json::json!({
            "run_id": RunId::new().to_string(),
            "contract_id": "audit_report_v1",
            "override_type": "implementation_status",
            "from_status": "needs_code_fixes",
            "to_status": "implemented",
            "reason": "operator verified",
            "source_artifacts": [],
            "expires_at_stage": "state_11_manual_release",
        }),
        &pool,
        &engine::command_handler::CommandHandler::new(
            pool.clone(),
            engine::event_bus::new_bus(16),
            engine::work_queue::WorkQueue::new(pool.clone()),
        ),
        &principal(auth::PrincipalClass::Observer),
    )
    .await;
    assert!(
        result
            .unwrap_err()
            .to_string()
            .contains("requires operator principal"),
        "public artifacts.override_contract tool path must fail closed for non-operators"
    );
}

/// R12 API-002 / §9.3 AC-15: `artifacts.override_contract` must run
/// through `mcp_caller` so the ambient `MCP_REQUEST_ID` lands in the
/// command-journal row. Before this fix the tool built
/// `CallerContext::mcp(...)` directly and silently dropped the id.
#[tokio::test]
async fn artifacts_override_contract_attaches_ambient_mcp_request_id_to_journal() {
    use mcp_server::request_context::scope_request_id;

    let pool = create_pool("sqlite::memory:").await.unwrap();
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
            contract_id: "audit_report_v1".into(),
            canonical_path: "review/audit.json".into(),
            raw_path: "review/audit.json".into(),
            raw_status: "needs_code_fixes".into(),
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

    let handler = engine::command_handler::CommandHandler::new(
        pool.clone(),
        engine::event_bus::new_bus(16),
        engine::work_queue::WorkQueue::new(pool.clone()),
    );

    // Scope the MCP task-local the same way `handle_mcp_post` does
    // when an inbound HTTP request carries `X-Request-ID: rid-42`.
    let payload = scope_request_id(Some("rid-42".into()), async {
        mcp_server::tools::artifacts::execute(
            "artifacts.override_contract",
            serde_json::json!({
                "run_id": run_id.to_string(),
                "contract_id": "audit_report_v1",
                "override_type": "implementation_status",
                "from_status": "needs_code_fixes",
                "to_status": "implemented",
                "reason": "operator verified end-to-end",
                "source_artifacts": [],
                "expires_at_stage": "state_11_manual_release",
            }),
            &pool,
            &handler,
            &principal(auth::PrincipalClass::Operator),
        )
        .await
        .unwrap()
    })
    .await;

    let journal_id = payload["journal_id"]
        .as_str()
        .expect("journal_id on successful override response");
    let recorded = db::repos::command_journal::find_request_id(&pool, journal_id)
        .await
        .unwrap();
    assert_eq!(
        recorded.as_deref(),
        Some("rid-42"),
        "artifacts.override_contract must thread the ambient MCP request id \
         into command_journal.request_id via mcp_caller"
    );
}
